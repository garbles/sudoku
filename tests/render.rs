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

/// The limits of `reference::compare` for an image: the mean difference and the part of the pixels that
/// differ by more than 64. The text of ImageMagick and of tiny-skia has different anti-aliasing at the
/// edges of the glyphs. The license text page (R§9.4) has much more text than the other screens, thus
/// it has higher limits. The checks of the pixels away from the edges are the same for all images.
fn limits(name: &str) -> (f64, f64) {
    if name.starts_with("licenses-text") { (2.5, 0.015) } else { (1.5, 0.008) }
}

#[test]
fn screens_agree_with_the_reference_images() {
    reference::use_utc();
    let fonts = fonts();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for (name, setup) in reference::states() {
        let pm = reference::render(&fonts, setup);
        let want = decode_png(&std::fs::read(root.join("images").join(name)).unwrap());
        let (mean, big) = reference::compare(&pm, &want);
        let (max_mean, max_big) = limits(name);
        assert!(mean < max_mean && big < max_big, "{name}: mean difference {mean:.2}, {:.3}% of pixels differ", big * 100.0);
        // Each pixel that differs is at the edge of an item: no item is missing, extra or moved.
        let off2 = reference::differences_off_edges(&pm, &want, 2);
        assert!(off2.is_empty(), "{name}: pixels that differ away from all edges: {:?}", &off2[..off2.len().min(10)]);
        let off1 = reference::differences_off_edges(&pm, &want, 1);
        assert!(off1.len() <= 2, "{name}: pixels that differ more than 1 px from an edge: {:?}", &off1[..off1.len().min(10)]);
    }
}

#[test]
fn the_icon_sheet_shows_each_icon() {
    // Each tile (all looks of all icons, then the AppLoad icon) has pixels of an icon around its
    // center. The corner of the area is the background of the tile: white, or the black slot.
    let fonts = fonts();
    let sheet = reference::icon_sheet(&fonts);
    let tiles = reference::icon_tiles().len() as u32 + 1;
    assert_eq!(sheet.width(), reference::ICON_COLUMNS * reference::ICON_TILE_W);
    // images/icons.png is the current sheet (make media-images makes it again).
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let want = decode_png(&std::fs::read(root.join("images/icons.png")).unwrap());
    assert_eq!((want.width(), want.height()), (sheet.width(), sheet.height()), "images/icons.png: size");
    let (mean, big) = reference::compare(&sheet, &want);
    assert!(mean < 0.1 && big < 0.0005, "images/icons.png: mean difference {mean:.3}, {:.3}% of pixels differ", big * 100.0);
    for i in 0..tiles {
        let cx = (i % reference::ICON_COLUMNS * reference::ICON_TILE_W + reference::ICON_TILE_W / 2) as i32;
        let cy = (i / reference::ICON_COLUMNS * reference::ICON_TILE_H + reference::ICON_CENTER_Y) as i32;
        let background = rgb_at(&sheet, cx - 50, cy - 50);
        let area = (cx - 50..=cx + 50).flat_map(|x| (cy - 50..=cy + 50).map(move |y| (x, y)));
        let drawn = area.filter(|&(x, y)| rgb_at(&sheet, x, y) != background).count();
        assert!(drawn >= 20, "tile {i}: {drawn} pixels of an icon");
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
    // R§9.4: also the Licenses screens.
    for (name, setup) in reference::states().into_iter().filter(|(n, _)| n.starts_with("analytics") || n.starts_with("licenses")) {
        let pm = reference::render(&fonts, setup);
        let file = name.replace(".png", "-landscape.png");
        let want = decode_png(&std::fs::read(root.join("images").join(&file)).unwrap());
        let (mean, big) = reference::compare(&pm, &want);
        let (max_mean, max_big) = limits(&file);
        assert!(mean < max_mean && big < max_big, "{file}: mean difference {mean:.2}, {:.3}% of pixels differ", big * 100.0);
        let off2 = reference::differences_off_edges(&pm, &want, 2);
        assert!(off2.is_empty(), "{file}: pixels that differ away from all edges: {:?}", &off2[..off2.len().min(10)]);
    }
    sudoku::layout::set_orientation(sudoku::layout::Orientation::Portrait);
}

#[test]
fn landscape_color_menu_agrees_with_the_reference_image() {
    // NAV-17, COL-8, R§3.3, §15: the Cell colors panel in landscape. reference::states() draws in
    // portrait, thus this test uses the state of color-menu.png in landscape. Box 0 of this state is
    // full and has a clash with a cell outside the box. It is out of the panel and has no completed
    // color (COL-8).
    reference::use_utc();
    let fonts = fonts();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    sudoku::layout::set_orientation(sudoku::layout::Orientation::Landscape);
    let setup = reference::states().into_iter().find(|s| s.0 == "color-menu.png").expect("reference state").1;
    let pm = reference::render(&fonts, setup);
    sudoku::layout::set_orientation(sudoku::layout::Orientation::Portrait);
    let want = decode_png(&std::fs::read(root.join("images/color-menu-landscape.png")).unwrap());
    // compare() stops at the end of the shorter picture: the two sizes must be the same.
    assert_eq!((want.width(), want.height()), (2160, 1620), "the size of the image");
    assert_eq!((pm.width(), pm.height()), (2160, 1620), "the size of the app");
    let (mean, big) = reference::compare(&pm, &want);
    assert!(mean < 1.5 && big < 0.008, "mean difference {mean:.2}, {:.3}% of pixels differ", big * 100.0);
    let off2 = reference::differences_off_edges(&pm, &want, 2);
    assert!(off2.is_empty(), "pixels that differ away from all edges: {:?}", &off2[..off2.len().min(10)]);
    let off1 = reference::differences_off_edges(&pm, &want, 1);
    assert!(off1.len() <= 2, "pixels that differ more than 1 px from an edge: {:?}", &off1[..off1.len().min(10)]);
    // A panel at a different position by some pixels stays near the edges of the image. Thus the
    // lines of the panel must be at the exact pixels in the app and in the image.
    let probes = [(1248, 800), (1648, 800), (2046, 800), (1500, 112), (1900, 112), (1500, 1191), (1900, 1191)];
    for (x, y) in probes {
        assert_eq!(rgb_at(&pm, x, y), [0, 0, 0], "app: no line of the panel at ({x}, {y})");
        assert_eq!(rgb_at(&want, x, y), [0, 0, 0], "image: no line of the panel at ({x}, {y})");
    }
    // The empty space below the Micro-cell lines section is plain white (NAV-17).
    for y in 840..1185 {
        for x in (1655..2040).step_by(5) {
            assert_eq!(rgb_at(&pm, x, y), [255, 255, 255], "app: the empty space at ({x}, {y})");
            assert_eq!(rgb_at(&want, x, y), [255, 255, 255], "image: the empty space at ({x}, {y})");
        }
    }
}

#[test]
fn the_board_is_at_its_position_in_each_orientation() {
    // R§3.3, NAV-10: the board is at the center of the screen in portrait and in landscape. The
    // board is drawn in stripes on threads (render::draw_board_stripes): each thread must get the
    // rotation and the hand of the drawing thread, or the board goes to the portrait position.
    let fonts = fonts();
    reference::use_utc();
    for o in [sudoku::layout::Orientation::Portrait, sudoku::layout::Orientation::Landscape] {
        sudoku::layout::set_orientation(o);
        let pm = reference::render(&fonts, |a| a.game = reference::sample_game());
        let (bx, by) = (sudoku::layout::board_x(), sudoku::layout::board_y());
        let b = sudoku::layout::BOARD_SIZE;
        // The four thick borders of the board.
        for (x, y) in [(bx, by + b / 2), (bx + b, by + b / 2), (bx + b / 2, by), (bx + b / 2, by + b)] {
            let p = rgb_at(&pm, x, y);
            assert!(p[0] < 60, "{o:?}: no board border at ({x}, {y}): {p:?}");
        }
        // The white edge of the screen, at the left of and above the board.
        assert_eq!(rgb_at(&pm, bx - 20, by + b / 2), [255, 255, 255], "{o:?}: pixels at the left of the board");
        assert_eq!(rgb_at(&pm, bx + b / 2, by - 20), [255, 255, 255], "{o:?}: pixels above the board");
    }
    sudoku::layout::set_orientation(sudoku::layout::Orientation::Portrait);
}

/// The screen pixels of the framebuffer (physical, with the rotation of the tablet): the screenshot
/// of what the player sees.
fn screenshot(fb: &[u8]) -> tiny_skia::Pixmap {
    let (w, h) = (sudoku::layout::screen_w(), sudoku::layout::screen_h());
    let mut pm = tiny_skia::Pixmap::new(w, h).unwrap();
    let (base, sx, sy) = sudoku::layout::physical_steps();
    for y in 0..h as usize {
        for x in 0..w as usize {
            let i = (base + x as i64 * sx + y as i64 * sy) as usize * 3;
            let px = tiny_skia::PremultipliedColorU8::from_rgba(fb[i], fb[i + 1], fb[i + 2], 255).unwrap();
            pm.pixels_mut()[y * w as usize + x] = px;
        }
    }
    pm
}

/// Writes a screenshot to `target/screenshots/`, to examine the screen after a failure.
fn save_screenshot(name: &str, pm: &tiny_skia::Pixmap) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/screenshots");
    std::fs::create_dir_all(&dir).unwrap();
    let file = std::fs::File::create(dir.join(name)).unwrap();
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), pm.width(), pm.height());
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header().unwrap();
    let rgb: Vec<u8> = pm.data().chunks_exact(4).flat_map(|px| px[..3].to_vec()).collect();
    writer.write_image_data(&rgb).unwrap();
}

