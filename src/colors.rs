//! The color picker (the program `sudoku-colors`): a development app to select the colors of the
//! palette on the display of the tablet. Do not use it for a release.
//!
//! - It is a separate app on the tablet, `Sudoku (colors)` in the AppLoad folder `sudoku-colors`
//!   (`make install-colors`). Thus its installation does not change the usual Sudoku app.
//! - The screen has one column of three preview cells for each palette color: an empty cell, a cell
//!   with a number of the player and a cell with a starting number.
//! - The buttons select the color that the R, G and B sliders change.
//! - The app keeps the colors between starts (`save_path`). An agent reads this file to change the
//!   colors of the usual app. The reset button sets the colors of the code again.
//!
//! This file has no input or output. `platform/colors.rs` is the main loop on the tablet.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::font::{Anchor, Weight};
use crate::layout::{close_cx, CELL, CLOSE_CY};
use crate::render::{Canvas, Icon, Rgb};
use crate::settings::{Color, BLACK, ENTRY_DIGIT};

/// The file of the saved colors. The folder is separate from the data of the usual app.
pub fn save_path() -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/home/root"));
    home.join(".local/share/rm-sudoku-colors/colors.json")
}

const WHITE: Rgb = [255, 255, 255];
const TRACK: Rgb = [153, 153, 153];
const COLUMNS: usize = Color::ALL.len();
const ROWS: i32 = 3;

const LEFT: f32 = 148.0;
const RIGHT: f32 = 1471.0;
const TRACK_L: f32 = 220.0;
const TRACK_R: f32 = 1370.0;
const KNOB_R: f32 = 24.0;
const GRID_Y: f32 = 418.0;
const GRID_BORDER: f32 = 6.5;
const BUTTON_Y: f32 = GRID_Y - 106.5;
const BUTTON_H: f32 = 100.0;
const NUMBER_BUTTON_Y: f32 = GRID_Y + (ROWS * CELL) as f32 + 40.0;
const NUMBER_BUTTON_W: f32 = 2.0 * CELL as f32;
const TITLE_Y: f32 = 1140.0;
const FIRST_SLIDER_Y: f32 = TITLE_Y + 120.0;
const SLIDER_STEP: f32 = 150.0;
/// The hit area of the close button: one toolbar slot.
const CLOSE_SLOT: f32 = 112.0;

/// A rectangle of the screen: x, y, width, height.
pub type Area = (i32, i32, i32, i32);
const SCREEN_W: i32 = crate::layout::PHYSICAL_W as i32;
/// The row of the color buttons.
pub const BUTTONS_AREA: Area = (0, BUTTON_Y as i32 - 2, SCREEN_W, BUTTON_H as i32 + 2);
/// The button "Your number", the title and the sliders.
pub const LOWER_AREA: Area = (0, NUMBER_BUTTON_Y as i32 - 2, SCREEN_W, 720);
/// The title with the hex code and the sliders.
pub const SLIDERS_AREA: Area = (0, 1100, SCREEN_W, 510);

/// The colors that the player can change. They are saved.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Colors {
    /// The cell colors, in the order of `Color::ALL`.
    pub palette: [Rgb; COLUMNS],
    /// The color of the numbers of the player.
    pub number: Rgb,
}

impl Default for Colors {
    fn default() -> Colors {
        Colors { palette: Color::ALL.map(Color::rgb), number: ENTRY_DIGIT }
    }
}

/// The color that the sliders change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// An index into `Colors::palette`.
    Palette(usize),
    Number,
}

