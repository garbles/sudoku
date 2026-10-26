//! Tests of the rules of INVARIANTS.md about input on the board: BRD-3, BRD-4, MRK-3, MRK-5, INP-4,
//! INP-10 (the state), INP-14, QST-3, NAV-2 and CLK-6 (the 5000 ms limit). Each test puts the app in
//! the state where the rule can break and then examines the rule itself. The release build does not
//! run `invariants::check_app` after each input, thus the tests examine the rules directly.

mod common;

use common::*;
use sudoku::app::{new_game, App, Input, Panel, Screen, Tool, MAX_CLOCK_GAP_MS};
use sudoku::game::{cells, mark_bit, Cell, Change, Game, Marks};
use sudoku::layout::{self, Button, Rect, SWIPE_MIN};
use sudoku::store::{NewResult, ResultsDb};

/// The board state that input can change: values, marks and the lengths of the two histories.
type BoardState = ([[u8; 9]; 9], [[Marks; 9]; 9], usize, usize);

fn state(d: &Driver) -> BoardState {
    let g = &d.app.game;
    (g.value, g.marks, g.undo.len(), g.redo.len())
}

fn empty_cells(d: &Driver) -> Vec<Cell> {
    cells().filter(|&c| !d.app.game.is_filled(c)).collect()
}

fn given_cell(d: &Driver) -> Cell {
    cells().find(|&c| d.app.game.is_given(c)).unwrap()
}

fn solution_at(g: &Game, (r, c): Cell) -> u8 {
    g.solution[r as usize][c as usize]
}

fn center(r: Rect) -> (f32, f32) {
    (((r.x0 + r.x1) / 2) as f32, ((r.y0 + r.y1) / 2) as f32)
}

fn check(d: &Driver) {
    sudoku::invariants::check_app(&d.app).unwrap();
}

/// A pen stroke from one point to a different point, in small steps.
fn pen_stroke(d: &mut Driver, tool: Tool, from: (f32, f32), to: (f32, f32)) {
    d.send(Input::PenDown { x: from.0, y: from.1, tool });
    for k in 1..=10 {
        let t = k as f32 / 10.0;
        d.send(Input::PenMove { x: from.0 + (to.0 - from.0) * t, y: from.1 + (to.1 - from.1) * t });
    }
    d.send(Input::PenUp { x: to.0, y: to.1 });
}

/// A finger movement from one point to a different point.
fn finger_move(d: &mut Driver, from: (f32, f32), to: (f32, f32)) {
    d.send(Input::TouchDown { id: 1, x: from.0, y: from.1 });
    d.send(Input::TouchMove { id: 1, x: (from.0 + to.0) / 2.0, y: (from.1 + to.1) / 2.0 });
    d.send(Input::TouchUp { id: 1, x: to.0, y: to.1 });
}

fn app_with_results(n: usize) -> App {
    let mut db = ResultsDb::in_memory();
    for i in 0..n {
        db.insert(&NewResult { finished_at: 1_700_000_000 + i as i64 * 1000, mode: (i % 5) as u8, givens: 30, seconds: 100 + i as u64, seed: i as u64, generator: 2 })
            .unwrap();
    }
    App::new(None, None, db, Box::new(TestEnv { time: 1_790_000_000, seed: 1 }))
}

/// Input on a cell that must not change the value of the cell (BRD-3, BRD-4) or any marks (MRK-5):
/// finger taps, a finger swipe, a pen stroke from the cell into a different cell, and writing.
fn input_without_effect_on(d: &mut Driver, cell: Cell, other: Cell) {
    for digit in 1..=9 {
        let (x, y) = micro_center(cell, digit);
        d.finger_tap(x, y);
    }
    finger_move(d, micro_center(cell, 4), micro_center(other, 6));
    // A palm on the cell while the pen taps a different place.
    let (x, y) = micro_center(cell, 5);
    d.send(Input::TouchDown { id: 7, x, y });
    d.finger_tap(5.0, 1000.0);
    d.send(Input::TouchUp { id: 7, x, y });
}

// ---- BRD-3

