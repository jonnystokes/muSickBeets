
use std::rc::Rc;
use std::cell::RefCell;

use fltk::{app, enums::{Event, Font}, prelude::*};

use crate::app_state::AppState;
use crate::data::{self, TimeUnit};
use crate::app_state::format_time;
use crate::layout::Widgets;
use crate::ui::theme;

// ═══════════════════════════════════════════════════════════════════════════
//  DRAW CALLBACKS
// ═══════════════════════════════════════════════════════════════════════════

pub fn setup_draw_callbacks(
    widgets: &Widgets,
    state: &Rc<RefCell<AppState>>,
) {
    setup_spectrogram_draw(widgets, state);
    setup_spectrogram_mouse(widgets, state);
    setup_waveform_draw(widgets, state);
    setup_freq_axis_draw(widgets, state);
    setup_time_axis_draw(widgets, state);
}

// ── Spectrogram display ──
fn setup_spectrogram_draw(
    widgets: &Widgets,
    state: &Rc<RefCell<AppState>>,
) {
    let state = state.clone();

    let mut spec_display = widgets.spec_display.clone();
    spec_display.draw(move |w| {
        if !w.visible_r() || w.w() <= 0 || w.h() <= 0 {
            return;
        }

        // Extract everything we need from state, then DROP the borrow immediately.
        // This is critical: axis draw callbacks also need to borrow state in the
        // same paint cycle, and holding borrow_mut here blocks them.
        let draw_data = {
            let Ok(mut st) = state.try_borrow_mut() else { return; };
            if let Some(spec) = st.spectrogram.clone() {
                let view = st.view.clone();
                let proc_time_min = match st.fft_params.time_unit {
                    TimeUnit::Seconds => st.fft_params.start_time,
                    TimeUnit::Samples => st.fft_params.start_time / st.fft_params.sample_rate.max(1) as f64,
                };
                let proc_time_max = match st.fft_params.time_unit {
                    TimeUnit::Seconds => st.fft_params.stop_time,
                    TimeUnit::Samples => st.fft_params.stop_time / st.fft_params.sample_rate.max(1) as f64,
                };
                st.spec_renderer.draw(&spec, &view, proc_time_min, proc_time_max, w.x(), w.y(), w.w(), w.h());

                let cursor_cx = if st.transport.duration_seconds > 0.0 {
                    let playback_time = st.recon_start_time + st.audio_player.get_position_seconds();
                    let cursor_t = st.view.time_to_x(playback_time);
                    if cursor_t >= 0.0 && cursor_t <= 1.0 {
                        Some(w.x() + (cursor_t * w.w() as f64) as i32)
                    } else { None }
                } else { None };
                // borrow_mut dropped here at end of block
                Some(cursor_cx)
            } else {
                None
            }
        };
        // State borrow is now released — axis callbacks can borrow freely.

        match draw_data {
            Some(cursor_cx) => {
                if let Some(cx) = cursor_cx {
                    fltk::draw::set_draw_color(theme::color(theme::ACCENT_RED));
                    fltk::draw::draw_line(cx, w.y(), cx, w.y() + w.h());
                }
            }
            None => {
                fltk::draw::set_draw_color(theme::color(theme::BG_DARK));
                fltk::draw::draw_rectf(w.x(), w.y(), w.w(), w.h());
                fltk::draw::set_draw_color(theme::color(theme::TEXT_DISABLED));
                fltk::draw::set_font(Font::Helvetica, 14);
                fltk::draw::draw_text("Load an audio file to begin", w.x() + 10, w.y() + w.h() / 2);
            }
        }
    });
}

