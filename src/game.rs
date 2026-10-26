//! The game: the puzzle, the numbers, the pencil marks, the undo and redo history and the clock.
//!
//! All changes to the board go through this module. Refer to REQUIREMENTS.md §4, §6, §7 and §8.

use serde::{Deserialize, Serialize};

/// A cell position: (row, column), each 0..9.
pub type Cell = (u8, u8);

/// A set of pencil marks. Bit d (1..=9) is set if the cell has a mark for digit d.
pub type Marks = u16;

pub const ALL_MARKS: Marks = 0b11_1111_1110;

pub fn mark_bit(d: u8) -> Marks {
    debug_assert!((1..=9).contains(&d));
    1 << d
}

/// All cells, row by row.
pub fn cells() -> impl Iterator<Item = Cell> {
    (0..9u8).flat_map(|r| (0..9u8).map(move |c| (r, c)))
}

/// The 20 other cells in the row, the column and the 3x3 box of a cell.
pub fn peers((r, c): Cell) -> impl Iterator<Item = Cell> {
    cells().filter(move |&(i, j)| {
        (i, j) != (r, c) && (i == r || j == c || (i / 3 == r / 3 && j / 3 == c / 3))
    })
}

/// One change to the board. Each change is one item in the undo history (REQUIREMENTS.md §7).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Change {
    /// Add or remove one pencil mark of an empty cell. The change is its own opposite.
    ToggleMark { cell: Cell, digit: u8 },
    /// Add a number. `removed` gives the peer cells that lost the mark `digit` (automatic removal, §6.1).
    AddNumber { cell: Cell, digit: u8, removed: Vec<Cell> },
    /// Erase your number.
    Erase { cell: Cell, digit: u8 },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Game {
    pub seed: u64,
    pub mode: u8,
    pub solution: [[u8; 9]; 9],
    pub given: [[bool; 9]; 9],
    pub value: [[u8; 9]; 9],
    pub marks: [[Marks; 9]; 9],
    pub undo: Vec<Change>,
    pub redo: Vec<Change>,
    /// The clock, in milliseconds. Refer to §8.4.
    pub elapsed_ms: u64,
    pub completed: bool,
}

/// The result of an operation on the game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The operation had no effect.
    NoChange,
    /// The board changed.
    Changed,
    /// The board changed, and the puzzle is now complete.
    Completed,
}

impl Outcome {
    pub fn changed(self) -> bool {
        self != Outcome::NoChange
    }
}

impl Game {
    /// Makes a game from a puzzle. `givens` has 0 for the empty cells.
    pub fn new(seed: u64, mode: u8, solution: [[u8; 9]; 9], givens: [[u8; 9]; 9]) -> Game {
        let mut given = [[false; 9]; 9];
        for (r, c) in cells() {
            given[r as usize][c as usize] = givens[r as usize][c as usize] != 0;
        }
        let mut g = Game {
            seed,
            mode,
            solution,
            given,
            value: givens,
            marks: [[0; 9]; 9],
            undo: Vec::new(),
            redo: Vec::new(),
            elapsed_ms: 0,
            completed: false,
        };
        g.completed = g.is_solved_board();
        g
    }

    pub fn value_at(&self, (r, c): Cell) -> u8 {
        self.value[r as usize][c as usize]
    }

    pub fn marks_at(&self, (r, c): Cell) -> Marks {
        self.marks[r as usize][c as usize]
    }

    pub fn is_given(&self, (r, c): Cell) -> bool {
        self.given[r as usize][c as usize]
    }

    pub fn is_filled(&self, cell: Cell) -> bool {
        self.value_at(cell) != 0
    }

    /// The cell has your number (it is filled and it is not a starting number).
    pub fn is_entry(&self, cell: Cell) -> bool {
        self.is_filled(cell) && !self.is_given(cell)
    }

    /// The cell is filled and a peer has the same value (§6.3).
    pub fn is_clashing(&self, cell: Cell) -> bool {
        let v = self.value_at(cell);
        v != 0 && peers(cell).any(|p| self.value_at(p) == v)
    }

    pub fn givens_count(&self) -> u8 {
        cells().filter(|&c| self.is_given(c)).count() as u8
    }

