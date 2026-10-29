//! `relay mcp`: the MCP server that drives the running app (S1).

use std::process::ExitCode;

use relay_mcp::AppId;
use relay_mcp::bridge::default_endpoint_dir;
use relay_mcp::server::serve;

use crate::cli::McpArgs;

/// Why `relay mcp` stopped, with its exit code.
#[derive(Debug)]
enum Failure {
    /// Exit code 2: the arguments or the application id.
    Usage(String),
    /// Exit code 1: the server could not start or serve.
    Execution(String),
}

/// Runs `relay mcp` and returns its exit code.
pub(crate) fn mcp(args: McpArgs) -> ExitCode {
    // stdout carries the MCP frames, so the log goes to stderr.
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    match run(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(Failure::Usage(message)) => {
            eprintln!("error: {message}");
            ExitCode::from(2)
        }
        Err(Failure::Execution(message)) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}

/// Serves the tools until the client disconnects.
fn run(args: McpArgs) -> Result<(), Failure> {
    let app_id = AppId::new(&args.app_id)
        .map_err(|error| Failure::Usage(format!("invalid application id: {error}")))?;
    if let Some(workspace) = &args.workspace {
        log::debug!(
            "the ui_* tools do not read the workspace {}",
            workspace.display()
        );
    }
    let endpoint_dir =
        default_endpoint_dir().map_err(|error| Failure::Execution(error.to_string()))?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| {
            Failure::Execution(format!("could not start the async runtime: {error}"))
        })?;
    runtime
        .block_on(serve(endpoint_dir, app_id))
        .map_err(|error| Failure::Execution(error.to_string()))
}
