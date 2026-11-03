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

/// The number of steps of install.sh (STEPS=11, INS-1).
const STEPS: u32 = 11;

/// The lines of the step "n/11 ..." up to the next step.
fn step(out: &str, n: u32) -> String {
    let start = out.find(&format!(" {n}/{STEPS} ")).unwrap_or_else(|| panic!("no step {n}:\n{out}"));
    let rest = &out[start + 1..];
    let end = rest.find(&format!(" {}/{STEPS} ", n + 1)).unwrap_or(rest.len());
    rest[..end].to_string()
}

fn assert_has(out: &str, text: &str) {
    assert!(out.contains(text), "the output does not contain {text:?}:\n{out}");
}

/// All 11 steps show "(done already)" and none does an action.
fn assert_all_done(out: &str) {
    for n in 2..=STEPS {
        let s = step(out, n);
        if n == 2 || n == 3 {
            continue; // the download and the connection always operate
        }
        assert!(s.contains("(done already)"), "step {n} is not skipped:\n{s}");
        assert!(!s.contains('→'), "step {n} does an action:\n{s}");
    }
    assert_has(out, "Sudoku is ready");
}

/// INS-2: step 10 is done. It shows "(done already)" and it changes nothing.
fn assert_starter_done(out: &str) {
    let s = step(out, 10);
    assert_has(&s, "Power button starter");
    assert_has(&s, "The power button starts xovi. (done already)");
    assert!(!s.contains("Installing the power-button starter"), "step 10 installs:\n{s}");
    assert!(!s.contains("Repairing the power-button starter"), "step 10 repairs:\n{s}");
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
    // INS-7: without a tablet software change, step 7 shows the old information line.
    assert_has(&step(&out, 7), "xovi needs a table of the user interface files of the tablet.");
    assert_has(&step(&out, 8), "Copying the ink patch to the tablet...");
    assert_has(&step(&out, 8), "The ink patch is installed.");
    // The hashtable step warned already. Step 9 does not ask again.
    assert!(!step(&out, 9).contains("Before you continue"), "{out}");
    assert_has(&step(&out, 9), "xovi is on.");
    // INS-1, INS-3, INS-4: the power-button starter is step 10, before Sudoku (step 11).
    let s = step(&out, 10);
    assert_has(&s, "Power button starter");
    assert_has(&s, "After a restart, xovi is off, and AppLoad is not in the menu.");
    assert_has(&s, "A triple press of the power button starts xovi again.");
    assert_has(&s, "Installing the power-button starter...");
    assert_has(&s, "Installing xovi-tripletap");
    assert!(!s.contains("Repairing the power-button starter"), "the service was on:\n{s}");
    assert_has(&s, "The power button starts xovi.");
    // INS-1: Sudoku is step 11, after the power-button starter.
    assert_has(&step(&out, 11), "Sudoku v0.1.2 is installed.");
    // INS-5: the final box gives the restart lines.
    assert_has(&out, "After a restart, xovi is off. AppLoad is not in the menu.");
    assert_has(&out, "Press the power button 3 times to start xovi. The screen");
    assert_has(&out, "starts again after some seconds.");
    assert!(!out.contains("(done already)"), "{out}");
}

#[test]
fn second_run_skips_all_steps() {
    let (out, code) = run("twice");
    assert_eq!(code, 0, "{out}");
    assert_all_done(&out);
    // INS-2: the starter is on, and the step changes nothing.
    assert_starter_done(&out);
    // INS-5: every run that reaches step 11 shows the final box.
    assert_has(&out, "Press the power button 3 times to start xovi. The screen");
    assert!(!out.contains("password:"), "the second run must not ask for the password:\n{out}");
}

