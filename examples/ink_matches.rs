//! Prints the match of each line of an ink log (platform/mod.rs, `log-ink`):
//!     cargo run --release --example ink_matches -- ink-log.jsonl
fn main() {
    let path = std::env::args().nth(1).expect("ink log");
    for (i, line) in std::fs::read_to_string(path).unwrap().lines().enumerate() {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        let strokes: Vec<Vec<(f32, f32)>> = serde_json::from_value(v["strokes"].clone()).unwrap();
        println!("{i}: logged {} now {:?} {:?}", v["result"], sudoku::recognizer::recognize(&strokes), sudoku::recognizer::matches(&strokes));
    }
}
