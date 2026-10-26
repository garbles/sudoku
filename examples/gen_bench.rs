// Measures the time to generate each mode and the distribution of the starting numbers.
use std::time::Instant;
use sudoku::generator::{generate, MODES, MODE_LEVELS};
use sudoku::solver::{grade, Level};
fn main() {
    let n: u64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(20);
    for mode in 0..MODES {
        let mut times = Vec::new();
        let mut givens = Vec::new();
        for seed in 0..n {
            let t = Instant::now();
            let p = generate(seed * 7919 + 13, mode);
            times.push(t.elapsed().as_secs_f64());
            givens.push(p.givens_count());
            assert_eq!(grade(&p.givens, Level::Chains), Some(MODE_LEVELS[mode as usize]), "seed {seed} mode {mode}");
        }
        times.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let mean = times.iter().sum::<f64>() / n as f64;
        let g = givens.iter().sum::<usize>() as f64 / n as f64;
        println!("mode {mode}: mean {:.3}s median {:.3}s max {:.3}s, givens mean {g:.1} min {} max {}", mean, times[times.len() / 2], times[times.len() - 1], givens.iter().min().unwrap(), givens.iter().max().unwrap());
    }
}
