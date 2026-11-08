//! Tests for the rules of INVARIANTS.md about the result card of a complete puzzle and the darker
//! secondary gray: CMP-4, CMP-6 to CMP-10, CLK-4, DIF-5 to DIF-8, NAV-9, NAV-19, QST-5, PUZ-7 and
//! PUZ-8. The values come from INVARIANTS.md and REQUIREMENTS.md §8.3 and §15, not from the code. Each test makes
//! the state where the rule can break and examines the state or the pixels.

mod common;

use common::*;
use sudoku::app::{new_game_with_symmetry, App, Comparison, Input, Menu, Panel, Screen, Tool};
use sudoku::font::{Anchor, Fonts, Weight};
use sudoku::game::{cells, Cell, Game};
use sudoku::generator::{generate_version, MODES};
use sudoku::invariants::check_app;
use sudoku::layout::{self, Button, LevelButton, Orientation, Rect};
use sudoku::render::{gray, rgb_at, Canvas, Units, BLACK, SECONDARY};
use sudoku::store::ResultRow;
use tiny_skia::Pixmap;

// ---- Helpers

/// gray25 (#404040): the secondary gray of NAV-19 and QST-5.
const GRAY25: [u8; 3] = [0x40, 0x40, 0x40];
const WHITE: [u8; 3] = [255, 255, 255];

/// The four layouts of the game screen (the orientation and the hand of the player).
const LAYOUTS: [(Orientation, bool); 4] =
    [(Orientation::Portrait, false), (Orientation::Portrait, true), (Orientation::Landscape, false), (Orientation::Landscape, true)];

/// Sets a layout of this thread. When the value goes out of scope, it sets the default layout again.
struct InLayout;

impl InLayout {
    fn set(o: Orientation, right_handed: bool) -> InLayout {
        layout::set_orientation(o);
        layout::set_right_handed(right_handed);
        InLayout
    }
}

impl Drop for InLayout {
    fn drop(&mut self) {
        layout::set_orientation(Orientation::Portrait);
        layout::set_right_handed(false);
    }
}

/// The frame of the card from CMP-7 and R§8.3 (x0, y0, x1, y1, as in the documents).
fn documented_card(o: Orientation, right_handed: bool) -> (i32, i32, i32, i32) {
    match (o, right_handed) {
        (Orientation::Portrait, _) => (148, 1790, 1471, 2110),
        (Orientation::Landscape, false) => (48, 148, 370, 1471),
        (Orientation::Landscape, true) => (1790, 148, 2112, 1471),
    }
}

/// The pixels that the 2 px frame of the card can cover: the documented lines and one pixel outside.
fn card_pixels(o: Orientation, right_handed: bool) -> Rect {
    let (x0, y0, x1, y1) = documented_card(o, right_handed);
    Rect::new(x0 - 1, y0 - 1, x1 + 1, y1 + 1)
}

/// The board with its outer lines (§15: the board does not move with the hand).
fn board_area(o: Orientation) -> Rect {
    let (x, y) = match o {
        Orientation::Portrait => (148, 418),
        Orientation::Landscape => (418, 148),
    };
    Rect::new(x - 4, y - 4, x + layout::BOARD_SIZE + 4, y + layout::BOARD_SIZE + 4)
}

fn inside(r: Rect, (x, y): (i32, i32)) -> bool {
    x >= r.x0 && x < r.x1 && y >= r.y0 && y < r.y1
}

/// True if the two areas (x1 and y1 not included) have no pixel in common and are not adjacent.
fn apart(a: Rect, b: Rect) -> bool {
    a.x1 < b.x0 || b.x1 < a.x0 || a.y1 < b.y0 || b.y1 < a.y0
}

fn draw_app(app: &App, fonts: &Fonts) -> Pixmap {
    let mut pm = Pixmap::new(layout::screen_w(), layout::screen_h()).unwrap();
    sudoku::render::draw(&mut Canvas::new(&mut pm, fonts), app);
    pm
}

/// A white screen with only these texts (weight, size, x, y, anchor, text, color) on it.
type Text<'a> = (Weight, f32, f32, f32, Anchor, &'a str, [u8; 3]);

fn texts_only(fonts: &Fonts, texts: &[Text]) -> Pixmap {
    let mut pm = Pixmap::new(layout::screen_w(), layout::screen_h()).unwrap();
    let mut c = Canvas::new(&mut pm, fonts);
    c.clear();
    c.set_units(Units::Screen);
    for &(w, size, x, y, anchor, s, color) in texts {
        c.text(w, size, x, y, anchor, s, color);
    }
    pm
}

fn first_difference(a: &Pixmap, b: &Pixmap, r: Rect) -> Option<(i32, i32)> {
    (r.y0..r.y1).flat_map(|y| (r.x0..r.x1).map(move |x| (x, y))).find(|&(x, y)| rgb_at(a, x, y) != rgb_at(b, x, y))
}

/// The first pixel of area `r` of `a` that is different from the pixel at (dx, dy) from it in `b` by
/// more than 2 levels (the edges of a drawing at a different position can change by one level).
fn shifted_difference(a: &Pixmap, r: Rect, b: &Pixmap, dx: i32, dy: i32) -> Option<(i32, i32)> {
    (r.y0..r.y1)
        .flat_map(|y| (r.x0..r.x1).map(move |x| (x, y)))
        .find(|&(x, y)| rgb_at(a, x, y).iter().zip(rgb_at(b, x + dx, y + dy)).any(|(&p, q)| p.abs_diff(q) > 2))
}

fn darkest(pm: &Pixmap, r: Rect) -> u8 {
    (r.y0..r.y1).flat_map(|y| (r.x0..r.x1).map(move |x| (x, y))).map(|(x, y)| rgb_at(pm, x, y)[0]).min().unwrap()
}

fn all_white(pm: &Pixmap, r: Rect) -> bool {
    (r.y0..r.y1).all(|y| (r.x0..r.x1).all(|x| rgb_at(pm, x, y) == WHITE))
}

fn differences(a: &Pixmap, b: &Pixmap) -> Vec<(i32, i32)> {
    let w = a.width() as i32;
    let mut out = Vec::new();
    for (i, (pa, pb)) in a.data().chunks_exact(4).zip(b.data().chunks_exact(4)).enumerate() {
        if pa[..3] != pb[..3] {
            out.push((i as i32 % w, i as i32 / w));
        }
    }
    out
}

/// The m:ss or h:mm:ss text of CMP-4.
fn clock(s: u64) -> String {
    if s >= 3600 { format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60) } else { format!("{}:{:02}", s / 60, s % 60) }
}

/// A game of (seed, mode) with all cells filled except `missing` (a cell with no starting number).
fn almost_complete(seed: u64, mode: u8) -> (Game, Cell) {
    almost_complete_sym(seed, mode, false)
}

