#[macro_use]
#[allow(dead_code)]
#[path = "../fft_analyzer/debug_flags.rs"]
mod debug_flags;

#[allow(dead_code)]
#[path = "../fft_analyzer/ui/theme.rs"]
mod theme;
#[allow(dead_code)]
#[path = "../fft_analyzer/ui/tooltips.rs"]
mod tooltips;

mod app_state;
mod audio_gene;
mod callbacks_draw;
mod callbacks_file;
mod callbacks_nav;
mod frame_file;
mod layout;
mod poll_loop;
mod settings;
mod synth;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;
use std::thread;

use fltk::{
    app,
    enums::{Event, Key, Shortcut},
    prelude::*,
};

use crate::app_state::{AppState, PreparedEngine, WorkerMessage};
use crate::callbacks_draw::fit_visible_wave_amplitude;
use crate::layout::Widgets;
use crate::settings::AppSettings;
use crate::synth::{build_frame_ola_loop, preview_samples_for_frame};

pub(crate) fn widgets_ref_clone(w: &Widgets) -> Widgets {
    Widgets {
        root: w.root.clone(),
        menu: w.menu.clone(),
        msg_bar: w.msg_bar.clone(),
        btn_key: w.btn_key.clone(),
        btn_open_frame: w.btn_open_frame.clone(),
        btn_save_audio_gene: w.btn_save_audio_gene.clone(),
        btn_load_audio_gene: w.btn_load_audio_gene.clone(),
        btn_export_audio: w.btn_export_audio.clone(),
        btn_save_defaults: w.btn_save_defaults.clone(),
        slider_audio_gain: w.slider_audio_gain.clone(),
        lbl_audio_gain_value: w.lbl_audio_gain_value.clone(),
        btn_auto_gain: w.btn_auto_gain.clone(),
        engine_choice: w.engine_choice.clone(),
        window_choice: w.window_choice.clone(),
        slider_overlap: w.slider_overlap.clone(),
        lbl_overlap_value: w.lbl_overlap_value.clone(),
        slider_visual_gain: w.slider_visual_gain.clone(),
        lbl_visual_gain_value: w.lbl_visual_gain_value.clone(),
        btn_wave_max: w.btn_wave_max.clone(),
        btn_wave_zoom_in: w.btn_wave_zoom_in.clone(),
        btn_wave_zoom_out: w.btn_wave_zoom_out.clone(),
        btn_amp_zoom_in: w.btn_amp_zoom_in.clone(),
        btn_amp_zoom_out: w.btn_amp_zoom_out.clone(),
        btn_spec_freq_zoom_in: w.btn_spec_freq_zoom_in.clone(),
        btn_spec_freq_zoom_out: w.btn_spec_freq_zoom_out.clone(),
        btn_home: w.btn_home.clone(),
        spec_freq_scroll: w.spec_freq_scroll.clone(),
        info_output: w.info_output.clone(),
        waveform_display: w.waveform_display.clone(),
        wave_amp_axis: w.wave_amp_axis.clone(),
        wave_time_axis: w.wave_time_axis.clone(),
        spectrogram_display: w.spectrogram_display.clone(),
        spec_freq_axis: w.spec_freq_axis.clone(),
        cursor_readout: w.cursor_readout.clone(),
        view_readout: w.view_readout.clone(),
        btn_play: w.btn_play.clone(),
        btn_pause: w.btn_pause.clone(),
        btn_stop: w.btn_stop.clone(),
        status_bar: w.status_bar.clone(),
    }
}

fn refresh_all(widgets: &Widgets) {
    widgets.waveform_display.clone().redraw();
    widgets.wave_time_axis.clone().redraw();
    widgets.wave_amp_axis.clone().redraw();
    widgets.spectrogram_display.clone().redraw();
    widgets.spec_freq_axis.clone().redraw();
}

