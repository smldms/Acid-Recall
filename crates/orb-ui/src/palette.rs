//! The colors the windows share (task-128): the ink of the texts, the panel's background, the
//! slate of the boxes and the amber of the lamps. A color that one window alone uses stays in
//! that window's module.

use eframe::egui::Color32;

/// The text ink: the main texts of the windows.
pub const INK: Color32 = Color32::from_rgb(0xee, 0xee, 0xf0);
/// The dimmed ink: the labels and the secondary texts.
pub const INK_DIM: Color32 = Color32::from_rgb(0xaa, 0xaa, 0xb1);
/// The panel's background and the letterbox color.
pub const BG: Color32 = Color32::from_rgb(0x0e, 0x0e, 0x10);
/// The dark slate of the boxes' fills.
pub const SLATE_DARK: Color32 = Color32::from_rgb(0x20, 0x20, 0x23);
/// The light slate of the boxes' outlines and the selection.
pub const SLATE_LINE: Color32 = Color32::from_rgb(0x49, 0x49, 0x4e);
/// The amber of the panel's lamps: the accent of the windows (a title, a warning).
pub const AMBER: Color32 = Color32::from_rgb(0xd5, 0xe7, 0x86);

/// The default skin's value rings; shared across all machines for a calm rack.
pub const KNOB_ARC: [u8; 3] = [0xd5, 0xe7, 0x86];
