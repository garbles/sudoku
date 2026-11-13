//! Updates (REQUIREMENTS.md §9.2, INVARIANTS.md §16): the version of the app, the signed
//! `version.json` of the latest release, the update state and the installation of a release archive.
//!
//! This module does not use the network. The platform gets the files from GitHub and gives them to
//! `read_manifest` and `install` (platform/mod.rs). `Update` keeps the state for the app (app.rs).
//! The app installs no ink patch (owner decision 2026-11-12): in the frontend mode it deletes the
//! ink patch of an earlier release (`remove_ink_patch`, UPD-11).

use std::io::Read;
use std::path::{Component, Path, PathBuf};

use ed25519_dalek::{Signature, VerifyingKey};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::launch::{Launch, FRONTEND_FILES};

/// `version.json` of the latest stable release.
pub const VERSION_URL: &str = "https://github.com/garbles/sudoku/releases/latest/download/version.json";
/// The ed25519 signature of `version.json` (64 bytes).
pub const SIGNATURE_URL: &str = "https://github.com/garbles/sudoku/releases/latest/download/version.json.sig";

/// The public key for the signature of `version.json`. release.yml signs with the private key in the
/// secret SUDOKU_SIGNING_KEY (UPD-4).
pub const PUBLIC_KEY: [u8; 32] = [
    0xb0, 0xc7, 0x6e, 0x57, 0xdc, 0x3e, 0x95, 0x44, 0x5b, 0xe4, 0xdb, 0x0f, 0x3e, 0x2c, 0x8e, 0xd1, 0x89, 0x8e, 0xb1, 0x86,
    0xc3, 0x15, 0x24, 0xd9, 0xfc, 0x84, 0x11, 0x4e, 0xc7, 0x02, 0xb4, 0xd9,
];

/// The time that "Update failed" shows (UPD-6).
const FAILED_MS: u64 = 3000;
/// The largest archive that the app downloads.
pub const MAX_ARCHIVE_BYTES: u64 = 64 << 20;

/// The folder in the release archive. It is also the name of the AppLoad folder of the app.
const ARCHIVE_DIR: &str = "sudoku";
/// The program in the folder of the archive.
const PROGRAM: &str = "sudoku";
/// The files that the folder in the archive must have (UPD-12). The update of a release before the
/// frontend also refuses an archive without them.
const REQUIRED_FILES: [&str; 2] = [PROGRAM, "external.manifest.json"];
/// The manifest of an app with a QML frontend. An archive that has it must have all frontend files.
const FRONTEND_MANIFEST: &str = FRONTEND_FILES[0];
/// AppLoad starts this file. It starts the program (`appload/sudoku/backend/entry`).
const BACKEND_ENTRY: &str = FRONTEND_FILES[2];

/// The file name of the release archive in `update/`.
const ARCHIVE_NAME: &str = "sudoku.tar.gz";

/// A stable version: `v<major>.<minor>.<patch>`. The order of the fields is the order of the versions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version(pub u32, pub u32, pub u32);

/// The stable version in a text, or `None` for `pre-<sha>`, `dev` and all other texts.
fn parse_version(s: &str) -> Option<Version> {
    let mut parts = s.strip_prefix('v')?.split('.');
    let mut next = || -> Option<u32> {
        let p = parts.next()?;
        (!p.is_empty() && p.bytes().all(|b| b.is_ascii_digit())).then(|| p.parse().ok())?
    };
    let v = Version(next()?, next()?, next()?);
    parts.next().is_none().then_some(v)
}

/// True if `s` has only lowercase hexadecimal digits (the format of git commits and of `sha256_hex`).
fn is_lower_hex(s: &str) -> bool {
    s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// True for the name of a preview build: `pre-` and the first 7 to 40 lowercase hexadecimal digits of the
/// commit (release.yml; owner decision 2026-11-10).
fn is_preview(s: &str) -> bool {
    s.strip_prefix("pre-").is_some_and(|h| (7..=40).contains(&h.len()) && is_lower_hex(h))
}

/// True if the app of version `current` looks for updates: a stable build or a preview build, not a
/// `dev` build (UPD-1, owner decision 2026-11-01).
pub fn has_updates(current: &str) -> bool {
    parse_version(current).is_some() || is_preview(current)
}

/// True if the stable release `remote` is an update for `current` (UPD-2): a newer version for a
/// stable build. A preview build cannot compare its name with a version, thus each stable release
/// is an update for it (owner decision 2026-11-01).
pub fn is_update(remote: &str, current: &str) -> bool {
    match (parse_version(remote), parse_version(current)) {
        (Some(r), Some(c)) => r > c,
        (Some(_), None) => is_preview(current),
        (None, _) => false,
    }
}

/// The checked contents of `version.json` (`read_manifest`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Manifest {
    pub version: String,
    pub url: String,
    pub sha256: String,
}

/// The JSON format of `version.json` (REQUIREMENTS.md §9.2, owner approval 2026-11-12). The
/// `version.json` of a release from 2026-11-01 to that date also has `patch_url` and `patch_sha256`:
/// the app does not read them (UPD-4). `read_manifest` makes a `Manifest` from it.
#[derive(Deserialize)]
struct VersionJson {
    version: String,
    url: String,
    sha256: String,
}

fn is_sha256(s: &str) -> bool {
    s.len() == 64 && is_lower_hex(s)
}

