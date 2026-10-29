//! Command-line arguments.

use std::path::PathBuf;

#[cfg(test)]
use clap::CommandFactory as _;
use clap::{Args, Parser, Subcommand};

const AFTER_HELP: &str = "\
Without DIR, Relay opens the current directory when it holds a relay.toml, and an empty
window otherwise. A directory named like a subcommand needs a path prefix: relay ./run";

/// A local-first HTTP client. Without a subcommand, opens the Relay window.
#[derive(Debug, Parser)]
#[command(
    name = "relay",
    version,
    args_conflicts_with_subcommands = true,
    after_help = AFTER_HELP
)]
pub(crate) struct Cli {
    /// The workspace directory to open in the window.
    #[arg(value_name = "DIR")]
    pub(crate) workspace: Option<PathBuf>,
    /// What to do instead of opening the window.
    #[command(subcommand)]
    pub(crate) command: Option<Command>,
}

/// The subcommands.
#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Send one request of a workspace and write the response body to stdout.
    Run(RunArgs),
    /// Serve the MCP tools that drive the running app, over stdio.
    Mcp(McpArgs),
}

/// The application id the window reports, and the default `--app-id`.
pub(crate) const DEFAULT_APP_ID: &str = "relay";

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

const MCP_AFTER_HELP: &str = "\
The server speaks MCP on stdout, so stdout carries only protocol frames; logs go to stderr.

Tools, all driving a running Relay window through its gpui-mcp bridge:
  ui_list_targets, ui_select_target, ui_ping
  ui_tree, ui_find, ui_element, ui_element_bounds, ui_wait_for_element
  ui_click, ui_type, ui_key, ui_focus, ui_scroll

Policy, approvals and audit are not implemented yet: every tool runs.

Exit codes:
  0  the client closed the connection
  1  the server could not start or the bridge could not be served
  2  usage error: bad arguments or an invalid application id";

/// Arguments of `relay mcp`.
#[derive(Debug, Args)]
#[command(after_help = MCP_AFTER_HELP)]
pub(crate) struct McpArgs {
    /// The workspace directory. The ui_* tools do not read it yet.
    #[arg(long, value_name = "DIR")]
    pub(crate) workspace: Option<PathBuf>,
    /// The application id of the windows to drive.
    #[arg(long, value_name = "ID", default_value = DEFAULT_APP_ID)]
    pub(crate) app_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Cli {
        match Cli::try_parse_from(args) {
            Ok(cli) => cli,
            Err(error) => panic!("{args:?}: {error}"),
        }
    }

    #[test]
    fn no_arguments_opens_the_window_without_a_directory() {
        let cli = parse(&["relay"]);
        assert!(cli.command.is_none());
        assert_eq!(cli.workspace, None);
    }

    #[test]
    fn a_positional_directory_opens_that_workspace() {
        let cli = parse(&["relay", "some/dir"]);
        assert!(cli.command.is_none());
        assert_eq!(cli.workspace, Some(PathBuf::from("some/dir")));
    }

    #[test]
    fn run_is_still_the_subcommand() {
        let cli = parse(&["relay", "run", "x"]);
        assert_eq!(cli.workspace, None);
        let Some(Command::Run(args)) = cli.command else {
            panic!("expected relay run");
        };
        assert_eq!(args.request, "x");
    }

    #[test]
    fn mcp_defaults_to_the_relay_app_id() {
        let cli = parse(&["relay", "mcp"]);
        let Some(Command::Mcp(args)) = cli.command else {
            panic!("expected relay mcp");
        };
        assert_eq!(args.app_id, DEFAULT_APP_ID);
        assert_eq!(args.workspace, None);
    }

    #[test]
    fn mcp_takes_an_app_id_and_a_workspace() {
        let cli = parse(&["relay", "mcp", "--app-id", "other", "--workspace", "w"]);
        let Some(Command::Mcp(args)) = cli.command else {
            panic!("expected relay mcp");
        };
        assert_eq!(args.app_id, "other");
        assert_eq!(args.workspace, Some(PathBuf::from("w")));
    }

    #[test]
    fn mcp_help_documents_its_tools_and_stream() {
        let help = Cli::command()
            .find_subcommand_mut("mcp")
            .map_or_else(String::new, |command| {
                command.render_long_help().to_string()
            });
        for expected in ["ui_tree", "ui_select_target", "stdout", "Exit codes"] {
            assert!(help.contains(expected), "{expected} missing: {help}");
        }
    }

    /// A directory literally named `run` is the subcommand; `./run` opens it.
    #[test]
    fn a_directory_named_run_needs_a_path_prefix() {
        assert!(Cli::try_parse_from(["relay", "run"]).is_err());
        let cli = parse(&["relay", "./run"]);
        assert_eq!(cli.workspace, Some(PathBuf::from("./run")));
    }

    #[test]
    fn help_documents_the_workspace_directory() {
        let help = Cli::command().render_long_help().to_string();
        assert!(help.contains("[DIR]"), "{help}");
        assert!(help.contains("./run"), "{help}");
    }
}
