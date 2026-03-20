use std::cell::RefCell;
use std::rc::Rc;

use fltk::{
    app, draw,
    enums::{Align, Color, Event, Font, Shortcut},
    prelude::*,
};

use crate::app_state::AppState;
use crate::layout::Widgets;
use crate::theme;

fn format_time(seconds: f64) -> String {
    let minutes = (seconds / 60.0).floor() as u32;
    let secs = seconds - minutes as f64 * 60.0;
    format!("{}:{:05.2}", minutes, secs)
}

fn update_view_readout(view_readout: &mut fltk::frame::Frame, st: &AppState) {
    view_readout.set_label(&format!(
        "Wave {:.3}s-{:.3}s | Spec {:.0}-{:.0} Hz",
        st.wave_view.time_offset_sec,
        st.wave_view.time_offset_sec + st.wave_view.time_span_sec,
        st.spec_view.freq_min_hz,
        st.spec_view.freq_max_hz
    ));
    view_readout.redraw();
}

fn sync_spec_scrollbar(spec_freq_scroll: &mut fltk::valuator::Scrollbar, st: &AppState) {
    let max_freq = st
        .frame
        .as_ref()
        .map(|f| f.max_frequency_hz())
        .unwrap_or(22_050.0)
        .max(1.0);
    let range = (st.spec_view.freq_max_hz - st.spec_view.freq_min_hz).max(1.0);
    let slider = (range / max_freq).clamp(0.01, 1.0);
    let start = st
        .spec_view
        .freq_min_hz
        .clamp(0.0, max_freq - range.max(0.0));
    let pos = if max_freq > range {
        start / (max_freq - range)
    } else {
        0.0
    };
    spec_freq_scroll.set_minimum(0.0);
    spec_freq_scroll.set_maximum(1000.0);
    spec_freq_scroll.set_slider_size(slider);
    spec_freq_scroll.set_value((1000.0 * pos as f64).clamp(0.0, 1000.0));
}

fn clamp_time_view(st: &mut AppState) {
    let max_t = st.max_preview_time().max(st.wave_view.time_span_sec);
    if st.wave_view.time_offset_sec < 0.0 {
        st.wave_view.time_offset_sec = 0.0;
    }
    if st.wave_view.time_offset_sec + st.wave_view.time_span_sec > max_t {
        st.wave_view.time_offset_sec = (max_t - st.wave_view.time_span_sec).max(0.0);
    }
}

fn waveform_amp_max_visible(st: &mut AppState) {
    if st.preview_samples.is_empty() {
        return;
    }
    let sr = st.preview_sample_rate.max(1) as f64;
    let s0 = (st.wave_view.time_offset_sec * sr).floor().max(0.0) as usize;
    let s1 = ((st.wave_view.time_offset_sec + st.wave_view.time_span_sec) * sr).ceil() as usize;
    let s1 = s1.min(st.preview_samples.len());
    if s0 >= s1 {
        return;
    }
    let peak = st.preview_samples[s0..s1]
        .iter()
        .map(|v| v.abs())
        .fold(0.0_f32, f32::max);
    if peak > 1e-9 {
        st.wave_view.amp_visual_gain = 0.98 / peak;
    }
}

pub fn setup_draw_callbacks(widgets: &Widgets, state: &Rc<RefCell<AppState>>) {
    setup_waveform_draw(widgets, state);
    setup_waveform_mouse(widgets, state);
    setup_wave_time_axis_draw(widgets, state);
    setup_wave_amp_axis_draw(widgets, state);
    setup_spectrogram_draw(widgets, state);
    setup_spec_mouse(widgets, state);
    setup_spec_freq_axis_draw(widgets, state);
}

