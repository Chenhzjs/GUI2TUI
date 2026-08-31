//! Unix signal policy lives outside semantic runtime and renderer code.
use std::io;
use tokio::signal::unix::{Signal, SignalKind, signal};
pub enum RuntimeSignal {
    Stop,
    Detach,
    Reattach,
}
pub struct RuntimeSignals {
    interrupt: Signal,
    terminate: Signal,
    detach: Signal,
    reattach: Signal,
}
impl RuntimeSignals {
    pub fn new() -> io::Result<Self> {
        Ok(Self {
            interrupt: signal(SignalKind::interrupt())?,
            terminate: signal(SignalKind::terminate())?,
            detach: signal(SignalKind::user_defined1())?,
            reattach: signal(SignalKind::user_defined2())?,
        })
    }
    pub async fn recv(&mut self) -> RuntimeSignal {
        tokio::select! {
            _ = self.interrupt.recv() => RuntimeSignal::Stop,
            _ = self.terminate.recv() => RuntimeSignal::Stop,
            _ = self.detach.recv() => RuntimeSignal::Detach,
            _ = self.reattach.recv() => RuntimeSignal::Reattach,
        }
    }
}
