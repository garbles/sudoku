//! Makes a video of the app from a script of timed events. Use it on a computer:
//!
//!     cargo run --release --bin video -- video/demo.txt video/demo.mp4
//!
//! The tool runs the real app with no screen. It sends the pen, eraser and finger events of the
//! script to the app, draws a frame at each step of the frame rate and gives the frames to ffmpeg.
//! The puzzle, the clock and the handwriting are fixed, thus each run gives the same video. Markers
//! show where the pen, the eraser and the finger touch the screen. The markers are not a part of the
//! app.
//!
//! Script lines (a `#` starts a comment). Cells are rRcC, with rows and columns from 1 to 9:
//!
//!     fps N                       frames each second (default 30)
//!     puzzle seed S mode M        the puzzle (it must come before the first event)
//!     wait S                      no input for S seconds
//!     mark rRcC D...              pen taps on the micro-cells of the digits D
//!     write rRcC D                pen writes the digit D (handwriting of the recognizer examples)
//!     erase rRcC                  eraser end rubs the cell
//!     tap menu|colors|settings|undo|redo
//!     tap color starting|clashing|highlighted COLOR
//!
//! It uses the fonts in scripts/fonts (refer to scripts/fonts/README.md) and ffmpeg. An output file
//! that ends with .gif gives a GIF at a smaller size.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use sudoku::app::{new_game, App, Env, Input, Tool};
use sudoku::font::Fonts;
use sudoku::game::{mark_bit, Cell, Marks};
use sudoku::layout::{self, CELL};
use sudoku::render::{self, Canvas};
use sudoku::settings::{Color, Section};
use sudoku::store::ResultsDb;
use tiny_skia::{FillRule, LineCap, Paint, PathBuilder, Pixmap, Stroke, Transform};

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
const FINGER_FADE_MS: u64 = 400;
const TRAIL_MS: u64 = 200;
/// The size of the handwriting in a cell (the same as the tests).
const INK_BOX: f32 = 98.0;
const ERASER_R: f32 = 30.0;

struct FixedEnv(u64);

impl Env for FixedEnv {
    fn unix_time(&mut self) -> i64 {
        1_792_900_000
    }
    fn new_seed(&mut self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug)]
enum Ev {
    Input(Input),
    /// The pen is in the air above this point, or it is away (None).
    Hover(Option<(f32, f32)>, Tool),
}

/// What the script expects after an event. The tool stops with an error if the app does not agree.
#[derive(Clone, Copy, Debug)]
enum Check {
    Marks(Cell, Marks),
    Value(Cell, u8),
    Color(Section, Color),
}

struct Timeline {
    t: u64,
    events: Vec<(u64, Ev)>,
    checks: Vec<(u64, usize, Check)>,
    /// The pen point in the air (None when the pen is away).
    pen_at: Option<(f32, f32)>,
}

