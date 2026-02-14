use std::rc::Rc;
use std::cell::RefCell;
use std::sync::{mpsc, Arc};

use fltk::{app, dialog, prelude::*};

use crate::app_state::{AppState, SharedCallbacks, WorkerMessage};
use crate::data::{self, AudioData, TimeUnit, WindowType};
use crate::layout::Widgets;
use crate::processing::fft_engine::FftEngine;
use crate::processing::reconstructor::Reconstructor;
use crate::validation::{parse_or_zero_f64, parse_or_zero_f32, parse_or_zero_usize};
use crate::csv_export;

// ═══════════════════════════════════════════════════════════════════════════
//  FILE OPERATION CALLBACKS
// ═══════════════════════════════════════════════════════════════════════════

pub fn setup_file_callbacks(
    widgets: &Widgets,
    state: &Rc<RefCell<AppState>>,
    tx: &mpsc::Sender<WorkerMessage>,
    shared: &SharedCallbacks,
) {
    setup_open_callback(widgets, state, tx, shared);
    setup_save_fft_callback(widgets, state);
    setup_load_fft_callback(widgets, state, tx, shared);
    setup_save_wav_callback(widgets, state);
}

// ── Open Audio File ──
fn setup_open_callback(
    widgets: &Widgets,
    state: &Rc<RefCell<AppState>>,
    tx: &mpsc::Sender<WorkerMessage>,
    shared: &SharedCallbacks,
) {
    let state = state.clone();
    let mut status_bar = widgets.status_bar.clone();
    let mut input_stop = widgets.input_stop.clone();
    let mut input_recon_freq_max = widgets.input_recon_freq_max.clone();
    let mut spec_display = widgets.spec_display.clone();
    let mut waveform_display = widgets.waveform_display.clone();
    let tx = tx.clone();
    let update_info = shared.update_info.clone();
    let update_seg_label = shared.update_seg_label.clone();
    let enable_audio_widgets = shared.enable_audio_widgets.clone();

    let mut btn_open = widgets.btn_open.clone();
    btn_open.set_callback(move |_| {
        let mut chooser = dialog::NativeFileChooser::new(dialog::NativeFileChooserType::BrowseFile);
        chooser.set_filter("*.wav");
        chooser.show();

        let filename = chooser.filename();
        if filename.as_os_str().is_empty() {
            return;
        }

        status_bar.set_label("Loading audio...");
        app::awake();

        match AudioData::from_wav_file(&filename) {
            Ok(audio) => {
                let duration = audio.duration_seconds;
                let nyquist = audio.nyquist_freq();
                let sample_rate = audio.sample_rate;
                let audio = Arc::new(audio);

                let params_clone;
                {
                    let mut st = state.borrow_mut();
                    st.fft_params.sample_rate = sample_rate;
                    st.fft_params.stop_time = duration;
                    st.audio_data = Some(audio.clone());
                    st.has_audio = true;

                    // Set view bounds
                    st.view.data_time_min_sec = 0.0;
                    st.view.data_time_max_sec = duration;
                    st.view.time_min_sec = 0.0;
                    st.view.time_max_sec = duration;
                    st.view.data_freq_max_hz = nyquist;
                    st.view.freq_max_hz = 5000.0_f32.min(nyquist);
                    st.view.recon_freq_max_hz = nyquist;
                    st.view.max_freq_bins = st.fft_params.num_frequency_bins();
                    st.view.recon_freq_count = st.fft_params.num_frequency_bins();

                    st.transport.duration_seconds = duration;
                    st.transport.position_seconds = 0.0;

                    st.spec_renderer.invalidate();
                    st.wave_renderer.invalidate();

                    params_clone = st.fft_params.clone();
                    st.is_processing = true;
                }

                input_stop.set_value(&format!("{:.5}", duration));
                input_recon_freq_max.set_value(&format!("{:.0}", nyquist));

                (enable_audio_widgets.borrow_mut())();
                (update_info.borrow_mut())();
                (update_seg_label.borrow_mut())();

                // Launch background FFT (reconstruction auto-follows via FftComplete handler)
                let tx_clone = tx.clone();
                let audio_for_fft = audio.clone();
                std::thread::spawn(move || {
                    let spectrogram = FftEngine::process(&audio_for_fft, &params_clone);
                    tx_clone.send(WorkerMessage::FftComplete(spectrogram)).ok();
                });

                status_bar.set_label(&format!(
                    "Processing FFT... | {:.2}s | {} Hz | {}",
                    duration, sample_rate,
                    filename.file_name().unwrap_or_default().to_string_lossy()
                ));
                spec_display.redraw();
                waveform_display.redraw();
            }
            Err(e) => {
                dialog::alert_default(&format!("Error loading audio:\n{}", e));
                status_bar.set_label("Load failed");
            }
        }
    });
}

