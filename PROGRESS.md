# Operation Progress Tracker

## Recently Completed
- [x] Segmentation Overhaul (all 5 features from plan below):
  1. Resolution trade-off display (live multi-line info in ANALYSIS section)
  2. dB ceiling slider (DISPLAY section, auto-set from data, user-adjustable)
  3. Direct segment size input + presets (typed input + dropdown, +/- steps through presets)
  4. Zero-padding factor (1x/2x/4x/8x, full FFT pipeline integration)
  5. Hop size display (read-only, below overlap slider)
- [x] Bug fixes (Feb 2026):
  - Text field validation: switched from set_callback() to handle() so validation survives
    when functional callbacks are attached later. Spaces and invalid chars now blocked everywhere.
  - Spacebar guard: space consumed at window level (KeyDown+KeyUp+Shortcut all return true),
    preventing it from reaching focused buttons, dropdowns, or text fields.
  - Global time display: transport bar now shows "L" (local) and "G" (global) time.
    Global = absolute time in full file. Local = offset within reconstructed buffer.
  - Time calculation precision: recon_start_time now set from actual first FFT frame time
    (not user-typed Start value), so global time matches spectrogram cursor exactly.
  - Volume reduction fix: reconstructor overlap-add normalization uses adaptive threshold
    (10% of peak window_sum) to prevent edge amplification artifacts with few frames.
- [x] Custom gradient/color ramp editor (SebLague-inspired)
  - GradientStop data structure, eval_gradient(), default 7-stop rainbow
  - Custom variant added to ColormapId (8th dropdown option)
  - Interactive preview widget: click to add, drag to move, right-click to delete, double-click for color picker
  - ColorLUT extended with set_custom_stops() for dynamic gradient rendering
  - Save/load custom gradient to settings.ini
- [x] Remove 64-sample minimum on segment size (now allows down to 2)
- [x] Fix Save As Default button (SIDEBAR_INNER_H overflow from new gradient widget)
- [x] Spacebar guard v3 — FINAL (Feb 2026):
  - Three-layer approach: clear_visible_focus + per-widget handle() + window handler
  - Layer 1 (PRIMARY): `clear_visible_focus()` on ALL buttons, choices, checkbuttons, sliders,
    scrollbars prevents keyboard focus so space never reaches widgets
  - Layer 2 (BACKUP): `block_space!` macro + `handle()` intercepts space, triggers recompute
  - Layer 3 (FALLBACK): Window-level handler catches space when nothing is focused
  - Text inputs: `attach_float/uint_validation_with_recompute()` in validation.rs
  - `scrub_slider` and `gradient_preview` handlers include space blocking inline
  - Only exception: top menu bar (File, Analyze, Display) not guarded
  - Full rules documented in CLAUDE.md for future AI sessions
- [x] Lock to Active v2 (Feb 2026):
  - Now matches Home button behavior: snaps both time AND frequency to active range
  - Uses 0.5s delay via app::add_timeout3 after reconstruction completes
  - Frequency snaps to recon_freq_min/max (same as Home button)
  - Tooltip updated to mention both time and frequency

## Active TODO
- [ ] Segmentation Overhaul (see detailed plan below)
- [ ] Auto-regenerate mode (careful - software rendering only, no GPU)

## Backburner
- File open freeze  used to happens intermittently
  - Debug logging prints thread state to terminal when Open is clicked

---

## Feature: Segmentation Overhaul

### Vision
Give the user full control over FFT segmentation with intuitive controls:
- Select number of **bins per segment** (frequency resolution control)
- Select number of **segments per active window area** (time resolution control)
- Switch between **samples and seconds** for all time-based parameters, with automatic conversion between the two
- Live resolution feedback showing the time/frequency trade-off as parameters change

### Current State (What Exists)

**Segment size** (`fft_params.window_length`):
- Controlled by +/- buttons and preset Choice dropdown (256 to 65536 + Custom)
- Typed input field with uint validation (Input widget)
- Default: 8192 samples

**Overlap** (`fft_params.overlap_percent`):
- HorNiceSlider 0-95%, with hop size display below
- Default: 75%

**Window type** (`fft_params.window_type`):
- Choice dropdown: Hann/Hamming/Blackman/Kaiser
- Kaiser beta exposed as FloatInput (default 8.6)

**Zero-padding** (`fft_params.zero_pad_factor`):
- Choice dropdown: 1x/2x/4x/8x
- Interpolates frequency bins for smoother spectrograms

**Center/Pad** (`fft_params.use_center`):
- CheckButton toggle, adds zero-padding equal to window_length/2 on each side

**Resolution display** (read-only):
- Live multi-line info showing freq resolution, time resolution, bins, segments, hop size
- Updates on any parameter change

**Processing flow**:
- Recompute button (or spacebar) syncs all fields -> state, launches full-file FFT on background thread
- FFT processes entire file; sidebar Start/Stop = reconstruction time range only
- Reconstruction auto-triggered on FFT completion via WorkerMessage::FftComplete handler in main_fft.rs

### Proposed Changes

#### 1. Bins-Per-Segment Control
Allow the user to specify how many frequency bins they want per segment. This is derived from
`n_fft_padded / 2 + 1` currently but could be driven from the other direction — user picks
desired frequency resolution (bins or Hz/bin) and segment size is computed automatically.

#### 2. Segments-Per-Window Control
Allow the user to specify how many time frames (segments) should fit in the active processing
window. This would auto-compute overlap/hop size to achieve the desired segment count for the
given time range and segment size.

#### 3. Samples/Seconds Dual Mode (Enhanced)
The time unit toggle already converts Start/Stop between samples and seconds. Extend this to
ALL time-based displays and inputs so the user can think entirely in samples or entirely in
seconds. The resolution display, hop size, segment duration, etc. should all respect the chosen
unit. Automatic conversion between the two should be seamless.

### Files Likely Affected

| File | Changes |
|------|---------|
| `data/fft_params.rs` | New computed properties for bins-per-segment and segments-per-window |
| `layout.rs` | New input widgets for bin count and segment count controls |
| `callbacks_ui.rs` | New callbacks, bidirectional parameter computation |
| `callbacks_file.rs` | Sync new fields in rerun callback |
| `app_state.rs` | Update DerivedInfo for new computed stats |
| `settings.rs` | Persist new parameters |

### Edge Cases and Constraints
- **realfft requires even FFT sizes**: Any auto-computed segment size must be rounded to even
- **Minimum segment**: 2 samples
- **Maximum segment**: ~131072 before FFT becomes slow
- **Overlap >= 100%**: Capped at 95% by slider
- **CSV compatibility**: Old CSV files won't have new metadata keys. Default on import.
- **Settings migration**: New INI keys absent from old files fall through to defaults

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

## Settings File Location
`settings.ini` in the working directory (created on first run)

## Attribution
- Spectrogram visualization and reconstruction inspired by [Audio-Experiments](https://github.com/SebLague/Audio-Experiments) by Sebastian Lague (MIT License)
- Custom gradient editor inspired by [Gradient-Editor](https://github.com/SebLague/Gradient-Editor) by Sebastian Lague
