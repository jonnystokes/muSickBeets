# Progress Tracker

> **Docs:** [AGENTS](AGENTS.md) | [Progress](PROGRESS.md) | [Issues](CATEGORIZED_ISSUES.md) | [Architecture](map.md) | [Coding Rules](CODING_RULES.md) | [History](HISTORY.md) | [Tracker Guide](documentation.md) | [README](README.md)

This file is for the **main agent**. For completed features, see [HISTORY.md](HISTORY.md).

---

## Git Rules

**Always commit with `git add .` to avoid missing files.**

```bash
git add .
git commit -m "Your commit message here"
git push
```

- Update ALL files (code, docs, progress) BEFORE committing.
- Never commit `settings.ini` (it's in `.gitignore`, auto-generated at runtime).
- Always push to remote after updating progress or documentation files.

---

## Sub-Agent Launch Policy

**Before launching ANY sub-agent (Task tool), ask the user for confirmation.**

Sub-agents consume API usage aggressively and **cannot pause** if usage runs out —
they crash. The main agent CAN pause and wait for usage to refill; sub-agents cannot.

**Required flow:**
1. Use `mcp_question` to ask: "I'd like to launch a sub-agent for [task]. Proceed?"
2. If user says no, do the work yourself instead.
3. If user says yes, begin the sub-agent prompt with the preamble from AGENTS.md.
4. Never batch-launch multiple sub-agents without asking first.

---

## Active Work

### Code Review Issues ([CATEGORIZED_ISSUES.md](CATEGORIZED_ISSUES.md))

**ALL 9 categories COMPLETE** (35 issues total: 28 code fixes, 7 assessed/already-fixed/not-actionable).

- [x] **Category 7:** Memory Efficiency (2 items) — COMPLETE
  - Shared frequency vector on Spectrogram (~16 MB savings)
  - Zero-copy frame range for reconstruction (~49 MB savings)
- [x] **Category 8:** Rendering Performance (5 items) — COMPLETE
  - Binary search bin lookup (O(n) → O(log n))
  - Skip sort when all bins active
  - Proper view hash (DefaultHasher + to_bits)
  - Parallel waveform peak rendering (rayon)
  - GUI-blocking render assessed, deferred (existing mitigations sufficient)
- [x] **Category 9:** FFT/Reconstruction Pipeline (4 items) — COMPLETE
  - Thread-local FFT planner (reuse across rayon threads)
  - Worker cancellation via Arc<AtomicBool>
  - Sequential overlap-add assessed (inherent to algorithm)
  - Magnitude scaling already fixed in Category 3
  - Per-frame FFT planner allocation
  - No worker cancellation mechanism
  - Sequential overlap-add (inherent to algorithm)
  - Magnitude scaling mismatch (forward/inverse)

---

## Backburner

- [ ] File open freeze — intermittent, debug logging in place but root cause not found
- [ ] Analysis presets layer (transients, tonal, balanced)
- [ ] Per-section reset-to-default (Analysis / Display / Reconstruction)
- [ ] FFT Analyzer user guide (documentation.md is tracker-centric)
- [ ] Update map.md line counts and architecture summaries after major refactors
- [ ] Update README.md after this development stage

---

## Key Architecture Notes

- Settings loaded in main() before UI, applied to AppState
- FreqScale::Power(f32) replaces old Log/Linear toggle
- Audio normalization happens both on file load AND after reconstruction
- Zoom factors stored in AppState, read from settings
- Freq axis labels use pixel-space-first generation with binary search inversion
- is_seeking flag prevents playback from auto-pausing during cursor drag
- Save As Default captures current AppState into Settings struct and writes INI
- Custom gradient: Vec<GradientStop> in ViewState, piped through ColorLUT and SpectrogramRenderer
- Settings file: `settings.ini` in working directory (created on first run)
