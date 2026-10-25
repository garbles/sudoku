//! Tests of the behavior in REQUIREMENTS.md, with the input that a player gives. Each test gives the
//! rules of INVARIANTS.md that it examines. The debug build also checks all state rules after each
//! input (`invariants::check_app`).

mod common;

use common::*;
use sudoku::app::{App, Input, Panel, Screen, Tool, PER_PAGE};
use sudoku::game::{cells, mark_bit, peers, Cell};
use sudoku::layout::{self, Button};
use sudoku::settings::{Color, Settings};
use sudoku::store::{NewResult, ResultsDb};

/// The game without the clock.
fn board(g: &sudoku::game::Game) -> sudoku::game::Game {
    let mut g = g.clone();
    g.elapsed_ms = 0;
    g
}

fn empty_cell(app: &App) -> Cell {
    cells().find(|&c| !app.game.is_filled(c)).unwrap()
}

fn given_cell(app: &App) -> Cell {
    cells().find(|&c| app.game.is_given(c)).unwrap()
}

#[test]
fn first_start() {
    // DIF-2, NAV-1, PER-2
    let d = Driver::new(1);
    let a = &d.app;
    assert_eq!(a.screen, Screen::Game);
    assert_eq!(a.game.mode, 2);
    assert_eq!(a.settings, Settings::default());
    assert!(!a.ui.menu_open && a.ui.panel == Panel::None && a.ui.highlight.is_none());
    assert!(a.game.undo.is_empty() && a.game.redo.is_empty() && a.game.elapsed_ms == 0);
}

#[test]
fn tap_adds_and_removes_marks() {
    // INP-3, MRK-5, UND-1
    let mut d = Driver::new(2);
    let cell = empty_cell(&d.app);
    d.tap_micro(cell, 7);
    assert_eq!(d.app.game.marks_at(cell), mark_bit(7));
    d.tap_micro(cell, 3);
    d.tap_micro(cell, 7);
    assert_eq!(d.app.game.marks_at(cell), mark_bit(3));
    assert_eq!(d.app.game.undo.len(), 3);
}

#[test]
fn writing_adds_a_number_after_the_pause() {
    // INP-5, INP-6, MRK-4, BRD-4
    let mut d = Driver::new(3);
    let cell = empty_cell(&d.app);
    let v = d.app.game.solution[cell.0 as usize][cell.1 as usize];
    let peer = peers(cell).find(|&p| !d.app.game.is_filled(p)).unwrap();
    d.tap_micro(peer, v);
    d.tap_micro(cell, v);
    d.write_strokes(&digit_ink(cell, v));
    assert!(d.app.ui.pending_ink.is_some(), "the app waits for the pause");
    assert_eq!(d.app.game.value_at(cell), 0);
    d.wait(d.app.pause_ms + 10);
    assert!(d.app.ui.pending_ink.is_none());
    assert_eq!(d.app.game.value_at(cell), v);
    assert_eq!(d.app.game.marks_at(peer) & mark_bit(v), 0, "MRK-4: peer mark removed");
    assert_ne!(d.app.game.marks_at(cell) & mark_bit(v), 0, "the marks of the cell are hidden, not removed");
    // UND-4: undo puts the peer mark back.
    d.button(Button::Menu);
    d.button(Button::Undo);
    assert_eq!(d.app.game.value_at(cell), 0);
    assert_ne!(d.app.game.marks_at(peer) & mark_bit(v), 0);
}

#[test]
fn handwritten_digits_are_read_through_the_pen() {
    // INP-6: real handwriting of 5 writers, through the full pen input path.
    for v in 1..=9u8 {
        for writer in [3u8, 17, 29, 44, 58] {
            let mut d = Driver::new(40 + v as u64);
            let cell = empty_cell(&d.app);
            d.write_strokes(&digit_ink_by(cell, v, writer));
            d.wait(d.app.pause_ms + 10);
            assert_eq!(d.app.game.value_at(cell), v, "digit {v} of writer {writer}");
        }
    }
}

#[test]
fn strokes_with_a_short_lift_are_one_number() {
    // §5.1: a number with more than one stroke (4) is correct.
    let mut d = Driver::new(4);
    let cell = empty_cell(&d.app);
    let ink = digit_ink(cell, 4);
    assert_eq!(ink.len(), 2);
    d.write_strokes(&ink[..1]);
    d.wait(d.app.pause_ms / 2);
    assert!(d.app.ui.pending_ink.is_some());
    d.write_strokes(&ink[1..]);
    d.wait(d.app.pause_ms + 10);
    assert_eq!(d.app.game.value_at(cell), 4);
}

