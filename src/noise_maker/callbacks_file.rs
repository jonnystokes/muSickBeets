use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc;

use fltk::{dialog, prelude::*};

use crate::app_state::{
    AppState, EngineKind, LoadedDocument, PreparedDocument, WindowKind, WorkerMessage,
};
use crate::audio_gene::AudioGeneProject;
use crate::frame_file::FrameFile;
use crate::layout::Widgets;
use crate::synth::{build_frame_ola_loop, preview_samples_for_frame};

fn set_processing_ui(widgets: &Widgets, processing: bool) {
    let set_btn = |mut b: fltk::button::Button, processing: bool| {
        if processing {
            b.deactivate();
        } else {
            b.activate();
        }
    };

    set_btn(widgets.btn_open_frame.clone(), processing);
    set_btn(widgets.btn_save_audio_gene.clone(), processing);
    set_btn(widgets.btn_load_audio_gene.clone(), processing);
    set_btn(widgets.btn_export_audio.clone(), processing);
    set_btn(widgets.btn_save_defaults.clone(), processing);
    set_btn(widgets.btn_auto_gain.clone(), processing);
    set_btn(widgets.btn_wave_max.clone(), processing);
    set_btn(widgets.btn_wave_zoom_in.clone(), processing);
    set_btn(widgets.btn_wave_zoom_out.clone(), processing);
    set_btn(widgets.btn_amp_zoom_in.clone(), processing);
    set_btn(widgets.btn_amp_zoom_out.clone(), processing);
    set_btn(widgets.btn_spec_freq_zoom_in.clone(), processing);
    set_btn(widgets.btn_spec_freq_zoom_out.clone(), processing);
    set_btn(widgets.btn_home.clone(), processing);
    set_btn(widgets.btn_play.clone(), processing);
    set_btn(widgets.btn_pause.clone(), processing);
    set_btn(widgets.btn_stop.clone(), processing);
    if processing {
        widgets.engine_choice.clone().deactivate();
        widgets.window_choice.clone().deactivate();
        widgets.slider_overlap.clone().deactivate();
        widgets.slider_audio_gain.clone().deactivate();
        widgets.slider_visual_gain.clone().deactivate();
    }
}

pub fn setup_file_callbacks(
    widgets: &Widgets,
    state: &Rc<RefCell<AppState>>,
    tx: &mpsc::Sender<WorkerMessage>,
) {
    setup_open_callback(widgets, state, tx);
    setup_load_audio_gene_callback(widgets, state, tx);
    setup_save_audio_gene_callback(widgets, state, tx);
    setup_export_audio_callback(widgets, state, tx);
}

fn setup_open_callback(
    widgets: &Widgets,
    state: &Rc<RefCell<AppState>>,
    tx: &mpsc::Sender<WorkerMessage>,
) {
    let tx = tx.clone();
    let state = state.clone();
    let widgets_c = crate::widgets_ref_clone(widgets);
    let mut btn = widgets.btn_open_frame.clone();
    btn.set_callback(move |_| {
        let mut chooser = dialog::NativeFileChooser::new(dialog::NativeFileChooserType::BrowseFile);
        chooser.show();
        let path = chooser.filename();
        if path.as_os_str().is_empty() {
            return;
        }

        {
            let mut st = state.borrow_mut();
            st.status.set_activity("Loading file...");
            st.status.start_timing("Load");
        }
        set_processing_ui(&widgets_c, true);

        let defaults = {
            let st = state.borrow();
            (st.engine_kind, st.synth_window, st.overlap_percent)
        };

        let tx = tx.clone();
        std::thread::spawn(move || {
            let result = prepare_document(&path, defaults).map_err(|e| e.to_string());
            if let Err(err) = &result {
                app_log!(
                    "noise_maker",
                    "Prepare document failed for {:?}: {}",
                    path,
                    err
                );
            }
            let _ = tx.send(WorkerMessage::DocumentPrepared(result));
        });
    });
}

