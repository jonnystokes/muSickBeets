# Operation Progress Tracker

## Recently Completed
- [x] Custom gradient/color ramp editor (SebLague-inspired)
  - GradientStop data structure, eval_gradient(), default 7-stop rainbow
  - Custom variant added to ColormapId (8th dropdown option)
  - Interactive preview widget: click to add, drag to move, right-click to delete, double-click for color picker
  - ColorLUT extended with set_custom_stops() for dynamic gradient rendering
  - Save/load custom gradient to settings.ini
- [x] Remove 64-sample minimum on segment size (now allows down to 2)
- [x] Fix Save As Default button (SIDEBAR_INNER_H overflow from new gradient widget)

## Active TODO
- [ ] Segmentation Overhaul (see detailed notes below)
- [ ] Auto-regenerate mode (careful - software rendering only, no GPU)

## Backburner
- File open freeze still happens intermittently (audio device not properly recycled)
  - Debug logging prints thread state to terminal when Open is clicked
  - Possible cause: audio device not properly recycled on file change

---

## Feature: Segmentation Overhaul

### Problem Statement
The current FFT analysis controls are limited. Segment size only doubles/halves (power-of-2 steps via +/- buttons), there's no way to type an exact value, and the relationship between segment size, overlap, and time/frequency resolution is not intuitive. Users need finer control over FFT segmentation with real-time feedback about the trade-offs.

### Current State (What Exists)

**Segment size** (`fft_params.window_length`):
- Controlled by +/- buttons in layout.rs (lines 216-241)
- Halves/doubles on click in callbacks_ui.rs (min 2 samples)
- Label shows `"{size} smp / {ms} ms"` format
- Default: 8192 samples

**Overlap** (`fft_params.overlap_percent`):
- HorNiceSlider 0-95% in layout.rs (lines 244-258)
- Value synced to fft_params in rerun callback (callbacks_file.rs line 445)
- Default: 75%

**Window type** (`fft_params.window_type`):
- Choice dropdown: Hann/Hamming/Blackman/Kaiser in layout.rs (lines 261-279)
- Kaiser beta exposed as FloatInput (default 8.6)

**Center/Pad** (`fft_params.use_center`):
- CheckButton toggle in layout.rs (lines 281-286)
- Adds zero-padding equal to window_length/2 on each side

**Derived info** (read-only display):
- DerivedInfo in app_state.rs (lines 113-142) computes segments, freq_bins, freq_resolution, hop_length, bin_duration_ms
- Displayed in lbl_info Frame (110px tall, 10pt font)

**Processing flow**:
- Recompute button (or spacebar) syncs all fields -> state, launches full-file FFT on background thread
- FFT processes entire file; sidebar Start/Stop = reconstruction time range only
- Reconstruction auto-triggered on FFT completion via WorkerMessage::FftComplete handler in main_fft.rs

### Proposed Changes

#### 1. Direct Segment Size Input (Replace +/- With Typed Input + Presets)

**Why**: The +/- buttons only allow power-of-2 sizes (2, 4, 8, ..., 8192, 16384, ...). Users should be able to type exact values. Non-power-of-2 sizes work fine with realfft (it handles arbitrary sizes).

**UI changes** (`layout.rs`):
- Replace `btn_seg_minus` / `btn_seg_plus` / `lbl_seg_value` with:
  - A `FloatInput` (or `Input` with uint validation) for typing the segment size in samples
  - A `Choice` dropdown with common presets: 256, 512, 1024, 2048, 4096, 8192, 16384, 32768, 65536, "Custom"
  - Keep the +/- buttons but make them step through the preset list instead of just doubling/halving
- The label below should show computed values: `"{freq_res:.2} Hz/bin | {time_res:.2} ms/frame"`

**Widget changes** (`Widgets` struct in layout.rs):
- Add: `input_seg_size: Input`, `seg_preset_choice: Choice`
- Keep: `btn_seg_minus`, `btn_seg_plus` (repurpose to step through presets)
- Remove: `lbl_seg_value` -> replace with `lbl_seg_info: Frame` (shows resolution stats)

**Callback changes** (`callbacks_ui.rs`):
- Preset dropdown callback: sets input_seg_size value and updates state
- Input validation: clamp to 2..=131072, round to even number (realfft requires even)
- +/- buttons: step to next/prev preset in the list, or if Custom is selected, halve/double

**Data changes**: None - `fft_params.window_length` already accepts any usize

#### 2. Hop Size Control (Alternative to Overlap)

**Why**: Musicians and audio engineers often think in terms of hop size rather than overlap percentage. Hop size = window_length * (1 - overlap/100). Showing both gives better intuition.

**UI changes** (`layout.rs`):
- Add a read-only `Frame` below the overlap slider showing: `"Hop: {hop} smp ({hop_ms:.1} ms)"`
- Alternatively, add a toggle to switch the slider between "Overlap %" and "Hop Size" modes

**Implementation**:
- The hop is already computed in `fft_params.hop_length()` (fft_params.rs line 45-48)
- Just need UI display + optional input mode toggle
- When in hop-size mode, slider range = 1..window_length, and overlap is derived

#### 3. Zero-Padding Factor (Frequency Interpolation)

**Why**: Currently n_fft == window_length. Zero-padding the FFT to a larger size (e.g., 2x or 4x) interpolates between frequency bins, giving smoother spectrograms without changing the actual frequency resolution. This is standard in audio analysis tools.

**Data changes** (`fft_params.rs`):
- Add field: `pub zero_pad_factor: usize` (1 = no padding, 2 = 2x, 4 = 4x)
- Computed FFT size: `n_fft_padded = window_length * zero_pad_factor`
- `num_frequency_bins()` becomes `n_fft_padded / 2 + 1`
- `frequency_resolution()` becomes `sample_rate / n_fft_padded` (finer with padding)
- `generate_window()` stays at window_length (window is NOT padded)

