//! Draws the analytics summary or the games list from a results database, to compare with a
//! screenshot of the device:
//!     SUDOKU_ROTATION=2 cargo run --example render_analytics -- results.sqlite summary|list OUT.png
use std::path::Path;
use sudoku::app::{App, Env, Screen};
fn main() {
    struct E;
    impl Env for E { fn unix_time(&mut self) -> i64 { 0 } fn new_seed(&mut self) -> u64 { 1 } }
    let a: Vec<String> = std::env::args().collect();
    if let Some(r) = std::env::var("SUDOKU_ROTATION").ok().and_then(|v| v.parse().ok()) {
        sudoku::layout::set_rotation(r);
    }
    sudoku::reference::use_utc();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let fonts = sudoku::font::Fonts::from_files(&root.join("scripts/fonts/rmsans-400.ttf"), &root.join("scripts/fonts/rmsans-500.ttf")).unwrap();
    let db = sudoku::store::ResultsDb::open(Path::new(&a[1])).unwrap();
    let mut app = App::new(None, None, db, Box::new(E));
    app.screen = if a[2] == "list" { Screen::GamesList } else { Screen::Analytics };
    let mut pm = tiny_skia::Pixmap::new(sudoku::layout::screen_w(), sudoku::layout::screen_h()).unwrap();
    sudoku::render::draw(&mut sudoku::render::Canvas::new(&mut pm, &fonts), &app);
    let rgb: Vec<u8> = pm.pixels().iter().flat_map(|p| [p.red(), p.green(), p.blue()]).collect();
    let mut enc = png::Encoder::new(std::io::BufWriter::new(std::fs::File::create(&a[3]).unwrap()), pm.width(), pm.height());
    enc.set_color(png::ColorType::Rgb);
    enc.write_header().unwrap().write_image_data(&rgb).unwrap();
}
