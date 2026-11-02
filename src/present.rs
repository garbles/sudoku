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

/// The time that the app gives an update to complete on the panel (the UI waveform). An ink update
/// that overlaps an update in progress waits until this time is over (refer to `Scheduler`).
pub const INK_WAVE_MS: u64 = 400;

/// The time that the QML patch of AppLoad needs to apply a new pen region (it reads the region each
/// 15 ms). Ink waits this time after the region starts, and a redraw after the region stops.
pub const PEN_REGION_SETTLE_MS: u64 = 45;

/// A screen rectangle (x0, y0, x1, y1; all included).
pub type Rect = (i32, i32, i32, i32);

/// A physical rectangle (x, y, width, height).
pub type Update = (i32, i32, i32, i32);

/// A physical rectangle (x0, y0, x1, y1; all included).
type PRect = (i32, i32, i32, i32);

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
    /// The time when new points of the live stroke were drawn.
    last_ink_draw: u64,
    /// The cells in progress on the panel, and the ink that is not sent yet.
    scheduler: Scheduler,
    /// True if the QML patch of AppLoad shows a pen region (refer to `pen_region`).
    pub use_pen_region: bool,
    /// The area (physical) where xochitl sends the updates with the fast pen waveform: the cell of
    /// the ink while the player writes. None: no area.
    pen_region: Option<PRect>,
    /// The time of the last change of `pen_region`, and true if the platform must send it.
    region_since: u64,
    region_changed: bool,
    /// A redraw that waits until the patch removed the pen region: else xochitl sends it with the pen
    /// waveform, which leaves ghosts on colored cells.
    deferred_blit: bool,
    /// True while the player writes: a redraw waits until the app reads the last ink.
    writing: bool,
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
            last_ink_draw: 0,
            scheduler: Scheduler::new(INK_WAVE_MS),
            use_pen_region: false,
            pen_region: None,
            region_since: 0,
            region_changed: false,
            deferred_blit: false,
            writing: false,
            shown: None,
            times: Times::default(),
        }
    }

    /// Starts again after a rotation: the frame has the new size.
    pub fn reset(&mut self) {
        self.frame = Pixmap::new(screen_w(), screen_h()).unwrap();
        self.ink_box = None;
        // The full window is drawn again with the new size.
        self.scheduler = Scheduler::new(self.scheduler.wave_ms);
        self.scheduler.fast = self.pen_region.map(pixel_cells);
        self.shown = None;
    }

    /// Sets the time that an update needs on the panel (default INK_WAVE_MS).
    pub fn set_wave_ms(&mut self, ms: u64) {
        self.scheduler.wave_ms = ms;
    }

    /// The time in ms until the next ink timer (None if no ink waits).
    pub fn wait_ms(&self, now: u64) -> Option<u64> {
        let settle = (self.region_since + PEN_REGION_SETTLE_MS).saturating_sub(now);
        let ink = (self.ink_box.is_some() || self.scheduler.has_dirty())
            .then(|| (self.last_ink_update + INK_UPDATE_INTERVAL_MS).saturating_sub(now).max(if self.pen_region.is_some() { settle } else { 0 }));
        // A redraw that waits for the end of the writing goes after the next input or timer of the app.
        let blit = (self.deferred_blit && !self.writing).then_some(settle);
        ink.into_iter().chain(blit).min()
    }

    /// The new pen region (physical x, y, width, height; None: no region), if it changed since the
    /// last call. The platform gives it to the QML patch of AppLoad.
    pub fn take_region_change(&mut self) -> Option<Option<Update>> {
        std::mem::take(&mut self.region_changed).then(|| self.pen_region.map(to_update))
    }

    /// The pen region for the ink of `app`: the inside of the cell of the pending ink, or of the cell
    /// where the pen touched the board. The region starts at the pen down: the app knows that the
    /// touch is writing only when the pen leaves the micro-cell, and the first ink must also get the
    /// pen waveform. xochitl refreshes the full region when it starts and when it stops, thus the
    /// region must not touch the cells around it. Its edges go inward to the 4 px grid of the panel
    /// (refer to `cells`).
    fn wanted_region(&self, app: &App) -> Option<PRect> {
        if !self.use_pen_region {
            return None;
        }
        let cell = app.ui.pending_ink.as_ref().map(|i| i.cell).or_else(|| {
            let &(x, y) = app.live_stroke.as_ref()?.first()?;
            crate::layout::board_cell(x, y)
        })?;
        let (x, y) = crate::layout::cell_origin(cell);
        let c = crate::layout::CELL;
        Some(inner_cells(to_prect(crate::layout::physical_rect((x, y, x + c - 1, y + c - 1)))))
    }

    /// Draws the app, copies the changes into `fb` and calls `update` for each physical rectangle to
    /// send to the screen.
    pub fn present(&mut self, app: &mut App, fonts: &Fonts, fb: &mut [u8], now: u64, mut update: impl FnMut(Update)) {
        self.times = Times::default();
        let wanted = self.wanted_region(app);
        if wanted != self.pen_region {
            self.pen_region = wanted;
            self.region_since = now;
            self.region_changed = true;
            self.scheduler.fast = wanted.map(pixel_cells);
        }
        let settled = now.saturating_sub(self.region_since) >= PEN_REGION_SETTLE_MS;
        // While the player writes, a redraw waits: the player can write in many cells, one after the
        // other, and only the ink changes on the screen. An update in UI mode over the next cell
        // would stop the ink there. The redraw goes when the app has read the last ink.
        let writing = self.use_pen_region && (app.live_stroke.is_some() || app.ui.pending_ink.is_some());
        self.writing = writing;
        if app.redraw {
            // The new drawing contains the ink of the stroke. If the redraw waits, send this ink now.
            self.ink_box = app
                .live_stroke
                .as_ref()
                .filter(|_| writing)
                .and_then(|s| s.iter().fold(None, |b, &(x, y)| Some(union(b, (x as i32, y as i32, x as i32, y as i32)))))
                .map(|b| {
                    let m = render::INK_WIDTH.ceil() as i32 + 1;
                    let (sw, sh) = (screen_w() as i32, screen_h() as i32);
                    ((b.0 - m).max(0), (b.1 - m).max(0), (b.2 + m).min(sw - 1), (b.3 + m).min(sh - 1))
                });
            let t = Instant::now();
            render::draw(&mut Canvas::new(&mut self.frame, fonts), app);
            self.times.draw = t.elapsed();
            app.redraw = false;
            self.ink_drawn = (app.live_stroke_count, app.live_stroke.as_ref().map_or(0, |s| s.len()));
            self.deferred_blit = true;
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
            self.last_ink_draw = now;
        }
        // A redraw goes when the player stopped writing and the patch has applied the last change of
        // the pen region.
        if self.deferred_blit && !writing && (settled || !self.use_pen_region) {
            self.deferred_blit = false;
            let t = Instant::now();
            let mut spans = Vec::new();
            let changed = self.blit(fb, &mut spans);
            self.times.blit = t.elapsed();
            if let Some(r) = changed {
                // A change of the screen does not wait. The ink in progress that it stops is sent
                // again later.
                self.scheduler.send_now(to_prect(r), &spans, now, &mut update);
            }
        }
        // Ink in a new pen region waits until the patch has applied it.
        let region_ready = self.pen_region.is_none() || settled;
        let due = now.saturating_sub(self.last_ink_update) >= INK_UPDATE_INTERVAL_MS && region_ready && (writing || !self.deferred_blit);
        if due && (self.ink_box.is_some() || self.scheduler.has_dirty()) {
            if let Some(b) = self.ink_box.take() {
                let t = Instant::now();
                blit_rect(&self.frame, fb, b);
                self.ink_changed(b);
                self.times.ink_blit = t.elapsed();
            }
            // While the pen moves, the next segment starts at the pen tip. Thus the cells at the tip
            // wait one tick, and go with the next segment: else the next update would contain cells
            // that this update changes.
            let hold = app
                .live_stroke
                .as_ref()
                .and_then(|s| s.last())
                .filter(|_| now.saturating_sub(self.last_ink_draw) <= 2 * INK_UPDATE_INTERVAL_MS)
                // In the pen region, an overlap does not cause gaps: the tip need not wait.
                .filter(|_| self.pen_region.is_none())
                .map(|&(x, y)| {
                    let m = render::INK_WIDTH.ceil() as i32 + 2;
                    let (x, y) = (x.round() as i32, y.round() as i32);
                    let r = (x - m, y - m, x + m, y + m);
                    let (sw, sh) = (screen_w() as i32, screen_h() as i32);
                    let r = (r.0.max(0), r.1.max(0), r.2.min(sw - 1), r.3.min(sh - 1));
                    pixel_cells(to_prect(crate::layout::physical_rect(r)))
                });
            if self.scheduler.send_ink(now, hold, &mut update) {
                self.last_ink_update = now;
            }
        }
    }

    /// Records the pixels of the screen rectangle `b` that differ from the screen as it is shown, and
    /// updates `shown`.
    fn ink_changed(&mut self, b: Rect) {
        let Some(shown) = &mut self.shown else {
            self.scheduler.changed(to_prect(crate::layout::physical_rect(b)));
            return;
        };
        let (fw, w) = (shown.width() as usize, (b.2 - b.0 + 1) as usize);
        for y in b.1 as usize..=b.3 as usize {
            let o = (y * fw + b.0 as usize) * 4;
            let (new, old) = (&self.frame.data()[o..o + w * 4], &mut shown.data_mut()[o..o + w * 4]);
            if same(new, old) {
                continue;
            }
            let first = new.chunks_exact(4).zip(old.chunks_exact(4)).position(|(p, q)| p != q).unwrap();
            let last = w - 1 - new.chunks_exact(4).rev().zip(old.chunks_exact(4).rev()).position(|(p, q)| p != q).unwrap();
            old.copy_from_slice(new);
            let (x0, x1) = (b.0 + first as i32, b.0 + last as i32);
            self.scheduler.changed(to_prect(crate::layout::physical_rect((x0, y as i32, x1, y as i32))));
        }
    }
}

