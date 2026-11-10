//! Checks of the rules in INVARIANTS.md that apply to one state. The tests and the debug builds use
//! these checks after each input. The app also checks a saved game before it uses it (PER-5).
//!
//! Rules about a sequence of states (for example CLK-2 or UND-3) are in the tests.

use crate::app::{average_seconds, App, Comparison, Menu, Panel, Screen, HAND_TOUCH_MS, MANUAL_PAGES, RECOGNITION_PAUSE_MS, WRITING_IDLE_MS};
use crate::game::{cells, mark_bit, Board, Change, Game, Group, MAX_CHECKPOINTS};
use crate::generator::{count_solutions, is_valid_solution, partner, MODES, V1_MODES, VERSION};
use crate::layout::{self, LevelButton, Rect};
use crate::settings::Color;
use crate::update::{has_updates, is_update, PatchChange, UpToDateRow, UpdateState};

// The fixed times of the writing rules. A wrong value does not compile.
const _: () = assert!(RECOGNITION_PAUSE_MS == 1000, "INP-23");
const _: () = assert!(WRITING_IDLE_MS == RECOGNITION_PAUSE_MS, "INP-16");
const _: () = assert!(HAND_TOUCH_MS == 150, "INP-20");
// NAV-19, QST-5: the secondary text and marks are gray25 (#404040).
const _: () = assert!(crate::render::SECONDARY[0] == 0x40 && crate::render::SECONDARY[1] == 0x40 && crate::render::SECONDARY[2] == 0x40, "NAV-19");
// MRK-19, NAV-19: the pencil marks and the x are gray15 (#262626), Regular 31 px. INP-25: the ink is
// 5 px wide, and the dot of a tap has a radius of 2.5 px. The crisp pixels of these drawings and of
// all other drawings (NAV-20) are pixel rules: the tests examine them.
const _: () = assert!(crate::render::MARK_COLOR[0] == 0x26 && crate::render::MARK_COLOR[1] == 0x26 && crate::render::MARK_COLOR[2] == 0x26, "MRK-19");
// The contents of a cell come from the cell of the device (`layout::Device`). On the Paper Pro they are
// the values of the rules: marks of 31 px (MRK-19), at 24, 74 and 122 px (MRK-1, MRK-13), the x from
// -10 to +10 px (MRK-13), and the numbers of 104 px with the baseline at 111 px (the images).
const _: () = {
    let d = crate::layout::Device::PAPER_PRO;
    assert!(d.mark_size() == 31.0, "MRK-19");
    assert!(d.mark_pos(0) == 24.0 && d.mark_pos(1) == 74.0 && d.mark_pos(2) == 122.0 && d.mark_baseline() == 11.0, "MRK-13");
    assert!(d.refused_x_half() == 10.0, "MRK-13");
    assert!(d.digit_size() == 104.0 && d.digit_baseline() == 111.0, "BRD-8");
};
const _: () = assert!(crate::render::INK_WIDTH == 5.0 && crate::render::INK_DOT_RADIUS == 2.5, "INP-25");
// BRD-9: the dots of the micro-cell lines are gray15 (not black: the mono waveform), the pattern is
// the 4x4 Bayer matrix, and each 2x4 px run of a line has the quantity of dots of its gray (the same
// in the two directions: both sample the columns 0 and 1 of the matrix). BRD-10, BRD-11 and COL-13
// are pixel rules: the tests examine them.
const _: () = assert!(crate::render::LINE_DOT[0] == 0x26 && crate::render::LINE_DOT[1] == 0x26 && crate::render::LINE_DOT[2] == 0x26, "BRD-9");
const _: () = {
    let m = crate::render::BAYER;
    assert!(m[0][0] == 0 && m[0][1] == 8 && m[0][2] == 2 && m[0][3] == 10, "BRD-9");
    assert!(m[1][0] == 12 && m[1][1] == 4 && m[1][2] == 14 && m[1][3] == 6, "BRD-9");
    assert!(m[2][0] == 3 && m[2][1] == 11 && m[2][2] == 1 && m[2][3] == 9, "BRD-9");
    assert!(m[3][0] == 15 && m[3][1] == 7 && m[3][2] == 13 && m[3][3] == 5, "BRD-9");
};
const _: () = {
    // gray90, gray80, gray70, gray60, gray55, gray50, gray45, gray40 (the order of `LineGray::ALL`
    // after None) and the dots in each 2x4 px run.
    let percents = [90, 80, 70, 60, 55, 50, 45, 40];
    let dots = [1, 1, 3, 3, 3, 4, 5, 5];
    let mut i = 0;
    while i < 8 {
        assert!(pattern_dots_per_run(crate::render::gray(percents[i])[0]) == dots[i], "BRD-9");
        i += 1;
    }
};

