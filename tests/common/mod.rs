//! Helpers for the tests: an app with a known environment, and input as a player gives it.
#![allow(dead_code)]

use sudoku::app::{App, Env, Input, Tool};
use sudoku::font::Fonts;
use sudoku::game::{cells, Cell, Game};
use sudoku::layout::{self, cell_origin, Rect, CELL};
use sudoku::settings::{Color, Section, Switch};
use sudoku::store::{NewResult, ResultsDb};

/// The wall clock of `TestEnv` at the start of most tests (a Unix time).
pub const START_TIME: i64 = 1_790_000_000;

pub struct TestEnv {
    pub time: i64,
    pub seed: u64,
}

impl Env for TestEnv {
    fn unix_time(&mut self) -> i64 {
        self.time += 60;
        self.time
    }
    fn new_seed(&mut self) -> u64 {
        self.seed = self.seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.seed
    }
}

pub fn new_app(seed: u64) -> App {
    App::new(None, None, ResultsDb::in_memory(), Box::new(TestEnv { time: START_TIME, seed }), App::placeholder_ng_stops())
}

/// An app that opens with the saved game `game`, the default settings and no results.
pub fn app_with_game(game: Game, time: i64, seed: u64) -> App {
    App::new(Some(game), None, ResultsDb::in_memory(), Box::new(TestEnv { time, seed }), App::placeholder_ng_stops())
}

/// An app with `n` results in the results database (different modes, times and seeds).
pub fn app_with_results(n: usize) -> App {
    let mut db = ResultsDb::in_memory();
    for i in 0..n {
        db.insert(&NewResult { finished_at: 1_700_000_000 + i as i64 * 1000, mode: (i % 5) as u8, givens: 30, seconds: 100 + i as u64, seed: i as u64, generator: 2 })
            .unwrap();
    }
    App::new(None, None, db, Box::new(TestEnv { time: START_TIME, seed: 1 }), App::placeholder_ng_stops())
}

/// The fonts of the tablet, from `scripts/fonts/`.
pub fn fonts() -> Fonts {
    Fonts::repo().unwrap()
}

/// A driver that keeps the time.
pub struct Driver {
    pub app: App,
    pub now: u64,
}

impl Driver {
    pub fn new(seed: u64) -> Driver {
        Driver { app: new_app(seed), now: 1000 }
    }

    pub fn from(app: App) -> Driver {
        Driver { app, now: 1000 }
    }

    pub fn send(&mut self, i: Input) {
        self.now += 10;
        self.app.handle(i, self.now);
    }

    pub fn wait(&mut self, ms: u64) {
        let end = self.now + ms;
        while self.now < end {
            self.now = (self.now + 100).min(end);
            self.app.tick(self.now);
        }
    }

    pub fn finger_tap(&mut self, x: f32, y: f32) {
        self.send(Input::TouchDown { id: 1, x, y });
        self.send(Input::TouchUp { id: 1, x, y });
    }

    pub fn pen_tap(&mut self, x: f32, y: f32) {
        self.send(Input::PenDown { x, y, tool: Tool::Pen });
        self.send(Input::PenUp { x, y });
    }

    pub fn swipe(&mut self, left: bool) {
        let (x0, x1) = if left { (1200.0, 300.0) } else { (300.0, 1200.0) };
        self.send(Input::TouchDown { id: 2, x: x0, y: 1000.0 });
        self.send(Input::TouchMove { id: 2, x: (x0 + x1) / 2.0, y: 1000.0 });
        self.send(Input::TouchUp { id: 2, x: x1, y: 1000.0 });
    }

    /// A pen tap on micro-cell `digit` of a cell.
    pub fn tap_micro(&mut self, cell: Cell, digit: u8) {
        let (x, y) = micro_center(cell, digit);
        self.pen_tap(x, y);
    }

    pub fn tap_cell(&mut self, cell: Cell) {
        self.tap_micro(cell, 5);
    }

    /// Writes strokes and lifts the pen. It does not wait for the recognition pause.
    pub fn write_strokes(&mut self, strokes: &[Vec<(f32, f32)>]) {
        for s in strokes {
            self.send(Input::PenDown { x: s[0].0, y: s[0].1, tool: Tool::Pen });
            for p in &s[1..] {
                self.send(Input::PenMove { x: p.0, y: p.1 });
            }
            let last = s[s.len() - 1];
            self.send(Input::PenUp { x: last.0, y: last.1 });
        }
    }

    /// Writes a digit (one of the template shapes) in a cell and waits for the recognition pause.
    pub fn write(&mut self, cell: Cell, d: u8) {
        let ink = digit_ink(cell, d);
        self.write_strokes(&ink);
        self.wait(self.app.pause_ms + 50);
    }

    pub fn erase(&mut self, cell: Cell) {
        let (x, y) = cell_origin(cell);
        let (x, y) = (x as f32, y as f32);
        self.send(Input::PenDown { x: x + 30.0, y: y + 30.0, tool: Tool::Eraser });
        self.send(Input::PenMove { x: x + 100.0, y: y + 100.0 });
        self.send(Input::PenUp { x: x + 100.0, y: y + 100.0 });
    }