#[test]
fn unreadable_writing_shows_a_question_mark() {
    // QST-1, QST-2
    let mut d = Driver::new(5);
    let cell = empty_cell(&d.app);
    d.tap_micro(cell, 2);
    let before = d.app.game.clone();
    d.write_strokes(&scribble_ink(cell));
    d.wait(d.app.pause_ms + 10);
    assert_eq!(d.app.ui.question.map(|q| q.0), Some(cell));
    assert_eq!(board(&d.app.game), board(&before), "QST-2");
    d.wait(1000);
    assert!(d.app.ui.question.is_none());
    assert_eq!(board(&d.app.game), board(&before), "the cell is the same as before");
}

#[test]
fn writing_on_a_number_has_no_effect() {
    // INP-7, BRD-2, BRD-3
    let mut d = Driver::new(6);
    let given = given_cell(&d.app);
    let before = d.app.game.clone();
    d.write(given, if before.value_at(given) == 1 { 2 } else { 1 });
    assert_eq!(board(&d.app.game), board(&before));
    let cell = empty_cell(&d.app);
    d.write(cell, 5);
    let before = d.app.game.clone();
    d.write(cell, 3);
    assert_eq!(board(&d.app.game), board(&before));
    assert!(d.app.ui.pending_ink.is_none() && d.app.live_stroke.is_none(), "INP-10");
}

#[test]
fn eraser_removes_only_entries() {
    // INP-8, MRK-6, MRK-8
    let mut d = Driver::new(7);
    let cell = empty_cell(&d.app);
    let peer = peers(cell).find(|&p| !d.app.game.is_filled(p)).unwrap();
    d.tap_micro(cell, 1);
    d.tap_micro(peer, 6);
    d.write(cell, 6);
    assert_eq!(d.app.game.value_at(cell), 6);
    let undo_len = d.app.game.undo.len();
    let given = given_cell(&d.app);
    d.erase(given);
    d.erase(peer);
    assert_eq!(d.app.game.undo.len(), undo_len, "no effect on starting numbers and empty cells");
    d.erase(cell);
    assert_eq!(d.app.game.value_at(cell), 0);
    assert_eq!(d.app.game.marks_at(cell), mark_bit(1), "MRK-6: hidden marks show again");
    assert_eq!(d.app.game.marks_at(peer) & mark_bit(6), 0, "MRK-8: the eraser does not put marks back");
}

#[test]
fn finger_on_board_has_no_effect() {
    // INP-1
    let mut d = Driver::new(8);
    let cell = empty_cell(&d.app);
    let before = d.app.game.clone();
    let (x, y) = micro_center(cell, 5);
    d.finger_tap(x, y);
    d.send(Input::TouchDown { id: 3, x, y });
    d.send(Input::TouchMove { id: 3, x: x + 200.0, y: y + 50.0 });
    d.send(Input::TouchUp { id: 3, x: x + 200.0, y: y + 50.0 });
    assert_eq!(board(&d.app.game), board(&before));
}

#[test]
fn highlights() {
    // HLT-1 to HLT-5
    let mut d = Driver::new(9);
    let given = given_cell(&d.app);
    let v = d.app.game.value_at(given);
    let other = cells().find(|&c| d.app.game.is_filled(c) && d.app.game.value_at(c) != v).unwrap();
    let before = (d.app.game.value, d.app.game.marks, d.app.game.undo.len());
    d.tap_cell(given);
    assert_eq!(d.app.ui.highlight, Some(v));
    d.tap_cell(other);
    assert_eq!(d.app.ui.highlight, Some(d.app.game.value_at(other)), "a different number moves the highlight");
    d.tap_cell(other);
    assert_eq!(d.app.ui.highlight, None, "the same number removes it");
    d.tap_cell(given);
    let (x, y) = micro_center(empty_cell(&d.app), 5);
    d.finger_tap(x, y);
    assert_eq!(d.app.ui.highlight, None, "a finger touch removes it");
    d.tap_cell(given);
    d.button(Button::Menu);
    assert_eq!(d.app.ui.highlight, None, "the menu button removes it");
    d.tap_cell(given);
    assert_eq!(d.app.ui.highlight, Some(v), "the board operates while the menu is open");
    d.button(Button::Colors);
    assert_eq!(d.app.ui.highlight, None);
    assert_eq!((d.app.game.value, d.app.game.marks, d.app.game.undo.len()), before, "HLT-5");
}

