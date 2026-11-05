//! Tests of the behavior in REQUIREMENTS.md, with the input that a player gives. Each test gives the
//! rules of INVARIANTS.md that it examines. The debug build also checks all state rules after each
//! input (`invariants::check_app`).

mod common;

use common::*;
use sudoku::app::{App, Input, Menu, Panel, Screen, Tool, BACK_GUARD_MS, OVERLAY_MS};
use sudoku::game::{cells, mark_bit, peers, Cell};
use sudoku::layout::{self, Button};
use sudoku::reference;
use sudoku::render::rgb_at;
use sudoku::settings::{Color, Section, Settings, Switch};
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
    assert!(a.ui.highlight.is_none() && a.ui.question.is_none() && a.ui.refused.is_none() && a.ui.pending_ink.is_none());
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
        (d.app.settings.color_starting, d.app.settings.color_clash, d.app.settings.color_highlight, d.app.settings.color_completed),
        (Color::White, Color::White, Color::White, Color::White)
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
    // Colors that are not the defaults, thus the test examines a change of the state.
    assert_ne!(d.app.settings.color_highlight, Color::Yellow);
    assert_ne!(d.app.settings.color_completed, Color::Magenta);
    let (x, y) = color_point(Section::Highlight, Color::Yellow);
    d.finger_tap(x, y);
    let (x, y) = color_point(Section::Completed, Color::Magenta);
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
    assert_eq!(app.settings.color_highlight, Color::Yellow);
    assert_eq!(app.settings.color_completed, Color::Magenta);
    assert_eq!(app.screen, Screen::Game);
    assert_eq!(app.ui.menu, Menu::Open(None), "PER-2: the menu is open and the app does not keep the menu state");
    assert!(app.ui.highlight.is_none() && app.ui.question.is_none() && app.ui.refused.is_none() && app.ui.pending_ink.is_none());
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

#[test]
fn the_puzzle_made_while_the_panel_is_open_is_the_puzzle_of_the_seed() {
    // DIF-5, PUZ-5: while the New game panel is open, the app makes the puzzle of the slider mode in
    // the background. Each started game has a new seed, and its puzzle is the puzzle of that seed
    // and mode, also when the panel opens and closes before the start and the slider moves.
    let mut d = Driver::new(5);
    let (stops, row_y) = (d.app.ng_stops, layout::ng_slider_y() as f32);
    let mut seeds = vec![d.app.game.seed];
    d.button(Button::Menu);
    for (k, stop) in [3, 1, 5].into_iter().enumerate() {
        d.button(Button::NewGame);
        d.button(Button::NewGame); // the panel closes without a new game
        d.button(Button::NewGame);
        d.send(Input::PenDown { x: stops[0], y: row_y, tool: Tool::Pen });
        d.send(Input::PenMove { x: stops[stop], y: row_y });
        d.send(Input::PenUp { x: stops[stop], y: row_y });
        let mode = d.app.ui.slider;
        start_new_game(&mut d);
        let g = &d.app.game;
        let p = sudoku::generator::generate_version(g.generator, g.seed, g.mode);
        let given = p.givens.map(|row| row.map(|v| v != 0));
        assert_eq!((g.mode, g.solution, g.given), (mode, p.solution, given), "game {k}");
        assert!(!seeds.contains(&g.seed), "game {k}: a new seed");
        seeds.push(g.seed);
    }
}

// ---- The x of a refused tap (MRK-7, MRK-10 to MRK-18, R§5.1, R§6.1).

/// The cell and the digit of the x.
fn x_of(d: &Driver) -> Option<(Cell, u8)> {
    d.app.ui.refused.map(|(cell, digit, _)| (cell, digit))
}

/// MRK-12, from the documents and not from the code of `invariants::check_app`. These tests also run
/// in the release build, which does not do the checks after each input.
fn assert_x_rule(d: &Driver, case: &str) {
    sudoku::invariants::check_app(&d.app).unwrap_or_else(|e| panic!("{e} ({case})"));
    let Some((cell, digit, _)) = d.app.ui.refused else { return };
    let (a, g) = (&d.app, &d.app.game);
    assert_eq!(a.screen, Screen::Game, "MRK-12 ({case})");
    assert_eq!(a.ui.menu.panel(), None, "MRK-12: an open panel never shows an x ({case})");
    assert!(!g.completed && !a.update.dialog_shown(), "MRK-12 ({case})");
    assert!(!g.is_filled(cell), "MRK-12: the cell of the x is empty ({case})");
    assert!(a.settings.auto_hide_on, "MRK-12 ({case})");
    assert!(peer_has(g, cell, digit), "MRK-12: a peer has the digit ({case})");
    assert!(a.ui.highlight.is_none() && a.ui.pending_ink.is_none(), "MRK-12 ({case})");
    assert_ne!(a.ui.question.map(|q| q.0), Some(cell), "MRK-12: the \"?\" is on a different cell ({case})");
    assert_eq!(g.visible_marks(cell, true) & mark_bit(digit), 0, "MRK-12: a mark never shows under an x ({case})");
}

/// Sends the clock to the time `t`, with no input.
fn tick_to(d: &mut Driver, t: u64) {
    assert!(t >= d.now);
    d.now = t;
    d.app.tick(t);
}

/// Does each step of `f` in all layouts: portrait and landscape, left-handed and right-handed.
fn for_each_layout(mut f: impl FnMut(&str)) {
    for orientation in [layout::Orientation::Portrait, layout::Orientation::Landscape] {
        for right_handed in [false, true] {
            layout::set_orientation(orientation);
            layout::set_right_handed(right_handed);
            f(&format!("landscape: {}, right-handed: {right_handed}", orientation == layout::Orientation::Landscape));
        }
    }
    layout::set_orientation(layout::Orientation::Portrait);
    layout::set_right_handed(false);
}

/// Puts two marks in one cell and undoes one. Thus Undo and Redo have an action. The menu is open
/// at the end.
fn fill_stacks(d: &mut Driver) -> Cell {
    let g = &d.app.game;
    let (cell, a, b) = cells()
        .filter(|&c| !g.is_filled(c))
        .find_map(|c| {
            let free: Vec<u8> = (1..=9u8).filter(|&v| !peer_has(g, c, v)).collect();
            (free.len() >= 2).then(|| (c, free[0], free[1]))
        })
        .expect("a cell with two free digits");
    d.tap_micro(cell, a);
    d.tap_micro(cell, b);
    if !d.app.ui.menu.is_open() {
        d.button(Button::Menu);
    }
    d.button(Button::Undo);
    assert_eq!((d.app.game.undo.len(), d.app.game.redo.len()), (1, 1));
    cell
}

/// A finger presses a control and stays on it.
fn hold(d: &mut Driver, b: Button) -> (f32, f32) {
    let (x, y) = button_center(b);
    d.send(Input::TouchDown { id: 1, x, y });
    (x, y)
}

/// A refused tap on the first refused pair that is not in the cell `not`. The test examines the x.
fn refused_tap_not_in(d: &mut Driver, not: Option<Cell>) -> (Cell, u8) {
    let (cell, digit) = refused_pairs(&d.app.game).into_iter().find(|p| Some(p.0) != not).expect("a refused pair");
    d.tap_micro(cell, digit);
    assert_eq!(d.app.ui.refused, Some((cell, digit, d.now)), "MRK-10");
    (cell, digit)
}

fn refused_tap(d: &mut Driver) -> (Cell, u8) {
    refused_tap_not_in(d, None)
}

/// Fills all empty cells except `last` with the solution.
fn fill_except(d: &mut Driver, last: Cell) {
    for c in cells().collect::<Vec<_>>() {
        if c != last && !d.app.game.is_filled(c) {
            let v = d.app.game.solution[c.0 as usize][c.1 as usize];
            d.app.game.add_number(c, v);
        }
    }
    assert!(!d.app.game.completed);
}

/// A game where one Redo fills the last empty cell and completes the game. The test sets this state
/// directly, because the number made the game complete when the player wrote it, and a complete game
/// has no undo.
fn redo_completes(d: &mut Driver) -> Cell {
    let last = empty_cell(&d.app.game);
    fill_except(d, last);
    let v = d.app.game.solution[last.0 as usize][last.1 as usize];
    d.app.game.add_number(last, v);
    assert!(d.app.game.completed);
    d.app.game.completed = false;
    d.app.game.undo();
    assert_eq!(d.app.game.value_at(last), 0);
    assert!(!d.app.game.completed && !d.app.game.redo.is_empty());
    last
}

#[test]
fn a_refused_tap_shows_the_x_at_the_lift_and_changes_nothing() {
    // MRK-7, MRK-10, MRK-11, INP-3, UND-2: the x shows at the lift and not at the press. The numbers,
    // the marks, the two stacks and the saved data do not change. Also the redo history stays.
    let mut d = Driver::new(101);
    fill_stacks(&mut d);
    let (cell, digit) = refused_pair(&d.app.game);
    let before = board(&d.app.game);
    let (x, y) = micro_center(cell, digit);
    d.app.game_dirty = false;
    d.app.settings_dirty = false;

    d.send(Input::PenDown { x, y, tool: Tool::Pen });
    assert_eq!(d.app.ui.refused, None, "MRK-10: no x at the press");
    d.wait(200);
    d.send(Input::PenMove { x: x + 3.0, y: y + 3.0 });
    assert_eq!(d.app.ui.refused, None, "MRK-10: no x before the lift");
    d.send(Input::PenUp { x: x + 3.0, y: y + 3.0 });
    assert_eq!(d.app.ui.refused, Some((cell, digit, d.now)), "MRK-10: the x shows at the lift, with its cell and digit");
    assert_x_rule(&d, "lift");
    assert_eq!(board(&d.app.game), before, "MRK-11: the game does not change");
    assert!(!d.app.game_dirty && !d.app.settings_dirty, "MRK-11: no save");

    d.wait(OVERLAY_MS);
    assert_eq!(d.app.ui.refused, None);
    assert_eq!(board(&d.app.game), before, "MRK-11: the end of the x does not change the game");
    assert!(!d.app.game_dirty && !d.app.settings_dirty, "MRK-11: no save");

    // The redo history is still there: Redo operates.
    let redo = d.app.game.redo[0].clone();
    d.button(Button::Redo);
    assert!(d.app.game.redo.is_empty());
    assert_eq!(d.app.game.undo.last(), Some(&redo), "MRK-11: a refused tap does not empty the redo history");
}

#[test]
fn a_refused_tap_does_not_change_the_clock() {
    // MRK-11, CLK-1: the clock of a game with a refused tap and the clock of the same game with no input
    // show the same time. The x also does not stop the clock.
    let mut with_tap = Driver::new(102);
    let mut no_tap = Driver::new(102);
    tick_to(&mut with_tap, 2000);
    tick_to(&mut no_tap, 2000);
    refused_tap(&mut with_tap);
    tick_to(&mut no_tap, with_tap.now);
    assert_eq!(with_tap.app.game.elapsed_ms, no_tap.app.game.elapsed_ms, "MRK-11: at the lift");
    assert!(with_tap.app.game.elapsed_ms > 0);
    let end = with_tap.now + OVERLAY_MS + 500;
    tick_to(&mut with_tap, end);
    tick_to(&mut no_tap, end);
    assert_eq!(with_tap.app.game.elapsed_ms, no_tap.app.game.elapsed_ms, "MRK-11: after the x");
    assert_eq!(with_tap.app.ui.refused, None);
    assert!(!with_tap.app.game.completed);
}

#[test]
fn each_micro_cell_of_each_empty_cell_gives_the_x_or_the_mark_in_all_layouts() {
    // MRK-7, MRK-10, INP-3: in all layouts, in the corners and at the borders of the board, each tap
    // gives the x of that cell and digit if a peer has the digit, and otherwise a mark.
    for_each_layout(|case| {
        let mut d = Driver::new(103);
        for cell in cells().collect::<Vec<_>>() {
            if d.app.game.is_filled(cell) {
                continue;
            }
            for digit in 1..=9u8 {
                let case = format!("{case}, cell: {cell:?}, digit: {digit}");
                let before = board(&d.app.game);
                d.tap_micro(cell, digit);
                if peer_has(&d.app.game, cell, digit) {
                    assert_eq!(d.app.ui.refused, Some((cell, digit, d.now)), "MRK-10 ({case})");
                    assert_eq!(board(&d.app.game), before, "MRK-7 ({case})");
                    assert_x_rule(&d, &case);
                } else {
                    assert_eq!(d.app.ui.refused, None, "MRK-10: no peer has the digit ({case})");
                    assert_eq!(d.app.game.marks_at(cell) & mark_bit(digit), mark_bit(digit), "INP-3 ({case})");
                    d.tap_micro(cell, digit);
                    assert_eq!(d.app.game.marks_at(cell) & mark_bit(digit), 0, "INP-3 ({case})");
                    assert_eq!(d.app.ui.refused, None, "MRK-10 ({case})");
                }
            }
        }
    });
}

#[test]
fn refuses_mark_follows_the_rule() {
    // MRK-7, MRK-18: the rule for an empty cell, a filled cell, the switch and a complete game.
    let d = Driver::new(104);
    let g = &d.app.game;
    let (cell, digit) = refused_pair(g);
    assert!(g.refuses_mark(cell, digit, true));
    assert!(!g.refuses_mark(cell, digit, false), "MRK-7: the switch is off");
    let free = (1..=9u8).find(|&v| !peer_has(g, cell, v)).unwrap();
    assert!(!g.refuses_mark(cell, free, true), "MRK-7: no peer has the digit");
    assert!(!g.refuses_mark(cell, 0, true) && !g.refuses_mark(cell, 10, true));
    let given = given_cell(g);
    assert!((1..=9u8).all(|v| !g.refuses_mark(given, v, true)), "MRK-7: only an empty cell");
    let mut full = g.clone();
    full.completed = true;
    assert!(!full.refuses_mark(cell, digit, true), "MRK-10: a complete game");
}

