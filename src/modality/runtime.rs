use super::{ExternalModality, ModalityCandidate, ModalityMetadata, ModalityResolver};
use crate::backend::AtspiBackend;

/// On-demand only: keep expensive metadata probes out of ordinary bootstrap.
pub async fn resolve_atspi(
    backend: &AtspiBackend,
    candidate: &ModalityCandidate,
) -> ExternalModality {
    let mut metadata = Vec::new();
    for locator in &candidate.evidence_locators {
        if let Ok(probe) = backend.probe_modality_metadata(locator).await {
            metadata.push(ModalityMetadata {
                accessible_attributes: probe.accessible_attributes,
                document_attributes: probe.document_attributes,
                hyperlink_uris: probe.hyperlink_uris,
            });
        }
    }
    ModalityResolver::default().resolve(candidate, &metadata)
}
