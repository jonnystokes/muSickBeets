# Progress Tracker

> **Docs:** [AGENTS](AGENTS.md) | [Progress](PROGRESS.md) | [Architecture](map.md) | [Coding Rules](CODING_RULES.md) | [Tracker Guide](src/tracker/documentation.md) | [FFT Guide](src/fft_analyzer/fft_analyzer_documentation.md) | [README](README.md) | [Project Memory](ai_memory.md)

This file is for the **main agent**. Historical reference material is tracked separately in `ai_memory.md` and related design docs.

---

## Git Rules

- don't manage GitHub. The user will handle that.

---

## Sub-Agent Launch Policy

Use sub-agents proactively when they reduce context bloat or improve quality.

If the user has already given standing permission in the current conversation, repeated confirmation is not needed.

For small, local changes affecting only a few files, the main agent may work directly.

For tasks that require broad reading across many files, exploring unfamiliar areas, comparing design options, auditing tests, reviewing performance/risk, or reconstituting scoped context, prefer sub-agents over pulling everything into the main context.

**Working rules:**
1. Begin every sub-agent prompt with the preamble from `AGENTS.md`.
2. Research/review sub-agents may run in parallel when they are not editing files.
3. If any sub-agent is writing code, keep one writer per file set.
4. Use sub-agents well for: architecture tracing, data model mapping, test audits, dead-code/duplication hunts, build/deployment impact review, alternative-solution generation, migration slicing, debugging by competing hypotheses, risk checking, performance review.
5. A strong default pattern is: architect/research -> optional parallel reviewers -> one implementer -> one reviewer/test pass.
6. Sub-agents must follow the reading and prompt rules in `AGENTS.md`.

---

## Active Work

- In progress: single-frame FFT / reconstruction correctness research and bug-fix prep.

### Current Worklist

1. Done -- wrote a consolidated research/roadmap note for single-frame FFT behavior and the future instrument workflow.
2. Done -- instrumented one-frame reconstruction math (`num_frames`, output length, `window_sum`, zeroed spans).
3. Done -- audited center pad on/off semantics end-to-end and recorded the findings in `SINGLE_FRAME_FFT_NOTES.md`.
4. Done -- fixed centered reconstruction length/cropping to use actual frame support before cropping.
5. Next -- replace or redesign the aggressive `window_sum` edge-zeroing rule.
6. Measure blank-edge size per window type and document which settings actually control it.
7. Design a future dedicated single-frame export mode for the later instrument binary.

### Bug Fixes Completed Between Steps

- Fixed the centered solver/UI frame-count mismatch so the displayed segment count,
  derived info, and FFT progress totals align much better with the actual FFT
  engine behavior when center padding is enabled.

---

## Key Architecture Notes

- Refer to `map.md` for architecture summaries. This section stays empty so the map remains the single source of truth.
