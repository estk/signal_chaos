//! Yoink https://github.com/nextest-rs/nextest/blob/main/nextest-runner/src/signal.rs#L33

use nix::sys::signal::Signal;
use thiserror::Error;

mod imp;

#[derive(Debug)]
pub struct SignalHandler {
    signals: Option<imp::Signals>,
}

impl SignalHandler {
    /// Creates a new `SignalHandler` that handles Ctrl-C and other signals.
    pub fn new() -> Result<Self, SignalHandlerSetupError> {
        let signals = imp::Signals::new()?;
        Ok(Self {
            signals: Some(signals),
        })
    }

    /// Creates a new `SignalReceiver` that does nothing.
    pub(crate) fn noop() -> Self {
        Self { signals: None }
    }

    pub async fn recv(&mut self) -> Option<SignalEvent> {
        match &mut self.signals {
            Some(signals) => signals.recv().await,
            None => None,
        }
    }
}

#[derive(Debug, Error)]
#[error("error setting up signal handler")]
pub struct SignalHandlerSetupError(#[from] std::io::Error);

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SignalEvent {
    #[cfg(unix)]
    JobControl(JobControlEvent),
    Shutdown(ShutdownEvent),
}
impl SignalEvent {
    pub fn as_nix_sig(&self) -> Signal {
        match self {
            Self::JobControl(JobControlEvent::Stop) => Signal::SIGSTOP,
            Self::JobControl(JobControlEvent::Continue) => Signal::SIGCONT,
            Self::Shutdown(ShutdownEvent::Hangup) => Signal::SIGHUP,
            Self::Shutdown(ShutdownEvent::Interrupt) => Signal::SIGINT,
            Self::Shutdown(ShutdownEvent::Term) => Signal::SIGTERM,
        }
    }
}

// A job-control related signal event.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum JobControlEvent {
    #[cfg(unix)]
    Stop,
    #[cfg(unix)]
    Continue,
}

// A signal event that should cause a shutdown to happen.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ShutdownEvent {
    #[cfg(unix)]
    Hangup,
    #[cfg(unix)]
    Term,
    Interrupt,
}
