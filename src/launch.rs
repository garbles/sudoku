//! The launcher and the frontend (REQUIREMENTS.md §14.4, INVARIANTS.md §18): how AppLoad started the
//! program, the values that the program and the QML frontend (`appload/sudoku/ui/main.qml`) share,
//! the messages on the AppLoad socket, and the position of the pen area in the frontend.
//!
//! This module does not use the socket. `platform/appload.rs` sends and receives the packets.

use std::path::Path;

/// How AppLoad started the program (LCH-9). It does not change while the program runs, and the app
/// does not save it. `launch` and `frontend_files` of INVARIANTS.md are in this type: only the
/// external mode can have an update that is not complete, and only then can the restart dialog show
/// (UPD-10).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Launch {
    /// The frontend mode: AppLoad started `backend/entry` with the path of its socket, and the
    /// program is the backend of the QML frontend.
    Frontend,
    /// The external mode: the program has no socket argument. It runs as an AppLoad external app, as
    /// the releases before the frontend.
    External(Restart),
}

/// The external mode: must the reMarkable interface start again before the app can use the frontend
/// (`restart_needed` of INVARIANTS.md)?
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Restart {
    /// The app folder does not have the frontend files (a release before the frontend).
    NotNeeded,
    /// The update is not complete: the app folder has the frontend files, but AppLoad still uses the
    /// old manifest. `dialog`: the dialog "Restart the interface?" shows.
    Needed { dialog: bool },
}

impl Launch {
    /// The external mode at the start of the program. `frontend_files`: the app folder has the
    /// frontend files (`has_frontend_files`). `at_start` decides if the dialog shows.
    pub fn external(frontend_files: bool) -> Launch {
        Launch::External(if frontend_files { Restart::Needed { dialog: false } } else { Restart::NotNeeded })
    }

    /// The mode of a program with the first argument `socket` (LCH-9): the frontend mode exactly when
    /// AppLoad gave the path of its socket. `frontend_files` is used only in the external mode.
    pub fn of_start(socket: Option<&str>, frontend_files: impl FnOnce() -> bool) -> Launch {
        match socket {
            Some(_) => Launch::Frontend,
            None => Launch::external(frontend_files()),
        }
    }

    /// The state when the app starts, and the new saved value `restart_dialog_shown` (UPD-10, UPD-13).
    /// `shown` is the saved value. The frontend mode sets it to false. While the update is not
    /// complete, the dialog shows exactly when the saved value is false, and the value becomes true.
    pub fn at_start(self, shown: bool) -> (Launch, bool) {
        match self {
            Launch::Frontend => (self, false),
            Launch::External(Restart::NotNeeded) => (self, shown),
            Launch::External(Restart::Needed { .. }) => (Launch::External(Restart::Needed { dialog: !shown }), true),
        }
    }

    pub fn is_frontend(self) -> bool {
        self == Launch::Frontend
    }

    /// The update is not complete (`restart_needed`): the external mode, and the app folder has the
    /// frontend files.
    pub fn restart_needed(self) -> bool {
        matches!(self, Launch::External(Restart::Needed { .. }))
    }

    /// The dialog "Restart the interface?" shows.
    pub fn dialog_shown(self) -> bool {
        self == Launch::External(Restart::Needed { dialog: true })
    }

    /// Closes the dialog "Restart the interface?".
    pub fn close_dialog(&mut self) {
        if let Launch::External(Restart::Needed { dialog }) = self {
            *dialog = false;
        }
    }
}

/// The frontend files of an AppLoad folder (LCH-6). A folder has all of them or none of them.
pub const FRONTEND_FILES: [&str; 3] = ["manifest.json", "resources.rcc", "backend/entry"];

/// The files of a release folder `sudoku/` (LCH-6).
pub const APP_FOLDER_FILES: [&str; 6] = ["sudoku", "manifest.json", "resources.rcc", "backend/entry", "external.manifest.json", "icon.png"];

