use std::rc::Rc;
use std::cell::RefCell;

use fltk::prelude::*;

use crate::app_state::{AppState, SharedCallbacks, UpdateThrottle};
use crate::data::{ColormapId, FreqScale, GradientStop, TimeUnit, WindowType, eval_gradient};
use crate::layout::Widgets;
use crate::settings::Settings;
use crate::validation::{attach_float_validation, parse_or_zero_f32};

// ═══════════════════════════════════════════════════════════════════════════
//  PARAMETER CALLBACKS
// ═══════════════════════════════════════════════════════════════════════════

pub fn setup_parameter_callbacks(
    widgets: &Widgets,
    state: &Rc<RefCell<AppState>>,
    shared: &SharedCallbacks,
) {
    // Time unit toggle
    {
        let state = state.clone();
        let mut input_start = widgets.input_start.clone();
        let mut input_stop = widgets.input_stop.clone();

        let mut btn_time_unit = widgets.btn_time_unit.clone();
        btn_time_unit.set_callback(move |btn| {
            let mut st = state.borrow_mut();
            let sr = st.fft_params.sample_rate as f64;
            match st.fft_params.time_unit {
                TimeUnit::Seconds => {
                    // Convert seconds -> samples
                    let start_samples = (st.fft_params.start_time * sr) as u64;
                    let stop_samples = (st.fft_params.stop_time * sr) as u64;
                    st.fft_params.time_unit = TimeUnit::Samples;
                    st.fft_params.start_time = start_samples as f64;
                    st.fft_params.stop_time = stop_samples as f64;
                    input_start.set_value(&start_samples.to_string());
                    input_stop.set_value(&stop_samples.to_string());
                    btn.set_label("Unit: Samples");
                }
                TimeUnit::Samples => {
                    // Convert samples -> seconds
                    let start_secs = st.fft_params.start_time / sr;
                    let stop_secs = st.fft_params.stop_time / sr;
                    st.fft_params.time_unit = TimeUnit::Seconds;
                    st.fft_params.start_time = start_secs;
                    st.fft_params.stop_time = stop_secs;
                    input_start.set_value(&format!("{:.5}", start_secs));
                    input_stop.set_value(&format!("{:.5}", stop_secs));
                    btn.set_label("Unit: Seconds");
                }
            }
        });
    }

    // Overlap (with hop info update)
    {
        let mut lbl = widgets.lbl_overlap_val.clone();
        let mut lbl_hop = widgets.lbl_hop_info.clone();
        let state = state.clone();
        let update_info = shared.update_info.clone();

        let mut slider_overlap = widgets.slider_overlap.clone();
        slider_overlap.set_callback(move |s| {
            let val = s.value() as f32;
            lbl.set_label(&format!("Overlap: {}%", val as i32));
            {
                let mut st = state.borrow_mut();
                st.fft_params.overlap_percent = val;
                let hop = st.fft_params.hop_length();
                let hop_ms = hop as f64 / st.fft_params.sample_rate.max(1) as f64 * 1000.0;
                lbl_hop.set_label(&format!("Hop: {} smp ({:.1} ms)", hop, hop_ms));
            }
            (update_info.borrow_mut())();
        });
    }

    // Window type (kaiser beta is read at recompute time from the field)
    {
        let state = state.clone();
        let mut input_kaiser_beta = widgets.input_kaiser_beta.clone();

        let mut window_type_choice = widgets.window_type_choice.clone();
        window_type_choice.set_callback(move |c| {
            let mut st = state.borrow_mut();
            st.fft_params.window_type = match c.value() {
                0 => { input_kaiser_beta.deactivate(); WindowType::Hann }
                1 => { input_kaiser_beta.deactivate(); WindowType::Hamming }
                2 => { input_kaiser_beta.deactivate(); WindowType::Blackman }
                3 => {
                    input_kaiser_beta.activate();
                    let beta = parse_or_zero_f32(&input_kaiser_beta.value());
                    WindowType::Kaiser(if beta > 0.0 { beta } else { 8.6 })
                }
                _ => WindowType::Hann,
            };
        });
    }

    // Segment size preset dropdown
    {
        let state = state.clone();
        let update_info = shared.update_info.clone();
        let mut input_seg_size = widgets.input_seg_size.clone();

        let mut seg_preset_choice = widgets.seg_preset_choice.clone();
        seg_preset_choice.set_callback(move |c| {
            let idx = c.value();
            if idx >= 0 && idx < 9 {
                let size = SEG_PRESETS[idx as usize];
                input_seg_size.set_value(&size.to_string());
                state.borrow_mut().fft_params.window_length = size;
                (update_info.borrow_mut())();
            }
            // idx == 9 is "Custom" - leave input as-is
        });
    }

    // Segment size typed input (on Enter)
    {
        let state = state.clone();
        let update_info = shared.update_info.clone();
        let mut seg_preset_choice = widgets.seg_preset_choice.clone();

        let mut input_seg_size = widgets.input_seg_size.clone();
        input_seg_size.set_callback(move |inp| {
            let raw: usize = inp.value().parse().unwrap_or(8192);
            let clamped = raw.clamp(2, 131072);
            let even = round_even(clamped);
            inp.set_value(&even.to_string());
            state.borrow_mut().fft_params.window_length = even;

            // Sync preset dropdown
            let preset_idx = find_preset_index(even).map(|i| i as i32).unwrap_or(9);
            seg_preset_choice.set_value(preset_idx);

            (update_info.borrow_mut())();
        });
    }

    // Segment size +/- buttons (step through presets)
    {
        let state = state.clone();
        let update_info = shared.update_info.clone();
        let update_seg_label = shared.update_seg_label.clone();
        let mut input_seg_size = widgets.input_seg_size.clone();
        let mut seg_preset_choice = widgets.seg_preset_choice.clone();

        let mut btn_seg_minus = widgets.btn_seg_minus.clone();
        btn_seg_minus.set_callback(move |_| {
            let mut st = state.borrow_mut();
            let cur = st.fft_params.window_length;
            let new_wl = if let Some(idx) = find_preset_index(cur) {
                if idx > 0 { SEG_PRESETS[idx - 1] } else { SEG_PRESETS[0] }
            } else {
                // Custom: halve
                round_even((cur / 2).max(2))
            };
            st.fft_params.window_length = new_wl;
            drop(st);
            input_seg_size.set_value(&new_wl.to_string());
            let preset_idx = find_preset_index(new_wl).map(|i| i as i32).unwrap_or(9);
            seg_preset_choice.set_value(preset_idx);
            (update_info.borrow_mut())();
            (update_seg_label.borrow_mut())();
        });
    }
    {
        let state = state.clone();
        let update_info = shared.update_info.clone();
        let update_seg_label = shared.update_seg_label.clone();
        let mut input_seg_size = widgets.input_seg_size.clone();
        let mut seg_preset_choice = widgets.seg_preset_choice.clone();

        let mut btn_seg_plus = widgets.btn_seg_plus.clone();
        btn_seg_plus.set_callback(move |_| {
            let mut st = state.borrow_mut();
            let cur = st.fft_params.window_length;
            let new_wl = if let Some(idx) = find_preset_index(cur) {
                if idx < SEG_PRESETS.len() - 1 { SEG_PRESETS[idx + 1] } else { SEG_PRESETS[SEG_PRESETS.len() - 1] }
            } else {
                // Custom: double
                round_even((cur * 2).min(131072))
            };
            st.fft_params.window_length = new_wl;
            drop(st);
            input_seg_size.set_value(&new_wl.to_string());
            let preset_idx = find_preset_index(new_wl).map(|i| i as i32).unwrap_or(9);
            seg_preset_choice.set_value(preset_idx);
            (update_info.borrow_mut())();
            (update_seg_label.borrow_mut())();
        });
    }

    // Kaiser beta - read at recompute time, but also sync when window type changes
    {
        let mut input_kaiser_beta = widgets.input_kaiser_beta.clone();
        attach_float_validation(&mut input_kaiser_beta);
    }

    // Center/Pad
    {
        let state = state.clone();
        let update_info = shared.update_info.clone();

        let mut check_center = widgets.check_center.clone();
        check_center.set_callback(move |c| {
            state.borrow_mut().fft_params.use_center = c.is_checked();
            (update_info.borrow_mut())();
        });
    }

    // Zero-padding factor
    {
        let state = state.clone();
        let update_info = shared.update_info.clone();

        let mut zero_pad_choice = widgets.zero_pad_choice.clone();
        zero_pad_choice.set_callback(move |c| {
            let factor = match c.value() {
                0 => 1,
                1 => 2,
                2 => 4,
                3 => 8,
                _ => 1,
            };
            state.borrow_mut().fft_params.zero_pad_factor = factor;
            (update_info.borrow_mut())();
        });
    }
}