#[test]
fn only_the_eraser_or_undo_removes_an_entry() {
    // BRD-3: the pen, a finger, writing, a scribble and a "?" in a different cell do not remove or
    // replace an entry. Only the eraser and undo remove it.
    let mut d = Driver::new(101);
    let e = empty_cells(&d);
    let (cell, other) = (e[0], e[1]);
    let v = solution_at(&d.app.game, cell);
    d.tap_micro(cell, 2); // A hidden mark under the entry.
    d.write(cell, v);
    assert_eq!(d.app.game.value_at(cell), v);
    let entry_ok = |d: &Driver, why: &str| {
        assert_eq!(d.app.game.value_at(cell), v, "BRD-3: {why}");
        assert!(d.app.game.is_entry(cell));
        check(d);
    };

    for digit in 1..=9 {
        d.tap_micro(cell, digit);
        entry_ok(&d, &format!("pen tap on micro-cell {digit}"));
    }
    input_without_effect_on(&mut d, cell, other);
    entry_ok(&d, "finger");
    d.swipe(true);
    d.swipe(false);
    entry_ok(&d, "finger swipe");
    for w in [1u8, 7, 9] {
        d.write(cell, w);
        entry_ok(&d, &format!("writing {w} on the entry"));
    }
    d.write_strokes(&scribble_ink(cell));
    d.wait(d.app.pause_ms + 50);
    entry_ok(&d, "scribble on the entry");
    assert_eq!(d.app.ui.question, None, "no \"?\" on a filled cell");
    // Writing that starts in a different cell and goes across the entry belongs to the other cell.
    pen_stroke(&mut d, Tool::Pen, micro_center(other, 5), micro_center(cell, 5));
    d.wait(d.app.pause_ms + 50);
    entry_ok(&d, "a stroke from a different cell");
    // A "?" in a different cell, then the "?" goes away.
    let third = empty_cells(&d)[0];
    d.write_strokes(&scribble_ink(third));
    d.wait(d.app.pause_ms + 50);
    d.wait(1100);
    entry_ok(&d, "a \"?\" in a different cell");
    // The same input with the menu and each panel open.
    d.button(Button::Menu);
    for panel in [Button::Colors, Button::Settings, Button::NewGame] {
        d.button(panel);
        d.tap_micro(cell, 1);
        d.write(cell, 4);
        entry_ok(&d, &format!("pen on the entry with the panel of {panel:?}"));
    }
    // Undo and redo of a different change do not remove the entry.
    d.tap_micro(other, 3);
    d.button(Button::Undo);
    d.button(Button::Redo);
    entry_ok(&d, "undo and redo of a mark in a different cell");

    // The eraser removes the entry, undo puts it back.
    d.erase(cell);
    assert_eq!(d.app.game.value_at(cell), 0, "the eraser removes the entry");
    d.button(Button::Undo);
    entry_ok(&d, "undo of the erasure");
    // Undo of the number removes it.
    while d.app.game.value_at(cell) != 0 {
        assert!(d.app.game.can_undo(), "undo must remove the entry");
        d.button(Button::Undo);
    }
    check(&d);
}

// ---- BRD-4

