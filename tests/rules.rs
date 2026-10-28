//! Tests for the rules of INVARIANTS.md that the other tests do not name. `traceability` makes sure that
//! each rule of INVARIANTS.md has a test or a check.

mod common;

use std::path::Path;

use common::*;
use sudoku::app::{Input, Panel, Screen, Tool};
use sudoku::font::Fonts;
use sudoku::game::{cells, mark_bit, peers, Cell};
use sudoku::layout::{self, Button};
use sudoku::reference;
use sudoku::render::rgb_at;
use sudoku::settings::{Color, Section};

fn fonts() -> Fonts {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    Fonts::from_files(&root.join("scripts/fonts/rmsans-400.ttf"), &root.join("scripts/fonts/rmsans-500.ttf")).unwrap()
}

fn empty_cell(d: &Driver) -> Cell {
    cells().find(|&c| !d.app.game.is_filled(c)).unwrap()
}

#[test]
fn traceability() {
    // Each rule of INVARIANTS.md is named in a test or in src/invariants.rs.
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let rules = std::fs::read_to_string(root.join("INVARIANTS.md")).unwrap();
    let mut text = std::fs::read_to_string(root.join("src/invariants.rs")).unwrap();
    for dir in ["tests", "src"] {
        for e in std::fs::read_dir(root.join(dir)).unwrap().flatten() {
            if e.path().extension().is_some_and(|x| x == "rs") {
                text += &std::fs::read_to_string(e.path()).unwrap();
            }
        }
    }
    let ids: Vec<&str> = rules
        .lines()
        .filter_map(|l| l.strip_prefix("| "))
        .filter_map(|l| l.split(' ').next())
        .filter(|id| id.len() >= 5 && id.as_bytes()[3] == b'-' && id[..3].bytes().all(|b| b.is_ascii_uppercase()))
        .collect();
    assert!(ids.len() > 100, "{} rules", ids.len());
    let missing: Vec<&&str> = ids
        .iter()
        .filter(|id| {
            !text.match_indices(**id).any(|(i, _)| !text[i + id.len()..].starts_with(|c: char| c.is_ascii_digit()))
        })
        .collect();
    assert!(missing.is_empty(), "rules without a test or a check: {missing:?}");
}

#[test]
fn starting_numbers_do_not_change() {
    // PUZ-4: `given` changes only when a new puzzle replaces the game.
    let mut d = Driver::new(31);
    let given = d.app.game.given;
    let cell = empty_cell(&d);
    d.tap_micro(cell, 2);
    d.write(cell, 3);
    d.erase(cell);
    d.button(Button::Menu);
    d.button(Button::Undo);
    d.button(Button::Redo);
    assert_eq!(d.app.game.given, given);
}

#[test]
fn a_mark_for_a_digit_in_a_peer_stays() {
    // MRK-7: the app removes marks only when you add a number.
    let mut d = Driver::new(32);
    let given = cells().find(|&c| d.app.game.is_given(c)).unwrap();
    let v = d.app.game.value_at(given);
    let cell = peers(given).find(|&p| !d.app.game.is_filled(p)).unwrap();
    d.tap_micro(cell, v);
    assert_ne!(d.app.game.marks_at(cell) & mark_bit(v), 0);
    d.wait(5000);
    assert_ne!(d.app.game.marks_at(cell) & mark_bit(v), 0);
}

#[test]
fn an_incorrect_number_without_a_clash_looks_like_a_correct_number() {
    // CLS-4, BRD-5: the app never compares the board with the solution for the player.
    let fonts = fonts();
    let mut g = reference::sample_game();
    // Find an empty cell and a wrong digit that no peer has.
    let (cell, wrong) = cells()
        .filter(|&c| !g.is_filled(c))
        .find_map(|c| (1..=9u8).find(|&v| v != g.solution[c.0 as usize][c.1 as usize] && !peers(c).any(|p| g.value_at(p) == v)).map(|v| (c, v)))
        .unwrap();
    g.add_number(cell, wrong);
    assert!(!g.is_clashing(cell));
    let mut app = reference::app(&fonts);
    app.game = g;
    let mut pm = tiny_skia::Pixmap::new(1620, 2160).unwrap();
    sudoku::render::draw(&mut sudoku::render::Canvas::new(&mut pm, &fonts), &app);
    let (x, y) = layout::cell_origin(cell);
    assert_eq!(rgb_at(&pm, x + 8, y + 140), Color::White.rgb(), "no color for an incorrect number");
    // The digit has the color of your numbers (gray35).
    let darkest = (x + 20..x + 127).flat_map(|px| (y + 20..y + 127).map(move |py| (px, py))).map(|(px, py)| rgb_at(&pm, px, py)[0]).min().unwrap();
    assert_eq!(darkest, sudoku::render::gray(35)[0]);
}

#[test]
fn a_tap_on_a_number_changes_only_the_highlight() {
    // INP-4
    let mut d = Driver::new(33);
    let given = cells().find(|&c| d.app.game.is_given(c)).unwrap();
    let before = (d.app.game.value, d.app.game.marks, d.app.game.undo.len());
    d.tap_micro(given, 1);
    d.tap_micro(given, 9);
    assert_eq!((d.app.game.value, d.app.game.marks, d.app.game.undo.len()), before);
}

#[test]
fn undo_and_redo_do_not_change_the_puzzle_or_the_clock() {
    // UND-9
    let mut d = Driver::new(34);
    let cell = empty_cell(&d);
    d.write(cell, 2);
    d.button(Button::Menu);
    let key = |d: &Driver| (d.app.game.given, d.app.game.solution, d.app.game.seed, d.app.game.mode);
    let k = key(&d);
    let t = d.app.game.elapsed_ms;
    d.app.game.undo();
    d.app.game.redo();
    assert_eq!(key(&d), k);
    assert_eq!(d.app.game.elapsed_ms, t, "only time changes the clock");
}

