//! Draws the screens (REQUIREMENTS.md §3, §9 to §13 and §15). The drawing commands are the same as in
//! the scripts that make the images in `images/`.
//!
//! Coordinates are in ImageMagick units: a pixel center is at an integer coordinate. tiny-skia puts a
//! pixel center at +0.5. Thus each drawing gets a translation of 0.5 px.

use std::sync::OnceLock;

use tiny_skia::{FillRule, LineCap, LineJoin, Paint, Path, PathBuilder, Pixmap, PixmapPaint, Rect as SkRect, Stroke, Transform};

use crate::app::{average_seconds, App, Menu, Panel, Screen, MANUAL_PAGES};
use crate::font::{Anchor, Fonts, Weight};
use crate::game::{cells, format_clock, Game};
use crate::generator::MODES;
use crate::layout::{self, *};
use crate::settings::{Color, LineGray, Section, Settings, Switch};
use crate::store::ResultRow;
use crate::update::{Update, UpdateButton};

pub type Rgb = [u8; 3];

/// ImageMagick puts text a little to the left and higher than tiny-skia. This offset (px) gives the
/// smallest difference to the reference images (the snapshot tool measures it).
const TEXT_OFFSET: (f32, f32) = (-1.0, -1.0);
/// The same offset (screen px) for the screens with a scale (the analytics screens).
const TEXT_OFFSET_PAGE: (f32, f32) = (-0.5, -0.5);

/// ImageMagick draws strokes a little wider than tiny-skia. This extra width (px) gives the smallest
/// difference to the reference images.
const STROKE_EXTRA: f32 = 0.25;

pub const BLACK: Rgb = [0, 0, 0];
/// The width of the ink of the pen (px).
pub const INK_WIDTH: f32 = 5.0;
const WHITE: Rgb = [255, 255, 255];
/// The caution icon on the gear (REQUIREMENTS.md §9.2).
pub const CAUTION_RED: Rgb = [0xD6, 0x2D, 0x20];

/// The color of the micro-cell lines (BRD-6, the setting of §10). None: no lines.
pub fn line_rgb(g: LineGray) -> Option<Rgb> {
    g.percent().map(gray)
}

/// ImageMagick grayN: N percent of white.
pub const fn gray(n: u32) -> Rgb {
    let v = ((n * 255 + 50) / 100) as u8;
    [v, v, v]
}

/// The units of the coordinates that a [`Canvas`] gets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Units {
    /// Screen pixels (the game screen and the menu).
    Screen,
    /// The layout units of the manual and the analytics pages: `layout::PAGE_SCALE` screen pixels.
    Page,
}

impl Units {
    /// The quantity of screen pixels in one unit.
    fn scale(self) -> f32 {
        match self {
            Units::Screen => 1.0,
            Units::Page => PAGE_SCALE,
        }
    }