fn setup_waveform_draw(widgets: &Widgets, state: &Rc<RefCell<AppState>>) {
    let state = state.clone();
    let mut w = widgets.waveform_display.clone();
    w.draw(move |f| {
        draw::set_draw_color(theme::color(theme::BG_DARK));
        draw::draw_rectf(f.x(), f.y(), f.w(), f.h());

        let st = state.borrow();
        if st.preview_samples.is_empty() {
            draw::set_draw_color(theme::color(theme::TEXT_DISABLED));
            draw::set_font(Font::Helvetica, 14);
            draw::draw_text2("Waveform", f.x(), f.y(), f.w(), f.h(), Align::Center);
            return;
        }

        let mid_y = f.y() + f.h() / 2;
        draw::set_draw_color(theme::color(theme::BORDER));
        draw::draw_line(f.x(), mid_y, f.x() + f.w(), mid_y);
        for frac in [0.25_f32, 0.75_f32] {
            let y = f.y() + (f.h() as f32 * frac) as i32;
            draw::set_draw_color(theme::color(theme::SEPARATOR));
            draw::draw_line(f.x(), y, f.x() + f.w(), y);
        }

        let sr = st.preview_sample_rate.max(1) as f64;
        let t0 = st.wave_view.time_offset_sec;
        let span = st.wave_view.time_span_sec.max(1e-6);
        let samples = &st.preview_samples;

        let samples_per_px = span * sr / f.w().max(1) as f64;
        draw::set_draw_color(theme::color(theme::ACCENT_BLUE));
        if samples_per_px > 4.0 {
            for px in 0..f.w().max(1) {
                let time0 = t0 + px as f64 / f.w().max(1) as f64 * span;
                let time1 = t0 + (px + 1) as f64 / f.w().max(1) as f64 * span;
                let s0 = (time0 * sr).floor().max(0.0) as usize;
                let s1 = ((time1 * sr).ceil() as usize).min(samples.len());
                if s0 >= s1 || s0 >= samples.len() {
                    continue;
                }
                let mut min_v = f32::MAX;
                let mut max_v = f32::MIN;
                for &sample in &samples[s0..s1] {
                    min_v = min_v.min(sample);
                    max_v = max_v.max(sample);
                }
                let y_top = mid_y
                    - ((max_v * st.wave_view.amp_visual_gain).clamp(-1.0, 1.0)
                        * (f.h() as f32 * 0.46)) as i32;
                let y_bot = mid_y
                    - ((min_v * st.wave_view.amp_visual_gain).clamp(-1.0, 1.0)
                        * (f.h() as f32 * 0.46)) as i32;
                draw::draw_line(f.x() + px, y_top, f.x() + px, y_bot);
            }
        } else {
            let show_dots = samples_per_px < 0.5;
            let mut prev: Option<(i32, i32)> = None;
            for px in 0..f.w().max(1) {
                let time = t0 + px as f64 / f.w().max(1) as f64 * span;
                let idx = (time * sr).floor() as usize;
                if idx < samples.len() {
                    let v = (samples[idx] * st.wave_view.amp_visual_gain).clamp(-1.0, 1.0);
                    let y = mid_y - (v * (f.h() as f32 * 0.46)) as i32;
                    let x = f.x() + px;
                    if let Some((px0, py0)) = prev {
                        draw::draw_line(px0, py0, x, y);
                    }
                    if show_dots {
                        draw::set_draw_color(theme::color(theme::ACCENT_YELLOW));
                        draw::draw_rectf(x - 1, y - 1, 3, 3);
                        draw::set_draw_color(theme::color(theme::ACCENT_BLUE));
                    }
                    prev = Some((x, y));
                }
            }
        }

        if let Some(hover_t) = st.hover_wave_time_sec {
            if hover_t >= t0 && hover_t <= t0 + span {
                let frac = ((hover_t - t0) / span).clamp(0.0, 1.0);
                let x = f.x() + (frac * f.w() as f64) as i32;
                draw::set_draw_color(theme::color(theme::ACCENT_YELLOW));
                draw::draw_line(x, f.y(), x, f.y() + f.h());
            }
        }
    });
}

fn setup_wave_time_axis_draw(widgets: &Widgets, state: &Rc<RefCell<AppState>>) {
    let state = state.clone();
    let mut w = widgets.wave_time_axis.clone();
    w.draw(move |f| {
        draw::set_draw_color(theme::color(theme::BG_DARK));
        draw::draw_rectf(f.x(), f.y(), f.w(), f.h());
        let st = state.borrow();
        draw::set_draw_color(theme::color(theme::TEXT_SECONDARY));
        draw::set_font(Font::Helvetica, 11);
        for i in 0..=4 {
            let x = f.x() + (i * f.w() / 4);
            draw::draw_line(x, f.y(), x, f.y() + 4);
            let t = st.wave_view.time_offset_sec + st.wave_view.time_span_sec * (i as f64 / 4.0);
            draw::draw_text2(
                &format!("{:.3}s", t),
                x - 24,
                f.y() + 5,
                48,
                f.h() - 5,
                Align::Top,
            );
        }
    });
}

