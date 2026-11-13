//! The launcher and the frontend (INVARIANTS.md §18, REQUIREMENTS.md §14.4): the stop rules of the
//! frontend mode, the messages on the AppLoad socket, the pen area of the frontend, the values that the
//! frontend and the program share, the files of the app folder and the selection of the mode.
//!
//! The QML frontend (`appload/sudoku/ui/main.qml`) does not run on the computer. The tests compare its
//! text with the documents and with the source. `tests/qtfb_e2e.rs` starts the program in the frontend
//! mode on Linux.
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::Command;

use proptest::prelude::*;
use sudoku::launch::{self, Frontends, Launch, Picture, Restart, Stop};
use sudoku::layout::Model;

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn read(path: &str) -> String {
    std::fs::read_to_string(root().join(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// The lines of a Dockerfile, a shell script or a workflow that are not comments, in one text.
fn code_lines(text: &str) -> String {
    text.lines().filter(|l| !l.trim_start().starts_with('#')).collect::<Vec<_>>().join("\n")
}

/// The QML frontend of Sudoku.
fn qml() -> String {
    read("appload/sudoku/ui/main.qml")
}

/// The value of the QML property `name` (`readonly property <type> <name>: <value>`).
fn qml_property(qml: &str, name: &str) -> String {
    let values: Vec<&str> = qml
        .lines()
        .filter_map(|l| l.trim().strip_prefix("readonly property "))
        .filter_map(|l| l.split_once(' ').map(|(_, rest)| rest))
        .filter_map(|l| l.strip_prefix(name)?.strip_prefix(": "))
        .collect();
    assert_eq!(values.len(), 1, "main.qml must have the property {name} one time: {values:?}");
    values[0].trim().to_string()
}

/// A new empty folder for one test.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("sudoku-launch-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn mode_of(path: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).unwrap_or_else(|e| panic!("{}: {e}", path.display())).permissions().mode() & 0o777
}

// ---------------------------------------------------------------------------------------------
// LCH-1, LCH-2, LCH-10: the stop rules.

#[test]
fn the_program_stops_for_each_message_that_closes_the_window() {
    // LCH-1: the stop message, and a "frontend closed" message with the count 0. A "frontend closed"
    // message with a count that is not 0 and a "frontend connected" message do not stop the program.
    // The message types of AppLoad are -1, -2 and -3 (R§14.4).
    assert_eq!((launch::MSG_STOP, launch::MSG_FRONTEND_CONNECTED, launch::MSG_FRONTEND_CLOSED), (-1, -2, -3));
    for connected_first in [false, true] {
        let start = || {
            let mut f = Frontends::new();
            if connected_first {
                assert_eq!(f.message(launch::MSG_FRONTEND_CONNECTED, "1"), None);
            }
            f
        };
        let case = if connected_first { "after a frontend connected" } else { "before a frontend connected" };
        assert_eq!(start().message(launch::MSG_STOP, "stop"), Some(Stop::Message), "{case}");
        assert_eq!(start().message(launch::MSG_FRONTEND_CLOSED, "0"), Some(Stop::NoFrontendLeft), "{case}");
        for count in ["1", "2", "10"] {
            assert_eq!(start().message(launch::MSG_FRONTEND_CLOSED, count), None, "{case}: count {count}");
        }
        // The pen area message goes from the program to the frontend. Other types have no effect.
        for kind in [launch::MSG_PEN_AREA, 0, 2, 7, -4] {
            assert_eq!(start().message(kind, "0"), None, "{case}: type {kind}");
        }
    }
    // Two frontends, then each closes: the program stops only when the count is 0.
    let mut f = Frontends::new();
    assert_eq!(f.message(launch::MSG_FRONTEND_CONNECTED, "1"), None);
    assert_eq!(f.message(launch::MSG_FRONTEND_CONNECTED, "2"), None);
    assert_eq!(f.message(launch::MSG_FRONTEND_CLOSED, "1"), None);
    assert_eq!(f.message(launch::MSG_FRONTEND_CLOSED, "0"), Some(Stop::NoFrontendLeft));
}

#[test]
fn the_program_stops_when_no_frontend_connects_in_10_s() {
    // LCH-2, LCH-10: the connect time is 10 s after the start of the program. A frontend that
    // connects in that time stops the count.
    assert_eq!(launch::CONNECT_TIME_MS, 10_000, "LCH-10");
    let f = Frontends::new();
    assert!(!f.connected());
    for ms in [0, 1, 5_000, 9_999] {
        assert_eq!(f.late(ms), None, "{ms} ms");
        assert_eq!(f.wait_ms(ms), Some(10_000 - ms), "{ms} ms");
    }
    for ms in [10_000, 10_001, 60_000, u64::MAX] {
        assert_eq!(f.late(ms), Some(Stop::NoFrontendInTime), "{ms} ms");
        assert_eq!(f.wait_ms(ms), Some(0), "{ms} ms");
    }
    // A frontend at 9.999 s: no stop at a later time.
    let mut f = Frontends::new();
    assert_eq!(f.message(launch::MSG_FRONTEND_CONNECTED, "1"), None);
    assert!(f.connected());
    for ms in [9_999, 10_000, u64::MAX] {
        assert_eq!((f.late(ms), f.wait_ms(ms)), (None, None), "{ms} ms");
    }
    // A frontend that connected and closed: the connect time does not apply again. LCH-1 stops the
    // program with the count 0.
    assert_eq!(f.message(launch::MSG_FRONTEND_CLOSED, "1"), None);
    assert_eq!(f.late(20_000), None);
}

/// A message of AppLoad for the property test: the type and the count.
fn appload_message() -> impl Strategy<Value = (i32, u32)> {
    (prop_oneof![Just(-1), Just(-2), Just(-3), Just(1), -10..10i32], 0..3u32)
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 512, ..ProptestConfig::default() })]

    #[test]
    fn the_first_stop_comes_at_the_first_message_that_closes_the_window(
        messages in prop::collection::vec(appload_message(), 0..12),
        ms in 0..30_000u64,
    ) {
        // LCH-1, LCH-2: a model of the rules. The program stops at the first stop message or the first
        // "frontend closed" message with the count 0, and before that only when no frontend connected
        // in the connect time.
        let mut f = Frontends::new();
        let mut connected = false;
        for (kind, count) in messages {
            let want = match kind {
                -1 => Some(Stop::Message),
                -3 if count == 0 => Some(Stop::NoFrontendLeft),
                _ => None,
            };
            prop_assert_eq!(f.message(kind, &count.to_string()), want);
            connected |= kind == -2;
            prop_assert_eq!(f.connected(), connected);
            if want.is_some() {
                break;
            }
        }
        let late = (!connected && ms >= 10_000).then_some(Stop::NoFrontendInTime);
        prop_assert_eq!(f.late(ms), late);
        prop_assert_eq!(f.wait_ms(ms), (!connected).then(|| 10_000u64.saturating_sub(ms)));
    }
}

