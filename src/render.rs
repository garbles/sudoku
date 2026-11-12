//! Draws the screens (REQUIREMENTS.md §3, §9 to §13 and §15). The drawing commands are the same as in
//! the scripts that make the images in `images/pro/`.
//!
//! Coordinates are in ImageMagick units: a pixel center is at an integer coordinate. tiny-skia puts a
//! pixel center at +0.5 px. Thus each drawing gets this translation (`RasterMetrics::pixel_center_offset`).
//! The other calibrations against ImageMagick and the margins of the drawings are also in
//! `RasterMetrics`, and the ink of the pen is in `InkMetrics`.

use std::sync::OnceLock;

use tiny_skia::{FillRule, LineCap, LineJoin, Paint, Path, PathBuilder, Pixmap, PixmapPaint, Rect as SkRect, Stroke, Transform};

use crate::app::{average_seconds, App, Comparison, Menu, Panel, Screen, MANUAL_PAGES};
use crate::font::{Anchor, Fonts, Weight};
use crate::game::{cells, format_clock, Board, CellSet, Game};
use crate::generator::MODES;
use crate::layout::{self, *};
use crate::settings::{Color, LineGray, Section, Settings, Switch};
use crate::store::ResultRow;
use crate::update::{Update, UpdateButton};

pub type Rgb = [u8; 3];

pub use crate::settings::{BLACK, ENTRY_DIGIT};

const WHITE: Rgb = [255, 255, 255];
/// The caution icon on the gear (REQUIREMENTS.md §9.2).
pub const CAUTION_RED: Rgb = [0xD6, 0x2D, 0x20];

/// The gray of the micro-cell lines (BRD-6, the setting of §10), as a value. No pixel has this gray:
/// the lines show it as a pattern of gray15 and white pixels with its darkness (BRD-9, NAV-19). None:
/// no lines.
pub fn line_rgb(g: LineGray) -> Option<Rgb> {
    g.percent().map(gray)
}

/// The darkness of the micro-cell lines: the gray (0 black to 255 white) that the pattern shows
/// (BRD-9). None: no lines.
pub fn line_shade(g: LineGray) -> Option<u8> {
    line_rgb(g).map(|c| c[0])
}

/// The pattern of the micro-cell lines (BRD-9): an ordered 4x4 Bayer dither. The row index is the
/// position along the line (modulo 4), the column index is the position across it.
pub const BAYER: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];

/// The color of the dots of the pattern of the micro-cell lines (BRD-9): gray15, the gray of the
/// pencil marks. Not black: with black dots, an empty cell was a pure black and white area, and
/// xochitl selected its mono waveform when a color appeared in the cell (R§4.1).
pub const LINE_DOT: Rgb = MARK_COLOR;

/// True if the pattern of the gray `gray` (0 to 255) has a dot (gray15) at the pixel `along` px from
/// the first pixel of the line and `across` px from its offset pixel (BRD-9): `m * 16 + 8 <= 255 -
/// gray` for the entry m = `BAYER[along mod 4][across mod 4]`.
pub const fn pattern_dot(gray: u8, along: i32, across: i32) -> bool {
    BAYER[along.rem_euclid(4) as usize][across.rem_euclid(4) as usize] as u32 * 16 + 8 <= 255 - gray as u32
}

/// The axis along which a pattern runs (BRD-9): the row index of `BAYER` is the position on this
/// axis, and the column index is the position on the other axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    /// A horizontal line: `BAYER[dx mod 4][dy]`.
    X,
    /// A vertical line, or the disc of a swatch: `BAYER[dy mod 4][dx]`.
    Y,
}

/// ImageMagick grayN: N percent of white.
pub const fn gray(n: u32) -> Rgb {
    let v = ((n * 255 + 50) / 100) as u8;
    [v, v, v]
}

/// The color of all secondary text and marks (gray25, `#404040`; NAV-19, QST-5). No text or mark
/// uses gray40: it was hard to read on the tablet.
pub const SECONDARY: Rgb = gray(25);

/// The units of the coordinates that a [`Canvas`] gets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Units {
    /// Screen pixels (the game screen and the menu).
    Screen,
    /// The layout units of the manual and the analytics pages: `layout::page_scale()` screen pixels.
    Page,
}

impl Units {
    /// The quantity of screen pixels in one unit.
    fn scale(self) -> f32 {
        match self {
            Units::Screen => 1.0,
            Units::Page => page_scale(),
        }
    }

    /// The offset of the text (screen px) that gives the smallest difference to the reference images.
    fn text_offset(self) -> (f32, f32) {
        let raster = layout::device().raster;
        match self {
            Units::Screen => raster.text_offset,
            Units::Page => raster.page_text_offset,
        }
    }
}

pub struct Canvas<'a> {
    pm: &'a mut Pixmap,
    pub fonts: &'a Fonts,
    units: Units,
    /// The record of the frame (refer to [`FrameRecord`]). None: each drawing writes the full frame.
    record: Option<&'a mut FrameRecord>,
    /// The screen row of the first row of `pm` (not 0 only for a stripe of the board, refer to
    /// `draw_board_stripes`).
    top: i32,
    /// True while `ink` draws: the drawing is ink of the pen.
    ink: bool,
}

/// An area of the screen in px (x1 and y1 are not included).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Area {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
}

impl Area {
    fn xywh(x: usize, y: usize, w: usize, h: usize) -> Area {
        Area { x0: x as i32, y0: y as i32, x1: (x + w) as i32, y1: (y + h) as i32 }
    }

    /// The pixels that a drawing in the box `b` (screen px) can change, with `margin` px more on
    /// each side, in the frame of `w` x `h` px. None if no pixel of the frame.
    fn around(b: SkRect, margin: f32, w: u32, h: u32) -> Option<Area> {
        let a = Area {
            x0: ((b.left() - margin).floor() as i32).max(0),
            y0: ((b.top() - margin).floor() as i32).max(0),
            x1: ((b.right() + margin).ceil() as i32 + 1).min(w as i32),
            y1: ((b.bottom() + margin).ceil() as i32 + 1).min(h as i32),
        };
        (a.x0 < a.x1 && a.y0 < a.y1).then_some(a)
    }

    fn overlaps(&self, o: &Area) -> bool {
        self.x0 < o.x1 && o.x0 < self.x1 && self.y0 < o.y1 && o.y0 < self.y1
    }

    fn contains(&self, o: &Area) -> bool {
        self.x0 <= o.x0 && self.y0 <= o.y0 && o.x1 <= self.x1 && o.y1 <= self.y1
    }

    fn union(&self, o: &Area) -> Area {
        Area { x0: self.x0.min(o.x0), y0: self.y0.min(o.y0), x1: self.x1.max(o.x1), y1: self.y1.max(o.y1) }
    }
}

/// The areas of a frame that changed since the last [`FrameRecord::take_damage`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Damage {
    /// All pixels can have changed.
    All,
    /// Only the pixels in these areas can have changed.
    Areas(Vec<Area>),
}

/// More damage areas than this become `Damage::All`.
const MAX_DAMAGE: usize = 1024;
/// More marks than this become one mark: the box of all marks.
const MAX_MARKS: usize = 256;

/// What a frame contains after the drawings of a [`Canvas::retained`] on it. With it, the next
/// drawing of the game screen writes only the areas that can change: the tiles of the board that
/// are not in the frame, and the areas of the other drawings. It also gives the damage: the areas
/// that the presenter must compare with the screen.
pub struct FrameRecord {
    /// None: the record does not know the pixels of the frame.
    known: Option<Known>,
    damage: Damage,
    /// The board tiles that the screen shows: the slots at the last copy of the frame to the screen
    /// (`take_damage`). None: not known.
    shown_tiles: Option<ShownTiles>,
    /// The areas of the drawings since the last copy that are not board tiles and not ink: a panel,
    /// a dialog, or a screen that is not the game.
    covers: Damage,
}

impl Default for FrameRecord {
    fn default() -> FrameRecord {
        FrameRecord { known: None, damage: Damage::All, shown_tiles: None, covers: Damage::All }
    }
}

/// The board tiles at a copy of the frame to the screen (refer to `FrameRecord::take_damage`).
struct ShownTiles {
    size: (u32, u32),
    board: (i32, i32),
    /// The tile of each board slot (refer to `Known::slots`).
    tiles: Vec<Option<TileKey>>,
}

/// The pixels of a frame of the game screen.
struct Known {
    /// The size of the frame, and the board origin (the hand of the player moves the board).
    size: (u32, u32),
    board: (i32, i32),
    /// The area of each board slot (refer to `board_slots`) and the tile in it. None: the slot has
    /// other pixels.
    slots: Vec<(Area, Option<TileKey>)>,
    /// The areas of the drawings after the board tiles. Out of the board, all other pixels are white.
    marks: Vec<Area>,
}

impl FrameRecord {
    /// The damage since the last call. The caller copies the frame to the screen now: the record
    /// keeps the board tiles of this frame as the tiles that the screen shows.
    pub fn take_damage(&mut self) -> Damage {
        self.shown_tiles = self.known.as_ref().map(|k| ShownTiles { size: k.size, board: k.board, tiles: k.slots.iter().map(|(_, t)| *t).collect() });
        self.covers = Damage::Areas(Vec::new());
        std::mem::replace(&mut self.damage, Damage::Areas(Vec::new()))
    }

    /// The items of the board cell `cell` that the pen waveform shows after the change of the cell
    /// since the last copy of the frame to the screen (R§14.4), or None if the change needs the UI
    /// waveform. `drawn`: the items of the cell that the pen waveform drew (the pen region stayed on
    /// the cell since then). The pen waveform shows the change when the cell is an empty cell on the
    /// screen and in the frame, with the same micro-cell lines, thus only its pencil marks, its x or
    /// the ink changed (crisp drawings, exactly black or gray15 on white, MRK-19 and INP-25), and when
    /// each mark or x that goes away is in `drawn`. A mark that the UI waveform drew (for example
    /// before the region came to the cell) has colored dots of the color dither on the panel: the pen
    /// waveform cannot remove them, and a ghost of the mark stays. The UI waveform is also necessary
    /// when the cell has a number or a "?" (gray25), when the lines changed, when a panel, a dialog
    /// or a different screen covers it, or when the record does not know the frame or the screen.
    pub fn pen_change(&self, cell: crate::game::Cell, drawn: PenDrawn) -> Option<PenDrawn> {
        let (Some(k), Some(shown)) = (&self.known, &self.shown_tiles) else { return None };
        if shown.size != k.size || shown.board != k.board {
            return None;
        }
        let i = cells().position(|c| c == cell)?;
        let (slot, tile) = &k.slots[i];
        let covered = match &self.covers {
            Damage::All => true,
            Damage::Areas(v) => v.iter().any(|a| a.overlaps(slot)),
        };
        if covered {
            return None;
        }
        CellLook::pen_change(shown.tiles[i].and_then(|t| t.5), tile.and_then(|t| t.5), drawn)
    }

    fn damage(&mut self, a: Area) {
        push_bounded(&mut self.damage, a);
    }

    /// The frame has pixels that the record does not know.
    fn forget(&mut self) {
        self.known = None;
        self.damage = Damage::All;
        self.covers = Damage::All;
    }

    /// A drawing (not a board tile) changed the pixels of `a`. `ink`: the drawing is ink of the pen,
    /// which the pen waveform shows (refer to `pen_change`).
    fn touch(&mut self, a: Area, ink: bool) {
        self.damage(a);
        if !ink {
            push_bounded(&mut self.covers, a);
        }
        if let Some(k) = &mut self.known {
            if k.marks.last().is_none_or(|l| !l.contains(&a)) {
                k.marks.push(a);
            }
            if k.marks.len() > MAX_MARKS {
                let all = k.marks.iter().skip(1).fold(k.marks[0], |b, m| b.union(m));
                k.marks = vec![all];
            }
        }
    }
}

/// Adds `a` to the areas of `d`. With more than MAX_DAMAGE areas, `d` becomes `Damage::All`.
fn push_bounded(d: &mut Damage, a: Area) {
    if let Damage::Areas(v) = d {
        if v.len() >= MAX_DAMAGE {
            *d = Damage::All;
        } else if v.last().is_none_or(|l| !l.contains(&a)) {
            v.push(a);
        }
    }
}

impl Known {
    fn unset_slots(&mut self, a: &Area) {
        for (slot, tile) in &mut self.slots {
            if slot.overlaps(a) {
                *tile = None;
            }
        }
    }
}

/// The paint of a drawing of the color `c`. All drawings are crisp (no anti-aliasing, NAV-20): each
/// pixel is fully `c`, or it stays the pixel that was under it.
fn paint(c: Rgb) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color_rgba8(c[0], c[1], c[2], 255);
    p.anti_alias = false;
    p
}

fn stroke(w: f32, round: bool) -> Stroke {
    Stroke {
        width: w + layout::device().raster.stroke_extra,
        line_cap: if round { LineCap::Round } else { LineCap::Butt },
        line_join: if round { LineJoin::Round } else { LineJoin::Miter },
        ..Stroke::default()
    }
}

