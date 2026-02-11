# muSickBeets - Project Map

A CSV-driven music tracker synthesizer with real-time audio synthesis and FFT spectrogram analyzer. The project contains two main binaries: the **Tracker** (a music synthesizer) and the **FFT Analyzer** (a spectrogram tool).

---

## FFT Analyzer (`src/fft_analyzer/`)

The FFT Analyzer is an interactive GUI tool for loading WAV audio files, computing spectrograms via FFT, visualizing the results with configurable colormaps and frequency scaling, reconstructing audio from frequency data, and exporting results. It uses FLTK for the GUI and miniaudio for real-time playback.

### Entry Point & Orchestration

**`main_fft.rs`** (397 lines) — Binary entry point for the FFT Analyzer. Declares all submodules, creates the FLTK application, builds the UI via `layout::build_ui()`, wires up all callbacks by calling setup functions from each callback module, creates shared callback closures for cross-module widget state management, and runs the main 16ms poll loop that handles worker thread messages (FFT completion, reconstruction completion), transport position updates, and scrollbar synchronization.

**`app_state.rs`** (158 lines) — Central application state and supporting types. Contains `AppState` (holds audio data, spectrogram, FFT parameters, view state, renderers, player, and processing flags), `DerivedInfo` (computed stats like segment count and frequency resolution), `UpdateThrottle` (prevents excessive redraws), `WorkerMessage` (enum for background thread communication), `SharedCallbacks` (struct grouping the shared closure handles used across callback modules), and the `format_time()` helper.

**`validation.rs`** (68 lines) — Input validation and parsing utilities. Provides revert-based validation for float and unsigned integer text inputs that works reliably on VNC/Termux/remote desktop where keystroke blocking fails. Includes `attach_float_validation()` and `attach_uint_validation()` which set FLTK callbacks, plus `parse_or_zero_*` helpers that treat empty strings as zero.

**`layout.rs`** (723 lines) — UI layout definition. Defines the `Widgets` struct containing cloneable handles to all ~50 FLTK widgets that callbacks need to access (buttons, sliders, inputs, display widgets, scrollbars, etc.). The `build_ui()` function constructs the entire window layout: menu bar, left sidebar with file operations, analysis parameters, display settings, reconstruction controls, and info panel; right panel with waveform strip, spectrogram display with frequency/time axes and zoom controls, and transport bar with playback controls.

### Callback Modules

**`callbacks_file.rs`** (431 lines) — File operation and recompute callbacks. Handles Open Audio (WAV loading with automatic FFT launch), Save FFT (CSV export filtered to processing time range), Load FFT (CSV import with automatic reconstruction), Export WAV (save reconstructed audio), and the Recompute+Rebuild callback that syncs all field values into state, launches full-file FFT on a background thread, with reconstruction auto-triggered on completion.

**`callbacks_ui.rs`** (372 lines) — Parameter, display, playback, and miscellaneous UI callbacks. Parameter callbacks handle time unit toggling (seconds/samples conversion), overlap slider, window type selection (Hann/Hamming/Blackman/Kaiser), segment size +/- buttons, and center/pad toggle. Display callbacks handle colormap selection, frequency scale (log/linear), and throttled threshold/brightness/gamma sliders. Playback callbacks handle play/pause/stop, scrub slider seeking, and repeat mode. Misc callbacks handle tooltip toggle, lock-to-active viewport, and home button (snap to processing range).

**`callbacks_draw.rs`** (402 lines) — Rendering and mouse interaction callbacks. Contains draw callbacks for the spectrogram display (delegates to SpectrogramRenderer with playback cursor overlay), waveform display (delegates to WaveformRenderer with cursor), frequency axis labels (log or linear with reconstruction range markers), and time axis labels (adaptive step sizing with processing range markers). Also handles spectrogram mouse events: click/drag to seek, hover for frequency/dB/time readout, mouse wheel for time zoom, and Ctrl+wheel for frequency zoom.

**`callbacks_nav.rs`** (313 lines) — Navigation, scrollbar, zoom, and keyboard callbacks. Sets up menu bar items (File, Analysis, Display menus with keyboard shortcuts), X/Y scrollbar callbacks with generation counters to avoid fighting user drags, time and frequency zoom +/- button callbacks, snap-to-view (copies viewport bounds into processing fields and triggers recompute), and the spacebar handler (window-level KeyUp triggers recompute).

### Data Module (`data/`)

