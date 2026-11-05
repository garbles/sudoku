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
    presenter.reset();
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
