//! The device: AppLoad (qtfb) input and output, the save files and the main loop.

pub mod evdev;
pub mod qtfb;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};


use crate::app::{App, Input, Tool, DEFAULT_PAUSE_MS};
use crate::font::Fonts;
use crate::game::Game;
use crate::present::{self, Presenter};
use crate::render;
use crate::settings::Settings;
use crate::store::{self, ResultsDb};
use crate::system::SystemEnv;
use crate::update::{self, Manifest, UpdateRequest};

/// The longest wait for the rotation of the tablet before the first drawing.
const START_ROTATION_WAIT: Duration = Duration::from_millis(300);

/// The clock is saved at least this often (PER-7).
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
        Paths { game: dir.join("game.json"), settings: dir.join("settings.json"), results: dir.join(store::RESULTS_FILE) }
    }
}

/// The ink patch: a QML patch of AppLoad that shows the pen region (`appload/sudoku/sudoku-penregion.qmd`).
/// install.sh and the update (§9.2) copy it to `update::PATCH_DIR`.
fn pen_region_patch() -> PathBuf {
    Path::new(update::PATCH_DIR).join(update::PATCH_NAME)
}

/// True if xochitl applied the ink patch: the patch is installed, and xochitl (the parent process,
/// because AppLoad starts the app) started after the patch was copied. xochitl reads the patches only
/// when it starts. If the parent is not xochitl, the file is sufficient.
fn pen_region_patch_active() -> bool {
    let Ok(modified) = std::fs::metadata(pen_region_patch()).and_then(|m| m.modified()) else { return false };
    // SAFETY: getppid has no preconditions.
    let ppid = unsafe { libc::getppid() };
    let comm = std::fs::read_to_string(format!("/proc/{ppid}/comm")).unwrap_or_default();
    if comm.trim() != "xochitl" {
        return true;
    }
    // The boot time has whole seconds, thus the start time can be up to 1 s early. install.sh copies
    // the patch and starts xochitl again at once: a tolerance of 2 s accepts this.
    match process_start(ppid) {
        Some(started) => modified <= started + Duration::from_secs(2),
        None => true,
    }
}

/// The start time of the process `pid`: the boot time (/proc/stat "btime") plus the start time in
/// clock ticks after boot (/proc/PID/stat, field 22).
fn process_start(pid: libc::pid_t) -> Option<std::time::SystemTime> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // The second field (the name) can contain spaces: count the fields after the last ')'.
    let ticks: u64 = stat.rsplit_once(')')?.1.split_whitespace().nth(19)?.parse().ok()?;
    let btime: u64 = std::fs::read_to_string("/proc/stat")
        .ok()?
        .lines()
        .find_map(|l| l.strip_prefix("btime ").and_then(|v| v.trim().parse().ok()))?;
    // SAFETY: sysconf has no preconditions.
    let hz = unsafe { libc::sysconf(libc::_SC_CLK_TCK) }.max(1) as u64;
    Some(std::time::UNIX_EPOCH + Duration::from_secs(btime) + Duration::from_millis(ticks * 1000 / hz))
}

/// The file that gives the pen region to the patch: "x y width height" (physical pixels), or empty
/// for no region. The patch reads it each 15 ms.
const PEN_REGION_FILE: &str = "/tmp/sudoku-pen-region";

