//! The send history in the app state: schema migration, recording, listing and limits.

#![allow(
    clippy::panic,
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test helpers fail the calling test"
)]

use std::path::{Path, PathBuf};

use pretty_assertions::assert_eq;
use reqflux_engine::state::{
    AppState, Draft, HISTORY_MAX_BODY_BYTES, HISTORY_MAX_ENTRIES, HistoryLimits, HistoryOutcome,
    HistoryRecord, RecordedResponse, StateError,
};
use reqflux_engine::{SentBody, SentRequest};

fn state_path(dir: &tempfile::TempDir) -> PathBuf {
    dir.path().join("state.sqlite3")
}

fn sent(url: &str) -> SentRequest {
    SentRequest {
        method: "POST".to_owned(),
        url: url.to_owned(),
        headers: vec![("Accept".to_owned(), "*/*".to_owned())],
        body: SentBody::Text("{\"a\":1}".to_owned()),
    }
}

fn record(n: u32) -> HistoryRecord {
    HistoryRecord {
        sent_at: 1_000 + i64::from(n),
        request_id: format!("req-{n}"),
        request_name: format!("Request {n}"),
        request_path: format!("api/r{n}"),
        document: format!("name = \"Request {n}\"\n"),
        sent: sent(&format!("https://example.test/{n}?q=1")),
        outcome: HistoryOutcome::Response(RecordedResponse {
            status: 200,
            reason: Some("OK".to_owned()),
            headers: vec![("content-type".to_owned(), "text/plain".to_owned())],
            body: format!("body {n}").into_bytes(),
            truncated: false,
            elapsed_ms: 12,
            size: 6,
        }),
    }
}

fn open(path: &Path) -> AppState {
    match AppState::open(path) {
        Ok(state) => state,
        Err(error) => panic!("open: {error}"),
    }
}

/// Writes a schema v1 file the way a previous build did, with one draft in it.
fn write_v1(path: &Path) {
    let conn = rusqlite::Connection::open(path).unwrap();
    conn.execute_batch(
        "CREATE TABLE drafts (
             workspace_id TEXT NOT NULL,
             entity_id TEXT NOT NULL,
             payload TEXT NOT NULL,
             base_hash TEXT,
             updated_at INTEGER NOT NULL,
             PRIMARY KEY (workspace_id, entity_id)
         );
         INSERT INTO drafts VALUES ('ws', 'req', 'a = 1', 'hash-1', 7);
         PRAGMA user_version = 1;",
    )
    .unwrap();
}

fn user_version(path: &Path) -> i64 {
    let conn = rusqlite::Connection::open(path).unwrap();
    conn.query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap()
}

#[test]
fn a_v1_file_upgrades_to_v2_and_keeps_its_drafts() {
    let dir = tempfile::tempdir().unwrap();
    let path = state_path(&dir);
    write_v1(&path);

    let mut state = open(&path);

    assert_eq!(user_version(&path), 2);
    let draft = state.draft("ws", "req").unwrap().unwrap();
    assert_eq!(draft.payload, "a = 1");
    assert_eq!(draft.base_hash.as_deref(), Some("hash-1"));
    assert_eq!(draft.updated_at, 7);
    // The history table exists after the upgrade.
    state.record_history("ws", record(1)).unwrap();
    assert_eq!(state.history("ws", 10).unwrap().len(), 1);
}

#[test]
fn a_fresh_file_is_created_at_v2() {
    let dir = tempfile::tempdir().unwrap();
    let path = state_path(&dir);
    open(&path);
    assert_eq!(user_version(&path), 2);
}

#[test]
fn a_file_newer_than_the_build_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = state_path(&dir);
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch("PRAGMA user_version = 3;").unwrap();
    }
    match AppState::open(&path) {
        Err(StateError::UnsupportedVersion { version, .. }) => assert_eq!(version, 3),
        other => panic!("expected UnsupportedVersion, got {other:?}"),
    }
}

#[test]
fn drafts_still_work_after_the_migration() {
    let dir = tempfile::tempdir().unwrap();
    let path = state_path(&dir);
    write_v1(&path);
    let mut state = open(&path);

    state
        .put_draft("ws", "other", Draft::new("b = 2", None))
        .unwrap();
    assert_eq!(
        state.draft("ws", "other").unwrap().unwrap().payload,
        "b = 2"
    );
    assert_eq!(state.drafts("ws").unwrap().len(), 2);
    assert!(state.delete_draft("ws", "req").unwrap());
    assert_eq!(state.draft("ws", "req").unwrap(), None);
}