impl<'a> Canvas<'a> {
    pub fn new(pm: &'a mut Pixmap, fonts: &'a Fonts) -> Canvas<'a> {
        Canvas { pm, fonts, units: Units::Screen, record: None, top: 0, ink: false }
    }

    /// A canvas that keeps `record` (refer to [`FrameRecord`]). All drawings on `pm` after the first
    /// drawing with `record` must use the same record.
    pub fn retained(pm: &'a mut Pixmap, record: &'a mut FrameRecord, fonts: &'a Fonts) -> Canvas<'a> {
        Canvas { pm, fonts, units: Units::Screen, record: Some(record), top: 0, ink: false }
    }

    /// Records that a drawing in the box `b` (screen px), with `margin` px more on each side, can
    /// change pixels.
    fn touched(&mut self, b: SkRect, margin: f32) {
        let (w, h) = (self.pm.width(), self.pm.height());
        let ink = self.ink;
        if let (Some(rec), Some(a)) = (self.record.as_deref_mut(), Area::around(b, margin, w, h)) {
            rec.touch(a, ink);
        }
    }

    /// The paint of the color `c`.
    fn paint(&self, c: Rgb) -> Paint<'static> {
        paint(c)
    }

    /// Draws one stroke of the ink of the pen (`InkMetrics::width`, black): a dot of
    /// `InkMetrics::dot_radius` for one point, else a line with round ends and joins. The ink is a
    /// crisp drawing (INP-25): each pixel is black or the pixel under it, also over a filled cell.
    /// The ink is not a drawing that covers a board cell (refer to `FrameRecord::pen_change`).
    pub fn ink(&mut self, stroke: &[(f32, f32)]) {
        let ink = layout::device().ink;
        self.ink = true;
        match stroke {
            [] => {}
            [(x, y)] => self.circle(*x, *y, ink.dot_radius, Some(BLACK), None),
            _ => self.polyline(stroke, ink.width, BLACK, true),
        }
        self.ink = false;
    }

    /// Writes the dots of the pattern of the micro-cell lines (BRD-9) in the box with its top left at
    /// (x, y) (screen px) and the size `w` x `h` px: the pixel (dx, dy) of the box, from its top left,
    /// gets gray15 (`LINE_DOT`) when `keep(dx, dy)` is true and the pattern of the gray `gray` has a
    /// dot there (`pattern_dot`; the position along the line is dy for `Axis::Y` and dx for
    /// `Axis::X`). The other pixels keep what is under them, thus two patterns that cross give the
    /// union. A direct write of pixels, with no anti-aliasing: no pixel gets an intermediate value,
    /// and the pixels are a function of the position in the box and of the gray only (NAV-20), on
    /// each thread (a stripe with `top`) and in each frame.
    pub fn pattern(&mut self, x: i32, y: i32, w: i32, h: i32, along: Axis, gray: u8, keep: impl Fn(i32, i32) -> bool) {
        debug_assert_eq!(self.units, Units::Screen, "the pattern is in screen pixels");
        let (fw, fh, top) = (self.pm.width() as i32, self.pm.height() as i32, self.top);
        let data = self.pm.data_mut();
        let dot = [LINE_DOT[0], LINE_DOT[1], LINE_DOT[2], 255];
        for dy in 0..h {
            let row = y + dy - top;
            if row < 0 || row >= fh {
                continue;
            }
            for dx in (0..w).filter(|&dx| (0..fw).contains(&(x + dx))) {
                let (along_px, across_px) = match along {
                    Axis::X => (dx, dy),
                    Axis::Y => (dy, dx),
                };
                if keep(dx, dy) && pattern_dot(gray, along_px, across_px) {
                    let o = ((row * fw + x + dx) * 4) as usize;
                    data[o..o + 4].copy_from_slice(&dot);
                }
            }
        }
        if let Some(r) = SkRect::from_ltrb(x as f32, y as f32, (x + w) as f32, (y + h) as f32) {
            self.touched(r, 0.0);
        }
    }

    /// Records that a drawing can change all pixels.
    fn touched_all(&mut self) {
        if let Some(rec) = self.record.as_deref_mut() {
            rec.forget();
        }
    }

    /// Records a drawing of `path` with `tf`. A stroke of the width `w` (before the transform) goes
    /// out of the path by less than 2 w: the miter limit of tiny-skia is 4 half widths.
    fn touched_path(&mut self, path: &Path, tf: Transform, w: f32) {
        let b = path.bounds();
        let mut pts = [
            tiny_skia::Point::from_xy(b.left(), b.top()),
            tiny_skia::Point::from_xy(b.right(), b.top()),
            tiny_skia::Point::from_xy(b.left(), b.bottom()),
            tiny_skia::Point::from_xy(b.right(), b.bottom()),
        ];
        tf.map_points(&mut pts);
        let (x0, x1) = pts.iter().fold((f32::MAX, f32::MIN), |(a, c), p| (a.min(p.x), c.max(p.x)));
        let (y0, y1) = pts.iter().fold((f32::MAX, f32::MIN), |(a, c), p| (a.min(p.y), c.max(p.y)));
        let scale = tf.sx.abs().max(tf.sy.abs()).max(tf.kx.abs()).max(tf.ky.abs());
        let raster = layout::device().raster;
        // A box with no width or height (a straight line) gets a small size.
        match SkRect::from_ltrb(x0, y0, x1.max(x0 + 0.01), y1.max(y0 + 0.01)) {
            Some(r) => self.touched(r, 2.0 * (w + raster.stroke_extra) * scale + raster.path_record_margin),
            None => self.touched_all(),
        }
    }

    /// Starts a drawing of the game screen: the area `board` (the board slots) gets its tiles in
    /// `draw_board`, and the other pixels must be white. With a record of the same layout, only the
    /// areas of the last drawings outside the tiles get white.
    fn start_game_frame(&mut self, board: (usize, usize, usize, usize)) {
        let size = (self.pm.width(), self.pm.height());
        let origin = (board_x(), board_y());
        let known = self.record.as_deref_mut().and_then(|r| r.known.as_mut()).filter(|k| k.size == size && k.board == origin);
        let Some(k) = known else {
            self.clear_except(board);
            if let Some(rec) = self.record.as_deref_mut() {
                rec.known = Some(Known { size, board: origin, slots: board_slots().map(|a| (a, None)).collect(), marks: Vec::new() });
                rec.damage = Damage::All;
            }
            return;
        };
        let marks = std::mem::take(&mut k.marks);
        let fw = size.0 as usize;
        let data = self.pm.data_mut();
        for a in &marks {
            k.unset_slots(a);
            for r in a.y0 as usize..a.y1 as usize {
                data[(r * fw + a.x0 as usize) * 4..(r * fw + a.x1 as usize) * 4].fill(255);
            }
        }
        let rec = self.record.as_deref_mut().unwrap();
        for a in marks {
            rec.damage(a);
        }
    }

    /// True if a drawing of this frame (after the board tiles) touched the area `a` (or the record
    /// does not know the frame).
    fn frame_touched(&self, a: &Area) -> bool {
        self.record.as_ref().and_then(|r| r.known.as_ref()).is_none_or(|k| k.marks.iter().any(|m| m.overlaps(a)))
    }

    /// True if the board slot `i` of the record has the tile `key`.
    fn slot_has(&self, i: usize, key: &TileKey) -> bool {
        self.record.as_ref().and_then(|r| r.known.as_ref()).is_some_and(|k| k.slots[i].1.as_ref() == Some(key))
    }

    /// The board slot `i` has the tile `key` now.
    fn set_slot(&mut self, i: usize, key: TileKey) {
        if let Some(rec) = self.record.as_deref_mut() {
            if let Some(k) = rec.known.as_mut() {
                k.slots[i].1 = Some(key);
                let a = k.slots[i].0;
                rec.damage(a);
            }
        }
    }

    fn tf(&self) -> Transform {
        let (scale, center) = (self.units.scale(), layout::device().raster.pixel_center_offset);
        Transform::from_row(scale, 0.0, 0.0, scale, center, center - self.top as f32)
    }

    pub fn clear(&mut self) {
        self.pm.fill(tiny_skia::Color::WHITE);
        self.touched_all();
    }

    /// Makes all pixels white, except the rectangle (x, y, w, h).
    fn clear_except(&mut self, (x, y, w, h): (usize, usize, usize, usize)) {
        let fw = self.pm.width() as usize;
        let data = self.pm.data_mut();
        data[..y * fw * 4].fill(255);
        for r in y..y + h {
            data[r * fw * 4..(r * fw + x) * 4].fill(255);
            data[(r * fw + x + w) * 4..(r + 1) * fw * 4].fill(255);
        }
        data[(y + h) * fw * 4..].fill(255);
    }

    /// ImageMagick `rectangle x0,y0 x1,y1` with a fill: the pixels from x0 to x1, x1 included.
    pub fn rect(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, c: Rgb) {
        let h = layout::device().raster.pixel_center_offset / self.units.scale();
        if let Some(r) = SkRect::from_ltrb(x0 - h, y0 - h, x1 + h, y1 + h) {
            let tf = self.tf();
            self.pm.fill_rect(r, &self.paint(c), tf, None);
            self.touched_path(&PathBuilder::from_rect(r), tf, 0.0);
        }
    }

    /// A rectangle with a transparent white fill (the pale list behind a dialog).
    fn veil(&mut self, alpha: f32) {
        self.touched_all();
        let mut p = Paint::default();
        p.set_color(tiny_skia::Color::from_rgba(1.0, 1.0, 1.0, alpha).unwrap());
        // On an opaque screen, each channel of the result depends only on the same channel of the
        // pixel. Thus tiny-skia draws the veil on the 256 levels once, and the table gives the result
        // of each pixel (the same values as tiny-skia on the full screen).
        if self.pm.data().chunks_exact(4).fold(255, |a, px| a & px[3]) == 255 {
            let mut levels = Pixmap::new(256, 1).unwrap();
            for (i, px) in levels.data_mut().chunks_exact_mut(4).enumerate() {
                px.copy_from_slice(&[i as u8, i as u8, i as u8, 255]);
            }
            levels.fill_rect(SkRect::from_xywh(0.0, 0.0, 256.0, 1.0).unwrap(), &p, Transform::identity(), None);
            let lut: Vec<u8> = levels.data().chunks_exact(4).map(|px| px[0]).collect();
            let gray = levels.data().chunks_exact(4).all(|px| px[0] == px[1] && px[0] == px[2]);
            // The table is the same as `s + (d * k + 255) / 256` (the blend of tiny-skia with 8 bits)
            // for one k: then this calculation is used, because it is faster than the table. It
            // keeps 255 as 255, thus it can also go on the alpha bytes (all 255): one simple loop
            // on all bytes is fast (SIMD).
            let s = lut[0] as u16;
            let k = (0..=255u16).find(|&k| (0..256u16).all(|d| s + ((d * k + 255) >> 8) == lut[d as usize] as u16));
            match k.filter(|_| gray && lut[255] == 255) {
                Some(k) => {
                    for v in self.pm.data_mut() {
                        *v = (s + ((*v as u16 * k + 255) >> 8)) as u8;
                    }
                }
                None => {
                    let lut: Vec<[u8; 3]> = levels.data().chunks_exact(4).map(|px| [px[0], px[1], px[2]]).collect();
                    for px in self.pm.data_mut().chunks_exact_mut(4) {
                        px[0] = lut[px[0] as usize][0];
                        px[1] = lut[px[1] as usize][1];
                        px[2] = lut[px[2] as usize][2];
                    }
                }
            }
            return;
        }
        let r = SkRect::from_xywh(0.0, 0.0, screen_w() as f32, screen_h() as f32).unwrap();
        self.pm.fill_rect(r, &p, Transform::identity(), None);
    }

    fn stroke_rect(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, w: f32, c: Rgb) {
        if let Some(r) = SkRect::from_ltrb(x0, y0, x1, y1) {
            let path = PathBuilder::from_rect(r);
            let tf = self.tf();
            self.pm.stroke_path(&path, &self.paint(c), &stroke(w, false), tf, None);
            self.touched_path(&path, tf, w);
        }
    }

    pub fn polyline(&mut self, pts: &[(f32, f32)], w: f32, c: Rgb, round: bool) {
        let mut pb = PathBuilder::new();
        pb.move_to(pts[0].0, pts[0].1);
        for p in &pts[1..] {
            pb.line_to(p.0, p.1);
        }
        if let Some(path) = pb.finish() {
            let tf = self.tf();
            self.pm.stroke_path(&path, &self.paint(c), &stroke(w, round), tf, None);
            self.touched_path(&path, tf, w);
        }
    }

    pub fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, w: f32, c: Rgb) {
        self.polyline(&[(x0, y0), (x1, y1)], w, c, true);
    }

    /// A line with flat ends.
    fn line_butt(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, w: f32, c: Rgb) {
        self.polyline(&[(x0, y0), (x1, y1)], w, c, false);
    }

    /// A circle with its center at (cx, cy). The circle is made near the origin and moved by a
    /// translation of whole units: the f32 rounding of the curves then does not depend on the
    /// position, thus a crisp circle has the same pixels at each position (NAV-20).
    pub fn circle(&mut self, cx: f32, cy: f32, r: f32, fill: Option<Rgb>, line: Option<(Rgb, f32)>) {
        let (ix, iy) = (cx.floor(), cy.floor());
        let Some(path) = PathBuilder::from_circle(cx - ix, cy - iy, r) else { return };
        let tf = self.tf().pre_translate(ix, iy);
        if let Some(f) = fill {
            self.pm.fill_path(&path, &self.paint(f), FillRule::Winding, tf, None);
        }
        if let Some((c, w)) = line {
            self.pm.stroke_path(&path, &self.paint(c), &stroke(w, true), tf, None);
        }
        self.touched_path(&path, tf, line.map_or(0.0, |l| l.1));
    }

    fn path_stroke(&mut self, pb: PathBuilder, tf: Transform, w: f32, c: Rgb) {
        if let Some(path) = pb.finish() {
            let tf = tf.post_concat(self.tf());
            self.pm.stroke_path(&path, &self.paint(c), &stroke(w, true), tf, None);
            self.touched_path(&path, tf, w);
        }
    }

    fn path_fill(&mut self, pb: PathBuilder, tf: Transform, c: Rgb) {
        if let Some(path) = pb.finish() {
            let tf = tf.post_concat(self.tf());
            self.pm.fill_path(&path, &self.paint(c), FillRule::Winding, tf, None);
            self.touched_path(&path, tf, 0.0);
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn text(&mut self, weight: Weight, size: f32, x: f32, y: f32, anchor: Anchor, s: &str, c: Rgb) {
        let font = self.fonts.get(weight);
        let tf = self.tf();
        let (dx, dy) = self.units.text_offset();
        let scale = self.units.scale();
        if let Some(path) = font.draw(self.pm, size, x + dx / scale, y + dy / scale, anchor, s, c, tf) {
            self.touched_path(&path, tf, 0.0);
        }
    }

    /// Sets the units of the coordinates of the next drawing operations.
    pub fn set_units(&mut self, units: Units) {
        self.units = units;
    }

    /// Calls `draw` with page units, then sets screen units again.
    fn in_page_units(&mut self, draw: impl FnOnce(&mut Self)) {
        self.set_units(Units::Page);
        draw(self);
        self.set_units(Units::Screen);
    }

    /// Draws img with its top left at (x, y). `mirror` draws its mirror image (left to right).
    pub fn image(&mut self, img: &Pixmap, x: i32, y: i32, mirror: bool) {
        let tf = if mirror { Transform::from_row(-1.0, 0.0, 0.0, 1.0, (2 * x + img.width() as i32) as f32, 0.0) } else { Transform::identity() };
        self.pm.draw_pixmap(x, y, img.as_ref(), &PixmapPaint::default(), tf, None);
        match SkRect::from_xywh(x as f32, y as f32, img.width() as f32, img.height() as f32) {
            Some(r) => self.touched(r, 1.0),
            None => self.touched_all(),
        }
    }
}

// ---- Images in the app

/// The manual pages (REQUIREMENTS.md §12) of the design images of a model (`dir`, refer to
/// `Model::images`), in order. `suffix` is "" for portrait and "-landscape" for landscape
/// (scripts/manual-*-landscape.sh).
macro_rules! manual_pages {
    ($dir:literal, $suffix:literal) => {
        [
            include_bytes!(concat!("../images/", $dir, "/manual-how-to-play", $suffix, ".png")),
            include_bytes!(concat!("../images/", $dir, "/manual-candidates", $suffix, ".png")),
            include_bytes!(concat!("../images/", $dir, "/manual-pen", $suffix, ".png")),
            include_bytes!(concat!("../images/", $dir, "/manual-pen-rules", $suffix, ".png")),
            include_bytes!(concat!("../images/", $dir, "/manual-helpers", $suffix, ".png")),
            include_bytes!(concat!("../images/", $dir, "/manual-mistakes", $suffix, ".png")),
            include_bytes!(concat!("../images/", $dir, "/manual-menu", $suffix, ".png")),
            include_bytes!(concat!("../images/", $dir, "/manual-new-game", $suffix, ".png")),
            include_bytes!(concat!("../images/", $dir, "/manual-finished", $suffix, ".png")),
        ]
    };
}

type ManualPngs = [&'static [u8]; MANUAL_PAGES as usize];
static MANUAL_PRO: ManualPngs = manual_pages!("pro", "");
static MANUAL_PRO_LANDSCAPE: ManualPngs = manual_pages!("pro", "-landscape");
static MANUAL_MOVE: ManualPngs = manual_pages!("move", "");
static MANUAL_MOVE_LANDSCAPE: ManualPngs = manual_pages!("move", "-landscape");

/// The manual pages of the device: portrait and landscape.
fn manual_pngs() -> (&'static ManualPngs, &'static ManualPngs) {
    match layout::device().model {
        layout::Model::PaperPro => (&MANUAL_PRO, &MANUAL_PRO_LANDSCAPE),
        layout::Model::Move => (&MANUAL_MOVE, &MANUAL_MOVE_LANDSCAPE),
    }
}

/// Decodes a PNG (gray, gray with alpha, RGB or RGBA; 8 or 16 bits) into a pixmap.
pub fn decode_png(data: &[u8]) -> Pixmap {
    let mut dec = png::Decoder::new(std::io::Cursor::new(data));
    dec.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = dec.read_info().expect("PNG header");
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).expect("PNG data");
    let (w, h) = (info.width, info.height);
    let mut pm = Pixmap::new(w, h).expect("pixmap");
    let out = pm.data_mut();
    let n = (w * h) as usize;
    // Premultiplied alpha. An opaque pixel keeps its values: (v * 255 + 127) / 255 = v.
    let premultiply = |px: &mut [u8], (r, g, b, a): (u8, u8, u8, u8)| {
        if a == 255 {
            px.copy_from_slice(&[r, g, b, 255]);
        } else {
            let pm_ = |v: u8| ((v as u32 * a as u32 + 127) / 255) as u8;
            px.copy_from_slice(&[pm_(r), pm_(g), pm_(b), a]);
        }
    };
    let out = out[..4 * n].chunks_exact_mut(4);
    match info.color_type {
        png::ColorType::Grayscale => out.zip(&buf[..n]).for_each(|(px, &v)| premultiply(px, (v, v, v, 255))),
        png::ColorType::GrayscaleAlpha => out.zip(buf.chunks_exact(2)).for_each(|(px, s)| premultiply(px, (s[0], s[0], s[0], s[1]))),
        png::ColorType::Rgb => out.zip(buf.chunks_exact(3)).for_each(|(px, s)| premultiply(px, (s[0], s[1], s[2], 255))),
        png::ColorType::Rgba => out.zip(buf.chunks_exact(4)).for_each(|(px, s)| premultiply(px, (s[0], s[1], s[2], s[3]))),
        png::ColorType::Indexed => unreachable!("expanded by the decoder"),
    }
    pm
}

/// A manual page, and true if all its pixels are opaque.
fn manual_page(page: u8) -> &'static (Pixmap, bool) {
    static PAGES: [OnceLock<(Pixmap, bool)>; MANUAL_PAGES as usize] = [const { OnceLock::new() }; MANUAL_PAGES as usize];
    static LANDSCAPE: [OnceLock<(Pixmap, bool)>; MANUAL_PAGES as usize] = [const { OnceLock::new() }; MANUAL_PAGES as usize];
    let i = (page.clamp(1, MANUAL_PAGES) - 1) as usize;
    let load = |png: &[u8]| {
        let pm = decode_png(png);
        let opaque = pm.pixels().iter().all(|p| p.alpha() == 255);
        (pm, opaque)
    };
    let (portrait, landscape) = manual_pngs();
    if layout::orientation() == layout::Orientation::Landscape {
        LANDSCAPE[i].get_or_init(|| load(landscape[i]))
    } else {
        PAGES[i].get_or_init(|| load(portrait[i]))
    }
}

