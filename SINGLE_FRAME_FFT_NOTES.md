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
- preserve enough metadata so future work remains possible later without shaping
  current bug-fix decisions around the instrument workflow

Strict boundary for this note:

- Future instrument/export ideas are reference material only.
- Do not treat them as active implementation work until the user explicitly says
  the current FFT analyzer foundation is stable and it is time to move on.

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

### 2. The original cliff drop came from an implementation rule

The reconstructor originally zeroed samples when:

`window_sum[i] < 0.1 * max(window_sum)`

In the one-frame case, `window_sum[n] = w[n]^2`, so this rule became:

`w[n]^2 < 0.1`

or equivalently:

`|w[n]| < sqrt(0.1) ≈ 0.316`

That was much more aggressive than the actual mathematical singularity at exact
window zeros.

For tapered windows like Hann and Blackman, it created broad hard-zero edge
regions instead of a smooth fade.

This old rule has now been replaced by tiny-epsilon normalization (see Step 5).

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

## Future Instrument Workflow Notes (Deferred)

This section is intentionally deferred reference material. It should not drive
active implementation until the user explicitly starts the later instrument
project.

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
3. How much residual blank-edge width remains after the epsilon-normalization
   fix, and what settings control it?
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

#### Non-centered true one-frame case before the normalization fix

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

### What the step-3 logs confirmed at the time

Using the user's `3.0s` active range example:

- `center = false`, `window_len = 33074`, `hop = 33074` -> `num_frames = 4`
- `center = true`, `window_len = 33074`, `hop = 33074` -> `num_frames = 5`

This is mathematically expected under centered padding because adding
`window_len / 2` pad on both sides increases the valid framed support.

The logs also showed that zero flatline between chunks appears in both modes.
That is not primarily caused by center padding itself. It is dominated by the
current reconstruction behavior:

- sparse / low-overlap window support in the selected setup
- broad hard-zeroing from the old `window_sum < 0.1 * max(window_sum)` rule

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
- the old broad `window_sum` threshold rule could create wider hard-zero
  regions than the underlying DSP math requires

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

## Centered Reconstruction Support Fix (Step 4)

Step 4 changed centered reconstruction to use actual frame support before
cropping, instead of collapsing the output length from `num_frames` alone.

### What changed

- centered reconstruction now builds the full raw overlap-add support
- then crops back to the actually covered unpadded support
- `recon_start_sample` is aligned to the kept support start rather than blindly
  using the first frame center

### What the logs now show

#### Centered one-window / one-second case

With:

- active range: `44100` samples (`1.0s`)
- `window_len = 44100`
- `hop = 44100`
- `center = true`

The logs show:

- `num_frames = 2`
- `raw_output_len = 88200`
- `Centered crop: keep_start=176400 keep_end=220500 crop_left=22050 crop_right=22050 final_len=44100`

Interpretation:

- centered analysis legitimately creates two supported centered frames in this
  configuration
- reconstruction now builds the full support and crops cleanly back to the
  requested 1-second ROI

#### Low-frame-count centered case with incomplete coverage

With:

- active range: `44100` samples (`1.0s`)
- `window_len = 8822`
- `hop = 8822`
- `center = true`

The logs show:

- `num_frames = 5`
- `raw_output_len = 44110`
- `Centered crop: keep_start=176400 keep_end=216099 crop_left=4411 crop_right=0 final_len=39699`

Interpretation:

- the selected centered frame supports do not fully cover the requested ROI end
- the reconstruction now returns the mathematically honest covered support
  rather than pretending it covers the full requested second

### Step-4 conclusion

This step appears to have fixed the structural centered-support bug:

- centered reconstruction is now support-based rather than `num_frames`-only
- asymmetric centered output support no longer appears to be the main issue
- the remaining visible/audio edge harshness is now more clearly attributable to
  the aggressive `window_sum` threshold rule

That was the state before step 5. Step 5 has now been completed.

---

## Edge Normalization Update (Step 5)

Step 5 replaced the old broad relative threshold rule with a tiny-epsilon
denominator check that is much closer to standard ISTFT / weighted overlap-add
 behavior.

### What changed

- old behavior: samples were zeroed whenever `window_sum < 0.1 * max(window_sum)`
- new behavior: samples are normalized wherever `window_sum` is greater than a
  tiny epsilon, and only truly unsupported samples remain zero

### What the user logs now show

Examples after the change:

- one-frame Hann, `1.0s`, center off:
  - `left_zeroed = 444` samples (`0.010068s`)
  - `right_zeroed = 444` samples (`0.010068s`)
  - kept support = `0.979864s`

- one-frame Hann, `3.0s`, center off:
  - `left_zeroed = 1332` samples (`0.030204s`)
  - `right_zeroed = 1332` samples (`0.030204s`)
  - kept support = `2.939592s`

Interpretation:

- the previous broad cliff-drop blank regions are gone
- the remaining silent spans are now small and consistent with support-limited
  gaps from tapered windows in one-frame / low-overlap cases
- this means step 5 appears to have succeeded structurally

### What remains after step 5

- quantify the remaining gaps by window type / overlap / frame count
- distinguish expected support-limited gaps from any still-suspicious behavior
- explain the remaining frame-boundary spikes separately from the silent gaps

That work is step 6.

---

## Residual Gap Measurements (Step 6, in progress)

Recent log-driven tests used a rising-tone source so audible gaps and spikes were
easy to hear and line up with frame boundaries.

### Non-centered Hann, 1 second ROI, 0% overlap

#### One frame

- active range: `44100` samples (`1.0s`)
- `window_len = 44100`
- `hop = 44100`
- `center = false`
- `num_frames = 1`
- `Gap runs: left=444 right=444 interior_count=0 max_interior=0`

Interpretation:

- the old broad cliff regions are gone
- residual silent edge spans are now about `444 / 44100 = 0.010068s` per side
- this is small enough to treat as support/window behavior rather than the old
  broken thresholding behavior

#### Two frames

- `window_len = 22050`
- `hop = 22050`
- `center = false`
- `num_frames = 2`
- `Gap runs: left=222 right=222 interior_count=1 max_interior=444`

Interpretation:

- edge gap shrinks with frame/window length
- there is one interior zero run centered on the frame transition
- its size is about the sum of the two adjacent edge gaps

#### Three frames

- `window_len = 14700`
- `hop = 14700`
- `center = false`
- `num_frames = 3`
- `Gap runs: left=148 right=148 interior_count=2 max_interior=296`

Interpretation:

- the pattern continues cleanly
- more frames create more interior seam gaps when overlap is zero
- each seam gap remains tied to the window support taper, not to a broad
  thresholding artifact

### Centered Hann, 1 second ROI, one-frame target

With:

- `window_len = 44100`
- `hop = 44100`
- `center = true`

The logs show:

- `num_frames = 2`
- `Centered crop: keep_start=176400 keep_end=220500 crop_left=22050 crop_right=22050 final_len=44100`
- `Gap runs: left=0 right=0 interior_count=1 max_interior=888`

Interpretation:

- under the current centered STFT definition, this is expected behavior:
  enabling center pad expands the valid support and produces two centered frames
  for this configuration
- the previous dead half-frame at the end is fixed
- a single interior seam gap remains, which is now the more important thing to
  characterize under step 6

### Current step-6 reading

- `center = true` producing two frames in the one-frame-target case is expected
  under the current centered framing scheme, not by itself a bug
- the large threshold-driven edge silences are gone
- what remains now looks like real support-limited seam/gap behavior for low or
  zero overlap with tapered windows
- the next job is to decide which of those remaining gaps are mathematically
  expected and which, if any, still point to a bug

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