#[test]
fn an_empty_cell_gets_a_value_only_from_reading_redo_or_undo_of_an_erasure() {
    // BRD-4: taps, a finger, a swipe, the eraser and a scribble do not put a value in an empty cell.
    // Only a number that the app reads, redo, and undo of an erasure do.
    let mut d = Driver::new(102);
    let e = empty_cells(&d);
    let (cell, other) = (e[0], e[1]);
    let v = solution_at(&d.app.game, cell);
    let still_empty = |d: &Driver, why: &str| {
        assert_eq!(d.app.game.value_at(cell), 0, "BRD-4: {why}");
        check(d);
    };

    for digit in 1..=9 {
        d.tap_micro(cell, digit);
        still_empty(&d, &format!("pen tap on micro-cell {digit}"));
    }
    input_without_effect_on(&mut d, cell, other);
    still_empty(&d, "finger");
    d.swipe(true);
    still_empty(&d, "finger swipe");
    d.erase(cell);
    pen_stroke(&mut d, Tool::Eraser, micro_center(other, 1), micro_center(cell, 9));
    still_empty(&d, "eraser");
    d.write_strokes(&scribble_ink(cell));
    d.wait(d.app.pause_ms + 50);
    assert_eq!(d.app.ui.question.map(|q| q.0), Some(cell));
    still_empty(&d, "scribble (\"?\")");
    d.wait(1100);
    still_empty(&d, "after the \"?\"");
    // A stroke that starts in a filled cell and goes into the empty cell is not writing in it.
    let given = given_cell(&d);
    pen_stroke(&mut d, Tool::Pen, micro_center(given, 5), micro_center(cell, 5));
    d.wait(d.app.pause_ms + 50);
    still_empty(&d, "a stroke from a filled cell");
    // Ink that waits in the cell while a finger is on the board: the finger does not read it.
    d.write_strokes(&digit_ink(cell, v));
    let (x, y) = micro_center(other, 5);
    d.finger_tap(x, y);
    assert_eq!(d.app.game.value_at(cell), 0, "BRD-4: the ink waits for the pause");
    d.wait(d.app.pause_ms + 50);
    assert_eq!(d.app.game.value_at(cell), v, "the app reads the number");

    // Undo removes the number, redo puts it back.
    d.button(Button::Menu);
    d.button(Button::Undo);
    still_empty(&d, "undo of the number");
    d.button(Button::Redo);
    assert_eq!(d.app.game.value_at(cell), v, "redo");
    // The eraser removes it, undo of the erasure puts it back.
    d.erase(cell);
    still_empty(&d, "erasure");
    d.button(Button::Undo);
    assert_eq!(d.app.game.value_at(cell), v, "undo of the erasure");
    check(&d);
}

// ---- MRK-3

#[test]
fn starting_numbers_never_get_marks() {
    // MRK-3: pen taps on all micro-cells of a starting number, with the menu closed, with the menu
    // open, with each panel open, and with undo and redo, give no marks.
    let mut d = Driver::new(103);
    let given = given_cell(&d);
    let no_marks = |d: &Driver, why: &str| {
        for c in cells().filter(|&c| d.app.game.is_given(c)) {
            assert_eq!(d.app.game.marks_at(c), 0, "MRK-3: {why} at {c:?}");
        }
        check(d);
    };
    for digit in 1..=9 {
        d.tap_micro(given, digit);
    }
    no_marks(&d, "pen taps, menu closed");
    d.button(Button::Menu);
    for digit in 1..=9 {
        d.tap_micro(given, digit);
    }
    no_marks(&d, "pen taps, menu open");
    for panel in [Button::Colors, Button::Settings, Button::NewGame] {
        d.button(panel);
        d.tap_micro(given, 1);
        d.tap_micro(given, 9);
        no_marks(&d, &format!("pen taps with the panel of {panel:?}"));
    }
    let cell = empty_cells(&d)[0];
    d.tap_micro(cell, 4);
    d.tap_micro(given, 4);
    d.button(Button::Undo);
    d.button(Button::Redo);
    d.button(Button::Undo);
    for digit in 1..=9 {
        d.tap_micro(given, digit);
    }
    d.button(Button::Redo);
    no_marks(&d, "undo and redo");
    d.erase(given);
    d.write(given, 5);
    d.write_strokes(&scribble_ink(given));
    d.wait(d.app.pause_ms + 50);
    no_marks(&d, "eraser and writing");
    for c in cells().filter(|&c| d.app.game.is_given(c)) {
        assert!(d.app.game.is_filled(c), "MRK-3: a starting number is never empty");
    }

    // A saved game with a mark on a starting number is not used (PER-5).
    let mut g = new_game(77, 2);
    let gc = cells().find(|&c| g.is_given(c)).unwrap();
    g.marks[gc.0 as usize][gc.1 as usize] = mark_bit(3);
    let app = App::new(Some(g.clone()), None, ResultsDb::in_memory(), Box::new(TestEnv { time: 1_790_000_000, seed: 5 }));
    assert_ne!(app.game, g, "MRK-3: the app does not use a saved game with marks on a starting number");
    assert!(cells().filter(|&c| app.game.is_given(c)).all(|c| app.game.marks_at(c) == 0));
}

// ---- MRK-5

