//! Tests for the rules of INVARIANTS.md about the puzzle (§1), the cells (§2) and the clashes (§4).
//! Each test gives the rules that it examines. `invariants::check_app` runs only in debug builds, thus
//! these tests assert the rules directly. They also pass with `cargo test --release`.

mod common;

use common::*;
use sudoku::app::{new_game, App, Panel};
use sudoku::game::{cells, mark_bit, peers, Cell, Game, ALL_MARKS};
use sudoku::generator::{count_solutions, generate, is_valid_solution, MODES, MODE_LEVELS};
use sudoku::solver::{grade, Level};
use sudoku::layout::{self, Button};
use sudoku::settings::Settings;
use sudoku::store::ResultsDb;

type Grid = [[u8; 9]; 9];

fn empty_cell(app: &App) -> Cell {
    cells().find(|&c| !app.game.is_filled(c)).unwrap()
}

fn given_cell(app: &App) -> Cell {
    cells().find(|&c| app.game.is_given(c)).unwrap()
}

fn start_new_game(d: &mut Driver) {
    d.finger_tap(1100.0, (layout::tb_bottom_y() + 56) as f32);
}

/// Opens the New game panel, selects `mode` on the slider and starts a new game.
fn start_new_game_in_mode(d: &mut Driver, mode: u8) {
    if !d.app.ui.menu_open {
        d.button(Button::Menu);
    }
    if d.app.ui.panel != Panel::NewGame {
        d.button(Button::NewGame);
    }
    let stops = d.app.ng_stops;
    d.finger_tap(stops[mode as usize], layout::ng_slider_y() as f32);
    assert_eq!(d.app.ui.slider, mode);
    start_new_game(d);
    assert_eq!(d.app.game.mode, mode);
}

/// An app that starts with no saved game and the last mode `mode`.
fn app_in_mode(mode: u8, seed: u64) -> App {
    let s = Settings { last_mode: mode, ..Settings::default() };
    App::new(None, Some(s), ResultsDb::in_memory(), Box::new(TestEnv { time: 1_790_000_000, seed }))
}

/// The 27 units (9 rows, 9 columns, 9 boxes) as lists of cells. This does not use the code of the app.
fn units() -> Vec<Vec<(usize, usize)>> {
    let mut u = Vec::new();
    for i in 0..9 {
        u.push((0..9).map(|j| (i, j)).collect());
        u.push((0..9).map(|j| (j, i)).collect());
        u.push((0..9).map(|j| (i / 3 * 3 + j / 3, i % 3 * 3 + j % 3)).collect());
    }
    u
}

/// Each unit has the digits 1 to 9 one time. This does not use the code of the app.
fn is_complete_sudoku(g: &Grid) -> bool {
    units().iter().all(|u| {
        let mut seen = [false; 10];
        u.iter().all(|&(r, c)| {
            let v = g[r][c] as usize;
            (1..=9).contains(&v) && !std::mem::replace(&mut seen[v], true)
        })
    })
}

/// A simple backtracking solver. It counts the solutions up to `limit` and keeps the first one.
fn solve_grid(g: &mut Grid, limit: usize, count: &mut usize, first: &mut Option<Grid>) {
    if *count >= limit {
        return;
    }
    let Some((r, c)) = (0..81).map(|i| (i / 9, i % 9)).find(|&(r, c)| g[r][c] == 0) else {
        *count += 1;
        if first.is_none() {
            *first = Some(*g);
        }
        return;
    };
    for v in 1..=9u8 {
        let ok = (0..9).all(|k| {
            g[r][k] != v && g[k][c] != v && g[r / 3 * 3 + k / 3][c / 3 * 3 + k % 3] != v
        });
        if ok {
            g[r][c] = v;
            solve_grid(g, limit, count, first);
            g[r][c] = 0;
        }
    }
}

fn independent_count(givens: &Grid, limit: usize) -> (usize, Option<Grid>) {
    let mut g = *givens;
    let (mut count, mut first) = (0, None);
    solve_grid(&mut g, limit, &mut count, &mut first);
    (count, first)
}

fn givens_of(g: &Game) -> Grid {
    let mut out = [[0u8; 9]; 9];
    for (r, c) in cells() {
        if g.is_given((r, c)) {
            out[r as usize][c as usize] = g.value_at((r, c));
        }
    }
    out
}