// ── Save FFT to CSV ──
fn setup_save_fft_callback(
    widgets: &Widgets,
    state: &Rc<RefCell<AppState>>,
) {
    let state = state.clone();
    let mut status_bar = widgets.status_bar.clone();

    let mut btn_save_fft = widgets.btn_save_fft.clone();
    btn_save_fft.set_callback(move |_| {
        let st = state.borrow();
        if st.spectrogram.is_none() {
            dialog::alert_default("No FFT data to save!");
            return;
        }

        let mut chooser = dialog::NativeFileChooser::new(dialog::NativeFileChooserType::BrowseSaveFile);
        chooser.set_filter("*.csv");
        chooser.set_preset_file("fft_data.csv");
        chooser.show();

        let filename = chooser.filename();
        if filename.as_os_str().is_empty() {
            return;
        }

        // Filter spectrogram frames to processing time range (sidebar Start/Stop)
        // so the CSV only contains the section the user cares about.
        let spec_full = st.spectrogram.as_ref().unwrap();
        let proc_time_min = match st.fft_params.time_unit {
            TimeUnit::Seconds => st.fft_params.start_time,
            TimeUnit::Samples => st.fft_params.start_time / st.fft_params.sample_rate.max(1) as f64,
        };
        let proc_time_max = match st.fft_params.time_unit {
            TimeUnit::Seconds => st.fft_params.stop_time,
            TimeUnit::Samples => st.fft_params.stop_time / st.fft_params.sample_rate.max(1) as f64,
        };
        let filtered_frames: Vec<_> = spec_full.frames.iter()
            .filter(|f| f.time_seconds >= proc_time_min && f.time_seconds <= proc_time_max)
            .cloned()
            .collect();
        let filtered_spec = data::Spectrogram::from_frames(filtered_frames);

        match csv_export::export_to_csv(&filtered_spec, &st.fft_params, &st.view, &filename) {
            Ok(_) => {
                status_bar.set_label(&format!(
                    "FFT saved ({} frames, {:.2}s-{:.2}s)",
                    filtered_spec.num_frames(), proc_time_min, proc_time_max
                ));
            }
            Err(e) => {
                dialog::alert_default(&format!("Error saving CSV:\n{}", e));
                status_bar.set_label("Save failed");
            }
        }
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

        status_bar.set_label("Loading CSV...");
        app::awake();

        match csv_export::import_from_csv(&filename) {
            Ok((imported_spec, mut imported_params, recon_params)) => {
                let num_frames = imported_spec.num_frames();

                // Ensure proc_time covers the full spectrogram range
                let spec_min_time = imported_spec.min_time;
                let spec_max_time = imported_spec.max_time;
                imported_params.start_time = spec_min_time;
                imported_params.stop_time = spec_max_time;
                imported_params.time_unit = TimeUnit::Seconds;

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
                    st.view.freq_max_hz = 5000.0_f32.min(imported_spec.max_freq);
                    st.view.data_freq_max_hz = imported_spec.max_freq;

                    // Restore reconstruction params if present
                    if let Some((fc, fmin, fmax)) = recon_params {
                        st.view.recon_freq_count = fc;
                        st.view.recon_freq_min_hz = fmin;
                        st.view.recon_freq_max_hz = fmax;
                    }

                    st.spectrogram = Some(Arc::new(imported_spec));
                    st.spec_renderer.invalidate();
                    st.wave_renderer.invalidate();
                    st.recon_start_time = spec_min_time;
                    st.is_processing = true;
                    st.dirty = false;

                    // Prepare reconstruction data
                    let spec = st.spectrogram.clone().unwrap();
                    let params = st.fft_params.clone();
                    let view = st.view.clone();
                    (spec, params, view, spec_min_time, spec_max_time)
                };

                input_start.set_value(&format!("{:.5}", imported_params.start_time));
                input_stop.set_value(&format!("{:.5}", imported_params.stop_time));
                slider_overlap.set_value(imported_params.overlap_percent as f64);

                (enable_audio_widgets.borrow_mut())();
                (enable_spec_widgets.borrow_mut())();
                (update_info.borrow_mut())();
                (update_seg_label.borrow_mut())();

                status_bar.set_label(&format!(
                    "Loaded {} frames from CSV | Reconstructing...",
                    num_frames
                ));
                spec_display.redraw();

                // Auto-trigger reconstruction so sound can play
                let tx_clone = tx.clone();
                let (spec, params, view, proc_time_min, proc_time_max) = recon_data;
                std::thread::spawn(move || {
                    let filtered_frames: Vec<_> = spec.frames.iter()
                        .filter(|f| f.time_seconds >= proc_time_min && f.time_seconds <= proc_time_max)
                        .cloned()
                        .collect();
                    let filtered_spec = data::Spectrogram::from_frames(filtered_frames);
                    let reconstructed = Reconstructor::reconstruct(&filtered_spec, &params, &view);
                    tx_clone.send(WorkerMessage::ReconstructionComplete(reconstructed)).ok();
                });
            }
            Err(e) => {
                dialog::alert_default(&format!("Error loading CSV:\n{}", e));
                status_bar.set_label("CSV load failed");
            }
        }
    });
}

