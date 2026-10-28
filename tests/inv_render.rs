//! Tests for the rules of INVARIANTS.md that the drawn screen must obey (package C, "Rendering"). Each
//! test makes the state where the rule can break and examines the pixels. `invariants::check_app` runs
//! only in debug builds, thus the tests assert each rule directly.

mod common;

use std::path::Path;

use common::*;
use sudoku::app::{App, Panel, Screen};
use sudoku::font::{Anchor, Fonts};
use sudoku::game::{cells, mark_bit, Cell};
use sudoku::generator::MODES;
use sudoku::layout::{self, Button, Rect, CELL};
use sudoku::reference;
use sudoku::render::{gray, rgb_at, Canvas, BLACK};
use sudoku::settings::{Color, Section};
use sudoku::store::ResultRow;
use tiny_skia::Pixmap;

// ---- Helpers (copies of the helpers in tests/rules.rs and tests/behavior.rs).

fn fonts() -> Fonts {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    Fonts::from_files(&root.join("scripts/fonts/rmsans-400.ttf"), &root.join("scripts/fonts/rmsans-500.ttf")).unwrap()
}

/// The screen of an app.
fn draw_app(app: &App, fonts: &Fonts) -> Pixmap {
    let mut pm = Pixmap::new(layout::screen_w(), layout::screen_h()).unwrap();
    sudoku::render::draw(&mut Canvas::new(&mut pm, fonts), app);
    pm
}

fn empty_cell(d: &Driver) -> Cell {
    cells().find(|&c| !d.app.game.is_filled(c)).unwrap()
}

fn color_point(section: usize, i: usize) -> (f32, f32) {
    let (gx, gy) = layout::color_cell(section, i);
    (gx as f32 + 56.0, gy as f32 + 40.0)
}

/// A white screen with only the text `s` on it, at the same position as the app draws it.
#[allow(clippy::too_many_arguments)]
fn text_only(fonts: &Fonts, page_units: bool, medium: bool, size: f32, x: f32, y: f32, anchor: Anchor, s: &str) -> Pixmap {
    let mut pm = Pixmap::new(layout::screen_w(), layout::screen_h()).unwrap();
    let mut c = Canvas::new(&mut pm, fonts);
    c.clear();
    c.set_page_units(page_units);
    c.text(medium, size, x, y, anchor, s, BLACK);
    pm
}

/// The first pixel in the area (x1 and y1 not included) that is different in the two screens.
fn first_difference(a: &Pixmap, b: &Pixmap, r: Rect) -> Option<(i32, i32)> {
    (r.y0..r.y1).flat_map(|y| (r.x0..r.x1).map(move |x| (x, y))).find(|&(x, y)| rgb_at(a, x, y) != rgb_at(b, x, y))
}

/// The darkest red value in an area (x1 and y1 not included).
fn darkest(pm: &Pixmap, r: Rect) -> u8 {
    (r.y0..r.y1).flat_map(|y| (r.x0..r.x1).map(move |x| (x, y))).map(|(x, y)| rgb_at(pm, x, y)[0]).min().unwrap()
}

/// The board and the history of a game (not the clock).
fn board(g: &sudoku::game::Game) -> sudoku::game::Game {
    let mut b = g.clone();
    b.elapsed_ms = 0;
    b
}

/// The area of the toolbar slot with its top at y.
fn slot(y: i32) -> Rect {
    Rect::new(layout::tb_x(), y, layout::screen_w() as i32, y + layout::TB_SLOT)
}

// ---- BRD-6, BRD-8, MRK-1, MRK-2: the board.

