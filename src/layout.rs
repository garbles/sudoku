//! Screen positions and sizes. The values are the same as in `scripts/ui-lib.sh` and the other
//! scripts that make the images (REQUIREMENTS.md §3.1, §9, §10, §11, §15).
//!
//! The coordinates are in the units of the scripts (ImageMagick MVG). The renderer adds 0.5 px to
//! each coordinate (refer to `render.rs`).
//!
//! The values depend on the orientation of the tablet. In portrait (1620 x 2160) they are the values
//! of the images. In landscape (2160 x 1620) the board is at the center, the toolbar is at the right
//! edge and the panels touch the toolbar, as in portrait. The analytics pages have the same scale as in
//! portrait, across the full width; the manual has its own landscape images (REQUIREMENTS.md §3.3).

use std::cell::Cell as StdCell;

use crate::game::Cell;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Orientation {
    Portrait,
    Landscape,
}

/// The rotation of the tablet, as qtfb gives it: 0, 1 (90° left), 2 (90° right) or 3 (180°).
pub type Rotation = i32;

/// The landscape rotation in which the top of the screen is at the physical right edge (the bottom
/// of the app is at x = 1619 of the framebuffer). The other landscape rotation is the opposite. On the
/// Paper Pro, AppLoad sends 2 for the orientation "InvertedLandscape" of xochitl, and this mapping is
/// correct for it (examined on the device).
pub const ROTATION_TOP_RIGHT: Rotation = 2;

thread_local! {
    static ROTATION: StdCell<Rotation> = const { StdCell::new(0) };
    static RIGHT_HANDED: StdCell<bool> = const { StdCell::new(false) };
}

/// The handedness of the player (REQUIREMENTS.md §3.1). For a left-handed player (the images), the
/// toolbar is at the right edge and the close button is at the top left. For a right-handed player,
/// the game screen shows them as a mirror image: the toolbar is at the left edge and the close button
/// is at the top right. The platform reads the handedness of the device.
pub fn right_handed() -> bool {
    RIGHT_HANDED.with(|r| r.get())
}

pub fn set_right_handed(on: bool) {
    RIGHT_HANDED.with(|r| r.set(on));
}

/// The x of the mirror image of x (a pixel center), if the player is right-handed.
fn mirror_x(x: f32) -> f32 {
    if right_handed() { screen_w() as f32 - 1.0 - x } else { x }
}

/// The orientation of the layout. The platform sets the rotation when the tablet turns.
pub fn orientation() -> Orientation {
    match rotation() {
        1 | 2 => Orientation::Landscape,
        _ => Orientation::Portrait,
    }
}

pub fn rotation() -> Rotation {
    ROTATION.with(|r| r.get())
}

pub fn set_rotation(r: Rotation) {
    ROTATION.with(|c| c.set(r.clamp(0, 3)));
}

pub fn set_orientation(o: Orientation) {
    set_rotation(if o == Orientation::Landscape { ROTATION_TOP_RIGHT } else { 0 });
}

/// The pixel index in the physical framebuffer (1620 x 2160, portrait) of the screen point (x, y):
/// `base + x * sx + y * sy`.
pub fn physical_steps() -> (i64, i64, i64) {
    let (w, h) = (PHYSICAL_W as i64, PHYSICAL_H as i64);
    match rotation() {
        3 => ((h - 1) * w + (w - 1), -1, -w),
        r if r == ROTATION_TOP_RIGHT => ((h - 1) * w, -w, 1),
        1 | 2 => (w - 1, w, -1),
        _ => (0, 1, w),
    }
}

/// The physical point of a screen point.
pub fn to_physical(x: i32, y: i32) -> (i32, i32) {
    let (base, sx, sy) = physical_steps();
    let i = base + x as i64 * sx + y as i64 * sy;
    ((i % PHYSICAL_W as i64) as i32, (i / PHYSICAL_W as i64) as i32)
}

/// The screen point of a physical point (the input of qtfb is in physical pixels).
pub fn to_screen(px: f32, py: f32) -> (f32, f32) {
    let (w, h) = (PHYSICAL_W as f32, PHYSICAL_H as f32);
    match rotation() {
        3 => (w - 1.0 - px, h - 1.0 - py),
        r if r == ROTATION_TOP_RIGHT => (h - 1.0 - py, px),
        1 | 2 => (py, w - 1.0 - px),
        _ => (px, py),
    }
}

