use std::{
    collections::HashMap,
    sync::atomic::{AtomicU64, Ordering},
};

use thiserror::Error;
use url::Url;

use crate::semantic::{BackendLocator, RuntimeNodeId, SemanticCache, SemanticRole};

use super::{
    ExternalModality, ExternalModalityId, ModalityCapabilities, ModalityKind, ModalityResolution,
    ReferenceProvenance, ReferencedResource, ResourceReference, TransferPolicy,
};

static NEXT_MODALITY_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModalityCandidate {
    pub owner: RuntimeNodeId,
    pub locator: BackendLocator,
    pub evidence_locators: Vec<BackendLocator>,
    pub kind: ModalityKind,
    pub label: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ModalityMetadata {
    pub accessible_attributes: HashMap<String, String>,
    pub document_attributes: HashMap<String, String>,
    pub hyperlink_uris: Vec<String>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ResolutionError {
    #[error("resource reference uses a disallowed URI scheme: {0}")]
    DisallowedScheme(String),
    #[error("resource reference is malformed: {0}")]
    MalformedReference(String),
}

#[derive(Clone, Debug)]
pub struct ModalityResolver {
    policy: TransferPolicy,
}

impl Default for ModalityResolver {
    fn default() -> Self {
        Self {
            policy: TransferPolicy::PreferReference,
        }
    }
}

impl ModalityResolver {
    pub const fn new(policy: TransferPolicy) -> Self {
        Self { policy }
    }

    pub fn discover(cache: &SemanticCache) -> Vec<ModalityCandidate> {
        let mut candidates = Vec::new();
        for node in cache.nodes() {
            let kind = match &node.role {
                SemanticRole::Image => ModalityKind::Image,
                SemanticRole::Audio => ModalityKind::Audio,
                SemanticRole::Video => ModalityKind::Video,
                SemanticRole::Document => ModalityKind::Document,
                SemanticRole::Link => ModalityKind::Unknown,
                SemanticRole::Unknown(role)
                    if matches!(role.as_str(), "canvas" | "drawing area" | "3d view") =>
                {
                    ModalityKind::LiveVisual
                }
                _ => continue,
            };
            let mut evidence_locators = vec![node.backend_locator.clone()];
            if let Some(parent) = node.parent.and_then(|id| cache.node(id))
                && parent.role == SemanticRole::Link
            {
                evidence_locators.push(parent.backend_locator.clone());
            }
            for child in &node.children {
                if let Some(child) = cache.node(*child)
                    && child.role == SemanticRole::Link
                {
                    evidence_locators.push(child.backend_locator.clone());
                }
            }
            candidates.push(ModalityCandidate {
                owner: node.runtime_id,
                locator: node.backend_locator.clone(),
                evidence_locators,
                kind,
                label: node.name.clone().or_else(|| node.description.clone()),
            });
        }
        candidates.sort_by_key(|candidate| candidate.owner);
        candidates
    }

    pub fn resolve(
        &self,
        candidate: &ModalityCandidate,
        metadata: &[ModalityMetadata],
    ) -> ExternalModality {
        let reference = reference_from_metadata(metadata, candidate.kind == ModalityKind::Document);
        let resolution = if self.policy == TransferPolicy::Unavailable {
            ModalityResolution::Unavailable {
                reason: "disabled by transfer policy".to_owned(),
            }
        } else if candidate.kind == ModalityKind::LiveVisual {
            ModalityResolution::LiveVisualState {
                reason: "continuous graphical state has no portable semantic resource".to_owned(),
            }
        } else if let Some(resource) = reference {
            ModalityResolution::ReferencedResource(resource)
        } else {
            ModalityResolution::Unavailable {
                reason: "accessibility metadata exposes no trustworthy resource reference"
                    .to_owned(),
            }
        };
        // Resolution is not authorization or evidence of a connected handler.
        let capabilities = ModalityCapabilities::default();
        let kind = match &resolution {
            ModalityResolution::ReferencedResource(resource)
                if candidate.kind == ModalityKind::Unknown =>
            {
                resource
                    .mime
                    .as_deref()
                    .map(kind_for_mime)
                    .unwrap_or(ModalityKind::Unknown)
            }
            _ => candidate.kind,
        };
        ExternalModality {
            id: ExternalModalityId::new(NEXT_MODALITY_ID.fetch_add(1, Ordering::Relaxed)),
            owner: candidate.owner,
            kind,
            label: candidate.label.clone(),
            resolution,
            capabilities,
            transfer_policy: self.policy,
        }
    }

    /// An explicitly supplied minimal artifact is considered only when no
    /// usable reference was resolved. Live visual state is never converted.
    pub fn with_artifact(
        &self,
        mut modality: ExternalModality,
        artifact: super::PortableArtifact,
    ) -> ExternalModality {
        if matches!(modality.resolution, ModalityResolution::Unavailable { .. })
            && matches!(
                self.policy,
                TransferPolicy::PreferReference
                    | TransferPolicy::MinimalArtifactAllowed
                    | TransferPolicy::StaticVisualAllowed
            )
            && artifact.descriptor.kind == modality.kind
        {
            modality.resolution = ModalityResolution::PortableArtifact(artifact);
        }
        modality
    }
}

pub fn kind_for_mime(mime: &str) -> ModalityKind {
    if mime.starts_with("image/") {
        ModalityKind::Image
    } else if mime.starts_with("video/") {
        ModalityKind::Video
    } else if mime.starts_with("audio/") {
        ModalityKind::Audio
    } else if mime.starts_with("model/") {
        ModalityKind::PortableModel
    } else if mime == "application/pdf" {
        ModalityKind::Document
    } else {
        ModalityKind::Unknown
    }
}

impl ExternalModality {
    pub fn negotiate(&mut self, client: Option<&super::LocalModalityCapabilities>) {
        self.capabilities = ModalityCapabilities::default();
        let Some(client) = client else { return };
        match &self.resolution {
            ModalityResolution::ReferencedResource(resource) => {
                self.capabilities.reference_handoff = resource.provenance.trusted()
                    && resource
                        .mime
                        .as_deref()
                        .is_some_and(|mime| super::broker::is_viewable_mime(self.kind, mime))
                    && client.supports_reference(resource);
            }
            ModalityResolution::PortableArtifact(artifact) => {
                self.capabilities.artifact_handoff = client.artifact_receive
                    && client.supports_mime(&artifact.descriptor.mime)
                    && super::broker::is_viewable_mime(self.kind, &artifact.descriptor.mime);
            }
            _ => {}
        }
    }
}

fn reference_from_metadata(
    metadata: &[ModalityMetadata],
    allow_document_reference: bool,
) -> Option<ReferencedResource> {
    for source in metadata {
        for uri in &source.hyperlink_uris {
            if let Some(resource) = parse_reference(uri, ReferenceProvenance::HyperlinkUri, None) {
                return Some(resource);
            }
        }
    }
    if allow_document_reference {
        for source in metadata {
            for (key, value) in &source.document_attributes {
                if is_reference_key(key)
                    && let Some(resource) = parse_reference(
                        value,
                        ReferenceProvenance::DocumentAttribute,
                        mime_from_attributes(&source.document_attributes),
                    )
                {
                    return Some(resource);
                }
            }
        }
    }
    for source in metadata {
        for (key, value) in &source.accessible_attributes {
            if is_reference_key(key)
                && !matches!(key.to_ascii_lowercase().as_str(), "doc-url")
                && let Some(resource) = parse_reference(
                    value,
                    ReferenceProvenance::AccessibleAttribute,
                    mime_from_attributes(&source.accessible_attributes),
                )
            {
                return Some(resource);
            }
        }
    }
    None
}

fn is_reference_key(key: &str) -> bool {
    matches!(
        key.to_ascii_lowercase().as_str(),
        "uri" | "url" | "resource-uri" | "resource-url" | "src" | "doc-url"
    )
}

fn mime_from_attributes(attributes: &HashMap<String, String>) -> Option<String> {
    attributes.iter().find_map(|(key, value)| {
        matches!(
            key.to_ascii_lowercase().as_str(),
            "mime" | "mime-type" | "content-type"
        )
        .then(|| value.clone())
    })
}

fn parse_reference(
    raw: &str,
    provenance: ReferenceProvenance,
    mime: Option<String>,
) -> Option<ReferencedResource> {
    let url = Url::parse(raw).ok()?;
    let reference = match url.scheme() {
        "https" | "http" => ResourceReference::NetworkUri(url.to_string()),
        "file" => ResourceReference::LocalPath(url.to_file_path().ok()?.display().to_string()),
        _ => ResourceReference::OpaqueUri(url.to_string()),
    };
    Some(ReferencedResource {
        reference,
        mime: mime.or_else(|| mime_hint_from_uri(&url)),
        display_name: url
            .path_segments()
            .and_then(|mut segments| segments.next_back())
            .filter(|value| !value.is_empty())
            .map(str::to_owned),
        provenance,
    })
}

fn mime_hint_from_uri(url: &Url) -> Option<String> {
    let extension = url.path().rsplit('.').next()?.to_ascii_lowercase();
    Some(
        match extension.as_str() {
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "svg" => "image/svg+xml",
            "gif" => "image/gif",
            "webp" => "image/webp",
            "pdf" => "application/pdf",
            "mp4" => "video/mp4",
            "webm" => "video/webm",
            "mp3" => "audio/mpeg",
            "ogg" => "audio/ogg",
            "wav" => "audio/wav",
            "gltf" => "model/gltf+json",
            "glb" => "model/gltf-binary",
            "obj" => "model/obj",
            "html" | "htm" => "text/html",
            _ => return None,
        }
        .to_owned(),
    )
}

pub fn redact_reference(reference: &ResourceReference) -> String {
    match reference {
        ResourceReference::NetworkUri(raw) | ResourceReference::OpaqueUri(raw) => {
            let Ok(mut url) = Url::parse(raw) else {
                return "<malformed-uri>".to_owned();
            };
            if url.query().is_some() {
                url.set_query(Some("REDACTED"));
            }
            let _ = url.set_username("");
            let _ = url.set_password(None);
            url.set_fragment(None);
            url.to_string()
        }
        ResourceReference::LocalPath(path) => path.clone(),
        ResourceReference::MappedPath { remote } => {
            format!("mapped:{remote}")
        }
    }
}

pub fn format_external_modality(modality: &ExternalModality) -> String {
    let resolution = match &modality.resolution {
        ModalityResolution::ReferencedResource(resource) => format!(
            "reference={:?} provenance={:?} mime={:?}",
            redact_reference(&resource.reference),
            resource.provenance,
            resource.mime
        ),
        ModalityResolution::PortableArtifact(artifact) => format!(
            "artifact={} mime={} size={} sha256={}",
            artifact.descriptor.id,
            artifact.descriptor.mime,
            artifact.descriptor.size,
            artifact.descriptor.hash.hex()
        ),
        ModalityResolution::StaticVisualArtifact(artifact) => format!(
            "static-artifact={} mime={} size={}",
            artifact.descriptor.id, artifact.descriptor.mime, artifact.descriptor.size
        ),
        ModalityResolution::LiveVisualState { reason } => format!("live-visual reason={reason:?}"),
        ModalityResolution::Unavailable { reason } => format!("unavailable reason={reason:?}"),
    };
    format!(
        "Modality id={} owner={} kind={:?} label={:?} policy={:?} {}",
        modality.id,
        modality.owner,
        modality.kind,
        modality.label,
        modality.transfer_policy,
        resolution
    )
}

#[cfg(test)]
mod tests {
    use crate::semantic::{BackendLocator, DebugInfo, SemanticNode, TreeTruncation};

    use super::*;

    #[test]
    fn reference_first_and_live_state_cannot_be_replaced_by_artifact() {
        let candidate = ModalityCandidate {
            owner: RuntimeNodeId::new(4),
            locator: BackendLocator::new(":1.9", "/node/4"),
            evidence_locators: vec![],
            kind: ModalityKind::Image,
            label: None,
        };
        let artifact = super::super::PortableArtifact {
            descriptor: super::super::ArtifactDescriptor {
                id: super::super::ArtifactId::new(1),
                kind: ModalityKind::Image,
                mime: "image/png".into(),
                size: 0,
                hash: super::super::ArtifactHash::sha256(b""),
                display_name: None,
                lifetime: super::super::ArtifactLifetime::Session,
            },
        };
        let metadata = ModalityMetadata {
            hyperlink_uris: vec!["https://example.invalid/a.png".into()],
            ..Default::default()
        };
        let resolver = ModalityResolver::default();
        let referenced = resolver.resolve(&candidate, std::slice::from_ref(&metadata));
        assert!(matches!(
            resolver
                .with_artifact(referenced, artifact.clone())
                .resolution,
            ModalityResolution::ReferencedResource(_)
        ));
        let unresolved = resolver.resolve(&candidate, &[]);
        assert!(matches!(
            resolver
                .with_artifact(unresolved, artifact.clone())
                .resolution,
            ModalityResolution::PortableArtifact(_)
        ));
        let disabled = ModalityResolver::new(TransferPolicy::Unavailable);
        assert!(matches!(
            disabled
                .with_artifact(disabled.resolve(&candidate, &[]), artifact.clone())
                .resolution,
            ModalityResolution::Unavailable { .. }
        ));
        let live = ModalityCandidate {
            kind: ModalityKind::LiveVisual,
            ..candidate
        };
        assert!(matches!(
            resolver
                .with_artifact(resolver.resolve(&live, &[metadata]), artifact)
                .resolution,
            ModalityResolution::LiveVisualState { .. }
        ));
    }

    #[test]
    fn capability_requires_connected_matching_client_and_debug_redacts_credentials() {
        let candidate = ModalityCandidate {
            owner: RuntimeNodeId::new(4),
            locator: BackendLocator::new(":1.9", "/node/4"),
            evidence_locators: vec![],
            kind: ModalityKind::Image,
            label: None,
        };
        let mut modality = ModalityResolver::default().resolve(
            &candidate,
            &[ModalityMetadata {
                hyperlink_uris: vec!["https://example.invalid/a.png?token=secret".into()],
                ..Default::default()
            }],
        );
        modality.negotiate(None);
        assert!(!modality.capabilities.reference_handoff);
        let client = super::super::LocalModalityCapabilities {
            mime_patterns: std::collections::HashSet::from(["image/*".into()]),
            reference_schemes: std::collections::HashSet::from(["https".into()]),
            artifact_receive: false,
        };
        modality.negotiate(Some(&client));
        assert!(modality.capabilities.reference_handoff);
        let debug = format!(
            "{:?}",
            ResourceReference::NetworkUri(
                "https://user:password@example.invalid/a.png?token=secret#private".into()
            )
        );
        for secret in ["user", "password", "secret", "private"] {
            assert!(!debug.contains(secret));
        }
    }

    fn node(id: u64, role: SemanticRole, name: &str) -> SemanticNode {
        SemanticNode {
            runtime_id: RuntimeNodeId::new(id),
            backend_locator: BackendLocator::new(":1.9", format!("/node/{id}")),
            index_in_parent: None,
            role,
            name: Some(name.to_owned()),
            description: None,
            value: None,
            text_input_kind: None,
            states: Vec::new(),
            actions: Vec::new(),
            capabilities: Vec::new(),
            children: Vec::new(),
            truncations: Vec::<TreeTruncation>::new(),
            debug: DebugInfo::default(),
        }
    }

    #[test]
    fn hyperlink_reference_precedes_attributes_and_redacts_query() {
        let candidate = ModalityCandidate {
            owner: RuntimeNodeId::new(4),
            locator: BackendLocator::new(":1.9", "/node/4"),
            evidence_locators: Vec::new(),
            kind: ModalityKind::Image,
            label: Some("Diagram".to_owned()),
        };
        let metadata = ModalityMetadata {
            hyperlink_uris: vec!["https://cdn.example/diagram.png?token=secret".to_owned()],
            accessible_attributes: HashMap::from([(
                "uri".to_owned(),
                "https://wrong.example/image.png".to_owned(),
            )]),
            ..Default::default()
        };
        let modality = ModalityResolver::default().resolve(&candidate, &[metadata]);
        let ModalityResolution::ReferencedResource(resource) = modality.resolution else {
            panic!("expected reference")
        };
        assert_eq!(resource.provenance, ReferenceProvenance::HyperlinkUri);
        assert_eq!(
            redact_reference(&resource.reference),
            "https://cdn.example/diagram.png?REDACTED"
        );
    }

    #[test]
    fn hyperlink_precedence_is_global_and_image_never_uses_enclosing_document_url() {
        let candidate = ModalityCandidate {
            owner: RuntimeNodeId::new(4),
            locator: BackendLocator::new(":1.9", "/node/4"),
            evidence_locators: Vec::new(),
            kind: ModalityKind::Image,
            label: Some("Diagram".to_owned()),
        };
        let image = ModalityMetadata {
            document_attributes: HashMap::from([(
                "DocURL".to_owned(),
                "file:///enclosing-page.html".to_owned(),
            )]),
            ..Default::default()
        };
        let link = ModalityMetadata {
            hyperlink_uris: vec!["https://cdn.example/diagram.svg?token=secret".to_owned()],
            ..Default::default()
        };
        let modality = ModalityResolver::default().resolve(&candidate, &[image, link]);
        let ModalityResolution::ReferencedResource(resource) = modality.resolution else {
            panic!("expected reference")
        };
        assert_eq!(resource.provenance, ReferenceProvenance::HyperlinkUri);
        assert!(matches!(
            resource.reference,
            ResourceReference::NetworkUri(_)
        ));
    }

    #[test]
    fn names_and_descriptions_are_never_guessed_as_references() {
        let candidate = ModalityCandidate {
            owner: RuntimeNodeId::new(4),
            locator: BackendLocator::new(":1.9", "/node/4"),
            evidence_locators: Vec::new(),
            kind: ModalityKind::Image,
            label: Some("../../image.png https://unsafe.invalid".to_owned()),
        };
        assert!(matches!(
            ModalityResolver::default()
                .resolve(&candidate, &[])
                .resolution,
            ModalityResolution::Unavailable { .. }
        ));
    }

    #[test]
    fn live_visual_never_becomes_a_portable_artifact_without_evidence() {
        let mut root = node(1, SemanticRole::Application, "App");
        root.children.push(node(
            2,
            SemanticRole::Unknown("canvas".to_owned()),
            "Viewport",
        ));
        let cache = SemanticCache::from_snapshot(root).unwrap();
        let candidate = ModalityResolver::discover(&cache).pop().unwrap();
        assert!(matches!(
            ModalityResolver::default()
                .resolve(&candidate, &[])
                .resolution,
            ModalityResolution::LiveVisualState { .. }
        ));
    }
}