/// A game of (seed, mode, symmetric) with all cells filled except `missing`.
fn almost_complete_sym(seed: u64, mode: u8, symmetric: bool) -> (Game, Cell) {
    let mut g = new_game_with_symmetry(seed, mode, symmetric);
    let missing = cells().find(|&c| !g.is_filled(c)).unwrap();
    for c in cells() {
        if c != missing && !g.is_filled(c) {
            g.add_number(c, g.solution[c.0 as usize][c.1 as usize]);
        }
    }
    assert!(!g.completed);
    (g, missing)
}

/// A complete game of (seed, mode) with a final time of `seconds`.
fn complete(seed: u64, mode: u8, seconds: u64) -> Game {
    complete_sym(seed, mode, false, seconds)
}

/// A complete game of (seed, mode, symmetric) with a final time of `seconds`.
fn complete_sym(seed: u64, mode: u8, symmetric: bool, seconds: u64) -> Game {
    let (mut g, missing) = almost_complete_sym(seed, mode, symmetric);
    g.add_number(missing, g.solution[missing.0 as usize][missing.1 as usize]);
    assert!(g.completed, "the full board is not complete");
    g.elapsed_ms = seconds * 1000 + 999;
    g
}

fn row(mode: u8, seconds: u64, seed: u64) -> ResultRow {
    ResultRow { id: seed as i64 + 1, finished_at: 1_700_000_000 + seed as i64, mode, givens: 30, seconds, seed, symmetric: false }
}

/// An app that shows the complete game `g` with the menu closed and the results `results`.
fn card_app(g: Game, results: Vec<ResultRow>) -> App {
    let mut app = app_with_game(g, 1_700_000_000, 11);
    app.ui.menu = Menu::Closed;
    app.results = results;
    app
}

/// The center of a level button.
fn level_center(b: LevelButton) -> (f32, f32) {
    let r = layout::level_button(b);
    ((r.x0 + r.x1) as f32 / 2.0, (r.y0 + r.y1) as f32 / 2.0)
}

/// A tap with the pen tip (0), the eraser (1) or a finger (2).
fn tap_with(d: &mut Driver, tool: u8, (x, y): (f32, f32)) {
    match tool {
        0 => d.pen_tap(x, y),
        1 => {
            d.send(Input::PenDown { x, y, tool: Tool::Eraser });
            d.send(Input::PenUp { x, y });
        }
        _ => d.finger_tap(x, y),
    }
}

/// A press at `from` and a release at `to`, with the pen tip (0), the eraser (1) or a finger (2).
fn drag_with(d: &mut Driver, tool: u8, from: (f32, f32), to: (f32, f32)) {
    if tool < 2 {
        let t = if tool == 0 { Tool::Pen } else { Tool::Eraser };
        d.send(Input::PenDown { x: from.0, y: from.1, tool: t });
        d.send(Input::PenMove { x: to.0, y: to.1 });
        d.send(Input::PenUp { x: to.0, y: to.1 });
    } else {
        d.send(Input::TouchDown { id: 1, x: from.0, y: from.1 });
        d.send(Input::TouchMove { id: 1, x: to.0, y: to.1 });
        d.send(Input::TouchUp { id: 1, x: to.0, y: to.1 });
    }
}

/// The large knob of a meter: the only stop with a dark pixel 8 px above its center (the knob is
/// larger than the stops).
fn knob(pm: &Pixmap, stops: &[f32], y: i32) -> Option<usize> {
    let dark: Vec<usize> = stops.iter().enumerate().filter(|(_, &x)| rgb_at(pm, x.round() as i32, y - 8)[0] < 100).map(|(i, _)| i).collect();
    if dark.len() == 1 { Some(dark[0]) } else { None }
}

/// The stops of the small meter of a level button (R§8.3): portrait, step 30 px with the last stop
/// 40 px from the right frame; landscape, step 36 px, centered.
fn button_meter(b: LevelButton) -> (Vec<f32>, i32) {
    let r = layout::level_button(b);
    if layout::orientation() == Orientation::Portrait {
        let last = (r.x1 - 40) as f32;
        ((0..MODES).map(|i| last - (MODES - 1 - i) as f32 * 30.0).collect(), (r.y0 + r.y1) / 2)
    } else {
        let cx = (r.x0 + r.x1) as f32 / 2.0;
        ((0..MODES).map(|i| cx + (i as f32 - 3.0) * 36.0).collect(), r.y0 + 88)
    }
}

// ---- CMP-7: the position of the card in the four layouts.

#[test]
fn the_card_has_the_documented_frame_in_the_four_layouts() {
    // CMP-7, R§3.1, R§3.3, R§8.3: the frame is 2 px black with a white inside, at the documented
    // position for each layout.
    let fonts = fonts();
    for (o, rh) in LAYOUTS {
        let _l = InLayout::set(o, rh);
        let what = format!("{o:?}, right-handed {rh}");
        let (x0, y0, x1, y1) = documented_card(o, rh);
        let r = layout::result_card();
        assert_eq!((r.x0, r.y0, r.x1, r.y1), (x0, y0, x1, y1), "CMP-7: {what}");
        let pm = draw_app(&card_app(reference_complete(), vec![]), &fonts);
        // The four sides, at the middle of each side, are black. Inside the frame is white.
        let (mx, my) = ((x0 + x1) / 2, (y0 + y1) / 2);
        for (x, y) in [(x0, my), (x1, my), (mx, y0), (mx, y1)] {
            assert_eq!(rgb_at(&pm, x, y), [0, 0, 0], "CMP-7: {what}: no black frame at ({x}, {y})");
        }
        for (x, y) in [(x0 + 3, my), (x1 - 3, my), (x0 + 3, y0 + 3), (x1 - 3, y1 - 3)] {
            assert_eq!(rgb_at(&pm, x, y), WHITE, "CMP-7: {what}: the inside is not white at ({x}, {y})");
        }
        for (x, y) in [(x0 - 2, my), (x1 + 2, my), (mx, y0 - 2), (mx, y1 + 2)] {
            assert_eq!(rgb_at(&pm, x, y), WHITE, "CMP-7: {what}: the frame is wider than 2 px at ({x}, {y})");
        }
    }
}

/// The complete game of the reference states: mode 2 and 23:41.
fn reference_complete() -> Game {
    complete(1001, 2, 23 * 60 + 41)
}

