# muSickBeets FFT Analyzer — Categorized Issues with CMDL Scores

**Date:** 2026-02-25
**Scope:** All validated issues from all four AI reviews, deduplicated and verified against actual code.
**Sort Order:** Easiest category first (by estimated total effort).

## Session Instructions

When marking a category or item COMPLETE, append a short block:

- **How to test:** 1-3 sentences describing what the user should do to verify the fix works.
- **Next step:** What comes after user confirms it's working (next category, next bug, etc).

---

## Difficulty Measurement System (COMPACTION-SAFE)

> COMPACTION RULE: This entire "Difficulty Measurement System" section, from this line through the "END COMPACTION-SAFE BLOCK" marker, MUST be included verbatim in every compaction summary. This instruction itself is part of the verbatim block. The system is used to estimate code modification effort for all issues in the muSickBeets FFT Analyzer review.

### CMDL Score Format

Every issue gets a score written as: **CMDL(T | F, E, R)**

Where:
- **T** = Total (sum of F + E + R) — for sorting
- **F** = Files Touched — count of source files that must be edited
- **E** = Edit Sites — count of distinct code locations where changes are made (a contiguous block of changed/added lines = 1 site)
- **R** = Ripple Risk — does the change alter a function signature, data structure, or public API that other code depends on?
  - 0 = No, change is self-contained
  - 1 = Yes, but ripple stays within 1 file
  - 2 = Yes, ripple crosses file boundaries (callers in other files must update)
  - 3 = Yes, ripple crosses 3+ files or changes a core data structure

### Effort Bands (by T value)

| Band     | T range | Meaning                                                                 |
|----------|---------|-------------------------------------------------------------------------|
| Trivial  | 2–3     | Single-file, 1–2 edit sites, no ripple                                  |
| Easy     | 4–5     | 1–2 files, a few edit sites, minimal ripple                             |
| Moderate | 6–8     | 2–3 files, multiple edit sites, some API changes                        |
| Hard     | 9–12    | 3+ files, many edit sites, structural/API changes                       |
| Major    | 13+     | Architectural change touching many files with cross-cutting concerns    |

### Reading Example

CMDL(6 | 2, 2, 2) means: Total effort 6 (Moderate), touching 2 files, 2 edit sites, with cross-file ripple.
CMDL(3 | 1, 2, 0) means: Total effort 3 (Trivial), 1 file, 2 edit sites, no ripple.

> END COMPACTION-SAFE BLOCK

---

## CATEGORY LIST

Sorted by estimated total effort (easiest first). Each category will be populated with individual issues and CMDL scores in subsequent passes.

---

### Category 1: Input Validation & Edge Cases
- **Items:** 7
- **Estimated Difficulty:** Trivial–Easy
- **Category CMDL Total:** 19 (sum of all item T values)
- **Description:** Boundary conditions, degenerate inputs, missing defensive checks, and fragile API contracts. These are localized fixes — typically adding a guard clause, a clamp, or a defensive sort at a single code location.
- **Why this difficulty:** Most fixes are 1–3 line additions in a single file with no signature changes. Things like adding a `sort_by` call, clamping a value, or adding a NaN check.
- **Status:** COMPLETE

#### Items (sorted by CMDL T, easiest first):

