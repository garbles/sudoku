//! Property-based tests: random sequences of player input. After each input, the debug build checks all
//! state rules of INVARIANTS.md (`invariants::check_app`, in `App::handle`). This test also checks the
//! rules about two states one after the other (CLK-2, CLK-3, CMP-5, INP-1, INP-9, UND-2, UND-3, ANA-2,
//! NAV-3, NAV-9, NAV-16, MRK-10 to MRK-16, HLT-1 to HLT-9). The two-hand sequences examine the x of a
//! refused tap, the highlight, the ink and the hand touches at each event (INP-2, INP-5, INP-20, INP-21,
//! INP-22, INP-24). The group sequences at the end examine COL-8 to COL-12 against a separate model
//! (`inv_groups`).

mod common;
mod inv_groups;

use common::*;
use proptest::prelude::*;
use sudoku::app::{App, Input, Menu, Panel, Screen, Tool, HAND_TOUCH_MS, OVERLAY_MS};
use sudoku::game::{cells, mark_bit, Cell, Change, Game, Group};
use sudoku::invariants::check_app;
use sudoku::layout::{self, Button};
use sudoku::reference;
use sudoku::settings::{Color, Section, Settings, Switch};

#[derive(Clone, Debug)]
enum Act {
    TapMicro(u8, u8, u8),
    Write(u8, u8, u8),
    WriteSolution(u8, u8),
    Scribble(u8, u8),
    Erase(u8, u8),
    FingerBoard(u8, u8),
    Button(u8),
    Point(u16, u16, bool),
    Swipe(bool),
    Wait(u16),
    UndoRedo,
    Solve,
    Suspend,
    /// Opens the New game panel, selects a mode and maybe starts a new game.
    NewGame(u8, bool),
    /// Opens the Cell colors panel and taps a color.
    PickColor(Section, Color),
    /// A tap on a control of the analytics screens.
    ListControl(u8),
    /// Goes to the games list, taps a bin and maybe confirms the dialog.
    DeleteFlow(u8, Option<bool>),
    /// Opens the Settings panel and taps a switch.
    Switch(Switch),
    /// Goes to the statistics, taps "Delete all games" and maybe confirms.
    DeleteAllFlow(Option<bool>),
}

fn act() -> impl Strategy<Value = Act> {
    let cell = (0..9u8, 0..9u8);
    prop_oneof![
        4 => (cell.clone(), 1..=9u8).prop_map(|((r, c), d)| Act::TapMicro(r, c, d)),
        3 => (cell.clone(), 1..=9u8).prop_map(|((r, c), d)| Act::Write(r, c, d)),
        4 => cell.clone().prop_map(|(r, c)| Act::WriteSolution(r, c)),
        1 => cell.clone().prop_map(|(r, c)| Act::Scribble(r, c)),
        2 => cell.clone().prop_map(|(r, c)| Act::Erase(r, c)),
        1 => cell.prop_map(|(r, c)| Act::FingerBoard(r, c)),
        5 => (0..8u8).prop_map(Act::Button),
        4 => (0..1620u16, 0..2160u16, any::<bool>()).prop_map(|(x, y, pen)| Act::Point(x, y, pen)),
        1 => any::<bool>().prop_map(Act::Swipe),
        2 => (0..3000u16).prop_map(Act::Wait),
        1 => Just(Act::UndoRedo),
        1 => Just(Act::Solve),
        1 => Just(Act::Suspend),
        1 => (0..5u8, any::<bool>()).prop_map(|(m, s)| Act::NewGame(m, s)),
        // u8 ranges, so that a seed in properties.proptest-regressions gives the same acts.
        2 => (0..Section::ALL.len() as u8, 0..9u8).prop_map(|(s, i)| Act::PickColor(Section::ALL[s as usize], Color::ALL[i as usize])),
        4 => (0..22u8).prop_map(Act::ListControl),
        1 => (0..12u8, proptest::option::of(any::<bool>())).prop_map(|(i, c)| Act::DeleteFlow(i, c)),
        2 => (0..Switch::ALL.len() as u8).prop_map(|i| Act::Switch(Switch::ALL[i as usize])),
        1 => proptest::option::of(any::<bool>()).prop_map(Act::DeleteAllFlow),
    ]
}

const BUTTONS: [Button; 8] =
    [Button::Menu, Button::Colors, Button::Undo, Button::Redo, Button::Help, Button::Statistics, Button::NewGame, Button::Close];

/// The board state that only changes to the board can change: seed, values, marks, the lengths of the
/// two stacks and the completion.
type Board = (u64, [[u8; 9]; 9], [[u16; 9]; 9], usize, usize, bool);

fn board(g: &Game) -> Board {
    (g.seed, g.value, g.marks, g.undo.len(), g.redo.len(), g.completed)
}

/// The player waits until the toolbar operates again after "< Back" (NAV-16). The acts that operate
/// a panel with more than one tap call it first, because their taps must operate the toolbar.
fn wait_for_toolbar(d: &mut Driver) {
    if let Some(t) = d.app.toolbar_ignored_until {
        d.wait(t.saturating_sub(d.now));
    }
}

/// The acts that use the toolbar for more than one tap: they wait for the toolbar first (NAV-16).
fn uses_toolbar_flow(a: &Act) -> bool {
    matches!(a, Act::UndoRedo | Act::Solve | Act::NewGame(..) | Act::PickColor(..) | Act::DeleteFlow(..) | Act::Switch(_) | Act::DeleteAllFlow(_))
}

/// The items of the state before an act that the rules of the act need.
struct BeforeAct {
    toolbar_ignored_until: Option<u64>,
    screen: Screen,
    menu_open: bool,
    panel: Option<Panel>,
    highlight: Option<u8>,
    highlight_on: bool,
    /// The cell of the waiting ink.
    ink: Option<Cell>,
    seed: u64,
    value: [[u8; 9]; 9],
}

impl BeforeAct {
    fn of(d: &Driver) -> BeforeAct {
        let a = &d.app;
        BeforeAct {
            toolbar_ignored_until: a.toolbar_ignored_until,
            screen: a.screen,
            menu_open: a.ui.menu.is_open(),
            panel: a.ui.menu.panel(),
            highlight: a.ui.highlight,
            highlight_on: a.settings.highlight_on,
            ink: a.ui.pending_ink.as_ref().map(|i| i.cell),
            seed: a.game.seed,
            value: a.game.value,
        }
    }
}

/// True if the act was one tap on a toolbar button or on the close button of the game screen while
/// the toolbar ignored a press (NAV-16). Then the act must change nothing. The test calls this after
/// the act, because `Driver::send` can wait before a press (INP-20): the press of a tap occurred 10 ms
/// before its release.
fn was_ignored_toolbar_tap(a: &Act, b: &BeforeAct, d: &Driver) -> bool {
    let press_at = d.now - 10;
    let ignored = b.toolbar_ignored_until.is_some_and(|t| press_at < t);
    let point = match *a {
        Act::Button(i) => button_center(BUTTONS[i as usize]),
        Act::Point(x, y, _) => (x as f32, y as f32),
        _ => return false,
    };
    ignored && b.screen == Screen::Game && layout::game_button(point.0, point.1, b.menu_open).is_some()
}

fn run(d: &mut Driver, a: &Act) {
    if uses_toolbar_flow(a) {
        wait_for_toolbar(d);
    }
    match *a {
        Act::TapMicro(r, c, digit) => d.tap_micro((r, c), digit),
        Act::Write(r, c, digit) => d.write((r, c), digit),
        Act::WriteSolution(r, c) => {
            let v = d.app.game.solution[r as usize][c as usize];
            d.write((r, c), v)
        }
        Act::Scribble(r, c) => {
            d.write_strokes(&scribble_ink((r, c)));
            d.wait(d.app.pause_ms + 10);
        }
        Act::Erase(r, c) => d.erase((r, c)),
        Act::FingerBoard(r, c) => {
            let (x, y) = micro_center((r, c), 5);
            d.finger_tap(x, y);
        }
        Act::Button(i) => d.button(BUTTONS[i as usize]),
        Act::Point(x, y, pen) => {
            if pen {
                d.pen_tap(x as f32, y as f32)
            } else {
                d.finger_tap(x as f32, y as f32)
            }
        }
        Act::Swipe(left) => d.swipe(left),
        Act::Wait(ms) => d.wait(ms as u64),
        Act::UndoRedo => {
            if d.app.screen == Screen::Game && d.app.ui.menu == Menu::Open(None) && d.app.ui.pending_ink.is_none() && d.app.game.can_undo() {
                let before = board(&d.app.game);
                d.button(Button::Undo);
                d.button(Button::Redo);
                assert_eq!(board(&d.app.game).1, before.1, "UND-3");
                assert_eq!(board(&d.app.game).2, before.2, "UND-3");
            }
        }
        Act::Solve => {
            if d.app.screen == Screen::Game && d.app.ui.menu.panel().is_none() && !d.app.game.completed {
                // Close the menu, so that no cell is below a panel.
                if d.app.ui.menu.is_open() {
                    d.button(Button::Menu);
                }
                solve(d);
            }
        }
        Act::Suspend => {
            d.app.suspend();
            d.now += 600_000;
            d.app.tick(d.now);
        }
        Act::NewGame(mode, start) => {
            if d.app.screen != Screen::Game {
                return;
            }
            if !d.app.ui.menu.is_open() {
                d.button(Button::Menu);
            }
            if d.app.ui.menu.panel() != Some(Panel::NewGame) {
                d.button(Button::NewGame);
            }
            let x = d.app.ng_stops[mode as usize];
            d.finger_tap(x, layout::ng_slider_y() as f32);
            if start {
                d.finger_tap(layout::ng_panel().x0 as f32 + 100.0, (layout::tb_bottom_y() + 56) as f32);
            }
        }
        Act::PickColor(section, color) => {
            if d.app.screen != Screen::Game {
                return;
            }
            if !d.app.ui.menu.is_open() {
                d.button(Button::Menu);
            }
            if d.app.ui.menu.panel() != Some(Panel::Colors) {
                d.button(Button::Colors);
            }
            let (x, y) = color_point(section, color);
            d.pen_tap(x, y);
        }
        Act::DeleteFlow(i, confirm) => {
            if d.app.screen == Screen::Game {
                if d.app.ui.menu.panel().is_some() {
                    d.button(Button::Menu);
                }
                if !d.app.ui.menu.is_open() {
                    d.button(Button::Menu);
                }
                d.button(Button::Statistics);
            }
            if d.app.screen == Screen::Statistics {
                let (x, y) = pixel_center(layout::all_games_button());
                d.finger_tap(x, y);
            }
            if d.app.screen == Screen::GamesList && d.app.list.confirm.is_none() {
                let (x, y) = pixel_center(layout::bin_button(i as usize));
                d.finger_tap(x, y);
            }
            if let Some(c) = confirm {
                let (x, y) = pixel_center(if c { layout::dialog_delete() } else { layout::dialog_cancel() });
                d.finger_tap(x, y);
            }
        }
        Act::Switch(s) => {
            if d.app.screen != Screen::Game {
                return;
            }
            if !d.app.ui.menu.is_open() {
                d.button(Button::Menu);
            }
            if d.app.ui.menu.panel() != Some(Panel::Settings) {
                d.button(Button::Settings);
            }
            let (x, y) = switch_point(s);
            d.finger_tap(x, y);
        }
        Act::DeleteAllFlow(confirm) => {
            if d.app.screen == Screen::Game {
                if d.app.ui.menu.panel().is_some() {
                    d.button(Button::Menu);
                }
                if !d.app.ui.menu.is_open() {
                    d.button(Button::Menu);
                }
                d.button(Button::Statistics);
            }
            if d.app.screen == Screen::Statistics {
                let (x, y) = pixel_center(layout::delete_all_button());
                d.finger_tap(x, y);
                if let Some(c) = confirm {
                    let (x, y) = pixel_center(if c { layout::dialog_delete() } else { layout::dialog_cancel() });
                    d.finger_tap(x, y);
                }
            }
        }
        Act::ListControl(k) => {
            let r = match k {
                0 => layout::back_button(),
                1 => layout::all_games_button(),
                2 => layout::prev_arrow(),
                3 => layout::next_arrow(),
                4 => layout::dialog_cancel(),
                5 => layout::dialog_delete(),
                6..=11 => layout::filter_segment(layout::list_filters().nth(k as usize - 6).unwrap()),
                _ => layout::bin_button(k as usize - 12),
            };
            let (x, y) = pixel_center(r);
            d.finger_tap(x, y);
        }
    }
}

// ---- The x of a refused tap: the rules of the state and of the acts.

/// The numbers in the peers of a cell (the same row, column or box), from the board only (MRK-18).
fn peer_numbers(g: &Game, (r, c): Cell) -> u16 {
    cells()
        .filter(|&(pr, pc)| (pr, pc) != (r, c) && (pr == r || pc == c || (pr / 3 == r / 3 && pc / 3 == c / 3)))
        .map(|p| g.value_at(p))
        .filter(|&v| v != 0)
        .fold(0, |m, v| m | mark_bit(v))
}

/// MRK-12, MRK-14, CMP-3, written from INVARIANTS.md and separate from `invariants::check_app`.
fn x_state_rule(app: &App) -> Result<(), String> {
    let Some((cell, digit, shown_at)) = app.ui.refused else {
        return Ok(());
    };
    let g = &app.game;
    let fail = |why: &str| Err(format!("MRK-12: the x {:?} with {why}", (cell, digit, shown_at)));
    if app.screen != Screen::Game {
        return fail("a different screen");
    }
    if app.ui.menu.panel().is_some() {
        return fail("an open panel");
    }
    if g.completed {
        return fail("a complete game");
    }
    if app.update.dialog_shown() {
        return fail("the update dialog");
    }
    if g.value_at(cell) != 0 {
        return fail("a filled cell");
    }
    if !app.settings.auto_hide_on {
        return fail("the switch off");
    }
    if !(1..=9).contains(&digit) || peer_numbers(g, cell) & mark_bit(digit) == 0 {
        return fail("no peer that has the digit");
    }
    if app.ui.highlight.is_some() {
        return fail("a highlight");
    }
    if app.ui.pending_ink.is_some() {
        return fail("waiting ink");
    }
    if app.ui.question.is_some_and(|(q, _)| q == cell) {
        return fail("a \"?\" on its cell");
    }
    // MRK-14: the main loop wakes at the end of the time of the x.
    if app.next_deadline().is_none_or(|t| t > shown_at + OVERLAY_MS) {
        return Err(format!("MRK-14: the x shown at {shown_at} and the next timer at {:?}", app.next_deadline()));
    }
    Ok(())
}

/// The rules about an act of the sequences. MRK-10: only a pen tap on the board makes an x. MRK-15: a
/// finger or the eraser removes the x, except a press that the app ignores after "< Back" (NAV-16).
/// MRK-14: the time alone removes the x at the end of its time, and not before.
fn x_act_rule(a: &Act, before: Option<(Cell, u8, u64)>, ignored_toolbar_tap: bool, d: &Driver) -> Result<(), String> {
    let x = d.app.ui.refused;
    let new_x = x.is_some() && x != before;
    let no_pen_tap = matches!(a, Act::FingerBoard(..) | Act::Erase(..) | Act::Button(_) | Act::Swipe(_) | Act::Wait(_) | Act::Point(_, _, false));
    if no_pen_tap && new_x {
        return Err(format!("MRK-10: the new x {x:?}"));
    }
    match a {
        Act::Wait(_) => match before {
            Some((.., t)) if d.now < t + OVERLAY_MS && x != before => Err(format!("MRK-14: the x {before:?} went at {}", d.now)),
            Some((.., t)) if d.now >= t + OVERLAY_MS && x.is_some() => Err(format!("MRK-14: the x {x:?} stays at {}", d.now)),
            _ => Ok(()),
        },
        Act::FingerBoard(..) | Act::Erase(..) | Act::Button(_) | Act::Swipe(_) | Act::Point(_, _, false) => {
            // `Driver::send` can wait before the press (INP-20): the time of the x can end in that wait.
            let time_ended = before.is_some_and(|(.., t)| d.now >= t + OVERLAY_MS);
            if ignored_toolbar_tap && x != before && !(x.is_none() && time_ended) {
                Err(format!("NAV-16: an ignored press changed the x from {before:?} to {x:?}"))
            } else if !ignored_toolbar_tap && x.is_some() {
                Err(format!("MRK-15: the x {x:?} stays after a press"))
            } else {
                Ok(())
            }
        }
        _ => Ok(()),
    }
}

// ---- The highlight: the rules of the state and of the acts, written from INVARIANTS.md.

/// HLT-1, HLT-2, HLT-4, HLT-6, CMP-3 for one state.
fn highlight_state_rule(app: &App) -> Result<(), String> {
    let Some(h) = app.ui.highlight else {
        return Ok(());
    };
    if !(1..=9).contains(&h) {
        return Err(format!("HLT-2: the highlight {h}"));
    }
    if app.screen != Screen::Game {
        return Err(format!("HLT-4: the highlight {h} on the screen {:?}", app.screen));
    }
    if !app.settings.highlight_on {
        return Err(format!("HLT-6: the highlight {h} with the switch off"));
    }
    if app.game.completed {
        return Err(format!("HLT-1, CMP-3: the highlight {h} on a complete game"));
    }
    if app.update.dialog_shown() {
        return Err(format!("HLT-2: the highlight {h} with the update dialog"));
    }
    Ok(())
}

/// The value of `highlight` after a pen tap on the board with no panel (HLT-2, HLT-3, HLT-6).
fn highlight_after_tap(value: u8, at_press: Option<u8>, on: bool) -> Option<u8> {
    match value {
        0 => None,
        v if !on || at_press == Some(v) => None,
        v => Some(v),
    }
}

