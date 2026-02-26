# muSickBeets - Project Map

A CSV-driven music tracker synthesizer with real-time audio synthesis and FFT spectrogram analyzer. The project contains two main binaries: the **Tracker** (a music synthesizer) and the **FFT Analyzer** (a spectrogram tool).

---

## FFT Analyzer (`src/fft_analyzer/`)

The FFT Analyzer is an interactive GUI tool for loading WAV audio files, computing spectrograms via FFT, visualizing the results with configurable colormaps and frequency scaling, reconstructing audio from frequency data, and exporting results. It uses FLTK for the GUI and miniaudio for real-time playback.

### Entry Point & Orchestration

**`main_fft.rs`** (920 lines) — Binary entry point for the FFT Analyzer. Declares all submodules, creates the FLTK application, builds the UI via `layout::build_ui()`, loads settings from INI via `Settings::load_or_create()` and applies them to `AppState` (including custom gradient), wires up all callbacks by calling setup functions from each callback module (including the gradient editor and spacebar guards — which MUST be last), creates shared callback closures for cross-module widget state management (`update_info`, `update_seg_label`, `enable_audio_widgets`, `enable_spec_widgets`, `enable_wav_export`), disables GTK/Zenity file dialogs (freeze fix for non-GNOME environments), and runs the main 16ms poll loop that handles worker thread messages (`FftComplete` with auto-reconstruction, `ReconstructionComplete` with Lock-to-Active delayed viewport snap, `AudioLoaded`, `WavSaved`, `CsvSaved`, `WorkerPanic`, `Disconnected` detection), transport position updates (local + global time), scrollbar synchronization, and axis label redraws. Includes `is_idle` guard to skip expensive updates when no audio is loaded.

**`app_state.rs`** (296 lines) — Central application state and supporting types. Contains `AppState` (holds audio data, spectrogram, FFT parameters, view state, renderers, player, processing flags, `source_norm_gain`, `play_pending`, `device_sample_rate`), `DerivedInfo` (computed stats like segment count and frequency resolution), `UpdateThrottle` (prevents excessive redraws), `WorkerMessage` (enum for background thread communication: `FftComplete`, `ReconstructionComplete`, `AudioLoaded`, `WavSaved`, `CsvSaved`, `WorkerPanic`), `SharedCallbacks` (struct grouping the shared closure handles), `MsgLevel` enum and `set_msg()` helper for color-coded transient messages in the top message bar (Info/Warning/Error), and the `format_time()` helper.

**`validation.rs`** (205 lines) — Input validation and parsing utilities. Provides revert-based validation for float and unsigned integer text inputs using `handle()` (not `set_callback()`) so validation survives when functional callbacks are later attached. Four validation functions: `attach_float_validation()` and `attach_uint_validation()` (plain validation with space blocking), plus `attach_float_validation_with_recompute()` and `attach_uint_validation_with_recompute()` (validation + spacebar blocking + recompute trigger via `btn_rerun.do_callback()`). The `_with_recompute` variants are applied by `setup_spacebar_guards()` to REPLACE the plain handlers. Also includes `parse_or_zero_*` helpers that treat empty strings as zero.

**`debug_flags.rs`** (23 lines) — Toggleable debug flags (`CURSOR_DBG`, `FFT_DBG`, `PLAYBACK_DBG`, `RENDER_DBG`) as `const bool` plus `dbg_log!` macro for conditional debug output. All flags `#[allow(dead_code)]`.

**`layout.rs`** (1004 lines) — UI layout definition. Defines the `Widgets` struct containing cloneable handles to all ~50 FLTK widgets that callbacks need to access. The `build_ui()` function constructs the entire window layout: menu bar row (MenuBar 180px fixed + separator + `msg_bar` Frame for transient messages filling remaining width), left sidebar (in a Scroll widget, SIDEBAR_INNER_H=1400) with file operations, analysis parameters (segment size with preset Choice dropdown + typed Input field, overlap slider, segments-per-active and bins-per-segment inputs, window type choice, Kaiser beta, center/pad toggle, zero-pad factor, resolution info display), display settings (colormap dropdown, gradient editor preview, frequency scale, threshold/ceiling/brightness/gamma sliders), reconstruction controls (freq count, freq min/max, snap-to-view), info panel, and misc (tooltips toggle, lock-to-active, home, save defaults); right panel with waveform strip, spectrogram display with frequency/time axes and zoom controls, and transport bar (2 rows: scrub slider + controls with cursor readout showing local + global time).

