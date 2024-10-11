use std::{env, thread::sleep, time::Duration};

use async_scoped::TokioScope;
use clap::Parser;
use nix::unistd::Pid;
use signal_chaos::SignalHandler;
use tokio::{process::Command, sync::broadcast};
use tracing::info;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[derive(Parser, Debug)]
struct Cli {
    #[clap(env = "CHAOS_WORKER", short, long)]
    worker: bool,
}

fn main() {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    info!("cli: {cli:?}");
    if cli.worker {
        println!("i am worker");
        sleep(Duration::from_secs(10));
    } else {
        println!("i am manager");
        manager()
    }
}
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
                        println!("sending {event:?}");
                        event_tx_ref.send(event).unwrap();
                    }
                    _ = cancellation_receiver.recv() => {
                        break;
                    }
                };
            }
            println!("exiting event loop");
        };
        scope.spawn_cancellable(event_fut, || ());

        let worker_fut = async move {
            let mut event_rx = event_tx_ref.subscribe();
            let mut cmd = Command::new(exe);
            let mut jh = cmd.arg("-w").spawn().unwrap();
            // let id = jh.id().unwrap();

            loop {
                tokio::select! {
                    evt = event_rx.recv() => {
                        // send this sig to the current processes group
                        let pid = Pid::from_raw(0);
                        let sig = evt.unwrap().as_nix_sig();

                        println!("killing {pid:?}");
                        nix::sys::signal::kill(pid, sig).unwrap();
                    }
                    _ = jh.wait() => {
                        println!("wait done");
                        cancellation_sender.send(());
                        break
                    }
                };
            }
            println!("exiting worker manager loop");
        };
        scope.spawn_cancellable(worker_fut, || ());
    });
}
