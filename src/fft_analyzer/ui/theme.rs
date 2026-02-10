use fltk::{app, enums::Color};

// Dark theme color palette
pub const BG_DARK: u32        = 0x1e1e2e;  // main background
pub const BG_PANEL: u32       = 0x2a2a3a;  // panel background
pub const BG_WIDGET: u32      = 0x363646;  // widget/input background
pub const TEXT_PRIMARY: u32   = 0xcdd6f4;  // main text
pub const TEXT_SECONDARY: u32 = 0xa6adc8;  // dimmed text
pub const TEXT_DISABLED: u32  = 0x6c7086;  // disabled/greyed text
pub const ACCENT_BLUE: u32   = 0x89b4fa;  // primary accent
pub const ACCENT_GREEN: u32  = 0xa6e3a1;  // positive / active
pub const ACCENT_RED: u32    = 0xf38ba8;  // warning / cursor
pub const ACCENT_YELLOW: u32 = 0xf9e2af;  // highlights
pub const ACCENT_MAUVE: u32  = 0xcba6f7;  // section headers
pub const BORDER: u32        = 0x45475a;  // subtle borders
pub const SEPARATOR: u32     = 0x585b70;  // separator lines

pub fn apply_dark_theme() {
    app::set_background_color(
        ((BG_PANEL >> 16) & 0xFF) as u8,
        ((BG_PANEL >> 8) & 0xFF) as u8,
        (BG_PANEL & 0xFF) as u8,
    );
    app::set_background2_color(
        ((BG_WIDGET >> 16) & 0xFF) as u8,
        ((BG_WIDGET >> 8) & 0xFF) as u8,
        (BG_WIDGET & 0xFF) as u8,
    );
    app::set_foreground_color(
        ((TEXT_PRIMARY >> 16) & 0xFF) as u8,
        ((TEXT_PRIMARY >> 8) & 0xFF) as u8,
        (TEXT_PRIMARY & 0xFF) as u8,
    );
    app::set_selection_color(
        ((ACCENT_BLUE >> 16) & 0xFF) as u8,
        ((ACCENT_BLUE >> 8) & 0xFF) as u8,
        (ACCENT_BLUE & 0xFF) as u8,
    );
    app::set_inactive_color(
        ((TEXT_DISABLED >> 16) & 0xFF) as u8,
        ((TEXT_DISABLED >> 8) & 0xFF) as u8,
        (TEXT_DISABLED & 0xFF) as u8,
    );

    // Use the plastic scheme for better dark theme compatibility
    app::set_scheme(app::Scheme::Gtk);
}

pub fn color(hex: u32) -> Color {
    Color::from_hex(hex)
}

pub fn section_header_color() -> Color {
    Color::from_hex(ACCENT_MAUVE)
}

pub fn accent_color() -> Color {
    Color::from_hex(ACCENT_BLUE)
}