#[test]
fn the_clock_does_not_show_during_the_game() {
    // CLK-4
    let fonts = fonts();
    let a = reference::render(&fonts, |a| a.game = reference::sample_game());
    let b = reference::render(&fonts, |a| {
        a.game = reference::sample_game();
        a.game.elapsed_ms = 3_725_000;
    });
    assert!(a.data() == b.data(), "a different clock gives the same screen");
}

#[test]
fn the_selected_color_of_each_section_has_the_black_box() {
    // COL-2
    let fonts = fonts();
    let pm = reference::render(&fonts, |a| {
        a.game = reference::sample_game();
        a.settings.color_starting = Color::Blue;
        a.settings.color_clash = Color::White;
        a.settings.color_highlight = Color::Magenta;
        a.ui.menu_open = true;
        a.ui.panel = Panel::Colors;
    });
    let selected = [Color::Blue, Color::White, Color::Magenta];
    for (s, _) in Section::ALL.iter().enumerate() {
        for (i, &c) in Color::ALL.iter().enumerate() {
            let (gx, gy) = layout::color_cell(s, i);
            let corner = rgb_at(&pm, gx + 4, gy + 4);
            assert_eq!(corner == [0, 0, 0], c == selected[s], "section {s}, color {c:?}");
        }
    }
}

#[test]
fn back_is_on_each_manual_page() {
    // MAN-3
    for page in 1..=9u8 {
        let mut d = Driver::new(35);
        d.button(Button::Menu);
        d.button(Button::Help);
        for _ in 1..page {
            d.swipe(true);
        }
        assert_eq!(d.app.manual_page, page);
        d.finger_tap(40.0, 50.0);
        assert_eq!(d.app.screen, Screen::Game, "page {page}");
    }
}

#[test]
fn analytics_does_not_change_the_game_and_the_clock_continues() {
    // ANA-10
    let mut d = Driver::new(36);
    let cell = empty_cell(&d);
    d.tap_micro(cell, 4);
    let before = (d.app.game.value, d.app.game.marks, d.app.game.undo.len(), d.app.game.redo.len());
    let t = d.app.game.elapsed_ms;
    d.button(Button::Menu);
    d.button(Button::Statistics);
    let all = layout::all_games_button();
    d.finger_tap((all.x0 + 50) as f32, (all.y0 + 20) as f32);
    d.wait(3000);
    d.finger_tap(40.0, 50.0);
    d.finger_tap(40.0, 50.0);
    assert_eq!(d.app.screen, Screen::Game);
    assert_eq!((d.app.game.value, d.app.game.marks, d.app.game.undo.len(), d.app.game.redo.len()), before);
    assert!(d.app.game.elapsed_ms >= t + 2900);
}

#[test]
fn dates_are_shown_in_local_time() {
    // ANA-11: finished_at is Unix time (UTC). With TZ=UTC, the local time is UTC.
    reference::use_utc();
    assert_eq!(sudoku::render::format_date(1_792_693_500), "22 Oct 2026, 18:25");
    assert_eq!(sudoku::render::format_date(0), "1 Jan 1970, 00:00");
}

#[test]
fn each_change_marks_the_data_for_saving() {
    // PER-7: the platform saves the game after each change and the settings after each change.
    let mut d = Driver::new(37);
    d.app.game_dirty = false;
    d.tap_micro(empty_cell(&d), 5);
    assert!(d.app.game_dirty);
    d.app.game_dirty = false;
    d.button(Button::Menu);
    d.button(Button::Colors);
    assert!(!d.app.game_dirty, "a panel is not a change of the game");
    let (gx, gy) = layout::color_cell(0, 2);
    d.finger_tap(gx as f32 + 56.0, gy as f32 + 40.0);
    assert!(d.app.settings_dirty);
    // A pen stroke that is not complete is not saved (PER-6); suspend() removes it.
    d.button(Button::Colors);
    let cell = empty_cell(&d);
    let (x, y) = micro_center(cell, 1);
    d.send(Input::PenDown { x, y, tool: Tool::Pen });
    d.send(Input::PenMove { x: x + 60.0, y: y + 60.0 });
    d.app.suspend();
    assert!(d.app.ui.pending_ink.is_none() && d.app.game_dirty);
}

#[test]
fn the_landscape_layout_operates() {
    // The same behavior in landscape: each control reacts where it shows (layout.rs).
    layout::set_orientation(layout::Orientation::Landscape);
    let mut d = Driver::new(38);
    d.app.ng_stops = sudoku::render::ng_stops(&fonts());
    let cell = empty_cell(&d);
    d.tap_micro(cell, 9);
    assert_eq!(d.app.game.marks_at(cell), mark_bit(9));
    d.write(cell, 6);
    assert_eq!(d.app.game.value_at(cell), 6);
    d.button(Button::Menu);
    d.button(Button::Undo);
    assert_eq!(d.app.game.value_at(cell), 0);
    d.button(Button::Colors);
    assert_eq!(d.app.ui.panel, Panel::Colors);
    let (gx, gy) = layout::color_cell(1, 6);
    d.finger_tap(gx as f32 + 56.0, gy as f32 + 40.0);
    assert_eq!(d.app.settings.color_clash, Color::Cyan);
    d.button(Button::NewGame);
    d.finger_tap(d.app.ng_stops[0], layout::ng_slider_y() as f32);
    d.finger_tap(layout::ng_pl() as f32 + 50.0, layout::ng_start_y() as f32);
    assert_eq!((d.app.game.mode, d.app.ui.menu_open), (0, false));
    // Manual: the arrows at the bottom center.
    d.button(Button::Menu);
    d.button(Button::Help);
    let n = layout::next_arrow();
    d.finger_tap(((n.x0 + n.x1) / 2) as f32, ((n.y0 + n.y1) / 2) as f32);
    assert_eq!(d.app.manual_page, 2);
    d.finger_tap(40.0, 50.0);
    // Statistics and the list in one table across the full width.
    solve(&mut d);
    d.button(Button::Menu);
    d.button(Button::Statistics);
    let all = layout::all_games_button();
    d.finger_tap((all.x1 - 30) as f32, (all.y0 + 20) as f32);
    assert_eq!(d.app.screen, Screen::GamesList);
    let seg = layout::filter_segment(1);
    d.finger_tap((seg.x0 + seg.x1) as f32 / 2.0, (seg.y0 + seg.y1) as f32 / 2.0);
    assert_eq!(d.app.list.filter, Some(0));
    let bin = layout::bin_button(0);
    d.finger_tap((bin.x0 + bin.x1) as f32 / 2.0, (bin.y0 + bin.y1) as f32 / 2.0);
    assert!(d.app.list.confirm.is_some());
    let del = layout::dialog_delete();
    d.finger_tap((del.x0 + del.x1) as f32 / 2.0, (del.y0 + del.y1) as f32 / 2.0);
    assert!(d.app.results.is_empty());
    layout::set_orientation(layout::Orientation::Portrait);
}

