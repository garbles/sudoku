//! The Licenses screens (REQUIREMENTS.md §9.4) and the data file assets/licenses.json.

mod common;

use common::*;
use sudoku::app::{Menu, Screen, BACK_GUARD_MS};
use sudoku::layout::{self, Button};
use sudoku::licenses;

/// A tap on the row "Third-party licenses" of the Settings panel.
fn open_licenses(d: &mut Driver) {
    d.button(Button::Menu);
    d.button(Button::Settings);
    let (x, y) = pixel_center(layout::settings_row(layout::LICENSES_ROW));
    d.finger_tap(x, y);
}

#[test]
fn the_settings_row_opens_the_licenses_list_and_back_shows_the_game() {
    // LIC-1, NAV-3, NAV-5, NAV-16
    let mut d = Driver::new(31);
    open_licenses(&mut d);
    assert_eq!((d.app.screen, d.app.licenses.page), (Screen::Licenses, 1));
    assert_eq!(d.app.ui.menu, Menu::Closed, "NAV-5");
    let (x, y) = pixel_center(layout::back_button());
    d.finger_tap(x, y);
    assert_eq!(d.app.screen, Screen::Game);
    assert_eq!(d.app.ui.menu, Menu::Open(None), "NAV-3");
    assert!(d.app.toolbar_ignored_until.is_some(), "NAV-16");
    d.wait(BACK_GUARD_MS);
    // The list always opens at page 1.
    open_licenses(&mut d);
    assert_eq!(d.app.licenses.page, 1);
}

#[test]
fn the_pages_of_the_list_stop_at_the_first_and_the_last_page() {
    // LIC-2
    let mut d = Driver::new(32);
    open_licenses(&mut d);
    let pages = licenses::pages(licenses::count(), layout::licenses_rows());
    assert!(pages > 1);
    d.swipe(false);
    assert_eq!(d.app.licenses.page, 1, "no page before page 1");
    for p in 2..=pages {
        if p % 2 == 0 {
            d.swipe(true);
        } else {
            let (x, y) = pixel_center(layout::next_arrow());
            d.pen_tap(x, y);
        }
        assert_eq!(d.app.licenses.page, p);
    }
    d.swipe(true);
    assert_eq!(d.app.licenses.page, pages, "no page after the last page");
    assert_eq!(d.app.licenses_page_rows().end, licenses::count(), "the last page shows the last crate");
}

#[test]
fn a_row_opens_its_license_text_and_back_shows_the_same_list_page() {
    // LIC-3
    let mut d = Driver::new(33);
    open_licenses(&mut d);
    d.swipe(true);
    let row = 2;
    let (x, y) = pixel_center(layout::licenses_row_rect(row));
    d.finger_tap(x, y);
    assert_eq!(d.app.screen, Screen::LicenseText);
    assert_eq!(d.app.licenses.open, layout::licenses_rows() + row);
    assert_eq!(d.app.licenses.text_page, 1);
    let pages = d.app.license_text_pages();
    for _ in 0..pages + 1 {
        d.swipe(true);
    }
    assert_eq!(d.app.licenses.text_page, pages, "no page after the last page");
    let (x, y) = pixel_center(layout::back_button());
    d.finger_tap(x, y);
    assert_eq!((d.app.screen, d.app.licenses.page), (Screen::Licenses, 2));
}

#[test]
fn each_page_of_license_text_has_lines_that_fit() {
    // LIC-4
    for i in 0..licenses::count() {
        let lines = licenses::lines(i);
        assert!(!lines.is_empty());
        for line in lines {
            if let licenses::Line::Body(s) = line {
                assert!(s.chars().count() <= licenses::WRAP, "{s}");
            }
        }
    }
}

#[test]
fn the_licenses_file_agrees_with_cargo_metadata() {
    // LIC-5: scripts/licenses.py --check makes the file again from `cargo metadata` and compares it.
    let out = std::process::Command::new("python3")
        .args(["scripts/licenses.py", "--check"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("python3 must be on PATH");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
}
