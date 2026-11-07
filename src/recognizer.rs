//! Reads handwriting in one cell (REQUIREMENTS.md §5.1, INVARIANTS.md INP-6).
//!
//! The result is a number from 1 to 9, or None ("not 1–9": a scribble, a letter, 0, or a number with
//! two digits or more). There is no other result.
//!
//! The recognizer compares the ink with real handwriting: the UJI Pen Characters database (60
//! writers; refer to assets/README.md). It calculates two features of the ink: the directions of the
//! pen movement in an 8 x 8 grid, and the pen path. Then it finds the nearest examples. The examples
//! include characters that are not 1 to 9 (0, letters and symbols). Ink that is nearer to those
//! examples is "not 1–9". Before the match, the recognizer rejects ink that is too small, ink with two
//! or more characters side by side (for example 12) and scribbles.
//!
//! Each example is stored in the two pen directions (`PEN_DIRECTIONS`, INP-17). The ink of the player
//! is not reversed: its features are calculated one time, as written (INP-18).

use std::sync::OnceLock;

use crate::early::Early;
use crate::system::threads;

pub type Point = (f32, f32);
pub type Stroke = Vec<Point>;

/// The ink is too small to be a number if the box of the ink is smaller than this (px).
const MIN_SIZE: f32 = 18.0;
/// The largest ratio of the ink length to the size of the ink. Numbers have a ratio of approximately
/// 4 or less. A scribble goes many times across the same area, thus its ratio is larger.
const MAX_DENSITY: f32 = 6.0;
/// A digit is accepted if its distance is not more than ALPHA times the distance of the nearest
/// example that is not a digit. A value more than 1 prefers digits: in a sudoku cell, a digit is
/// more probable than a letter. The test `writer_independent_accuracy` sets this value.
const ALPHA: f32 = 1.4;
/// The quantity of nearest digit examples that vote.
const K: usize = 3;

const GRID: usize = 8;
const DIRS: usize = 8;
const PATH_POINTS: usize = 24;
const PATH_WEIGHT: f32 = 0.25;
/// The copies of each digit example: (slant, rotation in degrees, width scale). Handwriting of different
/// writers has a different slant, rotation and width. The slant is the change of x for each unit of
/// height.
const DIGIT_COPIES: [(f32, f32, f32); 9] = [
    (0.0, 0.0, 1.0),
    (-0.12, 0.0, 1.0),
    (0.12, 0.0, 1.0),
    (-0.25, 0.0, 1.0),
    (0.25, 0.0, 1.0),
    (0.0, -8.0, 1.0),
    (0.0, 8.0, 1.0),
    (0.0, 0.0, 0.8),
    (0.0, 0.0, 1.2),
];
/// The one copy of an example that is not a digit: no change.
const OTHER_COPIES: [(f32, f32, f32); 1] = [(0.0, 0.0, 1.0)];
/// The stored items of each copy of an example: one as written and one as its full reversal (the last
/// stroke first, and each stroke from its end to its start). `example_features` adds them together,
/// thus a copy with only one direction cannot occur (INP-17).
const PEN_DIRECTIONS: usize = 2;
const DIM: usize = GRID * GRID * DIRS + 2 * PATH_POINTS;

/// Reads the ink with the examples of all 60 writers. If `read_early` started a reading of the
/// same strokes, this gives its result (it waits for the end of that reading).
pub fn recognize(strokes: &[Stroke]) -> Option<u8> {
    EARLY.get(&strokes.to_vec(), || default().read(strokes))
}

/// The reading of the pending ink that started before the end of the pause (INP-5).
static EARLY: Early<Vec<Stroke>, Option<u8>> = Early::new();

/// Starts to read `strokes` on a background thread. The app calls this at each pen lift, so that
/// `recognize` at the end of the pause does not wait for the recognizer. The reading uses one
/// thread fewer than `recognize`: the main loop can draw the ink of the next stroke.
pub fn read_early(strokes: &[Stroke]) {
    EARLY.start(strokes.to_vec(), |s| default().read_in(s, (threads() - 1).max(1)));
}

/// Waits for the end of the reading that started early, if there is one (for the bench).
pub fn finish_early() {
    EARLY.finish(|s| default().read(s));
}

/// The nearest examples of the ink (for an examination of the recognizer).
pub fn matches(strokes: &[Stroke]) -> Option<Match> {
    default().matches(strokes)
}

/// Prepares the stored examples (approximately 40 000). The app calls this one time at each start, on
/// a background thread, so that the first number does not wait for it. The features stay in memory
/// only, and not on disk (R§5.1).
pub fn warm_up() {
    default();
}

fn default() -> &'static Recognizer {
    static R: OnceLock<Recognizer> = OnceLock::new();
    R.get_or_init(|| Recognizer::new(samples().iter().chain(typeface_examples().iter()).chain(owner_examples().iter())))
}

static OWNER_DATA: &str = include_str!("../assets/owner-handwriting.jsonl");

/// The examples of assets/owner-handwriting.jsonl: handwriting of the owner on the device. The
/// corpus build collects them (src/corpus.rs, examples/corpus.rs, the skill `remarkable-corpus`).
/// Each line has the class and the strokes, in a box of 0..255 (as the database). The writer is 0.
pub fn owner_examples() -> &'static [Sample] {
    static S: OnceLock<Vec<Sample>> = OnceLock::new();
    S.get_or_init(|| {
        #[derive(serde::Deserialize)]
        struct Line {
            class: u8,
            strokes: Vec<Stroke>,
        }
        OWNER_DATA
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| serde_json::from_str::<Line>(l).expect("assets/owner-handwriting.jsonl: a line is not correct"))
            // The class is 0 to 9 (INP-6). An example with a different class is not used.
            .filter(|l| l.class <= 9)
            .map(|l| Sample { class: l.class, writer: 0, strokes: l.strokes })
            .collect()
    })
}