#[test]
fn the_eraser_and_other_input_do_not_change_marks() {
    // MRK-5: the eraser over an empty cell with marks and over an entry with hidden marks does not
    // change the marks. A finger, a swipe, a scribble and writing on a filled cell do not change them.
    let mut d = Driver::new(104);
    let e = empty_cells(&d);
    let (a, b, other) = (e[0], e[1], e[2]);
    d.tap_micro(a, 1);
    d.tap_micro(a, 8);
    d.tap_micro(b, 2);
    d.tap_micro(b, 5);
    let w = (1..=9).find(|&w| w != 2 && w != 5 && w != 1 && w != 8).unwrap();
    d.write(b, w);
    assert_eq!(d.app.game.value_at(b), w);
    let marks = d.app.game.marks;
    let same = |d: &Driver, why: &str| {
        assert_eq!(d.app.game.marks, marks, "MRK-5: {why}");
        check(d);
    };

    d.erase(a);
    pen_stroke(&mut d, Tool::Eraser, micro_center(a, 1), micro_center(a, 9));
    same(&d, "eraser on an empty cell with marks");
    d.erase(given_cell(&d));
    same(&d, "eraser on a starting number");
    input_without_effect_on(&mut d, a, other);
    input_without_effect_on(&mut d, b, other);
    d.swipe(true);
    same(&d, "finger and swipe");
    d.write(b, 3);
    d.write_strokes(&scribble_ink(b));
    d.wait(d.app.pause_ms + 50);
    same(&d, "writing on a filled cell");
    d.write_strokes(&scribble_ink(a));
    d.wait(d.app.pause_ms + 50);
    assert_eq!(d.app.ui.question.map(|q| q.0), Some(a));
    same(&d, "\"?\" on a cell with marks");
    d.wait(1100);
    same(&d, "after the \"?\"");
    // A pen tap on a cell with marks while a panel is open only closes the panel.
    d.button(Button::Menu);
    d.button(Button::Colors);
    d.tap_micro(a, 1);
    assert_eq!(d.app.ui.panel, Panel::None);
    same(&d, "pen tap that closes a panel");
    // The eraser over the entry with hidden marks: the marks show again and do not change.
    d.erase(b);
    assert_eq!(d.app.game.value_at(b), 0);
    same(&d, "eraser on an entry with hidden marks");
    // An eraser stroke across many cells.
    let (r0, r1) = (layout::board_x() as f32 + 5.0, (layout::board_x() + layout::BOARD_SIZE) as f32 - 5.0);
    let y = micro_center(a, 5).1;
    pen_stroke(&mut d, Tool::Eraser, (r0, y), (r1, y));
    same(&d, "eraser stroke across a row");
    // Undo and redo of the erasure do not change the marks.
    d.button(Button::Undo);
    d.button(Button::Redo);
    same(&d, "undo and redo of the erasure");
}

// ---- INP-4

#[test]
fn a_tap_on_an_entry_changes_only_the_highlight() {
    // INP-4: a tap on each micro-cell of an entry (with hidden marks, and with a redo history) does
    // not change the values, the marks or the histories. Only the highlight changes.
    let mut d = Driver::new(105);
    let e = empty_cells(&d);
    let (cell, other) = (e[0], e[1]);
    let v = solution_at(&d.app.game, cell);
    d.tap_micro(cell, 3);
    d.write(cell, v);
    d.button(Button::Menu);
    d.tap_micro(other, 6);
    d.button(Button::Undo);
    assert_eq!(d.app.game.redo.len(), 1);
    let before = state(&d);
    for digit in 1..=9 {
        let h = d.app.ui.highlight;
        d.tap_micro(cell, digit);
        assert_eq!(state(&d), before, "INP-4: tap on micro-cell {digit}");
        // HLT-3: the same digit removes the highlight, else the highlight is the digit of the cell.
        let expected = if h == Some(v) { None } else { Some(v) };
        assert_eq!(d.app.ui.highlight, expected);
        check(&d);
    }
    // The redo history is still there.
    d.button(Button::Redo);
    assert_eq!(d.app.game.marks_at(other), mark_bit(6));
}

// ---- INP-10 (the state; the drawing is in another test file)

