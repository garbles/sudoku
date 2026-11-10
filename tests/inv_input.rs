//! Tests of the rules of INVARIANTS.md about input on the board: BRD-3, BRD-4, MRK-3, MRK-5, INP-4,
//! INP-10 (the state), INP-14, QST-3, NAV-2 and CLK-6 (the 5000 ms limit), the highlight (HLT-2 to HLT-9),
//! the hand touches and the ink (INP-2, INP-5, INP-7, INP-12, INP-15, INP-16, INP-20 to INP-24, QST-4,
//! MRK-15, NAV-11, CMP-3). Each test puts the app in the state where the rule can break and then examines
//! the rule itself. The release build does not run `invariants::check_app` after each input, thus the
//! tests examine the rules directly.

mod common;

use common::*;
use sudoku::app::{new_game, App, Input, Menu, Panel, Screen, Tool, HAND_TOUCH_MS, MAX_CLOCK_GAP_MS, OVERLAY_MS, RECOGNITION_PAUSE_MS, WRITING_IDLE_MS};
use sudoku::game::{cells, mark_bit, Cell, Change, Game, Marks};
use sudoku::layout::{self, Button, SWIPE_MIN};
use sudoku::reference;
use sudoku::settings::{Color, Section, Switch};

/// The board state that input can change: values, marks and the lengths of the two histories.
type BoardState = ([[u8; 9]; 9], [[Marks; 9]; 9], usize, usize);

fn state(d: &Driver) -> BoardState {
    let g = &d.app.game;
    (g.value, g.marks, g.undo.len(), g.redo.len())
}

fn empty_cells(d: &Driver) -> Vec<Cell> {
    cells().filter(|&c| !d.app.game.is_filled(c)).collect()
}

/// The digits that no peer of the cell has. A pen tap on another digit does not add a mark, and it
/// shows the x (MRK-7, MRK-10). Thus the fixtures that need a mark use these digits.
fn free_digits(d: &Driver, cell: Cell) -> Vec<u8> {
    (1..=9).filter(|&v| d.app.game.peer_digits(cell) & mark_bit(v) == 0).collect()
}

fn solution_at(g: &Game, (r, c): Cell) -> u8 {
    g.solution[r as usize][c as usize]
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
    assert_eq!(d.app.game.marks_at(cell), mark_bit(2), "the fixture has the mark");
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
    for panel in [Button::Colors, Button::Settings, Button::Checkpoints, Button::NewGame] {
        d.button(panel);
        d.tap_micro(cell, 1);
        d.write(cell, 4);
        entry_ok(&d, &format!("pen on the entry with the panel of {panel:?}"));
    }
    // Undo and redo of a different change do not remove the entry.
    let mark = free_digits(&d, other)[0];
    d.tap_micro(other, mark);
    assert_eq!(d.app.game.marks_at(other), mark_bit(mark), "the fixture has the mark in the different cell");
    d.button(Button::Undo);
    assert_eq!(d.app.game.marks_at(other), 0, "the undo removes the mark, not the entry");
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
    let given = given_cell(&d.app.game);
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
    let given = given_cell(&d.app.game);
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
    for panel in [Button::Colors, Button::Settings, Button::Checkpoints, Button::NewGame] {
        d.button(panel);
        d.tap_micro(given, 1);
        d.tap_micro(given, 9);
        no_marks(&d, &format!("pen taps with the panel of {panel:?}"));
    }
    let cell = empty_cells(&d)[0];
    let mark = free_digits(&d, cell)[0];
    d.tap_micro(cell, mark);
    assert_eq!(d.app.game.marks_at(cell), mark_bit(mark), "the fixture has the mark");
    d.tap_micro(given, mark);
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
    let app = app_with_game(g.clone(), START_TIME, 5);
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
    // Each digit that no peer has: the empty cell and the entry have marks (MRK-7).
    for cell in [a, b] {
        for v in free_digits(&d, cell) {
            d.tap_micro(cell, v);
        }
        assert_ne!(d.app.game.marks_at(cell), 0, "the fixture has the marks of {cell:?}");
    }
    let w = (1..=9).find(|&w| d.app.game.marks_at(a) & mark_bit(w) == 0 && d.app.game.marks_at(b) & mark_bit(w) == 0).unwrap();
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
    d.erase(given_cell(&d.app.game));
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
    assert_eq!(d.app.ui.menu.panel(), None);
    same(&d, "pen tap that closes a panel");
    // The eraser over the entry with hidden marks: the marks show again and do not change.
    d.erase(b);
    assert_eq!(d.app.game.value_at(b), 0);
    same(&d, "eraser on an entry with hidden marks");
    // An eraser stroke across many cells.
    let (r0, r1) = (layout::board_x() as f32 + 5.0, (layout::board_x() + layout::board_size()) as f32 - 5.0);
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
    let mark = free_digits(&d, cell)[0];
    d.tap_micro(cell, mark);
    assert_eq!(d.app.game.marks_at(cell), mark_bit(mark), "the fixture has the mark");
    d.write(cell, v);
    d.button(Button::Menu);
    let mark = free_digits(&d, other)[0];
    d.tap_micro(other, mark);
    assert_eq!(d.app.game.marks_at(other), mark_bit(mark), "the fixture has the mark");
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
    assert_eq!(d.app.game.marks_at(other), mark_bit(mark));
}

// ---- INP-10 (the state; the drawing is in another test file)

#[test]
fn no_ink_stays_after_the_app_reads_or_removes_it() {
    // INP-10, INP-21: after a number, after a "?", after writing on a filled cell, after a press in a
    // different cell, and after the release of a control while the pen writes, no ink waits and no live
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
    d.write(given_cell(&d.app.game), 4);
    no_ink(&d, "writing on a starting number");

    // A press in a different cell reads the ink immediately.
    d.write_strokes(&digit_ink(e[2], 3));
    d.tap_micro(e[3], 1);
    no_ink(&d, "a press in a different cell");

    // The pen writes, and a finger presses the menu button before the pen lifts. The press does not read
    // the ink (INP-21). The release on the control reads it, and the rest of the stroke is not ink.
    let (x, y) = micro_center(e[4], 1);
    d.send(Input::PenDown { x, y, tool: Tool::Pen });
    d.send(Input::PenMove { x: x + 40.0, y: y + 40.0 });
    d.send(Input::PenMove { x: x + 70.0, y: y + 70.0 });
    assert!(d.app.ui.pending_ink.is_some() && d.app.live_stroke.is_some());
    let (bx, by) = button_center(Button::Menu);
    d.send(Input::TouchDown { id: 1, x: bx, y: by });
    assert!(d.app.ui.pending_ink.is_some() && d.app.live_stroke.is_some(), "INP-21: a finger press does not read the ink");
    d.send(Input::TouchUp { id: 1, x: bx, y: by });
    no_ink(&d, "the release of a control while the pen writes");
    d.send(Input::PenMove { x: x + 90.0, y: y + 20.0 });
    no_ink(&d, "the rest of the stroke");
    d.send(Input::PenUp { x: x + 90.0, y: y + 20.0 });
    d.wait(d.app.pause_ms + 50);
    no_ink(&d, "the lift of the pen");
    d.wait(OVERLAY_MS + 50);

    // The same, but the pen lifts before the finger: the ink waits until the release of the control.
    let cell = empty_cells(&d)[0];
    let ink = digit_ink(cell, solution_at(&d.app.game, cell));
    d.send(Input::TouchDown { id: 1, x: bx, y: by });
    d.write_strokes(&ink);
    assert!(d.app.ui.pending_ink.is_some(), "INP-21: the finger on the control does not stop the writing");
    d.send(Input::TouchUp { id: 1, x: bx, y: by });
    no_ink(&d, "the release of a control after the pen lifts");
    d.wait(d.app.pause_ms + 50);

    // The pen writes and the eraser tool presses a different cell.
    d.write_strokes(&digit_ink(e[5], 6));
    d.erase(e[0]);
    no_ink(&d, "an eraser press in a different cell");
}

// ---- INP-14

