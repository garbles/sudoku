//! Tests of the behavior in REQUIREMENTS.md, with the input that a player gives. Each test gives the
//! rules of INVARIANTS.md that it examines. The debug build also checks all state rules after each
//! input (`invariants::check_app`).

mod common;

use common::*;
use sudoku::app::{App, Input, Menu, Panel, Screen, Tool, BACK_GUARD_MS};
use sudoku::game::{cells, mark_bit, peers, Cell};
use sudoku::layout::{self, Button};
use sudoku::settings::{Color, Section, Settings};
use sudoku::store::ResultsDb;

/// The game without the clock.
fn board(g: &sudoku::game::Game) -> sudoku::game::Game {
    let mut g = g.clone();
    g.elapsed_ms = 0;
    g
}

/// An empty cell, an empty peer and the digit v of the solution of the cell. No peer of the peer
/// has v as a number, thus a tap adds the mark v to the peer.
fn cell_and_peer(app: &App) -> (Cell, Cell, u8) {
    let g = &app.game;
    cells()
        .filter(|&c| !g.is_filled(c))
        .find_map(|c| {
            let v = g.solution[c.0 as usize][c.1 as usize];
            peers(c).find(|&p| !g.is_filled(p) && g.peer_digits(p) & mark_bit(v) == 0).map(|p| (c, p, v))
        })
        .unwrap()
}

#[test]
fn first_start() {
    // DIF-2, NAV-1, PER-2
    let d = Driver::opened(new_app(1));
    let a = &d.app;
    assert_eq!(a.screen, Screen::Game);
    assert_eq!(a.game.mode, 2);
    assert_eq!(a.settings, Settings::default());
    assert_eq!(a.ui.menu, Menu::Open(None), "NAV-1, PER-2: the menu is open and no panel is open");
    assert!(a.ui.highlight.is_none() && a.ui.question.is_none() && a.ui.pending_ink.is_none());
    assert!(a.game.undo.is_empty() && a.game.redo.is_empty() && a.game.elapsed_ms == 0);
}

#[test]
fn tap_adds_and_removes_marks() {
    // INP-3, MRK-5, UND-1
    let mut d = Driver::new(2);
    let cell = free_cell(&d.app, &[3, 7]);
    d.tap_micro(cell, 7);
    assert_eq!(d.app.game.marks_at(cell), mark_bit(7));
    d.tap_micro(cell, 3);
    d.tap_micro(cell, 7);
    assert_eq!(d.app.game.marks_at(cell), mark_bit(3));
    assert_eq!(d.app.game.undo.len(), 3);
}

#[test]
fn writing_adds_a_number_after_the_pause() {
    // INP-5, INP-6, MRK-4, MRK-5, BRD-4
    let mut d = Driver::new(3);
    let (cell, peer, v) = cell_and_peer(&d.app);
    d.tap_micro(peer, v);
    d.tap_micro(cell, v);
    d.write_strokes(&digit_ink(cell, v));
    assert!(d.app.ui.pending_ink.is_some(), "the app waits for the pause");
    assert_eq!(d.app.game.value_at(cell), 0);
    d.wait(d.app.pause_ms + 10);
    assert!(d.app.ui.pending_ink.is_none());
    assert_eq!(d.app.game.value_at(cell), v);
    assert_eq!(d.app.game.visible_marks(peer, true) & mark_bit(v), 0, "MRK-4: the peer mark is hidden");
    assert_ne!(d.app.game.marks_at(peer) & mark_bit(v), 0, "MRK-5: the peer mark is kept");
    assert_ne!(d.app.game.marks_at(cell) & mark_bit(v), 0, "the marks of the cell are hidden, not removed");
    // UND-4: undo shows the peer mark again.
    d.button(Button::Menu);
    d.button(Button::Undo);
    assert_eq!(d.app.game.value_at(cell), 0);
    assert_ne!(d.app.game.visible_marks(peer, true) & mark_bit(v), 0);
}

