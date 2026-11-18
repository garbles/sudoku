//! Tests for the rules of INVARIANTS.md about the puzzle (§1: the symmetry PUZ-5 to PUZ-9, the generator
//! versions and the solver of version 3 PUZ-10 to PUZ-13), the cells (§2) and the clashes (§4).
//! Each test gives the rules that it examines. `invariants::check_app` runs only in debug builds, thus
//! these tests assert the rules directly. They also pass with `cargo test --release`.

mod common;

use common::*;
use sudoku::app::{game_of_version, new_game, new_game_with_symmetry, App, Input, Panel};
use sudoku::game::{cells, mark_bit, peers, Cell, Game, ALL_MARKS};
use sudoku::generator::{
    count_solutions, generate, generate_version, generate_with_symmetry, is_valid_solution, partner, Puzzle, Version, FIRST_MODE, FIRST_SYMMETRIC, MODES,
    MODE_LEVELS, MODE_RULES, VERSION,
};
use sudoku::invariants::{check_game, check_puzzle};
use sudoku::layout::{self, Button, LevelButton};
use sudoku::solver::{grade, grade_v3, Board, Grade, Level, TechniqueGroup};
use sudoku::settings::Settings;
use sudoku::store::ResultsDb;

type Grid = [[u8; 9]; 9];

fn start_new_game(d: &mut Driver) {
    d.finger_tap(1100.0, (layout::tb_bottom_y() + 56) as f32);
}

/// Opens the New game panel, selects `mode` on the slider and starts a new game.
fn start_new_game_in_mode(d: &mut Driver, mode: u8) {
    if !d.app.ui.menu.is_open() {
        d.button(Button::Menu);
    }
    if d.app.ui.menu.panel() != Some(Panel::NewGame) {
        d.button(Button::NewGame);
    }
    let stops = d.app.ng_stops;
    d.finger_tap(stops[mode as usize], layout::ng_slider_y() as f32);
    assert_eq!(d.app.ui.slider, mode);
    start_new_game(d);
    assert_eq!(d.app.game.mode, mode);
}

/// Opens the New game panel, selects `mode` and the symmetry `symmetric`, and starts a new game.
fn start_new_game_with(d: &mut Driver, mode: u8, symmetric: bool) {
    if !d.app.ui.menu.is_open() {
        d.button(Button::Menu);
    }
    if d.app.ui.menu.panel() != Some(Panel::NewGame) {
        d.button(Button::NewGame);
    }
    if d.app.ui.symmetric_sel != symmetric {
        tap_symmetric_row(d);
    }
    assert_eq!(d.app.ui.symmetric_sel, symmetric, "DIF-8");
    start_new_game_in_mode(d, mode);
    assert_eq!(d.app.game.symmetric, symmetric, "DIF-5");
}

/// A tap on the Symmetrical row of the New game panel (DIF-8).
fn tap_symmetric_row(d: &mut Driver) {
    d.finger_tap(layout::ng_pl() as f32 + 50.0, layout::ng_symmetric_y() as f32);
}

/// An app that starts with no saved game and the last mode `mode`.
fn app_in_mode(mode: u8, seed: u64) -> App {
    let s = Settings { last_mode: mode, ..Settings::default() };
    App::new(None, Some(s), ResultsDb::in_memory(), Box::new(TestEnv { time: START_TIME, seed }), App::placeholder_ng_stops())
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
    // PUZ-5, PUZ-13: the game after Start new game is the game of its (seed, mode, symmetric) with the
    // current version (3), with the two symmetries. The same key always gives the same starting numbers
    // and the same solution.
    let mut d = Driver::new(42);
    for symmetric in [true, false] {
        for mode in 0..MODES {
            start_new_game_with(&mut d, mode, symmetric);
            let g = &d.app.game;
            let again = new_game_with_symmetry(g.seed, g.mode, g.symmetric);
            assert_eq!((again.given, again.value, again.solution), (g.given, g.value, g.solution), "PUZ-5: mode {mode}, symmetric {symmetric}");
            assert_eq!((g.generator, again.generator), (3, 3), "PUZ-13: mode {mode}, symmetric {symmetric}");
            assert_eq!(generate_version(3, g.seed, mode, symmetric), generate_version(3, g.seed, mode, symmetric));
            let p = generate_version(3, g.seed, mode, symmetric);
            assert_eq!((givens_of(g), g.solution), (p.givens, p.solution), "PUZ-5: mode {mode}, symmetric {symmetric}");
            assert_ne!(p, generate_version(2, g.seed, mode, symmetric), "PUZ-5: the version 2 puzzle of the key is a different puzzle, mode {mode}");
        }
    }
    // A different key gives a different puzzle (no part of the key is ignored).
    let seed = d.app.game.seed;
    assert_ne!(generate(seed, 0), generate(seed, 1));
    assert_ne!(generate(seed, 0), generate(seed ^ 1, 0));
    assert_ne!(generate_version(2, seed, 0, true), generate_version(2, seed, 0, false));
    assert_ne!(generate_version(3, seed, 0, true), generate_version(3, seed, 0, false));
    assert_ne!(generate_version(3, seed, 0, false), generate_version(2, seed, 0, false), "PUZ-5: the version is a part of the key");
    // A result row keeps (seed, mode, symmetric). The key gives the same game again.
    for symmetric in [true, false] {
        start_new_game_with(&mut d, 0, symmetric);
        let g = d.app.game.clone();
        solve(&mut d);
        assert!(d.app.game.completed);
        let row = d.app.results.iter().find(|r| r.seed == g.seed).unwrap().clone();
        assert_eq!((row.mode, row.symmetric), (0, symmetric), "CMP-5");
        let again = new_game_with_symmetry(row.seed, row.mode, row.symmetric);
        assert_eq!((again.given, again.solution), (g.given, g.solution), "PUZ-5: the result row gives the game again");
    }
}