/// The manifest in `json`, if `signature` is the ed25519 signature of these bytes with `key` (UPD-4).
pub fn read_manifest(json: &[u8], signature: &[u8], key: &[u8; 32]) -> Result<Manifest, String> {
    let key = VerifyingKey::from_bytes(key).map_err(|e| format!("public key: {e}"))?;
    let signature: [u8; 64] = signature.try_into().map_err(|_| format!("the signature has {} bytes, not 64", signature.len()))?;
    key.verify_strict(json, &Signature::from_bytes(&signature)).map_err(|_| "the signature of version.json is not correct".to_string())?;
    let raw: VersionJson = serde_json::from_slice(json).map_err(|e| format!("version.json: {e}"))?;
    if parse_version(&raw.version).is_none() {
        return Err(format!("version.json: {:?} is not a stable version", raw.version));
    }
    if !raw.url.starts_with("https://") {
        return Err(format!("version.json: the URL {:?} is not HTTPS", raw.url));
    }
    if !is_sha256(&raw.sha256) {
        return Err(format!("version.json: {:?} is not a SHA-256 checksum", raw.sha256));
    }
    Ok(Manifest { version: raw.version, url: raw.url, sha256: raw.sha256 })
}

/// The SHA-256 checksum of `bytes` in lowercase hexadecimal.
fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// The folders that an installation uses next to the AppLoad folder `app_dir` of the app.
fn sibling(app_dir: &Path, suffix: &str) -> Result<PathBuf, String> {
    let name = app_dir.file_name().ok_or_else(|| format!("{} has no folder name", app_dir.display()))?;
    Ok(app_dir.with_file_name(format!("{}.{suffix}", name.to_string_lossy())))
}

/// Deletes the folders that stay from an earlier installation: `<app>.old`, `<app>.new` and
/// `staging` (REQUIREMENTS.md §9.2, step 5).
pub fn clean(staging: &Path, app_dir: &Path) {
    for dir in [sibling(app_dir, "old"), sibling(app_dir, "new"), Ok(staging.to_path_buf())].into_iter().flatten() {
        if dir.exists() {
            if let Err(e) = std::fs::remove_dir_all(&dir) {
                eprintln!("sudoku: cannot delete {}: {e}", dir.display());
            }
        }
    }
}

/// What the folder of a release archive has (UPD-12).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArchiveKind {
    /// The program and `external.manifest.json`, and no `manifest.json`: a release before the
    /// frontend.
    External,
    /// Also all frontend files (`launch::FRONTEND_FILES`).
    Frontend,
}

/// Examines the extracted folder `sudoku/` of a release archive (UPD-12). The folder must have the
/// program and `external.manifest.json`. A folder that has `manifest.json` must also have the other
/// frontend files: thus an installation never makes an app folder with only a part of them.
pub fn check_folder(folder: &Path) -> Result<ArchiveKind, String> {
    let missing = |f: &str| format!("the archive has no {ARCHIVE_DIR}/{f}");
    if let Some(f) = REQUIRED_FILES.iter().find(|f| !folder.join(f).is_file()) {
        return Err(missing(f));
    }
    if !folder.join(FRONTEND_MANIFEST).is_file() {
        return Ok(ArchiveKind::External);
    }
    match FRONTEND_FILES.iter().find(|f| !folder.join(f).is_file()) {
        Some(f) => Err(missing(f)),
        None => Ok(ArchiveKind::Frontend),
    }
}

/// Sets the executable mode of the program in the app folder `folder`, and of `backend/entry` if the
/// folder has it (UPD-12).
fn set_executables(folder: &Path) -> Result<(), String> {
    set_executable(&folder.join(PROGRAM))?;
    let entry = folder.join(BACKEND_ENTRY);
    if entry.is_file() {
        set_executable(&entry)?;
    }
    Ok(())
}

/// Installs the release archive `archive` of the manifest `m` in place of the AppLoad folder
/// `app_dir` (REQUIREMENTS.md §9.2). `staging` is an empty folder for the download (on /home). If
/// this fails, `app_dir` does not change (UPD-6).
pub fn install(archive: &[u8], m: &Manifest, staging: &Path, app_dir: &Path) -> Result<(), String> {
    let sum = sha256_hex(archive);
    if sum != m.sha256 {
        return Err(format!("the checksum of the archive is {sum}, not {}", m.sha256));
    }
    let new = sibling(app_dir, "new")?;
    let old = sibling(app_dir, "old")?;
    clean(staging, app_dir);
    // The archive goes to `update/` in the data folder (REQUIREMENTS.md §9.2, step 1). The app
    // extracts that file.
    let archive_file = staging.join(ARCHIVE_NAME);
    std::fs::create_dir_all(staging).map_err(|e| format!("cannot make {}: {e}", staging.display()))?;
    std::fs::write(&archive_file, archive).map_err(|e| format!("cannot write {}: {e}", archive_file.display()))?;
    let extract = staging.join("extract");
    std::fs::create_dir_all(&extract).map_err(|e| format!("cannot make {}: {e}", extract.display()))?;
    let file = std::fs::File::open(&archive_file).map_err(|e| format!("cannot read {}: {e}", archive_file.display()))?;
    unpack(file, &extract)?;
    let folder = extract.join(ARCHIVE_DIR);
    check_folder(&folder)?;
    set_executables(&folder)?;

    // The new folder goes next to the app folder. A rename is not possible across file systems: then
    // copy the files and the subfolders (`backend/`).
    if std::fs::rename(&folder, &new).is_err() {
        copy_folder(&folder, &new)?;
        set_executables(&new)?;
    }
    // Two renames: the program that operates keeps its files (it is not possible to write over a
    // program that operates).
    std::fs::rename(app_dir, &old).map_err(|e| format!("cannot rename {}: {e}", app_dir.display()))?;
    if let Err(e) = std::fs::rename(&new, app_dir) {
        let back = std::fs::rename(&old, app_dir);
        return Err(format!("cannot rename {}: {e} (rename back: {back:?})", new.display()));
    }
    clean(staging, app_dir);
    Ok(())
}

