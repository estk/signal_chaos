use clap::Parser;
use tracing::info;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[derive(Parser, Debug)]
struct Cli {
    #[clap(env = "CHAOS_RUNNER", short, long)]
    runner: bool,
}

fn main() {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    info!("cli: {cli:?}");
}