### Callback Modules

**`callbacks_file.rs`** (576 lines) — File operation and recompute callbacks. Handles Open Audio (WAV loading on background thread with `AudioLoaded` message), Save FFT (CSV export on background thread with `CsvSaved`), Load FFT (CSV import with automatic reconstruction), Export WAV (save on background thread with `WavSaved`), and the Recompute+Rebuild callback that syncs all field values into state, launches full-file FFT on a background thread with `catch_unwind` panic protection. All `thread::spawn` sites send `WorkerPanic` on unwind. Includes memory warning for huge zero-padded FFTs.

**`callbacks_ui.rs`** (1051 lines) — Parameter, display, gradient editor, playback, and miscellaneous UI callbacks. Parameter callbacks handle time unit toggling (seconds/samples conversion), overlap slider, window type selection (Hann/Hamming/Blackman/Kaiser), segment size preset Choice and typed Input (both clear `target_segments_per_active` on explicit user changes to prevent solver override), segments-per-active and bins-per-segment live inputs, zero-pad factor choice, and center/pad toggle. Segment size controls use `msg_bar` for color-coded feedback when the solver adjusts values. Display callbacks handle colormap selection (including Custom with gradient preview redraw), frequency scale (log/linear), and throttled threshold/ceiling/brightness/gamma sliders. Gradient editor section (`setup_gradient_editor`) provides draw and handle callbacks for the interactive gradient preview widget (with spacebar blocking inline). Playback callbacks handle play/pause/stop, scrub slider seeking (with spacebar blocking inline), and repeat mode. Misc callbacks handle tooltip toggle, lock-to-active viewport, home button (snaps both time AND frequency), and Save As Default (writes current AppState to settings.ini).

**`callbacks_draw.rs`** (718 lines) — Rendering and mouse interaction callbacks. Contains draw callbacks for the spectrogram display (delegates to SpectrogramRenderer with playback cursor overlay), waveform display (delegates to WaveformRenderer with cursor), frequency axis labels (pixel-space-first generation with binary search inversion and nice-number rounding), time axis labels (adaptive step sizing with processing range markers), and cursor readout. Handles spectrogram mouse events: click/drag to seek, hover for frequency/dB/time readout in cursor_readout Frame, mouse wheel with modifier-based navigation (no mod=pan freq, Ctrl=pan time, Alt=zoom freq, Alt+Ctrl=zoom time, configurable via `swap_zoom_axes`). Includes `format_with_commas` (negative-number safe).

**`callbacks_nav.rs`** (529 lines) — Navigation, scrollbar, zoom, keyboard, and spacebar guard callbacks. Sets up menu bar items (File, Analysis, Display menus with keyboard shortcuts), X/Y scrollbar callbacks with generation counters to avoid fighting user drags, time and frequency zoom +/- button callbacks, snap-to-view (copies viewport bounds into processing fields and triggers recompute), the window-level spacebar handler (KeyUp triggers recompute), and `setup_spacebar_guards()` — the per-widget spacebar defense system using a `block_space!` macro and `clear_visible_focus()` on all interactive widgets (buttons, choices, checkbuttons, sliders, scrollbars). Also re-attaches text input validation handlers with recompute-aware versions. Must be called LAST in the callback setup chain.

### Data Module (`data/`)

**`data/audio_data.rs`** (120 lines) — WAV file loading and audio data container. Loads WAV files via the `hound` crate, normalizes multi-channel audio to mono by averaging channels, supports 8/16/24/32-bit PCM and float formats. Rejects sample_rate==0. Provides utilities for duration calculation, Nyquist frequency, sample slicing, and WAV export.

