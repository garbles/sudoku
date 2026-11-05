//! Checks of the rules in INVARIANTS.md that apply to one state. The tests and the debug builds use
//! these checks after each input. The app also checks a saved game before it uses it (PER-5).
//!
//! Rules about a sequence of states (for example CLK-2 or UND-3) are in the tests.

use crate::app::{App, Menu, Panel, Screen, MANUAL_PAGES};
use crate::game::{cells, mark_bit, Change, Game};
use crate::generator::{count_solutions, is_valid_solution, MODES, V1_MODES, VERSION};
use crate::update::{has_updates, is_update, PatchChange, UpToDateRow, UpdateState};

fn fail(rule: &str, msg: impl std::fmt::Display) -> Result<(), String> {
    Err(format!("{rule}: {msg}"))
}

macro_rules! ensure {
    ($cond:expr, $rule:expr, $($msg:tt)+) => {
        if !$cond {
            return fail($rule, format!($($msg)+));
        }
    };
}

/// The rules for the game only (§1 to §4, §7 and §8 of INVARIANTS.md).
pub fn check_game(g: &Game) -> Result<(), String> {
    ensure!(g.mode < MODES, "DIF-1", "mode {} is not in 0..6", g.mode);
    ensure!(g.generator == VERSION || (g.generator == 1 && g.mode < V1_MODES), "PUZ-5", "generator {} mode {}", g.generator, g.mode);
    ensure!(is_valid_solution(&g.solution), "PUZ-1", "the solution is not a correct sudoku");

    let mut givens = [[0u8; 9]; 9];
    for (r, c) in cells() {
        let (ri, ci) = (r as usize, c as usize);
        let v = g.value[ri][ci];
        ensure!(v <= 9, "BRD-1", "value {v} at ({r},{c})");
        ensure!(g.marks[ri][ci] & !crate::game::ALL_MARKS == 0, "MRK-1", "marks out of 1..9 at ({r},{c})");
        if g.given[ri][ci] {
            ensure!(v == g.solution[ri][ci], "PUZ-3", "starting number at ({r},{c}) is not the solution");
            ensure!(g.marks[ri][ci] == 0, "MRK-3", "starting number at ({r},{c}) has marks");
            givens[ri][ci] = v;
        }
    }
    ensure!(count_solutions(&givens, 2) == 1, "PUZ-2", "the starting numbers do not have only one solution");

    let solved = g.is_solved_board();
    ensure!(g.completed == solved, "CMP-1", "completed = {} but solved board = {}", g.completed, solved);
    if g.completed {
        ensure!(g.value == g.solution, "CMP-2", "a complete board is not the solution");
    }
    for (r, c) in cells() {
        if g.is_given((r, c)) && g.is_clashing((r, c)) {
            ensure!(
                crate::game::peers((r, c)).any(|p| g.is_entry(p) && g.value_at(p) == g.value_at((r, c))),
                "CLS-3",
                "two starting numbers clash at ({r},{c})"
            );
        }
    }

    // UND-1, UND-3: the board is the result of the changes in the undo history, applied in order to
    // the puzzle. The redo history applies to the current board in the opposite order.
    let mut replay = Game { generator: g.generator, ..Game::new(g.seed, g.mode, g.solution, givens) };
    for (i, change) in g.undo.iter().enumerate() {
        check_change_applies(&replay, change).map_err(|e| format!("UND-1: undo item {i}: {e}"))?;
        replay.apply(change);
    }
    ensure!(replay.value == g.value, "UND-3", "the undo history does not give the board");
    ensure!(replay.marks == g.marks, "UND-3", "the undo history does not give the marks");
    for (i, change) in g.redo.iter().rev().enumerate() {
        check_change_applies(&replay, change).map_err(|e| format!("UND-3: redo item {i}: {e}"))?;
        replay.apply(change);
    }
    Ok(())
}