#[test]
fn record_get_and_round_trip_every_field() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = open(&state_path(&dir));

    let id = state.record_history("ws", record(1)).unwrap();
    let entry = state.history_entry(id).unwrap().unwrap();

    assert_eq!(entry.id, id);
    assert_eq!(entry.workspace_id, "ws");
    assert_eq!(entry.record, record(1));
    assert_eq!(state.history_entry(id + 100).unwrap(), None);
}

#[test]
fn a_failed_send_and_the_body_markers_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = open(&state_path(&dir));
    let mut failed = record(1);
    failed.sent.body = SentBody::Omitted("multipart form data".to_owned());
    failed.outcome = HistoryOutcome::Failed {
        error: "connection refused".to_owned(),
        elapsed_ms: 3,
    };
    let mut empty = record(2);
    empty.sent.body = SentBody::None;

    let failed_id = state.record_history("ws", failed.clone()).unwrap();
    let empty_id = state.record_history("ws", empty.clone()).unwrap();

    assert_eq!(
        state.history_entry(failed_id).unwrap().unwrap().record,
        failed
    );
    assert_eq!(
        state.history_entry(empty_id).unwrap().unwrap().record,
        empty
    );
}

#[test]
fn list_is_newest_first_limited_and_per_workspace() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = open(&state_path(&dir));
    for n in 1..=4 {
        state.record_history("ws", record(n)).unwrap();
    }
    state.record_history("other", record(9)).unwrap();

    let listed: Vec<String> = state
        .history("ws", 3)
        .unwrap()
        .into_iter()
        .map(|entry| entry.record.request_id)
        .collect();

    assert_eq!(listed, ["req-4", "req-3", "req-2"]);
    assert_eq!(state.history("other", 10).unwrap().len(), 1);
}

#[test]
fn clear_removes_only_that_workspace() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = open(&state_path(&dir));
    state.record_history("ws", record(1)).unwrap();
    state.record_history("ws", record(2)).unwrap();
    state.record_history("other", record(3)).unwrap();
    state.put_draft("ws", "req", Draft::new("a", None)).unwrap();

    assert_eq!(state.clear_history("ws").unwrap(), 2);

    assert!(state.history("ws", 10).unwrap().is_empty());
    assert_eq!(state.history("other", 10).unwrap().len(), 1);
    assert!(state.draft("ws", "req").unwrap().is_some());
}

#[test]
fn the_default_limits_are_500_entries_and_5_mb() {
    assert_eq!(HISTORY_MAX_ENTRIES, 500);
    assert_eq!(HISTORY_MAX_BODY_BYTES, 5 * 1024 * 1024);
    assert_eq!(
        HistoryLimits::default(),
        HistoryLimits {
            max_entries: 500,
            max_body_bytes: 5 * 1024 * 1024,
        }
    );
}

#[test]
fn recording_prunes_a_workspace_to_its_newest_entries() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = open(&state_path(&dir));
    let limits = HistoryLimits {
        max_entries: 3,
        ..HistoryLimits::default()
    };
    state.record_history("other", record(99)).unwrap();
    for n in 1..=5 {
        state
            .record_history_with_limits("ws", record(n), limits)
            .unwrap();
    }

    let listed: Vec<String> = state
        .history("ws", 10)
        .unwrap()
        .into_iter()
        .map(|entry| entry.record.request_id)
        .collect();

    assert_eq!(listed, ["req-5", "req-4", "req-3"]);
    assert_eq!(state.history("other", 10).unwrap().len(), 1);
}

#[test]
fn a_body_over_the_cap_is_stored_cut_with_the_flag() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = open(&state_path(&dir));
    let mut big = record(1);
    let body = vec![b'x'; HISTORY_MAX_BODY_BYTES + 10];
    if let HistoryOutcome::Response(response) = &mut big.outcome {
        response.body = body.clone();
        response.size = body.len() as u64;
    }
    let mut exact = record(2);
    if let HistoryOutcome::Response(response) = &mut exact.outcome {
        response.body = vec![b'y'; HISTORY_MAX_BODY_BYTES];
    }

    let big_id = state.record_history("ws", big).unwrap();
    let exact_id = state.record_history("ws", exact).unwrap();

    let HistoryOutcome::Response(stored) =
        state.history_entry(big_id).unwrap().unwrap().record.outcome
    else {
        panic!("expected a response");
    };
    assert_eq!(stored.body.len(), HISTORY_MAX_BODY_BYTES);
    assert!(stored.truncated);
    assert_eq!(stored.size, (HISTORY_MAX_BODY_BYTES + 10) as u64);
    let HistoryOutcome::Response(stored) = state
        .history_entry(exact_id)
        .unwrap()
        .unwrap()
        .record
        .outcome
    else {
        panic!("expected a response");
    };
    assert_eq!(stored.body.len(), HISTORY_MAX_BODY_BYTES);
    assert!(!stored.truncated);
}