#[test]
fn only_empty_cells_show_the_micro_cell_grid() {
    // BRD-6: an empty cell shows the micro-cell grid (gray80). A filled cell does not. An empty cell
    // with the "?" does not show the grid.
    let fonts = fonts();
    let mut app = reference::app(&fonts);
    app.game = reference::sample_game();
    let question = cells().filter(|&c| !app.game.is_filled(c)).nth(3).unwrap();
    app.ui.question = Some((question, 0));
    let pm = draw_app(&app, &fonts);
    let grid = gray(80);
    for cell in cells() {
        let (x, y) = layout::cell_origin(cell);
        // Points on the two vertical and the two horizontal micro-cell lines, away from digits and marks.
        let points: Vec<(i32, i32)> = (1..=2)
            .flat_map(|k| {
                let o = layout::micro_offset(k);
                [(x + o, y + 20), (x + o, y + 135), (x + 20, y + o), (x + 135, y + o)]
            })
            .collect();
        let bg = rgb_at(&pm, x + 20, y + 20);
        if !app.game.is_filled(cell) && cell != question {
            for &(px, py) in &points {
                assert_eq!(rgb_at(&pm, px, py), grid, "empty cell {cell:?}: no grid line at ({px}, {py})");
            }
        } else {
            assert_ne!(bg, grid, "cell {cell:?}");
            for &(px, py) in &points {
                assert_eq!(rgb_at(&pm, px, py), bg, "filled cell or \"?\" {cell:?}: a grid line at ({px}, {py})");
            }
        }
    }
}

#[test]
fn starting_digits_are_black_and_entries_are_dark_gray() {
    // BRD-8: the darkest pixel of a starting digit is black. The darkest pixel of an entry is gray35.
    // The cell colors (also clash and highlight) do not change the digit color.
    let fonts = fonts();
    for (starting, highlight) in [(Color::Gray, None), (Color::Blue, Some(9)), (Color::White, Some(2))] {
        let mut app = reference::app(&fonts);
        app.game = reference::sample_game();
        app.settings.color_starting = starting;
        app.ui.highlight = highlight;
        let pm = draw_app(&app, &fonts);
        for cell in cells().filter(|&c| app.game.is_filled(c)) {
            let (x, y) = layout::cell_origin(cell);
            let d = darkest(&pm, Rect::new(x + 20, y + 20, x + CELL - 20, y + CELL - 20));
            let want = if app.game.is_given(cell) { 0 } else { gray(35)[0] };
            assert_eq!(d, want, "cell {cell:?}, given {}", app.game.is_given(cell));
        }
    }
    // An entry that the pen writes.
    let mut d = Driver::new(70);
    let cell = empty_cell(&d);
    d.write(cell, 6);
    assert!(d.app.game.is_entry(cell));
    let pm = draw_app(&d.app, &fonts);
    let (x, y) = layout::cell_origin(cell);
    assert_eq!(darkest(&pm, Rect::new(x + 20, y + 20, x + CELL - 20, y + CELL - 20)), gray(35)[0]);
}

#[test]
fn each_mark_is_at_its_keypad_position() {
    // MRK-1: a pen tap on micro-cell d adds only mark d. The mark shows only in micro-cell d (1 at the
    // top left, 9 at the bottom right), as the digit d.
    const MARK_POS: [f32; 3] = [24.0, 74.0, 122.0];
    let fonts = fonts();
    let mut d = Driver::new(71);
    let cell = empty_cell(&d);
    let (x, y) = layout::cell_origin(cell);
    let (xf, yf) = (x as f32, y as f32);
    for digit in 1..=9u8 {
        d.tap_micro(cell, digit);
        assert_eq!(d.app.game.marks_at(cell), mark_bit(digit), "the tap on micro-cell {digit}");
        let pm = draw_app(&d.app, &fonts);
        // The same digit, drawn alone at the keypad position.
        let i = (digit - 1) as usize;
        let mut want = Pixmap::new(layout::screen_w(), layout::screen_h()).unwrap();
        let mut c = Canvas::new(&mut want, &fonts);
        c.clear();
        c.text(false, 31.0, xf + MARK_POS[i % 3], yf + MARK_POS[i / 3] + 11.0, Anchor::Middle, &digit.to_string(), gray(15));
        // Each micro-cell without its edges (the board lines and the micro-cell lines).
        for m in 0..9 {
            let (mx, my) = (m % 3, m / 3);
            let r = Rect::new(
                x + layout::micro_offset(mx) + 8,
                y + layout::micro_offset(my) + 8,
                x + layout::micro_offset(mx + 1) - 8,
                y + layout::micro_offset(my + 1) - 8,
            );
            if m == i as i32 {
                assert!(darkest(&pm, r) < 100, "mark {digit} does not show in its micro-cell");
            }
            assert_eq!(first_difference(&pm, &want, r), None, "mark {digit}, micro-cell {}", m + 1);
        }
        d.tap_micro(cell, digit);
        assert_eq!(d.app.game.marks_at(cell), 0);
    }
}

