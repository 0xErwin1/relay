//! The Relay binary: the GUI when started without a subcommand, on the workspace given as
//! `relay [<DIR>]`, and `relay run` to send one
//! request from the command line. Later, the `mcp` and `import` subcommands.

mod cli;
mod gui;
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
    }
}