#[test]
fn the_quantity_of_starting_numbers_comes_from_the_game() {
    // PUZ-6, PUZ-11: the quantity in each result is the quantity of the game, which the generator
    // (version 3) sets for the mode and the symmetry. This test cannot show that no other part of the
    // app uses a fixed quantity (that is a rule about the code). It shows only that the result agrees
    // with the game for different modes and the two symmetries.
    let mut d = Driver::new(43);
    for symmetric in [true, false] {
        let mut counts = Vec::new();
        for mode in [0u8, 4] {
            start_new_game_with(&mut d, mode, symmetric);
            let g = d.app.game.clone();
            solve(&mut d);
            assert!(d.app.game.completed);
            let row = d.app.results.iter().find(|r| r.seed == g.seed).unwrap();
            assert_eq!(g.generator, 3, "PUZ-13");
            let generated = generate_version(3, g.seed, mode, symmetric).givens_count();
            assert_eq!(row.mode, mode);
            assert_eq!(row.givens, g.givens_count(), "PUZ-6: result and game, mode {mode}, symmetric {symmetric}");
            assert_eq!(row.givens as usize, generated, "PUZ-6: result and generator, mode {mode}, symmetric {symmetric}");
            counts.push(row.givens);
        }
        assert!(counts[0] > counts[1], "PUZ-6: the quantity changes with the mode: {counts:?}");
        // PUZ-11: version 3 gives only a minimum (44 for Easy, 28 for mode 4), no exact quantity.
        assert!(counts[0] >= 44 && counts[1] >= 28, "PUZ-11: the minimum of the mode, symmetric {symmetric}: {counts:?}");
    }
    // PUZ-6: version 2 sets the most difficult technique of each mode, and each puzzle needs it, with
    // the two symmetries. PUZ-11 gives the same for version 3 (its own tests).
    for seed in [3u64, 8] {
        for m in 0..MODES {
            for symmetric in [false, true] {
                let p = generate_version(2, seed, m, symmetric);
                assert_eq!(grade(&p.givens, Level::Chains), Some(MODE_LEVELS[m as usize]), "PUZ-6: seed {seed} mode {m} symmetric {symmetric}");
            }
        }
    }
}

// ---- PUZ-5, PUZ-6, PUZ-7, PUZ-8, PUZ-9: the symmetry.

/// FNV-1a of the starting numbers and the solutions of the puzzles of `seeds`.
fn puzzles_hash(seeds: &[u64], puzzle: impl Fn(u64) -> Puzzle) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for &seed in seeds {
        let p = puzzle(seed);
        for row in p.givens.iter().chain(p.solution.iter()) {
            for &v in row {
                h ^= v as u64;
                h = h.wrapping_mul(0x100000001b3);
            }
        }
    }
    h
}

const HASH_SEEDS: [u64; 6] = [0, 1, 2, 77, 12345, u64::MAX];

#[test]
fn puzzles_that_are_not_symmetric_did_not_change() {
    // PUZ-5, PUZ-12: "For symmetric = false, the puzzle of each (seed, mode) of the versions 1 and 2 is
    // the same as before the symmetry was added." The hashes V2 and V1 come from `generate(seed, mode)`
    // and `generate_version(1, seed, mode)` of the app before the symmetry (commit bcdc7f4), for 6
    // seeds. The hashes V3 are the fixed values of version 3 (2026-11-17): `generate` and
    // `new_game` give the current version (PUZ-13).
    const V2: [u64; 7] =
        [0x7e921fddb4b615e6, 0x3ca48556cc1892cb, 0x7224f68ff1e545dc, 0x3f6a18f632ee7b80, 0x912af17b4975f6ca, 0xe44aea34298043c0, 0xd5ffae4ed593f705];
    const V1: [u64; 5] = [0x17df1cac0077edd9, 0xe4e1af30ce40a16d, 0x6a8335052c12b620, 0x08e983964b6e7207, 0xfb0a129795a626e3];
    const V3: [u64; 7] = [0xace58d20d759095f, 0x120ce9f913ddbcb6, 0xe3819ca8875c4892, 0x9ea9cae80b32e54a, 0xcf994f1628b522dd, 0x9123b37ffe3a1b52, 0xbb838b801dfe21c5];
    for mode in 0..MODES {
        assert_eq!(puzzles_hash(&HASH_SEEDS, |s| generate_version(2, s, mode, false)), V2[mode as usize], "PUZ-5, PUZ-12: version 2, mode {mode}");
        let want = V3[mode as usize];
        assert_eq!(puzzles_hash(&HASH_SEEDS, |s| generate_version(3, s, mode, false)), want, "PUZ-5: version 3, mode {mode}");
        assert_eq!(puzzles_hash(&HASH_SEEDS, |s| generate(s, mode)), want, "PUZ-13: generate, mode {mode}");
        assert_eq!(puzzles_hash(&HASH_SEEDS, |s| generate_with_symmetry(s, mode, false)), want, "PUZ-13: mode {mode}");
        assert_eq!(new_game(HASH_SEEDS[1], mode), new_game_with_symmetry(HASH_SEEDS[1], mode, false), "PUZ-5: mode {mode}");
    }
    for mode in 0..5u8 {
        assert_eq!(puzzles_hash(&HASH_SEEDS, |s| generate_version(1, s, mode, false)), V1[mode as usize], "PUZ-5, PUZ-12: version 1, mode {mode}");
    }
}

#[test]
fn the_key_with_the_symmetry_sets_the_puzzle() {
    // PUZ-5: (seed, mode, symmetric, version) sets the full puzzle. The same key gives the same
    // puzzle; the symmetric puzzle of a seed is not the puzzle of the same seed with no symmetry (it
    // uses a different random sequence, R§14.5). PUZ-8: the symmetric puzzle of the versions 2 and 3 has
    // symmetric starting numbers, and the puzzle has one solution. The center cell has no partner: it
    // goes out alone, thus the quantity is odd if and only if the center is a starting number.
    for version in [2u8, 3] {
        for mode in 0..MODES {
            for seed in [5u64, 6, 1 << 40, u64::MAX] {
                let what = format!("version {version} seed {seed} mode {mode}");
                let a = generate_version(version, seed, mode, true);
                assert_eq!(a, generate_version(version, seed, mode, true), "PUZ-5: {what}");
                if version == VERSION {
                    assert_eq!(a, generate_with_symmetry(seed, mode, true), "PUZ-5, PUZ-13: {what}");
                }
                let b = generate_version(version, seed, mode, false);
                assert_ne!(a.givens, b.givens, "PUZ-5: {what}: the symmetry changes the puzzle");
                assert_eq!(count_solutions(&a.givens, 2), 1, "PUZ-2: {what}");
                assert!(is_complete_sudoku(&a.solution), "PUZ-1: {what}");
                for (r, c) in cells().map(|(r, c)| (r as usize, c as usize)) {
                    assert_eq!(a.givens[r][c] != 0, a.givens[8 - r][8 - c] != 0, "PUZ-8: {what} ({r}, {c})");
                    assert!(a.givens[r][c] == 0 || a.givens[r][c] == a.solution[r][c], "PUZ-3: {what}");
                }
                assert_eq!(a.givens_count() % 2 == 1, a.givens[4][4] != 0, "PUZ-8: {what}: the center and the odd quantity");
            }
        }
    }
    // The fixed values of the symmetric puzzles of version 3 (2026-11-17), from `generate_with_symmetry`
    // (the current version, PUZ-13). The frozen values of version 2 are in
    // `the_puzzles_of_versions_1_and_2_did_not_change`.
    const V3_SYMMETRIC: [u64; 7] = [0x3e25f49bf466a3ee, 0x695e3efa88bd924a, 0x4ed39fbd1bbc0e84, 0xb9693d37e131c859, 0xa7edbabde0863deb, 0x8363733b1ef20037, 0x2251c21be023632f];
    for mode in 0..MODES {
        assert_eq!(puzzles_hash(&HASH_SEEDS, |s| generate_with_symmetry(s, mode, true)), V3_SYMMETRIC[mode as usize], "PUZ-5: version 3 symmetric, mode {mode}");
    }
}