/// The items of HLT-2 that all input can cause: a new game or a completed puzzle (item 5), a change of
/// screen (item 6) and turning off the switch (item 8). A read of waiting ink is item 4.
fn may_remove_highlight(b: &BeforeAct, d: &Driver) -> bool {
    let a = &d.app;
    let read = b.ink.is_some() && a.ui.pending_ink.is_none();
    read || a.game.seed != b.seed || a.game.completed || a.screen != Screen::Game || (b.highlight_on && !a.settings.highlight_on)
}

/// HLT-2, HLT-3, HLT-7 for one act of the first sequences: only the listed items change `highlight`.
fn highlight_act_rule(act: &Act, b: &BeforeAct, d: &Driver) -> Result<(), String> {
    let h = d.app.ui.highlight;
    let keep = |why: &str| {
        if h == b.highlight {
            Ok(())
        } else {
            Err(format!("HLT-7: {why} changed the highlight from {:?} to {h:?}", b.highlight))
        }
    };
    let point = match *act {
        Act::TapMicro(r, c, digit) => Some(micro_center((r, c), digit)),
        Act::Point(x, y, true) => Some((x as f32, y as f32)),
        _ => None,
    };
    match *act {
        // A suspension removes the ink, but the app does not read it.
        Act::Suspend => return keep("a suspension"),
        // The eraser on waiting ink in its own cell removes the ink. The app does not read it.
        Act::Erase(r, c) if b.ink.is_none() || b.ink == Some((r, c)) => return keep("the eraser"),
        _ => {}
    }
    // A pen tap on the board, with no panel and no ink: the exact result.
    if let (Some((x, y)), None, None, Screen::Game) = (point, b.panel, b.ink, b.screen) {
        if let (Some(((r, c), _)), false) = (layout::board_micro(x, y), d.app.game.completed) {
            let want = highlight_after_tap(b.value[r as usize][c as usize], b.highlight, b.highlight_on);
            return if h == want { Ok(()) } else { Err(format!("HLT-2, HLT-3: a pen tap gave {h:?} and not {want:?}, from {:?}", b.highlight)) };
        }
    }
    if h == b.highlight {
        return Ok(());
    }
    // More than one stroke: the first can close a panel, and the others can operate the board.
    let writes = matches!(act, Act::Write(..) | Act::WriteSolution(..) | Act::Scribble(..) | Act::Solve);
    if h.is_some() {
        // HLT-2: a value comes only from a pen tap on a number. A stroke of a digit can be a tap.
        if writes || (point.is_some() && b.ink.is_some()) {
            return Ok(());
        }
        return Err(format!("HLT-2: the highlight {h:?} did not come from a pen tap"));
    }
    if writes || may_remove_highlight(b, d) {
        return Ok(());
    }
    Err(format!("HLT-2: no item of the list removed the highlight {:?}", b.highlight))
}

/// True if the action can not change the board when no ink waits.
fn is_not_a_board_action(a: &Act, d: &Driver) -> bool {
    match a {
        Act::FingerBoard(..) | Act::Swipe(_) | Act::Suspend => true,
        Act::Button(i) => matches!(BUTTONS[*i as usize], Button::Menu | Button::Colors | Button::Help | Button::Statistics | Button::Close)
            || (BUTTONS[*i as usize] == Button::NewGame && d.app.ui.menu.panel() != Some(Panel::NewGame)),
        Act::Wait(_) | Act::PickColor(..) | Act::Switch(_) => true,
        Act::DeleteAllFlow(_) => d.app.screen != Screen::Game || d.app.ui.pending_ink.is_none(),
        Act::ListControl(_) => d.app.screen != Screen::Game,
        _ => false,
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 96, max_shrink_iters: 2000, ..ProptestConfig::default() })]

    #[test]
    fn invariants_hold_for_all_input(seed in any::<u64>(), acts in proptest::collection::vec(act(), 1..90)) {
        // Half of the runs start from the real start state, with the menu open (PER-2, NAV-1). The
        // other half start with the menu closed (`Driver::new`).
        let mut d = if seed % 2 == 0 { Driver::opened(new_app(seed)) } else { Driver::new(seed) };
        if seed % 2 == 0 {
            prop_assert_eq!((d.app.screen, d.app.ui.menu), (Screen::Game, Menu::Open(None)), "PER-2, NAV-1");
        }
        check_app(&d.app).unwrap();
        for a in &acts {
            let before = d.app.game.clone();
            let results_before = d.app.results.clone();
            let screen_before = d.app.screen;
            let confirm_all_before = d.app.confirm_all;
            let ink_before = d.app.ui.pending_ink.is_some();
            let not_board = is_not_a_board_action(a, &d) && !ink_before;
            let before_act = BeforeAct::of(&d);
            let menu_before = d.app.ui.menu;
            let x_before = d.app.ui.refused;

            run(&mut d, a);
            let ignored_toolbar_tap = was_ignored_toolbar_tap(a, &before_act, &d);
            check_app(&d.app).map_err(|e| TestCaseError::fail(format!("{e} after {a:?}")))?;
            x_state_rule(&d.app).map_err(|e| TestCaseError::fail(format!("{e} after {a:?}")))?;
            highlight_state_rule(&d.app).map_err(|e| TestCaseError::fail(format!("{e} after {a:?}")))?;
            highlight_act_rule(a, &before_act, &d).map_err(|e| TestCaseError::fail(format!("{e} after {a:?}")))?;
            x_act_rule(a, x_before, ignored_toolbar_tap, &d).map_err(|e| TestCaseError::fail(format!("{e} after {a:?}")))?;
            group_state_rule(&d.app).map_err(|e| TestCaseError::fail(format!("{e} after {a:?}")))?;

            // NAV-16: shortly after "< Back", a tap on the toolbar or on the close button changes
            // nothing.
            if ignored_toolbar_tap {
                prop_assert!(!d.app.quit, "NAV-16: the app closed after {:?}", a);
                prop_assert_eq!((d.app.screen, d.app.ui.menu), (Screen::Game, menu_before), "NAV-16 after {:?}", a);
                prop_assert_eq!(board(&d.app.game), board(&before), "NAV-16 after {:?}", a);
            }
            // NAV-3: when Manual or Statistics shows the game again, the menu is open with no panel.
            if matches!(screen_before, Screen::Manual | Screen::Statistics) && d.app.screen == Screen::Game {
                prop_assert_eq!(d.app.ui.menu, Menu::Open(None), "NAV-3 after {:?}", a);
            }

            let g = &d.app.game;
            let same_game = g.seed == before.seed && (g.undo.len() + g.redo.len() > 0 || before.undo.len() + before.redo.len() == 0 || g.elapsed_ms >= before.elapsed_ms);
            let new_game = g.seed != before.seed;
            // CLK-2: the clock never goes back, except for a new game.
            if !new_game && same_game {
                prop_assert!(g.elapsed_ms >= before.elapsed_ms, "CLK-2 after {:?}", a);
            }
            // CLK-3, INP-9, CMP-3: a complete game does not change.
            if before.completed && !new_game {
                prop_assert_eq!(board(g), board(&before), "INP-9 after {:?}", a);
                prop_assert_eq!(g.elapsed_ms, before.elapsed_ms, "CLK-3 after {:?}", a);
                prop_assert_eq!(&g.checkpoints, &before.checkpoints, "CMP-3, CKP-9 after {:?}", a);
            }
            // UND-2, INP-1, HLT-5, COL-4: actions that are not changes to the board.
            if not_board && !new_game {
                prop_assert_eq!(board(g), board(&before), "UND-2 after {:?}", a);
            }
            // UND-6, DIF-5
            if new_game {
                prop_assert!(g.undo.is_empty() && g.redo.is_empty() && !g.completed, "UND-6");
                prop_assert!(g.checkpoints.is_empty(), "UND-6, DIF-5");
                prop_assert_eq!(g.mode, d.app.settings.last_mode, "DIF-5");
                // NAV-9: after Start new game, the menu is open with no panel.
                prop_assert_eq!((d.app.screen, d.app.ui.menu), (Screen::Game, Menu::Open(None)), "NAV-9 after {:?}", a);
            }
            // CMP-5, ANA-2: results change only by completion (one row) or by a delete on the list.
            if !before.completed && g.completed && !new_game {
                prop_assert_eq!(d.app.results.len(), results_before.len() + 1, "CMP-5");
                let r = d.app.results.iter().find(|r| !results_before.contains(r)).unwrap();
                prop_assert_eq!((r.mode, r.givens, r.seconds, r.seed), (g.mode, g.givens_count(), g.seconds(), g.seed));
            } else if (matches!(a, Act::DeleteAllFlow(Some(true))) || confirm_all_before) && d.app.results.len() < results_before.len() {
                prop_assert!(d.app.results.is_empty(), "ANA-12: Delete all removes all results");
            } else if (screen_before == Screen::GamesList || matches!(a, Act::DeleteFlow(..))) && d.app.results.len() < results_before.len() {
                prop_assert_eq!(d.app.results.len() + 1, results_before.len(), "one delete");
            } else {
                prop_assert_eq!(&d.app.results, &results_before, "ANA-2 after {:?}", a);
            }
            if d.app.quit {
                // The close button: the app saves and stops. The test continues with a new start.
                prop_assert!(d.app.game_dirty, "NAV-6: the game is saved before the app closes");
                d.app.quit = false;
            }
        }
    }
}

#[test]
fn panels_never_cover_the_toolbar_and_the_toolbar_never_covers_the_board() {
    // NAV-10
    for o in [layout::Orientation::Portrait, layout::Orientation::Landscape] {
        layout::set_orientation(o);
        // NAV-15: for a right-handed player, the mirror image.
        layout::set_right_handed(true);
        assert_eq!(layout::tb_x(), 0, "{o:?}");
        assert_eq!(layout::colors_panel().x0, layout::TB_SLOT, "{o:?}");
        assert_eq!(layout::ng_panel().x0, layout::TB_SLOT, "{o:?}");
        assert_eq!(layout::ckp_panel(0).x0, layout::TB_SLOT, "NAV-18: {o:?}");
        assert!(layout::TB_SLOT < layout::board_x(), "{o:?}");
        layout::set_right_handed(false);
        assert!(layout::colors_panel().x1 <= layout::tb_x(), "{o:?}");
        assert!(layout::ng_panel().x1 <= layout::tb_x(), "{o:?}");
        assert_eq!(layout::ckp_panel(5).x1, layout::tb_x(), "NAV-18: {o:?}");
        assert!(layout::ckp_panel(5).y1 <= layout::screen_h() as i32, "NAV-18: {o:?}");
        assert!(layout::board_x() + layout::BOARD_SIZE < layout::tb_x(), "{o:?}");
        assert!(layout::colors_panel().y1 <= layout::screen_h() as i32, "{o:?}");
        assert!(layout::meter_y() + 20 < layout::screen_h() as i32, "{o:?}");
    }
    layout::set_orientation(layout::Orientation::Portrait);
}

#[test]
fn touch_with_two_fingers_uses_only_the_first() {
    let mut d = Driver::new(3);
    d.send(Input::TouchDown { id: 1, x: 1563.0, y: 55.0 });
    d.send(Input::TouchDown { id: 2, x: 1563.0, y: 55.0 });
    d.send(Input::TouchUp { id: 2, x: 1563.0, y: 55.0 });
    assert!(!d.app.ui.menu.is_open());
    d.send(Input::TouchUp { id: 1, x: 1563.0, y: 55.0 });
    assert!(d.app.ui.menu.is_open());
}

/// The screen without the ink (the platform draws the ink in parts while the pen writes).
fn render_without_ink(app: &mut sudoku::app::App, fonts: &sudoku::font::Fonts) -> tiny_skia::Pixmap {
    let pending = app.ui.pending_ink.take();
    let live = app.live_stroke.take();
    let mut pm = tiny_skia::Pixmap::new(1620, 2160).unwrap();
    sudoku::render::draw(&mut sudoku::render::Canvas::new(&mut pm, fonts), app);
    app.ui.pending_ink = pending;
    app.live_stroke = live;
    pm
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 24, max_shrink_iters: 200, ..ProptestConfig::default() })]

    /// When `redraw` is false, the screen (without the ink) is the same as at the last full drawing:
    /// the platform can skip the full drawing.
    #[test]
    fn no_redraw_means_no_change_on_the_screen(seed in any::<u64>(), acts in proptest::collection::vec(act(), 1..30)) {
        let fonts = sudoku::font::Fonts::repo().unwrap();
        let mut d = Driver::new(seed);
        d.app.ng_stops = sudoku::render::ng_stops(&fonts);
        let mut last = render_without_ink(&mut d.app, &fonts);
        d.app.redraw = false;
        for a in &acts {
            run(&mut d, a);
            d.app.quit = false;
            let now = render_without_ink(&mut d.app, &fonts);
            // BRD-7, COL-10: the pixels of each cell. A panel can cover a part of the board.
            if d.app.screen == Screen::Game && d.app.ui.menu.panel().is_none() {
                let error = inv_groups::board_color_error(&now, &d.app);
                prop_assert!(error.is_none(), "{:?} after {:?}", error, a);
            }
            if !d.app.redraw {
                prop_assert!(now.data() == last.data(), "the screen changed without redraw after {:?}", a);
            }
            last = now;
            d.app.redraw = false;
        }
    }
}

// ---- MRK-10 to MRK-16, CMP-3, NAV-11: the two-hand sequences. A finger stays on a control while the
// pen does a refused tap. The checks run after each event, not only after each act.

/// The layout of a sequence. When the value goes out of scope, it sets the default layout again.
struct InLayout;

impl InLayout {
    fn set(landscape: bool, right_handed: bool) -> InLayout {
        layout::set_orientation(if landscape { layout::Orientation::Landscape } else { layout::Orientation::Portrait });
        layout::set_right_handed(right_handed);
        InLayout
    }
}

impl Drop for InLayout {
    fn drop(&mut self) {
        layout::set_orientation(layout::Orientation::Portrait);
        layout::set_right_handed(false);
    }
}

/// The items that the sequences count, to prove that they reach the state of each rule.
#[derive(Default)]
struct Reached {
    /// A refused tap showed the x.
    shown: usize,
    /// The x showed while a finger stayed on the screen.
    shown_with_finger: usize,
    /// A press removed the x.
    press: usize,
    /// The release of a control that did an action removed the x.
    release: usize,
    /// A press that the app ignored kept the x (NAV-16).
    guard: usize,
    /// A second finger on the toolbar in the guard time removed the x and operated nothing (NAV-16,
    /// INP-15, MRK-15).
    guard_second_finger: usize,
    /// The time removed the x.
    time: usize,
    /// A turn kept the x.
    turn: usize,
    /// A finger press or release kept a highlight (HLT-7).
    highlight_finger: usize,
    /// The release of a control that did an action kept a highlight (HLT-7).
    highlight_control: usize,
    /// A highlight stayed while a panel was open (HLT-4).
    highlight_panel: usize,
    /// The eraser kept a highlight (HLT-7).
    highlight_eraser: usize,
    /// A pen lift removed or moved a highlight (HLT-2, HLT-3).
    highlight_lift: usize,
    /// A read of waiting ink removed a highlight (HLT-2).
    highlight_read: usize,
    /// A pen touch started on a filled cell (INP-22).
    filled_press: usize,
    /// A pen press on an empty cell started the pen area, because the player writes (INP-24).
    area_at_press: usize,
    /// A pen press on an empty cell did not start the pen area (INP-2).
    no_area_at_press: usize,
    /// A hand touch changed nothing (INP-20).
    hand: usize,
    /// A hand touch on a button kept the x (INP-20, MRK-15).
    hand_x: usize,
    /// A hand touch while a finger stays on the screen kept the x (INP-20, INP-15).
    hand_second: usize,
    /// A finger press kept waiting ink (INP-21).
    finger_keeps_ink: usize,
}

/// A driver that examines the rules of the x at each event.
struct Hands {
    d: Driver,
    /// The tool of the pen touch that is on the screen.
    tool: Option<Tool>,
    /// A turn stopped the pen touch that is on the screen.
    pen_stopped: bool,
    /// The touch that the app follows (INP-15): its id and the time of its last event.
    followed: Option<(i32, u64)>,
    /// The pen press on the board that is still down, as a tap or as a stroke.
    press: Option<BoardPress>,
    reached: Reached,
}

/// A pen press on the board with no panel (HLT-2, HLT-3, INP-2, INP-22).
#[derive(Clone, Copy, Debug)]
struct BoardPress {
    cell: Cell,
    digit: u8,
    /// The press is on a filled cell (INP-22).
    filled: bool,
    /// The press continues the waiting ink in its own cell: it is writing from the start.
    writing: bool,
    /// The pen left the micro-cell: this touch is not a tap (INP-2).
    left: bool,
    /// The highlight at the press (HLT-3).
    highlight: Option<u8>,
}

/// The state before one event that the rules of the highlight and of the ink need.
struct BeforeEvent {
    act: BeforeAct,
    /// The time when the pen lifted from the waiting ink.
    ink_lift: Option<u64>,
    /// `App::is_writing`.
    writing: bool,
    /// A pen touch is on the screen. The sequences can send a press with no lift first.
    pen_down: bool,
}

/// All that a hand touch must not change (INP-20).
type Everything = (Items, Option<u8>, Option<(Cell, u8, u64)>, Option<(Cell, u64)>, Option<sudoku::app::PendingInk>, Option<Vec<(f32, f32)>>, u8, u32, usize, bool);

fn everything(d: &Driver) -> Everything {
    let a = &d.app;
    (items(d), a.ui.highlight, a.ui.refused, a.ui.question, a.ui.pending_ink.clone(), a.live_stroke.clone(), a.ui.slider, a.manual_page as u32, a.list.page, a.confirm_all)
}

impl Hands {
    fn new(d: Driver) -> Hands {
        Hands { d, tool: None, pen_stopped: false, followed: None, press: None, reached: Reached::default() }
    }

