use std::cell::RefCell;
use std::ops::Range;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use rayon::prelude::*;
use realfft::RealFftPlanner;
use rustfft::num_complex::Complex;

use crate::data::{compute_active_bins, AudioData, FftParams, Spectrogram, ViewState};
use crate::debug_flags;

thread_local! {
    /// Per-thread IFFT planner cache. Reusing one planner per rayon thread
    /// avoids re-planning for every frame during reconstruction.
    static IFFT_PLANNER: RefCell<RealFftPlanner<f32>> = RefCell::new(RealFftPlanner::new());
}

/// Reconstructs audio from spectrogram data with configurable frequency filtering.
pub struct Reconstructor;

impl Reconstructor {
    /// Reconstruct audio from all frames in a spectrogram.
    #[allow(dead_code)]
    pub fn reconstruct(
        spectrogram: &Spectrogram,
        params: &FftParams,
        view: &ViewState,
        cancel: &AtomicBool,
        progress: Option<&AtomicUsize>,
    ) -> AudioData {
        Self::reconstruct_range(
            spectrogram,
            params,
            view,
            0..spectrogram.num_frames(),
            cancel,
            progress,
        )
    }

    /// Reconstruct audio from a subset of frames identified by index range.
    ///
    /// This avoids cloning frames for time-filtered reconstruction: the caller
    /// computes the index range on the main thread and passes it here (zero-copy).
    /// For a 1000-frame spectrogram with 4096 bins, this saves ~49 MB of cloning.
    /// If `cancel` is set to true, processing stops early.
    /// If `progress` is provided, it is incremented after each frame completes.
    pub fn reconstruct_range(
        spectrogram: &Spectrogram,
        params: &FftParams,
        view: &ViewState,
        frame_range: Range<usize>,
        cancel: &AtomicBool,
        progress: Option<&AtomicUsize>,
    ) -> AudioData {
        let hop = params.hop_length();
        let window_len = params.window_length;
        let n_fft = params.n_fft_padded();
        let num_frames = frame_range.len();
        let window = params.generate_window();

        if num_frames == 0 {
            return AudioData {
                samples: Arc::new(vec![]),
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

        dbg_log!(
            debug_flags::SINGLE_FRAME_DBG,
            "SingleFrame",
            "Reconstruct start: frame_range={}..{} num_frames={} window_len={} hop={} n_fft={} zero_pad={} center={} output_len={}",
            frame_range.start,
            frame_range.end,
            num_frames,
            window_len,
            hop,
            n_fft,
            params.zero_pad_factor,
            params.use_center,
            output_length
        );

        // Phase 1: Parallel IFFT for each frame in the range.
        // Cancelled frames return None and are filtered out.
        let frame_indices: Vec<usize> = frame_range.collect();
        let frame_results: Vec<(usize, Vec<f32>)> = frame_indices
            .par_iter()
            .enumerate()
            .filter_map(|(local_idx, &global_idx)| {
                // Check cancellation before expensive work
                if cancel.load(Ordering::Relaxed) {
                    return None;
                }

                let frame = &spectrogram.frames[global_idx];
                let ifft = IFFT_PLANNER.with(|p| p.borrow_mut().plan_fft_inverse(n_fft));

                let mut spectrum = ifft.make_input_vec();
                let mut time_buffer = ifft.make_output_vec();

                // Determine active bins using shared logic (same as renderer).
                let active = compute_active_bins(
                    &frame.magnitudes,
                    &spectrogram.frequencies,
                    view.recon_freq_min_hz,
                    view.recon_freq_max_hz,
                    view.recon_freq_count,
                );

                // Zero the spectrum, then fill in active bins
                for s in spectrum.iter_mut() {
                    *s = Complex::new(0.0, 0.0);
                }

                for (i, &is_active) in active.iter().enumerate() {
                    if !is_active || i >= spectrum.len() {
                        continue;
                    }
                    let mag = frame.magnitudes[i];
                    let phase = frame.phases[i];

                    // Undo the forward-pass scaling to recover raw spectrum values.
                    // Forward pass stored: mag = (|X[k]| / N) * amplitude_scale
                    //   DC/Nyquist (amplitude_scale=1): mag = |X[k]| / N  -> recover: mag * N
                    //   Other bins (amplitude_scale=2):  mag = |X[k]| * 2 / N -> recover: mag * N / 2
                    let raw_mag = if i == 0 || i == spectrum.len() - 1 {
                        mag * n_fft as f32 // undo /N only
                    } else {
                        mag * n_fft as f32 / 2.0 // undo /N and *2
                    };

                    if i == 0 || i == spectrum.len() - 1 {
                        // DC and Nyquist bins are real-valued
                        spectrum[i] = Complex::new(raw_mag * phase.cos(), 0.0);
                    } else {
                        spectrum[i] = Complex::from_polar(raw_mag, phase);
                    }
                }

                ifft.process(&mut spectrum, &mut time_buffer)
                    .expect("IFFT processing failed");

                // realfft's inverse produces N * x[n], so divide by N
                let norm = 1.0 / n_fft as f32;
                for s in time_buffer.iter_mut() {
                    *s *= norm;
                }

                // Apply synthesis window to first window_len samples only
                // (discard zero-padding extension from IFFT output)
                let windowed: Vec<f32> = time_buffer
                    .iter()
                    .take(window_len)
                    .zip(window.iter())
                    .map(|(&s, &w)| s * w)
                    .collect();

                if let Some(ctr) = progress {
                    ctr.fetch_add(1, Ordering::Relaxed);
                }

                // Use local index for overlap-add positioning
                let start_pos = local_idx * hop;

                Some((start_pos, windowed))
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
        let first_above_threshold = window_sum.iter().position(|&v| v >= threshold);
        let last_above_threshold = window_sum.iter().rposition(|&v| v >= threshold);
        for i in 0..output.len() {
            if window_sum[i] >= threshold {
                output[i] /= window_sum[i];
            } else {
                // Insufficient overlap — fade to zero rather than amplify artifacts
                output[i] = 0.0;
            }
        }

        let left_zeroed = first_above_threshold.unwrap_or(output.len());
        let right_zeroed = last_above_threshold
            .map(|idx| output.len().saturating_sub(idx + 1))
            .unwrap_or(output.len());
        let kept_len = match (first_above_threshold, last_above_threshold) {
            (Some(a), Some(b)) if b >= a => b - a + 1,
            _ => 0,
        };

        dbg_log!(
            debug_flags::SINGLE_FRAME_DBG,
            "SingleFrame",
            "Reconstruct window_sum: max={:.6} threshold={:.6} first_keep={:?} last_keep={:?} left_zeroed={} right_zeroed={} kept_len={} duration={:.6}s",
            max_wsum,
            threshold,
            first_above_threshold,
            last_above_threshold,
            left_zeroed,
            right_zeroed,
            kept_len,
            output.len() as f64 / params.sample_rate as f64
        );

        if num_frames == 1 {
            let left_sec = left_zeroed as f64 / params.sample_rate as f64;
            let right_sec = right_zeroed as f64 / params.sample_rate as f64;
            dbg_log!(
                debug_flags::SINGLE_FRAME_DBG,
                "SingleFrame",
                "One-frame recon summary: left_zeroed={:.6}s right_zeroed={:.6}s kept={:.6}s of total={:.6}s window_type={:?}",
                left_sec,
                right_sec,
                kept_len as f64 / params.sample_rate as f64,
                output.len() as f64 / params.sample_rate as f64,
                params.window_type
            );
        }

        let duration_seconds = output.len() as f64 / params.sample_rate as f64;

        AudioData {
            samples: Arc::new(output),
            sample_rate: params.sample_rate,
            duration_seconds,
        }
    }
}
