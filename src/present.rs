//! The output of the main loop: it draws the app into the frame, copies the changes into the
//! framebuffer and gives the rectangles to update. It has no input or output, thus the bench and the
//! device use the same code.

use std::marker::PhantomData;
use std::time::{Duration, Instant};

use tiny_skia::Pixmap;

use crate::app::App;
use crate::font::Fonts;
use crate::layout::{screen_h, screen_w};
use crate::render::{self, Canvas, Damage, FrameRecord};

/// The shortest time between two updates of the ink while the pen writes.
const INK_UPDATE_INTERVAL_MS: u64 = 30;

/// The time that the app gives an update to complete on the panel (the UI waveform). An ink update
/// that overlaps an update in progress waits until this time is over (refer to `Scheduler`).
pub const INK_WAVE_MS: u64 = 400;

/// The time that the QML patch of AppLoad needs to apply a new pen region (it reads the region each
/// 15 ms). Ink waits this time after the region starts, and a redraw after the region stops.
const PEN_REGION_SETTLE_MS: u64 = 45;

/// A physical rectangle to send to the screen (x, y, width, height). The platform gives it to qtfb.
pub type Update = (i32, i32, i32, i32);

/// A rectangle with its corners (x0, y0, x1, y1; all included). `U` is the unit of the coordinates:
/// `Screen` pixels, `Physical` pixels or 4 px cells of the panel grid (`PanelGrid`). Thus the compiler finds a mix of the units.
#[derive(Debug, PartialEq, Eq)]
struct Corners<U> {
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    unit: PhantomData<U>,
}

/// The pixels of the screen as the player sees it (with the rotation of the window).
#[derive(Debug, PartialEq, Eq)]
enum Screen {}
/// The pixels of the framebuffer (always portrait).
#[derive(Debug, PartialEq, Eq)]
enum Physical {}
/// The 4x4 px cells of the panel (refer to `PanelCells`).
#[derive(Debug, PartialEq, Eq)]
enum PanelGrid {}

type ScreenRect = Corners<Screen>;
type PhysicalRect = Corners<Physical>;

/// The 4x4 px cells of the panel. The display controller of xochitl aligns each update to 4 px (x)
/// and 2 px (y), and AppLoad adds 1 px at the right and at the bottom. Cells of 4 px in the two
/// directions agree also with a rotated window.
type PanelCells = Corners<PanelGrid>;

// A derive would copy only when `U` is Copy.
impl<U> Clone for Corners<U> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<U> Copy for Corners<U> {}

impl<U> Corners<U> {
    const fn new(x0: i32, y0: i32, x1: i32, y1: i32) -> Self {
        Corners { x0, y0, x1, y1, unit: PhantomData }
    }

    fn point(x: i32, y: i32) -> Self {
        Self::new(x, y, x, y)
    }

    /// The rectangle with `m` more at each side.
    fn expand(self, m: i32) -> Self {
        Self::new(self.x0 - m, self.y0 - m, self.x1 + m, self.y1 + m)
    }

    /// The rectangle, or `None` if it has no width or no height.
    fn non_empty(self) -> Option<Self> {
        (self.x0 <= self.x1 && self.y0 <= self.y1).then_some(self)
    }

    fn contains(&self, x: i32, y: i32) -> bool {
        self.x0 <= x && x <= self.x1 && self.y0 <= y && y <= self.y1
    }
}

impl ScreenRect {
    /// The part of the rectangle that is on the screen.
    fn clip_to_screen(self) -> ScreenRect {
        let (w, h) = (screen_w() as i32, screen_h() as i32);
        ScreenRect::new(self.x0.max(0), self.y0.max(0), self.x1.min(w - 1), self.y1.min(h - 1))
    }

    /// The same pixels in the framebuffer.
    fn physical(self) -> PhysicalRect {
        PhysicalRect::from_update(crate::layout::physical_rect((self.x0, self.y0, self.x1, self.y1)))
    }
}

impl PhysicalRect {
    fn from_update((x, y, w, h): Update) -> PhysicalRect {
        PhysicalRect::new(x, y, x + w - 1, y + h - 1)
    }

    fn to_update(self) -> Update {
        (self.x0, self.y0, self.x1 - self.x0 + 1, self.y1 - self.y0 + 1)
    }

    /// The cells that an update of the rectangle touches (with the extra pixel of AppLoad).
    fn touched_cells(self) -> PanelCells {
        PanelCells::new(self.x0 >> 2, self.y0 >> 2, (self.x1 + 1) >> 2, (self.y1 + 1) >> 2)
    }

    /// The cells that contain the pixels of the rectangle.
    fn pixel_cells(self) -> PanelCells {
        PanelCells::new(self.x0 >> 2, self.y0 >> 2, self.x1 >> 2, self.y1 >> 2)
    }

    /// The largest rectangle inside this one whose edges are on the 4 px grid of the panel (whole
    /// cells only).
    fn inner_cells(self) -> PhysicalRect {
        PhysicalRect::new((self.x0 + 3) & !3, (self.y0 + 3) & !3, ((self.x1 + 1) & !3) - 1, ((self.y1 + 1) & !3) - 1)
    }
}

impl PanelCells {
    /// The physical rectangle to send for the cells: its update touches these cells and no other
    /// cells. The last pixel column and row of the cells come from the extra pixel of AppLoad.
    fn rect(self) -> PhysicalRect {
        let (w, h) = (crate::layout::PHYSICAL_W as i32, crate::layout::PHYSICAL_H as i32);
        PhysicalRect::new(self.x0 * 4, self.y0 * 4, (self.x1 * 4 + 2).min(w - 1), (self.y1 * 4 + 2).min(h - 1))
    }

