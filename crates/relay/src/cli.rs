//! Command-line arguments.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

/// A local-first HTTP client. Without a subcommand, opens the Relay window.
#[derive(Debug, Parser)]
#[command(name = "relay", version)]
pub(crate) struct Cli {
    /// What to do instead of opening the window.
    #[command(subcommand)]
    pub(crate) command: Option<Command>,
}

/// The subcommands.
#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Send one request of a workspace and write the response body to stdout.
    Run(RunArgs),
}

const RUN_AFTER_HELP: &str = "\
Output:
  stdout  the response body, byte for byte; with --include, the status line and the
          headers first, then a blank line
  stderr  one `warning:` line per workspace load warning and per unresolved variable,
          then `<METHOD> <url> -> <code> <reason> in <ms> ms, <size>`

Exit codes:
  0  a response was received, whatever its status
  1  the request could not be executed: transport error, unsupported auth, environment
     not allowed for the collection, invalid GraphQL variables
  2  usage error: bad arguments, unknown request or environment, workspace that fails
     to load";

/// Arguments of `relay run`.
#[derive(Debug, Args)]
#[command(after_help = RUN_AFTER_HELP)]
pub(crate) struct RunArgs {
    /// The request to send: its id, or its slug path
    /// `<collection>/<folder>/.../<request>`.
    pub(crate) request: String,
    /// The workspace directory. Defaults to the current directory.
    #[arg(long, value_name = "DIR")]
    pub(crate) workspace: Option<PathBuf>,
    /// The slug of the environment to use. Without it, no environment applies.
    #[arg(long, value_name = "SLUG")]
    pub(crate) env: Option<String>,
    /// Sets a local variable, which overrides globals, collection and environment
    /// variables. Repeatable. Use it to supply secret values.
    #[arg(long = "var", value_name = "KEY=VALUE")]
    pub(crate) vars: Vec<String>,
    /// Write the status line and the response headers before the body.
    #[arg(short, long)]
    pub(crate) include: bool,
}