fn to_prect((x, y, w, h): Update) -> PRect {
    (x, y, x + w - 1, y + h - 1)
}

fn to_update((x0, y0, x1, y1): PRect) -> Update {
    (x0, y0, x1 - x0 + 1, y1 - y0 + 1)
}

/// The 4x4 px cells of the panel. The display controller of xochitl aligns each update to 4 px (x)
/// and 2 px (y), and AppLoad adds 1 px at the right and at the bottom. Cells of 4 px in the two
/// directions agree also with a rotated window. A cell rectangle is (cx0, cy0, cx1, cy1; all
/// included).
type Cells = (i32, i32, i32, i32);

/// The cells that an update of the physical rectangle `r` touches (with the extra pixel of AppLoad).
fn cells(r: PRect) -> Cells {
    (r.0 >> 2, r.1 >> 2, (r.2 + 1) >> 2, (r.3 + 1) >> 2)
}

/// The cells that contain the pixels of `r`.
fn pixel_cells(r: PRect) -> Cells {
    (r.0 >> 2, r.1 >> 2, r.2 >> 2, r.3 >> 2)
}

/// The largest physical rectangle in `r` whose edges are on the 4 px grid of the panel (whole
/// cells only).
fn inner_cells(r: PRect) -> PRect {
    ((r.0 + 3) & !3, (r.1 + 3) & !3, ((r.2 + 1) & !3) - 1, ((r.3 + 1) & !3) - 1)
}

