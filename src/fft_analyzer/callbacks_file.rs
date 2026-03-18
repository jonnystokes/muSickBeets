use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{mpsc, Arc};

use fltk::{app, dialog, prelude::*};

use crate::app_state::{update_status_bar, AppState, FftStage, SharedCallbacks, WorkerMessage};
use crate::csv_export;
use crate::data::{AudioData, TimeUnit, WindowType};
use crate::debug_flags;
use crate::layout::Widgets;
use crate::processing::fft_engine::FftEngine;
use crate::processing::reconstructor::Reconstructor;
use crate::validation::{parse_or_zero_f32, parse_or_zero_f64, parse_or_zero_usize};

// ═══════════════════════════════════════════════════════════════════════════
//  FILE OPERATION CALLBACKS
// ═══════════════════════════════════════════════════════════════════════════

pub fn setup_file_callbacks(
    widgets: &Widgets,
    state: &Rc<RefCell<AppState>>,
    tx: &mpsc::Sender<WorkerMessage>,
    shared: &SharedCallbacks,
    win: &fltk::window::Window,
) {
    setup_open_callback(widgets, state, tx, shared, win);
    setup_save_fft_callback(widgets, state, tx, shared);
    setup_load_fft_callback(widgets, state, tx, shared, win);
    setup_save_wav_callback(widgets, state, tx, shared);
}

pub fn spawn_fft_stage(
    state: &Rc<RefCell<AppState>>,
    tx: &mpsc::Sender<WorkerMessage>,
    audio: Arc<AudioData>,
    params: crate::data::FftParams,
    stage: FftStage,
) {
    let cancel = state.borrow_mut().new_cancel_flag();

    let progress = state.borrow().progress_counter.clone();
    progress.store(0, std::sync::atomic::Ordering::Relaxed);
    {
        let total_active = params.stop_sample.saturating_sub(params.start_sample);
        state.borrow_mut().progress_total = params.num_segments(total_active);
    }

    let tx_clone = tx.clone();
    std::thread::spawn(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            FftEngine::process(&audio, &params, &cancel, Some(&progress))
        }));
        match result {
            Ok(spectrogram) => {
                if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                    tx_clone
                        .send(WorkerMessage::Cancelled(stage.label().to_string()))
                        .ok();
                } else {
                    tx_clone
                        .send(WorkerMessage::FftStageComplete(stage, spectrogram))
                        .ok();
                }
            }
            Err(panic) => {
                let msg = panic
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
                    .unwrap_or_else(|| "unknown panic".to_string());
                app_log!("FFT thread", "PANIC: {}", msg);
                tx_clone.send(WorkerMessage::WorkerPanic(msg)).ok();
            }
        }
    });
}

