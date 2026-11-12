//! The reMarkable Paper Pro Move (REQUIREMENTS.md §1, owner decisions 2026-11-10). The device is a
//! value of the process (`layout::set_device`), thus this file is a separate test program: each test
//! sets the Move first. The other test programs use the Paper Pro.
//!
//! The owner has no Move. These tests examine the layout rules and compare the app with the design
//! images of the Move (`images/move/`, `DEVICE=move sh scripts/build.sh screens`).

mod common;

use std::path::Path;

use common::fonts;
use sudoku::layout::{self, Device, Model, Orientation, Rect};
use sudoku::reference;
use sudoku::render::decode_png;

fn move_device() {
    assert!(layout::set_device(Device::MOVE), "the device of this test program is the Move");
}

/// The four layouts: the orientation and the hand.
const LAYOUTS: [(Orientation, bool); 4] =
    [(Orientation::Portrait, false), (Orientation::Portrait, true), (Orientation::Landscape, false), (Orientation::Landscape, true)];

fn for_each_layout(f: impl Fn(&str)) {
    for (o, rh) in LAYOUTS {
        layout::set_orientation(o);
        layout::set_right_handed(rh);
        f(&format!("{o:?}, right-handed {rh}"));
    }
    layout::set_orientation(Orientation::Portrait);
    layout::set_right_handed(false);
}

fn inside(r: Rect, outer: Rect) -> bool {
    r.x0 >= outer.x0 && r.y0 >= outer.y0 && r.x1 <= outer.x1 && r.y1 <= outer.y1
}

fn screen() -> Rect {
    Rect::new(0, 0, layout::screen_w() as i32, layout::screen_h() as i32)
}

fn toolbar() -> Rect {
    Rect::new(layout::tb_x(), 0, layout::tb_x() + layout::tb_slot(), layout::screen_h() as i32)
}

#[test]
fn the_device_comes_from_the_code_name() {
    // NAV-21, R§1: Chiappa is the Move, Ferrari and an unknown name are the Paper Pro.
    assert_eq!(Model::detect("reMarkable Chiappa\n").device(), Device::MOVE);
    assert_eq!(Model::detect("reMarkable Ferrari\n").device(), Device::PAPER_PRO);
    assert_eq!(Model::detect("").device(), Device::PAPER_PRO);
}

#[test]
fn the_move_has_the_values_of_the_owner_decisions() {
    // NAV-21, R§1.1, owner decisions 2026-11-10: 954 x 1696, qtfb type 4, cells of 78 px, slots of 96 px.
    move_device();
    let d = layout::device();
    assert_eq!((d.physical_w, d.physical_h, d.fb_type, d.cell, d.tb_slot), (954, 1696, 4, 78, 96));
    assert!((layout::page_scale() * 1404.0 - 954.0).abs() < 0.01, "the page is 1404 units wide");
    // The contents of a cell come from the 147 px reference cell (layout::Device::scale).
    assert_eq!((d.digit_size(), d.digit_baseline(), d.mark_size()), (55.0, 59.0, 16.0));
    assert_eq!((d.mark_pos(0), d.mark_pos(1), d.mark_pos(2)), (13.0, 39.0, 65.0));
    // BRD-11, owner request 2026-11-11: the border and the box lines are 7 px, the cell lines 3 px, on
    // their own pixels: 9 cells of 78 px and the lines make a board of 748 px.
    assert_eq!((d.line_width(0), d.line_width(1), layout::board_size()), (7, 3, 748));
}

#[test]
fn the_portrait_pages_fill_the_taller_screen() {
    // R§1.1 (owner decision 2026-11-10), ANA-6, LIC-2: in portrait the page is 624 units taller than on
    // the Paper Pro, and the lists and the "By difficulty" table use that height. Landscape does not change.
    move_device();
    let counts = || (layout::list_rows(), layout::licenses_rows(), layout::license_lines(), layout::summary_row());
    assert_eq!(counts(), (18, 20, 57, 186.0));
    // The last row of each list is above the page arrows (102 units above the bottom of the 2496 units).
    assert!(layout::list_row(layout::list_rows()) <= 2496.0 - 102.0);
    assert!(layout::licenses_row(layout::licenses_rows()) <= 2496.0 - 102.0);
    assert!(layout::license_line(layout::license_lines() - 1) <= 2496.0 - 102.0);
    layout::set_orientation(Orientation::Landscape);
    assert_eq!(counts(), (6, 9, 27, 76.0));
    layout::set_orientation(Orientation::Portrait);
}

