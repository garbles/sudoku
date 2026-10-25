//! Reads straight lines of different lengths, slants and directions.
fn main() {
    for (len, dx, rev) in [(100.0, 0.0, false), (100.0, 0.0, true), (80.0, 0.0, false), (100.0, 8.0, false), (100.0, -8.0, true), (60.0, 0.0, false)] {
        let mut s: Vec<(f32, f32)> = (0..=40).map(|i| { let t = i as f32 / 40.0; (300.0 + dx * t, 600.0 + len * t) }).collect();
        if rev { s.reverse(); }
        println!("len {len} dx {dx} up {rev}: {:?} {:?}", sudoku::recognizer::recognize(&[s.clone()]), sudoku::recognizer::matches(&[s]));
    }
}
