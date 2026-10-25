//! The device: AppLoad (qtfb) input and output, the save files and the main loop.

pub mod evdev;
pub mod qtfb;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use tiny_skia::Pixmap;

use crate::app::{App, Input, Tool, DEFAULT_PAUSE_MS};
use crate::font::Fonts;
use crate::game::Game;
use crate::layout::{screen_h, screen_w};
use crate::render::{self, Canvas};
use crate::settings::Settings;
use crate::store::{self, ResultsDb};
use crate::system::SystemEnv;

/// The longest wait for the rotation of the tablet before the first drawing.
const START_ROTATION_WAIT: Duration = Duration::from_millis(300);

/// The shortest time between two updates of the ink while the pen writes.
const INK_UPDATE_INTERVAL: Duration = Duration::from_millis(30);

/// The time after the last ink update before the app sends the full ink area again.
const INK_REPAIR_DELAY: Duration = Duration::from_millis(400);

/// The clock is saved at least this often (PER-1).
const CLOCK_SAVE_INTERVAL: Duration = Duration::from_secs(5);

static STOP: AtomicBool = AtomicBool::new(false);

extern "C" fn on_signal(_: libc::c_int) {
    STOP.store(true, Ordering::SeqCst);
}

struct Paths {
    game: PathBuf,
    settings: PathBuf,
    results: PathBuf,
}

impl Paths {
    fn new() -> Paths {
        let dir = store::data_dir();
        Paths { game: dir.join("game.json"), settings: dir.join("settings.json"), results: dir.join("results.sqlite") }
    }
}

fn save(app: &mut App, paths: &Paths) {
    if app.game_dirty {
        match store::save_json(&paths.game, &app.game) {
            Ok(()) => app.game_dirty = false,
            Err(e) => eprintln!("sudoku: cannot save the game: {e}"),
        }
    }
    if app.settings_dirty {
        match store::save_json(&paths.settings, &app.settings) {
            Ok(()) => app.settings_dirty = false,
            Err(e) => eprintln!("sudoku: cannot save the settings: {e}"),
        }
    }
}

/// Converts a qtfb input message to an app input.
fn to_input(m: &qtfb::UserInput, eraser: &evdev::Eraser) -> Option<Input> {
    let (x, y) = crate::layout::to_screen(m.x as f32, m.y as f32);
    Some(match m.kind {
        qtfb::INPUT_PEN_PRESS => {
            let tool = if eraser.is_active() { Tool::Eraser } else { Tool::Pen };
            Input::PenDown { x, y, tool }
        }
        qtfb::INPUT_PEN_UPDATE => Input::PenMove { x, y },
        qtfb::INPUT_PEN_RELEASE => Input::PenUp { x, y },
        qtfb::INPUT_TOUCH_PRESS => Input::TouchDown { id: m.dev_id, x, y },
        qtfb::INPUT_TOUCH_UPDATE => Input::TouchMove { id: m.dev_id, x, y },
        qtfb::INPUT_TOUCH_RELEASE => Input::TouchUp { id: m.dev_id, x, y },
        _ => return None,
    })
}

/// Copies the changed area of the frame (screen coordinates) into the framebuffer (physical
/// coordinates, with the rotation of the tablet). Returns the changed physical rectangle.
fn blit(frame: &Pixmap, fb: &mut [u8]) -> Option<(i32, i32, i32, i32)> {
    let w = frame.width() as usize;
    let px = frame.pixels();
    let (base, sx, sy) = crate::layout::physical_steps();
    let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for y in 0..frame.height() as usize {
        let row = &px[y * w..(y + 1) * w];
        let mut i = base + y as i64 * sy;
        for (x, p) in row.iter().enumerate() {
            let rgb = [p.red(), p.green(), p.blue()];
            let o = &mut fb[i as usize * 3..i as usize * 3 + 3];
            if o != rgb {
                o.copy_from_slice(&rgb);
                x0 = x0.min(x as i32);
                x1 = x1.max(x as i32);
                y0 = y0.min(y as i32);
                y1 = y1.max(y as i32);
            }
            i += sx;
        }
    }
    (x0 != i32::MAX).then(|| crate::layout::physical_rect((x0, y0, x1, y1)))
}

