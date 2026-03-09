# Single-Frame FFT Notes

This note captures the current research, known bugs, mathematical expectations,
and future plans for single-frame FFT behavior in the FFT analyzer.

It is intentionally focused on current understanding, not project history.

---

## Scope

This note is about the case where the active time range is configured so the
analysis produces exactly one FFT frame, or as close to that as the current
solver/center-padding rules allow.

Related goals:

- make single-frame analysis/reconstruction mathematically correct
- understand center pad on/off behavior exactly
- understand why blank or quiet edges appear
- preserve the information needed for a future instrument-oriented single-frame
  export workflow

---

## Terms

- **active time range / analysis region** -- the user-selected Start/Stop span
- **single analysis frame** -- one FFT frame spanning the analysis region
- **overview layer** -- whole-file FFT rendered with moderate default settings
- **focus layer** -- ROI FFT rendered with the user's current settings
- **frame support** -- the actual time-domain support of a frame/window

---

## What Zero Padding Does

Zero padding means extending the time-domain frame with zeros before computing
the FFT.

What it does:

- increases frequency sampling density
- makes spectral peaks and harmonics look smoother in the frequency direction
- changes FFT bin spacing

What it does **not** do:

- add new information to the original frame
- improve true time resolution
- directly create or remove blank time-domain edges by itself

So if zero padding changes edge behavior visually or audibly, that is likely an
indirect effect through solver behavior, frame ownership, or reconstruction
path logic -- not because zero padding is itself a time-domain edge operator.

---

## Current Research Findings

### 1. A single frame should taper smoothly before any thresholding

In the current STFT/ISTFT structure:

- forward FFT analyzes `x[n] * w[n]`
- inverse FFT reconstructs that windowed grain
- reconstruction applies the synthesis window again
- overlap-add normalization divides by accumulated `w[n]^2`

In a one-frame case, before any thresholding, the raw envelope is `w[n]^2`.

That means the mathematically expected edge behavior is a **smooth taper** that
follows the chosen window, not a broad hard-zero plateau.

### 2. The current cliff drop mostly comes from an implementation rule

The reconstructor currently zeros samples when:

`window_sum[i] < 0.1 * max(window_sum)`

In the one-frame case, `window_sum[n] = w[n]^2`, so this rule becomes:

`w[n]^2 < 0.1`

or equivalently:

`|w[n]| < sqrt(0.1) ≈ 0.316`

That is much more aggressive than the actual mathematical singularity at exact
window zeros.

For tapered windows like Hann and Blackman, this creates broad hard-zero edge
regions instead of a smooth fade.

### 3. Centered one-frame reconstruction is currently suspicious / inconsistent

Research found that centered one-frame behavior is not currently trustworthy.

Important concerns:

- turning on center pad often forces more than one frame in the normal path
- true single-frame centered reconstruction has inconsistent buffer-length and
  cropping semantics in the current code
- this needs direct instrumentation and bug fixing before centered one-frame
  behavior can be trusted

### 4. The solver/UI and actual FFT engine may disagree in one-frame centered cases

The app can present settings that appear to request one frame, but centered
analysis may mathematically imply more than one frame in the current engine.

That mismatch needs to be measured directly and then corrected.

### 5. Right-edge display stretching was a separate issue

The large right-edge stretch in the spectrogram display was primarily a frame
ownership / display support problem, not a core FFT math problem.

Recent renderer work moved spectrogram display toward interval-based frame
ownership. That issue is now much better behaved, but the single-frame
reconstruction math still needs dedicated attention.

---

## What Settings Affect Single-Frame Edge Behavior

Main controls:

- `window_length`
- `window_type`
- `num_frames`
- `overlap_percent` (once there is more than one frame)
- `use_center`

Indirect / not primary:

- `zero_pad_factor`

Expected effects:

- larger `window_length` changes the time support of the frame
- stronger taper windows increase edge attenuation
- more frames and overlap allow neighboring frames to fill edges during OLA
- center pad changes support semantics and can change frame count behavior
- zero padding should mostly change frequency sampling density, not blank-edge
  duration directly

---

## What the Future Instrument Workflow Needs

