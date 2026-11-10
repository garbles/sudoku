//! The states of the reference images in `images/` (the same data as the scripts in `scripts/`). The
//! snapshot tool and the rendering tests use them to compare the app with the images. The snapshot
//! tool also writes the sheet of all icons (`icon_sheet`).

use tiny_skia::{Color as SkColor, Pixmap};

use crate::app::{App, Menu, Panel, PendingInk, Screen};
use crate::font::{Anchor, Fonts, Weight};
use crate::game::{peers, Checkpoint, Game};
use crate::render::{self, decode_png, gray, Canvas, Icon, Units, BLACK};
use crate::script::FixedEnv;
use crate::store::{NewResult, ResultsDb};
use crate::update::{Manifest, Update};

// The game of scripts/board-state.py.
const SOLUTION: [&str; 9] =
    ["534678912", "672195348", "198342567", "859761423", "426853791", "713924856", "961537284", "287419635", "345286179"];
const GIVENS: [&str; 9] =
    ["53..7....", "6..195...", ".98....6.", "8...6...3", "4..8.3..1", "7...2...6", ".6....28.", "...419..5", "....8..79"];
const ENTERED: [(u8, u8); 12] = [(0, 2), (0, 3), (1, 1), (1, 2), (2, 0), (3, 1), (4, 4), (5, 4), (6, 0), (7, 0), (7, 8), (8, 6)];
const MARKED: [(u8, u8); 14] =
    [(0, 5), (0, 6), (1, 6), (2, 3), (2, 4), (3, 2), (3, 5), (4, 2), (5, 2), (6, 3), (6, 5), (8, 1), (8, 2), (4, 6)];

/// The numbers that `board-state.py completed-group` adds. Then box 0, column 4 and row 7 are complete.
const GROUP_ENTERED: [(u8, u8); 6] = [(2, 4), (6, 4), (7, 1), (7, 2), (7, 6), (7, 7)];

/// The results of scripts/analytics.py (tests/fixtures/analytics-results.json), as (unix time in UTC,
/// mode, seconds).
fn analytics_results() -> Vec<(i64, u8, u64)> {
    let json: serde_json::Value =
        serde_json::from_str(include_str!("../tests/fixtures/analytics-results.json")).expect("fixture");
    json.as_array()
        .expect("array")
        .iter()
        .map(|r| (r["finished_at"].as_i64().unwrap(), r["mode"].as_u64().unwrap() as u8, r["seconds"].as_u64().unwrap()))
        .collect()
}

fn grid(rows: &[&str; 9]) -> [[u8; 9]; 9] {
    let mut g = [[0u8; 9]; 9];
    for (r, row) in rows.iter().enumerate() {
        for (c, ch) in row.bytes().enumerate() {
            g[r][c] = if ch == b'.' { 0 } else { ch - b'0' };
        }
    }
    g
}

/// The update state of a stable build v0.1.0 (REQUIREMENTS.md §9.2): v0.2.0 is available, or the app
/// is up to date (a request with the Settings panel open found no update).
pub fn update_state(available: bool) -> Update {
    let mut u = Update::new("v0.1.0");
    u.request = None;
    let version = if available { "v0.2.0" } else { "v0.1.0" };
    let m = Manifest { version: version.to_string(), url: String::new(), sha256: String::new(), ..Manifest::default() };
    u.checked(Ok(m), true);
    u
}

/// The update v0.2.0 is installed and replaced the ink patch (§9.2, UPD-10). `dialog`: the dialog
/// "Restart the interface?" shows (else the user tapped "Later").
fn restart_state(dialog: bool) -> Update {
    let mut u = update_state(true);
    u.tap();
    u.request = None;
    u.installed(Ok(true), 0);
    if !dialog {
        u.close_dialog();
    }
    u
}

