//! The puzzle generator (REQUIREMENTS.md §14.5).
//!
//! `generate_with_symmetry(seed, mode, symmetric)` always gives the same puzzle for the same values
//! (INVARIANTS.md PUZ-5). `generate(seed, mode)` is the puzzle that is not symmetric. The
//! generator uses its own random number generator, so that a change of a library does not change the
//! puzzles of the saved seeds. Each puzzle has only one solution (PUZ-2).
//!
//! Version 2 makes the puzzles with the human-style solver (`solver.rs`). Each mode has a most
//! difficult technique level, and the puzzle needs a technique of that level (PUZ-6). Version 1 used
//! only a quantity of starting numbers. The app keeps version 1 only to examine the games that it saved
//! before version 2 (PUZ-5).
//!
//! A symmetric puzzle has 2-way rotational symmetry: cell (r, c) is a starting number if and only if
//! cell (8 - r, 8 - c) is a starting number (PUZ-8). Only version 2 has the symmetry. A puzzle that is
//! not symmetric is the same as before the symmetry option.

use crate::game::{cells, Cell};
use crate::solver::{self, Level};

pub const MODES: u8 = 7;
/// The mode of the first game (DIF-2).
pub const FIRST_MODE: u8 = 2;
/// The symmetry of the first game, and of a settings file with no symmetry (DIF-9, PER-9).
pub const FIRST_SYMMETRIC: bool = true;

/// The version of the generator. A different version gives a different puzzle for the same seed.
pub const VERSION: u8 = 2;
/// The quantity of modes of version 1, from 0 (Easy) to 4.
pub const V1_MODES: u8 = 5;

/// The most difficult technique level of each mode, from 0 (Easy) to 6 (Hard). Only the generator
/// uses these values (PUZ-6).
pub const MODE_LEVELS: [Level; MODES as usize] = [
    Level::Singles,
    Level::Singles,
    Level::LockedCandidates,
    Level::Subsets,
    Level::BasicFish,
    Level::Wings,
    Level::Chains,
];

/// The generator stops to remove starting numbers at this quantity. Only Easy stops early: more
/// starting numbers give more possible moves at each step.
const MIN_GIVENS: [usize; MODES as usize] = [44, 0, 0, 0, 0, 0, 0];

/// The maximum quantity of attempts to get a puzzle that needs the level of the mode.
const MAX_ATTEMPTS: u64 = 400;

/// Version 1: the target quantity of starting numbers for each mode, from 0 (Easy) to 4.
const V1_TARGET_GIVENS: [usize; V1_MODES as usize] = [44, 37, 31, 27, 24];

/// Version 1: the quantity of attempts to get to the target. The generator keeps the best attempt.
const V1_ATTEMPTS: u64 = 12;

/// SplitMix64. It is small, fast and fully specified, thus the sequence never changes.
#[derive(Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A value in 0..n.
    pub fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }

    fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = self.below(i + 1);
            items.swap(i, j);
        }
    }
}

/// A puzzle: the solution and the starting numbers (0 = empty).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Puzzle {
    pub solution: [[u8; 9]; 9],
    pub givens: [[u8; 9]; 9],
}

impl Puzzle {
    pub fn givens_count(&self) -> usize {
        self.givens.iter().flatten().filter(|&&v| v != 0).count()
    }
}

/// The puzzle of `(seed, mode)` that is not symmetric, with the current version of the generator. It
/// is the same puzzle as before the symmetry option (PUZ-5).
pub fn generate(seed: u64, mode: u8) -> Puzzle {
    generate_with_symmetry(seed, mode, false)
}

/// The puzzle of `(seed, mode, symmetric)` with the current version of the generator. If
/// `generate_early` started the same puzzle, this gives its result (it waits for the end of that
/// calculation).
pub fn generate_with_symmetry(seed: u64, mode: u8, symmetric: bool) -> Puzzle {
    EARLY.get(&(seed, mode, symmetric), || generate_v2(seed, mode, symmetric))
}

/// The key of a puzzle of the current version: the seed, the mode and the symmetry (PUZ-5, PUZ-7).
pub type PuzzleKey = (u64, u8, bool);

/// The puzzle of the next new game, made while the New game panel is open or the result card shows
/// (PUZ-5, PUZ-7: the same puzzle).
static EARLY: crate::early::Early<PuzzleKey, Puzzle> = crate::early::Early::new();

/// Starts to make the puzzle of `(seed, mode, symmetric)` on a background thread, so that Start new
/// game and a level button of the result card do not wait for the generator.
pub fn generate_early(seed: u64, mode: u8, symmetric: bool) {
    EARLY.start((seed, mode, symmetric), |&(seed, mode, symmetric)| generate_v2(seed, mode, symmetric));
}

