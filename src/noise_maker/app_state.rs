use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::audio_gene::AudioGeneProject;
use crate::frame_file::FrameFile;
use crate::synth::SynthPlayer;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EngineKind {
    OscBank,
    FrameOla,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WindowKind {
    Rectangular,
    Hann,
    Hamming,
    Blackman,
    Kaiser,
}

impl WindowKind {
    pub fn label(self) -> &'static str {
        match self {
            WindowKind::Rectangular => "Rectangular",
            WindowKind::Hann => "Hann",
            WindowKind::Hamming => "Hamming",
            WindowKind::Blackman => "Blackman",
            WindowKind::Kaiser => "Kaiser",
        }
    }
}

/// View/engine state restored when loading an audio_gene project.
#[derive(Clone, Copy, Debug)]
pub struct SavedViewState {
    pub engine_kind: EngineKind,
    pub synth_window: WindowKind,
    pub overlap_percent: f32,
    pub wave_time_offset_sec: f64,
    pub wave_time_span_sec: f64,
    pub wave_amp_visual_gain: f32,
    pub spec_freq_min_hz: f32,
    pub spec_freq_max_hz: f32,
    pub audio_gain_user: f32,
    pub audio_gain_auto: f32,
}

#[derive(Clone)]
pub enum LoadedDocument {
    Frame(FrameFile, PathBuf),
    AudioGene(AudioGeneProject, PathBuf),
}

#[derive(Clone)]
pub struct PreparedDocument {
    pub document: LoadedDocument,
    pub preview_samples: Vec<f32>,
    pub preview_sample_rate: u32,
    pub suggested_auto_gain: Option<f32>,
    pub playback_loop: Option<Vec<f32>>,
    pub preview_peak: f32,
    pub playback_boundary_jump: f32,
    pub playback_hop_samples: usize,
    pub playback_fade_samples: usize,
}

pub struct PreparedEngine {
    pub frame: FrameFile,
    pub engine_kind: EngineKind,
    pub synth_window: WindowKind,
    pub overlap_percent: f32,
    pub preview_samples: Vec<f32>,
    pub preview_sample_rate: u32,
    pub playback_loop: Option<Vec<f32>>,
    pub preview_peak: f32,
    pub playback_boundary_jump: f32,
    pub playback_hop_samples: usize,
    pub playback_fade_samples: usize,
}

pub enum WorkerMessage {
    DocumentPrepared(Result<PreparedDocument, String>),
    EnginePrepared(Result<PreparedEngine, String>),
    AudioGeneSaved(Result<PathBuf, String>),
    AudioExported(Result<PathBuf, String>),
}

#[derive(Clone, Debug)]
struct TimedEntry {
    key: String,
    duration: Duration,
}

pub struct StatusBarManager {
    activity: String,
    timings: VecDeque<TimedEntry>,
    operation_start: Option<(String, Instant)>,
}

impl StatusBarManager {
    pub fn new() -> Self {
        Self {
            activity: "Ready | Load a frame or audio_gene to begin".to_string(),
            timings: VecDeque::new(),
            operation_start: None,
        }
    }

    pub fn set_activity(&mut self, text: &str) {
        self.activity = text.to_string();
    }

    pub fn start_timing(&mut self, key: &str) {
        self.operation_start = Some((key.to_string(), Instant::now()));
    }

    pub fn finish_timing(&mut self) -> Option<Duration> {
        if let Some((key, start)) = self.operation_start.take() {
            let duration = start.elapsed();
            self.timings.retain(|e| e.key != key);
            self.timings.push_front(TimedEntry { key, duration });
            Some(duration)
        } else {
            None
        }
    }

    pub fn cancel_timing(&mut self) {
        self.operation_start = None;
    }

    pub fn render(&self) -> String {
        let mut parts = vec![self.activity.clone()];
        for entry in &self.timings {
            let secs = entry.duration.as_secs_f64();
            if secs >= 60.0 {
                let mins = secs as u32 / 60;
                let rem = secs % 60.0;
                parts.push(format!("{}: {}m {:.1}s", entry.key, mins, rem));
            } else {
                parts.push(format!("{}: {:.2}s", entry.key, secs));
            }
        }
        parts.push(format!("Memory: {}", format_memory_usage()));
        parts.join("  |  ")
    }