/// The game in progress of board-state.py, made with the same operations as a player.
pub fn sample_game() -> Game {
    let sol = grid(&SOLUTION);
    let mut g = Game::new(1, 2, sol, grid(&GIVENS));
    for &(r, c) in &ENTERED {
        g.add_number((r, c), sol[r as usize][c as usize]);
    }
    g.add_number((5, 1), 9); // The clash in column 1.
    add_marks(&mut g);
    g
}

/// Pencil marks: the possible digits of each marked cell that has no number.
fn add_marks(g: &mut Game) {
    add_marks_of(g, &MARKED);
}

fn add_marks_of(g: &mut Game, marked: &[(u8, u8)]) {
    for &(r, c) in marked {
        if g.is_filled((r, c)) {
            continue;
        }
        for d in 1..=9u8 {
            if !peers((r, c)).any(|p| g.value_at(p) == d) {
                g.toggle_mark((r, c), d);
            }
        }
    }
}

/// The game of screen-completed-group.png (`board-state.py completed-group`): no clash, and box 0,
/// column 4 and row 7 are complete.
pub fn completed_group_game() -> Game {
    let sol = grid(&SOLUTION);
    let mut g = Game::new(1, 2, sol, grid(&GIVENS));
    for &(r, c) in ENTERED.iter().chain(&GROUP_ENTERED) {
        g.add_number((r, c), sol[r as usize][c as usize]);
    }
    add_marks(&mut g);
    g
}

/// The sample game with one undone change: undo and redo are available (black in the images).
fn sample_game_with_redo() -> Game {
    let mut g = sample_game();
    g.toggle_mark((8, 0), 3); // 3 is free in (8, 0) (MRK-7).
    g.undo();
    g
}

/// A board of a checkpoint (`MINI_STATES` of board-state.py): the numbers of `entered`, the clash of
/// sample_game() if `mistake`, and the marks of `marked` that are free in THIS board.
fn checkpoint(entered: &[(u8, u8)], mistake: bool, marked: &[(u8, u8)], minutes: u64, seconds: u64) -> Checkpoint {
    let sol = grid(&SOLUTION);
    let mut g = Game::new(1, 2, sol, grid(&GIVENS));
    for &(r, c) in entered {
        g.add_number((r, c), sol[r as usize][c as usize]);
    }
    if mistake {
        g.add_number((5, 1), 9);
    }
    add_marks_of(&mut g, marked);
    Checkpoint { board: g.board(), elapsed_ms: (minutes * 60 + seconds) * 1000 }
}

/// The game of checkpoints-menu*.png (REQUIREMENTS.md §9.3): the same board as
/// sample_game_with_redo(). `list` is the list of checkpoints, newest first. The clock is the time of
/// the newest row of checkpoints-menu-current.png (CKP-4).
fn game_with_checkpoints(list: Vec<Checkpoint>) -> Game {
    let mut g = sample_game_with_redo();
    g.elapsed_ms = (20 * 60 + 14) * 1000;
    g.checkpoints = list;
    g
}

/// The three rows of checkpoints-menu.png. They do not equal the board, thus all show "Restore".
fn three_checkpoints() -> Vec<Checkpoint> {
    vec![
        checkpoint(&ENTERED, false, &MARKED, 18, 32),
        checkpoint(&ENTERED[..6], false, &MARKED[..8], 11, 7),
        checkpoint(&ENTERED[..2], false, &MARKED[..4], 4, 15),
    ]
}

/// The game of checkpoints-menu.png.
pub fn checkpoints_game() -> Game {
    game_with_checkpoints(three_checkpoints())
}

/// The game of checkpoints-menu-current.png: the maximum of five rows. The newest row has the same
/// board as the game ("Current"), thus a save is not available for two causes (CKP-1).
pub fn full_checkpoints_game() -> Game {
    let group: Vec<(u8, u8)> = ENTERED.iter().chain(&GROUP_ENTERED).copied().collect();
    let mut list = vec![checkpoint(&ENTERED, true, &MARKED, 20, 14), checkpoint(&group, false, &MARKED, 19, 26)];
    list.extend(three_checkpoints());
    game_with_checkpoints(list)
}