fn setup_load_audio_gene_callback(
    widgets: &Widgets,
    state: &Rc<RefCell<AppState>>,
    tx: &mpsc::Sender<WorkerMessage>,
) {
    let tx = tx.clone();
    let state = state.clone();
    let widgets_c = crate::widgets_ref_clone(widgets);
    let mut btn = widgets.btn_load_audio_gene.clone();
    btn.set_callback(move |_| {
        let mut chooser = dialog::NativeFileChooser::new(dialog::NativeFileChooserType::BrowseFile);
        chooser.show();
        let path = chooser.filename();
        if path.as_os_str().is_empty() {
            return;
        }

        {
            let mut st = state.borrow_mut();
            st.status.set_activity("Loading audio_gene...");
            st.status.start_timing("Load");
        }
        set_processing_ui(&widgets_c, true);

        let defaults = {
            let st = state.borrow();
            (st.engine_kind, st.synth_window, st.overlap_percent)
        };

        let tx = tx.clone();
        std::thread::spawn(move || {
            let result = AudioGeneProject::from_path(&path)
                .and_then(|project| {
                    prepare_loaded_document(
                        LoadedDocument::AudioGene(project, path.clone()),
                        defaults,
                    )
                })
                .map_err(|e| e.to_string());
            if let Err(err) = &result {
                app_log!(
                    "noise_maker",
                    "Prepare audio_gene failed for {:?}: {}",
                    path,
                    err
                );
            }
            let _ = tx.send(WorkerMessage::DocumentPrepared(result));
        });
    });
}

fn setup_save_audio_gene_callback(
    widgets: &Widgets,
    state: &Rc<RefCell<AppState>>,
    tx: &mpsc::Sender<WorkerMessage>,
) {
    let tx = tx.clone();
    let state = state.clone();
    let widgets_c = crate::widgets_ref_clone(widgets);
    let mut btn = widgets.btn_save_audio_gene.clone();
    btn.set_callback(move |_| {
        let Some(project) = current_project_snapshot(&state) else {
            return;
        };
        let mut chooser =
            dialog::NativeFileChooser::new(dialog::NativeFileChooserType::BrowseSaveFile);
        chooser.set_preset_file("project.audio_gene");
        chooser.show();
        let path = chooser.filename();
        if path.as_os_str().is_empty() {
            return;
        }

        state
            .borrow_mut()
            .status
            .set_activity("Saving audio_gene...");
        state.borrow_mut().status.start_timing("Save audio_gene");
        set_processing_ui(&widgets_c, true);

        let tx = tx.clone();
        std::thread::spawn(move || {
            let result = project
                .save_to_path(&path)
                .map(|_| path.clone())
                .map_err(|e| e.to_string());
            if let Err(err) = &result {
                app_log!(
                    "noise_maker",
                    "Save audio_gene failed for {:?}: {}",
                    path,
                    err
                );
            }
            let _ = tx.send(WorkerMessage::AudioGeneSaved(result));
        });
    });
}

fn setup_export_audio_callback(
    widgets: &Widgets,
    state: &Rc<RefCell<AppState>>,
    tx: &mpsc::Sender<WorkerMessage>,
) {
    let tx = tx.clone();
    let state = state.clone();
    let widgets_c = crate::widgets_ref_clone(widgets);
    let mut btn = widgets.btn_export_audio.clone();
    btn.set_callback(move |_| {
        let (samples, sr) = {
            let st = state.borrow();
            (st.preview_samples.clone(), st.preview_sample_rate)
        };
        if samples.is_empty() {
            return;
        }
        let mut chooser =
            dialog::NativeFileChooser::new(dialog::NativeFileChooserType::BrowseSaveFile);
        chooser.set_preset_file("noise_maker_preview.wav");
        chooser.show();
        let path = chooser.filename();
        if path.as_os_str().is_empty() {
            return;
        }

        state.borrow_mut().status.set_activity("Exporting audio...");
        state.borrow_mut().status.start_timing("Export audio");
        set_processing_ui(&widgets_c, true);

        let tx = tx.clone();
        std::thread::spawn(move || {
            let result = save_preview_wav(&path, &samples, sr)
                .map(|_| path.clone())
                .map_err(|e| e.to_string());
            if let Err(err) = &result {
                app_log!("noise_maker", "Export audio failed for {:?}: {}", path, err);
            }
            let _ = tx.send(WorkerMessage::AudioExported(result));
        });
    });
}

