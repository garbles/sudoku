//! An end-to-end test of the device path on Linux: a fake qtfb server (as AppLoad) starts the real
//! executable, reads its framebuffer, sends pen and touch input and closes it. Then the test reads the
//! saved data (PER-1, NAV-6).
#![cfg(target_os = "linux")]

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use sudoku::game::{cells, mark_bit, Game};

const W: usize = 1620;
const H: usize = 2160;
const KEY: i32 = 4242;

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
            Server { listen: fd, conn: -1, shm: std::ptr::null_mut() }
        }
    }

    /// Accepts the client and answers its init message with a framebuffer.
    fn accept(&mut self) {
        unsafe {
            self.conn = libc::accept(self.listen, std::ptr::null_mut(), std::ptr::null_mut());
            assert!(self.conn >= 0);
            let tv = libc::timeval { tv_sec: 10, tv_usec: 0 };
            libc::setsockopt(self.conn, libc::SOL_SOCKET, libc::SO_RCVTIMEO, &tv as *const _ as *const _, std::mem::size_of::<libc::timeval>() as _);
            let mut buf = [0u8; 64];
            let n = libc::recv(self.conn, buf.as_mut_ptr() as *mut _, 64, 0);
            assert_eq!(n, 24, "init message size");
            assert_eq!(buf[0], 0, "MESSAGE_INITIALIZE");
            assert_eq!(i32::from_ne_bytes(buf[4..8].try_into().unwrap()), KEY);
            assert_eq!(buf[8], 1, "FBFMT_RMPP_RGB888");
            let size = W * H * 3;
            let name = std::ffi::CString::new(format!("/qtfb_{KEY}")).unwrap();
            libc::shm_unlink(name.as_ptr());
            let shm_fd = libc::shm_open(name.as_ptr(), libc::O_RDWR | libc::O_CREAT, 0o666);
            assert!(shm_fd >= 0);
            assert_eq!(libc::ftruncate(shm_fd, size as _), 0);
            let p = libc::mmap(std::ptr::null_mut(), size, libc::PROT_READ | libc::PROT_WRITE, libc::MAP_SHARED, shm_fd, 0);
            assert_ne!(p, libc::MAP_FAILED);
            std::ptr::write_bytes(p as *mut u8, 0xFF, size);
            self.shm = p as *mut u8;
            // InitMessageResponseContents: int shmKeyDefined, then size_t shmSize at offset 8.
            let mut body = [0u8; 16];
            body[0..4].copy_from_slice(&KEY.to_ne_bytes());
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
    let root = env!("CARGO_MANIFEST_DIR");
    let data = std::env::temp_dir().join(format!("sudoku-e2e-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&data);
    let mut server = Server::new();
    let mut child = Command::new(env!("CARGO_BIN_EXE_sudoku"))
        .env("QTFB_KEY", KEY.to_string())
        .env("SUDOKU_DATA_DIR", &data)
        .env("SUDOKU_FONT", format!("{root}/scripts/fonts/rmsans-400.ttf"))
        .env("SUDOKU_FONT_MEDIUM", format!("{root}/scripts/fonts/rmsans-500.ttf"))
        .stderr(Stdio::inherit())
        .spawn()
        .expect("start the app");
    server.accept();
    assert!(server.wait_update(), "first frame");

    // The board: the thick line at x 148 is black, a cell center is white or gray.
    assert_eq!(server.pixel(148, 1000), [0, 0, 0]);
    assert_eq!(server.pixel(5, 1000), [255, 255, 255]);
    // The close button does not show while the menu is closed.
    assert_eq!(server.pixel(38, 55), [255, 255, 255]);

    // The game is saved at the start. Find an empty cell.
    let game_path = data.join("game.json");
    assert!(wait_until(|| game_path.exists()));
    let g: Game = serde_json::from_slice(&std::fs::read(&game_path).unwrap()).unwrap();
    assert_eq!(g.mode, 2);
    let cell = cells().find(|&c| !g.is_filled(c)).unwrap();
    let (x, y) = sudoku::layout::cell_origin(cell);

    // A pen tap on micro-cell 5 adds a pencil mark.
    server.pen_tap(x + 73, y + 73);
    // A finger opens the menu: the close button shows.
    server.touch_tap(1563, 55);
    assert!(wait_until(|| server.pixel(38, 55) != [255, 255, 255]), "the close button shows");

    // The close button: the app saves and stops.
    server.touch_tap(55, 55);
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
    let mut server2 = Server::new();
    let mut child = Command::new(env!("CARGO_BIN_EXE_sudoku"))
        .env("QTFB_KEY", KEY.to_string())
        .env("SUDOKU_DATA_DIR", &data)
        .env("SUDOKU_FONT", format!("{root}/scripts/fonts/rmsans-400.ttf"))
        .env("SUDOKU_FONT_MEDIUM", format!("{root}/scripts/fonts/rmsans-500.ttf"))
        .spawn()
        .unwrap();
    server2.accept();
    assert!(server2.wait_update());
    std::thread::sleep(Duration::from_millis(1500));
    unsafe { libc::kill(child.id() as i32, libc::SIGTERM) };
    assert!(child.wait().unwrap().success());
    let g3: Game = serde_json::from_slice(&std::fs::read(&game_path).unwrap()).unwrap();
    assert_eq!((g3.seed, g3.value, g3.marks), (g2.seed, g2.value, g2.marks));
    assert!(g3.elapsed_ms > g2.elapsed_ms, "the clock continues from the saved time");
    let _ = std::fs::remove_dir_all(&data);
}