For a future binary that turns one frame into a sustained instrument, a single
frame export should preserve:

- per-bin magnitude
- per-bin phase
- sample rate
- window length
- window type
- zero-pad factor
- center-pad flag
- overlap / hop metadata
- frame identity / frame position semantics
- root-pitch metadata (to be added later)

Good news:

- the analyzer already preserves magnitude and phase per frame
- FFT CSV export/import already carries most FFT-analysis metadata

Important limitation:

- a single frame is a spectral snapshot, not a full evolving note
- it is suitable as seed material for a sustained instrument, but not as a full
  replacement for attack/transient/residual modeling

---

## Current Hypotheses To Verify In Code

These need instrumentation, not guessing:

1. In the normal source-audio path, can `use_center = true` truly produce one
   frame, or does it always produce at least two?
2. Exactly how many samples are being hard-zeroed by the current
   `window_sum` threshold rule for each window type?
3. Is the current one-frame blank-edge width mostly determined by the threshold
   rule or by something else in centered mode?
4. Does a one-frame centered reconstruction currently produce the correct output
   length and support, or is it mathematically inconsistent in code?

---

## Instrumented Findings (Step 2)

Direct instrumentation was added to the FFT engine and reconstructor to log:

- active sample count / seconds
- actual `num_frames`
- output length
- `window_sum` max / threshold
- first/last kept sample
- left/right zeroed span

### Confirmed behavior from logs

#### Non-centered true one-frame case

Example measured case:

- active range: `132300` samples (`3.000000s`)
- `window_len = 132300`
- `center = false`
- `num_frames = 1`
- output length = `132300`
- `left_zeroed = 25150` samples (`0.570295s`)
- `right_zeroed = 25150` samples (`0.570295s`)
- kept middle = `82000` samples (`1.859410s`)
- window type = `Hann`

Interpretation:

- the broad hard cutoffs are real
- they are symmetric in the one-frame non-centered case
- they are strongly explained by the current `window_sum < 0.1 * max(window_sum)`
  rule rather than by unavoidable FFT theory

#### Centered mode does not currently behave like true one-frame analysis

Measured centered examples for the same `3.0s` active range with
`window_len = 132300`:

- `center = true`, `hop = 33075` -> `num_frames = 5`
- `center = true`, `hop = 132300` -> `num_frames = 2`

Interpretation:

- enabling center pad expands the effective analyzed support enough that the
  current engine no longer yields a true single-frame result in normal use
- this matches the observed UI behavior where center pad prevents the expected
  one-frame case

#### Centered reconstruction support/cropping looks inconsistent

Measured centered reconstruction example:

- `num_frames = 5`
- `left_zeroed = 28288`
- `right_zeroed = 0`

Interpretation:

- centered reconstruction currently shows asymmetric support / cropping behavior
- this should be treated as a correctness bug before redesigning the edge-zeroing
  rule

### Step-2 conclusion

The new measurements strongly support this order:

1. Audit/fix centered semantics first.
2. Redesign the aggressive edge-zeroing rule second.

The main reason is that non-centered one-frame behavior is now well quantified,
while centered one-frame behavior is still not internally consistent.

---

## Center Pad Audit (Step 3)

Step 3 audited what `use_center = false` and `use_center = true` actually mean
in the current code, and compared that to standard STFT semantics.

### What `center = false` means in the current code

- analysis uses only full windows fully inside `start_sample..stop_sample`
- no extra left/right padding is added
- stored `time_seconds` is the frame start time
- reconstruction output length is `(num_frames - 1) * hop + window_len`
- playback alignment uses the first selected frame start time

This is broadly consistent with standard non-centered STFT behavior.

### What `center = true` means in the current code

- the active slice is padded by `window_len / 2` on both sides before FFT
- stored `time_seconds` acts like a frame center, not a frame start
- frame count increases because the padded region admits additional valid windows
- reconstruction output length is currently `(num_frames - 1) * hop`
- playback alignment uses the first selected frame center time

This is partly consistent with centered STFT semantics, but there are important
problems in the one-frame / low-frame-count path.

### What the recent logs confirm

Using the user's `3.0s` active range example:

