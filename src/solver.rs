//! The human-style solver (REQUIREMENTS.md §14.5).
//!
//! The solver uses only the techniques that a person uses. It tries them from the easiest to the most
//! difficult, and it starts again from the easiest after each step. The most difficult technique that
//! it must use gives the level of the puzzle. The generator uses the solver to make the puzzle of each
//! mode and to grade it. The solver never guesses: if no technique of the permitted levels can make a
//! step, the solver stops.
//!
//! Each step is a logical deduction from the starting numbers. Thus, if the solver completes the grid,
//! the puzzle has only one solution (PUZ-2).

use std::sync::OnceLock;

/// The groups of techniques, from the easiest to the most difficult.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Level {
    /// Full house, naked single, hidden single.
    Singles,
    /// Pointing and claiming.
    LockedCandidates,
    /// Naked and hidden pairs, triples and quads.
    Subsets,
    /// X-Wing, Swordfish, XY-Wing.
    BasicFish,
    /// Skyscraper, 2-String Kite, XYZ-Wing, W-Wing, simple colouring, Jellyfish.
    Wings,
    /// X-Chains, XY-Chains and other alternating inference chains (AIC).
    Chains,
}

/// The techniques, in the order that the solver tries them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Technique {
    HiddenSingle,
    NakedSingle,
    Pointing,
    Claiming,
    NakedPair,
    HiddenPair,
    NakedTriple,
    HiddenTriple,
    NakedQuad,
    HiddenQuad,
    XWing,
    Swordfish,
    XYWing,
    Skyscraper,
    TwoStringKite,
    XYZWing,
    WWing,
    SimpleColouring,
    Jellyfish,
    Chain,
}

impl Technique {
    pub const ALL: [Technique; 20] = [
        Technique::HiddenSingle,
        Technique::NakedSingle,
        Technique::Pointing,
        Technique::Claiming,
        Technique::NakedPair,
        Technique::HiddenPair,
        Technique::NakedTriple,
        Technique::HiddenTriple,
        Technique::NakedQuad,
        Technique::HiddenQuad,
        Technique::XWing,
        Technique::Swordfish,
        Technique::XYWing,
        Technique::Skyscraper,
        Technique::TwoStringKite,
        Technique::XYZWing,
        Technique::WWing,
        Technique::SimpleColouring,
        Technique::Jellyfish,
        Technique::Chain,
    ];

    pub fn level(self) -> Level {
        use Technique::*;
        match self {
            HiddenSingle | NakedSingle => Level::Singles,
            Pointing | Claiming => Level::LockedCandidates,
            NakedPair | HiddenPair | NakedTriple | HiddenTriple | NakedQuad | HiddenQuad => Level::Subsets,
            XWing | Swordfish | XYWing => Level::BasicFish,
            Skyscraper | TwoStringKite | XYZWing | WWing | SimpleColouring | Jellyfish => Level::Wings,
            Chain => Level::Chains,
        }
    }
}

/// A set of cells: bit i is cell i (row * 9 + column).
type Set = u128;

struct Tables {
    /// The 9 cells of each unit: rows 0..9, columns 9..18, boxes 18..27.
    units: [[usize; 9]; 27],
    unit_set: [Set; 27],
    /// The row, the column and the box of each cell.
    cell_units: [[usize; 3]; 81],
    /// The 20 peers of each cell.
    peers: [Set; 81],
}

fn tables() -> &'static Tables {
    static T: OnceLock<Tables> = OnceLock::new();
    T.get_or_init(|| {
        let mut units = [[0; 9]; 27];
        let mut cell_units = [[0; 3]; 81];
        for i in 0..81 {
            let (r, c) = (i / 9, i % 9);
            let b = r / 3 * 3 + c / 3;
            units[r][c] = i;
            units[9 + c][r] = i;
            units[18 + b][(r % 3) * 3 + c % 3] = i;
            cell_units[i] = [r, 9 + c, 18 + b];
        }
        let unit_set = units.map(|u| u.iter().fold(0, |s, &i| s | 1 << i));
        let peers = std::array::from_fn(|i| {
            let [r, c, b] = cell_units[i];
            (unit_set[r] | unit_set[c] | unit_set[b]) & !(1 << i)
        });
        Tables { units, unit_set, cell_units, peers }
    })
}

