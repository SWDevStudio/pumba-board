//! Autumn Breeze Dark palette (taken from the nvim colorscheme).
use ratatui::style::Color;

const fn rgb(hex: u32) -> Color {
    Color::Rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// Terminal's own background (transparent): used for every background fill.
pub const BG: Color = Color::Reset;
/// Solid Autumn Breeze background: only as a foreground for dark text on bright accent backgrounds.
pub const BG_SOLID: Color = rgb(0x211E1B);
pub const PANEL: Color = rgb(0x282421);
pub const FG: Color = rgb(0xF4E9D6);
pub const MUTED: Color = rgb(0x665B4F);
pub const ORANGE: Color = rgb(0xF09138);
/// Muted orange: secondary markers that must not compete with the selection border.
pub const ORANGE_DIM: Color = rgb(0xB5702E);
pub const RED: Color = rgb(0xD84233);
pub const GREEN: Color = rgb(0xB4AF4C);
pub const STRING: Color = rgb(0xD8AA7F);
pub const PURPLE: Color = rgb(0x81738A);
pub const PINK: Color = rgb(0xB45B69);
pub const PARAM: Color = rgb(0xDFC9A4);

pub const COLOR_NAMES: [&str; 6] = ["purple", "orange", "green", "red", "pink", "tan"];

pub fn by_name(name: &str) -> Color {
    match name {
        "orange" => ORANGE,
        "green" => GREEN,
        "red" => RED,
        "pink" => PINK,
        "tan" => STRING,
        "purple" => PURPLE,
        _ => MUTED,
    }
}
