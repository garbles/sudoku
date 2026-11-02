//! Makes a video of the app from a script of timed events. Use it on a computer:
//!
//!     cargo run --release --bin video -- video/demo.txt video/demo.mp4
//!
//! The tool runs the real app with no screen. It sends the pen, eraser and finger events of the
//! script to the app, draws a frame at each step of the frame rate and gives the frames to ffmpeg.
//! The puzzle, the clock and the handwriting are fixed, thus each run gives the same video. Markers
//! show where the pen, the eraser and the finger touch the screen. The markers are not a part of the
//! app.
//!
//! Script lines (a `#` starts a comment). Cells are rRcC, with rows and columns from 1 to 9:
//!
//!     fps N                       frames each second (default 30)
//!     puzzle seed S mode M        the puzzle (it must come before the first event)
//!     hand right|left             the handedness of the player (default right, as on the device). It
//!                                 must come before the first event.
//!     wait S                      no input for S seconds
//!     mark rRcC D...              pen taps on the micro-cells of the digits D
//!     write rRcC D [writer N]     pen writes the digit D (handwriting of the recognizer examples,
//!                                 writer 1 to 60, default 1)
//!     erase rRcC                  eraser end rubs the cell
//!     tap menu|colors|settings|undo|redo
//!     tap color starting|clashing|highlighted COLOR
//!
//! It uses the fonts in scripts/fonts (refer to scripts/fonts/README.md) and ffmpeg. After the MP4, the
//! tool makes a smaller GIF from it with the same name (for example video/demo.gif), for README.md.
//! With `--no-gif`, it makes only the MP4 (for example video/demo-full.mp4, for the website).

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use sudoku::app::{Input, Tool};
use sudoku::font::Fonts;
use sudoku::layout;
use sudoku::render::{self, Canvas};
use sudoku::script::{parse, verify, Ev, Script};
use tiny_skia::{FillRule, LineCap, Paint, PathBuilder, Pixmap, Stroke, Transform};

/// The time that the bright markers stay after the finger lifts or the pen moves.
const FINGER_FADE_MS: u64 = 400;
const TRAIL_MS: u64 = 200;
const ERASER_R: f32 = 30.0;

/// The state of the markers at a time.
#[derive(Default)]
struct Markers {
    /// The pen point, the end of the pen, and true if it touches the screen.
    pen: Option<(f32, f32, Tool, bool)>,
    /// The points of the pen tip while it touches, with their times.
    trail: Vec<(u64, f32, f32)>,
    /// The finger point and the time when it lifted (None while it touches).
    finger: Option<(f32, f32, Option<u64>)>,
}

impl Markers {
    fn apply(&mut self, t: u64, ev: &Ev) {
        match *ev {
            Ev::Hover(p, tool) => self.pen = p.map(|(x, y)| (x, y, tool, false)),
            Ev::Input(Input::PenDown { x, y, tool }) => {
                self.pen = Some((x, y, tool, true));
                self.trail.clear();
                if tool == Tool::Pen {
                    self.trail.push((t, x, y));
                }
            }
            Ev::Input(Input::PenMove { x, y }) => {
                if let Some((_, _, tool, _)) = self.pen {
                    self.pen = Some((x, y, tool, true));
                    if tool == Tool::Pen {
                        self.trail.push((t, x, y));
                    }
                }
            }
            Ev::Input(Input::PenUp { x, y }) => {
                if let Some((_, _, tool, _)) = self.pen {
                    self.pen = Some((x, y, tool, false));
                }
            }
            Ev::Input(Input::TouchDown { x, y, .. }) => self.finger = Some((x, y, None)),
            Ev::Input(Input::TouchUp { x, y, .. }) => self.finger = Some((x, y, Some(t))),
            Ev::Input(Input::TouchMove { x, y, .. }) => self.finger = Some((x, y, None)),
        }
    }