impl Target {
    /// The preview cells that use this color, in the inside of the thick border: a column for a
    /// palette color, the row "Yours" for the color of the numbers. A refresh changes only this area.
    pub fn preview_area(self) -> Area {
        // The first and the last pixel of the area include the partly covered edge pixels of the lines.
        let (s, first, last) = (CELL, GRID_BORDER as i32 + 1, GRID_BORDER as i32);
        let (left, top) = (LEFT as i32, GRID_Y as i32);
        match self {
            Target::Palette(i) => {
                let (i, end) = (i as i32, COLUMNS as i32 - 1);
                let x0 = left + i * s + if i == 0 { first } else { 1 };
                let x1 = left + (i + 1) * s - if i == end { last } else { 1 };
                (x0, top + first, x1 - x0, ROWS * s - first - last)
            }
            Target::Number => (left + first, top + s + 1, COLUMNS as i32 * s - first - last, s - 2),
        }
    }

    /// The button of this color.
    pub fn button_area(self) -> Area {
        match self {
            Target::Palette(i) => (LEFT as i32 + i as i32 * CELL, BUTTON_Y as i32, CELL, BUTTON_H as i32 + 2),
            Target::Number => (LEFT as i32, NUMBER_BUTTON_Y as i32, NUMBER_BUTTON_W as i32, BUTTON_H as i32 + 2),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Target::Palette(i) => Color::ALL[i].name(),
            Target::Number => "Your number",
        }
    }
}

fn hex(rgb: Rgb) -> String {
    format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2])
}

/// The result of an input, for the main loop.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
    None,
    /// The selected color changed. The preview cells do not change.
    Selection,
    /// A slider moved and the pen or the finger stays down.
    Slider,
    /// These colors are not the same as before: the pen or the finger lifted from a slider, or the
    /// player selected the reset button.
    Refresh(Vec<Target>),
    /// The player selected the close button.
    Quit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Gesture {
    Close,
    Reset,
    Slider { channel: usize, start: u8 },
}

/// The state of the screen.
#[derive(Debug)]
pub struct Picker {
    pub colors: Colors,
    pub selected: Target,
    /// The pointer that is down (the pen or one touch point) and what it does.
    active: Option<(i32, Gesture)>,
}

fn in_close(x: f32, y: f32) -> bool {
    x < CLOSE_SLOT && y < CLOSE_SLOT
}

fn in_reset(x: f32, y: f32) -> bool {
    (RIGHT - NUMBER_BUTTON_W..RIGHT).contains(&x) && (NUMBER_BUTTON_Y..NUMBER_BUTTON_Y + BUTTON_H).contains(&y)
}

fn slider_y(channel: usize) -> f32 {
    FIRST_SLIDER_Y + SLIDER_STEP * channel as f32
}

/// The channel of the slider at the position. The hit area is larger than the track.
fn slider_at(x: f32, y: f32) -> Option<usize> {
    if !(TRACK_L - 2.0 * KNOB_R..=TRACK_R + 2.0 * KNOB_R).contains(&x) {
        return None;
    }
    (0..3).find(|&channel| (y - slider_y(channel)).abs() <= SLIDER_STEP / 2.0)
}

fn slider_value(x: f32) -> u8 {
    (((x - TRACK_L) / (TRACK_R - TRACK_L)).clamp(0.0, 1.0) * 255.0).round() as u8
}

fn button_at(x: f32, y: f32) -> Option<Target> {
    let s = CELL as f32;
    if (BUTTON_Y..BUTTON_Y + BUTTON_H).contains(&y) && (LEFT..LEFT + s * COLUMNS as f32).contains(&x) {
        return Some(Target::Palette(((x - LEFT) / s) as usize));
    }
    if (NUMBER_BUTTON_Y..NUMBER_BUTTON_Y + BUTTON_H).contains(&y) && (LEFT..LEFT + NUMBER_BUTTON_W).contains(&x) {
        return Some(Target::Number);
    }
    None
}

impl Picker {
    pub fn new(colors: Colors) -> Picker {
        let selected = Target::Palette(Color::ALL.iter().position(|c| *c == Color::Cyan).unwrap_or(0));
        Picker { colors, selected, active: None }
    }

    fn selected_rgb(&mut self) -> &mut Rgb {
        match self.selected {
            Target::Palette(i) => &mut self.colors.palette[i],
            Target::Number => &mut self.colors.number,
        }
    }