// ---------------------------------------------------------------------------------------------
// LCH-9, R§14.4: the messages on the AppLoad socket.

/// A header packet as R§14.4 gives it: the type and the length of the text, two `int`.
fn documented_header(kind: i32, len: i32) -> Vec<u8> {
    [kind.to_ne_bytes(), len.to_ne_bytes()].concat()
}

#[test]
fn a_message_is_a_header_and_a_text_that_is_never_empty() {
    // R§14.4: each message is two packets, a header (the type and the length, two int) and the text.
    // A text is never empty. LCH-9: the pen area is the type 1 with "x y width height", or the text
    // `none` for "no area".
    assert_eq!(launch::MSG_PEN_AREA, 1);
    assert_eq!(launch::NO_AREA, "none");
    assert_eq!(launch::pen_area_text(None), "none");
    assert_eq!(launch::pen_area_text(Some((0, 0, 147, 147))), "0 0 147 147");
    assert_eq!(launch::pen_area_text(Some((1473, 2013, 147, 147))), "1473 2013 147 147");
    assert_eq!(launch::HEADER_BYTES, 8);
    assert_eq!(launch::header(launch::MSG_PEN_AREA, 4).to_vec(), documented_header(1, 4));
    assert_eq!(launch::read_header(&documented_header(-3, 1)), Some((-3, 1)));
    // A packet that is not a header: no message.
    for packet in [&[][..], &[0u8; 4][..], &[0u8; 7][..], &[0u8; 9][..], &[0u8; 16][..]] {
        assert_eq!(launch::read_header(packet), None, "{} bytes", packet.len());
    }
    assert_eq!(launch::read_header(&documented_header(1, -1)), None, "a negative length");
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

    #[test]
    fn the_pen_area_text_gives_the_rectangle(x in -100..3000i32, y in -100..3000i32, w in 1..3000i32, h in 1..3000i32) {
        // LCH-3, LCH-9: the text has the four values in the order x, y, width, height, and the frontend
        // reads the same rectangle. The text is never empty and never `none` for an area.
        let text = launch::pen_area_text(Some((x, y, w, h)));
        prop_assert!(!text.is_empty() && text != launch::NO_AREA);
        let values: Vec<i32> = text.split(' ').map(|v| v.parse().unwrap()).collect();
        prop_assert_eq!(values, vec![x, y, w, h]);
        let header = launch::header(launch::MSG_PEN_AREA, text.len());
        prop_assert_eq!(header.to_vec(), documented_header(1, text.len() as i32));
        prop_assert_eq!(launch::read_header(&header), Some((1, text.len())));
    }

    #[test]
    fn a_header_gives_its_type_and_length_again(kind in any::<i32>(), len in 0..=i32::MAX) {
        prop_assert_eq!(launch::read_header(&launch::header(kind, len as usize)), Some((kind, len as usize)));
    }
}

// ---------------------------------------------------------------------------------------------
// LCH-3: the pen area of the frontend.

/// The screen of `model` as the player sees it in the rotation `rotation` (0, 3: portrait; 1, 2:
/// landscape; R§14.4 and `Picture::fit`).
fn screen_as_seen(model: Model, rotation: i32) -> (u32, u32) {
    let d = model.device();
    if rotation == 1 || rotation == 2 {
        (d.physical_h, d.physical_w)
    } else {
        (d.physical_w, d.physical_h)
    }
}

#[test]
fn on_the_full_screen_the_pen_area_is_the_given_rectangle() {
    // LCH-3: on the full screen of each model and in each rotation, the scale is 1 and the offset is
    // 0, thus the area on the screen is the rectangle of the program.
    for model in Model::ALL {
        let d = model.device();
        for rotation in 0..4 {
            let (w, h) = screen_as_seen(model, rotation);
            let p = Picture::fit((d.physical_w, d.physical_h), rotation, (f64::from(w), f64::from(h))).unwrap();
            let case = format!("{model:?}, rotation {rotation}");
            assert!(p.is_exact(), "{case}: {p:?}");
            assert_eq!((p.s, p.ox, p.oy), (1.0, 0.0, 0.0), "{case}");
            let cell = d.cell;
            for rect in [(0, 0, cell, cell), (w as i32 - cell, h as i32 - cell, cell, cell), (100, 200, 3 * cell, cell)] {
                let (x, y, rw, rh) = rect;
                assert_eq!(p.area(rect), (f64::from(x), f64::from(y), f64::from(rw), f64::from(rh)), "{case}");
            }
        }
    }
}

