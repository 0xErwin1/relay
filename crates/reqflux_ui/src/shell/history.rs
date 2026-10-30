//! Send history in the window (history S6-S8): recording every completed or failed send in
//! the app state, the History panel rows, reopening an entry and Resend.
//!
//! Recording and reading go through `reqflux_engine::state`; the window never writes the
//! table itself. Reopening an entry never touches the request or its draft: it shows the
//! recorded response in the response pane of the request's tab, labelled
//! `From history · <time>`. An entry whose request no longer exists opens in a read-only
//! view of the document island instead (see [`Orphan`]), where Resend is disabled.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use gpui::{Context, Window};
use reqflux_core::model::{Item, Request, Workspace};
use reqflux_engine::state::{HistoryEntry, HistoryOutcome, HistoryRecord, RecordedResponse};
use reqflux_engine::{Response, ScriptOutcome, SentRequest};

use super::Shell;
use super::last_request::path_of;
use super::request::{Completed, Failure, Received};
use super::send::RequestSend;
use super::sidebar_history::{HistoryRow, Panel};

/// How many entries the History panel lists.
const PANEL_LIMIT: usize = 200;

/// What a send records besides its outcome, captured when it starts.
#[derive(Debug, Clone)]
pub(crate) struct Pending {
    pub(crate) sent_at: i64,
    pub(crate) request_id: String,
    pub(crate) request_name: String,
    pub(crate) request_path: String,
    /// The request document sent, as TOML.
    pub(crate) document: String,
}

/// A recorded response shown in the response pane, reopened from history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Reopened {
    /// When the request was sent, in milliseconds since the Unix epoch.
    pub(crate) sent_at: i64,
    /// The request document that was sent, as TOML; what Resend sends again.
    pub(crate) document: String,
    /// Whether Resend can run: the request still exists.
    pub(crate) resendable: bool,
}

impl Reopened {
    /// `From history · <time ago>`.
    pub(crate) fn label(&self) -> String {
        format!("From history · {}", time_ago(self.sent_at, now_millis()))
    }
}

/// A history entry whose request no longer exists, shown read-only in the document island
/// over every tab; opening or activating a request leaves it. Its response lives in the
/// shell's sends under the deleted request's id, so the response sub-tabs work as usual.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Orphan {
    pub(crate) request_id: String,
    pub(crate) name: String,
    pub(crate) method: String,
    pub(crate) url: String,
}

impl Orphan {
    /// `METHOD url`, as recorded.
    pub(crate) fn request_line(&self) -> String {
        format!("{} {}", self.method, self.url)
    }
}

/// Milliseconds since the Unix epoch now.
pub(crate) fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| {
            i64::try_from(since.as_millis()).unwrap_or(i64::MAX)
        })
}

/// `just now` under a minute, else `<n> min ago`, `<n> h ago` or `<n> d ago`.
pub(crate) fn time_ago(then: i64, now: i64) -> String {
    let seconds = now.saturating_sub(then).max(0) / 1000;
    match seconds {
        0..60 => "just now".to_owned(),
        60..3600 => format!("{} min ago", seconds / 60),
        3600..86_400 => format!("{} h ago", seconds / 3600),
        _ => format!("{} d ago", seconds / 86_400),
    }
}

/// `METHOD path · status · time ago`, or `failed` in place of the status.
pub(crate) fn row_text(record: &HistoryRecord, now: i64) -> String {
    let status = match &record.outcome {
        HistoryOutcome::Response(response) => response.status.to_string(),
        HistoryOutcome::Failed { .. } => "failed".to_owned(),
    };
    format!(
        "{} {} · {status} · {}",
        record.sent.method,
        path_of(&record.sent.url),
        time_ago(record.sent_at, now)
    )
}

/// The slug path (`collection/folder/request`) and the saved document of the request `id`.
pub(crate) fn find_request<'a>(
    workspace: &'a Workspace,
    id: &str,
) -> Option<(String, &'a Request)> {
    fn walk<'a>(items: &'a [Item], id: &str, path: &str) -> Option<(String, &'a Request)> {
        items.iter().find_map(|item| match item {
            Item::Request(node) => (node.request.id.to_string() == id)
                .then(|| (format!("{path}/{}", node.slug), &node.request)),
            Item::Folder(folder) => walk(&folder.items, id, &format!("{path}/{}", folder.slug)),
        })
    }
    workspace
        .collections
        .iter()
        .find_map(|collection| walk(&collection.items, id, &collection.slug))
}

/// The outcome a recorded entry shows in the response pane.
fn completed_of(entry: &HistoryEntry) -> Completed {
    match &entry.record.outcome {
        HistoryOutcome::Response(recorded) => {
            Completed::Response(received_of(recorded, &entry.record.sent))
        }
        HistoryOutcome::Failed { error, elapsed_ms } => Completed::Failure(Failure {
            error: error.clone(),
            elapsed: Duration::from_millis(*elapsed_ms),
            unresolved: Vec::new(),
            scripts: ScriptOutcome::default(),
        }),
    }
}

fn received_of(recorded: &RecordedResponse, sent: &SentRequest) -> Received {
    let response = Response {
        status: recorded.status,
        reason: recorded.reason.clone(),
        version: String::new(),
        headers: recorded.headers.clone(),
        body: recorded.body.clone(),
        final_url: sent.url.clone(),
        elapsed: Duration::from_millis(recorded.elapsed_ms),
    };
    Received::new(response, Vec::new(), ScriptOutcome::default())
}

