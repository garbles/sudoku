//! The game: the puzzle, the numbers, the pencil marks, the undo and redo history, the checkpoints
//! and the clock.
//!
//! All changes to the board go through this module. Refer to REQUIREMENTS.md §4, §6, §7, §8 and §9.3.

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

/// A group of nine cells (INVARIANTS.md §0). Each index is 0..9. A box has its index row by row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Row(u8),
    Column(u8),
    Box(u8),
}

impl Group {
    /// All 27 groups.
    pub fn all() -> impl Iterator<Item = Group> {
        (0..9).flat_map(|i| [Group::Row(i), Group::Column(i), Group::Box(i)])
    }

    /// The nine cells of the group.
    pub fn cells(self) -> [Cell; 9] {
        std::array::from_fn(|i| {
            let i = i as u8;
            match self {
                Group::Row(r) => (r, i),
                Group::Column(c) => (i, c),
                Group::Box(b) => (b / 3 * 3 + i / 3, b % 3 * 3 + i % 3),
            }
        })
    }
}

/// A set of cells of the board.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CellSet(u128);

impl CellSet {
    fn bit((r, c): Cell) -> u128 {
        1 << (r as u32 * 9 + c as u32)
    }

    pub fn insert(&mut self, cell: Cell) {
        self.0 |= Self::bit(cell);
    }

    pub fn contains(self, cell: Cell) -> bool {
        self.0 & Self::bit(cell) != 0
    }
}

/// The sets of a board that the drawing needs. They come from `value` only (CLS-1, COL-9).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BoardSets {
    pub clashing: CellSet,
    /// The cells of the completed groups.
    pub completed: CellSet,
}

/// The numbers of all cells. 0 is an empty cell.
pub type Values = [[u8; 9]; 9];

/// The maximum quantity of checkpoints of a game (§9.3, CKP-4).
pub const MAX_CHECKPOINTS: usize = 4;

/// True if the cell is filled and a peer has the same number (§6.3).
fn clashes(value: &Values, cell: Cell) -> bool {
    let v = value[cell.0 as usize][cell.1 as usize];
    v != 0 && peers(cell).any(|(r, c)| value[r as usize][c as usize] == v)
}

/// The sets of the board with these numbers. It examines each cell one time for clashes, thus a
/// drawing of a board calls it one time, not for each cell.
fn sets_of(value: &Values) -> BoardSets {
    let mut clashing = CellSet::default();
    cells().filter(|&c| clashes(value, c)).for_each(|c| clashing.insert(c));
    let mut completed = CellSet::default();
    for group in Group::all().map(Group::cells) {
        if group.iter().all(|&(r, c)| value[r as usize][c as usize] != 0 && !clashing.contains((r, c))) {
            group.into_iter().for_each(|c| completed.insert(c));
        }
    }
    BoardSets { clashing, completed }
}

/// The digits of the numbers in the peers of a cell, as a set of marks.
fn digits_of_peers(value: &Values, cell: Cell) -> Marks {
    peers(cell).fold(0, |m, (r, c)| match value[r as usize][c as usize] {
        0 => m,
        v => m | mark_bit(v),
    })
}

/// The marks `marks` of a cell that show. Refer to `Game::visible_marks`.
fn visible(value: &Values, marks: Marks, cell: Cell, hide: bool) -> Marks {
    if value[cell.0 as usize][cell.1 as usize] != 0 {
        0
    } else if hide {
        marks & !digits_of_peers(value, cell)
    } else {
        marks
    }
}

/// All cells are filled and no cell clashes (§8.3).
fn solved(value: &Values) -> bool {
    value.iter().flatten().all(|&v| v != 0) && !cells().any(|c| clashes(value, c))
}

/// The numbers and all pencil marks of the board, also the marks that do not show (§9.3).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Board {
    pub value: Values,
    pub marks: [[Marks; 9]; 9],
}

impl Board {
    /// The quantity of filled cells, also the starting numbers.
    pub fn filled_count(&self) -> usize {
        self.value.iter().flatten().filter(|&&v| v != 0).count()
    }

    /// The clashing cells and the cells of the completed groups of this board.
    pub fn board_sets(&self) -> BoardSets {
        sets_of(&self.value)
    }