#[test]
fn complete_installation_skips_all_steps() {
    let (out, code) = run("again");
    assert_eq!(code, 0, "{out}");
    assert_all_done(&out);
    // INS-2: the starter is on, and the step changes nothing.
    assert_starter_done(&out);
    // INS-5: the final box gives the restart lines.
    assert_has(&out, "After a restart, xovi is off. AppLoad is not in the menu.");
    assert_has(&out, "Press the power button 3 times to start xovi. The screen");
    assert_has(&out, "starts again after some seconds.");
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
    assert_has(&step(&out, 8), "(done already)");
    assert_has(&step(&out, 9), "Before you continue");
    assert_has(&step(&out, 9), "xovi is on.");
    // INS-2: a restart stops xovi, but the power-button starter stays on: step 10 is done.
    assert_starter_done(&out);
    assert_has(&step(&out, 11), "A different version of Sudoku is on the tablet.");
    assert_has(&step(&out, 11), "Your games, settings and statistics stay on the tablet.");
    assert_has(&step(&out, 11), "Sudoku v0.1.2 is installed.");
}

/// A new ink patch: the installer copies it, and xovi starts again (one time) to apply it.
#[test]
fn new_ink_patch_starts_xovi_again() {
    let (out, code) = run("patch");
    assert_eq!(code, 0, "{out}");
    assert_has(&step(&out, 8), "Copying the ink patch to the tablet...");
    assert_has(&step(&out, 8), "The ink patch is installed.");
    assert_has(&step(&out, 9), "xovi must start again to apply the ink patch.");
    assert_has(&step(&out, 9), "Before you continue");
    assert_has(&step(&out, 9), "xovi is on.");
    assert_starter_done(&out);
    assert_has(&step(&out, 11), "(done already)");
}

/// The ink patch is installed, but an earlier installer did not write the stamp file. The installer
/// writes the stamp file. The patch did not change, thus xovi does not start again.
#[test]
fn missing_stamp_file_is_written_without_a_restart() {
    let (out, code) = run("stamp");
    assert_eq!(code, 0, "{out}");
    assert_has(&step(&out, 8), "Writing the stamp file of the ink patch...");
    assert_has(&step(&out, 8), "The ink patch is installed.");
    assert!(!step(&out, 8).contains("Copying the ink patch"), "{out}");
    assert_has(&step(&out, 9), "xovi is on. (done already)");
    assert_starter_done(&out);
    assert_has(&step(&out, 11), "(done already)");
}

/// A patch that does not agree with the checksum of the release stops the installer before it
/// connects to the tablet.
#[test]
fn bad_patch_checksum_stops_the_installer() {
    let (out, code) = run("badpatch");
    assert_ne!(code, 0, "{out}");
    assert_has(&step(&out, 2), "Downloading the ink patch...");
    assert_has(&step(&out, 2), "The checksum of the download is not correct.");
    assert!(!out.contains("3/11"), "{out}");
}

/// REMARKABLE_HOST gives a Wi-Fi address. The first connection fails, and the user tries again on
/// Wi-Fi.
#[test]
fn wifi_address_is_used_and_can_be_tried_again() {
    let (out, code) = run("wifi");
    assert_eq!(code, 0, "{out}");
    let s = step(&out, 3);
    assert_has(&s, "REMARKABLE_HOST is set: the installer connects to 192.168.1.20.");
    assert_has(&s, "No answer from the tablet at 192.168.1.20.");
    assert_has(&s, "Connect the tablet on Wi-Fi");
    assert_has(&s, "Try again on Wi-Fi.");
    assert_has(&s, "Found the tablet at 192.168.1.20.");
    assert!(!out.contains("Looking for the tablet at 10.11.99.1"), "{out}");
}

/// Wi-Fi does not operate. The user selects the USB cable, and the installer continues on USB.
#[test]
fn wifi_failure_can_continue_with_the_usb_cable() {
    let (out, code) = run("wifi-usb");
    assert_eq!(code, 0, "{out}");
    let s = step(&out, 3);
    assert_has(&s, "No answer from the tablet at 192.168.1.20.");
    assert_has(&s, "The installer uses the USB cable.");
    assert_has(&s, "Found the tablet at 10.11.99.1.");
}