/// The cells of a set, from the lowest.
fn members(mut s: Set) -> impl Iterator<Item = usize> {
    std::iter::from_fn(move || {
        (s != 0).then(|| {
            let i = s.trailing_zeros() as usize;
            s &= s - 1;
            i
        })
    })
}

/// The digits of a candidate mask (bit d - 1 is digit d), from the lowest.
fn digits(mut m: u16) -> impl Iterator<Item = u8> {
    std::iter::from_fn(move || {
        (m != 0).then(|| {
            let d = m.trailing_zeros() as u8 + 1;
            m &= m - 1;
            d
        })
    })
}

fn bit(d: u8) -> u16 {
    1 << (d - 1)
}

fn sees(a: usize, b: usize) -> bool {
    tables().peers[a] >> b & 1 != 0
}

/// Calls `f` with each combination of `n` items of `items`, until `f` gives true.
fn combinations(items: &[usize], n: usize, f: &mut impl FnMut(&[usize]) -> bool) -> bool {
    fn go(items: &[usize], n: usize, start: usize, pick: &mut Vec<usize>, f: &mut impl FnMut(&[usize]) -> bool) -> bool {
        if pick.len() == n {
            return f(pick);
        }
        for k in start..items.len() {
            pick.push(items[k]);
            if go(items, n, k + 1, pick, f) {
                return true;
            }
            pick.pop();
        }
        false
    }
    go(items, n, 0, &mut Vec::with_capacity(n), f)
}

/// The board of the solver: the numbers and the candidates of the empty cells.
#[derive(Clone, Debug)]
pub struct Board {
    value: [u8; 81],
    /// The candidates of each empty cell (bit d - 1 is digit d). 0 for a filled cell.
    cand: [u16; 81],
}

impl Board {
    /// The board of the starting numbers (0 = empty), with all the candidates that the numbers permit.
    pub fn new(givens: &[[u8; 9]; 9]) -> Board {
        let mut b = Board { value: [0; 81], cand: [0x1FF; 81] };
        for i in 0..81 {
            let v = givens[i / 9][i % 9];
            if v != 0 {
                b.place(i, v);
            }
        }
        b
    }

    pub fn value(&self, i: usize) -> u8 {
        self.value[i]
    }

    /// The candidates of cell i (bit d - 1 is digit d).
    pub fn candidates(&self, i: usize) -> u16 {
        self.cand[i]
    }

    pub fn is_solved(&self) -> bool {
        self.value.iter().all(|&v| v != 0)
    }

    /// Makes one step with the easiest technique that can make progress and that is not more difficult
    /// than `max`. None if no such technique can make progress.
    pub fn step(&mut self, max: Level) -> Option<Technique> {
        Technique::ALL.into_iter().take_while(|t| t.level() <= max).find(|&t| self.apply(t))
    }

    fn apply(&mut self, t: Technique) -> bool {
        use Technique::*;
        match t {
            HiddenSingle => self.hidden_single(),
            NakedSingle => self.naked_single(),
            Pointing => self.pointing(),
            Claiming => self.claiming(),
            NakedPair => self.naked_subset(2),
            HiddenPair => self.hidden_subset(2),
            NakedTriple => self.naked_subset(3),
            HiddenTriple => self.hidden_subset(3),
            NakedQuad => self.naked_subset(4),
            HiddenQuad => self.hidden_subset(4),
            XWing => self.fish(2),
            Swordfish => self.fish(3),
            Jellyfish => self.fish(4),
            XYWing => self.xy_wing(),
            XYZWing => self.xyz_wing(),
            WWing => self.w_wing(),
            Skyscraper => self.turbot(true),
            TwoStringKite => self.turbot(false),
            SimpleColouring => self.simple_colouring(),
            Chain => self.chain(),
        }
    }