// ── Open Audio File ──
fn setup_open_callback(
    widgets: &Widgets,
    state: &Rc<RefCell<AppState>>,
    tx: &mpsc::Sender<WorkerMessage>,
    shared: &SharedCallbacks,
    _win: &fltk::window::Window,
) {
    let state = state.clone();
    let mut status_bar = widgets.status_bar.clone();
    let tx = tx.clone();
    let shared_cb = shared.clone();

    let mut btn_open = widgets.btn_open.clone();
    btn_open.set_callback(move |_| {
        // Debug: log thread state when Open is clicked
        {
            let st = state.borrow();
            app_log!(
                "Open",
                "is_processing={}, has_audio={}, has_spectrogram={}, has_recon_audio={}, playback_state={:?}",
                st.is_processing,
                st.has_audio,
                st.spectrogram.is_some(),
                st.reconstructed_audio.is_some(),
                st.audio_player.get_state(),
            );
        }

        // Don't allow opening a new file while processing
        {
            let st = state.borrow();
            if st.is_processing {
                update_status_bar(&mut status_bar, "Still processing... please wait.");
                app_log!("Open", "Blocked: still processing");
                return;
            }
        }

        let mut chooser = dialog::NativeFileChooser::new(dialog::NativeFileChooserType::BrowseFile);
        chooser.set_filter("*.wav");
        chooser.show();

        let filename = chooser.filename();
        if filename.as_os_str().is_empty() {
            return;
        }

        // Read normalization settings before spawning thread
        let (do_normalize, norm_peak) = {
            let st = state.borrow();
            (st.normalize_audio, st.normalize_peak)
        };

        // Mark as processing so re-entry is blocked
        {
            let mut st = state.borrow_mut();
            st.is_processing = true;
            st.status.set_activity("Loading audio...");
            st.status.start_timing("Audio load");
        }
        (shared_cb.disable_for_processing.borrow_mut())();
        (shared_cb.set_btn_busy_mode.borrow_mut())();

        update_status_bar(&mut status_bar, "Loading audio...");

        // Move file I/O + normalization to a background thread to keep the GUI responsive.
        // The heavy work (disk read + peak scan) runs off the main thread.
        // State setup happens later in the AudioLoaded handler (main_fft.rs poll loop).
        dbg_log!(
            debug_flags::FILE_IO_DBG,
            "File",
            "Opening audio file: {:?} (normalize={}, peak={:.2})",
            filename,
            do_normalize,
            norm_peak
        );
        app_log!("Open", "Loading file: {:?}", filename);
        let tx_clone = tx.clone();
        let filename_for_thread = filename.clone();
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut audio = AudioData::from_wav_file(&filename_for_thread)
                    .unwrap_or_else(|e| panic!("Failed to load: {}", e));
                app_log!(
                    "Open",
                    "File loaded: {} samples, {} Hz, {:.2}s",
                    audio.num_samples(),
                    audio.sample_rate,
                    audio.duration_seconds
                );

                let norm_gain = if do_normalize {
                    let gain = audio.normalize(norm_peak);
                    if gain != 1.0 {
                        app_log!(
                            "Open",
                            "Audio normalized: gain = {:.3}x (original peak = {:.3})",
                            gain,
                            norm_peak / gain
                        );
                    }
                    gain
                } else {
                    1.0
                };
                (audio, norm_gain)
            }));
            match result {
                Ok((audio, norm_gain)) => {
                    tx_clone.send(WorkerMessage::AudioLoaded(
                        audio, filename_for_thread, norm_gain
                    )).ok();
                }
                Err(panic) => {
                    let msg = panic.downcast_ref::<String>().cloned()
                        .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
                        .unwrap_or_else(|| "unknown panic".to_string());
                    app_log!("Open", "PANIC: {}", msg);
                    tx_clone.send(WorkerMessage::WorkerPanic(msg)).ok();
                }
            }
        });

        update_status_bar(&mut status_bar, "Loading audio file...");
    });
}

// ── Save FFT to CSV ──
fn setup_save_fft_callback(
    widgets: &Widgets,
    state: &Rc<RefCell<AppState>>,
    tx: &mpsc::Sender<WorkerMessage>,
    shared: &SharedCallbacks,
) {
    let state = state.clone();
    let mut status_bar = widgets.status_bar.clone();
    let tx = tx.clone();
    let shared_cb = shared.clone();

    let mut btn_save_fft = widgets.btn_save_fft.clone();
    btn_save_fft.set_callback(move |_| {
        // Extract everything needed from state, then drop the borrow before spawning.
        // Extract the Arc<Spectrogram> + params directly — no frame cloning needed.
        // The export function filters by time range internally.
        let export_data = {
            let st = state.borrow();
            if st.active_spectrogram().is_none() {
                dialog::alert_default("No FFT data to save!");
                return;
            }

            let spec = st.active_spectrogram().unwrap();
            let proc_time_min = st.fft_params.start_seconds();
            let proc_time_max = st.fft_params.stop_seconds();
            let num_frames = spec
                .frames
                .iter()
                .filter(|f| f.time_seconds >= proc_time_min && f.time_seconds <= proc_time_max)
                .count();
            let params = st.fft_params.clone();
            let view = st.view.clone();
            (spec, params, view, proc_time_min, proc_time_max, num_frames)
        };
        // state borrow is dropped here

        let mut chooser =
            dialog::NativeFileChooser::new(dialog::NativeFileChooserType::BrowseSaveFile);
        chooser.set_filter("*.csv");
        chooser.set_preset_file("fft_data.csv");
        chooser.show();

        let filename = chooser.filename();
        if filename.as_os_str().is_empty() {
            return;
        }

        {
            let mut st = state.borrow_mut();
            st.status
                .set_activity(&format!("Saving FFT data ({} frames)...", export_data.5));
            st.status.start_timing("FFT save");
        }
        update_status_bar(&mut status_bar, &state.borrow().status.render());
        let tx_clone = tx.clone();
        let (spec, params, view, proc_time_min, proc_time_max, num_frames) = export_data;
        let num_bins = spec.frequencies.len();
        dbg_log!(
            debug_flags::FILE_IO_DBG,
            "File",
            "Saving FFT CSV: {} frames x {} bins, time range {:.3}s-{:.3}s, file {:?}",
            num_frames,
            num_bins,
            proc_time_min,
            proc_time_max,
            filename
        );
        (shared_cb.set_btn_busy_mode.borrow_mut())();
        std::thread::spawn(move || {
            let result = csv_export::export_to_csv(
                &spec,
                &params,
                &view,
                &filename,
                Some((proc_time_min, proc_time_max)),
            );
            match result {
                Ok(_) => {
                    tx_clone
                        .send(WorkerMessage::CsvSaved(Ok((
                            filename,
                            num_frames,
                            proc_time_min,
                            proc_time_max,
                        ))))
                        .ok();
                }
                Err(e) => {
                    tx_clone
                        .send(WorkerMessage::CsvSaved(Err(format!("{}", e))))
                        .ok();
                }
            }
        });
    });
}