/// The physical rectangle to send for the cells `c`: its update touches these cells and no other
/// cells. The last pixel column and row of `c` come from the extra pixel of AppLoad.
fn cell_rect(c: Cells) -> PRect {
    let (w, h) = (crate::layout::PHYSICAL_W as i32, crate::layout::PHYSICAL_H as i32);
    (c.0 * 4, c.1 * 4, (c.2 * 4 + 2).min(w - 1), (c.3 * 4 + 2).min(h - 1))
}

fn inside(c: Cells, x: i32, y: i32) -> bool {
    c.0 <= x && x <= c.2 && c.1 <= y && y <= c.3
}

/// Sends the ink so that an update never contains a cell that an update in progress changes.
///
/// The display controller of xochitl stops an older update where a new update overlaps it. A pixel
/// that the older update changed, but did not complete, can then stay gray or white: the ink had
/// gaps (DECISIONS.md). Qt merges all the updates of one frame into one rectangle, thus the app
/// sends one rectangle for each ink tick. The scheduler keeps, for each cell, the time when its
/// change is complete, and the cells that changed but are not sent yet.
#[derive(Clone, Debug)]
struct Scheduler {
    /// The time that an update needs on the panel.
    wave_ms: u64,
    /// The size of the grid in cells.
    w: i32,
    h: i32,
    /// For each cell: the time when its change is complete (0: no change).
    busy: Vec<u64>,
    /// For each cell: the framebuffer has a change that is not sent yet.
    dirty: Vec<bool>,
    /// The cells that contain all the dirty cells (None: no dirty cell).
    dirty_box: Option<Cells>,
    /// The cells of the pen region: xochitl sends them with the fast pen waveform, which does not
    /// leave gaps. Thus their changes need no time to complete.
    fast: Option<Cells>,
}