    /// The (x, y) of each cell, row by row and then from left to right.
    fn each(self) -> impl Iterator<Item = (i32, i32)> {
        (self.y0..=self.y1).flat_map(move |y| (self.x0..=self.x1).map(move |x| (x, y)))
    }
}

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
    frame: Pixmap,
    /// What the frame contains (refer to `FrameRecord`): all drawings on the frame use it.
    record: FrameRecord,
    /// The live stroke (`App::live_stroke_count`) and the quantity of its points that the frame shows.
    ink_drawn: (u64, usize),
    /// The area of ink that is drawn in the frame but not yet sent. AppLoad repaints the screen for
    /// each update, thus the app sends at most one ink update each INK_UPDATE_INTERVAL_MS.
    ink_box: Option<ScreenRect>,
    last_ink_update: u64,
    /// The time when new points of the live stroke were drawn.
    last_ink_draw: u64,
    /// The cells in progress on the panel, and the ink that is not sent yet.
    scheduler: Scheduler,
    /// True if the QML patch of AppLoad shows a pen region (refer to `pen_region`).
    pub use_pen_region: bool,
    /// The area (physical) where xochitl sends the updates with the fast pen waveform: the cell of
    /// the ink while the player writes. None: no area.
    pen_region: Option<PhysicalRect>,
    /// The time of the last change of `pen_region`, and true if the platform must send it.
    region_since: u64,
    region_changed: bool,
    /// A redraw that waits until the patch removed the pen region: else xochitl sends it with the pen
    /// waveform, which leaves ghosts on colored cells.
    deferred_blit: bool,
    /// True while the player writes: a redraw waits until the writing stops (`App::is_writing`).
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
            record: FrameRecord::default(),
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
        self.record = FrameRecord::default();
        self.ink_box = None;
        // The full window is drawn again with the new size.
        self.scheduler = Scheduler::new(self.scheduler.wave_ms);
        self.scheduler.fast = self.pen_region.map(PhysicalRect::pixel_cells);
        self.shown = None;
    }

    /// The frame: the drawing of the app.
    pub fn frame(&self) -> &Pixmap {
        &self.frame
    }

    /// Draws the full app and copies it into `fb` (the first drawing, before the main loop). Returns
    /// the changed physical rectangle.
    pub fn draw_all(&mut self, app: &App, fonts: &Fonts, fb: &mut [u8]) -> Option<Update> {
        render::draw(&mut Canvas::retained(&mut self.frame, &mut self.record, fonts), app);
        self.record.take_damage();
        self.shown = None;
        blit(&self.frame, fb)
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
        std::mem::take(&mut self.region_changed).then(|| self.pen_region.map(PhysicalRect::to_update))
    }

    /// The pen region for the ink of `app`: the inside of the cell of the pending ink, or of the cell
    /// of the stroke that writes (`App::writing_stroke`). The region starts only when the touch is
    /// writing: a tap never starts it, and its mark goes to the screen at once. The ink of a tap
    /// goes with the UI waveform. Ink after the start of the region waits until the patch has
    /// applied it (PEN_REGION_SETTLE_MS), so that it gets the pen waveform. xochitl refreshes the full region when it starts and when it stops, thus the
    /// region must not touch the cells around it. Its edges go inward to the 4 px grid of the panel
    /// (refer to `PanelCells`).
    fn wanted_region(&self, app: &App) -> Option<PhysicalRect> {
        if !self.use_pen_region {
            return None;
        }
        let cell = app.ui.pending_ink.as_ref().map(|i| i.cell).or_else(|| {
            let &(x, y) = app.writing_stroke()?.first()?;
            crate::layout::board_cell(x, y)
        })?;
        let (x, y) = crate::layout::cell_origin(cell);
        let c = crate::layout::CELL;
        Some(ScreenRect::new(x, y, x + c - 1, y + c - 1).physical().inner_cells())
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
            self.scheduler.fast = wanted.map(PhysicalRect::pixel_cells);
        }
        let settled = now.saturating_sub(self.region_since) >= PEN_REGION_SETTLE_MS;
        // While the player writes, a redraw waits: the player can write in many cells, one after the
        // other, and only the ink changes on the screen. An update in UI mode over the next cell
        // would stop the ink there. The redraw goes when the writing stops (`App::is_writing`).
        let writing = self.use_pen_region && app.is_writing();
        self.writing = writing;
        if app.redraw {
            // The new drawing contains the ink of the stroke. If the redraw waits, send this ink now.
            self.ink_box = app
                .live_stroke
                .as_ref()
                .filter(|_| writing)
                .and_then(|s| bounding_box(s.iter().map(|&(x, y)| (x as i32, y as i32))))
                .map(|b: ScreenRect| b.expand(render::INK_WIDTH.ceil() as i32 + 1).clip_to_screen());
            let t = Instant::now();
            render::draw(&mut Canvas::retained(&mut self.frame, &mut self.record, fonts), app);
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
            Canvas::retained(&mut self.frame, &mut self.record, fonts).polyline(new, render::INK_WIDTH, render::BLACK, true);
            self.times.ink_draw = t.elapsed();
            let (x0, y0, x1, y1) = new.iter().fold((f32::MAX, f32::MAX, f32::MIN, f32::MIN), |(a, b, c, d), &(x, y)| {
                (a.min(x), b.min(y), c.max(x), d.max(y))
            });
            let m = render::INK_WIDTH;
            let rect = ScreenRect::new(
                (x0 - m).floor() as i32,
                (y0 - m).floor() as i32,
                (x1 + m).ceil() as i32,
                (y1 + m).ceil() as i32,
            )
            .clip_to_screen();
            grow(&mut self.ink_box, rect);
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
                self.scheduler.send_now(r, &spans, now, &mut update);
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
                    ScreenRect::point(x, y).expand(m).clip_to_screen().physical().pixel_cells()
                });
            if self.scheduler.send_ink(now, hold, &mut update) {
                self.last_ink_update = now;
            }
        }
    }

    /// Records the pixels of the screen rectangle `b` that differ from the screen as it is shown, and
    /// updates `shown`.
    fn ink_changed(&mut self, b: ScreenRect) {
        let Some(shown) = &mut self.shown else {
            self.scheduler.changed(b.physical());
            return;
        };
        let (fw, w) = (shown.width() as usize, (b.x1 - b.x0 + 1) as usize);
        for y in b.y0 as usize..=b.y1 as usize {
            let o = (y * fw + b.x0 as usize) * 4;
            let (new, old) = (&self.frame.data()[o..o + w * 4], &mut shown.data_mut()[o..o + w * 4]);
            let Some((first, last)) = changed_span::<4>(new, old) else {
                continue;
            };
            old.copy_from_slice(new);
            let (x0, x1) = (b.x0 + first as i32, b.x0 + last as i32);
            self.scheduler.changed(ScreenRect::new(x0, y as i32, x1, y as i32).physical());
        }
    }
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
    dirty_box: Option<PanelCells>,
    /// The cells of the pen region: xochitl sends them with the fast pen waveform, which does not
    /// leave gaps. Thus their changes need no time to complete.
    fast: Option<PanelCells>,
}

