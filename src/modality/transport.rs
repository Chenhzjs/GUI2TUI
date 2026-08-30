use std::{
    fs::{self, File},
    io::{Read, Write},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use sha2::{Digest, Sha256};
use thiserror::Error;

use super::{
    ArtifactDescriptor, ArtifactHash, AuthorizationDecision, BrokerError, LocalModalityBroker,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HandoffMetrics {
    pub reference_hits: u64,
    pub artifact_fallbacks: u64,
    pub unresolved: u64,
    pub live_fallback: u64,
    pub reference_only_handoffs: u64,
    pub artifact_bytes: u64,
    pub authorization_denied: u64,
    pub transfer_cancelled: u64,
    pub handler_unavailable: u64,
}

#[derive(Clone, Default)]
pub struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

#[derive(Debug, Error)]
pub enum TransferError {
    #[error(transparent)]
    Broker(#[from] BrokerError),
    #[error("artifact exceeds configured maximum size of {0} bytes")]
    TooLarge(u64),
    #[error("artifact transfer was cancelled")]
    Cancelled,
    #[error("artifact transfer exceeded configured timeout of {0:?}")]
    Timeout(Duration),
    #[error("artifact length mismatch: expected {expected}, received {actual}")]
    LengthMismatch { expected: u64, actual: u64 },
    #[error("artifact SHA-256 mismatch")]
    HashMismatch,
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[derive(Clone, Debug)]
pub struct ArtifactTransport {
    pub max_size: u64,
    pub chunk_size: usize,
    pub timeout: Duration,
}

impl Default for ArtifactTransport {
    fn default() -> Self {
        Self {
            max_size: 512 * 1024 * 1024,
            chunk_size: 64 * 1024,
            timeout: Duration::from_secs(300),
        }
    }
}

impl ArtifactTransport {
    pub fn transfer<R: Read>(
        &self,
        broker: &mut LocalModalityBroker,
        descriptor: &ArtifactDescriptor,
        mut payload: R,
        authorization: AuthorizationDecision,
        cancellation: &CancellationToken,
    ) -> Result<u64, TransferError> {
        if descriptor.size > self.max_size {
            return Err(TransferError::TooLarge(self.max_size));
        }
        // Control plane authorization is completed before the first payload
        // read or temporary partial file creation.
        broker.authorize_artifact(descriptor, authorization)?;
        let partial = broker.artifact_partial_path(descriptor);
        let complete = broker.artifact_complete_path(descriptor);
        let result = self.copy_verified(descriptor, &mut payload, &partial, cancellation);
        let bytes = match result {
            Ok(bytes) => bytes,
            Err(error) => {
                let _ = fs::remove_file(&partial);
                if matches!(error, TransferError::Cancelled) {
                    broker.mark_cancelled();
                }
                return Err(error);
            }
        };
        fs::rename(&partial, &complete)?;
        if let Err(error) = broker.finish_artifact(descriptor, complete.clone(), bytes) {
            let _ = fs::remove_file(complete);
            return Err(error.into());
        }
        Ok(bytes)
    }

    fn copy_verified<R: Read>(
        &self,
        descriptor: &ArtifactDescriptor,
        payload: &mut R,
        partial: &std::path::Path,
        cancellation: &CancellationToken,
    ) -> Result<u64, TransferError> {
        let mut output = File::create(partial)?;
        let started = Instant::now();
        let mut hasher = Sha256::new();
        let mut received = 0_u64;
        let mut buffer = vec![0_u8; self.chunk_size.max(1)];
        loop {
            if cancellation.is_cancelled() {
                return Err(TransferError::Cancelled);
            }
            if started.elapsed() > self.timeout {
                return Err(TransferError::Timeout(self.timeout));
            }
            let count = payload.read(&mut buffer)?;
            if started.elapsed() > self.timeout {
                return Err(TransferError::Timeout(self.timeout));
            }
            if count == 0 {
                break;
            }
            received = received.saturating_add(count as u64);
            if received > descriptor.size || received > self.max_size {
                return Err(TransferError::LengthMismatch {
                    expected: descriptor.size,
                    actual: received,
                });
            }
            hasher.update(&buffer[..count]);
            output.write_all(&buffer[..count])?;
        }
        output.flush()?;
        if received != descriptor.size {
            return Err(TransferError::LengthMismatch {
                expected: descriptor.size,
                actual: received,
            });
        }
        let actual = ArtifactHash(hasher.finalize().into());
        if actual != descriptor.hash {
            return Err(TransferError::HashMismatch);
        }
        Ok(received)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashSet,
        io::Cursor,
        sync::atomic::{AtomicU64, Ordering},
        time::Duration,
    };

    use crate::modality::{
        ArtifactId, ArtifactLifetime, HandlerRegistry, LocalModalityCapabilities, LocalResource,
        ModalityKind, RecordingHandler,
    };

    use super::*;

    fn broker_and_descriptor(
        bytes: &[u8],
    ) -> (
        LocalModalityBroker,
        ArtifactDescriptor,
        RecordingHandler,
        std::path::PathBuf,
    ) {
        let recording = RecordingHandler::default();
        let mut registry = HandlerRegistry::default();
        registry.register("image/*", Box::new(recording.clone()));
        let capabilities = LocalModalityCapabilities {
            reference_schemes: HashSet::new(),
            mime_patterns: HashSet::from(["image/*".to_owned()]),
            artifact_receive: true,
        };
        static NEXT_TEST_ROOT: AtomicU64 = AtomicU64::new(1);
        let root = std::env::temp_dir().join(format!(
            "gui2tui-artifact-{}-{}-{}",
            std::process::id(),
            bytes.len(),
            NEXT_TEST_ROOT.fetch_add(1, Ordering::Relaxed),
        ));
        let broker = LocalModalityBroker::new(capabilities, registry, &root).unwrap();
        let descriptor = ArtifactDescriptor {
            id: ArtifactId::new(bytes.len() as u64 + 1),
            kind: ModalityKind::Image,
            mime: "image/svg+xml".to_owned(),
            size: bytes.len() as u64,
            hash: ArtifactHash::sha256(bytes),
            display_name: Some("../../evil.sh".to_owned()),
            lifetime: ArtifactLifetime::Temporary {
                ttl: Duration::from_secs(300),
            },
        };
        (broker, descriptor, recording, root)
    }

    #[test]
    fn deny_occurs_before_payload_read_and_transfers_zero_bytes() {
        struct PanicReader;
        impl Read for PanicReader {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                panic!("payload must not be read before authorization")
            }
        }
        let (mut broker, descriptor, _, root) = broker_and_descriptor(b"image");
        assert!(matches!(
            ArtifactTransport::default().transfer(
                &mut broker,
                &descriptor,
                PanicReader,
                AuthorizationDecision::Deny,
                &CancellationToken::default()
            ),
            Err(TransferError::Broker(BrokerError::Denied))
        ));
        assert_eq!(broker.metrics().artifact_bytes, 0);
        drop(broker);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn verified_artifact_uses_generated_name_and_cleanup() {
        let bytes = b"<svg xmlns='http://www.w3.org/2000/svg'/>";
        let (mut broker, descriptor, recording, root) = broker_and_descriptor(bytes);
        let transferred = ArtifactTransport::default()
            .transfer(
                &mut broker,
                &descriptor,
                Cursor::new(bytes),
                AuthorizationDecision::Once,
                &CancellationToken::default(),
            )
            .unwrap();
        assert_eq!(transferred, bytes.len() as u64);
        let invocations = recording.invocations();
        let LocalResource::Path(path) = &invocations[0].0 else {
            panic!("expected local path")
        };
        assert!(
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("artifact-")
        );
        assert!(!path.display().to_string().contains("evil.sh"));
        assert!(path.exists());
        broker.cleanup();
        assert!(!path.exists());
        drop(broker);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn cancellation_removes_partial_and_does_not_invoke_handler() {
        let bytes = b"some image bytes";
        let (mut broker, descriptor, recording, root) = broker_and_descriptor(bytes);
        let cancellation = CancellationToken::default();
        cancellation.cancel();
        assert!(matches!(
            ArtifactTransport::default().transfer(
                &mut broker,
                &descriptor,
                Cursor::new(bytes),
                AuthorizationDecision::Once,
                &cancellation,
            ),
            Err(TransferError::Cancelled)
        ));
        assert!(recording.invocations().is_empty());
        assert_eq!(broker.metrics().transfer_cancelled, 1);
        drop(broker);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn cancellation_during_transfer_removes_partial() {
        struct CancellingReader {
            bytes: Cursor<Vec<u8>>,
            cancellation: CancellationToken,
            reads: usize,
        }
        impl Read for CancellingReader {
            fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
                self.reads += 1;
                let count = self.bytes.read(buffer)?;
                if self.reads == 1 {
                    self.cancellation.cancel();
                }
                Ok(count)
            }
        }
        let bytes = vec![7_u8; 256];
        let (mut broker, descriptor, recording, root) = broker_and_descriptor(&bytes);
        let cancellation = CancellationToken::default();
        let transport = ArtifactTransport {
            chunk_size: 8,
            ..Default::default()
        };
        let reader = CancellingReader {
            bytes: Cursor::new(bytes),
            cancellation: cancellation.clone(),
            reads: 0,
        };
        assert!(matches!(
            transport.transfer(
                &mut broker,
                &descriptor,
                reader,
                AuthorizationDecision::Once,
                &cancellation,
            ),
            Err(TransferError::Cancelled)
        ));
        assert!(!broker.artifact_partial_path(&descriptor).exists());
        assert!(recording.invocations().is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn integrity_mismatch_deletes_partial() {
        let bytes = b"image";
        let (mut broker, mut descriptor, _, root) = broker_and_descriptor(bytes);
        descriptor.hash = ArtifactHash::sha256(b"different");
        assert!(matches!(
            ArtifactTransport::default().transfer(
                &mut broker,
                &descriptor,
                Cursor::new(bytes),
                AuthorizationDecision::Once,
                &CancellationToken::default(),
            ),
            Err(TransferError::HashMismatch)
        ));
        assert!(!broker.artifact_partial_path(&descriptor).exists());
        drop(broker);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn cooperative_timeout_removes_partial() {
        struct SlowReader;
        impl Read for SlowReader {
            fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
                std::thread::sleep(Duration::from_millis(3));
                buffer[0] = 1;
                Ok(1)
            }
        }
        let (mut broker, descriptor, recording, root) = broker_and_descriptor(&[1]);
        let transport = ArtifactTransport {
            timeout: Duration::from_millis(1),
            ..Default::default()
        };
        assert!(matches!(
            transport.transfer(
                &mut broker,
                &descriptor,
                SlowReader,
                AuthorizationDecision::Once,
                &CancellationToken::default(),
            ),
            Err(TransferError::Timeout(_))
        ));
        assert!(!broker.artifact_partial_path(&descriptor).exists());
        assert!(recording.invocations().is_empty());
        let _ = fs::remove_dir_all(root);
    }
}