#[test]
fn a_tap_on_a_hidden_mark_shows_the_x_and_the_mark_stays() {
    // MRK-7 (owner decision): the cell has the mark d, a peer has the number d and the player taps d.
    // The tap has no effect and the x shows. When the number goes, the mark shows again.
    let mut d = Driver::new(105);
    let (cell, peer, v) = cell_and_peer(&d.app);
    d.tap_micro(peer, v);
    assert_eq!(d.app.game.marks_at(peer), mark_bit(v));
    d.write(cell, v);
    assert_eq!(d.app.game.value_at(cell), v);
    assert_eq!(d.app.game.visible_marks(peer, true) & mark_bit(v), 0, "MRK-4: the mark is hidden");
    let before = board(&d.app.game);
    d.tap_micro(peer, v);
    assert_eq!(d.app.ui.refused, Some((peer, v, d.now)), "MRK-7");
    assert_eq!(board(&d.app.game), before, "MRK-7: the tap has no effect");
    assert_eq!(d.app.game.marks_at(peer), mark_bit(v), "MRK-7: the hidden mark stays");
    d.wait(OVERLAY_MS);
    d.erase(cell);
    assert_ne!(d.app.game.visible_marks(peer, true) & mark_bit(v), 0, "MRK-8: the mark shows again");
}

#[test]
fn with_the_setting_off_a_tap_changes_the_mark_and_no_x_shows() {
    // MRK-7, MRK-10, R§9.1: with the switch off, the tap adds and removes the mark, also a hidden mark.
    // No x shows. When the switch is on again, the tap is a refused tap again.
    let mut d = Driver::new(106);
    let (cell, digit) = refused_pair(&d.app.game);
    d.button(Button::Menu);
    d.button(Button::Settings);
    let (sx, sy) = switch_point(Switch::HideMarksAutomatically);
    d.finger_tap(sx, sy);
    assert!(!d.app.settings.auto_hide_on);
    d.button(Button::Settings);
    assert_eq!(d.app.ui.menu.panel(), None);

    d.tap_micro(cell, digit);
    assert_eq!(d.app.ui.refused, None, "MRK-7: no x when the switch is off");
    assert_eq!(d.app.game.marks_at(cell), mark_bit(digit), "MRK-7: the tap adds the mark");
    assert_eq!(d.app.game.undo.len(), 1);
    d.tap_micro(cell, digit);
    assert_eq!(d.app.ui.refused, None, "MRK-7: no x when the switch is off");
    assert_eq!(d.app.game.marks_at(cell), 0, "MRK-7: the tap removes the mark");
    assert_eq!(d.app.game.undo.len(), 2);

    d.tap_micro(cell, digit);
    d.button(Button::Settings);
    d.finger_tap(sx, sy);
    assert!(d.app.settings.auto_hide_on);
    d.button(Button::Settings);
    let before = board(&d.app.game);
    d.tap_micro(cell, digit);
    assert_eq!(d.app.ui.refused, Some((cell, digit, d.now)), "MRK-7: the switch is on again");
    assert_eq!(board(&d.app.game), before, "MRK-7");
    assert_eq!(d.app.game.marks_at(cell), mark_bit(digit), "MRK-7: the hidden mark stays");
}

#[test]
fn a_finger_and_the_eraser_never_show_the_x() {
    // MRK-10, INP-1, INP-8: a finger and the eraser on a micro-cell of a refused tap make no x.
    let mut d = Driver::new(107);
    let (cell, digit) = refused_pair(&d.app.game);
    let (x, y) = micro_center(cell, digit);
    let before = board(&d.app.game);
    d.send(Input::TouchDown { id: 1, x, y });
    assert_eq!(d.app.ui.refused, None, "MRK-10: finger press");
    d.send(Input::TouchUp { id: 1, x, y });
    assert_eq!(d.app.ui.refused, None, "MRK-10: finger lift");
    d.send(Input::PenDown { x, y, tool: Tool::Eraser });
    assert_eq!(d.app.ui.refused, None, "MRK-10: eraser press");
    d.send(Input::PenUp { x, y });
    assert_eq!(d.app.ui.refused, None, "MRK-10: eraser lift");
    assert_eq!(board(&d.app.game), before, "INP-1");
}

#[test]
fn writing_never_shows_the_x() {
    // MRK-10, INP-2: a touch that leaves the micro-cell is writing, also if it starts or ends in the
    // micro-cell of a refused tap. A touch that stays in the micro-cell is a tap.
    let mut d = Driver::new(108);
    let (cell, digit) = refused_pair(&d.app.game);
    let (x, y) = micro_center(cell, digit);
    let (ox, oy) = micro_center(cell, if digit == 5 { 1 } else { 5 });

    // Out of the micro-cell and back.
    d.send(Input::PenDown { x, y, tool: Tool::Pen });
    d.send(Input::PenMove { x: ox, y: oy });
    d.send(Input::PenMove { x, y });
    d.send(Input::PenUp { x, y });
    assert_eq!(d.app.ui.refused, None, "MRK-10: this touch is writing");
    assert!(d.app.ui.pending_ink.is_some(), "INP-2: the ink waits");
    d.wait(d.app.pause_ms + 10);
    assert_eq!(d.app.ui.refused, None, "MRK-10: after the app reads the ink");

    // A movement in the micro-cell: a tap. The app can read the ink as a number, thus the test finds
    // the pair again.
    d.wait(OVERLAY_MS);
    let (cell, digit) = refused_pair(&d.app.game);
    let (x, y) = micro_center(cell, digit);
    d.send(Input::PenDown { x, y, tool: Tool::Pen });
    d.send(Input::PenMove { x: x + 5.0, y: y - 5.0 });
    d.send(Input::PenUp { x: x + 5.0, y: y - 5.0 });
    assert_eq!(d.app.ui.refused, Some((cell, digit, d.now)), "INP-2: this touch is a tap");
}

#[test]
fn a_short_touch_in_the_cell_of_waiting_ink_shows_no_x() {
    // MRK-10, INP-11: this touch is writing, also on the micro-cell of a refused tap.
    let mut d = Driver::new(109);
    let (cell, digit) = refused_pair(&d.app.game);
    let ink = scribble_ink(cell);
    d.write_strokes(&ink);
    d.tap_micro(cell, digit);
    assert_eq!(d.app.ui.refused, None, "MRK-10");
    assert_eq!(d.app.ui.pending_ink.as_ref().map(|i| i.strokes.len()), Some(ink.len() + 1), "INP-11: the touch is a part of the ink");
    d.wait(d.app.pause_ms + 10);
    assert_eq!(d.app.ui.refused, None, "MRK-10: after the app reads the ink");
}

#[test]
fn a_tap_on_a_filled_cell_shows_no_x() {
    // MRK-10, INP-4: a starting number and a number of the player, on each micro-cell, with the
    // highlight on and off.
    let mut d = Driver::new(110);
    let given = given_cell(&d.app.game);
    let (cell, _, v) = cell_and_peer(&d.app);
    d.write(cell, v);
    assert_eq!(d.app.game.value_at(cell), v);
    let before = board(&d.app.game);
    for highlight_on in [true, false] {
        d.app.settings.highlight_on = highlight_on;
        for c in [given, cell] {
            for digit in 1..=9u8 {
                d.tap_micro(c, digit);
                assert_eq!(d.app.ui.refused, None, "MRK-10: {c:?}, {digit}, highlight: {highlight_on}");
            }
        }
    }
    assert_eq!(board(&d.app.game), before, "INP-4");
}

#[test]
fn a_tap_on_a_complete_game_shows_no_x() {
    // MRK-10, INP-9, CMP-3: a complete game has no x, with the pen and with the eraser.
    let mut d = Driver::new(111);
    solve(&mut d);
    assert!(d.app.game.completed);
    let before = board(&d.app.game);
    for cell in [(0, 0), (4, 4), (8, 8)] {
        for digit in 1..=9u8 {
            d.tap_micro(cell, digit);
            assert_eq!(d.app.ui.refused, None, "MRK-10");
        }
        d.erase(cell);
        assert_eq!(d.app.ui.refused, None, "MRK-10");
    }
    assert_eq!(board(&d.app.game), before, "INP-9");
    assert_eq!(d.app.ui.highlight, None, "INP-9");
}

#[test]
fn a_tap_with_an_open_panel_only_closes_the_panel() {
    // MRK-10, MRK-12, NAV-11: with a panel open, the tap outside the panel only closes it. Then, with
    // the menu open and no panel, the same tap shows the x. A tap in the panel also shows no x. All
    // layouts and all panels.
    for_each_layout(|case| {
        for (b, panel) in [(Button::Colors, Panel::Colors), (Button::Settings, Panel::Settings), (Button::NewGame, Panel::NewGame)] {
            let case = format!("{case}, panel: {panel:?}");
            let mut d = Driver::new(112);
            d.button(Button::Menu);
            d.button(b);
            assert_eq!(d.app.ui.menu.panel(), Some(panel));
            let area = match panel {
                Panel::Colors => layout::colors_panel(),
                Panel::Settings => layout::settings_panel(d.app.update.has_row()),
                Panel::NewGame => layout::ng_panel(),
            };
            let before = board(&d.app.game);
            let pairs = refused_pairs(&d.app.game);
            let in_panel = |p: &(Cell, u8)| {
                let (x, y) = micro_center(p.0, p.1);
                area.contains(x, y)
            };
            let &(cell, digit) = pairs.iter().find(|p| !in_panel(p)).expect("a refused pair outside the panel");
            d.tap_micro(cell, digit);
            assert_eq!(d.app.ui.refused, None, "MRK-10 ({case})");
            assert_eq!(d.app.ui.menu, Menu::Open(None), "NAV-11: the tap closes the panel ({case})");
            assert_eq!(board(&d.app.game), before, "NAV-11 ({case})");
            d.tap_micro(cell, digit);
            assert_eq!(d.app.ui.refused, Some((cell, digit, d.now)), "MRK-10: the menu is open and no panel is open ({case})");
            assert_eq!(d.app.ui.menu, Menu::Open(None), "MRK-10 ({case})");
            assert_x_rule(&d, &case);

            // A tap on a micro-cell under the panel. It can operate an item of the panel, thus this
            // part has a new driver, and it does not use the row "Start new game". The tap makes no x.
            let inside = pairs.iter().find(|p| {
                let (x, y) = micro_center(p.0, p.1);
                in_panel(p) && !layout::ng_start_row().contains(x, y)
            });
            if let Some(&(cell, digit)) = inside {
                let mut d = Driver::new(112);
                d.button(Button::Menu);
                d.button(b);
                d.tap_micro(cell, digit);
                assert_eq!(d.app.ui.refused, None, "MRK-10: a tap in the panel ({case})");
                assert_eq!(board(&d.app.game), before, "NAV-11 ({case})");
            }
        }
    });
}

#[test]
fn with_the_menu_open_a_refused_tap_shows_the_x() {
    // MRK-10: the app opens with the menu open (PER-2), and no panel is open.
    let mut d = Driver::opened(new_app(113));
    assert_eq!(d.app.ui.menu, Menu::Open(None));
    let (cell, digit) = refused_tap(&mut d);
    assert_eq!(x_of(&d), Some((cell, digit)));
    assert_eq!(d.app.ui.menu, Menu::Open(None), "MRK-10: the menu stays open");
}

#[test]
fn a_refused_tap_removes_the_highlight() {
    // MRK-12, MRK-15, HLT-2: an x never shows with a highlight.
    let mut d = Driver::new(114);
    let given = given_cell(&d.app.game);
    d.tap_cell(given);
    assert!(d.app.ui.highlight.is_some());
    refused_tap(&mut d);
    assert_eq!(d.app.ui.highlight, None, "MRK-12");
    assert_x_rule(&d, "highlight");
}

#[test]
fn the_x_goes_at_the_end_of_its_time_with_no_other_input() {
    // MRK-14, QST-1: the x stays until OVERLAY_MS. The main loop wakes at this time (`next_deadline`),
    // and the x goes with a redraw and with no other input.
    assert_eq!(OVERLAY_MS, 1000, "MRK-14");
    let mut d = Driver::new(115);
    assert_eq!(d.app.next_deadline(), None);
    let (cell, digit) = refused_pair(&d.app.game);
    let (x, y) = micro_center(cell, digit);
    d.send(Input::PenDown { x, y, tool: Tool::Pen });
    assert_eq!(d.app.next_deadline(), None, "MRK-10: no time before the lift");
    d.app.redraw = false;
    d.send(Input::PenUp { x, y });
    let shown = d.now;
    assert_eq!(d.app.ui.refused, Some((cell, digit, shown)));
    assert!(d.app.redraw, "the x needs a redraw");
    assert_eq!(d.app.next_deadline(), Some(shown + OVERLAY_MS), "MRK-14: the main loop wakes at the end of the time");

    d.app.redraw = false;
    tick_to(&mut d, shown + OVERLAY_MS - 1);
    assert_eq!(d.app.ui.refused, Some((cell, digit, shown)), "MRK-14: the x stays before the end");
    assert!(!d.app.redraw);
    assert_eq!(d.app.next_deadline(), Some(shown + OVERLAY_MS));
    tick_to(&mut d, shown + OVERLAY_MS);
    assert_eq!(d.app.ui.refused, None, "MRK-14: the x goes at the end");
    assert!(d.app.redraw, "MRK-14: the end of the x needs a redraw");
    assert_eq!(d.app.next_deadline(), None, "MRK-14: no timer after the x");

    // The "?" uses the same time (QST-1).
    let other = empty_cell(&d.app.game);
    d.write_strokes(&scribble_ink(other));
    d.wait(d.app.pause_ms);
    let (_, at) = d.app.ui.question.expect("the ?");
    tick_to(&mut d, at + OVERLAY_MS - 1);
    assert!(d.app.ui.question.is_some(), "QST-1");
    tick_to(&mut d, at + OVERLAY_MS);
    assert_eq!(d.app.ui.question, None, "QST-1, MRK-14: one value");
}