/// Waits for the end of the puzzle that started early, if there is one (for the bench).
pub fn finish_early() {
    EARLY.finish(|&(seed, mode, symmetric)| generate_v2(seed, mode, symmetric));
}

/// The puzzle of `(seed, mode, symmetric)` with a version of the generator. Version 1 has only the
/// modes 0..5 and has no symmetry (PUZ-8): a symmetric puzzle of version 1 is a bug of the caller.
pub fn generate_version(version: u8, seed: u64, mode: u8, symmetric: bool) -> Puzzle {
    match version {
        1 => {
            assert!(!symmetric, "generator version 1 has no symmetry");
            generate_v1(seed, mode)
        }
        2 => generate_v2(seed, mode, symmetric),
        _ => panic!("generator version {version} is not known"),
    }
}

/// Removes starting numbers while the solver, limited to the level of the mode, can complete the grid.
/// Then it grades the puzzle. If the puzzle does not need the level of the mode, it starts again.
/// The symmetry changes the random sequence (the bit 40 of the start value): a puzzle that is not
/// symmetric has the same sequence as before the symmetry option (PUZ-5).
fn generate_v2(seed: u64, mode: u8, symmetric: bool) -> Puzzle {
    assert!(mode < MODES, "mode {mode} is not in 0..{MODES}");
    let level = MODE_LEVELS[mode as usize];
    let attempt = |attempt: u64| -> (Level, Puzzle) {
        let mut rng = Rng::new(seed ^ (u64::from(mode) << 56) ^ (u64::from(VERSION) << 48) ^ attempt.wrapping_mul(0xA24B_AED4_963E_E407) ^ (u64::from(symmetric) << 40));
        let solution = full_grid(&mut rng);
        let givens = dig(&mut rng, solution, level, MIN_GIVENS[mode as usize], symmetric);
        let grade = solver::grade(&givens, level).expect("the dig keeps a puzzle that the solver completes");
        (grade, Puzzle { solution, givens })
    };
    // The attempts do not depend on each other: after the first attempt (it is often correct), the
    // threads make batches of attempts. The results are examined in the order of the attempts, thus
    // the puzzle is the same as with one attempt after the other (PUZ-5).
    let threads = crate::system::threads() as u64;
    let mut best: Option<(Level, Puzzle)> = None;
    let mut next = 0;
    while next < MAX_ATTEMPTS {
        let n = if next == 0 { 1 } else { threads.min(MAX_ATTEMPTS - next) };
        let batch: Vec<(Level, Puzzle)> = if n == 1 {
            vec![attempt(next)]
        } else {
            std::thread::scope(|s| {
                let handles: Vec<_> = (next..next + n).map(|a| s.spawn(move || attempt(a))).collect();
                handles.into_iter().map(|h| h.join().expect("generator thread")).collect()
            })
        };
        next += n;
        for (grade, p) in batch {
            if grade == level {
                return p;
            }
            if best.as_ref().map(|(g, _)| grade > *g).unwrap_or(true) {
                best = Some((grade, p));
            }
        }
    }
    best.expect("one attempt or more").1
}

/// Removes starting numbers in a random order. A removal stays only if the solver, limited to `level`,
/// can complete the grid. Thus the puzzle has only one solution.
///
/// If `symmetric`, the order is an order of the pairs: a cell and its partner (`partner`) go out
/// together, and the center cell goes out alone. A pair that goes below `min_givens` stays, and the
/// removal continues with the other pairs (thus a symmetric Easy puzzle has 44 or 45 starting
/// numbers, PUZ-9). If not `symmetric`, the order and the steps are the same as before the symmetry
/// option (PUZ-5).
fn dig(rng: &mut Rng, solution: [[u8; 9]; 9], level: Level, min_givens: usize, symmetric: bool) -> [[u8; 9]; 9] {
    let mut givens = solution;
    // One cell of each pair: the cell that is before its partner (or is its partner: the center).
    let mut order: Vec<Cell> = cells().filter(|&cell| !symmetric || cell <= partner(cell)).collect();
    rng.shuffle(&mut order);
    let mut count = 81;
    for cell in order {
        if count <= min_givens {
            break;
        }
        let other = partner(cell);
        let group: &[Cell] = if symmetric && other != cell { &[cell, other] } else { &[cell] };
        if count < min_givens + group.len() {
            continue;
        }
        let keep: [u8; 2] = std::array::from_fn(|k| group.get(k).map_or(0, |&(r, c)| givens[r as usize][c as usize]));
        for &(r, c) in group {
            givens[r as usize][c as usize] = 0;
        }
        if solver::grade(&givens, level).is_some() {
            count -= group.len();
        } else {
            for (&(r, c), v) in group.iter().zip(keep) {
                givens[r as usize][c as usize] = v;
            }
        }
    }
    givens
}