pub(crate) fn update_info_if_loaded(state: &Rc<RefCell<AppState>>, widgets: &Widgets) {
    let Ok(st) = state.try_borrow() else {
        return;
    };
    if let Some(frame) = st.frame.as_ref() {
        widgets.info_output.clone().set_value(&format!(
            "Frame file loaded\n\nFile: {}\nSample rate: {} Hz\nFFT size: {}\nFrame index: {}\nFrame time: {:.5}s\nActive bins: {}\nMax freq: {:.1} Hz\nEngine: {:?}\nWindow: {}\nOverlap: {:.0}%\nWave span: {:.4}s\nAudio gain: {:.2}x ({:.2}x auto * {:.2}x user)\nWave visual gain: {:.2}x\nFreq view: {:.1}-{:.1} Hz\nPreview peak: {:.4}\nPlayback buffer: {} samples ({:.3}s)\nOLA hop: {} samples\nOLA fade: {} samples\nLoop boundary jump: {:.6}\nHealth: {}",
            st.current_filename,
            frame.sample_rate,
            frame.fft_size,
            frame.frame_index,
            frame.frame_time_seconds,
            frame.active_bin_count,
            frame.max_frequency_hz(),
            st.engine_kind,
            st.synth_window.label(),
            st.overlap_percent,
            st.wave_view.time_span_sec,
            st.audio_gain_auto * st.audio_gain_user,
            st.audio_gain_auto,
            st.audio_gain_user,
            st.wave_view.amp_visual_gain,
            st.spec_view.freq_min_hz,
            st.spec_view.freq_max_hz,
            st.preview_peak,
            st.playback_buffer_len,
            st.playback_buffer_len as f64 / st.preview_sample_rate.max(1) as f64,
            st.playback_hop_samples,
            st.playback_fade_samples,
            st.playback_boundary_jump,
            if st.playback_boundary_jump > 0.05 {
                "boundary risk"
            } else if st.preview_peak >= 0.98 {
                "near full-scale"
            } else if st.preview_peak <= 0.05 {
                "very quiet"
            } else {
                "ok"
            },
        ));
        widgets.view_readout.clone().set_label(&format!(
            "{:?} | Wave {:.3}s-{:.3}s | Spec {:.0}-{:.0} Hz | Jump {:.5}",
            st.engine_kind,
            st.wave_view.time_offset_sec,
            st.wave_view.time_offset_sec + st.wave_view.time_span_sec,
            st.spec_view.freq_min_hz,
            st.spec_view.freq_max_hz,
            st.playback_boundary_jump,
        ));
        widgets.view_readout.clone().redraw();
    }
}

fn auto_gain_from_preview(state: &Rc<RefCell<AppState>>) {
    let peak = state
        .borrow()
        .preview_samples
        .iter()
        .take(10 * state.borrow().preview_sample_rate as usize)
        .map(|v| v.abs())
        .fold(0.0_f32, f32::max);
    let mut st = state.borrow_mut();
    st.audio_gain_auto = if peak > 1e-9 { 0.98 / peak } else { 1.0 };
    st.auto_gain_applied_from_load = true;
    st.set_combined_audio_gain();
}

fn sync_spec_scrollbar(state: &Rc<RefCell<AppState>>, widgets: &Widgets) {
    let st = state.borrow();
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
    let mut sb = widgets.spec_freq_scroll.clone();
    sb.set_minimum(0.0);
    sb.set_maximum(1000.0);
    sb.set_slider_size(slider);
    sb.set_value((1000.0 * pos as f64).clamp(0.0, 1000.0));
}

fn update_engine_control_relevance(state: &Rc<RefCell<AppState>>, widgets: &Widgets) {
    let is_frame_ola = state.borrow().engine_kind == crate::app_state::EngineKind::FrameOla;
    if is_frame_ola {
        widgets.window_choice.clone().activate();
        widgets.slider_overlap.clone().activate();
    } else {
        widgets.window_choice.clone().deactivate();
        widgets.slider_overlap.clone().deactivate();
    }
}

