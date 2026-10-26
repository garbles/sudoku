# Sudoku puzzle generation: research

This document records the results of a web search about algorithms that make Sudoku puzzles. It
also compares these algorithms with `src/generator.rs`. Date of the search: 2026-10-25.

## 1. Summary

- Almost all generators use the same two steps. First, they make a full, correct grid. Then they
  remove numbers ("dig holes") while the puzzle keeps only one solution.
- The quantity of starting numbers does not give a good measure of difficulty. A good generator
  uses a solver that copies human techniques, and it grades each puzzle by the hardest technique
  that is necessary.
- To get a puzzle of a specific difficulty, generators make many puzzles and discard the puzzles
  that do not agree with the target. Hard puzzles take much more time to find than easy puzzles.
- A uniqueness check with a fast backtracking solver that stops at two solutions is sufficient for
  9x9 puzzles.

## 2. Step 1: make a full grid

### 2.1 Random backtracking

Fill the cells in order. For each cell, try the digits in a random order. If no digit is possible,
go back to the last cell and try its next digit. An empty 9x9 grid always has a solution, and this
search finds one quickly. The "Minimum Remaining Values" heuristic (fill the cell with the fewest
possible digits first) makes the search faster [1].

`src/generator.rs` uses this method (`full_grid` and `fill`).

### 2.2 Transformations of a known grid

Some operations change a correct grid into a different correct grid:

- Relabel the digits (9! ways).
- Change the order of the three bands, or of the three stacks (3! ways each).
- Change the order of the rows in a band, or of the columns in a stack (3! ways for each band and
  each stack).
- Transpose the grid (2 ways).

Without relabelling, there are 3!^8 x 2 = 3,359,232 such transformations. With relabelling, there
are 1,218,998,108,160 [9][10]. There are 6,670,903,752,021,072,936,960 full grids, but only
5,472,730,538 grids that are "essentially different" under these operations [11].

These operations also keep the uniqueness and the difficulty of a puzzle. Thus a program can keep
a small list of good puzzles and show each one in many different forms. This is very fast, but the
player can possibly see the same pattern of starting numbers again.

## 3. Step 2: remove numbers

### 3.1 Dig holes with a uniqueness check

1. Put the 81 cells in a random order.
2. For each cell: remove its number. If the puzzle now has more than one solution, put the number
   back.
3. Stop at a target quantity of starting numbers, or when no more numbers can be removed.

This is the standard method [1][2]. If the loop continues until no number can be removed, the
result is a *minimal* puzzle: each starting number is necessary.

`src/generator.rs` uses this method (`remove_cells`). It stops at a target quantity for each mode
(44, 37, 31, 27, 24) and makes up to 12 attempts to get to the target.

### 3.2 How to examine uniqueness

- **Backtracking that counts to two.** Search for solutions. Stop when the search finds the second
  solution. This is fast for 9x9 grids. `count_solutions(grid, 2)` in `src/generator.rs` does this.
- **Constraint propagation and search.** Norvig's solver removes candidates with two rules (a cell
  with one candidate, and a digit with one place in a unit). Then it searches from the cell with
  the fewest candidates [12]. This is much faster than plain backtracking on hard grids.
- **Exact cover (Algorithm X, Dancing Links).** Each placement "digit d in cell (r, c)" is a row of
  a 729 x 324 matrix. The columns are the four constraints: each cell has one digit, and each row,
  column and box has each digit one time. Knuth's Algorithm X finds all exact covers. Dancing Links
  makes the backtracking steps fast [5][6][7].
- **SAT solvers.** Find a solution. Add a clause that blocks it. Solve again. If the solver finds
  a second solution, the puzzle is not unique [2]. This is slower than a special Sudoku solver for
  9x9 grids [3].
- **Specialized fast solvers.** tdoku (C++) uses bit operations on bands and a large set of
  inference rules. Its author reports that it is the fastest solver on hard puzzles and on puzzles
  with no solution or many solutions [13]. For a generator, the speed of the "two solutions" case
  is the most important, because the generator rejects many removals.

### 3.3 Symmetry

