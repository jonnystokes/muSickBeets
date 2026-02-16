# Operation Progress Tracker

## Completed
- [x] 1. INI settings file system (muSickBeets.ini) - auto-creates with defaults
- [x] 2. Separate view (100-2000 Hz) vs reconstruction (0-5000 Hz) frequency ranges
- [x] 3. Freq scale power slider (0=linear, 1=log, anything between)
- [x] 4. Smart adaptive axis labels for frequency (works at any zoom/scale)
- [x] 5. Smart adaptive time labels (auto-precision on zoom)
- [x] 6. Audio normalization on load (peak normalize to 97%)
- [x] 7. Reconstructed audio normalization (fixes flat waveform + quiet playback)
- [x] 8. Loudness floor dB threshold default -87 dB
- [x] 9. CenterPad default off
- [x] 10. Default view freq 100-2000 Hz
- [x] 11. Configurable zoom factors (button + mouse wheel) via INI
- [x] 12. All settings wired through INI -> AppState -> UI
- [x] 13. BUG FIX: Dragging cursor to end no longer stops playback while mouse held
    - Added `is_seeking` flag to audio player
    - Set on Push/Drag, cleared on Release
    - Audio callback skips auto-pause while seeking
- [x] 14. BUG FIX: File open freeze - added debug logging + stop playback before loading new audio
    - Logs thread state on Open click (is_processing, has_audio, playback_state, etc.)
    - Logs file load details, normalization, FFT thread spawn
    - Stops audio player before loading new audio data
- [x] 15. FEATURE: Filename shown in window title (muSickBeets - filename.wav)
- [x] 16. FEATURE: Save As Default button (writes current settings to muSickBeets.ini)
    - Added to bottom of sidebar
    - Reads all current state values and writes to INI
    - Does NOT auto-save - only saves when button is clicked
- [x] 17. Clearer segmentation controls (segment label now shows: smp / ms / bins)
- [x] 18. More prominent derived values (freq res Hz/bin, time res ms/frame, hop ms)
- [x] 19. Home button now resets BOTH time AND frequency to full data range
- [x] 20. FIX: Settings were not loading on startup
    - Window size from settings.ini now applied after build_ui()
    - File open no longer hardcodes view freq (100-2000) or recon_freq_max (5000)
    - Now clamps existing state values to nyquist instead of overwriting
    - Save As Default now also captures current window dimensions
    - Debug log on startup shows loaded settings values
- [x] 21. Renamed muSickBeets.ini -> settings.ini (auto-migrates old file)

## Still TODO
- [ ] Gradient/color ramp editing from SebLague (custom gradient support)
- [ ] Auto-regenerate mode (like SebLague's autoRegenerate)
    // we have to be careful about regenerating too often because I don't have GPU hardware access and everything must be software rendered.

## Settings File Location
`settings.ini` in the working directory (created on first run, auto-migrates from muSickBeets.ini)

## Key Architecture Notes
- Settings loaded in main() before UI, applied to AppState
- FreqScale::Power(f32) replaces old Log/Linear toggle
- Audio normalization happens both on file load AND after reconstruction
- Zoom factors stored in AppState, read from settings
- Freq axis labels now use smart adaptive spacing with "nice number" candidates
- is_seeking flag prevents playback from auto-pausing during cursor drag
- Save As Default captures current AppState into Settings struct and writes INI

## Known Issues
- File open freeze still happens intermittently when opening a new file while one is loaded
  - Debug logging now prints thread state to terminal when Open is clicked
  - Possible cause: audio device not properly recycled on file change
  - Check terminal output for thread state when freeze occurs