/// The partner of a cell under a half turn of the board: (8 - r, 8 - c). The center is its own
/// partner (PUZ-8).
pub fn partner((r, c): Cell) -> Cell {
    (8 - r, 8 - c)
}

fn generate_v1(seed: u64, mode: u8) -> Puzzle {
    assert!(mode < V1_MODES, "mode {mode} is not in 0..5");
    let target = V1_TARGET_GIVENS[mode as usize];
    let mut best: Option<Puzzle> = None;
    for attempt in 0..V1_ATTEMPTS {
        let mut rng = Rng::new(seed ^ (u64::from(mode) << 56) ^ attempt.wrapping_mul(0xA24B_AED4_963E_E407));
        let solution = full_grid(&mut rng);
        let p = remove_cells(&mut rng, solution, target);
        let better = match &best {
            None => true,
            Some(b) => p.givens_count().abs_diff(target) < b.givens_count().abs_diff(target),
        };
        if better {
            best = Some(p);
        }
        if best.as_ref().map(|b| b.givens_count() == target).unwrap_or(false) {
            break;
        }
    }
    best.expect("one attempt or more")
}

/// A complete, correct grid. The rng sets the order of the digits that the solver tries.
fn full_grid(rng: &mut Rng) -> [[u8; 9]; 9] {
    let mut order = [[0u8; 9]; 81];
    for digits in &mut order {
        *digits = std::array::from_fn(|k| k as u8 + 1);
        rng.shuffle(digits);
    }
    let mut fill = Fill { grid: [[0; 9]; 9], used: [[0; 9]; 3] };
    let ok = fill.fill(&order, 0);
    assert!(ok, "an empty grid always has a solution");
    fill.grid
}

/// The backtracking of `full_grid`: the grid, and the digits that each row, column and box has.
struct Fill {
    grid: [[u8; 9]; 9],
    /// `used[0][r]`, `used[1][c]` and `used[2][b]`: bit d - 1 for each digit d of the row, the
    /// column and the box.
    used: [[u16; 9]; 3],
}

impl Fill {
    /// Fills the cells from `i` (in the order of `cells`), each with the first digit of its order
    /// that its row, column and box do not have.
    fn fill(&mut self, order: &[[u8; 9]; 81], i: usize) -> bool {
        if i == 81 {
            return true;
        }
        let (r, c) = (i / 9, i % 9);
        let b = r / 3 * 3 + c / 3;
        for d in order[i] {
            let bit = 1 << (d - 1);
            if (self.used[0][r] | self.used[1][c] | self.used[2][b]) & bit == 0 {
                self.grid[r][c] = d;
                self.used[0][r] |= bit;
                self.used[1][c] |= bit;
                self.used[2][b] |= bit;
                if self.fill(order, i + 1) {
                    return true;
                }
                self.grid[r][c] = 0;
                self.used[0][r] &= !bit;
                self.used[1][c] &= !bit;
                self.used[2][b] &= !bit;
            }
        }
        false
    }
}

fn allowed(grid: &[[u8; 9]; 9], (r, c): Cell, d: u8) -> bool {
    crate::game::peers((r, c)).all(|(i, j)| grid[i as usize][j as usize] != d)
}

/// Version 1: removes starting numbers in a random order while the puzzle keeps only one solution,
/// until the target quantity is reached.
fn remove_cells(rng: &mut Rng, solution: [[u8; 9]; 9], target: usize) -> Puzzle {
    let mut givens = solution;
    let mut order: Vec<Cell> = cells().collect();
    rng.shuffle(&mut order);
    let mut count = 81;
    for (r, c) in order {
        if count <= target {
            break;
        }
        let keep = givens[r as usize][c as usize];
        givens[r as usize][c as usize] = 0;
        if count_solutions(&givens, 2) == 1 {
            count -= 1;
        } else {
            givens[r as usize][c as usize] = keep;
        }
    }
    Puzzle { solution, givens }
}

/// Counts the solutions of a grid, up to `limit`.
pub fn count_solutions(grid: &[[u8; 9]; 9], limit: usize) -> usize {
    let mut rows = [0u16; 9];
    let mut cols = [0u16; 9];
    let mut boxes = [0u16; 9];
    let mut empty = Vec::new();
    for (r, c) in cells() {
        let (ri, ci) = (r as usize, c as usize);
        let v = grid[ri][ci];
        if v == 0 {
            empty.push((ri, ci));
        } else {
            let bit = 1u16 << v;
            let b = ri / 3 * 3 + ci / 3;
            if rows[ri] & bit != 0 || cols[ci] & bit != 0 || boxes[b] & bit != 0 {
                return 0;
            }
            rows[ri] |= bit;
            cols[ci] |= bit;
            boxes[b] |= bit;
        }
    }
    let mut count = 0;
    search(&mut empty, &mut rows, &mut cols, &mut boxes, limit, &mut count);
    count
}

