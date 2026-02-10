mod data;
mod processing;
mod rendering;
mod playback;
mod ui;
mod csv_export;

use fltk::{
    app,
    button::Button,
    enums::{Align, CallbackTrigger, Event, FrameType, Font, Key, Shortcut},
    frame::Frame,
    group::Flex,
    input::{Input, FloatInput},
    menu::{Choice, MenuBar, MenuFlag},
    prelude::*,
    valuator::{HorNiceSlider, HorSlider},
    widget::Widget,
    window::Window,
    dialog,
};

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use data::{AudioData, FftParams, Spectrogram, ViewState, FreqScale, ColormapId, TransportState, WindowType, TimeUnit, ActiveMask};
use processing::fft_engine::FftEngine;
use processing::reconstructor::Reconstructor;
use processing::waveform_cache::WaveformPeaks;
use rendering::spectrogram_renderer::SpectrogramRenderer;
use rendering::waveform_renderer::WaveformRenderer;
use playback::audio_player::{AudioPlayer, PlaybackState};
use ui::theme;
use ui::tooltips::{TooltipManager, set_tooltip};

// ─── Messages ──────────────────────────────────────────────────────────────────

enum WorkerMessage {
    FftComplete(Spectrogram),
    ReconstructionComplete(AudioData),
    WaveformReady(WaveformPeaks),
    Error(String),
}

// ─── App State ─────────────────────────────────────────────────────────────────

struct AppState {
    audio_data: Option<Arc<AudioData>>,
    spectrogram: Option<Arc<Spectrogram>>,
    fft_params: FftParams,
    view: ViewState,
    transport: TransportState,

    audio_player: AudioPlayer,
    spec_renderer: SpectrogramRenderer,
    wave_renderer: WaveformRenderer,
    waveform_peaks: WaveformPeaks,

    reconstructed_audio: Option<AudioData>,
    active_mask: Option<ActiveMask>,
    is_processing: bool,
    has_audio: bool,

    tooltip_mgr: TooltipManager,
}

impl AppState {
    fn new() -> Self {
        Self {
            audio_data: None,
            spectrogram: None,
            fft_params: FftParams::default(),
            view: ViewState::default(),
            transport: TransportState::default(),

            audio_player: AudioPlayer::new(),
            spec_renderer: SpectrogramRenderer::new(),
            wave_renderer: WaveformRenderer::new(),
            waveform_peaks: WaveformPeaks {
                peaks: vec![],
                time_start: 0.0,
                time_end: 0.0,
            },

            reconstructed_audio: None,
            active_mask: None,
            is_processing: false,
            has_audio: false,

            tooltip_mgr: TooltipManager::new(),
        }
    }

    /// Compute all derived info values from current params
    fn derived_info(&self) -> DerivedInfo {
        let total_samples = if let Some(ref audio) = self.audio_data {
            let start = self.fft_params.start_sample().min(audio.num_samples());
            let stop = self.fft_params.stop_sample().min(audio.num_samples());
            stop.saturating_sub(start)
        } else {
            0
        };

        let freq_bins = self.fft_params.num_frequency_bins();
        let freq_res = self.fft_params.frequency_resolution();
        let hop = self.fft_params.hop_length();
        let segments = self.fft_params.num_segments(total_samples);
        let bin_duration_ms = self.fft_params.bin_duration_seconds() * 1000.0;

        DerivedInfo {
            total_samples,
            freq_bins,
            freq_resolution: freq_res,
            hop_length: hop,
            segments,
            bin_duration_ms,
            window_length: self.fft_params.window_length,
            sample_rate: self.fft_params.sample_rate,
        }
    }
}

struct DerivedInfo {
    total_samples: usize,
    freq_bins: usize,
    freq_resolution: f32,
    hop_length: usize,
    segments: usize,
    bin_duration_ms: f64,
    window_length: usize,
    sample_rate: u32,
}

impl DerivedInfo {
    fn format_info(&self) -> String {
        format!(
            "Segments: {}\nSamples: {}\nFreq bins: {}\nFreq res: {:.2} Hz\nHop: {} smp\nBin dur: {:.2} ms\nWindow: {} smp",
            self.segments, self.total_samples, self.freq_bins,
            self.freq_resolution, self.hop_length, self.bin_duration_ms,
            self.window_length
        )
    }
}

/// Throttle helper to prevent excessive redraws
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
}

// ─── Helper: enable/disable a group of widgets ─────────────────────────────────

fn set_widgets_active(widgets: &mut [&mut dyn WidgetExt], active: bool) {
    for w in widgets.iter_mut() {
        if active {
            w.activate();
        } else {
            w.deactivate();
        }
    }
}

// ─── Format time as M:SS.ms ───────────────────────────────────────────────────

fn format_time(seconds: f64) -> String {
    let mins = (seconds / 60.0) as u32;
    let secs = seconds % 60.0;
    format!("{}:{:05.2}", mins, secs)
}

// ─── Float/Int Input Validation ──────────────────────────────────────────────
//
// Revert-based validation: let the character enter, then validate and revert
// if invalid. This works on VNC/Termux/remote desktop where keystroke blocking
// doesn't work because input arrives as Shortcut/Paste events.

fn is_valid_float_input(text: &str) -> bool {
    let digits = text.strip_prefix('-').unwrap_or(text);
    if digits.is_empty() { return true; }
    if digits.starts_with('.') { return false; }
    let parts: Vec<&str> = digits.split('.').collect();
    parts.len() <= 2 && parts.iter().all(|p| p.is_empty() || p.chars().all(|c| c.is_ascii_digit()))
}

fn is_valid_uint_input(text: &str) -> bool {
    text.is_empty() || text.chars().all(|c| c.is_ascii_digit())
}

fn attach_float_validation(input: &mut FloatInput) {
    let mut last_valid = String::new();
    input.set_trigger(CallbackTrigger::Changed);
    input.set_callback(move |field| {
        let current = field.value();
        let minus_just_added = current.contains('-') && !last_valid.contains('-');
        let typed_at_start = field.position() == 1;
        if is_valid_float_input(&current) && !(minus_just_added && !typed_at_start) {
            last_valid = current;
        } else {
            let restore = field.position().saturating_sub(1);
            field.set_value(&last_valid);
            field.set_position(restore).ok();
        }
    });
}

fn attach_uint_validation(input: &mut Input) {
    let mut last_valid = String::new();
    input.set_trigger(CallbackTrigger::Changed);
    input.set_callback(move |field| {
        let current = field.value();
        if is_valid_uint_input(&current) {
            last_valid = current;
        } else {
            let restore = field.position().saturating_sub(1);
            field.set_value(&last_valid);
            field.set_position(restore).ok();
        }
    });
}

// Helper: parse a field value as f64, treating empty as 0
fn parse_or_zero_f64(s: &str) -> f64 {
    if s.is_empty() { 0.0 } else { s.parse().unwrap_or(0.0) }
}

fn parse_or_zero_usize(s: &str) -> usize {
    if s.is_empty() { 0 } else { s.parse().unwrap_or(0) }
}

fn parse_or_zero_f32(s: &str) -> f32 {
    if s.is_empty() { 0.0 } else { s.parse().unwrap_or(0.0) }
}

// ─── MAIN ──────────────────────────────────────────────────────────────────────