**`data/audio_data.rs`** (112 lines) — WAV file loading and audio data container. Loads WAV files via the `hound` crate, normalizes multi-channel audio to mono by averaging channels, supports 8/16/24/32-bit PCM and float formats, and provides utilities for duration calculation, Nyquist frequency, sample slicing, and WAV export.

**`data/fft_params.rs`** (147 lines) — FFT analysis configuration. Defines parameters including window length, overlap percentage, sample rate, time range, window type (Hann, Hamming, Blackman, Kaiser with configurable beta), and center/pad option. Provides computed properties like hop length, frequency bins, frequency resolution, and segment count.

**`data/spectrogram.rs`** (80 lines) — FFT analysis results container. Stores a collection of `SpectrogramFrame` structs (each with a timestamp, frequency resolution, and magnitude vector). Provides utilities for magnitude-to-dB conversion, frame lookup by time, frequency bin lookup, and construction from frame vectors with automatic min/max time/freq tracking.

**`data/view_state.rs`** (180 lines) — Viewport and display settings. Manages frequency scaling (linear/log), visible time and frequency ranges, data bounds, colormap selection, display parameters (threshold, brightness, gamma), and reconstruction parameters (frequency count, min/max freq). Provides coordinate mapping functions (`time_to_x`, `x_to_time`, `freq_to_y`, `y_to_freq`) used by all rendering and interaction code.

**`data/mod.rs`** (10 lines) — Module aggregator that re-exports all public types from the data submodules.

### Processing Module (`processing/`)

**`processing/fft_engine.rs`** (92 lines) — Parallel FFT computation engine. Takes audio data and FFT parameters, applies windowing functions, computes forward FFT using the `realfft` crate, and produces a `Spectrogram`. Uses `rayon` for parallel processing of FFT segments.

**`processing/reconstructor.rs`** (146 lines) — Audio reconstruction from spectrogram data. Performs inverse FFT (overlap-add synthesis) with frequency filtering based on reconstruction parameters (frequency count, min/max frequency). Selects the top-N magnitude bins per frame and zeros out bins outside the frequency range.

### Rendering Module (`rendering/`)

**`rendering/color_lut.rs`** (188 lines) — Color lookup table for spectrogram visualization. Pre-computes 256-entry RGB lookup tables for multiple colormaps (Classic rainbow, Viridis, Magma, Inferno, Greyscale, Inverted Grey, Geek green). Applies gamma correction and brightness scaling. Each colormap is defined by interpolating between key color stops.

**`rendering/spectrogram_renderer.rs`** (260 lines) — Spectrogram image rendering with caching. Converts spectrogram data to an RGB pixel buffer using the color LUT, with support for log/linear frequency scaling. Uses parallel pixel processing via rayon. Caches rendered images and invalidates on parameter changes. Grays out regions outside the active processing time range.

**`rendering/waveform_renderer.rs`** (370 lines) — Audio waveform rendering with adaptive detail. Renders waveforms at two detail levels: peak-based rendering when zoomed out (shows min/max envelope) and sample-accurate rendering when zoomed in. Includes playback cursor overlay, zero-line, and cached rendering with invalidation.

### Playback Module (`playback/`)

**`playback/audio_player.rs`** (160 lines) — Real-time audio playback using the miniaudio crate. Supports play, pause, stop, seek, and repeat. Manages a shared audio buffer accessed by the miniaudio callback thread. Tracks playback position for cursor display synchronization.

### UI Module (`ui/`)

**`ui/theme.rs`** (61 lines) — Dark theme configuration using a Catppuccin-inspired color palette. Defines color constants for backgrounds, text, accents, borders, and separators. Applies the theme globally to FLTK widgets via `apply_dark_theme()`.

**`ui/tooltips.rs`** (35 lines) — Tooltip manager with dark theme styling. Provides `set_tooltip()` to apply styled tooltips to widgets and `TooltipManager` to globally enable/disable tooltips.

### Utilities

**`csv_export.rs`** (252 lines) — CSV import/export for spectrogram data. Exports spectrograms with a metadata header containing FFT parameters and reconstruction settings, followed by frame data (time, frequency, magnitude columns). Import parses the metadata header and reconstructs the spectrogram and parameter objects.

**`test_audio_gen.rs`** (127 lines) — Standalone binary that generates test WAV files for analyzer testing. Creates sine waves, chirps (frequency sweeps), multi-tone signals, and white noise at configurable durations and sample rates.

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
