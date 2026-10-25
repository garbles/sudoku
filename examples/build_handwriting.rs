//! Makes `assets/handwriting.bin` from the UJI Pen Characters (Version 2) file:
//!
//!     curl -O https://archive.ics.uci.edu/ml/machine-learning-databases/uji-penchars/version2/ujipenchars2.txt
//!     cargo run --release --example build_handwriting -- ujipenchars2.txt
//!
//! Refer to assets/README.md for the format and the license of the data.

use std::io::Write;

/// Characters with the same shape as a digit (1). The recognizer does not use them as "not 1-9"
/// examples, because in a sudoku cell a vertical stroke is a 1. The dots of `:` and `;` are in a
/// vertical row: after the scale to the box, their pen path is the same as a vertical line. A `.` is
/// one small dot: after the scale to the box, it is the same as a short stroke.
const DIGIT_LIKE: &[&str] = &["l", "I", "|", "'", ",", "!", "¡", "i", "í", "Í", "j", ":", ";", "."];
/// The maximum quantity of points of one sample.
const POINTS: f32 = 40.0;

/// The label, the writer and the strokes of one example.
type Raw = (String, u8, Vec<Vec<(f32, f32)>>);

fn main() {
    let path = std::env::args().nth(1).expect("path of ujipenchars2.txt");
    let text = std::fs::read_to_string(path).expect("read");
    let mut samples: Vec<Raw> = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("WORD ") {
            let mut it = rest.split_whitespace();
            let label = it.next().unwrap().to_string();
            let id = it.next().unwrap();
            // trn_UJI_W01-01: writer 1.
            let w: u8 = id.split("_W").nth(1).unwrap()[..2].parse().unwrap();
            samples.push((label, w, Vec::new()));
        } else if let Some(rest) = t.strip_prefix("POINTS ") {
            let nums: Vec<f32> = rest.split('#').nth(1).unwrap().split_whitespace().map(|v| v.parse().unwrap()).collect();
            samples.last_mut().unwrap().2.push(nums.chunks(2).map(|p| (p[0], p[1])).collect());
        }
    }
    let mut out = Vec::new();
    let mut count = 0u16;
    for (label, writer, strokes) in &samples {
        if DIGIT_LIKE.contains(&label.as_str()) {
            continue;
        }
        let b = label.as_bytes();
        let class = if b.len() == 1 && (b'1'..=b'9').contains(&b[0]) { b[0] - b'0' } else { 0 };
        let all = strokes.iter().flatten();
        let (x0, y0, x1, y1) = all.fold((f32::MAX, f32::MAX, f32::MIN, f32::MIN), |(a, b, c, d), &(x, y)| (a.min(x), b.min(y), c.max(x), d.max(y)));
        let size = (x1 - x0).max(y1 - y0);
        if size < 1.0 || strokes.len() > 255 {
            continue;
        }
        let q = |x: f32, y: f32| (((x - x0) / size * 255.0).round() as u8, ((y - y0) / size * 255.0).round() as u8);
        let len = |s: &Vec<(f32, f32)>| s.windows(2).map(|w| ((w[1].0 - w[0].0).powi(2) + (w[1].1 - w[0].1).powi(2)).sqrt()).sum::<f32>();
        let total: f32 = strokes.iter().map(len).sum::<f32>().max(1e-3);
        out.extend([class, *writer, strokes.len() as u8]);
        for s in strokes {
            let n = ((POINTS * len(s) / total).round() as usize).clamp(1, 255).min(s.len().max(1));
            let pts = resample(s, n.max(if s.len() > 1 { 2 } else { 1 }));
            out.push(pts.len() as u8);
            for (x, y) in pts {
                let (a, b) = q(x, y);
                out.extend([a, b]);
            }
        }
        count += 1;
    }
    let mut f = std::fs::File::create("assets/handwriting.bin").unwrap();
    f.write_all(b"HW01").unwrap();
    f.write_all(&count.to_le_bytes()).unwrap();
    f.write_all(&out).unwrap();
    println!("{count} samples, {} bytes", out.len() + 6);
}

/// n points at equal distances along a stroke.
fn resample(s: &[(f32, f32)], n: usize) -> Vec<(f32, f32)> {
    if n <= 1 || s.len() < 2 {
        return vec![s[0]];
    }
    let total: f32 = s.windows(2).map(|w| ((w[1].0 - w[0].0).powi(2) + (w[1].1 - w[0].1).powi(2)).sqrt()).sum();
    if total <= 0.0 {
        return vec![s[0]];
    }
    let mut out = vec![s[0]];
    let step = total / (n - 1) as f32;
    let (mut acc, mut target) = (0.0, step);
    for w in s.windows(2) {
        let d = ((w[1].0 - w[0].0).powi(2) + (w[1].1 - w[0].1).powi(2)).sqrt();
        while d > 0.0 && acc + d >= target && out.len() < n - 1 {
            let t = (target - acc) / d;
            out.push((w[0].0 + t * (w[1].0 - w[0].0), w[0].1 + t * (w[1].1 - w[0].1)));
            target += step;
        }
        acc += d;
    }
    out.push(*s.last().unwrap());
    out
}
