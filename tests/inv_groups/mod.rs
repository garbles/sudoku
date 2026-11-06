//! A separate model of the rules of completed groups (COL-8 to COL-12, BRD-7), from INVARIANTS.md and
//! REQUIREMENTS.md §4.2, §6.4 and §15. It does not use the code of the app that makes these decisions
//! (`Game::is_clashing`, `Game::completed_cells`, `Settings::shows_completed`, `Color::rgb`). The tests
//! compare the app with this model. It also has the fixtures that make a completed group.
#![allow(dead_code)]

use sudoku::app::{new_game, App};
use sudoku::game::{Cell, Game};
use sudoku::layout;
use sudoku::render::rgb_at;
use sudoku::settings::{Color, Settings};
use tiny_skia::Pixmap;

pub type Values = [[u8; 9]; 9];

/// The kinds of a group.
pub const ROW: u8 = 0;
pub const COLUMN: u8 = 1;
pub const BOX: u8 = 2;

/// The nine cells of a group (INVARIANTS.md §0). A box has its index row by row.
pub fn group_cells(kind: u8, i: u8) -> [Cell; 9] {
    std::array::from_fn(|k| {
        let k = k as u8;
        match kind {
            ROW => (i, k),
            COLUMN => (k, i),
            _ => (i / 3 * 3 + k / 3, i % 3 * 3 + k % 3),
        }
    })
}

/// All 27 groups.
pub fn all_groups() -> Vec<[Cell; 9]> {
    [ROW, COLUMN, BOX].into_iter().flat_map(|kind| (0..9).map(move |i| group_cells(kind, i))).collect()
}

/// The two cells are different and are in the same row, column or box.
pub fn are_peers(a: Cell, b: Cell) -> bool {
    a != b && (a.0 == b.0 || a.1 == b.1 || (a.0 / 3 == b.0 / 3 && a.1 / 3 == b.1 / 3))
}

/// INVARIANTS.md §0: the cell is filled and a peer has the same value.
pub fn clashing(values: &Values, cell: Cell) -> bool {
    let v = values[cell.0 as usize][cell.1 as usize];
    v != 0 && (0..9u8).flat_map(|r| (0..9u8).map(move |c| (r, c))).any(|p| are_peers(cell, p) && values[p.0 as usize][p.1 as usize] == v)
}

/// COL-8: all nine cells are filled and no cell of the group is clashing, also with a cell outside it.
pub fn group_is_complete(values: &Values, group: &[Cell; 9]) -> bool {
    group.iter().all(|&(r, c)| values[r as usize][c as usize] != 0 && !clashing(values, (r, c)))
}

/// The cells of all completed groups.
pub fn cells_in_complete_groups(values: &Values) -> [[bool; 9]; 9] {
    let mut out = [[false; 9]; 9];
    for group in all_groups().iter().filter(|g| group_is_complete(values, g)) {
        for &(r, c) in group {
            out[r as usize][c as usize] = true;
        }
    }
    out
}

/// INVARIANTS.md §0: `shows_completed`.
pub fn shows_completed(s: &Settings, g: &Game, (r, c): Cell) -> bool {
    cells_in_complete_groups(&g.value)[r as usize][c as usize] && s.completed_color_on && s.color_completed != Color::White && !g.completed
}

/// §15: the palette, with the values of the document.
pub fn palette_rgb(color: Color) -> [u8; 3] {
    match color {
        Color::Gray => [0xAE, 0xAE, 0xAE],
        Color::White => [0xFF, 0xFF, 0xFF],
        Color::Yellow => [0xFF, 0xD7, 0x40],
        Color::Blue => [0x40, 0x8C, 0xFF],
        Color::Red => [0xFF, 0x8E, 0x8E],
        Color::Green => [0x48, 0xD6, 0x30],
        Color::Cyan => [0x6F, 0xCF, 0xE6],
        Color::Magenta => [0xE6, 0x50, 0xE0],
        Color::Orange => [0xFF, 0x9A, 0x1F],
    }
}

