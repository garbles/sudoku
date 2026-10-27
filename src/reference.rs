//! The states of the reference images in `images/` (the same data as the scripts in `scripts/`). The
//! snapshot tool and the rendering tests use them to compare the app with the images.

use tiny_skia::Pixmap;

use crate::app::{App, Env, Panel, PendingInk, Screen};
use crate::font::Fonts;
use crate::game::{peers, Game};
use crate::render::{self, Canvas};
use crate::store::{NewResult, ResultsDb};

struct FixedEnv;

impl Env for FixedEnv {
    fn unix_time(&mut self) -> i64 {
        1_792_900_000
    }
    fn new_seed(&mut self) -> u64 {
        7
    }
}

// The game of scripts/board-state.py.
const SOLUTION: [&str; 9] =
    ["534678912", "672195348", "198342567", "859761423", "426853791", "713924856", "961537284", "287419635", "345286179"];
const GIVENS: [&str; 9] =
    ["53..7....", "6..195...", ".98....6.", "8...6...3", "4..8.3..1", "7...2...6", ".6....28.", "...419..5", "....8..79"];
const ENTERED: [(u8, u8); 12] = [(0, 2), (0, 3), (1, 1), (1, 2), (2, 0), (3, 1), (4, 4), (5, 4), (6, 0), (7, 0), (7, 8), (8, 6)];
const MARKED: [(u8, u8); 14] =
    [(0, 5), (0, 6), (1, 6), (2, 3), (2, 4), (3, 2), (3, 5), (4, 2), (5, 2), (6, 3), (6, 5), (8, 1), (8, 2), (4, 6)];

/// The results of scripts/analytics.py (tests/fixtures/analytics-results.json), as (unix time in UTC,
/// mode, seconds).
pub fn analytics_results() -> Vec<(i64, u8, u64)> {
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

/// The game in progress of board-state.py, made with the same operations as a player.
pub fn sample_game() -> Game {
    let sol = grid(&SOLUTION);
    let mut g = Game::new(1, 2, sol, grid(&GIVENS));
    for &(r, c) in &ENTERED {
        g.add_number((r, c), sol[r as usize][c as usize]);
    }
    g.add_number((5, 1), 9); // The clash in column 1.
    // Pencil marks: the possible digits of each marked cell.
    for &(r, c) in &MARKED {
        for d in 1..=9u8 {
            if !peers((r, c)).any(|p| g.value_at(p) == d) {
                g.toggle_mark((r, c), d);
            }
        }
    }
    g
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
pub fn empty_game() -> Game {
    let mut g = Game::new(1, 2, grid(&SOLUTION), [[0; 9]; 9]);
    g.completed = false;
    g
}

pub fn app(fonts: &Fonts) -> App {
    let mut db = ResultsDb::in_memory();
    for (i, (t, mode, seconds)) in analytics_results().into_iter().enumerate() {
        db.insert(&NewResult { finished_at: t, mode, givens: 30, seconds, seed: i as u64, generator: 2 }).unwrap();
    }
    let mut app = App::new(None, None, db, Box::new(FixedEnv));
    app.ng_stops = render::ng_stops(fonts);
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
        ("screen-completed.png", |a| a.game = completed_game()),
        ("screen-toolbar.png", |a| {
            a.game = empty_game();
            a.ui.menu_open = true
        }),
        ("new-game-menu.png", |a| {
            a.game = empty_game();
            a.ui.menu_open = true;
            a.ui.panel = Panel::NewGame
        }),
        ("color-menu.png", |a| {
            // The image shows undo and redo in black: a change was undone.
            a.game = sample_game();
            a.game.toggle_mark((8, 0), 1);
            a.game.undo();
            a.ui.menu_open = true;
            a.ui.panel = Panel::Colors
        }),
        ("color-menu-right-handed.png", |a| {
            // The same state as color-menu.png, for a right-handed player (REQUIREMENTS.md §3.1).
            crate::layout::set_right_handed(true);
            a.game = sample_game();
            a.game.toggle_mark((8, 0), 1);
            a.game.undo();
            a.ui.menu_open = true;
            a.ui.panel = Panel::Colors
        }),
        ("screen-completed-menu.png", |a| {
            a.game = completed_game();
            a.ui.menu_open = true
        }),
        ("settings-menu.png", |a| {
            a.game = sample_game();
            a.game.toggle_mark((8, 0), 1);
            a.game.undo();
            a.ui.menu_open = true;
            a.ui.panel = Panel::Settings
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
        ("manual-how-to-play.png", |a| {
            a.screen = Screen::Manual;
            a.manual_page = 1
        }),
        ("manual-finished.png", |a| {
            a.screen = Screen::Manual;
            a.manual_page = 9
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
    // The images are left-handed, if the setup does not change this.
    crate::layout::set_right_handed(false);
    let mut a = app(fonts);
    setup(&mut a);
    let mut pm = Pixmap::new(crate::layout::screen_w(), crate::layout::screen_h()).unwrap();
    render::draw(&mut Canvas::new(&mut pm, fonts), &a);
    crate::layout::set_right_handed(false);
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
/// Antialiasing changes only pixels near edges. Thus a result other than 0 is a real difference (for
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