#[test]
fn turning_the_tablet_keeps_the_game() {
    // NAV-13: a rotation removes only the ink that waits and the touches in progress.
    let mut d = Driver::new(39);
    let cell = empty_cell(&d);
    d.tap_micro(cell, 3);
    d.write_strokes(&digit_ink(cell, 7));
    let before = (d.app.game.value, d.app.game.marks, d.app.game.undo.len(), d.app.game.seed);
    layout::set_orientation(layout::Orientation::Landscape);
    let stops = sudoku::render::ng_stops(&fonts());
    d.app.rotate(stops);
    assert!(d.app.ui.pending_ink.is_none() && d.app.redraw);
    assert_eq!(d.app.ng_stops, stops, "the slider stops of the landscape layout");
    assert_eq!((d.app.game.value, d.app.game.marks, d.app.game.undo.len(), d.app.game.seed), before);
    sudoku::invariants::check_app(&d.app).unwrap();
    layout::set_orientation(layout::Orientation::Portrait);
}

// ---- Tests from the audit of INVARIANTS.md (one test for each rule that had no real test).

#[test]
fn undo_and_redo_do_not_put_a_number_under_the_question_mark() {
    // QST-1, QST-4: the press on Undo or Redo reads the waiting ink and can show the "?". If the undo or
    // the redo then puts a number in that cell, the "?" must go.
    let mut d = Driver::new(40);
    let cell = empty_cell(&d);
    d.button(Button::Menu);
    d.write(cell, 7);
    assert_eq!(d.app.game.value_at(cell), 7);
    d.erase(cell);
    // Undo of the erasure.
    d.write_strokes(&scribble_ink(cell));
    d.button(Button::Undo);
    assert_eq!(d.app.game.value_at(cell), 7);
    assert_eq!(d.app.ui.question, None, "QST-1 after undo");
    sudoku::invariants::check_app(&d.app).unwrap();
    // Redo of the number.
    d.button(Button::Undo);
    d.button(Button::Undo);
    assert_eq!(d.app.game.value_at(cell), 0);
    d.write_strokes(&scribble_ink(cell));
    d.button(Button::Redo);
    assert_eq!(d.app.game.value_at(cell), 7);
    assert_eq!(d.app.ui.question, None, "QST-1 after redo");
    sudoku::invariants::check_app(&d.app).unwrap();
}

#[test]
fn a_second_finger_removes_the_highlight_and_the_question_mark() {
    // HLT-2, QST-4: each finger touch is an action, also when a different finger (for example a palm)
    // is already on the screen.
    let mut d = Driver::new(41);
    let palm = empty_cell(&d);
    let (px, py) = micro_center(palm, 5);
    d.send(Input::TouchDown { id: 5, x: px, y: py });
    let given = cells().find(|&c| d.app.game.is_given(c)).unwrap();
    d.tap_cell(given);
    assert!(d.app.ui.highlight.is_some());
    d.finger_tap(5.0, 1000.0);
    assert_eq!(d.app.ui.highlight, None, "HLT-2");
    let cell = cells().find(|&c| !d.app.game.is_filled(c) && c != palm).unwrap();
    d.write_strokes(&scribble_ink(cell));
    d.wait(d.app.pause_ms + 10);
    assert!(d.app.ui.question.is_some());
    d.finger_tap(5.0, 1000.0);
    assert_eq!(d.app.ui.question, None, "QST-4");
    d.send(Input::TouchUp { id: 5, x: px, y: py });
}

#[test]
fn the_menu_button_and_the_close_button_only_close_a_panel() {
    // NAV-12
    let mut d = Driver::new(42);
    d.button(Button::Menu);
    for panel in [Button::Colors, Button::NewGame] {
        d.button(panel);
        assert_ne!(d.app.ui.panel, Panel::None);
        d.button(Button::Menu);
        assert_eq!(d.app.ui.panel, Panel::None);
        assert!(d.app.ui.menu_open, "the menu stays open");
        d.button(panel);
        d.button(Button::Close);
        assert_eq!(d.app.ui.panel, Panel::None);
        assert!(d.app.ui.menu_open && !d.app.quit, "the app does not close");
    }
}