#[test]
fn each_press_removes_the_x_before_its_action() {
    // MRK-15, INP-1, HLT-6: the x goes at the press, in all layouts. The state goes at the press also
    // when the press starts writing (the pixels wait for the redraw, R§5.1).
    for_each_layout(|case| {
        let mut base = Driver::new(116);
        let (cell, digit) = refused_tap(&mut base);
        let g = base.app.game.clone();
        let given = given_cell(&g);
        let free = (1..=9u8).find(|&v| !peer_has(&g, cell, v)).unwrap();
        let other_cell = cells().find(|&c| !g.is_filled(c) && c != cell).unwrap();
        let on_cell = micro_center(cell, digit);
        let presses: [(&str, Input); 10] = [
            ("pen, same micro-cell", Input::PenDown { x: on_cell.0, y: on_cell.1, tool: Tool::Pen }),
            ("pen, a free micro-cell of the cell", { let (x, y) = micro_center(cell, free); Input::PenDown { x, y, tool: Tool::Pen } }),
            ("pen, a different empty cell", { let (x, y) = micro_center(other_cell, 5); Input::PenDown { x, y, tool: Tool::Pen } }),
            ("pen, a filled cell", { let (x, y) = micro_center(given, 5); Input::PenDown { x, y, tool: Tool::Pen } }),
            ("pen, a toolbar button", { let (x, y) = button_center(Button::Menu); Input::PenDown { x, y, tool: Tool::Pen } }),
            ("eraser, same cell", Input::PenDown { x: on_cell.0, y: on_cell.1, tool: Tool::Eraser }),
            ("eraser, a filled cell", { let (x, y) = micro_center(given, 5); Input::PenDown { x, y, tool: Tool::Eraser } }),
            ("finger, same cell", Input::TouchDown { id: 1, x: on_cell.0, y: on_cell.1 }),
            ("finger, a toolbar button", { let (x, y) = button_center(Button::Menu); Input::TouchDown { id: 1, x, y } }),
            ("finger, a filled cell", { let (x, y) = micro_center(given, 5); Input::TouchDown { id: 1, x, y } }),
        ];
        for (name, press) in presses {
            let case = format!("{case}, {name}");
            let mut d = Driver::new(116);
            d.tap_micro(cell, digit);
            assert_eq!(x_of(&d), Some((cell, digit)), "{case}");
            let before = board(&d.app.game);
            d.send(press);
            assert_eq!(d.app.ui.refused, None, "MRK-15 ({case})");
            assert_eq!(board(&d.app.game), before, "MRK-15, INP-1: the press changes nothing else ({case})");
            assert_x_rule(&d, &case);
        }
    });
}

#[test]
fn a_tap_on_a_number_removes_the_x_with_the_highlight_off() {
    // MRK-15, HLT-6, INP-4: a tap on a number changes nothing else, but it still removes the x.
    let mut d = Driver::new(117);
    d.app.settings.highlight_on = false;
    let given = given_cell(&d.app.game);
    refused_tap(&mut d);
    d.tap_cell(given);
    assert_eq!(d.app.ui.refused, None, "HLT-6");
    assert_eq!(d.app.ui.highlight, None, "HLT-6");
}

#[test]
fn a_second_finger_removes_the_x() {
    // MRK-15, INP-15: a second finger removes the x and operates no item. Also a touch that lost its
    // release does not stop the removal.
    let mut d = Driver::new(118);
    let palm = empty_cell(&d.app.game);
    let (px, py) = micro_center(palm, 5);

    // On the board.
    d.send(Input::TouchDown { id: 5, x: px, y: py });
    refused_tap(&mut d);
    let (x, y) = micro_center(given_cell(&d.app.game), 5);
    d.send(Input::TouchDown { id: 6, x, y });
    assert_eq!(d.app.ui.refused, None, "MRK-15: a second finger on the board");
    d.send(Input::TouchUp { id: 6, x, y });

    // On a toolbar button, outside the guard time of NAV-16.
    d.send(Input::TouchMove { id: 5, x: px, y: py });
    refused_tap(&mut d);
    d.send(Input::TouchMove { id: 5, x: px, y: py });
    let menu = d.app.ui.menu;
    let (x, y) = button_center(Button::Menu);
    d.send(Input::TouchDown { id: 6, x, y });
    assert_eq!(d.app.ui.refused, None, "MRK-15: a second finger on the toolbar");
    d.send(Input::TouchUp { id: 6, x, y });
    assert_eq!(d.app.ui.menu, menu, "INP-15: the second finger operates no item");

    // The first touch has no event for more than 0.15 s: a new touch is a usual press. It also
    // removes the x.
    refused_tap(&mut d);
    d.wait(200);
    assert!(d.app.ui.refused.is_some());
    let (x, y) = micro_center(given_cell(&d.app.game), 5);
    d.send(Input::TouchDown { id: 6, x, y });
    assert_eq!(d.app.ui.refused, None, "MRK-15, INP-15: a new touch after a lost release");
}

#[test]
fn a_new_refused_tap_starts_the_time_again() {
    // MRK-14, MRK-15, R§6.1: the press removes the x, the lift shows it again, and the time starts
    // again. At most one x shows, also when the new tap is in a different cell.
    let mut d = Driver::new(119);
    let pairs = refused_pairs(&d.app.game);
    let (cell, digit) = pairs[0];
    let &(other_cell, other_digit) = pairs.iter().find(|p| p.0 != cell).unwrap();
    d.tap_micro(cell, digit);
    let first = d.now;
    d.wait(OVERLAY_MS - 100);
    let (x, y) = micro_center(cell, digit);
    d.send(Input::PenDown { x, y, tool: Tool::Pen });
    assert_eq!(d.app.ui.refused, None, "MRK-15: the press removes the x");
    d.send(Input::PenUp { x, y });
    let second = d.now;
    assert_eq!(d.app.ui.refused, Some((cell, digit, second)), "MRK-14: the time starts again");
    assert_eq!(d.app.next_deadline(), Some(second + OVERLAY_MS));
    tick_to(&mut d, first + OVERLAY_MS);
    assert_eq!(x_of(&d), Some((cell, digit)), "MRK-14: the first time does not remove the new x");
    d.tap_micro(other_cell, other_digit);
    let third = d.now;
    assert_eq!(d.app.ui.refused, Some((other_cell, other_digit, third)), "MRK-14: at most one x");
    tick_to(&mut d, second + OVERLAY_MS);
    assert!(d.app.ui.refused.is_some(), "MRK-14: the x has its own time");
    tick_to(&mut d, third + OVERLAY_MS);
    assert_eq!(d.app.ui.refused, None);
}

#[test]
fn a_press_that_the_guard_time_ignores_does_not_remove_the_x() {
    // NAV-16, MRK-15, QST-4: in the guard time after "< Back", a first press on a toolbar button or on
    // the close button does not remove the x or the "?". A press on the board operates as usual. After
    // the guard time, the same press removes them. Left-handed and right-handed.
    for right_handed in [false, true] {
        layout::set_right_handed(right_handed);
        let case = format!("right-handed: {right_handed}");
        let mut d = Driver::new(120);
        back_from(&mut d, Button::Help, false);
        let back_at = d.now;

        // The press of the refused tap reads the ink, thus the "?" and the x show together.
        let (cell, digit) = refused_pair(&d.app.game);
        let other = cells().find(|&c| !d.app.game.is_filled(c) && c != cell).unwrap();
        d.write_strokes(&scribble_ink(other));
        d.tap_micro(cell, digit);
        let x = d.app.ui.refused;
        let question = d.app.ui.question;
        assert_eq!(x, Some((cell, digit, d.now)), "{case}");
        assert_eq!(question.map(|q| q.0), Some(other), "{case}");

        for b in [Button::Menu, Button::Colors, Button::Settings, Button::Undo, Button::Redo, Button::Help, Button::Statistics, Button::NewGame, Button::Close] {
            let (px, py) = button_center(b);
            d.send(Input::TouchDown { id: 1, x: px, y: py });
            assert_eq!((d.app.ui.refused, d.app.ui.question), (x, question), "NAV-16: finger press ({case}, {b:?})");
            d.send(Input::TouchUp { id: 1, x: px, y: py });
            assert_eq!((d.app.ui.refused, d.app.ui.question), (x, question), "NAV-16: finger lift ({case}, {b:?})");
        }
        let (px, py) = button_center(Button::Close);
        d.pen_tap(px, py);
        assert_eq!((d.app.ui.refused, d.app.ui.question), (x, question), "NAV-16: pen tap ({case})");
        assert!(!d.app.quit, "NAV-16 ({case})");
        assert_eq!((d.app.screen, d.app.ui.menu), (Screen::Game, Menu::Open(None)), "NAV-16 ({case})");
        assert!(d.now < back_at + BACK_GUARD_MS, "the presses are in the guard time ({case})");

        // After the guard time, the x and the "?" are still there, and a press removes them.
        tick_to(&mut d, back_at + BACK_GUARD_MS);
        assert_eq!((d.app.ui.refused, d.app.ui.question), (x, question), "{case}");
        hold(&mut d, Button::Menu);
        assert_eq!((d.app.ui.refused, d.app.ui.question), (None, None), "MRK-15: after the guard time ({case})");

        // A press on the board in the guard time operates as usual.
        let mut d = Driver::new(120);
        back_from(&mut d, Button::Help, false);
        let back_at = d.now;
        let (cell, digit) = refused_tap(&mut d);
        let (px, py) = micro_center(cell, digit);
        d.send(Input::PenDown { x: px, y: py, tool: Tool::Pen });
        assert_eq!(d.app.ui.refused, None, "MRK-15, NAV-16: the board operates as usual ({case})");
        assert!(d.now < back_at + BACK_GUARD_MS, "{case}");
    }
    layout::set_right_handed(false);
}

#[test]
fn the_release_of_a_control_removes_the_x() {
    // MRK-15, MRK-12: a finger stays on a control while the pen does a refused tap. The release does the
    // action of the control and removes the x. All controls of the toolbar, in all layouts.
    for_each_layout(|case| {
        for b in [Button::Menu, Button::Colors, Button::Settings, Button::NewGame, Button::Undo, Button::Redo, Button::Help, Button::Statistics, Button::Close] {
            let case = format!("{case}, control: {b:?}");
            let mut d = Driver::new(121);
            fill_stacks(&mut d);
            let before = board(&d.app.game);
            let (x, y) = hold(&mut d, b);
            refused_tap(&mut d);
            assert_x_rule(&d, &case);
            d.send(Input::TouchUp { id: 1, x, y });
            assert_eq!(d.app.ui.refused, None, "MRK-15 ({case})");
            assert_x_rule(&d, &case);
            let done = match b {
                Button::Menu => d.app.ui.menu == Menu::Closed,
                Button::Colors => d.app.ui.menu.panel() == Some(Panel::Colors),
                Button::Settings => d.app.ui.menu.panel() == Some(Panel::Settings),
                Button::NewGame => d.app.ui.menu.panel() == Some(Panel::NewGame),
                Button::Undo | Button::Redo => board(&d.app.game) != before,
                Button::Help => d.app.screen == Screen::Manual,
                Button::Statistics => d.app.screen == Screen::Statistics,
                Button::Close => d.app.quit,
            };
            assert!(done, "the release does the action of the control ({case})");
        }
    });
}

#[test]
fn the_release_of_a_control_keeps_a_question_mark_in_a_different_cell() {
    // MRK-15, QST-4, R§9: the release removes only the x. The "?" stays, with its own time. A change of
    // screen removes the two (NAV-5).
    for b in [Button::Colors, Button::Settings, Button::NewGame, Button::Undo, Button::Redo] {
        let mut d = Driver::new(122);
        fill_stacks(&mut d);
        let (x, y) = hold(&mut d, b);
        let (cell, digit) = refused_pair(&d.app.game);
        let other = cells().find(|&c| !d.app.game.is_filled(c) && c != cell).unwrap();
        d.write_strokes(&scribble_ink(other));
        d.tap_micro(cell, digit);
        let question = d.app.ui.question;
        assert_eq!(question.map(|q| q.0), Some(other), "{b:?}");
        assert_eq!(x_of(&d), Some((cell, digit)), "{b:?}");
        d.send(Input::TouchUp { id: 1, x, y });
        assert_eq!(d.app.ui.refused, None, "MRK-15 ({b:?})");
        assert_eq!(d.app.ui.question, question, "MRK-15: the \"?\" in a different cell stays ({b:?})");
        tick_to(&mut d, question.unwrap().1 + OVERLAY_MS);
        assert_eq!(d.app.ui.question, None, "QST-1 ({b:?})");
    }
    for b in [Button::Help, Button::Statistics] {
        let mut d = Driver::new(122);
        fill_stacks(&mut d);
        let (x, y) = hold(&mut d, b);
        let (cell, digit) = refused_pair(&d.app.game);
        let other = cells().find(|&c| !d.app.game.is_filled(c) && c != cell).unwrap();
        d.write_strokes(&scribble_ink(other));
        d.tap_micro(cell, digit);
        assert!(d.app.ui.question.is_some() && d.app.ui.refused.is_some(), "{b:?}");
        d.send(Input::TouchUp { id: 1, x, y });
        assert_eq!((d.app.ui.refused, d.app.ui.question), (None, None), "MRK-15, NAV-5 ({b:?})");
    }
}