#[test]
fn in_a_scaled_picture_the_pen_area_follows_the_scale_and_the_offset() {
    // LCH-3: the area on the screen is (ox + s·x, oy + s·y, s·width, s·height). The picture is the
    // largest that fits, at the center. A rotation of 90° exchanges the width and the height.
    let fb = (1620, 2160);
    // A window of half the size: s = 0.5, no offset.
    let p = Picture::fit(fb, 0, (810.0, 1080.0)).unwrap();
    assert_eq!((p.s, p.ox, p.oy), (0.5, 0.0, 0.0));
    assert_eq!(p.area((300, 600, 140, 136)), (150.0, 300.0, 70.0, 68.0));
    assert!(!p.is_exact());
    // A wide window: the picture is at the center in x.
    let p = Picture::fit(fb, 0, (1620.0, 1080.0)).unwrap();
    assert_eq!((p.s, p.ox, p.oy), (0.5, 405.0, 0.0));
    assert_eq!(p.area((0, 0, 1620, 2160)), (405.0, 0.0, 810.0, 1080.0));
    // A tall window: the picture is at the center in y.
    let p = Picture::fit(fb, 0, (810.0, 2160.0)).unwrap();
    assert_eq!((p.s, p.ox, p.oy), (0.5, 0.0, 540.0));
    assert_eq!(p.area((10, 20, 30, 40)), (5.0, 550.0, 15.0, 20.0));
    // Landscape (rotation 1 and 2) in a portrait window: the picture is 2160 x 1620 as the player sees
    // it. Rotation 3 does not exchange the sides.
    for rotation in [1, 2] {
        let p = Picture::fit(fb, rotation, (1620.0, 2160.0)).unwrap();
        assert_eq!((p.s, p.ox, p.oy), (0.75, 0.0, (2160.0 - 0.75 * 1620.0) / 2.0), "rotation {rotation}");
        assert_eq!(p.area((2160 - 147, 0, 147, 147)), (0.75 * 2013.0, p.oy, 0.75 * 147.0, 0.75 * 147.0));
    }
    let p = Picture::fit(fb, 3, (1620.0, 2160.0)).unwrap();
    assert!(p.is_exact(), "rotation 3");
    // The framebuffer of the Paper Pro in a window of the size of the Move screen: a scale that is not
    // 1, thus the area is correct only by calculation (R§14.4).
    let mv = Model::Move.device();
    let (mw, mh) = (f64::from(mv.physical_w), f64::from(mv.physical_h));
    let p = Picture::fit(fb, 0, (mw, mh)).unwrap();
    let s = (mw / 1620.0).min(mh / 2160.0);
    assert_eq!((p.s, p.ox, p.oy), (s, (mw - s * 1620.0) / 2.0, (mh - s * 2160.0) / 2.0));
    assert_eq!(p.area((147, 294, 147, 147)), (p.ox + s * 147.0, p.oy + s * 294.0, s * 147.0, s * 147.0));
    // No size: no picture, thus no pen area.
    for (fb, item) in [((0, 2160), (1620.0, 2160.0)), ((1620, 0), (1620.0, 2160.0)), ((1620, 2160), (0.0, 2160.0)), ((1620, 2160), (1620.0, 0.0)), ((1620, 2160), (-1.0, 5.0))] {
        assert_eq!(Picture::fit(fb, 0, item), None, "{fb:?} in {item:?}");
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 512, ..ProptestConfig::default() })]

    #[test]
    fn the_picture_is_the_largest_that_fits_at_the_center(
        fw in 1..4000u32, fh in 1..4000u32, rotation in 0..4i32,
        iw in 1.0..4000.0f64, ih in 1.0..4000.0f64,
        rx in 0.0..1.0f64, ry in 0.0..1.0f64, rw in 0.0..1.0f64, rh in 0.0..1.0f64,
    ) {
        // LCH-3.
        let (w, h) = if rotation == 1 || rotation == 2 { (f64::from(fh), f64::from(fw)) } else { (f64::from(fw), f64::from(fh)) };
        let p = Picture::fit((fw, fh), rotation, (iw, ih)).unwrap();
        let eps = 1e-9 * (1.0 + iw + ih);
        prop_assert!(p.s > 0.0);
        // It fits, and it touches two sides of the item.
        prop_assert!(p.s * w <= iw + eps && p.s * h <= ih + eps);
        prop_assert!((p.s * w - iw).abs() <= eps || (p.s * h - ih).abs() <= eps);
        // At the center.
        prop_assert!(p.ox >= -eps && p.oy >= -eps);
        prop_assert!((2.0 * p.ox + p.s * w - iw).abs() <= eps && (2.0 * p.oy + p.s * h - ih).abs() <= eps);
        // The area of a rectangle in the picture.
        let x = (rx * w) as i32;
        let y = (ry * h) as i32;
        let rect = (x, y, ((w - f64::from(x)) * rw) as i32, ((h - f64::from(y)) * rh) as i32);
        let a = p.area(rect);
        prop_assert_eq!(a, (p.ox + p.s * f64::from(rect.0), p.oy + p.s * f64::from(rect.1), p.s * f64::from(rect.2), p.s * f64::from(rect.3)));
        prop_assert!(a.0 >= -eps && a.1 >= -eps && a.0 + a.2 <= iw + eps && a.1 + a.3 <= ih + eps, "the area is in the window");
        // Only an item of the size of the picture gives the exact picture.
        prop_assert_eq!(p.is_exact(), p.s == 1.0 && p.ox == 0.0 && p.oy == 0.0);
        let exact = Picture::fit((fw, fh), rotation, (w, h)).unwrap();
        prop_assert!(exact.is_exact());
        prop_assert_eq!(exact.area(rect), (f64::from(rect.0), f64::from(rect.1), f64::from(rect.2), f64::from(rect.3)));
    }
}