/// The clashing cells, from the values only. This does not use `Game::is_clashing`.
fn clash_set(g: &Game) -> Vec<Cell> {
    cells()
        .filter(|&(r, c)| {
            let v = g.value[r as usize][c as usize];
            v != 0
                && cells().any(|(i, j)| {
                    (i, j) != (r, c)
                        && (i == r || j == c || (i / 3 == r / 3 && j / 3 == c / 3))
                        && g.value[i as usize][j as usize] == v
                })
        })
        .collect()
}

/// CLS-1: the clashes that the app gives are the same as the clashes from the values.
fn assert_clashes_agree(g: &Game, step: &str) {
    let app: Vec<Cell> = cells().filter(|&c| g.is_clashing(c)).collect();
    assert_eq!(app, clash_set(g), "CLS-1: after {step}");
}

/// CLS-3: each pair of clashing cells includes one or more entries.
fn assert_each_clash_has_an_entry(g: &Game) {
    for a in cells() {
        for b in peers(a) {
            if g.is_filled(a) && g.value_at(a) == g.value_at(b) {
                assert!(g.is_entry(a) || g.is_entry(b), "CLS-3: {a:?} and {b:?} are two starting numbers that clash");
            }
        }
    }
}

/// PUZ-3, CLS-3: each starting number is the solution, and no two starting numbers clash.
fn assert_givens_agree(g: &Game) {
    for (r, c) in cells() {
        if g.is_given((r, c)) {
            assert_eq!(g.value_at((r, c)), g.solution[r as usize][c as usize], "PUZ-3 at ({r},{c})");
        }
    }
    let givens = givens_of(g);
    for a in cells() {
        for b in peers(a) {
            let (va, vb) = (givens[a.0 as usize][a.1 as usize], givens[b.0 as usize][b.1 as usize]);
            assert!(va == 0 || va != vb, "CLS-3: starting numbers at {a:?} and {b:?} clash");
        }
    }
}

/// The type of a cell (BRD-1). `None` if the cell is none of the four types.
#[derive(Debug, PartialEq, Eq)]
enum Kind {
    Given,
    Entry,
    Empty,
    EmptyWithMarks,
}

fn kind(g: &Game, (r, c): Cell) -> Option<Kind> {
    let (v, m, given) = (g.value_at((r, c)), g.marks_at((r, c)), g.is_given((r, c)));
    if m & !ALL_MARKS != 0 || v > 9 {
        return None;
    }
    match (given, v, m) {
        (true, 1..=9, 0) => Some(Kind::Given),
        (true, _, _) => None,
        (false, 1..=9, _) => Some(Kind::Entry),
        (false, 0, 0) => Some(Kind::Empty),
        (false, 0, _) => Some(Kind::EmptyWithMarks),
        _ => None,
    }
}

fn assert_cell_types(g: &Game, step: &str) {
    for cell in cells() {
        assert!(kind(g, cell).is_some(), "BRD-1: after {step}, cell {cell:?} has no type: {:?}", (g.value_at(cell), g.marks_at(cell), g.is_given(cell)));
    }
}

#[test]
fn each_solution_is_a_complete_correct_sudoku() {
    // PUZ-1: for many seeds and all modes, each row, column and box of `solution` has 1–9 one time.
    for mode in 0..MODES {
        for seed in 0..12u64 {
            let p = generate(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15), mode);
            assert!(is_complete_sudoku(&p.solution), "PUZ-1: seed {seed} mode {mode}");
            assert!(is_valid_solution(&p.solution));
        }
    }
    // The check of the app refuses a grid that is not a complete, correct sudoku.
    let sol = generate(7, 2).solution;
    let mut swap = sol;
    swap[0].swap(0, 1);
    assert!(!is_complete_sudoku(&swap) && !is_valid_solution(&swap), "PUZ-1: a swap in a row");
    let mut zero = sol;
    zero[4][4] = 0;
    assert!(!is_valid_solution(&zero), "PUZ-1: an empty cell");
    let mut ten = sol;
    ten[8][8] = 10;
    assert!(!is_valid_solution(&ten), "PUZ-1: a value of 10");
    let shifted: Grid = std::array::from_fn(|r| std::array::from_fn(|c| ((r + c) % 9 + 1) as u8));
    assert!(!is_valid_solution(&shifted), "PUZ-1: rows and columns are correct, boxes are not");
}

