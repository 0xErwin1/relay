//! `reqflux secret`: set, unset and list secret variable values in the OS keyring
//! (keyring-secrets S6). The commands take the store as a trait object, so tests run them
//! against a [`MemorySecretStore`](reqflux_engine::MemorySecretStore); `main` passes the
//! keyring. No message ever contains a value.

use std::io::{self, IsTerminal as _, Read};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use reqflux_core::model::{Variable, Workspace};
use reqflux_engine::{Engine, KeyringSecretStore, SecretKey, SecretScope, SecretStore};

use crate::cli::{SecretArgs, SecretCommand, SecretKeyArgs, SecretListArgs};

/// Why a `reqflux secret` command stopped, with its exit code.
#[derive(Debug)]
pub(crate) enum Failure {
    /// Exit code 2: the arguments, the scope or key, the value, or the workspace.
    Usage(String),
    /// Exit code 1: the keyring failed or is unavailable.
    Store(String),
}

/// What `reqflux secret list` prints: `lines` on stdout, `warnings` on stderr.
#[derive(Debug, Default)]
pub(crate) struct Listing {
    pub(crate) lines: Vec<String>,
    pub(crate) warnings: Vec<String>,
}

/// Runs `reqflux secret` against the OS keyring and returns its exit code.
pub(crate) fn secret(args: SecretArgs) -> ExitCode {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("error")).init();
    match dispatch(args.command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(Failure::Usage(message)) => {
            eprintln!("error: {message}");
            ExitCode::from(2)
        }
        Err(Failure::Store(message)) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}

fn dispatch(command: SecretCommand) -> Result<(), Failure> {
    match command {
        SecretCommand::Set(args) => {
            let engine = open(args.workspace.clone())?;
            let workspace = engine.workspace();
            // Check the scope and key before waiting on stdin.
            secret_key(workspace, &args.scope, &args.key)?;
            let value = prompt_value(&args)?;
            set(
                workspace,
                engine.secret_store().as_ref(),
                &args.scope,
                &args.key,
                &value,
            )
        }
        SecretCommand::Unset(args) => {
            let engine = open(args.workspace.clone())?;
            unset(
                engine.workspace(),
                engine.secret_store().as_ref(),
                &args.scope,
                &args.key,
            )
        }
        SecretCommand::List(SecretListArgs { workspace }) => {
            let engine = open(workspace)?;
            let listing = list(engine.workspace(), engine.secret_store().as_ref());
            for warning in &listing.warnings {
                eprintln!("warning: {warning}");
            }
            for line in &listing.lines {
                println!("{line}");
            }
            Ok(())
        }
    }
}

/// Loads the workspace at `root`, the current directory by default, with the OS keyring.
fn open(root: Option<PathBuf>) -> Result<Engine, Failure> {
    let root = root.unwrap_or_else(|| PathBuf::from("."));
    let engine = Engine::open_with_secrets(root, Arc::new(KeyringSecretStore))
        .map_err(|error| Failure::Usage(error.to_string()))?;
    for warning in engine.warnings() {
        eprintln!("warning: {warning}");
    }
    Ok(engine)
}

/// Reads the value without echo from a terminal, after a prompt on stderr, or else all of
/// stdin.
fn prompt_value(args: &SecretKeyArgs) -> Result<String, Failure> {
    let unreadable = |error: io::Error| Failure::Usage(format!("cannot read the value: {error}"));
    if io::stdin().is_terminal() {
        eprint!("Value for {}: ", args.key);
        rpassword::read_password().map_err(unreadable)
    } else {
        read_value(io::stdin().lock())
    }
}

/// All of `input`, without one trailing newline (`\n` or `\r\n`).
pub(crate) fn read_value(mut input: impl Read) -> Result<String, Failure> {
    let mut value = String::new();
    input
        .read_to_string(&mut value)
        .map_err(|error| Failure::Usage(format!("cannot read the value from stdin: {error}")))?;
    if value.ends_with('\n') {
        value.pop();
        if value.ends_with('\r') {
            value.pop();
        }
    }
    Ok(value)
}

/// Stores `value` for the secret variable `key` declared in `scope`.
pub(crate) fn set(
    workspace: &Workspace,
    store: &dyn SecretStore,
    scope: &str,
    key: &str,
    value: &str,
) -> Result<(), Failure> {
    let secret = secret_key(workspace, scope, key)?;
    if value.is_empty() {
        return Err(Failure::Usage(format!("the value for {key} is empty")));
    }
    store
        .set(&secret, value)
        .map_err(|error| Failure::Store(error.to_string()))
}

