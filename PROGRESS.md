# Progress Tracker

> **Docs:** [AGENTS](AGENTS.md) | [Progress](PROGRESS.md) | [Architecture](map.md) | [Coding Rules](CODING_RULES.md) | [Tracker Guide](src/tracker/documentation.md) | [FFT Guide](src/fft_analyzer/fft_analyzer_documentation.md) | [README](README.md) | [Project Memory](ai_memory.md)

This file is for the **main agent**. Historical reference material is tracked separately in `ai_memory.md` and related design docs.

---

## Git Rules

- don't manage GitHub. The user will handle that.

---

## Sub-Agent Launch Policy

**Before launching ANY sub-agent (Task tool), ask the user for confirmation.**

For small, local changes affecting only a few files, the main agent may work directly.

For tasks that require broad reading across many files or exploring unfamiliar areas, the main agent should ask for confirmation and then use a sub-agent instead of reading the whole codebase itself.

**Required flow:**
1. Ask the user whether to launch a sub-agent for the task.
2. If the user says no, do the work yourself instead.
3. If the user says yes, begin the sub-agent prompt with the preamble from AGENTS.md.
4. Never batch-launch multiple sub-agents without asking first.
5. Sub-agents must follow the reading and prompt rules in AGENTS.md.

---

## Active Work

- Idle — awaiting the next user request.

---

## Backburner

- Re-run timing validation only if a future regression is reported (instrumentation is in place for quick checks).
- Monitor resident memory during large FFT sessions; consider an optional "discard spectrogram" control if RAM pressure becomes a recurring issue.

---

## Key Architecture Notes

- Refer to `map.md` for architecture summaries. This section stays empty so the map remains the single source of truth.


