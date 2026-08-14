//! Colour palettes, switchable while the app is running.
//!
//! The roles are Catppuccin's, because that is the palette the UI was built
//! against and its names say what a colour is *for* rather than what it looks
//! like — `overlay0` is the border colour whether it ends up grey or beige.
//! The other themes are mapped onto those same roles, so a theme is a table of
//! colours and nothing else has to know which one is in use.
//!
//! Where a theme has no obvious counterpart for a role — most of them have no
//! distinct "flamingo" — the nearest neighbour is reused. The mapping is a
//! judgement, not a specification, so it is written out in full here rather
//! than computed, and can be corrected one line at a time.
//!
//! Colours are read through functions rather than constants because the theme
//! changes at runtime; the index is an atomic, so reading one costs a relaxed
//! load and drawing stays lock-free.

use ratatui::style::Color;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Every colour role the UI draws with.
///
/// The set is complete even where the UI has no use for a role yet: a partial
/// palette would make each new theme a guess about which entries matter, and
/// the tables below are copied from published palettes that define all of
/// them. Accessors exist only for the roles something actually draws with, so
/// an unused one shows up as dead code rather than sitting here forever.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    pub base: Color,
    pub surface0: Color,
    pub surface1: Color,
    pub surface2: Color,
    pub overlay0: Color,
    pub overlay1: Color,
    pub overlay2: Color,
    pub subtext0: Color,
    pub subtext1: Color,
    pub text: Color,
    pub rosewater: Color,
    pub flamingo: Color,
    pub pink: Color,
    pub mauve: Color,
    pub red: Color,
    pub maroon: Color,
    pub peach: Color,
    pub yellow: Color,
    pub green: Color,
    pub teal: Color,
    pub sky: Color,
    pub sapphire: Color,
    pub blue: Color,
    pub lavender: Color,
}

/// A named palette.
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub name: &'static str,
    pub palette: Palette,
}

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(r, g, b)
}