| # | Issue | CMDL | Band | Files Changed | What Was Done |
|---|-------|------|------|---------------|---------------|
| 1 | EDGE-4: `freq_min_hz` stored as 0.0 but displayed as 1.0 | CMDL(2 \| 1, 1, 0) | Trivial | `view_state.rs` | Changed `reset_zoom()` to set `freq_min_hz = 1.0` instead of 0.0, matching the internal clamp in `y_to_freq`/`freq_to_y`. |
| 2 | EDGE-1: No rejection of sample_rate==0 in WAV loading | CMDL(2 \| 1, 1, 0) | Trivial | `audio_data.rs` | Added early `bail!` if WAV header reports sample_rate==0, preventing nonsensical time calculations downstream. |
| 3 | EDGE-6: `reconstructed_audio.take()` pattern loses data on panic | CMDL(2 \| 1, 1, 0) | Trivial | `callbacks_draw.rs` | Added explanatory comment documenting why `take()+put-back` is the correct pattern here (borrow checker constraint, single-threaded FLTK). Not a code change — the pattern itself is the best available option. |
| 4 | EDGE-5: NaN panic in `frame_at_time` binary search | CMDL(2 \| 1, 1, 0) | Trivial | `spectrogram.rs` | Replaced `.unwrap()` on `partial_cmp` with `.unwrap_or(Equal)`. Added NaN guard on input `time_seconds`. |
| 5 | BUG-7: `from_frames` assumes sorted input | CMDL(2 \| 1, 1, 0) | Trivial | `spectrogram.rs` | Added defensive `sort_by` on `time_seconds` with NaN-safe comparison. Signature changed from `frames: Vec` to `mut frames: Vec` (no external ripple — callers pass owned Vecs). |
| 6 | EDGE-2: Minimum window length of 2 is useless | CMDL(3 \| 1, 2, 0) | Trivial | `segmentation_solver.rs` | Raised `min_window` default from 2 to 4. Updated `clamp_even()` floor from 2 to 4. Updated one test assertion. |
| 7 | EDGE-3: No warning for enormous zero-padded FFT sizes | CMDL(2 \| 1, 1, 0) | Trivial | `callbacks_file.rs` | Added `eprintln!` warning when estimated peak FFT buffer memory exceeds 256 MB (computed from `n_fft * cores * 8 bytes`). |

**Category 1 actual effort band: Trivial** (all items T=2 or T=3, total=15, avg=2.1)

### Category 2: Idle/Polling Overhead
- **Items:** 2 (1 fixed, 1 assessed as not worth changing)
- **Estimated Difficulty:** Trivial
- **Category CMDL Total:** 3 (one real fix)
- **Description:** Wasted CPU cycles when the application is idle (no file loaded, no playback). The 16ms poll timer runs unconditionally, and ViewState is cloned every draw frame even during playback.
- **Why this difficulty:** The timer fix is adding an early-return condition check. The ViewState clone was assessed and found to be ~200 bytes per frame — not worth the refactoring complexity.
- **Status:** COMPLETE

#### Items (sorted by CMDL T, easiest first):

| # | Issue | CMDL | Band | Files Changed | What Was Done |
|---|-------|------|------|---------------|---------------|
| 1 | PERF-9: 16ms poll timer runs when idle | CMDL(3 \| 1, 2, 0) | Trivial | `main_fft.rs` | Added `is_idle` check (no audio loaded AND not processing). When idle, skips `update_info()` and scrollbar sync. Worker message polling still runs so FFT completion is never missed. |
| 2 | PERF-10: ViewState cloned every draw frame | — | N/A | — | **Not fixed.** Assessed the actual cost: ViewState contains a few floats and a 7-element `Vec<GradientStop>`. Total clone is ~200 bytes. At 60 FPS this is 12 KB/s — negligible. The clone is necessary because `spec_renderer.draw(&mut self)` conflicts with an immutable borrow of `st.view`. Eliminating it would require unsafe code or a major refactor of the renderer API for no measurable gain. |

**Category 2 actual effort band: Trivial** (T=3 for the one real fix)

### Category 3: Data Correctness
- **Items:** 4
- **Estimated Difficulty:** Easy–Moderate
- **Category CMDL Total:** 11 (sum of all item T values)
- **Description:** Code that produces wrong values — FFT magnitude scaling mismatch between forward and inverse, destructive source normalization, `format_with_commas` breaking for negatives, and CSV time key precision collisions.
- **Why this difficulty:** Most are small code changes, but the magnitude scaling fix requires understanding the math and testing both forward and inverse paths. The normalization fix needs a design decision (keep original vs. normalized copy).
- **Status:** COMPLETE