#[test]
fn the_card_touches_no_button_and_not_the_board() {
    // CMP-7, NAV-15: with the menu closed, the card does not touch the toolbar, the close button, the
    // menu button or the board. With the menu open in landscape, the toolbar and the card are at
    // opposite sides of the board. The areas are from §3 and §15; the card has its frame pixels.
    for (o, rh) in LAYOUTS {
        let _l = InLayout::set(o, rh);
        let what = format!("{o:?}, right-handed {rh}");
        let card = card_pixels(o, rh);
        let (w, h) = (layout::screen_w() as i32, layout::screen_h() as i32);
        let toolbar = if rh { Rect::new(0, 0, layout::TB_SLOT, h) } else { Rect::new(w - layout::TB_SLOT, 0, w, h) };
        let menu_button = Rect::new(toolbar.x0, 0, toolbar.x1, layout::TB_SLOT);
        let close = if rh { Rect::new(w - layout::TB_SLOT, 0, w, layout::TB_SLOT) } else { Rect::new(0, 0, layout::TB_SLOT, layout::TB_SLOT) };
        for (name, r) in [("toolbar", toolbar), ("menu button", menu_button), ("close button", close), ("board", board_area(o))] {
            assert!(apart(card, r), "CMP-7: {what}: the card touches the {name}");
        }
        // The hit areas agree: no point of the card is a button of the game screen.
        for y in (card.y0..card.y1).step_by(7) {
            for x in (card.x0..card.x1).step_by(7) {
                assert_eq!(layout::game_button(x as f32 + 0.5, y as f32 + 0.5, false), None, "CMP-7: {what}: ({x}, {y})");
                assert_eq!(layout::board_micro(x as f32 + 0.5, y as f32 + 0.5), None, "CMP-7: {what}: ({x}, {y}) is on the board");
            }
        }
        // The level buttons are in the card.
        for b in LevelButton::ALL {
            let br = layout::level_button(b);
            assert!(br.x0 > card.x0 + 2 && br.x1 < card.x1 - 2 && br.y0 > card.y0 + 2 && br.y1 < card.y1 - 2, "CMP-9: {what}: {b:?} is not in the card");
        }
        if o == Orientation::Landscape {
            let board = board_area(o);
            if rh {
                assert!(toolbar.x1 <= board.x0 && card.x0 >= board.x1, "CMP-7: {what}: not at opposite sides");
            } else {
                assert!(card.x1 <= board.x0 && toolbar.x0 >= board.x1, "CMP-7: {what}: not at opposite sides");
            }
        }
    }
}

#[test]
fn the_card_changes_only_its_own_pixels_and_replaces_the_meter() {
    // CMP-4, DIF-6, CMP-7: the last number of the puzzle changes only the board, the toolbar, the
    // meter and the card. The meter does not show on a complete game, and the card does not show on
    // a game that is not complete.
    let fonts = fonts();
    for mode in [0, 6] {
        let (g, missing) = almost_complete(5, mode);
        for (o, rh) in LAYOUTS {
            for menu in [Menu::Closed, Menu::Open(None)] {
                let _l = InLayout::set(o, rh);
                let what = format!("{o:?}, right-handed {rh}, mode {mode}, {menu:?}");
                let mut before = card_app(g.clone(), vec![]);
                before.ui.menu = menu;
                let mut after = before.clone_for_test(&g, missing);
                after.ui.menu = menu;
                let (pb, pa) = (draw_app(&before, &fonts), draw_app(&after, &fonts));
                let (w, h) = (layout::screen_w() as i32, layout::screen_h() as i32);
                let toolbar = if rh { Rect::new(0, 0, layout::TB_SLOT, h) } else { Rect::new(w - layout::TB_SLOT, 0, w, h) };
                let close = if rh { Rect::new(w - layout::TB_SLOT, 0, w, layout::TB_SLOT) } else { Rect::new(0, 0, layout::TB_SLOT, layout::TB_SLOT) };
                let meter = Rect::new(layout::meter_xl() - 10, layout::meter_y() - 25, layout::meter_xr() + 10, layout::meter_y() + 25);
                let card = card_pixels(o, rh);
                for p in differences(&pb, &pa) {
                    let ok = inside(board_area(o), p) || inside(toolbar, p) || inside(close, p) || inside(meter, p) || inside(card, p);
                    assert!(ok, "CMP-7: {what}: the completion changed the pixel {p:?}");
                }
                // Not complete: the meter shows, and no pixel of the card area outside the meter.
                let mut stops = layout::slider_stops(layout::meter_xl(), layout::meter_xr(), meter_label(&fonts, "Easy"), meter_label(&fonts, "Hard")).to_vec();
                stops.truncate(MODES as usize);
                assert_eq!(knob(&pb, &stops, layout::meter_y()), Some(mode as usize), "DIF-6: {what}: the meter does not show the mode");
                for y in card.y0..card.y1 {
                    for x in card.x0..card.x1 {
                        if !inside(meter, (x, y)) {
                            assert_eq!(rgb_at(&pb, x, y), WHITE, "CMP-4: {what}: the card shows on a game that is not complete at ({x}, {y})");
                        }
                    }
                }
                // Complete: no meter. In portrait the part of the meter above the card is white; in
                // landscape all of the meter is white.
                let above = if o == Orientation::Portrait { Rect::new(meter.x0, meter.y0, meter.x1, card.y0) } else { meter };
                assert!(all_white(&pa, above), "CMP-4: {what}: the meter shows on a complete game");
                assert!(darkest(&pa, card) == 0, "CMP-4: {what}: no card");
            }
        }
    }
}

fn meter_label(fonts: &Fonts, s: &str) -> f32 {
    fonts.get(Weight::Medium).width(25.0, s)
}

/// The app `self` with the last number added to its game (the same history as a player gives).
trait CloneForTest {
    fn clone_for_test(&self, g: &Game, missing: Cell) -> App;
}

impl CloneForTest for App {
    fn clone_for_test(&self, g: &Game, missing: Cell) -> App {
        let mut done = g.clone();
        done.add_number(missing, done.solution[missing.0 as usize][missing.1 as usize]);
        assert!(done.completed);
        card_app(done, self.results.clone())
    }
}

#[test]
fn the_card_never_shows_on_other_screens() {
    // CMP-7: the card shows only when `screen = Game`.
    let fonts = fonts();
    for screen in [Screen::Statistics, Screen::Manual] {
        let mut app = card_app(reference_complete(), vec![]);
        let card = draw_app(&app, &fonts);
        app.screen = screen;
        let other = draw_app(&app, &fonts);
        let (x0, y0, x1, y1) = documented_card(Orientation::Portrait, false);
        // The frame lines of the card are not on the other screen.
        let frame = (y0..=y1).all(|y| rgb_at(&other, x0, y) == [0, 0, 0]) && (x0..=x1).all(|x| rgb_at(&other, x, y1) == [0, 0, 0]);
        assert!(!frame, "CMP-7: {screen:?} shows the card");
        assert_ne!(first_difference(&card, &other, card_pixels(Orientation::Portrait, false)), None, "CMP-7: {screen:?}");
    }
}

// ---- CMP-4, CLK-4: the time and the meter in the card.