#[test]
fn clashes_are_found_after_each_change() {
    // CLS-1, CLS-2, UND-5, BRD-5
    let mut d = Driver::new(10);
    let given = given_cell(&d.app);
    let v = d.app.game.value_at(given);
    let cell = peers(given).find(|&p| !d.app.game.is_filled(p)).unwrap();
    d.write(cell, v);
    assert!(d.app.game.is_clashing(cell) && d.app.game.is_clashing(given));
    d.erase(cell);
    assert!(!d.app.game.is_clashing(given));
    d.button(Button::Menu);
    d.button(Button::Undo);
    assert!(d.app.game.is_clashing(given));
    d.button(Button::Redo);
    assert!(!d.app.game.is_clashing(given));
}

#[test]
fn undo_and_redo() {
    // UND-1, UND-3, UND-6, UND-7
    let mut d = Driver::new(11);
    d.button(Button::Menu);
    assert!(!d.app.game.can_undo() && !d.app.game.can_redo());
    let before = d.app.game.clone();
    d.button(Button::Undo);
    assert_eq!(board(&d.app.game), board(&before), "a gray button has no effect");
    let c1 = empty_cell(&d.app);
    d.tap_micro(c1, 1);
    d.write(c1, 4);
    let after = (d.app.game.value, d.app.game.marks);
    d.button(Button::Undo);
    d.button(Button::Undo);
    assert_eq!((d.app.game.value, d.app.game.marks), (before.value, before.marks));
    assert!(d.app.game.can_redo() && !d.app.game.can_undo());
    d.button(Button::Redo);
    d.button(Button::Redo);
    assert_eq!((d.app.game.value, d.app.game.marks), after);
    d.button(Button::Undo);
    d.tap_micro(c1, 9);
    assert!(d.app.game.redo.is_empty(), "a new change removes the redo history");
    // UND-2: color changes, panels and screens do not add to undo.
    let n = d.app.game.undo.len();
    d.button(Button::Colors);
    let (x, y) = color_point(0, 2);
    d.finger_tap(x, y);
    d.button(Button::Colors);
    d.button(Button::Help);
    d.finger_tap(40.0, 50.0);
    assert_eq!(d.app.game.undo.len(), n);
    // UND-6
    d.button(Button::Menu);
    d.button(Button::NewGame);
    start_new_game(&mut d);
    assert!(d.app.game.undo.is_empty() && d.app.game.redo.is_empty());
}

fn color_point(section: usize, i: usize) -> (f32, f32) {
    let (gx, gy) = layout::color_cell(section, i);
    (gx as f32 + 56.0, gy as f32 + 40.0)
}

fn start_new_game(d: &mut Driver) {
    d.finger_tap(1100.0, (layout::tb_bottom_y() + 56) as f32);
}

#[test]
fn completion_locks_the_board_and_saves_one_result() {
    // CMP-1 to CMP-6, CLK-3, INP-9, UND-8, ANA-2
    let mut d = Driver::new(12);
    d.wait(3000);
    solve(&mut d);
    let g = d.app.game.clone();
    assert!(g.completed);
    assert_eq!(g.value, g.solution, "CMP-2");
    assert_eq!(d.app.results.len(), 1, "CMP-5");
    let r = &d.app.results[0];
    assert_eq!((r.mode, r.givens, r.seconds, r.seed), (g.mode, g.givens_count(), g.seconds(), g.seed));
    // The lock.
    let given = given_cell(&d.app);
    d.tap_cell(given);
    assert_eq!(d.app.ui.highlight, None, "HLT-1");
    d.erase(cells().find(|&c| d.app.game.is_entry(c)).unwrap());
    d.button(Button::Menu);
    d.button(Button::Undo);
    d.write(given, 1);
    d.wait(10_000);
    assert_eq!(d.app.game, g, "CMP-3, CLK-3: nothing changes");
    assert!(!d.app.game.can_undo() && !d.app.game.can_redo(), "UND-8");
    assert_eq!(d.app.results.len(), 1);
    // CMP-6: the menu continues to operate; only Start new game ends the state.
    d.button(Button::Help);
    assert_eq!(d.app.screen, Screen::Manual);
    d.finger_tap(40.0, 50.0);
    d.button(Button::Menu);
    d.button(Button::NewGame);
    start_new_game(&mut d);
    assert!(!d.app.game.completed && d.app.game.elapsed_ms < 1000);
    assert_eq!(d.app.results.len(), 1, "DIF-7");
}