// ---- Screens

/// Draws the full screen for the state of the app.
pub fn draw(c: &mut Canvas, app: &App) {
    // `draw_board` writes all pixels of the board area: the clear does not need to write them.
    match board_area(c.pm).filter(|_| app.screen == Screen::Game) {
        Some(a) => c.start_game_frame(a),
        None => c.clear(),
    }
    match app.screen {
        Screen::Game => draw_game(c, app),
        Screen::Manual => {
            let (page, opaque) = manual_page(app.manual_page);
            if *opaque && page.width() == c.pm.width() && page.height() == c.pm.height() {
                // An opaque page covers the full screen: the result of the drawing is the page.
                c.pm.data_mut().copy_from_slice(page.data());
                c.touched_all();
            } else {
                c.image(page, 0, 0, false);
            }
        }
        Screen::Statistics => c.in_page_units(|c| draw_summary(c, &app.results, app.confirm_all)),
        Screen::GamesList => c.in_page_units(|c| draw_list(c, app)),
        Screen::Licenses => c.in_page_units(|c| draw_licenses(c, app)),
        Screen::LicenseText => c.in_page_units(|c| draw_license_text(c, app)),
    }
    if app.update.dialog_shown() {
        c.in_page_units(restart_dialog);
    }
}

/// The font of the two labels of the difficulty slider. The size is `Device::slider`.
const SLIDER_LABEL_WEIGHT: Weight = Weight::Medium;

/// The text widths of the labels "Easy" and "Hard" of the difficulty slider.
pub fn slider_label_widths(fonts: &Fonts) -> (f32, f32) {
    let font = fonts.get(SLIDER_LABEL_WEIGHT);
    let size = layout::device().slider.label_size;
    (font.width(size, "Easy"), font.width(size, "Hard"))
}

/// The x positions of the stops of a difficulty slider with its labels from `xl` to `xr`.
pub fn slider_stops(fonts: &Fonts, xl: i32, xr: i32) -> [f32; STOPS as usize] {
    let (easy_w, hard_w) = slider_label_widths(fonts);
    layout::slider_stops(xl, xr, easy_w, hard_w)
}

/// The x positions of the stops of the New game slider (layout.rs).
pub fn ng_stops(fonts: &Fonts) -> [f32; STOPS as usize] {
    slider_stops(fonts, ng_tx(), ng_tr())
}

fn draw_game(c: &mut Canvas, app: &App) {
    let g = &app.game;
    let overlays = BoardOverlays {
        highlight: app.ui.highlight,
        question: app.ui.question.map(|(cell, _)| cell),
        refused: app.ui.refused.map(|(cell, digit, _)| (cell, digit)),
    };
    draw_board(c, g, &app.settings, overlays);

    // Ink that waits for the recognition pause, and the stroke that the pen draws now.
    let ink = app.ui.pending_ink.iter().flat_map(|i| i.strokes.iter()).chain(app.live_stroke.iter());
    for s in ink {
        c.ink(s);
    }

    // The difficulty meter, or the result card in its place (CMP-4, DIF-6).
    if g.completed {
        let look = OverlayLook::Card { mode: g.mode, seconds: g.seconds(), comparison: app.card_comparison() };
        put_overlay(c, look, |c| draw_result_card(c, g.mode, g.seconds(), app.card_comparison()));
    } else {
        put_overlay(c, OverlayLook::Meter(g.mode), |c| labeled_slider(c, meter_xl(), meter_xr(), meter_y(), g.mode));
    }

    // The menu button. Its icon shows the menu state (NAV-7). The toolbar of a right-handed player
    // shows the mirror image of the icon.
    let menu = OverlayLook::Menu {
        panel: app.ui.menu.is_open().then(|| app.ui.menu.panel()),
        caution: app.update.caution(),
        undo: app.game.can_undo(),
        redo: app.game.can_redo(),
        bookmark: !app.game.completed,
    };
    put_overlay(c, menu, |c| {
        let (x, y) = icon_center(Button::Menu);
        menu_icon(c, x, y, app.ui.menu.is_open(), layout::right_handed(), BLACK);
        if let Menu::Open(panel) = app.ui.menu {
            draw_toolbar(c, app, panel);
        }
    });
    if let Menu::Open(Some(panel)) = app.ui.menu {
        draw_panel(c, app, panel);
    }
}

/// The result card of a complete puzzle (REQUIREMENTS.md §8.3, CMP-4, CMP-7 to CMP-9): "Completed
/// in", the time, the meter of the mode, the comparison with the earlier results at this mode, and
/// the level buttons Easier, Same and Harder. The three buttons have the same look (Same has no
/// highlight, `ButtonLook`). The contents of a landscape column are the same for the two hands.
fn draw_result_card(c: &mut Canvas, mode: u8, seconds: u64, comparison: Comparison) {
    let r = layout::result_card();
    card_frame(c, r, BLACK);
    let time = format_clock(seconds);
    match layout::card_layout() {
        CardLayout::Below(card) => draw_card_below(c, card, r, mode, &time, comparison),
        CardLayout::Column(card) => draw_card_column(c, card, r, mode, &time, comparison),
        CardLayout::CompactBelow(card) => draw_compact_card_below(c, card, r, mode, &time, comparison),
    }
}

/// The look of a level button: an available button has a black frame, a black label and the meter
/// of its target mode. A button that is not available has a gray80 frame, a gray70 label and no
/// meter.
struct ButtonLook {
    frame: Rgb,
    label: Rgb,
    target: Option<u8>,
}

impl ButtonLook {
    fn of(b: LevelButton, mode: u8) -> ButtonLook {
        match b.target(mode) {
            Some(m) => ButtonLook { frame: BLACK, label: BLACK, target: Some(m) },
            None => ButtonLook { frame: gray(80), label: gray(70), target: None },
        }
    }
}

/// The x of the first stop of a meter with `step` between its stops, centered at `cx`.
fn centered_meter_x(cx: f32, step: f32) -> f32 {
    cx - (MODES - 1) as f32 / 2.0 * step
}

/// The comparison of a portrait card in one line.
fn comparison_line(comparison: Comparison) -> String {
    match comparison {
        Comparison::Earlier { average, best } => format!("Average {}  ·  Previous best {}", format_clock(average), format_clock(best)),
        Comparison::First => FIRST_GAME.to_string(),
    }
}

/// A line of text of the result card at `x`, with its baseline `line.baseline` below `top`.
#[allow(clippy::too_many_arguments)]
fn card_text(c: &mut Canvas, weight: Weight, line: CardText, x: f32, top: f32, anchor: Anchor, s: &str, color: Rgb) {
    c.text(weight, line.size, x, top + line.baseline as f32, anchor, s, color);
}

/// A small meter of the result card at the center `cx`, with its center line `m.y` below `top`.
fn card_meter(c: &mut Canvas, m: CardMeter, cx: f32, top: f32, level: u8) {
    meter(c, centered_meter_x(cx, m.step), top + m.y as f32, m.step, level, MeterStyle::ON_WHITE);
}

/// The comparison of a landscape card (CMP-8) at `x`, below the top `y0` of the card.
fn comparison_column(c: &mut Canvas, sizes: ComparisonColumn, x: f32, y0: f32, comparison: Comparison) {
    match comparison {
        Comparison::Earlier { average, best } => {
            for (k, (label, v)) in [("Average at this level", average), ("Previous best", best)].into_iter().enumerate() {
                let y = y0 + sizes.label.baseline as f32 + k as f32 * sizes.pitch as f32;
                c.text(Weight::Regular, sizes.label.size, x, y, Anchor::Start, label, SECONDARY);
                card_text(c, Weight::Medium, sizes.value, x, y, Anchor::Start, &format_clock(v), BLACK);
            }
        }
        Comparison::First => card_text(c, Weight::Regular, sizes.label, x, y0, Anchor::Start, FIRST_GAME, SECONDARY),
    }
}

/// The level buttons of a card with `LabelAboveMeter` contents: each label above its meter, at the
/// center of the button.
fn level_buttons_above(c: &mut Canvas, contents: LabelAboveMeter, mode: u8) {
    for b in LevelButton::ALL {
        let (br, look) = (layout::level_button(b), ButtonLook::of(b, mode));
        let (by0, bcx) = (br.y0 as f32, (br.x0 as f32 + br.x1 as f32) / 2.0);
        card_frame(c, br, look.frame);
        card_text(c, Weight::Medium, contents.label, bcx, by0, Anchor::Middle, b.label(), look.label);
        if let Some(m) = look.target {
            card_meter(c, contents.meter, bcx, by0, m);
        }
    }
}

/// `CardLayout::Below`: the result at the left, a vertical divider, then "New game" and a column
/// of level buttons, each with its label at the left and its meter at the right.
fn draw_card_below(c: &mut Canvas, card: BelowCard, r: Rect, mode: u8, time: &str, comparison: Comparison) {
    let (y0, y1) = (r.y0 as f32, r.y1 as f32);
    let divider = layout::card_divider() as f32;
    let text_x = r.x0 + card.text_x;
    let tx = text_x as f32;
    card_text(c, Weight::Regular, card.heading, tx, y0, Anchor::Start, "Completed in", SECONDARY);
    card_text(c, Weight::Medium, card.time, tx, y0, Anchor::Start, time, BLACK);
    labeled_slider(c, text_x, text_x + layout::device().slider.width, r.y0 + card.slider_y, mode);
    card_text(c, Weight::Regular, card.comparison, tx, y0, Anchor::Start, &comparison_line(comparison), SECONDARY);
    c.rect(divider - 1.0, y0 + card.divider_top as f32, divider, y1 - card.divider_bottom as f32, gray(70));
    let title_x = divider + card.buttons.inset as f32;
    card_text(c, Weight::Medium, card.title, title_x, y0, Anchor::Start, "New game", BLACK);
    let contents = card.button;
    for b in LevelButton::ALL {
        let (br, look) = (layout::level_button(b), ButtonLook::of(b, mode));
        let (bx0, by0, bx1, by1) = (br.x0 as f32, br.y0 as f32, br.x1 as f32, br.y1 as f32);
        let cy = (by0 + by1) / 2.0;
        card_frame(c, br, look.frame);
        card_text(c, Weight::Medium, contents.label, bx0 + contents.label_x as f32, cy, Anchor::Start, b.label(), look.label);
        if let Some(m) = look.target {
            let step = contents.meter_step;
            meter(c, bx1 - contents.meter_right as f32 - (MODES - 1) as f32 * step, cy, step, m, MeterStyle::ON_WHITE);
        }
    }
}

/// `CardLayout::Column` (the normal card and the compact card): the result at the top, a horizontal
/// divider, then "New game" (if the card has a title) and a column of level buttons, each with its
/// label above its meter.
fn draw_card_column(c: &mut Canvas, card: ColumnCard, r: Rect, mode: u8, time: &str, comparison: Comparison) {
    let (x0, y0, x1) = (r.x0 as f32, r.y0 as f32, r.x1 as f32);
    let divider = layout::card_divider() as f32;
    let (tx, cx) = (x0 + card.text_x as f32, (x0 + x1) / 2.0);
    card_text(c, Weight::Regular, card.heading, tx, y0, Anchor::Start, "Completed in", SECONDARY);
    card_text(c, Weight::Medium, card.time, tx, y0, Anchor::Start, time, BLACK);
    card_meter(c, card.meter, cx, y0, mode);
    comparison_column(c, card.comparison, tx, y0, comparison);
    c.rect(tx, divider - 1.0, x1 - card.divider_right as f32, divider, gray(70));
    if let Some(title) = card.title {
        card_text(c, Weight::Medium, title, tx, divider, Anchor::Start, "New game", BLACK);
    }
    level_buttons_above(c, card.button, mode);
}

/// `CardLayout::CompactBelow` (a small screen, owner decision 2026-11-10): the contents of
/// `draw_card_below` with smaller spaces. The result at the top, a horizontal divider, then the
/// three level buttons in one row, each with its label above its meter. No "New game" title.
fn draw_compact_card_below(c: &mut Canvas, card: CompactBelowCard, r: Rect, mode: u8, time: &str, comparison: Comparison) {
    let (x0, y0, x1) = (r.x0 as f32, r.y0 as f32, r.x1 as f32);
    let divider = layout::card_divider() as f32;
    let text_x = r.x0 + card.text_x;
    let tx = text_x as f32;
    card_text(c, Weight::Regular, card.heading, tx, y0, Anchor::Start, "Completed in", SECONDARY);
    card_text(c, Weight::Medium, card.time, tx, y0, Anchor::Start, time, BLACK);
    labeled_slider(c, text_x, text_x + layout::device().slider.width, r.y0 + card.slider_y, mode);
    card_text(c, Weight::Regular, card.comparison, tx, y0, Anchor::Start, &comparison_line(comparison), SECONDARY);
    c.rect(x0 + card.divider_left as f32, divider - 1.0, x1 - card.divider_right as f32, divider, gray(70));
    level_buttons_above(c, card.button, mode);
}

/// A frame of 2 px of the result card or of a level button: the pixel columns x0 - 1, x0, x1 - 1
/// and x1, and the same rows (ui-lib.sh `card_frame`). It has sharp edges.
fn card_frame(c: &mut Canvas, r: Rect, color: Rgb) {
    let (x0, y0, x1, y1) = (r.x0 as f32, r.y0 as f32, r.x1 as f32, r.y1 as f32);
    c.rect(x0 - 1.0, y0 - 1.0, x1, y0, color);
    c.rect(x0 - 1.0, y1 - 1.0, x1, y1, color);
    c.rect(x0 - 1.0, y0 - 1.0, x0, y1, color);
    c.rect(x1 - 1.0, y0 - 1.0, x1, y1, color);
}

/// The text of the result card with no earlier result at the mode (CMP-8).
const FIRST_GAME: &str = "First game at this level";

/// A group of drawings out of the board that the overlay cache keeps (refer to `put_overlay`): all
/// that its pixels depend on, with the layout.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum OverlayLook {
    /// The difficulty meter at this mode.
    Meter(u8),
    /// The result card of a complete game of this mode and time (`draw_result_card`).
    Card { mode: u8, seconds: u64, comparison: Comparison },
    /// The menu button and, if `panel` is Some, the toolbar with the button of this panel active
    /// (`draw_toolbar`, without the panel). `bookmark` is false when the bookmark is gray (CKP-9).
    Menu { panel: Option<Option<Panel>>, caution: bool, undo: bool, redo: bool, bookmark: bool },
}

/// The key of a cached overlay: the fonts (address), the layout (rotation and hand) and the look.
type OverlayKey = (usize, layout::Rotation, bool, OverlayLook);

/// The pixels of each cached overlay: one piece for each area of its drawings.
type OverlayCache = std::collections::HashMap<OverlayKey, Vec<(Area, Pixmap)>, std::hash::BuildHasherDefault<FxHasher>>;

/// More cached overlays than this: the cache starts again.
const MAX_OVERLAYS: usize = 64;

thread_local! {
    static OVERLAYS: std::cell::RefCell<OverlayCache> =
        std::cell::RefCell::new(Default::default());
}

