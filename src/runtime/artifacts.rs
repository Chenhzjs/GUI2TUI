//! Private, leased artifact namespaces. Crash recovery removes only explicitly
//! registered regular files of an unlocked, marked namespace. Never recursive
//! deletion, glob cleanup, PID guessing, or following symlinks.
use super::RuntimeSessionId;
use rustix::fs::{FlockOperation, OFlags, flock};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::{self, Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const MARKER: &str = "GUI2TUI-OWNED-ARTIFACTS-v1";
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Ownership {
    marker: String,
    session: String,
    operation: u64,
    created_unix: u64,
    expires_unix: u64,
    files: Vec<String>,
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn nofollow(path: &Path, write: bool) -> io::Result<File> {
    fs::OpenOptions::new()
        .read(true)
        .write(write)
        .custom_flags(OFlags::NOFOLLOW.bits() as i32)
        .open(path)
}
fn private_owned(path: &Path, directory: bool) -> io::Result<()> {
    let m = fs::symlink_metadata(path)?;
    if m.uid() != rustix::process::geteuid().as_raw()
        || m.mode() & 0o077 != 0
        || (directory && !m.is_dir())
        || (!directory && (!m.is_file() || m.nlink() != 1))
    {
        return Err(io::Error::other("not a private owned artifact path"));
    }
    Ok(())
}
fn root_in(base: &Path) -> io::Result<PathBuf> {
    let root = base.join(format!(
        "gui2tui-owned-{}",
        rustix::process::geteuid().as_raw()
    ));
    match fs::DirBuilder::new().mode(0o700).create(&root) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e),
    }
    private_owned(&root, true)?;
    Ok(root)
}
use std::os::unix::fs::DirBuilderExt;

pub struct OwnedArtifactDirectory {
    directory: tempfile::TempDir,
    _lease: File,
    ownership: Ownership,
}
impl OwnedArtifactDirectory {
    pub fn new(ttl_seconds: u64) -> io::Result<Self> {
        Self::new_in(&std::env::temp_dir(), ttl_seconds)
    }
    pub fn new_in(base: &Path, ttl_seconds: u64) -> io::Result<Self> {
        let root = root_in(base)?;
        Self::in_root(&root, ttl_seconds)
    }
    fn in_root(root: &Path, ttl_seconds: u64) -> io::Result<Self> {
        private_owned(root, true)?;
        let directory = tempfile::Builder::new()
            .prefix("operation-")
            .permissions(fs::Permissions::from_mode(0o700))
            .tempdir_in(root)?;
        let lease = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(directory.path().join("lease"))?;
        flock(&lease, FlockOperation::NonBlockingLockExclusive)?;
        let ownership = Ownership {
            marker: MARKER.into(),
            session: serde_json::to_string(&RuntimeSessionId::default())?,
            operation: 1,
            created_unix: now(),
            expires_unix: now().saturating_add(ttl_seconds.min(1800)),
            files: Vec::new(),
        };
        let this = Self {
            directory,
            _lease: lease,
            ownership,
        };
        this.persist()?;
        Ok(this)
    }
    pub fn path(&self) -> &Path {
        self.directory.path()
    }
    pub fn create_file(&mut self, suffix: &str) -> io::Result<tempfile::NamedTempFile> {
        self.ownership
            .files
            .retain(|name| self.directory.path().join(name).exists());
        if self.ownership.files.len() >= 256 {
            return Err(io::Error::other("artifact namespace file limit"));
        }
        if !suffix
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.')
        {
            return Err(io::Error::other("invalid artifact suffix"));
        }
        let file = tempfile::Builder::new()
            .prefix("artifact-")
            .suffix(suffix)
            .tempfile_in(self.path())?;
        self.ownership.files.push(
            file.path()
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
        );
        self.persist()?;
        Ok(file)
    }
    fn persist(&self) -> io::Result<()> {
        let mut tmp = tempfile::NamedTempFile::new_in(self.path())?;
        serde_json::to_writer(&mut tmp, &self.ownership)?;
        tmp.flush()?;
        tmp.persist(self.path().join("ownership.json"))
            .map_err(|e| e.error)?;
        Ok(())
    }
}

