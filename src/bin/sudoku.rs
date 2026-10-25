//! The app for the reMarkable Paper Pro. AppLoad starts it (refer to `appload/`).

#[cfg(target_os = "linux")]
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let result = if args.get(1).map(String::as_str) == Some("--test-results") {
        // `sudoku --test-results add|remove`: the 14 test results in the results database (store.rs).
        match args.get(2).map(String::as_str) {
            Some(arg @ ("add" | "remove")) => {
                let add = arg == "add";
                let path = sudoku::store::data_dir().join(sudoku::store::RESULTS_FILE);
                sudoku::store::ResultsDb::open(&path)
                    .and_then(|mut db| db.test_results(add))
                    .map(|n| println!("{} {n} test results", if add { "added" } else { "removed" }))
                    .map_err(|e| e.to_string())
            }
            _ => Err("usage: sudoku --test-results add|remove".to_string()),
        }
    } else if args.get(1).map(String::as_str) == Some("--self-test") {
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
