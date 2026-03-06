use std::cell::RefCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use rayon::prelude::*;
use realfft::RealFftPlanner;

use crate::data::{AudioData, FftFrame, FftParams, Spectrogram};

thread_local! {
    /// Per-thread FFT planner cache. `RealFftPlanner` caches FFT plans internally,
    /// so reusing one planner per rayon thread avoids re-planning for every frame.
    static FFT_PLANNER: RefCell<RealFftPlanner<f32>> = RefCell::new(RealFftPlanner::new());
}

pub struct FftEngine;

impl FftEngine {
    /// Process audio into a spectrogram using parallel FFT computation.
    /// Each frame's FFT runs independently on a rayon thread.
    /// If `cancel` is set to true, processing stops early and returns
    /// whatever frames have been computed so far (may be empty).
    pub fn process(audio: &AudioData, params: &FftParams, cancel: &AtomicBool) -> Spectrogram {
        let start_sample = params.start_sample;
        let stop_sample = params.stop_sample.min(audio.num_samples());

        if start_sample >= stop_sample {
            return Spectrogram::default();
        }

        let audio_slice = audio.get_slice(start_sample, stop_sample);
        let hop = params.hop_length();
        let window_len = params.window_length;
        let n_fft = params.n_fft_padded();

        let (padded_audio, _offset) = if params.use_center {
            let pad = window_len / 2;
            let mut padded = vec![0.0; audio_slice.len() + 2 * pad];
            padded[pad..pad + audio_slice.len()].copy_from_slice(audio_slice);
            (padded, pad)
        } else {
            (audio_slice.to_vec(), 0)
        };

        let num_frames = if padded_audio.len() >= window_len {
            (padded_audio.len() - window_len) / hop + 1
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

        // Compute frequency bin values once — shared across all frames.
        // Previously each frame stored its own copy (~16 MB waste for 1000 frames).
        let num_bins = n_fft / 2 + 1; // realfft output size
        let frequencies: Vec<f32> = (0..num_bins)
            .map(|bin_idx| bin_idx as f32 * freq_resolution)
            .collect();

        // Parallel FFT: each frame is independent.
        // Each frame checks the cancellation flag before doing work;
        // cancelled frames return None and are filtered out.
        let frames: Vec<FftFrame> = (0..num_frames)
            .into_par_iter()
            .filter_map(|frame_idx| {
                // Check cancellation before expensive work
                if cancel.load(Ordering::Relaxed) {
                    return None;
                }

                let fft = FFT_PLANNER.with(|p| p.borrow_mut().plan_fft_forward(n_fft));

                let start = frame_idx * hop;
                let mut indata = vec![0.0f32; n_fft];
                let mut spectrum = fft.make_output_vec();

                // Apply window to first window_len samples; rest stays zero (zero-padding)
                for i in 0..window_len {
                    indata[i] = padded_arc[start + i] * window_arc[i];
                }

                fft.process(&mut indata, &mut spectrum)
                    .expect("FFT processing failed");

                let actual_sample = start_sample + frame_idx * hop;
                let time_seconds = actual_sample as f64 / audio.sample_rate as f64;

                let spec_bins = spectrum.len();
                let mut magnitudes = Vec::with_capacity(spec_bins);
                let mut phases = Vec::with_capacity(spec_bins);

                for (bin_idx, complex_val) in spectrum.iter().enumerate() {
                    // Normalize magnitude by FFT size and scale by 2 for non-DC/Nyquist bins
                    let amplitude_scale = if bin_idx == 0 || bin_idx == spec_bins - 1 {
                        1.0
                    } else {
                        2.0
                    };
                    magnitudes.push((complex_val.norm() / n_fft as f32) * amplitude_scale);

                    phases.push(complex_val.arg());
                }

                Some(FftFrame {
                    time_seconds,
                    magnitudes,
                    phases,
                })
            })
            .collect();

        Spectrogram::from_frames_with_frequencies(frames, frequencies)
    }
}
