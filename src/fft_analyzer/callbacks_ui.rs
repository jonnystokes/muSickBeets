use std::cell::{Cell, RefCell};
use std::rc::Rc;

use fltk::{enums::CallbackTrigger, prelude::*};

use crate::app_state::{set_msg, AppState, MsgLevel, SharedCallbacks, UpdateThrottle};
use crate::data::{
    ColormapId, FreqScale, LastEditedField, SolverConstraints, TimeUnit, WindowType,
};
use crate::layout::Widgets;
use crate::settings::Settings;
use crate::validation::{attach_float_validation, parse_or_zero_f32, parse_or_zero_usize};

// ═══════════════════════════════════════════════════════════════════════════
//  PARAMETER CALLBACKS
// ═══════════════════════════════════════════════════════════════════════════

pub fn setup_parameter_callbacks(
    widgets: &Widgets,
    state: &Rc<RefCell<AppState>>,
    shared: &SharedCallbacks,
) {
    let suppress_solver_inputs = Rc::new(Cell::new(false));
    // Time unit toggle
    {
        let state = state.clone();
        let mut input_start = widgets.input_start.clone();
        let mut input_stop = widgets.input_stop.clone();

        let mut btn_time_unit = widgets.btn_time_unit.clone();
        btn_time_unit.set_callback(move |btn| {
            let mut st = state.borrow_mut();
            match st.fft_params.time_unit {
                TimeUnit::Seconds => {
                    st.fft_params.time_unit = TimeUnit::Samples;
                    input_start.set_value(&st.fft_params.start_sample.to_string());
                    input_stop.set_value(&st.fft_params.stop_sample.to_string());
                    btn.set_label("Unit: Samples");
                }
                TimeUnit::Samples => {
                    st.fft_params.time_unit = TimeUnit::Seconds;
                    input_start.set_value(&format!("{:.5}", st.fft_params.start_seconds()));
                    input_stop.set_value(&format!("{:.5}", st.fft_params.stop_seconds()));
                    btn.set_label("Unit: Seconds");
                }
            }
        });
    }

    // Overlap (last-edited solver path)
    {
        let mut lbl = widgets.lbl_overlap_val.clone();
        let mut lbl_hop = widgets.lbl_hop_info.clone();
        let state = state.clone();
        let update_info = shared.update_info.clone();
        let mut input_seg_size = widgets.input_seg_size.clone();
        let mut input_segments = widgets.input_segments_per_active.clone();
        let mut input_bins = widgets.input_bins_per_segment.clone();
        let suppress_solver_inputs = suppress_solver_inputs.clone();

        let mut slider_overlap = widgets.slider_overlap.clone();
        slider_overlap.set_callback(move |s| {
            let val = s.value() as f32;
            lbl.set_label(&format!("Overlap: {}%", val as i32));
            {
                let mut st = state.borrow_mut();
                st.fft_params.overlap_percent = val;
                st.fft_params.last_edited_field = LastEditedField::Overlap;
                apply_segmentation_solver(&mut st);
                suppress_solver_inputs.set(true);
                input_seg_size.set_value(&st.fft_params.window_length.to_string());
                if let Some(seg) = st.fft_params.target_segments_per_active {
                    input_segments.set_value(&seg.to_string());
                }
                if let Some(b) = st.fft_params.target_bins_per_segment {
                    input_bins.set_value(&b.to_string());
                }
                suppress_solver_inputs.set(false);
                let hop = st.fft_params.hop_length();
                let hop_ms = hop as f64 / st.fft_params.sample_rate.max(1) as f64 * 1000.0;
                lbl_hop.set_label(&format!("Hop: {} smp ({:.1} ms)", hop, hop_ms));
            }
            (update_info.borrow_mut())();
        });
    }

    // Segments per active area (live while typing)
    {
        let state = state.clone();
        let update_info = shared.update_info.clone();
        let mut input_seg_size = widgets.input_seg_size.clone();
        let mut input_bins = widgets.input_bins_per_segment.clone();
        let suppress_solver_inputs = suppress_solver_inputs.clone();

        let mut input_segments = widgets.input_segments_per_active.clone();
        input_segments.set_trigger(CallbackTrigger::Changed);
        input_segments.set_callback(move |inp| {
            if inp.value().contains(' ') {
                inp.set_value(&inp.value().replace(' ', ""));
                return;
            }
            if suppress_solver_inputs.get() {
                return;
            }
            if inp.value().trim().is_empty() {
                return;
            }
            let mut st = state.borrow_mut();
            let target = parse_or_zero_usize(&inp.value()).max(1);
            st.fft_params.target_segments_per_active = Some(target);
            st.fft_params.last_edited_field = LastEditedField::SegmentsPerActive;
            apply_segmentation_solver(&mut st);

            suppress_solver_inputs.set(true);
            input_seg_size.set_value(&st.fft_params.window_length.to_string());
            input_bins.set_value(
                &st.fft_params
                    .target_bins_per_segment
                    .unwrap_or(st.fft_params.num_frequency_bins())
                    .to_string(),
            );
            suppress_solver_inputs.set(false);
            drop(st);
            (update_info.borrow_mut())();
        });
    }

    // Bins per segment (live while typing)
    {
        let state = state.clone();
        let update_info = shared.update_info.clone();
        let mut input_seg_size = widgets.input_seg_size.clone();
        let mut input_segments = widgets.input_segments_per_active.clone();
        let suppress_solver_inputs = suppress_solver_inputs.clone();

        let mut input_bins = widgets.input_bins_per_segment.clone();
        input_bins.set_trigger(CallbackTrigger::Changed);
        input_bins.set_callback(move |inp| {
            if inp.value().contains(' ') {
                inp.set_value(&inp.value().replace(' ', ""));
                return;
            }
            if suppress_solver_inputs.get() {
                return;
            }
            if inp.value().trim().is_empty() {
                return;
            }
            let mut st = state.borrow_mut();
            let target = parse_or_zero_usize(&inp.value()).max(1);
            st.fft_params.target_bins_per_segment = Some(target);
            st.fft_params.last_edited_field = LastEditedField::BinsPerSegment;
            apply_segmentation_solver(&mut st);

            suppress_solver_inputs.set(true);
            input_seg_size.set_value(&st.fft_params.window_length.to_string());
            input_segments.set_value(
                &st.fft_params
                    .target_segments_per_active
                    .unwrap_or(st.fft_params.num_segments(current_active_samples(&st)))
                    .to_string(),
            );
            suppress_solver_inputs.set(false);
            drop(st);
            (update_info.borrow_mut())();
        });
    }

    // Window type
    {
        let state = state.clone();
        let mut input_kaiser_beta = widgets.input_kaiser_beta.clone();

        let mut window_type_choice = widgets.window_type_choice.clone();
        window_type_choice.set_callback(move |c| {
            let mut st = state.borrow_mut();
            st.fft_params.window_type = match c.value() {
                0 => {
                    input_kaiser_beta.deactivate();
                    WindowType::Hann
                }
                1 => {
                    input_kaiser_beta.deactivate();
                    WindowType::Hamming
                }
                2 => {
                    input_kaiser_beta.deactivate();
                    WindowType::Blackman
                }
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
        let update_seg_label = shared.update_seg_label.clone();
        let mut input_seg_size = widgets.input_seg_size.clone();
        let mut msg_bar = widgets.msg_bar.clone();
        let suppress_solver_inputs = suppress_solver_inputs.clone();

        let mut seg_preset_choice = widgets.seg_preset_choice.clone();
        seg_preset_choice.set_callback(move |c| {
            let idx = c.value();
            if idx == 9 {
                // "Custom" selected — just focus the text field so user can type
                input_seg_size.take_focus().ok();
                return;
            }
            if (0..9).contains(&idx) {
                let requested_size = SEG_PRESETS[idx as usize];
                let mut st = state.borrow_mut();

                // Clear stale segment target — user is explicitly choosing a size
                st.fft_params.target_segments_per_active = None;
                st.fft_params.target_bins_per_segment = None;

                st.fft_params.window_length = requested_size;
                st.fft_params.last_edited_field = LastEditedField::Overlap;
                apply_segmentation_solver(&mut st);

                // Re-sync widgets after solver (it may have adjusted window_length)
                let final_wl = st.fft_params.window_length;
                let preset_idx = find_preset_index(final_wl).map(|i| i as i32).unwrap_or(9);
                drop(st);

                suppress_solver_inputs.set(true);
                input_seg_size.set_value(&final_wl.to_string());
                c.set_value(preset_idx);
                suppress_solver_inputs.set(false);

                // Message bar feedback if solver changed the user's choice
                if final_wl != requested_size {
                    set_msg(
                        &mut msg_bar,
                        MsgLevel::Warning,
                        &format!(
                            "Segment size {} -> {} (clamped to active range)",
                            requested_size, final_wl
                        ),
                    );
                } else {
                    set_msg(&mut msg_bar, MsgLevel::Info, "");
                }

                (update_info.borrow_mut())();
                (update_seg_label.borrow_mut())();
            }
        });
    }

    // Segment size typed input (fires on every change, like all other text fields)
    {
        let state = state.clone();
        let update_info = shared.update_info.clone();
        let update_seg_label = shared.update_seg_label.clone();
        let mut seg_preset_choice = widgets.seg_preset_choice.clone();
        let mut msg_bar = widgets.msg_bar.clone();
        let suppress_solver_inputs = suppress_solver_inputs.clone();

        let mut input_seg_size = widgets.input_seg_size.clone();
        input_seg_size.set_trigger(CallbackTrigger::Changed);
        input_seg_size.set_callback(move |inp| {
            // Space defense-in-depth: strip any spaces that sneak through handle()
            if inp.value().contains(' ') {
                inp.set_value(&inp.value().replace(' ', ""));
                return;
            }
            if suppress_solver_inputs.get() {
                return;
            }
            if inp.value().trim().is_empty() {
                return;
            }

            let raw: usize = inp.value().parse().unwrap_or(0);
            if raw == 0 {
                return; // partial input like "" after delete — wait for more
            }
            let mut st = state.borrow_mut();
            let max_window = current_active_samples(&st).max(4);
            let clamped = raw.clamp(4, max_window);
            let even = round_even(clamped);

            // Clear stale segment target — user is explicitly choosing a size
            st.fft_params.target_segments_per_active = None;
            st.fft_params.target_bins_per_segment = None;

            st.fft_params.window_length = even;
            st.fft_params.last_edited_field = LastEditedField::Overlap;
            apply_segmentation_solver(&mut st);

            let final_wl = st.fft_params.window_length;
            let preset_idx = find_preset_index(final_wl).map(|i| i as i32).unwrap_or(9);

            suppress_solver_inputs.set(true);
            seg_preset_choice.set_value(preset_idx);
            suppress_solver_inputs.set(false);

            // Message bar feedback if solver clamped
            if final_wl != raw {
                set_msg(
                    &mut msg_bar,
                    MsgLevel::Warning,
                    &format!(
                        "Segment size {} -> {} (must be even, range 4..{})",
                        raw, final_wl, max_window
                    ),
                );
            } else {
                set_msg(&mut msg_bar, MsgLevel::Info, "");
            }

            drop(st);
            (update_info.borrow_mut())();
            (update_seg_label.borrow_mut())();
        });
    }

    {
        let mut input_kaiser_beta = widgets.input_kaiser_beta.clone();
        attach_float_validation(&mut input_kaiser_beta);
    }

    {
        let state = state.clone();
        let update_info = shared.update_info.clone();

        let mut check_center = widgets.check_center.clone();
        check_center.set_callback(move |c| {
            state.borrow_mut().fft_params.use_center = c.is_checked();
            (update_info.borrow_mut())();
        });
    }

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
            let mut st = state.borrow_mut();
            st.fft_params.zero_pad_factor = factor;
            apply_segmentation_solver(&mut st);
            (update_info.borrow_mut())();
        });
    }
}

