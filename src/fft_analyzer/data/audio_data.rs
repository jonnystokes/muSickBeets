use anyhow::{Context, Result};
use hound::{WavReader, WavWriter, WavSpec, SampleFormat};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct AudioData {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub duration_seconds: f64,
}

impl AudioData {
    pub fn from_wav_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let mut reader = WavReader::open(&path)
            .with_context(|| format!("Failed to open WAV file: {:?}", path.as_ref()))?;

        let spec = reader.spec();
        let sample_rate = spec.sample_rate;
        let channels = spec.channels;
        let bits_per_sample = spec.bits_per_sample;
        let sample_format = spec.sample_format;

        let samples: Vec<f32> = match sample_format {
            SampleFormat::Float => {
                reader
                    .samples::<f32>()
                    .collect::<Result<Vec<f32>, _>>()
                    .context("Failed to read float samples")?
            }
            SampleFormat::Int => {
                match bits_per_sample {
                    16 => {
                        reader
                            .samples::<i16>()
                            .map(|s| s.map(|v| v as f32 / i16::MAX as f32))
                            .collect::<Result<Vec<f32>, _>>()
                            .context("Failed to read i16 samples")?
                    }
                    24 => {
                        reader
                            .samples::<i32>()
                            .map(|s| s.map(|v| v as f32 / 8388608.0))
                            .collect::<Result<Vec<f32>, _>>()
                            .context("Failed to read i24 samples")?
                    }
                    32 => {
                        reader
                            .samples::<i32>()
                            .map(|s| s.map(|v| v as f32 / i32::MAX as f32))
                            .collect::<Result<Vec<f32>, _>>()
                            .context("Failed to read i32 samples")?
                    }
                    _ => {
                        anyhow::bail!("Unsupported bit depth: {}", bits_per_sample);
                    }
                }
            }
        };

        let mono_samples = if channels == 1 {
            samples
        } else {
            samples
                .chunks(channels as usize)
                .map(|chunk| chunk.iter().sum::<f32>() / channels as f32)
                .collect()
        };

        let duration_seconds = mono_samples.len() as f64 / sample_rate as f64;

        Ok(AudioData {
            samples: mono_samples,
            sample_rate,
            duration_seconds,
        })
    }

    pub fn save_wav<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let spec = WavSpec {
            channels: 1,
            sample_rate: self.sample_rate,
            bits_per_sample: 16,
            sample_format: SampleFormat::Int,
        };
        let mut writer = WavWriter::create(&path, spec)
            .with_context(|| format!("Failed to create WAV file: {:?}", path.as_ref()))?;

        for &sample in &self.samples {
            let s = (sample * i16::MAX as f32).clamp(i16::MIN as f32, i16::MAX as f32) as i16;
            writer.write_sample(s)?;
        }
        writer.finalize()?;
        Ok(())
    }

    #[inline]
    pub fn num_samples(&self) -> usize {
        self.samples.len()
    }

    #[inline]
    pub fn get_slice(&self, start_sample: usize, end_sample: usize) -> &[f32] {
        let start = start_sample.min(self.samples.len());
        let end = end_sample.min(self.samples.len());
        &self.samples[start..end]
    }

    pub fn nyquist_freq(&self) -> f32 {
        self.sample_rate as f32 / 2.0
    }

    /// Peak-normalize audio so the loudest sample reaches `target_peak` (e.g. 0.97).
    /// Returns the gain factor applied, or 1.0 if no normalization was needed.
    pub fn normalize(&mut self, target_peak: f32) -> f32 {
        let peak = self.samples.iter()
            .map(|s| s.abs())
            .fold(0.0_f32, f32::max);

        if peak <= 0.0 || (peak - target_peak).abs() < 0.01 {
            return 1.0;
        }

        let gain = target_peak / peak;
        for s in &mut self.samples {
            *s *= gain;
        }
        gain
    }
}
