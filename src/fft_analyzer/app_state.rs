use std::sync::Arc;
use std::time::{Duration, Instant};
use std::rc::Rc;
use std::cell::RefCell;

use crate::data::{AudioData, FftParams, Spectrogram, ViewState, TransportState};
use crate::rendering::spectrogram_renderer::SpectrogramRenderer;
use crate::rendering::waveform_renderer::WaveformRenderer;
use crate::playback::audio_player::AudioPlayer;
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
    pub recon_start_time: f64,
    pub is_processing: bool,
    pub dirty: bool,
    pub lock_to_active: bool,
    pub has_audio: bool,

    pub tooltip_mgr: TooltipManager,
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
            recon_start_time: 0.0,
            is_processing: false,
            dirty: false,
            lock_to_active: false,
            has_audio: false,

            tooltip_mgr: TooltipManager::new(),
        }
    }

    /// Compute all derived info values from current params
    pub fn derived_info(&self) -> DerivedInfo {
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
}

impl DerivedInfo {
    pub fn format_info(&self) -> String {
        format!(
            "Segments: {}\nSamples: {}\nFreq bins: {}\nFreq res: {:.2} Hz\nHop: {} smp\nBin dur: {:.2} ms\nWindow: {} smp",
            self.segments, self.total_samples, self.freq_bins,
            self.freq_resolution, self.hop_length, self.bin_duration_ms,
            self.window_length
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
    format!("{}:{:05.2}", mins, secs)
}
