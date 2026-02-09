

mod audio_loader;
mod audio_player;
mod csv_export;
mod fft_engine;
mod fft_params;
mod spectrogram;
mod audio_reconstructor;
mod color_lut;
mod spectrogram_renderer;

use fltk::{
    app,
    button::Button,
    enums::{Align, Color, FrameType},
    frame::Frame,
    group::Flex,
    input::Input,
    menu::Choice,
    misc::Progress,
    prelude::*,
    valuator::HorNiceSlider,
    widget::Widget,
    window::Window,
    dialog,
};

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use audio_loader::AudioData;
use audio_player::AudioPlayer;
use csv_export::{export_to_csv, import_from_csv};
use fft_engine::FftEngine;
use fft_params::{FftParams, TimeUnit, WindowType};
use spectrogram::Spectrogram;
use audio_reconstructor::{AudioReconstructor, ReconstructionQuality};
use spectrogram_renderer::SpectrogramRenderer;

/// Message types for background processing
enum ProcessingMessage {
    FftComplete(Spectrogram),
    ReconstructionComplete(AudioData),
    Error(String),
}

struct AppState {
    audio_data: Option<AudioData>,
    fft_params: FftParams,
    spectrogram: Option<Spectrogram>,
    audio_player: AudioPlayer,
    reconstruction_quality: ReconstructionQuality,
    reconstructed_audio: Option<AudioData>,
    renderer: SpectrogramRenderer,
    is_processing: bool,
}

impl AppState {
    fn new() -> Self {
        Self {
            audio_data: None,
            fft_params: FftParams::default(),
            spectrogram: None,
            audio_player: AudioPlayer::new(),
            reconstruction_quality: ReconstructionQuality::High,
            reconstructed_audio: None,
            renderer: SpectrogramRenderer::new(),
            is_processing: false,
        }
    }
}

/// Throttle helper to prevent excessive updates
struct UpdateThrottle {
    last_update: Instant,
    min_interval: Duration,
}

impl UpdateThrottle {
    fn new(min_interval_ms: u64) -> Self {
        Self {
            last_update: Instant::now() - Duration::from_millis(min_interval_ms + 1),
            min_interval: Duration::from_millis(min_interval_ms),
        }
    }

    fn should_update(&mut self) -> bool {
        let now = Instant::now();
        if now.duration_since(self.last_update) >= self.min_interval {
            self.last_update = now;
            true
        } else {
            false
        }
    }

    fn force_update(&mut self) {
        self.last_update = Instant::now();
    }
}

