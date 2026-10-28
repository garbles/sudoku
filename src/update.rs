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

/// The time between two requests after a successful request.
pub const CHECK_INTERVAL_MS: u64 = 24 * 60 * 60 * 1000;
/// The time before the next request after a failed request.
pub const RETRY_INTERVAL_MS: u64 = 60 * 60 * 1000;
/// The time that "Update failed" shows (UPD-6).
pub const FAILED_MS: u64 = 3000;
/// The largest archive that the app downloads.
pub const MAX_ARCHIVE_BYTES: u64 = 64 << 20;

/// The folder in the release archive. It is also the name of the AppLoad folder of the app.
const ARCHIVE_DIR: &str = "sudoku";
/// The files that the folder in the archive must have.
const REQUIRED_FILES: [&str; 2] = ["sudoku", "external.manifest.json"];

/// A stable version: `v<major>.<minor>.<patch>`. The order of the fields is the order of the versions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version(pub u32, pub u32, pub u32);

/// The stable version in a text, or `None` for `pre-<sha>`, `dev` and all other texts.
pub fn parse_version(s: &str) -> Option<Version> {
    let mut parts = s.strip_prefix('v')?.split('.');
    let mut next = || -> Option<u32> {
        let p = parts.next()?;
        (!p.is_empty() && p.bytes().all(|b| b.is_ascii_digit())).then(|| p.parse().ok())?
    };
    let v = Version(next()?, next()?, next()?);
    parts.next().is_none().then_some(v)
}

/// The contents of `version.json`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize)]
pub struct Manifest {
    pub version: String,
    pub url: String,
    pub sha256: String,
}

/// The manifest in `json`, if `signature` is the ed25519 signature of these bytes with `key` (UPD-4).
pub fn read_manifest(json: &[u8], signature: &[u8], key: &[u8; 32]) -> Result<Manifest, String> {
    let key = VerifyingKey::from_bytes(key).map_err(|e| format!("public key: {e}"))?;
    let signature: [u8; 64] = signature.try_into().map_err(|_| format!("the signature has {} bytes, not 64", signature.len()))?;
    key.verify_strict(json, &Signature::from_bytes(&signature)).map_err(|_| "the signature of version.json is not correct".to_string())?;
    let m: Manifest = serde_json::from_slice(json).map_err(|e| format!("version.json: {e}"))?;
    if parse_version(&m.version).is_none() {
        return Err(format!("version.json: {:?} is not a stable version", m.version));
    }
    if !m.url.starts_with("https://") {
        return Err(format!("version.json: the URL {:?} is not HTTPS", m.url));
    }
    if m.sha256.len() != 64 || !m.sha256.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
        return Err(format!("version.json: {:?} is not a SHA-256 checksum", m.sha256));
    }
    Ok(m)
}

/// The SHA-256 checksum of `bytes` in lowercase hexadecimal.
pub fn sha256_hex(bytes: &[u8]) -> String {
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
    let extract = staging.join("extract");
    std::fs::create_dir_all(&extract).map_err(|e| format!("cannot make {}: {e}", extract.display()))?;
    unpack(archive, &extract)?;
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

/// Extracts the gzip tar archive in `dest`. Each entry must be a file or a folder in `sudoku/`.
fn unpack(archive: &[u8], dest: &Path) -> Result<(), String> {
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

/// Reads at most `MAX_ARCHIVE_BYTES` from `r`.
pub fn read_limited(r: impl Read) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    r.take(MAX_ARCHIVE_BYTES + 1).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_ARCHIVE_BYTES {
        return Err("the download is too large".to_string());
    }
    Ok(bytes)
}

/// The update state (INVARIANTS.md §16).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum UpdateState {
    /// Not a stable build: no requests and no update row (UPD-1).
    Off,
    /// The first request is not complete.
    Unknown,
    /// All requests failed.
    CheckFailed,
    UpToDate,
    Available(Manifest),
    Installing(Manifest),
    /// The installation failed at this time (ms). After `FAILED_MS`, the button shows again.
    Failed(Manifest, u64),
    /// The new version operates after the next start.
    Installed,
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
    /// A request for `version.json` is in progress (UPD-7).
    pub checking: bool,
    /// The time (ms) of the next request.
    next_check: Option<u64>,
    /// Work that the platform must start.
    pub request: Option<UpdateRequest>,
}

