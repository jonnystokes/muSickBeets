use std::cell::RefCell;
use std::rc::Rc;

use fltk::{
    button::Button,
    enums::{Event, Key},
    input::{FloatInput, Input},
    prelude::*,
};

// ─── Float/Int Input Validation ──────────────────────────────────────────────
//
// Revert-based validation using handle() instead of set_callback().
// This is critical: set_callback() can only have ONE callback per widget,
// so if a functional callback is set later, it overwrites validation.
// handle() is independent and always fires, regardless of other callbacks.
//
// Works on VNC/Termux/remote desktop where input arrives as Shortcut/Paste events.
// On every text change, we check validity and revert if invalid.

pub fn is_valid_float_input(text: &str) -> bool {
    let digits = text.strip_prefix('-').unwrap_or(text);
    if digits.is_empty() {
        return true;
    }
    if digits.starts_with('.') {
        return false;
    }
    let parts: Vec<&str> = digits.split('.').collect();
    parts.len() <= 2
        && parts
            .iter()
            .all(|p| p.is_empty() || p.chars().all(|c| c.is_ascii_digit()))
}

pub fn is_valid_uint_input(text: &str) -> bool {
    text.is_empty() || text.chars().all(|c| c.is_ascii_digit())
}

/// Attach revert-based float validation via handle().
/// Survives any later set_callback() calls on the same widget.
pub fn attach_float_validation(input: &mut FloatInput) {
    let last_valid = Rc::new(RefCell::new(input.value()));
    input.handle(move |field, ev| {
        // Block spacebar from reaching the text field entirely.
        // Space is a global shortcut for recompute; it must never insert into text.
        if fltk::app::event_key() == fltk::enums::Key::from_char(' ') {
            return matches!(ev, Event::KeyDown | Event::KeyUp | Event::Shortcut);
        }
        // We care about any event that may have changed the text:
        // KeyUp, Paste, Shortcut, Unfocus, etc.
        // Rather than guessing events, just check on every event whether text changed.
        match ev {
            Event::KeyUp | Event::Paste | Event::Shortcut | Event::Unfocus => {
                let current = field.value();
                let lv = last_valid.borrow().clone();
                if current == lv {
                    return false; // no change
                }
                let minus_just_added = current.contains('-') && !lv.contains('-');
                let typed_at_start = field.position() == 1;
                if (typed_at_start || !minus_just_added) && is_valid_float_input(&current) {
                    *last_valid.borrow_mut() = current;
                } else {
                    let restore = field.position().saturating_sub(1);
                    field.set_value(&lv);
                    field.set_position(restore).ok();
                }
                false // don't consume — let other handlers see it too
            }
            _ => false,
        }
    });
}

/// Attach revert-based uint validation via handle().
/// Survives any later set_callback() calls on the same widget.
pub fn attach_uint_validation(input: &mut Input) {
    let last_valid = Rc::new(RefCell::new(input.value()));
    input.handle(move |field, ev| {
        // Block spacebar from reaching the text field entirely.
        if fltk::app::event_key() == fltk::enums::Key::from_char(' ') {
            return matches!(ev, Event::KeyDown | Event::KeyUp | Event::Shortcut);
        }
        match ev {
            Event::KeyUp | Event::Paste | Event::Shortcut | Event::Unfocus => {
                let current = field.value();
                let lv = last_valid.borrow().clone();
                if current == lv {
                    return false;
                }
                if is_valid_uint_input(&current) {
                    *last_valid.borrow_mut() = current;
                } else {
                    let restore = field.position().saturating_sub(1);
                    field.set_value(&lv);
                    field.set_position(restore).ok();
                }
                false
            }
            _ => false,
        }
    });
}