/// Draws the group `draw` with the look `look`. On a frame with a record, the pixels come from the
/// cache. A copy of the cached area gives the same pixels as the drawing only if the area is white:
/// no other drawing of this frame and no board tile is in it. Else (and without a record) the group
/// is drawn. A group that was drawn on a white area goes into the cache.
fn put_overlay(c: &mut Canvas, look: OverlayLook, draw: impl FnOnce(&mut Canvas)) {
    let board = board_area(c.pm).map(|(x, y, w, h)| Area::xywh(x, y, w, h));
    let (Some(board), true) = (board, c.record.as_ref().is_some_and(|r| r.known.is_some())) else {
        draw(c);
        return;
    };
    let key = (c.fonts as *const Fonts as usize, layout::rotation(), layout::right_handed(), look);
    let copied = OVERLAYS.with(|o| {
        let o = o.borrow();
        let pieces = o.get(&key).filter(|p| !p.iter().any(|(a, _)| c.frame_touched(a)))?;
        for (a, tile) in pieces {
            copy_px(tile, 0, 0, tile.width() as usize, tile.height() as usize, c.pm, a.x0 as usize, a.y0 as usize);
        }
        Some(pieces.iter().map(|(a, _)| *a).collect::<Vec<_>>())
    });
    if let Some(areas) = copied {
        let rec = c.record.as_deref_mut().unwrap();
        for a in areas {
            rec.touch(a, false);
        }
        return;
    }
    // Draw the group, with its own list of areas.
    let earlier = std::mem::take(&mut c.record.as_deref_mut().unwrap().known.as_mut().unwrap().marks);
    draw(c);
    let Some(k) = c.record.as_deref_mut().and_then(|r| r.known.as_mut()) else {
        // The group changed pixels that the record does not know.
        return;
    };
    let group = std::mem::replace(&mut k.marks, earlier);
    // The group was drawn on white if no earlier drawing of this frame and no tile is in its areas.
    let on_white = !group.is_empty() && group.iter().all(|a| !a.overlaps(&board) && !k.marks.iter().any(|m| m.overlaps(a)));
    if on_white {
        let pieces = group
            .iter()
            .map(|a| {
                let (x, y, w, h) = (a.x0 as usize, a.y0 as usize, (a.x1 - a.x0) as usize, (a.y1 - a.y0) as usize);
                let mut tile = Pixmap::new(w as u32, h as u32).unwrap();
                copy_px(c.pm, x, y, w, h, &mut tile, 0, 0);
                (*a, tile)
            })
            .collect();
        OVERLAYS.with(|o| {
            let mut o = o.borrow_mut();
            if o.len() >= MAX_OVERLAYS {
                o.clear();
            }
            o.insert(key, pieces);
        });
    }
    k.marks.extend(group);
}

/// The look of one cell: all that its pixels depend on.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum CellLook {
    /// A number: the digit, the background and the color of the digit.
    Value(u8, Rgb, Rgb),
    /// The "?" of a number that the app did not know (INP-7).
    Question,
    /// An empty cell with its pencil marks, the gray of its micro-cell lines (`LineGray::None`: no
    /// lines; the lines show the gray as a pattern, BRD-9) and the digit of the micro-cell that shows
    /// the x of a refused tap (None: no x).
    Empty(crate::game::Marks, LineGray, Option<u8>),
}

impl CellLook {
    /// The items that the pen waveform shows after the change of a cell from the look `from` to the
    /// look `to` (R§14.4), or None if the change needs the UI waveform. The two looks must be empty
    /// cells with the same micro-cell lines, thus only the pencil marks and the x differ (gray15 on
    /// white, as the ink), and each mark or x that goes away must be in `drawn` (the items that the
    /// pen waveform drew). A number with its color, the "?" (gray25) and a change of the lines need
    /// the UI waveform. None is the band of the board.
    fn pen_change(from: Option<CellLook>, to: Option<CellLook>, drawn: PenDrawn) -> Option<PenDrawn> {
        let (Some(CellLook::Empty(ma, la, xa)), Some(CellLook::Empty(mb, lb, xb))) = (from, to) else { return None };
        let removed = ma & !mb;
        if la != lb || removed & !drawn.marks != 0 || (xa.is_some() && xa != xb && drawn.refused != xa) {
            return None;
        }
        let refused = xb.filter(|_| xb != xa || drawn.refused == xb);
        Some(PenDrawn { marks: (drawn.marks | (mb & !ma)) & mb, refused })
    }
}

/// The pencil marks and the x of a refused tap that the pen waveform drew in the cell of the pen
/// region (refer to `FrameRecord::pen_change`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PenDrawn {
    pub marks: crate::game::Marks,
    /// The digit of the micro-cell of the x.
    pub refused: Option<u8>,
}

/// The temporary items on the board (§3.2): the digit of the highlight, the cell of the "?", and the
/// cell and the digit of the x.
#[derive(Clone, Copy)]
struct BoardOverlays {
    highlight: Option<u8>,
    question: Option<crate::game::Cell>,
    refused: Option<(crate::game::Cell, u8)>,
}

/// The cached areas of the board (refer to `draw_board`). An area is drawn at its true position: the
/// float rounding of the glyph outlines changes with the position.
#[derive(Default)]
struct TileCache {
    /// The pixels of each area, and the last frame that used them.
    map: std::collections::HashMap<TileKey, (Pixmap, u64), std::hash::BuildHasherDefault<FxHasher>>,
    /// The number of the frame (`draw_board` calls).
    frame: u64,
}

impl TileCache {
    /// Starts a frame. If the cache is full, it keeps only the tiles of the last frame.
    fn begin_frame(&mut self) {
        self.frame += 1;
        if self.map.len() > MAX_TILES {
            let last = self.frame - 1;
            self.map.retain(|_, (_, used)| *used == last);
        }
    }

    fn get(&mut self, key: &TileKey) -> Option<&Pixmap> {
        let frame = self.frame;
        self.map.get_mut(key).map(|(pm, used)| {
            *used = frame;
            &*pm
        })
    }

    fn insert(&mut self, key: TileKey, pm: Pixmap) {
        self.map.insert(key, (pm, self.frame));
    }
}

/// A small, fast hash (the method of rustc's FxHasher). The keys are not from a user, thus a
/// hash with protection against collisions is not necessary.
#[derive(Default)]
struct FxHasher(u64);

impl std::hash::Hasher for FxHasher {
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.write_u64(b as u64);
        }
    }
    fn write_u8(&mut self, i: u8) {
        self.write_u64(i as u64);
    }
    fn write_u16(&mut self, i: u16) {
        self.write_u64(i as u64);
    }
    fn write_u32(&mut self, i: u32) {
        self.write_u64(i as u64);
    }
    fn write_i32(&mut self, i: i32) {
        self.write_u64(i as u32 as u64);
    }
    fn write_usize(&mut self, i: usize) {
        self.write_u64(i as u64);
    }
    fn write_u64(&mut self, i: u64) {
        self.0 = (self.0.rotate_left(5) ^ i).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
    fn finish(&self) -> u64 {
        self.0
    }
}

thread_local! {
    static TILES: std::cell::RefCell<TileCache> = std::cell::RefCell::new(TileCache::default());
    /// A white pixmap of the screen size, to draw new tiles at their true position.
    static SCRATCH: std::cell::RefCell<Option<Pixmap>> = const { std::cell::RefCell::new(None) };
}

/// Copies the rectangle (sx, sy, w, h) of `src` to (dx, dy) in `dst`. The rectangles must be inside.
#[allow(clippy::too_many_arguments)]
fn copy_px(src: &Pixmap, sx: usize, sy: usize, w: usize, h: usize, dst: &mut Pixmap, dx: usize, dy: usize) {
    let (sw, dw) = (src.width() as usize * 4, dst.width() as usize * 4);
    let (s, d) = (src.data(), dst.data_mut());
    for r in 0..h {
        let so = (sy + r) * sw + sx * 4;
        let doff = (dy + r) * dw + dx * 4;
        d[doff..doff + w * 4].copy_from_slice(&s[so..so + w * 4]);
    }
}

/// The anchor point of the pencil mark of a digit, in a cell with its top left at (xf, yf): the
/// keypad position of the digit (MRK-1).
fn mark_anchor(xf: f32, yf: f32, digit: u8) -> (f32, f32) {
    let (d, i) = (layout::device(), (digit - 1) as usize);
    (xf + d.mark_pos(i % 3), yf + d.mark_pos(i / 3))
}

/// The color of the pencil marks and of the x of a refused tap (§15). They are crisp drawings
/// (MRK-19): each pixel of them is this color.
pub const MARK_COLOR: Rgb = gray(15);

/// Draws one cell with its top left at (xf, yf; integer screen px): the background or the faint
/// micro-cell grid (a pattern, BRD-9), then the number, the "?" or the pencil marks and the x of a
/// refused tap (MRK-19). All are crisp drawings (NAV-20). The cell is drawn the same on the main
/// thread, on a stripe thread and into a tile: the pattern is a function of the position in the
/// cell only.
fn draw_cell(c: &mut Canvas, look: CellLook, xf: f32, yf: f32) {
    let d = layout::device();
    let cs = d.cell as f32;
    match look {
        CellLook::Value(_, bg, _) => c.rect(xf, yf, xf + cs - 1.0, yf + cs - 1.0, bg),
        CellLook::Question => {}
        CellLook::Empty(_, lines, _) => {
            // The micro-cell grid of an empty cell (BRD-6): lines of `LineWidths::micro_line` px
            // from the pixels `micro_offset(k)`, over the full cell, as a pattern of gray15 and white
            // pixels with the darkness of the gray of the setting (BRD-9). The pattern starts at the first pixel of
            // each line: the row index of the matrix is the position along the line, the column index
            // the position across it. A line writes only its gray15 pixels, thus a crossing is the
            // union of the two patterns. None: no lines.
            if let Some(shade) = line_shade(lines) {
                let (x, y, w) = (xf as i32, yf as i32, d.lines.micro_line);
                for k in 1..=2 {
                    let o = micro_offset(k);
                    c.pattern(x + o, y, w, d.cell, Axis::Y, shade, |_, _| true);
                    c.pattern(x, y + o, d.cell, w, Axis::X, shade, |_, _| true);
                }
            }
        }
    }
    // Numbers: starting numbers black, your numbers `ENTRY_DIGIT` (BRD-8), crisp (NAV-20).
    // Pencil marks only on empty cells (MRK-2), gray15, at their keypad positions (MRK-1).
    match look {
        CellLook::Value(v, _, col) => c.text(Weight::Regular, d.digit_size(), xf + (d.cell / 2) as f32, yf + d.digit_baseline(), Anchor::Middle, &v.to_string(), col),
        CellLook::Question => c.text(Weight::Regular, d.digit_size(), xf + (d.cell / 2) as f32, yf + d.digit_baseline(), Anchor::Middle, "?", SECONDARY),
        // The marks and the x are crisp drawings (MRK-19, NAV-20): each pixel is gray15 or the pixel
        // of the cell without them (white, or a gray15 dot of the grid).
        CellLook::Empty(marks, _, refused) => {
            for digit in 1..=9u8 {
                if marks & crate::game::mark_bit(digit) != 0 {
                    let (cx, cy) = mark_anchor(xf, yf, digit);
                    c.text(Weight::Regular, d.mark_size(), cx, cy + d.mark_baseline(), Anchor::Middle, &digit.to_string(), MARK_COLOR);
                }
            }
            // MRK-13: only in its micro-cell. It does not touch the lines or the other marks, and a
            // peer has its digit, thus no mark shows under it (MRK-12).
            if let Some(digit) = refused {
                let (cx, cy) = mark_anchor(xf, yf, digit);
                let (h, w) = (d.refused_x_half(), d.lines.refused_x_line);
                c.line(cx - h, cy - h, cx + h, cy + h, w, MARK_COLOR);
                c.line(cx - h, cy + h, cx + h, cy - h, w, MARK_COLOR);
            }
        }
    }
}

/// The lines of the board: the border, the box lines and the cell lines, each on its own pixels
/// between the cells (`Device::line_offset`, BRD-11). Only the lines that `keep` accepts (the index 0
/// to 9 and true for a vertical line). With `near`, only the part of each line near the area (x0, y0,
/// x1, y1): the pixels of that area are the same as with the full lines. The lines are crisp drawings
/// (BRD-11): black rectangles on full pixels, with no anti-aliasing.
fn board_lines(c: &mut Canvas, keep: impl Fn(i32, bool) -> bool, near: Option<(i32, i32, i32, i32)>) {
    let d = layout::device();
    let (x0, y0, b, m) = (board_x(), board_y(), d.board_size(), d.raster.line_near_margin);
    // The part of a line from `a` to `a + b - 1` near the span s0..s1.
    let part = |a: i32, s0: i32, s1: i32| match near {
        None => (a, a + b - 1),
        Some(_) => ((s0 - m).max(a), (s1 + m).min(a + b - 1)),
    };
    let (nx0, ny0, nx1, ny1) = near.unwrap_or((0, 0, 0, 0));
    let (vy0, vy1) = part(y0, ny0, ny1);
    let (hx0, hx1) = part(x0, nx0, nx1);
    for k in 0..=9 {
        let (o, w) = (d.line_offset(k), d.line_width(k));
        if keep(k, true) {
            c.rect((x0 + o) as f32, vy0 as f32, (x0 + o + w - 1) as f32, vy1 as f32, BLACK);
        }
        if keep(k, false) {
            c.rect(hx0 as f32, (y0 + o) as f32, hx1 as f32, (y0 + o + w - 1) as f32, BLACK);
        }
    }
}

/// The key of a cached area of the board: the fonts (address), the board origin, the area origin and
/// the cell look (None: the band around the cells).
type TileKey = (usize, i32, i32, i32, i32, Option<CellLook>);

/// Gives the cached pixels of an area of the board, or draws them with `draw` in a white scratch
/// pixmap at their true position. Copies them into the frame at (x, y), (w, h) pixels.
fn put_cached(c: &mut Canvas, key: TileKey, x: usize, y: usize, w: usize, h: usize, draw: impl FnOnce(&mut Canvas)) {
    let (fw, fh) = (c.pm.width(), c.pm.height());
    TILES.with(|tiles| {
        let mut tiles = tiles.borrow_mut();
        if let Some(tile) = tiles.get(&key) {
            copy_px(tile, 0, 0, w, h, c.pm, x, y);
            return;
        }
        {
            let tile = SCRATCH.with(|scratch| {
                let mut scratch = scratch.borrow_mut();
                if scratch.as_ref().is_none_or(|p| p.width() != fw || p.height() != fh) {
                    *scratch = Pixmap::new(fw, fh);
                }
                let pm = scratch.as_mut().unwrap();
                for r in 0..h {
                    let o = ((y + r) * fw as usize + x) * 4;
                    pm.data_mut()[o..o + w * 4].fill(255);
                }
                draw(&mut Canvas::new(pm, c.fonts));
                let mut tile = Pixmap::new(w as u32, h as u32).unwrap();
                copy_px(pm, x, y, w, h, &mut tile, 0, 0);
                tile
            });
            copy_px(&tile, 0, 0, w, h, c.pm, x, y);
            tiles.insert(key, tile);
        }
    });
}