/// True if the AppLoad folder `app_dir` has all frontend files (`frontend_files` of INVARIANTS.md).
pub fn has_frontend_files(app_dir: &Path) -> bool {
    FRONTEND_FILES.iter().all(|f| app_dir.join(f).is_file())
}

/// The values that an app and its frontend share (LCH-5). `manifest.json` and the QML file have the
/// same values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AppIds {
    /// The app ID of the AppLoad manifest and of the AppLoad endpoint of the frontend.
    pub app_id: &'static str,
    /// The qtfb framebuffer ID of the `FBController` of the frontend. In the frontend mode, the
    /// program connects to qtfb only with this ID.
    pub framebuffer_id: i32,
}

/// The values of Sudoku.
pub const SUDOKU: AppIds = AppIds { app_id: "sudoku", framebuffer_id: 0x5355_444F };
/// The values of the corpus app `Sudoku (corpus)`. `appload/make-folder.sh` puts them in its frontend.
pub const CORPUS: AppIds = AppIds { app_id: "sudoku-corpus", framebuffer_id: 0x5355_4443 };
/// The values of this build.
pub const IDS: AppIds = if cfg!(feature = "corpus") { CORPUS } else { SUDOKU };

/// The message of the program with the pen area: "x y width height", or `NO_AREA`.
pub const MSG_PEN_AREA: i32 = 1;
/// AppLoad: the program must stop.
pub const MSG_STOP: i32 = -1;
/// AppLoad: a frontend connected. The text is the quantity of frontends.
pub const MSG_FRONTEND_CONNECTED: i32 = -2;
/// AppLoad: a frontend closed. The text is the quantity of frontends that stay.
pub const MSG_FRONTEND_CLOSED: i32 = -3;

/// The text of the pen area message when there is no area. A text is never empty: AppLoad reads an
/// empty packet as the end of the connection.
pub const NO_AREA: &str = "none";

/// The time after the start of the program in which a frontend must connect (LCH-2, LCH-10).
pub const CONNECT_TIME_MS: u64 = 10_000;

/// The text of the pen area message for `area` (x, y, width, height in the coordinates of the window:
/// the screen as the player sees it). None: no area.
pub fn pen_area_text(area: Option<(i32, i32, i32, i32)>) -> String {
    area.map_or_else(|| NO_AREA.to_string(), |(x, y, w, h)| format!("{x} {y} {w} {h}"))
}

/// The size of the header packet of a message: the type and the length of the text, two `int`.
pub const HEADER_BYTES: usize = 8;

/// The header packet of a message of the type `kind` with a text of `len` bytes.
pub fn header(kind: i32, len: usize) -> [u8; HEADER_BYTES] {
    let mut bytes = [0; HEADER_BYTES];
    bytes[..4].copy_from_slice(&kind.to_ne_bytes());
    bytes[4..].copy_from_slice(&(len as i32).to_ne_bytes());
    bytes
}

/// The type and the length of the text in a header packet. None: the packet is not a header.
pub fn read_header(packet: &[u8]) -> Option<(i32, usize)> {
    let bytes: &[u8; HEADER_BYTES] = packet.try_into().ok()?;
    let kind = i32::from_ne_bytes(bytes[..4].try_into().unwrap());
    let len = i32::from_ne_bytes(bytes[4..].try_into().unwrap());
    Some((kind, usize::try_from(len).ok()?))
}

/// Why the program stops in the frontend mode (LCH-1, LCH-2). For each cause, the program saves the
/// game, removes the waiting ink and stops.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stop {
    /// The stop message of AppLoad. The frontend also causes it when AppLoad unloads the frontend
    /// (the X of the AppLoad top bar).
    Message,
    /// A "frontend closed" message with the count 0.
    NoFrontendLeft,
    /// The end of the connection to AppLoad.
    ConnectionEnd,
    /// No frontend connected in the connect time (LCH-2).
    NoFrontendInTime,
}

/// The frontends of the program, from the messages of AppLoad.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Frontends {
    /// A frontend connected after the start of the program.
    connected: bool,
}

