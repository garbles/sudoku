//! The installer (install.sh, R§14.6) in the simulation of tests/install/run.sh. Fake
//! ssh, ssh-keygen and curl programs take the place of the tablet and the network. Each test runs one
//! scenario and examines the terminal output. bash must be available (on macOS, bash 3.2).

use std::process::Command;

/// The output of the scenario (without colors) and the exit code of install.sh.
fn run(scenario: &str) -> (String, i32) {
    let out = Command::new("bash")
        .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/install/run.sh"))
        .arg(scenario)
        .output()
        .expect("bash runs");
    assert!(out.stderr.is_empty(), "{scenario}: stderr: {}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8(out.stdout).expect("UTF-8 output");
    (strip_ansi(&text), out.status.code().unwrap_or(-1))
}

fn strip_ansi(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            for c in chars.by_ref() {
                if c.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// The lines of the step "n/9 ..." up to the next step.
fn step(out: &str, n: u32) -> String {
    let start = out.find(&format!(" {n}/9 ")).unwrap_or_else(|| panic!("no step {n}:\n{out}"));
    let rest = &out[start + 1..];
    let end = rest.find(&format!(" {}/9 ", n + 1)).unwrap_or(rest.len());
    rest[..end].to_string()
}

fn assert_has(out: &str, text: &str) {
    assert!(out.contains(text), "the output does not contain {text:?}:\n{out}");
}

/// All 9 steps show "(done already)" and none does an action.
fn assert_all_done(out: &str) {
    for n in 2..=9 {
        let s = step(out, n);
        if n == 2 || n == 3 {
            continue; // the download and the connection always operate
        }
        assert!(s.contains("(done already)"), "step {n} is not skipped:\n{s}");
        assert!(!s.contains('→'), "step {n} does an action:\n{s}");
    }
    assert_has(out, "Sudoku is ready");
}

#[test]
fn first_installation_does_all_steps() {
    let (out, code) = run("first");
    assert_eq!(code, 0, "{out}");
    assert_has(&step(&out, 4), "Made the SSH key ~/.ssh/id_ed25519_remarkable.");
    assert_has(&step(&out, 4), "Copyrights and licenses");
    assert_has(&step(&out, 4), "The tablet accepts the SSH key. You do not need the password again.");
    assert_has(&step(&out, 4), "Tablet: reMarkable Paper Pro, software 3.29.0.153.");
    assert_has(&step(&out, 5), "Vellum is installed.");
    assert!(!out.contains("exec bash --login"), "the Vellum hint must not show:\n{out}");
    assert_has(&step(&out, 6), "AppLoad is installed.");
    assert_has(&step(&out, 7), "Before you continue");
    assert_has(&step(&out, 7), "The hashtable is available.");
    // The hashtable step warned already. Step 8 does not ask again.
    assert!(!step(&out, 8).contains("Before you continue"), "{out}");
    assert_has(&step(&out, 8), "xovi is on.");
    assert_has(&step(&out, 9), "Sudoku v0.1.2 is installed.");
    assert!(!out.contains("(done already)"), "{out}");
}

#[test]
fn second_run_skips_all_steps() {
    let (out, code) = run("twice");
    assert_eq!(code, 0, "{out}");
    assert_all_done(&out);
    assert!(!out.contains("password:"), "the second run must not ask for the password:\n{out}");
}

#[test]
fn complete_installation_skips_all_steps() {
    let (out, code) = run("again");
    assert_eq!(code, 0, "{out}");
    assert_all_done(&out);
}

#[test]
fn connection_help_for_usb_and_developer_mode() {
    let (out, code) = run("connect");
    assert_eq!(code, 0, "{out}");
    let s = step(&out, 3);
    assert_has(&s, "No answer from the tablet at 10.11.99.1.");
    assert_has(&s, "Connect the tablet to this computer with a USB-C cable.");
    assert_has(&s, "The tablet is connected, but SSH is off.");
    assert_has(&s, "CAUTION: Developer mode erases all data on the tablet.");
    assert_has(&s, "Found the tablet at 10.11.99.1.");
}

#[test]
fn restart_starts_xovi_and_replaces_an_old_version() {
    let (out, code) = run("restart");
    assert_eq!(code, 0, "{out}");
    assert_has(&step(&out, 7), "(done already)");
    assert_has(&step(&out, 8), "Before you continue");
    assert_has(&step(&out, 8), "xovi is on.");
    assert_has(&step(&out, 9), "A different version of Sudoku is on the tablet.");
    assert_has(&step(&out, 9), "Your games, settings and statistics stay on the tablet.");
    assert_has(&step(&out, 9), "Sudoku v0.1.2 is installed.");
}

#[test]
fn incorrect_password_gives_help_and_a_new_try() {
    let (out, code) = run("password");
    assert_eq!(code, 0, "{out}");
    let s = step(&out, 4);
    assert_has(&s, "The tablet did not accept the password.");
    assert_has(&s, "Upper and lower case letters are different.");
    assert_has(&s, "The tablet accepts the SSH key.");
}

#[test]
fn changed_host_key_is_removed_after_a_confirmation() {
    let (out, code) = run("hostkey");
    assert_eq!(code, 0, "{out}");
    assert_has(&step(&out, 3), "The tablet has a new SSH identity (host key).");
    assert_has(&step(&out, 3), "Removed the old identity.");
}

#[test]
fn tablet_without_internet_gets_wifi_help() {
    let (out, code) = run("offline");
    assert_eq!(code, 0, "{out}");
    assert_has(&step(&out, 5), "The tablet is not connected to the internet.");
    assert_has(&step(&out, 5), "Settings > Wi-Fi");
    assert_has(&step(&out, 5), "Vellum is installed.");
}

#[test]
fn failed_download_tries_again() {
    let (out, code) = run("download");
    assert_eq!(code, 0, "{out}");
    assert_has(&step(&out, 2), "The download failed: curl: (6) Could not resolve host: github.com");
    assert_has(&step(&out, 2), "The checksum is correct.");
}

#[test]
fn q_stops_the_installer() {
    let (out, code) = run("stop");
    assert_eq!(code, 1, "{out}");
    assert_has(&out, "The installer stopped. Run the same command again to continue.");
    assert!(!out.contains(" 4/9 "), "{out}");
}

#[test]
fn other_tablet_needs_a_confirmation() {
    let (out, code) = run("other");
    assert_eq!(code, 1, "{out}");
    assert_has(&out, "This tablet is not a reMarkable Paper Pro");
    assert!(!out.contains(" 5/9 "), "{out}");
}

/// All boxes have the same width, and no line is wider than the 90 columns of the screenshots.
#[test]
fn output_fits_the_terminal() {
    for scenario in ["first", "connect", "password", "hostkey", "offline", "download", "stop", "other"] {
        let (out, _) = run(scenario);
        for line in out.lines() {
            let width = line.chars().count();
            assert!(width <= 90, "{scenario}: {width} columns: {line}");
            if line.starts_with("  │") || line.starts_with("  ╭") || line.starts_with("  ╰") {
                assert_eq!(width, 72, "{scenario}: box line: {line}");
            }
        }
    }
}

/// release.yml replaces the two values with sed. Each must be in install.sh exactly one time.
#[test]
fn release_placeholders() {
    let script = include_str!("../install.sh");
    for p in ["@SUDOKU_VERSION@", "@SUDOKU_SHA256@"] {
        assert_eq!(script.matches(p).count(), 1, "{p}");
    }
    let status = Command::new("bash").arg("-n").arg(concat!(env!("CARGO_MANIFEST_DIR"), "/install.sh")).status().unwrap();
    assert!(status.success());
}
