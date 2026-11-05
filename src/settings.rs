//! Settings that the app keeps when it closes (REQUIREMENTS.md §10, §14.2).

use serde::{Deserialize, Serialize};

use crate::game::{Cell, Game};
use crate::generator::{FIRST_MODE, MODES};

/// The cell colors (REQUIREMENTS.md §15). Black is not available (COL-6).
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

    /// The position of the color in the 3x3 grid of the Cell colors panel (`Color::ALL`), from the top left.
    pub fn grid_index(self) -> usize {
        self as usize
    }

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
            Color::Gray => [0xAE, 0xAE, 0xAE],
            Color::White => [0xFF, 0xFF, 0xFF],
            Color::Yellow => [0xFF, 0xD7, 0x40],
            Color::Blue => [0x40, 0x8C, 0xFF],
            Color::Red => [0xFF, 0x8C, 0x8C],
            Color::Green => [0x48, 0xD6, 0x30],
            Color::Cyan => [0x6F, 0xCF, 0xE6],
            Color::Magenta => [0xE6, 0x50, 0xE0],
            Color::Orange => [0xFF, 0x9A, 0x1F],
        }
    }
}

/// The gray of the micro-cell lines (REQUIREMENTS.md §10, BRD-6): None (no lines) or an ImageMagick
/// grayN (N percent of white). The Micro-cell lines section of the Cell colors panel selects it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LineGray {
    None,
    #[default]
    Gray90,
    Gray80,
    Gray70,
    Gray60,
    Gray55,
    Gray50,
    Gray45,
    Gray40,
}

impl LineGray {
    /// The order in the 3x3 grid of the Micro-cell lines section, from the top left: from no lines
    /// to the darkest gray.
    pub const ALL: [LineGray; 9] = [
        LineGray::None,
        LineGray::Gray90,
        LineGray::Gray80,
        LineGray::Gray70,
        LineGray::Gray60,
        LineGray::Gray55,
        LineGray::Gray50,
        LineGray::Gray45,
        LineGray::Gray40,
    ];

    /// The position in the 3x3 grid (`LineGray::ALL`), from the top left.
    pub fn grid_index(self) -> usize {
        self as usize
    }

    /// The N of grayN. None: no lines.
    pub fn percent(self) -> Option<u32> {
        match self {
            LineGray::None => None,
            LineGray::Gray90 => Some(90),
            LineGray::Gray80 => Some(80),
            LineGray::Gray70 => Some(70),
            LineGray::Gray60 => Some(60),
            LineGray::Gray55 => Some(55),
            LineGray::Gray50 => Some(50),
            LineGray::Gray45 => Some(45),
            LineGray::Gray40 => Some(40),
        }
    }

    /// The name below the swatch: "None", or N of grayN.
    pub fn name(self) -> String {
        self.percent().map_or_else(|| "None".to_string(), |n| n.to_string())
    }
}

/// The four color sections of the Cell colors panel, in the order of the columns: from the top of
/// the left column, then from the top of the right column (`layout::color_section_top`). The
/// Micro-cell lines section (`LineGray`) follows them. COL-5 applies to these four sections.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    Starting,
    Clash,
    Highlight,
    Completed,
}

impl Section {
    pub const ALL: [Section; 4] = [Section::Starting, Section::Clash, Section::Highlight, Section::Completed];

    /// The position of the section in the Cell colors panel (`Section::ALL`).
    pub fn panel_index(self) -> usize {
        self as usize
    }

    pub fn title(self) -> &'static str {
        match self {
            Section::Starting => "Starting numbers",
            Section::Clash => "Clashing numbers",
            Section::Highlight => "Highlighted numbers",
            Section::Completed => "Completed groups",
        }
    }
}

/// The switches of the Settings panel (REQUIREMENTS.md §9.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Switch {
    HighlightNumbers,
    HideMarksAutomatically,
    ColorCompletedGroups,
}

impl Switch {
    /// The order of the switch rows in the Settings panel, from the top.
    pub const ALL: [Switch; 3] = [Switch::HighlightNumbers, Switch::HideMarksAutomatically, Switch::ColorCompletedGroups];

    /// The row of the switch in the Settings panel (`layout::settings_row`).
    pub fn row(self) -> usize {
        self as usize
    }

