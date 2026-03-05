# Coding Rules

> **Docs:** [AGENTS](AGENTS.md) | [Progress](PROGRESS.md) | [Issues](CATEGORIZED_ISSUES.md) | [Architecture](map.md) | [Coding Rules](CODING_RULES.md) | [History](HISTORY.md) | [Tracker Guide](documentation.md) | [README](README.md)

Read this file before writing or modifying code in the muSickBeets project.

---

## Spacebar Behavior — CRITICAL

Spacebar is a **global shortcut** for "Recompute + Rebuild" (FFT reanalysis and
reconstruction). It must work regardless of which widget has focus. Three defense
layers are required, and all three must be in place:

| Layer | Mechanism | Where | Role |
|-------|-----------|-------|------|
| 1 (PRIMARY) | `clear_visible_focus()` | `setup_spacebar_guards()` in `callbacks_nav.rs` | Prevents widgets from receiving keyboard focus |
| 2 (BACKUP) | `block_space!` macro per-widget `handle()` | `setup_spacebar_guards()` in `callbacks_nav.rs` | Intercepts space if widget somehow gets focus |
| 3 (FALLBACK) | Window-level `handle()` | `setup_spacebar_handler()` in `callbacks_nav.rs` | Catches space when nothing else does |

**Why three layers:** FLTK processes keyboard events at the widget level first. A
window-level `handle()` alone CANNOT reliably block space because the focused widget's
internal C++ handler runs before our Rust callback. Buttons activate on space, Choice
dropdowns open, CheckButtons toggle — all before `handle()` sees the event.

**Exception:** The top-level menu bar (File, Analyze, Display) is NOT guarded.

`setup_spacebar_guards()` **MUST be called LAST** in the callback setup chain — it sets
`handle()` on widgets, which would be overwritten by any later `handle()` call.

### Adding New Widgets — Spacebar Checklist

**Buttons, choices, checkbuttons, sliders, scrollbars:**
1. Add `block_space!(widgets.my_widget.clone(), btn_rerun);` in `setup_spacebar_guards()`
2. Add `widgets.my_widget.clone().clear_visible_focus();` right after
3. Both lines required — `clear_visible_focus()` is the one that actually works.

**Text input fields (`FloatInput` / `Input`):**
1. In `layout.rs`: call `attach_float_validation()` or `attach_uint_validation()`
2. In `setup_spacebar_guards()`: call `attach_float_validation_with_recompute()` or
   `attach_uint_validation_with_recompute()` — replaces plain handler with recompute-aware version
3. In callback: use `set_trigger(CallbackTrigger::Changed)` — **never EnterKey**
4. In callback body first line: `if inp.value().contains(' ') { inp.set_value(&inp.value().replace(' ', "")); return; }`

**Widgets with custom `handle()` callbacks (like scrub_slider, gradient_preview):**
Add at the top of the existing handle closure:
```rust
if fltk::app::event_key() == fltk::enums::Key::from_char(' ') {
    return match ev {
        fltk::enums::Event::KeyDown | fltk::enums::Event::Shortcut => true,
        fltk::enums::Event::KeyUp => { btn_rerun_clone.do_callback(); true }
        _ => false,
    };
}
```

### Where The Code Lives

- **`callbacks_nav.rs` → `setup_spacebar_handler()`** — Window-level handler (Layer 3)
- **`callbacks_nav.rs` → `setup_spacebar_guards()`** — Per-widget guards (Layers 1+2), `block_space!` macro, `clear_visible_focus()` calls
- **`validation.rs`** — Text input handlers include space blocking + recompute trigger
- **`callbacks_ui.rs`** — `scrub_slider` and `gradient_preview` handlers include space blocking inline

---

## Text Field Validation Rules

All text inputs are controlled — only valid numeric characters allowed:
- **Float fields** (`FloatInput`): digits `0-9`, at most one `-` (position 0 only), at most one `.`
- **Unsigned int fields** (`Input`): digits `0-9` only
- **No spaces ever.** No letters, symbols, or other characters.

**ALL text fields MUST use `CallbackTrigger::Changed`.** Never `EnterKey` or
`EnterKeyAlways` — these break the validation/spacebar system by disrupting FLTK's
internal event processing. `EnterKey` causes Enter to select-all and disrupts the
`handle()` event flow, allowing invalid characters to bypass validation.

**If you want "apply on Enter" behavior:** Keep `CallbackTrigger::Changed` and add
Enter-key detection inside the `handle()` callback, NOT by changing the trigger.

### Implementation Details

Validation uses `handle()` (NOT `set_callback()`) so it survives when functional
callbacks are later attached. In FLTK-rs, `handle()` and `set_callback()` are
independent — setting one does not overwrite the other. But calling `handle()` twice
on the same widget DOES overwrite the first handler.

This is why `setup_spacebar_guards()` uses `attach_float_validation_with_recompute()`
to REPLACE the plain validation handler — the new handler includes both validation AND
space blocking with recompute trigger.

Any text field with live `Changed` callbacks must sanitize spaces in the callback
itself (`replace(' ', "")`) before numeric parsing.

**When adding new text input fields:**
1. In `layout.rs`: call `attach_float_validation(&mut field)` or `attach_uint_validation(&mut field)`
2. In `setup_spacebar_guards()` (callbacks_nav.rs): call `attach_float_validation_with_recompute()` or `attach_uint_validation_with_recompute()`
3. In the callback setup: use `set_trigger(CallbackTrigger::Changed)` — **never EnterKey**
4. In the callback body: first line must be the space-stripping defense

See `src/fft_analyzer/validation.rs` for all four validation functions.

---

## Transport Time Display

The transport bar shows two time values:
- **L (Local):** Time within the reconstructed audio buffer (0 to duration)
- **G (Global):** Absolute time in the full audio file (`recon_start_time` + local)

`recon_start_time` is set from the **actual first FFT frame's time** after filtering
(not the user-typed Start value), ensuring global time precisely matches the
spectrogram cursor position.

---

## Lock to Active

When the "Lock to Active" checkbox is enabled, after reconstruction completes the
viewport auto-snaps to the active processing range — **both time AND frequency**. This
uses the same logic as the Home button but with a 0.5-second delay (via
`app::add_timeout3`) to let the UI finish updating.

The delay and Home-equivalent logic lives in the `ReconstructionComplete` handler in
`main_fft.rs`.

---

## Settings File

`settings.ini` (or legacy `muSickBeets.ini`) is auto-generated at runtime. It is in
`.gitignore` and must **NEVER be committed**.