#[test]
fn the_card_shows_completed_in_and_the_time() {
    // CMP-4, CLK-4, NAV-19, R§8.3, R§15: "Completed in" (Regular 30, gray25) and the time (Medium 96 in
    // portrait, Medium 72 in landscape, black), m:ss and h:mm:ss after one hour. The time fits in the
    // landscape column.
    let fonts = fonts();
    let g = reference_complete();
    for (o, rh) in LAYOUTS {
        let _l = InLayout::set(o, rh);
        let (x0, y0, x1, _) = documented_card(o, rh);
        for seconds in [7, 23 * 60 + 41, 3599, 3600, 3725, 36_000 + 59] {
            let what = format!("{o:?}, right-handed {rh}, {seconds} s");
            let mut game = g.clone();
            game.elapsed_ms = seconds * 1000 + 999;
            let pm = draw_app(&card_app(game, vec![]), &fonts);
            let time = clock(seconds);
            let (label_y, time_y, size, label_area, time_area) = if o == Orientation::Portrait {
                (y0 + 66, y0 + 180, 96.0, Rect::new(x0 + 4, y0 + 4, 780, y0 + 95), Rect::new(x0 + 4, y0 + 95, 780, y0 + 205))
            } else {
                (y0 + 70, y0 + 185, 72.0, Rect::new(x0 + 4, y0 + 4, x1 - 4, y0 + 95), Rect::new(x0 + 4, y0 + 95, x1 - 3, y0 + 215))
            };
            let tx = (x0 + if o == Orientation::Portrait { 40 } else { 28 }) as f32;
            let want = texts_only(
                &fonts,
                &[
                    (Weight::Regular, 30.0, tx, label_y as f32, Anchor::Start, "Completed in", GRAY25),
                    (Weight::Medium, size, tx, time_y as f32, Anchor::Start, &time, BLACK),
                ],
            );
            assert_eq!(first_difference(&pm, &want, label_area), None, "CMP-4, NAV-19: {what}: not \"Completed in\" in gray25");
            assert_eq!(first_difference(&pm, &want, time_area), None, "CMP-4, CLK-4: {what}: not the time {time}");
            // The time fits in the column: all its pixels are inside the frame.
            let ink: Vec<i32> = (time_area.y0..time_area.y1)
                .flat_map(|y| (0..layout::screen_w() as i32).map(move |x| (x, y)))
                .filter(|&(x, y)| rgb_at(&want, x, y) != WHITE)
                .map(|(x, _)| x)
                .collect();
            assert!(ink.iter().all(|&x| x > x0 + 1 && x < x1 - 1), "CMP-4: {what}: the time {time} does not fit in the card");
        }
    }
}

#[test]
fn the_card_shows_the_meter_of_the_mode() {
    // CMP-4, R§8.3: the card shows the meter of `game.mode`. In portrait it is the same drawing as the
    // meter below the board, from x 188 to 628 with its center line at y 2022. In landscape it is a
    // meter with no labels, stop step 40 px, centered in the column at y0 + 255. The player cannot
    // change it.
    let fonts = fonts();
    for mode in 0..MODES {
        let (g, missing) = almost_complete(40 + mode as u64, mode);
        let playing = draw_app(&card_app(g.clone(), vec![]), &fonts);
        let app = card_app(g.clone(), vec![]).clone_for_test(&g, missing);
        let pm = draw_app(&app, &fonts);
        let meter = Rect::new(layout::meter_xl() - 10, layout::meter_y() - 20, layout::meter_xr() + 10, layout::meter_y() + 20);
        let (dx, dy) = (188 - layout::meter_xl(), 2022 - layout::meter_y());
        assert_eq!(shifted_difference(&playing, meter, &pm, dx, dy), None, "CMP-4: mode {mode}: the meter of the card is not the meter of the board");
        // Taps on the meter of the card do not change the game.
        let mut d = Driver::from(app);
        let before = d.app.game.clone();
        let stops = layout::slider_stops(188, 628, meter_label(&fonts, "Easy"), meter_label(&fonts, "Hard"));
        for &x in &stops {
            d.finger_tap(x, 2022.0);
            d.pen_tap(x, 2022.0);
        }
        assert_eq!(d.app.game, before, "CMP-4: mode {mode}: a tap changed the meter of the card");
        for rh in [false, true] {
            let _l = InLayout::set(Orientation::Landscape, rh);
            let pm = draw_app(&d.app, &fonts);
            let (x0, y0, x1, _) = documented_card(Orientation::Landscape, rh);
            let cx = (x0 + x1) as f32 / 2.0;
            let stops: Vec<f32> = (0..MODES).map(|i| cx + (i as f32 - 3.0) * 40.0).collect();
            assert_eq!(knob(&pm, &stops, y0 + 255), Some(mode as usize), "CMP-4: landscape, right-handed {rh}: the meter does not show mode {mode}");
        }
    }
}

// ---- CMP-8: the comparison.

#[test]
fn the_comparison_uses_only_the_earlier_results_at_the_mode() {
    // CMP-8, R§8.3: the average (Statistics rounding) and the minimum of the results at the same mode
    // with a different seed. The result of the current game (the same seed) and the results of other
    // modes are not used. With no earlier result: only "First game at this level".
    let g = reference_complete();
    let (mode, seed) = (g.mode, g.seed);
    let other = (mode + 1) % MODES;
    let cases: Vec<(Vec<ResultRow>, Comparison)> = vec![
        (vec![], Comparison::First),
        (vec![row(mode, 100, seed), row(other, 50, 99)], Comparison::First),
        (vec![row(mode, 100, 7)], Comparison::Earlier { average: 100, best: 100 }),
        (vec![row(mode, 100, 7), row(mode, 200, 8), row(mode, 30, seed), row(other, 10, 9)], Comparison::Earlier { average: 150, best: 100 }),
        (vec![row(mode, 61, 7), row(mode, 62, 8), row(mode, 64, 10)], Comparison::Earlier { average: 62, best: 61 }),
        (vec![row(mode, 3600, 7), row(mode, 7300, 8)], Comparison::Earlier { average: 5450, best: 3600 }),
    ];
    for (results, want) in cases {
        let app = card_app(g.clone(), results.clone());
        assert_eq!(app.card_comparison(), want, "CMP-8: {results:?}");
        if let Comparison::Earlier { average, best } = want {
            assert!(best <= average, "CMP-8");
        }
    }
}

