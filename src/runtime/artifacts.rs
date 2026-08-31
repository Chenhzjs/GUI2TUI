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
    sync::atomic::{AtomicU64, Ordering},
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
        Self::new_in(&crate::product::paths::runtime_dir()?, ttl_seconds)
    }
    pub fn new_in(base: &Path, ttl_seconds: u64) -> io::Result<Self> {
        Self::new_owned_in(base, ttl_seconds, RuntimeSessionId::default(), 1)
    }
    pub fn new_owned(
        ttl_seconds: u64,
        session: RuntimeSessionId,
        operation: u64,
    ) -> io::Result<Self> {
        Self::new_owned_in(
            &crate::product::paths::runtime_dir()?,
            ttl_seconds,
            session,
            operation,
        )
    }
    pub fn new_owned_in(
        base: &Path,
        ttl_seconds: u64,
        session: RuntimeSessionId,
        operation: u64,
    ) -> io::Result<Self> {
        let root = root_in(base)?;
        Self::in_root(&root, ttl_seconds, session, operation)
    }
    fn in_root(
        root: &Path,
        ttl_seconds: u64,
        session: RuntimeSessionId,
        operation: u64,
    ) -> io::Result<Self> {
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
        // Shared live leases allow a TTL reaper to take ownership before the
        // producer releases its lease. Recovery always requires exclusive access.
        flock(&lease, FlockOperation::NonBlockingLockShared)?;
        let ownership = Ownership {
            marker: MARKER.into(),
            session: serde_json::to_string(&session)?,
            operation,
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
    pub fn create_file(&mut self, suffix: &str) -> io::Result<OwnedArtifactFile> {
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
        static NEXT_FILE: AtomicU64 = AtomicU64::new(1);
        let name = format!(
            "artifact-{}-{}{}",
            std::process::id(),
            NEXT_FILE.fetch_add(1, Ordering::Relaxed),
            suffix
        );
        // Ownership is durable before the filesystem entry and, critically,
        // before any payload byte. A crash can therefore never leave an
        // unprovable partial artifact in an otherwise valid namespace.
        self.ownership.files.push(name.clone());
        self.persist()?;
        debug_crash_failpoint("B");
        let path = self.path().join(name);
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)?;
        Ok(OwnedArtifactFile {
            file,
            path,
            remove_on_drop: true,
        })
    }
    fn persist(&self) -> io::Result<()> {
        let pending = self.path().join("ownership.pending");
        let mut tmp = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&pending)?;
        serde_json::to_writer(&mut tmp, &self.ownership)?;
        tmp.flush()?;
        tmp.sync_all()?;
        fs::rename(pending, self.path().join("ownership.json"))?;
        File::open(self.path())?.sync_all()?;
        Ok(())
    }

    pub(crate) fn reaper_lease(directory: &Path) -> io::Result<File> {
        private_owned(directory, true)?;
        private_owned(&directory.join("lease"), false)?;
        let lease = nofollow(&directory.join("lease"), true)?;
        flock(&lease, FlockOperation::NonBlockingLockShared)?;
        private_owned(&directory.join("ownership.json"), false)?;
        let ownership: Ownership = serde_json::from_reader(
            nofollow(&directory.join("ownership.json"), false)?.take(65536),
        )?;
        if ownership.marker != MARKER {
            return Err(io::Error::other("invalid artifact ownership"));
        }
        Ok(lease)
    }

    pub fn keep(self) -> PathBuf {
        let Self {
            directory, _lease, ..
        } = self;
        let path = directory.keep();
        drop(_lease);
        path
    }
}