fn reset_views(state: &Rc<RefCell<AppState>>, widgets: &Widgets) {
    {
        let mut st = state.borrow_mut();
        st.wave_view.time_offset_sec = 0.0;
        st.wave_view.time_span_sec = 0.050;
        st.wave_view.amp_visual_gain = 1.0;
        st.spec_view.freq_min_hz = 0.0;
        st.spec_view.freq_max_hz = st
            .frame
            .as_ref()
            .map(|f| f.max_frequency_hz().max(1000.0))
            .unwrap_or(5000.0);
    }
    widgets.slider_visual_gain.clone().set_value(1.0);
    widgets.lbl_visual_gain_value.clone().set_label("1.00x");
    sync_spec_scrollbar(state, widgets);
    refresh_all(widgets);
    update_info_if_loaded(state, widgets);
}

fn rebuild_from_current_engine(state: &Rc<RefCell<AppState>>, widgets: &Widgets) {
    widgets.msg_bar.clone().set_label("Rebuilding engine...");
    state
        .borrow_mut()
        .status
        .set_activity("Rebuilding engine...");
    widgets.engine_choice.clone().deactivate();
    widgets.window_choice.clone().deactivate();
    widgets.slider_overlap.clone().deactivate();
    widgets.btn_play.clone().deactivate();
    widgets.btn_pause.clone().deactivate();
    widgets.btn_stop.clone().deactivate();
    widgets.btn_wave_zoom_in.clone().deactivate();
    widgets.btn_wave_zoom_out.clone().deactivate();
    widgets.btn_amp_zoom_in.clone().deactivate();
    widgets.btn_amp_zoom_out.clone().deactivate();
    widgets.btn_spec_freq_zoom_in.clone().deactivate();
    widgets.btn_spec_freq_zoom_out.clone().deactivate();
    widgets.btn_home.clone().deactivate();
}

fn request_engine_prepare(state: &Rc<RefCell<AppState>>, tx: &mpsc::Sender<WorkerMessage>) {
    let maybe = {
        let st = state.borrow();
        st.frame.as_ref().map(|f| {
            (
                f.clone(),
                st.engine_kind,
                st.synth_window,
                st.overlap_percent,
            )
        })
    };
    let Some((frame, engine_kind, synth_window, overlap_percent)) = maybe else {
        return;
    };
    app_log!(
        "noise_maker",
        "Prepare engine request: engine={:?} window={} overlap={:.1}% frame_bins={} fft_size={}",
        engine_kind,
        synth_window.label(),
        overlap_percent,
        frame.active_bin_count,
        frame.fft_size,
    );
    let tx = tx.clone();
    thread::spawn(move || {
        let result = (|| {
            let (
                preview_samples,
                playback_loop,
                playback_boundary_jump,
                playback_hop_samples,
                playback_fade_samples,
            ) = if engine_kind == crate::app_state::EngineKind::FrameOla {
                let loop_metrics = build_frame_ola_loop(&frame, synth_window, overlap_percent)?;
                (
                    loop_metrics
                        .samples
                        .iter()
                        .copied()
                        .cycle()
                        .take(262_144)
                        .collect(),
                    Some(loop_metrics.samples),
                    loop_metrics.boundary_jump,
                    loop_metrics.hop_samples,
                    loop_metrics.fade_samples,
                )
            } else {
                (
                    preview_samples_for_frame(
                        &frame,
                        engine_kind,
                        synth_window,
                        overlap_percent,
                        262_144,
                    )?,
                    None,
                    0.0,
                    0,
                    0,
                )
            };
            let sample_rate = frame.sample_rate;
            Ok::<PreparedEngine, anyhow::Error>(PreparedEngine {
                frame,
                engine_kind,
                synth_window,
                overlap_percent,
                preview_sample_rate: sample_rate,
                preview_peak: preview_samples
                    .iter()
                    .map(|v| v.abs())
                    .fold(0.0_f32, f32::max),
                preview_samples,
                playback_loop,
                playback_boundary_jump,
                playback_hop_samples,
                playback_fade_samples,
            })
        })()
        .map_err(|e| e.to_string());
        let _ = tx.send(WorkerMessage::EnginePrepared(result));
    });
}

