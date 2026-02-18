# Operation Progress Tracker

## Active TODO
- [ ] Custom gradient/color ramp editor (inspired by [SebLague Gradient-Editor](https://github.com/SebLague/Gradient-Editor))
- [ ] Expanded settings sidebar with scrollbar and full parameter controls
- [ ] Auto-regenerate mode (careful - software rendering only, no GPU)

## Backburner
- File open freeze still happens intermittently (audio device not properly recycled)
  - Debug logging prints thread state to terminal when Open is clicked
  - Possible cause: audio device not properly recycled on file change

## Key Architecture Notes
- Settings loaded in main() before UI, applied to AppState
- FreqScale::Power(f32) replaces old Log/Linear toggle
- Audio normalization happens both on file load AND after reconstruction
- Zoom factors stored in AppState, read from settings
- Freq axis labels use pixel-space-first generation with binary search inversion
- is_seeking flag prevents playback from auto-pausing during cursor drag
- Save As Default captures current AppState into Settings struct and writes INI

## Settings File Location
`settings.ini` in the working directory (created on first run)

## Attribution
- Spectrogram visualization and reconstruction inspired by [Audio-Experiments](https://github.com/SebLague/Audio-Experiments) by Sebastian Lague (MIT License)
- Custom gradient editor inspired by [Gradient-Editor](https://github.com/SebLague/Gradient-Editor) by Sebastian Lague