/// BRD-9: the quantity of gray15 pixels in a 2x4 px run of a micro-cell line of the gray `gray`
/// (2 px across the line, 4 px along it), the same for the two directions.
const fn pattern_dots_per_run(gray: u8) -> u32 {
    let mut n = 0;
    let mut along = 0;
    while along < 4 {
        let mut across = 0;
        while across < 2 {
            if crate::render::pattern_dot(gray, along, across) {
                n += 1;
            }
            across += 1;
        }
        along += 1;
    }
    n
}

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
    ensure!(!g.symmetric || g.generator == VERSION, "PUZ-8", "a symmetric game of generator {}", g.generator);
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
    if g.symmetric {
        // PUZ-8: a half turn of the board does not change the starting cells.
        for cell @ (r, c) in cells() {
            let (i, j) = partner(cell);
            ensure!(g.given[r as usize][c as usize] == g.given[i as usize][j as usize], "PUZ-8", "({r},{c}) and ({i},{j}) of a symmetric game");
        }
        // PUZ-9: a symmetric Easy puzzle has 44 or 45 starting numbers.
        let n = g.givens_count();
        ensure!(g.mode != 0 || (44..=45).contains(&n), "PUZ-9", "a symmetric Easy game has {n} starting numbers");
    }

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

    check_completed_groups(g)?;
    check_checkpoints(g)?;

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

/// CKP-5: a board of a checkpoint or of a Restore item is possible in this game. The drawing and undo
/// can use the values of a saved file only after this check (PER-5).
fn check_saved_board(g: &Game, b: &Board) -> Result<(), String> {
    for (r, c) in cells() {
        let (ri, ci) = (r as usize, c as usize);
        let (v, marks) = (b.value[ri][ci], b.marks[ri][ci]);
        if v > 9 {
            return Err(format!("value {v} at ({r},{c})"));
        }
        if marks & !crate::game::ALL_MARKS != 0 {
            return Err(format!("marks out of 1..9 at ({r},{c})"));
        }
        if g.given[ri][ci] && (v != g.solution[ri][ci] || marks != 0) {
            return Err(format!("the starting number at ({r},{c}) is not the solution or has marks"));
        }
    }
    if b.is_solved() {
        return Err("the board is solved".to_string());
    }
    Ok(())
}

/// CKP-4, CKP-5: the list of checkpoints. A checkpoint on a complete game is correct.
fn check_checkpoints(g: &Game) -> Result<(), String> {
    let list = &g.checkpoints;
    ensure!(list.len() <= MAX_CHECKPOINTS, "CKP-4", "{} checkpoints", list.len());
    for (i, k) in list.iter().enumerate() {
        check_saved_board(g, &k.board).map_err(|e| format!("CKP-5: checkpoint {i}: {e}"))?;
        ensure!(list[..i].iter().all(|newer| newer.board != k.board), "CKP-4", "checkpoint {i} has the board of a newer checkpoint");
        let limit = if i == 0 { g.elapsed_ms } else { list[i - 1].elapsed_ms };
        ensure!(k.elapsed_ms <= limit, "CKP-4", "the clock of checkpoint {i} is more than {limit}");
    }
    Ok(())
}