#[test]
fn the_marks_of_a_filled_cell_are_hidden() {
    // MRK-2, BRD-6: a filled cell can have marks, but they do not show. The eraser makes them show.
    let fonts = fonts();
    // All entries of the sample game get marks: the screen does not change.
    let mut app = reference::app(&fonts);
    app.game = reference::sample_game();
    let without = draw_app(&app, &fonts);
    let entries: Vec<Cell> = cells().filter(|&c| app.game.is_entry(c)).collect();
    for cell in entries {
        app.game.marks[cell.0 as usize][cell.1 as usize] = (1..=9u8).fold(0, |m, v| m | mark_bit(v));
    }
    assert!(draw_app(&app, &fonts).data() == without.data(), "the marks of entries show");

    // The pen puts marks in a cell, then writes a number in it.
    let mut d = Driver::new(72);
    let cell = empty_cell(&d);
    for digit in [1, 5, 9] {
        d.tap_micro(cell, digit);
    }
    let marks = d.app.game.marks_at(cell);
    let with_marks = draw_app(&d.app, &fonts);
    let value = (1..=9u8).find(|v| marks & mark_bit(*v) == 0).unwrap();
    d.write(cell, value);
    assert_eq!(d.app.game.value_at(cell), value);
    assert_eq!(d.app.game.marks_at(cell), marks, "the cell keeps its marks");
    let filled = draw_app(&d.app, &fonts);
    let (x, y) = layout::cell_origin(cell);
    let mut no_marks = Driver::from(new_app(72));
    no_marks.app.game = d.app.game.clone();
    no_marks.app.game.marks[cell.0 as usize][cell.1 as usize] = 0;
    let filled_no_marks = draw_app(&no_marks.app, &fonts);
    assert!(filled.data() == filled_no_marks.data(), "the marks of a filled cell show");
    // The eraser removes the number. The marks show again at the same pixels as before.
    d.erase(cell);
    assert_eq!(d.app.game.value_at(cell), 0);
    let erased = draw_app(&d.app, &fonts);
    assert_eq!(first_difference(&erased, &with_marks, Rect::new(x, y, x + CELL, y + CELL)), None);
}

// ---- UND-7, UND-8: undo and redo buttons.

#[test]
fn undo_and_redo_are_black_only_when_they_operate() {
    // UND-7: each button is black only if it can operate. A gray button has no effect.
    let fonts = fonts();
    let mut d = Driver::new(73);
    d.button(Button::Menu);
    let cell = empty_cell(&d);
    let check = |d: &mut Driver, undo: bool, redo: bool| {
        assert_eq!((d.app.game.can_undo(), d.app.game.can_redo()), (undo, redo));
        let pm = draw_app(&d.app, &fonts);
        for (k, on) in [(3, undo), (4, redo)] {
            let want = if on { 0 } else { gray(70)[0] };
            assert_eq!(darkest(&pm, slot(k * layout::TB_SLOT)), want, "button {k}, undo {undo}, redo {redo}");
        }
        for (b, on) in [(Button::Undo, undo), (Button::Redo, redo)] {
            if !on {
                let g = board(&d.app.game);
                d.button(b);
                assert_eq!(board(&d.app.game), g, "the gray button {b:?} changed the game");
            }
        }
    };
    check(&mut d, false, false);
    d.tap_micro(cell, 1);
    check(&mut d, true, false);
    d.tap_micro(cell, 2);
    d.button(Button::Undo);
    check(&mut d, true, true);
    d.button(Button::Undo);
    check(&mut d, false, true);
    d.button(Button::Redo);
    d.button(Button::Redo);
    check(&mut d, true, false);
}