impl Scheduler {
    fn new(wave_ms: u64) -> Scheduler {
        let w = (crate::layout::PHYSICAL_W as i32 + 3) / 4;
        let h = (crate::layout::PHYSICAL_H as i32 + 3) / 4;
        let n = (w * h) as usize;
        Scheduler { wave_ms, w, h, busy: vec![0; n], dirty: vec![false; n], dirty_box: None, fast: None }
    }

    fn clip(&self, c: Cells) -> Option<Cells> {
        let c = (c.0.max(0), c.1.max(0), c.2.min(self.w - 1), c.3.min(self.h - 1));
        (c.0 <= c.2 && c.1 <= c.3).then_some(c)
    }

    /// The time when a change of the cell (x, y) sent at `now` is complete.
    fn until(&self, x: i32, y: i32, now: u64) -> u64 {
        if self.fast.is_some_and(|f| inside(f, x, y)) { now } else { now + self.wave_ms }
    }

    fn at(&self, x: i32, y: i32) -> usize {
        (y * self.w + x) as usize
    }

    /// True if new ink in the cell (x, y) must wait for an update in progress. Ink in the pen
    /// region does not wait: it can stop an older update there, because xochitl refreshes all the
    /// region with the UI waveform when the region stops.
    fn waits(&self, x: i32, y: i32, now: u64) -> bool {
        self.busy[self.at(x, y)] > now && !self.fast.is_some_and(|f| inside(f, x, y))
    }

    /// Records changed pixels of the framebuffer that are not sent yet.
    fn changed(&mut self, r: PRect) {
        let Some(c) = self.clip(pixel_cells(r)) else { return };
        for y in c.1..=c.3 {
            for x in c.0..=c.2 {
                let i = self.at(x, y);
                self.dirty[i] = true;
            }
        }
        self.dirty_box = Some(union(self.dirty_box, c));
    }

    fn has_dirty(&self) -> bool {
        self.dirty_box.is_some()
    }

    fn shrink_dirty_box(&mut self) {
        let Some(d) = self.dirty_box else { return };
        let mut b: Option<Cells> = None;
        for y in d.1..=d.3 {
            for x in d.0..=d.2 {
                if self.dirty[self.at(x, y)] {
                    b = Some(union(b, (x, y, x, y)));
                }
            }
        }
        self.dirty_box = b;
    }

    /// Sends the update `r` at once (a change of the screen). `spans` are the changed pixels. The
    /// ink in progress that `r` stops is sent again later.
    fn send_now(&mut self, r: PRect, spans: &[PRect], now: u64, update: &mut impl FnMut(Update)) {
        if let Some(t) = self.clip(cells(r)) {
            for y in t.1..=t.3 {
                for x in t.0..=t.2 {
                    let i = self.at(x, y);
                    if self.busy[i] > now {
                        // An update in progress stops here: send this cell again.
                        self.dirty[i] = true;
                        self.dirty_box = Some(union(self.dirty_box, (x, y, x, y)));
                    } else if self.dirty[i] {
                        // Ink that waited goes with this update.
                        self.dirty[i] = false;
                        self.busy[i] = self.until(x, y, now);
                    }
                }
            }
        }
        update(to_update(r));
        for &s in spans {
            let Some(c) = self.clip(pixel_cells(s)) else { continue };
            for y in c.1..=c.3 {
                for x in c.0..=c.2 {
                    let i = self.at(x, y);
                    self.busy[i] = self.busy[i].max(self.until(x, y, now));
                }
            }
        }
        self.shrink_dirty_box();
    }

