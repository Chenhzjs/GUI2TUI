use serde::{Deserialize, Serialize};
use std::{collections::HashSet, fmt, time::Duration};

use crate::semantic::RuntimeNodeId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ExternalModalityId(u64);

impl ExternalModalityId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for ExternalModalityId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "modality-{}", self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModalityKind {
    Image,
    Document,
    Video,
    Audio,
    VectorGraphic,
    PortableModel,
    LiveVisual,
    Unknown,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TransferPolicy {
    ReferenceOnly,
    #[default]
    PreferReference,
    MinimalArtifactAllowed,
    StaticVisualAllowed,
    Unavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReferenceProvenance {
    HyperlinkUri,
    DocumentAttribute,
    AccessibleAttribute,
    LocalFileReference,
    SharedPathMapping,
    UserConfiguredMapping,
    Unknown,
}

impl ReferenceProvenance {
    pub const fn trusted(self) -> bool {
        !matches!(self, Self::Unknown)
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ResourceReference {
    NetworkUri(String),
    LocalPath(String),
    MappedPath { remote: String },
    OpaqueUri(String),
}

impl fmt::Debug for ResourceReference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", super::redact_reference(self))
    }
}

impl ResourceReference {
    pub fn scheme(&self) -> &str {
        match self {
            Self::NetworkUri(uri) | Self::OpaqueUri(uri) => {
                uri.split_once(':').map_or("unknown", |(scheme, _)| scheme)
            }
            Self::LocalPath(_) => "file",
            Self::MappedPath { .. } => "mapped-path",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReferencedResource {
    pub reference: ResourceReference,
    pub mime: Option<String>,
    pub display_name: Option<String>,
    pub provenance: ReferenceProvenance,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ArtifactId(u64);

impl ArtifactId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
}

impl fmt::Display for ArtifactId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "artifact-{}", self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactHash(pub [u8; 32]);

impl ArtifactHash {
    pub fn sha256(bytes: &[u8]) -> Self {
        use sha2::{Digest, Sha256};
        Self(Sha256::digest(bytes).into())
    }

    pub fn hex(&self) -> String {
        self.0.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArtifactLifetime {
    Session,
    Temporary { ttl: Duration },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArtifactOrigin {
    #[default]
    OriginalResource,
    RenderedSnapshot,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactDescriptor {
    /// Legacy protocol descriptors identify supplied original resources.
    #[serde(default)]
    pub origin: ArtifactOrigin,
    pub id: ArtifactId,
    pub kind: ModalityKind,
    pub mime: String,
    pub size: u64,
    pub hash: ArtifactHash,
    pub display_name: Option<String>,
    pub lifetime: ArtifactLifetime,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PortableArtifact {
    pub descriptor: ArtifactDescriptor,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StaticVisualArtifact {
    pub descriptor: ArtifactDescriptor,
    pub source_region_only: bool,
    pub region: super::acquisition::ScreenRegion,
    pub quality: super::acquisition::CaptureQuality,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModalityResolution {
    ReferencedResource(ReferencedResource),
    OriginalArtifact(PortableArtifact),
    RenderedSnapshot(StaticVisualArtifact),
    LiveVisualState { reason: String },
    Unavailable { reason: String },
}

/// Resource resolution is independent of endpoint presence or user disposition.
pub type ModalityResource = ModalityResolution;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModalityDisposition {
    InspectReference,
    MaterializeOnHost,
    OpenSameHost,
    SendToEndpoint,
    Unavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeploymentTopology {
    Headless,
    SameHostEndpoint,
    RemoteEndpoint,
}

impl ModalityResolution {
    pub fn dispositions(&self, topology: DeploymentTopology) -> Vec<ModalityDisposition> {
        use ModalityDisposition::*;
        let mut result = match self {
            Self::ReferencedResource(_) => vec![InspectReference],
            Self::OriginalArtifact(_) | Self::RenderedSnapshot(_) => vec![MaterializeOnHost],
            _ => return vec![Unavailable],
        };
        // These are possible dispositions, not a claim that a handler is available.
        match topology {
            DeploymentTopology::Headless => {}
            DeploymentTopology::SameHostEndpoint => result.push(OpenSameHost),
            DeploymentTopology::RemoteEndpoint => result.push(SendToEndpoint),
        }
        result
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ModalityCapabilities {
    pub reference_handoff: bool,
    pub artifact_handoff: bool,
    pub static_visual_request: bool,
    pub live_external_fallback: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ModalityResolutionMetrics {
    pub resolutions: u64,
    pub reference_hits: u64,
    pub artifact_fallbacks: u64,
    pub unresolved: u64,
    pub live_fallback: u64,
}

impl ModalityResolutionMetrics {
    pub fn observe(&mut self, resolution: &ModalityResolution) {
        self.resolutions += 1;
        match resolution {
            ModalityResolution::ReferencedResource(_) => self.reference_hits += 1,
            ModalityResolution::OriginalArtifact(_) | ModalityResolution::RenderedSnapshot(_) => {
                self.artifact_fallbacks += 1;
            }
            ModalityResolution::LiveVisualState { .. } => self.live_fallback += 1,
            ModalityResolution::Unavailable { .. } => self.unresolved += 1,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalModality {
    pub id: ExternalModalityId,
    pub owner: RuntimeNodeId,
    pub kind: ModalityKind,
    pub label: Option<String>,
    pub resolution: ModalityResolution,
    pub capabilities: ModalityCapabilities,
    pub transfer_policy: TransferPolicy,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalModalityCapabilities {
    pub reference_schemes: HashSet<String>,
    pub mime_patterns: HashSet<String>,
    pub artifact_receive: bool,
}

impl LocalModalityCapabilities {
    pub fn supports_reference(&self, resource: &ReferencedResource) -> bool {
        self.reference_schemes.contains(resource.reference.scheme())
            && resource
                .mime
                .as_deref()
                .is_some_and(|mime| self.supports_mime(mime))
    }

    pub fn supports_mime(&self, mime: &str) -> bool {
        self.mime_patterns.contains(mime)
            || mime
                .split_once('/')
                .is_some_and(|(class, _)| self.mime_patterns.contains(&format!("{class}/*")))
    }
}
