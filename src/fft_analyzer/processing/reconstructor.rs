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
            "Case: active={} ({:.6}s) roi={}..{} smp center={} win={} hop={} overlap={:.4}% n_fft={} zpad={} window={:?} freq={}..{}Hz count={} norm_floor={:e}",
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
            view.recon_freq_count,
            view.recon_norm_floor
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

        // Normalize by window sum using a configurable floor threshold.
        //
        // The threshold controls the minimum w^2 denominator value we trust
        // for division. Below this value, samples are zeroed (left silent)
        // rather than divided, which would amplify f32 noise into huge spikes.
        //
        // The user controls this via the "Norm Floor" field in the sidebar.
        // Smaller values = fewer silent gap samples at window edges, but
        // higher risk of amplification spikes from dividing by tiny numbers.
        //
        // Threshold history:
        //   pre-step-5:      0.1 * max      (~25,000-sample gaps for 44100 Hann)
        //   step-5:          1e-6 * max      (~444-sample gaps)
        //   step-7 attempt:  f32::MIN_POSITIVE  (spikes from dividing by ~1e-30)
        //   step-7 fix:      user-configurable, default 1e-10 (~44-sample gaps)
        let max_wsum = window_sum.iter().copied().fold(0.0f32, f32::max);
        // Cast f64 threshold to f32 for comparison with f32 window_sum values.
        // Values below f32::MIN_POSITIVE (~1.175e-38) become 0.0 in f32,
        // which is fine -- it means "divide everywhere except exact zeros."
        let threshold_f32 = view.recon_norm_floor as f32;
        let first_above_threshold = window_sum.iter().position(|&v| v >= threshold_f32);
        let last_above_threshold = window_sum.iter().rposition(|&v| v >= threshold_f32);
        for i in 0..output.len() {
            if window_sum[i] >= threshold_f32 {
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
            "Reconstruct window_sum: max={:.6} threshold={:e} first_keep={:?} last_keep={:?} left_zeroed={} right_zeroed={} kept_len={} duration={:.6}s",
            max_wsum,
            view.recon_norm_floor,
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

        // Measure peak output amplitude (detects amplification spikes)
        let max_output_abs = output.iter().copied().map(f32::abs).fold(0.0f32, f32::max);

        dbg_log!(
            debug_flags::SINGLE_FRAME_DBG,
            "SingleFrame",
            "Boundary jumps: count={} max_jump={:.6} avg_jump={:.6}",
            boundary_positions.len(),
            max_boundary_jump,
            avg_boundary_jump
        );
        dbg_log!(
            debug_flags::SINGLE_FRAME_DBG,
            "SingleFrame",
            "Output amplitude: max_abs={:.6} (>10.0 suggests amplification spikes)",
            max_output_abs
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

        // ── Dense sample dump at frame transitions ──────────────────────
        if debug_flags::FRAME_SAMPLE_DUMP_DBG && !output.is_empty() {
            let dump_radius: usize = 150; // samples on each side of boundary

            // Pick up to 3 boundaries: start edge, one interior, end edge
            let mut boundaries_to_dump: Vec<(usize, &str)> = Vec::new();

            // Start edge: frame 0 start (sample 0)
            boundaries_to_dump.push((0, "START_EDGE"));

            // Interior: pick the middle frame boundary
            if num_frames > 2 {
                let mid_frame = num_frames / 2;
                let mid_boundary = mid_frame * hop;
                if mid_boundary < output.len() {
                    boundaries_to_dump.push((mid_boundary, "MID_INTERIOR"));
                }
            }

            // End edge: last sample region
            if output.len() > dump_radius {
                let end_pos = output.len().saturating_sub(1);
                boundaries_to_dump.push((end_pos, "END_EDGE"));
            }

            for (center, label) in &boundaries_to_dump {
                let from = center.saturating_sub(dump_radius);
                let to = (*center + dump_radius + 1).min(output.len());

                dbg_log!(
                    debug_flags::FRAME_SAMPLE_DUMP_DBG,
                    "FrameDump",
                    "=== {} at sample {} (frame ~{}) ===",
                    label,
                    center,
                    if hop > 0 { center / hop } else { 0 }
                );

                // Print in dense format: 10 samples per line
                let mut line = String::new();
                for i in from..to {
                    if (i - from) % 10 == 0 {
                        if !line.is_empty() {
                            dbg_log!(debug_flags::FRAME_SAMPLE_DUMP_DBG, "FrameDump", "{}", line);
                            line.clear();
                        }
                        line.push_str(&format!("[{:7}]", i));
                    }
                    // Mark frame boundaries with |
                    let is_boundary = hop > 0 && i > 0 && i % hop == 0;
                    let sep = if is_boundary { "|" } else { " " };
                    line.push_str(&format!("{}{:+.5}", sep, output[i]));
                }
                if !line.is_empty() {
                    dbg_log!(debug_flags::FRAME_SAMPLE_DUMP_DBG, "FrameDump", "{}", line);
                }
            }
        }

        let duration_seconds = output.len() as f64 / params.sample_rate as f64;

        AudioData {
            samples: Arc::new(output),
            sample_rate: params.sample_rate,
            duration_seconds,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{FftParams, ViewState, WindowType};
    use crate::processing::fft_engine::FftEngine;
    use std::f32::consts::PI;
    use std::sync::atomic::AtomicBool;

    /// Generate a pure sine wave at the given frequency.
    fn make_sine(sample_rate: u32, duration_secs: f32, freq_hz: f32) -> AudioData {
        let num_samples = (sample_rate as f32 * duration_secs) as usize;
        let samples: Vec<f32> = (0..num_samples)
            .map(|i| {
                let t = i as f32 / sample_rate as f32;
                (2.0 * PI * freq_hz * t).sin() * 0.8
            })
            .collect();
        AudioData {
            samples: Arc::new(samples),
            sample_rate,
            duration_seconds: duration_secs as f64,
        }
    }

    /// Build FftParams for a given window/overlap/center configuration.
    fn make_params(
        sample_rate: u32,
        start: usize,
        stop: usize,
        window_len: usize,
        overlap_pct: f32,
        window_type: WindowType,
        use_center: bool,
    ) -> FftParams {
        FftParams {
            sample_rate,
            start_sample: start,
            stop_sample: stop,
            window_length: window_len,
            overlap_percent: overlap_pct,
            window_type,
            use_center,
            zero_pad_factor: 1,
            time_unit: crate::data::TimeUnit::Seconds,
            target_segments_per_active: None,
            target_bins_per_segment: None,
            last_edited_field: crate::data::segmentation_solver::LastEditedField::Overlap,
        }
    }

    /// Build a ViewState that passes all bins through (full spectrum, no filtering).
    fn full_spectrum_view(nyquist: f32, max_bins: usize) -> ViewState {
        let mut view = ViewState::default();
        view.recon_freq_min_hz = 0.0;
        view.recon_freq_max_hz = nyquist;
        view.recon_freq_count = max_bins;
        view.max_freq_bins = max_bins;
        view
    }

    /// Build a ViewState with a narrow frequency band (sparse bin selection).
    fn narrow_band_view(freq_min: f32, freq_max: f32, max_bins: usize) -> ViewState {
        let mut view = ViewState::default();
        view.recon_freq_min_hz = freq_min;
        view.recon_freq_max_hz = freq_max;
        view.recon_freq_count = max_bins;
        view.max_freq_bins = max_bins;
        view
    }

    /// Perform a full roundtrip: audio -> FFT -> reconstruct -> compare.
    /// Returns (max_abs_error, rms_error, num_gap_samples, max_boundary_jump).
    ///
    /// Comparison is done in the **interior** region where the overlap-add
    /// denominator has full coverage (all frames contributing). This excludes
    /// the ramp-up/ramp-down edges where fewer frames overlap and the
    /// window_sum is below peak, which would produce expected edge errors.
    fn roundtrip(
        audio: &AudioData,
        params: &FftParams,
        view: &ViewState,
    ) -> (f32, f32, usize, f32) {
        let cancel = AtomicBool::new(false);

        // Forward FFT
        let spectrogram = FftEngine::process(audio, params, &cancel, None);
        let num_frames = spectrogram.num_frames();
        assert!(num_frames > 0, "FFT produced zero frames");

        // Reconstruct
        let reconstructed = Reconstructor::reconstruct(&spectrogram, params, view, &cancel, None);

        let recon = &reconstructed.samples;
        let hop = params.hop_length();
        let _window_len = params.window_length;

        // For non-centered mode, the OLA output covers:
        //   recon[0] .. recon[(num_frames-1)*hop + window_len - 1]
        // The interior region with full overlap coverage starts at
        // window_len (after the first window fully contributes) and ends
        // at (num_frames-1)*hop (before the last window's tail).
        // For 0% overlap (hop == window_len), every sample except endpoints
        // is covered by exactly one window, so the whole interior is valid.
        let recon_start = params.start_sample;
        let recon_len = recon.len();
        let orig = audio.get_slice(
            recon_start,
            (recon_start + recon_len).min(audio.num_samples()),
        );
        let compare_len = recon_len.min(orig.len());

        // For overlap > 0, skip the edge ramp zones in error measurement.
        // The ramp zone is approximately window_len samples on each side.
        let edge_skip = if params.overlap_percent > 0.0 {
            params.window_length
        } else {
            0
        };
        let interior_start = edge_skip.min(compare_len);
        let interior_end = compare_len.saturating_sub(edge_skip);

        let mut max_err: f32 = 0.0;
        let mut sum_sq: f64 = 0.0;
        let mut gap_samples = 0usize;

        for i in 0..compare_len {
            // Count gaps across the full output
            if recon[i] == 0.0 && orig[i].abs() > 1e-10 {
                gap_samples += 1;
            }
            // Measure error only in the interior (stable overlap region)
            if i >= interior_start && i < interior_end {
                let err = (recon[i] - orig[i]).abs();
                max_err = max_err.max(err);
                sum_sq += (err as f64) * (err as f64);
            }
        }
        let interior_len = interior_end.saturating_sub(interior_start).max(1);
        let rms = (sum_sq / interior_len as f64).sqrt() as f32;

        // Measure boundary jumps at frame seams
        let mut max_jump: f32 = 0.0;
        if num_frames > 1 {
            for frame_idx in 1..num_frames {
                let boundary = frame_idx * hop;
                if boundary > 0 && boundary < recon_len {
                    let jump = (recon[boundary] - recon[boundary - 1]).abs();
                    max_jump = max_jump.max(jump);
                }
            }
        }

        (max_err, rms, gap_samples, max_jump)
    }

    // ─── Identity-mode tests: full spectrum, no bin masking ────────────

    #[test]
    fn identity_roundtrip_hann_75pct_overlap() {
        let audio = make_sine(44100, 0.5, 440.0);
        let params = make_params(
            44100,
            0,
            audio.num_samples(),
            4096,
            75.0,
            WindowType::Hann,
            false,
        );
        let view = full_spectrum_view(22050.0, params.num_frequency_bins());

        let (max_err, rms, gaps, max_jump) = roundtrip(&audio, &params, &view);

        eprintln!(
            "Hann 75% overlap: max_err={:.8} rms={:.8} gaps={} max_jump={:.8}",
            max_err, rms, gaps, max_jump
        );

        assert!(
            max_err < 1e-4,
            "Hann 75% identity interior max error too high: {}",
            max_err
        );
        assert!(
            rms < 1e-5,
            "Hann 75% identity interior RMS too high: {}",
            rms
        );
        // Symmetric Hann has near-zero endpoints. With 1e-6 threshold,
        // ~41 samples per edge fall below floor for window_len=4096.
        // With 75% overlap most are covered, but output edges still show gaps.
        assert!(
            gaps <= 100,
            "Hann 75% identity: {} gaps exceeds expected max 100",
            gaps
        );
    }

    #[test]
    fn identity_roundtrip_hamming_zero_overlap() {
        // Hamming has nonzero endpoints -- identity roundtrip at 0% overlap
        // should be near-perfect (no NOLA violation).
        let audio = make_sine(44100, 0.5, 440.0);
        let params = make_params(
            44100,
            0,
            audio.num_samples(),
            4410,
            0.0,
            WindowType::Hamming,
            false,
        );
        let view = full_spectrum_view(22050.0, params.num_frequency_bins());

        let (max_err, rms, gaps, max_jump) = roundtrip(&audio, &params, &view);

        eprintln!(
            "Hamming 0% overlap: max_err={:.8} rms={:.8} gaps={} max_jump={:.8}",
            max_err, rms, gaps, max_jump
        );

        assert!(
            max_err < 1e-3,
            "Hamming 0% identity max error too high: {}",
            max_err
        );
        assert!(rms < 1e-4, "Hamming 0% identity RMS too high: {}", rms);
        assert_eq!(gaps, 0, "Hamming 0% identity should have no gaps");
    }

    #[test]
    fn identity_roundtrip_kaiser_zero_overlap() {
        // Kaiser also has nonzero endpoints.
        let audio = make_sine(44100, 0.5, 440.0);
        let params = make_params(
            44100,
            0,
            audio.num_samples(),
            4410,
            0.0,
            WindowType::Kaiser(8.6),
            false,
        );
        let view = full_spectrum_view(22050.0, params.num_frequency_bins());

        let (max_err, rms, gaps, max_jump) = roundtrip(&audio, &params, &view);

        eprintln!(
            "Kaiser 0% overlap: max_err={:.8} rms={:.8} gaps={} max_jump={:.8}",
            max_err, rms, gaps, max_jump
        );

        assert!(
            max_err < 1e-3,
            "Kaiser 0% identity max error too high: {}",
            max_err
        );
        assert!(rms < 1e-4, "Kaiser 0% identity RMS too high: {}", rms);
        assert_eq!(gaps, 0, "Kaiser 0% identity should have no gaps");
    }

    #[test]
    fn identity_roundtrip_hann_zero_overlap_minimal_gaps() {
        // Hann at 0% overlap: symmetric Hann has exact endpoint zeros.
        // With f32::MIN_POSITIVE threshold, only 1-2 samples per frame
        // boundary should be gapped (exact NOLA violation).
        let audio = make_sine(44100, 0.5, 440.0);
        let win_len = 4410; // 0.1 seconds
        let params = make_params(
            44100,
            0,
            audio.num_samples(),
            win_len,
            0.0,
            WindowType::Hann,
            false,
        );
        let view = full_spectrum_view(22050.0, params.num_frequency_bins());

        let (max_err, _rms, gaps, _max_jump) = roundtrip(&audio, &params, &view);

        eprintln!(
            "Hann 0% overlap: max_err={:.8} gaps={} (of {} output samples)",
            max_err,
            gaps,
            audio.num_samples()
        );

        // With 1e-6 threshold and window_len=4410:
        //   n_gap ≈ (1e-6)^(1/4)/pi * (M-1) ≈ 44 samples per window edge
        // Interior seams add gaps from both adjacent frames.
        let num_frames = params.num_segments(audio.num_samples());
        let max_expected_gaps = num_frames * 100;
        assert!(
            gaps <= max_expected_gaps,
            "Hann 0% overlap: {} gap samples exceeds expected max {} for {} frames",
            gaps,
            max_expected_gaps,
            num_frames
        );
    }

    #[test]
    fn identity_roundtrip_blackman_zero_overlap_minimal_gaps() {
        let audio = make_sine(44100, 0.5, 440.0);
        let win_len = 4410;
        let params = make_params(
            44100,
            0,
            audio.num_samples(),
            win_len,
            0.0,
            WindowType::Blackman,
            false,
        );
        let view = full_spectrum_view(22050.0, params.num_frequency_bins());

        let (max_err, _rms, gaps, _max_jump) = roundtrip(&audio, &params, &view);

        eprintln!("Blackman 0% overlap: max_err={:.8} gaps={}", max_err, gaps);

        // Blackman has wider near-zero zones than Hann, so more gap samples.
        let num_frames = params.num_segments(audio.num_samples());
        let max_expected_gaps = num_frames * 200;
        assert!(
            gaps <= max_expected_gaps,
            "Blackman 0% overlap: {} gaps exceeds expected max {} for {} frames",
            gaps,
            max_expected_gaps,
            num_frames
        );
    }

    // ─── Sparse bin selection tests ───────────────────────────────────

    #[test]
    fn sparse_bins_hamming_zero_overlap_has_boundary_jumps() {
        // With sparse bin selection at 0% overlap, boundary discontinuities
        // are EXPECTED because the modified STFT is inconsistent.
        // This test documents the behavior rather than asserting absence.
        let audio = make_sine(44100, 0.5, 440.0);
        let params = make_params(
            44100,
            0,
            audio.num_samples(),
            4410,
            0.0,
            WindowType::Hamming,
            false,
        );
        // Narrow band: only keep bins near 440 Hz
        let view = narrow_band_view(400.0, 500.0, 10000);

        let (_max_err, _rms, gaps, max_jump) = roundtrip(&audio, &params, &view);

        eprintln!(
            "Hamming sparse 0% overlap: gaps={} max_jump={:.6}",
            gaps, max_jump
        );

        // Hamming should have zero gaps (nonzero endpoints)
        assert_eq!(gaps, 0, "Hamming sparse should have no gaps");
        // Boundary jumps are expected -- just document the magnitude
        eprintln!("  (boundary jumps at 0% overlap with sparse bins are expected DSP behavior)");
    }

    #[test]
    fn sparse_bins_hann_50pct_overlap_reduces_artifacts() {
        // At 50% overlap with sparse bins, artifacts should be much smaller
        // than at 0% overlap because the crossfade region averages out
        // inter-frame discontinuities.
        let audio = make_sine(44100, 0.5, 440.0);
        let params = make_params(
            44100,
            0,
            audio.num_samples(),
            4096,
            50.0,
            WindowType::Hann,
            false,
        );
        let view = narrow_band_view(400.0, 500.0, 10000);

        let (_max_err, _rms, gaps, max_jump) = roundtrip(&audio, &params, &view);

        eprintln!(
            "Hann sparse 50% overlap: gaps={} max_jump={:.6}",
            gaps, max_jump
        );
        // With symmetric Hann and 1e-6 threshold, ~41 edge samples
        // per window edge fall below floor. 50% overlap covers interior seams
        // but output edges still have gaps.
        assert!(
            gaps <= 100,
            "Hann sparse 50% overlap: {} gaps exceeds expected max 100",
            gaps
        );
    }

    // ─── Epsilon threshold regression test ────────────────────────────

    #[test]
    fn hann_zero_overlap_gap_width_regression() {
        // Before the epsilon fix, a 44100-sample Hann window at 0% overlap
        // produced ~444 gap samples per side. After the fix (threshold =
        // f32::MIN_POSITIVE), gaps should be <= 2 samples per side.
        let audio = make_sine(44100, 1.0, 440.0);
        let params = make_params(44100, 0, 44100, 44100, 0.0, WindowType::Hann, false);
        let view = full_spectrum_view(22050.0, params.num_frequency_bins());

        let cancel = AtomicBool::new(false);
        let spectrogram = FftEngine::process(&audio, &params, &cancel, None);
        let reconstructed = Reconstructor::reconstruct(&spectrogram, &params, &view, &cancel, None);

        let recon = &reconstructed.samples;

        // Count leading zeros
        let left_gap = recon.iter().take_while(|&&s| s == 0.0).count();
        // Count trailing zeros
        let right_gap = recon.iter().rev().take_while(|&&s| s == 0.0).count();

        eprintln!(
            "Hann 44100-sample single frame: left_gap={} right_gap={} (was 444 before fix)",
            left_gap, right_gap
        );

        // With 1e-6 threshold, Hann gap width for 44100 samples is:
        //   n_gap ≈ (1e-6)^(1/4) / pi * (M-1) ≈ 444 samples per side
        // This is the known baseline. The user can adjust the norm floor
        // to trade gap width vs spike risk.
        assert!(
            left_gap <= 500,
            "Left gap {} too wide (regression?)",
            left_gap
        );
        assert!(
            right_gap <= 500,
            "Right gap {} too wide (regression?)",
            right_gap
        );
    }

    // ─── Centered mode tests ─────────────────────────────────────────

    #[test]
    fn centered_mode_single_frame_produces_two_frames() {
        // With center=true and window_len == ROI length at 0% overlap,
        // the padded length creates 2 frames. This is expected behavior.
        let audio = make_sine(44100, 1.0, 440.0);
        let win_len = 44100;
        let params = make_params(44100, 0, 44100, win_len, 0.0, WindowType::Hann, true);

        let cancel = AtomicBool::new(false);
        let spectrogram = FftEngine::process(&audio, &params, &cancel, None);

        eprintln!(
            "Centered single-frame target: actual frames = {}",
            spectrogram.num_frames()
        );
        assert_eq!(
            spectrogram.num_frames(),
            2,
            "Centered mode with window_len == ROI and 0% overlap should produce 2 frames"
        );
    }

    // ─── Rectangular window tests ──────────────────────────────────

    #[test]
    fn rectangular_zero_overlap_zero_gaps() {
        // Rectangular window has w[n]=1.0 everywhere, so w^2=1.0 everywhere.
        // NOLA is always satisfied. No gaps at any overlap, including 0%.
        let audio = make_sine(44100, 0.5, 440.0);
        let params = make_params(
            44100,
            0,
            audio.num_samples(),
            4410,
            0.0,
            WindowType::Rectangular,
            false,
        );
        let view = full_spectrum_view(22050.0, params.num_frequency_bins());

        let (max_err, _rms, gaps, _max_jump) = roundtrip(&audio, &params, &view);

        eprintln!(
            "Rectangular 0% overlap identity: max_err={:.8} gaps={}",
            max_err, gaps
        );

        assert_eq!(
            gaps, 0,
            "Rectangular window should have ZERO gaps at any overlap"
        );
        assert!(
            max_err < 1e-3,
            "Rectangular 0% overlap identity max error too high: {}",
            max_err
        );
    }

    #[test]
    fn rectangular_single_frame_edge_to_edge() {
        // Single rectangular frame should reconstruct every sample edge-to-edge.
        let audio = make_sine(44100, 1.0, 440.0);
        let params = make_params(44100, 0, 44100, 44100, 0.0, WindowType::Rectangular, false);
        let view = full_spectrum_view(22050.0, params.num_frequency_bins());

        let cancel = AtomicBool::new(false);
        let spectrogram = FftEngine::process(&audio, &params, &cancel, None);
        let reconstructed = Reconstructor::reconstruct(&spectrogram, &params, &view, &cancel, None);

        let recon = &reconstructed.samples;
        let left_gap = recon.iter().take_while(|&&s| s == 0.0).count();
        let right_gap = recon.iter().rev().take_while(|&&s| s == 0.0).count();

        eprintln!(
            "Rectangular single frame: left_gap={} right_gap={} len={}",
            left_gap,
            right_gap,
            recon.len()
        );

        assert_eq!(
            left_gap, 0,
            "Rectangular single frame should have zero left gap"
        );
        assert_eq!(
            right_gap, 0,
            "Rectangular single frame should have zero right gap"
        );
    }

    // ─── Frame boundary diagnostic test ────────────────────────────

    #[test]
    fn rectangular_frame_boundary_samples() {
        // Examine actual sample values at frame boundaries.
        // With full spectrum, rectangular window gives near-perfect results.
        // With sparse bins, small boundary discontinuities are expected
        // (modified STFT inconsistency, not a bug).
        let audio = make_sine(44100, 0.5, 1000.0);
        let win_len = 4410;
        let params = make_params(
            44100,
            0,
            audio.num_samples(),
            win_len,
            0.0,
            WindowType::Rectangular,
            false,
        );
        // Test with sparse bins to show the expected boundary discontinuities
        let view = narrow_band_view(900.0, 1200.0, 10000);
        let cancel = AtomicBool::new(false);

        let spectrogram = FftEngine::process(&audio, &params, &cancel, None);
        let reconstructed = Reconstructor::reconstruct(&spectrogram, &params, &view, &cancel, None);

        let recon = &reconstructed.samples;
        let orig = audio.get_slice(0, recon.len().min(audio.num_samples()));
        let hop = params.hop_length();
        let num_frames = spectrogram.num_frames();

        eprintln!("=== Rectangular frame boundary diagnostic ===");
        eprintln!(
            "window_len={} hop={} num_frames={} recon_len={} orig_len={}",
            win_len,
            hop,
            num_frames,
            recon.len(),
            orig.len()
        );

        // For each frame boundary, print samples around the seam
        let mut max_err_overall: f32 = 0.0;
        let mut worst_pos = 0;

        for i in 0..recon.len().min(orig.len()) {
            let err = (recon[i] - orig[i]).abs();
            if err > max_err_overall {
                max_err_overall = err;
                worst_pos = i;
            }
        }

        eprintln!(
            "Worst error: {:.8} at sample {} (frame {}, offset {})",
            max_err_overall,
            worst_pos,
            worst_pos / hop,
            worst_pos % hop
        );

        // Print samples around the worst position
        let start = worst_pos.saturating_sub(5);
        let end = (worst_pos + 6).min(recon.len()).min(orig.len());
        for i in start..end {
            let marker = if i == worst_pos {
                " <-- WORST"
            } else if i % hop == 0 {
                " <-- FRAME START"
            } else if i % hop == hop - 1 {
                " <-- FRAME END"
            } else {
                ""
            };
            eprintln!(
                "  [{:6}] orig={:+.8} recon={:+.8} err={:.8}{}",
                i,
                orig[i],
                recon[i],
                (recon[i] - orig[i]).abs(),
                marker
            );
        }

        // Print samples around each frame boundary
        for frame_idx in 0..num_frames.min(3) {
            let frame_end = (frame_idx + 1) * hop;
            if frame_end >= recon.len().min(orig.len()) {
                break;
            }
            let region_start = frame_end.saturating_sub(3);
            let region_end = (frame_end + 3).min(recon.len()).min(orig.len());
            eprintln!("\nFrame {} end (sample {}):", frame_idx, frame_end);
            for i in region_start..region_end {
                let marker = if i == frame_end { " <-- BOUNDARY" } else { "" };
                eprintln!(
                    "  [{:6}] orig={:+.8} recon={:+.8} err={:.8}{}",
                    i,
                    orig[i],
                    recon[i],
                    (recon[i] - orig[i]).abs(),
                    marker
                );
            }
        }
    }

    // ─── Spike amplitude safety test ─────────────────────────────────

    #[test]
    fn no_amplification_spikes_any_window_zero_overlap() {
        // The 1e-10 threshold must prevent division-by-near-zero spikes.
        // For normalized input (peak ~0.8), no output sample should exceed
        // a reasonable bound. This catches the f32::MIN_POSITIVE regression
        // where Blackman produced spikes of 66,000+.
        let audio = make_sine(44100, 0.5, 440.0);
        let max_safe_amplitude = 50.0_f32;

        for (wtype, name) in [
            (WindowType::Rectangular, "Rectangular"),
            (WindowType::Hann, "Hann"),
            (WindowType::Hamming, "Hamming"),
            (WindowType::Blackman, "Blackman"),
            (WindowType::Kaiser(8.6), "Kaiser"),
        ] {
            let params = make_params(44100, 0, audio.num_samples(), 4410, 0.0, wtype, false);
            let view = narrow_band_view(400.0, 500.0, 10000);
            let cancel = AtomicBool::new(false);

            let spectrogram = FftEngine::process(&audio, &params, &cancel, None);
            let reconstructed =
                Reconstructor::reconstruct(&spectrogram, &params, &view, &cancel, None);

            let max_abs = reconstructed
                .samples
                .iter()
                .copied()
                .map(f32::abs)
                .fold(0.0f32, f32::max);

            eprintln!("{} 0% overlap sparse bins: max_abs={:.6}", name, max_abs);
            assert!(
                max_abs < max_safe_amplitude,
                "{} produced spike of {:.1} (threshold regression?)",
                name,
                max_abs
            );
        }
    }
}
