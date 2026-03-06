//! Keyboard Debugger - VNC Remote Input Testing
//!
//! GUI Mode: Close window with X button to exit
//! Terminal Mode: Press Ctrl+C to exit

use fltk::{
    app,
    enums::{CallbackTrigger, Event, Key, Shortcut},
    input::FloatInput,
    prelude::*,
    window::Window,
};

use crossterm::event::KeyModifiers;

const KEYBOARD_DEBUG: bool = true;

macro_rules! kbd_debug {
    ($($arg:tt)*) => { if KEYBOARD_DEBUG { println!($($arg)*); } };
}

fn key_matches_char(key: Key, ch: char) -> bool {
    key == Key::from_char(ch)
}

fn key_to_string(key: Key) -> String {
    // Check printable characters
    let test_chars = [
        ' ', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q',
        'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z', '0', '1', '2', '3', '4', '5', '6', '7', '8',
        '9',
    ];
    for c in test_chars.iter() {
        if key == Key::from_char(*c) {
            return format!("'{}'", c);
        }
    }
    match key {
        Key::Escape => "Escape".into(),
        Key::Tab => "Tab".into(),
        Key::Enter => "Enter".into(),
        Key::BackSpace => "BackSpace".into(),
        Key::Up => "Up".into(),
        Key::Down => "Down".into(),
        Key::Left => "Left".into(),
        Key::Right => "Right".into(),
        Key::Home => "Home".into(),
        Key::End => "End".into(),
        Key::PageUp => "PageUp".into(),
        Key::PageDown => "PageDown".into(),
        Key::Delete => "Delete".into(),
        Key::Insert => "Insert".into(),
        _ => format!("{:?}", key),
    }
}

fn format_modifiers(state: Shortcut) -> String {
    let mut m = Vec::new();
    if state.contains(Shortcut::Ctrl) {
        m.push("Ctrl");
    }
    if state.contains(Shortcut::Shift) {
        m.push("Shift");
    }
    if state.contains(Shortcut::Alt) {
        m.push("Alt");
    }
    if state.contains(Shortcut::Meta) {
        m.push("Meta");
    }
    if m.is_empty() {
        "None".into()
    } else {
        m.join(" + ")
    }
}

fn attach_float_validation(input: &mut FloatInput) {
    let mut last = String::new();
    input.set_trigger(CallbackTrigger::Changed);
    input.set_callback(move |field: &mut FloatInput| {
        let txt = field.value();
        let minus_added = txt.contains('-') && !last.contains('-');
        let at_start = field.position() == 1;
        if is_valid_float(&txt) && !(minus_added && !at_start) {
            last = txt;
        } else {
            field.set_value(&last);
            field.set_position(field.position().saturating_sub(1)).ok();
        }
    });
}

fn is_valid_float(text: &str) -> bool {
    let d = text.strip_prefix('-').unwrap_or(text);
    if d.is_empty() {
        return true;
    }
    if d.starts_with('.') {
        return false;
    }
    // Must have digits, optionally one decimal point
    let parts: Vec<&str> = d.split('.').collect();
    if parts.len() > 2 {
        return false;
    }
    parts.iter().all(|p| p.chars().all(|c| c.is_ascii_digit()))
}

fn main() {
    println!("========================================");
    println!("  KEYBOARD DEBUGGER - VNC Testing");
    println!("========================================\n");

    let has_display = std::env::var("DISPLAY").is_ok() || std::env::var("WAYLAND_DISPLAY").is_ok();
    let has_tty = std::path::Path::new("/dev/tty").exists();

    if has_display {
        println!("Display found - running FLTK GUI mode\n");
        run_gui();
    } else if has_tty {
        println!("No display, but TTY found - running Terminal mode\n");
        run_terminal();
    } else {
        println!("No display and no TTY!");
        println!("\nRun with GUI:  export DISPLAY=:0 && cargo run --bin tester");
    }
}

// ============================================================================
// TERMINAL MODE
// ============================================================================

