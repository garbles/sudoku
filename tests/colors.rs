//! The color picker app (`sudoku::colors`): the inputs and the areas of the refresh. It is a
//! development app, thus REQUIREMENTS.md and INVARIANTS.md do not give rules for it.

use sudoku::colors::{self, Area, Change, Colors, Picker, Target};
use sudoku::font::Fonts;
use sudoku::render::{rgb_at, Canvas};
use tiny_skia::Pixmap;

const PEN: i32 = -1;
const YELLOW: usize = 2;
const YELLOW_BUTTON: (f32, f32) = (148.0 + 147.0 * 2.5, 350.0);
const NUMBER_BUTTON: (f32, f32) = (200.0, 950.0);
const RESET_BUTTON: (f32, f32) = (1300.0, 950.0);
const CLOSE_BUTTON: (f32, f32) = (50.0, 50.0);
const R_SLIDER_Y: f32 = 1260.0;
const G_SLIDER_Y: f32 = 1410.0;

fn picker() -> Picker {
    Picker::new(Colors::default())
}

fn tap(p: &mut Picker, (x, y): (f32, f32)) -> Change {
    let down = p.press(PEN, x, y);
    let up = p.release(PEN, x, y);
    if up == Change::None { down } else { up }
}

fn drag(p: &mut Picker, y: f32, from: f32, to: f32) -> Change {
    p.press(PEN, from, y);
    p.moved(PEN, to);
    p.release(PEN, to, y)
}

#[test]
fn a_button_selects_a_color_and_changes_no_color() {
    let mut p = picker();
    assert_eq!(tap(&mut p, YELLOW_BUTTON), Change::Selection);
    assert_eq!(p.selected, Target::Palette(YELLOW));
    assert_eq!(tap(&mut p, YELLOW_BUTTON), Change::None);
    assert_eq!(tap(&mut p, NUMBER_BUTTON), Change::Selection);
    assert_eq!(p.selected, Target::Number);
    assert_eq!(p.colors, Colors::default());
}

#[test]
fn a_slider_changes_only_the_selected_color_and_refreshes_at_the_lift() {
    let mut p = picker();
    tap(&mut p, YELLOW_BUTTON);
    p.press(PEN, 400.0, G_SLIDER_Y);
    assert_eq!(p.moved(PEN, 500.0), Change::Slider);
    assert_eq!(p.release(PEN, 500.0, G_SLIDER_Y), Change::Refresh(vec![Target::Palette(YELLOW)]));
    let defaults = Colors::default();
    assert_ne!(p.colors.palette[YELLOW][1], defaults.palette[YELLOW][1]);
    assert_eq!(p.colors.palette[YELLOW][0], defaults.palette[YELLOW][0]);
    for i in (0..defaults.palette.len()).filter(|&i| i != YELLOW) {
        assert_eq!(p.colors.palette[i], defaults.palette[i]);
    }
    assert_eq!(p.colors.number, defaults.number);
}

#[test]
fn a_slider_that_goes_back_to_its_value_has_no_refresh() {
    let mut p = picker();
    let (x, y) = (800.0, R_SLIDER_Y);
    tap(&mut p, YELLOW_BUTTON);
    drag(&mut p, y, x, 1370.0); // The value 255 is the value of Yellow.
    assert_eq!(p.colors, Colors::default());
    assert_eq!(p.press(PEN, 300.0, y), Change::Slider);
    assert_eq!(p.moved(PEN, 1370.0), Change::Slider);
    assert_eq!(p.release(PEN, 1370.0, y), Change::Slider);
}

#[test]
fn a_slider_stays_in_its_range_out_of_the_track() {
    let mut p = picker();
    tap(&mut p, YELLOW_BUTTON);
    drag(&mut p, G_SLIDER_Y, 400.0, -500.0);
    assert_eq!(p.colors.palette[YELLOW][1], 0);
    drag(&mut p, G_SLIDER_Y, 400.0, 5000.0);
    assert_eq!(p.colors.palette[YELLOW][1], 255);
}

