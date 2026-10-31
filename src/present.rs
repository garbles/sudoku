//! The output of the main loop: it draws the app into the frame, copies the changes into the
//! framebuffer and gives the rectangles to update. It has no input or output, thus the bench and the
//! device use the same code.

use std::time::{Duration, Instant};

use tiny_skia::Pixmap;

use crate::app::App;
use crate::font::Fonts;
use crate::layout::{screen_h, screen_w};
use crate::render::{self, Canvas};

/// The shortest time between two updates of the ink while the pen writes.
pub const INK_UPDATE_INTERVAL_MS: u64 = 30;

/// The time after the last ink update before the app sends the full ink area again.
pub const INK_REPAIR_DELAY_MS: u64 = 400;

/// A screen rectangle (x0, y0, x1, y1; all included).
pub type Rect = (i32, i32, i32, i32);

/// A physical rectangle (x, y, width, height).
pub type Update = (i32, i32, i32, i32);

/// The time of each phase of the last `present` (zero if the phase did not occur).
#[derive(Clone, Copy, Debug, Default)]
pub struct Times {
    /// The full drawing of the app.
    pub draw: Duration,
    /// The copy of the full frame into the framebuffer.
    pub blit: Duration,
    /// The drawing of the new segments of the live stroke.
    pub ink_draw: Duration,
    /// The copy of the ink area into the framebuffer.
    pub ink_blit: Duration,
}

pub struct Presenter {
    pub frame: Pixmap,
    /// The live stroke (`App::live_stroke_count`) and the quantity of its points that the frame shows.
    ink_drawn: (u64, usize),
    /// The area of ink that is drawn in the frame but not yet sent. AppLoad repaints the screen for
    /// each update, thus the app sends at most one ink update each INK_UPDATE_INTERVAL_MS.
    ink_box: Option<Rect>,
    last_ink_update: u64,
    /// The area of all recent ink updates. The panel can skip pixels of an update that overlaps an
    /// update that is still in progress. Thus, when the ink stops for INK_REPAIR_DELAY_MS, the app
    /// sends this area one more time, after the earlier updates are complete.
    repair_box: Option<Rect>,
    /// A copy of the frame as the framebuffer shows it (None: not known). A comparison of two RGBA
    /// rows is much faster than a comparison with the RGB framebuffer.
    shown: Option<Pixmap>,
    pub times: Times,
}

impl Default for Presenter {
    fn default() -> Self {
        Presenter::new()
    }
}

impl Presenter {
    pub fn new() -> Presenter {
        Presenter {
            frame: Pixmap::new(screen_w(), screen_h()).unwrap(),
            ink_drawn: (0, 0),
            ink_box: None,
            last_ink_update: 0,
            repair_box: None,
            shown: None,
            times: Times::default(),
        }
    }

    /// Starts again after a rotation: the frame has the new size.
    pub fn reset(&mut self) {
        self.frame = Pixmap::new(screen_w(), screen_h()).unwrap();
        self.ink_box = None;
        self.repair_box = None;
        self.shown = None;
    }

    /// The time in ms until the next ink timer (None if no ink waits).
    pub fn wait_ms(&self, now: u64) -> Option<u64> {
        let until = |d: u64| d.saturating_sub(now.saturating_sub(self.last_ink_update));
        let a = self.ink_box.map(|_| until(INK_UPDATE_INTERVAL_MS));
        let b = self.repair_box.map(|_| until(INK_REPAIR_DELAY_MS));
        a.into_iter().chain(b).min()
    }