#[test]
fn a_symmetric_easy_puzzle_has_44_or_45_starting_numbers() {
    // PUZ-9, PUZ-6 (version 2 only): a symmetric Easy puzzle never goes below 44 and has 44 or 45
    // starting numbers (81 - 44 = 37 is odd, thus 44 needs the center cell to be empty). A puzzle with
    // 44 has an empty center; a puzzle with 45 has the center. A non-symmetric Easy puzzle has 44.
    // Version 3 has only the minimum 44 (PUZ-11, `a_game_of_version_3_with_more_starting_numbers_...`).
    let mut seen = [0usize; 2];
    for seed in 0..120u64 {
        let p = generate_version(2, seed, 0, true);
        let n = p.givens_count();
        assert!((44..=45).contains(&n), "PUZ-9: seed {seed}: {n} starting numbers");
        assert_eq!(n == 45, p.givens[4][4] != 0, "PUZ-8, PUZ-9: seed {seed}: the center and the odd quantity");
        seen[n - 44] += 1;
        assert_eq!(grade(&p.givens, Level::Chains), Some(MODE_LEVELS[0]), "PUZ-6: seed {seed}");
        assert_eq!(generate_version(2, seed, 0, false).givens_count(), 44, "PUZ-6: seed {seed}, not symmetric");
    }
    assert!(seen[0] > 0, "PUZ-9: no symmetric Easy puzzle has 44: {seen:?}");
}

#[test]
fn the_background_puzzle_of_the_panel_has_the_key_of_the_selection() {
    // PUZ-7, DIF-3, DIF-4, DIF-8: while the New game panel is open, the key of the background puzzle
    // is (next_seed, slider, symmetric_sel). A change of the slider or of the switch changes the key.
    // Start new game gives the puzzle of that key. A press on the row and a release on a different
    // place changes nothing. When the panel closes, the selection goes back to the setting.
    for start_sym in [true, false] {
        let s = Settings { last_mode: 3, last_symmetric: start_sym, ..Settings::default() };
        let mut d = Driver::from(App::new(None, Some(s), ResultsDb::in_memory(), Box::new(TestEnv { time: START_TIME, seed: 47 }), App::placeholder_ng_stops()));
        assert_eq!(d.app.game.symmetric, start_sym, "DIF-5");
        d.button(Button::Menu);
        d.button(Button::NewGame);
        assert_eq!((d.app.ui.slider, d.app.ui.symmetric_sel), (3, start_sym), "DIF-3");
        let (seed, mode, sym) = d.app.early_game().expect("PUZ-7: a background puzzle while the panel is open");
        assert_eq!((mode, sym), (3, start_sym), "PUZ-7: the key of the panel");
        tap_symmetric_row(&mut d);
        assert_eq!(d.app.ui.symmetric_sel, !start_sym, "DIF-8");
        assert_eq!(d.app.settings.last_symmetric, start_sym, "DIF-4");
        assert_eq!(d.app.early_game(), Some((seed, 3, !start_sym)), "PUZ-7: the switch changes the key");
        // DIF-8: a press on the row and a release on the slider row changes nothing.
        let (x, y) = (layout::ng_pl() as f32 + 50.0, layout::ng_symmetric_y() as f32);
        d.send(Input::TouchDown { id: 1, x, y });
        d.send(Input::TouchUp { id: 1, x, y: layout::ng_slider_y() as f32 });
        assert_eq!(d.app.ui.symmetric_sel, !start_sym, "DIF-8: a release on a different place");
        d.finger_tap(d.app.ng_stops[6], layout::ng_slider_y() as f32);
        assert_eq!(d.app.early_game(), Some((seed, 6, !start_sym)), "PUZ-7: the slider changes the key");
        // The key at the right edge of the row: a tap at any x of the row.
        d.finger_tap(layout::ng_symmetric_row().x1 as f32 - 2.0, layout::ng_symmetric_y() as f32);
        assert_eq!(d.app.early_game(), Some((seed, 6, start_sym)), "PUZ-7, DIF-8: the right edge of the row");
        tap_symmetric_row(&mut d);
        start_new_game(&mut d);
        let g = d.app.game.clone();
        assert_eq!((g.seed, g.mode, g.symmetric), (seed, 6, !start_sym), "PUZ-7, DIF-5");
        assert_eq!((d.app.settings.last_mode, d.app.settings.last_symmetric), (6, !start_sym), "DIF-5");
        let p = generate_version(3, seed, 6, !start_sym);
        assert_eq!((givens_of(&g), g.solution), (p.givens, p.solution), "PUZ-7, PUZ-5");
        assert_eq!(g.generator, 3, "PUZ-7, PUZ-13: the background puzzle has the current version");

        // DIF-4, DIF-8: a change of the switch and a close of the panel: the change is lost.
        d.button(Button::NewGame);
        assert_eq!(d.app.ui.symmetric_sel, !start_sym, "DIF-3: the symmetry of the last new game");
        tap_symmetric_row(&mut d);
        assert_eq!(d.app.ui.symmetric_sel, start_sym);
        d.button(Button::NewGame);
        assert_ne!(d.app.ui.menu.panel(), Some(Panel::NewGame));
        assert_eq!(d.app.ui.symmetric_sel, !start_sym, "DIF-4, DIF-8: the selection after the close");
        assert_eq!(d.app.settings.last_symmetric, !start_sym, "DIF-4");
        assert_eq!(Game { elapsed_ms: 0, ..d.app.game.clone() }, Game { elapsed_ms: 0, ..g }, "DIF-4: the game does not change (only the clock)");
    }
}

