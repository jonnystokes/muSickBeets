

use realfft::{RealFftPlanner, RealToComplex};
use std::sync::Arc;

use super::{AudioData, FftParams};

pub struct FftEngine {
    fft: Arc<dyn RealToComplex<f32>>,
    params: FftParams,
    window: Vec<f32>,
}

#[derive(Debug, Clone)]
pub struct FftFrame {
    pub time_seconds: f64,
    pub frequencies: Vec<f32>,      // Frequency bins (Hz)
    pub magnitudes: Vec<f32>,       // Magnitude of each bin
    pub phases: Vec<f32>,           // Phase of each bin (radians)
}

impl FftEngine {
    pub fn new(params: FftParams) -> Self {
        let mut planner = RealFftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(params.window_length);
        let window = params.generate_window();

        Self {
            fft,
            params,
            window,
        }
    }

    pub fn update_params(&mut self, params: FftParams) {
        // Only recreate FFT if window length changed
        if params.window_length != self.params.window_length {
            let mut planner = RealFftPlanner::<f32>::new();
            self.fft = planner.plan_fft_forward(params.window_length);
        }

        // Always regenerate window if params changed
        self.window = params.generate_window();
        self.params = params;
    }

    pub fn process_audio(&self, audio: &AudioData) -> Vec<FftFrame> {
        let start_sample = self.params.start_sample();
        let stop_sample = self.params.stop_sample().min(audio.num_samples());
        
        if start_sample >= stop_sample {
            return Vec::new();
        }

        let audio_slice = audio.get_slice(start_sample, stop_sample);
        let hop = self.params.hop_length();
        let n_fft = self.params.window_length;

        // Calculate padding for centering
        let (padded_audio, _offset) = if self.params.use_center {
            let pad = n_fft / 2;
            let mut padded = vec![0.0; audio_slice.len() + 2 * pad];
            padded[pad..pad + audio_slice.len()].copy_from_slice(audio_slice);
            (padded, pad)
        } else {
            (audio_slice.to_vec(), 0)
        };

        let num_frames = (padded_audio.len().saturating_sub(n_fft)) / hop + 1;
        let mut frames = Vec::with_capacity(num_frames);

        // Prepare buffers for FFT
        let mut indata = vec![0.0f32; n_fft];
        let mut spectrum = self.fft.make_output_vec();

        for frame_idx in 0..num_frames {
            let start = frame_idx * hop;
            if start + n_fft > padded_audio.len() {
                break;
            }

            // Apply window and copy to FFT input
            for i in 0..n_fft {
                indata[i] = padded_audio[start + i] * self.window[i];
            }

            // Perform FFT
            self.fft
                .process(&mut indata, &mut spectrum)
                .expect("FFT processing failed");

            // Calculate actual time of this frame
            let actual_sample = start_sample + frame_idx * hop;
            let time_seconds = audio.sample_to_time(actual_sample);

            // Extract magnitude and phase
            let num_bins = spectrum.len();
            let mut frequencies = Vec::with_capacity(num_bins);
            let mut magnitudes = Vec::with_capacity(num_bins);
            let mut phases = Vec::with_capacity(num_bins);

            let freq_resolution = audio.sample_rate as f32 / n_fft as f32;

            for (bin_idx, complex_val) in spectrum.iter().enumerate() {
                let freq = bin_idx as f32 * freq_resolution;
                let mag = complex_val.norm();
                let phase = complex_val.arg();

                frequencies.push(freq);
                magnitudes.push(mag);
                phases.push(phase);
            }

            frames.push(FftFrame {
                time_seconds,
                frequencies,
                magnitudes,
                phases,
            });
        }

        frames
    }

    pub fn num_frequency_bins(&self) -> usize {
        self.params.window_length / 2 + 1
    }

    pub fn frequency_resolution(&self) -> f32 {
        self.params.sample_rate as f32 / self.params.window_length as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fft_engine_creation() {
        let params = FftParams::default();
        let engine = FftEngine::new(params);
        assert_eq!(engine.num_frequency_bins(), 1025); // 2048/2 + 1
    }

    #[test]
    fn test_frequency_resolution() {
        let params = FftParams {
            window_length: 2048,
            sample_rate: 48000,
            ..Default::default()
        };
        let engine = FftEngine::new(params);
        assert!((engine.frequency_resolution() - 23.4375).abs() < 0.001);
    }
}