/// The physical rectangle (x, y, width, height) of a screen rectangle (x0, y0, x1, y1; all included).
pub fn physical_rect((x0, y0, x1, y1): (i32, i32, i32, i32)) -> (i32, i32, i32, i32) {
    let (a, b) = (to_physical(x0, y0), to_physical(x1, y1));
    let (px0, px1) = (a.0.min(b.0), a.0.max(b.0));
    let (py0, py1) = (a.1.min(b.1), a.1.max(b.1));
    (px0, py0, px1 - px0 + 1, py1 - py0 + 1)
}

fn landscape() -> bool {
    orientation() == Orientation::Landscape
}

/// The physical framebuffer is always portrait.
pub const PHYSICAL_W: u32 = 1620;
pub const PHYSICAL_H: u32 = 2160;

/// The width and the height of the screen as the player sees it.
pub fn screen_w() -> u32 {
    if landscape() { PHYSICAL_H } else { PHYSICAL_W }
}

pub fn screen_h() -> u32 {
    if landscape() { PHYSICAL_W } else { PHYSICAL_H }
}

/// Board: 9x9 cells of 147 px. Micro-cells of 49 px.
pub const CELL: i32 = 147;
pub const BOARD_SIZE: i32 = 9 * CELL;

/// The top left of the board: (148, 418) in portrait, (418, 148) in landscape.
pub fn board_x() -> i32 {
    (screen_w() as i32 - BOARD_SIZE) / 2
}

pub fn board_y() -> i32 {
    (screen_h() as i32 - BOARD_SIZE) / 2
}

/// The offset of micro-cell line k (0..=3) in a cell: round(k * 147 / 3).
pub fn micro_offset(k: i32) -> i32 {
    (k * CELL + 1) / 3
}

/// The board cell at a screen point, if the point is on the board.
pub fn board_cell(x: f32, y: f32) -> Option<Cell> {
    let (bx, by) = (x - board_x() as f32, y - board_y() as f32);
    if bx < 0.0 || by < 0.0 || bx >= BOARD_SIZE as f32 || by >= BOARD_SIZE as f32 {
        return None;
    }
    Some(((by as i32 / CELL) as u8, (bx as i32 / CELL) as u8))
}

/// The cell and the micro-cell digit (1..=9, keypad positions) at a screen point.
pub fn board_micro(x: f32, y: f32) -> Option<(Cell, u8)> {
    let cell = board_cell(x, y)?;
    let ix = (x as i32 - board_x() - cell.1 as i32 * CELL) * 3 / CELL;
    let iy = (y as i32 - board_y() - cell.0 as i32 * CELL) * 3 / CELL;
    Some((cell, (iy.clamp(0, 2) * 3 + ix.clamp(0, 2) + 1) as u8))
}

pub fn cell_origin((r, c): Cell) -> (i32, i32) {
    (board_x() + c as i32 * CELL, board_y() + r as i32 * CELL)
}

/// The difficulty meter below the board (render.sh): "Easy" at `meter_xl`, "Hard" ends at `meter_xr`.
pub const DIFF_W: i32 = 440;
pub const STOPS: i32 = crate::generator::MODES as i32;

pub fn meter_y() -> i32 {
    board_y() + BOARD_SIZE + 50 // 1791 in portrait
}

pub fn meter_xl() -> i32 {
    board_x() + BOARD_SIZE / 2 - DIFF_W / 2 // 589 in portrait
}

pub fn meter_xr() -> i32 {
    meter_xl() + DIFF_W
}

/// The toolbar (the open menu) at the right edge: x 1508 and the bottom slot at y 2048 in portrait.
/// For a right-handed player it is at the left edge (x 0).
pub const TB_SLOT: i32 = 112;

pub fn tb_x() -> i32 {
    if right_handed() { 0 } else { screen_w() as i32 - TB_SLOT }
}

/// The x of the toolbar edge that the panels touch.
fn tb_inner_x() -> i32 {
    if right_handed() { TB_SLOT } else { tb_x() }
}

pub fn tb_cx() -> f32 {
    tb_x() as f32 + 55.5
}

pub fn tb_bottom_y() -> i32 {
    screen_h() as i32 - TB_SLOT
}

/// The center y of top slot k.
pub fn tb_cy(k: i32) -> f32 {
    55.5 + (k * TB_SLOT) as f32
}

/// The close button (top left, or top right for a right-handed player). Hit area: one toolbar slot.
pub fn close_cx() -> f32 {
    mirror_x(55.5)
}

pub const CLOSE_CY: f32 = 55.5;