**Processing changes** (`fft_engine.rs`):
- Windowed data: apply window to window_length samples, then pad with zeros to n_fft_padded
- FFT plan uses n_fft_padded instead of n_fft
- `indata` buffer = n_fft_padded length, first window_length samples windowed, rest zeros
- Frequency vector: bin_idx * (sample_rate / n_fft_padded)

**Reconstruction changes** (`reconstructor.rs`):
- IFFT plan uses n_fft_padded
- Magnitude normalization: divide by n_fft_padded (not window_length)
- Output buffer: n_fft_padded length per frame
- Overlap-add hop still based on window_length, not padded size

**UI changes** (`layout.rs`):
- Add `Choice` dropdown: "1x (none)", "2x", "4x", "8x"
- Or a small `Input` field for typing the factor
- Position in ANALYSIS section, after segment size

**Settings** (`settings.rs`):
- Add `zero_pad_factor: usize` to Settings struct and INI [Analysis] section

#### 4. Resolution Trade-off Display (Live Feedback Widget)

**Why**: The time-frequency trade-off is the fundamental concept of STFT analysis. Users need to see how their parameter choices affect both resolutions simultaneously.

**UI changes** (`layout.rs`):
- Replace the current `lbl_seg_value` single-line label with a multi-line info block:
  ```
  Window: 8192 smp (170.7 ms)
  Freq res: 5.86 Hz/bin (4097 bins)
  Time res: 42.7 ms/frame (234 frames)
  Hop: 2048 smp (42.7 ms)
  ```
- This replaces the bottom portion of the current INFO section with live-updating resolution stats
- Update on any parameter change (segment size, overlap, zero-pad factor)

**Callback changes** (`callbacks_ui.rs`):
- New `update_resolution_display()` helper called from:
  - Segment size input/preset changes
  - Overlap slider changes
  - Zero-pad factor changes
  - Window type changes (doesn't affect resolution, but good to confirm)

#### 5. dB Ceiling Slider (Expose Hidden Parameter)

**Why**: `view.db_ceiling` is auto-computed from data max magnitude but never exposed to the user. The ceiling defines the top of the dB range. Combined with threshold_db, this defines the dynamic range of the colormap. Users analyzing quiet recordings or very loud recordings need to adjust this.

**Current state**:
- `db_ceiling` exists in ViewState but is only set when loading CSV data (callbacks_file.rs line 300)
- ColorLUT uses it in `set_params()` (spectrogram_renderer.rs line 34)
- Default: 0.0 dB

**UI changes** (`layout.rs`):
- Add `slider_ceiling: HorNiceSlider` in DISPLAY section, after threshold
- Range: -40.0 to 20.0 dB
- Label: `lbl_ceiling_val: Frame` showing "Ceiling: {val} dB"
- Tooltip: "Maximum dB level for color mapping. Auto-set from data. Adjust to change dynamic range."

**Widget changes**: Add `slider_ceiling` and `lbl_ceiling_val` to Widgets struct

**Callback changes** (`callbacks_ui.rs`):
- Slider callback: update `view.db_ceiling`, invalidate renderer, set dirty flag
- Throttled like the existing threshold/brightness/gamma sliders

**Settings** (`settings.rs`):
- Add `db_ceiling: f32` to Settings struct

### Implementation Order (Recommended)

1. **Resolution display** (least disruptive, immediate UX improvement)
2. **dB ceiling slider** (small addition, completes display controls)
3. **Direct segment size input + presets** (medium complexity, most user impact)
4. **Zero-padding factor** (touches processing pipeline, moderate risk)
5. **Hop size control** (nice-to-have, depends on segment size input existing)

### Files Affected Summary

| File | Changes |
|------|---------|
| `data/fft_params.rs` | Add `zero_pad_factor`, update `num_frequency_bins()`, `frequency_resolution()` |
| `data/view_state.rs` | No changes needed (db_ceiling already exists) |
| `layout.rs` | New/modified widgets for segment input, preset choice, zero-pad choice, ceiling slider, resolution display |
| `callbacks_ui.rs` | New callbacks for all new widgets, `update_resolution_display()` helper |
| `callbacks_file.rs` | Sync zero_pad_factor in rerun callback |
| `processing/fft_engine.rs` | Use n_fft_padded for FFT plan and buffer sizes |
| `processing/reconstructor.rs` | Use n_fft_padded for IFFT plan and normalization |
| `rendering/spectrogram_renderer.rs` | No changes (already reads from spectrogram data) |
| `app_state.rs` | Update DerivedInfo to include zero_pad_factor stats |
| `settings.rs` | Add zero_pad_factor, db_ceiling to Settings struct and INI |
| `csv_export.rs` | Include zero_pad_factor in metadata header |

### Edge Cases and Constraints

- **realfft requires even FFT sizes**: Segment size input must validate to even numbers. Zero-padded size is always window_length * factor, so if window_length is even, padded is even.
- **Minimum segment**: Currently 2 samples. With zero-padding, minimum padded size = 2 * factor. Still fine.
- **Maximum segment**: Practical limit ~131072 (2^17) before FFT becomes slow. Cap the input.
- **Overlap >= 100%**: Already capped at 95% by slider. Keep this constraint.
- **CSV compatibility**: Old CSV files won't have zero_pad_factor in metadata. Default to 1 on import.
- **SIDEBAR_INNER_H**: Currently 1200px. Adding ~4 new widgets (choice + slider + 2 labels) = ~110px. May need to increase to 1320. Check total before committing.
- **Settings migration**: New INI keys absent from old files should fall through to defaults (already handled by parse_ini pattern).

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
