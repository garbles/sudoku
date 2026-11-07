//! The third-party licenses (the Licenses screens). `scripts/licenses.py` makes
//! `assets/licenses.json` from `cargo metadata`: the crates in the tablet program, with the text of
//! each license file. This module reads the file, wraps the texts and divides them into pages. It
//! does not use the font, thus the app knows the quantity of pages without a font.

use std::sync::OnceLock;

use serde::Deserialize;

static JSON: &str = include_str!("../assets/licenses.json");

/// The maximum quantity of characters in a line of license text. Most license files have lines of
/// 80 characters or less. The page shows 80 characters of capital letters in its width.
pub const WRAP: usize = 80;

#[derive(Deserialize)]
pub struct Licenses {
    pub crates: Vec<Crate>,
    texts: Vec<String>,
}

#[derive(Deserialize)]
pub struct Crate {
    pub name: String,
    pub version: String,
    /// The SPDX license expression of the crate.
    pub license: String,
    pub files: Vec<LicenseFile>,
}

#[derive(Deserialize)]
pub struct LicenseFile {
    pub name: String,
    text: usize,
}

/// A line of a license page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Line {
    /// The name of a license file.
    Heading(&'static str),
    Body(&'static str),
    Blank,
}

/// The licenses of the program (parsed one time).
pub fn all() -> &'static Licenses {
    static ALL: OnceLock<Licenses> = OnceLock::new();
    ALL.get_or_init(|| serde_json::from_str(JSON).expect("assets/licenses.json"))
}

/// The quantity of crates.
pub fn count() -> usize {
    all().crates.len()
}

/// The lines of all license files of crate `index`: for each file, its name, then its text.
pub fn lines(index: usize) -> Vec<Line> {
    let licenses = all();
    let mut out = Vec::new();
    for (k, f) in licenses.crates[index].files.iter().enumerate() {
        if k > 0 {
            out.push(Line::Blank);
        }
        out.push(Line::Heading(&f.name));
        out.push(Line::Blank);
        for line in licenses.texts[f.text].split('\n') {
            if line.is_empty() {
                out.push(Line::Blank);
            } else {
                out.extend(wrap(line, WRAP).into_iter().map(Line::Body));
            }
        }
    }
    out
}

/// Divides a line into parts of at most `width` characters, at spaces if possible.
pub fn wrap(line: &str, width: usize) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut rest = line;
    while rest.chars().count() > width {
        // The byte index of the character after the limit.
        let limit = rest.char_indices().nth(width).map_or(rest.len(), |(i, _)| i);
        // A space at the limit is also a good cut: the part before it has `width` characters.
        let cut = if rest[limit..].starts_with(' ') { limit } else { rest[..limit].rfind(' ').filter(|&i| i > 0).unwrap_or(limit) };
        parts.push(rest[..cut].trim_end());
        rest = rest[cut..].trim_start();
    }
    if !rest.is_empty() || parts.is_empty() {
        parts.push(rest);
    }
    parts
}

/// The quantity of pages of `n` items with `per_page` items on each page (at least 1).
pub fn pages(n: usize, per_page: usize) -> usize {
    n.div_ceil(per_page).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_keeps_short_lines_and_cuts_long_lines_at_spaces() {
        assert_eq!(wrap("abc def", 80), vec!["abc def"]);
        assert_eq!(wrap("aaa bbb ccc", 7), vec!["aaa bbb", "ccc"]);
        assert_eq!(wrap("abcdefghij", 4), vec!["abcd", "efgh", "ij"]);
        assert!(lines(0).iter().all(|l| match l {
            Line::Body(s) => s.chars().count() <= WRAP,
            _ => true,
        }));
    }

    #[test]
    fn each_crate_has_a_license_and_a_text() {
        assert!(count() > 0);
        for (i, c) in all().crates.iter().enumerate() {
            assert!(!c.license.is_empty() && !c.files.is_empty(), "{}", c.name);
            assert!(lines(i).iter().any(|l| matches!(l, Line::Body(_))), "{}", c.name);
        }
    }
}