#[test]
fn a_release_outside_the_control_keeps_the_x() {
    // MRK-15: only the release of a control that does an action removes the x. A finger that lifts
    // outside the control does no action.
    let mut d = Driver::new(123);
    fill_stacks(&mut d);
    let (cell, digit) = refused_pair(&d.app.game);
    hold(&mut d, Button::Colors);
    d.tap_micro(cell, digit);
    let x = d.app.ui.refused;
    assert!(x.is_some());
    let (ox, oy) = micro_center(given_cell(&d.app.game), 5);
    d.send(Input::TouchUp { id: 1, x: ox, y: oy });
    assert_eq!(d.app.ui.menu.panel(), None, "the control does no action");
    assert_eq!(d.app.ui.refused, x, "MRK-15");
}

#[test]
fn an_undo_that_removes_the_number_of_the_peer_leaves_no_x() {
    // MRK-15, MRK-12: the release of Undo removes the x, thus no x stays with no number in a peer, and
    // no mark shows under the x. The next tap on this micro-cell adds the mark.
    let mut d = Driver::new(124);
    let (cell, peer, v) = cell_and_peer(&d.app);
    d.tap_micro(peer, v);
    d.write(cell, v);
    assert_eq!(d.app.game.value_at(cell), v);
    d.button(Button::Menu);
    let (x, y) = hold(&mut d, Button::Undo);
    d.tap_micro(peer, v);
    assert_eq!(x_of(&d), Some((peer, v)), "MRK-7: a hidden mark");
    d.send(Input::TouchUp { id: 1, x, y });
    assert_eq!(d.app.game.value_at(cell), 0, "the undo removed the number");
    assert_ne!(d.app.game.visible_marks(peer, true) & mark_bit(v), 0, "the mark shows again");
    assert_eq!(d.app.ui.refused, None, "MRK-12: a mark never shows under an x");
    assert_x_rule(&d, "undo");
    d.tap_micro(peer, v);
    assert_eq!(d.app.ui.refused, None, "MRK-10: no peer has the digit now");
    assert_eq!(d.app.game.marks_at(peer) & mark_bit(v), 0, "the tap removes the mark");
}

#[test]
fn a_redo_that_fills_the_cell_of_the_x_leaves_no_x() {
    // MRK-15, MRK-12: the cell of the x stays empty.
    let mut d = Driver::new(125);
    let (cell, digit) = refused_pair(&d.app.game);
    let v = d.app.game.solution[cell.0 as usize][cell.1 as usize];
    d.write(cell, v);
    d.button(Button::Menu);
    d.button(Button::Undo);
    assert_eq!(d.app.game.value_at(cell), 0);
    let (x, y) = hold(&mut d, Button::Redo);
    d.tap_micro(cell, digit);
    assert_eq!(x_of(&d), Some((cell, digit)));
    d.send(Input::TouchUp { id: 1, x, y });
    assert_eq!(d.app.game.value_at(cell), v);
    assert_eq!(d.app.ui.refused, None, "MRK-12");
    assert_x_rule(&d, "redo");
}

#[test]
fn a_redo_that_completes_the_game_leaves_no_x() {
    // MRK-15, CMP-3, MRK-12: the x goes, and a tap in progress at the completion makes no x (INP-9).
    let mut d = Driver::new(126);
    d.button(Button::Menu);
    let last = redo_completes(&mut d);
    let v = d.app.game.solution[last.0 as usize][last.1 as usize];
    let digit = (1..=9u8).find(|&x| x != v).unwrap();
    assert!(peer_has(&d.app.game, last, digit));
    let (x, y) = hold(&mut d, Button::Redo);
    d.tap_micro(last, digit);
    assert_eq!(d.app.ui.refused, Some((last, digit, d.now)));
    d.send(Input::TouchUp { id: 1, x, y });
    assert!(d.app.game.completed, "the redo completes the game");
    assert_eq!(d.app.ui.refused, None, "CMP-3");
    assert_eq!(d.app.next_deadline(), None, "no timer on a complete game");
    assert_x_rule(&d, "completion");

    // The pen is on the board when the redo completes the game.
    let mut d = Driver::new(127);
    d.button(Button::Menu);
    let last = redo_completes(&mut d);
    let v = d.app.game.solution[last.0 as usize][last.1 as usize];
    let digit = (1..=9u8).find(|&x| x != v).unwrap();
    let (x, y) = hold(&mut d, Button::Redo);
    let (px, py) = micro_center(last, digit);
    d.send(Input::PenDown { x: px, y: py, tool: Tool::Pen });
    d.send(Input::TouchUp { id: 1, x, y });
    assert!(d.app.game.completed);
    d.send(Input::PenUp { x: px, y: py });
    assert_eq!(d.app.ui.refused, None, "MRK-10, INP-9: a tap in progress at the completion");
    assert_eq!(d.app.ui.highlight, None, "INP-9");
}

#[test]
fn the_x_comes_from_the_board_at_the_lift() {
    // MRK-10, MRK-12: the x shows at the lift, thus the numbers at the lift decide. An undo during the
    // press removes the number of the peer: the tap adds the mark. A redo during the press puts the number
    // in a peer: the x shows.
    let mut d = Driver::new(128);
    let (cell, peer, v) = cell_and_peer(&d.app);
    d.write(cell, v);
    assert_eq!(d.app.game.value_at(cell), v);
    d.button(Button::Menu);
    let (px, py) = micro_center(peer, v);

    let (x, y) = hold(&mut d, Button::Undo);
    d.send(Input::PenDown { x: px, y: py, tool: Tool::Pen });
    d.send(Input::TouchUp { id: 1, x, y });
    assert_eq!(d.app.game.value_at(cell), 0);
    d.send(Input::PenUp { x: px, y: py });
    assert_eq!(d.app.ui.refused, None, "MRK-7: no peer has the digit at the lift");
    assert_eq!(d.app.game.marks_at(peer), mark_bit(v), "the tap adds the mark");
    d.tap_micro(peer, v);
    assert_eq!(d.app.game.marks_at(peer), 0);

    // The two marks are new changes, thus they emptied the redo history. Write the number again.
    d.write(cell, v);
    assert_eq!(d.app.game.value_at(cell), v);
    d.button(Button::Undo);
    assert_eq!(d.app.game.value_at(cell), 0);
    let (x, y) = hold(&mut d, Button::Redo);
    d.send(Input::PenDown { x: px, y: py, tool: Tool::Pen });
    d.send(Input::TouchUp { id: 1, x, y });
    assert_eq!(d.app.game.value_at(cell), v);
    assert_eq!(d.app.ui.refused, None, "MRK-10: no x before the lift");
    d.send(Input::PenUp { x: px, y: py });
    assert_eq!(d.app.ui.refused, Some((peer, v, d.now)), "MRK-7: a peer has the digit at the lift");
    assert_x_rule(&d, "redo during the press");
}

#[test]
fn the_question_mark_and_the_x_show_together_each_with_its_own_time() {
    // MRK-14, MRK-12, QST-1, R§3.2 (owner decision): the press of the tap reads the ink of a different
    // cell. The "?" starts at the press, and the x starts at the lift. In all layouts.
    for_each_layout(|case| {
        let mut d = Driver::new(129);
        let (b, digit) = refused_pair(&d.app.game);
        let a = cells().find(|&c| !d.app.game.is_filled(c) && c != b).unwrap();
        d.write_strokes(&scribble_ink(a));
        let (x, y) = micro_center(b, digit);
        d.send(Input::PenDown { x, y, tool: Tool::Pen });
        let question_at = d.now;
        assert_eq!(d.app.ui.question, Some((a, question_at)), "INP-12 ({case})");
        assert_eq!(d.app.ui.refused, None, "MRK-10: no x at the press ({case})");
        d.wait(300);
        d.send(Input::PenUp { x, y });
        let x_at = d.now;
        assert_eq!(d.app.ui.refused, Some((b, digit, x_at)), "MRK-14 ({case})");
        assert_eq!(d.app.ui.question, Some((a, question_at)), "MRK-14 ({case})");
        assert_x_rule(&d, case);

        assert_eq!(d.app.next_deadline(), Some(question_at + OVERLAY_MS), "MRK-14: the first of the two times ({case})");
        tick_to(&mut d, question_at + OVERLAY_MS - 1);
        assert!(d.app.ui.question.is_some() && d.app.ui.refused.is_some(), "{case}");
        tick_to(&mut d, question_at + OVERLAY_MS);
        assert_eq!(d.app.ui.question, None, "QST-1 ({case})");
        assert_eq!(d.app.ui.refused, Some((b, digit, x_at)), "MRK-14: the x has its own time ({case})");
        assert_eq!(d.app.next_deadline(), Some(x_at + OVERLAY_MS), "MRK-14 ({case})");
        tick_to(&mut d, x_at + OVERLAY_MS);
        assert_eq!(d.app.ui.refused, None, "MRK-14 ({case})");
    });
}

#[test]
fn a_press_removes_the_question_mark_and_the_x_together() {
    // MRK-15, QST-4, R§3.2: pen, finger and second finger.
    let setup = || {
        let mut d = Driver::new(130);
        let (b, digit) = refused_pair(&d.app.game);
        let a = cells().find(|&c| !d.app.game.is_filled(c) && c != b).unwrap();
        d.write_strokes(&scribble_ink(a));
        d.tap_micro(b, digit);
        assert!(d.app.ui.question.is_some() && d.app.ui.refused.is_some());
        d
    };
    let (x, y) = (5.0, 1000.0);
    let mut d = setup();
    d.send(Input::PenDown { x, y, tool: Tool::Pen });
    assert_eq!((d.app.ui.question, d.app.ui.refused), (None, None), "pen");
    let mut d = setup();
    d.send(Input::TouchDown { id: 1, x, y });
    assert_eq!((d.app.ui.question, d.app.ui.refused), (None, None), "finger");

    // The refused tap on the cell of the "?": the press removes the "?", and the x shows alone.
    let mut d = setup();
    let (a, _) = d.app.ui.question.unwrap();
    d.wait(OVERLAY_MS);
    d.write_strokes(&scribble_ink(a));
    d.wait(d.app.pause_ms);
    assert_eq!(d.app.ui.question.map(|q| q.0), Some(a));
    let digit = (1..=9u8).find(|&v| peer_has(&d.app.game, a, v)).unwrap();
    d.tap_micro(a, digit);
    assert_eq!(d.app.ui.question, None, "QST-4");
    assert_eq!(d.app.ui.refused, Some((a, digit, d.now)), "MRK-12: never on the same cell");
    assert_x_rule(&d, "same cell");
}

#[test]
fn a_number_that_the_tap_reads_causes_the_x() {
    // MRK-10, MRK-11, INP-12, owner decision: the press reads the ink in a peer as a number. The peer then
    // has the digit, thus the tap is a refused tap: the number and the x show together.
    let mut d = Driver::new(131);
    let (a, b, v) = cell_and_peer(&d.app);
    assert!(!peer_has(&d.app.game, b, v), "no peer has the digit before the app reads the ink");
    d.write_strokes(&digit_ink(a, v));
    d.tap_micro(b, v);
    assert_eq!(d.app.game.value_at(a), v, "INP-12: the press reads the ink");
    assert_eq!(d.app.ui.refused, Some((b, v, d.now)), "MRK-10");
    assert_eq!(d.app.game.marks_at(b), 0, "MRK-11: no mark");
    assert_x_rule(&d, "number and x");
}

#[test]
fn ink_that_completes_the_game_gives_no_x() {
    // MRK-10, INP-12, CMP-3, owner decision: the press of a tap reads the ink, and the game is
    // complete. The tap makes no x and no highlight.
    let mut d = Driver::new(132);
    let last = empty_cell(&d.app.game);
    fill_except(&mut d, last);
    let v = d.app.game.solution[last.0 as usize][last.1 as usize];
    d.write_strokes(&digit_ink(last, v));
    let other = given_cell(&d.app.game);
    let digit = if v == 1 { 2 } else { 1 };
    d.tap_micro(other, digit);
    assert!(d.app.game.completed, "INP-12: the press reads the ink");
    assert_eq!(d.app.ui.refused, None, "CMP-3");
    assert_eq!(d.app.ui.highlight, None, "CMP-3");
    assert_eq!(d.app.next_deadline(), None, "CMP-3");
}

#[test]
fn a_turn_keeps_the_x_for_the_rest_of_its_time() {
    // MRK-16, NAV-13: the x stays on its cell and digit with the same time, in each direction of the
    // turn.
    for (from, to) in [(layout::Orientation::Portrait, layout::Orientation::Landscape), (layout::Orientation::Landscape, layout::Orientation::Portrait)] {
        layout::set_orientation(from);
        let mut d = Driver::new(133);
        refused_tap(&mut d);
        let x = d.app.ui.refused;
        let shown = x.unwrap().2;
        d.wait(400);
        layout::set_orientation(to);
        let stops = d.app.ng_stops;
        d.app.rotate(stops);
        assert_eq!(d.app.ui.refused, x, "MRK-16");
        assert_eq!(d.app.next_deadline(), Some(shown + OVERLAY_MS), "MRK-16: the rest of the time");
        assert_x_rule(&d, "turn");
        tick_to(&mut d, shown + OVERLAY_MS - 1);
        assert_eq!(d.app.ui.refused, x, "MRK-16");
        tick_to(&mut d, shown + OVERLAY_MS);
        assert_eq!(d.app.ui.refused, None, "MRK-14");
    }
    layout::set_orientation(layout::Orientation::Portrait);
}

#[test]
fn a_tap_in_progress_at_a_turn_shows_no_x() {
    // MRK-16, NAV-13: the turn stops the pen touch. The lift has no effect, at the old or the new position.
    for (old_position, name) in [(true, "old position"), (false, "new position")] {
        let mut d = Driver::new(134);
        let (cell, digit) = refused_pair(&d.app.game);
        let before = board(&d.app.game);
        let (x, y) = micro_center(cell, digit);
        d.send(Input::PenDown { x, y, tool: Tool::Pen });
        layout::set_orientation(layout::Orientation::Landscape);
        let stops = d.app.ng_stops;
        d.app.rotate(stops);
        let (ux, uy) = if old_position { (x, y) } else { micro_center(cell, digit) };
        d.send(Input::PenUp { x: ux, y: uy });
        assert_eq!(d.app.ui.refused, None, "MRK-16 ({name})");
        assert_eq!(board(&d.app.game), before, "MRK-16 ({name})");
        assert_eq!(d.app.next_deadline(), None, "MRK-16 ({name})");
        layout::set_orientation(layout::Orientation::Portrait);
    }
}

