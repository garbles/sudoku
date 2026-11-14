//! An end-to-end test of the device path on Linux: a fake qtfb server (as AppLoad) starts the real
//! executable, reads its framebuffer, sends pen and touch input and closes it. Then the test reads the
//! saved data (PER-1, NAV-6). The main loop is only on Linux, thus the clock ticks and the clock saves
//! of the platform (CLK-6, PER-7) are also examined here.
//!
//! The program runs in the two modes of LCH-9. With no argument, it is an external app (`QTFB_KEY`).
//! With the path of a socket as argument 1, it is the backend of the QML frontend: a fake AppLoad sends
//! the messages of R§14.4 on that socket (LCH-1, LCH-2, LCH-5).
#![cfg(target_os = "linux")]

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use sudoku::game::{cells, mark_bit, Game};

const W: usize = 1620;
const H: usize = 2160;
const KEY: i32 = 4242;

/// The fake server uses the fixed socket path of qtfb. Only one test can use it at a time.
static SERVER: Mutex<()> = Mutex::new(());

/// The C `qtfb::ServerMessage` on a 64-bit system: uint8_t type, then a union with alignment 8 at
/// offset 8. The full size is 32 bytes.
fn server_message(kind: u8, body: &[u8]) -> [u8; 32] {
    let mut m = [0u8; 32];
    m[0] = kind;
    m[8..8 + body.len()].copy_from_slice(body);
    m
}

struct Server {
    listen: i32,
    conn: i32,
    shm: *mut u8,
    /// The framebuffer ID of the shared memory, after `accept_key`.
    key: Option<i32>,
}

/// The size of the shared memory of the framebuffer (FBFMT_RMPP_RGB888).
const SHM_SIZE: usize = W * H * 3;

fn shm_name(key: i32) -> std::ffi::CString {
    std::ffi::CString::new(format!("/qtfb_{key}")).unwrap()
}

/// Releases the shared memory when the server goes away. A mapping keeps its memory in `/dev/shm`
/// also after `shm_unlink`. In a Docker container, `/dev/shm` has only 64 MB, thus the mappings of
/// approximately 6 servers fill it, and the next write gives SIGBUS. Each test that uses a server holds
/// the lock `SERVER`, thus the removal of the name does not affect a different test.
impl Drop for Server {
    fn drop(&mut self) {
        unsafe {
            if !self.shm.is_null() {
                libc::munmap(self.shm as *mut _, SHM_SIZE);
            }
            if let Some(key) = self.key {
                libc::shm_unlink(shm_name(key).as_ptr());
            }
            if self.conn >= 0 {
                libc::close(self.conn);
            }
            libc::close(self.listen);
        }
    }
}

impl Server {
    fn new() -> Server {
        unsafe {
            let path = "/tmp/qtfb.sock";
            let _ = std::fs::remove_file(path);
            let fd = libc::socket(libc::AF_UNIX, libc::SOCK_SEQPACKET, 0);
            assert!(fd >= 0);
            let mut addr: libc::sockaddr_un = std::mem::zeroed();
            addr.sun_family = libc::AF_UNIX as _;
            for (i, b) in path.bytes().enumerate() {
                addr.sun_path[i] = b as _;
            }
            assert_eq!(libc::bind(fd, &addr as *const _ as *const _, std::mem::size_of::<libc::sockaddr_un>() as _), 0);
            assert_eq!(libc::listen(fd, 1), 0);
            Server { listen: fd, conn: -1, shm: std::ptr::null_mut(), key: None }
        }
    }

    /// Accepts the client and answers its init message with a framebuffer.
    fn accept(&mut self) {
        self.accept_key(KEY);
    }

    /// True if a client tries to connect now.
    fn pending(&self) -> bool {
        let mut pfd = libc::pollfd { fd: self.listen, events: libc::POLLIN, revents: 0 };
        unsafe { libc::poll(&mut pfd, 1, 0) > 0 }
    }

    /// Reads the client messages that are here, with no wait.
    fn drain(&self) {
        if self.conn < 0 {
            return;
        }
        let mut buf = [0u8; 64];
        while unsafe { libc::recv(self.conn, buf.as_mut_ptr() as *mut _, 64, libc::MSG_DONTWAIT) } > 0 {}
    }