Many published puzzles have a symmetric pattern of starting numbers. To get this, the generator
removes a cell and its symmetric partners together. Simon Tatham's "Solo" supports no symmetry,
2-way and 4-way rotation, and 2-, 4- and 8-way reflection. Its default is 2-way rotation [14].
The KSudoku documentation says that puzzles with no symmetry are usually slightly harder, and that
hard, highly symmetric puzzles take more time to generate [15].

### 3.4 Speed problem: the heavy tail

Norvig found that most random puzzles solve in less than 0.01 s, but some take 190 s or more. The
cause is a bad early choice in the search, not only the puzzle [12]. A generator on a small device
must thus limit the work of each attempt (for example, with a node limit) and start again if the
limit is reached.

## 4. Difficulty

### 4.1 The quantity of starting numbers is a weak measure

A puzzle with 24 starting numbers can need only singles, and a puzzle with 30 can need advanced
techniques. Research about human solvers found that difficulty has two sources: the complexity of
each logic step, and the structure of the dependencies between the steps [4]. Pelánek's best metric
had a correlation of 0.95 with the solving time of humans, on data from more than 2,000 puzzles
[4]. The Umbrella Method paper also says that the difficulty must come from the techniques that are
necessary [8].

### 4.2 Grade with a human-style solver

Make a solver that uses only human techniques, in a fixed order from easy to hard. The difficulty
of the puzzle is the hardest technique that the solver must use. Usual techniques, from easy to
hard:

1. Full house, naked single, hidden single.
2. Locked candidates (pointing, claiming).
3. Naked and hidden pairs, triples, quads.
4. Fish (X-Wing, Swordfish, Jellyfish), XY-Wing, colouring.
5. Chains (forcing chains, AIC) and other advanced techniques.
6. Guessing (backtracking).

Solo's levels and techniques [14]:

| Level        | Techniques that the solver can use                        |
|--------------|-----------------------------------------------------------|
| Trivial      | Positional elimination in blocks only                     |
| Basic        | Positional and numeric elimination (singles)              |
| Intermediate | + Intersectional analysis (locked candidates)             |
| Advanced     | + Set elimination (naked and hidden subsets)              |
| Extreme      | + Forcing chains                                          |
| Unreasonable | + Recursion (guessing)                                    |

HoDoKu gives each technique a level (Easy, Medium, Hard, Unfair, Extreme) and a score. The score
of a puzzle is the sum of the scores of its steps. The level of a puzzle is at least the level of
its hardest step, and a high score can make the level higher [16]. HoDoKu says that its rating is
configurable, thus it is not an absolute scale. It recommends Sudoku Explainer for a more stable
rating [16].

**Sudoku Explainer (SE)** by Nicolas Juillerat is the community reference scale (approximately 1.0
to 12.0). Each technique has a fixed value, and the rating of a puzzle is the value of its hardest
step [17][18]. The search did not find a reliable source for the value of each technique.

### 4.3 Generate to a target difficulty

**Solo's method [14].** This method is good for this project:

1. Make a full grid.
2. Put the cells (or symmetric groups of cells) in a random order.
3. For each one: remove it. Run the human-style solver, limited to the techniques of the target
   level. If the solver can complete the grid, keep the removal. If not, put the numbers back.
4. At the end, grade the puzzle. If the hardest necessary technique is the target level, use the
   puzzle. If the puzzle is easier, start again with a new grid.

The solver in step 3 completes the grid only with logic, thus each accepted puzzle has only one
solution. The generator does not need a separate uniqueness check (except at the "guessing"
level). Step 3 makes sure that the puzzle is not too hard. Step 4 makes sure that it is not too
easy. The source comment says: "Loop until we get a grid of the required difficulty. This is
nasty, but it seems to be unpleasantly hard to generate difficult grids otherwise."

**Filter method (HoDoKu) [16].** Make puzzles with the standard method. Grade each one. Discard the
puzzles that are too easy, too hard, or that contain an unwanted technique. HoDoKu can also search
for puzzles that contain one specific technique, for practice.