// ── Spectrogram mouse handling (seek + hover readout + zoom) ──
fn setup_spectrogram_mouse(
    widgets: &Widgets,
    state: &Rc<RefCell<AppState>>,
) {
    let state = state.clone();
    let mut status_bar = widgets.status_bar.clone();
    let mut spec_display_c = widgets.spec_display.clone();
    let mut waveform_display_c = widgets.waveform_display.clone();
    let mut freq_axis_c = widgets.freq_axis.clone();
    let mut time_axis_c = widgets.time_axis.clone();

    let mut spec_display = widgets.spec_display.clone();
    spec_display.handle(move |w, ev| {
        match ev {
            Event::Push => {
                // Click to seek - convert spectrogram time to audio position
                let mx = app::event_x() - w.x();
                let t = mx as f64 / w.w() as f64;
                let st = state.borrow();
                let time = st.view.x_to_time(t);
                // Seek is relative to recon_start_time
                let audio_pos = (time - st.recon_start_time).max(0.0);
                st.audio_player.set_seeking(true);
                st.audio_player.seek_to(audio_pos);
                true
            }
            Event::Move => {
                // Hover readout
                let mx = app::event_x() - w.x();
                let my = app::event_y() - w.y();
                let tx_norm = mx as f64 / w.w() as f64;
                let ty_norm = 1.0 - (my as f32 / w.h() as f32);  // flip Y

                let st = state.borrow();
                let time = st.view.x_to_time(tx_norm);
                let freq = st.view.y_to_freq(ty_norm);

                if let Some(ref spec) = st.spectrogram {
                    if let Some(frame_idx) = spec.frame_at_time(time) {
                        if let Some(bin_idx) = spec.bin_at_freq(freq) {
                            if let Some(mag) = spec.frames.get(frame_idx)
                                .and_then(|f| f.magnitudes.get(bin_idx))
                            {
                                let db = data::Spectrogram::magnitude_to_db(*mag);
                                status_bar.set_label(&format!(
                                    "Cursor: {:.1} Hz | {:.1} dB | {:.5}s",
                                    freq, db, time
                                ));
                            }
                        }
                    }
                }
                true
            }
            Event::MouseWheel => {
                let dy = app::event_dy();
                let mx = app::event_x() - w.x();
                let my = app::event_y() - w.y();

                // MouseWheel::Down = zoom out, Up = zoom in
                let zoom_in = matches!(dy, fltk::app::MouseWheel::Up);

                let mut st = state.borrow_mut();

                if app::event_state().contains(fltk::enums::Shortcut::Ctrl.into()) {
                    // Ctrl+wheel: zoom frequency axis
                    let focus_t = 1.0 - (my as f32 / w.h() as f32);
                    let focus_freq = st.view.y_to_freq(focus_t);

                    let mzf = st.mouse_zoom_factor;
                    let zoom_factor = if zoom_in { 1.0 / mzf } else { mzf };
                    let range = st.view.visible_freq_range();
                    let new_range = (range * zoom_factor).clamp(10.0, st.view.data_freq_max_hz);

                    let ratio = focus_t;
                    st.view.freq_min_hz = (focus_freq - new_range * ratio).max(1.0);
                    st.view.freq_max_hz = st.view.freq_min_hz + new_range;
                } else {
                    // Wheel: zoom time axis
                    let focus_t = mx as f64 / w.w() as f64;
                    let focus_time = st.view.x_to_time(focus_t);

                    let mzf = st.mouse_zoom_factor as f64;
                    let zoom_factor = if zoom_in { 1.0 / mzf } else { mzf };
                    let range = st.view.visible_time_range();
                    let new_range = (range * zoom_factor).clamp(
                        0.001,
                        st.view.data_time_max_sec - st.view.data_time_min_sec
                    );

                    let ratio = focus_t;
                    st.view.time_min_sec = (focus_time - new_range * ratio).max(st.view.data_time_min_sec);
                    st.view.time_max_sec = st.view.time_min_sec + new_range;
                    if st.view.time_max_sec > st.view.data_time_max_sec {
                        st.view.time_max_sec = st.view.data_time_max_sec;
                        st.view.time_min_sec = (st.view.time_max_sec - new_range).max(st.view.data_time_min_sec);
                    }
                }

                st.spec_renderer.invalidate();
                st.wave_renderer.invalidate();
                drop(st);
                spec_display_c.redraw();
                waveform_display_c.redraw();
                freq_axis_c.redraw();
                time_axis_c.redraw();
                true
            }
            Event::Drag => {
                // Drag for seeking
                let mx = app::event_x() - w.x();
                let t = mx as f64 / w.w() as f64;
                let st = state.borrow();
                let time = st.view.x_to_time(t);
                let audio_pos = (time - st.recon_start_time).max(0.0);
                st.audio_player.seek_to(audio_pos);
                true
            }
            Event::Released => {
                // End seeking - allow normal end-of-track behavior
                let st = state.borrow();
                st.audio_player.set_seeking(false);
                true
            }
            _ => false,
        }
    });
}

