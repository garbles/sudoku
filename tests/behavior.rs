//! Tests of the behavior in REQUIREMENTS.md, with the input that a player gives. Each test gives the
//! rules of INVARIANTS.md that it examines. The debug build also checks all state rules after each
//! input (`invariants::check_app`).

mod common;

use common::*;
use sudoku::app::{App, Input, Menu, Panel, Screen, Tool, BACK_GUARD_MS, HAND_TOUCH_MS, OVERLAY_MS, RECOGNITION_PAUSE_MS, WRITING_IDLE_MS};
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
    // HLT-1 to HLT-5, HLT-7. The other tests of the highlight are at the end of this file.
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
    assert_eq!(d.app.ui.highlight, Some(v), "HLT-7: a finger touch keeps it");
    d.button(Button::Menu);
    assert_eq!(d.app.ui.highlight, Some(v), "HLT-7: the menu button keeps it");
    d.tap_cell(given);
    assert_eq!(d.app.ui.highlight, None, "HLT-4: the board operates while the menu is open");
    d.tap_cell(given);
    assert_eq!(d.app.ui.highlight, Some(v), "HLT-4: the board operates while the menu is open");
    d.button(Button::Colors);
    assert_eq!(d.app.ui.highlight, Some(v), "HLT-4: a highlight shows with a panel");
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
    // CMP-6: the menu continues to operate; only Start new game and the level buttons of the card end the state.
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
    assert!(!d.app.quit, "NAV-6: a finger does not close the app");
    let (x, y) = button_center(Button::Close);
    d.pen_tap(x, y);
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
                } else if pen {
                    assert!(d.app.quit, "NAV-16: the close button operates again ({case})");
                } else {
                    assert!(!d.app.quit, "NAV-6: a finger does not close the app ({case})");
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
        for b in [Button::Menu, Button::Colors, Button::Settings, Button::Undo, Button::Redo, Button::Checkpoints, Button::Help, Button::Statistics, Button::NewGame, Button::Close] {
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
            // Only the pen operates the close button (NAV-6).
            if b == Button::Close {
                let (x, y) = button_center(b);
                d.pen_tap(x, y);
            } else {
                d.button(b);
            }
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

    // HLT-9: a pen press does not change the highlight. The tap removes it at the pen lift, with its
    // mark. INP-22: a stroke from a number shows no ink, thus only a highlight needs a drawing.
    let given = cells().find(|&c| d.app.game.is_given(c)).unwrap();
    d.tap_cell(given);
    let empty = cells().find(|&c| !d.app.game.is_filled(c)).unwrap();
    let (x, y) = micro_center(empty, 1);
    d.app.redraw = false;
    d.send(Input::PenDown { x, y, tool: Tool::Pen });
    assert!(!d.app.redraw, "HLT-9: the highlight does not go at the press");
    assert!(d.app.ui.highlight.is_some(), "HLT-9");
    d.send(Input::PenUp { x, y });
    assert!(d.app.redraw, "HLT-2: the mark and the removal of the highlight");
    assert_eq!(d.app.ui.highlight, None, "HLT-2");
    // The tap above can show an x, and a press removes an x (MRK-15).
    d.wait(OVERLAY_MS);
    let (gx, gy) = micro_center(given, 1);
    d.app.redraw = false;
    d.send(Input::PenDown { x: gx, y: gy, tool: Tool::Pen });
    d.send(Input::PenMove { x: gx + 60.0, y: gy + 60.0 });
    assert!(!d.app.redraw, "INP-22: no ink on a starting number");
    d.send(Input::PenUp { x: gx + 60.0, y: gy + 60.0 });
    assert!(!d.app.redraw, "INP-7: the stroke changes nothing when no highlight shows");
    d.wait(d.app.pause_ms + 50);
    assert!(!d.app.is_writing(), "INP-16");
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
        let p = sudoku::generator::generate_version(g.generator, g.seed, g.mode, g.symmetric);
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
        // HLT-6: the switch off removes the highlight, as a tap on the switch does.
        d.app.settings.highlight_on = highlight_on;
        d.app.ui.highlight = None;
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
        for (b, panel) in [(Button::Colors, Panel::Colors), (Button::Settings, Panel::Settings), (Button::NewGame, Panel::NewGame), (Button::Checkpoints, Panel::Checkpoints)] {
            let case = format!("{case}, panel: {panel:?}");
            let mut d = Driver::new(112);
            d.button(Button::Menu);
            d.button(b);
            assert_eq!(d.app.ui.menu.panel(), Some(panel));
            let area = match panel {
                Panel::Colors => layout::colors_panel(),
                Panel::Settings => layout::settings_panel(d.app.update.has_row()),
                Panel::NewGame => layout::ng_panel(),
                Panel::Checkpoints => layout::ckp_panel(d.app.game.checkpoints.len()),
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
            // part has a new driver, and it does not use the row "Start new game" or the row "Save
            // checkpoint". The tap makes no x.
            let inside = pairs.iter().find(|p| {
                let (x, y) = micro_center(p.0, p.1);
                let control = match panel {
                    Panel::NewGame => layout::ng_start_row().contains(x, y),
                    Panel::Checkpoints => layout::ckp_save_row().contains(x, y),
                    _ => false,
                };
                in_panel(p) && !control
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
    // The second finger is not a hand touch (INP-20), and the first touch is active (INP-15).
    d.wait(HAND_TOUCH_MS);
    d.send(Input::TouchMove { id: 5, x: px, y: py });
    assert!(d.app.ui.refused.is_some());
    let (x, y) = micro_center(given_cell(&d.app.game), 5);
    d.send(Input::TouchDown { id: 6, x, y });
    assert_eq!(d.app.ui.refused, None, "MRK-15: a second finger on the board");
    d.send(Input::TouchUp { id: 6, x, y });

    // On a toolbar button, outside the guard time of NAV-16.
    d.send(Input::TouchMove { id: 5, x: px, y: py });
    refused_tap(&mut d);
    d.wait(HAND_TOUCH_MS);
    d.send(Input::TouchMove { id: 5, x: px, y: py });
    assert!(d.app.ui.refused.is_some());
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

        // The presses are not hand touches (INP-20). They are 1 ms apart, thus all are in the guard time.
        d.wait(HAND_TOUCH_MS);
        for b in [Button::Menu, Button::Colors, Button::Settings, Button::Undo, Button::Redo, Button::Checkpoints, Button::Help, Button::Statistics, Button::NewGame, Button::Close] {
            let (px, py) = button_center(b);
            d.now += 1;
            d.app.handle(Input::TouchDown { id: 1, x: px, y: py }, d.now);
            assert_eq!((d.app.ui.refused, d.app.ui.question), (x, question), "NAV-16: finger press ({case}, {b:?})");
            d.now += 1;
            d.app.handle(Input::TouchUp { id: 1, x: px, y: py }, d.now);
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
        for b in [Button::Menu, Button::Colors, Button::Settings, Button::Checkpoints, Button::NewGame, Button::Undo, Button::Redo, Button::Help, Button::Statistics] {
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
                Button::Checkpoints => d.app.ui.menu.panel() == Some(Panel::Checkpoints),
                Button::Undo | Button::Redo => board(&d.app.game) != before,
                Button::Help => d.app.screen == Screen::Manual,
                Button::Statistics => d.app.screen == Screen::Statistics,
                Button::Close => unreachable!("NAV-6: a finger does not operate the close button"),
            };
            assert!(done, "the release does the action of the control ({case})");
        }
    });
}

#[test]
fn only_the_pen_operates_the_close_button() {
    // NAV-6, INP-1, INP-15, INP-21, MRK-15: all layouts, with no panel open.
    for_each_layout(|case| {
        let mut d = Driver::new(121);
        fill_stacks(&mut d);
        let before = control_state(&d);

        // A finger stays on the button while the pen does a refused tap. The release has no action,
        // thus the x stays.
        let (x, y) = hold(&mut d, Button::Close);
        refused_tap(&mut d);
        d.send(Input::TouchUp { id: 1, x, y });
        assert_eq!(control_state(&d), before, "NAV-6 ({case})");
        assert!(d.app.ui.refused.is_some(), "NAV-6: the release has no action ({case})");

        // A new finger press removes the x, and it has no other effect.
        d.finger_tap(x, y);
        assert_eq!(d.app.ui.refused, None, "MRK-15 ({case})");
        assert_eq!(control_state(&d), before, "NAV-6 ({case})");

        // A finger does not read waiting ink, at the press and at the release (INP-21).
        let cell = free_cell(&d.app, &[1]);
        d.write_strokes(&digit_ink(cell, 1));
        assert!(d.app.ui.pending_ink.is_some(), "INP-5 ({case})");
        d.finger_tap(x, y);
        assert!(d.app.ui.pending_ink.is_some(), "INP-21 ({case})");
        assert!(!d.app.quit, "NAV-6 ({case})");

        // The two ends of the pen close the app.
        for tool in [Tool::Pen, Tool::Eraser] {
            let mut d = Driver::new(121);
            fill_stacks(&mut d);
            d.send(Input::PenDown { x, y, tool });
            assert!(!d.app.quit, "a control acts at the release ({case})");
            d.send(Input::PenUp { x, y });
            assert!(d.app.quit && d.app.game_dirty, "NAV-6: {tool:?} ({case})");
        }
    });
}

#[test]
fn the_release_of_a_control_keeps_a_question_mark_in_a_different_cell() {
    // MRK-15, QST-4, R§9: the release removes only the x. The "?" stays, with its own time. A change of
    // screen removes the two (NAV-5).
    for b in [Button::Colors, Button::Settings, Button::Checkpoints, Button::NewGame, Button::Undo, Button::Redo] {
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
const RED: [u8; 3] = [0xFF, 0x8E, 0x8E];
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
        // Not symmetric (DIF-3, DIF-5): row 0 of the puzzle of (323, 0, not symmetric) is full.
        let settings = Settings { last_mode: 0, last_symmetric: false, ..Settings::default() };
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
            Panel::Checkpoints => layout::ckp_panel(app.game.checkpoints.len()).contains(x, y + 111.0),
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
fn the_settings_panel_has_an_update_row_only_in_a_build_that_can_update() {
    // UPD-1, LIC-1, §9.1: five rows when `state = Off`, else six. A tap in the update row keeps the
    // panel open, a tap in the licenses row opens the Licenses screen, and a tap below the panel only
    // closes it.
    use sudoku::update::Update;
    let fonts = fonts();
    let hex40 = "0123456789abcdef0123456789abcdef01234567";
    let cases = [
        ("dev".to_string(), 5),
        ("v1".to_string(), 5),
        ("pre-abc".to_string(), 5),
        (format!("pre-{hex40}0"), 5),
        ("v0.1.0".to_string(), 6),
        ("pre-abcdef0".to_string(), 6),
        (format!("pre-{hex40}"), 6),
    ];
    for_each_layout(|layout_case| {
        for (version, rows) in &cases {
            let case = format!("{layout_case}, {version}");
            let mut d = Driver::new(217);
            d.app.update = Update::new(version);
            d.app.update.request = None;
            assert_eq!(d.app.update.has_row(), *rows == 6, "{case}");
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
            if *rows == 6 {
                d.finger_tap(margin, (documented_settings_row_y(5) + 56) as f32);
                assert_eq!(d.app.ui.menu.panel(), Some(Panel::Settings), "{case}: the update row");
            }
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
        // One pixel out of the panel: the tap only closes it (NAV-11, NAV-12). At y 700, the toolbar has
        // no button (top slot 5, y 560-671, has the bookmark).
        for (x, y) in [(x0 - 1, 700), (x1 + 1, 700), (x0 + 100, 111), (x0 + 100, 1193)] {
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
    // it does not cover a toolbar button, the close button, the meter or the result card (CMP-7).
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
        for b in [Button::Menu, Button::Colors, Button::Settings, Button::Undo, Button::Redo, Button::Checkpoints, Button::Help, Button::Statistics, Button::NewGame, Button::Close] {
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

// ---- The Checkpoints panel (REQUIREMENTS.md §9.3). The positions come from the documents, not from
// `layout`. A value of the documents is for a left-handed player in portrait: each x below is the
// distance from the left edge of the panel (NAV-18).

/// `gray70` of the documents: 70 percent of white.
const GRAY70: u8 = 179;

#[derive(Clone, Copy, Debug)]
enum By {
    Pen,
    Eraser,
    Finger,
}

const ALL_TOOLS: [By; 3] = [By::Pen, By::Eraser, By::Finger];

fn tap_by(d: &mut Driver, by: By, (x, y): (f32, f32)) {
    match by {
        By::Pen => d.pen_tap(x, y),
        By::Finger => d.finger_tap(x, y),
        By::Eraser => {
            d.send(Input::PenDown { x, y, tool: Tool::Eraser });
            d.send(Input::PenUp { x, y });
        }
    }
}

fn row_y(i: usize) -> i32 {
    672 + 180 * i as i32
}

fn at(x: i32, y: i32) -> (f32, f32) {
    (x as f32, y as f32)
}

/// The center of the row "Save checkpoint".
fn save_point() -> (f32, f32) {
    at(documented_checkpoints_x().0 + 300, 616)
}

/// The center of the button of row `i` ("Restore" or "Current").
fn restore_point(i: usize) -> (f32, f32) {
    at(documented_checkpoints_x().0 + 451, row_y(i) + 90)
}

/// The center of the bin of row `i`.
fn bin_point(i: usize) -> (f32, f32) {
    at(documented_checkpoints_x().0 + 560, row_y(i) + 90)
}

fn panel_of(d: &Driver) -> Option<Panel> {
    d.app.ui.menu.panel()
}

/// Opens the Checkpoints panel with a tap on the bookmark.
fn open_checkpoints(d: &mut Driver) {
    if !d.app.ui.menu.is_open() {
        d.button(Button::Menu);
    }
    if panel_of(d) != Some(Panel::Checkpoints) {
        d.button(Button::Checkpoints);
    }
    assert_eq!(panel_of(d), Some(Panel::Checkpoints), "NAV-18: the bookmark opens the panel");
}

/// Closes the Checkpoints panel with a tap on the bookmark.
fn close_checkpoints(d: &mut Driver) {
    if panel_of(d) == Some(Panel::Checkpoints) {
        d.button(Button::Checkpoints);
    }
    assert_eq!(d.app.ui.menu, Menu::Open(None), "NAV-18: a second tap on the bookmark closes the panel");
}

/// Adds a mark with the pen to an empty cell that has no mark. This makes a board that is different
/// from all earlier boards.
fn new_mark(d: &mut Driver) -> (Cell, u8) {
    close_checkpoints(d);
    let g = &d.app.game;
    let (cell, digit) = cells()
        .filter(|&c| !g.is_filled(c) && g.marks_at(c) == 0)
        .find_map(|c| (1..=9u8).find(|&v| !peer_has(g, c, v)).map(|v| (c, v)))
        .expect("an empty cell with a free digit");
    d.tap_micro(cell, digit);
    assert_eq!(d.app.game.marks_at(cell), mark_bit(digit), "the pen adds the mark");
    (cell, digit)
}

/// A driver with `n` checkpoints, each with a different board and clock, and with the panel open.
/// The board is the board of the newest checkpoint.
fn with_checkpoints(seed: u64, n: usize) -> Driver {
    let mut d = Driver::new(seed);
    d.button(Button::Menu);
    for k in 0..n {
        new_mark(&mut d);
        d.wait(1000);
        open_checkpoints(&mut d);
        tap_by(&mut d, By::Finger, save_point());
        assert_eq!(d.app.game.checkpoints.len(), k + 1, "CKP-1");
    }
    open_checkpoints(&mut d);
    d
}

/// The game without the clock and without the list of checkpoints.
fn play_state(g: &sudoku::game::Game) -> sudoku::game::Game {
    let mut g = board(g);
    g.checkpoints.clear();
    g
}

/// The smallest and the largest gray value in an area, with the two ends.
fn gray_range(pm: &tiny_skia::Pixmap, (x0, y0): (i32, i32), (x1, y1): (i32, i32)) -> (u8, u8) {
    let mut range = (255, 0);
    for y in y0..=y1 {
        for x in x0..=x1 {
            let v = rgb_at(pm, x, y)[0];
            range = (range.0.min(v), range.1.max(v));
        }
    }
    range
}

/// The gray values of the icon in the slot of the bookmark.
fn bookmark_grays(app: &App) -> (u8, u8) {
    let pm = draw_screen(app, &fonts());
    let (x, y) = (layout::tb_cx() as i32, layout::tb_cy(5) as i32);
    gray_range(&pm, (x - 16, y - 22), (x + 16, y + 22))
}

/// The gray values of the plus icon and the text of the row "Save checkpoint".
fn save_row_grays(app: &App) -> (u8, u8) {
    let pm = draw_screen(app, &fonts());
    let x0 = documented_checkpoints_x().0;
    gray_range(&pm, (x0 + 40, 595), (x0 + 340, 635))
}

/// The color of a point in the button of row `i` that has no border and no text.
fn button_fill(app: &App, i: usize) -> [u8; 3] {
    let pm = draw_screen(app, &fonts());
    rgb_at(&pm, documented_checkpoints_x().0 + 391, row_y(i) + 62)
}

const BUTTON_RESTORE: [u8; 3] = [255, 255, 255];
const BUTTON_CURRENT: [u8; 3] = [0, 0, 0];

#[test]
fn a_save_adds_the_newest_checkpoint_and_changes_nothing_else() {
    // CKP-1, CKP-4, PER-7, UND-9. The four layouts, the pen, the eraser and a finger.
    for_each_layout(|case| {
        for by in ALL_TOOLS {
            let case = format!("{case}, {by:?}");
            let mut d = Driver::new(300);
            d.button(Button::Menu);
            fill_stacks(&mut d);
            d.wait(3000);
            open_checkpoints(&mut d);
            assert_eq!(save_row_grays(&d.app).0, 0, "CKP-1: the row is black when a save is available ({case})");
            let before = d.app.game.clone();
            let t = d.now;
            d.app.game_dirty = false;
            tap_by(&mut d, by, save_point());
            let g = &d.app.game;
            assert_eq!(play_state(g), play_state(&before), "CKP-1: the board and the stacks ({case})");
            assert_eq!(g.elapsed_ms, before.elapsed_ms + (d.now - t), "CKP-1, UND-9: the clock goes on as usual ({case})");
            assert_eq!(g.checkpoints.len(), 1, "CKP-1 ({case})");
            let k = &g.checkpoints[0];
            assert_eq!((k.board.value, k.board.marks), (before.value, before.marks), "CKP-1 ({case})");
            assert_eq!(k.elapsed_ms, g.elapsed_ms, "CKP-1: the clock at the save ({case})");
            assert!(d.app.game_dirty, "PER-7: a save saves the game file ({case})");
            assert_eq!(panel_of(&d), Some(Panel::Checkpoints), "the panel stays open ({case})");
            assert_eq!(save_row_grays(&d.app).0, GRAY70, "CKP-1: no save is available ({case})");

            // The same board: no second checkpoint, also at the corners of the row.
            let (x0, x1) = documented_checkpoints_x();
            for p in [save_point(), at(x0, 560), at(x1, 671)] {
                d.app.game_dirty = false;
                let same = d.app.game.checkpoints.clone();
                tap_by(&mut d, by, p);
                assert_eq!(d.app.game.checkpoints, same, "CKP-1, CKP-4: the same board ({case})");
                assert!(!d.app.game_dirty, "CKP-1: the tap changes nothing ({case})");
            }

            // A new board. The newest is first, and the clock of the older checkpoint is not larger.
            let older = d.app.game.checkpoints[0].clone();
            new_mark(&mut d);
            d.wait(1000);
            assert!(d.app.game.can_save_checkpoint(), "CKP-1 ({case})");
            open_checkpoints(&mut d);
            assert_eq!(save_row_grays(&d.app).0, 0, "CKP-1: a save is available again ({case})");
            tap_by(&mut d, by, save_point());
            let list = &d.app.game.checkpoints;
            assert_eq!(list.len(), 2, "CKP-1 ({case})");
            assert_eq!(list[1], older, "CKP-4: newest first ({case})");
            assert!(list[0].elapsed_ms > older.elapsed_ms, "CKP-4 ({case})");
        }
    });
}

#[test]
fn the_full_row_is_the_tap_area_of_save_in_all_layouts() {
    // CKP-1, NAV-18: the corners of the row operate. y 672 is below the row.
    for_each_layout(|case| {
        let (x0, x1) = documented_checkpoints_x();
        for p in [at(x0, 560), at(x1, 560), at(x0, 671), at(x1, 671)] {
            let mut d = with_checkpoints(301, 0);
            d.finger_tap(p.0, p.1);
            assert_eq!(d.app.game.checkpoints.len(), 1, "CKP-1: {p:?} ({case})");
        }
        let mut d = with_checkpoints(301, 0);
        d.finger_tap((x0 + 300) as f32, 672.0);
        assert!(d.app.game.checkpoints.is_empty(), "CKP-1 ({case})");
        assert_eq!(panel_of(&d), Some(Panel::Checkpoints), "NAV-18: no control ({case})");
    });
}

#[test]
fn a_full_list_has_no_save() {
    // CKP-1, CKP-4: the limit of 5, with a board that no checkpoint has. A delete makes a save
    // available again.
    let mut d = with_checkpoints(302, 5);
    new_mark(&mut d);
    open_checkpoints(&mut d);
    let list = d.app.game.checkpoints.clone();
    let board_now = play_state(&d.app.game);
    assert!(list.iter().all(|k| !d.app.game.same_board(&k.board)), "the board is new");
    assert!(!d.app.game.can_save_checkpoint(), "CKP-1");
    assert_eq!(save_row_grays(&d.app).0, GRAY70, "CKP-1");
    d.app.game_dirty = false;
    d.finger_tap(save_point().0, save_point().1);
    assert_eq!(d.app.game.checkpoints, list, "CKP-1, CKP-4");
    assert_eq!(play_state(&d.app.game), board_now, "CKP-1");
    assert!(!d.app.game_dirty, "CKP-1");

    d.finger_tap(bin_point(4).0, bin_point(4).1);
    assert_eq!(d.app.game.checkpoints.len(), 4, "CKP-3");
    assert_eq!(d.app.game.checkpoints[..], list[..4], "CKP-3: the bin of the last row");
    assert!(d.app.game.can_save_checkpoint(), "CKP-1");
    assert_eq!(save_row_grays(&d.app).0, 0, "CKP-1");
    d.wait(BACK_GUARD_MS); // CKP-8
    d.finger_tap(save_point().0, save_point().1);
    assert_eq!(d.app.game.checkpoints.len(), 5, "CKP-1");
    assert!(d.app.game.same_board(&d.app.game.checkpoints[0].board), "CKP-1: the newest is first");
}

#[test]
fn a_save_is_not_available_when_an_older_checkpoint_is_current() {
    // CKP-1, CKP-4: "no checkpoint is current" examines all rows, not only the newest.
    let mut d = with_checkpoints(303, 2);
    d.finger_tap(restore_point(1).0, restore_point(1).1);
    assert!(d.app.game.same_board(&d.app.game.checkpoints[1].board), "CKP-2");
    assert!(!d.app.game.can_save_checkpoint(), "CKP-1: the older checkpoint is current");
    assert_eq!(save_row_grays(&d.app).0, GRAY70, "CKP-1");
    let list = d.app.game.checkpoints.clone();
    d.finger_tap(save_point().0, save_point().1);
    assert_eq!(d.app.game.checkpoints, list, "CKP-1, CKP-4: no two checkpoints have the same board");
}

#[test]
fn a_restore_is_one_undo_item_that_replaces_all_numbers_and_marks() {
    // CKP-2, UND-9, MRK-5, MRK-6, PER-7. The checkpoint has a mark in an empty cell. After the save, the
    // player removes that mark, adds a different mark and writes a number in that cell.
    for_each_layout(|case| {
        for by in ALL_TOOLS {
            let case = format!("{case}, {by:?}");
            let mut d = Driver::new(304);
            d.button(Button::Menu);
            let (cell, a) = new_mark(&mut d);
            d.wait(2000);
            open_checkpoints(&mut d);
            tap_by(&mut d, By::Finger, save_point());
            assert_eq!(button_fill(&d.app, 0), BUTTON_CURRENT, "CKP-2: the same board ({case})");
            let saved = d.app.game.board();
            let saved_clock = d.app.game.checkpoints[0].elapsed_ms;
            close_checkpoints(&mut d);

            // The changes after the save.
            d.tap_micro(cell, a);
            assert_eq!(d.app.game.marks_at(cell), 0, "the tap removes the mark");
            let b = (1..=9u8).find(|&v| v != a && !peer_has(&d.app.game, cell, v)).expect("a second free digit");
            d.tap_micro(cell, b);
            let v = d.app.game.solution[cell.0 as usize][cell.1 as usize];
            d.write(cell, v);
            assert_eq!(d.app.game.value_at(cell), v, "the app read the number ({case})");
            new_mark(&mut d);
            let (extra, _) = new_mark(&mut d);
            d.button(Button::Undo);
            assert_eq!(d.app.game.marks_at(extra), 0, "the undo removes the last mark ({case})");
            assert_eq!(d.app.game.redo.len(), 1);

            d.wait(5000);
            open_checkpoints(&mut d);
            assert_eq!(button_fill(&d.app, 0), BUTTON_RESTORE, "CKP-2: a different board ({case})");
            let before = d.app.game.clone();
            let list = before.checkpoints.clone();
            let t = d.now;
            d.app.game_dirty = false;
            tap_by(&mut d, by, restore_point(0));

            let g = &d.app.game;
            assert_eq!(g.board(), saved, "CKP-2, MRK-5: all numbers and marks ({case})");
            assert_eq!(g.value_at(cell), 0, "CKP-2: an entry goes ({case})");
            assert_eq!(g.marks_at(cell), mark_bit(a), "MRK-6: the marks of the checkpoint, not the marks before ({case})");
            assert_eq!(g.undo.len(), before.undo.len() + 1, "CKP-2: one item for many changes ({case})");
            assert!(g.redo.is_empty(), "CKP-2 ({case})");
            assert_eq!(g.elapsed_ms, before.elapsed_ms + (d.now - t), "CKP-2, UND-9: the clock does not go back ({case})");
            assert!(g.elapsed_ms > saved_clock + 5000, "CKP-2 ({case})");
            assert_eq!(g.checkpoints, list, "CKP-2 ({case})");
            assert_eq!(panel_of(&d), Some(Panel::Checkpoints), "CKP-2: the panel stays open ({case})");
            assert_eq!(button_fill(&d.app, 0), BUTTON_CURRENT, "CKP-2: the row shows Current ({case})");
            assert!(d.app.game_dirty, "PER-7 ({case})");

            // The first tap closes the panel (NAV-12). Undo gives the board before, with its marks.
            close_checkpoints(&mut d);
            d.button(Button::Undo);
            let g = &d.app.game;
            assert_eq!((g.value, g.marks), (before.value, before.marks), "CKP-2, MRK-6: undo ({case})");
            assert_eq!(g.marks_at(cell), mark_bit(b), "MRK-6: the marks before the Restore ({case})");
            assert_eq!(g.value_at(cell), v, "CKP-2: undo ({case})");
            assert_eq!(g.undo.len(), before.undo.len(), "CKP-2 ({case})");
            assert_eq!(g.redo.len(), 1, "CKP-2 ({case})");
            d.button(Button::Redo);
            assert_eq!(d.app.game.board(), saved, "CKP-2, MRK-6: redo ({case})");
            assert_eq!(d.app.game.undo.len(), before.undo.len() + 1, "CKP-2 ({case})");
            assert!(d.app.game.redo.is_empty(), "CKP-2 ({case})");
        }
    });
}

#[test]
fn current_has_no_effect_and_keeps_the_redo_history() {
    // CKP-2: the undo returns the board to the checkpoint, thus the row shows Current while the redo
    // history has an item. A tap on Current must not remove it, and it must not add an undo item.
    for_each_layout(|case| {
        for by in ALL_TOOLS {
            let case = format!("{case}, {by:?}");
            let mut d = with_checkpoints(305, 1);
            new_mark(&mut d);
            d.button(Button::Undo);
            assert_eq!(d.app.game.redo.len(), 1);
            open_checkpoints(&mut d);
            assert_eq!(button_fill(&d.app, 0), BUTTON_CURRENT, "CKP-2 ({case})");
            let before = board(&d.app.game);
            d.app.game_dirty = false;
            tap_by(&mut d, by, restore_point(0));
            assert_eq!(board(&d.app.game), before, "CKP-2: Current changes nothing ({case})");
            assert_eq!(d.app.game.redo.len(), 1, "CKP-2: the redo history stays ({case})");
            assert!(!d.app.game_dirty, "CKP-2 ({case})");
            assert_eq!(panel_of(&d), Some(Panel::Checkpoints), "CKP-2 ({case})");
        }
    });
}

#[test]
fn only_the_row_with_the_same_board_shows_current() {
    // CKP-2: each of the rows in turn. The panel stays open between the restores.
    let mut d = with_checkpoints(306, 3);
    for current in [2, 0, 1] {
        d.finger_tap(restore_point(current).0, restore_point(current).1);
        for i in 0..3 {
            let want = if i == current { BUTTON_CURRENT } else { BUTTON_RESTORE };
            assert_eq!(button_fill(&d.app, i), want, "CKP-2: row {i}, current {current}");
        }
    }
}

#[test]
fn a_difference_only_in_hidden_marks_shows_restore() {
    // CKP-2, MRK-5, MRK-6, §6.1: a mark that a number of a peer hides counts in the comparison. The
    // Restore adds one undo item with no visible change, and undo gives the hidden mark back.
    let mut d = Driver::new(307);
    d.button(Button::Menu);
    let (cell, peer, v) = cell_and_peer(&d.app);
    d.write(cell, v);
    open_checkpoints(&mut d);
    d.finger_tap(save_point().0, save_point().1);
    let saved = d.app.game.board();
    close_checkpoints(&mut d);

    // The same numbers, and the peer now has the mark v, which the number hides.
    d.erase(cell);
    d.tap_micro(peer, v);
    d.write(cell, v);
    let g = &d.app.game;
    assert_eq!(g.value, saved.value, "the same numbers");
    assert_eq!(g.marks_at(peer), mark_bit(v), "MRK-6: the app keeps the hidden mark");
    assert!(!g.same_board(&saved), "same_board includes the hidden marks");
    let with_hidden = g.board();
    for c in cells() {
        assert_eq!(with_hidden.visible_marks(c, true), saved.visible_marks(c, true), "no visible difference at {c:?}");
    }
    assert!(g.can_save_checkpoint(), "CKP-1: no checkpoint is current");

    open_checkpoints(&mut d);
    assert_eq!(button_fill(&d.app, 0), BUTTON_RESTORE, "CKP-2");
    let undo = d.app.game.undo.len();
    d.finger_tap(restore_point(0).0, restore_point(0).1);
    assert_eq!(d.app.game.board(), saved, "CKP-2");
    assert_eq!(d.app.game.undo.len(), undo + 1, "CKP-2: one undo item");
    assert_eq!(button_fill(&d.app, 0), BUTTON_CURRENT, "CKP-2");

    close_checkpoints(&mut d);
    d.button(Button::Undo);
    assert_eq!(d.app.game.board(), with_hidden, "MRK-6: the hidden mark is back");
}

#[test]
fn a_delete_removes_only_its_row_and_keeps_the_restore_items() {
    // CKP-3, CKP-2, UND-9, PER-7: the bin of the row in the middle, with no dialog and no undo. The undo
    // and redo of a Restore give the same boards after the delete of its checkpoint.
    for_each_layout(|case| {
        for by in ALL_TOOLS {
            let case = format!("{case}, {by:?}");
            let mut d = with_checkpoints(308, 3);
            let list = d.app.game.checkpoints.clone();
            new_mark(&mut d);
            open_checkpoints(&mut d);
            let board_before = d.app.game.board();
            d.finger_tap(restore_point(1).0, restore_point(1).1);
            let restored = d.app.game.board();
            assert_eq!(restored, list[1].board, "CKP-2 ({case})");
            d.wait(1000);

            let before = play_state(&d.app.game);
            let t = (d.now, d.app.game.elapsed_ms);
            d.app.game_dirty = false;
            tap_by(&mut d, by, bin_point(1));
            assert_eq!(d.app.game.checkpoints, vec![list[0].clone(), list[2].clone()], "CKP-3 ({case})");
            assert_eq!(play_state(&d.app.game), before, "CKP-3: the board and the stacks ({case})");
            assert_eq!(d.app.game.elapsed_ms, t.1 + (d.now - t.0), "CKP-3: the clock ({case})");
            assert!(d.app.game_dirty, "PER-7 ({case})");
            assert_eq!(panel_of(&d), Some(Panel::Checkpoints), "CKP-3 ({case})");

            close_checkpoints(&mut d);
            d.button(Button::Undo);
            assert_eq!(d.app.game.board(), board_before, "CKP-3: undo after the delete ({case})");
            d.button(Button::Redo);
            assert_eq!(d.app.game.board(), restored, "CKP-2, CKP-3: redo after the delete ({case})");
            assert_eq!(d.app.game.checkpoints.len(), 2, "CKP-3: undo and redo do not change the list ({case})");
        }
    });
}

#[test]
fn a_delete_of_the_current_checkpoint_makes_a_save_available() {
    // CKP-3, CKP-1: the bin of the only row. The board stays.
    let mut d = with_checkpoints(309, 1);
    let board_now = d.app.game.board();
    assert!(!d.app.game.can_save_checkpoint());
    d.finger_tap(bin_point(0).0, bin_point(0).1);
    assert!(d.app.game.checkpoints.is_empty(), "CKP-3");
    assert_eq!(d.app.game.board(), board_now, "CKP-3");
    assert!(d.app.game.can_save_checkpoint(), "CKP-1");
    assert_eq!(panel_of(&d), Some(Panel::Checkpoints), "CKP-3");
    assert_eq!(save_row_grays(&d.app).0, 0, "CKP-1");
}

#[test]
fn a_change_of_the_list_stops_the_other_gesture() {
    // CKP-6: a pen or a finger stays on a control while the other changes the list. The release of the
    // first has no effect: the rows moved, thus it never operates a different checkpoint.
    type Hold = fn(usize) -> (f32, f32);
    for_each_layout(|case| {
        // A delete of row 0 moves the rows up. A save moves them down.
        let cases: [(&str, Hold, usize, (f32, f32), usize); 4] = [
            ("bin 1, delete 0", bin_point, 1, bin_point(0), 2),
            ("button 1, delete 0", restore_point, 1, bin_point(0), 2),
            ("bin 0, save", bin_point, 0, save_point(), 4),
            ("button 0, save", restore_point, 0, save_point(), 4),
        ];
        for (name, held, row, action, want) in cases {
            for finger_holds in [true, false] {
                let case = format!("{case}, {name}, finger holds: {finger_holds}");
                let mut d = with_checkpoints(310, 3);
                // A new board: a save is available and each Restore has an effect.
                new_mark(&mut d);
                open_checkpoints(&mut d);
                let (x, y) = held(row);
                if finger_holds {
                    d.send(Input::TouchDown { id: 1, x, y });
                    d.pen_tap(action.0, action.1);
                } else {
                    d.send(Input::PenDown { x, y, tool: Tool::Pen });
                    d.send(Input::TouchDown { id: 2, x: action.0, y: action.1 });
                    d.send(Input::TouchUp { id: 2, x: action.0, y: action.1 });
                }
                assert_eq!(d.app.game.checkpoints.len(), want, "the action operates ({case})");
                let list = d.app.game.checkpoints.clone();
                let state = play_state(&d.app.game);
                if finger_holds {
                    d.send(Input::TouchUp { id: 1, x, y });
                } else {
                    d.send(Input::PenUp { x, y });
                }
                assert_eq!(d.app.game.checkpoints, list, "CKP-6: the release ({case})");
                assert_eq!(play_state(&d.app.game), state, "CKP-6: the release ({case})");
                assert_eq!(panel_of(&d), Some(Panel::Checkpoints), "CKP-6 ({case})");
            }
        }
    });
}

/// A driver with 3 checkpoints after a tap on the bin of row 0 at the time that it returns.
fn just_deleted(seed: u64) -> (Driver, u64) {
    let mut d = with_checkpoints(seed, 4);
    d.finger_tap(bin_point(0).0, bin_point(0).1);
    assert_eq!(d.app.game.checkpoints.len(), 3, "CKP-3");
    let t = d.now;
    (d, t)
}

/// The state that the guard of CKP-8 must keep.
fn guard_state(d: &Driver) -> (sudoku::game::Game, Menu) {
    (board(&d.app.game), d.app.ui.menu)
}

#[test]
fn the_second_tap_of_a_double_tap_on_a_bin_changes_nothing() {
    // CKP-8: the row that moved up is at the same position. The pen, the eraser and a finger, the four
    // layouts, the bin, the button and the row "Save checkpoint".
    for_each_layout(|case| {
        for by in ALL_TOOLS {
            let case = format!("{case}, {by:?}");
            let (mut d, _) = just_deleted(311);
            let before = guard_state(&d);
            for p in [bin_point(0), restore_point(1), save_point()] {
                tap_by(&mut d, by, p);
                assert_eq!(guard_state(&d), before, "CKP-8: {p:?} ({case})");
            }
        }
    });
}

#[test]
fn the_guard_uses_the_area_of_the_panel_at_its_maximum_size() {
    // CKP-8: the panel has 3 rows and ends at y 1212. A press below it, in the area of the panel with
    // 5 rows, does not close it. The four corners of the area are included. One pixel out of the area,
    // the tap closes the panel (NAV-11).
    for_each_layout(|case| {
        let (x0, x1) = documented_checkpoints_x();
        for p in [at(x0, 560), at(x1, 560), at(x0, 1572), at(x1, 1572), at(x0 + 100, 1400)] {
            let (mut d, _) = just_deleted(312);
            let before = guard_state(&d);
            d.finger_tap(p.0, p.1);
            assert_eq!(guard_state(&d), before, "CKP-8: {p:?} ({case})");
        }
        for p in [at(x0 - 1, 1000), at(x1 + 1, 1000), at(x0 + 100, 559), at(x0 + 100, 1573)] {
            let (mut d, _) = just_deleted(312);
            let (game, _) = guard_state(&d);
            d.finger_tap(p.0, p.1);
            assert_eq!(d.app.ui.menu, Menu::Open(None), "CKP-8, NAV-11: {p:?} ({case})");
            assert_eq!(board(&d.app.game), game, "CKP-8, NAV-11: {p:?} ({case})");
        }
    });
}

#[test]
fn the_guard_ends_after_half_a_second() {
    // CKP-8: the limit, in milliseconds. A press at 499 ms changes nothing, also when its release
    // comes after the time. A press at 500 ms operates.
    for_each_layout(|case| {
        let (x, y) = bin_point(0);
        let (mut d, t) = just_deleted(313);
        let before = guard_state(&d);
        d.wait(489);
        d.send(Input::TouchDown { id: 1, x, y });
        assert_eq!(d.now - t, 499, "{case}");
        assert_eq!(guard_state(&d), before, "CKP-8 ({case})");
        d.wait(100);
        d.send(Input::TouchUp { id: 1, x, y });
        assert_eq!(guard_state(&d), before, "CKP-8: the release of a press in the time ({case})");

        let (mut d, t) = just_deleted(313);
        d.wait(490);
        d.send(Input::TouchDown { id: 1, x, y });
        assert_eq!(d.now - t, 500, "{case}");
        d.send(Input::TouchUp { id: 1, x, y });
        assert_eq!(d.app.game.checkpoints.len(), 2, "CKP-8: the same tap operates after the time ({case})");

        // The time starts again at each delete. A tap below the panel closes it only after the time.
        let (x0, _) = documented_checkpoints_x();
        d.finger_tap((x0 + 100) as f32, 1400.0);
        assert_eq!(panel_of(&d), Some(Panel::Checkpoints), "CKP-8: the time of the second delete ({case})");
        d.wait(BACK_GUARD_MS);
        d.finger_tap((x0 + 100) as f32, 1400.0);
        assert_eq!(d.app.ui.menu, Menu::Open(None), "CKP-8, NAV-11: after the time ({case})");
    });
}

#[test]
fn a_press_on_the_toolbar_operates_as_usual_in_the_guard_time() {
    // CKP-8, NAV-12: the guard does not apply to the toolbar. Undo only closes the panel.
    for_each_layout(|case| {
        let (mut d, _) = just_deleted(314);
        let game = board(&d.app.game);
        d.button(Button::Undo);
        assert_eq!(d.app.ui.menu, Menu::Open(None), "CKP-8 ({case})");
        assert_eq!(board(&d.app.game), game, "NAV-12: only the panel closes ({case})");

        let (mut d, _) = just_deleted(314);
        d.button(Button::Checkpoints);
        assert_eq!(d.app.ui.menu, Menu::Open(None), "CKP-8 ({case})");

        let (mut d, _) = just_deleted(314);
        d.button(Button::Colors);
        assert_eq!(panel_of(&d), Some(Panel::Colors), "CKP-8 ({case})");

        let (mut d, _) = just_deleted(314);
        d.button(Button::Close);
        assert!(!d.app.quit, "NAV-12 ({case})");
        assert_eq!(d.app.ui.menu, Menu::Open(None), "CKP-8 ({case})");
    });
}

/// A game with `n` checkpoints and one empty cell. The panel is closed.
fn nearly_complete(seed: u64, n: usize) -> (Driver, Cell, u8) {
    let mut d = with_checkpoints(seed, n);
    close_checkpoints(&mut d);
    let last = empty_cell(&d.app.game);
    fill_except(&mut d, last);
    let v = d.app.game.solution[last.0 as usize][last.1 as usize];
    (d, last, v)
}

#[test]
fn the_bookmark_is_gray_and_has_no_effect_on_a_complete_puzzle() {
    // CKP-9, CMP-3, CMP-6, NAV-12, UND-6: the list stays until a new game. A different panel stays open,
    // and only Start new game or a level button ends the state. Then the panel operates again.
    for_each_layout(|case| {
        let (mut d, last, v) = nearly_complete(315, 2);
        assert_eq!(bookmark_grays(&d.app), (0, 255), "CKP-9: black in a game in progress ({case})");
        d.write(last, v);
        assert!(d.app.game.completed, "{case}");
        assert_eq!(d.app.game.checkpoints.len(), 2, "CKP-9 ({case})");
        let game = d.app.game.clone();
        assert_eq!(bookmark_grays(&d.app).0, GRAY70, "CKP-9: gray 70 ({case})");

        for by in ALL_TOOLS {
            tap_by(&mut d, by, button_center(Button::Checkpoints));
            assert_eq!(d.app.ui.menu, Menu::Open(None), "CKP-9: {by:?} ({case})");
        }
        // The exception to NAV-12: the other panels stay open.
        for (b, panel) in [(Button::Colors, Panel::Colors), (Button::Settings, Panel::Settings), (Button::NewGame, Panel::NewGame)] {
            d.button(b);
            d.button(Button::Checkpoints);
            assert_eq!(panel_of(&d), Some(panel), "CKP-9 ({case})");
            d.button(b);
            assert_eq!(d.app.ui.menu, Menu::Open(None), "{case}");
        }
        assert_eq!(d.app.game, game, "CMP-3, CKP-9 ({case})");
        assert_eq!(bookmark_grays(&d.app).0, GRAY70, "CKP-9 ({case})");

        // CMP-6: only Start new game or a level button ends the state. UND-6: the list goes.
        d.button(Button::NewGame);
        let (x, y) = pixel_center(layout::ng_start_row());
        d.finger_tap(x, y);
        assert!(!d.app.game.completed, "CMP-6 ({case})");
        assert!(d.app.game.checkpoints.is_empty(), "UND-6 ({case})");
        assert_eq!(bookmark_grays(&d.app), (0, 255), "CKP-9 ({case})");
        open_checkpoints(&mut d);
    });
}

#[test]
fn a_tap_on_the_bookmark_that_reads_the_last_number_does_not_open_the_panel() {
    // CKP-9, CMP-3: the tap reads the waiting ink, and that ink completes the puzzle. A second case: the
    // finger stays on the bookmark while the pen writes the last number.
    for_each_layout(|case| {
        let (mut d, last, v) = nearly_complete(316, 1);
        d.write_strokes(&digit_ink(last, v));
        assert!(!d.app.game.completed, "the ink waits ({case})");
        d.button(Button::Checkpoints);
        assert!(d.app.game.completed, "the tap reads the ink ({case})");
        assert_eq!(d.app.ui.menu, Menu::Open(None), "CKP-9 ({case})");
        assert_eq!(d.app.game.checkpoints.len(), 1, "CKP-9 ({case})");

        let (mut d, last, v) = nearly_complete(316, 1);
        let (x, y) = hold(&mut d, Button::Checkpoints);
        d.write(last, v);
        assert!(d.app.game.completed, "{case}");
        d.send(Input::TouchUp { id: 1, x, y });
        assert_eq!(d.app.ui.menu, Menu::Open(None), "CKP-9: the release ({case})");
    });
}

#[test]
fn a_tap_on_the_bookmark_reads_the_waiting_ink_and_opens_the_panel() {
    // NAV-18: with the panel open, no ink waits and no stroke is live. The number of the ink is on the
    // board before the panel opens, thus a save keeps it.
    for_each_layout(|case| {
        let mut d = Driver::new(317);
        d.button(Button::Menu);
        let cell = empty_cell(&d.app.game);
        let v = d.app.game.solution[cell.0 as usize][cell.1 as usize];
        d.write_strokes(&digit_ink(cell, v));
        assert!(d.app.ui.pending_ink.is_some(), "{case}");
        d.button(Button::Checkpoints);
        assert_eq!(panel_of(&d), Some(Panel::Checkpoints), "NAV-18 ({case})");
        assert!(d.app.ui.pending_ink.is_none(), "NAV-18 ({case})");
        assert_eq!(d.app.game.value_at(cell), v, "the tap reads the ink ({case})");
        d.finger_tap(save_point().0, save_point().1);
        assert_eq!(d.app.game.checkpoints[0].board.value[cell.0 as usize][cell.1 as usize], v, "CKP-1 ({case})");
    });
}

#[test]
fn the_panel_never_has_a_live_stroke() {
    // NAV-18: the pen in the panel starts no stroke. The bookmark opens the panel while the pen is on the
    // board, and the stroke stops.
    for_each_layout(|case| {
        let mut d = with_checkpoints(318, 1);
        let (x, y) = at(documented_checkpoints_x().0 + 200, row_y(0) + 90);
        d.send(Input::PenDown { x, y, tool: Tool::Pen });
        d.send(Input::PenMove { x: x + 40.0, y: y + 40.0 });
        assert!(d.app.live_stroke.is_none(), "NAV-18 ({case})");
        d.send(Input::PenUp { x: x + 40.0, y: y + 40.0 });
        assert!(d.app.ui.pending_ink.is_none(), "NAV-18 ({case})");
        assert_eq!(panel_of(&d), Some(Panel::Checkpoints), "NAV-18: no control ({case})");

        // The pen is on the board. A finger taps the bookmark.
        let mut d = Driver::new(318);
        d.button(Button::Menu);
        let cell = empty_cell(&d.app.game);
        let (x, y) = micro_center(cell, 5);
        d.send(Input::PenDown { x, y, tool: Tool::Pen });
        d.send(Input::PenMove { x: x + 5.0, y: y + 5.0 });
        d.button(Button::Checkpoints);
        assert_eq!(panel_of(&d), Some(Panel::Checkpoints), "NAV-18 ({case})");
        assert!(d.app.live_stroke.is_none() && d.app.ui.pending_ink.is_none(), "NAV-18: a finger on the bookmark ({case})");
        d.send(Input::PenMove { x: x + 10.0, y: y + 10.0 });
        d.send(Input::PenUp { x: x + 10.0, y: y + 10.0 });
        assert!(d.app.live_stroke.is_none() && d.app.ui.pending_ink.is_none(), "NAV-18: after the release ({case})");

        // A finger stays on the bookmark while the pen starts a stroke. The release opens the panel.
        let mut d = Driver::new(318);
        d.button(Button::Menu);
        let (fx, fy) = hold(&mut d, Button::Checkpoints);
        d.send(Input::PenDown { x, y, tool: Tool::Pen });
        d.send(Input::PenMove { x: x + 5.0, y: y + 5.0 });
        d.send(Input::TouchUp { id: 1, x: fx, y: fy });
        assert_eq!(panel_of(&d), Some(Panel::Checkpoints), "NAV-18 ({case})");
        assert!(d.app.live_stroke.is_none() && d.app.ui.pending_ink.is_none(), "NAV-18: a held finger ({case})");
    });
}

#[test]
fn a_new_game_removes_the_checkpoints() {
    // UND-6: undo, redo and the list. The old boards do not stay as a Restore or a Current.
    for_each_layout(|case| {
        let mut d = with_checkpoints(319, 3);
        new_mark(&mut d);
        d.button(Button::Undo);
        assert!(d.app.game.can_undo() && d.app.game.can_redo());
        d.button(Button::NewGame);
        let (x, y) = pixel_center(layout::ng_start_row());
        d.finger_tap(x, y);
        let g = &d.app.game;
        assert!(g.undo.is_empty() && g.redo.is_empty() && g.checkpoints.is_empty(), "UND-6 ({case})");
        assert!(g.can_save_checkpoint(), "CKP-1 ({case})");
    });
}

#[test]
fn with_the_panel_open_undo_redo_and_the_other_buttons_only_close_it() {
    // NAV-12: the first tap only closes the panel, and a second tap on undo or redo does the action.
    // The pen and a finger, the four layouts.
    for_each_layout(|case| {
        for b in [Button::Menu, Button::Close, Button::Undo, Button::Redo, Button::Help, Button::Statistics] {
            for by in [By::Finger, By::Pen] {
                let case = format!("{case}, {b:?}, {by:?}");
                let mut d = Driver::new(320);
                d.button(Button::Menu);
                fill_stacks(&mut d);
                d.button(Button::Checkpoints);
                assert_eq!(panel_of(&d), Some(Panel::Checkpoints), "{case}");
                let game = board(&d.app.game);
                tap_by(&mut d, by, button_center(b));
                assert_eq!(d.app.ui.menu, Menu::Open(None), "NAV-12: the menu stays and the panel closes ({case})");
                assert_eq!(d.app.screen, Screen::Game, "NAV-12 ({case})");
                assert!(!d.app.quit, "NAV-12 ({case})");
                assert_eq!(board(&d.app.game), game, "NAV-12 ({case})");
                match b {
                    Button::Undo => {
                        tap_by(&mut d, by, button_center(b));
                        assert_eq!((d.app.game.undo.len(), d.app.game.redo.len()), (0, 2), "NAV-12: the second tap does the undo ({case})");
                    }
                    Button::Redo => {
                        tap_by(&mut d, by, button_center(b));
                        assert_eq!((d.app.game.undo.len(), d.app.game.redo.len()), (2, 0), "NAV-12: the second tap does the redo ({case})");
                    }
                    _ => {}
                }
            }
        }
    });
}

#[test]
fn the_panel_buttons_change_the_panel() {
    // NAV-12, NAV-18: the bookmark, the paint bucket, the gear and the + change the panel. A second tap
    // on the bookmark closes its panel.
    for_each_layout(|case| {
        let mut d = Driver::new(321);
        d.button(Button::Menu);
        for (b, panel) in [(Button::Colors, Panel::Colors), (Button::Settings, Panel::Settings), (Button::NewGame, Panel::NewGame)] {
            d.button(Button::Checkpoints);
            assert_eq!(panel_of(&d), Some(Panel::Checkpoints), "NAV-18 ({case})");
            d.button(b);
            assert_eq!(panel_of(&d), Some(panel), "NAV-12 ({case})");
            d.button(Button::Checkpoints);
            assert_eq!(panel_of(&d), Some(Panel::Checkpoints), "NAV-12 ({case})");
            d.button(Button::Checkpoints);
            assert_eq!(d.app.ui.menu, Menu::Open(None), "NAV-18 ({case})");
        }
    });
}

#[test]
fn the_bookmark_shows_the_state_of_its_panel() {
    // NAV-8, CKP-9: a black icon on white. While the panel is open: a white icon on a black slot.
    for_each_layout(|case| {
        let mut d = Driver::new(322);
        d.button(Button::Menu);
        assert_eq!(bookmark_grays(&d.app), (0, 255), "CKP-9 ({case})");
        let pm = draw_screen(&d.app, &fonts());
        let (x, y) = (layout::tb_cx() as i32, layout::tb_cy(5) as i32);
        assert_eq!(rgb_at(&pm, x - 50, y - 50), [255, 255, 255], "NAV-8 ({case})");
        d.button(Button::Checkpoints);
        assert_eq!(bookmark_grays(&d.app), (0, 255), "CKP-9 ({case})");
        let pm = draw_screen(&d.app, &fonts());
        assert_eq!(rgb_at(&pm, x - 50, y - 50), [0, 0, 0], "NAV-8: the slot is black ({case})");
    });
}

#[test]
fn the_bookmark_is_in_top_slot_5_and_obeys_the_guard_after_back() {
    // NAV-16, NAV-18: y 560-671 in the four layouts, with the two ends. For BACK_GUARD_MS after "< Back",
    // a press on these pixels changes nothing, with the pen and with a finger. Then it opens the panel.
    for_each_layout(|case| {
        let x = layout::tb_cx();
        assert_eq!(layout::game_button(x, 559.0, true), Some(Button::Redo), "{case}");
        assert_eq!(layout::game_button(x, 560.0, true), Some(Button::Checkpoints), "NAV-18 ({case})");
        assert_eq!(layout::game_button(x, 671.0, true), Some(Button::Checkpoints), "NAV-18 ({case})");
        assert_eq!(layout::game_button(x, 672.0, true), None, "NAV-18 ({case})");
        assert_eq!(layout::game_button(x, 615.0, false), None, "the closed menu has no bookmark ({case})");
        for y in [560.0, 615.0, 671.0] {
            for by in [By::Finger, By::Pen] {
                let case = format!("{case}, y {y}, {by:?}");
                let mut d = Driver::new(323);
                back_from(&mut d, Button::Help, false);
                let game = board(&d.app.game);
                d.wait(BACK_GUARD_MS - 50);
                tap_by(&mut d, by, (x, y));
                assert_eq!((d.app.screen, d.app.ui.menu), (Screen::Game, Menu::Open(None)), "NAV-16 ({case})");
                assert_eq!(board(&d.app.game), game, "NAV-16 ({case})");
                d.wait(50);
                tap_by(&mut d, by, (x, y));
                assert_eq!(panel_of(&d), Some(Panel::Checkpoints), "NAV-16: it operates after the time ({case})");
            }
        }
    });
}

#[test]
fn the_panel_has_the_documented_position_in_the_four_layouts() {
    // NAV-18: the x range, y 560, and the bottom y 864 or 672 + 180 n. The corners are in the panel: a tap on
    // a corner has no effect, and the border is black. One pixel out of the panel, a tap only closes it.
    for_each_layout(|case| {
        let (x0, x1) = documented_checkpoints_x();
        assert_eq!(x1 - x0 + 1, 600, "NAV-18 ({case})");
        for (n, bottom) in [(0, 864), (1, 852), (2, 1032), (3, 1212), (5, 1572)] {
            let case = format!("{case}, {n} rows");
            let r = layout::ckp_panel(n);
            assert_eq!((r.x0, r.y0, r.x1 - 1, r.y1 - 1), (x0, 560, x1, bottom), "NAV-18 ({case})");

            let d = with_checkpoints(324, n);
            let pm = draw_screen(&d.app, &fonts());
            for (x, y) in [(x0, 560), (x1, 560), (x0, bottom), (x1, bottom)] {
                assert_eq!(rgb_at(&pm, x, y), [0, 0, 0], "NAV-18: the border at ({x}, {y}) ({case})");
            }

            // The corners of the bottom edge are on no control. So are the corners of the top edge when
            // a save is not available.
            let mut corners = vec![at(x0, bottom), at(x1, bottom)];
            if n > 0 {
                corners.extend([at(x0, 560), at(x1, 560)]);
            }
            let mut d = with_checkpoints(324, n);
            let game = board(&d.app.game);
            for (x, y) in corners {
                for by in ALL_TOOLS {
                    tap_by(&mut d, by, (x, y));
                    assert_eq!(panel_of(&d), Some(Panel::Checkpoints), "NAV-18: ({x}, {y}), {by:?} ({case})");
                    assert_eq!(board(&d.app.game), game, "NAV-18: ({x}, {y}), {by:?} ({case})");
                }
            }

            for (x, y) in [(x0 - 1, 700), (x1 + 1, 700), (x0 + 100, 559), (x0 + 100, bottom + 1)] {
                let mut d = with_checkpoints(324, n);
                let game = board(&d.app.game);
                d.finger_tap(x as f32, y as f32);
                assert_eq!(d.app.ui.menu, Menu::Open(None), "NAV-11, NAV-18: ({x}, {y}) ({case})");
                assert_eq!(board(&d.app.game), game, "NAV-11: ({x}, {y}) ({case})");
            }
        }
    });
}

#[test]
fn each_control_of_the_panel_is_at_its_documented_place() {
    // NAV-18, R§9.3: the same distance from the left edge of the panel in the four layouts, with the two
    // ends of each area. The 11 px between the button and the bin operate nothing.
    use layout::{checkpoint_control_at as control, CheckpointControl as C};
    for_each_layout(|case| {
        let (x0, x1) = documented_checkpoints_x();
        let px = |x: i32, y: i32| control(x as f32, y as f32, 5);
        assert_eq!(px(x0, 560), Some(C::Save), "{case}");
        assert_eq!(px(x1, 671), Some(C::Save), "{case}");
        assert_eq!(px(x0 + 300, 672), None, "{case}");
        for i in 0..5 {
            let y0 = row_y(i);
            let case = format!("{case}, row {i}");
            for (dx, dy) in [(381, 55), (521, 55), (381, 125), (521, 125), (451, 90)] {
                assert_eq!(px(x0 + dx, y0 + dy), Some(C::Restore(i)), "the button ({case}): {dx}, {dy}");
            }
            for (dx, dy) in [(380, 90), (522, 90), (451, 54), (451, 126), (532, 90), (100, 90), (240, 90), (0, 90)] {
                assert_eq!(px(x0 + dx, y0 + dy), None, "no control ({case}): {dx}, {dy}");
            }
            for (x, dy) in [(x0 + 533, 0), (x1, 0), (x0 + 533, 179), (x1, 179), (x0 + 560, 90)] {
                assert_eq!(px(x, y0 + dy), Some(C::Delete(i)), "the bin ({case}): {x}, {dy}");
            }
        }
        // The last row has no controls below it. With fewer rows, the place of a row has none.
        assert_eq!(px(x1, row_y(5)), None, "{case}");
        assert_eq!(control((x0 + 451) as f32, (row_y(2) + 90) as f32, 2), None, "{case}");
        assert_eq!(control((x0 + 560) as f32, (row_y(2) + 90) as f32, 2), None, "{case}");
    });
}

#[test]
fn a_tap_in_the_panel_on_no_control_has_no_effect() {
    // NAV-18, R§9.3: the small board, the text, the gap of 11 px and the place below the button. The panel
    // covers cells of the board, but the board does not get the tap (NAV-11).
    for_each_layout(|case| {
        let (x0, _) = documented_checkpoints_x();
        let mut d = with_checkpoints(325, 2);
        new_mark(&mut d);
        open_checkpoints(&mut d);
        let game = board(&d.app.game);
        for by in ALL_TOOLS {
            for (dx, dy) in [(100, 90), (240, 90), (527, 90), (451, 40), (451, 140)] {
                tap_by(&mut d, by, at(x0 + dx, row_y(1) + dy));
                assert_eq!(panel_of(&d), Some(Panel::Checkpoints), "NAV-18: {dx}, {dy}, {by:?} ({case})");
                assert_eq!(board(&d.app.game), game, "NAV-18: {dx}, {dy}, {by:?} ({case})");
            }
        }
    });
}

#[test]
fn the_taps_operate_the_documented_row_in_the_four_layouts() {
    // NAV-18, CKP-2, CKP-3: a real tap at the two ends of the areas. It selects the row of the documents.
    for_each_layout(|case| {
        let (x0, x1) = documented_checkpoints_x();
        for (p, row) in [(at(x0 + 381, row_y(2) + 55), 2), (at(x0 + 521, row_y(1) + 125), 1), (at(x0 + 451, row_y(0) + 90), 0)] {
            let mut d = with_checkpoints(326, 3);
            new_mark(&mut d);
            open_checkpoints(&mut d);
            let want = d.app.game.checkpoints[row].board.clone();
            d.finger_tap(p.0, p.1);
            assert_eq!(d.app.game.board(), want, "CKP-2: row {row} ({case})");
        }
        for (p, row) in [(at(x0 + 533, row_y(2)), 2), (at(x1, row_y(1) + 179), 1), (at(x1, row_y(0)), 0)] {
            let mut d = with_checkpoints(326, 3);
            let mut want = d.app.game.checkpoints.clone();
            want.remove(row);
            d.finger_tap(p.0, p.1);
            assert_eq!(d.app.game.checkpoints, want, "CKP-3: row {row} ({case})");
        }
    });
}

#[test]
fn the_panel_covers_no_toolbar_button_close_button_or_meter() {
    // NAV-18: the panel at its maximum size, in the four layouts.
    for_each_layout(|case| {
        let r = layout::ckp_panel(5);
        for y in (r.y0..r.y1).step_by(4) {
            for x in (r.x0..r.x1).step_by(4) {
                assert_eq!(layout::game_button(x as f32, y as f32, true), None, "NAV-18: ({x}, {y}) ({case})");
            }
        }
        for b in [Button::Menu, Button::Colors, Button::Settings, Button::Undo, Button::Redo, Button::Checkpoints, Button::Help, Button::Statistics, Button::NewGame, Button::Close] {
            let (x, y) = button_center(b);
            assert!(!r.contains(x, y), "NAV-18: {b:?} ({case})");
        }
        for x in [layout::meter_xl(), layout::meter_xr()] {
            assert!(!r.contains(x as f32, layout::meter_y() as f32), "NAV-18: the meter ({case})");
        }
    });
}

// ---- The highlight, hand touches and the ink start (owner decision 2026-11-07): HLT-2 to HLT-9,
// INP-5, INP-7, INP-12, INP-16, INP-20 to INP-24, QST-4, MRK-15, NAV-11, NAV-14.

/// Taps the first starting number and returns its cell and its value (HLT-3).
fn highlight_a_number(d: &mut Driver) -> (Cell, u8) {
    let given = given_cell(&d.app.game);
    let v = d.app.game.value_at(given);
    d.tap_cell(given);
    assert_eq!(d.app.ui.highlight, Some(v), "HLT-3");
    (given, v)
}

/// A filled cell with a value that is not `v`.
fn other_number(d: &Driver, v: u8) -> (Cell, u8) {
    let c = cells().find(|&c| d.app.game.is_filled(c) && d.app.game.value_at(c) != v).unwrap();
    (c, d.app.game.value_at(c))
}

/// A point off the board that is on no control, in the current layout.
fn no_control_point(menu_open: bool) -> (f32, f32) {
    for y in (4..layout::screen_h() as i32).step_by(8) {
        for x in (4..layout::screen_w() as i32).step_by(8) {
            let (x, y) = (x as f32, y as f32);
            if layout::board_cell(x, y).is_none() && layout::board_micro(x, y).is_none() && layout::game_button(x, y, menu_open).is_none() {
                return (x, y);
            }
        }
    }
    panic!("no point off the board");
}

/// A pen stroke from micro-cell 1 to micro-cell 9 of a cell. The test sends the pen lift.
fn stroke_down(d: &mut Driver, cell: Cell) -> (f32, f32) {
    let (x0, y0) = micro_center(cell, 1);
    let (x1, y1) = micro_center(cell, 9);
    d.send(Input::PenDown { x: x0, y: y0, tool: Tool::Pen });
    d.send(Input::PenMove { x: (x0 + x1) / 2.0, y: (y0 + y1) / 2.0 });
    d.send(Input::PenMove { x: x1, y: y1 });
    (x1, y1)
}

/// `PUZZLE`'s solution with all numbers given except the digit `digit`: only one solution, and no
/// cell has `digit`.
fn board_without(digit: u8) -> sudoku::game::Game {
    let rows: Vec<String> = SOLUTION.iter().map(|r| r.replace(char::from(b'0' + digit), ".")).collect();
    let rows: [&str; 9] = std::array::from_fn(|i| rows[i].as_str());
    board_of(&rows)
}

#[test]
fn a_pen_tap_on_a_number_changes_the_highlight_only_at_the_pen_lift() {
    // HLT-2, HLT-3, HLT-9: the toggle and the move are at the pen lift, and the press changes nothing.
    for_each_layout(|case| {
        let mut d = Driver::new(401);
        let (given, v) = highlight_a_number(&mut d);
        let (other, w) = other_number(&d, v);
        let (x, y) = micro_center(other, 5);
        d.send(Input::PenDown { x, y, tool: Tool::Pen });
        assert_eq!(d.app.ui.highlight, Some(v), "HLT-9: a press on a different number ({case})");
        d.send(Input::PenUp { x, y });
        assert_eq!(d.app.ui.highlight, Some(w), "HLT-3: the move at the lift ({case})");
        let (x, y) = micro_center(other, 9);
        d.send(Input::PenDown { x, y, tool: Tool::Pen });
        assert_eq!(d.app.ui.highlight, Some(w), "HLT-9: a press on the same number ({case})");
        d.send(Input::PenUp { x, y });
        assert_eq!(d.app.ui.highlight, None, "HLT-3: the toggle at the lift ({case})");
        // A different cell with the same value also toggles.
        d.tap_cell(given);
        let same = cells().find(|&c| c != given && d.app.game.value_at(c) == v);
        if let Some(same) = same {
            d.tap_cell(same);
            assert_eq!(d.app.ui.highlight, None, "HLT-3: the same value in a different cell ({case})");
        }
    });
}

#[test]
fn a_pen_tap_on_an_empty_cell_removes_the_highlight_at_the_lift_with_its_mark_or_x() {
    // HLT-2 (item 2), HLT-9, MRK-12: the removal and the mark, or the x, are in the same step.
    for_each_layout(|case| {
        let mut d = Driver::new(402);
        highlight_a_number(&mut d);
        let (cell, digit) = cells()
            .filter(|&c| !d.app.game.is_filled(c))
            .find_map(|c| (1..=9u8).find(|&v| !peer_has(&d.app.game, c, v)).map(|v| (c, v)))
            .unwrap();
        let (x, y) = micro_center(cell, digit);
        d.app.redraw = false;
        d.send(Input::PenDown { x, y, tool: Tool::Pen });
        assert!(d.app.ui.highlight.is_some() && !d.app.redraw, "HLT-9: the press ({case})");
        assert_eq!(d.app.game.marks_at(cell), 0, "HLT-9 ({case})");
        d.send(Input::PenUp { x, y });
        assert_eq!(d.app.ui.highlight, None, "HLT-2 ({case})");
        assert_eq!(d.app.game.marks_at(cell), mark_bit(digit), "the mark at the lift ({case})");
        assert!(d.app.redraw, "{case}");

        highlight_a_number(&mut d);
        let (cell, digit) = refused_pair(&d.app.game);
        let (x, y) = micro_center(cell, digit);
        d.send(Input::PenDown { x, y, tool: Tool::Pen });
        assert!(d.app.ui.highlight.is_some(), "HLT-9: the press of a refused tap ({case})");
        assert_eq!(d.app.ui.refused, None, "{case}");
        d.send(Input::PenUp { x, y });
        assert_eq!(d.app.ui.highlight, None, "HLT-2 ({case})");
        assert_eq!(d.app.ui.refused, Some((cell, digit, d.now)), "the x at the lift ({case})");
    });
}

#[test]
fn a_stroke_from_a_filled_cell_removes_the_highlight_at_the_pen_lift() {
    // HLT-2 (item 3), HLT-9, INP-7, INP-16, INP-22: a starting number and an entry. The stroke changes
    // only the highlight. The redraw can wait up to the idle time, and a tap in this time makes a
    // highlight.
    for_each_layout(|case| {
        let mut d = Driver::new(403);
        let entry = empty_cell(&d.app.game);
        let value = d.app.game.solution[entry.0 as usize][entry.1 as usize];
        d.write(entry, value);
        assert_eq!(d.app.game.value_at(entry), value, "{case}");
        let given = given_cell(&d.app.game);
        for cell in [given, entry] {
            let case = format!("{case}, cell {cell:?}");
            d.wait(d.app.pause_ms);
            d.tap_cell(cell);
            let v = d.app.game.value_at(cell);
            assert_eq!(d.app.ui.highlight, Some(v), "{case}");
            let before = board(&d.app.game);
            let (x, y) = stroke_down(&mut d, cell);
            assert_eq!(d.app.ui.highlight, Some(v), "HLT-9: during the stroke ({case})");
            d.send(Input::PenUp { x, y });
            assert_eq!(d.app.ui.highlight, None, "INP-7: at the pen lift ({case})");
            assert!(d.app.ui.pending_ink.is_none(), "INP-7: no read ({case})");
            assert_eq!(play_state(&d.app.game), play_state(&before), "INP-7 ({case})");
            assert!(d.app.is_writing(), "INP-16: the timer runs ({case})");
            let lift = d.now;
            tick_to(&mut d, lift + WRITING_IDLE_MS - 1);
            assert!(d.app.is_writing(), "INP-16: 1 s ({case})");
            tick_to(&mut d, lift + WRITING_IDLE_MS);
            assert!(!d.app.is_writing(), "INP-16: 1 s ({case})");
            // A tap on that number in the idle time.
            let (x, y) = stroke_down(&mut d, cell);
            d.send(Input::PenUp { x, y });
            d.tap_cell(cell);
            assert_eq!(d.app.ui.highlight, Some(v), "HLT-9: a tap in the idle time ({case})");
            d.tap_cell(cell);
            assert_eq!(d.app.ui.highlight, None, "{case}");
        }
    });
}

#[test]
fn each_read_of_waiting_ink_removes_the_highlight_with_the_result() {
    // HLT-2 (item 4), HLT-9, INP-5, INP-12, INP-21: all causes of a read, with a number and with a "?".
    // While the ink waits, the highlight stays.
    #[derive(Clone, Copy, Debug)]
    enum Cause {
        Pause,
        PenOnEmptyCell,
        PenOnNumber,
        PenOffBoard,
        PenOnControl,
        FingerOnControl,
    }
    for_each_layout(|case| {
        for cause in [Cause::Pause, Cause::PenOnEmptyCell, Cause::PenOnNumber, Cause::PenOffBoard, Cause::PenOnControl, Cause::FingerOnControl] {
            for number in [true, false] {
                let case = format!("{case}, {cause:?}, number: {number}");
                let mut d = Driver::new(404);
                let (given, v) = highlight_a_number(&mut d);
                let (cell, second) = {
                    let mut e = cells().filter(|&c| !d.app.game.is_filled(c));
                    (e.next().unwrap(), e.next().unwrap())
                };
                let value = 4;
                let ink = if number { digit_ink(cell, value) } else { scribble_ink(cell) };
                d.write_strokes(&ink);
                assert_eq!(d.app.ui.highlight, Some(v), "HLT-9: the highlight stays with waiting ink ({case})");
                assert!(d.app.ui.pending_ink.is_some(), "{case}");
                let expect_read = |d: &Driver, what: &str| {
                    assert!(d.app.ui.pending_ink.is_none(), "{what}: the ink is read ({case})");
                    if number {
                        assert_eq!(d.app.game.value_at(cell), value, "{what} ({case})");
                    } else {
                        assert_eq!(d.app.ui.question.map(|q| q.0), Some(cell), "{what} ({case})");
                    }
                };
                match cause {
                    Cause::Pause => {
                        d.wait(d.app.pause_ms);
                        expect_read(&d, "the pause");
                        assert_eq!(d.app.ui.highlight, None, "HLT-2 ({case})");
                    }
                    Cause::PenOnEmptyCell | Cause::PenOnNumber | Cause::PenOffBoard | Cause::PenOnControl => {
                        let (x, y) = match cause {
                            Cause::PenOnEmptyCell => micro_center(second, 5),
                            Cause::PenOnNumber => micro_center(given, 5),
                            Cause::PenOffBoard => no_control_point(false),
                            _ => button_center(Button::Menu),
                        };
                        d.send(Input::PenDown { x, y, tool: Tool::Pen });
                        expect_read(&d, "the pen press");
                        assert_eq!(d.app.ui.highlight, None, "HLT-2 ({case})");
                        d.send(Input::PenUp { x, y });
                        if !matches!(cause, Cause::PenOnNumber) {
                            assert_eq!(d.app.ui.highlight, None, "HLT-2 ({case})");
                        }
                    }
                    Cause::FingerOnControl => {
                        let (x, y) = button_center(Button::Menu);
                        d.send(Input::TouchDown { id: 1, x, y });
                        assert_eq!(d.app.ui.highlight, Some(v), "INP-21: no read at the finger press ({case})");
                        assert!(d.app.ui.pending_ink.is_some(), "INP-21 ({case})");
                        d.send(Input::TouchUp { id: 1, x, y });
                        expect_read(&d, "the release");
                        assert_eq!(d.app.ui.highlight, None, "HLT-2 ({case})");
                        assert!(d.app.ui.menu.is_open(), "{case}");
                    }
                }
            }
        }
    });
}

#[test]
fn a_tap_on_a_number_with_ink_elsewhere_uses_the_highlight_of_the_press() {
    // HLT-3, HLT-9, INP-12: highlight v and ink in a different cell. The press reads the ink. At the
    // lift, a tap on v gives no highlight, and a tap on w gives w. A "?" can show with w.
    for_each_layout(|case| {
        let mut d = Driver::new(405);
        let (given, v) = highlight_a_number(&mut d);
        let cell = empty_cell(&d.app.game);
        d.write_strokes(&scribble_ink(cell));
        let (x, y) = micro_center(given, 5);
        d.send(Input::PenDown { x, y, tool: Tool::Pen });
        assert_eq!(d.app.ui.highlight, None, "HLT-2 (item 4): the read at the press ({case})");
        assert!(d.app.ui.pending_ink.is_none(), "INP-12 ({case})");
        d.send(Input::PenUp { x, y });
        assert_eq!(d.app.ui.highlight, None, "HLT-9: a tap on v ({case})");
        assert_eq!(d.app.ui.question.map(|q| q.0), Some(cell), "{case}");

        let mut d = Driver::new(405);
        let (_, v2) = highlight_a_number(&mut d);
        assert_eq!(v2, v);
        let (other, w) = other_number(&d, v);
        let cell = empty_cell(&d.app.game);
        d.write_strokes(&scribble_ink(cell));
        d.tap_cell(other);
        assert_eq!(d.app.ui.highlight, Some(w), "HLT-9: a tap on w ({case})");
        assert_eq!(d.app.ui.question.map(|q| q.0), Some(cell), "HLT-9: the \"?\" shows with w ({case})");

        // No highlight at the press: a tap on a number makes a highlight, also with ink elsewhere.
        let mut d = Driver::new(405);
        let cell = empty_cell(&d.app.game);
        let (other, w) = other_number(&d, 0);
        d.write_strokes(&digit_ink(cell, 4));
        d.tap_cell(other);
        assert_eq!(d.app.ui.highlight, Some(w), "HLT-3 ({case})");
        assert_eq!(d.app.game.value_at(cell), 4, "INP-12 ({case})");
    });
}

#[test]
fn a_new_game_a_restart_and_a_completed_puzzle_remove_the_highlight() {
    // HLT-1, HLT-2 (item 5).
    for_each_layout(|case| {
        // A new game, with the panel of the menu.
        let mut d = Driver::new(406);
        let (_, v) = highlight_a_number(&mut d);
        d.button(Button::Menu);
        d.button(Button::NewGame);
        assert_eq!(d.app.ui.highlight, Some(v), "HLT-7: the panel keeps it ({case})");
        let (x, y) = exact_center(layout::ng_start_row());
        d.finger_tap(x, y);
        assert_eq!(d.app.ui.menu.panel(), None, "the game started ({case})");
        assert_eq!(d.app.ui.highlight, None, "HLT-2: a new game ({case})");

        // A restart of the app: the highlight is not saved.
        let mut d = Driver::new(407);
        highlight_a_number(&mut d);
        let again = app_with_game(d.app.game.clone(), START_TIME, 1);
        assert_eq!(again.game.seed, d.app.game.seed, "the saved game opens ({case})");
        assert_eq!(again.ui.highlight, None, "HLT-2: a restart ({case})");

        // A completed puzzle, with a number that the pause reads and with one that a pen press reads.
        for press in [false, true] {
            let mut d = Driver::from(app_with_game(puzzle_with(&[]), START_TIME, 1));
            let last = empty_cell(&d.app.game);
            fill_except(&mut d, last);
            highlight_a_number(&mut d);
            let value = d.app.game.solution[last.0 as usize][last.1 as usize];
            d.write_strokes(&digit_ink(last, value));
            assert!(d.app.ui.highlight.is_some(), "HLT-9 ({case})");
            if press {
                let (x, y) = no_control_point(false);
                d.pen_tap(x, y);
            } else {
                d.wait(d.app.pause_ms);
            }
            assert!(d.app.game.completed, "{case}");
            assert_eq!(d.app.ui.highlight, None, "HLT-1, HLT-2: a completed puzzle ({case})");
        }
    });
}

#[test]
fn help_and_statistics_remove_the_highlight_and_it_does_not_come_back() {
    // HLT-2 (item 6), HLT-4: with the pen and with a finger.
    for_each_layout(|case| {
        for page in [Button::Help, Button::Statistics] {
            for pen in [false, true] {
                let case = format!("{case}, {page:?}, pen: {pen}");
                let mut d = Driver::new(408);
                let (_, v) = highlight_a_number(&mut d);
                d.button(Button::Menu);
                assert_eq!(d.app.ui.highlight, Some(v), "{case}");
                let (x, y) = button_center(page);
                if pen { d.pen_tap(x, y) } else { d.finger_tap(x, y) }
                assert_ne!(d.app.screen, Screen::Game, "{case}");
                assert_eq!(d.app.ui.highlight, None, "HLT-4 ({case})");
                tap_back(&mut d, pen);
                assert_eq!(d.app.screen, Screen::Game, "{case}");
                assert_eq!(d.app.ui.highlight, None, "HLT-4: not on return ({case})");
            }
        }
    });
}

#[test]
fn the_update_dialog_removes_the_highlight_and_it_does_not_come_back() {
    // HLT-2 (item 7), UPD-10.
    for_each_layout(|case| {
        let mut d = Driver::new(409);
        d.app.update = reference::update_state(true);
        d.app.update.tap();
        d.app.update.request = None;
        highlight_a_number(&mut d);
        let now = d.now;
        d.app.update_installed(Ok(true), now);
        assert!(d.app.update.dialog_shown(), "{case}");
        assert_eq!(d.app.ui.highlight, None, "HLT-2 ({case})");
        let (x, y) = exact_center(layout::dialog_cancel());
        d.finger_tap(x, y);
        assert!(!d.app.update.dialog_shown(), "{case}");
        assert_eq!(d.app.ui.highlight, None, "HLT-2: not after Later ({case})");
    });
}

#[test]
fn turning_off_the_highlight_switch_removes_the_highlight_in_that_redraw() {
    // HLT-2 (item 8), HLT-6, NAV-14: the panel stays open. The other switches and the version row keep
    // the highlight. Turning it on again makes no highlight.
    for_each_layout(|case| {
        for pen in [false, true] {
            let case = format!("{case}, pen: {pen}");
            let mut d = Driver::new(410);
            let (given, v) = highlight_a_number(&mut d);
            d.button(Button::Menu);
            d.button(Button::Settings);
            let tap = |d: &mut Driver, (x, y): (f32, f32)| if pen { d.pen_tap(x, y) } else { d.finger_tap(x, y) };
            for s in [Switch::HideMarksAutomatically, Switch::ColorCompletedGroups] {
                tap(&mut d, switch_point(s));
                assert_eq!(d.app.ui.highlight, Some(v), "NAV-14: {s:?} ({case})");
            }
            tap(&mut d, exact_center(layout::settings_row(3)));
            assert_eq!(d.app.ui.highlight, Some(v), "NAV-14: the version row ({case})");
            let (x, y) = switch_point(Switch::HighlightNumbers);
            d.app.redraw = false;
            d.send(if pen { Input::PenDown { x, y, tool: Tool::Pen } } else { Input::TouchDown { id: 1, x, y } });
            assert_eq!(d.app.ui.highlight, Some(v), "the action is at the release ({case})");
            d.send(if pen { Input::PenUp { x, y } } else { Input::TouchUp { id: 1, x, y } });
            assert!(!d.app.settings.highlight_on, "{case}");
            assert_eq!(d.app.ui.highlight, None, "HLT-6 ({case})");
            assert!(d.app.redraw, "HLT-6: that redraw ({case})");
            assert_eq!(d.app.ui.menu.panel(), Some(Panel::Settings), "HLT-6: the panel stays open ({case})");
            tap(&mut d, switch_point(Switch::HighlightNumbers));
            assert!(d.app.settings.highlight_on, "{case}");
            assert_eq!(d.app.ui.highlight, None, "HLT-3: only a tap makes a highlight ({case})");
            d.button(Button::Settings);
            d.tap_cell(given);
            assert_eq!(d.app.ui.highlight, Some(v), "{case}");
        }
    });
}

#[test]
fn a_finger_or_a_hand_keeps_the_highlight() {
    // HLT-7, INP-1, INP-15, INP-20: on an empty cell, on a number, off the board, on the toolbar, a
    // second finger, and a hand touch. The finger removes a "?" and an x (QST-4, MRK-15).
    for_each_layout(|case| {
        let mut d = Driver::new(411);
        let (given, v) = highlight_a_number(&mut d);
        let empty = empty_cell(&d.app.game);
        for (x, y) in [micro_center(empty, 5), micro_center(given, 5), no_control_point(false)] {
            d.send(Input::TouchDown { id: 1, x, y });
            d.send(Input::TouchMove { id: 1, x: x + 90.0, y });
            d.send(Input::TouchUp { id: 1, x: x + 90.0, y });
            assert_eq!(d.app.ui.highlight, Some(v), "HLT-7: ({x}, {y}) ({case})");
        }
        // A second finger.
        d.send(Input::TouchDown { id: 1, x: 300.0, y: 900.0 });
        d.send(Input::TouchDown { id: 2, x: 400.0, y: 900.0 });
        d.send(Input::TouchUp { id: 2, x: 400.0, y: 900.0 });
        d.send(Input::TouchUp { id: 1, x: 300.0, y: 900.0 });
        assert_eq!(d.app.ui.highlight, Some(v), "HLT-7: a second finger ({case})");
        // A hand touch after a press that keeps the highlight.
        let (x, y) = no_control_point(false);
        d.pen_tap(x, y);
        d.hand_tap(300.0, 900.0);
        assert_eq!(d.app.ui.highlight, Some(v), "HLT-7: a hand touch ({case})");
    });
}

#[test]
fn the_menu_the_panels_and_their_options_keep_the_highlight() {
    // HLT-4, HLT-7, NAV-11: a highlight shows with the menu and each panel. Each option keeps it, with
    // the pen and with a finger. The drawing agrees with the model of the colors (BRD-7).
    let fonts = fonts();
    for_each_layout(|case| {
        for pen in [false, true] {
            let case = format!("{case}, pen: {pen}");
            let mut d = Driver::new(412);
            let (_, v) = highlight_a_number(&mut d);
            let tap = |d: &mut Driver, (x, y): (f32, f32)| if pen { d.pen_tap(x, y) } else { d.finger_tap(x, y) };
            let check = |d: &Driver, what: &str| {
                assert_eq!(d.app.ui.highlight, Some(v), "HLT-7: {what} ({case})");
                sudoku::invariants::check_app(&d.app).unwrap_or_else(|e| panic!("{e} ({case})"));
            };
            tap(&mut d, button_center(Button::Menu));
            assert!(d.app.ui.menu.is_open(), "{case}");
            check(&d, "the menu");

            tap(&mut d, button_center(Button::Colors));
            assert_eq!(d.app.ui.menu.panel(), Some(Panel::Colors), "{case}");
            check(&d, "the Cell colors panel");
            tap(&mut d, color_point(Section::Starting, Color::Yellow));
            check(&d, "a color");
            tap(&mut d, line_gray_point(sudoku::settings::LineGray::ALL[0]));
            check(&d, "a line gray");
            assert_board(&d.app, &fonts, &case);

            tap(&mut d, button_center(Button::Settings));
            assert_eq!(d.app.ui.menu.panel(), Some(Panel::Settings), "{case}");
            check(&d, "the Settings panel");

            tap(&mut d, button_center(Button::NewGame));
            assert_eq!(d.app.ui.menu.panel(), Some(Panel::NewGame), "{case}");
            check(&d, "the New game panel");
            let (x, y) = exact_center(layout::ng_slider_row());
            tap(&mut d, (x, y));
            check(&d, "the slider");

            tap(&mut d, button_center(Button::Checkpoints));
            assert_eq!(d.app.ui.menu.panel(), Some(Panel::Checkpoints), "{case}");
            check(&d, "the Checkpoints panel");
            tap(&mut d, save_point());
            assert_eq!(d.app.game.checkpoints.len(), 1, "{case}");
            check(&d, "the save of a checkpoint");
            assert_board(&d.app, &fonts, &case);

            tap(&mut d, button_center(Button::Checkpoints));
            assert_eq!(d.app.ui.menu.panel(), None, "{case}");
            check(&d, "the close of a panel");
            tap(&mut d, button_center(Button::Menu));
            assert!(!d.app.ui.menu.is_open(), "{case}");
            check(&d, "the close of the menu");
        }
    });
}

#[test]
fn a_pen_press_on_the_board_with_a_panel_open_only_closes_the_panel() {
    // NAV-11, HLT-7, INP-22: also if the pen moves, and on an empty cell or on a number. The game and
    // the highlight do not change, no ink shows, and the lift does nothing.
    for_each_layout(|case| {
        let mut d = Driver::new(413);
        let (_, v) = highlight_a_number(&mut d);
        // Cells that the panel does not cover.
        let free = |c: &Cell| {
            let (x, y) = micro_center(*c, 1);
            let (x1, y1) = micro_center(*c, 9);
            !layout::settings_panel(false).contains(x, y) && !layout::settings_panel(false).contains(x1, y1)
        };
        let other = cells().filter(free).find(|&c| d.app.game.is_filled(c) && d.app.game.value_at(c) != v).unwrap();
        let empty = cells().filter(free).find(|&c| !d.app.game.is_filled(c)).unwrap();
        d.button(Button::Menu);
        for (start, moves) in [(empty, false), (empty, true), (other, false), (other, true)] {
            let case = format!("{case}, {start:?}, moves: {moves}");
            d.button(Button::Settings);
            assert_eq!(d.app.ui.menu.panel(), Some(Panel::Settings), "{case}");
            let before = board(&d.app.game);
            let (x, y) = micro_center(start, 1);
            let (x1, y1) = micro_center(start, 9);
            d.send(Input::PenDown { x, y, tool: Tool::Pen });
            assert_eq!(d.app.ui.menu.panel(), None, "NAV-11 ({case})");
            if moves {
                d.send(Input::PenMove { x: x1, y: y1 });
            }
            assert!(d.app.live_stroke.is_none() && d.app.writing_stroke().is_none(), "NAV-11: no ink ({case})");
            d.send(Input::PenUp { x: x1, y: y1 });
            assert_eq!(d.app.ui.highlight, Some(v), "HLT-7 ({case})");
            assert_eq!(board(&d.app.game), before, "NAV-11 ({case})");
            assert!(d.app.ui.pending_ink.is_none() && !d.app.is_writing(), "NAV-11 ({case})");
            assert_eq!(d.app.ui.refused, None, "NAV-11 ({case})");
        }
    });
}

#[test]
fn undo_redo_and_restore_keep_the_highlight_also_when_its_digit_goes() {
    // HLT-7, HLT-8, UND-9, BRD-7: no cell has the digit, the highlight stays, and it shows again when
    // the digit comes back. A tap on that digit then removes it.
    let fonts = fonts();
    for_each_layout(|case| {
        for by in [By::Finger, By::Pen] {
            let case = format!("{case}, {by:?}");
            let mut d = Driver::new(414);
            d.app.game = board_without(7);
            let cell = empty_cell(&d.app.game);
            d.button(Button::Menu);
            open_checkpoints(&mut d);
            tap_by(&mut d, By::Finger, save_point());
            close_checkpoints(&mut d);
            d.write(cell, 7);
            assert_eq!(d.app.game.value_at(cell), 7, "{case}");
            d.tap_cell(cell);
            assert_eq!(d.app.ui.highlight, Some(7), "{case}");

            // Undo removes the last 7.
            tap_by(&mut d, by, button_center(Button::Undo));
            assert_eq!(d.app.game.value_at(cell), 0, "{case}");
            assert_eq!(d.app.ui.highlight, Some(7), "HLT-8: Undo ({case})");
            assert_board(&d.app, &fonts, &case);
            // Redo brings it back, and the cell has the highlight color.
            tap_by(&mut d, by, button_center(Button::Redo));
            assert_eq!(d.app.game.value_at(cell), 7, "{case}");
            assert_eq!(d.app.ui.highlight, Some(7), "HLT-8: Redo ({case})");
            let pm = draw_screen(&d.app, &fonts);
            assert_eq!(cell_background(&pm, cell), d.app.settings.color_highlight.rgb(), "HLT-8: it shows again ({case})");

            // Restore removes it.
            open_checkpoints(&mut d);
            tap_by(&mut d, by, restore_point(0));
            assert_eq!(d.app.game.value_at(cell), 0, "CKP-2 ({case})");
            assert_eq!(d.app.ui.highlight, Some(7), "HLT-8: Restore ({case})");
            close_checkpoints(&mut d);
            assert_eq!(d.app.ui.highlight, Some(7), "HLT-7 ({case})");
            // A number of the same digit comes back with Undo.
            tap_by(&mut d, by, button_center(Button::Undo));
            assert_eq!(d.app.game.value_at(cell), 7, "UND-9 ({case})");
            let pm = draw_screen(&d.app, &fonts);
            assert_eq!(cell_background(&pm, cell), d.app.settings.color_highlight.rgb(), "HLT-8 ({case})");
            d.tap_cell(cell);
            assert_eq!(d.app.ui.highlight, None, "HLT-8: a tap on the digit removes it ({case})");
        }
    });
}

#[test]
fn undo_with_ink_waiting_reads_the_ink_and_that_removes_the_highlight() {
    // HLT-7, INP-21: the last sentence of HLT-7. A finger control reads the ink at its release.
    for_each_layout(|case| {
        let mut d = Driver::new(415);
        d.button(Button::Menu);
        let (cell, _) = new_mark(&mut d);
        let (_, v) = highlight_a_number(&mut d);
        let other = cells().find(|&c| !d.app.game.is_filled(c) && c != cell).unwrap();
        d.write_strokes(&scribble_ink(other));
        let (x, y) = button_center(Button::Undo);
        d.send(Input::TouchDown { id: 1, x, y });
        assert_eq!(d.app.ui.highlight, Some(v), "INP-21 ({case})");
        d.send(Input::TouchUp { id: 1, x, y });
        assert_eq!(d.app.ui.highlight, None, "HLT-7 ({case})");
        assert!(d.app.ui.pending_ink.is_none(), "INP-21 ({case})");
    });
}

#[test]
fn the_eraser_keeps_the_highlight() {
    // HLT-7, INP-8, HLT-8: on an entry (also the last number with the digit), on a starting number, on
    // an empty cell and on waiting ink in its own cell. On waiting ink in a different cell, it reads
    // that ink and the read removes the highlight (INP-12).
    for_each_layout(|case| {
        let mut d = Driver::new(416);
        d.app.game = board_without(7);
        let cell = empty_cell(&d.app.game);
        let empty = cells().find(|&c| !d.app.game.is_filled(c) && c != cell).unwrap();
        d.write(cell, 7);
        d.tap_cell(cell);
        assert_eq!(d.app.ui.highlight, Some(7), "{case}");
        d.erase(given_cell(&d.app.game));
        d.erase(empty);
        assert_eq!(d.app.ui.highlight, Some(7), "HLT-7: no effect ({case})");
        d.erase(cell);
        assert_eq!(d.app.game.value_at(cell), 0, "{case}");
        assert_eq!(d.app.ui.highlight, Some(7), "HLT-8: the eraser removes the last 7 ({case})");

        // On waiting ink in its own cell: the app does not read it.
        d.write_strokes(&scribble_ink(cell));
        assert_eq!(d.app.ui.highlight, Some(7), "{case}");
        d.erase(cell);
        assert!(d.app.ui.pending_ink.is_none() && d.app.ui.question.is_none(), "INP-8 ({case})");
        assert_eq!(d.app.ui.highlight, Some(7), "HLT-7: the eraser on its ink ({case})");

        // On waiting ink in a different cell: the read removes it.
        d.write_strokes(&scribble_ink(cell));
        d.erase(empty);
        assert!(d.app.ui.pending_ink.is_none() && d.app.ui.question.is_some(), "INP-12 ({case})");
        assert_eq!(d.app.ui.highlight, None, "HLT-2: the read ({case})");
    });
}

#[test]
fn a_pen_press_off_the_board_keeps_the_highlight_when_no_ink_waits() {
    // HLT-7, HLT-9, INP-12: menu closed and open. If ink waits, the press reads it (HLT-2, item 4).
    for_each_layout(|case| {
        for menu in [false, true] {
            let case = format!("{case}, menu: {menu}");
            let mut d = Driver::new(417);
            let (_, v) = highlight_a_number(&mut d);
            if menu {
                d.button(Button::Menu);
            }
            let (x, y) = no_control_point(menu);
            d.send(Input::PenDown { x, y, tool: Tool::Pen });
            assert_eq!(d.app.ui.highlight, Some(v), "HLT-9 ({case})");
            d.send(Input::PenMove { x: x + 1.0, y: y + 1.0 });
            d.send(Input::PenUp { x: x + 1.0, y: y + 1.0 });
            assert_eq!(d.app.ui.highlight, Some(v), "HLT-7 ({case})");
            assert_eq!(d.app.ui.menu.is_open(), menu, "{case}");

            d.write_strokes(&scribble_ink(empty_cell(&d.app.game)));
            assert_eq!(d.app.ui.highlight, Some(v), "HLT-9 ({case})");
            d.send(Input::PenDown { x, y, tool: Tool::Pen });
            assert!(d.app.ui.pending_ink.is_none(), "INP-12 ({case})");
            assert_eq!(d.app.ui.highlight, None, "HLT-2 ({case})");
            d.send(Input::PenUp { x, y });
        }
    });
}

#[test]
fn a_turn_and_a_suspension_remove_the_ink_and_keep_the_highlight() {
    // HLT-7, NAV-13, PER-6.
    let stops = sudoku::render::ng_stops(&fonts());
    for suspend in [false, true] {
        let mut d = Driver::new(418);
        let (_, v) = highlight_a_number(&mut d);
        let cell = empty_cell(&d.app.game);
        d.write_strokes(&scribble_ink(cell));
        assert_eq!(d.app.ui.highlight, Some(v));
        if suspend {
            d.app.suspend();
        } else {
            layout::set_orientation(layout::Orientation::Landscape);
            d.app.rotate(stops);
        }
        assert!(d.app.ui.pending_ink.is_none(), "NAV-13, PER-6: suspend: {suspend}");
        assert_eq!(d.app.ui.highlight, Some(v), "HLT-7: suspend: {suspend}");
        assert_eq!(d.app.ui.question, None, "the app did not read the ink: suspend: {suspend}");
        layout::set_orientation(layout::Orientation::Portrait);
    }
}

// ---- One redraw for each action (HLT-9). The test counts the steps of the main loop that change the
// framebuffer, with the same output code as the device.

struct Shown {
    d: Driver,
    p: sudoku::present::Presenter,
    fonts: sudoku::font::Fonts,
    fb: Vec<u8>,
    last: Vec<u8>,
    changes: usize,
}

impl Shown {
    fn new(seed: u64) -> Shown {
        let size = layout::physical_w() as usize * layout::physical_h() as usize * 3;
        let mut p = sudoku::present::Presenter::new();
        p.use_pen_region = true;
        let mut s = Shown { d: Driver::new(seed), p, fonts: fonts(), fb: vec![0; size], last: vec![0; size], changes: 0 };
        s.pass(120);
        s
    }

    fn show(&mut self) {
        self.p.present(&mut self.d.app, &self.fonts, &mut self.fb, self.d.now, |_| {});
        if self.fb != self.last {
            self.changes += 1;
            self.last.clone_from(&self.fb);
        }
    }

    fn send(&mut self, i: Input) {
        self.d.send(i);
        self.show();
    }

    /// The main loop runs for `ms`, with steps of 15 ms.
    fn pass(&mut self, ms: u64) {
        let end = self.d.now + ms;
        while self.d.now < end {
            self.d.now = (self.d.now + 15).min(end);
            self.d.app.tick(self.d.now);
            self.show();
        }
    }

    fn pen_tap(&mut self, (x, y): (f32, f32)) {
        self.send(Input::PenDown { x, y, tool: Tool::Pen });
        self.send(Input::PenUp { x, y });
    }

    /// The color of a point of the background of a cell in the framebuffer.
    fn background(&self, cell: Cell) -> [u8; 3] {
        let (x, y) = layout::cell_origin(cell);
        let (px, py, _, _) = layout::physical_rect((x + 10, y + 135, x + 10, y + 135));
        let i = (py as usize * layout::physical_w() as usize + px as usize) * 3;
        [self.fb[i], self.fb[i + 1], self.fb[i + 2]]
    }

    /// The framebuffer shows all of the state of the app (no old pixels and no part of a change).
    fn assert_complete(&self, case: &str) {
        let mut fresh = vec![0; self.fb.len()];
        sudoku::present::Presenter::new().draw_all(&self.d.app, &self.fonts, &mut fresh);
        assert!(fresh == self.fb, "the framebuffer does not show the state ({case})");
    }
}

#[test]
fn a_tap_changes_the_highlight_in_one_redraw() {
    // HLT-2, HLT-3, HLT-9: the toggle, the move, a mark and an x. Nothing changes at the press. All
    // layouts.
    for_each_layout(|case| {
        let mut s = Shown::new(420);
        let given = given_cell(&s.d.app.game);
        let v = s.d.app.game.value_at(given);
        let (other, w) = other_number(&s.d, v);
        let (mark_cell, mark) = cells()
            .filter(|&c| !s.d.app.game.is_filled(c))
            .find_map(|c| (1..=9u8).find(|&m| !peer_has(&s.d.app.game, c, m)).map(|m| (c, m)))
            .unwrap();
        let (x_cell, x_digit) = refused_pair(&s.d.app.game);
        let plain = s.background(given);

        // (what, the cell of the tap, the micro-cell of the tap, the highlight after the tap)
        let steps: Vec<(&str, Cell, u8, Option<u8>)> = vec![
            ("a new highlight", given, 5, Some(v)),
            ("the move", other, 5, Some(w)),
            ("the toggle", other, 5, None),
            ("a new highlight", given, 5, Some(v)),
            ("a mark", mark_cell, mark, None),
            ("a new highlight", given, 5, Some(v)),
            ("an x", x_cell, x_digit, None),
        ];
        for (what, cell, digit, want) in steps {
            let case = format!("{case}, {what}");
            s.pass(120);
            s.changes = 0;
            let (x, y) = micro_center(cell, digit);
            s.send(Input::PenDown { x, y, tool: Tool::Pen });
            s.pass(60);
            assert_eq!(s.changes, 0, "HLT-9: no change at the press ({case})");
            s.send(Input::PenUp { x, y });
            s.pass(120);
            assert_eq!(s.d.app.ui.highlight, want, "{case}");
            assert_eq!(s.changes, 1, "HLT-9: one redraw ({case})");
            s.assert_complete(&case);
            if want.is_none() {
                assert_eq!(s.background(given), plain, "the highlight went ({case})");
            }
        }
    });
}

#[test]
fn writing_changes_the_highlight_in_one_redraw_with_the_number() {
    // HLT-2 (item 4), HLT-9, INP-5: the highlight shows during the writing and until the read. Then
    // the number and the removal go in one redraw. All layouts, and also with a "?".
    for_each_layout(|case| {
        for number in [true, false] {
            let case = format!("{case}, number: {number}");
            let mut s = Shown::new(421);
            let given = given_cell(&s.d.app.game);
            let plain = s.background(given);
            s.pen_tap(micro_center(given, 5));
            s.pass(120);
            let lit = s.background(given);
            assert_ne!(lit, plain, "{case}");
            let cell = empty_cell(&s.d.app.game);
            let ink = if number { digit_ink(cell, 4) } else { scribble_ink(cell) };
            for stroke in &ink {
                s.send(Input::PenDown { x: stroke[0].0, y: stroke[0].1, tool: Tool::Pen });
                assert_eq!(s.background(given), lit, "HLT-9: the press ({case})");
                for p in &stroke[1..] {
                    s.send(Input::PenMove { x: p.0, y: p.1 });
                }
                let last = stroke[stroke.len() - 1];
                s.send(Input::PenUp { x: last.0, y: last.1 });
                assert_eq!(s.background(given), lit, "HLT-9: the lift ({case})");
            }
            let lift = s.d.now;
            s.pass(RECOGNITION_PAUSE_MS - 30);
            assert_eq!(s.background(given), lit, "HLT-9: during the pause ({case})");
            assert!(s.d.app.ui.pending_ink.is_some(), "{case}");
            s.changes = 0;
            let until = lift + RECOGNITION_PAUSE_MS + 300;
            s.pass(until - s.d.now);
            assert!(s.d.app.ui.pending_ink.is_none(), "{case}");
            assert_eq!(s.changes, 1, "HLT-9: one redraw with the result ({case})");
            assert_eq!(s.background(given), plain, "{case}");
            s.assert_complete(&case);
        }
    });
}

#[test]
fn a_tap_on_a_number_with_ink_elsewhere_goes_in_one_redraw() {
    // HLT-9: the read at the press and the result of the lift go in one redraw, also with a pen area
    // that stops. A tap on v gives no highlight, and a tap on w gives w with the "?".
    for_each_layout(|case| {
        let mut s = Shown::new(422);
        let given = given_cell(&s.d.app.game);
        let v = s.d.app.game.value_at(given);
        let (other, w) = other_number(&s.d, v);
        for (target, want) in [(given, None), (other, Some(w))] {
            let case = format!("{case}, {target:?}");
            s.pen_tap(micro_center(given, 5));
            s.pass(120);
            let cell = cells().find(|&c| !s.d.app.game.is_filled(c) && s.d.app.ui.question.is_none_or(|q| q.0 != c)).unwrap();
            for stroke in &scribble_ink(cell) {
                s.send(Input::PenDown { x: stroke[0].0, y: stroke[0].1, tool: Tool::Pen });
                for p in &stroke[1..] {
                    s.send(Input::PenMove { x: p.0, y: p.1 });
                }
                let last = stroke[stroke.len() - 1];
                s.send(Input::PenUp { x: last.0, y: last.1 });
            }
            s.pass(150);
            s.changes = 0;
            let (x, y) = micro_center(target, 5);
            s.send(Input::PenDown { x, y, tool: Tool::Pen });
            s.pass(90);
            assert_eq!(s.changes, 0, "HLT-9: the press waits for the lift ({case})");
            assert!(s.d.app.ui.question.is_some(), "INP-12: the state changed at the press ({case})");
            s.send(Input::PenUp { x, y });
            s.pass(120);
            assert_eq!(s.d.app.ui.highlight, want, "HLT-9 ({case})");
            assert_eq!(s.changes, 1, "HLT-9: one redraw ({case})");
            s.assert_complete(&case);
            s.pass(OVERLAY_MS);
            if want.is_some() {
                s.pen_tap(micro_center(other, 5));
                s.pass(120);
            }
        }
    });
}

#[test]
fn a_press_on_a_control_while_ink_waits_is_one_of_the_two_accepted_cases() {
    // HLT-9: a pen press on a control while ink waits: the number at the press, the action at the
    // release. Two updates, and the first one shows the number and not the action.
    for_each_layout(|case| {
        let mut s = Shown::new(423);
        let cell = empty_cell(&s.d.app.game);
        let given = given_cell(&s.d.app.game);
        s.pen_tap(micro_center(given, 5));
        s.pass(120);
        for stroke in &digit_ink(cell, 4) {
            s.send(Input::PenDown { x: stroke[0].0, y: stroke[0].1, tool: Tool::Pen });
            for p in &stroke[1..] {
                s.send(Input::PenMove { x: p.0, y: p.1 });
            }
            let last = stroke[stroke.len() - 1];
            s.send(Input::PenUp { x: last.0, y: last.1 });
        }
        s.pass(150);
        s.changes = 0;
        let (x, y) = button_center(Button::Menu);
        s.send(Input::PenDown { x, y, tool: Tool::Pen });
        s.pass(90);
        assert_eq!(s.d.app.game.value_at(cell), 4, "{case}");
        assert_eq!(s.changes, 1, "HLT-9: the number at the press ({case})");
        s.assert_complete(&case);
        s.send(Input::PenUp { x, y });
        s.pass(90);
        assert!(s.d.app.ui.menu.is_open(), "{case}");
        assert_eq!(s.changes, 2, "HLT-9: the action at the release ({case})");
        s.assert_complete(&case);
    });
}

// ---- INP-20: a hand touch.

/// All that a control can change.
fn control_state(d: &Driver) -> (Screen, Menu, bool, sudoku::game::Game) {
    (d.app.screen, d.app.ui.menu, d.app.quit, play_state(&d.app.game))
}

#[test]
fn a_hand_touch_operates_no_toolbar_button_and_a_touch_at_150_ms_does() {
    // INP-15, INP-20: on each button of the toolbar. The limit: 149 ms and 150 ms after the pen lift.
    // The move and the release of a hand touch also have no effect. All layouts.
    assert_eq!(HAND_TOUCH_MS, 150, "INP-20");
    for_each_layout(|case| {
        for b in [Button::Menu, Button::Colors, Button::Settings, Button::Undo, Button::Redo, Button::Checkpoints, Button::Help, Button::Statistics, Button::NewGame, Button::Close] {
            let case = format!("{case}, {b:?}");
            let mut d = Driver::new(430);
            fill_stacks(&mut d);
            let (x, y) = button_center(b);
            let (px, py) = no_control_point(true);
            for ms in [0, 97, 149] {
                d.pen_tap(px, py);
                let before = control_state(&d);
                d.touch_down_after_lift(ms, x, y);
                d.send_at_once(Input::TouchMove { id: 1, x: x + 1.0, y });
                assert_eq!(control_state(&d), before, "INP-20: press at {ms} ms ({case})");
                d.send_at_once(Input::TouchUp { id: 1, x, y });
                assert_eq!(control_state(&d), before, "INP-20: release at {ms} ms ({case})");
            }
            d.pen_tap(px, py);
            let before = control_state(&d);
            d.touch_down_after_lift(150, x, y);
            d.send_at_once(Input::TouchUp { id: 1, x, y });
            if b == Button::Close {
                assert_eq!(control_state(&d), before, "NAV-6: a finger does not close the app ({case})");
            } else {
                assert_ne!(control_state(&d), before, "INP-20: a touch at 150 ms operates ({case})");
            }
        }
    });
}

#[test]
fn the_lift_of_each_tool_starts_the_time_of_a_hand_touch() {
    // INP-20: the eraser is a pen on the other end. A pen lift off the board also starts the time. The
    // time starts again at each pen lift, thus the second lift counts and the first one does not.
    let mut d = Driver::new(431);
    let (x, y) = button_center(Button::Menu);
    d.send(Input::PenDown { x: 10.0, y: 1000.0, tool: Tool::Eraser });
    d.send(Input::PenUp { x: 10.0, y: 1000.0 });
    d.touch_down_after_lift(100, x, y);
    d.send_at_once(Input::TouchUp { id: 1, x, y });
    assert_eq!(d.app.ui.menu, Menu::Closed, "INP-20: the eraser");

    d.wait(1000);
    let (px, py) = no_control_point(false);
    d.pen_tap(px, py);
    d.wait(100);
    d.pen_tap(px, py);
    d.touch_down_after_lift(100, x, y);
    d.send_at_once(Input::TouchUp { id: 1, x, y });
    assert_eq!(d.app.ui.menu, Menu::Closed, "INP-20: the time of the last lift");

    // A touch with no pen lift before it operates: the time of the start of the app does not count.
    let mut d = Driver::new(431);
    d.now = 0;
    d.send_at_once(Input::TouchDown { id: 1, x, y });
    d.send_at_once(Input::TouchUp { id: 1, x, y });
    assert!(d.app.ui.menu.is_open(), "INP-20: no pen lift");
}

#[test]
fn a_hand_touch_does_not_remove_an_x_or_a_question_mark_and_a_touch_at_150_ms_does() {
    // INP-20, QST-4, MRK-15: on the board, off the board and on a control. All layouts.
    for_each_layout(|case| {
        for at in [micro_center((4, 4), 5), no_control_point(false), button_center(Button::Menu)] {
            let case = format!("{case}, {at:?}");
            let (x, y) = at;
            // The x.
            let mut d = Driver::new(432);
            let (cell, digit) = refused_tap(&mut d);
            d.touch_down_after_lift(149, x, y);
            assert!(d.app.ui.refused.is_some(), "MRK-15: a hand touch, press ({case})");
            d.send_at_once(Input::TouchMove { id: 1, x: x + 1.0, y });
            d.send_at_once(Input::TouchUp { id: 1, x, y });
            assert!(d.app.ui.refused.is_some(), "MRK-15: a hand touch, release ({case})");
            assert_eq!(d.app.ui.menu, Menu::Closed, "INP-20 ({case})");
            d.tap_micro(cell, digit);
            d.touch_down_after_lift(150, x, y);
            assert_eq!(d.app.ui.refused, None, "MRK-15: a touch at 150 ms ({case})");
            d.send_at_once(Input::TouchUp { id: 1, x, y });

            // The "?". The press of the pen on a number reads the ink, thus the "?" shows just
            // before the pen lift.
            let mut d = Driver::new(432);
            let ink_cell = cells().find(|&c| !d.app.game.is_filled(c) && c != (4, 4)).unwrap();
            let given = given_cell(&d.app.game);
            d.write_strokes(&scribble_ink(ink_cell));
            d.tap_cell(given);
            assert!(d.app.ui.question.is_some(), "{case}");
            d.touch_down_after_lift(149, x, y);
            assert!(d.app.ui.question.is_some(), "QST-4: a hand touch, press ({case})");
            d.send_at_once(Input::TouchUp { id: 1, x, y });
            assert!(d.app.ui.question.is_some(), "QST-4: a hand touch, release ({case})");
            d.write_strokes(&scribble_ink(ink_cell));
            d.tap_cell(given);
            assert!(d.app.ui.question.is_some(), "{case}");
            d.touch_down_after_lift(150, x, y);
            assert_eq!(d.app.ui.question, None, "QST-4: a touch at 150 ms ({case})");
        }
    });
}

#[test]
fn a_hand_touch_as_a_second_touch_has_no_effect() {
    // INP-20, INP-15: a finger is on a control and the pen does a refused tap. A hand touch does not
    // remove the x, and a second finger at 150 ms does. The first finger still operates its control.
    let mut d = Driver::new(433);
    let (bx, by) = hold(&mut d, Button::Menu);
    refused_tap(&mut d);
    d.send_at_once(Input::TouchDown { id: 7, x: 800.0, y: 900.0 });
    assert!(d.app.ui.refused.is_some(), "INP-20");
    d.send_at_once(Input::TouchUp { id: 7, x: 800.0, y: 900.0 });
    assert!(d.app.ui.refused.is_some(), "INP-20");
    d.wait(HAND_TOUCH_MS - 30);
    d.send(Input::TouchMove { id: 1, x: bx, y: by });
    d.send(Input::TouchDown { id: 7, x: 800.0, y: 900.0 });
    assert_eq!(d.app.ui.refused, None, "MRK-15, INP-15: a second finger at 150 ms");
    d.send(Input::TouchUp { id: 7, x: 800.0, y: 900.0 });
    d.send(Input::TouchUp { id: 1, x: bx, y: by });
    assert!(d.app.ui.menu.is_open(), "the first finger operates");
}

#[test]
fn a_hand_touch_operates_nothing_on_a_panel_a_dialog_or_a_page() {
    // INP-20, INP-15, UPD-10: an option of a panel, the buttons of the dialog, "< Back" and a swipe in
    // the manual. At 150 ms, each of these operates.
    for_each_layout(|case| {
        // A panel.
        let mut d = Driver::new(434);
        d.button(Button::Menu);
        d.button(Button::Settings);
        let (x, y) = switch_point(Switch::ColorCompletedGroups);
        let on = d.app.settings.completed_color_on;
        // A pen tap outside closes the panel: thus the pen tap is on the row of the version.
        let (vx, vy) = exact_center(layout::settings_row(3));
        d.pen_tap(vx, vy);
        assert_eq!(d.app.ui.menu.panel(), Some(Panel::Settings), "{case}");
        d.touch_down_after_lift(149, x, y);
        d.send_at_once(Input::TouchUp { id: 1, x, y });
        assert_eq!(d.app.settings.completed_color_on, on, "INP-20: a switch ({case})");
        d.pen_tap(vx, vy);
        d.touch_down_after_lift(150, x, y);
        d.send_at_once(Input::TouchUp { id: 1, x, y });
        assert_eq!(d.app.settings.completed_color_on, !on, "INP-20: a switch at 150 ms ({case})");
        // A hand touch outside the panel does not close it.
        d.pen_tap(vx, vy);
        d.touch_down_after_lift(149, 400.0, 1000.0);
        d.send_at_once(Input::TouchUp { id: 1, x: 400.0, y: 1000.0 });
        assert_eq!(d.app.ui.menu.panel(), Some(Panel::Settings), "INP-20: outside the panel ({case})");

        // The dialog.
        let mut d = Driver::new(435);
        d.app.update = reference::update_state(true);
        d.app.update.tap();
        d.app.update.request = None;
        let now = d.now;
        d.app.update_installed(Ok(true), now);
        let (x, y) = exact_center(layout::dialog_cancel());
        d.pen_tap(5.0, 5.0);
        assert!(d.app.update.dialog_shown(), "{case}");
        d.touch_down_after_lift(149, x, y);
        d.send_at_once(Input::TouchUp { id: 1, x, y });
        assert!(d.app.update.dialog_shown(), "INP-20: \"Later\" ({case})");
        d.pen_tap(5.0, 5.0);
        d.touch_down_after_lift(150, x, y);
        d.send_at_once(Input::TouchUp { id: 1, x, y });
        assert!(!d.app.update.dialog_shown(), "INP-20: \"Later\" at 150 ms ({case})");

        // The manual.
        let mut d = Driver::new(436);
        d.button(Button::Menu);
        d.button(Button::Help);
        assert_eq!(d.app.screen, Screen::Manual, "{case}");
        let (bx, by) = back_center();
        let (sx, sy) = (layout::screen_w() as f32 / 2.0, layout::screen_h() as f32 / 2.0);
        d.pen_tap(sx, sy);
        d.touch_down_after_lift(149, bx, by);
        d.send_at_once(Input::TouchUp { id: 1, x: bx, y: by });
        assert_eq!(d.app.screen, Screen::Manual, "INP-20: \"< Back\" ({case})");
        d.pen_tap(sx, sy);
        d.touch_down_after_lift(149, sx + 300.0, sy);
        d.send_at_once(Input::TouchMove { id: 1, x: sx, y: sy });
        d.send_at_once(Input::TouchUp { id: 1, x: sx - 300.0, y: sy });
        assert_eq!(d.app.manual_page, 1, "INP-20: a swipe ({case})");
        d.pen_tap(sx, sy);
        d.touch_down_after_lift(150, sx + 300.0, sy);
        d.send_at_once(Input::TouchUp { id: 1, x: sx - 300.0, y: sy });
        assert_eq!(d.app.manual_page, 2, "INP-20: a swipe at 150 ms ({case})");
    });
}

// ---- INP-21: a finger and waiting ink.

#[test]
fn a_finger_press_does_not_read_ink_or_stop_the_writing_and_a_control_does_at_its_release() {
    // INP-1, INP-12, INP-16, INP-21: on the board, off the board and on a control. A release off the
    // control does nothing. All layouts.
    for_each_layout(|case| {
        let on_board = micro_center((4, 4), 5);
        for (what, (x, y)) in [("the board", on_board), ("no control", no_control_point(false)), ("a control", button_center(Button::Menu))] {
            let case = format!("{case}, {what}");
            let mut d = Driver::new(440);
            let cell = cells().find(|&c| !d.app.game.is_filled(c) && c != (4, 4)).unwrap();
            d.write_strokes(&digit_ink(cell, 4));
            let lift = d.now;
            let before = board(&d.app.game);
            // The press is a real touch, and it stays down.
            d.send(Input::TouchDown { id: 1, x, y });
            assert!(d.now >= lift + HAND_TOUCH_MS, "{case}");
            assert!(d.app.ui.pending_ink.is_some(), "INP-21: no read at the press ({case})");
            assert!(d.app.is_writing(), "INP-21: the writing does not stop ({case})");
            assert_eq!(play_state(&d.app.game), play_state(&before), "{case}");
            // A release far from the press: on no control (off the control).
            let (ox, oy) = micro_center((8, 8), 5);
            d.send(Input::TouchMove { id: 1, x: ox, y: oy });
            d.send(Input::TouchUp { id: 1, x: ox, y: oy });
            assert!(d.app.ui.pending_ink.is_some(), "INP-21: a release off the control ({case})");
            assert!(d.app.is_writing(), "INP-21: a release off the control ({case})");
            assert_eq!(d.app.ui.menu, Menu::Closed, "{case}");
            // The pause does not change, and the app reads the ink at its time.
            tick_to(&mut d, lift + RECOGNITION_PAUSE_MS - 1);
            assert!(d.app.ui.pending_ink.is_some(), "INP-5 ({case})");
            tick_to(&mut d, lift + RECOGNITION_PAUSE_MS);
            assert_eq!(d.app.game.value_at(cell), 4, "INP-5 ({case})");
        }
    });
}

#[test]
fn a_finger_control_reads_the_ink_and_stops_the_writing_at_its_release() {
    // INP-12, INP-16, INP-21, HLT-2: the ink and the timer with no ink. A pen press does this at the press.
    for_each_layout(|case| {
        let (x, y) = button_center(Button::Menu);
        // The ink.
        let mut d = Driver::new(441);
        let cell = empty_cell(&d.app.game);
        d.write_strokes(&digit_ink(cell, 4));
        d.send(Input::TouchDown { id: 1, x, y });
        assert!(d.app.is_writing() && d.app.ui.pending_ink.is_some(), "INP-21 ({case})");
        d.send(Input::TouchUp { id: 1, x, y });
        assert!(d.app.ui.pending_ink.is_none(), "INP-21 ({case})");
        assert_eq!(d.app.game.value_at(cell), 4, "INP-21 ({case})");
        assert!(!d.app.is_writing(), "INP-21: the timer stops ({case})");
        assert!(d.app.ui.menu.is_open(), "{case}");

        // The timer with no ink: a stroke from a filled cell started it.
        let mut d = Driver::new(442);
        let given = given_cell(&d.app.game);
        let (x1, y1) = stroke_down(&mut d, given);
        d.send(Input::PenUp { x: x1, y: y1 });
        assert!(d.app.is_writing(), "INP-16 ({case})");
        d.send(Input::TouchDown { id: 1, x, y });
        assert!(d.app.is_writing(), "INP-21: a finger press does not stop it ({case})");
        d.send(Input::TouchUp { id: 1, x, y });
        assert!(!d.app.is_writing(), "INP-21: the release stops it ({case})");

        // A pen press stops it at the press, also off the board.
        let mut d = Driver::new(443);
        let given = given_cell(&d.app.game);
        let (x1, y1) = stroke_down(&mut d, given);
        d.send(Input::PenUp { x: x1, y: y1 });
        assert!(d.app.is_writing(), "INP-16 ({case})");
        let (px, py) = no_control_point(false);
        d.send(Input::PenDown { x: px, y: py, tool: Tool::Pen });
        assert!(!d.app.is_writing(), "INP-16, INP-12: a pen press ({case})");
        d.send(Input::PenUp { x: px, y: py });
    });
}

#[test]
fn the_release_of_a_control_removes_the_x_and_keeps_the_highlight() {
    // MRK-15, HLT-7: a finger stays on a control. The release of a control removes an x, and it does not
    // remove the highlight. A pen press removes the x before the pen makes a highlight (HLT-3).
    let mut d = Driver::new(444);
    let (given, v) = highlight_a_number(&mut d);
    let (x, y) = hold(&mut d, Button::Menu);
    refused_tap(&mut d);
    assert_eq!(d.app.ui.highlight, None, "HLT-2: the refused tap");
    d.tap_cell(given);
    assert_eq!(d.app.ui.highlight, Some(v));
    assert_eq!(d.app.ui.refused, None, "MRK-15: the pen press");
    d.send(Input::TouchUp { id: 1, x, y });
    assert_eq!(d.app.ui.highlight, Some(v), "HLT-7");
    assert!(d.app.ui.menu.is_open());

    // The "?" and a highlight with a finger on a control: the release does not remove a "?" in a
    // different cell, and keeps the highlight.
    let mut d = Driver::new(445);
    let (given, v) = highlight_a_number(&mut d);
    let cell = empty_cell(&d.app.game);
    let (x, y) = hold(&mut d, Button::Menu);
    d.write_strokes(&scribble_ink(cell));
    d.tap_cell(given);
    assert_eq!(d.app.ui.highlight, None, "HLT-9: the tap on v");
    d.tap_cell(given);
    assert_eq!(d.app.ui.highlight, Some(v), "HLT-3");
    assert!(d.app.ui.question.is_none(), "QST-4: the pen press of the second tap");
    d.send(Input::TouchUp { id: 1, x, y });
    assert_eq!(d.app.ui.highlight, Some(v), "HLT-7");
}

// ---- INP-5, INP-16, INP-23: the pause.

#[test]
fn the_pause_is_1_s_from_the_pen_lift_and_a_new_stroke_stops_it() {
    // INP-2, INP-5, INP-16, INP-23: a stroke that takes longer than the pause. The app reads at exactly
    // 1000 ms after the lift of the last stroke, not before. A new stroke stops the pause, and the
    // pause starts again at its lift.
    assert_eq!(RECOGNITION_PAUSE_MS, 1000, "INP-23");
    assert_eq!(WRITING_IDLE_MS, RECOGNITION_PAUSE_MS, "INP-16");
    for_each_layout(|case| {
        let mut d = Driver::new(450);
        assert_eq!(d.app.pause_ms, 1000, "INP-23 ({case})");
        let cell = empty_cell(&d.app.game);
        let ink = digit_ink(cell, 4);
        assert!(ink.len() >= 2, "a 4 with two strokes");
        // The first stroke: 3 s with the pen on the screen.
        let first = &ink[0];
        d.send(Input::PenDown { x: first[0].0, y: first[0].1, tool: Tool::Pen });
        for p in &first[1..] {
            d.send(Input::PenMove { x: p.0, y: p.1 });
        }
        let t = d.now + 3000;
        tick_to(&mut d, t);
        assert_eq!(d.app.game.value_at(cell), 0, "INP-5: no read while the pen is down ({case})");
        assert!(d.app.ui.pending_ink.is_some() && d.app.is_writing(), "INP-5 ({case})");
        let last = first[first.len() - 1];
        d.send(Input::PenUp { x: last.0, y: last.1 });
        let lift = d.now;
        assert_eq!(d.app.next_deadline(), Some(lift + 1000), "INP-5: the timer starts at the lift ({case})");
        tick_to(&mut d, lift + 999);
        assert!(d.app.ui.pending_ink.is_some(), "INP-5: 999 ms ({case})");

        // The second stroke at 999 ms: 3 s with the pen on the screen.
        let second = &ink[1];
        d.app.handle(Input::PenDown { x: second[0].0, y: second[0].1, tool: Tool::Pen }, d.now);
        tick_to(&mut d, lift + 4000);
        assert!(d.app.ui.pending_ink.is_some(), "INP-5: the new stroke stops the pause ({case})");
        for p in &second[1..] {
            d.send(Input::PenMove { x: p.0, y: p.1 });
        }
        let last = second[second.len() - 1];
        d.send(Input::PenUp { x: last.0, y: last.1 });
        let lift = d.now;
        tick_to(&mut d, lift + 999);
        assert!(d.app.ui.pending_ink.is_some(), "INP-5: the pause starts again ({case})");
        assert!(d.app.is_writing(), "INP-16 ({case})");
        tick_to(&mut d, lift + 1000);
        assert!(d.app.ui.pending_ink.is_none(), "INP-5: 1000 ms ({case})");
        assert_eq!(d.app.game.value_at(cell), 4, "INP-5: all the ink is one item ({case})");
        assert!(!d.app.is_writing(), "INP-16: the timer ends with the pause ({case})");
    });
}

#[test]
fn the_pause_has_one_source_and_the_manifest_does_not_set_it() {
    // INP-23: the manifest has no pause setting and no environment block. The source reads no
    // environment variable for the pause.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(root.join("appload/sudoku/external.manifest.json")).unwrap();
    let manifest: serde_json::Value = serde_json::from_str(&text).unwrap();
    let object = manifest.as_object().expect("the manifest is an object");
    assert_eq!(object.get("application").and_then(|v| v.as_str()), Some("sudoku"), "INP-23: this is the manifest");
    assert!(!object.contains_key("environment"), "INP-23: an environment block");
    assert!(!text.to_uppercase().contains("PAUSE"), "INP-23: a pause in the manifest");

    let name = concat!("SUDOKU_", "RECOGNITION_", "PAUSE");
    let mut files: Vec<std::path::PathBuf> = Vec::new();
    let mut dirs = vec![root.join("src"), root.join("appload")];
    while let Some(dir) = dirs.pop() {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let path = e.path();
            if path.is_dir() {
                dirs.push(path);
            } else if path.extension().is_some_and(|x| x == "rs" || x == "json" || x == "qmd") {
                files.push(path);
            }
        }
    }
    for e in std::fs::read_dir(root).unwrap().flatten() {
        if e.file_name().to_string_lossy().starts_with("Dockerfile") {
            files.push(e.path());
        }
    }
    assert!(files.len() > 20, "{} files", files.len());
    for f in &files {
        let text = std::fs::read_to_string(f).unwrap();
        assert!(!text.contains(name), "INP-23: {f:?} has the old environment variable");
        for line in text.lines() {
            let lower = line.to_lowercase();
            let reads_env = line.contains("env::var") || line.contains("test_setting(") || line.contains("test_switch(") || line.contains("ENV ") || line.contains("\"environment\"");
            assert!(!(reads_env && lower.contains("pause")), "INP-23: {f:?}: {line}");
        }
    }
}

// ---- INP-22: ink on a filled cell.

/// The quantity of pixels in the inside of a cell that are not the same in two drawings.
fn changed_in_cell(a: &tiny_skia::Pixmap, b: &tiny_skia::Pixmap, cell: Cell) -> usize {
    let (x0, y0) = layout::cell_origin(cell);
    let mut n = 0;
    for y in y0 + 4..y0 + layout::cell_size() - 4 {
        for x in x0 + 4..x0 + layout::cell_size() - 4 {
            n += usize::from(rgb_at(a, x, y) != rgb_at(b, x, y));
        }
    }
    n
}

#[test]
fn a_touch_on_a_filled_cell_shows_no_ink() {
    // INP-22, INP-7, INP-4: a tap on each micro-cell and a stroke, on a starting number and on an entry.
    // No live stroke, no pen area and no change of the pixels. All layouts.
    let fonts = fonts();
    for_each_layout(|case| {
        let mut d = Driver::new(460);
        let entry = empty_cell(&d.app.game);
        let value = d.app.game.solution[entry.0 as usize][entry.1 as usize];
        d.write(entry, value);
        assert_eq!(d.app.game.value_at(entry), value, "{case}");
        let given = given_cell(&d.app.game);
        for cell in [given, entry] {
            let case = format!("{case}, cell {cell:?}");
            let count = d.app.live_stroke_count;
            let before = draw_screen(&d.app, &fonts);
            for digit in [1, 5, 9] {
                let (x, y) = micro_center(cell, digit);
                d.send(Input::PenDown { x, y, tool: Tool::Pen });
                assert!(d.app.live_stroke.is_none(), "INP-22: a tap, micro-cell {digit} ({case})");
                assert!(d.app.writing_stroke().is_none(), "INP-22: no pen area ({case})");
                assert!(d.app.ui.pending_ink.is_none(), "INP-22 ({case})");
                d.send(Input::PenUp { x, y });
                assert!(d.app.live_stroke.is_none() && d.app.ui.pending_ink.is_none(), "INP-22 ({case})");
                d.app.ui.highlight = None;
            }
            // A stroke, also while the player writes (INP-24 does not apply to a filled cell).
            for _ in 0..2 {
                let (x, y) = stroke_down(&mut d, cell);
                assert!(d.app.live_stroke.is_none(), "INP-22: a stroke ({case})");
                assert!(d.app.writing_stroke().is_none(), "INP-22: no pen area ({case})");
                assert!(d.app.ui.pending_ink.is_none(), "INP-22 ({case})");
                assert_eq!(changed_in_cell(&draw_screen(&d.app, &fonts), &before, cell), 0, "INP-22: pixels ({case})");
                d.send(Input::PenUp { x, y });
                assert!(d.app.live_stroke.is_none() && d.app.ui.pending_ink.is_none(), "INP-22 ({case})");
            }
            assert_eq!(d.app.live_stroke_count, count, "INP-22: no live stroke ({case})");
            d.wait(d.app.pause_ms);
        }
    });
}

#[test]
fn a_stroke_from_an_empty_cell_over_a_filled_cell_shows_ink() {
    // INP-22, INP-12: the ink belongs to the cell where the stroke started. All layouts.
    let fonts = fonts();
    for_each_layout(|case| {
        let mut d = Driver::new(461);
        let g = d.app.game.clone();
        let (from, over) = cells()
            .filter(|&c| !g.is_filled(c))
            .find_map(|c| {
                let right = (c.0, c.1 + 1);
                (c.1 < 8 && g.is_filled(right)).then_some((c, right))
            })
            .unwrap();
        let before = draw_screen(&d.app, &fonts);
        let (x0, y0) = micro_center(from, 5);
        let (x1, y1) = micro_center(over, 5);
        d.send(Input::PenDown { x: x0, y: y0, tool: Tool::Pen });
        assert_eq!(d.app.pen_cell(), Some(from), "INP-24: the pen area from the pen down ({case})");
        for k in 1..=10 {
            let t = k as f32 / 10.0;
            d.send(Input::PenMove { x: x0 + (x1 - x0) * t, y: y0 + (y1 - y0) * t });
        }
        assert!(d.app.writing_stroke().is_some(), "INP-22: the pen area ({case})");
        assert!(changed_in_cell(&draw_screen(&d.app, &fonts), &before, over) > 0, "INP-22: ink on the filled cell, live ({case})");
        d.send(Input::PenUp { x: x1, y: y1 });
        assert_eq!(d.app.ui.pending_ink.as_ref().map(|i| i.cell), Some(from), "INP-12 ({case})");
        assert!(changed_in_cell(&draw_screen(&d.app, &fonts), &before, over) > 0, "INP-22: ink on the filled cell, waiting ({case})");
    });
}

#[test]
fn the_corpus_log_gets_the_strokes_that_do_not_show() {
    // INP-22: the log gets a tap and a stroke on a filled cell, also when no ink shows.
    use std::{cell::RefCell, rc::Rc};
    use sudoku::app::BoardInput;
    let mut d = Driver::new(462);
    let log: Rc<RefCell<Vec<(Cell, usize)>>> = Rc::default();
    let sink = log.clone();
    d.app.stroke_log = Some(Box::new(move |i| {
        if let BoardInput::Stroke(cell, points) = i {
            sink.borrow_mut().push((cell, points.len()));
        }
    }));
    let given = given_cell(&d.app.game);
    d.tap_cell(given);
    let (x, y) = stroke_down(&mut d, given);
    d.send(Input::PenUp { x, y });
    let log = log.borrow();
    assert_eq!(log.len(), 2, "INP-22: {log:?}");
    assert!(log.iter().all(|e| e.0 == given), "INP-22: {log:?}");
    assert!(log[0].1 >= 1 && log[1].1 >= 3, "INP-22: {log:?}");
}

#[test]
fn a_finger_redo_that_fills_the_cell_of_the_touch_stops_the_pen_area() {
    // INP-22, INP-5: a finger holds Redo and the pen is on an empty cell while the player writes. Redo
    // fills that cell. Then no pen area and no ink stay on the filled cell.
    for_each_layout(|case| {
        let mut d = Driver::new(463);
        d.button(Button::Menu);
        let (cell, other) = {
            let mut e = cells().filter(|&c| !d.app.game.is_filled(c));
            (e.next().unwrap(), e.next().unwrap())
        };
        d.write(cell, 4);
        assert_eq!(d.app.game.value_at(cell), 4, "{case}");
        d.button(Button::Undo);
        assert_eq!(d.app.game.value_at(cell), 0, "{case}");
        let (bx, by) = hold(&mut d, Button::Redo);
        d.write_strokes(&scribble_ink(other));
        let (x, y) = micro_center(cell, 5);
        d.send(Input::PenDown { x, y, tool: Tool::Pen });
        assert!(d.app.writing_stroke().is_some(), "INP-24 ({case})");
        d.send(Input::TouchUp { id: 1, x: bx, y: by });
        assert_eq!(d.app.game.value_at(cell), 4, "the redo ({case})");
        assert!(d.app.writing_stroke().is_none(), "INP-22: no area on a filled cell ({case})");
        let (x1, y1) = micro_center(cell, 9);
        d.send(Input::PenMove { x: x1, y: y1 });
        assert!(d.app.writing_stroke().is_none(), "INP-22 ({case})");
        d.send(Input::PenUp { x: x1, y: y1 });
        assert!(d.app.ui.pending_ink.is_none(), "INP-5 ({case})");
        assert!(d.app.live_stroke.is_none(), "INP-5 ({case})");
    });
}

// ---- INP-2, INP-24: the ink start.

#[test]
fn a_press_on_an_empty_cell_starts_the_pen_area_at_the_pen_down() {
    // INP-2, INP-24, INP-16, INP-21. Each pen press on an empty cell starts the area at the pen down:
    // the first stroke of a session, a stroke with ink waiting or with the timer, a tap, and a press
    // after a tap. All layouts.
    for_each_layout(|case| {
        let mut d = Driver::new(470);
        let empties: Vec<Cell> = cells().filter(|&c| !d.app.game.is_filled(c)).collect();
        let (a, b, c) = (empties[0], empties[1], empties[2]);
        let given = given_cell(&d.app.game);

        // The first stroke.
        let (x, y) = micro_center(a, 1);
        d.send(Input::PenDown { x, y, tool: Tool::Pen });
        assert_eq!(d.app.pen_cell(), Some(a), "INP-24: the first stroke ({case})");
        assert_eq!(d.app.live_stroke.as_ref().map(|s| s[0]), Some((x, y)), "INP-24: from the pen down ({case})");
        let (x1, y1) = micro_center(a, 9);
        d.send(Input::PenMove { x: x1, y: y1 });
        assert!(d.app.writing_stroke().is_some(), "INP-2: the pen left the micro-cell ({case})");
        d.send(Input::PenUp { x: x1, y: y1 });
        assert_eq!(d.app.pen_cell(), Some(a), "INP-24: the cell of the waiting ink ({case})");

        // Ink waits: the next press on a different empty cell. The press reads the ink of `a`.
        let (x, y) = micro_center(b, 1);
        d.send(Input::PenDown { x, y, tool: Tool::Pen });
        assert_eq!(d.app.pen_cell(), Some(b), "INP-24: with ink ({case})");
        let (x1, y1) = micro_center(b, 9);
        d.send(Input::PenMove { x: x1, y: y1 });
        d.send(Input::PenUp { x: x1, y: y1 });
        assert!(d.app.ui.pending_ink.is_some(), "{case}");

        // A finger press does not stop this (INP-21).
        let (fx, fy) = no_control_point(false);
        d.send(Input::TouchDown { id: 1, x: fx, y: fy });
        d.send(Input::TouchUp { id: 1, x: fx, y: fy });
        let (x, y) = micro_center(c, 5);
        d.send(Input::PenDown { x, y, tool: Tool::Pen });
        assert_eq!(d.app.pen_cell(), Some(c), "INP-24, INP-21: after a finger ({case})");

        // A tap: its mark or x comes at the lift, and it starts no timer. The next press starts the
        // area again at its pen down.
        d.send(Input::PenUp { x, y });
        assert!(d.app.pen_cell().is_none(), "{case}");
        assert!(d.app.game.marks_at(c) == mark_bit(5) || d.app.ui.refused.is_some(), "INP-24: the mark or the x at the lift ({case})");
        assert!(!d.app.is_writing(), "INP-24: a tap starts no timer ({case})");
        // A press on a micro-cell that shows a mark asks for no area while it can be a tap: the tap
        // removes the mark with the UI waveform (R§14.4).
        if d.app.game.marks_at(c) == mark_bit(5) {
            d.send(Input::PenDown { x, y, tool: Tool::Pen });
            assert_eq!(d.app.pen_cell(), None, "INP-24: a press on a mark ({case})");
            let (x1, y1) = micro_center(c, 9);
            d.send(Input::PenMove { x: x1, y: y1 });
            assert_eq!(d.app.pen_cell(), Some(c), "INP-24: the pen left the micro-cell of the mark ({case})");
            d.send(Input::PenUp { x: x1, y: y1 });
        }
        let (x, y) = micro_center(c, 7);
        d.send(Input::PenDown { x, y, tool: Tool::Pen });
        assert_eq!(d.app.pen_cell(), Some(c), "INP-24: the press after a tap ({case})");
        d.send(Input::PenUp { x, y });

        // The timer with no ink: a stroke from a filled cell. A press on an empty cell starts the area.
        let (x1, y1) = stroke_down(&mut d, given);
        d.send(Input::PenUp { x: x1, y: y1 });
        assert!(d.app.ui.pending_ink.is_none() && d.app.is_writing(), "{case}");
        let (x, y) = micro_center(empties[3], 1);
        d.send(Input::PenDown { x, y, tool: Tool::Pen });
        assert_eq!(d.app.pen_cell(), Some(empties[3]), "INP-24: with the timer ({case})");
        let (x1, y1) = micro_center(empties[3], 9);
        d.send(Input::PenMove { x: x1, y: y1 });
        d.send(Input::PenUp { x: x1, y: y1 });

        // After the end of the pause and of the timer: the same start.
        d.wait(d.app.pause_ms);
        assert!(!d.app.is_writing(), "INP-16 ({case})");
        let (x, y) = micro_center(empties[4], 1);
        d.send(Input::PenDown { x, y, tool: Tool::Pen });
        assert_eq!(d.app.pen_cell(), Some(empties[4]), "INP-24: a new session ({case})");
        d.send(Input::PenUp { x, y });
    });
}

#[test]
fn a_touch_on_a_filled_cell_while_the_player_writes_starts_no_pen_area() {
    // INP-22, INP-24: only an empty cell starts the area at the pen down. The press reads the ink.
    for_each_layout(|case| {
        let mut d = Driver::new(471);
        let cell = empty_cell(&d.app.game);
        d.write_strokes(&digit_ink(cell, 4));
        let (x, y) = micro_center(given_cell(&d.app.game), 5);
        d.send(Input::PenDown { x, y, tool: Tool::Pen });
        assert!(d.app.writing_stroke().is_none() && d.app.live_stroke.is_none(), "INP-22 ({case})");
        d.send(Input::PenUp { x, y });
    });
}

#[test]
fn a_touch_on_a_complete_game_changes_nothing_and_starts_no_timer() {
    // INP-9, INP-16, INP-20, INP-22, HLT-1: the completed game. A hand touch and a stroke have no effect.
    let mut d = Driver::new(472);
    solve(&mut d);
    assert!(d.app.game.completed);
    d.wait(d.app.pause_ms);
    let before = board(&d.app.game);
    let (x, y) = stroke_down(&mut d, (4, 4));
    assert!(d.app.live_stroke.is_none(), "INP-22");
    d.send(Input::PenUp { x, y });
    assert!(!d.app.is_writing(), "INP-16: no timer on a complete game");
    assert_eq!(d.app.ui.highlight, None, "HLT-1");
    let menu = d.app.ui.menu;
    let (bx, by) = button_center(Button::Menu);
    d.touch_down_after_lift(100, bx, by);
    d.send_at_once(Input::TouchUp { id: 1, x: bx, y: by });
    assert_eq!(d.app.ui.menu, menu, "INP-20");
    assert_eq!(board(&d.app.game), before, "INP-9");
}

#[test]
fn a_tap_on_an_empty_cell_with_ink_elsewhere_keeps_the_pixels_until_the_lift() {
    // HLT-9, INP-24: the press reads the ink and starts the pen area. The highlight and the number of
    // the read do not go to the screen before the lift. Then the screen shows the full state.
    for_each_layout(|case| {
        let mut s = Shown::new(424);
        let given = given_cell(&s.d.app.game);
        let (a, b) = {
            let mut e = cells().filter(|&c| !s.d.app.game.is_filled(c));
            (e.next().unwrap(), e.next().unwrap())
        };
        s.pen_tap(micro_center(given, 5));
        s.pass(120);
        let lit = s.background(given);
        for stroke in &digit_ink(a, 4) {
            s.send(Input::PenDown { x: stroke[0].0, y: stroke[0].1, tool: Tool::Pen });
            for p in &stroke[1..] {
                s.send(Input::PenMove { x: p.0, y: p.1 });
            }
            let last = stroke[stroke.len() - 1];
            s.send(Input::PenUp { x: last.0, y: last.1 });
        }
        s.pass(150);
        let (x, y) = micro_center(b, 5);
        s.send(Input::PenDown { x, y, tool: Tool::Pen });
        assert!(s.d.app.writing_stroke().is_some(), "INP-24 ({case})");
        assert_eq!(s.d.app.game.value_at(a), 4, "INP-12 ({case})");
        assert_eq!(s.d.app.ui.highlight, None, "HLT-2 ({case})");
        s.pass(90);
        assert_eq!(s.background(given), lit, "HLT-9: the pixels wait for the lift ({case})");
        s.changes = 0;
        s.send(Input::PenUp { x, y });
        s.pass(120);
        assert_ne!(s.background(given), lit, "HLT-2 ({case})");
        assert_eq!(s.changes, 1, "HLT-9: one redraw after the lift ({case})");
        s.assert_complete(&case);
    });
}

#[test]
fn a_stroke_from_a_number_with_the_menu_open_removes_the_highlight() {
    // HLT-2 (item 3), NAV-11: the item applies while `panel = None`, also with the menu open. The test
    // `a_pen_press_on_the_board_with_a_panel_open_only_closes_the_panel` has the case of a panel.
    for_each_layout(|case| {
        let mut d = Driver::new(419);
        let (given, v) = highlight_a_number(&mut d);
        d.button(Button::Menu);
        let (x, y) = stroke_down(&mut d, given);
        assert_eq!(d.app.ui.highlight, Some(v), "HLT-9 ({case})");
        d.send(Input::PenUp { x, y });
        assert_eq!(d.app.ui.highlight, None, "HLT-2 ({case})");
        assert!(d.app.ui.menu.is_open(), "{case}");
    });
}

// ---- The result card of a complete puzzle (REQUIREMENTS.md §8.3, §11; CMP-7 to CMP-10, PUZ-7).

/// The center of a level button, from the documented values (R§8.3, CMP-7), not from `layout`.
/// Portrait: the buttons are at x 820–1439, y 1865–1935, 1945–2015 and 2025–2095. Landscape: x0 + 20
/// to x1 − 20 of the column (x 48–370 for a left-handed player, 1790–2112 for a right-handed player),
/// 120 px high with a pitch of 140 px, the first at y 911 + 90.
fn documented_level_center(b: layout::LevelButton) -> (f32, f32) {
    let i = layout::LevelButton::ALL.iter().position(|&x| x == b).unwrap() as f32;
    if layout::orientation() == layout::Orientation::Landscape {
        let x0 = if layout::right_handed() { 1790.0 } else { 48.0 };
        (x0 + 161.0, 1001.0 + i * 140.0 + 60.0)
    } else {
        (1129.5, 1865.0 + i * 80.0 + 35.0)
    }
}

/// The tool of a tap on a level button: the pen tip, the eraser or a finger.
#[derive(Clone, Copy, Debug)]
enum Tapper {
    Pen,
    Eraser,
    Finger,
}

fn tap_with(d: &mut Driver, by: Tapper, (x, y): (f32, f32)) {
    match by {
        Tapper::Pen => d.pen_tap(x, y),
        Tapper::Eraser => {
            d.send(Input::PenDown { x, y, tool: Tool::Eraser });
            d.send(Input::PenUp { x, y });
        }
        Tapper::Finger => d.finger_tap(x, y),
    }
}

/// A driver with the game `new_game(seed, mode)` and the results `db`, completed with the pen: the
/// last number completes the game at the read of the ink at the end of the pause (a tick). The menu
/// is closed.
fn completed_with(seed: u64, mode: u8, db: ResultsDb) -> Driver {
    completed_sym(seed, mode, false, db)
}

/// `completed_with` for the game `new_game_with_symmetry(seed, mode, symmetric)`.
fn completed_sym(seed: u64, mode: u8, symmetric: bool, db: ResultsDb) -> Driver {
    let game = sudoku::app::new_game_with_symmetry(seed, mode, symmetric);
    let app = App::new(Some(game), None, db, Box::new(TestEnv { time: START_TIME, seed }), App::placeholder_ng_stops());
    let mut d = Driver::from(app);
    complete_with_the_pen(&mut d);
    d
}

fn complete_with_the_pen(d: &mut Driver) {
    let last = empty_cell(&d.app.game);
    fill_except(d, last);
    d.write(last, d.app.game.solution[last.0 as usize][last.1 as usize]);
    assert!(d.app.game.completed && d.app.screen == Screen::Game);
}

fn completed_at(seed: u64, mode: u8) -> Driver {
    completed_with(seed, mode, ResultsDb::in_memory())
}

/// The new game after a level button: DIF-5 with `mode`, NAV-9 (the menu closed, no panel), PUZ-5.
fn assert_level_game(d: &Driver, old_seed: u64, mode: u8, case: &str) {
    assert_level_game_sym(d, old_seed, mode, false, case);
}

/// `assert_level_game` for a completed game with the symmetry `symmetric`: CMP-10 and DIF-5 give the
/// new game that symmetry, and `last_symmetric` becomes that symmetry.
fn assert_level_game_sym(d: &Driver, old_seed: u64, mode: u8, symmetric: bool, case: &str) {
    let g = &d.app.game;
    assert_eq!((g.mode, d.app.settings.last_mode), (mode, mode), "DIF-5, CMP-10 ({case})");
    assert_eq!((g.symmetric, d.app.settings.last_symmetric), (symmetric, symmetric), "DIF-5, CMP-10: symmetry ({case})");
    assert_ne!(g.seed, old_seed, "DIF-5: a new seed ({case})");
    assert!(!g.completed && g.elapsed_ms < 1000, "CMP-6, DIF-5 ({case})");
    assert!(g.undo.is_empty() && g.redo.is_empty() && g.checkpoints.is_empty(), "DIF-5 ({case})");
    assert!(cells().all(|c| g.is_given(c) || (!g.is_filled(c) && g.marks_at(c) == 0)), "DIF-5 ({case})");
    assert_eq!((d.app.screen, d.app.ui.menu), (Screen::Game, Menu::Closed), "NAV-9, CMP-10 ({case})");
    let p = sudoku::generator::generate_version(2, g.seed, mode, symmetric);
    let given = p.givens.map(|row| row.map(|v| v != 0));
    assert_eq!((g.generator, g.solution, g.given), (2, p.solution, given), "PUZ-5, PUZ-7 ({case})");
}

#[test]
fn a_tap_on_a_level_button_starts_a_game_at_its_mode_in_all_layouts() {
    // CMP-9, CMP-10, DIF-5, DIF-7, NAV-9, PUZ-7: Easier, Same and Harder at the modes 0, 3 and 6, with
    // the pen tip, the eraser and a finger, in the four layouts. One tap starts the game, with no
    // confirmation, and the menu is closed after it.
    use layout::LevelButton::{Easier, Harder, Same};
    let mut k = 0u64;
    for_each_layout(|layout_case| {
        for (mode, b, by) in [
            (3, Easier, Tapper::Pen),
            (3, Same, Tapper::Eraser),
            (3, Harder, Tapper::Finger),
            (0, Same, Tapper::Finger),
            (0, Harder, Tapper::Eraser),
            (6, Easier, Tapper::Finger),
            (6, Same, Tapper::Pen),
        ] {
            k += 1;
            let case = format!("{layout_case}, mode {mode}, {b:?}, {by:?}");
            let mut d = completed_at(600 + k % 3, mode);
            let (old_seed, results) = (d.app.game.seed, d.app.results.len());
            let target = match b {
                Easier => mode - 1,
                Same => mode,
                Harder => mode + 1,
            };
            tap_with(&mut d, by, documented_level_center(b));
            assert_level_game(&d, old_seed, target, &case);
            assert_eq!(d.app.results.len(), results, "DIF-7 ({case})");
            assert_eq!(d.app.ui.slider, target, "R§11: the slider takes the new mode ({case})");
            d.button(Button::Menu);
            d.button(Button::NewGame);
            assert_eq!(d.app.ui.slider, target, "DIF-5: the New game panel opens at last_mode ({case})");
        }
    });
}

#[test]
fn a_level_button_with_the_menu_open_closes_the_menu() {
    // NAV-9, CMP-7, CMP-10: with the menu open and no panel, a level button starts the game and closes
    // the menu. In landscape the toolbar and the card are at opposite sides of the board.
    for_each_layout(|case| {
        let mut d = completed_at(611, 2);
        d.button(Button::Menu);
        assert_eq!(d.app.ui.menu, Menu::Open(None));
        let old = d.app.game.seed;
        let (x, y) = documented_level_center(layout::LevelButton::Harder);
        d.finger_tap(x, y);
        assert_level_game(&d, old, 3, case);
    });
}

#[test]
fn start_new_game_on_a_complete_game_keeps_the_menu_open() {
    // NAV-9, CMP-6, DIF-5: the card does not change Start new game. The menu stays open after it.
    let mut d = completed_at(612, 2);
    d.button(Button::Menu);
    d.button(Button::NewGame);
    let (stops, row_y) = (d.app.ng_stops, layout::ng_slider_y() as f32);
    d.finger_tap(stops[5], row_y);
    start_new_game(&mut d);
    let g = &d.app.game;
    assert!(!g.completed, "CMP-6");
    assert_eq!((g.mode, d.app.settings.last_mode), (5, 5), "DIF-5");
    assert_eq!(d.app.ui.menu, Menu::Open(None), "NAV-9");
}

#[test]
fn a_release_on_a_different_place_does_nothing() {
    // CMP-10: a press on a level button and a release on a different place (an other button, the
    // board, the frame of the card, the gap above the button) changes nothing, with the pen, the
    // eraser and a finger.
    use layout::LevelButton::{Harder, Same};
    for_each_layout(|layout_case| {
        let mut d = completed_at(613, 3);
        let g = d.app.game.clone();
        let (sx, sy) = documented_level_center(Same);
        let landscape = layout::orientation() == layout::Orientation::Landscape;
        let card_frame = if landscape { (sx, 1471.0 - 4.0) } else { (sx, 2110.0 - 4.0) };
        let (bx, by) = layout::cell_origin((4, 4));
        let board = (bx as f32 + 73.0, by as f32 + 73.0);
        // The gap between Easier and Same: y 1935–1945 in portrait, 1121–1141 in landscape.
        let gap = (sx, if landscape { 1131.0 } else { 1940.0 });
        for to in [documented_level_center(Harder), board, card_frame, gap] {
            for by in [Tapper::Pen, Tapper::Eraser, Tapper::Finger] {
                let case = format!("{layout_case}, {by:?}, release at {to:?}");
                match by {
                    Tapper::Finger => {
                        d.send(Input::TouchDown { id: 1, x: sx, y: sy });
                        d.send(Input::TouchMove { id: 1, x: to.0, y: to.1 });
                        d.send(Input::TouchUp { id: 1, x: to.0, y: to.1 });
                    }
                    _ => {
                        let tool = if matches!(by, Tapper::Pen) { Tool::Pen } else { Tool::Eraser };
                        d.send(Input::PenDown { x: sx, y: sy, tool });
                        d.send(Input::PenMove { x: to.0, y: to.1 });
                        d.send(Input::PenUp { x: to.0, y: to.1 });
                    }
                }
                d.wait(d.app.pause_ms + 50);
                assert_eq!(d.app.game, g, "CMP-10 ({case})");
                assert_eq!((d.app.screen, d.app.ui.menu), (Screen::Game, Menu::Closed), "{case}");
            }
        }
        // A tap after this still operates.
        tap_with(&mut d, Tapper::Finger, (sx, sy));
        assert_level_game(&d, g.seed, 3, layout_case);
    });
}

#[test]
fn a_gray_level_button_does_nothing() {
    // CMP-9, CMP-10: Easier after an Easy game (mode 0) and Harder after a Hard game (mode 6) are not
    // available. A tap on them changes nothing. A level button never gives a mode outside 0..6.
    use layout::LevelButton::{Easier, Harder};
    for_each_layout(|layout_case| {
        for (mode, b) in [(0u8, Easier), (6, Harder)] {
            let mut d = completed_at(614, mode);
            let g = d.app.game.clone();
            let last_mode = d.app.settings.last_mode;
            for by in [Tapper::Pen, Tapper::Eraser, Tapper::Finger] {
                let case = format!("{layout_case}, mode {mode}, {b:?}, {by:?}");
                tap_with(&mut d, by, documented_level_center(b));
                d.wait(d.app.pause_ms + 50);
                assert_eq!(d.app.game, g, "CMP-9 ({case})");
                assert_eq!(d.app.settings.last_mode, last_mode, "CMP-9 ({case})");
                assert_eq!((d.app.screen, d.app.ui.menu), (Screen::Game, Menu::Closed), "{case}");
            }
            assert_eq!(b.target(mode), None, "CMP-10 ({layout_case}, mode {mode}, {b:?})");
            d.app.level_button(b);
            assert_eq!(d.app.game, g, "CMP-10 ({layout_case}, mode {mode}, {b:?})");
        }
    });
}

#[test]
fn a_tap_on_the_card_with_a_panel_open_only_closes_the_panel() {
    // CMP-10, NAV-11, CMP-6: with a panel open, a tap on a level button outside the panel only closes
    // the panel. The next tap starts the game.
    use layout::LevelButton::{Easier, Harder, Same};
    for_each_layout(|layout_case| {
        for (b, by) in [(Easier, Tapper::Finger), (Same, Tapper::Pen), (Harder, Tapper::Eraser)] {
            let case = format!("{layout_case}, {b:?}, {by:?}");
            let mut d = completed_at(615, 3);
            let g = d.app.game.clone();
            d.button(Button::Menu);
            d.button(Button::Colors);
            assert_eq!(d.app.ui.menu.panel(), Some(Panel::Colors), "CMP-6 ({case})");
            tap_with(&mut d, by, documented_level_center(b));
            assert_eq!(d.app.game, g, "CMP-10 ({case})");
            assert_eq!(d.app.ui.menu, Menu::Open(None), "NAV-11 ({case})");
            tap_with(&mut d, by, documented_level_center(b));
            assert_level_game(&d, g.seed, b.target(3).unwrap(), &case);
        }
    });
}

#[test]
fn the_same_puzzle_starts_at_completion_by_the_read_of_the_ink() {
    // PUZ-7, PUZ-5: the read of the ink at the end of the pause (a tick, not an input) completes the
    // game. At that tick the app already has early_game = (next_seed, mode). Same, Easier and Harder
    // use that seed, with their own mode.
    use layout::LevelButton::{Easier, Harder, Same};
    for b in [Same, Easier, Harder] {
        let mut d = Driver::from(app_with_game(sudoku::app::new_game(620, 3), START_TIME, 620));
        assert_eq!(d.app.early_game(), None);
        let last = empty_cell(&d.app.game);
        fill_except(&mut d, last);
        let v = d.app.game.solution[last.0 as usize][last.1 as usize];
        d.write_strokes(&digit_ink(last, v));
        assert!(!d.app.game.completed);
        assert_eq!(d.app.early_game(), None, "PUZ-7: no early game before the completion");
        let end = d.now + d.app.pause_ms + 500;
        while !d.app.game.completed {
            assert!(d.now < end, "the ink was not read");
            d.now += 10;
            d.app.tick(d.now);
        }
        let (seed, mode, symmetric) = d.app.early_game().expect("PUZ-7: the early game starts at the completing tick");
        assert_eq!((mode, symmetric), (3, false), "PUZ-7: the Same mode and the symmetry of the game");
        assert_ne!(seed, d.app.game.seed, "PUZ-7: the next seed");
        d.wait(2000);
        assert_eq!(d.app.early_game(), Some((seed, 3, false)), "PUZ-7");
        let old = d.app.game.seed;
        let (x, y) = documented_level_center(b);
        d.finger_tap(x, y);
        let target = b.target(3).unwrap();
        assert_eq!(d.app.game.seed, seed, "PUZ-7: {b:?} uses next_seed");
        assert_level_game(&d, old, target, &format!("{b:?}"));
        assert_eq!(d.app.game, sudoku::app::new_game(seed, target), "PUZ-5, PUZ-7 ({b:?})");
        assert_eq!(d.app.early_game(), None, "PUZ-7: the new game is not complete");
    }
}

#[test]
fn the_same_puzzle_starts_when_a_completed_game_opens_from_the_save() {
    // PUZ-7: a completed game that opens from the save starts the early game at once, before an input.
    // The game is symmetric: the key and the game of Same have its symmetry (PUZ-7, CMP-10).
    let done = completed_sym(621, 4, true, ResultsDb::in_memory()).app.game;
    assert!(done.symmetric);
    let mut d = Driver::opened(app_with_game(done.clone(), START_TIME, 77));
    assert!(d.app.game.completed);
    let (seed, mode, symmetric) = d.app.early_game().expect("PUZ-7: the early game starts at the open");
    assert_eq!((mode, symmetric), (4, true), "PUZ-7");
    assert_ne!(seed, done.seed);
    // A game that is not complete has no early game at the open (the menu is open, no panel).
    let other = Driver::opened(app_with_game(sudoku::app::new_game(621, 4), START_TIME, 77));
    assert_eq!(other.app.early_game(), None, "PUZ-7");
    // Same uses that seed. The app opened with the menu open: after the tap it is closed (NAV-9).
    d.wait(1000);
    let (x, y) = documented_level_center(layout::LevelButton::Same);
    d.finger_tap(x, y);
    assert_eq!(d.app.game, sudoku::app::new_game_with_symmetry(seed, 4, true), "PUZ-5, PUZ-7");
    assert_eq!(d.app.ui.menu, Menu::Closed, "NAV-9");
}

#[test]
fn the_comparison_uses_the_earlier_results_at_the_same_mode() {
    // CMP-8: the average (ANA-4) and the minimum of the earlier results at the mode of the game, but not
    // the result of the game itself (the same seed) and not the results of other modes. After a delete
    // in Statistics the card shows the new values.
    use sudoku::app::Comparison;
    use sudoku::store::NewResult;
    let mut db = ResultsDb::in_memory();
    let row = |t: i64, mode: u8, seconds: u64, seed: u64| NewResult { finished_at: 1_700_000_000 + t, mode, givens: 30, seconds, seed, generator: 2, symmetric: seed % 2 == 0 };
    for r in [row(0, 2, 100, 1), row(1, 2, 300, 2), row(2, 2, 500, 3), row(3, 3, 50, 4), row(4, 1, 20, 5)] {
        db.insert(&r).unwrap();
    }
    let mut d = completed_with(630, 2, db);
    let own = d.app.results.iter().find(|r| r.seed == d.app.game.seed).expect("CMP-5").seconds;
    assert!(own < 100);
    assert_eq!(d.app.card_comparison(), Comparison::Earlier { average: 300, best: 100 }, "CMP-8");
    // The delete of the oldest result at mode 2 (100 s), in the list of completed games.
    d.button(Button::Menu);
    d.button(Button::Statistics);
    let all = layout::all_games_button();
    d.finger_tap((all.x0 + 100) as f32, (all.y0 + 20) as f32);
    let seg = layout::filter_segment(Some(2));
    d.finger_tap((seg.x0 + 10) as f32, (seg.y0 + 10) as f32);
    assert_eq!(d.app.page_rows().iter().map(|r| r.seconds).collect::<Vec<_>>(), [own, 500, 300, 100]);
    let bin = layout::bin_button(3);
    d.finger_tap(((bin.x0 + bin.x1) / 2) as f32, ((bin.y0 + bin.y1) / 2) as f32);
    let del = layout::dialog_delete();
    d.finger_tap((del.x0 + 20) as f32, (del.y0 + 20) as f32);
    d.finger_tap(40.0, 50.0);
    d.finger_tap(40.0, 50.0);
    assert_eq!(d.app.screen, Screen::Game);
    assert_eq!(d.app.card_comparison(), Comparison::Earlier { average: 400, best: 300 }, "CMP-8 after a delete");
}

#[test]
fn the_first_game_at_a_mode_has_no_comparison() {
    // CMP-8: with no earlier result at the mode (only a result of an other mode and the result of the
    // game itself), the card shows only "First game at this level". After Same, the completed game
    // is an earlier result of the next game.
    use sudoku::app::Comparison;
    use sudoku::store::NewResult;
    let mut db = ResultsDb::in_memory();
    db.insert(&NewResult { finished_at: 1_700_000_000, mode: 3, givens: 30, seconds: 77, seed: 9, generator: 2, symmetric: true }).unwrap();
    let mut d = completed_with(631, 2, db);
    assert!(d.app.results.iter().any(|r| r.mode == 2 && r.seed == d.app.game.seed), "CMP-5");
    assert_eq!(d.app.card_comparison(), Comparison::First, "CMP-8");
    let first = d.app.results.iter().find(|r| r.seed == d.app.game.seed).unwrap().seconds;
    let (x, y) = documented_level_center(layout::LevelButton::Same);
    d.finger_tap(x, y);
    complete_with_the_pen(&mut d);
    assert_eq!(d.app.card_comparison(), Comparison::Earlier { average: first, best: first }, "CMP-8");
}

// ---- The Symmetrical switch of the New game panel (R§11, DIF-3 to DIF-5, DIF-8, PUZ-7, PUZ-8, CMP-10,
// PER-9, ANA-1).

/// A tap on the start row of the New game panel, in all layouts.
fn tap_start(d: &mut Driver) {
    d.finger_tap(layout::ng_pl() as f32 + 50.0, layout::ng_start_y() as f32);
}

/// Opens the menu (if it is closed) and the New game panel.
fn open_new_game_panel(d: &mut Driver) {
    if d.app.ui.menu == Menu::Closed {
        d.button(Button::Menu);
    }
    d.button(Button::NewGame);
    assert_eq!(d.app.ui.menu.panel(), Some(Panel::NewGame));
}

/// PUZ-8: the starting numbers are the same after a half turn of the board.
fn givens_are_symmetric(g: &sudoku::game::Game) -> bool {
    cells().all(|(r, c)| g.is_given((r, c)) == g.is_given((8 - r, 8 - c)))
}

/// The points of the Symmetrical row that the tests tap: the four corners and the center. R§11: the
/// row is the 112 px directly above the bottom toolbar slot, the full width of the panel.
fn symmetric_row_points() -> Vec<(f32, f32)> {
    let (x0, x1) = (layout::ng_pl() as f32, layout::ng_pr() as f32);
    let (y0, y1) = ((layout::tb_bottom_y() - 112) as f32, (layout::tb_bottom_y() - 1) as f32);
    vec![(x0 + 1.0, y0 + 0.5), (x1 - 1.0, y0 + 0.5), (x0 + 1.0, y1 + 0.5), (x1 - 1.0, y1 + 0.5), ((x0 + x1) / 2.0, (y0 + y1) / 2.0)]
}

#[test]
fn the_new_game_panel_has_the_documented_symmetrical_row() {
    // R§11: in portrait, the panel top is at 2048 - 226 = 1822, the slider at top + 56, and the
    // Symmetrical row is y 1936 to 2047 (center 1992), from the panel left (1002).
    assert_eq!(layout::tb_bottom_y(), 2048);
    assert_eq!(layout::ng_top(), 1822);
    assert_eq!(layout::ng_slider_y(), 1822 + 56);
    assert_eq!(layout::ng_symmetric_y(), 1992);
    let row = layout::ng_symmetric_row();
    assert_eq!((row.x0, row.y0, row.y1), (1002, 1936, 2048));
    assert_eq!(row.x1, layout::ng_pr() + 1);
    assert_eq!(layout::ng_pr() - layout::ng_pl() + 1, 506, "R§11: the width does not change");
}

#[test]
fn a_tap_on_the_symmetrical_row_changes_only_the_selection() {
    // DIF-8, DIF-4, DIF-3, DIF-9: at the first start the switch is on. A tap anywhere on the row (pen
    // tip or finger) changes symmetric_sel to its opposite value. The game, last_mode, last_symmetric
    // and the slider do not change, and the panel stays open. In the four layouts.
    for_each_layout(|layout_case| {
        let mut d = Driver::new(700);
        assert!(d.app.settings.last_symmetric && d.app.game.symmetric, "DIF-9 ({layout_case})");
        assert!(givens_are_symmetric(&d.app.game), "PUZ-8, DIF-9 ({layout_case})");
        open_new_game_panel(&mut d);
        assert!(d.app.ui.symmetric_sel, "DIF-3 ({layout_case})");
        let (game, slider) = (d.app.game.clone(), d.app.ui.slider);
        let mut want = true;
        for (k, (x, y)) in symmetric_row_points().into_iter().enumerate() {
            for pen in [false, true] {
                let case = format!("{layout_case}, point {k}, pen {pen}");
                if pen {
                    d.pen_tap(x, y);
                } else {
                    d.finger_tap(x, y);
                }
                want = !want;
                assert_eq!(d.app.ui.symmetric_sel, want, "DIF-8 ({case})");
                assert_eq!(d.app.ui.menu.panel(), Some(Panel::NewGame), "DIF-8: the panel stays open ({case})");
                assert_eq!(board(&d.app.game), board(&game), "DIF-4 ({case})");
                assert_eq!((d.app.settings.last_mode, d.app.settings.last_symmetric, d.app.ui.slider), (2, true, slider), "DIF-4 ({case})");
            }
        }
    });
}

#[test]
fn a_press_on_the_symmetrical_row_and_a_release_out_of_it_changes_nothing() {
    // DIF-8: a press on the row and a release on a different place (the slider row, the start row, the
    // bottom divider, the board) changes nothing, with the pen and a finger. A press on the slider row
    // or the start row and a release on the Symmetrical row changes nothing also.
    for_each_layout(|layout_case| {
        let mut d = Driver::new(701);
        open_new_game_panel(&mut d);
        let game = d.app.game.clone();
        let (x, y) = (layout::ng_tx() as f32 + 20.0, layout::ng_symmetric_y() as f32);
        let (bx, by) = layout::cell_origin((4, 4));
        let others = [
            (x, layout::ng_slider_y() as f32 - 40.0),
            (x, layout::ng_start_y() as f32),
            (bx as f32 + 73.0, by as f32 + 73.0),
            (x, (layout::tb_bottom_y() + 1) as f32),
        ];
        for to in others {
            for pen in [false, true] {
                let case = format!("{layout_case}, {to:?}, pen {pen}");
                // A press on the board closes the panel (NAV-11): only the press on the row for it.
                let board_point = to == others[2];
                let moves: &[((f32, f32), (f32, f32))] = if board_point { &[((x, y), to)] } else { &[((x, y), to), (to, (x, y))] };
                for &(from, at) in moves {
                    if pen {
                        d.send(Input::PenDown { x: from.0, y: from.1, tool: Tool::Pen });
                        d.send(Input::PenMove { x: at.0, y: at.1 });
                        d.send(Input::PenUp { x: at.0, y: at.1 });
                    } else {
                        d.send(Input::TouchDown { id: 1, x: from.0, y: from.1 });
                        d.send(Input::TouchMove { id: 1, x: at.0, y: at.1 });
                        d.send(Input::TouchUp { id: 1, x: at.0, y: at.1 });
                    }
                    d.wait(d.app.pause_ms + 50);
                    assert!(d.app.ui.symmetric_sel, "DIF-8 ({case}, from {from:?})");
                    assert_eq!(d.app.ui.menu.panel(), Some(Panel::NewGame), "{case}, from {from:?}");
                    assert_eq!(board(&d.app.game), board(&game), "{case}, from {from:?}");
                }
            }
        }
        // A tap after this still operates.
        d.finger_tap(x, y);
        assert!(!d.app.ui.symmetric_sel, "DIF-8 ({layout_case})");
    });
}

#[test]
fn the_selection_is_lost_when_the_panel_closes() {
    // DIF-4, DIF-8, DIF-3: the panel closes with a tap on + , a tap out of the panel (the board) or
    // an other panel (Colors, Settings). The selection is lost: the panel opens again with
    // last_symmetric, and last_symmetric and the game do not change.
    for_each_layout(|layout_case| {
        for last in [true, false] {
            let settings = Settings { last_symmetric: last, ..Settings::default() };
            let app = App::new(None, Some(settings), ResultsDb::in_memory(), Box::new(TestEnv { time: START_TIME, seed: 702 }), App::placeholder_ng_stops());
            let mut d = Driver::from(app);
            assert_eq!(d.app.game.symmetric, last, "DIF-5: the first game has last_symmetric ({layout_case})");
            let game = d.app.game.clone();
            let (bx, by) = layout::cell_origin((4, 4));
            for close in ["+", "board", "colors", "settings"] {
                let case = format!("{layout_case}, last {last}, close by {close}");
                open_new_game_panel(&mut d);
                assert_eq!(d.app.ui.symmetric_sel, last, "DIF-3 ({case})");
                d.finger_tap(layout::ng_tx() as f32, layout::ng_symmetric_y() as f32);
                assert_eq!(d.app.ui.symmetric_sel, !last, "DIF-8 ({case})");
                match close {
                    "+" => d.button(Button::NewGame),
                    "board" => d.finger_tap(bx as f32 + 73.0, by as f32 + 73.0),
                    "colors" => d.button(Button::Colors),
                    _ => d.button(Button::Settings),
                }
                assert_ne!(d.app.ui.menu.panel(), Some(Panel::NewGame), "{case}");
                assert_eq!(d.app.ui.symmetric_sel, last, "DIF-4, DIF-8: the selection is lost ({case})");
                assert_eq!(d.app.settings.last_symmetric, last, "DIF-4 ({case})");
                assert_eq!(board(&d.app.game), board(&game), "DIF-4 ({case})");
                if d.app.ui.menu.panel().is_some() {
                    d.button(Button::Menu);
                }
                open_new_game_panel(&mut d);
                assert_eq!(d.app.ui.symmetric_sel, last, "DIF-3: the panel opens with last_symmetric ({case})");
                d.button(Button::NewGame);
                if d.app.ui.menu != Menu::Closed {
                    d.button(Button::Menu);
                }
            }
        }
    });
}

#[test]
fn start_new_game_keeps_the_selected_symmetry() {
    // DIF-5, DIF-3, PUZ-5, PUZ-8: Start new game makes the puzzle of (next_seed, slider, symmetric_sel)
    // and keeps the two values. The panel opens again with them. Both directions of the switch, with
    // a slider change, in the four layouts.
    for_each_layout(|layout_case| {
        let mut d = Driver::new(703);
        let stops = d.app.ng_stops;
        for (k, (stop, sym)) in [(4usize, false), (0, true), (6, false), (1, false), (3, true)].into_iter().enumerate() {
            let case = format!("{layout_case}, game {k}");
            open_new_game_panel(&mut d);
            if d.app.ui.symmetric_sel != sym {
                d.pen_tap(layout::ng_tx() as f32 + 100.0, layout::ng_symmetric_y() as f32);
            }
            d.finger_tap(stops[stop], layout::ng_slider_y() as f32);
            assert_eq!((d.app.ui.slider, d.app.ui.symmetric_sel), (stop as u8, sym), "{case}");
            let old = d.app.game.seed;
            tap_start(&mut d);
            let g = &d.app.game;
            assert_ne!(g.seed, old, "DIF-5 ({case})");
            assert_eq!((g.mode, g.symmetric, g.generator), (stop as u8, sym, 2), "DIF-5, PUZ-8 ({case})");
            assert_eq!((d.app.settings.last_mode, d.app.settings.last_symmetric), (stop as u8, sym), "DIF-5 ({case})");
            assert_eq!(*g, sudoku::app::new_game_with_symmetry(g.seed, stop as u8, sym), "PUZ-5 ({case})");
            if sym {
                assert!(givens_are_symmetric(g), "PUZ-8 ({case})");
            }
            if sym && stop == 0 {
                assert!((44..=45).contains(&g.givens_count()), "PUZ-9 ({case})");
            }
            assert_eq!(d.app.ui.menu, Menu::Open(None), "NAV-9 ({case})");
            d.button(Button::NewGame);
            assert_eq!((d.app.ui.slider, d.app.ui.symmetric_sel), (stop as u8, sym), "DIF-3 ({case})");
            d.button(Button::NewGame);
        }
    });
}

#[test]
fn the_background_puzzle_of_the_panel_follows_the_switch() {
    // PUZ-7, DIF-8: while the panel is open, the key of the background puzzle is (next_seed, slider,
    // symmetric_sel). A change of the switch starts it again for the new key. Start new game gives the
    // game of that key, the same as the generator makes at the time of need (PUZ-5).
    let mut d = Driver::new(704);
    let stops = d.app.ng_stops;
    open_new_game_panel(&mut d);
    let (seed, mode, sym) = d.app.early_game().expect("PUZ-7: the panel starts the background puzzle");
    assert_eq!((mode, sym), (2, true), "PUZ-7");
    let (x, y) = (layout::ng_tx() as f32, layout::ng_symmetric_y() as f32);
    d.finger_tap(x, y);
    assert_eq!(d.app.early_game(), Some((seed, 2, false)), "PUZ-7: a switch change starts it again");
    d.finger_tap(stops[5], layout::ng_slider_y() as f32);
    assert_eq!(d.app.early_game(), Some((seed, 5, false)), "PUZ-7");
    d.finger_tap(x, y);
    assert_eq!(d.app.early_game(), Some((seed, 5, true)), "PUZ-7");
    d.finger_tap(x, y);
    d.wait(2000);
    assert_eq!(d.app.early_game(), Some((seed, 5, false)), "PUZ-7");
    tap_start(&mut d);
    assert_eq!(d.app.game.seed, seed, "PUZ-7: Start new game uses next_seed");
    let p = sudoku::generator::generate_version(2, seed, 5, false);
    assert_eq!((d.app.game.solution, d.app.game.given), (p.solution, p.givens.map(|r| r.map(|v| v != 0))), "PUZ-5, PUZ-7");
    assert!(!d.app.game.symmetric);
    // The non-symmetric puzzle of a seed is not its symmetric puzzle (R§14.5: a different sequence).
    assert_ne!(sudoku::generator::generate_version(2, seed, 5, true).givens, p.givens, "R§14.5");
}

#[test]
fn a_level_button_keeps_the_symmetry_of_the_completed_game() {
    // CMP-10, DIF-5, PUZ-7, CMP-9: Easier, Same and Harder start the new game with the symmetry of the
    // completed game, also when last_symmetric is different. That symmetry becomes last_symmetric, and
    // the panel opens with it (DIF-3). The background puzzle of the card has the symmetry of the game.
    use layout::LevelButton::{Easier, Harder, Same};
    for_each_layout(|layout_case| {
        for (k, (b, sym)) in [(Same, true), (Easier, true), (Harder, true), (Same, false), (Harder, false)].into_iter().enumerate() {
            let case = format!("{layout_case}, {b:?}, symmetric {sym}");
            let seed = 710 + k as u64;
            let game = sudoku::app::new_game_with_symmetry(seed, 3, sym);
            // last_symmetric is the opposite of the symmetry of the completed game.
            let settings = Settings { last_mode: 3, last_symmetric: !sym, ..Settings::default() };
            let app = App::new(Some(game), Some(settings), ResultsDb::in_memory(), Box::new(TestEnv { time: START_TIME, seed }), App::placeholder_ng_stops());
            let mut d = Driver::from(app);
            complete_with_the_pen(&mut d);
            assert_eq!(d.app.settings.last_symmetric, !sym, "the completion does not change it ({case})");
            assert_eq!(d.app.results.iter().find(|r| r.seed == seed).map(|r| r.symmetric), Some(sym), "CMP-5 ({case})");
            let (next, mode, key_sym) = d.app.early_game().expect("PUZ-7");
            assert_eq!((mode, key_sym), (3, sym), "PUZ-7 ({case})");
            let old = d.app.game.seed;
            tap_with(&mut d, Tapper::Finger, documented_level_center(b));
            let target = b.target(3).unwrap();
            assert_eq!(d.app.game.seed, next, "PUZ-7 ({case})");
            assert_level_game_sym(&d, old, target, sym, &case);
            assert_eq!(d.app.game, sudoku::app::new_game_with_symmetry(next, target, sym), "PUZ-5, PUZ-7 ({case})");
            if sym {
                assert!(givens_are_symmetric(&d.app.game), "PUZ-8 ({case})");
            }
            open_new_game_panel(&mut d);
            assert_eq!((d.app.ui.slider, d.app.ui.symmetric_sel), (target, sym), "DIF-3 ({case})");
        }
    });
}

#[test]
fn the_files_of_an_earlier_version_get_the_documented_symmetry() {
    // PER-9, PER-3, PER-1, PER-5, PUZ-8: a settings file with no symmetry gets last_symmetric = true. A
    // game file with no symmetry is not symmetric, and the app keeps it. A saved symmetric game stays
    // symmetric. The app does not use a game whose symmetry does not agree with its puzzle, or a
    // symmetric game of version 1. Each start gives the same result.
    use sudoku::game::Game;
    let dir = std::env::temp_dir().join(format!("sudoku-symmetry-files-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (gp, sp) = (dir.join("game.json"), dir.join("settings.json"));
    let start = || App::new(sudoku::store::load_json(&gp), sudoku::store::load_json(&sp), ResultsDb::in_memory(), Box::new(TestEnv { time: 0, seed: 720 }), App::placeholder_ng_stops());
    // Settings of an earlier version: on. A settings file with false keeps false (PER-3).
    std::fs::write(&sp, r#"{"color_starting":"Gray","color_clash":"Red","color_highlight":"Cyan","color_completed":"Green","last_mode":4}"#).unwrap();
    for _ in 0..2 {
        assert!(start().settings.last_symmetric, "PER-9");
    }
    let off = Settings { last_symmetric: false, ..Settings::default() };
    sudoku::store::save_json(&sp, &off).unwrap();
    assert!(!start().settings.last_symmetric, "PER-3");
    // A game file of an earlier version: not symmetric. The app keeps the game (PUZ-5).
    let old_game = sudoku::app::new_game(721, 3);
    let json = serde_json::to_string(&old_game).unwrap().replace(r#""symmetric":false,"#, "");
    assert!(!json.contains("symmetric"));
    std::fs::write(&gp, &json).unwrap();
    for _ in 0..2 {
        let app = start();
        assert_eq!(app.game, old_game, "PER-9: the game of an earlier version is not symmetric");
        assert!(!app.game.symmetric);
    }
    // A symmetric game keeps its symmetry (PER-1).
    let sym_game = sudoku::app::new_game_with_symmetry(722, 3, true);
    sudoku::store::save_json(&gp, &sym_game).unwrap();
    assert_eq!(start().game, sym_game, "PER-1");
    // The symmetric puzzle with symmetric = false, and the non-symmetric puzzle with symmetric = true:
    // not the puzzle of the generator, thus the app makes a new game (PUZ-5, PER-5).
    for wrong in [Game { symmetric: false, ..sym_game.clone() }, Game { symmetric: true, ..old_game.clone() }] {
        sudoku::store::save_json(&gp, &wrong).unwrap();
        let app = start();
        assert_ne!(app.game.seed, wrong.seed, "PER-5, PUZ-5 (symmetric {})", wrong.symmetric);
        assert_eq!(app.game.symmetric, false, "DIF-5: the new game has last_symmetric");
    }
    // A symmetric game of version 1 (the puzzle of version 1 of this seed): the app does not use it.
    let p1 = sudoku::generator::generate_version(1, 723, 2, false);
    let v1 = Game { generator: 1, ..Game::new(723, 2, p1.solution, p1.givens) };
    sudoku::store::save_json(&gp, &v1).unwrap();
    assert_eq!(start().game, v1, "PUZ-5: the app keeps a game of version 1");
    sudoku::store::save_json(&gp, &Game { symmetric: true, ..v1.clone() }).unwrap();
    assert_ne!(start().game.seed, 723, "PUZ-8, PER-5: symmetric with version 1");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_results_keep_the_symmetry_and_an_earlier_database_gets_false() {
    // ANA-1, PER-9, CMP-5, ANA-4: a results file of version 2 (no symmetric column) opens with all rows
    // not symmetric, with the same ids and values. A new result keeps its symmetry, also after the app
    // opens the file again. The averages do not use the symmetry.
    let dir = std::env::temp_dir().join(format!("sudoku-symmetry-db-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(sudoku::store::RESULTS_FILE);
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE results (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                finished_at INTEGER NOT NULL,
                mode        INTEGER NOT NULL CHECK (mode BETWEEN 0 AND 6),
                givens      INTEGER NOT NULL CHECK (givens BETWEEN 0 AND 81),
                seconds     INTEGER NOT NULL CHECK (seconds >= 0),
                seed        INTEGER NOT NULL,
                generator   INTEGER NOT NULL CHECK (generator >= 1)
            );
            INSERT INTO results VALUES (4, 100, 2, 30, 200, 41, 2), (9, 200, 6, 24, 900, 42, 2), (12, 300, 2, 30, 400, 43, 1);
            PRAGMA user_version = 2;",
        )
        .unwrap();
    }
    let mut db = ResultsDb::open(&path).unwrap();
    let rows: Vec<(i64, u8, u64, u64, bool)> = db.all().unwrap().iter().map(|r| (r.id, r.mode, r.seconds, r.seed, r.symmetric)).collect();
    assert_eq!(rows.len(), 3, "ANA-1: the app uses all rows");
    for want in [(4, 2, 200, 41, false), (9, 6, 900, 42, false), (12, 2, 400, 43, false)] {
        assert!(rows.contains(&want), "ANA-1, PER-9: {want:?} in {rows:?}");
    }
    let new = db
        .insert(&sudoku::store::NewResult { finished_at: 400, mode: 2, givens: 30, seconds: 600, seed: 44, generator: 2, symmetric: true })
        .unwrap();
    assert!(new.symmetric && new.id > 12, "CMP-5");
    drop(db);
    for _ in 0..2 {
        let db = ResultsDb::open(&path).unwrap();
        let all = db.all().unwrap();
        assert_eq!(all.len(), 4, "the migration is done one time");
        assert_eq!(all.iter().filter(|r| r.symmetric).map(|r| r.seed).collect::<Vec<_>>(), [44], "ANA-1, PER-9");
        // ANA-4: the mode alone sets the group (200, 400 and 600 s at mode 2).
        assert_eq!(sudoku::app::average_seconds(all.iter().filter(|r| r.mode == 2)), Some(400), "ANA-4");
    }
    // The completion of a symmetric game in the app writes true (CMP-5).
    let game = sudoku::app::new_game_with_symmetry(724, 1, true);
    let app = App::new(Some(game), None, ResultsDb::open(&path).unwrap(), Box::new(TestEnv { time: START_TIME, seed: 724 }), App::placeholder_ng_stops());
    let mut d = Driver::from(app);
    complete_with_the_pen(&mut d);
    let row = d.app.results.iter().find(|r| r.seed == 724).expect("CMP-5");
    assert!(row.symmetric, "CMP-5");
    drop(d);
    let again = ResultsDb::open(&path).unwrap().all().unwrap();
    assert_eq!(again.iter().filter(|r| r.symmetric).count(), 2, "CMP-5: the row is in the file");
    std::fs::remove_dir_all(&dir).ok();
}
