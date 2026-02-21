
# muSickBeets Next Steps Roadmap

This roadmap is derived from the current root documentation (`README.md`, `PROGRESS.md`, `map.md`, `ai_memory.md`, `documentation.md`, and `CLAUDE.md`).

## 1) Finish and de-risk FFT segmentation controls (highest impact)

- Complete the remaining **Segmentation Overhaul** items around user-driven bins-per-segment and segments-per-window controls.
- Add robust bidirectional parameter solving and clamping rules (even FFT sizes, min/max segment sizes, overlap limits).
- Persist all new knobs in settings and make CSV import/export metadata backward-compatible.

## 2) Stabilize file-open and background processing behavior

- Reproduce and resolve the intermittent file-open freeze noted in progress tracking.
- Add explicit cancellation/replace behavior when opening new files during ongoing processing.
- Improve status messaging and error handling around worker thread transitions.

## 3) UX polish: make advanced controls easier to discover

- Add a focused "analysis presets" layer (e.g., transients, tonal, balanced).
- Improve tooltip/inline help text for segmentation, zero-padding, and frequency scaling tradeoffs.
- Add reset-to-default per section (Analysis / Display / Reconstruction).

## 4) Documentation alignment and cleanup

- Update `PROGRESS.md` to avoid contradiction between "completed" and "active TODO" segmentation status.
- Add a dedicated FFT Analyzer user guide in root docs (current long-form `documentation.md` is tracker-centric).
- Keep `map.md` line counts and architecture summaries synchronized after major refactors.

## 5) Tracker-side maintenance (lower priority for current focus)

- Keep tracker stable and documented, but prioritize analyzer-focused roadmap items above.
- Limit tracker work to bug fixes and compatibility unless roadmap focus changes.

  