fn setup_wave_amp_axis_draw(widgets: &Widgets, state: &Rc<RefCell<AppState>>) {
    let state = state.clone();
    let mut w = widgets.wave_amp_axis.clone();
    w.draw(move |f| {
        draw::set_draw_color(theme::color(theme::BG_DARK));
        draw::draw_rectf(f.x(), f.y(), f.w(), f.h());
        let st = state.borrow();
        draw::set_draw_color(theme::color(theme::TEXT_SECONDARY));
        draw::set_font(Font::Helvetica, 10);
        for (i, val) in [1.0_f32, 0.5, 0.0, -0.5, -1.0].iter().enumerate() {
            let y = f.y() + (i as i32 * f.h() / 4);
            draw::draw_line(f.x() + f.w() - 4, y, f.x() + f.w(), y);
            let shown = *val / st.wave_view.amp_visual_gain.max(1e-6);
            draw::draw_text2(
                &format!("{:.2}", shown),
                f.x(),
                y - 6,
                f.w() - 6,
                12,
                Align::Right,
            );
        }
    });
}

fn setup_spectrogram_draw(widgets: &Widgets, state: &Rc<RefCell<AppState>>) {
    let state = state.clone();
    let mut w = widgets.spectrogram_display.clone();
    w.draw(move |f| {
        draw::set_draw_color(theme::color(theme::BG_DARK));
        draw::draw_rectf(f.x(), f.y(), f.w(), f.h());
        let st = state.borrow();
        let Some(frame) = st.frame.as_ref() else {
            draw::set_draw_color(theme::color(theme::TEXT_DISABLED));
            draw::set_font(Font::Helvetica, 14);
            draw::draw_text2(
                "Frame Spectrogram",
                f.x(),
                f.y(),
                f.w(),
                f.h(),
                Align::Center,
            );
            return;
        };
        let max_mag = frame
            .bins
            .iter()
            .map(|b| b.magnitude)
            .fold(1e-9_f32, f32::max);
        let fmin = st.spec_view.freq_min_hz;
        let fmax = st.spec_view.freq_max_hz.max(fmin + 1.0);
        for frac in [0.25_f32, 0.5, 0.75] {
            let y = f.y() + (f.h() as f32 * (1.0 - frac)) as i32;
            draw::set_draw_color(theme::color(theme::SEPARATOR));
            draw::draw_line(f.x(), y, f.x() + f.w(), y);
        }
        for py in 0..f.h().max(1) {
            let t = 1.0 - py as f32 / f.h().max(1) as f32;
            let freq = fmin + (fmax - fmin) * t;
            let mut nearest_mag = 0.0_f32;
            let mut best_df = f32::MAX;
            for bin in &frame.bins {
                let df = (bin.frequency_hz - freq).abs();
                if df < best_df {
                    best_df = df;
                    nearest_mag = bin.magnitude;
                }
            }
            let normalized = (nearest_mag / max_mag).sqrt().clamp(0.0, 1.0);
            let color = if normalized < 0.33 {
                Color::from_rgb(42, 42, 58)
            } else if normalized < 0.66 {
                Color::from_rgb(137, 180, 250)
            } else {
                Color::from_rgb(249, 226, 175)
            };
            draw::set_draw_color(color);
            draw::draw_line(f.x(), f.y() + py, f.x() + f.w(), f.y() + py);
        }

        if let Some(hover_f) = st.hover_spec_freq_hz {
            if hover_f >= fmin && hover_f <= fmax {
                let frac = 1.0 - ((hover_f - fmin) / (fmax - fmin).max(1e-6));
                let y = f.y() + (frac * f.h() as f32) as i32;
                draw::set_draw_color(theme::color(theme::ACCENT_YELLOW));
                draw::draw_line(f.x(), y, f.x() + f.w(), y);
            }
        }
    });
}