    fn draw(&mut self, pm: &mut Pixmap, t: u64) {
        let paint = |rgb: [u8; 3], a: f32| {
            let mut p = Paint::default();
            p.set_color_rgba8(rgb[0], rgb[1], rgb[2], (a.clamp(0.0, 1.0) * 255.0) as u8);
            p.anti_alias = true;
            p
        };
        let circle = |pm: &mut Pixmap, x: f32, y: f32, r: f32, fill: Option<Paint>, line: Option<(Paint, f32)>| {
            let Some(path) = PathBuilder::from_circle(x, y, r) else { return };
            if let Some(f) = fill {
                pm.fill_path(&path, &f, FillRule::Winding, Transform::identity(), None);
            }
            if let Some((p, w)) = line {
                pm.stroke_path(&path, &p, &Stroke { width: w, ..Stroke::default() }, Transform::identity(), None);
            }
        };
        const DARK: [u8; 3] = [20, 20, 20];
        const RED: [u8; 3] = [210, 40, 40];
        const GRAY: [u8; 3] = [110, 110, 110];

        if let Some((fx, fy, lifted)) = self.finger {
            let a = match lifted {
                None => 1.0,
                Some(l) => 1.0 - t.saturating_sub(l) as f32 / FINGER_FADE_MS as f32,
            };
            if a <= 0.0 {
                self.finger = None;
            } else {
                circle(pm, fx, fy, 36.0, Some(paint(GRAY, 0.35 * a)), Some((paint(GRAY, 0.8 * a), 3.0)));
            }
        }

        self.trail.retain(|&(pt, _, _)| t.saturating_sub(pt) <= TRAIL_MS);
        let touching = matches!(self.pen, Some((_, _, Tool::Pen, true)));
        if touching {
            for w in self.trail.windows(2) {
                let a = 1.0 - t.saturating_sub(w[1].0) as f32 / TRAIL_MS as f32;
                let mut pb = PathBuilder::new();
                pb.move_to(w[0].1, w[0].2);
                pb.line_to(w[1].1, w[1].2);
                if let Some(path) = pb.finish() {
                    let s = Stroke { width: 6.0, line_cap: LineCap::Round, ..Stroke::default() };
                    pm.stroke_path(&path, &paint(DARK, 0.4 * a), &s, Transform::identity(), None);
                }
            }
        }

        match self.pen {
            Some((x, y, Tool::Pen, true)) => circle(pm, x, y, 8.0, Some(paint(DARK, 0.9)), None),
            Some((x, y, Tool::Pen, false)) => circle(pm, x, y, 12.0, None, Some((paint(DARK, 0.6), 2.0))),
            Some((x, y, Tool::Eraser, true)) => {
                circle(pm, x, y, ERASER_R, Some(paint(RED, 0.2)), Some((paint(RED, 0.9), 3.0)))
            }
            Some((x, y, Tool::Eraser, false)) => circle(pm, x, y, ERASER_R, None, Some((paint(RED, 0.6), 2.0))),
            None => {}
        }
    }
}

/// The number of ffmpeg threads. The output of x264 changes with the number of threads, so a fixed number
/// gives the same files on all computers (Dockerfile.video, the GitHub workflow).
const THREADS: &str = "4";
/// The scaler flags for the same pixels on all CPUs (no fast SIMD rounding).
const EXACT: &str = "bitexact+accurate_rnd+full_chroma_int";

fn ffmpeg(out: &Path, fps: u64, w: u32, h: u32) -> std::process::Child {
    let size = format!("{w}x{h}");
    let rate = fps.to_string();
    Command::new("ffmpeg")
        .args(["-loglevel", "error", "-y", "-sws_flags", EXACT, "-f", "rawvideo", "-pix_fmt", "rgb24", "-s", &size, "-r", &rate, "-i", "-"])
        .args(["-c:v", "libx264", "-pix_fmt", "yuv420p", "-crf", "18", "-movflags", "+faststart", "-threads", THREADS])
        .arg(out)
        .stdin(Stdio::piped())
        .spawn()
        .expect("ffmpeg (install it and put it on PATH)")
}

