//! Tests for the rules of the saved data and the results: PER-1, PER-3, PER-4, PER-5, PER-7, PER-8, CKP-4,
//! CKP-5 and ANA-1. The
//! platform part of PER-7 and CLK-6 (the main loop on Linux) is in tests/qtfb_e2e.rs.
//!
//! `invariants::check_app` runs after each input only in debug builds. These tests assert the rules
//! directly, thus they also examine the release build.

mod common;
mod inv_groups;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use common::*;
use sudoku::app::{new_game, App, Env, Panel, Screen};
use sudoku::game::{cells, mark_bit, peers, Board, Cell, Change, Checkpoint, Game};
use sudoku::generator::{count_solutions, generate_version, FIRST_MODE, MODES, VERSION};
use sudoku::invariants::{check_app, check_game};
use sudoku::layout::{self, Button};
use sudoku::settings::{Color, LineGray, Section, Settings, Switch};
use sudoku::store::{load_json, save_json, ResultsDb, RESULTS_FILE};

// ---- Helpers of this file. The shared helpers are in tests/common.

/// An empty data folder for one test.
fn data_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("sudoku-persist-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The app opens with the saved data, as the platform does at the start. The wall clock of each open
/// is later when `seed` is larger.
fn open_app(game: Option<Game>, settings: Option<Settings>, db: ResultsDb, seed: u64) -> App {
    App::new(game, settings, db, Box::new(TestEnv { time: START_TIME + seed as i64 * 1000, seed }), App::placeholder_ng_stops())
}

/// A game of `seed` and `mode` with all cells correct except one. The numbers go into the undo
/// history in order, as a player adds them. Returns the game and the empty cell.
fn nearly_complete(seed: u64, mode: u8) -> (Game, Cell) {
    let mut g = new_game(seed, mode);
    let empty: Vec<Cell> = cells().filter(|&c| !g.is_filled(c)).collect();
    let (last, rest) = empty.split_last().unwrap();
    for &(r, c) in rest {
        g.add_number((r, c), g.solution[r as usize][c as usize]);
    }
    assert!(!g.completed && check_game(&g).is_ok());
    (g, *last)
}

/// The player writes the last number of the game with the pen. The game becomes complete.
fn write_last_number(d: &mut Driver, last: Cell) {
    let v = d.app.game.solution[last.0 as usize][last.1 as usize];
    d.write(last, v);
    assert!(d.app.game.completed, "the app did not read {v} at {last:?}");
}

/// The seed of the new game that `open_app(.., seed)` makes when it does not use the saved game.
fn first_seed(seed: u64) -> u64 {
    TestEnv { time: 0, seed }.new_seed()
}

// ---- PER-3

#[test]
fn settings_and_last_mode_are_kept_when_the_app_opens_again() {
    // PER-3: the four colors, the micro-cell line gray (COL-7), `last_mode` (from Start new game) and the
    // three switches go through the settings file. DIF-3: the slider opens at the kept mode.
    let mut d = Driver::new(61);
    d.button(Button::Menu);
    d.button(Button::Colors);
    let (x, y) = color_point(Section::Starting, Color::Yellow);
    d.finger_tap(x, y);
    let (x, y) = color_point(Section::Highlight, Color::Magenta);
    d.finger_tap(x, y);
    let (x, y) = color_point(Section::Completed, Color::Orange);
    d.finger_tap(x, y);
    let (x, y) = line_gray_point(LineGray::Gray55);
    d.finger_tap(x, y);
    d.button(Button::Settings);
    for s in Switch::ALL {
        let (x, y) = switch_point(s);
        d.finger_tap(x, y);
    }
    d.button(Button::NewGame);
    d.finger_tap(d.app.ng_stops[4], layout::ng_slider_y() as f32);
    d.finger_tap(layout::ng_pl() as f32 + 50.0, layout::ng_start_y() as f32);
    let s = d.app.settings.clone();
    assert_eq!((s.color_starting, s.color_clash, s.color_highlight, s.color_completed), (Color::Yellow, Color::Red, Color::Magenta, Color::Orange));
    assert_eq!(s.micro_line, LineGray::Gray55);
    assert_eq!(s.last_mode, 4);
    assert!(!s.highlight_on && !s.auto_hide_on && !s.completed_color_on);
    assert!(d.app.settings_dirty, "PER-7: Start new game changes the settings");

    let dir = data_dir("settings");
    let (gp, sp) = (dir.join("game.json"), dir.join("settings.json"));
    save_json(&gp, &d.app.game).unwrap();
    save_json(&sp, &d.app.settings).unwrap();
    let app = open_app(load_json(&gp), load_json(&sp), ResultsDb::in_memory(), 3);
    assert_eq!(app.settings, s);
    assert_eq!(app.ui.slider, 4);
    assert_eq!(app.game, d.app.game);

    // With no saved game, the new game has the kept mode.
    let app = open_app(None, load_json(&sp), ResultsDb::in_memory(), 3);
    assert_eq!(app.settings, s);
    assert_eq!(app.game.mode, 4);
    // Two more opens do not change the settings.
    save_json(&sp, &app.settings).unwrap();
    let again = open_app(None, load_json(&sp), ResultsDb::in_memory(), 4);
    assert_eq!(again.settings, s);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_completed_color_and_its_switch_go_through_the_file() {
    // PER-3, COL-1, COL-11: each color of the Completed groups section, with the switch on and off. The
    // file has the names of INVARIANTS.md §0. The app opens with the same values, and a second save and
    // a second open do not change them.
    let dir = data_dir("completed-round-trip");
    let path = dir.join("settings.json");
    for color in Color::ALL {
        for on in [true, false] {
            let s = Settings {
                color_starting: Color::White,
                color_clash: Color::White,
                color_highlight: Color::White,
                color_completed: color,
                completed_color_on: on,
                ..Settings::default()
            };
            assert!(s.is_valid(), "{color:?}");
            save_json(&path, &s).unwrap();
            let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
            assert_eq!(json["color_completed"], format!("{color:?}"), "PER-3: the name in the file");
            assert_eq!(json["completed_color_on"], on, "PER-3: the name in the file");
            let app = open_app(None, load_json(&path), ResultsDb::in_memory(), 5);
            assert_eq!(app.settings, s, "PER-3: {color:?}, switch {on}");
            save_json(&path, &app.settings).unwrap();
            assert_eq!(load_json::<Settings>(&path), Some(s), "PER-3: the second open, {color:?}, switch {on}");
        }
    }
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_saved_files_do_not_have_the_completed_groups() {
    // COL-9, R§14.3: the app does not save the completed color of a group. The game file has the same
    // names with a completed group and without it, and the same game gives the same groups after an open.
    let (game, row) = inv_groups::game_with_full_row(66, 2);
    assert!(game.is_completed_group(sudoku::game::Group::Row(row)), "the fixture has a completed row");
    let names = |g: &Game| -> HashSet<String> { serde_json::to_value(g).unwrap().as_object().unwrap().keys().cloned().collect() };
    assert_eq!(names(&game), names(&new_game(66, 2)), "COL-9: the saved names");
    assert!(names(&game).iter().all(|n| !n.contains("group") && !n.contains("clash")), "COL-9, CLS-1: {:?}", names(&game));
    let dir = data_dir("no-groups");
    let path = dir.join("game.json");
    save_json(&path, &game).unwrap();
    let app = open_app(load_json(&path), None, ResultsDb::in_memory(), 5);
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(app.game, game, "the app uses the saved game");
    assert_eq!(app.game.completed_cells(), game.completed_cells(), "COL-9");
}

// ---- PER-8

/// A settings file of an earlier version: three colors, and no `color_completed`. The other values are
/// not the defaults, thus the test sees if they stay.
fn old_file(starting: Color, clash: Color, highlight: Color) -> String {
    format!(r#"{{"color_starting":"{starting:?}","color_clash":"{clash:?}","color_highlight":"{highlight:?}","last_mode":3,"highlight_on":false,"auto_remove_on":false,"micro_line":"Gray55"}}"#)
}

/// The app opens with the text of a settings file. Examines that each start gives the same result and
/// that the start does not change the file (PER-8).
fn open_with_file(name: &str, text: &str) -> Settings {
    let dir = data_dir(&format!("per8-{name}"));
    let path = dir.join("settings.json");
    std::fs::write(&path, text).unwrap();
    let app = open_app(None, load_json(&path), ResultsDb::in_memory(), 5);
    assert!(!app.settings_dirty, "PER-8: {name}: the start asks for a save");
    let second = open_app(None, load_json(&path), ResultsDb::in_memory(), 6);
    assert_eq!(app.settings, second.settings, "PER-8: {name}: each start gives the same result");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), text, "PER-8: {name}: the start changed the file");
    std::fs::remove_dir_all(&dir).ok();
    check_app(&app).unwrap_or_else(|e| panic!("{name}: {e}"));
    app.settings
}

/// PER-8, written from INVARIANTS.md: the four colors of a file with no `color_completed`, or None if
/// the file breaks COL-5. The order is the order of the palette (§0), without White.
fn per_8_model(starting: Color, clash: Color, highlight: Color) -> Option<[Color; 4]> {
    use Color::*;
    if (starting != White && (starting == clash || starting == highlight)) || (clash != White && clash == highlight) {
        return None;
    }
    let first_free = |used: &[Color]| [Gray, Yellow, Blue, Red, Green, Cyan, Magenta, Orange].into_iter().find(|c| !used.contains(c)).unwrap();
    let new_highlight = match highlight {
        Green if starting == Cyan || clash == Cyan => first_free(&[starting, clash]),
        Green => Cyan,
        other => other,
    };
    let used = [starting, clash, new_highlight];
    let completed = if used.contains(&Green) { first_free(&used) } else { Green };
    Some([starting, clash, new_highlight, completed])
}

fn colors_of(s: &Settings) -> [Color; 4] {
    [s.color_starting, s.color_clash, s.color_highlight, s.color_completed]
}

#[test]
fn an_earlier_settings_file_gets_the_colors_of_per_8() {
    // PER-8, R§14.2: each case with its values. The switch is on, and the other values stay.
    use Color::*;
    let cases = [
        ("green-highlight", (Gray, Red, Green), [Gray, Red, Cyan, Green]),
        ("green-highlight-starting-cyan", (Cyan, Red, Green), [Cyan, Red, Gray, Green]),
        ("green-highlight-clash-cyan", (Gray, Cyan, Green), [Gray, Cyan, Yellow, Green]),
        ("green-highlight-both-before-cyan", (Yellow, Cyan, Green), [Yellow, Cyan, Gray, Green]),
        ("starting-green", (Green, Red, Cyan), [Green, Red, Cyan, Gray]),
        ("clash-green", (Gray, Green, Cyan), [Gray, Green, Cyan, Yellow]),
        ("starting-green-other-highlight", (Green, Gray, Blue), [Green, Gray, Blue, Yellow]),
        ("white-highlight", (Gray, Red, White), [Gray, Red, White, Green]),
        ("white-highlight-starting-green", (Green, Red, White), [Green, Red, White, Gray]),
        ("other-highlight", (Gray, Red, Magenta), [Gray, Red, Magenta, Green]),
        ("all-white", (White, White, White), [White, White, White, Green]),
        ("white-and-green-highlight", (White, White, Green), [White, White, Cyan, Green]),
    ];
    for (name, (a, b, c), want) in cases {
        let s = open_with_file(name, &old_file(a, b, c));
        assert_eq!(colors_of(&s), want, "PER-8: {name}");
        assert!(s.completed_color_on, "PER-8: {name}: the switch");
        assert_eq!((s.last_mode, s.highlight_on, s.auto_hide_on, s.micro_line), (3, false, false, LineGray::Gray55), "PER-3: {name}: the other values");
    }
}

#[test]
fn each_earlier_settings_file_agrees_with_a_separate_model_of_per_8() {
    // PER-8, COL-1, COL-5: all 729 files of three colors. A file that breaks COL-5 gets the defaults, also
    // the values that are not colors. Each other file has a result that agrees with COL-5.
    for a in Color::ALL {
        for b in Color::ALL {
            for c in Color::ALL {
                let s: Settings = serde_json::from_str(&old_file(a, b, c)).unwrap();
                let what = format!("{a:?} {b:?} {c:?}");
                match per_8_model(a, b, c) {
                    Some(want) => {
                        assert_eq!(colors_of(&s), want, "PER-8: {what}");
                        assert!(s.completed_color_on && s.is_valid(), "PER-8, COL-5: {what}");
                    }
                    None => assert_eq!(s, Settings::default(), "PER-5, PER-8: {what}"),
                }
            }
        }
    }
}

#[test]
fn an_earlier_settings_file_that_breaks_col_5_gets_the_defaults() {
    // PER-8 part 1, PER-5: also when part 2 can remove the break (two Green sections), and when the color
    // in the file is not Green. The app does not keep the values of a file that breaks the rule.
    use Color::*;
    for (name, a, b, c) in [("red-red", Red, Red, Blue), ("two-green", Green, Red, Green), ("clash-highlight-green", Gray, Green, Green), ("two-cyan", Cyan, Cyan, Green)] {
        let s = open_with_file(name, &old_file(a, b, c));
        assert_eq!(s, Settings::default(), "PER-8: {name}");
    }
}

#[test]
fn a_file_with_a_completed_color_does_not_change() {
    // PER-8: a file that has `color_completed` keeps its colors, also with a Green highlight. A file with a
    // color and no value for the switch gets the switch on. A switch in the file stays. PER-5: a file
    // that has `color_completed` and breaks COL-5 gets the defaults.
    use Color::*;
    let file = |completed: Color, switch: &str| {
        format!(r#"{{"color_starting":"Gray","color_clash":"Red","color_highlight":"Green","color_completed":"{completed:?}","last_mode":3{switch}}}"#)
    };
    let s = open_with_file("green-highlight-kept", &file(Blue, r#","completed_color_on":true"#));
    assert_eq!(colors_of(&s), [Gray, Red, Green, Blue], "PER-8: a Green highlight stays");
    assert!(s.completed_color_on);

    let s = open_with_file("switch-off-kept", &file(Blue, r#","completed_color_on":false"#));
    assert_eq!(colors_of(&s), [Gray, Red, Green, Blue], "PER-8");
    assert!(!s.completed_color_on, "PER-3: the switch stays off");

    let s = open_with_file("no-switch", &file(Blue, ""));
    assert_eq!(colors_of(&s), [Gray, Red, Green, Blue], "PER-8: no value for the switch: the colors do not change");
    assert!(s.completed_color_on, "PER-8: no value for the switch: on");

    let s = open_with_file("white-completed", &file(White, ""));
    assert_eq!(colors_of(&s), [Gray, Red, Green, White], "PER-8: White stays");

    let s = open_with_file("completed-breaks-col-5", &file(Red, ""));
    assert_eq!(s, Settings::default(), "PER-5: the completed color and the clash color are the same");
}

#[test]
fn a_switch_in_a_file_with_no_completed_color_does_not_stay_off() {
    // PER-8 part 4: with no `color_completed`, `completed_color_on = true`, also when the file has the
    // value false.
    let text = r#"{"color_starting":"Gray","color_clash":"Red","color_highlight":"Blue","last_mode":3,"completed_color_on":false}"#;
    let s = open_with_file("no-color-switch-off", text);
    assert!(s.completed_color_on, "PER-8");
    assert_eq!(colors_of(&s), [Color::Gray, Color::Red, Color::Blue, Color::Green], "PER-8");
}

#[test]
fn an_earlier_file_changes_only_at_the_next_change_of_the_settings() {
    // PER-8, PER-7: the start does not save. The next change of the settings saves all values, and the
    // next start gives the same settings: the steps of PER-8 do not apply again.
    let dir = data_dir("per8-next-change");
    let path = dir.join("settings.json");
    std::fs::write(&path, old_file(Color::Gray, Color::Red, Color::Green)).unwrap();
    let mut d = Driver::from(open_app(None, load_json(&path), ResultsDb::in_memory(), 5));
    assert!(!d.app.settings_dirty, "PER-8");
    d.button(Button::Menu);
    d.button(Button::Colors);
    assert!(!d.app.settings_dirty, "PER-8: the panels are not a change of the settings");
    let (x, y) = color_point(Section::Completed, Color::Blue);
    d.finger_tap(x, y);
    assert!(d.app.settings_dirty, "PER-7");
    let (x, y) = color_point(Section::Highlight, Color::Green);
    d.finger_tap(x, y);
    assert_eq!(colors_of(&d.app.settings), [Color::Gray, Color::Red, Color::Green, Color::Blue], "COL-3");
    save_json(&path, &d.app.settings).unwrap();
    let app = open_app(None, load_json(&path), ResultsDb::in_memory(), 6);
    assert_eq!(app.settings, d.app.settings, "PER-8: a Green highlight that the player selects stays");
    std::fs::remove_dir_all(&dir).ok();
}

// ---- PER-4

#[test]
fn results_are_kept_when_the_app_opens_again() {
    // PER-4: a result that the app adds stays in the database file. Only ANA-2 changes the results:
    // to open the app does not change them, and a delete is kept.
    let dir = data_dir("results");
    let path = dir.join(RESULTS_FILE);
    let mut saved = Vec::new();
    for (i, mode) in [1u8, 3].into_iter().enumerate() {
        let (g, last) = nearly_complete(700 + i as u64, mode);
        let mut d = Driver::from(open_app(Some(g), None, ResultsDb::open(&path).unwrap(), 70 + i as u64));
        write_last_number(&mut d, last);
        assert_eq!(d.app.results.len(), i + 1);
        saved = d.app.results.clone();
    }
    // The app closes and opens again (with the complete game and with a new game).
    for seed in [80, 81] {
        let app = open_app(None, None, ResultsDb::open(&path).unwrap(), seed);
        assert_eq!(app.results, saved);
        drop(app);
    }
    assert_eq!(ResultsDb::open(&path).unwrap().all().unwrap(), saved);

    // A delete in the games list is kept.
    let mut d = Driver::from(open_app(None, None, ResultsDb::open(&path).unwrap(), 82));
    d.button(Button::Menu);
    d.button(Button::Statistics);
    let (x, y) = exact_center(layout::all_games_button());
    d.finger_tap(x, y);
    let (x, y) = exact_center(layout::bin_button(0));
    d.finger_tap(x, y);
    let (x, y) = exact_center(layout::dialog_delete());
    d.finger_tap(x, y);
    assert_eq!(d.app.results.len(), 1);
    let after = d.app.results.clone();
    drop(d);
    let app = open_app(None, None, ResultsDb::open(&path).unwrap(), 83);
    assert_eq!(app.results, after);
    assert_eq!(after[0], saved[1], "the newest result was deleted, the other result stays");
    std::fs::remove_dir_all(&dir).ok();
}

// ---- PER-5

/// The corrupt game breaks `rule`, the save file keeps it, and the app does not use it: the app makes a
/// new game of the last mode, and the new state agrees with all rules.
fn assert_rejected(base: &Game, bad: &Game, rule: &str, name: &str) {
    let control = open_app(Some(base.clone()), None, ResultsDb::in_memory(), 5);
    assert_eq!(&control.game, base, "{name}: the game before the corruption is correct");
    let err = check_game(bad).expect_err(name);
    assert!(err.contains(rule), "{name}: the corruption must break {rule}, but the check gives: {err}");

    let dir = data_dir(name);
    let path = dir.join("game.json");
    save_json(&path, bad).unwrap();
    let loaded: Game = load_json(&path).expect("the file is JSON of a game");
    assert_eq!(&loaded, bad);
    let app = open_app(Some(loaded), None, ResultsDb::in_memory(), 5);
    std::fs::remove_dir_all(&dir).ok();
    assert_ne!(&app.game, bad, "{name}: the app used a saved game that breaks {rule}");
    assert_eq!(app.game, new_game(first_seed(5), FIRST_MODE), "{name}: a new game of the last mode");
    check_app(&app).unwrap_or_else(|e| panic!("{name}: {e}"));
}

/// A game with one mark, one number and one erased number in its undo history, and one redo item.
fn base_game() -> Game {
    let mut g = new_game(901, 2);
    let mut empty = cells().filter(|&c| !g.is_filled(c));
    let (a, b) = (empty.next().unwrap(), empty.next().unwrap());
    g.toggle_mark(a, 3);
    g.add_number(b, 1);
    g.erase(b);
    g.add_number(b, g.solution[b.0 as usize][b.1 as usize]);
    g.undo();
    assert!(check_game(&g).is_ok() && !g.redo.is_empty());
    g
}

#[test]
fn a_saved_game_with_marks_on_a_starting_number_is_not_used() {
    // PER-5, MRK-3
    let base = base_game();
    let mut bad = base.clone();
    let (r, c) = given_cell(&base);
    bad.marks[r as usize][c as usize] = mark_bit(4);
    assert_rejected(&base, &bad, "MRK-3", "marks-on-given");
}

#[test]
fn a_saved_game_with_a_mark_out_of_1_to_9_is_not_used() {
    // PER-5, MRK-1: bit 0 and bit 10 are not marks.
    let base = base_game();
    let (r, c) = cells().find(|&c| !base.is_filled(c)).unwrap();
    for (bit, name) in [(1u16, "mark-bit-0"), (1 << 10, "mark-bit-10")] {
        let mut bad = base.clone();
        bad.marks[r as usize][c as usize] |= bit;
        assert_rejected(&base, &bad, "MRK-1", name);
    }
}

#[test]
fn a_saved_game_with_value_10_is_not_used() {
    // PER-5, BRD-1
    let base = base_game();
    let (r, c) = cells().find(|&c| !base.is_filled(c)).unwrap();
    let mut bad = base.clone();
    bad.value[r as usize][c as usize] = 10;
    assert_rejected(&base, &bad, "BRD-1", "value-10");
}

#[test]
fn a_saved_game_file_that_cannot_be_read_is_not_used() {
    // PER-5, BRD-1: a value that is not a number of 0..255, and a file that stops in the middle.
    let base = base_game();
    let dir = data_dir("unreadable");
    let path = dir.join("game.json");
    save_json(&path, &base).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let (r, c) = cells().find(|&c| !base.is_filled(c)).unwrap();
    let mut value = base.value;
    value[r as usize][c as usize] = 77;
    let marked = text.replace(&serde_json::to_string(&base.value).unwrap(), &serde_json::to_string(&value).unwrap());
    assert_ne!(marked, text);
    for (name, bad) in [("value-300", marked.replace("77", "300")), ("truncated", text[..text.len() / 2].to_string())] {
        std::fs::write(&path, bad).unwrap();
        let loaded: Option<Game> = load_json(&path);
        assert!(loaded.is_none(), "{name}: the file must not give a game");
        let app = open_app(loaded, None, ResultsDb::in_memory(), 5);
        assert_eq!(app.game, new_game(first_seed(5), FIRST_MODE), "{name}");
        check_app(&app).unwrap_or_else(|e| panic!("{name}: {e}"));
    }
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_saved_game_with_mode_7_is_not_used() {
    // PER-5, DIF-1
    let base = base_game();
    let bad = Game { mode: 7, ..base.clone() };
    assert_rejected(&base, &bad, "DIF-1", "mode-7");
}

#[test]
fn a_saved_game_with_a_wrong_completed_flag_is_not_used() {
    // PER-5, CMP-1: `completed` on a board that is not solved, and not `completed` on a solved board.
    let base = base_game();
    let bad = Game { completed: true, ..base.clone() };
    assert_rejected(&base, &bad, "CMP-1", "completed-not-solved");

    let (mut solved, last) = nearly_complete(902, 1);
    solved.add_number(last, solved.solution[last.0 as usize][last.1 as usize]);
    assert!(solved.completed);
    let bad = Game { completed: false, ..solved.clone() };
    assert_rejected(&solved, &bad, "CMP-1", "solved-not-completed");
}

#[test]
fn a_saved_game_whose_history_does_not_give_the_board_is_not_used() {
    // PER-5, UND-3: an undo item that is not on the board, a number with no undo item, and a redo
    // item that does not apply to the board. UND-1: an undo item that changes a starting number.
    let base = base_game();
    let mut empty = cells().filter(|&c| !base.is_filled(c) && base.marks_at(c) == 0);
    let (e1, e2) = (empty.next().unwrap(), empty.next().unwrap());

    let mut bad = base.clone();
    bad.undo.push(Change::ToggleMark { cell: e1, digit: 6 });
    assert_rejected(&base, &bad, "UND-3", "undo-not-on-board");

    let mut bad = base.clone();
    bad.value[e1.0 as usize][e1.1 as usize] = base.solution[e1.0 as usize][e1.1 as usize];
    assert_rejected(&base, &bad, "UND-3", "number-without-undo-item");

    let mut bad = base.clone();
    bad.redo.push(Change::Erase { cell: e2, digit: 3 });
    assert_rejected(&base, &bad, "UND-3", "redo-does-not-apply");

    let mut bad = base.clone();
    let g = given_cell(&base);
    bad.undo.insert(0, Change::ToggleMark { cell: g, digit: 2 });
    bad.undo.insert(1, Change::ToggleMark { cell: g, digit: 2 });
    assert_rejected(&base, &bad, "UND-1", "undo-mark-on-given");
}

#[test]
fn a_saved_number_with_a_wrong_list_of_removed_marks_is_not_used() {
    // PER-5, MRK-5: a new number has no `removed` list. A game of an earlier version can have one
    // (the old automatic removal): then it is all peers with the mark. Here it also has a peer
    // without the mark.
    let mut base = new_game(903, 2);
    let (cell, p1, p2) = cells()
        .filter(|&c| !base.is_filled(c))
        .find_map(|c| {
            let mut e = peers(c).filter(|&p| !base.is_filled(p));
            Some((c, e.next()?, e.next()?))
        })
        .unwrap();
    let d = base.solution[cell.0 as usize][cell.1 as usize];
    base.toggle_mark(p1, d);
    base.add_number(cell, d);
    assert_eq!(base.undo.last(), Some(&Change::AddNumber { cell, digit: d, removed: vec![] }));
    assert_ne!(base.marks_at(p1) & mark_bit(d), 0, "MRK-5: the number keeps the peer mark");
    assert!(check_game(&base).is_ok());
    assert_eq!(open_app(Some(base.clone()), None, ResultsDb::in_memory(), 5).game, base);

    // A game of an earlier version: the number removed the mark of p1.
    let mut old = base.clone();
    *old.undo.last_mut().unwrap() = Change::AddNumber { cell, digit: d, removed: vec![p1] };
    old.marks[p1.0 as usize][p1.1 as usize] &= !mark_bit(d);
    assert!(check_game(&old).is_ok());
    assert_eq!(open_app(Some(old.clone()), None, ResultsDb::in_memory(), 5).game, old);

    let mut bad = old.clone();
    *bad.undo.last_mut().unwrap() = Change::AddNumber { cell, digit: d, removed: vec![p1, p2] };
    assert_rejected(&old, &bad, "UND-4", "removed-list-wrong");
}

#[test]
fn a_saved_game_whose_starting_numbers_have_two_solutions_is_not_used() {
    // PER-5, PUZ-2: remove starting numbers until the puzzle has more than one solution.
    let base = new_game(904, 4);
    let mut bad = base.clone();
    for (r, c) in cells() {
        if bad.is_given((r, c)) {
            bad.given[r as usize][c as usize] = false;
            bad.value[r as usize][c as usize] = 0;
            let mut givens = bad.value;
            for (i, j) in cells() {
                if !bad.is_given((i, j)) {
                    givens[i as usize][j as usize] = 0;
                }
            }
            if count_solutions(&givens, 2) > 1 {
                break;
            }
        }
    }
    assert_rejected(&base, &bad, "PUZ-2", "two-solutions");
}

#[test]
fn a_saved_game_with_a_solution_that_is_not_a_sudoku_is_not_used() {
    // PER-5, PUZ-1: two cells of a row that are not starting numbers change places in the solution.
    // The starting numbers stay the solution (PUZ-3).
    let base = new_game(905, 2);
    let (r, a, b) = (0..9u8)
        .find_map(|r| {
            let mut free = (0..9u8).filter(|&c| !base.is_given((r, c)));
            Some((r, free.next()?, free.next()?))
        })
        .unwrap();
    let mut bad = base.clone();
    bad.solution[r as usize].swap(a as usize, b as usize);
    assert_rejected(&base, &bad, "PUZ-1", "solution-not-sudoku");
}

#[test]
fn saved_settings_that_break_a_rule_are_not_used() {
    // PER-5, DIF-1: `last_mode` 5 or 9. COL-5: two of the four sections with the same color (not White).
    // A correct file with other values than the defaults is used.
    let dir = data_dir("bad-settings");
    let path = dir.join("settings.json");
    let good = Settings { color_starting: Color::White, color_clash: Color::White, last_mode: 0, highlight_on: false, ..Settings::default() };
    let bad = [
        ("last-mode-9", Settings { last_mode: 9, ..good.clone() }),
        ("last-mode-5", Settings { last_mode: MODES, ..good.clone() }),
        ("same-color", Settings { color_highlight: Color::Blue, color_starting: Color::Blue, ..good.clone() }),
        ("completed-as-highlight", Settings { color_completed: Color::Cyan, ..good.clone() }),
        ("completed-as-clash", Settings { color_completed: Color::Blue, color_clash: Color::Blue, ..good.clone() }),
        ("completed-as-starting", Settings { color_completed: Color::Blue, color_starting: Color::Blue, ..good.clone() }),
    ];
    save_json(&path, &good).unwrap();
    assert_eq!(open_app(None, load_json(&path), ResultsDb::in_memory(), 5).settings, good);
    for (name, s) in bad {
        assert!(!s.is_valid(), "{name}");
        save_json(&path, &s).unwrap();
        let app = open_app(None, load_json(&path), ResultsDb::in_memory(), 5);
        assert_eq!(app.settings, Settings::default(), "{name}");
        assert_eq!((app.game.mode, app.ui.slider), (FIRST_MODE, FIRST_MODE), "{name}");
        check_app(&app).unwrap_or_else(|e| panic!("{name}: {e}"));
    }
    std::fs::remove_dir_all(&dir).ok();
}

// ---- PER-7

// ---- PER-1, PER-5, CKP-4, CKP-5: the checkpoints and the Restore items in the game file

/// A digit that a tap can add as a mark in `cell`: no peer has it as a number (MRK-7).
fn free_mark(g: &Game, cell: Cell) -> u8 {
    (1..=9).find(|&d| !peer_has(g, cell, d) && g.marks_at(cell) & mark_bit(d) == 0).unwrap()
}

/// A digit that is not the digit of the solution in this cell.
fn other_digit(g: &Game, (r, c): Cell) -> u8 {
    g.solution[r as usize][c as usize] % 9 + 1
}

/// A correct game in progress: 3 checkpoints, a Restore item in `undo` with one item after it, and a
/// Restore item in `redo`.
fn checkpoint_game() -> Game {
    let mut g = new_game(903, 2);
    let empty: Vec<Cell> = cells().filter(|&c| !g.is_filled(c)).collect();
    let (a, b, c, entry) = (empty[0], empty[1], empty[2], empty[3]);
    assert!(g.add_number(entry, g.solution[entry.0 as usize][entry.1 as usize]).changed());
    assert!(g.toggle_mark(a, free_mark(&g, a)).changed());
    g.elapsed_ms = 1000;
    assert!(g.save_checkpoint());
    assert!(g.add_number(b, g.solution[b.0 as usize][b.1 as usize]).changed());
    g.elapsed_ms = 2000;
    assert!(g.save_checkpoint());
    assert!(g.toggle_mark(c, free_mark(&g, c)).changed());
    g.elapsed_ms = 3000;
    assert!(g.save_checkpoint());
    assert!(g.restore_checkpoint(2).changed());
    assert!(g.toggle_mark(c, free_mark(&g, c)).changed());
    assert!(g.restore_checkpoint(1).changed());
    assert!(g.undo().changed());
    g.elapsed_ms = 4000;
    check_game(&g).expect("the fixture is correct");
    assert_eq!(g.checkpoints.len(), 3);
    assert!(matches!(g.undo[g.undo.len() - 2], Change::Restore { .. }), "the fixture has a Restore item in the undo history");
    assert!(matches!(g.redo[..], [Change::Restore { .. }]), "the fixture has a Restore item in the redo history");
    g
}

/// The index of the Restore item in the undo history of `checkpoint_game`.
fn undo_restore(g: &Game) -> usize {
    g.undo.len() - 2
}

/// The two boards of a Restore item.
fn restore_boards(change: &mut Change) -> (&mut Board, &mut Board) {
    match change {
        Change::Restore { before, after } => (before, after),
        other => panic!("not a Restore item: {other:?}"),
    }
}

/// The app does not use the game when `edit` changes one board. CKP-5 applies to all checkpoints and
/// to the two boards of each Restore item, in the two histories. `edit` makes one change only.
fn assert_each_board_rejected(name: &str, edit: impl Fn(&Game, &mut Board)) {
    let base = checkpoint_game();
    let places: [(&str, fn(&mut Game) -> &mut Board); 6] = [
        ("newest-checkpoint", |g| &mut g.checkpoints[0].board),
        ("oldest-checkpoint", |g| &mut g.checkpoints[2].board),
        ("undo-before", |g| {
            let i = undo_restore(g);
            restore_boards(&mut g.undo[i]).0
        }),
        ("undo-after", |g| {
            let i = undo_restore(g);
            restore_boards(&mut g.undo[i]).1
        }),
        ("redo-before", |g| restore_boards(&mut g.redo[0]).0),
        ("redo-after", |g| restore_boards(&mut g.redo[0]).1),
    ];
    for (place, board) in places {
        let mut bad = base.clone();
        edit(&base, board(&mut bad));
        assert_ne!(bad, base, "{name}, {place}: the edit changes the file");
        assert_rejected(&base, &bad, "CKP-5", &format!("{name}-{place}"));
    }
}

#[test]
fn a_checkpoint_board_with_a_value_out_of_range_is_not_used() {
    // PER-5, CKP-5: each value is 0 to 9.
    for v in [10, 255] {
        assert_each_board_rejected(&format!("value-{v}"), |g, b| {
            let (r, c) = cells().find(|&c| !g.is_given(c)).unwrap();
            b.value[r as usize][c as usize] = v;
        });
    }
}

#[test]
fn a_checkpoint_board_with_a_changed_given_cell_is_not_used() {
    // PER-5, CKP-5: a given cell has its value from `solution`. A different digit, and no digit.
    assert_each_board_rejected("given-other-digit", |g, b| {
        let (r, c) = given_cell(g);
        b.value[r as usize][c as usize] = other_digit(g, (r, c));
    });
    assert_each_board_rejected("given-empty", |g, b| {
        let (r, c) = given_cell(g);
        b.value[r as usize][c as usize] = 0;
    });
}

#[test]
fn a_checkpoint_board_with_marks_on_a_given_cell_is_not_used() {
    // PER-5, CKP-5: a given cell has no marks.
    assert_each_board_rejected("given-mark", |g, b| {
        let (r, c) = given_cell(g);
        b.marks[r as usize][c as usize] = mark_bit(4);
    });
}

#[test]
fn a_checkpoint_board_with_a_mark_out_of_range_is_not_used() {
    // PER-5, CKP-5: the marks of the other cells are in 1 to 9. Bit 0 and bit 10 are next to the
    // correct range.
    for (bit, name) in [(1u16, "mark-bit-0"), (1 << 10, "mark-bit-10")] {
        assert_each_board_rejected(name, |g, b| {
            let (r, c) = cells().find(|&c| !g.is_given(c)).unwrap();
            b.marks[r as usize][c as usize] |= bit;
        });
    }
}

#[test]
fn a_checkpoint_board_that_is_solved_is_not_used() {
    // PER-5, CKP-5: the board is not solved.
    assert_each_board_rejected("solved", |g, b| b.value = g.solution);
}

#[test]
fn a_restore_with_a_solved_board_is_not_used_when_all_else_agrees() {
    // PER-5, CKP-5, CMP-1: these files agree with UND-3 and with `completed`. Only the clause of the
    // solved board breaks. Thus a Restore cannot complete a game, and it cannot leave a complete game.
    let (base, last) = nearly_complete(904, 1);
    let mut solved = base.board();
    solved.value[last.0 as usize][last.1 as usize] = base.solution[last.0 as usize][last.1 as usize];
    let mut bad = base.clone();
    bad.undo.push(Change::Restore { before: Box::new(base.board()), after: Box::new(solved.clone()) });
    bad.value = solved.value;
    bad.marks = solved.marks;
    bad.completed = true;
    assert_rejected(&base, &bad, "CKP-5", "restore-after-solved");

    let (mut complete, last) = nearly_complete(905, 1);
    complete.add_number(last, complete.solution[last.0 as usize][last.1 as usize]);
    assert!(complete.completed);
    let mut earlier = complete.board();
    earlier.value[last.0 as usize][last.1 as usize] = 0;
    let mut bad = complete.clone();
    bad.undo.push(Change::Restore { before: Box::new(complete.board()), after: Box::new(earlier.clone()) });
    bad.value = earlier.value;
    bad.marks = earlier.marks;
    bad.completed = false;
    assert_rejected(&complete, &bad, "CKP-5", "restore-before-solved");
}

/// A game with exactly 5 checkpoints. They are different only in one mark, and all have the clock of
/// their digit. The board of the game is different from all of them.
fn five_checkpoints() -> Game {
    let mut g = new_game(906, 2);
    let (r, c) = empty_cell(&g);
    for d in 1..=5 {
        g.marks[r as usize][c as usize] = mark_bit(d);
        g.checkpoints.insert(0, Checkpoint { board: g.board(), elapsed_ms: d as u64 * 100 });
    }
    g.marks[r as usize][c as usize] = 0;
    g.elapsed_ms = 900;
    g
}

#[test]
fn more_than_five_checkpoints_are_not_used() {
    // PER-5, CKP-4: 5 is correct, and 6 is not.
    let base = five_checkpoints();
    let (r, c) = empty_cell(&base);
    let mut sixth = base.board();
    sixth.marks[r as usize][c as usize] = mark_bit(6);
    let mut bad = base.clone();
    bad.checkpoints.insert(0, Checkpoint { board: sixth, elapsed_ms: 600 });
    assert_rejected(&base, &bad, "CKP-4", "six-checkpoints");
}

#[test]
fn two_checkpoints_with_the_same_board_are_not_used() {
    // PER-5, CKP-4: next to each other and not next to each other. The clock is not part of the board.
    let base = checkpoint_game();
    for (a, b, name) in [(0, 1, "equal-next"), (0, 2, "equal-not-next")] {
        let mut bad = base.clone();
        bad.checkpoints[b].board = bad.checkpoints[a].board.clone();
        assert_rejected(&base, &bad, "CKP-4", name);
    }
}

#[test]
fn checkpoints_in_the_wrong_order_are_not_used() {
    // PER-5, CKP-4: each clock is at most the clock of the next newer checkpoint. Only one more
    // millisecond breaks the rule.
    let base = checkpoint_game();
    let mut bad = base.clone();
    bad.checkpoints[1].elapsed_ms = bad.checkpoints[0].elapsed_ms + 1;
    assert_rejected(&base, &bad, "CKP-4", "wrong-order-newer");
    let mut bad = base.clone();
    bad.checkpoints[2].elapsed_ms = bad.checkpoints[1].elapsed_ms + 1;
    assert_rejected(&base, &bad, "CKP-4", "wrong-order-older");
    let mut bad = base.clone();
    bad.checkpoints.reverse();
    assert_rejected(&base, &bad, "CKP-4", "oldest-first");
}

#[test]
fn a_checkpoint_clock_above_the_game_clock_is_not_used() {
    // PER-5, CKP-4: only one more millisecond than the clock of the game breaks the rule.
    let base = checkpoint_game();
    let mut bad = base.clone();
    bad.checkpoints[0].elapsed_ms = bad.elapsed_ms + 1;
    assert_rejected(&base, &bad, "CKP-4", "clock-above-game");
    let mut bad = base.clone();
    bad.elapsed_ms = bad.checkpoints[0].elapsed_ms - 1;
    assert_rejected(&base, &bad, "CKP-4", "game-clock-below-checkpoint");
}

#[test]
fn a_restore_whose_before_is_not_that_point_of_the_history_is_not_used() {
    // PER-5, CKP-5: `before` is a possible board and `after` stays correct, thus the undo history still
    // gives the board. Only `before` is wrong. The undo of this item would give a board that the
    // player never had. In the two histories. Also a difference only in a hidden mark.
    let base = checkpoint_game();
    // The last empty cell: it is empty and has no marks at all points of the history.
    let free = cells().filter(|&c| !base.is_filled(c)).last().unwrap();
    assert_eq!(base.marks_at(free), 0);
    let filled = cells().find(|&c| base.is_entry(c)).unwrap();
    let solution = base.solution[free.0 as usize][free.1 as usize];
    let edits: [(&str, Cell, u16, u8); 3] = [("mark", free, mark_bit(free_mark(&base, free)), 0), ("hidden-mark", filled, mark_bit(1), 0), ("number", free, 0, solution)];
    for (what, (r, c), mark, value) in edits {
        let edit = |b: &mut Board| {
            b.marks[r as usize][c as usize] ^= mark;
            if value != 0 {
                b.value[r as usize][c as usize] = value;
            }
        };
        let mut bad = base.clone();
        let i = undo_restore(&bad);
        edit(restore_boards(&mut bad.undo[i]).0);
        assert_rejected(&base, &bad, "CKP-5", &format!("undo-before-{what}"));
        let mut bad = base.clone();
        edit(restore_boards(&mut bad.redo[0]).0);
        assert_rejected(&base, &bad, "CKP-5", &format!("redo-before-{what}"));
    }
}

#[test]
fn a_restore_with_before_equal_to_after_is_not_used() {
    // PER-5, CKP-5: all else agrees, because the item does not change the board. In the two histories.
    let mut base = checkpoint_game();
    base.redo.clear();
    let same = Change::Restore { before: Box::new(base.board()), after: Box::new(base.board()) };
    let mut bad = base.clone();
    bad.undo.push(same.clone());
    assert_rejected(&base, &bad, "CKP-5", "undo-before-equal-after");
    let mut bad = base.clone();
    bad.redo.push(same);
    assert_rejected(&base, &bad, "CKP-5", "redo-before-equal-after");
}

#[test]
fn a_game_file_with_a_checkpoint_that_cannot_be_read_is_not_used() {
    // PER-5, BRD-1, CKP-5: a value above 255, a negative clock, a Restore item with one board and a
    // list that is not a list. The file does not give a game.
    let base = checkpoint_game();
    let dir = data_dir("ckp-unreadable");
    let path = dir.join("game.json");
    save_json(&path, &base).unwrap();
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let i = undo_restore(&base);
    let edits: [(&str, fn(&mut serde_json::Value, usize)); 4] = [
        ("value-300", |j, _| j["checkpoints"][0]["board"]["value"][0][0] = 300.into()),
        ("negative-clock", |j, _| j["checkpoints"][0]["elapsed_ms"] = (-1).into()),
        ("restore-with-one-board", |j, i| {
            j["undo"][i]["Restore"].as_object_mut().unwrap().remove("after").unwrap();
        }),
        ("list-is-not-a-list", |j, _| j["checkpoints"] = 5.into()),
    ];
    for (name, edit) in edits {
        let mut bad = json.clone();
        edit(&mut bad, i);
        std::fs::write(&path, bad.to_string()).unwrap();
        let loaded: Option<Game> = load_json(&path);
        assert!(loaded.is_none(), "{name}: the file must not give a game");
        let app = open_app(loaded, None, ResultsDb::in_memory(), 5);
        assert_eq!(app.game, new_game(first_seed(5), FIRST_MODE), "{name}");
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// The app uses the game from the file, with no change (PER-1, PER-5).
fn assert_used(g: &Game, name: &str) {
    check_game(g).unwrap_or_else(|e| panic!("{name}: {e}"));
    let dir = data_dir(name);
    let path = dir.join("game.json");
    save_json(&path, g).unwrap();
    let loaded: Game = load_json(&path).expect("the file is JSON of a game");
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(&loaded, g, "{name}: the file gives the same game");
    let app = open_app(Some(loaded), None, ResultsDb::in_memory(), 5);
    assert_eq!(&app.game, g, "{name}: the app must use this game");
    check_app(&app).unwrap_or_else(|e| panic!("{name}: {e}"));
}

#[test]
fn the_limits_of_ckp_4_and_ckp_5_are_correct_in_a_file() {
    // PER-5, CKP-4, CKP-5: the files next to the rejected files. Else a check that rejects too much
    // also passes the tests above.
    let base = checkpoint_game();
    assert_used(&base, "ckp-base");

    // All clocks equal, and equal to the clock of the game: "at most".
    let mut g = base.clone();
    g.elapsed_ms = 2000;
    for k in &mut g.checkpoints {
        k.elapsed_ms = 2000;
    }
    assert_used(&g, "ckp-equal-clocks");

    // Exactly 5 checkpoints.
    assert_used(&five_checkpoints(), "ckp-five");

    // Two boards with the same numbers and the same visible marks, different only in a hidden mark of
    // an entry: they are different boards (`same_board`).
    let mut g = new_game(907, 2);
    let c = empty_cell(&g);
    let v = g.solution[c.0 as usize][c.1 as usize];
    assert!(g.toggle_mark(c, v).changed());
    assert!(g.add_number(c, v).changed());
    assert!(g.save_checkpoint(), "the mark is hidden below the number");
    assert!(g.erase(c).changed());
    assert!(g.toggle_mark(c, v).changed());
    assert!(g.add_number(c, v).changed());
    assert_eq!(g.value, g.checkpoints[0].board.value);
    assert_ne!(g.marks, g.checkpoints[0].board.marks);
    assert!(g.save_checkpoint());
    assert!(g.restore_checkpoint(1).changed(), "a difference in a hidden mark is a Restore");
    assert_used(&g, "ckp-hidden-mark");

    // A board with all cells filled that has a clash is not solved: a checkpoint can keep it.
    let (mut g, last) = nearly_complete(908, 1);
    assert!(g.add_number(last, other_digit(&g, last)).changed());
    assert!(!g.completed);
    assert!(g.save_checkpoint(), "a full board with a clash");
    assert!(g.erase(last).changed());
    assert!(g.restore_checkpoint(0).changed());
    assert_used(&g, "ckp-full-with-clash");

    // A checkpoint on a complete game stays in the file until a new game, also with a Restore before
    // the completion (CKP-9).
    let (mut g, last) = nearly_complete(909, 1);
    let mark = free_mark(&g, last);
    assert!(g.toggle_mark(last, mark).changed());
    g.elapsed_ms = 500;
    assert!(g.save_checkpoint());
    assert!(g.toggle_mark(last, mark).changed());
    assert!(g.restore_checkpoint(0).changed());
    g.elapsed_ms = 900;
    assert!(g.add_number(last, g.solution[last.0 as usize][last.1 as usize]).changed());
    assert!(g.completed && g.checkpoints.len() == 1);
    assert_used(&g, "ckp-complete");
}

#[test]
fn a_game_file_of_an_earlier_version_has_no_checkpoints() {
    // CKP-5, PER-2, PER-5: a file with no field `checkpoints` loads. The app uses it with an empty list,
    // and a save operates on it. An empty list does not write the field: an earlier version can read
    // a game with no checkpoint and no Restore item.
    let g = base_game();
    let dir = data_dir("ckp-old-file");
    let path = dir.join("game.json");
    save_json(&path, &g).unwrap();
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let fields = json.as_object().unwrap();
    assert!(!fields.contains_key("checkpoints"), "PER-1: an empty list does not write the field");
    let earlier = ["seed", "mode", "generator", "solution", "given", "value", "marks", "undo", "redo", "elapsed_ms", "completed"];
    let mut keys: Vec<&str> = fields.keys().map(String::as_str).collect();
    keys.sort_unstable();
    let mut want = earlier.to_vec();
    want.sort_unstable();
    assert_eq!(keys, want, "the file has only the fields of an earlier version");
    let loaded: Game = load_json(&path).expect("the file of an earlier version gives a game");
    std::fs::remove_dir_all(&dir).ok();
    assert!(loaded.checkpoints.is_empty());
    assert_eq!(loaded, g);

    let mut d = Driver::opened(open_app(Some(loaded), None, ResultsDb::in_memory(), 5));
    assert_eq!(d.app.game, g, "the app uses the game of the file");
    d.button(Button::Checkpoints);
    let (x, y) = exact_center(layout::ckp_save_row());
    d.finger_tap(x, y);
    assert_eq!(d.app.game.checkpoints.len(), 1, "CKP-1: a save operates on the game of an earlier version");
    check_app(&d.app).unwrap();

    // An explicit empty list gives the same game.
    let mut text = serde_json::to_value(&g).unwrap();
    text["checkpoints"] = serde_json::json!([]);
    assert_eq!(serde_json::from_value::<Game>(text).unwrap(), g);
}

#[test]
fn the_checkpoints_and_the_restore_items_go_through_the_file() {
    // PER-1, PER-2, CKP-3, UND-3, UND-9: the app opens with the same list and the same two histories.
    // The panel is closed. The undo and the redo of the items of the file give the correct boards,
    // also after the delete of the checkpoints.
    let g = checkpoint_game();
    let dir = data_dir("ckp-round-trip");
    let path = dir.join("game.json");
    save_json(&path, &g).unwrap();
    let loaded: Game = load_json(&path).unwrap();
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(loaded, g);
    let mut app = open_app(Some(loaded), None, ResultsDb::in_memory(), 5);
    assert_eq!(app.game, g);
    assert_eq!((app.screen, app.ui.menu.panel()), (Screen::Game, None), "PER-2");

    // The boards of the history. This model reads the items, and it does not use `undo` or `redo`.
    let start = new_game(903, 2);
    let (mut value, mut marks) = (start.value, start.marks);
    let mut boards = vec![(value, marks)];
    for change in &g.undo {
        match change {
            Change::Restore { after, .. } => (value, marks) = (after.value, after.marks),
            Change::ToggleMark { cell, digit } => marks[cell.0 as usize][cell.1 as usize] ^= mark_bit(*digit),
            Change::AddNumber { cell, digit, .. } => value[cell.0 as usize][cell.1 as usize] = *digit,
            Change::Erase { cell, .. } => value[cell.0 as usize][cell.1 as usize] = 0,
        }
        boards.push((value, marks));
    }
    assert_eq!(boards.last(), Some(&(app.game.value, app.game.marks)), "UND-3: the items give the board");

    while !app.game.checkpoints.is_empty() {
        assert!(app.game.delete_checkpoint(0));
    }
    for want in boards.iter().rev().skip(1) {
        assert!(app.game.undo().changed());
        assert_eq!(&(app.game.value, app.game.marks), want, "UND-3, CKP-3: undo after the delete of the checkpoints");
    }
    assert!(!app.game.can_undo());
    for want in boards.iter().skip(1) {
        assert!(app.game.redo().changed());
        assert_eq!(&(app.game.value, app.game.marks), want, "UND-3, CKP-3: redo after the delete of the checkpoints");
    }
    let Change::Restore { after, .. } = &g.redo[0] else { panic!("the fixture has a Restore item") };
    assert!(app.game.redo().changed(), "the Restore item of the file is in the redo history");
    assert!(app.game.same_board(after), "CKP-2: redo gives the board of the checkpoint");
    assert_eq!(app.game.elapsed_ms, g.elapsed_ms, "UND-9");
}

#[test]
fn each_change_of_the_board_and_the_settings_is_marked_for_saving() {
    // PER-7: each change to the board sets `game_dirty`; each change of the settings sets
    // `settings_dirty`. Input with no change sets neither. The platform saves when a flag is set.
    // Also each save and each delete of a checkpoint, and a Restore. A tap on "Current" and a save that
    // is not available change nothing, thus they set no flag (CKP-1, CKP-2, CKP-3).
    let mut d = Driver::new(62);
    let clear = |d: &mut Driver| {
        d.app.game_dirty = false;
        d.app.settings_dirty = false;
    };
    let cell = free_cell(&d.app, &[5]);
    type Step = (&'static str, fn(&mut Driver, Cell), bool, bool);
    let steps: [Step; 11] = [
        ("mark", |d, c| d.tap_micro(c, 5), true, false),
        ("number", |d, c| d.write(c, d.app.game.solution[c.0 as usize][c.1 as usize]), true, false),
        ("erase", |d, c| d.erase(c), true, false),
        ("menu", |d, _| d.button(Button::Menu), false, false),
        ("undo", |d, _| d.button(Button::Undo), true, false),
        ("redo", |d, _| d.button(Button::Redo), true, false),
        ("color", |d, _| {
            d.button(Button::Colors);
            let (x, y) = color_point(Section::Clash, Color::Orange);
            d.finger_tap(x, y);
        }, false, true),
        ("switch", |d, _| {
            d.button(Button::Settings);
            let (x, y) = switch_point(Switch::HideMarksAutomatically);
            d.pen_tap(x, y);
        }, false, true),
        ("switch of the completed groups", |d, _| {
            let (x, y) = switch_point(Switch::ColorCompletedGroups);
            d.pen_tap(x, y);
        }, false, true),
        ("completed color", |d, _| {
            d.button(Button::Colors);
            let (x, y) = color_point(Section::Completed, Color::Blue);
            d.finger_tap(x, y);
        }, false, true),
        ("start new game", |d, _| {
            d.button(Button::NewGame);
            d.finger_tap(d.app.ng_stops[0], layout::ng_slider_y() as f32);
            d.finger_tap(layout::ng_pl() as f32 + 50.0, layout::ng_start_y() as f32);
        }, true, true),
    ];
    for (name, step, game, settings) in steps {
        clear(&mut d);
        step(&mut d, cell);
        assert_eq!((d.app.game_dirty, d.app.settings_dirty), (game, settings), "{name}");
    }
    assert_eq!((d.app.game.mode, d.app.settings.color_clash, d.app.settings.auto_hide_on), (0, Color::Orange, false));
    assert_eq!((d.app.settings.color_completed, d.app.settings.completed_color_on), (Color::Blue, false));
    // The checkpoints. The finger taps come after the guard of NAV-16 and of CKP-8.
    d.button(Button::Checkpoints);
    assert_eq!(d.app.ui.menu.panel(), Some(Panel::Checkpoints));
    let (sx, sy) = exact_center(layout::ckp_save_row());
    let (bx, by) = exact_center(layout::ckp_button(0));
    clear(&mut d);
    d.finger_tap(sx, sy);
    assert_eq!(d.app.game.checkpoints.len(), 1);
    assert!(d.app.game_dirty, "PER-7: a save");
    clear(&mut d);
    d.finger_tap(sx, sy);
    assert_eq!(d.app.game.checkpoints.len(), 1, "CKP-1: a checkpoint has this board");
    assert!(!d.app.game_dirty, "PER-7: a save that is not available changes nothing");
    d.finger_tap(bx, by);
    assert!(!d.app.game_dirty, "PER-7: \"Current\" changes nothing");
    let target = empty_cell(&d.app.game);
    let digit = free_mark(&d.app.game, target);
    d.button(Button::Menu);
    d.tap_micro(target, digit);
    assert_eq!(d.app.game.marks_at(target), mark_bit(digit));
    d.button(Button::Checkpoints);
    clear(&mut d);
    d.finger_tap(bx, by);
    assert_eq!(d.app.game.marks_at(target), 0, "CKP-2");
    assert!(d.app.game_dirty, "PER-7: a Restore");
    assert_eq!(d.app.ui.menu.panel(), Some(Panel::Checkpoints), "CKP-2");
    clear(&mut d);
    let (x, y) = exact_center(layout::ckp_bin_area(0));
    d.finger_tap(x, y);
    assert!(d.app.game.checkpoints.is_empty(), "CKP-3");
    assert!(d.app.game_dirty, "PER-7: a delete");
    d.button(Button::Menu);
    // A finger on the board and a tap on a starting number change nothing that is saved.
    clear(&mut d);
    let (x, y) = micro_center(empty_cell(&d.app.game), 5);
    d.finger_tap(x, y);
    d.tap_cell(given_cell(&d.app.game));
    assert!(!d.app.game_dirty && !d.app.settings_dirty);
    assert_eq!((d.app.screen, d.app.ui.menu.panel()), (Screen::Game, None));
}

// ---- ANA-1

#[test]
fn each_result_has_correct_values_and_a_unique_id() {
    // ANA-1: complete games of all modes, delete one result, complete more. Each result has a mode in
    // 0..4, the starting numbers of its puzzle (≤ 81), the seconds of the clock, the seed, the time of
    // completion and an id that no other result has. ANA-2: one result for each complete game.
    let dir = data_dir("ana1");
    let path = dir.join(RESULTS_FILE);
    let mut games: HashMap<u64, Game> = HashMap::new();
    let complete = |games: &mut HashMap<u64, Game>, i: u64, mode: u8| {
        let (g, last) = nearly_complete(1000 + i, mode);
        let mut d = Driver::from(open_app(Some(g), None, ResultsDb::open(&path).unwrap(), 90 + i));
        d.wait(1500 + i * 700);
        write_last_number(&mut d, last);
        check_app(&d.app).unwrap();
        games.insert(d.app.game.seed, d.app.game.clone());
        d
    };
    for (i, mode) in [0u8, 1, 2].into_iter().enumerate() {
        complete(&mut games, i as u64, mode);
    }
    // Delete one result in the games list.
    let mut d = complete(&mut games, 3, 3);
    d.button(Button::Menu);
    d.button(Button::Statistics);
    let (x, y) = exact_center(layout::all_games_button());
    d.finger_tap(x, y);
    let (x, y) = exact_center(layout::bin_button(1));
    d.finger_tap(x, y);
    let victim = d.app.list.confirm.clone().expect("the delete dialog");
    let (x, y) = exact_center(layout::dialog_delete());
    d.finger_tap(x, y);
    assert!(d.app.results.iter().all(|r| r.id != victim.id));
    drop(d);
    games.remove(&victim.seed);
    for (i, mode) in [4u8, 5, 6, 2, 0].into_iter().enumerate() {
        complete(&mut games, 4 + i as u64, mode);
    }

    let results = ResultsDb::open(&path).unwrap().all().unwrap();
    assert_eq!(results.len(), 8);
    let ids: HashSet<i64> = results.iter().map(|r| r.id).collect();
    assert_eq!(ids.len(), results.len(), "each id is unique");
    assert_eq!(results.iter().map(|r| r.mode).collect::<HashSet<_>>(), (0..MODES).collect());
    for r in &results {
        let g = games.get(&r.seed).unwrap_or_else(|| panic!("no complete game for {r:?}"));
        assert!(r.mode < MODES && r.givens <= 81, "{r:?}");
        assert_eq!((r.mode, r.givens, r.seconds), (g.mode, g.givens_count(), g.seconds()), "{r:?}");
        assert!(r.seconds >= 1, "the clock operated: {r:?}");
        assert!(r.finished_at > 1_790_000_000, "Unix time of the completion: {r:?}");
    }
    let app = open_app(None, None, ResultsDb::open(&path).unwrap(), 99);
    assert_eq!(app.results, results);
    check_app(&app).unwrap();
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_game_saved_before_generator_version_2_is_used() {
    // PUZ-5, PER-5: a game file of version 1 has no "generator". It is a puzzle of version 1, and the
    // app continues it. The same file with a puzzle of version 2, or with mode 5 or 6, is not correct.
    let p = generate_version(1, 908, 3);
    let v1 = Game { generator: 1, ..Game::new(908, 3, p.solution, p.givens) };
    let mut json = serde_json::to_value(&v1).unwrap();
    json.as_object_mut().unwrap().remove("generator");
    let dir = data_dir("puz5-v1");
    let path = dir.join("game.json");
    std::fs::write(&path, json.to_string()).unwrap();
    let loaded: Game = load_json(&path).expect("a game file of version 1");
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(loaded, v1, "a file with no generator is version 1");
    let app = open_app(Some(loaded), None, ResultsDb::in_memory(), 5);
    assert_eq!(app.game, v1, "the app continues the game of version 1");
    check_app(&app).unwrap();

    let v2_puzzle = Game { generator: 1, ..new_game(908, 3) };
    let v1_mode_5 = Game { generator: 1, ..new_game(909, 5) };
    for (name, bad) in [("version 2 puzzle", v2_puzzle), ("mode 5", v1_mode_5)] {
        let app = open_app(Some(bad.clone()), None, ResultsDb::in_memory(), 5);
        assert_ne!(app.game, bad, "{name}: the app used a game that is not generate_version(1, seed, mode)");
        assert_eq!(app.game, new_game(first_seed(5), FIRST_MODE), "{name}: a new game of the last mode");
        assert_eq!(app.game.generator, VERSION, "{name}: a new game has the current version");
    }
}

#[test]
fn a_saved_game_that_is_not_the_puzzle_of_its_seed_and_mode_is_not_used() {
    // PER-5, PUZ-5 (confirmed 2026-10-25): the starting numbers and the solution must be
    // `generate(seed, mode)`. Each corruption agrees with all other rules, thus only PUZ-5 rejects it.
    let base = new_game(905, 2);
    let control = open_app(Some(base.clone()), None, ResultsDb::in_memory(), 5);
    assert_eq!(control.game, base, "the game of its own seed and mode is used");

    let other_seed = Game { seed: 906, ..base.clone() };
    let other_mode = Game { mode: 3, ..base.clone() };
    let other_puzzle = Game { seed: base.seed, mode: base.mode, ..new_game(907, 2) };
    for (name, bad) in [("seed", other_seed), ("mode", other_mode), ("puzzle", other_puzzle)] {
        assert!(check_game(&bad).is_ok(), "{name}: the corruption agrees with the other rules");
        let dir = data_dir(&format!("puz5-{name}"));
        let path = dir.join("game.json");
        save_json(&path, &bad).unwrap();
        let loaded: Game = load_json(&path).unwrap();
        std::fs::remove_dir_all(&dir).ok();
        let app = open_app(Some(loaded), None, ResultsDb::in_memory(), 5);
        assert_ne!(app.game, bad, "{name}: the app used a game that is not generate(seed, mode)");
        assert_eq!(app.game, new_game(first_seed(5), FIRST_MODE), "{name}: a new game of the last mode");
        check_app(&app).unwrap_or_else(|e| panic!("{name}: {e}"));
    }
}

#[test]
fn result_rows_that_break_ana_1_are_not_used() {
    // ANA-1, PER-5 (confirmed 2026-10-25): the app does not use a result row that breaks ANA-1. The
    // schema of the app does not let such a row in, thus the test makes a database without the CHECK
    // constraints, as an older or changed file can have.
    let dir = data_dir("ana1-rows");
    let path = dir.join(RESULTS_FILE);
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE results (
                 id          INTEGER PRIMARY KEY AUTOINCREMENT,
                 finished_at INTEGER NOT NULL,
                 mode        INTEGER NOT NULL,
                 givens      INTEGER NOT NULL,
                 seconds     INTEGER NOT NULL,
                 seed        INTEGER NOT NULL
             );
             INSERT INTO results (finished_at, mode, givens, seconds, seed) VALUES
                 (1790000000, 1, 40, 300, 11),
                 (1790000001, 7, 40, 300, 12),
                 (1790000002, 5, 40, 300, 13),
                 (1790000003, -1, 40, 300, 14),
                 (1790000004, 2, 82, 300, 15),
                 (1790000005, 2, -1, 300, 16),
                 (1790000006, 2, 40, -5, 17),
                 (1790000007, 'x', 40, 300, 18),
                 (1790000008, 4, 81, 0, 19);",
        )
        .unwrap();
    }
    let app = open_app(None, None, ResultsDb::open(&path).unwrap(), 5);
    let seeds: HashSet<u64> = app.results.iter().map(|r| r.seed).collect();
    assert_eq!(seeds, HashSet::from([11, 19]), "only the rows that agree with ANA-1 are used");
    for r in &app.results {
        assert!(r.mode < MODES && r.givens <= 81, "ANA-1: {r:?}");
    }
    check_app(&app).unwrap();
    std::fs::remove_dir_all(&dir).ok();
}

// ---- MRK-17, PER-2: the x is not saved.

#[test]
fn the_saved_files_have_no_x_and_the_app_opens_with_no_x() {
    // MRK-17, PER-2, MRK-11: the game file and the settings file are the same with the x as without
    // it. The app opens from these files with no x, and the refused tap stays out of the history.
    let dir = data_dir("no-x");
    let mut d = Driver::new(64);
    let g = &d.app.game;
    let (cell, digit) = cells()
        .filter(|&c| !g.is_filled(c))
        .flat_map(|c| (1..=9u8).map(move |v| (c, v)))
        .find(|&(c, v)| g.peer_digits(c) & mark_bit(v) != 0)
        .expect("a refused tap");
    let (gp, sp) = (dir.join("game.json"), dir.join("settings.json"));
    let read = |gp: &PathBuf, sp: &PathBuf, app: &sudoku::app::App| {
        save_json(gp, &app.game).unwrap();
        save_json(sp, &app.settings).unwrap();
        let mut game: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(gp).unwrap()).unwrap();
        let settings: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(sp).unwrap()).unwrap();
        // MRK-11: the x does not change the clock, but the time of the input does: not a part of the x.
        game["elapsed_ms"] = 0.into();
        (game, settings)
    };
    let without = read(&gp, &sp, &d.app);
    d.tap_micro(cell, digit);
    assert_eq!(d.app.ui.refused, Some((cell, digit, d.now)), "MRK-10");
    let with = read(&gp, &sp, &d.app);
    assert_eq!(with, without, "MRK-17: the x changed the saved files");
    for text in [&with.0, &with.1] {
        let text = text.to_string();
        assert!(!text.contains("refused"), "MRK-17: a saved file has the x: {text}");
    }
    let game = load_json::<Game>(&gp).unwrap();
    let app = open_app(Some(game), None, ResultsDb::in_memory(), 64);
    assert_eq!(app.game.seed, d.app.game.seed, "the app used the saved game");
    assert_eq!(app.ui.refused, None, "MRK-17: the app opens with no x");
    std::fs::remove_dir_all(&dir).ok();
}

// ---- HLT-2 item 5, PER-2: a restart.

#[test]
fn a_restart_of_the_app_removes_a_highlight() {
    // HLT-2 item 5, PER-2 (owner decision 2026-11-07): the app does not save the highlight, thus it does not
    // come back at the next start. The game itself comes back.
    let mut d = Driver::new(701);
    let given = given_cell(&d.app.game);
    let (x, y) = micro_center(given, 5);
    d.pen_tap(x, y);
    assert_eq!(d.app.ui.highlight, Some(d.app.game.value_at(given)), "the fixture has the highlight");
    let text = serde_json::to_string(&d.app.game).unwrap();
    assert!(!text.to_lowercase().contains("highlight"), "HLT-2: the saved game has the highlight");
    let saved: Game = serde_json::from_str(&text).unwrap();
    let app = app_with_game(saved, START_TIME, 7);
    assert_eq!(app.game.value, d.app.game.value, "PER-2: the saved game comes back");
    assert_eq!(app.ui.highlight, None, "HLT-2, PER-2: no highlight at the start");
    check_app(&app).unwrap();
}