fn load_document(path: &PathBuf) -> anyhow::Result<LoadedDocument> {
    let text = std::fs::read_to_string(path)?;
    let magic = text.lines().next().unwrap_or_default().trim();
    match magic {
        "MUSICKBEETS_AUDIO_GENE_V1" => {
            let project = AudioGeneProject::parse(&text)?;
            Ok(LoadedDocument::AudioGene(project, path.clone()))
        }
        "MUSICKBEETS_FRAME_V1" => {
            let frame = FrameFile::parse(&text)?;
            Ok(LoadedDocument::Frame(frame, path.clone()))
        }
        _ => {
            let frame = FrameFile::parse(&text)?;
            Ok(LoadedDocument::Frame(frame, path.clone()))
        }
    }
}

fn prepare_document(
    path: &PathBuf,
    defaults: (EngineKind, WindowKind, f32),
) -> anyhow::Result<PreparedDocument> {
    let doc = load_document(path)?;
    prepare_loaded_document(doc, defaults)
}

fn prepare_loaded_document(
    doc: LoadedDocument,
    defaults: (EngineKind, WindowKind, f32),
) -> anyhow::Result<PreparedDocument> {
    let (frame, engine_kind, synth_window, overlap_percent, suggested_auto_gain) = match &doc {
        LoadedDocument::Frame(frame, _) => {
            (frame.clone(), defaults.0, defaults.1, defaults.2, None)
        }
        LoadedDocument::AudioGene(project, _) => (
            project.frame.clone(),
            project.engine_kind,
            project.synth_window,
            project.overlap_percent,
            Some(project.audio_gain_auto),
        ),
    };

    let (playback_loop, playback_boundary_jump, playback_hop_samples, playback_fade_samples) =
        if engine_kind == EngineKind::FrameOla {
            let loop_metrics = build_frame_ola_loop(&frame, synth_window, overlap_percent)?;
            (
                Some(loop_metrics.samples),
                loop_metrics.boundary_jump,
                loop_metrics.hop_samples,
                loop_metrics.fade_samples,
            )
        } else {
            (None, 0.0, 0, 0)
        };
    let preview_samples = if let Some(loop_buf) = &playback_loop {
        loop_buf.iter().copied().cycle().take(262_144).collect()
    } else {
        preview_samples_for_frame(&frame, engine_kind, synth_window, overlap_percent, 262_144)?
    };
    let preview_peak = preview_samples
        .iter()
        .take(10 * frame.sample_rate as usize)
        .map(|v| v.abs())
        .fold(0.0_f32, f32::max);
    let suggested_auto_gain = suggested_auto_gain.or_else(|| {
        Some(if preview_peak > 1e-9 {
            0.98 / preview_peak
        } else {
            1.0
        })
    });

    Ok(PreparedDocument {
        document: doc,
        preview_samples,
        preview_sample_rate: frame.sample_rate,
        suggested_auto_gain,
        playback_loop,
        preview_peak,
        playback_boundary_jump,
        playback_hop_samples,
        playback_fade_samples,
    })
}

fn current_project_snapshot(state: &Rc<RefCell<AppState>>) -> Option<AudioGeneProject> {
    let st = state.borrow();
    let frame = st.frame.clone()?;
    Some(AudioGeneProject {
        current_filename: st.current_filename.clone(),
        engine_kind: st.engine_kind,
        synth_window: st.synth_window,
        overlap_percent: st.overlap_percent,
        audio_gain_user: st.audio_gain_user,
        audio_gain_auto: st.audio_gain_auto,
        wave_time_offset_sec: st.wave_view.time_offset_sec,
        wave_time_span_sec: st.wave_view.time_span_sec,
        wave_amp_visual_gain: st.wave_view.amp_visual_gain,
        spec_freq_min_hz: st.spec_view.freq_min_hz,
        spec_freq_max_hz: st.spec_view.freq_max_hz,
        frame,
    })
}

fn save_preview_wav(path: &PathBuf, samples: &[f32], sample_rate: u32) -> anyhow::Result<()> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec)?;
    for &s in samples {
        writer.write_sample((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)?;
    }
    writer.finalize()?;
    Ok(())
}