#[test]
fn clashes_follow_the_values_after_each_change_and_a_new_game() {
    // CLS-1, CLS-3: after each change (a new number, an erasure, undo, redo and a new game) the set
    // of clashing cells is the set from the values. Each clash includes an entry.
    let mut d = Driver::new(44);
    assert_clashes_agree(&d.app.game, "the start");
    let given = given_cell(&d.app.game);
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
    let given = given_cell(&d.app.game);
    let v = d.app.game.value_at(given);
    let row = cells().find(|&c| c.0 == given.0 && !d.app.game.is_filled(c)).unwrap();
    let col = cells().find(|&c| c.1 == given.1 && !d.app.game.is_filled(c)).unwrap();
    d.write(row, v);
    d.write(col, v);
    // An entry that clashes only with an entry.
    let a = empty_cell(&d.app.game);
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
    let given = given_cell(&d.app.game);
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
    let mark = (1..=9).find(|&v| d.app.game.peer_digits(empties[2]) & sudoku::game::mark_bit(v) == 0).unwrap();
    d.tap_micro(empties[2], mark);
    assert_eq!(kind(&d.app.game, empties[2]), Some(Kind::EmptyWithMarks), "MRK-7: no peer has the digit");
    assert_cell_types(&d.app.game, "a mark after redo");

    let dir = std::env::temp_dir().join(format!("sudoku-inv-puzzle-{}", std::process::id()));
    let gp = dir.join("game.json");
    sudoku::store::save_json(&gp, &d.app.game).unwrap();
    let app = App::new(sudoku::store::load_json(&gp), None, ResultsDb::in_memory(), Box::new(TestEnv { time: 0, seed: 7 }), App::placeholder_ng_stops());
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
        let app = app_with_game(g.clone(), 0, 8);
        assert_ne!(app.game, g, "BRD-1: a saved game with {what} is used");
        assert_cell_types(&app.game, what);
    }
}

// ---- PUZ-5, PUZ-8, PUZ-9, PUZ-10, PUZ-11, PUZ-12, PUZ-13: the generator versions and the solver of
// version 3.

/// The rows, the columns and the boxes as lists of solver cells (r * 9 + c). This does not use the code
/// of the app.
fn lines() -> Vec<Vec<usize>> {
    (0..9).flat_map(|i| [(0..9).map(|j| i * 9 + j).collect(), (0..9).map(|j| j * 9 + i).collect()]).collect()
}

fn boxes() -> Vec<Vec<usize>> {
    (0..9).map(|b| (0..9).map(|k| (b / 3 * 3 + k / 3) * 9 + b % 3 * 3 + k % 3).collect()).collect()
}

/// The numbers and the candidates of a solver board.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Snap {
    value: [u8; 81],
    cand: [u16; 81],
}

fn snap(b: &Board) -> Snap {
    Snap { value: std::array::from_fn(|i| b.value(i)), cand: std::array::from_fn(|i| b.candidates(i)) }
}

fn bit(d: u8) -> u16 {
    1 << (d - 1)
}

/// The full houses: a unit with one empty cell, and the digit that its cell must get.
fn full_houses(s: &Snap) -> Vec<(usize, u8)> {
    let mut out = Vec::new();
    for u in lines().into_iter().chain(boxes()) {
        let empty: Vec<usize> = u.iter().copied().filter(|&i| s.value[i] == 0).collect();
        if let [i] = empty[..] {
            let d = (1..=9u8).find(|&d| u.iter().all(|&j| s.value[j] != d)).unwrap();
            out.push((i, d));
        }
    }
    out
}

/// The hidden singles in `units`: a digit that is not a number of the unit and that is a candidate of
/// only one cell of the unit.
fn hidden_singles(s: &Snap, units: &[Vec<usize>]) -> Vec<(usize, u8)> {
    let mut out = Vec::new();
    for u in units {
        for d in 1..=9u8 {
            let at: Vec<usize> = u.iter().copied().filter(|&i| s.value[i] == 0 && s.cand[i] & bit(d) != 0).collect();
            if at.len() == 1 && u.iter().all(|&i| s.value[i] != d) {
                out.push((at[0], d));
            }
        }
    }
    out
}

fn naked_singles(s: &Snap) -> Vec<(usize, u8)> {
    (0..81).filter(|&i| s.value[i] == 0 && s.cand[i].count_ones() == 1).map(|i| (i, s.cand[i].trailing_zeros() as u8 + 1)).collect()
}

/// The placements of each single group (index 0: group 1, the box singles; 1: the line singles; 2:
/// the naked singles), from the technique group terms of INVARIANTS.md §0.
fn singles(s: &Snap) -> [Vec<(usize, u8)>; 3] {
    let mut box_singles = full_houses(s);
    box_singles.extend(hidden_singles(s, &boxes()));
    [box_singles, hidden_singles(s, &lines()), naked_singles(s)]
}

/// True if a locked candidate (pointing or claiming) can remove a candidate.
fn locked_candidate(s: &Snap) -> bool {
    let candidate = |i: usize, d: u8| s.value[i] == 0 && s.cand[i] & bit(d) != 0;
    let (lines, boxes) = (lines(), boxes());
    (1..=9u8).any(|d| {
        // Pointing: the candidates of a box are in one line. The line has the candidate outside the box.
        let pointing = boxes.iter().any(|b| {
            let at: Vec<usize> = b.iter().copied().filter(|&i| candidate(i, d)).collect();
            !at.is_empty()
                && lines.iter().any(|l| at.iter().all(|i| l.contains(i)) && l.iter().any(|&i| !b.contains(&i) && candidate(i, d)))
        });
        // Claiming: the candidates of a line are in one box. The box has the candidate outside the line.
        let claiming = lines.iter().any(|l| {
            let at: Vec<usize> = l.iter().copied().filter(|&i| candidate(i, d)).collect();
            !at.is_empty()
                && boxes.iter().any(|b| at.iter().all(|i| b.contains(i)) && b.iter().any(|&i| !l.contains(&i) && candidate(i, d)))
        });
        pointing || claiming
    })
}

/// What a run of the solver did, from its steps.
#[derive(Default)]
struct Run {
    /// The grade from the steps (PUZ-10). None if the solver stopped before the grid was complete.
    grade: Option<Grade>,
    /// The groups that the steps used.
    used: Vec<TechniqueGroup>,
    /// The largest quantity of removal steps in one stop.
    longest_stop: usize,
}