/// Makes a GIF from the MP4: 15 frames each second, 540 px wide, with one palette for all frames.
fn gif(mp4: &Path, out: &Path) -> bool {
    Command::new("ffmpeg")
        .args(["-loglevel", "error", "-y", "-sws_flags", EXACT, "-threads", THREADS, "-filter_threads", THREADS, "-i"])
        .arg(mp4)
        .args(["-vf", &format!("fps=15,scale=540:-1:flags=lanczos+{EXACT},split[a][b];[a]palettegen[p];[b][p]paletteuse"), "-threads", THREADS])
        .arg(out)
        .status()
        .is_ok_and(|s| s.success())
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let no_gif = args.first().is_some_and(|a| a == "--no-gif");
    if no_gif {
        args.remove(0);
    }
    let [script_path, out] = args.as_slice() else {
        eprintln!("usage: video [--no-gif] SCRIPT OUT.mp4");
        std::process::exit(2);
    };
    if !out.ends_with(".mp4") {
        eprintln!("the output file must end with .mp4");
        std::process::exit(2);
    }
    let fonts = Fonts::for_tools().expect("fonts");

    let text = std::fs::read_to_string(script_path).expect("script");
    let script = match parse(&text, sudoku::app::DEFAULT_PAUSE_MS) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{script_path}: {e}");
            std::process::exit(1);
        }
    };
    let mut app = script.app(&fonts);
    let Script { fps, timeline, .. } = script;

    let (w, h) = (layout::screen_w(), layout::screen_h());
    let mut child = ffmpeg(Path::new(out), fps, w, h);
    let mut stdin = child.stdin.take().unwrap();

    // The video ends one second after the last event.
    let last = timeline.events.last().map_or(0, |e| e.0).max(timeline.checks.last().map_or(0, |c| c.0).max(timeline.t));
    let end = last + 1000;
    let frames = end * fps / 1000;
    let mut events = timeline.events.iter().peekable();
    let mut checks = timeline.checks.clone();
    checks.sort_by_key(|c| (c.0, c.1));
    let mut checks = checks.into_iter().peekable();
    let mut markers = Markers::default();
    let mut pm = Pixmap::new(w, h).unwrap();
    let mut rgb = vec![0u8; (w * h * 3) as usize];
    app.tick(0);
    for f in 0..=frames {
        let t = f * 1000 / fps;
        while let Some(&&(et, ev)) = events.peek() {
            if et > t {
                break;
            }
            if let Ev::Input(input) = ev {
                app.handle(input, et);
            }
            markers.apply(et, &ev);
            events.next();
        }
        app.tick(t);
        while let Some(&(ct, _, c)) = checks.peek() {
            if ct > t {
                break;
            }
            if let Err(e) = verify(&app, c) {
                eprintln!("{script_path}: at {:.2} s: {e}", ct as f64 / 1000.0);
                drop(stdin);
                let _ = child.wait();
                std::process::exit(1);
            }
            checks.next();
        }
        render::draw(&mut Canvas::new(&mut pm, &fonts), &app);
        markers.draw(&mut pm, t);
        for (o, p) in rgb.chunks_exact_mut(3).zip(pm.pixels()) {
            o.copy_from_slice(&[p.red(), p.green(), p.blue()]);
        }
        stdin.write_all(&rgb).expect("ffmpeg stopped");
    }
    drop(stdin);
    let status = child.wait().expect("ffmpeg");
    if !status.success() {
        eprintln!("ffmpeg failed");
        std::process::exit(1);
    }
    println!("{out}: {} frames, {:.1} s", frames + 1, end as f64 / 1000.0);
    if no_gif {
        return;
    }

    let gif_out = Path::new(out).with_extension("gif");
    if !gif(Path::new(out), &gif_out) {
        eprintln!("ffmpeg failed to make {}", gif_out.display());
        std::process::exit(1);
    }
    println!("{}", gif_out.display());
}
