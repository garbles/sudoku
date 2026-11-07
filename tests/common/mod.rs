//! Helpers for the tests: an app with a known environment, and input as a player gives it.
#![allow(dead_code)]

use sudoku::app::{App, Env, Input, Menu, Panel, Tool, HAND_TOUCH_MS};
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
    /// The time of the last pen lift that this driver sent (INP-20).
    pen_lift: Option<u64>,
}

impl Driver {
    /// A driver for a new app, with the menu closed. Refer to `Driver::from`.
    pub fn new(seed: u64) -> Driver {
        Driver::from(new_app(seed))
    }

    /// A driver for `app`, with the menu closed. The app opens with the menu open (PER-2), but most
    /// tests start from the board alone and open the menu with a tap. The tests of the start state
    /// use `Driver::opened`.
    pub fn from(mut app: App) -> Driver {
        app.ui.menu = Menu::Closed;
        Driver { app, now: 1000, pen_lift: None }
    }

    /// A driver for `app` in the state in which the app opened (the menu open, PER-2).
    pub fn opened(app: App) -> Driver {
        Driver { app, now: 1000, pen_lift: None }
    }

    /// Sends an input as a player gives it, 10 ms after the last input. A new touch does not come
    /// from the hand: it starts `HAND_TOUCH_MS` or more after the last pen lift (INP-20). A test of a
    /// hand touch uses `hand_down`, `touch_down_after_lift` or `send_at_once`.
    pub fn send(&mut self, i: Input) {
        if let (Input::TouchDown { .. }, Some(lift)) = (&i, self.pen_lift) {
            let first = lift + HAND_TOUCH_MS - 10;
            if self.now < first {
                self.wait(first - self.now);
            }
        }
        self.send_at_once(i);
    }

    /// Sends an input 10 ms after the last input, also if it is a touch less than `HAND_TOUCH_MS`
    /// after the last pen lift.
    pub fn send_at_once(&mut self, i: Input) {
        self.now += 10;
        if matches!(i, Input::PenUp { .. }) {
            self.pen_lift = Some(self.now);
        }
        self.app.handle(i, self.now);
    }

    /// The press of touch 1 at exactly `ms` after the last pen lift. It ticks the app up to that
    /// time. The test must not be later than that time already.
    pub fn touch_down_after_lift(&mut self, ms: u64, x: f32, y: f32) {
        let at = self.pen_lift.expect("a pen lift") + ms;
        assert!(self.now <= at, "the test is at {} and the press must be at {at}", self.now);
        self.wait(at - self.now);
        self.app.handle(Input::TouchDown { id: 1, x, y }, at);
    }

    /// The press of a hand touch (touch 1): the first possible time after the last pen lift is the
    /// time of the next input. The tablet sends it approximately 97 ms after the lift (INP-20).
    pub fn hand_down(&mut self, x: f32, y: f32) {
        self.send_at_once(Input::TouchDown { id: 1, x, y });
    }

