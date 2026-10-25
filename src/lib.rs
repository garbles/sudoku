//! Sudoku for the reMarkable Paper Pro. Refer to REQUIREMENTS.md and INVARIANTS.md.

pub mod app;
pub mod game;
pub mod generator;
pub mod invariants;
pub mod layout;
pub mod recognizer;
pub mod settings;
pub mod store;
pub mod font;
pub mod render;
pub mod system;
#[cfg(target_os = "linux")]
pub mod platform;
pub mod reference;