#### Items (sorted by CMDL T, easiest first):

| # | Issue | CMDL | Band | Files Changed | What Was Done |
|---|-------|------|------|---------------|---------------|
| 1 | BUG-6: `format_with_commas` breaks for negative numbers | CMDL(2 \| 1, 1, 0) | Trivial | `callbacks_draw.rs` | Separated the minus sign from the digit string before applying comma grouping. The minus is prepended after formatting so it doesn't throw off the `(len - i) % 3` modular arithmetic. |
| 2 | BUG-8: CSV time key precision collisions | CMDL(2 \| 1, 1, 0) | Trivial | `csv_export.rs` | Increased time format precision from `{:.5}` (10us resolution) to `{:.10}` (0.1ns resolution). At 48kHz with hop=1, frame step is ~20.8us — 5 decimal places could collide with pathological settings. 10 decimal places eliminates collisions for any conceivable sample rate. Backward compatible: old CSVs import fine. |
| 3 | BUG-2: Destructive source audio normalization | CMDL(3 \| 2, 2, 0) | Trivial | `app_state.rs`, `callbacks_file.rs` | Added `source_norm_gain: f32` field to `AppState` (default 1.0). The normalization gain applied on file load is now stored so the original peak level can be recovered (`original = normalized / gain`). Enhanced the log message to show the original peak. The normalization itself is still in-place (avoiding double memory), but the gain is no longer lost. |
| 4 | BUG-4: Forward/inverse FFT magnitude scaling mismatch | CMDL(4 \| 1, 2, 0) | Easy | `reconstructor.rs` | Fixed the reconstructor to undo the forward-pass scaling before feeding magnitudes into the IFFT. Forward pass stores `mag = (|X[k]| / N) * amp_scale` where amp_scale is 2 for non-DC/Nyquist, 1 for DC/Nyquist. Reconstructor now multiplies by `N` (DC/Nyquist) or `N/2` (other bins) to recover raw `X[k]` before IFFT. Previously, the double `1/N` division and uncompensated `*2` scaling produced incorrect relative amplitudes between DC/Nyquist and other bins, masked by post-reconstruction normalization. |

**Category 3 actual effort band: Trivial–Easy** (T range 2–4, total=11, avg=2.75)

### Category 4: Error Handling & Resilience
- **Items:** 4
- **Estimated Difficulty:** Easy–Moderate
- **Category CMDL Total:** 16 (sum of all item T values)
- **Description:** Missing panic handlers on worker threads (causing stuck `is_processing`), silent rendering skips via `try_borrow_mut`, poisoned mutex propagation in audio callback, and CSV import skipping row 2 without validation.
- **Why this difficulty:** Adding `catch_unwind` wrappers and `Disconnected` detection is straightforward but touches multiple spawn sites. The try_borrow_mut fix is just adding a debug log.
- **Status:** COMPLETE

#### Items (sorted by CMDL T, easiest first):

