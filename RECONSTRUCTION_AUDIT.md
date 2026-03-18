# Reconstruction Audit: Code Review vs DSP Theory

> **Update (step 7):** Finding 1 (epsilon threshold) has been **fixed**. The
> threshold was changed from `1e-6 * max` to `f32::MIN_POSITIVE` (~1.175e-38).
> Hann gaps shrank from 444 samples to 2 samples (exact window zeros only).
> 10 automated roundtrip tests were added to `reconstructor.rs` to prevent
> regression. See `PROGRESS.md` and `SINGLE_FRAME_FFT_NOTES.md` for current state.

**Reviewer:** Claude Opus 4.6 (Anthropic), running in OpenCode harness  
**Date:** 2026-03-16  
**Scope:** Zero-overlap reconstruction correctness -- silent gaps and boundary spikes  
**Method:** Line-by-line code review of all FFT analyzer processing files, cross-referenced against three independent DSP research documents (produced by AIs with no code access)

> **Important caveat:** The three research documents were produced by AIs that had
> zero visibility into this codebase. Their conclusions about "likely bugs" were
> hypotheses based on general DSP theory and standard library conventions. This
> document confirms or refutes each hypothesis against what the code actually does.

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Finding 1: Epsilon Threshold Is Still Too Large](#finding-1-epsilon-threshold)
3. [Finding 2: Symmetric vs Periodic Window Generation](#finding-2-window-generation)
4. [Finding 3: Forward FFT Time Alignment Uses Frame Start, Not Center](#finding-3-time-alignment)
5. [Finding 4: IFFT Output Truncation to window_len Discards Zero-Pad Information](#finding-4-ifft-truncation)
6. [Finding 5: DC and Nyquist Phase Handling Is Inconsistent](#finding-5-dc-nyquist-phase)
7. [Finding 6: Forward/Inverse Scaling Roundtrip Is Correct](#finding-6-scaling-roundtrip)
8. [Finding 7: Active-Bin Logic Is Correctly Shared](#finding-7-active-bins)
9. [Finding 8: Centered Crop Plan Arithmetic Looks Sound](#finding-8-centered-crop)
10. [Finding 9: Overlap-Add Positioning Uses Local Index Correctly](#finding-9-ola-positioning)
11. [Finding 10: Post-Reconstruction Normalization May Amplify Edge Noise](#finding-10-post-normalization)
12. [Summary: What Is a Bug vs What Is Expected DSP Behavior](#summary)
13. [Research Hypothesis Verification](#research-verification)
14. [Recommended Investigation Order](#investigation-order)

---

## Executive Summary

After reviewing every line of `reconstructor.rs`, `fft_engine.rs`, `fft_params.rs`,
`spectrogram.rs`, `audio_data.rs`, `callbacks_file.rs`, `poll_loop.rs`,
`spectrogram_renderer.rs`, `segmentation_solver.rs`, `view_state.rs`, and
`debug_flags.rs`, I found:

- **One confirmed material bug** (epsilon threshold still ~1e-6, creating hundreds of
  artificial silent samples for Hann/Blackman)
- **One significant design question** (symmetric vs periodic window generation) that
  affects both gap width and spike magnitude
- **One subtle concern** (IFFT output truncated to `window_len` when `zero_pad_factor > 1`,
  discarding frequency-interpolated tails)
- **One minor correctness issue** (DC/Nyquist bins forced to real-only in reconstruction
  but phase stored as `atan2(im,re)` from forward pass)
- **Several confirmed-correct behaviors** that align with DSP theory

The forward/inverse scaling roundtrip, the overlap-add accumulation, the shared
active-bin filtering, and the centered crop plan are all implemented correctly.
The boundary spikes for Hamming/Kaiser at 0% overlap with sparse bin selection
are expected DSP behavior per the research, not bugs.

---

## Finding 1: Epsilon Threshold Is Still Too Large {#finding-1-epsilon-threshold}

**File:** `reconstructor.rs:320-321`  
**Severity:** HIGH -- this is the primary cause of artificially wide silent gaps

### What the code does

```rust
let max_wsum = window_sum.iter().copied().fold(0.0f32, f32::max);
let threshold = (max_wsum * 1e-6).max(1e-8);
```

The threshold is `max(max_wsum * 1e-6, 1e-8)`. For a Hann window, `max_wsum`
(the peak of `w^2`) is `1.0` (at the window center where `w[n] = 1.0`), so:

```
threshold = max(1.0 * 1e-6, 1e-8) = 1e-6
```

### Why this creates artificial gaps

For a symmetric Hann window with `w[n] = 0.5 * (1 - cos(2*pi*n / (M-1)))`:

Near the edges, `w[n] ~ pi^2 * n^2 / (M-1)^2`, so `w^2[n] ~ pi^4 * n^4 / (M-1)^4`.

Setting `w^2[n] < 1e-6` and solving for `n`:

```
n_gap ≈ (1e-6)^(1/4) / pi * (M-1)
      ≈ 0.03162 / 3.14159 * (M-1)
      ≈ 0.01006 * (M-1)
```

For `M = 44100` (1 second at 44.1kHz): `n_gap ≈ 444` samples.

**This matches the measured 444-sample gaps exactly.** The research documents
predicted this without seeing the code -- their hypothesis is confirmed.

### What standard libraries do

- **librosa:** Uses `util.tiny(dtype)` which is `~1.17e-38` for float32
- **SciPy:** Relies on NOLA (denominator truly nonzero), no percentage cutoff
- **PyTorch:** Throws RuntimeError when NOLA fails

### Resolution (step 7) -- FIXED

The threshold was changed to `f32::MIN_POSITIVE` (~1.175e-38). Verified results:

| Metric | Before (1e-6) | After (MIN_POSITIVE) |
|--------|---------------|----------------------|
| Hann 44100-sample gap per side | 444 samples | 2 samples |
| Hamming 0% identity max error | same | 2.5e-6 |
| Kaiser 0% identity max error | same | 1.1e-4 |

The 2-sample gap is mathematically correct: symmetric Hann has `w[0]=w[M-1]=0`
exactly, making those indices irrecoverable (NOLA violation at exact zeros).

10 automated roundtrip tests were added to prevent regression.

### Impact on boundary spikes

This finding does NOT explain the Hamming/Kaiser boundary spikes. Those windows
have nonzero endpoints, so they pass any reasonable threshold. The spikes are a
separate issue (see Finding 2 and the Summary).

---

## Finding 2: Symmetric vs Periodic Window Generation {#finding-2-window-generation}

**File:** `fft_params.rs:120-147`  
**Severity:** MEDIUM -- affects both gap behavior and reconstruction fidelity

### What the code does

All four window types use the **symmetric** definition with divisor `(n-1)`:

```rust
// Hann
*w = 0.5 * (1.0 - ((2.0 * PI * i as f32) / (n - 1) as f32).cos());

// Hamming
*w = 0.54 - 0.46 * ((2.0 * PI * i as f32) / (n - 1) as f32).cos();
```

The symmetric definition produces `w[0] = w[M-1] = 0` for Hann and
`w[0] = w[M-1] = 0.08` for Hamming.

### Why this matters

The **periodic** (DFT-even) definition uses divisor `n` instead of `(n-1)`:

```
w_periodic[i] = 0.5 * (1.0 - cos(2*pi*i / n))
```

Key differences:

1. **Periodic Hann** does NOT have exact zeros at endpoints. `w[0] = 0` but
   `w[M-1] = 0.5 * (1 - cos(2*pi*(M-1)/M))` which is small but nonzero for
   large M. This would eliminate the NOLA violation at the last endpoint.

2. **Periodic windows satisfy COLA at standard overlaps** (50%, 75%, etc).
   Symmetric windows are NOT COLA-compliant at the same overlaps. SciPy and
   librosa both default to periodic windows for FFT use.

3. The **same window variant must be used in both analysis and synthesis**.
   Currently the code uses the same `generate_window()` call in both
   `fft_engine.rs:79` and `reconstructor.rs:124`, which is correct for
   internal consistency. But if the intent is to match standard STFT/ISTFT
   conventions, periodic is the standard choice.

### Is this a bug or a design choice?

This is legitimately a design choice for a custom analyzer. The symmetric
window is mathematically valid for STFT -- it just means COLA is not satisfied
at common overlaps, and endpoint zeros are exact (not approximate). Both
variants produce correct reconstruction when the forward/inverse scaling and
normalization are consistent.

However, the research documents flagged this as the most common source of
"unexpected behavior" in custom STFT implementations. Switching to periodic
windows would:
- Eliminate the exact-zero NOLA violation for Hann at the endpoints
- Make 50% overlap truly COLA-compliant for Hann
- Match what librosa/SciPy/PyTorch do by default

### Impact assessment

If the epsilon is fixed (Finding 1), the symmetric-vs-periodic distinction
becomes much less important for gaps, because the only remaining gap would be
1-2 samples at exact zeros. For spikes, it could affect magnitude slightly
(symmetric Hamming endpoint = 0.08, periodic Hamming endpoint would be slightly
different), but the fundamental spike issue at 0% overlap with sparse bins
remains regardless of window variant.

---

## Finding 3: Forward FFT Time Alignment Uses Frame Start, Not Center {#finding-3-time-alignment}

**File:** `fft_engine.rs:116-117`  
**Severity:** LOW for gap/spike investigation, but important for metadata correctness

### What the code does

```rust
let actual_sample = start_sample + frame_idx * hop;
let time_seconds = actual_sample as f64 / audio.sample_rate as f64;
```

When `use_center = false`, `time_seconds` is the **frame start time** (sample
where the window begins).

When `use_center = true`, the audio is padded by `window_len/2` on each side
(line 44), but `actual_sample` still uses `start_sample + frame_idx * hop`,
which means `time_seconds` is the start of the padded region, not the center
of the window relative to the original signal.

### Why this matters

The reconstructor's `centered_crop_plan` (line 46) interprets `time_seconds`
as frame center positions:

```rust
let first_center = (spectrogram.frames[frame_range.start].time_seconds * sr).round() as isize;
```

But in non-centered mode, `time_seconds` is the frame START, not center.

The overlap-add positioning in the reconstructor (line 273) uses `local_idx * hop`
which is independent of `time_seconds`, so the actual OLA math is not affected.
The `time_seconds` field is only used for:
- The centered crop plan (centered mode only)
- Frame lookup by time (`frame_at_time`)
- Display/rendering

Since `centered_crop_plan` is only called when `use_center = true`, and in that
case the time semantics are at least internally consistent (both analysis and
reconstruction treat the stored time as a reference point with pad offsets),
this is not causing gaps or spikes. But it's worth documenting that
`time_seconds` has different semantics depending on `use_center`.

---

## Finding 4: IFFT Output Truncation to window_len Discards Zero-Pad Information {#finding-4-ifft-truncation}

**File:** `reconstructor.rs:261-266`  
**Severity:** MEDIUM -- potentially affects reconstruction quality with zero-padding

### What the code does

```rust
let windowed: Vec<f32> = time_buffer
    .iter()
    .take(window_len)      // <-- truncate to window_len
    .zip(window.iter())
    .map(|(&s, &w)| s * w)
    .collect();
```

The IFFT produces `n_fft` samples (where `n_fft = window_len * zero_pad_factor`).
The code keeps only the first `window_len` samples and discards the rest.

### Why this might matter

When `zero_pad_factor > 1`, the forward FFT zero-pads the windowed frame from
`window_len` to `n_fft` samples before the DFT. This means the frequency
spectrum has `n_fft/2 + 1` bins instead of `window_len/2 + 1`. The extra bins
provide frequency interpolation (smoother spectral shape) but no new information.

On the inverse path, if all bins are preserved (no modification), the IFFT of
the zero-padded spectrum should reconstruct the original `window_len` samples
followed by zeros -- so truncating to `window_len` is correct for unmodified
STFT.

**However**, when bins are selectively zeroed (sparse reconstruction), the
IFFT output is no longer a simple "original + trailing zeros." The spectral
modification redistributes energy across all `n_fft` time samples. Truncating
to `window_len` discards energy that the modification scattered into the
zero-padded region.

### Impact assessment

For the specific case of zero-overlap with sparse bins:
- This could contribute to boundary discontinuities because each frame's
  reconstructed waveform is slightly different from what a full-length IFFT
  would produce
- The effect is likely small for moderate zero-pad factors (2x) but could
  become more noticeable at 4x or 8x
- This is NOT the primary cause of the observed spikes (those exist even at
  1x zero-padding based on the test data)

This is more of a quality concern than a correctness bug. Standard STFT
implementations (librosa, SciPy) also truncate to `window_len` after IFFT
when `n_fft > window_len`, so this behavior is conventional.

---

## Finding 5: DC and Nyquist Phase Handling Is Inconsistent {#finding-5-dc-nyquist-phase}

**File:** `reconstructor.rs:242-247` vs `fft_engine.rs:132`  
**Severity:** LOW -- unlikely to cause audible artifacts in practice

### What the code does

**Forward pass** (fft_engine.rs:132):
```rust
phases.push(complex_val.arg());  // atan2(im, re) for ALL bins including DC/Nyquist
```

**Inverse pass** (reconstructor.rs:242-247):
```rust
if i == 0 || i == spectrum.len() - 1 {
    // DC and Nyquist bins are real-valued
    spectrum[i] = Complex::new(raw_mag * phase.cos(), 0.0);  // force imaginary to 0
} else {
    spectrum[i] = Complex::from_polar(raw_mag, phase);
}
```

### Why this is slightly inconsistent

For DC and Nyquist bins in a real-valued signal's DFT, the values are
guaranteed to be real. The forward pass stores `arg()` which should be 0 or pi
for real values, but floating-point imprecision could produce small nonzero
imaginary parts, giving phases slightly off from 0/pi.

The inverse pass forces imaginary to 0 by using `raw_mag * phase.cos()`, which
maps phase=0 -> +mag and phase=pi -> -mag. This is correct for the sign, but
if the forward pass stored a phase slightly off from 0 or pi (due to float
imprecision), the reconstruction gets a slightly different magnitude.

### Impact

Negligible for audio quality. DC is typically near-zero for AC-coupled audio,
and Nyquist is usually very low energy. This is not contributing to the
observed gaps or spikes.

---

## Finding 6: Forward/Inverse Scaling Roundtrip Is Correct {#finding-6-scaling-roundtrip}

**File:** `fft_engine.rs:124-130` and `reconstructor.rs:234-240`  
**Severity:** NONE -- confirmed correct

### Forward scaling (fft_engine.rs)

```rust
let amplitude_scale = if bin_idx == 0 || bin_idx == spec_bins - 1 { 1.0 } else { 2.0 };
magnitudes.push((complex_val.norm() / n_fft as f32) * amplitude_scale);
```

Stored magnitude = `|X[k]| / N * amplitude_scale`

- DC/Nyquist: `stored = |X[k]| / N`
- Other bins: `stored = |X[k]| * 2 / N`

### Inverse scaling (reconstructor.rs)

```rust
let raw_mag = if i == 0 || i == spectrum.len() - 1 {
    mag * n_fft as f32           // undo /N only -> |X[k]|
} else {
    mag * n_fft as f32 / 2.0     // undo /N and *2 -> |X[k]|
};
```

Then after IFFT:
```rust
let norm = 1.0 / n_fft as f32;
for s in time_buffer.iter_mut() { *s *= norm; }
```

The realfft crate's inverse produces `N * x[n]`, so dividing by N gives `x[n]`.

**Roundtrip:**
```
Forward: x -> X -> |X|/N * scale -> store
Inverse: store -> |X|/N * scale * N / scale = |X| -> IFFT -> N*x -> /N -> x
```

This is correct. The research documents hypothesized scaling mismatch as a
possible cause of artifacts -- **refuted by code inspection**.

---

## Finding 7: Active-Bin Logic Is Correctly Shared {#finding-7-active-bins}

**File:** `spectrogram.rs:151-191`  
**Severity:** NONE -- confirmed correct

The `compute_active_bins()` function is used identically by both the
spectrogram renderer (`spectrogram_renderer.rs:224-236`) and the reconstructor
(`reconstructor.rs:211-217`). Both pass the same parameters:
`magnitudes`, `frequencies`, `recon_freq_min_hz`, `recon_freq_max_hz`,
`recon_freq_count`.

The skeleton_map.md noted this was previously duplicated and was extracted to
a shared function. Confirmed: there is exactly one implementation.

---

## Finding 8: Centered Crop Plan Arithmetic Looks Sound {#finding-8-centered-crop}

**File:** `reconstructor.rs:32-67`  
**Severity:** NONE -- confirmed correct

The centered crop plan:
1. Computes raw support from first frame center - window/2 to last frame center + window/2
2. Clips to the requested ROI (start_sample..stop_sample)
3. Returns crop_left/crop_right values used to trim the OLA output

The arithmetic handles the asymmetric case (where frame support doesn't fully
cover the ROI) by computing `keep_start = max(raw_start, start_sample)` and
`keep_end = min(raw_end, stop_sample)`.

The step 4 fix (commit `1bf2129`) appears to have resolved the centered
reconstruction issues documented in SINGLE_FRAME_FFT_NOTES.md.

---

## Finding 9: Overlap-Add Positioning Uses Local Index Correctly {#finding-9-ola-positioning}

**File:** `reconstructor.rs:273-274` and `reconstructor.rs:304-311`  
**Severity:** NONE -- confirmed correct

```rust
let start_pos = local_idx * hop;  // Phase 1: per-frame positioning
```

```rust
for (start_pos, windowed, _) in &frame_results {
    for (i, &sample) in windowed.iter().enumerate() {
        let pos = start_pos + i;
        if pos < output.len() {
            output[pos] += sample;
            window_sum[pos] += window[i] * window[i];
        }
    }
}
```

The OLA uses `local_idx` (0, 1, 2, ...) not `global_idx`, which correctly
places frames at `0, hop, 2*hop, ...` in the output buffer. The window_sum
accumulates `w[i]^2` at the matching positions. This is textbook OLA.

---

## Finding 10: Post-Reconstruction Normalization May Amplify Edge Noise {#finding-10-post-normalization}

**File:** `poll_loop.rs:696-701`  
**Severity:** LOW -- design consideration, not a bug

### What the code does

```rust
if st.normalize_audio {
    reconstructed.normalize(st.normalize_peak);
}
```

After reconstruction, the output is peak-normalized to 97% (default). This is
the same normalization applied to source audio on load.

### Why this is worth noting

If the reconstruction has edge artifacts (quiet edges from window taper,
or small spikes at boundaries), normalization scales everything so the
loudest sample hits 97%. If the loudest sample happens to be a boundary
spike, normalization could make the overall audio quieter while preserving
the spike's relative prominence.

This is not a bug -- it's standard behavior. But it means that boundary
spikes could appear more prominent in normalized playback than they would
in raw reconstruction output.

---

## Summary: What Is a Bug vs What Is Expected DSP Behavior {#summary}

### Confirmed Bug

| Issue | Where | Impact |
|-------|-------|--------|
| Epsilon threshold ~1e-6 instead of ~1e-38 | `reconstructor.rs:321` | Creates 444-sample silent gaps per side for 44100-sample Hann frames. Affects all zero-endpoint windows. |

### Design Decision Worth Revisiting

| Issue | Where | Impact |
|-------|-------|--------|
| Symmetric window generation (divisor `n-1`) | `fft_params.rs:120-147` | Standard FFT libraries default to periodic (divisor `n`). Symmetric has exact endpoint zeros; periodic has near-zero. Both are valid but periodic is more conventional. |

### Expected DSP Behavior (Not Bugs)

| Behavior | Explanation |
|----------|-------------|
| Hann/Blackman silent gaps at 0% overlap (1-2 samples after epsilon fix) | NOLA violation at exact window zeros. Mathematically irrecoverable. |
| Hamming/Kaiser boundary spikes at 0% overlap with sparse bins | Modified STFT is inconsistent (doesn't correspond to any real signal). Each frame's IFFT produces an independently filtered waveform. No crossfade at hop=window_len means raw discontinuities. |
| Spike magnitude correlates with window endpoint value | Higher endpoint = more energy at frame boundaries = larger jump when adjacent frames disagree. |
| Centered mode producing 2 frames for 1-frame-target ROI | Zero-padding by window/2 on each side expands the valid framed support. Standard centered STFT behavior. |

### Unlikely to Cause Audible Issues

| Issue | Where | Assessment |
|-------|-------|------------|
| IFFT truncation with zero-padding | `reconstructor.rs:261` | Conventional behavior; effect is small for moderate pad factors |
| DC/Nyquist phase forcing | `reconstructor.rs:242-247` | Negligible energy in those bins for typical audio |
| Post-recon normalization | `poll_loop.rs:696-701` | Standard; could amplify spike perception but doesn't create spikes |

---

## Research Hypothesis Verification {#research-verification}

The three research documents made several specific predictions. Here's how they
held up against the actual code:

### Confirmed by code inspection

| Hypothesis | Status |
|------------|--------|
| "Threshold likely ~1e-6 based on gap width math" | **CONFIRMED** -- line 321 uses `max_wsum * 1e-6` |
| "Gap width scales as (epsilon^(1/4) / pi) * (M-1)" | **CONFIRMED** -- code uses relative threshold, and the quartic Hann edge profile produces exactly the measured 444/222/148 gaps |
| "Interior gap = sum of adjacent edge gaps at 0% overlap" | **CONFIRMED** -- OLA positioning puts frames back-to-back; each contributes its edge gap at the seam |
| "Centered 1-frame target -> 2 frames is expected" | **CONFIRMED** -- `fft_engine.rs:52-56` computes `num_frames` from `padded_audio.len()` which includes center padding |
| "Forward/inverse scaling must be consistent" | **CONFIRMED CORRECT** -- roundtrip math checks out |
| "Analysis and synthesis must use same window" | **CONFIRMED CORRECT** -- both call `params.generate_window()` |

### Refuted or not applicable

| Hypothesis | Status |
|------------|--------|
| "Likely using periodic windows" | **REFUTED** -- code uses symmetric (divisor `n-1`). The research assumed standard library defaults; this code has its own window generation. |
| "Window embedding mismatch when win_length < n_fft" | **NOT APPLICABLE** -- zero-padding is appended after windowing (engine line 109), not center-padded around the window. Both forward and inverse handle this identically. |
| "Frame boundary sample ownership bug" | **NOT FOUND** -- OLA positioning is clean; no off-by-one in frame placement |
| "Onesided spectrum conjugate symmetry bug" | **NOT FOUND** -- `realfft` handles this internally; the code correctly uses one-sided spectrum throughout |

### Partially confirmed

| Hypothesis | Status |
|------------|--------|
| "Spikes from sparse bin selection are expected at 0% overlap" | **CONFIRMED as theory** -- code does independent per-frame bin selection via `compute_active_bins`, and with hop=window_len there is zero crossfade. Whether the spike magnitude matches theory requires runtime testing. |
| "Identity mode should produce near-perfect reconstruction" | **CANNOT VERIFY from code alone** -- need to test with all bins active, no frequency limiting. The scaling roundtrip is correct in code, but symmetric window + 0% overlap still has endpoint NOLA issues for Hann. |

---

## Recommended Investigation Order {#investigation-order}

Based on this audit, here is the recommended order for addressing the findings:

### 1. Fix the epsilon threshold (Finding 1)

This is the single highest-impact change. Replace:

```rust
let threshold = (max_wsum * 1e-6).max(1e-8);
```

with something close to:

```rust
let threshold = f32::MIN_POSITIVE;  // ~1.175e-38
```

or even a small multiple like `1e-10` if there are concerns about denormal
performance on the target hardware (Android/ARM).

After this fix:
- Hann/Blackman gaps should shrink from hundreds of samples to 1-2 samples
- Those 1-2 sample gaps are mathematically correct (NOLA violation at exact zeros)
- Hamming/Kaiser behavior should be unchanged (their endpoints were never below 1e-6)

### 2. Test identity-mode reconstruction

Before investigating spikes further, verify that full-spectrum (no bin masking,
no frequency limiting) reconstruction produces near-perfect output for each
window type. This separates implementation bugs from expected modified-STFT
artifacts.

Suggested test: Load a simple tone, set `recon_freq_count` to maximum and
`recon_freq_min/max` to full range. Compare reconstructed audio to original
(should be nearly identical for any window type that passes NOLA).

### 3. Evaluate symmetric vs periodic windows

After the epsilon fix, if the 1-2 sample endpoint gaps for Hann are bothersome,
switching to periodic window generation would eliminate them. This is a clean
change (modify `fft_params.rs:120-147` to use divisor `n` instead of `n-1`).

But this changes the window shape slightly, which would affect all existing
settings and saved FFT CSVs. Consider whether backward compatibility matters.

### 4. Document the 0%-overlap spike behavior

After steps 1-3, whatever boundary spikes remain at 0% overlap with sparse
bins are expected modified-STFT behavior. Document this in the user-facing
FFT documentation rather than trying to fix it in the reconstructor.

The research consistently says: overlap is the standard solution. 50% overlap
is the minimum for COLA compliance; 75% is the common "safe default" for
modified spectrograms. Communicating this to the user (perhaps as a UI hint
when overlap is very low and bin selection is active) would be more valuable
than complex mitigation code.

### 5. Consider zero-pad truncation (optional, low priority)

If reconstruction quality at high zero-pad factors (4x, 8x) with sparse bins
is unsatisfactory, consider whether keeping the full IFFT output (all `n_fft`
samples) and windowing the full length would produce better results. This
would require changing the OLA output length calculation and the window array
length. Low priority because the effect is likely small.

---

## Appendix: File-by-File Review Notes

### reconstructor.rs (491 lines)

- **Lines 120-132:** Output length calculation is correct for both centered and
  non-centered modes
- **Lines 194-277:** Phase 1 (parallel IFFT) is well-structured; cancellation
  checks are in the right place
- **Lines 300-311:** Phase 2 (sequential OLA) accumulates both signal and
  window_sum correctly
- **Lines 320-321:** THE BUG -- epsilon too large
- **Lines 324-330:** The normalization loop itself is correct (divide where
  above threshold, zero where below)
- **Lines 394-410:** Boundary position calculation handles centered crop
  offset correctly
- **Lines 432-462:** Gap run detection is purely diagnostic; does not affect output

### fft_engine.rs (161 lines)

- **Lines 43-50:** Center padding implementation is straightforward and correct
- **Lines 52-56:** Frame count calculation matches `FftParams::num_segments()`
- **Lines 108-111:** Windowing applies to first `window_len` samples; rest stays
  zero (correct for zero-padding)
- **Lines 116-117:** Time alignment uses `start_sample + frame_idx * hop`
  (frame start, not center). Internally consistent with reconstructor's OLA
  positioning.

### fft_params.rs (170 lines)

- **Lines 112-150:** Window generation uses symmetric formulas throughout.
  This is internally consistent but differs from standard library defaults.
- **Lines 57-59:** `hop_length()` uses float arithmetic, which could produce
  slightly different values for the same inputs depending on rounding. In
  practice this is fine for reasonable window sizes.

### spectrogram.rs (191 lines)

- **Lines 151-191:** `compute_active_bins()` is clean and efficient. Two-pass
  design (freq range filter, then top-N by magnitude) is correct.
- **Lines 33-65:** `from_frames_with_frequencies()` sorts defensively, which
  is good practice.

### callbacks_file.rs (956 lines)

- **Lines 696-714:** Rerun callback correctly handles cancellation when
  `is_processing` is true
- **Lines 725-850:** Full rerun path syncs all UI params before launching
  FFT. The overview -> focus -> reconstruct pipeline is well-orchestrated.
- **Lines 851-954:** Reconstruction-only path (CSV-loaded, no source audio)
  correctly skips FFT and goes straight to reconstruction.

### poll_loop.rs (977 lines)

- **Lines 445-664:** FFT completion handler correctly sequences overview ->
  focus -> reconstruction stages
- **Lines 666-789:** Reconstruction completion handler correctly loads audio
  into player, handles lock-to-active viewport snap
- **Lines 696-701:** Post-reconstruction normalization is applied (noted in
  Finding 10 as a design consideration)
