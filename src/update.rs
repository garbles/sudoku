//! Updates (REQUIREMENTS.md §9.2, INVARIANTS.md §16): the version of the app, the signed
//! `version.json` of the latest release, the update state and the installation of a release archive.
//!
//! This module does not use the network. The platform gets the files from GitHub and gives them to
//! `read_manifest` and `install` (platform/mod.rs). `Update` keeps the state for the app (app.rs).

use std::io::Read;
use std::path::{Component, Path, PathBuf};

use ed25519_dalek::{Signature, VerifyingKey};
use serde::Deserialize;
use sha2::{Digest, Sha256};

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
/// The files that the folder in the archive must have.
const REQUIRED_FILES: [&str; 2] = ["sudoku", "external.manifest.json"];

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

/// True for the name of a preview build: `pre-` and the first 7 to 40 hexadecimal digits of the commit
/// (release.yml).
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
    /// The ink patch `sudoku-penregion.qmd` of the release (owner decision 2026-11-01). Older releases
    /// do not have it.
    pub patch: Option<PatchRef>,
}

/// The download URL and the SHA-256 checksum of the ink patch of a release.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PatchRef {
    pub url: String,
    pub sha256: String,
}

/// The JSON format of `version.json` (AGENTS.md, owner approval): the patch fields are optional,
/// but a release gives the two or none of them. `read_manifest` makes a `Manifest` from it.
#[derive(Deserialize)]
struct VersionJson {
    version: String,
    url: String,
    sha256: String,
    #[serde(default)]
    patch_url: Option<String>,
    #[serde(default)]
    patch_sha256: Option<String>,
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
    let patch = match (raw.patch_url, raw.patch_sha256) {
        (None, None) => None,
        (Some(url), Some(sha256)) => {
            if !url.starts_with("https://") {
                return Err(format!("version.json: the patch URL {url:?} is not HTTPS"));
            }
            if !is_sha256(&sha256) {
                return Err(format!("version.json: {sha256:?} is not a SHA-256 checksum"));
            }
            Some(PatchRef { url, sha256 })
        }
        _ => return Err("version.json: patch_url and patch_sha256 must be together".to_string()),
    };
    Ok(Manifest { version: raw.version, url: raw.url, sha256: raw.sha256, patch })
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
    for f in REQUIRED_FILES {
        if !folder.join(f).is_file() {
            return Err(format!("the archive has no {ARCHIVE_DIR}/{f}"));
        }
    }
    set_executable(&folder.join("sudoku"))?;