impl Update {
    /// The state when the app opens (UPD-8): a stable build sends a request immediately.
    pub fn new(current: &str) -> Update {
        let stable = parse_version(current).is_some();
        Update {
            current: current.to_string(),
            state: if stable { UpdateState::Unknown } else { UpdateState::Off },
            checking: stable,
            next_check: None,
            request: stable.then_some(UpdateRequest::Check),
        }
    }

    /// The Settings panel has the update row (UPD-1).
    pub fn has_row(&self) -> bool {
        self.state != UpdateState::Off
    }

    /// The gear has the caution icon (UPD-3).
    pub fn caution(&self) -> bool {
        matches!(self.state, UpdateState::Available(_) | UpdateState::Installing(_) | UpdateState::Failed(..) | UpdateState::Installed)
    }

    /// The text of the update row, or `None` for the button (REQUIREMENTS.md §9.2).
    pub fn row_text(&self) -> Option<&'static str> {
        Some(match self.state {
            UpdateState::Off | UpdateState::Available(_) => return None,
            UpdateState::Unknown => "Looking for updates...",
            UpdateState::CheckFailed => "Cannot look for updates",
            UpdateState::UpToDate => "Sudoku is up to date",
            UpdateState::Installing(_) => "Installing update...",
            UpdateState::Failed(..) => "Update failed",
            UpdateState::Installed => "Restart to apply update",
        })
    }

    /// The text of the button.
    pub fn button_text(&self) -> Option<String> {
        match &self.state {
            UpdateState::Available(m) => Some(format!("Update to {}", m.version)),
            _ => None,
        }
    }

    fn start_check(&mut self) {
        if !self.checking && self.state != UpdateState::Off {
            self.checking = true;
            self.next_check = None;
            self.request = Some(UpdateRequest::Check);
        }
    }

    /// Moves the timers to `now` (ms). True if the row changed.
    pub fn tick(&mut self, now: u64) -> bool {
        if self.next_check.is_some_and(|t| now >= t) && !matches!(self.state, UpdateState::Installing(_) | UpdateState::Installed) {
            self.start_check();
        }
        if let UpdateState::Failed(m, at) = &self.state {
            if now >= at + FAILED_MS {
                self.state = UpdateState::Available(m.clone());
                return true;
            }
        }
        false
    }

    /// The Settings panel opens: if all requests failed, try again now.
    pub fn panel_opened(&mut self) {
        if self.state == UpdateState::CheckFailed {
            self.start_check();
        }
    }

    /// The result of a request (UPD-2, UPD-7).
    pub fn checked(&mut self, result: Result<Manifest, String>, now: u64) {
        self.checking = false;
        let current = parse_version(&self.current);
        let can_change = matches!(self.state, UpdateState::Unknown | UpdateState::CheckFailed | UpdateState::UpToDate | UpdateState::Available(_));
        match result {
            Ok(m) => {
                self.next_check = Some(now + CHECK_INTERVAL_MS);
                if can_change {
                    let newer = parse_version(&m.version).is_some_and(|v| current.is_some_and(|c| v > c));
                    self.state = if newer { UpdateState::Available(m) } else { UpdateState::UpToDate };
                }
            }
            Err(e) => {
                eprintln!("sudoku: cannot look for updates: {e}");
                self.next_check = Some(now + RETRY_INTERVAL_MS);
                if self.state == UpdateState::Unknown {
                    self.state = UpdateState::CheckFailed;
                }
            }
        }
    }

    /// A tap on the button (UPD-5). True if an installation starts.
    pub fn tap(&mut self) -> bool {
        if let UpdateState::Available(m) = &self.state {
            self.request = Some(UpdateRequest::Install(m.clone()));
            self.state = UpdateState::Installing(m.clone());
            return true;
        }
        false
    }

    /// The result of an installation (UPD-6).
    pub fn installed(&mut self, result: Result<(), String>, now: u64) {
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
        Manifest { version: version.to_string(), url: "https://example.com/a.tar.gz".to_string(), sha256: sha256_hex(archive) }
    }

    #[test]
    fn versions() {
        assert_eq!(parse_version("v0.1.0"), Some(Version(0, 1, 0)));
        assert_eq!(parse_version("v12.0.3"), Some(Version(12, 0, 3)));
        for s in ["dev", "pre-4118468", "0.1.0", "v0.1", "v0.1.0.1", "v0..1", "v0.1.x", "v+1.0.0", "v0.1.0-rc1", ""] {
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
    }

    #[test]
    fn the_public_key_is_valid() {
        assert!(VerifyingKey::from_bytes(&PUBLIC_KEY).is_ok());
    }

    #[test]
    fn only_stable_builds_look_for_updates() {
        // UPD-1, UPD-8
        for v in ["dev", "pre-4118468"] {
            let mut u = Update::new(v);
            assert_eq!(u.state, UpdateState::Off);
            assert_eq!(u.request, None);
            u.tick(u64::MAX / 2);
            u.panel_opened();
            assert_eq!(u.request, None);
            assert!(!u.has_row());
        }
        let u = Update::new("v0.1.0");
        assert_eq!((u.state.clone(), u.request.clone()), (UpdateState::Unknown, Some(UpdateRequest::Check)));
        assert!(u.has_row());
    }

    #[test]
    fn only_a_newer_version_is_an_update() {
        // UPD-2
        for (remote, want) in [("v0.1.0", false), ("v0.0.9", false), ("v0.1.1", true), ("v1.0.0", true)] {
            let mut u = Update::new("v0.1.0");
            u.checked(Ok(manifest(remote, b"")), 0);
            assert_eq!(matches!(u.state, UpdateState::Available(_)), want, "{remote}");
            assert_eq!(u.caution(), want);
        }
    }

    #[test]
    fn requests_repeat_and_a_failure_keeps_the_last_result() {
        // UPD-7
        let mut u = Update::new("v0.1.0");
        u.request = None;
        u.checked(Err("no network".into()), 1000);
        assert_eq!(u.state, UpdateState::CheckFailed);
        assert!(!u.tick(1000 + RETRY_INTERVAL_MS - 1));
        assert_eq!(u.request, None);
        u.panel_opened();
        assert_eq!(u.request.take(), Some(UpdateRequest::Check));
        u.panel_opened();
        assert_eq!(u.request, None, "one request at a time");
        u.checked(Ok(manifest("v0.2.0", b"")), 5000);
        assert!(matches!(u.state, UpdateState::Available(_)));
        u.tick(5000 + CHECK_INTERVAL_MS);
        assert_eq!(u.request.take(), Some(UpdateRequest::Check));
        u.checked(Err("no network".into()), 6000);
        assert!(matches!(u.state, UpdateState::Available(_)), "a failure keeps the button");
    }

    #[test]
    fn a_failed_installation_shows_the_button_again() {
        // UPD-5, UPD-6
        let mut u = Update::new("v0.1.0");
        u.request = None;
        assert!(!u.tap());
        u.checked(Ok(manifest("v0.2.0", b"x")), 0);
        assert_eq!(u.button_text().as_deref(), Some("Update to v0.2.0"));
        assert!(u.tap());
        assert!(matches!(u.request.take(), Some(UpdateRequest::Install(_))));
        assert!(!u.tap(), "one installation at a time");
        assert_eq!(u.row_text(), Some("Installing update..."));
        u.checked(Ok(manifest("v0.3.0", b"x")), 10);
        assert!(matches!(u.state, UpdateState::Installing(_)), "a request does not change an installation");
        u.installed(Err("checksum".into()), 100);
        assert_eq!(u.row_text(), Some("Update failed"));
        assert!(!u.tick(100 + FAILED_MS - 1));
        assert!(u.tick(100 + FAILED_MS));
        assert!(u.tap());
        assert!(matches!(u.request.take(), Some(UpdateRequest::Install(_))));
        u.installed(Ok(()), 200);
        assert_eq!(u.row_text(), Some("Restart to apply update"));
        assert!(u.caution());
        u.tick(200 + CHECK_INTERVAL_MS * 2);
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