impl Frontends {
    /// The start of the program: no frontend connected yet.
    pub fn new() -> Frontends {
        Frontends::default()
    }

    /// True after the first frontend connected.
    pub fn connected(&self) -> bool {
        self.connected
    }

    /// A message of AppLoad of the type `kind` with the text `text`. Returns the cause if the program
    /// must stop (LCH-1). A message of an unknown type has no effect.
    pub fn message(&mut self, kind: i32, text: &str) -> Option<Stop> {
        match kind {
            MSG_STOP => Some(Stop::Message),
            MSG_FRONTEND_CONNECTED => {
                self.connected = true;
                None
            }
            MSG_FRONTEND_CLOSED => (text.trim().parse::<u32>() == Ok(0)).then_some(Stop::NoFrontendLeft),
            _ => None,
        }
    }

    /// `ms` after the start of the program: the cause if the program must stop, because no frontend
    /// connected in the connect time (LCH-2).
    pub fn late(&self, ms: u64) -> Option<Stop> {
        (!self.connected && ms >= CONNECT_TIME_MS).then_some(Stop::NoFrontendInTime)
    }

    /// The time in ms until the end of the connect time, `ms` after the start of the program. None: a
    /// frontend connected.
    pub fn wait_ms(&self, ms: u64) -> Option<u64> {
        (!self.connected).then(|| CONNECT_TIME_MS.saturating_sub(ms))
    }
}

/// The framebuffer picture in the item of the frontend (LCH-3): the scale `s` and the offset
/// `(ox, oy)`, in the coordinates of the window. The frontend (`main.qml`, `picture`) calculates
/// the same values.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Picture {
    pub s: f64,
    pub ox: f64,
    pub oy: f64,
}

impl Picture {
    /// The picture of a framebuffer of `fb` pixels (width, height) that AppLoad draws with the rotation
    /// `rotation` (0, 1: 90° left, 2: 90° right, 3: 180°) in an item of the size `item`: the largest
    /// picture that fits, at the center. None: a size is not known.
    pub fn fit(fb: (u32, u32), rotation: i32, item: (f64, f64)) -> Option<Picture> {
        // The picture as the player sees it: a rotation of 90° exchanges the width and the height.
        let (w, h) = if rotation == 1 || rotation == 2 { (fb.1, fb.0) } else { fb };
        if w == 0 || h == 0 || item.0 <= 0.0 || item.1 <= 0.0 {
            return None;
        }
        let (w, h) = (f64::from(w), f64::from(h));
        let s = (item.0 / w).min(item.1 / h);
        Some(Picture { s, ox: (item.0 - s * w) / 2.0, oy: (item.1 - s * h) / 2.0 })
    }

    /// The pen area on the screen for the rectangle `area` that the program gave (x, y, width, height):
    /// `(ox + s·x, oy + s·y, s·width, s·height)`.
    pub fn area(&self, (x, y, w, h): (i32, i32, i32, i32)) -> (f64, f64, f64, f64) {
        (self.ox + self.s * f64::from(x), self.oy + self.s * f64::from(y), self.s * f64::from(w), self.s * f64::from(h))
    }