/// Without REMARKABLE_HOST, the installer tells that it uses the USB cable.
#[test]
fn usb_is_the_default() {
    let (out, _) = run("again");
    assert_has(&step(&out, 3), "REMARKABLE_HOST is not set: the installer uses the USB cable.");
    assert_has(&step(&out, 3), "Found the tablet at 10.11.99.1.");
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

/// INS-3: the service is off after Vellum installs the starter. The repair runs the starter's
/// enable.sh and starts the service. Then Sudoku (step 11) is installed.
#[test]
fn power_button_starter_is_repaired() {
    let (out, code) = run("tripletap-repair");
    assert_eq!(code, 0, "{out}");
    let s = step(&out, 10);
    assert_has(&s, "Installing the power-button starter...");
    assert_has(&s, "Repairing the power-button starter...");
    assert_has(&s, "xovi-tripletap: enabled and started with enable.sh.");
    assert_has(&s, "The power button starts xovi.");
    assert!(
        s.find("Repairing the power-button starter...").unwrap() < s.find("The power button starts xovi.").unwrap(),
        "{s}"
    );
    assert_has(&step(&out, 11), "Sudoku v0.1.2 is installed.");
    // INS-5: the final box comes after Sudoku (step 11).
    assert_has(&out, "Press the power button 3 times to start xovi. The screen");
}

/// INS-3: Vellum cannot install the power-button starter. The installer stops before Sudoku, and it
/// does not show the final box (INS-5).
#[test]
fn vellum_failure_stops_before_sudoku() {
    let (out, code) = run("tripletap-install-fail");
    assert_eq!(code, 1, "{out}");
    let s = step(&out, 10);
    assert_has(&s, "Installing the power-button starter...");
    assert_has(&s, "Vellum did not install the power-button starter.");
    assert!(!s.contains("Repairing the power-button starter"), "{s}");
    assert!(!out.contains(" 11/11 "), "{out}");
    assert!(!out.contains("Sudoku is ready"), "{out}");
    assert!(!out.contains("Press the power button 3 times"), "{out}");
}

/// INS-3: the repair does not start the service. The installer stops with "The power-button starter
/// is not on." before Sudoku.
#[test]
fn power_button_starter_not_on_stops_the_installer() {
    let (out, code) = run("tripletap-repair-fail");
    assert_eq!(code, 1, "{out}");
    let s = step(&out, 10);
    assert_has(&s, "Repairing the power-button starter...");
    assert_has(&s, "The power-button starter is not on.");
    assert!(!out.contains(" 11/11 "), "{out}");
    assert!(!out.contains("Sudoku is ready"), "{out}");
    assert!(!out.contains("Press the power button 3 times"), "{out}");
}

/// INS-3: the tablet has no internet when step 10 starts. The installer shows the Wi-Fi box and the
/// pause before it installs the starter.
#[test]
fn power_button_starter_needs_internet() {
    let (out, code) = run("tripletap-offline");
    assert_eq!(code, 0, "{out}");
    let s = step(&out, 10);
    assert_has(&s, "The tablet is not connected to the internet.");
    assert_has(&s, "Settings > Wi-Fi");
    assert_has(&s, "Installing the power-button starter...");
    assert_has(&s, "The power button starts xovi.");
    assert_has(&step(&out, 11), "Sudoku v0.1.2 is installed.");
}

/// INS-7: the tablet software was updated. Step 7 syncs Vellum before it rebuilds the hashtable
/// (the hashtab file exists), step 9 restarts xovi, and steps 10 and 11 are done.
#[test]
fn tablet_update_syncs_vellum_and_restarts_xovi() {
    let (out, code) = run("tablet-update");
    assert_eq!(code, 0, "{out}");
    let s = step(&out, 7);
    assert_has(&s, "The tablet software changed. The installer updates Vellum and the xovi files.");
    assert!(!s.contains("xovi needs a table of the user interface files of the tablet."), "{s}");
    assert_has(&s, "Before you continue");
    assert_has(&s, "Updating Vellum after the tablet software change...");
    assert_has(&s, "Re-enabling packages after OS upgrade...");
    assert_has(&s, "  tripletap");
    assert_has(&s, "Done.");
    // INS-7: the sync is before the hashtable rebuild, and the rebuild is forced.
    assert!(
        s.find("Updating Vellum after the tablet software change...").unwrap()
            < s.find("Making the hashtable. Wait 1 to 2 minutes...").unwrap(),
        "{s}"
    );
    assert!(!s.contains("(done already)"), "step 7 is skipped:\n{s}");
    assert_has(&s, "The hashtable is available.");
    // INS-7: step 9 shows the restart information line and starts xovi.
    let x = step(&out, 9);
    assert_has(&x, "xovi must start again after the tablet software change.");
    assert_has(&x, "xovi is on.");
    assert!(!x.contains("(done already)"), "step 9 is skipped:\n{x}");
    // INS-7: steps 10 and 11 are done, and the final box shows.
    assert_starter_done(&out);
    assert_has(&step(&out, 11), "Sudoku v0.1.2 is installed. (done already)");
    assert_has(&out, "Press the power button 3 times to start xovi. The screen");
}

/// INS-8: a sync failure and a still-stale Vellum state stop with "Vellum could not update the
/// packages after the tablet software change." before the hashtable rebuild and before step 9.
#[test]
fn vellum_sync_failure_stops_at_step_7() {
    for scenario in ["tablet-update-sync-fail", "tablet-update-still-stale"] {
        let (out, code) = run(scenario);
        assert_eq!(code, 1, "{scenario}:\n{out}");
        let s = step(&out, 7);
        assert_has(&s, "Updating Vellum after the tablet software change...");
        assert_has(&s, "Vellum could not update the packages after the tablet software change.");
        assert!(!out.contains("Making the hashtable"), "{scenario}:\n{out}");
        assert!(!out.contains(" 9/11 "), "{scenario}:\n{out}");
        assert!(!out.contains("Sudoku is ready"), "{scenario}:\n{out}");
        assert!(!out.contains("Press the power button 3 times"), "{scenario}:\n{out}");
    }
}

/// INS-1: the header of install.sh lists 11 steps, and the power-button starter is step 10.
#[test]
fn header_lists_the_steps() {
    let script = include_str!("../install.sh");
    assert!(script.contains("\nSTEPS=11\n"), "STEPS is not 11");
    assert!(script.contains("\n#  10. Install the power-button starter."), "step 10 is not in the header");
    assert!(script.contains("\n#  11. Install the Sudoku folder in AppLoad."), "step 11 is not in the header");
}

#[test]
fn failed_download_tries_again() {
    let (out, code) = run("download");
    assert_eq!(code, 0, "{out}");
    assert_has(&step(&out, 2), "The download failed: curl: (6) Could not resolve host: github.com");
    assert_has(&step(&out, 2), "The checksums are correct.");
}

#[test]
fn q_stops_the_installer() {
    let (out, code) = run("stop");
    assert_eq!(code, 1, "{out}");
    assert_has(&out, "The installer stopped. Run the same command again to continue.");
    assert!(!out.contains(" 4/11 "), "{out}");
}

#[test]
fn other_tablet_needs_a_confirmation() {
    let (out, code) = run("other");
    assert_eq!(code, 1, "{out}");
    assert_has(&out, "This tablet is not a reMarkable Paper Pro");
    assert!(!out.contains(" 5/11 "), "{out}");
}

#[test]
fn selected_version_replaces_the_version_of_the_installer() {
    let (out, code) = run("preview");
    assert_eq!(code, 0, "{out}");
    assert_has(&out, "Installer · preview pre-1a2b3c4");
    assert_has(&step(&out, 2), "pre-1a2b3c4 is a preview version.");
    assert_has(&step(&out, 2), "Downloaded Sudoku pre-1a2b3c4 and the ink patch. The checksums are correct.");
    assert_has(&step(&out, 11), "Sudoku pre-1a2b3c4 is installed.");
    assert!(!out.contains("Which version"), "SUDOKU_VERSION: no question:\n{out}");
}

#[test]
fn latest_version_is_the_default() {
    let (out, code) = run("again");
    assert_eq!(code, 0, "{out}");
    assert_has(&step(&out, 2), "Which version of Sudoku do you want to install?");
    assert_has(&step(&out, 2), "Version: the latest version (v0.1.2).");
}

#[test]
fn list_of_versions_selects_a_preview_version() {
    let (out, code) = run("select");
    assert_eq!(code, 0, "{out}");
    let s = step(&out, 2);
    // The newest first. The first stable release is the latest. The notes do not give a row.
    let rows: Vec<&str> = s.lines().filter(|l| l.contains(" 2026-")).collect();
    assert_eq!(rows.len(), 4, "{s}");
    for (row, want) in rows.iter().zip([
        "1. pre-9f8e7d6  preview  2026-11-02 09:12",
        "2. v0.1.2       latest   2026-11-01 06:30",
        "3. pre-1a2b3c4  preview  2026-11-01 05:35",
        "4. v0.1.1       stable   2026-10-28 05:14",
    ]) {
        assert!(row.ends_with(want), "{row:?} is not {want:?}");
    }
    assert_eq!(s.matches("Type a number from 1 to 4.").count(), 2, "{s}");
    assert_has(&s, "Version: pre-1a2b3c4.");
    assert_has(&s, "Downloaded Sudoku pre-1a2b3c4 and the ink patch. The checksums are correct.");
    assert_has(&step(&out, 11), "Sudoku pre-1a2b3c4 is installed.");
}

#[test]
fn no_list_of_versions_lets_the_user_type_the_version() {
    let (out, code) = run("typed");
    assert_eq!(code, 0, "{out}");
    assert_has(&step(&out, 2), "GitHub did not give the list of versions.");
    assert_has(&step(&out, 11), "Sudoku pre-1a2b3c4 is installed.");
}

#[test]
fn unknown_version_stops_the_installer() {
    let (out, code) = run("missing");
    assert_eq!(code, 1, "{out}");
    assert_has(&step(&out, 2), "Sudoku v9.9.9 is not available.");
    assert!(!out.contains(" 3/11 "), "{out}");
}

/// The menus in a terminal (tests/install/tty.py pushes the keys). It needs python3.
#[test]
fn arrow_keys_select_a_version_in_a_terminal() {
    let Ok(out) = Command::new("python3")
        .args(["-I", concat!(env!("CARGO_MANIFEST_DIR"), "/tests/install/tty.py")])
        .args(["select", "down", "enter", "down", "down", "enter"])
        .output()
    else {
        eprintln!("python3 is not available: the test does not run");
        return;
    };
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = strip_ansi(&String::from_utf8(out.stdout).expect("UTF-8 output"));
    assert_has(&text, "↑↓ move · Enter select · q stop");
    // No echo of the keys.
    assert!(!text.contains("^["), "{text}");
    assert_has(&text, "Version: pre-1a2b3c4.");
    assert_has(&text, "Sudoku pre-1a2b3c4 is installed.");
}

/// All boxes have the same width, and no line is wider than the 90 columns of the screenshots.
/// INS-5: the three restart lines of the final box fit in the box (72 columns with the two leading
/// spaces). INS-7 and INS-8: the new lines of a tablet software change fit also.
#[test]
fn output_fits_the_terminal() {
    for scenario in [
        "first", "connect", "password", "hostkey", "offline", "download", "stop", "other", "patch", "stamp",
        "missing", "select", "typed", "wifi", "wifi-usb", "restart", "again", "tripletap-repair",
        "tripletap-repair-fail", "tripletap-install-fail", "tripletap-offline", "tablet-update",
        "tablet-update-sync-fail", "tablet-update-still-stale",
    ] {
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

/// INS-5: the final box gives the three approved restart lines, each in a 72-column box line.
#[test]
fn final_box_gives_the_restart_lines() {
    let (out, code) = run("first");
    assert_eq!(code, 0, "{out}");
    for text in [
        "After a restart, xovi is off. AppLoad is not in the menu.",
        "Press the power button 3 times to start xovi. The screen",
        "starts again after some seconds.",
    ] {
        let line = out.lines().find(|l| l.contains(text)).unwrap_or_else(|| panic!("no line {text:?}:\n{out}"));
        assert_eq!(line.chars().count(), 72, "{line:?}");
    }
}

/// release.yml replaces the three values with sed. Each must be in install.sh exactly one time.
#[test]
fn release_placeholders() {
    let script = include_str!("../install.sh");
    for p in ["@SUDOKU_VERSION@", "@SUDOKU_SHA256@", "@SUDOKU_PATCH_SHA256@"] {
        assert_eq!(script.matches(p).count(), 1, "{p}");
    }
    let status = Command::new("bash").arg("-n").arg(concat!(env!("CARGO_MANIFEST_DIR"), "/install.sh")).status().unwrap();
    assert!(status.success());
}