    /// The marks that show in a cell of this board (MRK-2, MRK-4).
    pub fn visible_marks(&self, cell: Cell, hide: bool) -> Marks {
        visible(&self.value, self.marks[cell.0 as usize][cell.1 as usize], cell, hide)
    }

    /// All cells are filled and no cell clashes (CKP-5).
    pub fn is_solved(&self) -> bool {
        solved(&self.value)
    }
}

/// A copy of the board and of the clock at the time of a save (§9.3).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Checkpoint {
    pub board: Board,
    /// The clock at the save, in milliseconds.
    pub elapsed_ms: u64,
}

impl Checkpoint {
    /// Seconds on the clock at the save (whole seconds, rounded down).
    pub fn seconds(&self) -> u64 {
        self.elapsed_ms / 1000
    }
}

/// One change to the board. Each change is one item in the undo history (REQUIREMENTS.md §7).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Change {
    /// Add or remove one pencil mark of an empty cell. The change is its own opposite.
    ToggleMark { cell: Cell, digit: u8 },
    /// Add a number. A new number does not change the marks of other cells: it only hides them
    /// (§6.1). `removed` is always empty for a new number. Old saved games can have the peer cells
    /// that lost the mark `digit` (the old automatic removal), thus undo and redo still use it.
    AddNumber {
        cell: Cell,
        digit: u8,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        removed: Vec<Cell>,
    },
    /// Erase your number.
    Erase { cell: Cell, digit: u8 },
    /// Replace all numbers and all marks with those of a checkpoint (§9.3, CKP-2). The item keeps the
    /// two boards, thus its undo and redo do not need the checkpoint (CKP-3).
    Restore { before: Box<Board>, after: Box<Board> },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Game {
    pub seed: u64,
    pub mode: u8,
    /// The version of the generator that made the puzzle (PUZ-5). A game that was saved before
    /// version 2 has no version in its file: it is version 1.
    #[serde(default = "first_generator")]
    pub generator: u8,
    /// The starting numbers have 2-way rotational symmetry (PUZ-8): `given[r][c] = given[8 - r][8 -
    /// c]`. Only version 2 of the generator makes a symmetric puzzle. A game file of an earlier
    /// version has no value: the game is not symmetric (PER-9).
    #[serde(default)]
    pub symmetric: bool,
    pub solution: [[u8; 9]; 9],
    pub given: [[bool; 9]; 9],
    pub value: [[u8; 9]; 9],
    pub marks: [[Marks; 9]; 9],
    pub undo: Vec<Change>,
    pub redo: Vec<Change>,
    /// The clock, in milliseconds. Refer to §8.4.
    pub elapsed_ms: u64,
    pub completed: bool,
    /// The checkpoints, the newest first (§9.3, CKP-4). A game file of an earlier version has none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checkpoints: Vec<Checkpoint>,
}