// ── Waveform display ──
fn setup_waveform_draw(
    widgets: &Widgets,
    state: &Rc<RefCell<AppState>>,
) {
    let state = state.clone();

    let mut waveform_display = widgets.waveform_display.clone();
    waveform_display.draw(move |w| {
        if !w.visible_r() || w.w() <= 0 || w.h() <= 0 {
            return;
        }

        // Borrow mut, do all work, then release borrow before returning.
        // This is critical: axis draw callbacks also need to borrow state in the
        // same paint cycle, and holding borrow_mut here blocks them.
        {
            let Ok(mut st) = state.try_borrow_mut() else { return; };

            let cursor_x = if st.transport.duration_seconds > 0.0 {
                let playback_time = st.recon_start_time + st.audio_player.get_position_seconds();
                let t = st.view.time_to_x(playback_time);
                if t >= 0.0 && t <= 1.0 {
                    Some((t * w.w() as f64) as i32)
                } else {
                    None
                }
            } else {
                None
            };

            let view = st.view.clone();
            let audio_opt = st.reconstructed_audio.take();
            let recon_start = st.recon_start_time;
            if let Some(ref audio) = audio_opt {
                st.wave_renderer.draw(&audio.samples, audio.sample_rate, recon_start, &view, cursor_x, w.x(), w.y(), w.w(), w.h());
            } else {
                st.wave_renderer.draw(&[], 44100, 0.0, &view, cursor_x, w.x(), w.y(), w.w(), w.h());
            }
            st.reconstructed_audio = audio_opt;
        }
        // State borrow released — axis callbacks can borrow freely.
    });
}

// ── Frequency axis labels ──
fn setup_freq_axis_draw(
    widgets: &Widgets,
    state: &Rc<RefCell<AppState>>,
) {
    let state = state.clone();

    let mut freq_axis = widgets.freq_axis.clone();
    freq_axis.draw(move |w| {
        if !w.visible_r() || w.w() <= 0 || w.h() <= 0 {
            return;
        }

        fltk::draw::set_draw_color(theme::color(theme::BG_DARK));
        fltk::draw::draw_rectf(w.x(), w.y(), w.w(), w.h());

        let Ok(st) = state.try_borrow() else { return; };
        if st.spectrogram.is_none() { return; }

        // Generate frequency ticks locked to Hz values (stable during scrolling)
        let ticks = generate_freq_ticks(
            st.view.freq_min_hz,
            st.view.freq_max_hz,
            &|f| st.view.freq_to_y(f),
            w.h(),
        );

        fltk::draw::set_font(Font::Helvetica, 9);
        for &(freq, y_norm) in &ticks {
            let py = w.y() + w.h() - (y_norm * w.h() as f32) as i32;

            // Notch tick mark (right-aligned, pointing toward spectrogram)
            fltk::draw::set_draw_color(theme::color(theme::BORDER));
            fltk::draw::draw_line(w.x() + w.w() - 6, py, w.x() + w.w(), py);

            // Label text - always integers with commas
            let label = format_freq_label(freq);
            fltk::draw::set_draw_color(theme::color(theme::TEXT_SECONDARY));
            fltk::draw::draw_text(&label, w.x() + 2, py + 3);
        }

        // Draw boundary lines for recon freq range
        fltk::draw::set_draw_color(fltk::enums::Color::from_hex(0xf9e2af));
        let recon_min_t = st.view.freq_to_y(st.view.recon_freq_min_hz);
        if recon_min_t > 0.01 && recon_min_t < 0.99 {
            let py = w.y() + w.h() - (recon_min_t * w.h() as f32) as i32;
            fltk::draw::set_line_style(fltk::draw::LineStyle::Dash, 1);
            fltk::draw::draw_line(w.x(), py, w.x() + w.w(), py);
            fltk::draw::set_line_style(fltk::draw::LineStyle::Solid, 0);
        }
        let recon_max_t = st.view.freq_to_y(st.view.recon_freq_max_hz);
        if recon_max_t > 0.01 && recon_max_t < 0.99 {
            let py = w.y() + w.h() - (recon_max_t * w.h() as f32) as i32;
            fltk::draw::set_line_style(fltk::draw::LineStyle::Dash, 1);
            fltk::draw::draw_line(w.x(), py, w.x() + w.w(), py);
            fltk::draw::set_line_style(fltk::draw::LineStyle::Solid, 0);
        }
    });
}

