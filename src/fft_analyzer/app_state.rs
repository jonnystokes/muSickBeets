use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use fltk::{
    app,
    output::Output,
    prelude::{InputExt, WidgetExt},
};

use crate::data::{AudioData, FftParams, Spectrogram, TransportState, ViewState};
use crate::playback::audio_player::AudioPlayer;
use crate::rendering::spectrogram_renderer::SpectrogramRenderer;
use crate::rendering::waveform_renderer::WaveformRenderer;
use crate::ui::tooltips::TooltipManager;

// ─── Messages ──────────────────────────────────────────────────────────────────

pub enum WorkerMessage {
    FftComplete(Spectrogram),
    ReconstructionComplete(AudioData),
    /// Audio file loaded from disk. Contains (audio, filename, norm_gain).
    AudioLoaded(AudioData, std::path::PathBuf, f32),
    /// WAV export finished. Contains Ok(filename) or Err(message).
    WavSaved(Result<std::path::PathBuf, String>),
    /// CSV export finished. Contains Ok((filename, num_frames, time_min, time_max)) or Err(message).
    CsvSaved(Result<(std::path::PathBuf, usize, f64, f64), String>),
    /// Worker thread panicked. Contains the panic message for logging.
    WorkerPanic(String),
    /// Worker was cancelled via the cancel flag. Contains a description of what was cancelled.
    Cancelled(String),
}

// ─── App State ─────────────────────────────────────────────────────────────────

pub struct AppState {
    pub audio_data: Option<Arc<AudioData>>,
    pub spectrogram: Option<Arc<Spectrogram>>,
    pub fft_params: FftParams,
    pub view: ViewState,
    pub transport: TransportState,

    pub audio_player: AudioPlayer,
    pub spec_renderer: SpectrogramRenderer,
    pub wave_renderer: WaveformRenderer,

    pub reconstructed_audio: Option<AudioData>,
    /// Reconstruction start position in samples (ground truth).
    pub recon_start_sample: usize,
    pub is_processing: bool,
    pub dirty: bool,
    /// When true, auto-start playback after the next reconstruction completes.
    /// Set by the Play button when it triggers a recompute due to dirty state.
    pub play_pending: bool,
    pub lock_to_active: bool,
    pub has_audio: bool,
    pub current_filename: String,

    pub tooltip_mgr: TooltipManager,

    // Zoom factors (configurable via INI)
    pub time_zoom_factor: f32,
    pub freq_zoom_factor: f32,
    pub mouse_zoom_factor: f32,

    // When true: Alt+scroll zooms time, Alt+Ctrl+scroll zooms frequency
    // When false (default): Alt+scroll zooms frequency, Alt+Ctrl+scroll zooms time
    pub swap_zoom_axes: bool,

    // Audio normalization settings
    pub normalize_audio: bool,
    pub normalize_peak: f32,

    /// Gain factor applied during source audio normalization (1.0 = no change).
    /// Stored so the original peak level can be recovered: original = normalized / gain.
    pub source_norm_gain: f32,

    /// Cancellation flag for in-flight FFT/reconstruction workers.
    /// Set to `true` to request cancellation; workers check this periodically.
    /// A new `Arc` is created for each operation so stale cancellations don't
    /// affect newly-launched workers.
    pub cancel_flag: Arc<AtomicBool>,

    // ── Timing for status bar ──
    /// When the current FFT processing started.
    pub fft_start_time: Option<Instant>,
    /// Duration of the last completed FFT pass.
    pub last_fft_duration: Option<Duration>,
    /// When the current reconstruction started.
    pub recon_start_time: Option<Instant>,
    /// Duration of the last completed reconstruction pass.
    pub last_recon_duration: Option<Duration>,
    /// What the worker is currently doing (for status bar display).
    /// Dynamic string so it can include filenames, frame counts, etc.
    pub current_activity: String,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            audio_data: None,
            spectrogram: None,
            fft_params: FftParams::default(),
            view: ViewState::default(),
            transport: TransportState::default(),

            audio_player: AudioPlayer::new(),
            spec_renderer: SpectrogramRenderer::new(),
            wave_renderer: WaveformRenderer::new(),

            reconstructed_audio: None,
            recon_start_sample: 0,
            is_processing: false,
            dirty: false,
            play_pending: false,
            lock_to_active: false,
            has_audio: false,
            current_filename: String::new(),

            tooltip_mgr: TooltipManager::new(),