/// The folder of the ink patch of the releases before the frontend (REQUIREMENTS.md §14.4).
/// qt-resource-rebuilder reads the patches here when xochitl starts.
pub const PATCH_DIR: &str = "/home/root/xovi/exthome/qt-resource-rebuilder";
/// The file name of the ink patch in `PATCH_DIR`. The corpus app had its own copy of the patch: it
/// does not use and does not delete the patch of Sudoku.
pub const PATCH_NAME: &str = if cfg!(feature = "corpus") { "sudoku-corpus-penregion.qmd" } else { "sudoku-penregion.qmd" };
/// The stamp file next to the patch: an earlier release and the installer wrote the version and the
/// checksum of the patch in it. The app does not read it and does not write it (UPD-11). The corpus
/// app had no stamp file.
pub const PATCH_STAMP: &str = if cfg!(feature = "corpus") { "sudoku-corpus-penregion.json" } else { "sudoku-penregion.json" };

/// Deletes the ink patch and its stamp file in `patch_dir`, and the pen area file `pen_area_file`
/// of the external mode, if they are there (UPD-11). Only the frontend mode calls it: the external
/// mode uses the patch and the file.
pub fn remove_ink_patch(patch_dir: &Path, pen_area_file: &Path) {
    for file in [patch_dir.join(PATCH_NAME), patch_dir.join(PATCH_STAMP), pen_area_file.to_path_buf()] {
        match std::fs::remove_file(&file) {
            Ok(()) => eprintln!("sudoku: deleted {}", file.display()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => eprintln!("sudoku: cannot delete {}: {e}", file.display()),
        }
    }
}

/// Extracts the gzip tar archive `archive` in `dest`. Each entry must be a file or a folder in `sudoku/`.
fn unpack(archive: impl Read, dest: &Path) -> Result<(), String> {
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(archive));
    tar.set_preserve_permissions(false);
    let mut size = 0u64;
    for entry in tar.entries().map_err(|e| format!("archive: {e}"))? {
        let mut entry = entry.map_err(|e| format!("archive: {e}"))?;
        let path = entry.path().map_err(|e| format!("archive: {e}"))?.into_owned();
        let mut parts = path.components();
        let in_folder = parts.next() == Some(Component::Normal(ARCHIVE_DIR.as_ref()));
        if !in_folder || !parts.all(|c| matches!(c, Component::Normal(_))) {
            return Err(format!("archive: the path {} is not in {ARCHIVE_DIR}/", path.display()));
        }
        let kind = entry.header().entry_type();
        if !kind.is_file() && !kind.is_dir() {
            return Err(format!("archive: {} is not a file or a folder", path.display()));
        }
        size += entry.size();
        if size > MAX_ARCHIVE_BYTES {
            return Err("archive: the files are too large".to_string());
        }
        if !entry.unpack_in(dest).map_err(|e| format!("archive: {}: {e}", path.display()))? {
            return Err(format!("archive: the path {} is not in {ARCHIVE_DIR}/", path.display()));
        }
    }
    Ok(())
}

fn set_executable(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).map_err(|e| format!("cannot set the mode of {}: {e}", path.display()))?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

/// Copies the files and the subfolders of the folder `from` to the new folder `to` (UPD-12: also
/// `backend/`). `unpack` makes only files and folders, thus `from` has no links.
fn copy_folder(from: &Path, to: &Path) -> Result<(), String> {
    std::fs::create_dir(to).map_err(|e| format!("cannot make {}: {e}", to.display()))?;
    for entry in std::fs::read_dir(from).map_err(|e| format!("{}: {e}", from.display()))? {
        let entry = entry.map_err(|e| format!("{}: {e}", from.display()))?;
        let dest = to.join(entry.file_name());
        let kind = entry.file_type().map_err(|e| format!("{}: {e}", entry.path().display()))?;
        if kind.is_dir() {
            copy_folder(&entry.path(), &dest)?;
        } else {
            std::fs::copy(entry.path(), &dest).map_err(|e| format!("cannot copy to {}: {e}", dest.display()))?;
        }
    }
    Ok(())
}

/// The update state (INVARIANTS.md §16). The fields `checking` and `up_to_date_shown` of INVARIANTS.md
/// are in the variants, thus only their correct combinations can occur: a request is in progress only
/// in `Unknown`, `CheckFailed` and `UpToDate` (UPD-7, UPD-9), `Unknown` always has its request in
/// progress (UPD-8), and "Sudoku is up to date" shows only in `UpToDate` with no request (UPD-9).
/// The field `dialog` of INVARIANTS.md does not depend on this state: it is in `Update::launch`
/// (`launch::Restart::Needed`), thus the dialog "Restart the interface?" can show only while the
/// update is not complete (UPD-10).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum UpdateState {
    /// A `dev` build (not a stable build and not a preview build): no requests and no update row (UPD-1).
    Off,
    /// The first request (when the app opens) is in progress (UPD-8).
    Unknown,
    /// All requests failed. `checking`: a request from the button "Check for updates" is in progress.
    CheckFailed { checking: bool },
    /// The last result found no update.
    UpToDate(UpToDateRow),
    Available(Manifest),
    Installing(Manifest),
    /// The installation failed at this time (ms). After `FAILED_MS`, the button shows again.
    Failed(Manifest, u64),
    /// The new version operates after the next start of the app.
    Installed,
}

/// The update row in the state `UpToDate` (UPD-9).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UpToDateRow {
    /// The button "Check for updates".
    CheckButton,
    /// "Looking for updates...": a request from the button is in progress.
    Checking,
    /// "Sudoku is up to date": a request found no update while the Settings panel was open, and the
    /// panel did not close after it.
    UpToDateText,
}