**`data/fft_params.rs`** (167 lines) — FFT analysis configuration. Defines parameters including window length, overlap percentage, sample rate, time range, window type (Hann, Hamming, Blackman, Kaiser with configurable beta), center/pad option, zero-pad factor, `target_segments_per_active`, `target_bins_per_segment`, and `last_edited_field`. Provides computed properties like hop length, frequency bins, frequency resolution, and segment count.

**`data/spectrogram.rs`** (101 lines) — FFT analysis results container. Stores a collection of `SpectrogramFrame` structs (each with a timestamp, frequency resolution, and magnitude vector). Provides utilities for magnitude-to-dB conversion, NaN-safe frame lookup by time (binary search with `unwrap_or(Equal)`), frequency bin lookup, and construction from frame vectors with defensive sort and automatic min/max time/freq tracking.

**`data/segmentation_solver.rs`** (321 lines) — Bidirectional segmentation parameter solver. Given `LastEditedField` (Overlap, SegmentsPerActive, or BinsPerSegment) and optional target values, solves for the dependent parameters while respecting constraints (even window, min 4, max = active samples, overlap 0-99%). Uses brute-force local search around analytical approximation for segment count matching. Includes comprehensive test suite.

**`data/view_state.rs`** (313 lines) — Viewport, display settings, and gradient data. Defines `GradientStop` (position + RGB color, all 0..1 floats), `eval_gradient()` for linear interpolation between sorted stops, and `default_custom_gradient()` (SebLague-style 7-stop rainbow). Defines `FreqScale` (Linear/Log/Power blend), `ColormapId` (8 variants: Classic, Viridis, Magma, Inferno, Greyscale, InvertedGrey, Geek, Custom), `ViewState` (frequency/time viewport ranges, display params, custom_gradient vec, reconstruction params, data bounds), coordinate mapping functions (`time_to_x`, `x_to_time`, `freq_to_y`, `y_to_freq` with Power blend using geometric interpolation and binary search inversion), and `TransportState`. `reset_zoom()` uses `freq_min_hz = 1.0`.

**`data/mod.rs`** (15 lines) — Module aggregator that re-exports all public types from the data submodules including `GradientStop`, `default_custom_gradient`, `eval_gradient`, `LastEditedField`, and `SolverConstraints`.

### Processing Module (`processing/`)

**`processing/fft_engine.rs`** (101 lines) — Parallel FFT computation engine. Takes audio data and FFT parameters, applies windowing functions, computes forward FFT using the `realfft` crate, and produces a `Spectrogram`. Uses `rayon` for parallel processing of FFT segments. Stores magnitudes as `(|X[k]| / N) * amplitude_scale` where amplitude_scale is 2 for non-DC/Nyquist, 1 for DC/Nyquist.

**`processing/reconstructor.rs`** (165 lines) — Audio reconstruction from spectrogram data. Performs inverse FFT (overlap-add synthesis) with frequency filtering based on reconstruction parameters (frequency count, min/max frequency). Selects the top-N magnitude bins per frame and zeros out bins outside the frequency range. Correctly undoes forward-pass scaling before IFFT (multiplies by N for DC/Nyquist, N/2 for other bins).

### Rendering Module (`rendering/`)

**`rendering/color_lut.rs`** (279 lines) — Color lookup table for spectrogram visualization. Pre-computes 1024-entry RGB lookup tables for 8 colormaps (Classic rainbow, Viridis, Magma, Inferno, Greyscale, Inverted Grey, Geek green, Custom). Applies gamma correction and brightness scaling. The Custom colormap reads from a dynamic `custom_stops: Vec<GradientStop>` field updated via `set_custom_stops()`, using `eval_gradient()` for linear interpolation. Each built-in colormap is defined by interpolating between key color stops.

**`rendering/spectrogram_renderer.rs`** (328 lines) — Spectrogram image rendering with caching. Converts spectrogram data to an RGB pixel buffer using the color LUT, with support for log/linear frequency scaling. Uses parallel pixel processing via rayon. Caches rendered images and invalidates on parameter changes (including custom gradient changes via hash). Passes custom gradient stops through to ColorLUT via `update_lut()`. Grays out regions outside the active processing time range.