fn current_active_samples(st: &AppState) -> usize {
    if let Some(ref audio) = st.audio_data {
        let start = st.fft_params.start_sample.min(audio.num_samples());
        let stop = st.fft_params.stop_sample.min(audio.num_samples());
        stop.saturating_sub(start)
    } else {
        0
    }
}

fn apply_segmentation_solver(st: &mut AppState) {
    let active_samples = current_active_samples(st);
    let max_window = active_samples.max(2);
    let out =
        crate::data::segmentation_solver::solve(crate::data::segmentation_solver::SolverInput {
            active_samples,
            window_length: st.fft_params.window_length,
            overlap_percent: st.fft_params.overlap_percent,
            use_center: st.fft_params.use_center,
            zero_pad_factor: st.fft_params.zero_pad_factor,
            target_segments_per_active: st.fft_params.target_segments_per_active,
            target_bins_per_segment: st.fft_params.target_bins_per_segment,
            last_edited: st.fft_params.last_edited_field,
            constraints: SolverConstraints {
                max_window,
                ..SolverConstraints::default()
            },
        });

    st.fft_params.window_length = out.window_length;
    st.fft_params.overlap_percent = out.overlap_percent;
    if st.fft_params.target_segments_per_active.is_none() {
        st.fft_params.target_segments_per_active = Some(out.segments_per_active.max(1));
    }
    if st.fft_params.target_bins_per_segment.is_none() {
        st.fft_params.target_bins_per_segment = Some(out.bins_per_segment.max(1));
    }
    st.dirty = true;
}