#[test]
fn no_ink_stays_after_the_app_reads_or_removes_it() {
    // INP-10: after a number, after a "?", after writing on a filled cell, after a press in a different
    // cell, and after a finger press on a control while the pen writes, no ink waits and no live
    // stroke stays.
    let mut d = Driver::new(106);
    let no_ink = |d: &Driver, why: &str| {
        assert!(d.app.ui.pending_ink.is_none(), "INP-10: pending ink after {why}");
        assert!(d.app.live_stroke.is_none(), "INP-10: live stroke after {why}");
        check(d);
    };
    let e = empty_cells(&d);
    d.write(e[0], solution_at(&d.app.game, e[0]));
    assert!(d.app.game.is_filled(e[0]));
    no_ink(&d, "a number");

    d.write_strokes(&scribble_ink(e[1]));
    d.wait(d.app.pause_ms + 50);
    assert!(d.app.ui.question.is_some());
    no_ink(&d, "a \"?\"");
    d.wait(1100);
    no_ink(&d, "the end of the \"?\"");

    d.write(e[0], 4);
    no_ink(&d, "writing on a filled cell");
    d.write(given_cell(&d), 4);
    no_ink(&d, "writing on a starting number");

    // A press in a different cell reads the ink immediately.
    d.write_strokes(&digit_ink(e[2], 3));
    d.tap_micro(e[3], 1);
    no_ink(&d, "a press in a different cell");

    // The pen writes, and a finger presses the menu button before the pen lifts. The press reads the
    // ink. The rest of the stroke is not ink.
    let (x, y) = micro_center(e[4], 1);
    d.send(Input::PenDown { x, y, tool: Tool::Pen });
    d.send(Input::PenMove { x: x + 40.0, y: y + 40.0 });
    d.send(Input::PenMove { x: x + 70.0, y: y + 70.0 });
    assert!(d.app.ui.pending_ink.is_some() && d.app.live_stroke.is_some());
    let (bx, by) = button_center(Button::Menu);
    d.send(Input::TouchDown { id: 1, x: bx, y: by });
    no_ink(&d, "a finger press on a control while the pen writes");
    d.send(Input::PenMove { x: x + 90.0, y: y + 20.0 });
    d.send(Input::PenUp { x: x + 90.0, y: y + 20.0 });
    d.send(Input::TouchUp { id: 1, x: bx, y: by });
    d.wait(d.app.pause_ms + 50);
    no_ink(&d, "the release of the pen and the finger");

    // The pen writes and the eraser tool presses a different cell.
    d.write_strokes(&digit_ink(e[5], 6));
    d.erase(e[0]);
    no_ink(&d, "an eraser press in a different cell");
}

// ---- INP-14

/// The point is not on a control of the manual or the games list, thus the touch is only a swipe.
fn not_on_a_control(x: f32, y: f32) -> bool {
    let mut rects = vec![layout::back_button(), layout::prev_arrow(), layout::next_arrow()];
    rects.extend((0..6).map(layout::filter_segment));
    rects.extend((0..12).map(layout::bin_button));
    !rects.iter().any(|r| r.contains(x, y))
}

/// A finger movement with this dx and dy, from a point that is not on a control.
fn finger_dxdy(d: &mut Driver, dx: f32, dy: f32) {
    let (x0, y0) = (800.0, 1000.0);
    assert!(not_on_a_control(x0, y0) && not_on_a_control(x0 + dx, y0 + dy));
    finger_move(d, (x0, y0), (x0 + dx, y0 + dy));
}

/// Examines the limits of a swipe on the current screen. `page` gives the page number.
fn swipe_limits(d: &mut Driver, page: fn(&App) -> usize) {
    let p = page(&d.app);
    assert!(p >= 2, "the test needs a page before and a page after");
    for (dx, dy, why) in [
        (SWIPE_MIN - 1.0, 0.0, "dx 119"),
        (-(SWIPE_MIN - 1.0), 0.0, "dx -119"),
        (150.0, 150.0, "|dx| = |dy|"),
        (-150.0, -150.0, "|dx| = |dy|, up"),
        (-150.0, 150.0, "|dx| = |dy|, down"),
        (-130.0, 200.0, "more vertical than horizontal"),
        (0.0, 0.0, "no movement"),
    ] {
        finger_dxdy(d, dx, dy);
        assert_eq!(page(&d.app), p, "INP-14: {why} is not a swipe");
    }
    finger_dxdy(d, -SWIPE_MIN, 0.0);
    assert_eq!(page(&d.app), p + 1, "INP-14: dx -120 is a swipe to the next page");
    finger_dxdy(d, SWIPE_MIN, 0.0);
    assert_eq!(page(&d.app), p, "INP-14: dx 120 is a swipe to the page before");
    finger_dxdy(d, -SWIPE_MIN, SWIPE_MIN - 1.0);
    assert_eq!(page(&d.app), p + 1, "INP-14: more horizontal than vertical");
    finger_dxdy(d, SWIPE_MIN, -(SWIPE_MIN - 1.0));
    assert_eq!(page(&d.app), p);
    // The pen does not swipe.
    pen_stroke(d, Tool::Pen, (800.0, 1000.0), (500.0, 1000.0));
    assert_eq!(page(&d.app), p, "the pen does not swipe");
}

