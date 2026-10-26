//! Property-based tests: random sequences of player input. After each input, the debug build checks all
//! state rules of INVARIANTS.md (`invariants::check_app`, in `App::handle`). This test also checks the
//! rules about two states one after the other (CLK-2, CLK-3, CMP-5, INP-1, INP-9, UND-2, UND-3, ANA-2).

mod common;

use common::*;
use proptest::prelude::*;
use sudoku::app::{Input, Panel, Screen};
use sudoku::game::Game;
use sudoku::invariants::check_app;
use sudoku::layout::{self, Button};

#[derive(Clone, Debug)]
enum Act {
    TapMicro(u8, u8, u8),
    Write(u8, u8, u8),
    WriteSolution(u8, u8),
    Scribble(u8, u8),
    Erase(u8, u8),
    FingerBoard(u8, u8),
    Button(u8),
    Point(u16, u16, bool),
    Swipe(bool),
    Wait(u16),
    UndoRedo,
    Solve,
    Suspend,
    /// Opens the New game panel, selects a mode and maybe starts a new game.
    NewGame(u8, bool),
    /// Opens the Cell colors panel and taps a color.
    PickColor(u8, u8),
    /// A tap on a control of the analytics screens.
    ListControl(u8),
    /// Goes to the games list, taps a bin and maybe confirms the dialog.
    DeleteFlow(u8, Option<bool>),
    /// Opens the Settings panel and taps a switch.
    Switch(u8),
    /// Goes to the statistics, taps "Delete all games" and maybe confirms.
    DeleteAllFlow(Option<bool>),
}

fn act() -> impl Strategy<Value = Act> {
    let cell = (0..9u8, 0..9u8);
    prop_oneof![
        4 => (cell.clone(), 1..=9u8).prop_map(|((r, c), d)| Act::TapMicro(r, c, d)),
        3 => (cell.clone(), 1..=9u8).prop_map(|((r, c), d)| Act::Write(r, c, d)),
        4 => cell.clone().prop_map(|(r, c)| Act::WriteSolution(r, c)),
        1 => cell.clone().prop_map(|(r, c)| Act::Scribble(r, c)),
        2 => cell.clone().prop_map(|(r, c)| Act::Erase(r, c)),
        1 => cell.prop_map(|(r, c)| Act::FingerBoard(r, c)),
        5 => (0..8u8).prop_map(Act::Button),
        4 => (0..1620u16, 0..2160u16, any::<bool>()).prop_map(|(x, y, pen)| Act::Point(x, y, pen)),
        1 => any::<bool>().prop_map(Act::Swipe),
        2 => (0..3000u16).prop_map(Act::Wait),
        1 => Just(Act::UndoRedo),
        1 => Just(Act::Solve),
        1 => Just(Act::Suspend),
        1 => (0..5u8, any::<bool>()).prop_map(|(m, s)| Act::NewGame(m, s)),
        2 => (0..3u8, 0..9u8).prop_map(|(s, i)| Act::PickColor(s, i)),
        4 => (0..22u8).prop_map(Act::ListControl),
        1 => (0..12u8, proptest::option::of(any::<bool>())).prop_map(|(i, c)| Act::DeleteFlow(i, c)),
        2 => (0..2u8).prop_map(Act::Switch),
        1 => proptest::option::of(any::<bool>()).prop_map(Act::DeleteAllFlow),
    ]
}

const BUTTONS: [Button; 8] =
    [Button::Menu, Button::Colors, Button::Undo, Button::Redo, Button::Help, Button::Statistics, Button::NewGame, Button::Close];

/// The board state that only changes to the board can change: seed, values, marks, the lengths of the
/// two stacks and the completion.
type Board = (u64, [[u8; 9]; 9], [[u16; 9]; 9], usize, usize, bool);

fn board(g: &Game) -> Board {
    (g.seed, g.value, g.marks, g.undo.len(), g.redo.len(), g.completed)
}

