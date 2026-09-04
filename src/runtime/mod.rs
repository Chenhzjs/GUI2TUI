//! Deployment lifetime, separate from the frozen semantic/content IR.
//! Only the TUI/runtime owner mutates this registry. Workers receive tickets
//! and cooperative cancellation tokens, never mutable semantic state.
pub mod artifacts;
pub mod signals;

use crate::{modality::CancellationToken, semantic::BackendLocator};
use serde::Serialize;
use std::{
    collections::HashMap,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, serde::Deserialize)]
pub struct RuntimeSessionId(String);

impl Default for RuntimeSessionId {
    fn default() -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        Self(format!(
            "{}-{stamp:x}-{}",
            std::process::id(),
            NEXT_SESSION.fetch_add(1, Ordering::Relaxed)
        ))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
pub struct ApplicationGenerationId(pub u64);
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
pub struct OperationId(pub u64);
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
pub struct EndpointProfileId(pub String);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum SessionState {
    Starting,
    Running,
    Degraded,
    Suspended,
    Stopping,
    Stopped,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum TerminalState {
    Attached,
    Detached,
    Reattaching,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum EndpointState {
    Unavailable,
    Connecting,
    Available,
    Disconnected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, thiserror::Error)]
pub enum RuntimeError {
    #[error("The operation belongs to an expired application generation.")]
    StaleIdentity,
    #[error("Application is no longer available.")]
    TargetGone,
    #[error("Accessibility service is temporarily unavailable.")]
    BackendUnavailable,
    #[error("The operation exceeded its time limit.")]
    Timeout,
    #[error("This semantic operation is not supported.")]
    Unsupported,
    #[error("The requested capability is unavailable.")]
    CapabilityUnavailable,
    #[error("Permission was denied.")]
    PermissionDenied,
    #[error("Operation cancelled.")]
    Cancelled,
    #[error("Local modality endpoint disconnected.")]
    EndpointLost,
    #[error("Artifact integrity verification failed.")]
    IntegrityFailure,
    #[error("Invalid protocol response.")]
    ProtocolViolation,
    #[error("Resource unavailable.")]
    ResourceUnavailable,
    #[error("Internal runtime failure.")]
    Internal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum OperationKind {
    Search,
    Materialization,
    SnapshotAcquisition,
    ArtifactTransfer,
    ReferenceHandoff,
    TextInteraction,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OperationTicket {
    session: RuntimeSessionId,
    generation: ApplicationGenerationId,
    id: OperationId,
}
impl OperationTicket {
    pub fn session_id(&self) -> RuntimeSessionId {
        self.session.clone()
    }
    pub fn operation_id(&self) -> u64 {
        self.id.0
    }
}
struct Operation {
    ticket: OperationTicket,
    cancel: CancellationToken,
    _kind: OperationKind,
}

#[derive(Default, Debug, Serialize)]
pub struct RuntimeMetrics {
    pub application_generations: u64,
    pub application_restarts: u64,
    pub terminal_detaches: u64,
    pub terminal_resumes: u64,
    pub endpoint_disconnects: u64,
    pub endpoint_reconnects: u64,
    pub backend_reconnects: u64,
    pub backend_losses: u64,
    pub backend_reconnect_attempts: u64,
    pub owner_loss_cancellations: u64,
    pub rejected_late_results: u64,
}

/// Configuration-ready bounds; no dependency on application/toolkit names.
pub struct RuntimeLimits {
    pub backend_operation: Duration,
    pub lifecycle_probe: Duration,
    pub operation_capacity: usize,
}
impl Default for RuntimeLimits {
    fn default() -> Self {
        Self {
            backend_operation: Duration::from_secs(5),
            lifecycle_probe: Duration::from_secs(2),
            operation_capacity: 32,
        }
    }
}

pub struct RuntimeSession {
    pub id: RuntimeSessionId,
    pub state: SessionState,
    pub terminal: TerminalState,
    pub endpoint: EndpointState,
    pub endpoint_profile: Option<EndpointProfileId>,
    pub metrics: RuntimeMetrics,
    started: Instant,
    generation: Option<ApplicationGenerationId>,
    application: Option<BackendLocator>,
    operations: HashMap<OperationId, Operation>,
    next_operation: u64,
}
impl Default for RuntimeSession {
    fn default() -> Self {
        Self {
            id: RuntimeSessionId::default(),
            state: SessionState::Starting,
            terminal: TerminalState::Attached,
            endpoint: EndpointState::Unavailable,
            endpoint_profile: None,
            metrics: RuntimeMetrics::default(),
            started: Instant::now(),
            generation: None,
            application: None,
            operations: HashMap::new(),
            next_operation: 1,
        }
    }
}
impl RuntimeSession {
    /// Called only after an explicit selection and a NEW cache construction.
    pub fn open_application(&mut self, locator: BackendLocator) -> ApplicationGenerationId {
        self.invalidate_application();
        self.metrics.application_generations += 1;
        self.metrics.application_restarts = self.metrics.application_generations.saturating_sub(1);
        let generation = ApplicationGenerationId(self.metrics.application_generations);
        self.generation = Some(generation);
        self.application = Some(locator);
        self.state = SessionState::Running;
        generation
    }
    pub fn generation(&self) -> Option<ApplicationGenerationId> {
        self.generation
    }
    pub fn validates_application(&self, locator: &BackendLocator) -> bool {
        self.application.as_ref() == Some(locator)
    }
    pub fn invalidate_application(&mut self) {
        for (_, operation) in self.operations.drain() {
            operation.cancel.cancel();
            self.metrics.owner_loss_cancellations += 1;
        }
        self.generation = None;
        self.application = None;
        self.state = SessionState::Degraded;
    }
    pub fn begin(
        &mut self,
        kind: OperationKind,
        cancel: CancellationToken,
    ) -> Result<OperationTicket, RuntimeError> {
        if matches!(self.state, SessionState::Stopping | SessionState::Stopped) {
            return Err(RuntimeError::Cancelled);
        }
        let generation = self.generation.ok_or(RuntimeError::TargetGone)?;
        if self.operations.len() >= RuntimeLimits::default().operation_capacity {
            return Err(RuntimeError::CapabilityUnavailable);
        }
        let ticket = OperationTicket {
            session: self.id.clone(),
            generation,
            id: OperationId(self.next_operation),
        };
        self.next_operation += 1;
        self.operations.insert(
            ticket.id,
            Operation {
                ticket: ticket.clone(),
                cancel,
                _kind: kind,
            },
        );
        Ok(ticket)
    }
    pub fn complete(&mut self, ticket: &OperationTicket) -> Result<(), RuntimeError> {
        if ticket.session != self.id || Some(ticket.generation) != self.generation {
            self.metrics.rejected_late_results += 1;
            return Err(RuntimeError::StaleIdentity);
        }
        match self.operations.remove(&ticket.id) {
            Some(op) if op.ticket == *ticket && !op.cancel.is_cancelled() => Ok(()),
            _ => {
                self.metrics.rejected_late_results += 1;
                Err(RuntimeError::Cancelled)
            }
        }
    }
    pub fn set_terminal_attached(&mut self, attached: bool) {
        let next = if attached {
            TerminalState::Attached
        } else {
            TerminalState::Detached
        };
        if self.terminal != next {
            if attached {
                self.metrics.terminal_resumes += 1;
            } else {
                self.metrics.terminal_detaches += 1;
            }
            self.terminal = next;
        }
    }
    pub fn begin_terminal_reattach(&mut self) {
        self.terminal = TerminalState::Reattaching;
    }
    pub fn set_endpoint(&mut self, state: EndpointState) {
        if state == EndpointState::Disconnected && self.endpoint != state {
            self.metrics.endpoint_disconnects += 1;
            self.operations.retain(|_, operation| {
                if matches!(
                    operation._kind,
                    OperationKind::ArtifactTransfer | OperationKind::ReferenceHandoff
                ) {
                    operation.cancel.cancel();
                    self.metrics.owner_loss_cancellations += 1;
                    false
                } else {
                    true
                }
            });
        }
        if state == EndpointState::Connecting && self.endpoint == EndpointState::Disconnected {
            self.metrics.endpoint_reconnects += 1;
        }
        self.endpoint = state;
    }
    pub fn record_backend_reconnect(&mut self) {
        self.metrics.backend_reconnects += 1;
    }
    pub fn record_backend_loss(&mut self) {
        self.metrics.backend_losses += 1;
    }
    pub fn record_backend_reconnect_attempt(&mut self) {
        self.metrics.backend_reconnect_attempts += 1;
    }
    pub fn shutdown(&mut self) {
        self.state = SessionState::Stopping;
        self.invalidate_application();
        self.state = SessionState::Stopped;
    }
    /// Deliberately excludes labels, locators, text, URIs and submitted values.
    pub fn status(&self) -> serde_json::Value {
        serde_json::json!({"session": self.id, "state": self.state, "generation": self.generation,
            "terminal": self.terminal, "endpoint": self.endpoint, "active_operations": self.operations.len(),
            "uptime_seconds": self.started.elapsed().as_secs(), "metrics": self.metrics})
    }
}
impl Drop for RuntimeSession {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn app() -> BackendLocator {
        BackendLocator::new(":1.2", "/app")
    }
    #[test]
    fn independent_sessions_and_twenty_generations_reject_late_results() {
        let mut session = RuntimeSession::default();
        assert_ne!(session.id, RuntimeSession::default().id);
        for generation in 1..=20 {
            assert_eq!(
                session.open_application(app()),
                ApplicationGenerationId(generation)
            );
            let cancel = CancellationToken::default();
            let ticket = session
                .begin(OperationKind::Search, cancel.clone())
                .unwrap();
            session.invalidate_application();
            assert!(cancel.is_cancelled());
            assert_eq!(session.complete(&ticket), Err(RuntimeError::StaleIdentity));
        }
        assert_eq!(session.metrics.owner_loss_cancellations, 20);
        assert_eq!(session.metrics.rejected_late_results, 20);
    }
    #[test]
    fn cancelled_ticket_cannot_complete_twice_or_in_new_generation() {
        let mut s = RuntimeSession::default();
        s.open_application(app());
        let t = s
            .begin(OperationKind::Materialization, Default::default())
            .unwrap();
        assert_eq!(s.complete(&t), Ok(()));
        assert_eq!(s.complete(&t), Err(RuntimeError::Cancelled));
        s.open_application(app());
        assert_eq!(s.complete(&t), Err(RuntimeError::StaleIdentity));
    }
    #[test]
    fn detach_keeps_generation_and_no_endpoint_is_legal() {
        let mut s = RuntimeSession::default();
        let g = s.open_application(app());
        s.set_terminal_attached(false);
        assert_eq!(s.generation(), Some(g));
        s.set_terminal_attached(true);
        assert_eq!(s.metrics.terminal_resumes, 1);
        assert_eq!(s.state, SessionState::Running);
        assert_eq!(s.endpoint, EndpointState::Unavailable);
    }
    #[test]
    fn bounded_registry_shutdown_cancels_every_owner() {
        let mut s = RuntimeSession::default();
        s.open_application(app());
        let cancel = CancellationToken::default();
        for _ in 0..32 {
            s.begin(OperationKind::ArtifactTransfer, cancel.clone())
                .unwrap();
        }
        assert_eq!(
            s.begin(OperationKind::Search, Default::default()),
            Err(RuntimeError::CapabilityUnavailable)
        );
        s.shutdown();
        assert!(cancel.is_cancelled());
        assert_eq!(s.state, SessionState::Stopped);
    }

    #[test]
    fn endpoint_reconnect_is_counted_at_explicit_connect_attempt() {
        let mut session = RuntimeSession::default();
        session.set_endpoint(EndpointState::Connecting);
        session.set_endpoint(EndpointState::Available);
        assert_eq!(session.metrics.endpoint_reconnects, 0);
        session.set_endpoint(EndpointState::Disconnected);
        session.set_endpoint(EndpointState::Connecting);
        session.set_endpoint(EndpointState::Available);
        assert_eq!(session.metrics.endpoint_disconnects, 1);
        assert_eq!(session.metrics.endpoint_reconnects, 1);
    }

    #[test]
    fn endpoint_loss_cancels_transfer_and_rejects_late_success_after_reconnect() {
        let mut session = RuntimeSession::default();
        session.open_application(BackendLocator::new(":1.2", "/app"));
        session.set_endpoint(EndpointState::Available);
        let cancel = CancellationToken::default();
        let old = session
            .begin(OperationKind::ArtifactTransfer, cancel.clone())
            .unwrap();
        session.set_endpoint(EndpointState::Disconnected);
        assert!(cancel.is_cancelled());
        session.set_endpoint(EndpointState::Connecting);
        session.set_endpoint(EndpointState::Available);
        assert!(session.complete(&old).is_err());
        assert_eq!(session.metrics.rejected_late_results, 1);
        let fresh = session
            .begin(
                OperationKind::ArtifactTransfer,
                CancellationToken::default(),
            )
            .unwrap();
        assert!(session.complete(&fresh).is_ok());
    }
}
