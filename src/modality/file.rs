use super::{ArtifactDescriptor, ArtifactHash, ArtifactId, ArtifactLifetime, ModalityKind};
use std::io::Read;
pub fn file_type(path: &std::path::Path) -> Option<(&'static str, ModalityKind)> {
    use ModalityKind::*;
    Some(
        match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
            "png" => ("image/png", Image),
            "jpg" | "jpeg" => ("image/jpeg", Image),
            "svg" => ("image/svg+xml", VectorGraphic),
            "gif" => ("image/gif", Image),
            "webp" => ("image/webp", Image),
            "pdf" => ("application/pdf", Document),
            "mp4" => ("video/mp4", Video),
            "webm" => ("video/webm", Video),
            "mp3" => ("audio/mpeg", Audio),
            "ogg" => ("audio/ogg", Audio),
            "wav" => ("audio/wav", Audio),
            "glb" => ("model/gltf-binary", PortableModel),
            _ => return None,
        },
    )
}
pub fn describe_file(
    input: &std::path::Path,
    mime: String,
    kind: ModalityKind,
) -> Result<(ArtifactDescriptor, std::fs::File), Box<dyn std::error::Error>> {
    describe_file_cancellable(input, mime, kind, &super::CancellationToken::default())
}

pub fn describe_file_cancellable(
    input: &std::path::Path,
    mime: String,
    kind: ModalityKind,
    cancel: &super::CancellationToken,
) -> Result<(ArtifactDescriptor, std::fs::File), Box<dyn std::error::Error>> {
    use sha2::{Digest, Sha256};
    use std::io::{Seek, SeekFrom};
    // Reject special files before opening; O_NONBLOCK also closes the FIFO race.
    use std::os::unix::fs::OpenOptionsExt;
    if !std::fs::metadata(input)?.is_file() {
        return Err("artifact source must be a regular file".into());
    }
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(rustix::fs::OFlags::NONBLOCK.bits() as i32)
        .open(input)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err("artifact source must be a regular file".into());
    }
    if metadata.len() > 512 * 1024 * 1024 {
        return Err("artifact exceeds 512 MiB producer limit".into());
    }
    let mut hasher = Sha256::new();
    let mut buffer = [0; 65536];
    let mut size = 0;
    loop {
        if cancel.is_cancelled() {
            return Err("file preparation cancelled".into());
        }
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        size += count as u64;
        if size > 512 * 1024 * 1024 {
            return Err("artifact grew beyond limit".into());
        }
        hasher.update(&buffer[..count]);
    }
    file.seek(SeekFrom::Start(0))?;
    Ok((
        ArtifactDescriptor {
            origin: Default::default(),
            id: ArtifactId::new(1),
            kind,
            mime,
            size,
            hash: ArtifactHash(hasher.finalize().into()),
            display_name: input.file_name().map(|s| s.to_string_lossy().into_owned()),
            lifetime: ArtifactLifetime::Session,
        },
        file,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_file_is_hashed_rewound_and_special_sources_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.pdf");
        std::fs::write(&path, b"fixture").unwrap();
        let (descriptor, mut file) =
            describe_file(&path, "application/pdf".into(), ModalityKind::Document).unwrap();
        assert_eq!(descriptor.hash, ArtifactHash::sha256(b"fixture"));
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, b"fixture");
        assert!(
            describe_file(dir.path(), "application/pdf".into(), ModalityKind::Document).is_err()
        );
        let cancel = super::super::CancellationToken::default();
        cancel.cancel();
        assert!(
            describe_file_cancellable(
                &path,
                "application/pdf".into(),
                ModalityKind::Document,
                &cancel
            )
            .is_err()
        );
        assert!(file_type(std::path::Path::new("script.sh")).is_none());
    }
}
