//! The UI font: reMarkable Sans (REQUIREMENTS.md §1). The app loads the font from the tablet when it
//! starts. The app does not include a copy of the font.

use std::path::Path;

use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Transform};
use ttf_parser::{Face, GlyphId, Tag};

/// The font file of the device. It is a variable font (axes `wght` and `ital`).
const DEVICE_FONT: &str = "/usr/share/remarkable/webui/reMarkableSans.woff2";

pub struct Font {
    face: Face<'static>,
}

pub struct Fonts {
    /// reMarkable Sans Regular (weight 400): board digits and body text.
    regular: Font,
    /// reMarkable Sans Medium (weight 500): UI text.
    medium: Font,
}

/// The weight of reMarkable Sans that a text uses.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Weight {
    /// Weight 400: board digits and body text.
    Regular,
    /// Weight 500: UI text.
    Medium,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
    Start,
    Middle,
    End,
}

impl Fonts {
    /// Loads the variable font of the device (or a different file, for the tests on a computer).
    /// `SUDOKU_FONT` changes the file. `SUDOKU_FONT_MEDIUM` gives a second file for the medium weight,
    /// if the first file is not a variable font.
    pub fn load() -> Result<Fonts, String> {
        let regular = std::env::var("SUDOKU_FONT").unwrap_or_else(|_| DEVICE_FONT.to_string());
        let medium = std::env::var("SUDOKU_FONT_MEDIUM").unwrap_or_else(|_| regular.clone());
        Ok(Fonts { regular: Font::load(Path::new(&regular), 400.0)?, medium: Font::load(Path::new(&medium), 500.0)? })
    }

    /// Loads the static fonts of the repository (scripts/fonts/rmsans-400.ttf and rmsans-500.ttf), for
    /// the tests and the tools on a computer (refer to scripts/fonts/README.md).
    pub fn repo() -> Result<Fonts, String> {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/fonts");
        Ok(Fonts { regular: Font::load(&dir.join("rmsans-400.ttf"), 400.0)?, medium: Font::load(&dir.join("rmsans-500.ttf"), 500.0)? })
    }

    /// The fonts of the tools on a computer (bench, snapshot, video): the file of `SUDOKU_FONT` (as
    /// `load`) if it is set, else the fonts of the repository.
    pub fn for_tools() -> Result<Fonts, String> {
        if std::env::var_os("SUDOKU_FONT").is_some() {
            Fonts::load()
        } else {
            Fonts::repo()
        }
    }

    /// The font of a weight.
    pub fn get(&self, weight: Weight) -> &Font {
        match weight {
            Weight::Regular => &self.regular,
            Weight::Medium => &self.medium,
        }
    }
}

impl Font {
    pub fn load(path: &Path, weight: f32) -> Result<Font, String> {
        let raw = std::fs::read(path).map_err(|e| format!("cannot read the font {}: {e}", path.display()))?;
        let data = if raw.starts_with(b"wOF2") {
            wuff::decompress_woff2(&raw).map_err(|e| format!("cannot decode the font {}: {e:?}", path.display()))?
        } else if raw.starts_with(b"wOFF") {
            wuff::decompress_woff1(&raw).map_err(|e| format!("cannot decode the font {}: {e:?}", path.display()))?
        } else {
            raw
        };
        // The app keeps the font until it stops.
        let data: &'static [u8] = Box::leak(data.into_boxed_slice());
        let mut face = Face::parse(data, 0).map_err(|e| format!("cannot parse the font {}: {e}", path.display()))?;
        if face.is_variable() {
            face.set_variation(Tag::from_bytes(b"wght"), weight);
            face.set_variation(Tag::from_bytes(b"ital"), 0.0);
        }
        Ok(Font { face })
    }

    fn scale(&self, size: f32) -> f32 {
        size / self.face.units_per_em() as f32
    }

    /// The advance of a glyph in px, rounded to a full pixel as FreeType does (the reference images
    /// use FreeType through ImageMagick).
    fn advance(&self, gid: GlyphId, s: f32) -> f32 {
        (self.face.glyph_hor_advance(gid).unwrap_or(0) as f32 * s).round()
    }

    fn glyph(&self, ch: char) -> GlyphId {
        self.face.glyph_index(ch).unwrap_or(GlyphId(0))
    }

    /// The advance width of a text, in px.
    pub fn width(&self, size: f32, text: &str) -> f32 {
        let s = self.scale(size);
        text.chars().map(|ch| self.advance(self.glyph(ch), s)).sum()
    }

    /// Draws a crisp text (no anti-aliasing, NAV-20), with its baseline at y: each pixel is fully
    /// `color` or stays as it was. `transform` maps the result to the screen. Returns the path of the
    /// glyphs (None: no glyph outline).
    #[allow(clippy::too_many_arguments)]
    pub fn draw(&self, pixmap: &mut Pixmap, size: f32, x: f32, y: f32, anchor: Anchor, text: &str, color: [u8; 3], transform: Transform) -> Option<tiny_skia::Path> {
        let s = self.scale(size);
        let w = self.width(size, text);
        let mut pen_x = match anchor {
            Anchor::Start => x,
            Anchor::Middle => x - w / 2.0,
            Anchor::End => x - w,
        };
        let mut pb = PathBuilder::new();
        for ch in text.chars() {
            let gid = self.glyph(ch);
            // FreeType puts each glyph at a full pixel of the screen.
            let (sx, sy) = (transform.sx, transform.sy);
            let gx = ((pen_x * sx + transform.tx).round() - transform.tx) / sx;
            let gy = ((y * sy + transform.ty).round() - transform.ty) / sy;
            let mut b = GlyphBuilder { pb: &mut pb, x: gx, y: gy, s };
            self.face.outline_glyph(gid, &mut b);
            pen_x += self.advance(gid, s);
        }
        if let Some(path) = pb.finish() {
            let mut paint = Paint::default();
            paint.set_color_rgba8(color[0], color[1], color[2], 255);
            paint.anti_alias = false;
            pixmap.fill_path(&path, &paint, FillRule::Winding, transform, None);
            return Some(path);
        }
        None
    }
}

struct GlyphBuilder<'a> {
    pb: &'a mut PathBuilder,
    x: f32,
    y: f32,
    s: f32,
}

impl GlyphBuilder<'_> {
    fn p(&self, x: f32, y: f32) -> (f32, f32) {
        (self.x + x * self.s, self.y - y * self.s)
    }
}

impl ttf_parser::OutlineBuilder for GlyphBuilder<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.p(x, y);
        self.pb.move_to(x, y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.p(x, y);
        self.pb.line_to(x, y);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (x1, y1) = self.p(x1, y1);
        let (x, y) = self.p(x, y);
        self.pb.quad_to(x1, y1, x, y);
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (x1, y1) = self.p(x1, y1);
        let (x2, y2) = self.p(x2, y2);
        let (x, y) = self.p(x, y);
        self.pb.cubic_to(x1, y1, x2, y2, x, y);
    }
    fn close(&mut self) {
        self.pb.close();
    }
}
