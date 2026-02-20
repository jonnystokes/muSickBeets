# CLAUDE.md — Project Rules for AI Sessions

This file is the ONLY persistence between AI sessions. Future AIs will not have
memories from previous sessions. Follow these rules exactly.

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

## Spacebar Behavior — CRITICAL, READ CAREFULLY

The spacebar is a **global shortcut** that triggers "Recompute + Rebuild" (FFT reanalysis and reconstruction). It must ALWAYS work, regardless of which widget has focus.

### The Problem (Why This Is Hard)

FLTK processes keyboard events at the **widget level first**, then propagates to parents. A window-level `handle()` CANNOT reliably block space from reaching child widgets because:

1. The focused widget's internal C++ handler runs **before** our Rust `handle()` callback
2. Buttons activate on space, Choice dropdowns open on space, CheckButtons toggle on space
3. Returning `true` from `handle()` is supposed to skip default behavior, but **FLTK's internal widget handlers bypass this for keyboard events on buttons, choices, and other focusable widgets**

### The Solution (Three Layers)

Space blocking requires **all three** of these layers working together:

#### Layer 1: `clear_visible_focus()` — Prevents keyboard focus (PRIMARY DEFENSE)
Every interactive widget (buttons, choices, checkbuttons, sliders, scrollbars) has `clear_visible_focus()` called on it. This prevents the widget from receiving keyboard focus entirely. Without focus, space events never reach the widget. **This is the layer that actually works.**

#### Layer 2: Per-widget `handle()` via `block_space!` macro (BACKUP)
Every non-text interactive widget has a `handle()` callback that intercepts space:
- `KeyDown` / `Shortcut` → return `true` (consume, block)
- `KeyUp` → call `btn_rerun.do_callback()` then return `true` (trigger recompute)
This is a backup in case a widget somehow gains focus despite `clear_visible_focus()`.

#### Layer 3: Window-level `handle()` (FALLBACK)
`setup_spacebar_handler()` on the main window catches space when no widget consumed it (e.g., when nothing is focused). Same pattern: KeyDown/Shortcut consumed, KeyUp triggers recompute.

### Where The Code Lives

- **`callbacks_nav.rs` → `setup_spacebar_handler()`** — Window-level handler (Layer 3)
- **`callbacks_nav.rs` → `setup_spacebar_guards()`** — Per-widget guards (Layers 1+2). Contains the `block_space!` macro and `clear_visible_focus()` calls for ALL widgets
- **`validation.rs`** — Text input handlers include space blocking + recompute trigger
- **`callbacks_ui.rs`** — `scrub_slider` and `gradient_preview` handlers include space blocking inline

### Exception

The **top-level menu bar** (File, Analyze, Display) is NOT guarded. It must remain accessible via keyboard.

### When Adding New Widgets

**For buttons, choices, checkbuttons, sliders, scrollbars:**
1. Add `block_space!(widgets.my_new_widget.clone(), btn_rerun);` in `setup_spacebar_guards()`
2. Add `widgets.my_new_widget.clone().clear_visible_focus();` right after
3. Both lines are required. `clear_visible_focus()` is the one that actually prevents space from opening/activating the widget.

**For text input fields (`FloatInput` or `Input`):**
1. In `layout.rs`: call `attach_float_validation()` or `attach_uint_validation()` as usual
2. In `setup_spacebar_guards()`: call `attach_float_validation_with_recompute()` or `attach_uint_validation_with_recompute()` — this REPLACES the plain handler with one that also triggers `btn_rerun.do_callback()` on space KeyUp

**For widgets with custom `handle()` callbacks (like scrub_slider, gradient_preview):**
Add this at the top of the existing handle closure:
```rust
if fltk::app::event_key() == fltk::enums::Key::from_char(' ') {
    return match ev {
        fltk::enums::Event::KeyDown | fltk::enums::Event::Shortcut => true,
        fltk::enums::Event::KeyUp => { btn_rerun_clone.do_callback(); true }
        _ => false,
    };
}
```

### Call Order Matters

`setup_spacebar_guards()` MUST be called LAST in the callback setup chain (after all other `setup_*` functions) because it sets `handle()` on widgets — any later `handle()` call would overwrite it.

## Text Field Validation Rules

All text input fields MUST be controlled — they may only accept valid numeric characters:

- **Float fields** (`FloatInput`): digits `0-9`, at most one `-` (at position 0 only), at most one `.`
- **Unsigned int fields** (`Input` used for counts/sizes): digits `0-9` only
- **No spaces ever.** Spacebar is blocked by the validation handler.
- **No letters, symbols, or other characters.**

### Implementation Details

Validation uses `handle()` (NOT `set_callback()`) so it survives when functional callbacks are later attached via `set_callback()` on the same widget. In FLTK-rs, `handle()` and `set_callback()` are independent — setting one does not overwrite the other.

However, calling `handle()` twice on the same widget DOES overwrite the first handler. This is why `setup_spacebar_guards()` uses `attach_float_validation_with_recompute()` to REPLACE the plain validation handler — the new handler includes both validation AND space blocking with recompute trigger.

**When adding new text input fields:**
1. In `layout.rs`: call `attach_float_validation(&mut field)` or `attach_uint_validation(&mut field)`
2. In `setup_spacebar_guards()` (callbacks_nav.rs): call `attach_float_validation_with_recompute()` or `attach_uint_validation_with_recompute()` to replace with the recompute-aware version

See `src/fft_analyzer/validation.rs` for all four functions.

## Transport Time Display

The transport bar shows two time values:
- **L (Local):** Time within the reconstructed audio buffer (0 to duration)
- **G (Global):** Absolute time in the full audio file (recon_start_time + local)

`recon_start_time` is set from the **actual first FFT frame's time** after filtering (not the user-typed Start value), ensuring global time precisely matches the spectrogram cursor position.

## Lock to Active

When the "Lock to Active" checkbox is enabled, after reconstruction completes the viewport auto-snaps to the active processing range — **both time AND frequency**. This uses the same logic as the Home button but with a 0.5-second delay (via `app::add_timeout3`) to let the UI finish updating.

The delay and Home-equivalent logic lives in the `ReconstructionComplete` handler in `main_fft.rs`.

## Settings File

`settings.ini` (or legacy `muSickBeets.ini`) is auto-generated at runtime. It is in `.gitignore` and must NEVER be committed.
