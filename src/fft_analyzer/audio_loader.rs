
use anyhow::{Context, Result};
use hound::{WavReader, SampleFormat};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct AudioData {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
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

        // Read all samples and convert to f32
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
                            .map(|s| s.map(|v| v as f32 / 8388608.0)) // 2^23
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

        // Convert to mono if stereo (simple average)
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
            channels,
            duration_seconds,
        })
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

    #[inline]
    pub fn time_to_sample(&self, time_seconds: f64) -> usize {
        (time_seconds * self.sample_rate as f64) as usize
    }

    #[inline]
    pub fn sample_to_time(&self, sample: usize) -> f64 {
        sample as f64 / self.sample_rate as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_sample_conversion() {
        let audio = AudioData {
            samples: vec![0.0; 48000],
            sample_rate: 48000,
            channels: 1,
            duration_seconds: 1.0,
        };

        assert_eq!(audio.time_to_sample(0.5), 24000);
        assert_eq!(audio.sample_to_time(24000), 0.5);
    }

    #[test]
    fn test_get_slice() {
        let audio = AudioData {
            samples: (0..100).map(|i| i as f32).collect(),
            sample_rate: 100,
            channels: 1,
            duration_seconds: 1.0,
        };

        let slice = audio.get_slice(10, 20);
        assert_eq!(slice.len(), 10);
        assert_eq!(slice[0], 10.0);
    }

    #[test]
    fn test_get_slice_bounds() {
        let audio = AudioData {
            samples: vec![0.0; 100],
            sample_rate: 100,
            channels: 1,
            duration_seconds: 1.0,
        };

        // Test out of bounds
        let slice = audio.get_slice(50, 200);
        assert_eq!(slice.len(), 50);
    }
}

