//! The corpus build (the Cargo feature `corpus`): a development build that collects examples of
//! handwriting for the recognizer. Do not use it for a release. The skill `remarkable-corpus` tells
//! the procedure.
//!
//! - The board is blank: no starting numbers, no numbers of the player, no pencil marks.
//! - The data folder is `rm-sudoku-corpus` (`store::data_dir`), thus the build never changes the
//!   game, the settings or the results of the player.
//! - Clashes and the highlight do not show, because the player writes the same number in many cells.
//! - Each reading of ink goes to `LOG_FILE` in the data folder: one JSON line with the cell, the
//!   strokes (screen pixels) and the result of the recognizer.
//!
//! `examples/corpus.rs` reads the log on the computer.

use std::io::Write;
use std::path::Path;

use crate::app::{App, Menu};
use crate::game::Game;
use crate::settings::Color;

/// The name of the data folder of the corpus build (in `~/.local/share`).
pub const DATA_DIR_NAME: &str = "rm-sudoku-corpus";

/// The log of the ink, in the data folder.
pub const LOG_FILE: &str = "corpus-ink.jsonl";

/// Makes the board blank, closes the menu, and sends each reading of ink to the corpus log.
pub fn setup(app: &mut App, data_dir: &Path) {
    let g = &app.game;
    app.game = Game::new(g.seed, g.mode, g.solution, [[0; 9]; 9]);
    app.game_dirty = true;
    app.ui.menu = Menu::Closed;
    app.ui.highlight = None;
    app.settings.highlight_on = false;
    app.settings.color_clash = Color::White;
    app.settings_dirty = true;
    let log_path = data_dir.join(LOG_FILE);
    app.ink_log = Some(Box::new(move |cell, strokes, result| {
        let line = serde_json::json!({ "cell": cell, "strokes": strokes, "result": result });
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
