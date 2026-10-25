//! Measures the time to prepare the recognizer and to read one number.
fn main() {
    let t = std::time::Instant::now();
    sudoku::recognizer::warm_up();
    println!("warm up: {:?}", t.elapsed());
    let ink = sudoku::recognizer::example_ink(7, 50, 300.0, 600.0, 98.0).unwrap();
    let t = std::time::Instant::now();
    for _ in 0..20 {
        assert_eq!(sudoku::recognizer::recognize(&ink), Some(7));
    }
    println!("one reading: {:?}", t.elapsed() / 20);
}