pub struct OwnedArtifactFile {
    file: File,
    path: PathBuf,
    remove_on_drop: bool,
}
impl OwnedArtifactFile {
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn keep(mut self) -> PathBuf {
        self.remove_on_drop = false;
        self.path.clone()
    }
    pub fn as_file(&self) -> &File {
        &self.file
    }
    pub fn as_file_mut(&mut self) -> &mut File {
        &mut self.file
    }
}
impl Write for OwnedArtifactFile {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.file.write(bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}
impl Drop for OwnedArtifactFile {
    fn drop(&mut self) {
        if self.remove_on_drop {
            let _ = fs::remove_file(&self.path);
        }
    }
}

#[cfg(debug_assertions)]
pub(crate) fn debug_crash_failpoint(stage: &str) {
    if std::env::var("GUI2TUI_MATERIALIZER_CRASH_STAGE").as_deref() == Ok(stage) {
        // `process::exit` deliberately skips Rust destructors, accurately
        // modelling an independently killed materializer without producing a
        // platform crash report during the failpoint suite.
        std::process::exit(86);
    }
}
#[cfg(not(debug_assertions))]
pub(crate) fn debug_crash_failpoint(_: &str) {}

/// Run once at startup. Active leases are always skipped, even past TTL.
/// A crashed namespace is reclaimed on the next startup, not by a scan loop.
pub fn recover_abandoned() -> io::Result<usize> {
    let count = recover_in(&root_in(&crate::product::paths::runtime_dir()?)?)?;
    // Upgrade compatibility: inspect only the exact previous owned namespace,
    // with the same marker/lease/UID rules. Never scan arbitrary temp entries.
    let legacy = std::env::temp_dir().join(format!(
        "gui2tui-owned-{}",
        rustix::process::geteuid().as_raw()
    ));
    if legacy.exists() {
        Ok(count + recover_in(&legacy)?)
    } else {
        Ok(count)
    }
}

/// Read-only, contents-free diagnostics. No payload/manifest bodies, no cleanup.
/// The lease count is a point-in-time observation, not permission to scavenge.
pub fn health_counts(base: &Path) -> io::Result<(usize, usize)> {
    let root = base.join(format!(
        "gui2tui-owned-{}",
        rustix::process::geteuid().as_raw()
    ));
    if !root.exists() {
        return Ok((0, 0));
    }
    private_owned(&root, true)?;
    let mut namespaces = 0;
    let mut leased = 0;
    for entry in fs::read_dir(root)?.take(4096) {
        let path = entry?.path();
        private_owned(&path, true)?;
        private_owned(&path.join("lease"), false)?;
        let lease = nofollow(&path.join("lease"), true)?;
        if flock(&lease, FlockOperation::NonBlockingLockExclusive).is_err() {
            leased += 1;
        }
        namespaces += 1;
    }
    Ok((namespaces, leased))
}
#[cfg(test)]
pub(crate) fn recover_abandoned_in(base: &Path) -> io::Result<usize> {
    recover_in(&root_in(base)?)
}
pub(crate) fn recover_owned_directory(directory: &Path) -> io::Result<bool> {
    recover_one(directory)
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
        if !matches!(
            name.as_str(),
            "lease" | "ownership.json" | "ownership.pending"
        ) && !ownership.files.contains(&name)
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
        let active =
            OwnedArtifactDirectory::in_root(root.path(), 300, RuntimeSessionId::default(), 1)
                .unwrap();
        let mut crashed =
            OwnedArtifactDirectory::in_root(root.path(), 300, RuntimeSessionId::default(), 2)
                .unwrap();
        let file = crashed.create_file(".png").unwrap();
        let _ = file.keep();
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
        let crashed =
            OwnedArtifactDirectory::in_root(root.path(), 300, RuntimeSessionId::default(), 1)
                .unwrap();
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

    #[test]
    fn hardlinked_registered_file_is_not_removed() {
        let root = test_root();
        let mut crashed =
            OwnedArtifactDirectory::in_root(root.path(), 300, RuntimeSessionId::default(), 1)
                .unwrap();
        let file = crashed.create_file(".png").unwrap();
        let file_path = file.keep();
        let outside = root.path().join("outside-hardlink");
        fs::hard_link(&file_path, &outside).unwrap();
        let OwnedArtifactDirectory {
            directory, _lease, ..
        } = crashed;
        let path = directory.keep();
        drop(_lease);
        assert_eq!(recover_in(root.path()).unwrap(), 0);
        assert!(path.exists());
        assert!(outside.exists());
    }

    #[test]
    fn ttl_reaper_lease_protects_handoff_and_pending_manifest_is_recovered() {
        let root = test_root();
        let owned = OwnedArtifactDirectory::new_in(root.path(), 300).unwrap();
        let path = owned.path().to_path_buf();
        let reaper = OwnedArtifactDirectory::reaper_lease(&path).unwrap();
        let _ = owned.keep();
        assert_eq!(recover_abandoned_in(root.path()).unwrap(), 0);
        let mut pending = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path.join("ownership.pending"))
            .unwrap();
        pending.write_all(b"incomplete metadata update").unwrap();
        drop(pending);
        drop(reaper);
        assert_eq!(recover_abandoned_in(root.path()).unwrap(), 1);
        assert!(!path.exists());
    }
}
