use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::{mpsc, Arc};

use fltk::{app, prelude::*};

use crate::app_state::{format_time, update_status_bar, AppState, SharedCb, WorkerMessage};
use crate::data::TimeUnit;
use crate::playback::audio_player::PlaybackState;
use crate::processing::fft_engine::FftEngine;
use crate::processing::reconstructor::Reconstructor;

// ═══════════════════════════════════════════════════════════════════════════
//  POLL LOOP (16ms timer — worker messages, scrollbar sync, transport)
// ═══════════════════════════════════════════════════════════════════════════

/// Starts the 16ms poll loop that drives the entire application after setup.
///
/// Responsibilities:
/// - Receive and handle `WorkerMessage`s from background FFT/reconstruction threads
/// - Sync scrollbar positions with viewport state
/// - Update transport position (scrub slider, time label, cursor redraws)
/// - Periodic status bar refresh (timing, memory)
pub fn start_poll_loop(
    state: &Rc<RefCell<AppState>>,
    widgets: &crate::layout::Widgets,
    shared: &crate::app_state::SharedCallbacks,
    tx: &mpsc::Sender<WorkerMessage>,
    rx: mpsc::Receiver<WorkerMessage>,
    x_scroll_gen: Rc<Cell<u64>>,
    y_scroll_gen: Rc<Cell<u64>>,
    win: &fltk::window::Window,
) {
    let state = state.clone();
    let mut status_bar = widgets.status_bar.clone();
    let mut spec_display = widgets.spec_display.clone();
    let mut waveform_display = widgets.waveform_display.clone();
    let mut freq_axis = widgets.freq_axis.clone();
    let mut time_axis = widgets.time_axis.clone();
    let mut scrub_slider = widgets.scrub_slider.clone();
    let mut lbl_time = widgets.lbl_time.clone();
    let mut slider_ceiling = widgets.slider_ceiling.clone();
    let mut lbl_ceiling_val = widgets.lbl_ceiling_val.clone();
    let enable_spec_widgets = shared.enable_spec_widgets.clone();
    let enable_wav_export = shared.enable_wav_export.clone();
    let enable_audio_widgets = shared.enable_audio_widgets.clone();
    let update_info = shared.update_info.clone();
    let update_seg_label = shared.update_seg_label.clone();
    let mut input_stop = widgets.input_stop.clone();
    let mut input_recon_freq_max = widgets.input_recon_freq_max.clone();
    let mut x_scroll = widgets.x_scroll.clone();
    let mut y_scroll = widgets.y_scroll.clone();
    let tx = tx.clone();
    let x_scroll_gen = x_scroll_gen.clone();
    let y_scroll_gen = y_scroll_gen.clone();
    let mut win_poll = win.clone();

    // Track last-seen generation to detect user scrollbar interaction
    let mut last_x_gen: u64 = 0;
    let mut last_y_gen: u64 = 0;
    // Counter for periodic status bar refresh (memory, timing)
    let mut status_refresh_counter: u32 = 0;

    app::add_timeout3(0.016, move |handle| {
        // Skip expensive per-tick work when idle: no audio loaded means
        // no scrollbars to sync, no transport to update, no info to refresh.
        // Worker messages (rx) are still polled so FFT completion is handled.
        let is_idle = state
            .try_borrow()
            .map(|st| st.audio_data.is_none() && !st.is_processing)
            .unwrap_or(false);

        if !is_idle {
            (update_info.borrow_mut())();
        }

        // ── Sync scrollbars with view state (skip when idle) ──
        if !is_idle {
            sync_scrollbars(
                &state,
                &mut x_scroll,
                &mut y_scroll,
                &x_scroll_gen,
                &y_scroll_gen,
                &mut last_x_gen,
                &mut last_y_gen,
            );
        }

        // ── Periodic status bar refresh (~every 2s = 125 ticks at 16ms) ──
        status_refresh_counter += 1;
        if status_refresh_counter >= 125 {
            status_refresh_counter = 0;
            let text = state.try_borrow().map(|st| st.status_bar_text()).ok();
            if let Some(text) = text {
                update_status_bar(&mut status_bar, &text);
            }
        }

        // ── Process worker messages ──
        while let Ok(msg) = rx.try_recv() {
            match msg {
                WorkerMessage::FftComplete(spectrogram) => {
                    handle_fft_complete(
                        spectrogram,
                        &state,
                        &mut slider_ceiling,
                        &mut lbl_ceiling_val,
                        &mut status_bar,
                        &mut spec_display,
                        &mut waveform_display,
                        &enable_spec_widgets,
                        &update_info,
                        &tx,
                    );
                }
                WorkerMessage::ReconstructionComplete(reconstructed) => {
                    handle_reconstruction_complete(
                        reconstructed,
                        &state,
                        &mut status_bar,
                        &mut spec_display,
                        &mut waveform_display,
                        &mut freq_axis,
                        &mut time_axis,
                        &enable_wav_export,
                    );
                }
                WorkerMessage::AudioLoaded(audio, filename, norm_gain) => {
                    handle_audio_loaded(
                        audio,
                        filename,
                        norm_gain,
                        &state,
                        &mut status_bar,
                        &mut spec_display,
                        &mut waveform_display,
                        &mut input_stop,
                        &mut input_recon_freq_max,
                        &mut win_poll,
                        &enable_audio_widgets,
                        &update_info,
                        &update_seg_label,
                        &tx,
                    );
                }
                WorkerMessage::WavSaved(result) => match result {
                    Ok(path) => {
                        let msg = format!("WAV saved: {:?}", path);
                        update_status_bar(&mut status_bar, &msg);
                    }
                    Err(msg) => {
                        fltk::dialog::alert_default(&format!("Error saving WAV:\n{}", msg));
                        update_status_bar(&mut status_bar, "WAV save failed");
                    }
                },
                WorkerMessage::CsvSaved(result) => match result {
                    Ok((_, num_frames, time_min, time_max)) => {
                        let msg = format!(
                            "FFT saved ({} frames, {:.2}s-{:.2}s)",
                            num_frames, time_min, time_max
                        );
                        update_status_bar(&mut status_bar, &msg);
                    }
                    Err(msg) => {
                        fltk::dialog::alert_default(&format!("Error saving CSV:\n{}", msg));
                        update_status_bar(&mut status_bar, "Save failed");
                    }
                },
                WorkerMessage::WorkerPanic(msg) => {
                    app_log!("Worker", "PANIC: {}", msg);
                    {
                        let mut st = state.borrow_mut();
                        st.is_processing = false;
                        st.play_pending = false;
                        st.current_activity = "Ready";
                        st.fft_start_time = None;
                        st.recon_start_time = None;
                    }
                    let msg_text = format!("Error: worker thread panicked: {}", msg);
                    update_status_bar(&mut status_bar, &msg_text);
                }
                WorkerMessage::Cancelled(what) => {
                    app_log!("Worker", "Cancelled: {}", what);
                    // Don't reset is_processing or current_activity —
                    // a new operation likely replaced this one.
                }
            }
        }

        // Check for disconnected channel (worker panicked without sending)
        if state.borrow().is_processing {
            use std::sync::mpsc::TryRecvError;
            if let Err(TryRecvError::Disconnected) = rx.try_recv() {
                app_log!(
                    "Worker",
                    "Channel disconnected — worker thread likely panicked without sending a message"
                );
                let mut st = state.borrow_mut();
                st.is_processing = false;
                st.play_pending = false;
                drop(st);
                update_status_bar(
                    &mut status_bar,
                    "Error: processing failed (worker thread lost)",
                );
            }
        }

        // ── Update transport position ──
        update_transport(
            &state,
            &mut scrub_slider,
            &mut lbl_time,
            &mut spec_display,
            &mut waveform_display,
        );

        app::repeat_timeout3(0.016, handle);
    });
}