/// COL-8, COL-9: the completed groups, examined group by group, are the set that the drawing uses.
fn check_completed_groups(g: &Game) -> Result<(), String> {
    let set = g.completed_cells();
    let mut in_group = crate::game::CellSet::default();
    for group in Group::all() {
        let cells = group.cells();
        let full_no_clash = cells.iter().all(|&c| g.is_filled(c) && !g.is_clashing(c));
        ensure!(g.is_completed_group(group) == full_no_clash, "COL-8", "{group:?}");
        if full_no_clash {
            let digits = cells.iter().fold(0, |m, &c| m | mark_bit(g.value_at(c)));
            ensure!(digits == crate::game::ALL_MARKS, "COL-9", "{group:?} does not have the digits 1 to 9");
            cells.into_iter().for_each(|c| in_group.insert(c));
        }
    }
    ensure!(set == in_group, "COL-8", "the cells of the completed groups are not the cells of each group");
    for c in cells() {
        ensure!(g.is_filled(c) || !set.contains(c), "COL-8", "the empty cell {c:?} is in a completed group");
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
        Change::Restore { .. } => true,
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
        // CKP-5
        Change::Restore { before, after } => {
            check_saved_board(g, before).map_err(|e| format!("CKP-5: the board before: {e}"))?;
            check_saved_board(g, after).map_err(|e| format!("CKP-5: the board after: {e}"))?;
            if !g.same_board(before) {
                return Err("CKP-5: the board before a Restore is not the board at that point of the history".to_string());
            }
            if before == after {
                return Err("CKP-5: the two boards of a Restore are the same".to_string());
            }
        }
    }
    Ok(())
}

/// COL-11, COL-12: the completed color shows only with the switch on and a color on a game in progress.
fn check_completed_color(app: &App) -> Result<(), String> {
    let (s, g) = (&app.settings, &app.game);
    if let Some(color) = s.completed_fill(g) {
        ensure!(s.completed_color_on, "COL-11", "the completed color shows with the switch off");
        ensure!(color == s.color_completed && color != Color::White, "COL-11", "the completed color is {color:?}");
        ensure!(!g.completed, "COL-12", "the completed color shows on a complete game");
    } else {
        ensure!(
            !s.completed_color_on || s.color_completed == Color::White || g.completed,
            "COL-11",
            "no completed color with the switch on, the color {:?} and a game in progress",
            s.color_completed
        );
    }
    Ok(())
}

/// NAV-14, NAV-17: the positions of the open panel for the current layout.
fn check_panel_layout(app: &App) -> Result<(), String> {
    match app.ui.menu.panel() {
        Some(Panel::Colors) => {
            let p = layout::colors_panel();
            let x0 = match (layout::right_handed(), layout::orientation()) {
                (true, _) => 112,
                (false, layout::Orientation::Portrait) => 708,
                (false, layout::Orientation::Landscape) => 1248,
            };
            ensure!((p.x0, p.x1, p.y0, p.y1) == (x0, x0 + 800, 112, 1193), "NAV-17", "the Cell colors panel is at {p:?}");
        }
        Some(Panel::Settings) => {
            let p = layout::settings_panel(app.update.has_row());
            ensure!(p.x1 - p.x0 == 400, "NAV-14", "the Settings panel is at {p:?}");
        }
        Some(Panel::Checkpoints) => {
            let x0 = match (layout::right_handed(), layout::orientation()) {
                (true, _) => 112,
                (false, layout::Orientation::Portrait) => 908,
                (false, layout::Orientation::Landscape) => 1448,
            };
            let n = app.game.checkpoints.len();
            let bottom = if n == 0 { 864 } else { 672 + 180 * n as i32 };
            let p = layout::ckp_panel(n);
            ensure!((p.x0, p.x1, p.y0, p.y1) == (x0, x0 + 600, 560, bottom + 1), "NAV-18", "the Checkpoints panel is at {p:?}");
            for corner in [(p.x0, p.y0), (p.x1 - 1, p.y0), (p.x0, p.y1 - 1), (p.x1 - 1, p.y1 - 1)] {
                ensure!(layout::game_button(corner.0 as f32, corner.1 as f32, true).is_none(), "NAV-18", "the panel covers a button at {corner:?}");
            }
            ensure!(p.y1 + 13 < layout::meter_y() || p.x1 <= layout::meter_xl() || p.x0 >= layout::meter_xr(), "NAV-18", "the panel covers the meter");
        }
        _ => {}
    }
    Ok(())
}