#[test]
fn the_x_is_not_saved_and_the_app_opens_with_no_x() {
    // MRK-17, PER-2: the saved data with the x are the same as with no x. An app that opens from data
    // saved during the x has no x.
    let mut d = Driver::new(135);
    let no_x = (serde_json::to_string(&board(&d.app.game)).unwrap(), serde_json::to_string(&d.app.settings).unwrap());
    refused_tap(&mut d);
    assert!(d.app.ui.refused.is_some());
    let with_x = (serde_json::to_string(&board(&d.app.game)).unwrap(), serde_json::to_string(&d.app.settings).unwrap());
    assert_eq!(with_x, no_x, "MRK-17");
    assert!(!with_x.0.contains("refused"), "MRK-17");

    let dir = std::env::temp_dir().join(format!("sudoku-test-x-{}", std::process::id()));
    let (gp, sp) = (dir.join("game.json"), dir.join("settings.json"));
    sudoku::store::save_json(&gp, &d.app.game).unwrap();
    sudoku::store::save_json(&sp, &d.app.settings).unwrap();
    let app = App::new(sudoku::store::load_json(&gp), sudoku::store::load_json(&sp), ResultsDb::in_memory(), Box::new(TestEnv { time: 0, seed: 99 }), App::placeholder_ng_stops());
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(app.ui.refused, None, "MRK-17, PER-2");
    assert_eq!(app.next_deadline(), None, "MRK-17");
    assert_eq!(app.game, d.app.game, "PER-2");
}

#[test]
fn a_new_game_a_change_of_screen_and_the_update_dialog_remove_the_x() {
    // MRK-15, NAV-5, NAV-9, UPD-10, CMP-3.
    // A new game. The x cannot show while the New game panel is open, thus the test calls the action.
    let mut d = Driver::new(136);
    refused_tap(&mut d);
    d.app.start_new_game();
    assert_eq!(d.app.ui.refused, None, "MRK-15: a new game");
    assert_eq!(d.app.next_deadline(), None, "MRK-15");

    // A change of screen: the x does not show again after "< Back".
    let mut d = Driver::new(137);
    d.button(Button::Menu);
    for page in [Button::Help, Button::Statistics] {
        let (x, y) = hold(&mut d, page);
        refused_tap(&mut d);
        d.send(Input::TouchUp { id: 1, x, y });
        assert_ne!(d.app.screen, Screen::Game);
        assert_eq!(d.app.ui.refused, None, "NAV-5");
        let (x, y) = back_center();
        d.finger_tap(x, y);
        assert_eq!(d.app.screen, Screen::Game);
        assert_eq!(d.app.ui.refused, None, "NAV-5");
        d.wait(BACK_GUARD_MS);
    }

    // The update dialog, also with a "?" in a different cell.
    let mut d = Driver::new(138);
    d.app.update = sudoku::reference::update_state(true);
    d.app.update.tap();
    d.app.update.request = None;
    let (cell, digit) = refused_pair(&d.app.game);
    let other = cells().find(|&c| !d.app.game.is_filled(c) && c != cell).unwrap();
    d.write_strokes(&scribble_ink(other));
    d.tap_micro(cell, digit);
    assert!(d.app.ui.refused.is_some() && d.app.ui.question.is_some());
    let now = d.now;
    d.app.update_installed(Ok(true), now);
    assert!(d.app.update.dialog_shown());
    assert_eq!((d.app.ui.refused, d.app.ui.question), (None, None), "UPD-10");
    assert_x_rule(&d, "dialog");
}

#[test]
fn the_update_dialog_stops_a_tap_and_shows_no_x() {
    // MRK-10, MRK-12, UPD-10: a pen on the screen has no more effect when the dialog shows. With the
    // dialog, a tap on the board does nothing.
    let mut d = Driver::new(139);
    d.app.update = sudoku::reference::update_state(true);
    d.app.update.tap();
    d.app.update.request = None;
    let before = board(&d.app.game);
    let (cell, digit) = refused_pairs(&d.app.game)
        .into_iter()
        .find(|p| {
            let (x, y) = micro_center(p.0, p.1);
            !layout::dialog_cancel().contains(x, y) && !layout::dialog_delete().contains(x, y)
        })
        .unwrap();
    let (x, y) = micro_center(cell, digit);
    d.send(Input::PenDown { x, y, tool: Tool::Pen });
    let now = d.now;
    d.app.update_installed(Ok(true), now);
    assert!(d.app.update.dialog_shown());
    d.send(Input::PenUp { x, y });
    assert_eq!(d.app.ui.refused, None, "UPD-10: the tap in progress");
    d.tap_micro(cell, digit);
    assert_eq!(d.app.ui.refused, None, "MRK-12: a tap under the dialog");
    assert_eq!(board(&d.app.game), before, "UPD-10");
}

#[test]
fn a_tap_on_another_screen_shows_no_x() {
    // MRK-10, MRK-12: the x shows only on the game screen.
    for page in [Button::Help, Button::Statistics] {
        let mut d = Driver::new(140);
        let (cell, digit) = refused_pair(&d.app.game);
        d.button(Button::Menu);
        d.button(page);
        d.tap_micro(cell, digit);
        assert_eq!(d.app.ui.refused, None, "MRK-10 ({page:?})");
    }
}

#[test]
fn an_incorrect_number_in_a_peer_causes_the_x() {
    // MRK-18: the x comes only from the numbers in the peers, not from the solution. An incorrect number of
    // the player causes an x. The correct digit of the cell gives a mark, because no peer has it.
    let mut d = Driver::new(141);
    let g = &d.app.game;
    let sol = |c: Cell| g.solution[c.0 as usize][c.1 as usize];
    let (a, b, w) = cells()
        .filter(|&a| !g.is_filled(a))
        .find_map(|a| {
            peers(a).filter(|&b| !g.is_filled(b)).find_map(|b| {
                (1..=9u8).find(|&w| w != sol(a) && w != sol(b) && !peer_has(g, a, w) && !peer_has(g, b, w)).map(|w| (a, b, w))
            })
        })
        .expect("an empty cell, an empty peer and an incorrect digit");
    let correct = sol(b);
    assert_ne!(sol(a), w);
    d.write(a, w);
    assert_eq!(d.app.game.value_at(a), w);

    d.tap_micro(b, w);
    assert_eq!(d.app.ui.refused, Some((b, w, d.now)), "MRK-18: an incorrect number in a peer");
    assert_eq!(d.app.game.marks_at(b), 0, "MRK-7");
    d.wait(OVERLAY_MS);

    d.tap_micro(b, correct);
    assert_eq!(d.app.ui.refused, None, "MRK-18: the solution does not cause an x");
    assert_eq!(d.app.game.marks_at(b), mark_bit(correct));

    // With no number in the peer, the same tap adds the mark.
    d.erase(a);
    d.tap_micro(b, w);
    assert_eq!(d.app.ui.refused, None, "MRK-18");
    assert_ne!(d.app.game.marks_at(b) & mark_bit(w), 0);
}

// ---- Completed groups, the Settings panel and the Cell colors panel (REQUIREMENTS.md §4.2, §6.4, §9.1,
// §10; BRD-7, COL-3, COL-5, COL-8 to COL-12, NAV-14, NAV-17, UPD-1). Each expected color comes from the
// documents or from the separate model in tests/common/mod.rs.

const GRAY: [u8; 3] = [0xAE, 0xAE, 0xAE];
const WHITE: [u8; 3] = [0xFF, 0xFF, 0xFF];
const RED: [u8; 3] = [0xFF, 0x8C, 0x8C];
const GREEN: [u8; 3] = [0x48, 0xD6, 0x30];
const CYAN: [u8; 3] = [0x6F, 0xCF, 0xE6];

/// The entries that fill row 0 of `PUZZLE` with the solution. (0, 0), (0, 1) and (0, 4) are starting numbers.
const ROW_0: [(Cell, u8); 6] = [((0, 2), 4), ((0, 3), 6), ((0, 5), 8), ((0, 6), 9), ((0, 7), 1), ((0, 8), 2)];

/// `PUZZLE` with the completed group row 0, and no other completed group.
fn row_0_board() -> sudoku::game::Game {
    puzzle_with(&ROW_0)
}

/// A board with each case of BRD-7. Row 0 is a completed group, with the starting numbers (0, 0) and
/// (0, 4) and the entry (0, 8). The digit 7 is at (0, 4), at the clashing cells (5, 0) (a starting number)
/// and (5, 2) (an entry), at the starting number (8, 7) and at the entry (4, 6). (1, 0) is a starting number
/// and (3, 1) an entry with no color. (8, 0) is empty.
fn order_board() -> sudoku::game::Game {
    let mut g = row_0_board();
    g.add_number((5, 2), 7);
    g.add_number((4, 6), 7);
    g.add_number((3, 1), 5);
    g
}

/// Each listed cell has the color.
fn assert_cells(d: &Driver, fonts: &sudoku::font::Fonts, case: &str, want: &[(Cell, [u8; 3])]) {
    let pm = draw_screen(&d.app, fonts);
    for &(cell, rgb) in want {
        assert_eq!(cell_background(&pm, cell), rgb, "{case}: {cell:?}");
    }
    assert_board(&d.app, fonts, case);
}

#[test]
fn the_background_of_a_filled_cell_is_the_first_rule_that_applies() {
    // BRD-7, §4.2, HLT-5: clash, highlight, completed, starting number, white. All layouts.
    let fonts = fonts();
    for_each_layout(|layout_case| {
        let mut d = Driver::new(200);
        d.app.game = order_board();
        assert_cells(
            &d,
            &fonts,
            layout_case,
            &[
                ((0, 0), GREEN), // Completed over starting.
                ((0, 4), GREEN),
                ((0, 8), GREEN), // Completed over white.
                ((5, 0), RED),   // Clash over starting.
                ((5, 2), RED),
                ((1, 0), GRAY),
                ((8, 7), GRAY),
                ((3, 1), WHITE),
                ((4, 6), WHITE),
                ((8, 0), WHITE),
            ],
        );
        // A pen tap makes the highlight of the digit 7.
        d.tap_cell((0, 4));
        assert_eq!(d.app.ui.highlight, Some(7));
        assert_cells(
            &d,
            &fonts,
            layout_case,
            &[
                ((0, 4), CYAN), // Highlight over completed.
                ((8, 7), CYAN), // Highlight over starting.
                ((4, 6), CYAN), // Highlight over white.
                ((5, 0), RED),  // Clash over highlight.
                ((5, 2), RED),
                ((0, 0), GREEN),
                ((1, 0), GRAY),
            ],
        );
        // A second tap removes the highlight, and the cell has the completed color again.
        d.tap_cell((0, 4));
        assert_eq!(d.app.ui.highlight, None);
        assert_cells(&d, &fonts, layout_case, &[((0, 4), GREEN), ((8, 7), GRAY)]);
    });
}

#[test]
fn the_order_of_the_colors_does_not_depend_on_the_selected_colors() {
    // BRD-7, COL-3, COL-11: the order with other colors, and with White in each section. A White
    // section that applies gives white: the next rule does not apply.
    let fonts = fonts();
    let mut d = Driver::new(201);
    d.app.game = order_board();
    d.app.settings.color_starting = Color::Yellow;
    d.app.settings.color_clash = Color::Blue;
    d.app.settings.color_highlight = Color::Magenta;
    d.app.settings.color_completed = Color::Orange;
    d.app.ui.highlight = Some(7);
    let c = |x: Color| x.rgb();
    assert_cells(
        &d,
        &fonts,
        "other colors",
        &[((0, 0), c(Color::Orange)), ((0, 4), c(Color::Magenta)), ((5, 0), c(Color::Blue)), ((1, 0), c(Color::Yellow)), ((3, 1), WHITE)],
    );
    d.app.settings.color_highlight = Color::White;
    assert_cells(&d, &fonts, "a White highlight", &[((0, 4), WHITE), ((8, 7), WHITE), ((0, 0), c(Color::Orange))]);
    d.app.settings.color_clash = Color::White;
    assert_cells(&d, &fonts, "a White clash", &[((5, 0), WHITE), ((5, 2), WHITE)]);
    d.app.ui.highlight = None;
    d.app.settings.color_starting = Color::White;
    assert_cells(&d, &fonts, "a White starting color", &[((0, 0), c(Color::Orange)), ((1, 0), WHITE)]);
}

#[test]
fn a_cell_with_a_question_mark_has_no_completed_color() {
    // BRD-7, COL-8, QST-1: the cell of the "?" is empty, thus its row is not a completed group.
    let fonts = fonts();
    let mut d = Driver::new(202);
    d.app.game = puzzle_with(&ROW_0[..5]);
    d.write_strokes(&scribble_ink((0, 8)));
    d.wait(d.app.pause_ms + 10);
    assert!(d.app.ui.question.is_some(), "the app did not read a number");
    assert_cells(&d, &fonts, "with the question mark", &[((0, 8), WHITE), ((0, 0), GRAY)]);
    assert!(!d.app.game.completed_cells().contains((0, 8)));
    d.wait(OVERLAY_MS);
    assert!(d.app.ui.question.is_none());
    d.write((0, 8), 2);
    assert_cells(&d, &fonts, "with the number", &[((0, 8), GREEN), ((0, 0), GREEN)]);
}