/// The board (render.sh and board-state.py).
///
/// Each cell is a cached tile with the final pixels of its area (CELL + 1 pixels on each side): the
/// cell, then the lines at its four sides. The text of a cell does not touch the lines, and the lines
/// cover the edge pixels of the tile fully, thus the pixels are the same as a drawing of all cells,
/// then all lines, then all text. The band around the cells has only lines on white.
fn draw_board(c: &mut Canvas, g: &Game, settings: &Settings, overlays: BoardOverlays) {
    let (bx, by) = (board_x(), board_y());
    let colors = BoardColors::of(g, settings);
    let Some(area) = board_area(c.pm) else {
        // Not on the screen: draw without the cache (this does not occur with the layouts).
        for cell in cells() {
            let (x, y) = cell_origin(cell);
            draw_cell(c, cell_look(g, settings, &colors, overlays, cell), x as f32, y as f32);
        }
        board_lines(c, |_, _| true, None);
        return;
    };
    let fonts = c.fonts as *const Fonts as usize;
    let looks: Vec<_> = cells().map(|cell| (cell, cell_look(g, settings, &colors, overlays, cell))).collect();
    let key = |a: &Area, look| (fonts, bx, by, a.x0, a.y0, look);
    // The slots of the board: the cells (in the order of `cells`), then the band.
    let slots: Vec<(Area, TileKey)> = board_slots()
        .zip(looks.iter().map(|&(_, look)| Some(look)).chain(std::iter::repeat(None)))
        .map(|(a, look)| (a, key(&a, look)))
        .collect();
    let missing = TILES.with(|t| {
        let mut t = t.borrow_mut();
        t.begin_frame();
        slots[..looks.len()].iter().filter(|(_, k)| !t.map.contains_key(k)).count()
    });
    if missing > COLD_TILES {
        // Many new tiles (for example the first frame): one full drawing of the board is faster.
        // Then the tiles come from the frame.
        let threads = crate::system::threads();
        draw_board_stripes(c, area, &looks, threads);
        TILES.with(|t| {
            let mut t = t.borrow_mut();
            for (a, k) in &slots {
                let (w, h) = ((a.x1 - a.x0) as u32, (a.y1 - a.y0) as u32);
                let mut tile = Pixmap::new(w, h).unwrap();
                copy_px(c.pm, a.x0 as usize, a.y0 as usize, w as usize, h as usize, &mut tile, 0, 0);
                t.insert(*k, tile);
            }
        });
        for (i, (_, k)) in slots.iter().enumerate() {
            c.set_slot(i, *k);
        }
        return;
    }
    for (i, (a, k)) in slots.iter().enumerate() {
        if c.slot_has(i, k) {
            // The frame has the pixels of this tile. The cache keeps it as a tile of this frame.
            TILES.with(|t| {
                let _ = t.borrow_mut().get(k);
            });
            continue;
        }
        let (x, y, w, h) = (a.x0 as usize, a.y0 as usize, (a.x1 - a.x0) as usize, (a.y1 - a.y0) as usize);
        match k.5 {
            Some(look) => {
                let (row, col) = looks[i].0;
                let (r, kc) = (row as i32, col as i32);
                let (xi, yi) = (a.x0, a.y0);
                // The tile is the cell and the lines at its right and below it (`board_slots`).
                put_cached(c, *k, x, y, w, h, |c| {
                    draw_cell(c, look, xi as f32, yi as f32);
                    let near = Some((xi, yi, a.x1 - 1, a.y1 - 1));
                    board_lines(c, |i, vertical| if vertical { i == kc + 1 } else { i == r + 1 }, near);
                });
            }
            None => put_cached(c, *k, x, y, w, h, |c| board_lines(c, |_, _| true, None)),
        }
        c.set_slot(i, *k);
    }
}