/// The button of the update row (REQUIREMENTS.md §9.2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpdateButton {
    /// "Update to v<version>": starts the installation.
    Install(String),
    /// "Check for updates": sends a request for `version.json`.
    Check,
    /// "Restart interface": runs xovi/start while the update is not complete (UPD-10).
    Restart,
}

impl UpdateButton {
    pub fn text(&self) -> &str {
        match self {
            UpdateButton::Install(t) => t,
            UpdateButton::Check => "Check for updates",
            UpdateButton::Restart => "Restart interface",
        }
    }
}

/// Work for the platform.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpdateRequest {
    /// Get and examine `version.json` (`Update::checked`).
    Check,
    /// Download and install the archive (`Update::install_finished`).
    Install(Manifest),
}

/// The update state of the app. The methods do not do input or output: the platform takes
/// `request` and gives the results.
#[derive(Clone, Debug)]
pub struct Update {
    /// The version of the app (`crate::VERSION`).
    pub current: String,
    pub state: UpdateState,
    /// Work that the platform must start.
    pub request: Option<UpdateRequest>,
    /// How AppLoad started the program (LCH-9), with the restart state of the external mode
    /// (UPD-10): `restart_needed` and `dialog` of INVARIANTS.md. Only `App::with_launch` sets it, at
    /// the start, and "Later" closes the dialog.
    pub launch: Launch,
}

impl Update {
    /// The state when the app opens (UPD-8): a stable or a preview build sends a request immediately. This is the
    /// only automatic request. After it, only the button "Check for updates" sends a request.
    /// The launch is the frontend mode (the usual mode): `App::with_launch` gives the mode of the
    /// program.
    pub fn new(current: &str) -> Update {
        let on = has_updates(current);
        Update {
            current: current.to_string(),
            state: if on { UpdateState::Unknown } else { UpdateState::Off },
            request: on.then_some(UpdateRequest::Check),
            launch: Launch::Frontend,
        }
    }

    /// The update is not complete (`restart_needed` of INVARIANTS.md, UPD-10): the program runs in
    /// the external mode, and its app folder has the frontend files. AppLoad uses the new manifest
    /// only after the reMarkable interface starts again.
    pub fn restart_needed(&self) -> bool {
        self.launch.restart_needed()
    }

    /// The dialog "Restart the interface?" shows (UPD-10).
    pub fn dialog_shown(&self) -> bool {
        self.launch.dialog_shown()
    }

    /// Closes the dialog "Restart the interface?" ("Later", or xovi/start failed). The row continues to
    /// show the button "Restart interface" (UPD-10).
    pub fn close_dialog(&mut self) {
        self.launch.close_dialog();
    }

    /// The Settings panel has the update row (UPD-1).
    pub fn has_row(&self) -> bool {
        self.state != UpdateState::Off
    }

    /// The gear has the caution icon (UPD-3): an update is available or installed, or the update is
    /// not complete in a build that has the update row.
    pub fn caution(&self) -> bool {
        matches!(self.state, UpdateState::Available(_) | UpdateState::Installing(_) | UpdateState::Failed(..) | UpdateState::Installed)
            || (self.restart_needed() && self.has_row())
    }