    /// True if one pixel of the framebuffer is one pixel of the window, at the same position: the
    /// picture has the size of the item.
    pub fn is_exact(&self) -> bool {
        self.s == 1.0 && self.ox == 0.0 && self.oy == 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_socket_argument_selects_the_frontend_mode() {
        // LCH-9
        assert_eq!(Launch::of_start(Some("/tmp/sudoku.sock"), || unreachable!()), Launch::Frontend);
        assert_eq!(Launch::of_start(None, || false), Launch::External(Restart::NotNeeded));
        assert_eq!(Launch::of_start(None, || true), Launch::External(Restart::Needed { dialog: false }));
    }

    #[test]
    fn the_dialog_shows_only_at_the_first_start_with_an_update_that_is_not_complete() {
        // UPD-10, UPD-13
        let needed = Launch::external(true);
        assert_eq!(needed.at_start(false), (Launch::External(Restart::Needed { dialog: true }), true));
        assert_eq!(needed.at_start(true), (Launch::External(Restart::Needed { dialog: false }), true));
        for shown in [false, true] {
            assert_eq!(Launch::Frontend.at_start(shown), (Launch::Frontend, false));
            assert_eq!(Launch::external(false).at_start(shown), (Launch::External(Restart::NotNeeded), shown));
        }
        let mut launch = needed.at_start(false).0;
        assert!(launch.dialog_shown() && launch.restart_needed());
        launch.close_dialog();
        assert!(!launch.dialog_shown() && launch.restart_needed());
    }

    #[test]
    fn the_program_stops_for_each_cause_of_a_closed_window() {
        // LCH-1, LCH-2, LCH-10
        let mut f = Frontends::new();
        assert_eq!(f.late(CONNECT_TIME_MS - 1), None);
        assert_eq!(f.late(CONNECT_TIME_MS), Some(Stop::NoFrontendInTime));
        assert_eq!(f.wait_ms(4_000), Some(6_000));
        assert_eq!(f.message(MSG_FRONTEND_CONNECTED, "1"), None);
        assert!(f.connected());
        assert_eq!((f.late(CONNECT_TIME_MS * 2), f.wait_ms(0)), (None, None));
        assert_eq!(f.message(MSG_FRONTEND_CLOSED, "1"), None);
        assert_eq!(f.message(MSG_FRONTEND_CLOSED, "0"), Some(Stop::NoFrontendLeft));
        assert_eq!(f.message(MSG_STOP, "close"), Some(Stop::Message));
        assert_eq!(f.message(MSG_PEN_AREA, "none"), None);
    }

    #[test]
    fn the_messages_have_a_header_and_a_text() {
        assert_eq!(pen_area_text(Some((4, 8, 140, 136))), "4 8 140 136");
        assert_eq!(pen_area_text(None), "none");
        assert_eq!(read_header(&header(MSG_PEN_AREA, 4)), Some((1, 4)));
        assert_eq!(read_header(&header(MSG_FRONTEND_CLOSED, 1)), Some((-3, 1)));
        assert_eq!(read_header(&[0; 7]), None);
        assert_eq!(read_header(&header(1, 0)[..]).map(|h| h.1), Some(0));
        let mut negative = header(1, 0);
        negative[4..].copy_from_slice(&(-1i32).to_ne_bytes());
        assert_eq!(read_header(&negative), None);
    }

    #[test]
    fn the_ids_of_the_two_apps_are_different() {
        // LCH-5
        assert_eq!((SUDOKU.app_id, SUDOKU.framebuffer_id), ("sudoku", 0x5355444F));
        assert_eq!((CORPUS.app_id, CORPUS.framebuffer_id), ("sudoku-corpus", 0x53554443));
        assert!(SUDOKU.app_id != CORPUS.app_id && SUDOKU.framebuffer_id != CORPUS.framebuffer_id);
    }

    #[test]
    fn the_pen_area_follows_the_picture() {
        // LCH-3: the full screen of the Paper Pro, in portrait and in landscape.
        for (rotation, item) in [(0, (1620.0, 2160.0)), (3, (1620.0, 2160.0)), (1, (2160.0, 1620.0)), (2, (2160.0, 1620.0))] {
            let p = Picture::fit((1620, 2160), rotation, item).unwrap();
            assert!(p.is_exact(), "rotation {rotation}");
            assert_eq!(p.area((300, 600, 140, 136)), (300.0, 600.0, 140.0, 136.0));
        }
        // A smaller item: the picture has the same shape, at the center.
        let p = Picture::fit((1620, 2160), 0, (1000.0, 1080.0)).unwrap();
        assert_eq!((p.s, p.ox, p.oy), (0.5, 95.0, 0.0));
        assert_eq!(p.area((300, 600, 140, 136)), (245.0, 300.0, 70.0, 68.0));
        assert!(!p.is_exact());
        assert_eq!(Picture::fit((0, 0), 0, (1620.0, 2160.0)), None);
    }
}
