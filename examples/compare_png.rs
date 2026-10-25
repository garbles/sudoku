//! Compares two PNG files of the screen: cargo run --example compare_png -- A.png B.png
fn main() {
    let a = sudoku::render::decode_png(&std::fs::read(std::env::args().nth(1).unwrap()).unwrap());
    let b = sudoku::render::decode_png(&std::fs::read(std::env::args().nth(2).unwrap()).unwrap());
    let (mean, big) = sudoku::reference::compare(&a, &b);
    let off2 = sudoku::reference::differences_off_edges(&a, &b, 2).len();
    let off1 = sudoku::reference::differences_off_edges(&a, &b, 1).len();
    println!("mean {mean:.2}, {:.3}% of pixels differ by more than 64, off edges: r2 {off2}, r1 {off1}", big * 100.0);
}
