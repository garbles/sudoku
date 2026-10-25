//! The app for the reMarkable Paper Pro. AppLoad starts it (refer to `appload/`).

#[cfg(target_os = "linux")]
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let result = if args.get(1).map(String::as_str) == Some("--self-test") {
        let out = args.get(2).map(String::as_str).unwrap_or("/tmp/sudoku-self-test.png");
        sudoku::platform::self_test(out, args.get(3).and_then(|s| s.parse().ok()).unwrap_or(0))
    } else {
        sudoku::platform::run()
    };
    if let Err(e) = result {
        eprintln!("sudoku: {e}");
        std::process::exit(1);
    }
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("sudoku: this app operates only on the reMarkable Paper Pro (Linux). Use the snapshot tool on a computer.");
    std::process::exit(1);
}
