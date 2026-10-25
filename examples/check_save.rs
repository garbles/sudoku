//! Checks a saved game file with the rules of INVARIANTS.md: cargo run --example check_save -- game.json
fn main() {
    let path = std::env::args().nth(1).expect("game.json");
    let g: sudoku::game::Game = serde_json::from_slice(&std::fs::read(path).unwrap()).expect("parse");
    match sudoku::invariants::check_game(&g) {
        Ok(()) => println!("the saved game agrees with all rules"),
        Err(e) => println!("rule failed: {e}"),
    }
}