| # | Issue | CMDL | Band | Files Changed | What Was Done |
|---|-------|------|------|---------------|---------------|
| 1 | NEW-6: CSV import skips row 2 without validation | CMDL(2 \| 1, 1, 0) | Trivial | `csv_export.rs` | Added validation around `records.next()`: bails with error if file has no data rows, warns if the skipped row looks like data instead of a header (first field parses as a number). |
| 2 | NEW-4: `try_borrow_mut` silently skips rendering | CMDL(3 \| 1, 4, 0) | Trivial | `callbacks_draw.rs` | Added `dbg_log!(RENDER_DBG, ...)` at all 4 silent-return sites (spectrogram, waveform, freq axis, time axis). Controlled by `debug_flags::RENDER_DBG` flag (default off). |
| 3 | Poisoned mutex in audio callback | CMDL(3 \| 1, 2, 0) | Trivial | `audio_player.rs` | Replaced all 13 `.lock().unwrap()` calls with a `lock_playback()` helper that uses `.unwrap_or_else(\|e\| e.into_inner())` to recover from poisoned mutexes instead of panicking. Especially critical for the audio device callback (line 71) which runs on a real-time OS thread. |
| 4 | NEW-3: No panic handler on worker threads | CMDL(8 \| 2, 6, 0) | Moderate | `callbacks_file.rs`, `main_fft.rs` | Wrapped all 4 `thread::spawn` closures with `catch_unwind(AssertUnwindSafe(...))`. On panic, extracts the panic message and sends `WorkerMessage::WorkerPanic(msg)` through the channel. Added `WorkerPanic` variant to `WorkerMessage` enum. Poll loop now handles `WorkerPanic` (resets `is_processing`, shows error in status bar) and detects `TryRecvError::Disconnected` as a fallback for panics that occur before catch_unwind (e.g., in the spawn setup). |

**Category 4 actual effort band: Trivial–Moderate** (T range 2–8, total=16, avg=4.0)

### Category 5: Audio Playback
- **Items:** 3
- **Estimated Difficulty:** Easy–Moderate
- **Category CMDL Total:** 10 (sum of all item T values)
- **Description:** Audio device not recreated on sample rate change (wrong-speed playback), audio samples cloned into player unnecessarily (memory duplication), and play-after-dirty not auto-starting playback.
- **Why this difficulty:** The sample rate fix is a small conditional in `load_audio()`. The memory duplication fix requires changing from `Vec<f32>` to `Arc<Vec<f32>>` which ripples into the playback data callback. Play-after-dirty is a UX logic tweak.
- **Status:** COMPLETE

#### Items (sorted by CMDL T, easiest first):

| # | Issue | CMDL | Band | Files Changed | What Was Done |
|---|-------|------|------|---------------|---------------|
| 1 | BUG-3: Audio device not recreated on sample rate change | CMDL(3 \| 1, 2, 0) | Trivial | `audio_player.rs` | Added `device_sample_rate: u32` field to `AudioPlayer`. In `load_audio()`, the device is now destroyed and recreated when the new sample rate differs from the current device rate. Previously, the device was only created once (`if self.device.is_none()`), so loading a 48kHz file after a 44.1kHz file would play at the wrong speed. |
| 2 | MEM-3: Audio samples cloned into player | CMDL(4 \| 2, 3, 1) | Easy | `audio_player.rs`, `main_fft.rs` | Changed `PlaybackData.samples` from `Vec<f32>` to `Arc<Vec<f32>>`. The `load_audio()` API now accepts `Arc<Vec<f32>>` + `sample_rate` instead of `&AudioData`. The player's data callback indexes into the Arc without any copy. Full zero-copy sharing (eliminating the clone at the call site) is deferred to Category 7 when `AudioData.samples` is changed to `Arc<Vec<f32>>`. |
| 3 | Play-after-dirty: No auto-play after recompute triggered by Play | CMDL(3 \| 2, 3, 0) | Trivial | `app_state.rs`, `main_fft.rs`, `callbacks_ui.rs` | Added `play_pending: bool` to `AppState`. The Play callback sets it when triggering a recompute due to dirty state. The `ReconstructionComplete` handler checks and consumes the flag, auto-starting playback. The flag is also cleared on reconstruction error, worker panic, and channel disconnect to prevent stale pending state. |

**Category 5 actual effort band: Trivial–Easy** (T range 3–4, total=10, avg=3.3)

### Category 6: UI Thread Blocking
- **Items:** 4 (3 fixes — items 2 and 4 share one fix)
- **Estimated Difficulty:** Moderate
- **Category CMDL Total:** 17 (sum of all item T values)
- **Description:** Operations that freeze the GUI by running synchronously on the main thread: WAV loading, WAV saving, CSV export, and the RefCell borrow held during WAV save I/O. File dialog blocking is inherent to FLTK and not fixable.
- **Why this difficulty:** Each fix follows the same pattern (move work to `thread::spawn`, send result back via mpsc), but requires adding new `WorkerMessage` variants, restructuring callbacks, and handling the async completion. The pattern is well-established in the codebase already.
- **Status:** COMPLETE

