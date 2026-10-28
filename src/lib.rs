//! Sudoku for the reMarkable Paper Pro. Refer to REQUIREMENTS.md and INVARIANTS.md.

pub mod app;
pub mod game;
pub mod generator;
pub mod invariants;
pub mod layout;
pub mod recognizer;
pub mod settings;
pub mod solver;
pub mod store;
pub mod font;
pub mod render;
pub mod system;
pub mod update;
#[cfg(target_os = "linux")]
pub mod platform;
pub mod reference;

/// The version of the app (REQUIREMENTS.md §9.1). release.yml sets SUDOKU_VERSION at build time
/// (pre-<sha> or v<version>). Other builds show "dev".
pub const VERSION: &str = match option_env!("SUDOKU_VERSION") {
    Some(v) => v,
    None => "dev",
};
