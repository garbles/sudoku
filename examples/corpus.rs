//! The computer part of the corpus procedure (src/corpus.rs, the skill `remarkable-corpus`).
//!
//!     cargo run --release --example corpus -- sheet LOG OUT.png DIGIT
//!     cargo run --release --example corpus -- add LOG DIGIT [--exclude 3,7,12]
//!     cargo run --release --example corpus -- count
//!
//! - The corpus log (`corpus-ink.jsonl` of the tablet) has one line for each pen stroke and each
//!   erasure, with its cell (src/corpus.rs). An example is all strokes of one cell, in the order of
//!   the log, until the eraser touches that cell. The examples are in the order of their first
//!   stroke.
//! - `sheet` draws each example in a numbered tile: the number of the example (from 1), the cell,
//!   the quantity of strokes and the result of the recognizer. A tile with a red frame has a result
//!   that is not DIGIT.
//! - `add` adds the examples of the log, except the excluded numbers, to
//!   assets/owner-handwriting.jsonl with the class DIGIT. It scales each example to the box 0..255.
//! - `count` gives the quantity of examples for each digit in assets/owner-handwriting.jsonl.

use std::io::Write;
use std::path::Path;

use sudoku::font::{Anchor, Fonts, Weight};
use sudoku::recognizer::{self, Stroke};
use tiny_skia::{Color, LineCap, LineJoin, Paint, PathBuilder, Pixmap, Rect, Stroke as Pen, Transform};

const CORPUS: &str = "assets/owner-handwriting.jsonl";
const COLUMNS: usize = 6;
const TILE_W: f32 = 240.0;
const TILE_H: f32 = 280.0;
const INK_BOX: f32 = 180.0;

struct Example {
    cell: (u8, u8),
    strokes: Vec<Stroke>,
}

fn read_log(path: &str) -> Vec<Example> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let mut examples: Vec<Example> = Vec::new();
    // The example that is open in each cell (an index in `examples`).
    let mut open: std::collections::HashMap<(u8, u8), usize> = std::collections::HashMap::new();
    for (i, l) in text.lines().enumerate().filter(|(_, l)| !l.trim().is_empty()) {
        let v: serde_json::Value = serde_json::from_str(l).unwrap_or_else(|e| panic!("{path} line {}: {e}", i + 1));
        let cell: (u8, u8) = serde_json::from_value(v["cell"].clone()).unwrap_or_else(|e| panic!("{path} line {}: cell: {e}", i + 1));
        if v["erase"] == serde_json::Value::Bool(true) {
            open.remove(&cell);
            continue;
        }
        let stroke: Stroke =
            serde_json::from_value(v["stroke"].clone()).unwrap_or_else(|e| panic!("{path} line {}: stroke: {e}", i + 1));
        match open.get(&cell) {
            Some(&k) => examples[k].strokes.push(stroke),
            None => {
                open.insert(cell, examples.len());
                examples.push(Example { cell, strokes: vec![stroke] });
            }
        }
    }
    examples
}

fn bbox(strokes: &[Stroke]) -> Option<(f32, f32, f32, f32)> {
    let mut it = strokes.iter().flatten();
    let &(x, y) = it.next()?;
    Some(it.fold((x, y, x, y), |(x0, y0, x1, y1), &(x, y)| (x0.min(x), y0.min(y), x1.max(x), y1.max(y))))
}

fn digit_arg(s: &str) -> u8 {
    match s.parse() {
        Ok(d @ 1..=9) => d,
        _ => panic!("DIGIT must be 1 to 9, not {s}"),
    }
}