#[test]
fn clock() {
    // CLK-1, CLK-2, CLK-5, CLK-6
    let mut d = Driver::new(13);
    d.wait(5000);
    let t = d.app.game.elapsed_ms;
    assert!((4900..=5100).contains(&t), "{t}");
    d.button(Button::Menu);
    d.button(Button::Help);
    d.wait(2000);
    assert!(d.app.game.elapsed_ms >= t + 1900, "the clock operates on the manual (MAN-4)");
    // A suspension: the app gets no events for one hour.
    d.app.suspend();
    let t = d.app.game.elapsed_ms;
    d.now += 3_600_000;
    d.app.tick(d.now);
    assert_eq!(d.app.game.elapsed_ms, t, "CLK-5");
    // A long gap without suspend() is also a suspension.
    d.now += 60_000;
    d.app.tick(d.now);
    assert_eq!(d.app.game.elapsed_ms, t);
}

#[test]
fn menu_and_panels() {
    // NAV-4, NAV-8, NAV-11, NAV-12, §9 diagram
    let mut d = Driver::new(14);
    d.button(Button::Colors);
    assert_eq!(d.app.ui.panel, Panel::None, "hidden buttons do not operate when the menu is closed");
    d.button(Button::Menu);
    assert!(d.app.ui.menu_open);
    d.button(Button::Colors);
    assert_eq!(d.app.ui.panel, Panel::Colors);
    d.button(Button::NewGame);
    assert_eq!(d.app.ui.panel, Panel::NewGame);
    d.button(Button::Colors);
    assert_eq!(d.app.ui.panel, Panel::Colors);
    d.button(Button::Colors);
    assert_eq!(d.app.ui.panel, Panel::None);
    // A tap outside the panel only closes the panel, also on the board.
    d.button(Button::Colors);
    let cell = cells().find(|&c| !d.app.game.is_filled(c) && c.1 < 4).unwrap();
    let before = d.app.game.clone();
    d.tap_micro(cell, 1);
    assert_eq!(d.app.ui.panel, Panel::None);
    assert!(d.app.ui.menu_open);
    assert_eq!(d.app.game.marks, before.marks);
    d.button(Button::NewGame);
    d.button(Button::Undo);
    assert_eq!(d.app.ui.panel, Panel::None, "undo outside the panel only closes it");
    d.button(Button::Menu);
    assert!(!d.app.ui.menu_open);
    // The close button shows only with the open menu (NAV-6).
    d.finger_tap(layout::CLOSE_CX, layout::CLOSE_CY);
    assert!(!d.app.quit);
    d.button(Button::Menu);
    d.button(Button::Close);
    assert!(d.app.quit && d.app.game_dirty);
}

#[test]
fn cell_colors() {
    // COL-1 to COL-6
    let mut d = Driver::new(15);
    d.button(Button::Menu);
    d.button(Button::Colors);
    let red = 4;
    let (x, y) = color_point(0, red);
    d.finger_tap(x, y);
    assert_eq!(d.app.settings.color_starting, Color::Gray, "Red is taken by the clash section");
    let (x, y) = color_point(0, 2);
    d.finger_tap(x, y);
    assert_eq!(d.app.settings.color_starting, Color::Yellow);
    assert!(d.app.settings_dirty);
    for s in 0..3 {
        let (x, y) = color_point(s, 1);
        d.pen_tap(x, y);
    }
    assert_eq!(
        (d.app.settings.color_starting, d.app.settings.color_clash, d.app.settings.color_highlight),
        (Color::White, Color::White, Color::White)
    );
    assert_eq!(d.app.ui.panel, Panel::Colors, "the panel stays open");
}