    /// A finger stays on the screen.
    fn finger(&self) -> bool {
        self.followed.is_some()
    }

    /// Sends one event and examines MRK-10, MRK-11, MRK-12, MRK-15, QST-4, INP-15, NAV-16, the rules of
    /// the highlight and the rules of the ink (`event_rule`). `Driver::send` makes each press a touch
    /// that does not come from the hand, thus it can wait first (INP-20). Thus the rules that use the
    /// time of the press are after the send.
    fn send(&mut self, input: Input) -> Result<(), String> {
        let followed = self.followed;
        let before_event = BeforeEvent {
            act: BeforeAct::of(&self.d),
            ink_lift: self.d.app.ui.pending_ink.as_ref().and_then(|i| i.last_lift_at),
            writing: self.d.app.is_writing(),
            pen_down: self.tool.is_some(),
        };
        let d = &mut self.d;
        let before = d.app.ui.refused;
        let question_before = d.app.ui.question;
        let board_before = board(&d.app.game);
        let press = match input {
            Input::PenDown { x, y, .. } | Input::TouchDown { x, y, .. } => Some((x, y)),
            _ => None,
        };
        let control_before = (d.app.screen, d.app.ui.menu, d.app.game.undo.len(), d.app.game.redo.len(), d.app.quit);

        d.send(input);

        let in_guard = before_event.act.toolbar_ignored_until.is_some_and(|t| d.now < t);
        let ignored = press.is_some_and(|(x, y)| in_guard && before_event.act.screen == Screen::Game && layout::game_button(x, y, before_event.act.menu_open).is_some());
        // INP-15: a press with a different id, while the followed touch had an event less than 0.15 s
        // before, is a second finger. Else the app forgets the followed touch: this is a first press.
        let second_finger = match (input, followed) {
            (Input::TouchDown { id, .. }, Some((first, last))) => id != first && d.now - last < 150,
            _ => false,
        };
        // The time of the x or of the "?" can end in the wait before a press.
        let x_ended = before.is_some_and(|(.., t)| d.now >= t + OVERLAY_MS);
        let question_ended = question_before.is_some_and(|(_, t)| d.now >= t + OVERLAY_MS);

        check_app(&d.app)?;
        x_state_rule(&d.app)?;
        let x = d.app.ui.refused;
        if press.is_some() {
            if second_finger {
                // INP-15, MRK-15, QST-4, and NAV-16 in the guard time (owner decision 2026-11-05): a
                // second finger operates no item, but it removes the x and the "?". It does not remove a
                // highlight: `event_rule` examines this (HLT-7).
                if x.is_some() || d.app.ui.question.is_some() {
                    return Err(format!("INP-15, MRK-15, QST-4: the x {x:?} or the \"?\" {:?} stays after the second finger {input:?}", d.app.ui.question));
                }
                let control = (d.app.screen, d.app.ui.menu, d.app.game.undo.len(), d.app.game.redo.len(), d.app.quit);
                if control != control_before {
                    return Err(format!("INP-15, NAV-16: the second finger {input:?} operated an item"));
                }
                self.reached.guard_second_finger += usize::from(ignored && before.is_some());
                self.reached.press += usize::from(before.is_some());
            } else if ignored {
                let x_same = x == before || (x.is_none() && x_ended);
                let question_same = d.app.ui.question == question_before || (d.app.ui.question.is_none() && question_ended);
                if !x_same || !question_same {
                    return Err(format!("NAV-16: an ignored press changed the x from {before:?} to {x:?}, or the \"?\" from {question_before:?} to {:?}", d.app.ui.question));
                }
                self.reached.guard += usize::from(x.is_some());
            } else {
                if x.is_some() {
                    return Err(format!("MRK-15: the x {x:?} stays after the press {input:?}"));
                }
                self.reached.press += usize::from(before.is_some());
            }
        }
        if x.is_some() && x != before {
            let tool = self.tool;
            if !matches!(input, Input::PenUp { .. }) || tool != Some(Tool::Pen) || self.pen_stopped {
                return Err(format!("MRK-10, MRK-16: the event {input:?} of the tool {tool:?} made the x {x:?}"));
            }
            if x.is_some_and(|(.., t)| t != d.now) {
                return Err(format!("MRK-10: the x {x:?} at the lift at {}", d.now));
            }
            if board(&d.app.game) != board_before {
                return Err(format!("MRK-11: the refused tap changed the board, x {x:?}"));
            }
            self.reached.shown += 1;
            self.reached.shown_with_finger += usize::from(followed.is_some());
        }
        if matches!(input, Input::TouchUp { .. }) {
            let control = (d.app.screen, d.app.ui.menu, d.app.game.undo.len(), d.app.game.redo.len(), d.app.quit);
            if control != control_before {
                if x.is_some() {
                    return Err(format!("MRK-15: the x {x:?} stays after the release of a control"));
                }
                self.reached.release += usize::from(before.is_some());
            }
        }
        match input {
            Input::PenDown { tool, .. } => {
                self.tool = Some(tool);
                self.pen_stopped = false;
            }
            Input::PenUp { .. } => {
                self.tool = None;
                self.pen_stopped = false;
            }
            Input::TouchDown { id, .. } if !second_finger => self.followed = Some((id, d.now)),
            Input::TouchMove { id, .. } if followed.is_some_and(|(first, _)| first == id) => self.followed = Some((id, d.now)),
            Input::TouchUp { id, .. } if followed.is_some_and(|(first, _)| first == id) => self.followed = None,
            _ => {}
        }
        if matches!(input, Input::TouchDown { .. } | Input::TouchUp { .. }) && before_event.act.highlight.is_some() && d.app.ui.highlight.is_some() {
            self.reached.highlight_finger += 1;
            let control = (d.app.screen, d.app.ui.menu, d.app.game.undo.len(), d.app.game.redo.len(), d.app.quit);
            self.reached.highlight_control += usize::from(control != control_before);
        }
        self.reached.highlight_panel += usize::from(d.app.ui.highlight.is_some() && d.app.ui.menu.panel().is_some());
        self.event_rule(input, &before_event)
    }

    /// The rules of the highlight and of the ink, for one event. `b` is the state before the event.
    /// HLT-2, HLT-3, HLT-7, HLT-9: only the listed items change `highlight`, and at the listed event.
    /// INP-2, INP-22, INP-24: where the ink shows and where the pen area starts. INP-21: a finger press
    /// does not read the ink.
    fn event_rule(&mut self, input: Input, b: &BeforeEvent) -> Result<(), String> {
        highlight_state_rule(&self.d.app)?;
        let (act, now) = (&b.act, self.d.now);
        let app = &self.d.app;
        let h = app.ui.highlight;
        let read = act.ink.is_some() && app.ui.pending_ink.is_none();
        // Only the items of HLT-2 that all input can cause.
        let by_list = may_remove_highlight(act, &self.d);
        // A change of the panel, the screen or the game stops a pen touch (the press is not a tap now).
        if act.panel != app.ui.menu.panel() || app.screen != Screen::Game || app.game.seed != act.seed || app.update.dialog_shown() {
            self.press = None;
        }
        let keep_or_list = || {
            if h == act.highlight || (h.is_none() && by_list) {
                Ok(())
            } else {
                Err(format!("HLT-7, HLT-9: {input:?} changed the highlight from {:?} to {h:?}", act.highlight))
            }
        };
        match input {
            Input::PenDown { x, y, tool } => {
                let on_board = act.screen == Screen::Game && act.panel.is_none() && !app.update.dialog_shown() && !app.game.completed;
                let micro = layout::board_micro(x, y).filter(|_| on_board);
                let own_ink = micro.is_some_and(|(cell, _)| act.ink == Some(cell));
                // HLT-9: a pen press changes the highlight only by a read of waiting ink. The pen and the
                // eraser in the cell of the ink do not read it (INP-5, HLT-7).
                let read_at_press = read && !own_ink;
                if h != act.highlight && !(h.is_none() && read_at_press) {
                    return Err(format!("HLT-9: the pen press {input:?} changed the highlight from {:?} to {h:?}", act.highlight));
                }
                self.reached.highlight_read += usize::from(act.highlight.is_some() && h.is_none());
                self.reached.highlight_eraser += usize::from(tool == Tool::Eraser && h.is_some());
                if act.panel.is_some() && layout::board_micro(x, y).is_some() && app.live_stroke.is_some() {
                    return Err(format!("NAV-11, INP-2: the press {input:?} with a panel shows ink"));
                }
                self.press = None;
                if let (Some((cell, digit)), Tool::Pen) = (micro, tool) {
                    let filled = app.game.is_filled(cell);
                    self.press = Some(BoardPress { cell, digit, filled, writing: own_ink, left: false, highlight: act.highlight });
                    if filled {
                        self.reached.filled_press += 1;
                    } else {
                        if app.live_stroke.is_none() {
                            return Err(format!("INP-2: no ink at the press {input:?} on the empty cell {cell:?}"));
                        }
                        // INP-2, INP-24: the area starts at the pen down only while the player writes.
                        if !b.pen_down && app.writing_stroke().is_some() != b.writing {
                            return Err(format!("INP-2, INP-24: the pen area at the press {input:?} is {:?}, and the player writes: {}", app.writing_stroke().is_some(), b.writing));
                        }
                        self.reached.area_at_press += usize::from(!b.pen_down && b.writing);
                        self.reached.no_area_at_press += usize::from(!b.pen_down && !b.writing);
                    }
                }
            }
            Input::PenMove { x, y } => {
                if h != act.highlight && !(h.is_none() && read) {
                    return Err(format!("HLT-9: the pen movement changed the highlight from {:?} to {h:?}", act.highlight));
                }
                if let Some(p) = &mut self.press {
                    p.left |= layout::board_micro(x, y) != Some((p.cell, p.digit));
                }
            }
            Input::PenUp { x, y } => match self.press.take() {
                Some(p) if !app.game.completed => {
                    // The value of the cell at the lift: a finger Redo can fill the cell of the touch.
                    let left = p.left || layout::board_micro(x, y) != Some((p.cell, p.digit));
                    let want = if p.writing || (left && !p.filled) {
                        act.highlight
                    } else if left {
                        None
                    } else {
                        highlight_after_tap(act.value[p.cell.0 as usize][p.cell.1 as usize], p.highlight, act.highlight_on)
                    };
                    if h != want {
                        return Err(format!("HLT-2, HLT-3: the lift of {p:?} gave the highlight {h:?}, and not {want:?}"));
                    }
                    self.reached.highlight_lift += usize::from(h != act.highlight);
                }
                _ => keep_or_list()?,
            },
            Input::TouchDown { .. } => {
                // HLT-7, INP-21: a finger press does not read the ink. Only the time can read it, in the
                // wait before the press.
                let pause_ended = b.ink_lift.is_some_and(|t| now >= t + app.pause_ms);
                if read && !pause_ended {
                    return Err(format!("INP-21: the finger press {input:?} read the waiting ink"));
                }
                if h != act.highlight && !(h.is_none() && read) {
                    return Err(format!("HLT-7: the finger press {input:?} changed the highlight from {:?} to {h:?}", act.highlight));
                }
                self.reached.finger_keeps_ink += usize::from(act.ink.is_some() && !read);
            }
            Input::TouchMove { .. } => {
                if h != act.highlight {
                    return Err(format!("HLT-7: the finger movement changed the highlight from {:?} to {h:?}", act.highlight));
                }
            }
            Input::TouchUp { .. } => keep_or_list()?,
        }
        // INP-22: a touch from a filled cell shows no ink and starts no pen area. The stroke that starts
        // the area is on an empty cell.
        if self.press.is_some_and(|p| p.filled) && (app.live_stroke.is_some() || app.writing_stroke().is_some()) {
            return Err(format!("INP-22: ink of {:?} after {input:?}", self.press));
        }
        if let Some(cell) = app.writing_stroke().and_then(|s| s.first()).and_then(|&(x, y)| layout::board_cell(x, y)) {
            if app.game.is_filled(cell) {
                return Err(format!("INP-22: the pen area on the filled cell {cell:?} after {input:?}"));
            }
        }
        Ok(())
    }

    /// A hand touch at a point, `delay` ms (less than `HAND_TOUCH_MS`) after the pen lift that this
    /// value sent last. The press, a movement and the release change nothing (INP-20). The events have
    /// the same time, thus no timer ends between them. `first_finger`: a finger stays on the screen.
    fn hand_touch(&mut self, delay: u64, (x, y): (f32, f32), first_finger: bool) -> Result<(), String> {
        assert!(delay < HAND_TOUCH_MS);
        self.d.wait(delay);
        let before = everything(&self.d);
        let on_button = before.2.is_some() && layout::game_button(x, y, self.d.app.ui.menu.is_open()).is_some();
        let id = if first_finger { 2 } else { 1 };
        if first_finger {
            self.d.app.handle(Input::TouchDown { id, x, y }, self.d.now);
        } else {
            self.d.touch_down_after_lift(delay, x, y);
        }
        let now = self.d.now;
        check_app(&self.d.app)?;
        let same = |d: &Driver, event: &str| {
            if everything(d) == before {
                Ok(())
            } else {
                Err(format!("INP-20: the {event} of a hand touch at ({x}, {y}), {delay} ms after the lift, changed the state"))
            }
        };
        same(&self.d, "press")?;
        self.d.app.handle(Input::TouchMove { id, x: x + 150.0, y }, now);
        same(&self.d, "movement")?;
        self.d.app.handle(Input::TouchUp { id, x: x + 150.0, y }, now);
        same(&self.d, "release")?;
        self.reached.hand += 1;
        self.reached.hand_x += usize::from(on_button);
        self.reached.hand_second += usize::from(first_finger && before.2.is_some());
        Ok(())
    }

    /// The time goes until a new touch does not come from the hand (INP-20). The first finger moves at
    /// that time: it stays an active touch, thus the next touch is a second finger (INP-15).
    fn first_finger_after_hand_time(&mut self, x: f32, y: f32) -> Result<(), String> {
        self.wait(HAND_TOUCH_MS)?;
        self.send(Input::TouchMove { id: 1, x, y })
    }

    /// The time goes with no input (MRK-14).
    fn wait(&mut self, ms: u64) -> Result<(), String> {
        let before = self.d.app.ui.refused;
        let highlight = self.d.app.ui.highlight;
        let ink = self.d.app.ui.pending_ink.is_some();
        self.d.wait(ms);
        check_app(&self.d.app)?;
        x_state_rule(&self.d.app)?;
        // HLT-2, HLT-7: the time changes the highlight only by a read of the waiting ink.
        if self.d.app.ui.highlight != highlight && !(self.d.app.ui.highlight.is_none() && ink && self.d.app.ui.pending_ink.is_none()) {
            return Err(format!("HLT-7: the time changed the highlight from {highlight:?} to {:?}", self.d.app.ui.highlight));
        }
        let x = self.d.app.ui.refused;
        if let Some((.., t)) = before {
            if self.d.now < t + OVERLAY_MS && x != before {
                return Err(format!("MRK-14: the x {before:?} went at {}", self.d.now));
            }
            if self.d.now >= t + OVERLAY_MS {
                if x.is_some() {
                    return Err(format!("MRK-14: the x {x:?} stays at {}", self.d.now));
                }
                self.reached.time += 1;
            }
        } else if x.is_some() {
            return Err(format!("MRK-10: the time made the x {x:?}"));
        }
        Ok(())
    }

    /// The tablet turns (MRK-16, NAV-13). The pen touch and the finger touch stop.
    fn turn(&mut self) -> Result<(), String> {
        let before = self.d.app.ui.refused;
        let landscape = layout::screen_w() < layout::screen_h();
        layout::set_orientation(if landscape { layout::Orientation::Landscape } else { layout::Orientation::Portrait });
        self.d.app.rotate(App::placeholder_ng_stops());
        self.pen_stopped = self.tool.is_some();
        self.followed = None;
        self.press = None;
        let highlight = self.d.app.ui.highlight;
        check_app(&self.d.app)?;
        x_state_rule(&self.d.app)?;
        if self.d.app.ui.refused != before {
            return Err(format!("MRK-16: the turn changed the x from {before:?} to {:?}", self.d.app.ui.refused));
        }
        if self.d.app.ui.highlight != highlight {
            return Err(format!("HLT-7: the turn changed the highlight from {highlight:?} to {:?}", self.d.app.ui.highlight));
        }
        self.reached.turn += usize::from(before.is_some());
        Ok(())
    }

    /// The point of a refused tap on the board (the k-th of all that the board has now), if one exists.
    fn refused_point(&self, k: u8) -> Option<(f32, f32)> {
        let g = &self.d.app.game;
        let spots: Vec<(Cell, u8)> =
            cells().filter(|&c| !g.is_filled(c)).flat_map(|c| (1..=9u8).map(move |v| (c, v))).filter(|&(c, v)| peer_numbers(g, c) & mark_bit(v) != 0).collect();
        (!spots.is_empty()).then(|| {
            let (c, v) = spots[k as usize % spots.len()];
            micro_center(c, v)
        })
    }

    /// A pen tap on the k-th filled cell.
    fn number_tap(&mut self, k: u8) -> Result<(), String> {
        let filled: Vec<Cell> = cells().filter(|&c| self.d.app.game.is_filled(c)).collect();
        let (x, y) = micro_center(filled[k as usize % filled.len()], 1 + k % 9);
        self.send(Input::PenDown { x, y, tool: Tool::Pen })?;
        self.send(Input::PenUp { x, y })
    }

    /// A pen tap on the k-th refused point.
    fn refused_tap(&mut self, k: u8) -> Result<(), String> {
        if let Some((x, y)) = self.refused_point(k) {
            self.send(Input::PenDown { x, y, tool: Tool::Pen })?;
            self.send(Input::PenUp { x, y })?;
        }
        Ok(())
    }
}