    /// Selects one rectangle of dirty cells that contains no cell in progress and no cell of `hold`
    /// (the pen tip), and sends it. Returns false if no cell can go now.
    fn send_ink(&mut self, now: u64, hold: Option<Cells>, update: &mut impl FnMut(Update)) -> bool {
        let Some(d) = self.dirty_box else { return false };
        let held = |x: i32, y: i32| hold.is_some_and(|h| inside(h, x, y));
        let free = |s: &Scheduler, x: i32, y: i32| {
            s.dirty[s.at(x, y)] && !s.waits(x, y, now) && !held(x, y)
        };
        let blocked = |s: &Scheduler, x: i32, y: i32| s.waits(x, y, now) || held(x, y);
        // The box of the free cells in `c`, and their number.
        let free_box = |s: &Scheduler, c: Cells| {
            let (mut b, mut n): (Option<Cells>, usize) = (None, 0);
            for y in c.1..=c.3 {
                for x in c.0..=c.2 {
                    if free(s, x, y) {
                        b = Some(union(b, (x, y, x, y)));
                        n += 1;
                    }
                }
            }
            b.map(|b| (b, n))
        };
        let Some((mut r, _)) = free_box(self, d) else { return false };
        // Make the rectangle smaller until it contains no blocked cell. Each step removes all the
        // blocked cells at one side, at the side that keeps the most free cells.
        for _ in 0..16 {
            let mut bb: Option<Cells> = None;
            for y in r.1..=r.3 {
                for x in r.0..=r.2 {
                    if blocked(self, x, y) {
                        bb = Some(union(bb, (x, y, x, y)));
                    }
                }
            }
            let Some(b) = bb else { break };
            let options = [(b.2 + 1, r.1, r.2, r.3), (r.0, r.1, b.0 - 1, r.3), (r.0, b.3 + 1, r.2, r.3), (r.0, r.1, r.2, b.1 - 1)];
            let best = options
                .iter()
                .filter(|o| o.0 <= o.2 && o.1 <= o.3)
                .filter_map(|&o| free_box(self, o))
                .max_by_key(|&(_, n)| n);
            match best {
                Some((nr, _)) => r = nr,
                None => {
                    // The free cells are all between blocked cells: send the first free cell.
                    let mut one = None;
                    'find: for y in r.1..=r.3 {
                        for x in r.0..=r.2 {
                            if free(self, x, y) && !blocked(self, x, y) {
                                one = Some((x, y, x, y));
                                break 'find;
                            }
                        }
                    }
                    match one {
                        Some(o) => r = o,
                        None => return false,
                    }
                }
            }
        }
        if (r.1..=r.3).any(|y| (r.0..=r.2).any(|x| blocked(self, x, y))) {
            return false;
        }
        update(to_update(cell_rect(r)));
        for y in r.1..=r.3 {
            for x in r.0..=r.2 {
                let i = self.at(x, y);
                if self.dirty[i] {
                    self.dirty[i] = false;
                    self.busy[i] = self.until(x, y, now);
                }
            }
        }
        self.shrink_dirty_box();
        true
    }
}

impl Presenter {
    /// Copies the changes of the frame into the framebuffer. Returns the changed physical rectangle,
    /// and adds the changed pixels of each row to `spans` (physical rectangles).
    fn blit(&mut self, fb: &mut [u8], spans: &mut Vec<PRect>) -> Option<Update> {
        let frame = &self.frame;
        let Some(shown) = self.shown.as_mut().filter(|s| s.width() == frame.width() && s.height() == frame.height()) else {
            let r = blit(frame, fb);
            self.shown = Some(frame.clone());
            spans.extend(r.map(to_prect));
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
            spans.push(to_prect(crate::layout::physical_rect((first as i32, y as i32, last as i32, y as i32))));
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};

    /// A small deterministic generator (xorshift).
    struct Rng(u64);
    impl Rng {
        fn next(&mut self, n: i32) -> i32 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            (self.0 % n as u64) as i32
        }
    }

    fn cell_set(c: Cells) -> impl Iterator<Item = (i32, i32)> {
        (c.1..=c.3).flat_map(move |y| (c.0..=c.2).map(move |x| (x, y)))
    }

    /// Calls `send_ink` each 30 ms until no ink waits. Returns the updates and the time.
    fn drain(s: &mut Scheduler, mut now: u64) -> (Vec<Update>, u64) {
        let mut sent = Vec::new();
        for _ in 0..200 {
            if !s.has_dirty() {
                return (sent, now);
            }
            now += 30;
            s.send_ink(now, None, &mut |u| sent.push(u));
        }
        panic!("the ink does not end: {:?}", s.dirty_box);
    }