#[test]
fn undo_and_redo_are_gray_on_a_complete_game() {
    // UND-8: when the game is complete, the two buttons are gray and have no effect, also when the
    // stacks are not empty.
    let fonts = fonts();
    let mut d = Driver::new(74);
    solve(&mut d);
    assert!(d.app.game.completed && !d.app.game.undo.is_empty());
    // The redo stack is not empty also: the eraser on the last entry. The app cannot make this state,
    // but the rule must hold.
    let entry = cells().filter(|&c| d.app.game.is_entry(c)).last().unwrap();
    let digit = d.app.game.value_at(entry);
    d.app.game.redo.push(sudoku::game::Change::Erase { cell: entry, digit });
    d.button(Button::Menu);
    assert!(d.app.ui.menu_open);
    let pm = draw_app(&d.app, &fonts);
    for k in [3, 4] {
        assert_eq!(darkest(&pm, slot(k * layout::TB_SLOT)), gray(70)[0], "button {k} is not gray");
    }
    let g = d.app.game.clone();
    d.button(Button::Undo);
    d.button(Button::Redo);
    assert_eq!(d.app.game, g, "a gray button changed the complete game");
}

// ---- CMP-4: "Completed in" and the meter.

#[test]
fn completed_in_replaces_the_meter() {
    // CMP-4: a complete game shows "Completed in" and the time (m:ss, h:mm:ss after one hour) in place
    // of the meter. A game that is not complete shows the meter. The player cannot change the meter.
    let fonts = fonts();
    let area = Rect::new(layout::board_x(), layout::meter_y() - 30, layout::board_x() + layout::BOARD_SIZE, layout::meter_y() + 30);
    let (cx, ty) = ((layout::board_x() + layout::BOARD_SIZE / 2) as f32, (layout::meter_y() + 9) as f32);
    for (seconds, text) in [(7, "0:07"), (23 * 60 + 41, "23:41"), (3599, "59:59"), (3600, "1:00:00"), (3725, "1:02:05"), (36_000 + 59, "10:00:59")] {
        let mut app = reference::app(&fonts);
        app.game = reference::completed_game();
        app.game.elapsed_ms = seconds * 1000 + 999;
        let pm = draw_app(&app, &fonts);
        let want = text_only(&fonts, false, true, 25.0, cx, ty, Anchor::Middle, &format!("Completed in {text}"));
        assert_eq!(first_difference(&pm, &want, area), None, "{seconds} s: not \"Completed in {text}\" alone");
    }

    // Not complete: the meter shows, not the text.
    let mut d = Driver::new(75);
    let pm = draw_app(&d.app, &fonts);
    let want = text_only(&fonts, false, true, 25.0, cx, ty, Anchor::Middle, "Completed in 0:01");
    assert!(first_difference(&pm, &want, area).is_some());
    let (we, wi) = (fonts.medium.width(25.0, "Easy"), fonts.medium.width(25.0, "Hard"));
    let stops = layout::slider_stops(layout::meter_xl(), layout::meter_xr(), we, wi);
    let knob = stops[d.app.game.mode as usize];
    assert!(rgb_at(&pm, knob.round() as i32, layout::meter_y() - 8)[0] < 100, "no knob at the mode");
    // Pen and finger taps on the meter do not change it.
    let mode = d.app.game.mode;
    for &x in &stops {
        d.finger_tap(x, layout::meter_y() as f32);
        d.pen_tap(x, layout::meter_y() as f32);
    }
    assert_eq!(d.app.game.mode, mode);
    let after = draw_app(&d.app, &fonts);
    assert_eq!(first_difference(&pm, &after, area), None, "the meter changed");
}

// ---- NAV-7, NAV-8: the toolbar.