fn sheet(log: &str, out: &str, digit: u8) {
    let examples = read_log(log);
    let fonts = Fonts::for_tools().expect("fonts (refer to scripts/fonts/README.md)");
    let (font, bold) = (fonts.get(Weight::Regular), fonts.get(Weight::Medium));
    let rows = examples.len().div_ceil(COLUMNS).max(1);
    let mut pm = Pixmap::new((COLUMNS as f32 * TILE_W) as u32, (rows as f32 * TILE_H) as u32).unwrap();
    pm.fill(Color::WHITE);
    let id = Transform::identity();
    let mut wrong = Vec::new();
    for (i, ex) in examples.iter().enumerate() {
        let (tx, ty) = ((i % COLUMNS) as f32 * TILE_W, (i / COLUMNS) as f32 * TILE_H);
        let result = recognizer::recognize(&ex.strokes);
        let ok = result == Some(digit);
        if !ok {
            wrong.push(i + 1);
        }
        // The frame: gray, or red if the recognizer does not read DIGIT.
        let mut paint = Paint::default();
        paint.set_color_rgba8(if ok { 190 } else { 220 }, if ok { 190 } else { 30 }, if ok { 190 } else { 30 }, 255);
        let frame = PathBuilder::from_rect(Rect::from_xywh(tx + 4.0, ty + 4.0, TILE_W - 8.0, TILE_H - 8.0).unwrap());
        pm.stroke_path(&frame, &paint, &Pen { width: if ok { 2.0 } else { 6.0 }, ..Pen::default() }, id, None);
        // The labels: the number of the example, the cell and the result.
        bold.draw(&mut pm, 40.0, tx + 16.0, ty + 48.0, Anchor::Start, &format!("{}", i + 1), [0, 0, 0], id);
        font.draw(&mut pm, 22.0, tx + TILE_W - 16.0, ty + 34.0, Anchor::End, &format!("r{}c{} ({}s)", ex.cell.0 + 1, ex.cell.1 + 1, ex.strokes.len()), [90, 90, 90], id);
        let label = result.map_or("read: ?".to_string(), |d| format!("read: {d}"));
        let color = if ok { [0, 120, 0] } else { [200, 0, 0] };
        font.draw(&mut pm, 22.0, tx + TILE_W - 16.0, ty + 62.0, Anchor::End, &label, color, id);
        // The ink, scaled to INK_BOX (the same scale for x and y), centered in the low part of the tile.
        let Some((x0, y0, x1, y1)) = bbox(&ex.strokes) else { continue };
        let k = INK_BOX / (x1 - x0).max(y1 - y0).max(1.0);
        let (ox, oy) = (tx + (TILE_W - (x1 - x0) * k) / 2.0, ty + 80.0 + (INK_BOX - (y1 - y0) * k) / 2.0);
        let mut pb = PathBuilder::new();
        for s in &ex.strokes {
            let mut pts = s.iter().map(|&(x, y)| (ox + (x - x0) * k, oy + (y - y0) * k));
            let Some((x, y)) = pts.next() else { continue };
            pb.move_to(x, y);
            pb.line_to(x + 0.01, y);
            for (x, y) in pts {
                pb.line_to(x, y);
            }
        }
        if let Some(path) = pb.finish() {
            let mut ink = Paint::default();
            ink.set_color_rgba8(0, 0, 0, 255);
            ink.anti_alias = true;
            let pen = Pen { width: 4.0, line_cap: LineCap::Round, line_join: LineJoin::Round, ..Pen::default() };
            pm.stroke_path(&path, &ink, &pen, id, None);
        }
    }
    let rgb: Vec<u8> = pm.pixels().iter().flat_map(|p| [p.red(), p.green(), p.blue()]).collect();
    let file = std::io::BufWriter::new(std::fs::File::create(out).unwrap_or_else(|e| panic!("{out}: {e}")));
    let mut enc = png::Encoder::new(file, pm.width(), pm.height());
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().unwrap().write_image_data(&rgb).unwrap();
    println!("{out}: {} examples", examples.len());
    if wrong.is_empty() {
        println!("The recognizer reads {digit} in all examples.");
    } else {
        println!("The recognizer does not read {digit} in the examples {wrong:?} (red frame).");
    }
}

fn add(log: &str, digit: u8, exclude: &[usize]) {
    let examples = read_log(log);
    if let Some(n) = exclude.iter().find(|&&n| n == 0 || n > examples.len()) {
        panic!("--exclude {n}: the log has the examples 1 to {}", examples.len());
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(root.join(CORPUS)).expect(CORPUS);
    let mut added = 0;
    for (i, ex) in examples.iter().enumerate() {
        if exclude.contains(&(i + 1)) {
            continue;
        }
        let strokes: Vec<Stroke> = ex.strokes.iter().filter(|s| !s.is_empty()).cloned().collect();
        let Some((x0, y0, x1, y1)) = bbox(&strokes) else {
            println!("Example {} has no ink: not added.", i + 1);
            continue;
        };
        let size = (x1 - x0).max(y1 - y0);
        if size <= 0.0 {
            println!("Example {} is one point: not added.", i + 1);
            continue;
        }
        // The same box as assets/handwriting.bin: the larger side is 0..255. One decimal is sufficient.
        let k = 255.0 / size;
        let r = |v: f32| ((v as f64 * 10.0).round() / 10.0).clamp(0.0, 255.0);
        let strokes: Vec<Vec<[f64; 2]>> =
            strokes.iter().map(|s| s.iter().map(|&(x, y)| [r((x - x0) * k), r((y - y0) * k)]).collect()).collect();
        writeln!(f, "{}", serde_json::json!({ "class": digit, "strokes": strokes })).unwrap();
        added += 1;
    }
    println!("{added} examples of {digit} added to {CORPUS} ({} excluded).", exclude.len());
}

fn count() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(root.join(CORPUS)).unwrap_or_default();
    let mut n = [0usize; 10];
    for l in text.lines().filter(|l| !l.trim().is_empty()) {
        let v: serde_json::Value = serde_json::from_str(l).expect(CORPUS);
        n[v["class"].as_u64().expect("class") as usize] += 1;
    }
    for (d, c) in n.iter().enumerate().skip(1) {
        println!("{d}: {c}");
    }
    println!("all: {}", n.iter().sum::<usize>());
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let a: Vec<&str> = args.iter().map(String::as_str).collect();
    match a.as_slice() {
        ["sheet", log, out, d] => sheet(log, out, digit_arg(d)),
        ["add", log, d] => add(log, digit_arg(d), &[]),
        ["add", log, d, "--exclude", list] => {
            let exclude: Vec<usize> =
                list.split(',').filter(|s| !s.trim().is_empty()).map(|s| s.trim().parse().expect("--exclude: numbers with commas")).collect();
            add(log, digit_arg(d), &exclude)
        }
        ["count"] => count(),
        _ => {
            eprintln!("usage: corpus sheet LOG OUT.png DIGIT | corpus add LOG DIGIT [--exclude 3,7,12] | corpus count");
            std::process::exit(2);
        }
    }
}
