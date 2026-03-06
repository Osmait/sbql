//! Catppuccin Mocha colour palette.
//!
//! All UI files should import colours from here instead of using
//! `ratatui::style::Color::*` named variants.
//!
//! The full palette is defined as a reference; individual entries may not all
//! be used by the current UI, hence the blanket allow.
#![allow(dead_code)]

use ratatui::style::Color;

// ---------------------------------------------------------------------------
// Base / surface
// ---------------------------------------------------------------------------

/// Base background `(30, 30, 46)`.
pub const BASE: Color = Color::Rgb(30, 30, 46);
/// Elevated surface `(49, 50, 68)`.
pub const SURFACE0: Color = Color::Rgb(49, 50, 68);
/// Higher surface `(69, 71, 90)`.
pub const SURFACE1: Color = Color::Rgb(69, 71, 90);
/// Highest surface `(88, 91, 112)`.
pub const SURFACE2: Color = Color::Rgb(88, 91, 112);

// ---------------------------------------------------------------------------
// Overlay / text
// ---------------------------------------------------------------------------

/// Subtle borders / help text `(108, 112, 134)`.
pub const OVERLAY0: Color = Color::Rgb(108, 112, 134);
/// Muted text `(127, 132, 156)`.
pub const OVERLAY1: Color = Color::Rgb(127, 132, 156);
/// Normal text `(147, 153, 178)`.
pub const OVERLAY2: Color = Color::Rgb(147, 153, 178);

/// Slightly dimmed text `(186, 194, 222)`.
pub const SUBTEXT1: Color = Color::Rgb(186, 194, 222);
/// Dimmed text `(166, 173, 200)`.
pub const SUBTEXT0: Color = Color::Rgb(166, 173, 200);
/// Bright / primary text `(205, 214, 244)`.
pub const TEXT: Color = Color::Rgb(205, 214, 244);

// ---------------------------------------------------------------------------
// Accent colours
// ---------------------------------------------------------------------------

/// Warm accent `(245, 224, 220)`.
pub const ROSEWATER: Color = Color::Rgb(245, 224, 220);
/// Pink-red accent `(242, 205, 205)`.
pub const FLAMINGO: Color = Color::Rgb(242, 205, 205);
/// Pink accent `(245, 189, 230)`.
pub const PINK: Color = Color::Rgb(245, 189, 230);
/// Purple accent `(203, 166, 247)`.
pub const MAUVE: Color = Color::Rgb(203, 166, 247);

/// Error / delete `(243, 139, 168)`.
pub const RED: Color = Color::Rgb(243, 139, 168);
/// Warning-red `(235, 160, 172)`.
pub const MAROON: Color = Color::Rgb(235, 160, 172);
/// Warning / staged edits `(250, 179, 135)`.
pub const PEACH: Color = Color::Rgb(250, 179, 135);
/// Staged / pending `(249, 226, 175)`.
pub const YELLOW: Color = Color::Rgb(249, 226, 175);

/// Success / active `(166, 227, 161)`.
pub const GREEN: Color = Color::Rgb(166, 227, 161);
/// Info `(148, 226, 213)`.
pub const TEAL: Color = Color::Rgb(148, 226, 213);
/// Light blue `(137, 220, 235)`.
pub const SKY: Color = Color::Rgb(137, 220, 235);
/// Medium blue `(116, 199, 236)`.
pub const SAPPHIRE: Color = Color::Rgb(116, 199, 236);
/// Primary accent `(137, 180, 250)`.
pub const BLUE: Color = Color::Rgb(137, 180, 250);
/// Soft accent `(180, 190, 254)`.
pub const LAVENDER: Color = Color::Rgb(180, 190, 254);
