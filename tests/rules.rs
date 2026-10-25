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
    d.button(Button::Analytics);
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
    // Analytics and the list in two columns.
    solve(&mut d);
    d.button(Button::Menu);
    d.button(Button::Analytics);
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
