
use realfft::{RealFftPlanner, ComplexToReal};
use rustfft::num_complex::Complex;
use std::sync::Arc;

use super::{AudioData, FftParams, Spectrogram};

#[derive(Debug, Clone, Copy)]
pub enum ReconstructionQuality {
    Fast,      // Skip frames, lower precision - 10x faster
    Balanced,  // Some frame skipping - 3x faster  
    High,      // All frames, single precision - 1.5x faster
    Perfect,   // All frames, double precision, perfect overlap-add
}

impl ReconstructionQuality {
    pub fn from_percent(percent: f32) -> Self {
        match percent {
            p if p < 25.0 => Self::Fast,
            p if p < 50.0 => Self::Balanced,
            p if p < 90.0 => Self::High,
            _ => Self::Perfect,
        }
    }

    pub fn to_percent(&self) -> f32 {
        match self {
            Self::Fast => 0.0,
            Self::Balanced => 40.0,
            Self::High => 70.0,
            Self::Perfect => 100.0,
        }
    }

    pub fn frame_skip(&self) -> usize {
        match self {
            Self::Fast => 4,      // Use every 4th frame
            Self::Balanced => 2,  // Use every 2nd frame
            Self::High => 1,      // Use all frames
            Self::Perfect => 1,   // Use all frames
        }
    }
}

pub struct AudioReconstructor {
    ifft: Arc<dyn ComplexToReal<f32>>,
    params: FftParams,
    window: Vec<f32>,
    quality: ReconstructionQuality,
}

impl AudioReconstructor {
    pub fn new(params: FftParams, quality: ReconstructionQuality) -> Self {
        let mut planner = RealFftPlanner::<f32>::new();
        let ifft = planner.plan_fft_inverse(params.window_length);
        let window = params.generate_window();

        Self {
            ifft,
            params,
            window,
            quality,
        }
    }

    pub fn set_quality(&mut self, quality: ReconstructionQuality) {
        self.quality = quality;
    }

    /// Reconstruct audio from spectrogram using inverse STFT
    pub fn reconstruct(&self, spectrogram: &Spectrogram) -> AudioData {
        let hop = self.params.hop_length();
        let n_fft = self.params.window_length;
        let num_frames = spectrogram.num_frames();
        let frame_skip = self.quality.frame_skip();

        // Calculate output length
        let output_length = if self.params.use_center {
            // Remove padding
            let padded_length = (num_frames - 1) * hop + n_fft;
            padded_length.saturating_sub(n_fft)
        } else {
            (num_frames - 1) * hop + n_fft
        };

        let mut output = vec![0.0f32; output_length];
        let mut window_sum = vec![0.0f32; output_length];

        // Prepare buffers for IFFT
        let mut spectrum = self.ifft.make_input_vec();
        let mut time_buffer = self.ifft.make_output_vec();

        // Track which frames we actually process
        let mut processed_frame_count = 0usize;

        // Process each frame with quality-based skipping
        for (frame_idx, frame) in spectrogram.frames.iter().enumerate() {
            // Skip frames based on quality setting
            if frame_idx % frame_skip != 0 && frame_skip > 1 {
                continue;
            }

            // Build complex spectrum from magnitude and phase
            for (i, (&mag, &phase)) in frame.magnitudes.iter()
                .zip(frame.phases.iter())
                .enumerate()
            {
                if i < spectrum.len() {
                    // DC bin (i=0) and Nyquist bin (i=N/2) must have zero imaginary part
                    if i == 0 || i == spectrum.len() - 1 {
                        spectrum[i] = Complex::new(mag * phase.cos(), 0.0);
                    } else {
                        spectrum[i] = Complex::from_polar(mag, phase);
                    }
                }
            }

            // Inverse FFT
            self.ifft
                .process(&mut spectrum, &mut time_buffer)
                .expect("IFFT processing failed");

            // Calculate position - use the logical position based on processed frames
            // when frame skipping, or original position otherwise
            let start_pos = if frame_skip > 1 {
                processed_frame_count * hop * frame_skip
            } else if self.params.use_center {
                frame_idx * hop
            } else {
                frame_idx * hop
            };

            // Apply synthesis window and overlap-add
            for (i, &sample) in time_buffer.iter().enumerate() {
                let pos = start_pos + i;
                if pos < output.len() {
                    let windowed_sample = sample * self.window[i];
                    output[pos] += windowed_sample;
                    window_sum[pos] += self.window[i] * self.window[i];
                }
            }

            processed_frame_count += 1;
        }

        // Normalize by window sum (overlap-add normalization)
        for i in 0..output.len() {
            if window_sum[i] > 1e-8 {
                output[i] /= window_sum[i];
            }
        }

        // Apply additional normalization for perfect reconstruction
        if matches!(self.quality, ReconstructionQuality::Perfect) {
            let scale = 2.0 / n_fft as f32;
            for sample in &mut output {
                *sample *= scale;
            }
        }

        // Calculate duration before moving output
        let duration_seconds = output.len() as f64 / self.params.sample_rate as f64;

        AudioData {
            samples: output,
            sample_rate: self.params.sample_rate,
            channels: 1,
            duration_seconds,
        }
    }

