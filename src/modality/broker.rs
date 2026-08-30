use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Component, Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex},
};

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
        let status = Command::new(&self.program)
            .arg(target)
            .status()
            .map_err(|error| error.to_string())?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("local launcher exited with {status}"))
        }
    }
}

#[derive(Default)]
pub struct HandlerRegistry {
    handlers: HashMap<String, Box<dyn LocalHandler>>,
}

impl HandlerRegistry {
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
    temp_root: PathBuf,
    temporary_artifacts: Vec<PathBuf>,
    session_authorized: HashSet<(ModalityKind, String)>,
    metrics: HandoffMetrics,
}

impl LocalModalityBroker {
    pub fn new(
        capabilities: LocalModalityCapabilities,
        registry: HandlerRegistry,
        temp_root: impl Into<PathBuf>,
    ) -> Result<Self, BrokerError> {
        let temp_root = temp_root.into();
        fs::create_dir_all(&temp_root)?;
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
        let mime = resource.mime.as_deref().unwrap_or(default_mime(kind));
        self.authorize(kind, mime, authorization)?;
        let local = self.resolve_local_reference(&resource.reference)?;
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
                let scheme = uri.split_once(':').map_or("", |(scheme, _)| scheme);
                if !matches!(scheme, "https" | "http") {
                    return Err(BrokerError::SchemeDenied(scheme.to_owned()));
                }
                Ok(LocalResource::Uri(uri.clone()))
            }
            ResourceReference::LocalPath(path) => {
                let path = validate_absolute_clean(path.into())?;
                Ok(LocalResource::Path(fs::canonicalize(path)?))
            }
            ResourceReference::MappedPath { remote, local: _ } => self
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
        if !self.capabilities.artifact_receive || !self.capabilities.supports_mime(&descriptor.mime)
        {
            return Err(BrokerError::Unsupported);
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
        if self.session_authorized.contains(&key) {
            return Ok(());
        }
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

    pub(crate) fn artifact_partial_path(&self, descriptor: &ArtifactDescriptor) -> PathBuf {
        self.temp_root.join(format!("{}.part", descriptor.id))
    }

    pub(crate) fn artifact_complete_path(&self, descriptor: &ArtifactDescriptor) -> PathBuf {
        self.temp_root.join(format!(
            "{}.{}",
            descriptor.id,
            safe_extension(&descriptor.mime)
        ))
    }

    pub(crate) fn finish_artifact(
        &mut self,
        descriptor: &ArtifactDescriptor,
        path: PathBuf,
        bytes: u64,
    ) -> Result<(), BrokerError> {
        let Some(handler) = self.registry.handler(&descriptor.mime) else {
            self.metrics.handler_unavailable += 1;
            return Err(BrokerError::HandlerUnavailable(descriptor.mime.clone()));
        };
        handler
            .open(&LocalResource::Path(path.clone()), &descriptor.mime)
            .map_err(BrokerError::HandlerFailed)?;
        self.temporary_artifacts.push(path);
        self.metrics.artifact_fallbacks += 1;
        self.metrics.artifact_bytes += bytes;
        Ok(())
    }

    pub(crate) fn mark_cancelled(&mut self) {
        self.metrics.transfer_cancelled += 1;
    }

    pub fn cleanup(&mut self) {
        for path in self.temporary_artifacts.drain(..) {
            let _ = fs::remove_file(path);
        }
    }
}

impl Drop for LocalModalityBroker {
    fn drop(&mut self) {
        self.cleanup();
    }
}

fn default_mime(kind: ModalityKind) -> &'static str {
    match kind {
        ModalityKind::Image | ModalityKind::VectorGraphic => "image/*",
        ModalityKind::Document => "application/pdf",
        ModalityKind::Video => "video/*",
        ModalityKind::Audio => "audio/*",
        ModalityKind::PortableModel => "model/*",
        ModalityKind::LiveVisual | ModalityKind::Unknown => "application/octet-stream",
    }
}

fn is_viewable_mime(kind: ModalityKind, mime: &str) -> bool {
    match kind {
        ModalityKind::Image | ModalityKind::VectorGraphic => mime.starts_with("image/"),
        ModalityKind::Document => mime == "application/pdf",
        ModalityKind::Video => mime.starts_with("video/"),
        ModalityKind::Audio => mime.starts_with("audio/"),
        ModalityKind::PortableModel => mime.starts_with("model/"),
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

#[cfg(test)]
mod tests {
    use crate::modality::ReferenceProvenance;

    use super::*;

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
        broker
            .handoff_reference(ModalityKind::Image, &resource, AuthorizationDecision::Deny)
            .unwrap();
        assert_eq!(recording.invocations().len(), 3);
        let _ = fs::remove_dir_all(root);
    }
}
