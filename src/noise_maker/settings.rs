use anyhow::{Context, Result};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use crate::app_state::{EngineKind, WindowKind};

#[derive(Clone, Debug)]
pub struct AppSettings {
    pub engine_kind: EngineKind,
    pub synth_window: WindowKind,
    pub overlap_percent: f32,
    pub audio_gain_user: f32,
    pub wave_amp_visual_gain: f32,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            engine_kind: EngineKind::OscBank,
            synth_window: WindowKind::Hann,
            overlap_percent: 0.0,
            audio_gain_user: 1.0,
            wave_amp_visual_gain: 1.0,
        }
    }
}

impl AppSettings {
    pub fn path() -> PathBuf {
        PathBuf::from("noise_maker_settings.ini")
    }

    pub fn load_from_path<P: AsRef<Path>>(path: P) -> Result<Self> {
        let text = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read settings: {:?}", path.as_ref()))?;
        let mut s = Self::default();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                match k.trim() {
                    "engine_kind" => {
                        s.engine_kind = match v.trim() {
                            "frame_ola" => EngineKind::FrameOla,
                            _ => EngineKind::OscBank,
                        }
                    }
                    "synth_window" => {
                        s.synth_window = match v.trim() {
                            "rectangular" => WindowKind::Rectangular,
                            "hann" => WindowKind::Hann,
                            "hamming" => WindowKind::Hamming,
                            "blackman" => WindowKind::Blackman,
                            "kaiser" => WindowKind::Kaiser,
                            _ => WindowKind::Hann,
                        }
                    }
                    "overlap_percent" => {
                        s.overlap_percent = v.trim().parse().unwrap_or(s.overlap_percent)
                    }
                    "audio_gain_user" => {
                        s.audio_gain_user = v.trim().parse().unwrap_or(s.audio_gain_user)
                    }
                    "wave_amp_visual_gain" => {
                        s.wave_amp_visual_gain = v.trim().parse().unwrap_or(s.wave_amp_visual_gain)
                    }
                    _ => {}
                }
            }
        }
        Ok(s)
    }

    pub fn load_or_default() -> Self {
        Self::load_from_path(Self::path()).unwrap_or_default()
    }

    pub fn save_to_path<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let mut out = String::new();
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
        let _ = writeln!(out, "wave_amp_visual_gain={}", self.wave_amp_visual_gain);
        fs::write(&path, out)
            .with_context(|| format!("Failed to write settings: {:?}", path.as_ref()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_roundtrip_preserves_values() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!(
            "noise_maker_settings_test_{}.ini",
            std::process::id()
        ));
        let settings = AppSettings {
            engine_kind: EngineKind::FrameOla,
            synth_window: WindowKind::Blackman,
            overlap_percent: 61.0,
            audio_gain_user: 1.5,
            wave_amp_visual_gain: 3.25,
        };
        settings
            .save_to_path(&path)
            .expect("settings save should succeed");
        let loaded = AppSettings::load_from_path(&path).expect("settings load should succeed");
        let _ = std::fs::remove_file(&path);

        assert_eq!(loaded.engine_kind, settings.engine_kind);
        assert_eq!(loaded.synth_window, settings.synth_window);
        assert_eq!(loaded.overlap_percent, settings.overlap_percent);
        assert_eq!(loaded.audio_gain_user, settings.audio_gain_user);
        assert_eq!(loaded.wave_amp_visual_gain, settings.wave_amp_visual_gain);
    }
}