/// Every theme on offer, in the order the picker lists them.
pub const THEMES: &[Theme] = &[
    Theme {
        name: "Catppuccin Mocha",
        palette: Palette {
            base: rgb(30, 30, 46),
            surface0: rgb(49, 50, 68),
            surface1: rgb(69, 71, 90),
            surface2: rgb(88, 91, 112),
            overlay0: rgb(108, 112, 134),
            overlay1: rgb(127, 132, 156),
            overlay2: rgb(147, 153, 178),
            subtext0: rgb(166, 173, 200),
            subtext1: rgb(186, 194, 222),
            text: rgb(205, 214, 244),
            rosewater: rgb(245, 224, 220),
            flamingo: rgb(242, 205, 205),
            pink: rgb(245, 189, 230),
            mauve: rgb(203, 166, 247),
            red: rgb(243, 139, 168),
            maroon: rgb(235, 160, 172),
            peach: rgb(250, 179, 135),
            yellow: rgb(249, 226, 175),
            green: rgb(166, 227, 161),
            teal: rgb(148, 226, 213),
            sky: rgb(137, 220, 235),
            sapphire: rgb(116, 199, 236),
            blue: rgb(137, 180, 250),
            lavender: rgb(180, 190, 254),
        },
    },
    Theme {
        name: "Catppuccin Macchiato",
        palette: Palette {
            base: rgb(36, 39, 58),
            surface0: rgb(54, 58, 79),
            surface1: rgb(73, 77, 100),
            surface2: rgb(91, 96, 120),
            overlay0: rgb(110, 115, 141),
            overlay1: rgb(128, 135, 162),
            overlay2: rgb(147, 154, 183),
            subtext0: rgb(165, 173, 203),
            subtext1: rgb(184, 192, 224),
            text: rgb(202, 211, 245),
            rosewater: rgb(244, 219, 214),
            flamingo: rgb(240, 198, 198),
            pink: rgb(245, 189, 230),
            mauve: rgb(198, 160, 246),
            red: rgb(237, 135, 150),
            maroon: rgb(238, 153, 160),
            peach: rgb(245, 169, 127),
            yellow: rgb(238, 212, 159),
            green: rgb(166, 218, 149),
            teal: rgb(139, 213, 202),
            sky: rgb(145, 215, 227),
            sapphire: rgb(125, 196, 228),
            blue: rgb(138, 173, 244),
            lavender: rgb(183, 189, 248),
        },
    },
    Theme {
        name: "Catppuccin Frappé",
        palette: Palette {
            base: rgb(48, 52, 70),
            surface0: rgb(65, 69, 89),
            surface1: rgb(81, 87, 109),
            surface2: rgb(98, 104, 128),
            overlay0: rgb(115, 121, 148),
            overlay1: rgb(131, 139, 167),
            overlay2: rgb(148, 156, 187),
            subtext0: rgb(165, 173, 206),
            subtext1: rgb(181, 191, 226),
            text: rgb(198, 208, 245),
            rosewater: rgb(242, 213, 207),
            flamingo: rgb(238, 190, 190),
            pink: rgb(244, 184, 228),
            mauve: rgb(202, 158, 230),
            red: rgb(231, 130, 132),
            maroon: rgb(234, 153, 156),
            peach: rgb(239, 159, 118),
            yellow: rgb(229, 200, 144),
            green: rgb(166, 209, 137),
            teal: rgb(129, 200, 190),
            sky: rgb(153, 209, 219),
            sapphire: rgb(133, 193, 220),
            blue: rgb(140, 170, 238),
            lavender: rgb(186, 187, 241),
        },
    },
    Theme {
        name: "Catppuccin Latte",
        palette: Palette {
            base: rgb(239, 241, 245),
            surface0: rgb(204, 208, 218),
            surface1: rgb(188, 192, 204),
            surface2: rgb(172, 176, 190),
            overlay0: rgb(156, 160, 176),
            overlay1: rgb(140, 143, 161),
            overlay2: rgb(124, 127, 147),
            subtext0: rgb(108, 111, 133),
            subtext1: rgb(92, 95, 119),
            text: rgb(76, 79, 105),
            rosewater: rgb(220, 138, 120),
            flamingo: rgb(221, 120, 120),
            pink: rgb(234, 118, 203),
            mauve: rgb(136, 57, 239),
            red: rgb(210, 15, 57),
            maroon: rgb(230, 69, 83),
            peach: rgb(254, 100, 11),
            yellow: rgb(223, 142, 29),
            green: rgb(64, 160, 43),
            teal: rgb(23, 146, 153),
            sky: rgb(4, 165, 229),
            sapphire: rgb(32, 159, 181),
            blue: rgb(30, 102, 245),
            lavender: rgb(114, 135, 253),
        },
    },
    Theme {
        name: "Tokyo Night",
        palette: Palette {
            base: rgb(26, 27, 38),
            surface0: rgb(36, 40, 59),
            surface1: rgb(41, 46, 66),
            surface2: rgb(59, 66, 97),
            overlay0: rgb(86, 95, 137),
            overlay1: rgb(107, 115, 148),
            overlay2: rgb(121, 130, 169),
            subtext0: rgb(154, 165, 206),
            subtext1: rgb(169, 177, 214),
            text: rgb(192, 202, 245),
            rosewater: rgb(255, 158, 100),
            flamingo: rgb(247, 118, 142),
            pink: rgb(187, 154, 247),
            mauve: rgb(187, 154, 247),
            red: rgb(247, 118, 142),
            maroon: rgb(255, 117, 127),
            peach: rgb(255, 158, 100),
            yellow: rgb(224, 175, 104),
            green: rgb(158, 206, 106),
            teal: rgb(115, 218, 202),
            sky: rgb(125, 207, 255),
            sapphire: rgb(42, 195, 222),
            blue: rgb(122, 162, 247),
            lavender: rgb(180, 249, 248),
        },
    },
    Theme {
        name: "Gruvbox Dark",
        palette: Palette {
            base: rgb(40, 40, 40),
            surface0: rgb(60, 56, 54),
            surface1: rgb(80, 73, 69),
            surface2: rgb(102, 92, 84),
            overlay0: rgb(124, 111, 100),
            overlay1: rgb(146, 131, 116),
            overlay2: rgb(168, 153, 132),
            subtext0: rgb(189, 174, 147),
            subtext1: rgb(213, 196, 161),
            text: rgb(235, 219, 178),
            rosewater: rgb(211, 134, 155),
            flamingo: rgb(234, 105, 98),
            pink: rgb(211, 134, 155),
            mauve: rgb(211, 134, 155),
            red: rgb(251, 73, 52),
            maroon: rgb(204, 36, 29),
            peach: rgb(254, 128, 25),
            yellow: rgb(250, 189, 47),
            green: rgb(184, 187, 38),
            teal: rgb(142, 192, 124),
            sky: rgb(131, 165, 152),
            sapphire: rgb(69, 133, 136),
            blue: rgb(131, 165, 152),
            lavender: rgb(211, 134, 155),
        },
    },
    Theme {
        name: "Nord",
        palette: Palette {
            base: rgb(46, 52, 64),
            surface0: rgb(59, 66, 82),
            surface1: rgb(67, 76, 94),
            surface2: rgb(76, 86, 106),
            overlay0: rgb(97, 110, 136),
            overlay1: rgb(109, 122, 148),
            overlay2: rgb(123, 136, 161),
            subtext0: rgb(216, 222, 233),
            subtext1: rgb(229, 233, 240),
            text: rgb(236, 239, 244),
            rosewater: rgb(216, 222, 233),
            flamingo: rgb(208, 135, 112),
            pink: rgb(180, 142, 173),
            mauve: rgb(180, 142, 173),
            red: rgb(191, 97, 106),
            maroon: rgb(191, 97, 106),
            peach: rgb(208, 135, 112),
            yellow: rgb(235, 203, 139),
            green: rgb(163, 190, 140),
            teal: rgb(143, 188, 187),
            sky: rgb(136, 192, 208),
            sapphire: rgb(129, 161, 193),
            blue: rgb(94, 129, 172),
            lavender: rgb(180, 142, 173),
        },
    },
    Theme {
        name: "Dracula",
        palette: Palette {
            base: rgb(40, 42, 54),
            surface0: rgb(68, 71, 90),
            surface1: rgb(77, 80, 102),
            surface2: rgb(86, 89, 115),
            overlay0: rgb(98, 114, 164),
            overlay1: rgb(114, 130, 180),
            overlay2: rgb(130, 146, 196),
            subtext0: rgb(216, 216, 210),
            subtext1: rgb(232, 232, 226),
            text: rgb(248, 248, 242),
            rosewater: rgb(255, 184, 108),
            flamingo: rgb(255, 121, 198),
            pink: rgb(255, 121, 198),
            mauve: rgb(189, 147, 249),
            red: rgb(255, 85, 85),
            maroon: rgb(255, 110, 110),
            peach: rgb(255, 184, 108),
            yellow: rgb(241, 250, 140),
            green: rgb(80, 250, 123),
            teal: rgb(139, 233, 253),
            sky: rgb(139, 233, 253),
            sapphire: rgb(98, 114, 164),
            blue: rgb(189, 147, 249),
            lavender: rgb(189, 147, 249),
        },
    },
    Theme {
        name: "Rosé Pine",
        palette: Palette {
            base: rgb(25, 23, 36),
            surface0: rgb(31, 29, 46),
            surface1: rgb(38, 35, 58),
            surface2: rgb(47, 43, 67),
            overlay0: rgb(110, 106, 134),
            overlay1: rgb(125, 122, 149),
            overlay2: rgb(144, 140, 170),
            subtext0: rgb(183, 179, 204),
            subtext1: rgb(203, 200, 222),
            text: rgb(224, 222, 244),
            rosewater: rgb(235, 188, 186),
            flamingo: rgb(235, 111, 146),
            pink: rgb(196, 167, 231),
            mauve: rgb(196, 167, 231),
            red: rgb(235, 111, 146),
            maroon: rgb(215, 130, 126),
            peach: rgb(246, 193, 119),
            yellow: rgb(246, 193, 119),
            green: rgb(156, 207, 216),
            teal: rgb(49, 116, 143),
            sky: rgb(156, 207, 216),
            sapphire: rgb(49, 116, 143),
            blue: rgb(62, 143, 176),
            lavender: rgb(196, 167, 231),
        },
    },
];