/// Writes the pen region for the patch. A rename replaces the file in one step, so that the patch
/// never reads a part of it.
fn write_pen_region(region: Option<present::Update>) {
    let text = region.map_or(String::new(), |(x, y, w, h)| format!("{x} {y} {w} {h}\n"));
    let tmp = format!("{PEN_REGION_FILE}.tmp");
    if let Err(e) = std::fs::write(&tmp, text).and_then(|_| std::fs::rename(&tmp, PEN_REGION_FILE)) {
        eprintln!("sudoku: cannot write {PEN_REGION_FILE}: {e}");
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

/// The result of update work on a background thread (REQUIREMENTS.md §9.2).
enum UpdateResult {
    Checked(Result<Manifest, String>),
    /// `Ok(true)`: the installation replaced the ink patch.
    Installed(Result<bool, String>),
}

/// Gets a file over HTTPS. GitHub sends a redirect to the file.
fn download(url: &str, timeout: Duration) -> Result<Vec<u8>, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(timeout)).https_only(true).build().into();
    let mut response = agent.get(url).call().map_err(|e| format!("{url}: {e}"))?;
    response.body_mut().with_config().limit(update::MAX_ARCHIVE_BYTES).read_to_vec().map_err(|e| format!("{url}: {e}"))
}

/// Starts the update work of `request` on a new thread. The result goes to `results`.
fn start_update(request: UpdateRequest, results: mpsc::Sender<UpdateResult>, staging: PathBuf, app_dir: Option<PathBuf>) {
    std::thread::spawn(move || {
        let result = match request {
            UpdateRequest::Check => UpdateResult::Checked((|| {
                let json = download(update::VERSION_URL, Duration::from_secs(30))?;
                let signature = download(update::SIGNATURE_URL, Duration::from_secs(30))?;
                update::read_manifest(&json, &signature, &update::PUBLIC_KEY)
            })()),
            UpdateRequest::Install(m) => UpdateResult::Installed((|| {
                let app_dir = app_dir.ok_or("the folder of the app is not known")?;
                eprintln!("sudoku: install {} from {} in {}", m.version, m.url, app_dir.display());
                let archive = download(&m.url, Duration::from_secs(300))?;
                let patch = match &m.patch {
                    Some(p) => Some(download(&p.url, Duration::from_secs(60))?),
                    None => None,
                };
                let changed = update::install_release(&archive, patch.as_deref(), &m, &staging, &app_dir, Path::new(update::PATCH_DIR))?;
                eprintln!("sudoku: installed {} (ink patch {})", m.version, if changed { "replaced" } else { "not changed" });
                Ok(changed)
            })()),
        };
        let _ = results.send(result);
    });
}

/// The script of xovi that starts xochitl again with the extensions and the patches.
const XOVI_START: &str = "/home/root/xovi/start";

