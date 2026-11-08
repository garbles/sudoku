//! The format of INVARIANTS.md: the visualizer (scripts/invariants.py, `make invariants`) must be able
//! to read each rule, the state model and the terms. The format is in the comment at the start of
//! INVARIANTS.md. Dockerfile.test runs these tests on CI (it has python3).

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// Runs `scripts/invariants.py --check` on a file. Gives the result and the error output.
fn check(file: &Path) -> (bool, String) {
    let out = Command::new("python3")
        .arg("scripts/invariants.py")
        .arg("--check")
        .arg("--file")
        .arg(file)
        .current_dir(root())
        .output()
        .expect("python3 must be on PATH");
    (out.status.success(), String::from_utf8_lossy(&out.stderr).into_owned())
}

fn original() -> String {
    std::fs::read_to_string(root().join("INVARIANTS.md")).unwrap()
}

/// Writes a changed copy of INVARIANTS.md to a temporary file.
fn copy_with(name: &str, text: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("sudoku-invariants-format-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{name}.md"));
    std::fs::write(&path, text).unwrap();
    path
}

/// Replaces the first occurrence of `from`. The test fails if `from` is not in the text.
fn replace_once(text: &str, from: &str, to: &str) -> String {
    assert!(text.contains(from), "the text has no {from:?}");
    text.replacen(from, to, 1)
}

/// The line of a rule in the original file.
fn rule_line(id: &str) -> String {
    original().lines().find(|l| l.starts_with(&format!("| {id} |"))).unwrap().to_string()
}

fn assert_finds(name: &str, text: &str, message: &str) {
    let (ok, err) = check(&copy_with(name, text));
    assert!(!ok, "{name}: the check did not find the problem");
    assert!(err.contains(message), "{name}: no {message:?} in:\n{err}");
}

#[test]
fn invariants_md_has_the_format_of_the_visualizer() {
    let (ok, err) = check(&root().join("INVARIANTS.md"));
    assert!(ok, "INVARIANTS.md cannot be read by scripts/invariants.py:\n{err}");
}

#[test]
fn the_check_finds_a_rule_row_with_a_line_break() {
    let row = rule_line("BRD-1");
    let (a, b) = row.split_at(row.len() / 2);
    let broken = replace_once(&original(), &row, &format!("{a}\n{b}"));
    assert_finds("line-break", &broken, "BRD-1");
}

#[test]
fn the_check_finds_a_bar_out_of_a_code_span() {
    let row = rule_line("BRD-1");
    let broken = replace_once(&original(), &row, &row.replacen(" Each cell ", " Each | cell ", 1));
    assert_finds("bar", &broken, "has 4 cells, not 3");
}

#[test]
fn the_check_finds_a_code_span_that_is_not_closed() {
    let row = rule_line("BRD-2");
    let broken = replace_once(&original(), &row, &row.replacen("`given", "given", 1));
    assert_finds("backtick", &broken, "a code span is not closed");
}

#[test]
fn the_check_finds_two_rules_with_the_same_id() {
    let row = rule_line("BRD-1");
    let broken = replace_once(&original(), &row, &format!("{row}\n{row}"));
    assert_finds("duplicate", &broken, "the ID BRD-1 is also on line");
}

#[test]
fn the_check_finds_a_reference_to_a_rule_that_is_not_there() {
    let row = rule_line("BRD-1");
    let broken = replace_once(&original(), &row, &row.replacen("| R§4.1 |", "| R§4.1, BRD-999 |", 1));
    assert_finds("reference", &broken, "BRD-999 is not a rule");
}

#[test]
fn the_check_finds_a_missing_marker() {
    let broken = replace_once(&original(), "<!-- invariants:terms:end -->\n", "");
    assert_finds("marker", &broken, "<!-- invariants:terms:end --> must be there one time");
}

#[test]
fn the_check_finds_a_term_with_a_different_format() {
    let broken = replace_once(&original(), "- **peers(r, c)**:", "- peers(r, c):");
    assert_finds("term", &broken, "a term must be");
}

#[test]
fn the_check_finds_a_heading_with_no_number() {
    let broken = replace_once(&original(), "## 2. Cells", "## Cells");
    assert_finds("heading", &broken, "a section heading must be");
}

#[test]
fn the_check_finds_a_state_model_line_with_fields_of_different_domains() {
    let broken = replace_once(&original(), "  id, finished_at\n  mode            ∈ 0..6\n", "  id, finished_at, mode ∈ 0..6, seed\n");
    assert_finds("mixed-domains", &broken, "one line has fields with different domains");
}

#[test]
fn the_check_finds_a_state_model_line_with_a_different_indent() {
    let broken = replace_once(&original(), "  seed            : integer", "    seed          : integer");
    assert_finds("indent", &broken, "a field line must have an indent of 2 spaces");
}

#[test]
fn the_check_finds_a_state_model_field_that_is_not_in_the_grammar() {
    let broken = replace_once(&original(), "  seed            : integer", "  seed            = integer");
    assert_finds("grammar", &broken, "a field line must be");
}

#[test]
fn the_check_finds_two_fields_with_the_same_name_in_a_type() {
    let broken = replace_once(&original(), "  completed       : bool\n", "  completed       : bool\n  completed       : bool\n");
    assert_finds("same-field", &broken, "the field Game.completed is also on line");
}