fn run(d: &mut Driver, a: &Act) {
    match *a {
        Act::TapMicro(r, c, digit) => d.tap_micro((r, c), digit),
        Act::Write(r, c, digit) => d.write((r, c), digit),
        Act::WriteSolution(r, c) => {
            let v = d.app.game.solution[r as usize][c as usize];
            d.write((r, c), v)
        }
        Act::Scribble(r, c) => {
            d.write_strokes(&scribble_ink((r, c)));
            d.wait(d.app.pause_ms + 10);
        }
        Act::Erase(r, c) => d.erase((r, c)),
        Act::FingerBoard(r, c) => {
            let (x, y) = micro_center((r, c), 5);
            d.finger_tap(x, y);
        }
        Act::Button(i) => d.button(BUTTONS[i as usize]),
        Act::Point(x, y, pen) => {
            if pen {
                d.pen_tap(x as f32, y as f32)
            } else {
                d.finger_tap(x as f32, y as f32)
            }
        }
        Act::Swipe(left) => d.swipe(left),
        Act::Wait(ms) => d.wait(ms as u64),
        Act::UndoRedo => {
            if d.app.screen == Screen::Game && d.app.ui.menu_open && d.app.ui.panel == Panel::None && d.app.ui.pending_ink.is_none() && d.app.game.can_undo() {
                let before = board(&d.app.game);
                d.button(Button::Undo);
                d.button(Button::Redo);
                assert_eq!(board(&d.app.game).1, before.1, "UND-3");
                assert_eq!(board(&d.app.game).2, before.2, "UND-3");
            }
        }
        Act::Solve => {
            if d.app.screen == Screen::Game && d.app.ui.panel == Panel::None && !d.app.game.completed {
                // Close the menu, so that no cell is below a panel.
                if d.app.ui.menu_open {
                    d.button(Button::Menu);
                }
                solve(d);
            }
        }
        Act::Suspend => {
            d.app.suspend();
            d.now += 600_000;
            d.app.tick(d.now);
        }
        Act::NewGame(mode, start) => {
            if d.app.screen != Screen::Game {
                return;
            }
            if !d.app.ui.menu_open {
                d.button(Button::Menu);
            }
            if d.app.ui.panel != Panel::NewGame {
                d.button(Button::NewGame);
            }
            let x = d.app.ng_stops[mode as usize];
            d.finger_tap(x, layout::ng_slider_y() as f32);
            if start {
                d.finger_tap(1100.0, (layout::tb_bottom_y() + 56) as f32);
            }
        }
        Act::PickColor(section, i) => {
            if d.app.screen != Screen::Game {
                return;
            }
            if !d.app.ui.menu_open {
                d.button(Button::Menu);
            }
            if d.app.ui.panel != Panel::Colors {
                d.button(Button::Colors);
            }
            let (gx, gy) = layout::color_cell(section as usize, i as usize);
            d.pen_tap(gx as f32 + 56.0, gy as f32 + 40.0);
        }
        Act::DeleteFlow(i, confirm) => {
            let center = |r: layout::Rect| (((r.x0 + r.x1) / 2) as f32, ((r.y0 + r.y1) / 2) as f32);
            if d.app.screen == Screen::Game {
                if d.app.ui.panel != Panel::None {
                    d.button(Button::Menu);
                }
                if !d.app.ui.menu_open {
                    d.button(Button::Menu);
                }
                d.button(Button::Statistics);
            }
            if d.app.screen == Screen::Statistics {
                let (x, y) = center(layout::all_games_button());
                d.finger_tap(x, y);
            }
            if d.app.screen == Screen::GamesList && d.app.list.confirm.is_none() {
                let (x, y) = center(layout::bin_button(i as usize));
                d.finger_tap(x, y);
            }
            if let Some(c) = confirm {
                let (x, y) = center(if c { layout::dialog_delete() } else { layout::dialog_cancel() });
                d.finger_tap(x, y);
            }
        }
        Act::Switch(i) => {
            if d.app.screen != Screen::Game {
                return;
            }
            if !d.app.ui.menu_open {
                d.button(Button::Menu);
            }
            if d.app.ui.panel != Panel::Settings {
                d.button(Button::Settings);
            }
            let r = layout::settings_row(i as usize);
            d.finger_tap((r.x0 + r.x1) as f32 / 2.0, (r.y0 + r.y1) as f32 / 2.0);
        }
        Act::DeleteAllFlow(confirm) => {
            let center = |r: layout::Rect| (((r.x0 + r.x1) / 2) as f32, ((r.y0 + r.y1) / 2) as f32);
            if d.app.screen == Screen::Game {
                if d.app.ui.panel != Panel::None {
                    d.button(Button::Menu);
                }
                if !d.app.ui.menu_open {
                    d.button(Button::Menu);
                }
                d.button(Button::Statistics);
            }
            if d.app.screen == Screen::Statistics {
                let (x, y) = center(layout::delete_all_button());
                d.finger_tap(x, y);
                if let Some(c) = confirm {
                    let (x, y) = center(if c { layout::dialog_delete() } else { layout::dialog_cancel() });
                    d.finger_tap(x, y);
                }
            }
        }
        Act::ListControl(k) => {
            let center = |r: layout::Rect| (((r.x0 + r.x1) / 2) as f32, ((r.y0 + r.y1) / 2) as f32);
            let r = match k {
                0 => layout::back_button(),
                1 => layout::all_games_button(),
                2 => layout::prev_arrow(),
                3 => layout::next_arrow(),
                4 => layout::dialog_cancel(),
                5 => layout::dialog_delete(),
                6..=11 => layout::filter_segment(k as usize - 6),
                _ => layout::bin_button(k as usize - 12),
            };
            let (x, y) = center(r);
            d.finger_tap(x, y);
        }
    }
}

