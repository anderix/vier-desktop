// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use eframe::egui::Color32;

use crate::board::Player;

pub const DISC_RED: Color32 = Color32::from_rgb(0xE6, 0x39, 0x46);
pub const DISC_YELLOW: Color32 = Color32::from_rgb(0xFF, 0xC9, 0x3C);
pub const BOARD_BLUE: Color32 = Color32::from_rgb(0x1D, 0x35, 0x57);
pub const SEGMENT_TRACK: Color32 = Color32::from_rgb(0x1C, 0x1C, 0x1E);
pub const SEGMENT_SELECTED: Color32 = Color32::from_rgb(0x63, 0x63, 0x66);
pub const QUIET_GRAY: Color32 = Color32::from_rgb(0x5A, 0x5A, 0x5E);

pub fn disc_color(player: Player) -> Color32 {
    match player {
        Player::Red => DISC_RED,
        Player::Yellow => DISC_YELLOW,
    }
}
