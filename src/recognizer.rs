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

use std::sync::OnceLock;

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
const ALPHA: f32 = 1.5;
/// The quantity of nearest digit examples that vote.
const K: usize = 3;

const GRID: usize = 8;
const DIRS: usize = 8;
const PATH_POINTS: usize = 24;
const PATH_WEIGHT: f32 = 0.25;
/// The slants of the copies of the digit examples (x changes by this part of the height).
const SHEARS: [f32; 5] = [0.0, -0.12, 0.12, -0.25, 0.25];
const DIM: usize = GRID * GRID * DIRS + 2 * PATH_POINTS;

/// Reads the ink with the examples of all 60 writers.
pub fn recognize(strokes: &[Stroke]) -> Option<u8> {
    default().read(strokes)
}

/// Prepares the examples (approximately 10 000). The app calls this when it starts, so that the first
/// number does not wait for it.
pub fn warm_up() {
    default();
}

fn default() -> &'static Recognizer {
    static R: OnceLock<Recognizer> = OnceLock::new();
    R.get_or_init(|| Recognizer::new(samples().iter()))
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
            .collect()
    })
}

pub struct Recognizer {
    classes: Vec<u8>,
    features: Vec<f32>,
    /// Refer to ALPHA.
    pub alpha: f32,
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
        let mut classes = Vec::new();
        let mut features = Vec::new();
        for s in samples {
            // Digits also get copies with a different slant, because the slant of handwriting is
            // different for each writer.
            let shears: &[f32] = if s.class == 0 { &[0.0] } else { &SHEARS };
            for &k in shears {
                let strokes: Vec<Stroke> = s.strokes.iter().map(|st| st.iter().map(|&(x, y)| (x + k * (255.0 - y), y)).collect()).collect();
                if let Some(f) = features_of(&strokes) {
                    classes.push(s.class);
                    features.extend(f);
                }
            }
        }
        Recognizer { classes, features, alpha: ALPHA }
    }

    pub fn read(&self, strokes: &[Stroke]) -> Option<u8> {
        let m = self.matches(strokes)?;
        (m.digit_distance <= self.alpha * m.other_distance).then_some(m.digit)
    }

    /// The nearest examples, or None if the ink is not one character (too small, two characters side
    /// by side, or a scribble).
    pub fn matches(&self, strokes: &[Stroke]) -> Option<Match> {
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
        let f = features_of(&strokes)?;

        let mut nearest: [(f32, u8); K] = [(f32::MAX, 0); K];
        let mut other = f32::MAX;
        for (class, t) in self.classes.iter().zip(self.features.chunks_exact(DIM)) {
            let limit = if *class == 0 { other } else { nearest[K - 1].0 };
            let Some(d) = distance(&f, t, limit) else { continue };
            if *class == 0 {
                other = d;
            } else {
                let mut j = K - 1;
                nearest[j] = (d, *class);
                while j > 0 && nearest[j].0 < nearest[j - 1].0 {
                    nearest.swap(j, j - 1);
                    j -= 1;
                }
            }
        }
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

/// The squared distance, or None if it is more than `limit` (the search stops early).
fn distance(a: &[f32], b: &[f32], limit: f32) -> Option<f32> {
    let mut sum = 0.0;
    for (ca, cb) in a.chunks(64).zip(b.chunks(64)) {
        sum += ca.iter().zip(cb).map(|(x, y)| (x - y) * (x - y)).sum::<f32>();
        if sum > limit {
            return None;
        }
    }
    Some(sum)
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

/// The strokes in a unit box (the same scale for x and y), with the center of the ink at the center.
fn normalize(strokes: &[Stroke]) -> Option<Vec<Stroke>> {
    let (x0, y0, x1, y1) = bbox(strokes.iter().flatten());
    let size = (x1 - x0).max(y1 - y0);
    if size.is_nan() || size <= 0.0 {
        return None;
    }
    let (ox, oy) = (0.5 - (x1 - x0) / size / 2.0, 0.5 - (y1 - y0) / size / 2.0);
    Some(strokes.iter().map(|s| s.iter().map(|&(x, y)| ((x - x0) / size + ox, (y - y0) / size + oy)).collect()).collect())
}

/// The feature vector: the pen directions in a grid, then the pen path.
fn features_of(strokes: &[Stroke]) -> Option<Vec<f32>> {
    let ns = normalize(strokes)?;
    let mut f = directions(&ns);
    f.extend(pen_path(&ns).into_iter().map(|v| v * PATH_WEIGHT));
    debug_assert_eq!(f.len(), DIM);
    Some(f)
}

/// The length of the pen movement in each grid cell and each of 8 directions (with the sense of the
/// movement), with a small blur. The result has a length of 1.
fn directions(strokes: &[Stroke]) -> Vec<f32> {
    use std::f32::consts::TAU;
    let (g, d) = (GRID, DIRS);
    let mut f = vec![0f32; g * g * d];
    for s in strokes {
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
    let mut out = vec![0f32; f.len()];
    for y in 0..g {
        for x in 0..g {
            for c in 0..d {
                let mut s = 0.0;
                for dy in -1i32..=1 {
                    for dx in -1i32..=1 {
                        let (xx, yy) = (x as i32 + dx, y as i32 + dy);
                        if xx < 0 || yy < 0 || xx >= g as i32 || yy >= g as i32 {
                            continue;
                        }
                        let w = if dx == 0 && dy == 0 { 4.0 } else if dx == 0 || dy == 0 { 2.0 } else { 1.0 };
                        s += w * f[(yy as usize * g + xx as usize) * d + c];
                    }
                }
                out[(y * g + x) * d + c] = s;
            }
        }
    }
    let n = out.iter().map(|v| v * v).sum::<f32>().sqrt().max(1e-6);
    out.iter_mut().for_each(|v| *v /= n);
    out
}

/// The strokes joined in the order of writing, at PATH_POINTS equal distances: x0, y0, x1, y1, ...
fn pen_path(strokes: &[Stroke]) -> Vec<f32> {
    let pts: Vec<Point> = strokes.iter().flatten().copied().collect();
    let total: f32 = path_length(&pts);
    let mut out = Vec::with_capacity(2 * PATH_POINTS);
    if total <= 0.0 {
        for _ in 0..PATH_POINTS {
            out.extend([pts[0].0, pts[0].1]);
        }
        return out;
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
            out.extend([pts[i].0 + t * (pts[i + 1].0 - pts[i].0), pts[i].1 + t * (pts[i + 1].1 - pts[i].1)]);
        } else {
            let p = pts[pts.len() - 1];
            out.extend([p.0, p.1]);
        }
    }
    out
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
        let r = Recognizer::new(samples().iter().filter(|s| s.writer <= 40));
        (r, samples().iter().filter(|s| s.writer > 40).collect())
    }

    /// Puts an example on a cell of the board, with the size of handwriting (approximately 100 px).
    fn place(s: &Sample) -> Vec<Stroke> {
        s.strokes.iter().map(|st| st.iter().map(|&(x, y)| (300.0 + x * 0.4, 600.0 + y * 0.4)).collect()).collect()
    }

    #[test]
    fn writer_independent_accuracy() {
        let (r, test) = split();
        let (mut n, mut ok, mut wrong) = (0, 0, 0);
        let (mut on, mut rejected) = (0, 0);
        for s in &test {
            let got = r.read(&place(s));
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
        let (acc, wrong_rate, rej) = (ok as f64 / n as f64, wrong as f64 / n as f64, rejected as f64 / on as f64);
        eprintln!(
            "new writers: {:.1}% of digits correct, {:.1}% a wrong digit; {:.1}% of other characters rejected",
            acc * 100.0,
            wrong_rate * 100.0,
            rej * 100.0
        );
        // Measured: 96.7% correct, 0.8% a wrong digit, 90.3% of other characters rejected.
        assert!(acc >= 0.96, "digits read correctly: {:.1}%", acc * 100.0);
        assert!(wrong_rate <= 0.012, "digits read as a different digit: {:.1}%", wrong_rate * 100.0);
        assert!(rej >= 0.89, "other characters rejected: {:.1}%", rej * 100.0);
    }

    #[test]
    #[ignore]
    fn tune_alpha() {
        let (mut r, test) = split();
        for alpha in [1.2f32, 1.3, 1.4, 1.5, 1.6, 1.8] {
            r.alpha = alpha;
            let (mut n, mut ok, mut wrong, mut on, mut rej) = (0, 0, 0, 0, 0);
            for s in &test {
                let got = r.read(&place(s));
                if s.class != 0 {
                    n += 1;
                    if got == Some(s.class) { ok += 1 } else if got.is_some() { wrong += 1 }
                } else {
                    on += 1;
                    if got.is_none() { rej += 1 }
                }
            }
            println!("alpha {alpha}: {:.1}% correct, {:.1}% wrong digit, {:.1}% others rejected", 100.0 * ok as f64 / n as f64, 100.0 * wrong as f64 / n as f64, 100.0 * rej as f64 / on as f64);
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
