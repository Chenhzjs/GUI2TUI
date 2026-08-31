//! Host-local storage, deliberately independent of broker and transport.
use super::{
    ArtifactDescriptor, ArtifactHash, ArtifactOrigin, CancellationToken,
    acquisition::{CaptureQuality, MAX_ARTIFACT_BYTES, ScreenRegion},
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, BufRead, Read, Write},
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterializationMetadata {
    pub ownership_marker: String,
    pub session_id: crate::runtime::RuntimeSessionId,
    pub operation_id: u64,
    pub created_unix: u64,
    pub descriptor: ArtifactDescriptor,
    pub region: Option<ScreenRegion>,
    pub quality: Option<CaptureQuality>,
    pub expires_unix: u64,
    pub filename: String,
}

pub struct MaterializedArtifact {
    directory: crate::runtime::artifacts::OwnedArtifactDirectory,
    pub metadata: MaterializationMetadata,
}

impl MaterializedArtifact {
    pub fn path(&self) -> PathBuf {
        self.directory.path().join(&self.metadata.filename)
    }
    pub fn expired(&self) -> bool {
        unix_now() >= self.metadata.expires_unix
    }

    /// The inspector starts its own restricted TTL reaper, not a viewer endpoint.
    /// Failure to start the reaper leaves RAII cleanup enabled.
    pub fn detach_with_reaper(self, inspector: &Path) -> io::Result<PathBuf> {
        let path = self.path();
        let mut child = std::process::Command::new(inspector)
            .arg("--reap-materialized")
            .arg(self.directory.path())
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("reaper pipe missing"))?;
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        let reader = std::thread::spawn(move || {
            let mut ready = String::new();
            let valid = io::BufReader::new(stdout).read_line(&mut ready).is_ok()
                && ready.trim() == "OWNED_ARTIFACT_LEASE_READY";
            let _ = sender.send(valid);
        });
        if receiver.recv_timeout(Duration::from_secs(5)) != Ok(true) {
            let _ = child.kill();
            let _ = child.wait();
            let _ = reader.join();
            return Err(io::Error::other(
                "artifact reaper did not acquire ownership",
            ));
        }
        let _ = reader.join();
        let _ = self.directory.keep();
        Ok(path)
    }
}

pub struct ArtifactMaterializer;
impl ArtifactMaterializer {
    pub fn materialize(
        descriptor: ArtifactDescriptor,
        source: impl Read,
        snapshot: Option<(ScreenRegion, CaptureQuality)>,
        ttl: Duration,
        explicit: bool,
        cancel: &CancellationToken,
    ) -> io::Result<MaterializedArtifact> {
        Self::materialize_owned(descriptor, source, snapshot, ttl, explicit, cancel, None)
    }

