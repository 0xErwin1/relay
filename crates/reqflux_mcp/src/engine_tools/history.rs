//! `history_list` and `history_get` (S5): the window's history of the open workspace,
//! read from the per-user app state. History is recorded masked; the request document is
//! masked again here because it keeps literal auth values as written (S7).

use reqflux_core::model::Request;
use reqflux_engine::SentBody;
use reqflux_engine::state::{AppState, HistoryEntry, HistoryOutcome};
use serde_json::{Value, json};

use super::request::masked;
use super::{EngineState, EntryArgs, HistoryListArgs, cut, encode};
use crate::tools::ToolError;

/// How many entries `history_list` returns unless told otherwise.
const DEFAULT_LIMIT: u32 = 20;

/// The most entries `history_list` returns.
const MAX_LIMIT: u32 = 200;

impl EngineState {
    /// The app state and the open workspace's id. Only the history tools need the app
    /// state, so failing to open it fails them alone.
    fn history_source(&mut self) -> Result<(AppState, String), ToolError> {
        let workspace_id = self.engine()?.workspace().manifest.id.to_string();
        let opened = match &self.state_path {
            Some(path) => AppState::open(path),
            None => AppState::open_default(),
        };
        let app = opened
            .map_err(|error| ToolError::Engine(format!("could not open the app state: {error}")))?;
        Ok((app, workspace_id))
    }
}

fn state_error(error: impl std::fmt::Display) -> ToolError {
    ToolError::Engine(format!("could not read the history: {error}"))
}

/// `history_list`: the newest `limit` entries of the open workspace, newest first.
///
/// # Errors
///
/// No open workspace, or the app state cannot open or be read.
pub fn history_list(state: &mut EngineState, args: HistoryListArgs) -> Result<String, ToolError> {
    let (app, workspace_id) = state.history_source()?;
    let limit = args.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
    let entries = app
        .history(&workspace_id, usize::try_from(limit).unwrap_or(usize::MAX))
        .map_err(state_error)?;
    let entries: Vec<Value> = entries.iter().map(summary).collect();
    encode(&json!({ "entries": entries }))
}

/// `history_get`: one entry of the open workspace with its request and response.
///
/// # Errors
///
/// No open workspace, the app state cannot open or be read, or no entry of the open
/// workspace has this id.
pub fn history_get(state: &mut EngineState, args: EntryArgs) -> Result<String, ToolError> {
    let (app, workspace_id) = state.history_source()?;
    let entry = app
        .history_entry(args.entry_id)
        .map_err(state_error)?
        .filter(|entry| entry.workspace_id == workspace_id)
        .ok_or_else(|| {
            ToolError::Engine(format!(
                "history entry {} not found in the open workspace",
                args.entry_id
            ))
        })?;
    encode(&detail(&entry))
}

fn summary(entry: &HistoryEntry) -> Value {
    let record = &entry.record;
    let (status, error, elapsed_ms) = match &record.outcome {
        HistoryOutcome::Response(response) => (Some(response.status), None, response.elapsed_ms),
        HistoryOutcome::Failed { error, elapsed_ms } => (None, Some(error), *elapsed_ms),
    };
    json!({
        "entry_id": entry.id,
        "sent_at": record.sent_at,
        "request_id": record.request_id,
        "request_name": record.request_name,
        "request_path": record.request_path,
        "method": record.sent.method,
        "url": record.sent.url,
        "status": status,
        "elapsed_ms": elapsed_ms,
        "error": error,
    })
}

fn detail(entry: &HistoryEntry) -> Value {
    let record = &entry.record;
    let (kind, text) = match &record.sent.body {
        SentBody::None => ("none", None),
        SentBody::Text(text) => ("text", Some(text)),
        SentBody::Omitted(note) => ("omitted", Some(note)),
    };
    let (response, error) = match &record.outcome {
        HistoryOutcome::Response(response) => {
            let (body, body_bytes, cut_here) = match std::str::from_utf8(&response.body) {
                Ok(text) => {
                    let (text, cut_here) = cut(text.to_owned());
                    (Some(text), None, cut_here)
                }
                Err(_) => (None, Some(response.body.len()), false),
            };
            let response = json!({
                "status": response.status,
                "reason": response.reason,
                "headers": response.headers,
                "body": body,
                "body_bytes": body_bytes,
                "truncated": response.truncated || cut_here,
                "elapsed_ms": response.elapsed_ms,
                "size": response.size,
            });
            (response, None)
        }
        HistoryOutcome::Failed { error, .. } => (Value::Null, Some(error)),
    };
    json!({
        "entry_id": entry.id,
        "sent_at": record.sent_at,
        "request_id": record.request_id,
        "request_name": record.request_name,
        "request_path": record.request_path,
        "document": masked_document(&record.document),
        "request": {
            "method": record.sent.method,
            "url": record.sent.url,
            "headers": record.sent.headers,
            "body": { "kind": kind, "text": text },
        },
        "response": response,
        "error": error,
    })
}

/// The recorded document with its auth secrets masked, or `None` when it no longer parses
/// as a request (its auth could not be masked, so it is not returned).
fn masked_document(document: &str) -> Option<String> {
    let mut request: Request = toml::from_str(document).ok()?;
    request.auth = masked(&request.auth);
    toml::to_string(&request).ok()
}