    /// The offset of the text (screen px) that gives the smallest difference to the reference images.
    fn text_offset(self) -> (f32, f32) {
        match self {
            Units::Screen => TEXT_OFFSET,
            Units::Page => TEXT_OFFSET_PAGE,
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
}

impl Default for FrameRecord {
    fn default() -> FrameRecord {
        FrameRecord { known: None, damage: Damage::All }
    }
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
    /// The damage since the last call.
    pub fn take_damage(&mut self) -> Damage {
        std::mem::replace(&mut self.damage, Damage::Areas(Vec::new()))
    }

    fn damage(&mut self, a: Area) {
        if let Damage::Areas(v) = &mut self.damage {
            if v.len() >= MAX_DAMAGE {
                self.damage = Damage::All;
            } else if v.last().is_none_or(|l| !l.contains(&a)) {
                v.push(a);
            }
        }
    }

    /// The frame has pixels that the record does not know.
    fn forget(&mut self) {
        self.known = None;
        self.damage = Damage::All;
    }

    /// A drawing (not a board tile) changed the pixels of `a`.
    fn touch(&mut self, a: Area) {
        self.damage(a);
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

impl Known {
    fn unset_slots(&mut self, a: &Area) {
        for (slot, tile) in &mut self.slots {
            if slot.overlaps(a) {
                *tile = None;
            }
        }
    }
}

fn paint(c: Rgb) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color_rgba8(c[0], c[1], c[2], 255);
    p.anti_alias = true;
    p
}

fn stroke(w: f32, round: bool) -> Stroke {
    Stroke {
        width: w + STROKE_EXTRA,
        line_cap: if round { LineCap::Round } else { LineCap::Butt },
        line_join: if round { LineJoin::Round } else { LineJoin::Miter },
        ..Stroke::default()
    }
}

impl<'a> Canvas<'a> {
    pub fn new(pm: &'a mut Pixmap, fonts: &'a Fonts) -> Canvas<'a> {
        Canvas { pm, fonts, units: Units::Screen, record: None, top: 0 }
    }

    /// A canvas that keeps `record` (refer to [`FrameRecord`]). All drawings on `pm` after the first
    /// drawing with `record` must use the same record.
    pub fn retained(pm: &'a mut Pixmap, record: &'a mut FrameRecord, fonts: &'a Fonts) -> Canvas<'a> {
        Canvas { pm, fonts, units: Units::Screen, record: Some(record), top: 0 }
    }

    /// Records that a drawing in the box `b` (screen px), with `margin` px more on each side, can
    /// change pixels.
    fn touched(&mut self, b: SkRect, margin: f32) {
        let (w, h) = (self.pm.width(), self.pm.height());
        if let (Some(rec), Some(a)) = (self.record.as_deref_mut(), Area::around(b, margin, w, h)) {
            rec.touch(a);
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
        // A box with no width or height (a straight line) gets a small size.
        match SkRect::from_ltrb(x0, y0, x1.max(x0 + 0.01), y1.max(y0 + 0.01)) {
            Some(r) => self.touched(r, 2.0 * (w + STROKE_EXTRA) * scale + 2.0),
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
        let scale = self.units.scale();
        Transform::from_row(scale, 0.0, 0.0, scale, 0.5, 0.5 - self.top as f32)
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
        let h = 0.5 / self.units.scale();
        if let Some(r) = SkRect::from_ltrb(x0 - h, y0 - h, x1 + h, y1 + h) {
            let tf = self.tf();
            self.pm.fill_rect(r, &paint(c), tf, None);
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
            self.pm.stroke_path(&path, &paint(c), &stroke(w, false), tf, None);
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
            self.pm.stroke_path(&path, &paint(c), &stroke(w, round), tf, None);
            self.touched_path(&path, tf, w);
        }
    }

    pub fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, w: f32, c: Rgb) {
        self.polyline(&[(x0, y0), (x1, y1)], w, c, true);
    }

    /// A line with square ends (the thick lines of the board).
    fn line_square(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, w: f32, c: Rgb) {
        let mut pb = PathBuilder::new();
        pb.move_to(x0, y0);
        pb.line_to(x1, y1);
        let path = pb.finish().unwrap();
        let mut s = stroke(w, false);
        s.line_cap = LineCap::Square;
        let tf = self.tf();
        self.pm.stroke_path(&path, &paint(c), &s, tf, None);
        self.touched_path(&path, tf, w);
    }

    /// A line with flat ends.
    fn line_butt(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, w: f32, c: Rgb) {
        self.polyline(&[(x0, y0), (x1, y1)], w, c, false);
    }

    pub fn circle(&mut self, cx: f32, cy: f32, r: f32, fill: Option<Rgb>, line: Option<(Rgb, f32)>) {
        let Some(path) = PathBuilder::from_circle(cx, cy, r) else { return };
        let tf = self.tf();
        if let Some(f) = fill {
            self.pm.fill_path(&path, &paint(f), FillRule::Winding, tf, None);
        }
        if let Some((c, w)) = line {
            self.pm.stroke_path(&path, &paint(c), &stroke(w, true), tf, None);
        }
        self.touched_path(&path, tf, line.map_or(0.0, |l| l.1));
    }

    fn path_stroke(&mut self, pb: PathBuilder, tf: Transform, w: f32, c: Rgb) {
        if let Some(path) = pb.finish() {
            let tf = tf.post_concat(self.tf());
            self.pm.stroke_path(&path, &paint(c), &stroke(w, true), tf, None);
            self.touched_path(&path, tf, w);
        }
    }

    fn path_fill(&mut self, pb: PathBuilder, tf: Transform, c: Rgb) {
        if let Some(path) = pb.finish() {
            let tf = tf.post_concat(self.tf());
            self.pm.fill_path(&path, &paint(c), FillRule::Winding, tf, None);
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

static TOGGLE_CLOSED: &[u8] = include_bytes!("../scripts/icons/ic-toggle-closed.png");
static TOGGLE_OPEN: &[u8] = include_bytes!("../scripts/icons/ic-toggle.png");

/// The manual pages (REQUIREMENTS.md §12), in order.
static MANUAL: [&[u8]; MANUAL_PAGES as usize] = [
    include_bytes!("../images/manual-how-to-play.png"),
    include_bytes!("../images/manual-candidates.png"),
    include_bytes!("../images/manual-pen.png"),
    include_bytes!("../images/manual-pen-rules.png"),
    include_bytes!("../images/manual-helpers.png"),
    include_bytes!("../images/manual-mistakes.png"),
    include_bytes!("../images/manual-menu.png"),
    include_bytes!("../images/manual-new-game.png"),
    include_bytes!("../images/manual-finished.png"),
];

/// The same pages for the landscape orientation (scripts/manual-*-landscape.sh).
static MANUAL_LANDSCAPE: [&[u8]; MANUAL_PAGES as usize] = [
    include_bytes!("../images/manual-how-to-play-landscape.png"),
    include_bytes!("../images/manual-candidates-landscape.png"),
    include_bytes!("../images/manual-pen-landscape.png"),
    include_bytes!("../images/manual-pen-rules-landscape.png"),
    include_bytes!("../images/manual-helpers-landscape.png"),
    include_bytes!("../images/manual-mistakes-landscape.png"),
    include_bytes!("../images/manual-menu-landscape.png"),
    include_bytes!("../images/manual-new-game-landscape.png"),
    include_bytes!("../images/manual-finished-landscape.png"),
];

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

fn toggle_icon(open: bool) -> &'static Pixmap {
    static OPEN: OnceLock<Pixmap> = OnceLock::new();
    static CLOSED: OnceLock<Pixmap> = OnceLock::new();
    if open {
        OPEN.get_or_init(|| decode_png(TOGGLE_OPEN))
    } else {
        CLOSED.get_or_init(|| decode_png(TOGGLE_CLOSED))
    }
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
    if layout::orientation() == layout::Orientation::Landscape {
        LANDSCAPE[i].get_or_init(|| load(MANUAL_LANDSCAPE[i]))
    } else {
        PAGES[i].get_or_init(|| load(MANUAL[i]))
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
    }
    if app.update.dialog_shown() {
        c.in_page_units(restart_dialog);
    }
}

/// The font and the size of the two labels of the difficulty slider.
const SLIDER_LABEL_WEIGHT: Weight = Weight::Medium;
const SLIDER_LABEL_SIZE: f32 = 25.0;

/// The text widths of the labels "Easy" and "Hard" of the difficulty slider.
pub fn slider_label_widths(fonts: &Fonts) -> (f32, f32) {
    let font = fonts.get(SLIDER_LABEL_WEIGHT);
    (font.width(SLIDER_LABEL_SIZE, "Easy"), font.width(SLIDER_LABEL_SIZE, "Hard"))
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
    draw_board(c, g, &app.settings, app.ui.highlight, app.ui.question.map(|q| q.0));

    // Ink that waits for the recognition pause, and the stroke that the pen draws now.
    let ink = app.ui.pending_ink.iter().flat_map(|i| i.strokes.iter()).chain(app.live_stroke.iter());
    for s in ink {
        if s.len() == 1 {
            c.circle(s[0].0, s[0].1, 2.5, Some(BLACK), None);
        } else if s.len() > 1 {
            c.polyline(s, INK_WIDTH, BLACK, true);
        }
    }

    // The difficulty meter, or "Completed in" (CMP-4, DIF-6).
    if g.completed {
        let text = format!("Completed in {}", format_clock(g.seconds()));
        c.text(Weight::Medium, 25.0, (board_x() + BOARD_SIZE / 2) as f32, (meter_y() + 9) as f32, Anchor::Middle, &text, BLACK);
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
    };
    put_overlay(c, menu, |c| {
        c.image(toggle_icon(app.ui.menu.is_open()), tb_x(), 0, layout::right_handed());
        if let Menu::Open(panel) = app.ui.menu {
            draw_toolbar(c, app, panel);
        }
    });
    if let Menu::Open(Some(panel)) = app.ui.menu {
        draw_panel(c, app, panel);
    }
}

/// A group of drawings out of the board that the overlay cache keeps (refer to `put_overlay`): all
/// that its pixels depend on, with the layout.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum OverlayLook {
    /// The difficulty meter at this mode.
    Meter(u8),
    /// The menu button and, if `panel` is Some, the toolbar with the button of this panel active
    /// (`draw_toolbar`, without the panel).
    Menu { panel: Option<Option<Panel>>, caution: bool, undo: bool, redo: bool },
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
            rec.touch(a);
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
    /// An empty cell with its pencil marks and the color of its micro-cell lines (None: no lines).
    Empty(crate::game::Marks, Option<Rgb>),
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

/// Draws one cell with its top left at (xf, yf): the background or the faint micro-cell grid, then
/// the number, the "?" or the pencil marks.
fn draw_cell(c: &mut Canvas, look: CellLook, xf: f32, yf: f32) {
    let cs = CELL as f32;
    match look {
        CellLook::Value(_, bg, _) => c.rect(xf, yf, xf + cs, yf + cs, bg),
        CellLook::Question => {}
        CellLook::Empty(_, None) => {}
        CellLook::Empty(_, Some(line)) => {
            // The micro-cell grid of an empty cell, in the gray of the setting (BRD-6).
            for k in 1..=2 {
                let o = micro_offset(k) as f32;
                c.line_butt(xf + o, yf, xf + o, yf + cs, 2.0, line);
                c.line_butt(xf, yf + o, xf + cs, yf + o, 2.0, line);
            }
        }
    }
    // Numbers: starting numbers black, your numbers gray35 (BRD-8). Pencil marks only on empty cells
    // (MRK-2), gray15, at their keypad positions (MRK-1).
    const MARK_POS: [f32; 3] = [24.0, 74.0, 122.0];
    match look {
        CellLook::Value(v, _, col) => c.text(Weight::Regular, 104.0, xf + (CELL / 2) as f32, yf + 111.0, Anchor::Middle, &v.to_string(), col),
        CellLook::Question => c.text(Weight::Regular, 104.0, xf + (CELL / 2) as f32, yf + 111.0, Anchor::Middle, "?", gray(40)),
        CellLook::Empty(marks, _) => {
            for d in 1..=9u8 {
                if marks & crate::game::mark_bit(d) != 0 {
                    let i = (d - 1) as usize;
                    let (cx, cy) = (xf + MARK_POS[i % 3], yf + MARK_POS[i / 3]);
                    c.text(Weight::Regular, 31.0, cx, cy + 11.0, Anchor::Middle, &d.to_string(), gray(15));
                }
            }
        }
    }
}

/// The lines of the board, in the order of the drawing: the thin lines, then the thick lines. Only the
/// lines that `keep` accepts (the index 0 to 9 and true for a vertical line). With `near`, only the
/// part of each line near the area (x0, y0, x1, y1): the pixels of that area are the same as with the
/// full lines, because the lines are straight and the ends of the parts are out of the area.
fn board_lines(c: &mut Canvas, keep: impl Fn(i32, bool) -> bool, near: Option<(i32, i32, i32, i32)>) {
    let (x0, y0, b) = (board_x() as f32, board_y() as f32, BOARD_SIZE as f32);
    // The part of a line from `a` to `a + b` near the span s0..s1 (with a margin larger than the
    // ends of the thick lines).
    let part = |a: f32, s0: i32, s1: i32| match near {
        None => (a, a + b),
        Some(_) => ((s0 as f32 - 16.0).max(a), (s1 as f32 + 16.0).min(a + b)),
    };
    let (nx0, ny0, nx1, ny1) = near.unwrap_or((0, 0, 0, 0));
    let (vy0, vy1) = part(y0, ny0, ny1);
    let (hx0, hx1) = part(x0, nx0, nx1);
    for i in (0..=9).filter(|i| i % 3 != 0) {
        let o = (i * CELL) as f32;
        if keep(i, true) {
            c.line_butt(x0 + o, vy0, x0 + o, vy1, 3.0, BLACK);
        }
        if keep(i, false) {
            c.line_butt(hx0, y0 + o, hx1, y0 + o, 3.0, BLACK);
        }
    }
    for i in (0..=9).step_by(3) {
        let o = (i * CELL) as f32;
        if keep(i, true) {
            c.line_square(x0 + o, vy0, x0 + o, vy1, 13.0, BLACK);
        }
        if keep(i, false) {
            c.line_square(hx0, y0 + o, hx1, y0 + o, 13.0, BLACK);
        }
    }
}

/// The width of the band around the cells that the thick lines of the board touch (px).
const BOARD_MARGIN: usize = 8;

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
fn draw_board(c: &mut Canvas, g: &Game, settings: &Settings, highlight: Option<u8>, question: Option<crate::game::Cell>) {
    let (bx, by) = (board_x(), board_y());
    let Some(area) = board_area(c.pm) else {
        // Not on the screen: draw without the cache (this does not occur with the layouts).
        for cell in cells() {
            let (x, y) = cell_origin(cell);
            draw_cell(c, cell_look(g, settings, highlight, question, cell), x as f32, y as f32);
        }
        board_lines(c, |_, _| true, None);
        return;
    };
    let fonts = c.fonts as *const Fonts as usize;
    let n = CELL as usize + 1;
    let looks: Vec<_> = cells().map(|cell| (cell, cell_look(g, settings, highlight, question, cell))).collect();
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
                put_cached(c, *k, x, y, n, n, |c| {
                    draw_cell(c, look, xi as f32, yi as f32);
                    let near = Some((xi, yi, xi + CELL, yi + CELL));
                    board_lines(c, |i, vertical| if vertical { i == kc || i == kc + 1 } else { i == r || i == r + 1 }, near);
                });
            }
            None => put_cached(c, *k, x, y, w, h, |c| board_lines(c, |_, _| true, None)),
        }
        c.set_slot(i, *k);
    }
}

/// The rows that a stripe of the board draws above and below its rows (refer to `draw_board_stripes`).
const STRIPE_MARGIN: usize = 8;

/// Draws the full board (all cells, then all lines) on white in the area `area` of the frame. The
/// threads draw horizontal stripes of the area: each thread draws all of the board on its own
/// stripe pixmap, with the screen rows of the stripe (an integer translation gives the same pixels).
fn draw_board_stripes(c: &mut Canvas, (ax, ay, aw, ah): (usize, usize, usize, usize), looks: &[(crate::game::Cell, CellLook)], threads: usize) {
    let fw = c.pm.width();
    let fonts = c.fonts;
    // The stripes end at the borders of cell rows: a glyph that the edge of the pixmap clips gets
    // other pixels (tiny-skia cuts its curves), and no glyph crosses these borders.
    let by = board_y() as usize;
    let mut edges: Vec<usize> = (1..threads).map(|k| by + (9 * k / threads) * CELL as usize).collect();
    edges.insert(0, ay);
    edges.push(ay + ah);
    edges.dedup();
    let stripes: Vec<(usize, usize, usize, Pixmap)> = std::thread::scope(|scope| {
        let handles: Vec<_> = edges
            .windows(2)
            .map(|e| {
                let (y0, h) = (e[0], e[1] - e[0]);
                scope.spawn(move || {
                    // The clip at the edge of a pixmap changes the pixels near it: the stripe pixmap
                    // has STRIPE_MARGIN more rows above and below.
                    let top = y0 - STRIPE_MARGIN.min(y0);
                    let mut pm = Pixmap::new(fw, (y0 - top + h + STRIPE_MARGIN) as u32).unwrap();
                    pm.fill(tiny_skia::Color::WHITE);
                    let mut s = Canvas { pm: &mut pm, fonts, units: Units::Screen, record: None, top: top as i32 };
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

/// The slots of the board: the area of each cell tile (CELL + 1 px, in the order of `cells`), then
/// the band around the cells (top, bottom, left and right). Together they are the board area.
fn board_slots() -> impl Iterator<Item = Area> {
    let n = CELL as usize + 1;
    let (mu, bu) = (BOARD_MARGIN, BOARD_SIZE as usize);
    let (bxu, byu) = (board_x() as usize, board_y() as usize);
    let s = bxu - mu;
    let bands = [
        Area::xywh(s, byu - mu, bu + 1 + 2 * mu, mu),
        Area::xywh(s, byu + bu + 1, bu + 1 + 2 * mu, mu),
        Area::xywh(s, byu, mu, bu + 1),
        Area::xywh(bxu + bu + 1, byu, mu, bu + 1),
    ];
    cells()
        .map(move |cell| {
            let (x, y) = cell_origin(cell);
            Area::xywh(x as usize, y as usize, n, n)
        })
        .chain(bands)
}

/// The board area that `draw_board` writes fully: the cells and the band around them (x, y, w, h).
/// None if the area is not fully on the screen (then `draw_board` does not use the tiles).
fn board_area(pm: &Pixmap) -> Option<(usize, usize, usize, usize)> {
    let (bx, by, m) = (board_x(), board_y(), BOARD_MARGIN as i32);
    let (fw, fh) = (pm.width() as i32, pm.height() as i32);
    if bx < m || by < m || bx + BOARD_SIZE + m >= fw || by + BOARD_SIZE + m >= fh {
        return None;
    }
    let n = (BOARD_SIZE + 1 + 2 * m) as usize;
    Some(((bx - m) as usize, (by - m) as usize, n, n))
}

/// The largest quantity of cached tiles (approximately 87 KB each, thus 22 MB). One frame uses 85.
/// When the cache is full, it keeps only the tiles of the last frame.
const MAX_TILES: usize = 256;

/// With more new tiles than this, `draw_board` draws the full board (it is faster than the tiles).
const COLD_TILES: usize = 16;

/// The look of a cell for the state of the game.
fn cell_look(g: &Game, settings: &Settings, highlight: Option<u8>, question: Option<crate::game::Cell>, cell: crate::game::Cell) -> CellLook {
    let v = g.value_at(cell);
    if v != 0 {
        // BRD-7: the first rule that applies.
        let bg = if g.is_clashing(cell) {
            settings.color_clash.rgb()
        } else if highlight == Some(v) {
            settings.color_highlight.rgb()
        } else if g.is_given(cell) {
            settings.color_starting.rgb()
        } else {
            Color::White.rgb()
        };
        CellLook::Value(v, bg, if g.is_given(cell) { BLACK } else { gray(35) })
    } else if question == Some(cell) {
        CellLook::Question
    } else {
        CellLook::Empty(g.visible_marks(cell, settings.auto_hide_on), line_rgb(settings.micro_line))
    }
}

/// The difficulty slider with its labels (ui-lib.sh `labeled_slider`).
fn labeled_slider(c: &mut Canvas, xl: i32, xr: i32, y: i32, level: u8) {
    c.text(SLIDER_LABEL_WEIGHT, SLIDER_LABEL_SIZE, xl as f32, (y + 9) as f32, Anchor::Start, "Easy", BLACK);
    c.text(SLIDER_LABEL_WEIGHT, SLIDER_LABEL_SIZE, xr as f32, (y + 9) as f32, Anchor::End, "Hard", BLACK);
    let stops = slider_stops(c.fonts, xl, xr);
    slider(c, &stops, y as f32, level);
}

fn slider(c: &mut Canvas, stops: &[f32; STOPS as usize], y: f32, level: u8) {
    let (xa, xb) = (stops[0], stops[STOPS as usize - 1]);
    let xk = stops[level as usize];
    c.line(xk, y, xb, y, 3.0, gray(60));
    c.line(xa, y, xk, y, 5.0, BLACK);
    for (i, &x) in stops.iter().enumerate() {
        let fill = if (i as u8) < level { BLACK } else { WHITE };
        c.circle(x, y, 6.0, Some(fill), Some((BLACK, 3.0)));
    }
    c.circle(xk, y, 13.0, Some(BLACK), Some((WHITE, 3.0)));
}

/// The open menu (toolbar.sh).
fn draw_toolbar(c: &mut Canvas, app: &App, panel: Option<Panel>) {
    // The close button (top left, or top right for a right-handed player).
    let (cx, cy) = (close_cx(), CLOSE_CY);
    c.circle(cx, cy, 17.5, None, Some((BLACK, 3.0)));
    c.line(cx - 5.5, cy - 5.5, cx + 5.5, cy + 5.5, 3.0, BLACK);
    c.line(cx + 5.5, cy - 5.5, cx - 5.5, cy + 5.5, 3.0, BLACK);

    // A panel button is white on black while its panel is open (NAV-8).
    let colors_active = panel == Some(Panel::Colors);
    let settings_active = panel == Some(Panel::Settings);
    let plus_active = panel == Some(Panel::NewGame);
    if colors_active {
        active_slot(c, TB_SLOT);
    }
    if settings_active {
        active_slot(c, 2 * TB_SLOT);
    }
    gear(c, tb_cx(), tb_cy(2), if settings_active { WHITE } else { BLACK });
    if app.update.caution() {
        caution(c, tb_cx() + 16.0, tb_cy(2) - 16.0);
    }
    if plus_active {
        active_slot(c, tb_bottom_y());
    }

    // Cell colors: a paint bucket.
    let line = if colors_active { WHITE } else { BLACK };
    let tf = Transform::from_translate(tb_cx(), tb_cy(1));
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
    c.path_stroke(pb, tf, 3.0, line);
    let mut pb = PathBuilder::new();
    pb.move_to(16.0, 3.0);
    pb.cubic_to(16.0, 3.0, 21.0, 10.0, 21.0, 13.0);
    pb.cubic_to(21.0, 16.0, 18.8, 18.0, 16.0, 18.0);
    pb.cubic_to(13.2, 18.0, 11.0, 16.0, 11.0, 13.0);
    pb.cubic_to(11.0, 10.0, 16.0, 3.0, 16.0, 3.0);
    pb.close();
    c.path_fill(pb, tf, line);

    // Undo and redo: black when available, gray when not (UND-7, UND-8).
    for (k, available) in [(3, app.game.can_undo()), (4, app.game.can_redo())] {
        let sx = if k == 4 { -1.0 } else { 1.0 };
        let tf = Transform::from_row(sx, 0.0, 0.0, 1.0, tb_cx(), tb_cy(k));
        let col = if available { BLACK } else { gray(70) };
        let mut pb = PathBuilder::new();
        pb.move_to(-18.0, -4.0);
        pb.line_to(3.0, -4.0);
        pb.cubic_to(13.0, -4.0, 19.0, 4.0, 19.0, 15.0);
        pb.move_to(-10.0, -12.0);
        pb.line_to(-18.0, -4.0);
        pb.line_to(-10.0, 4.0);
        c.path_stroke(pb, tf, 3.0, col);
    }

    // + (new game), Analytics and Help at the bottom.
    let cy = (tb_bottom_y() + 56) as f32;
    let fg = if plus_active { WHITE } else { BLACK };
    let x = tb_cx();
    c.line(x - 18.0, cy, x + 18.0, cy, 3.0, fg);
    c.line(tb_cx(), cy - 18.0, tb_cx(), cy + 18.0, 3.0, fg);
    let ay = (tb_bottom_y() - 56) as f32;
    c.polyline(&[(x - 18.0, ay - 18.0), (x - 18.0, ay + 18.0), (x + 18.0, ay + 18.0)], 3.0, BLACK, true);
    c.polyline(&[(x - 12.0, ay + 8.0), (x - 4.0, ay - 2.0), (x + 3.0, ay + 4.0), (x + 15.0, ay - 12.0)], 3.0, BLACK, true);
    let hy = (tb_bottom_y() - 168) as f32;
    c.circle(tb_cx(), hy, 19.0, None, Some((BLACK, 3.0)));
    c.text(Weight::Medium, 26.0, tb_cx(), hy + 9.0, Anchor::Middle, "?", BLACK);

}

/// The open panel of the menu.
fn draw_panel(c: &mut Canvas, app: &App, panel: Panel) {
    match panel {
        Panel::Colors => draw_colors_panel(c, &app.settings),
        Panel::Settings => draw_settings_panel(c, &app.settings, &app.update),
        Panel::NewGame => draw_new_game_panel(c, app.ui.slider),
    }
}

fn active_slot(c: &mut Canvas, y: i32) {
    c.rect(tb_x() as f32, y as f32, (tb_x() + TB_SLOT) as f32, (y + 111) as f32, BLACK);
}

/// A pop-up frame that touches the toolbar (toolbar.sh `panel`).
fn panel_frame(c: &mut Canvas, pl: i32, pr: i32, top: i32, bot: i32) {
    let (pl, pr, top, bot) = (pl as f32, pr as f32, top as f32, bot as f32);
    c.rect(pl, top, pr, bot, WHITE);
    c.rect(pl, top, pr, top + 1.0, BLACK);
    c.rect(pl, top, pl + 1.0, bot, BLACK);
    c.rect(pr - 1.0, top, pr, bot, BLACK);
    if bot < screen_h() as f32 {
        c.rect(pl, bot - 1.0, pr, bot, BLACK);
    }
}

/// Settings (slot 2): a gear with eight teeth (scripts/toolbar.sh `gear_icon`).
fn gear(c: &mut Canvas, cx: f32, cy: f32, col: Rgb) {
    c.circle(cx, cy, 11.0, None, Some((col, 3.0)));
    c.circle(cx, cy, 4.5, None, Some((col, 3.0)));
    for k in 0..8 {
        let a = (k as f32 * 45.0).to_radians();
        let (s, co) = a.sin_cos();
        c.line_butt(cx + 11.0 * co, cy + 11.0 * s, cx + 17.0 * co, cy + 17.0 * s, 6.0, col);
    }
}

/// The red caution icon at the top right of the gear (REQUIREMENTS.md §9.2), with its center at (x, y).
fn caution(c: &mut Canvas, x: f32, y: f32) {
    c.circle(x, y, 10.0, Some(CAUTION_RED), Some((WHITE, 3.0)));
    c.line(x, y - 6.0, x, y + 1.5, 3.0, WHITE);
    c.circle(x, y + 5.5, 1.6, Some(WHITE), None);
}

/// The Settings panel (REQUIREMENTS.md §9.1): one row with a name and a switch for each `Switch`, the version, and the
/// update row of a stable or a preview build (§9.2).
fn draw_settings_panel(c: &mut Canvas, settings: &Settings, update: &Update) {
    let (pl, pr) = (colors_pl(), colors_pr());
    let p = layout::settings_panel(update.has_row());
    panel_frame(c, pl, pr, p.y0, p.y1 - 1);
    for s in Switch::ALL {
        let r = layout::settings_row(s.row());
        if s.row() > 0 {
            c.rect(pl as f32, (r.y0 - 2) as f32, pr as f32, (r.y0 - 1) as f32, BLACK);
        }
        let cy = (r.y0 + 56) as f32;
        let name = s.label();
        let first = if name.len() == 1 { cy + 9.0 } else { cy - 7.0 };
        for (k, line) in name.iter().enumerate() {
            c.text(Weight::Medium, 25.0, (pl + 33) as f32, first + 32.0 * k as f32, Anchor::Start, line, BLACK);
        }
        switch(c, (pr - 33) as f32, cy, settings.is_on(s));
    }
    let r = layout::settings_row(layout::VERSION_ROW);
    c.rect(pl as f32, (r.y0 - 2) as f32, pr as f32, (r.y0 - 1) as f32, BLACK);
    let y = (r.y0 + 65) as f32;
    c.text(Weight::Medium, 25.0, (pl + 33) as f32, y, Anchor::Start, "Version", BLACK);
    c.text(Weight::Regular, 25.0, (pr - 33) as f32, y, Anchor::End, &update.current, BLACK);
    if !update.has_row() {
        return;
    }
    let r = layout::settings_row(layout::UPDATE_ROW);
    c.rect(pl as f32, (r.y0 - 2) as f32, pr as f32, (r.y0 - 1) as f32, BLACK);
    if let Some(text) = update.row_text() {
        c.text(Weight::Regular, 25.0, (pl + 33) as f32, (r.y0 + 65) as f32, Anchor::Start, text, gray(40));
    } else if let Some(button) = update.button() {
        // A box with a 2 px border, the icon and the text.
        let b = layout::update_button();
        let (x0, y0, x1, y1) = (b.x0 as f32, b.y0 as f32, b.x1 as f32, b.y1 as f32);
        c.rect(x0, y0, x1, y0 + 1.0, BLACK);
        c.rect(x0, y1 - 1.0, x1, y1, BLACK);
        c.rect(x0, y0, x0 + 1.0, y1, BLACK);
        c.rect(x1 - 1.0, y0, x1, y1, BLACK);
        let (ix, cy) = (x0 + 40.0, (r.y0 + 56) as f32);
        match button {
            UpdateButton::Install(_) => {
                // A down arrow into a tray.
                c.line(ix, cy - 13.0, ix, cy + 5.0, 3.0, BLACK);
                c.polyline(&[(ix - 8.0, cy - 3.0), (ix, cy + 5.0), (ix + 8.0, cy - 3.0)], 3.0, BLACK, true);
                c.polyline(&[(ix - 13.0, cy + 3.0), (ix - 13.0, cy + 13.0), (ix + 13.0, cy + 13.0), (ix + 13.0, cy + 3.0)], 3.0, BLACK, true);
            }
            UpdateButton::Check => {
                // A circular arrow: an arc (radius 12 px) clockwise from the right (0 degrees) through
                // the bottom and the top to 255 degrees, then a gap, and an arrow head on the circle at
                // 330 degrees. The head points clockwise along the circle: two 8 px strokes, 45 degrees
                // to each side of the line back from its tip.
                let arc: Vec<(f32, f32)> = (0..=17)
                    .map(|k| {
                        let a = (15.0 * k as f32).to_radians();
                        (ix + 12.0 * a.cos(), cy + 12.0 * a.sin())
                    })
                    .collect();
                c.polyline(&arc, 3.0, BLACK, true);
                let t = 330f32.to_radians();
                let (ex, ey) = (ix + 12.0 * t.cos(), cy + 12.0 * t.sin());
                let wing = |deg: f32| (ex + 8.0 * deg.to_radians().cos(), ey + 8.0 * deg.to_radians().sin());
                c.polyline(&[wing(195.0), (ex, ey), wing(285.0)], 3.0, BLACK, true);
            }
            UpdateButton::Restart => {
                // A power symbol: an arc (radius 12 px) clockwise from 300 to 600 (240) degrees, with
                // the gap at the top, and a line from 15 px to 3 px above the center.
                let arc: Vec<(f32, f32)> = (0..=20)
                    .map(|k| {
                        let a = (300.0 + 15.0 * k as f32).to_radians();
                        (ix + 12.0 * a.cos(), cy + 12.0 * a.sin())
                    })
                    .collect();
                c.polyline(&arc, 3.0, BLACK, true);
                c.polyline(&[(ix, cy - 15.0), (ix, cy - 3.0)], 3.0, BLACK, true);
            }
        }
        c.text(Weight::Medium, 25.0, x0 + 72.0, cy + 9.0, Anchor::Start, button.text(), BLACK);
    }
}

/// A switch with its right end at x: a track of 72 x 36 px and a knob of 28 px.
fn switch(c: &mut Canvas, x1: f32, cy: f32, on: bool) {
    let (x0, r) = (x1 - 72.0, 18.0);
    let mut pb = PathBuilder::new();
    pb.move_to(x0 + r, cy - r);
    pb.line_to(x1 - r, cy - r);
    pb.cubic_to(x1 - r + 9.94, cy - r, x1, cy - 9.94, x1, cy);
    pb.cubic_to(x1, cy + 9.94, x1 - r + 9.94, cy + r, x1 - r, cy + r);
    pb.line_to(x0 + r, cy + r);
    pb.cubic_to(x0 + r - 9.94, cy + r, x0, cy + 9.94, x0, cy);
    pb.cubic_to(x0, cy - 9.94, x0 + r - 9.94, cy - r, x0 + r, cy - r);
    pb.close();
    let track = pb.clone();
    c.path_fill(pb, Transform::identity(), if on { BLACK } else { WHITE });
    c.path_stroke(track, Transform::identity(), 3.0, BLACK);
    if on {
        c.circle(x1 - r, cy, 14.0, Some(WHITE), None);
    } else {
        c.circle(x0 + r, cy, 14.0, Some(BLACK), None);
    }
}

fn draw_colors_panel(c: &mut Canvas, settings: &Settings) {
    let (pl, pr) = (colors_pl(), colors_pr());
    panel_frame(c, pl, pr, COLORS_TOP, COLORS_TOP + COLOR_SECTIONS * COLOR_SECTION_H);
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
            (color.rgb(), color.name().to_string(), state)
        });
        color_section(c, section.panel_index(), section.title(), &items);
    }
    // The Micro-cell lines section: "None" is a white dot.
    let items = LineGray::ALL.map(|g| {
        let state = if g == settings.micro_line { Swatch::Selected } else { Swatch::Available };
        (line_rgb(g).unwrap_or(WHITE), g.name(), state)
    });
    color_section(c, LINES_SECTION, "Micro-cell lines", &items);
}

/// The state of an item in a section of the Cell colors panel.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Swatch {
    Available,
    Selected,
    /// A different section uses the color (COL-5).
    Taken,
}

/// One section of the Cell colors panel (toolbar.sh `color_section`): the divider above it (except
/// the first section), the title, and a 3x3 grid of swatches with their names. A black box shows the
/// selected item. A taken item has a gray ring, a gray name and a line through the dot.
fn color_section(c: &mut Canvas, index: usize, title: &str, items: &[(Rgb, String, Swatch); 9]) {
    let (pl, pr) = (colors_pl(), colors_pr());
    let top = layout::color_section_top(index);
    if index > 0 {
        c.rect(pl as f32, top as f32, pr as f32, (top + 1) as f32, BLACK);
    }
    c.text(Weight::Medium, 25.0, (pl + 33) as f32, (top + COLOR_TITLE_Y) as f32, Anchor::Start, title, BLACK);
    for (i, (rgb, name, state)) in items.iter().enumerate() {
        let cell = layout::color_grid_cell(index, i);
        let (x0, y0) = (cell.x0 as f32, cell.y0 as f32);
        // The swatch: centered in the width of the cell, above the name.
        let (cx, cy) = (x0 + (COLOR_CELL / 2) as f32, y0 + 30.0);
        let (fg, ring) = match state {
            Swatch::Selected => {
                c.rect(x0, y0, (cell.x1 - 1) as f32, (cell.y1 - 1) as f32, BLACK);
                (WHITE, WHITE)
            }
            Swatch::Taken => (gray(70), gray(70)),
            Swatch::Available => (BLACK, BLACK),
        };
        c.circle(cx, cy, 19.0, Some(*rgb), Some((ring, 2.0)));
        if *state == Swatch::Taken {
            c.line(cx - 17.0, cy + 17.0, cx + 17.0, cy - 17.0, 2.5, gray(50));
        }
        c.text(Weight::Medium, 25.0, cx, y0 + 76.0, Anchor::Middle, name, fg);
    }
}

fn draw_new_game_panel(c: &mut Canvas, slider_level: u8) {
    panel_frame(c, ng_pl(), ng_pr(), ng_top(), screen_h() as i32);
    c.rect(ng_pl() as f32, tb_bottom_y() as f32, ng_pr() as f32, (tb_bottom_y() + 1) as f32, BLACK);
    labeled_slider(c, ng_tx(), ng_tr(), ng_slider_y(), slider_level);
    c.text(Weight::Medium, 25.0, ng_tx() as f32, (ng_start_y() + 9) as f32, Anchor::Start, "Start new game", BLACK);
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
    c.line(xk, y, xb, y, 2.5, METER_TRACK);
    c.line(xa, y, xk, y, 4.5, on);
    for i in 0..MODES {
        let x = xa + i as f32 * step;
        c.circle(x, y, 5.0, Some(if i < level { on } else { hollow }), Some((on, 2.5)));
    }
    c.circle(xk, y, 11.0, Some(on), Some((hollow, 2.5)));
}

/// "< Back", the title and the page arrows.
fn chrome(c: &mut Canvas, title: &str, page: Option<(usize, usize)>) {
    c.polyline(&[(48.0, 37.0), (38.0, 48.0), (48.0, 59.0)], 3.0, BLACK, true);
    c.text(Weight::Medium, 22.0, 66.0, 56.0, Anchor::Start, "Back", BLACK);
    c.text(Weight::Medium, 84.0, 72.0, 170.0, Anchor::Start, title, BLACK);
    if let Some((n, total)) = page {
        let (cx, ny) = page_number_pos();
        c.text(Weight::Regular, 28.0, cx, ny, Anchor::Middle, &format!("{n} / {total}"), gray(50));
        let prev = if n == 1 { gray(80) } else { BLACK };
        let next = if n == total { gray(80) } else { BLACK };
        c.polyline(&[(cx - 74.0, ny - 20.0), (cx - 84.0, ny - 10.0), (cx - 74.0, ny)], 3.0, prev, true);
        c.polyline(&[(cx + 74.0, ny - 20.0), (cx + 84.0, ny - 10.0), (cx + 74.0, ny)], 3.0, next, true);
    }
}

fn hline(c: &mut Canvas, y: f32, x0: f32, x1: f32) {
    c.rect(x0, y, x1, y + 1.7, BLACK);
}

fn avg_text(rows: &[&ResultRow]) -> String {
    average_seconds(rows.iter().copied()).map(format_clock).unwrap_or_else(|| "–".to_string())
}

/// Draws the statistics page (in page units).
fn draw_summary(c: &mut Canvas, results: &[ResultRow], confirm_all: bool) {
    chrome(c, "Statistics", None);
    let all: Vec<&ResultRow> = results.iter().collect();
    // ANA-3
    let (bw, right, count_x) = (summary_box_w(), page_right(), summary_count_x());
    for (x0, label, value) in [(72.0, "Puzzles completed", all.len().to_string()), (100.0 + bw, "Average time", avg_text(&all))] {
        c.stroke_rect(x0, 250.0, x0 + bw, 440.0, 1.7, BLACK);
        c.text(Weight::Regular, 30.0, x0 + 30.0, 305.0, Anchor::Start, label, gray(40));
        c.text(Weight::Medium, 84.0, x0 + 30.0, 405.0, Anchor::Start, &value, BLACK);
    }
    c.text(Weight::Medium, 50.0, 72.0, 550.0, Anchor::Start, "By difficulty", BLACK);
    c.text(Weight::Regular, 30.0, 72.0, 615.0, Anchor::Start, "Difficulty", gray(40));
    c.text(Weight::Regular, 30.0, count_x, 615.0, Anchor::End, "Completed", gray(40));
    c.text(Weight::Regular, 30.0, right, 615.0, Anchor::End, "Average time", gray(40));
    let mut y = SUMMARY_TABLE_Y;
    hline(c, y, 72.0, right);
    // ANA-4
    let row = summary_row();
    for mode in 0..MODES {
        let rows: Vec<&ResultRow> = results.iter().filter(|r| r.mode == mode).collect();
        let cy = y + (row / 2.0).floor();
        let name = match mode {
            0 => "Easy",
            m if m == MODES - 1 => "Hard",
            _ => "",
        };
        c.text(Weight::Regular, 34.0, 72.0, cy + 12.0, Anchor::Start, name, BLACK);
        meter(c, 230.0, cy, METER_STEP, mode, MeterStyle::ON_WHITE);
        c.text(Weight::Regular, 34.0, count_x, cy + 12.0, Anchor::End, &rows.len().to_string(), BLACK);
        c.text(Weight::Regular, 34.0, right, cy + 12.0, Anchor::End, &avg_text(&rows), BLACK);
        y += row;
        hline(c, y, 72.0, right);
    }
    y += 70.0;
    debug_assert_eq!(y, summary_buttons_y());
    let all_right = right - DELETE_ALL_W - 28.0;
    c.stroke_rect(72.0, y, all_right, y + ROW, 1.7, BLACK);
    let mid = y + (ROW / 2.0).floor();
    c.text(Weight::Medium, 34.0, 102.0, mid + 12.0, Anchor::Start, "All completed games", BLACK);
    c.polyline(&[(all_right - 40.0, mid - 12.0), (all_right - 28.0, mid), (all_right - 40.0, mid + 12.0)], 3.0, BLACK, true);
    // "Delete all games": gray, with no effect, if there are no results (ANA-12).
    let col = if results.is_empty() { gray(70) } else { BLACK };
    let x0 = right - DELETE_ALL_W;
    c.stroke_rect(x0, y, right, y + ROW, 1.7, col);
    bin(c, x0 + 50.0, mid, col);
    c.text(Weight::Medium, 34.0, x0 + 90.0, mid + 12.0, Anchor::Start, "Delete all games", col);
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
    c.veil(0.6);
    let (dx0, dy0, dx1, dy1) = dialog();
    c.rect(dx0, dy0, dx1, dy1, WHITE);
    c.stroke_rect(dx0, dy0, dx1, dy1, 1.7, BLACK);
    c.text(Weight::Medium, 44.0, dx0 + 40.0, dy0 + 75.0, Anchor::Start, title, BLACK);
    c.text(Weight::Regular, 34.0, dx0 + 40.0, dy0 + 140.0, Anchor::Start, lines[0], gray(25));
    c.text(Weight::Regular, 34.0, dx0 + 40.0, dy0 + 190.0, Anchor::Start, lines[1], gray(25));
    hline(c, dy1 - ROW, dx0, dx1);
    let mid = ((dx0 + dx1) / 2.0).floor();
    c.rect(mid, dy1 - ROW, dx1, dy1, BLACK);
    let by = dy1 - (ROW / 2.0).floor() + 12.0;
    c.text(Weight::Medium, 34.0, ((dx0 * 3.0 + dx1) / 4.0).floor(), by, Anchor::Middle, buttons[0], BLACK);
    c.text(Weight::Medium, 34.0, ((dx0 + dx1 * 3.0) / 4.0).floor(), by, Anchor::Middle, buttons[1], WHITE);
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
    // The filter: All, then one segment for each mode. The selected segment is black.
    let seg = seg_w();
    for filter in list_filters() {
        let x = filter_segment_x(filter);
        let selected = app.list.filter == filter;
        if selected {
            c.rect(x, SEG_Y, x + seg, SEG_Y + 80.0, BLACK);
        }
        c.stroke_rect(x, SEG_Y, x + seg, SEG_Y + 80.0, 1.7, BLACK);
        match filter {
            None => {
                c.text(Weight::Medium, 32.0, x + seg / 2.0, SEG_Y + 52.0, Anchor::Middle, "All", if selected { WHITE } else { BLACK });
            }
            Some(mode) => {
                // The meter is at the center of the segment (24.75 units from the left in portrait).
                let mx = x + (seg - (MODES - 1) as f32 * FILTER_METER_STEP) / 2.0;
                let style = if selected { MeterStyle::ON_BLACK } else { MeterStyle::ON_WHITE };
                meter(c, mx, SEG_Y + 40.0, FILTER_METER_STEP, mode, style);
            }
        }
    }
    // One table across the full width: 12 rows in portrait, 6 rows in landscape (layout.rs).
    let (meter_dx, time_dx, bin_dx) = list_offsets();
    let (x, w) = list_column();
    let rows = app.page_rows();
    debug_assert!(rows.len() <= list_rows());
    c.text(Weight::Regular, 30.0, x, 395.0, Anchor::Start, "Date", gray(40));
    c.text(Weight::Regular, 30.0, x + meter_dx, 395.0, Anchor::Start, "Difficulty", gray(40));
    c.text(Weight::Regular, 30.0, x + time_dx, 395.0, Anchor::End, "Time", gray(40));
    hline(c, LIST_Y, x, x + w);
    for (i, r) in rows.iter().enumerate() {
        let y = list_row(i);
        let cy = y + (ROW / 2.0).floor();
        c.text(Weight::Regular, 34.0, x, cy + 12.0, Anchor::Start, &format_date(r.finished_at), BLACK);
        meter(c, x + meter_dx, cy, METER_STEP, r.mode, MeterStyle::ON_WHITE);
        c.text(Weight::Regular, 34.0, x + time_dx, cy + 12.0, Anchor::End, &format_clock(r.seconds), BLACK);
        bin(c, x + bin_dx, cy, BLACK);
        hline(c, y + ROW, x, x + w);
    }
    if let Some(r) = &app.list.confirm {
        let when = format!("{}, completed in {}.", format_date(r.finished_at), format_clock(r.seconds));
        confirm_dialog(c, "Delete this game?", &when);
    }
}

/// The bin icon of a delete button, with its center at (bx, cy).
fn bin(c: &mut Canvas, bx: f32, cy: f32, col: Rgb) {
    let w = 2.6;
    c.line(bx - 14.0, cy - 11.0, bx + 14.0, cy - 11.0, w, col);
    c.polyline(&[(bx - 5.0, cy - 11.0), (bx - 5.0, cy - 15.0), (bx + 5.0, cy - 15.0), (bx + 5.0, cy - 11.0)], w, col, true);
    c.polyline(&[(bx - 10.0, cy - 11.0), (bx - 8.0, cy + 15.0), (bx + 8.0, cy + 15.0), (bx + 10.0, cy - 11.0)], w, col, true);
    c.line(bx - 3.0, cy - 5.0, bx - 3.0, cy + 9.0, w, col);
    c.line(bx + 3.0, cy - 5.0, bx + 3.0, cy + 9.0, w, col);
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
        // all colors, the "?", pencil marks at all positions, the micro-cell lines) at all rows.
        let fonts = Fonts::repo().unwrap();
        // Each digit in each cell, then all pencil marks (with micro-cell lines), then "?".
        for pattern in 0..11 {
            let looks: Vec<_> = cells()
                .map(|cell| {
                    let k = (cell.0 * 9 + cell.1) as usize;
                    let look = match pattern {
                        0..=8 => CellLook::Value(((k + pattern) % 9) as u8 + 1, if k.is_multiple_of(2) { WHITE } else { gray(90) }, BLACK),
                        9 => CellLook::Empty(0x1FF, Some(gray(50))),
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