#[test]
fn only_the_pointer_that_started_a_drag_moves_the_slider() {
    let mut p = picker();
    p.press(PEN, 400.0, G_SLIDER_Y);
    let value = p.colors;
    assert_eq!(p.press(3, 900.0, R_SLIDER_Y), Change::None);
    assert_eq!(p.moved(3, 900.0), Change::None);
    assert_eq!(p.release(3, 900.0, R_SLIDER_Y), Change::None);
    assert_eq!(p.colors, value);
}

#[test]
fn reset_refreshes_only_the_colors_that_are_different() {
    let mut p = picker();
    assert_eq!(tap(&mut p, RESET_BUTTON), Change::None);
    tap(&mut p, YELLOW_BUTTON);
    drag(&mut p, G_SLIDER_Y, 400.0, 500.0);
    tap(&mut p, NUMBER_BUTTON);
    drag(&mut p, R_SLIDER_Y, 400.0, 900.0);
    assert_eq!(tap(&mut p, RESET_BUTTON), Change::Refresh(vec![Target::Palette(YELLOW), Target::Number]));
    assert_eq!(p.colors, Colors::default());
    assert_eq!(p.selected, Target::Number);
}

#[test]
fn a_lift_out_of_a_button_has_no_effect() {
    let mut p = picker();
    tap(&mut p, YELLOW_BUTTON);
    drag(&mut p, G_SLIDER_Y, 400.0, 500.0);
    let changed = p.colors;
    p.press(PEN, RESET_BUTTON.0, RESET_BUTTON.1);
    assert_eq!(p.release(PEN, 700.0, 1800.0), Change::None);
    assert_eq!(p.colors, changed);
    p.press(PEN, CLOSE_BUTTON.0, CLOSE_BUTTON.1);
    assert_eq!(p.release(PEN, 700.0, 1800.0), Change::None);
    assert_eq!(tap(&mut p, CLOSE_BUTTON), Change::Quit);
}

#[test]
fn the_saved_form_keeps_the_colors() {
    let mut p = picker();
    tap(&mut p, NUMBER_BUTTON);
    drag(&mut p, R_SLIDER_Y, 400.0, 900.0);
    let json = serde_json::to_string(&p.colors).unwrap();
    assert_eq!(serde_json::from_str::<Colors>(&json).unwrap(), p.colors);
}

fn frame(p: &Picker, fonts: &Fonts) -> Pixmap {
    let mut pm = Pixmap::new(1620, 2160).unwrap();
    pm.fill(tiny_skia::Color::WHITE);
    colors::draw(&mut Canvas::new(&mut pm, fonts), p);
    pm
}

fn inside((x, y, w, h): Area, px: i32, py: i32) -> bool {
    px >= x && px < x + w && py >= y && py < y + h
}

/// The refresh of a color must update each pixel that the color changes, and no other preview cell.
#[test]
fn a_color_changes_pixels_only_in_its_refresh_areas() {
    let fonts = Fonts::repo().unwrap();
    let targets = (0..Colors::default().palette.len()).map(Target::Palette).chain([Target::Number]);
    for target in targets {
        let mut p = picker();
        p.selected = target;
        let before = frame(&p, &fonts);
        match target {
            Target::Palette(i) => p.colors.palette[i] = [1, 2, 3],
            Target::Number => p.colors.number = [200, 10, 90],
        }
        let after = frame(&p, &fonts);
        let (mut in_preview, mut outside) = (0, 0);
        for y in 0..2160 {
            for x in 0..1620 {
                if rgb_at(&before, x, y) != rgb_at(&after, x, y) {
                    if inside(target.preview_area(), x, y) {
                        in_preview += 1;
                    } else if !inside(target.button_area(), x, y) && !inside(colors::SLIDERS_AREA, x, y) {
                        outside += 1;
                    }
                }
            }
        }
        assert!(in_preview > 0, "{target:?}: no pixel changed in the preview area");
        assert_eq!(outside, 0, "{target:?}: pixels changed out of the refresh areas");
    }
}
