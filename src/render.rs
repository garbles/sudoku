//! Draws the screens (REQUIREMENTS.md §3, §9 to §13 and §15). The drawing commands are the same as in
//! the scripts that make the images in `images/`.
//!
//! Coordinates are in ImageMagick units: a pixel center is at an integer coordinate. tiny-skia puts a
//! pixel center at +0.5. Thus each drawing gets a translation of 0.5 px.

use std::sync::OnceLock;

use tiny_skia::{FillRule, LineCap, LineJoin, Paint, PathBuilder, Pixmap, PixmapPaint, Rect as SkRect, Stroke, Transform};

use crate::app::{average_seconds, App, Panel, Screen, MANUAL_PAGES, PER_PAGE};
use crate::font::{Anchor, Font, Fonts};
use crate::game::{cells, format_clock, Game};
use crate::generator::MODES;
use crate::layout::{self, *};
use crate::settings::{Color, Section, Settings};
use crate::store::ResultRow;

pub type Rgb = [u8; 3];

/// ImageMagick puts text a little to the left and higher than tiny-skia. This offset (px) gives the
/// smallest difference to the reference images (the snapshot tool measures it).
const TEXT_OFFSET: (f32, f32) = (-1.0, -1.0);
/// The same offset (screen px) for the screens with a scale (the analytics screens).
const TEXT_OFFSET_SCALED: (f32, f32) = (-0.5, -0.5);

/// ImageMagick draws strokes a little wider than tiny-skia. This extra width (px) gives the smallest
/// difference to the reference images.
const STROKE_EXTRA: f32 = 0.25;

pub const BLACK: Rgb = [0, 0, 0];
/// The width of the ink of the pen (px).
pub const INK_WIDTH: f32 = 5.0;
pub const WHITE: Rgb = [255, 255, 255];

/// ImageMagick grayN: N percent of white.
pub const fn gray(n: u32) -> Rgb {
    let v = ((n * 255 + 50) / 100) as u8;
    [v, v, v]
}

