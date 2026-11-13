# Reproduce a screen problem in a test

The tests on the computer cannot show the panel. But `present.rs` has no input or output, thus a
test can run the real app and the real presenter, and examine each update that the app sends. When
the update log of the tablet shows the wrong updates, make a test that gets the same updates.

## Two levels

- **The scheduler only** (`present::Scheduler`): give it changed rectangles and times, and examine
  what `send_ink` and `send_now` send. Fast and exact, for a rule of the scheduler. Examples:
  `ink_in_the_pen_region_does_not_wait_for_a_redraw`, and the random test
  `ink_never_touches_cells_in_progress_and_all_ink_is_sent`.
- **The app and the presenter**: make an `App` with a fixed seed, a `Presenter`, a framebuffer and
  the fonts of `scripts/fonts/`. Give pen events with times (`app.handle`, `app.tick`), call
  `present` after each event, and record the updates with their times. Example:
  `a_redraw_waits_while_the_player_writes_in_many_cells` in `src/present.rs`.

## Rules

- Use the same steps as the log of the tablet: for example a stroke in one cell, then at once a
  stroke in the next cell, with one point each 10 ms.
- Examine what the log showed was wrong, as an assertion on each update: for example "all the
  updates while the pen writes are inside the pen area".
- **Make sure that the test fails before the fix.** Disable the fix for a moment (for example with
  `.filter(|_| false)` or `false &&`), run the test, and look at the failure message. It must show
  the same problem as the log of the tablet (for example the same wide update). Then put the fix
  back.
- At the end, examine that the framebuffer shows the frame: no change is lost.
- `layout::physical_rect` gives (x, y, width, height), not two corners.
