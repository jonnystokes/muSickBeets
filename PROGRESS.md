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

- Historical milestone details intentionally cleared from this file.
- Use this file for current direction only, not long-form history.
- If older implementation details matter later, use git history and the dedicated technical docs.

---

## Active Work

Ready for the next feature add-on.

---

## Backburner

None currently tracked here.

---

## Test Suite

- Refer to the source tests for current coverage instead of tracking details here.

---

## Strict Project Boundary

- Do not move into instrument-project planning until the user says so.