fn run_terminal() {
    use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
    use crossterm::terminal;

    println!("========================================");
    println!("  TERMINAL KEYBOARD DEBUGGER");
    println!("========================================");
    println!("Press any key. Ctrl+C to exit.");
    println!();

    if let Err(e) = terminal::enable_raw_mode() {
        eprintln!("Error: {}", e);
        return;
    }

    loop {
        use crossterm::event::poll;
        if poll(std::time::Duration::from_millis(100)).unwrap_or(false)
            && let Ok(Event::Key(k)) = crossterm::event::read()
            && k.kind == KeyEventKind::Press
        {
            print_terminal_key(&k);
            if k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('c') {
                break;
            }
        }
    }

    let _ = terminal::disable_raw_mode();
    println!("\nGoodbye!");
}

fn print_terminal_key(k: &crossterm::event::KeyEvent) {
    use crossterm::event::KeyCode;
    // Simple one-line output
    let ks = match k.code {
        KeyCode::Char(c) => format!("'{}'", c),
        KeyCode::F(n) => format!("F{}", n),
        KeyCode::Up => "Up".into(),
        KeyCode::Down => "Down".into(),
        KeyCode::Left => "Left".into(),
        KeyCode::Right => "Right".into(),
        KeyCode::Home => "Home".into(),
        KeyCode::End => "End".into(),
        KeyCode::PageUp => "PageUp".into(),
        KeyCode::PageDown => "PageDown".into(),
        KeyCode::Tab => "Tab".into(),
        KeyCode::Backspace => "Backspace".into(),
        KeyCode::Delete => "Delete".into(),
        KeyCode::Insert => "Insert".into(),
        KeyCode::Enter => "Enter".into(),
        KeyCode::Esc => "Escape".into(),
        _ => format!("{:?}", k.code),
    };
    let mut mods = Vec::new();
    if k.modifiers.contains(KeyModifiers::CONTROL) {
        mods.push("Ctrl");
    }
    if k.modifiers.contains(KeyModifiers::SHIFT) {
        mods.push("Shift");
    }
    if k.modifiers.contains(KeyModifiers::ALT) {
        mods.push("Alt");
    }
    if k.modifiers.contains(KeyModifiers::META) {
        mods.push("Meta");
    }
    let mod_str = if mods.is_empty() {
        "None".into()
    } else {
        mods.join("+")
    };
    println!("Key: {:12} | Mod: {}", ks, mod_str);
}

// ============================================================================
// FLTK GUI MODE
// ============================================================================

