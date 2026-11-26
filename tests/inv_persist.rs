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
use sudoku::app::{game_of_version, new_game, new_game_with_symmetry, App, Env, Panel, Screen};
use sudoku::game::{cells, mark_bit, peers, Board, Cell, Change, Checkpoint, Game};
use sudoku::generator::{count_solutions, generate_version, partner, Version, FIRST_MODE, FIRST_SYMMETRIC, MODES, VERSION};
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
    nearly_complete_sym(seed, mode, false)
}

/// As `nearly_complete`, for the puzzle of `(seed, mode, symmetric)`.
fn nearly_complete_sym(seed: u64, mode: u8, symmetric: bool) -> (Game, Cell) {
    let mut g = new_game_with_symmetry(seed, mode, symmetric);
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
    // PER-3: the four colors, the micro-cell line gray (COL-7), `last_mode` and `last_symmetric` (from
    // Start new game) and the three switches go through the settings file. DIF-3: the slider and the
    // Symmetrical switch open at the kept values. The kept symmetry is off: the opposite of the default.
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
    assert!(d.app.ui.symmetric_sel, "DIF-9: the switch is on at the first start");
    d.finger_tap(layout::ng_pl() as f32 + 50.0, layout::ng_symmetric_y() as f32);
    assert!(!d.app.ui.symmetric_sel, "DIF-8");
    d.finger_tap(layout::ng_pl() as f32 + 50.0, layout::ng_start_y() as f32);
    let s = d.app.settings.clone();
    assert_eq!((s.color_starting, s.color_clash, s.color_highlight, s.color_completed), (Color::Yellow, Color::Red, Color::Magenta, Color::Orange));
    assert_eq!(s.micro_line, LineGray::Gray55);
    assert_eq!(s.last_mode, 4);
    assert!(!s.last_symmetric && !d.app.game.symmetric, "DIF-5");
    assert!(!s.highlight_on && !s.auto_hide_on && !s.completed_color_on);
    assert!(d.app.settings_dirty, "PER-7: Start new game changes the settings");

    let dir = data_dir("settings");
    let (gp, sp) = (dir.join("game.json"), dir.join("settings.json"));
    save_json(&gp, &d.app.game).unwrap();
    save_json(&sp, &d.app.settings).unwrap();
    let app = open_app(load_json(&gp), load_json(&sp), ResultsDb::in_memory(), 3);
    assert_eq!(app.settings, s);
    assert_eq!((app.ui.slider, app.ui.symmetric_sel), (4, false), "DIF-3");
    assert_eq!(app.game, d.app.game);

    // With no saved game, the new game has the kept mode and the kept symmetry.
    let app = open_app(None, load_json(&sp), ResultsDb::in_memory(), 3);
    assert_eq!(app.settings, s);
    assert_eq!((app.game.mode, app.game.symmetric), (4, false));
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
    assert_eq!(app.game, new_game_with_symmetry(first_seed(5), FIRST_MODE, FIRST_SYMMETRIC), "{name}: a new game of the last mode");
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
        assert_eq!(app.game, new_game_with_symmetry(first_seed(5), FIRST_MODE, FIRST_SYMMETRIC), "{name}");
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
        assert_eq!((app.game.symmetric, app.ui.symmetric_sel), (FIRST_SYMMETRIC, FIRST_SYMMETRIC), "DIF-9, PER-5: {name}");
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
fn checkpoints(n: u8) -> Game {
    // A game with `n` correct checkpoints, each with a different mark in one cell, the newest first.
    let mut g = new_game(906, 2);
    let (r, c) = empty_cell(&g);
    for d in 1..=n {
        g.marks[r as usize][c as usize] = mark_bit(d);
        g.checkpoints.insert(0, Checkpoint { board: g.board(), elapsed_ms: d as u64 * 100 });
    }
    g.marks[r as usize][c as usize] = 0;
    g.elapsed_ms = 900;
    g
}

#[test]
fn a_file_with_more_than_four_checkpoints_keeps_the_four_newest() {
    // CKP-4, CKP-5, PER-5: an earlier version allowed 5 checkpoints. A file with 5 (or more) correct
    // checkpoints is the same game with its 4 newest checkpoints, not a new game. The other values of
    // the file do not change.
    for n in [5, 6] {
        let name = format!("ckp-{n}");
        let g = checkpoints(n);
        assert!(check_game(&g).expect_err(&name).contains("CKP-4"), "{name}: the file breaks CKP-4");
        let dir = data_dir(&name);
        let path = dir.join("game.json");
        save_json(&path, &g).unwrap();
        let loaded: Game = load_json(&path).expect("the file is JSON of a game");
        std::fs::remove_dir_all(&dir).ok();
        let app = open_app(Some(loaded), None, ResultsDb::in_memory(), 5);
        let mut want = g.clone();
        want.checkpoints.truncate(4);
        assert_eq!(app.game, want, "{name}: the 4 newest checkpoints stay");
        assert_eq!(app.game.checkpoints[0], g.checkpoints[0], "{name}: the newest is first");
        check_app(&app).unwrap_or_else(|e| panic!("{name}: {e}"));
    }
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
        assert_eq!(app.game, new_game_with_symmetry(first_seed(5), FIRST_MODE, FIRST_SYMMETRIC), "{name}");
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

    // Exactly 4 checkpoints.
    assert_used(&checkpoints(4), "ckp-four");

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
    // "symmetric" and "hints" are the only fields that are new after the checkpoints (PER-9, HNT-6).
    // An earlier version does not know them and ignores them. With no revealed incorrect number, the
    // file has no field "incorrect_shown" (PER-1, HNT-8).
    let earlier = ["seed", "mode", "generator", "symmetric", "solution", "given", "value", "marks", "undo", "redo", "elapsed_ms", "completed", "hints"];
    let mut keys: Vec<&str> = fields.keys().map(String::as_str).collect();
    keys.sort_unstable();
    let mut want = earlier.to_vec();
    want.sort_unstable();
    assert_eq!(keys, want, "the file has only the fields of an earlier version, the symmetry and the hint count");
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
    // CMP-5: the even games are symmetric, the odd games are not.
    let complete = |games: &mut HashMap<u64, Game>, i: u64, mode: u8| {
        let (g, last) = nearly_complete_sym(1000 + i, mode, i % 2 == 0);
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
    assert!(results.iter().any(|r| r.symmetric) && results.iter().any(|r| !r.symmetric), "CMP-5: the two symmetries");
    let ids: HashSet<i64> = results.iter().map(|r| r.id).collect();
    assert_eq!(ids.len(), results.len(), "each id is unique");
    assert_eq!(results.iter().map(|r| r.mode).collect::<HashSet<_>>(), (0..MODES).collect());
    for r in &results {
        let g = games.get(&r.seed).unwrap_or_else(|| panic!("no complete game for {r:?}"));
        assert!(r.mode < MODES && r.givens <= 81, "{r:?}");
        assert_eq!((r.mode, r.givens, r.seconds), (g.mode, g.givens_count(), g.seconds()), "{r:?}");
        assert_eq!(r.symmetric, g.symmetric, "CMP-5: the symmetry of the game: {r:?}");
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
    // PUZ-5, PUZ-13, PER-5: a game file of version 1 has no "generator". It is a puzzle of version 1,
    // and the app continues it. The same file with a puzzle of version 2 or 3, or with mode 5 or 6, is
    // not correct.
    let p = generate_version(1, 908, 3, false);
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

    let v2_puzzle = Game { generator: 1, ..game_of_version(Version::V2, 908, 3, false) };
    let v3_puzzle = Game { generator: 1, ..new_game(908, 3) };
    let v1_mode_5 = Game { generator: 1, ..new_game(909, 5) };
    for (name, bad) in [("version 2 puzzle", v2_puzzle), ("version 3 puzzle", v3_puzzle), ("mode 5", v1_mode_5)] {
        let app = open_app(Some(bad.clone()), None, ResultsDb::in_memory(), 5);
        assert_ne!(app.game, bad, "{name}: the app used a game that is not generate_version(1, seed, mode)");
        assert_eq!(app.game, new_game_with_symmetry(first_seed(5), FIRST_MODE, FIRST_SYMMETRIC), "{name}: a new game of the last mode");
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
        assert_eq!(app.game, new_game_with_symmetry(first_seed(5), FIRST_MODE, FIRST_SYMMETRIC), "{name}: a new game of the last mode");
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
        assert!(!r.symmetric, "ANA-1, PER-9: a row of an earlier database is not symmetric: {r:?}");
        assert_eq!(r.hints, 0, "ANA-1, PER-9: a row of an earlier database has no hints: {r:?}");
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

// ---- HLT-2 item 2, PER-2: a restart.

#[test]
fn a_restart_of_the_app_removes_a_highlight() {
    // HLT-2 item 2, PER-2 (owner decision 2026-11-07): the app does not save the highlight, thus it does not
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

// ---- PUZ-5, PUZ-8, PER-5, PER-9: the symmetry in the saved files.

/// Saves `bad` to a file, opens the app with it and makes sure that the app does not use it (a new
/// game of the default settings, PER-5). Unlike `assert_rejected`, `check_game` can accept `bad`:
/// only the generator (PUZ-5) can find the error.
fn assert_not_used(bad: &Game, name: &str) {
    let dir = data_dir(name);
    let path = dir.join("game.json");
    save_json(&path, bad).unwrap();
    let loaded: Game = load_json(&path).expect("the file is JSON of a game");
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(&loaded, bad, "{name}");
    let app = open_app(Some(loaded), None, ResultsDb::in_memory(), 5);
    assert_ne!(&app.game, bad, "{name}: the app used the saved game");
    assert_eq!(app.game, new_game_with_symmetry(first_seed(5), FIRST_MODE, FIRST_SYMMETRIC), "{name}: a new game");
    check_app(&app).unwrap_or_else(|e| panic!("{name}: {e}"));
}

#[test]
fn a_symmetric_saved_game_of_version_1_is_not_used() {
    // PUZ-8, PER-5: version 1 has no symmetry. A game file with `symmetric` and version 1 is not
    // correct, also when its puzzle is the puzzle of version 1. The same file with no symmetry is used.
    let p = generate_version(1, 910, 2, false);
    let v1 = Game { generator: 1, ..Game::new(910, 2, p.solution, p.givens) };
    assert!(!v1.symmetric);
    let bad = Game { symmetric: true, ..v1.clone() };
    // The rule of version 1 is the first part of PUZ-8 that the check finds, also if the starting
    // numbers are not symmetric.
    let err = check_game(&bad).expect_err("PUZ-8: a symmetric game of version 1");
    assert!(err.contains("PUZ-8"), "PUZ-8: {err}");
    assert_rejected(&v1, &bad, "PUZ-8", "puz8-symmetric-v1");
}

#[test]
#[should_panic]
fn version_1_of_the_generator_has_no_symmetry() {
    // PUZ-5, PUZ-8: "Version 1 has no symmetry". The generator does not make a symmetric puzzle of
    // version 1.
    let _ = generate_version(1, 910, 2, true);
}

#[test]
fn a_saved_game_with_the_wrong_symmetry_is_not_used() {
    // PUZ-5, PER-5: the puzzle must be generate_version(generator, seed, mode, symmetric). A symmetric
    // game whose flag is false, a symmetric flag on a puzzle that is not symmetric, and a symmetric
    // puzzle of a different seed are not used. The correct symmetric game is used.
    for mode in [0u8, 3, 6] {
        let base = new_game_with_symmetry(911 + mode as u64, mode, true);
        let control = open_app(Some(base.clone()), None, ResultsDb::in_memory(), 5);
        assert_eq!(control.game, base, "PUZ-5: mode {mode}: the symmetric game of its own key is used");
        check_app(&control).unwrap();

        // PUZ-8 applies only to a symmetric game: the check accepts this file. Only PUZ-5 rejects it.
        let flag_off = Game { symmetric: false, ..base.clone() };
        assert!(check_game(&flag_off).is_ok(), "mode {mode}: the corruption agrees with the other rules");
        assert_not_used(&flag_off, &format!("puz5-flag-off-{mode}"));

        let other_seed = Game { seed: base.seed + 1, ..base.clone() };
        assert!(check_game(&other_seed).is_ok(), "mode {mode}: the corruption agrees with the other rules");
        assert_not_used(&other_seed, &format!("puz5-sym-seed-{mode}"));

        // The non-symmetric puzzle of the same key with the flag on: not used (PUZ-5, and PUZ-8 if
        // its starting numbers are not symmetric).
        let flag_on = Game { symmetric: true, ..new_game(base.seed, mode) };
        assert_not_used(&flag_on, &format!("puz5-flag-on-{mode}"));
    }
}

#[test]
fn the_files_of_an_earlier_version_get_the_defaults_of_per_9() {
    // PER-9, DIF-3, PER-5: a settings file with no `last_symmetric` gets true; the other values do not
    // change. A game file with no `symmetric` is not symmetric: the app uses a game that is not
    // symmetric, and does not use a symmetric game (its puzzle is not the puzzle of the key with no
    // symmetry, PUZ-5). Two starts with the same files give the same result.
    let dir = data_dir("per9");
    let (gp, sp) = (dir.join("game.json"), dir.join("settings.json"));
    let settings = Settings { last_mode: 5, last_symmetric: false, highlight_on: false, ..Settings::default() };
    let mut json = serde_json::to_value(&settings).unwrap();
    assert_eq!(json["last_symmetric"], false, "the name of INVARIANTS.md §0 in the file");
    json.as_object_mut().unwrap().remove("last_symmetric");
    std::fs::write(&sp, json.to_string()).unwrap();
    let loaded: Settings = load_json(&sp).expect("a settings file of an earlier version");
    assert_eq!(loaded, Settings { last_symmetric: true, ..settings.clone() }, "PER-9: last_symmetric = true");

    let (mut g, _) = nearly_complete(920, 4);
    g.elapsed_ms = 70_000;
    let mut json = serde_json::to_value(&g).unwrap();
    assert_eq!(json["symmetric"], false, "the name of INVARIANTS.md §0 in the file");
    json.as_object_mut().unwrap().remove("symmetric");
    std::fs::write(&gp, json.to_string()).unwrap();
    for start in 0..2 {
        let app = open_app(load_json(&gp), load_json(&sp), ResultsDb::in_memory(), 5);
        assert_eq!(app.game, g, "PER-9: start {start}: the game that is not symmetric is used");
        assert!(app.settings.last_symmetric && app.ui.symmetric_sel, "PER-9, DIF-3: start {start}");
        assert_eq!(app.settings.last_mode, 5, "PER-9: start {start}");
        check_app(&app).unwrap();
    }
    // With no game file, the new game uses the settings of the earlier file: symmetric (PER-9, DIF-5).
    let app = open_app(None, load_json(&sp), ResultsDb::in_memory(), 5);
    assert_eq!(app.game, new_game_with_symmetry(first_seed(5), 5, true), "PER-9: a symmetric game of the kept mode");

    // A symmetric game in a file with no symmetry value is read as not symmetric: it is not used.
    let sym = new_game_with_symmetry(921, 2, true);
    let mut json = serde_json::to_value(&sym).unwrap();
    json.as_object_mut().unwrap().remove("symmetric");
    std::fs::write(&gp, json.to_string()).unwrap();
    let loaded: Game = load_json(&gp).unwrap();
    assert!(!loaded.symmetric, "PER-9: no value is not symmetric");
    for start in 0..2 {
        let app = open_app(load_json(&gp), None, ResultsDb::in_memory(), 5);
        assert_ne!(app.game.seed, sym.seed, "PUZ-5, PER-9: start {start}: the app used the puzzle with the wrong symmetry");
        assert_eq!(app.game, new_game_with_symmetry(first_seed(5), FIRST_MODE, FIRST_SYMMETRIC), "start {start}");
    }
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_symmetric_game_goes_through_the_file() {
    // PER-1, PER-2, PUZ-8: the symmetry of a game in progress goes through the file, and the app
    // continues the same game. The starting numbers in the file are symmetric.
    for mode in 0..MODES {
        let (g, _) = nearly_complete_sym(930 + mode as u64, mode, true);
        let dir = data_dir(&format!("sym-file-{mode}"));
        let path = dir.join("game.json");
        save_json(&path, &g).unwrap();
        let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(json["symmetric"], true, "PER-1: mode {mode}");
        let loaded: Game = load_json(&path).unwrap();
        std::fs::remove_dir_all(&dir).ok();
        assert!(cells().all(|c| loaded.given[c.0 as usize][c.1 as usize] == {
            let (i, j) = partner(c);
            loaded.given[i as usize][j as usize]
        }), "PUZ-8: mode {mode}");
        let app = open_app(Some(loaded), None, ResultsDb::in_memory(), 5);
        assert_eq!(app.game, g, "PER-2: mode {mode}");
        check_app(&app).unwrap();
    }
}

#[test]
fn a_results_database_of_version_2_gets_the_symmetry_false() {
    // ANA-1, PER-9, CMP-5: the rows of a version 2 database (with `generator`, with no `symmetric`)
    // get symmetric = false. The ids and the other values do not change. After the change, a symmetric
    // result goes into the same file with true. A row with a symmetric value that is not 0 or 1 is not
    // used (ANA-1), and it stays in the file.
    let dir = data_dir("ana1-v2");
    let path = dir.join(RESULTS_FILE);
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
             INSERT INTO results (id, finished_at, mode, givens, seconds, seed, generator) VALUES
                 (4, 1790000000, 6, 24, 900, 41, 2),
                 (7, 1790000001, 0, 44, 300, 42, 1);
             PRAGMA user_version = 2;",
        )
        .unwrap();
    }
    let mut db = ResultsDb::open(&path).unwrap();
    let rows = db.all().unwrap();
    let got: Vec<(i64, u8, u8, u64, u64, bool)> = rows.iter().map(|r| (r.id, r.mode, r.givens, r.seconds, r.seed, r.symmetric)).collect();
    assert_eq!(got, vec![(7, 0, 44, 300, 42, false), (4, 6, 24, 900, 41, false)], "ANA-1, PER-9");
    assert!(rows.iter().all(|r| r.hints == 0), "ANA-1, PER-9: the rows of a version 2 database get hints = 0");
    let new = db
        .insert(&sudoku::store::NewResult { finished_at: 1790000002, mode: 0, givens: 45, seconds: 200, seed: 43, generator: 2, symmetric: true, hints: 0 })
        .unwrap();
    assert!(new.symmetric, "CMP-5");
    drop(db);
    // A changed file: a symmetric value of 2 (the CHECK constraint is not there in a changed file).
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch("PRAGMA ignore_check_constraints = ON; INSERT INTO results (finished_at, mode, givens, seconds, seed, generator, symmetric) VALUES (1790000003, 1, 40, 100, 44, 2, 2);").unwrap();
    }
    for start in 0..2 {
        let app = open_app(None, None, ResultsDb::open(&path).unwrap(), 5);
        let seeds: Vec<(u64, bool)> = app.results.iter().map(|r| (r.seed, r.symmetric)).collect();
        assert_eq!(seeds, vec![(43, true), (42, false), (41, false)], "ANA-1, PER-9: start {start}");
        check_app(&app).unwrap();
    }
    let conn = rusqlite::Connection::open(&path).unwrap();
    let kept: i64 = conn.query_row("SELECT count(*) FROM results WHERE seed = 44", [], |r| r.get(0)).unwrap();
    assert_eq!(kept, 1, "ANA-1: the row that the app does not use stays in the file");
    std::fs::remove_dir_all(&dir).ok();
}

// ---- PUZ-5, PUZ-7, PUZ-12, PUZ-13, PER-5, CMP-5, ANA-4: generator version 3 and the saved games of
// the versions 1 and 2.

/// The generator version of the result row of `seed` in the results file (the column `generator`).
fn result_generator(path: &PathBuf, seed: u64) -> i64 {
    let conn = rusqlite::Connection::open(path).unwrap();
    conn.query_row("SELECT generator FROM results WHERE seed = ?1", [seed as i64], |r| r.get(0)).unwrap()
}

/// True if the two games have the same puzzle (the solution and the starting cells).
fn same_puzzle(a: &Game, b: &Game) -> bool {
    a.solution == b.solution && a.given == b.given
}

#[test]
fn the_current_version_is_3_and_a_new_game_has_it() {
    // PUZ-13, PUZ-5: the first start (no saved game) and a new game that replaces a saved game that
    // the app does not use have generator = 3 and the puzzle of version 3 of their key.
    assert_eq!(VERSION, 3, "PUZ-13: the current version");
    let first = open_app(None, None, ResultsDb::in_memory(), 5);
    let want = game_of_version(Version::V3, first_seed(5), FIRST_MODE, FIRST_SYMMETRIC);
    assert_eq!(first.game.generator, 3, "PUZ-13: the first start");
    assert_eq!(first.game, want, "PUZ-5, PUZ-13: the first start");
    // A saved game that the app does not use (PER-5): the new game has version 3.
    let bad = Game { seed: 999_999, ..new_game(960, 2) };
    let app = open_app(Some(bad), None, ResultsDb::in_memory(), 5);
    assert_eq!(app.game, want, "PUZ-13: the new game that replaces the saved data");
    // The background puzzle of the New game panel has the key of the panel, and Start new game gives
    // the puzzle of version 3 (PUZ-7).
    let mut d = Driver::from(first);
    d.button(Button::Menu);
    d.button(Button::NewGame);
    let (seed, mode, sym) = d.app.early_game().expect("PUZ-7: the panel starts the background puzzle");
    d.finger_tap(layout::ng_panel().x0 as f32 + 100.0, (layout::tb_bottom_y() + 56) as f32);
    assert_eq!(d.app.game, game_of_version(Version::V3, seed, mode, sym), "PUZ-7, PUZ-13: Start new game");
    check_app(&d.app).unwrap();
}

#[test]
fn a_saved_game_of_version_2_opens_and_continues() {
    // PUZ-5, PUZ-13, PER-5, PER-1: a saved game of version 2 that is the puzzle of its own key opens
    // with version 2, symmetric and not, at the first, a middle and the last mode. The player continues
    // it, and it keeps version 2 through the game file. Its puzzle is not the puzzle of version 3 of the
    // same key (PUZ-12): thus the app did not make the game again.
    for (seed, mode, sym) in [(930u64, 0u8, false), (931, 0, true), (932, 4, true), (933, 6, false), (934, 6, true)] {
        let what = format!("seed {seed}, mode {mode}, symmetric {sym}");
        let v2 = game_of_version(Version::V2, seed, mode, sym);
        assert_eq!(v2.generator, 2, "{what}");
        assert!(!same_puzzle(&v2, &game_of_version(Version::V3, seed, mode, sym)), "{what}: the two versions have the same puzzle");
        let dir = data_dir(&format!("v2-continue-{seed}"));
        let path = dir.join("game.json");
        save_json(&path, &v2).unwrap();
        let loaded: Game = load_json(&path).expect("a game file of version 2");
        let app = open_app(Some(loaded), None, ResultsDb::in_memory(), 5);
        assert_eq!(app.game, v2, "PUZ-5, PER-5: {what}: the app uses the game of version 2");
        check_app(&app).unwrap_or_else(|e| panic!("{what}: {e}"));
        // The player continues the game.
        let mut d = Driver::from(app);
        let cell = cells().find(|&c| !d.app.game.is_filled(c)).unwrap();
        let v = d.app.game.solution[cell.0 as usize][cell.1 as usize];
        d.write(cell, v);
        assert_eq!(d.app.game.value_at(cell), v, "{what}: the app did not read {v}");
        assert_eq!(d.app.game.generator, 2, "PUZ-13: {what}: the generator of a game does not change");
        save_json(&path, &d.app.game).unwrap();
        let again = open_app(load_json(&path), None, ResultsDb::in_memory(), 6);
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(again.game, d.app.game, "PER-1: {what}: the continued game of version 2");
        assert_eq!(again.game.generator, 2, "PUZ-13: {what}");
    }
}

#[test]
fn a_saved_game_with_the_puzzle_of_a_different_version_is_not_used() {
    // PUZ-5, PUZ-12, PER-5: the puzzle of version 2 with generator 3, and the puzzle of version 3 with
    // generator 2, of the same (seed, mode, symmetric): the app does not use them. The new game has
    // version 3 (PUZ-13). The correct games of the two versions are used.
    for (seed, mode, sym) in [(940u64, 2u8, false), (941, 5, true), (942, 3, true), (943, 6, false)] {
        let what = format!("seed {seed}, mode {mode}, symmetric {sym}");
        let v2 = game_of_version(Version::V2, seed, mode, sym);
        let v3 = game_of_version(Version::V3, seed, mode, sym);
        assert!(!same_puzzle(&v2, &v3), "{what}: the fixture needs two different puzzles");
        for good in [&v2, &v3] {
            assert_eq!(&open_app(Some(good.clone()), None, ResultsDb::in_memory(), 5).game, good, "PUZ-5: {what}: version {}", good.generator);
        }
        for (name, bad) in [("v2-puzzle-as-3", Game { generator: 3, ..v2.clone() }), ("v3-puzzle-as-2", Game { generator: 2, ..v3.clone() })] {
            assert_not_used(&bad, &format!("puz5-{name}-{seed}"));
            let app = open_app(Some(bad), None, ResultsDb::in_memory(), 5);
            assert_eq!(app.game.generator, 3, "PUZ-13: {what}, {name}");
        }
    }
}

#[test]
fn a_game_file_with_a_version_that_is_not_1_2_or_3_is_not_used() {
    // PUZ-5, PER-5: generator ∈ {1, 2, 3}. A game file with version 0, 4 or 255 is not correct: the
    // app does not use it, it does not stop, and the new game has version 3 (PUZ-13). The puzzle of
    // the file is a correct puzzle of version 3, thus only the version is wrong.
    let base = new_game(950, 3);
    for (k, version) in [0u8, 4, 255].into_iter().enumerate() {
        let bad = Game { generator: version, ..base.clone() };
        let err = check_game(&bad).expect_err("PUZ-5: a version that is not 1, 2 or 3");
        assert!(err.contains("PUZ-5"), "PUZ-5: version {version}: {err}");
        assert_rejected(&base, &bad, "PUZ-5", &format!("puz5-version-{version}"));
        // The same file as text, with the number of the version in JSON.
        let dir = data_dir(&format!("puz5-version-text-{version}"));
        let path = dir.join("game.json");
        let mut json = serde_json::to_value(&base).unwrap();
        json["generator"] = version.into();
        std::fs::write(&path, json.to_string()).unwrap();
        let app = open_app(load_json(&path), None, ResultsDb::in_memory(), 7 + k as u64);
        std::fs::remove_dir_all(&dir).ok();
        assert_ne!(app.game.seed, base.seed, "PER-5: version {version}: the app used the game");
        assert_eq!(app.game.generator, 3, "PUZ-13: version {version}");
        check_app(&app).unwrap();
    }
}

#[test]
fn a_completed_game_of_version_2_keeps_version_2_and_the_next_game_has_version_3() {
    // CMP-5, PUZ-13, PUZ-7: the result of a game of version 2 has generator 2 in the results file. The
    // background puzzle and the level buttons make games of version 3, symmetric and not. A completed
    // game of version 2 that opens from the save keeps version 2 (PER-1).
    use sudoku::layout::LevelButton;
    for (seed, mode, sym, b) in [(970u64, 3u8, false, LevelButton::Same), (971, 4, true, LevelButton::Easier), (972, 2, true, LevelButton::Harder)] {
        let what = format!("seed {seed}, mode {mode}, symmetric {sym}, {b:?}");
        let mut g = game_of_version(Version::V2, seed, mode, sym);
        let empty: Vec<Cell> = cells().filter(|&c| !g.is_filled(c)).collect();
        let (last, rest) = empty.split_last().unwrap();
        for &(r, c) in rest {
            g.add_number((r, c), g.solution[r as usize][c as usize]);
        }
        let dir = data_dir(&format!("v2-complete-{seed}"));
        let path = dir.join(RESULTS_FILE);
        let mut d = Driver::from(open_app(Some(g.clone()), None, ResultsDb::open(&path).unwrap(), 20));
        assert_eq!(d.app.game.generator, 2, "{what}: the app uses the game of version 2");
        d.wait(1500);
        write_last_number(&mut d, *last);
        assert_eq!(d.app.game.generator, 2, "PUZ-13: {what}: the completed game keeps its version");
        assert_eq!(result_generator(&path, seed), 2, "CMP-5, PUZ-13: {what}: the result has the version of its game");
        let row = d.app.results.iter().find(|r| r.seed == seed).expect("CMP-5: the result");
        assert_eq!((row.mode, row.symmetric, row.givens), (mode, sym, g.givens_count()), "CMP-5: {what}");
        check_app(&d.app).unwrap();
        // The completed game of version 2 opens from the save with version 2.
        let completed = d.app.game.clone();
        let reopened = open_app(Some(completed.clone()), None, ResultsDb::open(&path).unwrap(), 21);
        assert_eq!(reopened.game, completed, "PER-1: {what}: the completed game of version 2");
        // PUZ-7: the background puzzle is the Same mode with the symmetry of the game, and the level
        // button makes the puzzle of version 3.
        let (next, early_mode, early_sym) = d.app.early_game().expect("PUZ-7: the background puzzle at the completion");
        assert_eq!((early_mode, early_sym), (mode, sym), "PUZ-7: {what}");
        let r = layout::level_button(b);
        d.finger_tap((r.x0 + r.x1) as f32 / 2.0, (r.y0 + r.y1) as f32 / 2.0);
        let target = b.target(mode).unwrap();
        assert_eq!(d.app.game, game_of_version(Version::V3, next, target, sym), "PUZ-7, PUZ-13: {what}: the next game");
        assert_eq!(d.app.game.generator, 3, "PUZ-13: {what}");
        assert_eq!(result_generator(&path, seed), 2, "CMP-5: {what}: the row does not change");
        check_app(&d.app).unwrap();
        std::fs::remove_dir_all(&dir).ok();
    }
}

#[test]
fn the_results_of_all_versions_of_a_mode_are_one_group() {
    // ANA-4, CMP-8, ANA-1: rows of the generator versions 1, 2 and 3 at the same mode are in one group:
    // the app uses all of them, the average and the best time of the result card use all of them.
    let dir = data_dir("ana4-versions");
    let path = dir.join(RESULTS_FILE);
    let mut db = ResultsDb::open(&path).unwrap();
    for (k, (generator, seconds)) in [(1u8, 100u64), (2, 200), (3, 600), (2, 900)].into_iter().enumerate() {
        let mode = if k == 3 { 4 } else { 2 };
        db.insert(&sudoku::store::NewResult { finished_at: 1_790_000_000 + k as i64, mode, givens: 30, seconds, seed: 80 + k as u64, generator, symmetric: k % 2 == 0, hints: 0 }).unwrap();
    }
    drop(db);
    let (g, last) = nearly_complete(975, 2);
    let mut d = Driver::from(open_app(Some(g), None, ResultsDb::open(&path).unwrap(), 30));
    assert_eq!(d.app.results.len(), 4, "ANA-1: the app uses the rows of all versions");
    let mode_2: Vec<_> = d.app.results.iter().filter(|r| r.mode == 2).collect();
    assert_eq!(mode_2.len(), 3, "ANA-4: the versions 1, 2 and 3 of mode 2 are one group");
    assert_eq!(sudoku::app::average_seconds(mode_2.into_iter()), Some(300), "ANA-4: (100 + 200 + 600) / 3");
    write_last_number(&mut d, last);
    assert_eq!(result_generator(&path, 975), 3, "CMP-5: the result of a game of version 3");
    assert_eq!(
        d.app.card_comparison(),
        sudoku::app::Comparison::Earlier { average: 300, best: 100 },
        "CMP-8, ANA-4: the earlier results of mode 2 of all versions"
    );
    std::fs::remove_dir_all(&dir).ok();
}

// ---- PER-1, PER-2, PER-5, PER-9, HNT-6, HNT-8, HNT-9, ANA-1, CMP-5: the hint count, the revealed
// incorrect number and the column `hints` of the results.

/// A game of `seed` with one incorrect entry that does not clash (BRD-5), as the player writes it.
/// Returns the game, the cell and the digit.
fn game_with_incorrect(seed: u64) -> (Game, Cell, u8) {
    let mut g = new_game(seed, 2);
    let (cell, digit) = cells()
        .filter(|&c| !g.is_filled(c))
        .find_map(|c @ (r, k)| (1..=9u8).find(|&v| v != g.solution[r as usize][k as usize] && g.peer_digits(c) & mark_bit(v) == 0).map(|v| (c, v)))
        .expect("an empty cell with an incorrect digit that does not clash");
    g.add_number(cell, digit);
    assert!(check_game(&g).is_ok());
    (g, cell, digit)
}

/// Opens the Hint panel (the menu is closed) and taps the row "Reveal one number".
fn request_hint(d: &mut Driver) {
    if !d.app.ui.menu.is_open() {
        d.button(Button::Menu);
    }
    if d.app.ui.menu.panel() != Some(Panel::Hint) {
        d.button(Button::Hint);
    }
    assert_eq!(d.app.ui.menu.panel(), Some(Panel::Hint), "HNT-1: the panel is open");
    let (x, y) = pixel_center(layout::hint_row());
    d.finger_tap(x, y);
}

/// The game goes through a file, and the app opens with it (PER-1, PER-2).
fn through_file(g: &Game, name: &str, seed: u64) -> (App, serde_json::Value) {
    let dir = data_dir(name);
    let path = dir.join("game.json");
    save_json(&path, g).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    let app = open_app(load_json(&path), None, ResultsDb::in_memory(), seed);
    std::fs::remove_dir_all(&dir).ok();
    (app, json)
}

#[test]
fn the_hint_count_and_the_revealed_incorrect_number_go_through_the_file() {
    // PER-1, PER-2, HNT-6, HNT-7, HNT-8: the app saves `hints` and `incorrect_shown`, and it does not
    // save `revealed`. When the app opens again, `revealed = ∅` and no panel is open.
    let mut d = Driver::from(open_app(Some(new_game(950, 2)), None, ResultsDb::in_memory(), 5));
    request_hint(&mut d);
    request_hint(&mut d);
    assert_eq!((d.app.game.hints, d.app.ui.revealed.iter().count()), (2, 2), "HNT-4: two reveals");
    let (app, json) = through_file(&d.app.game, "hnt-file-revealed", 5);
    assert_eq!(json["hints"], 2, "PER-1: the name of INVARIANTS.md §0 in the file");
    assert!(json.get("incorrect_shown").is_none_or(|v| v.is_null()), "PER-1: no revealed incorrect number");
    assert!(!json.to_string().contains("revealed"), "PER-1: the app does not save `revealed`");
    assert_eq!(app.game, d.app.game, "PER-2: the same game, with the hint count");
    assert!(app.ui.revealed.is_empty(), "PER-2: revealed = ∅ after a start");
    assert_eq!((app.screen, app.ui.menu), (Screen::Game, sudoku::app::Menu::Open(None)), "PER-2");
    check_app(&app).unwrap();

    // An incorrect number with no clash, then a request: the revealed incorrect number goes through
    // the file with its cell and its digit.
    let (g, cell, digit) = game_with_incorrect(951);
    let mut d = Driver::from(open_app(Some(Game { hints: 3, ..g }), None, ResultsDb::in_memory(), 5));
    request_hint(&mut d);
    let shown = sudoku::game::IncorrectShown { cell, digit };
    assert_eq!(d.app.game.incorrect_shown, Some(shown), "HNT-3");
    let (app, json) = through_file(&d.app.game, "hnt-file-incorrect", 5);
    assert_eq!(json["hints"], 3, "PER-1");
    assert!(json.get("incorrect_shown").is_some_and(|v| !v.is_null()), "PER-1: the file has the revealed incorrect number");
    assert_eq!(app.game, d.app.game, "PER-2");
    assert_eq!(app.game.incorrect_shown, Some(shown), "PER-2: HNT-8 is true, thus the value stays");
    assert_eq!(app.game.hint_state(), sudoku::game::HintState::Incorrect, "HNT-2");
    check_app(&app).unwrap();

    // HNT-6: no upper limit. A count above 99 and the maximum of the app go through the file.
    for hints in [100, 81_000, u32::MAX] {
        let g = Game { hints, ..new_game(952, 3) };
        let (app, json) = through_file(&g, "hnt-file-max", 5);
        assert_eq!(json["hints"], hints, "PER-1: {hints}");
        assert_eq!(app.game, g, "HNT-6, PER-2: the app uses the count {hints}");
        check_app(&app).unwrap();
    }
}

#[test]
fn an_earlier_game_file_has_no_hint_count_and_no_revealed_incorrect_number() {
    // PER-9, PER-5: a game file with no `hints` and no `incorrect_shown` gets 0 and None. The app uses
    // the game, and two starts give the same result.
    let (g, _, _) = game_with_incorrect(953);
    let dir = data_dir("hnt-per9");
    let path = dir.join("game.json");
    let mut json = serde_json::to_value(&Game { hints: 4, incorrect_shown: None, ..g.clone() }).unwrap();
    assert_eq!(json["hints"], 4, "the name of INVARIANTS.md §0 in the file");
    json.as_object_mut().unwrap().remove("hints");
    json.as_object_mut().unwrap().remove("incorrect_shown");
    std::fs::write(&path, json.to_string()).unwrap();
    for start in 0..2 {
        let loaded: Game = load_json(&path).expect("a game file of an earlier version");
        assert_eq!((loaded.hints, loaded.incorrect_shown), (0, None), "PER-9: start {start}");
        let app = open_app(Some(loaded), None, ResultsDb::in_memory(), 5);
        assert_eq!(app.game, Game { hints: 0, incorrect_shown: None, ..g.clone() }, "PER-9: start {start}: the app uses the game");
        assert_eq!(app.game.hint_state(), sudoku::game::HintState::Available, "CLS-4: the incorrect entry is not revealed");
        check_app(&app).unwrap();
    }
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_game_file_with_a_bad_revealed_incorrect_number_opens_without_it() {
    // HNT-9, PER-5, PER-2: a game file whose `incorrect_shown` breaks HNT-8 opens with
    // `incorrect_shown = None`. All other values stay: the app does not make a new game.
    use sudoku::game::IncorrectShown;
    let (mut g, cell @ (r, c), digit) = game_with_incorrect(954);
    g.hints = 7;
    let empty = empty_cell(&g);
    let given = given_cell(&g);
    let correct = cells().find(|&p| !g.is_given(p) && g.is_filled(p) && p != cell);
    let mut with_correct = g.clone();
    let correct = correct.unwrap_or_else(|| {
        let e = empty_cell(&with_correct);
        with_correct.add_number(e, with_correct.solution[e.0 as usize][e.1 as usize]);
        e
    });
    let other = (1..=9).find(|&v| v != digit).unwrap();
    let (complete, _) = nearly_complete(955, 1);
    let complete = {
        let mut k = complete;
        let last = empty_cell(&k);
        k.add_number(last, k.solution[last.0 as usize][last.1 as usize]);
        assert!(k.completed);
        k
    };
    let complete_entry = cells().find(|&p| !complete.is_given(p)).unwrap();
    let cases: Vec<(&str, Game, IncorrectShown)> = vec![
        ("an empty cell", g.clone(), IncorrectShown { cell: empty, digit }),
        ("a starting number", g.clone(), IncorrectShown { cell: given, digit: g.value_at(given) }),
        ("a starting number with a different digit", g.clone(), IncorrectShown { cell: given, digit: (g.value_at(given) % 9) + 1 }),
        ("a different digit", g.clone(), IncorrectShown { cell, digit: other }),
        ("the digit of the solution", with_correct.clone(), IncorrectShown { cell: correct, digit: with_correct.value_at(correct) }),
        ("a complete game", complete.clone(), IncorrectShown { cell: complete_entry, digit: complete.value_at(complete_entry) }),
        ("a row out of the board", g.clone(), IncorrectShown { cell: (9, c), digit }),
        ("a column out of the board", g.clone(), IncorrectShown { cell: (r, 200), digit }),
        ("the digit 0", g.clone(), IncorrectShown { cell, digit: 0 }),
        ("the digit 10", g.clone(), IncorrectShown { cell, digit: 10 }),
    ];
    for (name, base, shown) in cases {
        let bad = Game { incorrect_shown: Some(shown), ..base.clone() };
        assert!(check_game(&bad).is_err(), "{name}: the value breaks HNT-8");
        let dir = data_dir(&format!("hnt9-{}", name.replace(' ', "-")));
        let path = dir.join("game.json");
        save_json(&path, &bad).unwrap();
        for start in 0..2 {
            let loaded: Game = load_json(&path).expect("the file is JSON of a game");
            assert_eq!(loaded, bad, "{name}");
            let app = open_app(Some(loaded), None, ResultsDb::in_memory(), 5);
            assert_eq!(app.game, Game { incorrect_shown: None, ..base.clone() }, "HNT-9: {name}, start {start}: the other values stay");
            assert!(app.ui.revealed.is_empty(), "PER-2: {name}");
            check_app(&app).unwrap_or_else(|e| panic!("{name}: {e}"));
        }
        std::fs::remove_dir_all(&dir).ok();
    }
    // The control: a valid value stays (PER-2).
    let good = Game { incorrect_shown: Some(IncorrectShown { cell, digit }), ..g.clone() };
    let (app, _) = through_file(&good, "hnt9-good", 5);
    assert_eq!(app.game, good, "PER-2: HNT-8 is true, thus the value stays");
}

#[test]
fn each_hint_request_that_changes_the_game_is_marked_for_saving() {
    // PER-1, PER-7, HNT-3, HNT-4: a reveal and the reveal of an incorrect number set `game_dirty`. A tap
    // on the gray row, on the box and on the lightbulb change nothing that is saved.
    let (g, cell, _) = game_with_incorrect(956);
    let mut d = Driver::from(open_app(Some(g), None, ResultsDb::in_memory(), 5));
    d.button(Button::Menu);
    d.button(Button::Hint);
    d.app.game_dirty = false;
    request_hint(&mut d);
    assert!(d.app.game.incorrect_shown.is_some() && d.app.game_dirty, "HNT-3, PER-1: the app saves the revealed incorrect number");
    d.app.game_dirty = false;
    request_hint(&mut d);
    let (x, y) = ((layout::hint_text_x() + 40) as f32, (layout::hint_box_top() + 30) as f32);
    d.pen_tap(x, y);
    d.button(Button::Hint);
    d.button(Button::Hint);
    assert!(!d.app.game_dirty, "HNT-2: the gray row, the box and the lightbulb change nothing that is saved");
    assert_eq!(d.app.ui.menu.panel(), Some(Panel::Hint));
    d.button(Button::Menu);
    d.erase(cell);
    assert_eq!(d.app.game.incorrect_shown, None, "HNT-8: the eraser removed the number");
    d.app.game_dirty = false;
    let before = d.app.game.hints;
    request_hint(&mut d);
    assert_eq!(d.app.game.hints, before + 1, "HNT-4");
    assert!(d.app.game_dirty, "PER-1: the app saves a reveal");
}

/// A results table of schema 3: the column `symmetric`, no column `hints`.
fn results_v3(path: &PathBuf) {
    let conn = rusqlite::Connection::open(path).unwrap();
    conn.execute_batch(
        "CREATE TABLE results (
             id          INTEGER PRIMARY KEY AUTOINCREMENT,
             finished_at INTEGER NOT NULL,
             mode        INTEGER NOT NULL CHECK (mode BETWEEN 0 AND 6),
             givens      INTEGER NOT NULL CHECK (givens BETWEEN 0 AND 81),
             seconds     INTEGER NOT NULL CHECK (seconds >= 0),
             seed        INTEGER NOT NULL,
             generator   INTEGER NOT NULL CHECK (generator >= 1),
             symmetric   INTEGER NOT NULL DEFAULT 0 CHECK (symmetric IN (0, 1))
         );
         INSERT INTO results (id, finished_at, mode, givens, seconds, seed, generator, symmetric) VALUES
             (3, 1790000000, 6, 24, 900, 61, 3, 1),
             (9, 1790000001, 0, 44, 300, 62, 2, 0);
         PRAGMA user_version = 3;",
    )
    .unwrap();
}

/// The column `hints` of the schema: (type, not null, default).
fn hints_column(path: &PathBuf) -> (String, i64, Option<String>) {
    let conn = rusqlite::Connection::open(path).unwrap();
    let mut stmt = conn.prepare("SELECT type, \"notnull\", dflt_value FROM pragma_table_info('results') WHERE name = 'hints'").unwrap();
    stmt.query_row([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap()
}

fn user_version(path: &PathBuf) -> i64 {
    rusqlite::Connection::open(path).unwrap().query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap()
}

#[test]
fn a_results_database_of_schema_3_gets_the_hints_0() {
    // ANA-1, PER-9: the database changes to schema 4. The column `hints` is a whole number, not null,
    // with the default 0. The rows of schema 3 get hints = 0, and their ids and other values do not
    // change. A row that an earlier version of the app adds after a downgrade gets 0.
    let dir = data_dir("ana1-v3");
    let path = dir.join(RESULTS_FILE);
    results_v3(&path);
    let db = ResultsDb::open(&path).unwrap();
    let got: Vec<(i64, u8, u8, u64, u64, bool, u64)> = db.all().unwrap().iter().map(|r| (r.id, r.mode, r.givens, r.seconds, r.seed, r.symmetric, r.hints)).collect();
    assert_eq!(got, vec![(9, 0, 44, 300, 62, false, 0), (3, 6, 24, 900, 61, true, 0)], "ANA-1, PER-9");
    drop(db);
    assert_eq!(user_version(&path), 4, "ANA-1: schema 4");
    assert_eq!(hints_column(&path), ("INTEGER".to_string(), 1, Some("0".to_string())), "ANA-1: the column hints");
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch("INSERT INTO results (finished_at, mode, givens, seconds, seed, generator, symmetric) VALUES (1790000002, 2, 30, 100, 63, 3, 0);").unwrap();
    }
    for start in 0..2 {
        let app = open_app(None, None, ResultsDb::open(&path).unwrap(), 5);
        let rows: Vec<(u64, u64)> = app.results.iter().map(|r| (r.seed, r.hints)).collect();
        assert_eq!(rows, vec![(63, 0), (62, 0), (61, 0)], "ANA-1, PER-9: start {start}");
        check_app(&app).unwrap();
    }
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_databases_of_schema_1_and_2_get_the_hints_0() {
    // ANA-1, PER-9: a database of schema 1 (user_version 0) and of schema 2 changes to schema 4, and
    // each row gets hints = 0.
    for (name, version, columns, values) in [
        ("ana1-hints-v1", 0, "", ""),
        ("ana1-hints-v2", 2, ", generator INTEGER NOT NULL CHECK (generator >= 1)", ", 2"),
    ] {
        let dir = data_dir(name);
        let path = dir.join(RESULTS_FILE);
        {
            let conn = rusqlite::Connection::open(&path).unwrap();
            conn.execute_batch(&format!(
                "CREATE TABLE results (
                     id          INTEGER PRIMARY KEY AUTOINCREMENT,
                     finished_at INTEGER NOT NULL,
                     mode        INTEGER NOT NULL,
                     givens      INTEGER NOT NULL,
                     seconds     INTEGER NOT NULL,
                     seed        INTEGER NOT NULL{columns}
                 );
                 INSERT INTO results (finished_at, mode, givens, seconds, seed{}) VALUES (1790000000, 1, 40, 300, 71{values}), (1790000001, 3, 30, 200, 72{values});
                 PRAGMA user_version = {version};",
                if columns.is_empty() { "" } else { ", generator" }
            ))
            .unwrap();
        }
        let app = open_app(None, None, ResultsDb::open(&path).unwrap(), 5);
        let rows: Vec<(u64, u64)> = app.results.iter().map(|r| (r.seed, r.hints)).collect();
        assert_eq!(rows, vec![(72, 0), (71, 0)], "ANA-1, PER-9: {name}");
        check_app(&app).unwrap();
        drop(app);
        assert_eq!(user_version(&path), 4, "ANA-1: {name}");
        assert_eq!(hints_column(&path).1, 1, "ANA-1: {name}: not null");
        std::fs::remove_dir_all(&dir).ok();
    }
}

#[test]
fn each_hint_count_that_the_app_writes_is_read_again() {
    // ANA-1, HNT-6, CMP-5: `hints` has no upper limit. The app reads each count that it can write, also
    // above 81. A row with a negative count breaks ANA-1: the app does not use it, and it stays in the
    // file.
    let dir = data_dir("ana1-hints-max");
    let path = dir.join(RESULTS_FILE);
    let mut db = ResultsDb::open(&path).unwrap();
    let counts = [0u32, 1, 81, 82, 99, 100, 1000, u32::MAX];
    for (i, &hints) in counts.iter().enumerate() {
        let row = db
            .insert(&sudoku::store::NewResult { finished_at: 1_790_000_000 + i as i64, mode: (i % 7) as u8, givens: 30, seconds: 60, seed: 500 + i as u64, generator: 3, symmetric: false, hints })
            .unwrap();
        assert_eq!(row.hints, u64::from(hints), "CMP-5");
    }
    drop(db);
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch("PRAGMA ignore_check_constraints = ON; INSERT INTO results (finished_at, mode, givens, seconds, seed, generator, symmetric, hints) VALUES (1790000100, 1, 40, 100, 599, 3, 0, -1);").unwrap();
    }
    for start in 0..2 {
        let app = open_app(None, None, ResultsDb::open(&path).unwrap(), 5);
        let mut got: Vec<u64> = app.results.iter().map(|r| r.hints).collect();
        got.reverse();
        assert_eq!(got, counts.map(u64::from).to_vec(), "ANA-1, HNT-6: start {start}: each count is read");
        assert!(app.results.iter().all(|r| r.seed != 599), "ANA-1: the row with a negative count is not used");
        check_app(&app).unwrap();
    }
    let conn = rusqlite::Connection::open(&path).unwrap();
    let kept: i64 = conn.query_row("SELECT count(*) FROM results WHERE seed = 599", [], |r| r.get(0)).unwrap();
    assert_eq!(kept, 1, "ANA-1: the row stays in the file");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_completion_writes_the_hint_count_of_the_game() {
    // CMP-5, HNT-6, ANA-1, PER-4: the result row gets the `hints` of the game, also the maximum of the
    // app, and the app reads it again from the file. A completion by a reveal counts that reveal.
    let dir = data_dir("cmp5-hints");
    let path = dir.join(RESULTS_FILE);
    let (g, last) = nearly_complete(957, 2);
    let mut d = Driver::from(open_app(Some(Game { hints: 150, ..g }), None, ResultsDb::open(&path).unwrap(), 30));
    write_last_number(&mut d, last);
    assert_eq!(d.app.results[0].hints, 150, "CMP-5");
    drop(d);
    let (g, _) = nearly_complete(958, 4);
    let mut d = Driver::from(open_app(Some(Game { hints: u32::MAX - 1, ..g }), None, ResultsDb::open(&path).unwrap(), 31));
    request_hint(&mut d);
    assert!(d.app.game.completed, "HNT-4: the reveal completed the puzzle");
    assert_eq!(d.app.game.hints, u32::MAX, "HNT-6: the reveal adds 1");
    assert_eq!(d.app.ui.menu.panel(), None, "HNT-4, CMP-3: the panel closes");
    drop(d);
    let app = open_app(None, None, ResultsDb::open(&path).unwrap(), 32);
    let rows: Vec<(u64, u64)> = app.results.iter().map(|r| (r.seed, r.hints)).collect();
    assert_eq!(rows, vec![(958, u64::from(u32::MAX)), (957, 150)], "CMP-5, ANA-1: the counts in the file");
    std::fs::remove_dir_all(&dir).ok();
}

/// HNT-12, written from INVARIANTS.md: the text of the average s / n of the hints of n results.
fn model_average_hints(s: u128, n: u128) -> String {
    if n == 0 {
        return "–".to_string();
    }
    // The average in hundredths, rounded with ties to even: compare the remainder with one half.
    let (q, rest) = ((100 * s) / n, (100 * s) % n);
    let h = match (2 * rest).cmp(&n) {
        std::cmp::Ordering::Less => q,
        std::cmp::Ordering::Greater => q + 1,
        std::cmp::Ordering::Equal => q + q % 2,
    };
    let text = format!("{}.{:02}", h / 100, h % 100);
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

#[test]
fn the_average_of_the_hints_has_at_most_two_decimals_and_no_end_zeros() {
    // HNT-12, ANA-4: the examples of the documents, and each sum and quantity in a range against the
    // model and the check of the app.
    use sudoku::app::average_hints_text;
    for (s, n, want) in [(0, 0, "–"), (0, 3, "0"), (4, 2, "2"), (2, 7, "0.29"), (5, 2, "2.5"), (1, 8, "0.12"), (3, 8, "0.38"), (5, 8, "0.62"), (1, 3, "0.33"), (2, 3, "0.67"), (100, 1, "100")] {
        assert_eq!(average_hints_text(s, n), want, "HNT-12: {s} / {n}");
        assert_eq!(model_average_hints(s, n), want, "the model: {s} / {n}");
    }
    for n in 0..=60u128 {
        for s in 0..=400u128 {
            let text = average_hints_text(s, n);
            assert_eq!(text, model_average_hints(s, n), "HNT-12: {s} / {n}");
            sudoku::invariants::check_average_hints(s, n).unwrap_or_else(|e| panic!("{s} / {n}: {e}"));
        }
    }
    // Counts with no upper limit (HNT-6): large sums of large counts.
    let max = u128::from(u32::MAX);
    for (s, n) in [(max, 1), (max * 3 + 1, 8), (max * 1000, 999), (u128::from(u64::MAX), 7)] {
        assert_eq!(average_hints_text(s, n), model_average_hints(s, n), "HNT-12: {s} / {n}");
    }
    // ANA-4: the average of the rows of the results, for each mode.
    let rows: Vec<sudoku::store::ResultRow> = [0u64, 1, 0, 0, 0, 0, 0, 0]
        .iter()
        .enumerate()
        .map(|(i, &hints)| sudoku::store::ResultRow { id: i as i64, finished_at: i as i64, mode: 2, givens: 30, seconds: 60, seed: i as u64, symmetric: false, hints })
        .collect();
    assert_eq!(sudoku::app::average_hints(rows.iter()), "0.12", "HNT-12: 1 hint in 8 games");
    assert_eq!(sudoku::app::average_hints(std::iter::empty()), "–", "HNT-12, ANA-4: no results");
}
