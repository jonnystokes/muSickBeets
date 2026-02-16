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

        let Ok(mut st) = state.try_borrow_mut() else { return; };

        if let Some(spec) = st.spectrogram.clone() {
            let view = st.view.clone();
            // Get processing time range from fft_params (sidebar Start/Stop)
            let proc_time_min = match st.fft_params.time_unit {
                TimeUnit::Seconds => st.fft_params.start_time,
                TimeUnit::Samples => st.fft_params.start_time / st.fft_params.sample_rate.max(1) as f64,
            };
            let proc_time_max = match st.fft_params.time_unit {
                TimeUnit::Seconds => st.fft_params.stop_time,
                TimeUnit::Samples => st.fft_params.stop_time / st.fft_params.sample_rate.max(1) as f64,
            };
            st.spec_renderer.draw(&spec, &view, proc_time_min, proc_time_max, w.x(), w.y(), w.w(), w.h());

            // Draw playback cursor (playback position is relative to recon_start_time)
            if st.transport.duration_seconds > 0.0 {
                let playback_time = st.recon_start_time + st.audio_player.get_position_seconds();
                let cursor_t = st.view.time_to_x(playback_time);
                if cursor_t >= 0.0 && cursor_t <= 1.0 {
                    let cx = w.x() + (cursor_t * w.w() as f64) as i32;
                    fltk::draw::set_draw_color(theme::color(theme::ACCENT_RED));
                    fltk::draw::draw_line(cx, w.y(), cx, w.y() + w.h());
                }
            }
        } else {
            fltk::draw::set_draw_color(theme::color(theme::BG_DARK));
            fltk::draw::draw_rectf(w.x(), w.y(), w.w(), w.h());
            fltk::draw::set_draw_color(theme::color(theme::TEXT_DISABLED));
            fltk::draw::set_font(Font::Helvetica, 14);
            fltk::draw::draw_text("Load an audio file to begin", w.x() + 10, w.y() + w.h() / 2);
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
                                    "Cursor: {:.1} Hz | {:.1} dB | {:.3}s",
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

        let Ok(mut st) = state.try_borrow_mut() else { return; };

        // Compute cursor position: playback is relative to recon_start_time
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

        // Clone view and take audio out temporarily to avoid simultaneous mut/immut borrow
        let view = st.view.clone();
        let audio_opt = st.reconstructed_audio.take();
        let recon_start = st.recon_start_time;
        if let Some(ref audio) = audio_opt {
            st.wave_renderer.draw(&audio.samples, audio.sample_rate, recon_start, &view, cursor_x, w.x(), w.y(), w.w(), w.h());
        } else {
            st.wave_renderer.draw(&[], 44100, 0.0, &view, cursor_x, w.x(), w.y(), w.w(), w.h());
        }
        st.reconstructed_audio = audio_opt;
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

        fltk::draw::set_draw_color(theme::color(theme::TEXT_SECONDARY));
        fltk::draw::set_font(Font::Helvetica, 9);

        // Smart adaptive frequency labels that work for any scale mode.
        // Target: ~1 label per 40 pixels of height, minimum spacing 25px.
        let min_spacing_px = 25;
        let target_labels = (w.h() / 40).max(3).min(20) as usize;

        // Generate candidate frequencies using "nice" numbers
        let nice_freqs: Vec<f32> = vec![
            10.0, 15.0, 20.0, 25.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0, 90.0,
            100.0, 125.0, 150.0, 175.0, 200.0, 250.0, 300.0, 350.0, 400.0, 450.0, 500.0,
            600.0, 700.0, 800.0, 900.0,
            1000.0, 1250.0, 1500.0, 1750.0, 2000.0, 2500.0, 3000.0, 3500.0, 4000.0, 4500.0, 5000.0,
            6000.0, 7000.0, 8000.0, 9000.0,
            10000.0, 12000.0, 14000.0, 16000.0, 18000.0, 20000.0, 22000.0, 24000.0,
        ];

        // Filter to visible range and convert to pixel positions
        let candidates: Vec<(f32, i32)> = nice_freqs.iter()
            .filter(|&&f| f >= st.view.freq_min_hz && f <= st.view.freq_max_hz)
            .map(|&f| {
                let t = st.view.freq_to_y(f);
                let py = w.y() + w.h() - (t * w.h() as f32) as i32;
                (f, py)
            })
            .collect();

        // Greedily select labels with minimum spacing
        let mut selected: Vec<(f32, i32)> = Vec::new();
        for &(freq, py) in &candidates {
            let too_close = selected.iter().any(|&(_, spy)| (py - spy).abs() < min_spacing_px);
            if !too_close {
                selected.push((freq, py));
            }
            if selected.len() >= target_labels { break; }
        }

        // Draw the selected labels
        for &(freq, py) in &selected {
            let label = format_freq_label(freq);
            fltk::draw::set_draw_color(theme::color(theme::TEXT_SECONDARY));
            fltk::draw::draw_text(&label, w.x() + 2, py + 3);

            // Tick mark
            fltk::draw::set_draw_color(theme::color(theme::BORDER));
            fltk::draw::draw_line(w.x() + w.w() - 4, py, w.x() + w.w(), py);
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

/// Format a frequency value as a readable label.
fn format_freq_label(freq: f32) -> String {
    if freq >= 1000.0 {
        let k = freq / 1000.0;
        if k == k.floor() {
            format!("{}k", k as i32)
        } else {
            format!("{:.1}k", k)
        }
    } else if freq >= 100.0 {
        format!("{}", freq as i32)
    } else if freq == freq.floor() {
        format!("{}", freq as i32)
    } else {
        format!("{:.1}", freq)
    }
}