    #[test]
    fn the_rectangle_of_cells_touches_these_cells_only() {
        let (w, h) = (crate::layout::PHYSICAL_W as i32, crate::layout::PHYSICAL_H as i32);
        let mut rng = Rng(7);
        for _ in 0..1000 {
            let (x0, y0) = (rng.next(w / 4), rng.next(h / 4));
            let c = (x0, y0, x0 + rng.next(w / 4 - x0), y0 + rng.next(h / 4 - y0));
            assert_eq!(cells(cell_rect(c)), c);
        }
        // An update touches the cells of the extra pixel of AppLoad too.
        assert_eq!(cells((0, 0, 2, 2)), (0, 0, 0, 0));
        assert_eq!(cells((0, 0, 3, 3)), (0, 0, 1, 1));
    }

    #[test]
    fn ink_over_cells_in_progress_waits_until_they_are_complete() {
        let mut s = Scheduler::new(400);
        let mut sent = Vec::new();
        s.changed((10, 10, 40, 40));
        assert!(s.send_ink(0, None, &mut |u| sent.push(u)));
        assert_eq!(sent, vec![(8, 8, 35, 35)]);
        sent.clear();
        s.changed((30, 30, 70, 70));
        assert!(s.send_ink(30, None, &mut |u| sent.push(u)));
        // One rectangle, outside the cells of the first update.
        assert_eq!(sent.len(), 1);
        let c = cells(to_prect(sent[0]));
        assert!(cell_set(c).all(|(x, y)| !(x <= 10 && y <= 10)), "{c:?}");
        // The joint goes when the first update is complete.
        let (late, end) = drain(&mut s, 30);
        assert!(end >= 400 && end <= 460, "{end}");
        assert!(late.iter().any(|&u| inside(cells(to_prect(u)), 9, 9)), "{late:?}");
    }

    #[test]
    fn the_inner_cells_are_on_the_grid_and_inside_the_rectangle() {
        assert_eq!(inner_cells((148, 418, 294, 564)), (148, 420, 291, 563));
        let mut rng = Rng(3);
        for _ in 0..1000 {
            let (x0, y0) = (rng.next(1000), rng.next(1000));
            let r = (x0, y0, x0 + 8 + rng.next(200), y0 + 8 + rng.next(200));
            let i = inner_cells(r);
            assert!(i.0 >= r.0 && i.1 >= r.1 && i.2 <= r.2 && i.3 <= r.3, "{r:?} {i:?}");
            assert!(i.0 % 4 == 0 && i.1 % 4 == 0 && (i.2 + 1) % 4 == 0 && (i.3 + 1) % 4 == 0, "{i:?}");
        }
    }

    #[test]
    fn ink_in_the_pen_region_does_not_wait() {
        let mut s = Scheduler::new(400);
        s.fast = Some(pixel_cells((0, 0, 99, 99)));
        let mut sent = Vec::new();
        s.changed((10, 10, 40, 40));
        s.send_ink(0, None, &mut |u| sent.push(u));
        s.changed((30, 30, 70, 70));
        s.send_ink(30, None, &mut |u| sent.push(u));
        // The second update goes complete at once, over the first one.
        assert_eq!(sent.len(), 2);
        assert!(!s.has_dirty());
        assert_eq!(cells(to_prect(sent[1])), pixel_cells((30, 30, 70, 70)));
    }

    /// A redraw in UI mode covers the next cell, and the player writes in that cell at once: the
    /// ink in the pen region does not wait until the redraw is complete.
    #[test]
    fn ink_in_the_pen_region_does_not_wait_for_a_redraw() {
        let mut s = Scheduler::new(400);
        let mut sent = Vec::new();
        s.send_now((0, 0, 199, 99), &[(0, 0, 199, 99)], 0, &mut |u| sent.push(u));
        sent.clear();
        s.fast = Some(pixel_cells((100, 0, 199, 99)));
        s.changed((130, 30, 170, 70));
        assert!(s.send_ink(30, None, &mut |u| sent.push(u)));
        assert_eq!(cells(to_prect(sent[0])), pixel_cells((130, 30, 170, 70)));
        // Outside the pen region, the ink waits.
        s.changed((30, 30, 70, 70));
        assert!(!s.send_ink(60, None, &mut |u| sent.push(u)));
        assert!(s.send_ink(400, None, &mut |u| sent.push(u)));
    }

