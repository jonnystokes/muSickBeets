use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use fltk::{
    app,
    output::MultilineOutput,
    prelude::{InputExt, WidgetExt},
};

use crate::data::{AudioData, FftParams, Spectrogram, TransportState, ViewState};
use crate::playback::audio_player::AudioPlayer;
use crate::rendering::spectrogram_renderer::SpectrogramRenderer;
use crate::rendering::waveform_renderer::WaveformRenderer;
use crate::ui::tooltips::TooltipManager;

// ─── Messages ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FftStage {
    Overview,
    Focus,
}

impl FftStage {
    pub fn label(self) -> &'static str {
        match self {
            FftStage::Overview => "Overview FFT",
            FftStage::Focus => "FFT",
        }
    }

    pub fn activity_text(self) -> &'static str {
        match self {
            FftStage::Overview => "Processing overview FFT...",
            FftStage::Focus => "Processing focused FFT...",
        }
    }
}

pub enum WorkerMessage {
    FftStageComplete(FftStage, Spectrogram),
    ReconstructionComplete(AudioData),
    /// Audio file loaded from disk. Contains (audio, filename, norm_gain).
    AudioLoaded(AudioData, std::path::PathBuf, f32),
    /// WAV export finished. Contains Ok(filename) or Err(message).
    WavSaved(Result<std::path::PathBuf, String>),
    /// CSV export finished. Contains Ok((filename, num_frames, time_min, time_max)) or Err(message).
    CsvSaved(Result<(std::path::PathBuf, usize, f64, f64), String>),
    /// CSV/FFT data loaded from disk. Contains Ok((spectrogram, params, recon_params, view_params, filename))
    /// or Err(message).
    CsvLoaded(
        Result<
            (
                Spectrogram,
                crate::data::FftParams,
                Option<crate::csv_export::ReconParams>,
                crate::csv_export::ImportedViewParams,
                std::path::PathBuf,
            ),
            String,
        >,
    ),
    /// Worker thread panicked. Contains the panic message for logging.
    WorkerPanic(String),
    /// Worker was cancelled via the cancel flag. Contains a description of what was cancelled.
    Cancelled(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseMode {
    Time,
    Move,
    SelectZoom,
    RoiSelect,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseSurface {
    Spectrogram,
    Waveform,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MouseSelection {
    pub surface: MouseSurface,
    pub start_x: i32,
    pub start_y: i32,
    pub current_x: i32,
    pub current_y: i32,
}

// ─── Status Bar Manager ────────────────────────────────────────────────────────
//
// Single system managing all status bar writes.
//
// Layout: `Activity  |  Timing1  |  Timing2  |  ...  |  Memory: X MB`
//
// - First slot: current activity (what the program is doing NOW)
// - Middle slots: timed items list (FILO -- most recent first, never deleted,
//   only replaced when a new timing for the same key arrives)
// - Last slot: memory usage (always present, always last)
// - Optional progress text appended to the activity slot

/// A single timed entry in the status bar (e.g. "FFT load 4.7s").
#[derive(Clone, Debug)]
#[allow(dead_code)]
struct TimedEntry {
    /// Unique key for this category (e.g. "FFT", "Reconstruction", "CSV load").
    key: String,
    /// How long the operation took.
    duration: Duration,
    /// When this entry was recorded (for FILO ordering -- most recent first).
    recorded_at: Instant,
}

/// Unified manager for the bottom status bar.
///
/// All status bar writes go through this struct. Call `render()` to get the
/// final display string.
pub struct StatusBarManager {
    /// Current activity text (first slot): "Ready", "Loading audio...", etc.
    activity: String,
    /// Optional progress indicator appended to activity: " (42%)", " (1200/3000 frames)".
    progress: Option<String>,
    /// Timed entries (middle slots). Ordered by most-recently-recorded first.
    /// Items are never removed, only replaced when the same key is recorded again.
    timings: VecDeque<TimedEntry>,
    /// Optional start time for the currently running operation.
    /// Used to compute elapsed time for in-progress operations.
    operation_start: Option<(String, Instant)>,
}

impl StatusBarManager {
    pub fn new() -> Self {
        Self {
            activity: "Ready | Load an audio file to begin".to_string(),
            progress: None,
            timings: VecDeque::new(),
            operation_start: None,
        }
    }

    /// Set the current activity text (first slot).
    /// Clears any progress indicator.
    pub fn set_activity(&mut self, text: &str) {
        self.activity = text.to_string();
        self.progress = None;
    }

    /// Set an optional progress indicator that is appended to the activity slot.
    /// Pass `None` to clear. Examples: `Some("42%")`, `Some("1200/3000 frames")`.
    #[allow(dead_code)]
    pub fn set_progress(&mut self, text: Option<&str>) {
        self.progress = text.map(|s| s.to_string());
    }

    /// Record a timed operation. If `key` already exists, it is replaced.
    /// New/replaced entries go to the front (most recent first -- FILO).
    pub fn record_timing(&mut self, key: &str, duration: Duration) {
        // Remove existing entry with same key
        self.timings.retain(|e| e.key != key);
        // Insert at front (most recent first)
        self.timings.push_front(TimedEntry {
            key: key.to_string(),
            duration,
            recorded_at: Instant::now(),
        });
    }

    /// Start timing an operation. Call `finish_timing()` when it completes.
    /// This is a convenience wrapper -- you can also just call `record_timing()`
    /// directly if you manage your own `Instant`.
    pub fn start_timing(&mut self, key: &str) {
        self.operation_start = Some((key.to_string(), Instant::now()));
    }

    /// Finish the currently timed operation and record its duration.
    /// Returns the duration if an operation was in progress, None otherwise.
    pub fn finish_timing(&mut self) -> Option<Duration> {
        if let Some((key, start)) = self.operation_start.take() {
            let duration = start.elapsed();
            self.record_timing(&key, duration);
            Some(duration)
        } else {
            None
        }
    }

    /// Clear the currently timed operation without recording it.
    /// Used when an operation is cancelled or errors out.
    pub fn cancel_timing(&mut self) {
        self.operation_start = None;
    }

    /// Clear all timed entries. Used on full reset (e.g. new file load).
    pub fn clear_timings(&mut self) {
        self.timings.clear();
        self.operation_start = None;
    }

    /// Build the full status bar text.
    ///
    /// Format: `Activity (progress)  |  Timing1  |  Timing2  |  Memory: X MB`
    pub fn render(&self) -> String {
        let mem = format_memory_usage();

        // Build activity slot with optional progress
        let activity = match &self.progress {
            Some(p) => format!("{} ({})", self.activity, p),
            None => self.activity.clone(),
        };

        let mut parts = vec![activity];

        // Add timed entries (already in FILO order -- most recent first)
        for entry in &self.timings {
            let secs = entry.duration.as_secs_f64();
            if secs >= 60.0 {
                let mins = secs as u32 / 60;
                let remaining = secs % 60.0;
                parts.push(format!("{}: {}m {:.1}s", entry.key, mins, remaining));
            } else {
                parts.push(format!("{}: {:.2}s", entry.key, secs));
            }
        }

        parts.push(format!("Memory: {}", mem));
        parts.join("  |  ")
    }

    /// Build the status bar text, inserting line breaks at `|` boundaries
    /// when a line would exceed `max_chars` characters. Returns the wrapped
    /// text ready for a MultilineOutput widget.
    pub fn render_wrapped(&self, max_chars: usize) -> String {
        let mem = format_memory_usage();

        // Build activity slot with optional progress
        let activity = match &self.progress {
            Some(p) => format!("{} ({})", self.activity, p),
            None => self.activity.clone(),
        };

        let sep = "  |  ";
        let mut parts: Vec<String> = vec![activity];

        for entry in &self.timings {
            let secs = entry.duration.as_secs_f64();
            if secs >= 60.0 {
                let mins = secs as u32 / 60;
                let remaining = secs % 60.0;
                parts.push(format!("{}: {}m {:.1}s", entry.key, mins, remaining));
            } else {
                parts.push(format!("{}: {:.2}s", entry.key, secs));
            }
        }
        parts.push(format!("Memory: {}", mem));

        // Join parts, wrapping to new lines at `|` boundaries
        let max = max_chars.max(20);
        let mut lines: Vec<String> = Vec::new();
        let mut current_line = String::new();

        for (i, part) in parts.iter().enumerate() {
            let candidate = if current_line.is_empty() {
                part.clone()
            } else {
                format!("{}{}{}", current_line, sep, part)
            };

            if candidate.len() > max && !current_line.is_empty() {
                // Current line is full -- push it and start new line with this part
                lines.push(current_line);
                current_line = part.clone();
            } else {
                current_line = candidate;
            }

            // Last part -- always push
            if i == parts.len() - 1 {
                lines.push(current_line.clone());
            }
        }

        lines.join("\n")
    }

    /// Estimate the pixel height needed for the status bar given a window
    /// width in pixels. Uses 7px per character estimate (same as status_fft).
    /// Returns pixel height (minimum 25px for single line).
    pub fn measure_height(&self, win_width: i32) -> i32 {
        let max_chars = ((win_width - 16).max(40) / 7).max(20) as usize;
        let text = self.render_wrapped(max_chars);
        let line_count = text.lines().count().max(1) as i32;
        // 17px per line + 8px padding, minimum 25px (single line)
        (line_count * 17 + 8).max(25)
    }
}

// ─── App State ─────────────────────────────────────────────────────────────────

pub struct AppState {
    pub audio_data: Option<Arc<AudioData>>,
    pub spectrogram: Option<Arc<Spectrogram>>,
    #[allow(dead_code)]
    pub overview_spectrogram: Option<Arc<Spectrogram>>,
    #[allow(dead_code)]
    pub focus_spectrogram: Option<Arc<Spectrogram>>,
    pub overview_spec_params: Option<FftParams>,
    pub focus_spec_params: Option<FftParams>,
    pub fft_params: FftParams,
    pub overview_fft_defaults: FftParams,
    pub view: ViewState,
    pub transport: TransportState,

    pub audio_player: AudioPlayer,
    pub spec_renderer: SpectrogramRenderer,
    #[allow(dead_code)]
    pub overview_spec_renderer: SpectrogramRenderer,
    #[allow(dead_code)]
    pub focus_spec_renderer: SpectrogramRenderer,
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
    pub render_full_file_outside_roi: bool,
    pub has_audio: bool,
    pub current_filename: String,
    pub mouse_mode: MouseMode,
    pub mouse_selection: Option<MouseSelection>,

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

    /// Unified status bar manager. All status bar writes go through this.
    pub status: StatusBarManager,

    /// Progress counter for in-flight operations. Shared with worker threads
    /// via `Arc`. The UI poll loop reads this to update the status bar progress.
    /// Reset to 0 at the start of each operation.
    pub progress_counter: Arc<AtomicUsize>,
    /// Total number of items for the current operation (frames, rows, etc.).
    /// Used to compute percentage: `progress_counter / progress_total * 100`.
    pub progress_total: usize,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            audio_data: None,
            spectrogram: None,
            overview_spectrogram: None,
            focus_spectrogram: None,
            overview_spec_params: None,
            focus_spec_params: None,
            fft_params: FftParams::default(),
            overview_fft_defaults: FftParams::default(),
            view: ViewState::default(),
            transport: TransportState::default(),

            audio_player: AudioPlayer::new(),
            spec_renderer: SpectrogramRenderer::new(),
            overview_spec_renderer: SpectrogramRenderer::new(),
            focus_spec_renderer: SpectrogramRenderer::new(),
            wave_renderer: WaveformRenderer::new(),

            reconstructed_audio: None,
            recon_start_sample: 0,
            is_processing: false,
            dirty: false,
            play_pending: false,
            lock_to_active: false,
            render_full_file_outside_roi: true,
            has_audio: false,
            current_filename: String::new(),
            mouse_mode: MouseMode::Time,
            mouse_selection: None,

            tooltip_mgr: TooltipManager::new(),

            time_zoom_factor: 1.5,
            freq_zoom_factor: 1.5,
            mouse_zoom_factor: 1.2,
            swap_zoom_axes: false,

            normalize_audio: true,
            normalize_peak: 0.97,
            source_norm_gain: 1.0,
            cancel_flag: Arc::new(AtomicBool::new(false)),
            status: StatusBarManager::new(),
            progress_counter: Arc::new(AtomicUsize::new(0)),
            progress_total: 0,
        }
    }

    /// Spectrogram currently used by legacy single-layer code paths.
    /// Prefer `focus_spectrogram` when present, otherwise fall back to overview.
    #[allow(dead_code)]
    pub fn active_spectrogram(&self) -> Option<Arc<Spectrogram>> {
        self.focus_spectrogram
            .clone()
            .or_else(|| self.overview_spectrogram.clone())
            .or_else(|| self.spectrogram.clone())
    }

    /// Invalidate all spectrogram renderers.
    /// Useful during the transition from one-layer to two-layer rendering.
    #[allow(dead_code)]
    pub fn invalidate_all_spectrogram_renderers(&mut self) {
        self.spec_renderer.invalidate();
        self.overview_spec_renderer.invalidate();
        self.focus_spec_renderer.invalidate();
    }

    pub fn overview_params_for_audio(&self, total_samples: usize) -> FftParams {
        let mut params = self.overview_fft_defaults.clone();
        params.sample_rate = self.fft_params.sample_rate;
        params.time_unit = self.fft_params.time_unit;
        params.start_sample = 0;
        params.stop_sample = total_samples;
        params
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

#[derive(Clone)]
pub struct SharedCallbacks {
    pub update_info: SharedCb,
    pub update_seg_label: SharedCb,
    pub enable_audio_widgets: SharedCb,
    pub enable_spec_widgets: SharedCb,
    pub enable_wav_export: SharedCb,
    /// Disable sidebar/transport widgets during processing.
    /// The rerun button gets special treatment via `set_btn_cancel_mode` / `set_btn_busy_mode`.
    pub disable_for_processing: SharedCb,
    /// Re-enable sidebar/transport widgets after processing completes.
    pub enable_after_processing: SharedCb,
    /// Set the rerun button to "Cancel" mode (red, active, triggers cancellation).
    pub set_btn_cancel_mode: SharedCb,
    /// Set the rerun button to "Busy..." mode (gray, inactive).
    pub set_btn_busy_mode: SharedCb,
    /// Restore the rerun button to normal "Recompute + Rebuild" mode.
    pub set_btn_normal_mode: SharedCb,
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
pub fn update_status_bar(status_bar: &mut MultilineOutput, text: &str) {
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