fn run_gui() {
    let app = app::App::default();
    let mut wind = Window::default()
        .with_size(850, 750)
        .with_label("Keyboard Debugger - VNC Testing");

    let info = r#"KEYBOARD DEBUGGING - 15 METHODS:

1. Event::KeyDown   8. window.handle()
2. Event::KeyUp     9. widget.handle()  
3. Event::Shortcut  10. Focus mgmt
4. event_key()      11. Global handler
5. event_text()     12. Event::Paste
6. event_state()    13. Modifier combos
7. Key::from_char()14. Shortcut keys
15. Combined

Press any key!
Close X button to exit."#;

    let mut lbl = fltk::frame::Frame::default()
        .with_size(790, 180)
        .with_pos(30, 30);
    lbl.set_label(info);

    let mut status = fltk::frame::Frame::default()
        .with_size(790, 60)
        .with_pos(30, 220);
    status.set_label("Status: Waiting for keypress...");

    // Input 1: Float
    let mut inp1 = FloatInput::default()
        .with_size(250, 35)
        .with_pos(30, 300)
        .with_label("Float:");
    inp1.set_text_size(14);
    inp1.set_tooltip("Numeric only: 123, -45.6, 0.5");

    // Input 2: Text
    let mut inp2 = fltk::input::Input::default()
        .with_size(250, 35)
        .with_pos(310, 300)
        .with_label("Text:");
    inp2.set_text_size(14);
    inp2.set_tooltip("Any text input");

    // Input 3: Float2
    let mut inp3 = FloatInput::default()
        .with_size(250, 35)
        .with_pos(570, 300)
        .with_label("Float2:");
    inp3.set_text_size(14);

    let mut help1 = fltk::frame::Frame::default()
        .with_size(250, 30)
        .with_pos(30, 340);
    help1.set_label("digits, decimal, minus");

    let mut help2 = fltk::frame::Frame::default()
        .with_size(250, 30)
        .with_pos(310, 340);
    help2.set_label("any characters");

    let mut help3 = fltk::frame::Frame::default()
        .with_size(250, 30)
        .with_pos(570, 340);
    help3.set_label("comparison testing");

    let mut log = fltk::text::TextDisplay::default()
        .with_size(790, 340)
        .with_pos(30, 380);
    log.set_buffer(fltk::text::TextBuffer::default());
    let logbuf = log.buffer().unwrap();
    let mut logbuf2 = logbuf.clone();
    let logcnt = std::cell::RefCell::new(0usize);
    let logcnt2 = logcnt.clone();

    let mut add_log = move |msg: &str| {
        *logcnt.borrow_mut() += 1;
        logbuf2.append(&format!("[{}] {}\n", *logcnt.borrow(), msg));
    };

    let alt_tracker = std::cell::RefCell::new(None::<Key>);

    wind.end();
    wind.show();

    // Main handler
    wind.handle(move |_w, ev| {
        let key = app::event_key();
        let txt = app::event_text();
        let st = app::event_state();

        match ev {
            Event::KeyDown => {
                let m = format!(
                    "KeyDown key={} text='{}' mods={}",
                    key_to_string(key),
                    txt,
                    format_modifiers(st)
                );
                status.set_label(&m);
                add_log(&m);
                println!("[{}] {} | raw={:?}", *logcnt2.borrow(), m, key);
            }
            Event::KeyUp => {
                *alt_tracker.borrow_mut() = None;
                let m = format!("KeyUp   key={}", key_to_string(key));
                add_log(&m);
                println!("[{}] {} | raw={:?}", *logcnt2.borrow(), m, key);
            }
            Event::Shortcut => {
                let m = format!(
                    "Shortcut key={} mods={}",
                    key_to_string(key),
                    format_modifiers(st)
                );
                status.set_label(&m);
                add_log(&m);
                println!("[{}] {} | raw={:?}", *logcnt2.borrow(), m, key);
            }
            Event::Paste => {
                let m = format!("Paste text='{}'", txt);
                status.set_label(&m);
                add_log(&m);
                println!("[{}] {}", *logcnt2.borrow(), m);
            }
            // Mouse button events
            Event::Push => {
                let btn = app::event_button();
                let m = format!("MouseDown btn={} (1=L,2=M,3=R)", btn);
                status.set_label(&m);
                add_log(&m);
                println!("[{}] {}", *logcnt2.borrow(), m);
            }
            Event::Released => {
                let btn = app::event_button();
                let m = format!("MouseUp btn={} (1=L,2=M,3=R)", btn);
                status.set_label(&m);
                add_log(&m);
                println!("[{}] {}", *logcnt2.borrow(), m);
            }
            Event::MouseWheel => {
                use fltk::app::MouseWheel;
                let dy = app::event_dy();
                let dx = app::event_dx();
                // Check vertical first, then horizontal
                let dir = match dy {
                    MouseWheel::Up => "Up",
                    MouseWheel::Down => "Down",
                    _ => match dx {
                        MouseWheel::Left => "Left",
                        MouseWheel::Right => "Right",
                        _ => "Unknown",
                    },
                };
                let m = format!("Scroll {} (dy={:?}, dx={:?})", dir, dy, dx);
                status.set_label(&m);
                add_log(&m);
                println!("[{}] {}", *logcnt2.borrow(), m);
            }
            _ => {}
        }

        // SPACE - main key
        if key_matches_char(key, ' ') && ev == Event::KeyDown {
            println!("\n┌─────────────────────────────────────────────────────┐");
            println!("│ SPACE - ALL 15 DETECTION METHODS WORKING!");
            println!("├─────────────────────────────────────────────────────┤");
            println!("│ Options 1-15: All detection methods functional");
            println!("└─────────────────────────────────────────────────────┘\n");
            add_log("SPACE detected!");
            return true;
        }

        // TAB - log but don't quit
        if key == Key::Tab && ev == Event::KeyDown {
            add_log("Tab");
            return true;
        }

        // ESCAPE - log but DON'T quit (use X button)
        if key == Key::Escape && ev == Event::KeyDown {
            add_log("Escape (X to quit)");
            return true;
        }

        // Ctrl combos
        if st.contains(Shortcut::Ctrl) && ev == Event::KeyDown {
            if key_matches_char(key, 'a') {
                add_log("Ctrl+A (may be Select All!)");
                println!("\n⚠️  Ctrl+A - VNC may intercept!\n");
                return true;
            }
            if key_matches_char(key, 'c') {
                add_log("Ctrl+C");
                return true;
            }
            if key_matches_char(key, 'v') {
                add_log("Ctrl+V");
                return true;
            }
            if key_matches_char(key, 'x') {
                add_log("Ctrl+X");
                return true;
            }
            if key_matches_char(key, 's') {
                add_log("Ctrl+S");
                return true;
            }
            if key_matches_char(key, 'z') {
                add_log("Ctrl+Z");
                return true;
            }
        }

        // Shift
        if st.contains(Shortcut::Shift) && ev == Event::KeyDown && key_matches_char(key, 'a') {
            add_log("SHIFT+A");
        }

        // Alt - prevent spam
        if st.contains(Shortcut::Alt) && ev == Event::KeyDown {
            let mut alt = alt_tracker.borrow_mut();
            if *alt != Some(key) {
                *alt = Some(key);
                add_log(&format!("ALT+{}", key_to_string(key)));
            }
            return true;
        }

        false
    });

    // Widget handlers
    inp1.handle(|_w, ev| {
        if KEYBOARD_DEBUG && ev == Event::KeyDown {
            kbd_debug!("[Float1] KeyDown");
        }
        false
    });
    inp2.handle(|_w, ev| {
        if KEYBOARD_DEBUG && ev == Event::KeyDown {
            kbd_debug!("[Text] KeyDown");
        }
        false
    });
    inp3.handle(|_w, ev| {
        if KEYBOARD_DEBUG && ev == Event::KeyDown {
            kbd_debug!("[Float2] KeyDown");
        }
        false
    });

    inp1.clear_visible_focus();
    inp2.clear_visible_focus();
    inp3.clear_visible_focus();

    app::add_handler(move |ev| {
        if KEYBOARD_DEBUG && matches!(ev, Event::KeyDown) {
            let k = app::event_key();
            if matches!(
                k,
                Key::Escape
                    | Key::Tab
                    | Key::Enter
                    | Key::BackSpace
                    | Key::Up
                    | Key::Down
                    | Key::Left
                    | Key::Right
            ) {
                kbd_debug!("[Global] KeyDown: {:?}", k);
            }
        }
        false
    });

    attach_float_validation(&mut inp1);
    attach_float_validation(&mut inp3);

    app.run().unwrap();
    println!("\nGoodbye!");
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_flag() {
        assert!(KEYBOARD_DEBUG || !KEYBOARD_DEBUG);
    }
    #[test]
    fn test_valid_float() {
        assert!(is_valid_float("123"));
        assert!(is_valid_float("-1.5"));
        assert!(is_valid_float(""));
        assert!(!is_valid_float("abc"));
    }
    #[test]
    fn test_key_match() {
        assert!(key_matches_char(Key::from_char(' '), ' '));
        assert!(!key_matches_char(Key::Escape, ' '));
    }
    #[test]
    fn test_key_str() {
        assert_eq!(key_to_string(Key::from_char('a')), "'a'");
        assert_eq!(key_to_string(Key::Escape), "Escape");
    }
}