/// A change agrees with the board before the change.
fn check_change_applies(g: &Game, change: &Change) -> Result<(), String> {
    // A saved file can have any cell. Examine the range before the board is read (PER-5).
    let in_range = |&(r, c): &crate::game::Cell| r < 9 && c < 9;
    let cells_ok = match change {
        Change::ToggleMark { cell, .. } | Change::Erase { cell, .. } => in_range(cell),
        Change::AddNumber { cell, removed, .. } => in_range(cell) && removed.iter().all(in_range),
    };
    if !cells_ok {
        return Err(format!("a cell is not on the board: {change:?}"));
    }
    match change {
        Change::ToggleMark { cell, digit } => {
            if g.is_filled(*cell) || !(1..=9).contains(digit) {
                return Err(format!("mark {digit} on a filled cell {cell:?}"));
            }
        }
        Change::AddNumber { cell, digit, removed } => {
            if g.is_filled(*cell) || !(1..=9).contains(digit) {
                return Err(format!("number {digit} on a filled cell {cell:?}"));
            }
            let expected: Vec<_> =
                crate::game::peers(*cell).filter(|&p| g.marks_at(p) & crate::game::mark_bit(*digit) != 0).collect();
            // A new number has no removed list. A number of an earlier version removed the mark of all
            // peers that had it (the exception of MRK-5, UND-4).
            if *removed != expected && !removed.is_empty() {
                return Err(format!("removed marks {removed:?} are not {expected:?} (MRK-5, UND-4)"));
            }
        }
        Change::Erase { cell, digit } => {
            if !g.is_entry(*cell) || g.value_at(*cell) != *digit {
                return Err(format!("erase of {digit} at {cell:?}, which is not that entry"));
            }
        }
    }
    Ok(())
}