#[test]
fn a_number_removes_the_hidden_marks_of_filled_peers() {
    // MRK-4: also the hidden marks of a peer with a number lose the digit.
    let mut d = Driver::new(43);
    let a = empty_cell(&d);
    let p = peers(a).find(|&c| !d.app.game.is_filled(c)).unwrap();
    d.tap_micro(p, 7);
    d.tap_micro(p, 2);
    d.write(p, 3);
    assert_eq!(d.app.game.value_at(p), 3);
    d.write(a, 7);
    assert_eq!(d.app.game.value_at(a), 7);
    assert!(peers(a).all(|c| d.app.game.marks_at(c) & mark_bit(7) == 0), "no peer has the mark 7");
    // The erasure shows the hidden marks without the removed mark.
    d.erase(p);
    assert_eq!(d.app.game.marks_at(p), mark_bit(2));
}

#[test]
fn a_press_on_a_button_reads_the_waiting_ink() {
    // INP-12: a press on a different item reads the waiting ink immediately.
    let mut d = Driver::new(44);
    let cell = empty_cell(&d);
    d.write_strokes(&digit_ink(cell, 6));
    assert!(d.app.ui.pending_ink.is_some());
    let (x, y) = button_center(Button::Menu);
    d.send(Input::TouchDown { id: 1, x, y });
    assert!(d.app.ui.pending_ink.is_none());
    assert_eq!(d.app.game.value_at(cell), 6, "read at the press, before the pause");
    d.send(Input::TouchUp { id: 1, x, y });
    assert!(d.app.ui.menu_open);
}

#[test]
fn the_meter_shows_the_mode_of_the_game() {
    // DIF-6: the large knob of the meter is at the stop of `game.mode`.
    let fonts = fonts();
    let (we, wi) = (fonts.medium.width(25.0, "Easy"), fonts.medium.width(25.0, "Hard"));
    let stops = layout::slider_stops(layout::meter_xl(), layout::meter_xr(), we, wi);
    let y = layout::meter_y() - 8;
    for mode in 0..5u8 {
        let mut app = reference::app(&fonts);
        app.game = sudoku::app::new_game(9, mode);
        app.ui.slider = (mode + 2) % 5; // The slider of the panel does not change the meter.
        let mut pm = tiny_skia::Pixmap::new(1620, 2160).unwrap();
        sudoku::render::draw(&mut sudoku::render::Canvas::new(&mut pm, &fonts), &app);
        for (i, &x) in stops.iter().enumerate() {
            let dark = rgb_at(&pm, x.round() as i32, y)[0] < 100;
            assert_eq!(dark, i == mode as usize, "mode {mode}, stop {i}");
        }
    }
}

#[test]
fn colors_and_analytics_operate_on_a_complete_game() {
    // CMP-6: only Start new game ends the complete state. The other menu items continue to operate.
    let mut d = Driver::new(45);
    solve(&mut d);
    assert!(d.app.game.completed);
    let g = d.app.game.clone();
    d.button(Button::Menu);
    d.button(Button::Colors);
    assert_eq!(d.app.ui.panel, Panel::Colors);
    let (gx, gy) = layout::color_cell(0, 3);
    d.finger_tap(gx as f32 + 56.0, gy as f32 + 40.0);
    assert_eq!(d.app.settings.color_starting, Color::Blue);
    d.button(Button::Colors);
    d.button(Button::Statistics);
    assert_eq!(d.app.screen, Screen::Statistics);
    d.finger_tap(40.0, 50.0);
    assert_eq!(d.app.screen, Screen::Game);
    assert_eq!(d.app.game, g, "the game stays complete and does not change");
}

#[test]
fn the_manual_does_not_change_the_game() {
    // MAN-4: pen, eraser and finger input on the manual do not change the game. The clock operates.
    let mut d = Driver::new(46);
    let cell = empty_cell(&d);
    d.tap_micro(cell, 4);
    d.write(cells().filter(|&c| !d.app.game.is_filled(c)).nth(1).unwrap(), 2);
    let key = |d: &Driver| (d.app.game.value, d.app.game.marks, d.app.game.undo.clone(), d.app.game.redo.clone());
    let before = key(&d);
    let t = d.app.game.elapsed_ms;
    d.button(Button::Menu);
    d.button(Button::Help);
    for c in [cell, (4, 4), (8, 8)] {
        d.tap_micro(c, 4);
        d.write_strokes(&digit_ink(c, 5));
        d.wait(d.app.pause_ms + 10);
        d.erase(c);
        let (x, y) = micro_center(c, 5);
        d.finger_tap(x, y);
    }
    d.swipe(true);
    d.swipe(false);
    d.finger_tap(40.0, 50.0);
    assert_eq!(d.app.screen, Screen::Game);
    assert_eq!(key(&d), before);
    assert!(d.app.game.elapsed_ms > t);
}

#[test]
fn black_is_not_in_the_palette() {
    // COL-6, COL-1: nine different colors, none of them black.
    let rgbs: Vec<[u8; 3]> = Color::ALL.iter().map(|c| c.rgb()).collect();
    assert!(rgbs.iter().all(|&c| c != [0, 0, 0]));
    assert!(rgbs.iter().enumerate().all(|(i, a)| rgbs[i + 1..].iter().all(|b| a != b)));
    let names: Vec<&str> = Color::ALL.iter().map(|c| c.name()).collect();
    assert_eq!(names, ["Gray", "White", "Yellow", "Blue", "Red", "Green", "Cyan", "Magenta", "Orange"]);
}

#[test]
fn averages_of_the_summary() {
    // ANA-3, ANA-4: the average of `seconds`, or none ("–") if there are no results. The quantities
    // of all modes add up to the total.
    use sudoku::app::average_seconds;
    use sudoku::store::ResultRow;
    let row = |id, mode, seconds| ResultRow { id, finished_at: id, mode, givens: 30, seconds, seed: 0 };
    let rows = [row(1, 0, 100), row(2, 0, 200), row(3, 3, 61)];
    assert_eq!(average_seconds([].iter()), None);
    assert_eq!(average_seconds(rows.iter()), Some(120));
    assert_eq!(average_seconds(rows.iter().filter(|r| r.mode == 0)), Some(150));
    assert_eq!(average_seconds(rows.iter().filter(|r| r.mode == 1)), None);
    let d = Driver::from(reference::app(&fonts()));
    let per_mode: usize = (0..5u8).map(|m| d.app.results.iter().filter(|r| r.mode == m).count()).sum();
    assert_eq!(per_mode, d.app.results.len());
}