    /// Draws the app, copies the changes into `fb` and calls `update` for each physical rectangle to
    /// send to the screen.
    pub fn present(&mut self, app: &mut App, fonts: &Fonts, fb: &mut [u8], now: u64, mut update: impl FnMut(Update)) {
        self.times = Times::default();
        if app.redraw {
            self.ink_box = None;
            let t = Instant::now();
            render::draw(&mut Canvas::new(&mut self.frame, fonts), app);
            self.times.draw = t.elapsed();
            app.redraw = false;
            self.ink_drawn = (app.live_stroke_count, app.live_stroke.as_ref().map_or(0, |s| s.len()));
            let t = Instant::now();
            let changed = self.blit(fb);
            self.times.blit = t.elapsed();
            if let Some(r) = changed {
                update(r);
            }
        } else if let Some((s, drawn)) = app
            .live_stroke
            .as_ref()
            // A new stroke (for example the second stroke of a 4) has no points in the frame yet.
            .map(|s| (s, if self.ink_drawn.0 == app.live_stroke_count { self.ink_drawn.1 } else { 0 }))
            .filter(|(s, drawn)| s.len() > (*drawn).max(1))
        {
            // Only the ink is new: draw the new segments and send only their area. A full drawing
            // is too slow to follow the pen.
            let t = Instant::now();
            let new = &s[drawn.max(1) - 1..];
            Canvas::new(&mut self.frame, fonts).polyline(new, render::INK_WIDTH, render::BLACK, true);
            self.times.ink_draw = t.elapsed();
            let (x0, y0, x1, y1) = new.iter().fold((f32::MAX, f32::MAX, f32::MIN, f32::MIN), |(a, b, c, d), &(x, y)| {
                (a.min(x), b.min(y), c.max(x), d.max(y))
            });
            let m = render::INK_WIDTH;
            let rect = (
                ((x0 - m).floor() as i32).max(0),
                ((y0 - m).floor() as i32).max(0),
                ((x1 + m).ceil() as i32).min(screen_w() as i32 - 1),
                ((y1 + m).ceil() as i32).min(screen_h() as i32 - 1),
            );
            self.ink_box = Some(union(self.ink_box, rect));
            self.ink_drawn = (app.live_stroke_count, s.len());
        }
        if let Some(b) = self.ink_box {
            if now.saturating_sub(self.last_ink_update) >= INK_UPDATE_INTERVAL_MS {
                let t = Instant::now();
                update(blit_rect(&self.frame, fb, b));
                if let Some(shown) = &mut self.shown {
                    let w = (b.2 - b.0 + 1) as usize;
                    for y in b.1 as usize..=b.3 as usize {
                        let o = (y * shown.width() as usize + b.0 as usize) * 4;
                        shown.data_mut()[o..o + w * 4].copy_from_slice(&self.frame.data()[o..o + w * 4]);
                    }
                }
                self.times.ink_blit = t.elapsed();
                self.ink_box = None;
                self.last_ink_update = now;
                self.repair_box = Some(union(self.repair_box, b));
            }
        }
        if let Some(r) = self.repair_box {
            if self.ink_box.is_none() && now.saturating_sub(self.last_ink_update) >= INK_REPAIR_DELAY_MS {
                update(crate::layout::physical_rect(r));
                self.repair_box = None;
            }
        }
    }
}

impl Presenter {
    /// Copies the changes of the frame into the framebuffer. Returns the changed physical rectangle.
    fn blit(&mut self, fb: &mut [u8]) -> Option<Update> {
        let frame = &self.frame;
        let Some(shown) = self.shown.as_mut().filter(|s| s.width() == frame.width() && s.height() == frame.height()) else {
            let r = blit(frame, fb);
            self.shown = Some(frame.clone());
            return r;
        };
        let w = frame.width() as usize;
        let (base, sx, sy) = crate::layout::physical_steps();
        let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0, 0);
        let (new, old) = (frame.data(), shown.data_mut());
        for y in 0..frame.height() as usize {
            let row = y * w * 4..(y + 1) * w * 4;
            let (a, b) = (&new[row.clone()], &mut old[row]);
            if same(a, b) {
                continue;
            }
            let first = a.chunks_exact(4).zip(b.chunks_exact(4)).position(|(p, q)| p != q).unwrap();
            let last = w - 1 - a.chunks_exact(4).rev().zip(b.chunks_exact(4).rev()).position(|(p, q)| p != q).unwrap();
            let mut i = base + y as i64 * sy + first as i64 * sx;
            for p in a[first * 4..(last + 1) * 4].chunks_exact(4) {
                fb[i as usize * 3..i as usize * 3 + 3].copy_from_slice(&p[..3]);
                i += sx;
            }
            b[first * 4..(last + 1) * 4].copy_from_slice(&a[first * 4..(last + 1) * 4]);
            x0 = x0.min(first);
            x1 = x1.max(last);
            y0 = y0.min(y);
            y1 = y1.max(y);
        }
        (x0 != usize::MAX).then(|| crate::layout::physical_rect((x0 as i32, y0 as i32, x1 as i32, y1 as i32)))
    }
}