/// The dark pixels in the inner part of the menu button: the dot. Returns its center relative to the
/// center of the ring.
fn menu_dot(pm: &Pixmap) -> (f32, f32) {
    let r = slot(0);
    let dark: Vec<(i32, i32)> =
        (r.y0..r.y1).flat_map(|y| (r.x0..r.x1).map(move |x| (x, y))).filter(|&(x, y)| rgb_at(pm, x, y)[0] < 128).collect();
    assert!(!dark.is_empty(), "no menu button");
    let (x0, x1) = (dark.iter().map(|p| p.0).min().unwrap(), dark.iter().map(|p| p.0).max().unwrap());
    let (y0, y1) = (dark.iter().map(|p| p.1).min().unwrap(), dark.iter().map(|p| p.1).max().unwrap());
    let (cx, cy, radius) = ((x0 + x1) as f32 / 2.0, (y0 + y1) as f32 / 2.0, (x1 - x0) as f32 / 2.0);
    let dot: Vec<(f32, f32)> = dark
        .iter()
        .map(|&(x, y)| (x as f32 - cx, y as f32 - cy))
        .filter(|&(dx, dy)| (dx * dx + dy * dy).sqrt() < radius * 0.6)
        .collect();
    assert!(!dot.is_empty(), "no dot in the menu button");
    let n = dot.len() as f32;
    (dot.iter().map(|p| p.0).sum::<f32>() / n, dot.iter().map(|p| p.1).sum::<f32>() / n)
}

#[test]
fn the_dot_of_the_menu_button_shows_the_menu_state() {
    // NAV-7: the dot is at the bottom left when the menu is closed, and at the top left when it is open.
    let fonts = fonts();
    let mut d = Driver::new(76);
    for open in [false, true, false, true] {
        assert_eq!(d.app.ui.menu_open, open);
        let (dx, dy) = menu_dot(&draw_app(&d.app, &fonts));
        assert!(dx < -2.0, "open {open}: the dot is not at the left ({dx})");
        if open {
            assert!(dy < -2.0, "the menu is open, but the dot is not at the top ({dy})");
        } else {
            assert!(dy > 2.0, "the menu is closed, but the dot is not at the bottom ({dy})");
        }
        d.button(Button::Menu);
    }
    // A panel does not change the dot.
    d.button(Button::Menu);
    d.button(Button::Colors);
    assert!(menu_dot(&draw_app(&d.app, &fonts)).1 < -2.0);
}

#[test]
fn only_the_button_of_the_open_panel_is_white_on_black() {
    // NAV-8: the button of the open panel is white on black. The other buttons are black on white.
    let fonts = fonts();
    let slots = [
        (Panel::Colors, layout::TB_SLOT),
        (Panel::Settings, 2 * layout::TB_SLOT),
        (Panel::None, 3 * layout::TB_SLOT),
        (Panel::None, 4 * layout::TB_SLOT),
        (Panel::None, layout::tb_bottom_y() - 2 * layout::TB_SLOT),
        (Panel::None, layout::tb_bottom_y() - layout::TB_SLOT),
        (Panel::NewGame, layout::tb_bottom_y()),
    ];
    let mut d = Driver::new(77);
    d.button(Button::Menu);
    for (button, panel) in [(None, Panel::None), (Some(Button::Colors), Panel::Colors), (Some(Button::Settings), Panel::Settings), (Some(Button::NewGame), Panel::NewGame), (Some(Button::NewGame), Panel::None)] {
        if let Some(b) = button {
            d.button(b);
        }
        assert_eq!(d.app.ui.panel, panel);
        let pm = draw_app(&d.app, &fonts);
        for &(p, y) in &slots {
            let s = slot(y);
            let corner = rgb_at(&pm, s.x0 + 4, s.y0 + 4);
            let active = p != Panel::None && p == panel;
            if active {
                assert_eq!(corner, [0, 0, 0], "panel {panel:?}: its button at y {y} is not black");
                let light = (s.y0..s.y1).flat_map(|y| (s.x0..s.x1).map(move |x| (x, y))).any(|(x, y)| rgb_at(&pm, x, y)[0] > 200);
                assert!(light, "panel {panel:?}: no white icon at y {y}");
            } else {
                assert_eq!(corner, [255, 255, 255], "panel {panel:?}: the button at y {y} is not white");
                // The icon is black, or gray70 for undo and redo when they cannot operate (UND-7).
                assert!(darkest(&pm, s) <= gray(70)[0], "panel {panel:?}: no icon at y {y}");
            }
        }
    }
}

// ---- NAV-15: the handedness.