#[test]
fn the_clash_color_comes_before_the_highlight_color() {
    // BRD-7: a clashing number that is also highlighted has the clash color.
    let fonts = fonts();
    let g = reference::sample_game();
    let mut app = reference::app(&fonts);
    app.game = g.clone();
    app.ui.highlight = Some(9);
    let mut pm = tiny_skia::Pixmap::new(1620, 2160).unwrap();
    sudoku::render::draw(&mut sudoku::render::Canvas::new(&mut pm, &fonts), &app);
    let nines: Vec<Cell> = cells().filter(|&c| g.value_at(c) == 9).collect();
    assert!(nines.iter().any(|&c| g.is_clashing(c)) && nines.iter().any(|&c| !g.is_clashing(c)));
    for c in nines {
        let (x, y) = layout::cell_origin(c);
        let want = if g.is_clashing(c) { Color::Red } else { Color::Green };
        assert_eq!(rgb_at(&pm, x + 8, y + 140), want.rgb(), "cell {c:?}");
    }
}

#[test]
fn a_touch_without_a_release_does_not_stop_the_next_touches() {
    // INP-15: on the device, AppLoad did not send the release of touch 1384; then all finger presses
    // had no effect. The log of the device: a press and four moves of 1384, no release, then new
    // touches.
    let mut d = Driver::new(40);
    d.send(Input::TouchDown { id: 1384, x: 800.0, y: 1900.0 });
    for k in 0..4 {
        d.send(Input::TouchMove { id: 1384, x: 800.0 + k as f32, y: 1900.0 });
    }
    // No release of 1384. After a short time, a finger taps the menu button.
    d.wait(300);
    let (x, y) = button_center(Button::Menu);
    d.send(Input::TouchDown { id: 1387, x, y });
    d.send(Input::TouchUp { id: 1387, x, y });
    assert!(d.app.ui.menu_open, "the tap operates");
    // AppLoad also sends the press of the same touch again: the touch starts again.
    d.send(Input::TouchDown { id: 1390, x, y });
    d.send(Input::TouchDown { id: 1390, x, y });
    d.send(Input::TouchUp { id: 1390, x, y });
    assert!(!d.app.ui.menu_open, "one tap, not two");
}

fn switch_point(i: usize) -> (f32, f32) {
    let r = layout::settings_row(i);
    ((r.x0 + r.x1) as f32 / 2.0, (r.y0 + r.y1) as f32 / 2.0)
}

fn open_settings(d: &mut Driver) {
    if !d.app.ui.menu_open {
        d.button(Button::Menu);
    }
    d.button(Button::Settings);
    assert_eq!(d.app.ui.panel, Panel::Settings);
}

#[test]
fn switches_change_only_the_settings() {
    // NAV-14, UND-2, PER-3: a switch changes its setting immediately; the panel stays open; the game
    // and the undo history do not change; the setting is kept for the next start.
    let mut d = Driver::new(41);
    d.tap_micro(empty_cell(&d), 2);
    let before = (d.app.game.value, d.app.game.marks, d.app.game.undo.len(), d.app.game.redo.len());
    open_settings(&mut d);
    let (x, y) = switch_point(0);
    d.finger_tap(x, y);
    assert!(!d.app.settings.highlight_on && d.app.settings.auto_remove_on);
    let (x, y) = switch_point(1);
    d.pen_tap(x, y);
    assert!(!d.app.settings.auto_remove_on);
    assert_eq!(d.app.ui.panel, Panel::Settings, "the panel stays open");
    assert!(d.app.settings_dirty);
    assert_eq!((d.app.game.value, d.app.game.marks, d.app.game.undo.len(), d.app.game.redo.len()), before);
    let saved: sudoku::settings::Settings = serde_json::from_str(&serde_json::to_string(&d.app.settings).unwrap()).unwrap();
    assert_eq!(saved, d.app.settings);
    // The gear and the other panel buttons change the panel (NAV-12).
    d.button(Button::Colors);
    assert_eq!(d.app.ui.panel, Panel::Colors);
    d.button(Button::Settings);
    assert_eq!(d.app.ui.panel, Panel::Settings);
    d.button(Button::Settings);
    assert_eq!(d.app.ui.panel, Panel::None);
}

#[test]
fn the_version_row_has_no_effect() {
    // NAV-14: a tap on the version row changes no setting, and the panel stays open.
    let mut d = Driver::new(43);
    open_settings(&mut d);
    let before = d.app.settings.clone();
    let (x, y) = switch_point(layout::SETTINGS_ROWS as usize);
    d.finger_tap(x, y);
    d.pen_tap(x, y);
    assert_eq!(d.app.settings, before);
    assert_eq!(d.app.ui.panel, Panel::Settings, "the panel stays open");
    assert!(!d.app.settings_dirty);
}

#[test]
fn no_highlight_when_highlights_are_off() {
    // HLT-6
    let mut d = Driver::new(42);
    open_settings(&mut d);
    let (x, y) = switch_point(0);
    d.finger_tap(x, y);
    d.button(Button::Settings);
    let given = cells().find(|&c| d.app.game.is_given(c)).unwrap();
    d.tap_cell(given);
    assert_eq!(d.app.ui.highlight, None);
    // The tap still is an action: it removes a "?" (QST-4).
    let cell = empty_cell(&d);
    d.write_strokes(&scribble_ink(cell));
    d.wait(d.app.pause_ms + 10);
    assert!(d.app.ui.question.is_some());
    d.tap_cell(given);
    assert!(d.app.ui.question.is_none() && d.app.ui.highlight.is_none());
}