impl Scheduler {
    fn new(wave_ms: u64) -> Scheduler {
        let w = (crate::layout::PHYSICAL_W as i32 + 3) / 4;
        let h = (crate::layout::PHYSICAL_H as i32 + 3) / 4;
        let n = (w * h) as usize;
        Scheduler { wave_ms, w, h, busy: vec![0; n], dirty: vec![false; n], dirty_box: None, fast: None }
    }

    fn clip(&self, c: PanelCells) -> Option<PanelCells> {
        PanelCells::new(c.x0.max(0), c.y0.max(0), c.x1.min(self.w - 1), c.y1.min(self.h - 1)).non_empty()
    }

    /// The time when a change of the cell (x, y) sent at `now` is complete.
    fn until(&self, x: i32, y: i32, now: u64) -> u64 {
        if self.fast.is_some_and(|f| f.contains(x, y)) { now } else { now + self.wave_ms }
    }

    fn at(&self, x: i32, y: i32) -> usize {
        (y * self.w + x) as usize
    }

    /// True if new ink in the cell (x, y) must wait for an update in progress. Ink in the pen
    /// region does not wait: it can stop an older update there, because xochitl refreshes all the
    /// region with the UI waveform when the region stops.
    fn waits(&self, x: i32, y: i32, now: u64) -> bool {
        self.busy[self.at(x, y)] > now && !self.fast.is_some_and(|f| f.contains(x, y))
    }

    /// Records changed pixels of the framebuffer that are not sent yet.
    fn changed(&mut self, r: PhysicalRect) {
        let Some(c) = self.clip(r.pixel_cells()) else { return };
        for (x, y) in c.each() {
            let i = self.at(x, y);
            self.dirty[i] = true;
        }
        grow(&mut self.dirty_box, c);
    }

    fn has_dirty(&self) -> bool {
        self.dirty_box.is_some()
    }

    fn shrink_dirty_box(&mut self) {
        let Some(d) = self.dirty_box else { return };
        self.dirty_box = bounding_box(d.each().filter(|&(x, y)| self.dirty[self.at(x, y)]));
    }

    /// Sends the update `r` at once (a change of the screen). `spans` are the changed pixels. The
    /// ink in progress that `r` stops is sent again later.
    fn send_now(&mut self, r: PhysicalRect, spans: &[PhysicalRect], now: u64, update: &mut impl FnMut(Update)) {
        if let Some(t) = self.clip(r.touched_cells()) {
            for (x, y) in t.each() {
                let i = self.at(x, y);
                if self.busy[i] > now {
                    // An update in progress stops here: send this cell again.
                    self.dirty[i] = true;
                    grow(&mut self.dirty_box, PanelCells::point(x, y));
                } else if self.dirty[i] {
                    // Ink that waited goes with this update.
                    self.dirty[i] = false;
                    self.busy[i] = self.until(x, y, now);
                }
            }
        }
        update(r.to_update());
        for &s in spans {
            let Some(c) = self.clip(s.pixel_cells()) else { continue };
            for (x, y) in c.each() {
                let i = self.at(x, y);
                self.busy[i] = self.busy[i].max(self.until(x, y, now));
            }
        }
        self.shrink_dirty_box();
    }

    /// Selects one rectangle of dirty cells that contains no cell in progress and no cell of `hold`
    /// (the pen tip), and sends it. Returns false if no cell can go now.
    fn send_ink(&mut self, now: u64, hold: Option<PanelCells>, update: &mut impl FnMut(Update)) -> bool {
        let Some(d) = self.dirty_box else { return false };
        let held = |x: i32, y: i32| hold.is_some_and(|h| h.contains(x, y));
        let free = |s: &Scheduler, x: i32, y: i32| {
            s.dirty[s.at(x, y)] && !s.waits(x, y, now) && !held(x, y)
        };
        let blocked = |s: &Scheduler, x: i32, y: i32| s.waits(x, y, now) || held(x, y);
        // The box of the free cells in `c`, and their number.
        let free_box = |s: &Scheduler, c: PanelCells| {
            let (mut b, mut n) = (None, 0usize);
            for (x, y) in c.each().filter(|&(x, y)| free(s, x, y)) {
                grow(&mut b, PanelCells::point(x, y));
                n += 1;
            }
            b.map(|b| (b, n))
        };
        let Some((mut r, _)) = free_box(self, d) else { return false };
        // Make the rectangle smaller until it contains no blocked cell. Each step removes all the
        // blocked cells at one side, at the side that keeps the most free cells.
        for _ in 0..16 {
            let Some(b) = bounding_box::<PanelGrid>(r.each().filter(|&(x, y)| blocked(self, x, y))) else { break };
            // The parts of `r` at the right, left, bottom and top of the blocked cells.
            let options = [
                PanelCells::new(b.x1 + 1, r.y0, r.x1, r.y1),
                PanelCells::new(r.x0, r.y0, b.x0 - 1, r.y1),
                PanelCells::new(r.x0, b.y1 + 1, r.x1, r.y1),
                PanelCells::new(r.x0, r.y0, r.x1, b.y0 - 1),
            ];
            let best = options
                .iter()
                .filter_map(|o| o.non_empty())
                .filter_map(|o| free_box(self, o))
                .max_by_key(|&(_, n)| n);
            match best {
                Some((nr, _)) => r = nr,
                None => {
                    // The free cells are all between blocked cells: send the first free cell.
                    let first = r.each().find(|&(x, y)| free(self, x, y) && !blocked(self, x, y));
                    match first {
                        Some((x, y)) => r = PanelCells::point(x, y),
                        None => return false,
                    }
                }
            }
        }
        if r.each().any(|(x, y)| blocked(self, x, y)) {
            return false;
        }
        update(r.rect().to_update());
        for (x, y) in r.each() {
            let i = self.at(x, y);
            if self.dirty[i] {
                self.dirty[i] = false;
                self.busy[i] = self.until(x, y, now);
            }
        }
        self.shrink_dirty_box();
        true
    }
}

