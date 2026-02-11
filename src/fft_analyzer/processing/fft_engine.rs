use rayon::prelude::*;
use realfft::RealFftPlanner;
use std::sync::Arc;

use crate::data::{AudioData, FftParams, FftFrame, Spectrogram};

pub struct FftEngine;

impl FftEngine {
    /// Process audio into a spectrogram using parallel FFT computation.
    /// Each frame's FFT runs independently on a rayon thread.
    pub fn process(audio: &AudioData, params: &FftParams) -> Spectrogram {
        let start_sample = params.start_sample();
        let stop_sample = params.stop_sample().min(audio.num_samples());

        if start_sample >= stop_sample {
            return Spectrogram::default();
        }

        let audio_slice = audio.get_slice(start_sample, stop_sample);
        let hop = params.hop_length();
        let n_fft = params.window_length;

        let (padded_audio, _offset) = if params.use_center {
            let pad = n_fft / 2;
            let mut padded = vec![0.0; audio_slice.len() + 2 * pad];
            padded[pad..pad + audio_slice.len()].copy_from_slice(audio_slice);
            (padded, pad)
        } else {
            (audio_slice.to_vec(), 0)
        };

        let num_frames = if padded_audio.len() >= n_fft {
            (padded_audio.len() - n_fft) / hop + 1
        } else {
            0
        };

        if num_frames == 0 {
            return Spectrogram::default();
        }

        let window = params.generate_window();
        let freq_resolution = audio.sample_rate as f32 / n_fft as f32;
        let padded_arc = Arc::new(padded_audio);
        let window_arc = Arc::new(window);

        // Parallel FFT: each frame is independent
        let frames: Vec<FftFrame> = (0..num_frames)
            .into_par_iter()
            .map(|frame_idx| {
                let mut planner = RealFftPlanner::<f32>::new();
                let fft = planner.plan_fft_forward(n_fft);

                let start = frame_idx * hop;
                let mut indata = vec![0.0f32; n_fft];
                let mut spectrum = fft.make_output_vec();

                for i in 0..n_fft {
                    indata[i] = padded_arc[start + i] * window_arc[i];
                }

                fft.process(&mut indata, &mut spectrum)
                    .expect("FFT processing failed");

                let actual_sample = start_sample + frame_idx * hop;
                let time_seconds = actual_sample as f64 / audio.sample_rate as f64;

                let num_bins = spectrum.len();
                let mut frequencies = Vec::with_capacity(num_bins);
                let mut magnitudes = Vec::with_capacity(num_bins);
                let mut phases = Vec::with_capacity(num_bins);

                for (bin_idx, complex_val) in spectrum.iter().enumerate() {
                    frequencies.push(bin_idx as f32 * freq_resolution);
                    magnitudes.push(complex_val.norm());
                    phases.push(complex_val.arg());
                }

                FftFrame {
                    time_seconds,
                    frequencies,
                    magnitudes,
                    phases,
                }
            })
            .collect();

        Spectrogram::from_frames(frames)
    }
}