/// The control under the finger of a two-hand act.
#[derive(Clone, Copy, Debug)]
enum Control {
    /// A button of the toolbar or the close button.
    Button(u8),
    /// A switch of the Settings panel. The act opens the panel first.
    Switch(u8),
    /// The Start new game button. The act opens the panel first.
    StartNewGame,
    /// A point in the Cell colors panel. The act opens the panel first.
    Color(u8, u8),
    /// A point on the screen.
    Point(u16, u16),
}

#[derive(Clone, Debug)]
enum Hand {
    /// An act of the first sequences.
    Base(Act),
    /// One refused tap (MRK-10).
    Refused(u8),
    /// A finger on a control and a refused tap, in one of the six orders of the four events. With
    /// `again`, the pen does a second tap before the finger lifts (the first tap can close a panel).
    TwoHands { control: Control, order: u8, spot: u8, again: bool, turn: bool },
    /// The ink of a digit that stays waiting (MRK-12, INP-12).
    Ink(u8, u8, u8),
    /// A scribble that stays waiting, which gives a "?" at the next press.
    ScribbleWaits(u8, u8),
    /// A refused tap, then a second finger (INP-15, MRK-15), with the first finger on the board.
    SecondFinger(u8),
    /// A refused tap, then the eraser presses the board (MRK-15).
    EraserPress(u8),
    /// The manual and "< Back", a refused tap, then a press on the toolbar in the guard time (NAV-16).
    /// With the last value, this press is a second finger (INP-15).
    Guard(u8, u8, bool),
    /// The time goes.
    Wait(u16),
    /// The tablet turns (MRK-16).
    Turn,
    /// The update dialog shows (MRK-15).
    Dialog,
    /// The update dialog closes.
    Later,
    /// A pen tap on the k-th filled cell (HLT-3).
    NumberTap(u8),
    /// A pen tap on the k-th filled cell, then the eraser on the cell at (row, column) (HLT-7).
    NumberThenEraser(u8, u8, u8),
    /// A pen tap on the k-th filled cell, then ink that stays waiting in the cell at (row, column). The
    /// next acts read it or remove it (HLT-2, HLT-7, HLT-9).
    NumberThenInk(u8, u8, u8, u8),
    /// A pen stroke from the k-th filled cell to the cell at (row, column) (INP-7, INP-22, HLT-2).
    FilledStroke(u8, u8, u8),
    /// A pen tap and then a hand touch, this number of ms after the lift (INP-20). With the last value,
    /// a finger stays on the board during the pen tap, thus the hand touch is a second touch.
    HandTouch { control: Control, spot: u8, delay: u8, first_finger: bool },
}

fn control() -> impl Strategy<Value = Control> {
    prop_oneof![
        5 => (0..9u8).prop_map(Control::Button),
        2 => (0..Switch::ALL.len() as u8).prop_map(Control::Switch),
        1 => Just(Control::StartNewGame),
        1 => (0..Section::ALL.len() as u8, 0..9u8).prop_map(|(s, c)| Control::Color(s, c)),
        1 => (0..2160u16, 0..2160u16).prop_map(|(x, y)| Control::Point(x, y)),
    ]
}

fn hand() -> impl Strategy<Value = Hand> {
    let spot = any::<u8>();
    prop_oneof![
        5 => act().prop_filter("an act that is not slow", |a| !matches!(a, Act::Solve | Act::DeleteFlow(..) | Act::DeleteAllFlow(_))).prop_map(Hand::Base),
        5 => spot.clone().prop_map(Hand::Refused),
        9 => (control(), 0..6u8, spot.clone(), any::<bool>(), proptest::bool::weighted(0.1))
            .prop_map(|(control, order, spot, again, turn)| Hand::TwoHands { control, order, spot, again, turn }),
        1 => (0..9u8, 0..9u8, 1..=9u8).prop_map(|(r, c, v)| Hand::Ink(r, c, v)),
        1 => (0..9u8, 0..9u8).prop_map(|(r, c)| Hand::ScribbleWaits(r, c)),
        2 => spot.clone().prop_map(Hand::SecondFinger),
        1 => spot.clone().prop_map(Hand::EraserPress),
        3 => (spot, 0..9u8, any::<bool>()).prop_map(|(s, b, second)| Hand::Guard(s, b, second)),
        3 => (0..1500u16).prop_map(Hand::Wait),
        1 => Just(Hand::Turn),
        1 => Just(Hand::Dialog),
        1 => Just(Hand::Later),
        6 => any::<u8>().prop_map(Hand::NumberTap),
        2 => (any::<u8>(), 0..9u8, 0..9u8).prop_map(|(k, r, c)| Hand::NumberThenEraser(k, r, c)),
        3 => (any::<u8>(), 0..9u8, 0..9u8, 1..=9u8).prop_map(|(k, r, c, v)| Hand::NumberThenInk(k, r, c, v)),
        2 => (any::<u8>(), 0..9u8, 0..9u8).prop_map(|(k, r, c)| Hand::FilledStroke(k, r, c)),
        6 => (control(), any::<u8>(), 0..HAND_TOUCH_MS as u8, proptest::bool::weighted(0.25))
            .prop_map(|(control, spot, delay, first_finger)| Hand::HandTouch { control, spot, delay, first_finger }),
    ]
}

/// The buttons of the toolbar and the close button.
const CONTROLS: [Button; 9] =
    [Button::Menu, Button::Colors, Button::Settings, Button::Undo, Button::Redo, Button::Help, Button::Statistics, Button::NewGame, Button::Close];

/// Opens the menu and, if it is not open, a panel. The player waits for the toolbar (NAV-16).
fn open_panel(h: &mut Hands, button: Button, panel: Panel) {
    wait_for_toolbar(&mut h.d);
    if h.d.app.screen != Screen::Game {
        return;
    }
    if !h.d.app.ui.menu.is_open() {
        h.d.button(Button::Menu);
    }
    if h.d.app.ui.menu.panel() != Some(panel) {
        h.d.button(button);
    }
}

/// The point of the control. It opens the menu or the panel of the control, if necessary.
fn control_point(h: &mut Hands, c: Control) -> (f32, f32) {
    match c {
        Control::Button(i) => {
            let b = CONTROLS[i as usize];
            wait_for_toolbar(&mut h.d);
            if b != Button::Menu && !h.d.app.ui.menu.is_open() && h.d.app.screen == Screen::Game {
                h.d.button(Button::Menu);
            }
            button_center(b)
        }
        Control::Switch(i) => {
            open_panel(h, Button::Settings, Panel::Settings);
            switch_point(Switch::ALL[i as usize])
        }
        Control::StartNewGame => {
            open_panel(h, Button::NewGame, Panel::NewGame);
            (layout::ng_panel().x0 as f32 + 400.0, (layout::tb_bottom_y() + 56) as f32)
        }
        Control::Color(s, i) => {
            open_panel(h, Button::Colors, Panel::Colors);
            color_point(Section::ALL[s as usize], Color::ALL[i as usize])
        }
        Control::Point(x, y) => (x as f32, y as f32),
    }
}

fn run_hand(h: &mut Hands, a: &Hand) -> Result<(), String> {
    match *a {
        Hand::Base(ref act) => {
            if !h.d.app.update.dialog_shown() {
                let before = BeforeAct::of(&h.d);
                run(&mut h.d, act);
                check_app(&h.d.app)?;
                x_state_rule(&h.d.app)?;
                highlight_state_rule(&h.d.app)?;
                highlight_act_rule(act, &before, &h.d)?;
                // The next press does not come from the hand (INP-20).
                h.d.wait(HAND_TOUCH_MS);
            }
        }
        Hand::Refused(k) => h.refused_tap(k)?,
        Hand::TwoHands { control, order, spot, again, turn } => {
            let (x, y) = control_point(h, control);
            let finger_down = Input::TouchDown { id: 1, x, y };
            let finger_up = Input::TouchUp { id: 1, x, y };
            // The six orders. F: the finger, P: the pen. The first is the main case: F P P F.
            let steps: [u8; 4] = match order {
                0 => [0, 2, 3, 1],
                1 => [0, 2, 1, 3],
                2 => [0, 1, 2, 3],
                3 => [2, 0, 3, 1],
                4 => [2, 0, 1, 3],
                _ => [2, 3, 0, 1],
            };
            let mut pen = None;
            for (k, step) in steps.into_iter().enumerate() {
                match step {
                    0 => h.send(finger_down)?,
                    1 => {
                        if again && h.finger() {
                            h.refused_tap(spot)?;
                        }
                        h.send(finger_up)?
                    }
                    2 => {
                        pen = h.refused_point(spot);
                        if let Some((x, y)) = pen {
                            h.send(Input::PenDown { x, y, tool: Tool::Pen })?;
                        }
                    }
                    _ => {
                        if let Some((x, y)) = pen {
                            h.send(Input::PenUp { x, y })?;
                        }
                    }
                }
                if turn && k == 1 {
                    h.turn()?;
                }
            }
        }
        Hand::Ink(r, c, v) => {
            let ink = digit_ink((r, c), v);
            for s in &ink {
                h.send(Input::PenDown { x: s[0].0, y: s[0].1, tool: Tool::Pen })?;
                for p in &s[1..] {
                    h.send(Input::PenMove { x: p.0, y: p.1 })?;
                }
                let last = s[s.len() - 1];
                h.send(Input::PenUp { x: last.0, y: last.1 })?;
            }
        }
        Hand::ScribbleWaits(r, c) => {
            for s in &scribble_ink((r, c)) {
                h.send(Input::PenDown { x: s[0].0, y: s[0].1, tool: Tool::Pen })?;
                for p in &s[1..] {
                    h.send(Input::PenMove { x: p.0, y: p.1 })?;
                }
                let last = s[s.len() - 1];
                h.send(Input::PenUp { x: last.0, y: last.1 })?;
            }
        }
        Hand::SecondFinger(k) => {
            let (x, y) = micro_center((4, 4), 5);
            h.send(Input::TouchDown { id: 1, x, y })?;
            h.refused_tap(k)?;
            h.first_finger_after_hand_time(x, y)?;
            h.send(Input::TouchDown { id: 2, x, y })?;
            h.send(Input::TouchUp { id: 2, x, y })?;
            h.send(Input::TouchUp { id: 1, x, y })?;
        }
        Hand::EraserPress(k) => {
            h.refused_tap(k)?;
            let (x, y) = micro_center((4, 4), 5);
            h.send(Input::PenDown { x, y, tool: Tool::Eraser })?;
            h.send(Input::PenUp { x, y })?;
        }
        Hand::Guard(k, b, second) => {
            wait_for_toolbar(&mut h.d);
            if h.d.app.screen == Screen::Game && h.d.app.ui.menu.panel().is_none() && !h.d.app.update.dialog_shown() {
                if !h.d.app.ui.menu.is_open() {
                    h.d.button(Button::Menu);
                }
                h.d.button(Button::Help);
                if h.d.app.screen == Screen::Manual {
                    let (x, y) = pixel_center(layout::back_button());
                    h.d.finger_tap(x, y);
                    let (bx, by) = micro_center((4, 4), 5);
                    if second {
                        h.send(Input::TouchDown { id: 1, x: bx, y: by })?;
                    }
                    h.refused_tap(k)?;
                    if second {
                        h.first_finger_after_hand_time(bx, by)?;
                    }
                    let (x, y) = button_center(CONTROLS[b as usize]);
                    let id = if second { 2 } else { 1 };
                    h.send(Input::TouchDown { id, x, y })?;
                    h.send(Input::TouchUp { id, x, y })?;
                    if second {
                        h.send(Input::TouchUp { id: 1, x: bx, y: by })?;
                    }
                }
            }
        }
        Hand::Wait(ms) => h.wait(ms as u64)?,
        Hand::Turn => h.turn()?,
        Hand::NumberTap(k) => h.number_tap(k)?,
        Hand::NumberThenEraser(k, r, c) => {
            h.number_tap(k)?;
            let (x, y) = micro_center((r, c), 5);
            h.send(Input::PenDown { x, y, tool: Tool::Eraser })?;
            h.send(Input::PenMove { x: x + 20.0, y })?;
            h.send(Input::PenUp { x: x + 20.0, y })?;
        }
        Hand::NumberThenInk(k, r, c, v) => {
            h.number_tap(k)?;
            for s in &digit_ink((r, c), v) {
                h.send(Input::PenDown { x: s[0].0, y: s[0].1, tool: Tool::Pen })?;
                for p in &s[1..] {
                    h.send(Input::PenMove { x: p.0, y: p.1 })?;
                }
                let last = s[s.len() - 1];
                h.send(Input::PenUp { x: last.0, y: last.1 })?;
            }
        }
        Hand::FilledStroke(k, r, c) => {
            let filled: Vec<Cell> = cells().filter(|&c| h.d.app.game.is_filled(c)).collect();
            let (x0, y0) = micro_center(filled[k as usize % filled.len()], 5);
            let (x1, y1) = micro_center((r, c), 5);
            h.send(Input::PenDown { x: x0, y: y0, tool: Tool::Pen })?;
            h.send(Input::PenMove { x: (x0 + x1) / 2.0, y: (y0 + y1) / 2.0 })?;
            h.send(Input::PenMove { x: x0 + 45.0, y: y0 + 45.0 })?;
            h.send(Input::PenUp { x: x1, y: y1 })?;
        }
        Hand::HandTouch { control, spot, delay, first_finger } => {
            let dialog = h.d.app.update.dialog_shown();
            // With the dialog, the hand touch is on one of its two buttons (UPD-10).
            let point = if dialog { pixel_center(if spot % 2 == 0 { layout::dialog_cancel() } else { layout::dialog_delete() }) } else { control_point(h, control) };
            let on_board = micro_center((4, 4), 5);
            if first_finger {
                h.send(Input::TouchDown { id: 1, x: on_board.0, y: on_board.1 })?;
            }
            // A pen lift. A refused tap gives the x, if the board has one. A panel closes with it.
            match h.refused_point(spot).filter(|_| !dialog) {
                Some((x, y)) => {
                    h.send(Input::PenDown { x, y, tool: Tool::Pen })?;
                    h.send(Input::PenUp { x, y })?;
                }
                None => {
                    h.send(Input::PenDown { x: 5.0, y: 1000.0, tool: Tool::Pen })?;
                    h.send(Input::PenUp { x: 5.0, y: 1000.0 })?;
                }
            }
            h.hand_touch(delay as u64, point, first_finger)?;
            if first_finger {
                h.send(Input::TouchUp { id: 1, x: on_board.0, y: on_board.1 })?;
            }
        }
        Hand::Dialog => {
            if !h.d.app.update.dialog_shown() {
                let mut u = reference::update_state(true);
                u.tap();
                u.request = None;
                h.d.app.update = u;
                let now = h.d.now;
                h.d.app.update_installed(Ok(true), now);
                check_app(&h.d.app)?;
                x_state_rule(&h.d.app)?;
                if h.d.app.ui.refused.is_some() {
                    return Err("MRK-15: the x stays when the update dialog shows".to_string());
                }
                if h.d.app.ui.highlight.is_some() {
                    return Err("HLT-2: the highlight stays when the update dialog shows".to_string());
                }
            }
        }
        Hand::Later => h.d.app.update.close_dialog(),
    }
    h.d.app.quit = false;
    Ok(())
}

/// The driver of a two-hand sequence: half of the sequences start with the open menu. The two bits of
/// the seed above that select the layout.
fn hands_for(seed: u64) -> (Hands, InLayout) {
    let layout = InLayout::set(seed & 2 != 0, seed & 4 != 0);
    let d = if seed & 1 == 0 { Driver::opened(new_app(seed)) } else { Driver::new(seed) };
    (Hands::new(d), layout)
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 64, max_shrink_iters: 2000, ..ProptestConfig::default() })]

    /// MRK-10 to MRK-16 at each event of the two-hand sequences, in all layouts.
    #[test]
    fn the_rules_of_the_x_hold_for_two_hands(seed in any::<u64>(), acts in proptest::collection::vec(hand(), 1..40)) {
        let (mut h, _layout) = hands_for(seed);
        for a in &acts {
            run_hand(&mut h, a).map_err(|e| TestCaseError::fail(format!("{e} after {a:?}")))?;
        }
    }
}

#[test]
fn the_random_sequences_reach_the_x() {
    // MRK-10: the first random sequences show the x, thus their checks of MRK-12 examine it.
    use proptest::strategy::ValueTree;
    use proptest::test_runner::TestRunner;
    let mut runner = TestRunner::deterministic();
    // HLT-2, HLT-4, HLT-9, INP-5, QST-1, CMP-3: also the other states of the rules. The finger events of
    // these sequences come after the hand time (`Driver::send`), thus a hand touch does not stop them.
    let mut seen = 0;
    let mut other = [0usize; 6];
    for case in 0..24 {
        let acts = proptest::collection::vec(act(), 1..90).new_tree(&mut runner).unwrap().current();
        let mut d = Driver::new(case);
        for a in &acts {
            run(&mut d, a);
            d.app.quit = false;
            seen += usize::from(d.app.ui.refused.is_some());
            let ui = &d.app.ui;
            let states = [ui.highlight.is_some(), ui.highlight.is_some() && ui.menu.panel().is_some(), ui.question.is_some(), d.app.game.completed, d.app.screen == Screen::Manual, d.app.screen == Screen::GamesList];
            for (count, state) in other.iter_mut().zip(states) {
                *count += usize::from(state);
            }
        }
    }
    assert!(seen > 0, "no random sequence showed the x");
    let names = ["a highlight", "a highlight with a panel", "a \"?\"", "a complete game", "the manual", "the games list"];
    for (count, name) in other.iter().zip(names) {
        assert!(*count > 0, "no random sequence reached {name}");
    }
}