/// Runs the version 3 solver step by step with the limit `permitted` and examines each step (PUZ-10):
/// the solver uses the easiest group that can make progress, never a group above the limit; a single
/// puts one number (the solution) and only removes candidates of other cells; a step of the groups 4
/// to 8 puts no number, removes one or more candidates and never removes the solution (no guess). The
/// grade comes from the steps, not from `grade_v3`.
fn run_steps(givens: &Grid, solution: &Grid, permitted: TechniqueGroup, what: &str) -> Run {
    let mut b = Board::new(givens);
    let mut run = Run::default();
    let mut grade = Grade { hardest: TechniqueGroup::BoxSingles, stops: 0 };
    let mut stop_len = 0;
    loop {
        let before = snap(&b);
        if before.value.iter().all(|&v| v != 0) {
            run.grade = Some(grade);
            return run;
        }
        let probe = b.clone();
        let found = singles(&before);
        let easiest_single = TechniqueGroup::ALL[..3].iter().zip(&found).find(|(_, f)| !f.is_empty()).map(|(&g, _)| g).filter(|&g| g <= permitted);
        let Some(g) = b.step_v3(permitted) else {
            assert_eq!(easiest_single, None, "PUZ-10: {what}: no step, but a single is possible");
            assert!(!(permitted >= TechniqueGroup::LockedCandidates && locked_candidate(&before)), "PUZ-10: {what}: no step, but a locked candidate is possible");
            assert!(snap(&b) == before, "PUZ-10: {what}: a step with no result changed the board");
            return run;
        };
        run.used.push(g);
        let after = snap(&b);
        assert!(g <= permitted, "PUZ-10: {what}: group {g:?} is above the limit {permitted:?}");
        match easiest_single {
            Some(s) => assert_eq!(g, s, "PUZ-10: {what}: the easiest group that can make progress"),
            None => assert!(!g.is_single(), "PUZ-10: {what}: {g:?} with no single on the board"),
        }
        if !g.is_single() && permitted >= TechniqueGroup::LockedCandidates && locked_candidate(&before) {
            assert_eq!(g, TechniqueGroup::LockedCandidates, "PUZ-10: {what}: a locked candidate is possible");
        }
        if g > TechniqueGroup::LockedCandidates {
            let easier = TechniqueGroup::ALL[g.number() as usize - 2];
            assert_eq!(probe.clone().step_v3(easier), None, "PUZ-10: {what}: {g:?} while {easier:?} or an easier group can make progress");
        }
        for i in 0..81 {
            assert_eq!(after.cand[i] & !before.cand[i], 0, "PUZ-10: {what}: {g:?} added a candidate at {i}");
            if after.value[i] == 0 {
                let sol = solution[i / 9][i % 9];
                assert_ne!(after.cand[i] & bit(sol), 0, "PUZ-10: {what}: {g:?} removed the solution {sol} at {i}");
            }
        }
        let placed: Vec<(usize, u8)> = (0..81).filter(|&i| after.value[i] != before.value[i]).map(|i| (i, after.value[i])).collect();
        if g.is_single() {
            let [(i, v)] = placed[..] else { panic!("PUZ-10: {what}: {g:?} changed {placed:?}") };
            assert_eq!((before.value[i], v), (0, solution[i / 9][i % 9]), "PUZ-10: {what}: {g:?} at {i}");
            assert!(found[g.number() as usize - 1].contains(&(i, v)), "PUZ-10: {what}: {v} at {i} is not a placement of {g:?}");
            stop_len = 0;
        } else {
            assert!(placed.is_empty(), "PUZ-10: {what}: {g:?} put a number: {placed:?}");
            assert!(after.cand != before.cand, "PUZ-10: {what}: {g:?} removed no candidate");
            // A stop: one or more removal steps between two singles (§0, terms).
            if stop_len == 0 {
                grade.stops += 1;
            }
            stop_len += 1;
            run.longest_stop = run.longest_stop.max(stop_len);
        }
        grade.hardest = grade.hardest.max(g);
    }
}

#[test]
fn the_technique_groups_have_the_documented_order() {
    // PUZ-10, §0 (technique group): eight groups, from the easiest (1) to the most difficult (8). The
    // groups 1 to 3 put a number; the groups 4 to 8 only remove candidates.
    use TechniqueGroup::*;
    let want = [BoxSingles, LineSingles, NakedSingles, LockedCandidates, Subsets, Fish, Wings, Chains];
    assert_eq!(TechniqueGroup::ALL, want, "PUZ-10");
    for (k, g) in want.into_iter().enumerate() {
        assert_eq!(g.number() as usize, k + 1, "PUZ-10: {g:?}");
        assert_eq!(g.is_single(), k < 3, "PUZ-10: {g:?}");
        assert!(want[..k].iter().all(|&e| e < g), "PUZ-10: the order of {g:?}");
    }
}

#[test]
fn the_solver_of_version_3_uses_the_easiest_group_at_each_step() {
    // PUZ-10: for puzzles of each mode and the two symmetries, with the limit of the mode and with no
    // limit, each step is examined (`run_steps`). The grade of the steps is the grade of `grade_v3`, and
    // two runs give the same grade. Also puzzles of version 2 (they need other groups), and a hard
    // puzzle with low limits (the solver stops, with no group above the limit).
    let mut used = [0usize; 8];
    let mut longest_stop = 0;
    let mut stopped = 0;
    let mut cases: Vec<(String, Puzzle, TechniqueGroup)> = Vec::new();
    for mode in 0..MODES {
        for symmetric in [false, true] {
            for seed in [21u64, 22] {
                let p = generate_version(3, seed, mode, symmetric);
                let rule = MODE_RULES[mode as usize];
                cases.push((format!("v3 seed {seed} mode {mode} symmetric {symmetric}, limit of the mode"), p.clone(), rule.permitted));
                cases.push((format!("v3 seed {seed} mode {mode} symmetric {symmetric}, no limit"), p, TechniqueGroup::Chains));
            }
        }
    }
    for seed in [3u64, 4, 5] {
        for mode in [5u8, 6] {
            cases.push((format!("v2 seed {seed} mode {mode}"), generate_version(2, seed, mode, false), TechniqueGroup::Chains));
        }
    }
    let hard = generate_version(3, 23, 6, false);
    for limit in [TechniqueGroup::BoxSingles, TechniqueGroup::LineSingles, TechniqueGroup::NakedSingles, TechniqueGroup::LockedCandidates, TechniqueGroup::Subsets] {
        cases.push((format!("v3 seed 23 mode 6, limit {limit:?}"), hard.clone(), limit));
    }
    for (what, p, limit) in &cases {
        let run = run_steps(&p.givens, &p.solution, *limit, what);
        assert_eq!(run.grade, grade_v3(&p.givens, *limit, None), "PUZ-10: {what}: the grade of the steps");
        assert_eq!(grade_v3(&p.givens, *limit, None), grade_v3(&p.givens, *limit, None), "PUZ-10: {what}: two runs");
        if let Some(g) = run.grade {
            assert_eq!(g.hardest, run.used.iter().copied().max().unwrap_or(TechniqueGroup::BoxSingles), "PUZ-10: {what}");
            // The limit of stops: the grade with the exact quantity, and None with one stop less.
            assert_eq!(grade_v3(&p.givens, *limit, Some(g.stops)), Some(g), "PUZ-10: {what}: max_stops = stops");
            if g.stops > 0 {
                assert_eq!(grade_v3(&p.givens, *limit, Some(g.stops - 1)), None, "PUZ-10: {what}: max_stops = stops - 1");
            }
        } else {
            stopped += 1;
        }
        for g in &run.used {
            used[g.number() as usize - 1] += 1;
        }
        longest_stop = longest_stop.max(run.longest_stop);
    }
    // The cases reach each group, a stop with two removal steps or more (it counts as one stop), and a
    // solver that stops at its limit.
    assert!(used.iter().all(|&n| n > 0), "the cases do not use each group: {used:?}");
    assert!(longest_stop >= 2, "no stop has two removal steps one after the other");
    assert!(stopped > 0, "no run stops at its limit");
}

