//! The performance bench on a computer (refer to `src/bench.rs` and `scripts/bench.sh`):
//!
//!     cargo run --release --bin bench -- [OUT.json] [--script FILE] [--gen N] [--recognize-step N] [--draw-reps N]
//!
//! It uses the fonts in scripts/fonts. `SUDOKU_FONT` selects a different font file.

use sudoku::bench;
use sudoku::font::Fonts;

fn main() {
    let result = bench::main(std::env::args().skip(1).collect(), Fonts::for_tools);
    if let Err(e) = result {
        eprintln!("bench: {e}");
        std::process::exit(1);
    }
}
