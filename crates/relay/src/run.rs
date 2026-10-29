//! `relay run`: send one request and print the response (S11).

use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use relay_engine::{Engine, ExecuteError, ExecuteOptions, Response};

use crate::cli::RunArgs;

/// Why `relay run` stopped, with its exit code.
#[derive(Debug)]
enum Failure {
    /// Exit code 2: the arguments, the request or environment name, or the workspace.
    Usage(String),
    /// Exit code 1: the request could not be executed.
    Execution(String),
}

impl From<ExecuteError> for Failure {
    fn from(error: ExecuteError) -> Self {
        if error.is_usage() {
            Self::Usage(error.to_string())
        } else {
            Self::Execution(error.to_string())
        }
    }
}

/// Runs `relay run` and returns its exit code.
pub(crate) fn run(args: RunArgs) -> ExitCode {
    // Load warnings are printed below in their documented format, not through the logger.
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("error")).init();
    match send(args) {
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

fn send(args: RunArgs) -> Result<(), Failure> {
    let vars = args
        .vars
        .iter()
        .map(|var| parse_var(var))
        .collect::<Result<Vec<_>, _>>()?;
    let root = args.workspace.unwrap_or_else(|| PathBuf::from("."));
    let engine = Engine::open(root).map_err(|error| Failure::Usage(error.to_string()))?;
    for warning in engine.warnings() {
        eprintln!("warning: {warning}");
    }

    let options = ExecuteOptions {
        environment: args.env,
        vars,
    };
    let execution = engine.execute(&args.request, &options)?;
    for name in &execution.unresolved {
        eprintln!("warning: unresolved variable {{{{{name}}}}}");
    }
    let (method, url) = (execution.method.clone(), execution.url.clone());
    let response = engine.wait(execution)?;

    write_response(&response, args.include)
        .map_err(|error| Failure::Execution(format!("could not write the response: {error}")))?;
    eprintln!(
        "{method} {url} -> {} in {} ms, {}",
        status(&response),
        response.elapsed.as_millis(),
        size(response.size())
    );
    Ok(())
}

/// Splits `<key>=<value>` at the first `=`.
fn parse_var(var: &str) -> Result<(String, String), Failure> {
    var.split_once('=')
        .map(|(key, value)| (key.to_owned(), value.to_owned()))
        .ok_or_else(|| Failure::Usage(format!("invalid --var {var}: expected <key>=<value>")))
}

/// Writes the body bytes unchanged, after the status line and headers with `include`.
fn write_response(response: &Response, include: bool) -> io::Result<()> {
    let mut stdout = io::stdout().lock();
    if include {
        writeln!(stdout, "HTTP/{} {}", response.version, status(response))?;
        for (name, value) in &response.headers {
            writeln!(stdout, "{name}: {value}")?;
        }
        writeln!(stdout)?;
    }
    stdout.write_all(&response.body)?;
    stdout.flush()
}

/// `<code> <reason>`, or the code alone when it has no reason phrase.
fn status(response: &Response) -> String {
    match &response.reason {
        Some(reason) => format!("{} {reason}", response.status),
        None => response.status.to_string(),
    }
}

/// `<n> B`, `<n.n> KB` or `<n.n> MB`, 1024 based.
fn size(bytes: usize) -> String {
    const KB: f64 = 1024.0;
    let value = bytes as f64;
    if value < KB {
        format!("{bytes} B")
    } else if value < KB * KB {
        format!("{:.1} KB", value / KB)
    } else {
        format!("{:.1} MB", value / (KB * KB))
    }
}

#[cfg(test)]
mod tests {
    use super::size;

    #[test]
    fn sizes_are_1024_based() {
        assert_eq!(size(0), "0 B");
        assert_eq!(size(1023), "1023 B");
        assert_eq!(size(1024), "1.0 KB");
        assert_eq!(size(1536), "1.5 KB");
        assert_eq!(size(1024 * 1024), "1.0 MB");
        assert_eq!(size(5 * 1024 * 1024 + 512 * 1024), "5.5 MB");
    }
}