/// Draws the full board (all cells, then all lines) on white in the area `area` of the frame. The
/// threads draw horizontal stripes of the area: each thread draws all of the board on its own
/// stripe pixmap, with the screen rows of the stripe (an integer translation gives the same pixels).
fn draw_board_stripes(c: &mut Canvas, (ax, ay, aw, ah): (usize, usize, usize, usize), looks: &[(crate::game::Cell, CellLook)], threads: usize) {
    let fw = c.pm.width();
    let fonts = c.fonts;
    // The rotation and the hand are thread-local: a new thread starts with the portrait layout of a
    // left-handed player. The positions of the board (cell_origin, board_lines) use these values, thus
    // each stripe thread gets the values of this thread.
    let (rotation, right_handed) = (crate::layout::rotation(), crate::layout::right_handed());
    // The stripes end at the borders of cell rows: a glyph that the edge of the pixmap clips gets
    // other pixels (tiny-skia cuts its curves), and no glyph crosses these borders.
    let (d, by) = (crate::layout::device(), board_y() as usize);
    let margin = d.raster.stripe_margin as usize;
    let mut edges: Vec<usize> = (1..threads).map(|k| by + d.line_offset((9 * k / threads) as i32) as usize).collect();
    edges.insert(0, ay);
    edges.push(ay + ah);
    edges.dedup();
    let stripes: Vec<(usize, usize, usize, Pixmap)> = std::thread::scope(|scope| {
        let handles: Vec<_> = edges
            .windows(2)
            .map(|e| {
                let (y0, h) = (e[0], e[1] - e[0]);
                scope.spawn(move || {
                    crate::layout::set_rotation(rotation);
                    crate::layout::set_right_handed(right_handed);
                    // The clip at the edge of a pixmap changes the pixels near it: the stripe pixmap
                    // has `RasterMetrics::stripe_margin` more rows above and below.
                    let top = y0 - margin.min(y0);
                    let mut pm = Pixmap::new(fw, (y0 - top + h + margin) as u32).unwrap();
                    pm.fill(tiny_skia::Color::WHITE);
                    let mut s = Canvas { pm: &mut pm, fonts, units: Units::Screen, record: None, top: top as i32, ink: false };
                    for &(cell, look) in looks {
                        let (x, y) = cell_origin(cell);
                        draw_cell(&mut s, look, x as f32, y as f32);
                    }
                    board_lines(&mut s, |_, _| true, None);
                    (y0, y0 - top, h, pm)
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().expect("board stripe thread")).collect()
    });
    for (y0, sy, h, pm) in stripes {
        copy_px(&pm, ax, sy, aw, h, c.pm, ax, y0);
    }
}

/// The slots of the board: the area of each cell tile (the cell and the lines at its right and below
/// it, in the order of `cells`), then the band around the cells with the top and the left border (top,
/// bottom, left and right). Together they are the board area.
fn board_slots() -> impl Iterator<Item = Area> {
    let d = layout::device();
    let (mu, bu, lw) = (d.raster.board_margin as usize, d.board_size() as usize, d.lines.box_line as usize);
    let (bxu, byu) = (board_x() as usize, board_y() as usize);
    let s = bxu - mu;
    let bands = [
        Area::xywh(s, byu - mu, bu + 2 * mu, mu + lw),
        Area::xywh(s, byu + bu, bu + 2 * mu, mu),
        Area::xywh(s, byu + lw, mu + lw, bu - lw),
        Area::xywh(bxu + bu, byu + lw, mu, bu - lw),
    ];
    let c = d.cell as usize;
    cells()
        .map(move |(r, k)| {
            let (x, y) = cell_origin((r, k));
            let (w, h) = (c + d.line_width(k as i32 + 1) as usize, c + d.line_width(r as i32 + 1) as usize);
            Area::xywh(x as usize, y as usize, w, h)
        })
        .chain(bands)
}

/// The board area that `draw_board` writes fully: the cells and the band around them (x, y, w, h).
/// None if the area is not fully on the screen (then `draw_board` does not use the tiles).
fn board_area(pm: &Pixmap) -> Option<(usize, usize, usize, usize)> {
    let (bx, by, m) = (board_x(), board_y(), layout::device().raster.board_margin);
    let (fw, fh) = (pm.width() as i32, pm.height() as i32);
    if bx < m || by < m || bx + board_size() + m >= fw || by + board_size() + m >= fh {
        return None;
    }
    let n = (board_size() + 2 * m) as usize;
    Some(((bx - m) as usize, (by - m) as usize, n, n))
}

/// The largest quantity of cached tiles (approximately 87 KB each, thus 22 MB). One frame uses 85.
/// When the cache is full, it keeps only the tiles of the last frame.
const MAX_TILES: usize = 256;

/// With more new tiles than this, `draw_board` draws the full board (it is faster than the tiles).
const COLD_TILES: usize = 16;

/// The clashes and the completed color of a board. `draw_board` calculates them one time for all
/// its cells.
struct BoardColors {
    clashing: CellSet,
    /// The cells of the completed groups and their color, if the completed color shows (COL-11, COL-12).
    completed: Option<(CellSet, Rgb)>,
}

impl BoardColors {
    fn of(g: &Game, settings: &Settings) -> BoardColors {
        let sets = g.board_sets();
        BoardColors { clashing: sets.clashing, completed: settings.completed_fill(g).map(|color| (sets.completed, color.rgb())) }
    }
}

/// The look of a cell for the state of the game.
fn cell_look(g: &Game, settings: &Settings, colors: &BoardColors, overlays: BoardOverlays, cell: crate::game::Cell) -> CellLook {
    let v = g.value_at(cell);
    if v != 0 {
        // BRD-7: the first rule that applies.
        let completed = colors.completed.filter(|(set, _)| set.contains(cell)).map(|(_, rgb)| rgb);
        let bg = if colors.clashing.contains(cell) {
            settings.color_clash.rgb()
        } else if overlays.highlight == Some(v) {
            settings.color_highlight.rgb()
        } else if let Some(rgb) = completed {
            rgb
        } else if g.is_given(cell) {
            settings.color_starting.rgb()
        } else {
            Color::White.rgb()
        };
        CellLook::Value(v, bg, if g.is_given(cell) { BLACK } else { ENTRY_DIGIT })
    } else if overlays.question == Some(cell) {
        CellLook::Question
    } else {
        let refused = overlays.refused.filter(|&(x_cell, _)| x_cell == cell).map(|(_, digit)| digit);
        CellLook::Empty(g.visible_marks(cell, settings.auto_hide_on), settings.micro_line, refused)
    }
}

/// The difficulty slider with its labels (ui-lib.sh `labeled_slider`).
fn labeled_slider(c: &mut Canvas, xl: i32, xr: i32, y: i32, level: u8) {
    let sizes = layout::device().slider;
    let baseline = (y + sizes.label_baseline) as f32;
    c.text(SLIDER_LABEL_WEIGHT, sizes.label_size, xl as f32, baseline, Anchor::Start, "Easy", BLACK);
    c.text(SLIDER_LABEL_WEIGHT, sizes.label_size, xr as f32, baseline, Anchor::End, "Hard", BLACK);
    let stops = slider_stops(c.fonts, xl, xr);
    slider(c, &stops, y as f32, level);
}

fn slider(c: &mut Canvas, stops: &[f32; STOPS as usize], y: f32, level: u8) {
    let (xa, xb) = (stops[0], stops[STOPS as usize - 1]);
    let xk = stops[level as usize];
    let m = layout::device().slider.meter;
    c.line(xk, y, xb, y, m.track, gray(60));
    c.line(xa, y, xk, y, m.active_track, BLACK);
    for (i, &x) in stops.iter().enumerate() {
        let fill = if (i as u8) < level { BLACK } else { WHITE };
        c.circle(x, y, m.stop_radius, Some(fill), Some((BLACK, m.outline)));
    }
    c.circle(xk, y, m.knob_radius, Some(BLACK), Some((WHITE, m.outline)));
}

/// The open menu (toolbar.sh).
fn draw_toolbar(c: &mut Canvas, app: &App, panel: Option<Panel>) {
    let draw = |c: &mut Canvas, icon: Icon, b: Button, color| {
        let (x, y) = icon_center(b);
        icon.draw(c, x, y, color);
    };

    // The close button (top left, or top right for a right-handed player).
    draw(c, Icon::Close, Button::Close, BLACK);

    // A panel button is white on black while its panel is open (NAV-8).
    let colors_active = panel == Some(Panel::Colors);
    let settings_active = panel == Some(Panel::Settings);
    let plus_active = panel == Some(Panel::NewGame);
    let bookmark_active = panel == Some(Panel::Checkpoints);
    if colors_active {
        active_slot(c, slot_top(Button::Colors));
    }
    if settings_active {
        active_slot(c, slot_top(Button::Settings));
    }
    draw(c, Icon::Gear, Button::Settings, if settings_active { WHITE } else { BLACK });
    if app.update.caution() {
        let (x, y) = icon_center(Button::Settings);
        let offset = layout::device().icons.caution_offset;
        Icon::Caution.draw(c, x + offset, y - offset, WHITE);
    }
    if plus_active {
        active_slot(c, slot_top(Button::NewGame));
    }

    draw(c, Icon::PaintBucket, Button::Colors, if colors_active { WHITE } else { BLACK });

    // Undo and redo: black when available, gray when not (UND-7, UND-8).
    for (icon, b, available) in [(Icon::Undo, Button::Undo, app.game.can_undo()), (Icon::Redo, Button::Redo, app.game.can_redo())] {
        draw(c, icon, b, if available { BLACK } else { gray(70) });
    }

    // The bookmark: gray on a complete puzzle (CKP-9), and white on black while its panel is open (NAV-8).
    if bookmark_active {
        active_slot(c, slot_top(Button::Checkpoints));
    }
    let bookmark = match (bookmark_active, app.game.completed) {
        (true, _) => WHITE,
        (false, true) => gray(70),
        (false, false) => BLACK,
    };
    draw(c, Icon::Bookmark, Button::Checkpoints, bookmark);

    // + (new game), Analytics and Help at the bottom.
    draw(c, Icon::Plus, Button::NewGame, if plus_active { WHITE } else { BLACK });
    draw(c, Icon::Statistics, Button::Statistics, BLACK);
    draw(c, Icon::Help, Button::Help, BLACK);
}

/// The open panel of the menu.
fn draw_panel(c: &mut Canvas, app: &App, panel: Panel) {
    match panel {
        Panel::Colors => draw_colors_panel(c, &app.settings),
        Panel::Settings => draw_settings_panel(c, &app.settings, &app.update),
        Panel::NewGame => draw_new_game_panel(c, app.ui.slider, app.ui.symmetric_sel),
        Panel::Checkpoints => draw_checkpoints_panel(c, &app.game, &app.settings),
    }
}

fn active_slot(c: &mut Canvas, y: i32) {
    c.rect(tb_x() as f32, y as f32, (tb_x() + tb_slot()) as f32, (y + tb_slot() - 1) as f32, BLACK);
}

/// A pop-up frame that touches the toolbar (toolbar.sh `panel`), with a border of
/// `PanelMetrics::line`. `pr` and `bot` are the last pixels of the frame.
fn panel_frame(c: &mut Canvas, pl: i32, pr: i32, top: i32, bot: i32) {
    // The offset of the last pixel of a line from its first pixel.
    let last = (layout::device().panel.line - 1) as f32;
    let (pl, pr, top, bot) = (pl as f32, pr as f32, top as f32, bot as f32);
    c.rect(pl, top, pr, bot, WHITE);
    c.rect(pl, top, pr, top + last, BLACK);
    c.rect(pl, top, pl + last, bot, BLACK);
    c.rect(pr - last, top, pr, bot, BLACK);
    if bot < screen_h() as f32 {
        c.rect(pl, bot - last, pr, bot, BLACK);
    }
}

/// The divider of `PanelMetrics::line` between two rows of a panel from `pl` to `pr` (the last
/// pixel), directly above the row with its top at `y`.
fn panel_divider(c: &mut Canvas, pl: i32, pr: i32, y: i32) {
    c.rect(pl as f32, (y - layout::device().panel.line) as f32, pr as f32, (y - 1) as f32, BLACK);
}

/// The Settings panel (REQUIREMENTS.md §9.1): one row with a name and a switch for each `Switch`, the version, and the
/// update row of a stable or a preview build (§9.2).
fn draw_settings_panel(c: &mut Canvas, settings: &Settings, update: &Update) {
    let panel = layout::device().panel;
    let (pl, pr) = (layout::settings_pl(), layout::settings_pr());
    let (text_x, value_x) = ((pl + panel.inset) as f32, (pr - panel.inset) as f32);
    let frame = layout::settings_panel(update.has_row());
    panel_frame(c, pl, pr, frame.y0, frame.y1 - 1);
    // Draws the divider above a row, and gives the center line and the baseline of the row.
    let divider_row = |c: &mut Canvas, i: usize| {
        let r = layout::settings_row(i);
        if i > 0 {
            panel_divider(c, pl, pr, r.y0);
        }
        let cy = r.y0 + panel.center;
        (cy as f32, (cy + panel.baseline) as f32)
    };
    for s in Switch::ALL {
        let (cy, baseline) = divider_row(c, s.row());
        let name = s.label();
        let first = if name.len() == 1 { baseline } else { cy - panel.first_of_two as f32 };
        for (k, line) in name.iter().enumerate() {
            c.text(Weight::Medium, panel.text_size, text_x, first + (panel.line_pitch * k as i32) as f32, Anchor::Start, line, BLACK);
        }
        switch(c, value_x, cy, settings.is_on(s));
    }
    let (_, baseline) = divider_row(c, layout::VERSION_ROW);
    c.text(Weight::Medium, panel.text_size, text_x, baseline, Anchor::Start, "Version", BLACK);
    c.text(Weight::Regular, panel.text_size, value_x, baseline, Anchor::End, &update.current, BLACK);
    let (cy, baseline) = divider_row(c, layout::LICENSES_ROW);
    c.text(Weight::Medium, panel.text_size, text_x, baseline, Anchor::Start, "Third-party licenses", BLACK);
    Icon::OpenList.draw(c, (pr - panel.icon_inset) as f32, cy, BLACK);
    if !update.has_row() {
        return;
    }
    let (cy, baseline) = divider_row(c, layout::UPDATE_ROW);
    if let Some(text) = update.row_text() {
        c.text(Weight::Regular, panel.text_size, text_x, baseline, Anchor::Start, text, SECONDARY);
    } else if let Some(button) = update.button() {
        // A box with a border of the width of the panel lines, the icon and the text.
        let m = layout::device().settings.update_button;
        let b = layout::update_button();
        let last = (panel.line - 1) as f32;
        let (x0, y0, x1, y1) = (b.x0 as f32, b.y0 as f32, b.x1 as f32, b.y1 as f32);
        c.rect(x0, y0, x1, y0 + last, BLACK);
        c.rect(x0, y1 - last, x1, y1, BLACK);
        c.rect(x0, y0, x0 + last, y1, BLACK);
        c.rect(x1 - last, y0, x1, y1, BLACK);
        let icon = match button {
            UpdateButton::Install(_) => Icon::Install,
            UpdateButton::Check => Icon::CheckForUpdate,
            UpdateButton::Restart => Icon::Restart,
        };
        icon.draw(c, x0 + m.icon_x as f32, cy, BLACK);
        c.text(Weight::Medium, panel.text_size, x0 + m.text_x as f32, baseline, Anchor::Start, button.text(), BLACK);
    }
}

/// A switch with its right end at x (`SwitchMetrics`). The curves are made at the origin and moved by
/// a transform, because the f32 rounding of curves at absolute positions gives other edge pixels at a
/// different x. Thus the switch has the same pixels for the two hands (NAV-15).
fn switch(c: &mut Canvas, x1: f32, cy: f32, on: bool) {
    let m = layout::device().switch;
    let (w, r, k) = (m.length, m.radius, m.curve_control);
    let mut pb = PathBuilder::new();
    pb.move_to(r, -r);
    pb.line_to(w - r, -r);
    pb.cubic_to(w - r + k, -r, w, -k, w, 0.0);
    pb.cubic_to(w, k, w - r + k, r, w - r, r);
    pb.line_to(r, r);
    pb.cubic_to(r - k, r, 0.0, k, 0.0, 0.0);
    pb.cubic_to(0.0, -k, r - k, -r, r, -r);
    pb.close();
    let track = pb.clone();
    let x0 = x1 - w;
    let at = Transform::from_translate(x0, cy);
    c.path_fill(pb, at, if on { BLACK } else { WHITE });
    c.path_stroke(track, at, m.outline, BLACK);
    let mut knob = PathBuilder::new();
    knob.push_circle(0.0, 0.0, m.knob_radius);
    let (knob_x, knob_color) = if on { (x1 - r, WHITE) } else { (x0 + r, BLACK) };
    c.path_fill(knob, Transform::from_translate(knob_x, cy), knob_color);
}

fn draw_colors_panel(c: &mut Canvas, settings: &Settings) {
    let (pl, pr) = (colors_pl(), colors_pr());
    let (top, bottom) = (layout::colors_top(), layout::colors_bottom());
    let d = layout::device();
    panel_frame(c, pl, pr, top, bottom);
    // The lines between the columns (NAV-17).
    for k in 1..layout::color_columns() {
        let x = pl + k * d.colors_panel.section_width;
        c.rect(x as f32, top as f32, (x + d.panel.line - 1) as f32, bottom as f32, BLACK);
    }
    for section in Section::ALL {
        let selected = settings.color(section);
        let items = Color::ALL.map(|color| {
            let state = if color == selected {
                Swatch::Selected
            } else if settings.is_taken(section, color) {
                Swatch::Taken
            } else {
                Swatch::Available
            };
            (Dot::Color(color.rgb()), color.name().to_string(), state)
        });
        color_section(c, section.panel_index(), section.title(), &items);
    }
    // The Micro-cell lines section: the pattern of each gray on a white disc, "None" a white dot (COL-13).
    let items = LineGray::ALL.map(|g| {
        let state = if g == settings.micro_line { Swatch::Selected } else { Swatch::Available };
        (Dot::Lines(g), g.name(), state)
    });
    color_section(c, LINES_SECTION, "Micro-cell lines", &items);
}

/// The dot of an item of the Cell colors panel: all that its pixels depend on, except its state.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Dot {
    /// A color of the four color sections.
    Color(Rgb),
    /// An item of the Micro-cell lines section: the pattern of its gray on a white disc (COL-13), or
    /// a white dot for `LineGray::None`.
    Lines(LineGray),
}

/// The state of an item in a section of the Cell colors panel.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Swatch {
    Available,
    Selected,
    /// A different section uses the color (COL-5).
    Taken,
}

/// One section of the Cell colors panel (toolbar.sh `color_section`): the divider above it across its
/// column (except the first section of a column), the title, and a 3x3 grid of swatches with their names. A black box shows the
/// selected item. A taken item has a gray ring, a gray name and a line through the dot.
fn color_section(c: &mut Canvas, index: usize, title: &str, items: &[(Dot, String, Swatch); 9]) {
    let (left, top) = (layout::color_section_left(index), layout::color_section_top(index));
    let (panel, m) = (layout::device().panel, layout::device().colors_panel);
    if top > layout::colors_top() {
        c.rect(left as f32, top as f32, (left + m.section_width - 1) as f32, (top + panel.line - 1) as f32, BLACK);
    }
    let (x, y) = ((left + panel.inset) as f32, (top + m.title_baseline) as f32);
    c.text(Weight::Medium, panel.text_size, x, y, Anchor::Start, title, BLACK);
    for (i, (dot, name, state)) in items.iter().enumerate() {
        let cell = layout::color_grid_cell(index, i);
        // The panel is white under the item, and the item draws only in its cell.
        let look = BoxLook::Swatch(*dot, name.clone(), *state);
        put_box(c, (cell.x0, cell.y0, cell.x1, cell.y1), look, |c| color_item(c, cell, *dot, name, *state));
    }
}

/// A drawing that the box cache keeps (refer to `put_box`): all that its pixels depend on.
#[derive(Clone, PartialEq, Eq, Hash)]
enum BoxLook {
    /// An item of the Cell colors panel: its dot (a color, or the gray of the pattern), its name and
    /// its state.
    Swatch(Dot, String, Swatch),
    /// A row of the list of completed games: the date, the mode and the time.
    ListRow(String, u8, String),
    /// A row of the Checkpoints panel.
    CheckpointRow(Box<CheckpointRow>),
}

/// The key of a cached box: the fonts (address), the layout, the box (screen px) and the look.
type BoxKey = (usize, layout::Rotation, bool, (i32, i32, i32, i32), BoxLook);

/// More bytes of cached boxes than this: the cache starts again. A page of the games list (rows of
/// 0.7 MB), all items of the Cell colors panel and 5 rows of the Checkpoints panel (0.4 MB each) fit.
const MAX_BOX_BYTES: usize = 16 << 20;

thread_local! {
    static BOXES: std::cell::RefCell<std::collections::HashMap<BoxKey, Pixmap, std::hash::BuildHasherDefault<FxHasher>>> =
        std::cell::RefCell::new(Default::default());
}

/// Draws `look` with `draw` in the screen box `(x0, y0, x1, y1)` (x1 and y1 not included). The
/// caller makes sure that the box is white before the drawing and that `draw` changes pixels only
/// in the box: then the cached pixels of the box are the same as a new drawing. A redraw of a screen
/// draws only the boxes that changed (a tap in the Cell colors panel, the dialog over the list).
fn put_box(c: &mut Canvas, (x0, y0, x1, y1): (i32, i32, i32, i32), look: BoxLook, draw: impl FnOnce(&mut Canvas)) {
    if c.top != 0 || x0 < 0 || y0 < 0 || x1 > c.pm.width() as i32 || y1 > c.pm.height() as i32 || x1 <= x0 || y1 <= y0 {
        draw(c);
        return;
    }
    let key = (c.fonts as *const Fonts as usize, layout::rotation(), layout::right_handed(), (x0, y0, x1, y1), look);
    let (x, y, w, h) = (x0 as usize, y0 as usize, (x1 - x0) as usize, (y1 - y0) as usize);
    let hit = BOXES.with(|t| {
        let t = t.borrow();
        let tile = t.get(&key)?;
        copy_px(tile, 0, 0, w, h, c.pm, x, y);
        Some(())
    });
    if hit.is_some() {
        if let Some(r) = SkRect::from_ltrb(x0 as f32, y0 as f32, x1 as f32, y1 as f32) {
            c.touched(r, 0.0);
        }
        return;
    }
    draw(c);
    let mut tile = Pixmap::new(w as u32, h as u32).unwrap();
    copy_px(c.pm, x, y, w, h, &mut tile, 0, 0);
    BOXES.with(|t| {
        let mut t = t.borrow_mut();
        if t.values().map(|p| p.data().len()).sum::<usize>() + tile.data().len() > MAX_BOX_BYTES {
            t.clear();
        }
        t.insert(key, tile);
    });
}

/// One item of the Cell colors panel (`ColorItemMetrics`): the swatch (a circle with a ring), and its
/// name below it. A `Dot::Lines` swatch is white, with the pattern of its gray inside the ring (COL-13).
fn color_item(c: &mut Canvas, cell: Rect, dot: Dot, name: &str, state: Swatch) {
    let (text_size, m) = (layout::device().panel.text_size, layout::device().colors_panel.item);
    let (x0, y0) = (cell.x0 as f32, cell.y0 as f32);
    // The swatch: centered in the width of the cell, above the name.
    let (cx, cy) = (x0 + (m.width / 2) as f32, (cell.y0 + m.swatch_y) as f32);
    let (fg, ring) = match state {
        Swatch::Selected => {
            c.rect(x0, y0, (cell.x1 - 1) as f32, (cell.y1 - 1) as f32, BLACK);
            (WHITE, WHITE)
        }
        Swatch::Taken => (gray(70), gray(70)),
        Swatch::Available => (BLACK, BLACK),
    };
    let fill = match dot {
        Dot::Color(rgb) => rgb,
        Dot::Lines(_) => WHITE,
    };
    c.circle(cx, cy, m.swatch_radius, Some(fill), Some((ring, m.ring)));
    if let Dot::Lines(g) = dot {
        if let Some(shade) = line_shade(g) {
            // COL-13: the pattern of the gray on the white disc of `pattern_radius` inside the ring,
            // from the top left of the box of the disc (`BAYER[dy mod 4][dx mod 4]`). A pixel (integer
            // screen position, the center of the pixel in the coordinates of the canvas) is inside the
            // disc when its distance to the center is less than the radius.
            let r = m.pattern_radius;
            let (ox, oy) = ((cx - r).floor() as i32 - 1, (cy - r).floor() as i32 - 1);
            let size = 2 * (r.ceil() as i32 + 1);
            c.pattern(ox, oy, size, size, Axis::Y, shade, |dx, dy| {
                let (px, py) = ((ox + dx) as f32 - cx, (oy + dy) as f32 - cy);
                px * px + py * py < r * r
            });
        }
    }
    if state == Swatch::Taken {
        let r = m.strike_reach;
        c.line(cx - r, cy + r, cx + r, cy - r, m.strike_width, gray(50));
    }
    c.text(Weight::Medium, text_size, cx, (cell.y0 + m.name_baseline) as f32, Anchor::Middle, name, fg);
}

/// The New game panel (§11): the slider, the Symmetrical row with the switch of the Settings panel
/// (DIF-8), and Start new game, with a divider above each of the last two rows. The names and the
/// switch have the text, the baseline and the inset of the rows of the Settings panel
/// (`PanelMetrics`).
fn draw_new_game_panel(c: &mut Canvas, slider_level: u8, symmetric: bool) {
    let panel = layout::device().panel;
    let (pl, pr) = (ng_pl(), ng_pr());
    panel_frame(c, pl, pr, ng_top(), screen_h() as i32);
    for row in [layout::ng_symmetric_row(), layout::ng_start_row()] {
        panel_divider(c, pl, pr, row.y0);
    }
    labeled_slider(c, ng_tx(), ng_tr(), ng_slider_y(), slider_level);
    let name = |c: &mut Canvas, cy: i32, text: &str| {
        c.text(Weight::Medium, panel.text_size, ng_tx() as f32, (cy + panel.baseline) as f32, Anchor::Start, text, BLACK);
    };
    let cy = layout::ng_symmetric_y();
    name(c, cy, "Symmetrical");
    switch(c, (pr - panel.inset) as f32, cy as f32, symmetric);
    name(c, ng_start_y(), "Start new game");
}

/// The Checkpoints panel (§9.3, toolbar.sh `ckp_frame` and `ckp_row`).
fn draw_checkpoints_panel(c: &mut Canvas, g: &Game, settings: &Settings) {
    let n = g.checkpoints.len();
    let p = layout::ckp_panel(n);
    let (pl, pr, bottom) = (p.x0, p.x1 - 1, p.y1 - 1);
    panel_frame(c, pl, pr, p.y0, bottom);
    // The record keeps one area for all drawings in the panel, and not one area for each digit.
    // The margin contains the margin of a drawing at the edge of the panel.
    let d = layout::device();
    if let Some(r) = SkRect::from_ltrb(pl as f32, p.y0 as f32, p.x1 as f32, p.y1 as f32) {
        c.touched(r, d.raster.checkpoints_record_margin);
    }
    let (panel, m) = (d.panel, d.checkpoints);
    let at = |dx: i32| (pl + dx) as f32;

    // The row "Save checkpoint" (CKP-1).
    panel_divider(c, pl, pr, layout::ckp_row_top(0));
    let save = if g.can_save_checkpoint() { BLACK } else { gray(70) };
    let top = layout::ckp_top();
    let cy = top + panel.center;
    Icon::Plus.draw(c, at(m.save_icon_x), cy as f32, save);
    c.text(Weight::Medium, m.title_size, at(m.save_text_x), (cy + panel.baseline) as f32, Anchor::Start, "Save checkpoint", save);

    if n == 0 {
        let e = m.empty;
        c.text(Weight::Regular, m.title_size, at(e.center_x), (top + e.title_baseline) as f32, Anchor::Middle, "No checkpoints", SECONDARY);
        let hint = "Save the board before a difficult guess.";
        c.text(Weight::Regular, panel.text_size, at(e.center_x), (top + e.hint_baseline) as f32, Anchor::Middle, hint, SECONDARY);
    }
    for (i, k) in g.checkpoints.iter().enumerate() {
        let y0 = layout::ckp_row_top(i);
        if i > 0 {
            panel_divider(c, pl, pr, y0);
        }
        let look = CheckpointRow {
            board: k.board.clone(),
            seconds: k.seconds(),
            current: g.same_board(&k.board),
            given: g.given,
            starting: settings.color_starting.rgb(),
            clash: settings.color_clash.rgb(),
            completed: settings.completed_fill(g).map(Color::rgb),
            hide: settings.auto_hide_on,
        };
        // The box is in the white panel, between the side lines and the lines between the rows.
        let row_box = (pl + panel.line, y0, pr + 1 - panel.line, y0 + m.row - panel.line);
        put_box(c, row_box, BoxLook::CheckpointRow(Box::new(look.clone())), |c| checkpoint_row(c, &look, pl, y0, i));
    }
}

/// All that the pixels of a checkpoint row depend on, with its position (CKP-7).
#[derive(Clone, PartialEq, Eq, Hash)]
struct CheckpointRow {
    board: Board,
    seconds: u64,
    /// The board of the game is the same as this checkpoint: the button shows "Current".
    current: bool,
    /// The starting cells of the game.
    given: [[bool; 9]; 9],
    starting: Rgb,
    clash: Rgb,
    /// The completed color, if it shows (COL-11, COL-12).
    completed: Option<Rgb>,
    /// The setting "Hide pencil marks automatically".
    hide: bool,
}

/// The contents of row `i` of the Checkpoints panel, in a white box.
fn checkpoint_row(c: &mut Canvas, look: &CheckpointRow, pl: i32, y0: i32, i: usize) {
    let d = layout::device();
    let (size, m) = (d.panel.text_size, d.checkpoints);
    let at = |dx: i32| (pl + dx) as f32;
    small_board(c, look, pl + m.board.x, y0 + m.board.y);

    let played = format!("{} played", format_clock(look.seconds));
    c.text(Weight::Regular, size, at(m.text.x), (y0 + m.text.clock_baseline) as f32, Anchor::Start, &played, SECONDARY);
    let filled = format!("{} of 81 cells", look.board.filled_count());
    c.text(Weight::Regular, size, at(m.text.x), (y0 + m.text.cells_baseline) as f32, Anchor::Start, &filled, SECONDARY);

    // The button (CKP-2): the pixels from the first to the last x and y of `ckp_button`.
    let b = layout::ckp_button(i);
    let (bx0, by0, bx1, by1) = (b.x0 as f32, b.y0 as f32, (b.x1 - 1) as f32, (b.y1 - 1) as f32);
    c.rect(bx0, by0, bx1, by1, BLACK);
    let (label, label_color) = if look.current {
        ("Current", WHITE)
    } else {
        let line = d.panel.line as f32;
        c.rect(bx0 + line, by0 + line, bx1 - line, by1 - line, WHITE);
        ("Restore", BLACK)
    };
    c.text(Weight::Medium, size, at(m.button.label_x), (y0 + m.button.label_baseline) as f32, Anchor::Middle, label, label_color);

    Icon::Bin.draw(c, at(m.bin.x), (y0 + m.bin.y) as f32, BLACK);
}

/// The small board of a checkpoint with its first cell at (bx, by) (CKP-7, board-state.py `mini_board`).
/// The colors have the same rules as the board, with no highlight.
fn small_board(c: &mut Canvas, look: &CheckpointRow, bx: i32, by: i32) {
    let m = layout::device().checkpoints.board;
    let size = m.cell;
    let board = &look.board;
    let sets = board.board_sets();
    let origin = |(r, col): crate::game::Cell| (bx + col as i32 * size, by + r as i32 * size);
    let filled = || cells().filter(|&(r, col)| board.value[r as usize][col as usize] != 0);
    let is_given = |(r, col): crate::game::Cell| look.given[r as usize][col as usize];

    for cell in filled() {
        let (x, y) = origin(cell);
        let bg = if sets.clashing.contains(cell) {
            look.clash
        } else if let Some(rgb) = look.completed.filter(|_| sets.completed.contains(cell)) {
            rgb
        } else if is_given(cell) {
            look.starting
        } else {
            Color::White.rgb()
        };
        c.rect(x as f32, y as f32, (x + size - 1) as f32, (y + size - 1) as f32, bg);
    }
    // One dot of 2x2 px for each mark that shows, at its keypad position.
    for cell in cells() {
        let marks = board.visible_marks(cell, look.hide);
        let (x, y) = origin(cell);
        for d in 1..=9 {
            if marks & crate::game::mark_bit(d as u8) != 0 {
                let (column, line) = ((d - 1) % 3, (d - 1) / 3);
                let (dx, dy) = (x + m.mark_inset + m.mark_pitch * column, y + m.mark_inset + m.mark_pitch * line);
                c.rect(dx as f32, dy as f32, (dx + 1) as f32, (dy + 1) as f32, SECONDARY);
            }
        }
    }
    // The lines: first the thin lines, then the thick lines. A thin line is on the first pixel of the
    // cell after it. A thick line is on the last pixel of the cell before it and that first pixel.
    let side = 9 * size;
    for i in (1..9).filter(|i| i % 3 != 0) {
        let a = i * size;
        c.rect((bx + a) as f32, by as f32, (bx + a) as f32, (by + side - 1) as f32, gray(60));
        c.rect(bx as f32, (by + a) as f32, (bx + side - 1) as f32, (by + a) as f32, gray(60));
    }
    for i in [0, 3, 6, 9] {
        let a = i * size;
        c.rect((bx + a - 1) as f32, (by - 1) as f32, (bx + a) as f32, (by + side) as f32, BLACK);
        c.rect((bx - 1) as f32, (by + a - 1) as f32, (bx + side) as f32, (by + a) as f32, BLACK);
    }
    for cell in filled() {
        let (x, y) = origin(cell);
        let col = if is_given(cell) { BLACK } else { ENTRY_DIGIT };
        let digit = board.value[cell.0 as usize][cell.1 as usize].to_string();
        c.text(Weight::Regular, m.digit_size, x as f32 + size as f32 / 2.0, (y + m.digit_baseline) as f32, Anchor::Middle, &digit, col);
    }
}

// ---- Analytics (analytics.py)

/// The colors of a difficulty meter. `on` is the line and the stops up to the level, and the frame of
/// all stops. `hollow` is the inside of the stops after the level, and the ring of the knob. It is
/// the color of the background.
#[derive(Clone, Copy)]
struct MeterStyle {
    on: Rgb,
    hollow: Rgb,
}

impl MeterStyle {
    /// A black meter on a white background (the tables, a filter segment that is not selected).
    const ON_WHITE: MeterStyle = MeterStyle { on: BLACK, hollow: WHITE };
    /// A white meter on a black background (the selected filter segment).
    const ON_BLACK: MeterStyle = MeterStyle { on: WHITE, hollow: BLACK };
}

/// The color of the meter line after the level, for all meter styles.
const METER_TRACK: Rgb = gray(60);

/// The difficulty meter of the analytics screens (no labels). `step` is the distance between the stops.
fn meter(c: &mut Canvas, xa: f32, y: f32, step: f32, level: u8, style: MeterStyle) {
    let MeterStyle { on, hollow } = style;
    let (xk, xb) = (xa + level as f32 * step, xa + (MODES - 1) as f32 * step);
    let m = layout::device().meter;
    c.line(xk, y, xb, y, m.track, METER_TRACK);
    c.line(xa, y, xk, y, m.active_track, on);
    for i in 0..MODES {
        let x = xa + i as f32 * step;
        c.circle(x, y, m.stop_radius, Some(if i < level { on } else { hollow }), Some((on, m.outline)));
    }
    c.circle(xk, y, m.knob_radius, Some(on), Some((hollow, m.outline)));
}

/// "< Back", the title and the page arrows.
fn chrome(c: &mut Canvas, title: &str, page: Option<(usize, usize)>) {
    let p = layout::device().page;
    let back = p.back;
    Icon::Back.draw(c, back.icon_x, back.icon_y, BLACK);
    c.text(Weight::Medium, back.text.size, back.text_x, back.text.baseline, Anchor::Start, "Back", BLACK);
    c.text(Weight::Medium, p.title.size, p.margin, p.title.baseline, Anchor::Start, title, BLACK);
    if let Some((n, total)) = page {
        let number = p.page_number;
        let (cx, ny) = page_number_pos();
        c.text(Weight::Regular, number.text.size, cx, ny, Anchor::Middle, &format!("{n} / {total}"), SECONDARY);
        let prev = if n == 1 { gray(80) } else { BLACK };
        let next = if n == total { gray(80) } else { BLACK };
        let arrow_y = ny - number.arrow_dy;
        Icon::PagePrevious.draw(c, cx - number.arrow_dx, arrow_y, prev);
        Icon::PageNext.draw(c, cx + number.arrow_dx, arrow_y, next);
    }
}

/// A horizontal line of a page from `x0` to `x1`, with its top at `y`.
fn hline(c: &mut Canvas, y: f32, x0: f32, x1: f32) {
    c.rect(x0, y, x1, y + layout::device().page.line, BLACK);
}

/// The center line of a row of a page with its top at `top`, on a full layout unit (rounded
/// down). The icons and the meters of the row are on it, and the baseline of its text is
/// `PageMetrics::row_text` below it.
fn row_center(top: f32, height: f32) -> f32 {
    top + (height / 2.0).floor()
}

fn avg_text(rows: &[&ResultRow]) -> String {
    average_seconds(rows.iter().copied()).map(format_clock).unwrap_or_else(|| "–".to_string())
}

/// Draws the statistics page (in page units).
fn draw_summary(c: &mut Canvas, results: &[ResultRow], confirm_all: bool) {
    chrome(c, "Statistics", None);
    let d = layout::device();
    let (p, s) = (d.page, d.summary);
    let all: Vec<&ResultRow> = results.iter().collect();
    // ANA-3
    let (bw, right, count_x) = (summary_box_w(), page_right(), summary_count_x());
    let b = s.boxes;
    for (x0, label, value) in [(p.margin, "Puzzles completed", all.len().to_string()), (p.margin + b.gap + bw, "Average time", avg_text(&all))] {
        c.stroke_rect(x0, b.top, x0 + bw, b.bottom, p.line, BLACK);
        c.text(Weight::Regular, b.label.size, x0 + b.text_x, b.top + b.label.baseline, Anchor::Start, label, SECONDARY);
        c.text(Weight::Medium, b.value.size, x0 + b.text_x, b.top + b.value.baseline, Anchor::Start, &value, BLACK);
    }
    c.text(Weight::Medium, s.heading.size, p.margin, s.heading.baseline, Anchor::Start, "By difficulty", BLACK);
    let t = s.table;
    for (x, anchor, title) in [(p.margin, Anchor::Start, "Difficulty"), (count_x, Anchor::End, "Completed"), (right, Anchor::End, "Average time")] {
        c.text(Weight::Regular, t.header.size, x, t.header.baseline, anchor, title, SECONDARY);
    }
    let mut y = t.top;
    hline(c, y, p.margin, right);
    // ANA-4
    let row = summary_row();
    let text = p.row_text;
    for mode in 0..MODES {
        let rows: Vec<&ResultRow> = results.iter().filter(|r| r.mode == mode).collect();
        let cy = row_center(y, row);
        let name = match mode {
            0 => "Easy",
            m if m == MODES - 1 => "Hard",
            _ => "",
        };
        c.text(Weight::Regular, text.size, p.margin, cy + text.baseline, Anchor::Start, name, BLACK);
        meter(c, t.meter_x, cy, t.meter_step, mode, MeterStyle::ON_WHITE);
        c.text(Weight::Regular, text.size, count_x, cy + text.baseline, Anchor::End, &rows.len().to_string(), BLACK);
        c.text(Weight::Regular, text.size, right, cy + text.baseline, Anchor::End, &avg_text(&rows), BLACK);
        y += row;
        hline(c, y, p.margin, right);
    }
    let buttons = s.buttons;
    y += buttons.below_table;
    debug_assert_eq!(y, summary_buttons_y());
    let all_right = all_games_right();
    c.stroke_rect(p.margin, y, all_right, y + p.row, p.line, BLACK);
    let mid = row_center(y, p.row);
    c.text(Weight::Medium, text.size, p.margin + buttons.all_games_text_x, mid + text.baseline, Anchor::Start, "All completed games", BLACK);
    Icon::OpenList.draw(c, all_right - buttons.all_games_icon_dx, mid, BLACK);
    // "Delete all games": gray, with no effect, if there are no results (ANA-12).
    let col = if results.is_empty() { gray(70) } else { BLACK };
    let x0 = delete_all_x();
    c.stroke_rect(x0, y, right, y + p.row, p.line, col);
    Icon::Bin.draw(c, x0 + buttons.delete_all_icon_x, mid, col);
    c.text(Weight::Medium, text.size, x0 + buttons.delete_all_text_x, mid + text.baseline, Anchor::Start, "Delete all games", col);
    if confirm_all {
        let n = results.len();
        let what = if n == 1 { "1 completed game is deleted.".to_string() } else { format!("All {n} completed games are deleted.") };
        confirm_dialog(c, "Delete all games?", &what);
    }
}

/// A confirmation dialog at the center of the page (analytics.py): a title, one line, "You cannot
/// undo this action.", Cancel and Delete.
fn confirm_dialog(c: &mut Canvas, title: &str, line: &str) {
    dialog_box(c, title, [line, "You cannot undo this action."], ["Cancel", "Delete"]);
}

/// The dialog "Restart the interface?" (§9.2, UPD-10), over all screens. The buttons are "Later" and
/// "Restart".
const RESTART_TITLE: &str = "Restart the interface?";
const RESTART_LINES: [&str; 2] = ["The update has a new ink patch. To use it,", "the reMarkable interface must start again."];

/// Draws the dialog (in page units).
fn restart_dialog(c: &mut Canvas) {
    dialog_box(c, RESTART_TITLE, RESTART_LINES, ["Later", "Restart"]);
}

/// A dialog at the center of the page, on the pale screen (analytics.py): a title, two lines, and two
/// buttons in the last row. The right button is white on black.
fn dialog_box(c: &mut Canvas, title: &str, lines: [&str; 2], buttons: [&str; 2]) {
    let (p, d) = (layout::device().page, layout::device().dialog);
    c.veil(0.6);
    let (dx0, dy0, dx1, dy1) = dialog();
    c.rect(dx0, dy0, dx1, dy1, WHITE);
    c.stroke_rect(dx0, dy0, dx1, dy1, p.line, BLACK);
    let tx = dx0 + d.text_x;
    c.text(Weight::Medium, d.title.size, tx, dy0 + d.title.baseline, Anchor::Start, title, BLACK);
    for (line, baseline) in lines.into_iter().zip(d.body_baselines) {
        c.text(Weight::Regular, d.body_size, tx, dy0 + baseline, Anchor::Start, line, SECONDARY);
    }
    let buttons_y = dy1 - p.row;
    hline(c, buttons_y, dx0, dx1);
    let mid = ((dx0 + dx1) / 2.0).floor();
    c.rect(mid, buttons_y, dx1, dy1, BLACK);
    let by = dy1 - (p.row / 2.0).floor() + p.row_text.baseline;
    c.text(Weight::Medium, p.row_text.size, ((dx0 * 3.0 + dx1) / 4.0).floor(), by, Anchor::Middle, buttons[0], BLACK);
    c.text(Weight::Medium, p.row_text.size, ((dx0 + dx1 * 3.0) / 4.0).floor(), by, Anchor::Middle, buttons[1], WHITE);
}

/// A date in local time, for example "22 Oct 2026, 20:22".
pub fn format_date(unix: i64) -> String {
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let t = unix as _;
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    // SAFETY: localtime_r writes only into `tm`.
    let ok = unsafe { !libc::localtime_r(&t, &mut tm).is_null() };
    if !ok {
        return String::from("?");
    }
    format!(
        "{} {} {}, {:02}:{:02}",
        tm.tm_mday,
        MONTHS[tm.tm_mon.clamp(0, 11) as usize],
        tm.tm_year + 1900,
        tm.tm_hour,
        tm.tm_min
    )
}

/// Draws the list of the completed games (in page units).
fn draw_list(c: &mut Canvas, app: &App) {
    let pages = app.page_count();
    chrome(c, "Completed games", Some((app.list.page, pages)));
    let d = layout::device();
    let (p, g) = (d.page, d.games_list);
    // The filter: All, then one segment for each mode. The selected segment is black.
    let (f, seg) = (g.filter, seg_w());
    let (top, bottom) = (f.top, f.top + f.height);
    for filter in list_filters() {
        let x = filter_segment_x(filter);
        let selected = app.list.filter == filter;
        if selected {
            c.rect(x, top, x + seg, bottom, BLACK);
        }
        c.stroke_rect(x, top, x + seg, bottom, p.line, BLACK);
        match filter {
            None => {
                c.text(Weight::Medium, f.all.size, x + seg / 2.0, top + f.all.baseline, Anchor::Middle, "All", if selected { WHITE } else { BLACK });
            }
            Some(mode) => {
                // The meter is at the center of the segment (24.75 units from the left in portrait on the Paper Pro).
                let mx = x + (seg - (MODES - 1) as f32 * f.meter_step) / 2.0;
                let style = if selected { MeterStyle::ON_BLACK } else { MeterStyle::ON_WHITE };
                meter(c, mx, row_center(top, f.height), f.meter_step, mode, style);
            }
        }
    }
    // One table across the full width, with the rows of `list_rows` (layout.rs).
    let (meter_dx, time_dx, bin_dx) = list_offsets();
    let (x, w) = list_column();
    let rows = app.page_rows();
    debug_assert!(rows.len() <= list_rows());
    let (row, text, header) = (p.row, p.row_text, g.header);
    c.text(Weight::Regular, header.size, x, header.baseline, Anchor::Start, "Date", SECONDARY);
    c.text(Weight::Regular, header.size, x + meter_dx, header.baseline, Anchor::Start, "Difficulty", SECONDARY);
    c.text(Weight::Regular, header.size, x + time_dx, header.baseline, Anchor::End, "Time", SECONDARY);
    hline(c, g.top, x, x + w);
    for (i, r) in rows.iter().enumerate() {
        let y = list_row(i);
        let cy = row_center(y, row);
        let (date, clock) = (format_date(r.finished_at), format_clock(r.seconds));
        // The box between the lines above and below the row (screen px): the page is white there.
        let sc = |v: f32| v * page_scale() + 0.5;
        let m = g.row_box_margin;
        let row_box = (sc(x) as i32 - m, sc(y + p.line) as i32 + m, sc(x + w) as i32 + m, sc(y + row) as i32 - 1);
        put_box(c, row_box, BoxLook::ListRow(date.clone(), r.mode, clock.clone()), |c| {
            c.text(Weight::Regular, text.size, x, cy + text.baseline, Anchor::Start, &date, BLACK);
            meter(c, x + meter_dx, cy, g.meter_step, r.mode, MeterStyle::ON_WHITE);
            c.text(Weight::Regular, text.size, x + time_dx, cy + text.baseline, Anchor::End, &clock, BLACK);
            Icon::Bin.draw(c, x + bin_dx, cy, BLACK);
        });
        hline(c, y + row, x, x + w);
    }
    if let Some(r) = &app.list.confirm {
        let when = format!("{}, completed in {}.", format_date(r.finished_at), format_clock(r.seconds));
        confirm_dialog(c, "Delete this game?", &when);
    }
}

/// The Licenses list (in page units): one row for each crate, with its name and its version. A tap
/// on a row shows its license texts.
fn draw_licenses(c: &mut Canvas, app: &App) {
    let all = crate::licenses::all();
    let pages = crate::licenses::pages(all.crates.len(), layout::licenses_rows());
    chrome(c, "Licenses", Some((app.licenses.page, pages)));
    let d = layout::device();
    let (row, l) = (d.page.row, d.licenses.list);
    let (x, w) = list_column();
    c.text(Weight::Regular, l.intro.size, x, l.intro.baseline, Anchor::Start, "The app contains these open source components.", SECONDARY);
    hline(c, l.top, x, x + w);
    for (i, index) in app.licenses_page_rows().enumerate() {
        let k = &all.crates[index];
        let y = layout::licenses_row(i);
        // The center line of the row is not rounded down (not `row_center`).
        let center = y + row / 2.0;
        let baseline = center + l.text_below_center;
        c.text(Weight::Medium, l.name_size, x, baseline, Anchor::Start, &k.name, BLACK);
        c.text(Weight::Regular, l.version_size, x + w - l.version_end_from_right, baseline, Anchor::End, &k.version, SECONDARY);
        Icon::OpenList.draw(c, x + w - l.icon_from_right, center, BLACK);
        hline(c, y + row, x, x + w);
    }
}

/// The license texts of one crate (in page units), a page at a time.
fn draw_license_text(c: &mut Canvas, app: &App) {
    use crate::licenses::Line;
    let k = &crate::licenses::all().crates[app.licenses.open];
    let (lines, pages) = app.license_text_page();
    chrome(c, &k.name, Some((app.licenses.text_page, pages)));
    let t = layout::device().licenses.text;
    let (x, w) = list_column();
    c.text(Weight::Regular, t.subtitle.size, x, t.subtitle.baseline, Anchor::Start, &format!("Version {} · {}", k.version, k.license.replace(" OR ", " / ")), SECONDARY);
    hline(c, t.divider_top, x, x + w);
    for (i, line) in lines.iter().enumerate() {
        let y = layout::license_line(i);
        match line {
            Line::Heading(s) => c.text(Weight::Medium, t.heading_size, x, y, Anchor::Start, s, BLACK),
            Line::Body(s) => c.text(Weight::Regular, t.body_size, x, y, Anchor::Start, s, BLACK),
            Line::Blank => {}
        }
    }
}

// ---- Icons

/// An icon of the app. The app and the icon sheet of the snapshot tool (`reference::icon_sheet`) use
/// `Icon::draw`, thus the sheet shows the same icons as the app. Only the menu button of the game
/// screen calls `menu_icon` directly, because its icon can be a mirror image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    MenuClosed,
    MenuOpen,
    Close,
    PaintBucket,
    Gear,
    Caution,
    Undo,
    Redo,
    Bookmark,
    Help,
    Statistics,
    Plus,
    Install,
    CheckForUpdate,
    Restart,
    Back,
    PagePrevious,
    PageNext,
    OpenList,
    Bin,
}