**`rendering/waveform_renderer.rs`** (433 lines) — Audio waveform rendering with adaptive detail. Renders waveforms at two detail levels: peak-based rendering when zoomed out (shows min/max envelope) and sample-accurate rendering when zoomed in. Includes playback cursor overlay, zero-line, and cached rendering with invalidation.

### Playback Module (`playback/`)

**`playback/audio_player.rs`** (202 lines) — Real-time audio playback using the miniaudio crate. Supports play, pause, stop, seek, and repeat. Uses `Arc<Vec<f32>>` for zero-copy sample sharing. Manages a shared audio buffer accessed by the miniaudio callback thread via `lock_playback()` helper (recovers from poisoned mutexes instead of panicking). Tracks `device_sample_rate` and recreates the device when sample rate changes. Tracks playback position for cursor display synchronization.

### UI Module (`ui/`)

**`ui/theme.rs`** (59 lines) — Dark theme configuration using a Catppuccin-inspired color palette. Defines color constants for backgrounds (`BG_DARK`, `BG_PANEL`, `BG_WIDGET`), text (`TEXT_PRIMARY`, `TEXT_SECONDARY`, `TEXT_DISABLED`), accents (`ACCENT_BLUE`, `ACCENT_GREEN`, `ACCENT_RED`, `ACCENT_YELLOW`, `ACCENT_MAUVE`), borders, and separators. Applies the theme globally to FLTK widgets via `apply_dark_theme()`.

**`ui/tooltips.rs`** (34 lines) — Tooltip manager with dark theme styling. Provides `set_tooltip()` to apply styled tooltips to widgets and `TooltipManager` to globally enable/disable tooltips.

### Settings

**`settings.rs`** (767 lines) — INI-based settings persistence. Defines the `Settings` struct with all saveable parameters (analysis including `target_segments_per_active`/`target_bins_per_segment`/`last_edited_field`, view, display, reconstruction, audio normalization, zoom factors, `swap_zoom_axes`, window dimensions, axis labels, waveform height, UI toggles, playback, custom gradient, and theme colors). Supports `load_or_create()` (with migration from old `muSickBeets.ini`), `save()`, `from_app_state()` (snapshot current state for Save As Default), and `parse_custom_gradient()`. Custom gradient is serialized as pipe-delimited `pos:r:g:b` float strings. INI parsing uses a flat key-value map (section headers ignored). Comprehensive test suite with round-trip and backward-compat tests.

### Utilities

**`csv_export.rs`** (378 lines) — CSV import/export for spectrogram data. Exports spectrograms with a metadata header containing FFT parameters and reconstruction settings (including solver targets), followed by frame data (time at `{:.10}` precision, frequency, magnitude columns). Import parses the metadata header and reconstructs the spectrogram and parameter objects. Row 2 skip validates existence. Comprehensive test suite.

**`test_audio_gen.rs`** (126 lines) — Standalone binary that generates test WAV files for analyzer testing. Creates sine waves, chirps (frequency sweeps), multi-tone signals, and white noise at configurable durations and sample rates.

---

## Tracker (`src/tracker/`)

The Tracker is a CSV-driven music synthesizer that reads song data from spreadsheet-compatible CSV files and renders them to audio. It supports multiple channels, instruments with anti-aliased waveforms, ADSR envelopes, per-channel effects, and master bus processing.

**`main.rs`** (453 lines) — Tracker binary entry point. Handles configuration management (instrument assignments, tempo, effects), CSV song file loading via the parser, real-time playback with miniaudio, and WAV export. Provides a text-based UI for selecting and playing songs.

**`parser.rs`** (1121 lines) — Forgiving CSV song file parser. Converts spreadsheet cells into playable `Action` structs, handling note notation (e.g., "C#4"), instrument selection, volume, effects commands, and master bus commands. Supports inline comments, instrument aliases, and provides detailed error/warning reporting for malformed input.