#[test]
fn the_frontend_converts_the_area_as_the_program() {
    // LCH-3: the frontend calculates the picture and the area with the same expressions as
    // `launch::Picture` (the documents: the largest picture that fits, at the center; a rotation of 90°
    // exchanges the sides; the area is (ox + s·x, oy + s·y, s·width, s·height)).
    let q = qml();
    for line in [
        "const turned = fb.fbRotation === FBController.Deg90L || fb.fbRotation === FBController.Deg90R",
        "const w = turned ? size.height : size.width",
        "const h = turned ? size.width : size.height",
        "const s = Math.min(fb.width / w, fb.height / h)",
        "return { s: s, ox: (fb.width - s * w) / 2, oy: (fb.height - s * h) / 2 }",
        "x: root.penAreaOn ? root.picture.ox + root.picture.s * root.area[0] : 0",
        "y: root.penAreaOn ? root.picture.oy + root.picture.s * root.area[1] : 0",
        "width: root.penAreaOn ? root.picture.s * root.area[2] : 0",
        "height: root.penAreaOn ? root.picture.s * root.area[3] : 0",
    ] {
        assert!(q.lines().any(|l| l.trim() == line), "main.qml must have: {line}");
    }
    // The frontend reads the text of the program: "x y width height".
    assert!(q.contains(".split(\" \").map(Number)"), "the frontend splits the text at the spaces");
    assert!(q.contains("v.length === 4"), "the frontend needs four values");
}

// ---------------------------------------------------------------------------------------------
// LCH-4, LCH-8: the conditions of the frontend.

#[test]
fn the_frontend_shows_no_pen_area_in_the_cases_of_lch_4() {
    // LCH-4: no pen area when the last message gave `none`, when the program stopped, when the window
    // is minimized or not active, and when the window is not the full screen. The full window always
    // has the UI mode (3), and only the pen area has the pen mode (0).
    let q = qml();
    let on = q.lines().skip_while(|l| !l.contains("readonly property bool penAreaOn:")).take(2).collect::<Vec<_>>().join(" ");
    for term in ["root.area !== null", "!root.stopped", "fb.active", "root.visible", "!root.minimized", "root.fullScreen", "root.picture !== null"] {
        assert!(on.contains(term), "LCH-4: penAreaOn must have {term}: {on}");
    }
    assert!(!on.contains("||"), "LCH-4: each condition is necessary: {on}");
    assert_eq!(qml_property(&q, "penMode"), "0");
    assert_eq!(qml_property(&q, "uiMode"), "3", "R§14.4: the number 3, not the name of the item");
    // The two areas: the full window with the UI mode, and the pen area only when penAreaOn.
    let areas: Vec<&str> = q.split("DisplayMethodArea {").skip(1).map(|a| a.split('}').next().unwrap()).collect();
    assert_eq!(areas.len(), 2, "two DisplayMethodArea items");
    assert!(areas[0].contains("anchors.fill: parent") && areas[0].contains("displayMethod: root.uiMode"), "{}", areas[0]);
    assert!(areas[1].contains("visible: root.penAreaOn") && areas[1].contains("displayMethod: root.penMode"), "{}", areas[1]);
    assert!(!q.contains("DisplayMethodArea.Content") && !q.contains("DisplayMethodArea.UI"), "R§14.4: no mode names");
    // `none` gives no area. When AppLoad unloads the frontend, the area goes away and the program stops.
    assert!(q.contains("if (text === root.noArea)\n            return null"), "LCH-4: none gives no area");
    let unloading = q.split("function unloading()").nth(1).unwrap().split('}').next().unwrap();
    for line in ["root.stopped = true", "root.area = null", "backend.terminate()"] {
        assert!(unloading.contains(line), "LCH-1, LCH-4: unloading() must have {line}");
    }
    // The full screen comes from the window of AppLoad, else from the exact picture (scale 1, offset 0).
    let full = q.lines().skip_while(|l| !l.contains("readonly property bool fullScreen:")).take(3).collect::<Vec<_>>().join(" ");
    assert!(full.contains("root.win.fullscreen === true"), "{full}");
    assert!(full.contains("root.picture.s === 1 && root.picture.ox === 0 && root.picture.oy === 0"), "{full}");
    assert!(q.contains("readonly property bool minimized: root.win !== null && root.win.minimized === true"));
}

#[test]
fn without_the_rotation_or_the_top_bar_the_frontend_stays_in_portrait() {
    // LCH-8: the frontend turns the picture only when it found the rotation value and the top bar
    // function of the AppLoad window. Else the app stays in portrait, and the log gets one line. The
    // picture keeps its shape: no cut screen.
    let q = qml();
    let can_turn = q.lines().skip_while(|l| !l.contains("readonly property bool canTurn:")).take(2).collect::<Vec<_>>().join(" ");
    for term in ["root.win !== null", "root.win.globalRotation !== undefined", "typeof root.win.swipeFromTheTop === \"function\""] {
        assert!(can_turn.contains(term), "LCH-8: canTurn must have {term}: {can_turn}");
    }
    let turn = q.split("function turn()").nth(1).unwrap().split('}').next().unwrap();
    assert!(turn.contains("if (root.canTurn)") && turn.contains("fb.fbRotation = root.win.globalRotation"), "LCH-8: {turn}");
    assert_eq!(q.matches("fb.fbRotation = ").count(), 1, "LCH-8: only turn() sets the rotation");
    let completed = q.split("Component.onCompleted:").nth(1).unwrap().split("\n    }").next().unwrap();
    assert!(completed.contains("if (!root.canTurn)\n            console.log("), "LCH-8: one log line: {completed}");
    assert_eq!(q.matches("console.log(").count(), 1, "LCH-8: the log gets one line");
    assert!(completed.contains("fb.fillMode = FBController.PreserveAspectFit"), "LCH-8: no cut screen");
    // A drag from the top edge uses the top bar function only when it is there.
    assert!(q.contains("if (root.win !== null && typeof root.win.swipeFromTheTop === \"function\")\n                root.win.swipeFromTheTop()"));
}

