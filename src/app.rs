//! The app: screens, the menu, the panels, the overlays, input and the clock.
//!
//! The app does not do input or output. The platform gives it input events and the time. The platform
//! reads the flags `game_dirty` and `settings_dirty` and saves the data, and it stops when `quit` is
//! true. Refer to REQUIREMENTS.md §2, §3, §5, §8 to §13 and INVARIANTS.md.

use crate::game::{Cell, Game, Outcome};
use crate::generator::{generate, MODES};
use crate::layout::{self, Button, Rect};
use crate::recognizer::{self, Stroke};
use crate::settings::{Color, Section, Settings};
use crate::store::{NewResult, ResultRow, ResultsDb};

/// The recognition pause after the pen lifts (§5.1). `SUDOKU_RECOGNITION_PAUSE_MS` changes it.
pub const DEFAULT_PAUSE_MS: u64 = 500;
/// The time that the "?" shows (§5.1).
pub const QUESTION_MS: u64 = 1000;
/// A gap between two events that is longer than this is a suspension. The clock does not count it.
pub const MAX_CLOCK_GAP_MS: u64 = 5000;

pub const MANUAL_PAGES: u8 = 9;
/// A finger touch with no event for this time is forgotten when a new touch starts (INP-15).
pub const STALE_TOUCH_MS: u64 = 150;
pub const PER_PAGE: usize = 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Screen {
    Game,
    Manual,
    Statistics,
    GamesList,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Panel {
    None,
    Colors,
    Settings,
    NewGame,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Pen,
    Eraser,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Input {
    PenDown { x: f32, y: f32, tool: Tool },
    PenMove { x: f32, y: f32 },
    PenUp { x: f32, y: f32 },
    TouchDown { id: i32, x: f32, y: f32 },
    TouchMove { id: i32, x: f32, y: f32 },
    TouchUp { id: i32, x: f32, y: f32 },
}

/// Ink that waits for the recognition pause (INP-5).
#[derive(Clone, Debug, PartialEq)]
pub struct PendingInk {
    pub cell: Cell,
    pub strokes: Vec<Stroke>,
    /// The time when the pen lifted. None while the pen writes.
    pub last_lift_at: Option<u64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GameUi {
    pub menu_open: bool,
    pub panel: Panel,
    pub highlight: Option<u8>,
    pub question: Option<(Cell, u64)>,
    pub pending_ink: Option<PendingInk>,
    pub slider: u8,
}

impl GameUi {
    fn closed(slider: u8) -> GameUi {
        GameUi { menu_open: false, panel: Panel::None, highlight: None, question: None, pending_ink: None, slider }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ListUi {
    /// None is "All".
    pub filter: Option<u8>,
    pub page: usize,
    pub confirm: Option<ResultRow>,
}

/// Saves a game immediately; true if the save is done.
pub type SaveGame = Box<dyn FnMut(&Game) -> bool>;

/// Receives each reading of ink: the strokes and the result. Only an examination of the recognizer on
/// a device uses it (`platform`, the file `log-ink` in the data folder).
pub type InkLog = Box<dyn FnMut(&[Stroke], Option<u8>)>;

/// Things that the platform gives to the app: the wall clock and new seeds.
pub trait Env {
    /// Unix time in seconds.
    fn unix_time(&mut self) -> i64;
    fn new_seed(&mut self) -> u64;
}

/// What a pen or a finger does from its press to its release.
#[derive(Clone, Debug, PartialEq)]
enum Gesture {
    /// The touch has no more effect.
    Ignored,
    /// A press on a control. The action occurs on the release if the touch is still on the control.
    Control(Target),
    /// A pen press on the board. It is a tap while it stays in the micro-cell.
    BoardTap { cell: Cell, digit: u8, prev_highlight: Option<u8> },
    /// Writing in an empty cell. The stroke goes into the pending ink.
    Writing,
    /// Writing that has no effect (on a filled cell). The app removes the ink on the release.
    DiscardInk,
    Eraser,
    /// The finger or pen moves the slider of the New game panel.
    Slider,
    /// A finger on the manual or the games list. It can be a swipe.
    Swipe { x0: f32, y0: f32, target: Option<Target> },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Target {
    Game(Button),
    Color(usize, usize),
    StartNewGame,
    Back,
    Prev,
    Next,
    AllGames,
    Filter(usize),
    Bin(usize),
    DialogCancel,
    DialogDelete,
    /// A row of the Settings panel (0: highlight numbers, 1: remove pencil marks automatically).
    Switch(usize),
    DeleteAll,
    AllCancel,
    AllDelete,
}

pub struct App {
    pub screen: Screen,
    pub game: Game,
    pub ui: GameUi,
    pub manual_page: u8,
    pub list: ListUi,
    /// The "Delete all games" dialog shows on the summary (ANA-12).
    pub confirm_all: bool,
    pub settings: Settings,
    pub results: Vec<ResultRow>,
    db: ResultsDb,
    env: Box<dyn Env>,

    pub pause_ms: u64,
    /// The x positions of the stops of the New game slider. They depend on the font (layout.rs).
    pub ng_stops: [f32; 5],
    /// The stroke that the pen draws now (to show the ink).
    pub live_stroke: Option<Stroke>,
    /// The quantity of live strokes that began. The platform uses it to know that `live_stroke` is a
    /// new stroke, also when no full drawing occurred between two strokes.
    pub live_stroke_count: u64,

    pen: Option<Gesture>,
    /// The finger touch that the app follows: its id, its gesture and the time of its last event.
    touch: Option<(i32, Gesture, u64)>,
    last_clock: Option<u64>,

    pub game_dirty: bool,
    pub settings_dirty: bool,
    pub quit: bool,
    /// Saves the game immediately. The app calls it when a puzzle becomes complete, before it adds the
    /// result: thus a power loss can never record the same game two times (CMP-5).
    pub save_game_now: Option<SaveGame>,
    pub ink_log: Option<InkLog>,
    /// The screen can be different from the last full drawing. False if the only change is new points
    /// of `live_stroke`: then the platform draws only the new part of the ink, so that the ink follows
    /// the pen.
    pub redraw: bool,
}

impl App {
    /// Makes the app from the saved data (PER-2). If there is no saved game, or the saved game is
    /// not correct (PER-5), the app makes a new game of the last selected mode.
    pub fn new(saved: Option<Game>, settings: Option<Settings>, db: ResultsDb, mut env: Box<dyn Env>) -> App {
        let settings = settings.filter(Settings::is_valid).unwrap_or_default();
        let game = match saved.filter(|g| crate::invariants::check_game(g).is_ok()) {
            Some(g) => g,
            None => new_game(env.new_seed(), settings.last_mode),
        };
        let results = db.all().unwrap_or_default();
        let slider = settings.last_mode;
        App {
            screen: Screen::Game,
            game,
            ui: GameUi::closed(slider),
            manual_page: 1,
            list: ListUi { filter: None, page: 1, confirm: None },
            confirm_all: false,
            settings,
            results,
            db,
            env,
            pause_ms: DEFAULT_PAUSE_MS,
            ng_stops: layout::slider_stops(layout::ng_tx(), layout::ng_tr(), 50.0, 70.0),
            live_stroke: None,
            live_stroke_count: 0,
            pen: None,
            touch: None,
            last_clock: None,
            game_dirty: true,
            settings_dirty: false,
            quit: false,
            redraw: true,
            save_game_now: None,
            ink_log: None,
        }
    }

    // ---- Time

    /// Moves the clock and the timers to `now` (milliseconds, monotonic).
    pub fn tick(&mut self, now: u64) {
        if let Some(last) = self.last_clock {
            let dt = now.saturating_sub(last);
            if dt <= MAX_CLOCK_GAP_MS {
                self.game.tick(dt);
            }
        }
        self.last_clock = Some(now);

        if let Some((_, shown_at)) = self.ui.question {
            if now.saturating_sub(shown_at) >= QUESTION_MS {
                self.ui.question = None;
                self.redraw = true;
            }
        }
        if let Some(ink) = &self.ui.pending_ink {
            if let Some(lift) = ink.last_lift_at {
                if now.saturating_sub(lift) >= self.pause_ms {
                    self.read_ink(now);
                    self.redraw = true;
                }
            }
        }
    }

    /// The tablet turned. The positions of ink and of touches are not correct now: the app removes the
    /// ink that waits and stops the touches. The game does not change.
    /// `ng_stops` are the slider stops of the new layout (`render::ng_stops`).
    pub fn rotate(&mut self, ng_stops: [f32; 5]) {
        self.ng_stops = ng_stops;
        self.ui.pending_ink = None;
        self.live_stroke = None;
        self.pen = None;
        self.touch = None;
        self.redraw = true;
    }

    /// The app is suspended or closed: the clock stops (CLK-5). Ink that waits is removed (PER-6).
    pub fn suspend(&mut self) {
        self.last_clock = None;
        self.ui.pending_ink = None;
        self.live_stroke = None;
        self.pen = None;
        self.touch = None;
        self.game_dirty = true;
        self.redraw = true;
    }

    /// The time of the next timer, if a timer runs.
    pub fn next_deadline(&self) -> Option<u64> {
        let q = self.ui.question.map(|(_, t)| t + QUESTION_MS);
        let ink = self.ui.pending_ink.as_ref().and_then(|i| i.last_lift_at).map(|t| t + self.pause_ms);
        match (q, ink) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    // ---- Input

    pub fn handle(&mut self, input: Input, now: u64) {
        self.tick(now);
        // A pen movement while the pen writes changes only the ink. Other input needs a full drawing
        // only if it changes something that the screen shows.
        let ink_only = matches!(input, Input::PenMove { .. })
            && matches!(self.pen, Some(Gesture::Writing | Gesture::DiscardInk | Gesture::BoardTap { .. }));
        let before = (!ink_only).then(|| self.view_key());
        match input {
            Input::PenDown { x, y, tool } => {
                let g = self.press(x, y, Some(tool), now);
                self.pen = Some(g);
            }
            Input::PenMove { x, y } => {
                if let Some(g) = self.pen.take() {
                    self.pen = Some(self.pen_move(g, x, y));
                }
            }
            Input::PenUp { x, y } => {
                if let Some(g) = self.pen.take() {
                    // The release point is a part of the movement of the touch (INP-2).
                    let g = self.pen_move(g, x, y);
                    self.pen_up(g, x, y, now);
                }
            }
            Input::TouchDown { id, x, y } => {
                // INP-15: AppLoad sometimes does not send the release of a touch (for example a palm
                // touch that the system cancels), and sometimes sends the press of a touch again. The
                // app forgets the touch that it follows if the new press has the same id, or if that
                // touch has had no event for STALE_TOUCH_MS. Thus a lost release never stops the next
                // touches. A real second finger (an active touch) has no effect.
                let stale = match self.touch {
                    None => true,
                    Some((tid, _, last)) => tid == id || now.saturating_sub(last) >= STALE_TOUCH_MS,
                };
                if stale {
                    let g = self.press(x, y, None, now);
                    self.touch = Some((id, g, now));
                } else {
                    // A second finger has no other effect. But it is an action: it removes the
                    // highlight and the "?" (HLT-2, QST-4).
                    self.ui.highlight = None;
                    self.ui.question = None;
                }
            }
            Input::TouchMove { id, x, y } => {
                if let Some((tid, g, last)) = self.touch.take() {
                    self.touch = Some(if tid == id { (tid, self.finger_move(g, x, y), now) } else { (tid, g, last) });
                }
            }
            Input::TouchUp { id, x, y } => match self.touch.take() {
                Some((tid, g, _)) if tid == id => self.finger_up(g, x, y),
                other => self.touch = other,
            },
        }
        if before.is_some_and(|b| self.view_key() != b) {
            self.redraw = true;
        }
        #[cfg(debug_assertions)]
        if let Err(e) = crate::invariants::check_app(self) {
            panic!("invariant failed after {input:?}: {e}");
        }
    }

    /// A press of the pen (`tool` is Some) or of a finger (`tool` is None).
    fn press(&mut self, x: f32, y: f32, tool: Option<Tool>, now: u64) -> Gesture {
        // Each action removes the highlight before the app does the action (HLT-2).
        let prev_highlight = self.ui.highlight.take();
        self.ui.question = None;

        // Ink that waits in a different cell is read now. Ink in the same cell gets the new stroke.
        let cell = layout::board_cell(x, y);
        let continues_ink = self.screen == Screen::Game
            && self.ui.panel == Panel::None
            && tool == Some(Tool::Pen)
            && cell.is_some()
            && self.ui.pending_ink.as_ref().map(|i| Some(i.cell) == cell).unwrap_or(false);
        if continues_ink {
            let ink = self.ui.pending_ink.as_mut().expect("pending ink");
            ink.last_lift_at = None;
            self.start_live_stroke(x, y);
            return Gesture::Writing;
        }
        // A finger on the board has no effect on the game (INP-1), thus it does not read the ink (for
        // example a palm on the screen between two strokes of a 4).
        let finger_on_board = tool.is_none() && self.screen == Screen::Game && cell.is_some();
        if self.ui.pending_ink.is_some() && !finger_on_board {
            self.read_ink(now);
        }

        match self.screen {
            Screen::Game => self.press_game(x, y, tool, prev_highlight),
            Screen::Manual | Screen::Statistics | Screen::GamesList => {
                let target = self.page_target(x, y);
                if tool.is_none() && self.screen != Screen::Statistics {
                    Gesture::Swipe { x0: x, y0: y, target }
                } else {
                    target.map(Gesture::Control).unwrap_or(Gesture::Ignored)
                }
            }
        }
    }

    fn press_game(&mut self, x: f32, y: f32, tool: Option<Tool>, prev_highlight: Option<u8>) -> Gesture {
        let button = layout::game_button(x, y, self.ui.menu_open);

        // A panel is open: a tap outside the panel only closes the panel (NAV-11). The paint bucket and
        // the + also change the panel (§9).
        if self.ui.panel != Panel::None {
            if let Some(b @ (Button::Colors | Button::Settings | Button::NewGame)) = button {
                return Gesture::Control(Target::Game(b));
            }
            match self.ui.panel {
                Panel::Settings if layout::settings_panel().contains(x, y) => {
                    return layout::settings_at(x, y).map(|i| Gesture::Control(Target::Switch(i))).unwrap_or(Gesture::Ignored);
                }
                Panel::Colors if layout::colors_panel().contains(x, y) => {
                    return layout::color_at(x, y)
                        .map(|(s, i)| Gesture::Control(Target::Color(s, i)))
                        .unwrap_or(Gesture::Ignored);
                }
                Panel::NewGame if layout::ng_panel().contains(x, y) => {
                    if layout::ng_slider_row().contains(x, y) {
                        self.ui.slider = layout::nearest_stop(&self.ng_stops, x);
                        return Gesture::Slider;
                    }
                    if layout::ng_start_row().contains(x, y) {
                        return Gesture::Control(Target::StartNewGame);
                    }
                    return Gesture::Ignored;
                }
                _ => {
                    self.close_panel();
                    return Gesture::Ignored;
                }
            }
        }

        if let Some(b) = button {
            return Gesture::Control(Target::Game(b));
        }

        // The board. Only the pen operates it (§5.1). A complete puzzle is locked (§8.3).
        let (Some(tool), Some((cell, digit))) = (tool, layout::board_micro(x, y)) else {
            return Gesture::Ignored;
        };
        if self.game.completed {
            return Gesture::Ignored;
        }
        match tool {
            Tool::Eraser => {
                self.erase_at(x, y);
                Gesture::Eraser
            }
            Tool::Pen => {
                self.start_live_stroke(x, y);
                Gesture::BoardTap { cell, digit, prev_highlight }
            }
        }
    }

    fn start_live_stroke(&mut self, x: f32, y: f32) {
        self.live_stroke = Some(vec![(x, y)]);
        self.live_stroke_count += 1;
    }

    fn pen_move(&mut self, g: Gesture, x: f32, y: f32) -> Gesture {
        match g {
            Gesture::BoardTap { cell, digit, .. } => {
                if let Some(s) = self.live_stroke.as_mut() {
                    s.push((x, y));
                }
                if layout::board_micro(x, y) == Some((cell, digit)) {
                    return g;
                }
                // The pen left the micro-cell: this touch is writing (INP-2).
                if self.game.is_filled(cell) {
                    Gesture::DiscardInk
                } else {
                    let stroke = self.live_stroke.clone().unwrap_or_default();
                    self.ui.pending_ink = Some(PendingInk { cell, strokes: Vec::new(), last_lift_at: None });
                    self.live_stroke = Some(stroke);
                    Gesture::Writing
                }
            }
            Gesture::Writing | Gesture::DiscardInk => {
                if let Some(s) = self.live_stroke.as_mut() {
                    s.push((x, y));
                }
                g
            }
            Gesture::Eraser => {
                self.erase_at(x, y);
                g
            }
            Gesture::Slider => {
                self.ui.slider = layout::nearest_stop(&self.ng_stops, x);
                g
            }
            other => other,
        }
    }

    fn pen_up(&mut self, g: Gesture, x: f32, y: f32, now: u64) {
        match g {
            Gesture::BoardTap { cell, digit, prev_highlight } => {
                self.live_stroke = None;
                self.board_tap(cell, digit, prev_highlight);
            }
            Gesture::Writing => {
                let stroke = self.live_stroke.take().unwrap_or_default();
                if let Some(ink) = self.ui.pending_ink.as_mut() {
                    ink.strokes.push(stroke);
                    ink.last_lift_at = Some(now);
                }
            }
            Gesture::DiscardInk => self.live_stroke = None,
            Gesture::Control(t) => self.release_control(t, x, y),
            _ => {}
        }
    }

    fn finger_move(&mut self, g: Gesture, x: f32, _y: f32) -> Gesture {
        if g == Gesture::Slider {
            self.ui.slider = layout::nearest_stop(&self.ng_stops, x);
        }
        g
    }

    fn finger_up(&mut self, g: Gesture, x: f32, y: f32) {
        match g {
            Gesture::Control(t) => self.release_control(t, x, y),
            Gesture::Swipe { x0, y0, target } => {
                let (dx, dy) = (x - x0, y - y0);
                if dx.abs() >= layout::SWIPE_MIN && dx.abs() > dy.abs() {
                    self.swipe(dx < 0.0);
                } else if let Some(t) = target {
                    self.release_control(t, x, y);
                }
            }
            _ => {}
        }
    }

    /// A tap on the board (§5.2).
    fn board_tap(&mut self, cell: Cell, digit: u8, prev_highlight: Option<u8>) {
        if self.game.completed {
            return;
        }
        let v = self.game.value_at(cell);
        if v != 0 {
            // HLT-3: the same digit removes the highlight. A different digit moves it. HLT-6: no
            // highlight if the switch is off.
            self.ui.highlight = if !self.settings.highlight_on || prev_highlight == Some(v) { None } else { Some(v) };
        } else {
            let o = self.game.toggle_mark(cell, digit);
            self.after_game_change(o);
        }
    }

    fn erase_at(&mut self, x: f32, y: f32) {
        if let Some(cell) = layout::board_cell(x, y) {
            let o = self.game.erase(cell);
            self.after_game_change(o);
        }
    }

    /// Reads the pending ink (INP-5, INP-6).
    fn read_ink(&mut self, now: u64) {
        let Some(ink) = self.ui.pending_ink.take() else { return };
        self.live_stroke = None;
        if self.pen == Some(Gesture::Writing) {
            self.pen = Some(Gesture::Ignored);
        }
        if self.game.completed || self.game.is_filled(ink.cell) {
            return;
        }
        // INP-6: the result is a number from 1 to 9, or "not 1–9" (the "?").
        let result = recognizer::recognize(&ink.strokes).filter(|d| (1..=9).contains(d));
        if let Some(log) = self.ink_log.as_mut() {
            log(&ink.strokes, result);
        }
        match result {
            Some(d) => {
                let o = self.game.add_number_with(ink.cell, d, self.settings.auto_remove_on);
                self.after_game_change(o);
            }
            None => self.ui.question = Some((ink.cell, now)),
        }
    }

    fn after_game_change(&mut self, o: Outcome) {
        if !o.changed() {
            return;
        }
        self.game_dirty = true;
        // Undo or redo can put a number in the cell of the "?". The "?" shows only on an empty cell
        // (QST-1).
        if self.ui.question.is_some_and(|(cell, _)| self.game.is_filled(cell)) {
            self.ui.question = None;
        }
        if o == Outcome::Completed {
            // §8.3: the board locks and the app saves the result.
            self.ui.highlight = None;
            self.ui.question = None;
            self.ui.pending_ink = None;
            self.live_stroke = None;
            if let Some(save) = self.save_game_now.as_mut() {
                if save(&self.game) {
                    self.game_dirty = false;
                }
            }
            let r = NewResult {
                finished_at: self.env.unix_time(),
                mode: self.game.mode,
                givens: self.game.givens_count(),
                seconds: self.game.seconds(),
                seed: self.game.seed,
            };
            match self.db.insert(&r) {
                Ok(_) => self.refresh_results(),
                Err(e) => eprintln!("sudoku: cannot save the result: {e}"),
            }
        }
    }

    fn refresh_results(&mut self) {
        if let Ok(all) = self.db.all() {
            self.results = all;
        }
    }

    // ---- Controls

    fn page_target(&self, x: f32, y: f32) -> Option<Target> {
        let hit = |r: Rect| r.contains(x, y);
        match self.screen {
            Screen::Game => None,
            Screen::Manual => {
                if hit(layout::back_button()) {
                    Some(Target::Back)
                } else if hit(layout::prev_arrow()) {
                    Some(Target::Prev)
                } else if hit(layout::next_arrow()) {
                    Some(Target::Next)
                } else {
                    None
                }
            }
            Screen::Statistics => {
                // The "Delete all games" dialog accepts only its two buttons (ANA-12).
                if self.confirm_all {
                    return if hit(layout::dialog_cancel()) {
                        Some(Target::AllCancel)
                    } else if hit(layout::dialog_delete()) {
                        Some(Target::AllDelete)
                    } else {
                        None
                    };
                }
                if hit(layout::back_button()) {
                    Some(Target::Back)
                } else if hit(layout::all_games_button()) {
                    Some(Target::AllGames)
                } else if hit(layout::delete_all_button()) && !self.results.is_empty() {
                    // Gray, with no effect, if there are no results.
                    Some(Target::DeleteAll)
                } else {
                    None
                }
            }
            Screen::GamesList => {
                // The dialog accepts only its two buttons (ANA-7).
                if self.list.confirm.is_some() {
                    return if hit(layout::dialog_cancel()) {
                        Some(Target::DialogCancel)
                    } else if hit(layout::dialog_delete()) {
                        Some(Target::DialogDelete)
                    } else {
                        None
                    };
                }
                if hit(layout::back_button()) {
                    return Some(Target::Back);
                }
                if hit(layout::prev_arrow()) {
                    return Some(Target::Prev);
                }
                if hit(layout::next_arrow()) {
                    return Some(Target::Next);
                }
                if let Some(i) = (0..=MODES as usize).find(|&i| hit(layout::filter_segment(i))) {
                    return Some(Target::Filter(i));
                }
                let rows = self.page_rows().len();
                (0..rows).find(|&i| hit(layout::bin_button(i))).map(Target::Bin)
            }
        }
    }

    fn target_rect_contains(&self, t: Target, x: f32, y: f32) -> bool {
        match t {
            Target::Game(b) => layout::game_button(x, y, self.ui.menu_open) == Some(b),
            Target::Color(s, i) => layout::color_at(x, y) == Some((s, i)),
            Target::StartNewGame => layout::ng_start_row().contains(x, y),
            Target::Switch(i) => layout::settings_at(x, y) == Some(i),
            _ => self.page_target(x, y) == Some(t),
        }
    }

    fn release_control(&mut self, t: Target, x: f32, y: f32) {
        if !self.target_rect_contains(t, x, y) {
            return;
        }
        match t {
            Target::Game(b) => self.game_button(b),
            Target::Color(s, i) => {
                if self.settings.select(Section::ALL[s], Color::ALL[i]) {
                    self.settings_dirty = true;
                }
            }
            Target::StartNewGame => self.start_new_game(),
            Target::Back => match self.screen {
                Screen::GamesList => self.screen = Screen::Statistics,
                _ => self.show_game(),
            },
            Target::Prev => self.swipe(false),
            Target::Next => self.swipe(true),
            Target::AllGames => {
                self.screen = Screen::GamesList;
                self.list = ListUi { filter: None, page: 1, confirm: None };
            }
            Target::Switch(i) => {
                // NAV-14: only the switch changes; the game and the undo history do not.
                if i == 0 {
                    self.settings.highlight_on = !self.settings.highlight_on;
                } else {
                    self.settings.auto_remove_on = !self.settings.auto_remove_on;
                }
                self.settings_dirty = true;
            }
            Target::DeleteAll => self.confirm_all = true,
            Target::AllCancel => self.confirm_all = false,
            Target::AllDelete => {
                if let Err(e) = self.db.delete_all() {
                    eprintln!("sudoku: cannot delete the results: {e}");
                }
                self.refresh_results();
                self.list = ListUi { filter: None, page: 1, confirm: None };
                self.confirm_all = false;
            }
            Target::Filter(i) => {
                self.list.filter = if i == 0 { None } else { Some(i as u8 - 1) };
                self.list.page = 1;
            }
            Target::Bin(i) => {
                if let Some(r) = self.page_rows().get(i) {
                    self.list.confirm = Some((*r).clone());
                }
            }
            Target::DialogCancel => self.list.confirm = None,
            Target::DialogDelete => {
                if let Some(r) = self.list.confirm.take() {
                    if let Err(e) = self.db.delete(r.id) {
                        eprintln!("sudoku: cannot delete the result: {e}");
                    }
                    self.refresh_results();
                    // ANA-8: if the page is now empty, show the page before it.
                    self.list.page = self.list.page.min(self.page_count());
                }
            }
        }
    }

    fn game_button(&mut self, b: Button) {
        match b {
            Button::Menu => {
                self.ui.menu_open = !self.ui.menu_open;
                self.ui.panel = Panel::None;
            }
            Button::Colors | Button::Settings | Button::NewGame => {
                let panel = match b {
                    Button::Colors => Panel::Colors,
                    Button::Settings => Panel::Settings,
                    _ => Panel::NewGame,
                };
                let open = self.ui.panel != panel;
                self.close_panel(); // DIF-3: the slider shows the last mode.
                if open {
                    self.ui.panel = panel;
                }
            }
            Button::Undo => {
                let o = self.game.undo();
                self.after_game_change(o);
            }
            Button::Redo => {
                let o = self.game.redo();
                self.after_game_change(o);
            }
            Button::Help => {
                self.leave_game();
                self.screen = Screen::Manual;
                self.manual_page = 1;
            }
            Button::Statistics => {
                self.leave_game();
                self.screen = Screen::Statistics;
                self.confirm_all = false;
            }
            Button::Close => {
                eprintln!("sudoku: the close button closes the app");
                self.game_dirty = true;
                self.quit = true;
            }
        }
    }

    fn close_panel(&mut self) {
        self.ui.panel = Panel::None;
        self.ui.slider = self.settings.last_mode; // DIF-4: the selection is not kept.
        self.cancel_gestures();
    }

    /// The panel, the screen or the game changed under the other pen or finger. Its touch has no more
    /// effect, thus its release does not operate the board of a different state (HLT-4, NAV-11).
    /// The touch that does the action is not in `pen` or `touch` now.
    fn cancel_gestures(&mut self) {
        if self.pen.is_some() {
            self.pen = Some(Gesture::Ignored);
            self.live_stroke = None;
        }
        if let Some((id, _, last)) = self.touch {
            self.touch = Some((id, Gesture::Ignored, last));
        }
    }

    fn leave_game(&mut self) {
        self.ui = GameUi::closed(self.settings.last_mode);
        self.live_stroke = None;
        self.cancel_gestures();
    }

    fn show_game(&mut self) {
        self.screen = Screen::Game;
        self.leave_game();
    }

    fn swipe(&mut self, next: bool) {
        match self.screen {
            Screen::Manual => {
                if next && self.manual_page < MANUAL_PAGES {
                    self.manual_page += 1;
                } else if !next && self.manual_page > 1 {
                    self.manual_page -= 1;
                }
            }
            Screen::GamesList if self.list.confirm.is_none() => {
                if next && self.list.page < self.page_count() {
                    self.list.page += 1;
                } else if !next && self.list.page > 1 {
                    self.list.page -= 1;
                }
            }
            _ => {}
        }
    }

    /// Start new game (§11).
    pub fn start_new_game(&mut self) {
        let mode = self.ui.slider;
        eprintln!("sudoku: start new game (mode {mode})");
        self.settings.last_mode = mode;
        self.settings_dirty = true;
        self.game = new_game(self.env.new_seed(), mode);
        self.game_dirty = true;
        self.ui = GameUi::closed(mode);
        self.live_stroke = None;
        self.cancel_gestures();
    }

    // ---- Analytics

    /// The results that agree with the filter, with the newest first (ANA-5).
    pub fn filtered(&self) -> Vec<&ResultRow> {
        self.results.iter().filter(|r| self.list.filter.map(|m| r.mode == m).unwrap_or(true)).collect()
    }

    pub fn page_count(&self) -> usize {
        self.filtered().len().div_ceil(PER_PAGE).max(1)
    }

    pub fn page_rows(&self) -> Vec<&ResultRow> {
        self.filtered().into_iter().skip((self.list.page - 1) * PER_PAGE).take(PER_PAGE).collect()
    }

    /// A value that changes when the screen changes, except for the clock and for new points of the
    /// ink (the platform draws those points without a full drawing). The key counts the strokes that
    /// show: a stroke with one point shows nothing, and a finished stroke shows the same as the pending
    /// ink that keeps it.
    pub fn view_key(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        let g = &self.game;
        (self.screen, g.value, g.marks, g.given, g.undo.len(), g.redo.len(), g.completed).hash(&mut h);
        if g.completed {
            g.seconds().hash(&mut h);
        }
        let ui = &self.ui;
        (ui.menu_open, ui.panel, ui.highlight, ui.question.map(|q| q.0), ui.slider).hash(&mut h);
        let pending = ui.pending_ink.as_ref().map_or(0, |i| i.strokes.len());
        let live = usize::from(self.live_stroke.as_ref().is_some_and(|s| s.len() >= 2));
        (pending + live).hash(&mut h);
        (self.manual_page, self.list.filter, self.list.page, self.list.confirm.as_ref().map(|r| r.id)).hash(&mut h);
        (self.settings.color_starting, self.settings.color_clash, self.settings.color_highlight).hash(&mut h);
        (self.settings.highlight_on, self.settings.auto_remove_on, self.confirm_all).hash(&mut h);
        self.results.len().hash(&mut h);
        self.results.iter().for_each(|r| r.id.hash(&mut h));
        h.finish()
    }

    /// The pen or a finger is on the screen now.
    pub fn is_touching(&self) -> bool {
        self.pen.is_some() || self.touch.is_some()
    }
}

pub fn new_game(seed: u64, mode: u8) -> Game {
    let p = generate(seed, mode);
    Game::new(seed, mode, p.solution, p.givens)
}

/// The average of the times, rounded to whole seconds. A half rounds to the even value, as in the
/// reference images (scripts/analytics.py).
pub fn average_seconds<'a>(rows: impl Iterator<Item = &'a ResultRow>) -> Option<u64> {
    let (n, sum) = rows.fold((0u64, 0u64), |(n, s), r| (n + 1, s + r.seconds));
    (n > 0).then(|| (sum as f64 / n as f64).round_ties_even() as u64)
}