impl Presenter {
    /// Copies the changes of the frame into the framebuffer. Returns the changed physical rectangle,
    /// and adds the changed pixels of each row to `spans` (physical rectangles).
    fn blit(&mut self, fb: &mut [u8], spans: &mut Vec<PhysicalRect>) -> Option<PhysicalRect> {
        let frame = &self.frame;
        let damage = self.record.take_damage();
        let Some(shown) = self.shown.as_mut().filter(|s| s.width() == frame.width() && s.height() == frame.height()) else {
            let r = blit(frame, fb).map(PhysicalRect::from_update);
            self.shown = Some(frame.clone());
            spans.extend(r);
            return r;
        };
        let (w, h) = (frame.width() as usize, frame.height() as usize);
        // The pixels of each row that can have changed: x from `rows[y].0` to `rows[y].1` (not
        // included). Out of the damage, the frame is the same as `shown`.
        let rows: Vec<(usize, usize)> = match damage {
            Damage::All => vec![(0, w); h],
            Damage::Areas(areas) => {
                let mut rows = vec![(w, 0); h];
                for a in areas {
                    for r in &mut rows[a.y0 as usize..a.y1 as usize] {
                        *r = (r.0.min(a.x0 as usize), r.1.max(a.x1 as usize));
                    }
                }
                rows
            }
        };
        let (base, sx, sy) = crate::layout::physical_steps();
        // The screen box of the changed pixels.
        let mut changed: Option<ScreenRect> = None;
        let (new, old) = (frame.data(), shown.data_mut());
        for (y, &(x0, x1)) in rows.iter().enumerate().filter(|(_, r)| r.0 < r.1) {
            let row = (y * w + x0) * 4..(y * w + x1) * 4;
            let (a, b) = (&new[row.clone()], &mut old[row]);
            let Some((first, last)) = changed_span::<4>(a, b) else {
                continue;
            };
            let mut i = base + y as i64 * sy + (x0 + first) as i64 * sx;
            for p in a[first * 4..(last + 1) * 4].chunks_exact(4) {
                fb[i as usize * 3..i as usize * 3 + 3].copy_from_slice(&p[..3]);
                i += sx;
            }
            b[first * 4..(last + 1) * 4].copy_from_slice(&a[first * 4..(last + 1) * 4]);
            let row_span = ScreenRect::new((x0 + first) as i32, y as i32, (x0 + last) as i32, y as i32);
            spans.push(row_span.physical());
            grow(&mut changed, row_span);
        }
        // The damage contains all changes of the frame: after the copy, `shown` is the frame.
        debug_assert!(new == &old[..], "a change of the frame is not in the damage of the frame record");
        changed.map(ScreenRect::physical)
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

/// The first and the last pixel (indices; `BPP` bytes for each pixel) where the row `new` is not
/// the same as the row `old`, or `None` if the two rows are the same. The fast comparison `same`
/// examines the full row first, because most rows do not change.
#[inline]
fn changed_span<const BPP: usize>(new: &[u8], old: &[u8]) -> Option<(usize, usize)> {
    if same(new, old) {
        return None;
    }
    let (new, old) = (new.chunks_exact(BPP), old.chunks_exact(BPP));
    let first = new.clone().zip(old.clone()).position(|(p, q)| p != q)?;
    let last = new.len() - 1 - new.rev().zip(old.rev()).position(|(p, q)| p != q)?;
    Some((first, last))
}

/// Makes the box `b` larger so that it contains `r`. `None` is the empty box.
#[inline]
fn grow<U>(b: &mut Option<Corners<U>>, r: Corners<U>) {
    *b = Some(match *b {
        None => r,
        Some(a) => Corners::new(a.x0.min(r.x0), a.y0.min(r.y0), a.x1.max(r.x1), a.y1.max(r.y1)),
    });
}

/// The smallest box that contains all the `points`, or `None` if there are no points.
fn bounding_box<U>(points: impl Iterator<Item = (i32, i32)>) -> Option<Corners<U>> {
    let mut b = None;
    for (x, y) in points {
        grow(&mut b, Corners::point(x, y));
    }
    b
}

/// Copies the changed area of the frame (screen coordinates) into the framebuffer (physical
/// coordinates, with the rotation of the tablet, 3 bytes for each pixel). Returns the changed
/// physical rectangle.
pub fn blit(frame: &Pixmap, fb: &mut [u8]) -> Option<Update> {
    let w = frame.width() as usize;
    let px = frame.data();
    let (base, sx, sy) = crate::layout::physical_steps();
    // The screen box of the changed pixels.
    let mut changed: Option<ScreenRect> = None;
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
            // The changed pixels of the row, in framebuffer order.
            let Some((first, last)) = changed_span::<3>(&rgb, dst) else {
                continue;
            };
            dst[first * 3..=last * 3 + 2].copy_from_slice(&rgb[first * 3..=last * 3 + 2]);
            let (a, b) = if sx == 1 { (first, last) } else { (w - 1 - last, w - 1 - first) };
            grow(&mut changed, ScreenRect::new(a as i32, y as i32, b as i32, y as i32));
        }
    } else {
        // A screen row is a framebuffer column. The copy goes in tiles of TILE x TILE pixels, so
        // that the reads of the frame and the writes of the framebuffer stay in the cache. The
        // first and the last changed pixel of each screen row give the changed box.
        const TILE: usize = 16;
        let h = frame.height() as usize;
        let mut rows = vec![(usize::MAX, 0usize); h];
        for y0 in (0..h).step_by(TILE) {
            for x0 in (0..w).step_by(TILE) {
                for x in x0..(x0 + TILE).min(w) {
                    for (y, row) in rows.iter_mut().enumerate().take((y0 + TILE).min(h)).skip(y0) {
                        let p = &px[(y * w + x) * 4..(y * w + x) * 4 + 3];
                        let i = (base + x as i64 * sx + y as i64 * sy) as usize * 3;
                        let o = &mut fb[i..i + 3];
                        if *o != *p {
                            o.copy_from_slice(p);
                            *row = (row.0.min(x), row.1.max(x));
                        }
                    }
                }
            }
        }
        for (y, &(first, last)) in rows.iter().enumerate().filter(|(_, r)| r.0 != usize::MAX) {
            grow(&mut changed, ScreenRect::new(first as i32, y as i32, last as i32, y as i32));
        }
    }
    changed.map(|r| r.physical().to_update())
}