/// Examples with the shapes of the typeface of the screen: a closed 4, a 9 with a round loop and a
/// straight tail, and a 1 as a straight line. Few writers of the database write these shapes (or write
/// them from the bottom up), but players copy the digits of the board. The list has each shape one
/// time: `example_features` adds the reversal, as for all examples. A reversed item in this list also
/// would store the shape two times in one direction, and duplicates can give one shape more of the K
/// votes (INP-17). The writer is 0 (not a writer of the database).
fn typeface_examples() -> &'static [Sample] {
    static S: OnceLock<Vec<Sample>> = OnceLock::new();
    S.get_or_init(|| {
        // Strokes in a box of 100 x 140 (y down).
        let line = |pts: &[(f32, f32)]| -> Stroke {
            let mut out = Vec::new();
            for w in pts.windows(2) {
                let n = (dist(w[0], w[1]) / 4.0).ceil().max(1.0) as usize;
                out.extend((0..n).map(|k| {
                    let t = k as f32 / n as f32;
                    (w[0].0 + t * (w[1].0 - w[0].0), w[0].1 + t * (w[1].1 - w[0].1))
                }));
            }
            out.push(*pts.last().unwrap());
            out
        };
        let arc = |cx: f32, cy: f32, rx: f32, ry: f32, a0: f32, a1: f32| -> Stroke {
            let n = ((a1 - a0).abs() / 6.0).ceil() as usize;
            (0..=n)
                .map(|k| {
                    let a = (a0 + (a1 - a0) * k as f32 / n as f32).to_radians();
                    (cx + rx * a.cos(), cy + ry * a.sin())
                })
                .collect()
        };
        let join = |a: Stroke, b: Stroke| -> Stroke { a.into_iter().chain(b).collect() };
        let shapes: Vec<(u8, Vec<Stroke>)> = vec![
            // 1: a straight vertical line (written down or up).
            (1, vec![line(&[(50.0, 0.0), (50.0, 140.0)])]),
            // Closed 4 in one stroke: the vertical line, the diagonal and the bar.
            (4, vec![line(&[(70.0, 140.0), (70.0, 0.0), (5.0, 95.0), (100.0, 95.0)])]),
            // Closed 4 in two strokes: the diagonal and the bar, then the vertical line.
            (4, vec![line(&[(70.0, 0.0), (5.0, 95.0), (100.0, 95.0)]), line(&[(70.0, 10.0), (70.0, 140.0)])]),
            // 9: a round loop, then a straight tail (and the tail first, then the loop).
            (9, vec![join(arc(45.0, 40.0, 40.0, 40.0, 0.0, -360.0), line(&[(85.0, 40.0), (82.0, 140.0)]))]),
            (9, vec![join(arc(45.0, 40.0, 40.0, 40.0, 0.0, 360.0), line(&[(85.0, 40.0), (82.0, 140.0)]))]),
            (9, vec![join(line(&[(78.0, 140.0), (85.0, 40.0)]), arc(45.0, 40.0, 40.0, 40.0, 0.0, -340.0))]),
            (9, vec![join(line(&[(60.0, 125.0), (70.0, 140.0), (85.0, 110.0), (85.0, 40.0)]), arc(45.0, 40.0, 40.0, 40.0, 0.0, -340.0))]),
        ];
        shapes
            .into_iter()
            .map(|(class, strokes)| {
                // The same box of 0..255 as the database (the larger side is 255).
                let (x0, y0, x1, y1) = bbox(strokes.iter().flatten());
                let k = 255.0 / (x1 - x0).max(y1 - y0);
                let strokes: Vec<Stroke> =
                    strokes.iter().map(|st| st.iter().map(|&(x, y)| ((x - x0) * k, (y - y0) * k)).collect()).collect();
                Sample { class, writer: 0, strokes }
            })
            .collect()
    })
}

/// One example of handwriting: the class (1 to 9, or 0 for "not 1–9"), the writer (1 to 60) and the
/// strokes, in a box of 0..255.
pub struct Sample {
    pub class: u8,
    pub writer: u8,
    pub strokes: Vec<Stroke>,
}

static DATA: &[u8] = include_bytes!("../assets/handwriting.bin");

/// All examples of assets/handwriting.bin.
pub fn samples() -> &'static [Sample] {
    static S: OnceLock<Vec<Sample>> = OnceLock::new();
    S.get_or_init(|| {
        assert_eq!(&DATA[..4], b"HW01", "handwriting data format");
        let count = u16::from_le_bytes([DATA[4], DATA[5]]) as usize;
        let mut i = 6;
        let mut next = || {
            i += 1;
            DATA[i - 1]
        };
        (0..count)
            .map(|_| {
                let (class, writer, n) = (next(), next(), next());
                let strokes = (0..n)
                    .map(|_| {
                        let m = next();
                        (0..m).map(|_| (next() as f32, next() as f32)).collect()
                    })
                    .collect();
                Sample { class, writer, strokes }
            })
            // The class is 0 to 9 (INP-6). An example with a different class is not used.
            .filter(|s| s.class <= 9)
            .collect()
    })
}

/// The examples are in blocks of LANES examples. The distances of one block are calculated together,
/// one example in each lane (SIMD).
const LANES: usize = 16;

/// The values of the distance are added in chunks of this size (the same sums for each lane).
const CHUNK: usize = 64;

struct Recognizer {
    classes: Vec<u8>,
    /// The features of block b, value j, lane l are at (b * DIM + j) * LANES + l. The lanes after the
    /// last example are zero.
    blocks: Vec<f32>,
}

/// The result of the match.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Match {
    /// The digit with the most votes of the nearest digit examples.
    pub digit: u8,
    /// The distance to the nearest example of that digit.
    pub digit_distance: f32,
    /// The distance to the nearest example that is not 1 to 9.
    pub other_distance: f32,
}

