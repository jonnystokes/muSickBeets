use fltk::prelude::*;

/// Global tooltip enable/disable state
pub struct TooltipManager {
    enabled: bool,
}

impl TooltipManager {
    pub fn new() -> Self {
        // Configure FLTK tooltip appearance for dark theme
        fltk::misc::Tooltip::set_color(fltk::enums::Color::from_hex(0x363646));
        fltk::misc::Tooltip::set_text_color(fltk::enums::Color::from_hex(0xcdd6f4));
        fltk::misc::Tooltip::set_font_size(11);
        fltk::misc::Tooltip::set_delay(0.5);
        fltk::misc::Tooltip::set_margin_width(4);
        fltk::misc::Tooltip::set_margin_height(4);

        Self { enabled: true }
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if enabled {
            fltk::misc::Tooltip::enable(true);
        } else {
            fltk::misc::Tooltip::disable();
        }
    }
}

/// Helper to set a tooltip on any widget
pub fn set_tooltip<W: WidgetExt>(widget: &mut W, text: &str) {
    widget.set_tooltip(text);
}