/// Copies a screen rectangle of the frame into the framebuffer.
fn blit_rect(frame: &Pixmap, fb: &mut [u8], r: ScreenRect) {
    let w = frame.width() as usize;
    let px = frame.pixels();
    let (base, sx, sy) = crate::layout::physical_steps();
    for y in r.y0 as usize..=r.y1 as usize {
        for x in r.x0 as usize..=r.x1 as usize {
            let p = px[y * w + x];
            let i = (base + x as i64 * sx + y as i64 * sy) as usize * 3;
            fb[i..i + 3].copy_from_slice(&[p.red(), p.green(), p.blue()]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};

    #[test]
    fn the_tiled_copy_of_a_rotated_frame_is_the_same_as_a_copy_of_each_pixel() {
        // `blit` copies a landscape frame in tiles. The framebuffer and the changed box must be the
        // same as a copy of each pixel, also when only some pixels change.
        for rotation in [1, 2] {
            crate::layout::set_rotation(rotation);
            let mut frame = Pixmap::new(screen_w(), screen_h()).unwrap();
            for (k, p) in frame.data_mut().chunks_exact_mut(4).enumerate() {
                p.copy_from_slice(&[(k * 7) as u8, (k / 3) as u8, (k % 251) as u8, 255]);
            }
            let n = (crate::layout::PHYSICAL_W * crate::layout::PHYSICAL_H * 3) as usize;
            let (mut fb, mut want) = (vec![0u8; n], vec![0u8; n]);
            for change in 0..3 {
                if change > 0 {
                    // A few pixels change (a box in the middle of the screen, and one corner).
                    for (x, y) in [(500 + change * 3, 300), (501, 301 + change), (0, 0)] {
                        frame.data_mut()[(y * screen_w() as usize + x) * 4] ^= 0x55;
                    }
                }
                let got = blit(&frame, &mut fb);
                // The copy of each pixel.
                let (base, sx, sy) = crate::layout::physical_steps();
                let mut b: Option<ScreenRect> = None;
                for y in 0..screen_h() as usize {
                    for x in 0..screen_w() as usize {
                        let p = &frame.data()[(y * screen_w() as usize + x) * 4..][..3];
                        let i = (base + x as i64 * sx + y as i64 * sy) as usize * 3;
                        if want[i..i + 3] != *p {
                            want[i..i + 3].copy_from_slice(p);
                            grow(&mut b, ScreenRect::point(x as i32, y as i32));
                        }
                    }
                }
                assert!(fb == want, "rotation {rotation}, change {change}: framebuffer");
                assert_eq!(got, b.map(|r| r.physical().to_update()), "rotation {rotation}, change {change}: box");
            }
        }
        crate::layout::set_rotation(0);
    }

    /// The app with the puzzle of seed 7 and the menu closed. The app opens with the menu open
    /// (PER-2), but these tests examine only the board: the closed menu does not change when the
    /// undo button becomes black.
    fn game_app() -> App {
        let mut app = App::new(Some(crate::app::new_game(7, 0)), None, crate::store::ResultsDb::in_memory(), Box::new(crate::script::FixedEnv(7)), App::placeholder_ng_stops());
        app.ui.menu = crate::app::Menu::Closed;
        app
    }

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
            let c = PanelCells::new(x0, y0, x0 + rng.next(w / 4 - x0), y0 + rng.next(h / 4 - y0));
            assert_eq!(c.rect().touched_cells(), c);
        }
        // An update touches the cells of the extra pixel of AppLoad too.
        assert_eq!(PhysicalRect::new(0, 0, 2, 2).touched_cells(), PanelCells::new(0, 0, 0, 0));
        assert_eq!(PhysicalRect::new(0, 0, 3, 3).touched_cells(), PanelCells::new(0, 0, 1, 1));
    }

    #[test]
    fn ink_over_cells_in_progress_waits_until_they_are_complete() {
        let mut s = Scheduler::new(400);
        let mut sent = Vec::new();
        s.changed(PhysicalRect::new(10, 10, 40, 40));
        assert!(s.send_ink(0, None, &mut |u| sent.push(u)));
        assert_eq!(sent, vec![(8, 8, 35, 35)]);
        sent.clear();
        s.changed(PhysicalRect::new(30, 30, 70, 70));
        assert!(s.send_ink(30, None, &mut |u| sent.push(u)));
        // One rectangle, outside the cells of the first update.
        assert_eq!(sent.len(), 1);
        let c = PhysicalRect::from_update(sent[0]).touched_cells();
        assert!(c.each().all(|(x, y)| !(x <= 10 && y <= 10)), "{c:?}");
        // The joint goes when the first update is complete.
        let (late, end) = drain(&mut s, 30);
        assert!(end >= 400 && end <= 460, "{end}");
        assert!(late.iter().any(|&u| PhysicalRect::from_update(u).touched_cells().contains(9, 9)), "{late:?}");
    }

    #[test]
    fn the_inner_cells_are_on_the_grid_and_inside_the_rectangle() {
        assert_eq!(PhysicalRect::new(148, 418, 294, 564).inner_cells(), PhysicalRect::new(148, 420, 291, 563));
        let mut rng = Rng(3);
        for _ in 0..1000 {
            let (x0, y0) = (rng.next(1000), rng.next(1000));
            let r = PhysicalRect::new(x0, y0, x0 + 8 + rng.next(200), y0 + 8 + rng.next(200));
            let i = r.inner_cells();
            assert!(i.x0 >= r.x0 && i.y0 >= r.y0 && i.x1 <= r.x1 && i.y1 <= r.y1, "{r:?} {i:?}");
            assert!(i.x0 % 4 == 0 && i.y0 % 4 == 0 && (i.x1 + 1) % 4 == 0 && (i.y1 + 1) % 4 == 0, "{i:?}");
        }
    }

    #[test]
    fn ink_in_the_pen_region_does_not_wait() {
        let mut s = Scheduler::new(400);
        s.fast = Some(PhysicalRect::new(0, 0, 99, 99).pixel_cells());
        let mut sent = Vec::new();
        s.changed(PhysicalRect::new(10, 10, 40, 40));
        s.send_ink(0, None, &mut |u| sent.push(u));
        s.changed(PhysicalRect::new(30, 30, 70, 70));
        s.send_ink(30, None, &mut |u| sent.push(u));
        // The second update goes complete at once, over the first one.
        assert_eq!(sent.len(), 2);
        assert!(!s.has_dirty());
        assert_eq!(PhysicalRect::from_update(sent[1]).touched_cells(), PhysicalRect::new(30, 30, 70, 70).pixel_cells());
    }

    /// A redraw in UI mode covers the next cell, and the player writes in that cell at once: the
    /// ink in the pen region does not wait until the redraw is complete.
    #[test]
    fn ink_in_the_pen_region_does_not_wait_for_a_redraw() {
        let mut s = Scheduler::new(400);
        let mut sent = Vec::new();
        let all = PhysicalRect::new(0, 0, 199, 99);
        s.send_now(all, &[all], 0, &mut |u| sent.push(u));
        sent.clear();
        s.fast = Some(PhysicalRect::new(100, 0, 199, 99).pixel_cells());
        s.changed(PhysicalRect::new(130, 30, 170, 70));
        assert!(s.send_ink(30, None, &mut |u| sent.push(u)));
        assert_eq!(PhysicalRect::from_update(sent[0]).touched_cells(), PhysicalRect::new(130, 30, 170, 70).pixel_cells());
        // Outside the pen region, the ink waits.
        s.changed(PhysicalRect::new(30, 30, 70, 70));
        assert!(!s.send_ink(60, None, &mut |u| sent.push(u)));
        assert!(s.send_ink(400, None, &mut |u| sent.push(u)));
    }

    /// The player writes in one cell and at once in a second cell (pen region on). The ink of the
    /// second cell goes while the pen moves, and the redraw of the first cell waits until the app
    /// has read the last ink.
    #[test]
    fn a_redraw_waits_while_the_player_writes_in_many_cells() {
        use crate::app::{Input, Tool};
        let fonts = Fonts::repo().unwrap();
        let mut app = game_app();
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
            // While the pen writes, the pen region is on, and all the ink is in it (the first ink of
            // a stroke also gets the pen waveform).
            if app.writing_stroke().is_some() {
                let r = p.pen_region.expect("the pen region is on while the pen writes");
                for &u in &sent {
                    let u = PhysicalRect::from_update(u);
                    assert!(u.x0 >= r.x0 && u.y0 >= r.y0 && u.x1 <= r.x1 + 1 && u.y1 <= r.y1 + 1, "{u:?} is not in {r:?}");
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
            let r = ScreenRect::new(x, y, x + c - 1, y + c - 1).physical();
            let u = PhysicalRect::from_update(u);
            u.x0 >= r.x0 - 4 && u.y0 >= r.y0 - 4 && u.x1 <= r.x1 + 4 && u.y1 <= r.y1 + 4
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

    /// The player writes a number and moves the pen slowly to the next cell (pen region on). The app
    /// reads the ink after the recognition pause, but the redraw waits until no stroke occurred for
    /// WRITING_IDLE_MS. A new stroke starts the time again. A press that does not write stops it.
    #[test]
    fn a_redraw_waits_until_the_player_has_not_written_for_the_idle_time() {
        use crate::app::{Input, Tool, WRITING_IDLE_MS};
        let fonts = Fonts::repo().unwrap();
        let mut app = game_app();
        let mut p = Presenter::new();
        p.use_pen_region = true;
        let mut fb = vec![0u8; (crate::layout::PHYSICAL_W * crate::layout::PHYSICAL_H * 3) as usize];
        let empty: Vec<_> = (0..9u8).flat_map(|r| (0..9u8).map(move |c| (r, c))).filter(|&c| !app.game.is_filled(c)).take(2).collect();
        let (a, b) = (empty[0], empty[1]);
        let step = |app: &mut App, p: &mut Presenter, fb: &mut Vec<u8>, now: u64, input: Option<Input>| {
            if let Some(i) = input {
                app.handle(i, now);
            }
            app.tick(now);
            p.present(app, &fonts, fb, now, |_| {});
        };
        // A vertical stroke in the cell (a 1), one point each 10 ms.
        let write = |app: &mut App, p: &mut Presenter, fb: &mut Vec<u8>, now: &mut u64, cell: crate::game::Cell| {
            let (x, y) = crate::layout::cell_origin(cell);
            let (x, c) = (x as f32 + crate::layout::CELL as f32 / 2.0, crate::layout::CELL as f32);
            let (y0, y1) = (y as f32 + 0.2 * c, y as f32 + 0.8 * c);
            step(app, p, fb, *now, Some(Input::PenDown { x, y: y0, tool: Tool::Pen }));
            for k in 1..=20 {
                *now += 10;
                step(app, p, fb, *now, Some(Input::PenMove { x, y: y0 + (y1 - y0) * k as f32 / 20.0 }));
            }
            *now += 10;
            step(app, p, fb, *now, Some(Input::PenUp { x, y: y1 }));
        };
        let mut now = 1000;
        step(&mut app, &mut p, &mut fb, now, None);

        write(&mut app, &mut p, &mut fb, &mut now, a);
        let lift = now;
        // The app reads the ink after the recognition pause, but the redraw waits.
        while now + 30 < lift + WRITING_IDLE_MS {
            now += 30;
            step(&mut app, &mut p, &mut fb, now, None);
        }
        assert!(app.ui.pending_ink.is_none(), "the ink is not read after the recognition pause");
        assert!(app.is_writing() && p.deferred_blit, "{now}: the redraw went before the idle time");

        // A second number starts the idle time again.
        write(&mut app, &mut p, &mut fb, &mut now, b);
        let lift = now;
        while now + 30 < lift + WRITING_IDLE_MS {
            now += 30;
            step(&mut app, &mut p, &mut fb, now, None);
            assert!(p.deferred_blit, "{now}: the redraw went before the idle time");
        }
        // The idle time is over: the deadline of the app wakes the loop, and the redraw goes.
        assert_eq!(app.next_deadline(), Some(lift + WRITING_IDLE_MS));
        now = lift + WRITING_IDLE_MS;
        step(&mut app, &mut p, &mut fb, now, None);
        now += PEN_REGION_SETTLE_MS;
        step(&mut app, &mut p, &mut fb, now, None);
        assert!(!app.is_writing() && !p.deferred_blit, "the redraw did not go after the idle time");
        let mut expect = vec![0u8; fb.len()];
        blit(&p.frame, &mut expect);
        assert!(fb == expect, "the framebuffer does not show the frame");

        // A press that does not write (here a press out of the board) stops the writing at once.
        write(&mut app, &mut p, &mut fb, &mut now, a);
        assert!(app.is_writing());
        now += 10;
        step(&mut app, &mut p, &mut fb, now, Some(Input::PenDown { x: 5.0, y: 5.0, tool: Tool::Pen }));
        assert!(!app.is_writing());
    }

    /// A tap shows its ink with the UI waveform, without the pen region (INP-2, MRK-5). After a tap
    /// that changes nothing (a mark that a peer hides), the screen is the same as before. When the
    /// pen leaves the micro-cell, the touch is writing and the pen region starts.
    #[test]
    fn a_tap_does_not_start_the_pen_region_and_a_tap_that_changes_nothing_leaves_no_ink() {
        use crate::app::{Input, Tool};
        let fonts = Fonts::repo().unwrap();
        let mut app = game_app();
        let mut p = Presenter::new();
        p.use_pen_region = true;
        let mut fb = vec![0u8; (crate::layout::PHYSICAL_W * crate::layout::PHYSICAL_H * 3) as usize];
        let step = |app: &mut App, p: &mut Presenter, fb: &mut Vec<u8>, now: u64, input: Option<Input>| {
            if let Some(i) = input {
                app.handle(i, now);
            }
            app.tick(now);
            p.present(app, &fonts, fb, now, |_| {});
        };
        // An empty cell and a digit that a peer has: a tap cannot add this mark.
        let g = &app.game;
        let (cell, d) = crate::game::cells()
            .filter(|&c| !g.is_filled(c))
            .find_map(|c| (1..=9u8).find(|&d| g.peer_digits(c) & crate::game::mark_bit(d) != 0).map(|d| (c, d)))
            .unwrap();
        let (x, y) = crate::layout::cell_origin(cell);
        let m = crate::layout::CELL as f32 / 3.0;
        let (x, y) = (x as f32 + m * (((d - 1) % 3) as f32 + 0.5), y as f32 + m * (((d - 1) / 3) as f32 + 0.5));
        let mut now = 1000;
        step(&mut app, &mut p, &mut fb, now, None);
        for _ in 0..20 {
            now += 30;
            step(&mut app, &mut p, &mut fb, now, None);
        }
        let before = fb.clone();
        step(&mut app, &mut p, &mut fb, now, Some(Input::PenDown { x, y, tool: Tool::Pen }));
        for k in 1..=3 {
            now += 10;
            step(&mut app, &mut p, &mut fb, now, Some(Input::PenMove { x: x + k as f32, y }));
            assert!(p.pen_region.is_none() && app.writing_stroke().is_none(), "a tap started the pen region");
        }
        now += 40;
        step(&mut app, &mut p, &mut fb, now, None);
        assert!(fb != before, "the ink of the tap does not show");
        now += 10;
        step(&mut app, &mut p, &mut fb, now, Some(Input::PenUp { x: x + 3.0, y }));
        for _ in 0..20 {
            now += 30;
            step(&mut app, &mut p, &mut fb, now, None);
        }
        assert_eq!(app.game.marks_at(cell), 0);
        assert!(fb == before, "the ink of a tap that changes nothing stays on the screen");

        // The pen leaves the micro-cell: the touch is writing, and the pen region starts.
        now += 300;
        step(&mut app, &mut p, &mut fb, now, Some(Input::PenDown { x, y, tool: Tool::Pen }));
        now += 10;
        step(&mut app, &mut p, &mut fb, now, Some(Input::PenMove { x, y: y + m }));
        assert_eq!(app.writing_stroke().map(|s| s.len()), Some(2));
        assert!(p.pen_region.is_some(), "writing did not start the pen region");
    }

    /// A tap that adds a mark does not start the pen region. The mark goes to the screen at the pen
    /// up, in a small update inside the micro-cell.
    #[test]
    fn a_mark_goes_to_the_screen_at_the_pen_up() {
        use crate::app::{Input, Tool};
        let fonts = Fonts::repo().unwrap();
        let mut app = game_app();
        let mut p = Presenter::new();
        p.use_pen_region = true;
        let mut fb = vec![0u8; (crate::layout::PHYSICAL_W * crate::layout::PHYSICAL_H * 3) as usize];
        let step = |app: &mut App, p: &mut Presenter, fb: &mut Vec<u8>, now: u64, input: Option<Input>| {
            if let Some(i) = input {
                app.handle(i, now);
            }
            app.tick(now);
            let mut sent = Vec::new();
            p.present(app, &fonts, fb, now, |u| sent.push(u));
            assert!(p.pen_region.is_none() && !p.region_changed, "{now}: a tap started the pen region");
            sent
        };
        // An empty cell and the digit of its solution: a tap adds this mark.
        let cell = crate::game::cells().find(|&c| !app.game.is_filled(c)).unwrap();
        let d = app.game.solution[cell.0 as usize][cell.1 as usize];
        let (cx, cy) = crate::layout::cell_origin(cell);
        let m = crate::layout::CELL as f32 / 3.0;
        let (mx, my) = (cx as f32 + m * ((d - 1) % 3) as f32, cy as f32 + m * ((d - 1) / 3) as f32);
        let (x, y) = (mx + m / 2.0, my + m / 2.0);
        let mut now = 1000;
        step(&mut app, &mut p, &mut fb, now, None);
        for _ in 0..20 {
            now += 30;
            step(&mut app, &mut p, &mut fb, now, None);
        }
        let micro = ScreenRect::new(mx as i32, my as i32, (mx + m) as i32, (my + m) as i32).physical().expand(4);
        let inside = |u: Update| {
            let u = PhysicalRect::from_update(u);
            u.x0 >= micro.x0 && u.y0 >= micro.y0 && u.x1 <= micro.x1 && u.y1 <= micro.y1
        };
        let mut sent = step(&mut app, &mut p, &mut fb, now, Some(Input::PenDown { x, y, tool: Tool::Pen }));
        now += 10;
        sent.extend(step(&mut app, &mut p, &mut fb, now, Some(Input::PenMove { x: x + 1.0, y })));
        now += 10;
        let at_up = step(&mut app, &mut p, &mut fb, now, Some(Input::PenUp { x: x + 1.0, y }));
        assert_ne!(app.game.marks_at(cell) & crate::game::mark_bit(d), 0);
        assert!(!at_up.is_empty(), "the mark is not sent at the pen up");
        for u in sent.into_iter().chain(at_up) {
            assert!(inside(u), "{u:?} is not in the micro-cell {micro:?}");
        }
        let mut expect = vec![0u8; fb.len()];
        blit(&p.frame, &mut expect);
        assert!(fb == expect, "the framebuffer does not show the mark");
    }

    #[test]
    fn the_pen_tip_waits_while_the_pen_moves() {
        let mut s = Scheduler::new(400);
        let mut sent = Vec::new();
        s.changed(PhysicalRect::new(100, 100, 140, 140));
        let tip = PhysicalRect::new(128, 128, 140, 140).pixel_cells();
        assert!(s.send_ink(0, Some(tip), &mut |u| sent.push(u)));
        for &u in &sent {
            let c = PhysicalRect::from_update(u).touched_cells();
            assert!(c.each().all(|(x, y)| !tip.contains(x, y)), "{c:?}");
        }
        // When the pen stops, the tip goes.
        let (late, _) = drain(&mut s, 0);
        assert!(late.iter().any(|&u| PhysicalRect::from_update(u).touched_cells().contains(34, 34)), "{late:?}");
    }

    #[test]
    fn a_change_of_the_screen_does_not_wait_and_the_stopped_ink_is_sent_again() {
        let mut s = Scheduler::new(400);
        let mut sent = Vec::new();
        s.changed(PhysicalRect::new(10, 10, 40, 40));
        s.send_ink(0, None, &mut |u| sent.push(u));
        sent.clear();
        let top = PhysicalRect::new(0, 0, 99, 19);
        s.send_now(top, &[top], 10, &mut |u| sent.push(u));
        assert_eq!(sent, vec![(0, 0, 100, 20)]);
        assert!(s.has_dirty());
        let (late, end) = drain(&mut s, 10);
        assert!(end >= 410, "{end}");
        assert!(late.iter().any(|&u| PhysicalRect::from_update(u).touched_cells().contains(3, 3)), "{late:?}");
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
            let tick = |s: &mut Scheduler, now: u64, hold: Option<PanelCells>, dirty: &mut HashSet<(i32, i32)>, busy: &mut HashMap<(i32, i32), u64>| {
                let mut sent = Vec::new();
                s.send_ink(now, hold, &mut |u| sent.push(u));
                assert!(sent.len() <= 1);
                for u in sent {
                    let c = PhysicalRect::from_update(u).touched_cells();
                    for p in c.each() {
                        assert!(busy.get(&p).copied().unwrap_or(0) <= now, "{p:?} in progress");
                        assert!(!hold.is_some_and(|h| h.contains(p.0, p.1)), "{p:?} at the tip");
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
                    let r = PhysicalRect::point(px, py).expand(3);
                    s.changed(r);
                    dirty.extend(r.pixel_cells().each());
                }
                (x, y) = (nx, ny);
                let hold = Some(PhysicalRect::point(x, y).expand(7).pixel_cells());
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