#[test]
fn the_two_hand_sequences_reach_each_rule_of_the_x() {
    // The two-hand sequences must reach each state that their checks examine: else they prove nothing.
    use proptest::strategy::ValueTree;
    use proptest::test_runner::TestRunner;
    let mut runner = TestRunner::deterministic();
    let mut all = Reached::default();
    for case in 0..64 {
        let acts = proptest::collection::vec(hand(), 1..40).new_tree(&mut runner).unwrap().current();
        let (mut h, _layout) = hands_for(case);
        for a in &acts {
            run_hand(&mut h, a).unwrap_or_else(|e| panic!("{e} after {a:?}"));
        }
        all.shown += h.reached.shown;
        all.shown_with_finger += h.reached.shown_with_finger;
        all.press += h.reached.press;
        all.release += h.reached.release;
        all.guard += h.reached.guard;
        all.guard_second_finger += h.reached.guard_second_finger;
        all.time += h.reached.time;
        all.turn += h.reached.turn;
        all.highlight_finger += h.reached.highlight_finger;
        all.highlight_control += h.reached.highlight_control;
        all.highlight_panel += h.reached.highlight_panel;
        all.highlight_eraser += h.reached.highlight_eraser;
        all.highlight_lift += h.reached.highlight_lift;
        all.highlight_read += h.reached.highlight_read;
        all.filled_press += h.reached.filled_press;
        all.area_at_press += h.reached.area_at_press;
        all.no_area_at_press += h.reached.no_area_at_press;
        all.hand += h.reached.hand;
        all.hand_x += h.reached.hand_x;
        all.hand_second += h.reached.hand_second;
        all.finger_keeps_ink += h.reached.finger_keeps_ink;
    }
    println!("shown {}, with a finger {}, press {}, release {}, guard {}, second finger in the guard time {}, time {}, turn {}", all.shown, all.shown_with_finger, all.press, all.release, all.guard, all.guard_second_finger, all.time, all.turn);
    println!(
        "highlight: finger {}, control {}, panel {}, eraser {}, lift {}, read {}; filled press {}, area at press {}, no area {}; hand {}, hand with x {}, hand second {}; finger keeps ink {}",
        all.highlight_finger, all.highlight_control, all.highlight_panel, all.highlight_eraser, all.highlight_lift, all.highlight_read, all.filled_press, all.area_at_press, all.no_area_at_press, all.hand, all.hand_x, all.hand_second, all.finger_keeps_ink
    );
    assert!(all.shown > 0, "no x");
    assert!(all.shown_with_finger > 0, "no x while a finger stays on the screen");
    assert!(all.press > 0, "no press removed the x");
    assert!(all.release > 0, "no release of a control removed the x");
    assert!(all.guard > 0, "no ignored press kept the x");
    assert!(all.guard_second_finger > 0, "no second finger in the guard time removed the x");
    assert!(all.time > 0, "the time did not remove the x");
    assert!(all.turn > 0, "no turn kept the x");
    assert!(all.highlight_finger > 0, "HLT-7: no finger press or release with a highlight");
    assert!(all.highlight_control > 0, "HLT-7: no control kept a highlight");
    assert!(all.highlight_panel > 0, "HLT-4: no highlight with an open panel");
    assert!(all.highlight_eraser > 0, "HLT-7: no eraser with a highlight");
    assert!(all.highlight_lift > 0, "HLT-2, HLT-3: no pen lift changed a highlight");
    assert!(all.highlight_read > 0, "HLT-2: no read removed a highlight");
    assert!(all.filled_press > 0, "INP-22: no pen press on a filled cell");
    assert!(all.area_at_press > 0, "INP-24: no pen press started the pen area");
    assert!(all.no_area_at_press > 0, "INP-2: all pen presses started the pen area");
    assert!(all.hand > 0, "INP-20: no hand touch");
    assert!(all.hand_x > 0, "INP-20: no hand touch on a button with the x");
    assert!(all.hand_second > 0, "INP-20: no hand touch with a finger on the screen");
    assert!(all.finger_keeps_ink > 0, "INP-21: no finger press with waiting ink");
}

/// A cell and a digit of a refused tap, and the cell of the only peer with the number. `d` has the menu
/// closed. The pen writes the number `v` in the cell `a`, with `a` a peer of the cell `b`. No other peer
/// of `b` has `v`. Thus the refused tap on `b` depends only on the number in `a`.
fn only_peer_with_number(d: &mut Driver) -> (Cell, Cell, u8) {
    let g = &d.app.game;
    let (a, b, v) = cells()
        .filter(|&b| !g.is_filled(b))
        .flat_map(|b| (1..=9u8).map(move |v| (b, v)))
        .filter(|&(b, v)| peer_numbers(g, b) & mark_bit(v) == 0)
        .find_map(|(b, v)| {
            cells()
                .find(|&a| a != b && !g.is_filled(a) && (a.0 == b.0 || a.1 == b.1 || (a.0 / 3 == b.0 / 3 && a.1 / 3 == b.1 / 3)))
                .map(|a| (a, b, v))
        })
        .expect("two empty peers and a free digit");
    d.write(a, v);
    assert_eq!(d.app.game.value_at(a), v, "the app did not read {v}");
    (a, b, v)
}

/// A finger on `button`, a refused tap on micro-cell `digit` of `cell`. The x shows, and the finger
/// stays on the button.
fn hold_and_refuse(h: &mut Hands, button: Button, cell: Cell, digit: u8) -> (f32, f32) {
    let (x, y) = button_center(button);
    h.send(Input::TouchDown { id: 1, x, y }).unwrap();
    let (px, py) = micro_center(cell, digit);
    h.send(Input::PenDown { x: px, y: py, tool: Tool::Pen }).unwrap();
    h.send(Input::PenUp { x: px, y: py }).unwrap();
    assert_eq!(h.d.app.ui.refused, Some((cell, digit, h.d.now)), "MRK-10: the x shows while the finger stays on {button:?}");
    (x, y)
}

#[test]
fn the_release_of_undo_that_removes_the_peer_number_leaves_no_x() {
    // MRK-15, MRK-12: an undo that removes the number of the only peer must not leave the x.
    for (landscape, right_handed) in [(false, false), (false, true), (true, false), (true, true)] {
        let _layout = InLayout::set(landscape, right_handed);
        let mut h = Hands::new(Driver::new(400));
        let (a, b, v) = only_peer_with_number(&mut h.d);
        h.d.button(Button::Menu);
        let (x, y) = hold_and_refuse(&mut h, Button::Undo, b, v);
        h.send(Input::TouchUp { id: 1, x, y }).unwrap();
        assert_eq!(h.d.app.game.value_at(a), 0, "the release does the undo");
        assert_eq!(h.d.app.ui.refused, None, "MRK-15: the x stays with no number in a peer");
    }
}

#[test]
fn the_release_of_redo_that_fills_the_cell_of_the_x_leaves_no_x() {
    // MRK-15, MRK-12: a redo that fills the cell of the x must not leave the x on a filled cell.
    let mut h = Hands::new(Driver::new(401));
    let (a, _, v) = only_peer_with_number(&mut h.d);
    h.d.button(Button::Menu);
    h.d.button(Button::Undo);
    assert_eq!(h.d.app.game.value_at(a), 0);
    let digit = (1..=9u8).find(|&x| peer_numbers(&h.d.app.game, a) & mark_bit(x) != 0).expect("a peer with a number");
    let (x, y) = hold_and_refuse(&mut h, Button::Redo, a, digit);
    assert_eq!(h.d.app.game.redo.len(), 1, "MRK-11: the refused tap keeps the redo history");
    h.send(Input::TouchUp { id: 1, x, y }).unwrap();
    assert_eq!(h.d.app.game.value_at(a), v, "the release does the redo");
    assert_eq!(h.d.app.ui.refused, None, "MRK-15: the x stays on a filled cell");
}

#[test]
fn the_release_of_redo_that_completes_the_game_leaves_no_x() {
    // MRK-15, MRK-12, CMP-3: the last number comes back with redo. The game is complete, thus no
    // overlay stays. A player cannot make the state with a redo of the last number, because a complete
    // game has no undo (UND-8), but the rule must hold.
    let mut h = Hands::new(Driver::new(402));
    let g = &mut h.d.app.game;
    let empty: Vec<Cell> = cells().filter(|&c| !g.is_filled(c)).collect();
    let (&last, rest) = empty.split_last().unwrap();
    for &(r, c) in rest {
        g.add_number((r, c), g.solution[r as usize][c as usize]);
    }
    g.add_number(last, g.solution[last.0 as usize][last.1 as usize]);
    assert!(g.completed);
    g.completed = false;
    g.undo();
    assert!(!g.completed && g.value_at(last) == 0 && g.can_redo());
    h.d.button(Button::Menu);
    let digit = (1..=9u8).find(|&x| peer_numbers(&h.d.app.game, last) & mark_bit(x) != 0).unwrap();
    let (x, y) = hold_and_refuse(&mut h, Button::Redo, last, digit);
    h.send(Input::TouchUp { id: 1, x, y }).unwrap();
    assert!(h.d.app.game.completed, "the release does the redo");
    assert_eq!(h.d.app.ui.refused, None, "CMP-3, MRK-15: the x stays on a complete game");
}

#[test]
fn the_release_of_a_button_that_opens_a_panel_or_a_screen_leaves_no_x() {
    // MRK-15, MRK-12, NAV-11: an open panel and a different screen never show an x. The Menu button
    // also does an action: it closes the menu.
    for (landscape, right_handed) in [(false, false), (false, true), (true, false), (true, true)] {
        for button in [Button::Menu, Button::Colors, Button::Settings, Button::NewGame, Button::Help, Button::Statistics] {
            let _layout = InLayout::set(landscape, right_handed);
            let mut h = Hands::new(Driver::new(403));
            h.d.button(Button::Menu);
            let g = &h.d.app.game;
            let (cell, digit) = cells()
                .filter(|&c| !g.is_filled(c))
                .flat_map(|c| (1..=9u8).map(move |v| (c, v)))
                .find(|&(c, v)| peer_numbers(g, c) & mark_bit(v) != 0)
                .unwrap();
            let (x, y) = hold_and_refuse(&mut h, button, cell, digit);
            h.send(Input::TouchUp { id: 1, x, y }).unwrap();
            let opened = match button {
                Button::Menu => !h.d.app.ui.menu.is_open(),
                Button::Colors | Button::Settings | Button::NewGame => h.d.app.ui.menu.panel().is_some(),
                Button::Help => h.d.app.screen == Screen::Manual,
                Button::Statistics => h.d.app.screen == Screen::Statistics,
                _ => unreachable!("this test does not press {button:?}"),
            };
            assert!(opened, "the release of {button:?} does its action");
            assert_eq!(h.d.app.ui.refused, None, "MRK-15, MRK-12: {button:?}");
        }
    }
}

#[test]
fn the_release_of_a_control_keeps_a_question_mark_in_a_different_cell() {
    // MRK-15, QST-4, §3.2, MRK-14: the "?" shows in A and the x in B, while the finger stays on a
    // control. The release removes only the x. The "?" stays until its own time.
    let mut h = Hands::new(Driver::new(405));
    h.d.button(Button::Menu);
    let g = &h.d.app.game;
    let (b, digit) = cells()
        .filter(|&c| !g.is_filled(c))
        .flat_map(|c| (1..=9u8).map(move |v| (c, v)))
        .find(|&(c, v)| peer_numbers(g, c) & mark_bit(v) != 0)
        .unwrap();
    let a = cells().find(|&c| !g.is_filled(c) && c != b).unwrap();
    let (x, y) = button_center(Button::Undo);
    h.send(Input::TouchDown { id: 1, x, y }).unwrap();
    h.d.write_strokes(&scribble_ink(a));
    let (px, py) = micro_center(b, digit);
    h.send(Input::PenDown { x: px, y: py, tool: Tool::Pen }).unwrap();
    let pressed = h.d.now;
    h.send(Input::PenUp { x: px, y: py }).unwrap();
    assert_eq!(h.d.app.ui.question, Some((a, pressed)), "§3.2: the pen press reads the ink: the \"?\" in A");
    assert_eq!(h.d.app.ui.refused, Some((b, digit, h.d.now)), "§3.2: the x in B");
    h.send(Input::TouchUp { id: 1, x, y }).unwrap();
    assert_eq!(h.d.app.ui.refused, None, "MRK-15: the release removes the x");
    assert_eq!(h.d.app.ui.question, Some((a, pressed)), "§3.2: the release does not remove the \"?\" in a different cell");
    h.d.now = pressed + OVERLAY_MS;
    h.d.app.tick(h.d.now);
    assert_eq!(h.d.app.ui.question, None, "QST-1");
}

#[test]
fn a_finger_that_stays_does_not_change_the_time_of_the_x() {
    // MRK-14, MRK-15: the x goes at the end of its time while the finger stays. A new refused tap
    // removes the x at its press and starts the time again. A release after the x has no effect on it.
    let mut h = Hands::new(Driver::new(406));
    h.d.button(Button::Menu);
    let g = &h.d.app.game;
    let (cell, digit) = cells()
        .filter(|&c| !g.is_filled(c))
        .flat_map(|c| (1..=9u8).map(move |v| (c, v)))
        .find(|&(c, v)| peer_numbers(g, c) & mark_bit(v) != 0)
        .unwrap();
    let (x, y) = hold_and_refuse(&mut h, Button::Colors, cell, digit);
    let first = h.d.now;
    h.wait(OVERLAY_MS - 20).unwrap();
    assert_eq!(h.d.app.ui.refused, Some((cell, digit, first)), "MRK-14: the x went too early");
    let (px, py) = micro_center(cell, digit);
    h.send(Input::PenDown { x: px, y: py, tool: Tool::Pen }).unwrap();
    assert_eq!(h.d.app.ui.refused, None, "MRK-15: a new press removes the x");
    h.send(Input::PenUp { x: px, y: py }).unwrap();
    let second = h.d.now;
    assert_eq!(h.d.app.ui.refused, Some((cell, digit, second)), "MRK-15: the lift shows the x again, with a new time");
    h.d.now = second + OVERLAY_MS;
    h.d.app.tick(h.d.now);
    assert_eq!(h.d.app.ui.refused, None, "MRK-14");
    h.send(Input::TouchUp { id: 1, x, y }).unwrap();
    assert_eq!(h.d.app.ui.refused, None);
}

#[test]
fn a_switch_under_a_finger_does_not_turn_off_under_the_x() {
    // MRK-12, MRK-7, NAV-11: the finger stays on "Hide pencil marks automatically" in the Settings panel.
    // The first pen tap only closes the panel, and the second is a refused tap. The finger lifts: the
    // x never shows with the switch off.
    let mut h = Hands::new(Driver::new(407));
    h.d.button(Button::Menu);
    h.d.button(Button::Settings);
    let g = &h.d.app.game;
    let (cell, digit) = cells()
        .filter(|&c| !g.is_filled(c))
        .flat_map(|c| (1..=9u8).map(move |v| (c, v)))
        .find(|&(c, v)| peer_numbers(g, c) & mark_bit(v) != 0)
        .unwrap();
    let (x, y) = switch_point(Switch::HideMarksAutomatically);
    h.send(Input::TouchDown { id: 1, x, y }).unwrap();
    let (px, py) = micro_center(cell, digit);
    h.send(Input::PenDown { x: px, y: py, tool: Tool::Pen }).unwrap();
    h.send(Input::PenUp { x: px, y: py }).unwrap();
    assert_eq!(h.d.app.ui.menu.panel(), None, "NAV-11: the pen tap closes the panel");
    assert_eq!(h.d.app.ui.refused, None, "MRK-10: the tap only closes the panel");
    h.send(Input::PenDown { x: px, y: py, tool: Tool::Pen }).unwrap();
    h.send(Input::PenUp { x: px, y: py }).unwrap();
    assert_eq!(h.d.app.ui.refused, Some((cell, digit, h.d.now)), "MRK-10");
    h.send(Input::TouchUp { id: 1, x, y }).unwrap();
    x_state_rule(&h.d.app).unwrap();
    assert!(h.d.app.settings.auto_hide_on || h.d.app.ui.refused.is_none());
}

#[test]
fn the_update_dialog_removes_the_x_and_no_tap_makes_an_x_under_it() {
    // MRK-15, MRK-12, UPD-10: the dialog removes the x. A refused tap under the dialog makes no x.
    let mut h = Hands::new(Driver::new(408));
    let g = &h.d.app.game;
    let (cell, digit) = cells()
        .filter(|&c| !g.is_filled(c))
        .flat_map(|c| (1..=9u8).map(move |v| (c, v)))
        .find(|&(c, v)| peer_numbers(g, c) & mark_bit(v) != 0)
        .unwrap();
    h.d.tap_micro(cell, digit);
    assert!(h.d.app.ui.refused.is_some());
    run_hand(&mut h, &Hand::Dialog).unwrap();
    assert!(h.d.app.update.dialog_shown());
    assert_eq!(h.d.app.ui.refused, None, "MRK-15");
    let (px, py) = micro_center(cell, digit);
    h.send(Input::PenDown { x: px, y: py, tool: Tool::Pen }).unwrap();
    h.send(Input::PenUp { x: px, y: py }).unwrap();
    assert_eq!(h.d.app.ui.refused, None, "MRK-12: an x under the dialog");
}

/// The first refused tap of the board, as a cell and a digit.
fn first_refused_spot(g: &Game) -> (Cell, u8) {
    cells()
        .filter(|&c| !g.is_filled(c))
        .flat_map(|c| (1..=9u8).map(move |v| (c, v)))
        .find(|&(c, v)| peer_numbers(g, c) & mark_bit(v) != 0)
        .expect("a refused tap")
}