#[test]
fn the_starting_numbers_have_only_one_solution() {
    // PUZ-2: the starting numbers have only one solution, and that solution is `solution`.
    for mode in 0..MODES {
        for seed in [1u64, 99, 12345] {
            let p = generate(seed, mode);
            assert_eq!(count_solutions(&p.givens, 2), 1, "PUZ-2: seed {seed} mode {mode}");
            let (n, first) = independent_count(&p.givens, 2);
            assert_eq!(n, 1, "PUZ-2: the independent solver finds {n} solutions, seed {seed} mode {mode}");
            assert_eq!(first, Some(p.solution), "PUZ-2: the only solution is `solution`");
        }
    }
    // `count_solutions` finds more than one solution.
    assert_eq!(count_solutions(&[[0; 9]; 9], 2), 2, "an empty grid");
    let p = generate(5, 4);
    let mut givens = p.givens;
    let mut removed = false;
    for (r, c) in cells() {
        let (r, c) = (r as usize, c as usize);
        if givens[r][c] != 0 {
            let v = std::mem::replace(&mut givens[r][c], 0);
            if count_solutions(&givens, 2) == 2 {
                assert_eq!(independent_count(&givens, 2).0, 2, "the independent solver also finds two");
                removed = true;
                break;
            }
            givens[r][c] = v;
        }
    }
    assert!(removed, "the removal of one starting number of a hard puzzle gives two solutions");
    // A full correct grid has one solution; a grid with a clash has none.
    assert_eq!(count_solutions(&p.solution, 2), 1);
    let mut clash = p.givens;
    let (r, c) = cells().map(|(r, c)| (r as usize, c as usize)).find(|&(r, c)| clash[r][c] == 0).unwrap();
    let v = (0..9).map(|k| clash[r][k]).find(|&v| v != 0).unwrap();
    clash[r][c] = v;
    assert_eq!(count_solutions(&clash, 2), 0);
}

#[test]
fn each_starting_number_is_the_solution_in_each_mode() {
    // PUZ-3, CLS-3: through the app. App::new in each mode, and Start new game in each mode.
    for mode in 0..MODES {
        let app = app_in_mode(mode, 40 + mode as u64);
        assert_eq!(app.game.mode, mode);
        assert_givens_agree(&app.game);
    }
    let mut d = Driver::new(41);
    for mode in (0..MODES).rev().chain(0..MODES) {
        start_new_game_in_mode(&mut d, mode);
        assert_givens_agree(&d.app.game);
        sudoku::invariants::check_app(&d.app).unwrap();
    }
}

#[test]
fn seed_and_mode_set_the_full_game() {
    // PUZ-5: the game after Start new game is the game of its (seed, mode). The same pair always gives
    // the same starting numbers and the same solution.
    let mut d = Driver::new(42);
    for mode in 0..MODES {
        start_new_game_in_mode(&mut d, mode);
        let g = &d.app.game;
        let again = new_game(g.seed, g.mode);
        assert_eq!((again.given, again.value, again.solution), (g.given, g.value, g.solution), "PUZ-5: mode {mode}");
        assert_eq!(generate(g.seed, mode), generate(g.seed, mode));
        let p = generate(g.seed, mode);
        assert_eq!((givens_of(g), g.solution), (p.givens, p.solution));
    }
    // A different pair gives a different puzzle (the pair is not ignored).
    let seed = d.app.game.seed;
    assert_ne!(generate(seed, 0), generate(seed, 1));
    assert_ne!(generate(seed, 0), generate(seed ^ 1, 0));
    // A result row keeps (seed, mode). The pair gives the same game again.
    start_new_game_in_mode(&mut d, 0);
    let g = d.app.game.clone();
    solve(&mut d);
    assert!(d.app.game.completed);
    let row = d.app.results[0].clone();
    let again = new_game(row.seed, row.mode);
    assert_eq!((again.given, again.solution), (g.given, g.solution), "PUZ-5: the result row gives the game again");
}

