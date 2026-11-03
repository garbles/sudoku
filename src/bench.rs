//! The performance bench (`sudoku --bench` on the tablet, `cargo run --release --bin bench` on a
//! computer). It plays an event script with no screen, with the same main-loop output code as the
//! device (`present.rs`), and measures the time of each phase. It also gives hashes of the output, so
//! that an optimization that changes the output is found.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use crate::app::{App, Input, DEFAULT_PAUSE_MS};
use crate::font::Fonts;
use crate::layout::{PHYSICAL_H, PHYSICAL_W};
use crate::present::{Presenter, Update};
use crate::render::{self, Canvas};
use crate::script::{self, Ev};

/// The script of the bench: the extended demo, which plays a full game.
pub const SCRIPT: &str = include_str!("../video/demo-full.txt");

/// The longest wait of the main loop without input or timers (platform/mod.rs).
const IDLE_WAKE_MS: u64 = 1000;

pub struct Options {
    /// The script text.
    pub script: String,
    /// The quantity of seeds for each mode of the generator (0: no generator phase).
    pub gen_seeds: u64,
    /// Read each n-th recognizer example (0: no recognizer phase).
    pub recognize_step: usize,
    /// The quantity of full drawings of each reference state.
    pub draw_reps: usize,
    /// Writes the framebuffer at each check of the script into this folder (frame-NNN.rgb), and each
    /// reference state (state-NAME.rgba), to find the pixels that an optimization changed.
    pub dump: Option<std::path::PathBuf>,
    /// Compares the frame after each redraw with a full drawing of the app (for the tests: the
    /// presenter draws only the areas that can change).
    pub check_frames: bool,
}

impl Default for Options {
    fn default() -> Options {
        Options { script: SCRIPT.to_string(), gen_seeds: 3, recognize_step: 5, draw_reps: 5, dump: None, check_frames: false }
    }
}

/// The measured times of one phase.
#[derive(Default)]
struct Samples(Vec<Duration>);

impl Samples {
    fn add(&mut self, d: Duration) {
        self.0.push(d);
    }

    fn json(&self) -> serde_json::Value {
        let mut v: Vec<f64> = self.0.iter().map(|d| d.as_secs_f64() * 1e6).collect();
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let q = |f: f64| if v.is_empty() { 0.0 } else { v[((v.len() - 1) as f64 * f).round() as usize] };
        let total: f64 = v.iter().sum();
        serde_json::json!({
            "n": v.len(),
            "total_ms": round(total / 1000.0),
            "mean_us": round(if v.is_empty() { 0.0 } else { total / v.len() as f64 }),
            "p50_us": round(q(0.5)),
            "p95_us": round(q(0.95)),
            "max_us": round(q(1.0)),
        })
    }
}

fn round(x: f64) -> f64 {
    (x * 10.0).round() / 10.0
}

/// FNV-1a, for the output hashes.
#[derive(Clone, Copy)]
struct Hash(u64);

impl Hash {
    fn new() -> Hash {
        Hash(0xcbf2_9ce4_8422_2325)
    }

    fn bytes(&mut self, b: &[u8]) {
        // Eight bytes at a time: the framebuffer is large.
        let mut chunks = b.chunks_exact(8);
        for c in &mut chunks {
            self.0 = (self.0 ^ u64::from_le_bytes(c.try_into().unwrap())).wrapping_mul(0x0100_0000_01b3);
        }
        for &x in chunks.remainder() {
            self.0 = (self.0 ^ x as u64).wrapping_mul(0x0100_0000_01b3);
        }
    }

    fn u64(&mut self, x: u64) {
        self.bytes(&x.to_le_bytes());
    }

    fn hex(self) -> String {
        format!("{:016x}", self.0)
    }
}