/// The buttons of the game screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Menu,
    Colors,
    Settings,
    Undo,
    Redo,
    Help,
    Statistics,
    NewGame,
    Close,
}

/// The game-screen button at a point. Only the menu button and the close button show when the menu
/// is closed. The close button shows only when the menu is open.
pub fn game_button(x: f32, y: f32, menu_open: bool) -> Option<Button> {
    let (x, y) = (mirror_x(x) as i32, y as i32);
    if x < TB_SLOT && y < TB_SLOT {
        return menu_open.then_some(Button::Close);
    }
    if x < screen_w() as i32 - TB_SLOT {
        return None;
    }
    let top = y / TB_SLOT;
    if top == 0 {
        return Some(Button::Menu);
    }
    if !menu_open {
        return None;
    }
    match top {
        1 => return Some(Button::Colors),
        2 => return Some(Button::Settings),
        3 => return Some(Button::Undo),
        4 => return Some(Button::Redo),
        _ => {}
    }
    if y >= tb_bottom_y() {
        Some(Button::NewGame)
    } else if y >= tb_bottom_y() - TB_SLOT {
        Some(Button::Statistics)
    } else if y >= tb_bottom_y() - 2 * TB_SLOT {
        Some(Button::Help)
    } else {
        None
    }
}

/// Rectangle: x0, y0, x1, y1 (x1 and y1 not included).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
}

impl Rect {
    pub const fn new(x0: i32, y0: i32, x1: i32, y1: i32) -> Rect {
        Rect { x0, y0, x1, y1 }
    }

    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x0 as f32 && x < self.x1 as f32 && y >= self.y0 as f32 && y < self.y1 as f32
    }
}

/// The Cell colors panel (toolbar.sh): three sections of 448 px from y 112.
pub const COLORS_TOP: i32 = 112;
pub const COLOR_SECTION_H: i32 = 448;

/// The left and the right x of a panel of width w (pixels pl..=pr) that touches the toolbar.
fn panel_x(w: i32) -> (i32, i32) {
    if right_handed() { (tb_inner_x(), tb_inner_x() + w - 1) } else { (tb_inner_x() - w, tb_inner_x() - 1) }
}

pub fn colors_pl() -> i32 {
    panel_x(400).0 // 1108 in portrait
}

pub fn colors_pr() -> i32 {
    panel_x(400).1 // 1507 in portrait
}

pub fn colors_panel() -> Rect {
    Rect::new(colors_pl(), COLORS_TOP, colors_pr() + 1, COLORS_TOP + 3 * COLOR_SECTION_H + 1)
}

/// The top left of the grid cell for palette index i (0..9) in a section.
pub fn color_cell(section: usize, i: usize) -> (i32, i32) {
    let top = COLORS_TOP + section as i32 * COLOR_SECTION_H;
    (colors_pl() + 32 + (i as i32 % 3) * 112, top + 79 + (i as i32 / 3) * 112)
}

/// The Settings panel (REQUIREMENTS.md §9.1): two rows of 112 px from the top of the gear (y 224),
/// with the width and the border of the Cell colors panel.
pub const SETTINGS_TOP: i32 = 2 * TB_SLOT;
pub const SETTINGS_ROWS: i32 = 2;

pub fn settings_panel() -> Rect {
    // 2 px border at the top and at the bottom, 2 px line between the rows.
    Rect::new(colors_pl(), SETTINGS_TOP, colors_pr() + 1, SETTINGS_TOP + SETTINGS_ROWS * TB_SLOT + 6)
}

/// The row of a switch (0: highlight numbers, 1: remove pencil marks automatically).
pub fn settings_row(i: usize) -> Rect {
    let y = SETTINGS_TOP + 2 + i as i32 * (TB_SLOT + 2);
    Rect::new(colors_pl(), y, colors_pr() + 1, y + TB_SLOT)
}

/// The switch at a point in the Settings panel.
pub fn settings_at(x: f32, y: f32) -> Option<usize> {
    (0..SETTINGS_ROWS as usize).find(|&i| settings_row(i).contains(x, y))
}

/// The section and the palette index at a point in the Cell colors panel.
pub fn color_at(x: f32, y: f32) -> Option<(usize, usize)> {
    for s in 0..3 {
        for i in 0..9 {
            let (gx, gy) = color_cell(s, i);
            if Rect::new(gx, gy, gx + 112, gy + 112).contains(x, y) {
                return Some((s, i));
            }
        }
    }
    None
}