    fn set_channel(&mut self, channel: usize, x: f32) -> bool {
        let value = slider_value(x);
        let rgb = self.selected_rgb();
        let changed = rgb[channel] != value;
        rgb[channel] = value;
        changed
    }

    /// Sets the colors of the code again. Only the colors that are different need a refresh.
    fn reset(&mut self) -> Change {
        let defaults = Colors::default();
        let mut changed: Vec<Target> =
            (0..COLUMNS).filter(|&i| self.colors.palette[i] != defaults.palette[i]).map(Target::Palette).collect();
        if self.colors.number != defaults.number {
            changed.push(Target::Number);
        }
        self.colors = defaults;
        if changed.is_empty() {
            Change::None
        } else {
            Change::Refresh(changed)
        }
    }

    /// A pen or a touch point goes down. `pointer` identifies it until it lifts.
    pub fn press(&mut self, pointer: i32, x: f32, y: f32) -> Change {
        if self.active.is_some() {
            return Change::None;
        }
        if in_close(x, y) {
            self.active = Some((pointer, Gesture::Close));
            return Change::None;
        }
        if in_reset(x, y) {
            self.active = Some((pointer, Gesture::Reset));
            return Change::None;
        }
        if let Some(channel) = slider_at(x, y) {
            let start = self.selected_rgb()[channel];
            self.active = Some((pointer, Gesture::Slider { channel, start }));
            return if self.set_channel(channel, x) { Change::Slider } else { Change::None };
        }
        match button_at(x, y) {
            Some(target) if target != self.selected => {
                self.selected = target;
                Change::Selection
            }
            _ => Change::None,
        }
    }

    pub fn moved(&mut self, pointer: i32, x: f32) -> Change {
        let Some((p, Gesture::Slider { channel, .. })) = self.active else { return Change::None };
        if p == pointer && self.set_channel(channel, x) {
            Change::Slider
        } else {
            Change::None
        }
    }

    pub fn release(&mut self, pointer: i32, x: f32, y: f32) -> Change {
        let Some((p, gesture)) = self.active else { return Change::None };
        if p != pointer {
            return Change::None;
        }
        self.active = None;
        match gesture {
            Gesture::Close if in_close(x, y) => Change::Quit,
            Gesture::Close => Change::None,
            Gesture::Reset if in_reset(x, y) => self.reset(),
            Gesture::Reset => Change::None,
            Gesture::Slider { channel, start } => {
                self.set_channel(channel, x);
                if self.selected_rgb()[channel] != start {
                    Change::Refresh(vec![self.selected])
                } else {
                    Change::Slider
                }
            }
        }
    }
}

/// A button that selects a color: a dot with the current color and the name.
fn button(c: &mut Canvas, x0: f32, x1: f32, y: f32, name: &str, rgb: Rgb, selected: bool) {
    let cx = (x0 + x1) / 2.0;
    c.rect(x0, y, x1, y + BUTTON_H, BLACK);
    if !selected {
        c.rect(x0 + 2.0, y + 2.0, x1 - 2.0, y + BUTTON_H - 2.0, WHITE);
    }
    let fg = if selected { WHITE } else { BLACK };
    c.circle(cx, y + 35.0, 19.0, Some(rgb), Some((fg, 2.0)));
    c.text(Weight::Medium, 25.0, cx, y + 81.0, Anchor::Middle, name, fg);
}

