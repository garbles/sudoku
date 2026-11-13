//! The app: screens, the menu, the panels, the overlays, input and the clock.
//!
//! The app does not do input or output. The platform gives it input events and the time. The platform
//! reads the flags `game_dirty` and `settings_dirty` and saves the data, and it stops when `quit` is
//! true. Refer to REQUIREMENTS.md §2, §3, §5, §8 to §13 and INVARIANTS.md.

use crate::game::{Cell, Game, Outcome};
use crate::generator::{generate_early, generate_with_symmetry, PuzzleKey};
use crate::launch::Launch;
use crate::layout::{self, Button, CheckpointControl, LevelButton, Rect};
use crate::recognizer::{self, Stroke};
use crate::settings::{Color, LineGray, Section, Settings, Switch};
use crate::store::{NewResult, ResultRow, ResultsDb};
use crate::update::{Manifest, Update};

/// The recognition pause after the pen lifts (§5.1, INP-5). This is the only source of the value: no
/// AppLoad manifest of the app (`manifest.json` and `external.manifest.json`) sets it, and no
/// environment variable changes it (INP-23).
pub const RECOGNITION_PAUSE_MS: u64 = 1000;
/// The time that the "?" and the x of a refused tap show (§5.1, §6.1, QST-1, MRK-14).
pub const OVERLAY_MS: u64 = 1000;
/// The writing stops when the pen has written nothing on the board for this time after the last
/// stroke (§5.1, §14.4, pen area). Until then, a redraw waits, so that the redraw does not start while
/// the pen moves to the next cell. This time is equal to the recognition pause (INP-16).
pub const WRITING_IDLE_MS: u64 = RECOGNITION_PAUSE_MS;
/// A touch that starts less than this time after a pen lift is a hand touch: it has no effect (INP-20).
/// The tablet sends a touch from the hand approximately 97 ms after each pen lift.
pub const HAND_TOUCH_MS: u64 = 150;
/// A gap between two events that is longer than this is a suspension. The clock does not count it.
pub const MAX_CLOCK_GAP_MS: u64 = 5000;