/// BRD-7: the background of a cell. The first item that applies: clash, highlight, completed, starting
/// number, white. An empty cell is white.
pub fn background(s: &Settings, g: &Game, highlight: Option<u8>, cell: Cell) -> [u8; 3] {
    let v = g.value[cell.0 as usize][cell.1 as usize];
    if v == 0 {
        palette_rgb(Color::White)
    } else if clashing(&g.value, cell) {
        palette_rgb(s.color_clash)
    } else if highlight == Some(v) {
        palette_rgb(s.color_highlight)
    } else if shows_completed(s, g, cell) {
        palette_rgb(s.color_completed)
    } else if g.given[cell.0 as usize][cell.1 as usize] {
        palette_rgb(s.color_starting)
    } else {
        palette_rgb(Color::White)
    }
}

/// BRD-7: the first cell where the drawn background is not the background of the model. The point is
/// away from the digit, the marks and the x, and inside the board lines.
pub fn board_color_error(pm: &Pixmap, app: &App) -> Option<String> {
    (0..9u8).flat_map(|r| (0..9u8).map(move |c| (r, c))).find_map(|cell| {
        let (x, y) = layout::cell_origin(cell);
        let (got, want) = (rgb_at(pm, x + 8, y + 8), background(&app.settings, &app.game, app.ui.highlight, cell));
        (got != want).then(|| format!("BRD-7: the cell {cell:?} (value {}) has the background {got:?}, not {want:?}", app.game.value[cell.0 as usize][cell.1 as usize]))
    })
}

/// The cells of the board in order.
fn all_cells() -> impl Iterator<Item = Cell> {
    (0..9u8).flat_map(|r| (0..9u8).map(move |c| (r, c)))
}

/// A real game of `seed` and `mode` that has a completed row with starting numbers and entries. The
/// player adds the correct numbers, thus the undo history has them. Returns the game and the row.
pub fn game_with_full_row(seed: u64, mode: u8) -> (Game, u8) {
    let mut g = new_game(seed, mode);
    let count = |g: &Game, r: u8, given: bool| (0..9u8).filter(|&c| g.given[r as usize][c as usize] == given).count();
    let row = (0..9u8).find(|&r| count(&g, r, true) >= 2 && count(&g, r, false) >= 2).expect("a row with starting numbers and empty cells");
    let empty: Vec<u8> = (0..9u8).filter(|&c| !g.given[row as usize][c as usize]).collect();
    for c in empty {
        g.add_number((row, c), g.solution[row as usize][c as usize]);
    }
    assert!(group_is_complete(&g.value, &group_cells(ROW, row)), "the fixture has a completed row");
    (g, row)
}

/// The far cells of an outside clash (COL-8, COL-10): the cell `inside` of the full row, and an empty
/// cell `outside` in its column, in a different box. The number of `inside` written in `outside` makes
/// a clash that stops the row. All other cells of the row are not peers of `outside`. Returns the two
/// cells and the number.
pub fn outside_clash(g: &Game, row: u8) -> (Cell, Cell, u8) {
    all_cells()
        .filter(|&(r, c)| r / 3 != row / 3 && g.value[r as usize][c as usize] == 0 && g.given[row as usize][c as usize])
        .map(|(r, c)| ((row, c), (r, c), g.value[row as usize][c as usize]))
        .next()
        .expect("an empty cell in the column of a starting number of the row, in a different box")
}

/// A real game with a completed row that has incorrect entries and no clash: two entries of the row
/// have changed places (BRD-5, COL-9). Returns the game, the row and the two columns.
pub fn game_with_incorrect_full_row() -> (Game, u8, u8, u8) {
    for seed in 1..200u64 {
        let base = new_game(seed, 4);
        for r in 0..9u8 {
            let free: Vec<u8> = (0..9u8).filter(|&c| !base.given[r as usize][c as usize]).collect();
            for (i, &a) in free.iter().enumerate() {
                for &b in &free[i + 1..] {
                    let mut g = base.clone();
                    let sol = base.solution[r as usize];
                    for &c in &free {
                        let digit = if c == a { sol[b as usize] } else if c == b { sol[a as usize] } else { sol[c as usize] };
                        g.add_number((r, c), digit);
                    }
                    if group_is_complete(&g.value, &group_cells(ROW, r)) {
                        return (g, r, a, b);
                    }
                }
            }
        }
    }
    panic!("no game has two entries of a row that can change places with no clash");
}
