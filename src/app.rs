//! The app: screens, the menu, the panels, the overlays, input and the clock.
//!
//! The app does not do input or output. The platform gives it input events and the time. The platform
//! reads the flags `game_dirty` and `settings_dirty` and saves the data, and it stops when `quit` is
//! true. Refer to REQUIREMENTS.md §2, §3, §5, §8 to §13 and INVARIANTS.md.

use crate::game::{Cell, Game, Outcome};
use crate::generator::generate;
use crate::layout::{self, Button, Rect};
use crate::recognizer::{self, Stroke};
use crate::settings::{Color, LineGray, Section, Settings, Switch};
use crate::store::{NewResult, ResultRow, ResultsDb};
use crate::update::{Manifest, Update};

/// The recognition pause after the pen lifts (§5.1). `SUDOKU_RECOGNITION_PAUSE_MS` changes it.
pub const DEFAULT_PAUSE_MS: u64 = 500;
/// The time that the "?" shows (§5.1).
const QUESTION_MS: u64 = 1000;
/// The writing stops when the pen has written nothing on the board for this time after the last
/// stroke (§5.1, ink patch). Until then, a redraw waits. This time is longer than the recognition
/// pause, so that the redraw does not start while the pen moves to the next cell.
pub const WRITING_IDLE_MS: u64 = 1000;
/// A gap between two events that is longer than this is a suspension. The clock does not count it.
pub const MAX_CLOCK_GAP_MS: u64 = 5000;

pub const MANUAL_PAGES: u8 = 9;
/// A finger touch with no event for this time is forgotten when a new touch starts (INP-15).
const STALE_TOUCH_MS: u64 = 150;
/// After "< Back" from the manual or from Statistics, the app ignores a press on the toolbar and on
/// the close button for this time (NAV-16).
pub const BACK_GUARD_MS: u64 = 500;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Screen {
    Game,
    Manual,
    Statistics,
    GamesList,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Panel {
    Colors,
    Settings,
    NewGame,
}

/// The toolbar menu of the game screen (§9). A panel is open only when the menu is open: the
/// type makes INVARIANTS.md NAV-4 true.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Menu {
    Closed,
    /// The menu is open, with the open panel (if there is one).
    Open(Option<Panel>),
}

impl Menu {
    pub fn is_open(self) -> bool {
        matches!(self, Menu::Open(_))
    }

    /// The open panel, or `None` if no panel is open.
    pub fn panel(self) -> Option<Panel> {
        match self {
            Menu::Open(panel) => panel,
            Menu::Closed => None,
        }
    }
}

impl Panel {
    /// The panel that a toolbar button opens and closes (§9), or `None` if the button does not
    /// operate a panel.
    fn of_button(b: Button) -> Option<Panel> {
        match b {
            Button::Colors => Some(Panel::Colors),
            Button::Settings => Some(Panel::Settings),
            Button::NewGame => Some(Panel::NewGame),
            Button::Menu | Button::Undo | Button::Redo | Button::Help | Button::Statistics | Button::Close => None,
        }
    }
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
    pub menu: Menu,
    pub highlight: Option<u8>,
    pub question: Option<(Cell, u64)>,
    pub pending_ink: Option<PendingInk>,
    pub slider: u8,
}

impl GameUi {
    /// The game screen with the menu open, no panel, no highlight, no "?" and no ink. The game
    /// screen shows in this state when the app opens (PER-2), after "< Back" (NAV-3) and after
    /// Start new game (NAV-9).
    fn shown(slider: u8) -> GameUi {
        GameUi { menu: Menu::Open(None), ..GameUi::closed(slider) }
    }

