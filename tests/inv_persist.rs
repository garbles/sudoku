//! Tests for the rules of the saved data and the results: PER-3, PER-4, PER-5, PER-7 and ANA-1. The
//! platform part of PER-7 and CLK-6 (the main loop on Linux) is in tests/qtfb_e2e.rs.
//!
//! `invariants::check_app` runs after each input only in debug builds. These tests assert the rules
//! directly, thus they also examine the release build.

mod common;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use common::*;
use sudoku::app::{new_game, App, Env, Panel, Screen};
use sudoku::game::{cells, mark_bit, peers, Cell, Change, Game};
use sudoku::generator::{count_solutions, generate_version, FIRST_MODE, MODES, VERSION};
use sudoku::invariants::{check_app, check_game};
use sudoku::layout::{self, Button};
use sudoku::settings::{Color, Settings};
use sudoku::store::{load_json, save_json, ResultsDb, RESULTS_FILE};

// ---- Helpers (copies of the helpers in tests/behavior.rs and tests/rules.rs)

fn empty_cell(app: &App) -> Cell {
    cells().find(|&c| !app.game.is_filled(c)).unwrap()
}

fn given_cell(g: &Game) -> Cell {
    cells().find(|&c| g.is_given(c)).unwrap()
}

fn color_point(section: usize, i: usize) -> (f32, f32) {
    let (gx, gy) = layout::color_cell(section, i);
    (gx as f32 + 56.0, gy as f32 + 40.0)
}

fn switch_point(i: usize) -> (f32, f32) {
    let r = layout::settings_row(i);
    ((r.x0 + r.x1) as f32 / 2.0, (r.y0 + r.y1) as f32 / 2.0)
}

fn center(r: layout::Rect) -> (f32, f32) {
    ((r.x0 + r.x1) as f32 / 2.0, (r.y0 + r.y1) as f32 / 2.0)
}

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
    App::new(game, settings, db, Box::new(TestEnv { time: 1_790_000_000 + seed as i64 * 1000, seed }))
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
    // PER-3: the colors, `last_mode` (from Start new game) and the two switches go through the
    // settings file. DIF-3: the slider opens at the kept mode.
    let mut d = Driver::new(61);
    d.button(Button::Menu);
    d.button(Button::Colors);
    let (x, y) = color_point(0, 2);
    d.finger_tap(x, y);
    let (x, y) = color_point(2, 7);
    d.finger_tap(x, y);
    d.button(Button::Settings);
    for i in [0, 1] {
        let (x, y) = switch_point(i);
        d.finger_tap(x, y);
    }
    d.button(Button::NewGame);
    d.finger_tap(d.app.ng_stops[4], layout::ng_slider_y() as f32);
    d.finger_tap(layout::ng_pl() as f32 + 50.0, layout::ng_start_y() as f32);
    let s = d.app.settings.clone();
    assert_eq!((s.color_starting, s.color_clash, s.color_highlight), (Color::Yellow, Color::Red, Color::Magenta));
    assert_eq!(s.last_mode, 4);
    assert!(!s.highlight_on && !s.auto_remove_on);
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
    let (x, y) = center(layout::all_games_button());
    d.finger_tap(x, y);
    let (x, y) = center(layout::bin_button(0));
    d.finger_tap(x, y);
    let (x, y) = center(layout::dialog_delete());
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
    // PER-5, MRK-4, MRK-9: the `removed` list of AddNumber is all peers with the mark, or no cell.
    // Here it also has a peer without the mark.
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
    assert_eq!(base.undo.last(), Some(&Change::AddNumber { cell, digit: d, removed: vec![p1] }));
    assert!(check_game(&base).is_ok());
    let mut bad = base.clone();
    *bad.undo.last_mut().unwrap() = Change::AddNumber { cell, digit: d, removed: vec![p1, p2] };
    assert_rejected(&base, &bad, "MRK-4", "removed-list-wrong");

    // The same number with automatic removal off (MRK-9) is correct.
    let mut off = new_game(903, 2);
    off.toggle_mark(p1, d);
    off.add_number_with(cell, d, false);
    assert!(check_game(&off).is_ok());
    assert_eq!(open_app(Some(off.clone()), None, ResultsDb::in_memory(), 5).game, off);
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
    // PER-5, DIF-1: `last_mode` 5 or 9. COL-5: two sections with the same color (not White).
    // A correct file with other values than the defaults is used.
    let dir = data_dir("bad-settings");
    let path = dir.join("settings.json");
    let good = Settings { color_starting: Color::White, color_clash: Color::White, last_mode: 0, highlight_on: false, ..Settings::default() };
    let bad = [
        ("last-mode-9", Settings { last_mode: 9, ..good.clone() }),
        ("last-mode-5", Settings { last_mode: MODES, ..good.clone() }),
        ("same-color", Settings { color_highlight: Color::Blue, color_starting: Color::Blue, ..good.clone() }),
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

#[test]
fn each_change_of_the_board_and_the_settings_is_marked_for_saving() {
    // PER-7: each change to the board sets `game_dirty`; each change of the settings sets
    // `settings_dirty`. Input with no change sets neither. The platform saves when a flag is set.
    let mut d = Driver::new(62);
    let clear = |d: &mut Driver| {
        d.app.game_dirty = false;
        d.app.settings_dirty = false;
    };
    let cell = empty_cell(&d.app);
    type Step = (&'static str, fn(&mut Driver, Cell), bool, bool);
    let steps: [Step; 9] = [
        ("mark", |d, c| d.tap_micro(c, 5), true, false),
        ("number", |d, c| d.write(c, d.app.game.solution[c.0 as usize][c.1 as usize]), true, false),
        ("erase", |d, c| d.erase(c), true, false),
        ("menu", |d, _| d.button(Button::Menu), false, false),
        ("undo", |d, _| d.button(Button::Undo), true, false),
        ("redo", |d, _| d.button(Button::Redo), true, false),
        ("color", |d, _| {
            d.button(Button::Colors);
            let (x, y) = color_point(1, 8);
            d.finger_tap(x, y);
        }, false, true),
        ("switch", |d, _| {
            d.button(Button::Settings);
            let (x, y) = switch_point(1);
            d.pen_tap(x, y);
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
    assert_eq!((d.app.game.mode, d.app.settings.color_clash, d.app.settings.auto_remove_on), (0, Color::Orange, false));
    // A finger on the board and a tap on a starting number change nothing that is saved.
    clear(&mut d);
    let (x, y) = micro_center(empty_cell(&d.app), 5);
    d.finger_tap(x, y);
    d.tap_cell(given_cell(&d.app.game));
    assert!(!d.app.game_dirty && !d.app.settings_dirty);
    assert_eq!((d.app.screen, d.app.ui.panel), (Screen::Game, Panel::None));
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
    let (x, y) = center(layout::all_games_button());
    d.finger_tap(x, y);
    let (x, y) = center(layout::bin_button(1));
    d.finger_tap(x, y);
    let victim = d.app.list.confirm.clone().expect("the delete dialog");
    let (x, y) = center(layout::dialog_delete());
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