#[test]
fn no_automatic_removal_when_it_is_off() {
    // MRK-9, UND-4: with the switch off, a new number changes no other cell; undo and redo change only
    // that cell. With the switch on again, the next number removes marks (MRK-4).
    let mut d = Driver::new(43);
    let cell = empty_cell(&d);
    let v = d.app.game.solution[cell.0 as usize][cell.1 as usize];
    let peer = peers(cell).find(|&p| !d.app.game.is_filled(p)).unwrap();
    d.tap_micro(peer, v);
    open_settings(&mut d);
    let (x, y) = switch_point(1);
    d.finger_tap(x, y);
    d.button(Button::Settings);
    let marks = d.app.game.marks;
    d.write(cell, v);
    assert_eq!(d.app.game.value_at(cell), v);
    let mut want = marks;
    want[cell.0 as usize][cell.1 as usize] = d.app.game.marks_at(cell);
    assert_eq!(d.app.game.marks, want, "no other cell changes");
    d.button(Button::Undo);
    d.button(Button::Redo);
    assert_eq!(d.app.game.marks, want);
    sudoku::invariants::check_app(&d.app).unwrap();
    // On again: the next number removes the mark of a peer.
    d.button(Button::Undo);
    d.button(Button::Settings);
    let (x, y) = switch_point(1);
    d.finger_tap(x, y);
    d.button(Button::Settings);
    d.write(cell, v);
    assert_eq!(d.app.game.marks_at(peer) & mark_bit(v), 0, "MRK-4 when the switch is on");
}

#[test]
fn delete_all_games() {
    // ANA-12, ANA-2, ANA-8
    let mut d = Driver::new(44);
    solve(&mut d);
    d.button(Button::Menu);
    d.button(Button::NewGame);
    d.finger_tap(layout::ng_pl() as f32 + 50.0, layout::ng_start_y() as f32);
    solve(&mut d);
    assert_eq!(d.app.results.len(), 2);
    d.button(Button::Menu);
    d.button(Button::Statistics);
    let b = layout::delete_all_button();
    let center = |r: layout::Rect| ((r.x0 + r.x1) as f32 / 2.0, (r.y0 + r.y1) as f32 / 2.0);
    let (x, y) = center(b);
    d.finger_tap(x, y);
    assert!(d.app.confirm_all);
    // The dialog accepts only its buttons.
    d.finger_tap(40.0, 50.0);
    assert_eq!(d.app.screen, Screen::Statistics);
    let (cx, cy) = center(layout::dialog_cancel());
    d.finger_tap(cx, cy);
    assert!(!d.app.confirm_all);
    assert_eq!(d.app.results.len(), 2, "Cancel makes no change");
    d.finger_tap(x, y);
    let (dx, dy) = center(layout::dialog_delete());
    d.finger_tap(dx, dy);
    assert!(!d.app.confirm_all && d.app.results.is_empty());
    // With no results, the button has no effect.
    d.finger_tap(x, y);
    assert!(!d.app.confirm_all);
    // The list opens empty on page 1.
    let (ax, ay) = center(layout::all_games_button());
    d.finger_tap(ax, ay);
    assert_eq!((d.app.screen, d.app.list.page, d.app.page_rows().len()), (Screen::GamesList, 1, 0));
}

#[test]
fn delete_all_dialog_text() {
    // REQUIREMENTS.md §13.1: "1 completed game is deleted." for one result.
    let fonts = fonts();
    let mut one = reference::app(&fonts);
    while one.results.len() > 1 {
        let id = one.results[0].id;
        one.results.retain(|r| r.id != id);
    }
    one.screen = Screen::Statistics;
    one.confirm_all = true;
    let mut pm = tiny_skia::Pixmap::new(1620, 2160).unwrap();
    sudoku::render::draw(&mut sudoku::render::Canvas::new(&mut pm, &fonts), &one);
    let mut two = reference::app(&fonts);
    two.screen = Screen::Statistics;
    two.confirm_all = true;
    let mut pm2 = tiny_skia::Pixmap::new(1620, 2160).unwrap();
    sudoku::render::draw(&mut sudoku::render::Canvas::new(&mut pm2, &fonts), &two);
    assert!(pm.data() != pm2.data(), "the text depends on the quantity");
}

// ---- Tests from the final audit (Settings panel, Statistics, INP-15).

/// The screen of an app.
fn draw_app(app: &sudoku::app::App, fonts: &Fonts) -> tiny_skia::Pixmap {
    let mut pm = tiny_skia::Pixmap::new(1620, 2160).unwrap();
    sudoku::render::draw(&mut sudoku::render::Canvas::new(&mut pm, fonts), app);
    pm
}

#[test]
fn a_control_released_after_a_pen_tap_removes_the_highlight() {
    // HLT-2, HLT-4: a finger holds the menu button and the pen taps a number. When the finger lifts,
    // the menu opens. The menu also removes this highlight.
    let mut d = Driver::new(50);
    let (x, y) = button_center(Button::Menu);
    d.send(Input::TouchDown { id: 1, x, y });
    let given = cells().find(|&c| d.app.game.is_given(c)).unwrap();
    d.tap_cell(given);
    assert!(d.app.ui.highlight.is_some());
    d.send(Input::TouchUp { id: 1, x, y });
    assert!(d.app.ui.menu_open);
    assert_eq!(d.app.ui.highlight, None, "HLT-4: the menu opened with a highlight");
}