    /// The name in the Settings panel, one item for each line of text.
    pub fn label(self) -> &'static [&'static str] {
        match self {
            Switch::HighlightNumbers => &["Highlight numbers"],
            Switch::HideMarksAutomatically => &["Hide pencil marks", "automatically"],
            Switch::ColorCompletedGroups => &["Color completed", "groups"],
        }
    }
}

/// The settings. The app reads a settings file through `SavedSettings`, which also reads the files
/// of earlier versions (PER-8).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "SavedSettings")]
pub struct Settings {
    pub color_starting: Color,
    pub color_clash: Color,
    pub color_highlight: Color,
    /// The completed color (§6.4). White: no completed color.
    pub color_completed: Color,
    pub last_mode: u8,
    /// The switches of the Settings panel (REQUIREMENTS.md §9.1).
    pub highlight_on: bool,
    /// Hide pencil marks automatically (§6.1).
    pub auto_hide_on: bool,
    /// Color completed groups (§6.4).
    pub completed_color_on: bool,
    /// The gray of the micro-cell lines (§10).
    pub micro_line: LineGray,
}

/// A settings file, also of an earlier version. An earlier version has no value for some fields:
/// the switches are then on, and `micro_line` has its default.
#[derive(Deserialize)]
struct SavedSettings {
    color_starting: Color,
    color_clash: Color,
    color_highlight: Color,
    /// A file of an earlier version does not have it (PER-8).
    color_completed: Option<Color>,
    last_mode: u8,
    #[serde(default = "on")]
    highlight_on: bool,
    /// Earlier versions saved it as `auto_remove_on`.
    #[serde(default = "on", alias = "auto_remove_on")]
    auto_hide_on: bool,
    #[serde(default = "on")]
    completed_color_on: bool,
    #[serde(default)]
    micro_line: LineGray,
}

fn on() -> bool {
    true
}

/// The colors are all different, except White (COL-5).
fn colors_are_distinct(colors: &[Color]) -> bool {
    colors.iter().enumerate().all(|(i, &a)| a == Color::White || !colors[i + 1..].contains(&a))
}

/// `wanted`, or the first free color if another section uses it: the first color of the palette that
/// is not White and that is not in `used` (PER-8). `used` has at most three colors, thus a free color
/// is always there.
fn free_color(wanted: Color, used: &[Color]) -> Color {
    if !used.contains(&wanted) {
        return wanted;
    }
    Color::ALL
        .into_iter()
        .find(|c| *c != Color::White && !used.contains(c))
        .expect("the palette has more colors than the sections")
}

impl From<SavedSettings> for Settings {
    /// PER-8. This is a function of the file only, thus each start gives the same result.
    fn from(s: SavedSettings) -> Settings {
        let (color_highlight, color_completed, completed_color_on) = match s.color_completed {
            Some(completed) => (s.color_highlight, completed, s.completed_color_on),
            None => {
                if !colors_are_distinct(&[s.color_starting, s.color_clash, s.color_highlight]) {
                    return Settings::default();
                }
                let others = [s.color_starting, s.color_clash];
                let highlight = if s.color_highlight == Color::Green { free_color(Color::Cyan, &others) } else { s.color_highlight };
                (highlight, free_color(Color::Green, &[s.color_starting, s.color_clash, highlight]), true)
            }
        };
        Settings {
            color_starting: s.color_starting,
            color_clash: s.color_clash,
            color_highlight,
            color_completed,
            last_mode: s.last_mode,
            highlight_on: s.highlight_on,
            auto_hide_on: s.auto_hide_on,
            completed_color_on,
            micro_line: s.micro_line,
        }
    }
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            color_starting: Color::Gray,
            color_clash: Color::Red,
            color_highlight: Color::Cyan,
            color_completed: Color::Green,
            last_mode: FIRST_MODE,
            highlight_on: true,
            auto_hide_on: true,
            completed_color_on: true,
            micro_line: LineGray::default(),
        }
    }
}

impl Settings {
    pub fn color(&self, s: Section) -> Color {
        match s {
            Section::Starting => self.color_starting,
            Section::Clash => self.color_clash,
            Section::Highlight => self.color_highlight,
            Section::Completed => self.color_completed,
        }
    }

    /// The color is not available for section `s`, because a different section uses it (COL-5).
    /// White is always available.
    pub fn is_taken(&self, s: Section, color: Color) -> bool {
        color != Color::White && Section::ALL.iter().any(|&o| o != s && self.color(o) == color)
    }

