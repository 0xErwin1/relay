//! The last send of the window, as the status bar shows it: `<METHOD> <path> · <status> ·
//! <time>`.

use std::time::Instant;

use super::request::{Completed, SendState, StatusClass, elapsed_text};

/// The request the window sent last, with the method and URL it was sent with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LastSend {
    /// The request id, whose send state holds the outcome.
    pub(crate) id: String,
    pub(crate) method: String,
    /// The path of the URL, without scheme, host or query.
    pub(crate) path: String,
}

/// What the status bar icon shows for the last send.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LastKind {
    Sending,
    /// The server answered with a status of this class.
    Answered(StatusClass),
    Failed,
}

/// The path of `url`: what follows the host (or a leading `{{variable}}`) up to the query,
/// or `/` when there is none.
pub(crate) fn path_of(url: &str) -> String {
    let after_scheme = url.split_once("://").map_or(url, |(_, rest)| rest);
    let without_query = after_scheme
        .split_once(['?', '#'])
        .map_or(after_scheme, |(path, _)| path);
    match without_query.find('/') {
        Some(start) => without_query.get(start..).unwrap_or("/").to_owned(),
        None => "/".to_owned(),
    }
}

/// The status bar text and icon of a send of `method` to `path` in `state`, measuring a send
/// in flight up to `now`; `None` before the first send.
pub(crate) fn summary(
    method: &str,
    path: &str,
    state: &SendState,
    now: Instant,
) -> Option<(String, LastKind)> {
    let (status, elapsed, kind) = match state {
        SendState::Idle => return None,
        SendState::Sending { started, .. } => (
            "sending".to_owned(),
            now.saturating_duration_since(*started),
            LastKind::Sending,
        ),
        SendState::Done(Completed::Response(received)) => (
            received.status.to_string(),
            received.elapsed,
            LastKind::Answered(StatusClass::of(received.status)),
        ),
        SendState::Done(Completed::Failure(failure)) => {
            ("failed".to_owned(), failure.elapsed, LastKind::Failed)
        }
    };
    Some((
        format!("{method} {path} · {status} · {}", elapsed_text(elapsed)),
        kind,
    ))
}