/// "< Back" from the manual. The guard time of NAV-16 starts, and the menu is open (NAV-3).
fn back_from_manual(h: &mut Hands) {
    wait_for_toolbar(&mut h.d);
    h.d.button(Button::Help);
    assert_eq!(h.d.app.screen, Screen::Manual);
    let (x, y) = pixel_center(layout::back_button());
    h.d.finger_tap(x, y);
    assert_eq!((h.d.app.screen, h.d.app.ui.menu), (Screen::Game, Menu::Open(None)), "NAV-3");
    assert!(h.d.app.toolbar_ignored_until.is_some(), "NAV-16: the guard time starts");
}

/// The items that a press on a toolbar button can change.
type Items = (Screen, Menu, Board, Settings, bool);

fn items(d: &Driver) -> Items {
    (d.app.screen, d.app.ui.menu, board(&d.app.game), d.app.settings.clone(), d.app.quit)
}

/// A driver with a board where Undo and Redo can operate, thus each toolbar button can do an action.
fn hands_with_history(seed: u64) -> Hands {
    let mut d = Driver::new(seed);
    let cell = free_cell(&d.app, &[]);
    let free: Vec<u8> = (1..=9u8).filter(|&v| peer_numbers(&d.app.game, cell) & mark_bit(v) == 0).collect();
    d.tap_micro(cell, free[0]);
    d.button(Button::Menu);
    d.write(cells().find(|&c| c != cell && !d.app.game.is_filled(c)).unwrap(), 5);
    d.button(Button::Undo);
    assert!(d.app.game.can_undo() && d.app.game.can_redo(), "the fixture has the two histories");
    Hands::new(d)
}

#[test]
fn a_second_finger_on_the_toolbar_in_the_guard_time_removes_the_overlays_and_operates_nothing() {
    // NAV-16, INP-15, MRK-15, QST-4 (owner decision 2026-11-05): in the guard time after "< Back", a
    // second finger on a toolbar button or on the close button operates nothing, but it removes the x
    // and the "?". All buttons, in all layouts.
    for (landscape, right_handed) in [(false, false), (false, true), (true, false), (true, true)] {
        let _layout = InLayout::set(landscape, right_handed);
        let what = format!("landscape {landscape}, right-handed {right_handed}");
        let mut h = hands_with_history(409);
        let on_board = micro_center((4, 4), 5);
        for button in CONTROLS {
            let (x, y) = button_center(button);

            // The x.
            back_from_manual(&mut h);
            let (cell, digit) = first_refused_spot(&h.d.app.game);
            let before = items(&h.d);
            h.send(Input::TouchDown { id: 1, x: on_board.0, y: on_board.1 }).unwrap();
            let (px, py) = micro_center(cell, digit);
            h.send(Input::PenDown { x: px, y: py, tool: Tool::Pen }).unwrap();
            h.send(Input::PenUp { x: px, y: py }).unwrap();
            assert_eq!(h.d.app.ui.refused, Some((cell, digit, h.d.now)), "MRK-10: {what}, {button:?}");
            h.first_finger_after_hand_time(on_board.0, on_board.1).unwrap();
            h.send(Input::TouchDown { id: 2, x, y }).unwrap();
            assert!(h.d.app.toolbar_ignored_until.is_some_and(|t| h.d.now < t), "{what}: the press is in the guard time");
            assert_eq!(h.d.app.ui.refused, None, "MRK-15, INP-15: {what}: the second finger on {button:?}");
            assert_eq!(items(&h.d), before, "NAV-16, INP-15: {what}: the second finger on {button:?} operated it");
            h.send(Input::TouchUp { id: 2, x, y }).unwrap();
            assert_eq!(items(&h.d), before, "NAV-16, INP-15: {what}: the lift of the second finger on {button:?}");
            h.send(Input::TouchUp { id: 1, x: on_board.0, y: on_board.1 }).unwrap();
            assert_eq!(items(&h.d), before, "NAV-16, INP-15: {what}: the lift of the first finger, {button:?}");

            // The "?". A finger on the board does not read the ink (INP-1), thus a short pause reads it
            // in the guard time. Then the first finger moves: it stays an active touch (INP-15).
            back_from_manual(&mut h);
            let empty = cells().find(|&c| !h.d.app.game.is_filled(c)).unwrap();
            let before = items(&h.d);
            let pause = std::mem::replace(&mut h.d.app.pause_ms, 10);
            h.send(Input::TouchDown { id: 1, x: on_board.0, y: on_board.1 }).unwrap();
            h.d.write_strokes(&scribble_ink(empty));
            h.d.wait(20);
            h.d.app.pause_ms = pause;
            assert_eq!(h.d.app.ui.question.map(|q| q.0), Some(empty), "QST-1: {what}, {button:?}");
            h.send(Input::TouchMove { id: 1, x: on_board.0, y: on_board.1 }).unwrap();
            h.send(Input::TouchDown { id: 2, x, y }).unwrap();
            assert!(h.d.app.toolbar_ignored_until.is_some_and(|t| h.d.now < t), "{what}: the press is in the guard time");
            assert_eq!(h.d.app.ui.question, None, "QST-4, INP-15: {what}: the second finger on {button:?}");
            assert_eq!(items(&h.d), before, "NAV-16, INP-15: {what}: the second finger on {button:?} operated it");
            h.send(Input::TouchUp { id: 2, x, y }).unwrap();
            h.send(Input::TouchUp { id: 1, x: on_board.0, y: on_board.1 }).unwrap();
            assert_eq!(items(&h.d), before, "NAV-16, INP-15: {what}: the lifts, {button:?}");
        }
    }
}

#[test]
fn a_first_press_on_the_toolbar_in_the_guard_time_keeps_the_overlays() {
    // NAV-16, INP-15, MRK-15, QST-4: the other side of the owner decision of 2026-11-05. With no other
    // finger on the screen, or when the other touch has had no event for 0.15 s (a lost release), the
    // press is a first press. The app ignores it, thus the x and the "?" stay.
    let mut h = hands_with_history(410);
    let on_board = micro_center((4, 4), 5);
    let (x, y) = button_center(Button::Colors);
    for stale_finger in [false, true] {
        back_from_manual(&mut h);
        let (cell, digit) = first_refused_spot(&h.d.app.game);
        let before = items(&h.d);
        if stale_finger {
            h.send(Input::TouchDown { id: 1, x: on_board.0, y: on_board.1 }).unwrap();
        }
        let (px, py) = micro_center(cell, digit);
        h.send(Input::PenDown { x: px, y: py, tool: Tool::Pen }).unwrap();
        h.send(Input::PenUp { x: px, y: py }).unwrap();
        let x_shown = Some((cell, digit, h.d.now));
        assert_eq!(h.d.app.ui.refused, x_shown, "MRK-10");
        if stale_finger {
            h.wait(150).unwrap();
        }
        h.send(Input::TouchDown { id: 2, x, y }).unwrap();
        assert!(h.d.app.toolbar_ignored_until.is_some_and(|t| h.d.now < t), "the press is in the guard time");
        assert_eq!(h.d.app.ui.refused, x_shown, "NAV-16: an ignored first press, lost release {stale_finger}");
        assert_eq!(items(&h.d), before, "NAV-16: an ignored first press, lost release {stale_finger}");
        h.send(Input::TouchUp { id: 2, x, y }).unwrap();
        assert_eq!(items(&h.d), before, "NAV-16: the lift, lost release {stale_finger}");
        h.d.wait(OVERLAY_MS);
    }

    // The "?" with no other finger. The usual pause is as long as the guard time, thus the test sets a
    // short pause.
    h.d.app.pause_ms = 50;
    back_from_manual(&mut h);
    let empty = cells().find(|&c| !h.d.app.game.is_filled(c)).unwrap();
    h.d.write_strokes(&scribble_ink(empty));
    h.d.wait(60);
    let question = h.d.app.ui.question;
    assert_eq!(question.map(|q| q.0), Some(empty), "the pause reads the ink");
    h.send(Input::TouchDown { id: 2, x, y }).unwrap();
    assert!(h.d.app.toolbar_ignored_until.is_some_and(|t| h.d.now < t), "the press is in the guard time");
    assert_eq!(h.d.app.ui.question, question, "NAV-16: an ignored first press keeps the \"?\"");
    h.send(Input::TouchUp { id: 2, x, y }).unwrap();
}

// ---- HLT-2, HLT-4, HLT-6, HLT-7, HLT-8: the sequences from a highlight. No ink waits, thus only an item of
// the list of HLT-2 can remove the highlight. Each input of these sequences is in the list of HLT-7, or it
// operates a control at a random point.

#[derive(Clone, Debug)]
enum Keep {
    /// A finger tap at a point.
    Finger(u16, u16),
    /// A finger moves from one point to a different point.
    FingerMove(u16, u16, u16, u16),
    /// A second finger on a point while a first finger stays on a point.
    SecondFinger(u16, u16, u16, u16),
    /// A finger tap on a button.
    Button(u8),
    /// A finger tap on a switch that is not `highlight_on`. The act opens the panel first.
    OtherSwitch(bool),
    /// The eraser in the cell at (row, column).
    Eraser(u8, u8),
    /// The eraser on the first entry of the board, if the board has one.
    EraseEntry,
    /// A pen tap at a point that is not on the board.
    PenOff(u16, u16),
    /// A pen press on the cell at (row, column) with an open panel. It only closes the panel, also if
    /// the pen moves. The act opens the panel first.
    PenWithPanel(u8, u8, u8, bool),
    /// A hand touch at a point, after a pen lift off the board.
    Hand(u16, u16, u8),
    Turn,
    Suspend,
    Wait(u16),
}

fn keep_act() -> impl Strategy<Value = Keep> {
    let point = (0..1620u16, 0..2160u16);
    prop_oneof![
        5 => point.clone().prop_map(|(x, y)| Keep::Finger(x, y)),
        2 => (point.clone(), point.clone()).prop_map(|((x0, y0), (x1, y1))| Keep::FingerMove(x0, y0, x1, y1)),
        2 => (point.clone(), point.clone()).prop_map(|((x0, y0), (x1, y1))| Keep::SecondFinger(x0, y0, x1, y1)),
        8 => (0..CONTROLS.len() as u8).prop_map(Keep::Button),
        2 => any::<bool>().prop_map(Keep::OtherSwitch),
        3 => (0..9u8, 0..9u8).prop_map(|(r, c)| Keep::Eraser(r, c)),
        3 => Just(Keep::EraseEntry),
        3 => point.clone().prop_map(|(x, y)| Keep::PenOff(x, y)),
        3 => (0..9u8, 0..9u8, 0..4u8, any::<bool>()).prop_map(|(r, c, p, m)| Keep::PenWithPanel(r, c, p, m)),
        3 => (point, 0..HAND_TOUCH_MS as u8).prop_map(|((x, y), ms)| Keep::Hand(x, y, ms)),
        1 => Just(Keep::Turn),
        1 => Just(Keep::Suspend),
        2 => (0..3000u16).prop_map(Keep::Wait),
    ]
}

/// The items that the sequences from a highlight count.
#[derive(Default)]
struct KeepReached {
    with_panel: usize,
    undo: usize,
    eraser_changed: usize,
    pen_closed_panel: usize,
    hand: usize,
    turn: usize,
    suspend: usize,
    /// A control at a random point did an item of the list of HLT-2: the sequence stops there.
    removed: usize,
}

/// A driver with a highlight, an entry and a redo. Returns the driver and its layout.
fn keep_start(seed: u64) -> (Driver, InLayout) {
    let layout = InLayout::set(seed & 1 != 0, seed & 2 != 0);
    let mut d = Driver::new(seed);
    let cell = cells().find(|&c| !d.app.game.is_filled(c)).unwrap();
    let v = d.app.game.solution[cell.0 as usize][cell.1 as usize];
    d.write(cell, v);
    let number = if seed & 4 == 0 { cell } else { cells().find(|&c| d.app.game.is_given(c)).unwrap() };
    let (x, y) = micro_center(number, 5);
    d.pen_tap(x, y);
    assert_eq!(d.app.ui.highlight, Some(d.app.game.value_at(number)), "the fixture has the highlight");
    (d, layout)
}

/// One act of the sequences. Returns false when an item of the list of HLT-2 removed the highlight.
fn keep_step(d: &mut Driver, a: &Keep, r: &mut KeepReached) -> Result<bool, String> {
    let h = d.app.ui.highlight;
    let seed = d.app.game.seed;
    let undo = d.app.game.undo.len();
    let value = d.app.game.value;
    if !matches!(a, Keep::Finger(..) | Keep::FingerMove(..) | Keep::SecondFinger(..) | Keep::Wait(_) | Keep::Turn | Keep::Suspend | Keep::Hand(..)) {
        wait_for_toolbar(d);
    }
    match *a {
        Keep::Finger(x, y) => d.finger_tap(x as f32, y as f32),
        Keep::FingerMove(x0, y0, x1, y1) => {
            d.send(Input::TouchDown { id: 1, x: x0 as f32, y: y0 as f32 });
            d.send(Input::TouchMove { id: 1, x: x1 as f32, y: y1 as f32 });
            d.send(Input::TouchUp { id: 1, x: x1 as f32, y: y1 as f32 });
        }
        Keep::SecondFinger(x0, y0, x1, y1) => {
            d.send(Input::TouchDown { id: 1, x: x0 as f32, y: y0 as f32 });
            d.send(Input::TouchDown { id: 2, x: x1 as f32, y: y1 as f32 });
            d.send(Input::TouchUp { id: 2, x: x1 as f32, y: y1 as f32 });
            // The release of the first finger is off a control: the second finger operates nothing.
            d.send(Input::TouchUp { id: 1, x: x0 as f32 + 300.0, y: y0 as f32 + 300.0 });
        }
        Keep::Button(i) => {
            let b = CONTROLS[i as usize];
            if !d.app.ui.menu.is_open() && d.app.screen == Screen::Game {
                d.button(Button::Menu);
            }
            d.button(b);
        }
        Keep::OtherSwitch(first) => {
            if d.app.screen == Screen::Game {
                if !d.app.ui.menu.is_open() {
                    d.button(Button::Menu);
                }
                if d.app.ui.menu.panel() != Some(Panel::Settings) {
                    d.button(Button::Settings);
                }
                let (x, y) = switch_point(if first { Switch::HideMarksAutomatically } else { Switch::ColorCompletedGroups });
                d.finger_tap(x, y);
            }
        }
        Keep::Eraser(row, col) => d.erase((row, col)),
        Keep::EraseEntry => {
            if let Some(cell) = cells().find(|&c| d.app.game.is_entry(c)) {
                d.erase(cell);
            }
        }
        Keep::PenOff(x, y) => {
            if layout::board_micro(x as f32, y as f32).is_none() {
                d.pen_tap(x as f32, y as f32);
            }
        }
        Keep::PenWithPanel(row, col, p, moves) => {
            if d.app.screen == Screen::Game {
                if !d.app.ui.menu.is_open() {
                    d.button(Button::Menu);
                }
                let (button, panel) = [(Button::Colors, Panel::Colors), (Button::Settings, Panel::Settings), (Button::Checkpoints, Panel::Checkpoints), (Button::NewGame, Panel::NewGame)][p as usize];
                if d.app.ui.menu.panel() != Some(panel) {
                    d.button(button);
                }
                let (x, y) = micro_center((row, col), 5);
                d.send(Input::PenDown { x, y, tool: Tool::Pen });
                if d.app.live_stroke.is_some() {
                    return Err(format!("NAV-11: ink with the panel {panel:?}"));
                }
                if moves {
                    d.send(Input::PenMove { x: x + 90.0, y: y + 90.0 });
                    if d.app.live_stroke.is_some() || d.app.ui.pending_ink.is_some() {
                        return Err(format!("NAV-11: ink with the panel {panel:?} after the pen moves"));
                    }
                }
                d.send(Input::PenUp { x: x + 90.0, y: y + 90.0 });
                r.pen_closed_panel += usize::from(d.app.ui.menu.panel().is_none());
            }
        }
        Keep::Hand(x, y, ms) => {
            // The pen lift is on no control. With a panel, the pen press would close it.
            if d.app.ui.menu.panel().is_none() && d.app.screen == Screen::Game {
                let (ox, oy) = (0..layout::screen_h() as i32)
                    .step_by(40)
                    .flat_map(|y| (0..layout::screen_w() as i32).step_by(40).map(move |x| (x as f32 + 5.0, y as f32 + 5.0)))
                    .find(|&(x, y)| layout::board_micro(x, y).is_none() && layout::game_button(x, y, true).is_none() && layout::game_button(x, y, false).is_none())
                    .expect("a point off the board");
                d.pen_tap(ox, oy);
                d.touch_down_after_lift(ms as u64, x as f32, y as f32);
                d.send_at_once(Input::TouchUp { id: 1, x: x as f32, y: y as f32 });
                r.hand += 1;
                if d.app.game.seed != seed || d.app.screen != Screen::Game || !d.app.settings.highlight_on || d.app.ui.highlight != h {
                    return Err(format!("INP-20: a hand touch at ({x}, {y}), {ms} ms after a pen lift, changed the state"));
                }
            }
        }
        Keep::Turn => {
            let landscape = layout::screen_w() < layout::screen_h();
            layout::set_orientation(if landscape { layout::Orientation::Landscape } else { layout::Orientation::Portrait });
            d.app.rotate(App::placeholder_ng_stops());
            r.turn += 1;
        }
        Keep::Suspend => {
            d.app.suspend();
            d.now += 600_000;
            d.app.tick(d.now);
            r.suspend += 1;
        }
        Keep::Wait(ms) => d.wait(ms as u64),
    }
    d.app.quit = false;
    check_app(&d.app)?;
    highlight_state_rule(&d.app)?;
    if d.app.ui.pending_ink.is_some() || d.app.live_stroke.is_some() {
        return Err("the sequences from a highlight must have no ink".to_string());
    }
    let after = d.app.ui.highlight;
    // HLT-2 items 5, 6 and 8: the items that a control at a random point can do. The list of the other
    // items needs a pen on the board, or ink, or the update dialog: the sequences do not have them.
    let removed = d.app.game.seed != seed || d.app.game.completed || d.app.screen != Screen::Game || !d.app.settings.highlight_on;
    if removed {
        if after.is_some() {
            return Err(format!("HLT-1, HLT-2, HLT-4, HLT-6: the highlight {after:?} stays on {:?}", d.app.screen));
        }
        r.removed += 1;
        return Ok(false);
    }
    if after != h {
        return Err(format!("HLT-7: the highlight changed from {h:?} to {after:?}"));
    }
    r.with_panel += usize::from(d.app.ui.menu.panel().is_some());
    r.undo += usize::from(d.app.game.undo.len() != undo);
    r.eraser_changed += usize::from(matches!(a, Keep::Eraser(..) | Keep::EraseEntry) && d.app.game.value != value);
    Ok(true)
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 48, max_shrink_iters: 1000, ..ProptestConfig::default() })]

    /// HLT-2, HLT-4, HLT-6, HLT-7, HLT-8, INP-20, NAV-11 in all layouts, at each act.
    #[test]
    fn a_highlight_stays_for_all_input_that_does_not_remove_it(seed in any::<u64>(), acts in proptest::collection::vec(keep_act(), 1..40)) {
        let (mut d, _layout) = keep_start(seed);
        let mut reached = KeepReached::default();
        for a in &acts {
            let more = keep_step(&mut d, a, &mut reached).map_err(|e| TestCaseError::fail(format!("{e} after {a:?}")))?;
            if !more {
                break;
            }
        }
    }
}

