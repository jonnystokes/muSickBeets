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

#[derive(Debug, Clone, Copy)]
struct CenteredCropPlan {
    raw_len: usize,
    keep_start: usize,
    keep_end: usize,
    crop_left: usize,
    crop_right: usize,
}

impl Reconstructor {
    fn centered_crop_plan(
        spectrogram: &Spectrogram,
        params: &FftParams,
        frame_range: Range<usize>,
    ) -> Option<CenteredCropPlan> {
        if frame_range.start >= frame_range.end || frame_range.end > spectrogram.frames.len() {
            return None;
        }

        let pad_left = (params.window_length / 2) as isize;
        let pad_right = (params.window_length - params.window_length / 2) as isize;
        let sr = params.sample_rate.max(1) as f64;

        let first_center =
            (spectrogram.frames[frame_range.start].time_seconds * sr).round() as isize;
        let last_center =
            (spectrogram.frames[frame_range.end - 1].time_seconds * sr).round() as isize;

        let raw_start = first_center - pad_left;
        let raw_end = last_center + pad_right;
        let raw_len = raw_end.saturating_sub(raw_start).max(0) as usize;

        let keep_start = raw_start.max(params.start_sample as isize) as usize;
        let keep_end = raw_end.min(params.stop_sample as isize).max(raw_start) as usize;

        let crop_left = (keep_start as isize - raw_start).max(0) as usize;
        let crop_right = (raw_end - keep_end as isize).max(0) as usize;

        Some(CenteredCropPlan {
            raw_len,
            keep_start,
            keep_end,
            crop_left,
            crop_right,
        })
    }

    pub fn reconstruction_start_sample(
        spectrogram: &Spectrogram,
        params: &FftParams,
        frame_range: Range<usize>,
    ) -> Option<usize> {
        if frame_range.start >= frame_range.end || frame_range.end > spectrogram.frames.len() {
            return None;
        }

        if params.use_center {
            Self::centered_crop_plan(spectrogram, params, frame_range).map(|p| p.keep_start)
        } else {
            let sr = params.sample_rate.max(1) as f64;
            Some((spectrogram.frames[frame_range.start].time_seconds * sr).round() as usize)
        }
    }

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

        let centered_crop = if params.use_center {
            Self::centered_crop_plan(spectrogram, params, frame_range.clone())
        } else {
            None
        };

        let active_samples = params.stop_sample.saturating_sub(params.start_sample);

        // Build the full raw overlap-add support first. For centered mode we
        // crop back to the actually covered unpadded support after OLA.
        let output_length = centered_crop
            .as_ref()
            .map(|p| p.raw_len)
            .unwrap_or_else(|| (num_frames - 1) * hop + window_len);

        if debug_flags::SINGLE_FRAME_DBG {
            eprintln!();
        }

