//! The puzzle generator (REQUIREMENTS.md §14.5).
//!
//! `generate(seed, mode)` always gives the same puzzle for the same pair (INVARIANTS.md PUZ-5). The
//! generator uses its own random number generator, so that a change of a library does not change the
//! puzzles of the saved seeds. Each puzzle has only one solution (PUZ-2).

use crate::game::{cells, Cell};

pub const MODES: u8 = 5;
pub const MIDDLE_MODE: u8 = 2;

/// The target quantity of starting numbers for each mode, from 0 (Easy) to 4 (Hard). Only the
/// generator uses these values (PUZ-6).
const TARGET_GIVENS: [usize; MODES as usize] = [44, 37, 31, 27, 24];

/// The quantity of attempts to get to the target. The generator keeps the best attempt.
const ATTEMPTS: u64 = 12;

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

    pub fn shuffle<T>(&mut self, items: &mut [T]) {
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

pub fn generate(seed: u64, mode: u8) -> Puzzle {
    assert!(mode < MODES, "mode {mode} is not in 0..5");
    let target = TARGET_GIVENS[mode as usize];
    let mut best: Option<Puzzle> = None;
    for attempt in 0..ATTEMPTS {
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
    let mut grid = [[0u8; 9]; 9];
    let mut order: Vec<Vec<u8>> = (0..81)
        .map(|_| {
            let mut d: Vec<u8> = (1..=9).collect();
            rng.shuffle(&mut d);
            d
        })
        .collect();
    let ok = fill(&mut grid, &mut order);
    assert!(ok, "an empty grid always has a solution");
    grid
}

fn fill(grid: &mut [[u8; 9]; 9], order: &mut Vec<Vec<u8>>) -> bool {
    let Some((r, c)) = cells().find(|&(r, c)| grid[r as usize][c as usize] == 0) else {
        return true;
    };
    let digits = order[(r * 9 + c) as usize].clone();
    for d in digits {
        if allowed(grid, (r, c), d) {
            grid[r as usize][c as usize] = d;
            if fill(grid, order) {
                return true;
            }
            grid[r as usize][c as usize] = 0;
        }
    }
    false
}

fn allowed(grid: &[[u8; 9]; 9], (r, c): Cell, d: u8) -> bool {
    crate::game::peers((r, c)).all(|(i, j)| grid[i as usize][j as usize] != d)
}

/// Removes starting numbers in a random order while the puzzle keeps only one solution, until the
/// target quantity is reached.
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
    fn easier_modes_have_more_givens() {
        let avg = |mode| (0..8u64).map(|s| generate(s, mode).givens_count()).sum::<usize>() as f64 / 8.0;
        let a: Vec<f64> = (0..MODES).map(avg).collect();
        for w in a.windows(2) {
            assert!(w[0] > w[1], "{a:?}");
        }
    }
}
