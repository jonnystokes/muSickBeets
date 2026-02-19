mod data;
mod processing;
mod rendering;
mod playback;
mod ui;
mod csv_export;
mod app_state;
mod validation;
mod layout;
mod callbacks_file;
mod callbacks_ui;
mod callbacks_draw;
mod callbacks_nav;
mod settings;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::{mpsc, Arc};

use fltk::{app, prelude::*};

use app_state::{AppState, SharedCb, SharedCallbacks, WorkerMessage, format_time};
use data::TimeUnit;
use layout::Widgets;
use playback::audio_player::PlaybackState;
use processing::reconstructor::Reconstructor;

// ═══════════════════════════════════════════════════════════════════════════
//  CREATE SHARED CALLBACKS
// ═══════════════════════════════════════════════════════════════════════════

fn create_shared_callbacks(widgets: &Widgets, state: &Rc<RefCell<AppState>>) -> SharedCallbacks {
    // Track whether the user has manually edited the freq count field.
    // If not, it always syncs to max bins. If yes, it only clamps down.
    let freq_count_user_adjusted = Rc::new(Cell::new(false));

    // Set callback on the freq count input to detect manual edits
    {
        let flag = freq_count_user_adjusted.clone();
        let mut input_freq_count = widgets.input_freq_count.clone();
        input_freq_count.set_callback(move |_| {
            flag.set(true);
        });
    }

    let update_info: SharedCb = {
        let state = state.clone();
        let mut lbl_info = widgets.lbl_info.clone();
        let mut lbl_resolution_info = widgets.lbl_resolution_info.clone();
        let mut lbl_hop_info = widgets.lbl_hop_info.clone();
        let mut input_freq_count = widgets.input_freq_count.clone();
        let flag = freq_count_user_adjusted.clone();
        Rc::new(RefCell::new(Box::new(move || {
            let st = state.borrow();
            let info = st.derived_info();
            lbl_info.set_label(&info.format_info());
            lbl_resolution_info.set_label(&info.format_resolution());

            // Update hop display
            let hop_ms = info.hop_length as f64 / info.sample_rate.max(1) as f64 * 1000.0;
            lbl_hop_info.set_label(&format!("Hop: {} smp ({:.1} ms)", info.hop_length, hop_ms));

            let current: usize = input_freq_count.value().parse().unwrap_or(info.freq_bins);
            if !flag.get() {
                // User hasn't manually adjusted: always track max
                input_freq_count.set_value(&info.freq_bins.to_string());
            } else if current > info.freq_bins {
                // User adjusted, but current exceeds new max: clamp down
                input_freq_count.set_value(&info.freq_bins.to_string());
            }
        })))
    };

    let update_seg_label: SharedCb = {
        let state = state.clone();
        let mut input_seg_size = widgets.input_seg_size.clone();
        let mut seg_preset_choice = widgets.seg_preset_choice.clone();
        Rc::new(RefCell::new(Box::new(move || {
            let st = state.borrow();
            let wl = st.fft_params.window_length;
            input_seg_size.set_value(&wl.to_string());
            // Sync preset dropdown
            let preset_idx = match wl {
                256 => 0, 512 => 1, 1024 => 2, 2048 => 3, 4096 => 4,
                8192 => 5, 16384 => 6, 32768 => 7, 65536 => 8,
                _ => 9, // Custom
            };
            seg_preset_choice.set_value(preset_idx);
        })))
    };

    let enable_audio_widgets: SharedCb = {
        let mut btn_time_unit = widgets.btn_time_unit.clone();
        let mut input_start = widgets.input_start.clone();
        let mut input_stop = widgets.input_stop.clone();
        let mut btn_seg_minus = widgets.btn_seg_minus.clone();
        let mut btn_seg_plus = widgets.btn_seg_plus.clone();
        let mut input_seg_size = widgets.input_seg_size.clone();
        let mut seg_preset_choice = widgets.seg_preset_choice.clone();
        let mut slider_overlap = widgets.slider_overlap.clone();
        let mut window_type_choice = widgets.window_type_choice.clone();
        let mut check_center = widgets.check_center.clone();
        let mut zero_pad_choice = widgets.zero_pad_choice.clone();
        let mut btn_rerun = widgets.btn_rerun.clone();
        Rc::new(RefCell::new(Box::new(move || {
            btn_time_unit.activate();
            input_start.activate();
            input_stop.activate();
            btn_seg_minus.activate();
            btn_seg_plus.activate();
            input_seg_size.activate();
            seg_preset_choice.activate();
            slider_overlap.activate();
            window_type_choice.activate();
            check_center.activate();
            zero_pad_choice.activate();
            btn_rerun.activate();
        })))
    };

    let enable_spec_widgets: SharedCb = {
        let mut btn_save_fft = widgets.btn_save_fft.clone();
        let mut input_freq_count = widgets.input_freq_count.clone();
        let mut input_recon_freq_min = widgets.input_recon_freq_min.clone();
        let mut input_recon_freq_max = widgets.input_recon_freq_max.clone();
        let mut btn_play = widgets.btn_play.clone();
        let mut btn_pause = widgets.btn_pause.clone();
        let mut btn_stop = widgets.btn_stop.clone();
        let mut scrub_slider = widgets.scrub_slider.clone();
        let mut repeat_choice = widgets.repeat_choice.clone();
        let mut btn_snap_to_view = widgets.btn_snap_to_view.clone();
        Rc::new(RefCell::new(Box::new(move || {
            btn_save_fft.activate();
            input_freq_count.activate();
            input_recon_freq_min.activate();
            input_recon_freq_max.activate();
            btn_play.activate();
            btn_pause.activate();
            btn_stop.activate();
            scrub_slider.activate();
            repeat_choice.activate();
            btn_snap_to_view.activate();
        })))
    };

    let enable_wav_export: SharedCb = {
        let mut btn_save_wav = widgets.btn_save_wav.clone();
        Rc::new(RefCell::new(Box::new(move || {
            btn_save_wav.activate();
        })))
    };

    SharedCallbacks {
        update_info,
        update_seg_label,
        enable_audio_widgets,
        enable_spec_widgets,
        enable_wav_export,
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  MAIN
// ═══════════════════════════════════════════════════════════════════════════

fn main() {
    // Load settings from INI (or create default INI if missing)
    let cfg = settings::Settings::load_or_create();
    eprintln!("[Settings] Loaded: recon_freq_max={}Hz, view_freq={}-{}Hz, window={}x{}",
        cfg.recon_freq_max_hz, cfg.view_freq_min_hz, cfg.view_freq_max_hz,
        cfg.window_width, cfg.window_height);

    let app = app::App::default();

    // Apply dark theme
    ui::theme::apply_dark_theme();
    app::set_visual(fltk::enums::Mode::Rgb8).ok();

    let (mut win, widgets) = layout::build_ui();

    // Apply window size from settings
    win.set_size(cfg.window_width, cfg.window_height);

    // Apply settings to state
    let state = {
        let mut st = AppState::new();
        st.fft_params.window_length = cfg.window_length;
        st.fft_params.overlap_percent = cfg.overlap_percent;
        st.fft_params.use_center = cfg.center_pad;
        st.view.freq_min_hz = cfg.view_freq_min_hz;
        st.view.freq_max_hz = cfg.view_freq_max_hz;
        st.view.freq_scale = data::FreqScale::Power(cfg.freq_scale_power);
        st.view.threshold_db = cfg.threshold_db;
        st.view.brightness = cfg.brightness;
        st.view.gamma = cfg.gamma;
        st.view.colormap = data::ColormapId::from_index(cfg.colormap_index());
        st.view.custom_gradient = cfg.parse_custom_gradient();
        st.view.recon_freq_min_hz = cfg.recon_freq_min_hz;
        st.view.recon_freq_max_hz = cfg.recon_freq_max_hz;
        st.view.recon_freq_count = cfg.recon_freq_count;
        st.lock_to_active = cfg.lock_to_active;
        st.time_zoom_factor = cfg.time_zoom_factor;
        st.freq_zoom_factor = cfg.freq_zoom_factor;
        st.mouse_zoom_factor = cfg.mouse_zoom_factor;
        st.normalize_audio = cfg.normalize_audio;
        st.normalize_peak = cfg.normalize_peak;
        st.view.db_ceiling = cfg.db_ceiling;
        st.fft_params.zero_pad_factor = cfg.zero_pad_factor;
        Rc::new(RefCell::new(st))
    };
    let (tx, rx) = mpsc::channel::<WorkerMessage>();

    // Create shared callbacks
    let shared = create_shared_callbacks(&widgets, &state);

    // Wire up all callbacks
    callbacks_nav::setup_menu_callbacks(&widgets, &state);
    callbacks_file::setup_file_callbacks(&widgets, &state, &tx, &shared, &win);
    callbacks_file::setup_rerun_callback(&widgets, &state, &tx, &shared);
    callbacks_ui::setup_parameter_callbacks(&widgets, &state, &shared);
    callbacks_ui::setup_display_callbacks(&widgets, &state);
    callbacks_ui::setup_gradient_editor(&widgets, &state);
    callbacks_ui::setup_playback_callbacks(&widgets, &state);
    callbacks_ui::setup_misc_callbacks(&widgets, &state, &win);
    callbacks_draw::setup_draw_callbacks(&widgets, &state);
    let (x_scroll_gen, y_scroll_gen) = callbacks_nav::setup_scrollbar_callbacks(&widgets, &state);
    callbacks_nav::setup_zoom_callbacks(&widgets, &state);
    callbacks_nav::setup_snap_to_view(&widgets, &state);
    callbacks_nav::setup_spacebar_handler(&mut win, &widgets);

    // ═══════════════════════════════════════════════════════════════════════════
    //  MAIN POLL LOOP (16ms)
    // ═══════════════════════════════════════════════════════════════════════════

    {
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
        let update_info = shared.update_info.clone();
        let mut x_scroll = widgets.x_scroll.clone();
        let mut y_scroll = widgets.y_scroll.clone();
        let tx = tx.clone();
        let x_scroll_gen = x_scroll_gen.clone();
        let y_scroll_gen = y_scroll_gen.clone();

        // Track last-seen generation to detect user scrollbar interaction
        let mut last_x_gen: u64 = 0;
        let mut last_y_gen: u64 = 0;

        app::add_timeout3(0.016, move |handle| {
            // ── Sync scrollbars with view state ──
            let cur_x_gen = x_scroll_gen.get();
            let cur_y_gen = y_scroll_gen.get();
            let x_user_active = cur_x_gen != last_x_gen;
            let y_user_active = cur_y_gen != last_y_gen;
            last_x_gen = cur_x_gen;
            last_y_gen = cur_y_gen;

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
                    } else { 0.0 };
                    Some((ratio, frac * 10000.0))
                } else { None };

                let y_data = if data_freq_range > 1.0 {
                    let vis_freq = st.view.visible_freq_range();
                    let ratio = (vis_freq / data_freq_range).clamp(0.02, 1.0);
                    let scroll_range = (data_freq_range - vis_freq).max(0.0);
                    let frac = if scroll_range > 0.1 {
                        ((st.view.freq_min_hz - data_freq_min) / scroll_range).clamp(0.0, 1.0) as f64
                    } else { 0.0 };
                    Some((ratio, (1.0 - frac) * 10000.0))
                } else { None };

                Some((x_data, y_data))
            } else { None };

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

            // ── Process worker messages ──
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    WorkerMessage::FftComplete(spectrogram) => {
                        let num_frames = spectrogram.num_frames();

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

                            st.spec_renderer.invalidate();

                            // Prepare reconstruction: filter spectrogram to processing time range
                            let spec = st.spectrogram.clone().unwrap();
                            let params = st.fft_params.clone();
                            let view = st.view.clone();

                            // Get processing time range
                            let proc_time_min = match params.time_unit {
                                TimeUnit::Seconds => params.start_time,
                                TimeUnit::Samples => params.start_time / params.sample_rate.max(1) as f64,
                            };
                            let proc_time_max = match params.time_unit {
                                TimeUnit::Seconds => params.stop_time,
                                TimeUnit::Samples => params.stop_time / params.sample_rate.max(1) as f64,
                            };

                            st.recon_start_time = proc_time_min;

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
                        status_bar.set_label(&format!(
                            "FFT done ({} frames) | Reconstructing...",
                            num_frames
                        ));
                        spec_display.redraw();
                        freq_axis.redraw();
                        time_axis.redraw();

                        // Auto-trigger reconstruction with time-filtered spectrogram
                        let tx_clone = tx.clone();
                        let (spec, params, view, proc_time_min, proc_time_max) = recon_data;

                        // Pre-filter frames on main thread so we can set recon_start_time
                        // precisely from the actual first frame (not the user-typed start)
                        let filtered_frames: Vec<_> = spec.frames.iter()
                            .filter(|f| f.time_seconds >= proc_time_min && f.time_seconds <= proc_time_max)
                            .cloned()
                            .collect();

                        // Set recon_start_time from actual first frame time
                        if let Some(first) = filtered_frames.first() {
                            state.borrow_mut().recon_start_time = first.time_seconds;
                        }

                        std::thread::spawn(move || {
                            let filtered_spec = data::Spectrogram::from_frames(filtered_frames);
                            let reconstructed = Reconstructor::reconstruct(&filtered_spec, &params, &view);
                            tx_clone.send(WorkerMessage::ReconstructionComplete(reconstructed)).ok();
                        });
                    }
                    WorkerMessage::ReconstructionComplete(mut reconstructed) => {
                        // Normalize reconstructed audio for proper playback volume
                        {
                            let st = state.borrow();
                            if st.normalize_audio {
                                reconstructed.normalize(st.normalize_peak);
                            }
                        }
                        let recon_result = {
                            let mut st = state.borrow_mut();
                            match st.audio_player.load_audio(&reconstructed) {
                                Ok(_) => {
                                    let duration = reconstructed.duration_seconds;
                                    let samples = reconstructed.num_samples();
                                    st.transport.duration_seconds = duration;
                                    st.wave_renderer.invalidate();

                                    st.reconstructed_audio = Some(reconstructed);
                                    st.is_processing = false;
                                    st.dirty = false;

                                    // If "Lock to Active" is on, snap viewport to processing range
                                    if st.lock_to_active {
                                        let proc_min = st.recon_start_time;
                                        let proc_max = st.recon_start_time + duration;
                                        if proc_max > proc_min {
                                            st.view.time_min_sec = proc_min.max(st.view.data_time_min_sec);
                                            st.view.time_max_sec = proc_max.min(st.view.data_time_max_sec);
                                            st.spec_renderer.invalidate();
                                        }
                                    }

                                    Ok((duration, samples))
                                }
                                Err(e) => {
                                    st.is_processing = false;
                                    Err(e)
                                }
                            }
                        };
                        match recon_result {
                            Ok((duration, samples)) => {
                                (enable_wav_export.borrow_mut())();
                                status_bar.set_label(&format!(
                                    "Reconstructed | {:.2}s | {} samples",
                                    duration, samples
                                ));
                                spec_display.redraw();
                                waveform_display.redraw();
                                freq_axis.redraw();
                                time_axis.redraw();
                            }
                            Err(e) => {
                                status_bar.set_label(&format!("Reconstruction error: {}", e));
                                fltk::dialog::alert_default(&format!("Failed to load reconstructed audio:\n{}", e));
                            }
                        }
                    }
                }
            }

            // ── Update transport position ──
            let transport_data = {
                let Ok(mut st) = state.try_borrow_mut() else {
                    app::repeat_timeout3(0.016, handle);
                    return;
                };
                if st.audio_player.has_audio() {
                    let audio_pos = st.audio_player.get_position_seconds();
                    let playing = st.audio_player.get_state() == PlaybackState::Playing;
                    let global_pos = st.recon_start_time + audio_pos;
                    st.transport.position_seconds = global_pos;
                    Some((audio_pos, st.transport.duration_seconds, global_pos, playing))
                } else {
                    None
                }
            };
            if let Some((audio_pos, dur, global_pos, playing)) = transport_data {
                if dur > 0.0 {
                    scrub_slider.set_value((audio_pos / dur).clamp(0.0, 1.0));
                }
                lbl_time.set_label(&format!(
                    "L {} / {}\nG {}",
                    format_time(audio_pos),
                    format_time(dur),
                    format_time(global_pos),
                ));
                if playing {
                    spec_display.redraw();
                    waveform_display.redraw();
                }
            }

            app::repeat_timeout3(0.016, handle);
        });
    }

    win.show();
    app.run().unwrap();
}