#[test]
fn new_game_panel_and_difficulty() {
    // DIF-1 to DIF-7, NAV-9
    let mut d = Driver::new(16);
    let stops = d.app.ng_stops;
    let row_y = layout::ng_slider_y() as f32;
    d.write(empty_cell(&d.app), 1);
    d.button(Button::Menu);
    d.button(Button::NewGame);
    assert_eq!(d.app.ui.slider, 2, "DIF-3");
    d.finger_tap(stops[4], row_y);
    assert_eq!(d.app.ui.slider, 4);
    assert_eq!((d.app.game.mode, d.app.settings.last_mode), (2, 2), "DIF-4");
    d.button(Button::NewGame);
    d.button(Button::NewGame);
    assert_eq!(d.app.ui.slider, 2, "the selection is not kept");
    // Drag the slider with the pen.
    d.send(Input::PenDown { x: stops[1], y: row_y, tool: Tool::Pen });
    d.send(Input::PenMove { x: stops[0] - 5.0, y: row_y });
    d.send(Input::PenUp { x: stops[0] - 5.0, y: row_y });
    assert_eq!(d.app.ui.slider, 0);
    let seed = d.app.game.seed;
    start_new_game(&mut d);
    let g = &d.app.game;
    assert_eq!((g.mode, d.app.settings.last_mode), (0, 0), "DIF-5");
    assert_ne!(g.seed, seed);
    assert!(cells().all(|c| g.is_given(c) || (!g.is_filled(c) && g.marks_at(c) == 0)));
    assert!(!d.app.ui.menu_open && d.app.ui.panel == Panel::None, "NAV-9");
    assert!(d.app.results.is_empty(), "DIF-7");
    d.button(Button::Menu);
    d.button(Button::NewGame);
    assert_eq!(d.app.ui.slider, 0, "the slider opens at the last mode");
}

#[test]
fn manual() {
    // MAN-1, MAN-2, MAN-3, MAN-4, NAV-2, NAV-3
    let mut d = Driver::new(17);
    d.button(Button::Menu);
    d.button(Button::Help);
    assert_eq!((d.app.screen, d.app.manual_page), (Screen::Manual, 1));
    assert!(!d.app.ui.menu_open, "NAV-5");
    let prev = layout::prev_arrow();
    let next = layout::next_arrow();
    let center = |r: layout::Rect| (((r.x0 + r.x1) / 2) as f32, ((r.y0 + r.y1) / 2) as f32);
    d.finger_tap(center(prev).0, center(prev).1);
    assert_eq!(d.app.manual_page, 1, "the gray arrow has no effect");
    for p in 2..=9 {
        if p % 2 == 0 {
            d.swipe(true);
        } else {
            d.pen_tap(center(next).0, center(next).1);
        }
        assert_eq!(d.app.manual_page, p);
    }
    d.swipe(true);
    assert_eq!(d.app.manual_page, 9);
    d.swipe(false);
    assert_eq!(d.app.manual_page, 8);
    d.finger_tap(40.0, 50.0);
    assert_eq!(d.app.screen, Screen::Game);
    assert!(!d.app.ui.menu_open && d.app.ui.panel == Panel::None);
    d.button(Button::Menu);
    d.button(Button::Help);
    assert_eq!(d.app.manual_page, 1, "the manual always opens at page 1");
}

fn app_with_results(n: usize) -> App {
    let mut db = ResultsDb::in_memory();
    for i in 0..n {
        db.insert(&NewResult { finished_at: 1_700_000_000 + i as i64 * 1000, mode: (i % 5) as u8, givens: 30, seconds: 100 + i as u64, seed: i as u64 }).unwrap();
    }
    App::new(None, None, db, Box::new(TestEnv { time: 1_790_000_000, seed: 1 }))
}