**Add or remove numbers (Umbrella Method) [8].** Make a unique puzzle. Then add or remove starting
numbers until the grade is at the target.

**Problem at the low end.** Solo's own notes say that users find its lowest level harder than the
easy puzzles in newspapers. The author thinks that easy puzzles have many possible moves at each
step, so the player does not have to search for the one possible move [14]. A grade that counts the
quantity of possible moves at each step (not only the hardest technique) can help with this.

## 5. Bias of random generators

Generators of the "make a grid, then remove numbers" type do not give a uniform sample of all
puzzles. The distribution of the quantity of starting numbers depends on the procedure. Berthier
studied this and made a "controlled-bias" generator that corrects the bias with known weights
[19][20]. One source reports a mean of approximately 26.6 starting numbers for the estimated true
distribution of minimal puzzles, and a lower mean for a usual generator [20]. (The search tool gave
these values from a summary; read Table 6.1 of [20] before you use them.) An open-source generator
reports that most of its puzzles have 23 to 26 starting numbers, and that it cannot control the
quantity or the difficulty [23]. This bias is not important for a game, but it is important for
statistics about puzzles.

## 6. Limits

- **17** is the minimum quantity of starting numbers for a puzzle with only one solution. McGuire,
  Tugemann and Civario proved this in 2012 with an exhaustive computer search for 16-number puzzles
  [21][22]. Thus a target of less than 17 is not possible, and targets near 17 are very slow to get
  with random digging.
- With fewer than 8 different digits in the starting numbers, a puzzle always has two or more
  solutions (the two missing digits can change places) [12].

## 7. Comparison with `src/generator.rs`

| Item                   | `src/generator.rs`                         | Alternative from the research                  |
|------------------------|--------------------------------------------|------------------------------------------------|
| Full grid              | Random backtracking                        | Same. Transformations are an option.           |
| Removal                | Random order, target quantity of numbers   | Random order, limit by a human-style solver    |
| Uniqueness             | Backtracking, stop at 2 solutions          | Comes from the human-style solver              |
| Difficulty             | Quantity of starting numbers (44 ... 24)   | Hardest necessary technique (and step count)   |
| Symmetry               | None                                       | Optional 2-way rotation                        |
| Determinism (PUZ-5)    | SplitMix64, own RNG                        | Same method is possible with each alternative  |

Possible changes (not in REQUIREMENTS.md; ask the owner first):

1. Add a human-style solver (singles, locked candidates, subsets, fish, ...). Use it to grade the
   puzzles, and possibly also to show hints.
2. Change the five modes to technique levels, and generate them with Solo's method (section 4.3).
   Keep the determinism: the seed and the attempt number set the random sequence.
3. Limit the time of each attempt for the hard modes, because the tablet is slow and the search for
   a hard puzzle can need many attempts.
4. If generation on the tablet is too slow, keep a list of graded puzzles and use the
   transformations of section 2.2 to make different forms of them.

## 8. Recommendation: a world-class generator

This section gives the design that the research supports. It is a recommendation only. The owner
must approve each part before it goes into REQUIREMENTS.md or INVARIANTS.md.

### 8.1 Goals

A world-class generator gives puzzles that:

1. Always have only one solution.
2. Have the difficulty that the player selected, and the same difficulty each time (not only the
   same quantity of starting numbers).
3. Feel fair: a human can solve them with logic only, and no step needs a guess (except at a level
   that the owner specifies for guessing).
4. Look good: a balanced and, as an option, symmetric pattern of starting numbers.
5. Are different each time, and the same for the same seed (PUZ-5).
6. Come quickly on the tablet, with no wait that the player can see.

### 8.2 The pipeline

```
seed ─▶ full grid ─▶ dig (logic-limited) ─▶ grade ─▶ quality filters ─▶ accept
            ▲                                  │            │
            └──────── new attempt ◀────────────┴────────────┘  (too easy, or bad pattern)
```

1. **Full grid.** Random backtracking with the project RNG (section 2.1). This step is already
   fast and correct.