#### Items (sorted by CMDL T, easiest first):

| # | Issue | CMDL | Band | Files Changed | What Was Done |
|---|-------|------|------|---------------|---------------|
| 1 | WAV loading blocks UI — `AudioData::from_wav_file` runs on main thread | CMDL(7 \| 2, 4, 1) | Moderate | `callbacks_file.rs`, `main_fft.rs`, `app_state.rs` | File I/O + normalization moved to `thread::spawn` in the Open callback. New `AudioLoaded(AudioData, PathBuf, f32)` WorkerMessage variant. The poll loop's `AudioLoaded` handler does all state setup (view bounds, transport, params), UI widget sync (stop input, recon freq max), enables widgets, then spawns the FFT thread — same flow as before but the disk read no longer blocks the GUI. |
| 2+4 | WAV saving blocks UI + RefCell held during I/O | CMDL(5 \| 2, 3, 1) | Easy | `callbacks_file.rs`, `main_fft.rs`, `app_state.rs` | Audio data cloned out of `state.borrow()`, borrow dropped, then `save_wav` runs in a spawned thread. New `WavSaved(Result<PathBuf, String>)` WorkerMessage variant. The poll loop handler shows success/error status. RefCell issue is automatically fixed: the borrow is released before any I/O. |
| 3 | CSV export blocks UI — frame filtering + `export_to_csv` runs on main thread | CMDL(5 \| 2, 3, 1) | Easy | `callbacks_file.rs`, `main_fft.rs`, `app_state.rs` | Spectrogram frames filtered and params/view cloned out of `state.borrow()`, borrow dropped, then `export_to_csv` runs in a spawned thread. New `CsvSaved(Result<(PathBuf, usize, f64, f64), String>)` WorkerMessage variant. The poll loop handler shows frame count and time range on success, or alert dialog on error. |

**Category 6 actual effort band: Easy–Moderate** (T range 5–7, total=17, avg=5.7)

### Category 7: Memory Efficiency
- **Items:** 2
- **Estimated Difficulty:** Moderate
- **Description:** Redundant per-frame frequency vectors (all frames store identical frequency data, wasting ~16 MB for 1000 frames), and frame cloning for reconstruction (copying ~49 MB on the main thread).
- **Why this difficulty:** Both require changing the `FftFrame` or `Spectrogram` data structure, which ripples through the FFT engine, reconstructor, CSV export/import, spectrogram renderer, and draw callbacks.
- **Status:** NOT STARTED

### Category 8: Rendering Performance
- **Items:** 5
- **Estimated Difficulty:** Moderate–Hard
- **Description:** Spectrogram rendering blocking the GUI thread, O(n) linear bin lookup per pixel row, unnecessary per-frame sort when all bins are active, single-threaded waveform peak rendering, and weak view hash causing potential stale renders.
- **Why this difficulty:** The bin lookup and sort skip are easy wins. But moving spectrogram rendering off the main thread is an architectural change: it requires a dedicated render thread, async cache updates, and a "show stale image while rendering" pattern. The waveform parallelization touches the renderer internals.
- **Status:** NOT STARTED

### Category 9: FFT/Reconstruction Pipeline
- **Items:** 4
- **Estimated Difficulty:** Moderate–Hard
- **Description:** Per-frame FFT planner allocation, no worker cancellation mechanism, sequential overlap-add (inherent to algorithm), and magnitude scaling mismatch between forward/inverse FFT.
- **Why this difficulty:** The planner fix is straightforward (thread-local or shared planner). But worker cancellation requires adding an `AtomicBool` cancellation flag, threading it through rayon iterators, and handling partial results. The magnitude scaling fix requires careful DSP understanding.
- **Status:** NOT STARTED

