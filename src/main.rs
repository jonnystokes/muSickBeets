
use fltk::{app, enums::{CallbackTrigger, Event, Key}, input::FloatInput, prelude::*, window::Window};

// ── Reusable: attach to any FloatInput to enforce valid decimal number entry ──
// Rules: digits only, one optional leading minus, one optional decimal point,
// decimal must follow a digit (not first char, not after minus).
fn attach_float_validation(input: &mut FloatInput) {
    let mut last_valid_text = String::new();
    input.set_trigger(CallbackTrigger::Changed);
    input.set_callback(move |field| {
        let current_text = field.value();
        let minus_just_added = current_text.contains('-') && !last_valid_text.contains('-');
        let typed_at_start = field.position() == 1;

        if is_valid_float_input(&current_text) && !(minus_just_added && !typed_at_start) {
            last_valid_text = current_text;
        } else {
            let restore_position = field.position().saturating_sub(1);
            field.set_value(&last_valid_text);
            field.set_position(restore_position).ok();
        }
    });
}

fn is_valid_float_input(text: &str) -> bool {
    let digits = text.strip_prefix('-').unwrap_or(text);
    if digits.is_empty() { return true; }
    if digits.starts_with('.') { return false; }
    let parts: Vec<&str> = digits.split('.').collect();
    parts.len() <= 2 && parts.iter().all(|p| p.is_empty() || p.chars().all(|c| c.is_ascii_digit()))
}

// ── Example usage ─────────────────────────────────────────────────────────────
fn main() {
    let app = app::App::default();
    let mut wind = Window::default().with_size(300, 80).with_label("Number Entry");

    let mut input = FloatInput::default().with_size(200, 30).with_pos(50, 25).with_label("Value:");
    input.set_text_size(16);
    attach_float_validation(&mut input);

    wind.end();
    wind.show();

    wind.handle(move |_, event| {
        if event == Event::KeyUp && app::event_key() == Key::from_char(' ') {
            println!("Space bar press detected");
        }
        false
    });

    app.run().unwrap();
}

