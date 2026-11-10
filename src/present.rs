//! The output of the main loop: it draws the app into the frame, copies the changes into the
//! framebuffer and gives the rectangles to update. It has no input or output, thus the bench and the
//! device use the same code.

use std::marker::PhantomData;
use std::time::{Duration, Instant};

use tiny_skia::Pixmap;

use crate::app::App;
use crate::font::Fonts;
use crate::game::Cell;
use crate::layout::{screen_h, screen_w};
use crate::render::{self, Canvas, Damage, FrameRecord, PenDrawn};

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

    /// The same pixels in the coordinates of the window of AppLoad (x, y, width, height): the
    /// coordinates of the screen as the player sees it. AppLoad turns the framebuffer when it draws
    /// it (`fbRotation`), but the items of its window, also the pen area of the ink patch, use the
    /// coordinates of the turned window. In portrait (rotation 0) they are the physical coordinates.
    fn to_window(self) -> Update {
        crate::layout::screen_rect(self.to_update())
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
        let (w, h) = (crate::layout::physical_w() as i32, crate::layout::physical_h() as i32);
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
    /// True if the QML patch of AppLoad shows a pen region (refer to `pen_cell`).
    pub use_pen_region: bool,
    /// The cell of the pen region: xochitl sends the updates inside the cell with the fast pen
    /// waveform (R§14.4). None: no region.
    pen_cell: Option<Cell>,
    /// The marks and the x that the pen waveform drew in `pen_cell` (refer to `FrameRecord::pen_change`).
    pen_drawn: PenDrawn,
    /// The time of the last change of `pen_cell`, and true if the platform must send the region.
    region_since: u64,
    region_changed: bool,
    /// A redraw that waits: while the player writes, and until the patch applied the last change of
    /// the pen region. A redraw that needs the UI waveform in the region (a number, a "?", a panel)
    /// stops the region first: else xochitl sends it with the pen waveform, which leaves ghosts.
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
            pen_cell: None,
            pen_drawn: PenDrawn::default(),
            region_since: 0,
            region_changed: false,
            deferred_blit: false,
            writing: false,
            shown: None,
            times: Times::default(),
        }
    }

    /// Starts again after a rotation at the time `now`: the frame has the new size. The pen region
    /// stops: its cell is at a different place now, and the full window is drawn again.
    pub fn reset(&mut self, now: u64) {
        self.frame = Pixmap::new(screen_w(), screen_h()).unwrap();
        self.record = FrameRecord::default();
        self.ink_box = None;
        self.scheduler = Scheduler::new(self.scheduler.wave_ms);
        self.set_region(None, now);
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
            .then(|| (self.last_ink_update + INK_UPDATE_INTERVAL_MS).saturating_sub(now).max(if self.pen_cell.is_some() { settle } else { 0 }));
        // A redraw that waits for the end of the writing goes after the next input or timer of the app.
        let blit = (self.deferred_blit && !self.writing).then_some(settle);
        ink.into_iter().chain(blit).min()
    }

    /// The new pen region (x, y, width, height in the coordinates of the window of AppLoad; None: no
    /// region), if it changed since the last call. The platform gives it to the QML patch of AppLoad.
    /// The patch puts the area in the window, thus a physical rectangle is at the wrong place when
    /// the tablet is turned (landscape, or portrait at 180°): the ink then gets the UI waveform.
    pub fn take_region_change(&mut self) -> Option<Option<Update>> {
        std::mem::take(&mut self.region_changed).then(|| self.pen_region().map(PhysicalRect::to_window))
    }

    /// The pen region (physical): the inside of `pen_cell`. None: no region.
    fn pen_region(&self) -> Option<PhysicalRect> {
        self.pen_cell.map(cell_region)
    }

    /// Puts the pen region on `cell` (None: no region) at the time `now`, if it is not there already.
    /// Ink after a change waits until the patch has applied it (PEN_REGION_SETTLE_MS), so that it gets
    /// the pen waveform. A redraw also waits for it.
    fn set_region(&mut self, cell: Option<Cell>, now: u64) {
        if cell == self.pen_cell {
            return;
        }
        self.pen_cell = cell;
        self.pen_drawn = PenDrawn::default();
        self.region_since = now;
        self.region_changed = true;
        self.scheduler.fast = self.pen_region().map(PhysicalRect::pixel_cells);
    }

    /// True if the patch has applied the last change of the pen region.
    fn region_settled(&self, now: u64) -> bool {
        now.saturating_sub(self.region_since) >= PEN_REGION_SETTLE_MS
    }

    /// Draws the app, copies the changes into `fb` and calls `update` for each physical rectangle to
    /// send to the screen.
    pub fn present(&mut self, app: &mut App, fonts: &Fonts, fb: &mut [u8], now: u64, mut update: impl FnMut(Update)) {
        self.times = Times::default();
        if self.use_pen_region {
            // The pen region follows the pen (`App::pen_cell`): the cell of the waiting ink, or the
            // empty cell of the pen touch, from the pen down (INP-24). With no cell from the app, the
            // region stays on its cell: the mark or the x of a tap goes to the screen in the region
            // at the pen lift, and a stop would refresh the full cell (R§14.4). The region never stays
            // on a filled cell (INP-22). It stops before a redraw that needs the UI waveform in it
            // (below), and at a turn of the tablet (`reset`).
            let wanted = app.pen_cell().or_else(|| self.pen_cell.filter(|&c| !app.game.is_filled(c)));
            self.set_region(wanted, now);
        }
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
            Canvas::retained(&mut self.frame, &mut self.record, fonts).ink(new);
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
        if self.deferred_blit && !writing {
            // The pen waveform shows the marks, the x and the ink of the cell of the region: they are
            // crisp drawings, exactly black or gray15 on white (R§14.4, MRK-19, INP-25), thus the
            // pixel conversion of the pen waveform does not dither them. A redraw with any other
            // change in that cell (a number, a "?", a panel over it) stops the region first, and
            // goes when the patch applied the stop: else xochitl sends it with the pen waveform,
            // which leaves ghosts on gray and colored pixels.
            // The pen waveform also cannot remove a mark that the UI waveform drew: a ghost stays.
            let pen_drawn = self.pen_cell.and_then(|cell| self.record.pen_change(cell, self.pen_drawn));
            if pen_drawn.is_none() {
                self.set_region(None, now);
            }
            if !self.use_pen_region || self.region_settled(now) {
                self.deferred_blit = false;
                let t = Instant::now();
                let mut spans = Vec::new();
                let changed = self.blit(fb, &mut spans);
                self.times.blit = t.elapsed();
                if let Some(d) = pen_drawn {
                    self.pen_drawn = d;
                }
                if let Some(r) = changed {
                    // A change of the screen does not wait. The ink in progress that it stops is sent
                    // again later.
                    self.scheduler.send_now(r, &spans, now, &mut update);
                }
            }
        }
        // Ink in a new pen region waits until the patch has applied it.
        let region_ready = self.pen_cell.is_none() || self.region_settled(now);
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
                .filter(|_| self.pen_cell.is_none())
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

/// The pen region for the cell: the inside of the cell, with its edges inward on the 4 px grid of the
/// panel (refer to `PanelCells`). xochitl refreshes the full region when it starts and when it stops,
/// thus the region must not touch the cells around it (R§14.4).
fn cell_region(cell: Cell) -> PhysicalRect {
    let (x, y) = crate::layout::cell_origin(cell);
    let c = crate::layout::cell_size();
    ScreenRect::new(x, y, x + c - 1, y + c - 1).physical().inner_cells()
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
        let w = (crate::layout::physical_w() as i32 + 3) / 4;
        let h = (crate::layout::physical_h() as i32 + 3) / 4;
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
        let new = frame.data();
        let pixels = rows.iter().map(|r| r.1.saturating_sub(r.0)).sum();
        let bands = in_bands(h, pixels, shown.data_mut(), w * 4, fb, |ys, old, fb, fb_at| {
            let (mut band_spans, mut changed) = (Vec::new(), None);
            for y in ys.clone() {
                let (x0, x1) = rows[y];
                if x0 >= x1 {
                    continue;
                }
                let at = ((y - ys.start) * w + x0) * 4..((y - ys.start) * w + x1) * 4;
                let (a, b) = (&new[(y * w + x0) * 4..(y * w + x1) * 4], &mut old[at]);
                let Some((first, last)) = changed_span::<4>(a, b) else {
                    continue;
                };
                let mut i = base + y as i64 * sy + (x0 + first) as i64 * sx - fb_at;
                for p in a[first * 4..(last + 1) * 4].chunks_exact(4) {
                    fb[i as usize * 3..i as usize * 3 + 3].copy_from_slice(&p[..3]);
                    i += sx;
                }
                b[first * 4..(last + 1) * 4].copy_from_slice(&a[first * 4..(last + 1) * 4]);
                let row_span = ScreenRect::new((x0 + first) as i32, y as i32, (x0 + last) as i32, y as i32);
                band_spans.push(row_span);
                grow(&mut changed, row_span);
            }
            (band_spans, changed)
        });
        // The screen box of the changed pixels.
        let mut changed: Option<ScreenRect> = None;
        for (band_spans, band_changed) in bands {
            // On this thread: the rotation is a value of each thread.
            spans.extend(band_spans.into_iter().map(ScreenRect::physical));
            if let Some(r) = band_changed {
                grow(&mut changed, r);
            }
        }
        // The damage contains all changes of the frame: after the copy, `shown` is the frame.
        debug_assert!(new == shown.data(), "a change of the frame is not in the damage of the frame record");
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

/// A copy of fewer changed pixels than this goes on one thread: the start of the threads takes
/// more time than they save.
const BAND_MIN_PIXELS: usize = 400_000;

/// Calls `copy` for bands of the screen rows `0..h`, and gives the results in the order of the rows.
/// `copy` gets the rows of the band, its rows of `rows_buf` (`row_bytes` for each row), its part of
/// the framebuffer `fb`, and the pixel index of the start of that part. The bands go on threads when
/// `pixels` (the pixels that can have changed) is large and each screen row is a framebuffer row
/// (portrait): then the parts of the framebuffer do not overlap. Each row gets the same copy as
/// with one band, thus the result does not depend on the threads. The rotation is a value of each
/// thread (`layout::rotation`): `copy` must not use it, or functions that use it.
fn in_bands<R: Send>(
    h: usize,
    pixels: usize,
    rows_buf: &mut [u8],
    row_bytes: usize,
    fb: &mut [u8],
    copy: impl Fn(std::ops::Range<usize>, &mut [u8], &mut [u8], i64) -> R + Sync,
) -> Vec<R> {
    let (base, sx, sy) = crate::layout::physical_steps();
    let threads = crate::system::threads();
    if threads == 1 || pixels < BAND_MIN_PIXELS || sx.abs() != 1 {
        return vec![copy(0..h, rows_buf, fb, 0)];
    }
    let w = screen_w() as i64;
    let edges: Vec<usize> = (0..=threads).map(|k| h * k / threads).collect();
    // The pixel range of the framebuffer of each band (its rows are framebuffer rows).
    let ranges: Vec<(i64, i64)> = edges
        .windows(2)
        .map(|e| {
            let ends = [e[0] as i64, e[1] as i64 - 1].map(|y| base + y * sy);
            let ends = [ends[0], ends[0] + (w - 1) * sx, ends[1], ends[1] + (w - 1) * sx];
            (*ends.iter().min().unwrap(), *ends.iter().max().unwrap() + 1)
        })
        .collect();
    // Split the framebuffer into the parts of the bands, from the lowest address.
    let mut order: Vec<usize> = (0..ranges.len()).collect();
    order.sort_by_key(|&k| ranges[k].0);
    let mut parts: Vec<Option<&mut [u8]>> = (0..ranges.len()).map(|_| None).collect();
    let (mut rest, mut at) = (fb, 0i64);
    for k in order {
        let (lo, hi) = ranges[k];
        let (_, tail) = std::mem::take(&mut rest).split_at_mut(((lo - at) * 3) as usize);
        let (part, tail) = tail.split_at_mut(((hi - lo) * 3) as usize);
        parts[k] = Some(part);
        (rest, at) = (tail, hi);
    }
    let mut rows_rest = rows_buf;
    let jobs: Vec<_> = edges
        .windows(2)
        .zip(parts)
        .zip(&ranges)
        .map(|((e, part), &(lo, _))| {
            let (band_rows, tail) = std::mem::take(&mut rows_rest).split_at_mut((e[1] - e[0]) * row_bytes);
            rows_rest = tail;
            (e[0]..e[1], band_rows, part.expect("a part for each band"), lo)
        })
        .collect();
    let copy = &copy;
    std::thread::scope(|scope| {
        let mut jobs = jobs.into_iter();
        let (ys, band_rows, part, lo) = jobs.next().expect("one band or more");
        let handles: Vec<_> = jobs.map(|(ys, r, p, lo)| scope.spawn(move || copy(ys, r, p, lo))).collect();
        let first = copy(ys, band_rows, part, lo);
        std::iter::once(first).chain(handles.into_iter().map(|t| t.join().expect("copy thread"))).collect()
    })
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
        let h = frame.height() as usize;
        let bands = in_bands(h, w * h, &mut [], 0, fb, |ys, _, fb, fb_at| {
            let mut rgb = vec![0u8; w * 3];
            let mut changed = None;
            for y in ys {
                let src = &px[y * w * 4..(y + 1) * w * 4];
                let start = base + y as i64 * sy - fb_at;
                // The first byte of the row in the part of the framebuffer.
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
            changed
        });
        for r in bands.into_iter().flatten() {
            grow(&mut changed, r);
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
    fn the_copy_of_a_frame_is_the_same_as_a_copy_of_each_pixel() {
        // `blit` copies a landscape frame in tiles, and a portrait frame in bands of rows on
        // threads. The framebuffer and the changed box must be the same as a copy of each pixel, in
        // each rotation, also when only some pixels change.
        for rotation in 0..4 {
            crate::layout::set_rotation(rotation);
            let mut frame = Pixmap::new(screen_w(), screen_h()).unwrap();
            for (k, p) in frame.data_mut().chunks_exact_mut(4).enumerate() {
                p.copy_from_slice(&[(k * 7) as u8, (k / 3) as u8, (k % 251) as u8, 255]);
            }
            let n = (crate::layout::physical_w() * crate::layout::physical_h() * 3) as usize;
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

    #[test]
    fn the_copy_of_a_large_damage_in_bands_gives_each_changed_row() {
        // `Presenter::blit` copies a large damage in bands of rows on threads. The framebuffer, the
        // span of each row and the changed box must be the same as a copy of each pixel, in each
        // rotation (the rotation is a value of each thread).
        for rotation in 0..4 {
            crate::layout::set_rotation(rotation);
            let mut p = Presenter::new();
            p.frame.fill(tiny_skia::Color::WHITE);
            let n = (crate::layout::physical_w() * crate::layout::physical_h() * 3) as usize;
            let (mut fb, mut want) = (vec![0u8; n], vec![0u8; n]);
            p.blit(&mut fb, &mut Vec::new());
            want.copy_from_slice(&fb);
            // Most rows change: the damage is large.
            let (w, h) = (screen_w() as usize, screen_h() as usize);
            for (k, px) in p.frame.data_mut().chunks_exact_mut(4).enumerate() {
                if (k / w) % 7 != 3 {
                    px.copy_from_slice(&[(k * 7) as u8, (k / 3) as u8, (k % 251) as u8, 255]);
                }
            }
            p.record = FrameRecord::default();
            let mut spans = Vec::new();
            let got = p.blit(&mut fb, &mut spans);
            let (base, sx, sy) = crate::layout::physical_steps();
            let (mut b, mut want_spans): (Option<ScreenRect>, Vec<PhysicalRect>) = (None, Vec::new());
            for y in 0..h {
                let mut row: Option<ScreenRect> = None;
                for x in 0..w {
                    let px = &p.frame.data()[(y * w + x) * 4..][..3];
                    let i = (base + x as i64 * sx + y as i64 * sy) as usize * 3;
                    if want[i..i + 3] != *px {
                        want[i..i + 3].copy_from_slice(px);
                        grow(&mut row, ScreenRect::point(x as i32, y as i32));
                    }
                }
                if let Some(r) = row {
                    want_spans.push(r.physical());
                    grow(&mut b, r);
                }
            }
            assert!(fb == want, "rotation {rotation}: framebuffer");
            assert_eq!(spans, want_spans, "rotation {rotation}: spans");
            assert_eq!(got, b.map(ScreenRect::physical), "rotation {rotation}: box");
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

    /// The four layouts: the orientation, and a right-handed player.
    const LAYOUTS: [(crate::layout::Orientation, bool); 4] = [
        (crate::layout::Orientation::Portrait, false),
        (crate::layout::Orientation::Portrait, true),
        (crate::layout::Orientation::Landscape, false),
        (crate::layout::Orientation::Landscape, true),
    ];

    /// Runs `test` in each layout. The layout is a value of each thread.
    fn in_each_layout(mut test: impl FnMut(&str)) {
        for (orientation, right_handed) in LAYOUTS {
            crate::layout::set_orientation(orientation);
            crate::layout::set_right_handed(right_handed);
            test(&format!("{orientation:?}, right-handed {right_handed}"));
        }
        crate::layout::set_orientation(crate::layout::Orientation::Portrait);
        crate::layout::set_right_handed(false);
    }

    /// The app and the presenter, with the steps of the main loop. The test gives the time.
    struct Rig {
        fonts: Fonts,
        app: App,
        p: Presenter,
        fb: Vec<u8>,
        now: u64,
        /// The changes of the pen region that the platform gets: the time and the region.
        regions: Vec<(u64, Option<Update>)>,
    }

    impl Rig {
        /// The game of seed 7 with the first drawing on the screen, and no update in progress.
        fn new(use_pen_region: bool) -> Rig {
            let mut p = Presenter::new();
            p.use_pen_region = use_pen_region;
            let fb = vec![0u8; (crate::layout::physical_w() * crate::layout::physical_h() * 3) as usize];
            let mut r = Rig { fonts: Fonts::repo().unwrap(), app: game_app(), p, fb, now: 1000, regions: Vec::new() };
            r.step(None);
            r.wait(600);
            r
        }

        /// One pass of the main loop at the current time. Returns the updates of the screen.
        fn step(&mut self, input: Option<crate::app::Input>) -> Vec<Update> {
            if let Some(i) = input {
                self.app.handle(i, self.now);
            }
            self.app.tick(self.now);
            let mut sent = Vec::new();
            self.p.present(&mut self.app, &self.fonts, &mut self.fb, self.now, |u| sent.push(u));
            if let Some(region) = self.p.take_region_change() {
                self.regions.push((self.now, region));
            }
            sent
        }

        fn after(&mut self, ms: u64, input: Option<crate::app::Input>) -> Vec<Update> {
            self.now += ms;
            self.step(input)
        }

        /// The time passes in steps of 15 ms. Returns the updates with their times.
        fn wait(&mut self, ms: u64) -> Vec<(u64, Update)> {
            let end = self.now + ms;
            let mut log = Vec::new();
            while self.now < end {
                let sent = self.after(15, None);
                log.extend(sent.into_iter().map(|u| (self.now, u)));
            }
            log
        }

        fn down(&mut self, (x, y): (f32, f32)) -> Vec<Update> {
            self.after(10, Some(crate::app::Input::PenDown { x, y, tool: crate::app::Tool::Pen }))
        }

        fn to(&mut self, (x, y): (f32, f32)) -> Vec<Update> {
            self.after(10, Some(crate::app::Input::PenMove { x, y }))
        }

        fn up(&mut self, (x, y): (f32, f32)) -> Vec<Update> {
            self.after(10, Some(crate::app::Input::PenUp { x, y }))
        }

        /// A vertical stroke (a 1) in the middle of the cell. Returns the updates up to the pen lift.
        fn write(&mut self, cell: crate::game::Cell) -> Vec<Update> {
            let points = stroke_in(cell);
            let mut sent = self.down(points[0]);
            for &q in &points[1..] {
                sent.extend(self.to(q));
            }
            sent.extend(self.up(*points.last().unwrap()));
            sent
        }

        /// A pen tap in the middle of the cell. Returns the updates up to the pen lift, and the updates
        /// of the step of the pen lift.
        fn tap(&mut self, cell: crate::game::Cell) -> (Vec<Update>, Vec<Update>) {
            let (x, y) = middle(cell);
            let mut before = self.down((x, y));
            before.extend(self.to((x + 1.0, y)));
            (before, self.up((x + 1.0, y)))
        }

        fn empty_cells(&self) -> Vec<crate::game::Cell> {
            crate::game::cells().filter(|&c| !self.app.game.is_filled(c)).collect()
        }

        fn filled_cell(&self) -> crate::game::Cell {
            crate::game::cells().find(|&c| self.app.game.is_filled(c)).unwrap()
        }

        fn cells_with(&self, digit: u8) -> Vec<crate::game::Cell> {
            crate::game::cells().filter(|&c| self.app.game.value_at(c) == digit).collect()
        }

        /// Two digits that the board shows.
        fn two_digits(&self) -> (u8, u8) {
            let d: Vec<u8> = (1..=9).filter(|&d| !self.cells_with(d).is_empty()).take(2).collect();
            (d[0], d[1])
        }

        /// A pen tap makes the highlight of `digit`, and its update goes to the panel.
        fn make_highlight(&mut self, digit: u8) {
            let cell = self.cells_with(digit)[0];
            self.tap(cell);
            self.wait(600);
            assert_eq!(self.app.ui.highlight, Some(digit));
            self.assert_highlight(digit, true);
        }

        /// The background of a cell in the framebuffer: a point without lines, digits or marks.
        fn background(&self, cell: crate::game::Cell) -> [u8; 3] {
            let (x, y) = crate::layout::cell_origin(cell);
            let (base, sx, sy) = crate::layout::physical_steps();
            let i = (base + (x + 8) as i64 * sx + (y + 140) as i64 * sy) as usize * 3;
            [self.fb[i], self.fb[i + 1], self.fb[i + 2]]
        }

        /// All cells with `digit` show the highlight color in the framebuffer, or none shows it.
        fn assert_highlight(&self, digit: u8, shows: bool) {
            let color = self.app.settings.color_highlight.rgb();
            let cells = self.cells_with(digit);
            assert!(!cells.is_empty());
            for cell in cells {
                assert_eq!(self.background(cell) == color, shows, "the cell {cell:?} of the digit {digit}");
            }
        }

        fn cell_pixels_of(&self, fb: &[u8], cell: crate::game::Cell) -> Vec<u8> {
            let (ox, oy) = crate::layout::cell_origin(cell);
            let (base, sx, sy) = crate::layout::physical_steps();
            let mut out = Vec::new();
            for y in oy..oy + crate::layout::cell_size() {
                for x in ox..ox + crate::layout::cell_size() {
                    let i = (base + x as i64 * sx + y as i64 * sy) as usize * 3;
                    out.extend_from_slice(&fb[i..i + 3]);
                }
            }
            out
        }

        /// The pixels of a cell in the framebuffer.
        fn cell_pixels(&self, cell: crate::game::Cell) -> Vec<u8> {
            self.cell_pixels_of(&self.fb, cell)
        }

        /// The pixels of a cell in the frame (not the framebuffer).
        fn frame_cell(&self, cell: crate::game::Cell) -> Vec<u8> {
            let (ox, oy) = crate::layout::cell_origin(cell);
            let w = self.p.frame.width() as usize;
            let mut out = Vec::new();
            for y in oy as usize..(oy + crate::layout::cell_size()) as usize {
                for x in ox as usize..(ox + crate::layout::cell_size()) as usize {
                    out.extend_from_slice(&self.p.frame.data()[(y * w + x) * 4..][..3]);
                }
            }
            out
        }

        fn shows_the_frame(&self) -> bool {
            let mut copy = self.fb.clone();
            blit(&self.p.frame, &mut copy).is_none()
        }
    }

    /// The middle of a cell: the middle micro-cell.
    fn middle(cell: crate::game::Cell) -> (f32, f32) {
        let (x, y) = crate::layout::cell_origin(cell);
        let c = crate::layout::cell_size() as f32;
        (x as f32 + c / 2.0, y as f32 + c / 2.0)
    }

    /// The points of a vertical stroke in a cell, from the top micro-cell to the bottom micro-cell.
    fn stroke_in(cell: crate::game::Cell) -> Vec<(f32, f32)> {
        let (x, y) = crate::layout::cell_origin(cell);
        let c = crate::layout::cell_size() as f32;
        (0..=20).map(|k| (x as f32 + c / 2.0, y as f32 + c * (0.2 + 0.6 * k as f32 / 20.0))).collect()
    }

    /// The pen region for the ink of `cell`, as the platform gets it: the inside of the cell, on the
    /// 4 px grid of the panel, in the coordinates of the window of AppLoad (R§14.4).
    fn region_of(cell: crate::game::Cell) -> Update {
        let (x, y) = crate::layout::cell_origin(cell);
        let c = crate::layout::cell_size();
        ScreenRect::new(x, y, x + c - 1, y + c - 1).physical().inner_cells().to_window()
    }

    /// True if the update contains the area of the cell that its background color fills.
    fn covers(u: Update, cell: crate::game::Cell) -> bool {
        let (x, y) = crate::layout::cell_origin(cell);
        let c = crate::layout::cell_size();
        let r = ScreenRect::new(x + 8, y + 8, x + c - 9, y + c - 9).physical();
        let u = PhysicalRect::from_update(u);
        u.x0 <= r.x0 && u.y0 <= r.y0 && u.x1 >= r.x1 && u.y1 >= r.y1
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
        let (w, h) = (crate::layout::physical_w() as i32, crate::layout::physical_h() as i32);
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
        let mut fb = vec![0u8; (crate::layout::physical_w() * crate::layout::physical_h() * 3) as usize];
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
                let r = p.pen_region().expect("the pen region is on while the pen writes");
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
            let c = crate::layout::cell_size() as f32;
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
            let c = crate::layout::cell_size();
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

    /// INP-5, INP-16, INP-21: the player writes a number (pen region on). The pause and the idle time
    /// are equal (1 s from the pen lift): at that time the app reads the ink, and the redraw goes when
    /// the patch stopped the region. A stroke in a different cell before that time reads the first
    /// ink, and the redraw waits for the idle time of the new stroke. A finger press does not stop the
    /// writing. A pen press that does not write stops it at once.
    #[test]
    fn a_redraw_waits_until_the_player_has_not_written_for_the_idle_time() {
        use crate::app::{Input, Tool, RECOGNITION_PAUSE_MS, WRITING_IDLE_MS};
        assert_eq!(WRITING_IDLE_MS, RECOGNITION_PAUSE_MS, "INP-16: the idle time is equal to the pause");
        let mut r = Rig::new(true);
        let empty = r.empty_cells();
        let (a, b, c) = (empty[0], empty[1], empty[2]);

        r.write(a);
        let lift = r.now;
        let shown = r.fb.clone();
        // No read and no redraw before the end of the pause.
        while r.now + 30 < lift + WRITING_IDLE_MS {
            r.after(30, None);
            assert!(r.app.ui.pending_ink.is_some(), "{}: the ink is read before the pause ends", r.now - lift);
            assert!(r.app.is_writing(), "{}: the writing stops before the idle time ends", r.now - lift);
        }
        assert_eq!(r.app.game.value_at(a), 0);
        assert_eq!(r.app.next_deadline(), Some(lift + WRITING_IDLE_MS));
        assert!(r.cell_pixels_of(&shown, b) == r.cell_pixels(b), "a redraw changes the next cell");

        // A second number reads the first ink at the press, and starts the idle time again.
        r.write(b);
        let lift = r.now;
        assert_ne!(r.app.game.value_at(a), 0, "the press in a different cell does not read the ink");
        assert!(r.app.ui.pending_ink.is_some());
        while r.now + 30 < lift + WRITING_IDLE_MS {
            r.after(30, None);
            assert!(r.app.is_writing() && r.p.deferred_blit, "{}: the redraw went before the idle time", r.now - lift);
        }
        assert_eq!(r.app.next_deadline(), Some(lift + WRITING_IDLE_MS));
        // The idle time is over: the deadline of the app wakes the loop, the app reads the ink at that
        // time and the region stops. The redraw goes after the patch applied that change.
        r.now = lift + WRITING_IDLE_MS;
        r.step(None);
        assert!(r.app.ui.pending_ink.is_none(), "the ink is not read at the end of the pause");
        assert_ne!(r.app.game.value_at(b), 0);
        assert!(!r.app.is_writing() && r.p.pen_region().is_none());
        assert!(r.p.deferred_blit, "the redraw went before the region stopped");
        r.after(PEN_REGION_SETTLE_MS, None);
        assert!(!r.p.deferred_blit, "the redraw did not go after the idle time");
        assert!(r.shows_the_frame(), "the framebuffer does not show the frame");

        // A finger press does not stop the writing (INP-21). Its time is after the time of a hand touch (INP-20).
        r.write(c);
        assert!(r.app.is_writing());
        r.after(crate::app::HAND_TOUCH_MS, Some(Input::TouchDown { id: 1, x: 5.0, y: 5.0 }));
        assert!(r.app.is_writing() && r.app.ui.pending_ink.is_some(), "a finger press stops the writing");
        r.after(10, Some(Input::TouchUp { id: 1, x: 5.0, y: 5.0 }));
        assert!(r.app.is_writing() && r.app.ui.pending_ink.is_some(), "a finger release off a control stops the writing");
        // A pen press that does not write (here a press off the board) stops the writing at once.
        r.after(10, Some(Input::PenDown { x: 5.0, y: 5.0, tool: Tool::Pen }));
        assert!(!r.app.is_writing());
    }

    /// The point of the pen for a tap on the micro-cell of `digit` in `cell`: its center.
    fn micro_point(cell: crate::game::Cell, digit: u8) -> (f32, f32) {
        let (x, y) = crate::layout::cell_origin(cell);
        let m = crate::layout::cell_size() as f32 / 3.0;
        (x as f32 + m * (((digit - 1) % 3) as f32 + 0.5), y as f32 + m * (((digit - 1) / 3) as f32 + 0.5))
    }

    /// True if the update is inside the micro-cell of `digit` in `cell` (with 4 px for the alignment
    /// of the panel).
    fn inside_micro(u: Update, cell: crate::game::Cell, digit: u8) -> bool {
        let (x, y) = crate::layout::cell_origin(cell);
        let m = crate::layout::cell_size() / 3;
        let (mx, my) = (x + m * ((digit - 1) % 3) as i32, y + m * ((digit - 1) / 3) as i32);
        let micro = ScreenRect::new(mx, my, mx + m, my + m).physical().expand(4);
        let u = PhysicalRect::from_update(u);
        u.x0 >= micro.x0 && u.y0 >= micro.y0 && u.x1 <= micro.x1 && u.y1 <= micro.y1
    }

    /// An empty cell and a digit that a peer has (a tap on it is refused, MRK-7), or that no peer
    /// has (a tap adds the mark).
    fn tap_target(r: &Rig, refused: bool) -> (crate::game::Cell, u8) {
        let g = &r.app.game;
        r.empty_cells()
            .into_iter()
            .find_map(|c| (1..=9u8).find(|&d| (g.peer_digits(c) & crate::game::mark_bit(d) != 0) == refused).map(|d| (c, d)))
            .unwrap()
    }

    /// INP-2, INP-24, MRK-10, MRK-13, MRK-14, INP-10: a tap starts the pen region at the pen down,
    /// also the first touch of a session. A tap that changes nothing (a mark that a peer hides) shows
    /// its ink, then the x at the pen lift, in the region. The region stays: a stop would refresh
    /// the full cell. After the time of the x, the screen is the same as before (the ink of the tap
    /// went away), and the end of the x goes in the region too, inside the micro-cell.
    #[test]
    fn a_tap_starts_the_pen_region_and_the_x_of_a_refused_tap_goes_in_it() {
        in_each_layout(|layout| {
            let mut r = Rig::new(true);
            let (cell, d) = tap_target(&r, true);
            let q = micro_point(cell, d);
            let before = r.fb.clone();
            r.regions.clear();
            r.down(q);
            let press = r.now;
            assert_eq!(r.regions, vec![(press, Some(region_of(cell)))], "{layout}: the region at the pen down");
            r.to((q.0 + 1.0, q.1));
            let ink = r.wait(PEN_REGION_SETTLE_MS + 30);
            assert!(!ink.is_empty() && r.fb != before, "{layout}: the ink of the tap does not show");
            assert!(ink.iter().all(|&(_, u)| inside_micro(u, cell, d)), "{layout}: the ink is not in the micro-cell: {ink:?}");
            let at_lift = r.up((q.0 + 1.0, q.1));
            let lift = r.now;
            assert_eq!(r.app.ui.refused, Some((cell, d, lift)), "{layout}");
            assert_eq!(at_lift.len(), 1, "{layout}: the x does not go at the pen lift: {at_lift:?}");
            assert!(inside_micro(at_lift[0], cell, d), "{layout}: {:?} is not in the micro-cell", at_lift[0]);
            assert!(r.shows_the_frame(), "{layout}: the framebuffer does not show the x");
            assert_eq!(r.regions, vec![(press, Some(region_of(cell)))], "{layout}: the region changes at the lift");
            assert!(!r.app.is_writing(), "{layout}: a tap starts the writing timer");
            assert_eq!(r.wait(crate::app::OVERLAY_MS - 60), vec![], "{layout}: an update while the x shows");
            // The end of the x: one update in the micro-cell, with the region on.
            r.now = lift + crate::app::OVERLAY_MS;
            let end = r.step(None);
            assert_eq!(r.app.ui.refused, None, "{layout}");
            assert_eq!(end.len(), 1, "{layout}: the end of the x: {end:?}");
            assert!(inside_micro(end[0], cell, d), "{layout}: {:?} is not in the micro-cell", end[0]);
            assert_eq!(r.app.game.marks_at(cell), 0, "{layout}");
            assert!(r.fb == before, "{layout}: the x or the ink of a tap that changes nothing stays on the screen");
            assert_eq!(r.regions, vec![(press, Some(region_of(cell)))], "{layout}: the region changes after the tap");
        });
    }

    /// INP-24, MRK-5: a tap that adds a mark. The region starts at the pen down and stays at the lift.
    /// The mark goes to the screen at the pen lift, with the region on (the pen waveform), in one
    /// update inside the micro-cell, and the ink of the tap goes away with it. A tap that is shorter
    /// than the time that the patch needs to apply the region: the mark waits for that time.
    #[test]
    fn a_mark_goes_to_the_screen_at_the_pen_lift_inside_the_pen_region() {
        in_each_layout(|layout| {
            let mut r = Rig::new(true);
            let (cell, d) = tap_target(&r, false);
            let q = micro_point(cell, d);
            let before = r.fb.clone();
            r.regions.clear();
            r.down(q);
            let press = r.now;
            assert_eq!(r.regions, vec![(press, Some(region_of(cell)))], "{layout}: the region at the pen down");
            r.to((q.0 + 1.0, q.1));
            r.wait(PEN_REGION_SETTLE_MS + 30);
            assert!(r.fb != before, "{layout}: the ink of the tap does not show");
            let at_lift = r.up((q.0 + 1.0, q.1));
            assert_ne!(r.app.game.marks_at(cell) & crate::game::mark_bit(d), 0, "{layout}");
            assert_eq!(at_lift.len(), 1, "{layout}: the mark does not go at the pen lift: {at_lift:?}");
            assert!(inside_micro(at_lift[0], cell, d), "{layout}: {:?} is not in the micro-cell", at_lift[0]);
            assert!(r.shows_the_frame(), "{layout}: the framebuffer does not show the mark");
            assert!(r.frame_cell(cell) != r.cell_pixels_of(&before, cell), "{layout}: the cell does not change");
            assert_eq!(r.wait(600), vec![], "{layout}: an update after the mark");
            assert_eq!(r.regions, vec![(press, Some(region_of(cell)))], "{layout}: the region changes at the lift");

            // A second tap removes the mark. The region is on the cell already: no change of the
            // region, and the mark goes at the lift.
            r.down(q);
            r.wait(PEN_REGION_SETTLE_MS + 30);
            let at_lift = r.up(q);
            assert_eq!(r.app.game.marks_at(cell) & crate::game::mark_bit(d), 0, "{layout}");
            assert_eq!(at_lift.len(), 1, "{layout}: the removal: {at_lift:?}");
            assert!(inside_micro(at_lift[0], cell, d), "{layout}: {:?} is not in the micro-cell", at_lift[0]);
            assert!(r.fb == before, "{layout}: the cell is not as before the two taps");
            assert_eq!(r.regions, vec![(press, Some(region_of(cell)))], "{layout}: the region changes");

            // A short tap in a different cell: the region moves at the pen down, and the mark waits
            // until the patch applied the region.
            let (other, d2) = r.empty_cells().into_iter().filter(|&c| c != cell).find_map(|c| (1..=9u8).find(|&d| r.app.game.peer_digits(c) & crate::game::mark_bit(d) == 0).map(|d| (c, d))).unwrap();
            let q2 = micro_point(other, d2);
            r.regions.clear();
            r.down(q2);
            let press = r.now;
            assert_eq!(r.regions, vec![(press, Some(region_of(other)))], "{layout}: the region moves at the pen down");
            let at_lift = r.up(q2);
            assert_eq!(at_lift, vec![], "{layout}: the mark goes before the patch applied the region");
            let mark = r.wait(PEN_REGION_SETTLE_MS);
            assert_eq!(mark.len(), 1, "{layout}: the mark: {mark:?}");
            assert!(mark[0].0 >= press + PEN_REGION_SETTLE_MS && inside_micro(mark[0].1, other, d2), "{layout}: {mark:?}");
            assert!(r.shows_the_frame(), "{layout}");
            assert_eq!(r.regions, vec![(press, Some(region_of(other)))], "{layout}: the region changes at the lift");
        });
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

    /// HLT-3, HLT-9: a tap on a number changes no pixel at the press. The removal or the move of the
    /// highlight goes in one update at the pen lift, and no other update follows. This is the same
    /// with the pen region and without it.
    #[test]
    fn a_tap_on_a_number_changes_the_highlight_in_one_update() {
        in_each_layout(|layout| {
            for use_region in [true, false] {
                // `same`: a tap on the digit of the highlight (it removes). Else, a tap on a different digit (it moves).
                for same in [false, true] {
                    let mut r = Rig::new(use_region);
                    let (v, w) = r.two_digits();
                    r.make_highlight(v);
                    let target = if same { v } else { w };
                    // A cell that is not the first cell of `v`: the cell of the tap does not matter.
                    let cell = *r.cells_with(target).last().unwrap();
                    let shown = r.fb.clone();
                    let (before, at_lift) = r.tap(cell);
                    let name = format!("{layout}, pen region {use_region}, same digit {same}");
                    assert_eq!(before, vec![], "{name}: an update before the pen lift");
                    assert_eq!(at_lift.len(), 1, "{name}: the updates of the pen lift: {at_lift:?}");
                    assert!(r.fb != shown, "{name}: the framebuffer does not change");
                    assert!(r.shows_the_frame(), "{name}: the framebuffer does not show the frame");
                    assert_eq!(r.app.ui.highlight, (!same).then_some(w), "{name}");
                    r.assert_highlight(v, false);
                    r.assert_highlight(w, !same);
                    // The one update contains the cells of the two digits.
                    for c in r.cells_with(v).into_iter().chain((!same).then(|| r.cells_with(w)).into_iter().flatten()) {
                        assert!(covers(at_lift[0], c), "{name}: {:?} does not contain the cell {c:?}", at_lift[0]);
                    }
                    assert_eq!(r.wait(600), vec![], "{name}: an update after the pen lift");
                }
            }
        });
    }

    /// HLT-2 item 2, HLT-9: a tap on an empty cell removes the highlight in one update with the mark, at
    /// the pen lift. The pixels of the highlight do not change before that.
    /// R§14.4, INP-24, MRK-5: the pen waveform removes only the marks that it drew. A mark that the
    /// UI waveform showed (here the area went to a different cell, for a number or for a mark) has
    /// colored dots of the color dither on the panel, and the pen waveform leaves a ghost of it. Thus
    /// a tap on it does not change the area (a start or a stop refreshes the full cell), and its
    /// removal goes at the pen lift with the UI waveform, only in the micro-cell. When the area came
    /// to the cell for a different mark, the removal of the old mark stops the area first. A mark
    /// that the pen waveform drew in the area goes away in the area.
    #[test]
    fn a_mark_that_the_ui_waveform_drew_goes_away_with_the_ui_waveform() {
        // `away`: a number (true) or a mark (false) in a different cell, between the two taps.
        for away in [true, false] {
            let mut r = Rig::new(true);
            let g = &r.app.game;
            let (a, d, d3) = r
                .empty_cells()
                .into_iter()
                .find_map(|c| {
                    let free: Vec<u8> = (1..=9u8).filter(|&x| g.peer_digits(c) & crate::game::mark_bit(x) == 0).collect();
                    (free.len() >= 2).then(|| (c, free[0], free[1]))
                })
                .unwrap();
            let q = micro_point(a, d);
            r.down(q);
            r.wait(PEN_REGION_SETTLE_MS + 30);
            r.up(q);
            r.wait(600);
            assert_ne!(r.app.game.marks_at(a) & crate::game::mark_bit(d), 0);
            let g = &r.app.game;
            let (b, d2) = r
                .empty_cells()
                .into_iter()
                .filter(|&c| c.0 != a.0 && c.1 != a.1)
                .find_map(|c| (1..=9u8).find(|&x| g.peer_digits(c) & crate::game::mark_bit(x) == 0).map(|x| (c, x)))
                .unwrap();
            if away {
                r.write(b);
                r.wait(2500);
                assert_eq!(r.app.game.value_at(b), 1, "the number in {b:?}");
            } else {
                let q2 = micro_point(b, d2);
                r.down(q2);
                r.wait(PEN_REGION_SETTLE_MS + 30);
                r.up(q2);
                r.wait(600);
                assert_ne!(r.app.game.marks_at(b) & crate::game::mark_bit(d2), 0);
            }
            let region = r.p.pen_cell;
            r.regions.clear();
            let before = r.fb.clone();
            r.down(q);
            r.to((q.0 + 1.0, q.1));
            let ink = r.wait(PEN_REGION_SETTLE_MS + 30);
            assert!(r.fb != before, "away {away}: the ink of the tap does not show");
            assert!(ink.iter().all(|&(_, u)| inside_micro(u, a, d)), "away {away}: the ink is not in the micro-cell: {ink:?}");
            let at_lift = r.up(q);
            assert_eq!(r.app.game.marks_at(a) & crate::game::mark_bit(d), 0, "away {away}");
            assert_eq!(at_lift.len(), 1, "away {away}: the removal does not go at the pen lift: {at_lift:?}");
            assert!(inside_micro(at_lift[0], a, d), "away {away}: {:?} is not in the micro-cell", at_lift[0]);
            assert_eq!(r.p.pen_cell, region, "away {away}: the region changes");
            assert_ne!(r.p.pen_cell, Some(a), "away {away}");
            assert!(r.shows_the_frame(), "away {away}");
            assert_eq!(r.regions, vec![], "away {away}: the region changes");

            // The mark again (the pen waveform draws it), then a different mark in the cell: the
            // area is on the cell. The removal of the mark of the pen goes in the area.
            let q3 = micro_point(a, d3);
            for p in [q, q3, q] {
                r.down(p);
                r.wait(PEN_REGION_SETTLE_MS + 30);
                r.up(p);
                r.wait(600);
            }
            assert_eq!(r.p.pen_cell, Some(a), "away {away}");
            assert_eq!(r.app.game.marks_at(a) & crate::game::mark_bit(d), 0, "away {away}");
            assert!(r.shows_the_frame(), "away {away}");

            // The area leaves the cell, and comes back for a new mark. The old mark (d3) has the UI
            // waveform on the panel: its removal stops the area first.
            let g = &r.app.game;
            let (c, d4) = r
                .empty_cells()
                .into_iter()
                .filter(|&c| c != a && c != b && g.marks_at(c) == 0)
                .find_map(|c| (1..=9u8).find(|&x| g.peer_digits(c) & crate::game::mark_bit(x) == 0).map(|x| (c, x)))
                .unwrap();
            r.down(micro_point(c, d4));
            r.wait(PEN_REGION_SETTLE_MS + 30);
            r.up(micro_point(c, d4));
            r.wait(600);
            assert_eq!(r.p.pen_cell, Some(c), "away {away}");
            r.down(q);
            r.wait(PEN_REGION_SETTLE_MS + 30);
            r.up(q);
            r.wait(600);
            assert_eq!(r.p.pen_cell, Some(a), "away {away}");
            let q3 = micro_point(a, d3);
            r.regions.clear();
            r.down(q3);
            r.wait(PEN_REGION_SETTLE_MS + 30);
            let at_lift = r.up(q3);
            let lift = r.now;
            assert_eq!(r.app.game.marks_at(a) & crate::game::mark_bit(d3), 0, "away {away}");
            assert_eq!(at_lift, vec![], "away {away}: the removal goes with the region on");
            assert_eq!(r.regions, vec![(lift, None)], "away {away}: the region does not stop");
            let removal = r.wait(PEN_REGION_SETTLE_MS);
            assert_eq!(removal.len(), 1, "away {away}: the removal: {removal:?}");
            assert!(r.shows_the_frame(), "away {away}");
        }
    }

    #[test]
    fn a_tap_on_an_empty_cell_removes_the_highlight_in_one_update_with_the_mark() {
        in_each_layout(|layout| {
            let mut r = Rig::new(true);
            let (v, _) = r.two_digits();
            r.make_highlight(v);
            let g = &r.app.game;
            let (cell, d) = r.empty_cells().into_iter().find_map(|c| (1..=9u8).find(|&d| g.peer_digits(c) & crate::game::mark_bit(d) == 0).map(|d| (c, d))).unwrap();
            let (ox, oy) = crate::layout::cell_origin(cell);
            let m = crate::layout::cell_size() as f32 / 3.0;
            let q = (ox as f32 + m * (((d - 1) % 3) as f32 + 0.5), oy as f32 + m * (((d - 1) / 3) as f32 + 0.5));
            r.down(q);
            r.to((q.0 + 1.0, q.1));
            // The region starts at the pen down: the mark waits until the patch applied it.
            r.wait(PEN_REGION_SETTLE_MS + 30);
            r.assert_highlight(v, true);
            let at_lift = r.up((q.0 + 1.0, q.1));
            assert_eq!(r.app.ui.highlight, None, "{layout}");
            assert_ne!(r.app.game.marks_at(cell) & crate::game::mark_bit(d), 0, "{layout}: no mark");
            assert_eq!(at_lift.len(), 1, "{layout}: the updates of the pen lift: {at_lift:?}");
            r.assert_highlight(v, false);
            assert!(r.shows_the_frame(), "{layout}: the mark is not in the framebuffer");
            for c in r.cells_with(v) {
                assert!(covers(at_lift[0], c), "{layout}: {:?} does not contain the cell {c:?}", at_lift[0]);
            }
        });
    }

    /// HLT-2 item 4, HLT-9: while the player writes, the highlight stays on the screen and only the ink
    /// changes. The read gives one update, with the removal of the highlight and the number together.
    #[test]
    fn writing_while_a_highlight_shows_changes_the_highlight_and_the_number_in_one_update() {
        let check = |r: &mut Rig, name: &str| {
            let (v, _) = r.two_digits();
            r.make_highlight(v);
            let a = r.empty_cells()[0];
            let (ox, oy) = crate::layout::cell_origin(a);
            let c = crate::layout::cell_size();
            let ink_cell = ScreenRect::new(ox, oy, ox + c - 1, oy + c - 1).physical().expand(4);
            let ink = r.write(a);
            let lift = r.now;
            assert!(!ink.is_empty(), "{name}: the ink does not show");
            assert_eq!(r.app.ui.highlight, Some(v), "{name}: writing changes the highlight before the read");
            r.assert_highlight(v, true);
            let mut before_read = r.wait(crate::app::RECOGNITION_PAUSE_MS - 60);
            before_read.extend(ink.iter().map(|&u| (lift, u)));
            for (t, u) in before_read {
                let u = PhysicalRect::from_update(u);
                assert!(u.x0 >= ink_cell.x0 && u.y0 >= ink_cell.y0 && u.x1 <= ink_cell.x1 && u.y1 <= ink_cell.y1, "{name}: at {t}, {u:?} is not ink in the cell");
            }
            r.assert_highlight(v, true);
            assert_eq!(r.app.game.value_at(a), 0, "{name}");
            let ink_on_screen = r.cell_pixels(a);

            // The read, at the exact time of the app, then the time of the redraw.
            r.now = lift + crate::app::RECOGNITION_PAUSE_MS;
            let mut log: Vec<(u64, Update)> = r.step(None).into_iter().map(|u| (r.now, u)).collect();
            assert_eq!(r.app.ui.highlight, None, "{name}: the read does not remove the highlight");
            let d = r.app.game.value_at(a);
            assert_ne!(d, 0, "{name}: the read gives no number");
            log.extend(r.wait(600));
            assert_eq!(log.len(), 1, "{name}: the updates after the read: {log:?}");
            let u = log[0].1;
            assert!(log[0].0 - lift >= crate::app::RECOGNITION_PAUSE_MS, "{name}");
            // The highlight and the number show in the same update.
            assert!(r.shows_the_frame(), "{name}: the framebuffer does not show the frame");
            r.assert_highlight(v, false);
            assert!(r.cell_pixels(a) != ink_on_screen, "{name}: the number does not replace the ink");
            assert!(covers(u, a), "{name}: {u:?} does not contain the cell of the number");
            for c in r.cells_with(v) {
                assert!(covers(u, c), "{name}: {u:?} does not contain the cell {c:?}");
            }
        };
        in_each_layout(|layout| check(&mut Rig::new(true), layout));
        check(&mut Rig::new(false), "with no pen region");
    }

    /// HLT-3, HLT-9: a tap on a number while ink waits in a different cell reads the ink at the press.
    /// The read and the result of the lift go in one update, and the tap compares with the highlight
    /// at the press: a tap on v gives no highlight, and a tap on w gives w.
    #[test]
    fn a_tap_on_a_number_while_ink_waits_gives_one_update_with_the_read() {
        in_each_layout(|layout| {
            for same in [false, true] {
                let name = format!("{layout}, same digit {same}");
                let mut r = Rig::new(true);
                let (v, w) = r.two_digits();
                r.make_highlight(v);
                let a = r.empty_cells()[0];
                r.write(a);
                r.wait(300);
                let target = if same { v } else { w };
                let cell = r.cells_with(target)[0];
                let (x, y) = middle(cell);
                let shown = r.fb.clone();
                let mut log = r.down((x, y));
                assert_ne!(r.app.game.value_at(a), 0, "{name}: the press does not read the ink");
                assert_eq!(r.app.ui.highlight, None, "{name}: the read does not remove the highlight");
                assert_eq!(r.p.pen_region(), None, "{name}: a press on a filled cell keeps the pen region");
                // The pen stays down after the region stopped: the read must still wait for the lift.
                log.extend(r.wait(PEN_REGION_SETTLE_MS + 30).into_iter().map(|(_, u)| u));
                log.extend(r.to((x + 1.0, y)));
                assert!(r.fb == shown && log.is_empty(), "{name}: an update before the pen lift: {log:?}");
                log.extend(r.up((x + 1.0, y)));
                assert_eq!(r.app.ui.highlight, (!same).then_some(w), "{name}");
                log.extend(r.wait(600).into_iter().map(|(_, u)| u));
                assert_eq!(log.len(), 1, "{name}: the updates of the read and of the tap: {log:?}");
                assert!(r.shows_the_frame(), "{name}");
                r.assert_highlight(v, false);
                r.assert_highlight(w, !same);
                for c in r.cells_with(v).into_iter().chain(std::iter::once(a)) {
                    assert!(covers(log[0], c), "{name}: {:?} does not contain the cell {c:?}", log[0]);
                }
            }
        });
    }

    /// R§14.4: the platform gives the pen region in the coordinates of the window of AppLoad, which
    /// are the coordinates of the screen as the player sees it. In each rotation of the tablet, the
    /// region is the inside of the cell of the pen, on the 4 px grid of the panel. Before, the region
    /// was in physical coordinates: in landscape the pen area was on a different cell, and the ink
    /// got the UI waveform (choppy ink with gaps).
    #[test]
    fn the_pen_region_is_in_the_coordinates_of_the_window_in_each_rotation() {
        for rotation in 0..4 {
            crate::layout::set_rotation(rotation);
            let mut r = Rig::new(true);
            let cell = r.empty_cells()[0];
            r.regions.clear();
            r.down(stroke_in(cell)[0]);
            let Some(&(_, Some((x, y, w, h)))) = r.regions.last() else {
                panic!("rotation {rotation}: no pen region at the pen down: {:?}", r.regions);
            };
            let (cx, cy) = crate::layout::cell_origin(cell);
            let c = crate::layout::cell_size();
            // The inside of the cell on the screen: at most 3 px from each edge of the cell.
            assert!(
                (cx..cx + 4).contains(&x) && (cy..cy + 4).contains(&y) && (c - 7..=c).contains(&w) && (c - 7..=c).contains(&h),
                "rotation {rotation}: the region ({x}, {y}, {w}, {h}) is not the inside of the cell at ({cx}, {cy})"
            );
            // The same pixels in the framebuffer are on the 4 px grid of the panel.
            let p = PhysicalRect::from_update(crate::layout::physical_rect((x, y, x + w - 1, y + h - 1)));
            assert_eq!(p, p.inner_cells(), "rotation {rotation}: the region is not on the 4 px grid");
            assert_eq!(Some(p), r.p.pen_region(), "rotation {rotation}");
        }
        crate::layout::set_rotation(0);
    }

    /// INP-24: while ink waits, each pen press on an empty cell starts the region at the pen down, on
    /// the cell of the press, with no stop between the cells. The ink goes only in that region.
    #[test]
    fn while_ink_waits_a_pen_press_starts_the_region_at_the_pen_down_with_no_stop() {
        in_each_layout(|layout| {
            let mut r = Rig::new(true);
            let empty = r.empty_cells();
            let (a, b, c) = (empty[0], empty[1], empty[2]);
            r.write(a);
            r.regions.clear();
            r.wait(100);
            assert_eq!(r.regions, vec![], "{layout}: the region stops after the pen lift");
            for cell in [b, c] {
                let points = stroke_in(cell);
                let sent = r.down(points[0]);
                let press = r.now;
                assert_eq!(r.regions, vec![(press, Some(region_of(cell)))], "{layout}: the region at the pen down in {cell:?}");
                assert_eq!(r.app.writing_stroke().map(|s| s.len()), Some(1), "{layout}: the first point starts the region");
                assert_eq!(sent, vec![], "{layout}: an update at the pen down");
                r.regions.clear();
                let region = cell_region(cell);
                for &q in &points[1..] {
                    for u in r.to(q) {
                        let u = PhysicalRect::from_update(u);
                        assert!(u.x0 >= region.x0 && u.y0 >= region.y0 && u.x1 <= region.x1 + 1 && u.y1 <= region.y1 + 1, "{layout}: {u:?} is not in {region:?}");
                    }
                }
                r.up(*points.last().unwrap());
                r.wait(100);
                assert_eq!(r.regions, vec![], "{layout}: the region changes during the stroke or at the lift");
            }
            // A new stroke in the cell of the waiting ink keeps the region.
            let points = stroke_in(c);
            r.down(points[0]);
            assert_eq!(r.regions, vec![], "{layout}: a stroke in the same cell changes the region");
            assert_eq!(r.p.pen_region().map(PhysicalRect::to_window), Some(region_of(c)), "{layout}");
        });
    }

    /// INP-24: "the player writes" also means that the writing timer runs with no waiting ink: a stroke
    /// from a filled cell starts it. A finger press does not stop it (INP-21), thus the next pen press
    /// starts the region at the pen down. A hand touch also has no effect (INP-20).
    #[test]
    fn while_the_writing_timer_runs_a_pen_press_starts_the_region_at_the_pen_down() {
        in_each_layout(|layout| {
            let mut r = Rig::new(true);
            let filled = r.filled_cell();
            let a = r.empty_cells()[0];
            let points = stroke_in(filled);
            r.down(points[0]);
            for &q in &points[1..] {
                r.to(q);
            }
            r.up(*points.last().unwrap());
            assert!(r.app.ui.pending_ink.is_none(), "{layout}");
            assert_eq!(r.regions, vec![], "{layout}: a stroke from a filled cell starts the region");
            // A hand touch, then a finger, on the board. They do not stop the timer.
            r.after(10, Some(crate::app::Input::TouchDown { id: 1, x: 5.0, y: 5.0 }));
            r.after(10, Some(crate::app::Input::TouchUp { id: 1, x: 5.0, y: 5.0 }));
            let (x, y) = middle(a);
            r.after(crate::app::HAND_TOUCH_MS, Some(crate::app::Input::TouchDown { id: 2, x, y }));
            r.after(10, Some(crate::app::Input::TouchUp { id: 2, x, y }));
            r.down((x, y));
            assert_eq!(r.regions, vec![(r.now, Some(region_of(a)))], "{layout}: the region at the pen down");
        });
    }

    /// INP-24, HLT-9, INP-12: a tap while ink waits in a different cell. The press reads the ink and
    /// moves the region to the cell of the tap at the pen down. At the lift, one update shows the
    /// number of the first cell (out of the region) and the mark (in the region). The region stays
    /// on the cell of the tap, and the next press on an empty cell moves it at the pen down.
    #[test]
    fn a_tap_while_ink_waits_reads_the_ink_and_keeps_the_region_on_the_tapped_cell() {
        in_each_layout(|layout| {
            let mut r = Rig::new(true);
            let empty = r.empty_cells();
            let (a, c) = (empty[0], *empty.last().unwrap());
            r.write(a);
            let (b, d) = r.empty_cells().into_iter().filter(|&x| x != a && x != c).find_map(|x| (1..=9u8).find(|&d| r.app.game.peer_digits(x) & crate::game::mark_bit(d) == 0).map(|d| (x, d))).unwrap();
            let q = micro_point(b, d);
            r.regions.clear();
            r.down(q);
            let press = r.now;
            assert_ne!(r.app.game.value_at(a), 0, "{layout}: the press does not read the ink");
            assert_eq!(r.regions, vec![(press, Some(region_of(b)))], "{layout}: the region does not move at the pen down");
            r.wait(PEN_REGION_SETTLE_MS + 30);
            let at_lift = r.up(q);
            let lift = r.now;
            assert_ne!(r.app.game.marks_at(b) & crate::game::mark_bit(d), 0, "{layout}: the state changes at the pen lift");
            assert!(!r.app.is_writing(), "{layout}: a tap starts the writing timer");
            assert_eq!(at_lift.len(), 1, "{layout}: the updates of the pen lift: {at_lift:?}");
            assert!(covers(at_lift[0], a), "{layout}: {:?} does not contain the number", at_lift[0]);
            assert!(r.shows_the_frame(), "{layout}: the framebuffer does not show the number and the mark");
            assert_eq!(r.regions, vec![(press, Some(region_of(b)))], "{layout}: the region stops at the pen lift");
            assert_eq!(r.wait(600), vec![], "{layout}: an update after the lift");
            assert_eq!(r.regions, vec![(press, Some(region_of(b)))], "{layout}: the region stops after the tap");
            // The next press on an empty cell moves the region at the pen down.
            r.regions.clear();
            let points = stroke_in(c);
            r.down(points[0]);
            assert_eq!(r.regions, vec![(r.now, Some(region_of(c)))], "{layout}: the press after a tap does not move the region");
            let _ = lift;
        });
    }

    /// INP-2, INP-24: the first stroke of a session starts the region at the pen down, also while the
    /// pen is still in the micro-cell. At the end of the pause the app reads the number: the region
    /// stops before the redraw of the number, and the redraw goes when the patch applied the stop.
    #[test]
    fn the_first_stroke_of_a_session_starts_the_region_at_the_pen_down() {
        in_each_layout(|layout| {
            let mut r = Rig::new(true);
            let empty = r.empty_cells();
            let m = crate::layout::cell_size() as f32 / 3.0;
            for cell in [empty[0], empty[1]] {
                r.regions.clear();
                let (x, y) = middle(cell);
                r.down((x, y));
                let press = r.now;
                assert_eq!(r.regions, vec![(press, Some(region_of(cell)))], "{layout}: {cell:?}: the region does not start at the pen down");
                assert_eq!(r.app.writing_stroke().map(|s| s.len()), Some(1), "{layout}");
                for k in 1..=3 {
                    r.to((x + k as f32, y));
                }
                r.to((x, y + m));
                r.up((x, y + m));
                let lift = r.now;
                assert_eq!(r.regions, vec![(press, Some(region_of(cell)))], "{layout}: {cell:?}: the region changes during the stroke");
                // The read at the end of the pause: the region stops, then the number goes.
                r.now = lift + crate::app::RECOGNITION_PAUSE_MS;
                let at_read = r.step(None);
                assert_ne!(r.app.game.value_at(cell), 0, "{layout}: {cell:?}: no number");
                assert_eq!(at_read, vec![], "{layout}: the number goes before the region stopped");
                assert_eq!(r.regions, vec![(press, Some(region_of(cell))), (r.now, None)], "{layout}: the region does not stop for the number");
                let number = r.wait(PEN_REGION_SETTLE_MS + 15);
                assert_eq!(number.len(), 1, "{layout}: the number: {number:?}");
                assert!(number[0].0 >= lift + crate::app::RECOGNITION_PAUSE_MS + PEN_REGION_SETTLE_MS && covers(number[0].1, cell), "{layout}: {number:?}");
                assert!(r.shows_the_frame() && !r.app.is_writing(), "{layout}");
            }
        });
    }

    /// R§14.4: after a tap, the region stays on its cell. A redraw that changes only pixels out of
    /// the cell (a panel that does not cover it) goes with the region on. A redraw that covers the
    /// cell with a panel needs the UI waveform in the cell: the region stops first, and the redraw
    /// goes when the patch applied the stop.
    #[test]
    fn the_region_stops_only_before_a_redraw_that_needs_the_ui_waveform_in_its_cell() {
        in_each_layout(|layout| {
            let panel = crate::layout::colors_panel();
            let under = |cell: crate::game::Cell| {
                let (x, y) = crate::layout::cell_origin(cell);
                panel.contains(x as f32 + 10.0, y as f32 + 10.0)
            };
            for covered in [false, true] {
                let name = format!("{layout}, the panel covers the cell: {covered}");
                let mut r = Rig::new(true);
                r.app.ui.menu = crate::app::Menu::Open(None);
                r.app.redraw = true;
                r.wait(600);
                let Some((cell, d)) = r.empty_cells().into_iter().filter(|&c| under(c) == covered).find_map(|c| (1..=9u8).find(|&d| r.app.game.peer_digits(c) & crate::game::mark_bit(d) == 0).map(|d| (c, d))) else {
                    continue;
                };
                let q = micro_point(cell, d);
                r.regions.clear();
                r.down(q);
                let press = r.now;
                r.wait(PEN_REGION_SETTLE_MS + 30);
                r.up(q);
                r.wait(100);
                assert_eq!(r.regions, vec![(press, Some(region_of(cell)))], "{name}: the region after the tap");
                // A finger opens the Cell colors panel.
                let (bx, by) = (0..crate::layout::screen_h() as i32)
                    .step_by(8)
                    .flat_map(|y| (0..crate::layout::screen_w() as i32).step_by(8).map(move |x| (x as f32, y as f32)))
                    .find(|&(x, y)| crate::layout::game_button(x, y, true) == Some(crate::layout::Button::Colors))
                    .unwrap();
                r.regions.clear();
                r.after(crate::app::HAND_TOUCH_MS, Some(crate::app::Input::TouchDown { id: 1, x: bx, y: by }));
                let at_release = r.after(10, Some(crate::app::Input::TouchUp { id: 1, x: bx, y: by }));
                let release = r.now;
                assert_eq!(r.app.ui.menu.panel(), Some(crate::app::Panel::Colors), "{name}");
                if covered {
                    assert_eq!(r.regions, vec![(release, None)], "{name}: the region does not stop before the panel");
                    assert_eq!(at_release, vec![], "{name}: the panel goes before the region stopped");
                    let panel = r.wait(PEN_REGION_SETTLE_MS + 15);
                    assert_eq!(panel.len(), 1, "{name}: the panel: {panel:?}");
                    assert!(panel[0].0 >= release + PEN_REGION_SETTLE_MS, "{name}: {panel:?}");
                } else {
                    assert_eq!(r.regions, vec![], "{name}: the region stops for a panel that does not cover its cell");
                    assert_eq!(at_release.len(), 1, "{name}: the panel does not go at the release: {at_release:?}");
                }
                assert!(r.shows_the_frame(), "{name}: the framebuffer does not show the panel");
            }
        });
    }

    /// NAV-13: a turn of the tablet stops the region: its cell is at a different place now.
    #[test]
    fn a_turn_of_the_tablet_stops_the_region() {
        let mut r = Rig::new(true);
        let (cell, d) = tap_target(&r, false);
        let q = micro_point(cell, d);
        r.down(q);
        r.wait(PEN_REGION_SETTLE_MS + 30);
        r.up(q);
        r.wait(100);
        assert!(r.p.pen_region().is_some());
        r.regions.clear();
        crate::layout::set_orientation(crate::layout::Orientation::Landscape);
        r.app.rotate(App::placeholder_ng_stops());
        r.now += 10;
        r.p.reset(r.now);
        let turn = r.now;
        let at_turn = r.step(None);
        assert_eq!(r.regions, vec![(turn, None)], "the region does not stop at the turn");
        assert_eq!(at_turn, vec![], "the redraw goes before the region stopped");
        let redraw = r.wait(PEN_REGION_SETTLE_MS + 15);
        assert!(!redraw.is_empty() && r.shows_the_frame(), "the screen is not drawn again after the turn: {redraw:?}");
        crate::layout::set_orientation(crate::layout::Orientation::Portrait);
    }

    /// The steps of a pen touch that starts on a filled cell, and `redraw` asks for a full drawing at
    /// each step. No step draws ink or starts the pen region (INP-22).
    fn assert_no_ink_and_no_region(r: &mut Rig, points: &[(f32, f32)], redraw: bool, name: &str) {
        let frame = r.p.frame.data().to_vec();
        let shown = r.fb.clone();
        r.regions.clear();
        let check = |r: &mut Rig, sent: Vec<Update>, at: &str| {
            assert_eq!(r.regions, vec![], "{name}: {at}: the pen region starts");
            assert_eq!(r.p.pen_region(), None, "{name}: {at}");
            assert_eq!(r.app.writing_stroke(), None, "{name}: {at}");
            assert_eq!(r.app.live_stroke, None, "{name}: {at}: a live stroke");
            assert_eq!(sent, vec![], "{name}: {at}: an update");
            assert!(r.p.frame.data() == &frame[..], "{name}: {at}: the frame changes");
            assert!(r.fb == shown, "{name}: {at}: the framebuffer changes");
        };
        let sent = r.down(points[0]);
        check(r, sent, "the pen down");
        for &q in &points[1..] {
            if redraw {
                r.app.redraw = true;
            }
            let sent = r.to(q);
            check(r, sent, "a pen move");
            if redraw {
                // A step with no input, as at a timer of the main loop.
                r.app.redraw = true;
                let sent = r.after(1, None);
                check(r, sent, "a full drawing");
            }
        }
    }

    /// The test of `a_touch_from_a_filled_cell_...` in each layout: the tap, a stroke in the cell, a
    /// stroke out to an empty cell, and a touch on a completed game.
    fn check_a_touch_from_a_filled_cell(redraw: bool) {
        in_each_layout(|layout| {
            for use_region in [true, false] {
                let name = format!("{layout}, pen region {use_region}");
                let mut r = Rig::new(use_region);
                let empty = r.empty_cells()[0];
                let filled = r.filled_cell();
                let (x, y) = middle(filled);
                // A tap, then a second tap: the highlight comes and goes, and no ink stays.
                let frame = r.p.frame.data().to_vec();
                assert_no_ink_and_no_region(&mut r, &[(x, y), (x + 1.0, y)], redraw, &name);
                r.up((x + 1.0, y));
                assert!(r.app.ui.highlight.is_some(), "{name}: the tap makes no highlight");
                r.wait(100);
                assert_no_ink_and_no_region(&mut r, &[(x, y), (x + 1.0, y)], redraw, &name);
                r.up((x + 1.0, y));
                r.wait(100);
                assert_eq!(r.app.ui.highlight, None, "{name}");
                assert!(r.p.frame.data() == &frame[..] && r.shows_the_frame(), "{name}: the tap leaves ink");

                // A stroke in the cell, then a stroke that goes out to an empty cell.
                let out = middle(empty);
                let long: Vec<_> = (0..=10).map(|k| {
                    let t = k as f32 / 10.0;
                    (x + (out.0 - x) * t, y + (out.1 - y) * t)
                }).collect();
                for points in [stroke_in(filled), long] {
                    assert_no_ink_and_no_region(&mut r, &points, redraw, &name);
                    assert_eq!(r.app.game.value_at(empty), 0, "{name}");
                    r.up(*points.last().unwrap());
                    assert!(r.app.ui.pending_ink.is_none(), "{name}: a stroke from a filled cell gives waiting ink");
                    r.wait(crate::app::WRITING_IDLE_MS + PEN_REGION_SETTLE_MS + 30);
                    assert!(r.p.frame.data() == &frame[..] && r.shows_the_frame(), "{name}: the stroke leaves ink");
                }

                // CMP-3: all cells of a completed game are filled, and a touch has no ink.
                let mut r = Rig::new(use_region);
                r.app.game = crate::reference::completed_game();
                r.app.redraw = true;
                r.wait(600);
                let (x, y) = middle((4, 4));
                assert_no_ink_and_no_region(&mut r, &[(x, y), (x + 1.0, y), (x, y + 60.0)], redraw, &format!("{name}, completed game"));
            }
        });
    }

    /// INP-22: the drawing of the ink in parts (no full drawing in the touch).
    #[test]
    fn a_touch_from_a_filled_cell_starts_no_region_and_draws_no_ink_in_the_incremental_path() {
        check_a_touch_from_a_filled_cell(false);
    }

    /// INP-22: the same touch with a full drawing at each step (a timer, or an update of the app).
    #[test]
    fn a_touch_from_a_filled_cell_starts_no_region_and_draws_no_ink_in_a_full_drawing() {
        check_a_touch_from_a_filled_cell(true);
    }

    /// INP-22: a pen press on a filled cell while ink waits reads the ink (INP-12), stops the region
    /// and starts no new region. INP-24 applies only to an empty cell.
    #[test]
    fn a_press_on_a_filled_cell_while_the_player_writes_stops_the_region() {
        in_each_layout(|layout| {
            let mut r = Rig::new(true);
            let a = r.empty_cells()[0];
            let filled = r.filled_cell();
            r.write(a);
            r.wait(100);
            r.regions.clear();
            let (x, y) = middle(filled);
            r.down((x, y));
            assert_eq!(r.regions, vec![(r.now, None)], "{layout}: the region at the press on a filled cell");
            assert_ne!(r.app.game.value_at(a), 0, "{layout}: the press does not read the ink");
            assert_eq!(r.app.live_stroke, None, "{layout}");
            r.to((x, y + crate::layout::cell_size() as f32 / 3.0));
            assert_eq!(r.regions, vec![(r.now - 10, None)], "{layout}: a stroke from a filled cell starts the region");
        });
    }

    /// INP-22: a stroke that starts on an empty cell and crosses a filled cell shows ink over the filled
    /// cell, in the incremental path and in a full drawing. The region stays on the cell of the start.
    #[test]
    fn a_stroke_from_an_empty_cell_shows_ink_over_a_filled_cell() {
        in_each_layout(|layout| {
            for redraw in [false, true] {
                let name = format!("{layout}, full drawing {redraw}");
                let mut r = Rig::new(true);
                // An empty cell next to a filled cell, in a row.
                let (e, f) = crate::game::cells()
                    .filter(|&(_, c)| c < 8)
                    .map(|(row, c)| ((row, c), (row, c + 1)))
                    .find(|&(e, f)| !r.app.game.is_filled(e) && r.app.game.is_filled(f))
                    .unwrap();
                let (start, end) = (middle(e), middle(f));
                // The pen moves along the top of the digit in the filled cell.
                let c = crate::layout::cell_size() as f32;
                let y = start.1 - 0.4 * c;
                let before = r.frame_cell(f);
                r.down((start.0, y));
                let steps = 20;
                for k in 1..=steps {
                    r.to((start.0 + (end.0 - start.0) * k as f32 / steps as f32, y));
                    if redraw {
                        r.app.redraw = true;
                        r.after(1, None);
                    }
                }
                assert_eq!(r.p.pen_region().map(PhysicalRect::to_window), Some(region_of(e)), "{name}: the region is not on the cell of the start");
                assert!(r.frame_cell(f) != before, "{name}: no ink over the filled cell");
                r.up((end.0, y));
                assert_eq!(r.app.ui.pending_ink.as_ref().map(|i| i.cell), Some(e), "{name}: the ink is not on the cell of the start");
            }
        });
    }

    /// INP-22: a finger Redo fills the cell of the pen touch: the region stops, because the region
    /// never stays on a filled cell.
    #[test]
    fn a_redo_that_fills_the_cell_of_the_touch_stops_the_region() {
        in_each_layout(|layout| {
            let mut r = Rig::new(true);
            let filled = r.filled_cell();
            let a = r.empty_cells()[0];
            let d = r.app.game.solution[a.0 as usize][a.1 as usize];
            r.app.game.add_number(a, d);
            r.app.game.undo();
            r.app.ui.menu = crate::app::Menu::Open(None);
            r.app.redraw = true;
            r.wait(600);
            assert!(!r.app.game.redo.is_empty() && !r.app.game.is_filled(a), "{layout}: the test needs a redo");
            // The position of the Redo button, from the layout.
            let (rx, ry) = (0..crate::layout::screen_h() as i32)
                .step_by(8)
                .flat_map(|y| (0..crate::layout::screen_w() as i32).step_by(8).map(move |x| (x as f32, y as f32)))
                .find(|&(x, y)| crate::layout::game_button(x, y, true) == Some(crate::layout::Button::Redo))
                .unwrap();
            // A stroke from a filled cell starts the timer. The next press starts the region at the pen down.
            let points = stroke_in(filled);
            r.down(points[0]);
            r.to(points[20]);
            r.up(points[20]);
            let (x, y) = middle(a);
            r.down((x, y));
            assert_eq!(r.p.pen_region().map(PhysicalRect::to_window), Some(region_of(a)), "{layout}: the test needs a region");
            r.after(crate::app::HAND_TOUCH_MS, Some(crate::app::Input::TouchDown { id: 1, x: rx, y: ry }));
            r.after(10, Some(crate::app::Input::TouchUp { id: 1, x: rx, y: ry }));
            assert!(r.app.game.is_filled(a), "{layout}: the Redo does not fill the cell");
            assert_eq!(r.p.pen_region(), None, "{layout}: the region stays on a filled cell");
            assert_eq!(r.app.writing_stroke(), None, "{layout}");
            // The pen moves on: the region does not start again.
            r.to((x, y + crate::layout::cell_size() as f32 / 3.0));
            assert_eq!(r.p.pen_region(), None, "{layout}: the region starts on a filled cell");
        });
    }
}
