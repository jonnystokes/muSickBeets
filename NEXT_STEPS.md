# muSickBeets Next Steps Roadmap

This roadmap is derived from the current root documentation (`README.md`, `PROGRESS.md`, `map.md`, `ai_memory.md`, `documentation.md`, and `CLAUDE.md`).

## 1) Finish and de-risk FFT segmentation controls (highest impact)

- Complete the remaining **Segmentation Overhaul** items around user-driven bins-per-segment and segments-per-window controls.
- Add robust bidirectional parameter solving and clamping rules (even FFT sizes, min/max segment sizes, overlap limits).
- Persist all new knobs in settings and make CSV import/export metadata backward-compatible.

## 2) Add optional auto-regenerate mode with guardrails

- Implement an opt-in "auto-regenerate" mode for FFT/reconstruction updates on parameter edits.
- Add debounce/throttle controls and a quality/performance mode switch to prevent software-rendering stalls.
- Keep manual recompute as default/safe mode.

## 3) Improve performance and responsiveness for large files

- Add instrumentation around FFT/reconstruction/render times.
- Introduce frame decimation / progressive rendering for deep zoom-out views.
- Investigate memory pressure reduction for long files and high zero-padding.

## 4) Stabilize file-open and background processing behavior

- Reproduce and resolve the intermittent file-open freeze noted in progress tracking.
- Add explicit cancellation/replace behavior when opening new files during ongoing processing.
- Improve status messaging and error handling around worker thread transitions.

## 5) Strengthen FFT analyzer test coverage

- Add unit tests for FFT param derivations (hop, bins, resolution, zero-padding math).
- Add deterministic tests for reconstructor edge behavior and window-sum normalization thresholding.
- Add CSV round-trip tests for metadata + frame integrity.

## 6) UX polish: make advanced controls easier to discover

- Add a focused "analysis presets" layer (e.g., transients, tonal, balanced).
- Improve tooltip/inline help text for segmentation, zero-padding, and frequency scaling tradeoffs.
- Add reset-to-default per section (Analysis / Display / Reconstruction).

## 7) Documentation alignment and cleanup

- Update `PROGRESS.md` to avoid contradiction between "completed" and "active TODO" segmentation status.
- Add a dedicated FFT Analyzer user guide in root docs (current long-form `documentation.md` is tracker-centric).
- Keep `map.md` line counts and architecture summaries synchronized after major refactors.

## 8) Tracker-side maintenance (lower priority for current focus)

- Keep tracker stable and documented, but prioritize analyzer-focused roadmap items above.
- Limit tracker work to bug fixes and compatibility unless roadmap focus changes.
