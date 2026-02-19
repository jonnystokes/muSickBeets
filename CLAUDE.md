# CLAUDE.md — Project Rules for AI Sessions

## Git Commit Rules

**Always commit with `git add .` to avoid missing files.**

The correct commit sequence is:

```bash
git add .
git commit -m "Your commit message here"
git push -u origin <branch-name>
```

**Why:** Using specific file names causes forgotten files (e.g., updating code but forgetting to commit a modified progress/notes file). `git add .` catches everything.

**Commit timing:** Update ALL files (code, progress notes, documentation) BEFORE committing. The very last action should be the commit+push so that all changes are available on the remote together.

**Always push to remote after updating progress or notes files.** If you update PROGRESS.md, ai_memory.md, or any documentation, that update must be included in the next commit and push — never leave documentation changes uncommitted.

## Text Field Validation Rules

All text input fields MUST be controlled — they may only accept valid numeric characters:

- **Float fields** (`FloatInput`): digits `0-9`, at most one `-` (at position 0 only), at most one `.`
- **Unsigned int fields** (`Input` used for counts/sizes): digits `0-9` only
- **No spaces ever.** Spacebar is intercepted at the window level and never reaches text fields.
- **No letters, symbols, or other characters.**

Validation uses `handle()` (not `set_callback()`) so it survives when functional callbacks are later attached to the same widget. See `src/fft_analyzer/validation.rs` for the implementation.

When adding new text input fields, always call:
- `attach_float_validation(&mut field)` for float fields
- `attach_uint_validation(&mut field)` for unsigned integer fields

## Spacebar Behavior

The spacebar is a **global shortcut** that triggers "Recompute + Rebuild" (FFT reanalysis and reconstruction).

- Spacebar is consumed at the window level (`setup_spacebar_handler` in `callbacks_nav.rs`)
- It must NEVER reach any widget: no space in text fields, no activating focused buttons/dropdowns
- Both `KeyDown` and `KeyUp` for space return `true` (consumed)
- `Shortcut` event for space also returns `true` (VNC/remote desktop compatibility)
- Only `KeyUp` triggers the actual recompute (to avoid double-fire)

## Transport Time Display

The transport bar shows two time values:
- **L (Local):** Time within the reconstructed audio buffer (0 to duration)
- **G (Global):** Absolute time in the full audio file (recon_start_time + local)

`recon_start_time` is set from the **actual first FFT frame's time** after filtering (not the user-typed Start value), ensuring global time precisely matches the spectrogram cursor position.