/// The New game panel (toolbar.sh): from the slot above the + to the bottom of the screen.
pub fn ng_pl() -> i32 {
    panel_x(DIFF_W + 66).0 // 1002 in portrait
}

pub fn ng_pr() -> i32 {
    panel_x(DIFF_W + 66).1
}

pub fn ng_top() -> i32 {
    tb_bottom_y() - 112
}

pub fn ng_panel() -> Rect {
    Rect::new(ng_pl(), ng_top(), ng_pr() + 1, screen_h() as i32)
}

pub fn ng_tx() -> i32 {
    ng_pl() + 33
}

pub fn ng_tr() -> i32 {
    ng_tx() + DIFF_W
}

pub fn ng_slider_y() -> i32 {
    tb_bottom_y() - 56
}

pub fn ng_start_y() -> i32 {
    tb_bottom_y() + 56
}

pub fn ng_slider_row() -> Rect {
    Rect::new(ng_pl(), ng_top(), ng_pr() + 1, tb_bottom_y())
}

pub fn ng_start_row() -> Rect {
    Rect::new(ng_pl(), tb_bottom_y() + 2, ng_pr() + 1, screen_h() as i32)
}

/// The x positions of the stops of a labeled slider from `xl` to `xr` (ui-lib.sh `labeled_slider`).
/// `easy_w` and `insane_w` are the text widths of the two labels.
pub fn slider_stops(xl: i32, xr: i32, easy_w: f32, insane_w: f32) -> [f32; STOPS as usize] {
    let xa = xl as f32 + easy_w.round() + 24.0;
    let xb = xr as f32 - insane_w.round() - 24.0;
    let step = (xb - xa) / (STOPS - 1) as f32;
    std::array::from_fn(|i| xa + i as f32 * step)
}

/// The nearest stop to x.
pub fn nearest_stop(stops: &[f32; STOPS as usize], x: f32) -> u8 {
    let mut best = 0;
    for (i, s) in stops.iter().enumerate() {
        if (s - x).abs() < (stops[best] - x).abs() {
            best = i;
        }
    }
    best as u8
}

// ---- Manual and analytics: layout units of 1404x1872, with a scale to the screen (manual-lib.sh).

/// The scale of the layout units of the manual and the analytics pages (manual-lib.sh). The pages are
/// 1404 x 1872 units in portrait and 1872 x 1404 units in landscape.
pub fn page_scale() -> f32 {
    1.153846
}

/// The width and the height of a page, in layout units.
pub fn page_w() -> f32 {
    screen_w() as f32 / page_scale()
}

pub fn page_h() -> f32 {
    screen_h() as f32 / page_scale()
}

/// The right edge of the content of a page (1332 in portrait).
pub fn page_right() -> f32 {
    page_w() - 72.0
}

/// Converts a rectangle in the layout units of the manual to screen pixels.
pub fn page_rect(x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
    Rect::new(
        (x0 * page_scale()) as i32,
        (y0 * page_scale()) as i32,
        (x1 * page_scale()).ceil() as i32,
        (y1 * page_scale()).ceil() as i32,
    )
}

/// "< Back" at the top left of the manual and the analytics screens.
pub fn back_button() -> Rect {
    page_rect(0.0, 0.0, 180.0, 100.0)
}

/// The center x and the baseline of the page number. The arrows are at 74 to 84 units from the
/// center (in portrait: ‹ at x 618..628, › at x 776..786, y 1815..1835).
pub fn page_number_pos() -> (f32, f32) {
    (page_w() / 2.0, page_h() - 37.0)
}

pub fn prev_arrow() -> Rect {
    let (cx, _) = page_number_pos();
    page_rect(cx - 142.0, page_h() - 102.0, cx - 42.0, page_h())
}

pub fn next_arrow() -> Rect {
    let (cx, _) = page_number_pos();
    page_rect(cx + 42.0, page_h() - 102.0, cx + 142.0, page_h())
}

/// Analytics summary: the "All completed games" button.
pub const ROW: f32 = 97.0;
pub const SUMMARY_TABLE_Y: f32 = 640.0;

/// The top of the row of "All completed games" and "Delete all games".
pub fn summary_buttons_y() -> f32 {
    SUMMARY_TABLE_Y + STOPS as f32 * summary_row() + 70.0
}

/// The height of a row of the "By difficulty" table: 97 units in portrait. In landscape the seven rows
/// must fit on a page of 1404 units, thus the rows are 76 units (REQUIREMENTS.md §13.1).
pub fn summary_row() -> f32 {
    if landscape() { 76.0 } else { ROW }
}