/// CMP-7: the frame of the result card in the current layout. With the menu closed it touches no
/// toolbar button, not the close button, not the menu button and not the board. The toolbar column
/// (all its height) and the card never touch, thus in landscape they are at opposite sides also when
/// the menu is open.
fn check_card_layout() -> Result<(), String> {
    let r = layout::result_card();
    let expected = match (layout::orientation(), layout::right_handed()) {
        (layout::Orientation::Portrait, _) => Rect::new(148, 1790, 1471, 2110),
        (layout::Orientation::Landscape, false) => Rect::new(48, 148, 370, 1471),
        (layout::Orientation::Landscape, true) => Rect::new(1790, 148, 2112, 1471),
    };
    ensure!(r == expected, "CMP-7", "the result card is at {r:?}");
    let (w, h) = (layout::screen_w() as i32, layout::screen_h() as i32);
    let toolbar = Rect::new(layout::tb_x(), 0, layout::tb_x() + layout::TB_SLOT, h);
    let close_x = if layout::right_handed() { w - layout::TB_SLOT } else { 0 };
    let close = Rect::new(close_x, 0, close_x + layout::TB_SLOT, layout::TB_SLOT);
    let board = Rect::new(layout::board_x(), layout::board_y(), layout::board_x() + layout::board_size(), layout::board_y() + layout::board_size());
    for (name, o) in [("the toolbar", toolbar), ("the close button", close), ("the board", board)] {
        ensure!(!r.overlaps(&o), "CMP-7", "the result card {r:?} touches {name} {o:?}");
    }
    for b in LevelButton::ALL {
        let br = layout::level_button(b);
        ensure!(br.x0 > r.x0 && br.x1 < r.x1 && br.y0 > r.y0 && br.y1 < r.y1, "CMP-9", "{b:?} {br:?} is not in the card {r:?}");
    }
    Ok(())
}

