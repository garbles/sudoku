//! The event scripts of the video tool and the bench (`video/*.txt`). A script gives timed pen, eraser
//! and finger events. `src/bin/video.rs` gives the commands of the script.

use crate::app::{new_game, App, Env, Input, Menu, Tool};
use crate::font::Fonts;
use crate::game::{mark_bit, Cell, Marks};
use crate::layout;
use crate::render;
use crate::settings::{Color, Section};
use crate::store::ResultsDb;

/// The time that the pen stays in the air at a point before it touches the screen and after it lifts.
const HOVER_MS: u64 = 250;
/// The speed of the pen in the air and while it writes (px for each ms).
const AIR_SPEED: f32 = 2.0;
const INK_SPEED: f32 = 0.6;
/// The time between two pen or eraser events while the pen moves (the tablet sends more).
const STEP_MS: u64 = 8;
const TAP_MS: u64 = 80;
/// The time between the taps of one `mark` line.
const MARK_GAP_MS: u64 = 300;
/// The pen lifts for this time between the strokes of one digit (less than the recognition pause).
const STROKE_GAP_MS: u64 = 150;
const FINGER_MS: u64 = 120;
/// The size of the handwriting in a cell (the same as the tests).
const INK_BOX: f32 = 98.0;

/// A fixed clock and seed, thus each run is the same. The scripts and the reference states use it.
pub struct FixedEnv(pub u64);