/// True if the action can not change the board when no ink waits.
fn is_not_a_board_action(a: &Act, d: &Driver) -> bool {
    match a {
        Act::FingerBoard(..) | Act::Swipe(_) | Act::Suspend => true,
        Act::Button(i) => matches!(BUTTONS[*i as usize], Button::Menu | Button::Colors | Button::Help | Button::Statistics | Button::Close)
            || (BUTTONS[*i as usize] == Button::NewGame && d.app.ui.panel != Panel::NewGame),
        Act::Wait(_) | Act::PickColor(..) | Act::Switch(_) => true,
        Act::DeleteAllFlow(_) => d.app.screen != Screen::Game || d.app.ui.pending_ink.is_none(),
        Act::ListControl(_) => d.app.screen != Screen::Game,
        _ => false,
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 96, max_shrink_iters: 2000, ..ProptestConfig::default() })]

    #[test]
    fn invariants_hold_for_all_input(seed in any::<u64>(), acts in proptest::collection::vec(act(), 1..90)) {
        let mut d = Driver::new(seed);
        check_app(&d.app).unwrap();
        for a in &acts {
            let before = d.app.game.clone();
            let results_before = d.app.results.clone();
            let screen_before = d.app.screen;
            let confirm_all_before = d.app.confirm_all;
            let ink_before = d.app.ui.pending_ink.is_some();
            let not_board = is_not_a_board_action(a, &d) && !ink_before;

            run(&mut d, a);
            check_app(&d.app).map_err(|e| TestCaseError::fail(format!("{e} after {a:?}")))?;

            let g = &d.app.game;
            let same_game = g.seed == before.seed && (g.undo.len() + g.redo.len() > 0 || before.undo.len() + before.redo.len() == 0 || g.elapsed_ms >= before.elapsed_ms);
            let new_game = g.seed != before.seed;
            // CLK-2: the clock never goes back, except for a new game.
            if !new_game && same_game {
                prop_assert!(g.elapsed_ms >= before.elapsed_ms, "CLK-2 after {:?}", a);
            }
            // CLK-3, INP-9, CMP-3: a complete game does not change.
            if before.completed && !new_game {
                prop_assert_eq!(board(g), board(&before), "INP-9 after {:?}", a);
                prop_assert_eq!(g.elapsed_ms, before.elapsed_ms, "CLK-3 after {:?}", a);
            }
            // UND-2, INP-1, HLT-5, COL-4: actions that are not changes to the board.
            if not_board && !new_game {
                prop_assert_eq!(board(g), board(&before), "UND-2 after {:?}", a);
            }
            // UND-6, DIF-5
            if new_game {
                prop_assert!(g.undo.is_empty() && g.redo.is_empty() && !g.completed, "UND-6");
                prop_assert_eq!(g.mode, d.app.settings.last_mode, "DIF-5");
            }
            // CMP-5, ANA-2: results change only by completion (one row) or by a delete on the list.
            if !before.completed && g.completed && !new_game {
                prop_assert_eq!(d.app.results.len(), results_before.len() + 1, "CMP-5");
                let r = d.app.results.iter().find(|r| !results_before.contains(r)).unwrap();
                prop_assert_eq!((r.mode, r.givens, r.seconds, r.seed), (g.mode, g.givens_count(), g.seconds(), g.seed));
            } else if (matches!(a, Act::DeleteAllFlow(Some(true))) || confirm_all_before) && d.app.results.len() < results_before.len() {
                prop_assert!(d.app.results.is_empty(), "ANA-12: Delete all removes all results");
            } else if (screen_before == Screen::GamesList || matches!(a, Act::DeleteFlow(..))) && d.app.results.len() < results_before.len() {
                prop_assert_eq!(d.app.results.len() + 1, results_before.len(), "one delete");
            } else {
                prop_assert_eq!(&d.app.results, &results_before, "ANA-2 after {:?}", a);
            }
            if d.app.quit {
                // The close button: the app saves and stops. The test continues with a new start.
                prop_assert!(d.app.game_dirty, "NAV-6: the game is saved before the app closes");
                d.app.quit = false;
            }
        }
    }
}