#[test]
fn the_sequences_from_a_highlight_reach_each_input() {
    // The sequences must reach each input that their checks examine: else they prove nothing.
    use proptest::strategy::ValueTree;
    use proptest::test_runner::TestRunner;
    let mut runner = TestRunner::deterministic();
    let mut r = KeepReached::default();
    for case in 0..48 {
        let acts = proptest::collection::vec(keep_act(), 1..40).new_tree(&mut runner).unwrap().current();
        let (mut d, _layout) = keep_start(case);
        for a in &acts {
            if !keep_step(&mut d, a, &mut r).unwrap_or_else(|e| panic!("{e} after {a:?}")) {
                break;
            }
        }
    }
    println!("panel {}, undo {}, eraser {}, pen closed a panel {}, hand {}, turn {}, suspend {}, removed {}", r.with_panel, r.undo, r.eraser_changed, r.pen_closed_panel, r.hand, r.turn, r.suspend, r.removed);
    assert!(r.with_panel > 0, "HLT-4: no highlight with a panel");
    assert!(r.undo > 0, "HLT-7: no Undo or Redo kept the highlight");
    assert!(r.eraser_changed > 0, "HLT-7: no eraser changed the board with a highlight");
    assert!(r.pen_closed_panel > 0, "NAV-11: no pen press closed a panel");
    assert!(r.hand > 0, "INP-20: no hand touch");
    assert!(r.turn > 0 && r.suspend > 0, "HLT-7: no turn or suspension");
    assert!(r.removed > 0, "HLT-2: no control did an item of the list");
}

// ---- COL-8 to COL-12: the group sequences. They start from a board that is almost full, thus groups get
// and lose the color often. The rules come from `inv_groups`, not from the code of the app.

/// COL-8, COL-9, COL-11, COL-12 for one state, against the separate model.
fn group_state_rule(app: &App) -> Result<(), String> {
    let (g, s) = (&app.game, &app.settings);
    let want = inv_groups::cells_in_complete_groups(&g.value);
    let set = g.completed_cells();
    for cell in cells() {
        let (r, c) = (cell.0 as usize, cell.1 as usize);
        if set.contains(cell) != want[r][c] {
            return Err(format!("COL-8: the cell {cell:?}: the app {}, the model {}", set.contains(cell), want[r][c]));
        }
        if want[r][c] && g.value[r][c] == 0 {
            return Err(format!("COL-8: the empty cell {cell:?} is in a completed group"));
        }
        let shows = inv_groups::shows_completed(s, g, cell);
        if s.shows_completed(g, cell) != shows {
            return Err(format!("COL-11, COL-12: the cell {cell:?}: the model {shows}, the switch {}, the color {:?}, completed {}", s.completed_color_on, s.color_completed, g.completed));
        }
    }
    type MakeGroup = fn(u8) -> Group;
    let kinds: [(u8, MakeGroup); 3] = [(inv_groups::ROW, Group::Row), (inv_groups::COLUMN, Group::Column), (inv_groups::BOX, Group::Box)];
    for (kind, make) in kinds {
        for i in 0..9u8 {
            let group = inv_groups::group_cells(kind, i);
            let complete = inv_groups::group_is_complete(&g.value, &group);
            if make(i).cells() != group {
                return Err(format!("COL-8: the cells of {:?}", make(i)));
            }
            if g.is_completed_group(make(i)) != complete {
                return Err(format!("COL-8: {:?}: the model {complete}", make(i)));
            }
            if complete {
                // COL-9: this follows from the other rules. A failure here is a failure of the model or of a rule.
                let digits = group.iter().fold(0u16, |m, &(r, c)| m | mark_bit(g.value[r as usize][c as usize]));
                if digits != (1..=9u8).fold(0, |m, d| m | mark_bit(d)) {
                    return Err(format!("COL-9: {:?} does not have the digits 1 to 9", make(i)));
                }
            }
        }
    }
    // COL-9: a function of `value` only. All other values of the game change, and the set does not.
    let mut other = g.clone();
    other.solution = [[1; 9]; 9];
    other.given = [[false; 9]; 9];
    other.marks = [[0; 9]; 9];
    other.undo.clear();
    other.redo.clear();
    other.completed = !g.completed;
    other.elapsed_ms = g.elapsed_ms + 1;
    if other.completed_cells() != set {
        return Err("COL-9: the set of completed groups depends on a value that is not `value`".to_string());
    }
    Ok(())
}

/// A game of the seed with all empty cells correct, except each fifth cell.
fn almost_full_game(seed: u64) -> Game {
    let mut g = sudoku::app::new_game(seed, 2);
    let empty: Vec<Cell> = cells().filter(|&c| !g.is_filled(c)).collect();
    for (i, &(r, c)) in empty.iter().enumerate() {
        if i % 5 != 0 {
            g.add_number((r, c), g.solution[r as usize][c as usize]);
        }
    }
    g
}

/// The acts of the group sequences: most of them change the board.
fn group_act() -> impl Strategy<Value = Act> {
    let cell = (0..9u8, 0..9u8);
    prop_oneof![
        5 => cell.clone().prop_map(|(r, c)| Act::WriteSolution(r, c)),
        4 => (cell.clone(), 1..=9u8).prop_map(|((r, c), d)| Act::Write(r, c, d)),
        5 => cell.clone().prop_map(|(r, c)| Act::Erase(r, c)),
        // The indexes of Undo and Redo in `BUTTONS`.
        4 => Just(Act::Button(2)),
        3 => Just(Act::Button(3)),
        1 => cell.prop_map(|(r, c)| Act::TapMicro(r, c, 5)),
        1 => (0..Section::ALL.len() as u8, 0..9u8).prop_map(|(s, i)| Act::PickColor(Section::ALL[s as usize], Color::ALL[i as usize])),
        1 => Just(Act::Switch(Switch::ColorCompletedGroups)),
        1 => (0..2u8).prop_map(|m| Act::NewGame(m, true)),
    ]
}

/// The items that the group sequences count, to prove that they reach the state of each rule.
#[derive(Default)]
struct GroupsReached {
    /// A cell got into a completed group.
    gained: usize,
    /// A cell went out of a completed group.
    lost: usize,
    /// One changed cell changed a group of a cell that is not its peer (COL-10).
    far: usize,
    /// A completed group has an entry with `value ≠ solution` (COL-9).
    incorrect: usize,
    /// A completed group with no completed color, on a game in progress (COL-11).
    hidden: usize,
    /// A complete game (COL-12).
    completed: usize,
    /// A new game (COL-10).
    new_game: usize,
}

/// One sequence. The layout comes from the seed. A second app has the opposite position of the switch:
/// its game and its results must stay the same (COL-10). After each act, the state agrees with the model,
/// and each cell has the background of the model (BRD-7).
fn group_sequence(seed: u64, acts: &[Act]) -> Result<GroupsReached, String> {
    let _layout = InLayout::set(seed & 2 != 0, seed & 4 != 0);
    let game = almost_full_game(seed);
    let fonts = fonts();
    let mut d = Driver::opened(app_with_game(game.clone(), START_TIME, seed));
    let mut twin = Driver::opened(app_with_game(game, START_TIME, seed));
    twin.app.settings.completed_color_on = false;
    let mut reached = GroupsReached::default();
    for a in acts {
        let before = d.app.game.clone();
        let results_before = d.app.results.clone();
        let cells_before = inv_groups::cells_in_complete_groups(&before.value);
        run(&mut d, a);
        run(&mut twin, a);
        let fail = |e: String| format!("{e} after {a:?}");
        check_app(&d.app).map_err(fail)?;
        group_state_rule(&d.app).map_err(fail)?;
        let g = &d.app.game;
        if *g != twin.app.game {
            return Err(fail("COL-10: the color changed the game, the stacks or the clock".to_string()));
        }
        if d.app.results != twin.app.results {
            return Err(fail("COL-10: the color changed the results".to_string()));
        }
        let new_game = g.seed != before.seed;
        if !new_game {
            // UND-1, UND-2: only a change of the numbers or of the marks changes the stacks.
            if g.value == before.value && g.marks == before.marks && (g.undo.len(), g.redo.len(), g.completed) != (before.undo.len(), before.redo.len(), before.completed) {
                return Err(fail("UND-2, COL-10: the stacks or `completed` changed with no change of the board".to_string()));
            }
            // CMP-5, ANA-2: one result only at completion.
            let want = results_before.len() + usize::from(g.completed && !before.completed);
            if d.app.results.len() != want {
                return Err(fail(format!("COL-10, ANA-2: {} results, not {want}", d.app.results.len())));
            }
        }
        if d.app.screen == Screen::Game && d.app.ui.menu.panel().is_none() {
            let mut pm = tiny_skia::Pixmap::new(layout::screen_w(), layout::screen_h()).unwrap();
            sudoku::render::draw(&mut sudoku::render::Canvas::new(&mut pm, &fonts), &d.app);
            if let Some(e) = inv_groups::board_color_error(&pm, &d.app) {
                return Err(fail(e));
            }
        }

        // The items that the sequences reach.
        let now = inv_groups::cells_in_complete_groups(&g.value);
        reached.new_game += usize::from(new_game);
        reached.completed += usize::from(g.completed);
        if !new_game {
            let flipped: Vec<Cell> = cells().filter(|&(r, c)| now[r as usize][c as usize] != cells_before[r as usize][c as usize]).collect();
            reached.gained += flipped.iter().filter(|&&(r, c)| now[r as usize][c as usize]).count();
            reached.lost += flipped.iter().filter(|&&(r, c)| !now[r as usize][c as usize]).count();
            let changed: Vec<Cell> = cells().filter(|&(r, c)| g.value[r as usize][c as usize] != before.value[r as usize][c as usize]).collect();
            if let [one] = changed[..] {
                reached.far += flipped.iter().filter(|&&f| f != one && !inv_groups::are_peers(f, one)).count();
            }
        }
        reached.incorrect += cells().filter(|&(r, c)| now[r as usize][c as usize] && g.value[r as usize][c as usize] != g.solution[r as usize][c as usize]).count();
        reached.hidden += usize::from(!g.completed && cells().any(|c| now[c.0 as usize][c.1 as usize] && !d.app.settings.shows_completed(g, c)));
        d.app.quit = false;
        twin.app.quit = false;
    }
    Ok(reached)
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 24, max_shrink_iters: 200, ..ProptestConfig::default() })]

    /// COL-8 to COL-12, BRD-7, UND-2 after random numbers, erasures, undo and redo, in all layouts.
    #[test]
    fn the_completed_groups_follow_each_change(seed in any::<u64>(), acts in proptest::collection::vec(group_act(), 1..40)) {
        group_sequence(seed, &acts).map_err(TestCaseError::fail)?;
    }
}

#[test]
fn the_group_sequences_reach_each_rule() {
    // The group sequences must reach each state that their checks examine: else they prove nothing.
    use proptest::strategy::ValueTree;
    use proptest::test_runner::TestRunner;
    let mut runner = TestRunner::deterministic();
    let mut all = GroupsReached::default();
    for case in 0..24 {
        let acts = proptest::collection::vec(group_act(), 20..40).new_tree(&mut runner).unwrap().current();
        let r = group_sequence(case, &acts).unwrap_or_else(|e| panic!("{e}"));
        all.gained += r.gained;
        all.lost += r.lost;
        all.far += r.far;
        all.incorrect += r.incorrect;
        all.hidden += r.hidden;
        all.completed += r.completed;
        all.new_game += r.new_game;
    }
    println!("gained {}, lost {}, far {}, incorrect {}, hidden {}, completed {}, new game {}", all.gained, all.lost, all.far, all.incorrect, all.hidden, all.completed, all.new_game);
    assert!(all.gained > 0, "COL-10: no cell got the color");
    assert!(all.lost > 0, "COL-10: no cell lost the color");
    assert!(all.far > 0, "COL-10: no far cell changed");
    assert!(all.hidden > 0, "COL-11: no completed group with no color");
    assert!(all.new_game > 0, "COL-10: no new game");
}

// ---- UND-1, UND-3, UND-6, UND-9, CMP-1, CMP-3, CKP-1 to CKP-5, CKP-8, CKP-9, PER-1: the checkpoint
// sequences. A separate model keeps the boards of the two histories and the list of checkpoints. It does
// not use the code of the app. All controls operate with taps at the positions of `layout`, in all layouts.

/// The numbers and all marks of a board.
type Snap = ([[u8; 9]; 9], [[u16; 9]; 9]);

fn snap(g: &Game) -> Snap {
    (g.value, g.marks)
}

/// The acts of the checkpoint sequences. The cell is an index in the list of open cells.
#[derive(Clone, Debug)]
enum CkpAct {
    Mark(u8, u8),
    Write(u8, u8),
    Correct(u8),
    Erase(u8),
    Undo,
    Redo,
    /// `true`: the pen. `false`: a finger.
    Save(bool),
    Restore(u8, bool),
    /// `true` as the second value: a double tap (CKP-8).
    Delete(u8, bool),
    Wait(u16),
    NewGame,
}

fn ckp_act() -> impl Strategy<Value = CkpAct> {
    prop_oneof![
        3 => (0..4u8, 1..=9u8).prop_map(|(i, d)| CkpAct::Mark(i, d)),
        2 => (0..4u8, 1..=9u8).prop_map(|(i, d)| CkpAct::Write(i, d)),
        3 => (0..4u8).prop_map(CkpAct::Correct),
        2 => (0..4u8).prop_map(CkpAct::Erase),
        4 => Just(CkpAct::Undo),
        3 => Just(CkpAct::Redo),
        4 => any::<bool>().prop_map(CkpAct::Save),
        4 => (0..5u8, any::<bool>()).prop_map(|(i, pen)| CkpAct::Restore(i, pen)),
        2 => (0..5u8, any::<bool>()).prop_map(|(i, double)| CkpAct::Delete(i, double)),
        1 => (0..2000u16).prop_map(CkpAct::Wait),
    ]
}

/// The same acts, and sometimes a new game.
fn ckp_act_with_new_game() -> impl Strategy<Value = CkpAct> {
    prop_oneof![30 => ckp_act(), 1 => Just(CkpAct::NewGame)]
}

/// A game in progress with all cells correct, except 1 to 3 open cells: then the sequences get to the
/// completion. The model keeps the boards of the numbers that fill the other cells.
fn ckp_start(seed: u64) -> (Game, Vec<Cell>, Vec<Snap>) {
    let mut g = sudoku::app::new_game(seed, 2);
    let empty: Vec<Cell> = cells().filter(|&c| !g.is_filled(c)).collect();
    let (open, rest) = empty.split_at(1 + seed as usize % 3);
    let mut past = Vec::new();
    for &(r, c) in rest {
        past.push(snap(&g));
        g.add_number((r, c), g.solution[r as usize][c as usize]);
    }
    (g, open.to_vec(), past)
}

/// The model of the game: the two histories and the list.
struct CkpModel {
    /// The board before each item of `undo`.
    past: Vec<Snap>,
    /// The board after each item of `redo`.
    future: Vec<Snap>,
    /// The boards and the clocks, the newest first.
    list: Vec<(Snap, u64)>,
}

/// The items that the checkpoint sequences count, to prove that they reach the state of each rule.
#[derive(Default)]
struct CkpReached {
    saves: usize,
    /// A tap on the row "Save checkpoint" with a full list.
    full: usize,
    /// A tap on the row "Save checkpoint" when a checkpoint that is not the newest has the board.
    same_old: usize,
    same_new: usize,
    restores: usize,
    /// A tap on "Current" with a redo history that stays.
    current_keeps_redo: usize,
    /// A Restore that removed a redo history.
    restore_clears_redo: usize,
    deletes: usize,
    /// A second tap of a double tap in the guard time, with a different row at that position.
    guarded: usize,
    undo_of_restore: usize,
    redo_of_restore: usize,
    /// A Restore item changes the board, and no checkpoint in the list has the board of the item.
    restore_item_without_checkpoint: usize,
    /// A game that became complete after a Restore.
    completed_after_restore: usize,
    /// A tap on the controls of a complete game.
    acts_on_complete: usize,
    full_board_not_solved: usize,
    new_games: usize,
}

/// True if the top of the undo history is a Restore item.
fn top_is_restore(stack: &[Change]) -> bool {
    matches!(stack.last(), Some(Change::Restore { .. }))
}