#[test]
fn undo_with_a_finger_while_the_pen_writes_keeps_no_ink_on_a_number() {
    // INP-5: a finger holds Undo and the pen writes in a cell. The undo puts a number back in that cell.
    // Then no ink can wait in the cell.
    let mut d = Driver::new(51);
    let cell = empty_cell(&d);
    d.button(Button::Menu);
    d.write(cell, 7);
    assert_eq!(d.app.game.value_at(cell), 7);
    d.erase(cell);
    let (x, y) = button_center(Button::Undo);
    d.send(Input::TouchDown { id: 1, x, y });
    d.write_strokes(&digit_ink(cell, 3));
    assert!(d.app.ui.pending_ink.is_some());
    d.send(Input::TouchUp { id: 1, x, y });
    sudoku::invariants::check_app(&d.app).unwrap();
    assert!(d.app.ui.pending_ink.is_none());
}

#[test]
fn a_panel_does_not_open_over_waiting_ink() {
    // NAV-11, INP-12: a finger holds the paint bucket and the pen writes. The release of the finger
    // reads the ink, as a press does. Then a tap outside the panel only closes the panel.
    let mut d = Driver::new(52);
    d.button(Button::Menu);
    let left = |d: &Driver| cells().filter(|&c| !d.app.game.is_filled(c) && c.1 < 4).collect::<Vec<_>>();
    let (cell, other) = (left(&d)[0], left(&d)[1]);
    let (x, y) = button_center(Button::Colors);
    d.send(Input::TouchDown { id: 1, x, y });
    d.write_strokes(&digit_ink(cell, 6));
    d.send(Input::TouchUp { id: 1, x, y });
    assert_eq!(d.app.ui.panel, Panel::Colors);
    assert!(d.app.ui.pending_ink.is_none(), "the release reads the ink");
    let g = d.app.game.clone();
    d.tap_micro(other, 1);
    assert_eq!((d.app.ui.panel, d.app.ui.menu_open), (Panel::None, true));
    assert_eq!((d.app.game.value, d.app.game.marks, d.app.game.undo.len()), (g.value, g.marks, g.undo.len()));
}

#[test]
fn a_pen_touch_is_a_tap_only_if_it_stays_in_one_micro_cell() {
    // INP-2: a movement in the micro-cell is a tap. A movement out of it and back is writing.
    let mut d = Driver::new(53);
    let cell = empty_cell(&d);
    let (x, y) = micro_center(cell, 5);
    d.send(Input::PenDown { x, y, tool: Tool::Pen });
    d.send(Input::PenMove { x: x + 10.0, y: y - 10.0 });
    d.send(Input::PenUp { x: x - 10.0, y: y + 10.0 });
    assert_eq!(d.app.game.marks_at(cell), mark_bit(5), "a tap");
    d.send(Input::PenDown { x, y, tool: Tool::Pen });
    d.send(Input::PenMove { x: x + 40.0, y });
    d.send(Input::PenUp { x, y });
    assert_eq!(d.app.game.marks_at(cell), mark_bit(5), "writing, not a tap");
    assert!(d.app.ui.pending_ink.is_some());
}

#[test]
fn the_settings_panel_closes_as_the_other_panels() {
    // NAV-11, NAV-12 for the Settings panel: the menu button, the close button and the other toolbar
    // buttons only close it. A tap on the board only closes it.
    let mut d = Driver::new(54);
    d.button(Button::Menu);
    for b in [Button::Menu, Button::Close, Button::Undo, Button::Help, Button::Statistics] {
        d.button(Button::Settings);
        assert_eq!(d.app.ui.panel, Panel::Settings);
        d.button(b);
        assert_eq!(d.app.ui.panel, Panel::None, "{b:?}");
        assert!(d.app.ui.menu_open && !d.app.quit && d.app.screen == Screen::Game, "{b:?}");
    }
    d.button(Button::Settings);
    let cell = cells().find(|&c| !d.app.game.is_filled(c) && c.1 < 4).unwrap();
    let before = (d.app.game.marks, d.app.game.undo.len(), d.app.settings.clone());
    d.tap_micro(cell, 1);
    assert_eq!((d.app.ui.panel, d.app.ui.menu_open), (Panel::None, true));
    assert_eq!((d.app.game.marks, d.app.game.undo.len(), d.app.settings.clone()), before);
}

#[test]
fn a_switch_does_not_change_the_changes_in_the_history() {
    // NAV-14, UND-4: a number that removed marks while the switch was on keeps them in its change.
    // When the switch is off, undo puts the marks back and redo removes them again.
    let mut d = Driver::new(55);
    let cell = empty_cell(&d);
    let v = d.app.game.solution[cell.0 as usize][cell.1 as usize];
    let peer = peers(cell).find(|&p| !d.app.game.is_filled(p)).unwrap();
    d.tap_micro(peer, v);
    d.write(cell, v);
    assert_eq!(d.app.game.marks_at(peer) & mark_bit(v), 0);
    let history = (d.app.game.undo.clone(), d.app.game.redo.clone());
    open_settings(&mut d);
    for i in [1, 0] {
        let (x, y) = switch_point(i);
        d.finger_tap(x, y);
    }
    assert!(!d.app.settings.highlight_on && !d.app.settings.auto_remove_on);
    assert_eq!((d.app.game.undo.clone(), d.app.game.redo.clone()), history);
    d.button(Button::Settings);
    d.button(Button::Undo);
    assert_ne!(d.app.game.marks_at(peer) & mark_bit(v), 0, "undo puts the mark back");
    d.button(Button::Redo);
    assert_eq!(d.app.game.marks_at(peer) & mark_bit(v), 0, "redo removes it again");
}