pub fn completed_game() -> Game {
    let sol = grid(&SOLUTION);
    let mut g = Game::new(1, 2, sol, grid(&GIVENS));
    for (r, c) in crate::game::cells() {
        g.add_number((r, c), sol[r as usize][c as usize]);
    }
    g.elapsed_ms = (23 * 60 + 41) * 1000;
    g
}

/// The empty board of screen.png. It is not a puzzle; only the images use it.
fn empty_game() -> Game {
    let mut g = Game::new(1, 2, grid(&SOLUTION), [[0; 9]; 9]);
    g.completed = false;
    g
}

pub fn app(fonts: &Fonts) -> App {
    let mut db = ResultsDb::in_memory();
    for (i, (t, mode, seconds)) in analytics_results().into_iter().enumerate() {
        db.insert(&NewResult { finished_at: t, mode, givens: 30, seconds, seed: i as u64, generator: 2, symmetric: false }).unwrap();
    }
    let mut app = App::new(None, None, db, Box::new(FixedEnv(7)), render::ng_stops(fonts));
    // The app opens with the menu open (PER-2), but most images show the menu closed. Each state
    // that shows the open menu sets it (for example screen-toolbar.png).
    app.ui.menu = Menu::Closed;
    app
}

pub type Setup = fn(&mut App);

