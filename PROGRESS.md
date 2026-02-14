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

## Still TODO (for next session)
- [ ] Clearer segmentation controls UI (show total segments, samples/segment, bins/segment more prominently)
- [ ] More prominent display of derived values (freq resolution, time resolution, etc.)
- [ ] Gradient/color ramp editing from SebLague (custom gradient support)
- [ ] Save settings back to INI when changed in UI (currently only loads on startup)
- [ ] Auto-regenerate mode (like SebLague's autoRegenerate)

## Settings File Location
`muSickBeets.ini` in the working directory (created on first run)

## Key Architecture Notes
- Settings loaded in main() before UI, applied to AppState
- FreqScale::Power(f32) replaces old Log/Linear toggle
- Audio normalization happens both on file load AND after reconstruction
- Zoom factors stored in AppState, read from settings
- Freq axis labels now use smart adaptive spacing with "nice number" candidates