// ── Time axis labels ──
fn setup_time_axis_draw(
    widgets: &Widgets,
    state: &Rc<RefCell<AppState>>,
) {
    let state = state.clone();

    let mut time_axis = widgets.time_axis.clone();
    time_axis.draw(move |w| {
        if !w.visible_r() || w.w() <= 0 || w.h() <= 0 {
            return;
        }

        fltk::draw::set_draw_color(theme::color(theme::BG_DARK));
        fltk::draw::draw_rectf(w.x(), w.y(), w.w(), w.h());

        let Ok(st) = state.try_borrow() else { return; };
        if st.spectrogram.is_none() { return; }

        fltk::draw::set_draw_color(theme::color(theme::TEXT_SECONDARY));
        fltk::draw::set_font(Font::Helvetica, 9);

        // Smart adaptive time labels: target ~1 label per 80px, find a nice step
        let range = st.view.visible_time_range();
        let target_labels = ((w.w() - 50) as f64 / 80.0).max(2.0);
        let raw_step = range / target_labels;
        // Snap to nice step values
        let nice_steps = [0.001, 0.002, 0.005, 0.01, 0.02, 0.05, 0.1, 0.2, 0.5,
                         1.0, 2.0, 5.0, 10.0, 15.0, 30.0, 60.0, 120.0, 300.0, 600.0];
        let step = nice_steps.iter()
            .find(|&&s| s >= raw_step)
            .copied()
            .unwrap_or(raw_step);

        let mut t = (st.view.time_min_sec / step).ceil() * step;
        while t <= st.view.time_max_sec {
            let x_norm = st.view.time_to_x(t);
            let px = w.x() + 50 + ((x_norm * (w.w() - 50) as f64) as i32);  // offset for freq axis
            let label = if step < 0.01 {
                format!("{:.3}s", t)
            } else if step < 0.1 {
                format!("{:.2}s", t)
            } else if step < 1.0 {
                format!("{:.1}s", t)
            } else {
                format_time(t)
            };
            fltk::draw::draw_text(&label, px - 15, w.y() + 14);

            // Tick mark
            fltk::draw::set_draw_color(theme::color(theme::BORDER));
            fltk::draw::draw_line(px, w.y(), px, w.y() + 4);
            fltk::draw::set_draw_color(theme::color(theme::TEXT_SECONDARY));

            t += step;
        }

        // Draw boundary lines for processing time range
        let proc_start = match st.fft_params.time_unit {
            TimeUnit::Seconds => st.fft_params.start_time,
            TimeUnit::Samples => st.fft_params.start_time / st.fft_params.sample_rate.max(1) as f64,
        };
        let proc_stop = match st.fft_params.time_unit {
            TimeUnit::Seconds => st.fft_params.stop_time,
            TimeUnit::Samples => st.fft_params.stop_time / st.fft_params.sample_rate.max(1) as f64,
        };
        fltk::draw::set_draw_color(fltk::enums::Color::from_hex(0xf9e2af)); // accent yellow
        let t_start = st.view.time_to_x(proc_start);
        if t_start > 0.01 && t_start < 0.99 {
            let px = w.x() + 50 + ((t_start * (w.w() - 50) as f64) as i32);
            fltk::draw::set_line_style(fltk::draw::LineStyle::Dash, 1);
            fltk::draw::draw_line(px, w.y(), px, w.y() + w.h());
            fltk::draw::set_line_style(fltk::draw::LineStyle::Solid, 0);
        }
        let t_stop = st.view.time_to_x(proc_stop);
        if t_stop > 0.01 && t_stop < 0.99 {
            let px = w.x() + 50 + ((t_stop * (w.w() - 50) as f64) as i32);
            fltk::draw::set_line_style(fltk::draw::LineStyle::Dash, 1);
            fltk::draw::draw_line(px, w.y(), px, w.y() + w.h());
            fltk::draw::set_line_style(fltk::draw::LineStyle::Solid, 0);
        }
    });
}

