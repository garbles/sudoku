//! The app draws the same pixels as the reference images in `images/` (REQUIREMENTS.md §15). The
//! renderer (tiny-skia) and ImageMagick (FreeType) use different antialiasing. Thus a difference is
//! permitted only on the pixels at the edge of a line or a glyph (within 1 px; 2 px for a maximum of
//! two pixels).

mod common;

use std::path::Path;

use common::fonts;
use sudoku::game::cells;
use sudoku::reference;
use sudoku::render::{decode_png, rgb_at};
use sudoku::settings::Color;

#[test]
fn screens_agree_with_the_reference_images() {
    reference::use_utc();
    let fonts = fonts();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for (name, setup) in reference::states() {
        let pm = reference::render(&fonts, setup);
        let want = decode_png(&std::fs::read(root.join("images").join(name)).unwrap());
        let (mean, big) = reference::compare(&pm, &want);
        assert!(mean < 1.5 && big < 0.008, "{name}: mean difference {mean:.2}, {:.3}% of pixels differ", big * 100.0);
        // Each pixel that differs is at the edge of an item: no item is missing, extra or moved.
        let off2 = reference::differences_off_edges(&pm, &want, 2);
        assert!(off2.is_empty(), "{name}: pixels that differ away from all edges: {:?}", &off2[..off2.len().min(10)]);
        let off1 = reference::differences_off_edges(&pm, &want, 1);
        assert!(off1.len() <= 2, "{name}: pixels that differ more than 1 px from an edge: {:?}", &off1[..off1.len().min(10)]);
    }
}

#[test]
fn cell_colors_follow_the_settings() {
    // BRD-7, COL-3: the board shows the selected colors immediately.
    let fonts = fonts();
    let pm = reference::render(&fonts, |a| {
        a.game = reference::sample_game();
        a.settings.color_starting = Color::Yellow;
        a.settings.color_clash = Color::Blue;
        a.settings.color_highlight = Color::Orange;
        a.ui.highlight = Some(5);
    });
    let g = reference::sample_game();
    for cell in cells() {
        let (x, y) = sudoku::layout::cell_origin(cell);
        // A point in the cell without lines, digits or marks.
        let px = rgb_at(&pm, x + 8, y + 140);
        let v = g.value_at(cell);
        let want = if v == 0 {
            Color::White
        } else if g.is_clashing(cell) {
            Color::Blue
        } else if v == 5 {
            Color::Orange
        } else if g.is_given(cell) {
            Color::Yellow
        } else {
            Color::White
        };
        assert_eq!(px, want.rgb(), "cell {cell:?}");
    }
}

#[test]
fn ink_shows_while_it_waits() {
    // INP-10: ink shows only while it waits; then the board has only typed numbers.
    let fonts = fonts();
    let with = reference::render(&fonts, reference::ink_state);
    let without = reference::render(&fonts, |a| a.game = reference::sample_game());
    assert_eq!(rgb_at(&with, 330, 1060), [0, 0, 0]);
    assert_ne!(rgb_at(&without, 330, 1060), [0, 0, 0]);
}

#[test]
fn ink_drawn_in_parts_is_the_same_as_ink_drawn_at_once() {
    // The platform draws the live stroke in parts while the pen moves (platform/mod.rs).
    let fonts = fonts();
    let stroke: Vec<(f32, f32)> = (0..40).map(|i| (320.0 + (i as f32 * 0.3).sin() * 30.0, 980.0 + i as f32 * 3.0)).collect();
    let full = reference::render(&fonts, |a| {
        a.game = reference::sample_game();
        a.live_stroke = Some((0..40).map(|i| (320.0 + (i as f32 * 0.3).sin() * 30.0, 980.0 + i as f32 * 3.0)).collect());
    });
    let mut parts = reference::render(&fonts, |a| {
        a.game = reference::sample_game();
        a.live_stroke = Some((0..2).map(|i| (320.0 + (i as f32 * 0.3).sin() * 30.0, 980.0 + i as f32 * 3.0)).collect());
    });
    let mut drawn = 2;
    for end in [7, 15, 16, 30, 40] {
        sudoku::render::Canvas::new(&mut parts, &fonts).polyline(&stroke[drawn - 1..end], sudoku::render::INK_WIDTH, sudoku::render::BLACK, true);
        drawn = end;
    }
    let (mean, big) = reference::compare(&parts, &full);
    assert!(big < 0.00002 && mean < 0.01, "mean {mean}, {big}");
}