fn grid(c: &mut Canvas, colors: &Colors) {
    let s = CELL as f32;
    for (col, rgb) in colors.palette.iter().enumerate() {
        let x = LEFT + col as f32 * s;
        for (row, digit) in [None, Some(colors.number), Some(BLACK)].into_iter().enumerate() {
            let y = GRID_Y + row as f32 * s;
            c.rect(x, y, x + s, y + s, *rgb);
            if let Some(digit) = digit {
                c.text(Weight::Regular, 104.0, x + s / 2.0, y + 111.0, Anchor::Middle, "5", digit);
            }
        }
    }
    let (w, h) = (COLUMNS as f32 * s, ROWS as f32 * s);
    for i in 1..COLUMNS {
        let x = LEFT + i as f32 * s;
        c.line(x, GRID_Y, x, GRID_Y + h, 3.0, BLACK);
    }
    for i in 1..ROWS {
        let y = GRID_Y + i as f32 * s;
        c.line(LEFT, y, LEFT + w, y, 3.0, BLACK);
    }
    let (o, x1, y1) = (GRID_BORDER, LEFT + w, GRID_Y + h);
    c.rect(LEFT - o, GRID_Y - o, LEFT + o, y1 + o, BLACK);
    c.rect(x1 - o, GRID_Y - o, x1 + o, y1 + o, BLACK);
    c.rect(LEFT - o, GRID_Y - o, x1 + o, GRID_Y + o, BLACK);
    c.rect(LEFT - o, y1 - o, x1 + o, y1 + o, BLACK);
    for (row, label) in ["Cell", "Yours", "Puzzle"].iter().enumerate() {
        let y = GRID_Y + row as f32 * s + s / 2.0 + 9.0;
        c.text(Weight::Medium, 25.0, LEFT - 20.0, y, Anchor::End, label, BLACK);
    }
}

fn slider(c: &mut Canvas, y: f32, label: &str, value: u8) {
    let k = TRACK_L + (TRACK_R - TRACK_L) * value as f32 / 255.0;
    c.text(Weight::Medium, 25.0, LEFT, y + 9.0, Anchor::Start, label, BLACK);
    c.line(k, y, TRACK_R, y, 3.0, TRACK);
    c.line(TRACK_L, y, k, y, 6.0, BLACK);
    c.circle(k, y, KNOB_R, Some(BLACK), Some((WHITE, 3.0)));
    c.text(Weight::Medium, 25.0, RIGHT, y + 9.0, Anchor::End, &value.to_string(), BLACK);
}

/// Draws the full screen. The canvas must be white.
pub fn draw(c: &mut Canvas, picker: &Picker) {
    let (colors, s) = (&picker.colors, CELL as f32);
    Icon::Close.draw(c, close_cx(), CLOSE_CY, BLACK);
    for (i, color) in Color::ALL.iter().enumerate() {
        let x = LEFT + i as f32 * s;
        button(c, x, x + s, BUTTON_Y, color.name(), colors.palette[i], picker.selected == Target::Palette(i));
    }
    grid(c, colors);
    button(c, LEFT, LEFT + NUMBER_BUTTON_W, NUMBER_BUTTON_Y, "Your number", colors.number, picker.selected == Target::Number);
    c.rect(RIGHT - NUMBER_BUTTON_W, NUMBER_BUTTON_Y, RIGHT, NUMBER_BUTTON_Y + BUTTON_H, BLACK);
    c.rect(RIGHT - NUMBER_BUTTON_W + 2.0, NUMBER_BUTTON_Y + 2.0, RIGHT - 2.0, NUMBER_BUTTON_Y + BUTTON_H - 2.0, WHITE);
    c.text(Weight::Medium, 25.0, RIGHT - NUMBER_BUTTON_W / 2.0, NUMBER_BUTTON_Y + 59.0, Anchor::Middle, "Reset colors", BLACK);
    let rgb = match picker.selected {
        Target::Palette(i) => colors.palette[i],
        Target::Number => colors.number,
    };
    c.text(Weight::Medium, 25.0, LEFT, TITLE_Y, Anchor::Start, picker.selected.name(), BLACK);
    c.text(Weight::Medium, 25.0, RIGHT, TITLE_Y, Anchor::End, &hex(rgb), BLACK);
    c.line(LEFT, TITLE_Y + 28.0, RIGHT, TITLE_Y + 28.0, 2.0, BLACK);
    for (channel, label) in ["R", "G", "B"].iter().enumerate() {
        slider(c, slider_y(channel), label, rgb[channel]);
    }
}