/// Starts the reMarkable interface again with xovi, so that xochitl reads the new ink patch (§9.2,
/// "Restart"). xovi/start stops xochitl, and systemd then stops all processes of the xochitl
/// service, also this app and its children. Thus systemd-run starts the script in a unit of its own.
/// If systemd-run is not available, the script starts in a new session.
fn restart_interface() -> Result<(), String> {
    use std::process::{Command, Stdio};
    let log = || std::fs::File::create("/tmp/sudoku-xovi.log").map(Stdio::from).unwrap_or_else(|_| Stdio::null());
    let unit = Command::new("systemd-run")
        .args(["--collect", "--unit", "sudoku-xovi-start", XOVI_START])
        .stdin(Stdio::null())
        .stdout(log())
        .stderr(log())
        .status();
    match unit {
        Ok(s) if s.success() => return Ok(()),
        Ok(s) => eprintln!("sudoku: systemd-run {XOVI_START}: {s}"),
        Err(e) => eprintln!("sudoku: systemd-run {XOVI_START}: {e}"),
    }
    use std::os::unix::process::CommandExt;
    let mut cmd = Command::new(XOVI_START);
    cmd.stdin(Stdio::null()).stdout(log()).stderr(log());
    // SAFETY: setsid is async-signal-safe and has no preconditions.
    unsafe {
        cmd.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    cmd.spawn().map(|_| ()).map_err(|e| format!("{XOVI_START}: {e}"))
}

/// The AppLoad folder of the app: the folder of the program.
fn app_dir() -> Option<PathBuf> {
    std::env::current_exe().ok()?.parent().map(Path::to_path_buf)
}

/// A test setting with a value (the `remarkable-debug` skill, `techniques/app-settings.md`): the
/// environment variable `env` if it is set, else the contents of the file `file` in the data folder.
/// The caller trims and parses the text.
fn test_setting(env: &str, file: &str) -> Option<String> {
    std::env::var(env).ok().or_else(|| std::fs::read_to_string(store::data_dir().join(file)).ok())
}

/// A test switch: on when the environment variable `env` is set (to any value), or when the file
/// `file` is in the data folder. The app does not read the file.
fn test_switch(env: &str, file: &str) -> bool {
    std::env::var_os(env).is_some() || store::data_dir().join(file).exists()
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

/// `sudoku --self-test OUT.png [SECONDS]`: examines the device without qtfb. It loads the font of the
/// device, draws the state of images/screen-in-progress.png into OUT.png, reads one example of
/// handwriting, and watches the pen tools for SECONDS (default 0).
pub fn self_test(out: &str, seconds: u64) -> Result<(), String> {
    let t = Instant::now();
    let fonts = Fonts::load()?;
    let (easy_w, hard_w) = render::slider_label_widths(&fonts);
    println!("self-test: font loaded in {:?} (Easy: {:.2} px, Hard: {:.2} px)", t.elapsed(), easy_w, hard_w);
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
    // The toolbar is at the side of the hand that does not write (REQUIREMENTS.md §3.1).
    // SUDOKU_XOCHITL_CONF replaces the path of the file (for the tests).
    let conf_path = std::env::var("SUDOKU_XOCHITL_CONF").unwrap_or_else(|_| crate::system::XOCHITL_CONF.to_string());
    let conf = std::fs::read_to_string(conf_path).unwrap_or_default();
    crate::layout::set_right_handed(crate::system::conf_right_handed(&conf));
    let paths = Paths::new();
    std::fs::create_dir_all(paths.results.parent().unwrap()).map_err(|e| format!("cannot make the data folder: {e}"))?;
    let db = ResultsDb::open(&paths.results).map_err(|e| format!("cannot open the results database: {e}"))?;
    let saved: Option<Game> = store::load_json(&paths.game);
    let settings: Option<Settings> = store::load_json(&paths.settings);
    let mut app = App::new(saved, settings, db, Box::new(SystemEnv::new()), render::ng_stops(&fonts));
    let game_path = paths.game.clone();
    // If the file `log-ink` is in the data folder, each reading of ink goes to `ink-log.jsonl` (one JSON
    // line: the strokes and the result). It is only for an examination of the recognizer.
    let data_dir = store::data_dir();
    if data_dir.join("log-ink").exists() {
        let log_path = data_dir.join("ink-log.jsonl");
        app.ink_log = Some(Box::new(move |cell, strokes, result| {
            let line = serde_json::json!({ "cell": cell, "strokes": strokes, "result": result });
            if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&log_path) {
                use std::io::Write;
                let _ = writeln!(f, "{line}");
            }
        }));
    }
    app.save_game_now = Some(Box::new(move |g: &Game| match store::save_json(&game_path, g) {
        Ok(()) => true,
        Err(e) => {
            eprintln!("sudoku: cannot save the game: {e}");
            false
        }
    }));
    // The corpus build: a blank board, and each ink goes to the corpus log (corpus.rs).
    #[cfg(feature = "corpus")]
    crate::corpus::setup(&mut app, &data_dir);
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
    let mut presenter = Presenter::new();
    // The ink patch shows the pen region. Without it, the app does not use one.
    presenter.use_pen_region = pen_region_patch_active();
    eprintln!("sudoku: pen region {}", if presenter.use_pen_region { "on" } else { "off" });
    write_pen_region(None);
    // For a test on the device: SUDOKU_INK_WAVE_MS, or the file `ink-wave-ms` in the data folder,
    // changes the time that the app gives an update to complete (default present::INK_WAVE_MS).
    if let Some(ms) = test_setting("SUDOKU_INK_WAVE_MS", "ink-wave-ms").and_then(|s| s.trim().parse::<u64>().ok()) {
        eprintln!("sudoku: ink wave time {ms} ms");
        presenter.set_wave_ms(ms);
    }
    // One refresh mode for the full time: a change of the mode makes AppLoad refresh the full window,
    // which shows as a flash. For a test on the device, SUDOKU_REFRESH_MODE, or the file
    // `refresh-mode` in the data folder, selects another mode (refer to `RefreshMode::from_name`).
    let refresh_mode = test_setting("SUDOKU_REFRESH_MODE", "refresh-mode")
        .and_then(|name| qtfb::RefreshMode::from_name(&name))
        .unwrap_or(qtfb::RefreshMode::Ui);
    eprintln!("sudoku: refresh mode {refresh_mode:?}");
    conn.set_refresh_mode(refresh_mode).ok();
    presenter.draw_all(&app, &fonts, conn.pixels());
    conn.update_all().map_err(|e| format!("qtfb update: {e}"))?;
    for m in early_input {
        if let Some(input) = to_input(&m, &eraser) {
            app.handle(input, now_ms());
        }
    }
    let mut last_clock_save = Instant::now();
    // SUDOKU_LOG_INPUT=1, or the file `log-input` in the data folder, writes each input message to the
    // system log (for an examination on the device).
    let log_input = test_switch("SUDOKU_LOG_INPUT", "log-input");
    // Updates (REQUIREMENTS.md §9.2). The folder of the program is found now: an installation renames
    // it. Each build deletes the folders that stay from an earlier installation.
    let app_dir = app_dir();
    let staging = store::data_dir().join("update");
    if let Some(dir) = app_dir.as_deref() {
        update::clean(&staging, dir);
    }
    let (update_tx, update_rx) = mpsc::channel();

    while !app.quit && !STOP.load(Ordering::SeqCst) {
        // Wait for input, a timer of the app, or the next clock save.
        let now = now_ms();
        let mut wait = app.next_deadline().map(|d| d.saturating_sub(now)).unwrap_or(1000).min(1000);
        if let Some(w) = presenter.wait_ms(now) {
            // The ink timers have ms steps: wait one ms more, so that the loop does not spin.
            wait = wait.min(w + 1);
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
                        presenter.reset();
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
        if let Some(request) = app.update.request.take() {
            start_update(request, update_tx.clone(), staging.clone(), app_dir.clone());
        }
        while let Ok(result) = update_rx.try_recv() {
            match result {
                UpdateResult::Checked(r) => app.update_checked(r),
                UpdateResult::Installed(r) => app.update_installed(r, now_ms()),
            }
        }

        if last_clock_save.elapsed() >= CLOCK_SAVE_INTERVAL && !app.game.completed {
            app.game_dirty = true;
            last_clock_save = Instant::now();
        }
        save(&mut app, &paths);
        // "Restart" in the dialog "Restart the interface?": the game is saved now. The app continues
        // until xovi/start stops xochitl.
        if app.restart_ui {
            app.restart_ui = false;
            if let Err(e) = restart_interface() {
                eprintln!("sudoku: cannot restart the interface: {e}");
                app.restart_failed();
            }
        }

        let mut updates = Vec::new();
        presenter.present(&mut app, &fonts, conn.pixels(), now_ms(), |u| updates.push(u));
        if let Some(region) = presenter.take_region_change() {
            write_pen_region(region);
        }
        for (x, y, w, h) in updates {
            conn.update(x, y, w, h).ok();
        }
    }
    write_pen_region(None);

    eprintln!("sudoku: the app stops (quit {}, signal {})", app.quit, STOP.load(Ordering::SeqCst));
    // The app closes: ink that waits is removed (PER-6) and the game is saved (PER-1, NAV-6).
    app.tick(now_ms());
    app.suspend();
    save(&mut app, &paths);
    Ok(())
}
