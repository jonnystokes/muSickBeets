use fltk::{
    enums::CallbackTrigger,
    input::{Input, FloatInput},
    prelude::*,
};

// ─── Float/Int Input Validation ──────────────────────────────────────────────
//
// Revert-based validation: let the character enter, then validate and revert
// if invalid. This works on VNC/Termux/remote desktop where keystroke blocking
// doesn't work because input arrives as Shortcut/Paste events.

pub fn is_valid_float_input(text: &str) -> bool {
    let digits = text.strip_prefix('-').unwrap_or(text);
    if digits.is_empty() { return true; }
    if digits.starts_with('.') { return false; }
    let parts: Vec<&str> = digits.split('.').collect();
    parts.len() <= 2 && parts.iter().all(|p| p.is_empty() || p.chars().all(|c| c.is_ascii_digit()))
}

pub fn is_valid_uint_input(text: &str) -> bool {
    text.is_empty() || text.chars().all(|c| c.is_ascii_digit())
}

pub fn attach_float_validation(input: &mut FloatInput) {
    let mut last_valid = String::new();
    input.set_trigger(CallbackTrigger::Changed);
    input.set_callback(move |field| {
        let current = field.value();
        let minus_just_added = current.contains('-') && !last_valid.contains('-');
        let typed_at_start = field.position() == 1;
        if is_valid_float_input(&current) && !(minus_just_added && !typed_at_start) {
            last_valid = current;
        } else {
            let restore = field.position().saturating_sub(1);
            field.set_value(&last_valid);
            field.set_position(restore).ok();
        }
    });
}

pub fn attach_uint_validation(input: &mut Input) {
    let mut last_valid = String::new();
    input.set_trigger(CallbackTrigger::Changed);
    input.set_callback(move |field| {
        let current = field.value();
        if is_valid_uint_input(&current) {
            last_valid = current;
        } else {
            let restore = field.position().saturating_sub(1);
            field.set_value(&last_valid);
            field.set_position(restore).ok();
        }
    });
}

// Helper: parse a field value, treating empty as 0
pub fn parse_or_zero_f64(s: &str) -> f64 {
    if s.is_empty() { 0.0 } else { s.parse().unwrap_or(0.0) }
}

pub fn parse_or_zero_usize(s: &str) -> usize {
    if s.is_empty() { 0 } else { s.parse().unwrap_or(0) }
}

pub fn parse_or_zero_f32(s: &str) -> f32 {
    if s.is_empty() { 0.0 } else { s.parse().unwrap_or(0.0) }
}