    fn place(&mut self, i: usize, d: u8) {
        self.value[i] = d;
        self.cand[i] = 0;
        for p in members(tables().peers[i]) {
            self.cand[p] &= !bit(d);
        }
    }

    /// Removes the candidates `mask` from the empty cells of `cells`. True if a candidate was removed.
    fn eliminate(&mut self, cells: Set, mask: u16) -> bool {
        let mut changed = false;
        for i in members(cells) {
            if self.cand[i] & mask != 0 {
                self.cand[i] &= !mask;
                changed = true;
            }
        }
        changed
    }

    /// The empty cells that have the candidate d.
    fn with(&self, d: u8) -> Set {
        (0..81).filter(|&i| self.cand[i] & bit(d) != 0).fold(0, |s, i| s | 1 << i)
    }

    fn hidden_single(&mut self) -> bool {
        let t = tables();
        for u in 0..27 {
            for d in 1..=9 {
                let pos = t.unit_set[u] & self.with(d);
                if pos.count_ones() == 1 {
                    self.place(pos.trailing_zeros() as usize, d);
                    return true;
                }
            }
        }
        false
    }

    fn naked_single(&mut self) -> bool {
        match (0..81).find(|&i| self.cand[i].count_ones() == 1) {
            Some(i) => {
                self.place(i, self.cand[i].trailing_zeros() as u8 + 1);
                true
            }
            None => false,
        }
    }