// ── Load FFT from CSV ──
fn setup_load_fft_callback(
    widgets: &Widgets,
    state: &Rc<RefCell<AppState>>,
    tx: &mpsc::Sender<WorkerMessage>,
    shared: &SharedCallbacks,
    _win: &fltk::window::Window,
) {
    let state = state.clone();
    let tx = tx.clone();
    let mut status_bar = widgets.status_bar.clone();
    let shared_cb = shared.clone();

    let mut btn_load_fft = widgets.btn_load_fft.clone();
    btn_load_fft.set_callback(move |_| {
        // Don't allow loading while already processing
        {
            let st = state.borrow();
            if st.is_processing {
                update_status_bar(&mut status_bar, "Still processing... please wait.");
                return;
            }
        }

        let mut chooser = dialog::NativeFileChooser::new(dialog::NativeFileChooserType::BrowseFile);
        chooser.set_filter("*.csv");
        chooser.show();

        let filename = chooser.filename();
        if filename.as_os_str().is_empty() {
            return;
        }

        dbg_log!(
            debug_flags::FILE_IO_DBG,
            "File",
            "Loading FFT CSV from {:?}",
            filename
        );

        // Start timing and set status
        {
            let mut st = state.borrow_mut();
            st.is_processing = true;
            st.status.set_activity("Loading FFT data...");
            st.status.start_timing("FFT load");
        }
        (shared_cb.disable_for_processing.borrow_mut())();
        (shared_cb.set_btn_busy_mode.borrow_mut())();
        let max_chars = ((status_bar.w() - 16).max(40) / 7).max(20) as usize;
        update_status_bar(
            &mut status_bar,
            &state.borrow().status.render_wrapped(max_chars),
        );

        let tx_clone = tx.clone();
        let filename_for_thread = filename.clone();
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                csv_export::import_from_csv(&filename_for_thread)
            }));
            match result {
                Ok(Ok((spec, params, recon, view))) => {
                    tx_clone
                        .send(WorkerMessage::CsvLoaded(Ok((
                            spec,
                            params,
                            recon,
                            view,
                            filename_for_thread,
                        ))))
                        .ok();
                }
                Ok(Err(e)) => {
                    tx_clone
                        .send(WorkerMessage::CsvLoaded(Err(e.to_string())))
                        .ok();
                }
                Err(panic) => {
                    let msg = panic
                        .downcast_ref::<String>()
                        .cloned()
                        .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
                        .unwrap_or_else(|| "unknown panic".to_string());
                    app_log!("CSV load thread", "PANIC: {}", msg);
                    tx_clone.send(WorkerMessage::WorkerPanic(msg)).ok();
                }
            }
        });
    });
}