    /// The text of the update row, or `None` for a button (REQUIREMENTS.md §9.2). While the update is
    /// not complete, the row shows only the button "Restart interface" (UPD-14).
    pub fn row_text(&self) -> Option<&'static str> {
        if self.restart_needed() {
            return None;
        }
        Some(match self.state {
            UpdateState::Off | UpdateState::CheckFailed { checking: false } | UpdateState::UpToDate(UpToDateRow::CheckButton) | UpdateState::Available(_) => {
                return None
            }
            UpdateState::Unknown | UpdateState::CheckFailed { checking: true } | UpdateState::UpToDate(UpToDateRow::Checking) => "Looking for updates...",
            UpdateState::UpToDate(UpToDateRow::UpToDateText) => "Sudoku is up to date",
            UpdateState::Installing(_) => "Installing update...",
            UpdateState::Failed(..) => "Update failed",
            UpdateState::Installed => "Restart Sudoku to update",
        })
    }

    /// The button of the update row, or `None` for a text (REQUIREMENTS.md §9.2). While the update is
    /// not complete, a build with the update row shows "Restart interface" for each state (UPD-10,
    /// UPD-14).
    pub fn button(&self) -> Option<UpdateButton> {
        if self.restart_needed() {
            return self.has_row().then_some(UpdateButton::Restart);
        }
        match &self.state {
            UpdateState::Available(m) => Some(UpdateButton::Install(format!("Update to {}", m.version))),
            UpdateState::CheckFailed { checking: false } | UpdateState::UpToDate(UpToDateRow::CheckButton) => Some(UpdateButton::Check),
            UpdateState::Off
            | UpdateState::Unknown
            | UpdateState::CheckFailed { checking: true }
            | UpdateState::UpToDate(UpToDateRow::Checking | UpToDateRow::UpToDateText)
            | UpdateState::Installing(_)
            | UpdateState::Failed(..)
            | UpdateState::Installed => None,
        }
    }

    /// Moves the timers to `now` (ms). True if the row changed.
    pub fn tick(&mut self, now: u64) -> bool {
        if let UpdateState::Failed(m, at) = &self.state {
            if now >= at + FAILED_MS {
                self.state = UpdateState::Available(m.clone());
                return true;
            }
        }
        false
    }

    /// The time (ms) at which "Update failed" goes away (UPD-6), if it shows. The main loop wakes at
    /// this time, thus the text shows for 3 seconds.
    pub fn deadline(&self) -> Option<u64> {
        match &self.state {
            UpdateState::Failed(_, at) => Some(at + FAILED_MS),
            _ => None,
        }
    }

    /// The Settings panel closed: the row shows the button "Check for updates" again (UPD-9).
    pub fn panel_closed(&mut self) {
        if self.state == UpdateState::UpToDate(UpToDateRow::UpToDateText) {
            self.state = UpdateState::UpToDate(UpToDateRow::CheckButton);
        }
    }

    /// The result of a request (UPD-2, UPD-7). `panel_open`: the Settings panel shows now. Then a
    /// result with no update shows "Sudoku is up to date" until the panel closes (UPD-9). While the
    /// update is not complete, the row does not show this text (UPD-14), thus the state does not
    /// keep it.
    pub fn checked(&mut self, result: Result<Manifest, String>, panel_open: bool) {
        let panel_open = panel_open && !self.restart_needed();
        let can_change = matches!(self.state, UpdateState::Unknown | UpdateState::CheckFailed { .. } | UpdateState::UpToDate(_) | UpdateState::Available(_));
        match result {
            Ok(m) => {
                if can_change {
                    self.state = if is_update(&m.version, &self.current) {
                        UpdateState::Available(m)
                    } else if panel_open {
                        UpdateState::UpToDate(UpToDateRow::UpToDateText)
                    } else {
                        UpdateState::UpToDate(UpToDateRow::CheckButton)
                    };
                }
            }
            Err(e) => {
                eprintln!("sudoku: cannot look for updates: {e}");
                // The request is complete. A failure keeps the last result.
                match self.state {
                    UpdateState::Unknown | UpdateState::CheckFailed { .. } => self.state = UpdateState::CheckFailed { checking: false },
                    UpdateState::UpToDate(UpToDateRow::Checking) => self.state = UpdateState::UpToDate(UpToDateRow::CheckButton),
                    UpdateState::Off
                    | UpdateState::UpToDate(UpToDateRow::CheckButton | UpToDateRow::UpToDateText)
                    | UpdateState::Available(_)
                    | UpdateState::Installing(_)
                    | UpdateState::Failed(..)
                    | UpdateState::Installed => {}
                }
            }
        }
    }

    /// A tap on the button (UPD-5, UPD-9, UPD-10). "Update to v<version>" starts an installation, and
    /// "Check for updates" sends a request. "Restart interface" changes nothing here: the app asks the
    /// platform to run xovi/start (`App::restart_ui`). While the update is not complete, the row has
    /// only that button, thus no installation and no request starts (UPD-14). True if the row changed.
    pub fn tap(&mut self) -> bool {
        if self.restart_needed() {
            return false;
        }
        let (state, request) = match &self.state {
            UpdateState::Available(m) => (UpdateState::Installing(m.clone()), UpdateRequest::Install(m.clone())),
            UpdateState::CheckFailed { checking: false } => (UpdateState::CheckFailed { checking: true }, UpdateRequest::Check),
            UpdateState::UpToDate(UpToDateRow::CheckButton) => (UpdateState::UpToDate(UpToDateRow::Checking), UpdateRequest::Check),
            // No button.
            UpdateState::Off
            | UpdateState::Unknown
            | UpdateState::CheckFailed { checking: true }
            | UpdateState::UpToDate(UpToDateRow::Checking | UpToDateRow::UpToDateText)
            | UpdateState::Installing(_)
            | UpdateState::Failed(..)
            | UpdateState::Installed => return false,
        };
        self.state = state;
        self.request = Some(request);
        true
    }

    /// The result of an installation (UPD-6, UPD-12). An installation never shows the dialog "Restart
    /// the interface?" (UPD-10).
    pub fn install_finished(&mut self, result: Result<(), String>, now: u64) {
        if let UpdateState::Installing(m) = &self.state {
            self.state = match result {
                Ok(()) => UpdateState::Installed,
                Err(e) => {
                    eprintln!("sudoku: the update failed: {e}");
                    UpdateState::Failed(m.clone(), now)
                }
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    fn manifest(version: &str, archive: &[u8]) -> Manifest {
        Manifest { version: version.to_string(), url: "https://example.com/a.tar.gz".to_string(), sha256: sha256_hex(archive), ..Manifest::default() }
    }

    #[test]
    fn versions() {
        assert_eq!(parse_version("v0.1.0"), Some(Version(0, 1, 0)));
        assert_eq!(parse_version("v12.0.3"), Some(Version(12, 0, 3)));
        for s in ["dev", "dev/update-icons", "pre-4118468", "0.1.0", "v0.1", "v0.1.0.1", "v0..1", "v0.1.x", "v+1.0.0", "v0.1.0-rc1", ""] {
            assert_eq!(parse_version(s), None, "{s}");
        }
        assert!(Version(0, 10, 0) > Version(0, 9, 9));
        assert!(Version(1, 0, 0) > Version(0, 99, 99));
    }

    #[test]
    fn the_manifest_needs_a_correct_signature() {
        // UPD-4
        let key = SigningKey::from_bytes(&[7; 32]);
        let public = key.verifying_key().to_bytes();
        let json = br#"{"version": "v0.2.0", "url": "https://x/y.tar.gz", "sha256": "0000000000000000000000000000000000000000000000000000000000000000"}"#;
        let sig = key.sign(json).to_bytes();
        let m = read_manifest(json, &sig, &public).unwrap();
        assert_eq!(m.version, "v0.2.0");
        let mut changed = json.to_vec();
        changed[14] = b'3';
        assert!(read_manifest(&changed, &sig, &public).is_err());
        assert!(read_manifest(json, &sig[..63], &public).is_err());
        let other = SigningKey::from_bytes(&[8; 32]).verifying_key().to_bytes();
        assert!(read_manifest(json, &sig, &other).is_err());
        let bad = br#"{"version": "pre-abc", "url": "https://x", "sha256": "00"}"#;
        assert!(read_manifest(bad, &key.sign(bad).to_bytes(), &public).is_err());
        // UPD-4: the patch fields of an earlier release have no effect, with each value.
        let z = "0".repeat(64);
        for patch in [
            format!(r#""patch_url": "https://x/p.qmd", "patch_sha256": "{z}""#),
            r#""patch_url": "http://x/p.qmd""#.to_string(),
            r#""patch_sha256": "00""#.to_string(),
        ] {
            let json = format!(r#"{{"version": "v0.2.0", "url": "https://x/y.tar.gz", "sha256": "{z}", {patch}}}"#);
            let with_patch = read_manifest(json.as_bytes(), &key.sign(json.as_bytes()).to_bytes(), &public).unwrap();
            assert_eq!(with_patch, m, "{patch}");
        }
    }

    #[test]
    fn the_public_key_is_valid() {
        assert!(VerifyingKey::from_bytes(&PUBLIC_KEY).is_ok());
    }

    #[test]
    fn only_dev_builds_do_not_look_for_updates() {
        // UPD-1, UPD-8
        assert!(is_preview("pre-4118468") && is_preview("pre-4118468aabbccddeeff00112233445566778899"));
        for v in ["pre-", "pre-411846", "pre-4118468X", "pre-ABCDEF0", "dev", "dev/update-icons", "v0.1.0"] {
            assert!(!is_preview(v), "{v}");
        }
        for v in ["dev", "dev/update-icons", "pre-", "pre-xyz", ""] {
            let mut u = Update::new(v);
            assert_eq!(u.state, UpdateState::Off);
            assert_eq!(u.request, None);
            u.tick(u64::MAX / 2);
            assert!(!u.tap());
            assert_eq!(u.request, None);
            assert!(!u.has_row());
        }
        for v in ["v0.1.0", "pre-4118468"] {
            let u = Update::new(v);
            assert_eq!((u.state.clone(), u.request.clone()), (UpdateState::Unknown, Some(UpdateRequest::Check)), "{v}");
            assert!(u.has_row());
        }
    }

    #[test]
    fn only_a_newer_version_is_an_update() {
        // UPD-2
        for (remote, want) in [("v0.1.0", false), ("v0.0.9", false), ("v0.1.1", true), ("v1.0.0", true)] {
            let mut u = Update::new("v0.1.0");
            u.checked(Ok(manifest(remote, b"")), false);
            assert_eq!(matches!(u.state, UpdateState::Available(_)), want, "{remote}");
            assert_eq!(u.caution(), want);
        }
        // A preview build: each stable release is an update.
        for remote in ["v0.0.1", "v0.1.0", "v9.0.0"] {
            let mut u = Update::new("pre-4118468");
            u.checked(Ok(manifest(remote, b"")), false);
            assert_eq!(u.button(), Some(UpdateButton::Install(format!("Update to {remote}"))), "{remote}");
        }
        assert!(!is_update("pre-1234567", "pre-4118468"));
        assert!(!is_update("v0.2.0", "dev"));
        assert!(!is_update("v0.2.0", "dev/update-icons"));
    }

    #[test]
    fn only_the_start_and_the_button_send_requests() {
        // UPD-7, UPD-8, UPD-9
        let mut u = Update::new("v0.1.0");
        assert_eq!(u.request.take(), Some(UpdateRequest::Check));
        assert_eq!((u.row_text(), u.button()), (Some("Looking for updates..."), None));
        u.checked(Err("no network".into()), true);
        assert_eq!(u.state, UpdateState::CheckFailed { checking: false });
        assert!(!u.tick(u64::MAX / 2));
        assert_eq!(u.request, None, "no automatic request after the start");
        assert_eq!((u.row_text(), u.button()), (None, Some(UpdateButton::Check)));
        assert!(u.tap());
        assert_eq!(u.request.take(), Some(UpdateRequest::Check));
        assert_eq!((u.row_text(), u.button()), (Some("Looking for updates..."), None));
        assert!(!u.tap(), "one request at a time");
        assert_eq!(u.request, None);
        u.checked(Ok(manifest("v0.1.0", b"")), true);
        assert_eq!(u.state, UpdateState::UpToDate(UpToDateRow::UpToDateText));
        assert_eq!((u.row_text(), u.button()), (Some("Sudoku is up to date"), None), "the panel is open");
        assert!(!u.tap());
        u.panel_closed();
        assert_eq!(u.button().map(|b| b.text().to_string()).as_deref(), Some("Check for updates"));
        assert!(u.tap());
        assert_eq!(u.request.take(), Some(UpdateRequest::Check));
        u.checked(Err("no network".into()), true);
        assert_eq!(u.state, UpdateState::UpToDate(UpToDateRow::CheckButton), "a failure keeps the last result");
        assert_eq!(u.button(), Some(UpdateButton::Check));
        assert!(u.tap());
        u.checked(Ok(manifest("v0.1.0", b"")), false);
        assert_eq!(u.button(), Some(UpdateButton::Check), "the panel is closed: no text");
        assert!(u.tap());
        u.checked(Ok(manifest("v0.2.0", b"")), true);
        assert!(matches!(u.state, UpdateState::Available(_)));
        assert_eq!(u.request.take(), Some(UpdateRequest::Check));
        u.checked(Ok(manifest("v0.2.0", b"")), true);
        u.tick(u64::MAX / 2);
        assert_eq!(u.request, None);
    }

    #[test]
    fn a_failed_installation_shows_the_button_again() {
        // UPD-5, UPD-6
        let mut u = Update::new("v0.1.0");
        u.request = None;
        assert!(!u.tap(), "no button while the first request is in progress");
        u.checked(Ok(manifest("v0.2.0", b"x")), true);
        assert_eq!(u.button(), Some(UpdateButton::Install("Update to v0.2.0".to_string())));
        assert!(u.tap());
        assert!(matches!(u.request.take(), Some(UpdateRequest::Install(_))));
        assert!(!u.tap(), "one installation at a time");
        assert_eq!(u.row_text(), Some("Installing update..."));
        u.checked(Ok(manifest("v0.3.0", b"x")), true);
        assert!(matches!(u.state, UpdateState::Installing(_)), "a request does not change an installation");
        u.install_finished(Err("checksum".into()), 100);
        assert_eq!(u.row_text(), Some("Update failed"));
        assert!(!u.tick(100 + FAILED_MS - 1));
        assert!(u.tick(100 + FAILED_MS));
        assert!(u.tap());
        assert!(matches!(u.request.take(), Some(UpdateRequest::Install(_))));
        u.install_finished(Ok(()), 200);
        assert_eq!((u.state.clone(), u.row_text()), (UpdateState::Installed, Some("Restart Sudoku to update")));
        assert!(!u.dialog_shown(), "an installation never shows the dialog (UPD-10)");
        assert!(u.caution());
        u.tick(u64::MAX / 2);
        assert_eq!(u.request, None, "no requests after the installation");
    }

    /// A gzip tar archive with the files (path, contents).
    fn archive(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut b = tar::Builder::new(flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default()));
        for (path, data) in files {
            let mut h = tar::Header::new_gnu();
            h.set_size(data.len() as u64);
            h.set_mode(0o644);
            h.set_entry_type(tar::EntryType::Regular);
            h.as_gnu_mut().unwrap().name[..path.len()].copy_from_slice(path.as_bytes());
            h.set_cksum();
            b.append(&h, *data).unwrap();
        }
        b.into_inner().unwrap().finish().unwrap()
    }

    fn temp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("sudoku-update-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("appload/sudoku")).unwrap();
        std::fs::write(d.join("appload/sudoku/sudoku"), b"old").unwrap();
        std::fs::write(d.join("appload/sudoku/external.manifest.json"), b"{}").unwrap();
        d
    }

    #[test]
    fn install_replaces_the_app_folder() {
        let d = temp_dir("ok");
        let app = d.join("appload/sudoku");
        let a = archive(&[("sudoku/sudoku", b"new"), ("sudoku/external.manifest.json", b"{\"name\": 1}"), ("sudoku/icon.png", b"png")]);
        install(&a, &manifest("v0.2.0", &a), &d.join("update"), &app).unwrap();
        assert_eq!(std::fs::read(app.join("sudoku")).unwrap(), b"new");
        assert_eq!(std::fs::read(app.join("icon.png")).unwrap(), b"png");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(app.join("sudoku")).unwrap().permissions().mode() & 0o777, 0o755);
        }
        let mut left: Vec<_> = std::fs::read_dir(d.join("appload")).unwrap().map(|e| e.unwrap().file_name()).collect();
        left.sort();
        assert_eq!(left, ["sudoku"], "no sudoku.old or sudoku.new");
        assert!(!d.join("update").exists());
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// The files of a release archive with the frontend (LCH-6).
    const FRONTEND_ARCHIVE: [(&str, &[u8]); 6] = [
        ("sudoku/sudoku", b"new"),
        ("sudoku/external.manifest.json", b"{}"),
        ("sudoku/manifest.json", b"{\"id\": \"sudoku\"}"),
        ("sudoku/resources.rcc", b"qres"),
        ("sudoku/backend/entry", b"#!/bin/sh"),
        ("sudoku/icon.png", b"png"),
    ];

    #[cfg(unix)]
    fn mode(path: &Path) -> u32 {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    #[test]
    fn install_keeps_the_frontend_files_and_makes_the_two_programs_executable() {
        // UPD-12
        let d = temp_dir("frontend");
        let app = d.join("appload/sudoku");
        let a = archive(&FRONTEND_ARCHIVE);
        install(&a, &manifest("v0.7.0", &a), &d.join("update"), &app).unwrap();
        for (path, data) in FRONTEND_ARCHIVE {
            assert_eq!(std::fs::read(d.join("appload").join(path)).unwrap(), data, "{path}");
        }
        assert!(crate::launch::has_frontend_files(&app));
        #[cfg(unix)]
        assert_eq!((mode(&app.join("sudoku")), mode(&app.join("backend/entry")), mode(&app.join("icon.png"))), (0o755, 0o755, 0o644));
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn an_archive_has_all_frontend_files_or_none() {
        // UPD-12, UPD-6
        let d = temp_dir("part");
        let app = d.join("appload/sudoku");
        for missing in ["sudoku/resources.rcc", "sudoku/backend/entry"] {
            let files: Vec<_> = FRONTEND_ARCHIVE.into_iter().filter(|(path, _)| *path != missing).collect();
            let a = archive(&files);
            let e = install(&a, &manifest("v0.7.0", &a), &d.join("update"), &app).unwrap_err();
            assert!(e.contains(missing), "{missing}: {e}");
            assert_eq!(std::fs::read(app.join("sudoku")).unwrap(), b"old", "{missing}");
            assert!(!app.join("manifest.json").exists(), "{missing}");
        }
        // No manifest.json: a release before the frontend. The other frontend files are not examined.
        let files: Vec<_> = FRONTEND_ARCHIVE.into_iter().filter(|(path, _)| *path != "sudoku/manifest.json").collect();
        let folder = d.join("folder");
        std::fs::create_dir_all(&folder).unwrap();
        unpack(&archive(&files)[..], &folder).unwrap();
        assert_eq!(check_folder(&folder.join(ARCHIVE_DIR)), Ok(ArchiveKind::External));
        std::fs::write(folder.join(ARCHIVE_DIR).join("manifest.json"), b"{}").unwrap();
        assert_eq!(check_folder(&folder.join(ARCHIVE_DIR)), Ok(ArchiveKind::Frontend));
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn the_copy_of_a_folder_includes_the_subfolder() {
        // UPD-12: the move across file systems.
        let d = temp_dir("copy");
        let from = d.join("from");
        std::fs::create_dir_all(&from).unwrap();
        unpack(&archive(&FRONTEND_ARCHIVE)[..], &from).unwrap();
        let (from, to) = (from.join(ARCHIVE_DIR), d.join("to"));
        copy_folder(&from, &to).unwrap();
        assert_eq!(check_folder(&to), Ok(ArchiveKind::Frontend));
        set_executables(&to).unwrap();
        for (path, data) in FRONTEND_ARCHIVE {
            assert_eq!(std::fs::read(to.join(path.strip_prefix("sudoku/").unwrap())).unwrap(), data, "{path}");
        }
        #[cfg(unix)]
        assert_eq!((mode(&to.join("sudoku")), mode(&to.join("backend/entry"))), (0o755, 0o755));
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn the_ink_patch_and_its_files_are_deleted() {
        // UPD-11
        let d = temp_dir("patch");
        let (patches, area) = (d.join("patches"), d.join("sudoku-pen-region"));
        std::fs::create_dir_all(&patches).unwrap();
        for file in [patches.join(PATCH_NAME), patches.join(PATCH_STAMP), patches.join("other.qmd"), area.clone()] {
            std::fs::write(file, b"x").unwrap();
        }
        remove_ink_patch(&patches, &area);
        let left: Vec<_> = std::fs::read_dir(&patches).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(left, ["other.qmd"]);
        assert!(!area.exists());
        // The files are not there: no effect.
        remove_ink_patch(&patches, &area);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn the_row_shows_only_the_restart_button_while_the_update_is_not_complete() {
        // UPD-3, UPD-10, UPD-14
        use crate::launch::Restart;
        assert_eq!(UpdateButton::Restart.text(), "Restart interface");
        let needed = Launch::External(Restart::Needed { dialog: true });
        let mut u = Update::new("v0.1.0");
        assert_eq!((u.launch, u.restart_needed(), u.dialog_shown(), u.caution()), (Launch::Frontend, false, false, false));
        u.launch = needed;
        assert!(u.dialog_shown() && u.caution(), "the caution icon from the start");
        assert_eq!((u.row_text(), u.button()), (None, Some(UpdateButton::Restart)));
        u.close_dialog();
        assert!(!u.dialog_shown() && u.restart_needed());
        // Each result of a request keeps the button, and a tap starts no installation and no request.
        for result in [Err("no network".to_string()), Ok(manifest("v0.1.0", b"x")), Ok(manifest("v0.2.0", b"x"))] {
            u.checked(result, true);
            assert_eq!((u.row_text(), u.button()), (None, Some(UpdateButton::Restart)), "{:?}", u.state);
            u.request = None;
            assert!(!u.tap());
            assert_eq!(u.request, None, "{:?}", u.state);
        }
        assert!(matches!(u.state, UpdateState::Available(_)));
        // A `dev` build has no update row: no button and no caution icon, but the dialog can show.
        let mut dev = Update::new("dev");
        dev.launch = needed;
        assert_eq!((dev.dialog_shown(), dev.caution(), dev.button(), dev.row_text()), (true, false, None, None));
        // The external mode with no frontend files is as the frontend mode.
        let mut old = Update::new("v0.1.0");
        old.launch = Launch::External(Restart::NotNeeded);
        old.checked(Ok(manifest("v0.2.0", b"x")), true);
        assert!(!old.restart_needed() && !old.dialog_shown());
        assert_eq!(old.button(), Some(UpdateButton::Install("Update to v0.2.0".to_string())));
    }

    #[test]
    fn a_bad_archive_does_not_change_the_app_folder() {
        // UPD-4, UPD-6
        let d = temp_dir("bad");
        let app = d.join("appload/sudoku");
        let good = archive(&[("sudoku/sudoku", b"new"), ("sudoku/external.manifest.json", b"{}")]);
        let mut wrong_sum = manifest("v0.2.0", &good);
        wrong_sum.sha256 = sha256_hex(b"other");
        let cases = [
            (good.clone(), wrong_sum),
            (archive(&[("sudoku/sudoku", b"new")]), manifest("v0.2.0", b"")),
            (archive(&[("other/sudoku", b"new"), ("other/external.manifest.json", b"{}")]), manifest("v0.2.0", b"")),
            (archive(&[("sudoku/../../evil", b"x"), ("sudoku/sudoku", b"new"), ("sudoku/external.manifest.json", b"{}")]), manifest("v0.2.0", b"")),
            (b"not an archive".to_vec(), manifest("v0.2.0", b"")),
        ];
        for (i, (a, mut m)) in cases.into_iter().enumerate() {
            if m.sha256 == sha256_hex(b"") {
                m.sha256 = sha256_hex(&a);
            }
            assert!(install(&a, &m, &d.join("update"), &app).is_err(), "case {i}");
            assert_eq!(std::fs::read(app.join("sudoku")).unwrap(), b"old", "case {i}");
            assert!(!d.join("evil").exists() && !d.join("appload/evil").exists(), "case {i}");
        }
        std::fs::remove_dir_all(&d).unwrap();
    }
}