// ---------------------------------------------------------------------------------------------
// LCH-5: the values that the frontend and the program share.

#[test]
fn the_frontend_and_the_program_have_the_same_ids_and_message_types() {
    // LCH-5: one value for each ID and each message type. Sudoku: the app ID `sudoku`, the framebuffer
    // ID 0x5355444F, the message type 1. The corpus app: `sudoku-corpus`, 0x53554443.
    assert_eq!((launch::SUDOKU.app_id, launch::SUDOKU.framebuffer_id), ("sudoku", 0x5355444F));
    assert_eq!((launch::CORPUS.app_id, launch::CORPUS.framebuffer_id), ("sudoku-corpus", 0x53554443));
    assert_ne!(launch::SUDOKU.app_id, launch::CORPUS.app_id);
    assert_ne!(launch::SUDOKU.framebuffer_id, launch::CORPUS.framebuffer_id);
    assert_eq!(launch::IDS, launch::SUDOKU, "the usual build");
    let q = qml();
    assert_eq!(qml_property(&q, "appId"), format!("\"{}\"", launch::SUDOKU.app_id));
    assert_eq!(qml_property(&q, "framebufferId"), format!("0x{:08X}", launch::SUDOKU.framebuffer_id));
    assert_eq!(qml_property(&q, "penAreaMessage"), launch::MSG_PEN_AREA.to_string());
    assert_eq!(qml_property(&q, "noArea"), format!("\"{}\"", launch::NO_AREA));
    // The frontend uses the properties, not other values.
    for line in ["framebufferID: root.framebufferId", "backend.applicationID = root.appId", "if (type === root.penAreaMessage)"] {
        assert!(q.lines().any(|l| l.trim() == line), "main.qml must have: {line}");
    }
    assert_eq!(q.matches("0x").count(), 1, "one framebuffer ID in the frontend");
    // manifest.json has the same app ID, and AppLoad starts the backend.
    let manifest: serde_json::Value = serde_json::from_str(&read("appload/sudoku/manifest.json")).unwrap();
    assert_eq!(manifest["id"], serde_json::json!(launch::SUDOKU.app_id));
    assert_eq!(manifest["name"], serde_json::json!("Sudoku"));
    assert_eq!(manifest["loadsBackend"], serde_json::json!(true));
    // R§14.4: the manifest of an app with a QML frontend has no disablesWindowedMode.
    assert!(manifest.get("disablesWindowedMode").is_none());
    // INP-23: no AppLoad manifest of the app sets the recognition pause or an environment variable.
    for path in ["appload/sudoku/manifest.json", "appload/sudoku/external.manifest.json"] {
        let text = read(path).to_lowercase();
        for word in ["environment", "pause", "sudoku_"] {
            assert!(!text.contains(word), "INP-23: {path} has {word}");
        }
    }
    // The entry of the manifest is the QML file in the resource file.
    assert_eq!(manifest["entry"], serde_json::json!("/ui/main.qml"));
    let qrc = read("appload/sudoku/application.qrc");
    assert!(qrc.contains("<qresource>") && qrc.contains("<file>ui/main.qml</file>"), "{qrc}");
}

/// Writes a fake Qt `rcc` in `dir`: it writes its arguments to `args`, and the QML of the resource
/// file to the output file. Thus the test can read the frontend of the folder.
fn fake_rcc(dir: &Path) -> (PathBuf, PathBuf) {
    use std::os::unix::fs::PermissionsExt;
    let rcc = dir.join("rcc");
    let args = dir.join("rcc-args");
    std::fs::write(
        &rcc,
        format!(
            "#!/bin/sh\necho \"$@\" > '{}'\nout=\nqrc=\nwhile [ $# -gt 0 ]; do\n  case \"$1\" in\n    -o) out=\"$2\"; shift 2 ;;\n    -*) shift ;;\n    *) qrc=\"$1\"; shift ;;\n  esac\ndone\ncat \"$(dirname \"$qrc\")/ui/main.qml\" > \"$out\"\n",
            args.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&rcc, std::fs::Permissions::from_mode(0o755)).unwrap();
    (rcc, args)
}

/// Runs `appload/make-folder.sh OUT [variant]` with the fake `rcc`. Returns the folder and the
/// arguments of rcc.
fn make_folder(name: &str, variant: Option<&str>) -> (PathBuf, String) {
    let dir = scratch(name);
    let (rcc, args) = fake_rcc(&dir);
    let out = dir.join("out");
    let mut command = Command::new("sh");
    command.arg(root().join("appload/make-folder.sh")).arg(&out).env("RCC", &rcc);
    if let Some(v) = variant {
        command.arg(v);
    }
    let output = command.output().unwrap();
    assert!(output.status.success(), "make-folder.sh {variant:?}: {}", String::from_utf8_lossy(&output.stderr));
    (out, std::fs::read_to_string(args).unwrap())
}

/// The files of the folder `dir` and of its subfolders, with the mode.
fn files_of(dir: &Path) -> Vec<(String, u32)> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, u32)>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                out.push((path.strip_prefix(root).unwrap().to_string_lossy().into_owned(), mode_of(&path)));
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out.sort();
    out
}

/// The files of a release folder without the program, which the build adds (LCH-6).
fn folder_without_program() -> Vec<(String, u32)> {
    let mut want: Vec<(String, u32)> =
        launch::APP_FOLDER_FILES.iter().filter(|f| **f != "sudoku").map(|f| (f.to_string(), if *f == "backend/entry" { 0o755 } else { 0o644 })).collect();
    want.sort();
    want
}