#[test]
fn panels_never_cover_the_toolbar_and_the_toolbar_never_covers_the_board() {
    // NAV-10
    for o in [layout::Orientation::Portrait, layout::Orientation::Landscape] {
        layout::set_orientation(o);
        assert!(layout::colors_panel().x1 <= layout::tb_x(), "{o:?}");
        assert!(layout::ng_panel().x1 <= layout::tb_x(), "{o:?}");
        assert!(layout::board_x() + layout::BOARD_SIZE < layout::tb_x(), "{o:?}");
        assert!(layout::colors_panel().y1 <= layout::screen_h() as i32, "{o:?}");
        assert!(layout::meter_y() + 20 < layout::screen_h() as i32, "{o:?}");
    }
    layout::set_orientation(layout::Orientation::Portrait);
}

#[test]
fn touch_with_two_fingers_uses_only_the_first() {
    let mut d = Driver::new(3);
    d.send(Input::TouchDown { id: 1, x: 1563.0, y: 55.0 });
    d.send(Input::TouchDown { id: 2, x: 1563.0, y: 55.0 });
    d.send(Input::TouchUp { id: 2, x: 1563.0, y: 55.0 });
    assert!(!d.app.ui.menu_open);
    d.send(Input::TouchUp { id: 1, x: 1563.0, y: 55.0 });
    assert!(d.app.ui.menu_open);
}

#[test]
#[ignore]
fn coverage_of_states() {
    use proptest::strategy::ValueTree;
    use proptest::test_runner::TestRunner;
    let mut runner = TestRunner::deterministic();
    let mut seen = std::collections::BTreeMap::<&str, usize>::new();
    for case in 0..96 {
        let acts = proptest::collection::vec(act(), 1..90).new_tree(&mut runner).unwrap().current();
        let mut d = Driver::new(case);
        for a in &acts {
            run(&mut d, a);
            d.app.quit = false;
            let a = &d.app;
            let mut mark = |k: &'static str, b: bool| if b { *seen.entry(k).or_default() += 1 };
            mark("completed", a.game.completed);
            mark("highlight", a.ui.highlight.is_some());
            mark("question", a.ui.question.is_some());
            mark("menu", a.ui.menu_open);
            mark("colors", a.ui.panel == Panel::Colors);
            mark("newgame", a.ui.panel == Panel::NewGame);
            mark("manual", a.screen == Screen::Manual);
            mark("manual>1", a.manual_page > 1);
            mark("analytics", a.screen == Screen::Statistics);
            mark("list", a.screen == Screen::GamesList);
            mark("confirm", a.list.confirm.is_some());
            mark("results", !a.results.is_empty());
            mark("clash", sudoku::game::cells().any(|c| a.game.is_clashing(c)));
            mark("redo", !a.game.redo.is_empty());
            mark("mode!=2", a.game.mode != 2);
            mark("color!=default", a.settings != Default::default());
            mark("list page>1", a.list.page > 1);
        }
    }
    println!("{seen:#?}");
}

/// The screen without the ink (the platform draws the ink in parts while the pen writes).
fn render_without_ink(app: &mut sudoku::app::App, fonts: &sudoku::font::Fonts) -> tiny_skia::Pixmap {
    let pending = app.ui.pending_ink.take();
    let live = app.live_stroke.take();
    let mut pm = tiny_skia::Pixmap::new(1620, 2160).unwrap();
    sudoku::render::draw(&mut sudoku::render::Canvas::new(&mut pm, fonts), app);
    app.ui.pending_ink = pending;
    app.live_stroke = live;
    pm
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 24, max_shrink_iters: 200, ..ProptestConfig::default() })]

    /// When `redraw` is false, the screen (without the ink) is the same as at the last full drawing:
    /// the platform can skip the full drawing.
    #[test]
    fn no_redraw_means_no_change_on_the_screen(seed in any::<u64>(), acts in proptest::collection::vec(act(), 1..30)) {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let fonts = sudoku::font::Fonts::from_files(&root.join("scripts/fonts/rmsans-400.ttf"), &root.join("scripts/fonts/rmsans-500.ttf")).unwrap();
        let mut d = Driver::new(seed);
        d.app.ng_stops = sudoku::render::ng_stops(&fonts);
        let mut last = render_without_ink(&mut d.app, &fonts);
        d.app.redraw = false;
        for a in &acts {
            run(&mut d, a);
            d.app.quit = false;
            let now = render_without_ink(&mut d.app, &fonts);
            if !d.app.redraw {
                prop_assert!(now.data() == last.data(), "the screen changed without redraw after {:?}", a);
            }
            last = now;
            d.app.redraw = false;
        }
    }
}