#[test]
fn each_group_gets_the_color_when_its_last_cell_is_filled() {
    // COL-8: all 27 groups. Only the nine cells of the group get the color, also when the
    // group has one entry.
    let fonts = fonts();
    let groups = model_groups();
    assert_eq!(groups.len(), 27);
    let mut d = Driver::new(203);
    for group in &groups {
        let last = group[8];
        d.app.game = board_with(&group[..8]);
        assert!(d.app.game.completed_cells() == Default::default(), "{group:?}: one cell is empty");
        assert_board(&d.app, &fonts, "eight cells");
        let v = digits(&SOLUTION)[last.0 as usize][last.1 as usize];
        d.app.game.add_number(last, v);
        let pm = draw_screen(&d.app, &fonts);
        for c in cells() {
            let want = if group.contains(&c) { GREEN } else { WHITE };
            assert_eq!(cell_background(&pm, c), want, "{group:?}: {c:?}");
        }
    }
}

#[test]
fn a_row_a_column_and_a_box_get_the_color_with_the_pen_in_all_layouts() {
    // COL-8, COL-10: the same with the input of a player. One cell is in the row, the column and the
    // box of the same number, and the three groups become completed at the same time.
    let fonts = fonts();
    let solution = digits(&SOLUTION);
    let row = (0..9).map(|c| (4u8, c));
    let column = (0..9).map(|r| (r, 4u8));
    let boxed = (3..6).flat_map(|r| (3..6u8).map(move |c| (r, c)));
    let puzzle = board_of(&PUZZLE);
    let entries: Vec<(Cell, u8)> = row
        .chain(column)
        .chain(boxed)
        .filter(|&c| c != (4, 4) && !puzzle.is_given(c))
        .map(|c| (c, solution[c.0 as usize][c.1 as usize]))
        .collect();
    for_each_layout(|layout_case| {
        let mut d = Driver::new(204);
        d.app.game = puzzle_with(&entries);
        // White starting numbers: only the completed color can show on these cells.
        d.app.settings.color_starting = Color::White;
        assert_cells(&d, &fonts, layout_case, &[((4, 0), WHITE), ((0, 4), WHITE), ((3, 3), WHITE)]);
        d.write((4, 4), 5);
        assert_eq!(d.app.game.value_at((4, 4)), 5);
        assert_cells(&d, &fonts, layout_case, &[((4, 4), GREEN), ((4, 0), GREEN), ((0, 4), GREEN), ((3, 3), GREEN), ((8, 8), WHITE)]);
        d.erase((4, 4));
        assert_cells(&d, &fonts, layout_case, &[((4, 4), WHITE), ((4, 0), WHITE), ((0, 4), WHITE), ((3, 3), WHITE)]);
    });
}

#[test]
fn a_clash_with_a_cell_outside_the_group_stops_the_group() {
    // COL-8, CLS-2, COL-10: all 27 groups. The group is full and has the digits 1 to 9 one time, but a
    // cell of the group clashes with a cell outside it. The erasure of that cell completes the group.
    let fonts = fonts();
    let solution = digits(&SOLUTION);
    let groups = model_groups();
    let mut d = Driver::new(205);
    for group in &groups {
        let p = group[0];
        let v = solution[p.0 as usize][p.1 as usize];
        let q = groups.iter().filter(|o| o.contains(&p)).flatten().copied().find(|c| !group.contains(c)).unwrap();
        d.app.game = board_with(group);
        assert_cells(&d, &fonts, "full", &[(p, GREEN), (group[8], GREEN)]);
        d.app.game.add_number(q, v);
        let mut in_group: Vec<u8> = group.iter().map(|&c| d.app.game.value_at(c)).collect();
        in_group.sort_unstable();
        assert_eq!(in_group, [1, 2, 3, 4, 5, 6, 7, 8, 9], "{group:?}");
        assert_cells(&d, &fonts, "outside clash", &[(p, RED), (q, RED), (group[1], GRAY), (group[8], GRAY)]);
        d.app.game.erase(q);
        assert_cells(&d, &fonts, "after the erasure", &[(p, GREEN), (q, WHITE), (group[8], GREEN)]);
    }
}

#[test]
fn a_group_with_an_incorrect_number_can_be_a_completed_group() {
    // COL-9, CLS-4, BRD-5: the color comes only from `value`. Two entries are incorrect and no cell
    // clashes: the group has the same colors as a correct group.
    let fonts = fonts();
    let mut d = Driver::new(206);
    d.app.game = puzzle_with(&[((0, 2), 4), ((0, 3), 6), ((0, 5), 8), ((0, 8), 2)]);
    // The solution has 9 at (0, 6) and 1 at (0, 7).
    d.write((0, 6), 1);
    d.write((0, 7), 9);
    let g = d.app.game.clone();
    assert_eq!((g.value_at((0, 6)), g.value_at((0, 7))), (1, 9));
    assert_eq!((g.solution[0][6], g.solution[0][7]), (9, 1), "the entries are incorrect");
    assert_cells(&d, &fonts, "incorrect entries", &[((0, 6), GREEN), ((0, 7), GREEN), ((0, 8), GREEN), ((0, 0), GREEN)]);
    // The solution has no effect on the colors: the same board with a different solution.
    let incorrect = draw_screen(&d.app, &fonts);
    let swapped = |v: u8| match v {
        1 => 9,
        9 => 1,
        v => v,
    };
    d.app.game.solution = g.solution.map(|row| row.map(swapped));
    assert_eq!(d.app.game.value_at((0, 7)), d.app.game.solution[0][7], "now the entries are correct");
    assert_eq!(d.app.game.completed_cells(), g.completed_cells());
    assert!(draw_screen(&d.app, &fonts).data() == incorrect.data(), "CLS-4: the screen does not change");
    // The app does not save the groups (COL-9): the saved game has no data for them, and the
    // set comes again from the numbers.
    let json = serde_json::to_string(&g).unwrap();
    assert!(!json.contains("group"), "{json}");
    let saved: sudoku::game::Game = serde_json::from_str(&json).unwrap();
    assert_eq!(saved.completed_cells(), g.completed_cells());
}

#[test]
fn a_full_group_of_starting_numbers_has_the_color_from_the_first_frame() {
    // COL-9, §6.4: Easy puzzles of the generator (PUZ-5) with a row, a column, a box, or two groups
    // that are full at the start. There is no exception for them.
    let fonts = fonts();
    let cases: [(u64, Cell); 4] = [(323, (0, 8)), (4, (8, 0)), (249, (2, 8)), (368, (5, 8))];
    for (seed, cell) in cases {
        let game = sudoku::app::new_game(seed, 0);
        let d = Driver::opened(app_with_game(game.clone(), START_TIME, seed));
        assert_eq!(d.app.game, game, "the app accepts the puzzle");
        assert!(d.app.game.is_given(cell) && d.app.game.undo.is_empty(), "seed {seed}");
        let groups = model_completed_groups(&d.app.game);
        assert!(!groups.is_empty(), "seed {seed}");
        assert!(groups.iter().flatten().all(|&c| d.app.game.is_given(c)), "seed {seed}");
        assert_cells(&d, &fonts, "first frame", &[(cell, GREEN)]);
    }
}

#[test]
fn a_new_game_gets_the_colors_of_its_own_numbers() {
    // COL-10: a new game replaces the colors of the old board, in one action and with empty stacks.
    let fonts = fonts();
    for_each_layout(|layout_case| {
        let settings = Settings { last_mode: 0, ..Settings::default() };
        let app = App::new(None, Some(settings), ResultsDb::in_memory(), Box::new(sudoku::script::FixedEnv(323)), App::placeholder_ng_stops());
        let mut d = Driver::from(app);
        d.app.game = order_board();
        d.button(Button::Menu);
        d.button(Button::NewGame);
        d.finger_tap(layout::ng_pl() as f32 + 50.0, layout::ng_start_y() as f32);
        assert_eq!(d.app.game, sudoku::app::new_game(323, 0), "{layout_case}");
        assert!(d.app.game.undo.is_empty() && d.app.game.redo.is_empty());
        // Row 0 of this puzzle is full. (5, 0) of the old board clashed.
        assert_cells(&d, &fonts, layout_case, &[((0, 8), GREEN)]);
        assert_ne!(cell_background(&draw_screen(&d.app, &fonts), (5, 0)), RED, "{layout_case}");
    });
}

/// The steps of the far recolor: row 8 is full, and a number in (6, 0) clashes with the entry (8, 0) of that
/// row. `at` gets the name, the two stack lengths without the six entries of the start, and whether row 8
/// is a completed group.
fn far_recolor_steps(d: &mut Driver, mut at: impl FnMut(&Driver, &str, (usize, usize), bool)) {
    d.app.game = puzzle_with(&[((8, 0), 3), ((8, 1), 4), ((8, 2), 5), ((8, 3), 2), ((8, 5), 6), ((8, 6), 1)]);
    d.button(Button::Menu);
    at(d, "start", (0, 0), true);
    d.write((6, 0), 3);
    assert_eq!(d.app.game.value_at((6, 0)), 3);
    at(d, "a number", (1, 0), false);
    d.erase((6, 0));
    at(d, "an erasure", (2, 0), true);
    d.button(Button::Undo);
    at(d, "undo of the erasure", (1, 1), false);
    d.button(Button::Redo);
    at(d, "redo of the erasure", (2, 0), true);
    d.button(Button::Undo);
    d.button(Button::Undo);
    at(d, "undo of the number", (0, 2), true);
    d.button(Button::Redo);
    at(d, "redo of the number", (1, 1), false);
}

#[test]
fn one_input_recolors_cells_far_from_the_changed_cell() {
    // COL-10, UND-2, UND-5: (8, 8) and (8, 4) are not peers of (6, 0), but their row gets and loses the
    // color at each number, erasure, undo and redo. Only the changes of the board go to the stacks.
    let fonts = fonts();
    assert!(!peers((6, 0)).any(|p| p == (8, 8) || p == (8, 4)));
    for_each_layout(|layout_case| {
        let mut d = Driver::new(207);
        far_recolor_steps(&mut d, |d, case, stacks, complete| {
            let case = format!("{layout_case}, {case}");
            assert_eq!((d.app.game.undo.len() - 6, d.app.game.redo.len()), stacks, "{case}");
            assert_eq!(d.app.game.completed_cells().contains((8, 8)), complete, "{case}");
            // Far starting numbers, a far entry, and the entry that clashes.
            let (far, far_entry, near) = if complete { (GREEN, GREEN, GREEN) } else { (GRAY, WHITE, RED) };
            assert_cells(d, &fonts, &case, &[((8, 8), far), ((8, 4), far), ((8, 6), far_entry), ((8, 0), near)]);
            assert!(!d.app.game.completed && d.app.results.is_empty(), "{case}");
        });
    });
}

#[test]
fn the_color_does_not_change_the_game_the_clock_or_the_results() {
    // COL-10, UND-2, CLK-1: the same inputs with and without the color give the same game, with the
    // same stacks, the same clock, the same `completed` and the same results.
    let run = |on: bool| {
        let mut d = Driver::new(208);
        d.app.settings.completed_color_on = on;
        far_recolor_steps(&mut d, |_, _, _, _| {});
        d
    };
    let (on, off) = (run(true), run(false));
    assert_eq!(on.app.game, off.app.game);
    assert!(on.app.game.elapsed_ms > 0);
    assert_eq!(on.app.results.len(), off.app.results.len());
    assert_eq!(on.now, off.now);
}

#[test]
fn the_switch_changes_the_board_immediately_and_nothing_else() {
    // NAV-14, COL-11, UND-2, BRD-7: a tap on the third row. The cell (0, 4) is not under the Settings
    // panel in any layout.
    let fonts = fonts();
    for_each_layout(|layout_case| {
        let mut d = Driver::new(209);
        d.app.game = row_0_board();
        d.tap_micro((4, 4), 5);
        d.app.game.undo();
        let game = d.app.game.clone();
        d.button(Button::Menu);
        d.button(Button::Settings);
        let (x0, _) = documented_settings_x();
        let (x, y) = ((x0 + 200) as f32, (documented_settings_row_y(2) + 56) as f32);
        assert_cells(&d, &fonts, layout_case, &[((0, 4), GREEN)]);
        d.finger_tap(x, y);
        assert!(!d.app.settings.completed_color_on, "{layout_case}");
        assert_eq!(d.app.ui.menu.panel(), Some(Panel::Settings), "the panel stays open");
        assert!(d.app.settings_dirty, "PER-7");
        assert_cells(&d, &fonts, layout_case, &[((0, 4), GRAY)]);
        d.pen_tap(x, y);
        assert!(d.app.settings.completed_color_on, "{layout_case}");
        assert_cells(&d, &fonts, layout_case, &[((0, 4), GREEN)]);
        assert_eq!((&d.app.game.value, &d.app.game.marks), (&game.value, &game.marks));
        assert_eq!((&d.app.game.undo, &d.app.game.redo), (&game.undo, &game.redo), "UND-2");
    });
}

#[test]
fn the_switch_off_and_white_give_the_same_board() {
    // COL-11, BRD-7: the two conditions give the same pixels, and a starting number keeps its starting
    // color. They do not change the set of completed groups.
    let fonts = fonts();
    for_each_layout(|layout_case| {
        let mut d = Driver::new(210);
        d.app.game = order_board();
        let set = d.app.game.completed_cells();
        let on = draw_screen(&d.app, &fonts);
        d.app.settings.completed_color_on = false;
        assert_cells(&d, &fonts, layout_case, &[((0, 0), GRAY), ((0, 8), WHITE), ((0, 4), GRAY), ((5, 0), RED)]);
        let off = draw_screen(&d.app, &fonts);
        d.app.settings.completed_color_on = true;
        d.app.settings.color_completed = Color::White;
        assert_cells(&d, &fonts, layout_case, &[((0, 0), GRAY), ((0, 8), WHITE), ((0, 4), GRAY), ((5, 0), RED)]);
        let white = draw_screen(&d.app, &fonts);
        assert!(off.data() == white.data(), "{layout_case}: the two conditions give the same board");
        assert!(on.data() != off.data(), "{layout_case}");
        d.app.settings.completed_color_on = false;
        assert!(draw_screen(&d.app, &fonts).data() == off.data(), "{layout_case}: the two conditions together");
        assert_eq!(d.app.game.completed_cells(), set, "{layout_case}");
    });
}