impl Recognizer {
    pub fn new<'a>(samples: impl Iterator<Item = &'a Sample>) -> Recognizer {
        // The features of each example do not depend on the other examples: the threads calculate
        // parts of the list, and the result keeps the order of the list.
        let samples: Vec<&Sample> = samples.collect();
        let threads = threads();
        // Each part gives the classes and the features of the stored items (DIM values for each
        // class, in one vector).
        let parts: Vec<(Vec<u8>, Vec<f32>)> = std::thread::scope(|scope| {
            let handles: Vec<_> = samples
                .chunks(samples.len().div_ceil(threads).max(1))
                .map(|part| {
                    scope.spawn(move || {
                        // The exact size at the start: a vector that grows makes copies and frees
                        // large blocks.
                        let items: usize = part.iter().map(|s| copies_of(s.class).len() * PEN_DIRECTIONS).sum();
                        let (mut classes, mut features) = (Vec::with_capacity(items), Vec::with_capacity(items * DIM));
                        for s in part {
                            example_features(s, &mut classes, &mut features);
                        }
                        (classes, features)
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().expect("recognizer thread")).collect()
        });
        let n: usize = parts.iter().map(|p| p.0.len()).sum();
        let mut classes = Vec::with_capacity(n);
        let mut blocks = vec![0f32; n.div_ceil(LANES) * DIM * LANES];
        let all = parts.iter().flat_map(|(c, f)| c.iter().zip(f.chunks_exact(DIM)));
        for (i, (&class, f)) in all.enumerate() {
            classes.push(class);
            let (b, l) = (i / LANES, i % LANES);
            for (j, &v) in f.iter().enumerate() {
                blocks[(b * DIM + j) * LANES + l] = v;
            }
        }
        Recognizer { classes, blocks }
    }

    pub fn read(&self, strokes: &[Stroke]) -> Option<u8> {
        self.read_in(strokes, threads())
    }

    /// `read` with `threads` ranges of the examples (the same result for each quantity).
    fn read_in(&self, strokes: &[Stroke], threads: usize) -> Option<u8> {
        let m = self.matches_in(strokes, threads)?;
        (m.digit_distance <= ALPHA * m.other_distance).then_some(m.digit)
    }

    /// The nearest examples, or None if the ink is not one character (too small, two characters side
    /// by side, or a scribble).
    pub fn matches(&self, strokes: &[Stroke]) -> Option<Match> {
        self.matches_in(strokes, threads())
    }

    /// `matches` with `threads` ranges of the examples.
    fn matches_in(&self, strokes: &[Stroke], threads: usize) -> Option<Match> {
        let strokes: Vec<Stroke> = strokes.iter().filter(|s| !s.is_empty()).cloned().collect();
        if strokes.is_empty() {
            return None;
        }
        let (x0, y0, x1, y1) = bbox(strokes.iter().flatten());
        let size = (x1 - x0).max(y1 - y0);
        if size < MIN_SIZE || side_by_side(&strokes) {
            return None;
        }
        let length: f32 = strokes.iter().map(|s| path_length(s)).sum();
        if length / size > MAX_DENSITY {
            return None;
        }
        // The features of the ink, one time and as written (INP-18). The stored items have the two
        // pen directions (INP-17).
        let f = features_of(&strokes)?;

        // The threads scan ranges of the blocks. Each range keeps all the examples that it accepts,
        // in order. A range sees fewer examples than the full scan, thus its limit is never lower:
        // it accepts each example that the full scan accepts, with the full distance (the sums of
        // a block stop only when all are over the limit). The scan of the accepted examples of all
        // ranges in order then gives the same result as one scan of all blocks.
        let blocks: Vec<&[f32]> = self.blocks.chunks_exact(DIM * LANES).collect();
        let per = blocks.len().div_ceil(threads).max(1);
        let scan = |first: usize, part: &[&[f32]]| {
            let mut near = Nearest::new();
            let mut accepted = Vec::new();
            for (k, blk) in part.iter().enumerate() {
                let b = first + k;
                let sums = block_distances(&f, blk, near.limit());
                for (l, &d) in sums.iter().enumerate() {
                    let Some(&class) = self.classes.get(b * LANES + l) else { break };
                    if near.offer(d, class) {
                        accepted.push((d, class));
                    }
                }
            }
            accepted
        };
        let ranges: Vec<Vec<(f32, u8)>> = std::thread::scope(|s| {
            let scan = &scan;
            let handles: Vec<_> = blocks.chunks(per).enumerate().skip(1).map(|(r, part)| s.spawn(move || scan(r * per, part))).collect();
            let first = scan(0, &blocks[..per.min(blocks.len())]);
            std::iter::once(first).chain(handles.into_iter().map(|h| h.join().expect("recognizer thread"))).collect()
        });
        let mut near = Nearest::new();
        for (d, class) in ranges.into_iter().flatten() {
            near.offer(d, class);
        }
        let Nearest { nearest, other } = near;
        let mut votes = [0usize; 10];
        for &(_, c) in &nearest {
            votes[c as usize] += 1;
        }
        // The most votes; with equal votes, the class of the nearer example.
        let digit = nearest
            .iter()
            .map(|&(_, c)| c)
            .filter(|&c| c != 0)
            .max_by_key(|&c| (votes[c as usize], std::cmp::Reverse(nearest.iter().position(|n| n.1 == c))))?;
        let digit_distance = nearest.iter().find(|n| n.1 == digit).map(|n| n.0)?;
        Some(Match { digit, digit_distance, other_distance: other })
    }
}

/// The K nearest digit examples and the distance of the nearest example that is not a digit, in
/// the order that the examples are offered.
struct Nearest {
    nearest: [(f32, u8); K],
    other: f32,
}

impl Nearest {
    fn new() -> Nearest {
        Nearest { nearest: [(f32::MAX, 0); K], other: f32::MAX }
    }

    /// An example with a larger distance than this cannot change the result.
    fn limit(&self) -> f32 {
        self.other.max(self.nearest[K - 1].0)
    }

    /// Offers the example of `class` at the distance `d`. True if it changed the result.
    fn offer(&mut self, d: f32, class: u8) -> bool {
        let limit = if class == 0 { self.other } else { self.nearest[K - 1].0 };
        if d > limit {
            return false;
        }
        if class == 0 {
            self.other = d;
        } else {
            let mut j = K - 1;
            self.nearest[j] = (d, class);
            while j > 0 && self.nearest[j].0 < self.nearest[j - 1].0 {
                self.nearest.swap(j, j - 1);
                j -= 1;
            }
        }
        true
    }
}

/// The copies of an example of `class`: digits also get copies with a different slant, rotation and
/// width.
fn copies_of(class: u8) -> &'static [(f32, f32, f32)] {
    if class == 0 { &OTHER_COPIES } else { &DIGIT_COPIES }
}

/// The class and the features of an example and of its copies (`copies_of`), each in the
/// PEN_DIRECTIONS directions (INP-17): first as written, then fully reversed.
/// Adds the class to `classes` and the DIM features to `features` for each stored item. A copy with
/// no size has no features, thus it gives no item in the two directions.
fn example_features(s: &Sample, classes: &mut Vec<u8>, features: &mut Vec<f32>) {
    for &(k, rot, sx) in copies_of(s.class) {
        let (sin, cos) = rot.to_radians().sin_cos();
        let ink = Flat::new(&s.strokes, |(x, y)| {
            let (x, y) = ((x + k * (255.0 - y) - 128.0) * sx, y - 128.0);
            (x * cos - y * sin + 128.0, x * sin + y * cos + 128.0)
        });
        let Some(ink) = normalize(ink) else { continue };
        flat_features(&ink, features);
        flat_features(&ink.reversed(), features);
        classes.extend([s.class; PEN_DIRECTIONS]);
    }
}

/// The squared distances from `f` to the LANES examples of a block. Each lane adds the squares in the
/// same order: in chunks of CHUNK values, each chunk from the first value. If all the distances are
/// more than `limit`, the calculation can stop early: then the result has distances more than
/// `limit`, but not the full distances. The partial sums only increase, thus a distance that is more
/// than `limit` at a chunk is more than `limit` at the end.
fn block_distances(f: &[f32], blk: &[f32], limit: f32) -> [f32; LANES] {
    let mut sum = [0f32; LANES];
    for start in (0..DIM).step_by(CHUNK) {
        let mut cs = [-0f32; LANES];
        for j in start..(start + CHUNK).min(DIM) {
            let q = f[j];
            let row: &[f32; LANES] = blk[j * LANES..(j + 1) * LANES].try_into().unwrap();
            for l in 0..LANES {
                let x = q - row[l];
                cs[l] += x * x;
            }
        }
        for l in 0..LANES {
            sum[l] += cs[l];
        }
        if sum.iter().all(|&s| s > limit) {
            break;
        }
    }
    sum
}

fn bbox<'a>(points: impl Iterator<Item = &'a Point>) -> (f32, f32, f32, f32) {
    points.fold((f32::MAX, f32::MAX, f32::MIN, f32::MIN), |(a, b, c, d), &(x, y)| {
        (a.min(x), b.min(y), c.max(x), d.max(y))
    })
}