/// The point is not on a control of the manual or the games list, thus the touch is only a swipe.
fn not_on_a_control(x: f32, y: f32) -> bool {
    let mut rects = vec![layout::back_button(), layout::prev_arrow(), layout::next_arrow()];
    rects.extend(layout::list_filters().take(6).map(layout::filter_segment));
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
    let (x, y) = pixel_center(layout::all_games_button());
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

    // 2. Undo of the erasure and redo of the number complete the puzzle. A pen press on the button reads
    // a scribble in the cell and shows the "?", and a finger reads it at the release (INP-21). The
    // release does the undo or the redo. With a finger and with the pen.
    for undo_erasure in [true, false] {
        for finger in [true, false] {
            let (g, x) = last_cell_game(109, undo_erasure);
            let app = app_with_game(g.clone(), START_TIME, 3);
            assert_eq!(app.game, g, "the app uses the saved game");
            let mut d = Driver::from(app);
            d.button(Button::Menu);
            d.write_strokes(&scribble_ink(x));
            let (bx, by) = button_center(if undo_erasure { Button::Undo } else { Button::Redo });
            if finger {
                // INP-21: a finger press does not read the ink. The release reads it, and then does the
                // action.
                d.send(Input::TouchDown { id: 1, x: bx, y: by });
                assert!(d.app.ui.pending_ink.is_some() && d.app.ui.question.is_none(), "INP-21: the press does not read the scribble");
            } else {
                d.send(Input::PenDown { x: bx, y: by, tool: Tool::Pen });
                assert_eq!(d.app.ui.question.map(|q| q.0), Some(x), "INP-12: the pen press reads the scribble");
            }
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
    let mut d = Driver::from(app_with_game(g, START_TIME, 3));
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
        assert!(!d.app.ui.menu.is_open());
    }
    d.button(Button::Menu);
    for panel in [Button::Colors, Button::Settings, Button::Checkpoints, Button::NewGame] {
        for b in [Button::Help, Button::Statistics] {
            d.button(panel);
            d.button(b);
            assert_eq!(d.app.screen, Screen::Game, "NAV-2: {b:?} with the panel of {panel:?} only closes the panel");
            assert_eq!(d.app.ui.menu, Menu::Open(None));
        }
    }
    // A press on Help and a release at a different place does not open the manual.
    let (x, y) = button_center(Button::Help);
    finger_move(&mut d, (x, y), (x - 600.0, y));
    assert_eq!(d.app.screen, Screen::Game);

    // Only Statistics opens the games list. The manual has no "All completed games" button.
    let (ax, ay) = pixel_center(layout::all_games_button());
    d.finger_tap(ax, ay);
    assert_ne!(d.app.screen, Screen::GamesList, "NAV-2: the game screen does not open the games list");
    d.button(Button::Help);
    assert_eq!(d.app.screen, Screen::Manual);
    d.finger_tap(ax, ay);
    d.pen_tap(ax, ay);
    assert_eq!(d.app.screen, Screen::Manual, "NAV-2: the manual does not open the games list");
    let (backx, backy) = pixel_center(layout::back_button());
    d.finger_tap(backx, backy);
    assert_eq!(d.app.screen, Screen::Game, "Back: Manual to Game");
    assert_eq!(d.app.ui.menu, Menu::Open(None), "NAV-3: the menu is open after Back");
    // NAV-16: the toolbar operates again after BACK_GUARD_MS.
    d.wait(sudoku::app::BACK_GUARD_MS);
    d.button(Button::Statistics);
    assert_eq!(d.app.screen, Screen::Statistics);
    d.pen_tap(ax, ay);
    assert_eq!(d.app.screen, Screen::GamesList, "Statistics opens the games list");
    d.finger_tap(backx, backy);
    assert_eq!(d.app.screen, Screen::Statistics, "Back: GamesList to Statistics");
    d.pen_tap(backx, backy);
    assert_eq!(d.app.screen, Screen::Game, "Back: Statistics to Game");
    assert_eq!(d.app.ui.menu, Menu::Open(None), "NAV-3: the menu is open after Back");
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
    let (mark, other_mark) = (free_digits(&d, cell)[0], free_digits(&d, other)[0]);
    d.tap_micro(cell, mark);
    d.tap_micro(other, other_mark);
    assert_eq!((d.app.game.marks_at(cell), d.app.game.marks_at(other)), (mark_bit(mark), mark_bit(other_mark)), "the fixture has the marks");
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

// ---- HLT-2 to HLT-9, INP-20 to INP-24: in all layouts.

const LAYOUTS: [(layout::Orientation, bool); 4] = [
    (layout::Orientation::Portrait, false),
    (layout::Orientation::Portrait, true),
    (layout::Orientation::Landscape, false),
    (layout::Orientation::Landscape, true),
];

/// The layout of a test. When the value goes out of scope, it sets the default layout again.
struct InLayout;

impl InLayout {
    fn set(o: layout::Orientation, right_handed: bool) -> InLayout {
        layout::set_orientation(o);
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

/// Two filled cells with different digits.
fn two_numbers(g: &Game) -> ((Cell, u8), (Cell, u8)) {
    let a = cells().find(|&c| g.is_given(c)).unwrap();
    let b = cells().find(|&c| g.is_given(c) && g.value_at(c) != g.value_at(a)).unwrap();
    ((a, g.value_at(a)), (b, g.value_at(b)))
}

/// A pen tap on a number: the highlight shows.
fn highlight_number(d: &mut Driver, cell: Cell) -> u8 {
    let v = d.app.game.value_at(cell);
    d.tap_micro(cell, 5);
    assert_eq!(d.app.ui.highlight, Some(v), "the fixture has the highlight");
    v
}

/// A point on no control and not on the board of the game screen, in the current layout.
fn off_board_on_no_control() -> (f32, f32) {
    (0..layout::screen_h() as i32)
        .step_by(40)
        .flat_map(|y| (0..layout::screen_w() as i32).step_by(40).map(move |x| (x as f32 + 5.0, y as f32 + 5.0)))
        .find(|&(x, y)| layout::board_micro(x, y).is_none() && layout::game_button(x, y, true).is_none() && layout::game_button(x, y, false).is_none())
        .expect("a point off the board")
}

/// The update dialog shows (UPD-10).
fn show_update_dialog(d: &mut Driver) {
    let mut u = reference::update_state(true);
    u.tap();
    u.request = None;
    d.app.update = u;
    let now = d.now;
    d.app.update_installed(Ok(true), now);
    assert!(d.app.update.dialog_shown(), "the fixture has the dialog");
}

/// An empty cell and a mark that a pen tap adds to it.
fn free_spot(d: &Driver) -> (Cell, u8) {
    let cell = cells().find(|&c| !d.app.game.is_filled(c) && !free_digits(d, c).is_empty()).unwrap();
    (cell, free_digits(d, cell)[0])
}

#[test]
fn a_tap_removes_or_moves_a_highlight_only_at_the_pen_lift() {
    // HLT-2 items 1 and 2, HLT-3, HLT-9, MRK-10: all layouts. The press changes nothing. The tap on the
    // same number removes it, on a different number moves it, on an empty cell removes it with the mark
    // or with the x.
    for (o, right_handed) in LAYOUTS {
        let _layout = InLayout::set(o, right_handed);
        let what = format!("{o:?}, right-handed {right_handed}");
        let mut d = Driver::new(601);
        let ((a, v), (b, w)) = two_numbers(&d.app.game);

        // No highlight: the lift makes it, the press does not.
        let (x, y) = micro_center(a, 3);
        d.send(Input::PenDown { x, y, tool: Tool::Pen });
        assert_eq!(d.app.ui.highlight, None, "HLT-9: {what}");
        d.send(Input::PenUp { x, y });
        assert_eq!(d.app.ui.highlight, Some(v), "HLT-3: {what}");

        // A different number.
        let (x, y) = micro_center(b, 7);
        d.send(Input::PenDown { x, y, tool: Tool::Pen });
        assert_eq!(d.app.ui.highlight, Some(v), "HLT-9: {what}: the press on a different number");
        d.send(Input::PenUp { x, y });
        assert_eq!(d.app.ui.highlight, Some(w), "HLT-3: {what}: the highlight moves");

        // The same number, on a different cell with the same digit if the board has one.
        let same = cells().find(|&c| c != b && d.app.game.value_at(c) == w).unwrap_or(b);
        let (x, y) = micro_center(same, 1);
        d.send(Input::PenDown { x, y, tool: Tool::Pen });
        assert_eq!(d.app.ui.highlight, Some(w), "HLT-9: {what}: the press on the same number");
        d.send(Input::PenUp { x, y });
        assert_eq!(d.app.ui.highlight, None, "HLT-3: {what}: the same digit removes the highlight");

        // An empty cell: the mark and the removal at the lift.
        highlight_number(&mut d, a);
        let (cell, mark) = free_spot(&d);
        let (x, y) = micro_center(cell, mark);
        d.send(Input::PenDown { x, y, tool: Tool::Pen });
        assert_eq!((d.app.ui.highlight, d.app.game.marks_at(cell)), (Some(v), 0), "HLT-9: {what}: the press on an empty cell");
        d.send(Input::PenUp { x, y });
        assert_eq!((d.app.ui.highlight, d.app.game.marks_at(cell)), (None, mark_bit(mark)), "HLT-2: {what}: the mark");

        // An empty cell: the x and the removal at the lift.
        highlight_number(&mut d, a);
        let (cell, digit) = refused_pair(&d.app.game);
        let (x, y) = micro_center(cell, digit);
        d.send(Input::PenDown { x, y, tool: Tool::Pen });
        assert_eq!((d.app.ui.highlight, d.app.ui.refused), (Some(v), None), "HLT-9: {what}: the press of a refused tap");
        d.send(Input::PenUp { x, y });
        assert_eq!(d.app.ui.highlight, None, "HLT-2: {what}: the x");
        assert_eq!(d.app.ui.refused, Some((cell, digit, d.now)), "MRK-10: {what}");
        check(&d);
    }
}

#[test]
fn a_stroke_from_a_filled_cell_removes_a_highlight_at_the_pen_lift() {
    // HLT-2 item 3, INP-7, INP-22, HLT-9: the press and the movement change nothing. The lift removes the
    // highlight. The board, the marks and the stacks do not change, and no ink waits. The writing timer
    // starts (INP-16).
    for (o, right_handed) in LAYOUTS {
        let _layout = InLayout::set(o, right_handed);
        let what = format!("{o:?}, right-handed {right_handed}");
        let mut d = Driver::new(602);
        let e = empty_cells(&d);
        d.write(e[0], solution_at(&d.app.game, e[0]));
        assert!(d.app.game.is_entry(e[0]), "the fixture has an entry");
        d.wait(WRITING_IDLE_MS);
        for from in [e[0], given_cell(&d.app.game)] {
            let v = highlight_number(&mut d, from);
            let before = state(&d);
            let (x0, y0) = micro_center(from, 1);
            let (x1, y1) = micro_center(e[1], 9);
            d.send(Input::PenDown { x: x0, y: y0, tool: Tool::Pen });
            d.send(Input::PenMove { x: (x0 + x1) / 2.0, y: (y0 + y1) / 2.0 });
            assert_eq!(d.app.ui.highlight, Some(v), "HLT-9: {what}: the highlight stays until the lift");
            d.send(Input::PenUp { x: x1, y: y1 });
            assert_eq!(d.app.ui.highlight, None, "HLT-2: {what}: the lift of the stroke");
            assert_eq!(state(&d), before, "INP-7: {what}");
            assert!(d.app.ui.pending_ink.is_none() && d.app.live_stroke.is_none(), "INP-22: {what}");
            assert!(d.app.is_writing(), "INP-16: {what}: a stroke on the board starts the timer");
            d.wait(WRITING_IDLE_MS);
            assert!(!d.app.is_writing(), "INP-16: {what}: the timer ends after 1 s");
            check(&d);
        }
    }
}

#[test]
fn a_read_of_waiting_ink_removes_a_highlight_by_any_cause() {
    // HLT-2 item 4, HLT-9, INP-5, INP-12, INP-21: a highlight stays while ink waits. The read removes it:
    // at the end of the pause, at a pen press in a different cell, off the board on no control, on a
    // control, and at the release of a control that a finger operates. It is a number and a "?".
    for (o, right_handed) in LAYOUTS {
        let _layout = InLayout::set(o, right_handed);
        let what = format!("{o:?}, right-handed {right_handed}");
        for number in [true, false] {
            let what = format!("{what}, number {number}");
            let ink_of = |d: &Driver, cell: Cell| if number { digit_ink(cell, solution_at(&d.app.game, cell)) } else { scribble_ink(cell) };
            let read_ok = |d: &Driver, cell: Cell, why: &str| {
                assert_eq!(d.app.ui.highlight, None, "HLT-2: {what}: {why}");
                assert!(d.app.ui.pending_ink.is_none(), "INP-5: {what}: {why}");
                assert_eq!(d.app.game.value_at(cell) != 0, number, "INP-5: {what}: {why}: the value");
                assert_eq!(d.app.ui.question.is_some_and(|q| q.0 == cell), !number, "QST-1: {what}: {why}: the \"?\"");
                check(d);
            };
            let mut d = Driver::new(603);
            let given = given_cell(&d.app.game);
            d.button(Button::Menu);

            // The pause.
            let e = empty_cells(&d);
            let v = highlight_number(&mut d, given);
            let ink = ink_of(&d, e[0]);
            d.write_strokes(&ink);
            assert_eq!(d.app.ui.highlight, Some(v), "HLT-9: {what}: the highlight stays with the ink");
            d.wait(RECOGNITION_PAUSE_MS - 1);
            assert_eq!(d.app.ui.highlight, Some(v), "INP-5: {what}: 1 ms before the end of the pause");
            assert!(d.app.ui.pending_ink.is_some(), "INP-5: {what}");
            d.wait(1);
            read_ok(&d, e[0], "the pause");
            d.wait(OVERLAY_MS);

            // A pen press in a different cell.
            let e = empty_cells(&d);
            highlight_number(&mut d, given);
            let ink = ink_of(&d, e[0]);
            d.write_strokes(&ink);
            let (x, y) = micro_center(e[1], 5);
            d.send(Input::PenDown { x, y, tool: Tool::Pen });
            read_ok(&d, e[0], "a pen press in a different cell");
            d.send(Input::PenUp { x, y });
            d.wait(OVERLAY_MS);

            // A pen press off the board, on no control.
            let e = empty_cells(&d);
            highlight_number(&mut d, given);
            let ink = ink_of(&d, e[0]);
            d.write_strokes(&ink);
            let (x, y) = off_board_on_no_control();
            d.send(Input::PenDown { x, y, tool: Tool::Pen });
            read_ok(&d, e[0], "a pen press off the board");
            d.send(Input::PenUp { x, y });
            d.wait(OVERLAY_MS);

            // A pen press on a control: the number at the press, the action at the release.
            let e = empty_cells(&d);
            highlight_number(&mut d, given);
            let ink = ink_of(&d, e[0]);
            d.write_strokes(&ink);
            let (x, y) = button_center(Button::Colors);
            d.send(Input::PenDown { x, y, tool: Tool::Pen });
            read_ok(&d, e[0], "a pen press on a control");
            assert_eq!(d.app.ui.menu.panel(), None, "HLT-9: {what}: the action comes at the release");
            d.send(Input::PenUp { x, y });
            assert_eq!(d.app.ui.menu.panel(), Some(Panel::Colors), "{what}");
            d.button(Button::Colors);
            d.wait(OVERLAY_MS);

            // A finger on a control: the press keeps the ink and the highlight, the release reads it.
            let e = empty_cells(&d);
            let v = highlight_number(&mut d, given);
            let ink = ink_of(&d, e[0]);
            d.write_strokes(&ink);
            let (x, y) = button_center(Button::Colors);
            d.send(Input::TouchDown { id: 1, x, y });
            assert_eq!(d.app.ui.highlight, Some(v), "INP-21: {what}: the finger press");
            assert!(d.app.ui.pending_ink.is_some(), "INP-21: {what}: the finger press");
            d.send(Input::TouchUp { id: 1, x, y });
            read_ok(&d, e[0], "the release of a control");
            assert_eq!(d.app.ui.menu.panel(), Some(Panel::Colors), "{what}");
        }
    }
}

#[test]
fn a_tap_on_a_number_with_ink_in_a_different_cell_uses_the_highlight_at_the_press() {
    // HLT-3, HLT-9, INP-12, HLT-6: the press reads the ink, and the read removes the highlight v. At the
    // lift, a tap on v gives no highlight and a tap on w gives w. A "?" from that read shows with w.
    for (o, right_handed) in LAYOUTS {
        let _layout = InLayout::set(o, right_handed);
        let what = format!("{o:?}, right-handed {right_handed}");
        let mut d = Driver::new(604);
        let ((a, v), (b, w)) = two_numbers(&d.app.game);
        let e = empty_cells(&d);

        highlight_number(&mut d, a);
        d.write_strokes(&scribble_ink(e[0]));
        let (x, y) = micro_center(a, 5);
        d.send(Input::PenDown { x, y, tool: Tool::Pen });
        assert_eq!(d.app.ui.highlight, None, "HLT-9: {what}: the read at the press");
        assert_eq!(d.app.ui.question.map(|q| q.0), Some(e[0]), "INP-12: {what}");
        d.send(Input::PenUp { x, y });
        assert_eq!(d.app.ui.highlight, None, "HLT-3, HLT-9: {what}: a tap on v gives no highlight");
        d.wait(OVERLAY_MS);

        highlight_number(&mut d, a);
        d.write_strokes(&scribble_ink(e[0]));
        let (x, y) = micro_center(b, 5);
        d.send(Input::PenDown { x, y, tool: Tool::Pen });
        assert_eq!(d.app.ui.highlight, None, "HLT-9: {what}: the read at the press");
        d.send(Input::PenUp { x, y });
        assert_eq!(d.app.ui.highlight, Some(w), "HLT-3, HLT-9: {what}: a tap on w gives w");
        assert_eq!(d.app.ui.question.map(|q| q.0), Some(e[0]), "HLT-9: {what}: the \"?\" shows with w");
        assert_ne!(v, w);
        check(&d);
    }
}

#[test]
fn a_new_game_a_completed_puzzle_and_the_update_dialog_remove_a_highlight() {
    // HLT-1, HLT-2 items 5 and 7, CMP-3, UPD-10: a new game and a completed puzzle. The dialog removes the
    // highlight, and it does not come back after "Later". While the dialog shows, a tap on a number makes
    // no highlight.
    for (o, right_handed) in LAYOUTS {
        let _layout = InLayout::set(o, right_handed);
        let what = format!("{o:?}, right-handed {right_handed}");

        let mut d = Driver::new(605);
        let given = given_cell(&d.app.game);
        d.button(Button::Menu);
        highlight_number(&mut d, given);
        d.button(Button::NewGame);
        assert!(d.app.ui.highlight.is_some(), "HLT-4: {what}: the New game panel keeps the highlight");
        let seed = d.app.game.seed;
        d.finger_tap(layout::ng_panel().x0 as f32 + 100.0, (layout::tb_bottom_y() + 56) as f32);
        assert_ne!(d.app.game.seed, seed, "{what}: a new game");
        assert_eq!(d.app.ui.highlight, None, "HLT-2: {what}: a new game");

        let mut d = Driver::new(606);
        let given = given_cell(&d.app.game);
        highlight_number(&mut d, given);
        let empties = empty_cells(&d);
        let (&last, rest) = empties.split_last().unwrap();
        for &c in rest {
            let v = solution_at(&d.app.game, c);
            d.app.game.add_number(c, v);
        }
        let ink = digit_ink(last, solution_at(&d.app.game, last));
        d.write_strokes(&ink);
        assert!(d.app.ui.highlight.is_some(), "HLT-9: {what}: the highlight stays with the ink");
        d.wait(d.app.pause_ms);
        assert!(d.app.game.completed, "{what}: the read completes the puzzle");
        assert_eq!(d.app.ui.highlight, None, "HLT-1, CMP-3: {what}");
        // CMP-3, INP-16: a tap and a stroke on a complete game change nothing, and no timer runs.
        let (x, y) = micro_center(given, 5);
        d.pen_tap(x, y);
        assert_eq!(d.app.ui.highlight, None, "CMP-3: {what}: a tap on a complete game");
        pen_stroke(&mut d, Tool::Pen, (x, y), micro_center(last, 5));
        assert_eq!(d.app.ui.highlight, None, "CMP-3: {what}: a stroke on a complete game");
        assert!(!d.app.is_writing() && d.app.live_stroke.is_none() && d.app.ui.pending_ink.is_none(), "CMP-3, INP-16: {what}");
        assert_eq!(d.app.next_deadline(), None, "INP-16: {what}: a timer runs on a complete game");
        check(&d);

        let mut d = Driver::new(607);
        let given = given_cell(&d.app.game);
        let v = highlight_number(&mut d, given);
        show_update_dialog(&mut d);
        assert_eq!(d.app.ui.highlight, None, "HLT-2: {what}: the dialog");
        d.tap_micro(given, 5);
        assert_eq!(d.app.ui.highlight, None, "UPD-10: {what}: a tap under the dialog");
        let (x, y) = pixel_center(layout::dialog_cancel());
        d.finger_tap(x, y);
        assert!(!d.app.update.dialog_shown(), "{what}: Later");
        assert_eq!(d.app.ui.highlight, None, "HLT-2: {what}: no highlight after Later");
        // After the dialog, a tap makes a highlight again.
        d.tap_micro(given, 5);
        assert_eq!(d.app.ui.highlight, Some(v), "HLT-3: {what}");
    }
}

#[test]
fn leaving_the_board_removes_a_highlight_and_it_does_not_come_back() {
    // HLT-2 item 6, HLT-4: Help and Statistics, also with the "< Back" of each. All layouts.
    for (o, right_handed) in LAYOUTS {
        let _layout = InLayout::set(o, right_handed);
        for button in [Button::Help, Button::Statistics] {
            let what = format!("{o:?}, right-handed {right_handed}, {button:?}");
            let mut d = Driver::new(608);
            let given = given_cell(&d.app.game);
            d.button(Button::Menu);
            highlight_number(&mut d, given);
            d.button(button);
            assert_ne!(d.app.screen, Screen::Game, "{what}");
            assert_eq!(d.app.ui.highlight, None, "HLT-4: {what}");
            let (x, y) = pixel_center(layout::back_button());
            d.finger_tap(x, y);
            assert_eq!(d.app.screen, Screen::Game, "{what}");
            assert_eq!(d.app.ui.highlight, None, "HLT-4: {what}: the highlight does not come back");
            check(&d);
        }
    }
}

#[test]
fn turning_off_the_switch_removes_a_highlight_and_a_tap_then_makes_none() {
    // HLT-6, NAV-14: the panel stays open, the highlight goes at the release, and a redraw occurs. A tap
    // on a number makes no highlight, but it still removes the x and the "?". Turning the switch on does
    // not bring the highlight back. The other switches keep the highlight (HLT-7).
    for (o, right_handed) in LAYOUTS {
        let _layout = InLayout::set(o, right_handed);
        let what = format!("{o:?}, right-handed {right_handed}");
        let mut d = Driver::new(609);
        let given = given_cell(&d.app.game);
        d.button(Button::Menu);
        let v = highlight_number(&mut d, given);
        d.button(Button::Settings);
        for s in [Switch::HideMarksAutomatically, Switch::ColorCompletedGroups] {
            let (x, y) = switch_point(s);
            for _ in 0..2 {
                d.finger_tap(x, y);
                assert_eq!(d.app.ui.highlight, Some(v), "HLT-7, NAV-14: {what}: the switch {s:?}");
            }
        }
        let (x, y) = switch_point(Switch::HighlightNumbers);
        d.app.redraw = false;
        d.send(Input::TouchDown { id: 1, x, y });
        assert_eq!(d.app.ui.highlight, Some(v), "HLT-6: {what}: the press");
        d.send(Input::TouchUp { id: 1, x, y });
        assert_eq!(d.app.ui.highlight, None, "HLT-6: {what}: the release");
        assert!(d.app.redraw, "HLT-6: {what}: the redraw");
        assert!(!d.app.settings.highlight_on, "{what}");
        assert_eq!(d.app.ui.menu.panel(), Some(Panel::Settings), "HLT-6, NAV-14: {what}: the panel stays open");
        // The switch on: no highlight comes back.
        d.finger_tap(x, y);
        assert_eq!(d.app.ui.highlight, None, "HLT-6: {what}: the switch on again");
        d.finger_tap(x, y);
        d.button(Button::Settings);

        // The switch off: a tap on a number makes no highlight, but it removes the x and the "?".
        let (cell, digit) = refused_pair(&d.app.game);
        d.tap_micro(cell, digit);
        assert!(d.app.ui.refused.is_some(), "{what}: the fixture has the x");
        d.tap_micro(given, 5);
        assert_eq!(d.app.ui.highlight, None, "HLT-6: {what}");
        assert_eq!(d.app.ui.refused, None, "MRK-15: {what}");
        let e = empty_cells(&d);
        d.write_strokes(&scribble_ink(e[0]));
        d.wait(d.app.pause_ms);
        assert!(d.app.ui.question.is_some(), "{what}: the fixture has the \"?\"");
        d.tap_micro(given, 5);
        assert_eq!(d.app.ui.highlight, None, "HLT-6: {what}");
        assert_eq!(d.app.ui.question, None, "QST-4: {what}");
        check(&d);
    }
}

#[test]
fn no_other_input_changes_a_highlight() {
    // HLT-4, HLT-7, NAV-11, NAV-12, NAV-13, INP-1, INP-15: all layouts. A finger, a second finger, the menu,
    // the panels and their options, Undo, Redo and Restore, the eraser, a pen press off the board, a pen
    // press on the board with a panel, a turn, a suspension and the time keep the highlight.
    for (o, right_handed) in LAYOUTS {
        let _layout = InLayout::set(o, right_handed);
        let what = format!("{o:?}, right-handed {right_handed}");
        let mut d = Driver::new(610);
        let given = given_cell(&d.app.game);
        let e = empty_cells(&d);
        // The fixture has an entry and a mark, thus Undo, Redo and the eraser do an action.
        d.write(e[0], solution_at(&d.app.game, e[0]));
        let v = highlight_number(&mut d, given);
        let keep = |d: &Driver, why: &str| {
            assert_eq!(d.app.ui.highlight, Some(v), "HLT-7: {what}: {why}");
            check(d);
        };

        // Fingers on the board.
        for cell in [given, e[0], e[1]] {
            let (x, y) = micro_center(cell, 5);
            d.finger_tap(x, y);
            keep(&d, "a finger on the board");
        }
        let (x, y) = micro_center(e[1], 5);
        d.send(Input::TouchDown { id: 1, x, y });
        d.send(Input::TouchDown { id: 2, x: x + 200.0, y });
        keep(&d, "a second finger");
        d.send(Input::TouchUp { id: 2, x: x + 200.0, y });
        d.send(Input::TouchUp { id: 1, x, y });
        keep(&d, "the release of two fingers");
        d.swipe(true);
        d.swipe(false);
        keep(&d, "a swipe");
        d.wait(OVERLAY_MS + WRITING_IDLE_MS + 500);
        keep(&d, "the time");

        // A finger off the board.
        let (x, y) = off_board_on_no_control();
        d.finger_tap(x, y);
        keep(&d, "a finger off the board");

        // The pen off the board, on no control.
        d.pen_tap(x, y);
        keep(&d, "a pen press off the board");
        d.send(Input::PenDown { x, y, tool: Tool::Eraser });
        d.send(Input::PenUp { x, y });
        keep(&d, "an eraser press off the board");

        // The menu, with a finger and with the pen.
        d.button(Button::Menu);
        keep(&d, "the menu opens");
        d.button(Button::Menu);
        keep(&d, "the menu closes");
        let (x, y) = button_center(Button::Menu);
        d.pen_tap(x, y);
        keep(&d, "the pen opens the menu");

        // The panels and their options.
        for panel in [Button::Colors, Button::Settings, Button::Checkpoints, Button::NewGame] {
            d.button(panel);
            assert!(d.app.ui.menu.panel().is_some(), "{what}: {panel:?}");
            keep(&d, "a panel opens");
            d.button(panel);
            keep(&d, "a panel closes");
            let (x, y) = button_center(panel);
            d.pen_tap(x, y);
            keep(&d, "the pen opens a panel");

            // NAV-11, HLT-7: a finger and a pen on the board only close the panel, also if the pen moves.
            for stroke in [false, true] {
                let (x, y) = micro_center(e[2], 5);
                let before = state(&d);
                if stroke {
                    let mut last = (x, y);
                    d.send(Input::PenDown { x, y, tool: Tool::Pen });
                    assert!(d.app.live_stroke.is_none(), "NAV-11: {what}: no ink under a panel");
                    for k in 1..5 {
                        last = (x + 40.0 * k as f32, y);
                        d.send(Input::PenMove { x: last.0, y });
                        assert!(d.app.live_stroke.is_none() && d.app.ui.pending_ink.is_none(), "NAV-11: {what}: no ink under a panel");
                    }
                    d.send(Input::PenUp { x: last.0, y });
                } else {
                    d.finger_tap(x, y);
                }
                if d.app.ui.menu.panel().is_none() {
                    assert_eq!(state(&d), before, "NAV-11: {what}: {panel:?}");
                }
                keep(&d, "a press on the board with a panel");
                if d.app.ui.menu.panel().is_none() {
                    d.button(panel);
                }
            }
            if d.app.ui.menu.panel().is_some() {
                d.button(panel);
            }
        }

        // The options of the panels.
        d.button(Button::Colors);
        let (x, y) = color_point(Section::Clash, Color::Blue);
        d.finger_tap(x, y);
        keep(&d, "a color");
        d.pen_tap(x, y);
        keep(&d, "a color with the pen");
        d.button(Button::Colors);
        d.button(Button::NewGame);
        let x = d.app.ng_stops[1];
        d.finger_tap(x, layout::ng_slider_y() as f32);
        keep(&d, "the slider");
        d.button(Button::NewGame);

        // Undo, Redo and Restore.
        d.button(Button::Undo);
        assert_eq!(d.app.game.value_at(e[0]), 0, "{what}: the undo");
        keep(&d, "Undo");
        d.button(Button::Redo);
        assert_ne!(d.app.game.value_at(e[0]), 0, "{what}: the redo");
        keep(&d, "Redo");
        d.button(Button::Checkpoints);
        let (x, y) = exact_center(layout::ckp_save_row());
        d.finger_tap(x, y);
        assert_eq!(d.app.game.checkpoints.len(), 1, "{what}: the save");
        keep(&d, "Save checkpoint");
        d.button(Button::Checkpoints);
        d.button(Button::Undo);
        d.button(Button::Checkpoints);
        let (x, y) = exact_center(layout::ckp_button(0));
        d.finger_tap(x, y);
        keep(&d, "Restore");
        if d.app.ui.menu.panel().is_some() {
            d.button(Button::Checkpoints);
        }
        d.button(Button::Menu);

        // The eraser: on an entry, a mark, an empty cell, a starting number, and a stroke across cells.
        assert_ne!(d.app.game.value_at(e[0]), 0, "{what}: the entry is back");
        d.erase(e[0]);
        assert_eq!(d.app.game.value_at(e[0]), 0, "{what}: the eraser");
        keep(&d, "the eraser on an entry");
        d.erase(e[1]);
        keep(&d, "the eraser on an empty cell");
        d.erase(given);
        keep(&d, "the eraser on a starting number");
        pen_stroke(&mut d, Tool::Eraser, micro_center(e[1], 1), micro_center(e[3], 9));
        keep(&d, "an eraser stroke");

        // The eraser on waiting ink in its own cell: the ink goes, and the app does not read it.
        let ink = digit_ink(e[1], solution_at(&d.app.game, e[1]));
        d.write_strokes(&ink);
        keep(&d, "the ink waits");
        d.erase(e[1]);
        assert!(d.app.ui.pending_ink.is_none(), "INP-8: {what}");
        assert_eq!(d.app.game.value_at(e[1]), 0, "INP-8: {what}");
        keep(&d, "the eraser on waiting ink in its own cell");

        // A turn and a suspension, also on waiting ink: the ink goes, and the app does not read it.
        for waiting in [false, true] {
            if waiting {
                d.write_strokes(&digit_ink(e[1], solution_at(&d.app.game, e[1])));
            }
            let other = if o == layout::Orientation::Portrait { layout::Orientation::Landscape } else { layout::Orientation::Portrait };
            layout::set_orientation(other);
            d.app.rotate(App::placeholder_ng_stops());
            assert!(d.app.ui.pending_ink.is_none(), "NAV-13: {what}");
            keep(&d, "a turn");
            layout::set_orientation(o);
            d.app.rotate(App::placeholder_ng_stops());
            keep(&d, "a turn back");
            if waiting {
                d.write_strokes(&digit_ink(e[1], solution_at(&d.app.game, e[1])));
            }
            d.app.suspend();
            assert!(d.app.ui.pending_ink.is_none(), "PER-6: {what}");
            keep(&d, "a suspension");
            d.now += 60_000;
            d.app.tick(d.now);
            keep(&d, "the start after a suspension");
        }
    }
}

#[test]
fn a_highlight_stays_when_undo_removes_the_last_number_of_its_digit() {
    // HLT-8, HLT-3, HLT-5: a puzzle where only an entry has the digit. Undo and the Restore of a checkpoint
    // keep the highlight. It shows again when the digit comes back. A tap on that digit then removes it.
    // A generator puzzle can have no starting number of one digit: this seed has one.
    let game = new_game(6, 2);
    let digit = (1..=9u8).find(|&v| cells().all(|c| game.value_at(c) != v)).expect("the seed has a digit with no starting number");
    for (o, right_handed) in LAYOUTS {
        let _layout = InLayout::set(o, right_handed);
        let what = format!("{o:?}, right-handed {right_handed}");
        let mut d = Driver::from(app_with_game(game.clone(), START_TIME, 3));
        assert_eq!(d.app.game, game, "the app uses the game");
        let cell = cells().find(|&c| d.app.game.solution[c.0 as usize][c.1 as usize] == digit).unwrap();
        d.write(cell, digit);
        assert_eq!(d.app.game.value_at(cell), digit, "the fixture has the entry");
        d.tap_micro(cell, 5);
        assert_eq!(d.app.ui.highlight, Some(digit), "HLT-3: {what}");
        d.button(Button::Menu);

        d.button(Button::Undo);
        assert!(cells().all(|c| d.app.game.value_at(c) != digit), "{what}: no cell has the digit");
        assert_eq!(d.app.ui.highlight, Some(digit), "HLT-8: {what}: Undo");
        check(&d);
        d.button(Button::Redo);
        assert_eq!(d.app.ui.highlight, Some(digit), "HLT-8: {what}: Redo");
        d.button(Button::Undo);

        // The digit comes back by Redo: a tap on it removes the highlight.
        d.button(Button::Redo);
        assert_eq!(d.app.game.value_at(cell), digit, "{what}: the digit is back");
        assert_eq!(d.app.ui.highlight, Some(digit), "HLT-8: {what}: the highlight shows again");
        d.tap_micro(cell, 2);
        assert_eq!(d.app.ui.highlight, None, "HLT-8, HLT-3: {what}: the tap on the digit");
        check(&d);

        // Restore: a checkpoint with no entry of the digit. The highlight stays with no cell.
        d.tap_micro(cell, 2);
        assert_eq!(d.app.ui.highlight, Some(digit), "{what}");
        d.button(Button::Undo);
        d.button(Button::Checkpoints);
        let (x, y) = exact_center(layout::ckp_save_row());
        d.finger_tap(x, y);
        assert_eq!(d.app.game.checkpoints.len(), 1, "{what}: the save");
        d.button(Button::Checkpoints);
        d.button(Button::Redo);
        assert_eq!(d.app.game.value_at(cell), digit, "{what}");
        d.button(Button::Checkpoints);
        let (x, y) = exact_center(layout::ckp_button(0));
        d.finger_tap(x, y);
        assert!(cells().all(|c| d.app.game.value_at(c) != digit), "{what}: the Restore removes the digit");
        assert_eq!(d.app.ui.highlight, Some(digit), "HLT-8: {what}: Restore");
        check(&d);
    }
}

#[test]
fn a_touch_less_than_150_ms_after_a_pen_lift_has_no_effect() {
    // INP-20, QST-4, MRK-15, INP-15: all layouts, at each limit, after a lift of the pen and of the eraser.
    // The press, a movement and a release that comes later do nothing. At 150 ms, the touch operates.
    assert_eq!(HAND_TOUCH_MS, 150, "INP-20");
    for (o, right_handed) in LAYOUTS {
        let _layout = InLayout::set(o, right_handed);
        for tool in [Tool::Pen, Tool::Eraser] {
            for (ms, button) in [(0, Button::Colors), (10, Button::Undo), (97, Button::Menu), (149, Button::Colors), (149, Button::Undo), (149, Button::Menu), (149, Button::Help), (149, Button::Close)] {
                let what = format!("{o:?}, right-handed {right_handed}, {tool:?}, {ms} ms, {button:?}");
                let mut d = Driver::new(611);
                let (cell, mark) = free_spot(&d);
                d.tap_micro(cell, mark);
                d.button(Button::Menu);
                let scribble = empty_cells(&d).into_iter().find(|&c| c != cell).unwrap();
                d.write_strokes(&scribble_ink(scribble));
                if tool == Tool::Pen {
                    // The press of a refused tap reads the ink: the "?". The lift shows the x.
                    let (r, digit) = refused_pairs(&d.app.game).into_iter().find(|p| p.0 != scribble).unwrap();
                    d.tap_micro(r, digit);
                    assert!(d.app.ui.refused.is_some(), "{what}: the fixture has the x");
                } else {
                    let (x, y) = off_board_on_no_control();
                    d.send(Input::PenDown { x, y, tool });
                    d.send(Input::PenUp { x, y });
                }
                assert!(d.app.ui.question.is_some(), "{what}: the fixture has the \"?\"");
                let (question, refused, menu, board) = (d.app.ui.question, d.app.ui.refused, d.app.ui.menu, state(&d));
                let (x, y) = button_center(button);
                d.touch_down_after_lift(ms, x, y);
                let same = |d: &Driver, event: &str| {
                    assert_eq!(d.app.ui.question, question, "QST-4, INP-20: {what}: {event}");
                    assert_eq!(d.app.ui.refused, refused, "MRK-15, INP-20: {what}: {event}");
                    assert_eq!((d.app.ui.menu, d.app.screen, d.app.quit), (menu, Screen::Game, false), "INP-15, INP-20: {what}: {event}");
                    assert_eq!(state(d), board, "INP-20: {what}: {event}");
                    check(d);
                };
                same(&d, "the press");
                d.send_at_once(Input::TouchMove { id: 1, x: x + 1.0, y });
                same(&d, "the movement");
                d.send_at_once(Input::TouchUp { id: 1, x, y });
                same(&d, "the release");
            }
        }

        let what = format!("{o:?}, right-handed {right_handed}");

        // The release comes long after the press: it still has no effect.
        let mut d = Driver::new(612);
        d.button(Button::Menu);
        let (x, y) = off_board_on_no_control();
        d.pen_tap(x, y);
        let (bx, by) = button_center(Button::Colors);
        d.touch_down_after_lift(149, bx, by);
        d.wait(2000);
        d.send(Input::TouchMove { id: 1, x: bx, y: by });
        d.send(Input::TouchUp { id: 1, x: bx, y: by });
        assert_eq!(d.app.ui.menu.panel(), None, "INP-20: {what}: a late release");

        // 150 ms: the touch operates. A touch that comes after a hand touch operates also.
        let mut d = Driver::new(613);
        d.button(Button::Menu);
        let (cell, digit) = refused_pair(&d.app.game);
        let scribble = empty_cells(&d).into_iter().find(|&c| c != cell).unwrap();
        d.write_strokes(&scribble_ink(scribble));
        d.tap_micro(cell, digit);
        assert!(d.app.ui.refused.is_some() && d.app.ui.question.is_some(), "{what}: the fixture has the x and the \"?\"");
        d.touch_down_after_lift(150, bx, by);
        assert_eq!(d.app.ui.refused, None, "MRK-15, INP-20: {what}: the press at 150 ms removes the x");
        assert_eq!(d.app.ui.question, None, "QST-4, INP-20: {what}: the press at 150 ms removes the \"?\"");
        d.send(Input::TouchUp { id: 1, x: bx, y: by });
        assert_eq!(d.app.ui.menu.panel(), Some(Panel::Colors), "INP-20: {what}: the touch at 150 ms operates");
        d.button(Button::Colors);

        // A slider, Start new game and a switch do not operate for a hand touch.
        d.button(Button::NewGame);
        let slider = d.app.ui.slider;
        let (here, to) = (d.app.ng_stops[slider as usize], d.app.ng_stops[(slider as usize + 1) % 3]);
        d.pen_tap(here, layout::ng_slider_y() as f32);
        d.hand_tap(to, layout::ng_slider_y() as f32);
        assert_eq!(d.app.ui.slider, slider, "INP-20: {what}: the slider");
        let start = (layout::ng_panel().x0 as f32 + 100.0, (layout::tb_bottom_y() + 56) as f32);
        let seed = d.app.game.seed;
        d.pen_tap(here, layout::ng_slider_y() as f32);
        d.hand_tap(start.0, start.1);
        assert_eq!(d.app.game.seed, seed, "INP-20: {what}: Start new game");
        d.button(Button::NewGame);

        d.button(Button::Settings);
        let settings = d.app.settings.clone();
        let (x, y) = switch_point(Switch::HighlightNumbers);
        d.pen_tap(x, y);
        let after_pen = d.app.settings.clone();
        assert_ne!(after_pen, settings, "{what}: the pen operates the switch");
        d.hand_tap(x, y);
        assert_eq!(d.app.settings, after_pen, "INP-20: {what}: the switch");

        // A hand touch on the other screens.
        let mut d = Driver::new(614);
        d.button(Button::Menu);
        d.button(Button::Help);
        let page = d.app.manual_page;
        let (nx, ny) = pixel_center(layout::next_arrow());
        d.pen_tap(nx, ny);
        let page_after_pen = d.app.manual_page;
        assert_eq!(page_after_pen, page + 1, "{what}: the pen operates the arrow");
        d.hand_tap(nx, ny);
        assert_eq!(d.app.manual_page, page_after_pen, "INP-20: {what}: the arrow");
        assert!(not_on_a_control(800.0, 1000.0), "{what}");
        d.pen_tap(800.0, 1000.0);
        d.hand_down(1200.0, 1000.0);
        d.send_at_once(Input::TouchMove { id: 1, x: 500.0, y: 1000.0 });
        d.send_at_once(Input::TouchUp { id: 1, x: 300.0, y: 1000.0 });
        assert_eq!(d.app.manual_page, page_after_pen, "INP-20: {what}: the swipe");
        d.pen_tap(800.0, 1000.0);
        let (bx, by) = pixel_center(layout::back_button());
        d.hand_tap(bx, by);
        assert_eq!(d.app.screen, Screen::Manual, "INP-20: {what}: < Back");

        // The update dialog: its buttons do not operate for a hand touch.
        let mut d = Driver::new(615);
        show_update_dialog(&mut d);
        d.pen_tap(5.0, 5.0);
        let (x, y) = pixel_center(layout::dialog_cancel());
        d.hand_tap(x, y);
        assert!(d.app.update.dialog_shown(), "INP-20: {what}: Later");
        d.pen_tap(5.0, 5.0);
        let (x, y) = pixel_center(layout::dialog_delete());
        d.hand_tap(x, y);
        assert!(d.app.update.dialog_shown() && !d.app.restart_ui, "INP-20: {what}: Restart");

        // A hand touch while a finger stays: a second finger, but it does not remove the x (INP-15).
        let mut d = Driver::new(616);
        let on_board = micro_center((4, 4), 5);
        d.send(Input::TouchDown { id: 1, x: on_board.0, y: on_board.1 });
        let (cell, digit) = refused_pair(&d.app.game);
        d.tap_micro(cell, digit);
        let x = d.app.ui.refused;
        assert!(x.is_some(), "{what}");
        d.send_at_once(Input::TouchDown { id: 2, x: on_board.0 + 100.0, y: on_board.1 });
        assert_eq!(d.app.ui.refused, x, "INP-20, MRK-15: {what}: a hand touch as a second finger");
        d.send_at_once(Input::TouchUp { id: 2, x: on_board.0 + 100.0, y: on_board.1 });
        d.send_at_once(Input::TouchUp { id: 1, x: on_board.0, y: on_board.1 });
        assert_eq!(d.app.ui.refused, x, "INP-20, MRK-15: {what}");
    }
}

#[test]
fn a_finger_press_does_not_read_the_ink_or_stop_the_writing() {
    // INP-1, INP-12, INP-15, INP-16, INP-21: on the board, on a control and off the board. The release of a
    // control on the control reads the ink and stops the timer. A release off the control does nothing.
    for (o, right_handed) in LAYOUTS {
        let _layout = InLayout::set(o, right_handed);
        let what = format!("{o:?}, right-handed {right_handed}");
        let mut d = Driver::new(617);
        d.button(Button::Menu);
        let e = empty_cells(&d);
        let v = solution_at(&d.app.game, e[0]);
        let ink = digit_ink(e[0], v);
        d.write_strokes(&ink);
        let lift = d.now;
        let waits = |d: &Driver, why: &str| {
            assert!(d.app.ui.pending_ink.is_some(), "INP-21: {what}: {why}");
            assert!(d.app.is_writing(), "INP-21: {what}: {why}");
            assert_eq!(d.app.game.value_at(e[0]), 0, "INP-21: {what}: {why}");
        };

        // The press on the board, off the board and on a control, with no release on a control.
        let (ox, oy) = off_board_on_no_control();
        for (x, y) in [micro_center(e[1], 5), micro_center(given_cell(&d.app.game), 5), (ox, oy)] {
            d.send(Input::TouchDown { id: 1, x, y });
            waits(&d, "a finger press");
            d.send(Input::TouchUp { id: 1, x, y });
            waits(&d, "a finger release that is not on a control");
        }
        // The press on a control and a release off the control: no action, no read.
        let (bx, by) = button_center(Button::Colors);
        d.send(Input::TouchDown { id: 1, x: bx, y: by });
        waits(&d, "a finger press on a control");
        d.send(Input::TouchUp { id: 1, x: ox, y: oy });
        waits(&d, "a release off the control");
        assert_eq!(d.app.ui.menu.panel(), None, "INP-21: {what}: a release off the control");
        // INP-5: the time of the pause does not change.
        assert_eq!(d.app.next_deadline(), Some(lift + RECOGNITION_PAUSE_MS), "INP-5: {what}");

        // The release on the control reads the ink and does the action.
        d.send(Input::TouchDown { id: 1, x: bx, y: by });
        waits(&d, "a finger press on a control");
        d.send(Input::TouchUp { id: 1, x: bx, y: by });
        assert!(d.app.ui.pending_ink.is_none(), "INP-21: {what}: the release reads the ink");
        assert_eq!(d.app.game.value_at(e[0]), v, "INP-21: {what}: the release reads the ink");
        assert!(!d.app.is_writing(), "INP-21: {what}: the release stops the timer");
        assert_eq!(d.app.ui.menu.panel(), Some(Panel::Colors), "{what}: the action");
        d.button(Button::Colors);

        // The timer alone, with no ink: a finger press does not stop it, the release of a control does.
        let given = given_cell(&d.app.game);
        pen_stroke(&mut d, Tool::Pen, micro_center(given, 1), micro_center(given, 9));
        assert!(d.app.is_writing(), "INP-16: {what}: the timer");
        d.send(Input::TouchDown { id: 1, x: bx, y: by });
        assert!(d.app.is_writing(), "INP-16, INP-21: {what}: the finger press");
        d.send(Input::TouchUp { id: 1, x: bx, y: by });
        assert!(!d.app.is_writing(), "INP-21: {what}: the release");
        d.button(Button::Colors);
        check(&d);
    }
}

#[test]
fn a_pen_touch_from_a_filled_cell_shows_no_ink_and_starts_no_pen_area() {
    // INP-22, INP-2, INP-7, HLT-9: a tap and a stroke from a starting number and from an entry, also while
    // the player writes. A stroke from an empty cell across a filled cell shows ink. All layouts.
    for (o, right_handed) in LAYOUTS {
        let _layout = InLayout::set(o, right_handed);
        let what = format!("{o:?}, right-handed {right_handed}");
        let mut d = Driver::new(618);
        let e = empty_cells(&d);
        d.write(e[0], solution_at(&d.app.game, e[0]));
        d.wait(WRITING_IDLE_MS);
        let given = given_cell(&d.app.game);
        for (from, writes) in [(given, false), (e[0], false), (given, true), (e[0], true)] {
            let what = format!("{what}, the cell {from:?}, the player writes {writes}");
            if writes {
                // A stroke on a filled cell starts the timer, with no ink.
                pen_stroke(&mut d, Tool::Pen, micro_center(given, 1), micro_center(given, 9));
                assert!(d.app.is_writing(), "{what}");
            }
            let before = state(&d);
            // The tap.
            let (x, y) = micro_center(from, 5);
            d.send(Input::PenDown { x, y, tool: Tool::Pen });
            assert!(d.app.live_stroke.is_none(), "INP-22: {what}: the ink of a tap");
            assert!(d.app.writing_stroke().is_none(), "INP-22: {what}: the pen area of a tap");
            assert!(d.app.is_writing(), "HLT-9: {what}: the redraw of a tap waits for its lift");
            d.send(Input::PenUp { x, y });
            assert!(d.app.ui.pending_ink.is_none() && d.app.live_stroke.is_none(), "INP-22: {what}");
            assert!(!d.app.is_writing(), "INP-16: {what}: a tap on a number does not write");

            // The stroke, in the micro-cell and then out of the cell.
            if writes {
                pen_stroke(&mut d, Tool::Pen, micro_center(given, 1), micro_center(given, 9));
            }
            d.send(Input::PenDown { x, y, tool: Tool::Pen });
            for k in 1..6 {
                d.send(Input::PenMove { x: x + 50.0 * k as f32, y: y + 30.0 * k as f32 });
                assert!(d.app.live_stroke.is_none(), "INP-22: {what}: the ink of a stroke");
                assert!(d.app.writing_stroke().is_none(), "INP-22: {what}: the pen area of a stroke");
                assert!(d.app.ui.pending_ink.is_none(), "INP-22: {what}: the ink of a stroke");
                check(&d);
            }
            d.send(Input::PenUp { x: x + 250.0, y: y + 150.0 });
            assert!(d.app.ui.pending_ink.is_none() && d.app.live_stroke.is_none(), "INP-22: {what}");
            assert_eq!(state(&d), before, "INP-7: {what}");
            d.wait(WRITING_IDLE_MS);
        }

        // A stroke from an empty cell across a filled cell shows ink, from the first cell.
        let (x0, y0) = micro_center(e[1], 5);
        let (x1, y1) = micro_center(given, 5);
        d.send(Input::PenDown { x: x0, y: y0, tool: Tool::Pen });
        assert!(d.app.live_stroke.is_some(), "INP-2: {what}: a press on an empty cell shows ink");
        let mut last = 0;
        for k in 1..=10 {
            d.send(Input::PenMove { x: x0 + (x1 - x0) * k as f32 / 10.0, y: y0 + (y1 - y0) * k as f32 / 10.0 });
            let n = d.app.live_stroke.as_ref().expect("the ink stays on a filled cell").len();
            assert!(n > last, "INP-22: {what}: the ink continues");
            last = n;
            assert!(d.app.writing_stroke().is_some() || layout::board_micro(x0, y0) == layout::board_micro(x0 + (x1 - x0) * k as f32 / 10.0, y0 + (y1 - y0) * k as f32 / 10.0), "INP-2: {what}");
        }
        d.send(Input::PenUp { x: x1, y: y1 });
        assert_eq!(d.app.ui.pending_ink.as_ref().map(|i| i.cell), Some(e[1]), "INP-22, INP-12: {what}: the ink belongs to the cell where the touch started");
        d.wait(d.app.pause_ms + OVERLAY_MS);
        check(&d);
    }
}

#[test]
fn a_finger_redo_that_fills_the_cell_of_the_touch_stops_the_pen_area_and_the_ink() {
    // INP-22, INP-24, HLT-3: the pen is down on an empty cell while the player writes, thus the area starts
    // at the press. A finger Redo fills that cell. The area stops at once. If the pen then stays in the
    // micro-cell, it is a tap on a number. If the pen leaves it, the app removes the ink.
    for (o, right_handed) in LAYOUTS {
        let _layout = InLayout::set(o, right_handed);
        for stroke in [false, true] {
            let what = format!("{o:?}, right-handed {right_handed}, stroke {stroke}");
            let mut d = Driver::new(619);
            let e = empty_cells(&d);
            let v = solution_at(&d.app.game, e[0]);
            d.write(e[0], v);
            d.button(Button::Menu);
            d.button(Button::Undo);
            assert_eq!(d.app.game.value_at(e[0]), 0, "{what}: the fixture has the redo");
            let given = given_cell(&d.app.game);
            pen_stroke(&mut d, Tool::Pen, micro_center(given, 1), micro_center(given, 9));
            assert!(d.app.is_writing(), "{what}: the player writes");

            let (x, y) = micro_center(e[0], 5);
            d.send(Input::PenDown { x, y, tool: Tool::Pen });
            assert!(d.app.writing_stroke().is_some(), "INP-24: {what}: the area starts at the press");
            let (bx, by) = button_center(Button::Redo);
            d.send(Input::TouchDown { id: 1, x: bx, y: by });
            assert!(d.app.writing_stroke().is_some(), "INP-21: {what}: the finger press changes nothing");
            d.send(Input::TouchUp { id: 1, x: bx, y: by });
            assert_eq!(d.app.game.value_at(e[0]), v, "{what}: the redo");
            assert!(d.app.writing_stroke().is_none(), "INP-22: {what}: the area on a filled cell");
            check(&d);
            if stroke {
                d.send(Input::PenMove { x: x + 120.0, y: y + 120.0 });
                assert!(d.app.writing_stroke().is_none(), "INP-22: {what}: the area on a filled cell");
                assert!(d.app.ui.pending_ink.is_none(), "INP-22: {what}");
                d.send(Input::PenUp { x: x + 120.0, y: y + 120.0 });
                assert_eq!(d.app.ui.highlight, None, "{what}: the stroke is not a tap");
            } else {
                d.send(Input::PenUp { x, y });
                assert_eq!(d.app.ui.highlight, Some(v), "HLT-3: {what}: the tap is on a number now");
            }
            assert!(d.app.live_stroke.is_none() && d.app.ui.pending_ink.is_none(), "INP-22: {what}");
            check(&d);
        }
    }
}

#[test]
fn a_finger_redo_that_completes_the_game_during_a_stroke_starts_no_timer() {
    // INP-16, CMP-3: the timer runs only on a game that is not complete. The pen is down on a filled cell or
    // on an empty cell, and a finger Redo completes the game. The lift starts no timer, and no ink stays.
    for (o, right_handed) in LAYOUTS {
        let _layout = InLayout::set(o, right_handed);
        for on_filled in [true, false] {
            let what = format!("{o:?}, right-handed {right_handed}, on a filled cell {on_filled}");
            let (g, x) = last_cell_game(109, false);
            let mut d = Driver::from(app_with_game(g, START_TIME, 3));
            d.button(Button::Menu);
            let from = if on_filled { given_cell(&d.app.game) } else { x };
            let (px, py) = micro_center(from, 5);
            d.send(Input::PenDown { x: px, y: py, tool: Tool::Pen });
            d.send(Input::PenMove { x: px + 60.0, y: py + 60.0 });
            let (bx, by) = button_center(Button::Redo);
            d.send(Input::TouchDown { id: 1, x: bx, y: by });
            d.send(Input::TouchUp { id: 1, x: bx, y: by });
            assert!(d.app.game.completed, "{what}: the redo completes the game");
            d.send(Input::PenMove { x: px + 90.0, y: py + 60.0 });
            d.send(Input::PenUp { x: px + 90.0, y: py + 60.0 });
            assert!(!d.app.is_writing(), "INP-16: {what}: a timer runs on a complete game");
            assert_eq!(d.app.next_deadline(), None, "INP-16: {what}");
            assert!(d.app.live_stroke.is_none() && d.app.ui.pending_ink.is_none(), "CMP-3: {what}");
            check(&d);
        }
    }
}

#[test]
fn the_pen_area_starts_at_each_pen_down_on_an_empty_cell() {
    // INP-2, INP-16, INP-24: each pen press on an empty cell starts the area at the pen down: the first
    // stroke of a session, a stroke while ink waits, a tap, and a press after the timer ended. A tap
    // starts no timer. A pen press that does not write stops the timer. A finger press does not.
    for (o, right_handed) in LAYOUTS {
        let _layout = InLayout::set(o, right_handed);
        let what = format!("{o:?}, right-handed {right_handed}");
        let mut d = Driver::new(620);
        let e = empty_cells(&d);
        let given = given_cell(&d.app.game);

        // The first stroke: the area from the pen down, also in the micro-cell.
        let (x, y) = micro_center(e[0], 5);
        d.send(Input::PenDown { x, y, tool: Tool::Pen });
        assert!(d.app.live_stroke.is_some() && d.app.pen_cell() == Some(e[0]), "INP-24: {what}: the pen down of the first stroke");
        d.send(Input::PenMove { x: x + 1.0, y: y + 1.0 });
        assert_eq!(d.app.pen_cell(), Some(e[0]), "INP-24: {what}: in the micro-cell");
        d.send(Input::PenMove { x: x + 60.0, y: y + 60.0 });
        assert!(d.app.writing_stroke().is_some(), "INP-2: {what}: the pen left the micro-cell");
        d.send(Input::PenUp { x: x + 60.0, y: y + 60.0 });
        assert!(d.app.is_writing() && d.app.pen_cell() == Some(e[0]), "INP-16: {what}: the area stays on the cell of the waiting ink");

        // Ink waits: a press in the same cell and in a different cell starts the area at the pen down.
        for cell in [e[0], e[1]] {
            let (x, y) = micro_center(cell, 5);
            assert!(d.app.is_writing(), "{what}");
            d.send(Input::PenDown { x, y, tool: Tool::Pen });
            assert_eq!(d.app.pen_cell(), Some(cell), "INP-24: {what}: the pen down on {cell:?}");
            d.send(Input::PenMove { x: x + 60.0, y: y + 60.0 });
            d.send(Input::PenUp { x: x + 60.0, y: y + 60.0 });
        }

        // A tap starts the area at the pen down. It stops the timer, and the next press starts the
        // area again at its pen down.
        let tap_cell = e[2];
        let (x, y) = micro_center(tap_cell, free_digits(&d, tap_cell)[0]);
        d.send(Input::PenDown { x, y, tool: Tool::Pen });
        assert_eq!(d.app.pen_cell(), Some(tap_cell), "INP-24: {what}: the tap");
        d.send(Input::PenUp { x, y });
        assert!(!d.app.is_writing() && d.app.pen_cell().is_none(), "INP-24: {what}: a tap starts no timer");
        let (x, y) = micro_center(e[3], 5);
        d.send(Input::PenDown { x, y, tool: Tool::Pen });
        assert_eq!(d.app.pen_cell(), Some(e[3]), "INP-24: {what}: the press after the tap");
        d.send(Input::PenUp { x, y });
        d.wait(d.app.pause_ms + OVERLAY_MS);

        // A press after the timer ended also starts the area at the pen down (INP-16: the timer is 1 s).
        for ms in [WRITING_IDLE_MS - 1, WRITING_IDLE_MS] {
            pen_stroke(&mut d, Tool::Pen, micro_center(given, 1), micro_center(given, 9));
            let (x, y) = micro_center(e[3], 5);
            d.wait(ms);
            assert_eq!(d.app.is_writing(), ms < WRITING_IDLE_MS, "INP-16: {what}: the timer {ms} ms after the last stroke");
            d.send(Input::PenDown { x, y, tool: Tool::Pen });
            assert_eq!(d.app.pen_cell(), Some(e[3]), "INP-24: {what}: the press {ms} ms after the last stroke");
            d.send(Input::PenUp { x, y });
            d.wait(WRITING_IDLE_MS + OVERLAY_MS);
        }

        // A press on a filled cell starts no area (INP-22).
        let (x, y) = micro_center(given, 5);
        d.send(Input::PenDown { x, y, tool: Tool::Pen });
        assert!(d.app.pen_cell().is_none() && d.app.writing_stroke().is_none(), "INP-22: {what}");
        d.send(Input::PenUp { x, y });

        // These stop the timer: a pen press off the board and the eraser (INP-16).
        pen_stroke(&mut d, Tool::Pen, micro_center(given, 1), micro_center(given, 9));
        let (ox, oy) = off_board_on_no_control();
        d.pen_tap(ox, oy);
        assert!(!d.app.is_writing(), "INP-16: {what}: a pen press off the board");
        pen_stroke(&mut d, Tool::Pen, micro_center(given, 1), micro_center(given, 9));
        d.erase(e[4]);
        assert!(!d.app.is_writing(), "INP-16: {what}: the eraser");
        check(&d);
    }
}

#[test]
fn the_recognition_pause_is_1_s_and_it_starts_at_the_pen_lift() {
    // INP-5, INP-16, INP-23: the time that the pen stays down does not count. A new stroke in the same cell
    // stops the pause, and the lift starts it again. The timer has the same value as the pause.
    assert_eq!(RECOGNITION_PAUSE_MS, 1000, "INP-23");
    assert_eq!(WRITING_IDLE_MS, RECOGNITION_PAUSE_MS, "INP-16");
    let mut d = Driver::new(621);
    assert_eq!(d.app.pause_ms, 1000, "INP-23: the app uses the value of the source");
    // The first empty cell whose ink leaves the micro-cell of the pen down at the middle point of the
    // first stroke: then the touch is writing (INP-2) while the pen stays down. The new game is
    // symmetric (DIF-9), thus the cell depends on its board.
    let (cell, ink) = empty_cells(&d)
        .into_iter()
        .map(|c| (c, digit_ink(c, solution_at(&d.app.game, c))))
        .find(|(_, ink)| {
            let s = &ink[0];
            layout::board_micro(s[0].0, s[0].1) != layout::board_micro(s[s.len() / 2].0, s[s.len() / 2].1)
        })
        .expect("an empty cell whose first stroke leaves its first micro-cell");
    let s = &ink[0];
    d.send(Input::PenDown { x: s[0].0, y: s[0].1, tool: Tool::Pen });
    d.send(Input::PenMove { x: s[s.len() / 2].0, y: s[s.len() / 2].1 });
    assert_eq!(d.app.ui.pending_ink.as_ref().map(|p| p.cell), Some(cell), "INP-2: the touch is writing");
    // The pen stays down for 3 s: no read.
    d.wait(3000);
    assert!(d.app.ui.pending_ink.is_some() && d.app.game.value_at(cell) == 0, "INP-5: the pause starts at the pen lift");
    assert_eq!(d.app.next_deadline(), None, "INP-5: no timer while the pen is down");
    d.send(Input::PenUp { x: s[s.len() - 1].0, y: s[s.len() - 1].1 });
    let lift = d.now;
    assert_eq!(d.app.next_deadline(), Some(lift + 1000), "INP-5, INP-16");
    d.wait(989);
    assert_eq!(d.app.game.value_at(cell), 0, "INP-5: 989 ms");
    assert!(d.app.is_writing(), "INP-16: 989 ms");

    // A new stroke in the same cell at 999 ms stops the pause (`Driver::send` adds 10 ms): the pen stays
    // down for 2 s, no read.
    d.send(Input::PenDown { x: s[0].0, y: s[0].1, tool: Tool::Pen });
    d.wait(2000);
    assert!(d.app.ui.pending_ink.is_some() && d.app.game.value_at(cell) == 0, "INP-5: a new stroke stops the pause");
    d.send(Input::PenUp { x: s[0].0 + 1.0, y: s[0].1 });
    let lift = d.now;
    assert_eq!(d.app.next_deadline(), Some(lift + 1000), "INP-5: the lift starts the pause again");
    d.wait(999);
    assert!(d.app.ui.pending_ink.is_some(), "INP-5: 999 ms after the second lift");
    d.wait(1);
    assert!(d.app.ui.pending_ink.is_none(), "INP-5: 1000 ms after the second lift");
    check(&d);
}

#[test]
fn the_pause_has_one_source_and_the_manifest_does_not_set_it() {
    // INP-23: the AppLoad manifest has no environment and does not give the pause. No source file reads an
    // environment variable for the pause.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest = std::fs::read_to_string(root.join("appload/sudoku/external.manifest.json")).unwrap();
    let json: serde_json::Value = serde_json::from_str(&manifest).unwrap();
    assert!(json.get("environment").is_none(), "INP-23: the manifest has an environment");
    assert!(!manifest.contains("PAUSE"), "INP-23: the manifest gives the pause");
    let mut files = vec![root.join("src")];
    let mut sources = 0;
    while let Some(path) = files.pop() {
        if path.is_dir() {
            files.extend(std::fs::read_dir(&path).unwrap().map(|e| e.unwrap().path()));
        } else if path.extension().is_some_and(|e| e == "rs") {
            sources += 1;
            let text = std::fs::read_to_string(&path).unwrap();
            assert!(!text.contains("RECOGNITION_PAUSE_MS\""), "INP-23: {path:?} reads the pause from the environment");
            assert!(!text.contains("PAUSE_MS\")"), "INP-23: {path:?} reads the pause from the environment");
        }
    }
    assert!(sources > 10, "the test did not read the source");
}