    pub fn can_undo(&self) -> bool {
        !self.completed && !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.completed && !self.redo.is_empty()
    }

    /// All cells are filled and no cell clashes (§8.3).
    pub fn is_solved_board(&self) -> bool {
        cells().all(|c| self.is_filled(c)) && !cells().any(|c| self.is_clashing(c))
    }

    /// Seconds on the clock (whole seconds, rounded down).
    pub fn seconds(&self) -> u64 {
        self.elapsed_ms / 1000
    }

    /// A pen tap on an empty cell: add or remove the mark of that micro-cell.
    pub fn toggle_mark(&mut self, cell: Cell, digit: u8) -> Outcome {
        if self.completed || self.is_filled(cell) || !(1..=9).contains(&digit) {
            return Outcome::NoChange;
        }
        self.record(Change::ToggleMark { cell, digit })
    }

    /// Writing that the app reads as `digit` on an empty cell.
    pub fn add_number(&mut self, cell: Cell, digit: u8) -> Outcome {
        self.add_number_with(cell, digit, true)
    }

    /// Adds a number. If `auto_remove`, the change also removes the digit from the marks of the peers
    /// (MRK-4); if not, it changes no other cell (MRK-9).
    pub fn add_number_with(&mut self, cell: Cell, digit: u8, auto_remove: bool) -> Outcome {
        if self.completed || self.is_filled(cell) || !(1..=9).contains(&digit) {
            return Outcome::NoChange;
        }
        let removed: Vec<Cell> = if auto_remove {
            peers(cell).filter(|&p| self.marks_at(p) & mark_bit(digit) != 0).collect()
        } else {
            Vec::new()
        };
        self.record(Change::AddNumber { cell, digit, removed })
    }

    /// The eraser on a cell. Only your numbers change.
    pub fn erase(&mut self, cell: Cell) -> Outcome {
        if self.completed || !self.is_entry(cell) {
            return Outcome::NoChange;
        }
        let digit = self.value_at(cell);
        self.record(Change::Erase { cell, digit })
    }

    pub fn undo(&mut self) -> Outcome {
        if !self.can_undo() {
            return Outcome::NoChange;
        }
        let change = self.undo.pop().expect("undo is not empty");
        self.revert(&change);
        self.redo.push(change);
        self.after_change()
    }

    pub fn redo(&mut self) -> Outcome {
        if !self.can_redo() {
            return Outcome::NoChange;
        }
        let change = self.redo.pop().expect("redo is not empty");
        self.apply(&change);
        self.undo.push(change);
        self.after_change()
    }

    /// Adds time to the clock. The clock stops when the puzzle is complete (§8.4).
    pub fn tick(&mut self, ms: u64) {
        if !self.completed {
            self.elapsed_ms = self.elapsed_ms.saturating_add(ms);
        }
    }

    fn record(&mut self, change: Change) -> Outcome {
        self.apply(&change);
        self.undo.push(change);
        self.redo.clear();
        self.after_change()
    }

    fn after_change(&mut self) -> Outcome {
        if self.is_solved_board() {
            self.completed = true;
            Outcome::Completed
        } else {
            Outcome::Changed
        }
    }

    pub(crate) fn apply(&mut self, change: &Change) {
        match *change {
            Change::ToggleMark { cell: (r, c), digit } => {
                self.marks[r as usize][c as usize] ^= mark_bit(digit);
            }
            Change::AddNumber { cell: (r, c), digit, ref removed } => {
                for &(pr, pc) in removed {
                    self.marks[pr as usize][pc as usize] &= !mark_bit(digit);
                }
                self.value[r as usize][c as usize] = digit;
            }
            Change::Erase { cell: (r, c), .. } => {
                self.value[r as usize][c as usize] = 0;
            }
        }
    }

    fn revert(&mut self, change: &Change) {
        match *change {
            Change::ToggleMark { .. } => self.apply(change),
            Change::AddNumber { cell: (r, c), digit, ref removed } => {
                self.value[r as usize][c as usize] = 0;
                for &(pr, pc) in removed {
                    self.marks[pr as usize][pc as usize] |= mark_bit(digit);
                }
            }
            Change::Erase { cell: (r, c), digit } => {
                self.value[r as usize][c as usize] = digit;
            }
        }
    }
}