/// All rules that apply to one state of the app.
pub fn check_app(app: &App) -> Result<(), String> {
    check_game(&app.game)?;
    let ui = &app.ui;
    let g = &app.game;

    if let Some(h) = ui.highlight {
        ensure!((1..=9).contains(&h), "HLT-2", "highlight {h}");
        ensure!(!g.completed, "HLT-1", "highlight on a complete game");
        ensure!(app.screen == Screen::Game && ui.menu.panel().is_none(), "HLT-4", "highlight with a panel or a different screen");
    }
    if let Some((cell, _)) = ui.question {
        ensure!(!g.is_filled(cell), "QST-1", "the ? is on a filled cell");
        ensure!(!g.completed, "QST-3", "the ? on a complete game");
    }
    // The time and the removal of the x (MRK-14, MRK-15) are rules about a sequence of states, thus
    // they are in the tests. MRK-17 and MRK-18: `refused` is not in the saved game, and this check
    // uses only the numbers of the board, not the solution.
    if let Some((cell, d, _)) = ui.refused {
        ensure!((1..=9).contains(&d), "MRK-12", "x with digit {d}");
        ensure!(app.screen == Screen::Game, "MRK-12", "x on screen {:?}", app.screen);
        ensure!(ui.menu.panel().is_none(), "MRK-12", "x with panel {:?}", ui.menu.panel());
        ensure!(!g.completed, "MRK-12", "x on a complete game");
        ensure!(!app.update.dialog_shown(), "MRK-12", "x under the update dialog");
        ensure!(!g.is_filled(cell), "MRK-12", "x on the filled cell {cell:?}");
        ensure!(app.settings.auto_hide_on, "MRK-12", "x while automatic hiding is off");
        ensure!(g.peer_digits(cell) & mark_bit(d) != 0, "MRK-12", "x of {d} at {cell:?}, and no peer has the number {d}");
        ensure!(ui.highlight.is_none(), "MRK-12", "x with a highlight");
        ensure!(ui.pending_ink.is_none(), "MRK-12", "x with waiting ink");
        ensure!(ui.question.is_none_or(|(q, _)| q != cell), "MRK-12", "x and ? on the cell {cell:?}");
    }
    if let Some(ink) = &ui.pending_ink {
        ensure!(!g.is_filled(ink.cell), "INP-5", "pending ink on a filled cell");
        ensure!(!g.completed, "INP-5", "pending ink on a complete game");
        ensure!(app.screen == Screen::Game, "INP-5", "pending ink on a different screen");
    }
    if app.writing_until.is_some() {
        ensure!(!g.completed && app.screen == Screen::Game, "INP-16", "writing timer on a complete game or a different screen");
    }
    if g.completed {
        ensure!(ui.highlight.is_none() && ui.question.is_none() && ui.refused.is_none() && ui.pending_ink.is_none(), "CMP-3", "overlay on a complete game");
    }
    // NAV-4: the type `Menu` keeps a panel only in `Menu::Open`.
    if app.screen != Screen::Game {
        ensure!(ui.menu == Menu::Closed, "NAV-5", "the menu is open on screen {:?}", app.screen);
    }
    // NAV-16: the toolbar ignores a press only on the game screen after "< Back". A press on the
    // board does not change the menu, thus the menu stays open with no panel until the time ends.
    if app.toolbar_ignored_until.is_some() {
        ensure!(app.screen == Screen::Game && ui.menu == Menu::Open(None), "NAV-16", "the toolbar ignores a press with screen {:?} and menu {:?}", app.screen, ui.menu);
    }
    ensure!(ui.slider < MODES && app.settings.last_mode < MODES, "DIF-1", "slider or last mode out of range");
    if ui.menu.panel() != Some(Panel::NewGame) {
        ensure!(ui.slider == app.settings.last_mode, "DIF-4", "the slider keeps a selection after the panel closed");
    }
    ensure!(app.settings.is_valid(), "COL-5", "settings {:?}", app.settings);
    ensure!((1..=MANUAL_PAGES).contains(&app.manual_page), "MAN-1", "manual page {}", app.manual_page);

    // Analytics (ANA-1, ANA-5, ANA-6, ANA-7).
    let mut ids = std::collections::HashSet::new();
    for r in &app.results {
        ensure!(r.mode < MODES && r.givens <= 81, "ANA-1", "result {r:?}");
        ensure!(ids.insert(r.id), "ANA-1", "two results with id {}", r.id);
    }
    for w in app.results.windows(2) {
        ensure!(w[0].finished_at >= w[1].finished_at, "ANA-5", "results not newest first");
    }
    let l = &app.list;
    ensure!(l.filter.map(|m| m < MODES).unwrap_or(true), "ANA-6", "filter {:?}", l.filter);
    ensure!(l.page >= 1 && l.page <= app.page_count(), "ANA-6", "page {} of {}", l.page, app.page_count());
    if app.confirm_all {
        ensure!(app.screen == Screen::Statistics, "ANA-12", "the delete-all dialog on screen {:?}", app.screen);
    }
    if !app.settings.highlight_on {
        ensure!(app.ui.highlight.is_none(), "HLT-6", "a highlight while highlights are off");
    }
    if l.confirm.is_some() {
        ensure!(app.screen == Screen::GamesList, "ANA-7", "the delete dialog on screen {:?}", app.screen);
    }

    // Updates (UPD-1, UPD-2, UPD-7, UPD-8, UPD-9, UPD-10).
    let u = &app.update;
    ensure!((u.state == UpdateState::Off) == !has_updates(&u.current), "UPD-1", "state {:?} for version {}", u.state, u.current);
    if let UpdateState::Available(m) | UpdateState::Installing(m) | UpdateState::Failed(m, _) = &u.state {
        ensure!(is_update(&m.version, &u.current), "UPD-2", "{} is not an update for {}", m.version, u.current);
    }
    // UPD-8 and the state parts of UPD-7 and UPD-9 (which states can have a request in progress or
    // "Sudoku is up to date") are in the type `UpdateState`.
    if u.state == UpdateState::Off {
        ensure!(u.request.is_none(), "UPD-1", "a request in a dev build");
    }
    if u.state == UpdateState::UpToDate(UpToDateRow::UpToDateText) {
        ensure!(app.ui.menu.panel() == Some(Panel::Settings) && app.screen == Screen::Game, "UPD-9", "\"up to date\" kept after the panel closed");
    }
    // UPD-10: the type `PatchChange` keeps the restart dialog in `Installed { patch: true }`.
    if app.restart_ui {
        ensure!(matches!(u.state, UpdateState::Installed(PatchChange::Replaced { .. })), "UPD-10", "a restart in state {:?}", u.state);
    }
    if u.dialog_shown() {
        ensure!(app.ui.pending_ink.is_none() && app.live_stroke.is_none(), "UPD-10", "ink under the restart dialog");
    }
    Ok(())
}
