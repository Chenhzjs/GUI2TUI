//! Host-local storage, deliberately independent of broker and transport.
use super::{
    ArtifactDescriptor, ArtifactHash, ArtifactOrigin, CancellationToken,
    acquisition::{CaptureQuality, MAX_ARTIFACT_BYTES, ScreenRegion},
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterializationMetadata {
    pub descriptor: ArtifactDescriptor,
    pub region: Option<ScreenRegion>,
    pub quality: Option<CaptureQuality>,
    pub expires_unix: u64,
    pub filename: String,
}

pub struct MaterializedArtifact {
    directory: tempfile::TempDir,
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
        std::process::Command::new(inspector)
            .arg("--reap-materialized")
            .arg(self.directory.path())
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()?;
        let _ = self.directory.keep();
        Ok(path)
    }
}

pub struct ArtifactMaterializer;
impl ArtifactMaterializer {
    pub fn materialize(
        mut descriptor: ArtifactDescriptor,
        mut source: impl Read,
        snapshot: Option<(ScreenRegion, CaptureQuality)>,
        ttl: Duration,
        explicit: bool,
        cancel: &CancellationToken,
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
        let mut builder = tempfile::Builder::new();
        builder.prefix("gui2tui-artifact-");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            builder.permissions(fs::Permissions::from_mode(0o700));
        }
        let directory = builder.tempdir()?;
        let filename = format!("artifact.{extension}");
        let mut file = tempfile::NamedTempFile::new_in(directory.path())?;
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
        }
        if count != descriptor.size || ArtifactHash(hash.finalize().into()) != descriptor.hash {
            return Err(io::Error::other("materialization size/hash mismatch"));
        }
        if cancel.is_cancelled() {
            return Err(io::Error::other("materialization cancelled"));
        }
        file.persist_noclobber(directory.path().join(&filename))
            .map_err(|e| e.error)?;
        let metadata = MaterializationMetadata {
            descriptor,
            region: snapshot.map(|x| x.0),
            quality: snapshot.map(|x| x.1),
            expires_unix: unix_now() + ttl.as_secs(),
            filename,
        };
        let mut manifest = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(directory.path().join("manifest.json"))?;
        serde_json::to_writer(&mut manifest, &metadata)?;
        Ok(MaterializedArtifact {
            directory,
            metadata,
        })
    }

    /// Exact private directory only; never follows a manifest-supplied path and
    /// never recursively removes arbitrary contents. No endpoint is involved.
    pub fn reap_after_ttl(directory: &Path) -> io::Result<()> {
        let stat = fs::symlink_metadata(directory)?;
        if !stat.is_dir()
            || !directory
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("gui2tui-artifact-"))
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
        let manifest = directory.join("manifest.json");
        if fs::symlink_metadata(&manifest)?.file_type().is_symlink() {
            return Err(io::Error::other("invalid manifest"));
        }
        let metadata: MaterializationMetadata =
            serde_json::from_reader(fs::File::open(&manifest)?.take(65536))?;
        if ![
            "artifact.png",
            "artifact.jpg",
            "artifact.svg",
            "artifact.pdf",
        ]
        .contains(&metadata.filename.as_str())
        {
            return Err(io::Error::other("invalid artifact filename"));
        }
        let wait = metadata.expires_unix.saturating_sub(unix_now());
        if wait > 1800 {
            return Err(io::Error::other("invalid artifact expiry"));
        }
        std::thread::sleep(Duration::from_secs(wait));
        fs::remove_file(directory.join(metadata.filename))?;
        fs::remove_file(manifest)?;
        fs::remove_dir(directory)
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
        assert_eq!(path.file_name().unwrap(), "artifact.png");
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
        ArtifactMaterializer::reap_after_ttl(path.parent().unwrap()).unwrap();
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
