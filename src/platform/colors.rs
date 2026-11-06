//! The main loop of the color picker app (`crate::colors`).

use std::collections::VecDeque;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use tiny_skia::Pixmap;

use super::{on_signal, qtfb, STOP};
use crate::colors::{self, Area, Change, Colors, Picker};
use crate::font::Fonts;
use crate::layout::{PHYSICAL_H, PHYSICAL_W};
use crate::render::Canvas;
use crate::store;

/// The pointer value of the pen. The touch points have values from 0.
const PEN: i32 = -1;
/// The shortest time between two updates of the sliders while the pen moves.
const SLIDER_INTERVAL: Duration = Duration::from_millis(100);
/// Qt merges the updates of one frame into their bounding box: keep separate updates apart.
const UPDATE_SPACING: Duration = Duration::from_millis(40);
/// The time of the black step and of the white step of a refresh of the preview cells. It is longer
/// than the time of the UI waveform, thus a step does not stop the step before it.
const FLASH_STEP: Duration = Duration::from_millis(500);

const FULL_SCREEN: Area = (0, 0, PHYSICAL_W as i32, PHYSICAL_H as i32);

/// A refresh of the preview cells of one color: black, then white, then the new color.
struct Refresh {
    area: Area,
    /// The button of the color: its dot shows the new color at the end.
    button: Area,
    /// The screen is white now (else black).
    white: bool,
    /// The time of the next step.
    at: Instant,
}

fn draw_frame(frame: &mut Pixmap, picker: &Picker, fonts: &Fonts) {
    frame.fill(tiny_skia::Color::WHITE);
    colors::draw(&mut Canvas::new(frame, fonts), picker);
}

fn copy_area(frame: &Pixmap, fb: &mut [u8], (x, y, w, h): Area) {
    let (stride, px) = (PHYSICAL_W as usize, frame.data());
    for row in y as usize..(y + h) as usize {
        for col in x as usize..(x + w) as usize {
            let (s, d) = ((row * stride + col) * 4, (row * stride + col) * 3);
            fb[d..d + 3].copy_from_slice(&px[s..s + 3]);
        }
    }
}

fn fill_area(fb: &mut [u8], (x, y, w, h): Area, value: u8) {
    let stride = PHYSICAL_W as usize * 3;
    for row in y as usize..(y + h) as usize {
        let start = row * stride + x as usize * 3;
        fb[start..start + w as usize * 3].fill(value);
    }
}

fn queue(pending: &mut VecDeque<Area>, area: Area) {
    if !pending.contains(&area) {
        pending.push_back(area);
    }
}