    pub fn materialize_owned(
        mut descriptor: ArtifactDescriptor,
        mut source: impl Read,
        snapshot: Option<(ScreenRegion, CaptureQuality)>,
        ttl: Duration,
        explicit: bool,
        cancel: &CancellationToken,
        owner: Option<(crate::runtime::RuntimeSessionId, u64)>,
    ) -> io::Result<MaterializedArtifact> {
        if !explicit || cancel.is_cancelled() {
            return Err(io::Error::other(
                "explicit materialization required; request cancelled or absent",
            ));
        }
        if descriptor.size > MAX_ARTIFACT_BYTES || !(1..=1800).contains(&ttl.as_secs()) {
            return Err(io::Error::other(
                "artifact size or TTL exceeds materialization limit",
            ));
        }
        if (descriptor.origin == ArtifactOrigin::RenderedSnapshot) != snapshot.is_some() {
            return Err(io::Error::other(
                "artifact origin and snapshot provenance disagree",
            ));
        }
        let ttl = match descriptor.lifetime {
            super::ArtifactLifetime::Temporary { ttl: source_ttl } => ttl.min(source_ttl),
            super::ArtifactLifetime::Session => ttl,
        };
        descriptor.lifetime = super::ArtifactLifetime::Temporary { ttl };
        if ttl.is_zero() {
            return Err(io::Error::other("artifact lifetime has expired"));
        }
        let extension = match descriptor.mime.as_str() {
            "image/png" => "png",
            "image/jpeg" => "jpg",
            "image/svg+xml" => "svg",
            "application/pdf" => "pdf",
            _ => return Err(io::Error::other("materialization MIME not supported")),
        };
        let (session, operation) =
            owner.unwrap_or_else(|| (crate::runtime::RuntimeSessionId::default(), 1));
        let test_base = if cfg!(debug_assertions) {
            std::env::var_os("GUI2TUI_ARTIFACT_TEST_BASE").map(PathBuf::from)
        } else {
            None
        };
        let mut directory = if let Some(base) = test_base {
            crate::runtime::artifacts::OwnedArtifactDirectory::new_owned_in(
                &base,
                ttl.as_secs(),
                session.clone(),
                operation,
            )?
        } else {
            crate::runtime::artifacts::OwnedArtifactDirectory::new_owned(
                ttl.as_secs(),
                session.clone(),
                operation,
            )?
        };
        crate::runtime::artifacts::debug_crash_failpoint("A");
        let mut file = directory.create_file(&format!(".{extension}"))?;
        crate::runtime::artifacts::debug_crash_failpoint("C");
        let filename = file
            .path()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let mut hash = sha2::Sha256::new();
        use sha2::Digest;
        let mut count = 0;
        let mut buffer = [0u8; 65536];
        loop {
            if cancel.is_cancelled() {
                return Err(io::Error::other("materialization cancelled"));
            }
            let n = source.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            count += n as u64;
            if count > descriptor.size || count > MAX_ARTIFACT_BYTES {
                return Err(io::Error::other("artifact length exceeds limit"));
            }
            hash.update(&buffer[..n]);
            file.write_all(&buffer[..n])?;
            crate::runtime::artifacts::debug_crash_failpoint("D");
        }
        if count != descriptor.size || ArtifactHash(hash.finalize().into()) != descriptor.hash {
            return Err(io::Error::other("materialization size/hash mismatch"));
        }
        if cancel.is_cancelled() {
            return Err(io::Error::other("materialization cancelled"));
        }
        let _ = file.keep();
        crate::runtime::artifacts::debug_crash_failpoint("E");
        let metadata = MaterializationMetadata {
            ownership_marker: "GUI2TUI-MATERIALIZATION-v1".into(),
            session_id: session,
            operation_id: operation,
            created_unix: unix_now(),
            descriptor,
            region: snapshot.map(|x| x.0),
            quality: snapshot.map(|x| x.1),
            expires_unix: unix_now() + ttl.as_secs(),
            filename,
        };
        let mut manifest = directory.create_file(".json")?;
        serde_json::to_writer(&mut manifest, &metadata)?;
        manifest.flush()?;
        let _ = manifest.keep();
        let mut complete = directory.create_file(".complete")?;
        complete.write_all(b"GUI2TUI-MATERIALIZATION-COMPLETE-v1")?;
        complete.flush()?;
        let _ = complete.keep();
        crate::runtime::artifacts::debug_crash_failpoint("F");
        Ok(MaterializedArtifact {
            directory,
            metadata,
        })
    }

