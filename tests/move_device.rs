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

/// The slot of a button of the toolbar: the full width of the column and the height of a slot.
fn slot(b: layout::Button) -> Rect {
    let y = layout::slot_top(b);
    Rect::new(layout::tb_x(), y, layout::tb_x() + layout::tb_slot(), y + layout::slot_height())
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
    // NAV-14, NAV-17, NAV-18, NAV-21, HNT-11, CMP-7, owner decisions 2026-11-10 and 2026-11-24.
    move_device();
    for_each_layout(|case| {
        let slot = layout::slot_height();
        let h = layout::screen_h() as i32;
        // The top group (7 slots) and the bottom group (3 slots) do not touch.
        assert_eq!(layout::Button::TOP.len(), 7, "{case}");
        let top_end = layout::slot_top(layout::Button::TOP[6]) + slot;
        let bottom_start = layout::slot_top(layout::Button::Help);
        assert!(top_end < bottom_start, "{case}: the two groups of the toolbar ({top_end}, {bottom_start})");
        assert_eq!(layout::slot_top(layout::Button::NewGame) + slot, h, "{case}: the + is at the bottom of the screen");
        // The panels are on the screen and do not cover the toolbar, at each size.
        use sudoku::game::HintState;
        let panels = [
            ("Hint", layout::hint_panel(HintState::Available)),
            ("Hint (two lines)", layout::hint_panel(HintState::Incorrect)),
            ("Cell colors", layout::colors_panel()),
            ("Settings", layout::settings_panel(false)),
            ("Settings (update row)", layout::settings_panel(true)),
            ("Checkpoints (empty)", layout::ckp_panel(0)),
            ("Checkpoints", layout::ckp_panel(sudoku::game::MAX_CHECKPOINTS)),
            ("New game", layout::ng_panel()),
        ];
        for (name, p) in panels {
            assert!(inside(p, screen()), "{case}: {name} {p:?} is not on the screen");
            assert!(!p.overlaps(&toolbar()), "{case}: {name} {p:?} covers the toolbar");
            // The panel touches the toolbar (NAV-21).
            let touches = if layout::right_handed() { p.x0 == layout::tb_x() + layout::tb_slot() } else { p.x1 == layout::tb_x() };
            assert!(touches, "{case}: {name} {p:?} does not touch the toolbar at x {}", layout::tb_x());
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
    // (1200 x 720 px from slot 2, y 190–910), and the Checkpoints panel starts at slot 1 (y 95). In
    // portrait the Cell colors panel is at y 192–1272 and the Checkpoints panel starts at slot 6 (y 576).
    move_device();
    layout::set_orientation(Orientation::Landscape);
    assert_eq!(layout::color_columns(), 3);
    assert_eq!(layout::colors_panel(), Rect::new(1600 - 1200, 190, 1600, 910 + 1));
    assert_eq!(layout::ckp_top(), 95);
    layout::set_orientation(Orientation::Portrait);
    assert_eq!(layout::color_columns(), 2);
    assert_eq!(layout::colors_panel(), Rect::new(858 - 800, 192, 858, 1272 + 1));
    assert_eq!(layout::ckp_top(), 576, "in portrait the panel starts at the bookmark (slot 6)");
}

#[test]
fn the_landscape_toolbar_has_slots_of_95_px() {
    // NAV-21, NAV-12, R§1.1 (owner decision 2026-11-24): on the Move in landscape each slot is 95 px
    // high, and the column stays 96 px wide. Top slot K starts at y 95 × K, and the bottom slots start
    // at y 669, 764 and 859. Portrait keeps the slots of 96 px. Each icon is at the center of its slot
    // (within 0.5 px), and a tap at the center of a slot operates its button.
    use layout::Button;
    move_device();
    let top = Button::TOP;
    assert_eq!(top, [Button::Menu, Button::Hint, Button::Colors, Button::Settings, Button::Undo, Button::Redo, Button::Checkpoints], "NAV-12");
    for_each_layout(|case| {
        let landscape = layout::orientation() == Orientation::Landscape;
        let height = if landscape { 95 } else { 96 };
        assert_eq!((layout::slot_height(), layout::tb_slot()), (height, 96), "{case}");
        for (k, &b) in top.iter().enumerate() {
            assert_eq!(layout::slot_top(b), height * k as i32, "{case}: {b:?}");
        }
        let bottom = if landscape { [669, 764, 859] } else { [1408, 1504, 1600] };
        for (b, y) in [Button::Help, Button::Statistics, Button::NewGame].into_iter().zip(bottom) {
            assert_eq!(layout::slot_top(b), y, "{case}: {b:?}");
        }
        for b in top.into_iter().chain([Button::Help, Button::Statistics, Button::NewGame]) {
            let s = slot(b);
            let (cx, cy) = layout::icon_center(b);
            let (mx, my) = ((s.x0 + s.x1) as f32 / 2.0, (s.y0 + s.y1) as f32 / 2.0);
            assert!((cx - mx).abs() <= 0.5 && (cy - my).abs() <= 0.5, "{case}: the icon of {b:?} at ({cx}, {cy}), the slot {s:?}");
            if b != Button::Menu {
                assert_eq!(layout::game_button(cx, cy, true), Some(b), "{case}: a tap on {b:?}");
            }
        }
    });
}

#[test]
fn a_landscape_panel_that_does_not_fit_below_its_button_starts_at_slot_1() {
    // NAV-21, NAV-14, NAV-18, HNT-11, R§1.1 (owner decision 2026-11-24): on the Move in landscape, the
    // Settings panel with the update row and the Checkpoints panel (at each quantity of checkpoints)
    // start at the top of slot 1 (y 95). The Settings panel with no update row starts at its slot (slot
    // 3, y 285). The Hint panel starts at slot 1 (y 95), the Cell colors panel at slot 2 (y 190). In
    // portrait each panel starts at the slot of its button.
    use layout::Button;
    move_device();
    for right_handed in [false, true] {
        layout::set_right_handed(right_handed);
        layout::set_orientation(Orientation::Landscape);
        let case = format!("landscape, right-handed {right_handed}");
        assert_eq!(layout::settings_panel(true).y0, 95, "NAV-14: {case}: with the update row");
        assert_eq!(layout::settings_panel(false).y0, 3 * 95, "NAV-14: {case}: with no update row");
        assert!(3 * 95 + layout::settings_panel(true).y1 - layout::settings_panel(true).y0 > 954, "{case}: the panel with the update row does not fit below slot 3");
        for n in 0..=sudoku::game::MAX_CHECKPOINTS {
            assert_eq!(layout::ckp_panel(n).y0, 95, "NAV-18: {case}: {n} checkpoints");
        }
        assert_eq!(layout::hint_panel(sudoku::game::HintState::Available).y0, 95, "HNT-11: {case}");
        assert_eq!(layout::colors_panel().y0, 190, "NAV-17: {case}");
        layout::set_orientation(Orientation::Portrait);
        let case = format!("portrait, right-handed {right_handed}");
        assert_eq!(layout::settings_panel(true).y0, layout::slot_top(Button::Settings), "NAV-14: {case}");
        assert_eq!(layout::settings_panel(true).y0, 288, "NAV-14: {case}");
        assert_eq!(layout::ckp_panel(4).y0, 576, "NAV-18: {case}");
        assert_eq!(layout::hint_panel(sudoku::game::HintState::Available).y0, 96, "HNT-11: {case}");
    }
    layout::set_right_handed(false);
}

#[test]
fn the_hint_panel_has_the_widths_of_the_paper_pro() {
    // HNT-11, R§1.1: on the Move the Hint panel is 500 px wide in portrait and 392 px wide in
    // landscape. It touches the toolbar. In landscape it is at x 1208–1599 for a left-handed player,
    // and it covers only a part (at most 7 px of the width) of the cells of rows 1 to 3 of one edge
    // column of the board. In portrait it covers no cell.
    use sudoku::game::{cells, HintState};
    move_device();
    for_each_layout(|case| {
        let landscape = layout::orientation() == Orientation::Landscape;
        for (state, h) in [(HintState::Available, 198), (HintState::Clash, 236), (HintState::Incorrect, 236)] {
            let p = layout::hint_panel(state);
            assert_eq!((p.x1 - p.x0, p.y1 - p.y0), (if landscape { 392 } else { 500 }, h), "{case}: {state:?}");
            if landscape && !layout::right_handed() {
                assert_eq!((p.x0, p.x1 - 1), (1208, 1599), "{case}: {state:?}");
            }
            for cell @ (r, c) in cells() {
                let (x, y) = layout::cell_origin(cell);
                let cr = Rect::new(x, y, x + layout::device().cell, y + layout::device().cell);
                if !cr.overlaps(&p) {
                    continue;
                }
                assert!(landscape, "{case}: {state:?}: the panel covers the cell {cell:?}");
                let edge = if layout::right_handed() { 0 } else { 8 };
                assert!(c == edge && r <= 2, "{case}: {state:?}: the panel covers the cell {cell:?}");
                let covered = cr.x1.min(p.x1) - cr.x0.max(p.x0);
                assert!(covered <= 7, "{case}: {state:?}: the panel covers {covered} px of the cell {cell:?}");
            }
        }
    });
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

#[test]
fn the_count_on_the_lightbulb_stays_in_its_slot() {
    // HNT-10, NAV-21: each count text, also "99+", stays 4 px or more inside slot 1 in all four layouts
    // of the Move (also in the slots of 95 px). A count above 99 shows "99+".
    move_device();
    let fonts = fonts();
    for_each_layout(|case| {
        let s = slot(layout::Button::Hint);
        let draw = |hints: u32| {
            let mut app = reference::app(&fonts);
            app.game = reference::hint_game(false);
            app.ui.menu = sudoku::app::Menu::Open(None);
            app.game.hints = hints;
            let mut pm = tiny_skia::Pixmap::new(layout::screen_w(), layout::screen_h()).unwrap();
            sudoku::render::draw(&mut sudoku::render::Canvas::new(&mut pm, &fonts), &app);
            pm
        };
        let zero = draw(0);
        let mut shapes = Vec::new();
        for hints in [1u32, 99, 100, 5000] {
            let pm = draw(hints);
            let w = pm.width() as usize;
            let changed: Vec<(i32, i32)> = pm
                .pixels()
                .iter()
                .zip(zero.pixels())
                .enumerate()
                .filter(|(_, (p, q))| p != q)
                .map(|(i, _)| ((i % w) as i32, (i / w) as i32))
                .collect();
            assert!(!changed.is_empty(), "{case}: no count for {hints}");
            for &(x, y) in &changed {
                assert!(x >= s.x0 + 4 && x < s.x1 - 4 && y >= s.y0 + 4 && y < s.y1 - 4, "{case}: {hints}: the pixel ({x}, {y}) is not 4 px inside the slot {s:?}");
            }
            shapes.push(changed.iter().map(|&(x, y)| (x - s.x0, y - s.y0)).collect::<Vec<_>>());
        }
        assert_eq!(shapes[2], shapes[3], "{case}: 100 and 5000 show the same text (99+)");
    });
}