pub fn run() -> Result<(), String> {
    // SAFETY: the handler only stores into an atomic.
    unsafe {
        for sig in [libc::SIGTERM, libc::SIGINT, libc::SIGHUP] {
            libc::signal(sig, on_signal as *const () as libc::sighandler_t);
        }
    }
    let fonts = Fonts::load()?;
    let save_path = colors::save_path();
    let saved: Option<Colors> = store::load_json(&save_path);
    let mut picker = Picker::new(saved.unwrap_or_default());

    let key: i32 = std::env::var("QTFB_KEY")
        .map_err(|_| "QTFB_KEY is not set. Start the app from AppLoad.".to_string())?
        .parse()
        .map_err(|_| "QTFB_KEY is not a number".to_string())?;
    let mut conn = qtfb::Connection::connect(key).map_err(|e| format!("cannot connect to qtfb: {e}"))?;
    conn.set_refresh_mode(qtfb::RefreshMode::Ui).ok();

    let mut frame = Pixmap::new(PHYSICAL_W, PHYSICAL_H).ok_or("cannot make the frame")?;
    draw_frame(&mut frame, &picker, &fonts);
    copy_area(&frame, conn.pixels(), FULL_SCREEN);
    conn.update_all().map_err(|e| format!("qtfb update: {e}"))?;

    let mut pending: VecDeque<Area> = VecDeque::new();
    let mut last_update = Instant::now();
    let mut sliders_dirty = false;
    let mut last_sliders = Instant::now();
    let mut refreshes: Vec<Refresh> = Vec::new();

    while !STOP.load(Ordering::SeqCst) {
        let now = Instant::now();
        let mut wake = refreshes.iter().map(|r| r.at).min();
        let mut sooner = |t: Instant| wake = Some(wake.map_or(t, |w| w.min(t)));
        if !pending.is_empty() {
            sooner(last_update + UPDATE_SPACING);
        }
        if sliders_dirty {
            sooner(last_sliders + SLIDER_INTERVAL);
        }
        let wait = wake.map_or(1000, |w| w.saturating_duration_since(now).as_millis().min(1000) as libc::c_int + 1);
        let mut pfd = libc::pollfd { fd: conn.fd(), events: libc::POLLIN, revents: 0 };
        // SAFETY: one valid pollfd.
        unsafe { libc::poll(&mut pfd, 1, wait) };

        let mut quit = false;
        loop {
            let m = match conn.try_recv() {
                // The window stays in portrait (`supportsRotation` is false).
                Ok(Some(qtfb::Event::Rotation(_))) => continue,
                Ok(Some(qtfb::Event::Input(m))) => m,
                Ok(None) => break,
                Err(_) => {
                    quit = true;
                    break;
                }
            };
            let (x, y) = (m.x as f32, m.y as f32);
            let change = match m.kind {
                qtfb::INPUT_PEN_PRESS => picker.press(PEN, x, y),
                qtfb::INPUT_PEN_UPDATE => picker.moved(PEN, x),
                qtfb::INPUT_PEN_RELEASE => picker.release(PEN, x, y),
                qtfb::INPUT_TOUCH_PRESS => picker.press(m.dev_id, x, y),
                qtfb::INPUT_TOUCH_UPDATE => picker.moved(m.dev_id, x),
                qtfb::INPUT_TOUCH_RELEASE => picker.release(m.dev_id, x, y),
                _ => Change::None,
            };
            match change {
                Change::None => {}
                Change::Quit => quit = true,
                Change::Slider => sliders_dirty = true,
                Change::Selection => {
                    draw_frame(&mut frame, &picker, &fonts);
                    for area in [colors::BUTTONS_AREA, colors::LOWER_AREA] {
                        copy_area(&frame, conn.pixels(), area);
                        queue(&mut pending, area);
                    }
                }
                Change::Refresh(targets) => {
                    if let Err(e) = store::save_json(&save_path, &picker.colors) {
                        eprintln!("sudoku-colors: cannot save the colors: {e}");
                    }
                    // The sliders show the last value. Then the refresh of these colors starts.
                    draw_frame(&mut frame, &picker, &fonts);
                    copy_area(&frame, conn.pixels(), colors::SLIDERS_AREA);
                    queue(&mut pending, colors::SLIDERS_AREA);
                    sliders_dirty = false;
                    for target in &targets {
                        let area = target.preview_area();
                        fill_area(conn.pixels(), area, 0);
                        queue(&mut pending, area);
                    }
                    // The next step starts after the last black update has had its full time.
                    let at = Instant::now() + UPDATE_SPACING * pending.len() as u32 + FLASH_STEP;
                    for target in &targets {
                        let area = target.preview_area();
                        refreshes.retain(|r| r.area != area);
                        refreshes.push(Refresh { area, button: target.button_area(), white: false, at });
                    }
                }
            }
        }
        if quit {
            break;
        }

        let now = Instant::now();
        if sliders_dirty && now >= last_sliders + SLIDER_INTERVAL {
            sliders_dirty = false;
            last_sliders = now;
            draw_frame(&mut frame, &picker, &fonts);
            copy_area(&frame, conn.pixels(), colors::SLIDERS_AREA);
            queue(&mut pending, colors::SLIDERS_AREA);
        }
        let mut drawn = false;
        refreshes.retain_mut(|r| {
            if now < r.at {
                return true;
            }
            if !r.white {
                fill_area(conn.pixels(), r.area, 255);
                queue(&mut pending, r.area);
                r.white = true;
                r.at = now + UPDATE_SPACING * pending.len() as u32 + FLASH_STEP;
                return true;
            }
            // The last step: the new color, and the dot of its button.
            if !drawn {
                draw_frame(&mut frame, &picker, &fonts);
                drawn = true;
            }
            for area in [r.area, r.button] {
                copy_area(&frame, conn.pixels(), area);
                queue(&mut pending, area);
            }
            false
        });
        if now >= last_update + UPDATE_SPACING {
            if let Some((x, y, w, h)) = pending.pop_front() {
                conn.update(x, y, w, h).ok();
                last_update = now;
            }
        }
    }
    Ok(())
}
