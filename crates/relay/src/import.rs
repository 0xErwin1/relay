//! `relay import`: a Postman collection and its environments into a workspace (S10).

use std::fs;
use std::path::Path;
use std::process::ExitCode;

use relay_engine::{ImportError, ImportSummary, postman};

use crate::cli::ImportArgs;

/// Why `relay import` stopped, with its exit code.
#[derive(Debug)]
enum Failure {
    /// Exit code 2: the arguments or a path that cannot be read.
    Usage(String),
    /// Exit code 1: a document that does not import, or a target file that exists.
    Document(String),
}

/// Runs `relay import` and returns its exit code.
pub(crate) fn import(args: ImportArgs) -> ExitCode {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    match run(&args) {
        Ok(summary) => {
            print_summary(&summary);
            ExitCode::SUCCESS
        }
        Err(Failure::Usage(message)) => {
            eprintln!("error: {message}");
            ExitCode::from(2)
        }
        Err(Failure::Document(message)) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}

/// Reads the documents, imports them, and returns the summary.
fn run(args: &ImportArgs) -> Result<ImportSummary, Failure> {
    let root = match &args.workspace {
        Some(workspace) => workspace.clone(),
        None => std::env::current_dir().map_err(|error| {
            Failure::Usage(format!("cannot read the current directory: {error}"))
        })?,
    };
    let collection = read_json(&args.file)?;
    let environments = args
        .environments
        .iter()
        .map(|path| read_json(path))
        .collect::<Result<Vec<_>, _>>()?;
    postman(&root, &collection, &environments).map_err(|error| match error {
        ImportError::Postman(_) | ImportError::Exists { .. } => {
            Failure::Document(error.to_string())
        }
        ImportError::Store { .. } => Failure::Document(error.to_string()),
    })
}

/// Reads one JSON document, naming the path when it cannot be read or parsed.
fn read_json(path: &Path) -> Result<serde_json::Value, Failure> {
    let bytes = fs::read(path)
        .map_err(|error| Failure::Usage(format!("cannot read {}: {error}", path.display())))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| Failure::Document(format!("{} is not JSON: {error}", path.display())))
}

fn print_summary(summary: &ImportSummary) {
    println!("workspace {}", summary.root.display());
    println!(
        "collection {}: {} folders, {} requests, {} environments",
        summary.collection, summary.folders, summary.requests, summary.environments
    );
    for warning in &summary.warnings {
        eprintln!("warning: {warning}");
    }
}