    /// The player writes in one cell and at once in a second cell (pen region on). The ink of the
    /// second cell goes while the pen moves, and the redraw of the first cell waits until the app
    /// has read the last ink.
    #[test]
    fn a_redraw_waits_while_the_player_writes_in_many_cells() {
        use crate::app::{new_game, Input, Tool};
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let fonts = Fonts::from_files(&root.join("scripts/fonts/rmsans-400.ttf"), &root.join("scripts/fonts/rmsans-500.ttf")).unwrap();
        let mut app = App::new(Some(new_game(7, 0)), None, crate::store::ResultsDb::in_memory(), Box::new(crate::script::FixedEnv(7)));
        let mut p = Presenter::new();
        p.use_pen_region = true;
        let mut fb = vec![0u8; (crate::layout::PHYSICAL_W * crate::layout::PHYSICAL_H * 3) as usize];
        let empty: Vec<_> = (0..9u8).flat_map(|r| (0..9u8).map(move |c| (r, c))).filter(|&c| !app.game.is_filled(c)).take(2).collect();
        let (a, b) = (empty[0], empty[1]);
        let mut now = 1000;
        app.tick(now);
        let step = |app: &mut App, p: &mut Presenter, fb: &mut Vec<u8>, now: u64, input: Option<Input>| {
            if let Some(i) = input {
                app.handle(i, now);
            }
            app.tick(now);
            let mut sent = Vec::new();
            p.present(app, &fonts, fb, now, |u| sent.push(u));
            // While the pen writes, the pen region is on from the pen down, and all the ink is in it
            // (the first ink of a stroke also gets the pen waveform).
            if app.live_stroke.is_some() {
                let r = p.pen_region.expect("the pen region is on while the pen writes");
                for &u in &sent {
                    let u = to_prect(u);
                    assert!(u.0 >= r.0 && u.1 >= r.1 && u.2 <= r.2 + 1 && u.3 <= r.3 + 1, "{u:?} is not in {r:?}");
                }
            }
            sent
        };
        step(&mut app, &mut p, &mut fb, now, None);
        // A vertical stroke in a cell, one point each 10 ms.
        let stroke = |cell: crate::game::Cell| {
            let (x, y) = crate::layout::cell_origin(cell);
            let c = crate::layout::CELL as f32;
            (0..=20).map(move |k| (x as f32 + c / 2.0, y as f32 + c * (0.2 + 0.6 * k as f32 / 20.0)))
        };
        let mut log: Vec<(u64, Update)> = Vec::new();
        for cell in [a, b] {
            let mut pts = stroke(cell);
            let (x, y) = pts.next().unwrap();
            now += 10;
            log.extend(step(&mut app, &mut p, &mut fb, now, Some(Input::PenDown { x, y, tool: Tool::Pen })).into_iter().map(|u| (now, u)));
            let mut last = (x, y);
            for (x, y) in pts {
                now += 10;
                log.extend(step(&mut app, &mut p, &mut fb, now, Some(Input::PenMove { x, y })).into_iter().map(|u| (now, u)));
                last = (x, y);
            }
            now += 10;
            log.extend(step(&mut app, &mut p, &mut fb, now, Some(Input::PenUp { x: last.0, y: last.1 })).into_iter().map(|u| (now, u)));
            // The writing in the first cell is done: the redraw of its digit must not go yet.
            if cell == a {
                assert!(app.ui.pending_ink.is_some());
            }
        }
        // All the updates up to now are ink, inside one of the two cells.
        let inside_cell = |u: Update, cell| {
            let (x, y) = crate::layout::cell_origin(cell);
            let c = crate::layout::CELL;
            let r = to_prect(crate::layout::physical_rect((x, y, x + c - 1, y + c - 1)));
            let u = to_prect(u);
            u.0 >= r.0 - 4 && u.1 >= r.1 - 4 && u.2 <= r.2 + 4 && u.3 <= r.3 + 4
        };
        for &(t, u) in &log {
            assert!(inside_cell(u, a) || inside_cell(u, b), "{t}: {u:?} is not ink; cells {a:?} {b:?}: {:?} {:?}", crate::layout::cell_origin(a), crate::layout::cell_origin(b));
        }
        assert!(log.iter().any(|&(_, u)| inside_cell(u, b)), "no ink in the second cell: {log:?}");
        // After the recognition pause, the redraw goes and the framebuffer shows the frame.
        let mut redraw = false;
        for _ in 0..200 {
            now += 30;
            let sent = step(&mut app, &mut p, &mut fb, now, None);
            redraw |= !sent.is_empty();
        }
        assert!(app.ui.pending_ink.is_none());
        assert!(redraw);
        let mut expect = vec![0u8; fb.len()];
        blit(&p.frame, &mut expect);
        assert!(fb == expect, "the framebuffer does not show the frame");
    }

