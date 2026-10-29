//! The Relay binary: the GUI when started without a subcommand, on the workspace given as
//! `relay [<DIR>]`, `relay run` to send one request from the command line, and
//! `relay mcp` to serve the agent tools. Later, the `import` subcommand.

mod cli;
mod gui;
mod mcp;
mod run;
mod signals;

use std::process::ExitCode;

use clap::Parser;

use cli::{Cli, Command};

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        None => gui::open(cli.workspace),
        Some(Command::Run(args)) => run::run(args),
        Some(Command::Mcp(args)) => mcp::mcp(args),
    }
}