fn simple_zoom_callbacks(state: &Rc<RefCell<AppState>>, widgets: &Widgets) {
    {
        let state = state.clone();
        let widgets_c = widgets_ref_clone(widgets);
        let mut btn = widgets.btn_wave_zoom_in.clone();
        btn.set_callback(move |_| {
            let mut st = state.borrow_mut();
            st.wave_view.time_span_sec =
                (st.wave_view.time_span_sec / st.waveform_zoom_factor as f64).max(0.001);
            drop(st);
            refresh_all(&widgets_c);
            update_info_if_loaded(&state, &widgets_c);
        });
    }
    {
        let state = state.clone();
        let widgets_c = widgets_ref_clone(widgets);
        let mut btn = widgets.btn_wave_zoom_out.clone();
        btn.set_callback(move |_| {
            let mut st = state.borrow_mut();
            st.wave_view.time_span_sec = (st.wave_view.time_span_sec
                * st.waveform_zoom_factor as f64)
                .min(st.max_preview_time().max(0.001));
            drop(st);
            refresh_all(&widgets_c);
            update_info_if_loaded(&state, &widgets_c);
        });
    }
    {
        let state = state.clone();
        let widgets_c = widgets_ref_clone(widgets);
        let mut btn = widgets.btn_amp_zoom_in.clone();
        btn.set_callback(move |_| {
            let factor = state.borrow().waveform_amp_zoom_factor;
            state.borrow_mut().wave_view.amp_visual_gain *= factor;
            let gain = state.borrow().wave_view.amp_visual_gain;
            widgets_c.slider_visual_gain.clone().set_value(gain as f64);
            widgets_c
                .lbl_visual_gain_value
                .clone()
                .set_label(&format!("{:.2}x", gain));
            refresh_all(&widgets_c);
            update_info_if_loaded(&state, &widgets_c);
        });
    }
    {
        let state = state.clone();
        let widgets_c = widgets_ref_clone(widgets);
        let mut btn = widgets.btn_amp_zoom_out.clone();
        btn.set_callback(move |_| {
            let factor = state.borrow().waveform_amp_zoom_factor;
            state.borrow_mut().wave_view.amp_visual_gain /= factor;
            let gain = state.borrow().wave_view.amp_visual_gain;
            widgets_c.slider_visual_gain.clone().set_value(gain as f64);
            widgets_c
                .lbl_visual_gain_value
                .clone()
                .set_label(&format!("{:.2}x", gain));
            refresh_all(&widgets_c);
            update_info_if_loaded(&state, &widgets_c);
        });
    }
    {
        let state = state.clone();
        let widgets_c = widgets_ref_clone(widgets);
        let mut btn = widgets.btn_spec_freq_zoom_in.clone();
        btn.set_callback(move |_| {
            let mut st = state.borrow_mut();
            let range = (st.spec_view.freq_max_hz - st.spec_view.freq_min_hz).max(1.0);
            let center = (st.spec_view.freq_min_hz + st.spec_view.freq_max_hz) * 0.5;
            let new_range = (range / st.spec_freq_zoom_factor).max(50.0);
            st.spec_view.freq_min_hz = (center - new_range * 0.5).max(0.0);
            st.spec_view.freq_max_hz = center + new_range * 0.5;
            drop(st);
            sync_spec_scrollbar(&state, &widgets_c);
            refresh_all(&widgets_c);
            update_info_if_loaded(&state, &widgets_c);
        });
    }
    {
        let state = state.clone();
        let widgets_c = widgets_ref_clone(widgets);
        let mut btn = widgets.btn_spec_freq_zoom_out.clone();
        btn.set_callback(move |_| {
            let mut st = state.borrow_mut();
            let maxf = st
                .frame
                .as_ref()
                .map(|f| f.max_frequency_hz())
                .unwrap_or(22_050.0);
            let range = (st.spec_view.freq_max_hz - st.spec_view.freq_min_hz).max(1.0);
            let center = (st.spec_view.freq_min_hz + st.spec_view.freq_max_hz) * 0.5;
            let new_range = (range * st.spec_freq_zoom_factor).min(maxf.max(50.0));
            st.spec_view.freq_min_hz = (center - new_range * 0.5).max(0.0);
            st.spec_view.freq_max_hz = (st.spec_view.freq_min_hz + new_range).min(maxf);
            drop(st);
            sync_spec_scrollbar(&state, &widgets_c);
            refresh_all(&widgets_c);
            update_info_if_loaded(&state, &widgets_c);
        });
    }
}

