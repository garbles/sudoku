//! The color picker app for the reMarkable Paper Pro, for development only. AppLoad starts it
//! (`make install-colors`). Refer to `sudoku::colors`.

#[cfg(target_os = "linux")]
fn main() {
    if let Err(e) = sudoku::platform::colors::run() {
        eprintln!("sudoku-colors: {e}");
        std::process::exit(1);
    }
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("sudoku-colors runs only on the tablet.");
}