#[test]
fn the_card_shows_the_comparison_text() {
    // CMP-8, NAV-19, R§8.3, R§15: portrait, one line (Regular 25, gray25) at x 188, baseline 2083;
    // landscape, the labels (Regular 25, gray25) at y0 + 370 and y0 + 480 and the values (Medium 40,
    // black) at y0 + 418 and y0 + 528. The values follow `results`: after a delete, the new values.
    let fonts = fonts();
    let g = reference_complete();
    let (mode, seed) = (g.mode, g.seed);
    let earlier = vec![row(mode, 23 * 60 + 41, 7), row(mode, 3725, 8), row(mode, 30, seed)];
    // The average of 1421 and 3725 is 2573 (42:53), the minimum is 1421 (23:41).
    let after_delete = vec![row(mode, 3725, 8), row(mode, 30, seed)];
    for (o, rh) in LAYOUTS {
        let _l = InLayout::set(o, rh);
        let (x0, y0, x1, _) = documented_card(o, rh);
        for (results, avg, best) in [(earlier.clone(), Some("42:53"), "23:41"), (after_delete.clone(), Some("1:02:05"), "1:02:05"), (vec![], None, "")] {
            let what = format!("{o:?}, right-handed {rh}, {results:?}");
            let pm = draw_app(&card_app(g.clone(), results.clone()), &fonts);
            let line;
            let (want, area) = if o == Orientation::Portrait {
                line = match avg {
                    Some(a) => format!("Average {a}  ·  Previous best {best}"),
                    None => "First game at this level".to_string(),
                };
                (texts_only(&fonts, &[(Weight::Regular, 25.0, 188.0, 2083.0, Anchor::Start, &line, GRAY25)]), Rect::new(x0 + 4, 2050, 780, y0 + 316))
            } else {
                let tx = (x0 + 28) as f32;
                let y = y0 as f32;
                let texts: Vec<Text> = match avg {
                    Some(a) => vec![
                        (Weight::Regular, 25.0, tx, y + 370.0, Anchor::Start, "Average at this level", GRAY25),
                        (Weight::Medium, 40.0, tx, y + 418.0, Anchor::Start, a, BLACK),
                        (Weight::Regular, 25.0, tx, y + 480.0, Anchor::Start, "Previous best", GRAY25),
                        (Weight::Medium, 40.0, tx, y + 528.0, Anchor::Start, best, BLACK),
                    ],
                    None => vec![(Weight::Regular, 25.0, tx, y + 370.0, Anchor::Start, "First game at this level", GRAY25)],
                };
                (texts_only(&fonts, &texts), Rect::new(x0 + 4, y0 + 300, x1 - 4, layout::card_divider() - 30))
            };
            assert_eq!(first_difference(&pm, &want, area), None, "CMP-8, NAV-19: {what}");
        }
    }
}

#[test]
fn the_comparison_follows_a_delete_in_statistics() {
    // CMP-8, ANA-2: the player deletes a result on the list of completed games. When the game screen
    // shows again, the card uses the results that stay.
    let mut d = Driver::from(app_with_game(reference_complete(), 1_700_000_000, 3));
    let mode = d.app.game.mode;
    d.app.results = vec![row(mode, 500, 70), row(mode, 100, 71)];
    assert_eq!(d.app.card_comparison(), Comparison::Earlier { average: 300, best: 100 });
    d.app.results.retain(|r| r.seconds != 100);
    assert_eq!(d.app.card_comparison(), Comparison::Earlier { average: 500, best: 500 }, "CMP-8 after a delete");
    d.app.results.clear();
    assert_eq!(d.app.card_comparison(), Comparison::First, "CMP-8 after Delete all");
}

// ---- CMP-9: the level buttons.

#[test]
fn the_level_buttons_have_the_documented_rectangles() {
    // CMP-9, R§8.3: portrait x 820–1439, y 1865–1935, 1945–2015, 2025–2095 for the two hands;
    // landscape x0 + 20 to x1 − 20, 120 px high, pitch 140 px, the first 90 px below the divider at
    // y1 − 560.
    for (o, rh) in LAYOUTS {
        let _l = InLayout::set(o, rh);
        let (x0, _, x1, y1) = documented_card(o, rh);
        for (i, b) in LevelButton::ALL.into_iter().enumerate() {
            let r = layout::level_button(b);
            let i = i as i32;
            let want = if o == Orientation::Portrait {
                (820, 1865 + 80 * i, 1439, 1935 + 80 * i)
            } else {
                (x0 + 20, y1 - 560 + 90 + 140 * i, x1 - 20, y1 - 560 + 210 + 140 * i)
            };
            assert_eq!((r.x0, r.y0, r.x1, r.y1), want, "CMP-9: {o:?}, right-handed {rh}, {b:?}");
            let (cx, cy) = level_center(b);
            assert_eq!(layout::level_button_at(cx, cy), Some(b), "CMP-9");
        }
        assert_eq!(layout::card_divider(), if o == Orientation::Portrait { 788 } else { 911 }, "R§8.3: the divider");
    }
}

#[test]
fn the_level_buttons_look_the_same_and_the_unavailable_button_is_gray() {
    // CMP-9, R§8.3: an available button has a black frame, a white inside, a black label and the
    // small meter of target(b). Same has no highlight. A button that is not available (Easier after
    // Easy, Harder after Hard) has a gray80 frame, a gray70 label and no meter. In all four layouts.
    let fonts = fonts();
    let games: Vec<Game> = (0..MODES).map(|m| complete(60 + m as u64, m, 300)).collect();
    for (o, rh) in LAYOUTS {
        let _l = InLayout::set(o, rh);
        for g in &games {
            let mode = g.mode;
            let pm = draw_app(&card_app(g.clone(), vec![]), &fonts);
            for b in LevelButton::ALL {
                let what = format!("{o:?}, right-handed {rh}, mode {mode}, {b:?}");
                let r = layout::level_button(b);
                let target = match b {
                    LevelButton::Easier => mode.checked_sub(1),
                    LevelButton::Same => Some(mode),
                    LevelButton::Harder => (mode + 1 < MODES).then_some(mode + 1),
                };
                assert_eq!(b.target(mode), target, "CMP-9: {what}: target(b)");
                let (frame, label) = if target.is_some() { ([0, 0, 0], BLACK) } else { (gray(80), gray(70)) };
                let cy = (r.y0 + r.y1) / 2;
                for (x, y) in [(r.x0, cy), (r.x1, cy), (r.x0 + 10, r.y0), (r.x0 + 10, r.y1)] {
                    assert_eq!(rgb_at(&pm, x, y), frame, "CMP-9: {what}: the frame at ({x}, {y})");
                }
                for (x, y) in [(r.x0 + 6, r.y0 + 6), (r.x1 - 6, r.y1 - 6), (r.x0 + 6, cy)] {
                    assert_eq!(rgb_at(&pm, x, y), WHITE, "CMP-9: {what}: the inside is not white at ({x}, {y})");
                }
                let (stops, my) = button_meter(b);
                let (label_text, label_area, meter_area) = if o == Orientation::Portrait {
                    let t: Text = (Weight::Medium, 30.0, (r.x0 + 28) as f32, (cy + 11) as f32, Anchor::Start, b.label(), label);
                    (t, Rect::new(r.x0 + 3, r.y0 + 3, stops[0] as i32 - 20, r.y1 - 2), Rect::new(stops[0] as i32 - 15, r.y0 + 3, r.x1 - 2, r.y1 - 2))
                } else {
                    let t: Text = (Weight::Medium, 32.0, (r.x0 + r.x1) as f32 / 2.0, (r.y0 + 50) as f32, Anchor::Middle, b.label(), label);
                    (t, Rect::new(r.x0 + 3, r.y0 + 3, r.x1 - 2, r.y0 + 66), Rect::new(r.x0 + 3, r.y0 + 70, r.x1 - 2, r.y1 - 2))
                };
                let want = texts_only(&fonts, &[label_text]);
                assert_eq!(first_difference(&pm, &want, label_area), None, "CMP-9: {what}: the label");
                match target {
                    Some(m) => assert_eq!(knob(&pm, &stops, my), Some(m as usize), "CMP-9: {what}: the meter does not show target(b)"),
                    None => assert!(all_white(&pm, meter_area), "CMP-9: {what}: a meter on a button that is not available"),
                }
            }
        }
    }
}