// ═══════════════════════════════════════════════════════════════════════════
//  FREQUENCY TICK GENERATION (PIXEL-SPACE-FIRST)
// ═══════════════════════════════════════════════════════════════════════════

/// Generate frequency axis ticks by working BACKWARDS from evenly-spaced pixels.
/// 
/// This approach works universally for Linear, Log, and Power-blended scales without
/// any mode-specific logic. The key insight: start with even pixel spacing, convert
/// to frequencies using the inverse mapping, then round to nice values.
/// 
/// Algorithm:
/// 1. Generate evenly-spaced Y positions in normalized space (0.0 to 1.0)
/// 2. Convert each Y to frequency using y_to_freq() - handles ALL scaling modes
/// 3. Round each frequency to a nearby "nice" value
/// 4. Remove duplicates that may arise from rounding
/// 5. Convert back to Y positions for final rendering via freq_to_y()
/// 
/// Ticks are LOCKED to frequency values - scrolling moves them smoothly in pixel space.
/// Recalculation happens when: zoom changes, window resizes, or scale slider moves.
fn generate_freq_ticks(
    freq_min_hz: f32,
    freq_max_hz: f32,
    freq_to_y: &dyn Fn(f32) -> f32,
    widget_h: i32,
) -> Vec<(f32, f32)> {
    // Target spacing: approximately 45 pixels between ticks
    let desired_pixel_gap = 45.0;
    let target_count = (widget_h as f32 / desired_pixel_gap).max(3.0).min(20.0) as usize;
    
    if target_count == 0 {
        return vec![];
    }
    
    // Step 1: Generate evenly-spaced Y positions (normalized 0.0 to 1.0, bottom to top)
    let margin = 0.03; // 3% margin top and bottom
    let usable = 1.0 - 2.0 * margin;
    let y_step = if target_count <= 1 { usable } else { usable / (target_count - 1) as f32 };
    
    let y_positions: Vec<f32> = (0..target_count)
        .map(|i| margin + i as f32 * y_step)
        .collect();
    
    // Step 2: Convert Y positions to raw frequencies using INVERSE mapping
    // This is where the magic happens - binary search inversion handles all scaling modes
    let raw_freqs: Vec<f32> = y_positions.iter()
        .map(|&y| y_to_freq_inverse(y, freq_min_hz, freq_max_hz, freq_to_y))
        .collect();
    
    // Step 3: Round each frequency to a nice value
    // Calculate local step size to determine appropriate rounding
    let nice_freqs: Vec<f32> = raw_freqs.iter().enumerate()
        .map(|(i, &freq)| {
            // Determine local step by looking at neighbors
            let local_step = if i + 1 < raw_freqs.len() {
                raw_freqs[i + 1] - freq
            } else if i > 0 {
                freq - raw_freqs[i - 1]
            } else {
                (freq_max_hz - freq_min_hz) / target_count as f32
            };
            
            round_to_nice_freq(freq, local_step)
        })
        .collect();
    
    // Step 4: Remove duplicates that may have resulted from rounding
    let mut unique_freqs: Vec<f32> = Vec::new();
    for &freq in &nice_freqs {
        if unique_freqs.is_empty() || (freq - *unique_freqs.last().unwrap()).abs() > 0.5 {
            unique_freqs.push(freq);
        }
    }
    
    // Step 5: Convert back to Y positions for rendering
    let final_ticks: Vec<(f32, f32)> = unique_freqs.iter()
        .map(|&freq| (freq, freq_to_y(freq)))
        .collect();
    
    final_ticks
}

