//! The performance bench on a computer (refer to `src/bench.rs` and `scripts/bench.sh`):
//!
//!     cargo run --release --bin bench -- [OUT.json] [--script FILE] [--gen N] [--recognize-step N] [--draw-reps N]
//!
//! It uses the fonts in scripts/fonts. `SUDOKU_FONT` selects a different font file.

use std::path::Path;

use sudoku::bench;
use sudoku::font::Fonts;

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let result = bench::main(std::env::args().skip(1).collect(), || {
        if std::env::var_os("SUDOKU_FONT").is_some() {
            Fonts::load()
        } else {
            Fonts::from_files(&root.join("scripts/fonts/rmsans-400.ttf"), &root.join("scripts/fonts/rmsans-500.ttf"))
        }
    });
    if let Err(e) = result {
        eprintln!("bench: {e}");
        std::process::exit(1);
    }
}