#[test]
fn the_board_keeps_its_physical_position_when_the_tablet_turns() {
    // NAV-21: the board is at the center of the screen in all layouts, thus the
    // same physical pixels show it in portrait and in landscape.
    move_device();
    let physical = |o: Orientation| {
        layout::set_orientation(o);
        let (x, y, b) = (layout::board_x(), layout::board_y(), layout::board_size());
        layout::physical_rect((x, y, x + b - 1, y + b - 1))
    };
    let portrait = physical(Orientation::Portrait);
    assert_eq!(physical(Orientation::Landscape), portrait, "Landscape");
    layout::set_orientation(Orientation::Portrait);
    assert_eq!((layout::board_x(), layout::board_y(), layout::board_size()), (103, 474, 748));
}

#[test]
fn the_toolbar_the_panels_and_the_card_fit_in_all_layouts() {
    // NAV-14, NAV-17, NAV-18, NAV-21, CMP-7, owner decisions 2026-11-10.
    move_device();
    for_each_layout(|case| {
        let slot = layout::tb_slot();
        let h = layout::screen_h() as i32;
        // The top group (6 slots) and the bottom group (3 slots) do not touch.
        assert!(6 * slot < layout::tb_bottom_y() - 2 * slot, "{case}: the two groups of the toolbar");
        // The panels are on the screen and do not cover the toolbar.
        let panels = [
            ("Cell colors", layout::colors_panel()),
            ("Settings", layout::settings_panel(true)),
            ("Checkpoints", layout::ckp_panel(sudoku::game::MAX_CHECKPOINTS)),
            ("New game", layout::ng_panel()),
        ];
        for (name, p) in panels {
            assert!(inside(p, screen()), "{case}: {name} {p:?} is not on the screen");
            assert!(!p.overlaps(&toolbar()), "{case}: {name} {p:?} covers the toolbar");
        }
        // The meter is below the board and on the screen.
        assert!(layout::meter_y() + 13 < h, "{case}: the meter");
        // The result card does not touch the board, the toolbar or the close button (CMP-7), and its
        // three buttons are in the card and do not touch.
        let card = layout::result_card();
        let board = Rect::new(layout::board_x(), layout::board_y(), layout::board_x() + layout::board_size(), layout::board_y() + layout::board_size());
        // The board does not go under the toolbar (owner request 2026-11-10: cells of 80 px).
        assert!(!board.overlaps(&toolbar()), "{case}: the board {board:?} is under the toolbar");
        assert!(inside(card, screen()), "{case}: card {card:?}");
        assert!(!card.overlaps(&board) && !card.overlaps(&toolbar()), "{case}: card {card:?}");
        let buttons = layout::LevelButton::ALL.map(layout::level_button);
        for (i, b) in buttons.iter().enumerate() {
            assert!(inside(*b, card), "{case}: button {i} {b:?} is not in the card {card:?}");
            for other in &buttons[i + 1..] {
                assert!(!b.overlaps(other), "{case}: buttons {b:?} and {other:?}");
            }
        }
    });
}

#[test]
fn the_landscape_panels_have_the_move_shape() {
    // NAV-17, NAV-18, R§1.1: in landscape, the Cell colors panel has three columns of two rows
    // (1200 x 720 px from slot 1), and the Checkpoints panel starts at slot 1.
    move_device();
    layout::set_orientation(Orientation::Landscape);
    assert_eq!(layout::color_columns(), 3);
    assert_eq!(layout::colors_panel(), Rect::new(1600 - 1200, 96, 1600, 96 + 720 + 1));
    assert_eq!(layout::ckp_top(), 96);
    layout::set_orientation(Orientation::Portrait);
    assert_eq!(layout::color_columns(), 2);
    assert_eq!(layout::colors_panel(), Rect::new(858 - 800, 96, 858, 96 + 1080 + 1));
    assert_eq!(layout::ckp_top(), 480, "in portrait the panel starts at the bookmark (slot 5)");
}