/// True if the two slices have the same bytes. The comparison of 64-byte chunks as words is fast
/// (SIMD). The `memcmp` of musl (the C library of the tablet build) compares one byte at a time.
fn same(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let word = |c: &[u8], i: usize| u64::from_ne_bytes(c[i * 8..i * 8 + 8].try_into().unwrap());
    let (ca, cb) = (a.chunks_exact(64), b.chunks_exact(64));
    let (ra, rb) = (ca.remainder(), cb.remainder());
    ca.zip(cb).all(|(x, y)| (0..8).fold(0, |d, i| d | (word(x, i) ^ word(y, i))) == 0) && ra.iter().zip(rb).all(|(x, y)| x == y)
}

fn union(a: Option<Rect>, b: Rect) -> Rect {
    match a {
        None => b,
        Some(a) => (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)),
    }
}

/// Copies the changed area of the frame (screen coordinates) into the framebuffer (physical
/// coordinates, with the rotation of the tablet, 3 bytes for each pixel). Returns the changed
/// physical rectangle.
pub fn blit(frame: &Pixmap, fb: &mut [u8]) -> Option<Update> {
    let w = frame.width() as usize;
    let px = frame.data();
    let (base, sx, sy) = crate::layout::physical_steps();
    let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0, 0);
    if sx == 1 || sx == -1 {
        // A screen row is one framebuffer row (forward or reversed): convert the full row, and
        // compare it with one slice comparison.
        let mut rgb = vec![0u8; w * 3];
        for y in 0..frame.height() as usize {
            let src = &px[y * w * 4..(y + 1) * w * 4];
            let start = base + y as i64 * sy;
            // The first byte of the row in the framebuffer.
            let o = if sx == 1 { start } else { start - (w as i64 - 1) } as usize * 3;
            let dst = &mut fb[o..o + w * 3];
            if sx == 1 {
                for (d, s) in rgb.chunks_exact_mut(3).zip(src.chunks_exact(4)) {
                    d.copy_from_slice(&s[..3]);
                }
            } else {
                for (d, s) in rgb.chunks_exact_mut(3).rev().zip(src.chunks_exact(4)) {
                    d.copy_from_slice(&s[..3]);
                }
            }
            if same(dst, &rgb) {
                continue;
            }
            // The changed pixels of the row, in framebuffer order.
            let first = dst.chunks_exact(3).zip(rgb.chunks_exact(3)).position(|(a, b)| a != b).unwrap();
            let last = w - 1 - dst.chunks_exact(3).rev().zip(rgb.chunks_exact(3).rev()).position(|(a, b)| a != b).unwrap();
            dst[first * 3..=last * 3 + 2].copy_from_slice(&rgb[first * 3..=last * 3 + 2]);
            let (a, b) = if sx == 1 { (first, last) } else { (w - 1 - last, w - 1 - first) };
            x0 = x0.min(a);
            x1 = x1.max(b);
            y0 = y0.min(y);
            y1 = y1.max(y);
        }
    } else {
        for y in 0..frame.height() as usize {
            let row = &px[y * w * 4..(y + 1) * w * 4];
            let mut i = base + y as i64 * sy;
            for (x, p) in row.chunks_exact(4).enumerate() {
                let o = &mut fb[i as usize * 3..i as usize * 3 + 3];
                if *o != p[..3] {
                    o.copy_from_slice(&p[..3]);
                    x0 = x0.min(x);
                    x1 = x1.max(x);
                    y0 = y0.min(y);
                    y1 = y1.max(y);
                }
                i += sx;
            }
        }
    }
    (x0 != usize::MAX).then(|| crate::layout::physical_rect((x0 as i32, y0 as i32, x1 as i32, y1 as i32)))
}

/// Copies a screen rectangle of the frame into the framebuffer. Returns the physical rectangle.
pub fn blit_rect(frame: &Pixmap, fb: &mut [u8], r: Rect) -> Update {
    let w = frame.width() as usize;
    let px = frame.pixels();
    let (base, sx, sy) = crate::layout::physical_steps();
    for y in r.1 as usize..=r.3 as usize {
        for x in r.0 as usize..=r.2 as usize {
            let p = px[y * w + x];
            let i = (base + x as i64 * sx + y as i64 * sy) as usize * 3;
            fb[i..i + 3].copy_from_slice(&[p.red(), p.green(), p.blue()]);
        }
    }
    crate::layout::physical_rect(r)
}