fn path_length(s: &[Point]) -> f32 {
    s.windows(2).map(|w| dist(w[0], w[1])).sum()
}

fn dist(a: Point, b: Point) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

/// True if the strokes make two or more groups side by side, with a horizontal space between them, and
/// each group is large enough to be a character.
fn side_by_side(strokes: &[Stroke]) -> bool {
    let (_, y0, _, y1) = bbox(strokes.iter().flatten());
    let height = (y1 - y0).max(MIN_SIZE);
    let mut spans: Vec<(f32, f32, f32)> = strokes
        .iter()
        .map(|s| {
            let (a, b, c, d) = bbox(s.iter());
            (a, c, (c - a).max(d - b))
        })
        .collect();
    spans.sort_by(|a, b| a.0.total_cmp(&b.0));
    // Merge the spans that overlap. Each merged span is one group.
    let mut groups: Vec<(f32, f32, f32)> = Vec::new();
    let gap = 0.08 * height;
    for (a, b, size) in spans {
        match groups.last_mut() {
            Some(g) if a <= g.1 + gap => {
                g.1 = g.1.max(b);
                g.2 = g.2.max(size);
            }
            _ => groups.push((a, b, size)),
        }
    }
    groups.iter().filter(|g| g.2 >= 0.35 * height).count() >= 2
}

/// Strokes in one list of points (fewer allocations): stroke k is `points[starts[k]..starts[k + 1]]`.
struct Flat {
    points: Vec<Point>,
    starts: Vec<usize>,
}

impl Flat {
    /// The strokes with `map` on each point.
    fn new(strokes: &[Stroke], map: impl Fn(Point) -> Point) -> Flat {
        let mut points = Vec::with_capacity(strokes.iter().map(Vec::len).sum());
        let mut starts = Vec::with_capacity(strokes.len() + 1);
        starts.push(0);
        for st in strokes {
            points.extend(st.iter().map(|&p| map(p)));
            starts.push(points.len());
        }
        Flat { points, starts }
    }

    /// The full reversal: the last stroke first, and each stroke from its end to its start.
    fn reversed(mut self) -> Flat {
        let total = self.points.len();
        self.points.reverse();
        self.starts.reverse();
        self.starts.iter_mut().for_each(|s| *s = total - *s);
        self
    }

    fn strokes(&self) -> impl Iterator<Item = &[Point]> {
        self.starts.windows(2).map(|w| &self.points[w[0]..w[1]])
    }
}

/// The strokes in a unit box (the same scale for x and y), with the center of the ink at the center.
fn normalize(mut ink: Flat) -> Option<Flat> {
    let (x0, y0, x1, y1) = bbox(ink.points.iter());
    let size = (x1 - x0).max(y1 - y0);
    if size.is_nan() || size <= 0.0 {
        return None;
    }
    let (ox, oy) = (0.5 - (x1 - x0) / size / 2.0, 0.5 - (y1 - y0) / size / 2.0);
    for p in &mut ink.points {
        *p = ((p.0 - x0) / size + ox, (p.1 - y0) / size + oy);
    }
    Some(ink)
}

/// The feature vector of the strokes, as written: the pen directions in a grid, then the pen path.
/// None if the ink has no size.
fn features_of(strokes: &[Stroke]) -> Option<Vec<f32>> {
    let ink = normalize(Flat::new(strokes, |p| p))?;
    let mut f = Vec::with_capacity(DIM);
    flat_features(&ink, &mut f);
    Some(f)
}

/// Adds the DIM features of the ink (from `normalize`) to `out`.
fn flat_features(ink: &Flat, out: &mut Vec<f32>) {
    let start = out.len();
    directions(ink, out);
    pen_path(&ink.points, out);
    debug_assert_eq!(out.len() - start, DIM);
}

