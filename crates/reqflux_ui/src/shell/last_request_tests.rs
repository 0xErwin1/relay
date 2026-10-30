//! The status bar text of the last send (S10).

use std::time::{Duration, Instant};

use super::last_request::{LastKind, path_of, summary};
use super::request::{Completed, Failure, SendState, StatusClass};
use reqflux_engine::ScriptOutcome;

fn failed() -> SendState {
    SendState::Done(Completed::Failure(Failure {
        error: "request cancelled".to_owned(),
        elapsed: Duration::from_millis(3020),
        unresolved: Vec::new(),
        scripts: ScriptOutcome::default(),
    }))
}

#[test]
fn the_path_drops_the_scheme_host_and_query() {
    assert_eq!(
        path_of("https://api.test/v1/invoices?expand=lines"),
        "/v1/invoices"
    );
    assert_eq!(path_of("http://127.0.0.1:8080/ping"), "/ping");
    assert_eq!(path_of("https://api.test"), "/");
    assert_eq!(path_of("{{base_url}}/v1/invoices?a=1"), "/v1/invoices");
    assert_eq!(path_of("not a url"), "/");
}

#[test]
fn a_send_in_flight_counts_its_time() {
    let started = Instant::now();
    let state = SendState::Sending {
        started,
        previous: None,
    };
    let now = started + Duration::from_millis(1840);
    assert_eq!(
        summary("POST", "/v1/invoices", &state, now),
        Some((
            "POST /v1/invoices · sending · 1.84 s".to_owned(),
            LastKind::Sending
        ))
    );
}

#[test]
fn a_failed_send_says_failed() {
    assert_eq!(
        summary("GET", "/ping", &failed(), Instant::now()),
        Some(("GET /ping · failed · 3.02 s".to_owned(), LastKind::Failed))
    );
}

#[test]
fn an_idle_request_has_no_summary() {
    assert_eq!(
        summary("GET", "/ping", &SendState::Idle, Instant::now()),
        None
    );
}

#[test]
fn a_response_shows_its_code_and_class() {
    assert_eq!(StatusClass::of(404), StatusClass::Warning);
    let kind = LastKind::Answered(StatusClass::of(201));
    assert_eq!(kind, LastKind::Answered(StatusClass::Success));
}