#[test]
fn each_reference_state_obeys_the_rules_in_all_layouts() {
    // All rules of INVARIANTS.md that the app checks (invariants::check_app), on the Move.
    move_device();
    reference::use_utc();
    let fonts = fonts();
    for_each_layout(|case| {
        for (name, setup) in reference::states() {
            let mut app = reference::app(&fonts);
            setup(&mut app);
            // Some images show an empty board: it is not a puzzle, and only the images use it.
            if sudoku::game::cells().all(|c| !app.game.is_given(c)) {
                continue;
            }
            sudoku::invariants::check_app(&app).unwrap_or_else(|e| panic!("{name} ({case}): {e}"));
        }
    });
}

/// Compares the app with an image of `images/move/`: the mean difference, the part of the pixels
/// that differ by more than 64, and the pixels that differ away from all edges (refer to
/// tests/render.rs). The small text of the pages (the page scale of the Move, and the longer lists of a
/// portrait page) has more edges, thus the limits of the mean and of the large differences are higher.
fn assert_agrees(pm: &tiny_skia::Pixmap, file: &str) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let want = decode_png(&std::fs::read(reference::image_path(root, file)).unwrap());
    assert_eq!((pm.width(), pm.height()), (want.width(), want.height()), "{file}: the size");
    let (mean, big) = reference::compare(pm, &want);
    assert!(mean < 3.5 && big < 0.02, "{file}: mean difference {mean:.2}, {:.3}% of pixels differ", big * 100.0);
    let off2 = reference::differences_off_edges(pm, &want, 2);
    assert!(off2.len() <= 2, "{file}: pixels that differ away from all edges: {:?}", &off2[..off2.len().min(10)]);
}

#[test]
fn screens_agree_with_the_move_images() {
    // R§17: the app agrees with the design images of the Move, in portrait and in landscape.
    move_device();
    reference::use_utc();
    let fonts = fonts();
    for (name, setup) in reference::states() {
        assert_agrees(&reference::render(&fonts, setup), name);
    }
    layout::set_orientation(Orientation::Landscape);
    let landscape = ["color-menu.png", "checkpoints-menu.png"]
        .into_iter()
        .chain(reference::states().into_iter().map(|(n, _)| n).filter(|n| n.starts_with("analytics") || n.starts_with("licenses")));
    for name in landscape {
        let setup = reference::states().into_iter().find(|s| s.0 == name).expect("reference state").1;
        assert_agrees(&reference::render(&fonts, setup), &name.replace(".png", "-landscape.png"));
    }
    layout::set_orientation(Orientation::Portrait);
}

#[test]
fn the_manual_shows_the_move_pages() {
    // R§12: the app shows the manual pages of the Move, pixel for pixel, in the two orientations.
    move_device();
    let fonts = fonts();
    let names = ["how-to-play", "candidates", "pen", "pen-rules", "helpers", "mistakes", "menu", "new-game", "finished"];
    for (o, suffix) in [(Orientation::Portrait, ""), (Orientation::Landscape, "-landscape")] {
        layout::set_orientation(o);
        for (i, name) in names.iter().enumerate() {
            let mut app = reference::app(&fonts);
            app.screen = sudoku::app::Screen::Manual;
            app.manual_page = i as u8 + 1;
            let mut pm = tiny_skia::Pixmap::new(layout::screen_w(), layout::screen_h()).unwrap();
            sudoku::render::draw(&mut sudoku::render::Canvas::new(&mut pm, &fonts), &app);
            let file = format!("manual-{name}{suffix}.png");
            let want = decode_png(&std::fs::read(reference::image_path(Path::new(env!("CARGO_MANIFEST_DIR")), &file)).unwrap());
            assert_eq!(reference::compare(&pm, &want).1, 0.0, "{file}");
        }
    }
    layout::set_orientation(Orientation::Portrait);
}