/// `sudoku --self-test OUT.png [SECONDS]`: examines the device without qtfb. It loads the font of the
/// device, draws the state of images/screen-in-progress.png into OUT.png, reads one example of
/// handwriting, and watches the pen tools for SECONDS (default 0).
pub fn self_test(out: &str, seconds: u64) -> Result<(), String> {
    let t = Instant::now();
    let fonts = Fonts::load()?;
    println!("self-test: font loaded in {:?} (Easy: {:.2} px, Hard: {:.2} px)", t.elapsed(), fonts.medium.width(25.0, "Easy"), fonts.medium.width(25.0, "Hard"));
    let t = Instant::now();
    let pm = crate::reference::render(&fonts, |a| a.game = crate::reference::sample_game());
    println!("self-test: screen drawn in {:?}", t.elapsed());
    let rgb: Vec<u8> = pm.pixels().iter().flat_map(|p| [p.red(), p.green(), p.blue()]).collect();
    let file = std::io::BufWriter::new(std::fs::File::create(out).map_err(|e| e.to_string())?);
    let mut enc = png::Encoder::new(file, pm.width(), pm.height());
    enc.set_color(png::ColorType::Rgb);
    enc.write_header().and_then(|mut w| w.write_image_data(&rgb)).map_err(|e| e.to_string())?;
    let t = Instant::now();
    crate::recognizer::warm_up();
    println!("self-test: recognizer ready in {:?}", t.elapsed());
    let ink = crate::recognizer::example_ink(7, 50, 300.0, 600.0, 98.0).ok_or("no example")?;
    let t = Instant::now();
    let got = crate::recognizer::recognize(&ink);
    println!("self-test: one reading in {:?}: {got:?}", t.elapsed());
    match evdev::find_digitizer() {
        Some(_) => println!("self-test: digitizer with pen and eraser found"),
        None => println!("self-test: NO digitizer with pen and eraser"),
    }
    if seconds > 0 {
        println!("self-test: touch the screen with the pen tip, then with the eraser ({seconds} s)");
        let (pen, rubber) = evdev::watch_tools(seconds)?;
        println!("self-test: pen {pen} times, eraser {rubber} times");
    }
    Ok(())
}