/// CMP-1, CKP-4, UND-1, UND-3, CKP-1 and PER-1 for one state, against the model.
fn ckp_state_rule(d: &Driver, m: &CkpModel) -> Result<(), String> {
    let g = &d.app.game;
    check_app(&d.app)?;
    // UND-1, UND-3: the stacks have one item for each change. The list has the boards of the model.
    if g.undo.len() != m.past.len() || g.redo.len() != m.future.len() {
        return Err(format!("UND-1: the stacks have {} and {} items, the model {} and {}", g.undo.len(), g.redo.len(), m.past.len(), m.future.len()));
    }
    // CMP-1: from the values only, with the clash rule of the model.
    let full = g.value.iter().flatten().all(|&v| v != 0);
    let solved = full && !cells().any(|c| inv_groups::clashing(&g.value, c));
    if g.completed != solved {
        return Err(format!("CMP-1: completed = {}, the model {solved}", g.completed));
    }
    // CKP-1, CKP-3, CKP-4.
    let list: Vec<(Snap, u64)> = g.checkpoints.iter().map(|k| ((k.board.value, k.board.marks), k.elapsed_ms)).collect();
    if list != m.list {
        return Err("CKP-1, CKP-3: the list is not the list of the model".to_string());
    }
    if list.len() > 5 {
        return Err(format!("CKP-4: {} checkpoints", list.len()));
    }
    for (i, (board, clock)) in list.iter().enumerate() {
        if list[..i].iter().any(|(b, _)| b == board) {
            return Err(format!("CKP-4: checkpoint {i} has the board of a newer checkpoint"));
        }
        let limit = if i == 0 { g.elapsed_ms } else { list[i - 1].1 };
        if *clock > limit {
            return Err(format!("CKP-4: the clock of checkpoint {i} is {clock}, and the limit is {limit}"));
        }
        // CKP-5: the app never makes a board that the load refuses.
        let full = board.0.iter().flatten().all(|&v| v != 0);
        if full && !cells().any(|c| inv_groups::clashing(&board.0, c)) {
            return Err(format!("CKP-5: checkpoint {i} is a solved board"));
        }
    }
    // CKP-9, CMP-3
    if g.completed && d.app.ui.menu.panel() == Some(Panel::Checkpoints) {
        return Err("CKP-9: the Checkpoints panel is open on a complete game".to_string());
    }
    // PER-1: the text of the file gives the same game, and the load accepts it (PER-5).
    let text = serde_json::to_string(g).map_err(|e| e.to_string())?;
    let loaded: Game = serde_json::from_str(&text).map_err(|e| format!("PER-1: {e}"))?;
    if &loaded != g {
        return Err("PER-1: the file does not give the same game".to_string());
    }
    Ok(())
}

/// Opens the menu if it is closed.
fn ckp_menu(d: &mut Driver) {
    if !d.app.ui.menu.is_open() {
        d.button(Button::Menu);
    }
}

/// Closes a panel and leaves the menu open. Then the board and undo and redo operate as usual (NAV-11).
fn ckp_no_panel(d: &mut Driver) {
    ckp_menu(d);
    if d.app.ui.menu.panel().is_some() {
        d.button(Button::Menu);
    }
}

/// Opens the panel. On a complete game the tap on the bookmark has no effect (CKP-9).
fn ckp_panel(d: &mut Driver) {
    ckp_menu(d);
    if d.app.ui.menu.panel() != Some(Panel::Checkpoints) {
        d.button(Button::Checkpoints);
    }
}

fn ckp_tap(d: &mut Driver, (x, y): (f32, f32), pen: bool) {
    if pen {
        d.pen_tap(x, y);
    } else {
        d.finger_tap(x, y);
    }
}

/// One act and its rules. The model gives the expected result of each act before the act.
fn ckp_step(d: &mut Driver, m: &mut CkpModel, open: &[Cell], a: &CkpAct, r: &mut CkpReached) -> Result<(), String> {
    let before = d.app.game.clone();
    let start = d.now;
    let was = snap(&before);
    let in_progress = !before.completed;
    let (mut expect_board, mut one_change) = (was, false);
    let mut stays_in_panel = false;
    let cell_of = |i: u8| open[i as usize % open.len()];
    match *a {
        CkpAct::Mark(i, digit) => {
            ckp_no_panel(d);
            let cell = cell_of(i);
            d.tap_micro(cell, digit);
            let free = !before.is_filled(cell) && (!d.app.settings.auto_hide_on || !peer_has(&before, cell, digit));
            if in_progress && free {
                let mut want = was;
                want.1[cell.0 as usize][cell.1 as usize] ^= mark_bit(digit);
                expect_board = want;
                one_change = true;
            }
        }
        CkpAct::Write(i, digit) => {
            ckp_no_panel(d);
            let cell = cell_of(i);
            d.write(cell, digit);
            if in_progress && !before.is_filled(cell) {
                let mut want = was;
                want.0[cell.0 as usize][cell.1 as usize] = digit;
                expect_board = want;
                one_change = true;
            }
        }
        CkpAct::Correct(i) => {
            ckp_no_panel(d);
            let cell = cell_of(i);
            let digit = before.solution[cell.0 as usize][cell.1 as usize];
            d.write(cell, digit);
            if in_progress && !before.is_filled(cell) {
                let mut want = was;
                want.0[cell.0 as usize][cell.1 as usize] = digit;
                expect_board = want;
                one_change = true;
            }
        }
        CkpAct::Erase(i) => {
            ckp_no_panel(d);
            let cell = cell_of(i);
            d.erase(cell);
            if in_progress && before.is_entry(cell) {
                let mut want = was;
                want.0[cell.0 as usize][cell.1 as usize] = 0;
                expect_board = want;
                one_change = true;
            }
        }
        CkpAct::Undo => {
            ckp_no_panel(d);
            d.button(Button::Undo);
            if in_progress {
                if let Some(board) = m.past.pop() {
                    r.undo_of_restore += usize::from(top_is_restore(&before.undo));
                    m.future.push(was);
                    expect_board = board;
                }
            }
        }
        CkpAct::Redo => {
            ckp_no_panel(d);
            d.button(Button::Redo);
            if in_progress {
                if let Some(board) = m.future.pop() {
                    r.redo_of_restore += usize::from(top_is_restore(&before.redo));
                    if top_is_restore(&before.redo) && !m.list.iter().any(|(b, _)| *b == board) {
                        r.restore_item_without_checkpoint += 1;
                    }
                    m.past.push(was);
                    expect_board = board;
                }
            }
        }
        CkpAct::Save(pen) => {
            ckp_panel(d);
            ckp_tap(d, exact_center(layout::ckp_save_row()), pen);
            let same = m.list.iter().position(|(b, _)| *b == was);
            r.full += usize::from(in_progress && m.list.len() == 5);
            r.same_new += usize::from(in_progress && same == Some(0));
            r.same_old += usize::from(in_progress && same.is_some_and(|i| i > 0));
            if in_progress && m.list.len() < 5 && same.is_none() {
                // CKP-1: the clock at the save, between the clock before the act and the clock after it.
                let Some(k) = d.app.game.checkpoints.first() else { return Err(format!("CKP-1: no checkpoint after {a:?}")) };
                if k.elapsed_ms < before.elapsed_ms || k.elapsed_ms > d.app.game.elapsed_ms {
                    return Err(format!("CKP-1: the clock of the checkpoint is {}", k.elapsed_ms));
                }
                m.list.insert(0, (was, k.elapsed_ms));
                r.saves += 1;
            }
            stays_in_panel = in_progress;
        }
        CkpAct::Restore(i, pen) => {
            ckp_panel(d);
            if !in_progress {
                ckp_tap(d, exact_center(layout::ckp_save_row()), pen);
            } else if !m.list.is_empty() {
                let i = i as usize % m.list.len();
                let target = m.list[i].0;
                ckp_tap(d, exact_center(layout::ckp_button(i)), pen);
                if target == was {
                    // CKP-2: "Current". The redo history stays.
                    r.current_keeps_redo += usize::from(!m.future.is_empty());
                } else {
                    r.restores += 1;
                    r.restore_clears_redo += usize::from(!m.future.is_empty());
                    expect_board = target;
                    one_change = true;
                }
            }
            stays_in_panel = in_progress;
        }
        CkpAct::Delete(i, double) => {
            ckp_panel(d);
            if !in_progress {
                ckp_tap(d, exact_center(layout::ckp_save_row()), false);
            } else if !m.list.is_empty() {
                let i = i as usize % m.list.len();
                let bin = exact_center(layout::ckp_bin_area(i));
                d.finger_tap(bin.0, bin.1);
                m.list.remove(i);
                r.deletes += 1;
                if double {
                    // CKP-8: the second tap of a fast double tap has no effect.
                    d.finger_tap(bin.0, bin.1);
                    r.guarded += usize::from(i < m.list.len());
                }
                d.wait(510);
            }
            stays_in_panel = in_progress;
        }
        CkpAct::Wait(ms) => d.wait(ms as u64),
        CkpAct::NewGame => {
            run(d, &Act::NewGame(0, true));
            if d.app.game.seed != before.seed {
                let g = &d.app.game;
                if !(g.undo.is_empty() && g.redo.is_empty() && g.checkpoints.is_empty() && !g.completed) {
                    return Err("UND-6, DIF-5: a new game keeps the stacks or the checkpoints".to_string());
                }
                *m = CkpModel { past: Vec::new(), future: Vec::new(), list: Vec::new() };
                r.new_games += 1;
                return ckp_state_rule(d, m);
            }
        }
    }
    if snap(&d.app.game) != expect_board {
        return Err(format!("UND-1, UND-3, CKP-2: the board is not the board of the model after {a:?}"));
    }
    if one_change {
        // UND-1: one item, and no redo history.
        m.past.push(was);
        m.future.clear();
    }
    // CMP-1: only a new game sets it to false.
    if before.completed && !d.app.game.completed {
        return Err(format!("CMP-1: a game stopped being complete after {a:?}"));
    }
    // CMP-3, CKP-9: a complete game does not change.
    if before.completed {
        r.acts_on_complete += 1;
        if d.app.game != before {
            return Err(format!("CMP-3: a complete game changed after {a:?}"));
        }
    }
    if stays_in_panel && d.app.ui.menu.panel() != Some(Panel::Checkpoints) {
        return Err(format!("CKP-2: the panel closed after {a:?}"));
    }
    // UND-9, CKP-1, CKP-2, CKP-3: the clock only counts the time of the act. The time of an act is at
    // most the time of the driver.
    let (elapsed, time) = (d.app.game.elapsed_ms, d.now - start);
    if !before.completed && (elapsed < before.elapsed_ms || elapsed > before.elapsed_ms + time) {
        return Err(format!("UND-9: the clock went from {} to {elapsed} in {time} ms after {a:?}", before.elapsed_ms));
    }
    if !before.completed && d.app.game.completed {
        // The sequences count the completion that comes after a Restore.
        r.completed_after_restore += usize::from(d.app.game.undo.iter().any(|c| matches!(c, Change::Restore { .. })));
    }
    if d.app.game.value.iter().flatten().all(|&v| v != 0) && !d.app.game.completed {
        r.full_board_not_solved += 1;
    }
    ckp_state_rule(d, m)
}

/// One sequence, then the file: the app opens with the same game, and all items undo and redo to the boards
/// of the model (PER-1, PER-2, UND-3, CKP-3).
fn ckp_sequence(seed: u64, acts: &[CkpAct]) -> Result<CkpReached, String> {
    let _layout = InLayout::set(seed & 2 != 0, seed & 4 != 0);
    let (game, open, past) = ckp_start(seed);
    let mut d = Driver::opened(app_with_game(game.clone(), START_TIME, seed));
    if d.app.game != game {
        return Err("PER-5: the app did not use the start game".to_string());
    }
    // With the switch off, a tap adds each mark (MRK-7): the board of a game that is almost full gets more
    // different boards, and the list gets full.
    d.app.settings.auto_hide_on = seed & 8 == 0;
    let mut m = CkpModel { past, future: Vec::new(), list: Vec::new() };
    let mut r = CkpReached::default();
    ckp_state_rule(&d, &m)?;
    for a in acts {
        ckp_step(&mut d, &mut m, &open, a, &mut r)?;
        d.app.quit = false;
    }

    // PER-1, PER-2: the file gives the same game, and the app uses it (CKP-5).
    let text = serde_json::to_string(&d.app.game).unwrap();
    let loaded: Game = serde_json::from_str(&text).unwrap();
    let reopened = app_with_game(loaded, START_TIME, seed);
    if reopened.game != d.app.game {
        return Err("PER-1, PER-5: the app did not use the saved game".to_string());
    }
    if reopened.ui.menu.panel().is_some() {
        return Err("PER-2: a panel is open at the start".to_string());
    }
    // UND-3, CKP-3: down the undo history and up again, with no checkpoints in the list.
    let mut g = reopened.game.clone();
    if !g.completed {
        while !g.checkpoints.is_empty() {
            g.delete_checkpoint(0);
        }
        let top = snap(&g);
        let mut going_up = vec![top];
        for want in m.past.iter().rev() {
            g.undo();
            if snap(&g) != *want {
                return Err("UND-3: undo after the file is not the board of the model".to_string());
            }
            going_up.push(*want);
        }
        if g.can_undo() {
            return Err("UND-1: the undo history has more items than the model".to_string());
        }
        going_up.pop();
        while let Some(want) = going_up.pop() {
            g.redo();
            if snap(&g) != want {
                return Err("UND-3: redo after the file is not the board of the model".to_string());
            }
        }
        for want in m.future.iter().rev() {
            g.redo();
            if snap(&g) != *want {
                return Err("UND-3: an item of the redo history after the file is not the board of the model".to_string());
            }
        }
        if g.can_redo() {
            return Err("UND-1: the redo history has more items than the model".to_string());
        }
    }
    Ok(r)
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 32, max_shrink_iters: 300, ..ProptestConfig::default() })]

    /// UND-1, UND-3, CMP-1, CKP-1 to CKP-5, CKP-8, CKP-9, PER-1 with random saves, Restores, deletes, undo
    /// and redo, in all layouts.
    #[test]
    fn the_checkpoints_and_the_histories_agree_with_the_model(seed in any::<u64>(), acts in proptest::collection::vec(ckp_act_with_new_game(), 1..50)) {
        ckp_sequence(seed, &acts).map_err(TestCaseError::fail)?;
    }
}

#[test]
fn the_checkpoint_sequences_reach_each_rule() {
    // The sequences must reach each state that their checks examine: else they prove nothing.
    use proptest::strategy::ValueTree;
    use proptest::test_runner::TestRunner;
    let mut runner = TestRunner::deterministic();
    let mut all = CkpReached::default();
    for case in 0..24 {
        let mut acts = proptest::collection::vec(ckp_act(), 40..60).new_tree(&mut runner).unwrap().current();
        // A full list: with the switch off (bit 3 of the seed), each tap gives a different board. The
        // random acts delete too often to fill the list.
        if case & 8 != 0 {
            let fill = (1..=6).flat_map(|digit| [CkpAct::Mark(0, digit), CkpAct::Save(false)]);
            acts.splice(0..0, fill);
        }
        let r = ckp_sequence(case, &acts).unwrap_or_else(|e| panic!("{e}"));
        all.saves += r.saves;
        all.full += r.full;
        all.same_old += r.same_old;
        all.same_new += r.same_new;
        all.restores += r.restores;
        all.current_keeps_redo += r.current_keeps_redo;
        all.restore_clears_redo += r.restore_clears_redo;
        all.deletes += r.deletes;
        all.guarded += r.guarded;
        all.undo_of_restore += r.undo_of_restore;
        all.redo_of_restore += r.redo_of_restore;
        all.restore_item_without_checkpoint += r.restore_item_without_checkpoint;
        all.completed_after_restore += r.completed_after_restore;
        all.acts_on_complete += r.acts_on_complete;
        all.full_board_not_solved += r.full_board_not_solved;
    }
    println!(
        "saves {}, full {}, same old {}, same new {}, restores {}, current {}, clears redo {}, deletes {}, guarded {}, undo {}, redo {}, no checkpoint {}, completed {}, on complete {}, full {}",
        all.saves, all.full, all.same_old, all.same_new, all.restores, all.current_keeps_redo, all.restore_clears_redo, all.deletes, all.guarded, all.undo_of_restore, all.redo_of_restore, all.restore_item_without_checkpoint, all.completed_after_restore, all.acts_on_complete, all.full_board_not_solved
    );
    assert!(all.saves > 0, "CKP-1: no save");
    assert!(all.full > 0, "CKP-1, CKP-4: no tap with a full list");
    assert!(all.same_old > 0, "CKP-1, CKP-4: no tap with the board of an older checkpoint");
    assert!(all.same_new > 0, "CKP-1, CKP-4: no tap with the board of the newest checkpoint");
    assert!(all.restores > 0, "CKP-2: no Restore");
    assert!(all.current_keeps_redo > 0, "CKP-2: no tap on \"Current\" with a redo history");
    assert!(all.restore_clears_redo > 0, "UND-1: no Restore with a redo history");
    assert!(all.deletes > 0, "CKP-3: no delete");
    assert!(all.guarded > 0, "CKP-8: no second tap with a different row at that position");
    assert!(all.undo_of_restore > 0, "UND-3: no undo of a Restore");
    assert!(all.redo_of_restore > 0, "UND-3: no redo of a Restore");
    assert!(all.restore_item_without_checkpoint > 0, "CKP-3: no redo of a Restore after the delete of its checkpoint");
    assert!(all.completed_after_restore > 0, "CMP-1: no completion after a Restore");
    assert!(all.acts_on_complete > 0, "CMP-3, CKP-9: no act on a complete game");
    assert!(all.full_board_not_solved > 0, "CMP-1: no full board with a clash");
}