#[test]
fn the_solver_of_version_3_never_guesses() {
    // PUZ-10, PUZ-2: the solver never guesses and never uses a technique that assumes one solution.
    // Thus it cannot complete starting numbers that have two solutions. Each of its steps keeps the
    // candidates of the first solution (`run_steps`).
    for (seed, mode) in [(31u64, 4u8), (32, 5), (33, 6), (34, 0)] {
        let p = generate_version(3, seed, mode, false);
        let mut givens = p.givens;
        let mut two = false;
        for (r, c) in cells().map(|(r, c)| (r as usize, c as usize)) {
            if givens[r][c] != 0 {
                let v = std::mem::replace(&mut givens[r][c], 0);
                if count_solutions(&givens, 2) == 2 {
                    two = true;
                    break;
                }
                givens[r][c] = v;
            }
        }
        if !two {
            // Remove more starting numbers until there are two solutions.
            for (r, c) in cells().map(|(r, c)| (r as usize, c as usize)) {
                givens[r][c] = 0;
                if count_solutions(&givens, 2) == 2 {
                    two = true;
                    break;
                }
            }
        }
        assert!(two && independent_count(&givens, 2).0 == 2, "seed {seed} mode {mode}: starting numbers with two solutions");
        let what = format!("seed {seed} mode {mode} with two solutions");
        assert_eq!(grade_v3(&givens, TechniqueGroup::Chains, None), None, "PUZ-10: {what}");
        assert!(run_steps(&givens, &p.solution, TechniqueGroup::Chains, &what).grade.is_none(), "PUZ-10: {what}");
    }
}

#[test]
fn the_mode_values_of_version_3_are_the_documented_values() {
    // PUZ-11: (permitted, needed, floor, max_stops) of each mode, with the numbers of the groups.
    type Values = (u8, Option<u8>, Option<usize>, Option<u32>);
    let want: [Values; 7] = [
        (3, None, Some(44), Some(0)),
        (1, None, Some(38), Some(0)),
        (2, Some(2), Some(34), Some(0)),
        (3, Some(2), Some(31), Some(0)),
        (4, Some(4), Some(28), Some(2)),
        (6, Some(5), None, None),
        (8, Some(7), None, None),
    ];
    for (m, rule) in MODE_RULES.iter().enumerate() {
        let got = (rule.permitted.number(), rule.needed.map(TechniqueGroup::number), rule.floor, rule.max_stops);
        assert_eq!(got, want[m], "PUZ-11: mode {m}");
        // Item 3: the needed group or a more difficult group.
        for g in TechniqueGroup::ALL {
            assert_eq!(rule.accepts(g), want[m].1.is_none_or(|n| g.number() >= n), "PUZ-11: mode {m}, {g:?}");
        }
    }
}

/// PUZ-11: the four items of the rule for the puzzle of version 3 of `mode`, from the steps of the
/// solver (`run_steps`) and from `ModeRule::grade`. Gives the grade.
fn assert_mode_rule(p: &Puzzle, mode: u8, what: &str) -> Grade {
    let rule = MODE_RULES[mode as usize];
    let run = run_steps(&p.givens, &p.solution, rule.permitted, what);
    let g = run.grade.unwrap_or_else(|| panic!("PUZ-11 (1): {what}: the solver limited to {:?} does not complete the puzzle", rule.permitted));
    assert_eq!(rule.grade(&p.givens), Some(g), "PUZ-11: {what}: the grade of the mode");
    assert!(rule.max_stops.is_none_or(|max| g.stops <= max), "PUZ-11 (2): {what}: {} stops", g.stops);
    assert!(g.hardest <= rule.permitted, "PUZ-11 (3): {what}: {:?}", g.hardest);
    // The fallback of item 3 is not used by these fixed seeds (no fallback in 14,000 measured puzzles).
    assert!(rule.needed.is_none_or(|n| g.hardest >= n), "PUZ-11 (3): {what}: {:?} is easier than {:?}", g.hardest, rule.needed);
    if mode <= 4 {
        let floor = rule.floor.unwrap();
        assert!(p.givens_count() >= floor, "PUZ-11 (4): {what}: {} starting numbers, minimum {floor}", p.givens_count());
    }
    g
}

#[test]
fn each_puzzle_of_version_3_obeys_the_rule_of_its_mode() {
    // PUZ-11, PUZ-2, PUZ-3, PUZ-8, PUZ-13: for each mode and the two symmetries, the puzzles of version 3
    // agree with the four items of PUZ-11 and have one solution. A new game of the app has the same
    // puzzle, version 3, and `check_puzzle` accepts it.
    for mode in 0..MODES {
        for symmetric in [false, true] {
            let mut hardest = Vec::new();
            for seed in [0u64, 1, 2, 77, u64::MAX] {
                let what = format!("seed {seed} mode {mode} symmetric {symmetric}");
                let p = generate_version(3, seed, mode, symmetric);
                assert_eq!(count_solutions(&p.givens, 2), 1, "PUZ-2: {what}");
                let (n, first) = independent_count(&p.givens, 2);
                assert_eq!((n, first), (1, Some(p.solution)), "PUZ-2: {what}: the independent solver");
                hardest.push(assert_mode_rule(&p, mode, &what).hardest);
                if symmetric {
                    for cell @ (r, c) in cells() {
                        let (i, j) = partner(cell);
                        assert_eq!(p.givens[r as usize][c as usize] != 0, p.givens[i as usize][j as usize] != 0, "PUZ-8: {what}");
                    }
                }
                let g = new_game_with_symmetry(seed, mode, symmetric);
                assert_eq!((g.generator, g.symmetric, givens_of(&g), g.solution), (3, symmetric, p.givens, p.solution), "PUZ-13, PUZ-5: {what}");
                assert_eq!(check_puzzle(&g), Ok(()), "PUZ-11: {what}");
                check_game(&g).unwrap_or_else(|e| panic!("{what}: {e}"));
            }
            // The groups of the mode (PUZ-11, item 3): the limit and the needed group together.
            use TechniqueGroup::*;
            let allowed: &[TechniqueGroup] = match mode {
                0 => &[BoxSingles, LineSingles, NakedSingles],
                1 => &[BoxSingles],
                2 => &[LineSingles],
                3 => &[LineSingles, NakedSingles],
                4 => &[LockedCandidates],
                5 => &[Subsets, Fish],
                _ => &[Wings, Chains],
            };
            assert!(hardest.iter().all(|g| allowed.contains(g)), "PUZ-11: mode {mode} symmetric {symmetric}: {hardest:?}");
        }
    }
}