fn search(
    empty: &mut Vec<(usize, usize)>,
    rows: &mut [u16; 9],
    cols: &mut [u16; 9],
    boxes: &mut [u16; 9],
    limit: usize,
    count: &mut usize,
) {
    if *count >= limit {
        return;
    }
    if empty.is_empty() {
        *count += 1;
        return;
    }
    // Select the empty cell with the fewest possible digits.
    let mut best = 0;
    let mut best_free = u16::MAX;
    let mut best_n = 10;
    for (i, &(r, c)) in empty.iter().enumerate() {
        let free = !(rows[r] | cols[c] | boxes[r / 3 * 3 + c / 3]) & 0b11_1111_1110;
        let n = free.count_ones();
        if n < best_n {
            best = i;
            best_free = free;
            best_n = n;
            if n <= 1 {
                break;
            }
        }
    }
    if best_n == 0 {
        return;
    }
    let (r, c) = empty.swap_remove(best);
    let b = r / 3 * 3 + c / 3;
    let mut free = best_free;
    while free != 0 {
        let bit = free & free.wrapping_neg();
        free &= free - 1;
        rows[r] |= bit;
        cols[c] |= bit;
        boxes[b] |= bit;
        search(empty, rows, cols, boxes, limit, count);
        rows[r] &= !bit;
        cols[c] &= !bit;
        boxes[b] &= !bit;
        if *count >= limit {
            break;
        }
    }
    empty.push((r, c));
    let last = empty.len() - 1;
    empty.swap(best, last);
}

/// The solution is a complete, correct sudoku (PUZ-1).
pub fn is_valid_solution(grid: &[[u8; 9]; 9]) -> bool {
    cells().all(|(r, c)| {
        let v = grid[r as usize][c as usize];
        (1..=9).contains(&v) && allowed(grid, (r, c), v)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_puzzle() {
        for mode in 0..MODES {
            assert_eq!(generate(42, mode), generate(42, mode));
        }
        assert_ne!(generate(1, 2), generate(2, 2));
    }

    #[test]
    fn puzzles_are_unique_and_agree_with_solution() {
        for seed in 0..6u64 {
            for mode in 0..MODES {
                let p = generate(seed, mode);
                assert!(is_valid_solution(&p.solution));
                assert_eq!(count_solutions(&p.givens, 2), 1);
                for (r, c) in cells() {
                    let g = p.givens[r as usize][c as usize];
                    assert!(g == 0 || g == p.solution[r as usize][c as usize]);
                }
            }
        }
    }

    #[test]
    fn easy_has_the_most_givens() {
        // PUZ-6: Easy stops at 44 starting numbers. The other modes remove as many as possible.
        for seed in 0..8u64 {
            assert_eq!(generate(seed, 0).givens_count(), 44);
            for mode in 1..MODES {
                assert!(generate(seed, mode).givens_count() < 44);
            }
        }
    }

    #[test]
    fn version_1_puzzles_do_not_change() {
        // PUZ-5: the app examines the games that it saved before version 2 with version 1. These
        // puzzles come from the generator before version 2 (commit 0b855b0).
        for (seed, mode, givens) in [
            (42, 0, "097105060020978053503000908702010800361800040958207316000081030005000621000504789"),
            (42, 2, "800500700046180500017230000200006070900700600001008009009000000105070040470600920"),
            (7, 4, "500060000020030900090508000900000400210080000037010002300000890040000050005700000"),
        ] {
            let p = generate_version(1, seed, mode, false);
            let s: String = p.givens.iter().flatten().map(|v| char::from(b'0' + v)).collect();
            assert_eq!(s, givens, "seed {seed} mode {mode}");
        }
        let avg = |mode| (0..8u64).map(|s| generate_version(1, s, mode, false).givens_count()).sum::<usize>() as f64 / 8.0;
        let a: Vec<f64> = (0..V1_MODES).map(avg).collect();
        for w in a.windows(2) {
            assert!(w[0] > w[1], "{a:?}");
        }
    }

    #[test]
    fn symmetric_puzzles() {
        // PUZ-8: a half turn of the board does not change the starting cells. PUZ-9: a symmetric
        // Easy puzzle has 44 or 45 starting numbers. PUZ-2: only one solution.
        for seed in 0..4u64 {
            for mode in 0..MODES {
                let p = generate_with_symmetry(seed, mode, true);
                for cell @ (r, c) in cells() {
                    let (i, j) = partner(cell);
                    assert_eq!(p.givens[r as usize][c as usize] != 0, p.givens[i as usize][j as usize] != 0, "seed {seed} mode {mode}");
                }
                assert_eq!(count_solutions(&p.givens, 2), 1);
                if mode == 0 {
                    assert!((44..=45).contains(&p.givens_count()), "seed {seed}: {}", p.givens_count());
                }
            }
        }
    }
}
