mod config;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// a tool that rolls out artifacts based on configurable patterns
#[derive(Parser)]
struct Cli {
    /// sets the config
    #[arg(short, long, value_name = "path")]
    config: Option<PathBuf>,

    /// turns detailed logs on
    #[arg(short, long)]
    debug: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// list what the tool will do
    Plan,
    /// rollout the artifacts
    Roll,
}

fn main() {
    let cli = Cli::parse();
    let cfg = config::load_config(cli.config);
}