#[test]
fn check_puzzle_refuses_a_puzzle_that_breaks_the_rule_of_its_mode() {
    // PUZ-11: a game of version 3 with the starting numbers of a more difficult mode breaks item 1 or 4.
    // A game of version 2 is not a game of PUZ-11.
    let hard = new_game_with_symmetry(41, 6, false);
    for mode in 0..=4u8 {
        let g = Game { mode, ..hard.clone() };
        let e = check_puzzle(&g).expect_err("PUZ-11 (1): a puzzle of mode 6 as mode {mode}");
        assert!(e.contains("PUZ-11"), "{e}");
    }
    // Mode 3 starting numbers (31 or more) as Easy: the limited solver completes it, but item 4 (44).
    let p = generate_version(3, 42, 3, false);
    assert!(p.givens_count() < 44);
    let g = Game { mode: 0, ..new_game(42, 3) };
    let e = check_puzzle(&g).expect_err("PUZ-11 (4): mode 3 starting numbers as Easy");
    assert!(e.contains("PUZ-11"), "{e}");
    assert_eq!(check_puzzle(&Game { generator: 2, ..g }), Ok(()), "PUZ-11 is only for version 3");
}

#[test]
fn a_game_of_version_3_with_more_starting_numbers_than_easy_is_correct() {
    // PUZ-9, PUZ-11: 44 or 45 is a rule of version 2 only. Version 3 has a minimum (44), no maximum. A
    // symmetric Easy game with two more starting numbers (a cell and its partner) is correct for
    // version 3 and not correct for version 2.
    for seed in [51u64, 52] {
        let mut g = new_game_with_symmetry(seed, 0, true);
        let n = g.givens_count();
        assert!(n >= 44, "PUZ-11: seed {seed}: {n}");
        let cell = cells().find(|&c| !g.is_given(c) && partner(c) != c).unwrap();
        for (r, c) in [cell, partner(cell)] {
            g.given[r as usize][c as usize] = true;
            g.value[r as usize][c as usize] = g.solution[r as usize][c as usize];
        }
        assert_eq!(g.givens_count(), n + 2);
        assert!(n + 2 > 45);
        assert_eq!(check_game(&g), Ok(()), "PUZ-9: version 3, seed {seed}, {} starting numbers", n + 2);
        assert_eq!(check_puzzle(&g), Ok(()), "PUZ-11: seed {seed}");
        let e = check_game(&Game { generator: 2, ..g.clone() }).expect_err("PUZ-9: version 2");
        assert!(e.contains("PUZ-9"), "{e}");
    }
}

/// The seeds of the frozen puzzles (PUZ-12), and their FNV-1a hashes (`puzzles_hash`) for each mode.
/// The values come from the puzzles of the release before generator version 3 (version 0.7.1 and
/// the commit bc0a9df): `generate_version(1, ...)`, `generate_version(2, ...)`.
const FROZEN_SEEDS: [u64; 6] = [0, 1, 7, 12345, 3735928559, u64::MAX];
const FROZEN_V1: [u64; 5] = [0xf00e2ec2d0ea28b0, 0x3299025f42b46984, 0xf71f0c76c0887809, 0xa7f7f3b1ea377478, 0x73c06fd294ebdc6f];
const FROZEN_V2: [u64; 7] = [0x50d00ed2e1ba8b7b, 0xbf6212bf8cd25741, 0x6cc1883cf6c58865, 0x925322522f5a63e6, 0x637c459871b10ea4, 0xb851dff403f09551, 0x3fa6d1872860ec83];
const FROZEN_V2_SYMMETRIC: [u64; 7] = [0x25284fba69738a75, 0xc0342c203bb220e9, 0xf640119ff33ef8d6, 0xd673e34b683db686, 0xef32f62f201436ae, 0x62318df109213652, 0x2fb3965ddb709c55];

/// A frozen puzzle in full: (version, seed, mode, symmetric, givens, solution).
type Frozen = (u8, u64, u8, bool, &'static str, &'static str);

/// Three puzzles of the same release, in full.
const FROZEN_PUZZLES: [Frozen; 3] = [
    (1, 7, 4, false, "500060000020030900090508000900000400210080000037010002300000890040000050005700000", "571269348824137965693548721958672413216384579437915682362451897749823156185796234"),
    (2, 7, 0, false, "007010502405000710231079486708900340603200090049738200300056074514307008906000000", "897614532465823719231579486728965341653241897149738265382156974514397628976482153"),
    (2, 7, 6, true, "400020000050097400320000070500100060003050100070004008060000021002810040000040006", "487625913156397482329481675598173264643258197271964358864539721732816549915742836"),
];

fn grid(s: &str) -> Grid {
    let d: Vec<u8> = s.bytes().map(|b| b - b'0').collect();
    std::array::from_fn(|r| std::array::from_fn(|c| d[r * 9 + c]))
}

#[test]
fn the_puzzles_of_versions_1_and_2_did_not_change() {
    // PUZ-12, PUZ-5: each puzzle of version 1 and of version 2 (symmetric and not) is the same, bit for
    // bit, as in the release before version 3. The version 2 puzzle of the demo videos (seed 7, mode 0)
    // is one of them.
    for (version, seed, mode, symmetric, givens, solution) in FROZEN_PUZZLES {
        let p = generate_version(version, seed, mode, symmetric);
        assert_eq!((p.givens, p.solution), (grid(givens), grid(solution)), "PUZ-12: version {version} seed {seed} mode {mode} symmetric {symmetric}");
        let g = game_of_version(Version::from_number(version).unwrap(), seed, mode, symmetric);
        assert_eq!((g.generator, givens_of(&g), g.solution), (version, p.givens, p.solution), "PUZ-12");
        check_game(&g).unwrap();
    }
    for mode in 0..5u8 {
        assert_eq!(puzzles_hash(&FROZEN_SEEDS, |s| generate_version(1, s, mode, false)), FROZEN_V1[mode as usize], "PUZ-12: version 1 mode {mode}");
    }
    for mode in 0..MODES {
        assert_eq!(puzzles_hash(&FROZEN_SEEDS, |s| generate_version(2, s, mode, false)), FROZEN_V2[mode as usize], "PUZ-12: version 2 mode {mode}");
        assert_eq!(puzzles_hash(&FROZEN_SEEDS, |s| generate_version(2, s, mode, true)), FROZEN_V2_SYMMETRIC[mode as usize], "PUZ-12: version 2 symmetric mode {mode}");
        assert_eq!(puzzles_hash(&FROZEN_SEEDS, |s| Version::V2.generate(s, mode, true)), FROZEN_V2_SYMMETRIC[mode as usize], "PUZ-12: Version::V2");
    }
    // The version numbers and the modes of each version (PUZ-5, PUZ-8).
    assert_eq!((VERSION, Version::CURRENT), (3, Version::V3), "PUZ-13");
    assert_eq!([0u8, 1, 2, 3, 4, 255].map(|n| Version::from_number(n).map(Version::number)), [None, Some(1), Some(2), Some(3), None, None], "PUZ-5");
    for (v, modes, symmetry) in [(Version::V1, 5u8, false), (Version::V2, 7, true), (Version::V3, 7, true)] {
        for mode in 0..=MODES {
            for symmetric in [false, true] {
                assert_eq!(v.permits(mode, symmetric), mode < modes && (!symmetric || symmetry), "PUZ-5, PUZ-8: {v:?} mode {mode} symmetric {symmetric}");
            }
        }
    }
}

