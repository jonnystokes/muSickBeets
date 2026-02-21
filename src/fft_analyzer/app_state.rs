use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::data::{AudioData, FftParams, Spectrogram, TransportState, ViewState};
use crate::playback::audio_player::AudioPlayer;
use crate::rendering::spectrogram_renderer::SpectrogramRenderer;
use crate::rendering::waveform_renderer::WaveformRenderer;
use crate::ui::tooltips::TooltipManager;

// ─── Messages ──────────────────────────────────────────────────────────────────

pub enum WorkerMessage {
    FftComplete(Spectrogram),
    ReconstructionComplete(AudioData),
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
    pub lock_to_active: bool,
    pub has_audio: bool,
    pub current_filename: String,

    pub tooltip_mgr: TooltipManager,

    // Zoom factors (configurable via INI)
    pub time_zoom_factor: f32,
    pub freq_zoom_factor: f32,
    pub mouse_zoom_factor: f32,

    // Audio normalization settings
    pub normalize_audio: bool,
    pub normalize_peak: f32,
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
            lock_to_active: false,
            has_audio: false,
            current_filename: String::new(),

            tooltip_mgr: TooltipManager::new(),

            time_zoom_factor: 1.5,
            freq_zoom_factor: 1.5,
            mouse_zoom_factor: 1.2,

            normalize_audio: true,
            normalize_peak: 0.97,
        }
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

pub struct SharedCallbacks {
    pub update_info: SharedCb,
    pub update_seg_label: SharedCb,
    pub enable_audio_widgets: SharedCb,
    pub enable_spec_widgets: SharedCb,
    pub enable_wav_export: SharedCb,
}

// ─── Format time as M:SS.ms ───────────────────────────────────────────────────

pub fn format_time(seconds: f64) -> String {
    let mins = (seconds / 60.0) as u32;
    let secs = seconds % 60.0;
    format!("{}:{:08.5}", mins, secs)
}