pub const MANUAL_PAGES: u8 = 9;
/// A finger touch with no event for this time is forgotten when a new touch starts (INP-15).
const STALE_TOUCH_MS: u64 = 150;
/// After "< Back" from the manual or from Statistics, the app ignores a press on the toolbar and on
/// the close button for this time (NAV-16).
pub const BACK_GUARD_MS: u64 = 500;
/// After a delete of a checkpoint, a press in the area of the Checkpoints panel changes nothing for
/// this time (CKP-8).
pub const DELETE_GUARD_MS: u64 = 500;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Screen {
    Game,
    Manual,
    Statistics,
    GamesList,
    /// The list of the third-party crates and their licenses.
    Licenses,
    /// The license texts of one crate.
    LicenseText,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Panel {
    Colors,
    Settings,
    NewGame,
    Checkpoints,
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
            Button::Checkpoints => Some(Panel::Checkpoints),
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
    /// The x of a refused tap: the cell, the digit of the micro-cell and the time when it showed
    /// (MRK-10 to MRK-18). The app does not save it (MRK-17).
    pub refused: Option<(Cell, u8, u64)>,
    pub pending_ink: Option<PendingInk>,
    pub slider: u8,
    /// The selection of the Symmetrical switch in the New game panel (DIF-8). When the panel is not
    /// open, it is `settings.last_symmetric`.
    pub symmetric_sel: bool,
    /// The time when the guard after a delete of a checkpoint ends (CKP-8). The app does
    /// not save it. It is None when the Checkpoints panel is not open.
    pub delete_guard_until: Option<u64>,
}

impl GameUi {
    /// The game screen with the menu open, no panel, no highlight, no "?", no x and no ink. The game
    /// screen shows in this state when the app opens (PER-2), after "< Back" (NAV-3) and after
    /// Start new game (NAV-9). The selections of the New game panel are the saved values (DIF-3).
    fn shown(settings: &Settings) -> GameUi {
        GameUi { menu: Menu::Open(None), ..GameUi::closed(settings) }
    }

    /// The state of the game screen when a different screen shows (NAV-5), and after a level button
    /// of the result card (NAV-9): the menu is closed.
    fn closed(settings: &Settings) -> GameUi {
        GameUi {
            menu: Menu::Closed,
            highlight: None,
            question: None,
            refused: None,
            pending_ink: None,
            slider: settings.last_mode,
            symmetric_sel: settings.last_symmetric,
            delete_guard_until: None,
        }
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

/// The state of the Licenses screens.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LicensesUi {
    /// The page of the list (from 1).
    pub page: usize,
    /// The crate that the license text screen shows (an index of `licenses::all().crates`).
    pub open: usize,
    /// The page of the license text (from 1).
    pub text_page: usize,
}

impl LicensesUi {
    fn start() -> LicensesUi {
        LicensesUi { page: 1, open: 0, text_page: 1 }
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
    /// A pen press on an empty cell. It is a tap while it stays in the micro-cell. It shows ink, and
    /// the pen area covers its cell from the pen down (INP-24). `highlight_at_press`: refer to
    /// `App::board_tap`.
    BoardTap { cell: Cell, digit: u8, highlight_at_press: Option<u8> },
    /// Writing in an empty cell. The stroke goes into the pending ink.
    Writing,
    /// The touch started on an empty cell, and a finger Redo filled that cell before the pen left the
    /// micro-cell. The touch has no effect. The app removes its ink on the release.
    DiscardInk { cell: Cell },
    /// A pen press on a filled cell. It is a tap while it stays in the micro-cell. It shows no ink and
    /// starts no pen area (INP-22). `stroke` is only for `StrokeLog`. `highlight_at_press`: refer to
    /// `App::board_tap`.
    FilledTap { cell: Cell, digit: u8, stroke: Stroke, highlight_at_press: Option<u8> },
    /// A stroke that started on a filled cell. It shows no ink and starts no pen area (INP-22).
    /// `stroke` is only for `StrokeLog`.
    FilledStroke { cell: Cell, stroke: Stroke },
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
    /// A control of the Checkpoints panel.
    Checkpoint(CheckpointControl),
    StartNewGame,
    /// The Symmetrical row of the New game panel (DIF-8).
    Symmetric,
    /// A level button of the result card (CMP-10). Only an available button is a target.
    Level(LevelButton),
    Back,
    Prev,
    Next,
    AllGames,
    /// A filter segment of the games list: All (`None`) or one mode.
    Filter(Option<u8>),
    Bin(usize),
    /// The row "Third-party licenses" of the Settings panel.
    Licenses,
    /// Row i of a page of the Licenses list.
    LicenseRow(usize),
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
    pub licenses: LicensesUi,
    /// The "Delete all games" dialog shows on the summary (ANA-12).
    pub confirm_all: bool,
    pub settings: Settings,
    pub results: Vec<ResultRow>,
    /// The update state (§9.2). The platform takes `update.request` and gives the results. It also
    /// has the launch mode of the program and the restart dialog (`Update::launch`, LCH-9, UPD-10).
    pub update: Update,
    db: ResultsDb,
    env: Box<dyn Env>,

    /// The recognition pause. It is always `RECOGNITION_PAUSE_MS` in the app (INP-23). Only a test
    /// changes it, to read ink in a time that is shorter than the pause.
    pub pause_ms: u64,
    /// The x positions of the stops of the New game slider. They depend on the font (layout.rs).
    pub ng_stops: [f32; layout::STOPS as usize],
    /// The quantity of games on a page of the list in the current layout (`layout::list_rows`).
    per_page: usize,
    /// The quantity of crates on a page of the Licenses list (`layout::licenses_rows`).
    licenses_per_page: usize,
    /// The quantity of lines on a page of license text (`layout::license_lines`).
    license_lines: usize,
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
    /// The time of the last pen lift, to find a hand touch (INP-20).
    last_pen_lift: Option<u64>,
    last_clock: Option<u64>,
    /// The seed of the next new game (DIF-5: a new seed for each game). The app draws it when the
    /// New game panel is open or the result card shows, so that the generator can make the puzzle
    /// before Start new game or a level button.
    pub(crate) next_seed: Option<u64>,
    /// The puzzle `(next_seed, mode, symmetric)` that the generator makes in the background for the
    /// next game (PUZ-7, `start_early_game`). None: no puzzle started since the last new game.
    pub(crate) early_game: Option<PuzzleKey>,

    pub game_dirty: bool,
    pub settings_dirty: bool,
    pub quit: bool,
    /// The user tapped "Restart" in the dialog "Restart the interface?" or the button "Restart
    /// interface" (§9.2), which show only while the update is not complete (UPD-10). The platform
    /// saves the game and runs xovi/start. If the start fails, it calls `restart_failed`.
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
    /// The app starts in the frontend mode (the usual mode): `with_launch` gives a different mode.
    pub fn new(saved: Option<Game>, settings: Option<Settings>, db: ResultsDb, env: Box<dyn Env>, ng_stops: [f32; layout::STOPS as usize]) -> App {
        App::with_launch(Launch::Frontend, saved, settings, db, env, ng_stops)
    }

    /// Makes the app as `new`, for a program that AppLoad started in the mode `launch` (LCH-9).
    ///
    /// - The frontend mode sets the saved value `restart_dialog_shown` to false (UPD-13).
    /// - The external mode with the frontend files in the app folder (`Launch::external(true)`): the
    ///   update is not complete. The dialog "Restart the interface?" shows exactly when the saved
    ///   value is false, and the value becomes true (UPD-10). The app shows the game screen at each
    ///   start, no ink waits, and the board has no highlight, no "?" and no x: the dialog shows over
    ///   that screen. The `dialog` of `launch` has no effect.
    ///
    /// A change of the saved value sets `settings_dirty`, thus the platform saves it.
    pub fn with_launch(
        launch: Launch,
        saved: Option<Game>,
        settings: Option<Settings>,
        db: ResultsDb,
        mut env: Box<dyn Env>,
        ng_stops: [f32; layout::STOPS as usize],
    ) -> App {
        let mut settings = settings.filter(Settings::is_valid).unwrap_or_default();
        let (launch, dialog_shown) = launch.at_start(settings.restart_dialog_shown);
        let settings_dirty = dialog_shown != settings.restart_dialog_shown;
        settings.restart_dialog_shown = dialog_shown;
        // A game file of an earlier version can have more checkpoints than the maximum: keep the
        // newest (the list is newest first).
        let saved = saved.map(|mut g| {
            g.checkpoints.truncate(crate::game::MAX_CHECKPOINTS);
            g
        });
        let game = match saved.filter(|g| crate::invariants::check_game(g).is_ok() && is_generated(g)) {
            Some(g) => g,
            None => new_game_with_symmetry(env.new_seed(), settings.last_mode, settings.last_symmetric),
        };
        let results = db.all().unwrap_or_default();
        let mut app = App {
            screen: Screen::Game,
            game,
            ui: GameUi::shown(&settings),
            manual_page: 1,
            list: ListUi::start(),
            licenses: LicensesUi::start(),
            per_page: crate::layout::list_rows(),
            licenses_per_page: layout::licenses_rows(),
            license_lines: layout::license_lines(),
            confirm_all: false,
            settings,
            results,
            update: Update { launch, ..Update::new(crate::VERSION) },
            db,
            env,
            pause_ms: RECOGNITION_PAUSE_MS,
            ng_stops,
            live_stroke: None,
            live_stroke_count: 0,
            writing_until: None,
            toolbar_ignored_until: None,
            pen: None,
            touch: None,
            last_pen_lift: None,
            last_clock: None,
            next_seed: None,
            early_game: None,
            game_dirty: true,
            settings_dirty,
            quit: false,
            restart_ui: false,
            redraw: true,
            save_game_now: None,
            ink_log: None,
            stroke_log: None,
        };
        // PUZ-7: a completed game from the save shows the result card, thus the puzzle of Same
        // starts now.
        app.start_early_game();
        app
    }

    /// The New game slider stops with approximate text widths of the labels "Easy" and "Hard", for
    /// a caller that has no font (most tests). `render::ng_stops` uses the true widths of the font.
    pub fn placeholder_ng_stops() -> [f32; layout::STOPS as usize] {
        let slider = layout::device().slider;
        layout::slider_stops(layout::ng_tx(), layout::ng_tr(), slider.approx_easy_w, slider.approx_hard_w)
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
            if now.saturating_sub(shown_at) >= OVERLAY_MS {
                self.ui.question = None;
                self.redraw = true;
            }
        }
        if let Some((_, _, shown_at)) = self.ui.refused {
            if now.saturating_sub(shown_at) >= OVERLAY_MS {
                self.ui.refused = None;
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
        if self.ui.delete_guard_until.is_some_and(|t| now >= t) {
            self.ui.delete_guard_until = None;
        }
    }

    /// True while the presenter keeps a redraw: while the player writes (`player_writes`), and while
    /// the pen is on the board. A pen press that does not write (a tap, the eraser, a button) stops the
    /// writing. A finger press does not stop it (INP-21). A pen touch keeps the redraw until its lift,
    /// also on a filled cell, thus the result of a tap goes in one redraw (HLT-9).
    pub fn is_writing(&self) -> bool {
        self.live_stroke.is_some() || self.pen_on_filled_cell() || self.player_writes()
    }

    /// "The player writes" of INVARIANTS.md: ink waits, or the writing timer runs (INP-16).
    fn player_writes(&self) -> bool {
        self.ui.pending_ink.is_some() || self.writing_until.is_some()
    }

    /// True while a pen touch that started on a filled cell is on the board (INP-22).
    pub(crate) fn pen_on_filled_cell(&self) -> bool {
        matches!(self.pen, Some(Gesture::FilledTap { .. } | Gesture::FilledStroke { .. }))
    }

    /// The cell that the pen area must cover now (R§14.4, `present::Presenter`): the cell of the
    /// waiting ink, or the empty cell of the pen touch in progress, from the pen down (INP-24), also
    /// while the touch can still be a tap (INP-2). None when the pen is not on an empty cell and no
    /// ink waits: then the presenter decides what the area does. The area never covers a filled cell
    /// (INP-22), also when a finger Redo fills the cell of the touch. A touch on a micro-cell that
    /// shows a mark does not ask for the area while it can still be a tap: the tap removes the mark,
    /// and the pen waveform cannot remove a mark that the UI waveform drew (a ghost stays). A start
    /// or a stop of the area would refresh the full cell. The area comes to the cell when the pen
    /// leaves the micro-cell (the touch is writing).
    pub fn pen_cell(&self) -> Option<Cell> {
        match &self.pen {
            Some(Gesture::BoardTap { cell, digit, .. }) if !self.game.is_filled(*cell) => {
                let mark = self.game.visible_marks(*cell, self.settings.auto_hide_on) & crate::game::mark_bit(*digit) != 0;
                (!mark).then_some(*cell)
            }
            _ => self.ui.pending_ink.as_ref().map(|i| i.cell),
        }
    }

    /// The live stroke while it is in the cell of the pen area (`pen_cell`): the ink that gets the
    /// pen waveform. The ink of the live stroke shows also during a tap.
    pub fn writing_stroke(&self) -> Option<&Stroke> {
        self.live_stroke.as_ref().filter(|_| self.pen_cell().is_some())
    }

    /// The tablet turned. The positions of ink and of touches are not correct now: the app removes the
    /// ink that waits and stops the touches, thus a tap in progress makes no x. The game does not
    /// change. The "?" and the x stay on their cell for the rest of their time (MRK-16).
    /// `ng_stops` are the slider stops of the new layout (`render::ng_stops`).
    /// The games list keeps the first game of its page visible, because a page of the landscape layout
    /// has fewer games (`layout::list_rows`).
    pub fn rotate(&mut self, ng_stops: [f32; layout::STOPS as usize]) {
        self.ng_stops = ng_stops;
        let first = (self.list.page - 1) * self.per_page;
        self.per_page = crate::layout::list_rows();
        self.list.page = (first / self.per_page + 1).min(self.page_count());
        // The first crate and the first line of the page stay on the new page.
        let first = (self.licenses.page - 1) * self.licenses_per_page;
        self.licenses_per_page = layout::licenses_rows();
        self.licenses.page = first / self.licenses_per_page + 1;
        let first = (self.licenses.text_page - 1) * self.license_lines;
        self.license_lines = layout::license_lines();
        self.licenses.text_page = (first / self.license_lines + 1).min(self.license_text_pages());
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
        let q = self.ui.question.map(|(_, t)| t + OVERLAY_MS);
        // MRK-14: the main loop wakes at the end of the time of the x.
        let x = self.ui.refused.map(|(_, _, t)| t + OVERLAY_MS);
        let ink = self.ui.pending_ink.as_ref().and_then(|i| i.last_lift_at).map(|t| t + self.pause_ms);
        // UPD-6: the main loop wakes at the end of the 3 seconds of "Update failed".
        [q, x, ink, self.writing_until, self.update.deadline()].into_iter().flatten().min()
    }

    // ---- Input

    pub fn handle(&mut self, input: Input, now: u64) {
        self.tick(now);
        // A pen movement while the pen writes changes only the ink. Other input needs a full drawing
        // only if it changes something that the screen shows.
        let ink_only = matches!(input, Input::PenMove { .. })
            && matches!(
                self.pen,
                Some(Gesture::Writing | Gesture::DiscardInk { .. } | Gesture::BoardTap { .. } | Gesture::FilledTap { .. } | Gesture::FilledStroke { .. })
            );
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
                self.last_pen_lift = Some(now);
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
                //
                // INP-20: a hand touch has no effect at all. The app follows it as an ignored touch,
                // thus its move and its release also have no effect.
                let hand = self.last_pen_lift.is_some_and(|lift| now.saturating_sub(lift) < HAND_TOUCH_MS);
                if self.touch.as_ref().is_none_or(|t| t.is_replaced_by_press(id, now)) {
                    let gesture = if hand { Gesture::Ignored } else { self.press(x, y, None, now) };
                    self.touch = Some(FollowedTouch { id, gesture, last_event_ms: now });
                } else if !hand {
                    // A second finger operates no item, but it removes the "?" and the x (QST-4,
                    // MRK-15). It does not remove a highlight (HLT-7).
                    self.ui.question = None;
                    self.ui.refused = None;
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
        self.start_early_game();
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

    /// Starts to make the puzzle of the next game in the background (§11, PUZ-7), so that the new
    /// game does not wait for the generator. While the New game panel is open: the mode of the
    /// slider and the selected symmetry, when no pen or finger is down (not at each stop of a slider
    /// move). Else, on a complete game (the result card): the mode of Same and the symmetry of the
    /// game, immediately, also at a completion by the read of the ink at the end of the pause. The
    /// result is the same as at the time of need (PUZ-5).
    fn start_early_game(&mut self) {
        let (mode, symmetric) = if self.ui.menu.panel() == Some(Panel::NewGame) && self.screen == Screen::Game {
            if self.pen.is_some() || self.touch.is_some() {
                return;
            }
            (self.ui.slider, self.ui.symmetric_sel)
        } else if self.game.completed {
            (self.game.mode, self.game.symmetric)
        } else {
            return;
        };
        let seed = *self.next_seed.get_or_insert_with(|| self.env.new_seed());
        self.early_game = Some((seed, mode, symmetric));
        generate_early(seed, mode, symmetric);
    }

    /// A press of the pen (`tool` is Some) or of a finger (`tool` is None).
    fn press(&mut self, x: f32, y: f32, tool: Option<Tool>, now: u64) -> Gesture {
        // NAV-16: shortly after "< Back", a press on the toolbar or on the close button changes
        // nothing, also not the "?", the x or the waiting ink. It is often the second tap of a
        // fast double tap on "< Back", which is at the position of the close button (left-handed) or
        // of the menu button (right-handed).
        let toolbar_ignored = self.toolbar_ignored_until.is_some_and(|t| now < t);
        if toolbar_ignored && self.screen == Screen::Game && layout::game_button(x, y, self.ui.menu.is_open()).is_some() {
            return Gesture::Ignored;
        }
        // CKP-8: shortly after a delete, a press in the area of the Checkpoints panel at its maximum
        // size changes nothing. It is often the second tap of a fast double tap on a bin, and a
        // different row is at that position now. The dialog "Restart the interface?" covers the panel.
        let guarded = self.ui.delete_guard_until.is_some_and(|t| now < t);
        if guarded && !self.update.dialog_shown() && layout::ckp_guard_area().contains(x, y) {
            return Gesture::Ignored;
        }
        // Each press removes the "?" and the x before the app does the action (QST-4, MRK-15). A press
        // does not change the highlight, unless it reads waiting ink (HLT-9).
        self.ui.question = None;
        self.ui.refused = None;

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

        // This value is from before the press reads the ink: the toggle of a tap on a number
        // (`board_tap`).
        let highlight_at_press = self.ui.highlight;

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
        // A pen press reads the ink that waits in a different cell, also off the board (INP-12), and it
        // stops the writing: a redraw of its result must not wait. If the pen writes, the live stroke
        // continues the writing. A finger press does neither of these, also off the board (INP-21): a
        // control that a finger operates does them at its release.
        if tool.is_some() {
            self.read_ink(now);
            self.writing_until = None;
        }

        match self.screen {
            Screen::Game => self.press_game(x, y, tool, highlight_at_press),
            Screen::Manual | Screen::Statistics | Screen::GamesList | Screen::Licenses | Screen::LicenseText => {
                let target = self.page_target(x, y);
                if tool.is_none() && self.screen != Screen::Statistics {
                    Gesture::Swipe { x0: x, y0: y, target }
                } else {
                    target.map(Gesture::Control).unwrap_or(Gesture::Ignored)
                }
            }
        }
    }

    fn press_game(&mut self, x: f32, y: f32, tool: Option<Tool>, highlight_at_press: Option<u8>) -> Gesture {
        let button = layout::game_button(x, y, self.ui.menu.is_open());

        // A panel is open: a tap outside the panel only closes the panel (NAV-11). The panel buttons (the
        // paint bucket, the gear, the bookmark and the +) also change the panel (§9, NAV-12).
        if let Some(panel) = self.ui.menu.panel() {
            if let Some(b) = button.filter(|&b| Panel::of_button(b).is_some()) {
                return Gesture::Control(Target::Game(b));
            }
            match panel {
                Panel::Settings if layout::settings_panel(self.update.has_row()).contains(x, y) => {
                    if self.update.button().is_some() && layout::update_button_area().contains(x, y) {
                        return Gesture::Control(Target::Update);
                    }
                    if layout::settings_row(layout::LICENSES_ROW).contains(x, y) {
                        return Gesture::Control(Target::Licenses);
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
                    if layout::ng_symmetric_row().contains(x, y) {
                        return Gesture::Control(Target::Symmetric);
                    }
                    if layout::ng_start_row().contains(x, y) {
                        return Gesture::Control(Target::StartNewGame);
                    }
                    return Gesture::Ignored;
                }
                Panel::Checkpoints if layout::ckp_panel(self.game.checkpoints.len()).contains(x, y) => {
                    return layout::checkpoint_control_at(x, y, self.game.checkpoints.len())
                        .map(|control| Gesture::Control(Target::Checkpoint(control)))
                        .unwrap_or(Gesture::Ignored);
                }
                _ => {
                    self.close_panel();
                    return Gesture::Ignored;
                }
            }
        }

        // A finger does not operate the close button (NAV-6): a hand must not close the app.
        if button == Some(Button::Close) && tool.is_none() {
            return Gesture::Ignored;
        }
        if let Some(b) = button {
            return Gesture::Control(Target::Game(b));
        }

        // The level buttons of the result card (CMP-10): the pen tip, the eraser and a finger operate
        // them. A button that is not available changes nothing (CMP-9).
        if self.game.completed {
            if let Some(b) = layout::level_button_at(x, y) {
                return match b.target(self.game.mode) {
                    Some(_) => Gesture::Control(Target::Level(b)),
                    None => Gesture::Ignored,
                };
            }
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
            // INP-22: no ink and no pen area on a filled cell.
            Tool::Pen if self.game.is_filled(cell) => Gesture::FilledTap { cell, digit, stroke: vec![(x, y)], highlight_at_press },
            Tool::Pen => {
                self.start_live_stroke(x, y);
                Gesture::BoardTap { cell, digit, highlight_at_press }
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
                    Gesture::DiscardInk { cell }
                } else {
                    self.ui.pending_ink = Some(PendingInk { cell, strokes: Vec::new(), last_lift_at: None });
                    Gesture::Writing
                }
            }
            Gesture::Writing | Gesture::DiscardInk { .. } => {
                if let Some(s) = self.live_stroke.as_mut() {
                    s.push((x, y));
                }
                g
            }
            Gesture::FilledTap { cell, digit, mut stroke, highlight_at_press } => {
                stroke.push((x, y));
                // The pen left the micro-cell: this touch is a stroke (INP-2).
                if layout::board_micro(x, y) == Some((cell, digit)) {
                    Gesture::FilledTap { cell, digit, stroke, highlight_at_press }
                } else {
                    Gesture::FilledStroke { cell, stroke }
                }
            }
            Gesture::FilledStroke { cell, mut stroke } => {
                stroke.push((x, y));
                Gesture::FilledStroke { cell, stroke }
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
            Gesture::BoardTap { cell, digit, highlight_at_press, .. } => {
                let stroke = self.live_stroke.take().unwrap_or_default();
                self.log_stroke(cell, &stroke);
                self.board_tap(cell, digit, highlight_at_press, now);
            }
            Gesture::FilledTap { cell, digit, stroke, highlight_at_press } => {
                self.log_stroke(cell, &stroke);
                self.board_tap(cell, digit, highlight_at_press, now);
            }
            Gesture::Writing => {
                let stroke = self.live_stroke.take().unwrap_or_default();
                if let Some(ink) = self.ui.pending_ink.as_mut() {
                    if let Some(log) = self.stroke_log.as_mut() {
                        log(BoardInput::Stroke(ink.cell, &stroke));
                    }
                    ink.strokes.push(stroke);
                    ink.last_lift_at = Some(now);
                    // The pause gives the time to read the ink (INP-5): read_ink then gets the result.
                    recognizer::read_early(&ink.strokes);
                }
                self.start_writing_timer(now);
            }
            Gesture::DiscardInk { cell } => {
                let stroke = self.live_stroke.take().unwrap_or_default();
                self.log_stroke(cell, &stroke);
                self.start_writing_timer(now);
            }
            // INP-7: a stroke from a filled cell changes only the highlight (HLT-2). If no highlight
            // shows, nothing changes on the screen, thus no redraw occurs.
            Gesture::FilledStroke { cell, stroke } => {
                self.log_stroke(cell, &stroke);
                self.ui.highlight = None;
                self.start_writing_timer(now);
            }
            Gesture::Control(t) => self.release_control(t, x, y, now),
            // The erasure and the slider move occur during the motion. The release has no effect.
            Gesture::Ignored | Gesture::Eraser | Gesture::Slider | Gesture::Swipe { .. } => {}
        }
    }

    /// Gives a stroke to `StrokeLog`. A stroke is empty if a completed game removed the live stroke.
    fn log_stroke(&mut self, cell: Cell, stroke: &[recognizer::Point]) {
        if let Some(log) = self.stroke_log.as_mut().filter(|_| !stroke.is_empty()) {
            log(BoardInput::Stroke(cell, stroke));
        }
    }

    /// A stroke on the board ended (INP-16). A finger Redo can complete the game during the stroke:
    /// then the timer does not start.
    fn start_writing_timer(&mut self, now: u64) {
        if !self.game.completed {
            self.writing_until = Some(now + WRITING_IDLE_MS);
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
                if dx.abs() >= layout::device().page.swipe && dx.abs() > dy.abs() {
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
            | Gesture::DiscardInk { .. }
            | Gesture::FilledTap { .. }
            | Gesture::FilledStroke { .. }
            | Gesture::Eraser => {}
        }
    }

    /// A tap on the board at its pen lift (§5.2). The gesture of a tap exists only with no panel and
    /// no dialog: a change to these states stops the gesture (`cancel_gestures`). Thus a refused tap
    /// sets the x only in the states of MRK-10 and MRK-12.
    ///
    /// A tap on a number compares with the highlight at the press. The press can read waiting ink, and
    /// that read removes the highlight: with highlight v, a tap on v then gives no highlight at the
    /// lift, and a tap on a different digit gives that digit.
    fn board_tap(&mut self, cell: Cell, digit: u8, highlight_at_press: Option<u8>, now: u64) {
        if self.game.completed {
            return;
        }
        let v = self.game.value_at(cell);
        if v != 0 {
            // HLT-3: the same digit removes the highlight. A different digit moves it. HLT-6: no
            // highlight if the switch is off.
            self.ui.highlight = if !self.settings.highlight_on || highlight_at_press == Some(v) { None } else { Some(v) };
            return;
        }
        // HLT-2: a tap on an empty cell removes the highlight, in the same redraw as the mark or the x.
        self.ui.highlight = None;
        if self.game.refuses_mark(cell, digit, self.settings.auto_hide_on) {
            // MRK-7, MRK-11: the game, the redo history, the clock and the saved data do not change.
            self.ui.refused = Some((cell, digit, now));
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

    /// Reads the pending ink (INP-5, INP-6). A read removes the highlight, in the same redraw as the
    /// number or the "?" (HLT-2).
    fn read_ink(&mut self, now: u64) {
        let Some(ink) = self.ui.pending_ink.take() else { return };
        self.ui.highlight = None;
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
            self.ui.refused = None;
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
                symmetric: self.game.symmetric,
            };
            match self.db.insert(&r) {
                Ok(_) => self.refresh_results(),
                Err(e) => eprintln!("sudoku: cannot save the result: {e}"),
            }
            // PUZ-7: the puzzle of Same starts at the completion, not at the next input.
            self.start_early_game();
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
            Screen::Licenses | Screen::LicenseText => {
                if hit(layout::back_button()) {
                    Some(Target::Back)
                } else if hit(layout::prev_arrow()) {
                    Some(Target::Prev)
                } else if hit(layout::next_arrow()) {
                    Some(Target::Next)
                } else if self.screen == Screen::Licenses {
                    (0..self.licenses_page_rows().len()).find(|&i| hit(layout::licenses_row_rect(i))).map(Target::LicenseRow)
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
            Target::Checkpoint(control) => layout::checkpoint_control_at(x, y, self.game.checkpoints.len()) == Some(control),
            Target::StartNewGame => layout::ng_start_row().contains(x, y),
            Target::Symmetric => layout::ng_symmetric_row().contains(x, y),
            Target::Level(b) => layout::level_button_at(x, y) == Some(b),
            Target::Switch(s) => layout::settings_at(x, y) == Some(s),
            Target::Update => layout::update_button_area().contains(x, y),
            Target::Licenses => layout::settings_row(layout::LICENSES_ROW).contains(x, y),
            Target::RestartLater => layout::dialog_cancel().contains(x, y),
            Target::RestartNow => layout::dialog_delete().contains(x, y),
            // The controls of the manual, the statistics and the games list.
            Target::Back
            | Target::Prev
            | Target::Next
            | Target::AllGames
            | Target::Filter(_)
            | Target::Bin(_)
            | Target::LicenseRow(_)
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
        // The release does the action of the control. A finger press does not read waiting ink or stop
        // the writing (INP-21), and while a finger stays on the control, the pen can show an x or write
        // ink. Thus the release removes the x (MRK-15), reads the waiting ink (INP-12) and stops the
        // writing before the action. Then an undo or a redo cannot put a number under the ink (INP-5),
        // an undo cannot leave an x with no number in a peer, and a panel does not open over the ink or
        // the x (NAV-11, MRK-12). The highlight stays, unless the read removes it (HLT-7). The release
        // does not remove a "?" in a different cell.
        self.ui.refused = None;
        self.read_ink(now);
        self.writing_until = None;
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
            Target::Checkpoint(control) => self.checkpoint_control(control, now),
            Target::StartNewGame => self.start_new_game(),
            // DIF-8: only the selection changes. The background puzzle starts again for the new
            // selection at the end of the input (PUZ-7).
            Target::Symmetric => self.ui.symmetric_sel = !self.ui.symmetric_sel,
            Target::Level(b) => self.level_button(b),
            Target::Back => match self.screen {
                Screen::GamesList => self.screen = Screen::Statistics,
                Screen::LicenseText => self.screen = Screen::Licenses,
                _ => self.show_game(now),
            },
            Target::Prev => self.swipe(false),
            Target::Next => self.swipe(true),
            Target::Licenses => {
                self.leave_game();
                self.screen = Screen::Licenses;
                self.licenses = LicensesUi::start();
            }
            Target::LicenseRow(i) => {
                let index = (self.licenses.page - 1) * self.licenses_per_page + i;
                if index < crate::licenses::count() {
                    self.licenses.open = index;
                    self.licenses.text_page = 1;
                    self.screen = Screen::LicenseText;
                }
            }
            Target::AllGames => {
                self.screen = Screen::GamesList;
                self.list = ListUi::start();
            }
            Target::Switch(s) => {
                // NAV-14: only the switch changes; the game and the undo history do not. HLT-6: when
                // highlights go off, the highlight goes in this redraw.
                self.settings.toggle(s);
                self.settings_dirty = true;
                if !self.settings.highlight_on {
                    self.ui.highlight = None;
                }
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
            // CKP-9: on a complete puzzle, the tap has no effect. This includes a tap that read the ink
            // that completed the puzzle, and a different open panel stays open.
            Button::Checkpoints => {
                if !self.game.completed {
                    self.toggle_panel(Panel::Checkpoints);
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

    /// A release of a control of the Checkpoints panel (§9.3).
    fn checkpoint_control(&mut self, control: CheckpointControl, now: u64) {
        match control {
            CheckpointControl::Save => {
                if self.game.save_checkpoint() {
                    self.checkpoints_changed();
                }
            }
            CheckpointControl::Restore(i) => {
                let o = self.game.restore_checkpoint(i);
                self.after_game_change(o);
            }
            CheckpointControl::Delete(i) => {
                if self.game.delete_checkpoint(i) {
                    self.ui.delete_guard_until = Some(now + DELETE_GUARD_MS);
                    self.checkpoints_changed();
                }
            }
        }
    }

    /// The list of checkpoints changed: the app saves the game file (PER-7). The rows moved, thus the
    /// other pen or finger has no more effect: its release never operates a different checkpoint
    /// (CKP-6). `view_key` contains the list, thus the screen gets a redraw.
    fn checkpoints_changed(&mut self) {
        self.game_dirty = true;
        self.cancel_gestures();
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
        // DIF-4: the selections are not kept.
        self.ui.slider = self.settings.last_mode;
        self.ui.symmetric_sel = self.settings.last_symmetric;
        self.ui.delete_guard_until = None;
        self.cancel_gestures();
    }

    /// The panel, the screen or the game changed under the other pen or finger. Its touch has no more
    /// effect, thus its release does not operate the board of a different state (NAV-11).
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
        self.ui = GameUi::closed(&self.settings);
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
            Screen::Licenses => {
                let pages = self.licenses_page_count();
                if next && self.licenses.page < pages {
                    self.licenses.page += 1;
                } else if !next && self.licenses.page > 1 {
                    self.licenses.page -= 1;
                }
            }
            Screen::LicenseText => {
                if next && self.licenses.text_page < self.license_text_pages() {
                    self.licenses.text_page += 1;
                } else if !next && self.licenses.text_page > 1 {
                    self.licenses.text_page -= 1;
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

    /// Start new game (§11). The New game panel closes and the menu stays open (NAV-9).
    pub fn start_new_game(&mut self) {
        self.start_game_at(self.ui.slider, self.ui.symmetric_sel);
        self.ui = GameUi::shown(&self.settings);
    }

    /// A level button of the result card (§8.3, CMP-10): a new game at `target(b)`, with the menu
    /// closed and no panel (NAV-9), and the symmetry of the completed game. A button that is not
    /// available, or a game that is not complete, changes nothing.
    pub fn level_button(&mut self, b: LevelButton) {
        let Some(mode) = b.target(self.game.mode).filter(|_| self.game.completed && self.screen == Screen::Game) else {
            return;
        };
        self.start_game_at(mode, self.game.symmetric);
        self.ui = GameUi::closed(&self.settings);
        // NAV-16 guards only the second tap of a double tap on "< Back", with the menu open. A level
        // button is a different action, and it closes the menu: the guard stops.
        self.toolbar_ignored_until = None;
    }

    /// The steps of a new game at `mode` and `symmetric` (DIF-5), with the seed of the puzzle that the
    /// app made in the background, if there is one (PUZ-7). The caller sets the menu.
    fn start_game_at(&mut self, mode: u8, symmetric: bool) {
        eprintln!("sudoku: start new game (mode {mode}, symmetric {symmetric})");
        self.settings.last_mode = mode;
        self.settings.last_symmetric = symmetric;
        self.settings_dirty = true;
        let seed = self.next_seed.take().unwrap_or_else(|| self.env.new_seed());
        self.early_game = None;
        self.game = new_game_with_symmetry(seed, mode, symmetric);
        self.game_dirty = true;
        self.live_stroke = None;
        self.cancel_gestures();
    }

    /// The puzzle `(seed, mode, symmetric)` that the app makes in the background for the next game
    /// (PUZ-7).
    pub fn early_game(&self) -> Option<PuzzleKey> {
        self.early_game
    }

    /// The comparison of the result card (CMP-8).
    pub fn card_comparison(&self) -> Comparison {
        Comparison::of(&self.results, &self.game)
    }

    // ---- Analytics

    /// The results that agree with the filter, with the newest first (ANA-5).
    pub fn filtered(&self) -> Vec<&ResultRow> {
        self.results.iter().filter(|r| self.list.filter.map(|m| r.mode == m).unwrap_or(true)).collect()
    }

    pub fn page_count(&self) -> usize {
        self.filtered().len().div_ceil(self.per_page).max(1)
    }

    /// The indexes of the crates on the current page of the Licenses list.
    pub fn licenses_page_rows(&self) -> std::ops::Range<usize> {
        let first = (self.licenses.page - 1) * self.licenses_per_page;
        first.min(crate::licenses::count())..(first + self.licenses_per_page).min(crate::licenses::count())
    }

    /// The quantity of pages of the Licenses list (LIC-2).
    pub fn licenses_page_count(&self) -> usize {
        crate::licenses::pages(crate::licenses::count(), self.licenses_per_page)
    }

    /// The quantity of pages of the license text of the open crate.
    pub fn license_text_pages(&self) -> usize {
        crate::licenses::pages(crate::licenses::lines(self.licenses.open).len(), self.license_lines)
    }

    /// The lines of the current page of the license text, and the page count.
    pub fn license_text_page(&self) -> (Vec<crate::licenses::Line>, usize) {
        let all = crate::licenses::lines(self.licenses.open);
        let pages = crate::licenses::pages(all.len(), self.license_lines);
        let page = all.into_iter().skip((self.licenses.text_page - 1) * self.license_lines).take(self.license_lines).collect();
        (page, pages)
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
        (self.screen, g.value, g.marks, g.given, g.undo.len(), g.redo.len(), g.completed, &g.checkpoints).hash(&mut h);
        if g.completed {
            g.seconds().hash(&mut h);
        }
        let ui = &self.ui;
        (ui.menu, ui.highlight, ui.question.map(|q| q.0), ui.refused.map(|(cell, digit, _)| (cell, digit)), ui.slider, ui.symmetric_sel).hash(&mut h);
        let pending = ui.pending_ink.as_ref().map_or(0, |i| i.strokes.len());
        let live = usize::from(self.live_stroke.as_ref().is_some_and(|s| s.len() >= 2));
        (pending + live).hash(&mut h);
        (self.manual_page, self.list.filter, self.list.page, self.list.confirm.as_ref().map(|r| r.id)).hash(&mut h);
        self.licenses.hash(&mut h);
        (self.settings.color_starting, self.settings.color_clash, self.settings.color_highlight, self.settings.color_completed).hash(&mut h);
        (self.settings.highlight_on, self.settings.auto_hide_on, self.settings.completed_color_on, self.settings.micro_line, self.confirm_all).hash(&mut h);
        (&self.update.current, &self.update.state, self.update.launch, self.update.row_text()).hash(&mut h);
        self.results.len().hash(&mut h);
        self.results.iter().for_each(|r| r.id.hash(&mut h));
        h.finish()
    }

    /// The result of an update request from the platform (§9.2).
    pub fn update_checked(&mut self, result: Result<Manifest, String>) {
        self.update.checked(result, self.ui.menu.panel() == Some(Panel::Settings) && self.screen == Screen::Game);
        self.redraw = true;
    }

    /// The result of an installation from the platform (§9.2). An installation never shows the dialog
    /// "Restart the interface?": that dialog shows only at the start of the app (`with_launch`,
    /// UPD-10).
    pub fn install_finished(&mut self, result: Result<(), String>, now: u64) {
        self.update.install_finished(result, now);
        self.redraw = true;
    }

    /// "Restart" in the dialog or the button "Restart interface" (UPD-10): the platform saves the game
    /// and runs xovi/start. The two controls show only while the update is not complete, thus the app
    /// asks for xovi/start only in that state.
    fn request_restart(&mut self) {
        if !self.update.restart_needed() {
            return;
        }
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

/// A new game that is not symmetric: the puzzle of `(seed, mode)` with the current version of the
/// generator.
pub fn new_game(seed: u64, mode: u8) -> Game {
    new_game_with_symmetry(seed, mode, false)
}

/// A new game: the puzzle of `(seed, mode, symmetric)` with the current version of the generator.
pub fn new_game_with_symmetry(seed: u64, mode: u8, symmetric: bool) -> Game {
    let p = generate_with_symmetry(seed, mode, symmetric);
    Game { symmetric, ..Game::new(seed, mode, p.solution, p.givens) }
}

/// True if the starting numbers and the solution are the puzzle of `(seed, mode, symmetric)` with the
/// version of the generator of the game (PUZ-5). The app examines this only when it opens a saved
/// game, because the generator is slow for each input. `check_game` first makes sure that the
/// version, the mode and the symmetry are possible (PUZ-8).
fn is_generated(g: &Game) -> bool {
    let p = crate::generator::generate_version(g.generator, g.seed, g.mode, g.symmetric);
    let given = p.givens.map(|row| row.map(|v| v != 0));
    p.solution == g.solution && given == g.given
}

/// The comparison of the result card with the earlier results at the mode of the game (CMP-8).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Comparison {
    /// No earlier result at this mode: "First game at this level".
    First,
    /// The average (Statistics rounding, ANA-4) and the minimum of the earlier times, in seconds.
    Earlier { average: u64, best: u64 },
}

impl Comparison {
    /// The results at the mode of the game, without the result of the game itself (the same seed).
    pub fn of(results: &[ResultRow], game: &Game) -> Comparison {
        let earlier = || results.iter().filter(|r| r.mode == game.mode && r.seed != game.seed);
        match (average_seconds(earlier()), earlier().map(|r| r.seconds).min()) {
            (Some(average), Some(best)) => Comparison::Earlier { average, best },
            _ => Comparison::First,
        }
    }
}

/// The average of the times, rounded to whole seconds. A half rounds to the even value, as in the
/// reference images (scripts/analytics.py).
pub fn average_seconds<'a>(rows: impl Iterator<Item = &'a ResultRow>) -> Option<u64> {
    let (n, sum) = rows.fold((0u64, 0u64), |(n, s), r| (n + 1, s + r.seconds));
    (n > 0).then(|| (sum as f64 / n as f64).round_ties_even() as u64)
}

#[cfg(test)]
mod launch_tests {
    use super::*;
    use crate::launch::{Launch, Restart};

    struct TestEnv;

    impl Env for TestEnv {
        fn unix_time(&mut self) -> i64 {
            1_700_000_000
        }
        fn new_seed(&mut self) -> u64 {
            7
        }
    }

    fn app(launch: Launch, shown: bool) -> App {
        let settings = Settings { restart_dialog_shown: shown, ..Settings::default() };
        App::with_launch(launch, None, Some(settings), ResultsDb::in_memory(), Box::new(TestEnv), App::placeholder_ng_stops())
    }

    /// A finger tap at the center of `area`.
    fn tap(app: &mut App, area: Rect, now: u64) {
        let (x, y) = (((area.x0 + area.x1) / 2) as f32, ((area.y0 + area.y1) / 2) as f32);
        app.handle(Input::TouchDown { id: 1, x, y }, now);
        app.handle(Input::TouchUp { id: 1, x, y }, now + 50);
        crate::invariants::check_app(app).unwrap();
    }

    #[test]
    fn the_restart_dialog_shows_only_at_the_first_start_while_the_update_is_not_complete() {
        // UPD-10: the first start in this state shows the dialog and saves that it showed.
        let mut first = app(Launch::external(true), false);
        assert!(first.update.dialog_shown() && first.settings.restart_dialog_shown && first.settings_dirty);
        crate::invariants::check_app(&first).unwrap();
        // "Later" closes the dialog. The update is still not complete.
        tap(&mut first, layout::dialog_cancel(), 1000);
        assert!(!first.update.dialog_shown() && first.update.restart_needed() && !first.restart_ui);
        // A later start in this state: no dialog, and the saved value does not change.
        let later = app(Launch::external(true), true);
        assert!(!later.update.dialog_shown() && later.update.restart_needed() && !later.settings_dirty);
        crate::invariants::check_app(&later).unwrap();
        // "Restart" asks the platform for xovi/start after the game is saved. A failed start closes
        // the dialog.
        let mut restart = app(Launch::external(true), false);
        restart.game_dirty = false;
        tap(&mut restart, layout::dialog_delete(), 1000);
        assert!(restart.restart_ui && restart.game_dirty);
        restart.restart_failed();
        assert!(!restart.restart_ui && !restart.update.dialog_shown());
        // UPD-13: the frontend mode sets the saved value to false. The dialog can show again.
        let frontend = app(Launch::Frontend, true);
        assert!(!frontend.settings.restart_dialog_shown && frontend.settings_dirty && !frontend.update.dialog_shown());
        crate::invariants::check_app(&frontend).unwrap();
        // The external mode with no frontend files: a release before the frontend. Nothing changes.
        for shown in [false, true] {
            let old = app(Launch::external(false), shown);
            assert_eq!((old.update.launch, old.settings.restart_dialog_shown, old.settings_dirty), (Launch::External(Restart::NotNeeded), shown, false));
            assert!(!old.update.dialog_shown() && !old.update.restart_needed());
        }
        // `new` is the frontend mode.
        let usual = App::new(None, None, ResultsDb::in_memory(), Box::new(TestEnv), App::placeholder_ng_stops());
        assert_eq!(usual.update.launch, Launch::Frontend);
    }
}
