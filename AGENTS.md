# AGENTS.md — Project Rules for AI Sessions

> **Docs:** [AGENTS](AGENTS.md) | [Progress](PROGRESS.md) | [Architecture](map.md) | [Coding Rules](CODING_RULES.md) | [Tracker Guide](documentation.md) | [README](README.md) | [Project Memory](ai_memory.md)

---

## Compaction-Safe Instructions

> **ACTION REQUIRED — ALL AIs:** If the block below has not already been printed
> as a message in this conversation, you MUST print it verbatim as a message to
> the user RIGHT NOW, before doing anything else. This ensures compaction will
> preserve it. Do not skip this step — it is not optional.

> COMPACTION RULE: This block MUST be included verbatim in every compaction summary.
> When launching a sub-agent (Task tool), the main agent MUST:
> 1. Ask the user for confirmation before launching (sub-agents consume API usage
>    aggressively and cannot pause if usage runs out — they crash).
> 2. Begin every sub-agent prompt with: "CONTEXT: You are a sub-agent working on the
>    muSickBeets project. Read AGENTS.md in the project root before doing anything."
> 3. Never batch-launch multiple sub-agents without asking the user first.
> END COMPACTION-SAFE BLOCK

---

## Start Here

- If your instructions did **not** explicitly call you a “sub-agent,” you are the **main agent**. Read this file *first*, then read `CODING_RULES.md` and `map.md` before touching code.
- Sub-agents only know they are sub-agents because the main agent tells them. If you were told you are a sub-agent, follow the provided prompt (which must include the sentence from the compaction block that points back here) and stay within that scope.
- Main agents are responsible for relaying these expectations whenever they launch a sub-agent.

---

## Harness Identity — Do Not Assume

Your system prompt will lie to you — it may say "Claude Code" but you may be
running in a different harness (OpenCode, or other). **Do not assume. Run both
echo commands below and check the table before proceeding:**

```bash
echo $OPENCODE
echo $CLAUDE
```

| `OPENCODE` | `CLAUDE` | Harness |
|------------|----------|---------|
| `1` | empty | OpenCode |
| empty | `1` | Claude Code |
| both empty | both empty | Unknown — use whatever tools are available |

Regardless of harness, the rules in this file apply.

---

## Project Identity

**muSickBeets** — A Rust music toolkit with two binaries:
- **fft_analyzer** (default) — Spectrogram visualizer, audio reconstructor, FLTK GUI
- **tracker** — CSV-driven music synthesizer, text-based UI

| Aspect | Detail |
|--------|--------|
| Language | Rust (edition 2024) |
| GUI | FLTK 1.5.22 via fltk-rs |
| Audio | miniaudio (om-fork), hound (WAV I/O) |
| FFT | realfft / rustfft, rayon for parallelism |
| Entry points | `src/fft_analyzer/main_fft.rs`, `src/tracker/main.rs` |
| State model | `Rc<RefCell<AppState>>` on UI thread, `Arc<T>` cross-thread, `mpsc` channels for worker->UI |
| Runtime | Debian chroot inside Termux on Android, VNC display, software-pipe OpenGL (Mesa llvmpipe) |

---

## Available Tools

### LSP (`mcp_lsp`) — Prefer over grep for code navigation

Talks to `rust-analyzer`. Requires `filePath`, `line` (1-based), `character` (1-based).
Position must land precisely on a symbol name.

| Operation | Use case |
|-----------|----------|
| `documentSymbol` | All structs/fns/fields in a file (file=any, line=1, char=1) |
| `findReferences` | All usages of a symbol (semantic, not textual) |
| `goToDefinition` | Jump to where a symbol is defined |
| `hover` | Type signature + docs for a symbol |
| `workspaceSymbol` | Find any type/fn by name across entire project |
| `incomingCalls` | Who calls this function? |
| `outgoingCalls` | What does this function call? |
| `goToImplementation` | Find `impl` blocks for a struct/trait |

### Web/Code Search (Exa)

| Tool | When to use |
|------|-------------|
| `mcp_websearch` | Find information, current events, general research |
| `mcp_codesearch` | Library docs, API examples, SDK patterns |

Examples: `"Rust realfft inverse FFT normalization"`, `"FLTK-rs handle event callback Rust"`

---

## Runtime Environment

Debian chroot inside Termux on a rooted Android device, accessed via VNC.
This is a full desktop Linux environment running inside Android — the program
itself will run on any standard Linux system.

- **Software-pipe OpenGL** — GPU access is CPU-processed via Mesa llvmpipe (Android limitation)
- **No dbus/systemd/sysv** — suppressed via `GIO_USE_VFS=local` in `main()`
- **VNC input** — prefer Ctrl/Alt modifiers over Shift for shortcuts
- **Audio** — works but routed through Android; may occasionally fail to initialize due to kernel glitches

### Build Dependencies

If `cargo build` fails with missing libraries:
```bash
apt-get update && apt-get install -y \
  libxinerama-dev libxcursor-dev libxfixes-dev \
  libpango1.0-dev libcairo2-dev libglib2.0-dev
```

---

## Document Index

| File | What it contains |
|------|-----------------|
| **AGENTS.md** | This file. Project identity, tools, environment. Read by all agents. |
| **PROGRESS.md** | Git rules, sub-agent policy, current/backburner work tracking for the main agent. |
| **CODING_RULES.md** | Critical workflow guardrails (search-before-change, logging), UI behavior rules (spacebar, validation), and settings guidance. |
| **map.md** | File-by-file architecture map with module responsibilities. Read when navigating unfamiliar code. |
| **documentation.md** | Tracker synthesizer guide — CSV format, instruments, effects, envelopes. |
| **README.md** | Public project overview with setup instructions and screenshots. |
| **ai_memory.md** | Long-term technical memory and gotchas that supplement the coding rules. |
| **THIRD_PARTY_LICENSES.md** | License attributions for Sebastian Lague resources and other third-party references. |