impl Env for FixedEnv {
    fn unix_time(&mut self) -> i64 {
        1_792_900_000
    }
    fn new_seed(&mut self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Ev {
    Input(Input),
    /// The pen is in the air above this point, or it is away (None).
    Hover(Option<(f32, f32)>, Tool),
}

/// What the script expects after an event. The tool stops with an error if the app does not agree.
#[derive(Clone, Copy, Debug)]
pub enum Check {
    Marks(Cell, Marks),
    Value(Cell, u8),
    Color(Section, Color),
}

pub struct Timeline {
    pub t: u64,
    pub events: Vec<(u64, Ev)>,
    pub checks: Vec<(u64, usize, Check)>,
    /// The pen point in the air (None when the pen is away).
    pen_at: Option<(f32, f32)>,
}

impl Timeline {
    pub fn push(&mut self, ev: Ev) {
        self.events.push((self.t, ev));
    }

    /// Moves the pen in the air to (x, y), and keeps it there for HOVER_MS.
    fn approach(&mut self, x: f32, y: f32, tool: Tool) {
        if let Some(from) = self.pen_at {
            for p in walk(&[from, (x, y)], AIR_SPEED) {
                self.t += STEP_MS;
                self.push(Ev::Hover(Some(p), tool));
            }
        }
        self.push(Ev::Hover(Some((x, y)), tool));
        self.t += HOVER_MS;
        self.pen_at = Some((x, y));
    }

    pub fn leave(&mut self, tool: Tool) {
        self.t += HOVER_MS;
        self.push(Ev::Hover(None, tool));
        self.pen_at = None;
    }

    /// One stroke of the pen or the eraser along the points.
    pub fn stroke(&mut self, pts: &[(f32, f32)], tool: Tool, speed: f32) {
        let (x, y) = pts[0];
        self.push(Ev::Input(Input::PenDown { x, y, tool }));
        let mut last = (x, y);
        for p in walk(pts, speed) {
            self.t += STEP_MS;
            self.push(Ev::Input(Input::PenMove { x: p.0, y: p.1 }));
            last = p;
        }
        self.t += STEP_MS;
        self.push(Ev::Input(Input::PenUp { x: last.0, y: last.1 }));
        self.pen_at = Some(last);
    }

    pub fn check(&mut self, delay: u64, c: Check) {
        self.checks.push((self.t + delay, self.checks.len(), c));
    }
}

/// The points along a polyline at the speed, one for each STEP_MS (the first point is not included).
fn walk(pts: &[(f32, f32)], speed: f32) -> Vec<(f32, f32)> {
    let step = speed * STEP_MS as f32;
    let mut out = Vec::new();
    let mut carry = 0.0;
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
        let mut d = step - carry;
        while d <= len {
            let t = d / len;
            out.push((a.0 + t * (b.0 - a.0), a.1 + t * (b.1 - a.1)));
            d += step;
        }
        carry = len - (d - step);
    }
    if let (Some(&l), Some(&e)) = (pts.last(), out.last()) {
        if l != e {
            out.push(l);
        }
    } else if pts.len() > 1 {
        out.push(pts[pts.len() - 1]);
    }
    out
}

fn parse_cell(s: &str) -> Result<Cell, String> {
    let b = s.as_bytes();
    if b.len() == 4 && b[0] == b'r' && b[2] == b'c' && (b'1'..=b'9').contains(&b[1]) && (b'1'..=b'9').contains(&b[3]) {
        Ok((b[1] - b'1', b[3] - b'1'))
    } else {
        Err(format!("incorrect cell {s:?} (use rRcC, for example r3c4)"))
    }
}

fn parse_digit(s: &str) -> Result<u8, String> {
    match s.parse::<u8>() {
        Ok(d @ 1..=9) => Ok(d),
        _ => Err(format!("incorrect digit {s:?}")),
    }
}

pub fn micro_center(cell: Cell, d: u8) -> (f32, f32) {
    let (x, y) = layout::cell_origin(cell);
    let (ix, iy) = (((d - 1) % 3) as i32, ((d - 1) / 3) as i32);
    let mid = |k: i32| (layout::micro_offset(k) + layout::micro_offset(k + 1)) as f32 / 2.0;
    (x as f32 + mid(ix), y as f32 + mid(iy))
}

fn button_point(name: &str) -> Result<(f32, f32), String> {
    use layout::Button;
    let button = match name {
        "menu" => Button::Menu,
        "colors" => Button::Colors,
        "settings" => Button::Settings,
        "undo" => Button::Undo,
        "redo" => Button::Redo,
        // The New game button (at the bottom of the open menu).
        "newgame" => Button::NewGame,
        // "Start new game" in the New game panel.
        "start" => return Ok(((layout::ng_tx() + 80) as f32, layout::ng_start_y() as f32)),
        _ => return Err(format!("unknown button {name:?}")),
    };
    Ok(layout::icon_center(button))
}

pub struct Script {
    pub fps: u64,
    pub seed: u64,
    pub mode: u8,
    pub timeline: Timeline,
}

impl Script {
    /// The app at the start of the script: the puzzle of the script (not symmetric), the settings of
    /// a player whose last new game had the mode and the symmetry of the puzzle (thus the slider is
    /// at that mode and the switch is off, DIF-3), no results and a fixed clock.
    pub fn app(&self, fonts: &Fonts) -> App {
        let settings = crate::settings::Settings { last_mode: self.mode, last_symmetric: false, ..Default::default() };
        let app = App::new(Some(new_game(self.seed, self.mode)), Some(settings), ResultsDb::in_memory(), Box::new(FixedEnv(self.seed)), render::ng_stops(fonts));
        let mut app = app;
        // The real app opens with the menu open (PER-2). The scripts start with the menu closed:
        // they show the empty board first, and then they open the menu with a tap. Thus the
        // videos and the output hashes of the bench do not change (DECISIONS.md).
        app.ui.menu = Menu::Closed;
        app
    }
}

pub fn parse(text: &str, pause_ms: u64) -> Result<Script, String> {
    let mut fps = 30;
    let mut puzzle = None;
    layout::set_right_handed(true);
    let mut tl = Timeline { t: 0, events: Vec::new(), checks: Vec::new(), pen_at: None };
    // The pencil marks that the script made, to know the expected values.
    let mut marks = [[0u16; 9]; 9];
    for (n, raw) in text.lines().enumerate() {
        let line = raw.split('#').next().unwrap().trim();
        let w: Vec<&str> = line.split_whitespace().collect();
        if w.is_empty() {
            continue;
        }
        let err = |e: String| format!("line {}: {e}", n + 1);
        let need_puzzle = || if puzzle.is_none() { Err(err("the puzzle line must come first".into())) } else { Ok(()) };
        match w.as_slice() {
            ["fps", v] => fps = v.parse().map_err(|_| err(format!("incorrect fps {v:?}")))?,
            ["puzzle", "seed", s, "mode", m] => {
                let seed = s.parse().map_err(|_| err(format!("incorrect seed {s:?}")))?;
                let mode = m.parse().map_err(|_| err(format!("incorrect mode {m:?}")))?;
                puzzle = Some((seed, mode));
            }
            ["hand", h @ ("right" | "left")] => {
                if !tl.events.is_empty() {
                    return Err(err("the hand line must come before the first event".into()));
                }
                layout::set_right_handed(*h == "right");
            }
            ["wait", s] => {
                let secs: f64 = s.parse().map_err(|_| err(format!("incorrect time {s:?}")))?;
                tl.t += (secs * 1000.0).round() as u64;
            }
            ["mark", cell, digits @ ..] if !digits.is_empty() => {
                need_puzzle()?;
                let cell = parse_cell(cell).map_err(err)?;
                for (i, d) in digits.iter().enumerate() {
                    let d = parse_digit(d).map_err(err)?;
                    if i > 0 {
                        tl.t += MARK_GAP_MS - HOVER_MS.min(MARK_GAP_MS);
                    }
                    let (x, y) = micro_center(cell, d);
                    tl.approach(x, y, Tool::Pen);
                    tl.push(Ev::Input(Input::PenDown { x, y, tool: Tool::Pen }));
                    tl.t += TAP_MS;
                    tl.push(Ev::Input(Input::PenUp { x, y }));
                    marks[cell.0 as usize][cell.1 as usize] ^= mark_bit(d);
                }
                tl.check(0, Check::Marks(cell, marks[cell.0 as usize][cell.1 as usize]));
                tl.leave(Tool::Pen);
            }
            ["write", cell, d, rest @ ..] => {
                need_puzzle()?;
                let cell = parse_cell(cell).map_err(err)?;
                let d = parse_digit(d).map_err(err)?;
                let writer = match rest {
                    [] => 1,
                    ["writer", n] => n.parse().map_err(|_| err(format!("incorrect writer {n:?}")))?,
                    _ => return Err(err(format!("unknown line {line:?}"))),
                };
                let (x, y) = layout::cell_origin(cell);
                let off = (layout::cell_size() as f32 - INK_BOX) / 2.0;
                let ink = crate::recognizer::example_ink(d, writer, x as f32 + off, y as f32 + off, INK_BOX)
                    .ok_or_else(|| err(format!("no handwriting example of {d} by writer {writer}")))?;
                for (i, s) in ink.iter().enumerate() {
                    if i > 0 {
                        tl.t += STROKE_GAP_MS;
                        let (x, y) = s[0];
                        for p in walk(&[tl.pen_at.unwrap(), (x, y)], AIR_SPEED) {
                            tl.t += STEP_MS;
                            tl.push(Ev::Hover(Some(p), Tool::Pen));
                        }
                    } else {
                        tl.approach(s[0].0, s[0].1, Tool::Pen);
                    }
                    tl.stroke(s, Tool::Pen, INK_SPEED);
                    tl.push(Ev::Hover(tl.pen_at, Tool::Pen));
                }
                tl.check(pause_ms + 50, Check::Value(cell, d));
                tl.leave(Tool::Pen);
            }
            ["erase", cell] => {
                need_puzzle()?;
                let cell = parse_cell(cell).map_err(err)?;
                let (x, y) = layout::cell_origin(cell);
                let (cx, cy) = (x as f32 + layout::cell_size() as f32 / 2.0, y as f32 + layout::cell_size() as f32 / 2.0);
                let rub: Vec<(f32, f32)> =
                    (0..7).map(|i| (cx + if i % 2 == 0 { -40.0 } else { 40.0 }, cy - 30.0 + i as f32 * 10.0)).collect();
                tl.approach(rub[0].0, rub[0].1, Tool::Eraser);
                tl.stroke(&rub, Tool::Eraser, INK_SPEED * 1.5);
                tl.push(Ev::Hover(tl.pen_at, Tool::Eraser));
                tl.check(0, Check::Value(cell, 0));
                tl.leave(Tool::Eraser);
            }
            ["tap", "color", section, color] => {
                need_puzzle()?;
                let s = match *section {
                    "starting" => Section::Starting,
                    "clashing" => Section::Clash,
                    "highlighted" => Section::Highlight,
                    "completed" => Section::Completed,
                    _ => return Err(err(format!("unknown section {section:?}"))),
                };
                let c = Color::ALL
                    .into_iter()
                    .find(|c| c.name().eq_ignore_ascii_case(color))
                    .ok_or_else(|| err(format!("unknown color {color:?}")))?;
                let cell = layout::color_cell(s, c);
                finger(&mut tl, cell.x0 as f32 + 56.0, cell.y0 as f32 + 56.0);
                tl.check(0, Check::Color(s, c));
            }
            ["tap", name] => {
                need_puzzle()?;
                let (x, y) = button_point(name).map_err(err)?;
                finger(&mut tl, x, y);
            }
            _ => return Err(err(format!("unknown line {line:?}"))),
        }
    }
    let (seed, mode) = puzzle.ok_or("the script has no puzzle line")?;
    Ok(Script { fps, seed, mode, timeline: tl })
}

/// A finger tap. The pen goes away first, as a hand does.
pub fn finger(tl: &mut Timeline, x: f32, y: f32) {
    if tl.pen_at.is_some() {
        tl.leave(Tool::Pen);
    }
    tl.push(Ev::Input(Input::TouchDown { id: 1, x, y }));
    tl.t += FINGER_MS;
    tl.push(Ev::Input(Input::TouchUp { id: 1, x, y }));
}

pub fn verify(app: &App, c: Check) -> Result<(), String> {
    let ok = match c {
        Check::Marks(cell, m) => app.game.value_at(cell) == 0 && app.game.marks_at(cell) == m,
        Check::Value(cell, d) => app.game.value_at(cell) == d,
        Check::Color(s, color) => app.settings.color(s) == color,
    };
    if ok {
        Ok(())
    } else if let Check::Marks(cell, _) = c {
        let (v, m) = (app.game.value_at(cell), app.game.marks_at(cell));
        Err(format!("the app does not show the expected result: {c:?} (value {v}, marks {m})"))
    } else {
        Err(format!("the app does not show the expected result: {c:?}"))
    }
}
