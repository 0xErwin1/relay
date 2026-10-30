//! The ReqFlux binary: the GUI when started without a subcommand, on the workspace given as
//! `reqflux [<DIR>]`, `reqflux run` to send one request from the command line, and
//! `reqflux mcp` to serve the agent tools, and `reqflux import` to bring in a Postman
//! collection.

mod cli;
mod gui;
mod import;
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
        Some(Command::Import(args)) => import::import(args),
    }
}
