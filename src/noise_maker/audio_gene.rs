use anyhow::{bail, Context, Result};
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use crate::app_state::{EngineKind, WindowKind};
use crate::frame_file::FrameFile;

#[derive(Clone, Debug)]
pub struct AudioGeneProject {
    pub current_filename: String,
    pub engine_kind: EngineKind,
    pub synth_window: WindowKind,
    pub overlap_percent: f32,
    pub audio_gain_user: f32,
    pub audio_gain_auto: f32,
    pub wave_time_offset_sec: f64,
    pub wave_time_span_sec: f64,
    pub wave_amp_visual_gain: f32,
    pub spec_freq_min_hz: f32,
    pub spec_freq_max_hz: f32,
    pub frame: FrameFile,
}

impl AudioGeneProject {
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self> {
        let text = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read audio_gene: {:?}", path.as_ref()))?;
        Self::parse(&text)
    }

    pub fn parse(text: &str) -> Result<Self> {
        let mut lines = text.lines();
        let magic = lines.next().unwrap_or_default().trim();
        if magic != "MUSICKBEETS_AUDIO_GENE_V1" {
            bail!("Unsupported audio_gene magic: {}", magic);
        }

        let mut current_filename = String::new();
        let mut engine_kind = EngineKind::OscBank;
        let mut synth_window = WindowKind::Hann;
        let mut overlap_percent = 0.0_f32;
        let mut audio_gain_user = 1.0_f32;
        let mut audio_gain_auto = 1.0_f32;
        let mut wave_time_offset_sec = 0.0_f64;
        let mut wave_time_span_sec = 0.050_f64;
        let mut wave_amp_visual_gain = 1.0_f32;
        let mut spec_freq_min_hz = 0.0_f32;
        let mut spec_freq_max_hz = 5000.0_f32;
        let mut frame_text = String::new();
        let mut in_frame = false;

        for line in lines {
            let line = line.trim_end();
            if line == "---FRAME---" {
                in_frame = true;
                continue;
            }
            if in_frame {
                let _ = writeln!(frame_text, "{}", line);
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                match k.trim() {
                    "current_filename" => current_filename = v.trim().to_string(),
                    "engine_kind" => {
                        engine_kind = match v.trim() {
                            "osc_bank" => EngineKind::OscBank,
                            "frame_ola" => EngineKind::FrameOla,
                            _ => EngineKind::OscBank,
                        }
                    }
                    "synth_window" => {
                        synth_window = match v.trim() {
                            "rectangular" => WindowKind::Rectangular,
                            "hann" => WindowKind::Hann,
                            "hamming" => WindowKind::Hamming,
                            "blackman" => WindowKind::Blackman,
                            "kaiser" => WindowKind::Kaiser,
                            _ => WindowKind::Hann,
                        }
                    }
                    "overlap_percent" => {
                        overlap_percent = v.trim().parse().context("Invalid overlap_percent")?
                    }
                    "audio_gain_user" => {
                        audio_gain_user = v.trim().parse().context("Invalid audio_gain_user")?
                    }
                    "audio_gain_auto" => {
                        audio_gain_auto = v.trim().parse().context("Invalid audio_gain_auto")?
                    }
                    "wave_time_offset_sec" => {
                        wave_time_offset_sec =
                            v.trim().parse().context("Invalid wave_time_offset_sec")?
                    }
                    "wave_time_span_sec" => {
                        wave_time_span_sec =
                            v.trim().parse().context("Invalid wave_time_span_sec")?
                    }
                    "wave_amp_visual_gain" => {
                        wave_amp_visual_gain =
                            v.trim().parse().context("Invalid wave_amp_visual_gain")?
                    }
                    "spec_freq_min_hz" => {
                        spec_freq_min_hz = v.trim().parse().context("Invalid spec_freq_min_hz")?
                    }
                    "spec_freq_max_hz" => {
                        spec_freq_max_hz = v.trim().parse().context("Invalid spec_freq_max_hz")?
                    }
                    _ => {}
                }
            }
        }

        if frame_text.is_empty() {
            bail!("audio_gene missing embedded frame data");
        }

        Ok(Self {
            current_filename,
            engine_kind,
            synth_window,
            overlap_percent,
            audio_gain_user,
            audio_gain_auto,
            wave_time_offset_sec,
            wave_time_span_sec,
            wave_amp_visual_gain,
            spec_freq_min_hz,
            spec_freq_max_hz,
            frame: FrameFile::parse(&frame_text)?,
        })
    }