2. **Dig, limited by logic.** Use Solo's method (section 4.3). Remove cells (or symmetric groups)
   in a random order. Keep a removal only if the human-style solver, limited to the techniques of
   the target level, can complete the grid. This gives uniqueness and a maximum difficulty in one
   step.
3. **Grade.** Solve the final puzzle again with all techniques and record a difficulty profile
   (section 8.3). Reject the puzzle if it is easier than the target.
4. **Quality filters.** Reject the puzzle if it fails a filter of section 8.4.
5. **Independent check.** Count the solutions with a different solver (the backtracking solver
   that stops at two). Debug builds and tests always do this. This makes sure that an error in a
   technique of the human-style solver cannot give a puzzle with two solutions.

### 8.3 Difficulty: a profile, not one number

Grade each puzzle with a profile from the human-style solver:

- **Hardest technique.** The level of the most difficult step that is necessary. This is the
  primary value (as in Solo and Sudoku Explainer).
- **Quantity of hard steps.** One X-Wing is easier than five X-Wings.
- **Breadth.** The mean quantity of moves that are possible at each step. Easy puzzles have many
  possible moves, so the player does not have to search for the one move. This is the fix for the
  low-end problem that Solo's author records (section 4.2).
- **Bottlenecks.** The quantity of steps where only one move is possible. Many bottlenecks make a
  puzzle feel harder.

Each mode is then a range of the profile, for example:

| Mode   | Hardest technique                 | Other limits (examples)              |
|--------|-----------------------------------|--------------------------------------|
| Easy   | Naked and hidden singles          | High breadth, few bottlenecks        |
| 2      | Singles                           | Lower breadth                        |
| Middle | Locked candidates                 | —                                    |
| 4      | Naked and hidden subsets          | —                                    |
| Hard   | Fish, XY-Wing, simple chains      | No guessing                          |

The values in this table are examples only. Calibrate them with real data: Pelánek showed that a
model of human steps can predict human solving time well [4]. This app records the time of each
completed game, with its mode and seed, in SQLite (`store.rs`). If the app also records the
profile of each puzzle, the
owner can examine which profile values agree with the real solving times, and adjust the ranges.

### 8.4 Quality filters

- No box, row or column is full of starting numbers. No box is empty. Solo's notes recommend a
  limit on the maximum quantity of starting numbers in one box [14].
- The starting numbers include at least 8 different digits (a necessary condition, section 6).
- Optional 2-way rotational symmetry, as in most published puzzles. It makes hard puzzles slower
  to find (section 3.3), thus it is a setting, or it applies only to the easier modes.
- For easy modes, do not dig to a minimal puzzle. Stop at a target range of starting numbers.
  Extra starting numbers increase the breadth.

### 8.5 Speed on the tablet

- **Fast core.** Keep the candidates of each cell as a 9-bit mask, and the digits of each row,
  column and box as masks (as `count_solutions` does now). Each technique of the human-style solver
  uses these masks. Do not allocate memory in the inner loops.
- **Work limit.** Give each attempt a limit of solver steps. If the attempt gets to the limit,
  start a new attempt (section 3.4). The attempt number is part of the RNG seed, thus the result
  stays deterministic.
- **Generate before the player asks.** Make the next puzzle of each mode in the background while
  the player plays. Store it with the saved state.
- **Fallback.** If the hard mode is still too slow, ship a small list of graded puzzles. Show each
  one through a random transformation (section 2.2), selected by the seed. The transformations
  keep uniqueness and difficulty, and 1.2 x 10^12 forms of each puzzle are possible.

### 8.6 Determinism and versions

- Keep the own RNG (SplitMix64), so that a library update does not change puzzles (PUZ-5).
- A change to the generator changes the puzzle of each seed. The saved game (`game.json`) stores
  the full `Game`, thus a new version does not break a game in progress. But the results table
  stores only the mode, the quantity of starting numbers and the seed. After a change, an old seed
  gives a different puzzle. Give the generator a version number, and store it (and, as an option,
  the profile of section 8.3) in each result row.

### 8.7 Tests

- For many seeds in each mode: the puzzle has only one solution (two independent solvers agree),
  the profile is in the range of the mode, and the quality filters pass.
