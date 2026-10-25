//! Renders the states of the reference images in `images/` into a folder, and compares them with the
//! reference images. Use it on a computer:
//!
//!     cargo run --release --bin snapshot -- OUT_DIR
//!
//! It uses the fonts in scripts/fonts (refer to scripts/fonts/README.md). `SUDOKU_FONT` selects a
//! different font file (for example the variable font of the device).

use std::path::{Path, PathBuf};

use sudoku::font::Fonts;
use sudoku::reference;
use sudoku::render::decode_png;
use tiny_skia::Pixmap;

fn save(pm: &Pixmap, path: &Path) {
    // The screen is opaque, thus the premultiplied values are the same as the RGB values.
    let rgb: Vec<u8> = pm.pixels().iter().flat_map(|p| [p.red(), p.green(), p.blue()]).collect();
    let file = std::io::BufWriter::new(std::fs::File::create(path).unwrap());
    let mut enc = png::Encoder::new(file, pm.width(), pm.height());
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().unwrap().write_image_data(&rgb).unwrap();
}

fn main() {
    let out = PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "target/snapshots".into()));
    std::fs::create_dir_all(&out).unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let fonts = if std::env::var_os("SUDOKU_FONT").is_some() {
        Fonts::load().expect("fonts")
    } else {
        Fonts::from_files(&root.join("scripts/fonts/rmsans-400.ttf"), &root.join("scripts/fonts/rmsans-500.ttf")).expect("fonts")
    };
    reference::use_utc();

    let mut worst: f64 = 0.0;
    for (name, setup) in reference::states() {
        let pm = reference::render(&fonts, setup);
        save(&pm, &out.join(name));
        let reference = decode_png(&std::fs::read(root.join("images").join(name)).unwrap());
        let (mad, big) = reference::compare(&pm, &reference);
        worst = worst.max(big);
        let off = reference::differences_off_edges(&pm, &reference, 1);
        println!("{name:28} mean diff {mad:5.2}  pixels > 64: {:.3}%  not near an edge: {} {:?}", big * 100.0, off.len(), off.iter().take(5).collect::<Vec<_>>());
    }
    save(&reference::render(&fonts, reference::ink_state), &out.join("ink.png"));
    // The same states in landscape (no reference images).
    sudoku::layout::set_orientation(sudoku::layout::Orientation::Landscape);
    for (name, setup) in reference::states() {
        save(&reference::render(&fonts, setup), &out.join(format!("landscape-{name}")));
    }
    sudoku::layout::set_orientation(sudoku::layout::Orientation::Portrait);
    println!("worst: {:.3}%", worst * 100.0);
}