**`engine.rs`** (407 lines) — Main sequencing and mixing engine. Processes rows at the configured tempo, dispatches actions to channels, collects and mixes channel outputs, applies master bus effects, and writes the final stereo audio buffer. Handles row-level timing and smooth parameter transitions.

**`channel.rs`** (682 lines) — Individual synthesizer voice. Manages pitch (with portamento/slides), instrument selection (with crossfade transitions), ADSR envelope control, per-channel volume and panning, and routes audio through the channel effects chain. Supports note-on, note-off, pitch slides, and instrument changes.

**`instruments.rs`** (450 lines) — Sound generator registry. Defines 6 waveform types: sine, triangle-saw (morphable), square, pulse (variable width), and noise. Uses PolyBLEP anti-aliasing for band-limited synthesis of non-sinusoidal waveforms to minimize aliasing artifacts.

**`envelope.rs`** (551 lines) — ADSR envelope system with a registry of preset shapes. Each envelope has configurable attack, decay, sustain level, and release with selectable curve types (linear, exponential, logarithmic). The registry provides named presets like "pluck", "pad", "organ", "perc", etc.

**`effects/mod.rs`** (631 lines) — Unified effects system. Channel-level effects include vibrato, tremolo, bitcrusher, distortion, and chorus. Master bus effects include reverb (multi-tap delay network), stereo delay, and chorus. Each effect has configurable parameters and smooth interpolation for glitch-free transitions.

**`master_bus.rs`** (561 lines) — Final mixing stage. Applies global effects (reverb, delay, chorus) with smooth parameter transitions, handles master volume and panning, and manages the effects processing chain. Supports both wet/dry mixing and bypass modes.

**`audio.rs`** (326 lines) — WAV file I/O and audio analysis. Handles stereo WAV export with configurable bit depth, provides audio statistics (peak level, RMS, DC offset, clipping detection), and includes normalization and gain utilities.

**`helper.rs`** (423 lines) — Utility functions. Contains a pre-computed frequency table covering octaves 0-20 for fast note-to-frequency lookup, interpolation functions (linear, exponential, logarithmic), a simple random number generator, and pitch string parsing (e.g., "A4" to frequency).

---

## Root

**`src/main.rs`** (54 lines) — A small standalone demo/test binary with a float input validation example. Not the main application entry point (the actual binaries are `tracker` and `fft_analyzer`).

**`Cargo.toml`** — Project configuration. Defines three binary targets (`tracker`, `fft_analyzer`, `test_audio_gen`) with `fft_analyzer` as the default. Key dependencies: `fltk` (GUI), `miniaudio` (audio playback), `realfft`/`rustfft` (FFT), `rayon` (parallelism), `hound` (WAV I/O), `csv` (parsing).

---

## Documentation Files

| File | Purpose |
|------|---------|
| `AGENTS.md` / `CLAUDE.md` | AI session rules (identical, kept in sync). Spacebar rules, text field validation, CallbackTrigger rules, harness detection, sub-agent policy, experimental tools docs. |
| `CATEGORIZED_ISSUES.md` | Master issue tracker. 9 categories, 35 items. Categories 1-6 COMPLETE + standalone segment size fix. Categories 7-9 NOT STARTED. |
| `PROGRESS.md` | Feature/fix completion log with architecture notes. |
| `ai_memory.md` | Quick-reference architecture and gotchas for AI sessions. |
| `documentation.md` | Tracker user guide (instruments, effects, envelopes, song format). |
| `README.md` | Project overview, setup, screenshots. |
| `map.md` | This file — project structure map with per-file descriptions. |
| `vnc_input_testing.md` | FLTK keyboard/mouse VNC input testing reference. |
| `variables.md` | Environment variable dump for user study. |
| `claude_opus_new_finds_NOTES_PERF_AND_BUGS.md` | Cross-reference of all 4 AI code reviews (read-only reference). |
| `THIRD_PARTY_LICENSES.md` | Third-party license attributions. |