#[test]
fn the_build_script_makes_the_files_of_the_app_folder() {
    // LCH-6: the folder has manifest.json, resources.rcc, backend/entry (mode 755),
    // external.manifest.json and icon.png; the build adds `sudoku`. R§14.4: rcc makes resources.rcc
    // in the binary format with no compression. LCH-5: the frontend of the folder has the values of
    // Sudoku.
    let mut names = launch::APP_FOLDER_FILES.to_vec();
    names.sort();
    assert_eq!(names, ["backend/entry", "external.manifest.json", "icon.png", "manifest.json", "resources.rcc", "sudoku"], "LCH-6");
    assert_eq!(launch::FRONTEND_FILES, ["manifest.json", "resources.rcc", "backend/entry"]);
    let (out, args) = make_folder("folder", None);
    assert_eq!(files_of(&out), folder_without_program(), "LCH-6");
    let args: Vec<&str> = args.split_whitespace().collect();
    assert!(args.contains(&"--binary") && args.contains(&"--no-compress"), "R§14.4: rcc {args:?}");
    let frontend = std::fs::read_to_string(out.join("resources.rcc")).unwrap();
    assert_eq!(frontend, qml(), "the frontend of Sudoku is the QML of the repository");
    let manifest: serde_json::Value = serde_json::from_slice(&std::fs::read(out.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["id"], serde_json::json!(launch::SUDOKU.app_id));
    assert_eq!(std::fs::read(out.join("icon.png")).unwrap(), std::fs::read(root().join("appload/sudoku/icon.png")).unwrap());
    let _ = std::fs::remove_dir_all(out.parent().unwrap());
}

/// The corpus variant uses `sed -i` of GNU sed (the Linux image of the build). The Linux tests run it.
#[cfg(target_os = "linux")]
#[test]
fn the_corpus_app_has_its_own_ids() {
    // LCH-5: the corpus app has the app ID `sudoku-corpus` and the framebuffer ID 0x53554443, in its
    // manifest and in its frontend. LCH-6: its folder has all frontend files.
    let (out, _) = make_folder("corpus", Some("corpus"));
    assert_eq!(files_of(&out), folder_without_program(), "LCH-6");
    let q = std::fs::read_to_string(out.join("resources.rcc")).unwrap();
    assert_eq!(qml_property(&q, "appId"), format!("\"{}\"", launch::CORPUS.app_id));
    assert_eq!(qml_property(&q, "framebufferId"), format!("0x{:08X}", launch::CORPUS.framebuffer_id));
    assert_eq!(qml_property(&q, "penAreaMessage"), launch::MSG_PEN_AREA.to_string());
    let manifest: serde_json::Value = serde_json::from_slice(&std::fs::read(out.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["id"], serde_json::json!(launch::CORPUS.app_id));
    assert_eq!(manifest["name"], serde_json::json!("Sudoku (corpus)"));
    let external: serde_json::Value = serde_json::from_slice(&std::fs::read(out.join("external.manifest.json")).unwrap()).unwrap();
    assert_eq!(external["name"], serde_json::json!("Sudoku (corpus)"));
    let _ = std::fs::remove_dir_all(out.parent().unwrap());
}

// ---------------------------------------------------------------------------------------------
// LCH-6: the files of the app folder in the repository, the builds and the release.

#[test]
fn backend_entry_starts_the_program_of_the_app_folder_with_the_socket_path() {
    // LCH-6: backend/entry has the mode 755, and it starts the program `sudoku` of the same app folder
    // with the socket path as argument 1.
    use std::os::unix::fs::PermissionsExt;
    let entry = root().join("appload/sudoku/backend/entry");
    assert_eq!(mode_of(&entry), 0o755, "LCH-6: the mode of backend/entry in the repository");
    // git keeps the mode of the file. A file that git does not have yet has no mode in git.
    if let Ok(out) = Command::new("git").arg("-C").arg(root()).args(["ls-files", "-s", "appload/sudoku/backend/entry"]).output() {
        let line = String::from_utf8_lossy(&out.stdout).to_string();
        assert!(line.is_empty() || line.starts_with("100755 "), "LCH-6: the mode of backend/entry in git: {line}");
    }
    // An app folder in a path with a space, and a fake program that writes its arguments.
    let dir = scratch("entry").join("app folder");
    std::fs::create_dir_all(dir.join("backend")).unwrap();
    std::fs::copy(&entry, dir.join("backend/entry")).unwrap();
    std::fs::set_permissions(dir.join("backend/entry"), std::fs::Permissions::from_mode(0o755)).unwrap();
    let args = dir.join("args");
    std::fs::write(dir.join("sudoku"), format!("#!/bin/sh\nfor a in \"$@\"; do echo \"$a\"; done > '{}'\n", args.display())).unwrap();
    std::fs::set_permissions(dir.join("sudoku"), std::fs::Permissions::from_mode(0o755)).unwrap();
    // AppLoad starts the file with its full path, from a different working folder.
    let status = Command::new(dir.join("backend/entry")).arg("/tmp/appload socket 1").current_dir("/").status().unwrap();
    assert!(status.success());
    assert_eq!(std::fs::read_to_string(&args).unwrap(), "/tmp/appload socket 1\n", "LCH-6: argument 1 is the socket path");
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

/// The paths after each `chmod` in `text`, up to the end of the command.
fn chmod_paths(text: &str) -> Vec<String> {
    text.split("chmod ")
        .skip(1)
        .flat_map(|rest| {
            let command = rest.split("&&").next().unwrap().split('\n').next().unwrap();
            command.split_whitespace().skip(1).map(str::to_string).collect::<Vec<_>>()
        })
        .collect()
}

#[test]
fn each_build_makes_the_app_folder_of_lch_6() {
    // LCH-6: the Dockerfile, Dockerfile.dev and Dockerfile.corpus make the folder with all frontend
    // files (appload/make-folder.sh) and give `sudoku` and `backend/entry` the mode 755. The folder of
    // the colors app has no frontend file. No build copies an ink patch.
    for (file, out, variant) in [("Dockerfile", "/out/sudoku", ""), ("Dockerfile.dev", "/out/sudoku", ""), ("Dockerfile.corpus", "/out/sudoku-corpus", " corpus")] {
        let code = code_lines(&read(file));
        let make = format!("make-folder.sh {out}{variant}");
        assert!(code.lines().any(|l| l.trim_end() == format!("RUN sh /appload/{make}")), "LCH-6: {file} must run {make}");
        let chmod = chmod_paths(&code);
        for program in ["sudoku", "backend/entry"] {
            assert!(chmod.contains(&format!("{out}/{program}")), "LCH-6: {file} must set the mode of {out}/{program}: {chmod:?}");
        }
        assert!(code.contains("cp target/") && code.contains(&format!("{out}/sudoku")), "{file}: the program goes into the folder");
    }
    let colors = code_lines(&read("Dockerfile.colors"));
    assert!(!colors.contains("make-folder.sh"), "LCH-6: the colors app has no frontend");
    for name in ["resources.rcc", "backend", "application.qrc", "ui/main.qml"] {
        assert!(!colors.contains(name), "LCH-6: Dockerfile.colors has {name}");
    }
    assert!(!colors.replace("external.manifest.json", "").contains("manifest.json"), "LCH-6: Dockerfile.colors has manifest.json");
    for file in ["Dockerfile", "Dockerfile.dev", "Dockerfile.corpus", "Dockerfile.colors", "appload/make-folder.sh"] {
        let code = code_lines(&read(file));
        assert!(!code.contains(".qmd") && !code.contains("penregion"), "LCH-6: {file} names an ink patch");
    }
}

#[test]
fn the_release_has_the_files_of_lch_6_and_no_ink_patch() {
    // LCH-6: release.yml requires exactly the six files and the mode 755 of the two programs. UPD-4:
    // version.json has only version, url and sha256, and a release has no ink patch file.
    // Dockerfile.test does not copy .github/: there, this test examines nothing.
    let path = root().join(".github/workflows/release.yml");
    let Ok(text) = std::fs::read_to_string(&path) else {
        eprintln!("SKIPPED: {} is not here", path.display());
        return;
    };
    let code = code_lines(&text);
    let want = code.lines().find_map(|l| l.trim().strip_prefix("want=\"")).expect("release.yml: want=").trim_end_matches('"');
    let mut names = launch::APP_FOLDER_FILES.to_vec();
    names.sort();
    assert_eq!(want.split(' ').collect::<Vec<_>>(), names, "LCH-6");
    assert!(code.lines().any(|l| l.trim() == "for program in sudoku backend/entry; do"), "LCH-6: the mode of the two programs");
    assert!(code.contains("'{version: $version, url: $url, sha256: $sha256}' > version.json"), "UPD-4: the format of version.json");
    for word in ["patch_url", "patch_sha256", "penregion", ".qmd", "PATCH_SHA256"] {
        assert!(!code.contains(word), "UPD-4, LCH-6: release.yml has {word}");
    }
}

#[test]
fn the_repository_has_no_ink_patch_file() {
    // LCH-6 (owner decision 2026-11-12): no ink patch file stays in the repository, also not the copy
    // of the corpus app.
    fn walk(dir: &Path, found: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if path.is_dir() {
                // Build output, git data and the work folders of agents are not in the repository.
                if !["target", "dist", ".git", ".claude", ".scratchpad", ".beads", ".dolt", "node_modules"].contains(&name.as_str()) {
                    walk(&path, found);
                }
            } else if name.ends_with(".qmd") || name.contains("penregion") {
                found.push(path);
            }
        }
    }
    let mut found = Vec::new();
    walk(root(), &mut found);
    assert_eq!(found, Vec::<PathBuf>::new(), "LCH-6");
    assert!(!root().join("appload/sudoku-corpus").exists(), "LCH-6: the corpus copy of the patch is gone");
}

#[test]
fn the_update_code_of_each_earlier_release_accepts_the_app_folder() {
    // LCH-6: external.manifest.json stays, because the update code of the earlier releases refuses an
    // archive without it. The files that each earlier release requires (`REQUIRED_FILES` in its
    // src/update.rs) are in the release folder. Only with git (Dockerfile.test has no .git).
    let git = |args: &[&str]| Command::new("git").arg("-C").arg(root()).args(args).output().ok().filter(|o| o.status.success());
    let Some(tags) = git(&["tag", "-l", "v*"]) else {
        eprintln!("SKIPPED: no git");
        return;
    };
    let mut examined = 0;
    for tag in String::from_utf8_lossy(&tags.stdout).lines() {
        let Some(source) = git(&["show", &format!("{tag}:src/update.rs")]) else { continue };
        let source = String::from_utf8_lossy(&source.stdout).into_owned();
        let Some(line) = source.lines().find(|l| l.contains("REQUIRED_FILES: [&str;")) else { continue };
        let required: Vec<&str> = line.split('"').skip(1).step_by(2).collect();
        assert!(!required.is_empty(), "{tag}: {line}");
        for f in required {
            assert!(launch::APP_FOLDER_FILES.contains(&f), "LCH-6: {tag} requires {f}");
        }
        examined += 1;
    }
    eprintln!("examined the update code of {examined} releases");
}

// ---------------------------------------------------------------------------------------------
// LCH-9: the selection of the mode.

#[test]
fn the_socket_argument_selects_the_frontend_mode() {
    // LCH-9: launch = Frontend exactly when the program started with the path of the AppLoad socket as
    // argument 1. Else the external mode; restart_needed when the app folder has the frontend files
    // (UPD-10).
    for files in [false, true] {
        assert_eq!(Launch::of_start(Some("/tmp/appload-sock"), || files), Launch::Frontend);
        assert_eq!(Launch::of_start(None, || files), Launch::external(files));
    }
    assert_eq!(Launch::external(false), Launch::External(Restart::NotNeeded));
    assert_eq!(Launch::external(true), Launch::External(Restart::Needed { dialog: false }));
    assert!(Launch::Frontend.is_frontend() && !Launch::external(true).is_frontend());
    assert!(Launch::external(true).restart_needed() && !Launch::external(false).restart_needed() && !Launch::Frontend.restart_needed());

    // frontend_files: all three frontend files. A part of them is not sufficient.
    let dir = scratch("frontend-files");
    std::fs::create_dir_all(dir.join("backend")).unwrap();
    std::fs::write(dir.join("sudoku"), "").unwrap();
    std::fs::write(dir.join("external.manifest.json"), "{}").unwrap();
    assert!(!launch::has_frontend_files(&dir), "a release before the frontend");
    for (i, f) in launch::FRONTEND_FILES.iter().enumerate() {
        std::fs::write(dir.join(f), "").unwrap();
        assert_eq!(launch::has_frontend_files(&dir), i == 2, "after {f}");
    }
    std::fs::remove_file(dir.join("resources.rcc")).unwrap();
    assert!(!launch::has_frontend_files(&dir), "no resources.rcc");
    assert!(!launch::has_frontend_files(&dir.join("not-there")));
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------------------------
// LCH-7, LCH-1: the socket of the program (Linux only).

/// A listening SOCK_SEQPACKET socket as AppLoad makes it. Returns the descriptor.
#[cfg(target_os = "linux")]
fn appload_listen(path: &Path) -> i32 {
    use std::os::unix::ffi::OsStrExt;
    let _ = std::fs::remove_file(path);
    unsafe {
        let fd = libc::socket(libc::AF_UNIX, libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC, 0);
        assert!(fd >= 0);
        let mut addr: libc::sockaddr_un = std::mem::zeroed();
        addr.sun_family = libc::AF_UNIX as _;
        for (i, b) in path.as_os_str().as_bytes().iter().enumerate() {
            addr.sun_path[i] = *b as _;
        }
        assert_eq!(libc::bind(fd, &addr as *const _ as *const _, std::mem::size_of::<libc::sockaddr_un>() as _), 0);
        assert_eq!(libc::listen(fd, 1), 0);
        fd
    }
}

/// Reads one packet without a wait. None: no packet.
#[cfg(target_os = "linux")]
fn recv_now(fd: i32) -> Option<Vec<u8>> {
    let mut buf = [0u8; 256];
    let n = unsafe { libc::recv(fd, buf.as_mut_ptr() as *mut _, buf.len(), libc::MSG_DONTWAIT) };
    (n >= 0).then(|| buf[..n as usize].to_vec())
}

#[cfg(target_os = "linux")]
#[test]
fn the_send_of_the_pen_area_never_blocks() {
    // LCH-7: the send never blocks the input loop, also when the frontend does not read. LCH-3: the
    // frontend then gets the area that the program gave last. LCH-7: a send after the end of the
    // connection does not stop the app. LCH-1: the end of the connection is a stop.
    use sudoku::platform::appload::Socket;
    use std::time::{Duration, Instant};
    let path = std::env::temp_dir().join(format!("sudoku-lch7-{}.sock", std::process::id()));
    let listen = appload_listen(&path);
    let mut socket = Socket::connect(path.to_str().unwrap()).unwrap();
    // CLOEXEC: a program that a different test starts must not keep the connection open, else the
    // close below is not the end of the connection.
    let appload = unsafe { libc::accept4(listen, std::ptr::null_mut(), std::ptr::null_mut(), libc::SOCK_CLOEXEC) };
    assert!(appload >= 0);
    // AppLoad does not read. The program gives many areas.
    let start = Instant::now();
    let areas: Vec<Option<(i32, i32, i32, i32)>> = (0..5000).map(|i| if i % 7 == 0 { None } else { Some((i % 9 * 147, i % 13 * 147, 147, 147)) }).collect();
    for a in &areas {
        socket.send_pen_area(launch::pen_area_text(*a));
    }
    socket.send_pen_area(launch::pen_area_text(Some((441, 882, 147, 147))));
    assert!(start.elapsed() < Duration::from_secs(2), "LCH-7: {:?} for {} sends", start.elapsed(), areas.len() + 1);
    // AppLoad reads all packets. Each message is a header and a text; the last is the last area.
    let mut last = None;
    for _ in 0..20_000 {
        socket.flush();
        let Some(header) = recv_now(appload) else {
            if !socket.has_unsent() {
                break;
            }
            continue;
        };
        let (kind, len) = launch::read_header(&header).unwrap_or_else(|| panic!("a header: {header:?}"));
        assert_eq!(kind, launch::MSG_PEN_AREA);
        let text = (0..1000).find_map(|_| {
            socket.flush();
            recv_now(appload)
        });
        let text = text.expect("the text after the header");
        assert_eq!(text.len(), len, "the length of the header");
        assert!(!text.is_empty(), "R§14.4: a text is never empty");
        last = Some(String::from_utf8(text).unwrap());
    }
    assert!(!socket.has_unsent());
    assert_eq!(last.as_deref(), Some("441 882 147 147"), "LCH-3: the last area");
    // The end of the connection: a send does not stop the app, and the read gives the end.
    unsafe { libc::close(appload) };
    socket.send_pen_area(launch::pen_area_text(None));
    socket.send_pen_area(launch::pen_area_text(Some((0, 0, 147, 147))));
    let read = socket.try_recv();
    assert!(read.is_err(), "LCH-1: the end of the connection: {read:?}");
    unsafe { libc::close(listen) };
    let _ = std::fs::remove_file(&path);
}