/// The icon of the menu button (NAV-7), with its center at (cx, cy): a ring
/// (`IconMetrics::menu_radius`) and a dot (radius 4.5 px), 5.5 px to the left of the center and
/// 5.5 px above it (open) or below it (closed). `mirror` puts the dot to the right of the center
/// (NAV-15). The same values as `menu_icon` in scripts/ui-lib.sh.
fn menu_icon(c: &mut Canvas, cx: f32, cy: f32, open: bool, mirror: bool, col: Rgb) {
    let m = layout::device().icons;
    let dx = if mirror { 5.5 } else { -5.5 };
    let dy = if open { -5.5 } else { 5.5 };
    c.circle(cx, cy, m.menu_radius, None, Some((col, m.stroke)));
    c.circle(cx + dx, cy + dy, 4.5, Some(col), None);
}

impl Icon {
    pub const ALL: [Icon; 20] = [
        Icon::MenuClosed,
        Icon::MenuOpen,
        Icon::Close,
        Icon::PaintBucket,
        Icon::Gear,
        Icon::Caution,
        Icon::Undo,
        Icon::Redo,
        Icon::Bookmark,
        Icon::Help,
        Icon::Statistics,
        Icon::Plus,
        Icon::Install,
        Icon::CheckForUpdate,
        Icon::Restart,
        Icon::Back,
        Icon::PagePrevious,
        Icon::PageNext,
        Icon::OpenList,
        Icon::Bin,
    ];

