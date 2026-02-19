use rayon::prelude::*;
use realfft::RealFftPlanner;
use rustfft::num_complex::Complex;

use crate::data::{AudioData, FftParams, Spectrogram, ViewState};

/// Reconstructs audio from spectrogram data with configurable frequency filtering.
pub struct Reconstructor;

impl Reconstructor {
    /// Reconstruct audio from a spectrogram using current view state settings.
    /// - `freq_count`: how many top-magnitude bins to keep per frame (1..=max)
    /// - `freq_min/max`: frequency range to include
    pub fn reconstruct(
        spectrogram: &Spectrogram,
        params: &FftParams,
        view: &ViewState,
    ) -> AudioData {
        let hop = params.hop_length();
        let window_len = params.window_length;
        let n_fft = params.n_fft_padded();
        let num_frames = spectrogram.num_frames();
        let window = params.generate_window();

        if num_frames == 0 {
            return AudioData {
                samples: vec![],
                sample_rate: params.sample_rate,
                duration_seconds: 0.0,
            };
        }

        // Calculate output length (based on window_len, not n_fft)
        let output_length = if params.use_center {
            let padded_length = (num_frames - 1) * hop + window_len;
            padded_length.saturating_sub(window_len)
        } else {
            (num_frames - 1) * hop + window_len
        };

        // Phase 1: Parallel IFFT for each frame
        let frame_results: Vec<(usize, Vec<f32>)> = (0..num_frames)
            .into_par_iter()
            .map(|frame_idx| {
                let frame = &spectrogram.frames[frame_idx];
                let mut planner = RealFftPlanner::<f32>::new();
                let ifft = planner.plan_fft_inverse(n_fft);

                let mut spectrum = ifft.make_input_vec();
                let mut time_buffer = ifft.make_output_vec();

                // Build filtered spectrum
                // First, determine which bins to include based on frequency range
                let mut bin_mags: Vec<(usize, f32)> = Vec::new();

                for (i, (&mag, &freq)) in frame.magnitudes.iter()
                    .zip(frame.frequencies.iter())
                    .enumerate()
                {
                    if i < spectrum.len()
                        && freq >= view.recon_freq_min_hz
                        && freq <= view.recon_freq_max_hz
                    {
                        bin_mags.push((i, mag));
                    }
                }

                // Sort by magnitude descending, keep only top N
                bin_mags.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
                let keep_count = view.recon_freq_count.min(bin_mags.len());
                let kept_bins: Vec<usize> = bin_mags[..keep_count]
                    .iter()
                    .map(|&(idx, _)| idx)
                    .collect();

                // Zero the spectrum, then fill in kept bins
                for s in spectrum.iter_mut() {
                    *s = Complex::new(0.0, 0.0);
                }

                for &i in &kept_bins {
                    let mag = frame.magnitudes[i];
                    let phase = frame.phases[i];

                    if i == 0 || i == spectrum.len() - 1 {
                        spectrum[i] = Complex::new(mag * phase.cos(), 0.0);
                    } else {
                        spectrum[i] = Complex::from_polar(mag, phase);
                    }
                }

                ifft.process(&mut spectrum, &mut time_buffer)
                    .expect("IFFT processing failed");

                // Normalize IFFT output by FFT size
                let norm = 1.0 / n_fft as f32;
                for s in time_buffer.iter_mut() {
                    *s *= norm;
                }

                // Apply synthesis window to first window_len samples only
                // (discard zero-padding extension from IFFT output)
                let windowed: Vec<f32> = time_buffer.iter()
                    .take(window_len)
                    .zip(window.iter())
                    .map(|(&s, &w)| s * w)
                    .collect();

                let start_pos = frame_idx * hop;

                (start_pos, windowed)
            })
            .collect();

        // Phase 2: Sequential overlap-add
        let mut output = vec![0.0f32; output_length];
        let mut window_sum = vec![0.0f32; output_length];

        for (start_pos, windowed) in &frame_results {
            for (i, &sample) in windowed.iter().enumerate() {
                let pos = start_pos + i;
                if pos < output.len() {
                    output[pos] += sample;
                    window_sum[pos] += window[i] * window[i];
                }
            }
        }

        // Normalize by window sum, with adaptive threshold to prevent
        // edge amplification artifacts when few frames overlap.
        // With proper overlap (Hann 75%), window_sum is ~1.5 everywhere.
        // With few frames, edges have tiny window_sum → division amplifies noise.
        // Fix: use 10% of peak window_sum as minimum; silence below that.
        let max_wsum = window_sum.iter().copied().fold(0.0f32, f32::max);
        let threshold = (max_wsum * 0.1).max(1e-8);
        for i in 0..output.len() {
            if window_sum[i] >= threshold {
                output[i] /= window_sum[i];
            } else {
                // Insufficient overlap — fade to zero rather than amplify artifacts
                output[i] = 0.0;
            }
        }

        let duration_seconds = output.len() as f64 / params.sample_rate as f64;

        AudioData {
            samples: output,
            sample_rate: params.sample_rate,
            duration_seconds,
        }
    }
}