---

## CATEGORY SUMMARY TABLE

| # | Category                         | Items | Est. Band        | Status      |
|---|----------------------------------|-------|------------------|-------------|
| 1 | Input Validation & Edge Cases    | 7     | Trivial (avg T=2.1) | **COMPLETE** |
| 2 | Idle/Polling Overhead            | 2     | Trivial (T=3)    | **COMPLETE** |
| 3 | Data Correctness                 | 4     | Trivial–Easy (avg T=2.75) | **COMPLETE** |
| 4 | Error Handling & Resilience      | 4     | Trivial–Moderate (avg T=4.0) | **COMPLETE** |
| 5 | Audio Playback                   | 3     | Trivial–Easy (avg T=3.3) | **COMPLETE** |
| 6 | UI Thread Blocking                   | 4     | Easy–Moderate (avg T=5.7) | **COMPLETE** |
| 7 | Memory Efficiency                | 2     | Moderate         | NOT STARTED |
| 8 | Rendering Performance            | 5     | Moderate–Hard    | NOT STARTED |
| 9 | FFT/Reconstruction Pipeline      | 4     | Moderate–Hard    | NOT STARTED |
|   | **TOTAL**                        | **35**|                  |             |

---

## STANDALONE FIXES (outside original categories)

### Segment Size Controls Overhaul — COMPLETE

**Root cause:** `target_segments_per_active` was set to `Some(n)` on the first solver run and never reset to `None`. After that, the solver overrode every user change to window_length (dropdown, text field, +/- buttons) to maintain a stale segment count. This made all segment size controls effectively read-only after first use.

**What changed:**
- Dropdown and text field callbacks now clear `target_segments_per_active = None` and `target_bins_per_segment = None` before calling the solver, so user's explicit size choice is respected
- Text field changed from `CallbackTrigger::Changed` (per-keystroke) to `CallbackTrigger::EnterKey` — type a full number, press Enter
- Selecting "Custom" in the dropdown now focuses the text field
- +/- buttons removed (simplified to just dropdown + text field)
- Status bar feedback when solver adjusts the user's choice (e.g. clamped to active range)

**Files:** `layout.rs`, `callbacks_ui.rs`, `main_fft.rs`, `callbacks_nav.rs`

**How to test:** Load an audio file. Select different presets from the segment size dropdown — each should apply immediately and the text field should update to match. Type a custom number in the text field and press Enter — it should apply and the dropdown should switch to "Custom" (or the matching preset). Try switching back to a preset after using a custom value — it should work without getting stuck. Check the status bar for feedback if a value gets clamped.

**Next step:** Category 7: Memory Efficiency (2 items — redundant per-frame frequency vectors, frame cloning for reconstruction).

---

## NOTES FOR FUTURE SESSIONS

- Categories 1–6 are COMPLETE. Segment size controls overhaul is COMPLETE. Start from **Category 7: Memory Efficiency** (2 items).
- After each category, give the user concise testing instructions for the major changes, then mention which category comes next.
- The codebase uses `thread::spawn` + `mpsc::Sender<WorkerMessage>` for background work. The poll loop in `main_fft.rs` (16ms timer) handles all `WorkerMessage` variants. This pattern was extended in Category 6 with `AudioLoaded`, `WavSaved`, and `CsvSaved`.
- `Rc<RefCell<AppState>>` is `!Send` — never try to pass it to a thread. Extract owned data on main thread, spawn with owned data, send result back via mpsc.
- All debug logging uses `debug_flags.rs` flags + `dbg_log!` macro (not raw `eprintln!` — except for `[Open]`/`[FFT thread]` progress messages which are always-on).

*Each category will be expanded with individual issues, CMDL scores, and fix descriptions as work progresses.*