/// The width of "Delete all games" (layout units).
pub const DELETE_ALL_W: f32 = 400.0;

pub fn all_games_button() -> Rect {
    let y = summary_buttons_y();
    page_rect(72.0, y, page_right() - DELETE_ALL_W - 28.0, y + ROW)
}

/// "Delete all games", at the right of "All completed games" (REQUIREMENTS.md §13.1).
pub fn delete_all_button() -> Rect {
    let y = summary_buttons_y();
    page_rect(page_right() - DELETE_ALL_W, y, page_right(), y + ROW)
}

/// The width of each of the two boxes at the top of the summary (616 in portrait).
pub fn summary_box_w() -> f32 {
    (page_w() - 172.0) / 2.0
}

/// The right edge of the "Completed" column of the summary (900 in portrait).
pub fn summary_count_x() -> f32 {
    page_right() - 432.0
}

/// Games list: filter segments (All, then modes 0..6), 157.5 units wide in portrait.
pub const SEG_X: f32 = 72.0;
pub const SEG_Y: f32 = 240.0;

pub fn seg_w() -> f32 {
    (page_w() - 144.0) / (STOPS + 1) as f32
}

/// The distance between the stops of a difficulty meter of the analytics screens: 34 units in the
/// table and the list, and 18 units in the filter segments, so that the meter fits in the segment.
pub const METER_STEP: f32 = 34.0;
pub const FILTER_METER_STEP: f32 = 18.0;

pub fn filter_segment(i: usize) -> Rect {
    let x = SEG_X + i as f32 * seg_w();
    page_rect(x, SEG_Y, x + seg_w(), SEG_Y + 80.0)
}

pub const LIST_Y: f32 = 420.0;

/// The quantity of games on a page of the list: 12 in portrait, 6 in landscape (the height of the
/// screen is too small for 12 rows). The table is across the full width (REQUIREMENTS.md §3.3).
pub fn list_rows() -> usize {
    if landscape() { 6 } else { 12 }
}

/// The left x and the width of the games list.
pub fn list_column() -> (f32, f32) {
    (72.0, page_w() - 144.0)
}

/// The x offsets in a row of the games list: the meter, the end of the time, and the bin.
pub fn list_offsets() -> (f32, f32, f32) {
    let w = list_column().1;
    (488.0, w - 222.0, w - 42.0)
}

/// The top y of the row of game i of a page.
pub fn list_row(i: usize) -> f32 {
    LIST_Y + i as f32 * ROW
}

/// The bin of list row i.
pub fn bin_button(i: usize) -> Rect {
    let y = list_row(i);
    let bx = list_column().0 + list_offsets().2;
    page_rect(bx - 45.0, y, bx + 45.0, y + ROW)
}

/// The delete dialog (analytics.py): x 252..1152, y 760..1110 in portrait, at the center of the page.
/// The buttons are in the last row.
pub fn dialog() -> (f32, f32, f32, f32) {
    let (x0, y0) = (page_w() / 2.0 - 450.0, page_h() / 2.0 - 176.0);
    (x0, y0, x0 + 900.0, y0 + 350.0)
}

pub fn dialog_cancel() -> Rect {
    let (x0, _, x1, y1) = dialog();
    page_rect(x0, y1 - ROW, (x0 + x1) / 2.0, y1)
}

pub fn dialog_delete() -> Rect {
    let (x0, _, x1, y1) = dialog();
    page_rect((x0 + x1) / 2.0, y1 - ROW, x1, y1)
}

