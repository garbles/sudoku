//! Renders the states of the reference images in `images/pro/` into a folder, and compares them with the
//! reference images. Use it on a computer:
//!
//!     cargo run --release --bin snapshot -- OUT_DIR
//!
//! It also writes `icons.png` (all icons of the app in one image) and compares it with
//! `images/pro/icons.png`. To write only the sheet to a file (Dockerfile.images makes `images/pro/icons.png`
//! with this command):
//!
//!     cargo run --release --bin snapshot -- --icons FILE
//!
//! `--device move` (before the other arguments) draws the states for the Paper Pro Move and compares
//! them with `images/move/` (`layout::Model::images`). The default device is the Paper Pro. The icon
//! sheet is only in `images/pro/`: the icons have the same size on each model.
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
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|a| a == "--device") {
        let name = args.get(1).map(String::as_str);
        let Some(model) = name.and_then(sudoku::layout::Model::from_option_name) else {
            let names: Vec<_> = sudoku::layout::Model::ALL.map(sudoku::layout::Model::option_name).into();
            panic!("--device {}, not {name:?}", names.join("|"));
        };
        sudoku::layout::set_device(model.device());
        args.drain(..2);
    }
    let fonts = Fonts::for_tools().expect("fonts");
    if args.first().is_some_and(|a| a == "--icons") {
        let file = args.get(1).expect("--icons FILE");
        save(&reference::icon_sheet(&fonts), Path::new(file));
        return;
    }
    let out = PathBuf::from(args.first().map_or("target/snapshots", String::as_str));
    std::fs::create_dir_all(&out).unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    reference::use_utc();

    let mut worst: f64 = 0.0;
    for (name, setup) in reference::states() {
        let pm = reference::render(&fonts, setup);
        save(&pm, &out.join(name));
        let reference = decode_png(&std::fs::read(reference::image_path(root, name)).unwrap());
        let (mad, big) = reference::compare(&pm, &reference);
        worst = worst.max(big);
        let off = reference::differences_off_edges(&pm, &reference, 1);
        println!("{name:28} mean diff {mad:5.2}  pixels > 64: {:.3}%  not near an edge: {} {:?}", big * 100.0, off.len(), off.iter().take(5).collect::<Vec<_>>());
    }
    save(&reference::render(&fonts, reference::ink_state), &out.join("ink.png"));
    let sheet = reference::icon_sheet(&fonts);
    save(&sheet, &out.join("icons.png"));
    if let Ok(png) = std::fs::read(reference::image_path(root, "icons.png")) {
        let (mad, big) = reference::compare(&sheet, &decode_png(&png));
        worst = worst.max(big);
        println!("{:28} mean diff {mad:5.2}  pixels > 64: {:.3}%", "icons.png", big * 100.0);
    }
    // The same states in landscape (no reference images).
    sudoku::layout::set_orientation(sudoku::layout::Orientation::Landscape);
    for (name, setup) in reference::states() {
        save(&reference::render(&fonts, setup), &out.join(format!("landscape-{name}")));
    }
    sudoku::layout::set_orientation(sudoku::layout::Orientation::Portrait);
    println!("worst: {:.3}%", worst * 100.0);
}