fn transport_callbacks(state: &Rc<RefCell<AppState>>, widgets: &Widgets) {
    {
        let state = state.clone();
        let mut btn = widgets.btn_play.clone();
        let mut msg = widgets.msg_bar.clone();
        btn.set_callback(move |_| {
            if state.borrow().synth.has_frame() {
                state.borrow_mut().synth.play();
                msg.set_label("Playing continuous frame synthesis");
                msg.redraw();
            }
        });
    }
    {
        let state = state.clone();
        let mut btn = widgets.btn_pause.clone();
        let mut msg = widgets.msg_bar.clone();
        btn.set_callback(move |_| {
            state.borrow_mut().synth.pause();
            msg.set_label("Paused");
            msg.redraw();
        });
    }
    {
        let state = state.clone();
        let mut btn = widgets.btn_stop.clone();
        let mut msg = widgets.msg_bar.clone();
        btn.set_callback(move |_| {
            state.borrow_mut().synth.stop();
            msg.set_label("Stopped and phase reset");
            msg.redraw();
        });
    }
}

fn spec_scroll_callback(state: &Rc<RefCell<AppState>>, widgets: &Widgets) {
    let state = state.clone();
    let widgets_c = widgets_ref_clone(widgets);
    let mut sb = widgets.spec_freq_scroll.clone();
    sb.set_callback(move |s| {
        let mut st = state.borrow_mut();
        let maxf = st
            .frame
            .as_ref()
            .map(|f| f.max_frequency_hz())
            .unwrap_or(22_050.0)
            .max(1.0);
        let range = (st.spec_view.freq_max_hz - st.spec_view.freq_min_hz).max(1.0);
        let frac = (s.value() / s.maximum()).clamp(0.0, 1.0) as f32;
        st.spec_view.freq_min_hz = frac * (maxf - range).max(0.0);
        st.spec_view.freq_max_hz = st.spec_view.freq_min_hz + range;
        drop(st);
        refresh_all(&widgets_c);
        update_info_if_loaded(&state, &widgets_c);
    });
}