#[test]
fn a_swipe_is_120_px_or_more_and_more_horizontal_than_vertical() {
    // INP-14 on the manual and on the games list, in the two directions.
    let mut d = Driver::new(107);
    d.button(Button::Menu);
    d.button(Button::Help);
    d.swipe(true);
    assert_eq!((d.app.screen, d.app.manual_page), (Screen::Manual, 2));
    swipe_limits(&mut d, |a| a.manual_page as usize);

    let mut d = Driver::from(app_with_results(30));
    d.button(Button::Menu);
    d.button(Button::Statistics);
    let (x, y) = center(layout::all_games_button());
    d.finger_tap(x, y);
    assert_eq!(d.app.screen, Screen::GamesList);
    assert_eq!(d.app.page_count(), 3);
    d.swipe(true);
    assert_eq!(d.app.list.page, 2);
    swipe_limits(&mut d, |a| a.list.page);
}

// ---- QST-3

/// A saved game: all cells have the solution except `x`, which is empty. The last change of the undo
/// history is the erasure of the correct number in `x` (`undo_erasure`), or the redo history has the
/// correct number in `x`. Such a history is possible only in a saved game, because the correct number
/// in the last cell completes the puzzle at once. The app accepts it (PER-5).
fn last_cell_game(seed: u64, undo_erasure: bool) -> (Game, Cell) {
    let mut g = new_game(seed, 2);
    let empties: Vec<Cell> = cells().filter(|&c| !g.is_filled(c)).collect();
    let x = empties[0];
    let sx = solution_at(&g, x);
    for &c in &empties {
        let digit = solution_at(&g, c);
        if c != x {
            g.value[c.0 as usize][c.1 as usize] = digit;
            g.undo.push(Change::AddNumber { cell: c, digit, removed: Vec::new() });
        }
    }
    if undo_erasure {
        g.undo.insert(0, Change::AddNumber { cell: x, digit: sx, removed: Vec::new() });
        g.undo.push(Change::Erase { cell: x, digit: sx });
    } else {
        g.redo.push(Change::AddNumber { cell: x, digit: sx, removed: Vec::new() });
    }
    sudoku::invariants::check_game(&g).unwrap();
    (g, x)
}