#[test]
fn the_completed_section_operates_with_the_switch_off() {
    // COL-11, COL-5, COL-3: with the switch off, a tap selects a color, the color stays not available
    // for the other sections, and the board shows the color when the switch is on again.
    let fonts = fonts();
    for_each_layout(|layout_case| {
        let mut d = Driver::new(211);
        d.app.game = row_0_board();
        d.app.settings.completed_color_on = false;
        d.button(Button::Menu);
        d.button(Button::Colors);
        let tap = |d: &mut Driver, s: Section, c: Color| {
            let (x, y) = color_point(s, c);
            d.finger_tap(x, y);
        };
        tap(&mut d, Section::Highlight, Color::Green);
        assert_eq!(d.app.settings.color_highlight, Color::Cyan, "{layout_case}: Green stays not available");
        tap(&mut d, Section::Completed, Color::Orange);
        assert_eq!(d.app.settings.color_completed, Color::Orange, "{layout_case}");
        assert!(!d.app.settings.completed_color_on, "{layout_case}: a color does not change the switch");
        tap(&mut d, Section::Starting, Color::Orange);
        assert_eq!(d.app.settings.color_starting, Color::Gray, "{layout_case}: Orange is not available");
        tap(&mut d, Section::Highlight, Color::Green);
        assert_eq!(d.app.settings.color_highlight, Color::Green, "{layout_case}: Green is free now");
        d.button(Button::Colors);
        assert_cells(&d, &fonts, layout_case, &[((0, 0), GRAY), ((0, 8), WHITE)]);
        d.button(Button::Settings);
        let (x0, _) = documented_settings_x();
        d.finger_tap((x0 + 200) as f32, (documented_settings_row_y(2) + 56) as f32);
        d.button(Button::Settings);
        assert_cells(&d, &fonts, layout_case, &[((0, 0), Color::Orange.rgb()), ((0, 8), Color::Orange.rgb())]);
    });
}

#[test]
fn a_new_completed_color_shows_immediately_under_the_open_panel() {
    // COL-3, COL-4: the panel stays open, and the cells that it does not cover change at the tap.
    // The game and the stacks do not change.
    let fonts = fonts();
    for_each_layout(|layout_case| {
        let mut d = Driver::new(212);
        d.app.game = row_0_board();
        let game = d.app.game.clone();
        d.button(Button::Menu);
        d.button(Button::Colors);
        let visible = if layout::right_handed() { (0, 8) } else { (0, 0) };
        let (px, py) = micro_center(visible, 1);
        assert!(!layout::colors_panel().contains(px, py), "{layout_case}");
        assert_cells(&d, &fonts, layout_case, &[(visible, GREEN)]);
        for color in [Color::Yellow, Color::White, Color::Green] {
            let (x, y) = color_point(Section::Completed, color);
            d.finger_tap(x, y);
            assert_eq!(d.app.settings.color_completed, color, "{layout_case}");
            assert_eq!(d.app.ui.menu.panel(), Some(Panel::Colors), "{layout_case}");
            // White: no completed color. Then (0, 0) is a starting number and (0, 8) is an entry.
            let plain = if d.app.game.is_given(visible) { GRAY } else { WHITE };
            let want = if color == Color::White { plain } else { color.rgb() };
            assert_cells(&d, &fonts, layout_case, &[(visible, want)]);
        }
        assert_eq!((&d.app.game.value, &d.app.game.undo, &d.app.game.redo), (&game.value, &game.undo, &game.redo), "COL-4");
    });
}

/// All cells have the normal colors of a complete game: the starting color and white (COL-12).
fn assert_normal_colors(app: &App, fonts: &sudoku::font::Fonts, starting: [u8; 3], case: &str) {
    let pm = draw_screen(app, fonts);
    assert!(app.game.completed, "{case}");
    for c in cells() {
        let want = if app.game.is_given(c) { starting } else { WHITE };
        let (x, y) = micro_center(c, 1);
        let panel = app.ui.menu.panel().is_some_and(|p| match p {
            Panel::Colors => layout::colors_panel().contains(x, y + 111.0),
            Panel::Settings => layout::settings_panel(app.update.has_row()).contains(x, y + 111.0),
            Panel::NewGame => layout::ng_panel().contains(x, y + 111.0),
        });
        if !panel {
            assert_eq!(cell_background(&pm, c), want, "{case}: {c:?}");
        }
    }
}

#[test]
fn at_completion_the_groups_lose_the_color() {
    // COL-12, CMP-6, NAV-14: before the last number, each row except one has the color. At completion
    // no cell has it. The switch and the Completed groups section still operate, with no effect on the board.
    let fonts = fonts();
    for_each_layout(|layout_case| {
        let mut d = Driver::new(213);
        let last = empty_cell(&d.app.game);
        fill_except(&mut d, last);
        let other = (last.0 + 1) % 9;
        let (given, entry) = (
            (0..9).map(|c| (other, c)).find(|&c| d.app.game.is_given(c)).expect("a starting number"),
            (0..9).map(|c| (other, c)).find(|&c| d.app.game.is_entry(c)).expect("an entry"),
        );
        assert_cells(&d, &fonts, layout_case, &[(given, GREEN), (entry, GREEN)]);
        let v = d.app.game.solution[last.0 as usize][last.1 as usize];
        d.write(last, v);
        assert_normal_colors(&d.app, &fonts, GRAY, layout_case);
        assert_eq!(d.app.results.len(), 1, "CMP-5");
        // COL-11: the set does not depend on the color. All cells are in completed groups.
        assert!(cells().all(|c| d.app.game.completed_cells().contains(c)), "{layout_case}");
        // The switch operates and does not change the board.
        d.button(Button::Menu);
        d.button(Button::Settings);
        let (x0, _) = documented_settings_x();
        let (x, y) = ((x0 + 200) as f32, (documented_settings_row_y(2) + 56) as f32);
        d.finger_tap(x, y);
        assert!(!d.app.settings.completed_color_on, "{layout_case}");
        assert_normal_colors(&d.app, &fonts, GRAY, layout_case);
        d.finger_tap(x, y);
        assert!(d.app.settings.completed_color_on, "{layout_case}");
        assert_normal_colors(&d.app, &fonts, GRAY, layout_case);
        // A new completed color does not show. A new starting color shows.
        d.button(Button::Colors);
        let (x, y) = color_point(Section::Completed, Color::Orange);
        d.finger_tap(x, y);
        assert_eq!(d.app.settings.color_completed, Color::Orange, "{layout_case}");
        assert_normal_colors(&d.app, &fonts, GRAY, layout_case);
        let (x, y) = color_point(Section::Starting, Color::Yellow);
        d.finger_tap(x, y);
        assert_eq!(d.app.settings.color_starting, Color::Yellow, "{layout_case}");
        d.button(Button::Colors);
        assert_normal_colors(&d.app, &fonts, Color::Yellow.rgb(), layout_case);
        assert_eq!(d.app.results.len(), 1, "CMP-5");
    });
}

#[test]
fn a_redo_that_completes_the_game_removes_the_color() {
    // COL-12, COL-10: the last number comes from a redo.
    let fonts = fonts();
    let mut d = Driver::new(214);
    let last = redo_completes(&mut d);
    d.button(Button::Menu);
    let other = ((last.0 + 1) % 9, 0);
    assert_cells(&d, &fonts, "before the redo", &[(other, GREEN)]);
    d.button(Button::Redo);
    assert_normal_colors(&d.app, &fonts, GRAY, "after the redo");
}

