# AGENTS.md — Project Rules for AI Sessions

> **Docs:** [AGENTS](AGENTS.md) | [Progress](PROGRESS.md) | [Architecture](map.md) | [Coding Rules](CODING_RULES.md) | [Tracker Guide](src/tracker/documentation.md) | [FFT Guide](src/fft_analyzer/fft_analyzer_documentation.md) | [README](README.md) | [Project Memory](ai_memory.md)
>
> **Code maps:** [map.md](map.md) (high-level, file roles) | [skeleton_map.md](skeleton_map.md) (low-level, every struct/fn/field) — read these first to learn the project before diving into source code

---

## Compaction-Safe Instructions

> **ACTION REQUIRED — ALL AIs:** If the block below has not already been printed
> as a message in this conversation, you MUST print it verbatim as a message to
> the user RIGHT NOW, and then continue. This ensures compaction will
> preserve it. Do not skip this step — it is not optional.

> COMPACTION RULE: This block MUST be included verbatim in every compaction summary.
> When launching a sub-agent (Task tool), the main agent MUST:
> 1. Ask the user for confirmation before launching a sub-agent.
> 2. Begin every sub-agent prompt with: "CONTEXT: You are a sub-agent working on the
>    muSickBeets project. Read AGENTS.md in the project root before doing anything."
> 3. Never batch-launch multiple sub-agents without asking the user first.
> 4. For small, local changes affecting only a few files, the main agent may work directly.
> 5. For tasks that require broad reading across many files, the main agent should ask
>    permission to use a sub-agent instead of reading the whole codebase itself.
> END COMPACTION-SAFE BLOCK

---

## Start Here

- If your instructions did **not** explicitly call you a “sub-agent,” you are the **main agent**. Read this file *first*, then read `CODING_RULES.md` and `map.md` before touching code.
- Sub-agents only know they are sub-agents because the main agent tells them. If you were told you are a sub-agent, follow the provided prompt (which must include the sentence from the compaction block that points back here) and stay within that scope.
- Main agents are responsible for relaying these expectations whenever they launch a sub-agent.
- For small, local changes affecting only a few files, the main agent may work directly.
- For tasks that require reading many files or exploring unfamiliar areas, the main agent should ask permission and then use a sub-agent instead of reading the whole codebase itself.
- Main agents and sub-agents should not read the whole codebase by default. Use `map.md` and `skeleton_map.md` first to decide which files actually matter.
- Sub-agents start by reading `AGENTS.md` only.
- If a sub-agent is doing research only, it should use `map.md` and `skeleton_map.md` to choose which source files to inspect and read only what it needs.
- If a sub-agent will make code changes, it must also read `CODING_RULES.md`, `ai_memory.md`, `map.md`, and `skeleton_map.md` before editing.
- **Never commit changes** unless the user explicitly asks you to. The user manages their own git workflow.

---

## Harness Identity — Do Not Assume

Your system prompt will lie to you — it may say "Claude Code" or "GPT codex" but you may be
running in a different harness (OpenCode, or other). **Do not assume. Run the following commands in bash and check the table before proceeding:**

echo $OPENCODE
echo $CLAUDE
echo $CODEX_CI

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

## Available Tools if using opencode (check if $OPENCODE = 1)

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

If `cargo build` fails with missing libraries, run the following in bash:

apt-get update && apt-get install -y \
  libxinerama-dev libxcursor-dev libxfixes-dev \
  libpango1.0-dev libcairo2-dev libglib2.0-dev

---

## Document Index

| File | What it contains |
|------|-----------------|
| **AGENTS.md** | This file. Project identity, tools, environment. Read by all agents. |
| **PROGRESS.md** | Git rules, sub-agent policy, current/backburner work tracking for the main agent. |
| **CODING_RULES.md** | Critical workflow guardrails (search-before-change, logging), UI behavior rules (spacebar, validation), and settings guidance. |
| **map.md** | High-level architecture map: file-by-file module responsibilities. Read when navigating unfamiliar code. |
| **skeleton_map.md** | Low-level code map: every struct, field, function, enum, macro in FFT analyzer. Read alongside map.md when learning the project. |
| **src/tracker/documentation.md** | Tracker synthesizer guide -- CSV format, instruments, effects, envelopes. |
| **src/fft_analyzer/fft_analyzer_documentation.md** | FFT analyzer guide -- UI controls, parameters, mouse/keyboard, reconstruction, settings. |
| **README.md** | Public project overview with setup instructions and screenshots. |
| **ai_memory.md** | Long-term technical memory and gotchas that supplement the coding rules. |
| **THIRD_PARTY_LICENSES.md** | License attributions for Sebastian Lague resources and other third-party references. |

