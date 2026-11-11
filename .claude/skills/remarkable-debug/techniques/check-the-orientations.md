# Examine the portrait and the landscape screens

The tablet is not necessary for a problem of the layout: the app draws the same pixels on the
computer. The design images in `images/pro/` are the reference for the portrait screens. The landscape
screens have only images for the manual and the statistics pages. Thus examine the landscape game
screen on the computer with the snapshot tool.

## Procedure

1. Run `cargo run --release --bin snapshot -- target/snapshots`. The tool makes the reference states
   in portrait, and the same states in landscape with the prefix `landscape-` (they are not
   compared with `images/pro/`).
2. Look at `target/snapshots/landscape-screen-in-progress.png` and the other landscape images. Check
   the board, the toolbar, the meter and the panels.
3. To make a screen that the snapshot tool does not make, use `reference::render` in a test: set the
   orientation with `layout::set_orientation(layout::Orientation::Landscape)` before the drawing.
4. Compare the positions with `layout.rs`: the board is at the center (portrait and landscape), the
   toolbar is at the right edge (left-handed), and the meter is below the board.
5. Make a test for the problem (refer to `reproduce-in-a-test.md`). A test of the landscape layout
   does not need the tablet.
6. To examine a turn, use the test
   `tests/render.rs::a_turn_from_portrait_to_landscape_shows_the_landscape_screen`. It starts in
   portrait, gives the turn with `App::rotate` and `Presenter::reset`, then writes a screenshot of
   the framebuffer before and after the turn to `target/screenshots/`
   (`screen-portrait.png` and `screen-landscape-after-turn.png`). Copy the test and change the app
   state to examine another screen. The screenshot shows what the player sees: the framebuffer with
   the rotation of the tablet.

## Rules

- Set the orientation back to `Orientation::Portrait` at the end of the test, or the next test uses
  the landscape layout.
- The rotation and the hand are thread-local values. A drawing on a new thread starts with the
  portrait layout of a left-handed player. If a landscape drawing shows a part of the drawing at the
  portrait position, look for a thread that draws without the values of the main thread.