            time_zoom_factor: 1.5,
            freq_zoom_factor: 1.5,
            mouse_zoom_factor: 1.2,
            swap_zoom_axes: false,

            normalize_audio: true,
            normalize_peak: 0.97,
            source_norm_gain: 1.0,
            cancel_flag: Arc::new(AtomicBool::new(false)),
            fft_start_time: None,
            last_fft_duration: None,
            recon_start_time: None,
            last_recon_duration: None,
            current_activity: "Ready | Load an audio file to begin".to_string(),
        }
    }

    /// Cancel any in-flight worker, then create a fresh cancellation flag
    /// for the next operation. Returns the new flag (already stored in self).
    pub fn new_cancel_flag(&mut self) -> Arc<AtomicBool> {
        // Signal old workers to stop
        self.cancel_flag.store(true, Ordering::Relaxed);
        // Create a fresh flag for the new operation
        let flag = Arc::new(AtomicBool::new(false));
        self.cancel_flag = flag.clone();
        flag
    }

    /// Reconstruction start time in seconds, derived from sample count.
    pub fn recon_start_seconds(&self) -> f64 {
        self.recon_start_sample as f64 / self.fft_params.sample_rate.max(1) as f64
    }

    /// Build the status bar string showing current activity + timing + memory.
    ///
    /// Format when idle:
    ///   `Ready | FFT: 32.64s | Reconstruction: 4.78s | Memory: 1.2 GB`
    /// Format when processing:
    ///   `Processing FFT... | Memory: 1.2 GB`
    pub fn status_bar_text(&self) -> String {
        let mem = format_memory_usage();
        let activity = &self.current_activity;

        let mut parts = vec![activity.to_string()];

        // Show timing info
        if let Some(d) = self.last_fft_duration {
            parts.push(format!("FFT: {:.2}s", d.as_secs_f64()));
        }
        if let Some(d) = self.last_recon_duration {
            parts.push(format!("Reconstruction: {:.2}s", d.as_secs_f64()));
        }

        parts.push(format!("Memory: {}", mem));
        parts.join("  |  ")
    }

    /// Compute all derived info values from current params
    pub fn derived_info(&self) -> DerivedInfo {
        let total_samples = if let Some(ref audio) = self.audio_data {
            let start = self.fft_params.start_sample.min(audio.num_samples());
            let stop = self.fft_params.stop_sample.min(audio.num_samples());
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
            overlap_percent: self.fft_params.overlap_percent,
        }
    }
}

pub struct DerivedInfo {
    pub total_samples: usize,
    pub freq_bins: usize,
    pub freq_resolution: f32,
    pub hop_length: usize,
    pub segments: usize,
    pub bin_duration_ms: f64,
    pub window_length: usize,
    pub sample_rate: u32,
    pub overlap_percent: f32,
}

impl DerivedInfo {
    pub fn format_info(&self) -> String {
        format!(
            "Segments: {} x {} smp\n\
             Total samples: {}\n\
             Freq bins: {} / segment\n\
             Freq res: {:.2} Hz/bin\n\
             Time res: {:.2} ms/frame\n\
             Hop: {} smp ({:.1}ms)",
            self.segments,
            self.window_length,
            self.total_samples,
            self.freq_bins,
            self.freq_resolution,
            self.bin_duration_ms,
            self.hop_length,
            self.hop_length as f64 / self.sample_rate.max(1) as f64 * 1000.0,
        )
    }

    pub fn format_segmentation_sentence(&self) -> String {
        let sample_rate = self.sample_rate.max(1) as f64;
        let active_seconds = self.total_samples as f64 / sample_rate;
        let segment_seconds = self.window_length as f64 / sample_rate;
        let hop_seconds = self.hop_length as f64 / sample_rate;
        format!(
            "Active time range is {} samples ({:.5} s at {:.1} kHz), divided into {} overlapping segments, each of length {} samples ({:.5} s). With {:.0}% overlap, the hop distance between segment starts is {} samples ({:.5} s). Each segment yields {} frequency bins, with each bin covering {:.5} Hz – the frequency resolution per bin.",
            self.total_samples,
            active_seconds,
            sample_rate / 1000.0,
            self.segments,
            self.window_length,
            segment_seconds,
            self.overlap_percent.round(),
            self.hop_length,
            hop_seconds,
            self.freq_bins,
            self.freq_resolution,
        )
    }