#[test]
fn landscape_manual_pages_are_the_landscape_images() {
    // The manual in landscape shows the images of scripts/manual-*-landscape.sh.
    let fonts = fonts();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    sudoku::layout::set_orientation(sudoku::layout::Orientation::Landscape);
    let names = ["how-to-play", "candidates", "pen", "pen-rules", "helpers", "mistakes", "menu", "new-game", "finished"];
    for (i, name) in names.iter().enumerate() {
        let mut app = reference::app(&fonts);
        app.screen = sudoku::app::Screen::Manual;
        app.manual_page = i as u8 + 1;
        let mut pm = tiny_skia::Pixmap::new(2160, 1620).unwrap();
        sudoku::render::draw(&mut sudoku::render::Canvas::new(&mut pm, &fonts), &app);
        let want = decode_png(&std::fs::read(root.join(format!("images/manual-{name}-landscape.png"))).unwrap());
        assert_eq!(reference::compare(&pm, &want).1, 0.0, "page {}", i + 1);
    }
    sudoku::layout::set_orientation(sudoku::layout::Orientation::Portrait);
}

#[test]
fn landscape_analytics_agrees_with_the_reference_images() {
    // scripts/analytics.py makes the landscape screens with the positions of src/layout.rs.
    reference::use_utc();
    let fonts = fonts();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    sudoku::layout::set_orientation(sudoku::layout::Orientation::Landscape);
    for (name, setup) in reference::states().into_iter().filter(|(n, _)| n.starts_with("analytics")) {
        let pm = reference::render(&fonts, setup);
        let file = name.replace(".png", "-landscape.png");
        let want = decode_png(&std::fs::read(root.join("images").join(&file)).unwrap());
        let (mean, big) = reference::compare(&pm, &want);
        assert!(mean < 1.5 && big < 0.008, "{file}: mean difference {mean:.2}, {:.3}% of pixels differ", big * 100.0);
        let off2 = reference::differences_off_edges(&pm, &want, 2);
        assert!(off2.is_empty(), "{file}: pixels that differ away from all edges: {:?}", &off2[..off2.len().min(10)]);
    }
    sudoku::layout::set_orientation(sudoku::layout::Orientation::Portrait);
}

#[test]
fn a_retained_frame_is_the_same_as_a_full_drawing() {
    // render::FrameRecord: with a record, a drawing of the game screen writes only the areas that can
    // change. Each reference state after each other state (and after ink that goes away) must give
    // the same pixels as a full drawing.
    use sudoku::render::{draw, Canvas, FrameRecord};
    use tiny_skia::Pixmap;
    reference::use_utc();
    let fonts = fonts();
    let ink: (&str, reference::Setup) = ("ink", reference::ink_state);
    let no_ink: (&str, reference::Setup) = ("no ink", |a| a.game = reference::sample_game());
    let states = reference::states();
    let mut order = vec![ink, no_ink, ink];
    for s in states.iter().chain(states.iter().rev()) {
        order.extend([*s, ink, *s, no_ink]);
    }
    let mut pm = Pixmap::new(1, 1).unwrap();
    let mut record = FrameRecord::default();
    for (name, setup) in order {
        sudoku::layout::set_right_handed(false);
        let mut a = reference::app(&fonts);
        setup(&mut a);
        let (w, h) = (sudoku::layout::screen_w(), sudoku::layout::screen_h());
        if (pm.width(), pm.height()) != (w, h) {
            // A new size (the presenter after a rotation): a new frame and a new record.
            pm = Pixmap::new(w, h).unwrap();
            record = FrameRecord::default();
        }
        draw(&mut Canvas::retained(&mut pm, &mut record, &fonts), &a);
        let mut full = Pixmap::new(w, h).unwrap();
        draw(&mut Canvas::new(&mut full, &fonts), &a);
        assert!(pm.data() == full.data(), "{name}: the retained frame is not the same as a full drawing");
    }
    sudoku::layout::set_right_handed(false);
}

