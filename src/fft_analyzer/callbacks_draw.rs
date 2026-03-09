use std::cell::RefCell;
use std::rc::Rc;

use fltk::{
    app,
    enums::{Event, Font},
    prelude::*,
};

use crate::app_state::format_time;
use crate::app_state::AppState;
use crate::data;
use crate::debug_flags;
use crate::layout::Widgets;
use crate::ui::theme;

// ═══════════════════════════════════════════════════════════════════════════
//  DRAW CALLBACKS
// ═══════════════════════════════════════════════════════════════════════════

pub fn setup_draw_callbacks(widgets: &Widgets, state: &Rc<RefCell<AppState>>) {
    setup_spectrogram_draw(widgets, state);
    setup_spectrogram_mouse(widgets, state);
    setup_waveform_draw(widgets, state);
    setup_freq_axis_draw(widgets, state);
    setup_time_axis_draw(widgets, state);
    setup_scrubber_draw(widgets, state);
}

// ── Spectrogram display ──
fn setup_spectrogram_draw(widgets: &Widgets, state: &Rc<RefCell<AppState>>) {
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
            let Ok(mut st) = state.try_borrow_mut() else {
                dbg_log!(
                    debug_flags::RENDER_DBG,
                    "Render",
                    "Spectrogram draw skipped: state borrow conflict"
                );
                return;
            };
            let overview_spec = st.overview_spectrogram.clone();
            let focus_spec = st.focus_spectrogram.clone();
            let legacy_spec = st.spectrogram.clone();
            if overview_spec.is_some() || focus_spec.is_some() || legacy_spec.is_some() {
                let view = st.view.clone();
                let focus_params = st
                    .focus_spec_params
                    .clone()
                    .unwrap_or_else(|| st.fft_params.clone());
                let overview_params = st
                    .overview_spec_params
                    .clone()
                    .or_else(|| {
                        st.audio_data
                            .as_ref()
                            .map(|a| st.overview_params_for_audio(a.num_samples()))
                    })
                    .unwrap_or_else(|| st.overview_fft_defaults.clone());
                let proc_time_min = st.fft_params.start_seconds();
                let proc_time_max = st.fft_params.stop_seconds();
                let render_full_file_outside_roi = st.render_full_file_outside_roi;
                let ww = w.w().max(1);
                let wh = w.h().max(1);

                let first_px = (0..ww).find(|&px| {
                    let t = px as f64 / ww as f64;
                    let time = view.x_to_time(t);
                    time >= proc_time_min && time <= proc_time_max
                });
                let last_px = (0..ww).rfind(|&px| {
                    let t = px as f64 / ww as f64;
                    let time = view.x_to_time(t);
                    time >= proc_time_min && time <= proc_time_max
                });

                let first_py = (0..wh).find(|&py| {
                    let flipped_py = wh - 1 - py;
                    let t = flipped_py as f32 / wh as f32;
                    let freq = view.y_to_freq(t);
                    freq >= view.recon_freq_min_hz && freq <= view.recon_freq_max_hz
                });
                let last_py = (0..wh).rfind(|&py| {
                    let flipped_py = wh - 1 - py;
                    let t = flipped_py as f32 / wh as f32;
                    let freq = view.y_to_freq(t);
                    freq >= view.recon_freq_min_hz && freq <= view.recon_freq_max_hz
                });

                let roi_clip = match (first_px, last_px, first_py, last_py) {
                    (Some(px0), Some(px1), Some(py0), Some(py1)) if px1 >= px0 && py1 >= py0 => {
                        Some((w.x() + px0, w.y() + py0, px1 - px0 + 1, py1 - py0 + 1))
                    }
                    _ => None,
                };

                if let Some(spec) = overview_spec.or_else(|| legacy_spec.clone()) {
                    if focus_spec.is_some() {
                        if let Some((clip_x, clip_y, clip_w, clip_h)) = roi_clip {
                            let right_x = clip_x + clip_w;
                            let bottom_y = clip_y + clip_h;
                            let outside_regions = [
                                (w.x(), w.y(), w.w(), (clip_y - w.y()).max(0)),
                                (w.x(), bottom_y, w.w(), (w.y() + w.h() - bottom_y).max(0)),
                                (w.x(), clip_y, (clip_x - w.x()).max(0), clip_h),
                                (right_x, clip_y, (w.x() + w.w() - right_x).max(0), clip_h),
                            ];

                            for (cx, cy, cw, ch) in outside_regions {
                                if cw > 0 && ch > 0 {
                                    fltk::draw::push_clip(cx, cy, cw, ch);
                                    st.overview_spec_renderer.draw(
                                        &spec,
                                        &view,
                                        &overview_params,
                                        proc_time_min,
                                        proc_time_max,
                                        render_full_file_outside_roi,
                                        w.x(),
                                        w.y(),
                                        w.w(),
                                        w.h(),
                                    );
                                    fltk::draw::pop_clip();
                                }
                            }
                        } else {
                            st.overview_spec_renderer.draw(
                                &spec,
                                &view,
                                &overview_params,
                                proc_time_min,
                                proc_time_max,
                                render_full_file_outside_roi,
                                w.x(),
                                w.y(),
                                w.w(),
                                w.h(),
                            );
                        }
                    } else {
                        st.overview_spec_renderer.draw(
                            &spec,
                            &view,
                            &overview_params,
                            proc_time_min,
                            proc_time_max,
                            render_full_file_outside_roi,
                            w.x(),
                            w.y(),
                            w.w(),
                            w.h(),
                        );
                    }
                }

                if let Some(spec) = focus_spec.or(legacy_spec) {
                    if let Some((clip_x, clip_y, clip_w, clip_h)) = roi_clip {
                        fltk::draw::push_clip(clip_x, clip_y, clip_w, clip_h);
                        st.focus_spec_renderer.draw(
                            &spec,
                            &view,
                            &focus_params,
                            proc_time_min,
                            proc_time_max,
                            false,
                            w.x(),
                            w.y(),
                            w.w(),
                            w.h(),
                        );
                        fltk::draw::pop_clip();
                    }
                }

                let cursor_cx = if st.transport.duration_samples > 0 {
                    let playback_time =
                        st.recon_start_seconds() + st.audio_player.get_position_seconds();
                    let cursor_t = st.view.time_to_x(playback_time);
                    if (0.0..=1.0).contains(&cursor_t) {
                        Some(w.x() + (cursor_t * w.w() as f64) as i32)
                    } else {
                        None
                    }
                } else {
                    None
                };
                // borrow_mut dropped here at end of block
                Some(cursor_cx)
            } else {
                None
            }
        };
        // State borrow is now released — axis callbacks can borrow freely.

        match draw_data {
            Some(cursor_cx) => {
                let st = match state.try_borrow() {
                    Ok(st) => st,
                    Err(_) => return,
                };

                let time_to_x_unclamped = |time_sec: f64| {
                    let range = st.view.time_max_sec - st.view.time_min_sec;
                    if range <= 0.0 {
                        0.0
                    } else {
                        (time_sec - st.view.time_min_sec) / range
                    }
                };

                let freq_to_y_unclamped = |freq_hz: f32| {
                    let min = st.view.freq_min_hz.max(1.0);
                    let max = st.view.freq_max_hz.max(min + 1.0);
                    match st.view.freq_scale {
                        crate::data::FreqScale::Linear => (freq_hz - min) / (max - min),
                        crate::data::FreqScale::Log => (freq_hz / min).ln() / (max / min).ln(),
                        crate::data::FreqScale::Power(power) => {
                            let p = power.clamp(0.0, 1.0);
                            if p <= 0.001 {
                                (freq_hz - min) / (max - min)
                            } else if p >= 0.999 {
                                (freq_hz / min).ln() / (max / min).ln()
                            } else {
                                let eval_freq = |t: f32| {
                                    let linear_freq = min + (max - min) * t;
                                    let log_freq = min * (max / min).powf(t);
                                    linear_freq.powf(1.0 - p) * log_freq.powf(p)
                                };

                                let (mut lo, mut hi) = if freq_hz < min {
                                    let mut lo = -1.0_f32;
                                    let hi = 0.0_f32;
                                    while eval_freq(lo) > freq_hz {
                                        lo *= 2.0;
                                    }
                                    (lo, hi)
                                } else if freq_hz > max {
                                    let lo = 1.0_f32;
                                    let mut hi = 2.0_f32;
                                    while eval_freq(hi) < freq_hz {
                                        hi *= 2.0;
                                    }
                                    (lo, hi)
                                } else {
                                    (0.0_f32, 1.0_f32)
                                };

                                for _ in 0..40 {
                                    let mid = (lo + hi) / 2.0;
                                    let f = eval_freq(mid);
                                    if f < freq_hz {
                                        lo = mid;
                                    } else {
                                        hi = mid;
                                    }
                                }
                                (lo + hi) / 2.0
                            }
                        }
                    }
                };

                let roi_t0 = time_to_x_unclamped(st.fft_params.start_seconds());
                let roi_t1 = time_to_x_unclamped(st.fft_params.stop_seconds());
                let roi_f0 = freq_to_y_unclamped(st.view.recon_freq_min_hz);
                let roi_f1 = freq_to_y_unclamped(st.view.recon_freq_max_hz);

                if roi_t1 > roi_t0 && roi_f1 > roi_f0 {
                    let stroke_px = 3;

                    let roi_left = w.x() + (roi_t0 * w.w() as f64) as i32;
                    let roi_right = w.x() + (roi_t1 * w.w() as f64) as i32;
                    let roi_top = w.y() + ((1.0 - roi_f1) * w.h() as f32) as i32;
                    let roi_bottom = w.y() + ((1.0 - roi_f0) * w.h() as f32) as i32;

                    let vis_left = w.x();
                    let vis_right = w.x() + w.w();
                    let vis_top = w.y();
                    let vis_bottom = w.y() + w.h();

                    let horiz_x0 = roi_left.max(vis_left);
                    let horiz_x1 = roi_right.min(vis_right);
                    let vert_y0 = roi_top.max(vis_top);
                    let vert_y1 = roi_bottom.min(vis_bottom);
                    let left_edge_visible =
                        roi_left > vis_left + stroke_px && roi_left <= vis_right;
                    let right_edge_visible =
                        roi_right >= vis_left && roi_right < vis_right - stroke_px;
                    let has_visible_vertical_edge = left_edge_visible || right_edge_visible;

                    fltk::draw::set_draw_color(theme::color(theme::ACCENT_BLUE));

                    // Top edge: entirely outside the ROI, clipped to visible horizontal span.
                    if roi_top > vis_top + stroke_px
                        && roi_top <= vis_bottom
                        && horiz_x1 > horiz_x0
                        && has_visible_vertical_edge
                    {
                        fltk::draw::draw_rectf(
                            (horiz_x0 - stroke_px).max(vis_left),
                            roi_top - stroke_px,
                            ((horiz_x1 + stroke_px).min(vis_right)
                                - (horiz_x0 - stroke_px).max(vis_left))
                            .max(1),
                            stroke_px,
                        );
                    }

                    // Bottom edge: entirely outside the ROI.
                    if roi_bottom >= vis_top
                        && roi_bottom < vis_bottom - stroke_px
                        && horiz_x1 > horiz_x0
                        && has_visible_vertical_edge
                    {
                        fltk::draw::draw_rectf(
                            (horiz_x0 - stroke_px).max(vis_left),
                            roi_bottom,
                            ((horiz_x1 + stroke_px).min(vis_right)
                                - (horiz_x0 - stroke_px).max(vis_left))
                            .max(1),
                            stroke_px,
                        );
                    }

                    // Left edge: only draw if the actual left ROI boundary is visible.
                    if left_edge_visible && vert_y1 > vert_y0 {
                        fltk::draw::draw_rectf(
                            roi_left - stroke_px,
                            (vert_y0 - stroke_px).max(vis_top),
                            stroke_px,
                            ((vert_y1 + stroke_px).min(vis_bottom)
                                - (vert_y0 - stroke_px).max(vis_top))
                            .max(1),
                        );
                    }

                    // Right edge: only draw if the actual right ROI boundary is visible.
                    if right_edge_visible && vert_y1 > vert_y0 {
                        fltk::draw::draw_rectf(
                            roi_right,
                            (vert_y0 - stroke_px).max(vis_top),
                            stroke_px,
                            ((vert_y1 + stroke_px).min(vis_bottom)
                                - (vert_y0 - stroke_px).max(vis_top))
                            .max(1),
                        );
                    }
                }

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
fn setup_spectrogram_mouse(widgets: &Widgets, state: &Rc<RefCell<AppState>>) {
    let state = state.clone();
    let mut cursor_readout = widgets.cursor_readout.clone();
    let mut spec_display_c = widgets.spec_display.clone();
    let mut waveform_display_c = widgets.waveform_display.clone();
    let mut freq_axis_c = widgets.freq_axis.clone();
    let mut time_axis_c = widgets.time_axis.clone();

    let mut spec_display = widgets.spec_display.clone();
    spec_display.handle(move |w, ev| {
        match ev {
            Event::Enter => {
                // Must return true for Enter so FLTK sends Move events to this widget
                true
            }
            Event::Push => {
                // Click to seek - convert spectrogram time to audio position
                let mx = app::event_x() - w.x();
                let t = mx as f64 / w.w() as f64;
                let st = state.borrow();
                let time = st.view.x_to_time(t);
                // Seek is relative to recon_start_seconds
                let audio_pos = (time - st.recon_start_seconds()).max(0.0);
                st.audio_player.set_seeking(true);
                st.audio_player.seek_to(audio_pos);
                true
            }
            Event::Move => {
                // Hover readout
                let mx = app::event_x() - w.x();
                let my = app::event_y() - w.y();
                let tx_norm = mx as f64 / w.w() as f64;
                let ty_norm = 1.0 - (my as f32 / w.h() as f32); // flip Y

                let st = state.borrow();
                let time = st.view.x_to_time(tx_norm);
                let freq = st.view.y_to_freq(ty_norm);

                let in_time_roi =
                    time >= st.fft_params.start_seconds() && time <= st.fft_params.stop_seconds();
                let in_freq_roi =
                    freq >= st.view.recon_freq_min_hz && freq <= st.view.recon_freq_max_hz;
                let in_roi = in_time_roi && in_freq_roi;

                let hover_spec = if in_roi {
                    st.focus_spectrogram
                        .as_ref()
                        .or(st.spectrogram.as_ref())
                        .or(st.overview_spectrogram.as_ref())
                } else if st.render_full_file_outside_roi {
                    st.overview_spectrogram.as_ref().or(st.spectrogram.as_ref())
                } else {
                    None
                };

                if hover_spec.is_none() {
                    dbg_log!(debug_flags::CURSOR_DBG, "Cursor", "No spectrogram loaded");
                }

                if let Some(spec) = hover_spec {
                    let frame_idx_opt = spec.frame_at_time(time);
                    if frame_idx_opt.is_none() {
                        dbg_log!(
                            debug_flags::CURSOR_DBG,
                            "Cursor",
                            "frame_at_time({}) => None (frames={})",
                            time,
                            spec.num_frames()
                        );
                    }
                    if let Some(frame_idx) = frame_idx_opt {
                        let bin_idx_opt = spec.bin_at_freq(freq);
                        if bin_idx_opt.is_none() {
                            dbg_log!(
                                debug_flags::CURSOR_DBG,
                                "Cursor",
                                "bin_at_freq({}) => None (bins={})",
                                freq,
                                spec.num_bins()
                            );
                        }
                        if let Some(bin_idx) = bin_idx_opt {
                            if let Some(mag) = spec
                                .frames
                                .get(frame_idx)
                                .and_then(|f| f.magnitudes.get(bin_idx))
                            {
                                let db = data::Spectrogram::magnitude_to_db(*mag);
                                let text = format!("{:.1} Hz | {:.1} dB | {:.5}s", freq, db, time);
                                dbg_log!(
                                    debug_flags::CURSOR_DBG,
                                    "Cursor",
                                    "OK: {} | widget geom: x={} y={} w={} h={} visible={}",
                                    text,
                                    cursor_readout.x(),
                                    cursor_readout.y(),
                                    cursor_readout.w(),
                                    cursor_readout.h(),
                                    cursor_readout.visible()
                                );
                                cursor_readout.set_label(&text);
                                cursor_readout.redraw();
                            } else {
                                dbg_log!(
                                    debug_flags::CURSOR_DBG,
                                    "Cursor",
                                    "mag lookup failed: frame_idx={} bin_idx={}",
                                    frame_idx,
                                    bin_idx
                                );
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

                let modifiers = app::event_state();
                let has_ctrl = modifiers.contains(fltk::enums::Shortcut::Ctrl);
                let has_alt = modifiers.contains(fltk::enums::Shortcut::Alt);

                // Scroll direction: Up = positive, Down = negative
                let scroll_up = matches!(dy, fltk::app::MouseWheel::Up);

                let mut st = state.borrow_mut();

                // ── Navigation scheme ──
                // No modifier:     Pan frequency (scroll up = higher freq)
                // Ctrl:            Pan time (scroll up = earlier in time)
                // Alt:             Zoom frequency centered on cursor Y
                // Alt+Ctrl:        Zoom time centered on cursor X
                // swap_zoom_axes:  Swaps which axis Alt vs Alt+Ctrl zooms

                if has_alt {
                    // Alt held → ZOOM mode
                    let zoom_freq = if st.swap_zoom_axes {
                        has_ctrl
                    } else {
                        !has_ctrl
                    };

                    if zoom_freq {
                        // Zoom frequency axis centered on cursor Y
                        let focus_t = 1.0 - (my as f32 / w.h() as f32);
                        let focus_freq = st.view.y_to_freq(focus_t);

                        let mzf = st.mouse_zoom_factor;
                        let zoom_factor = if scroll_up { 1.0 / mzf } else { mzf };
                        let range = st.view.visible_freq_range();
                        let new_range = (range * zoom_factor).clamp(10.0, st.view.data_freq_max_hz);

                        let ratio = focus_t;
                        st.view.freq_min_hz = (focus_freq - new_range * ratio).max(1.0);
                        st.view.freq_max_hz = st.view.freq_min_hz + new_range;
                        if st.view.freq_max_hz > st.view.data_freq_max_hz {
                            st.view.freq_max_hz = st.view.data_freq_max_hz;
                            st.view.freq_min_hz = (st.view.freq_max_hz - new_range).max(1.0);
                        }
                    } else {
                        // Zoom time axis centered on cursor X
                        let focus_t = mx as f64 / w.w() as f64;
                        let focus_time = st.view.x_to_time(focus_t);

                        let mzf = st.mouse_zoom_factor as f64;
                        let zoom_factor = if scroll_up { 1.0 / mzf } else { mzf };
                        let range = st.view.visible_time_range();
                        let data_range = st.view.data_time_max_sec - st.view.data_time_min_sec;
                        let new_range = (range * zoom_factor).clamp(0.001, data_range);

                        let ratio = focus_t;
                        st.view.time_min_sec =
                            (focus_time - new_range * ratio).max(st.view.data_time_min_sec);
                        st.view.time_max_sec = st.view.time_min_sec + new_range;
                        if st.view.time_max_sec > st.view.data_time_max_sec {
                            st.view.time_max_sec = st.view.data_time_max_sec;
                            st.view.time_min_sec =
                                (st.view.time_max_sec - new_range).max(st.view.data_time_min_sec);
                        }
                    }
                } else if has_ctrl {
                    // Ctrl (no Alt) → Pan time axis
                    let range = st.view.visible_time_range();
                    let pan_step = range * 0.15; // 15% of visible range per scroll tick
                    let delta = if scroll_up { -pan_step } else { pan_step };

                    let data_min = st.view.data_time_min_sec;
                    let data_max = st.view.data_time_max_sec;

                    st.view.time_min_sec = (st.view.time_min_sec + delta).max(data_min);
                    st.view.time_max_sec = st.view.time_min_sec + range;
                    if st.view.time_max_sec > data_max {
                        st.view.time_max_sec = data_max;
                        st.view.time_min_sec = (data_max - range).max(data_min);
                    }
                } else {
                    // No modifier → Pan frequency axis
                    let range = st.view.visible_freq_range();
                    let pan_step = range * 0.15; // 15% of visible range per scroll tick
                    let delta = if scroll_up { pan_step } else { -pan_step };

                    let data_max = st.view.data_freq_max_hz;

                    st.view.freq_min_hz = (st.view.freq_min_hz + delta).max(1.0);
                    st.view.freq_max_hz = st.view.freq_min_hz + range;
                    if st.view.freq_max_hz > data_max {
                        st.view.freq_max_hz = data_max;
                        st.view.freq_min_hz = (data_max - range).max(1.0);
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
                let audio_pos = (time - st.recon_start_seconds()).max(0.0);
                st.audio_player.seek_to(audio_pos);
                true
            }
            Event::Released => {
                // End seeking - allow normal end-of-track behavior
                let st = state.borrow();
                st.audio_player.set_seeking(false);
                true
            }
            Event::Leave => {
                cursor_readout.set_label("");
                cursor_readout.redraw();
                true
            }
            _ => false,
        }
    });
}

// ── Waveform display ──
fn setup_waveform_draw(widgets: &Widgets, state: &Rc<RefCell<AppState>>) {
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
            let Ok(mut st) = state.try_borrow_mut() else {
                dbg_log!(
                    debug_flags::RENDER_DBG,
                    "Render",
                    "Waveform draw skipped: state borrow conflict"
                );
                return;
            };

            let cursor_x = if st.transport.duration_samples > 0 {
                let playback_time =
                    st.recon_start_seconds() + st.audio_player.get_position_seconds();
                let t = st.view.time_to_x(playback_time);
                if (0.0..=1.0).contains(&t) {
                    Some((t * w.w() as f64) as i32)
                } else {
                    None
                }
            } else {
                None
            };

            let view = st.view.clone();
            // take() temporarily moves audio out of AppState so we can pass an
            // immutable ref to wave_renderer.draw(&mut self) without conflicting
            // borrows. This is safe: FLTK is single-threaded, no other code can
            // observe the empty slot, and we immediately put it back. A panic here
            // would propagate through FLTK's C FFI (already UB), so the take/put
            // pattern adds no additional risk.
            let audio_opt = st.reconstructed_audio.take();
            let recon_start = st.recon_start_seconds();
            if let Some(ref audio) = audio_opt {
                st.wave_renderer.draw(
                    &audio.samples,
                    audio.sample_rate,
                    recon_start,
                    &view,
                    cursor_x,
                    w.x(),
                    w.y(),
                    w.w(),
                    w.h(),
                );
            } else {
                st.wave_renderer
                    .draw(&[], 44100, 0.0, &view, cursor_x, w.x(), w.y(), w.w(), w.h());
            }
            st.reconstructed_audio = audio_opt;
        }
        // State borrow released — axis callbacks can borrow freely.
    });
}

// ── Frequency axis labels ──
fn setup_freq_axis_draw(widgets: &Widgets, state: &Rc<RefCell<AppState>>) {
    let state = state.clone();

    let mut freq_axis = widgets.freq_axis.clone();
    freq_axis.draw(move |w| {
        if !w.visible_r() || w.w() <= 0 || w.h() <= 0 {
            return;
        }

        fltk::draw::set_draw_color(theme::color(theme::BG_DARK));
        fltk::draw::draw_rectf(w.x(), w.y(), w.w(), w.h());

        let Ok(st) = state.try_borrow() else {
            dbg_log!(
                debug_flags::RENDER_DBG,
                "Render",
                "Freq axis draw skipped: state borrow conflict"
            );
            return;
        };
        if st.active_spectrogram().is_none() {
            return;
        }

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
fn setup_time_axis_draw(widgets: &Widgets, state: &Rc<RefCell<AppState>>) {
    let state = state.clone();

    let mut time_axis = widgets.time_axis.clone();
    time_axis.draw(move |w| {
        if !w.visible_r() || w.w() <= 0 || w.h() <= 0 {
            return;
        }

        fltk::draw::set_draw_color(theme::color(theme::BG_DARK));
        fltk::draw::draw_rectf(w.x(), w.y(), w.w(), w.h());

        let Ok(st) = state.try_borrow() else {
            dbg_log!(
                debug_flags::RENDER_DBG,
                "Render",
                "Time axis draw skipped: state borrow conflict"
            );
            return;
        };
        if st.active_spectrogram().is_none() {
            return;
        }

        fltk::draw::set_draw_color(theme::color(theme::TEXT_SECONDARY));
        fltk::draw::set_font(Font::Helvetica, 9);

        let left_gutter = crate::layout::SPEC_LEFT_GUTTER_W;
        let right_gutter = crate::layout::SPEC_RIGHT_GUTTER_W;
        let drawable_w = (w.w() - left_gutter - right_gutter).max(1);

        // Smart adaptive time labels: target ~1 label per 80px, find a nice step
        let range = st.view.visible_time_range();
        let target_labels = (drawable_w as f64 / 80.0).max(2.0);
        let raw_step = range / target_labels;
        // Snap to nice step values
        let nice_steps = [
            0.001, 0.002, 0.005, 0.01, 0.02, 0.05, 0.1, 0.2, 0.5, 1.0, 2.0, 5.0, 10.0, 15.0, 30.0,
            60.0, 120.0, 300.0, 600.0,
        ];
        let step = nice_steps
            .iter()
            .find(|&&s| s >= raw_step)
            .copied()
            .unwrap_or(raw_step);

        let mut t = (st.view.time_min_sec / step).ceil() * step;
        while t <= st.view.time_max_sec {
            let x_norm = st.view.time_to_x(t);
            let px = w.x() + left_gutter + ((x_norm * drawable_w as f64) as i32);
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
        let proc_start = st.fft_params.start_seconds();
        let proc_stop = st.fft_params.stop_seconds();
        fltk::draw::set_draw_color(fltk::enums::Color::from_hex(0xf9e2af)); // accent yellow
        let t_start = st.view.time_to_x(proc_start);
        if t_start > 0.01 && t_start < 0.99 {
            let px = w.x() + left_gutter + ((t_start * drawable_w as f64) as i32);
            fltk::draw::set_line_style(fltk::draw::LineStyle::Dash, 1);
            fltk::draw::draw_line(px, w.y(), px, w.y() + w.h());
            fltk::draw::set_line_style(fltk::draw::LineStyle::Solid, 0);
        }
        let t_stop = st.view.time_to_x(proc_stop);
        if t_stop > 0.01 && t_stop < 0.99 {
            let px = w.x() + left_gutter + ((t_stop * drawable_w as f64) as i32);
            fltk::draw::set_line_style(fltk::draw::LineStyle::Dash, 1);
            fltk::draw::draw_line(px, w.y(), px, w.y() + w.h());
            fltk::draw::set_line_style(fltk::draw::LineStyle::Solid, 0);
        }
    });
}

// ── ROI-aware scrubber draw ──
fn setup_scrubber_draw(widgets: &Widgets, state: &Rc<RefCell<AppState>>) {
    let state = state.clone();

    let mut scrub_slider = widgets.scrub_slider.clone();
    scrub_slider.draw(move |w| {
        fltk::draw::set_draw_color(theme::color(theme::BG_WIDGET));
        fltk::draw::draw_rectf(w.x(), w.y(), w.w(), w.h());

        let Ok(st) = state.try_borrow() else {
            return;
        };

        let track_y = w.y() + w.h() / 2 - 2;
        let track_h = 4;

        // Base full-width track
        fltk::draw::set_draw_color(theme::color(theme::BG_PANEL));
        fltk::draw::draw_rectf(w.x(), track_y, w.w(), track_h);

        let roi_start = st.fft_params.start_seconds();
        let roi_stop = st.fft_params.stop_seconds();
        let roi_vis_start = st.view.time_min_sec.max(roi_start);
        let roi_vis_stop = st.view.time_max_sec.min(roi_stop);

        if roi_vis_stop > roi_vis_start {
            let x0 = w.x() + (st.view.time_to_x(roi_vis_start) * w.w() as f64) as i32;
            let x1 = w.x() + (st.view.time_to_x(roi_vis_stop) * w.w() as f64) as i32;
            let active_w = (x1 - x0).max(1);

            fltk::draw::set_draw_color(theme::color(theme::ACCENT_BLUE));
            fltk::draw::draw_rectf(x0, track_y, active_w, track_h);

            // Thumb only appears when the playback position is both inside the ROI
            // and inside the current viewport.
            if st.audio_player.has_audio() {
                let local_seconds = st.audio_player.get_position_samples() as f64
                    / st.transport.sample_rate.max(1) as f64;
                let global_seconds = st.recon_start_seconds() + local_seconds;
                if global_seconds >= roi_start
                    && global_seconds <= roi_stop
                    && global_seconds >= st.view.time_min_sec
                    && global_seconds <= st.view.time_max_sec
                {
                    let cx = w.x() + (st.view.time_to_x(global_seconds) * w.w() as f64) as i32;
                    fltk::draw::set_draw_color(theme::color(theme::ACCENT_RED));
                    fltk::draw::draw_rectf(cx - 2, w.y(), 4, w.h());
                }
            }
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
    let target_count = (widget_h as f32 / desired_pixel_gap).clamp(3.0, 20.0) as usize;

    if target_count == 0 {
        return vec![];
    }

    // Step 1: Generate evenly-spaced Y positions (normalized 0.0 to 1.0, bottom to top)
    let margin = 0.03; // 3% margin top and bottom
    let usable = 1.0 - 2.0 * margin;
    let y_step = if target_count <= 1 {
        usable
    } else {
        usable / (target_count - 1) as f32
    };

    let y_positions: Vec<f32> = (0..target_count)
        .map(|i| margin + i as f32 * y_step)
        .collect();

    // Step 2: Convert Y positions to raw frequencies using INVERSE mapping
    // This is where the magic happens - binary search inversion handles all scaling modes
    let raw_freqs: Vec<f32> = y_positions
        .iter()
        .map(|&y| y_to_freq_inverse(y, freq_min_hz, freq_max_hz, freq_to_y))
        .collect();

    // Step 3: Round each frequency to a nice value
    // Calculate local step size to determine appropriate rounding
    let nice_freqs: Vec<f32> = raw_freqs
        .iter()
        .enumerate()
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
    let final_ticks: Vec<(f32, f32)> = unique_freqs
        .iter()
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
    let (prefix, digits) = if n < 0 {
        ("-", (-n).to_string())
    } else {
        ("", n.to_string())
    };
    let chars: Vec<char> = digits.chars().collect();
    let mut result = String::from(prefix);

    for (i, ch) in chars.iter().enumerate() {
        if i > 0 && (chars.len() - i).is_multiple_of(3) {
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
    if raw <= 0.0 {
        return 1.0;
    }
    let exp = raw.log10().floor();
    let mag = 10f32.powf(exp);
    let m = raw / mag;
    let nice = if m <= 1.0 {
        1.0
    } else if m <= 2.0 {
        2.0
    } else if m <= 5.0 {
        5.0
    } else {
        10.0
    };
    nice * mag
}
