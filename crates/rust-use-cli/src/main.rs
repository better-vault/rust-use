//! Command-line client for driving a `rust-use` app from an agent.

use clap::Parser;

/// Drive a rust-use app from an agent.
#[derive(Parser)]
#[command(name = "rust-use", version, arg_required_else_help = true)]
struct Cli {}

fn main() {
    Cli::parse();
}
