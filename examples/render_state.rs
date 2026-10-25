//! Draws a saved game and settings with the menu and a panel open, to compare with a screenshot of
//! the device: cargo run --example render_state -- game.json settings.json colors|newgame|none OUT.png
use std::path::Path;
use sudoku::app::{App, Env, Panel};
fn main() {
    struct E;
    impl Env for E { fn unix_time(&mut self) -> i64 { 0 } fn new_seed(&mut self) -> u64 { 1 } }
    let a: Vec<String> = std::env::args().collect();
    let game = serde_json::from_slice(&std::fs::read(&a[1]).unwrap()).unwrap();
    let settings = serde_json::from_slice(&std::fs::read(&a[2]).unwrap()).unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let fonts = sudoku::font::Fonts::from_files(&root.join("scripts/fonts/rmsans-400.ttf"), &root.join("scripts/fonts/rmsans-500.ttf")).unwrap();
    let mut app = App::new(Some(game), Some(settings), sudoku::store::ResultsDb::in_memory(), Box::new(E));
    app.ng_stops = sudoku::render::ng_stops(&fonts);
    app.ui.menu_open = a[3] != "closed";
    app.ui.panel = match a[3].as_str() { "colors" => Panel::Colors, "newgame" => Panel::NewGame, _ => Panel::None };
    let mut pm = tiny_skia::Pixmap::new(1620, 2160).unwrap();
    sudoku::render::draw(&mut sudoku::render::Canvas::new(&mut pm, &fonts), &app);
    let rgb: Vec<u8> = pm.pixels().iter().flat_map(|p| [p.red(), p.green(), p.blue()]).collect();
    let mut enc = png::Encoder::new(std::io::BufWriter::new(std::fs::File::create(&a[4]).unwrap()), 1620, 2160);
    enc.set_color(png::ColorType::Rgb);
    enc.write_header().unwrap().write_image_data(&rgb).unwrap();
}