impl Shell {
    /// What a send of the request `id` with `document` records, or `None` when it cannot
    /// be serialized; it is logged.
    pub(super) fn pending_record(&self, id: &str, document: Option<&Request>) -> Option<Pending> {
        let super::OpenedWorkspace::Loaded(engine) = &self.workspace else {
            return None;
        };
        let (path, saved) = find_request(engine.workspace(), id)?;
        let request = document.unwrap_or(saved);
        match toml::to_string(request) {
            Ok(document) => Some(Pending {
                sent_at: now_millis(),
                request_id: id.to_owned(),
                request_name: request.name.clone(),
                request_path: path,
                document,
            }),
            Err(error) => {
                log::warn!("cannot serialize request {id} for history: {error}");
                None
            }
        }
    }

    /// Records a finished send in the app state and refreshes the History panel. A failure
    /// is logged and never reaches the send; without the app state nothing is recorded.
    pub(super) fn record_send(
        &mut self,
        pending: Pending,
        sent: SentRequest,
        outcome: HistoryOutcome,
        cx: &mut Context<Self>,
    ) {
        let (Some(state), Some(workspace_id)) = (self.state.as_mut(), self.workspace_id.as_deref())
        else {
            return;
        };
        let request_id = pending.request_id.clone();
        let record = HistoryRecord {
            sent_at: pending.sent_at,
            request_id: pending.request_id,
            request_name: pending.request_name,
            request_path: pending.request_path,
            document: pending.document,
            sent,
            outcome,
        };
        if let Err(error) = state.record_history(workspace_id, record) {
            log::warn!("cannot record the send of {request_id} in history: {error}");
        }
        self.refresh_history(cx);
    }

    /// Reads the newest entries of the workspace into the History panel.
    pub(super) fn refresh_history(&mut self, cx: &mut Context<Self>) {
        let (Some(state), Some(workspace_id)) = (self.state.as_ref(), self.workspace_id.as_deref())
        else {
            return;
        };
        let entries = match state.history(workspace_id, PANEL_LIMIT) {
            Ok(entries) => entries,
            Err(error) => {
                log::warn!("cannot read the history of workspace {workspace_id}: {error}");
                return;
            }
        };
        let now = now_millis();
        let rows = entries
            .iter()
            .map(|entry| HistoryRow {
                entry: entry.id,
                text: row_text(&entry.record, now),
            })
            .collect();
        self.sidebar
            .update(cx, |sidebar, cx| sidebar.set_history(rows, cx));
    }

    /// Shows `panel` in the sidebar island; History reads the entries again so their times
    /// are current.
    pub(super) fn show_panel(&mut self, panel: Panel, cx: &mut Context<Self>) {
        if panel == Panel::History {
            self.refresh_history(cx);
        }
        self.sidebar
            .update(cx, |sidebar, cx| sidebar.set_panel(panel, cx));
        cx.notify();
    }

    /// Opens the history entry `entry`: the request's tab with the recorded response, or the
    /// read-only view when the request no longer exists. The request and its draft are
    /// untouched; a send in flight of the request is replaced.
    pub(super) fn open_history_entry(&mut self, entry: i64, cx: &mut Context<Self>) {
        let Some(state) = self.state.as_ref() else {
            return;
        };
        let entry = match state.history_entry(entry) {
            Ok(Some(entry)) => entry,
            Ok(None) => return,
            Err(error) => {
                log::warn!("cannot read history entry {entry}: {error}");
                return;
            }
        };
        let id = entry.record.request_id.clone();
        let exists = super::documents::locate(&self.roots, &id).is_some();
        self.close_orphan();
        let send = RequestSend::reopened(
            completed_of(&entry),
            Reopened {
                sent_at: entry.record.sent_at,
                document: entry.record.document.clone(),
                resendable: exists,
            },
        );
        if exists {
            self.open_document(&id, cx);
        } else {
            self.orphan = Some(Orphan {
                request_id: id.clone(),
                name: entry.record.request_name.clone(),
                method: entry.record.sent.method.clone(),
                url: entry.record.sent.url.clone(),
            });
        }
        self.sends.insert(id, send);
        cx.notify();
    }

    /// Leaves the read-only view of a deleted request's entry, dropping its response.
    pub(super) fn close_orphan(&mut self) {
        if let Some(orphan) = self.orphan.take()
            && !self.documents.open_ids().contains(&orphan.request_id)
        {
            self.sends.remove(&orphan.request_id);
        }
    }

    /// Resend: sends the recorded document of the request `id`'s reopened entry again
    /// through the engine's draft path, without touching its draft or file. Does nothing
    /// when the request no longer exists.
    pub(super) fn resend(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(reopened) = self.sends.get(id).and_then(|send| send.history.clone()) else {
            return;
        };
        if !reopened.resendable {
            return;
        }
        match toml::from_str::<Request>(&reopened.document) {
            Ok(request) => self.send_document(id, Some(request), window, cx),
            Err(error) => log::warn!("the recorded document of {id} did not parse: {error}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::time_ago;

    #[test]
    fn time_ago_rounds_down_to_the_largest_unit() {
        let minute = 60_000;
        assert_eq!(time_ago(0, 59_999), "just now");
        assert_eq!(time_ago(5, 0), "just now");
        assert_eq!(time_ago(0, minute), "1 min ago");
        assert_eq!(time_ago(0, 59 * minute), "59 min ago");
        assert_eq!(time_ago(0, 60 * minute), "1 h ago");
        assert_eq!(time_ago(0, 24 * 60 * minute), "1 d ago");
    }
}