#[test]
fn a_turn_from_portrait_to_landscape_shows_the_landscape_screen() {
    // R§3.3, NAV-10, NAV-13: the app starts in portrait; a turn draws the landscape screen with the
    // board at the landscape position. The test takes a screenshot of the framebuffer before and
    // after the turn (target/screenshots/).
    reference::use_utc();
    let fonts = fonts();
    sudoku::layout::set_orientation(sudoku::layout::Orientation::Portrait);
    let mut app = reference::app(&fonts);
    app.game = reference::sample_game();
    let mut presenter = sudoku::present::Presenter::new();
    let n = (sudoku::layout::PHYSICAL_W * sudoku::layout::PHYSICAL_H * 3) as usize;
    let mut fb = vec![0u8; n];
    presenter.draw_all(&app, &fonts, &mut fb);
    save_screenshot("screen-portrait.png", &screenshot(&fb));

    // The tablet turns: the platform sets the rotation, the app removes the ink that waits, and the
    // presenter starts with a new frame.
    sudoku::layout::set_orientation(sudoku::layout::Orientation::Landscape);
    app.rotate(sudoku::render::ng_stops(&fonts));
    presenter.reset(100);
    let mut updates = Vec::new();
    for step in 0..100 {
        presenter.present(&mut app, &fonts, &mut fb, 100 + step * 30, |u| updates.push(u));
    }
    assert!(!updates.is_empty(), "the app did not send an update after the turn");

    let shot = screenshot(&fb);
    save_screenshot("screen-landscape-after-turn.png", &shot);
    // The screenshot is the full drawing of the app in landscape.
    let (w, h) = (sudoku::layout::screen_w(), sudoku::layout::screen_h());
    let mut want = tiny_skia::Pixmap::new(w, h).unwrap();
    sudoku::render::draw(&mut sudoku::render::Canvas::new(&mut want, &fonts), &app);
    assert!(shot.data() == want.data(), "the screenshot after the turn is not the landscape screen");
    // The board is at the landscape position: its four thick borders (NAV-10).
    let (bx, by) = (sudoku::layout::board_x(), sudoku::layout::board_y());
    let b = sudoku::layout::BOARD_SIZE;
    for (x, y) in [(bx, by + b / 2), (bx + b, by + b / 2), (bx + b / 2, by), (bx + b / 2, by + b)] {
        let p = rgb_at(&shot, x, y);
        assert!(p[0] < 60, "no board border after the turn at ({x}, {y}): {p:?}");
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
    // redo, the complete game with the gray bookmark (CKP-9), the caution, the mode, the hand) and puts
    // ink over them. Each retained frame must be the same as a full drawing.
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
        if next(3) == 0 {
            a.game.save_checkpoint();
        }
        let complete = next(6) == 0;
        if complete {
            a.game = reference::completed_game();
        }
        a.game.mode = next(7) as u8;
        a.ui.menu = match next(6) {
            0 => Menu::Closed,
            1 => Menu::Open(None),
            2 => Menu::Open(Some(Panel::NewGame)),
            3 => Menu::Open(Some(Panel::Colors)),
            // The Checkpoints panel is not available on a complete game (CKP-9).
            4 if !complete => Menu::Open(Some(Panel::Checkpoints)),
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

#[test]
fn the_cached_rows_of_the_games_list_are_the_same_as_a_new_drawing() {
    // The list of completed games keeps the pixels of each row. The delete dialog over the list
    // then gets its rows from the cache. They must be the same as a drawing with no cache.
    reference::use_utc();
    let fonts = fonts();
    let setup = |name: &str| reference::states().into_iter().find(|s| s.0 == name).expect("reference state").1;
    let (list, delete) = (setup("analytics-games.png"), setup("analytics-delete.png"));
    reference::render(&fonts, list);
    let cached = reference::render(&fonts, delete);
    let fresh = std::thread::scope(|s| {
        s.spawn(|| {
            reference::use_utc();
            reference::render(&fonts, delete)
        })
        .join()
        .unwrap()
    });
    assert!(cached.data() == fresh.data(), "the cached rows are the same as a new drawing");
}

/// The layouts of the game screen (R§3.1, R§3.3): each orientation with each hand.
const LAYOUTS: [(sudoku::layout::Orientation, bool); 4] = [
    (sudoku::layout::Orientation::Portrait, false),
    (sudoku::layout::Orientation::Portrait, true),
    (sudoku::layout::Orientation::Landscape, false),
    (sudoku::layout::Orientation::Landscape, true),
];

fn set_layout((orientation, right_handed): (sudoku::layout::Orientation, bool)) {
    sudoku::layout::set_orientation(orientation);
    sudoku::layout::set_right_handed(right_handed);
}

/// A game where a peer of `cell` has the number `digit`, and `cell` has all nine marks. Thus the mark
/// of `digit` is hidden (MRK-4), and the marks of the other digits show.
fn refused_game((r, c): sudoku::game::Cell, digit: u8) -> sudoku::game::Game {
    let mut givens = [[0u8; 9]; 9];
    givens[r as usize][(c as usize + 1) % 9] = digit;
    let mut g = sudoku::game::Game::new(1, 2, [[1; 9]; 9], givens);
    for d in 1..=9 {
        g.toggle_mark_with((r, c), d, false);
    }
    g
}

/// A full drawing of the app in the current layout.
fn draw_app(fonts: &sudoku::font::Fonts, app: &sudoku::app::App) -> tiny_skia::Pixmap {
    let mut pm = tiny_skia::Pixmap::new(sudoku::layout::screen_w(), sudoku::layout::screen_h()).unwrap();
    sudoku::render::draw(&mut sudoku::render::Canvas::new(&mut pm, fonts), app);
    pm
}

/// The pixels that are not the same in the two pictures.
fn changed_pixels(a: &tiny_skia::Pixmap, b: &tiny_skia::Pixmap) -> Vec<(i32, i32)> {
    let w = a.width() as usize;
    a.pixels()
        .iter()
        .zip(b.pixels())
        .enumerate()
        .filter(|(_, (p, q))| p != q)
        .map(|(i, _)| ((i % w) as i32, (i / w) as i32))
        .collect()
}

/// The center of the x of `digit` from the cell origin (MRK-13): the keypad position of the digit.
fn x_center(digit: u8) -> (i32, i32) {
    const ANCHOR: [i32; 3] = [24, 74, 122];
    let i = digit as usize - 1;
    (ANCHOR[i % 3], ANCHOR[i / 3])
}

/// True if each pixel of `pixels` has a pixel of `others` within 1 px.
fn all_near(pixels: &[(i32, i32)], others: &[(i32, i32)]) -> bool {
    pixels.iter().all(|&(x, y)| others.iter().any(|&(u, v)| (u - x).abs() <= 1 && (v - y).abs() <= 1))
}

#[test]
fn the_x_of_the_app_is_at_the_position_of_the_x_in_the_image() {
    // §15, MRK-13: the general comparison accepts all differences near an edge of the image, and
    // the lines of the x are thin. Thus most pixels of the x are near an edge. Thus this test compares
    // the x alone: the change from screen-in-progress.png to screen-invalid-mark.png must be at the
    // same pixels in the app and in the images (within 1 px). The edges can have a different gray.
    reference::use_utc();
    let fonts = fonts();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let image = |name: &str| decode_png(&std::fs::read(root.join("images").join(name)).unwrap());
    let (image_x, image_no_x) = (image("screen-invalid-mark.png"), image("screen-in-progress.png"));
    let setup = reference::states().into_iter().find(|s| s.0 == "screen-invalid-mark.png").expect("reference state").1;
    let app_x = reference::render(&fonts, setup);
    let app_no_x = reference::render(&fonts, |a| a.game = reference::sample_game());
    let in_image = changed_pixels(&image_x, &image_no_x);
    let in_app = changed_pixels(&app_x, &app_no_x);
    assert!(!in_image.is_empty(), "the image shows no x");
    assert!(!in_app.is_empty(), "the app shows no x");
    assert!(all_near(&in_image, &in_app), "the image changes pixels that the app does not change");
    assert!(all_near(&in_app, &in_image), "the app changes pixels that the image does not change");
    // The middle of each line: the same dark gray in the app and in the image.
    let (ox, oy) = sudoku::layout::cell_origin((1, 6));
    let (ax, ay) = x_center(6);
    let (cx, cy) = (ox + ax, oy + ay);
    for t in -8..=8 {
        for (x, y) in [(cx + t, cy + t), (cx + t, cy - t)] {
            let (p, q) = (rgb_at(&app_x, x, y), rgb_at(&image_x, x, y));
            assert!(p.iter().zip(q).all(|(&a, b)| (a as i32 - b as i32).abs() <= 64), "({x}, {y}): app {p:?}, image {q:?}");
        }
    }
}

#[test]
fn the_x_changes_only_its_micro_cell_in_each_layout() {
    // MRK-13, MRK-4, R§3.1, R§3.3, §15: for each of the 9 digits, in portrait and landscape, with
    // each hand, in the cells at the corners and in a middle cell. The x has straight lines of
    // gray15 from -10 to +10 px around the anchor of the digit. It stays between the lines of its
    // micro-cell. The other marks and the grid stay. No mark shows under the x.
    let fonts = fonts();
    let mut app = reference::app(&fonts);
    let dark = sudoku::render::gray(15);
    for layout in LAYOUTS {
        set_layout(layout);
        for cell in [(0, 0), (0, 8), (1, 6), (8, 0), (8, 8)] {
            let (ox, oy) = sudoku::layout::cell_origin(cell);
            for d in 1..=9u8 {
                let at = format!("{layout:?}, cell {cell:?}, digit {d}");
                app.game = refused_game(cell, d);
                app.ui.refused = None;
                let before = draw_app(&fonts, &app);
                app.ui.refused = Some((cell, d, 0));
                let with = draw_app(&fonts, &app);

                let (ax, ay) = x_center(d);
                let (cx, cy) = (ox + ax, oy + ay);
                let i = d as i32 - 1;
                let (x0, x1) = (ox + sudoku::layout::micro_offset(i % 3), ox + sudoku::layout::micro_offset(i % 3 + 1));
                let (y0, y1) = (oy + sudoku::layout::micro_offset(i / 3), oy + sudoku::layout::micro_offset(i / 3 + 1));
                let changed = changed_pixels(&with, &before);
                assert!(!changed.is_empty(), "{at}: no x");
                for &(x, y) in &changed {
                    // The extent: half of 20 px, then half of the 3 px line, then 1 px of edge.
                    assert!((x - cx).abs() <= 13 && (y - cy).abs() <= 13, "{at}: pixel ({x}, {y}) is out of the x");
                    // The lines are 2 px wide on the grid and 3 px wide on the cell edge.
                    assert!(x > x0 + 1 && x < x1 - 1 && y > y0 + 1 && y < y1 - 1, "{at}: pixel ({x}, {y}) touches a line");
                    assert_eq!(rgb_at(&before, x, y), [255, 255, 255], "{at}: a mark or a line is under the x at ({x}, {y})");
                }
                // The two lines, and their ends.
                for t in -10..=9 {
                    for (x, y) in [(cx + t, cy + t), (cx + t, cy - t - 1)] {
                        assert_eq!(rgb_at(&with, x, y), dark, "{at}: no line of the x at ({x}, {y})");
                    }
                }
                // The middle of each side of the x is empty.
                for (x, y) in [(cx, cy - 8), (cx, cy + 8), (cx - 8, cy), (cx + 8, cy)] {
                    assert_eq!(rgb_at(&with, x, y), [255, 255, 255], "{at}: the x is not an x at ({x}, {y})");
                }
                // The marks of the other digits show (else the test does not examine them).
                let other = if d == 1 { 2 } else { 1 };
                let (mx, my) = x_center(other);
                let mark_shows = (-12..12).any(|t| rgb_at(&with, ox + mx + t, oy + my) != [255, 255, 255]);
                assert!(mark_shows, "{at}: the mark {other} does not show");
            }
        }
    }
    sudoku::layout::set_orientation(sudoku::layout::Orientation::Portrait);
    sudoku::layout::set_right_handed(false);
}

#[test]
fn the_x_shows_with_no_grid_when_the_micro_cell_lines_are_none() {
    // MRK-13, COL-7: with the micro-cell lines set to None, the cell has only the x.
    let fonts = fonts();
    let mut app = reference::app(&fonts);
    app.settings.micro_line = sudoku::settings::LineGray::None;
    for layout in LAYOUTS {
        set_layout(layout);
        let cell = (4, 4);
        let (ox, oy) = sudoku::layout::cell_origin(cell);
        let (ax, ay) = x_center(9);
        let mut givens = [[0u8; 9]; 9];
        givens[4][0] = 9;
        app.game = sudoku::game::Game::new(1, 2, [[1; 9]; 9], givens);
        app.ui.refused = Some((cell, 9, 0));
        let pm = draw_app(&fonts, &app);
        let mut x_pixels = 0;
        for y in oy + 3..oy + sudoku::layout::CELL - 2 {
            for x in ox + 3..ox + sudoku::layout::CELL - 2 {
                if rgb_at(&pm, x, y) != [255, 255, 255] {
                    assert!((x - ox - ax).abs() <= 13 && (y - oy - ay).abs() <= 13, "{layout:?}: pixel ({x}, {y}) is not of the x");
                    x_pixels += 1;
                }
            }
        }
        assert!(x_pixels > 40, "{layout:?}: the x has only {x_pixels} pixels");
    }
    sudoku::layout::set_orientation(sudoku::layout::Orientation::Portrait);
    sudoku::layout::set_right_handed(false);
}

#[test]
fn the_x_and_the_question_mark_show_together_in_different_cells() {
    // MRK-14, QST-1: each one changes only its cell. The "?" hides the marks of its cell, and the x
    // does not hide the marks of its cell.
    let fonts = fonts();
    let (question, x) = ((4, 1), (1, 6));
    let mut app = reference::app(&fonts);
    app.game = reference::sample_game();
    for layout in LAYOUTS {
        set_layout(layout);
        app.ui.question = None;
        app.ui.refused = Some((x, 6, 0));
        let only_x = draw_app(&fonts, &app);
        app.ui.refused = None;
        app.ui.question = Some((question, 0));
        let only_question = draw_app(&fonts, &app);
        app.ui.refused = Some((x, 6, 0));
        let both = draw_app(&fonts, &app);
        let inside = |(x, y): (i32, i32), cell| {
            let (ox, oy) = sudoku::layout::cell_origin(cell);
            let s = sudoku::layout::CELL;
            x >= ox && x < ox + s && y >= oy && y < oy + s
        };
        let to_question = changed_pixels(&both, &only_x);
        let to_x = changed_pixels(&both, &only_question);
        assert!(!to_question.is_empty() && to_question.iter().all(|&p| inside(p, question)), "{layout:?}: the \"?\" changes other cells");
        assert!(!to_x.is_empty() && to_x.iter().all(|&p| inside(p, x)), "{layout:?}: the x changes other cells");
    }
    sudoku::layout::set_orientation(sudoku::layout::Orientation::Portrait);
    sudoku::layout::set_right_handed(false);
}

#[test]
fn a_retained_frame_shows_and_removes_the_x() {
    // MRK-13: after the x, the pixels of the cell are the same as before the x. render.rs keeps the
    // pixels of each cell: the key of a cell must contain the x. Thus the x goes to each digit and to
    // other cells on one frame. Each frame must be the same as a full drawing.
    use sudoku::render::{Canvas, FrameRecord};
    let fonts = fonts();
    let mut app = reference::app(&fonts);
    for layout in LAYOUTS {
        set_layout(layout);
        let mut pm = tiny_skia::Pixmap::new(sudoku::layout::screen_w(), sudoku::layout::screen_h()).unwrap();
        let mut record = FrameRecord::default();
        for cell in [(0, 0), (1, 6), (8, 8)] {
            for d in 1..=9u8 {
                app.game = refused_game(cell, d);
                let mut no_x = None;
                for refused in [None, Some((cell, d, 0)), None, Some((cell, d, 0)), Some((cell, d, 500))] {
                    app.ui.refused = refused;
                    sudoku::render::draw(&mut Canvas::retained(&mut pm, &mut record, &fonts), &app);
                    let full = draw_app(&fonts, &app);
                    assert!(pm.data() == full.data(), "{layout:?}, cell {cell:?}, digit {d}: {refused:?}: not the same as a full drawing");
                    if refused.is_none() {
                        let first = no_x.get_or_insert_with(|| pm.clone());
                        assert!(pm.data() == first.data(), "{layout:?}, cell {cell:?}, digit {d}: the x left pixels");
                    }
                }
            }
        }
    }
    sudoku::layout::set_orientation(sudoku::layout::Orientation::Portrait);
    sudoku::layout::set_right_handed(false);
}

#[test]
fn the_framebuffer_shows_the_x_and_then_the_old_pixels() {
    // MRK-13, MRK-14: the output of the main loop (present.rs) writes the x to the framebuffer with
    // an update. After the time of the x, the framebuffer has the same pixels as before the x.
    let fonts = fonts();
    let n = (sudoku::layout::PHYSICAL_W * sudoku::layout::PHYSICAL_H * 3) as usize;
    for layout in LAYOUTS {
        set_layout(layout);
        let mut app = reference::app(&fonts);
        app.game = reference::sample_game();
        let mut presenter = sudoku::present::Presenter::new();
        let mut fb = vec![0u8; n];
        presenter.draw_all(&app, &fonts, &mut fb);
        let before = screenshot(&fb);

        app.ui.refused = Some(((1, 6), 6, 0));
        app.redraw = true;
        let mut updates = 0;
        for step in 0..100 {
            presenter.present(&mut app, &fonts, &mut fb, 100 + step * 30, |_| updates += 1);
        }
        assert!(updates > 0, "{layout:?}: no update shows the x");
        let shown = screenshot(&fb);
        assert!(shown.data() == draw_app(&fonts, &app).data(), "{layout:?}: the framebuffer does not show the x");
        assert!(shown.data() != before.data(), "{layout:?}: the framebuffer did not change");

        app.ui.refused = None;
        app.redraw = true;
        updates = 0;
        for step in 0..100 {
            presenter.present(&mut app, &fonts, &mut fb, 10_000 + step * 30, |_| updates += 1);
        }
        assert!(updates > 0, "{layout:?}: no update removes the x");
        assert!(screenshot(&fb).data() == before.data(), "{layout:?}: the x left pixels in the framebuffer");
    }
    sudoku::layout::set_orientation(sudoku::layout::Orientation::Portrait);
    sudoku::layout::set_right_handed(false);
}

#[test]
fn portrait_manual_pages_are_the_portrait_images() {
    // §12, §15: the snapshot tool compares only the first and the last page. This test compares all
    // pages, also page 5, which tells about the x (images/manual-helpers.png).
    let fonts = fonts();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let names = ["how-to-play", "candidates", "pen", "pen-rules", "helpers", "mistakes", "menu", "new-game", "finished"];
    for (i, name) in names.iter().enumerate() {
        let mut app = reference::app(&fonts);
        app.screen = sudoku::app::Screen::Manual;
        app.manual_page = i as u8 + 1;
        let pm = draw_app(&fonts, &app);
        let want = decode_png(&std::fs::read(root.join(format!("images/manual-{name}.png"))).unwrap());
        assert_eq!(reference::compare(&pm, &want).1, 0.0, "page {}", i + 1);
    }
}

/// The state of a reference image.
fn reference_setup(name: &str) -> reference::Setup {
    reference::states().into_iter().find(|s| s.0 == name).expect("reference state").1
}

/// A point of the cell without lines, digits or marks.
fn cell_background(pm: &tiny_skia::Pixmap, cell: sudoku::game::Cell) -> [u8; 3] {
    let (x, y) = sudoku::layout::cell_origin(cell);
    rgb_at(pm, x + 8, y + 140)
}

/// The darkest pixel of the middle of a cell, with the three channels added: the color of its digit.
/// An edge pixel of the glyph can have a low red value and a higher green value.
fn digit_color(pm: &tiny_skia::Pixmap, cell: sudoku::game::Cell) -> [u8; 3] {
    let (ox, oy) = sudoku::layout::cell_origin(cell);
    let sum = |p: [u8; 3]| p.iter().map(|&v| v as u32).sum::<u32>();
    let mut dark = [255, 255, 255];
    for y in oy + 30..oy + 118 {
        for x in ox + 30..ox + 118 {
            let p = rgb_at(pm, x, y);
            if sum(p) < sum(dark) {
                dark = p;
            }
        }
    }
    dark
}

/// The cells of box 0, column 4 and row 7: the completed groups of screen-completed-group.png. This
/// list does not use the code of the app.
fn in_the_three_groups((r, c): sudoku::game::Cell) -> bool {
    (r < 3 && c < 3) || c == 4 || r == 7
}

fn reset_layout() {
    sudoku::layout::set_orientation(sudoku::layout::Orientation::Portrait);
    sudoku::layout::set_right_handed(false);
}

#[test]
fn the_completed_group_image_shows_the_colors_of_each_cell() {
    // BRD-7, COL-8, §15: the general comparison permits a difference of 64. This test requires the
    // exact color of each cell, in the app and in the image. The cell where column 4 and row 7 cross
    // has the color, and the starting numbers in the groups also have it. The other starting numbers
    // stay gray and the other numbers of the player stay white.
    let fonts = fonts();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let pm = reference::render(&fonts, reference_setup("screen-completed-group.png"));
    let image = decode_png(&std::fs::read(root.join("images/screen-completed-group.png")).unwrap());
    let g = reference::completed_group_game();
    assert!(!g.completed);
    assert!(cells().all(|c| !g.is_clashing(c)), "the state has no clash");
    let (mut green, mut gray, mut white) = (0, 0, 0);
    for cell in cells() {
        let want = if !g.is_filled(cell) {
            white += 1;
            Color::White
        } else if in_the_three_groups(cell) {
            green += 1;
            Color::Green
        } else if g.is_given(cell) {
            gray += 1;
            Color::Gray
        } else {
            white += 1;
            Color::White
        };
        assert_eq!(cell_background(&pm, cell), want.rgb(), "app, cell {cell:?}");
        assert_eq!(cell_background(&image, cell), want.rgb(), "image, cell {cell:?}");
    }
    // Box 0 (9 cells), column 4 (9 cells) and row 7 (9 cells), with the crossing cell (7, 4) one time.
    assert_eq!(green, 26);
    assert!(gray > 0 && white > 0);
    // A digit keeps its color on the completed color: black for a starting number, #1E1E1E for a
    // number of the player.
    for (cell, want) in [((0, 0), [0, 0, 0]), ((0, 2), [0x1e, 0x1e, 0x1e]), ((7, 1), [0x1e, 0x1e, 0x1e]), ((7, 4), [0, 0, 0])] {
        assert_eq!(digit_color(&pm, cell), want, "app, cell {cell:?}");
        assert_eq!(digit_color(&image, cell), want, "image, cell {cell:?}");
    }
}

#[test]
fn the_completed_groups_have_the_color_in_each_layout() {
    // BRD-7, COL-8, R§3.1, R§3.3: the same cells in each orientation, with each hand. The stripes of
    // the board use other threads: each thread must get the same colors.
    let fonts = fonts();
    let g = reference::completed_group_game();
    for layout in LAYOUTS {
        set_layout(layout);
        let mut app = reference::app(&fonts);
        app.game = g.clone();
        let pm = draw_app(&fonts, &app);
        for cell in cells() {
            let want = match (g.is_filled(cell) && in_the_three_groups(cell), g.is_given(cell)) {
                (true, _) => Color::Green,
                (false, true) => Color::Gray,
                (false, false) => Color::White,
            };
            assert_eq!(cell_background(&pm, cell), want.rgb(), "{layout:?}, cell {cell:?}");
        }
    }
    reset_layout();
}

#[test]
fn a_full_box_with_a_clash_outside_the_box_has_no_completed_color() {
    // COL-8, COL-10: box 0 of the sample game is full. Its 9 at (2, 1) clashes with the 9 at (5, 1), in
    // a different box. Thus box 0 is not complete, in the app and in screen-in-progress.png. The
    // erasure of (5, 1) removes the clash, and box 0, far from the erased cell, gets the color.
    let fonts = fonts();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let image = decode_png(&std::fs::read(root.join("images/screen-in-progress.png")).unwrap());
    let sample = reference::sample_game();
    let the_box = || (0..3u8).flat_map(|r| (0..3u8).map(move |c| (r, c)));
    assert!(the_box().all(|c| sample.is_filled(c)), "box 0 is full");
    assert!(sample.is_clashing((2, 1)) && sample.is_clashing((5, 1)));
    for cell in cells() {
        assert_ne!(cell_background(&image, cell), Color::Green.rgb(), "image, cell {cell:?}");
    }
    for layout in LAYOUTS {
        set_layout(layout);
        let mut app = reference::app(&fonts);
        app.game = sample.clone();
        let with_clash = draw_app(&fonts, &app);
        for cell in cells() {
            assert_ne!(cell_background(&with_clash, cell), Color::Green.rgb(), "{layout:?}, clash, cell {cell:?}");
        }
        assert_eq!(cell_background(&with_clash, (2, 1)), Color::Red.rgb(), "{layout:?}");
        app.game.erase((5, 1));
        let erased = draw_app(&fonts, &app);
        for cell in cells() {
            let want = if the_box().any(|c| c == cell) {
                Color::Green
            } else if app.game.is_given(cell) {
                Color::Gray
            } else {
                Color::White
            };
            assert_eq!(cell_background(&erased, cell), want.rgb(), "{layout:?}, erased, cell {cell:?}");
        }
    }
    reset_layout();
}

#[test]
fn the_highlight_is_on_top_of_the_completed_color() {
    // BRD-7: the highlight color comes before the completed color, in a group and out of a group. The
    // completed color comes before the starting color. The test also changes the colors, thus the
    // result does not come from a fixed color.
    let fonts = fonts();
    for layout in LAYOUTS {
        set_layout(layout);
        let mut app = reference::app(&fonts);
        app.game = reference::completed_group_game();
        app.settings.color_starting = Color::Yellow;
        app.settings.color_highlight = Color::Magenta;
        app.settings.color_completed = Color::Orange;
        app.ui.highlight = Some(5);
        let pm = draw_app(&fonts, &app);
        let g = &app.game;
        let (mut highlighted_in_group, mut highlighted_out) = (0, 0);
        for cell in cells() {
            let filled = g.is_filled(cell);
            let in_group = filled && in_the_three_groups(cell);
            let want = if filled && g.value_at(cell) == 5 {
                if in_group {
                    highlighted_in_group += 1;
                } else {
                    highlighted_out += 1;
                }
                Color::Magenta
            } else if in_group {
                Color::Orange
            } else if g.is_given(cell) {
                Color::Yellow
            } else {
                Color::White
            };
            assert_eq!(cell_background(&pm, cell), want.rgb(), "{layout:?}, cell {cell:?}");
        }
        assert!(highlighted_in_group > 0 && highlighted_out > 0, "{layout:?}: the test does not examine the two cases");
    }
    reset_layout();
}

#[test]
fn a_highlight_shows_with_the_menu_and_with_each_panel() {
    // HLT-4, HLT-7: a highlight can show while the menu or a panel is open. No image in images/ shows
    // this state. In each layout and with each panel, the cells outside the panel have the highlight
    // color, the other cells do not have it, and the panel does not change. A retained frame gives the
    // same pixels as a full drawing, when the highlight comes and when it goes.
    use sudoku::app::{Menu, Panel};
    use sudoku::layout::{ckp_panel, colors_panel, ng_panel, settings_panel};
    let fonts = fonts();
    for layout in LAYOUTS {
        set_layout(layout);
        let area = [
            (Menu::Open(None), None),
            (Menu::Open(Some(Panel::Colors)), Some(colors_panel())),
            (Menu::Open(Some(Panel::Settings)), Some(settings_panel(false))),
            (Menu::Open(Some(Panel::NewGame)), Some(ng_panel())),
            (Menu::Open(Some(Panel::Checkpoints)), Some(ckp_panel(0))),
        ];
        for (menu, panel) in area {
            let name = format!("{layout:?}, {menu:?}");
            let state = |highlight: Option<u8>| {
                let mut app = reference::app(&fonts);
                app.game = reference::sample_game();
                app.settings.color_highlight = Color::Magenta;
                app.ui.menu = menu;
                app.ui.highlight = highlight;
                sudoku::invariants::check_app(&app).unwrap_or_else(|e| panic!("{name}: {e}"));
                app
            };
            let (with, without) = (state(Some(5)), state(None));
            let (high, plain) = (draw_app(&fonts, &with), draw_app(&fonts, &without));

            // A cell shows its color if the point of `cell_background` is not under the panel.
            let visible = |cell: sudoku::game::Cell| {
                let (x, y) = sudoku::layout::cell_origin(cell);
                panel.is_none_or(|p| !p.contains((x + 8) as f32, (y + 140) as f32))
            };
            let (mut shown, mut other) = (0, 0);
            for cell in cells().filter(|&c| visible(c)) {
                if with.game.value_at(cell) == 5 {
                    assert_eq!(cell_background(&high, cell), Color::Magenta.rgb(), "{name}: cell {cell:?}");
                    assert_ne!(cell_background(&plain, cell), Color::Magenta.rgb(), "{name}: cell {cell:?}, no highlight");
                    shown += 1;
                } else {
                    assert_ne!(cell_background(&high, cell), Color::Magenta.rgb(), "{name}: cell {cell:?}");
                    other += 1;
                }
            }
            assert!(shown > 0 && other > 0, "{name}: no visible cell examines the highlight");

            // The highlight changes only the board, and not the panel on top of it.
            if let Some(p) = panel {
                for (x, y) in changed_pixels(&plain, &high) {
                    assert!(!p.contains(x as f32, y as f32), "{name}: the highlight changes ({x}, {y}) in the panel");
                }
            }

            // The cached drawing of the frame follows the highlight in the two directions.
            let mut record = sudoku::render::FrameRecord::default();
            let mut frame = tiny_skia::Pixmap::new(sudoku::layout::screen_w(), sudoku::layout::screen_h()).unwrap();
            for (app, full) in [(&without, &plain), (&with, &high), (&without, &plain)] {
                sudoku::render::draw(&mut sudoku::render::Canvas::retained(&mut frame, &mut record, &fonts), app);
                assert!(frame.data() == full.data(), "{name}: the retained frame is not the same as a full drawing");
            }
        }
    }
    reset_layout();
}

#[test]
fn the_switch_off_and_white_give_the_same_board_with_no_completed_color() {
    // COL-11, BRD-7: the two conditions give the same pixels of the board: a starting number keeps
    // its starting color. The menu is closed, thus the settings do not show in a panel.
    let fonts = fonts();
    for layout in LAYOUTS {
        set_layout(layout);
        let mut app = reference::app(&fonts);
        app.game = reference::completed_group_game();
        let on = draw_app(&fonts, &app);
        app.settings.completed_color_on = false;
        let off = draw_app(&fonts, &app);
        app.settings.completed_color_on = true;
        app.settings.color_completed = Color::White;
        let white = draw_app(&fonts, &app);
        assert!(off.data() == white.data(), "{layout:?}: the switch off and White are not the same");
        assert!(on.data() != off.data(), "{layout:?}: the switch changes nothing");
        for cell in cells() {
            let want = if app.game.is_given(cell) { Color::Gray } else { Color::White };
            assert_eq!(cell_background(&off, cell), want.rgb(), "{layout:?}, cell {cell:?}");
        }
    }
    reset_layout();
}

#[test]
fn a_completed_game_shows_no_completed_color() {
    // COL-12: all groups are full with no clash, but the board has the normal colors, in the app and in
    // screen-completed.png. A saved completed game has the same state.
    let fonts = fonts();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let image = decode_png(&std::fs::read(root.join("images/screen-completed.png")).unwrap());
    let g = reference::completed_game();
    assert!(g.completed);
    let want = |cell| if g.is_given(cell) { Color::Gray.rgb() } else { Color::White.rgb() };
    for cell in cells() {
        assert_eq!(cell_background(&image, cell), want(cell), "image, cell {cell:?}");
    }
    for layout in LAYOUTS {
        set_layout(layout);
        let mut app = reference::app(&fonts);
        app.game = g.clone();
        let pm = draw_app(&fonts, &app);
        for cell in cells() {
            assert_eq!(cell_background(&pm, cell), want(cell), "{layout:?}, cell {cell:?}");
        }
        // The switch does not change the board of a completed game.
        app.settings.completed_color_on = false;
        assert!(draw_app(&fonts, &app).data() == pm.data(), "{layout:?}: the switch changes a completed game");
    }
    reset_layout();
}

#[test]
fn a_retained_frame_follows_each_change_of_the_completed_color() {
    // BRD-7, COL-3, COL-10, COL-11, COL-12: render.rs keeps the pixels of each board cell. The key of a
    // cell must contain the completed color. A far recolor changes cells that are not peers of the
    // changed cell. Each frame must be the same as a full drawing.
    use sudoku::render::{Canvas, FrameRecord};
    let fonts = fonts();
    for layout in LAYOUTS {
        set_layout(layout);
        let mut app = reference::app(&fonts);
        let mut pm = tiny_skia::Pixmap::new(sudoku::layout::screen_w(), sudoku::layout::screen_h()).unwrap();
        let mut record = FrameRecord::default();
        let mut check = |app: &sudoku::app::App, step: &str| {
            sudoku::render::draw(&mut Canvas::retained(&mut pm, &mut record, &fonts), app);
            assert!(pm.data() == draw_app(&fonts, app).data(), "{layout:?}, {step}: not the same as a full drawing");
        };
        app.game = reference::sample_game();
        check(&app, "clash");
        app.game.erase((5, 1)); // Box 0 gets the color: the cells are far from (5, 1).
        check(&app, "erasure");
        app.game.undo();
        check(&app, "undo");
        app.game.redo();
        check(&app, "redo");
        app.game = reference::completed_group_game();
        check(&app, "three groups");
        app.game.add_number((2, 5), 9); // A clash stops the groups of the clashing cells.
        check(&app, "new clash");
        app.game.undo();
        check(&app, "the clash goes away");
        app.settings.color_completed = Color::Orange;
        check(&app, "other color");
        app.ui.highlight = Some(5);
        check(&app, "highlight");
        app.ui.highlight = None;
        check(&app, "no highlight");
        app.settings.completed_color_on = false;
        check(&app, "switch off");
        app.settings.completed_color_on = true;
        check(&app, "switch on");
        app.settings.color_completed = Color::White;
        check(&app, "white");
        app.settings.color_completed = Color::Green;
        check(&app, "green");
        app.game = reference::completed_game();
        check(&app, "completed game");
        app.game = reference::completed_group_game();
        check(&app, "new game");
    }
    reset_layout();
}

#[test]
fn the_framebuffer_shows_the_recolor_of_a_far_group() {
    // COL-10: the output of the main loop (present.rs). The erasure of (5, 1) recolors box 0, and the
    // copy to the framebuffer must contain all these cells.
    let fonts = fonts();
    let n = (sudoku::layout::PHYSICAL_W * sudoku::layout::PHYSICAL_H * 3) as usize;
    for layout in LAYOUTS {
        set_layout(layout);
        let mut app = reference::app(&fonts);
        app.game = reference::sample_game();
        let mut presenter = sudoku::present::Presenter::new();
        let mut fb = vec![0u8; n];
        presenter.draw_all(&app, &fonts, &mut fb);
        app.game.erase((5, 1));
        app.redraw = true;
        let mut updates = 0;
        for step in 0..100 {
            presenter.present(&mut app, &fonts, &mut fb, 100 + step * 30, |_| updates += 1);
        }
        assert!(updates > 0, "{layout:?}: no update shows the color");
        let shown = screenshot(&fb);
        assert!(shown.data() == draw_app(&fonts, &app).data(), "{layout:?}: the framebuffer does not show the color");
        assert_eq!(cell_background(&shown, (0, 0)), Color::Green.rgb(), "{layout:?}");
    }
    reset_layout();
}

#[test]
fn the_colors_panel_has_the_same_pixels_and_covers_nothing_in_each_layout() {
    // NAV-17: the panel is at its x range and at y 112 to 1192. The items of the panel are the same in
    // the four layouts, thus the order of the columns does not change with the hand. Out of the panel,
    // only the toolbar slot of the paint bucket changes when the panel opens: the panel does not cover
    // a toolbar button, the close button, the meter or "Completed in".
    use sudoku::app::{Menu, Panel};
    let fonts = fonts();
    let mut first: Option<Vec<[u8; 3]>> = None;
    for (layout, x0) in LAYOUTS.into_iter().zip([708, 112, 1248, 112]) {
        set_layout(layout);
        for (game, name) in [(reference::sample_game(), "game"), (reference::completed_game(), "completed game")] {
            let at = format!("{layout:?}, {name}");
            let mut app = reference::app(&fonts);
            app.game = game;
            app.ui.menu = Menu::Open(None);
            let closed = draw_app(&fonts, &app);
            app.ui.menu = Menu::Open(Some(Panel::Colors));
            let open = draw_app(&fonts, &app);
            let tb = sudoku::layout::tb_x();
            let mut slots = std::collections::BTreeSet::new();
            for (x, y) in changed_pixels(&open, &closed) {
                if (x0..x0 + 800).contains(&x) && (112..=1192).contains(&y) {
                    continue;
                }
                assert!((tb..tb + sudoku::layout::TB_SLOT).contains(&x), "{at}: the panel changes ({x}, {y})");
                slots.insert(y / sudoku::layout::TB_SLOT);
            }
            assert_eq!(slots.len(), 1, "{at}: the panel changes the toolbar slots {slots:?}");
            // The inner pixels of the panel: all of them, except the two side lines.
            let mut inner = Vec::with_capacity(796 * 1081);
            for y in 112..=1192 {
                for x in x0 + 2..x0 + 798 {
                    inner.push(rgb_at(&open, x, y));
                }
            }
            let first = first.get_or_insert_with(|| inner.clone());
            assert!(*first == inner, "{at}: the panel does not have the same pixels as in the first layout");
        }
    }
    reset_layout();
}

#[test]
fn the_cached_items_of_the_completed_section_are_the_same_as_a_new_drawing() {
    // COL-5, §10: a change of the completed color changes the state of items in the other sections
    // (a color that a section uses is not available). The cached items must follow it.
    let fonts = fonts();
    fn orange(a: &mut sudoku::app::App) {
        a.ui.menu = sudoku::app::Menu::Open(Some(sudoku::app::Panel::Colors));
        a.settings.color_completed = Color::Orange;
    }
    for layout in LAYOUTS {
        set_layout(layout);
        let green = reference::render(&fonts, |a| a.ui.menu = sudoku::app::Menu::Open(Some(sudoku::app::Panel::Colors)));
        let cached = reference::render(&fonts, orange);
        assert!(green.data() != cached.data(), "{layout:?}: the change of the color does not show");
        let fresh = std::thread::scope(|s| {
            s.spawn(|| {
                set_layout(layout);
                reference::render(&fonts, orange)
            })
            .join()
            .unwrap()
        });
        assert!(cached.data() == fresh.data(), "{layout:?}: the cached items are not the same as a new drawing");
    }
    reset_layout();
}

// ---- The Checkpoints panel (REQUIREMENTS.md §9.3). The expected positions are the values of the
// documents, as an offset from the left edge of the panel. They do not use the constants of layout.rs.

/// The left edge of the Checkpoints panel in each of the LAYOUTS (NAV-18).
const CKP_LEFT: [i32; 4] = [908, 112, 1448, 112];
const WHITE: [u8; 3] = [255, 255, 255];
const BLACK: [u8; 3] = [0, 0, 0];

fn ckp_row_top(i: i32) -> i32 {
    672 + 180 * i
}

/// The last y of the panel with `n` checkpoints (NAV-18).
fn ckp_bottom(n: i32) -> i32 {
    if n == 0 {
        864
    } else {
        ckp_row_top(n)
    }
}

fn with_panel(fonts: &sudoku::font::Fonts, game: sudoku::game::Game) -> sudoku::app::App {
    let mut app = reference::app(fonts);
    app.game = game;
    app.ui.menu = sudoku::app::Menu::Open(Some(sudoku::app::Panel::Checkpoints));
    app
}

/// A setup in the current layout, also the hand (reference::render uses a left-handed player).
fn draw_setup(fonts: &sudoku::font::Fonts, setup: reference::Setup) -> tiny_skia::Pixmap {
    let mut app = reference::app(fonts);
    setup(&mut app);
    draw_app(fonts, &app)
}

/// The pixels of the area around the baseline are the same as `text` on a white background.
fn assert_text(
    fonts: &sudoku::font::Fonts,
    pm: &tiny_skia::Pixmap,
    (weight, size, color): (sudoku::font::Weight, f32, [u8; 3]),
    (x, y): (i32, i32),
    anchor: sudoku::font::Anchor,
    text: &str,
    area: std::ops::Range<i32>,
) {
    let mut want = tiny_skia::Pixmap::new(pm.width(), pm.height()).unwrap();
    want.fill(tiny_skia::Color::WHITE);
    sudoku::render::Canvas::new(&mut want, fonts).text(weight, size, x as f32, y as f32, anchor, text, color);
    for py in y - size as i32..y + 8 {
        for px in area.clone() {
            assert_eq!(rgb_at(pm, px, py), rgb_at(&want, px, py), "the text \"{text}\" at ({px}, {py})");
        }
    }
}

/// The first pixel of a cell of the small board of row `i`.
fn small_cell((pl, i): (i32, i32), (r, c): sudoku::game::Cell) -> (i32, i32) {
    (pl + 18 + 17 * c as i32, ckp_row_top(i) + 14 + 17 * r as i32)
}

/// The background of a cell of the small board: the digit, the dots and the lines do not touch this pixel.
fn small_bg(pm: &tiny_skia::Pixmap, at: (i32, i32), cell: sudoku::game::Cell) -> [u8; 3] {
    let (x, y) = small_cell(at, cell);
    rgb_at(pm, x + 2, y + 2)
}

#[test]
fn landscape_checkpoints_menu_agrees_with_the_reference_image() {
    // NAV-18, R§3.3, §15: reference::states() draws in portrait, thus this test uses the state of
    // checkpoints-menu.png in landscape.
    reference::use_utc();
    let fonts = fonts();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    sudoku::layout::set_orientation(sudoku::layout::Orientation::Landscape);
    let pm = reference::render(&fonts, reference_setup("checkpoints-menu.png"));
    sudoku::layout::set_orientation(sudoku::layout::Orientation::Portrait);
    let want = decode_png(&std::fs::read(root.join("images/checkpoints-menu-landscape.png")).unwrap());
    assert_eq!((want.width(), want.height()), (2160, 1620), "the size of the image");
    assert_eq!((pm.width(), pm.height()), (2160, 1620), "the size of the app");
    let (mean, big) = reference::compare(&pm, &want);
    assert!(mean < 1.5 && big < 0.008, "mean difference {mean:.2}, {:.3}% of pixels differ", big * 100.0);
    let off2 = reference::differences_off_edges(&pm, &want, 2);
    assert!(off2.is_empty(), "pixels that differ away from all edges: {:?}", &off2[..off2.len().min(10)]);
    let off1 = reference::differences_off_edges(&pm, &want, 1);
    assert!(off1.len() <= 2, "pixels that differ more than 1 px from an edge: {:?}", &off1[..off1.len().min(10)]);
}

#[test]
fn the_lines_of_the_checkpoints_panel_are_at_the_exact_pixels_of_each_image() {
    // NAV-18, CKP-7: a panel at a different position by some pixels stays near the edges of the image.
    // Thus the frame, the lines between the rows and the buttons must be at the exact pixels in the
    // app and in the image.
    use sudoku::layout::Orientation::{Landscape, Portrait};
    let fonts = fonts();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let images = [
        ("checkpoints-menu-empty.png", "checkpoints-menu-empty.png", Portrait, 908, 0),
        ("checkpoints-menu.png", "checkpoints-menu.png", Portrait, 908, 3),
        ("checkpoints-menu-current.png", "checkpoints-menu-current.png", Portrait, 908, 5),
        ("checkpoints-menu-right-handed.png", "checkpoints-menu-right-handed.png", Portrait, 112, 3),
        ("checkpoints-menu.png", "checkpoints-menu-landscape.png", Landscape, 1448, 3),
    ];
    for (state, file, orientation, pl, n) in images {
        sudoku::layout::set_orientation(orientation);
        let pm = reference::render(&fonts, reference_setup(state));
        sudoku::layout::set_orientation(Portrait);
        let want = decode_png(&std::fs::read(root.join("images").join(file)).unwrap());
        let bottom = ckp_bottom(n);
        let mut black = vec![
            (pl, 616),
            (pl + 1, 616),
            (pl + 598, 616),
            (pl + 599, 616),
            (pl + 300, 560),
            (pl + 300, 561),
            (pl + 300, 670),
            (pl + 300, 671),
            (pl + 300, bottom - 1),
            (pl + 300, bottom),
        ];
        let mut white = vec![(pl + 2, 616), (pl + 597, 616), (pl + 300, 562), (pl + 300, 669), (pl + 300, 672), (pl + 300, bottom - 2)];
        for i in 0..n {
            let y0 = ckp_row_top(i);
            // The button: the first and the last pixels are in the button.
            black.extend([(pl + 381, y0 + 55), (pl + 521, y0 + 55), (pl + 381, y0 + 125), (pl + 521, y0 + 125)]);
            white.extend([(pl + 380, y0 + 90), (pl + 522, y0 + 90), (pl + 400, y0 + 54), (pl + 400, y0 + 126)]);
            if i > 0 {
                black.extend([(pl + 300, y0 - 2), (pl + 300, y0 - 1)]);
                white.extend([(pl + 300, y0 - 3), (pl + 300, y0)]);
            }
        }
        for (x, y) in black {
            assert_eq!(rgb_at(&pm, x, y), BLACK, "{file}, app: ({x}, {y})");
            assert_eq!(rgb_at(&want, x, y), BLACK, "{file}, image: ({x}, {y})");
        }
        for (x, y) in white {
            assert_eq!(rgb_at(&pm, x, y), WHITE, "{file}, app: ({x}, {y})");
            assert_eq!(rgb_at(&want, x, y), WHITE, "{file}, image: ({x}, {y})");
        }
    }
}

#[test]
fn the_checkpoints_panel_has_the_same_pixels_and_covers_nothing_in_each_layout() {
    // NAV-18: for each size of the list, the panel is at its x range and its y range. The contents have
    // the same order in the four layouts. Out of the panel, only the toolbar slot of the bookmark
    // changes: the panel does not cover a toolbar button, the close button or the meter.
    use sudoku::app::Menu;
    let fonts = fonts();
    let mut empty = reference::checkpoints_game();
    empty.checkpoints.clear();
    for (game, n) in [(empty, 0), (reference::checkpoints_game(), 3), (reference::full_checkpoints_game(), 5)] {
        let bottom = ckp_bottom(n);
        let mut first: Option<Vec<[u8; 3]>> = None;
        for (layout, pl) in LAYOUTS.into_iter().zip(CKP_LEFT) {
            set_layout(layout);
            let at = format!("{layout:?}, {n} rows");
            // The meter is out of the panel: below it, or next to it. The meter starts 13 px above meter_y.
            let (my, mxl, mxr) = (sudoku::layout::meter_y(), sudoku::layout::meter_xl(), sudoku::layout::meter_xr());
            assert!(bottom + 13 < my || pl + 600 <= mxl || pl >= mxr, "{at}: the panel covers the meter at x {mxl}-{mxr}, y {my}");
            let mut app = with_panel(&fonts, game.clone());
            let open = draw_app(&fonts, &app);
            app.ui.menu = Menu::Open(None);
            let closed = draw_app(&fonts, &app);
            let tb = sudoku::layout::tb_x();
            let mut slots = std::collections::BTreeSet::new();
            for (x, y) in changed_pixels(&open, &closed) {
                if (pl..pl + 600).contains(&x) && (560..=bottom).contains(&y) {
                    continue;
                }
                assert!((tb..tb + 112).contains(&x), "{at}: the panel changes ({x}, {y})");
                slots.insert(y / 112);
            }
            assert_eq!(slots, std::collections::BTreeSet::from([5]), "{at}: the panel changes the toolbar slots");
            // The panel touches the toolbar: there is no space between its side line and the slot.
            let side = if pl == 112 { pl } else { pl + 599 };
            assert_eq!(rgb_at(&open, side, 616), BLACK, "{at}: the side line next to the toolbar");
            let mut inner = Vec::new();
            for y in 560..=bottom {
                for x in pl + 2..pl + 598 {
                    inner.push(rgb_at(&open, x, y));
                }
            }
            let first = first.get_or_insert_with(|| inner.clone());
            // The same contents at the same offsets. A glyph has a different float position in each
            // layout, thus an edge pixel of a glyph can change by one or two steps of the
            // antialiasing (16 of 255). An item that moves by 1 px gives a larger difference.
            let w = 596;
            let diff: Vec<(usize, usize, [u8; 3], [u8; 3])> = (0..inner.len())
                .filter(|&i| (0..3).any(|k| first[i][k].abs_diff(inner[i][k]) > 32))
                .map(|i| (i % w + 2, i / w + 560, first[i], inner[i]))
                .collect();
            assert!(diff.is_empty(), "{at}: {} pixels are not the same as in the first layout (offset x, y, first, this): {:?}", diff.len(), &diff[..diff.len().min(12)]);
        }
    }
    reset_layout();
}

#[test]
fn the_panel_with_no_checkpoint_shows_the_save_row_and_two_lines_of_text() {
    // CKP-7, CKP-1: the exact text at the positions of R§9.3, and no row.
    use sudoku::font::{Anchor, Weight};
    use sudoku::render::gray;
    let fonts = fonts();
    let mut game = reference::checkpoints_game();
    game.checkpoints.clear();
    for (layout, pl) in LAYOUTS.into_iter().zip(CKP_LEFT) {
        set_layout(layout);
        let pm = draw_app(&fonts, &with_panel(&fonts, game.clone()));
        let inside = pl + 2..pl + 598;
        assert_text(&fonts, &pm, (Weight::Medium, 28.0, BLACK), (pl + 100, 625), Anchor::Start, "Save checkpoint", pl + 90..pl + 598);
        assert_text(&fonts, &pm, (Weight::Regular, 28.0, gray(25)), (pl + 299, 760), Anchor::Middle, "No checkpoints", inside.clone());
        assert_text(&fonts, &pm, (Weight::Regular, 25.0, gray(40)), (pl + 299, 800), Anchor::Middle, "Save the board before a difficult guess.", inside);
    }
    reset_layout();
}

#[test]
fn the_rows_show_the_clock_and_the_quantity_of_all_filled_cells() {
    // CKP-7: the clock of the checkpoint (whole seconds, rounded down, in the format of CMP-4), and n
    // counts all filled cells, also the starting numbers. ENTERED has two starting cells: these do not
    // count two times.
    use sudoku::font::{Anchor, Weight};
    use sudoku::render::gray;
    let fonts = fonts();
    let mut game = reference::full_checkpoints_game();
    game.checkpoints[4].elapsed_ms += 999;
    let rows = [("20:14", 41), ("19:26", 46), ("18:32", 40), ("11:07", 36), ("4:15", 32)];
    for (layout, pl) in LAYOUTS.into_iter().zip(CKP_LEFT) {
        set_layout(layout);
        let pm = draw_app(&fonts, &with_panel(&fonts, game.clone()));
        for (i, (clock, cells)) in rows.into_iter().enumerate() {
            let y0 = ckp_row_top(i as i32);
            let style = (Weight::Regular, 25.0, gray(25));
            let area = pl + 173..pl + 381;
            assert_text(&fonts, &pm, style, (pl + 190, y0 + 81), Anchor::Start, &format!("{clock} played"), area.clone());
            assert_text(&fonts, &pm, style, (pl + 190, y0 + 115), Anchor::Start, &format!("{cells} of 81 cells"), area);
        }
    }
    reset_layout();
}

#[test]
fn a_clock_with_two_digits_for_the_hours_does_not_touch_the_button() {
    // R§9.3, CKP-7: the first and the last clock with two digits for the hours, and a clock with wide
    // digits. The pixel before the button stays white, and the border of the button does not change.
    let fonts = fonts();
    for (layout, pl) in LAYOUTS.into_iter().zip(CKP_LEFT) {
        set_layout(layout);
        for seconds in [36_000u64, 359_999, 288_888] {
            let mut game = reference::checkpoints_game();
            game.elapsed_ms = 400_000 * 1000;
            game.checkpoints[0].elapsed_ms = seconds * 1000;
            let pm = draw_app(&fonts, &with_panel(&fonts, game));
            let y0 = ckp_row_top(0);
            for y in y0 + 55..=y0 + 90 {
                assert_eq!(rgb_at(&pm, pl + 380, y), WHITE, "{layout:?}, {seconds} s: the pixel before the button, y {y}");
                assert_eq!(rgb_at(&pm, pl + 381, y), BLACK, "{layout:?}, {seconds} s: the border of the button, y {y}");
            }
        }
    }
    reset_layout();
}

#[test]
fn the_small_board_has_the_colors_of_the_rules_of_the_board() {
    // CKP-7: the first of the clash color, the completed color (shows_completed), the starting color
    // and white. The values of the checkpoint select it, not the values of the game. The rows of
    // full_checkpoints_game: 0 has the clash of (5,1) and (2,1), 1 has complete box 0, column 4 and
    // row 7, 2 has complete box 0.
    use sudoku::app::App;
    let fonts = fonts();
    let draw = |change: fn(&mut App)| {
        let mut a = with_panel(&fonts, reference::full_checkpoints_game());
        change(&mut a);
        draw_app(&fonts, &a)
    };
    for (layout, pl) in LAYOUTS.into_iter().zip(CKP_LEFT) {
        set_layout(layout);
        let (starting, clash, completed) = (Color::Gray.rgb(), Color::Red.rgb(), Color::Green.rgb());
        let pm = draw(|_| {});
        let row = |i| (pl, i);
        // The clash color is first, also on a starting cell. A box with a clash is not complete.
        assert_eq!(small_bg(&pm, row(0), (5, 1)), clash, "{layout:?}: a number with a clash");
        assert_eq!(small_bg(&pm, row(0), (2, 1)), clash, "{layout:?}: a starting number with a clash");
        assert_eq!(small_bg(&pm, row(0), (0, 0)), starting, "{layout:?}: a starting number of a box with a clash");
        assert_eq!(small_bg(&pm, row(0), (0, 2)), WHITE, "{layout:?}: a number of a box with a clash");
        // The completed color is on top of the starting color. The game itself has a clash there.
        for i in [1, 2] {
            assert_eq!(small_bg(&pm, row(i), (2, 1)), completed, "{layout:?}: row {i}: a starting number of box 0");
            assert_eq!(small_bg(&pm, row(i), (0, 2)), completed, "{layout:?}: row {i}: a number of box 0");
        }
        assert_eq!(small_bg(&pm, row(1), (7, 0)), completed, "{layout:?}: a number of the complete row 7");
        assert_eq!(small_bg(&pm, row(2), (7, 0)), WHITE, "{layout:?}: row 7 is not complete in this checkpoint");
        assert_eq!(small_bg(&pm, row(2), (3, 0)), starting, "{layout:?}: a starting number");
        assert_eq!(small_bg(&pm, row(2), (3, 1)), WHITE, "{layout:?}: a number");
        assert_eq!(small_bg(&pm, row(2), (3, 3)), WHITE, "{layout:?}: an empty cell");

        // The settings of the colors and the switch (COL-11, COL-12).
        let pm = draw(|a| a.settings.color_clash = Color::Orange);
        assert_eq!(small_bg(&pm, row(0), (5, 1)), Color::Orange.rgb(), "{layout:?}: the clash color follows the setting");
        let pm = draw(|a| a.settings.color_starting = Color::Yellow);
        assert_eq!(small_bg(&pm, row(2), (3, 0)), Color::Yellow.rgb(), "{layout:?}: the starting color follows the setting");
        let pm = draw(|a| a.settings.color_completed = Color::Blue);
        assert_eq!(small_bg(&pm, row(2), (0, 2)), Color::Blue.rgb(), "{layout:?}: the completed color follows the setting");
        for change in [(|a: &mut App| a.settings.completed_color_on = false) as fn(&mut App), |a| a.settings.color_completed = Color::White] {
            let pm = draw(change);
            assert_eq!(small_bg(&pm, row(2), (2, 1)), starting, "{layout:?}: no completed color on a starting number");
            assert_eq!(small_bg(&pm, row(2), (0, 2)), WHITE, "{layout:?}: no completed color on a number");
        }

        // No highlight: the panel does not change when the game has a highlighted digit.
        let plain = draw(|_| {});
        let high = draw(|a| a.ui.highlight = Some(5));
        assert!(plain.data() != high.data(), "{layout:?}: the highlight does not show on the board");
        for (x, y) in changed_pixels(&plain, &high) {
            assert!(!(pl..pl + 600).contains(&x) || !(560..=1572).contains(&y), "{layout:?}: the highlight changes ({x}, {y}) in the panel");
        }
    }
    reset_layout();
}

#[test]
fn the_digits_of_the_small_board_have_the_color_of_the_board() {
    // CKP-7, BRD-8: black on a starting cell and #1E1E1E on a number of the player, at the center of
    // the cell and on the baseline of R§9.3.
    use sudoku::font::{Anchor, Weight};
    use sudoku::render::Canvas;
    let fonts = fonts();
    let pm = draw_app(&fonts, &with_panel(&fonts, reference::full_checkpoints_game()));
    let player = sudoku::settings::ENTRY_DIGIT;
    let cases = [((3, 0), "8", BLACK, Color::Gray.rgb()), ((3, 1), "5", player, WHITE), ((5, 1), "9", player, Color::Red.rgb())];
    for (cell, digit, color, bg) in cases {
        let (x, y) = small_cell((908, 0), cell);
        let mut want = tiny_skia::Pixmap::new(pm.width(), pm.height()).unwrap();
        let mut canvas = Canvas::new(&mut want, &fonts);
        canvas.rect(x as f32, y as f32, (x + 16) as f32, (y + 16) as f32, bg);
        canvas.text(Weight::Regular, 14.0, x as f32 + 8.5, (y + 14) as f32, Anchor::Middle, digit, color);
        for py in y + 1..y + 16 {
            for px in x + 1..x + 16 {
                assert_eq!(rgb_at(&pm, px, py), rgb_at(&want, px, py), "the digit {digit} of {cell:?} at ({px}, {py})");
            }
        }
    }
}

#[test]
fn the_small_board_shows_a_dot_for_each_visible_mark() {
    // CKP-7, MRK-4: a dot of 2x2 px, gray 40, at the keypad position, with the current setting. Only 3
    // is free in (8, 0). The other digits have a peer: their marks show only with automatic hiding off.
    use sudoku::render::gray;
    let fonts = fonts();
    let mut game = reference::sample_game();
    for d in 1..=9 {
        game.toggle_mark_with((8, 0), d, false);
    }
    assert!(game.save_checkpoint());
    for (layout, pl) in LAYOUTS.into_iter().zip(CKP_LEFT) {
        set_layout(layout);
        for hide in [true, false] {
            let mut app = with_panel(&fonts, game.clone());
            app.settings.auto_hide_on = hide;
            let pm = draw_app(&fonts, &app);
            let (x, y) = small_cell((pl, 0), (8, 0));
            for d in 1..=9 {
                let (dx, dy) = (x + 3 + 5 * ((d - 1) % 3), y + 3 + 5 * ((d - 1) / 3));
                let want = if !hide || d == 3 { gray(40) } else { WHITE };
                for (px, py) in [(dx, dy), (dx + 1, dy), (dx, dy + 1), (dx + 1, dy + 1)] {
                    assert_eq!(rgb_at(&pm, px, py), want, "{layout:?}, hide {hide}: the mark {d} at ({px}, {py})");
                }
                // The dot is 2x2 px: the pixels around it are white.
                for (px, py) in [(dx - 1, dy), (dx + 2, dy), (dx, dy - 1), (dx, dy + 2)] {
                    assert_eq!(rgb_at(&pm, px, py), WHITE, "{layout:?}, hide {hide}: around the mark {d} at ({px}, {py})");
                }
            }
        }
    }
    reset_layout();
}

#[test]
fn the_small_board_has_its_lines_at_the_exact_pixels() {
    // R§9.3, CKP-7, scripts/board-state.py: thin lines of 1 px, gray 60, on the first pixel of the cell.
    // Black lines of 2 px at the box borders and the outside. The outer box is 155 px: portrait
    // x 925-1079, y y0+13 to y0+167.
    use sudoku::render::gray;
    let fonts = fonts();
    for (layout, pl) in LAYOUTS.into_iter().zip(CKP_LEFT) {
        set_layout(layout);
        let pm = draw_app(&fonts, &with_panel(&fonts, reference::checkpoints_game()));
        for i in 0..3 {
            let (bx, by) = (pl + 18, ckp_row_top(i) + 14);
            for a in [1, 2, 4, 5, 7, 8].map(|k| k * 17) {
                assert_eq!(rgb_at(&pm, bx + a, by + 1), gray(60), "{layout:?}: thin vertical line {a}");
                assert_eq!(rgb_at(&pm, bx + 1, by + a), gray(60), "{layout:?}: thin horizontal line {a}");
            }
            for a in [0, 3, 6, 9].map(|k| k * 17) {
                for d in [-1, 0] {
                    assert_eq!(rgb_at(&pm, bx + a + d, by + 1), BLACK, "{layout:?}: black vertical line {a}");
                    assert_eq!(rgb_at(&pm, bx + 1, by + a + d), BLACK, "{layout:?}: black horizontal line {a}");
                }
            }
            for (x, y) in [(bx - 2, by + 8), (bx + 154, by + 8), (bx + 8, by - 2), (bx + 8, by + 154)] {
                assert_eq!(rgb_at(&pm, x, y), WHITE, "{layout:?}: out of the outer box at ({x}, {y})");
            }
        }
    }
    reset_layout();
}

#[test]
fn the_save_row_and_the_buttons_follow_the_board_of_the_game() {
    // CKP-1, CKP-2, CKP-7: the plus and the text are black exactly when can_save. A button is "Current"
    // (black) exactly when the checkpoint has the same numbers and marks, also the hidden marks. The
    // limits: a duplicate with a short list, a full list with no duplicate, and a difference only in a
    // hidden mark.
    use sudoku::font::{Anchor, Weight};
    use sudoku::render::gray;
    let fonts = fonts();

    let mut duplicate = reference::sample_game();
    assert!(duplicate.save_checkpoint());

    let mut four = reference::checkpoints_game();
    assert!(four.save_checkpoint());
    four.toggle_mark((8, 0), 3); // The game now differs from each row.

    let mut full = reference::full_checkpoints_game();
    full.checkpoints[0].board.marks[8][0] = sudoku::game::mark_bit(3);

    let mut hidden = reference::sample_game();
    hidden.toggle_mark_with((8, 0), 1, false); // A peer has a 1, thus this mark does not show.
    assert!(hidden.save_checkpoint());
    hidden.toggle_mark_with((8, 0), 1, false);

    let cases: [(&str, sudoku::game::Game, bool, &[bool]); 4] = [
        ("one row with the same board", duplicate, false, &[true]),
        ("four rows and a different board", four, true, &[false, false, false, false]),
        ("five rows and a different board", full, false, &[false, false, false, false, false]),
        ("a difference only in a hidden mark", hidden, true, &[false]),
    ];
    for (name, game, can_save, current) in cases {
        for (layout, pl) in LAYOUTS.into_iter().zip(CKP_LEFT) {
            set_layout(layout);
            let at = format!("{name}, {layout:?}");
            let pm = draw_app(&fonts, &with_panel(&fonts, game.clone()));
            let save = if can_save { BLACK } else { gray(70) };
            assert_eq!(game.can_save_checkpoint(), can_save, "{at}: can_save");
            assert_eq!(rgb_at(&pm, pl + 58, 616), save, "{at}: the plus");
            assert_text(&fonts, &pm, (Weight::Medium, 28.0, save), (pl + 100, 625), Anchor::Start, "Save checkpoint", pl + 90..pl + 598);
            for (i, &current) in current.iter().enumerate() {
                let y0 = ckp_row_top(i as i32);
                let (inner, text, label) = if current { (BLACK, WHITE, "Current") } else { (WHITE, BLACK, "Restore") };
                // The label on the color of the button, at the center of the button.
                let mut want = tiny_skia::Pixmap::new(pm.width(), pm.height()).unwrap();
                let mut canvas = sudoku::render::Canvas::new(&mut want, &fonts);
                canvas.rect((pl + 383) as f32, (y0 + 57) as f32, (pl + 519) as f32, (y0 + 123) as f32, inner);
                canvas.text(Weight::Medium, 25.0, (pl + 451) as f32, (y0 + 99) as f32, Anchor::Middle, label, text);
                for y in y0 + 57..=y0 + 123 {
                    for x in pl + 383..=pl + 519 {
                        assert_eq!(rgb_at(&pm, x, y), rgb_at(&want, x, y), "{at}: the label {label} at ({x}, {y})");
                    }
                }
                // The bin has its center at x 560, and the space before its tap area is white.
                let bin = (pl + 546..pl + 575).any(|x| rgb_at(&pm, x, y0 + 90) != WHITE);
                assert!(bin, "{at}: no bin in row {i}");
                for x in pl + 522..pl + 533 {
                    assert_eq!(rgb_at(&pm, x, y0 + 90), WHITE, "{at}: the space before the bin at x {x}");
                }
            }
        }
    }
    reset_layout();
}

#[test]
fn the_bookmark_has_the_color_of_the_state_of_the_game() {
    // NAV-8, CKP-9: black on a game in progress, white on a black slot while its panel is open, gray 70
    // on a complete game, also when a different panel is open. The shape is the closed line of R§9.
    use sudoku::app::{Menu, Panel};
    use sudoku::render::gray;
    let fonts = fonts();
    for layout in LAYOUTS {
        set_layout(layout);
        let tb = sudoku::layout::tb_x();
        let (cx, cy) = (tb + 56, 616);
        let cases = [
            ("game", reference::sample_game(), Menu::Open(None), BLACK, WHITE),
            ("game, other panel", reference::sample_game(), Menu::Open(Some(Panel::Settings)), BLACK, WHITE),
            ("game, its panel", reference::sample_game(), Menu::Open(Some(Panel::Checkpoints)), WHITE, BLACK),
            ("complete", reference::completed_game(), Menu::Open(None), gray(70), WHITE),
            ("complete, other panel", reference::completed_game(), Menu::Open(Some(Panel::Colors)), gray(70), WHITE),
        ];
        for (name, game, menu, icon, slot) in cases {
            let mut app = reference::app(&fonts);
            app.game = game;
            app.ui.menu = menu;
            let pm = draw_app(&fonts, &app);
            // Points on the line: the top side and the two long sides. Points out of it: the center,
            // the space below the notch and the corners of the slot.
            for (x, y) in [(cx, cy - 18), (cx - 12, cy), (cx + 12, cy)] {
                assert_eq!(rgb_at(&pm, x, y), icon, "{layout:?}, {name}: the line at ({x}, {y})");
            }
            for (x, y) in [(cx, cy), (cx, cy + 14), (tb + 4, 564), (tb + 107, 667)] {
                assert_eq!(rgb_at(&pm, x, y), slot, "{layout:?}, {name}: the slot at ({x}, {y})");
            }
        }
    }
    reset_layout();
}

#[test]
fn the_icon_sheet_has_the_three_looks_of_the_bookmark() {
    // CKP-9, NAV-8: the sheet has the bookmark in black, in white on black and in gray 70. This test
    // does not use images/icons.png.
    use reference::IconLook;
    use sudoku::render::{gray, Icon};
    let fonts = fonts();
    let tiles = reference::icon_tiles();
    let bookmark: Vec<(usize, IconLook)> =
        tiles.iter().enumerate().filter(|(_, t)| t.0 == Icon::Bookmark).map(|(i, t)| (i, t.1)).collect();
    let looks: Vec<IconLook> = bookmark.iter().map(|b| b.1).collect();
    assert_eq!(looks, [IconLook::Normal, IconLook::Active, IconLook::Unavailable(70)]);
    let sheet = reference::icon_sheet(&fonts);
    for (i, look) in bookmark {
        let i = i as u32;
        let cx = (i % reference::ICON_COLUMNS * reference::ICON_TILE_W + reference::ICON_TILE_W / 2) as i32;
        let cy = (i / reference::ICON_COLUMNS * reference::ICON_TILE_H + reference::ICON_CENTER_Y) as i32;
        let (icon, background) = match look {
            IconLook::Normal => (BLACK, WHITE),
            IconLook::Active => (WHITE, BLACK),
            IconLook::Unavailable(n) => (gray(n), WHITE),
        };
        assert_eq!(rgb_at(&sheet, cx, cy - 18), icon, "{look:?}: the line of the icon");
        assert_eq!(rgb_at(&sheet, cx, cy), background, "{look:?}: the center of the icon");
    }
}

#[test]
fn a_retained_frame_follows_each_change_of_the_checkpoints_panel() {
    // CKP-1, CKP-2, CKP-3, CKP-9, NAV-8: each change of the list, of the board and of the settings shows
    // on a retained frame. The first two steps change only the bookmark in the toolbar overlay: undo
    // and redo are gray in the two games.
    use sudoku::app::{App, Menu, Panel};
    use sudoku::game::Game;
    use sudoku::render::{draw, Canvas, FrameRecord};
    use tiny_skia::Pixmap;
    let fonts = fonts();
    fn open(a: &mut App) {
        a.ui.menu = Menu::Open(Some(Panel::Checkpoints));
    }
    let steps: [(&str, fn(&mut App)); 17] = [
        ("no history", |a| {
            a.game = Game::new(1, 2, [[1; 9]; 9], [[0; 9]; 9]);
            a.ui.menu = Menu::Open(None)
        }),
        ("complete", |a| a.game = reference::completed_game()),
        ("the game", |a| a.game = reference::sample_game()),
        ("open", open),
        ("save", |a| assert!(a.game.save_checkpoint())),
        ("a mark", |a| {
            a.game.toggle_mark((8, 0), 3);
        }),
        ("save again", |a| assert!(a.game.save_checkpoint())),
        ("restore the older row", |a| {
            a.game.restore_checkpoint(1);
        }),
        ("undo", |a| {
            a.game.undo();
        }),
        ("redo", |a| {
            a.game.redo();
        }),
        ("no hiding", |a| a.settings.auto_hide_on = false),
        ("a color", |a| a.settings.color_clash = Color::Orange),
        ("no completed color", |a| a.settings.completed_color_on = false),
        ("delete the first row", |a| assert!(a.game.delete_checkpoint(0))),
        ("delete the last row", |a| assert!(a.game.delete_checkpoint(0))),
        ("close", |a| a.ui.menu = Menu::Open(None)),
        ("the full list", |a| {
            a.game = reference::full_checkpoints_game();
            open(a)
        }),
    ];
    for layout in LAYOUTS {
        set_layout(layout);
        let (w, h) = (sudoku::layout::screen_w(), sudoku::layout::screen_h());
        let mut pm = Pixmap::new(w, h).unwrap();
        let mut record = FrameRecord::default();
        let mut app = reference::app(&fonts);
        for (name, step) in steps {
            step(&mut app);
            draw(&mut Canvas::retained(&mut pm, &mut record, &fonts), &app);
            let full = draw_app(&fonts, &app);
            assert!(pm.data() == full.data(), "{layout:?}, {name}: the retained frame is not the same as a full drawing");
        }
    }
    reset_layout();
}

#[test]
fn the_cached_rows_of_the_checkpoints_panel_are_the_same_as_a_new_drawing() {
    // CKP-7, R§9.3: the panel keeps the pixels of each row (put_box). The key must have all that the
    // pixels depend on: each change here must show and must be the same as a drawing with no cache (a
    // new thread). A deleted row moves the next rows to a new position.
    use sudoku::app::{App, Menu, Panel};
    let fonts = fonts();
    fn base(a: &mut App) {
        a.game = reference::full_checkpoints_game();
        a.ui.menu = Menu::Open(Some(Panel::Checkpoints));
    }
    // A mark of row 1 that does not show: a peer has the digit in that checkpoint.
    fn hidden(a: &mut App) {
        base(a);
        a.game.checkpoints[1].board.marks[8][0] = sudoku::game::mark_bit(1);
    }
    let changes: [(&str, reference::Setup, reference::Setup); 11] = [
        ("the clock", base, |a| {
            base(a);
            a.game.checkpoints[1].elapsed_ms -= 1000
        }),
        ("a number", base, |a| {
            base(a);
            a.game.checkpoints[1].board.value[8][0] = 3
        }),
        ("a mark", base, |a| {
            base(a);
            a.game.checkpoints[1].board.marks[8][0] = sudoku::game::mark_bit(3)
        }),
        ("Current", base, |a| {
            base(a);
            a.game.toggle_mark((8, 0), 3);
        }),
        ("a starting cell", base, |a| {
            base(a);
            a.game.given[0][0] = false
        }),
        ("the starting color", base, |a| {
            base(a);
            a.settings.color_starting = Color::Yellow
        }),
        ("the clash color", base, |a| {
            base(a);
            a.settings.color_clash = Color::Orange
        }),
        ("the completed color", base, |a| {
            base(a);
            a.settings.color_completed = Color::Blue
        }),
        ("the completed switch", base, |a| {
            base(a);
            a.settings.completed_color_on = false
        }),
        ("automatic hiding", hidden, |a| {
            hidden(a);
            a.settings.auto_hide_on = false
        }),
        ("a delete", base, |a| {
            base(a);
            a.game.checkpoints.remove(1);
        }),
    ];
    for layout in LAYOUTS {
        set_layout(layout);
        for (name, before, change) in changes {
            let first = draw_setup(&fonts, before);
            let cached = draw_setup(&fonts, change);
            assert!(first.data() != cached.data(), "{layout:?}, {name}: the change does not show");
            let fresh = std::thread::scope(|s| {
                s.spawn(|| {
                    set_layout(layout);
                    draw_setup(&fonts, change)
                })
                .join()
                .unwrap()
            });
            assert!(cached.data() == fresh.data(), "{layout:?}, {name}: the cached rows are not the same as a new drawing");
        }
    }
    reset_layout();
}
