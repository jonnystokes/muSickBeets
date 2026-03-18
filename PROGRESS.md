# Progress Tracker

> **Docs:** [AGENTS](AGENTS.md) | [Progress](PROGRESS.md) | [Architecture](map.md) | [Coding Rules](CODING_RULES.md) | [Tracker Guide](src/tracker/documentation.md) | [FFT Guide](src/fft_analyzer/fft_analyzer_documentation.md) | [README](README.md) | [Project Memory](ai_memory.md)

---

## Git Rules

- Don't manage GitHub. The user will handle that.
- Only push when the user explicitly asks.

---

## Sub-Agent Launch Policy

Ask for confirmation before launching sub-agents unless the user has given standing permission. Begin every sub-agent prompt with the preamble from `AGENTS.md`. Research sub-agents may run in parallel; only one writer per file set at a time.

---

## Completed Work

### Single-Frame FFT / Reconstruction Correctness -- CLOSED

All known issues resolved or documented as expected DSP behavior.

**What was done (steps 1-12):**
- Researched and documented STFT/ISTFT theory for edge cases
- Instrumented reconstruction with detailed logging
- Fixed centered reconstruction support cropping
- Replaced aggressive 10%-of-max threshold with user-configurable norm floor
- Attempted f32::MIN_POSITIVE threshold -- discovered division-by-near-zero spike issue, reverted to 1e-6
- Added rectangular window (zero gaps, edge-to-edge)
- Added "Max" button for setting recon freq to Nyquist
- Added dense sample dump logging (`FRAME_SAMPLE_DUMP_DBG`)
- Fixed Home button (sidebar overflow from new widgets)
- Upgraded norm floor to f64 for deeper precision (range 1e-30 to 1e-4)
- Fixed norm floor label not updating on recompute
- Added word wrap to resolution info text field
- Added 25 automated tests
- Updated all documentation

**What's established:**
- Silent gaps at window endpoints: expected NOLA violation for tapered windows. User-tunable via Norm Floor. Rectangular eliminates entirely.
- Boundary discontinuities with sparse bins at 0% overlap: expected spectrogram inconsistency. Not a bug. More overlap or full-spectrum reduces them.
- Default recon_freq_max = 5000Hz (not Nyquist). "Max" button sets to Nyquist.
- Rectangular window behaves identically to Hamming when all bins active (both have nonzero endpoints, near-perfect reconstruction). Differs in spectral analysis quality (more leakage, less sidelobe rejection).
- Identity-mode roundtrip: near-perfect with any window when all bins active.

---

## Active Work

Ready for next feature. Upcoming: new controls in the transport bar area (to the right of the Stop button), and spectrogram selection/editing features. Details pending.

---

## Backburner

| Item | Notes |
|------|-------|
| Periodic vs symmetric windows | Decided to keep symmetric. Low priority. |
| Noise floor measurement | User-directed: select a "silent" region, compute floor, save to settings. Not auto. |
| Synthesis window / cross-fade | WOLA smoothing for modified-STFT boundaries at low overlap. Deferred. |

---

## Test Suite (25 tests, all passing)

Located in `reconstructor.rs`:
- Identity roundtrip: Hann 75%, Hamming 0%, Kaiser 0%, Hann 0%, Blackman 0%
- Rectangular: zero gaps, single-frame edge-to-edge
- Sparse bins: Hamming 0% boundary jumps, Hann 50% reduced artifacts
- Gap regression: Hann 44100-sample single frame
- Spike safety: all 5 window types at 0% overlap with sparse bins
- Centered mode: frame count correctness
- Frame boundary diagnostic: rectangular with sparse bins

---

## Strict Project Boundary

- Do not move into instrument-project planning until the user says so.
- Rectangular window was added for future single-frame edge-to-edge capture needs.