#[test]
fn no_question_mark_on_a_complete_game() {
    // QST-3: the last number completes the puzzle while the "?" can show in its cell.
    // 1. A "?" in the last cell, then the correct number.
    let mut d = Driver::new(108);
    let empties = empty_cells(&d);
    let x = empties[0];
    for &c in &empties[1..] {
        d.app.game.add_number(c, solution_at(&d.app.game, c));
    }
    d.write_strokes(&scribble_ink(x));
    d.wait(d.app.pause_ms + 50);
    assert_eq!(d.app.ui.question.map(|q| q.0), Some(x));
    assert!(!d.app.game.completed);
    d.write(x, solution_at(&d.app.game, x));
    assert!(d.app.game.completed);
    assert_eq!(d.app.ui.question, None, "QST-3");
    check(&d);
    // Writing on the complete game does not show a "?".
    d.write_strokes(&scribble_ink(x));
    d.wait(d.app.pause_ms + 50);
    assert_eq!(d.app.ui.question, None, "QST-3 after a scribble on a complete game");

    // 2. Undo of the erasure and redo of the number complete the puzzle. The press on the button reads
    // a scribble in the cell and shows the "?"; the release does the undo or the redo. With a finger
    // and with the pen.
    for undo_erasure in [true, false] {
        for finger in [true, false] {
            let (g, x) = last_cell_game(109, undo_erasure);
            let app = App::new(Some(g.clone()), None, ResultsDb::in_memory(), Box::new(TestEnv { time: 1_790_000_000, seed: 3 }));
            assert_eq!(app.game, g, "the app uses the saved game");
            let mut d = Driver::from(app);
            d.button(Button::Menu);
            d.write_strokes(&scribble_ink(x));
            let (bx, by) = button_center(if undo_erasure { Button::Undo } else { Button::Redo });
            if finger {
                d.send(Input::TouchDown { id: 1, x: bx, y: by });
            } else {
                d.send(Input::PenDown { x: bx, y: by, tool: Tool::Pen });
            }
            assert_eq!(d.app.ui.question.map(|q| q.0), Some(x), "the press reads the scribble");
            assert!(!d.app.game.completed);
            if finger {
                d.send(Input::TouchUp { id: 1, x: bx, y: by });
            } else {
                d.send(Input::PenUp { x: bx, y: by });
            }
            assert!(d.app.game.completed, "undo_erasure {undo_erasure}, finger {finger}");
            assert_eq!(d.app.ui.question, None, "QST-3: undo_erasure {undo_erasure}, finger {finger}");
            check(&d);
            d.wait(2000);
            assert_eq!(d.app.ui.question, None);
        }
    }

    // 3. A finger holds Undo while the pen writes a scribble. The release reads the ink ("?") and then
    // does the undo.
    let (g, x) = last_cell_game(110, true);
    let mut d = Driver::from(App::new(Some(g), None, ResultsDb::in_memory(), Box::new(TestEnv { time: 1_790_000_000, seed: 3 })));
    d.button(Button::Menu);
    let (bx, by) = button_center(Button::Undo);
    d.send(Input::TouchDown { id: 1, x: bx, y: by });
    d.write_strokes(&scribble_ink(x));
    assert!(d.app.ui.pending_ink.is_some());
    d.send(Input::TouchUp { id: 1, x: bx, y: by });
    assert!(d.app.game.completed);
    assert_eq!(d.app.ui.question, None, "QST-3");
    check(&d);
}

// ---- NAV-2

#[test]
fn only_the_open_menu_opens_the_manual_and_statistics() {
    // NAV-2: with the menu closed, taps at the positions of Help and Statistics do nothing. With a
    // panel open, the tap only closes the panel. Only Statistics opens the games list. "< Back" goes
    // to the correct screen.
    let mut d = Driver::new(111);
    for b in [Button::Help, Button::Statistics] {
        let (x, y) = button_center(b);
        d.finger_tap(x, y);
        d.pen_tap(x, y);
        pen_stroke(&mut d, Tool::Eraser, (x, y), (x + 5.0, y + 5.0));
        assert_eq!(d.app.screen, Screen::Game, "NAV-2: {b:?} with the menu closed");
        assert!(!d.app.ui.menu_open);
    }
    d.button(Button::Menu);
    for panel in [Button::Colors, Button::Settings, Button::NewGame] {
        for b in [Button::Help, Button::Statistics] {
            d.button(panel);
            d.button(b);
            assert_eq!(d.app.screen, Screen::Game, "NAV-2: {b:?} with the panel of {panel:?} only closes the panel");
            assert_eq!((d.app.ui.panel, d.app.ui.menu_open), (Panel::None, true));
        }
    }
    // A press on Help and a release at a different place does not open the manual.
    let (x, y) = button_center(Button::Help);
    finger_move(&mut d, (x, y), (x - 600.0, y));
    assert_eq!(d.app.screen, Screen::Game);

    // Only Statistics opens the games list. The manual has no "All completed games" button.
    let (ax, ay) = center(layout::all_games_button());
    d.finger_tap(ax, ay);
    assert_ne!(d.app.screen, Screen::GamesList, "NAV-2: the game screen does not open the games list");
    d.button(Button::Help);
    assert_eq!(d.app.screen, Screen::Manual);
    d.finger_tap(ax, ay);
    d.pen_tap(ax, ay);
    assert_eq!(d.app.screen, Screen::Manual, "NAV-2: the manual does not open the games list");
    let (backx, backy) = center(layout::back_button());
    d.finger_tap(backx, backy);
    assert_eq!(d.app.screen, Screen::Game, "Back: Manual to Game");
    d.button(Button::Menu);
    d.button(Button::Statistics);
    assert_eq!(d.app.screen, Screen::Statistics);
    d.pen_tap(ax, ay);
    assert_eq!(d.app.screen, Screen::GamesList, "Statistics opens the games list");
    d.finger_tap(backx, backy);
    assert_eq!(d.app.screen, Screen::Statistics, "Back: GamesList to Statistics");
    d.pen_tap(backx, backy);
    assert_eq!(d.app.screen, Screen::Game, "Back: Statistics to Game");
    check(&d);
}