    // The new folder goes next to the app folder. A rename is not possible across file systems: then
    // copy the files.
    if std::fs::rename(&folder, &new).is_err() {
        copy_files(&folder, &new)?;
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

/// The folder of the ink patch (REQUIREMENTS.md §14.4). qt-resource-rebuilder reads the patches here
/// when xochitl starts.
pub const PATCH_DIR: &str = "/home/root/xovi/exthome/qt-resource-rebuilder";
/// The file name of the ink patch in `PATCH_DIR` and in the release.
pub const PATCH_NAME: &str = "sudoku-penregion.qmd";
/// The stamp file next to the patch: the version that installed the patch and the SHA-256 checksum
/// of the patch, `{"version": "v<version>", "sha256": "<checksum>"}`. install.sh writes the same file.
const PATCH_STAMP: &str = "sudoku-penregion.json";

#[derive(Deserialize)]
struct Stamp {
    sha256: String,
}

/// The checksum in the stamp file of `dir`, or `None` if the file is missing or not correct. Then
/// the patch must be replaced (owner decision 2026-11-01).
fn stamp_sha256(dir: &Path) -> Option<String> {
    let bytes = std::fs::read(dir.join(PATCH_STAMP)).ok()?;
    serde_json::from_slice::<Stamp>(&bytes).ok().map(|s| s.sha256).filter(|s| is_sha256(s))
}

/// The text of the stamp file.
fn stamp_text(version: &str, sha256: &str) -> String {
    format!("{{\"version\": \"{version}\", \"sha256\": \"{sha256}\"}}\n")
}

/// Writes `bytes` to `dir/name` with one rename, so that a reader never gets a part of the file.
fn write_atomic(dir: &Path, name: &str, bytes: &[u8]) -> Result<(), String> {
    let tmp = dir.join(format!("{name}.new"));
    std::fs::write(&tmp, bytes).map_err(|e| format!("cannot write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, dir.join(name)).map_err(|e| format!("cannot rename {}: {e}", tmp.display()))
}

/// Installs the release of the manifest `m` (REQUIREMENTS.md §9.2): the archive in place of the
/// AppLoad folder `app_dir` (`install`), then the downloaded ink patch `patch_bytes` in `patch_dir`.
/// The app examines the checksums of the two downloads before it changes a file. It replaces the
/// patch only when the checksum is different from the checksum in the stamp file, or when the stamp
/// file is missing. Returns true if it replaced the patch: then the interface must start again
/// (xovi/start).
pub fn install_release(archive: &[u8], patch_bytes: Option<&[u8]>, m: &Manifest, staging: &Path, app_dir: &Path, patch_dir: &Path) -> Result<bool, String> {
    let patch = match (patch_bytes, &m.patch) {
        (_, None) => None,
        (None, Some(_)) => return Err("the patch was not downloaded".to_string()),
        (Some(bytes), Some(PatchRef { sha256: want, .. })) => {
            let sum = sha256_hex(bytes);
            if &sum != want {
                return Err(format!("the checksum of the patch is {sum}, not {want}"));
            }
            Some((bytes, want))
        }
    };
    install(archive, m, staging, app_dir)?;
    let Some((bytes, want)) = patch else { return Ok(false) };
    if stamp_sha256(patch_dir).as_ref() == Some(want) {
        return Ok(false);
    }
    std::fs::create_dir_all(patch_dir).map_err(|e| format!("cannot make {}: {e}", patch_dir.display()))?;
    write_atomic(patch_dir, PATCH_NAME, bytes)?;
    write_atomic(patch_dir, PATCH_STAMP, stamp_text(&m.version, want).as_bytes())?;
    Ok(true)
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

/// Copies the files of the folder `from` (no subfolders) to the new folder `to`.
fn copy_files(from: &Path, to: &Path) -> Result<(), String> {
    std::fs::create_dir(to).map_err(|e| format!("cannot make {}: {e}", to.display()))?;
    for entry in std::fs::read_dir(from).map_err(|e| format!("{}: {e}", from.display()))? {
        let entry = entry.map_err(|e| format!("{}: {e}", from.display()))?;
        let dest = to.join(entry.file_name());
        std::fs::copy(entry.path(), &dest).map_err(|e| format!("cannot copy to {}: {e}", dest.display()))?;
    }
    set_executable(&to.join("sudoku"))
}

/// The update state (INVARIANTS.md §16). The fields `checking` and `up_to_date_shown` of INVARIANTS.md
/// are in the variants, thus only their correct combinations can occur: a request is in progress only
/// in `Unknown`, `CheckFailed` and `UpToDate` (UPD-7, UPD-9), `Unknown` always has its request in
/// progress (UPD-8), and "Sudoku is up to date" shows only in `UpToDate` with no request (UPD-9).
/// The field `dialog` of INVARIANTS.md is in `PatchChange::Replaced`, thus the dialog "Restart the
/// interface?" can show only in `Installed { patch: true }` of INVARIANTS.md (UPD-10).
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
    Installed(PatchChange),
}

/// What an installation did to the ink patch (UPD-10). `Unchanged` is `Installed { patch: false }`
/// of INVARIANTS.md, and `Replaced` is `Installed { patch: true }`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PatchChange {
    /// The installation did not replace the ink patch: only Sudoku must start again.
    Unchanged,
    /// The installation replaced the ink patch, thus the interface must start again (xovi/start).
    /// `dialog`: the dialog "Restart the interface?" shows.
    Replaced { dialog: bool },
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
    /// "Restart interface": runs xovi/start after an update that replaced the ink patch (UPD-10).
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
    /// Download and install the archive (`Update::installed`).
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
}

impl Update {
    /// The state when the app opens (UPD-8): a stable or a preview build sends a request immediately. This is the
    /// only automatic request. After it, only the button "Check for updates" sends a request.
    pub fn new(current: &str) -> Update {
        let on = has_updates(current);
        Update {
            current: current.to_string(),
            state: if on { UpdateState::Unknown } else { UpdateState::Off },
            request: on.then_some(UpdateRequest::Check),
        }
    }

    /// The dialog "Restart the interface?" shows (UPD-10).
    pub fn dialog_shown(&self) -> bool {
        self.state == UpdateState::Installed(PatchChange::Replaced { dialog: true })
    }

    /// Closes the dialog "Restart the interface?" ("Later", or xovi/start failed). The row continues to
    /// show the button "Restart interface" (UPD-10).
    pub fn close_dialog(&mut self) {
        if let UpdateState::Installed(PatchChange::Replaced { dialog }) = &mut self.state {
            *dialog = false;
        }
    }

    /// The Settings panel has the update row (UPD-1).
    pub fn has_row(&self) -> bool {
        self.state != UpdateState::Off
    }

    /// The gear has the caution icon (UPD-3).
    pub fn caution(&self) -> bool {
        matches!(self.state, UpdateState::Available(_) | UpdateState::Installing(_) | UpdateState::Failed(..) | UpdateState::Installed(_))
    }

    /// The text of the update row, or `None` for a button (REQUIREMENTS.md §9.2).
    pub fn row_text(&self) -> Option<&'static str> {
        Some(match self.state {
            UpdateState::Off
            | UpdateState::CheckFailed { checking: false }
            | UpdateState::UpToDate(UpToDateRow::CheckButton)
            | UpdateState::Available(_)
            | UpdateState::Installed(PatchChange::Replaced { .. }) => return None,
            UpdateState::Unknown | UpdateState::CheckFailed { checking: true } | UpdateState::UpToDate(UpToDateRow::Checking) => "Looking for updates...",
            UpdateState::UpToDate(UpToDateRow::UpToDateText) => "Sudoku is up to date",
            UpdateState::Installing(_) => "Installing update...",
            UpdateState::Failed(..) => "Update failed",
            UpdateState::Installed(PatchChange::Unchanged) => "Restart Sudoku to update",
        })
    }

    /// The button of the update row, or `None` for a text (REQUIREMENTS.md §9.2).
    pub fn button(&self) -> Option<UpdateButton> {
        match &self.state {
            UpdateState::Available(m) => Some(UpdateButton::Install(format!("Update to {}", m.version))),
            UpdateState::Installed(PatchChange::Replaced { .. }) => Some(UpdateButton::Restart),
            UpdateState::CheckFailed { checking: false } | UpdateState::UpToDate(UpToDateRow::CheckButton) => Some(UpdateButton::Check),
            UpdateState::Off
            | UpdateState::Unknown
            | UpdateState::CheckFailed { checking: true }
            | UpdateState::UpToDate(UpToDateRow::Checking | UpToDateRow::UpToDateText)
            | UpdateState::Installing(_)
            | UpdateState::Failed(..)
            | UpdateState::Installed(PatchChange::Unchanged) => None,
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

    /// The Settings panel closed: the row shows the button "Check for updates" again (UPD-9).
    pub fn panel_closed(&mut self) {
        if self.state == UpdateState::UpToDate(UpToDateRow::UpToDateText) {
            self.state = UpdateState::UpToDate(UpToDateRow::CheckButton);
        }
    }

    /// The result of a request (UPD-2, UPD-7). `panel_open`: the Settings panel shows now. Then a
    /// result with no update shows "Sudoku is up to date" until the panel closes (UPD-9).
    pub fn checked(&mut self, result: Result<Manifest, String>, panel_open: bool) {
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
                    | UpdateState::Installed(_) => {}
                }
            }
        }
    }

    /// A tap on the button (UPD-5, UPD-9, UPD-10). "Update to v<version>" starts an installation, and
    /// "Check for updates" sends a request. "Restart interface" changes nothing here: the app asks the
    /// platform to run xovi/start (`App::restart_ui`). True if the row changed.
    pub fn tap(&mut self) -> bool {
        let (state, request) = match &self.state {
            UpdateState::Available(m) => (UpdateState::Installing(m.clone()), UpdateRequest::Install(m.clone())),
            UpdateState::CheckFailed { checking: false } => (UpdateState::CheckFailed { checking: true }, UpdateRequest::Check),
            UpdateState::UpToDate(UpToDateRow::CheckButton) => (UpdateState::UpToDate(UpToDateRow::Checking), UpdateRequest::Check),
            // "Restart interface": the app asks the platform for the restart.
            UpdateState::Installed(PatchChange::Replaced { .. }) => return false,
            // No button.
            UpdateState::Off
            | UpdateState::Unknown
            | UpdateState::CheckFailed { checking: true }
            | UpdateState::UpToDate(UpToDateRow::Checking | UpToDateRow::UpToDateText)
            | UpdateState::Installing(_)
            | UpdateState::Failed(..)
            | UpdateState::Installed(PatchChange::Unchanged) => return false,
        };
        self.state = state;
        self.request = Some(request);
        true
    }

    /// The result of an installation (UPD-6, UPD-10). `Ok(true)`: the installation replaced the ink
    /// patch. Then the dialog "Restart the interface?" shows.
    pub fn installed(&mut self, result: Result<bool, String>, now: u64) {
        if let UpdateState::Installing(m) = &self.state {
            self.state = match result {
                Ok(true) => UpdateState::Installed(PatchChange::Replaced { dialog: true }),
                Ok(false) => UpdateState::Installed(PatchChange::Unchanged),
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
        assert_eq!(m.patch, None, "a release with no patch");
        let z = "0".repeat(64);
        let with_patch = format!(r#"{{"version": "v0.2.0", "url": "https://x/y.tar.gz", "sha256": "{z}", "patch_url": "https://x/p.qmd", "patch_sha256": "{z}"}}"#);
        let m = read_manifest(with_patch.as_bytes(), &key.sign(with_patch.as_bytes()).to_bytes(), &public).unwrap();
        assert_eq!(m.patch, Some(PatchRef { url: "https://x/p.qmd".to_string(), sha256: z.clone() }));
        for bad in [
            format!(r#"{{"version": "v0.2.0", "url": "https://x/y.tar.gz", "sha256": "{z}", "patch_url": "https://x/p.qmd"}}"#),
            format!(r#"{{"version": "v0.2.0", "url": "https://x/y.tar.gz", "sha256": "{z}", "patch_sha256": "{z}"}}"#),
            format!(r#"{{"version": "v0.2.0", "url": "https://x/y.tar.gz", "sha256": "{z}", "patch_url": "http://x/p.qmd", "patch_sha256": "{z}"}}"#),
            format!(r#"{{"version": "v0.2.0", "url": "https://x/y.tar.gz", "sha256": "{z}", "patch_url": "https://x/p.qmd", "patch_sha256": "00"}}"#),
        ] {
            assert!(read_manifest(bad.as_bytes(), &key.sign(bad.as_bytes()).to_bytes(), &public).is_err(), "{bad}");
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
        u.installed(Err("checksum".into()), 100);
        assert_eq!(u.row_text(), Some("Update failed"));
        assert!(!u.tick(100 + FAILED_MS - 1));
        assert!(u.tick(100 + FAILED_MS));
        assert!(u.tap());
        assert!(matches!(u.request.take(), Some(UpdateRequest::Install(_))));
        u.installed(Ok(false), 200);
        assert_eq!(u.row_text(), Some("Restart Sudoku to update"));
        assert!(!u.dialog_shown(), "no dialog when the patch did not change (UPD-10)");
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

    #[test]
    fn a_changed_patch_replaces_the_patch_and_the_stamp() {
        // UPD-4, UPD-10
        let d = temp_dir("patch");
        let (app, patches) = (d.join("appload/sudoku"), d.join("patches"));
        let a = archive(&[("sudoku/sudoku", b"new"), ("sudoku/external.manifest.json", b"{}")]);
        let mut m = manifest("v0.2.0", &a);
        m.patch = Some(PatchRef { url: "https://example.com/p.qmd".to_string(), sha256: sha256_hex(b"patch 2") });
        // No stamp file: the patch is replaced, also when the file has the same contents.
        std::fs::create_dir_all(&patches).unwrap();
        std::fs::write(patches.join(PATCH_NAME), b"patch 2").unwrap();
        assert_eq!(install_release(&a, Some(b"patch 2"), &m, &d.join("update"), &app, &patches), Ok(true));
        assert_eq!(std::fs::read(patches.join(PATCH_NAME)).unwrap(), b"patch 2");
        assert_eq!(std::fs::read_to_string(patches.join(PATCH_STAMP)).unwrap(), stamp_text("v0.2.0", &sha256_hex(b"patch 2")));
        assert_eq!(stamp_sha256(&patches), m.patch.as_ref().map(|p| p.sha256.clone()));
        // The same checksum as in the stamp: the patch does not change.
        std::fs::write(patches.join(PATCH_NAME), b"changed by hand").unwrap();
        assert_eq!(install_release(&a, Some(b"patch 2"), &m, &d.join("update"), &app, &patches), Ok(false));
        assert_eq!(std::fs::read(patches.join(PATCH_NAME)).unwrap(), b"changed by hand", "the stamp decides");
        // A different checksum: the patch is replaced.
        m.patch = Some(PatchRef { url: "https://example.com/p.qmd".to_string(), sha256: sha256_hex(b"patch 3") });
        assert_eq!(install_release(&a, Some(b"patch 3"), &m, &d.join("update"), &app, &patches), Ok(true));
        assert_eq!(std::fs::read(patches.join(PATCH_NAME)).unwrap(), b"patch 3");
        // A release with no patch does not change the patch.
        let plain = manifest("v0.3.0", &a);
        assert_eq!(install_release(&a, None, &plain, &d.join("update"), &app, &patches), Ok(false));
        assert_eq!(std::fs::read(patches.join(PATCH_NAME)).unwrap(), b"patch 3");
        let mut left: Vec<_> = std::fs::read_dir(&patches).unwrap().map(|e| e.unwrap().file_name()).collect();
        left.sort();
        assert_eq!(left, [PATCH_STAMP, PATCH_NAME], "no .new files");
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn a_bad_patch_changes_nothing() {
        // UPD-4, UPD-6
        let d = temp_dir("bad-patch");
        let (app, patches) = (d.join("appload/sudoku"), d.join("patches"));
        let a = archive(&[("sudoku/sudoku", b"new"), ("sudoku/external.manifest.json", b"{}")]);
        let mut m = manifest("v0.2.0", &a);
        m.patch = Some(PatchRef { url: "https://example.com/p.qmd".to_string(), sha256: sha256_hex(b"patch") });
        assert!(install_release(&a, Some(b"other"), &m, &d.join("update"), &app, &patches).is_err());
        assert!(install_release(&a, None, &m, &d.join("update"), &app, &patches).is_err());
        assert_eq!(std::fs::read(app.join("sudoku")).unwrap(), b"old");
        assert!(!patches.exists());
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn the_patch_dialog_shows_only_after_a_patch_change() {
        // UPD-10
        let mut u = Update::new("v0.1.0");
        u.checked(Ok(manifest("v0.2.0", b"x")), true);
        assert!(u.tap());
        u.request = None;
        u.installed(Ok(true), 100);
        assert_eq!(u.state, UpdateState::Installed(PatchChange::Replaced { dialog: true }));
        assert_eq!((u.row_text(), u.button()), (None, Some(UpdateButton::Restart)));
        assert_eq!(UpdateButton::Restart.text(), "Restart interface");
        assert!(u.dialog_shown());
        assert!(!u.tap(), "the restart is work of the app, not of the update state");
        assert_eq!(u.request, None);
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
