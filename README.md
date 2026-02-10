# muSickBeets

A music toolkit written in Rust featuring a CSV-driven tracker synthesizer and a real-time FFT spectrogram analyzer.

## Overview

muSickBeets is two tools in one:

- **Tracker** - Compose music by writing simple CSV text files. Each row is a moment in time, each column is a voice/channel. No complex DAW required.
- **FFT Analyzer** - Load WAV files, visualize their frequency content as a spectrogram, selectively reconstruct audio from chosen frequency ranges, and export the results.

## Features

### Tracker
- 12 independent channels with real-time playback
- 5 built-in instruments: Sine, Trisaw, Square, Noise, Pulse
- 6 preset envelopes (percussion, pad, organ, swell, etc.)
- Per-channel effects: amplitude, pan, vibrato, tremolo, bitcrush, distortion, chorus
- Master bus effects: reverb, delay, chorus
- Smooth note glides and effect transitions
- WAV export at 48kHz stereo

### FFT Analyzer
- Dark-themed cockpit UI built with FLTK
- Real-time spectrogram visualization with multiple colormaps
- Configurable FFT parameters (window size, hop length, window type)
- Frequency range and bin count filtering on the spectrogram display
- Audio reconstruction from selected frequency ranges
- CSV export/import of FFT data for external analysis
- Waveform display with sample-level detail when zoomed in
- Viewport controls: zoom, scroll, snap-to-view, lock-to-active

## Quick Start

### Prerequisites

**Rust toolchain** (stable, edition 2024):
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

**Linux build dependencies** (Debian/Ubuntu):
```bash
sudo apt install libxft-dev libpango1.0-dev libxinerama-dev libxcursor-dev libxfixes-dev
```

### Build and Run

```bash
# Run the FFT Analyzer (default binary)
cargo run --release

# Run the Tracker with a song file
cargo run --release --bin tracker

# Generate test WAV files
cargo run --release --bin test_audio_gen
```

## Tracker Song Format

Songs are CSV files. See `assets/song.csv` for a full example and `assets/Tracker_Synth_Command_Reference.txt` for the complete command reference.

```csv
Voice0,Voice1,Voice2,Voice3,Voice4,Voice5,Voice6,Voice7,Voice8,Voice9,Voice10,Voice11
config, title: My Song, export_wav: true, tick_duration: 0.25
c4 sine a:0.5, e4 trisaw:0.5 a:0.4, g4 square a:0.3,,,,,,,,,
-, -, -,,,,,,,,,
., ., .,,,,,,,,,
```

| Symbol | Meaning |
|--------|---------|
| `c4 sine a:0.5` | Trigger note C4, sine wave, 50% volume |
| `-` | Sustain current note |
| `.` | Release (begin fade out) |
| `master rv:0.6'0.4` | Master reverb: room 0.6, mix 0.4 |

For the full documentation on instruments, effects, envelopes, and how to extend the system, see [documentation.md](documentation.md).

## Project Structure

```
src/
  tracker/           Tracker synthesizer binary
    main.rs          Entry point, audio engine, playback
    instruments.rs   Waveform generators
    effects/         Channel and master effects processing
    parser.rs        CSV song file parser
    envelope.rs      ADSR envelope definitions
    ...

  fft_analyzer/      FFT Analyzer binary
    main_fft.rs      Entry point, UI, app state
    data/            Data types (AudioData, Spectrogram, ViewState)
    processing/      FFT engine, audio reconstructor
    rendering/       Spectrogram and waveform pixel renderers
    playback/        Audio output via miniaudio
    ui/              Theme, tooltips
    csv_export.rs    FFT data CSV import/export
```

## Dependencies

| Crate | Purpose |
|-------|---------|
| [fltk](https://crates.io/crates/fltk) | Cross-platform GUI |
| [miniaudio](https://crates.io/crates/om-fork-miniaudio) | Real-time audio playback |
| [hound](https://crates.io/crates/hound) | WAV file I/O |
| [rustfft](https://crates.io/crates/rustfft) / [realfft](https://crates.io/crates/realfft) | FFT computation |
| [rayon](https://crates.io/crates/rayon) | Parallel processing |
| [csv](https://crates.io/crates/csv) | CSV parsing |

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file.

Portions of the spectrogram visualization and reconstruction techniques were inspired by [Audio-Experiments](https://github.com/SebLague/Audio-Experiments) by Sebastian Lague (MIT License). See [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md) for details.

## Author

Jon Stokes