/// The reference states: the image name and the change from a new app.
pub fn states() -> Vec<(&'static str, Setup)> {
    vec![
        ("screen.png", |a| a.game = empty_game()),
        ("screen-in-progress.png", |a| a.game = sample_game()),
        ("screen-highlight.png", |a| {
            a.game = sample_game();
            a.ui.highlight = Some(5)
        }),
        ("screen-unrecognized.png", |a| {
            a.game = sample_game();
            a.ui.question = Some(((4, 1), 0))
        }),
        ("screen-invalid-mark.png", |a| {
            a.game = sample_game();
            a.ui.refused = Some(((1, 6), 6, 0))
        }),
        ("screen-completed-group.png", |a| a.game = completed_group_game()),
        ("screen-completed.png", |a| a.game = completed_game()),
        ("screen-toolbar.png", |a| {
            a.game = empty_game();
            a.ui.menu = Menu::Open(None)
        }),
        ("new-game-menu.png", |a| {
            a.game = empty_game();
            a.ui.menu = Menu::Open(Some(Panel::NewGame))
        }),
        ("color-menu.png", |a| {
            a.game = sample_game_with_redo();
            a.ui.menu = Menu::Open(Some(Panel::Colors))
        }),
        ("color-menu-right-handed.png", |a| {
            // The same state as color-menu.png, for a right-handed player (REQUIREMENTS.md §3.1).
            crate::layout::set_right_handed(true);
            a.game = sample_game_with_redo();
            a.ui.menu = Menu::Open(Some(Panel::Colors))
        }),
        // The same state as color-menu.png with the lightest and the darkest micro-cell lines (§10).
        ("color-menu-lines-90.png", |a| {
            a.game = sample_game_with_redo();
            a.settings.micro_line = crate::settings::LineGray::Gray90;
            a.ui.menu = Menu::Open(Some(Panel::Colors))
        }),
        ("color-menu-lines-40.png", |a| {
            a.game = sample_game_with_redo();
            a.settings.micro_line = crate::settings::LineGray::Gray40;
            a.ui.menu = Menu::Open(Some(Panel::Colors))
        }),
        ("screen-completed-menu.png", |a| {
            a.game = completed_game();
            a.ui.menu = Menu::Open(None)
        }),
        ("checkpoints-menu-empty.png", |a| {
            a.game = game_with_checkpoints(Vec::new());
            a.ui.menu = Menu::Open(Some(Panel::Checkpoints))
        }),
        ("checkpoints-menu.png", |a| {
            a.game = checkpoints_game();
            a.ui.menu = Menu::Open(Some(Panel::Checkpoints))
        }),
        ("checkpoints-menu-current.png", |a| {
            a.game = full_checkpoints_game();
            a.ui.menu = Menu::Open(Some(Panel::Checkpoints))
        }),
        ("checkpoints-menu-right-handed.png", |a| {
            // The same state as checkpoints-menu.png, for a right-handed player (NAV-18). The state
            // of checkpoints-menu.png in landscape gives checkpoints-menu-landscape.png.
            crate::layout::set_right_handed(true);
            a.game = checkpoints_game();
            a.ui.menu = Menu::Open(Some(Panel::Checkpoints))
        }),
        ("settings-menu.png", |a| {
            a.game = sample_game_with_redo();
            a.ui.menu = Menu::Open(Some(Panel::Settings))
        }),
        ("screen-toolbar-update.png", |a| {
            // REQUIREMENTS.md §9.2: an update is available, and the gear has the caution icon.
            a.game = sample_game_with_redo();
            a.update = update_state(true);
            a.ui.menu = Menu::Open(None)
        }),
        ("settings-update.png", |a| {
            a.game = sample_game_with_redo();
            a.update = update_state(true);
            a.ui.menu = Menu::Open(Some(Panel::Settings))
        }),
        ("settings-up-to-date.png", |a| {
            a.game = sample_game_with_redo();
            a.update = update_state(false);
            a.ui.menu = Menu::Open(Some(Panel::Settings))
        }),
        ("settings-check-updates.png", |a| {
            // UPD-9: the Settings panel closed after "Sudoku is up to date" and opened again.
            a.game = sample_game_with_redo();
            a.update = update_state(false);
            a.update.panel_closed();
            a.ui.menu = Menu::Open(Some(Panel::Settings))
        }),
        ("settings-restart.png", |a| {
            // UPD-10: the update replaced the ink patch, and the user tapped "Later" in the dialog.
            a.game = sample_game_with_redo();
            a.update = restart_state(false);
            a.ui.menu = Menu::Open(Some(Panel::Settings))
        }),
        ("update-restart.png", |a| {
            // UPD-10: the installation is complete and replaced the ink patch.
            a.game = sample_game_with_redo();
            a.update = restart_state(true);
            a.ui.menu = Menu::Open(Some(Panel::Settings))
        }),
        ("analytics.png", |a| a.screen = Screen::Statistics),
        ("analytics-delete-all.png", |a| {
            a.screen = Screen::Statistics;
            a.confirm_all = true
        }),
        ("analytics-games.png", |a| a.screen = Screen::GamesList),
        ("analytics-delete.png", |a| {
            a.screen = Screen::GamesList;
            a.list.confirm = Some(a.results[1].clone())
        }),
        // R§9.4: the first page of the Licenses list, and the first page of the license text of the
        // UJI data (its text does not change when the dependencies change).
        ("licenses.png", |a| a.screen = Screen::Licenses),
        ("licenses-text.png", |a| {
            a.screen = Screen::LicenseText;
            a.licenses.open = crate::licenses::all().crates.iter().position(|c| c.name == "UJI Pen Characters").expect("UJI entry")
        }),
        ("manual-how-to-play.png", |a| {
            a.screen = Screen::Manual;
            a.manual_page = 1
        }),
        ("manual-finished.png", |a| {
            a.screen = Screen::Manual;
            a.manual_page = 9
        }),
        // CMP-7, R§3.3: the result card in landscape, as a column at the side opposite the toolbar.
        // These states set the orientation; `render` sets the old orientation again after the drawing.
        ("screen-completed-landscape.png", |a| {
            crate::layout::set_orientation(crate::layout::Orientation::Landscape);
            a.game = completed_game()
        }),
        ("screen-completed-landscape-right-handed.png", |a| {
            crate::layout::set_orientation(crate::layout::Orientation::Landscape);
            crate::layout::set_right_handed(true);
            a.game = completed_game()
        }),
    ]
}