fn setup_spec_freq_axis_draw(widgets: &Widgets, state: &Rc<RefCell<AppState>>) {
    let state = state.clone();
    let mut w = widgets.spec_freq_axis.clone();
    w.draw(move |f| {
        draw::set_draw_color(theme::color(theme::BG_DARK));
        draw::draw_rectf(f.x(), f.y(), f.w(), f.h());
        let st = state.borrow();
        let fmin = st.spec_view.freq_min_hz;
        let fmax = st.spec_view.freq_max_hz;
        draw::set_draw_color(theme::color(theme::TEXT_SECONDARY));
        draw::set_font(Font::Helvetica, 10);
        for i in 0..=4 {
            let y = f.y() + (i * f.h() / 4);
            let t = 1.0 - i as f32 / 4.0;
            let freq = fmin + (fmax - fmin) * t;
            draw::draw_text2(
                &format!("{:.0}", freq),
                f.x(),
                y - 6,
                f.w() - 4,
                12,
                Align::Right,
            );
        }
    });
}

fn setup_waveform_mouse(widgets: &Widgets, state: &Rc<RefCell<AppState>>) {
    let state = state.clone();
    let mut wave = widgets.waveform_display.clone();
    let mut wave_axis = widgets.wave_time_axis.clone();
    let mut amp_axis = widgets.wave_amp_axis.clone();
    let mut cursor_readout = widgets.cursor_readout.clone();
    let mut view_readout = widgets.view_readout.clone();
    let last_drag_x = Rc::new(RefCell::new(0i32));
    let last_drag_x_push = last_drag_x.clone();
    let last_drag_x_drag = last_drag_x.clone();
    wave.handle(move |w, ev| match ev {
        Event::Push => {
            *last_drag_x_push.borrow_mut() = app::event_x();
            true
        }
        Event::Move => {
            let mut st = state.borrow_mut();
            let mx = (app::event_x() - w.x()).clamp(0, w.w().max(1));
            let my = (app::event_y() - w.y()).clamp(0, w.h().max(1));
            let time = st.wave_view.time_offset_sec
                + st.wave_view.time_span_sec * (mx as f64 / w.w().max(1) as f64);
            st.hover_wave_time_sec = Some(time);
            let idx = (time * st.preview_sample_rate.max(1) as f64).floor() as usize;
            let norm_y = 1.0 - (my as f32 / w.h().max(1) as f32);
            let amp_view = (norm_y - 0.5) * 2.0;
            if idx < st.preview_samples.len() {
                let sample = st.preview_samples[idx];
                cursor_readout.set_label(&format!(
                    "Wave | t {} | samp {:.4} | axis {:.4}",
                    format_time(time),
                    sample,
                    amp_view / st.wave_view.amp_visual_gain.max(1e-6)
                ));
            } else {
                cursor_readout.set_label("");
            }
            cursor_readout.redraw();
            w.redraw();
            true
        }
        Event::MouseWheel => {
            let dy = app::event_dy();
            let scroll_up = matches!(dy, app::MouseWheel::Up);
            let mods = app::event_state();
            let mut st = state.borrow_mut();
            if mods.contains(Shortcut::Shift) {
                let factor = if scroll_up {
                    1.0 / st.waveform_amp_zoom_factor
                } else {
                    st.waveform_amp_zoom_factor
                };
                st.wave_view.amp_visual_gain *= factor;
            } else if mods.contains(Shortcut::Alt) {
                let delta = if scroll_up { -0.15 } else { 0.15 } * st.wave_view.time_span_sec;
                st.wave_view.time_offset_sec += delta;
                clamp_time_view(&mut st);
            } else {
                let mx = app::event_x() - w.x();
                let focus = mx as f64 / w.w().max(1) as f64;
                let center_time = st.wave_view.time_offset_sec + st.wave_view.time_span_sec * focus;
                let factor = if scroll_up {
                    1.0 / st.waveform_zoom_factor as f64
                } else {
                    st.waveform_zoom_factor as f64
                };
                let new_span = (st.wave_view.time_span_sec * factor)
                    .clamp(0.001, st.max_preview_time().max(0.001));
                st.wave_view.time_offset_sec = center_time - new_span * focus;
                st.wave_view.time_span_sec = new_span;
                clamp_time_view(&mut st);
            }
            drop(st);
            w.redraw();
            wave_axis.redraw();
            amp_axis.redraw();
            update_view_readout(&mut view_readout, &state.borrow());
            true
        }
        Event::Drag => {
            let x = app::event_x();
            let dx = x - *last_drag_x_drag.borrow();
            *last_drag_x_drag.borrow_mut() = x;
            let mut st = state.borrow_mut();
            st.wave_view.time_offset_sec -=
                dx as f64 / w.w().max(1) as f64 * st.wave_view.time_span_sec;
            clamp_time_view(&mut st);
            drop(st);
            w.redraw();
            wave_axis.redraw();
            update_view_readout(&mut view_readout, &state.borrow());
            true
        }
        Event::Leave => {
            state.borrow_mut().hover_wave_time_sec = None;
            cursor_readout.set_label("");
            cursor_readout.redraw();
            w.redraw();
            true
        }
        _ => false,
    });
}

