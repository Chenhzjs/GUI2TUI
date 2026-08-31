//! Static acquisition contract. No toolkit names, capture protocol, or cache mutation.
use serde::{Deserialize, Serialize};
use std::io;

use super::{ArtifactOrigin, CancellationToken, ModalityKind, ModalityResource};

pub const MAX_ARTIFACT_BYTES: u64 = 32 * 1024 * 1024;
pub const MAX_REGION_PIXELS: u64 = 8 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScreenRegion {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SemanticVisualRegion {
    pub bounds: ScreenRegion,
    /// Native top-level client bounds, independently checked by the provider.
    pub window: ScreenRegion,
    pub process_id: u32,
}

impl ScreenRegion {
    pub fn overlaps(self, other: Self) -> bool {
        self.width > 0
            && self.height > 0
            && other.width > 0
            && other.height > 0
            && i64::from(self.x) < i64::from(other.x) + i64::from(other.width)
            && i64::from(other.x) < i64::from(self.x) + i64::from(self.width)
            && i64::from(self.y) < i64::from(other.y) + i64::from(other.height)
            && i64::from(other.y) < i64::from(self.y) + i64::from(self.height)
    }
    /// Screen-pixel coordinates only. Never silently clip or rescale.
    pub fn validate(self, source_width: i32, source_height: i32) -> io::Result<()> {
        if self.x < 0
            || self.y < 0
            || self.width <= 0
            || self.height <= 0
            || self
                .x
                .checked_add(self.width)
                .is_none_or(|x| x > source_width)
            || self
                .y
                .checked_add(self.height)
                .is_none_or(|y| y > source_height)
            || self.width as u64 * self.height as u64 > MAX_REGION_PIXELS
        {
            return Err(io::Error::other(
                "AcquisitionUnavailable: offscreen, clipped, invalid or oversized region",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CaptureQuality {
    CleanWindowCapture,
    /// Only the pixels currently composited at the requested coordinates.
    /// Another window may occlude them. This is NOT an isolated object image.
    CompositedScreenSnapshot,
    UnknownCaptureQuality,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AcquisitionCapabilities {
    pub available: bool,
    pub quality: CaptureQuality,
    pub limitation: &'static str,
}

/// Bytes deliberately have no Debug/Serialize implementation; never log pixels.
pub struct AcquiredVisual {
    pub bytes: Vec<u8>,
    pub region: ScreenRegion,
    pub quality: CaptureQuality,
    pub capture_source_bytes: u64,
}

pub trait StaticVisualAcquisitionProvider: Send + Sync {
    fn capabilities(&self) -> AcquisitionCapabilities;
    fn acquire(
        &self,
        region: SemanticVisualRegion,
        cancel: &CancellationToken,
    ) -> io::Result<AcquiredVisual>;
}

#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct ModalityMetrics {
    pub reference_resolved: u64,
    pub original_artifact: u64,
    pub snapshot_attempt: u64,
    pub snapshot_success: u64,
    pub snapshot_unavailable: u64,
    pub capture_source_bytes: u64,
    pub final_artifact_bytes: u64,
    pub headless_materialization: u64,
    pub same_host_open: u64,
    pub remote_transfer: u64,
}

/// This gate runs before any pixel/provider call. Candidate roles are separately
/// checked against live AT-SPI metadata (an Image that became a Button is refused).
pub fn permits_snapshot(resource: &ModalityResource, kind: ModalityKind, explicit: bool) -> bool {
    explicit
        && kind == ModalityKind::Image
        && matches!(resource, ModalityResource::Unavailable { .. })
}

pub fn origin_of(resource: &ModalityResource) -> Option<ArtifactOrigin> {
    match resource {
        ModalityResource::OriginalArtifact(_) => Some(ArtifactOrigin::OriginalResource),
        ModalityResource::RenderedSnapshot(_) => Some(ArtifactOrigin::RenderedSnapshot),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modality::{DeploymentTopology, ModalityDisposition};
    #[test]
    fn static_acquisition_is_explicit_image_only_and_never_live() {
        let unresolved = ModalityResource::Unavailable {
            reason: "no reference".into(),
        };
        assert!(permits_snapshot(&unresolved, ModalityKind::Image, true));
        assert!(!permits_snapshot(&unresolved, ModalityKind::Image, false));
        for kind in [
            ModalityKind::Document,
            ModalityKind::Video,
            ModalityKind::LiveVisual,
            ModalityKind::Unknown,
        ] {
            assert!(!permits_snapshot(&unresolved, kind, true));
        }
        assert!(!permits_snapshot(
            &ModalityResource::LiveVisualState {
                reason: "live".into()
            },
            ModalityKind::Image,
            true
        ));
    }
    #[test]
    fn coordinates_reject_clipping_offscreen_overflow_and_oversize() {
        let r = ScreenRegion {
            x: 10,
            y: 10,
            width: 100,
            height: 50,
        };
        assert!(r.validate(1280, 800).is_ok());
        for bad in [
            ScreenRegion { x: -1, ..r },
            ScreenRegion { x: i32::MAX, ..r },
            ScreenRegion { width: 0, ..r },
            ScreenRegion {
                width: 10000,
                height: 10000,
                ..r
            },
        ] {
            assert!(bad.validate(1280, 800).is_err());
        }
    }
    #[test]
    fn headless_original_is_materializable_without_endpoint() {
        let resource = ModalityResource::OriginalArtifact(crate::modality::PortableArtifact {
            descriptor: crate::modality::ArtifactDescriptor {
                origin: ArtifactOrigin::OriginalResource,
                id: crate::modality::ArtifactId::new(1),
                kind: ModalityKind::Image,
                mime: "image/png".into(),
                size: 0,
                hash: crate::modality::ArtifactHash::sha256(b""),
                display_name: None,
                lifetime: crate::modality::ArtifactLifetime::Session,
            },
        });
        assert_eq!(
            resource.dispositions(DeploymentTopology::Headless),
            vec![ModalityDisposition::MaterializeOnHost]
        );
        assert!(!permits_snapshot(&resource, ModalityKind::Image, true));
    }

    #[test]
    fn resource_provenance_and_topology_are_independent() {
        use crate::modality::*;
        let reference = ModalityResource::ReferencedResource(ReferencedResource {
            reference: ResourceReference::NetworkUri("https://example.invalid/diagram.png".into()),
            mime: Some("image/png".into()),
            display_name: None,
            provenance: ReferenceProvenance::HyperlinkUri,
        });
        assert_eq!(
            reference.dispositions(DeploymentTopology::Headless),
            vec![ModalityDisposition::InspectReference]
        );
        assert!(
            reference
                .dispositions(DeploymentTopology::SameHostEndpoint)
                .contains(&ModalityDisposition::OpenSameHost)
        );
        assert!(
            reference
                .dispositions(DeploymentTopology::RemoteEndpoint)
                .contains(&ModalityDisposition::SendToEndpoint)
        );
        assert!(!permits_snapshot(&reference, ModalityKind::Image, true));
        let rendered = ModalityResource::RenderedSnapshot(StaticVisualArtifact {
            descriptor: ArtifactDescriptor {
                origin: ArtifactOrigin::RenderedSnapshot,
                id: ArtifactId::new(1),
                kind: ModalityKind::Image,
                mime: "image/png".into(),
                size: 0,
                hash: ArtifactHash::sha256(b""),
                display_name: None,
                lifetime: ArtifactLifetime::Session,
            },
            source_region_only: true,
            region: ScreenRegion {
                x: 0,
                y: 0,
                width: 1,
                height: 1,
            },
            quality: CaptureQuality::CompositedScreenSnapshot,
        });
        assert_eq!(origin_of(&rendered), Some(ArtifactOrigin::RenderedSnapshot));
        assert_eq!(
            rendered.dispositions(DeploymentTopology::Headless),
            vec![ModalityDisposition::MaterializeOnHost]
        );
        assert!(!permits_snapshot(&rendered, ModalityKind::Image, true));
    }

    #[test]
    fn semantic_overlap_is_detected_without_clipping_or_position_guessing() {
        let image = ScreenRegion {
            x: 0,
            y: 0,
            width: 480,
            height: 180,
        };
        assert!(image.overlaps(ScreenRegion {
            x: 0,
            y: 0,
            width: 480,
            height: 25
        }));
        assert!(!image.overlaps(ScreenRegion {
            x: 0,
            y: 180,
            width: 480,
            height: 25
        }));
        assert!(!image.overlaps(ScreenRegion {
            x: 0,
            y: 0,
            width: 0,
            height: 25
        }));
    }
}