// ─── Segment size helpers ─────────────────────────────────────────────────────

const SEG_PRESETS: [usize; 9] = [256, 512, 1024, 2048, 4096, 8192, 16384, 32768, 65536];

fn find_preset_index(size: usize) -> Option<usize> {
    SEG_PRESETS.iter().position(|&p| p == size)
}

fn round_even(n: usize) -> usize {
    if n < 2 { 2 } else if n % 2 != 0 { n + 1 } else { n }
}

// ═══════════════════════════════════════════════════════════════════════════
//  DISPLAY CALLBACKS
// ═══════════════════════════════════════════════════════════════════════════

pub fn setup_display_callbacks(
    widgets: &Widgets,
    state: &Rc<RefCell<AppState>>,
) {
    // Colormap
    {
        let state = state.clone();
        let mut spec_display = widgets.spec_display.clone();
        let mut gradient_preview = widgets.gradient_preview.clone();

        let mut colormap_choice = widgets.colormap_choice.clone();
        colormap_choice.set_callback(move |c| {
            let mut st = state.borrow_mut();
            st.view.colormap = ColormapId::from_index(c.value() as usize);
            st.spec_renderer.invalidate();
            drop(st);
            spec_display.redraw();
            gradient_preview.redraw();
        });
    }

    // Freq Scale Power slider (0.0 = linear, 1.0 = log)
    {
        let mut lbl = widgets.lbl_scale_val.clone();
        let state = state.clone();
        let mut spec_display = widgets.spec_display.clone();
        let mut freq_axis = widgets.freq_axis.clone();
        let throttle = Rc::new(RefCell::new(UpdateThrottle::new(50)));

        let mut slider_scale = widgets.slider_scale.clone();
        slider_scale.set_callback(move |s| {
            let val = s.value() as f32;
            let label = if val <= 0.01 { "Scale: Linear".to_string() }
                       else if val >= 0.99 { "Scale: Log".to_string() }
                       else { format!("Scale: {:.0}%", val * 100.0) };
            lbl.set_label(&label);
            state.borrow_mut().view.freq_scale = FreqScale::Power(val);

            if throttle.borrow_mut().should_update() {
                state.borrow_mut().spec_renderer.invalidate();
                spec_display.redraw();
                freq_axis.redraw();
            }
        });
    }

    // Threshold
    {
        let mut lbl = widgets.lbl_threshold_val.clone();
        let state = state.clone();
        let mut spec_display = widgets.spec_display.clone();
        let throttle = Rc::new(RefCell::new(UpdateThrottle::new(50)));

        let mut slider_threshold = widgets.slider_threshold.clone();
        slider_threshold.set_callback(move |s| {
            let val = s.value() as f32;
            lbl.set_label(&format!("Threshold: {} dB", val as i32));
            state.borrow_mut().view.threshold_db = val;

            if throttle.borrow_mut().should_update() {
                state.borrow_mut().spec_renderer.invalidate();
                spec_display.redraw();
            }
        });
    }

    // dB Ceiling
    {
        let mut lbl = widgets.lbl_ceiling_val.clone();
        let state = state.clone();
        let mut spec_display = widgets.spec_display.clone();
        let throttle = Rc::new(RefCell::new(UpdateThrottle::new(50)));

        let mut slider_ceiling = widgets.slider_ceiling.clone();
        slider_ceiling.set_callback(move |s| {
            let val = s.value() as f32;
            lbl.set_label(&format!("Ceiling: {} dB", val as i32));
            state.borrow_mut().view.db_ceiling = val;

            if throttle.borrow_mut().should_update() {
                state.borrow_mut().spec_renderer.invalidate();
                spec_display.redraw();
            }
        });
    }

    // Brightness
    {
        let mut lbl = widgets.lbl_brightness_val.clone();
        let state = state.clone();
        let mut spec_display = widgets.spec_display.clone();
        let throttle = Rc::new(RefCell::new(UpdateThrottle::new(50)));

        let mut slider_brightness = widgets.slider_brightness.clone();
        slider_brightness.set_callback(move |s| {
            let val = s.value() as f32;
            lbl.set_label(&format!("Brightness: {:.1}", val));
            state.borrow_mut().view.brightness = val;

            if throttle.borrow_mut().should_update() {
                state.borrow_mut().spec_renderer.invalidate();
                spec_display.redraw();
            }
        });
    }

    // Gamma
    {
        let mut lbl = widgets.lbl_gamma_val.clone();
        let state = state.clone();
        let mut spec_display = widgets.spec_display.clone();
        let throttle = Rc::new(RefCell::new(UpdateThrottle::new(50)));

        let mut slider_gamma = widgets.slider_gamma.clone();
        slider_gamma.set_callback(move |s| {
            let val = s.value() as f32;
            lbl.set_label(&format!("Gamma: {:.1}", val));
            state.borrow_mut().view.gamma = val;

            if throttle.borrow_mut().should_update() {
                state.borrow_mut().spec_renderer.invalidate();
                spec_display.redraw();
            }
        });
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  PLAYBACK CALLBACKS
// ═══════════════════════════════════════════════════════════════════════════

pub fn setup_playback_callbacks(
    widgets: &Widgets,
    state: &Rc<RefCell<AppState>>,
) {
    {
        let state = state.clone();
        let mut btn_rerun = widgets.btn_rerun.clone();

        let mut btn_play = widgets.btn_play.clone();
        btn_play.set_callback(move |_| {
            let mut st = state.borrow_mut();
            if st.dirty {
                // Need to recompute first - trigger rerun, then play will happen after
                drop(st);
                btn_rerun.do_callback();
                return;
            }
            st.audio_player.play();
            st.transport.is_playing = true;
        });
    }
    {
        let state = state.clone();

        let mut btn_pause = widgets.btn_pause.clone();
        btn_pause.set_callback(move |_| {
            let mut st = state.borrow_mut();
            st.audio_player.pause();
            st.transport.is_playing = false;
        });
    }
    {
        let state = state.clone();

        let mut btn_stop = widgets.btn_stop.clone();
        btn_stop.set_callback(move |_| {
            let mut st = state.borrow_mut();
            st.audio_player.stop();
            st.transport.is_playing = false;
            st.transport.position_seconds = 0.0;
        });
    }

    // Scrub slider - seeks within the reconstructed audio
    {
        let state = state.clone();

        let mut scrub_slider = widgets.scrub_slider.clone();
        scrub_slider.handle(move |s, ev| {
            // Block spacebar from reaching the scrub slider
            if fltk::app::event_key() == fltk::enums::Key::from_char(' ') {
                return matches!(ev, fltk::enums::Event::KeyDown | fltk::enums::Event::KeyUp | fltk::enums::Event::Shortcut);
            }
            match ev {
                fltk::enums::Event::Push => {
                    let st = state.borrow();
                    st.audio_player.set_seeking(true);
                    let audio_position = s.value() * st.transport.duration_seconds;
                    st.audio_player.seek_to(audio_position);
                    true
                }
                fltk::enums::Event::Drag => {
                    let st = state.borrow();
                    let audio_position = s.value() * st.transport.duration_seconds;
                    st.audio_player.seek_to(audio_position);
                    true
                }
                fltk::enums::Event::Released => {
                    let st = state.borrow();
                    st.audio_player.set_seeking(false);
                    true
                }
                _ => false,
            }
        });
    }

    // Repeat
    {
        let state = state.clone();

        let mut repeat_choice = widgets.repeat_choice.clone();
        repeat_choice.set_callback(move |c| {
            let mut st = state.borrow_mut();
            st.audio_player.set_repeat(c.value() == 1);
            st.transport.repeat = c.value() == 1;
        });
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  MISC CALLBACKS
// ═══════════════════════════════════════════════════════════════════════════

pub fn setup_misc_callbacks(
    widgets: &Widgets,
    state: &Rc<RefCell<AppState>>,
    win: &fltk::window::Window,
) {
    // Tooltip toggle
    {
        let state = state.clone();

        let mut btn_tooltips = widgets.btn_tooltips.clone();
        btn_tooltips.set_callback(move |c| {
            state.borrow_mut().tooltip_mgr.set_enabled(c.is_checked());
        });
    }

    // Lock to Active toggle
    {
        let state = state.clone();

        let mut check_lock_active = widgets.check_lock_active.clone();
        check_lock_active.set_callback(move |c| {
            state.borrow_mut().lock_to_active = c.is_checked();
        });
    }

    // Home button — snap viewport to reconstruction time + freq range
    {
        let state = state.clone();
        let mut spec_display = widgets.spec_display.clone();
        let mut waveform_display = widgets.waveform_display.clone();
        let mut freq_axis = widgets.freq_axis.clone();
        let mut time_axis = widgets.time_axis.clone();

        let mut btn_home = widgets.btn_home.clone();
        btn_home.set_callback(move |_| {
            let mut st = state.borrow_mut();
            let proc_min = match st.fft_params.time_unit {
                TimeUnit::Seconds => st.fft_params.start_time,
                TimeUnit::Samples => st.fft_params.start_time / st.fft_params.sample_rate.max(1) as f64,
            };
            let proc_max = match st.fft_params.time_unit {
                TimeUnit::Seconds => st.fft_params.stop_time,
                TimeUnit::Samples => st.fft_params.stop_time / st.fft_params.sample_rate.max(1) as f64,
            };
            if proc_max > proc_min {
                st.view.time_min_sec = proc_min.max(st.view.data_time_min_sec);
                st.view.time_max_sec = proc_max.min(st.view.data_time_max_sec);
            }
            // Snap frequency to reconstruction range
            st.view.freq_min_hz = st.view.recon_freq_min_hz.max(1.0);
            st.view.freq_max_hz = st.view.recon_freq_max_hz.min(st.view.data_freq_max_hz);
            st.spec_renderer.invalidate();
            st.wave_renderer.invalidate();
            drop(st);
            spec_display.redraw();
            waveform_display.redraw();
            freq_axis.redraw();
            time_axis.redraw();
        });
    }

    // Save As Default — write current settings to INI
    {
        let state = state.clone();
        let win = win.clone();

        let mut btn_save_defaults = widgets.btn_save_defaults.clone();
        btn_save_defaults.set_callback(move |_| {
            let st = state.borrow();
            let mut cfg = Settings::from_app_state(&st);
            // Also capture current window dimensions
            cfg.window_width = win.w();
            cfg.window_height = win.h();
            cfg.save();
            eprintln!("[Settings] Saved current settings to settings.ini");
        });
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  GRADIENT EDITOR (draw + handle callbacks for the gradient preview widget)
// ═══════════════════════════════════════════════════════════════════════════

/// Internal state for the gradient editor mouse interaction.
struct GradientEditorState {
    selected_stop: Option<usize>,
    dragging: bool,
}

const GRAD_BAR_H: i32 = 20;   // height of the gradient bar
const STOP_HANDLE_H: i32 = 10; // height of the triangle handles below

/// Pixel margin from widget left/right edges for the gradient bar
const GRAD_MARGIN: i32 = 4;

pub fn setup_gradient_editor(
    widgets: &Widgets,
    state: &Rc<RefCell<AppState>>,
) {
    let editor_state = Rc::new(RefCell::new(GradientEditorState {
        selected_stop: None,
        dragging: false,
    }));

    // ── Draw callback ──
    {
        let state = state.clone();
        let editor_state = editor_state.clone();
        let mut gradient_preview = widgets.gradient_preview.clone();
        gradient_preview.draw(move |w| {
            use fltk::draw;
            use fltk::enums::{Color, Font};

            let x = w.x();
            let y = w.y();
            let ww = w.w();
            let _wh = w.h();

            // Background
            draw::set_draw_color(Color::from_hex(0x313244));
            draw::draw_rectf(x, y, ww, GRAD_BAR_H + STOP_HANDLE_H);

            let bar_x = x + GRAD_MARGIN;
            let bar_w = ww - GRAD_MARGIN * 2;
            if bar_w <= 0 { return; }

            // Draw gradient bar pixel by pixel
            let st = state.borrow();
            let is_custom = st.view.colormap == ColormapId::Custom;
            drop(st);

            // Read stops (we need them for drawing; re-borrow for short duration)
            let st = state.borrow();
            let stops_snapshot: Vec<GradientStop> = st.view.custom_gradient.clone();
            drop(st);

            for px in 0..bar_w {
                let t = px as f32 / bar_w.max(1) as f32;
                let (r, g, b) = eval_gradient(&stops_snapshot, t);
                let color = Color::from_rgb(
                    (r.clamp(0.0, 1.0) * 255.0) as u8,
                    (g.clamp(0.0, 1.0) * 255.0) as u8,
                    (b.clamp(0.0, 1.0) * 255.0) as u8,
                );
                draw::set_draw_color(color);
                draw::draw_rectf(bar_x + px, y, 1, GRAD_BAR_H);
            }

            // Draw border around bar
            draw::set_draw_color(Color::from_hex(0x6c7086));
            draw::draw_rect(bar_x, y, bar_w, GRAD_BAR_H);

            // Draw stop handles as triangles below the bar
            let es = editor_state.borrow();
            let handle_y = y + GRAD_BAR_H;
            for (i, stop) in stops_snapshot.iter().enumerate() {
                let cx = bar_x + (stop.position * bar_w as f32) as i32;
                let is_selected = es.selected_stop == Some(i) && is_custom;

                // Triangle pointing up
                let half = 4;
                if is_selected {
                    draw::set_draw_color(Color::from_hex(0xffffff));
                } else if is_custom {
                    draw::set_draw_color(Color::from_hex(0xcdd6f4));
                } else {
                    draw::set_draw_color(Color::from_hex(0x6c7086));
                }
                draw::begin_polygon();
                draw::vertex((cx - half) as f64, (handle_y + STOP_HANDLE_H) as f64);
                draw::vertex(cx as f64, handle_y as f64);
                draw::vertex((cx + half) as f64, (handle_y + STOP_HANDLE_H) as f64);
                draw::end_polygon();

                // Outline
                draw::set_draw_color(Color::from_hex(0x1e1e2e));
                draw::begin_line();
                draw::vertex((cx - half) as f64, (handle_y + STOP_HANDLE_H) as f64);
                draw::vertex(cx as f64, handle_y as f64);
                draw::vertex((cx + half) as f64, (handle_y + STOP_HANDLE_H) as f64);
                draw::end_line();
            }
            drop(es);

            // If not in custom mode, draw a subtle overlay label
            if !is_custom {
                draw::set_draw_color(Color::from_hex(0x6c7086));
                draw::set_font(Font::Helvetica, 9);
                let label = "Select 'Custom' to edit";
                let tw = draw::width(label) as i32;
                draw::draw_text(label, bar_x + (bar_w - tw) / 2, y + GRAD_BAR_H / 2 + 3);
            }
        });
    }

    // ── Handle callback (mouse interaction) ──
    {
        let state = state.clone();
        let editor_state = editor_state.clone();
        let mut spec_display = widgets.spec_display.clone();
        let mut gradient_preview_redraw = widgets.gradient_preview.clone();

        let mut gradient_preview = widgets.gradient_preview.clone();
        gradient_preview.handle(move |w, ev| {
            // Block spacebar from reaching the gradient editor
            if fltk::app::event_key() == fltk::enums::Key::from_char(' ') {
                return matches!(ev, fltk::enums::Event::KeyDown | fltk::enums::Event::KeyUp | fltk::enums::Event::Shortcut);
            }
            // Only allow interaction when colormap is Custom
            {
                let st = state.borrow();
                if st.view.colormap != ColormapId::Custom {
                    return false;
                }
            }

            let x = w.x();
            let ww = w.w();
            let bar_x = x + GRAD_MARGIN;
            let bar_w = ww - GRAD_MARGIN * 2;
            if bar_w <= 0 { return false; }

            match ev {
                fltk::enums::Event::Push => {
                    let mx = fltk::app::event_x();
                    let _my = fltk::app::event_y();
                    let button = fltk::app::event_button();
                    let clicks = fltk::app::event_clicks();

                    let pos_t = ((mx - bar_x) as f32 / bar_w as f32).clamp(0.0, 1.0);

                    // Check if clicking on an existing stop handle
                    let st = state.borrow();
                    let stops = &st.view.custom_gradient;
                    let hit_stop = find_stop_at_x(stops, pos_t, bar_w);
                    drop(st);

                    if button == 3 {
                        // Right-click: delete stop (keep minimum 2)
                        if let Some(idx) = hit_stop {
                            let mut st = state.borrow_mut();
                            if st.view.custom_gradient.len() > 2 {
                                st.view.custom_gradient.remove(idx);
                                st.spec_renderer.invalidate();
                                drop(st);
                                let mut es = editor_state.borrow_mut();
                                es.selected_stop = None;
                                es.dragging = false;
                                drop(es);
                                spec_display.redraw();
                                gradient_preview_redraw.redraw();
                            }
                        }
                        return true;
                    }

                    if let Some(idx) = hit_stop {
                        // Clicked on existing stop
                        let mut es = editor_state.borrow_mut();
                        es.selected_stop = Some(idx);
                        es.dragging = true;

                        // Double-click: open color picker
                        if clicks {
                            es.dragging = false;
                            drop(es);
                            let st = state.borrow();
                            let stop = st.view.custom_gradient[idx];
                            drop(st);

                            let cur_r = (stop.r.clamp(0.0, 1.0) * 255.0) as u8;
                            let cur_g = (stop.g.clamp(0.0, 1.0) * 255.0) as u8;
                            let cur_b = (stop.b.clamp(0.0, 1.0) * 255.0) as u8;
                            let (nr, ng, nb) = fltk::dialog::color_chooser_with_default(
                                "Pick Stop Color",
                                fltk::dialog::ColorMode::Rgb,
                                (cur_r, cur_g, cur_b),
                            );
                            // color_chooser_with_default returns the chosen color
                            // (same as input if cancelled - compare to detect change)
                            if nr != cur_r || ng != cur_g || nb != cur_b {
                                let mut st = state.borrow_mut();
                                st.view.custom_gradient[idx].r = nr as f32 / 255.0;
                                st.view.custom_gradient[idx].g = ng as f32 / 255.0;
                                st.view.custom_gradient[idx].b = nb as f32 / 255.0;
                                st.spec_renderer.invalidate();
                                drop(st);
                                spec_display.redraw();
                            }
                        }
                        gradient_preview_redraw.redraw();
                        return true;
                    }

                    // Clicked on empty space: add new stop with interpolated color
                    {
                        let mut st = state.borrow_mut();
                        let (r, g, b) = eval_gradient(&st.view.custom_gradient, pos_t);
                        let new_stop = GradientStop::new(pos_t, r, g, b);
                        // Insert sorted by position
                        let insert_idx = st.view.custom_gradient
                            .iter()
                            .position(|s| s.position > pos_t)
                            .unwrap_or(st.view.custom_gradient.len());
                        st.view.custom_gradient.insert(insert_idx, new_stop);
                        st.spec_renderer.invalidate();
                        drop(st);

                        let mut es = editor_state.borrow_mut();
                        es.selected_stop = Some(insert_idx);
                        es.dragging = true;
                        drop(es);

                        spec_display.redraw();
                        gradient_preview_redraw.redraw();
                    }
                    true
                }

                fltk::enums::Event::Drag => {
                    let es = editor_state.borrow();
                    if !es.dragging {
                        return false;
                    }
                    let idx = match es.selected_stop {
                        Some(i) => i,
                        None => return false,
                    };
                    drop(es);

                    let mx = fltk::app::event_x();
                    let pos_t = ((mx - bar_x) as f32 / bar_w as f32).clamp(0.0, 1.0);

                    let mut st = state.borrow_mut();
                    if idx < st.view.custom_gradient.len() {
                        st.view.custom_gradient[idx].position = pos_t;
                        // Re-sort stops and update selected index
                        let stop = st.view.custom_gradient[idx];
                        st.view.custom_gradient.sort_by(|a, b| a.position.partial_cmp(&b.position).unwrap());
                        // Find where our stop ended up after sort
                        let new_idx = st.view.custom_gradient.iter().position(|s| {
                            (s.position - stop.position).abs() < 1e-6
                                && (s.r - stop.r).abs() < 1e-6
                                && (s.g - stop.g).abs() < 1e-6
                                && (s.b - stop.b).abs() < 1e-6
                        });
                        drop(st);

                        let mut es = editor_state.borrow_mut();
                        es.selected_stop = new_idx;
                        drop(es);

                        state.borrow_mut().spec_renderer.invalidate();
                        spec_display.redraw();
                        gradient_preview_redraw.redraw();
                    }
                    true
                }

                fltk::enums::Event::Released => {
                    let mut es = editor_state.borrow_mut();
                    es.dragging = false;
                    true
                }

                _ => false,
            }
        });
    }
}

/// Find which stop handle (if any) is near the given normalized position.
/// Returns the index of the nearest stop within a 6-pixel tolerance.
fn find_stop_at_x(stops: &[GradientStop], pos_t: f32, bar_w: i32) -> Option<usize> {
    let tolerance = 6.0 / bar_w.max(1) as f32;
    let mut best: Option<(usize, f32)> = None;
    for (i, stop) in stops.iter().enumerate() {
        let dist = (stop.position - pos_t).abs();
        if dist < tolerance {
            if best.is_none() || dist < best.unwrap().1 {
                best = Some((i, dist));
            }
        }
    }
    best.map(|(i, _)| i)
}