fn main() {
    app::set_option(app::Option::FnfcUsesGtk, false);
    app::set_option(app::Option::FnfcUsesZenity, false);
    unsafe {
        std::env::set_var("GIO_USE_VFS", "local");
        std::env::set_var("GIO_USE_VOLUME_MONITOR", "unix");
        std::env::set_var("GVFS_REMOTE_VOLUME_MONITOR_IGNORE", "1");
    }

    let app = app::App::default();
    theme::apply_dark_theme();
    app::set_visual(fltk::enums::Mode::Rgb8).ok();

    let (mut win, widgets) = layout::build_ui();
    let _tooltip_mgr = tooltips::TooltipManager::new();
    let settings = AppSettings::load_or_default();
    let state = Rc::new(RefCell::new(AppState::new()));
    {
        let mut st = state.borrow_mut();
        st.engine_kind = settings.engine_kind;
        st.synth_window = settings.synth_window;
        st.overlap_percent = settings.overlap_percent;
        st.audio_gain_user = settings.audio_gain_user;
        st.wave_view.amp_visual_gain = settings.wave_amp_visual_gain;
    }
    let (tx, rx) = mpsc::channel();

    callbacks_draw::setup_draw_callbacks(&widgets, &state);
    callbacks_nav::setup_shortcut_key_button(&widgets);
    callbacks_nav::setup_menu_callbacks(&widgets, &state);
    callbacks_nav::setup_window_shortcuts(&widgets, &state);
    callbacks_file::setup_file_callbacks(&widgets, &state, &tx);
    poll_loop::start_poll_loop(&state, &widgets, &win, rx);

    widgets
        .engine_choice
        .clone()
        .set_value(match state.borrow().engine_kind {
            crate::app_state::EngineKind::OscBank => 0,
            crate::app_state::EngineKind::FrameOla => 1,
        });
    widgets
        .window_choice
        .clone()
        .set_value(match state.borrow().synth_window {
            crate::app_state::WindowKind::Rectangular => 0,
            crate::app_state::WindowKind::Hann => 1,
            crate::app_state::WindowKind::Hamming => 2,
            crate::app_state::WindowKind::Blackman => 3,
            crate::app_state::WindowKind::Kaiser => 4,
        });
    widgets
        .slider_overlap
        .clone()
        .set_value(state.borrow().overlap_percent as f64);
    widgets
        .lbl_overlap_value
        .clone()
        .set_label(&format!("{:.0}%", state.borrow().overlap_percent));
    widgets
        .slider_audio_gain
        .clone()
        .set_value(state.borrow().audio_gain_user as f64);
    widgets
        .lbl_audio_gain_value
        .clone()
        .set_label(&format!("{:.2}x", state.borrow().audio_gain_user));
    widgets
        .slider_visual_gain
        .clone()
        .set_value(state.borrow().wave_view.amp_visual_gain as f64);
    widgets
        .lbl_visual_gain_value
        .clone()
        .set_label(&format!("{:.2}x", state.borrow().wave_view.amp_visual_gain));
    update_engine_control_relevance(&state, &widgets);

    {
        let state = state.clone();
        let widgets_c = widgets_ref_clone(&widgets);
        let mut slider = widgets.slider_audio_gain.clone();
        slider.set_callback(move |s| {
            let mut st = state.borrow_mut();
            st.audio_gain_user = s.value() as f32;
            st.set_combined_audio_gain();
            widgets_c
                .lbl_audio_gain_value
                .clone()
                .set_label(&format!("{:.2}x", st.audio_gain_user));
            update_info_if_loaded(&state, &widgets_c);
        });
    }

    {
        let state = state.clone();
        let widgets_c = widgets_ref_clone(&widgets);
        let mut slider = widgets.slider_visual_gain.clone();
        slider.set_callback(move |s| {
            state.borrow_mut().wave_view.amp_visual_gain = s.value() as f32;
            widgets_c
                .lbl_visual_gain_value
                .clone()
                .set_label(&format!("{:.2}x", s.value()));
            refresh_all(&widgets_c);
            update_info_if_loaded(&state, &widgets_c);
        });
    }

    {
        let state = state.clone();
        let widgets_c = widgets_ref_clone(&widgets);
        let tx = tx.clone();
        let mut choice = widgets.engine_choice.clone();
        choice.set_callback(move |c| {
            state.borrow_mut().engine_kind = if c.value() == 0 {
                crate::app_state::EngineKind::OscBank
            } else {
                crate::app_state::EngineKind::FrameOla
            };
            rebuild_from_current_engine(&state, &widgets_c);
            request_engine_prepare(&state, &tx);
        });
    }

    {
        let state = state.clone();
        let widgets_c = widgets_ref_clone(&widgets);
        let tx = tx.clone();
        let mut choice = widgets.window_choice.clone();
        choice.set_callback(move |c| {
            state.borrow_mut().synth_window = match c.value() {
                0 => crate::app_state::WindowKind::Rectangular,
                1 => crate::app_state::WindowKind::Hann,
                2 => crate::app_state::WindowKind::Hamming,
                3 => crate::app_state::WindowKind::Blackman,
                _ => crate::app_state::WindowKind::Kaiser,
            };
            rebuild_from_current_engine(&state, &widgets_c);
            request_engine_prepare(&state, &tx);
        });
    }

    {
        let state_change = state.clone();
        let widgets_change = widgets_ref_clone(&widgets);
        let mut slider = widgets.slider_overlap.clone();
        slider.set_callback(move |s| {
            let val = s.value() as f32;
            state_change.borrow_mut().overlap_percent = val;
            widgets_change
                .lbl_overlap_value
                .clone()
                .set_label(&format!("{:.0}%", val));
        });

        let state = state.clone();
        let widgets_c = widgets_ref_clone(&widgets);
        let tx = tx.clone();
        let mut slider_release = widgets.slider_overlap.clone();
        slider_release.handle(move |_, ev| {
            if ev == Event::Released {
                rebuild_from_current_engine(&state, &widgets_c);
                request_engine_prepare(&state, &tx);
                true
            } else {
                false
            }
        });
    }

    {
        let state = state.clone();
        let widgets_c = widgets_ref_clone(&widgets);
        let mut btn = widgets.btn_auto_gain.clone();
        btn.set_callback(move |_| {
            auto_gain_from_preview(&state);
            let st = state.borrow();
            widgets_c
                .lbl_audio_gain_value
                .clone()
                .set_label(&format!("{:.2}x", st.audio_gain_user));
            drop(st);
            update_info_if_loaded(&state, &widgets_c);
            widgets_c
                .msg_bar
                .clone()
                .set_label("Auto gain applied from preview peak");
        });
    }

    {
        let state = state.clone();
        let widgets_c = widgets_ref_clone(&widgets);
        let mut btn = widgets.btn_wave_max.clone();
        btn.set_callback(move |_| {
            fit_visible_wave_amplitude(&state);
            let gain = state.borrow().wave_view.amp_visual_gain;
            widgets_c.slider_visual_gain.clone().set_value(gain as f64);
            widgets_c
                .lbl_visual_gain_value
                .clone()
                .set_label(&format!("{:.2}x", gain));
            refresh_all(&widgets_c);
            update_info_if_loaded(&state, &widgets_c);
        });
    }

    {
        let state = state.clone();
        let widgets_c = widgets_ref_clone(&widgets);
        let mut btn = widgets.btn_home.clone();
        btn.set_callback(move |_| {
            reset_views(&state, &widgets_c);
            widgets_c.msg_bar.clone().set_label("Views reset to home");
        });
    }

    {
        let state = state.clone();
        let mut msg = widgets.msg_bar.clone();
        let mut btn = widgets.btn_save_defaults.clone();
        btn.set_callback(move |_| {
            let st = state.borrow();
            let settings = AppSettings {
                engine_kind: st.engine_kind,
                synth_window: st.synth_window,
                overlap_percent: st.overlap_percent,
                audio_gain_user: st.audio_gain_user,
                wave_amp_visual_gain: st.wave_view.amp_visual_gain,
            };
            match settings.save_to_path(AppSettings::path()) {
                Ok(()) => {
                    msg.set_label("Saved noise_maker defaults");
                    msg.redraw();
                }
                Err(err) => {
                    msg.set_label(&format!("Save defaults failed: {}", err));
                    msg.redraw();
                }
            }
        });
    }

    simple_zoom_callbacks(&state, &widgets);
    transport_callbacks(&state, &widgets);
    spec_scroll_callback(&state, &widgets);

    {
        let mut btn_open = widgets.btn_open_frame.clone();
        let mut btn_play = widgets.btn_play.clone();
        let mut btn_pause = widgets.btn_pause.clone();
        let state_c = state.clone();
        win.handle(move |_, ev| match ev {
            Event::Shortcut => {
                if app::event_state().contains(Shortcut::Ctrl)
                    && app::event_key() == Key::from_char('o')
                {
                    btn_open.do_callback();
                    true
                } else if app::event_state().contains(Shortcut::Ctrl)
                    && app::event_key() == Key::from_char('q')
                {
                    app::quit();
                    true
                } else {
                    false
                }
            }
            Event::KeyUp if app::event_key() == Key::from_char(' ') => {
                if !state_c.borrow().has_frame() {
                    return true;
                }
                if state_c.borrow().synth.has_frame() {
                    let playback_state = state_c.borrow().synth.get_state();
                    if playback_state == crate::synth::PlaybackState::Playing {
                        btn_pause.do_callback();
                    } else {
                        btn_play.do_callback();
                    }
                }
                true
            }
            _ => false,
        });
    }

    win.show();
    app.run().unwrap();
}
