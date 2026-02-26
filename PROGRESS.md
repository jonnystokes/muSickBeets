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
- [x] Direct segment size input + presets (typed input + dropdown)
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
- [x] GTK file dialog freeze fix: disabled GTK/Zenity file dialogs, uses FLTK built-in chooser

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
- [x] **Category 3** -- Data Correctness (4 items)
  - format_with_commas fixed for negative numbers
  - CSV time key precision increased to {:.10}
  - source_norm_gain stored in AppState to preserve original peak level
  - Forward/inverse FFT magnitude scaling mismatch fixed in reconstructor
- [x] **Category 4** -- Error Handling & Resilience (4 items)
  - CSV import row 2 skip validates existence
  - dbg_log! at all try_borrow_mut silent-return sites
  - All 13 .lock().unwrap() replaced with lock_playback() helper (poisoned mutex safe)
  - All 4 thread::spawn sites wrapped with catch_unwind, WorkerPanic message variant
- [x] **Category 5** -- Audio Playback (3 items)
  - Audio device recreated when sample rate changes
  - Player uses Arc<Vec<f32>> for zero-copy sample sharing
  - play_pending flag for auto-play after recompute
- [x] **Category 6** -- UI Thread Blocking (4 items, 3 fixes)
  - WAV loading moved to background thread (AudioLoaded message)
  - WAV saving moved to background thread (WavSaved message)
  - CSV export moved to background thread (CsvSaved message)
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

### Segment Size Controls Overhaul (Feb 2026)
- [x] Fixed root cause: target_segments_per_active one-way latch cleared on explicit user changes
- [x] Removed +/- buttons (simplified to dropdown + text field)
- [x] "Custom" dropdown selection now focuses text field
- [x] Status feedback via msg_bar (top message area, color-coded) when solver adjusts values
- [x] Added msg_bar widget to menu bar row (right of File/Analysis/Display menus)
- [x] set_msg() helper with MsgLevel enum (Info/Warning/Error) for color-coded messages

### Documentation Updates (Feb 2026)
- [x] AGENTS.md: CallbackTrigger rules for text fields (NEVER use EnterKey)
- [x] AGENTS.md: Sub-agent briefing preamble (LSP tools, Exa tools, tool effectiveness report)
- [x] AGENTS.md: Harness detection (OpenCode vs Claude Code)
- [x] CATEGORIZED_ISSUES.md: Session instructions (how-to-test + next-step on completion)
- [x] CLAUDE.md synced to match AGENTS.md
- [x] map.md updated with current line counts and architecture descriptions
- [x] PROGRESS.md updated to reflect all completed work

---

## Active Work

### Code Review Issues (CATEGORIZED_ISSUES.md)
- [x] Categories 1-6 complete
- [x] Segment size controls overhaul complete
- [ ] Category 7: Memory Efficiency (2 items)
- [ ] Category 8: Rendering Performance (5 items)
- [ ] Category 9: FFT/Reconstruction Pipeline (4 items)

---

## Backburner

- [ ] Analysis presets layer (transients, tonal, balanced)
- [ ] Per-section reset-to-default (Analysis / Display / Reconstruction)
- [ ] FFT Analyzer user guide (documentation.md is tracker-centric)

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
- Background work pattern: thread::spawn + mpsc::Sender<WorkerMessage> -> poll loop in main_fft.rs
- Rc<RefCell<AppState>> is !Send — extract owned data, spawn with owned data, send result back
- GTK file dialogs disabled (FnfcUsesGtk=false, FnfcUsesZenity=false) — uses FLTK built-in chooser
- All text fields use CallbackTrigger::Changed (NEVER EnterKey — breaks validation/spacebar)
- Segment size solver: target_segments_per_active cleared when user explicitly changes window_length

## Attribution
- Spectrogram visualization and reconstruction inspired by [Audio-Experiments](https://github.com/SebLague/Audio-Experiments) by Sebastian Lague (MIT License)
- Custom gradient editor inspired by [Gradient-Editor](https://github.com/SebLague/Gradient-Editor) by Sebastian Lague