    pub fn render_wrapped(&self, max_chars: usize) -> String {
        let text = self.render();
        let parts: Vec<&str> = text.split("  |  ").collect();
        let sep = "  |  ";
        let mut lines: Vec<String> = Vec::new();
        let mut current = String::new();
        let max = max_chars.max(24);
        for part in parts {
            let candidate = if current.is_empty() {
                part.to_string()
            } else {
                format!("{}{}{}", current, sep, part)
            };
            if candidate.len() > max && !current.is_empty() {
                lines.push(current);
                current = part.to_string();
            } else {
                current = candidate;
            }
        }
        if !current.is_empty() {
            lines.push(current);
        }
        lines.join("\n")
    }
}

fn format_memory_usage() -> String {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("VmRSS:") {
            let kb = rest
                .split_whitespace()
                .next()
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0);
            return if kb >= 1024 {
                format!("{:.1} MB", kb as f64 / 1024.0)
            } else {
                format!("{} KB", kb)
            };
        }
    }
    "n/a".to_string()
}

#[derive(Clone, Debug)]
pub struct WaveViewState {
    pub time_offset_sec: f64,
    pub time_span_sec: f64,
    pub amp_visual_gain: f32,
}

impl Default for WaveViewState {
    fn default() -> Self {
        Self {
            time_offset_sec: 0.0,
            time_span_sec: 0.050,
            amp_visual_gain: 1.0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SpecViewState {
    pub freq_min_hz: f32,
    pub freq_max_hz: f32,
}

impl Default for SpecViewState {
    fn default() -> Self {
        Self {
            freq_min_hz: 0.0,
            freq_max_hz: 5000.0,
        }
    }
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseMode {
    Move,
    Amp,
}

pub struct AppState {
    pub frame: Option<FrameFile>,
    pub preview_samples: Vec<f32>,
    pub preview_sample_rate: u32,
    pub synth: SynthPlayer,
    pub wave_view: WaveViewState,
    pub spec_view: SpecViewState,
    pub audio_gain_user: f32,
    pub audio_gain_auto: f32,
    pub auto_gain_applied_from_load: bool,
    pub waveform_zoom_factor: f32,
    pub waveform_amp_zoom_factor: f32,
    pub spec_freq_zoom_factor: f32,
    pub hover_wave_time_sec: Option<f64>,
    pub hover_spec_freq_hz: Option<f32>,
    pub preview_peak: f32,
    pub playback_buffer_len: usize,
    pub playback_boundary_jump: f32,
    pub playback_hop_samples: usize,
    pub playback_fade_samples: usize,
    pub engine_kind: EngineKind,
    pub synth_window: WindowKind,
    pub overlap_percent: f32,
    #[allow(dead_code)]
    pub mouse_mode: MouseMode,
    pub status: StatusBarManager,
    pub current_filename: String,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            frame: None,
            preview_samples: Vec::new(),
            preview_sample_rate: 44100,
            synth: SynthPlayer::new(),
            wave_view: WaveViewState::default(),
            spec_view: SpecViewState::default(),
            audio_gain_user: 1.0,
            audio_gain_auto: 1.0,
            auto_gain_applied_from_load: false,
            waveform_zoom_factor: 1.35,
            waveform_amp_zoom_factor: 1.25,
            spec_freq_zoom_factor: 1.20,
            hover_wave_time_sec: None,
            hover_spec_freq_hz: None,
            preview_peak: 0.0,
            playback_buffer_len: 0,
            playback_boundary_jump: 0.0,
            playback_hop_samples: 0,
            playback_fade_samples: 0,
            engine_kind: EngineKind::OscBank,
            synth_window: WindowKind::Hann,
            overlap_percent: 0.0,
            mouse_mode: MouseMode::Move,
            status: StatusBarManager::new(),
            current_filename: String::new(),
        }
    }

    pub fn has_frame(&self) -> bool {
        self.frame.is_some()
    }

    pub fn set_combined_audio_gain(&mut self) {
        self.synth
            .set_gain(self.audio_gain_user * self.audio_gain_auto);
    }

    pub fn max_preview_time(&self) -> f64 {
        self.preview_samples.len() as f64 / self.preview_sample_rate.max(1) as f64
    }
}