fn setup_spec_mouse(widgets: &Widgets, state: &Rc<RefCell<AppState>>) {
    let state = state.clone();
    let mut spec = widgets.spectrogram_display.clone();
    let mut axis = widgets.spec_freq_axis.clone();
    let mut cursor_readout = widgets.cursor_readout.clone();
    let mut view_readout = widgets.view_readout.clone();
    let mut spec_freq_scroll = widgets.spec_freq_scroll.clone();
    spec.handle(move |w, ev| match ev {
        Event::Move => {
            let mut st = state.borrow_mut();
            let Some(frame) = st.frame.as_ref() else {
                cursor_readout.set_label("");
                cursor_readout.redraw();
                return true;
            };
            let my = (app::event_y() - w.y()).clamp(0, w.h().max(1));
            let t = 1.0 - my as f32 / w.h().max(1) as f32;
            let freq = st.spec_view.freq_min_hz
                + (st.spec_view.freq_max_hz - st.spec_view.freq_min_hz) * t;
            let mut nearest_mag = 0.0_f32;
            let mut nearest_bin = 0usize;
            let mut best_df = f32::MAX;
            for bin in &frame.bins {
                let df = (bin.frequency_hz - freq).abs();
                if df < best_df {
                    best_df = df;
                    nearest_mag = bin.magnitude;
                    nearest_bin = bin.bin_index;
                }
            }
            st.hover_spec_freq_hz = Some(freq);
            let db = 20.0 * nearest_mag.max(1e-12).log10();
            cursor_readout.set_label(&format!(
                "Spec | bin {} | {:.1} Hz | {:.1} dB",
                nearest_bin, freq, db
            ));
            cursor_readout.redraw();
            w.redraw();
            true
        }
        Event::MouseWheel => {
            let dy = app::event_dy();
            let scroll_up = matches!(dy, app::MouseWheel::Up);
            let mods = app::event_state();
            let mut st = state.borrow_mut();
            if mods.contains(Shortcut::Alt) {
                let delta = if scroll_up { 0.15 } else { -0.15 }
                    * (st.spec_view.freq_max_hz - st.spec_view.freq_min_hz);
                st.spec_view.freq_min_hz = (st.spec_view.freq_min_hz + delta).max(0.0);
                st.spec_view.freq_max_hz =
                    (st.spec_view.freq_max_hz + delta).max(st.spec_view.freq_min_hz + 1.0);
            } else {
                let my = app::event_y() - w.y();
                let focus = 1.0 - my as f32 / w.h().max(1) as f32;
                let focus_freq = st.spec_view.freq_min_hz
                    + (st.spec_view.freq_max_hz - st.spec_view.freq_min_hz) * focus;
                let factor = if scroll_up {
                    1.0 / st.spec_freq_zoom_factor
                } else {
                    st.spec_freq_zoom_factor
                };
                let range = (st.spec_view.freq_max_hz - st.spec_view.freq_min_hz).max(1.0);
                let new_range = (range * factor).clamp(
                    50.0,
                    st.frame
                        .as_ref()
                        .map(|f| f.max_frequency_hz())
                        .unwrap_or(22050.0)
                        .max(50.0),
                );
                st.spec_view.freq_min_hz = (focus_freq - new_range * focus).max(0.0);
                st.spec_view.freq_max_hz = st.spec_view.freq_min_hz + new_range;
            }
            drop(st);
            sync_spec_scrollbar(&mut spec_freq_scroll, &state.borrow());
            w.redraw();
            axis.redraw();
            update_view_readout(&mut view_readout, &state.borrow());
            true
        }
        Event::Leave => {
            state.borrow_mut().hover_spec_freq_hz = None;
            cursor_readout.set_label("");
            cursor_readout.redraw();
            w.redraw();
            true
        }
        _ => false,
    });
}

pub fn fit_visible_wave_amplitude(state: &Rc<RefCell<AppState>>) {
    let mut st = state.borrow_mut();
    waveform_amp_max_visible(&mut st);
}