#[test]
fn analytics_and_list() {
    // ANA-3 to ANA-8, NAV-2
    let mut d = Driver::from(app_with_results(30));
    d.button(Button::Menu);
    d.button(Button::Analytics);
    assert_eq!(d.app.screen, Screen::Analytics);
    let all = layout::all_games_button();
    d.finger_tap((all.x0 + 100) as f32, (all.y0 + 20) as f32);
    assert_eq!(d.app.screen, Screen::GamesList);
    assert_eq!((d.app.list.filter, d.app.list.page), (None, 1));
    assert_eq!(d.app.page_count(), 3);
    assert_eq!(d.app.page_rows().len(), PER_PAGE);
    assert!(d.app.page_rows()[0].finished_at > d.app.page_rows()[1].finished_at, "newest first");
    d.swipe(true);
    d.swipe(true);
    d.swipe(true);
    assert_eq!(d.app.list.page, 3);
    assert_eq!(d.app.page_rows().len(), 6);
    // A change of filter shows page 1.
    let seg = layout::filter_segment(3);
    d.finger_tap((seg.x0 + 10) as f32, (seg.y0 + 10) as f32);
    assert_eq!((d.app.list.filter, d.app.list.page), (Some(2), 1));
    assert!(d.app.page_rows().iter().all(|r| r.mode == 2));
    assert_eq!(d.app.page_rows().len(), 6);
    // The delete dialog: Cancel makes no change.
    let bin = layout::bin_button(0);
    let bc = (((bin.x0 + bin.x1) / 2) as f32, ((bin.y0 + bin.y1) / 2) as f32);
    d.finger_tap(bc.0, bc.1);
    let victim = d.app.list.confirm.clone().expect("dialog");
    d.swipe(true);
    d.finger_tap(40.0, 50.0);
    assert_eq!(d.app.screen, Screen::GamesList, "ANA-7: the dialog accepts no other input");
    let c = layout::dialog_cancel();
    d.finger_tap((c.x0 + 20) as f32, (c.y0 + 20) as f32);
    assert!(d.app.list.confirm.is_none());
    assert_eq!(d.app.results.len(), 30);
    // Delete removes the result.
    d.finger_tap(bc.0, bc.1);
    let del = layout::dialog_delete();
    d.finger_tap((del.x0 + 20) as f32, (del.y0 + 20) as f32);
    assert_eq!(d.app.results.len(), 29);
    assert!(d.app.results.iter().all(|r| r.id != victim.id));
    // ANA-8: delete the last game on the last page; the list shows the page before it.
    let seg = layout::filter_segment(0);
    d.finger_tap((seg.x0 + 10) as f32, (seg.y0 + 10) as f32);
    let matching = d.app.filtered().len();
    assert_eq!(matching, 29);
    d.swipe(true);
    d.swipe(true);
    assert_eq!(d.app.list.page, 3);
    for _ in 0..5 {
        d.finger_tap(bc.0, bc.1);
        d.finger_tap((del.x0 + 20) as f32, (del.y0 + 20) as f32);
    }
    assert_eq!(d.app.results.len(), 24);
    assert_eq!(d.app.list.page, 2);
    d.finger_tap(40.0, 50.0);
    assert_eq!(d.app.screen, Screen::Analytics);
    d.finger_tap(40.0, 50.0);
    assert_eq!(d.app.screen, Screen::Game);
    assert!(!d.app.ui.menu_open);
}

#[test]
fn empty_results() {
    // ANA-5, ANA-6
    let mut d = Driver::new(18);
    d.button(Button::Menu);
    d.button(Button::Analytics);
    let all = layout::all_games_button();
    d.finger_tap((all.x0 + 100) as f32, (all.y0 + 20) as f32);
    assert_eq!(d.app.page_count(), 1);
    assert!(d.app.page_rows().is_empty());
    let bin = layout::bin_button(0);
    d.finger_tap(((bin.x0 + bin.x1) / 2) as f32, ((bin.y0 + bin.y1) / 2) as f32);
    assert!(d.app.list.confirm.is_none());
}