fn first_generator() -> u8 {
    1
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
    /// Makes a game from a puzzle. `givens` has 0 for the empty cells. The game is not symmetric:
    /// `app::new_game` sets `symmetric` for a puzzle of the generator.
    pub fn new(seed: u64, mode: u8, solution: [[u8; 9]; 9], givens: [[u8; 9]; 9]) -> Game {
        let mut given = [[false; 9]; 9];
        for (r, c) in cells() {
            given[r as usize][c as usize] = givens[r as usize][c as usize] != 0;
        }
        let mut g = Game {
            seed,
            mode,
            generator: crate::generator::VERSION,
            symmetric: false,
            solution,
            given,
            value: givens,
            marks: [[0; 9]; 9],
            undo: Vec::new(),
            redo: Vec::new(),
            elapsed_ms: 0,
            completed: false,
            checkpoints: Vec::new(),
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

    /// The digits of the numbers in the peers of a cell, as a set of marks.
    pub fn peer_digits(&self, cell: Cell) -> Marks {
        digits_of_peers(&self.value, cell)
    }

    /// The marks that show in a cell. A filled cell shows no marks (MRK-2). If `hide`, a mark does
    /// not show while a peer has its digit as a number (§6.1, MRK-4).
    pub fn visible_marks(&self, cell: Cell, hide: bool) -> Marks {
        visible(&self.value, self.marks_at(cell), cell, hide)
    }

    /// The cell is filled and a peer has the same value (§6.3).
    pub fn is_clashing(&self, cell: Cell) -> bool {
        clashes(&self.value, cell)
    }

    /// The group is full and none of its cells clashes, also with a cell outside the group (§6.4, COL-8).
    pub fn is_completed_group(&self, group: Group) -> bool {
        group.cells().into_iter().all(|c| self.is_filled(c) && !self.is_clashing(c))
    }

    /// The clashing cells and the cells of the completed groups.
    pub fn board_sets(&self) -> BoardSets {
        sets_of(&self.value)
    }

    /// The cells of the completed groups (COL-8, COL-9).
    pub fn completed_cells(&self) -> CellSet {
        self.board_sets().completed
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
        solved(&self.value)
    }

    /// Seconds on the clock (whole seconds, rounded down).
    pub fn seconds(&self) -> u64 {
        self.elapsed_ms / 1000
    }

    /// A pen tap on an empty cell: add or remove the mark of that micro-cell. Automatic hiding is on.
    pub fn toggle_mark(&mut self, cell: Cell, digit: u8) -> Outcome {
        self.toggle_mark_with(cell, digit, true)
    }

    /// A pen tap on an empty cell. If `hide` and a peer has `digit` as a number, the mark is hidden
    /// (or would be hidden), thus the tap has no effect (MRK-7).
    pub fn toggle_mark_with(&mut self, cell: Cell, digit: u8, hide: bool) -> Outcome {
        if self.completed || self.is_filled(cell) || !(1..=9).contains(&digit) {
            return Outcome::NoChange;
        }
        if self.refuses_mark(cell, digit, hide) {
            return Outcome::NoChange;
        }
        self.record(Change::ToggleMark { cell, digit })
    }

    /// True if a tap on the micro-cell `digit` of the cell is a refused tap (MRK-7): the game is not
    /// complete, the cell is empty, `hide` is true and a peer has the number `digit`. The value comes
    /// only from the numbers in the peers, not from the solution (MRK-18).
    pub fn refuses_mark(&self, cell: Cell, digit: u8, hide: bool) -> bool {
        hide && !self.completed && !self.is_filled(cell) && (1..=9).contains(&digit) && self.peer_digits(cell) & mark_bit(digit) != 0
    }

    /// Writing that the app reads as `digit` on an empty cell. The marks of the peers do not change
    /// (MRK-5). Automatic hiding is a part of the display (`visible_marks`).
    pub fn add_number(&mut self, cell: Cell, digit: u8) -> Outcome {
        if self.completed || self.is_filled(cell) || !(1..=9).contains(&digit) {
            return Outcome::NoChange;
        }
        self.record(Change::AddNumber { cell, digit, removed: Vec::new() })
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

    /// A copy of the numbers and of all marks.
    pub fn board(&self) -> Board {
        Board { value: self.value, marks: self.marks }
    }

    /// The numbers and all marks are the same as `board`, also the marks that do not show (`same_board`
    /// of INVARIANTS.md §0).
    pub fn same_board(&self, board: &Board) -> bool {
        self.value == board.value && self.marks == board.marks
    }

    /// A save is available: the game is in progress, the list is not full and no checkpoint has this
    /// board (CKP-1).
    pub fn can_save_checkpoint(&self) -> bool {
        !self.completed && self.checkpoints.len() < MAX_CHECKPOINTS && !self.checkpoints.iter().any(|k| self.same_board(&k.board))
    }

    /// Adds the board and the clock as the newest checkpoint (CKP-1). The board, the undo and redo
    /// history and the clock do not change. False if a save is not available.
    pub fn save_checkpoint(&mut self) -> bool {
        let available = self.can_save_checkpoint();
        if available {
            self.checkpoints.insert(0, Checkpoint { board: self.board(), elapsed_ms: self.elapsed_ms });
        }
        available
    }

    /// "Restore" of the checkpoint `i`: one new item of the undo history (CKP-2). A checkpoint with the
    /// same board ("Current") has no effect and keeps the redo history. The clock and the list do
    /// not change.
    pub fn restore_checkpoint(&mut self, i: usize) -> Outcome {
        let after = match self.checkpoints.get(i) {
            Some(k) if !self.completed && !self.same_board(&k.board) => Box::new(k.board.clone()),
            _ => return Outcome::NoChange,
        };
        self.record(Change::Restore { before: Box::new(self.board()), after })
    }

    /// Removes the checkpoint `i` (CKP-3). The board, the undo and redo history and the clock do not
    /// change. A complete game keeps its list (CKP-9). False if nothing changed.
    pub fn delete_checkpoint(&mut self, i: usize) -> bool {
        let available = !self.completed && i < self.checkpoints.len();
        if available {
            self.checkpoints.remove(i);
        }
        available
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
            Change::Restore { ref after, .. } => self.set_board(after),
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
            Change::Restore { ref before, .. } => self.set_board(before),
        }
    }

    fn set_board(&mut self, board: &Board) {
        self.value = board.value;
        self.marks = board.marks;
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
    fn add_number_hides_peer_marks_and_undo_shows_them() {
        // MRK-4, MRK-5, UND-4
        let mut g = sample(&cells().collect::<Vec<_>>());
        assert_eq!(g.toggle_mark((0, 1), 5), Outcome::Changed);
        assert_eq!(g.toggle_mark((8, 8), 5), Outcome::Changed);
        assert_eq!(g.add_number((0, 0), 5), Outcome::Changed);
        assert_ne!(g.marks_at((0, 1)) & mark_bit(5), 0, "the mark is kept");
        assert_eq!(g.visible_marks((0, 1), true) & mark_bit(5), 0, "the mark is hidden");
        assert_ne!(g.visible_marks((0, 1), false) & mark_bit(5), 0, "the switch is off: it shows");
        assert_ne!(g.visible_marks((8, 8), true) & mark_bit(5), 0, "not a peer");
        g.undo();
        assert_ne!(g.visible_marks((0, 1), true) & mark_bit(5), 0);
        assert_eq!(g.value_at((0, 0)), 0);
    }

    #[test]
    fn eraser_shows_hidden_peer_marks_again() {
        // MRK-6, MRK-8
        let mut g = sample(&cells().collect::<Vec<_>>());
        g.toggle_mark((0, 1), 5);
        g.toggle_mark((0, 0), 3);
        g.add_number((0, 0), 5);
        g.erase((0, 0));
        assert_ne!(g.visible_marks((0, 1), true) & mark_bit(5), 0, "the peer mark shows again");
        assert_eq!(g.visible_marks((0, 0), true), mark_bit(3), "hidden marks show again");
    }

    #[test]
    fn tap_on_a_hidden_mark_has_no_effect() {
        // MRK-7
        let mut g = sample(&[(0, 0), (0, 1)]);
        g.toggle_mark_with((0, 1), 5, false);
        g.add_number((0, 0), 5);
        assert_eq!(g.toggle_mark((0, 1), 5), Outcome::NoChange, "the mark is hidden");
        assert_eq!(g.toggle_mark((0, 1), 4), Outcome::NoChange, "4 is a starting number in row 0");
        assert_ne!(g.marks_at((0, 1)) & mark_bit(5), 0);
        assert_eq!(g.toggle_mark_with((0, 1), 4, false), Outcome::Changed, "the switch is off");
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
    fn groups_have_nine_different_cells() {
        assert_eq!(Group::all().count(), 27);
        for g in Group::all() {
            let unique: std::collections::HashSet<_> = g.cells().into_iter().collect();
            assert_eq!(unique.len(), 9, "{g:?}");
        }
        assert_eq!(Group::Box(5).cells()[0], (3, 6));
        assert_eq!(Group::Box(5).cells()[8], (5, 8));
    }

    #[test]
    fn a_completed_group_is_full_and_has_no_clash() {
        // COL-8
        let mut g = sample(&[(0, 0), (0, 1)]);
        assert!(!g.is_completed_group(Group::Row(0)), "the row is not full");
        assert!(g.is_completed_group(Group::Row(1)));
        assert!(!g.completed_cells().contains((0, 0)), "an empty cell is never in a group");
        assert!(g.completed_cells().contains((0, 2)), "column 2 is full");
        g.add_number((0, 0), 3); // Column 0 has a starting 3 at (8, 0).
        g.add_number((0, 1), 5); // Column 1 has a starting 5 at (3, 1).
        assert!(!g.is_completed_group(Group::Row(0)), "the row has no two equal digits, but its cells clash with cells outside it");
        assert!(!g.completed_cells().contains((0, 1)));
        assert!(!g.is_completed_group(Group::Column(0)));
        assert!(!g.is_completed_group(Group::Box(0)));
    }

    #[test]
    fn an_erasure_can_complete_a_group_far_from_the_cell() {
        // COL-10: the erasure of (0, 0) removes the clash of the far cell (8, 0), thus row 8 is complete.
        let mut g = sample(&[(0, 0)]);
        g.add_number((0, 0), 3);
        assert!(!g.is_completed_group(Group::Row(8)), "row 8 has the clashing 3 at (8, 0)");
        assert_eq!(g.erase((0, 0)), Outcome::Changed);
        assert!(g.is_completed_group(Group::Row(8)));
        assert_eq!(g.undo.len(), 2, "UND-2: only the two changes of the board");
    }

    #[test]
    fn the_set_of_cells_agrees_with_a_check_of_each_group() {
        let mut g = sample(&[(0, 0), (4, 4), (8, 8)]);
        g.add_number((4, 4), 1);
        let set = g.board_sets();
        for c in cells() {
            assert_eq!(set.clashing.contains(c), g.is_clashing(c), "{c:?}");
            let in_group = Group::all().any(|x| g.is_completed_group(x) && x.cells().contains(&c));
            assert_eq!(set.completed.contains(c), in_group, "{c:?}");
        }
    }

    #[test]
    fn a_restore_is_one_undo_item_and_a_save_or_a_delete_is_none() {
        // CKP-1, CKP-2, CKP-3
        let mut g = sample(&[(0, 0), (0, 1), (4, 4)]);
        g.toggle_mark_with((0, 1), 1, false);
        g.elapsed_ms = 5000;
        let saved = g.board();
        assert!(g.save_checkpoint());
        assert!(!g.can_save_checkpoint(), "a checkpoint has this board");
        assert_eq!(g.undo.len(), 1);
        assert_eq!(g.restore_checkpoint(0), Outcome::NoChange, "Current");
        g.add_number((4, 4), 5);
        let later = g.board();
        g.toggle_mark_with((0, 1), 2, false);
        g.undo();
        assert!(!g.redo.is_empty());
        assert_eq!(g.restore_checkpoint(0), Outcome::Changed);
        assert!(g.redo.is_empty(), "a Restore removes the redo history");
        assert!(g.same_board(&saved));
        assert!(g.delete_checkpoint(0));
        assert_eq!(g.undo(), Outcome::Changed);
        assert!(g.same_board(&later));
        assert_eq!(g.redo(), Outcome::Changed);
        assert!(g.same_board(&saved), "the item does not need its checkpoint");
        assert_eq!(g.elapsed_ms, 5000);
    }

    #[test]
    fn the_list_has_at_most_four_checkpoints_the_newest_first() {
        // CKP-4
        let mut g = sample(&[(0, 0), (0, 1)]);
        for d in 1..=4 {
            g.elapsed_ms = d * 1000;
            g.toggle_mark_with((0, 0), d as u8, false);
            assert!(g.save_checkpoint());
        }
        g.toggle_mark_with((0, 0), 6, false);
        assert!(!g.save_checkpoint(), "the list is full");
        assert_eq!(g.checkpoints.iter().map(Checkpoint::seconds).collect::<Vec<_>>(), [4, 3, 2, 1]);
    }

    #[test]
    fn a_saved_game_keeps_the_symmetry_and_an_earlier_file_is_not_symmetric() {
        // PER-1, PER-9
        let mut g = sample(&[(0, 0)]);
        g.symmetric = true;
        let json = serde_json::to_string(&g).unwrap();
        assert!(serde_json::from_str::<Game>(&json).unwrap().symmetric);
        let earlier = json.replace(r#""symmetric":true,"#, "");
        assert!(!earlier.contains("symmetric"));
        assert!(!serde_json::from_str::<Game>(&earlier).unwrap().symmetric);
    }

    #[test]
    fn a_saved_game_keeps_the_checkpoints_and_an_earlier_file_has_none() {
        // PER-1, CKP-5
        let mut g = sample(&[(0, 0), (0, 1)]);
        let earlier = serde_json::to_string(&g).unwrap();
        assert!(!earlier.contains("checkpoints"));
        assert!(serde_json::from_str::<Game>(&earlier).unwrap().checkpoints.is_empty());
        g.save_checkpoint();
        g.toggle_mark_with((0, 1), 1, false);
        g.restore_checkpoint(0);
        assert_eq!(serde_json::from_str::<Game>(&serde_json::to_string(&g).unwrap()).unwrap(), g);
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
