# Progress Tracker

## Completed Features

### FFT Segmentation Redesign (Feb 2026)
Full overhaul of FFT segmentation controls with bidirectional parameter solving.

- [x] **Phase 1** -- Environment + session reliability
  - Resolved missing system linker dependencies for FLTK on Ubuntu
  - Documented required apt packages, mirrored guidance into AGENTS.md
- [x] **Phase 2** -- Status bar FFT diagnostic readout
  - Dedicated FFT status readout widget with tooltip showing segmentation explanation
  - Auto-growing bottom status region based on wrapped text length
- [x] **Phase 3** -- Segmentation control model (live recalculation)
  - Pure segmentation solver module with deterministic dependency resolution
  - Segments-per-active-area input + bins-per-segment input
  - `LastEditedField` enum tracks which control was last edited for solver routing
  - Live dependent field updates while typing
  - Hard constraints enforced (even window, min 4 samples, overlap capped, hop >= 1)
- [x] **Phase 4** -- Time unit toggle expansion
  - All editable sample/time fields toggle between samples and seconds
  - Internal storage remains sample-based; display converts on the fly
  - Transport, Start/Stop, info readouts, and navigation all respect the toggle
- [x] **Phase 5** -- Persistence + compatibility
  - Settings INI persistence for solver fields (target_segments, target_bins, last_edited)
  - CSV metadata export/import with backward compatibility for older files
  - Round-trip and backward-compat regression tests
- [x] **Phase 6** -- Validation + QA
  - `cargo build` + `cargo test` passing
  - Manual smoke checks completed

### Segmentation Overhaul Features (Feb 2026)
- [x] Resolution trade-off display (live multi-line info in ANALYSIS section)
- [x] dB ceiling slider (DISPLAY section, auto-set from data, user-adjustable)
- [x] Direct segment size input + presets (typed input + dropdown, +/- steps through presets)
- [x] Zero-padding factor (1x/2x/4x/8x, full FFT pipeline integration)
- [x] Hop size display (read-only, below overlap slider)

### Custom Gradient/Color Ramp Editor (Feb 2026)
- [x] GradientStop data structure, eval_gradient(), default 7-stop rainbow
- [x] Custom variant added to ColormapId (8th dropdown option)
- [x] Interactive preview widget: click to add, drag to move, right-click to delete, shift+click for color picker
- [x] ColorLUT extended with set_custom_stops() for dynamic gradient rendering
- [x] Save/load custom gradient to settings.ini

### Bug Fixes (Feb 2026)
- [x] Text field validation: switched from set_callback() to handle() so validation survives when functional callbacks are attached later
- [x] Spacebar guard v3 -- FINAL: three-layer approach (clear_visible_focus + block_space! macro + window handler)
- [x] Global time display: transport bar shows "L" (local) and "G" (global) time
- [x] Time calculation precision: recon_start_time from actual first FFT frame time
- [x] Volume reduction fix: reconstructor overlap-add normalization uses adaptive threshold
- [x] Remove 64-sample minimum on segment size (now allows down to 4)
- [x] Fix Save As Default button (SIDEBAR_INNER_H overflow from gradient widget)

### Lock to Active v2 (Feb 2026)
- [x] Matches Home button behavior: snaps both time AND frequency to active range
- [x] Uses 0.5s delay via app::add_timeout3 after reconstruction completes
- [x] Tooltip updated to mention both time and frequency

### Code Review + Fixes (Feb 2026)
4 AI reviewers (Claude Opus, Trinity, MiniMax, Big Pickle) reviewed the codebase.
Cross-referenced and deduplicated into CATEGORIZED_ISSUES.md (9 categories, 35 issues).

- [x] **Category 1** -- Input Validation & Edge Cases (7 items)
  - reset_zoom() uses freq_min_hz=1.0 not 0.0
  - Early bail for sample_rate==0
  - Documented take()+put-back pattern as intentional
  - NaN-safe binary search in frame_at_time()
  - Defensive sort in Spectrogram::from_frames()
  - Min window raised from 2 to 4
  - Memory warning for huge zero-padded FFTs
- [x] **Category 2** -- Idle/Polling Overhead (2 items)
  - is_idle guard skipping update_info() and scrollbar sync when no audio loaded
  - ViewState clone assessed as not worth fixing (~200 bytes)
- [x] **Infrastructure** -- Debug flags system
  - debug_flags.rs with toggleable CURSOR_DBG, FFT_DBG, PLAYBACK_DBG, RENDER_DBG
  - dbg_log! macro for conditional debug output

### UI Restructure -- Transport + Cursor Readout (Feb 2026)
- [x] Split transport from 1 row into 2 rows: scrubber (18px) + controls (28px)
- [x] cursor_readout Frame widget shows freq/dB/time between Stop button and L/G time
- [x] Event::Enter handler so FLTK delivers Event::Move to spectrogram Widget
- [x] Event::Leave clears readout when mouse exits spectrogram

### Mouse Navigation Redesign (Feb 2026)
- [x] New scroll wheel scheme:
  - No modifier: pan frequency axis (up/down)
  - Ctrl + scroll: pan time axis (left/right)
  - Alt + scroll: zoom frequency centered on cursor Y
  - Alt + Ctrl + scroll: zoom time centered on cursor X
- [x] swap_zoom_axes setting (persisted in settings.ini) swaps Alt vs Alt+Ctrl zoom axes
- [x] Pan step: 15% of visible range per tick; zoom uses mouse_zoom_factor setting

---

## Active Work

### Code Review Issues (CATEGORIZED_ISSUES.md)
Categories 1-2 complete. Remaining:
- [ ] Category 3: Data Correctness (4 items)
- [ ] Category 4: Error Handling & Resilience (4 items)
- [ ] Category 5: Audio Playback (3 items)
- [ ] Category 6: UI Thread Blocking (4 items)
- [ ] Category 7: Memory Efficiency (2 items)
- [ ] Category 8: Rendering Performance (5 items)
- [ ] Category 9: FFT/Reconstruction Pipeline (4 items)

---

## Backburner

- [ ] File open freeze -- intermittent, debug logging in place but root cause not found
- [ ] Auto-regenerate mode (careful -- software rendering only, no GPU)
- [ ] Analysis presets layer (transients, tonal, balanced)
- [ ] Per-section reset-to-default (Analysis / Display / Reconstruction)
- [ ] FFT Analyzer user guide (documentation.md is tracker-centric)
- [ ] Update map.md line counts and architecture summaries after major refactors
- [ ] Update README.md after this development stage

---

## Key Architecture Notes
- Settings loaded in main() before UI, applied to AppState
- FreqScale::Power(f32) replaces old Log/Linear toggle
- Audio normalization happens both on file load AND after reconstruction
- Zoom factors stored in AppState, read from settings
- Freq axis labels use pixel-space-first generation with binary search inversion
- is_seeking flag prevents playback from auto-pausing during cursor drag
- Save As Default captures current AppState into Settings struct and writes INI
- Custom gradient: Vec<GradientStop> in ViewState, piped through ColorLUT and SpectrogramRenderer
- Settings file: `settings.ini` in working directory (created on first run)

## Attribution
- Spectrogram visualization and reconstruction inspired by [Audio-Experiments](https://github.com/SebLague/Audio-Experiments) by Sebastian Lague (MIT License)
- Custom gradient editor inspired by [Gradient-Editor](https://github.com/SebLague/Gradient-Editor) by Sebastian Lague