    pub fn button(&mut self, b: layout::Button) {
        let (x, y) = button_center(b);
        self.finger_tap(x, y);
    }
}

/// The first empty cell of the board.
pub fn empty_cell(g: &Game) -> Cell {
    cells().find(|&c| !g.is_filled(c)).unwrap()
}

/// The first cell with a starting number.
pub fn given_cell(g: &Game) -> Cell {
    cells().find(|&c| g.is_given(c)).unwrap()
}

/// The center of an area, rounded down to a full pixel.
pub fn pixel_center(r: Rect) -> (f32, f32) {
    (((r.x0 + r.x1) / 2) as f32, ((r.y0 + r.y1) / 2) as f32)
}

/// The exact center of an area (it can be at a half pixel).
pub fn exact_center(r: Rect) -> (f32, f32) {
    ((r.x0 + r.x1) as f32 / 2.0, (r.y0 + r.y1) as f32 / 2.0)
}

/// The point that a test taps to select `color` in `section` of the Cell colors panel.
pub fn color_point(section: Section, color: Color) -> (f32, f32) {
    let cell = layout::color_cell(section, color);
    (cell.x0 as f32 + 56.0, cell.y0 as f32 + 40.0)
}

/// The point that a test taps to change the switch `s` in the Settings panel: the center of its row.
pub fn switch_point(s: Switch) -> (f32, f32) {
    exact_center(layout::settings_row(s.row()))
}

pub fn micro_center((r, c): Cell, digit: u8) -> (f32, f32) {
    let (x, y) = cell_origin((r, c));
    let i = (digit - 1) as i32;
    (x as f32 + ((i % 3) * 49 + 24) as f32, y as f32 + ((i / 3) * 49 + 24) as f32)
}

pub fn button_center(b: layout::Button) -> (f32, f32) {
    use layout::Button::*;
    let x = layout::tb_cx();
    match b {
        Menu => (x, layout::tb_cy(0)),
        Colors => (x, layout::tb_cy(1)),
        Settings => (x, layout::tb_cy(2)),
        Undo => (x, layout::tb_cy(3)),
        Redo => (x, layout::tb_cy(4)),
        Help => (x, (layout::tb_bottom_y() - 168) as f32),
        Statistics => (x, (layout::tb_bottom_y() - 56) as f32),
        NewGame => (x, (layout::tb_bottom_y() + 56) as f32),
        Close => (layout::close_cx(), layout::CLOSE_CY),
    }
}

/// Real handwriting of a digit (UJI Pen Characters, writer 1), in a box of 98 px at the center of a cell.
pub fn digit_ink(cell: Cell, d: u8) -> Vec<Vec<(f32, f32)>> {
    digit_ink_by(cell, d, 1)
}

pub fn digit_ink_by(cell: Cell, d: u8, writer: u8) -> Vec<Vec<(f32, f32)>> {
    let (x, y) = cell_origin(cell);
    let (ox, oy) = (x as f32 + (CELL as f32 - 98.0) / 2.0, y as f32 + (CELL as f32 - 98.0) / 2.0);
    sudoku::recognizer::example_ink(d, writer, ox, oy, 98.0).expect("example")
}

/// A scribble in a cell (not a number).
pub fn scribble_ink(cell: Cell) -> Vec<Vec<(f32, f32)>> {
    let (x, y) = cell_origin(cell);
    let (x, y) = (x as f32, y as f32);
    vec![(0..12).map(|i| (x + 20.0 + (i % 2) as f32 * 100.0, y + 15.0 + i as f32 * 10.0)).collect()]
}

/// Solves the current game with the pen, cell by cell. It erases wrong entries first.
pub fn solve(d: &mut Driver) {
    for r in 0..9u8 {
        for c in 0..9u8 {
            if d.app.game.is_entry((r, c)) && d.app.game.value_at((r, c)) != d.app.game.solution[r as usize][c as usize] {
                d.erase((r, c));
            }
        }
    }
    for r in 0..9u8 {
        for c in 0..9u8 {
            if !d.app.game.is_filled((r, c)) {
                let v = d.app.game.solution[r as usize][c as usize];
                d.write((r, c), v);
                assert_eq!(d.app.game.value_at((r, c)), v, "the app did not read {v} at ({r},{c})");
            }
        }
    }
}

/// An empty cell where no peer has one of `digits` as a number. A tap adds a mark for each of these
/// digits (MRK-7: a tap on a hidden mark has no effect).
pub fn free_cell(app: &App, digits: &[u8]) -> Cell {
    let g = &app.game;
    sudoku::game::cells()
        .find(|&c| !g.is_filled(c) && digits.iter().all(|&d| g.peer_digits(c) & sudoku::game::mark_bit(d) == 0))
        .expect("an empty cell with these free digits")
}