/// Handle successful CSV/FFT data load. Called from poll_loop when `CsvLoaded(Ok(...))` arrives.
///
/// Sets up spectrogram state, updates UI widgets, and auto-triggers reconstruction.
pub fn handle_csv_load_result(
    imported_spec: crate::data::Spectrogram,
    mut imported_params: crate::data::FftParams,
    recon_params: Option<crate::csv_export::ReconParams>,
    view_params: crate::csv_export::ImportedViewParams,
    filename: std::path::PathBuf,
    state: &Rc<RefCell<AppState>>,
    shared: &SharedCallbacks,
    tx: &mpsc::Sender<WorkerMessage>,
    status_bar: &mut fltk::output::MultilineOutput,
    win: &mut fltk::window::Window,
    spec_display: &mut fltk::widget::Widget,
    input_start: &mut fltk::input::FloatInput,
    input_stop: &mut fltk::input::FloatInput,
    slider_overlap: &mut fltk::valuator::HorNiceSlider,
) {
    let num_frames = imported_spec.num_frames();
    let num_bins = imported_spec.frequencies.len();
    let max_freq = imported_spec.max_freq;
    dbg_log!(
        debug_flags::FILE_IO_DBG,
        "File",
        "FFT CSV loaded: {} frames x {} bins, sr={}, max_freq={:.1}Hz, time {:.5}s-{:.5}s",
        num_frames,
        num_bins,
        imported_params.sample_rate,
        max_freq,
        imported_spec.min_time,
        imported_spec.max_time
    );

    // Ensure proc_time covers the full spectrogram range
    let spec_min_time = imported_spec.min_time;
    let spec_max_time = imported_spec.max_time;
    let sr = imported_params.sample_rate;
    // Convert spectrogram time range to sample counts (ground truth)
    imported_params.start_sample = (spec_min_time * sr as f64).round() as usize;
    imported_params.stop_sample = (spec_max_time * sr as f64).round() as usize;

    // Set filename for status bar and window title
    let csv_fname = filename
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let recon_data = {
        let mut st = state.borrow_mut();
        st.fft_params = imported_params.clone();
        st.current_filename = csv_fname.clone();

        // Compute adaptive dB ceiling from actual data max amplitude
        let max_mag = imported_spec.max_magnitude();
        if max_mag > 0.0 {
            st.view.db_ceiling = 20.0 * max_mag.log10();
        }

        st.view.time_min_sec = spec_min_time;
        st.view.time_max_sec = spec_max_time;
        st.view.data_time_min_sec = spec_min_time;
        st.view.data_time_max_sec = spec_max_time;
        st.view.data_freq_max_hz = imported_spec.max_freq;

        // Restore viewport frequency range if saved, otherwise default
        st.view.freq_min_hz = view_params.freq_min_hz.unwrap_or(1.0).max(1.0);
        st.view.freq_max_hz = view_params
            .freq_max_hz
            .unwrap_or(5000.0_f32.min(imported_spec.max_freq))
            .min(imported_spec.max_freq);

        // Restore reconstruction params if present
        if let Some((fc, fmin, fmax)) = recon_params {
            st.view.recon_freq_count = fc;
            st.view.recon_freq_min_hz = fmin;
            st.view.recon_freq_max_hz = fmax;
        }

        let imported_spec = Arc::new(imported_spec);
        st.spectrogram = Some(imported_spec.clone());
        st.overview_spectrogram = None;
        st.overview_spec_params = None;
        st.focus_spectrogram = Some(imported_spec);
        st.focus_spec_params = Some(imported_params.clone());
        st.invalidate_all_spectrogram_renderers();
        st.wave_renderer.invalidate();
        st.recon_start_sample = imported_params.start_sample;
        st.is_processing = true;
        st.dirty = false;
        let cancel = st.new_cancel_flag();

        // Prepare reconstruction data
        let spec = st.active_spectrogram().unwrap();
        let params = st.fft_params.clone();
        let view = st.view.clone();
        (spec, params, view, spec_min_time, spec_max_time, cancel)
    };

    // Display values based on time_unit (default: Seconds for CSV import)
    input_start.set_value(&format!("{:.5}", imported_params.start_seconds()));
    input_stop.set_value(&format!("{:.5}", imported_params.stop_seconds()));
    slider_overlap.set_value(imported_params.overlap_percent as f64);

    win.set_label(&format!("muSickBeets - {} (FFT)", csv_fname));

    (shared.enable_audio_widgets.borrow_mut())();
    (shared.enable_spec_widgets.borrow_mut())();
    (shared.update_info.borrow_mut())();
    (shared.update_seg_label.borrow_mut())();

    let csv_status = {
        let mut st = state.borrow_mut();
        st.status
            .set_activity(&format!("Reconstructing ({} frames)...", num_frames));
        st.status.finish_timing(); // Finish "FFT load" timing
        st.status.start_timing("Reconstruction");
        st.status.render()
    };
    update_status_bar(status_bar, &csv_status);
    spec_display.redraw();

    // Auto-trigger reconstruction so sound can play.
    // Zero-copy: pass Arc<Spectrogram> + index range instead of cloning frames.
    let tx_clone = tx.clone();
    let (spec, params, view, proc_time_min, proc_time_max, cancel) = recon_data;

    // Compute frame index range for the processing time window
    let frame_start = spec
        .frames
        .iter()
        .position(|f| f.time_seconds >= proc_time_min)
        .unwrap_or(0);
    let frame_end = spec
        .frames
        .iter()
        .rposition(|f| f.time_seconds <= proc_time_max)
        .map(|i| i + 1)
        .unwrap_or(0);

    if frame_start < frame_end {
        if let Some(start_sample) =
            Reconstructor::reconstruction_start_sample(&spec, &params, frame_start..frame_end)
        {
            state.borrow_mut().recon_start_sample = start_sample;
        }
    }

    let progress = state.borrow().progress_counter.clone();
    progress.store(0, std::sync::atomic::Ordering::Relaxed);
    state.borrow_mut().progress_total = frame_end.saturating_sub(frame_start);

    (shared.set_btn_cancel_mode.borrow_mut())();

    std::thread::spawn(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Reconstructor::reconstruct_range(
                &spec,
                &params,
                &view,
                frame_start..frame_end,
                &cancel,
                Some(&progress),
            )
        }));
        match result {
            Ok(reconstructed) => {
                if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                    tx_clone
                        .send(WorkerMessage::Cancelled("Reconstruction".to_string()))
                        .ok();
                } else {
                    tx_clone
                        .send(WorkerMessage::ReconstructionComplete(reconstructed))
                        .ok();
                }
            }
            Err(panic) => {
                let msg = panic
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
                    .unwrap_or_else(|| "unknown panic".to_string());
                app_log!("Reconstruction thread", "PANIC: {}", msg);
                tx_clone.send(WorkerMessage::WorkerPanic(msg)).ok();
            }
        }
    });
}

