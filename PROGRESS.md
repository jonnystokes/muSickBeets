# Progress Tracker

> **Docs:** [AGENTS](AGENTS.md) | [Progress](PROGRESS.md) | [Issues](CATEGORIZED_ISSUES.md) | [Architecture](map.md) | [Coding Rules](CODING_RULES.md) | [Tracker Guide](documentation.md) | [README](README.md)

This file is for the **main agent**. Historical reference material is tracked separately in `ai_memory.md` and related design docs.

---

## Git Rules

- don't  manage GitHub. The user will handle that. 

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

- Idle — awaiting the next user request.
---

## Backburner

- Re-run timing validation only if a future regression is reported (instrumentation is in place for quick checks).
- Monitor resident memory during large FFT sessions; consider an optional “discard spectrogram” control if RAM pressure becomes a recurring issue.
---

## Key Architecture Notes

- Refer to `map.md` for architecture summaries. This section stays empty so the map remains the single source of truth.