#[test]
fn a_saved_game_of_a_different_version_or_puzzle_is_not_used() {
    // PUZ-5, PUZ-8, PUZ-13, PER-5: the app does not use a saved game with a version that is not 1, 2 or
    // 3, with a mode or a symmetry that its version does not have, or with the puzzle of a different
    // version of the same key. It does not stop, and the new game has version 3.
    let v2 = game_of_version(Version::V2, 61, 3, true);
    let v3 = game_of_version(Version::V3, 61, 3, true);
    assert_ne!((v2.given, v2.solution), (v3.given, v3.solution));
    let v1 = game_of_version(Version::V1, 62, 4, false);
    let bad: Vec<(&str, Game)> = vec![
        ("version 0", Game { generator: 0, ..v3.clone() }),
        ("version 4", Game { generator: 4, ..v3.clone() }),
        ("version 255", Game { generator: 255, ..v3.clone() }),
        ("version 1 with mode 5", Game { mode: 5, ..v1.clone() }),
        ("version 1 symmetric", Game { symmetric: true, ..v1.clone() }),
        ("version 3 with the version 2 puzzle", Game { generator: 3, ..v2.clone() }),
        ("version 2 with the version 3 puzzle", Game { generator: 2, ..v3.clone() }),
        ("version 1 with the version 2 puzzle", Game { generator: 1, ..game_of_version(Version::V2, 62, 4, false) }),
        ("version 2 with the version 1 puzzle", Game { generator: 2, ..v1.clone() }),
    ];
    for (what, g) in bad {
        let app = app_with_game(g.clone(), START_TIME, 9);
        assert_ne!(app.game, g, "PUZ-5: {what}: the app used it");
        assert_eq!(app.game.generator, 3, "PUZ-13: {what}: the new game");
        check_game(&app.game).unwrap();
    }
    // A correct saved game of each version opens with its version (PUZ-13).
    for g in [v1, v2, v3] {
        let app = app_with_game(g.clone(), START_TIME, 9);
        assert_eq!(app.game, g, "PUZ-13: version {} is used", g.generator);
    }
}

/// The center of a level button of the result card.
fn level_center(b: LevelButton) -> (f32, f32) {
    let r = layout::level_button(b);
    ((r.x0 + r.x1) as f32 / 2.0, (r.y0 + r.y1) as f32 / 2.0)
}

#[test]
fn each_new_game_has_the_current_version_and_a_saved_game_keeps_its_version() {
    // PUZ-13, PUZ-7, CMP-5: the first start makes a game of version 3. A saved game of version 1 or 2
    // opens and keeps its version to the completion, and its result row has its version. The
    // background puzzle and each level button of the result card then give version 3, also for a mode
    // that version 1 does not have (Harder after mode 4).
    let first = new_app(71);
    assert_eq!((first.game.generator, first.game.mode, first.game.symmetric), (3, FIRST_MODE, FIRST_SYMMETRIC), "PUZ-13: the first start");
    let p = generate_version(3, first.game.seed, FIRST_MODE, FIRST_SYMMETRIC);
    assert_eq!((givens_of(&first.game), first.game.solution), (p.givens, p.solution), "PUZ-13, DIF-2");

    let dir = std::env::temp_dir().join(format!("sudoku-inv-puzzle-versions-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let cases = [(Version::V2, 3u8, true, LevelButton::Same), (Version::V2, 6, false, LevelButton::Easier), (Version::V1, 4, false, LevelButton::Harder)];
    for (k, (version, mode, symmetric, button)) in cases.into_iter().enumerate() {
        let what = format!("{version:?} mode {mode} symmetric {symmetric}, {button:?}");
        let saved = game_of_version(version, 80 + k as u64, mode, symmetric);
        let path = dir.join(format!("results-{k}.sqlite"));
        let db = sudoku::store::ResultsDb::open(&path).unwrap();
        let app = App::new(Some(saved.clone()), None, db, Box::new(TestEnv { time: START_TIME, seed: 90 + k as u64 }), App::placeholder_ng_stops());
        let mut d = Driver::from(app);
        assert_eq!(d.app.game, saved, "PUZ-13: {what}: the saved game opens");
        d.write(empty_cell(&d.app.game), 1);
        assert_eq!(d.app.game.generator, version.number(), "PUZ-13: {what}: the version does not change");
        solve(&mut d);
        assert!(d.app.game.completed, "{what}");
        assert_eq!(d.app.game.generator, version.number(), "PUZ-13: {what}: after the completion");
        let conn = rusqlite::Connection::open(&path).unwrap();
        let rows: Vec<(i64, i64)> =
            conn.prepare("SELECT seed, generator FROM results").unwrap().query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().map(|r| r.unwrap()).collect();
        assert_eq!(rows, vec![(saved.seed as i64, i64::from(version.number()))], "PUZ-13, CMP-5: {what}: the result row");
        let (next, early_mode, early_sym) = d.app.early_game().expect("PUZ-7: the background puzzle");
        assert_eq!((early_mode, early_sym), (mode, symmetric), "PUZ-7: {what}");
        if d.app.ui.menu.is_open() {
            d.button(Button::Menu);
        }
        let (x, y) = level_center(button);
        d.finger_tap(x, y);
        let target = button.target(mode).unwrap();
        let g = &d.app.game;
        assert_eq!((g.seed, g.mode, g.symmetric, g.generator), (next, target, symmetric, 3), "PUZ-13, PUZ-7: {what}: the new game");
        let p = generate_version(3, next, target, symmetric);
        assert_eq!((givens_of(g), g.solution), (p.givens, p.solution), "PUZ-7: {what}");
    }
    std::fs::remove_dir_all(&dir).ok();
}