    pub fn to_text(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "MUSICKBEETS_AUDIO_GENE_V1");
        let _ = writeln!(out, "current_filename={}", self.current_filename);
        let _ = writeln!(
            out,
            "engine_kind={}",
            match self.engine_kind {
                EngineKind::OscBank => "osc_bank",
                EngineKind::FrameOla => "frame_ola",
            }
        );
        let _ = writeln!(
            out,
            "synth_window={}",
            match self.synth_window {
                WindowKind::Rectangular => "rectangular",
                WindowKind::Hann => "hann",
                WindowKind::Hamming => "hamming",
                WindowKind::Blackman => "blackman",
                WindowKind::Kaiser => "kaiser",
            }
        );
        let _ = writeln!(out, "overlap_percent={}", self.overlap_percent);
        let _ = writeln!(out, "audio_gain_user={}", self.audio_gain_user);
        let _ = writeln!(out, "audio_gain_auto={}", self.audio_gain_auto);
        let _ = writeln!(out, "wave_time_offset_sec={}", self.wave_time_offset_sec);
        let _ = writeln!(out, "wave_time_span_sec={}", self.wave_time_span_sec);
        let _ = writeln!(out, "wave_amp_visual_gain={}", self.wave_amp_visual_gain);
        let _ = writeln!(out, "spec_freq_min_hz={}", self.spec_freq_min_hz);
        let _ = writeln!(out, "spec_freq_max_hz={}", self.spec_freq_max_hz);
        let _ = writeln!(out, "---FRAME---");
        out.push_str(&self.frame.to_text());
        out
    }

    pub fn save_to_path<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        fs::write(&path, self.to_text())
            .with_context(|| format!("Failed to write audio_gene: {:?}", path.as_ref()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame_file::{FrameBin, FrameFile};

    fn sample_project() -> AudioGeneProject {
        AudioGeneProject {
            current_filename: "test.audio_gene".to_string(),
            engine_kind: EngineKind::FrameOla,
            synth_window: WindowKind::Blackman,
            overlap_percent: 62.0,
            audio_gain_user: 1.7,
            audio_gain_auto: 0.42,
            wave_time_offset_sec: 0.125,
            wave_time_span_sec: 0.333,
            wave_amp_visual_gain: 4.2,
            spec_freq_min_hz: 120.0,
            spec_freq_max_hz: 3456.0,
            frame: FrameFile {
                sample_rate: 44_100,
                fft_size: 2048,
                frame_index: 3,
                frame_time_seconds: 1.2345,
                active_bin_count: 2,
                bins: vec![
                    FrameBin {
                        bin_index: 7,
                        frequency_hz: 150.0,
                        magnitude: 0.3,
                        phase_rad: 0.5,
                    },
                    FrameBin {
                        bin_index: 21,
                        frequency_hz: 440.0,
                        magnitude: 0.7,
                        phase_rad: -0.2,
                    },
                ],
            },
        }
    }

    #[test]
    fn audio_gene_roundtrip_preserves_core_state() {
        let project = sample_project();
        let text = project.to_text();
        let parsed = AudioGeneProject::parse(&text).expect("audio_gene parse should succeed");

        assert_eq!(parsed.current_filename, project.current_filename);
        assert_eq!(parsed.engine_kind, project.engine_kind);
        assert_eq!(parsed.synth_window, project.synth_window);
        assert_eq!(parsed.overlap_percent, project.overlap_percent);
        assert_eq!(parsed.audio_gain_user, project.audio_gain_user);
        assert_eq!(parsed.audio_gain_auto, project.audio_gain_auto);
        assert_eq!(parsed.wave_time_offset_sec, project.wave_time_offset_sec);
        assert_eq!(parsed.wave_time_span_sec, project.wave_time_span_sec);
        assert_eq!(parsed.wave_amp_visual_gain, project.wave_amp_visual_gain);
        assert_eq!(parsed.spec_freq_min_hz, project.spec_freq_min_hz);
        assert_eq!(parsed.spec_freq_max_hz, project.spec_freq_max_hz);
        assert_eq!(parsed.frame.sample_rate, project.frame.sample_rate);
        assert_eq!(parsed.frame.fft_size, project.frame.fft_size);
        assert_eq!(parsed.frame.bins.len(), project.frame.bins.len());
    }

    #[test]
    fn audio_gene_save_load_path_roundtrip() {
        let project = sample_project();
        let dir = std::env::temp_dir();
        let path = dir.join(format!(
            "noise_maker_project_test_{}.audio_gene",
            std::process::id()
        ));
        project
            .save_to_path(&path)
            .expect("audio_gene save should succeed");
        let loaded = AudioGeneProject::from_path(&path).expect("audio_gene load should succeed");
        let _ = std::fs::remove_file(&path);

        assert_eq!(loaded.engine_kind, project.engine_kind);
        assert_eq!(loaded.synth_window, project.synth_window);
        assert_eq!(loaded.overlap_percent, project.overlap_percent);
        assert_eq!(loaded.frame.frame_index, project.frame.frame_index);
        assert_eq!(loaded.frame.bins.len(), project.frame.bins.len());
    }
}