/// Run once at startup. Active leases are always skipped, even past TTL.
/// A crashed namespace is reclaimed on the next startup, not by a scan loop.
pub fn recover_abandoned() -> io::Result<usize> {
    recover_in(&root_in(&std::env::temp_dir())?)
}
fn recover_in(root: &Path) -> io::Result<usize> {
    private_owned(root, true)?;
    let mut recovered = 0;
    for entry in fs::read_dir(root)?.take(4096) {
        let entry = entry?;
        if !entry
            .file_name()
            .to_string_lossy()
            .starts_with("operation-")
        {
            continue;
        }
        if recover_one(&entry.path()).unwrap_or(false) {
            recovered += 1;
        }
    }
    Ok(recovered)
}
fn recover_one(directory: &Path) -> io::Result<bool> {
    private_owned(directory, true)?;
    let lease_path = directory.join("lease");
    private_owned(&lease_path, false)?;
    let lease = nofollow(&lease_path, true)?;
    if flock(&lease, FlockOperation::NonBlockingLockExclusive).is_err() {
        return Ok(false);
    }
    let manifest_path = directory.join("ownership.json");
    private_owned(&manifest_path, false)?;
    let ownership: Ownership =
        serde_json::from_reader(nofollow(&manifest_path, false)?.take(65536))?;
    if ownership.marker != MARKER
        || ownership.files.len() > 256
        || ownership.created_unix > ownership.expires_unix
    {
        return Err(io::Error::other("invalid artifact ownership"));
    }
    // Validate ALL entries before deleting ANY entry. Foreign additions,
    // links and unregistered crash-gap files make recovery conservative.
    let mut present = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if !matches!(name.as_str(), "lease" | "ownership.json") && !ownership.files.contains(&name)
        {
            return Err(io::Error::other("unregistered file in artifact namespace"));
        }
        private_owned(&entry.path(), false)?;
        present.push(entry.path());
    }
    for path in present {
        fs::remove_file(path)?;
    }
    fs::remove_dir(directory)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn test_root() -> tempfile::TempDir {
        tempfile::Builder::new()
            .permissions(fs::Permissions::from_mode(0o700))
            .tempdir()
            .unwrap()
    }
    #[test]
    fn live_sessions_are_isolated_and_crash_residue_recovered() {
        let root = test_root();
        let active = OwnedArtifactDirectory::in_root(root.path(), 300).unwrap();
        let mut crashed = OwnedArtifactDirectory::in_root(root.path(), 300).unwrap();
        let file = crashed.create_file(".png").unwrap();
        let _ = file.keep().unwrap();
        assert_eq!(recover_in(root.path()).unwrap(), 0);
        let OwnedArtifactDirectory {
            directory, _lease, ..
        } = crashed;
        let path = directory.keep();
        drop(_lease); // equivalent to process lease loss
        assert_eq!(recover_in(root.path()).unwrap(), 1);
        assert!(!path.exists());
        assert!(active.path().exists());
    }
    #[test]
    fn foreign_file_and_symlink_are_not_removed() {
        let root = test_root();
        let crashed = OwnedArtifactDirectory::in_root(root.path(), 300).unwrap();
        let OwnedArtifactDirectory {
            directory, _lease, ..
        } = crashed;
        let path = directory.keep();
        drop(_lease);
        fs::write(path.join("foreign"), b"not ours").unwrap();
        assert_eq!(recover_in(root.path()).unwrap(), 0);
        assert!(path.join("foreign").exists());
        std::os::unix::fs::symlink(&path, root.path().join("operation-link")).unwrap();
        assert_eq!(recover_in(root.path()).unwrap(), 0);
    }
}