    /// The color of the cells of the completed groups of the game, or None if the completed color does
    /// not show (COL-11, COL-12). The set of completed groups does not depend on this value.
    pub fn completed_fill(&self, g: &Game) -> Option<Color> {
        (self.completed_color_on && self.color_completed != Color::White && !g.completed).then_some(self.color_completed)
    }

    /// INVARIANTS.md §0, `shows_completed`. It examines the full board: to draw the board, use
    /// `completed_fill` and `Game::board_sets` one time for all cells.
    pub fn shows_completed(&self, g: &Game, cell: Cell) -> bool {
        self.completed_fill(g).is_some() && g.completed_cells().contains(cell)
    }

    pub fn is_on(&self, s: Switch) -> bool {
        match s {
            Switch::HighlightNumbers => self.highlight_on,
            Switch::HideMarksAutomatically => self.auto_hide_on,
            Switch::ColorCompletedGroups => self.completed_color_on,
        }
    }

    pub fn toggle(&mut self, s: Switch) {
        let on = match s {
            Switch::HighlightNumbers => &mut self.highlight_on,
            Switch::HideMarksAutomatically => &mut self.auto_hide_on,
            Switch::ColorCompletedGroups => &mut self.completed_color_on,
        };
        *on = !*on;
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
            Section::Completed => self.color_completed = color,
        }
        true
    }

    /// The settings agree with COL-5 and DIF-1.
    pub fn is_valid(&self) -> bool {
        colors_are_distinct(&Section::ALL.map(|s| self.color(s))) && self.last_mode < MODES
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_panel_positions_agree_with_the_order_of_all() {
        for (i, s) in Section::ALL.into_iter().enumerate() {
            assert_eq!(s.panel_index(), i);
        }
        for (i, c) in Color::ALL.into_iter().enumerate() {
            assert_eq!(c.grid_index(), i);
        }
    }

    #[test]
    fn defaults() {
        let s = Settings::default();
        // COL-1
        assert_eq!(
            (s.color_starting, s.color_clash, s.color_highlight, s.color_completed),
            (Color::Gray, Color::Red, Color::Cyan, Color::Green)
        );
        assert!(s.highlight_on && s.auto_hide_on && s.completed_color_on);
        assert_eq!(s.last_mode, 2);
        assert!(s.is_valid());
    }

    #[test]
    fn a_settings_file_without_the_switches_has_them_on() {
        // PER-3 for the settings files of an earlier version.
        let s: Settings =
            serde_json::from_str(r#"{"color_starting":"Gray","color_clash":"Red","color_highlight":"Green","last_mode":3}"#).unwrap();
        assert!(s.highlight_on && s.auto_hide_on && s.completed_color_on);
        assert_eq!(s.last_mode, 3);
    }

    #[test]
    fn the_old_name_of_the_hiding_switch_is_read() {
        // PER-3: earlier versions saved the second switch as `auto_remove_on`.
        let s: Settings = serde_json::from_str(
            r#"{"color_starting":"Gray","color_clash":"Red","color_highlight":"Green","last_mode":3,"auto_remove_on":false}"#,
        )
        .unwrap();
        assert!(s.highlight_on && !s.auto_hide_on);
        assert!(serde_json::to_string(&s).unwrap().contains("\"auto_hide_on\":false"));
    }

    #[test]
    fn an_earlier_settings_file_gets_the_default_line_gray() {
        // COL-7: a settings file of an earlier version has no `micro_line`: gray90.
        let s: Settings = serde_json::from_str(
            r#"{"color_starting":"Gray","color_clash":"Red","color_highlight":"Green","last_mode":3}"#,
        )
        .unwrap();
        assert_eq!(s.micro_line, LineGray::Gray90);
        assert_eq!(LineGray::default().percent(), Some(90));
        let s = Settings { micro_line: LineGray::None, ..s };
        let back: Settings = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(back.micro_line, LineGray::None);
    }

    #[test]
    fn colors_stay_different_except_white() {
        let mut s = Settings::default();
        assert!(!s.select(Section::Starting, Color::Red));
        assert!(s.select(Section::Starting, Color::White));
        assert!(s.select(Section::Clash, Color::White));
        assert!(s.select(Section::Highlight, Color::White));
        assert!(s.select(Section::Completed, Color::White));
        assert!(s.is_valid());
        assert!(s.select(Section::Clash, Color::Gray));
        assert!(!s.select(Section::Starting, Color::Gray));
        assert!(!s.select(Section::Completed, Color::Gray));
    }

    fn old_file(starting: &str, clash: &str, highlight: &str) -> Settings {
        let json = format!(r#"{{"color_starting":"{starting}","color_clash":"{clash}","color_highlight":"{highlight}","last_mode":3}}"#);
        serde_json::from_str(&json).unwrap()
    }

    fn colors(s: &Settings) -> [Color; 4] {
        Section::ALL.map(|x| s.color(x))
    }

    #[test]
    fn an_earlier_settings_file_gets_the_new_colors() {
        // PER-8, parts 2 to 4.
        use Color::*;
        let cases = [
            // A Green highlight becomes Cyan, and the completed color is Green.
            (("Gray", "Red", "Green"), [Gray, Red, Cyan, Green]),
            // Cyan is in use: the first free color of the palette.
            (("Cyan", "Red", "Green"), [Cyan, Red, Gray, Green]),
            (("Gray", "Cyan", "Green"), [Gray, Cyan, Yellow, Green]),
            // Another section has Green: the first free color for the completed groups.
            (("Green", "Red", "Cyan"), [Green, Red, Cyan, Gray]),
            (("Gray", "Green", "Cyan"), [Gray, Green, Cyan, Yellow]),
            // A highlight of another color does not change, also White.
            (("Gray", "Red", "White"), [Gray, Red, White, Green]),
            (("Gray", "Red", "Blue"), [Gray, Red, Blue, Green]),
            // White is never a free color.
            (("White", "White", "White"), [White, White, White, Green]),
            (("White", "White", "Green"), [White, White, Cyan, Green]),
        ];
        for ((a, b, c), want) in cases {
            let s = old_file(a, b, c);
            assert_eq!(colors(&s), want, "{a} {b} {c}");
            assert!(s.is_valid(), "{a} {b} {c}");
            assert!(s.completed_color_on);
            assert_eq!(s.last_mode, 3);
        }
    }

    #[test]
    fn an_earlier_settings_file_that_breaks_col_5_gets_the_defaults() {
        // PER-8, part 1.
        assert_eq!(old_file("Red", "Red", "Blue"), Settings::default());
        assert_eq!(old_file("Green", "Red", "Green"), Settings::default());
    }

    #[test]
    fn a_file_with_a_completed_color_does_not_change() {
        // PER-8: a Green highlight stays, here with a different completed color.
        let json = r#"{"color_starting":"Gray","color_clash":"Red","color_highlight":"Green","color_completed":"Blue","last_mode":3}"#;
        let s: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(colors(&s), [Color::Gray, Color::Red, Color::Green, Color::Blue]);
        assert!(s.completed_color_on, "no value for the switch: on");
        // A file that breaks COL-5 stays invalid: `App::new` then uses the defaults (PER-5).
        let json = r#"{"color_starting":"Gray","color_clash":"Red","color_highlight":"Green","color_completed":"Green","last_mode":3}"#;
        assert!(!serde_json::from_str::<Settings>(json).unwrap().is_valid());
    }

    #[test]
    fn the_new_saved_values_go_through_the_file() {
        // PER-3
        let s = Settings { color_completed: Color::Orange, completed_color_on: false, color_highlight: Color::Green, ..Settings::default() };
        let text = serde_json::to_string(&s).unwrap();
        assert!(text.contains(r#""color_completed":"Orange""#) && text.contains(r#""completed_color_on":false"#), "{text}");
        assert_eq!(serde_json::from_str::<Settings>(&text).unwrap(), s);
    }

    #[test]
    fn the_completed_fill_shows_only_with_the_switch_on_and_a_color_on_a_game_in_progress() {
        // COL-11, COL-12
        let mut blank = [[0u8; 9]; 9];
        blank[0][0] = 1;
        let g = Game::new(1, 2, [[1; 9]; 9], blank);
        let s = Settings::default();
        assert_eq!(s.completed_fill(&g), Some(Color::Green));
        assert_eq!(Settings { completed_color_on: false, ..s.clone() }.completed_fill(&g), None);
        assert_eq!(Settings { color_completed: Color::White, ..s.clone() }.completed_fill(&g), None);
        assert_eq!(s.completed_fill(&Game { completed: true, ..g }), None);
    }
}
