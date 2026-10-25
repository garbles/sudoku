fn main() {
    for mode in 0..5u8 {
        let t = std::time::Instant::now();
        let counts: Vec<usize> = (0..40u64).map(|s| sudoku::generator::generate(s * 7919, mode).givens_count()).collect();
        println!("mode {mode}: min {} max {} avg {:.1} ({:?}/puzzle)", counts.iter().min().unwrap(), counts.iter().max().unwrap(),
            counts.iter().sum::<usize>() as f64 / 40.0, t.elapsed() / 40);
    }
}
