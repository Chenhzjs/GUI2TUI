//! Unix signal policy lives outside semantic runtime and renderer code.
use std::io;
use tokio::signal::unix::{Signal, SignalKind, signal};
pub enum RuntimeSignal {
    Stop,
    Detach,
    Reattach,
    Suspend,
    Resume,
}
pub struct RuntimeSignals {
    interrupt: Signal,
    terminate: Signal,
    detach: Signal,
    reattach: Signal,
    suspend: Signal,
    resume: Signal,
}
impl RuntimeSignals {
    pub fn new() -> io::Result<Self> {
        Ok(Self {
            interrupt: signal(SignalKind::interrupt())?,
            terminate: signal(SignalKind::terminate())?,
            detach: signal(SignalKind::user_defined1())?,
            reattach: signal(SignalKind::user_defined2())?,
            suspend: signal(SignalKind::from_raw(rustix::process::Signal::TSTP.as_raw()))?,
            resume: signal(SignalKind::from_raw(rustix::process::Signal::CONT.as_raw()))?,
        })
    }
    pub async fn recv(&mut self) -> RuntimeSignal {
        tokio::select! {
            _ = self.interrupt.recv() => RuntimeSignal::Stop,
            _ = self.terminate.recv() => RuntimeSignal::Stop,
            _ = self.detach.recv() => RuntimeSignal::Detach,
            _ = self.reattach.recv() => RuntimeSignal::Reattach,
            _ = self.suspend.recv() => RuntimeSignal::Suspend,
            _ = self.resume.recv() => RuntimeSignal::Resume,
        }
    }
}

/// Stop only after terminal state and input ownership have been released.
/// SIGCONT resumes this call; the Tokio SIGCONT stream then records the event.
pub fn suspend_current_process() -> io::Result<()> {
    rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::STOP)?;
    Ok(())
}
