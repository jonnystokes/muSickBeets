use std::rc::Rc;
use std::cell::RefCell;

use fltk::prelude::*;

use crate::app_state::{AppState, SharedCallbacks, UpdateThrottle};
use crate::data::{ColormapId, FreqScale, TimeUnit, WindowType};
use crate::layout::Widgets;
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

    // Overlap
    {
        let mut lbl = widgets.lbl_overlap_val.clone();
        let state = state.clone();
        let update_info = shared.update_info.clone();

        let mut slider_overlap = widgets.slider_overlap.clone();
        slider_overlap.set_callback(move |s| {
            let val = s.value() as f32;
            lbl.set_label(&format!("Overlap: {}%", val as i32));
            state.borrow_mut().fft_params.overlap_percent = val;
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

    // Segment size +/- buttons
    {
        let state = state.clone();
        let update_info = shared.update_info.clone();
        let update_seg_label = shared.update_seg_label.clone();

        let mut btn_seg_minus = widgets.btn_seg_minus.clone();
        btn_seg_minus.set_callback(move |_| {
            let mut st = state.borrow_mut();
            let new_wl = (st.fft_params.window_length / 2).max(64);
            st.fft_params.window_length = new_wl;
            drop(st);
            (update_info.borrow_mut())();
            (update_seg_label.borrow_mut())();
        });
    }
    {
        let state = state.clone();
        let update_info = shared.update_info.clone();
        let update_seg_label = shared.update_seg_label.clone();

        let mut btn_seg_plus = widgets.btn_seg_plus.clone();
        btn_seg_plus.set_callback(move |_| {
            let mut st = state.borrow_mut();
            let new_wl = (st.fft_params.window_length * 2).min(65536);
            st.fft_params.window_length = new_wl;
            drop(st);
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

        let mut colormap_choice = widgets.colormap_choice.clone();
        colormap_choice.set_callback(move |c| {
            let mut st = state.borrow_mut();
            st.view.colormap = ColormapId::from_index(c.value() as usize);
            st.spec_renderer.invalidate();
            drop(st);
            spec_display.redraw();
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
        scrub_slider.set_callback(move |s| {
            let st = state.borrow();
            let audio_position = s.value() * st.transport.duration_seconds;
            st.audio_player.seek_to(audio_position);
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

    // Home button — snap viewport to processing time range
    {
        let state = state.clone();
        let mut spec_display = widgets.spec_display.clone();
        let mut waveform_display = widgets.waveform_display.clone();

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
                st.spec_renderer.invalidate();
                st.wave_renderer.invalidate();
            }
            drop(st);
            spec_display.redraw();
            waveform_display.redraw();
        });
    }
}