/// Handle CSV load error. Called from poll_loop when `CsvLoaded(Err(...))` arrives.
pub fn handle_csv_load_error(error_msg: &str) {
    dialog::alert_default(&format!("Error loading CSV:\n{}", error_msg));
}

// ── Export WAV ──
fn setup_save_wav_callback(
    widgets: &Widgets,
    state: &Rc<RefCell<AppState>>,
    tx: &mpsc::Sender<WorkerMessage>,
    shared: &SharedCallbacks,
) {
    let state = state.clone();
    let mut status_bar = widgets.status_bar.clone();
    let tx = tx.clone();
    let shared_cb = shared.clone();

    let mut btn_save_wav = widgets.btn_save_wav.clone();
    btn_save_wav.set_callback(move |_| {
        // Clone audio data out of borrow, then drop the borrow before spawning.
        // This fixes the RefCell-held-during-I/O issue (the borrow is NOT held
        // while the file is being written to disk).
        let audio_clone = {
            let st = state.borrow();
            match st.reconstructed_audio.as_ref() {
                Some(audio) => audio.clone(),
                None => {
                    dialog::alert_default(
                        "No reconstructed audio to save!\n\nReconstruct audio first.",
                    );
                    return;
                }
            }
        };

        let mut chooser =
            dialog::NativeFileChooser::new(dialog::NativeFileChooserType::BrowseSaveFile);
        chooser.set_filter("*.wav");
        chooser.set_preset_file("reconstructed.wav");
        chooser.show();

        let filename = chooser.filename();
        if filename.as_os_str().is_empty() {
            return;
        }

        {
            let mut st = state.borrow_mut();
            st.status.set_activity("Saving WAV...");
            st.status.start_timing("WAV save");
        }
        update_status_bar(&mut status_bar, &state.borrow().status.render());
        dbg_log!(
            debug_flags::FILE_IO_DBG,
            "File",
            "Saving WAV: {} samples, sr={}, file {:?}",
            audio_clone.num_samples(),
            audio_clone.sample_rate,
            filename
        );
        (shared_cb.set_btn_busy_mode.borrow_mut())();
        let tx_clone = tx.clone();
        std::thread::spawn(move || {
            let result = audio_clone.save_wav(&filename);
            match result {
                Ok(_) => {
                    tx_clone.send(WorkerMessage::WavSaved(Ok(filename))).ok();
                }
                Err(e) => {
                    tx_clone
                        .send(WorkerMessage::WavSaved(Err(format!("{}", e))))
                        .ok();
                }
            }
        });
    });
}

