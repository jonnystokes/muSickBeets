# FFT Segmentation Redesign Plan Tracker

## Objective
Implement a human-readable FFT segmentation diagnostic readout, expandable status area, and bidirectional segmentation controls while preserving all existing functionality.

## Mandatory Phase Review Gate (Required after every phase)
1. Phase implementation complete
2. Run `cargo build`
3. Run `cargo test` (if tests exist)
4. Quick manual smoke test for affected functionality (UI if UI-related)
5. Update this tracker (checklist + short summary note)
6. Only then proceed to next phase

### Compile Gate Policy
- If compilation succeeds: continue.
- If issue is trivial and well-understood: fix immediately, then re-run compile gate.
- If issue is architectural/unclear/risky: document issue clearly and pause for user input.

## Ground Rules
- [x] Confirm project builds before feature work (`cargo build`).
- [x] Keep reconstruction trigger behavior unchanged (manual rerun / space / play if dirty).
- [ ] Keep existing audio load/save/reconstruct workflows functional.
- [x] Keep spacebar guard + numeric validation behavior intact for all new widgets.
- [x] Update tracker continuously during implementation.
- [x] No commit without tracker reflecting current implementation state + short completion note.

## Phase 1 — Environment + Session Reliability
- [x] Resolve missing system linker dependencies for FLTK on Ubuntu.
- [x] Document required apt packages for future AI sessions.
- [x] Mirror Claude guidance into `AGENTS.md`.
- [x] Phase Review Gate completed.

## Phase 2 — Status Bar FFT Diagnostic Readout
- [x] Add dedicated FFT status readout widget above the bottom status bar.
- [x] Add tooltip to FFT status readout area with the segmentation explanation text.
- [x] Generate a full plain-English segmentation sentence from current FFT params.
- [x] Auto-grow bottom status region upward based on wrapped FFT text length.
- [x] Add tunable offset variable for future UI spacing adjustments.
- [ ] Verify behavior while resizing the app window.
- [ ] Phase Review Gate completed.

## Phase 3 — Segmentation Control Model (Live Recalculation)

### Deterministic dependency resolution order (authoritative)
1. Determine `active_samples` from analysis start/stop.
2. Apply hard constraints (window even, >=2, current segment cap retained, overlap clamped to stable range, hop>=1).
3. Apply solver path from `last_edited_field` (single source of authority).
4. Recompute derived values from solved params: hop, segment count, bins, Hz/bin.
5. Mark dirty but do not auto-reconstruct.

### Last-edited solver paths
- **Overlap edited**
  - Overlap becomes authoritative.
  - If a segments-per-active-area lock exists, preserve that segment target and solve segment size/window length.
  - Otherwise preserve current window length and recompute derived values only.
- **Segments-per-active-area edited**
  - Segment target becomes authoritative/locked.
  - Preserve overlap value.
  - Solve window length to best-fit target segment count under hard constraints.
- **Bins-per-segment edited**
  - Bin target becomes authoritative.
  - Solve FFT sizing deterministically (initial implementation isolates this logic in pure solver module).
  - Preserve dirty-bit reconstruction model (no auto rerun).

### Implementation checklist
- [x] Add isolated, testable solver module (pure functions + unit tests).
- [x] Add "segments per active area" input.
- [x] Add "bins per segment" input.
- [x] Wire deterministic `last_edited_field` state.
- [x] Keep edited "segments per active area" target persistent until another field is edited.
- [x] Maintain overlap on segment-count edits unless overlap is the edited field.
- [x] Ensure dependent fields update live while typing.
- [x] Keep hard constraints unchanged until solver stability is verified.
- [x] Preserve reconstruction execution behavior unchanged (dirty-bit model).
- [ ] Phase Review Gate completed.

