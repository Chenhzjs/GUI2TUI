use super::{ExternalModality, ModalityCandidate, ModalityMetadata, ModalityResolver};
use crate::backend::AtspiBackend;

pub fn materialized_reference(
    artifact: &super::materialize::MaterializedArtifact,
) -> super::ReferencedResource {
    super::ReferencedResource {
        reference: super::ResourceReference::LocalPath(
            artifact.path().to_string_lossy().into_owned(),
        ),
        mime: Some(artifact.metadata.descriptor.mime.clone()),
        display_name: artifact.metadata.descriptor.display_name.clone(),
        provenance: super::ReferenceProvenance::LocalFileReference,
    }
}

/// Explicit one-shot acquisition. It cannot turn a reference request into a
/// download/capture, and cannot add pixel payloads to the semantic cache.
pub async fn acquire_snapshot(
    backend: &AtspiBackend,
    candidate: &ModalityCandidate,
    resource: &super::ModalityResource,
    provider: std::sync::Arc<dyn super::acquisition::StaticVisualAcquisitionProvider>,
    cancel: super::CancellationToken,
    metrics: &mut super::acquisition::ModalityMetrics,
) -> std::io::Result<(super::StaticVisualArtifact, Vec<u8>)> {
    use super::{
        ArtifactDescriptor, ArtifactHash, ArtifactId, ArtifactLifetime, ArtifactOrigin,
        acquisition::*,
    };
    if !permits_snapshot(resource, candidate.kind, true) {
        return Err(std::io::Error::other(
            "RenderedSnapshot not requested for an unresolved static Image; keep reference/original/live resource",
        ));
    }
    metrics.snapshot_attempt += 1;
    let result = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        let before = backend
            .static_visual_region(&candidate.locator)
            .await
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        let token = cancel.clone();
        let captured = tokio::task::spawn_blocking(move || provider.acquire(before, &token))
            .await
            .map_err(|_| std::io::Error::other("static provider task failed"))??;
        let after = backend
            .static_visual_region(&candidate.locator)
            .await
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        if before != after || captured.region != before.bounds || cancel.is_cancelled() {
            return Err(std::io::Error::other(
                "AcquisitionUnavailable: object moved, changed or request cancelled",
            ));
        }
        let descriptor = ArtifactDescriptor {
            origin: ArtifactOrigin::RenderedSnapshot,
            id: {
                static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
                ArtifactId::new(NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
            },
            kind: super::ModalityKind::Image,
            mime: "image/png".into(),
            size: captured.bytes.len() as u64,
            hash: ArtifactHash::sha256(&captured.bytes),
            display_name: Some("RenderedSnapshot — screen composite; may be occluded".into()),
            lifetime: ArtifactLifetime::Temporary {
                ttl: std::time::Duration::from_secs(300),
            },
        };
        metrics.capture_source_bytes += captured.capture_source_bytes;
        metrics.final_artifact_bytes += descriptor.size;
        Ok((
            super::StaticVisualArtifact {
                descriptor,
                source_region_only: true,
                region: before.bounds,
                quality: captured.quality,
            },
            captured.bytes,
        ))
    })
    .await
    .unwrap_or_else(|_| {
        cancel.cancel();
        Err(std::io::Error::other(
            "AcquisitionUnavailable: static acquisition deadline exceeded",
        ))
    });
    if result.is_ok() {
        metrics.snapshot_success += 1;
    } else {
        metrics.snapshot_unavailable += 1;
    }
    result
}

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
