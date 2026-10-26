//! Settings that the app keeps when it closes (REQUIREMENTS.md §10, §14.2).

use serde::{Deserialize, Serialize};

use crate::generator::{MIDDLE_MODE, MODES};

/// The cell colors. They are pale, so that the digits are easy to read. Black is not available (COL-6).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Color {
    Gray,
    White,
    Yellow,
    Blue,
    Red,
    Green,
    Cyan,
    Magenta,
    Orange,
}

impl Color {
    /// The order of the colors in the 3x3 grid of the Cell colors panel.
    pub const ALL: [Color; 9] = [
        Color::Gray,
        Color::White,
        Color::Yellow,
        Color::Blue,
        Color::Red,
        Color::Green,
        Color::Cyan,
        Color::Magenta,
        Color::Orange,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Color::Gray => "Gray",
            Color::White => "White",
            Color::Yellow => "Yellow",
            Color::Blue => "Blue",
            Color::Red => "Red",
            Color::Green => "Green",
            Color::Cyan => "Cyan",
            Color::Magenta => "Magenta",
            Color::Orange => "Orange",
        }
    }

    /// The RGB value (toolbar.sh `COLORS`).
    pub fn rgb(self) -> [u8; 3] {
        match self {
            Color::Gray => [0xC8, 0xC8, 0xC8],
            Color::White => [0xFF, 0xFF, 0xFF],
            Color::Yellow => [0xFF, 0xE6, 0x80],
            Color::Blue => [0xAF, 0xC4, 0xF5],
            Color::Red => [0xF0, 0xA3, 0xA3],
            Color::Green => [0xB5, 0xDF, 0x9E],
            Color::Cyan => [0xA6, 0xE0, 0xEE],
            Color::Magenta => [0xDC, 0xB3, 0xE6],
            Color::Orange => [0xF7, 0xC5, 0x94],
        }
    }
}

/// The three sections of the Cell colors panel, from the top.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    Starting,
    Clash,
    Highlight,
}

impl Section {
    pub const ALL: [Section; 3] = [Section::Starting, Section::Clash, Section::Highlight];

    pub fn title(self) -> &'static str {
        match self {
            Section::Starting => "Starting numbers",
            Section::Clash => "Clashing numbers",
            Section::Highlight => "Highlighted numbers",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    pub color_starting: Color,
    pub color_clash: Color,
    pub color_highlight: Color,
    pub last_mode: u8,
    /// The switches of the Settings panel (REQUIREMENTS.md §9.1). A settings file of an earlier version
    /// does not have them: then they are on.
    #[serde(default = "on")]
    pub highlight_on: bool,
    #[serde(default = "on")]
    pub auto_remove_on: bool,
}

fn on() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            color_starting: Color::Gray,
            color_clash: Color::Red,
            color_highlight: Color::Green,
            last_mode: MIDDLE_MODE,
            highlight_on: true,
            auto_remove_on: true,
        }
    }
}

impl Settings {
    pub fn color(&self, s: Section) -> Color {
        match s {
            Section::Starting => self.color_starting,
            Section::Clash => self.color_clash,
            Section::Highlight => self.color_highlight,
        }
    }

    /// The color is not available for section `s`, because a different section uses it (COL-5).
    /// White is always available.
    pub fn is_taken(&self, s: Section, color: Color) -> bool {
        color != Color::White && Section::ALL.iter().any(|&o| o != s && self.color(o) == color)
    }

    /// Selects a color for a section. Returns false if the color is not available.
    pub fn select(&mut self, s: Section, color: Color) -> bool {
        if self.is_taken(s, color) {
            return false;
        }
        match s {
            Section::Starting => self.color_starting = color,
            Section::Clash => self.color_clash = color,
            Section::Highlight => self.color_highlight = color,
        }
        true
    }

    /// The settings agree with COL-5 and DIF-1.
    pub fn is_valid(&self) -> bool {
        let c = [self.color_starting, self.color_clash, self.color_highlight];
        let distinct = (0..3).all(|i| (i + 1..3).all(|j| c[i] != c[j] || c[i] == Color::White));
        distinct && self.last_mode < MODES
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults() {
        let s = Settings::default();
        assert_eq!((s.color_starting, s.color_clash, s.color_highlight), (Color::Gray, Color::Red, Color::Green));
        assert_eq!(s.last_mode, 2);
        assert!(s.is_valid());
    }

    #[test]
    fn a_settings_file_without_the_switches_has_them_on() {
        // PER-3 for the settings files of an earlier version.
        let s: Settings =
            serde_json::from_str(r#"{"color_starting":"Gray","color_clash":"Red","color_highlight":"Green","last_mode":3}"#).unwrap();
        assert!(s.highlight_on && s.auto_remove_on);
        assert_eq!(s.last_mode, 3);
    }

    #[test]
    fn colors_stay_different_except_white() {
        let mut s = Settings::default();
        assert!(!s.select(Section::Starting, Color::Red));
        assert!(s.select(Section::Starting, Color::White));
        assert!(s.select(Section::Clash, Color::White));
        assert!(s.select(Section::Highlight, Color::White));
        assert!(s.is_valid());
        assert!(s.select(Section::Clash, Color::Gray));
        assert!(!s.select(Section::Starting, Color::Gray));
    }
}