#[test]
fn the_card_has_no_other_buttons() {
    // CMP-9, CMP-10, CMP-3: a tap on each point of the card that is not a level button changes
    // nothing (the meter, the texts, the divider and the space).
    let g = complete(81, 3, 200);
    for (o, rh) in LAYOUTS {
        let _l = InLayout::set(o, rh);
        let mut d = Driver::from(card_app(g.clone(), vec![]));
        let card = card_pixels(o, rh);
        for y in (card.y0..card.y1).step_by(37) {
            for x in (card.x0..card.x1).step_by(41) {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                if layout::level_button_at(px, py).is_some() {
                    continue;
                }
                d.finger_tap(px, py);
                d.pen_tap(px, py);
                assert_eq!(board(&d.app.game), board(&g), "CMP-9: {o:?}, right-handed {rh}: a tap at ({x}, {y})");
                assert_eq!((d.app.screen, d.app.ui.menu), (Screen::Game, Menu::Closed), "CMP-9: {o:?}, right-handed {rh}: ({x}, {y})");
            }
        }
    }
}

fn board(g: &Game) -> Game {
    let mut b = g.clone();
    b.elapsed_ms = 0;
    b
}

// ---- CMP-10, CMP-6, DIF-5, DIF-7, NAV-9, PUZ-7: the effect of a level button.