#[test]
fn the_quantity_of_starting_numbers_comes_from_the_game() {
    // PUZ-6: the quantity in each result is the quantity of the game, which the generator sets for the
    // mode. This test cannot show that no other part of the app uses a fixed quantity (that is a rule
    // about the code). It shows only that the result agrees with the game for different modes.
    let mut d = Driver::new(43);
    let mut counts = Vec::new();
    for mode in [0u8, 4] {
        start_new_game_in_mode(&mut d, mode);
        let g = d.app.game.clone();
        solve(&mut d);
        assert!(d.app.game.completed);
        let row = d.app.results.iter().find(|r| r.seed == g.seed).unwrap();
        let generated = generate(g.seed, mode).givens_count();
        assert_eq!(row.mode, mode);
        assert_eq!(row.givens, g.givens_count(), "PUZ-6: result and game, mode {mode}");
        assert_eq!(row.givens as usize, generated, "PUZ-6: result and generator, mode {mode}");
        counts.push(row.givens);
    }
    assert!(counts[0] > counts[1], "PUZ-6: the quantity changes with the mode: {counts:?}");
    // The generator sets the most difficult technique of each mode, and each puzzle needs it.
    for seed in [3u64, 8] {
        for m in 0..MODES {
            let p = generate(seed, m);
            assert_eq!(grade(&p.givens, Level::Chains), Some(MODE_LEVELS[m as usize]), "PUZ-6: seed {seed} mode {m}");
        }
    }
}

#[test]
fn clashes_follow_the_values_after_each_change_and_a_new_game() {
    // CLS-1, CLS-3: after each change (a new number, an erasure, undo, redo and a new game) the set
    // of clashing cells is the set from the values. Each clash includes an entry.
    let mut d = Driver::new(44);
    assert_clashes_agree(&d.app.game, "the start");
    let given = given_cell(&d.app);
    let v = d.app.game.value_at(given);
    let cells_v: Vec<Cell> = peers(given).filter(|&p| !d.app.game.is_filled(p)).take(2).collect();
    d.write(cells_v[0], v);
    assert_eq!(d.app.game.value_at(cells_v[0]), v);
    assert_clashes_agree(&d.app.game, "a new number");
    assert!(d.app.game.is_clashing(given) && d.app.game.is_clashing(cells_v[0]));
    assert_each_clash_has_an_entry(&d.app.game);
    d.write(cells_v[1], v);
    assert_clashes_agree(&d.app.game, "a second clashing number");
    assert_each_clash_has_an_entry(&d.app.game);
    d.erase(cells_v[0]);
    assert_clashes_agree(&d.app.game, "an erasure");
    d.button(Button::Menu);
    d.button(Button::Undo);
    assert_clashes_agree(&d.app.game, "undo");
    assert_each_clash_has_an_entry(&d.app.game);
    d.button(Button::Undo);
    assert_clashes_agree(&d.app.game, "a second undo");
    d.button(Button::Redo);
    assert_clashes_agree(&d.app.game, "redo");
    assert!(!clash_set(&d.app.game).is_empty(), "the board has a clash before the new game");
    // A new game: no cell clashes.
    d.button(Button::NewGame);
    start_new_game(&mut d);
    assert_clashes_agree(&d.app.game, "a new game");
    assert!(cells().all(|c| !d.app.game.is_clashing(c)), "CLS-1: a cell clashes in the new game");
}

#[test]
fn starting_numbers_never_clash() {
    // CLS-3, PUZ-3: for many seeds and all modes, no two starting numbers clash. Thus each clash
    // includes one or more entries.
    for mode in 0..MODES {
        for seed in 100..112u64 {
            let g = new_game(seed, mode);
            assert_givens_agree(&g);
            assert!(clash_set(&g).is_empty(), "CLS-3: seed {seed} mode {mode}");
        }
    }
    // Clashes made in the app with entries in each unit of a starting number.
    let mut d = Driver::new(45);
    let given = given_cell(&d.app);
    let v = d.app.game.value_at(given);
    let row = cells().find(|&c| c.0 == given.0 && !d.app.game.is_filled(c)).unwrap();
    let col = cells().find(|&c| c.1 == given.1 && !d.app.game.is_filled(c)).unwrap();
    d.write(row, v);
    d.write(col, v);
    // An entry that clashes only with an entry.
    let a = empty_cell(&d.app);
    let free = (1..=9u8).find(|&x| peers(a).all(|p| d.app.game.value_at(p) != x)).unwrap();
    d.write(a, free);
    let b = peers(a).find(|&p| !d.app.game.is_filled(p)).unwrap();
    d.write(b, free);
    let g = &d.app.game;
    assert!(g.is_entry(row) && g.is_entry(col) && g.is_entry(a) && g.is_entry(b));
    assert!(g.is_clashing(given) && g.is_clashing(a) && g.is_clashing(b));
    assert_each_clash_has_an_entry(g);
    assert_clashes_agree(g, "the writing");
}

