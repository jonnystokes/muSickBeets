# noise_maker Progress

> Working tracker for the new `noise_maker` binary.

---

## Goal for First Testable 0.1.0

Reach a stable first-pass build where the user can:

- open a frame file or `audio_gene`
- inspect waveform + spectrogram with practical navigation
- switch between `Osc Bank` and `Frame OLA`
- play / pause / stop reliably
- save defaults, save `audio_gene`, and export preview audio
- begin focused bug hunting instead of broad feature construction

Current rough readiness: **86 / 100**

- Foundation and navigation are mostly in place.
- Project/file flow exists.
- Two playback engines exist.
- The biggest remaining risk is **audio behavior / engine quality**, not layout.

---

## Completed So Far

- Added `noise_maker` binary and made it the default run target.
- Built a left-sidebar + waveform + spectrogram + transport layout matching the analyzer style.
- Reused project theme/tooltips/logging style.
- Added unrestricted file loading for frame files and `audio_gene` projects.
- Added `audio_gene` save/load scaffold with embedded frame data.
- Added waveform navigation, visual gain, auto-gain button, Max Visible, axes, hover guides, and transport readouts.
- Added spectrogram frequency zoom/pan, axis, scrollbar sync, and hover readout.
- Added menu bar, key popup, status/message infrastructure, wrapped bottom status bar, and defaults persistence.
- Added two playback paths:
  - `Osc Bank`
  - `Frame OLA`
- Moved document prep and engine rebuild prep off the UI thread.
- Added transport readouts, hover guides, home/reset navigation, and basic playback diagnostics (`preview peak`, buffer length, rough health state).
- Added first correctness tests for:
  - `Osc Bank` render path
  - `Frame OLA` render path
  - preview length generation
  - `audio_gene` roundtrip persistence
- Added `Frame OLA` loop diagnostics:
  - playback buffer duration
  - hop size
  - boundary jump estimate
- Added `FrameFile` roundtrip persistence coverage.
- Added filesystem roundtrip coverage for:
  - `audio_gene`
  - `noise_maker` settings
- Added matrix coverage for `Frame OLA` across multiple windows and overlap amounts.
- Added UI hardening so engine rebuilds temporarily disable dependent controls while processing.
- Added a stronger loop-continuity assertion for `Frame OLA` boundary-jump behavior.

---

## Weighted Remaining Work

Scoring:

- **Impact**: how much it matters to reaching the first real bug-testing build
- **Risk**: chance of hidden bugs / regressions
- **Weight**: recommended priority for immediate work

### Critical Before First Bug-Testing Pass

| Item | Impact | Risk | Weight | Notes |
|------|--------|------|--------|-------|
| Playback correctness audit | 10 | 9 | 10 | Highest priority. Verify loudness, clipping, phase continuity, clicks, stop/reset behavior, and engine parity with expectations. |
| Frame OLA quality/tuning | 9 | 9 | 9 | Windowing, overlap behavior, loop smoothness, and spectral correctness need focused tuning. |
| Save/load reliability audit | 9 | 7 | 9 | `audio_gene` must reopen identically. Verify all key state is persisted and restored. |
| Status + error-path polish | 8 | 6 | 8 | Failed loads/exports/rebuilds should always leave the UI in a clear stable state. |
| UI state enable/disable audit | 8 | 5 | 8 | Check all controls when empty, loaded, rebuilding, failed, stopped, etc. |

### Important for 0.1.0 but Can Follow the First Test Pass

| Item | Impact | Risk | Weight | Notes |
|------|--------|------|--------|-------|
| Renderer polish and caching | 7 | 6 | 7 | Improves responsiveness and visual stability at larger preview sizes. |
| Export audio improvements | 7 | 5 | 7 | Exporting more than the current preview buffer may be needed. |
| Defaults/settings coverage expansion | 6 | 4 | 6 | Persist more UI state once the behavior is stable enough to freeze. |
| Menu completeness | 5 | 3 | 5 | More convenience than blocker now. |

### Probably After 0.1.0 Stabilization Starts

| Item | Impact | Risk | Weight | Notes |
|------|--------|------|--------|-------|
| Additional synthesis engines | 6 | 8 | 4 | Valuable, but not needed before the current two-engine build is proven. |
| Advanced display/editor tools | 5 | 6 | 4 | Good later, but not first-pass critical. |
| More project metadata / diagnostics panes | 4 | 3 | 3 | Nice for depth, not for the first stability milestone. |

---

## Recommended Build Order From Here

1. Playback correctness audit
2. Frame OLA quality tuning
3. Save/load reliability pass
4. Status/error-path cleanup
5. UI enable/disable audit
6. Renderer/caching polish

This order is chosen to get to "run it and report bugs" as soon as possible.

---

## Definition of Ready for User Bug Pass

The build is ready for the user to run and report bugs when all of the following are true:

- both engines produce sound consistently
- engine switching does not hang or corrupt state
- no obvious clicks / severe distortion on normal test inputs unless expected by engine design
- `audio_gene` roundtrips cleanly
- waveform and spectrogram navigation stay responsive
- status bar and message bar always explain current operation or failure
- no obvious control-state mismatches remain

Target readiness for that milestone: **85 / 100**

---

## Backburner for Later Feature Growth

- richer noise modeling / naturalization
- Perlin-driven modulation layers
- more advanced spectral editing tools
- more export/project formats if needed