    /// All the candidates d of a box are in one row or column: d is not in the rest of that line.
    fn pointing(&mut self) -> bool {
        let t = tables();
        for b in 18..27 {
            for d in 1..=9 {
                let pos = t.unit_set[b] & self.with(d);
                if pos == 0 {
                    continue;
                }
                let first = pos.trailing_zeros() as usize;
                for &line in &t.cell_units[first][..2] {
                    if pos & !t.unit_set[line] == 0 && self.eliminate(t.unit_set[line] & !t.unit_set[b], bit(d)) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// All the candidates d of a row or column are in one box: d is not in the rest of that box.
    fn claiming(&mut self) -> bool {
        let t = tables();
        for line in 0..18 {
            for d in 1..=9 {
                let pos = t.unit_set[line] & self.with(d);
                if pos == 0 {
                    continue;
                }
                let b = t.cell_units[pos.trailing_zeros() as usize][2];
                if pos & !t.unit_set[b] == 0 && self.eliminate(t.unit_set[b] & !t.unit_set[line], bit(d)) {
                    return true;
                }
            }
        }
        false
    }

    /// n cells of a unit have only n candidates together: these candidates are not in the other cells.
    fn naked_subset(&mut self, n: usize) -> bool {
        let t = tables();
        for u in 0..27 {
            let empty: Vec<usize> = t.units[u].iter().copied().filter(|&i| self.value[i] == 0).collect();
            if empty.len() <= n {
                continue;
            }
            let cand = self.cand;
            let mut found = None;
            combinations(&empty, n, &mut |pick| {
                let union = pick.iter().fold(0, |m, &i| m | cand[i]);
                let others = empty.iter().filter(|i| !pick.contains(i)).fold(0, |s, &i| s | 1 << i);
                let hit = union.count_ones() as usize == n && empty.iter().any(|i| !pick.contains(i) && cand[*i] & union != 0);
                if hit {
                    found = Some((others, union));
                }
                hit
            });
            if let Some((others, union)) = found {
                return self.eliminate(others, union);
            }
        }
        false
    }

    /// n candidates of a unit are only in n cells together: these cells have no other candidates.
    fn hidden_subset(&mut self, n: usize) -> bool {
        let t = tables();
        for u in 0..27 {
            let unit = t.unit_set[u];
            let open: Vec<usize> = (1..=9usize).filter(|&d| unit & self.with(d as u8) != 0).collect();
            if open.len() <= n {
                continue;
            }
            let pos: Vec<Set> = (0..=9).map(|d| if d == 0 { 0 } else { unit & self.with(d as u8) }).collect();
            let cand = self.cand;
            let mut found = None;
            combinations(&open, n, &mut |pick| {
                let cells = pick.iter().fold(0, |s, &d| s | pos[d]);
                let mask = pick.iter().fold(0u16, |m, &d| m | bit(d as u8));
                let hit = cells.count_ones() as usize == n && members(cells).any(|i| cand[i] & !mask != 0);
                if hit {
                    found = Some((cells, mask));
                }
                hit
            });
            if let Some((cells, mask)) = found {
                return self.eliminate(cells, !mask & 0x1FF);
            }
        }
        false
    }

    /// X-Wing (n = 2), Swordfish (3), Jellyfish (4): in n rows, the candidates d are in only n columns
    /// together (or the opposite). Then d is not in the other cells of these columns.
    fn fish(&mut self, n: usize) -> bool {
        let t = tables();
        for d in 1..=9 {
            let with = self.with(d);
            for base in [0, 9] {
                let cover = 9 - base;
                // For each base line: the indices of the cover lines that have d.
                let idx: Vec<u16> = (0..9)
                    .map(|k| t.units[base + k].iter().enumerate().filter(|(_, &i)| with >> i & 1 != 0).fold(0, |m, (j, _)| m | 1 << j))
                    .collect();
                let lines: Vec<usize> = (0..9).filter(|&k| (2..=n as u32).contains(&idx[k].count_ones())).collect();
                let mut found = None;
                combinations(&lines, n, &mut |pick| {
                    let cols = pick.iter().fold(0u16, |m, &k| m | idx[k]);
                    if cols.count_ones() as usize != n {
                        return false;
                    }
                    let base_cells = pick.iter().fold(0, |s, &k| s | t.unit_set[base + k]);
                    let cover_cells = (0..9).filter(|j| cols >> j & 1 != 0).fold(0, |s, j| s | t.unit_set[cover + j]);
                    let target = cover_cells & !base_cells & with;
                    if target != 0 {
                        found = Some(target);
                    }
                    target != 0
                });
                if let Some(target) = found {
                    return self.eliminate(target, bit(d));
                }
            }
        }
        false
    }

    fn bivalue(&self) -> Vec<usize> {
        (0..81).filter(|&i| self.cand[i].count_ones() == 2).collect()
    }

    /// A pivot {x, y} sees a wing {x, z} and a wing {y, z}: z is not in the cells that see both wings.
    fn xy_wing(&mut self) -> bool {
        let t = tables();
        let bi = self.bivalue();
        for &p in &bi {
            let pc = self.cand[p];
            for &w1 in bi.iter().filter(|&&w| sees(p, w)) {
                let shared = self.cand[w1] & pc;
                if shared.count_ones() != 1 {
                    continue;
                }
                let z = self.cand[w1] & !pc;
                let want = (pc & !shared) | z;
                let cand = self.cand;
                for &w2 in bi.iter().filter(|&&w| w != w1 && sees(p, w) && cand[w] == want) {
                    if self.eliminate(t.peers[w1] & t.peers[w2], z) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// A pivot {x, y, z} sees a wing {x, z} and a wing {y, z}: z is not in the cells that see all three.
    fn xyz_wing(&mut self) -> bool {
        let t = tables();
        let bi = self.bivalue();
        let cand = self.cand;
        for p in (0..81).filter(|&i| cand[i].count_ones() == 3) {
            let pc = self.cand[p];
            let wings: Vec<usize> = bi.iter().copied().filter(|&w| sees(p, w) && self.cand[w] & !pc == 0).collect();
            for (k, &w1) in wings.iter().enumerate() {
                for &w2 in &wings[k + 1..] {
                    let (c1, c2) = (self.cand[w1], self.cand[w2]);
                    let z = c1 & c2;
                    if c1 | c2 == pc && z.count_ones() == 1 && self.eliminate(t.peers[p] & t.peers[w1] & t.peers[w2], z) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// The pairs of cells that are the only two places of d in a unit (strong links). `lines_only`
    /// excludes the boxes. Each pair has the unit.
    fn strong_links(&self, d: u8, lines_only: bool) -> Vec<(usize, usize, usize)> {
        let t = tables();
        let with = self.with(d);
        let mut links: Vec<(usize, usize, usize)> = Vec::new();
        for u in 0..if lines_only { 18 } else { 27 } {
            let pos = t.unit_set[u] & with;
            if pos.count_ones() == 2 {
                let a = pos.trailing_zeros() as usize;
                let b = 127 - pos.leading_zeros() as usize;
                if !links.iter().any(|&(x, y, _)| (x, y) == (a, b)) {
                    links.push((a, b, u));
                }
            }
        }
        links
    }

    /// Two cells {x, y} that do not see each other, and a strong link on x with one end that sees each
    /// cell: y is not in the cells that see both cells.
    fn w_wing(&mut self) -> bool {
        let t = tables();
        let bi = self.bivalue();
        for (k, &a) in bi.iter().enumerate() {
            let cand = self.cand;
            for &b in bi[k + 1..].iter().filter(|&&b| cand[b] == cand[a] && !sees(a, b)) {
                for x in digits(self.cand[a]) {
                    let y = self.cand[a] & !bit(x);
                    for (p, q, _) in self.strong_links(x, false) {
                        if [p, q].contains(&a) || [p, q].contains(&b) {
                            continue;
                        }
                        let linked = (sees(p, a) && sees(q, b)) || (sees(p, b) && sees(q, a));
                        if linked && self.eliminate(t.peers[a] & t.peers[b], y) {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    /// Two strong links on d in rows or columns, with one end of each that see each other: d is not in
    /// the cells that see the two other ends. Skyscraper: the two links are both in rows or both in
    /// columns. 2-String Kite: one link is in a row and one is in a column.
    fn turbot(&mut self, parallel: bool) -> bool {
        let t = tables();
        for d in 1..=9 {
            let links = self.strong_links(d, true);
            for (k, &(a1, b1, u1)) in links.iter().enumerate() {
                for &(a2, b2, u2) in &links[k + 1..] {
                    let same_kind = (u1 < 9) == (u2 < 9);
                    if same_kind != parallel {
                        continue;
                    }
                    for (a, b) in [(a1, b1), (b1, a1)] {
                        for (c, e) in [(a2, b2), (b2, a2)] {
                            let distinct = a != c && a != e && b != c && b != e;
                            if distinct && sees(b, c) && self.eliminate(t.peers[a] & t.peers[e], bit(d)) {
                                return true;
                            }
                        }
                    }
                }
            }
        }
        false
    }

    /// Simple colouring: the strong links of d make chains of two colours. One colour is true. If two
    /// cells of one colour see each other, that colour is false. A cell that sees both colours is not d.
    fn simple_colouring(&mut self) -> bool {
        let t = tables();
        for d in 1..=9 {
            let links = self.strong_links(d, false);
            let mut colour = [0u8; 81]; // 0: no colour; 1 and 2: the two colours.
            for &(start, _, _) in &links {
                if colour[start] != 0 {
                    continue;
                }
                colour[start] = 1;
                let mut stack = vec![start];
                let mut sets = [0 as Set; 3];
                let mut odd = false;
                while let Some(i) = stack.pop() {
                    sets[colour[i] as usize] |= 1 << i;
                    for &(p, q, _) in &links {
                        let other = if p == i { q } else if q == i { p } else { continue };
                        if colour[other] == 0 {
                            colour[other] = 3 - colour[i];
                            stack.push(other);
                        } else if colour[other] == colour[i] {
                            odd = true;
                        }
                    }
                }
                if odd {
                    continue;
                }
                for c in [1, 2] {
                    if members(sets[c]).any(|i| t.peers[i] & sets[c] != 0) {
                        return self.eliminate(sets[c], bit(d));
                    }
                }
                let seen = |s: Set| members(s).fold(0, |acc, i| acc | t.peers[i]);
                let trap = seen(sets[1]) & seen(sets[2]) & !(sets[1] | sets[2]);
                if self.eliminate(trap & self.with(d), bit(d)) {
                    return true;
                }
            }
        }
        false
    }

    /// Alternating inference chains. A node is "digit d in cell i". If node A is false, the strong
    /// links (the only two candidates of a cell, or the only two places of a digit in a unit) make some
    /// nodes true, and the weak links (two candidates of a cell, or one digit in two peers) make other
    /// nodes false. If A is false makes node B true, then A or B is true. X-Chains and XY-Chains are
    /// such chains.
    fn chain(&mut self) -> bool {
        let t = tables();
        let node = |i: usize, d: u8| i * 9 + (d as usize - 1);
        let mut strong: Vec<Vec<usize>> = vec![Vec::new(); 729];
        for d in 1..=9 {
            for (p, q, _) in self.strong_links(d, false) {
                strong[node(p, d)].push(node(q, d));
                strong[node(q, d)].push(node(p, d));
            }
        }
        for i in self.bivalue() {
            let mut ds = digits(self.cand[i]);
            let (x, y) = (ds.next().unwrap(), ds.next().unwrap());
            strong[node(i, x)].push(node(i, y));
            strong[node(i, y)].push(node(i, x));
        }
        let with: Vec<Set> = (0..=9).map(|d| if d == 0 { 0 } else { self.with(d) }).collect();
        let starts: Vec<usize> = (0..729).filter(|&n| !strong[n].is_empty()).collect();
        let (mut is_true, mut is_false) = (vec![false; 729], vec![false; 729]);
        for &a in &starts {
            is_true.iter_mut().for_each(|v| *v = false);
            is_false.iter_mut().for_each(|v| *v = false);
            is_false[a] = true;
            let mut queue = std::collections::VecDeque::from([(a, false)]);
            while let Some((n, truth)) = queue.pop_front() {
                if !truth {
                    for &m in &strong[n] {
                        if !is_true[m] {
                            is_true[m] = true;
                            queue.push_back((m, true));
                        }
                    }
                } else {
                    let (i, d) = (n / 9, (n % 9) as u8 + 1);
                    for e in digits(self.cand[i] & !bit(d)) {
                        let m = node(i, e);
                        if !is_false[m] {
                            is_false[m] = true;
                            queue.push_back((m, false));
                        }
                    }
                    for j in members(t.peers[i] & with[d as usize]) {
                        let m = node(j, d);
                        if !is_false[m] {
                            is_false[m] = true;
                            queue.push_back((m, false));
                        }
                    }
                }
            }
            let (ia, da) = (a / 9, (a % 9) as u8 + 1);
            if is_true[a] {
                // A is false makes A true: A is true.
                self.place(ia, da);
                return true;
            }
            for b in (0..729).filter(|&b| b != a && is_true[b]) {
                let (ib, db) = (b / 9, (b % 9) as u8 + 1);
                let changed = if da == db {
                    self.eliminate(t.peers[ia] & t.peers[ib], bit(da))
                } else if ia == ib {
                    self.eliminate(1 << ia, !(bit(da) | bit(db)) & 0x1FF)
                } else if sees(ia, ib) {
                    self.eliminate(1 << ia, bit(db)) | self.eliminate(1 << ib, bit(da))
                } else {
                    false
                };
                if changed {
                    return true;
                }
            }
        }
        false
    }
}

/// Solves the puzzle with the techniques that are not more difficult than `max`. Gives the level of
/// the most difficult technique that was necessary, or None if the solver cannot complete the grid.
pub fn grade(givens: &[[u8; 9]; 9], max: Level) -> Option<Level> {
    let mut board = Board::new(givens);
    let mut hardest = Level::Singles;
    while !board.is_solved() {
        hardest = hardest.max(board.step(max)?.level());
    }
    Some(hardest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::{count_solutions, generate, generate_version, MODES, MODE_LEVELS};
    use std::collections::HashSet;

    /// Solves with all techniques. After each step, each empty cell keeps the digit of the solution,
    /// and each number is the digit of the solution. Gives the techniques that the solver used.
    fn solve_checked(givens: &[[u8; 9]; 9], solution: &[[u8; 9]; 9]) -> Vec<Technique> {
        let mut b = Board::new(givens);
        let mut used = Vec::new();
        while let Some(t) = b.step(Level::Chains) {
            used.push(t);
            for i in 0..81 {
                let s = solution[i / 9][i % 9];
                if b.value(i) == 0 {
                    assert!(b.candidates(i) & bit(s) != 0, "{t:?} removed {s} from cell {i}");
                } else {
                    assert_eq!(b.value(i), s, "{t:?} put a wrong number in cell {i}");
                }
            }
        }
        used
    }

    #[test]
    fn steps_never_remove_the_solution() {
        // PUZ-2: the puzzles of version 1 are not made with the solver, thus they also need the most
        // difficult techniques. Each step must agree with the only solution.
        for seed in 0..60 {
            for mode in 0..5 {
                let p = generate_version(1, seed, mode);
                solve_checked(&p.givens, &p.solution);
            }
        }
    }

    #[test]
    fn each_mode_needs_its_level() {
        // PUZ-2, PUZ-6
        for seed in 0..8 {
            for mode in 0..MODES {
                let p = generate(seed, mode);
                assert_eq!(count_solutions(&p.givens, 2), 1);
                assert_eq!(grade(&p.givens, Level::Chains), Some(MODE_LEVELS[mode as usize]), "seed {seed} mode {mode}");
                solve_checked(&p.givens, &p.solution);
            }
        }
    }

    /// An empty board: each cell has all candidates.
    fn open_board() -> Board {
        Board { value: [0; 81], cand: [0x1FF; 81] }
    }

    #[test]
    fn naked_quad() {
        // Row 0: four cells have only 1, 2, 3 and 4 together, with no pair or triple.
        let mut b = open_board();
        for (i, m) in [0b0011, 0b0110, 0b1100, 0b1001].into_iter().enumerate() {
            b.cand[i] = m;
        }
        assert!(b.naked_subset(4));
        assert!((4..9).all(|i| b.cand[i] == 0x1F0));
        assert!((9..81).all(|i| b.cand[i] == 0x1FF), "only row 0 changes");
    }

    #[test]
    fn hidden_quad() {
        // Row 0: the digits 1, 2, 3 and 4 are only in the first four cells.
        let mut b = open_board();
        for i in 4..9 {
            b.cand[i] = 0x1F0;
        }
        assert!(b.hidden_subset(4));
        assert!((0..4).all(|i| b.cand[i] == 0x00F));
    }

    #[test]
    fn jellyfish() {
        // In rows 0..4 the digit 1 is only in columns 0..4. Thus 1 is not in the rest of these columns.
        let mut b = open_board();
        for r in 0..4 {
            for c in 4..9 {
                b.cand[r * 9 + c] &= !1;
            }
        }
        assert!(b.fish(4));
        for r in 4..9 {
            for c in 0..9 {
                assert_eq!(b.cand[r * 9 + c] & 1 != 0, c >= 4, "cell ({r}, {c})");
            }
        }
    }

    #[test]
    fn the_puzzles_use_each_technique() {
        // Quads and Jellyfish are rare in real puzzles. They use the same code as the pairs, triples,
        // X-Wing and Swordfish, and the tests above examine them.
        let mut used = HashSet::new();
        for seed in 0..40 {
            for mode in 3..MODES {
                let p = generate(seed, mode);
                used.extend(solve_checked(&p.givens, &p.solution));
            }
        }
        let rare = [Technique::NakedQuad, Technique::HiddenQuad, Technique::Jellyfish];
        let missing: Vec<_> = Technique::ALL.iter().filter(|t| !used.contains(t) && !rare.contains(t)).collect();
        assert!(missing.is_empty(), "not used: {missing:?}");
    }
}
