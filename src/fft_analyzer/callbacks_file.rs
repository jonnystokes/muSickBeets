use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{mpsc, Arc};

use fltk::{app, dialog, prelude::*};

use crate::app_state::{update_status_bar, AppState, SharedCallbacks, WorkerMessage};
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
    setup_save_fft_callback(widgets, state, tx);
    setup_load_fft_callback(widgets, state, tx, shared);
    setup_save_wav_callback(widgets, state, tx);
}

// ── Open Audio File ──
fn setup_open_callback(
    widgets: &Widgets,
    state: &Rc<RefCell<AppState>>,
    tx: &mpsc::Sender<WorkerMessage>,
    _shared: &SharedCallbacks,
    _win: &fltk::window::Window,
) {
    let state = state.clone();
    let mut status_bar = widgets.status_bar.clone();
    let tx = tx.clone();

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
            st.current_activity = "Loading audio...";
        }

        update_status_bar(&mut status_bar, "Loading audio...");

        // Move file I/O + normalization to a background thread to keep the GUI responsive.
        // The heavy work (disk read + peak scan) runs off the main thread.
        // State setup happens later in the AudioLoaded handler (main_fft.rs poll loop).
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
) {
    let state = state.clone();
    let mut status_bar = widgets.status_bar.clone();
    let tx = tx.clone();

    let mut btn_save_fft = widgets.btn_save_fft.clone();
    btn_save_fft.set_callback(move |_| {
        // Extract everything needed from state, then drop the borrow before spawning.
        // Extract the Arc<Spectrogram> + params directly — no frame cloning needed.
        // The export function filters by time range internally.
        let export_data = {
            let st = state.borrow();
            if st.spectrogram.is_none() {
                dialog::alert_default("No FFT data to save!");
                return;
            }

            let spec = st.spectrogram.clone().unwrap();
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

        update_status_bar(&mut status_bar, "Saving CSV...");
        let tx_clone = tx.clone();
        let (spec, params, view, proc_time_min, proc_time_max, num_frames) = export_data;
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
) {
    let state = state.clone();
    let tx = tx.clone();
    let mut status_bar = widgets.status_bar.clone();
    let mut spec_display = widgets.spec_display.clone();
    let mut input_start = widgets.input_start.clone();
    let mut input_stop = widgets.input_stop.clone();
    let mut slider_overlap = widgets.slider_overlap.clone();
    let update_info = shared.update_info.clone();
    let update_seg_label = shared.update_seg_label.clone();
    let enable_audio_widgets = shared.enable_audio_widgets.clone();
    let enable_spec_widgets = shared.enable_spec_widgets.clone();

    let mut btn_load_fft = widgets.btn_load_fft.clone();
    btn_load_fft.set_callback(move |_| {
        let mut chooser = dialog::NativeFileChooser::new(dialog::NativeFileChooserType::BrowseFile);
        chooser.set_filter("*.csv");
        chooser.show();

        let filename = chooser.filename();
        if filename.as_os_str().is_empty() {
            return;
        }

        update_status_bar(&mut status_bar, "Loading CSV...");

        match csv_export::import_from_csv(&filename) {
            Ok((imported_spec, mut imported_params, recon_params, view_params)) => {
                let num_frames = imported_spec.num_frames();

                // Ensure proc_time covers the full spectrogram range
                let spec_min_time = imported_spec.min_time;
                let spec_max_time = imported_spec.max_time;
                let sr = imported_params.sample_rate;
                // Convert spectrogram time range to sample counts (ground truth)
                imported_params.start_sample = (spec_min_time * sr as f64).round() as usize;
                imported_params.stop_sample = (spec_max_time * sr as f64).round() as usize;

                let recon_data = {
                    let mut st = state.borrow_mut();
                    st.fft_params = imported_params.clone();

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

                    st.spectrogram = Some(Arc::new(imported_spec));
                    st.spec_renderer.invalidate();
                    st.wave_renderer.invalidate();
                    st.recon_start_sample = imported_params.start_sample;
                    st.is_processing = true;
                    st.dirty = false;
                    let cancel = st.new_cancel_flag();

                    // Prepare reconstruction data
                    let spec = st.spectrogram.clone().unwrap();
                    let params = st.fft_params.clone();
                    let view = st.view.clone();
                    (spec, params, view, spec_min_time, spec_max_time, cancel)
                };

                // Display values based on time_unit (default: Seconds for CSV import)
                input_start.set_value(&format!("{:.5}", imported_params.start_seconds()));
                input_stop.set_value(&format!("{:.5}", imported_params.stop_seconds()));
                slider_overlap.set_value(imported_params.overlap_percent as f64);

                (enable_audio_widgets.borrow_mut())();
                (enable_spec_widgets.borrow_mut())();
                (update_info.borrow_mut())();
                (update_seg_label.borrow_mut())();

                let csv_status = {
                    let mut st = state.borrow_mut();
                    st.current_activity = "Reconstructing...";
                    st.recon_start_time = Some(std::time::Instant::now());
                    st.last_fft_duration = None; // CSV import has no FFT pass
                    st.last_recon_duration = None;
                    format!(
                        "Loaded {} frames from CSV | {}",
                        num_frames,
                        st.status_bar_text()
                    )
                };
                update_status_bar(&mut status_bar, &csv_status);
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
                    let frame_sr = params.sample_rate as f64;
                    state.borrow_mut().recon_start_sample =
                        (spec.frames[frame_start].time_seconds * frame_sr).round() as usize;
                }

                std::thread::spawn(move || {
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        Reconstructor::reconstruct_range(
                            &spec,
                            &params,
                            &view,
                            frame_start..frame_end,
                            &cancel,
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
            Err(e) => {
                dialog::alert_default(&format!("Error loading CSV:\n{}", e));
                update_status_bar(&mut status_bar, "CSV load failed");
            }
        }
    });
}

// ── Export WAV ──
fn setup_save_wav_callback(
    widgets: &Widgets,
    state: &Rc<RefCell<AppState>>,
    tx: &mpsc::Sender<WorkerMessage>,
) {
    let state = state.clone();
    let mut status_bar = widgets.status_bar.clone();
    let tx = tx.clone();

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

        update_status_bar(&mut status_bar, "Saving WAV...");
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
    let check_center = widgets.check_center.clone();
    let update_info = shared.update_info.clone();
    let update_seg_label = shared.update_seg_label.clone();
    let window_type_choice = widgets.window_type_choice.clone();
    let input_kaiser_beta = widgets.input_kaiser_beta.clone();
    let input_seg_size = widgets.input_seg_size.clone();
    let zero_pad_choice = widgets.zero_pad_choice.clone();

    let mut btn_rerun = widgets.btn_rerun.clone();
    btn_rerun.set_callback(move |_| {
        // Sync all field values into state before running
        let prep_status = {
            let mut st = state.borrow_mut();
            if st.audio_data.is_none() {
                return;
            }
            if st.is_processing {
                return;
            }

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
                0 => WindowType::Hann,
                1 => WindowType::Hamming,
                2 => WindowType::Blackman,
                3 => {
                    let beta = parse_or_zero_f32(&input_kaiser_beta.value());
                    WindowType::Kaiser(if beta > 0.0 { beta } else { 8.6 })
                }
                _ => WindowType::Hann,
            };

            // Update reconstruction params
            let fc = parse_or_zero_usize(&input_freq_count.value()).max(1);
            st.view.recon_freq_count = fc;
            st.view.recon_freq_min_hz = parse_or_zero_f32(&input_recon_freq_min.value());
            st.view.recon_freq_max_hz = parse_or_zero_f32(&input_recon_freq_max.value());
            st.view.max_freq_bins = st.fft_params.num_frequency_bins();

            st.is_processing = true;
            st.dirty = false;
            st.current_activity = "Preparing FFT...";
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
            st.status_bar_text()
        };

        update_status_bar(&mut status_bar, &prep_status);
        app::awake();

        // FFT processes the FULL file; sidebar time range is for reconstruction only
        let (audio, params, cancel) = {
            let mut st = state.borrow_mut();
            let cancel = st.new_cancel_flag();
            let mut fft_params = st.fft_params.clone();
            // Override start/stop to process full file (sample-based)
            fft_params.start_sample = 0;
            fft_params.stop_sample = st.audio_data.as_ref().unwrap().num_samples();
            (st.audio_data.clone().unwrap(), fft_params, cancel)
        };

        // Warn if zero-padded FFT size is very large (memory estimate).
        // Each rayon thread allocates ~2 buffers of n_fft f32s.
        let n_fft = params.n_fft_padded();
        let per_thread_bytes = n_fft * 4 * 2; // input + output buffers
        let est_cores = rayon::current_num_threads();
        let est_peak_mb = (per_thread_bytes * est_cores) / (1024 * 1024);
        if est_peak_mb > 256 {
            app_log!(
                "FFT",
                "Warning: large zero-padded FFT (n_fft={}, {}x pad). Estimated peak memory: ~{} MB across {} threads.",
                n_fft,
                params.zero_pad_factor,
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
            st.current_activity = "Processing FFT (full file)...";
            st.fft_start_time = Some(std::time::Instant::now());
            dbg_log!(
                debug_flags::FFT_DBG,
                "FFT",
                "Launching FFT worker (n_fft={}, threads={} ) @ {}",
                params.n_fft_padded(),
                rayon::current_num_threads(),
                crate::debug_flags::instant_since_start(std::time::Instant::now())
            );
            st.status_bar_text()
        };
        update_status_bar(&mut status_bar, &processing_status);

        let tx_clone = tx.clone();
        std::thread::spawn(move || {
            dbg_log!(
                debug_flags::FFT_DBG,
                "FFT",
                "Worker thread started @ {}",
                crate::debug_flags::instant_since_start(std::time::Instant::now())
            );
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                FftEngine::process(&audio, &params, &cancel)
            }));
            match result {
                Ok(spectrogram) => {
                    if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                        tx_clone
                            .send(WorkerMessage::Cancelled("FFT".to_string()))
                            .ok();
                    } else {
                        tx_clone.send(WorkerMessage::FftComplete(spectrogram)).ok();
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
    });
}
