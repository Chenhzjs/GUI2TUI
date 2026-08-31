pub mod acquisition;
mod broker;
pub mod materialize;
mod model;
mod resolver;
pub mod runtime;
mod transport;
pub mod wire;

pub use broker::{
    AuthorizationDecision, BrokerError, HandlerRegistry, LocalHandler, LocalModalityBroker,
    LocalResource, PathMapping, ProcessHandler, RecordingHandler,
};
pub use model::{
    ArtifactDescriptor, ArtifactHash, ArtifactId, ArtifactLifetime, ArtifactOrigin,
    DeploymentTopology, ExternalModality, ExternalModalityId, LocalModalityCapabilities,
    ModalityCapabilities, ModalityDisposition, ModalityKind, ModalityResolution,
    ModalityResolutionMetrics, ModalityResource, PortableArtifact, ReferenceProvenance,
    ReferencedResource, ResourceReference, StaticVisualArtifact, TransferPolicy,
};
pub use resolver::{
    ModalityCandidate, ModalityMetadata, ModalityResolver, ResolutionError,
    format_external_modality, redact_reference,
};
pub use transport::{ArtifactTransport, CancellationToken, HandoffMetrics, TransferError};