/// The screen of an app for a player of one handedness.
fn draw_handed(app: &App, fonts: &Fonts, right_handed: bool) -> Pixmap {
    layout::set_right_handed(right_handed);
    let pm = draw_app(app, fonts);
    layout::set_right_handed(false);
    pm
}

#[test]
fn a_right_handed_player_has_the_toolbar_at_the_left() {
    // NAV-15: for a right-handed player the toolbar and its panels are at the left edge, with the same
    // icons, and the menu button and the close button are mirror images.
    let fonts = fonts();
    let mut d = Driver::new(79);
    for o in [layout::Orientation::Portrait, layout::Orientation::Landscape] {
        layout::set_orientation(o);
        let w = layout::screen_w() as i32;
        let shift = w - layout::TB_SLOT;
        for (button, panel) in [(Some(Button::Menu), Panel::None), (Some(Button::Colors), Panel::Colors), (Some(Button::Settings), Panel::Settings), (Some(Button::NewGame), Panel::NewGame)] {
            if let Some(b) = button {
                d.button(b);
            }
            assert_eq!(d.app.ui.panel, panel);
            let (left, right) = (draw_handed(&d.app, &fonts, false), draw_handed(&d.app, &fonts, true));
            let mirror = |x0: i32, y0: i32, x1: i32, y1: i32| (y0..y1).flat_map(move |y| (x0..x1).map(move |x| (x, y)));
            // The menu button and the close button: mirror images. The antialiasing of a curve is not
            // exactly symmetric, thus the edge pixels can be a small amount different.
            for (x, y) in mirror(0, 0, w, layout::TB_SLOT).filter(|&(x, _)| x < layout::TB_SLOT || x >= shift) {
                let (a, b) = (rgb_at(&right, x, y)[0], rgb_at(&left, w - 1 - x, y)[0]);
                assert!(a.abs_diff(b) <= 32, "{o:?} {panel:?}: top row at ({x}, {y}): {a} and {b}");
            }
            // The other toolbar slots: the same icons at the other edge.
            for (x, y) in mirror(0, layout::TB_SLOT, layout::TB_SLOT, layout::screen_h() as i32) {
                assert_eq!(rgb_at(&right, x, y), rgb_at(&left, x + shift, y), "{o:?} {panel:?}: toolbar at ({x}, {y})");
            }
            // The panel touches the toolbar and has the same contents.
            let p = match panel {
                Panel::Colors => Some(layout::colors_panel()),
                Panel::Settings => Some(layout::settings_panel(false)),
                Panel::NewGame => Some(layout::ng_panel()),
                Panel::None => None,
            };
            if let Some(p) = p {
                let pw = p.x1 - p.x0;
                for (x, y) in mirror(layout::TB_SLOT, p.y0, layout::TB_SLOT + pw, p.y1.min(layout::screen_h() as i32)) {
                    assert_eq!(rgb_at(&right, x, y), rgb_at(&left, x - layout::TB_SLOT + p.x0, y), "{o:?} {panel:?}: panel at ({x}, {y})");
                }
            }
        }
        // The first tap only closes the panel (NAV-12).
        d.button(Button::Menu);
        d.button(Button::Menu);
        assert!(!d.app.ui.menu_open);
    }
    layout::set_orientation(layout::Orientation::Portrait);
}

// ---- COL-5: the colors that another section uses.

