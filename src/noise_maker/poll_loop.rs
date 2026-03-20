use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::sync::mpsc;

use fltk::{app, prelude::*};

use crate::app_state::{
    AppState, EngineKind, LoadedDocument, PreparedDocument, PreparedEngine, SavedViewState,
    WindowKind, WorkerMessage,
};
use crate::layout::Widgets;

fn set_msg(bar: &mut fltk::frame::Frame, text: &str) {
    bar.set_label(text);
    bar.redraw();
}

fn refresh_status_bar(state: &Rc<RefCell<AppState>>, widgets: &Widgets) {
    let max_chars = ((widgets.status_bar.w() - 16).max(40) / 7).max(20) as usize;
    widgets
        .status_bar
        .clone()
        .set_value(&state.borrow().status.render_wrapped(max_chars));
}

/// Uses the canonical update_info_if_loaded from main.rs (includes health check).
fn update_info(state: &Rc<RefCell<AppState>>, widgets: &Widgets) {
    crate::update_info_if_loaded(state, widgets);
}

fn enable_loaded_controls(widgets: &Widgets) {
    widgets.btn_open_frame.clone().activate();
    widgets.btn_load_audio_gene.clone().activate();
    widgets.btn_save_defaults.clone().activate();
    widgets.engine_choice.clone().activate();
    widgets.slider_audio_gain.clone().activate();
    widgets.slider_visual_gain.clone().activate();
    widgets.btn_play.clone().activate();
    widgets.btn_pause.clone().activate();
    widgets.btn_stop.clone().activate();
    widgets.btn_auto_gain.clone().activate();
    widgets.btn_wave_max.clone().activate();
    widgets.btn_wave_zoom_in.clone().activate();
    widgets.btn_wave_zoom_out.clone().activate();
    widgets.btn_amp_zoom_in.clone().activate();
    widgets.btn_amp_zoom_out.clone().activate();
    widgets.btn_spec_freq_zoom_in.clone().activate();
    widgets.btn_spec_freq_zoom_out.clone().activate();
    widgets.btn_home.clone().activate();
    widgets.btn_save_audio_gene.clone().activate();
    widgets.btn_export_audio.clone().activate();
}

fn sync_engine_relevance(state: &Rc<RefCell<AppState>>, widgets: &Widgets) {
    let is_frame_ola = state.borrow().engine_kind == EngineKind::FrameOla;
    if is_frame_ola {
        widgets.window_choice.clone().activate();
        widgets.slider_overlap.clone().activate();
    } else {
        widgets.window_choice.clone().deactivate();
        widgets.slider_overlap.clone().deactivate();
    }
}

fn enable_idle_controls(widgets: &Widgets) {
    widgets.btn_open_frame.clone().activate();
    widgets.btn_load_audio_gene.clone().activate();
    widgets.btn_save_defaults.clone().activate();
}

