# FFT Analyzer Documentation

Spectrogram visualizer and audio reconstructor. Load WAV files, inspect their frequency content, selectively rebuild audio from chosen frequency ranges, and export the results.

## Quick Start

1. Run: `cargo run --release` (FFT analyzer is the default binary)
2. Click **Open** or press `Ctrl+O` to load a WAV file
3. The spectrogram and waveform render automatically
4. Adjust parameters, then press **Spacebar** to recompute
5. Use the transport controls to play back the reconstructed audio

---

## UI Layout

The window is split into two regions:

- **Left sidebar** -- All parameter controls, grouped into sections (File, Analysis, Display, Reconstruction, Info)
- **Right area** -- Waveform display (top), spectrogram with frequency axis (center), time axis (bottom), transport bar (bottom)
- **Status bars** -- Top bar for messages/warnings, bottom bar for timing and memory info

---

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Spacebar` | Recompute FFT + reconstruction with current parameters |
| `Ctrl+O` | Open WAV file |
| `Ctrl+S` | Save FFT data to CSV |
| `Ctrl+L` | Load FFT data from CSV |
| `Ctrl+E` | Export reconstructed audio as WAV |
| `Ctrl+Q` | Quit |

The **Spacebar** is the primary trigger for recomputation. It is intercepted globally -- pressing it on any widget (buttons, sliders, dropdowns) will trigger a recompute instead of activating that widget. Text input fields are the one exception: spacebar is blocked there too (spaces are not valid in numeric fields).

---

## Mouse Controls

### Spectrogram

| Action | Effect |
|--------|--------|
| **Scroll** (no modifier) | Pan frequency axis up/down |
| **Ctrl + Scroll** | Pan time axis left/right |
| **Alt + Scroll** | Zoom frequency axis (centered on cursor) |
| **Alt + Ctrl + Scroll** | Zoom time axis (centered on cursor) |
| **Click / Drag** | Seek playback position |
| **Hover** | Shows frequency, dB, and time readout below the spectrogram |

The `Swap Zoom Axes` setting in `muSickBeets.ini` swaps which axis Alt vs Alt+Ctrl zooms.

Pan and zoom step sizes are 15% of the visible range per scroll tick. Zoom factors are configurable in settings (default: 1.2x per scroll tick for mouse, 1.5x per click for buttons).

### Gradient Editor

When the colormap is set to **Custom**:

| Action | Effect |
|--------|--------|
| **Left-click** empty space | Add a new gradient stop (color interpolated from neighbors) |
| **Left-click** a stop handle | Select it |
| **Drag** a selected stop | Move it along the gradient |
| **Double-click** a stop | Open color picker |
| **Right-click** a stop | Delete it (minimum 2 stops enforced) |

When any other colormap is selected, the gradient editor displays a read-only preview with the label "Select 'Custom' to edit".

---

## Analysis Parameters

### Segment Size (Window Length)

The FFT window size in samples. Must be even, minimum 4. Larger windows give finer frequency resolution but coarser time resolution.

| Preset | Samples | Frequency Resolution (at 44.1kHz) |
|--------|---------|-----------------------------------|
| 256 | 256 | ~172 Hz |
| 512 | 512 | ~86 Hz |
| 1024 | 1024 | ~43 Hz |
| 2048 | 2048 | ~21.5 Hz |
| 4096 | 4096 | ~10.8 Hz |
| 8192 | 8192 | ~5.4 Hz |
| 16384 | 16384 | ~2.7 Hz |
| 32768 | 32768 | ~1.3 Hz |

You can also type a custom even number directly into the segment size field.

### Overlap

Percentage of overlap between consecutive FFT windows (0-99%). Higher overlap gives more time frames (smoother horizontal resolution) at the cost of more computation. Adjustable via slider.

### Window Type

| Type | Characteristics |
|------|----------------|
| **Hann** | Good general-purpose window. Smooth tapering, low spectral leakage. |
| **Hamming** | Similar to Hann but doesn't reach zero at edges. Slightly narrower main lobe. |
| **Blackman** | Wider main lobe but much lower sidelobes. Best for dynamic range. |
| **Kaiser** | Adjustable via beta parameter. Higher beta = lower sidelobes, wider main lobe. |

### Zero Padding

Multiplies the FFT size by 1x, 2x, 4x, or 8x. Zero-padding interpolates the frequency spectrum, giving smoother-looking results without changing the actual frequency resolution. Higher factors use significantly more memory.

### Center Padding

When enabled, pads the signal with `window_length/2` zeros at each end so the first and last FFT frames are centered on the signal boundaries rather than starting from the edge.

### Segmentation Solver

The solver keeps three related parameters consistent:

- **Segments per Active** -- How many FFT frames span the active time range
- **Bins per Segment** -- Frequency bins per FFT frame (derived from window size and zero-pad factor)
- **Overlap** -- Overlap percentage

When you edit one of these, the solver adjusts the others to maintain consistency. The "last edited" field gets priority.

---

## Display Controls

### Colormaps

| Colormap | Description |
|----------|-------------|
| **Classic** | Black-Purple-Blue-Green-Yellow-Orange-Red (inspired by SebLague) |
| **Viridis** | Perceptually uniform, colorblind-friendly (purple-teal-yellow) |
| **Magma** | Dark-to-bright (black-magenta-orange-yellow) |
| **Inferno** | Dark-to-bright (black-blue-orange-yellow) |
| **Greyscale** | Black to white |
| **Inverted Grey** | White to black |
| **Geek** | Black-green-white (terminal aesthetic) |
| **Custom** | User-defined gradient via the gradient editor |

### Frequency Scale

A slider from linear (0.0) to logarithmic (1.0). Values in between produce a power-law blend. Logarithmic scale better represents how humans perceive pitch (more space for bass, compressed treble).

### Threshold / Ceiling

- **Threshold** -- Minimum dB value displayed. Bins below this are drawn as the lowest color. Default: -87 dB.
- **Ceiling** -- Maximum dB value displayed. Auto-set from the loudest bin when FFT completes, but adjustable manually.

### Brightness / Gamma

- **Brightness** -- Linear scaling of the colormap intensity (0.1 to 3.0, default 1.0).
- **Gamma** -- Power curve applied to the normalized magnitude (0.1 to 3.0, default 1.0). Lower gamma brightens quiet content; higher gamma emphasizes loud content.

---

## Reconstruction

The analyzer can reconstruct audio from the spectrogram using inverse FFT with overlap-add synthesis.

### Parameters

- **Freq Count** -- Maximum number of frequency bins to keep per frame, sorted by magnitude. Lower values produce a cleaner but sparser reconstruction. If left at max, all bins in the frequency range are kept.
- **Freq Min / Max** -- Bandpass filter for reconstruction. Only bins within this Hz range are included in the inverse FFT.
- **Snap to View** -- Copies the current viewport's frequency range into the reconstruction frequency range, then recomputes.

### Active Region

The spectrogram always processes the **full file** for FFT, but reconstruction uses the start/stop time range you set in the Analysis section. The spectrogram dims bins outside the reconstruction's frequency range to visually indicate which content will be included.

Yellow dashed lines on the frequency and time axes mark the reconstruction boundaries.

### Lock to Active

When enabled, the viewport automatically snaps to the processing range after each recompute, so you always see what was just reconstructed.

---

## Transport

| Control | Action |
|---------|--------|
| **Play** | Plays reconstructed audio. If parameters changed since last recompute, triggers recompute first, then auto-plays when done. |
| **Pause** | Pauses playback at current position. |
| **Stop** | Stops playback and resets position to start. |
| **Scrub slider** | Drag to seek within the reconstructed audio. |
| **Repeat** | Toggle looping playback. |

Playback cursor is shown as a vertical line on both the spectrogram and waveform displays.

---

## File Operations

### Open Audio (`Ctrl+O`)

Loads a WAV file (8/16/24/32-bit PCM or 32-bit float, any sample rate, mono or stereo). Stereo files are automatically downmixed to mono. If normalization is enabled (default), the audio is normalized to 97% peak.

### Save FFT Data (`Ctrl+S`)

Exports the current spectrogram to CSV format with metadata headers (#sample_rate, #window_length, #overlap_percent, etc.) followed by one row per FFT frame. Can be loaded later to skip recomputation.

### Load FFT Data (`Ctrl+L`)

Imports a previously saved FFT CSV. Restores the spectrogram, parameters, and viewport state, then runs reconstruction automatically.

### Export WAV (`Ctrl+E`)

Saves the reconstructed audio as a 16-bit PCM WAV file.

---

## Settings (muSickBeets.ini)

All UI state is persisted to `muSickBeets.ini` via the **Save as Default** button. Settings include:

- Analysis parameters (window size, overlap, window type, zero-pad, solver targets)
- Display parameters (colormap, threshold, ceiling, brightness, gamma, freq scale)
- Reconstruction parameters (freq count, freq min/max)
- Viewport state (freq range, time range)
- Zoom factors (button zoom, mouse zoom, swap axes)
- Window dimensions and sidebar width
- Custom gradient stops
- Axis font size, waveform height
- Tooltip visibility, lock-to-active state, repeat playback

Settings are loaded automatically on startup. If the INI file is missing or corrupt, sensible defaults are used.

---

## Waveform Display

Shows the reconstructed audio waveform above the spectrogram.

- **Zoomed out** (>4 samples per pixel): Peak envelope rendering -- each column shows the min/max sample range as a filled band.
- **Zoomed in** (<=4 samples per pixel): Individual sample rendering with Bresenham lines connecting samples and 3x3 dots at each sample point.

A vertical cursor line tracks the current playback position.

---

## Troubleshooting

### Build Fails with Missing Libraries

```bash
apt-get update && apt-get install -y \
  libxinerama-dev libxcursor-dev libxfixes-dev \
  libpango1.0-dev libcairo2-dev libglib2.0-dev
```

### No Sound

- Audio is routed through the system audio device via miniaudio
- Ensure a playback device is available
- Check that reconstruction completed (status bar shows "Ready" with timing)
- Try pressing Play again after a fresh recompute

### Spectrogram Looks Wrong

- Adjust **Threshold** and **Ceiling** sliders to match your audio's dynamic range
- The ceiling auto-sets from the loudest bin; very quiet recordings may need manual adjustment
- Try different **Gamma** values to reveal quiet content

### High Memory Usage

- Large zero-pad factors (4x, 8x) with big window sizes consume significant memory
- The status bar shows current memory usage (VmRSS)
- A warning appears if estimated FFT memory exceeds 256 MB

### Recompute Not Triggering

- Press **Spacebar** anywhere in the window to recompute
- The "Reconstruct / Rerun FFT" button also works
- If the status shows "Processing...", wait for it to finish -- re-entry is blocked during computation