/// The length of the pen movement in each grid cell and each of 8 directions (with the sense of the
/// movement), with a small blur. Adds these GRID x GRID x DIRS values (with a length of 1 together)
/// to `features`.
fn directions(ink: &Flat, features: &mut Vec<f32>) {
    use std::f32::consts::TAU;
    let (g, d) = (GRID, DIRS);
    let mut f = [0f32; GRID * GRID * DIRS];
    for s in ink.strokes() {
        for w in s.windows(2) {
            let (a, b) = (w[0], w[1]);
            let len = dist(a, b);
            if len < 1e-6 {
                continue;
            }
            let th = (b.1 - a.1).atan2(b.0 - a.0).rem_euclid(TAU);
            let steps = (len / 0.02).ceil() as usize;
            for k in 0..steps {
                let t = (k as f32 + 0.5) / steps as f32;
                let (x, y) = (a.0 + t * (b.0 - a.0), a.1 + t * (b.1 - a.1));
                let gx = (x * g as f32 - 0.5).clamp(0.0, (g - 1) as f32);
                let gy = (y * g as f32 - 0.5).clamp(0.0, (g - 1) as f32);
                let tt = th / TAU * d as f32;
                let (ix, iy, it) = (gx.floor() as usize, gy.floor() as usize, tt.floor() as usize);
                let (fx, fy, ft) = (gx - ix as f32, gy - iy as f32, tt - it as f32);
                let w = len / steps as f32;
                for (dx, wx) in [(0, 1.0 - fx), (1, fx)] {
                    for (dy, wy) in [(0, 1.0 - fy), (1, fy)] {
                        for (dt, wt) in [(0, 1.0 - ft), (1, ft)] {
                            let (cx, cy, ct) = ((ix + dx).min(g - 1), (iy + dy).min(g - 1), (it + dt) % d);
                            f[(cy * g + cx) * d + ct] += w * wx * wy * wt;
                        }
                    }
                }
            }
        }
    }
    // The blur: each value gets 4 times itself, 2 times its 4 side neighbors and 1 time its 4
    // corner neighbors, added in the order of (dy, dx). The grid has a border of zeros: a value
    // out of the grid adds w * 0 = 0, thus the sums are the same as without the cells out of the
    // grid (all values are 0 or more), and the loop has no branches.
    const P: usize = GRID + 2;
    let mut padded = [0f32; P * P * DIRS];
    for y in 0..g {
        let row = (y + 1) * P + 1;
        padded[row * d..(row + g) * d].copy_from_slice(&f[y * g * d..(y + 1) * g * d]);
    }
    let start = features.len();
    features.resize(start + f.len(), 0f32);
    let out = &mut features[start..];
    for y in 0..g {
        for x in 0..g {
            let mut s = [0f32; DIRS];
            for dy in 0..3 {
                for dx in 0..3 {
                    let w = if dx == 1 && dy == 1 { 4.0 } else if dx == 1 || dy == 1 { 2.0 } else { 1.0 };
                    let src = ((y + dy) * P + x + dx) * d;
                    for (sc, v) in s.iter_mut().zip(&padded[src..src + DIRS]) {
                        *sc += w * v;
                    }
                }
            }
            out[(y * g + x) * d..(y * g + x + 1) * d].copy_from_slice(&s);
        }
    }
    let n = out.iter().map(|v| v * v).sum::<f32>().sqrt().max(1e-6);
    out.iter_mut().for_each(|v| *v /= n);
}

/// The strokes joined in the order of writing (`pts`), at PATH_POINTS equal distances: x0, y0, x1,
/// y1, ... Each value goes into `out` with the weight PATH_WEIGHT.
fn pen_path(pts: &[Point], out: &mut Vec<f32>) {
    let mut put = |x: f32, y: f32| out.extend([x * PATH_WEIGHT, y * PATH_WEIGHT]);
    let total: f32 = path_length(pts);
    if total <= 0.0 {
        for _ in 0..PATH_POINTS {
            put(pts[0].0, pts[0].1);
        }
        return;
    }
    let step = total / (PATH_POINTS - 1) as f32;
    let (mut acc, mut i) = (0.0, 0);
    for k in 0..PATH_POINTS {
        let target = k as f32 * step;
        while i + 1 < pts.len() && acc + dist(pts[i], pts[i + 1]) < target {
            acc += dist(pts[i], pts[i + 1]);
            i += 1;
        }
        if i + 1 < pts.len() {
            let d = dist(pts[i], pts[i + 1]);
            let t = if d > 0.0 { ((target - acc) / d).clamp(0.0, 1.0) } else { 0.0 };
            put(pts[i].0 + t * (pts[i + 1].0 - pts[i].0), pts[i].1 + t * (pts[i + 1].1 - pts[i].1));
        } else {
            let p = pts[pts.len() - 1];
            put(p.0, p.1);
        }
    }
}