fn main() {
    let app = app::App::default();

    // Apply dark theme
    theme::apply_dark_theme();
    app::set_visual(fltk::enums::Mode::Rgb8).ok();

    let mut win = Window::new(50, 50, 1400, 900, "muSickBeets FFT Analyzer");
    win.make_resizable(true);
    win.set_color(theme::color(theme::BG_DARK));

    let state = Rc::new(RefCell::new(AppState::new()));
    let (tx, rx) = mpsc::channel::<WorkerMessage>();

    // ═══════════════════════════════════════════════════════════════════════════
    //  MENU BAR
    // ═══════════════════════════════════════════════════════════════════════════

    let mut menu = MenuBar::default().with_size(1400, 25);
    menu.set_color(theme::color(theme::BG_PANEL));
    menu.set_text_color(theme::color(theme::TEXT_PRIMARY));
    menu.set_text_size(12);

    // Menu items will be wired via callbacks after layout

    // ═══════════════════════════════════════════════════════════════════════════
    //  ROOT LAYOUT (below menu bar)
    // ═══════════════════════════════════════════════════════════════════════════

    let mut root = Flex::default()
        .with_pos(0, 25)
        .with_size(1400, 850)
        .row();

    // ─── LEFT PANEL (Controls) ─────────────────────────────────────────────────

    let mut left_scroll = fltk::group::Scroll::default();
    left_scroll.set_color(theme::color(theme::BG_PANEL));
    root.fixed(&left_scroll, 280);

    let mut left = Flex::default()
        .with_size(275, 1200)  // tall enough for all controls
        .column();
    left.set_margin(5);
    left.set_pad(2);

    // ── Title ──
    let mut title = Frame::default().with_label("FFT Analyzer");
    title.set_label_size(15);
    title.set_label_color(theme::color(theme::ACCENT_BLUE));
    left.fixed(&title, 28);

    // ════════════════════════════════════════════════════════════════
    //  SECTION: File Operations
    // ════════════════════════════════════════════════════════════════

    let mut lbl_file = Frame::default().with_label("FILE");
    lbl_file.set_label_color(theme::section_header_color());
    lbl_file.set_label_size(11);
    lbl_file.set_align(Align::Inside | Align::Left);
    left.fixed(&lbl_file, 18);

    let mut btn_open = Button::default().with_label("Open Audio File");
    btn_open.set_color(theme::color(theme::BG_WIDGET));
    btn_open.set_label_color(theme::color(theme::TEXT_PRIMARY));
    set_tooltip(&mut btn_open, "Open a WAV audio file for analysis.\nSupports 8/16/24/32-bit PCM and float formats.");
    left.fixed(&btn_open, 28);

    let mut btn_save_fft = Button::default().with_label("Save FFT Data");
    btn_save_fft.set_color(theme::color(theme::BG_WIDGET));
    btn_save_fft.set_label_color(theme::color(theme::TEXT_PRIMARY));
    btn_save_fft.deactivate();
    set_tooltip(&mut btn_save_fft, "Export spectrogram data to CSV.\nRequires FFT data to be computed first.");
    left.fixed(&btn_save_fft, 28);

    let mut btn_load_fft = Button::default().with_label("Load FFT Data");
    btn_load_fft.set_color(theme::color(theme::BG_WIDGET));
    btn_load_fft.set_label_color(theme::color(theme::TEXT_PRIMARY));
    set_tooltip(&mut btn_load_fft, "Import previously saved FFT data from CSV.");
    left.fixed(&btn_load_fft, 28);

    let mut btn_save_wav = Button::default().with_label("Export WAV");
    btn_save_wav.set_color(theme::color(theme::BG_WIDGET));
    btn_save_wav.set_label_color(theme::color(theme::TEXT_PRIMARY));
    btn_save_wav.deactivate();
    set_tooltip(&mut btn_save_wav, "Save reconstructed audio as 16-bit WAV.\nReconstruct audio first, then export.");
    left.fixed(&btn_save_wav, 28);

    // Separator
    let mut sep1 = Frame::default();
    sep1.set_frame(FrameType::FlatBox);
    sep1.set_color(theme::color(theme::SEPARATOR));
    left.fixed(&sep1, 1);

    // ════════════════════════════════════════════════════════════════
    //  SECTION: Analysis Parameters
    // ════════════════════════════════════════════════════════════════

    let mut lbl_analysis = Frame::default().with_label("ANALYSIS");
    lbl_analysis.set_label_color(theme::section_header_color());
    lbl_analysis.set_label_size(11);
    lbl_analysis.set_align(Align::Inside | Align::Left);
    left.fixed(&lbl_analysis, 18);

    // Time range
    let mut btn_time_unit = Button::default().with_label("Unit: Seconds");
    btn_time_unit.set_color(theme::color(theme::BG_WIDGET));
    btn_time_unit.set_label_color(theme::color(theme::TEXT_PRIMARY));
    btn_time_unit.set_label_size(11);
    btn_time_unit.deactivate();
    set_tooltip(&mut btn_time_unit, "Toggle between Seconds and Samples.\nClicking converts the start/stop values.");
    left.fixed(&btn_time_unit, 25);

    let mut input_start = FloatInput::default().with_label("Start:");
    input_start.set_value("0");
    input_start.set_color(theme::color(theme::BG_WIDGET));
    input_start.set_text_color(theme::color(theme::TEXT_PRIMARY));
    input_start.deactivate();
    set_tooltip(&mut input_start, "Analysis start position.\nFunctional range: 0 to audio duration.\nYou can go outside this range if you want.");
    attach_float_validation(&mut input_start);
    left.fixed(&input_start, 25);

    let mut input_stop = FloatInput::default().with_label("Stop:");
    input_stop.set_value("0");
    input_stop.set_color(theme::color(theme::BG_WIDGET));
    input_stop.set_text_color(theme::color(theme::TEXT_PRIMARY));
    input_stop.deactivate();
    set_tooltip(&mut input_stop, "Analysis stop position.\nFunctional range: 0 to audio duration.\nYou can go outside this range if you want.");
    attach_float_validation(&mut input_stop);
    left.fixed(&input_stop, 25);

    // Window length (segments) with +/- buttons
    let mut lbl_wl = Frame::default().with_label("Segment Size:");
    lbl_wl.set_label_color(theme::color(theme::TEXT_SECONDARY));
    lbl_wl.set_label_size(11);
    lbl_wl.set_align(Align::Inside | Align::Left);
    left.fixed(&lbl_wl, 16);

    let mut seg_row = Flex::default().row();
    seg_row.set_pad(2);

    let mut btn_seg_minus = Button::default().with_label("-");
    btn_seg_minus.set_color(theme::color(theme::BG_WIDGET));
    btn_seg_minus.set_label_color(theme::color(theme::TEXT_PRIMARY));
    btn_seg_minus.set_label_size(14);
    btn_seg_minus.deactivate();
    set_tooltip(&mut btn_seg_minus, "Halve the segment size.\nSmaller segments = better time resolution, worse frequency resolution.");
    seg_row.fixed(&btn_seg_minus, 30);

    let mut btn_seg_plus = Button::default().with_label("+");
    btn_seg_plus.set_color(theme::color(theme::BG_WIDGET));
    btn_seg_plus.set_label_color(theme::color(theme::TEXT_PRIMARY));
    btn_seg_plus.set_label_size(14);
    btn_seg_plus.deactivate();
    set_tooltip(&mut btn_seg_plus, "Double the segment size.\nLarger segments = better frequency resolution, worse time resolution.");
    seg_row.fixed(&btn_seg_plus, 30);

    let mut lbl_seg_value = Frame::default().with_label("2048 smp / 42.67 ms");
    lbl_seg_value.set_label_color(theme::color(theme::TEXT_PRIMARY));
    lbl_seg_value.set_label_size(10);
    lbl_seg_value.set_align(Align::Inside | Align::Left);

    seg_row.end();
    left.fixed(&seg_row, 25);

    // Overlap
    let mut slider_overlap = HorNiceSlider::default();
    slider_overlap.set_minimum(0.0);
    slider_overlap.set_maximum(95.0);
    slider_overlap.set_value(75.0);
    slider_overlap.set_color(theme::color(theme::BG_WIDGET));
    slider_overlap.set_selection_color(theme::accent_color());
    slider_overlap.deactivate();
    set_tooltip(&mut slider_overlap, "Overlap between adjacent FFT windows.\nFunctional range: 0% to 95%.\nHigher = more time frames, smoother spectrogram.\n75% is standard for Hann window.");
    left.fixed(&slider_overlap, 22);

    let mut lbl_overlap_val = Frame::default().with_label("Overlap: 75%");
    lbl_overlap_val.set_label_color(theme::color(theme::TEXT_SECONDARY));
    lbl_overlap_val.set_label_size(11);
    lbl_overlap_val.set_align(Align::Inside | Align::Right);
    left.fixed(&lbl_overlap_val, 14);

    // Window type
    let mut window_type_choice = Choice::default();
    window_type_choice.add_choice("Hann");
    window_type_choice.add_choice("Hamming");
    window_type_choice.add_choice("Blackman");
    window_type_choice.add_choice("Kaiser");
    window_type_choice.set_value(0);
    window_type_choice.set_color(theme::color(theme::BG_WIDGET));
    window_type_choice.set_text_color(theme::color(theme::TEXT_PRIMARY));
    window_type_choice.deactivate();
    set_tooltip(&mut window_type_choice, "Windowing function applied to each FFT segment.\nHann: general purpose, good frequency resolution.\nHamming: slightly better sidelobe rejection.\nBlackman: best sidelobe rejection, wider main lobe.\nKaiser: configurable via beta parameter.");
    left.fixed(&window_type_choice, 25);

    let mut input_kaiser_beta = FloatInput::default().with_label("Kaiser B:");
    input_kaiser_beta.set_value("8.6");
    input_kaiser_beta.set_color(theme::color(theme::BG_WIDGET));
    input_kaiser_beta.set_text_color(theme::color(theme::TEXT_PRIMARY));
    input_kaiser_beta.deactivate();
    set_tooltip(&mut input_kaiser_beta, "Kaiser window beta parameter.\nFunctional range: 0.0 to 20.0.\nHigher = narrower main lobe, higher sidelobes.\n8.6 approximates a Blackman window.");
    left.fixed(&input_kaiser_beta, 25);

    let mut check_center = fltk::button::CheckButton::default().with_label(" Center/Pad");
    check_center.set_checked(true);
    check_center.set_label_color(theme::color(theme::TEXT_PRIMARY));
    check_center.deactivate();
    set_tooltip(&mut check_center, "Add zero-padding around signal for symmetric analysis.\nRecommended: ON for most use cases.");
    left.fixed(&check_center, 22);

    let mut btn_rerun = Button::default().with_label("Recompute + Rebuild (Space)");
    btn_rerun.set_color(theme::color(theme::ACCENT_BLUE));
    btn_rerun.set_label_color(theme::color(theme::BG_DARK));
    btn_rerun.set_label_size(11);
    btn_rerun.deactivate();
    set_tooltip(&mut btn_rerun, "Rerun FFT + reconstruct audio with current parameters.\nShortcut: Spacebar (works from anywhere).\nAll outputs (spectrogram, waveform, audio) will update.");
    left.fixed(&btn_rerun, 28);

    // Separator
    let mut sep2 = Frame::default();
    sep2.set_frame(FrameType::FlatBox);
    sep2.set_color(theme::color(theme::SEPARATOR));
    left.fixed(&sep2, 1);

    // ════════════════════════════════════════════════════════════════
    //  SECTION: Display
    // ════════════════════════════════════════════════════════════════

    let mut lbl_display = Frame::default().with_label("DISPLAY");
    lbl_display.set_label_color(theme::section_header_color());
    lbl_display.set_label_size(11);
    lbl_display.set_align(Align::Inside | Align::Left);
    left.fixed(&lbl_display, 18);

    // Lock toggles: when ON, viewport = processing window
    let mut check_lock_time = fltk::button::CheckButton::default().with_label(" Lock Time (H)");
    check_lock_time.set_checked(true);
    check_lock_time.set_label_color(theme::color(theme::TEXT_PRIMARY));
    set_tooltip(&mut check_lock_time, "Lock time viewport to processing window.\nON: scrollbar shifts time start/stop together, view = processed region.\nOFF: scrollbar moves viewport freely; unprocessed time is dimmed.");
    left.fixed(&check_lock_time, 20);

    let mut check_lock_freq = fltk::button::CheckButton::default().with_label(" Lock Freq (V)");
    check_lock_freq.set_checked(true);
    check_lock_freq.set_label_color(theme::color(theme::TEXT_PRIMARY));
    set_tooltip(&mut check_lock_freq, "Lock frequency viewport to reconstruction range.\nON: viewport zooms to freq min/max, scrollbar shifts range.\nOFF: viewport scrolls freely; out-of-range frequencies are dimmed.");
    left.fixed(&check_lock_freq, 20);

    // Colormap
    let mut colormap_choice = Choice::default();
    for cm in ColormapId::ALL {
        colormap_choice.add_choice(cm.name());
    }
    colormap_choice.set_value(0);
    colormap_choice.set_color(theme::color(theme::BG_WIDGET));
    colormap_choice.set_text_color(theme::color(theme::TEXT_PRIMARY));
    set_tooltip(&mut colormap_choice, "Color scheme for the spectrogram display.\nClassic: blue-cyan-green-yellow-red (rainbow)\nViridis/Magma/Inferno: perceptually uniform scientific colormaps\nGreyscale: black to white\nInverted Grey: white to black (print-friendly)");
    left.fixed(&colormap_choice, 25);

    // Scale toggle
    let mut scale_choice = Choice::default();
    scale_choice.add_choice("Log Frequency");
    scale_choice.add_choice("Linear Frequency");
    scale_choice.set_value(0);
    scale_choice.set_color(theme::color(theme::BG_WIDGET));
    scale_choice.set_text_color(theme::color(theme::TEXT_PRIMARY));
    set_tooltip(&mut scale_choice, "Frequency axis scaling.\nLog: musical/perceptual spacing (recommended).\n  Octaves get equal visual space.\nLinear: uniform Hz spacing.\n  High frequencies dominate the display.");
    left.fixed(&scale_choice, 25);

    // Threshold
    let mut slider_threshold = HorNiceSlider::default();
    slider_threshold.set_minimum(-120.0);
    slider_threshold.set_maximum(0.0);
    slider_threshold.set_value(-80.0);
    slider_threshold.set_color(theme::color(theme::BG_WIDGET));
    slider_threshold.set_selection_color(theme::accent_color());
    set_tooltip(&mut slider_threshold, "Minimum dB level to display.\nFunctional range: -120 dB to 0 dB.\nAnything below this threshold appears as background color.\nLower = show more quiet detail. Higher = focus on loud content.");
    left.fixed(&slider_threshold, 22);

    let mut lbl_threshold_val = Frame::default().with_label("Threshold: -80 dB");
    lbl_threshold_val.set_label_color(theme::color(theme::TEXT_SECONDARY));
    lbl_threshold_val.set_label_size(11);
    lbl_threshold_val.set_align(Align::Inside | Align::Right);
    left.fixed(&lbl_threshold_val, 14);

    // Brightness
    let mut slider_brightness = HorNiceSlider::default();
    slider_brightness.set_minimum(0.1);
    slider_brightness.set_maximum(3.0);
    slider_brightness.set_value(1.0);
    slider_brightness.set_color(theme::color(theme::BG_WIDGET));
    slider_brightness.set_selection_color(theme::accent_color());
    set_tooltip(&mut slider_brightness, "Overall brightness multiplier.\nFunctional range: 0.1 to 3.0.\n1.0 = neutral. Higher = brighter colors for quiet content.");
    left.fixed(&slider_brightness, 22);

    let mut lbl_brightness_val = Frame::default().with_label("Brightness: 1.0");
    lbl_brightness_val.set_label_color(theme::color(theme::TEXT_SECONDARY));
    lbl_brightness_val.set_label_size(11);
    lbl_brightness_val.set_align(Align::Inside | Align::Right);
    left.fixed(&lbl_brightness_val, 14);

    // Gamma
    let mut slider_gamma = HorNiceSlider::default();
    slider_gamma.set_minimum(0.5);
    slider_gamma.set_maximum(5.0);
    slider_gamma.set_value(2.2);
    slider_gamma.set_color(theme::color(theme::BG_WIDGET));
    slider_gamma.set_selection_color(theme::accent_color());
    set_tooltip(&mut slider_gamma, "Perceptual gamma correction for dB display.\nFunctional range: 0.5 to 5.0.\n2.2 = standard perceptual gamma (recommended).\nHigher = more contrast, quiet content less visible.\nLower = flatter, quiet content more visible.");
    left.fixed(&slider_gamma, 22);

    let mut lbl_gamma_val = Frame::default().with_label("Gamma: 2.2");
    lbl_gamma_val.set_label_color(theme::color(theme::TEXT_SECONDARY));
    lbl_gamma_val.set_label_size(11);
    lbl_gamma_val.set_align(Align::Inside | Align::Right);
    left.fixed(&lbl_gamma_val, 14);

    // Separator
    let mut sep3 = Frame::default();
    sep3.set_frame(FrameType::FlatBox);
    sep3.set_color(theme::color(theme::SEPARATOR));
    left.fixed(&sep3, 1);

    // ════════════════════════════════════════════════════════════════
    //  SECTION: Reconstruction
    // ════════════════════════════════════════════════════════════════

    let mut lbl_recon = Frame::default().with_label("RECONSTRUCTION");
    lbl_recon.set_label_color(theme::section_header_color());
    lbl_recon.set_label_size(11);
    lbl_recon.set_align(Align::Inside | Align::Left);
    left.fixed(&lbl_recon, 18);

    // Frequency count
    let mut lbl_fc = Frame::default().with_label("Freq Count:");
    lbl_fc.set_label_color(theme::color(theme::TEXT_SECONDARY));
    lbl_fc.set_label_size(11);
    lbl_fc.set_align(Align::Inside | Align::Left);
    left.fixed(&lbl_fc, 16);

    let mut input_freq_count = Input::default();
    input_freq_count.set_value("1025");
    input_freq_count.set_color(theme::color(theme::BG_WIDGET));
    input_freq_count.set_text_color(theme::color(theme::TEXT_PRIMARY));
    input_freq_count.deactivate();
    set_tooltip(&mut input_freq_count, "Number of top-magnitude frequency bins to keep per frame.\nFunctional range: 1 to max bins (shown in INFO).\nMax = perfect reconstruction. Lower = simplified/filtered sound.\nAt 1, only the loudest frequency per frame is reconstructed.");
    left.fixed(&input_freq_count, 25);

    // Frequency range
    let mut lbl_freq_min = Frame::default().with_label("Recon Min Freq (Hz):");
    lbl_freq_min.set_label_color(theme::color(theme::TEXT_SECONDARY));
    lbl_freq_min.set_label_size(11);
    lbl_freq_min.set_align(Align::Inside | Align::Left);
    left.fixed(&lbl_freq_min, 16);

    let mut input_recon_freq_min = FloatInput::default();
    input_recon_freq_min.set_value("0");
    input_recon_freq_min.set_color(theme::color(theme::BG_WIDGET));
    input_recon_freq_min.set_text_color(theme::color(theme::TEXT_PRIMARY));
    input_recon_freq_min.deactivate();
    set_tooltip(&mut input_recon_freq_min, "Minimum frequency for reconstruction.\nFunctional range: 0 to Nyquist.\nBins below this frequency are zeroed out.");
    left.fixed(&input_recon_freq_min, 25);

    let mut lbl_freq_max = Frame::default().with_label("Recon Max Freq (Hz):");
    lbl_freq_max.set_label_color(theme::color(theme::TEXT_SECONDARY));
    lbl_freq_max.set_label_size(11);
    lbl_freq_max.set_align(Align::Inside | Align::Left);
    left.fixed(&lbl_freq_max, 16);

    let mut input_recon_freq_max = FloatInput::default();
    input_recon_freq_max.set_value("24000");
    input_recon_freq_max.set_color(theme::color(theme::BG_WIDGET));
    input_recon_freq_max.set_text_color(theme::color(theme::TEXT_PRIMARY));
    input_recon_freq_max.deactivate();
    set_tooltip(&mut input_recon_freq_max, "Maximum frequency for reconstruction.\nFunctional range: 0 to Nyquist.\nBins above this frequency are zeroed out.");
    left.fixed(&input_recon_freq_max, 25);

    // Reconstruction is now combined with Recompute (spacebar).
    // No separate button needed.

    // Separator
    let mut sep4 = Frame::default();
    sep4.set_frame(FrameType::FlatBox);
    sep4.set_color(theme::color(theme::SEPARATOR));
    left.fixed(&sep4, 1);

    // ════════════════════════════════════════════════════════════════
    //  SECTION: Info Panel (read-only)
    // ════════════════════════════════════════════════════════════════

    let mut lbl_info_header = Frame::default().with_label("INFO");
    lbl_info_header.set_label_color(theme::section_header_color());
    lbl_info_header.set_label_size(11);
    lbl_info_header.set_align(Align::Inside | Align::Left);
    left.fixed(&lbl_info_header, 18);

    let mut lbl_info = Frame::default().with_label("No audio loaded");
    lbl_info.set_label_color(theme::color(theme::TEXT_SECONDARY));
    lbl_info.set_label_size(10);
    lbl_info.set_align(Align::Inside | Align::Left | Align::Top);
    left.fixed(&lbl_info, 110);

    // Separator
    let mut sep5 = Frame::default();
    sep5.set_frame(FrameType::FlatBox);
    sep5.set_color(theme::color(theme::SEPARATOR));
    left.fixed(&sep5, 1);

    // Tooltip toggle
    let mut btn_tooltips = fltk::button::CheckButton::default().with_label(" Show Tooltips");
    btn_tooltips.set_checked(true);
    btn_tooltips.set_label_color(theme::color(theme::TEXT_SECONDARY));
    btn_tooltips.set_label_size(10);
    set_tooltip(&mut btn_tooltips, "Toggle tooltip help bubbles on/off.");
    left.fixed(&btn_tooltips, 22);

    // Spacer to push everything up
    Frame::default();

    left.end();
    left_scroll.end();

    // ─── RIGHT PANEL (Display area) ────────────────────────────────────────────

    let mut right = Flex::default().column();
    right.set_margin(2);
    right.set_pad(2);

    // ── Waveform strip ──
    let mut waveform_display = Widget::default();
    waveform_display.set_frame(FrameType::FlatBox);
    waveform_display.set_color(theme::color(theme::BG_DARK));
    right.fixed(&waveform_display, 100);

    // ── Spectrogram area (with Y scrollbar) ──
    let mut spec_row = Flex::default().row();

    // Frequency axis label area
    let mut freq_axis = Widget::default();
    freq_axis.set_frame(FrameType::FlatBox);
    freq_axis.set_color(theme::color(theme::BG_DARK));
    spec_row.fixed(&freq_axis, 50);

    // Main spectrogram display
    let mut spec_display = Widget::default();
    spec_display.set_frame(FrameType::FlatBox);
    spec_display.set_color(theme::color(theme::BG_DARK));

    // Y-axis zoom scrollbar
    let mut y_scroll = fltk::valuator::Scrollbar::default();
    y_scroll.set_type(fltk::valuator::ScrollbarType::Vertical);
    y_scroll.set_color(theme::color(theme::BG_WIDGET));
    y_scroll.set_selection_color(theme::accent_color());
    set_tooltip(&mut y_scroll, "Frequency axis zoom.\nDrag to scroll. Scroll wheel on spectrogram to zoom.");
    spec_row.fixed(&y_scroll, 16);

    spec_row.end();

    // ── Time axis label area ──
    let mut time_axis = Widget::default();
    time_axis.set_frame(FrameType::FlatBox);
    time_axis.set_color(theme::color(theme::BG_DARK));
    right.fixed(&time_axis, 20);

    // ── X-axis zoom scrollbar ──
    let mut x_scroll = fltk::valuator::Scrollbar::default();
    x_scroll.set_type(fltk::valuator::ScrollbarType::Horizontal);
    x_scroll.set_color(theme::color(theme::BG_WIDGET));
    x_scroll.set_selection_color(theme::accent_color());
    set_tooltip(&mut x_scroll, "Time axis zoom.\nDrag to scroll. Scroll wheel on spectrogram to zoom.");
    right.fixed(&x_scroll, 16);

    // ── Transport bar ──
    let mut transport_row = Flex::default().row();
    transport_row.set_color(theme::color(theme::BG_PANEL));
    right.fixed(&transport_row, 32);

    let mut btn_play = Button::default().with_label("@>");
    btn_play.set_color(theme::color(theme::BG_WIDGET));
    btn_play.set_label_color(theme::color(theme::ACCENT_GREEN));
    btn_play.deactivate();
    set_tooltip(&mut btn_play, "Play audio from current position.");
    transport_row.fixed(&btn_play, 36);

    let mut btn_pause = Button::default().with_label("@||");
    btn_pause.set_color(theme::color(theme::BG_WIDGET));
    btn_pause.set_label_color(theme::color(theme::ACCENT_YELLOW));
    btn_pause.deactivate();
    set_tooltip(&mut btn_pause, "Pause playback at current position.");
    transport_row.fixed(&btn_pause, 36);

    let mut btn_stop = Button::default().with_label("@square");
    btn_stop.set_color(theme::color(theme::BG_WIDGET));
    btn_stop.set_label_color(theme::color(theme::ACCENT_RED));
    btn_stop.deactivate();
    set_tooltip(&mut btn_stop, "Stop playback and reset to start.");
    transport_row.fixed(&btn_stop, 36);

    // Scrub slider
    let mut scrub_slider = HorSlider::default();
    scrub_slider.set_minimum(0.0);
    scrub_slider.set_maximum(1.0);
    scrub_slider.set_value(0.0);
    scrub_slider.set_color(theme::color(theme::BG_WIDGET));
    scrub_slider.set_selection_color(theme::color(theme::ACCENT_RED));
    scrub_slider.deactivate();
    set_tooltip(&mut scrub_slider, "Playback position scrubber.\nDrag to seek. Audio plays from drag position when in play mode.");

    let mut lbl_time = Frame::default().with_label("0:00.00 / 0:00.00");
    lbl_time.set_label_color(theme::color(theme::TEXT_SECONDARY));
    lbl_time.set_label_size(11);
    transport_row.fixed(&lbl_time, 120);

    let mut repeat_choice = Choice::default();
    repeat_choice.add_choice("Single");
    repeat_choice.add_choice("Repeat");
    repeat_choice.set_value(0);
    repeat_choice.set_color(theme::color(theme::BG_WIDGET));
    repeat_choice.set_text_color(theme::color(theme::TEXT_PRIMARY));
    repeat_choice.deactivate();
    set_tooltip(&mut repeat_choice, "Single: stop at end.\nRepeat: loop continuously.");
    transport_row.fixed(&repeat_choice, 70);

    transport_row.end();

    right.end();
    root.end();

    // ─── STATUS BAR ───────────────────────────────────────────────────────────

    let mut status_bar = Frame::default()
        .with_pos(0, 875)
        .with_size(1400, 25)
        .with_label("Ready | Load an audio file to begin");
    status_bar.set_frame(FrameType::FlatBox);
    status_bar.set_color(theme::color(theme::BG_PANEL));
    status_bar.set_label_color(theme::color(theme::TEXT_SECONDARY));
    status_bar.set_label_size(11);
    status_bar.set_align(Align::Inside | Align::Left);

    win.end();

    // Make the window resize properly
    win.resizable(&root);

    // ═══════════════════════════════════════════════════════════════════════════
    //  MENU CALLBACKS
    // ═══════════════════════════════════════════════════════════════════════════

    {
        let mut btn_open = btn_open.clone();
        menu.add("&File/Open Audio\t", Shortcut::Ctrl | 'o', MenuFlag::Normal,
            move |_| { btn_open.do_callback(); });
    }
    {
        let mut btn_save_fft = btn_save_fft.clone();
        menu.add("&File/Save FFT Data\t", Shortcut::Ctrl | 's', MenuFlag::Normal,
            move |_| { btn_save_fft.do_callback(); });
    }
    {
        let mut btn_load_fft = btn_load_fft.clone();
        menu.add("&File/Load FFT Data\t", Shortcut::Ctrl | 'l', MenuFlag::Normal,
            move |_| { btn_load_fft.do_callback(); });
    }
    {
        let mut btn_save_wav = btn_save_wav.clone();
        menu.add("&File/Export WAV\t", Shortcut::Ctrl | 'e', MenuFlag::Normal,
            move |_| { btn_save_wav.do_callback(); });
    }
    menu.add("&File/Quit\t", Shortcut::Ctrl | 'q', MenuFlag::Normal,
        move |_| { app::quit(); });

    {
        let mut btn_rerun = btn_rerun.clone();
        menu.add("&Analysis/Recompute FFT\t", Shortcut::None, MenuFlag::Normal,
            move |_| { btn_rerun.do_callback(); });
    }

    {
        let state_c = state.clone();
        let mut spec_display_c = spec_display.clone();
        menu.add("&Display/Reset Zoom\t", Shortcut::None, MenuFlag::Normal,
            move |_| {
                let mut st = state_c.borrow_mut();
                st.view.reset_zoom();
                st.spec_renderer.invalidate();
                st.wave_renderer.invalidate();
                drop(st);
                spec_display_c.redraw();
            });
    }

    // ═══════════════════════════════════════════════════════════════════════════
    //  HELPER: Update derived info labels
    // ═══════════════════════════════════════════════════════════════════════════

    // Type alias for shared mutable closures
    type SharedCb = Rc<RefCell<Box<dyn FnMut()>>>;

    let update_info: SharedCb = {
        let state = state.clone();
        let mut lbl_info = lbl_info.clone();
        let mut input_freq_count = input_freq_count.clone();
        Rc::new(RefCell::new(Box::new(move || {
            let st = state.borrow();
            let info = st.derived_info();
            lbl_info.set_label(&info.format_info());

            // Clamp freq count display to max
            let current: usize = input_freq_count.value().parse().unwrap_or(info.freq_bins);
            if current > info.freq_bins {
                input_freq_count.set_value(&info.freq_bins.to_string());
            }
        })))
    };

    // Helper: update segment size label with dual display
    let update_seg_label: SharedCb = {
        let state = state.clone();
        let mut lbl_seg_value = lbl_seg_value.clone();
        Rc::new(RefCell::new(Box::new(move || {
            let st = state.borrow();
            let wl = st.fft_params.window_length;
            let sr = st.fft_params.sample_rate;
            let ms = wl as f64 / sr as f64 * 1000.0;
            lbl_seg_value.set_label(&format!("{} smp / {:.2} ms", wl, ms));
        })))
    };

    // Helper: enable widgets that require audio data
    let enable_audio_widgets: SharedCb = {
        let mut btn_time_unit = btn_time_unit.clone();
        let mut input_start = input_start.clone();
        let mut input_stop = input_stop.clone();
        let mut btn_seg_minus = btn_seg_minus.clone();
        let mut btn_seg_plus = btn_seg_plus.clone();
        let mut slider_overlap = slider_overlap.clone();
        let mut window_type_choice = window_type_choice.clone();
        let mut check_center = check_center.clone();
        let mut btn_rerun = btn_rerun.clone();
        Rc::new(RefCell::new(Box::new(move || {
            btn_time_unit.activate();
            input_start.activate();
            input_stop.activate();
            btn_seg_minus.activate();
            btn_seg_plus.activate();
            slider_overlap.activate();
            window_type_choice.activate();
            check_center.activate();
            btn_rerun.activate();
        })))
    };

    // Helper: enable widgets that require spectrogram data
    let enable_spec_widgets: SharedCb = {
        let mut btn_save_fft = btn_save_fft.clone();
        let mut input_freq_count = input_freq_count.clone();
        let mut input_recon_freq_min = input_recon_freq_min.clone();
        let mut input_recon_freq_max = input_recon_freq_max.clone();
        let mut btn_play = btn_play.clone();
        let mut btn_pause = btn_pause.clone();
        let mut btn_stop = btn_stop.clone();
        let mut scrub_slider = scrub_slider.clone();
        let mut repeat_choice = repeat_choice.clone();
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
        })))
    };

    // Helper: enable WAV export when reconstruction is ready
    let enable_wav_export: SharedCb = {
        let mut btn_save_wav = btn_save_wav.clone();
        Rc::new(RefCell::new(Box::new(move || {
            btn_save_wav.activate();
        })))
    };

    // ═══════════════════════════════════════════════════════════════════════════
    //  CALLBACKS
    // ═══════════════════════════════════════════════════════════════════════════

    // ── Open Audio File ──
    {
        let state = state.clone();
        let mut status_bar = status_bar.clone();
        let mut input_stop = input_stop.clone();
        let mut input_recon_freq_max = input_recon_freq_max.clone();
        let mut spec_display = spec_display.clone();
        let mut waveform_display = waveform_display.clone();
        let tx = tx.clone();
        let update_info = update_info.clone();
        let update_seg_label = update_seg_label.clone();
        let enable_audio_widgets = enable_audio_widgets.clone();

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
                        st.view.freq_max_hz = nyquist;
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
    {
        let state = state.clone();
        let mut status_bar = status_bar.clone();

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

            let spec_ref = st.spectrogram.as_ref().unwrap();

            match csv_export::export_to_csv(spec_ref, &st.fft_params, &st.view, &filename) {
                Ok(_) => {
                    status_bar.set_label("FFT data saved");
                }
                Err(e) => {
                    dialog::alert_default(&format!("Error saving CSV:\n{}", e));
                    status_bar.set_label("Save failed");
                }
            }
        });
    }

    // ── Load FFT from CSV ──
    {
        let state = state.clone();
        let mut status_bar = status_bar.clone();
        let mut spec_display = spec_display.clone();
        let mut input_start = input_start.clone();
        let mut input_stop = input_stop.clone();
        let mut slider_overlap = slider_overlap.clone();
        let update_info = update_info.clone();
        let update_seg_label = update_seg_label.clone();
        let enable_audio_widgets = enable_audio_widgets.clone();
        let enable_spec_widgets = enable_spec_widgets.clone();

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
                Ok((imported_spec, imported_params, recon_params)) => {
                    let num_frames = imported_spec.num_frames();
                    {
                        let mut st = state.borrow_mut();
                        st.fft_params = imported_params.clone();

                        st.view.time_min_sec = imported_spec.min_time;
                        st.view.time_max_sec = imported_spec.max_time;
                        st.view.data_time_min_sec = imported_spec.min_time;
                        st.view.data_time_max_sec = imported_spec.max_time;
                        st.view.freq_max_hz = imported_spec.max_freq;
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
                    }

                    input_start.set_value(&format!("{:.5}", imported_params.start_time));
                    input_stop.set_value(&format!("{:.5}", imported_params.stop_time));
                    slider_overlap.set_value(imported_params.overlap_percent as f64);

                    (enable_audio_widgets.borrow_mut())();
                    (enable_spec_widgets.borrow_mut())();
                    (update_info.borrow_mut())();
                    (update_seg_label.borrow_mut())();

                    status_bar.set_label(&format!("Loaded {} frames from CSV", num_frames));
                    spec_display.redraw();
                }
                Err(e) => {
                    dialog::alert_default(&format!("Error loading CSV:\n{}", e));
                    status_bar.set_label("CSV load failed");
                }
            }
        });
    }

    // ── Export WAV ──
    {
        let state = state.clone();
        let mut status_bar = status_bar.clone();

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

    // ── Rerun analysis + reconstruction (also triggered by spacebar) ──
    // Reads ALL current field values before launching FFT so that
    // typing a number + spacebar works without needing to press Enter.
    // After FFT completes, reconstruction is automatically triggered.
    {
        let state = state.clone();
        let mut status_bar = status_bar.clone();
        let mut spec_display = spec_display.clone();
        let mut waveform_display = waveform_display.clone();
        let tx = tx.clone();
        let input_start = input_start.clone();
        let input_stop = input_stop.clone();
        let slider_overlap = slider_overlap.clone();
        let input_freq_count = input_freq_count.clone();
        let input_recon_freq_min = input_recon_freq_min.clone();
        let input_recon_freq_max = input_recon_freq_max.clone();
        let check_center = check_center.clone();
        let update_info = update_info.clone();
        let update_seg_label = update_seg_label.clone();
        let window_type_choice = window_type_choice.clone();
        let input_kaiser_beta = input_kaiser_beta.clone();

        btn_rerun.set_callback(move |_| {
            // Sync all field values into state before running
            {
                let mut st = state.borrow_mut();
                if st.audio_data.is_none() { return; }
                if st.is_processing { return; }

                // Read current field values
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

                // Sync viewport when locked
                if st.view.lock_freq {
                    let fmin = st.view.recon_freq_min_hz.max(1.0);
                    let fmax = st.view.recon_freq_max_hz.max(fmin + 1.0);
                    st.view.freq_min_hz = fmin;
                    st.view.freq_max_hz = fmax;
                }

                st.is_processing = true;
                st.spec_renderer.invalidate();
                st.wave_renderer.invalidate();
            }

            let (audio, params) = {
                let st = state.borrow();
                (st.audio_data.clone().unwrap(), st.fft_params.clone())
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

    // ── Parameter callbacks ──

    // Time unit toggle
    {
        let state = state.clone();
        let mut input_start = input_start.clone();
        let mut input_stop = input_stop.clone();

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

    // Field values are read at recompute time (btn_rerun callback).
    // No individual callbacks needed for start/stop/window_len.

    // Overlap
    {
        let mut lbl = lbl_overlap_val.clone();
        let state = state.clone();
        let update_info = update_info.clone();

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
        let mut input_kaiser_beta = input_kaiser_beta.clone();

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
        let update_info = update_info.clone();
        let update_seg_label = update_seg_label.clone();

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
        let update_info = update_info.clone();
        let update_seg_label = update_seg_label.clone();

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
    attach_float_validation(&mut input_kaiser_beta);

    // Center/Pad
    {
        let state = state.clone();
        let update_info = update_info.clone();
        check_center.set_callback(move |c| {
            state.borrow_mut().fft_params.use_center = c.is_checked();
            (update_info.borrow_mut())();
        });
    }

    // ── Display callbacks ──

    // Colormap
    {
        let state = state.clone();
        let mut spec_display = spec_display.clone();

        colormap_choice.set_callback(move |c| {
            let mut st = state.borrow_mut();
            st.view.colormap = ColormapId::from_index(c.value() as usize);
            st.spec_renderer.invalidate();
            drop(st);
            spec_display.redraw();
        });
    }

    // Scale
    {
        let state = state.clone();
        let mut spec_display = spec_display.clone();

        scale_choice.set_callback(move |c| {
            let mut st = state.borrow_mut();
            st.view.freq_scale = if c.value() == 0 { FreqScale::Log } else { FreqScale::Linear };
            st.spec_renderer.invalidate();
            drop(st);
            spec_display.redraw();
        });
    }

    // Threshold
    {
        let mut lbl = lbl_threshold_val.clone();
        let state = state.clone();
        let mut spec_display = spec_display.clone();
        let throttle = Rc::new(RefCell::new(UpdateThrottle::new(50)));

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
        let mut lbl = lbl_brightness_val.clone();
        let state = state.clone();
        let mut spec_display = spec_display.clone();
        let throttle = Rc::new(RefCell::new(UpdateThrottle::new(50)));

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
        let mut lbl = lbl_gamma_val.clone();
        let state = state.clone();
        let mut spec_display = spec_display.clone();
        let throttle = Rc::new(RefCell::new(UpdateThrottle::new(50)));

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

    // Reconstruction is now triggered automatically after FFT completes.
    // No separate callback needed.

    // ── Playback callbacks ──

    {
        let state = state.clone();
        btn_play.set_callback(move |_| {
            let mut st = state.borrow_mut();
            st.audio_player.play();
            st.transport.is_playing = true;
        });
    }
    {
        let state = state.clone();
        btn_pause.set_callback(move |_| {
            let mut st = state.borrow_mut();
            st.audio_player.pause();
            st.transport.is_playing = false;
        });
    }
    {
        let state = state.clone();
        btn_stop.set_callback(move |_| {
            let mut st = state.borrow_mut();
            st.audio_player.stop();
            st.transport.is_playing = false;
            st.transport.position_seconds = 0.0;
        });
    }

    // Scrub slider
    {
        let state = state.clone();

        scrub_slider.set_callback(move |s| {
            let st = state.borrow();
            let position = s.value() * st.transport.duration_seconds;
            st.audio_player.seek_to(position);
        });
    }

    // Repeat
    {
        let state = state.clone();
        repeat_choice.set_callback(move |c| {
            let mut st = state.borrow_mut();
            st.audio_player.set_repeat(c.value() == 1);
            st.transport.repeat = c.value() == 1;
        });
    }

    // Lock time toggle
    {
        let state = state.clone();
        let mut spec_display = spec_display.clone();
        let input_start = input_start.clone();
        let input_stop = input_stop.clone();

        check_lock_time.set_callback(move |c| {
            let mut st = state.borrow_mut();
            st.view.lock_time = c.is_checked();
            if c.is_checked() {
                // When locking, sync viewport to processing time range
                let start = parse_or_zero_f64(&input_start.value());
                let stop = parse_or_zero_f64(&input_stop.value());
                let (start_sec, stop_sec) = match st.fft_params.time_unit {
                    TimeUnit::Seconds => (start, stop),
                    TimeUnit::Samples => {
                        let sr = st.fft_params.sample_rate as f64;
                        (start / sr.max(1.0), stop / sr.max(1.0))
                    }
                };
                st.view.time_min_sec = start_sec;
                st.view.time_max_sec = stop_sec.max(start_sec + 0.001);
            }
            st.spec_renderer.invalidate();
            st.wave_renderer.invalidate();
            drop(st);
            spec_display.redraw();
        });
    }

    // Lock freq toggle
    {
        let state = state.clone();
        let mut spec_display = spec_display.clone();
        let input_recon_freq_min = input_recon_freq_min.clone();
        let input_recon_freq_max = input_recon_freq_max.clone();

        check_lock_freq.set_callback(move |c| {
            let mut st = state.borrow_mut();
            st.view.lock_freq = c.is_checked();
            if c.is_checked() {
                // When locking, zoom viewport to reconstruction freq range
                let fmin = parse_or_zero_f32(&input_recon_freq_min.value()).max(1.0);
                let fmax = parse_or_zero_f32(&input_recon_freq_max.value()).max(fmin + 1.0);
                st.view.freq_min_hz = fmin;
                st.view.freq_max_hz = fmax;
            }
            st.spec_renderer.invalidate();
            drop(st);
            spec_display.redraw();
        });
    }

    // Tooltip toggle
    {
        let state = state.clone();
        btn_tooltips.set_callback(move |c| {
            state.borrow_mut().tooltip_mgr.set_enabled(c.is_checked());
        });
    }

    // ═══════════════════════════════════════════════════════════════════════════
    //  DRAW CALLBACKS
    // ═══════════════════════════════════════════════════════════════════════════

    // ── Spectrogram display ──
    {
        let state = state.clone();

        spec_display.draw(move |w| {
            if !w.visible_r() || w.w() <= 0 || w.h() <= 0 {
                return;
            }

            let Ok(mut st) = state.try_borrow_mut() else { return; };

            if let Some(spec) = st.spectrogram.clone() {
                let view = st.view.clone();
                let mask = st.active_mask.clone();
                st.spec_renderer.draw(&spec, &view, mask.as_ref(), w.x(), w.y(), w.w(), w.h());

                // Draw playback cursor
                if st.transport.duration_seconds > 0.0 {
                    let cursor_t = st.view.time_to_x(st.transport.position_seconds);
                    if cursor_t >= 0.0 && cursor_t <= 1.0 {
                        let cx = w.x() + (cursor_t * w.w() as f64) as i32;
                        fltk::draw::set_draw_color(theme::color(theme::ACCENT_RED));
                        fltk::draw::draw_line(cx, w.y(), cx, w.y() + w.h());
                    }
                }
            } else {
                fltk::draw::set_draw_color(theme::color(theme::BG_DARK));
                fltk::draw::draw_rectf(w.x(), w.y(), w.w(), w.h());
                fltk::draw::set_draw_color(theme::color(theme::TEXT_DISABLED));
                fltk::draw::set_font(Font::Helvetica, 14);
                fltk::draw::draw_text("Load an audio file to begin", w.x() + 10, w.y() + w.h() / 2);
            }
        });
    }

    // ── Spectrogram mouse handling (seek + hover readout + zoom) ──
    {
        let state = state.clone();
        let mut status_bar = status_bar.clone();
        let mut spec_display_c = spec_display.clone();
        let mut waveform_display_c = waveform_display.clone();

        spec_display.handle(move |w, ev| {
            match ev {
                Event::Push => {
                    // Click to seek
                    let mx = app::event_x() - w.x();
                    let t = mx as f64 / w.w() as f64;
                    let st = state.borrow();
                    let time = st.view.x_to_time(t);
                    st.audio_player.seek_to(time);
                    true
                }
                Event::Move => {
                    // Hover readout
                    let mx = app::event_x() - w.x();
                    let my = app::event_y() - w.y();
                    let tx_norm = mx as f64 / w.w() as f64;
                    let ty_norm = 1.0 - (my as f32 / w.h() as f32);  // flip Y

                    let st = state.borrow();
                    let time = st.view.x_to_time(tx_norm);
                    let freq = st.view.y_to_freq(ty_norm);

                    if let Some(ref spec) = st.spectrogram {
                        if let Some(frame_idx) = spec.frame_at_time(time) {
                            if let Some(bin_idx) = spec.bin_at_freq(freq) {
                                if let Some(mag) = spec.frames.get(frame_idx)
                                    .and_then(|f| f.magnitudes.get(bin_idx))
                                {
                                    let db = data::Spectrogram::magnitude_to_db(*mag);
                                    status_bar.set_label(&format!(
                                        "Cursor: {:.1} Hz | {:.1} dB | {:.3}s",
                                        freq, db, time
                                    ));
                                }
                            }
                        }
                    }
                    true
                }
                Event::MouseWheel => {
                    let dy = app::event_dy();
                    let mx = app::event_x() - w.x();
                    let my = app::event_y() - w.y();

                    // MouseWheel::Down = zoom out, Up = zoom in
                    let zoom_in = matches!(dy, fltk::app::MouseWheel::Up);

                    let mut st = state.borrow_mut();

                    if app::event_state().contains(fltk::enums::Shortcut::Ctrl.into()) {
                        // Ctrl+wheel: zoom frequency axis
                        let focus_t = 1.0 - (my as f32 / w.h() as f32);
                        let focus_freq = st.view.y_to_freq(focus_t);

                        let zoom_factor = if zoom_in { 1.0 / 1.2 } else { 1.2 };
                        let range = st.view.visible_freq_range();
                        let new_range = (range * zoom_factor).clamp(10.0, st.view.data_freq_max_hz);

                        let ratio = focus_t;
                        st.view.freq_min_hz = (focus_freq - new_range * ratio).max(1.0);
                        st.view.freq_max_hz = st.view.freq_min_hz + new_range;
                    } else {
                        // Wheel: zoom time axis
                        let focus_t = mx as f64 / w.w() as f64;
                        let focus_time = st.view.x_to_time(focus_t);

                        let zoom_factor = if zoom_in { 1.0 / 1.2 } else { 1.2 };
                        let range = st.view.visible_time_range();
                        let new_range = (range * zoom_factor).clamp(
                            0.001,
                            st.view.data_time_max_sec - st.view.data_time_min_sec
                        );

                        let ratio = focus_t;
                        st.view.time_min_sec = (focus_time - new_range * ratio).max(st.view.data_time_min_sec);
                        st.view.time_max_sec = st.view.time_min_sec + new_range;
                        if st.view.time_max_sec > st.view.data_time_max_sec {
                            st.view.time_max_sec = st.view.data_time_max_sec;
                            st.view.time_min_sec = (st.view.time_max_sec - new_range).max(st.view.data_time_min_sec);
                        }
                    }

                    st.spec_renderer.invalidate();
                    st.wave_renderer.invalidate();
                    drop(st);
                    spec_display_c.redraw();
                    waveform_display_c.redraw();
                    true
                }
                Event::Drag => {
                    // Middle button drag to pan (or any drag for seeking)
                    let mx = app::event_x() - w.x();
                    let t = mx as f64 / w.w() as f64;
                    let st = state.borrow();
                    let time = st.view.x_to_time(t);
                    st.audio_player.seek_to(time);
                    true
                }
                _ => false,
            }
        });
    }

    // ── Waveform display ──
    {
        let state = state.clone();

        waveform_display.draw(move |w| {
            if !w.visible_r() || w.w() <= 0 || w.h() <= 0 {
                return;
            }

            let Ok(mut st) = state.try_borrow_mut() else { return; };

            // Compute cursor position
            let cursor_x = if st.transport.duration_seconds > 0.0 {
                let t = st.view.time_to_x(st.transport.position_seconds);
                if t >= 0.0 && t <= 1.0 {
                    Some((t * w.w() as f64) as i32)
                } else {
                    None
                }
            } else {
                None
            };

            // Clone peaks to avoid simultaneous mutable/immutable borrow of st
            let peaks = st.waveform_peaks.clone();
            st.wave_renderer.draw(&peaks, cursor_x, w.x(), w.y(), w.w(), w.h());
        });
    }

    // ── Frequency axis labels ──
    {
        let state = state.clone();

        freq_axis.draw(move |w| {
            if !w.visible_r() || w.w() <= 0 || w.h() <= 0 {
                return;
            }

            fltk::draw::set_draw_color(theme::color(theme::BG_DARK));
            fltk::draw::draw_rectf(w.x(), w.y(), w.w(), w.h());

            let Ok(st) = state.try_borrow() else { return; };
            if st.spectrogram.is_none() { return; }

            fltk::draw::set_draw_color(theme::color(theme::TEXT_SECONDARY));
            fltk::draw::set_font(Font::Helvetica, 9);

            // Generate frequency labels
            let labels: Vec<(f32, &str)> = match st.view.freq_scale {
                FreqScale::Log => vec![
                    (20.0, "20"), (50.0, "50"), (100.0, "100"),
                    (200.0, "200"), (500.0, "500"), (1000.0, "1k"),
                    (2000.0, "2k"), (5000.0, "5k"), (10000.0, "10k"),
                    (20000.0, "20k"),
                ],
                FreqScale::Linear => {
                    let range = st.view.visible_freq_range();
                    let step = if range > 10000.0 { 5000.0 }
                              else if range > 5000.0 { 2000.0 }
                              else if range > 2000.0 { 1000.0 }
                              else if range > 500.0 { 200.0 }
                              else { 100.0 };

                    let mut labels = Vec::new();
                    let mut f = (st.view.freq_min_hz / step).ceil() * step;
                    while f <= st.view.freq_max_hz {
                        labels.push(f);
                        f += step;
                    }
                    // We need static strings, so just format at draw time
                    // (use a different approach)
                    let _ = labels;
                    vec![] // Will handle below
                }
            };

            if !labels.is_empty() {
                for (freq, label) in &labels {
                    if *freq < st.view.freq_min_hz || *freq > st.view.freq_max_hz {
                        continue;
                    }
                    let t = st.view.freq_to_y(*freq);
                    let py = w.y() + w.h() - (t * w.h() as f32) as i32;
                    fltk::draw::draw_text(label, w.x() + 2, py + 3);

                    // Tick mark
                    fltk::draw::set_draw_color(theme::color(theme::BORDER));
                    fltk::draw::draw_line(w.x() + w.w() - 4, py, w.x() + w.w(), py);
                    fltk::draw::set_draw_color(theme::color(theme::TEXT_SECONDARY));
                }
            } else {
                // Linear mode: format numbers dynamically
                let range = st.view.visible_freq_range();
                let step = if range > 10000.0 { 5000.0 }
                          else if range > 5000.0 { 2000.0 }
                          else if range > 2000.0 { 1000.0 }
                          else if range > 500.0 { 200.0 }
                          else { 100.0 };

                let mut f = (st.view.freq_min_hz / step).ceil() * step;
                while f <= st.view.freq_max_hz {
                    let t = st.view.freq_to_y(f);
                    let py = w.y() + w.h() - (t * w.h() as f32) as i32;
                    let label = if f >= 1000.0 {
                        format!("{}k", (f / 1000.0) as i32)
                    } else {
                        format!("{}", f as i32)
                    };
                    fltk::draw::draw_text(&label, w.x() + 2, py + 3);

                    fltk::draw::set_draw_color(theme::color(theme::BORDER));
                    fltk::draw::draw_line(w.x() + w.w() - 4, py, w.x() + w.w(), py);
                    fltk::draw::set_draw_color(theme::color(theme::TEXT_SECONDARY));

                    f += step;
                }
            }
        });
    }

    // ── Time axis labels ──
    {
        let state = state.clone();

        time_axis.draw(move |w| {
            if !w.visible_r() || w.w() <= 0 || w.h() <= 0 {
                return;
            }

            fltk::draw::set_draw_color(theme::color(theme::BG_DARK));
            fltk::draw::draw_rectf(w.x(), w.y(), w.w(), w.h());

            let Ok(st) = state.try_borrow() else { return; };
            if st.spectrogram.is_none() { return; }

            fltk::draw::set_draw_color(theme::color(theme::TEXT_SECONDARY));
            fltk::draw::set_font(Font::Helvetica, 9);

            let range = st.view.visible_time_range();
            let step = if range > 60.0 { 10.0 }
                      else if range > 30.0 { 5.0 }
                      else if range > 10.0 { 2.0 }
                      else if range > 5.0 { 1.0 }
                      else if range > 2.0 { 0.5 }
                      else if range > 1.0 { 0.2 }
                      else { 0.1 };

            let mut t = (st.view.time_min_sec / step).ceil() * step;
            while t <= st.view.time_max_sec {
                let x_norm = st.view.time_to_x(t);
                let px = w.x() + 50 + ((x_norm * (w.w() - 50) as f64) as i32);  // offset for freq axis
                let label = format_time(t);
                fltk::draw::draw_text(&label, px - 15, w.y() + 14);

                // Tick mark
                fltk::draw::set_draw_color(theme::color(theme::BORDER));
                fltk::draw::draw_line(px, w.y(), px, w.y() + 4);
                fltk::draw::set_draw_color(theme::color(theme::TEXT_SECONDARY));

                t += step;
            }
        });
    }

    // ═══════════════════════════════════════════════════════════════════════════
    //  SCROLLBAR CALLBACKS
    // ═══════════════════════════════════════════════════════════════════════════

    // X scrollbar: controls time panning
    // When locked: shifts start/stop equally (viewport = processing window)
    // When unlocked: moves viewport freely over full data range
    {
        let state = state.clone();
        let mut spec_display = spec_display.clone();
        let mut waveform_display = waveform_display.clone();
        let mut input_start = input_start.clone();
        let mut input_stop = input_stop.clone();

        x_scroll.set_minimum(0.0);
        x_scroll.set_maximum(1.0);
        x_scroll.set_slider_size(1.0);
        x_scroll.set_value(0.0);

        x_scroll.set_callback(move |s| {
            let Ok(mut st) = state.try_borrow_mut() else { return; };
            let data_range = st.view.data_time_max_sec - st.view.data_time_min_sec;
            if data_range <= 0.0 { return; }

            let vis_range = st.view.visible_time_range().max(0.001);
            let scroll_pos = s.value();
            let start = st.view.data_time_min_sec + scroll_pos * (data_range - vis_range).max(0.0);
            st.view.time_min_sec = start.max(st.view.data_time_min_sec);
            st.view.time_max_sec = (start + vis_range).min(st.view.data_time_max_sec);

            // When locked, also shift the processing time fields
            if st.view.lock_time {
                match st.fft_params.time_unit {
                    TimeUnit::Seconds => {
                        st.fft_params.start_time = st.view.time_min_sec;
                        st.fft_params.stop_time = st.view.time_max_sec;
                        input_start.set_value(&format!("{:.5}", st.view.time_min_sec));
                        input_stop.set_value(&format!("{:.5}", st.view.time_max_sec));
                    }
                    TimeUnit::Samples => {
                        let sr = st.fft_params.sample_rate as f64;
                        st.fft_params.start_time = (st.view.time_min_sec * sr).round();
                        st.fft_params.stop_time = (st.view.time_max_sec * sr).round();
                        input_start.set_value(&format!("{}", st.fft_params.start_time as u64));
                        input_stop.set_value(&format!("{}", st.fft_params.stop_time as u64));
                    }
                }
            }

            st.spec_renderer.invalidate();
            st.wave_renderer.invalidate();
            drop(st);
            spec_display.redraw();
            waveform_display.redraw();
        });
    }

    // Y scrollbar: controls frequency panning
    // When locked: shifts recon freq min/max equally
    // When unlocked: moves viewport freely
    {
        let state = state.clone();
        let mut spec_display = spec_display.clone();
        let mut input_recon_freq_min = input_recon_freq_min.clone();
        let mut input_recon_freq_max = input_recon_freq_max.clone();

        y_scroll.set_minimum(0.0);
        y_scroll.set_maximum(1.0);
        y_scroll.set_slider_size(1.0);
        y_scroll.set_value(0.0);

        y_scroll.set_callback(move |s| {
            let Ok(mut st) = state.try_borrow_mut() else { return; };
            let data_max = st.view.data_freq_max_hz;
            let data_min = 1.0_f32;
            let data_range = data_max - data_min;
            if data_range <= 0.0 { return; }

            let vis_range = st.view.visible_freq_range().max(1.0);
            let scroll_pos = 1.0 - s.value() as f32;
            let start = data_min + scroll_pos * (data_range - vis_range).max(0.0);
            st.view.freq_min_hz = start.max(data_min);
            st.view.freq_max_hz = (start + vis_range).min(data_max);

            // When locked, also shift the reconstruction freq range
            if st.view.lock_freq {
                st.view.recon_freq_min_hz = st.view.freq_min_hz;
                st.view.recon_freq_max_hz = st.view.freq_max_hz;
                input_recon_freq_min.set_value(&format!("{:.0}", st.view.freq_min_hz));
                input_recon_freq_max.set_value(&format!("{:.0}", st.view.freq_max_hz));
            }

            st.spec_renderer.invalidate();
            drop(st);
            spec_display.redraw();
        });
    }

    // ═══════════════════════════════════════════════════════════════════════════
    //  SPACEBAR HANDLER (window-level KeyUp)
    // ═══════════════════════════════════════════════════════════════════════════

    // Spacebar detection on the window using KeyUp.
    // Uses KeyUp (not KeyDown) because on some platforms KeyDown fires repeatedly
    // while held, but KeyUp fires exactly once per press.
    // Returns false so the event continues propagating to child widgets normally.
    // FloatInput only accepts digits/minus/decimal so space won't enter text fields.
    // When a native OS file dialog is open, the window loses focus and this
    // handler won't fire — no special handling needed for file choosers.
    {
        let mut btn_rerun = btn_rerun.clone();
        win.handle(move |_, event| {
            if event == Event::KeyUp && app::event_key() == Key::from_char(' ') {
                println!("Space bar press detected");
                btn_rerun.do_callback();
            }
            false
        });
    }

    // ═══════════════════════════════════════════════════════════════════════════
    //  MAIN POLL LOOP (16ms)
    // ═══════════════════════════════════════════════════════════════════════════

    {
        let state = state.clone();
        let mut status_bar = status_bar.clone();
        let mut spec_display = spec_display.clone();
        let mut waveform_display = waveform_display.clone();
        let mut scrub_slider = scrub_slider.clone();
        let mut lbl_time = lbl_time.clone();
        let enable_spec_widgets = enable_spec_widgets.clone();
        let enable_wav_export = enable_wav_export.clone();
        let update_info = update_info.clone();
        let mut x_scroll = x_scroll.clone();
        let mut y_scroll = y_scroll.clone();
        let tx = tx.clone();

        app::add_timeout3(0.016, move |handle| {
            // ── Sync scrollbars with view state ──
            // Extract scroll data first, then update widgets AFTER dropping borrow
            let scroll_data = if let Ok(st) = state.try_borrow() {
                let data_time_range = st.view.data_time_max_sec - st.view.data_time_min_sec;
                let data_freq_range = st.view.data_freq_max_hz - 20.0;

                let x_data = if data_time_range > 0.0 {
                    let vis_time = st.view.visible_time_range();
                    let slider_sz = (vis_time / data_time_range).clamp(0.01, 1.0) as f32;
                    let scroll_range = data_time_range - vis_time;
                    let pos = if scroll_range > 0.0 {
                        (st.view.time_min_sec - st.view.data_time_min_sec) / scroll_range
                    } else { 0.0 };
                    Some((slider_sz, pos.clamp(0.0, 1.0)))
                } else { None };

                let y_data = if data_freq_range > 0.0 {
                    let vis_freq = st.view.visible_freq_range();
                    let slider_sz = (vis_freq / data_freq_range).clamp(0.01, 1.0);
                    let scroll_range = data_freq_range - vis_freq;
                    let pos = if scroll_range > 0.0 {
                        (st.view.freq_min_hz - 20.0) / scroll_range
                    } else { 0.0 };
                    Some((slider_sz, (1.0 - pos).clamp(0.0, 1.0) as f64))
                } else { None };

                Some((x_data, y_data))
            } else { None };
            // Now update scrollbar widgets with borrow dropped
            if let Some((x_data, y_data)) = scroll_data {
                if let Some((sz, pos)) = x_data {
                    x_scroll.set_slider_size(sz);
                    x_scroll.set_value(pos);
                }
                if let Some((sz, pos)) = y_data {
                    y_scroll.set_slider_size(sz);
                    y_scroll.set_value(pos);
                }
            }

            // ── Process worker messages ──
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    WorkerMessage::FftComplete(spectrogram) => {
                        let num_frames = spectrogram.num_frames();

                        // Store spectrogram, compute active mask, then auto-reconstruct
                        let recon_data = {
                            let mut st = state.borrow_mut();

                            st.view.max_freq_bins = st.fft_params.num_frequency_bins();

                            let spec_arc = Arc::new(spectrogram);
                            let (min_t, max_t, max_f) = (spec_arc.min_time, spec_arc.max_time, spec_arc.max_freq);

                            // Build active mask before storing spectrogram
                            let mask = ActiveMask::compute(
                                &spec_arc,
                                st.view.recon_freq_count,
                                st.view.recon_freq_min_hz,
                                st.view.recon_freq_max_hz,
                                min_t,
                                max_t,
                            );
                            st.active_mask = Some(mask);

                            st.spectrogram = Some(spec_arc);

                            // Update data bounds
                            st.view.data_time_min_sec = min_t;
                            st.view.data_time_max_sec = max_t;
                            if max_f > 0.0 {
                                st.view.data_freq_max_hz = max_f;
                            }

                            // Sync viewport based on lock state
                            if st.view.lock_time {
                                st.view.time_min_sec = min_t;
                                st.view.time_max_sec = max_t;
                            }
                            if st.view.lock_freq {
                                let fmin = st.view.recon_freq_min_hz.max(1.0);
                                let fmax = st.view.recon_freq_max_hz.max(fmin + 1.0);
                                st.view.freq_min_hz = fmin;
                                st.view.freq_max_hz = fmax;
                            } else if max_f > 0.0 {
                                st.view.freq_max_hz = max_f;
                            }

                            st.spec_renderer.invalidate();

                            // Prepare reconstruction data
                            let spec = st.spectrogram.clone().unwrap();
                            let params = st.fft_params.clone();
                            let view = st.view.clone();
                            // Keep is_processing = true for reconstruction phase
                            (spec, params, view)
                        };

                        (enable_spec_widgets.borrow_mut())();
                        (update_info.borrow_mut())();
                        status_bar.set_label(&format!(
                            "FFT done ({} frames) | Reconstructing...",
                            num_frames
                        ));
                        spec_display.redraw();

                        // Auto-trigger reconstruction
                        let tx_clone = tx.clone();
                        let (spec, params, view) = recon_data;
                        std::thread::spawn(move || {
                            let reconstructed = Reconstructor::reconstruct(&spec, &params, &view);
                            tx_clone.send(WorkerMessage::ReconstructionComplete(reconstructed)).ok();
                        });
                    }
                    WorkerMessage::ReconstructionComplete(reconstructed) => {
                        let recon_result = {
                            let mut st = state.borrow_mut();
                            match st.audio_player.load_audio(&reconstructed) {
                                Ok(_) => {
                                    let duration = reconstructed.duration_seconds;
                                    let samples = reconstructed.num_samples();
                                    st.transport.duration_seconds = duration;

                                    let peaks = WaveformPeaks::compute(
                                        &reconstructed.samples,
                                        reconstructed.sample_rate,
                                        st.view.time_min_sec,
                                        st.view.time_max_sec,
                                        800,
                                    );
                                    st.waveform_peaks = peaks;
                                    st.wave_renderer.invalidate();

                                    st.reconstructed_audio = Some(reconstructed);
                                    st.is_processing = false;
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
                                waveform_display.redraw();
                            }
                            Err(e) => {
                                status_bar.set_label(&format!("Reconstruction error: {}", e));
                                dialog::alert_default(&format!("Failed to load reconstructed audio:\n{}", e));
                            }
                        }
                    }
                    WorkerMessage::WaveformReady(peaks) => {
                        let mut st = state.borrow_mut();
                        st.waveform_peaks = peaks;
                        st.wave_renderer.invalidate();
                        drop(st);
                        waveform_display.redraw();
                    }
                    WorkerMessage::Error(msg) => {
                        state.borrow_mut().is_processing = false;
                        status_bar.set_label(&format!("Error: {}", msg));
                        dialog::alert_default(&format!("Processing error:\n{}", msg));
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
                    let pos = st.audio_player.get_position_seconds();
                    let playing = st.audio_player.get_state() == PlaybackState::Playing;
                    st.transport.position_seconds = pos;
                    Some((pos, st.transport.duration_seconds, playing))
                } else {
                    None
                }
            };
            if let Some((pos, dur, playing)) = transport_data {
                if dur > 0.0 {
                    scrub_slider.set_value((pos / dur).clamp(0.0, 1.0));
                }
                lbl_time.set_label(&format!(
                    "{} / {}",
                    format_time(pos),
                    format_time(dur)
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
