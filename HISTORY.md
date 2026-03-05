# Project History

> **Docs:** [AGENTS](AGENTS.md) | [Progress](PROGRESS.md) | [Issues](CATEGORIZED_ISSUES.md) | [Architecture](map.md) | [Coding Rules](CODING_RULES.md) | [History](HISTORY.md) | [Tracker Guide](documentation.md) | [README](README.md)

Consolidated archive of completed work. For active work see [PROGRESS.md](PROGRESS.md).

---

## Code Review Campaign (Feb 2026)

4 AI reviewers (Claude Opus, Trinity, MiniMax, Big Pickle) independently reviewed the
codebase. Their findings were cross-referenced and deduplicated into 9 categories with
35 total issues, scored using the CMDL difficulty system (see CATEGORIZED_ISSUES.md for
the scoring rubric).

**Categories 1–6 completed (26 issues fixed):**

| Cat | Name | Items | Avg Effort | Key Changes |
|-----|------|-------|------------|-------------|
| 1 | Input Validation & Edge Cases | 7 | Trivial (T=2.1) | NaN-safe binary search, defensive sorts, freq_min_hz=1.0, memory warnings for huge FFTs |
| 2 | Idle/Polling Overhead | 2 | Trivial (T=3) | is_idle guard skipping update_info() when no audio loaded; ViewState clone assessed as negligible |
| 3 | Data Correctness | 4 | Easy (T=2.75) | Forward/inverse FFT magnitude scaling fix, non-destructive normalization (gain preserved), CSV precision to 10 decimals, format_with_commas negative fix |
| 4 | Error Handling & Resilience | 4 | Moderate (T=4.0) | catch_unwind on all worker threads with WorkerPanic message, poisoned mutex recovery, debug logging for try_borrow_mut skips, CSV import header validation |
| 5 | Audio Playback | 3 | Easy (T=3.3) | Device recreated on sample rate change, Arc<Vec<f32>> zero-copy sharing, play_pending flag for auto-play after dirty recompute |
| 6 | UI Thread Blocking | 4 | Moderate (T=5.7) | WAV load/save/CSV export all moved to background threads with WorkerMessage variants (AudioLoaded, WavSaved, CsvSaved) |

**Standalone fix:** Segment Size Controls Overhaul — cleared stale `target_segments_per_active`,
switched to EnterKey confirm for custom sizes, simplified to dropdown + text field.

**Cross-review accuracy assessment:** Claude Opus ~70%, MiniMax ~70%, Big Pickle ~50%, Trinity ~30%.
Full cross-review details in `claude_opus_new_finds_NOTES_PERF_AND_BUGS.md` (kept until
categories 7-9 are complete).

**Remaining:** Categories 7 (Memory), 8 (Rendering), 9 (FFT Pipeline) — NOT STARTED.

---

## Completed Features (Feb 2026)

### FFT Segmentation Redesign
Full overhaul of FFT segmentation controls with bidirectional parameter solving.
6 phases: environment setup, status bar readout, segmentation solver model,
time unit toggle expansion, settings persistence + CSV compatibility, validation + QA.

### Segmentation Overhaul Add-ons
- Resolution trade-off display (live multi-line info)
- dB ceiling slider (auto-set from data, user-adjustable)
- Direct segment size input + presets dropdown
- Zero-padding factor (1x/2x/4x/8x)
- Hop size display (read-only)

### Custom Gradient/Color Ramp Editor
Interactive preview widget with click-to-add, drag-to-move, right-click-delete,
shift+click color picker. 8th colormap dropdown option ("Custom"). Saves to settings.ini.

### UI Restructure — Transport + Cursor Readout
Split transport into 2 rows (scrubber + controls). Cursor readout shows freq/dB/time
between Stop button and L/G time. Event::Enter handler for spectrogram mouse tracking.

### Mouse Navigation Redesign
- No modifier: pan frequency (scroll up/down)
- Ctrl+scroll: pan time
- Alt+scroll: zoom frequency at cursor
- Alt+Ctrl+scroll: zoom time at cursor
- swap_zoom_axes setting persisted

### Lock to Active v2
Matches Home button: snaps both time AND frequency after reconstruction. 0.5s delay
via app::add_timeout3.

### Infrastructure — Debug Flags
debug_flags.rs with CURSOR_DBG, FFT_DBG, PLAYBACK_DBG, RENDER_DBG toggles. dbg_log! macro.

### Bug Fixes
- Text field validation: handle() instead of set_callback() for survival across callback attachment
- Spacebar guard v3 (final): three-layer defense system
- Global time display: L (local) + G (global) in transport
- Time precision: recon_start_time from actual first FFT frame
- Volume fix: overlap-add normalization with adaptive threshold
- Removed 64-sample minimum (now allows 4)
- Fixed Save As Default (SIDEBAR_INNER_H overflow)

---

## Archived Investigations

### VNC Input Testing (Feb 2026)
All 18 FLTK keyboard/mouse event detection methods verified working in VNC environment.
Findings incorporated into spacebar defense system (AGENTS.md Tier 2).

### Environment Variables Snapshot
Confirmed runtime: OPENCODE=1, OPENCODE_EXPERIMENTAL_LSP_TOOL=true, OPENCODE_ENABLE_EXA=1.
Debian chroot, VNC display :2, no GPU. Facts incorporated into AGENTS.md Tier 3.

---

## Attribution

- Spectrogram visualization and reconstruction inspired by
  [Audio-Experiments](https://github.com/SebLague/Audio-Experiments) by Sebastian Lague (MIT License)
- Custom gradient editor inspired by
  [Gradient-Editor](https://github.com/SebLague/Gradient-Editor) by Sebastian Lague