#[test]
fn a_color_of_a_different_section_is_not_available() {
    // COL-5: a color that a different section uses shows as not available (a gray slash) and a tap on it
    // has no effect. White is always available. All three sections can be White.
    let fonts = fonts();
    let mut d = Driver::new(78);
    d.button(Button::Menu);
    d.button(Button::Colors);
    let taken = |d: &Driver, s: usize, color: Color| {
        color != Color::White && Section::ALL.iter().enumerate().any(|(o, &sec)| o != s && d.app.settings.color(sec) == color)
    };
    let slash = gray(50);
    // A tap on each color of each section, from the default settings.
    for (s, &section) in Section::ALL.iter().enumerate() {
        for (i, &color) in Color::ALL.iter().enumerate() {
            let pm = draw_app(&d.app, &fonts);
            for (s2, _) in Section::ALL.iter().enumerate() {
                for (i2, &c2) in Color::ALL.iter().enumerate() {
                    let (gx, gy) = layout::color_cell(s2, i2);
                    let center = rgb_at(&pm, gx + 56, gy + 37);
                    assert_eq!(center == slash, taken(&d, s2, c2), "section {s2}, color {c2:?}: the slash");
                }
            }
            let before = d.app.settings.clone();
            let is_taken = taken(&d, s, color);
            let (x, y) = color_point(s, i);
            d.finger_tap(x, y);
            assert_eq!(d.app.ui.panel, Panel::Colors);
            if is_taken {
                assert_eq!(d.app.settings, before, "a tap on {color:?} (not available) in section {s}");
            } else {
                assert_eq!(d.app.settings.color(section), color, "a tap on {color:?} in section {s}");
            }
            let c = [d.app.settings.color_starting, d.app.settings.color_clash, d.app.settings.color_highlight];
            assert!((0..3).all(|a| (a + 1..3).all(|b| c[a] != c[b] || c[a] == Color::White)), "{c:?}");
        }
    }
    // All three White: no color is unavailable.
    let white = Color::ALL.iter().position(|&c| c == Color::White).unwrap();
    for s in 0..3 {
        let (x, y) = color_point(s, white);
        d.finger_tap(x, y);
    }
    let s = &d.app.settings;
    assert_eq!((s.color_starting, s.color_clash, s.color_highlight), (Color::White, Color::White, Color::White));
    let pm = draw_app(&d.app, &fonts);
    for s in 0..3 {
        for i in 0..9 {
            let (gx, gy) = layout::color_cell(s, i);
            assert_ne!(rgb_at(&pm, gx + 56, gy + 37), slash, "section {s}, color {i}");
        }
    }
}

// ---- ANA-3, ANA-4: the summary.

/// Examines the summary of `results` against the expected text: the total, the total average, and the
/// quantity and the average of each mode.
fn check_summary(fonts: &Fonts, pm: &Pixmap, total: &str, average: &str, modes: &[(String, String); MODES as usize]) {
    let bw = layout::summary_box_w();
    for (x0, value) in [(72.0, total), (100.0 + bw, average)] {
        let want = text_only(fonts, true, true, 84.0, x0 + 30.0, 405.0, Anchor::Start, value);
        let area = layout::page_rect(x0 + 10.0, 320.0, x0 + bw - 10.0, 432.0);
        assert_eq!(first_difference(pm, &want, area), None, "the box at {x0} does not show only \"{value}\"");
    }
    let (count_x, right) = (layout::summary_count_x(), layout::page_right());
    let mut y = layout::SUMMARY_TABLE_Y;
    for (mode, (count, avg)) in modes.iter().enumerate() {
        let row = layout::summary_row();
        let base = y + (row / 2.0).floor() + 12.0;
        let want = text_only(fonts, true, false, 34.0, count_x, base, Anchor::End, count);
        let area = layout::page_rect(600.0, y + 5.0, count_x + 10.0, y + row - 5.0);
        assert_eq!(first_difference(pm, &want, area), None, "mode {mode}: the quantity is not \"{count}\"");
        let want = text_only(fonts, true, false, 34.0, right, base, Anchor::End, avg);
        let area = layout::page_rect(count_x + 50.0, y + 5.0, right + 10.0, y + row - 5.0);
        assert_eq!(first_difference(pm, &want, area), None, "mode {mode}: the average is not \"{avg}\"");
        y += row;
    }
}

#[test]
fn the_summary_without_results_shows_zero_and_a_dash() {
    // ANA-3, ANA-4: with no results, "Puzzles completed" is 0, and each average is "–".
    let fonts = fonts();
    let mut d = Driver::new(79);
    d.button(Button::Menu);
    d.button(Button::Statistics);
    assert_eq!(d.app.screen, Screen::Statistics);
    assert!(d.app.results.is_empty());
    let pm = draw_app(&d.app, &fonts);
    let none = || ("0".to_string(), "–".to_string());
    check_summary(&fonts, &pm, "0", "–", &std::array::from_fn(|_| none()));
}