#[test]
fn the_switches_are_kept_when_the_app_opens_again() {
    // PER-3: the two switches go through the settings file.
    let mut d = Driver::new(56);
    open_settings(&mut d);
    for i in [0, 1] {
        let (x, y) = switch_point(i);
        d.finger_tap(x, y);
    }
    let dir = std::env::temp_dir().join(format!("sudoku-switches-{}", std::process::id()));
    let path = dir.join("settings.json");
    sudoku::store::save_json(&path, &d.app.settings).unwrap();
    let app = sudoku::app::App::new(
        None,
        sudoku::store::load_json(&path),
        sudoku::store::ResultsDb::in_memory(),
        Box::new(TestEnv { time: 0, seed: 1 }),
    );
    std::fs::remove_dir_all(&dir).ok();
    assert!(!app.settings.highlight_on && !app.settings.auto_remove_on);
    assert_eq!(app.settings, d.app.settings);
}

#[test]
fn the_marks_of_a_cell_do_not_show_under_the_question_mark() {
    // QST-1, BRD-6: while the "?" shows, the marks and the micro-cell grid of the cell are hidden.
    // After the "?", the marks show again.
    let fonts = fonts();
    let mut app = reference::app(&fonts);
    app.game = reference::sample_game();
    let cell = cells().find(|&c| !app.game.is_filled(c)).unwrap();
    app.ui.question = Some((cell, 0));
    let without = draw_app(&app, &fonts);
    app.game.marks[cell.0 as usize][cell.1 as usize] = mark_bit(1) | mark_bit(5) | mark_bit(9);
    assert!(draw_app(&app, &fonts).data() == without.data(), "the marks are hidden");
    app.ui.question = None;
    assert!(draw_app(&app, &fonts).data() != without.data(), "the marks show again");
}

#[test]
fn delete_all_games_is_gray_without_results() {
    // ANA-12: "Delete all games" is black if there are results, and gray if there are none.
    let fonts = fonts();
    let darkest = |app: &sudoku::app::App| {
        let pm = draw_app(app, &fonts);
        let b = layout::delete_all_button();
        (b.x0..b.x1).flat_map(|x| (b.y0..b.y1).map(move |y| (x, y))).map(|(x, y)| rgb_at(&pm, x, y)[0]).min().unwrap()
    };
    let mut app = reference::app(&fonts);
    app.screen = Screen::Statistics;
    assert!(!app.results.is_empty());
    assert!(darkest(&app) < 30, "black with results");
    app.results.clear();
    assert!(darkest(&app) > 150, "gray without results");
}

#[test]
fn the_update_button_starts_only_the_installation() {
    // UPD-5: a tap on the button starts one installation. The panel stays open, and the game and the
    // settings do not change. The other parts of the update row do nothing.
    use sudoku::update::{UpdateRequest, UpdateState};
    let mut d = Driver::new(56);
    d.app.update = reference::update_state(true);
    open_settings(&mut d);
    let board = |d: &Driver| (d.app.game.value, d.app.game.marks, d.app.game.undo.len(), d.app.game.redo.len(), d.app.settings.clone());
    let before = board(&d);
    let row = layout::settings_row(layout::UPDATE_ROW);
    d.finger_tap((row.x0 + 10) as f32, (row.y0 + 56) as f32);
    assert!(matches!(d.app.update.state, UpdateState::Available(_)), "a tap next to the button");
    let b = layout::update_button_area();
    let (x, y) = ((b.x0 + b.x1) as f32 / 2.0, (b.y0 + b.y1) as f32 / 2.0);
    d.finger_tap(x, y);
    assert!(matches!(d.app.update.state, UpdateState::Installing(_)));
    assert!(matches!(d.app.update.request.take(), Some(UpdateRequest::Install(m)) if m.version == "v0.2.0"));
    d.finger_tap(x, y);
    assert_eq!(d.app.update.request, None, "one installation at a time");
    assert_eq!((d.app.ui.panel, d.app.screen), (Panel::Settings, Screen::Game));
    assert_eq!(board(&d), before);
    let now = d.now;
    d.app.update_installed(Err("no network".into()), now);
    assert!(matches!(d.app.update.state, UpdateState::Failed(..)));
    d.wait(3000);
    assert!(matches!(d.app.update.state, UpdateState::Available(_)), "UPD-6: the button shows again");
}

#[test]
fn the_settings_panel_tries_a_failed_check_again() {
    // UPD-7: when all requests failed, the Settings panel starts a new request when it opens.
    use sudoku::update::{Update, UpdateRequest, UpdateState};
    let mut d = Driver::new(57);
    d.app.update = Update::new("v0.1.0");
    assert_eq!(d.app.update.request.take(), Some(UpdateRequest::Check));
    let now = d.now;
    d.app.update_checked(Err("no network".into()), now);
    assert_eq!(d.app.update.state, UpdateState::CheckFailed);
    open_settings(&mut d);
    assert_eq!(d.app.update.request.take(), Some(UpdateRequest::Check));
    d.button(Button::Settings);
    d.button(Button::Settings);
    assert_eq!(d.app.update.request, None, "a request is in progress");
}

#[test]
fn the_caution_icon_follows_the_update_state() {
    // UPD-3: the red icon on the gear shows only when an update is available, installs or is installed.
    use sudoku::update::{Update, UpdateState};
    let fonts = fonts();
    let (x, y) = ((layout::tb_cx() + 16.0 - 5.0) as i32, (layout::tb_cy(2) - 16.0) as i32);
    let available = reference::update_state(true);
    let mut installed = available.clone();
    installed.state = UpdateState::Installed;
    for (update, want) in [(available, true), (installed, true), (reference::update_state(false), false), (Update::new("dev"), false)] {
        let state = update.state.clone();
        sudoku::layout::set_right_handed(false);
        let mut a = reference::app(&fonts);
        a.update = update;
        a.ui.menu_open = true;
        let mut pm = tiny_skia::Pixmap::new(layout::screen_w(), layout::screen_h()).unwrap();
        sudoku::render::draw(&mut sudoku::render::Canvas::new(&mut pm, &fonts), &a);
        assert_eq!(rgb_at(&pm, x, y) == sudoku::render::CAUTION_RED, want, "{state:?}");
    }
}