/// A state that has no reference image: ink that waits for the recognition pause.
pub fn ink_state(a: &mut App) {
    a.game = sample_game();
    a.ui.pending_ink = Some(PendingInk {
        cell: (4, 1),
        strokes: vec![vec![(320.0, 1040.0), (330.0, 1010.0), (330.0, 1110.0)]],
        last_lift_at: Some(0),
    });
}

pub fn render(fonts: &Fonts, setup: Setup) -> Pixmap {
    // The images are left-handed, if the setup does not change this. A setup can also change the
    // orientation: after the drawing, the orientation of the caller applies again.
    let orientation = crate::layout::orientation();
    crate::layout::set_right_handed(false);
    let mut a = app(fonts);
    setup(&mut a);
    let mut pm = Pixmap::new(crate::layout::screen_w(), crate::layout::screen_h()).unwrap();
    render::draw(&mut Canvas::new(&mut pm, fonts), &a);
    crate::layout::set_right_handed(false);
    crate::layout::set_orientation(orientation);
    pm
}

/// The size of a tile of the icon sheet (px), and the quantity of tiles in a row.
pub const ICON_TILE_W: u32 = 200;
pub const ICON_TILE_H: u32 = 210;
pub const ICON_COLUMNS: u32 = 6;
/// The center of the icon and the base line of the name in a tile, from the top of the tile.
pub const ICON_CENTER_Y: u32 = 85;
const ICON_LABEL_Y: u32 = 195;

/// The look of an icon in the sheet: the colors that the app gives it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IconLook {
    /// Black, on the white screen.
    Normal,
    /// White on the black slot of a toolbar button with an open panel (NAV-8).
    Active,
    /// The gray of an action that is not available.
    Unavailable(u32),
}

/// All icons in all their looks, in the order of the tiles of the sheet.
pub fn icon_tiles() -> Vec<(Icon, IconLook)> {
    Icon::ALL
        .into_iter()
        .flat_map(|icon| {
            let other = match icon {
                Icon::PaintBucket | Icon::Gear | Icon::Plus => vec![IconLook::Active],
                Icon::Undo | Icon::Redo | Icon::Bin => vec![IconLook::Unavailable(70)],
                Icon::PagePrevious | Icon::PageNext => vec![IconLook::Unavailable(80)],
                // White on the black slot of the open panel (NAV-8), gray on a complete game (CKP-9).
                Icon::Bookmark => vec![IconLook::Active, IconLook::Unavailable(70)],
                _ => vec![],
            };
            std::iter::once(IconLook::Normal).chain(other).map(move |look| (icon, look))
        })
        .collect()
}

/// The sheet of all icons at the true size (the icons of the pages at the page scale), with the name
/// of each icon. The last tile is the icon of AppLoad (`appload/sudoku/icon.png`).
pub fn icon_sheet(fonts: &Fonts) -> Pixmap {
    let tiles = icon_tiles();
    let rows = (tiles.len() as u32 + 1).div_ceil(ICON_COLUMNS);
    let mut pm = Pixmap::new(ICON_COLUMNS * ICON_TILE_W, rows * ICON_TILE_H).unwrap();
    pm.fill(SkColor::WHITE);
    let mut c = Canvas::new(&mut pm, fonts);
    let origin = |i: u32| ((i % ICON_COLUMNS * ICON_TILE_W) as f32, (i / ICON_COLUMNS * ICON_TILE_H) as f32);
    let center = |(x, y): (f32, f32)| (x + (ICON_TILE_W / 2) as f32, y + ICON_CENTER_Y as f32);
    let label = |c: &mut Canvas, (x, y): (f32, f32), name: &str| {
        c.text(Weight::Regular, 20.0, x + (ICON_TILE_W / 2) as f32, y + ICON_LABEL_Y as f32, Anchor::Middle, name, render::SECONDARY);
    };
    for (i, &(icon, look)) in tiles.iter().enumerate() {
        let o = origin(i as u32);
        let (cx, cy) = center(o);
        let col = match look {
            IconLook::Normal => BLACK,
            IconLook::Active => {
                c.rect(cx - 56.0, cy - 56.0, cx + 56.0, cy + 56.0, BLACK);
                [255, 255, 255]
            }
            IconLook::Unavailable(n) => gray(n),
        };
        if icon.on_page() {
            c.set_units(Units::Page);
            icon.draw(&mut c, cx / crate::layout::page_scale(), cy / crate::layout::page_scale(), col);
            c.set_units(Units::Screen);
        } else {
            icon.draw(&mut c, cx, cy, col);
        }
        let name = match look {
            IconLook::Normal => format!("{icon:?}"),
            IconLook::Active => format!("{icon:?} (active)"),
            IconLook::Unavailable(_) => format!("{icon:?} (gray)"),
        };
        label(&mut c, o, &name);
    }
    let o = origin(tiles.len() as u32);
    let (cx, cy) = center(o);
    // Read at run time: the Docker builds of the app do not have this file when they compile.
    let app_icon = decode_png(&std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("appload/sudoku/icon.png")).unwrap());
    c.image(&app_icon, cx as i32 - app_icon.width() as i32 / 2, cy as i32 - app_icon.height() as i32 / 2, false);
    label(&mut c, o, "AppLoad");
    pm
}