    /// The state of the game screen when a different screen shows: the menu is closed (NAV-5).
    fn closed(slider: u8) -> GameUi {
        GameUi { menu: Menu::Closed, highlight: None, question: None, pending_ink: None, slider }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ListUi {
    /// None is "All".
    pub filter: Option<u8>,
    pub page: usize,
    pub confirm: Option<ResultRow>,
}

impl ListUi {
    /// The start state of the games list: all modes, the first page, no question open.
    fn start() -> ListUi {
        ListUi { filter: None, page: 1, confirm: None }
    }
}

/// Saves a game immediately; true if the save is done.
type SaveGame = Box<dyn FnMut(&Game) -> bool>;

/// Receives each reading of ink: the cell, the strokes and the result. Only an examination of the
/// recognizer on a device uses it (`platform`: the file `log-ink` in the data folder, or the corpus
/// build, `corpus.rs`).
pub type InkLog = Box<dyn FnMut(Cell, &[Stroke], Option<u8>)>;

/// An input on the board for `StrokeLog`.
pub enum BoardInput<'a> {
    /// A pen stroke that starts in this cell: also a tap, and a stroke that the app discards (a
    /// stroke on a filled cell).
    Stroke(Cell, &'a [recognizer::Point]),
    /// The eraser touched this cell.
    Erase(Cell),
}

/// Receives each pen stroke and each erasure on the board, with its cell. Only the corpus build
/// (`corpus.rs`) uses it: it keeps all strokes of a cell together, also when the app reads the ink
/// between two strokes.
pub type StrokeLog = Box<dyn FnMut(BoardInput)>;

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

/// The finger touch that the app follows (INP-15).
#[derive(Debug)]
struct FollowedTouch {
    id: i32,
    gesture: Gesture,
    /// The time of the last event of this touch.
    last_event_ms: u64,
}

impl FollowedTouch {
    /// A new press replaces this touch if the press has the same id (a press that AppLoad sends
    /// again), or if this touch has had no event for `STALE_TOUCH_MS` (a lost release).
    fn is_replaced_by_press(&self, id: i32, now: u64) -> bool {
        self.id == id || now.saturating_sub(self.last_event_ms) >= STALE_TOUCH_MS
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Target {
    Game(Button),
    /// A color in a section of the Cell colors panel.
    Color(Section, Color),
    /// A gray in the Micro-cell lines section of the Cell colors panel.
    LineGray(LineGray),
    StartNewGame,
    Back,
    Prev,
    Next,
    AllGames,
    /// A filter segment of the games list: All (`None`) or one mode.
    Filter(Option<u8>),
    Bin(usize),
    DialogCancel,
    DialogDelete,
    /// A switch row of the Settings panel.
    Switch(Switch),
    /// The button "Update to v<version>" (§9.2).
    Update,
    DeleteAll,
    AllCancel,
    AllDelete,
    /// "Later" in the dialog "Restart the interface?" (§9.2).
    RestartLater,
    /// "Restart" in the dialog "Restart the interface?" (§9.2).
    RestartNow,
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
    /// The update state (§9.2). The platform takes `update.request` and gives the results.
    pub update: Update,
    db: ResultsDb,
    env: Box<dyn Env>,

    pub pause_ms: u64,
    /// The x positions of the stops of the New game slider. They depend on the font (layout.rs).
    pub ng_stops: [f32; layout::STOPS as usize],
    /// The quantity of games on a page of the list in the current layout (`layout::list_rows`).
    per_page: usize,
    /// The stroke that the pen draws now (to show the ink).
    pub live_stroke: Option<Stroke>,
    /// The quantity of live strokes that began. The platform uses it to know that `live_stroke` is a
    /// new stroke, also when no full drawing occurred between two strokes.
    pub live_stroke_count: u64,
    /// The time when the writing stops (`WRITING_IDLE_MS` after the last stroke on the board). None:
    /// no stroke since the last press that does not write.
    pub(crate) writing_until: Option<u64>,
    /// Until this time, a press on the toolbar or on the close button changes nothing (NAV-16).
    /// "< Back" from the manual or from Statistics sets it. None: the toolbar operates.
    pub toolbar_ignored_until: Option<u64>,

    pen: Option<Gesture>,
    touch: Option<FollowedTouch>,
    last_clock: Option<u64>,

    pub game_dirty: bool,
    pub settings_dirty: bool,
    pub quit: bool,
    /// The user tapped "Restart" in the dialog "Restart the interface?" or the button "Restart
    /// interface" (§9.2). The platform saves the game and runs xovi/start. If the start fails, it calls `restart_failed`.
    pub restart_ui: bool,
    /// Saves the game immediately. The app calls it when a puzzle becomes complete, before it adds the
    /// result: thus a power loss can never record the same game two times (CMP-5).
    pub save_game_now: Option<SaveGame>,
    pub ink_log: Option<InkLog>,
    pub stroke_log: Option<StrokeLog>,
    /// The screen can be different from the last full drawing. False if the only change is new points
    /// of `live_stroke`: then the platform draws only the new part of the ink, so that the ink follows
    /// the pen.
    pub redraw: bool,
}

impl App {
    /// Makes the app from the saved data (PER-2). If there is no saved game, or the saved game is
    /// not correct (PER-5), the app makes a new game of the last selected mode.
    /// `ng_stops` are the slider stops of the current layout: `render::ng_stops` with the fonts of
    /// the tablet, or `App::placeholder_ng_stops` when there is no font.
    pub fn new(saved: Option<Game>, settings: Option<Settings>, db: ResultsDb, mut env: Box<dyn Env>, ng_stops: [f32; layout::STOPS as usize]) -> App {
        let settings = settings.filter(Settings::is_valid).unwrap_or_default();
        let game = match saved.filter(|g| crate::invariants::check_game(g).is_ok() && is_generated(g)) {
            Some(g) => g,
            None => new_game(env.new_seed(), settings.last_mode),
        };
        let results = db.all().unwrap_or_default();
        let slider = settings.last_mode;
        App {
            screen: Screen::Game,
            game,
            ui: GameUi::shown(slider),
            manual_page: 1,
            list: ListUi::start(),
            per_page: crate::layout::list_rows(),
            confirm_all: false,
            settings,
            results,
            update: Update::new(crate::VERSION),
            db,
            env,
            pause_ms: DEFAULT_PAUSE_MS,
            ng_stops,
            live_stroke: None,
            live_stroke_count: 0,
            writing_until: None,
            toolbar_ignored_until: None,
            pen: None,
            touch: None,
            last_clock: None,
            game_dirty: true,
            settings_dirty: false,
            quit: false,
            restart_ui: false,
            redraw: true,
            save_game_now: None,
            ink_log: None,
            stroke_log: None,
        }
    }

    /// The New game slider stops with approximate text widths of the labels "Easy" and "Hard", for
    /// a caller that has no font (most tests). `render::ng_stops` uses the true widths of the font.
    pub fn placeholder_ng_stops() -> [f32; layout::STOPS as usize] {
        const EASY_LABEL_W: f32 = 50.0;
        const HARD_LABEL_W: f32 = 70.0;
        layout::slider_stops(layout::ng_tx(), layout::ng_tr(), EASY_LABEL_W, HARD_LABEL_W)
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
        if self.update.tick(now) {
            self.redraw = true;
        }

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
        if self.writing_until.is_some_and(|t| now >= t) {
            self.writing_until = None;
        }
        if self.toolbar_ignored_until.is_some_and(|t| now >= t) {
            self.toolbar_ignored_until = None;
        }
    }

    /// True while the player writes: from the first stroke on the board until the app has read the
    /// last ink and no stroke occurred for `WRITING_IDLE_MS`. A press that does not write (a tap, the
    /// eraser, a button) stops the writing. The presenter keeps a redraw until the writing stops.
    pub fn is_writing(&self) -> bool {
        self.live_stroke.is_some() || self.ui.pending_ink.is_some() || self.writing_until.is_some()
    }

    /// The live stroke if the touch is writing. While the pen stays in the micro-cell where it
    /// touched the board, the touch can still be a tap (INP-2): then None. The ink of the live stroke
    /// shows also during a tap, but only writing starts the pen region (`present::wanted_region`).
    pub fn writing_stroke(&self) -> Option<&Stroke> {
        self.live_stroke.as_ref().filter(|_| !matches!(self.pen, Some(Gesture::BoardTap { .. })))
    }

    /// The tablet turned. The positions of ink and of touches are not correct now: the app removes the
    /// ink that waits and stops the touches. The game does not change.
    /// `ng_stops` are the slider stops of the new layout (`render::ng_stops`).
    /// The games list keeps the first game of its page visible, because a page of the landscape layout
    /// has fewer games (`layout::list_rows`).
    pub fn rotate(&mut self, ng_stops: [f32; layout::STOPS as usize]) {
        self.ng_stops = ng_stops;
        let first = (self.list.page - 1) * self.per_page;
        self.per_page = crate::layout::list_rows();
        self.list.page = (first / self.per_page + 1).min(self.page_count());
        self.ui.pending_ink = None;
        self.live_stroke = None;
        self.writing_until = None;
        self.pen = None;
        self.touch = None;
        self.redraw = true;
    }

    /// The app is suspended or closed: the clock stops (CLK-5). Ink that waits is removed (PER-6).
    pub fn suspend(&mut self) {
        self.last_clock = None;
        self.ui.pending_ink = None;
        self.live_stroke = None;
        self.writing_until = None;
        self.pen = None;
        self.touch = None;
        self.game_dirty = true;
        self.redraw = true;
    }

    /// The time of the next timer, if a timer runs.
    pub fn next_deadline(&self) -> Option<u64> {
        let q = self.ui.question.map(|(_, t)| t + QUESTION_MS);
        let ink = self.ui.pending_ink.as_ref().and_then(|i| i.last_lift_at).map(|t| t + self.pause_ms);
        [q, ink, self.writing_until].into_iter().flatten().min()
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
                // touches. A real second finger (an active touch) operates no item.
                if self.touch.as_ref().is_none_or(|t| t.is_replaced_by_press(id, now)) {
                    let gesture = self.press(x, y, None, now);
                    self.touch = Some(FollowedTouch { id, gesture, last_event_ms: now });
                } else {
                    // A second finger has no other effect. But it is an action: it removes the
                    // highlight and the "?" (HLT-2, QST-4).
                    self.ui.highlight = None;
                    self.ui.question = None;
                }
            }
            Input::TouchMove { id, x, y } => {
                if let Some(t) = self.touch.take_if(|t| t.id == id) {
                    let gesture = self.finger_move(t.gesture, x, y);
                    self.touch = Some(FollowedTouch { id, gesture, last_event_ms: now });
                }
            }
            Input::TouchUp { id, x, y } => {
                if let Some(t) = self.touch.take_if(|t| t.id == id) {
                    self.finger_up(t.gesture, x, y, now);
                }
            }
        }
        // §9.2: "Sudoku is up to date" shows only until the Settings panel closes.
        if self.ui.menu.panel() != Some(Panel::Settings) || self.screen != Screen::Game {
            self.update.panel_closed();
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
        // NAV-16: shortly after "< Back", a press on the toolbar or on the close button changes
        // nothing, also not the highlight, the "?" or the waiting ink. It is often the second tap of a
        // fast double tap on "< Back", which is at the position of the close button (left-handed) or
        // of the menu button (right-handed).
        let toolbar_ignored = self.toolbar_ignored_until.is_some_and(|t| now < t);
        if toolbar_ignored && self.screen == Screen::Game && layout::game_button(x, y, self.ui.menu.is_open()).is_some() {
            return Gesture::Ignored;
        }
        // Each action removes the highlight before the app does the action (HLT-2).
        let prev_highlight = self.ui.highlight.take();
        self.ui.question = None;

        // The dialog "Restart the interface?" accepts only its two buttons (UPD-10). It covers all
        // screens, thus a pen or a finger has no other effect.
        if self.update.dialog_shown() {
            return if layout::dialog_cancel().contains(x, y) {
                Gesture::Control(Target::RestartLater)
            } else if layout::dialog_delete().contains(x, y) {
                Gesture::Control(Target::RestartNow)
            } else {
                Gesture::Ignored
            };
        }

        // A press on the cell of the waiting ink, on the game screen with no panel.
        let cell = layout::board_cell(x, y);
        let on_pending_ink = self.screen == Screen::Game
            && self.ui.menu.panel().is_none()
            && cell.is_some()
            && self.ui.pending_ink.as_ref().is_some_and(|ink| Some(ink.cell) == cell);
        if on_pending_ink {
            match tool {
                // The pen continues the ink: the ink gets the new stroke.
                Some(Tool::Pen) => {
                    let ink = self.ui.pending_ink.as_mut().expect("pending ink");
                    ink.last_lift_at = None;
                    self.start_live_stroke(x, y);
                    return Gesture::Writing;
                }
                // The eraser removes the ink. The app does not read it, thus the board and the undo
                // history do not change (INP-8). The press continues below, as a press on the board.
                Some(Tool::Eraser) => {
                    self.ui.pending_ink = None;
                    self.live_stroke = None;
                }
                None => {}
            }
        }
        // Ink that waits in a different cell is read now.
        //
        // A finger on the board has no effect on the game (INP-1), thus it does not read the ink (for
        // example a palm on the screen between two strokes of a 4).
        let finger_on_board = tool.is_none() && self.screen == Screen::Game && cell.is_some();
        if self.ui.pending_ink.is_some() && !finger_on_board {
            self.read_ink(now);
        }
        // A press stops the writing: a redraw of its result must not wait. If the pen writes, the
        // live stroke continues the writing.
        if !finger_on_board {
            self.writing_until = None;
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
        let button = layout::game_button(x, y, self.ui.menu.is_open());

        // A panel is open: a tap outside the panel only closes the panel (NAV-11). The paint bucket and
        // the + also change the panel (§9).
        if let Some(panel) = self.ui.menu.panel() {
            if let Some(b) = button.filter(|&b| Panel::of_button(b).is_some()) {
                return Gesture::Control(Target::Game(b));
            }
            match panel {
                Panel::Settings if layout::settings_panel(self.update.has_row()).contains(x, y) => {
                    if self.update.button().is_some() && layout::update_button_area().contains(x, y) {
                        return Gesture::Control(Target::Update);
                    }
                    return layout::settings_at(x, y).map(|s| Gesture::Control(Target::Switch(s))).unwrap_or(Gesture::Ignored);
                }
                Panel::Colors if layout::colors_panel().contains(x, y) => {
                    return layout::color_at(x, y)
                        .map(|(s, c)| Target::Color(s, c))
                        .or_else(|| layout::line_gray_at(x, y).map(Target::LineGray))
                        .map(Gesture::Control)
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
            // The pen motion has no effect: a control acts on the release, and a swipe is a finger gesture.
            Gesture::Ignored | Gesture::Control(_) | Gesture::Swipe { .. } => g,
        }
    }

    fn pen_up(&mut self, g: Gesture, x: f32, y: f32, now: u64) {
        match g {
            Gesture::BoardTap { cell, digit, prev_highlight } => {
                if let (Some(log), Some(s)) = (self.stroke_log.as_mut(), self.live_stroke.as_ref()) {
                    log(BoardInput::Stroke(cell, s));
                }
                self.live_stroke = None;
                self.board_tap(cell, digit, prev_highlight);
            }
            Gesture::Writing => {
                let stroke = self.live_stroke.take().unwrap_or_default();
                if let Some(ink) = self.ui.pending_ink.as_mut() {
                    if let Some(log) = self.stroke_log.as_mut() {
                        log(BoardInput::Stroke(ink.cell, &stroke));
                    }
                    ink.strokes.push(stroke);
                    ink.last_lift_at = Some(now);
                }
                self.writing_until = Some(now + WRITING_IDLE_MS);
            }
            Gesture::DiscardInk => {
                let stroke = self.live_stroke.take().unwrap_or_default();
                let cell = stroke.first().and_then(|&(x, y)| layout::board_cell(x, y));
                if let (Some(log), Some(cell)) = (self.stroke_log.as_mut(), cell) {
                    log(BoardInput::Stroke(cell, &stroke));
                }
                self.writing_until = Some(now + WRITING_IDLE_MS);
            }
            Gesture::Control(t) => self.release_control(t, x, y, now),
            // The erasure and the slider move occur during the motion. The release has no effect.
            Gesture::Ignored | Gesture::Eraser | Gesture::Slider | Gesture::Swipe { .. } => {}
        }
    }

    fn finger_move(&mut self, g: Gesture, x: f32, _y: f32) -> Gesture {
        if g == Gesture::Slider {
            self.ui.slider = layout::nearest_stop(&self.ng_stops, x);
        }
        g
    }

    fn finger_up(&mut self, g: Gesture, x: f32, y: f32, now: u64) {
        match g {
            Gesture::Control(t) => self.release_control(t, x, y, now),
            Gesture::Swipe { x0, y0, target } => {
                let (dx, dy) = (x - x0, y - y0);
                if dx.abs() >= layout::SWIPE_MIN && dx.abs() > dy.abs() {
                    self.swipe(dx < 0.0);
                } else if let Some(t) = target {
                    self.release_control(t, x, y, now);
                }
            }
            // The slider moves during the motion. A finger cannot make the other gestures, or they have no effect.
            Gesture::Ignored
            | Gesture::Slider
            | Gesture::BoardTap { .. }
            | Gesture::Writing
            | Gesture::DiscardInk
            | Gesture::Eraser => {}
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
            let o = self.game.toggle_mark_with(cell, digit, self.settings.auto_hide_on);
            self.after_game_change(o);
        }
    }

    fn erase_at(&mut self, x: f32, y: f32) {
        if let Some(cell) = layout::board_cell(x, y) {
            if let Some(log) = self.stroke_log.as_mut() {
                log(BoardInput::Erase(cell));
            }
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
            log(ink.cell, &ink.strokes, result);
        }
        match result {
            Some(d) => {
                let o = self.game.add_number(ink.cell, d);
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
            self.writing_until = None;
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
                generator: self.game.generator,
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
                if let Some(filter) = layout::list_filters().find(|&f| hit(layout::filter_segment(f))) {
                    return Some(Target::Filter(filter));
                }
                let rows = self.page_rows().len();
                (0..rows).find(|&i| hit(layout::bin_button(i))).map(Target::Bin)
            }
        }
    }

    fn target_rect_contains(&self, t: Target, x: f32, y: f32) -> bool {
        match t {
            Target::Game(b) => layout::game_button(x, y, self.ui.menu.is_open()) == Some(b),
            Target::Color(s, c) => layout::color_at(x, y) == Some((s, c)),
            Target::LineGray(g) => layout::line_gray_at(x, y) == Some(g),
            Target::StartNewGame => layout::ng_start_row().contains(x, y),
            Target::Switch(s) => layout::settings_at(x, y) == Some(s),
            Target::Update => layout::update_button_area().contains(x, y),
            Target::RestartLater => layout::dialog_cancel().contains(x, y),
            Target::RestartNow => layout::dialog_delete().contains(x, y),
            // The controls of the manual, the statistics and the games list.
            Target::Back
            | Target::Prev
            | Target::Next
            | Target::AllGames
            | Target::Filter(_)
            | Target::Bin(_)
            | Target::DialogCancel
            | Target::DialogDelete
            | Target::DeleteAll
            | Target::AllCancel
            | Target::AllDelete => self.page_target(x, y) == Some(t),
        }
    }

    fn release_control(&mut self, t: Target, x: f32, y: f32, now: u64) {
        if !self.target_rect_contains(t, x, y) {
            return;
        }
        // The release does the action of the control. While a finger stays on the control, the pen can
        // make a highlight or write ink. Thus the release also removes the highlight (HLT-2, HLT-4) and
        // reads the waiting ink (INP-12) before the action, as a press does. Then an undo or a redo
        // cannot put a number under the ink (INP-5), and a panel does not open over the ink (NAV-11).
        self.ui.highlight = None;
        if self.ui.pending_ink.is_some() {
            self.read_ink(now);
        }
        match t {
            Target::Game(b) => self.game_button(b),
            Target::Color(s, c) => {
                if self.settings.select(s, c) {
                    self.settings_dirty = true;
                }
            }
            Target::LineGray(g) => {
                if self.settings.micro_line != g {
                    self.settings.micro_line = g;
                    self.settings_dirty = true;
                }
            }
            Target::StartNewGame => self.start_new_game(),
            Target::Back => match self.screen {
                Screen::GamesList => self.screen = Screen::Statistics,
                _ => self.show_game(now),
            },
            Target::Prev => self.swipe(false),
            Target::Next => self.swipe(true),
            Target::AllGames => {
                self.screen = Screen::GamesList;
                self.list = ListUi::start();
            }
            Target::Switch(s) => {
                // NAV-14: only the switch changes; the game and the undo history do not.
                self.settings.toggle(s);
                self.settings_dirty = true;
            }
            // UPD-5, UPD-9: only the update state changes. UPD-10: "Restart interface" asks the
            // platform to run xovi/start, after the game is saved.
            Target::Update => {
                if self.update.button() == Some(crate::update::UpdateButton::Restart) {
                    self.request_restart();
                } else {
                    self.update.tap();
                }
            }
            Target::DeleteAll => self.confirm_all = true,
            Target::AllCancel => self.confirm_all = false,
            Target::AllDelete => {
                if let Err(e) = self.db.delete_all() {
                    eprintln!("sudoku: cannot delete the results: {e}");
                }
                self.refresh_results();
                self.list = ListUi::start();
                self.confirm_all = false;
            }
            Target::Filter(filter) => {
                self.list.filter = filter;
                self.list.page = 1;
            }
            Target::Bin(i) => {
                if let Some(r) = self.page_rows().get(i) {
                    self.list.confirm = Some((*r).clone());
                }
            }
            Target::DialogCancel => self.list.confirm = None,
            // UPD-10: only the dialog changes. The row continues to show the button "Restart interface".
            Target::RestartLater => self.update.close_dialog(),
            Target::RestartNow => self.request_restart(),
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
                self.ui.menu = if self.ui.menu.is_open() { Menu::Closed } else { Menu::Open(None) };
            }
            Button::Colors => self.toggle_panel(Panel::Colors),
            Button::Settings => self.toggle_panel(Panel::Settings),
            Button::NewGame => self.toggle_panel(Panel::NewGame),
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

    /// Opens the panel, or closes it if it is open. An open panel of a different button closes.
    fn toggle_panel(&mut self, panel: Panel) {
        let open = self.ui.menu.panel() != Some(panel);
        self.close_panel(); // DIF-3: the slider shows the last mode.
        if open {
            self.ui.menu = Menu::Open(Some(panel));
        }
    }

    fn close_panel(&mut self) {
        // The panel closes, and the menu stays as it is (open or closed).
        if self.ui.menu.is_open() {
            self.ui.menu = Menu::Open(None);
        }
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
        if let Some(t) = &mut self.touch {
            t.gesture = Gesture::Ignored;
        }
    }

    fn leave_game(&mut self) {
        self.ui = GameUi::closed(self.settings.last_mode);
        self.live_stroke = None;
        self.toolbar_ignored_until = None;
        self.cancel_gestures();
    }

    /// "< Back" from the manual or from Statistics at the time `now`: the game shows with the menu
    /// open and no panel (NAV-3). For `BACK_GUARD_MS`, the toolbar and the close button ignore a
    /// press (NAV-16).
    fn show_game(&mut self, now: u64) {
        self.screen = Screen::Game;
        self.leave_game();
        self.ui.menu = Menu::Open(None);
        self.toolbar_ignored_until = Some(now + BACK_GUARD_MS);
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
        // The New game panel closes and the menu stays open (NAV-9).
        self.ui = GameUi::shown(mode);
        self.live_stroke = None;
        self.cancel_gestures();
    }

    // ---- Analytics

    /// The results that agree with the filter, with the newest first (ANA-5).
    pub fn filtered(&self) -> Vec<&ResultRow> {
        self.results.iter().filter(|r| self.list.filter.map(|m| r.mode == m).unwrap_or(true)).collect()
    }

    pub fn page_count(&self) -> usize {
        self.filtered().len().div_ceil(self.per_page).max(1)
    }

    pub fn page_rows(&self) -> Vec<&ResultRow> {
        self.filtered().into_iter().skip((self.list.page - 1) * self.per_page).take(self.per_page).collect()
    }

    /// A value that changes when the screen changes, except for the clock and for new points of the
    /// ink (the platform draws those points without a full drawing). The key counts the strokes that
    /// show: a stroke with one point shows nothing, and a finished stroke shows the same as the pending
    /// ink that keeps it.
    fn view_key(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        let g = &self.game;
        (self.screen, g.value, g.marks, g.given, g.undo.len(), g.redo.len(), g.completed).hash(&mut h);
        if g.completed {
            g.seconds().hash(&mut h);
        }
        let ui = &self.ui;
        (ui.menu, ui.highlight, ui.question.map(|q| q.0), ui.slider).hash(&mut h);
        let pending = ui.pending_ink.as_ref().map_or(0, |i| i.strokes.len());
        let live = usize::from(self.live_stroke.as_ref().is_some_and(|s| s.len() >= 2));
        (pending + live).hash(&mut h);
        (self.manual_page, self.list.filter, self.list.page, self.list.confirm.as_ref().map(|r| r.id)).hash(&mut h);
        (self.settings.color_starting, self.settings.color_clash, self.settings.color_highlight).hash(&mut h);
        (self.settings.highlight_on, self.settings.auto_hide_on, self.settings.micro_line, self.confirm_all).hash(&mut h);
        (&self.update.current, &self.update.state, self.update.row_text()).hash(&mut h);
        self.results.len().hash(&mut h);
        self.results.iter().for_each(|r| r.id.hash(&mut h));
        h.finish()
    }

    /// The result of an update request from the platform (§9.2).
    pub fn update_checked(&mut self, result: Result<Manifest, String>) {
        self.update.checked(result, self.ui.menu.panel() == Some(Panel::Settings) && self.screen == Screen::Game);
        self.redraw = true;
    }

    /// The result of an installation from the platform (§9.2). `Ok(true)`: the installation replaced
    /// the ink patch. Then the dialog "Restart the interface?" shows over the screen (UPD-10): the app
    /// reads the waiting ink first, and the touches that are on the screen have no more effect.
    pub fn update_installed(&mut self, result: Result<bool, String>, now: u64) {
        self.update.installed(result, now);
        if self.update.dialog_shown() {
            if self.ui.pending_ink.is_some() {
                self.read_ink(now);
            }
            self.ui.highlight = None;
            self.ui.question = None;
            self.live_stroke = None;
            self.cancel_gestures();
        }
        self.redraw = true;
    }

    /// "Restart" in the dialog or the button "Restart interface" (UPD-10): the platform saves the game
    /// and runs xovi/start.
    fn request_restart(&mut self) {
        eprintln!("sudoku: restart the interface (xovi/start)");
        self.game_dirty = true;
        self.restart_ui = true;
    }

    /// The platform could not run xovi/start: the dialog closes, and the row continues to show the
    /// button "Restart interface".
    pub fn restart_failed(&mut self) {
        self.restart_ui = false;
        self.update.close_dialog();
        self.redraw = true;
    }
}

pub fn new_game(seed: u64, mode: u8) -> Game {
    let p = generate(seed, mode);
    Game::new(seed, mode, p.solution, p.givens)
}

/// True if the starting numbers and the solution are the puzzle of `(seed, mode)` with the version of
/// the generator of the game (PUZ-5). The app examines this only when it opens a saved game, because
/// the generator is slow for each input. `check_game` first makes sure that the version and the mode
/// are possible.
fn is_generated(g: &Game) -> bool {
    let p = crate::generator::generate_version(g.generator, g.seed, g.mode);
    let given = p.givens.map(|row| row.map(|v| v != 0));
    p.solution == g.solution && given == g.given
}

/// The average of the times, rounded to whole seconds. A half rounds to the even value, as in the
/// reference images (scripts/analytics.py).
pub fn average_seconds<'a>(rows: impl Iterator<Item = &'a ResultRow>) -> Option<u64> {
    let (n, sum) = rows.fold((0u64, 0u64), |(n, s), r| (n + 1, s + r.seconds));
    (n > 0).then(|| (sum as f64 / n as f64).round_ties_even() as u64)
}