#[test]
fn saved_game_opens_again_exactly() {
    // PER-1, PER-2, PER-3, PER-6
    let mut d = Driver::new(19);
    let cell = empty_cell(&d.app);
    d.tap_micro(cell, 3);
    d.write(cell, 8);
    d.button(Button::Menu);
    d.button(Button::Undo);
    d.button(Button::Colors);
    let (x, y) = color_point(2, 6);
    d.finger_tap(x, y);
    d.wait(4000);
    // Ink that waits when the app closes is removed.
    let other = cells().find(|&c| !d.app.game.is_filled(c) && c != cell).unwrap();
    d.button(Button::Colors);
    d.write_strokes(&digit_ink(other, 2));
    d.app.suspend();
    assert!(d.app.ui.pending_ink.is_none());

    let dir = std::env::temp_dir().join(format!("sudoku-test-{}", std::process::id()));
    let gp = dir.join("game.json");
    let sp = dir.join("settings.json");
    sudoku::store::save_json(&gp, &d.app.game).unwrap();
    sudoku::store::save_json(&sp, &d.app.settings).unwrap();
    let app = App::new(sudoku::store::load_json(&gp), sudoku::store::load_json(&sp), ResultsDb::in_memory(), Box::new(TestEnv { time: 0, seed: 99 }));
    assert_eq!(app.game, d.app.game);
    assert_eq!(app.settings, d.app.settings);
    assert_eq!(app.settings.color_highlight, Color::Cyan);
    assert_eq!(app.screen, Screen::Game);
    assert!(!app.ui.menu_open && app.ui.panel == Panel::None && app.ui.highlight.is_none() && app.ui.question.is_none() && app.ui.pending_ink.is_none());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_wrong_saved_game_is_not_used() {
    // PER-5
    let d = Driver::new(20);
    let mut g = d.app.game.clone();
    let given = given_cell(&d.app);
    g.value[given.0 as usize][given.1 as usize] = 0;
    let app = App::new(Some(g.clone()), None, ResultsDb::in_memory(), Box::new(TestEnv { time: 0, seed: 5 }));
    assert_ne!(app.game, g);
    let s = Settings { color_clash: Color::Gray, ..Settings::default() };
    let app = App::new(None, Some(s), ResultsDb::in_memory(), Box::new(TestEnv { time: 0, seed: 5 }));
    assert_eq!(app.settings, Settings::default());
}

#[test]
fn eraser_stroke_across_cells() {
    // INP-13: the eraser removes your numbers that it rubs. Each removal is one change.
    let mut d = Driver::new(21);
    let row = (0..9u8).find(|&r| (0..9u8).filter(|&c| !d.app.game.is_filled((r, c))).count() >= 2).unwrap();
    let empties: Vec<Cell> = (0..9u8).map(|c| (row, c)).filter(|&c| !d.app.game.is_filled(c)).take(2).collect();
    for &c in &empties {
        d.write(c, d.app.game.solution[c.0 as usize][c.1 as usize]);
    }
    let n = d.app.game.undo.len();
    let (x0, y) = micro_center(empties[0], 5);
    let (x1, _) = micro_center(empties[1], 5);
    d.send(Input::PenDown { x: x0, y, tool: Tool::Eraser });
    d.send(Input::PenMove { x: (x0 + x1) / 2.0, y });
    d.send(Input::PenMove { x: x1, y });
    d.send(Input::PenUp { x: x1, y });
    assert!(empties.iter().all(|&c| !d.app.game.is_filled(c)));
    assert_eq!(d.app.game.undo.len(), n + 2);
}

#[test]
fn a_short_touch_while_ink_waits_is_writing() {
    // INP-11: the bar of a 7 stays in one micro-cell, but it is a part of the number.
    let mut d = Driver::new(22);
    let cell = empty_cell(&d.app);
    let ink = digit_ink_by(cell, 4, 9);
    d.write_strokes(&ink);
    let (x, y) = micro_center(cell, 5);
    d.pen_tap(x, y);
    assert_eq!(d.app.game.marks_at(cell), 0, "no pencil mark");
    assert_eq!(d.app.ui.pending_ink.as_ref().map(|i| i.strokes.len()), Some(ink.len() + 1));
}

#[test]
fn a_touch_in_a_different_cell_reads_the_waiting_ink() {
    // INP-12
    let mut d = Driver::new(23);
    let cells: Vec<Cell> = cells().filter(|&c| !d.app.game.is_filled(c)).take(2).collect();
    d.write_strokes(&digit_ink(cells[0], 6));
    d.tap_micro(cells[1], 3);
    assert!(d.app.ui.pending_ink.is_none());
    assert_eq!(d.app.game.value_at(cells[0]), 6, "read immediately, before the pause");
    assert_eq!(d.app.game.marks_at(cells[1]), mark_bit(3));
}

#[test]
fn each_press_removes_the_question_mark() {
    // QST-4
    let mut d = Driver::new(24);
    let cell = empty_cell(&d.app);
    d.write_strokes(&scribble_ink(cell));
    d.wait(d.app.pause_ms + 10);
    assert!(d.app.ui.question.is_some());
    d.finger_tap(5.0, 1000.0);
    assert!(d.app.ui.question.is_none());
}

#[test]
fn a_short_finger_movement_is_not_a_swipe() {
    // INP-14
    let mut d = Driver::new(25);
    d.button(Button::Menu);
    d.button(Button::Help);
    d.send(Input::TouchDown { id: 1, x: 900.0, y: 1000.0 });
    d.send(Input::TouchUp { id: 1, x: 800.0, y: 1000.0 });
    assert_eq!(d.app.manual_page, 1, "100 px is not a swipe");
    d.send(Input::TouchDown { id: 1, x: 900.0, y: 1000.0 });
    d.send(Input::TouchUp { id: 1, x: 760.0, y: 1300.0 });
    assert_eq!(d.app.manual_page, 1, "more vertical than horizontal");
    d.send(Input::TouchDown { id: 1, x: 900.0, y: 1000.0 });
    d.send(Input::TouchUp { id: 1, x: 770.0, y: 1010.0 });
    assert_eq!(d.app.manual_page, 2);
}

#[test]
fn only_ink_changes_while_the_pen_writes() {
    // The platform makes a full drawing only when `redraw` is true. While the pen writes, it draws only
    // the new ink, so that the ink follows the pen and the rest of the screen does not refresh.
    let mut d = Driver::new(26);
    let cell = empty_cell(&d.app);
    let (x, y) = micro_center(cell, 1);
    d.app.redraw = false;
    d.send(Input::PenDown { x, y, tool: Tool::Pen });
    assert!(!d.app.redraw, "the pen touches an empty cell: nothing else changes");
    for k in 1..20 {
        d.send(Input::PenMove { x: x + k as f32 * 4.0, y: y + k as f32 * 4.0 });
        assert!(!d.app.redraw, "a pen movement while writing changes only the ink");
    }
    assert_eq!(d.app.live_stroke.as_ref().map(|s| s.len()), Some(20));
    d.send(Input::PenUp { x: x + 80.0, y: y + 80.0 });
    assert!(!d.app.redraw, "the finished stroke looks the same as the waiting ink");
    d.wait(d.app.pause_ms + 50);
    assert!(d.app.redraw, "reading the ink changes the board");

    // A highlight that the pen-down removes, and ink that the app removes, need a full drawing.
    let given = cells().find(|&c| d.app.game.is_given(c)).unwrap();
    d.tap_cell(given);
    d.app.redraw = false;
    d.send(Input::PenDown { x, y, tool: Tool::Pen });
    assert!(d.app.redraw, "the highlight goes away");
    d.send(Input::PenUp { x, y });
    let (gx, gy) = micro_center(given, 1);
    d.send(Input::PenDown { x: gx, y: gy, tool: Tool::Pen });
    d.send(Input::PenMove { x: gx + 60.0, y: gy + 60.0 });
    d.app.redraw = false;
    d.send(Input::PenUp { x: gx + 60.0, y: gy + 60.0 });
    assert!(d.app.redraw, "ink on a starting number is removed");
}

#[test]
fn pen_touch_has_no_effect_after_a_finger_changes_the_screen() {
    let mut d = Driver::new(5);
    d.button(Button::Menu);
    let cell = given_cell(&d.app);
    let (x, y) = micro_center(cell, 5);
    d.send(Input::PenDown { x, y, tool: Tool::Pen });
    d.button(Button::Help);
    assert_eq!(d.app.screen, Screen::Manual);
    d.send(Input::PenMove { x: x + 60.0, y });
    d.send(Input::PenUp { x: x + 60.0, y });
    assert_eq!(d.app.ui.highlight, None);
    assert!(d.app.ui.pending_ink.is_none());
}

/// PER-5: a saved game with a cell that is not on the board is not correct. The app makes a new game.
#[test]
fn saved_history_with_a_cell_off_the_board_is_not_used() {
    let mut g = new_app(3).game.clone();
    g.redo.push(sudoku::game::Change::Erase { cell: (12, 0), digit: 3 });
    let app = App::new(Some(g.clone()), None, ResultsDb::in_memory(), Box::new(TestEnv { time: 0, seed: 1 }));
    assert_ne!(app.game, g);
}

/// INP-1: a finger (for example a palm) on the board between two strokes does not read the ink.
#[test]
fn finger_on_the_board_does_not_read_the_ink() {
    let mut d = Driver::new(5);
    let cell = empty_cell(&d.app);
    let ink = digit_ink(cell, 4);
    assert!(ink.len() >= 2, "a 4 with two strokes");
    d.write_strokes(&ink[..1]);
    d.finger_tap(800.0, 1000.0);
    assert!(d.app.ui.pending_ink.is_some());
    d.write_strokes(&ink[1..]);
    d.wait(d.app.pause_ms + 50);
    assert_eq!(d.app.game.value_at(cell), 4);
}

#[test]
fn a_complete_game_is_saved_before_its_result() {
    // CMP-5: when the result row is added, the saved game is already complete.
    use std::sync::{Arc, Mutex};
    let mut d = Driver::new(27);
    let saved: Arc<Mutex<Vec<bool>>> = Arc::default();
    let log = saved.clone();
    d.app.save_game_now = Some(Box::new(move |g: &sudoku::game::Game| {
        log.lock().unwrap().push(g.completed);
        true
    }));
    solve(&mut d);
    assert_eq!(*saved.lock().unwrap(), vec![true], "one immediate save, of the complete game");
    assert_eq!(d.app.results.len(), 1);
    assert!(!d.app.game_dirty);
}