fn main() {
    let app = app::App::default();
    
    // Enable double buffering for the app
    app::set_visual(fltk::enums::Mode::Rgb8).ok();
    
    let mut win = Window::new(100, 100, 1200, 1000, "FFT Audio Analyzer");
    win.make_resizable(true);

    let state = Rc::new(RefCell::new(AppState::new()));
    
    // Channel for background processing results
    let (tx, rx) = mpsc::channel::<ProcessingMessage>();

    // Root layout: left controls, right display
    let mut root = Flex::default_fill().row();

    // ---------------- Left Panel (Controls) ----------------
    let mut left = Flex::default().column();
    root.fixed(&left, 300);

    let mut title = Frame::default().with_label("FFT Audio Analyzer");
    title.set_label_size(16);
    left.fixed(&title, 30);

    // File operations
    let mut btn_open = Button::default().with_label("Open Audio File");
    left.fixed(&btn_open, 30);

    let mut btn_save_fft = Button::default().with_label("Save FFT Data");
    left.fixed(&btn_save_fft, 30);

    let mut btn_load_fft = Button::default().with_label("Load FFT Data");
    left.fixed(&btn_load_fft, 30);

    let mut btn_rerun = Button::default().with_label("Rerun Analysis");
    left.fixed(&btn_rerun, 30);

    let sep1 = Frame::default().with_label("─────────────────");
    left.fixed(&sep1, 20);

    // Time range controls
    let mut lbl_time_range = Frame::default().with_label("Time Range:");
    lbl_time_range.set_align(Align::Inside | Align::Left);
    left.fixed(&lbl_time_range, 20);

    let mut time_unit_choice = Choice::default();
    time_unit_choice.add_choice("Seconds");
    time_unit_choice.add_choice("Samples");
    time_unit_choice.set_value(0);
    left.fixed(&time_unit_choice, 25);

    let mut input_start = Input::default().with_label("Start:");
    input_start.set_value("0.00000");
    left.fixed(&input_start, 30);

    let mut input_stop = Input::default().with_label("Stop:");
    input_stop.set_value("0.00000");
    left.fixed(&input_stop, 30);

    let sep2 = Frame::default().with_label("─────────────────");
    left.fixed(&sep2, 20);

    // FFT parameters
    let mut lbl_fft_params = Frame::default().with_label("FFT Parameters:");
    lbl_fft_params.set_align(Align::Inside | Align::Left);
    left.fixed(&lbl_fft_params, 20);

    let mut lbl_window_len = Frame::default().with_label("Window Length (samples):");
    lbl_window_len.set_align(Align::Inside | Align::Left);
    left.fixed(&lbl_window_len, 20);

    let mut input_window_len = Input::default();
    input_window_len.set_value("2048");
    left.fixed(&input_window_len, 30);

    let mut slider_overlap = HorNiceSlider::default().with_label("Overlap %:");
    slider_overlap.set_minimum(0.0);
    slider_overlap.set_maximum(95.0);
    slider_overlap.set_value(75.0);
    left.fixed(&slider_overlap, 30);

    let mut lbl_overlap_val = Frame::default().with_label("75%");
    lbl_overlap_val.set_align(Align::Inside | Align::Right);
    left.fixed(&lbl_overlap_val, 20);

    let mut window_type_choice = Choice::default().with_label("Window:");
    window_type_choice.add_choice("Hann");
    window_type_choice.add_choice("Hamming");
    window_type_choice.add_choice("Blackman");
    window_type_choice.add_choice("Kaiser");
    window_type_choice.set_value(0);
    left.fixed(&window_type_choice, 30);

    let mut input_kaiser_beta = Input::default().with_label("Kaiser β:");
    input_kaiser_beta.set_value("8.6");
    input_kaiser_beta.deactivate();
    left.fixed(&input_kaiser_beta, 30);

    let mut check_center = fltk::button::CheckButton::default().with_label(" Center/Pad");
    check_center.set_checked(true);
    left.fixed(&check_center, 25);

    let sep3 = Frame::default().with_label("─────────────────");
    left.fixed(&sep3, 20);

    // Visualization controls
    let mut lbl_viz = Frame::default().with_label("Visualization:");
    lbl_viz.set_align(Align::Inside | Align::Left);
    left.fixed(&lbl_viz, 20);

    let mut slider_threshold = HorNiceSlider::default().with_label("Threshold (dB):");
    slider_threshold.set_minimum(-120.0);
    slider_threshold.set_maximum(0.0);
    slider_threshold.set_value(-80.0);
    left.fixed(&slider_threshold, 30);

    let mut lbl_threshold_val = Frame::default().with_label("-80 dB");
    lbl_threshold_val.set_align(Align::Inside | Align::Right);
    left.fixed(&lbl_threshold_val, 20);

    let mut slider_brightness = HorNiceSlider::default().with_label("Brightness:");
    slider_brightness.set_minimum(0.1);
    slider_brightness.set_maximum(3.0);
    slider_brightness.set_value(1.0);
    left.fixed(&slider_brightness, 30);

    let mut lbl_brightness_val = Frame::default().with_label("1.0");
    lbl_brightness_val.set_align(Align::Inside | Align::Right);
    left.fixed(&lbl_brightness_val, 20);

    let sep4 = Frame::default().with_label("─────────────────");
    left.fixed(&sep4, 20);

    // Reconstruction controls
    let mut lbl_reconstruction = Frame::default().with_label("Audio Reconstruction:");
    lbl_reconstruction.set_align(Align::Inside | Align::Left);
    left.fixed(&lbl_reconstruction, 20);

    let mut slider_quality = HorNiceSlider::default().with_label("Quality %:");
    slider_quality.set_minimum(0.0);
    slider_quality.set_maximum(100.0);
    slider_quality.set_value(70.0); // Default to "High"
    left.fixed(&slider_quality, 30);

    let mut lbl_quality_val = Frame::default().with_label("70% (High)");
    lbl_quality_val.set_align(Align::Inside | Align::Right);
    left.fixed(&lbl_quality_val, 20);

    // Quality preset buttons
    let preset_row = Flex::default().row();
    left.fixed(&preset_row, 30);
    
    let mut btn_fast = Button::default().with_label("Fast");
    let mut btn_balanced = Button::default().with_label("Balanced");
    let mut btn_perfect = Button::default().with_label("Perfect");
    
    preset_row.end();

    let mut btn_reconstruct = Button::default().with_label("Reconstruct Audio");
    left.fixed(&btn_reconstruct, 30);

    let mut lbl_est_time = Frame::default().with_label("Est. time: --");
    lbl_est_time.set_align(Align::Inside | Align::Left);
    left.fixed(&lbl_est_time, 20);

    let mut status = Frame::default().with_label("Status: Ready");
    status.set_align(Align::Inside | Align::Left);
    left.fixed(&status, 25);

    left.end();

    // ---------------- Right Panel (Display) ----------------
    let mut right = Flex::default().column();

    let mut spec_label = Frame::default().with_label("Spectrogram Display");
    spec_label.set_align(Align::Inside | Align::Left);
    right.fixed(&spec_label, 25);

    let mut spec_display = Widget::default();
    spec_display.set_frame(FrameType::DownBox);
    spec_display.set_color(Color::Black);

    // Playback controls
    let mut playback_row = Flex::default().row();
    right.fixed(&playback_row, 35);

    let mut btn_play = Button::default().with_label("▶ Play");
    playback_row.fixed(&btn_play, 80);

    let mut btn_pause = Button::default().with_label("⏸ Pause");
    playback_row.fixed(&btn_pause, 80);

    let mut btn_stop = Button::default().with_label("⏹ Stop");
    playback_row.fixed(&btn_stop, 80);

    let mut repeat_choice = Choice::default();
    repeat_choice.add_choice("Single");
    repeat_choice.add_choice("Repeat");
    repeat_choice.set_value(0);
    playback_row.fixed(&repeat_choice, 80);

    Frame::default(); // Spacer

    playback_row.end();

    // Progress bar
    let mut progress = Progress::default();
    progress.set_minimum(0.0);
    progress.set_maximum(100.0);
    progress.set_value(0.0);
    right.fixed(&progress, 20);

    right.end();
    root.end();

    // Store last widget size for resize detection
    let last_spec_size: Rc<RefCell<(i32, i32)>> = Rc::new(RefCell::new((0, 0)));

    // ---------------- Callbacks ----------------

    // Open audio file
    {
        let state = state.clone();
        let mut status = status.clone();
        let mut input_stop = input_stop.clone();
        let mut spec_display = spec_display.clone();
        let tx = tx.clone();
        let mut progress = progress.clone();

        btn_open.set_callback(move |_| {
            let mut chooser = dialog::NativeFileChooser::new(dialog::NativeFileChooserType::BrowseFile);
            chooser.set_filter("*.wav");
            chooser.show();

            let filename = chooser.filename();
            if filename.as_os_str().is_empty() {
                return;
            }

            status.set_label("Status: Loading audio...");
            app::awake();

            match AudioData::from_wav_file(&filename) {
                Ok(audio) => {
                    let duration = audio.duration_seconds;
                    let params_clone;
                    
                    {
                        let mut st = state.borrow_mut();
                        st.fft_params.sample_rate = audio.sample_rate;
                        st.fft_params.stop_time = duration;
                        st.audio_data = Some(audio);
                        st.renderer.invalidate();
                        params_clone = st.fft_params.clone();
                    }
                    
                    input_stop.set_value(&format!("{:.5}", duration));
                    status.set_label(&format!("Status: Loaded {:.2}s audio", duration));
                    
                    // Start background FFT processing
                    {
                        let mut st = state.borrow_mut();
                        st.is_processing = true;
                    }
                    
                    let audio_clone = state.borrow().audio_data.clone().unwrap();
                    let tx_clone = tx.clone();
                    
                    std::thread::spawn(move || {
                        let engine = FftEngine::new(params_clone);
                        let frames = engine.process_audio(&audio_clone);
                        let spectrogram = Spectrogram::from_frames(frames);
                        tx_clone.send(ProcessingMessage::FftComplete(spectrogram)).ok();
                    });
                    
                    progress.set_value(50.0);
                    status.set_label("Status: Processing FFT...");
                }
                Err(e) => {
                    dialog::alert_default(&format!("Error loading audio:\n{}", e));
                    status.set_label("Status: Load failed");
                }
            }
            
            spec_display.redraw();
        });
    }

    // Save FFT data to CSV
    {
        let state = state.clone();
        let mut status = status.clone();

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

            match export_to_csv(st.spectrogram.as_ref().unwrap(), &st.fft_params, &filename) {
                Ok(_) => {
                    status.set_label("Status: FFT data saved");
                    dialog::message_default(&format!("Saved to {:?}", filename));
                }
                Err(e) => {
                    dialog::alert_default(&format!("Error saving CSV:\n{}", e));
                    status.set_label("Status: Save failed");
                }
            }
        });
    }

    // Load FFT data from CSV
    {
        let state = state.clone();
        let mut status = status.clone();
        let mut spec_display = spec_display.clone();
        let mut input_start = input_start.clone();
        let mut input_stop = input_stop.clone();
        let mut input_window_len = input_window_len.clone();
        let mut slider_overlap = slider_overlap.clone();

        btn_load_fft.set_callback(move |_| {
            let mut chooser = dialog::NativeFileChooser::new(dialog::NativeFileChooserType::BrowseFile);
            chooser.set_filter("*.csv");
            chooser.show();

            let filename = chooser.filename();
            if filename.as_os_str().is_empty() {
                return;
            }

            status.set_label("Status: Loading CSV...");
            app::awake();

            match import_from_csv(&filename) {
                Ok((spectrogram, params)) => {
                    {
                        let mut st = state.borrow_mut();
                        st.spectrogram = Some(spectrogram);
                        st.fft_params = params.clone();
                        st.renderer.invalidate();
                    }

                    input_start.set_value(&format!("{:.5}", params.start_time));
                    input_stop.set_value(&format!("{:.5}", params.stop_time));
                    input_window_len.set_value(&params.window_length.to_string());
                    slider_overlap.set_value(params.overlap_percent as f64);

                    let num_frames = state.borrow().spectrogram.as_ref().unwrap().num_frames();
                    status.set_label(&format!("Status: Loaded {} frames from CSV", num_frames));
                    spec_display.redraw();
                }
                Err(e) => {
                    dialog::alert_default(&format!("Error loading CSV:\n{}", e));
                    status.set_label("Status: CSV load failed");
                }
            }
        });
    }

    // Rerun analysis button
    {
        let state = state.clone();
        let mut status = status.clone();
        let mut spec_display = spec_display.clone();
        let tx = tx.clone();
        let mut progress = progress.clone();

        btn_rerun.set_callback(move |_| {
            let (audio_clone, params) = {
                let st = state.borrow();
                if st.audio_data.is_none() {
                    dialog::alert_default("No audio loaded!");
                    return;
                }
                if st.is_processing {
                    dialog::alert_default("Already processing!");
                    return;
                }
                (st.audio_data.clone().unwrap(), st.fft_params.clone())
            };

            {
                let mut st = state.borrow_mut();
                st.is_processing = true;
                st.renderer.invalidate();
            }

            status.set_label("Status: Processing FFT...");
            progress.set_value(25.0);
            app::awake();

            let tx_clone = tx.clone();
            std::thread::spawn(move || {
                let engine = FftEngine::new(params);
                let frames = engine.process_audio(&audio_clone);
                let spectrogram = Spectrogram::from_frames(frames);
                tx_clone.send(ProcessingMessage::FftComplete(spectrogram)).ok();
            });
            
            spec_display.redraw();
        });
    }

    // Time unit choice
    {
        let state = state.clone();

        time_unit_choice.set_callback(move |c| {
            let mut st = state.borrow_mut();
            st.fft_params.time_unit = if c.value() == 0 {
                TimeUnit::Seconds
            } else {
                TimeUnit::Samples
            };
        });
    }

    // Start time input
    {
        let state = state.clone();

        input_start.set_callback(move |i| {
            if let Ok(val) = i.value().parse::<f64>() {
                let mut st = state.borrow_mut();
                st.fft_params.start_time = val;
            }
        });
    }

    // Stop time input
    {
        let state = state.clone();

        input_stop.set_callback(move |i| {
            if let Ok(val) = i.value().parse::<f64>() {
                let mut st = state.borrow_mut();
                st.fft_params.stop_time = val;
            }
        });
    }

    // Window length input
    {
        let state = state.clone();

        input_window_len.set_callback(move |i| {
            if let Ok(val) = i.value().parse::<usize>() {
                let mut st = state.borrow_mut();
                // Ensure power of 2
                let pow2 = val.next_power_of_two();
                st.fft_params.window_length = pow2;
                if pow2 != val {
                    i.set_value(&pow2.to_string());
                }
            }
        });
    }

    // Overlap slider
    {
        let mut lbl = lbl_overlap_val.clone();
        let state = state.clone();

        slider_overlap.set_callback(move |s| {
            let val = s.value() as f32;
            lbl.set_label(&format!("{}%", val as i32));
            let mut st = state.borrow_mut();
            st.fft_params.overlap_percent = val;
        });
    }

    // Window type choice
    {
        let state = state.clone();
        let mut input_kaiser_beta = input_kaiser_beta.clone();

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
                    let beta: f32 = input_kaiser_beta.value().parse().unwrap_or(8.6);
                    WindowType::Kaiser(beta)
                }
                _ => WindowType::Hann,
            };
        });
    }

    // Kaiser beta input
    {
        let state = state.clone();

        input_kaiser_beta.set_callback(move |i| {
            if let Ok(beta) = i.value().parse::<f32>() {
                let mut st = state.borrow_mut();
                if matches!(st.fft_params.window_type, WindowType::Kaiser(_)) {
                    st.fft_params.window_type = WindowType::Kaiser(beta);
                }
            }
        });
    }

    // Center/Pad checkbox
    {
        let state = state.clone();

        check_center.set_callback(move |c| {
            let mut st = state.borrow_mut();
            st.fft_params.use_center = c.is_checked();
        });
    }

    // Threshold slider - with throttling
    {
        let mut lbl = lbl_threshold_val.clone();
        let state = state.clone();
        let mut spec_display = spec_display.clone();
        let throttle = Rc::new(RefCell::new(UpdateThrottle::new(50))); // 50ms throttle

        slider_threshold.set_callback(move |s| {
            let val = s.value() as f32;
            lbl.set_label(&format!("{} dB", val as i32));
            
            // Always update state
            {
                let mut st = state.borrow_mut();
                let brightness = st.renderer.brightness();
                st.renderer.set_params(val, brightness);
            }
            
            // Throttle redraws
            if throttle.borrow_mut().should_update() {
                spec_display.redraw();
            }
        });
    }

    // Brightness slider - with throttling
    {
        let mut lbl = lbl_brightness_val.clone();
        let state = state.clone();
        let mut spec_display = spec_display.clone();
        let throttle = Rc::new(RefCell::new(UpdateThrottle::new(50))); // 50ms throttle

        slider_brightness.set_callback(move |s| {
            let val = s.value() as f32;
            lbl.set_label(&format!("{:.1}", val));
            
            // Always update state
            {
                let mut st = state.borrow_mut();
                let threshold = st.renderer.threshold_db();
                st.renderer.set_params(threshold, val);
            }
            
            // Throttle redraws
            if throttle.borrow_mut().should_update() {
                spec_display.redraw();
            }
        });
    }

    // Quality slider
    {
        let mut lbl = lbl_quality_val.clone();
        let state = state.clone();
        
        slider_quality.set_callback(move |s| {
            let quality = ReconstructionQuality::from_percent(s.value() as f32);
            let label = match quality {
                ReconstructionQuality::Fast => "0% (Fast)",
                ReconstructionQuality::Balanced => "40% (Balanced)",
                ReconstructionQuality::High => "70% (High)",
                ReconstructionQuality::Perfect => "100% (Perfect)",
            };
            lbl.set_label(label);
            
            let mut st = state.borrow_mut();
            st.reconstruction_quality = quality;
        });
    }

    // Quality preset buttons
    {
        let mut slider_quality = slider_quality.clone();
        btn_fast.set_callback(move |_| {
            slider_quality.set_value(0.0);
            slider_quality.do_callback();
        });
    }
    {
        let mut slider_quality = slider_quality.clone();
        btn_balanced.set_callback(move |_| {
            slider_quality.set_value(40.0);
            slider_quality.do_callback();
        });
    }
    {
        let mut slider_quality = slider_quality.clone();
        btn_perfect.set_callback(move |_| {
            slider_quality.set_value(100.0);
            slider_quality.do_callback();
        });
    }

    // Reconstruct audio button
    {
        let state = state.clone();
        let mut status = status.clone();
        let mut progress = progress.clone();
        let mut lbl_est_time = lbl_est_time.clone();
        let tx = tx.clone();
        
        btn_reconstruct.set_callback(move |_| {
            let (spec_clone, params, quality) = {
                let st = state.borrow();
                
                if st.spectrogram.is_none() {
                    drop(st);
                    dialog::alert_default("No FFT data to reconstruct!\n\nLoad audio or import CSV first.");
                    return;
                }
                
                if st.is_processing {
                    drop(st);
                    dialog::alert_default("Already processing!");
                    return;
                }
                
                (st.spectrogram.clone().unwrap(), st.fft_params.clone(), st.reconstruction_quality)
            };
            
            {
                let mut st = state.borrow_mut();
                st.is_processing = true;
            }
            
            status.set_label("Status: Reconstructing audio...");
            app::awake();
            
            // Show estimated time
            let reconstructor = AudioReconstructor::new(params.clone(), quality);
            let est_time = reconstructor.estimate_time(spec_clone.num_frames());
            lbl_est_time.set_label(&format!("Est. time: {:.2}s", est_time));
            
            progress.set_value(0.0);
            app::awake();
            
            // Background reconstruction
            let tx_clone = tx.clone();
            std::thread::spawn(move || {
                let reconstructor = AudioReconstructor::new(params, quality);
                let reconstructed = reconstructor.reconstruct(&spec_clone);
                tx_clone.send(ProcessingMessage::ReconstructionComplete(reconstructed)).ok();
            });
        });
    }

    // Play button
    {
        let state = state.clone();
        btn_play.set_callback(move |_| {
            let mut st = state.borrow_mut();
            st.audio_player.play();
        });
    }

    // Pause button
    {
        let state = state.clone();
        btn_pause.set_callback(move |_| {
            let mut st = state.borrow_mut();
            st.audio_player.pause();
        });
    }

    // Stop button
    {
        let state = state.clone();
        btn_stop.set_callback(move |_| {
            let mut st = state.borrow_mut();
            st.audio_player.stop();
        });
    }

    // Repeat toggle
    {
        let state = state.clone();
        repeat_choice.set_callback(move |c| {
            let mut st = state.borrow_mut();
            st.audio_player.set_repeat(c.value() == 1);
        });
    }

    // Spectrogram display - optimized draw callback
    {
        let state = state.clone();
        let last_size = last_spec_size.clone();
        
        spec_display.draw(move |w| {
            // Skip if not visible (widget hidden or window minimized)
            if !w.visible_r() || w.w() <= 0 || w.h() <= 0 {
                return;
            }
            
            let mut st = state.borrow_mut();
            
            // Check for resize
            let current_size = (w.w(), w.h());
            let mut size_ref = last_size.borrow_mut();
            if *size_ref != current_size {
                st.renderer.invalidate();
                *size_ref = current_size;
            }
            
            // Draw spectrogram using cached renderer
            // Take ownership temporarily to avoid borrow conflict
            if let Some(spec) = st.spectrogram.take() {
                st.renderer.draw(&spec, w.x(), w.y(), w.w(), w.h());
                st.spectrogram = Some(spec); // Put it back
            } else {
                // Draw "no data" state
                fltk::draw::set_draw_color(Color::Black);
                fltk::draw::draw_rectf(w.x(), w.y(), w.w(), w.h());
                fltk::draw::set_draw_color(Color::White);
                fltk::draw::set_font(fltk::enums::Font::Helvetica, 14);
                fltk::draw::draw_text("Load an audio file to begin", w.x() + 10, w.y() + w.h() / 2);
            }
        });
    }

    // Poll for background processing results
    {
        let state = state.clone();
        let mut status = status.clone();
        let mut progress = progress.clone();
        let mut spec_display = spec_display.clone();
        
        app::add_timeout3(0.016, move |handle| {
            // Check for messages from background threads
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    ProcessingMessage::FftComplete(spectrogram) => {
                        let num_frames = spectrogram.num_frames();
                        
                        {
                            let mut st = state.borrow_mut();
                            
                            // Load audio for playback
                            // Take audio temporarily to avoid borrow conflict
                            if let Some(audio) = st.audio_data.take() {
                                let start = st.fft_params.start_sample();
                                let stop = st.fft_params.stop_sample();
                                if let Err(e) = st.audio_player.load_audio(&audio, start, stop) {
                                    status.set_label(&format!("Status: Audio load error - {}", e));
                                }
                                st.audio_data = Some(audio); // Put it back
                            }
                            
                            st.spectrogram = Some(spectrogram);
                            st.renderer.invalidate();
                            st.is_processing = false;
                        }
                        
                        progress.set_value(100.0);
                        status.set_label(&format!("Status: Processed {} frames", num_frames));
                        spec_display.redraw();
                    }
                    ProcessingMessage::ReconstructionComplete(reconstructed) => {
                        let start = 0;
                        let end = reconstructed.num_samples();
                        
                        let mut st = state.borrow_mut();
                        match st.audio_player.load_audio(&reconstructed, start, end) {
                            Ok(_) => {
                                let duration = reconstructed.duration_seconds;
                                let samples = reconstructed.num_samples();
                                st.reconstructed_audio = Some(reconstructed);
                                st.is_processing = false;
                                progress.set_value(100.0);
                                status.set_label(&format!("Status: Audio reconstructed! ({:.2}s, {} samples)", 
                                    duration, samples));
                                drop(st);
                                dialog::message_default("Audio reconstructed successfully!\n\nClick Play to hear it.");
                            }
                            Err(e) => {
                                st.is_processing = false;
                                status.set_label(&format!("Status: Reconstruction error - {}", e));
                                drop(st);
                                dialog::alert_default(&format!("Failed to load reconstructed audio:\n{}", e));
                            }
                        }
                    }
                    ProcessingMessage::Error(msg) => {
                        let mut st = state.borrow_mut();
                        st.is_processing = false;
                        status.set_label(&format!("Status: Error - {}", msg));
                        dialog::alert_default(&format!("Processing error:\n{}", msg));
                    }
                }
            }
            
            // Re-register the timeout
            app::repeat_timeout3(0.016, handle);
        });
    }

    win.show();
    app.run().unwrap();
}