/// Copies a screen rectangle (x0, y0, x1, y1; all included) of the frame into the framebuffer.
/// Returns the physical rectangle (x, y, width, height).
fn blit_rect(frame: &Pixmap, fb: &mut [u8], r: (i32, i32, i32, i32)) -> (i32, i32, i32, i32) {
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

pub fn run() -> Result<(), String> {
    // SAFETY: the handler only stores into an atomic.
    unsafe {
        for sig in [libc::SIGTERM, libc::SIGINT, libc::SIGHUP] {
            libc::signal(sig, on_signal as *const () as libc::sighandler_t);
        }
    }
    // The recognizer prepares its examples while the app starts.
    std::thread::spawn(crate::recognizer::warm_up);
    let fonts = Fonts::load()?;
    let paths = Paths::new();
    std::fs::create_dir_all(paths.results.parent().unwrap()).map_err(|e| format!("cannot make the data folder: {e}"))?;
    let db = ResultsDb::open(&paths.results).map_err(|e| format!("cannot open the results database: {e}"))?;
    let saved: Option<Game> = store::load_json(&paths.game);
    let settings: Option<Settings> = store::load_json(&paths.settings);
    let mut app = App::new(saved, settings, db, Box::new(SystemEnv::new()));
    app.ng_stops = render::ng_stops(&fonts);
    let game_path = paths.game.clone();
    app.save_game_now = Some(Box::new(move |g: &Game| match store::save_json(&game_path, g) {
        Ok(()) => true,
        Err(e) => {
            eprintln!("sudoku: cannot save the game: {e}");
            false
        }
    }));
    app.pause_ms = std::env::var("SUDOKU_RECOGNITION_PAUSE_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(DEFAULT_PAUSE_MS);
    save(&mut app, &paths);

    let key: i32 = std::env::var("QTFB_KEY")
        .map_err(|_| "QTFB_KEY is not set. Start the app from AppLoad.".to_string())?
        .parse()
        .map_err(|_| "QTFB_KEY is not a number".to_string())?;
    let mut conn = qtfb::Connection::connect(key).map_err(|e| format!("cannot connect to qtfb: {e}"))?;
    let eraser = evdev::Eraser::start();

    let start = Instant::now();
    let now_ms = || start.elapsed().as_millis() as u64;
    // AppLoad sends the rotation of the tablet when the app connects. Wait for it (a short time) before
    // the first drawing, so that the app does not show a portrait screen first in landscape. Input
    // that comes first is kept for the main loop.
    let mut early_input = Vec::new();
    let connected = Instant::now();
    let deadline = connected + START_ROTATION_WAIT;
    'wait: while Instant::now() < deadline {
        let left = deadline.saturating_duration_since(Instant::now()).as_millis().max(1) as libc::c_int;
        let mut pfd = libc::pollfd { fd: conn.fd(), events: libc::POLLIN, revents: 0 };
        // SAFETY: one valid pollfd.
        unsafe { libc::poll(&mut pfd, 1, left) };
        loop {
            match conn.try_recv() {
                Ok(Some(qtfb::Event::Rotation(r))) => {
                    eprintln!("sudoku: rotation {r} at the start, {:?} after the connection", connected.elapsed());
                    crate::layout::set_rotation(r);
                    app.rotate(render::ng_stops(&fonts));
                    break 'wait;
                }
                Ok(Some(qtfb::Event::Input(m))) => early_input.push(m),
                Ok(None) => break,
                Err(e) => return Err(format!("qtfb: {e}")),
            }
        }
    }
    let mut frame = Pixmap::new(screen_w(), screen_h()).unwrap();
    // One refresh mode for the full time: a change of the mode makes AppLoad refresh the full window,
    // which shows as a flash.
    conn.set_refresh_mode(qtfb::RefreshMode::Ui).ok();
    render::draw(&mut Canvas::new(&mut frame, &fonts), &app);
    blit(&frame, conn.pixels());
    conn.update_all().map_err(|e| format!("qtfb update: {e}"))?;
    for m in early_input {
        if let Some(input) = to_input(&m, &eraser) {
            app.handle(input, now_ms());
        }
    }
    let mut last_clock_save = Instant::now();
    // The live stroke (`App::live_stroke_count`) and the quantity of its points that the frame shows.
    let mut ink_drawn = (0u64, 0usize);
    // The area of ink that is drawn in the frame but not yet sent, and the time of the last ink update.
    // AppLoad repaints the screen for each update, thus the app sends at most one ink update each
    // INK_UPDATE_INTERVAL.
    let mut ink_box: Option<(i32, i32, i32, i32)> = None;
    let mut last_ink_update = Instant::now();
    // The area of all recent ink updates. The panel can skip pixels of an update that overlaps an
    // update that is still in progress. Thus, when the ink stops for INK_REPAIR_DELAY, the app sends
    // this area one more time, after the earlier updates are complete.
    let mut repair_box: Option<(i32, i32, i32, i32)> = None;
    // SUDOKU_LOG_INPUT=1 writes each input message to the system log (for an examination on the device).
    let log_input = std::env::var_os("SUDOKU_LOG_INPUT").is_some();

    while !app.quit && !STOP.load(Ordering::SeqCst) {
        // Wait for input, a timer of the app, or the next clock save.
        // The ink timers round up: a wait of 0 ms before the deadline makes the loop spin.
        let until = |d: Duration| (d.saturating_sub(last_ink_update.elapsed()).as_micros() as u64).div_ceil(1000);
        let mut wait = app.next_deadline().map(|d| d.saturating_sub(now_ms())).unwrap_or(1000).min(1000);
        if ink_box.is_some() {
            wait = wait.min(until(INK_UPDATE_INTERVAL));
        }
        if repair_box.is_some() {
            wait = wait.min(until(INK_REPAIR_DELAY));
        }
        let mut pfd = libc::pollfd { fd: conn.fd(), events: libc::POLLIN, revents: 0 };
        // SAFETY: one valid pollfd.
        unsafe { libc::poll(&mut pfd, 1, wait as libc::c_int) };

        let mut closed = false;
        loop {
            match conn.try_recv() {
                Ok(Some(qtfb::Event::Rotation(r))) => {
                    eprintln!("sudoku: rotation {r}, {:?} after the start", start.elapsed());
                    // The layout keeps the rotation clamped to 0..=3: compare the same value.
                    if r.clamp(0, 3) != crate::layout::rotation() {
                        crate::layout::set_rotation(r);
                        app.rotate(render::ng_stops(&fonts));
                        frame = Pixmap::new(screen_w(), screen_h()).unwrap();
                        ink_box = None;
                        repair_box = None;
                    }
                }
                Ok(Some(qtfb::Event::Input(m))) => {
                    if log_input {
                        eprintln!("sudoku: input type {:#x} id {} at ({}, {}) d {}", m.kind, m.dev_id, m.x, m.y, m.d);
                    }
                    if let Some(input) = to_input(&m, &eraser) {
                        app.handle(input, now_ms());
                    }
                    // The close button stops the app: the input after it has no effect (NAV-6).
                    if app.quit {
                        break;
                    }
                }
                Ok(None) => break,
                Err(_) => {
                    closed = true;
                    break;
                }
            }
        }
        app.tick(now_ms());
        if closed {
            break;
        }

        if last_clock_save.elapsed() >= CLOCK_SAVE_INTERVAL && !app.game.completed {
            app.game_dirty = true;
            last_clock_save = Instant::now();
        }
        save(&mut app, &paths);

        if app.redraw {
            ink_box = None;
            render::draw(&mut Canvas::new(&mut frame, &fonts), &app);
            app.redraw = false;
            ink_drawn = (app.live_stroke_count, app.live_stroke.as_ref().map_or(0, |s| s.len()));
            if let Some((x, y, w, h)) = blit(&frame, conn.pixels()) {
                conn.update(x, y, w, h).ok();
            }
        } else if let Some((s, drawn)) = app
            .live_stroke
            .as_ref()
            // A new stroke (for example the second stroke of a 4) has no points in the frame yet.
            .map(|s| (s, if ink_drawn.0 == app.live_stroke_count { ink_drawn.1 } else { 0 }))
            .filter(|(s, drawn)| s.len() > (*drawn).max(1))
        {
            // Only the ink is new: draw the new segments and send only their area. A full drawing
            // takes approximately 65 ms on the device, which is too slow to follow the pen.
            let new = &s[drawn.max(1) - 1..];
            Canvas::new(&mut frame, &fonts).polyline(new, render::INK_WIDTH, render::BLACK, true);
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
            ink_box = Some(match ink_box {
                None => rect,
                Some(b) => (b.0.min(rect.0), b.1.min(rect.1), b.2.max(rect.2), b.3.max(rect.3)),
            });
            ink_drawn = (app.live_stroke_count, s.len());
        }
        if let Some(b) = ink_box {
            if last_ink_update.elapsed() >= INK_UPDATE_INTERVAL {
                let (x, y, w, h) = blit_rect(&frame, conn.pixels(), b);
                conn.update(x, y, w, h).ok();
                ink_box = None;
                last_ink_update = Instant::now();
                repair_box = Some(match repair_box {
                    None => b,
                    Some(r) => (r.0.min(b.0), r.1.min(b.1), r.2.max(b.2), r.3.max(b.3)),
                });
            }
        }
        if let Some(r) = repair_box {
            if ink_box.is_none() && last_ink_update.elapsed() >= INK_REPAIR_DELAY {
                let (x, y, w, h) = crate::layout::physical_rect(r);
                conn.update(x, y, w, h).ok();
                repair_box = None;
            }
        }
    }

    eprintln!("sudoku: the app stops (quit {}, signal {})", app.quit, STOP.load(Ordering::SeqCst));
    // The app closes: ink that waits is removed (PER-6) and the game is saved (PER-1, NAV-6).
    app.tick(now_ms());
    app.suspend();
    save(&mut app, &paths);
    Ok(())
}
