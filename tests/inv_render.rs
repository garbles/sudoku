//! Tests for the rules of INVARIANTS.md that the drawn screen must obey (package C, "Rendering"). Each
//! test makes the state where the rule can break and examines the pixels. `invariants::check_app` runs
//! only in debug builds, thus the tests assert each rule directly.

mod common;

use common::*;
use sudoku::app::{App, Input, Panel, Screen, Tool, OVERLAY_MS};
use sudoku::font::{Anchor, Fonts, Weight};
use sudoku::game::{cells, mark_bit, Cell, Game};
use sudoku::generator::MODES;
use sudoku::layout::{self, Button, Rect, CELL};
use sudoku::reference;
use sudoku::render::{gray, rgb_at, Canvas, Units, BLACK};
use sudoku::settings::{Color, LineGray, Section};
use sudoku::store::ResultRow;
use tiny_skia::Pixmap;

// ---- Helpers of this file. The shared helpers are in tests/common.

/// The screen of an app.
fn draw_app(app: &App, fonts: &Fonts) -> Pixmap {
    let mut pm = Pixmap::new(layout::screen_w(), layout::screen_h()).unwrap();
    sudoku::render::draw(&mut Canvas::new(&mut pm, fonts), app);
    pm
}

/// A white screen with only the text `s` on it, at the same position as the app draws it.
#[allow(clippy::too_many_arguments)]
fn text_only(fonts: &Fonts, units: Units, weight: Weight, size: f32, x: f32, y: f32, anchor: Anchor, s: &str) -> Pixmap {
    let mut pm = Pixmap::new(layout::screen_w(), layout::screen_h()).unwrap();
    let mut c = Canvas::new(&mut pm, fonts);
    c.clear();
    c.set_units(units);
    c.text(weight, size, x, y, anchor, s, BLACK);
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
fn board(g: &Game) -> Game {
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
    // BRD-6: an empty cell shows the micro-cell grid (the default gray, gray90). A filled cell does not. An empty cell
    // with the "?" does not show the grid.
    let fonts = fonts();
    let mut app = reference::app(&fonts);
    app.game = reference::sample_game();
    let question = cells().filter(|&c| !app.game.is_filled(c)).nth(3).unwrap();
    app.ui.question = Some((question, 0));
    let pm = draw_app(&app, &fonts);
    let grid = sudoku::render::line_rgb(sudoku::settings::LineGray::default()).unwrap();
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
fn the_micro_cell_lines_have_the_gray_of_the_setting() {
    // BRD-6, COL-7: the lines of an empty cell have the selected gray. With None, the cell has no lines.
    let fonts = fonts();
    for g in sudoku::settings::LineGray::ALL {
        let mut app = reference::app(&fonts);
        app.game = reference::sample_game();
        app.settings.micro_line = g;
        let pm = draw_app(&app, &fonts);
        let cell = cells().find(|&c| !app.game.is_filled(c) && app.game.marks_at(c) == 0).unwrap();
        let (x, y) = layout::cell_origin(cell);
        let o = layout::micro_offset(1);
        let expect = sudoku::render::line_rgb(g).unwrap_or(rgb_at(&pm, x + 8, y + 8));
        assert_eq!(rgb_at(&pm, x + o, y + 20), expect, "{g:?}");
        assert_eq!(rgb_at(&pm, x + 20, y + o), expect, "{g:?}");
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
    let cell = empty_cell(&d.app.game);
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
    // All nine digits: with the switch on, a digit of a peer would have no mark (MRK-7).
    d.app.settings.auto_hide_on = false;
    let cell = empty_cell(&d.app.game);
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
        c.text(Weight::Regular, 31.0, xf + MARK_POS[i % 3], yf + MARK_POS[i / 3] + 11.0, Anchor::Middle, &digit.to_string(), gray(15));
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

    // MRK-6: the pen puts marks in a cell, then writes a number in it. The fixture taps only digits
    // that no peer has: a refused tap adds no mark and shows an x (MRK-7, MRK-10), and the reference
    // picture must contain no x.
    let mut d = Driver::new(72);
    let free = |g: &sudoku::game::Game, c: Cell| (1..=9u8).filter(|&v| g.peer_digits(c) & mark_bit(v) == 0).collect::<Vec<_>>();
    let cell = cells().find(|&c| !d.app.game.is_filled(c) && free(&d.app.game, c).len() >= 2).expect("an empty cell with two free digits");
    let digits = free(&d.app.game, cell);
    for &digit in &digits {
        d.tap_micro(cell, digit);
        assert_eq!(d.app.ui.refused, None, "MRK-10: the digit {digit} is free, thus the tap is not refused");
    }
    let marks = d.app.game.marks_at(cell);
    assert_eq!(marks, digits.iter().fold(0, |m, &v| m | mark_bit(v)), "the cell has all the marks of the fixture");
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
    let cell = free_cell(&d.app, &[1, 2]);
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
    assert!(d.app.ui.menu.is_open());
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
        let want = text_only(&fonts, Units::Screen, Weight::Medium, 25.0, cx, ty, Anchor::Middle, &format!("Completed in {text}"));
        assert_eq!(first_difference(&pm, &want, area), None, "{seconds} s: not \"Completed in {text}\" alone");
    }

    // Not complete: the meter shows, not the text.
    let mut d = Driver::new(75);
    let pm = draw_app(&d.app, &fonts);
    let want = text_only(&fonts, Units::Screen, Weight::Medium, 25.0, cx, ty, Anchor::Middle, "Completed in 0:01");
    assert!(first_difference(&pm, &want, area).is_some());
    let (we, wh) = (fonts.get(Weight::Medium).width(25.0, "Easy"), fonts.get(Weight::Medium).width(25.0, "Hard"));
    let stops = layout::slider_stops(layout::meter_xl(), layout::meter_xr(), we, wh);
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
        assert_eq!(d.app.ui.menu.is_open(), open);
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
        (Some(Panel::Colors), layout::TB_SLOT),
        (Some(Panel::Settings), 2 * layout::TB_SLOT),
        (None, 3 * layout::TB_SLOT),
        (None, 4 * layout::TB_SLOT),
        (None, layout::tb_bottom_y() - 2 * layout::TB_SLOT),
        (None, layout::tb_bottom_y() - layout::TB_SLOT),
        (Some(Panel::NewGame), layout::tb_bottom_y()),
    ];
    let mut d = Driver::new(77);
    d.button(Button::Menu);
    for (button, panel) in [(None, None), (Some(Button::Colors), Some(Panel::Colors)), (Some(Button::Settings), Some(Panel::Settings)), (Some(Button::NewGame), Some(Panel::NewGame)), (Some(Button::NewGame), None)] {
        if let Some(b) = button {
            d.button(b);
        }
        assert_eq!(d.app.ui.menu.panel(), panel);
        let pm = draw_app(&d.app, &fonts);
        for &(p, y) in &slots {
            let s = slot(y);
            let corner = rgb_at(&pm, s.x0 + 4, s.y0 + 4);
            let active = p.is_some() && p == panel;
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
        for (button, panel) in [(Some(Button::Menu), None), (Some(Button::Colors), Some(Panel::Colors)), (Some(Button::Settings), Some(Panel::Settings)), (Some(Button::NewGame), Some(Panel::NewGame))] {
            if let Some(b) = button {
                d.button(b);
            }
            assert_eq!(d.app.ui.menu.panel(), panel);
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
                Some(Panel::Colors) => Some(layout::colors_panel()),
                Some(Panel::Settings) => Some(layout::settings_panel(false)),
                Some(Panel::NewGame) => Some(layout::ng_panel()),
                None => None,
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
        assert!(!d.app.ui.menu.is_open());
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
    let taken = |d: &Driver, s: Section, color: Color| {
        color != Color::White && Section::ALL.iter().any(|&o| o != s && d.app.settings.color(o) == color)
    };
    let slash = gray(50);
    // A tap on each color of each section, from the default settings.
    for section in Section::ALL {
        for color in Color::ALL {
            let pm = draw_app(&d.app, &fonts);
            for s2 in Section::ALL {
                for c2 in Color::ALL {
                    let cell = layout::color_cell(s2, c2);
                    let center = rgb_at(&pm, cell.x0 + 56, cell.y0 + 30);
                    assert_eq!(center == slash, taken(&d, s2, c2), "section {s2:?}, color {c2:?}: the slash");
                }
            }
            let before = d.app.settings.clone();
            let is_taken = taken(&d, section, color);
            let (x, y) = color_point(section, color);
            d.finger_tap(x, y);
            assert_eq!(d.app.ui.menu.panel(), Some(Panel::Colors));
            if is_taken {
                assert_eq!(d.app.settings, before, "a tap on {color:?} (not available) in section {section:?}");
            } else {
                assert_eq!(d.app.settings.color(section), color, "a tap on {color:?} in section {section:?}");
            }
            let c = [d.app.settings.color_starting, d.app.settings.color_clash, d.app.settings.color_highlight];
            assert!((0..3).all(|a| (a + 1..3).all(|b| c[a] != c[b] || c[a] == Color::White)), "{c:?}");
        }
    }
    // All three White: no color is unavailable.
    for s in Section::ALL {
        let (x, y) = color_point(s, Color::White);
        d.finger_tap(x, y);
    }
    let s = &d.app.settings;
    assert_eq!((s.color_starting, s.color_clash, s.color_highlight), (Color::White, Color::White, Color::White));
    let pm = draw_app(&d.app, &fonts);
    for s in Section::ALL {
        for c in Color::ALL {
            let cell = layout::color_cell(s, c);
            assert_ne!(rgb_at(&pm, cell.x0 + 56, cell.y0 + 30), slash, "section {s:?}, color {c:?}");
        }
    }
}

// ---- ANA-3, ANA-4: the summary.

/// Examines the summary of `results` against the expected text: the total, the total average, and the
/// quantity and the average of each mode.
fn check_summary(fonts: &Fonts, pm: &Pixmap, total: &str, average: &str, modes: &[(String, String); MODES as usize]) {
    let bw = layout::summary_box_w();
    for (x0, value) in [(72.0, total), (100.0 + bw, average)] {
        let want = text_only(fonts, Units::Page, Weight::Medium, 84.0, x0 + 30.0, 405.0, Anchor::Start, value);
        let area = layout::page_rect(x0 + 10.0, 320.0, x0 + bw - 10.0, 432.0);
        assert_eq!(first_difference(pm, &want, area), None, "the box at {x0} does not show only \"{value}\"");
    }
    let (count_x, right) = (layout::summary_count_x(), layout::page_right());
    let mut y = layout::SUMMARY_TABLE_Y;
    for (mode, (count, avg)) in modes.iter().enumerate() {
        let row = layout::summary_row();
        let base = y + (row / 2.0).floor() + 12.0;
        let want = text_only(fonts, Units::Page, Weight::Regular, 34.0, count_x, base, Anchor::End, count);
        let area = layout::page_rect(600.0, y + 5.0, count_x + 10.0, y + row - 5.0);
        assert_eq!(first_difference(pm, &want, area), None, "mode {mode}: the quantity is not \"{count}\"");
        let want = text_only(fonts, Units::Page, Weight::Regular, 34.0, right, base, Anchor::End, avg);
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
    a.ui.menu = d.app.ui.menu;
    a.ui.highlight = d.app.ui.highlight;
    a.ui.question = d.app.ui.question;
    a.ui.refused = d.app.ui.refused;
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
    let cell = empty_cell(&d.app.game);

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
    let other = empty_cell(&d.app.game);
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
    let third = empty_cell(&d.app.game);
    d.write(third, 2);
    assert_eq!(d.app.game.value_at(third), 2);
    assert_no_ink(&d, &fonts, "a number with the open menu");

    // INP-10, MRK-10: the x of a refused tap is not ink. The tap leaves no ink next to the x.
    let (cell, digit) = refused_spot(&d.app.game, |_, _| true);
    d.tap_micro(cell, digit);
    assert_eq!(d.app.ui.refused.map(|x| (x.0, x.1)), Some((cell, digit)));
    assert_no_ink(&d, &fonts, "the x");
}

// ---- PER-2, NAV-6: the first frame.

#[test]
fn the_first_frame_shows_the_open_toolbar_and_the_close_button() {
    // PER-2, NAV-1, NAV-6, NAV-15: the app opens with the menu open. The first frame of `App::new`
    // shows the close button and the open toolbar, for the left-handed and the right-handed layout.
    // With the menu closed, these pixels are white.
    let fonts = fonts();
    let app = App::new(None, None, sudoku::store::ResultsDb::in_memory(), Box::new(TestEnv { time: START_TIME, seed: 80 }), sudoku::render::ng_stops(&fonts));
    assert_eq!(app.ui.menu, sudoku::app::Menu::Open(None), "PER-2");
    let mut closed = App::new(None, None, sudoku::store::ResultsDb::in_memory(), Box::new(TestEnv { time: START_TIME, seed: 80 }), sudoku::render::ng_stops(&fonts));
    closed.ui.menu = sudoku::app::Menu::Closed;
    for right_handed in [false, true] {
        let (first, without_menu) = (draw_handed(&app, &fonts, right_handed), draw_handed(&closed, &fonts, right_handed));
        layout::set_right_handed(right_handed);
        // The center of the close button is the crossing of the two lines of the X.
        let (cx, cy) = (layout::close_cx() as i32, layout::CLOSE_CY as i32);
        // The gear (top slot 2) is a toolbar button that shows only in the open menu.
        let gear = Rect::new(layout::tb_x(), 2 * layout::TB_SLOT, layout::tb_x() + layout::TB_SLOT, 3 * layout::TB_SLOT);
        layout::set_right_handed(false);
        assert!(rgb_at(&first, cx, cy)[0] < 100, "NAV-6: the close button is not drawn (right-handed: {right_handed})");
        assert_eq!(rgb_at(&without_menu, cx, cy), [255, 255, 255], "NAV-6: the close button shows with the menu closed (right-handed: {right_handed})");
        assert!(darkest(&first, gear) < 100, "PER-2: the open toolbar is not drawn (right-handed: {right_handed})");
        assert_eq!(darkest(&without_menu, gear), 255, "the toolbar shows with the menu closed (right-handed: {right_handed})");
    }
}

// ---- MRK-13, MRK-14, MRK-16, BRD-6: the pixels of the x of a refused tap.

/// The offset of the anchor points of the pencil marks from the cell origin, on each axis (§15).
const X_ANCHOR: [i32; 3] = [24, 74, 122];
/// The half length of each line of the x on the two axes, and its line width (§15).
const X_HALF: f32 = 10.0;
const X_WIDTH: f32 = 3.0;
/// The half of the diagonal of a pixel. A pixel center nearer than this to the edge of a line can be
/// only a part of the line.
const PIXEL_HALF_DIAGONAL: f32 = 0.7072;

/// The four layouts of the game screen (the orientation and the hand of the player).
const LAYOUTS: [(layout::Orientation, bool); 4] = [
    (layout::Orientation::Portrait, false),
    (layout::Orientation::Portrait, true),
    (layout::Orientation::Landscape, false),
    (layout::Orientation::Landscape, true),
];

/// Sets a layout of this thread. When the value goes out of scope, it sets the default layout again.
struct InLayout;

impl InLayout {
    fn set(o: layout::Orientation, right_handed: bool) -> InLayout {
        layout::set_orientation(o);
        layout::set_right_handed(right_handed);
        InLayout
    }
}

impl Drop for InLayout {
    fn drop(&mut self) {
        layout::set_orientation(layout::Orientation::Portrait);
        layout::set_right_handed(false);
    }
}

/// The top left of the board in a layout (§15): the hand of the player does not move the board.
fn board_origin(o: layout::Orientation) -> (i32, i32) {
    match o {
        layout::Orientation::Portrait => (148, 418),
        layout::Orientation::Landscape => (418, 148),
    }
}

/// The numbers in the peers of a cell, from the board only (the same row, column or box).
fn peer_numbers(g: &Game, (r, c): Cell) -> u16 {
    cells()
        .filter(|&(pr, pc)| (pr, pc) != (r, c) && (pr == r || pc == c || (pr / 3 == r / 3 && pc / 3 == c / 3)))
        .map(|p| g.value_at(p))
        .filter(|&v| v != 0)
        .fold(0, |m, v| m | mark_bit(v))
}

/// The first empty cell and digit that make a refused tap (MRK-7), of those that `filter` accepts. The
/// row and the column of the cell are different, thus a swap of row and column shows in the pixels.
fn refused_spot(g: &Game, filter: impl Fn(Cell, u8) -> bool) -> (Cell, u8) {
    cells()
        .filter(|&c| c.0 != c.1 && !g.is_filled(c))
        .flat_map(|c| (1..=9u8).map(move |d| (c, d)))
        .find(|&(c, d)| peer_numbers(g, c) & mark_bit(d) != 0 && filter(c, d))
        .expect("a cell and a digit for a refused tap")
}

/// The center of the x of `digit`, from the values of §15: the keypad position (1 at the top left, 9
/// at the bottom right) from the origin of the cell (column on x, row on y).
fn x_center(origin: (i32, i32), (r, c): Cell, digit: u8) -> (f32, f32) {
    let i = digit as usize - 1;
    let (cell_x, cell_y) = (origin.0 + c as i32 * CELL, origin.1 + r as i32 * CELL);
    ((cell_x + X_ANCHOR[i % 3]) as f32, (cell_y + X_ANCHOR[i / 3]) as f32)
}

/// The distance from the point to the line from (x0, y0) to (x1, y1), the ends included.
fn to_segment(px: f32, py: f32, (x0, y0): (f32, f32), (x1, y1): (f32, f32)) -> f32 {
    let (dx, dy) = (x1 - x0, y1 - y0);
    let t = (((px - x0) * dx + (py - y0) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
    (px - x0 - t * dx).hypot(py - y0 - t * dy)
}

/// The distance from the center of the pixel (x, y) to the nearest line of the x. The coordinates of a
/// drawing are at pixel centers, as in the design images (ImageMagick).
fn to_x(x: i32, y: i32, (cx, cy): (f32, f32)) -> f32 {
    let (px, py) = (x as f32, y as f32);
    let h = X_HALF;
    to_segment(px, py, (cx - h, cy - h), (cx + h, cy + h)).min(to_segment(px, py, (cx - h, cy + h), (cx + h, cy - h)))
}

/// All pixels that are different in the two screens. The rows that are the same go fast.
fn differences(a: &Pixmap, b: &Pixmap) -> Vec<(i32, i32)> {
    assert_eq!((a.width(), a.height()), (b.width(), b.height()));
    let row = a.width() as usize * 4;
    let mut out = Vec::new();
    for (y, (ra, rb)) in a.data().chunks_exact(row).zip(b.data().chunks_exact(row)).enumerate() {
        if ra != rb {
            for (x, (pa, pb)) in ra.chunks_exact(4).zip(rb.chunks_exact(4)).enumerate() {
                if pa[..3] != pb[..3] {
                    out.push((x as i32, y as i32));
                }
            }
        }
    }
    out
}

/// MRK-13, §15: `with` is the screen `before` with one x at `center` and no other change. The pixels
/// come from the values of the documents: a full pixel of gray15 on the lines, no change away from the
/// lines, and a value between the two at the edge. A round end covers the full pixel at the end of
/// the line, a butt end covers only a part of it.
fn assert_only_x(fonts: &Fonts, before: &Pixmap, with: &Pixmap, center: (f32, f32), what: &str) {
    let (cx, cy) = (center.0 as i32, center.1 as i32);
    let radius = X_WIDTH / 2.0;
    let changed = differences(before, with);
    assert!(!changed.is_empty(), "{what}: the x does not show");
    for &(x, y) in &changed {
        assert!(
            to_x(x, y, center) < radius + PIXEL_HALF_DIAGONAL,
            "{what}: the pixel ({x}, {y}) changed, away from the x at ({cx}, {cy})"
        );
    }
    for y in cy - 14..cy + 14 {
        for x in cx - 14..cx + 14 {
            let (p, b) = (rgb_at(with, x, y), rgb_at(before, x, y));
            let d = to_x(x, y, center);
            if d <= radius - PIXEL_HALF_DIAGONAL {
                assert_eq!(p, gray(15), "{what}: the pixel ({x}, {y}) is on a line of the x");
            } else {
                assert!(p[0] >= gray(15)[0] && p[0] <= b[0] && p[0] == p[1] && p[1] == p[2], "{what}: the pixel ({x}, {y}) is {p:?} at the edge of the x, on {b:?}");
            }
        }
    }
    // The extent: 20 px between the line centers, and 1.5 px more at each end, thus 23 x 23 px (§15),
    // with its center at the center of the x.
    let (x0, x1) = (changed.iter().map(|p| p.0).min().unwrap(), changed.iter().map(|p| p.0).max().unwrap());
    let (y0, y1) = (changed.iter().map(|p| p.1).min().unwrap(), changed.iter().map(|p| p.1).max().unwrap());
    assert_eq!((x0 + x1, y0 + y1), (2 * cx, 2 * cy), "{what}: the center of the x");
    assert_eq!((x1 - x0 + 1, y1 - y0 + 1), (23, 23), "{what}: the extent of the x");
    // The same x from the drawing of two lines with round ends.
    let mut want = before.clone();
    let mut c = Canvas::new(&mut want, fonts);
    let h = X_HALF;
    let (fx, fy) = center;
    c.line(fx - h, fy - h, fx + h, fy + h, X_WIDTH, gray(15));
    c.line(fx - h, fy + h, fx + h, fy - h, X_WIDTH, gray(15));
    assert_eq!(differences(&want, with), vec![], "{what}: not two lines with round ends");
}

/// A cell with all its free digits as visible marks, for the digit `digit` of a refused tap. If
/// `hidden`, the cell also has the hidden mark `digit` (MRK-7: owner decision 2026-11-04).
fn cell_with_marks(d: &mut Driver, digit: u8, hidden: bool) -> Cell {
    let (cell, _) = refused_spot(&d.app.game, |_, v| v == digit);
    for v in 1..=9u8 {
        if peer_numbers(&d.app.game, cell) & mark_bit(v) == 0 && d.app.game.marks_at(cell) & mark_bit(v) == 0 {
            d.tap_micro(cell, v);
        }
    }
    if hidden && d.app.game.marks_at(cell) & mark_bit(digit) == 0 {
        d.app.settings.auto_hide_on = false;
        d.tap_micro(cell, digit);
        d.app.settings.auto_hide_on = true;
    }
    d.wait(OVERLAY_MS);
    assert_eq!(d.app.ui.refused, None, "the fixture has no x");
    assert_ne!(d.app.game.visible_marks(cell, true), 0, "the fixture has other visible marks");
    cell
}

/// The grid lines of a cell (BRD-6), at points away from the marks and from the x. The cell shows the
/// color `grid`, or no line if it is None.
fn assert_grid(pm: &Pixmap, cell: Cell, grid: Option<[u8; 3]>, what: &str) {
    let (x, y) = layout::cell_origin(cell);
    let bg = rgb_at(pm, x + 8, y + 8);
    for k in 1..=2 {
        let o = layout::micro_offset(k);
        for p in [8, 49 - 6, 49 + 6, 98 - 6, 98 + 6, 147 - 8] {
            for (px, py) in [(x + o, y + p), (x + p, y + o)] {
                assert_eq!(rgb_at(pm, px, py), grid.unwrap_or(bg), "{what}: the grid at ({px}, {py})");
            }
        }
    }
}

#[test]
fn the_x_changes_only_its_micro_cell_in_all_layouts() {
    // MRK-13, BRD-6, §15, MRK-1, MRK-7, MRK-10, MRK-14: all 9 digits in all layouts, with the
    // micro-cell lines of the default gray and set to None. The tap makes the x at the lift. All other
    // pixels of the screen stay the same: the other marks and the grid. The x does not show a hidden
    // mark. After the time of the x, the screen is the same as before.
    let fonts = fonts();
    for (o, right_handed) in LAYOUTS {
        let _layout = InLayout::set(o, right_handed);
        let mut d = Driver::new(300);
        for line in [LineGray::default(), LineGray::None] {
            d.app.settings.micro_line = line;
            let grid = sudoku::render::line_rgb(line);
            for digit in 1..=9u8 {
                let what = format!("{o:?}, right-handed {right_handed}, {line:?}, digit {digit}");
                let cell = cell_with_marks(&mut d, digit, digit % 2 == 0);
                assert_eq!(layout::cell_origin(cell), (board_origin(o).0 + cell.1 as i32 * CELL, board_origin(o).1 + cell.0 as i32 * CELL), "{what}");
                let center = x_center(board_origin(o), cell, digit);
                let before = draw_app(&d.app, &fonts);
                assert_grid(&before, cell, grid, &what);
                let game = d.app.game.clone();

                let (x, y) = micro_center(cell, digit);
                d.send(Input::PenDown { x, y, tool: Tool::Pen });
                assert_eq!(d.app.ui.refused, None, "MRK-10: {what}: an x at the pen down");
                d.app.redraw = false;
                d.send(Input::PenUp { x, y });
                let lift = d.now;
                assert_eq!(d.app.ui.refused, Some((cell, digit, lift)), "MRK-10: {what}");
                assert!(d.app.redraw, "MRK-10: {what}: the x asks for no redraw");
                assert_eq!(board(&d.app.game), board(&game), "MRK-11: {what}");

                let with = draw_app(&d.app, &fonts);
                assert_only_x(&fonts, &before, &with, center, &what);
                assert_grid(&with, cell, grid, &what);

                // MRK-14: the x stays for its full time, then the screen is the same as before.
                assert_eq!(d.app.next_deadline(), Some(lift + OVERLAY_MS), "MRK-14: {what}");
                d.now = lift + OVERLAY_MS - 1;
                d.app.tick(d.now);
                assert!(d.app.ui.refused.is_some(), "MRK-14: {what}: the x went too early");
                assert_eq!(differences(&draw_app(&d.app, &fonts), &with), vec![], "MRK-14: {what}");
                d.app.redraw = false;
                d.now = lift + OVERLAY_MS;
                d.app.tick(d.now);
                assert_eq!(d.app.ui.refused, None, "MRK-14: {what}: the x stays after its time");
                assert!(d.app.redraw, "MRK-14: {what}: the end of the x asks for no redraw");
                assert_eq!(differences(&draw_app(&d.app, &fonts), &before), vec![], "MRK-13: {what}: the pixels after the x");
            }
        }
    }
}

#[test]
fn the_x_is_at_the_keypad_position_of_its_digit() {
    // MRK-13, §15, MRK-1: the position of the x with literal values, for a cell with a different row
    // and column. The column of the cell and of the digit go on the x axis, the row on the y axis. A
    // swap of row and column, or a mirror image for a right-handed player, gives other pixels. The
    // drawing uses only the cell and the digit of the state, thus each digit can show in the cell.
    const OFFSETS: [(i32, i32); 9] = [(24, 24), (74, 24), (122, 24), (24, 74), (74, 74), (122, 74), (24, 122), (74, 122), (122, 122)];
    let fonts = fonts();
    for (o, right_handed) in LAYOUTS {
        let _layout = InLayout::set(o, right_handed);
        let mut app = reference::app(&fonts);
        app.game = reference::sample_game();
        let (r, c) = cells().find(|&c| c.0 != c.1 && !app.game.is_filled(c)).unwrap();
        let plain = draw_app(&app, &fonts);
        let (bx, by) = board_origin(o);
        for (digit, &(ox, oy)) in (1..=9u8).zip(&OFFSETS) {
            app.ui.refused = Some(((r, c), digit, 0));
            let want = ((bx + c as i32 * 147 + ox) as f32, (by + r as i32 * 147 + oy) as f32);
            assert_only_x(&fonts, &plain, &draw_app(&app, &fonts), want, &format!("{o:?}, right-handed {right_handed}, cell ({r}, {c}), digit {digit}"));
        }
    }
}

#[test]
fn the_x_stays_in_its_micro_cell_in_each_cell_of_the_board() {
    // MRK-13, §15, BRD-6: the limits. In each empty cell (also next to the thick lines and to the edge
    // of the board), the x in each corner micro-cell and at the center changes only its own pixels, and
    // it does not touch the micro-cell lines or the cell lines.
    let fonts = fonts();
    for (o, right_handed) in [LAYOUTS[0], LAYOUTS[3]] {
        let _layout = InLayout::set(o, right_handed);
        let mut app = reference::app(&fonts);
        app.game = reference::sample_game();
        let plain = draw_app(&app, &fonts);
        let empty: Vec<Cell> = cells().filter(|&c| !app.game.is_filled(c)).collect();
        assert!(empty.iter().any(|c| c.0 == 0) && empty.iter().any(|c| c.0 == 8) && empty.iter().any(|c| c.1 == 0) && empty.iter().any(|c| c.1 == 8), "the fixture has cells at the edges of the board");
        for &cell in &empty {
            let (x, y) = layout::cell_origin(cell);
            for digit in [1, 3, 5, 7, 9] {
                app.ui.refused = Some((cell, digit, 0));
                let with = draw_app(&app, &fonts);
                let what = format!("{o:?}, cell {cell:?}, digit {digit}");
                assert_only_x(&fonts, &plain, &with, x_center(board_origin(o), cell, digit), &what);
                let i = digit as i32 - 1;
                let (mx, my) = (i % 3, i / 3);
                let inside = Rect::new(x + layout::micro_offset(mx) + 2, y + layout::micro_offset(my) + 2, x + layout::micro_offset(mx + 1) - 1, y + layout::micro_offset(my + 1) - 1);
                for (px, py) in differences(&plain, &with) {
                    assert!(px >= inside.x0 && px < inside.x1 && py >= inside.y0 && py < inside.y1, "{what}: the pixel ({px}, {py}) is not in the micro-cell");
                }
            }
        }
    }
}

#[test]
fn the_x_and_the_question_mark_show_in_different_cells_with_their_own_times() {
    // MRK-14, MRK-13, QST-1, §3.2, INP-12: a scribble waits in cell A, and the player does a refused tap
    // in cell B. The press reads the ink, thus the "?" shows in A. The lift shows the x in B. The "?"
    // hides the marks of A, and the x does not hide the marks of B. Each has its own time.
    let fonts = fonts();
    for (o, right_handed) in LAYOUTS {
        let _layout = InLayout::set(o, right_handed);
        let what = format!("{o:?}, right-handed {right_handed}");
        let mut d = Driver::new(302);
        let digit = 3;
        let b = cell_with_marks(&mut d, digit, false);
        let a = cells().find(|&c| !d.app.game.is_filled(c) && c != b).unwrap();
        let free = (1..=9u8).find(|&v| peer_numbers(&d.app.game, a) & mark_bit(v) == 0 && d.app.game.marks_at(a) & mark_bit(v) == 0);
        if let Some(v) = free {
            d.tap_micro(a, v);
        }
        assert_ne!(d.app.game.visible_marks(a, true), 0, "{what}: the cell of the \"?\" has marks");
        let before = draw_app(&d.app, &fonts);

        d.write_strokes(&scribble_ink(a));
        assert!(d.app.ui.pending_ink.is_some(), "{what}");
        let (x, y) = micro_center(b, digit);
        d.send(Input::PenDown { x, y, tool: Tool::Pen });
        let pressed = d.now;
        assert_eq!(d.app.ui.question, Some((a, pressed)), "INP-12: {what}: the press reads the ink");
        assert_eq!(d.app.ui.refused, None, "MRK-10: {what}");
        d.send(Input::PenUp { x, y });
        let lift = d.now;
        assert_eq!(d.app.ui.refused, Some((b, digit, lift)), "MRK-10: {what}");
        assert_eq!(d.app.ui.question, Some((a, pressed)), "MRK-14: {what}: the \"?\" stays");

        // The screen with the "?" only, and with the "?" and the x.
        let with = draw_app(&d.app, &fonts);
        d.app.ui.refused = None;
        let only_question = draw_app(&d.app, &fonts);
        d.app.ui.refused = Some((b, digit, lift));
        assert_only_x(&fonts, &only_question, &with, x_center(board_origin(o), b, digit), &what);
        let (ax, ay) = layout::cell_origin(a);
        for (px, py) in differences(&before, &only_question) {
            assert!(px >= ax && px < ax + CELL && py >= ay && py < ay + CELL, "QST-1: {what}: the \"?\" changed the pixel ({px}, {py})");
        }
        let (gx, gy) = (ax + layout::micro_offset(1), ay + 8);
        assert_eq!(rgb_at(&before, gx, gy), sudoku::render::line_rgb(LineGray::default()).unwrap(), "{what}: the grid before the \"?\"");
        assert_eq!(rgb_at(&only_question, gx, gy), rgb_at(&only_question, ax + 8, ay + 8), "QST-1: {what}: the grid of the cell of the \"?\" shows");

        // The "?" goes first, at its own time. The x stays for its own time.
        assert_eq!(d.app.next_deadline(), Some(pressed + OVERLAY_MS), "MRK-14: {what}");
        d.now = pressed + OVERLAY_MS;
        d.app.tick(d.now);
        assert_eq!(d.app.ui.question, None, "QST-1: {what}");
        assert_eq!(d.app.ui.refused, Some((b, digit, lift)), "MRK-14: {what}: the x has its own time");
        assert_only_x(&fonts, &before, &draw_app(&d.app, &fonts), x_center(board_origin(o), b, digit), &what);
        assert_eq!(d.app.next_deadline(), Some(lift + OVERLAY_MS), "MRK-14: {what}");
        d.now = lift + OVERLAY_MS;
        d.app.tick(d.now);
        assert_eq!(d.app.ui.refused, None, "MRK-14: {what}");
        assert_eq!(differences(&draw_app(&d.app, &fonts), &before), vec![], "MRK-13: {what}: the pixels after the x");
    }
}

#[test]
fn the_x_stays_on_its_cell_and_digit_after_a_turn() {
    // MRK-16, §3.3, MRK-13, MRK-14: a turn does not change the x. It shows at the position of the new
    // layout for the rest of its time. A tap in progress at the turn makes no x (NAV-13).
    let fonts = fonts();
    let stops = sudoku::render::ng_stops(&fonts);
    for ((from, from_right), (to, _)) in [(LAYOUTS[0], LAYOUTS[2]), (LAYOUTS[2], LAYOUTS[0]), (LAYOUTS[1], LAYOUTS[3]), (LAYOUTS[3], LAYOUTS[1])] {
        let what = format!("{from:?} to {to:?}, right-handed {from_right}");
        let _layout = InLayout::set(from, from_right);
        let mut d = Driver::new(303);
        let (cell, digit) = refused_spot(&d.app.game, |_, v| v == 2 || v == 4 || v == 8);
        d.tap_micro(cell, digit);
        let lift = d.now;
        assert_eq!(d.app.ui.refused, Some((cell, digit, lift)), "{what}");
        let game = board(&d.app.game);

        d.now += 400;
        d.app.tick(d.now);
        layout::set_orientation(to);
        d.app.rotate(stops);
        d.app.tick(d.now);
        assert_eq!(d.app.ui.refused, Some((cell, digit, lift)), "MRK-16: {what}");
        assert_eq!(d.app.next_deadline(), Some(lift + OVERLAY_MS), "MRK-16: {what}: the rest of the time");
        let with = draw_app(&d.app, &fonts);
        d.app.ui.refused = None;
        let plain = draw_app(&d.app, &fonts);
        d.app.ui.refused = Some((cell, digit, lift));
        assert_only_x(&fonts, &plain, &with, x_center(board_origin(to), cell, digit), &what);
        d.now = lift + OVERLAY_MS;
        d.app.tick(d.now);
        assert_eq!(d.app.ui.refused, None, "MRK-14: {what}");
        assert_eq!(differences(&draw_app(&d.app, &fonts), &plain), vec![], "MRK-13: {what}");

        // The next turn stops a refused tap at the press: the lift makes no x.
        let (x, y) = micro_center(cell, digit);
        d.send(Input::PenDown { x, y, tool: Tool::Pen });
        layout::set_orientation(from);
        d.app.rotate(stops);
        let (x2, y2) = micro_center(cell, digit);
        for (px, py) in [(x2, y2), (x, y)] {
            d.send(Input::PenUp { x: px, y: py });
            assert_eq!(d.app.ui.refused, None, "MRK-16: {what}: a tap in progress at the turn");
        }
        assert_eq!(board(&d.app.game), game, "MRK-16: {what}: the turn changes the game");
    }
}

#[test]
fn the_framebuffer_shows_the_x_and_then_the_old_pixels() {
    // MRK-13, MRK-14, §15: the real output (`present.rs`) in all layouts. After the lift, only the pixels
    // of the x change in the framebuffer, with an update that covers them. After the time of the x,
    // the framebuffer is the same as before, with an update. The pen region is not active: a tap
    // does not start it, thus the redraw does not wait.
    use sudoku::present::{Presenter, Update};
    let fonts = fonts();
    let (w, h) = (layout::PHYSICAL_W as usize, layout::PHYSICAL_H as usize);
    for (o, right_handed) in LAYOUTS {
        let _layout = InLayout::set(o, right_handed);
        let mut d = Driver::new(304);
        let mut p = Presenter::new();
        let mut fb = vec![0u8; w * h * 3];
        let mut updates: Vec<Update> = Vec::new();
        let settle = |d: &mut Driver, p: &mut Presenter, fb: &mut Vec<u8>, updates: &mut Vec<Update>| {
            for _ in 0..12 {
                d.now += 30;
                d.app.tick(d.now);
                p.present(&mut d.app, &fonts, fb, d.now, |u| updates.push(u));
            }
        };
        for digit in [2, 4, 9] {
            let what = format!("{o:?}, right-handed {right_handed}, digit {digit}");
            let cell = cell_with_marks(&mut d, digit, false);
            settle(&mut d, &mut p, &mut fb, &mut updates);
            settle(&mut d, &mut p, &mut fb, &mut updates);
            let before = fb.clone();
            updates.clear();

            let (x, y) = micro_center(cell, digit);
            d.send(Input::PenDown { x, y, tool: Tool::Pen });
            p.present(&mut d.app, &fonts, &mut fb, d.now, |u| updates.push(u));
            assert!(d.app.writing_stroke().is_none(), "{what}: a tap started the pen region");
            d.send(Input::PenUp { x, y });
            let lift = d.now;
            assert_eq!(d.app.ui.refused, Some((cell, digit, lift)), "{what}");
            p.present(&mut d.app, &fonts, &mut fb, d.now, |u| updates.push(u));
            settle(&mut d, &mut p, &mut fb, &mut updates);
            assert!(d.app.ui.refused.is_some() && d.now < lift + OVERLAY_MS, "{what}: the x still shows");

            let center = x_center(board_origin(o), cell, digit);
            let changed: Vec<usize> = (0..w * h).filter(|&i| fb[i * 3..i * 3 + 3] != before[i * 3..i * 3 + 3]).collect();
            assert!(!changed.is_empty(), "{what}: the x is not in the framebuffer");
            for &i in &changed {
                let (sx, sy) = layout::to_screen((i % w) as f32, (i / w) as f32);
                let (sx, sy) = (sx as i32, sy as i32);
                assert!(to_x(sx, sy, center) < X_WIDTH / 2.0 + PIXEL_HALF_DIAGONAL, "{what}: the screen pixel ({sx}, {sy}) changed in the framebuffer");
                let covered = updates.iter().any(|&(ux, uy, uw, uh)| {
                    let (px, py) = ((i % w) as i32, (i / w) as i32);
                    px >= ux && px < ux + uw && py >= uy && py < uy + uh
                });
                assert!(covered, "{what}: no update covers the screen pixel ({sx}, {sy}): {updates:?}");
            }

            updates.clear();
            d.now = lift + OVERLAY_MS;
            d.app.tick(d.now);
            assert_eq!(d.app.ui.refused, None, "{what}");
            p.present(&mut d.app, &fonts, &mut fb, d.now, |u| updates.push(u));
            settle(&mut d, &mut p, &mut fb, &mut updates);
            assert!(fb == before, "MRK-13: {what}: the framebuffer after the x is not the same as before");
            assert!(!updates.is_empty(), "{what}: the end of the x sends no update");
        }
    }
}