/// Which theme is in use. An index rather than the palette itself so switching
/// is a single atomic store from anywhere.
static CURRENT: AtomicUsize = AtomicUsize::new(0);

/// The palette in use. Out-of-range indices fall back to the first theme, so a
/// stale name in the config file cannot leave the UI without colours.
fn current() -> &'static Palette {
    let i = CURRENT.load(Ordering::Relaxed);
    &THEMES.get(i).unwrap_or(&THEMES[0]).palette
}

/// Switch to a theme by index. Ignored if out of range.
pub fn set(index: usize) {
    if index < THEMES.len() {
        CURRENT.store(index, Ordering::Relaxed);
    }
}

pub fn current_index() -> usize {
    CURRENT.load(Ordering::Relaxed).min(THEMES.len() - 1)
}

pub fn current_name() -> &'static str {
    THEMES[current_index()].name
}

/// Switch by name, as stored in the config file. Returns whether it matched;
/// a name that does not is left to the caller to report or ignore.
pub fn set_by_name(name: &str) -> bool {
    match THEMES.iter().position(|t| t.name == name) {
        Some(i) => {
            set(i);
            true
        }
        None => false,
    }
}

pub fn base() -> Color {
    current().base
}

pub fn surface0() -> Color {
    current().surface0
}

pub fn surface2() -> Color {
    current().surface2
}