#[test]
fn each_cell_is_one_of_four_types() {
    // BRD-1: after marks, numbers, an erasure, undo, redo, a save and a load, each cell is a starting
    // number, an entry, an empty cell with no marks, or an empty cell with marks.
    let mut d = Driver::new(46);
    assert_cell_types(&d.app.game, "the start");
    let empties: Vec<Cell> = cells().filter(|&c| !d.app.game.is_filled(c)).take(3).collect();
    let given = given_cell(&d.app);
    d.tap_micro(empties[0], 2);
    d.tap_micro(empties[0], 6);
    d.tap_micro(given, 3);
    assert_cell_types(&d.app.game, "marks");
    assert_eq!(kind(&d.app.game, empties[0]), Some(Kind::EmptyWithMarks));
    assert_eq!(kind(&d.app.game, given), Some(Kind::Given), "MRK-3: a tap on a starting number adds no mark");
    d.write(empties[0], 7);
    d.write(empties[1], 4);
    d.write(given, 1);
    assert_cell_types(&d.app.game, "numbers");
    assert_eq!(kind(&d.app.game, empties[0]), Some(Kind::Entry));
    d.erase(empties[1]);
    d.erase(given);
    assert_cell_types(&d.app.game, "an erasure");
    assert_eq!(kind(&d.app.game, empties[1]), Some(Kind::Empty));
    d.button(Button::Menu);
    d.button(Button::Undo);
    assert_cell_types(&d.app.game, "undo");
    d.button(Button::Undo);
    d.button(Button::Undo);
    assert_cell_types(&d.app.game, "undo of a number");
    assert_eq!(kind(&d.app.game, empties[0]), Some(Kind::EmptyWithMarks));
    d.button(Button::Redo);
    assert_cell_types(&d.app.game, "redo");
    d.button(Button::Menu);
    d.tap_micro(empties[2], 9);
    assert_cell_types(&d.app.game, "a mark after redo");

    let dir = std::env::temp_dir().join(format!("sudoku-inv-puzzle-{}", std::process::id()));
    let gp = dir.join("game.json");
    sudoku::store::save_json(&gp, &d.app.game).unwrap();
    let app = App::new(sudoku::store::load_json(&gp), None, ResultsDb::in_memory(), Box::new(TestEnv { time: 0, seed: 7 }));
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(app.game, d.app.game);
    assert_cell_types(&app.game, "a save and a load");

    // A saved game with a cell that is none of the four types is not used. The app makes a new game.
    let base = d.app.game.clone();
    let empty = cells().find(|&c| !base.is_filled(c) && base.marks_at(c) == 0).unwrap();
    let mut bad: Vec<(&str, Game)> = Vec::new();
    let mut g = base.clone();
    g.value[empty.0 as usize][empty.1 as usize] = 10;
    bad.push(("value 10", g));
    let mut g = base.clone();
    g.marks[given.0 as usize][given.1 as usize] = mark_bit(3);
    bad.push(("marks on a starting number", g));
    let mut g = base.clone();
    g.marks[empty.0 as usize][empty.1 as usize] = 1;
    bad.push(("mark bit 0", g));
    for (what, g) in bad {
        assert!(cells().any(|c| kind(&g, c).is_none()), "the corrupt game has a cell with no type: {what}");
        let app = App::new(Some(g.clone()), None, ResultsDb::in_memory(), Box::new(TestEnv { time: 0, seed: 8 }));
        assert_ne!(app.game, g, "BRD-1: a saved game with {what} is used");
        assert_cell_types(&app.game, what);
    }
}
