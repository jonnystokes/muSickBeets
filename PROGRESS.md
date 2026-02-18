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
- Custom gradient: Vec<GradientStop> in ViewState, piped through ColorLUT and SpectrogramRenderer

## Settings File Location
`settings.ini` in the working directory (created on first run)

## Attribution
- Spectrogram visualization and reconstruction inspired by [Audio-Experiments](https://github.com/SebLague/Audio-Experiments) by Sebastian Lague (MIT License)
- Custom gradient editor inspired by [Gradient-Editor](https://github.com/SebLague/Gradient-Editor) by Sebastian Lague