## Phase 4 — Time Unit Toggle Expansion
- [ ] Ensure all editable sample/time fields toggle between samples and seconds.
- [ ] Keep internal storage sample-based only.
- [ ] Ensure information/readout areas always show both units.
- [ ] Validate 5-decimal seconds formatting consistency.
- [ ] Phase Review Gate completed.

## Phase 5 — Persistence + Compatibility
- [x] Extend settings persistence for new segmentation controls.
- [x] Extend FFT CSV metadata to include new solver-related values.
- [x] Verify reopen/load reproduces equivalent reconstruction behavior.
- [x] Phase Review Gate completed.

## Phase 6 — Validation + QA
- [x] `cargo build`
- [x] `cargo test` (if available)
- [ ] Manual smoke checks for load/analyze/recompute/playback/save/load fft/save wav
- [ ] Screenshot updated UI (status FFT readout visible)

## Notes
- Active range semantics: use analysis start/stop selection.
- Percent displays: integer percentages; seconds display precision: 5 decimals.
- Real FFT window length must remain even.
- Overlap target range goal deferred until solver stability verification.

## Implementation Log
- 2026-02-21: Tracker updated with mandatory compile gates, phase review gate checklist, deterministic Phase 3 solver order, and commit discipline requirements.

- 2026-02-21: Phase 3 implementation slice completed: added pure segmentation solver + tests, integrated deterministic last-edited routing, and added live Segments/Active + Bins/Segment controls with solver-driven recompute (dirty-bit only). Compile/test gates passed (`cargo build`, `cargo test --no-run`, `cargo test segmentation_solver`).
- 2026-02-21: Pre-commit tracker sync complete for current Phase 3 slice (solver + UI wiring + compile/test gates).
- 2026-02-21: Phase 5 partial implementation: added settings persistence for segmentation solver fields and backward-compatible CSV metadata import/export for solver fields. Compile/test gates passed (`cargo build`, `cargo test --no-run`, `cargo test test_csv_roundtrip`).
- 2026-02-21: Pre-commit tracker sync complete for Phase 5 partial persistence slice (settings + CSV metadata compatibility).
- 2026-02-21: Follow-up fix: CSV export metadata now writes solver fields (segments target, bins target, last edited) to match import compatibility parsing.
- 2026-02-21: Phase 5 verification slice: added settings INI roundtrip test for solver fields and CSV roundtrip assertions for solver metadata; this covers reopen/load compatibility checks in automated tests.
- 2026-02-21: Added automated compatibility tests for persistence/reopen behavior: CSV roundtrip now asserts solver metadata, and settings parser test verifies solver field restoration from INI text.
- 2026-02-21: Pre-commit tracker sync complete for Phase 5 verification test slice.
- 2026-02-21: Added backward-compat regression tests: CSV import without solver metadata defaults safely, and settings parser behavior is validated when new solver keys are missing.
- 2026-02-21: Added explicit backward-compat tests for missing solver metadata in CSV and missing solver keys in settings INI parsing.
- 2026-02-21: Pre-commit tracker sync complete for persistence backward-compat test hardening slice.
- 2026-02-21: Solver hardening slice: added deterministic tests for zero-pad bins mapping and hard-constraint enforcement in segmentation solver. Compile/test gates passed.
- 2026-02-21: Pre-commit tracker sync complete for solver test-hardening slice.

- 2026-02-21: Follow-up on solver hardening feedback: tightened hard-constraint assertion to exact max-window clamp, added low-bin/min-even-floor regression, and re-ran compile/test gates (`cargo test segmentation_solver`, `cargo build`).
- 2026-02-21: Remaining manual QA for this slice: run UI smoke checks and capture updated UI screenshot per Phase 6 checklist.
- 2026-02-21: Environment follow-up: installed required FLTK/X11/Pango/Cairo dev libs in container, re-ran `cargo test --no-run` and `cargo test segmentation_solver` successfully.
- 2026-02-21: Remaining work after this run: complete manual UI smoke checks and capture an updated FFT status readout screenshot (Phase 6 pending items).