pub struct Canvas<'a> {
    pub pm: &'a mut Pixmap,
    pub fonts: &'a Fonts,
    /// The scale of the layout units (1 on the game screen; `layout::page_scale` on the analytics).
    scale: f32,
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
        Canvas { pm, fonts, scale: 1.0 }
    }

    fn tf(&self) -> Transform {
        Transform::from_row(self.scale, 0.0, 0.0, self.scale, 0.5, 0.5)
    }

    pub fn clear(&mut self) {
        self.pm.fill(tiny_skia::Color::WHITE);
    }

    /// ImageMagick `rectangle x0,y0 x1,y1` with a fill: the pixels from x0 to x1, x1 included.
    pub fn rect(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, c: Rgb) {
        let h = 0.5 / self.scale;
        if let Some(r) = SkRect::from_ltrb(x0 - h, y0 - h, x1 + h, y1 + h) {
            let tf = self.tf();
            self.pm.fill_rect(r, &paint(c), tf, None);
        }
    }

    /// A rectangle with a transparent white fill (the pale list behind a dialog).
    pub fn veil(&mut self, alpha: f32) {
        let mut p = Paint::default();
        p.set_color(tiny_skia::Color::from_rgba(1.0, 1.0, 1.0, alpha).unwrap());
        let r = SkRect::from_xywh(0.0, 0.0, screen_w() as f32, screen_h() as f32).unwrap();
        self.pm.fill_rect(r, &p, Transform::identity(), None);
    }

    pub fn stroke_rect(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, w: f32, c: Rgb) {
        if let Some(r) = SkRect::from_ltrb(x0, y0, x1, y1) {
            let path = PathBuilder::from_rect(r);
            let tf = self.tf();
            self.pm.stroke_path(&path, &paint(c), &stroke(w, false), tf, None);
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
        }
    }

    pub fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, w: f32, c: Rgb) {
        self.polyline(&[(x0, y0), (x1, y1)], w, c, true);
    }

    /// A line with square ends (the thick lines of the board).
    pub fn line_square(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, w: f32, c: Rgb) {
        let mut pb = PathBuilder::new();
        pb.move_to(x0, y0);
        pb.line_to(x1, y1);
        let path = pb.finish().unwrap();
        let mut s = stroke(w, false);
        s.line_cap = LineCap::Square;
        let tf = self.tf();
        self.pm.stroke_path(&path, &paint(c), &s, tf, None);
    }

    /// A line with flat ends.
    pub fn line_butt(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, w: f32, c: Rgb) {
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
    }

    pub fn path_stroke(&mut self, pb: PathBuilder, tf: Transform, w: f32, c: Rgb) {
        if let Some(path) = pb.finish() {
            let tf = tf.post_concat(self.tf());
            self.pm.stroke_path(&path, &paint(c), &stroke(w, true), tf, None);
        }
    }

    pub fn path_fill(&mut self, pb: PathBuilder, tf: Transform, c: Rgb) {
        if let Some(path) = pb.finish() {
            let tf = tf.post_concat(self.tf());
            self.pm.fill_path(&path, &paint(c), FillRule::Winding, tf, None);
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn text(&mut self, medium: bool, size: f32, x: f32, y: f32, anchor: Anchor, s: &str, c: Rgb) {
        let font: &Font = if medium { &self.fonts.medium } else { &self.fonts.regular };
        let tf = self.tf();
        let (dx, dy) = if self.scale == 1.0 { TEXT_OFFSET } else { TEXT_OFFSET_SCALED };
        font.draw(self.pm, size, x + dx / self.scale, y + dy / self.scale, anchor, s, c, tf);
    }

    pub fn text_width(&self, medium: bool, size: f32, s: &str) -> f32 {
        let font = if medium { &self.fonts.medium } else { &self.fonts.regular };
        font.width(size, s)
    }

    /// Uses the layout units of the manual and the analytics pages (true) or screen pixels (false).
    pub fn set_page_units(&mut self, on: bool) {
        self.scale = if on { page_scale() } else { 1.0 };
    }

    /// Puts an image with its top left at a screen pixel.
    pub fn image(&mut self, img: &Pixmap, x: i32, y: i32) {
        self.pm.draw_pixmap(x, y, img.as_ref(), &PixmapPaint::default(), Transform::identity(), None);
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
    for i in 0..n {
        let (r, g, b, a) = match info.color_type {
            png::ColorType::Grayscale => (buf[i], buf[i], buf[i], 255),
            png::ColorType::GrayscaleAlpha => (buf[2 * i], buf[2 * i], buf[2 * i], buf[2 * i + 1]),
            png::ColorType::Rgb => (buf[3 * i], buf[3 * i + 1], buf[3 * i + 2], 255),
            png::ColorType::Rgba => (buf[4 * i], buf[4 * i + 1], buf[4 * i + 2], buf[4 * i + 3]),
            png::ColorType::Indexed => unreachable!("expanded by the decoder"),
        };
        // Premultiplied alpha.
        let pm_ = |v: u8| ((v as u32 * a as u32 + 127) / 255) as u8;
        out[4 * i..4 * i + 4].copy_from_slice(&[pm_(r), pm_(g), pm_(b), a]);
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

fn manual_page(page: u8) -> &'static Pixmap {
    static PAGES: [OnceLock<Pixmap>; MANUAL_PAGES as usize] = [const { OnceLock::new() }; MANUAL_PAGES as usize];
    static LANDSCAPE: [OnceLock<Pixmap>; MANUAL_PAGES as usize] = [const { OnceLock::new() }; MANUAL_PAGES as usize];
    let i = (page.clamp(1, MANUAL_PAGES) - 1) as usize;
    if layout::orientation() == layout::Orientation::Landscape {
        LANDSCAPE[i].get_or_init(|| decode_png(MANUAL_LANDSCAPE[i]))
    } else {
        PAGES[i].get_or_init(|| decode_png(MANUAL[i]))
    }
}

// ---- Screens

/// Draws the full screen for the state of the app.
pub fn draw(c: &mut Canvas, app: &App) {
    c.clear();
    match app.screen {
        Screen::Game => draw_game(c, app),
        Screen::Manual => {
            let page = manual_page(app.manual_page);
            c.image(page, 0, 0);
        }
        Screen::Analytics => draw_summary(c, &app.results),
        Screen::GamesList => draw_list(c, app),
    }
}

/// The x positions of the stops of the New game slider (layout.rs).
pub fn ng_stops(fonts: &Fonts) -> [f32; 5] {
    let (we, wi) = (fonts.medium.width(25.0, "Easy"), fonts.medium.width(25.0, "Hard"));
    layout::slider_stops(ng_tx(), ng_tr(), we, wi)
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
        c.text(true, 25.0, (board_x() + BOARD_SIZE / 2) as f32, (meter_y() + 9) as f32, Anchor::Middle, &text, BLACK);
    } else {
        labeled_slider(c, meter_xl(), meter_xr(), meter_y(), g.mode);
    }

    // The menu button. Its icon shows the menu state (NAV-7).
    c.image(toggle_icon(app.ui.menu_open), tb_x(), 0);
    if app.ui.menu_open {
        draw_toolbar(c, app);
    }
}

/// The board (render.sh and board-state.py).
pub fn draw_board(c: &mut Canvas, g: &Game, settings: &Settings, highlight: Option<u8>, question: Option<crate::game::Cell>) {
    let cs = CELL as f32;
    for cell in cells() {
        let (x, y) = cell_origin(cell);
        let (xf, yf) = (x as f32, y as f32);
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
            c.rect(xf, yf, xf + cs, yf + cs, bg);
        } else if question != Some(cell) {
            // The faint micro-cell grid of an empty cell (BRD-6).
            for k in 1..=2 {
                let o = micro_offset(k) as f32;
                c.line_butt(xf + o, yf, xf + o, yf + cs, 2.0, gray(80));
                c.line_butt(xf, yf + o, xf + cs, yf + o, 2.0, gray(80));
            }
        }
    }
    let (x0, y0, b) = (board_x() as f32, board_y() as f32, BOARD_SIZE as f32);
    for i in (0..=9).filter(|i| i % 3 != 0) {
        let o = (i * CELL) as f32;
        c.line_butt(x0 + o, y0, x0 + o, y0 + b, 3.0, BLACK);
        c.line_butt(x0, y0 + o, x0 + b, y0 + o, 3.0, BLACK);
    }
    for i in (0..=9).step_by(3) {
        let o = (i * CELL) as f32;
        c.line_square(x0 + o, y0, x0 + o, y0 + b, 13.0, BLACK);
        c.line_square(x0, y0 + o, x0 + b, y0 + o, 13.0, BLACK);
    }
    // Numbers: starting numbers black, your numbers gray35 (BRD-8). Pencil marks only on empty cells
    // (MRK-2), gray15, at their keypad positions (MRK-1).
    const MARK_POS: [f32; 3] = [24.0, 74.0, 122.0];
    for cell in cells() {
        let (x, y) = cell_origin(cell);
        let (xf, yf) = (x as f32, y as f32);
        let v = g.value_at(cell);
        if v != 0 {
            let col = if g.is_given(cell) { BLACK } else { gray(35) };
            c.text(false, 104.0, xf + (CELL / 2) as f32, yf + 111.0, Anchor::Middle, &v.to_string(), col);
        } else if question == Some(cell) {
            c.text(false, 104.0, xf + (CELL / 2) as f32, yf + 111.0, Anchor::Middle, "?", gray(40));
        } else {
            let marks = g.marks_at(cell);
            for d in 1..=9u8 {
                if marks & crate::game::mark_bit(d) != 0 {
                    let i = (d - 1) as usize;
                    let (cx, cy) = (xf + MARK_POS[i % 3], yf + MARK_POS[i / 3]);
                    c.text(false, 31.0, cx, cy + 11.0, Anchor::Middle, &d.to_string(), gray(15));
                }
            }
        }
    }
}

/// The difficulty slider with its labels (ui-lib.sh `labeled_slider`).
fn labeled_slider(c: &mut Canvas, xl: i32, xr: i32, y: i32, level: u8) {
    let (we, wi) = (c.text_width(true, 25.0, "Easy"), c.text_width(true, 25.0, "Hard"));
    c.text(true, 25.0, xl as f32, (y + 9) as f32, Anchor::Start, "Easy", BLACK);
    c.text(true, 25.0, xr as f32, (y + 9) as f32, Anchor::End, "Hard", BLACK);
    let stops = layout::slider_stops(xl, xr, we, wi);
    slider(c, &stops, y as f32, level);
}

fn slider(c: &mut Canvas, stops: &[f32; 5], y: f32, level: u8) {
    let (xa, xb) = (stops[0], stops[4]);
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
fn draw_toolbar(c: &mut Canvas, app: &App) {
    let panel = app.ui.panel;
    // The close button (top left).
    c.circle(CLOSE_CX, CLOSE_CY, 17.5, None, Some((BLACK, 3.0)));
    c.line(50.0, 50.0, 61.0, 61.0, 3.0, BLACK);
    c.line(61.0, 50.0, 50.0, 61.0, 3.0, BLACK);

    // A panel button is white on black while its panel is open (NAV-8).
    let colors_active = panel == Panel::Colors;
    let plus_active = panel == Panel::NewGame;
    if colors_active {
        active_slot(c, TB_SLOT);
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
    for (k, available) in [(2, app.game.can_undo()), (3, app.game.can_redo())] {
        let sx = if k == 3 { -1.0 } else { 1.0 };
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
    c.text(true, 26.0, tb_cx(), hy + 9.0, Anchor::Middle, "?", BLACK);

    match panel {
        Panel::Colors => draw_colors_panel(c, &app.settings),
        Panel::NewGame => draw_new_game_panel(c, app.ui.slider),
        Panel::None => {}
    }
}

fn active_slot(c: &mut Canvas, y: i32) {
    c.rect(tb_x() as f32, y as f32, screen_w() as f32, (y + 111) as f32, BLACK);
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

fn draw_colors_panel(c: &mut Canvas, settings: &Settings) {
    let (pl, pr) = (colors_pl(), colors_pr());
    panel_frame(c, pl, pr, COLORS_TOP, COLORS_TOP + 3 * COLOR_SECTION_H);
    for (si, &section) in Section::ALL.iter().enumerate() {
        let top = COLORS_TOP + si as i32 * COLOR_SECTION_H;
        if si > 0 {
            c.rect(pl as f32, top as f32, pr as f32, (top + 1) as f32, BLACK);
        }
        c.text(true, 25.0, (pl + 33) as f32, (top + 52) as f32, Anchor::Start, section.title(), BLACK);
        let selected = settings.color(section);
        for (i, &color) in Color::ALL.iter().enumerate() {
            let (gx, gy) = layout::color_cell(si, i);
            let (cx, cy) = ((gx + 56) as f32, (gy + 37) as f32);
            let (gxf, gyf) = (gx as f32, gy as f32);
            let mut fg = BLACK;
            let mut ring = BLACK;
            let taken = settings.is_taken(section, color);
            if color == selected {
                c.rect(gxf, gyf, gxf + 111.0, gyf + 111.0, BLACK);
                fg = WHITE;
                ring = WHITE;
            } else if taken {
                fg = gray(70);
                ring = gray(70);
            }
            c.circle(cx, cy, 19.0, Some(color.rgb()), Some((ring, 2.0)));
            if taken && color != selected {
                c.line(cx - 17.0, cy + 17.0, cx + 17.0, cy - 17.0, 2.5, gray(50));
            }
            c.text(true, 25.0, cx, gyf + 88.0, Anchor::Middle, color.name(), fg);
        }
    }
}

fn draw_new_game_panel(c: &mut Canvas, slider_level: u8) {
    panel_frame(c, ng_pl(), ng_pr(), ng_top(), screen_h() as i32);
    c.rect(ng_pl() as f32, tb_bottom_y() as f32, ng_pr() as f32, (tb_bottom_y() + 1) as f32, BLACK);
    labeled_slider(c, ng_tx(), ng_tr(), ng_slider_y(), slider_level);
    c.text(true, 25.0, ng_tx() as f32, (ng_start_y() + 9) as f32, Anchor::Start, "Start new game", BLACK);
}

// ---- Analytics (analytics.py)

/// The difficulty meter of the analytics screens (no labels).
fn meter(c: &mut Canvas, xa: f32, y: f32, level: u8, on: Rgb, off: Rgb, hollow: Rgb) {
    let step = 34.0;
    let (xk, xb) = (xa + level as f32 * step, xa + (MODES - 1) as f32 * step);
    c.line(xk, y, xb, y, 2.5, off);
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
    c.text(true, 22.0, 66.0, 56.0, Anchor::Start, "Back", BLACK);
    c.text(true, 84.0, 72.0, 170.0, Anchor::Start, title, BLACK);
    if let Some((n, total)) = page {
        let (cx, ny) = page_number_pos();
        c.text(false, 28.0, cx, ny, Anchor::Middle, &format!("{n} / {total}"), gray(50));
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

pub fn draw_summary(c: &mut Canvas, results: &[ResultRow]) {
    c.set_page_units(true);
    chrome(c, "Analytics", None);
    let all: Vec<&ResultRow> = results.iter().collect();
    // ANA-3
    let (bw, right, count_x) = (summary_box_w(), page_right(), summary_count_x());
    for (x0, label, value) in [(72.0, "Puzzles completed", all.len().to_string()), (100.0 + bw, "Average time", avg_text(&all))] {
        c.stroke_rect(x0, 250.0, x0 + bw, 440.0, 1.7, BLACK);
        c.text(false, 30.0, x0 + 30.0, 305.0, Anchor::Start, label, gray(40));
        c.text(true, 84.0, x0 + 30.0, 405.0, Anchor::Start, &value, BLACK);
    }
    c.text(true, 50.0, 72.0, 550.0, Anchor::Start, "By difficulty", BLACK);
    c.text(false, 30.0, 72.0, 615.0, Anchor::Start, "Difficulty", gray(40));
    c.text(false, 30.0, count_x, 615.0, Anchor::End, "Completed", gray(40));
    c.text(false, 30.0, right, 615.0, Anchor::End, "Average time", gray(40));
    let mut y = SUMMARY_TABLE_Y;
    hline(c, y, 72.0, right);
    // ANA-4
    for mode in 0..MODES {
        let rows: Vec<&ResultRow> = results.iter().filter(|r| r.mode == mode).collect();
        let cy = y + (ROW / 2.0).floor();
        let name = match mode {
            0 => "Easy",
            4 => "Hard",
            _ => "",
        };
        c.text(false, 34.0, 72.0, cy + 12.0, Anchor::Start, name, BLACK);
        meter(c, 230.0, cy, mode, BLACK, gray(60), WHITE);
        c.text(false, 34.0, count_x, cy + 12.0, Anchor::End, &rows.len().to_string(), BLACK);
        c.text(false, 34.0, right, cy + 12.0, Anchor::End, &avg_text(&rows), BLACK);
        y += ROW;
        hline(c, y, 72.0, right);
    }
    y += 70.0;
    c.stroke_rect(72.0, y, right, y + ROW, 1.7, BLACK);
    let mid = y + (ROW / 2.0).floor();
    c.text(true, 34.0, 102.0, mid + 12.0, Anchor::Start, "All completed games", BLACK);
    c.polyline(&[(right - 40.0, mid - 12.0), (right - 28.0, mid), (right - 40.0, mid + 12.0)], 3.0, BLACK, true);
    c.set_page_units(false);
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

fn draw_list(c: &mut Canvas, app: &App) {
    c.set_page_units(true);
    let pages = app.page_count();
    chrome(c, "Completed games", Some((app.list.page, pages)));
    // The filter: All, then one segment for each mode. The selected segment is black.
    let seg = seg_w();
    for i in 0..=MODES as usize {
        let x = SEG_X + i as f32 * seg;
        let selected = match app.list.filter {
            None => i == 0,
            Some(m) => i == m as usize + 1,
        };
        if selected {
            c.rect(x, SEG_Y, x + seg, SEG_Y + 80.0, BLACK);
        }
        c.stroke_rect(x, SEG_Y, x + seg, SEG_Y + 80.0, 1.7, BLACK);
        // The meter is at the center of the segment (37 units from the left in portrait).
        let mx = x + (seg - 136.0) / 2.0;
        if i == 0 {
            c.text(true, 32.0, x + seg / 2.0, SEG_Y + 52.0, Anchor::Middle, "All", if selected { WHITE } else { BLACK });
        } else if selected {
            meter(c, mx, SEG_Y + 40.0, i as u8 - 1, WHITE, gray(60), BLACK);
        } else {
            meter(c, mx, SEG_Y + 40.0, i as u8 - 1, BLACK, gray(60), WHITE);
        }
    }
    // One column of 12 rows in portrait, two columns of 6 rows in landscape (layout.rs).
    let (meter_dx, time_dx, bin_dx) = list_offsets();
    let rows = app.page_rows();
    debug_assert!(rows.len() <= PER_PAGE);
    for col in 0..list_columns() {
        let (x, w) = list_column(col);
        if col == 0 || rows.len() > col * LIST_ROWS / list_columns() {
            c.text(false, 30.0, x, 395.0, Anchor::Start, "Date", gray(40));
            c.text(false, 30.0, x + meter_dx, 395.0, Anchor::Start, "Difficulty", gray(40));
            c.text(false, 30.0, x + time_dx, 395.0, Anchor::End, "Time", gray(40));
            hline(c, LIST_Y, x, x + w);
        }
    }
    for (i, r) in rows.iter().enumerate() {
        let (col, y) = list_row(i);
        let (x, w) = list_column(col);
        let cy = y + (ROW / 2.0).floor();
        c.text(false, 34.0, x, cy + 12.0, Anchor::Start, &format_date(r.finished_at), BLACK);
        meter(c, x + meter_dx, cy, r.mode, BLACK, gray(60), WHITE);
        c.text(false, 34.0, x + time_dx, cy + 12.0, Anchor::End, &format_clock(r.seconds), BLACK);
        bin(c, x + bin_dx, cy);
        hline(c, y + ROW, x, x + w);
    }
    if let Some(r) = &app.list.confirm {
        c.veil(0.6);
        let (dx0, dy0, dx1, dy1) = dialog();
        c.rect(dx0, dy0, dx1, dy1, WHITE);
        c.stroke_rect(dx0, dy0, dx1, dy1, 1.7, BLACK);
        c.text(true, 44.0, dx0 + 40.0, dy0 + 75.0, Anchor::Start, "Delete this game?", BLACK);
        let when = format!("{}, completed in {}.", format_date(r.finished_at), format_clock(r.seconds));
        c.text(false, 34.0, dx0 + 40.0, dy0 + 140.0, Anchor::Start, &when, gray(25));
        c.text(false, 34.0, dx0 + 40.0, dy0 + 190.0, Anchor::Start, "You cannot undo this action.", gray(25));
        hline(c, dy1 - ROW, dx0, dx1);
        let mid = ((dx0 + dx1) / 2.0).floor();
        c.rect(mid, dy1 - ROW, dx1, dy1, BLACK);
        let by = dy1 - (ROW / 2.0).floor() + 12.0;
        c.text(true, 34.0, ((dx0 * 3.0 + dx1) / 4.0).floor(), by, Anchor::Middle, "Cancel", BLACK);
        c.text(true, 34.0, ((dx0 + dx1 * 3.0) / 4.0).floor(), by, Anchor::Middle, "Delete", WHITE);
    }
    c.set_page_units(false);
}

fn bin(c: &mut Canvas, bx: f32, cy: f32) {
    let w = 2.6;
    c.line(bx - 14.0, cy - 11.0, bx + 14.0, cy - 11.0, w, BLACK);
    c.polyline(&[(bx - 5.0, cy - 11.0), (bx - 5.0, cy - 15.0), (bx + 5.0, cy - 15.0), (bx + 5.0, cy - 11.0)], w, BLACK, true);
    c.polyline(&[(bx - 10.0, cy - 11.0), (bx - 8.0, cy + 15.0), (bx + 8.0, cy + 15.0), (bx + 10.0, cy - 11.0)], w, BLACK, true);
    c.line(bx - 3.0, cy - 5.0, bx - 3.0, cy + 9.0, w, BLACK);
    c.line(bx + 3.0, cy - 5.0, bx + 3.0, cy + 9.0, w, BLACK);
}

/// The RGB value of one pixel of an opaque pixmap.
pub fn rgb_at(pm: &Pixmap, x: i32, y: i32) -> Rgb {
    let p = pm.pixel(x as u32, y as u32).expect("pixel in the pixmap");
    [p.red(), p.green(), p.blue()]
}