    pub fn format_resolution(&self) -> String {
        let window_ms = self.window_length as f64 / self.sample_rate.max(1) as f64 * 1000.0;
        let hop_ms = self.hop_length as f64 / self.sample_rate.max(1) as f64 * 1000.0;
        format!(
            "Window: {} smp ({:.1} ms)\n\
             Freq res: {:.2} Hz/bin ({} bins)\n\
             Time res: {:.1} ms/frame ({} frames)\n\
             Hop: {} smp ({:.1} ms)",
            self.window_length,
            window_ms,
            self.freq_resolution,
            self.freq_bins,
            self.bin_duration_ms,
            self.segments,
            self.hop_length,
            hop_ms,
        )
    }
}

/// Throttle helper to prevent excessive redraws
pub struct UpdateThrottle {
    last_update: Instant,
    min_interval: Duration,
}

impl UpdateThrottle {
    pub fn new(min_interval_ms: u64) -> Self {
        Self {
            last_update: Instant::now() - Duration::from_millis(min_interval_ms + 1),
            min_interval: Duration::from_millis(min_interval_ms),
        }
    }

    pub fn should_update(&mut self) -> bool {
        let now = Instant::now();
        if now.duration_since(self.last_update) >= self.min_interval {
            self.last_update = now;
            true
        } else {
            false
        }
    }
}

// ─── Shared callback type ──────────────────────────────────────────────────────

pub type SharedCb = Rc<RefCell<Box<dyn FnMut()>>>;

pub struct SharedCallbacks {
    pub update_info: SharedCb,
    pub update_seg_label: SharedCb,
    pub enable_audio_widgets: SharedCb,
    pub enable_spec_widgets: SharedCb,
    pub enable_wav_export: SharedCb,
}

// ─── Message bar helper ───────────────────────────────────────────────────────
//
// Color-coded transient messages in the top message bar (right of menu).
// Use instead of status_bar for warnings, errors, and parameter-change notices.

#[derive(Clone, Copy)]
#[allow(dead_code)]
pub enum MsgLevel {
    Info,    // neutral notice — dimmed text
    Warning, // yellow — something was adjusted or unexpected
    Error,   // red — something failed
}

/// Set a color-coded message on the top message bar.
/// Call with an empty string to clear.
pub fn set_msg(bar: &mut fltk::frame::Frame, level: MsgLevel, text: &str) {
    use crate::ui::theme;
    let color = match level {
        MsgLevel::Info => theme::TEXT_SECONDARY,
        MsgLevel::Warning => theme::ACCENT_YELLOW,
        MsgLevel::Error => theme::ACCENT_RED,
    };
    bar.set_label_color(fltk::enums::Color::from_hex(color));
    if text.is_empty() {
        bar.set_label("");
    } else {
        let prefix = match level {
            MsgLevel::Info => "",
            MsgLevel::Warning => " Warning: ",
            MsgLevel::Error => " Error: ",
        };
        bar.set_label(&format!("{}{}", prefix, text));
    }
    bar.redraw();
}

/// Update the bottom status bar immediately and flush the UI so the change is
/// visible even if a long-running task begins right after.
pub fn update_status_bar(status_bar: &mut Output, text: &str) {
    status_bar.set_value(text);
    status_bar.redraw();
    app::flush();
}

// ─── Format time as M:SS.ms ───────────────────────────────────────────────────

pub fn format_time(seconds: f64) -> String {
    let mins = (seconds / 60.0) as u32;
    let secs = seconds % 60.0;
    format!("{}:{:08.5}", mins, secs)
}

// ─── Memory usage ─────────────────────────────────────────────────────────────

/// Read current process RSS (resident set size) from /proc/self/status.
/// Returns a human-readable string like "1.23 GB" or "456 MB".
/// Falls back to "N/A" if /proc is unavailable.
pub fn format_memory_usage() -> String {
    get_rss_kb().map_or_else(
        || "N/A".to_string(),
        |kb| {
            let mb = kb as f64 / 1024.0;
            if mb >= 1024.0 {
                format!("{:.2} GB", mb / 1024.0)
            } else {
                format!("{:.0} MB", mb)
            }
        },
    )
}

/// Read VmRSS from /proc/self/status in kB. Returns None if unavailable.
fn get_rss_kb() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("VmRSS:") {
            // Format: "    1234 kB"
            let trimmed = rest.trim().trim_end_matches(" kB").trim();
            return trimmed.parse().ok();
        }
    }
    None
}
