mod broker;
mod model;
mod resolver;
mod transport;

pub use broker::{
    AuthorizationDecision, BrokerError, HandlerRegistry, LocalHandler, LocalModalityBroker,
    LocalResource, PathMapping, ProcessHandler, RecordingHandler,
};
pub use model::{
    ArtifactDescriptor, ArtifactHash, ArtifactId, ArtifactLifetime, ExternalModality,
    ExternalModalityId, LocalModalityCapabilities, ModalityCapabilities, ModalityKind,
    ModalityResolution, ModalityResolutionMetrics, PortableArtifact, ReferenceProvenance,
    ReferencedResource, ResourceReference, StaticVisualArtifact, TransferPolicy,
};
pub use resolver::{
    ModalityCandidate, ModalityMetadata, ModalityResolver, ResolutionError,
    format_external_modality, redact_reference,
};
pub use transport::{ArtifactTransport, CancellationToken, HandoffMetrics, TransferError};
