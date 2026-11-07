//! Sudoku for the reMarkable Paper Pro. Refer to REQUIREMENTS.md and INVARIANTS.md.

pub mod app;
pub mod bench;
pub mod colors;
#[cfg(feature = "corpus")]
pub mod corpus;
pub mod early;
pub mod game;
pub mod generator;
pub mod invariants;
pub mod layout;
pub mod licenses;
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
pub mod present;
pub mod reference;
pub mod script;

/// The malloc of musl (the C library of the tablet build) is slow: the drawing, the fonts and the
/// startup make many small allocations.
#[cfg(target_env = "musl")]
#[global_allocator]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

/// The version of the app (REQUIREMENTS.md §9.1). release.yml sets SUDOKU_VERSION at build time
/// (pre-<sha> or v<version>). A dev install can set dev/<name> (make install-dev DEV_NAME=<name>).
/// Other builds show "dev".
pub const VERSION: &str = match option_env!("SUDOKU_VERSION") {
    Some(v) => v,
    None => "dev",
};
