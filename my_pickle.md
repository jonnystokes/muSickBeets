# Keyboard Debugger - Complete Detection Methods

This document lists ALL keyboard/mouse detection methods used in the tester program.

---

## Method 1: Event::KeyDown

**What it is**: FLTK event fired when a key is pressed down
**Crate**: fltk (fltk-rs crate)
**Code in program**: 
```rust
match ev {
    Event::KeyDown => { ... }
}
```
**Function to get key**: `app::event_key()`
**Detected in your test**:
- ControlL, ShiftL, AltL, MetaL (modifier keys)
- BackSpace, Escape, Enter
- 'a', '1' (printable chars)
- Raw values: `Key::from_i32(97)` for 'a', `Key::ControlL` etc.

---

## Method 2: Event::KeyUp

**What it is**: FLTK event fired when a key is released
**Crate**: fltk
**Code in program**:
```rust
match ev {
    Event::KeyUp => { ... }
}
```
**Function to get key**: `app::event_key()`
**Detected in your test**:
- ControlL, ShiftL, AltL, MetaL
- BackSpace, Escape, Enter
- Home, End, Up, Down, Left, Right (arrow keys)
- 'a', '1'
- Raw values: `Key::Home`, `Key::End`, `Key::Up`, etc.

---

## Method 3: Event::Shortcut

**What it is**: FLTK event fired when a key combination isn't handled by focused widget - bubbles to window
**Crate**: fltk
**Code in program**:
```rust
match ev {
    Event::Shortcut => { ... }
}
```
**Function to get key**: `app::event_key()`
**Detected in your test**:
- ControlL, ShiftL, AltL, MetaL
- BackSpace, Escape
- 'a', '1'
- Same as KeyDown but for unhandled shortcuts

---

## Method 4: app::event_key()

**What it is**: FLTK function to get the actual key code from the event
**Crate**: fltk (fltk::app module)
**Code in program**:
```rust
let key = app::event_key();
```
**Returns**: Key enum - e.g., `Key::from_i32(97)`, `Key::ControlL`, `Key::Escape`
**Detected in your test**: All keys via this function

---

## Method 5: app::event_text()

**What it is**: FLTK function to get the character text produced by the key
**Crate**: fltk
**Code in program**:
```rust
let text = app::event_text();
```
**Returns**: String - e.g., "a", "A", ""
**Detected in your test**: 
- 'a' → "a"
- '1' → "1" 
- Shift+a → "A"
- Modifiers → ""

---

## Method 6: app::event_state()

**What it is**: FLTK function to get modifier key state (Ctrl, Shift, Alt, Meta)
**Crate**: fltk
**Code in program**:
```rust
let state = app::event_state();
```
**Returns**: Shortcut enum with flags
**Detected in your test**:
- None (no modifiers)
- Ctrl
- Shift
- Alt
- Shift + Alt
- Ctrl + Shift

---

## Method 7: Key::from_char()

**What it is**: FLTK Key enum method to convert character to Key type
**Crate**: fltk (fltk::enums::Key)
**Code in program**:
```rust
fn key_matches_char(key: Key, ch: char) -> bool {
    key == Key::from_char(ch)
}
```
**Usage**: Comparing `app::event_key()` result with character
**Detected in your test**: Used to detect 'a', '1', ' ' (space)

---

## Method 8: window.handle() - Window-level handler

**What it is**: FLTK method to set event handler on the Window widget
**Crate**: fltk (widget handle method)
**Code in program**:
```rust
wind.handle(move |_w, ev| {
    // handle events here
})
```
**Purpose**: Catches all events at window level before widgets
**Detected in your test**: All keyboard and mouse events

---

## Method 9: widget.handle() - Widget-level handler

**What it is**: FLTK method to set event handler on specific widgets (input fields)
**Crate**: fltk
**Code in program**:
```rust
inp1.handle(|_w, ev| { ... });
inp2.handle(|_w, ev| { ... });
inp3.handle(|_w, ev| { ... });
```
**Purpose**: Catches events on specific input fields
**Detected in your test**: KeyDown events on FloatInput and Input widgets

---

## Method 10: set_visible_focus() / clear_visible_focus()

**What it is**: FLTK methods to control whether widget shows keyboard focus indicator
**Crate**: fltk
**Code in program**:
```rust
inp1.clear_visible_focus();
inp2.clear_visible_focus();
inp3.clear_visible_focus();
```
**Purpose**: Prevents input widgets from stealing keyboard focus, lets window capture keys
**Detected in your test**: Enabled window-level key capture