pub fn overlay0() -> Color {
    current().overlay0
}

pub fn overlay1() -> Color {
    current().overlay1
}

pub fn overlay2() -> Color {
    current().overlay2
}

pub fn subtext0() -> Color {
    current().subtext0
}

pub fn text() -> Color {
    current().text
}

pub fn flamingo() -> Color {
    current().flamingo
}

pub fn pink() -> Color {
    current().pink
}

pub fn mauve() -> Color {
    current().mauve
}

pub fn red() -> Color {
    current().red
}

pub fn maroon() -> Color {
    current().maroon
}

pub fn peach() -> Color {
    current().peach
}

pub fn yellow() -> Color {
    current().yellow
}

pub fn green() -> Color {
    current().green
}

pub fn teal() -> Color {
    current().teal
}

pub fn sky() -> Color {
    current().sky
}

pub fn sapphire() -> Color {
    current().sapphire
}

pub fn blue() -> Color {
    current().blue
}

pub fn lavender() -> Color {
    current().lavender
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The default has to stay the palette the UI was designed against.
    #[test]
    fn the_first_theme_is_the_one_the_ui_was_built_with() {
        assert_eq!(THEMES[0].name, "Catppuccin Mocha");
        assert_eq!(THEMES[0].palette.base, Color::Rgb(30, 30, 46));
    }

    /// A theme with two roles sharing a colour is fine, but a theme where the
    /// text and the background match is unreadable.
    #[test]
    fn every_theme_can_show_text_on_its_background() {
        for theme in THEMES {
            assert_ne!(
                theme.palette.text, theme.palette.base,
                "{} draws text in its own background colour",
                theme.name
            );
            assert_ne!(
                theme.palette.overlay0, theme.palette.base,
                "{} draws borders in its own background colour",
                theme.name
            );
        }
    }

    #[test]
    fn switching_changes_what_the_colour_functions_return() {
        set(0);
        let first = base();
        set(3);
        assert_ne!(base(), first);
        assert_eq!(current_name(), THEMES[3].name);
        set(0);
    }

    /// A name from a config file written by a newer version must not leave the
    /// UI colourless.
    #[test]
    fn an_unknown_name_is_refused_and_changes_nothing() {
        set(2);
        assert!(!set_by_name("Solarized From The Future"));
        assert_eq!(current_index(), 2);
        set(0);
    }

    #[test]
    fn names_round_trip() {
        for (i, theme) in THEMES.iter().enumerate() {
            assert!(set_by_name(theme.name));
            assert_eq!(current_index(), i);
        }
        set(0);
    }

    /// Two themes with the same name would make the config ambiguous.
    #[test]
    fn names_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for theme in THEMES {
            assert!(seen.insert(theme.name), "duplicate theme {}", theme.name);
        }
    }
}
