//! Command-line client for driving a rust-use app.
mod cli;
mod commands;
mod state;

use clap::Parser;
use std::process::ExitCode;

fn main() -> ExitCode {
    match commands::run(cli::Cli::parse()) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(2)
        }
    }
}