#[test]
fn a_saved_completed_game_opens_with_the_normal_colors() {
    // COL-12, PER-1, PER-2: the color does not come back when a completed game opens, in any layout.
    let fonts = fonts();
    let mut d = Driver::new(215);
    let last = empty_cell(&d.app.game);
    fill_except(&mut d, last);
    let v = d.app.game.solution[last.0 as usize][last.1 as usize];
    d.write(last, v);
    assert!(d.app.game.completed);
    let dir = std::env::temp_dir().join(format!("sudoku-completed-{}", std::process::id()));
    let (gp, sp) = (dir.join("game.json"), dir.join("settings.json"));
    sudoku::store::save_json(&gp, &d.app.game).unwrap();
    sudoku::store::save_json(&sp, &d.app.settings).unwrap();
    for_each_layout(|layout_case| {
        let app = App::new(sudoku::store::load_json(&gp), sudoku::store::load_json(&sp), ResultsDb::in_memory(), Box::new(TestEnv { time: 0, seed: 9 }), App::placeholder_ng_stops());
        assert_eq!(app.game, d.app.game, "{layout_case}");
        assert!(app.settings.completed_color_on && app.settings.color_completed == Color::Green, "{layout_case}");
        assert_normal_colors(&app, &fonts, GRAY, layout_case);
    });
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_settings_panel_has_three_switches_and_the_version_row_in_row_four() {
    // NAV-14, UPD-1, §9.1: the panel stays 400 px wide. The positions come from the documents.
    let fonts = fonts();
    assert_eq!(Switch::ColorCompletedGroups.label(), ["Color completed", "groups"]);
    for_each_layout(|layout_case| {
        let mut d = Driver::new(216);
        d.app.update = sudoku::update::Update::new("dev");
        let mark = free_cell(&d.app, &[5]);
        d.tap_micro(mark, 5);
        let game = (d.app.game.value, d.app.game.marks, d.app.game.undo.clone(), d.app.game.redo.clone());
        d.button(Button::Menu);
        d.button(Button::Settings);
        let (x0, x1) = documented_settings_x();
        assert_eq!(x1 - x0 + 1, 400, "{layout_case}");
        let row_center = |i: i32| (documented_settings_row_y(i) + 56) as f32;
        let switches = |d: &Driver| (d.app.settings.highlight_on, d.app.settings.auto_hide_on, d.app.settings.completed_color_on);

        // A tap on each row changes only its switch, also at the first and the last pixel of the row.
        let mx = (x0 + 200) as f32;
        d.finger_tap(mx, row_center(0));
        assert_eq!(switches(&d), (false, true, true), "{layout_case}");
        d.pen_tap(mx, row_center(1));
        assert_eq!(switches(&d), (false, false, true), "{layout_case}");
        d.finger_tap(mx, row_center(2));
        assert_eq!(switches(&d), (false, false, false), "{layout_case}");
        d.finger_tap(x0 as f32, row_center(2));
        assert_eq!(switches(&d), (false, false, true), "{layout_case}");
        d.finger_tap(x1 as f32, row_center(2));
        assert_eq!(switches(&d), (false, false, false), "{layout_case}");
        assert_eq!(d.app.ui.menu.panel(), Some(Panel::Settings), "{layout_case}");

        // The track of the third switch: black when on, white when off (§9.1).
        let track = (x1 - 33 - 36, row_center(2) as i32);
        assert_eq!(rgb_at(&draw_screen(&d.app, &fonts), track.0, track.1), WHITE, "{layout_case}: off");
        d.finger_tap(mx, row_center(2));
        assert_eq!(rgb_at(&draw_screen(&d.app, &fonts), track.0, track.1), [0, 0, 0], "{layout_case}: on");
        // The name has two lines of text in the third row.
        let pm = draw_screen(&d.app, &fonts);
        let top = documented_settings_row_y(2);
        let dark = |y0: i32, y1: i32| (x0 + 30..x0 + 250).any(|x| (y0..y1).any(|y| rgb_at(&pm, x, y)[0] < 100));
        assert!(dark(top + 8, top + 52) && dark(top + 60, top + 104), "{layout_case}: two lines of text");

        // The version row (row 4) changes nothing, and the panel stays open.
        d.app.settings_dirty = false;
        let before = d.app.settings.clone();
        d.finger_tap(mx, row_center(3));
        d.pen_tap(mx, row_center(3));
        assert_eq!(d.app.settings, before, "{layout_case}");
        assert!(!d.app.settings_dirty, "{layout_case}");
        assert_eq!(d.app.ui.menu.panel(), Some(Panel::Settings), "{layout_case}");

        // The game does not change.
        assert_eq!((d.app.game.value, d.app.game.marks, d.app.game.undo.clone(), d.app.game.redo.clone()), game, "{layout_case}");

        // A tap next to the panel only closes it: the panel is not wider than 400 px (NAV-11, NAV-12).
        for x in [x0 - 1, x1 + 1] {
            d.finger_tap(x as f32, row_center(2));
            assert_eq!(d.app.ui.menu, Menu::Open(None), "{layout_case}, x {x}");
            assert_eq!(d.app.settings, before, "{layout_case}, x {x}");
            d.button(Button::Settings);
        }
    });
}

#[test]
fn the_settings_panel_has_a_fifth_row_only_in_a_build_that_can_update() {
    // UPD-1, §9.1: four rows when `state = Off`, else five. A tap in the last row keeps the panel open,
    // and a tap one row lower only closes it.
    use sudoku::update::Update;
    let fonts = fonts();
    let hex40 = "0123456789abcdef0123456789abcdef01234567";
    let cases = [
        ("dev".to_string(), 4),
        ("v1".to_string(), 4),
        ("pre-abc".to_string(), 4),
        (format!("pre-{hex40}0"), 4),
        ("v0.1.0".to_string(), 5),
        ("pre-abcdef0".to_string(), 5),
        (format!("pre-{hex40}"), 5),
    ];
    for_each_layout(|layout_case| {
        for (version, rows) in &cases {
            let case = format!("{layout_case}, {version}");
            let mut d = Driver::new(217);
            d.app.update = Update::new(version);
            d.app.update.request = None;
            assert_eq!(d.app.update.has_row(), *rows == 5, "{case}");
            d.button(Button::Menu);
            d.button(Button::Settings);
            let (x0, _) = documented_settings_x();
            // The bottom border is the last 2 px of the panel.
            let border = documented_settings_row_y(*rows);
            let pm = draw_screen(&d.app, &fonts);
            let x = x0 + 200;
            assert_eq!(rgb_at(&pm, x, border - 2), [0, 0, 0], "{case}");
            assert_eq!(rgb_at(&pm, x, border - 1), [0, 0, 0], "{case}");
            let before = d.app.settings.clone();
            let margin = (x0 + 10) as f32;
            d.finger_tap(margin, (documented_settings_row_y(*rows - 1) + 56) as f32);
            assert_eq!(d.app.ui.menu.panel(), Some(Panel::Settings), "{case}: the last row");
            d.finger_tap(margin, (documented_settings_row_y(*rows) + 56) as f32);
            assert_eq!(d.app.ui.menu, Menu::Open(None), "{case}: below the panel");
            assert_eq!(d.app.settings, before, "{case}");
        }
    });
}

/// The state of the app that a tap with no effect does not change.
fn panel_state(d: &Driver) -> impl PartialEq + std::fmt::Debug {
    (d.app.game.value, d.app.game.marks, d.app.game.undo.clone(), d.app.game.redo.clone(), d.app.settings.clone(), d.app.settings_dirty, d.app.ui.highlight)
}

#[test]
fn a_tap_on_the_empty_space_or_a_title_of_the_colors_panel_has_no_effect() {
    // NAV-17, §10: all the parts of the panel that are not an item, in the four layouts, with the
    // finger and with the pen. The panel stays open.
    for_each_layout(|layout_case| {
        let mut d = Driver::new(218);
        d.button(Button::Menu);
        d.button(Button::Colors);
        d.app.settings_dirty = false;
        let (x0, x1) = documented_colors_x();
        let before = panel_state(&d);
        let mut points = vec![
            // The empty space below the Micro-cell lines section.
            (x0 + 600, 1012),
            (x0 + 402, 834),
            (x1 - 2, 1190),
            // The line between the columns.
            (x0 + 400, 300),
            (x0 + 400, 1000),
            // The corners of the panel.
            (x0, 112),
            (x1, 112),
            (x0, 1192),
            (x1, 1192),
            // The left and the right margin of a grid.
            (x0 + 10, 300),
            (x0 + 385, 300),
            (x0 + 410, 300),
            (x1 - 10, 300),
        ];
        // The five titles.
        for (left, top) in [(x0, 112), (x0, 472), (x0, 832), (x0 + 400, 112), (x0 + 400, 472)] {
            points.push((left + 53, top + 36));
        }
        for (x, y) in points {
            d.finger_tap(x as f32, y as f32);
            assert_eq!(d.app.ui.menu.panel(), Some(Panel::Colors), "{layout_case}: finger at ({x}, {y})");
            d.pen_tap(x as f32, y as f32);
            assert_eq!(d.app.ui.menu.panel(), Some(Panel::Colors), "{layout_case}: pen at ({x}, {y})");
            assert_eq!(panel_state(&d), before, "{layout_case}: ({x}, {y})");
        }
        // One pixel out of the panel: the tap only closes it (NAV-11, NAV-12).
        for (x, y) in [(x0 - 1, 600), (x1 + 1, 600), (x0 + 100, 111), (x0 + 100, 1193)] {
            d.finger_tap(x as f32, y as f32);
            assert_eq!(d.app.ui.menu, Menu::Open(None), "{layout_case}: ({x}, {y})");
            assert_eq!(panel_state(&d), before, "{layout_case}: ({x}, {y})");
            d.button(Button::Colors);
        }
    });
}

#[test]
fn the_colors_panel_does_not_cover_the_other_items() {
    // NAV-17, NAV-10: the panel changes only the pixels of its area (and the button that opened it). Thus
    // it does not cover a toolbar button, the close button, the meter or "Completed in".
    let fonts = fonts();
    for_each_layout(|layout_case| {
        let mut games = vec![reference::sample_game()];
        games.push(reference::completed_game());
        for game in games {
            let mut d = Driver::new(219);
            let completed = game.completed;
            d.app.game = game;
            d.app.ui.menu = Menu::Open(None);
            let without = draw_screen(&d.app, &fonts);
            d.app.ui.menu = Menu::Open(Some(Panel::Colors));
            let with = draw_screen(&d.app, &fonts);
            let (x0, x1) = documented_colors_x();
            let toolbar = (layout::tb_x(), layout::tb_x() + 112);
            for y in 0..layout::screen_h() as i32 {
                for x in 0..layout::screen_w() as i32 {
                    let in_panel = (x0..=x1).contains(&x) && (112..=1192).contains(&y);
                    let in_button = (toolbar.0..toolbar.1).contains(&x) && (112..224).contains(&y);
                    if !in_panel && !in_button {
                        assert_eq!(rgb_at(&with, x, y), rgb_at(&without, x, y), "{layout_case}, completed {completed}: ({x}, {y})");
                    }
                }
            }
            // The panel has all of its area: the border at the first and at the last pixel.
            assert_eq!(rgb_at(&with, x0, 600), [0, 0, 0], "{layout_case}");
            assert_eq!(rgb_at(&with, x1, 600), [0, 0, 0], "{layout_case}");
            assert_eq!(rgb_at(&with, x0 + 100, 112), [0, 0, 0], "{layout_case}");
            assert_eq!(rgb_at(&with, x0 + 100, 1192), [0, 0, 0], "{layout_case}");
        }
        // The buttons and the close button are out of the panel.
        let (x0, x1) = documented_colors_x();
        for b in [Button::Menu, Button::Colors, Button::Settings, Button::Undo, Button::Redo, Button::Help, Button::Statistics, Button::NewGame, Button::Close] {
            let (x, y) = button_center(b);
            assert!(x < x0 as f32 || x > x1 as f32 || y < 112.0 || y > 1192.0, "{layout_case}: {b:?}");
        }
    });
}

#[test]
fn each_item_of_the_colors_panel_is_at_its_place() {
    // NAV-17, §10, COL-2, COL-7: the order of the sections and of the columns is the same for the two
    // hands. A tap on the place of the documents selects the item of that section.
    use sudoku::settings::LineGray;
    for_each_layout(|layout_case| {
        let mut d = Driver::new(220);
        d.button(Button::Menu);
        d.button(Button::Colors);
        let (x0, _) = documented_colors_x();
        // A color that all sections can select: all sections have White.
        for s in Section::ALL {
            let (x, y) = color_point(s, Color::White);
            d.finger_tap(x, y);
            assert_eq!(d.app.settings.color(s), Color::White, "{layout_case}: {s:?}");
        }
        let places = [(Section::Starting, x0, 112), (Section::Clash, x0, 472), (Section::Highlight, x0, 832), (Section::Completed, x0 + 400, 112)];
        for (section, left, top) in places {
            for (i, color) in Color::ALL.into_iter().enumerate() {
                let r = layout::color_cell(section, color);
                let x = left + 32 + (i as i32 % 3) * 112;
                assert_eq!((r.x0, r.x1 - r.x0, r.y1 - r.y0), (x, 112, 90), "{layout_case}: {section:?} {color:?}");
                assert!(r.y0 >= top && r.y1 <= top + 360, "{layout_case}: {section:?} {color:?}");
                d.pen_tap((x + 56) as f32, (r.y0 + 40) as f32);
                assert_eq!(d.app.settings.color(section), color, "{layout_case}: {section:?} {color:?}");
                assert_eq!(d.app.ui.menu.panel(), Some(Panel::Colors));
            }
            d.finger_tap((left + 32 + 56 + 112) as f32, (layout::color_cell(section, Color::White).y0 + 40) as f32);
            assert_eq!(d.app.settings.color(section), Color::White, "{layout_case}: {section:?}");
        }
        for (i, gray) in LineGray::ALL.into_iter().enumerate() {
            let r = layout::line_gray_cell(gray);
            let x = x0 + 400 + 32 + (i as i32 % 3) * 112;
            assert_eq!((r.x0, r.x1 - r.x0, r.y1 - r.y0), (x, 112, 90), "{layout_case}: {gray:?}");
            assert!(r.y0 >= 472 && r.y1 <= 832, "{layout_case}: {gray:?}");
            d.finger_tap((x + 56) as f32, (r.y0 + 40) as f32);
            assert_eq!(d.app.settings.micro_line, gray, "{layout_case}: COL-7");
        }
        // COL-7: the section of the lines does not change a color.
        assert!(Section::ALL.iter().all(|&s| d.app.settings.color(s) == Color::White), "{layout_case}");
    });
}

#[test]
fn the_four_color_sections_cannot_have_the_same_color() {
    // COL-5: with taps, for each pair of the four sections. White is always available. A refused tap
    // changes nothing, and the panel stays open.
    use Color::*;
    for_each_layout(|layout_case| {
        let mut d = Driver::new(221);
        d.button(Button::Menu);
        d.button(Button::Colors);
        let tap = |d: &mut Driver, s: Section, c: Color| {
            let (x, y) = color_point(s, c);
            d.finger_tap(x, y);
        };
        // The default colors are not available for the other sections.
        for (taken, owner) in [(Gray, Section::Starting), (Red, Section::Clash), (Cyan, Section::Highlight), (Green, Section::Completed)] {
            for s in Section::ALL.into_iter().filter(|&s| s != owner) {
                let before = d.app.settings.clone();
                tap(&mut d, s, taken);
                assert_eq!(d.app.settings, before, "{layout_case}: {s:?} with {taken:?}");
                assert_eq!(d.app.ui.menu.panel(), Some(Panel::Colors), "{layout_case}");
                assert!(d.app.settings.color(s) != taken);
            }
        }
        // Each pair: a color moves from one section to the other when it is free.
        for a in Section::ALL {
            for b in Section::ALL.into_iter().filter(|&b| b != a) {
                let (old_a, old_b) = (d.app.settings.color(a), d.app.settings.color(b));
                tap(&mut d, a, Yellow);
                assert_eq!(d.app.settings.color(a), Yellow, "{layout_case}: {a:?}");
                tap(&mut d, b, Yellow);
                assert_eq!(d.app.settings.color(b), old_b, "{layout_case}: {b:?} has Yellow of {a:?}");
                tap(&mut d, a, Magenta);
                assert_eq!(d.app.settings.color(a), Magenta, "{layout_case}: {a:?}");
                tap(&mut d, b, Yellow);
                assert_eq!(d.app.settings.color(b), Yellow, "{layout_case}: Yellow is free");
                tap(&mut d, b, old_b);
                assert_eq!(d.app.settings.color(b), old_b, "{layout_case}: {b:?}");
                assert!(d.app.settings.is_valid(), "{layout_case}");
                // The next pair starts from the same colors.
                tap(&mut d, a, old_a);
                assert_eq!(d.app.settings.color(a), old_a, "{layout_case}: {a:?}");
            }
        }
        // All four can be White, and then each other color is free.
        for s in Section::ALL {
            tap(&mut d, s, White);
            assert_eq!(d.app.settings.color(s), White, "{layout_case}: {s:?}");
        }
        assert!(d.app.settings.is_valid(), "{layout_case}");
        tap(&mut d, Section::Starting, Green);
        tap(&mut d, Section::Completed, Green);
        assert_eq!((d.app.settings.color_starting, d.app.settings.color_completed), (Green, White), "{layout_case}");
    });
}

#[test]
fn a_settings_file_of_an_earlier_version_goes_through_the_app() {
    // PER-8, PER-5, COL-1: from a file, through `App::new`. Each start gives the same result, and the
    // file changes only at the next change of the settings. (tests/inv_persist.rs examines each case of
    // the colors.)
    let dir = std::env::temp_dir().join(format!("sudoku-old-settings-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("settings.json");
    let start = |path: &std::path::Path| {
        App::new(None, sudoku::store::load_json(path), ResultsDb::in_memory(), Box::new(TestEnv { time: 0, seed: 3 }), App::placeholder_ng_stops())
    };
    let old = r#"{"color_starting":"Gray","color_clash":"Red","color_highlight":"Green","last_mode":1,"highlight_on":false,"auto_remove_on":false}"#;
    std::fs::write(&path, old).unwrap();
    for _ in 0..2 {
        let app = start(&path);
        assert_eq!(app.settings.color_highlight, Color::Cyan);
        assert_eq!(app.settings.color_completed, Color::Green);
        assert!(app.settings.completed_color_on);
        assert_eq!((app.settings.last_mode, app.settings.highlight_on, app.settings.auto_hide_on), (1, false, false));
        assert!(!app.settings_dirty, "the file changes only at the next change");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), old);
    }
    // The next change of the settings keeps the new values.
    let mut d = Driver::from(start(&path));
    d.button(Button::Menu);
    d.button(Button::Colors);
    let (x, y) = color_point(Section::Completed, Color::Orange);
    d.finger_tap(x, y);
    assert!(d.app.settings_dirty);
    sudoku::store::save_json(&path, &d.app.settings).unwrap();
    assert_eq!(start(&path).settings, d.app.settings);
    assert_eq!(start(&path).settings.color_highlight, Color::Cyan);
    // A file that breaks COL-5: the default settings.
    std::fs::write(&path, r#"{"color_starting":"Red","color_clash":"Red","color_highlight":"Blue","last_mode":1}"#).unwrap();
    assert_eq!(start(&path).settings, Settings::default());
    // A file with a completed color that breaks COL-5: the default settings (PER-5).
    std::fs::write(&path, r#"{"color_starting":"Gray","color_clash":"Red","color_highlight":"Green","color_completed":"Green","last_mode":1}"#).unwrap();
    assert_eq!(start(&path).settings, Settings::default());
    std::fs::remove_dir_all(&dir).ok();
}