// ═══════════════════════════════════════════════════════════════════════════
//  RERUN CALLBACK (Recompute FFT + Reconstruct)
// ═══════════════════════════════════════════════════════════════════════════

pub fn setup_rerun_callback(
    widgets: &Widgets,
    state: &Rc<RefCell<AppState>>,
    tx: &mpsc::Sender<WorkerMessage>,
    shared: &SharedCallbacks,
) {
    let state = state.clone();
    let mut status_bar = widgets.status_bar.clone();
    let tx = tx.clone();
    let input_start = widgets.input_start.clone();
    let input_stop = widgets.input_stop.clone();
    let slider_overlap = widgets.slider_overlap.clone();
    let input_freq_count = widgets.input_freq_count.clone();
    let input_recon_freq_min = widgets.input_recon_freq_min.clone();
    let input_recon_freq_max = widgets.input_recon_freq_max.clone();
    let input_norm_floor = widgets.input_norm_floor.clone();
    let mut lbl_norm_floor_sci = widgets.lbl_norm_floor_sci.clone();
    let check_center = widgets.check_center.clone();
    let shared_cb = shared.clone();
    let update_info = shared.update_info.clone();
    let update_seg_label = shared.update_seg_label.clone();
    let window_type_choice = widgets.window_type_choice.clone();
    let input_kaiser_beta = widgets.input_kaiser_beta.clone();
    let input_seg_size = widgets.input_seg_size.clone();
    let zero_pad_choice = widgets.zero_pad_choice.clone();

    let mut btn_rerun = widgets.btn_rerun.clone();
    btn_rerun.set_callback(move |_| {
        // Check if we have anything to work with
        let has_audio;
        let has_spectrogram;
        {
            let mut st = state.borrow_mut();
            has_audio = st.audio_data.is_some();
            has_spectrogram = st.active_spectrogram().is_some();
            if !has_audio && !has_spectrogram {
                return; // Nothing to process
            }
            if st.is_processing {
                st.cancel_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                st.status.set_activity("Cancelling...");
                drop(st);
                // Don't return -- the cancellation message will arrive via the poll loop
                return;
            }
        }

        // ── Sync reconstruction params from UI (always needed) ──
        {
            let mut st = state.borrow_mut();
            let fc = parse_or_zero_usize(&input_freq_count.value()).max(1);
            st.view.recon_freq_count = fc;
            st.view.recon_freq_min_hz = parse_or_zero_f32(&input_recon_freq_min.value());
            st.view.recon_freq_max_hz = parse_or_zero_f32(&input_recon_freq_max.value());
            st.view.recon_norm_floor = {
                let val: f64 = input_norm_floor.value().parse().unwrap_or(1e-6);
                val.clamp(1e-30, 1e-4)
            };
            // Update the scientific notation display label
            lbl_norm_floor_sci.set_label(&format!(
                "{} = {}",
                crate::validation::format_norm_floor_with_commas_f64(st.view.recon_norm_floor),
                crate::validation::format_scientific_f64(st.view.recon_norm_floor),
            ));
        }

        if has_audio {
            // ── Full path: re-FFT from source audio + reconstruct ──
            let prep_status = {
                let mut st = state.borrow_mut();

                // Read current field values and convert to sample counts
                let sr = st.fft_params.sample_rate as f64;
                match st.fft_params.time_unit {
                    TimeUnit::Seconds => {
                        st.fft_params.start_sample =
                            (parse_or_zero_f64(&input_start.value()) * sr).round() as usize;
                        st.fft_params.stop_sample =
                            (parse_or_zero_f64(&input_stop.value()) * sr).round() as usize;
                    }
                    TimeUnit::Samples => {
                        st.fft_params.start_sample = parse_or_zero_usize(&input_start.value());
                        st.fft_params.stop_sample = parse_or_zero_usize(&input_stop.value());
                    }
                }

                // Read segment size from input field, validate
                let seg_size: usize = parse_or_zero_usize(&input_seg_size.value()).max(2);
                let seg_size = if !seg_size.is_multiple_of(2) {
                    seg_size + 1
                } else {
                    seg_size
                };
                let active_len = st
                    .fft_params
                    .stop_sample
                    .saturating_sub(st.fft_params.start_sample)
                    .max(2);
                st.fft_params.window_length = seg_size.min(active_len);

                // Read zero-pad factor from dropdown
                st.fft_params.zero_pad_factor = match zero_pad_choice.value() {
                    0 => 1,
                    1 => 2,
                    2 => 4,
                    3 => 8,
                    _ => 1,
                };

                st.fft_params.overlap_percent = slider_overlap.value() as f32;
                st.fft_params.use_center = check_center.is_checked();

                // Read window type + kaiser beta
                st.fft_params.window_type = match window_type_choice.value() {
                    0 => WindowType::Rectangular,
                    1 => WindowType::Hann,
                    2 => WindowType::Hamming,
                    3 => WindowType::Blackman,
                    4 => {
                        let beta = parse_or_zero_f32(&input_kaiser_beta.value());
                        WindowType::Kaiser(if beta > 0.0 { beta } else { 8.6 })
                    }
                    _ => WindowType::Hann,
                };

                st.view.max_freq_bins = st.fft_params.num_frequency_bins();

                st.is_processing = true;
                st.dirty = false;
                st.status.set_activity("Preparing FFT...");
                dbg_log!(
                    debug_flags::FFT_DBG,
                    "FFT",
                    "Rerun clicked – preparing window={} overlap={} start={} stop={} @ {}",
                    st.fft_params.window_length,
                    st.fft_params.overlap_percent,
                    st.fft_params.start_sample,
                    st.fft_params.stop_sample,
                    crate::debug_flags::instant_since_start(std::time::Instant::now())
                );
                st.status.render()
            };
            (shared_cb.disable_for_processing.borrow_mut())();
            (shared_cb.set_btn_cancel_mode.borrow_mut())();

            update_status_bar(&mut status_bar, &prep_status);
            app::awake();

            let (audio, overview_params) = {
                let st = state.borrow();
                let total_samples = st.audio_data.as_ref().unwrap().num_samples();
                let overview_params = st.overview_params_for_audio(total_samples);
                (st.audio_data.clone().unwrap(), overview_params)
            };

            // Warn if zero-padded FFT size is very large (memory estimate).
            // Each rayon thread allocates ~2 buffers of n_fft f32s.
            let n_fft = overview_params.n_fft_padded();
            let per_thread_bytes = n_fft * 4 * 2; // input + output buffers
            let est_cores = rayon::current_num_threads();
            let est_peak_mb = (per_thread_bytes * est_cores) / (1024 * 1024);
            if est_peak_mb > 256 {
                app_log!(
                    "FFT",
                    "Warning: large zero-padded FFT (n_fft={}, {}x pad). Estimated peak memory: ~{} MB across {} threads.",
                    n_fft,
                    overview_params.zero_pad_factor,
                    est_peak_mb,
                    est_cores
                );
            }

            (update_info.borrow_mut())();
            (update_seg_label.borrow_mut())();

            // Update status bar BEFORE spawning the worker so the user
            // sees immediate feedback even if the spawn itself is delayed.
            let processing_status = {
                let mut st = state.borrow_mut();
                st.status.set_activity(FftStage::Overview.activity_text());
                st.status.start_timing(FftStage::Overview.label());
                dbg_log!(
                    debug_flags::FFT_DBG,
                    "FFT",
                    "Launching FFT worker (n_fft={}, threads={} ) @ {}",
                    overview_params.n_fft_padded(),
                    rayon::current_num_threads(),
                    crate::debug_flags::instant_since_start(std::time::Instant::now())
                );
                st.status.render()
            };
            update_status_bar(&mut status_bar, &processing_status);
            spawn_fft_stage(&state, &tx, audio, overview_params, FftStage::Overview);
        } else {
            // ── Reconstruction-only path: spectrogram loaded from CSV, no source audio ──
            // Skip FFT, go straight to reconstruction with updated recon params.
            let recon_data = {
                let mut st = state.borrow_mut();
                st.is_processing = true;
                st.dirty = false;
                st.status.clear_timings();
                st.status.start_timing("Reconstruction");
                st.status.set_activity("Reconstructing...");
                let cancel = st.new_cancel_flag();

                let spec = st.active_spectrogram().unwrap();
                let params = st.fft_params.clone();
                let view = st.view.clone();
                let proc_time_min = params.start_seconds();
                let proc_time_max = params.stop_seconds();
                (spec, params, view, proc_time_min, proc_time_max, cancel)
            };
            (shared_cb.disable_for_processing.borrow_mut())();
            (shared_cb.set_btn_cancel_mode.borrow_mut())();

            (update_info.borrow_mut())();

            let recon_status = state.borrow().status.render();
            update_status_bar(&mut status_bar, &recon_status);
            app::awake();

            let tx_clone = tx.clone();
            let (spec, params, view, proc_time_min, proc_time_max, cancel) = recon_data;

            // Compute frame index range for the processing time window
            let frame_start = spec
                .frames
                .iter()
                .position(|f| f.time_seconds >= proc_time_min)
                .unwrap_or(0);
            let frame_end = spec
                .frames
                .iter()
                .rposition(|f| f.time_seconds <= proc_time_max)
                .map(|i| i + 1)
                .unwrap_or(0);

            if frame_start < frame_end {
                if let Some(start_sample) = Reconstructor::reconstruction_start_sample(
                    &spec,
                    &params,
                    frame_start..frame_end,
                ) {
                    state.borrow_mut().recon_start_sample = start_sample;
                }
            }

            dbg_log!(
                debug_flags::FFT_DBG,
                "FFT",
                "Rerun (recon-only): frames {}..{}, freq_count={}, freq_range={:.0}-{:.0}Hz",
                frame_start,
                frame_end,
                view.recon_freq_count,
                view.recon_freq_min_hz,
                view.recon_freq_max_hz
            );

            let progress = state.borrow().progress_counter.clone();
            progress.store(0, std::sync::atomic::Ordering::Relaxed);
            state.borrow_mut().progress_total = frame_end.saturating_sub(frame_start);

            std::thread::spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    Reconstructor::reconstruct_range(
                        &spec,
                        &params,
                        &view,
                        frame_start..frame_end,
                        &cancel,
                        Some(&progress),
                    )
                }));
                match result {
                    Ok(reconstructed) => {
                        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                            tx_clone
                                .send(WorkerMessage::Cancelled("Reconstruction".to_string()))
                                .ok();
                        } else {
                            tx_clone
                                .send(WorkerMessage::ReconstructionComplete(reconstructed))
                                .ok();
                        }
                    }
                    Err(panic) => {
                        let msg = panic
                            .downcast_ref::<String>()
                            .cloned()
                            .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
                            .unwrap_or_else(|| "unknown panic".to_string());
                        app_log!("Reconstruction thread", "PANIC: {}", msg);
                        tx_clone.send(WorkerMessage::WorkerPanic(msg)).ok();
                    }
                }
            });
        }
    });
}
