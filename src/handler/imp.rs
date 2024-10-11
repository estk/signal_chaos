use super::*;
use tokio::signal::unix::{signal, Signal, SignalKind};

/// Signals for SIGINT, SIGTERM and SIGHUP on Unix.
#[derive(Debug)]
pub(super) struct Signals {
    sigint: SignalWithDone,
    sighup: SignalWithDone,
    sigterm: SignalWithDone,
    sigtstp: SignalWithDone,
    sigcont: SignalWithDone,
}

impl Signals {
    pub(super) fn new() -> std::io::Result<Self> {
        let sigint = SignalWithDone::new(SignalKind::interrupt())?;
        let sighup = SignalWithDone::new(SignalKind::hangup())?;
        let sigterm = SignalWithDone::new(SignalKind::terminate())?;
        let sigtstp = SignalWithDone::new(SignalKind::from_raw(libc::SIGTSTP))?;
        let sigcont = SignalWithDone::new(SignalKind::from_raw(libc::SIGCONT))?;

        Ok(Self {
            sigint,
            sighup,
            sigterm,
            sigtstp,
            sigcont,
        })
    }

    pub(super) async fn recv(&mut self) -> Option<SignalEvent> {
        loop {
            tokio::select! {
                recv = self.sigint.signal.recv(), if !self.sigint.done => {
                    match recv {
                        Some(()) => break Some(SignalEvent::Shutdown(ShutdownEvent::Interrupt)),
                        None => self.sigint.done = true,
                    }
                }
                recv = self.sighup.signal.recv(), if !self.sighup.done => {
                    match recv {
                        Some(()) => break Some(SignalEvent::Shutdown(ShutdownEvent::Hangup)),
                        None => self.sighup.done = true,
                    }
                }
                recv = self.sigterm.signal.recv(), if !self.sigterm.done => {
                    match recv {
                        Some(()) => break Some(SignalEvent::Shutdown(ShutdownEvent::Term)),
                        None => self.sigterm.done = true,
                    }
                }
                recv = self.sigtstp.signal.recv(), if !self.sigtstp.done => {
                    match recv {
                        Some(()) => break Some(SignalEvent::JobControl(JobControlEvent::Stop)),
                        None => self.sigtstp.done = true,
                    }
                }
                recv = self.sigcont.signal.recv(), if !self.sigcont.done => {
                    match recv {
                        Some(()) => break Some(SignalEvent::JobControl(JobControlEvent::Continue)),
                        None => self.sigcont.done = true,
                    }
                }
                else => {
                    break None
                }
            }
        }
    }
}

#[derive(Debug)]
struct SignalWithDone {
    signal: Signal,
    done: bool,
}

impl SignalWithDone {
    fn new(kind: SignalKind) -> std::io::Result<Self> {
        let signal = signal(kind)?;
        Ok(Self {
            signal,
            done: false,
        })
    }
}