fn refresh_all(widgets: &Widgets) {
    widgets.waveform_display.clone().redraw();
    widgets.wave_time_axis.clone().redraw();
    widgets.wave_amp_axis.clone().redraw();
    widgets.spectrogram_display.clone().redraw();
    widgets.spec_freq_axis.clone().redraw();
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

fn apply_loaded_frame(
    state: &Rc<RefCell<AppState>>,
    widgets: &Widgets,
    prepared: PreparedDocument,
    filename: &Path,
    restore_saved_view: Option<SavedViewState>,
) -> anyhow::Result<()> {
    {
        let mut st = state.borrow_mut();
        let frame = match &prepared.document {
            LoadedDocument::Frame(frame, _) => frame.clone(),
            LoadedDocument::AudioGene(project, _) => project.frame.clone(),
        };
        st.current_filename = filename.to_string_lossy().into_owned();
        st.status.finish_timing();
        st.status.set_activity("Preparing synth");
        st.status.start_timing("Synth prep");
        st.frame = Some(frame.clone());
        if let Some(saved) = restore_saved_view {
            st.engine_kind = saved.engine_kind;
            st.synth_window = saved.synth_window;
            st.overlap_percent = saved.overlap_percent;
            st.wave_view.time_offset_sec = saved.wave_time_offset_sec;
            st.wave_view.time_span_sec = saved.wave_time_span_sec;
            st.wave_view.amp_visual_gain = saved.wave_amp_visual_gain;
            st.spec_view.freq_min_hz = saved.spec_freq_min_hz;
            st.spec_view.freq_max_hz = saved.spec_freq_max_hz;
            st.audio_gain_user = saved.audio_gain_user;
            st.audio_gain_auto = saved.audio_gain_auto;
            st.set_combined_audio_gain();
            st.auto_gain_applied_from_load = true;
        } else {
            // Keep current defaults for raw frame load.
            st.spec_view.freq_min_hz = 0.0;
            st.spec_view.freq_max_hz = frame.max_frequency_hz().max(1000.0);
            st.wave_view.time_offset_sec = 0.0;
            st.wave_view.time_span_sec = 0.050;
            st.wave_view.amp_visual_gain = 1.0;
            st.audio_gain_user = 1.0;
            st.audio_gain_auto = 1.0;
        }
        let engine_kind = st.engine_kind;
        let synth_window = st.synth_window;
        let overlap_percent = st.overlap_percent;
        if engine_kind == EngineKind::FrameOla {
            if let Some(loop_buf) = prepared.playback_loop.clone() {
                st.synth.load_loop_buffer(frame.sample_rate, loop_buf)?;
            } else {
                st.synth.load_frame_with_engine(
                    &frame,
                    engine_kind,
                    synth_window,
                    overlap_percent,
                )?;
            }
        } else {
            st.synth
                .load_frame_with_engine(&frame, engine_kind, synth_window, overlap_percent)?;
        }
        st.status.finish_timing();
        st.preview_samples = prepared.preview_samples.clone();
        st.preview_sample_rate = prepared.preview_sample_rate;
        st.preview_peak = prepared.preview_peak;
        st.playback_buffer_len = prepared
            .playback_loop
            .as_ref()
            .map(|b| b.len())
            .unwrap_or(0);
        st.playback_boundary_jump = prepared.playback_boundary_jump;
        st.playback_hop_samples = prepared.playback_hop_samples;
        st.playback_fade_samples = prepared.playback_fade_samples;
        if restore_saved_view.is_none() {
            st.audio_gain_auto = prepared.suggested_auto_gain.unwrap_or(1.0);
            st.auto_gain_applied_from_load = true;
            st.set_combined_audio_gain();
        }
        st.status.set_activity("Ready");
    }

    {
        let st = state.borrow();
        widgets
            .slider_audio_gain
            .clone()
            .set_value(st.audio_gain_user as f64);
        widgets
            .lbl_audio_gain_value
            .clone()
            .set_label(&format!("{:.2}x", st.audio_gain_user));
        widgets
            .slider_visual_gain
            .clone()
            .set_value(st.wave_view.amp_visual_gain as f64);
        widgets
            .lbl_visual_gain_value
            .clone()
            .set_label(&format!("{:.2}x", st.wave_view.amp_visual_gain));
        widgets
            .engine_choice
            .clone()
            .set_value(match st.engine_kind {
                EngineKind::OscBank => 0,
                EngineKind::FrameOla => 1,
            });
        widgets
            .window_choice
            .clone()
            .set_value(match st.synth_window {
                WindowKind::Rectangular => 0,
                WindowKind::Hann => 1,
                WindowKind::Hamming => 2,
                WindowKind::Blackman => 3,
                WindowKind::Kaiser => 4,
            });
        widgets
            .slider_overlap
            .clone()
            .set_value(st.overlap_percent as f64);
        widgets
            .lbl_overlap_value
            .clone()
            .set_label(&format!("{:.0}%", st.overlap_percent));
    }

    enable_loaded_controls(widgets);
    sync_engine_relevance(state, widgets);
    refresh_status_bar(state, widgets);
    refresh_all(widgets);
    sync_spec_scrollbar(state, widgets);
    update_info(state, widgets);
    set_msg(
        &mut widgets.msg_bar.clone(),
        &format!(
            "Loaded {}",
            filename.file_name().unwrap_or_default().to_string_lossy()
        ),
    );
    Ok(())
}

fn apply_prepared_engine(
    state: &Rc<RefCell<AppState>>,
    widgets: &Widgets,
    prepared: PreparedEngine,
) -> anyhow::Result<()> {
    app_log!(
        "noise_maker",
        "Apply prepared engine: engine={:?} window={:?} overlap={:.1}% preview_peak={:.6} buf_len={} hop={} fade={} jump={:.6}",
        prepared.engine_kind,
        prepared.synth_window,
        prepared.overlap_percent,
        prepared.preview_peak,
        prepared.playback_loop.as_ref().map(|b| b.len()).unwrap_or(0),
        prepared.playback_hop_samples,
        prepared.playback_fade_samples,
        prepared.playback_boundary_jump,
    );
    {
        let mut st = state.borrow_mut();
        let playback_buffer_len = prepared
            .playback_loop
            .as_ref()
            .map(|b| b.len())
            .unwrap_or(0);
        st.engine_kind = prepared.engine_kind;
        st.synth_window = prepared.synth_window;
        st.overlap_percent = prepared.overlap_percent;
        if prepared.engine_kind == EngineKind::FrameOla {
            if let Some(loop_buf) = prepared.playback_loop {
                st.synth
                    .load_loop_buffer(prepared.frame.sample_rate, loop_buf)?;
            }
        } else {
            st.synth.load_frame_with_engine(
                &prepared.frame,
                prepared.engine_kind,
                prepared.synth_window,
                prepared.overlap_percent,
            )?;
        }
        st.preview_samples = prepared.preview_samples;
        st.preview_sample_rate = prepared.preview_sample_rate;
        st.preview_peak = prepared.preview_peak;
        st.playback_buffer_len = playback_buffer_len;
        st.playback_boundary_jump = prepared.playback_boundary_jump;
        st.playback_hop_samples = prepared.playback_hop_samples;
        st.playback_fade_samples = prepared.playback_fade_samples;
        st.status.finish_timing();
        st.status.set_activity("Ready");
    }
    enable_loaded_controls(widgets);
    sync_engine_relevance(state, widgets);
    refresh_status_bar(state, widgets);
    refresh_all(widgets);
    sync_spec_scrollbar(state, widgets);
    update_info(state, widgets);
    set_msg(&mut widgets.msg_bar.clone(), "Engine updated");
    Ok(())
}

pub fn start_poll_loop(
    state: &Rc<RefCell<AppState>>,
    widgets: &Widgets,
    win: &fltk::window::Window,
    rx: mpsc::Receiver<WorkerMessage>,
) {
    let state = state.clone();
    let widgets = crate::widgets_ref_clone(widgets);
    let win_poll = win.clone();
    let mut root_poll = widgets.root.clone();
    let mut tick_count: u32 = 0;

    app::add_timeout3(0.016, move |handle| {
        tick_count += 1;
        if tick_count >= 30 {
            tick_count = 0;
            refresh_status_bar(&state, &widgets);
            let max_chars = ((widgets.status_bar.w() - 16).max(40) / 7).max(20) as usize;
            let text = state.borrow().status.render_wrapped(max_chars);
            let line_count = text.lines().count().max(1) as i32;
            let desired_h = (line_count * 17 + 8).max(25);
            if desired_h != widgets.status_bar.h() {
                let menu_h = 25;
                let win_w = win_poll.w();
                let win_h = win_poll.h();
                root_poll.resize(0, menu_h, win_w, win_h - menu_h - desired_h);
                widgets
                    .status_bar
                    .clone()
                    .resize(0, win_h - desired_h, win_w, desired_h);
            }
        }

        while let Ok(msg) = rx.try_recv() {
            match msg {
                WorkerMessage::DocumentPrepared(result) => match result {
                    Ok(prepared) => match prepared.document.clone() {
                        LoadedDocument::Frame(_, path) => {
                            if let Err(err) =
                                apply_loaded_frame(&state, &widgets, prepared, &path, None)
                            {
                                enable_idle_controls(&widgets);
                                state.borrow_mut().status.cancel_timing();
                                state.borrow_mut().status.set_activity("Load failed");
                                set_msg(
                                    &mut widgets.msg_bar.clone(),
                                    &format!("Load failed: {}", err),
                                );
                            }
                        }
                        LoadedDocument::AudioGene(project, path) => {
                            let restore = Some(SavedViewState {
                                engine_kind: project.engine_kind,
                                synth_window: project.synth_window,
                                overlap_percent: project.overlap_percent,
                                wave_time_offset_sec: project.wave_time_offset_sec,
                                wave_time_span_sec: project.wave_time_span_sec,
                                wave_amp_visual_gain: project.wave_amp_visual_gain,
                                spec_freq_min_hz: project.spec_freq_min_hz,
                                spec_freq_max_hz: project.spec_freq_max_hz,
                                audio_gain_user: project.audio_gain_user,
                                audio_gain_auto: project.audio_gain_auto,
                            });
                            if let Err(err) =
                                apply_loaded_frame(&state, &widgets, prepared, &path, restore)
                            {
                                enable_idle_controls(&widgets);
                                state.borrow_mut().status.cancel_timing();
                                state.borrow_mut().status.set_activity("Load failed");
                                set_msg(
                                    &mut widgets.msg_bar.clone(),
                                    &format!("Load failed: {}", err),
                                );
                            }
                        }
                    },
                    Err(err) => {
                        enable_idle_controls(&widgets);
                        state.borrow_mut().status.cancel_timing();
                        state.borrow_mut().status.set_activity("Load failed");
                        set_msg(
                            &mut widgets.msg_bar.clone(),
                            &format!("Load failed: {}", err),
                        );
                    }
                },
                WorkerMessage::AudioGeneSaved(result) => match result {
                    Ok(path) => {
                        enable_loaded_controls(&widgets);
                        state.borrow_mut().status.finish_timing();
                        state.borrow_mut().status.set_activity("Ready");
                        refresh_status_bar(&state, &widgets);
                        set_msg(
                            &mut widgets.msg_bar.clone(),
                            &format!(
                                "Saved {}",
                                path.file_name().unwrap_or_default().to_string_lossy()
                            ),
                        );
                    }
                    Err(err) => {
                        enable_loaded_controls(&widgets);
                        state.borrow_mut().status.cancel_timing();
                        state.borrow_mut().status.set_activity("Save failed");
                        refresh_status_bar(&state, &widgets);
                        set_msg(
                            &mut widgets.msg_bar.clone(),
                            &format!("Save failed: {}", err),
                        );
                    }
                },
                WorkerMessage::EnginePrepared(result) => match result {
                    Ok(prepared) => {
                        if let Err(err) = apply_prepared_engine(&state, &widgets, prepared) {
                            enable_loaded_controls(&widgets);
                            state.borrow_mut().status.cancel_timing();
                            state
                                .borrow_mut()
                                .status
                                .set_activity("Engine update failed");
                            refresh_status_bar(&state, &widgets);
                            set_msg(
                                &mut widgets.msg_bar.clone(),
                                &format!("Engine update failed: {}", err),
                            );
                        }
                    }
                    Err(err) => {
                        enable_loaded_controls(&widgets);
                        state.borrow_mut().status.cancel_timing();
                        state
                            .borrow_mut()
                            .status
                            .set_activity("Engine update failed");
                        refresh_status_bar(&state, &widgets);
                        set_msg(
                            &mut widgets.msg_bar.clone(),
                            &format!("Engine update failed: {}", err),
                        );
                    }
                },
                WorkerMessage::AudioExported(result) => match result {
                    Ok(path) => {
                        enable_loaded_controls(&widgets);
                        state.borrow_mut().status.finish_timing();
                        state.borrow_mut().status.set_activity("Ready");
                        refresh_status_bar(&state, &widgets);
                        set_msg(
                            &mut widgets.msg_bar.clone(),
                            &format!(
                                "Exported {}",
                                path.file_name().unwrap_or_default().to_string_lossy()
                            ),
                        );
                    }
                    Err(err) => {
                        enable_loaded_controls(&widgets);
                        state.borrow_mut().status.cancel_timing();
                        state.borrow_mut().status.set_activity("Export failed");
                        refresh_status_bar(&state, &widgets);
                        set_msg(
                            &mut widgets.msg_bar.clone(),
                            &format!("Export failed: {}", err),
                        );
                    }
                },
            }
        }

        app::repeat_timeout3(0.016, handle);
    });
}
