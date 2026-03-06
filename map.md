# muSickBeets – Project Map

> **Docs:** [AGENTS](AGENTS.md) | [Progress](PROGRESS.md) | [Architecture](map.md) | [Coding Rules](CODING_RULES.md) | [Tracker Guide](documentation.md) | [README](README.md) | [Project Memory](ai_memory.md)

muSickBeets ships two binaries:

1. **FFT Analyzer** (`src/fft_analyzer/`) – FLTK GUI for FFT analysis, selective reconstruction, and CSV interchange.
2. **Tracker** (`src/tracker/`) – CSV-driven synthesizer with multi-channel playback, effects, and WAV export.

Use the sections below to find the module that owns a piece of behavior. Line counts are approximate (from `wc -l`).

---

## FFT Analyzer (`src/fft_analyzer/`)

### Entry, Layout, and Shared State
- `main_fft.rs` (~1038 lines) — Binary entry point. Loads settings, builds UI (`layout::build_ui`), wires callbacks, manages the 16 ms poll loop (worker messages, scrollbar sync, transport, status refresh).
- `layout.rs` (~1004) — Declares `Widgets` and constructs the entire FLTK layout (menus, sidebar controls, displays, transport).
- `app_state.rs` (~402) — Central `AppState`, worker message enums, shared callback handles, derived info helpers, status-bar formatting.
- `validation.rs` (~205) — Input sanitizers (float/uint) plus `_with_recompute` variants that enforce the spacebar defenses.
- `settings.rs` (~773) — INI persistence (load/create/save, “Save as Default”, custom gradient serialization).
- `csv_export.rs` (~428) — FFT CSV import/export, including viewport metadata and post-import reconstruction.
- `test_audio_gen.rs` (~124) — Utility binary for generating chirps/noise for analyzer testing.

### UI Callbacks
- `callbacks_file.rs` (~665) — File I/O (open WAV, save/load FFT CSV, export WAV) and the Reconstruct/Rerun button; spawns FFT/reconstruction workers safely.
- `callbacks_ui.rs` (~1049) — Parameter, display, gradient editor, playback, tooltip, lock-to-active, and “save defaults” callbacks.
- `callbacks_nav.rs` (~529) — Menu actions, scrollbars, time/freq zoom buttons, snap-to-view, and the three-layer spacebar guard wiring.
- `callbacks_draw.rs` (~744) — Draw handlers for spectrogram, waveform, frequency axis, time axis, plus mouse/scroll interactions (seek, hover readout, zoom gestures).

### Data + View Models (`data/`)
- `audio_data.rs` (~120) — WAV loader/normalizer and simple analysis helpers.
- `fft_params.rs` (~167) — Analyzer parameter model (window, overlap, time spans, sample rate).
- `view_state.rs` (~310) — Viewport ranges, reconstruction settings, gradients, coordinate transforms.
- `segmentation_solver.rs` (~321) — Solver that keeps the “segments per active” and “bins per segment” constraints consistent.
- `spectrogram.rs` (~127) — Spectrogram frames, frequency table, helpers (find frame/bin by time/freq, magnitude→dB).
- `mod.rs` (~15) — Re-exports for convenience.

### Processing + Playback
- `processing/fft_engine.rs` (~121) — Rayon-powered forward FFT pipeline with cancellation checks and window management.
- `processing/reconstructor.rs` (~205) — Inverse FFT with overlap-add, freq-range filtering, and top-N bin selection.
- `playback/audio_player.rs` (~202) — Miniaudio device wrapper, playback state, ARC-managed sample buffers.

### Rendering (`rendering/`)
- `color_lut.rs` (~276) — Precomputed LUTs for built-in colormaps plus custom gradient support.
- `spectrogram_renderer.rs` (~324) — Cache-aware spectrogram rasterizer (parallel row rendering, grayed-out out-of-range regions).
- `waveform_renderer.rs` (~452) — Waveform rasterizer with peak/sampled detail levels, cursor overlays, cached RGB buffer.

### UI Utilities (`ui/`)
- `theme.rs` (~59) — Catppuccin-inspired palette + widget styling.
- `tooltips.rs` (~34) — Centralized tooltip enable/disable with theme colors.

---

## Tracker (`src/tracker/`)

### Entry + Sequencing
- `main.rs` (~476) — Tracker binary entry; loads songs, wires miniaudio playback, CLI for selecting tracks, WAV export hooks.
- `parser.rs` (~1136) — Lenient CSV parser (notes, instruments, envelope/effect commands, master bus directives).
- `engine.rs` (~408) — Song scheduler: advances rows, dispatches actions, mixes channel output, manages global tempo.
- `channel.rs` (~687) — Per-channel voice (pitch slides, instrument swaps, ADSR state, effect routing).
- `master_bus.rs` (~574) — Final mix plus master effects (reverb/delay/chorus) with smooth parameter changes.

### Sound Design
- `instruments.rs` (~463) — PolyBLEP-backed oscillators (sine, trisaw, square, pulse, noise) and morphing parameters.
- `envelope.rs` (~548) — ADSR shape registry, preset definitions, and curve interpolation utilities.
- `effects/mod.rs` (~636) — Channel effects (vibrato, tremolo, bitcrusher, distortion, chorus) and shared helpers.
- `audio.rs` (~341) — WAV writer, normalization, RMS/peak statistics, clipping detection.
- `helper.rs` (~435) — Common utilities (note→frequency tables, RNG, interpolation helpers).

---

## Shared / Other Sources
- `src/main.rs` (~526) — Standalone validation playground the project owner uses for experiments; not part of the shipped binaries.
- `fft_analyzer/mod.rs`, `playback/mod.rs`, `processing/mod.rs`, `rendering/mod.rs`, `ui/mod.rs`, `tracker/effects/mod.rs` — Lightweight module glue.
- `Cargo.toml` — Defines binaries (`fft_analyzer`, `tracker`, `test_audio_gen`) and shared dependencies: `fltk`, `miniaudio`, `hound`, `rayon`, `realfft`, `csv`, etc.

Keep this map updated when files move or grow significantly so future agents can jump directly to the right module.
