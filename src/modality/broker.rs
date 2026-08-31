use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tempfile::{NamedTempFile, TempDir};

use thiserror::Error;

use super::{
    ArtifactDescriptor, LocalModalityCapabilities, ModalityKind, ReferencedResource,
    ResourceReference, transport::HandoffMetrics,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthorizationDecision {
    Once,
    Session,
    Deny,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LocalResource {
    Uri(String),
    Path(PathBuf),
}

pub trait LocalHandler: Send + Sync {
    fn open(&self, resource: &LocalResource, mime: &str) -> Result<(), String>;
}

#[derive(Clone, Default)]
pub struct RecordingHandler {
    invocations: Arc<Mutex<Vec<(LocalResource, String)>>>,
}

impl RecordingHandler {
    pub fn invocations(&self) -> Vec<(LocalResource, String)> {
        self.invocations
            .lock()
            .expect("recording handler lock")
            .clone()
    }
}

impl LocalHandler for RecordingHandler {
    fn open(&self, resource: &LocalResource, mime: &str) -> Result<(), String> {
        self.invocations
            .lock()
            .map_err(|_| "recording handler lock poisoned".to_owned())?
            .push((resource.clone(), mime.to_owned()));
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct ProcessHandler {
    program: PathBuf,
}

impl ProcessHandler {
    /// The program is local user configuration. It is never accepted from a
    /// remote descriptor or server control message.
    pub fn configured_locally(program: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
        }
    }
}

impl LocalHandler for ProcessHandler {
    fn open(&self, resource: &LocalResource, _mime: &str) -> Result<(), String> {
        let target = match resource {
            LocalResource::Uri(uri) => uri.clone(),
            LocalResource::Path(path) => path.display().to_string(),
        };
        let mut child = Command::new(&self.program)
            .arg(target)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| error.to_string())?;
        let started = Instant::now();
        loop {
            if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
                return if status.success() {
                    Ok(())
                } else {
                    Err(format!("local launcher exited with {status}"))
                };
            }
            if started.elapsed() >= Duration::from_secs(2) {
                // Direct viewers may remain alive. Do not block the broker or
                // kill the user's viewer; reap it separately. Opened means
                // launcher accepted, not that visual consumption was proven.
                std::thread::spawn(move || {
                    let _ = child.wait();
                });
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

#[derive(Default)]
pub struct HandlerRegistry {
    handlers: HashMap<String, Box<dyn LocalHandler>>,
}

impl HandlerRegistry {
    pub fn mime_patterns(&self) -> HashSet<String> {
        self.handlers.keys().cloned().collect()
    }
    pub fn register(&mut self, mime_pattern: impl Into<String>, handler: Box<dyn LocalHandler>) {
        self.handlers.insert(mime_pattern.into(), handler);
    }

    pub fn handler(&self, mime: &str) -> Option<&dyn LocalHandler> {
        self.handlers
            .get(mime)
            .or_else(|| {
                mime.split_once('/')
                    .and_then(|(class, _)| self.handlers.get(&format!("{class}/*")))
            })
            .map(Box::as_ref)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PathMapping {
    source_prefix: PathBuf,
    destination_prefix: PathBuf,
}

impl PathMapping {
    pub fn new(
        source_prefix: impl Into<PathBuf>,
        destination_prefix: impl Into<PathBuf>,
    ) -> Result<Self, BrokerError> {
        let source_prefix = validate_absolute_clean(source_prefix.into())?;
        let destination_prefix = fs::canonicalize(destination_prefix.into())?;
        Ok(Self {
            source_prefix,
            destination_prefix,
        })
    }

    pub fn translate(&self, remote: &Path) -> Result<PathBuf, BrokerError> {
        let remote = validate_absolute_clean(remote.to_path_buf())?;
        let suffix = remote
            .strip_prefix(&self.source_prefix)
            .map_err(|_| BrokerError::PathOutsideMapping(remote.display().to_string()))?;
        let local = self.destination_prefix.join(suffix);
        let canonical = fs::canonicalize(&local)?;
        if !canonical.starts_with(&self.destination_prefix) {
            return Err(BrokerError::PathOutsideMapping(
                canonical.display().to_string(),
            ));
        }
        if !canonical.is_file() {
            return Err(BrokerError::Unsupported);
        }
        Ok(canonical)
    }
}

fn validate_absolute_clean(path: PathBuf) -> Result<PathBuf, BrokerError> {
    if !path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(BrokerError::UnsafePath(path.display().to_string()));
    }
    Ok(path)
}

#[derive(Debug, Error)]
pub enum BrokerError {
    #[error("local modality authorization denied")]
    Denied,
    #[error("local modality client does not support this resource")]
    Unsupported,
    #[error("no local handler is configured for MIME {0}")]
    HandlerUnavailable(String),
    #[error("local handler failed: {0}")]
    HandlerFailed(String),
    #[error("unsafe path: {0}")]
    UnsafePath(String),
    #[error("path escapes configured mapping: {0}")]
    PathOutsideMapping(String),
    #[error("untrusted reference provenance")]
    UntrustedReference,
    #[error("URI scheme is not permitted: {0}")]
    SchemeDenied(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub struct LocalModalityBroker {
    capabilities: LocalModalityCapabilities,
    registry: HandlerRegistry,
    mappings: Vec<PathMapping>,
    temp_root: TempDir,
    temporary_artifacts: Vec<(NamedTempFile, Instant)>,
    session_authorized: HashSet<(ModalityKind, String)>,
    metrics: HandoffMetrics,
}

impl LocalModalityBroker {
    pub fn new(
        mut capabilities: LocalModalityCapabilities,
        registry: HandlerRegistry,
        temp_root: impl Into<PathBuf>,
    ) -> Result<Self, BrokerError> {
        let temp_root = temp_root.into();
        fs::create_dir_all(&temp_root)?;
        // Never reuse a predictable server ID or client PID directory for data.
        let temp_root = tempfile::Builder::new()
            .prefix("gui2tui-session-")
            .tempdir_in(temp_root)?;
        capabilities
            .mime_patterns
            .retain(|mime| registry.handler(mime).is_some());
        Ok(Self {
            capabilities,
            registry,
            mappings: Vec::new(),
            temp_root,
            temporary_artifacts: Vec::new(),
            session_authorized: HashSet::new(),
            metrics: HandoffMetrics::default(),
        })
    }

    pub fn add_mapping(&mut self, mapping: PathMapping) {
        self.mappings.push(mapping);
    }

    pub fn capabilities(&self) -> &LocalModalityCapabilities {
        &self.capabilities
    }

    pub fn metrics(&self) -> HandoffMetrics {
        self.metrics
    }

    pub fn handoff_reference(
        &mut self,
        kind: ModalityKind,
        resource: &ReferencedResource,
        authorization: AuthorizationDecision,
    ) -> Result<(), BrokerError> {
        if !resource.provenance.trusted() {
            return Err(BrokerError::UntrustedReference);
        }
        if !self.capabilities.supports_reference(resource) {
            return Err(BrokerError::Unsupported);
        }
        let mime = resource.mime.as_deref().ok_or(BrokerError::Unsupported)?;
        self.authorize(kind, mime, authorization)?;
        let local = self.resolve_local_reference(&resource.reference)?;
        if let LocalResource::Path(path) = &local
            && !path_extension_matches_mime(path, mime)
        {
            return Err(BrokerError::Unsupported);
        }
        let Some(handler) = self.registry.handler(mime) else {
            self.metrics.handler_unavailable += 1;
            return Err(BrokerError::HandlerUnavailable(mime.to_owned()));
        };
        handler
            .open(&local, mime)
            .map_err(BrokerError::HandlerFailed)?;
        self.metrics.reference_hits += 1;
        self.metrics.reference_only_handoffs += 1;
        Ok(())
    }

    fn resolve_local_reference(
        &self,
        reference: &ResourceReference,
    ) -> Result<LocalResource, BrokerError> {
        match reference {
            ResourceReference::NetworkUri(uri) => {
                let parsed = url::Url::parse(uri).map_err(|_| BrokerError::Unsupported)?;
                if !matches!(parsed.scheme(), "https" | "http")
                    || parsed.host_str().is_none()
                    || !parsed.username().is_empty()
                    || parsed.password().is_some()
                {
                    return Err(BrokerError::Unsupported);
                }
                Ok(LocalResource::Uri(uri.clone()))
            }
            ResourceReference::LocalPath(path) => {
                let path = validate_absolute_clean(path.into())?;
                if !self.mappings.is_empty() {
                    return self
                        .mappings
                        .iter()
                        .find_map(|m| m.translate(&path).ok())
                        .map(LocalResource::Path)
                        .ok_or(BrokerError::Unsupported);
                }
                let path = fs::canonicalize(path)?;
                if !path.is_file() {
                    return Err(BrokerError::Unsupported);
                }
                Ok(LocalResource::Path(path))
            }
            ResourceReference::MappedPath { remote } => self
                .mappings
                .iter()
                .find_map(|mapping| mapping.translate(Path::new(remote)).ok())
                .map(LocalResource::Path)
                .ok_or_else(|| BrokerError::PathOutsideMapping(remote.clone())),
            ResourceReference::OpaqueUri(uri) => Err(BrokerError::SchemeDenied(
                uri.split_once(':')
                    .map_or("unknown", |(scheme, _)| scheme)
                    .to_owned(),
            )),
        }
    }

    pub(crate) fn authorize_artifact(
        &mut self,
        descriptor: &ArtifactDescriptor,
        decision: AuthorizationDecision,
    ) -> Result<(), BrokerError> {
        self.cleanup_expired();
        // Bound retained session storage as well as each individual transfer.
        // No LRU eviction of resources that a viewer may still be using.
        let retained_bytes: u64 = self
            .temporary_artifacts
            .iter()
            .filter_map(|(file, _)| file.as_file().metadata().ok())
            .map(|metadata| metadata.len())
            .sum();
        if self.temporary_artifacts.len() >= 64
            || retained_bytes.saturating_add(descriptor.size) > 1024 * 1024 * 1024
        {
            return Err(BrokerError::Unsupported);
        }
        if !self.capabilities.artifact_receive || !self.capabilities.supports_mime(&descriptor.mime)
        {
            return Err(BrokerError::Unsupported);
        }
        if self.registry.handler(&descriptor.mime).is_none() {
            self.metrics.handler_unavailable += 1;
            return Err(BrokerError::HandlerUnavailable(descriptor.mime.clone()));
        }
        self.authorize(descriptor.kind, &descriptor.mime, decision)
    }

    fn authorize(
        &mut self,
        kind: ModalityKind,
        mime: &str,
        decision: AuthorizationDecision,
    ) -> Result<(), BrokerError> {
        if !is_viewable_mime(kind, mime) {
            self.metrics.authorization_denied += 1;
            return Err(BrokerError::Unsupported);
        }
        let key = (kind, mime.to_owned());
        // An explicit denial always wins, even after a previous session grant.
        match decision {
            AuthorizationDecision::Once => Ok(()),
            AuthorizationDecision::Session => {
                self.session_authorized.insert(key);
                Ok(())
            }
            AuthorizationDecision::Deny => {
                self.metrics.authorization_denied += 1;
                Err(BrokerError::Denied)
            }
        }
    }

    pub fn session_allows(&self, kind: ModalityKind, mime: &str) -> bool {
        self.session_authorized.contains(&(kind, mime.to_owned()))
    }

    pub(crate) fn artifact_file(
        &self,
        descriptor: &ArtifactDescriptor,
    ) -> Result<NamedTempFile, BrokerError> {
        Ok(tempfile::Builder::new()
            .prefix("artifact-")
            .suffix(&format!(".{}", safe_extension(&descriptor.mime)))
            .tempfile_in(self.temp_root.path())?)
    }

    pub(crate) fn record_bytes(&mut self, count: usize) {
        self.metrics.artifact_bytes += count as u64;
    }

    pub(crate) fn finish_artifact(
        &mut self,
        descriptor: &ArtifactDescriptor,
        file: NamedTempFile,
    ) -> Result<(), BrokerError> {
        let Some(handler) = self.registry.handler(&descriptor.mime) else {
            self.metrics.handler_unavailable += 1;
            return Err(BrokerError::HandlerUnavailable(descriptor.mime.clone()));
        };
        handler
            .open(
                &LocalResource::Path(file.path().to_path_buf()),
                &descriptor.mime,
            )
            .map_err(BrokerError::HandlerFailed)?;
        let ttl = match descriptor.lifetime {
            super::ArtifactLifetime::Session => Duration::from_secs(1800),
            super::ArtifactLifetime::Temporary { ttl } => {
                ttl.clamp(Duration::from_secs(30), Duration::from_secs(1800))
            }
        };
        self.temporary_artifacts.push((file, Instant::now() + ttl));
        self.metrics.artifact_fallbacks += 1;
        Ok(())
    }

    pub(crate) fn mark_cancelled(&mut self) {
        self.metrics.transfer_cancelled += 1;
    }

    pub fn cleanup(&mut self) {
        self.temporary_artifacts.clear();
    }

    pub fn cleanup_expired(&mut self) {
        self.temporary_artifacts
            .retain(|(_, expiry)| *expiry > Instant::now());
    }
}

impl Drop for LocalModalityBroker {
    fn drop(&mut self) {
        self.cleanup();
    }
}

pub(crate) fn is_viewable_mime(kind: ModalityKind, mime: &str) -> bool {
    match kind {
        ModalityKind::Image | ModalityKind::VectorGraphic => matches!(
            mime,
            "image/png" | "image/jpeg" | "image/svg+xml" | "image/gif" | "image/webp"
        ),
        ModalityKind::Document => mime == "application/pdf",
        ModalityKind::Video => matches!(mime, "video/mp4" | "video/webm"),
        ModalityKind::Audio => matches!(mime, "audio/mpeg" | "audio/ogg" | "audio/wav"),
        ModalityKind::PortableModel => {
            matches!(mime, "model/gltf+json" | "model/gltf-binary" | "model/obj")
        }
        ModalityKind::LiveVisual | ModalityKind::Unknown => false,
    }
}

fn safe_extension(mime: &str) -> &'static str {
    match mime {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/svg+xml" => "svg",
        "application/pdf" => "pdf",
        "video/mp4" => "mp4",
        "audio/mpeg" => "mp3",
        "model/gltf+json" => "gltf",
        "model/gltf-binary" => "glb",
        _ => "bin",
    }
}

fn path_extension_matches_mime(path: &Path, mime: &str) -> bool {
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    matches!(
        (mime, extension.as_str()),
        ("image/png", "png")
            | ("image/jpeg", "jpg" | "jpeg")
            | ("image/svg+xml", "svg")
            | ("image/gif", "gif")
            | ("image/webp", "webp")
            | ("application/pdf", "pdf")
            | ("video/mp4", "mp4")
            | ("video/webm", "webm")
            | ("audio/mpeg", "mp3")
            | ("audio/ogg", "ogg")
            | ("audio/wav", "wav")
            | ("model/gltf+json", "gltf")
            | ("model/gltf-binary", "glb")
            | ("model/obj", "obj")
    )
}

#[cfg(test)]
mod tests {
    use crate::modality::ReferenceProvenance;

    use super::*;

    #[test]
    fn declared_image_mime_does_not_authorize_opening_a_script_or_installer() {
        for name in [
            "run.command",
            "run.sh",
            "installer.pkg",
            "program.exe",
            "no-extension",
        ] {
            assert!(!path_extension_matches_mime(Path::new(name), "image/png"));
        }
        assert!(path_extension_matches_mime(
            Path::new("diagram.PNG"),
            "image/png"
        ));
        assert!(!path_extension_matches_mime(
            Path::new("diagram.png"),
            "application/pdf"
        ));
    }

    #[test]
    fn mapped_symlink_escape_and_directory_are_rejected() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("image.png"), b"test").unwrap();
        std::os::unix::fs::symlink(outside.path(), root.path().join("escape")).unwrap();
        let mapping = PathMapping::new("/srv", root.path()).unwrap();
        assert!(
            mapping
                .translate(Path::new("/srv/escape/image.png"))
                .is_err()
        );
        assert!(mapping.translate(Path::new("/srv")).is_err());
    }

    #[test]
    fn retained_artifact_budget_and_expiration_are_enforced() {
        let dir = tempfile::tempdir().unwrap();
        let mut registry = HandlerRegistry::default();
        registry.register("image/*", Box::<RecordingHandler>::default());
        let mut broker = LocalModalityBroker::new(
            LocalModalityCapabilities {
                reference_schemes: HashSet::new(),
                mime_patterns: HashSet::from(["image/*".into()]),
                artifact_receive: true,
            },
            registry,
            dir.path(),
        )
        .unwrap();
        let descriptor = ArtifactDescriptor {
            id: super::super::ArtifactId::new(1),
            kind: ModalityKind::Image,
            mime: "image/png".into(),
            size: 0,
            hash: super::super::ArtifactHash::sha256(b""),
            display_name: None,
            lifetime: super::super::ArtifactLifetime::Session,
        };
        for _ in 0..64 {
            let file = broker.artifact_file(&descriptor).unwrap();
            broker.finish_artifact(&descriptor, file).unwrap();
        }
        assert!(
            broker
                .authorize_artifact(&descriptor, AuthorizationDecision::Once)
                .is_err()
        );
        broker.temporary_artifacts[0].1 = Instant::now();
        broker.cleanup_expired();
        assert_eq!(broker.temporary_artifacts.len(), 63);
        assert!(
            broker
                .authorize_artifact(&descriptor, AuthorizationDecision::Once)
                .is_ok()
        );
        let huge = ArtifactDescriptor {
            size: 1024 * 1024 * 1024 + 1,
            ..descriptor
        };
        assert!(
            broker
                .authorize_artifact(&huge, AuthorizationDecision::Once)
                .is_err()
        );
    }

    #[test]
    fn mapping_is_prefix_bound_and_rejects_parent_escape() {
        let root = std::env::temp_dir().join(format!("gui2tui-map-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let mapping = PathMapping::new("/srv/shared", &root).unwrap();
        let canonical_root = fs::canonicalize(&root).unwrap();
        fs::create_dir_all(canonical_root.join("images")).unwrap();
        fs::write(canonical_root.join("images/a.png"), b"fixture").unwrap();
        assert_eq!(
            mapping
                .translate(Path::new("/srv/shared/images/a.png"))
                .unwrap(),
            canonical_root.join("images/a.png")
        );
        assert!(mapping.translate(Path::new("/srv/other/a.png")).is_err());
        assert!(
            mapping
                .translate(Path::new("/srv/shared/../other/a.png"))
                .is_err()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn server_reference_cannot_select_an_executable_or_unsafe_scheme() {
        let recording = RecordingHandler::default();
        let mut registry = HandlerRegistry::default();
        registry.register("image/*", Box::new(recording));
        let capabilities = LocalModalityCapabilities {
            reference_schemes: HashSet::from(["https".to_owned()]),
            mime_patterns: HashSet::from(["image/*".to_owned()]),
            artifact_receive: true,
        };
        let root = std::env::temp_dir().join(format!("gui2tui-broker-{}", std::process::id()));
        let mut broker = LocalModalityBroker::new(capabilities, registry, &root).unwrap();
        let resource = ReferencedResource {
            reference: ResourceReference::OpaqueUri("javascript:alert(1)".to_owned()),
            mime: Some("image/png".to_owned()),
            display_name: None,
            provenance: ReferenceProvenance::AccessibleAttribute,
        };
        assert!(matches!(
            broker.handoff_reference(ModalityKind::Image, &resource, AuthorizationDecision::Once),
            Err(BrokerError::Unsupported | BrokerError::SchemeDenied(_))
        ));
        broker.cleanup();
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn once_session_and_deny_have_distinct_authorization_semantics() {
        let recording = RecordingHandler::default();
        let mut registry = HandlerRegistry::default();
        registry.register("image/*", Box::new(recording.clone()));
        let capabilities = LocalModalityCapabilities {
            reference_schemes: HashSet::from(["https".to_owned()]),
            mime_patterns: HashSet::from(["image/*".to_owned()]),
            artifact_receive: true,
        };
        let root = std::env::temp_dir().join(format!("gui2tui-auth-{}", std::process::id()));
        let mut broker = LocalModalityBroker::new(capabilities, registry, &root).unwrap();
        let resource = ReferencedResource {
            reference: ResourceReference::NetworkUri("https://example.invalid/a.png".to_owned()),
            mime: Some("image/png".to_owned()),
            display_name: None,
            provenance: ReferenceProvenance::HyperlinkUri,
        };
        broker
            .handoff_reference(ModalityKind::Image, &resource, AuthorizationDecision::Once)
            .unwrap();
        assert!(matches!(
            broker.handoff_reference(ModalityKind::Image, &resource, AuthorizationDecision::Deny),
            Err(BrokerError::Denied)
        ));
        broker
            .handoff_reference(
                ModalityKind::Image,
                &resource,
                AuthorizationDecision::Session,
            )
            .unwrap();
        assert!(broker.session_allows(ModalityKind::Image, "image/png"));
        assert!(matches!(
            broker.handoff_reference(ModalityKind::Image, &resource, AuthorizationDecision::Deny),
            Err(BrokerError::Denied)
        ));
        broker
            .handoff_reference(ModalityKind::Image, &resource, AuthorizationDecision::Once)
            .unwrap();
        assert_eq!(recording.invocations().len(), 3);
        let _ = fs::remove_dir_all(root);
    }
}