// ─── Segment size helpers ─────────────────────────────────────────────────────

const SEG_PRESETS: [usize; 9] = [256, 512, 1024, 2048, 4096, 8192, 16384, 32768, 65536];

fn find_preset_index(size: usize) -> Option<usize> {
    SEG_PRESETS.iter().position(|&p| p == size)
}

fn round_even(n: usize) -> usize {
    if n < 2 {
        2
    } else if !n.is_multiple_of(2) {
        n + 1
    } else {
        n
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  DISPLAY CALLBACKS
// ═══════════════════════════════════════════════════════════════════════════

pub fn setup_display_callbacks(widgets: &Widgets, state: &Rc<RefCell<AppState>>) {
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
            let label = if val <= 0.01 {
                "Scale: Linear".to_string()
            } else if val >= 0.99 {
                "Scale: Log".to_string()
            } else {
                format!("Scale: {:.0}%", val * 100.0)
            };
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

pub fn setup_playback_callbacks(widgets: &Widgets, state: &Rc<RefCell<AppState>>) {
    {
        let state = state.clone();
        let mut btn_rerun = widgets.btn_rerun.clone();

        let mut btn_play = widgets.btn_play.clone();
        btn_play.set_callback(move |_| {
            let mut st = state.borrow_mut();
            if st.dirty {
                // Need to recompute first — set play_pending so playback
                // auto-starts after reconstruction completes.
                st.play_pending = true;
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
            st.transport.position_samples = 0;
        });
    }

    // Scrub slider - seeks within the reconstructed audio
    {
        let state = state.clone();

        let mut scrub_slider = widgets.scrub_slider.clone();
        let mut btn_rerun_scrub = widgets.btn_rerun.clone();
        scrub_slider.handle(move |s, ev| {
            // Block spacebar and trigger recompute on KeyUp
            if fltk::app::event_key() == fltk::enums::Key::from_char(' ') {
                return match ev {
                    fltk::enums::Event::KeyDown | fltk::enums::Event::Shortcut => true,
                    fltk::enums::Event::KeyUp => {
                        btn_rerun_scrub.do_callback();
                        true
                    }
                    _ => false,
                };
            }

            let seek_from_widget_x = |st: &crate::app_state::AppState, mx: i32, widget_w: i32| {
                let roi_start = st.fft_params.start_seconds();
                let roi_stop = st.fft_params.stop_seconds();
                let roi_vis_start = st.view.time_min_sec.max(roi_start);
                let roi_vis_stop = st.view.time_max_sec.min(roi_stop);
                if roi_vis_stop <= roi_vis_start || widget_w <= 0 {
                    return None;
                }

                let x0 = (st.view.time_to_x(roi_vis_start) * widget_w as f64) as i32;
                let x1 = (st.view.time_to_x(roi_vis_stop) * widget_w as f64) as i32;
                let lo = x0.min(x1);
                let hi = x0.max(x1);
                if mx < lo || mx > hi {
                    return None;
                }

                let t = (mx as f64 / widget_w as f64).clamp(0.0, 1.0);
                let global_time = st.view.x_to_time(t).clamp(roi_start, roi_stop);
                let local_time = (global_time - st.recon_start_seconds()).max(0.0);
                let seek_sample = (local_time * st.transport.sample_rate.max(1) as f64) as usize;
                Some(seek_sample.min(st.transport.duration_samples))
            };

            match ev {
                fltk::enums::Event::Push => {
                    let mx = fltk::app::event_x() - s.x();
                    let st = state.borrow();
                    if let Some(seek_sample) = seek_from_widget_x(&st, mx, s.w()) {
                        st.audio_player.set_seeking(true);
                        st.audio_player.seek_to_sample(seek_sample);
                        true
                    } else {
                        false
                    }
                }
                fltk::enums::Event::Drag => {
                    let mx = fltk::app::event_x() - s.x();
                    let st = state.borrow();
                    if let Some(seek_sample) = seek_from_widget_x(&st, mx, s.w()) {
                        st.audio_player.seek_to_sample(seek_sample);
                        true
                    } else {
                        false
                    }
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

    // Render full file outside ROI toggle
    {
        let state = state.clone();
        let mut spec_display = widgets.spec_display.clone();
        let mut freq_axis = widgets.freq_axis.clone();
        let mut time_axis = widgets.time_axis.clone();

        let mut check_render_full_outside_roi = widgets.check_render_full_outside_roi.clone();
        check_render_full_outside_roi.set_callback(move |c| {
            let mut st = state.borrow_mut();
            st.render_full_file_outside_roi = c.is_checked();
            st.invalidate_all_spectrogram_renderers();
            drop(st);
            spec_display.redraw();
            freq_axis.redraw();
            time_axis.redraw();
        });
    }

    // Max freq button — set recon max to Nyquist
    {
        let state = state.clone();
        let mut input_recon_freq_max = widgets.input_recon_freq_max.clone();
        let mut btn_rerun = widgets.btn_rerun.clone();

        let mut btn_freq_max = widgets.btn_freq_max.clone();
        btn_freq_max.set_callback(move |_| {
            let nyquist = state.borrow().view.data_freq_max_hz;
            if nyquist > 0.0 {
                input_recon_freq_max.set_value(&format!("{:.0}", nyquist));
                btn_rerun.do_callback();
            }
        });
    }

    // Home button — snap viewport to reconstruction time + freq range
    {
        let state = state.clone();
        let mut spec_display = widgets.spec_display.clone();
        let mut waveform_display = widgets.waveform_display.clone();
        let mut freq_axis = widgets.freq_axis.clone();
        let mut time_axis = widgets.time_axis.clone();
        let mut scrub_slider = widgets.scrub_slider.clone();

        let mut btn_home = widgets.btn_home.clone();
        btn_home.set_callback(move |_| {
            let mut st = state.borrow_mut();
            let proc_min = st.fft_params.start_seconds();
            let proc_max = st.fft_params.stop_seconds();
            app_log!(
                "Home",
                "proc_time={:.3}-{:.3}s freq={:.0}-{:.0}Hz data_time={:.3}-{:.3}s data_freq={:.0}Hz",
                proc_min, proc_max,
                st.view.recon_freq_min_hz, st.view.recon_freq_max_hz,
                st.view.data_time_min_sec, st.view.data_time_max_sec,
                st.view.data_freq_max_hz
            );
            if proc_max > proc_min {
                st.view.time_min_sec = proc_min.max(st.view.data_time_min_sec);
                st.view.time_max_sec = proc_max.min(st.view.data_time_max_sec);
            }
            // Snap frequency to reconstruction range
            st.view.freq_min_hz = st.view.recon_freq_min_hz.max(1.0);
            st.view.freq_max_hz = st.view.recon_freq_max_hz.min(st.view.data_freq_max_hz);
            st.invalidate_all_spectrogram_renderers();
            st.wave_renderer.invalidate();
            drop(st);
            spec_display.redraw();
            waveform_display.redraw();
            freq_axis.redraw();
            time_axis.redraw();
            scrub_slider.redraw();
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
            app_log!("Settings", "Saved current settings to settings.ini");
        });
    }
}