/// CMP-8, CMP-9, PUZ-7: the result card of a complete game.
fn check_card(app: &App) -> Result<(), String> {
    let g = &app.game;
    if !g.completed {
        return Ok(());
    }
    // CMP-9: Easier is available ⇔ mode > 0, Harder ⇔ mode < 6, Same always. CMP-10: a level button
    // never gives a mode outside 0..6.
    for b in LevelButton::ALL {
        let t = b.target(g.mode);
        let available = match b {
            LevelButton::Easier => g.mode > 0,
            LevelButton::Same => true,
            LevelButton::Harder => g.mode < MODES - 1,
        };
        ensure!(t.is_some() == available, "CMP-9", "{b:?} after mode {} gives {t:?}", g.mode);
        ensure!(t.is_none_or(|m| m < MODES), "CMP-10", "{b:?} gives mode {t:?}");
    }
    // CMP-8: the comparison uses the earlier results at the mode, without the current game.
    let earlier: Vec<_> = app.results.iter().filter(|r| r.mode == g.mode && r.seed != g.seed).collect();
    let expected = match (average_seconds(earlier.iter().copied()), earlier.iter().map(|r| r.seconds).min()) {
        (Some(average), Some(best)) => Comparison::Earlier { average, best },
        _ => Comparison::First,
    };
    let shown = app.card_comparison();
    ensure!(shown == expected, "CMP-8", "the card shows {shown:?}, the results give {expected:?}");
    if let Comparison::Earlier { average, best } = shown {
        ensure!(best <= average, "CMP-8", "previous best {best} > average {average}");
    }
    // PUZ-7: the puzzle in the background is of the next seed, and on a complete game with no New
    // game panel it is of the mode of Same and of the symmetry of the game. (The start at the
    // completion is a rule about a sequence of states: the tests examine it.)
    if let Some((seed, mode, symmetric)) = app.early_game {
        ensure!(app.next_seed == Some(seed), "PUZ-7", "early puzzle of seed {seed}, next seed {:?}", app.next_seed);
        ensure!(mode < MODES, "PUZ-7", "early puzzle of mode {mode}");
        if app.ui.menu.panel() != Some(Panel::NewGame) {
            ensure!(mode == g.mode, "PUZ-7", "early puzzle of mode {mode} on a complete game of mode {}", g.mode);
            ensure!(symmetric == g.symmetric, "PUZ-7", "early puzzle of symmetry {symmetric} on a complete game of symmetry {}", g.symmetric);
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
        // A highlight can show with the menu, a panel and waiting ink (HLT-4, HLT-9).
        ensure!(app.screen == Screen::Game, "HLT-4", "highlight on screen {:?}", app.screen);
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
    // INP-22: no ink and no pen area on a filled cell. The stroke that can start the area is on an empty
    // cell, and a touch that started on a filled cell has no live stroke.
    if app.pen_on_filled_cell() {
        ensure!(app.live_stroke.is_none(), "INP-22", "ink of a pen touch that started on a filled cell");
    }
    if let Some(cell) = app.writing_stroke().and_then(|s| s.first()).and_then(|&(x, y)| layout::board_cell(x, y)) {
        ensure!(!g.is_filled(cell), "INP-22", "the pen area of a stroke on the filled cell {cell:?}");
    }
    if app.writing_until.is_some() {
        ensure!(!g.completed && app.screen == Screen::Game, "INP-16", "writing timer on a complete game or a different screen");
    }
    if g.completed {
        ensure!(ui.highlight.is_none() && ui.question.is_none() && ui.refused.is_none() && ui.pending_ink.is_none(), "CMP-3", "overlay on a complete game");
        ensure!(ui.menu.panel() != Some(Panel::Checkpoints), "CKP-9", "the Checkpoints panel is open on a complete game");
    }
    if ui.menu.panel() == Some(Panel::Checkpoints) {
        ensure!(ui.pending_ink.is_none() && app.live_stroke.is_none(), "NAV-18", "ink in the Checkpoints panel");
    } else {
        ensure!(ui.delete_guard_until.is_none(), "CKP-8", "the guard after a delete with panel {:?}", ui.menu.panel());
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
        ensure!(ui.symmetric_sel == app.settings.last_symmetric, "DIF-8", "the Symmetrical switch keeps a selection after the panel closed");
    }
    ensure!(app.settings.is_valid(), "COL-5", "settings {:?}", app.settings);
    check_completed_color(app)?;
    check_panel_layout(app)?;
    if g.completed && app.screen == Screen::Game {
        check_card_layout()?;
    }
    check_card(app)?;
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

    // The Licenses screens (LIC-2, LIC-3): the pages and the open entry are in their ranges.
    let lic = &app.licenses;
    let pages = app.licenses_page_count();
    ensure!(lic.page >= 1 && lic.page <= pages, "LIC-2", "licenses page {} of {}", lic.page, pages);
    ensure!(lic.open < crate::licenses::count(), "LIC-3", "open entry {} of {}", lic.open, crate::licenses::count());
    if app.screen == Screen::LicenseText {
        let pages = app.license_text_pages();
        ensure!(lic.text_page >= 1 && lic.text_page <= pages, "LIC-3", "text page {} of {}", lic.text_page, pages);
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