/// The mean absolute difference (0..255) and the fraction of pixels with a difference of more than 64.
pub fn compare(a: &Pixmap, b: &Pixmap) -> (f64, f64) {
    let (mut sum, mut big) = (0u64, 0u64);
    let n = (a.width() * a.height()) as u64;
    for (p, q) in a.pixels().iter().zip(b.pixels()) {
        let d = (p.red() as i32 - q.red() as i32).abs()
            + (p.green() as i32 - q.green() as i32).abs()
            + (p.blue() as i32 - q.blue() as i32).abs();
        sum += d as u64;
        if d / 3 > 64 {
            big += 1;
        }
    }
    (sum as f64 / n as f64 / 3.0, big as f64 / n as f64)
}

/// Uses UTC for the dates, as scripts/analytics.py does.
pub fn use_utc() {
    std::env::set_var("TZ", "UTC");
    extern "C" {
        fn tzset();
    }
    // SAFETY: tzset only reads TZ.
    unsafe { tzset() };
}

/// The pixels that differ by more than 64 and that are not near an edge of the reference image. A
/// pixel is near an edge if the reference has a contrast of more than 64 within `radius` pixels.
/// Anti-aliasing (of the design images) changes only pixels near edges. Thus a result other than 0 is a real difference (for
/// example a missing item or an item at a different position).
pub fn differences_off_edges(ours: &Pixmap, reference: &Pixmap, radius: i32) -> Vec<(u32, u32)> {
    let (w, h) = (reference.width() as i32, reference.height() as i32);
    let lum = |pm: &Pixmap, x: i32, y: i32| {
        let p = pm.pixel(x as u32, y as u32).unwrap();
        (p.red() as i32 * 299 + p.green() as i32 * 587 + p.blue() as i32 * 114) / 1000
    };
    let mut out = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let p = ours.pixel(x as u32, y as u32).unwrap();
            let q = reference.pixel(x as u32, y as u32).unwrap();
            let d = (p.red() as i32 - q.red() as i32).abs()
                + (p.green() as i32 - q.green() as i32).abs()
                + (p.blue() as i32 - q.blue() as i32).abs();
            if d / 3 <= 64 {
                continue;
            }
            let (mut lo, mut hi) = (255, 0);
            for yy in (y - radius).max(0)..=(y + radius).min(h - 1) {
                for xx in (x - radius).max(0)..=(x + radius).min(w - 1) {
                    let l = lum(reference, xx, yy);
                    lo = lo.min(l);
                    hi = hi.max(l);
                }
            }
            if hi - lo <= 64 {
                out.push((x as u32, y as u32));
            }
        }
    }
    out
}