#[derive(Default)]
struct Phases(BTreeMap<&'static str, Samples>);

impl Phases {
    fn add(&mut self, name: &'static str, d: Duration) {
        self.0.entry(name).or_default().add(d);
    }

    fn add_nonzero(&mut self, name: &'static str, d: Duration) {
        if !d.is_zero() {
            self.add(name, d);
        }
    }
}

/// Runs the bench. `load_fonts` loads the fonts (the bench measures it). Returns the JSON report.
pub fn run(load_fonts: impl FnOnce() -> Result<Fonts, String>, opts: &Options) -> Result<serde_json::Value, String> {
    let mut phases = Phases::default();
    let mut hashes = BTreeMap::new();

    // ---- Startup (as platform::run, without qtfb and files).
    let t = Instant::now();
    let fonts = load_fonts()?;
    phases.add("startup.fonts", t.elapsed());
    let t = Instant::now();
    crate::recognizer::warm_up();
    phases.add("startup.recognizer", t.elapsed());

    // ---- The script.
    let script = script::parse(&opts.script, DEFAULT_PAUSE_MS)?;
    let t = Instant::now();
    let mut app = script.app(&fonts);
    phases.add("startup.app", t.elapsed());
    let t = Instant::now();
    let mut presenter = Presenter::new();
    let mut fb = vec![0u8; (PHYSICAL_W * PHYSICAL_H * 3) as usize];
    presenter.draw_all(&app, &fonts, &mut fb);
    phases.add("startup.first_frame", t.elapsed());

    let mut updates = Hash::new();
    let mut frames = Hash::new();
    let mut update_count = 0u64;
    let mut checks = script.timeline.checks.clone();
    checks.sort_by_key(|c| (c.0, c.1));
    let mut checks = checks.into_iter().peekable();
    let mut now = 0u64;
    let mut frame_err = None;
    app.tick(now);

    // One step of the main loop at `now`: the input (if any), the timers, the output.
    let mut step = |app: &mut App, presenter: &mut Presenter, fb: &mut [u8], now: u64, input: Option<Input>, phases: &mut Phases| {
        let t = Instant::now();
        if let Some(i) = input {
            app.handle(i, now);
        }
        app.tick(now);
        let handle = t.elapsed();
        let mut sent = Vec::new();
        presenter.present(app, &fonts, fb, now, |u: Update| sent.push(u));
        let total = t.elapsed();
        let times = presenter.times;
        if opts.check_frames && !times.draw.is_zero() && frame_err.is_none() {
            let mut full = tiny_skia::Pixmap::new(presenter.frame().width(), presenter.frame().height()).unwrap();
            render::draw(&mut Canvas::new(&mut full, &fonts), app);
            if full.data() != presenter.frame().data() {
                frame_err = Some(format!("at {:.2} s: the frame is not the same as a full drawing", now as f64 / 1000.0));
            }
        }
        if input.is_some() {
            phases.add("handle", handle);
        }
        phases.add_nonzero("draw", times.draw);
        phases.add_nonzero("blit", times.blit);
        phases.add_nonzero("ink.draw", times.ink_draw);
        phases.add_nonzero("ink.blit", times.ink_blit);
        if !times.draw.is_zero() {
            // The main metric: from the input (or the timer) to the changed pixels.
            phases.add("input_to_pixels", total);
        }
        if !times.ink_draw.is_zero() || !times.ink_blit.is_zero() {
            phases.add("ink.step", total);
        }
        for u in &sent {
            updates.u64(now);
            for v in [u.0, u.1, u.2, u.3] {
                updates.u64(v as u64);
            }
        }
        update_count += sent.len() as u64;
    };

    let events: Vec<(u64, Input)> =
        script.timeline.events.iter().filter_map(|&(t, ev)| if let Ev::Input(i) = ev { Some((t, i)) } else { None }).collect();
    let end = events.last().map_or(0, |e| e.0).max(checks.clone().last().map_or(0, |c| c.0)) + IDLE_WAKE_MS;
    let mut events = events.into_iter().peekable();
    let mut verify_err = None;
    loop {
        // The next wake of the main loop: an input, a timer of the app, an ink timer, or the idle wake.
        let mut wake = now + IDLE_WAKE_MS;
        if let Some(d) = app.next_deadline() {
            wake = wake.min(d.max(now));
        }
        if let Some(w) = presenter.wait_ms(now) {
            wake = wake.min(now + w.max(1));
        }
        let input = match events.peek() {
            Some(&(et, i)) if et <= wake => {
                events.next();
                wake = et.max(now);
                Some(i)
            }
            _ => None,
        };
        if input.is_none() && wake > end {
            break;
        }
        now = wake;
        step(&mut app, &mut presenter, &mut fb, now, input, &mut phases);
        while let Some(&(ct, ci, c)) = checks.peek() {
            if ct > now {
                break;
            }
            if let Err(e) = script::verify(&app, c) {
                verify_err.get_or_insert(format!("at {:.2} s: {e}", ct as f64 / 1000.0));
            }
            frames.bytes(&fb);
            if let Some(dir) = &opts.dump {
                std::fs::write(dir.join(format!("frame-{ci:03}.rgb")), &fb).map_err(|e| e.to_string())?;
            }
            checks.next();
        }
    }
    if let Some(e) = verify_err.or(frame_err) {
        return Err(e);
    }
    frames.bytes(&fb);
    hashes.insert("script.frames", frames.hex());
    hashes.insert("script.updates", format!("{} {}", update_count, updates.hex()));

    // ---- Full drawings of the reference states.
    crate::reference::use_utc();
    let mut draw_hash = Hash::new();
    for (name, setup) in crate::reference::states().into_iter().chain([("ink", crate::reference::ink_state as crate::reference::Setup)]) {
        crate::layout::set_right_handed(false);
        let mut a = crate::reference::app(&fonts);
        setup(&mut a);
        let mut pm = tiny_skia::Pixmap::new(crate::layout::screen_w(), crate::layout::screen_h()).unwrap();
        for _ in 0..opts.draw_reps {
            let t = Instant::now();
            render::draw(&mut Canvas::new(&mut pm, &fonts), &a);
            phases.add("draw.states", t.elapsed());
            if std::env::var_os("BENCH_STATE_TIMES").is_some() {
                eprintln!("{name:40} {:?}", t.elapsed());
            }
        }
        if let Some(dir) = &opts.dump {
            std::fs::write(dir.join(format!("state-{name}.rgba")), pm.data()).map_err(|e| e.to_string())?;
        }
        draw_hash.bytes(pm.data());
    }
    crate::layout::set_right_handed(true);
    hashes.insert("draw.states", draw_hash.hex());

    // ---- The generator.
    if opts.gen_seeds > 0 {
        let mut h = Hash::new();
        for mode in 0..crate::generator::MODES {
            for seed in 0..opts.gen_seeds {
                let t = Instant::now();
                let p = crate::generator::generate(seed * 7919 + 13, mode);
                phases.add("gen", t.elapsed());
                for row in p.givens.iter().chain(p.solution.iter()) {
                    h.bytes(row);
                }
            }
        }
        hashes.insert("gen", h.hex());
    }

    // ---- The recognizer.
    if opts.recognize_step > 0 {
        let mut h = Hash::new();
        let mut correct = 0;
        let mut n = 0;
        for s in crate::recognizer::samples().iter().step_by(opts.recognize_step) {
            let t = Instant::now();
            let r = crate::recognizer::recognize(&s.strokes);
            phases.add("recognize", t.elapsed());
            h.u64(r.unwrap_or(0) as u64);
            // The distances also must not change (bit for bit).
            if let Some(m) = crate::recognizer::matches(&s.strokes) {
                h.u64(m.digit as u64);
                h.u64(m.digit_distance.to_bits() as u64);
                h.u64(m.other_distance.to_bits() as u64);
            }
            n += 1;
            correct += (r.unwrap_or(0) == s.class) as usize;
        }
        hashes.insert("recognize", format!("{correct}/{n} {}", h.hex()));
    }

    let phases: serde_json::Map<String, serde_json::Value> = phases.0.iter().map(|(k, v)| (k.to_string(), v.json())).collect();
    Ok(serde_json::json!({ "phases": phases, "hashes": hashes }))
}

/// Shows the report as a table (for a person).
pub fn table(report: &serde_json::Value) -> String {
    let mut out = format!("{:22} {:>6} {:>10} {:>10} {:>10} {:>10}\n", "phase", "n", "total ms", "p50 us", "p95 us", "max us");
    if let Some(p) = report["phases"].as_object() {
        for (k, v) in p {
            out += &format!(
                "{k:22} {:>6} {:>10} {:>10} {:>10} {:>10}\n",
                v["n"].to_string(),
                v["total_ms"].to_string(),
                v["p50_us"].to_string(),
                v["p95_us"].to_string(),
                v["max_us"].to_string()
            );
        }
    }
    if let Some(h) = report["hashes"].as_object() {
        for (k, v) in h {
            out += &format!("hash {k:17} {}\n", v.as_str().unwrap_or(""));
        }
    }
    out
}

/// The command line of the bench: `[OUT.json] [--script FILE] [--gen N] [--recognize-step N]
/// [--draw-reps N] [--dump DIR]`. It writes the table to stderr and the JSON report to OUT.json (or stdout).
pub fn main(args: Vec<String>, load_fonts: impl FnOnce() -> Result<Fonts, String>) -> Result<(), String> {
    let mut opts = Options::default();
    let mut out = None;
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        let mut value = || it.next().ok_or(format!("{a} needs a value"));
        let number = |v: String| v.parse::<u64>().map_err(|_| format!("incorrect number {v:?}"));
        match a.as_str() {
            "--script" => {
                let path = value()?;
                opts.script = std::fs::read_to_string(&path).map_err(|e| format!("{path}: {e}"))?;
            }
            "--gen" => opts.gen_seeds = number(value()?)?,
            "--recognize-step" => opts.recognize_step = number(value()?)? as usize,
            "--draw-reps" => opts.draw_reps = number(value()?)? as usize,
            "--dump" => {
                let dir = std::path::PathBuf::from(value()?);
                std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
                opts.dump = Some(dir);
            }
            _ if a.starts_with("--") => return Err(format!("unknown option {a}")),
            _ => out = Some(a),
        }
    }
    let report = run(load_fonts, &opts)?;
    eprint!("{}", table(&report));
    let json = serde_json::to_string_pretty(&report).unwrap();
    match out {
        Some(path) => std::fs::write(&path, json + "\n").map_err(|e| format!("{path}: {e}")),
        None => {
            println!("{json}");
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_redraw_of_the_demo_is_the_same_as_a_full_drawing() {
        // The presenter keeps a record of its frame and draws only the areas that can change
        // (render::FrameRecord). In debug builds, it also examines that the damage contains each
        // change of the frame.
        let opts = Options { gen_seeds: 0, recognize_step: 0, draw_reps: 0, check_frames: true, ..Options::default() };
        run(Fonts::repo, &opts).unwrap();
    }
}