#[test]
fn handwritten_digits_are_read_through_the_pen() {
    // INP-6: real handwriting of 5 writers, through the full pen input path.
    for v in 1..=9u8 {
        for writer in [3u8, 17, 29, 44, 58] {
            let mut d = Driver::new(40 + v as u64);
            let cell = empty_cell(&d.app.game);
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
    let cell = empty_cell(&d.app.game);
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
    let cell = empty_cell(&d.app.game);
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
    let given = given_cell(&d.app.game);
    let before = d.app.game.clone();
    d.write(given, if before.value_at(given) == 1 { 2 } else { 1 });
    assert_eq!(board(&d.app.game), board(&before));
    let cell = empty_cell(&d.app.game);
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
    let (cell, peer, v) = cell_and_peer(&d.app);
    let other = (1..=9u8).find(|&m| m != v && d.app.game.peer_digits(cell) & mark_bit(m) == 0).unwrap();
    d.tap_micro(cell, other);
    d.tap_micro(peer, v);
    d.write(cell, v);
    assert_eq!(d.app.game.value_at(cell), v);
    let undo_len = d.app.game.undo.len();
    let given = given_cell(&d.app.game);
    d.erase(given);
    d.erase(peer);
    assert_eq!(d.app.game.undo.len(), undo_len, "no effect on starting numbers and empty cells");
    d.erase(cell);
    assert_eq!(d.app.game.value_at(cell), 0);
    assert_eq!(d.app.game.visible_marks(cell, true), mark_bit(other), "MRK-6: hidden marks show again");
    assert_ne!(d.app.game.visible_marks(peer, true) & mark_bit(v), 0, "MRK-8: the peer mark shows again");
}

#[test]
fn finger_on_board_has_no_effect() {
    // INP-1
    let mut d = Driver::new(8);
    let cell = empty_cell(&d.app.game);
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
    let given = given_cell(&d.app.game);
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
    let (x, y) = micro_center(empty_cell(&d.app.game), 5);
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
    let given = given_cell(&d.app.game);
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
    let c1 = empty_cell(&d.app.game);
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
    let (x, y) = color_point(Section::Starting, Color::Yellow);
    d.finger_tap(x, y);
    d.button(Button::Colors);
    d.button(Button::Help);
    d.finger_tap(40.0, 50.0);
    assert_eq!(d.app.game.undo.len(), n);
    // UND-6. "< Back" showed the game with the menu open (NAV-3). The toolbar operates again after
    // BACK_GUARD_MS (NAV-16).
    d.wait(BACK_GUARD_MS);
    d.button(Button::NewGame);
    start_new_game(&mut d);
    assert!(d.app.game.undo.is_empty() && d.app.game.redo.is_empty());
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
    let given = given_cell(&d.app.game);
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
    assert_eq!(d.app.ui.menu, Menu::Open(None), "NAV-3");
    d.wait(BACK_GUARD_MS); // NAV-16
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
    assert_eq!(d.app.ui.menu.panel(), None, "hidden buttons do not operate when the menu is closed");
    d.button(Button::Menu);
    assert!(d.app.ui.menu.is_open());
    d.button(Button::Colors);
    assert_eq!(d.app.ui.menu.panel(), Some(Panel::Colors));
    d.button(Button::NewGame);
    assert_eq!(d.app.ui.menu.panel(), Some(Panel::NewGame));
    d.button(Button::Colors);
    assert_eq!(d.app.ui.menu.panel(), Some(Panel::Colors));
    d.button(Button::Colors);
    assert_eq!(d.app.ui.menu.panel(), None);
    // A tap outside the panel only closes the panel, also on the board.
    d.button(Button::Colors);
    let cell = cells().find(|&c| !d.app.game.is_filled(c) && c.1 < 4).unwrap();
    let before = d.app.game.clone();
    d.tap_micro(cell, 1);
    assert_eq!(d.app.ui.menu.panel(), None);
    assert!(d.app.ui.menu.is_open());
    assert_eq!(d.app.game.marks, before.marks);
    d.button(Button::NewGame);
    d.button(Button::Undo);
    assert_eq!(d.app.ui.menu.panel(), None, "undo outside the panel only closes it");
    d.button(Button::Menu);
    assert!(!d.app.ui.menu.is_open());
    // The close button shows only with the open menu (NAV-6).
    d.finger_tap(layout::close_cx(), layout::CLOSE_CY);
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
    let (x, y) = color_point(Section::Starting, Color::Red);
    d.finger_tap(x, y);
    assert_eq!(d.app.settings.color_starting, Color::Gray, "Red is taken by the clash section");
    let (x, y) = color_point(Section::Starting, Color::Yellow);
    d.finger_tap(x, y);
    assert_eq!(d.app.settings.color_starting, Color::Yellow);
    assert!(d.app.settings_dirty);
    for s in Section::ALL {
        let (x, y) = color_point(s, Color::White);
        d.pen_tap(x, y);
    }
    assert_eq!(
        (d.app.settings.color_starting, d.app.settings.color_clash, d.app.settings.color_highlight),
        (Color::White, Color::White, Color::White)
    );
    assert_eq!(d.app.ui.menu.panel(), Some(Panel::Colors), "the panel stays open");
}

#[test]
fn new_game_panel_and_difficulty() {
    // DIF-1 to DIF-7, NAV-9
    let mut d = Driver::new(16);
    let stops = d.app.ng_stops;
    let row_y = layout::ng_slider_y() as f32;
    d.write(empty_cell(&d.app.game), 1);
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
    assert_eq!(d.app.ui.menu, Menu::Open(None), "NAV-9: the panel closes and the menu stays open");
    assert!(d.app.results.is_empty(), "DIF-7");
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
    assert!(!d.app.ui.menu.is_open(), "NAV-5");
    let prev = layout::prev_arrow();
    let next = layout::next_arrow();
    d.finger_tap(pixel_center(prev).0, pixel_center(prev).1);
    assert_eq!(d.app.manual_page, 1, "the gray arrow has no effect");
    for p in 2..=9 {
        if p % 2 == 0 {
            d.swipe(true);
        } else {
            d.pen_tap(pixel_center(next).0, pixel_center(next).1);
        }
        assert_eq!(d.app.manual_page, p);
    }
    d.swipe(true);
    assert_eq!(d.app.manual_page, 9);
    d.swipe(false);
    assert_eq!(d.app.manual_page, 8);
    d.finger_tap(40.0, 50.0);
    assert_eq!(d.app.screen, Screen::Game);
    assert_eq!(d.app.ui.menu, Menu::Open(None), "NAV-3");
    d.wait(BACK_GUARD_MS); // NAV-16
    d.button(Button::Help);
    assert_eq!(d.app.manual_page, 1, "the manual always opens at page 1");
}

#[test]
fn the_landscape_list_keeps_the_first_game_of_the_page() {
    // R§3.3, ANA-6: a landscape page has 6 games in one table across the full width. A turn keeps
    // the first game of the page visible.
    let mut d = Driver::from(app_with_results(30));
    d.button(Button::Menu);
    d.button(Button::Statistics);
    let all = layout::all_games_button();
    d.finger_tap((all.x0 + 100) as f32, (all.y0 + 20) as f32);
    d.swipe(true);
    let first = d.app.page_rows()[0].id;
    assert_eq!((d.app.list.page, d.app.page_count()), (2, 3));
    layout::set_orientation(layout::Orientation::Landscape);
    d.app.rotate(d.app.ng_stops);
    assert_eq!((d.app.list.page, d.app.page_count(), d.app.page_rows().len()), (3, 5, 6));
    assert_eq!(d.app.page_rows()[0].id, first);
    sudoku::invariants::check_app(&d.app).unwrap();
    d.swipe(true);
    assert_eq!(d.app.list.page, 4);
    layout::set_orientation(layout::Orientation::Portrait);
    d.app.rotate(d.app.ng_stops);
    assert_eq!(d.app.list.page, 2, "game 19 is on portrait page 2");
    sudoku::invariants::check_app(&d.app).unwrap();
}

#[test]
fn analytics_and_list() {
    // ANA-3 to ANA-8, NAV-2
    let mut d = Driver::from(app_with_results(30));
    d.button(Button::Menu);
    d.button(Button::Statistics);
    assert_eq!(d.app.screen, Screen::Statistics);
    let all = layout::all_games_button();
    d.finger_tap((all.x0 + 100) as f32, (all.y0 + 20) as f32);
    assert_eq!(d.app.screen, Screen::GamesList);
    assert_eq!((d.app.list.filter, d.app.list.page), (None, 1));
    assert_eq!(d.app.page_count(), 3);
    assert_eq!(d.app.page_rows().len(), layout::list_rows());
    assert!(d.app.page_rows()[0].finished_at > d.app.page_rows()[1].finished_at, "newest first");
    d.swipe(true);
    d.swipe(true);
    d.swipe(true);
    assert_eq!(d.app.list.page, 3);
    assert_eq!(d.app.page_rows().len(), 6);
    // A change of filter shows page 1.
    let seg = layout::filter_segment(Some(2));
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
    let seg = layout::filter_segment(None);
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
    assert_eq!(d.app.screen, Screen::Statistics);
    d.finger_tap(40.0, 50.0);
    assert_eq!(d.app.screen, Screen::Game);
    assert_eq!(d.app.ui.menu, Menu::Open(None), "NAV-3");
}

#[test]
fn empty_results() {
    // ANA-5, ANA-6
    let mut d = Driver::new(18);
    d.button(Button::Menu);
    d.button(Button::Statistics);
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
    let cell = empty_cell(&d.app.game);
    d.tap_micro(cell, 3);
    d.write(cell, 8);
    d.button(Button::Menu);
    d.button(Button::Undo);
    d.button(Button::Colors);
    let (x, y) = color_point(Section::Highlight, Color::Cyan);
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
    let app = App::new(sudoku::store::load_json(&gp), sudoku::store::load_json(&sp), ResultsDb::in_memory(), Box::new(TestEnv { time: 0, seed: 99 }), App::placeholder_ng_stops());
    assert_eq!(app.game, d.app.game);
    assert_eq!(app.settings, d.app.settings);
    assert_eq!(app.settings.color_highlight, Color::Cyan);
    assert_eq!(app.screen, Screen::Game);
    assert_eq!(app.ui.menu, Menu::Open(None), "PER-2: the menu is open and the app does not keep the menu state");
    assert!(app.ui.highlight.is_none() && app.ui.question.is_none() && app.ui.pending_ink.is_none());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_app_opens_with_the_menu_open() {
    // NAV-1, PER-2: a new game and a saved game open with the menu open and no panel. The app does
    // not keep the menu state.
    let mut d = Driver::opened(new_app(40));
    assert_eq!(d.app.screen, Screen::Game);
    assert_eq!(d.app.ui.menu, Menu::Open(None), "a new game");
    // The menu button closes the open menu (§9).
    d.button(Button::Menu);
    assert_eq!(d.app.ui.menu, Menu::Closed);
    let cell = empty_cell(&d.app.game);
    d.tap_micro(cell, 4);
    d.tap_cell(given_cell(&d.app.game));
    assert!(d.app.ui.highlight.is_some());
    d.app.suspend();
    let app = app_with_game(d.app.game.clone(), START_TIME, 41);
    assert_eq!(app.game, d.app.game, "PER-2: the saved game is used");
    assert_eq!(app.screen, Screen::Game);
    assert_eq!(app.ui.menu, Menu::Open(None), "a saved game: the closed menu is not kept");
    assert!(app.ui.highlight.is_none() && app.ui.question.is_none() && app.ui.pending_ink.is_none());
    // A saved game with a panel open when the app closed.
    let mut d = Driver::opened(app);
    d.button(Button::Colors);
    assert_eq!(d.app.ui.menu, Menu::Open(Some(Panel::Colors)));
    d.app.suspend();
    let app = app_with_game(d.app.game.clone(), START_TIME, 42);
    assert_eq!(app.ui.menu, Menu::Open(None), "the panel is not kept");
}

#[test]
fn back_shows_the_game_with_the_menu_open() {
    // NAV-2, NAV-3, NAV-5, NAV-15: "< Back" from the manual and from Statistics shows the game with
    // the menu open and no panel, for the left-handed and the right-handed layout. "< Back" from the
    // games list shows Statistics.
    for right_handed in [false, true] {
        layout::set_right_handed(right_handed);
        let mut d = Driver::new(43);
        let back = |d: &mut Driver| d.finger_tap(40.0, 50.0);
        // From the manual.
        d.button(Button::Menu);
        d.button(Button::Help);
        assert_eq!((d.app.screen, d.app.ui.menu), (Screen::Manual, Menu::Closed), "NAV-5");
        back(&mut d);
        assert_eq!((d.app.screen, d.app.ui.menu), (Screen::Game, Menu::Open(None)), "NAV-3: from the manual (right-handed: {right_handed})");
        // From Statistics. A tap outside the open panel only closes the panel (§9). After "< Back",
        // the toolbar operates again after BACK_GUARD_MS (NAV-16).
        d.wait(BACK_GUARD_MS);
        d.button(Button::Settings);
        assert_eq!(d.app.ui.menu, Menu::Open(Some(Panel::Settings)));
        d.button(Button::Statistics);
        assert_eq!((d.app.screen, d.app.ui.menu), (Screen::Game, Menu::Open(None)));
        d.button(Button::Statistics);
        assert_eq!((d.app.screen, d.app.ui.menu), (Screen::Statistics, Menu::Closed), "NAV-5");
        back(&mut d);
        assert_eq!((d.app.screen, d.app.ui.menu), (Screen::Game, Menu::Open(None)), "NAV-3: from Statistics (right-handed: {right_handed})");
        // From the games list: Statistics shows, with the menu closed.
        d.wait(BACK_GUARD_MS);
        d.button(Button::Statistics);
        let all = layout::all_games_button();
        d.finger_tap((all.x0 + 100) as f32, (all.y0 + 20) as f32);
        assert_eq!(d.app.screen, Screen::GamesList);
        back(&mut d);
        assert_eq!((d.app.screen, d.app.ui.menu), (Screen::Statistics, Menu::Closed), "NAV-2, NAV-5: from the games list");
        back(&mut d);
        assert_eq!((d.app.screen, d.app.ui.menu), (Screen::Game, Menu::Open(None)), "NAV-3");
        // The open menu operates: the menu button closes it.
        d.wait(BACK_GUARD_MS);
        d.button(Button::Menu);
        assert_eq!(d.app.ui.menu, Menu::Closed);
    }
    layout::set_right_handed(false);
}

/// The center of "< Back" on the manual and on Statistics.
fn back_center() -> (f32, f32) {
    pixel_center(layout::back_button())
}

/// Opens the manual (`Button::Help`) or Statistics (`Button::Statistics`) from the closed menu, and
/// taps "< Back" with the pen or with a finger. The game shows with the menu open (NAV-3).
fn back_from(d: &mut Driver, page: Button, pen: bool) {
    d.button(Button::Menu);
    d.button(page);
    assert_ne!(d.app.screen, Screen::Game);
    tap_back(d, pen);
    assert_eq!((d.app.screen, d.app.ui.menu), (Screen::Game, Menu::Open(None)), "NAV-3");
}

fn tap_back(d: &mut Driver, pen: bool) {
    let (x, y) = back_center();
    if pen { d.pen_tap(x, y) } else { d.finger_tap(x, y) }
}

#[test]
fn a_fast_double_tap_on_back_does_not_close_the_app_or_the_menu() {
    // NAV-16, NAV-15: "< Back" is at the position of the close button (left-handed) or of the menu
    // button (right-handed). The second tap of a fast double tap on "< Back" changes nothing: the
    // app does not close and the menu stays open. Both layouts, from the manual and from
    // Statistics, with the pen and with a finger.
    for right_handed in [false, true] {
        layout::set_right_handed(right_handed);
        let under_back = if right_handed { Button::Menu } else { Button::Close };
        let (x, y) = back_center();
        assert_eq!(layout::game_button(x, y, true), Some(under_back), "the button under \"< Back\"");
        for page in [Button::Help, Button::Statistics] {
            for pen in [false, true] {
                let case = format!("right-handed: {right_handed}, page: {page:?}, pen: {pen}");
                let mut d = Driver::new(45);
                d.wait(2000);
                back_from(&mut d, page, pen);
                let game = d.app.game.clone();
                let settings = d.app.settings.clone();
                tap_back(&mut d, pen);
                assert!(!d.app.quit, "NAV-16: the app closed ({case})");
                assert_eq!((d.app.screen, d.app.ui.menu), (Screen::Game, Menu::Open(None)), "NAV-16 ({case})");
                assert_eq!(board(&d.app.game), board(&game), "NAV-16: the game changed ({case})");
                assert_eq!(d.app.settings, settings, "NAV-16 ({case})");
                // After BACK_GUARD_MS, the same tap operates.
                d.wait(BACK_GUARD_MS);
                tap_back(&mut d, pen);
                if right_handed {
                    assert!(!d.app.quit && d.app.ui.menu == Menu::Closed, "NAV-16: the menu button operates again ({case})");
                } else {
                    assert!(d.app.quit, "NAV-16: the close button operates again ({case})");
                }
            }
        }
    }
    layout::set_right_handed(false);
}

#[test]
fn the_toolbar_ignores_all_buttons_shortly_after_back() {
    // NAV-16: for BACK_GUARD_MS after "< Back", each toolbar button and the close button change
    // nothing, for the left-handed and the right-handed layout. After this time, they operate.
    for right_handed in [false, true] {
        layout::set_right_handed(right_handed);
        for b in [Button::Menu, Button::Colors, Button::Settings, Button::Undo, Button::Redo, Button::Help, Button::Statistics, Button::NewGame, Button::Close] {
            let case = format!("right-handed: {right_handed}, button: {b:?}");
            let mut d = Driver::new(46);
            // Two marks and one undo give the undo button and the redo button an action.
            let cell = free_cell(&d.app, &[1, 3]);
            d.tap_micro(cell, 1);
            d.tap_micro(cell, 3);
            d.button(Button::Menu);
            d.button(Button::Undo);
            d.button(Button::Menu);
            let game = d.app.game.clone();
            back_from(&mut d, Button::Help, false);
            d.wait(BACK_GUARD_MS - 50);
            d.button(b);
            assert!(!d.app.quit, "NAV-16 ({case})");
            assert_eq!((d.app.screen, d.app.ui.menu), (Screen::Game, Menu::Open(None)), "NAV-16 ({case})");
            assert_eq!(board(&d.app.game), board(&game), "NAV-16 ({case})");
            d.wait(50);
            d.button(b);
            let changed = d.app.quit || d.app.screen != Screen::Game || d.app.ui.menu != Menu::Open(None) || board(&d.app.game) != board(&game);
            assert!(changed, "NAV-16: the button operates after the time ({case})");
        }
    }
    layout::set_right_handed(false);
}

#[test]
fn the_board_operates_shortly_after_back() {
    // NAV-16: a press on the board operates as usual during BACK_GUARD_MS after "< Back".
    for right_handed in [false, true] {
        layout::set_right_handed(right_handed);
        for page in [Button::Help, Button::Statistics] {
            let mut d = Driver::new(47);
            back_from(&mut d, page, true);
            let back_at = d.now;
            let cell = free_cell(&d.app, &[2]);
            d.tap_micro(cell, 2);
            assert!(d.now < back_at + BACK_GUARD_MS, "the tap is in the time of NAV-16");
            assert_eq!(d.app.game.marks_at(cell) & mark_bit(2), mark_bit(2), "NAV-16: the board operates (right-handed: {right_handed}, page: {page:?})");
            assert_eq!(d.app.ui.menu, Menu::Open(None));
        }
    }
    layout::set_right_handed(false);
}

#[test]
fn start_new_game_keeps_the_menu_open() {
    // NAV-9: after Start new game, the New game panel closes and the menu stays open.
    let mut d = Driver::opened(new_app(44));
    let seed = d.app.game.seed;
    d.button(Button::NewGame);
    assert_eq!(d.app.ui.menu, Menu::Open(Some(Panel::NewGame)));
    start_new_game(&mut d);
    assert_ne!(d.app.game.seed, seed, "a new puzzle shows");
    assert_eq!((d.app.screen, d.app.ui.menu), (Screen::Game, Menu::Open(None)), "NAV-9");
    assert!(d.app.ui.highlight.is_none() && d.app.ui.question.is_none() && d.app.ui.pending_ink.is_none());
    // The menu buttons operate at once: + opens the panel again.
    d.button(Button::NewGame);
    assert_eq!(d.app.ui.menu, Menu::Open(Some(Panel::NewGame)));
}

#[test]
fn a_wrong_saved_game_is_not_used() {
    // PER-5
    let d = Driver::new(20);
    let mut g = d.app.game.clone();
    let given = given_cell(&d.app.game);
    g.value[given.0 as usize][given.1 as usize] = 0;
    let app = app_with_game(g.clone(), 0, 5);
    assert_ne!(app.game, g);
    let s = Settings { color_clash: Color::Gray, ..Settings::default() };
    let app = App::new(None, Some(s), ResultsDb::in_memory(), Box::new(TestEnv { time: 0, seed: 5 }), App::placeholder_ng_stops());
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
    let cell = empty_cell(&d.app.game);
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
    let free = free_cell(&d.app, &[3]);
    let first = cells().find(|&c| !d.app.game.is_filled(c) && c != free).unwrap();
    let cells: Vec<Cell> = vec![first, free];
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
    let cell = empty_cell(&d.app.game);
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
    let cell = empty_cell(&d.app.game);
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
    let cell = given_cell(&d.app.game);
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
    let app = app_with_game(g.clone(), 0, 1);
    assert_ne!(app.game, g);
}

/// INP-1: a finger (for example a palm) on the board between two strokes does not read the ink.
#[test]
fn finger_on_the_board_does_not_read_the_ink() {
    let mut d = Driver::new(5);
    let cell = empty_cell(&d.app.game);
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

/// The stroke log of the corpus build (src/corpus.rs): each stroke goes to the log with its cell,
/// also when the app reads the ink between two strokes (then the app can discard the second stroke
/// on the filled cell). The eraser goes to the log also.
#[test]
fn the_stroke_log_gets_each_stroke_with_its_cell() {
    use std::sync::{Arc, Mutex};
    use sudoku::app::BoardInput;
    let mut d = Driver::new(5);
    let cell = empty_cell(&d.app.game);
    let ink = digit_ink(cell, 4);
    assert!(ink.len() >= 2, "a 4 with two strokes");
    // Each input: the cell, and the quantity of points of a stroke (None: an erasure).
    type Log = Vec<(Cell, Option<usize>)>;
    let log: Arc<Mutex<Log>> = Arc::default();
    let l = log.clone();
    d.app.stroke_log = Some(Box::new(move |input| {
        l.lock().unwrap().push(match input {
            BoardInput::Stroke(c, s) => (c, Some(s.len())),
            BoardInput::Erase(c) => (c, None),
        })
    }));
    // The app reads the first stroke before the second stroke starts.
    d.write_strokes(&ink[..1]);
    d.wait(d.app.pause_ms + 50);
    assert!(d.app.ui.pending_ink.is_none());
    d.write_strokes(&ink[1..]);
    d.wait(d.app.pause_ms + 50);
    d.erase(cell);
    let got = log.lock().unwrap().clone();
    // The app adds the point of the pen lift to the stroke.
    let strokes: Vec<(Cell, Option<usize>)> = ink.iter().map(|s| (cell, Some(s.len() + 1))).collect();
    assert_eq!(&got[..ink.len()], &strokes[..], "all strokes of the 4, in the cell of the 4");
    assert!(got[ink.len()..].contains(&(cell, None)), "the erasure of the cell");
}
