//! The corpus build (the Cargo feature `corpus`): a development build that collects examples of
//! handwriting for the recognizer. Do not use it for a release. The skill `remarkable-corpus` tells
//! the procedure.
//!
//! - It is a separate app on the tablet, `Sudoku (corpus)` in the AppLoad folder `sudoku-corpus`
//!   (`make install-corpus`). Thus its installation does not change the usual Sudoku app.
//! - The board is blank: no starting numbers, no numbers of the player, no pencil marks.
//! - The data folder is `rm-sudoku-corpus` (`store::data_dir`), thus the build never changes the
//!   game, the settings or the results of the player.
//! - Clashes, the highlight and the completed color do not show, because the player writes the same
//!   number in many cells, or nine different numbers in a group.
//! - Each pen stroke and each erasure on the board goes to `LOG_FILE` in the data folder: one JSON
//!   line `{"cell": [r, c], "stroke": [[x, y], ...]}` (screen pixels) or `{"cell": [r, c], "erase":
//!   true}`. The log has all strokes, also a stroke that the app discards on a filled cell. Thus
//!   the computer keeps all strokes of a cell as one example (a 4 or an 8 with two strokes), also
//!   when the app reads the ink between the two strokes.
//!
//! `examples/corpus.rs` reads the log on the computer.

use std::io::Write;
use std::path::Path;

use crate::app::{App, BoardInput, Menu};
use crate::game::Game;
use crate::settings::Color;

/// The name of the data folder of the corpus build (in `~/.local/share`).
pub const DATA_DIR_NAME: &str = "rm-sudoku-corpus";

/// The log of the ink, in the data folder.
pub const LOG_FILE: &str = "corpus-ink.jsonl";

/// Makes the board blank, closes the menu, and sends each stroke and erasure to the corpus log.
pub fn setup(app: &mut App, data_dir: &Path) {
    let g = &app.game;
    app.game = Game::new(g.seed, g.mode, g.solution, [[0; 9]; 9]);
    app.game_dirty = true;
    app.ui.menu = Menu::Closed;
    app.ui.highlight = None;
    app.settings.highlight_on = false;
    app.settings.color_clash = Color::White;
    app.settings.completed_color_on = false;
    app.settings_dirty = true;
    let log_path = data_dir.join(LOG_FILE);
    app.stroke_log = Some(Box::new(move |input| {
        let line = match input {
            BoardInput::Stroke(cell, stroke) => serde_json::json!({ "cell": cell, "stroke": stroke }),
            BoardInput::Erase(cell) => serde_json::json!({ "cell": cell, "erase": true }),
        };
        match std::fs::OpenOptions::new().create(true).append(true).open(&log_path) {
            Ok(mut f) => {
                if let Err(e) = writeln!(f, "{line}") {
                    eprintln!("sudoku: cannot write the corpus log: {e}");
                }
            }
            Err(e) => eprintln!("sudoku: cannot open the corpus log: {e}"),
        }
    }));
}