/// Float validation + spacebar guard that also triggers recompute.
/// Used by setup_spacebar_guards() to replace the plain validation handler
/// so that space KeyUp triggers btn_rerun.do_callback().
pub fn attach_float_validation_with_recompute(input: &mut FloatInput, btn_rerun: &Button) {
    let last_valid = Rc::new(RefCell::new(input.value()));
    let mut btn = btn_rerun.clone();
    input.handle(move |field, ev| {
        if fltk::app::event_key() == Key::from_char(' ') {
            return match ev {
                Event::KeyDown | Event::Shortcut => true,
                Event::KeyUp => {
                    btn.do_callback();
                    true
                }
                _ => false,
            };
        }
        match ev {
            Event::KeyUp | Event::Paste | Event::Shortcut | Event::Unfocus => {
                let current = field.value();
                let lv = last_valid.borrow().clone();
                if current == lv {
                    return false;
                }
                let minus_just_added = current.contains('-') && !lv.contains('-');
                let typed_at_start = field.position() == 1;
                if (typed_at_start || !minus_just_added) && is_valid_float_input(&current) {
                    *last_valid.borrow_mut() = current;
                } else {
                    let restore = field.position().saturating_sub(1);
                    field.set_value(&lv);
                    field.set_position(restore).ok();
                }
                false
            }
            _ => false,
        }
    });
}

/// Uint validation + spacebar guard that also triggers recompute.
pub fn attach_uint_validation_with_recompute(input: &mut Input, btn_rerun: &Button) {
    let last_valid = Rc::new(RefCell::new(input.value()));
    let mut btn = btn_rerun.clone();
    input.handle(move |field, ev| {
        if fltk::app::event_key() == Key::from_char(' ') {
            return match ev {
                Event::KeyDown | Event::Shortcut => true,
                Event::KeyUp => {
                    btn.do_callback();
                    true
                }
                _ => false,
            };
        }
        match ev {
            Event::KeyUp | Event::Paste | Event::Shortcut | Event::Unfocus => {
                let current = field.value();
                let lv = last_valid.borrow().clone();
                if current == lv {
                    return false;
                }
                if is_valid_uint_input(&current) {
                    *last_valid.borrow_mut() = current;
                } else {
                    let restore = field.position().saturating_sub(1);
                    field.set_value(&lv);
                    field.set_position(restore).ok();
                }
                false
            }
            _ => false,
        }
    });
}

// Helper: parse a field value, treating empty as 0
pub fn parse_or_zero_f64(s: &str) -> f64 {
    if s.is_empty() {
        0.0
    } else {
        s.parse().unwrap_or(0.0)
    }
}

pub fn parse_or_zero_usize(s: &str) -> usize {
    if s.is_empty() {
        0
    } else {
        s.parse().unwrap_or(0)
    }
}

pub fn parse_or_zero_f32(s: &str) -> f32 {
    if s.is_empty() {
        0.0
    } else {
        s.parse().unwrap_or(0.0)
    }
}

// ─── Norm Floor Display ──────────────────────────────────────────────────────

/// Format a norm floor f64 value as a plain decimal string with commas
/// inserted every 3 digits in both the integer and fractional parts.
pub fn format_norm_floor_with_commas_f64(val: f64) -> String {
    let plain = format!("{:.30}", val);
    let trimmed = plain.trim_end_matches('0');
    let trimmed = if trimmed.ends_with('.') {
        &plain[..trimmed.len() + 1]
    } else {
        trimmed
    };

    let parts: Vec<&str> = trimmed.splitn(2, '.').collect();
    let int_part = parts[0];
    let frac_part = if parts.len() > 1 { parts[1] } else { "0" };

    let int_with_commas: String = {
        let chars: Vec<char> = int_part.chars().collect();
        let mut result = String::new();
        for (i, c) in chars.iter().rev().enumerate() {
            if i > 0 && i % 3 == 0 {
                result.push(',');
            }
            result.push(*c);
        }
        result.chars().rev().collect()
    };

    let frac_with_commas: String = {
        let chars: Vec<char> = frac_part.chars().collect();
        let mut result = String::new();
        for (i, c) in chars.iter().enumerate() {
            if i > 0 && i % 3 == 0 {
                result.push(',');
            }
            result.push(*c);
        }
        result
    };

    format!("{}.{}", int_with_commas, frac_with_commas)
}

/// Format a f64 value in scientific notation (e.g., "1e-20").
pub fn format_scientific_f64(val: f64) -> String {
    if val == 0.0 {
        return "0".to_string();
    }
    let exp = val.log10().floor() as i32;
    let mantissa = val / 10.0_f64.powi(exp);
    if (mantissa - 1.0).abs() < 0.01 {
        format!("1e{}", exp)
    } else {
        format!("{:.1}e{}", mantissa, exp)
    }
}