#[test]
fn a_retained_frame_follows_each_change_of_the_cached_overlays() {
    // render.rs keeps the pixels of the meter, the menu button and the toolbar (put_overlay). A
    // random sequence of states changes each input of these pixels (the menu, the panel, undo and
    // redo, the caution, the mode, the hand) and puts ink over them. Each retained frame must be the
    // same as a full drawing.
    use sudoku::app::{Menu, Panel};
    use sudoku::render::{draw, Canvas, FrameRecord};
    use tiny_skia::Pixmap;
    let fonts = fonts();
    let mut pm = Pixmap::new(1, 1).unwrap();
    let mut record = FrameRecord::default();
    let mut seed = 12345u64;
    let mut next = |n: u64| {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (seed >> 33) % n
    };
    for step in 0..400 {
        let right = next(4) == 0;
        sudoku::layout::set_right_handed(right);
        let mut a = reference::app(&fonts);
        a.game = reference::sample_game();
        // No undo (can redo: no), a few undos (both), or all (can undo: no).
        let undos = [0, 3, usize::MAX][next(3) as usize];
        for _ in 0..undos {
            if !a.game.can_undo() {
                break;
            }
            a.game.undo();
        }
        a.game.mode = next(7) as u8;
        a.ui.menu = match next(5) {
            0 => Menu::Closed,
            1 => Menu::Open(None),
            2 => Menu::Open(Some(Panel::NewGame)),
            3 => Menu::Open(Some(Panel::Colors)),
            _ => Menu::Open(Some(Panel::Settings)),
        };
        if next(4) == 0 {
            a.update = reference::update_state(true);
        }
        let (w, h) = (sudoku::layout::screen_w() as f32, sudoku::layout::screen_h() as f32);
        let tb = sudoku::layout::tb_x() as f32 + 50.0;
        let my = sudoku::layout::meter_y() as f32;
        a.live_stroke = match next(4) {
            // Over the toolbar, over the meter, or from the toolbar to the meter.
            0 => Some((0..20).map(|i| (tb + i as f32, 40.0 + i as f32 * 30.0)).collect()),
            1 => Some((0..20).map(|i| (w / 2.0 + i as f32 * 10.0, my - 5.0 + (i % 3) as f32 * 5.0)).collect()),
            2 => Some(vec![(tb, h / 3.0), (w / 2.0, my)]),
            _ => None,
        };
        let (w, h) = (sudoku::layout::screen_w(), sudoku::layout::screen_h());
        if (pm.width(), pm.height()) != (w, h) {
            pm = Pixmap::new(w, h).unwrap();
            record = FrameRecord::default();
        }
        draw(&mut Canvas::retained(&mut pm, &mut record, &fonts), &a);
        let mut full = Pixmap::new(w, h).unwrap();
        draw(&mut Canvas::new(&mut full, &fonts), &a);
        assert!(pm.data() == full.data(), "step {step}: the retained frame is not the same as a full drawing");
    }
    sudoku::layout::set_right_handed(false);
}

#[test]
fn the_cached_items_of_the_colors_panel_are_the_same_as_a_new_drawing() {
    // The Cell colors panel keeps the pixels of each item. After a change of a color, most items
    // come from the cache. They must be the same as a drawing with no cache (a new thread).
    let fonts = fonts();
    fn colors_panel(a: &mut sudoku::app::App) {
        a.ui.menu = sudoku::app::Menu::Open(Some(sudoku::app::Panel::Colors));
    }
    fn orange(a: &mut sudoku::app::App) {
        colors_panel(a);
        a.settings.color_clash = Color::Orange;
    }
    let first = reference::render(&fonts, colors_panel);
    let cached = reference::render(&fonts, orange);
    let fresh = std::thread::scope(|s| s.spawn(|| reference::render(&fonts, orange)).join().unwrap());
    assert_ne!(first.data(), cached.data(), "the change of the color shows");
    assert!(cached.data() == fresh.data(), "the cached items are the same as a new drawing");
}