---

## Method 11: app::add_handler() - Global handler

**What it is**: FLTK function to add a global event handler that runs for all events
**Crate**: fltk (fltk::app module)
**Code in program**:
```rust
app::add_handler(move |ev| {
    // handle events
    false
});
```
**Purpose**: Catches events at application level
**Detected in your test**: KeyDown for Escape, Tab, Enter, BackSpace, Up, Down, Left, Right

---

## Method 12: Event::Paste

**What it is**: FLTK event fired when text is pasted (e.g., Ctrl+V)
**Crate**: fltk
**Code in program**:
```rust
match ev {
    Event::Paste => { ... }
}
```
**Function**: `app::event_text()` to get pasted content
**Detected in your test**: Not triggered in your test run

---

## Method 13: Modifier Combination Detection

**What it is**: Using event_state() to detect Ctrl+key, Shift+key, Alt+key combinations
**Crate**: fltk
**Code in program**:
```rust
if state.contains(Shortcut::Ctrl) { ... }
if state.contains(Shortcut::Shift) { ... }
if state.contains(Shortcut::Alt) { ... }
```
**Detected in your test**:
- Ctrl+A
- Shift+A → "A"
- Alt+A
- Ctrl+Shift+A
- Ctrl+Alt+A
- Alt+Shift+A

---

## Method 14: Shortcut Detection (key + modifier combos)

**What it is**: Detecting specific keyboard shortcuts
**Crate**: fltk
**Code in program**:
```rust
if state.contains(Shortcut::Ctrl) && key_matches_char(key, 'a') {
    // Ctrl+A detected
}
```
**Detected in your test**: Ctrl+C, Ctrl+V, Ctrl+X, Ctrl+S, Ctrl+Z all detected

---

## Method 15: Combined Detection (All methods together)

**What it is**: Using multiple methods simultaneously to maximize detection
**Crate**: fltk
**Code in program**: The main window handler uses Event::KeyDown, KeyUp, Shortcut, event_key(), event_text(), event_state() all in one handler
**Detected in your test**: All keyboard events caught by multiple methods

---

## ADDITIONAL MOUSE METHODS

## Method 16: Event::Push (Mouse Down)

**What it is**: FLTK event when mouse button is pressed
**Crate**: fltk
**Code in program**:
```rust
Event::Push => {
    let btn = app::event_button();
}
```
**Function**: `app::event_button()` returns button number
**Detected in your test**:
- btn=1 → Left mouse button
- btn=2 → Middle mouse button  
- btn=3 → Right mouse button

---

## Method 17: Event::Released (Mouse Up)

**What it is**: FLTK event when mouse button is released
**Crate**: fltk
**Code in program**:
```rust
Event::Released => {
    let btn = app::event_button();
}
```
**Detected in your test**: btn=1, btn=2, btn=3 (same as Push)

---

## Method 18: Event::MouseWheel

**What it is**: FLTK event for mouse scroll wheel
**Crate**: fltk
**Code in program**:
```rust
Event::MouseWheel => {
    let dy = app::event_dy();
    let dx = app::event_dx();
}
```
**Functions**: 
- `app::event_dy()` - vertical scroll
- `app::event_dx()` - horizontal scroll
**Detected in your test**:
- Scroll Up
- Scroll Down
- Scroll Left
- Scroll Right

---

## Summary Table

| Method | What It Detects | Working? |
|--------|-----------------|----------|
| 1. Event::KeyDown | Key press | ✅ |
| 2. Event::KeyUp | Key release | ✅ |
| 3. Event::Shortcut | Unhandled shortcuts | ✅ |
| 4. app::event_key() | Key code | ✅ |
| 5. app::event_text() | Character text | ✅ |
| 6. app::event_state() | Modifiers | ✅ |
| 7. Key::from_char() | Char to Key | ✅ |
| 8. window.handle() | Window-level | ✅ |
| 9. widget.handle() | Widget-level | ✅ |
| 10. clear_visible_focus() | Focus management | ✅ |
| 11. app::add_handler() | Global handler | ✅ |
| 12. Event::Paste | Paste events | ⚠️ Not tested |
| 13. Modifier combos | Ctrl/Shift/Alt+key | ✅ |
| 14. Shortcut detection | Specific combos | ✅ |
| 15. Combined | All methods | ✅ |
| 16. Event::Push | Mouse down | ✅ |
| 17. Event::Released | Mouse up | ✅ |
| 18. Event::MouseWheel | Scroll wheel | ✅ |