#[test]
fn a_level_button_starts_a_new_game_at_its_target_with_the_menu_closed() {
    // CMP-10, CMP-6, DIF-5, DIF-7, NAV-9, CMP-5, PUZ-7, PUZ-5: in the four layouts, with the pen tip,
    // the eraser and a finger, with the menu closed and open: one tap starts the new game
    // immediately at target(b), with the next seed and the symmetry of the complete game (PUZ-8).
    // After it the menu is closed with no panel. The old game is not recorded again. A button that
    // is not available changes nothing. The setting of the symmetry is the opposite of the game
    // before the tap: the button must use the game, not the setting (DIF-5).
    let games: Vec<Game> =
        [(0u8, false), (0, true), (3, true), (6, false), (6, true)].iter().map(|&(m, s)| complete_sym(90 + m as u64, m, s, 400)).collect();
    let mut tool = 0u8;
    for (o, rh) in LAYOUTS {
        let _l = InLayout::set(o, rh);
        for g in &games {
            for b in LevelButton::ALL {
                for menu in [Menu::Closed, Menu::Open(None)] {
                    tool = (tool + 1) % 3;
                    let what = format!("{o:?}, right-handed {rh}, mode {}, {b:?}, {menu:?}, tool {tool}", g.mode);
                    let results = vec![row(g.mode, 999, g.seed), row(g.mode, 500, 1)];
                    let mut app = card_app(g.clone(), results.clone());
                    app.settings.last_symmetric = !g.symmetric;
                    app.ui.symmetric_sel = !g.symmetric;
                    let mut d = Driver::from(app);
                    d.app.ui.menu = menu;
                    let early = d.app.early_game();
                    // PUZ-7: a complete game from the save makes the puzzle of Same at once, with the
                    // symmetry of the complete game.
                    let (seed, early_mode, early_sym) =
                        early.unwrap_or_else(|| panic!("PUZ-7: {what}: no early game after the open of a complete game"));
                    assert_eq!(early_mode, g.mode, "PUZ-7: {what}: the early game is not the Same mode");
                    assert_eq!(early_sym, g.symmetric, "PUZ-7: {what}: the early game does not have the symmetry of the game");
                    let settings_before = (d.app.settings.last_mode, d.app.settings.last_symmetric);
                    tap_with(&mut d, tool, level_center(b));
                    let a = &d.app;
                    match b.target(g.mode) {
                        Some(target) => {
                            assert!(target < MODES, "CMP-10: {what}");
                            assert_eq!((a.game.seed, a.game.mode, a.settings.last_mode), (seed, target, target), "CMP-10, DIF-5, PUZ-7: {what}");
                            assert_eq!((a.game.symmetric, a.settings.last_symmetric), (g.symmetric, g.symmetric), "CMP-10, DIF-5: {what}: symmetry");
                            assert_eq!(a.ui.symmetric_sel, g.symmetric, "DIF-8: {what}");
                            assert!(!a.game.completed && a.game.elapsed_ms == 0, "CMP-6, DIF-5: {what}");
                            assert!(a.game.undo.is_empty() && a.game.redo.is_empty() && a.game.checkpoints.is_empty(), "DIF-5: {what}");
                            assert!(cells().all(|c| a.game.is_given(c) || !a.game.is_filled(c)), "DIF-5: {what}: entries");
                            assert!(cells().all(|c| a.game.marks_at(c) == 0), "DIF-5: {what}: marks");
                            assert_eq!((a.screen, a.ui.menu), (Screen::Game, Menu::Closed), "NAV-9, CMP-10: {what}");
                            assert_eq!(a.results, results, "CMP-5, DIF-7: {what}: the old game is recorded");
                            let p = generate_version(2, seed, target, g.symmetric);
                            assert_eq!(a.game.solution, p.solution, "PUZ-7, PUZ-5: {what}");
                            assert_eq!(a.game.given, p.givens.map(|r| r.map(|v| v != 0)), "PUZ-7, PUZ-5: {what}");
                        }
                        None => {
                            assert_eq!(board(&a.game), board(g), "CMP-9: {what}: the button is not available");
                            assert_eq!((a.screen, a.ui.menu), (Screen::Game, menu), "CMP-9: {what}");
                            assert_eq!((a.settings.last_mode, a.settings.last_symmetric), settings_before, "CMP-9: {what}");
                            assert_eq!(a.early_game(), early, "CMP-9: {what}");
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn a_press_on_a_level_button_and_a_release_elsewhere_changes_nothing() {
    // CMP-10: a press on a button and a release on a different place (another level button, the
    // card, the board) does not start a game. With each tool.
    let g = complete(120, 3, 77);
    for (o, rh) in [LAYOUTS[0], LAYOUTS[3]] {
        let _l = InLayout::set(o, rh);
        let card = layout::result_card();
        let spare = ((card.x0 + 12) as f32, (card.y1 - 12) as f32);
        for tool in 0..3u8 {
            for b in LevelButton::ALL {
                for to in [level_center(LevelButton::ALL[(LevelButton::ALL.iter().position(|&x| x == b).unwrap() + 1) % 3]), spare, (layout::board_x() as f32 + 70.0, layout::board_y() as f32 + 70.0)] {
                    let mut d = Driver::from(card_app(g.clone(), vec![]));
                    let (last_mode, early) = (d.app.settings.last_mode, d.app.early_game());
                    drag_with(&mut d, tool, level_center(b), to);
                    let what = format!("{o:?}, right-handed {rh}, tool {tool}, {b:?} to {to:?}");
                    assert_eq!(board(&d.app.game), board(&g), "CMP-10: {what}");
                    assert_eq!((d.app.settings.last_mode, d.app.early_game()), (last_mode, early), "CMP-10: {what}: last_mode");
                    assert_eq!((d.app.screen, d.app.ui.menu), (Screen::Game, Menu::Closed), "CMP-10: {what}");
                }
            }
        }
    }
}

#[test]
fn with_a_panel_open_a_tap_on_the_card_only_closes_the_panel() {
    // CMP-10, NAV-11, CMP-7: when a panel is open, a tap on a level button (outside the panel) only
    // closes the panel. A part of the card can be below the New game panel: there the tap operates
    // the panel, not the card.
    let g = complete(130, 3, 88);
    for (o, rh) in LAYOUTS {
        let _l = InLayout::set(o, rh);
        for panel in [Panel::Colors, Panel::Settings, Panel::NewGame] {
            let area = match panel {
                Panel::Colors => layout::colors_panel(),
                Panel::Settings => layout::settings_panel(false),
                _ => layout::ng_panel(),
            };
            for b in LevelButton::ALL {
                let p = level_center(b);
                if area.contains(p.0, p.1) {
                    continue;
                }
                for tool in [0u8, 2] {
                    let what = format!("{o:?}, right-handed {rh}, {panel:?}, {b:?}, tool {tool}");
                    let mut d = Driver::from(card_app(g.clone(), vec![]));
                    d.app.ui.menu = Menu::Open(Some(panel));
                    tap_with(&mut d, tool, p);
                    assert_eq!(board(&d.app.game), board(&g), "CMP-10: {what}: a new game");
                    assert_eq!(d.app.ui.menu.panel(), None, "NAV-11: {what}: the panel is open");
                    assert_eq!(d.app.screen, Screen::Game, "{what}");
                }
            }
        }
    }
}

#[test]
fn a_level_button_operates_right_after_back() {
    // NAV-16, CMP-10, NAV-9: for 0.5 s after "< Back" only a press on the toolbar or on the close
    // button changes nothing. A tap on a level button operates as usual: a new game at target(b), with
    // the menu closed. The debug build examines all rules after each input (check_app).
    for b in LevelButton::ALL {
        let g = complete(160, 3, 120);
        let mut d = Driver::from(card_app(g.clone(), vec![]));
        let (seed, _, _) = d.app.early_game().expect("PUZ-7");
        d.button(Button::Menu);
        d.button(Button::Statistics);
        assert_eq!(d.app.screen, Screen::Statistics);
        let (x, y) = pixel_center(layout::back_button());
        d.pen_tap(x, y);
        assert_eq!((d.app.screen, d.app.ui.menu), (Screen::Game, Menu::Open(None)), "NAV-3");
        assert!(d.app.toolbar_ignored_until.is_some_and(|t| d.now < t), "NAV-16: the time after < Back");
        let (cx, cy) = level_center(b);
        d.pen_tap(cx, cy);
        check_app(&d.app).unwrap_or_else(|e| panic!("{b:?} right after < Back: {e}"));
        let target = b.target(3).unwrap();
        assert_eq!((d.app.game.seed, d.app.game.mode), (seed, target), "CMP-10, NAV-16: {b:?}");
        assert_eq!((d.app.screen, d.app.ui.menu), (Screen::Game, Menu::Closed), "NAV-9: {b:?}");
        d.wait(600);
        check_app(&d.app).unwrap();
    }
}

#[test]
fn a_level_button_operates_only_on_a_complete_game() {
    // CMP-7, CMP-10: the card does not show on a game that is not complete. A tap at the position of
    // a level button does not start a game.
    for (o, rh) in LAYOUTS {
        let _l = InLayout::set(o, rh);
        let (g, _) = almost_complete(140, 3);
        for b in LevelButton::ALL {
            let mut d = Driver::from(card_app(g.clone(), vec![]));
            assert_eq!(d.app.early_game(), None, "PUZ-7: {o:?}: an early game on a game in progress with no panel");
            d.finger_tap(level_center(b).0, level_center(b).1);
            assert_eq!(d.app.game.seed, g.seed, "CMP-10: {o:?}, right-handed {rh}, {b:?}: a new game");
            assert!(!d.app.game.completed);
        }
    }
}

// ---- PUZ-7: the early puzzle of Same.

#[test]
fn the_early_game_starts_at_the_completion_by_the_ink() {
    // PUZ-7: the read of the ink at the end of the pause completes the puzzle. At that time, with no
    // other input, early_game = (next_seed, game.mode, game.symmetric). Same then uses this seed, and
    // Easier and Harder use the same seed with their own mode. The puzzle is the puzzle of (seed,
    // mode, symmetric) (PUZ-5, CMP-10).
    for (b, mode, sym) in [(LevelButton::Same, 6u8, true), (LevelButton::Easier, 4, false), (LevelButton::Harder, 1, true), (LevelButton::Same, 0, true)] {
        let (g, missing) = almost_complete_sym(150 + mode as u64, mode, sym);
        let mut d = Driver::from(app_with_game(g.clone(), 1_700_000_000, 21));
        assert_eq!(d.app.early_game(), None, "PUZ-7: mode {mode}: an early game before the completion");
        let v = g.solution[missing.0 as usize][missing.1 as usize];
        d.write_strokes(&digit_ink(missing, v));
        assert!(!d.app.game.completed, "the ink is read before the pause");
        // Only time goes by: the app reads the ink at the end of the pause.
        d.wait(d.app.pause_ms + 50);
        assert!(d.app.game.completed, "mode {mode}: the app did not read {v}");
        let (seed, early_mode, early_sym) = d.app.early_game().unwrap_or_else(|| panic!("PUZ-7: mode {mode}: no early game at the completion"));
        assert_eq!((early_mode, early_sym), (mode, g.symmetric), "PUZ-7: the early game is not the Same mode with the symmetry of the game");
        assert_ne!(seed, g.seed, "PUZ-7: the next seed is the seed of the game");
        let target = b.target(mode).unwrap();
        d.finger_tap(level_center(b).0, level_center(b).1);
        assert_eq!((d.app.game.seed, d.app.game.mode), (seed, target), "PUZ-7: {b:?} after mode {mode}");
        let p = generate_version(2, seed, target, g.symmetric);
        assert_eq!(d.app.game.symmetric, g.symmetric, "CMP-10, PUZ-7: symmetry");
        assert_eq!(d.app.game.solution, p.solution, "PUZ-7, PUZ-5");
        assert_eq!(d.app.game.given, p.givens.map(|r| r.map(|v| v != 0)), "PUZ-7, PUZ-5");
        assert_eq!(d.app.ui.menu, Menu::Closed, "NAV-9");
    }
}

#[test]
fn the_new_game_panel_on_a_complete_game_keeps_the_seed_of_the_next_game() {
    // PUZ-7, DIF-5, NAV-9: the New game panel on a complete game. Start new game uses the next seed at
    // the mode of the slider, and the menu stays open (not a level button).
    let mut d = Driver::from(card_app(complete(170, 2, 60), vec![]));
    let (seed, _, _) = d.app.early_game().expect("PUZ-7");
    d.button(Button::Menu);
    d.button(Button::NewGame);
    d.finger_tap(d.app.ng_stops[5], layout::ng_slider_y() as f32);
    d.finger_tap(layout::ng_panel().x0 as f32 + 100.0, (layout::tb_bottom_y() + 56) as f32);
    assert_eq!((d.app.game.seed, d.app.game.mode, d.app.settings.last_mode), (seed, 5, 5), "PUZ-7, DIF-5");
    // DIF-5, PUZ-7: Start new game uses the switch (the setting, on), not the symmetry of the complete
    // game (off).
    assert!(d.app.game.symmetric && d.app.settings.last_symmetric, "DIF-5: the symmetry of the switch");
    let p = generate_version(2, seed, 5, true);
    assert_eq!(d.app.game.solution, p.solution, "PUZ-7, PUZ-5");
    assert_eq!(d.app.ui.menu, Menu::Open(None), "NAV-9");
}

// ---- NAV-19, QST-5: the secondary gray.

#[test]
fn the_question_mark_is_regular_104_in_gray25() {
    // QST-5, NAV-19: the "?" is Regular 104 px in gray25 (#404040). Its darkest pixel is gray25, and
    // no pixel of it is darker.
    let fonts = fonts();
    for (o, rh) in LAYOUTS {
        let _l = InLayout::set(o, rh);
        let mut app = sudoku::reference::app(&fonts);
        app.game = sudoku::reference::sample_game();
        let cell = cells().find(|&c| !app.game.is_filled(c) && app.game.marks_at(c) == 0).unwrap();
        app.ui.question = Some((cell, 0));
        let pm = draw_app(&app, &fonts);
        let (x, y) = layout::cell_origin(cell);
        // Inside the lines of the board (the outer lines are 7 px wide).
        let r = Rect::new(x + 12, y + 12, x + layout::CELL - 12, y + layout::CELL - 12);
        assert_eq!(darkest(&pm, r), GRAY25[0], "QST-5: {o:?}, right-handed {rh}");
        let full = (r.y0..r.y1).flat_map(|y| (r.x0..r.x1).map(move |x| (x, y))).filter(|&(x, y)| rgb_at(&pm, x, y) == GRAY25).count();
        assert!(full > 100, "QST-5: {o:?}: the \"?\" is not gray25 ({full} pixels)");
        // The size: the glyph of Regular 104 has the height of the "?" of the cell.
        let rows: Vec<i32> = (r.y0..r.y1).filter(|&yy| (r.x0..r.x1).any(|xx| rgb_at(&pm, xx, yy)[0] < 200)).collect();
        let want = texts_only(&fonts, &[(Weight::Regular, 104.0, 400.0, 400.0, Anchor::Middle, "?", GRAY25)]);
        let want_rows = (250..450).filter(|&yy| (300..500).any(|xx| rgb_at(&want, xx, yy)[0] < 200)).count();
        assert_eq!(rows.len(), want_rows, "QST-5: {o:?}: the \"?\" is not 104 px");
    }
}

#[test]
fn the_secondary_gray_is_gray25_and_no_text_is_gray40() {
    // NAV-19, R§15: the secondary gray is gray25 (#404040), and no text or mark has gray40 (#666666).
    // A text or a mark of gray40 has a solid core: groups of 2 x 2 pixels of exactly gray40 (the
    // earlier images had hundreds). The antialiased edges of a darker gray have the value only at
    // single pixels. Thus no reference state (the card, the "?", Statistics, the list of completed
    // games, the Checkpoints panel, the Licenses screens, the update rows, the manual) has such a
    // group. The only exception is the item gray40 of the Micro-cell lines section of the Cell colors
    // panel (COL-7: a setting, not a text color).
    assert_eq!(SECONDARY, GRAY25, "NAV-19: the secondary gray");
    let fonts = fonts();
    let gray40 = [0x66, 0x66, 0x66];
    // The Micro-cell lines section in portrait (NAV-17: the right column, the second row), for the
    // left hand (x 708–1507) and the right hand (x 112–911).
    let line_section = |x: i32, y: i32| (472..833).contains(&y) && ((1108..1508).contains(&x) || (512..912).contains(&x));
    for (name, setup) in sudoku::reference::states() {
        let pm = sudoku::reference::render(&fonts, setup);
        let (w, h) = (pm.width() as i32, pm.height() as i32);
        let core = (0..h - 1).flat_map(|y| (0..w - 1).map(move |x| (x, y))).find(|&(x, y)| {
            (0..=1).all(|dy| (0..=1).all(|dx| rgb_at(&pm, x + dx, y + dy) == gray40)) && !(name.starts_with("color-menu") && line_section(x, y))
        });
        assert_eq!(core, None, "NAV-19: {name}: a group of gray40 pixels");
    }
}

#[test]
fn the_card_shows_no_symmetry_information() {
    // CMP-9: the card has no switch and shows no symmetry information. The same complete game with
    // `symmetric` true and false gives the same pixels, in the four layouts, with the menu closed and
    // open. The game is symmetric, thus the two flags are correct for its starting numbers (PUZ-8).
    let fonts = fonts();
    let g = complete_sym(180, 3, true, 300);
    for (o, rh) in LAYOUTS {
        let _l = InLayout::set(o, rh);
        for menu in [Menu::Closed, Menu::Open(None)] {
            let mut sym = card_app(g.clone(), vec![row(3, 500, 1)]);
            sym.ui.menu = menu;
            assert_eq!(sym.game, g, "the app uses the complete game");
            // The flag changes after the open: the app does not open a file with the wrong flag (PUZ-5).
            let mut flat = card_app(g.clone(), vec![row(3, 500, 1)]);
            flat.ui.menu = menu;
            flat.game.symmetric = false;
            assert_eq!(differences(&draw_app(&sym, &fonts), &draw_app(&flat, &fonts)), vec![], "CMP-9: {o:?}, right-handed {rh}, {menu:?}");
        }
    }
}
