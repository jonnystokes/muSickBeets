# muSickBeets FFT Analyzer — Categorized Issues with CMDL Scores

**Date:** 2026-02-25
**Scope:** All validated issues from all four AI reviews, deduplicated and verified against actual code.
**Sort Order:** Easiest category first (by estimated total effort).

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
- **Description:** Boundary conditions, degenerate inputs, missing defensive checks, and fragile API contracts. These are localized fixes — typically adding a guard clause, a clamp, or a defensive sort at a single code location.
- **Why this difficulty:** Most fixes are 1–3 line additions in a single file with no signature changes. Things like adding a `sort_by` call, clamping a value, or adding a NaN check.
- **Status:** NOT STARTED

### Category 2: Idle/Polling Overhead
- **Items:** 2
- **Estimated Difficulty:** Trivial–Easy
- **Description:** Wasted CPU cycles when the application is idle (no file loaded, no playback). The 16ms poll timer runs unconditionally, and ViewState is cloned every draw frame even during playback.
- **Why this difficulty:** The timer fix is adding an early-return condition check. The ViewState clone is a minor refactor to use references instead of cloning.
- **Status:** NOT STARTED

### Category 3: Data Correctness
- **Items:** 4
- **Estimated Difficulty:** Easy–Moderate
- **Description:** Code that produces wrong values — FFT magnitude scaling mismatch between forward and inverse, destructive source normalization, `format_with_commas` breaking for negatives, and CSV time key precision collisions.
- **Why this difficulty:** Most are small code changes, but the magnitude scaling fix requires understanding the math and testing both forward and inverse paths. The normalization fix needs a design decision (keep original vs. normalized copy).
- **Status:** NOT STARTED

### Category 4: Error Handling & Resilience
- **Items:** 4
- **Estimated Difficulty:** Easy–Moderate
- **Description:** Missing panic handlers on worker threads (causing stuck `is_processing`), silent rendering skips via `try_borrow_mut`, poisoned mutex propagation in audio callback, and CSV import skipping row 2 without validation.
- **Why this difficulty:** Adding `catch_unwind` wrappers and `Disconnected` detection is straightforward but touches multiple spawn sites. The try_borrow_mut fix is just adding a debug log.
- **Status:** NOT STARTED

### Category 5: Audio Playback
- **Items:** 3
- **Estimated Difficulty:** Easy–Moderate
- **Description:** Audio device not recreated on sample rate change (wrong-speed playback), audio samples cloned into player unnecessarily (memory duplication), and play-after-dirty not auto-starting playback.
- **Why this difficulty:** The sample rate fix is a small conditional in `load_audio()`. The memory duplication fix requires changing from `Vec<f32>` to `Arc<Vec<f32>>` which ripples into the playback data callback. Play-after-dirty is a UX logic tweak.
- **Status:** NOT STARTED

### Category 6: UI Thread Blocking
- **Items:** 4
- **Estimated Difficulty:** Moderate
- **Description:** Operations that freeze the GUI by running synchronously on the main thread: WAV loading, WAV saving, CSV export, and the RefCell borrow held during WAV save I/O. File dialog blocking is inherent to FLTK and not fixable.
- **Why this difficulty:** Each fix follows the same pattern (move work to `thread::spawn`, send result back via mpsc), but requires adding new `WorkerMessage` variants, restructuring callbacks, and handling the async completion. The pattern is well-established in the codebase already.
- **Status:** NOT STARTED

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
| 1 | Input Validation & Edge Cases    | 7     | Trivial–Easy     | NOT STARTED |
| 2 | Idle/Polling Overhead            | 2     | Trivial–Easy     | NOT STARTED |
| 3 | Data Correctness                 | 4     | Easy–Moderate    | NOT STARTED |
| 4 | Error Handling & Resilience      | 4     | Easy–Moderate    | NOT STARTED |
| 5 | Audio Playback                   | 3     | Easy–Moderate    | NOT STARTED |
| 6 | UI Thread Blocking               | 4     | Moderate         | NOT STARTED |
| 7 | Memory Efficiency                | 2     | Moderate         | NOT STARTED |
| 8 | Rendering Performance            | 5     | Moderate–Hard    | NOT STARTED |
| 9 | FFT/Reconstruction Pipeline      | 4     | Moderate–Hard    | NOT STARTED |
|   | **TOTAL**                        | **35**|                  |             |

---

*Each category will be expanded with individual issues, CMDL scores, and fix descriptions. Categories will receive a combined CMDL score once all items are scored.*