/// Inverse mapping from normalized Y (0.0 to 1.0, bottom to top) to frequency.
/// Uses binary search to invert the freq_to_y function, which handles all scaling modes.
/// This is more robust than duplicating the scaling logic from ViewState.
fn y_to_freq_inverse(
    y_target: f32,
    freq_min_hz: f32,
    freq_max_hz: f32,
    freq_to_y: &dyn Fn(f32) -> f32,
) -> f32 {
    // Binary search to find frequency that maps to y_target
    let mut low = freq_min_hz;
    let mut high = freq_max_hz;
    
    // 20 iterations gives us precision better than 1/1,000,000 of the range
    for _ in 0..20 {
        let mid = (low + high) / 2.0;
        let y_mid = freq_to_y(mid);
        
        if y_mid < y_target {
            low = mid;
        } else {
            high = mid;
        }
    }
    
    (low + high) / 2.0
}

/// Round a frequency to a "nice" value based on the local step size.
/// Uses the 1-2-5 pattern to find appropriate rounding granularity.
fn round_to_nice_freq(freq: f32, local_step: f32) -> f32 {
    if local_step <= 0.0 {
        return freq.round();
    }
    
    // Determine rounding granularity from step size
    let granularity = nice_step_value(local_step);
    
    // Round to nearest multiple of granularity
    (freq / granularity).round() * granularity
}

/// Format a frequency value as an integer with commas.
/// Never uses "k" suffix - always shows full number with thousand separators.
/// 
/// Examples:
/// - 100 → "100"
/// - 1000 → "1,000"
/// - 21100 → "21,100"
/// - 211000 → "211,000"
fn format_freq_label(freq: f32) -> String {
    let rounded = freq.round() as i64;
    format_with_commas(rounded)
}

/// Format an integer with comma thousand separators.
fn format_with_commas(n: i64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    let chars: Vec<char> = s.chars().collect();
    
    for (i, ch) in chars.iter().enumerate() {
        if i > 0 && (chars.len() - i) % 3 == 0 {
            result.push(',');
        }
        result.push(*ch);
    }
    
    result
}

/// Compute a "nice" step value using the 1-2-5 pattern across decades.
/// 
/// Given any raw step size, rounds it UP to the nearest value from:
/// ..., 0.5, 1, 2, 5, 10, 20, 50, 100, 200, 500, 1000, 2000, 5000, ...
/// 
/// This ensures tick spacing uses human-friendly round numbers.
fn nice_step_value(raw: f32) -> f32 {
    if raw <= 0.0 { return 1.0; }
    let exp = raw.log10().floor();
    let mag = 10f32.powf(exp);
    let m = raw / mag;
    let nice = if m <= 1.0 { 1.0 }
        else if m <= 2.0 { 2.0 }
        else if m <= 5.0 { 5.0 }
        else { 10.0 };
    nice * mag
}