- `center = false`, `window_len = 33074`, `hop = 33074` -> `num_frames = 4`
- `center = true`, `window_len = 33074`, `hop = 33074` -> `num_frames = 5`

This is mathematically expected under centered padding because adding
`window_len / 2` pad on both sides increases the valid framed support.

The logs also showed that zero flatline between chunks appears in both modes.
That is not primarily caused by center padding itself. It is dominated by the
current reconstruction behavior:

- sparse / low-overlap window support in the selected setup
- hard zeroing from the `window_sum < 0.1 * max(window_sum)` rule

### What is mathematically expected vs what looks wrong

Expected:

- `center = false` uses left-aligned windows on the real signal support
- `center = true` pads the signal and increases frame count near boundaries
- low-frame-count / no-overlap setups can produce isolated windowed chunks with
  silence between them if support does not overlap enough

Suspicious / implementation-specific:

- the solver/UI can imply a one-frame centered case while the actual FFT engine
  produces multiple frames
- the centered one-frame path is still not trustworthy
- the current centered reconstruction support/cropping is not yet proven correct
- the `window_sum` threshold rule can create wider hard-zero regions than the
  underlying DSP math requires

### Step-3 conclusion

The order of work still makes sense, but the centered problem is now sharper:

1. Centered semantics are not fundamentally wrong as a concept.
2. The current implementation is not cleanly aligned between solver/UI,
   frame-count expectations, and one-frame reconstruction behavior.
3. Step 4 should target centered one-frame reconstruction length/cropping
   correctness first.
4. Step 5 should then redesign the aggressive hard zeroing rule.

One useful refinement from this audit:

- part of step 4 should include checking the solver/UI count semantics so that
  the app's "one frame" expectation matches what the actual FFT engine will do
  when center padding is enabled.

### Centered solver/UI mismatch bug fix

After the audit, a small bug-fix slice was completed before step 4.

What was wrong:

- the segmentation solver always used non-centered counting
- the FFT engine used centered padded counting when `use_center = true`
- `FftParams::num_segments()` was only partly aligned with the engine behavior

What that caused:

- the UI could imply a centered one-frame case that the FFT engine would not
  actually produce
- derived info text and FFT progress totals could disagree with real centered
  frame counts

What was fixed:

- `segmentation_solver.rs` now accepts `use_center` and uses centered counting
  when needed
- `callbacks_ui.rs` now passes `use_center` into the solver
- `fft_params.rs::num_segments()` now matches centered counting even for short
  active ranges

Result:

- the centered/non-centered segment count shown in the UI now matches the actual
  FFT engine behavior much more closely
- this bug-fix did not change FFT generation or reconstruction math; it only
  fixed the solver/UI side

---

## Immediate Bug-Fix Order

The current recommended order is:

1. Instrument one-frame reconstruction math.
2. Audit center pad on/off semantics end-to-end.
3. Fix centered one-frame reconstruction length/cropping correctness.
4. Replace or redesign the aggressive `window_sum` edge-zeroing rule.
5. Measure blank-edge size per window type and document it.

Why this order:

- center pad correctness is a foundational bug
- the edge-zeroing rule is likely the main cause of cliff-drop behavior
- future single-frame export should be designed **after** the normal math path
  is trustworthy

---

## Likely Future Design Split

There are probably two different goals that should not be forced through the
same code path:

### A. Normal reconstruction for listening

- many-frame STFT/ISTFT behavior
- overlap-add reconstruction
- playback-oriented normalization
- visually and audibly faithful editing workflow

### B. Dedicated single-frame export / preview mode

- explicit frame selection for instrument creation
- preserves exact spectral snapshot and metadata
- may need different preview/export semantics than normal reconstruction

Recommendation:

- keep normal reconstruction mathematically honest
- build a dedicated single-frame export mode later instead of bending normal
  reconstruction to serve both jobs poorly

---

## Next Work Slices

1. Instrument one-frame reconstruction math (`num_frames`, output length,
   `window_sum`, zeroed spans).
2. Verify center pad on/off semantics against actual STFT behavior.
3. Fix centered one-frame reconstruction correctness.
4. Redesign the hard edge-zeroing rule.
5. Revisit future single-frame export design only after the above are stable.