    /// A full tap of a hand touch at a point: the press and the release (INP-20).
    pub fn hand_tap(&mut self, x: f32, y: f32) {
        self.hand_down(x, y);
        self.send_at_once(Input::TouchUp { id: 1, x, y });
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

/// The point that a test taps to select the micro-cell line gray `g` in the Cell colors panel.
pub fn line_gray_point(g: sudoku::settings::LineGray) -> (f32, f32) {
    let cell = layout::line_gray_cell(g);
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
        Checkpoints => (x, layout::tb_cy(layout::CKP_SLOT)),
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

/// True if a peer of `cell` has the number `digit`. It uses only the numbers of the board (MRK-18),
/// and it does not use the code of the app that makes this decision (`Game::refuses_mark`).
pub fn peer_has(g: &Game, cell: Cell, digit: u8) -> bool {
    sudoku::game::peers(cell).any(|p| g.value_at(p) == digit)
}

/// All pairs of an empty cell and a digit where a pen tap is a refused tap (MRK-7), if
/// "Hide pencil marks automatically" is on.
pub fn refused_pairs(g: &Game) -> Vec<(Cell, u8)> {
    cells().filter(|&c| !g.is_filled(c)).flat_map(|c| (1..=9u8).filter(move |&d| peer_has(g, c, d)).map(move |d| (c, d))).collect()
}

/// The first pair of `refused_pairs`.
pub fn refused_pair(g: &Game) -> (Cell, u8) {
    refused_pairs(g)[0]
}

// ---- A separate model of the cell colors (BRD-7, COL-8, COL-9). It uses only `value`, `given` and
// the settings, and it does not use the code of the app that calculates the groups or the clashes.

/// A solved board for tests that give their own starting numbers.
pub const SOLUTION: [&str; 9] =
    ["534678912", "672195348", "198342567", "859761423", "426853791", "713924856", "961537284", "287419635", "345286179"];

pub fn digits(rows: &[&str; 9]) -> [[u8; 9]; 9] {
    let mut g = [[0u8; 9]; 9];
    for (r, row) in rows.iter().enumerate() {
        for (c, ch) in row.bytes().enumerate() {
            g[r][c] = if ch == b'.' { 0 } else { ch - b'0' };
        }
    }
    g
}

/// The starting numbers of a puzzle with only one solution, `SOLUTION` (PUZ-2). A test that sends
/// input to the app uses it, because debug builds examine the rules after each input.
pub const PUZZLE: [&str; 9] =
    ["53..7....", "6..195...", ".98....6.", "8...6...3", "4..8.3..1", "7...2...6", ".6....28.", "...419..5", "....8..79"];

/// `PUZZLE` with entries: the player wrote each number of `entries`.
pub fn puzzle_with(entries: &[(Cell, u8)]) -> Game {
    let mut g = board_of(&PUZZLE);
    for &(cell, v) in entries {
        g.add_number(cell, v);
    }
    g
}

/// A game with the solution `SOLUTION` and these starting numbers (a '.' is an empty cell). It is not
/// a puzzle of the generator, thus `App::new` does not accept it as a saved game (PER-5). These starting
/// numbers do not always have only one solution: use it only for tests that send no input to the app.
pub fn board_of(rows: &[&str; 9]) -> Game {
    let solution = digits(&SOLUTION);
    let given = digits(rows);
    for (r, c) in cells() {
        let v = given[r as usize][c as usize];
        assert!(v == 0 || v == solution[r as usize][c as usize], "PUZ-3: the starting number at ({r},{c})");
    }
    Game::new(1, 2, solution, given)
}

/// A board with only the starting numbers at `given` (values of `SOLUTION`).
pub fn board_with(given: &[Cell]) -> Game {
    let solution = digits(&SOLUTION);
    let mut g = [[0u8; 9]; 9];
    for &(r, c) in given {
        g[r as usize][c as usize] = solution[r as usize][c as usize];
    }
    Game::new(1, 2, solution, g)
}

/// The 27 groups: 9 rows, 9 columns and 9 boxes.
pub fn model_groups() -> Vec<Vec<Cell>> {
    let mut groups = Vec::new();
    for i in 0..9u8 {
        groups.push((0..9u8).map(|k| (i, k)).collect());
        groups.push((0..9u8).map(|k| (k, i)).collect());
        groups.push((0..9u8).map(|k| (i / 3 * 3 + k / 3, i % 3 * 3 + k % 3)).collect());
    }
    groups
}

/// CLS-1: the cell is filled and another cell of one of its groups has the same value.
pub fn model_clashing(g: &Game, cell: Cell) -> bool {
    let v = g.value_at(cell);
    v != 0 && model_groups().iter().filter(|group| group.contains(&cell)).any(|group| group.iter().any(|&o| o != cell && g.value_at(o) == v))
}

/// COL-8: all nine cells are filled and no cell is clashing, also with a cell outside the group.
pub fn model_completed_groups(g: &Game) -> Vec<Vec<Cell>> {
    model_groups().into_iter().filter(|group| group.iter().all(|&c| g.value_at(c) != 0 && !model_clashing(g, c))).collect()
}

pub fn model_in_completed_group(g: &Game, cell: Cell) -> bool {
    model_completed_groups(g).iter().any(|group| group.contains(&cell))
}

/// BRD-7: the background of a cell, from the first rule that applies.
pub fn model_background(app: &App, cell: Cell) -> [u8; 3] {
    let (g, s) = (&app.game, &app.settings);
    let v = g.value_at(cell);
    if v == 0 {
        Color::White.rgb()
    } else if model_clashing(g, cell) {
        s.color_clash.rgb()
    } else if app.ui.highlight == Some(v) {
        s.color_highlight.rgb()
    } else if s.completed_color_on && s.color_completed != Color::White && !g.completed && model_in_completed_group(g, cell) {
        s.color_completed.rgb()
    } else if g.is_given(cell) {
        s.color_starting.rgb()
    } else {
        Color::White.rgb()
    }
}

/// The screen of an app, in the current layout.
pub fn draw_screen(app: &App, fonts: &Fonts) -> tiny_skia::Pixmap {
    let mut pm = tiny_skia::Pixmap::new(layout::screen_w(), layout::screen_h()).unwrap();
    sudoku::render::draw(&mut sudoku::render::Canvas::new(&mut pm, fonts), app);
    pm
}

/// A point of a cell that has only the background: no digit, no mark, no line.
fn background_point(cell: Cell) -> (i32, i32) {
    let (x, y) = cell_origin(cell);
    (x + 10, y + 135)
}

pub fn cell_background(pm: &tiny_skia::Pixmap, cell: Cell) -> [u8; 3] {
    let (x, y) = background_point(cell);
    sudoku::render::rgb_at(pm, x, y)
}

/// Each cell that the open panel does not cover has the background of the model, and the game agrees
/// with the model for the completed groups (BRD-7, COL-8, COL-9).
pub fn assert_board(app: &App, fonts: &Fonts, case: &str) {
    let pm = draw_screen(app, fonts);
    let set = app.game.completed_cells();
    for group in model_completed_groups(&app.game) {
        let mut d: Vec<u8> = group.iter().map(|&c| app.game.value_at(c)).collect();
        d.sort_unstable();
        assert_eq!(d, [1, 2, 3, 4, 5, 6, 7, 8, 9], "COL-9, {case}: {group:?}");
    }
    let panel = app.ui.menu.panel().map(|p| match p {
        Panel::Colors => layout::colors_panel(),
        Panel::Settings => layout::settings_panel(app.update.has_row()),
        Panel::NewGame => layout::ng_panel(),
        Panel::Checkpoints => layout::ckp_panel(app.game.checkpoints.len()),
    });
    for c in cells() {
        assert_eq!(set.contains(c), model_in_completed_group(&app.game, c), "COL-8, {case}: {c:?}");
        let (x, y) = background_point(c);
        if panel.is_some_and(|p| p.contains(x as f32, y as f32)) {
            continue;
        }
        assert_eq!(cell_background(&pm, c), model_background(app, c), "BRD-7, {case}: {c:?}");
    }
}

/// The x range of the Cell colors panel in the current layout, with the two ends (NAV-17).
pub fn documented_colors_x() -> (i32, i32) {
    match (layout::right_handed(), layout::screen_w() == 2160) {
        (true, _) => (112, 911),
        (false, false) => (708, 1507),
        (false, true) => (1248, 2047),
    }
}

/// The x range of the Settings panel in the current layout, with the two ends (NAV-14, NAV-15).
pub fn documented_settings_x() -> (i32, i32) {
    match (layout::right_handed(), layout::screen_w() == 2160) {
        (true, _) => (112, 511),
        (false, false) => (1108, 1507),
        (false, true) => (1648, 2047),
    }
}

/// The x range of the Checkpoints panel in the current layout, with the two ends (NAV-18).
pub fn documented_checkpoints_x() -> (i32, i32) {
    match (layout::right_handed(), layout::screen_w() == 2160) {
        (true, _) => (112, 711),
        (false, false) => (908, 1507),
        (false, true) => (1448, 2047),
    }
}

/// The first y of row `i` (from 0) of the Settings panel: y 224, a 2 px border and lines, rows of
/// 112 px (REQUIREMENTS.md §9.1).
pub fn documented_settings_row_y(i: i32) -> i32 {
    224 + 2 + i * 114
}