#[test]
fn the_summary_shows_the_quantity_and_the_average_of_each_mode() {
    // ANA-3, ANA-4: the total is |results|; each mode shows its quantity and the average of its
    // seconds, or "–" if it has no results. The quantities of the modes add up to the total.
    let fonts = fonts();
    let row = |id: i64, mode: u8, seconds: u64| ResultRow { id, finished_at: 1_790_000_000 + id, mode, givens: 30, seconds, seed: 0 };
    let mut app = reference::app(&fonts);
    app.screen = Screen::Statistics;
    // Mode 1: 100 s and 300 s (average 3:20). Mode 3: 4000 s (1:06:40). The total average is
    // (100 + 300 + 4000) / 3 = 1466.67 s, thus 24:27. Mode 6: 300 s.
    app.results = vec![row(1, 1, 100), row(2, 3, 4000), row(3, 1, 300), row(4, 6, 300)];
    let counts: usize = (0..MODES).map(|m| app.results.iter().filter(|r| r.mode == m).count()).sum();
    assert_eq!(counts, app.results.len());
    let pm = draw_app(&app, &fonts);
    let s = |a: &str, b: &str| (a.to_string(), b.to_string());
    // The total average is (100 + 300 + 4000 + 300) / 4 = 1175 s, thus 19:35.
    let none = || s("0", "–");
    check_summary(&fonts, &pm, "4", "19:35", &[none(), s("2", "3:20"), none(), s("1", "1:06:40"), none(), none(), s("1", "5:00")]);
}

// ---- INP-10: no ink stays on the board.

/// An app that never got ink, with the same game and the same screen state as `d`.
fn without_ink(d: &Driver) -> App {
    let mut a = new_app(1);
    a.game = d.app.game.clone();
    a.settings = d.app.settings.clone();
    a.ui.menu_open = d.app.ui.menu_open;
    a.ui.panel = d.app.ui.panel;
    a.ui.highlight = d.app.ui.highlight;
    a.ui.question = d.app.ui.question;
    a.ui.slider = d.app.ui.slider;
    a
}

fn assert_no_ink(d: &Driver, fonts: &Fonts, what: &str) {
    assert!(d.app.ui.pending_ink.is_none() && d.app.live_stroke.is_none(), "{what}: ink in the state");
    assert!(draw_app(&d.app, fonts).data() == draw_app(&without_ink(d), fonts).data(), "{what}: ink on the screen");
}

#[test]
fn no_ink_stays_on_the_board() {
    // INP-10 (the screen): after the app reads the ink, removes it or shows the "?", the screen is the
    // same as the screen of an app that never got ink.
    let fonts = fonts();
    let mut d = Driver::new(80);
    let cell = empty_cell(&d);

    // While the ink waits, it shows.
    d.write_strokes(&digit_ink(cell, 7));
    assert!(d.app.ui.pending_ink.is_some());
    assert!(draw_app(&d.app, &fonts).data() != draw_app(&without_ink(&d), &fonts).data(), "the waiting ink does not show");
    // The app reads a number.
    d.wait(d.app.pause_ms + 50);
    assert_eq!(d.app.game.value_at(cell), 7);
    assert_no_ink(&d, &fonts, "a number");

    // Writing on a number: the app removes the ink.
    d.write(cell, 3);
    assert_eq!(d.app.game.value_at(cell), 7);
    assert_no_ink(&d, &fonts, "writing on a number");

    // Writing that the app cannot read: the "?" shows, without the ink.
    let other = empty_cell(&d);
    d.write_strokes(&scribble_ink(other));
    d.wait(d.app.pause_ms + 50);
    assert_eq!(d.app.ui.question.map(|q| q.0), Some(other));
    assert_no_ink(&d, &fonts, "the \"?\"");
    // After the "?".
    d.wait(1100);
    assert!(d.app.ui.question.is_none());
    assert_no_ink(&d, &fonts, "after the \"?\"");

    // The menu is open: the same rules.
    d.button(Button::Menu);
    let third = empty_cell(&d);
    d.write(third, 2);
    assert_eq!(d.app.game.value_at(third), 2);
    assert_no_ink(&d, &fonts, "a number with the open menu");
}