impl Timeline {
    fn push(&mut self, ev: Ev) {
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

    fn leave(&mut self, tool: Tool) {
        self.t += HOVER_MS;
        self.push(Ev::Hover(None, tool));
        self.pen_at = None;
    }

    /// One stroke of the pen or the eraser along the points.
    fn stroke(&mut self, pts: &[(f32, f32)], tool: Tool, speed: f32) {
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

    fn check(&mut self, delay: u64, c: Check) {
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

fn micro_center(cell: Cell, d: u8) -> (f32, f32) {
    let (x, y) = layout::cell_origin(cell);
    let (ix, iy) = (((d - 1) % 3) as i32, ((d - 1) / 3) as i32);
    let mid = |k: i32| (layout::micro_offset(k) + layout::micro_offset(k + 1)) as f32 / 2.0;
    (x as f32 + mid(ix), y as f32 + mid(iy))
}

fn button_point(name: &str) -> Result<(f32, f32), String> {
    let slot = match name {
        "menu" => 0,
        "colors" => 1,
        "settings" => 2,
        "undo" => 3,
        "redo" => 4,
        _ => return Err(format!("unknown button {name:?}")),
    };
    Ok((layout::tb_cx(), layout::tb_cy(slot)))
}

struct Script {
    fps: u64,
    seed: u64,
    mode: u8,
    timeline: Timeline,
}

fn parse(text: &str, pause_ms: u64) -> Result<Script, String> {
    let mut fps = 30;
    let mut puzzle = None;
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
            ["write", cell, d] => {
                need_puzzle()?;
                let cell = parse_cell(cell).map_err(err)?;
                let d = parse_digit(d).map_err(err)?;
                let (x, y) = layout::cell_origin(cell);
                let off = (CELL as f32 - INK_BOX) / 2.0;
                let ink = sudoku::recognizer::example_ink(d, 1, x as f32 + off, y as f32 + off, INK_BOX)
                    .ok_or_else(|| err(format!("no handwriting example for {d}")))?;
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
                let (cx, cy) = (x as f32 + CELL as f32 / 2.0, y as f32 + CELL as f32 / 2.0);
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
                    _ => return Err(err(format!("unknown section {section:?}"))),
                };
                let ci = Color::ALL
                    .iter()
                    .position(|c| c.name().eq_ignore_ascii_case(color))
                    .ok_or_else(|| err(format!("unknown color {color:?}")))?;
                let si = Section::ALL.iter().position(|&x| x == s).unwrap();
                let (gx, gy) = layout::color_cell(si, ci);
                finger(&mut tl, gx as f32 + 56.0, gy as f32 + 56.0);
                tl.check(0, Check::Color(s, Color::ALL[ci]));
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
fn finger(tl: &mut Timeline, x: f32, y: f32) {
    if tl.pen_at.is_some() {
        tl.leave(Tool::Pen);
    }
    tl.push(Ev::Input(Input::TouchDown { id: 1, x, y }));
    tl.t += FINGER_MS;
    tl.push(Ev::Input(Input::TouchUp { id: 1, x, y }));
}

/// The state of the markers at a time.
#[derive(Default)]
struct Markers {
    /// The pen point, the end of the pen, and true if it touches the screen.
    pen: Option<(f32, f32, Tool, bool)>,
    /// The points of the pen tip while it touches, with their times.
    trail: Vec<(u64, f32, f32)>,
    /// The finger point and the time when it lifted (None while it touches).
    finger: Option<(f32, f32, Option<u64>)>,
}

impl Markers {
    fn apply(&mut self, t: u64, ev: &Ev) {
        match *ev {
            Ev::Hover(p, tool) => self.pen = p.map(|(x, y)| (x, y, tool, false)),
            Ev::Input(Input::PenDown { x, y, tool }) => {
                self.pen = Some((x, y, tool, true));
                self.trail.clear();
                if tool == Tool::Pen {
                    self.trail.push((t, x, y));
                }
            }
            Ev::Input(Input::PenMove { x, y }) => {
                if let Some((_, _, tool, _)) = self.pen {
                    self.pen = Some((x, y, tool, true));
                    if tool == Tool::Pen {
                        self.trail.push((t, x, y));
                    }
                }
            }
            Ev::Input(Input::PenUp { x, y }) => {
                if let Some((_, _, tool, _)) = self.pen {
                    self.pen = Some((x, y, tool, false));
                }
            }
            Ev::Input(Input::TouchDown { x, y, .. }) => self.finger = Some((x, y, None)),
            Ev::Input(Input::TouchUp { x, y, .. }) => self.finger = Some((x, y, Some(t))),
            Ev::Input(Input::TouchMove { x, y, .. }) => self.finger = Some((x, y, None)),
        }
    }

    fn draw(&mut self, pm: &mut Pixmap, t: u64) {
        let paint = |rgb: [u8; 3], a: f32| {
            let mut p = Paint::default();
            p.set_color_rgba8(rgb[0], rgb[1], rgb[2], (a.clamp(0.0, 1.0) * 255.0) as u8);
            p.anti_alias = true;
            p
        };
        let circle = |pm: &mut Pixmap, x: f32, y: f32, r: f32, fill: Option<Paint>, line: Option<(Paint, f32)>| {
            let Some(path) = PathBuilder::from_circle(x, y, r) else { return };
            if let Some(f) = fill {
                pm.fill_path(&path, &f, FillRule::Winding, Transform::identity(), None);
            }
            if let Some((p, w)) = line {
                pm.stroke_path(&path, &p, &Stroke { width: w, ..Stroke::default() }, Transform::identity(), None);
            }
        };
        const DARK: [u8; 3] = [20, 20, 20];
        const RED: [u8; 3] = [210, 40, 40];
        const GRAY: [u8; 3] = [110, 110, 110];

        if let Some((fx, fy, lifted)) = self.finger {
            let a = match lifted {
                None => 1.0,
                Some(l) => 1.0 - t.saturating_sub(l) as f32 / FINGER_FADE_MS as f32,
            };
            if a <= 0.0 {
                self.finger = None;
            } else {
                circle(pm, fx, fy, 36.0, Some(paint(GRAY, 0.35 * a)), Some((paint(GRAY, 0.8 * a), 3.0)));
            }
        }

        self.trail.retain(|&(pt, _, _)| t.saturating_sub(pt) <= TRAIL_MS);
        let touching = matches!(self.pen, Some((_, _, Tool::Pen, true)));
        if touching {
            for w in self.trail.windows(2) {
                let a = 1.0 - t.saturating_sub(w[1].0) as f32 / TRAIL_MS as f32;
                let mut pb = PathBuilder::new();
                pb.move_to(w[0].1, w[0].2);
                pb.line_to(w[1].1, w[1].2);
                if let Some(path) = pb.finish() {
                    let s = Stroke { width: 6.0, line_cap: LineCap::Round, ..Stroke::default() };
                    pm.stroke_path(&path, &paint(DARK, 0.4 * a), &s, Transform::identity(), None);
                }
            }
        }

        match self.pen {
            Some((x, y, Tool::Pen, true)) => circle(pm, x, y, 8.0, Some(paint(DARK, 0.9)), None),
            Some((x, y, Tool::Pen, false)) => circle(pm, x, y, 12.0, None, Some((paint(DARK, 0.6), 2.0))),
            Some((x, y, Tool::Eraser, true)) => {
                circle(pm, x, y, ERASER_R, Some(paint(RED, 0.2)), Some((paint(RED, 0.9), 3.0)))
            }
            Some((x, y, Tool::Eraser, false)) => circle(pm, x, y, ERASER_R, None, Some((paint(RED, 0.6), 2.0))),
            None => {}
        }
    }
}

fn verify(app: &App, c: Check) -> Result<(), String> {
    let ok = match c {
        Check::Marks(cell, m) => app.game.value_at(cell) == 0 && app.game.marks_at(cell) == m,
        Check::Value(cell, d) => app.game.value_at(cell) == d,
        Check::Color(s, color) => app.settings.color(s) == color,
    };
    if ok { Ok(()) } else { Err(format!("the app does not show the expected result: {c:?}")) }
}

fn ffmpeg(out: &Path, fps: u64, w: u32, h: u32) -> std::process::Child {
    let size = format!("{w}x{h}");
    let rate = fps.to_string();
    let mut cmd = Command::new("ffmpeg");
    cmd.args(["-loglevel", "error", "-y", "-f", "rawvideo", "-pix_fmt", "rgb24", "-s", &size, "-r", &rate, "-i", "-"]);
    if out.extension().is_some_and(|e| e == "gif") {
        cmd.args(["-vf", "fps=15,scale=540:-1:flags=lanczos,split[a][b];[a]palettegen[p];[b][p]paletteuse"]);
    } else {
        cmd.args(["-c:v", "libx264", "-pix_fmt", "yuv420p", "-crf", "18", "-movflags", "+faststart"]);
    }
    cmd.arg(out).stdin(Stdio::piped()).spawn().expect("ffmpeg (install it and put it on PATH)")
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [script_path, out] = args.as_slice() else {
        eprintln!("usage: video SCRIPT OUT.mp4|OUT.gif");
        std::process::exit(2);
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let fonts = if std::env::var_os("SUDOKU_FONT").is_some() {
        Fonts::load().expect("fonts")
    } else {
        Fonts::from_files(&root.join("scripts/fonts/rmsans-400.ttf"), &root.join("scripts/fonts/rmsans-500.ttf")).expect("fonts")
    };

    let text = std::fs::read_to_string(script_path).expect("script");
    let script = match parse(&text, sudoku::app::DEFAULT_PAUSE_MS) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{script_path}: {e}");
            std::process::exit(1);
        }
    };
    let Script { fps, seed, mode, timeline } = script;

    let mut app = App::new(Some(new_game(seed, mode)), None, ResultsDb::in_memory(), Box::new(FixedEnv(seed)));
    app.ng_stops = render::ng_stops(&fonts);
    app.ui.slider = mode;

    let (w, h) = (layout::screen_w(), layout::screen_h());
    let mut child = ffmpeg(Path::new(out), fps, w, h);
    let mut stdin = child.stdin.take().unwrap();

    // The video ends one second after the last event.
    let last = timeline.events.last().map_or(0, |e| e.0).max(timeline.checks.last().map_or(0, |c| c.0).max(timeline.t));
    let end = last + 1000;
    let frames = end * fps / 1000;
    let mut events = timeline.events.iter().peekable();
    let mut checks = timeline.checks.clone();
    checks.sort_by_key(|c| (c.0, c.1));
    let mut checks = checks.into_iter().peekable();
    let mut markers = Markers::default();
    let mut pm = Pixmap::new(w, h).unwrap();
    let mut rgb = vec![0u8; (w * h * 3) as usize];
    app.tick(0);
    for f in 0..=frames {
        let t = f * 1000 / fps;
        while let Some(&&(et, ev)) = events.peek() {
            if et > t {
                break;
            }
            if let Ev::Input(input) = ev {
                app.handle(input, et);
            }
            markers.apply(et, &ev);
            events.next();
        }
        app.tick(t);
        while let Some(&(ct, _, c)) = checks.peek() {
            if ct > t {
                break;
            }
            if let Err(e) = verify(&app, c) {
                eprintln!("{script_path}: at {:.2} s: {e}", ct as f64 / 1000.0);
                drop(stdin);
                let _ = child.wait();
                std::process::exit(1);
            }
            checks.next();
        }
        render::draw(&mut Canvas::new(&mut pm, &fonts), &app);
        markers.draw(&mut pm, t);
        for (o, p) in rgb.chunks_exact_mut(3).zip(pm.pixels()) {
            o.copy_from_slice(&[p.red(), p.green(), p.blue()]);
        }
        stdin.write_all(&rgb).expect("ffmpeg stopped");
    }
    drop(stdin);
    let status = child.wait().expect("ffmpeg");
    if !status.success() {
        eprintln!("ffmpeg failed");
        std::process::exit(1);
    }
    println!("{out}: {} frames, {:.1} s", frames + 1, end as f64 / 1000.0);
}