    /// Exact private directory only; never follows a manifest-supplied path and
    /// never recursively removes arbitrary contents. No endpoint is involved.
    pub fn reap_after_ttl(directory: &Path) -> io::Result<()> {
        let lease = crate::runtime::artifacts::OwnedArtifactDirectory::reaper_lease(directory)?;
        let stat = fs::symlink_metadata(directory)?;
        if !stat.is_dir()
            || !directory
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("operation-"))
        {
            return Err(io::Error::other("not a materializer directory"));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if stat.permissions().mode() & 0o077 != 0 {
                return Err(io::Error::other("artifact directory is not private"));
            }
        }
        let manifest = fs::read_dir(directory)?
            .filter_map(Result::ok)
            .find(|entry| {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                name != "ownership.json" && name.ends_with(".json")
            })
            .ok_or_else(|| io::Error::other("materialization manifest missing"))?
            .path();
        use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
        let file = fs::OpenOptions::new()
            .read(true)
            .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
            .open(&manifest)?;
        let stat = file.metadata()?;
        if !stat.is_file() || stat.nlink() != 1 || stat.uid() != rustix::process::geteuid().as_raw()
        {
            return Err(io::Error::other("unsafe materialization metadata"));
        }
        let metadata: MaterializationMetadata = serde_json::from_reader(file.take(65536))?;
        if metadata.ownership_marker != "GUI2TUI-MATERIALIZATION-v1"
            || metadata.created_unix > metadata.expires_unix
        {
            return Err(io::Error::other("invalid materialization ownership"));
        }
        if !metadata.filename.starts_with("artifact-")
            || !["png", "jpg", "svg", "pdf"]
                .iter()
                .any(|extension| metadata.filename.ends_with(&format!(".{extension}")))
        {
            return Err(io::Error::other("invalid artifact filename"));
        }
        let wait = metadata.expires_unix.saturating_sub(unix_now());
        if wait > 1800 {
            return Err(io::Error::other("invalid artifact expiry"));
        }
        println!("OWNED_ARTIFACT_LEASE_READY");
        io::stdout().flush()?;
        std::thread::sleep(Duration::from_secs(wait));
        drop(lease);
        if !crate::runtime::artifacts::recover_owned_directory(directory)? {
            return Err(io::Error::other(
                "materialized artifact lease is still active",
            ));
        }
        Ok(())
    }
}
fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn descriptor() -> ArtifactDescriptor {
        ArtifactDescriptor {
            origin: ArtifactOrigin::OriginalResource,
            id: super::super::ArtifactId::new(1),
            kind: super::super::ModalityKind::Image,
            mime: "image/png".into(),
            size: 3,
            hash: ArtifactHash::sha256(b"abc"),
            display_name: Some("../../untrusted.png".into()),
            lifetime: super::super::ArtifactLifetime::Session,
        }
    }

    #[cfg(debug_assertions)]
    #[test]
    fn materializer_crash_child() {
        if std::env::var_os("GUI2TUI_MATERIALIZER_CRASH_STAGE").is_none() {
            return;
        }
        let mut descriptor = descriptor();
        descriptor.size = 131_072;
        let bytes = vec![b'x'; descriptor.size as usize];
        descriptor.hash = ArtifactHash::sha256(&bytes);
        let _ = ArtifactMaterializer::materialize_owned(
            descriptor,
            &bytes[..],
            None,
            Duration::from_secs(300),
            true,
            &Default::default(),
            Some((crate::runtime::RuntimeSessionId::default(), 99)),
        );
        panic!("configured materializer crash failpoint was not reached");
    }

    #[cfg(debug_assertions)]
    #[test]
    fn every_pre_mid_post_payload_crash_window_is_recovered() {
        use std::os::unix::fs::PermissionsExt;
        use std::process::{Command, Stdio};
        for stage in ["A", "B", "C", "D", "E", "F"] {
            let base = tempfile::Builder::new()
                .permissions(std::fs::Permissions::from_mode(0o700))
                .tempdir()
                .unwrap();
            let status = Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "modality::materialize::tests::materializer_crash_child",
                    "--nocapture",
                ])
                .env("GUI2TUI_MATERIALIZER_CRASH_STAGE", stage)
                .env("GUI2TUI_ARTIFACT_TEST_BASE", base.path())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .unwrap();
            assert_eq!(status.code(), Some(86), "failpoint {stage}");
            assert_eq!(
                crate::runtime::artifacts::recover_abandoned_in(base.path()).unwrap(),
                1,
                "failpoint {stage}"
            );
            let owned_root = base.path().join(format!(
                "gui2tui-owned-{}",
                rustix::process::geteuid().as_raw()
            ));
            assert_eq!(fs::read_dir(owned_root).unwrap().count(), 0, "{stage}");
        }
    }
    #[test]
    fn materialization_is_private_hashed_bounded_and_not_transport() {
        let artifact = ArtifactMaterializer::materialize(
            descriptor(),
            &b"abc"[..],
            None,
            Duration::from_secs(1),
            true,
            &Default::default(),
        )
        .unwrap();
        let path = artifact.path();
        assert!(
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("artifact-")
        );
        assert_eq!(path.extension().unwrap(), "png");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                path.parent()
                    .unwrap()
                    .metadata()
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
        }
        assert_eq!(fs::read(&path).unwrap(), b"abc");
        drop(artifact);
        assert!(!path.exists());
        assert!(
            ArtifactMaterializer::materialize(
                descriptor(),
                &b"bad"[..],
                None,
                Duration::from_secs(1),
                true,
                &Default::default()
            )
            .is_err()
        );
        assert!(
            ArtifactMaterializer::materialize(
                descriptor(),
                &b"abc"[..],
                None,
                Duration::from_secs(1),
                false,
                &Default::default()
            )
            .is_err()
        );
        assert!(
            ArtifactMaterializer::materialize(
                descriptor(),
                &b"abcd"[..],
                None,
                Duration::from_secs(1),
                true,
                &Default::default()
            )
            .is_err()
        );
    }

    #[test]
    fn owned_materialization_records_runtime_operation_without_content() {
        let session = crate::runtime::RuntimeSessionId::default();
        let artifact = ArtifactMaterializer::materialize_owned(
            descriptor(),
            &b"abc"[..],
            None,
            Duration::from_secs(1),
            true,
            &Default::default(),
            Some((session.clone(), 42)),
        )
        .unwrap();
        assert_eq!(artifact.metadata.session_id, session);
        assert_eq!(artifact.metadata.operation_id, 42);
        assert_eq!(
            artifact.metadata.ownership_marker,
            "GUI2TUI-MATERIALIZATION-v1"
        );
    }
    #[test]
    fn snapshot_cannot_claim_original_provenance() {
        let mut d = descriptor();
        d.origin = ArtifactOrigin::RenderedSnapshot;
        assert!(
            ArtifactMaterializer::materialize(
                d,
                &b"abc"[..],
                None,
                Duration::from_secs(1),
                true,
                &Default::default()
            )
            .is_err()
        );
    }
    #[test]
    fn cancellation_prevents_materialization() {
        let cancel = CancellationToken::default();
        cancel.cancel();
        assert!(
            ArtifactMaterializer::materialize(
                descriptor(),
                &b"abc"[..],
                None,
                Duration::from_secs(1),
                true,
                &cancel
            )
            .is_err()
        );
    }

    #[test]
    fn expiry_reaps_only_generated_files_and_directory() {
        let artifact = ArtifactMaterializer::materialize(
            descriptor(),
            &b"abc"[..],
            None,
            Duration::from_secs(1),
            true,
            &Default::default(),
        )
        .unwrap();
        let path = artifact.path();
        let directory = artifact.directory.keep();
        ArtifactMaterializer::reap_after_ttl(&directory).unwrap();
        assert!(!path.exists());
        assert!(!path.parent().unwrap().exists());
    }

    #[test]
    fn rendered_metadata_never_calls_it_original() {
        let mut d = descriptor();
        d.origin = ArtifactOrigin::RenderedSnapshot;
        let artifact = ArtifactMaterializer::materialize(
            d,
            &b"abc"[..],
            Some((
                ScreenRegion {
                    x: 0,
                    y: 0,
                    width: 1,
                    height: 1,
                },
                CaptureQuality::CompositedScreenSnapshot,
            )),
            Duration::from_secs(10),
            true,
            &Default::default(),
        )
        .unwrap();
        let text = serde_json::to_string(&artifact.metadata).unwrap();
        assert!(text.contains("RenderedSnapshot"));
        assert!(!text.contains("OriginalResource"));
        assert!(text.contains("CompositedScreenSnapshot"));
    }
}
