//! The execution pipeline, collection runner, history and secrets.
//!
//! This is the single API the UI, the MCP server and the CLI use.
//!
//! # Executing a request
//!
//! [`Engine::open`] loads a workspace. [`Engine::prepare`] turns a request of it into a
//! [`PreparedRequest`](relay_http::PreparedRequest): it selects the request (by id or slug
//! path), checks the environment, resolves `{{variables}}`, builds the query string, applies
//! the effective auth, builds the body and fills in default settings. [`prepare`] is the same
//! step as a pure function, with no IO and no network. [`prepare_draft`] does the same for
//! a draft request document in the place of the stored one, keeping its parents.
//! [`Engine::send`] sends a prepared request; [`Engine::execute`] does both, after running
//! the request's effective pre-request script (see the `Scripting` section of
//! `ARCHITECTURE.md`): the most specific script on the request, its folders or the
//! collection, whose variable writes apply to that execution's scopes only.
//! [`Engine::wait`] runs the effective test script once the response arrives and returns
//! its results beside the response in [`Executed::scripts`].
//!
//! # Threading model
//!
//! The [`Engine`] owns a multi-thread Tokio runtime and every request runs on it as a spawned
//! task. [`Engine::send`] returns an [`Execution`] holding the task's
//! [`JoinHandle`](tokio::task::JoinHandle), which is an executor-agnostic future: a GPUI
//! front end awaits it on its own executor, so no GPUI code runs on Tokio and no HTTP code
//! runs on GPUI's executor. Synchronous callers such as the CLI use [`Engine::wait`] or
//! [`Engine::execute_blocking`], which block the calling thread and must not be called from
//! inside an async context. Dropping the engine shuts the runtime down, so it must not be
//! dropped from inside an async context either.
//!
//! # App state
//!
//! [`state::AppState`] is the per-user SQLite app state (drafts today, history later), at
//! `$XDG_STATE_HOME/relay/state.sqlite3`.

mod engine;
mod error;
mod import;
mod prepare;
mod scripts;
mod select;
pub mod state;

pub use engine::{Engine, Executed, Execution};
pub use error::{ExecuteError, ImportError, OpenError, SaveError};
pub use import::{ImportSummary, postman};
pub use prepare::{
    DEFAULT_TIMEOUT_MS, ExecuteOptions, Inspection, Prepared, Span, inspect, prepare, prepare_draft,
};
/// The transport types front ends read, re-exported so they depend on the engine only.
pub use relay_http::{HttpError, PreparedBody, PreparedRequest, Response};
pub use relay_script::{
    ConsoleEntry, ConsoleLevel, ScriptOutcome, TestResult, VariableScope, VariableWrite,
};
/// The token type of [`Execution::cancel`], re-exported so front ends can hold it.
pub use tokio_util::sync::CancellationToken;