// ── Export WAV ──
fn setup_save_wav_callback(
    widgets: &Widgets,
    state: &Rc<RefCell<AppState>>,
) {
    let state = state.clone();
    let mut status_bar = widgets.status_bar.clone();

    let mut btn_save_wav = widgets.btn_save_wav.clone();
    btn_save_wav.set_callback(move |_| {
        let st = state.borrow();
        if st.reconstructed_audio.is_none() {
            dialog::alert_default("No reconstructed audio to save!\n\nReconstruct audio first.");
            return;
        }

        let mut chooser = dialog::NativeFileChooser::new(dialog::NativeFileChooserType::BrowseSaveFile);
        chooser.set_filter("*.wav");
        chooser.set_preset_file("reconstructed.wav");
        chooser.show();

        let filename = chooser.filename();
        if filename.as_os_str().is_empty() {
            return;
        }

        match st.reconstructed_audio.as_ref().unwrap().save_wav(&filename) {
            Ok(_) => {
                status_bar.set_label(&format!("WAV saved: {:?}", filename));
            }
            Err(e) => {
                dialog::alert_default(&format!("Error saving WAV:\n{}", e));
                status_bar.set_label("WAV save failed");
            }
        }
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
    let mut spec_display = widgets.spec_display.clone();
    let mut waveform_display = widgets.waveform_display.clone();
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

    let mut btn_rerun = widgets.btn_rerun.clone();
    btn_rerun.set_callback(move |_| {
        // Sync all field values into state before running
        {
            let mut st = state.borrow_mut();
            if st.audio_data.is_none() { return; }
            if st.is_processing { return; }

            // Read current field values for processing time range
            st.fft_params.start_time = parse_or_zero_f64(&input_start.value());
            st.fft_params.stop_time = parse_or_zero_f64(&input_stop.value());

            // Window length is managed by +/- buttons, already in state
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
            st.spec_renderer.invalidate();
            st.wave_renderer.invalidate();
        }

        // FFT processes the FULL file; sidebar time range is for reconstruction only
        let (audio, params) = {
            let st = state.borrow();
            let mut fft_params = st.fft_params.clone();
            // Override start/stop to process full file
            fft_params.start_time = 0.0;
            fft_params.stop_time = st.audio_data.as_ref().unwrap().duration_seconds;
            fft_params.time_unit = TimeUnit::Seconds;
            (st.audio_data.clone().unwrap(), fft_params)
        };

        (update_info.borrow_mut())();
        (update_seg_label.borrow_mut())();
        status_bar.set_label("Processing FFT + Reconstruct...");
        app::awake();

        let tx_clone = tx.clone();
        std::thread::spawn(move || {
            let spectrogram = FftEngine::process(&audio, &params);
            tx_clone.send(WorkerMessage::FftComplete(spectrogram)).ok();
        });

        spec_display.redraw();
        waveform_display.redraw();
    });
}