    /// Accepts the client and answers its init message with a framebuffer. The client must ask for
    /// the framebuffer `key`.
    fn accept_key(&mut self, key: i32) {
        unsafe {
            self.conn = libc::accept(self.listen, std::ptr::null_mut(), std::ptr::null_mut());
            assert!(self.conn >= 0);
            let tv = libc::timeval { tv_sec: 10, tv_usec: 0 };
            libc::setsockopt(self.conn, libc::SOL_SOCKET, libc::SO_RCVTIMEO, &tv as *const _ as *const _, std::mem::size_of::<libc::timeval>() as _);
            let mut buf = [0u8; 64];
            let n = libc::recv(self.conn, buf.as_mut_ptr() as *mut _, 64, 0);
            assert_eq!(n, 24, "init message size");
            assert_eq!(buf[0], 0, "MESSAGE_INITIALIZE");
            assert_eq!(i32::from_ne_bytes(buf[4..8].try_into().unwrap()), key, "the framebuffer ID");
            assert_eq!(buf[8], 1, "FBFMT_RMPP_RGB888");
            let size = SHM_SIZE;
            let name = shm_name(key);
            libc::shm_unlink(name.as_ptr());
            let shm_fd = libc::shm_open(name.as_ptr(), libc::O_RDWR | libc::O_CREAT, 0o666);
            assert!(shm_fd >= 0);
            self.key = Some(key);
            assert_eq!(libc::ftruncate(shm_fd, size as _), 0);
            let p = libc::mmap(std::ptr::null_mut(), size, libc::PROT_READ | libc::PROT_WRITE, libc::MAP_SHARED, shm_fd, 0);
            libc::close(shm_fd);
            assert_ne!(p, libc::MAP_FAILED);
            std::ptr::write_bytes(p as *mut u8, 0xFF, size);
            self.shm = p as *mut u8;
            // InitMessageResponseContents: int shmKeyDefined, then size_t shmSize at offset 8.
            let mut body = [0u8; 16];
            body[0..4].copy_from_slice(&key.to_ne_bytes());
            body[8..16].copy_from_slice(&size.to_ne_bytes());
            let resp = server_message(0, &body);
            libc::send(self.conn, resp.as_ptr() as *const _, 32, 0);
        }
    }

    /// Reads client messages until an update arrives. Returns false if the client closed.
    fn wait_update(&self) -> bool {
        loop {
            let mut buf = [0u8; 64];
            let n = unsafe { libc::recv(self.conn, buf.as_mut_ptr() as *mut _, 64, 0) };
            if n <= 0 {
                return false;
            }
            if buf[0] == 1 {
                return true;
            }
        }
    }

    fn pixel(&self, x: usize, y: usize) -> [u8; 3] {
        let i = (y * W + x) * 3;
        unsafe { [*self.shm.add(i), *self.shm.add(i + 1), *self.shm.add(i + 2)] }
    }

    fn send(&self, kind: i32, x: i32, y: i32) {
        let body: Vec<u8> = [kind, 0, x, y, 50].iter().flat_map(|v| v.to_ne_bytes()).collect();
        let msg = server_message(4, &body);
        unsafe { libc::send(self.conn, msg.as_ptr() as *const _, 32, 0) };
    }

    fn touch_tap(&self, x: i32, y: i32) {
        self.send(0x10, x, y);
        self.send(0x11, x, y);
    }

    fn pen_tap(&self, x: i32, y: i32) {
        self.send(0x20, x, y);
        self.send(0x21, x, y);
    }
}

