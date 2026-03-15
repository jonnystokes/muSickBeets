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

- In progress: single-frame FFT / reconstruction correctness stabilization.

### Current Step System

0. Ongoing rule -- keep tracking/docs current, remove stale items, and keep the active worklist synchronized with reality.
1. Done -- wrote a consolidated research/roadmap note for single-frame FFT behavior and the future instrument workflow.
1.5. Partial / available -- added focused debugging instrumentation (`SINGLE_FRAME_DBG`) and can expand debug categories further if needed.
2. Done -- instrumented one-frame reconstruction math (`num_frames`, output length, `window_sum`, zeroed spans).
3. Done -- audited center pad on/off semantics end-to-end and recorded the findings in `SINGLE_FRAME_FFT_NOTES.md`.
3.5. Done -- fixed the centered solver/UI frame-count mismatch so displayed counts and progress totals match the FFT engine better.
4. Done -- fixed centered reconstruction length/cropping to use actual frame support before cropping.
5. Done -- replaced the old aggressive `window_sum` threshold with tiny-epsilon normalization and verified that broad cliff-drop regions shrank to small support-limited gaps.
6. In progress -- measure residual blank-edge / gap behavior by window type / overlap / frame count and document exactly which settings still control it.
7. Deferred -- future instrument/export work remains explicitly blocked until the user says the FFT analyzer foundation is solid and it is time to move on.

### Bug Fixes Completed Between Numbered Steps

- Step 3.5 fixed the centered solver/UI frame-count mismatch so the displayed
  segment count, derived info, and FFT progress totals align much better with
  the actual FFT engine behavior when center padding is enabled.

### Strict Project Boundary

- Do not move into instrument-project planning or implementation until the user
  explicitly says the current FFT analyzer bug-fix/stability work is complete.
- Future instrument ideas may be remembered in notes, but they are not active
  work and should not shape implementation beyond protecting current metadata
  correctness.

### Testing Guidance Rule

- When user testing is needed, provide instructions that are specific enough to
  reproduce the intended case and observe the expected result.
- Include exact settings only when they matter (for example: ROI duration,
  overlap, center on/off, frame count target, zero padding).
- Tell the user what to look/listen for and what measurements or logs would be
  useful.
- Keep testing instructions detailed when the case is subtle, and short when the
  case is simple.

---

## Key Architecture Notes

- Refer to `map.md` for architecture summaries. This section stays empty so the map remains the single source of truth.