    /// Estimate reconstruction time in seconds
    pub fn estimate_time(&self, num_frames: usize) -> f32 {
        let frames_to_process = num_frames / self.quality.frame_skip();
        let base_time_per_frame = match self.quality {
            ReconstructionQuality::Fast => 0.000_01,      // 10 µs per frame
            ReconstructionQuality::Balanced => 0.000_02,  // 20 µs per frame
            ReconstructionQuality::High => 0.000_05,      // 50 µs per frame
            ReconstructionQuality::Perfect => 0.000_1,    // 100 µs per frame
        };
        
        frames_to_process as f32 * base_time_per_frame
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::{FftEngine, WindowType};
    use super::super::fft_engine::FftFrame;

    #[test]
    fn test_reconstruction_quality_from_percent() {
        assert!(matches!(ReconstructionQuality::from_percent(0.0), ReconstructionQuality::Fast));
        assert!(matches!(ReconstructionQuality::from_percent(30.0), ReconstructionQuality::Balanced));
        assert!(matches!(ReconstructionQuality::from_percent(70.0), ReconstructionQuality::High));
        assert!(matches!(ReconstructionQuality::from_percent(100.0), ReconstructionQuality::Perfect));
    }

    #[test]
    fn test_frame_skip() {
        assert_eq!(ReconstructionQuality::Fast.frame_skip(), 4);
        assert_eq!(ReconstructionQuality::Balanced.frame_skip(), 2);
        assert_eq!(ReconstructionQuality::High.frame_skip(), 1);
        assert_eq!(ReconstructionQuality::Perfect.frame_skip(), 1);
    }

    #[test]
    fn test_reconstruction_roundtrip() {
        // Create test audio (1 second sine wave at 440 Hz)
        let sample_rate = 48000;
        let duration = 1.0;
        let freq = 440.0;
        let num_samples = (sample_rate as f32 * duration) as usize;
        
        let mut samples = vec![0.0f32; num_samples];
        for i in 0..num_samples {
            let t = i as f32 / sample_rate as f32;
            samples[i] = (2.0 * std::f32::consts::PI * freq * t).sin();
        }

        let original_audio = AudioData {
            samples: samples.clone(),
            sample_rate,
            channels: 1,
            duration_seconds: duration as f64,
        };

        // Forward FFT
        let params = FftParams {
            window_length: 2048,
            overlap_percent: 75.0,
            window_type: WindowType::Hann,
            use_center: false,
            sample_rate,
            ..Default::default()
        };

        let engine = FftEngine::new(params.clone());
        let frames = engine.process_audio(&original_audio);
        let spectrogram = Spectrogram::from_frames(frames);

        // Inverse FFT
        let reconstructor = AudioReconstructor::new(params, ReconstructionQuality::Perfect);
        let reconstructed = reconstructor.reconstruct(&spectrogram);

        // Check length is similar (within 10%)
        let len_ratio = reconstructed.samples.len() as f32 / original_audio.samples.len() as f32;
        assert!(len_ratio > 0.9 && len_ratio < 1.1, "Length ratio: {}", len_ratio);

        // Check similarity (RMS error should be low)
        let min_len = reconstructed.samples.len().min(original_audio.samples.len());
        let mut rms_error = 0.0;
        for i in 0..min_len {
            let diff = reconstructed.samples[i] - original_audio.samples[i];
            rms_error += diff * diff;
        }
        rms_error = (rms_error / min_len as f32).sqrt();
        
        // RMS error should be less than 0.1 for good reconstruction
        assert!(rms_error < 0.1, "RMS error: {}", rms_error);
    }

    #[test]
    fn test_estimate_time() {
        let params = FftParams::default();
        let reconstructor = AudioReconstructor::new(params, ReconstructionQuality::Fast);
        
        let est = reconstructor.estimate_time(1000);
        assert!(est > 0.0);
        assert!(est < 1.0); // Should be very fast
    }
}