    #[test]
    fn the_pen_tip_waits_while_the_pen_moves() {
        let mut s = Scheduler::new(400);
        let mut sent = Vec::new();
        s.changed((100, 100, 140, 140));
        let tip = pixel_cells((128, 128, 140, 140));
        assert!(s.send_ink(0, Some(tip), &mut |u| sent.push(u)));
        for &u in &sent {
            let c = cells(to_prect(u));
            assert!(cell_set(c).all(|(x, y)| !inside(tip, x, y)), "{c:?}");
        }
        // When the pen stops, the tip goes.
        let (late, _) = drain(&mut s, 0);
        assert!(late.iter().any(|&u| inside(cells(to_prect(u)), 34, 34)), "{late:?}");
    }

    #[test]
    fn a_change_of_the_screen_does_not_wait_and_the_stopped_ink_is_sent_again() {
        let mut s = Scheduler::new(400);
        let mut sent = Vec::new();
        s.changed((10, 10, 40, 40));
        s.send_ink(0, None, &mut |u| sent.push(u));
        sent.clear();
        s.send_now((0, 0, 99, 19), &[(0, 0, 99, 19)], 10, &mut |u| sent.push(u));
        assert_eq!(sent, vec![(0, 0, 100, 20)]);
        assert!(s.has_dirty());
        let (late, end) = drain(&mut s, 10);
        assert!(end >= 410, "{end}");
        assert!(late.iter().any(|&u| inside(cells(to_prect(u)), 3, 3)), "{late:?}");
    }

    /// Random strokes: each tick sends at most one rectangle; it never touches a cell that an update
    /// in progress changes or a cell of the pen tip; and all the ink is sent at the end.
    #[test]
    fn ink_never_touches_cells_in_progress_and_all_ink_is_sent() {
        let mut rng = Rng(0x9e3779b97f4a7c15);
        for _ in 0..200 {
            let wave = 100 + rng.next(400) as u64;
            let mut s = Scheduler::new(wave);
            // The model of the test: the cells not sent yet, and the time when each cell is complete.
            let mut dirty: HashSet<(i32, i32)> = HashSet::new();
            let mut busy: HashMap<(i32, i32), u64> = HashMap::new();
            let tick = |s: &mut Scheduler, now: u64, hold: Option<Cells>, dirty: &mut HashSet<(i32, i32)>, busy: &mut HashMap<(i32, i32), u64>| {
                let mut sent = Vec::new();
                s.send_ink(now, hold, &mut |u| sent.push(u));
                assert!(sent.len() <= 1);
                for u in sent {
                    let c = cells(to_prect(u));
                    for p in cell_set(c) {
                        assert!(busy.get(&p).copied().unwrap_or(0) <= now, "{p:?} in progress");
                        assert!(!hold.is_some_and(|h| inside(h, p.0, p.1)), "{p:?} at the tip");
                        if dirty.remove(&p) {
                            busy.insert(p, now + wave);
                        }
                    }
                }
            };
            let (mut x, mut y) = (20 + rng.next(80), 20 + rng.next(80));
            let mut now = 0;
            for _ in 0..30 {
                now += 30;
                let (nx, ny) = ((x + rng.next(25) - 12).clamp(5, 115), (y + rng.next(25) - 12).clamp(5, 115));
                for k in 0..=4 {
                    let (px, py) = (x + (nx - x) * k / 4, y + (ny - y) * k / 4);
                    let r = (px - 3, py - 3, px + 3, py + 3);
                    s.changed(r);
                    dirty.extend(cell_set(pixel_cells(r)));
                }
                (x, y) = (nx, ny);
                let hold = Some(pixel_cells((x - 7, y - 7, x + 7, y + 7)));
                tick(&mut s, now, hold, &mut dirty, &mut busy);
            }
            for _ in 0..200 {
                if !s.has_dirty() {
                    break;
                }
                now += 30;
                tick(&mut s, now, None, &mut dirty, &mut busy);
            }
            assert!(!s.has_dirty() && dirty.is_empty(), "{dirty:?}");
        }
    }
}
