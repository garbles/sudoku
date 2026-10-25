//! Loads a font file (ttf, woff or woff2) and prints the widths of some UI texts.
//!     cargo run --example font_check -- FONT [WEIGHT]
fn main() {
    let path = std::env::args().nth(1).expect("font path");
    let weight: f32 = std::env::args().nth(2).map(|w| w.parse().unwrap()).unwrap_or(400.0);
    let f = sudoku::font::Font::load(std::path::Path::new(&path), weight).expect("font");
    for t in ["Easy", "Insane", "Start new game", "123456789"] {
        println!("{t}: {:.2}", f.width(25.0, t));
    }
}