/// A horizontal finger movement of this length or more is a swipe.
pub const SWIPE_MIN: f32 = 120.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn board_geometry_agrees_with_requirements() {
        assert_eq!((board_x(), board_y()), (148, 418));
        assert_eq!(board_x() + BOARD_SIZE, 1471);
        assert_eq!(board_y() + BOARD_SIZE, 1741);
        assert_eq!([micro_offset(1), micro_offset(2), micro_offset(3)], [49, 98, 147]);
        // NAV-10: the open toolbar does not cover the board.
        for o in [Orientation::Portrait, Orientation::Landscape] {
            set_orientation(o);
            assert!(tb_x() > board_x() + BOARD_SIZE, "{o:?}");
        }
        set_orientation(Orientation::Portrait);
    }

    #[test]
    fn rotations_map_the_screen_onto_the_framebuffer() {
        for r in 0..4 {
            set_rotation(r);
            let (w, h) = (screen_w() as i32, screen_h() as i32);
            let mut seen = std::collections::HashSet::new();
            for (x, y) in [(0, 0), (w - 1, 0), (0, h - 1), (w - 1, h - 1), (w / 3, h / 5)] {
                let (px, py) = to_physical(x, y);
                assert!((0..PHYSICAL_W as i32).contains(&px) && (0..PHYSICAL_H as i32).contains(&py), "r {r}");
                assert!(seen.insert((px, py)));
                // The input of qtfb comes back to the same screen point.
                assert_eq!(to_screen(px as f32, py as f32), (x as f32, y as f32), "r {r}");
            }
        }
        set_rotation(0);
    }

    #[test]
    fn portrait_pages_keep_the_values_of_the_images() {
        set_rotation(0);
        assert_eq!((page_w().round(), page_h().round()), (1404.0, 1872.0));
        assert_eq!((page_right().round(), summary_box_w().round(), summary_count_x().round(), (seg_w() * 2.0).round()), (1332.0, 616.0, 900.0, 315.0));
        let (x0, y0, x1, y1) = dialog();
        assert_eq!((x0.round(), y0.round(), x1.round(), y1.round()), (252.0, 760.0, 1152.0, 1110.0));
        assert_eq!(((list_column().0 + list_offsets().1).round(), (list_column().0 + list_offsets().2).round()), (1110.0, 1290.0));
        assert_eq!(prev_arrow(), page_rect(560.0, 1770.0, 660.0, 1872.0));
        set_orientation(Orientation::Landscape);
        assert_eq!((list_column().0 + list_offsets().2).round(), (page_right() - 42.0).round(), "the table is across the full width");
        assert!(list_row(list_rows()) < page_h() - 102.0, "the rows fit above the page arrows");
        set_rotation(0);
    }

    #[test]
    fn micro_cells_use_keypad_positions() {
        let (x, y) = cell_origin((4, 4));
        assert_eq!(board_micro(x as f32 + 5.0, y as f32 + 5.0), Some(((4, 4), 1)));
        assert_eq!(board_micro(x as f32 + 140.0, y as f32 + 5.0), Some(((4, 4), 3)));
        assert_eq!(board_micro(x as f32 + 70.0, y as f32 + 70.0), Some(((4, 4), 5)));
        assert_eq!(board_micro(x as f32 + 140.0, y as f32 + 140.0), Some(((4, 4), 9)));
        assert_eq!(board_micro(100.0, 100.0), None);
    }

    #[test]
    fn toolbar_buttons() {
        assert_eq!(game_button(1560.0, 50.0, false), Some(Button::Menu));
        assert_eq!(game_button(1560.0, 160.0, false), None);
        assert_eq!(game_button(50.0, 50.0, false), None);
        assert_eq!(game_button(50.0, 50.0, true), Some(Button::Close));
        assert_eq!(game_button(1560.0, 160.0, true), Some(Button::Colors));
        assert_eq!(game_button(1560.0, 280.0, true), Some(Button::Settings));
        assert_eq!(game_button(1560.0, 390.0, true), Some(Button::Undo));
        assert_eq!(game_button(1560.0, 500.0, true), Some(Button::Redo));
        assert_eq!(game_button(1560.0, 1880.0, true), Some(Button::Help));
        assert_eq!(game_button(1560.0, 1992.0, true), Some(Button::Statistics));
        assert_eq!(game_button(1560.0, 2104.0, true), Some(Button::NewGame));
        assert_eq!(game_button(1560.0, 1000.0, true), None);
    }

    #[test]
    fn right_handed_toolbar_buttons() {
        // NAV-15: the toolbar is at the left edge and the close button at the top right.
        set_right_handed(true);
        assert_eq!(game_button(60.0, 50.0, false), Some(Button::Menu));
        assert_eq!(game_button(1560.0, 50.0, false), None);
        assert_eq!(game_button(1560.0, 50.0, true), Some(Button::Close));
        assert_eq!(close_cx(), 1563.5);
        assert_eq!(game_button(60.0, 160.0, true), Some(Button::Colors));
        assert_eq!(game_button(60.0, 2104.0, true), Some(Button::NewGame));
        assert_eq!(game_button(200.0, 160.0, true), None);
        assert_eq!((colors_pl(), colors_pr(), ng_pl(), ng_pr()), (112, 511, 112, 617));
        set_right_handed(false);
        assert_eq!((colors_pl(), colors_pr(), ng_pl(), ng_pr()), (1108, 1507, 1002, 1507));
    }
}
