use anyhow::{bail, Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Clone, Debug)]
pub struct FrameBin {
    #[allow(dead_code)]
    pub bin_index: usize,
    pub frequency_hz: f32,
    pub magnitude: f32,
    pub phase_rad: f32,
}

#[derive(Clone, Debug)]
pub struct FrameFile {
    pub sample_rate: u32,
    pub fft_size: usize,
    pub frame_index: usize,
    pub frame_time_seconds: f64,
    pub active_bin_count: usize,
    pub bins: Vec<FrameBin>,
}

impl FrameFile {
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self> {
        let text = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read frame file: {:?}", path.as_ref()))?;
        Self::parse(&text)
    }

    pub fn parse(text: &str) -> Result<Self> {
        let mut lines = text.lines();
        let magic = lines.next().unwrap_or_default().trim();
        if magic != "MUSICKBEETS_FRAME_V1" {
            bail!("Unsupported frame file magic: {}", magic);
        }

        let mut meta = HashMap::<String, String>::new();
        for line in &mut lines {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if line == "---" {
                break;
            }
            if let Some((k, v)) = line.split_once('=') {
                meta.insert(k.trim().to_string(), v.trim().to_string());
            }
        }

        let header = lines.next().unwrap_or_default().trim();
        if header != "bin_index,frequency_hz,magnitude,phase_rad" {
            bail!("Unexpected frame data header: {}", header);
        }

        let mut bins = Vec::new();
        for line in lines {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() != 4 {
                bail!("Malformed frame data row: {}", line);
            }
            bins.push(FrameBin {
                bin_index: parts[0].parse().context("Invalid bin_index")?,
                frequency_hz: parts[1].parse().context("Invalid frequency_hz")?,
                magnitude: parts[2].parse().context("Invalid magnitude")?,
                phase_rad: parts[3].parse().context("Invalid phase_rad")?,
            });
        }

        let sample_rate = meta
            .get("sample_rate")
            .context("Missing sample_rate")?
            .parse()
            .context("Invalid sample_rate")?;
        let fft_size = meta
            .get("fft_size")
            .context("Missing fft_size")?
            .parse()
            .context("Invalid fft_size")?;
        let frame_index = meta
            .get("frame_index")
            .context("Missing frame_index")?
            .parse()
            .context("Invalid frame_index")?;
        let frame_time_seconds = meta
            .get("frame_time_seconds")
            .context("Missing frame_time_seconds")?
            .parse()
            .context("Invalid frame_time_seconds")?;
        let active_bin_count = meta
            .get("active_bin_count")
            .context("Missing active_bin_count")?
            .parse()
            .context("Invalid active_bin_count")?;

        Ok(Self {
            sample_rate,
            fft_size,
            frame_index,
            frame_time_seconds,
            active_bin_count,
            bins,
        })
    }

    pub fn max_frequency_hz(&self) -> f32 {
        self.bins
            .iter()
            .map(|b| b.frequency_hz)
            .fold(0.0_f32, f32::max)
    }
}