// ═══════════════════════════════════════════════════════════════════════════
//  SCROLLBAR SYNC
// ═══════════════════════════════════════════════════════════════════════════

fn sync_scrollbars(
    state: &Rc<RefCell<AppState>>,
    x_scroll: &mut fltk::valuator::Scrollbar,
    y_scroll: &mut fltk::valuator::Scrollbar,
    x_scroll_gen: &Rc<Cell<u64>>,
    y_scroll_gen: &Rc<Cell<u64>>,
    last_x_gen: &mut u64,
    last_y_gen: &mut u64,
) {
    let cur_x_gen = x_scroll_gen.get();
    let cur_y_gen = y_scroll_gen.get();
    let x_user_active = cur_x_gen != *last_x_gen;
    let y_user_active = cur_y_gen != *last_y_gen;
    *last_x_gen = cur_x_gen;
    *last_y_gen = cur_y_gen;

    let scroll_data = if let Ok(st) = state.try_borrow() {
        let data_time_range = st.view.data_time_max_sec - st.view.data_time_min_sec;
        let data_freq_min = 1.0_f32;
        let data_freq_range = st.view.data_freq_max_hz - data_freq_min;

        let x_data = if data_time_range > 0.001 {
            let vis_time = st.view.visible_time_range();
            let ratio = (vis_time / data_time_range).clamp(0.02, 1.0) as f32;
            let scroll_range = (data_time_range - vis_time).max(0.0);
            let frac = if scroll_range > 0.001 {
                ((st.view.time_min_sec - st.view.data_time_min_sec) / scroll_range).clamp(0.0, 1.0)
            } else {
                0.0
            };
            Some((ratio, frac * 10000.0))
        } else {
            None
        };

        let y_data = if data_freq_range > 1.0 {
            let vis_freq = st.view.visible_freq_range();
            let ratio = (vis_freq / data_freq_range).clamp(0.02, 1.0);
            let scroll_range = (data_freq_range - vis_freq).max(0.0);
            let frac = if scroll_range > 0.1 {
                ((st.view.freq_min_hz - data_freq_min) / scroll_range).clamp(0.0, 1.0) as f64
            } else {
                0.0
            };
            Some((ratio, (1.0 - frac) * 10000.0))
        } else {
            None
        };

        Some((x_data, y_data))
    } else {
        None
    };

    if let Some((x_data, y_data)) = scroll_data {
        if let Some((sz, pos)) = x_data {
            x_scroll.set_slider_size(sz);
            if !x_user_active {
                x_scroll.set_value(pos);
            }
        }
        if let Some((sz, pos)) = y_data {
            y_scroll.set_slider_size(sz);
            if !y_user_active {
                y_scroll.set_value(pos);
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  WORKER MESSAGE HANDLERS
// ═══════════════════════════════════════════════════════════════════════════

fn handle_fft_complete(
    spectrogram: crate::data::Spectrogram,
    state: &Rc<RefCell<AppState>>,
    slider_ceiling: &mut fltk::valuator::HorNiceSlider,
    lbl_ceiling_val: &mut fltk::frame::Frame,
    status_bar: &mut fltk::output::Output,
    spec_display: &mut fltk::widget::Widget,
    waveform_display: &mut fltk::widget::Widget,
    enable_spec_widgets: &SharedCb,
    update_info: &SharedCb,
    tx: &mpsc::Sender<WorkerMessage>,
) {
    dbg_log!(
        crate::debug_flags::FFT_DBG,
        "FFT",
        "FftComplete received – {} frames @ {}",
        spectrogram.num_frames(),
        crate::debug_flags::instant_since_start(std::time::Instant::now())
    );
    // Store spectrogram, then auto-reconstruct
    let recon_data = {
        let mut st = state.borrow_mut();

        st.view.max_freq_bins = st.fft_params.num_frequency_bins();

        // Compute adaptive dB ceiling from actual data max amplitude
        let max_mag = spectrogram.max_magnitude();
        if max_mag > 0.0 {
            st.view.db_ceiling = 20.0 * max_mag.log10();
        }

        let spec_arc = Arc::new(spectrogram);
        let (min_t, max_t, max_f) = (spec_arc.min_time, spec_arc.max_time, spec_arc.max_freq);

        st.spectrogram = Some(spec_arc);

        // Update data bounds (full file range)
        st.view.data_time_min_sec = min_t;
        st.view.data_time_max_sec = max_t;
        if max_f > 0.0 {
            st.view.data_freq_max_hz = max_f;
        }

        // Set viewport to full file range on first load
        if st.view.time_max_sec <= 0.0 || st.view.time_max_sec == st.view.time_min_sec {
            st.view.time_min_sec = min_t;
            st.view.time_max_sec = max_t;
        }
        if max_f > 0.0 && st.view.freq_max_hz <= 1.0 {
            st.view.freq_max_hz = max_f;
        }

        // Prepare reconstruction: filter spectrogram to processing time range
        let spec = st.spectrogram.clone().unwrap();
        let params = st.fft_params.clone();
        let view = st.view.clone();

        // Get processing time range (derived from sample counts)
        let proc_time_min = params.start_seconds();
        let proc_time_max = params.stop_seconds();

        st.recon_start_sample = params.start_sample;

        (spec, params, view, proc_time_min, proc_time_max)
    };

    // Sync ceiling slider to auto-computed dB ceiling
    {
        let st = state.borrow();
        let ceil = st.view.db_ceiling;
        slider_ceiling.set_value(ceil as f64);
        lbl_ceiling_val.set_label(&format!("Ceiling: {} dB", ceil as i32));
    }

    (enable_spec_widgets.borrow_mut())();
    (update_info.borrow_mut())();

    // Record FFT timing, update activity to reconstruction, then invalidate renderers
    let recon_status = {
        let mut st = state.borrow_mut();
        if let Some(start) = st.fft_start_time.take() {
            st.last_fft_duration = Some(start.elapsed());
        }
        st.current_activity = "Reconstructing...";
        st.recon_start_time = Some(std::time::Instant::now());
        st.spec_renderer.invalidate();
        st.wave_renderer.invalidate();
        st.status_bar_text()
    };
    update_status_bar(status_bar, &recon_status);

    // Auto-trigger reconstruction with time-filtered spectrogram.
    // Instead of cloning frames, compute index range and pass
    // the Arc<Spectrogram> directly (zero-copy, ~49 MB savings).
    let tx_clone = tx.clone();
    let (spec, params, view, proc_time_min, proc_time_max) = recon_data;

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
        .map(|i| i + 1) // exclusive end
        .unwrap_or(0);

    // Set recon_start_sample from actual first frame time
    // and get a fresh cancel flag for the reconstruction phase
    let cancel = {
        let mut st = state.borrow_mut();
        if frame_start < frame_end {
            let sr = params.sample_rate as f64;
            st.recon_start_sample = (spec.frames[frame_start].time_seconds * sr).round() as usize;
        }
        st.new_cancel_flag()
    };

    dbg_log!(
        crate::debug_flags::FFT_DBG,
        "FFT",
        "Spawning reconstruction worker (frames {}..{}) @ {}",
        frame_start,
        frame_end,
        crate::debug_flags::instant_since_start(std::time::Instant::now())
    );

    std::thread::spawn(move || {
        dbg_log!(
            crate::debug_flags::FFT_DBG,
            "FFT",
            "Reconstruction worker running @ {}",
            crate::debug_flags::instant_since_start(std::time::Instant::now())
        );
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Reconstructor::reconstruct_range(&spec, &params, &view, frame_start..frame_end, &cancel)
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

    // Redraw displays to show new spectrogram
    spec_display.redraw();
    waveform_display.redraw();
}

fn handle_reconstruction_complete(
    mut reconstructed: crate::data::AudioData,
    state: &Rc<RefCell<AppState>>,
    status_bar: &mut fltk::output::Output,
    spec_display: &mut fltk::widget::Widget,
    waveform_display: &mut fltk::widget::Widget,
    freq_axis: &mut fltk::widget::Widget,
    time_axis: &mut fltk::widget::Widget,
    enable_wav_export: &SharedCb,
) {
    dbg_log!(
        crate::debug_flags::FFT_DBG,
        "FFT",
        "ReconstructionComplete received ({} samples) @ {}",
        reconstructed.num_samples(),
        crate::debug_flags::instant_since_start(std::time::Instant::now())
    );
    // Record reconstruction timing and update status text before redraws
    let ready_status = {
        let mut st = state.borrow_mut();
        if let Some(start) = st.recon_start_time.take() {
            st.last_recon_duration = Some(start.elapsed());
        }
        st.current_activity = "Ready";
        st.status_bar_text()
    };

    // Normalize reconstructed audio for proper playback volume
    {
        let st = state.borrow();
        if st.normalize_audio {
            reconstructed.normalize(st.normalize_peak);
        }
    }
    let recon_result = {
        let mut st = state.borrow_mut();
        // Wrap samples in Arc for the player. Currently still
        // clones the Vec; true zero-copy requires AudioData.samples
        // to become Arc<Vec<f32>> (planned for Category 7).
        let playback_samples = Arc::new(reconstructed.samples.clone());
        match st
            .audio_player
            .load_audio(playback_samples, reconstructed.sample_rate)
        {
            Ok(_) => {
                let num_smp = reconstructed.num_samples();
                let sr = reconstructed.sample_rate;
                st.transport.duration_samples = num_smp;
                st.transport.sample_rate = sr;
                st.wave_renderer.invalidate();

                st.reconstructed_audio = Some(reconstructed);
                st.is_processing = false;
                st.dirty = false;

                // Auto-start playback if Play was pressed while dirty
                let should_play = st.play_pending;
                st.play_pending = false;
                if should_play {
                    st.audio_player.play();
                    st.transport.is_playing = true;
                }

                Ok((num_smp, sr))
            }
            Err(e) => {
                st.is_processing = false;
                st.play_pending = false;
                Err(e)
            }
        }
    };
    match recon_result {
        Ok((_num_smp, _sr)) => {
            (enable_wav_export.borrow_mut())();
            update_status_bar(status_bar, &ready_status);
            spec_display.redraw();
            waveform_display.redraw();
            freq_axis.redraw();
            time_axis.redraw();

            // If "Lock to Active" is on, snap viewport to processing
            // range (time + frequency) after a short delay so the UI
            // has time to finish updating renderers/redraws.
            let lock_active = state.borrow().lock_to_active;
            if lock_active {
                let state_lock = state.clone();
                let mut spec_d = spec_display.clone();
                let mut wave_d = waveform_display.clone();
                let mut freq_a = freq_axis.clone();
                let mut time_a = time_axis.clone();
                app::add_timeout3(0.5, move |_| {
                    let mut st = state_lock.borrow_mut();
                    // Snap time to reconstruction range
                    let proc_min = st.recon_start_seconds();
                    let proc_max = proc_min + st.transport.duration_seconds();
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
                    spec_d.redraw();
                    wave_d.redraw();
                    freq_a.redraw();
                    time_a.redraw();
                });
            }
        }
        Err(e) => {
            let msg = format!("Reconstruction error: {}", e);
            update_status_bar(status_bar, &msg);
            fltk::dialog::alert_default(&format!("Failed to load reconstructed audio:\n{}", e));
        }
    }
}

fn handle_audio_loaded(
    audio: crate::data::AudioData,
    filename: std::path::PathBuf,
    norm_gain: f32,
    state: &Rc<RefCell<AppState>>,
    status_bar: &mut fltk::output::Output,
    spec_display: &mut fltk::widget::Widget,
    waveform_display: &mut fltk::widget::Widget,
    input_stop: &mut fltk::input::FloatInput,
    input_recon_freq_max: &mut fltk::input::FloatInput,
    win_poll: &mut fltk::window::Window,
    enable_audio_widgets: &SharedCb,
    update_info: &SharedCb,
    update_seg_label: &SharedCb,
    tx: &mpsc::Sender<WorkerMessage>,
) {
    let num_smp = audio.num_samples();
    let duration = audio.duration_seconds;
    let nyquist = audio.nyquist_freq();
    let sample_rate = audio.sample_rate;
    let audio = Arc::new(audio);

    let params_clone;
    {
        let mut st = state.borrow_mut();

        st.audio_player.stop();
        st.fft_params.sample_rate = sample_rate;
        st.fft_params.start_sample = 0;
        st.fft_params.stop_sample = num_smp;
        st.audio_data = Some(audio.clone());
        st.has_audio = true;
        st.source_norm_gain = norm_gain;

        st.view.data_time_min_sec = 0.0;
        st.view.data_time_max_sec = duration;
        st.view.time_min_sec = 0.0;
        st.view.time_max_sec = duration;
        st.view.data_freq_max_hz = nyquist;
        st.view.freq_min_hz = st.view.freq_min_hz.min(nyquist);
        st.view.freq_max_hz = st.view.freq_max_hz.min(nyquist);
        st.view.recon_freq_max_hz = st.view.recon_freq_max_hz.min(nyquist);
        st.view.max_freq_bins = st.fft_params.num_frequency_bins();
        st.view.recon_freq_count = st.fft_params.num_frequency_bins();

        st.transport.duration_samples = num_smp;
        st.transport.sample_rate = sample_rate;
        st.transport.position_samples = 0;

        st.spec_renderer.invalidate();
        st.wave_renderer.invalidate();

        params_clone = st.fft_params.clone();
        // is_processing stays true — FFT thread follows

        let fname = filename
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        st.current_filename = fname.clone();
        drop(st);
        win_poll.set_label(&format!("muSickBeets - {}", fname));
    }

    // Sync UI widgets
    {
        let st = state.borrow();
        match st.fft_params.time_unit {
            crate::data::TimeUnit::Seconds => {
                input_stop.set_value(&format!("{:.5}", duration));
            }
            crate::data::TimeUnit::Samples => {
                input_stop.set_value(&num_smp.to_string());
            }
        }
        input_recon_freq_max.set_value(&format!("{:.0}", st.view.recon_freq_max_hz));
    }

    (enable_audio_widgets.borrow_mut())();
    (update_info.borrow_mut())();
    (update_seg_label.borrow_mut())();

    // Launch background FFT (reconstruction auto-follows via FftComplete)
    app_log!(
        "Open",
        "Spawning FFT thread (window={}, overlap={}%)",
        params_clone.window_length,
        params_clone.overlap_percent
    );
    {
        let mut st = state.borrow_mut();
        st.current_activity = "Processing FFT...";
        st.fft_start_time = Some(std::time::Instant::now());
        // Clear previous timings for a fresh load
        st.last_fft_duration = None;
        st.last_recon_duration = None;
    }
    let cancel = state.borrow_mut().new_cancel_flag();
    let tx_clone = tx.clone();
    let audio_for_fft = audio.clone();
    std::thread::spawn(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            app_log!("FFT thread", "Started");
            let spectrogram = FftEngine::process(&audio_for_fft, &params_clone, &cancel);
            app_log!(
                "FFT thread",
                "Complete: {} frames",
                spectrogram.num_frames()
            );
            spectrogram
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
            Err(panic_val) => {
                let msg: String = panic_val
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| panic_val.downcast_ref::<&str>().map(|s| s.to_string()))
                    .unwrap_or_else(|| "unknown panic".to_string());
                app_log!("FFT thread", "PANIC: {}", msg);
                tx_clone.send(WorkerMessage::WorkerPanic(msg)).ok();
            }
        }
    });

    if let Some(text) = state.try_borrow().map(|st| st.status_bar_text()).ok() {
        update_status_bar(status_bar, &text);
    }
    spec_display.redraw();
    waveform_display.redraw();
}

// ═══════════════════════════════════════════════════════════════════════════
//  TRANSPORT UPDATE
// ═══════════════════════════════════════════════════════════════════════════

fn update_transport(
    state: &Rc<RefCell<AppState>>,
    scrub_slider: &mut fltk::valuator::HorSlider,
    lbl_time: &mut fltk::frame::Frame,
    spec_display: &mut fltk::widget::Widget,
    waveform_display: &mut fltk::widget::Widget,
) {
    let transport_data = {
        let Ok(mut st) = state.try_borrow_mut() else {
            return;
        };
        if st.audio_player.has_audio() {
            let local_samples = st.audio_player.get_position_samples();
            let playing = st.audio_player.get_state() == PlaybackState::Playing;
            let global_samples = st.recon_start_sample + local_samples;
            st.transport.position_samples = global_samples;
            let dur_samples = st.transport.duration_samples;
            let sr = st.transport.sample_rate;
            let time_unit = st.fft_params.time_unit;
            Some((
                local_samples,
                dur_samples,
                global_samples,
                sr,
                playing,
                time_unit,
            ))
        } else {
            None
        }
    };
    if let Some((local_smp, dur_smp, global_smp, sr, playing, time_unit)) = transport_data {
        if dur_smp > 0 {
            scrub_slider.set_value(local_smp as f64 / dur_smp as f64);
        }
        let label = match time_unit {
            TimeUnit::Samples => {
                format!("L {} / {}\nG {}", local_smp, dur_smp, global_smp)
            }
            TimeUnit::Seconds => {
                let sr_f = sr.max(1) as f64;
                format!(
                    "L {} / {}\nG {}",
                    format_time(local_smp as f64 / sr_f),
                    format_time(dur_smp as f64 / sr_f),
                    format_time(global_smp as f64 / sr_f),
                )
            }
        };
        lbl_time.set_label(&label);
        if playing {
            spec_display.redraw();
            waveform_display.redraw();
        }
    }
}
