use std::{
    env,
    io::{self, BufRead},
    path::Path,
    process::Stdio,
    thread,
    time::Duration,
};

use async_scoped::TokioScope;
use clap::Parser;
use nix::unistd::Pid;
use signal_chaos::SignalHandler;
use tokio::{io::AsyncWriteExt, process::Command, sync::broadcast};
use tracing::{debug, info, instrument};
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    prelude::*,
    EnvFilter,
};

#[derive(Parser, Debug)]
struct Cli {
    #[clap(env = "CHAOS_WORKER", short, long)]
    worker: bool,
}

fn main() {
    tracing_subscriber::registry()
        .with(fmt::layer().with_span_events(FmtSpan::ACTIVE))
        .with(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    info!("cli: {cli:?}");

    if cli.worker {
        worker();
    } else {
        manager()
    }
}

#[instrument]
fn worker() {
    let mut buf = String::new();

    let (timeout_tx, timeout_rx) = crossbeam::channel::bounded(0);

    thread::spawn(move || {
        loop {
            let mut stdin = io::stdin().lock();

            let read_count = stdin.read_line(&mut buf).unwrap();
            if read_count == 0 {
                continue;
            }
            let bs = buf.as_bytes();
            let sig = read_sig(bs);

            info!("read signal: {sig}");
            if sig == libc::SIGINT {
                break;
            }
        }
        timeout_tx.send(libc::SIGINT).unwrap()
    });

    let sig = timeout_rx.recv_timeout(Duration::from_secs(10));
    info!("signal: {sig:?}");
}

#[instrument]
fn manager() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _rtg = rt.enter();
    let exe = env::current_exe().unwrap();
    let mut handler = SignalHandler::new().unwrap();
    let handler_ref = &mut handler;

    let (event_tx, _event_rx) = broadcast::channel(10);
    let event_tx_ref = &event_tx;
    TokioScope::scope_and_block(move |scope| {
        let (cancellation_sender, mut cancellation_receiver) = broadcast::channel(1);

        let event_fut = async move {
            let mut done = false;
            loop {
                tokio::select! {
                    evt_opt = handler_ref.recv() => {
                        let event = match evt_opt {
                            Some(e) if !done => e,
                            Some(_) => continue,
                            None => {
                                done = true;
                                continue;
                            }
                        };
                        info!("sending {event:?}");
                        event_tx_ref.send(event).unwrap();
                    }
                    _ = cancellation_receiver.recv() => {
                        break;
                    }
                };
            }
        };
        scope.spawn_cancellable(event_fut, || ());

        let spawn_fut = async move {
            let mut event_rx = event_tx_ref.subscribe();
            let mut jh = spawn_child(&exe);

            let pid = Pid::from_raw(jh.id().unwrap().try_into().unwrap());
            let mut child_stdin = jh.stdin.take().unwrap();

            loop {
                tokio::select! {
                    evt = event_rx.recv() => {
                        let sig = evt.unwrap().as_sig();

                        // Here we just write the signal to the stdin of the child process.
                        // This is the simplest way of communicating this without actually passing a signal
                        // which would require the worker to implement signal handling as well.

                        debug!("forwarding signal to {pid:?}");

                        let line = sig_to_line_bs(sig);
                        child_stdin.write_all(&line).await.unwrap();
                        child_stdin.flush().await.unwrap();
                    }

                    _ = jh.wait() => {
                        cancellation_sender.send(()).unwrap();
                        break
                    }
                };
            }
        };
        scope.spawn_cancellable(spawn_fut, || ());
    });
}

fn read_sig(bs: &[u8]) -> i32 {
    let end = size_of::<i32>();
    let mut buf = 0_i32.to_ne_bytes();
    buf.copy_from_slice(&bs[..end]);
    i32::from_ne_bytes(buf)
}

fn sig_to_line_bs(sig: i32) -> Vec<u8> {
    let mut sbs = sig.to_ne_bytes().to_vec();
    sbs.push(b'\n');
    sbs
}

fn spawn_child(exe: &Path) -> tokio::process::Child {
    let mut cmd = Command::new(exe);

    // We set gid to 0 so that it gets its own group and does not get signals sent to the root process group.
    let jh = cmd
        .arg("-w")
        .process_group(0)
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    jh
}