/// Formats a time as `m:ss`, or `h:mm:ss` after one hour (§8.3).
pub fn format_clock(seconds: u64) -> String {
    let (h, m, s) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOLUTION: [&str; 9] = [
        "534678912", "672195348", "198342567", "859761423", "426853791", "713924856", "961537284",
        "287419635", "345286179",
    ];

    fn sample(blank: &[Cell]) -> Game {
        let mut sol = [[0u8; 9]; 9];
        for (r, row) in SOLUTION.iter().enumerate() {
            for (c, ch) in row.bytes().enumerate() {
                sol[r][c] = ch - b'0';
            }
        }
        let mut givens = sol;
        for &(r, c) in blank {
            givens[r as usize][c as usize] = 0;
        }
        Game::new(1, 2, sol, givens)
    }

    #[test]
    fn clock_format() {
        // ANA-9, CMP-4
        assert_eq!(format_clock(0), "0:00");
        assert_eq!(format_clock(23 * 60 + 41), "23:41");
        assert_eq!(format_clock(3600), "1:00:00");
        assert_eq!(format_clock(3600 + 62), "1:01:02");
    }

    #[test]
    fn peers_are_twenty() {
        for c in cells() {
            assert_eq!(peers(c).count(), 20);
        }
    }

    #[test]
    fn add_number_removes_peer_marks_and_undo_restores() {
        let mut g = sample(&[(0, 0), (0, 1), (1, 1), (8, 8)]);
        assert_eq!(g.toggle_mark((0, 1), 5), Outcome::Changed);
        assert_eq!(g.toggle_mark((8, 8), 5), Outcome::Changed);
        assert_eq!(g.add_number((0, 0), 5), Outcome::Changed);
        assert_eq!(g.marks_at((0, 1)) & mark_bit(5), 0);
        assert_ne!(g.marks_at((8, 8)) & mark_bit(5), 0, "not a peer");
        g.undo();
        assert_ne!(g.marks_at((0, 1)) & mark_bit(5), 0);
        assert_eq!(g.value_at((0, 0)), 0);
    }

    #[test]
    fn eraser_does_not_restore_peer_marks() {
        let mut g = sample(&[(0, 0), (0, 1), (8, 8)]);
        g.toggle_mark((0, 1), 5);
        g.toggle_mark((0, 0), 3);
        g.add_number((0, 0), 5);
        g.erase((0, 0));
        assert_eq!(g.marks_at((0, 1)) & mark_bit(5), 0);
        assert_eq!(g.marks_at((0, 0)), mark_bit(3), "hidden marks show again");
    }

    #[test]
    fn completion_locks() {
        let mut g = sample(&[(0, 0)]);
        assert!(!g.completed);
        assert_eq!(g.add_number((0, 0), 5), Outcome::Completed);
        assert!(g.completed);
        assert!(!g.can_undo());
        assert_eq!(g.undo(), Outcome::NoChange);
        assert_eq!(g.erase((0, 0)), Outcome::NoChange);
        let t = g.elapsed_ms;
        g.tick(1000);
        assert_eq!(g.elapsed_ms, t);
    }

    #[test]
    fn clashes_are_symmetric_and_block_completion() {
        let mut g = sample(&[(0, 0), (0, 1)]);
        g.add_number((0, 0), 3); // Column 0 has a starting 3 at (8, 0).
        g.add_number((0, 1), 5);
        assert!(g.is_clashing((8, 0)));
        assert!(!g.completed);
        assert!(g.is_clashing((0, 0)));
    }

    #[test]
    fn givens_and_filled_cells_ignore_input() {
        let mut g = sample(&[(4, 4)]);
        assert_eq!(g.erase((0, 0)), Outcome::NoChange);
        assert_eq!(g.toggle_mark((0, 0), 1), Outcome::NoChange);
        assert_eq!(g.add_number((0, 0), 1), Outcome::NoChange);
        assert_eq!(g.erase((4, 4)), Outcome::NoChange);
        assert!(g.undo.is_empty());
    }
}