    /// True if the app draws the icon in page units (the manual and the analytics pages), not in
    /// screen pixels.
    pub fn on_page(self) -> bool {
        matches!(self, Icon::Back | Icon::PagePrevious | Icon::PageNext | Icon::OpenList | Icon::Bin)
    }

    /// Draws the icon with its center at (cx, cy). The caution icon uses `col` for its border and its
    /// mark. Each icon is a crisp drawing (no anti-aliasing; NAV-20, owner approval 2026-11-08).
    /// The stroke widths and the outer sizes come from `IconMetrics`; the numbers here are the path
    /// points of each shape.
    pub fn draw(self, c: &mut Canvas, cx: f32, cy: f32, col: Rgb) {
        let m = layout::device().icons;
        match self {
            Icon::MenuClosed | Icon::MenuOpen => menu_icon(c, cx, cy, self == Icon::MenuOpen, false, col),
            Icon::Close => {
                c.circle(cx, cy, m.close_radius, None, Some((col, m.stroke)));
                c.line(cx - 5.5, cy - 5.5, cx + 5.5, cy + 5.5, m.stroke, col);
                c.line(cx + 5.5, cy - 5.5, cx - 5.5, cy + 5.5, m.stroke, col);
            }
            Icon::PaintBucket => {
                let tf = Transform::from_translate(cx, cy);
                let mut pb = PathBuilder::new();
                pb.move_to(-18.0, 1.0);
                pb.line_to(-3.0, -14.0);
                pb.line_to(12.0, 1.0);
                pb.line_to(-3.0, 16.0);
                pb.close();
                pb.move_to(-18.0, 1.0);
                pb.line_to(12.0, 1.0);
                pb.move_to(-10.0, -7.0);
                pb.line_to(-15.0, -12.0);
                pb.cubic_to(-17.0, -14.0, -15.0, -18.0, -12.0, -17.0);
                pb.line_to(-5.0, -12.0);
                c.path_stroke(pb, tf, m.stroke, col);
                let mut pb = PathBuilder::new();
                pb.move_to(16.0, 3.0);
                pb.cubic_to(16.0, 3.0, 21.0, 10.0, 21.0, 13.0);
                pb.cubic_to(21.0, 16.0, 18.8, 18.0, 16.0, 18.0);
                pb.cubic_to(13.2, 18.0, 11.0, 16.0, 11.0, 13.0);
                pb.cubic_to(11.0, 10.0, 16.0, 3.0, 16.0, 3.0);
                pb.close();
                c.path_fill(pb, tf, col);
            }
            // A gear with eight teeth (scripts/toolbar.sh `gear_icon`).
            Icon::Gear => {
                c.circle(cx, cy, m.gear_radius, None, Some((col, m.stroke)));
                c.circle(cx, cy, 4.5, None, Some((col, m.stroke)));
                for k in 0..8 {
                    let a = (k as f32 * 45.0).to_radians();
                    let (s, co) = a.sin_cos();
                    c.line_butt(cx + m.gear_radius * co, cy + m.gear_radius * s, cx + m.gear_tooth_tip * co, cy + m.gear_tooth_tip * s, m.gear_tooth_width, col);
                }
            }
            // The red caution icon at the top right of the gear (REQUIREMENTS.md §9.2).
            Icon::Caution => {
                c.circle(cx, cy, m.caution_radius, Some(CAUTION_RED), Some((col, m.stroke)));
                c.line(cx, cy - 6.0, cx, cy + 1.5, m.stroke, col);
                c.circle(cx, cy + 5.5, 1.6, Some(col), None);
            }
            Icon::Undo | Icon::Redo => {
                let sx = if self == Icon::Redo { -1.0 } else { 1.0 };
                let tf = Transform::from_row(sx, 0.0, 0.0, 1.0, cx, cy);
                let mut pb = PathBuilder::new();
                pb.move_to(-18.0, -4.0);
                pb.line_to(3.0, -4.0);
                pb.cubic_to(13.0, -4.0, 19.0, 4.0, 19.0, 15.0);
                pb.move_to(-10.0, -12.0);
                pb.line_to(-18.0, -4.0);
                pb.line_to(-10.0, 4.0);
                c.path_stroke(pb, tf, m.stroke, col);
            }
            // A bookmark with a notch at the bottom (§15): a closed line.
            Icon::Bookmark => {
                let mut pb = PathBuilder::new();
                pb.move_to(-12.0, -18.0);
                pb.line_to(12.0, -18.0);
                pb.line_to(12.0, 18.0);
                pb.line_to(0.0, 8.0);
                pb.line_to(-12.0, 18.0);
                pb.close();
                c.path_stroke(pb, Transform::from_translate(cx, cy), m.stroke, col);
            }
            Icon::Help => {
                c.circle(cx, cy, m.help_radius, None, Some((col, m.stroke)));
                c.text(Weight::Medium, m.help_text_size, cx, cy + 9.0, Anchor::Middle, "?", col);
            }
            // A line graph on axes.
            Icon::Statistics => {
                c.polyline(&[(cx - 18.0, cy - 18.0), (cx - 18.0, cy + 18.0), (cx + 18.0, cy + 18.0)], m.stroke, col, true);
                c.polyline(&[(cx - 12.0, cy + 8.0), (cx - 4.0, cy - 2.0), (cx + 3.0, cy + 4.0), (cx + 15.0, cy - 12.0)], m.stroke, col, true);
            }
            Icon::Plus => {
                c.line(cx - 18.0, cy, cx + 18.0, cy, m.stroke, col);
                c.line(cx, cy - 18.0, cx, cy + 18.0, m.stroke, col);
            }
            // A down arrow into a tray.
            Icon::Install => {
                c.line(cx, cy - 13.0, cx, cy + 5.0, m.stroke, col);
                c.polyline(&[(cx - 8.0, cy - 3.0), (cx, cy + 5.0), (cx + 8.0, cy - 3.0)], m.stroke, col, true);
                c.polyline(&[(cx - 13.0, cy + 3.0), (cx - 13.0, cy + 13.0), (cx + 13.0, cy + 13.0), (cx + 13.0, cy + 3.0)], m.stroke, col, true);
            }
            // A circular arrow: an arc (radius 12 px) clockwise from 330 degrees (the upper right)
            // through the bottom and the left to the top (630 degrees), with the gap at the upper
            // right. A short straight line continues the arc to the tip of the arrow head, 5 px to
            // the right of the top. The head points to the right: two 7 px strokes, 45 degrees to
            // each side of the line back from its tip.
            Icon::CheckForUpdate => {
                let mut arc: Vec<(f32, f32)> = (0..=20)
                    .map(|k| {
                        let a = (330.0 + 15.0 * k as f32).to_radians();
                        (cx + 12.0 * a.cos(), cy + 12.0 * a.sin())
                    })
                    .collect();
                let (ex, ey) = (cx + 5.0, arc[arc.len() - 1].1);
                arc.push((ex, ey));
                c.polyline(&arc, m.stroke, col, true);
                let wing = |deg: f32| (ex + 7.0 * deg.to_radians().cos(), ey + 7.0 * deg.to_radians().sin());
                c.polyline(&[wing(225.0), (ex, ey), wing(135.0)], m.stroke, col, true);
            }
            // A power symbol: an arc (radius 12 px) clockwise from 300 to 600 (240) degrees, with
            // the gap at the top, and a line from 15 px to 3 px above the center.
            Icon::Restart => {
                let arc: Vec<(f32, f32)> = (0..=20)
                    .map(|k| {
                        let a = (300.0 + 15.0 * k as f32).to_radians();
                        (cx + 12.0 * a.cos(), cy + 12.0 * a.sin())
                    })
                    .collect();
                c.polyline(&arc, m.stroke, col, true);
                c.polyline(&[(cx, cy - 15.0), (cx, cy - 3.0)], m.stroke, col, true);
            }
            Icon::Back => c.polyline(&[(cx + 5.0, cy - 11.0), (cx - 5.0, cy), (cx + 5.0, cy + 11.0)], m.stroke, col, true),
            Icon::PagePrevious => c.polyline(&[(cx + 5.0, cy - 10.0), (cx - 5.0, cy), (cx + 5.0, cy + 10.0)], m.stroke, col, true),
            Icon::PageNext => c.polyline(&[(cx - 5.0, cy - 10.0), (cx + 5.0, cy), (cx - 5.0, cy + 10.0)], m.stroke, col, true),
            Icon::OpenList => c.polyline(&[(cx - 6.0, cy - 12.0), (cx + 6.0, cy), (cx - 6.0, cy + 12.0)], m.stroke, col, true),
            // The bin of a delete button.
            Icon::Bin => {
                c.line(cx - 14.0, cy - 11.0, cx + 14.0, cy - 11.0, m.bin_stroke, col);
                c.polyline(&[(cx - 5.0, cy - 11.0), (cx - 5.0, cy - 15.0), (cx + 5.0, cy - 15.0), (cx + 5.0, cy - 11.0)], m.bin_stroke, col, true);
                c.polyline(&[(cx - 10.0, cy - 11.0), (cx - 8.0, cy + 15.0), (cx + 8.0, cy + 15.0), (cx + 10.0, cy - 11.0)], m.bin_stroke, col, true);
                c.line(cx - 3.0, cy - 5.0, cx - 3.0, cy + 9.0, m.bin_stroke, col);
                c.line(cx + 3.0, cy - 5.0, cx + 3.0, cy + 9.0, m.bin_stroke, col);
            }
        }
    }
}

/// The RGB value of one pixel of an opaque pixmap.
pub fn rgb_at(pm: &Pixmap, x: i32, y: i32) -> Rgb {
    let p = pm.pixel(x as u32, y as u32).expect("pixel in the pixmap");
    [p.red(), p.green(), p.blue()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_board_in_stripes_is_the_same_as_the_board_in_one_drawing() {
        // `draw_board_stripes`: each thread draws a stripe of the board. All cell looks (numbers in
        // all colors, the "?", pencil marks at all positions, the micro-cell lines, the x at all
        // positions) at all rows.
        let fonts = Fonts::repo().unwrap();
        // Each digit in each cell, then all pencil marks (with micro-cell lines), then the x with no
        // marks and no lines, then "?".
        for pattern in 0..12 {
            let looks: Vec<_> = cells()
                .map(|cell| {
                    let k = (cell.0 * 9 + cell.1) as usize;
                    let look = match pattern {
                        0..=8 => CellLook::Value(((k + pattern) % 9) as u8 + 1, if k.is_multiple_of(2) { WHITE } else { gray(90) }, BLACK),
                        9 => CellLook::Empty(0x1FF, LineGray::Gray50, None),
                        10 => CellLook::Empty(0, LineGray::None, Some((k % 9) as u8 + 1)),
                        _ => CellLook::Question,
                    };
                    (cell, look)
                })
                .collect();
            let draw = |threads: usize| {
                let mut pm = Pixmap::new(screen_w(), screen_h()).unwrap();
                let area = board_area(&pm).unwrap();
                draw_board_stripes(&mut Canvas::new(&mut pm, &fonts), area, &looks, threads);
                pm
            };
            let one = draw(1);
            for threads in 2..=4 {
                assert!(draw(threads).data() == one.data(), "pattern {pattern}, {threads} stripes");
            }
        }
    }
}