fn wait_until(mut f: impl FnMut() -> bool) -> bool {
    let end = Instant::now() + Duration::from_secs(10);
    while Instant::now() < end {
        if f() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

#[test]
fn the_executable_operates_through_qtfb() {
    let _lock = SERVER.lock().unwrap_or_else(|e| e.into_inner());
    let data = std::env::temp_dir().join(format!("sudoku-e2e-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&data);
    let mut server = Server::new();
    let mut child = app_command(&data).stderr(Stdio::inherit()).spawn().expect("start the app");
    server.accept();
    assert!(server.wait_update(), "first frame");

    // The board: its left border line is black, the screen at the left of the board is white.
    let (bx, by) = (sudoku::layout::board_x() as usize, sudoku::layout::board_y() as usize);
    assert_eq!(server.pixel(bx, by + 600), [0, 0, 0]);
    assert_eq!(server.pixel(5, 1000), [255, 255, 255]);
    // PER-2: the app opens with the menu open, thus the close button shows.
    assert_ne!(server.pixel(38, 55), [255, 255, 255], "the close button shows at the start");
    // A finger closes the menu: the close button does not show while the menu is closed.
    server.touch_tap(1563, 55);
    assert!(wait_until(|| server.pixel(38, 55) == [255, 255, 255]), "the close button does not show");

    // The game is saved at the start. Find an empty cell.
    let game_path = data.join("game.json");
    assert!(wait_until(|| game_path.exists()));
    let g: Game = serde_json::from_slice(&std::fs::read(&game_path).unwrap()).unwrap();
    assert_eq!(g.mode, 2);
    // An empty cell where no peer has the number 5 (MRK-7).
    let cell = cells().find(|&c| !g.is_filled(c) && g.peer_digits(c) & mark_bit(5) == 0).unwrap();
    let (x, y) = sudoku::layout::cell_origin(cell);

    // A pen tap on micro-cell 5 adds a pencil mark.
    server.pen_tap(x + 73, y + 73);
    // INP-20: a touch just after a pen lift is the hand. A finger comes after that time.
    std::thread::sleep(Duration::from_millis(sudoku::app::HAND_TOUCH_MS + 50));
    // A finger opens the menu again: the close button shows.
    server.touch_tap(1563, 55);
    assert!(wait_until(|| server.pixel(38, 55) != [255, 255, 255]), "the close button shows");

    // The close button: only the pen operates it (NAV-6). The app saves and stops.
    server.pen_tap(55, 55);
    let status = loop {
        if let Some(s) = child.try_wait().unwrap() {
            break s;
        }
        server.wait_update();
    };
    assert!(status.success());
    let g2: Game = serde_json::from_slice(&std::fs::read(&game_path).unwrap()).unwrap();
    assert_eq!(g2.marks_at(cell), mark_bit(5));
    assert_eq!(g2.seed, g.seed);
    assert!(data.join("results.sqlite").exists());

    // The app opens again with the same game, and SIGTERM (AppLoad closes the app) saves it.
    drop(server);
    let mut server2 = Server::new();
    let mut child = app_command(&data).spawn().unwrap();
    server2.accept();
    assert!(server2.wait_update());
    assert_ne!(server2.pixel(38, 55), [255, 255, 255], "PER-2: a saved game opens with the menu open");
    std::thread::sleep(Duration::from_millis(1500));
    unsafe { libc::kill(child.id() as i32, libc::SIGTERM) };
    assert!(child.wait().unwrap().success());
    let g3: Game = serde_json::from_slice(&std::fs::read(&game_path).unwrap()).unwrap();
    assert_eq!((g3.seed, g3.value, g3.marks), (g2.seed, g2.value, g2.marks));
    assert!(g3.elapsed_ms > g2.elapsed_ms, "the clock continues from the saved time");
    let _ = std::fs::remove_dir_all(&data);
}

/// The command that starts the executable as AppLoad does, with the data folder `data`, the fonts of
/// `scripts/fonts/` and the left-handed setting of the fixture.
fn app_command(data: &Path) -> Command {
    let root = env!("CARGO_MANIFEST_DIR");
    let mut command = Command::new(env!("CARGO_BIN_EXE_sudoku"));
    command
        .env("QTFB_KEY", KEY.to_string())
        .env("SUDOKU_DATA_DIR", data)
        .env("SUDOKU_FONT", format!("{root}/scripts/fonts/rmsans-400.ttf"))
        .env("SUDOKU_FONT_MEDIUM", format!("{root}/scripts/fonts/rmsans-500.ttf"))
        .env("SUDOKU_XOCHITL_CONF", format!("{root}/tests/fixtures/xochitl-left-handed.conf"));
    command
}

/// Starts the executable with the data folder `data`, and waits for its first frame.
fn start(data: &Path) -> (Server, Child) {
    let mut server = Server::new();
    let child = app_command(data).stderr(Stdio::inherit()).spawn().expect("start the app");
    server.accept();
    assert!(server.wait_update(), "first frame");
    (server, child)
}

fn read_game(path: &Path) -> Game {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

/// Sends a signal and waits until the app stops. The app must stop with success.
fn stop_with(child: &mut Child, signal: libc::c_int) {
    unsafe { libc::kill(child.id() as i32, signal) };
    let end = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success(), "signal {signal}: {status}");
            return;
        }
        assert!(Instant::now() < end, "signal {signal}: the app does not stop");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn the_clock_ticks_and_is_saved_with_no_input() {
    // CLK-6: with no input, the app ticks at least one time each second; thus no gap of more than 5 s
    // occurs and the clock counts all the time. PER-7: the app saves the clock at least each 5 seconds
    // (a power loss loses a maximum of 5 s of the clock), and it saves the game on SIGINT and SIGHUP.
    let _lock = SERVER.lock().unwrap_or_else(|e| e.into_inner());
    let data = std::env::temp_dir().join(format!("sudoku-e2e-clock-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&data);
    let game_path = data.join("game.json");
    let (_server, mut child) = start(&data);
    let shown = Instant::now();
    assert!(wait_until(|| game_path.exists()));
    assert_eq!(read_game(&game_path).elapsed_ms, 0, "a new game is saved at the start");

    // Read the save file while the app runs with no input. Keep each new value of the clock.
    const RUN: Duration = Duration::from_millis(11_500);
    // The clock starts at the first tick after the first frame (a maximum of 1 s), and the reading of
    // a file has a small delay.
    const SLACK_MS: u64 = 1500;
    let mut saves: Vec<u64> = Vec::new();
    while shown.elapsed() < RUN {
        let saved = read_game(&game_path).elapsed_ms;
        let wall = shown.elapsed().as_millis() as u64;
        assert!(saved <= wall, "the clock {saved} ms is more than the time {wall} ms");
        assert!(wall - saved <= 5000 + SLACK_MS, "PER-7: after {wall} ms the saved clock is {saved} ms");
        if saved > 0 && saves.last() != Some(&saved) {
            saves.push(saved);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(saves.len() >= 2, "PER-7: two clock saves in {RUN:?}: {saves:?}");
    for w in saves.windows(2) {
        let step = w[1] - w[0];
        // The save occurs at the first tick 5 s after the last save. If the app ticked less often than
        // each second, the step would be 6 s or more.
        assert!((5000..5800).contains(&step), "CLK-6, PER-7: the clock saves are {saves:?}");
    }

    // SIGINT saves the clock: the next clock save is not yet due.
    let before = *saves.last().unwrap();
    // The clock stops at the signal: the time to close the app (the last save) is not on the clock.
    let at_stop = shown.elapsed().as_millis() as u64;
    stop_with(&mut child, libc::SIGINT);
    let g = read_game(&game_path);
    assert!(g.elapsed_ms > before, "SIGINT saves the clock: {} ms, last clock save {before} ms", g.elapsed_ms);
    assert!(g.elapsed_ms + SLACK_MS >= at_stop, "CLK-6: the clock {} ms counts all of {at_stop} ms", g.elapsed_ms);

    // SIGHUP saves the clock. The app opens the saved game and saves it at the start; 2.5 s is less
    // than the clock save interval.
    let (_server, mut child) = start(&data);
    std::thread::sleep(Duration::from_millis(2500));
    assert_eq!(read_game(&game_path).elapsed_ms, g.elapsed_ms, "no clock save before 5 s");
    stop_with(&mut child, libc::SIGHUP);
    let g2 = read_game(&game_path);
    assert_eq!((g2.seed, g2.value, g2.marks), (g.seed, g.value, g.marks));
    assert!(g2.elapsed_ms >= g.elapsed_ms + 1000, "SIGHUP saves the clock: {} ms after {} ms", g2.elapsed_ms, g.elapsed_ms);
    let _ = std::fs::remove_dir_all(&data);
}

// ---------------------------------------------------------------------------------------------
// The frontend mode (LCH-9): a fake AppLoad starts the program as the backend of the QML frontend.

/// The framebuffer ID of Sudoku (LCH-5). In the frontend mode, the program uses only this ID.
const FRONTEND_KEY: i32 = 0x5355444F;

/// The pen area file of the external mode (LCH-9). The frontend mode deletes it (UPD-11).
const PEN_AREA_FILE: &str = "/tmp/sudoku-pen-region";

/// The fake AppLoad: the UNIX socket (SOCK_SEQPACKET) of the backend messages (R§14.4). Each message
/// is two packets: a header (the type and the length of the text, two `int`) and the text.
struct Appload {
    listen: i32,
    conn: i32,
    path: PathBuf,
}

impl Appload {
    fn new(name: &str) -> Appload {
        let path = std::env::temp_dir().join(format!("sudoku-e2e-appload-{name}-{}.sock", std::process::id()));
        let _ = std::fs::remove_file(&path);
        unsafe {
            let fd = libc::socket(libc::AF_UNIX, libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC, 0);
            assert!(fd >= 0);
            let mut addr: libc::sockaddr_un = std::mem::zeroed();
            addr.sun_family = libc::AF_UNIX as _;
            for (i, b) in path.to_str().unwrap().bytes().enumerate() {
                addr.sun_path[i] = b as _;
            }
            assert_eq!(libc::bind(fd, &addr as *const _ as *const _, std::mem::size_of::<libc::sockaddr_un>() as _), 0);
            assert_eq!(libc::listen(fd, 1), 0);
            Appload { listen: fd, conn: -1, path }
        }
    }

    /// Waits up to 10 s for the program to connect.
    fn accept(&mut self) {
        unsafe {
            let mut pfd = libc::pollfd { fd: self.listen, events: libc::POLLIN, revents: 0 };
            assert_eq!(libc::poll(&mut pfd, 1, 10_000), 1, "the program connects to AppLoad");
            // CLOEXEC: a program that a different test starts must not keep the connection open.
            self.conn = libc::accept4(self.listen, std::ptr::null_mut(), std::ptr::null_mut(), libc::SOCK_CLOEXEC);
            assert!(self.conn >= 0);
            let tv = libc::timeval { tv_sec: 0, tv_usec: 200_000 };
            libc::setsockopt(self.conn, libc::SOL_SOCKET, libc::SO_RCVTIMEO, &tv as *const _ as *const _, std::mem::size_of::<libc::timeval>() as _);
        }
    }

    fn send(&self, kind: i32, text: &str) {
        assert!(!text.is_empty(), "AppLoad sends no empty text");
        let header: Vec<u8> = [kind, text.len() as i32].iter().flat_map(|v| v.to_ne_bytes()).collect();
        unsafe {
            assert_eq!(libc::send(self.conn, header.as_ptr() as *const _, header.len(), libc::MSG_NOSIGNAL), 8);
            assert_eq!(libc::send(self.conn, text.as_ptr() as *const _, text.len(), libc::MSG_NOSIGNAL), text.len() as isize);
        }
    }

    /// The next message of the program, in a maximum of 200 ms. None: no message.
    fn recv(&self) -> Option<(i32, String)> {
        let mut buf = [0u8; 256];
        let n = unsafe { libc::recv(self.conn, buf.as_mut_ptr() as *mut _, buf.len(), 0) };
        if n <= 0 {
            return None;
        }
        assert_eq!(n, 8, "a header packet: {:?}", &buf[..n as usize]);
        let kind = i32::from_ne_bytes(buf[0..4].try_into().unwrap());
        let len = i32::from_ne_bytes(buf[4..8].try_into().unwrap());
        let n = unsafe { libc::recv(self.conn, buf.as_mut_ptr() as *mut _, buf.len(), 0) };
        assert!(n > 0, "R§14.4: the text of a message is never empty");
        assert_eq!(n, len as isize, "the length of the header");
        Some((kind, String::from_utf8(buf[..n as usize].to_vec()).unwrap()))
    }

    /// Reads the messages of the program for a maximum of `ms`, until `want` accepts one.
    fn recv_until(&self, ms: u64, mut want: impl FnMut(i32, &str) -> bool) -> Option<(i32, String)> {
        let end = Instant::now() + Duration::from_millis(ms);
        while Instant::now() < end {
            if let Some((kind, text)) = self.recv() {
                if want(kind, &text) {
                    return Some((kind, text));
                }
            }
        }
        None
    }

    /// Closes the connection: the end of the connection for the program.
    fn close_connection(&mut self) {
        unsafe { libc::close(self.conn) };
        self.conn = -1;
    }
}

impl Drop for Appload {
    fn drop(&mut self) {
        unsafe {
            if self.conn >= 0 {
                libc::close(self.conn);
            }
            libc::close(self.listen);
        }
        let _ = std::fs::remove_file(&self.path);
    }
}

/// The program, stopped at the end of the test also when an assertion fails.
struct Running(Child);

impl Drop for Running {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

impl Running {
    /// Waits until the program stops, for a maximum of `limit`. It reads the framebuffer messages,
    /// so that the program does not wait for the fake server.
    fn wait_stop(&mut self, server: &Server, limit: Duration) -> std::process::ExitStatus {
        let end = Instant::now() + limit;
        loop {
            if let Some(status) = self.0.try_wait().unwrap() {
                return status;
            }
            assert!(Instant::now() < end, "the program does not stop in {limit:?}");
            server.drain();
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn running(&mut self) -> bool {
        self.0.try_wait().unwrap().is_none()
    }
}

/// The command that starts the program as `backend/entry` does: with the socket path of AppLoad as
/// argument 1. `QTFB_KEY` has a different value: the program must not use it (LCH-5).
fn frontend_command(data: &Path, socket: &Path) -> Command {
    let mut command = app_command(data);
    command.arg(socket).stderr(Stdio::inherit());
    command
}

/// A data folder with the settings `restart_dialog_shown = true`: UPD-13 sets the value to false at a
/// start in the frontend mode.
fn frontend_data(name: &str) -> PathBuf {
    let data = std::env::temp_dir().join(format!("sudoku-e2e-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&data);
    std::fs::create_dir_all(&data).unwrap();
    let settings = sudoku::settings::Settings { restart_dialog_shown: true, ..Default::default() };
    std::fs::write(data.join("settings.json"), serde_json::to_string(&settings).unwrap()).unwrap();
    data
}

fn read_settings(data: &Path) -> sudoku::settings::Settings {
    serde_json::from_slice(&std::fs::read(data.join("settings.json")).unwrap()).unwrap()
}

#[test]
fn the_frontend_mode_saves_and_stops_when_the_window_closes() {
    // LCH-1: in the frontend mode, the program saves the game and stops for the stop message, for a
    // "frontend closed" message with the count 0 and for the end of the connection. A count that is
    // not 0 does not stop it. LCH-5: it connects to qtfb only with its framebuffer ID, after the first
    // "frontend connected" message (R§14.4). LCH-9: it gives the pen area as a message of type 1
    // ("none" or "x y width height"), and it does not write /tmp/sudoku-pen-region. UPD-11: it deletes
    // that file. UPD-13: it sets restart_dialog_shown to false.
    let _lock = SERVER.lock().unwrap_or_else(|e| e.into_inner());
    for cause in ["the stop message", "the last frontend closed", "the end of the connection"] {
        let data = frontend_data("frontend");
        std::fs::write(PEN_AREA_FILE, "1 2 3 4").unwrap();
        let mut appload = Appload::new("close");
        let mut server = Server::new();
        let mut child = Running(frontend_command(&data, &appload.path).spawn().expect("start the app"));
        appload.accept();
        // No framebuffer before a frontend connects.
        std::thread::sleep(Duration::from_millis(300));
        assert!(!server.pending(), "{cause}: R§14.4: the program connects to qtfb after the first frontend");
        appload.send(-2, "1");
        server.accept_key(FRONTEND_KEY);
        assert!(server.wait_update(), "{cause}: first frame");
        let (bx, by) = (sudoku::layout::board_x() as usize, sudoku::layout::board_y() as usize);
        assert_eq!(server.pixel(bx, by + 600), [0, 0, 0], "{cause}: the board shows");
        assert_eq!(appload.recv_until(3000, |kind, _| kind == 1), Some((1, "none".to_string())), "{cause}: LCH-9: no area at the start");
        assert!(!Path::new(PEN_AREA_FILE).exists(), "{cause}: UPD-11: the pen area file is deleted");

        // A pencil mark with the pen. The press in the cell gives the pen area of that cell.
        let game_path = data.join("game.json");
        assert!(wait_until(|| game_path.exists()));
        let g = read_game(&game_path);
        let cell = cells().find(|&c| !g.is_filled(c) && g.peer_digits(c) & mark_bit(5) == 0).unwrap();
        let (x, y) = sudoku::layout::cell_origin(cell);
        let (px, py) = (x + 73, y + 73);
        server.send(0x20, px, py);
        let area = appload.recv_until(3000, |kind, text| kind == 1 && text != "none");
        server.send(0x21, px, py);
        let (_, text) = area.unwrap_or_else(|| panic!("{cause}: LCH-9: a pen press in a cell gives a pen area"));
        let r: Vec<i32> = text.split(' ').map(|v| v.parse().unwrap()).collect();
        assert_eq!(r.len(), 4, "{cause}: \"x y width height\": {text}");
        assert!(r[0] <= px && px < r[0] + r[2] && r[1] <= py && py < r[1] + r[3], "{cause}: LCH-3: the area {text} has the pen at ({px}, {py})");
        assert!(!Path::new(PEN_AREA_FILE).exists(), "{cause}: LCH-9: no pen area file in the frontend mode");
        std::thread::sleep(Duration::from_millis(300));
        server.drain();

        match cause {
            "the stop message" => appload.send(-1, "stop"),
            "the last frontend closed" => {
                appload.send(-2, "2");
                appload.send(-3, "1");
                std::thread::sleep(Duration::from_millis(500));
                server.drain();
                assert!(child.running(), "LCH-1: one frontend stays");
                appload.send(-3, "0");
            }
            _ => appload.close_connection(),
        }
        let status = child.wait_stop(&server, Duration::from_secs(10));
        assert!(status.success(), "{cause}: {status}");
        let saved = read_game(&game_path);
        assert_eq!(saved.seed, g.seed, "{cause}");
        assert_eq!(saved.marks_at(cell), mark_bit(5), "{cause}: LCH-1: the game is saved");
        assert!(!read_settings(&data).restart_dialog_shown, "{cause}: UPD-13");
        let _ = std::fs::remove_dir_all(&data);
    }
}

#[test]
fn the_frontend_mode_stops_before_a_frontend_connects() {
    // LCH-1: the stop message and the end of the connection also stop a program whose frontend did
    // not connect yet. The program saves the game, and it does not connect to qtfb.
    let _lock = SERVER.lock().unwrap_or_else(|e| e.into_inner());
    for cause in ["the stop message", "the end of the connection"] {
        let data = frontend_data("early");
        let mut appload = Appload::new("early");
        let server = Server::new();
        let mut child = Running(frontend_command(&data, &appload.path).spawn().expect("start the app"));
        appload.accept();
        std::thread::sleep(Duration::from_millis(300));
        let start = Instant::now();
        if cause == "the stop message" {
            appload.send(-1, "stop");
        } else {
            appload.close_connection();
        }
        let status = child.wait_stop(&server, Duration::from_secs(5));
        assert!(status.success(), "{cause}: {status}");
        assert!(start.elapsed() < Duration::from_secs(3), "{cause}: the program stops at once: {:?}", start.elapsed());
        assert!(!server.pending(), "{cause}: no framebuffer");
        assert!(data.join("game.json").exists(), "{cause}: the game is saved");
        let _ = std::fs::remove_dir_all(&data);
    }
}

#[test]
fn the_frontend_mode_stops_when_no_frontend_connects_in_10_s() {
    // LCH-2, LCH-10: no frontend in 10 s after the start: the program saves the game and stops. It
    // does not connect to qtfb: no window shows.
    let _lock = SERVER.lock().unwrap_or_else(|e| e.into_inner());
    let data = frontend_data("late");
    let mut appload = Appload::new("late");
    let server = Server::new();
    let start = Instant::now();
    let mut child = Running(frontend_command(&data, &appload.path).spawn().expect("start the app"));
    appload.accept();
    std::thread::sleep(Duration::from_millis(9_000));
    assert!(child.running(), "LCH-10: the program waits 10 s");
    let status = child.wait_stop(&server, Duration::from_secs(5));
    let after = start.elapsed();
    assert!(status.success(), "{status}");
    assert!(after >= Duration::from_millis(9_900) && after < Duration::from_secs(13), "LCH-10: the program stopped after {after:?}");
    assert!(!server.pending(), "no framebuffer");
    assert!(data.join("game.json").exists(), "LCH-2: the game is saved");
    let _ = std::fs::remove_dir_all(&data);
}