        dbg_log!(
            debug_flags::SINGLE_FRAME_DBG,
            "SingleFrame",
            "------------------------------------------------------------"
        );
        dbg_log!(
            debug_flags::SINGLE_FRAME_DBG,
            "SingleFrame",
            "Case: active={} ({:.6}s) roi={}..{} smp center={} win={} hop={} overlap={:.4}% n_fft={} zpad={} window={:?} freq={}..{}Hz count={}",
            active_samples,
            active_samples as f64 / params.sample_rate.max(1) as f64,
            params.start_sample,
            params.stop_sample,
            params.use_center,
            window_len,
            hop,
            params.overlap_percent,
            n_fft,
            params.zero_pad_factor,
            params.window_type,
            view.recon_freq_min_hz,
            view.recon_freq_max_hz,
            view.recon_freq_count
        );
        dbg_log!(
            debug_flags::SINGLE_FRAME_DBG,
            "SingleFrame",
            "Reconstruct start: frame_range={}..{} num_frames={} window_len={} hop={} n_fft={} zero_pad={} center={} raw_output_len={}",
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
        let frame_results: Vec<(usize, Vec<f32>, usize)> = frame_indices
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
                let active_count = active.iter().filter(|&&b| b).count();

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

                Some((start_pos, windowed, active_count))
            })
            .collect();

        let active_min = frame_results.iter().map(|(_, _, c)| *c).min().unwrap_or(0);
        let active_max = frame_results.iter().map(|(_, _, c)| *c).max().unwrap_or(0);
        let active_avg = if frame_results.is_empty() {
            0.0
        } else {
            frame_results.iter().map(|(_, _, c)| *c as f64).sum::<f64>()
                / frame_results.len() as f64
        };

        dbg_log!(
            debug_flags::SINGLE_FRAME_DBG,
            "SingleFrame",
            "Active bins: min={} max={} avg={:.2} requested_freq_count={} freq_range={:.2}-{:.2}Hz",
            active_min,
            active_max,
            active_avg,
            view.recon_freq_count,
            view.recon_freq_min_hz,
            view.recon_freq_max_hz
        );

        // Phase 2: Sequential overlap-add
        let mut output = vec![0.0f32; output_length];
        let mut window_sum = vec![0.0f32; output_length];

        for (start_pos, windowed, _) in &frame_results {
            for (i, &sample) in windowed.iter().enumerate() {
                let pos = start_pos + i;
                if pos < output.len() {
                    output[pos] += sample;
                    window_sum[pos] += window[i] * window[i];
                }
            }
        }

        // Normalize by window sum using a tiny numerical epsilon only.
        // Standard weighted overlap-add / ISTFT practice is to divide wherever
        // the squared-window sum is nonzero, leaving only truly unsupported
        // samples at zero. Broad percentage-of-peak gating is not standard and
        // creates artificial cliff-drop blank edges in one-frame / low-overlap
        // cases.
        let max_wsum = window_sum.iter().copied().fold(0.0f32, f32::max);
        let threshold = (max_wsum * 1e-6).max(1e-8);
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

        let output = if let Some(plan) = centered_crop {
            let crop_end = output.len().saturating_sub(plan.crop_right);
            let cropped = if plan.crop_left < crop_end {
                output[plan.crop_left..crop_end].to_vec()
            } else {
                vec![]
            };

            dbg_log!(
                debug_flags::SINGLE_FRAME_DBG,
                "SingleFrame",
                "Centered crop: keep_start={} keep_end={} crop_left={} crop_right={} final_len={}",
                plan.keep_start,
                plan.keep_end,
                plan.crop_left,
                plan.crop_right,
                cropped.len()
            );
            cropped
        } else {
            output
        };

        let boundary_positions: Vec<usize> = if frame_results.len() <= 1 {
            Vec::new()
        } else if let Some(plan) = centered_crop {
            frame_results
                .iter()
                .skip(1)
                .map(|(start_pos, _, _)| start_pos.saturating_sub(plan.crop_left))
                .filter(|&pos| pos > 0 && pos < output.len())
                .collect()
        } else {
            frame_results
                .iter()
                .skip(1)
                .map(|(start_pos, _, _)| *start_pos)
                .filter(|&pos| pos > 0 && pos < output.len())
                .collect()
        };

        let mut max_boundary_jump = 0.0f32;
        let mut avg_boundary_jump = 0.0f64;
        for &pos in &boundary_positions {
            let jump = (output[pos] - output[pos - 1]).abs();
            max_boundary_jump = max_boundary_jump.max(jump);
            avg_boundary_jump += jump as f64;
        }
        if !boundary_positions.is_empty() {
            avg_boundary_jump /= boundary_positions.len() as f64;
        }

        dbg_log!(
            debug_flags::SINGLE_FRAME_DBG,
            "SingleFrame",
            "Boundary jumps: count={} max_jump={:.6} avg_jump={:.6}",
            boundary_positions.len(),
            max_boundary_jump,
            avg_boundary_jump
        );

        let mut runs: Vec<(usize, usize)> = Vec::new();
        let mut run_start: Option<usize> = None;
        for (i, &sample) in output.iter().enumerate() {
            if sample == 0.0 {
                if run_start.is_none() {
                    run_start = Some(i);
                }
            } else if let Some(start) = run_start.take() {
                runs.push((start, i));
            }
        }
        if let Some(start) = run_start {
            runs.push((start, output.len()));
        }

        let left_gap = runs
            .first()
            .filter(|&&(s, _)| s == 0)
            .map(|&(s, e)| e - s)
            .unwrap_or(0);
        let right_gap = runs
            .last()
            .filter(|&&(_, e)| e == output.len())
            .map(|&(s, e)| e - s)
            .unwrap_or(0);
        let interior_runs: Vec<(usize, usize)> = runs
            .iter()
            .copied()
            .filter(|&(s, e)| s > 0 && e < output.len())
            .collect();
        let max_interior_gap = interior_runs.iter().map(|&(s, e)| e - s).max().unwrap_or(0);

        dbg_log!(
            debug_flags::SINGLE_FRAME_DBG,
            "SingleFrame",
            "Gap runs: left={} right={} interior_count={} max_interior={} duration={:.6}s",
            left_gap,
            right_gap,
            interior_runs.len(),
            max_interior_gap,
            output.len() as f64 / params.sample_rate as f64
        );
        dbg_log!(
            debug_flags::SINGLE_FRAME_DBG,
            "SingleFrame",
            "------------------------------------------------------------"
        );
        if debug_flags::SINGLE_FRAME_DBG {
            eprintln!();
        }

        let duration_seconds = output.len() as f64 / params.sample_rate as f64;

        AudioData {
            samples: Arc::new(output),
            sample_rate: params.sample_rate,
            duration_seconds,
        }
    }
}