// ---- CLK-6 (the limit of 5000 ms; the platform part is in tests/qtfb_e2e.rs)

#[test]
fn a_gap_of_5000_ms_counts_and_a_gap_of_5001_ms_does_not() {
    // CLK-6
    assert_eq!(MAX_CLOCK_GAP_MS, 5000);
    let mut d = Driver::new(112);
    d.app.tick(d.now);
    let t = d.app.game.elapsed_ms;
    d.now += 5000;
    d.app.tick(d.now);
    assert_eq!(d.app.game.elapsed_ms, t + 5000, "CLK-6: a gap of 5000 ms counts");
    d.now += 5001;
    d.app.tick(d.now);
    assert_eq!(d.app.game.elapsed_ms, t + 5000, "CLK-6: a gap of 5001 ms does not count");
    // The clock continues after the gap.
    d.now += 100;
    d.app.tick(d.now);
    assert_eq!(d.app.game.elapsed_ms, t + 5100);

    // The same limits with input events instead of ticks.
    let t = d.app.game.elapsed_ms;
    let cell = empty_cells(&d)[0];
    let (x, y) = micro_center(cell, 1);
    d.now += 5000 - 10; // Driver::send adds 10 ms.
    d.send(Input::TouchDown { id: 1, x, y });
    assert_eq!(d.app.game.elapsed_ms, t + 5000, "CLK-6: an event after 5000 ms");
    d.now += 5001 - 10;
    d.send(Input::TouchUp { id: 1, x, y });
    assert_eq!(d.app.game.elapsed_ms, t + 5000, "CLK-6: an event after 5001 ms");
}

#[test]
fn the_eraser_on_waiting_ink_removes_the_ink() {
    // INP-8, INP-12, UND-1 (confirmed 2026-10-25): the eraser on the cell of the waiting ink removes
    // the ink. The app does not read it, thus there is no change and no undo item.
    let mut d = Driver::new(61);
    let empty = empty_cells(&d);
    let (cell, other) = (empty[0], empty[1]);
    d.tap_micro(cell, 4);
    d.tap_micro(other, 2);
    let v = solution_at(&d.app.game, cell);
    d.write_strokes(&digit_ink(cell, v));
    assert!(d.app.ui.pending_ink.is_some());
    let before = state(&d);
    let undo_before = d.app.game.undo.clone();
    d.erase(cell);
    assert!(d.app.ui.pending_ink.is_none() && d.app.live_stroke.is_none(), "the eraser removes the ink");
    assert_eq!(state(&d), before, "the eraser on waiting ink does not change the board or the history");
    assert_eq!(d.app.game.undo, undo_before);
    d.wait(d.app.pause_ms + 50);
    assert_eq!(state(&d), before, "the app never reads the removed ink");
    check(&d);

    // An undo after this undoes the change before the ink: the mark in the other cell.
    d.button(Button::Menu);
    d.button(Button::Undo);
    assert_eq!(d.app.game.marks_at(other), 0);
    assert_eq!(d.app.game.value_at(cell), 0);
    check(&d);

    // The eraser in a different cell reads the ink as a press on a different item does (INP-12). The
    // number stays, because the eraser on an empty cell makes no change (INP-8).
    let mut d = Driver::new(62);
    let empty = empty_cells(&d);
    let (cell, other) = (empty[0], empty[1]);
    let v = solution_at(&d.app.game, cell);
    d.write_strokes(&digit_ink(cell, v));
    let undo_len = d.app.game.undo.len();
    d.erase(other);
    assert!(d.app.ui.pending_ink.is_none());
    assert_eq!(d.app.game.value_at(cell), v, "the eraser in a different cell reads the ink");
    assert_eq!(d.app.game.undo.len(), undo_len + 1, "one undo item: the number");
    check(&d);
}