/// An example of the digit `d` by the writer `writer`, in a box of `size` px at (x, y). The tests use
/// it as the ink of a player.
pub fn example_ink(d: u8, writer: u8, x: f32, y: f32, size: f32) -> Option<Vec<Stroke>> {
    let s = samples().iter().find(|s| s.class == d && s.writer == writer)?;
    Some(s.strokes.iter().map(|st| st.iter().map(|&(px, py)| (x + px / 255.0 * size, y + py / 255.0 * size)).collect()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::Rng;

    /// The writers 41 to 60 are not in the examples of this test: the result shows the accuracy for a
    /// new writer.
    fn split() -> (Recognizer, Vec<&'static Sample>) {
        let r = Recognizer::new(
            samples().iter().filter(|s| s.writer <= 40).chain(typeface_examples().iter()).chain(owner_examples().iter()),
        );
        (r, samples().iter().filter(|s| s.writer > 40).collect())
    }

    /// Puts an example on a cell of the board, with the size of handwriting (approximately 100 px).
    fn place(s: &Sample) -> Vec<Stroke> {
        s.strokes.iter().map(|st| st.iter().map(|&(x, y)| (300.0 + x * 0.4, 600.0 + y * 0.4)).collect()).collect()
    }

    /// The full reversal of the ink, on the lists of strokes: the last stroke first, and each stroke
    /// from its end to its start. This does not use `Flat::reversed`.
    fn full_reversal(ink: &[Stroke]) -> Vec<Stroke> {
        ink.iter().rev().map(|s| s.iter().rev().copied().collect()).collect()
    }

    #[test]
    fn a_reading_that_starts_early_gives_the_same_result() {
        // INP-5, INP-6: the app starts the reading at each pen lift. The result at the end of the
        // pause must be the same as a reading at that time, also when a stroke came after the start
        // (other strokes) and when the reading did not finish yet.
        let r = default();
        let inks: Vec<Vec<Stroke>> = samples().iter().step_by(97).map(place).collect();
        for (k, ink) in inks.iter().enumerate() {
            let want = r.read(ink);
            read_early(ink);
            if k % 2 == 0 {
                finish_early();
            }
            assert_eq!(recognize(ink), want, "ink {k}");
            // The first strokes only: the early reading of all strokes is not used.
            if ink.len() > 1 {
                read_early(ink);
                assert_eq!(recognize(&ink[..1]), r.read(&ink[..1]), "ink {k}, first stroke");
            }
        }
    }

    #[test]
    fn the_ranges_of_the_scan_do_not_change_the_result() {
        // `matches` scans ranges of the examples on threads. The result must be the same, bit for
        // bit, as one scan of all examples.
        let r = default();
        for s in samples().iter().step_by(23) {
            let ink = place(s);
            let one = r.matches_in(&ink, 1).map(|m| (m.digit, m.digit_distance.to_bits(), m.other_distance.to_bits()));
            for threads in 2..=4 {
                let got = r.matches_in(&ink, threads).map(|m| (m.digit, m.digit_distance.to_bits(), m.other_distance.to_bits()));
                assert_eq!(got, one, "writer {} class {}, {threads} ranges", s.writer, s.class);
            }
        }
    }

    /// The result of one ink of the test set: the class, the reading of the ink as written, and the
    /// reading of its full reversal.
    struct Reading {
        class: u8,
        written: Option<u8>,
        reversed: Option<u8>,
    }

    /// The readings of the 3360 inks of writers 41 to 60, with the examples of writers 1 to 40 (INP-19).
    /// The accuracy tests use the same readings, thus the test run makes them one time only.
    fn readings() -> &'static [Reading] {
        static R: OnceLock<Vec<Reading>> = OnceLock::new();
        R.get_or_init(|| {
            let (r, test) = split();
            test.iter()
                .map(|s| {
                    let ink = place(s);
                    Reading { class: s.class, written: r.read(&ink), reversed: r.read(&full_reversal(&ink)) }
                })
                .collect()
        })
    }

    /// The accuracy limits for the 360 digits and the 3000 other characters of the test set. The
    /// result of `got` is the reading of each ink.
    fn assert_accuracy(name: &str, got: impl Fn(&Reading) -> Option<u8>) {
        let (mut n, mut ok, mut wrong) = (0, 0, 0);
        let (mut on, mut rejected) = (0, 0);
        for s in readings() {
            let got = got(s);
            if s.class != 0 {
                n += 1;
                match got {
                    Some(d) if d == s.class => ok += 1,
                    Some(_) => wrong += 1,
                    None => {}
                }
            } else {
                on += 1;
                if got.is_none() {
                    rejected += 1;
                }
            }
        }
        assert_eq!((n, on), (360, 3000), "{name}: the inks of the test set");
        let (acc, wrong_rate, rej) = (ok as f64 / n as f64, wrong as f64 / n as f64, rejected as f64 / on as f64);
        eprintln!(
            "{name}: {:.1}% of digits correct, {:.1}% a wrong digit; {:.1}% of other characters rejected",
            acc * 100.0,
            wrong_rate * 100.0,
            rej * 100.0
        );
        // Measured (macOS, one run): 98.6% correct, 0.6% a wrong digit, 90.5% of other characters
        // rejected. The limit 90.0% has owner approval (2026-11-06). The examples of the owner are
        // also in this test, thus an added example can change the result. The target of
        // REQUIREMENTS.md §5.1 is 96% correct and 1.2% wrong.
        assert!(acc >= 0.975, "{name}: digits read correctly: {:.1}%", acc * 100.0);
        assert!(wrong_rate <= 0.012, "{name}: digits read as a different digit: {:.1}%", wrong_rate * 100.0);
        assert!(rej >= 0.90, "{name}: other characters rejected: {:.1}%", rej * 100.0);
    }

    #[test]
    fn writer_independent_accuracy() {
        // R§5.1: the ink as written.
        assert_accuracy("new writers", |s| s.written);
    }

    #[test]
    fn full_reversals_of_new_writers_agree_with_the_accuracy_limits() {
        // R§5.1, INP-19: the full reversal of each ink, with the same limits as the ink as written.
        assert_accuracy("new writers, full reversals", |s| s.reversed);
    }

    #[test]
    fn an_ink_and_its_full_reversal_give_the_same_result() {
        // INP-19: this rule is for this test set only. It is not for all ink, and it is not for partly
        // reversed ink.
        let all = readings();
        assert_eq!(all.len(), 3360);
        let different: Vec<_> =
            all.iter().enumerate().filter(|(_, s)| s.written != s.reversed).map(|(i, s)| (i, s.class, s.written, s.reversed)).collect();
        assert!(different.is_empty(), "{} of 3360 inks (index, class, written, reversed): {different:?}", different.len());
    }

    /// The stored items of one sample: the classes, and DIM features for each item.
    fn stored_items(s: &Sample) -> (Vec<u8>, Vec<f32>) {
        let (mut classes, mut features) = (Vec::new(), Vec::new());
        example_features(s, &mut classes, &mut features);
        (classes, features)
    }

    /// True if the two lists of features are the same, bit for bit.
    fn same_bits(a: &[f32], b: &[f32]) -> bool {
        a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
    }

    /// INP-17: the quantity of stored items of a sample, and of the items in each direction. The stored
    /// items of a copy are at known positions: first as written, then the full reversal. The full
    /// reversal of the sample has the same items in the opposite order. Thus the test counts the
    /// positions that agree. It does not search for equal items, because the full reversal of a
    /// symmetrical shape can be equal to the shape.
    fn assert_two_directions(name: &str, s: &Sample) {
        let want = if s.class == 0 { 2 } else { 18 };
        let (classes, features) = stored_items(s);
        assert_eq!(classes.len(), want, "{name}: stored items of class {}", s.class);
        assert!(classes.iter().all(|&c| c == s.class), "{name}: classes of the stored items");
        assert_eq!(features.len(), want * DIM, "{name}: features of the stored items");

        let (_, opposite) = stored_items(&Sample { class: s.class, writer: s.writer, strokes: full_reversal(&s.strokes) });
        assert_eq!(opposite.len(), want * DIM, "{name}: features of the items of the full reversal");
        let item = |list: &'_ [f32], k: usize| -> Vec<f32> { list[k * DIM..(k + 1) * DIM].to_vec() };
        let same = |k: usize, other: usize| same_bits(&item(&features, k), &item(&opposite, other));
        let written = (0..want).step_by(2).filter(|&k| same(k, k + 1)).count();
        let reversed = (1..want).step_by(2).filter(|&k| same(k, k - 1)).count();
        assert_eq!((written, reversed), (want / 2, want / 2), "{name}: items as written and items fully reversed");
    }

    /// Ink with many strokes: empty strokes at the start, in the middle and at the end, and strokes of
    /// different lengths, in a box of 0..255.
    fn many_strokes() -> Vec<Stroke> {
        vec![
            vec![],
            vec![(10.0, 10.0)],
            vec![(20.0, 30.0), (200.0, 40.0)],
            vec![],
            vec![],
            vec![(50.0, 60.0), (60.0, 200.0), (240.0, 250.0)],
            vec![(100.0, 100.0), (90.0, 10.0)],
            vec![],
        ]
    }

    #[test]
    fn each_example_has_two_or_eighteen_stored_items_in_the_two_directions() {
        // INP-17, R§5.1: for all classes and sources, 2 items for "not 1-9" and 18 for a digit. Half are
        // as written, and half are the full reversal.
        let sources = [("database", samples()), ("typeface", typeface_examples()), ("owner", owner_examples())];
        let mut total = 0;
        for (name, list) in sources {
            assert!(!list.is_empty(), "{name}: no samples");
            for (i, s) in list.iter().enumerate() {
                assert_two_directions(&format!("{name} {i}"), s);
                total += if s.class == 0 { 2 } else { 18 };
            }
        }
        assert!(samples().iter().any(|s| s.class == 0) && samples().iter().any(|s| s.class != 0));
        // The recognizer of the app stores these items, and no other items.
        assert_eq!(default().classes.len(), total, "stored items of the app");
        assert_eq!(default().blocks.len(), total.div_ceil(LANES) * DIM * LANES, "stored features of the app");

        // Many strokes with empty strokes, and a shape with a full reversal that is equal to the shape.
        for class in [0, 7] {
            assert_two_directions("many strokes", &Sample { class, writer: 0, strokes: many_strokes() });
            let symmetrical = vec![vec![(0.0, 0.0), (200.0, 100.0), (0.0, 0.0)]];
            assert_eq!(full_reversal(&symmetrical), symmetrical);
            assert_two_directions("symmetrical", &Sample { class, writer: 0, strokes: symmetrical });
        }
    }

    #[test]
    fn a_typeface_shape_has_no_separate_reversed_item() {
        // INP-17: if the list has a shape and also its reversal, one shape has two items in one direction.
        // The test compares items of different shapes, thus a symmetrical shape does not change it.
        let copies: Vec<(u8, Vec<f32>)> = typeface_examples().iter().map(|s| (s.class, stored_items(s).1)).collect();
        for (i, (_, a)) in copies.iter().enumerate() {
            for (j, (_, b)) in copies.iter().enumerate().skip(i + 1) {
                for (x, y) in a.chunks_exact(DIM).flat_map(|x| b.chunks_exact(DIM).map(move |y| (x, y))) {
                    assert!(!same_bits(x, y), "the typeface shapes {i} and {j} have the same stored item");
                }
            }
        }
    }

    #[test]
    fn a_copy_with_no_size_has_no_item_in_the_two_directions() {
        // INP-17: a copy with only one direction must not occur. Ink with no size has no features.
        for strokes in [vec![], vec![vec![]], vec![vec![(9.0, 9.0)]], vec![vec![(9.0, 9.0), (9.0, 9.0)], vec![], vec![(9.0, 9.0)]]] {
            for class in [0, 5] {
                let (classes, features) = stored_items(&Sample { class, writer: 0, strokes: strokes.clone() });
                assert!(classes.is_empty() && features.is_empty(), "class {class}, strokes {strokes:?}");
            }
        }
    }

    #[test]
    fn a_full_reversal_keeps_the_limits_of_the_strokes() {
        // INP-17 (full reversal): `full_reversal` gives the term of INVARIANTS.md.
        let ink = vec![vec![(1.0, 2.0), (3.0, 4.0)], vec![], vec![(5.0, 6.0)]];
        assert_eq!(full_reversal(&ink), vec![vec![(5.0, 6.0)], vec![], vec![(3.0, 4.0), (1.0, 2.0)]]);

        let check = |ink: &[Stroke]| {
            let once = Flat::new(ink, |p| p).reversed();
            assert_eq!(once.starts.len(), ink.len() + 1, "{ink:?}");
            assert_eq!((once.starts[0], *once.starts.last().unwrap()), (0, once.points.len()), "{ink:?}");
            assert!(once.starts.windows(2).all(|w| w[0] <= w[1]), "{ink:?}");
            assert_eq!(once.strokes().map(<[Point]>::to_vec).collect::<Vec<_>>(), full_reversal(ink), "{ink:?}");

            let (twice, first) = (once.reversed(), Flat::new(ink, |p| p));
            assert_eq!((&twice.points, &twice.starts), (&first.points, &first.starts), "{ink:?}");
            assert_eq!(twice.strokes().map(<[Point]>::to_vec).collect::<Vec<_>>(), ink, "{ink:?}");
        };
        // All the parts of the ink with many strokes: an empty stroke can be first or last, and an ink
        // can have no strokes.
        let many = many_strokes();
        for first in 0..=many.len() {
            for last in first..=many.len() {
                check(&many[first..last]);
            }
        }
        // The real examples.
        for s in samples().iter().chain(typeface_examples()).chain(owner_examples()) {
            check(&s.strokes);
        }
    }

    /// A recognizer with exactly these items (it does not add the other direction).
    fn with_items(items: &[(u8, &[f32])]) -> Recognizer {
        let mut blocks = vec![0f32; items.len().div_ceil(LANES) * DIM * LANES];
        for (i, (_, f)) in items.iter().enumerate() {
            for (j, &v) in f.iter().enumerate() {
                blocks[(i / LANES * DIM + j) * LANES + i % LANES] = v;
            }
        }
        Recognizer { classes: items.iter().map(|i| i.0).collect(), blocks }
    }

    #[test]
    fn the_ink_of_the_player_is_read_as_written() {
        // INP-18: the recognizer in this test stores each shape in one direction only. If the app also
        // read the reversal of the ink, or read only the reversal, then the distances show it. A test
        // cannot see a second calculation that has no effect on the result.
        let mut inks = 0;
        for s in samples().iter().filter(|s| s.writer == 41 && s.class != 0) {
            let (ink, reversal) = (place(s), full_reversal(&place(s)));
            let (f, g) = (features_of(&ink).unwrap(), features_of(&reversal).unwrap());
            assert_eq!(f.len(), DIM, "class {}: one set of features", s.class);
            assert!(!same_bits(&f, &g), "class {}: the ink must not be symmetrical", s.class);
            let far: f32 = f.iter().zip(&g).map(|(a, b)| (a - b) * (a - b)).sum();

            // The features are the features of the first stored item: as written, with no change.
            let (_, stored) = stored_items(s);
            assert!(same_bits(&features_of(&s.strokes).unwrap(), &stored[..DIM]), "class {}: as written", s.class);

            for threads in [1, 4] {
                let only_written = with_items(&[(s.class, f.as_slice())]);
                let m = only_written.matches_in(&ink, threads).unwrap();
                assert_eq!((m.digit, m.digit_distance), (s.class, 0.0), "class {}: the ink", s.class);
                let m = only_written.matches_in(&reversal, threads).unwrap();
                assert!((m.digit_distance - far).abs() <= far * 1e-3 && far > 0.0, "class {}: the reversal: {m:?}, far {far}", s.class);

                let only_reversed = with_items(&[(s.class, g.as_slice())]);
                let m = only_reversed.matches_in(&reversal, threads).unwrap();
                assert_eq!((m.digit, m.digit_distance), (s.class, 0.0), "class {}: the reversal", s.class);
                let m = only_reversed.matches_in(&ink, threads).unwrap();
                assert!((m.digit_distance - far).abs() <= far * 1e-3, "class {}: the ink: {m:?}, far {far}", s.class);
            }

            // The result of the reading: the digit as written, and "not 1-9" as reversed.
            let r = with_items(&[(s.class, f.as_slice()), (0, g.as_slice())]);
            assert_eq!(r.read(&ink), Some(s.class), "class {}: the ink", s.class);
            assert_eq!(r.read(&reversal), None, "class {}: the reversal", s.class);
            inks += 1;
        }
        // The database has 2 samples of each digit for each writer.
        assert_eq!(inks, 18);
    }

    #[test]
    fn the_ink_is_compared_with_all_stored_items() {
        // INP-18: the item that agrees with the ink can be at each position of the stored items, also
        // in the last block, which has empty lanes.
        let s = samples().iter().find(|s| s.writer == 41 && s.class == 4).unwrap();
        let ink = place(s);
        let (f, g) = (features_of(&ink).unwrap(), features_of(&full_reversal(&ink)).unwrap());
        let count = 3 * LANES + 5;
        for position in 0..count {
            let items: Vec<(u8, &[f32])> =
                (0..count).map(|i| if i == position { (s.class, f.as_slice()) } else { (0, g.as_slice()) }).collect();
            let r = with_items(&items);
            for threads in 1..=4 {
                let m = r.matches_in(&ink, threads).unwrap();
                assert_eq!((m.digit, m.digit_distance), (s.class, 0.0), "position {position}, {threads} ranges");
                assert!(m.other_distance > 0.0, "position {position}, {threads} ranges");
            }
        }
    }

    #[test]
    fn rejects_random_scribbles() {
        let mut rng = Rng::new(99);
        let mut rf = |a: f32, b: f32| a + (b - a) * (rng.next_u64() % 10_000) as f32 / 10_000.0;
        let mut accepted = 0;
        for _ in 0..200 {
            let mut p = (rf(250.0, 350.0), rf(550.0, 650.0));
            let stroke: Stroke = (0..60)
                .map(|_| {
                    p = ((p.0 + rf(-30.0, 30.0)).clamp(250.0, 370.0), (p.1 + rf(-30.0, 30.0)).clamp(540.0, 680.0));
                    p
                })
                .collect();
            if recognize(&[stroke]).is_some() {
                accepted += 1;
            }
        }
        assert!(accepted < 10, "{accepted} of 200 random scribbles accepted");
    }

    /// An example with a height of `h` px, with its left edge at x.
    fn at_height(d: u8, w: u8, x: f32, h: f32) -> Vec<Stroke> {
        let ink = example_ink(d, w, 0.0, 0.0, 1.0).unwrap();
        let (x0, y0, _, y1) = bbox(ink.iter().flatten());
        let s = h / (y1 - y0);
        ink.iter().map(|st| st.iter().map(|&(px, py)| (x + (px - x0) * s, 600.0 + (py - y0) * s)).collect()).collect()
    }

    #[test]
    fn rejects_two_digits_side_by_side() {
        // "12" and "15" of the new writers: two digits of the same height, with a space of 15 px.
        let (r, _) = split();
        for w in 41..=60 {
            for second in [2u8, 5] {
                let mut ink = at_height(1, w, 300.0, 90.0);
                let right = bbox(ink.iter().flatten()).2;
                ink.extend(at_height(second, w, right + 15.0, 90.0));
                assert_eq!(r.read(&ink), None, "writer {w}: 1{second}");
            }
        }
    }

    #[test]
    fn handwriting_of_the_owner_is_read() {
        // tests/fixtures/owner-ink.jsonl: the ink log of the owner on the device (2026-10-25): five
        // 4s, six 9s and six 8s. The owner writes the 4 and the 9 like the typeface, from the bottom
        // up. The typeface examples (`typeface_examples`) were made after an examination of this ink,
        // and each example also has a reversed item (INP-17), which can agree with ink from the bottom
        // up. Thus this test is not independent; `writer_independent_accuracy` is.
        let want = [4, 4, 4, 4, 4, 9, 9, 9, 9, 9, 9, 8, 8, 8, 8, 8, 8];
        let log = include_str!("../tests/fixtures/owner-ink.jsonl");
        for (i, (line, &d)) in log.lines().zip(want.iter()).enumerate() {
            let v: serde_json::Value = serde_json::from_str(line).unwrap();
            let strokes: Vec<Stroke> = serde_json::from_value(v["strokes"].clone()).unwrap();
            assert_eq!(recognize(&strokes), Some(d), "sample {i}");
        }
    }

    #[test]
    fn a_straight_line_is_a_1_in_both_directions() {
        // The owner's request (2026-10-25): a straight vertical line, written down or up, is a 1.
        for (dx, up) in [(0.0, false), (0.0, true), (10.0, false), (-10.0, true), (6.0, true)] {
            for len in [60.0, 100.0, 130.0] {
                let mut s: Stroke = (0..=40).map(|i| (300.0 + dx * i as f32 / 40.0, 600.0 + len * i as f32 / 40.0)).collect();
                if up {
                    s.reverse();
                }
                assert_eq!(recognize(&[s]), Some(1), "length {len}, slant {dx}, up {up}");
            }
        }
        // A horizontal line is not a number.
        let s: Stroke = (0..=40).map(|i| (300.0 + 100.0 * i as f32 / 40.0, 600.0)).collect();
        assert_eq!(recognize(&[s]), None);
    }

    #[test]
    fn owner_examples_are_correct() {
        // assets/owner-handwriting.jsonl: each example is a digit 1 to 9 (the corpus build collects
        // only digits), in the box 0..255, and the recognizer can read its features.
        for (i, s) in owner_examples().iter().enumerate() {
            assert!((1..=9).contains(&s.class), "example {i}: class {}", s.class);
            assert!(s.strokes.iter().flatten().all(|&(x, y)| (0.0..=255.0).contains(&x) && (0.0..=255.0).contains(&y)), "example {i}");
            assert!(features_of(&s.strokes).is_some(), "example {i}");
        }
    }

    #[test]
    fn small_ink_is_not_a_number() {
        assert_eq!(recognize(&[vec![(10.0, 10.0), (14.0, 12.0)]]), None);
        assert_eq!(recognize(&[]), None);
    }

    #[test]
    fn examples_of_all_digits_and_writers() {
        for d in 1..=9 {
            for w in 1..=60 {
                assert!(example_ink(d, w, 0.0, 0.0, 100.0).is_some(), "digit {d}, writer {w}");
            }
        }
    }
}