/// Deletes the value of the secret variable `key` declared in `scope`; a value that is not
/// set is not an error.
pub(crate) fn unset(
    workspace: &Workspace,
    store: &dyn SecretStore,
    scope: &str,
    key: &str,
) -> Result<(), Failure> {
    let secret = secret_key(workspace, scope, key)?;
    store
        .delete(&secret)
        .map_err(|error| Failure::Store(error.to_string()))
}

/// One `<scope>\t<key>\t<set|not set|unknown>` line per secret variable: globals, then
/// collections, then environments, each by slug. A store error makes the line `unknown`
/// and adds a warning once per distinct error.
pub(crate) fn list(workspace: &Workspace, store: &dyn SecretStore) -> Listing {
    let mut listing = Listing::default();
    for (label, scope, variables) in scopes(workspace) {
        for variable in variables.iter().filter(|variable| variable.secret) {
            let key = SecretKey {
                workspace_id: workspace.manifest.id,
                scope,
                key: variable.key.clone(),
            };
            let state = match store.is_set(&key) {
                Ok(true) => "set",
                Ok(false) => "not set",
                Err(error) => {
                    let warning = format!("could not read the keyring: {error}");
                    if !listing.warnings.contains(&warning) {
                        listing.warnings.push(warning);
                    }
                    "unknown"
                }
            };
            listing
                .lines
                .push(format!("{label}\t{}\t{state}", variable.key));
        }
    }
    listing
}

/// Every variable scope of `workspace` with its CLI label: `global`, the collections by
/// slug, then `env:<slug>` by slug.
fn scopes(workspace: &Workspace) -> Vec<(String, SecretScope, &[Variable])> {
    let mut collections: Vec<_> = workspace.collections.iter().collect();
    collections.sort_by(|a, b| a.slug.cmp(&b.slug));
    let mut environments: Vec<_> = workspace.environments.iter().collect();
    environments.sort_by(|a, b| a.slug.cmp(&b.slug));
    let global = (
        "global".to_owned(),
        SecretScope::Global,
        workspace.manifest.globals.as_slice(),
    );
    std::iter::once(global)
        .chain(collections.into_iter().map(|node| {
            (
                node.slug.clone(),
                SecretScope::Collection(node.collection.id),
                node.collection.variables.as_slice(),
            )
        }))
        .chain(environments.into_iter().map(|node| {
            (
                format!("env:{}", node.slug),
                SecretScope::Environment(node.environment.id),
                node.environment.variables.as_slice(),
            )
        }))
        .collect()
}

/// The keyring key of the secret variable `key` declared in `scope`: `global`, a collection
/// slug or `env:<slug>`.
fn secret_key(workspace: &Workspace, scope: &str, key: &str) -> Result<SecretKey, Failure> {
    let (secret_scope, variables) = match scope.strip_prefix("env:") {
        _ if scope == "global" => (SecretScope::Global, workspace.manifest.globals.as_slice()),
        Some(slug) => workspace
            .environments
            .iter()
            .find(|node| node.slug == slug)
            .map(|node| {
                (
                    SecretScope::Environment(node.environment.id),
                    node.environment.variables.as_slice(),
                )
            })
            .ok_or_else(|| unknown_scope(scope))?,
        None => workspace
            .collections
            .iter()
            .find(|node| node.slug == scope)
            .map(|node| {
                (
                    SecretScope::Collection(node.collection.id),
                    node.collection.variables.as_slice(),
                )
            })
            .ok_or_else(|| unknown_scope(scope))?,
    };
    match variables.iter().find(|variable| variable.key == key) {
        Some(variable) if variable.secret => Ok(SecretKey {
            workspace_id: workspace.manifest.id,
            scope: secret_scope,
            key: key.to_owned(),
        }),
        Some(_) => Err(Failure::Usage(format!(
            "{key} in {scope} is not a secret variable"
        ))),
        None => Err(Failure::Usage(format!(
            "{scope} has no secret variable {key}"
        ))),
    }
}

fn unknown_scope(scope: &str) -> Failure {
    Failure::Usage(format!(
        "unknown scope {scope}: expected global, a collection slug or env:<slug>"
    ))
}

#[cfg(test)]
mod tests;