- The same seed and mode always give the same puzzle (PUZ-5).
- Each technique of the human-style solver has its own tests, with known puzzles from published
  collections where that technique is necessary.
- A speed test: the time to generate each mode on aarch64 (Docker) stays below a limit.
- A distribution test: the profiles of a large sample are different between the modes, with
  little overlap.

### 8.8 Order of work

1. Write the human-style solver and its technique tests. Use it first only to grade the current
   puzzles, and record the profiles. This shows how much the difficulty of the current modes
   changes.
2. Change the dig step to Solo's method, and the modes to profile ranges.
3. Add the quality filters, the work limit and the background generation.
4. Calibrate the ranges with the recorded solving times.

## 9. Sources

1. [Evaluating SAT and SMT Solvers on Large-Scale Sudoku Puzzles (arXiv 2501.08569)](https://arxiv.org/pdf/2501.08569)
2. [Effective problem solving using SAT solvers (arXiv 1906.06251)](https://arxiv.org/pdf/1906.06251)
3. [tdoku benchmarks](https://github.com/t-dillon/tdoku/blob/master/benchmarks/README.md)
4. [Pelánek: Difficulty Rating of Sudoku Puzzles by a Computational Model](https://www.fi.muni.cz/~xpelanek/publications/flairs-sudoku.pdf)
5. [Exact Cover (Columbia report)](https://www.cs.columbia.edu/~sedwards/classes/2021/4995-fall/reports/Exact-Cover.pdf)
6. [Solving Sudoku using Knuth's Algorithm X](https://shivankaul.com/blog/sudoku)
7. [xcov: exact cover crate for Rust](https://lib.rs/crates/xcov)
8. [The Umbrella Method for Compiling Sudoku (Swansea, MCM 2008)](https://math.swansea.ac.uk/staff/jhg/papers/MCM2008-Robert-Claire-Richard.pdf)
9. [Mathematics of Sudoku (encyclopedia.pub)](https://encyclopedia.pub/entry/29043)
10. [Sudoku's Hidden Symmetry: The 3,359,232 Faces of the Same Puzzle](https://thesudoku.com/blog/2026/04/15/sudokus-hidden-symmetry-the-3359232-faces-of-the-same-puzzle/)
11. [Mathematics of Sudoku (EPFL Graph Search)](https://graphsearch.epfl.ch/en/concept/2912292)
12. [Peter Norvig: Solving Every Sudoku Puzzle](https://norvig.com/sudoku.html)
13. [tdoku repository](https://github.com/t-dillon/tdoku)
14. [Simon Tatham's Puzzles, `solo.c` (GitHub mirror)](https://github.com/ghewgill/puzzles/blob/master/solo.c)
15. [KSudoku documentation](https://docs.kde.org/stable5/en/ksudoku/ksudoku/interface.html)
16. [HoDoKu, Chapter 4: Creating Sudokus](https://hodoku.sourceforge.net/en/docs_cre.php)
17. [Henz et al., IWAAI 2007 (mentions Sudoku Explainer)](https://www.comp.nus.edu.sg/~henz/papers/ps/IWAAI2007.pdf)
18. [SukakuExplainer (Sudoku Explainer fork)](https://github.com/SudokuMonster/SukakuExplainer)
19. [Berthier: Unbiased Statistics of a CSP - A Controlled-Bias Generator (arXiv 1111.4083)](https://arxiv.org/abs/1111.4083)
20. [Berthier: Pattern-Based Constraint Satisfaction and Logic Puzzles (arXiv 1304.1628)](https://arxiv.org/pdf/1304.1628)
21. [McGuire et al.: There is no 16-Clue Sudoku (arXiv 1201.0749)](https://arxiv.org/pdf/1201.0749)
22. [MIT Technology Review: Mathematicians Solve Minimum Sudoku Problem](https://www.technologyreview.com/2012/01/06/188520/mathematicians-solve-minimum-sudoku-problem/)
23. [@radcliffe/sudoku-generator (npm)](https://www.npmjs.org/package/%40radcliffe%2Fsudoku-generator)
