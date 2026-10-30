//! `pm.sendRequest` against fake host handlers — no HTTP (S13), and the concurrency
//! property of a script blocked in a host call (S14's relay_script half).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test helpers fail the calling test"
)]

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use relay_script::{
    RequestView, ScriptContext, ScriptErrorKind, ScriptRequest, ScriptResponse, ScriptStage,
    SendRequest, run,
};

fn context(stage: ScriptStage, send: Option<SendRequest>) -> ScriptContext {
    ScriptContext {
        stage,
        request: RequestView {
            method: "GET".into(),
            url: "https://api.example.com/main".into(),
            headers: BTreeMap::new(),
            body: None,
        },
        variables: BTreeMap::new(),
        response: None,
        send_request: send,
    }
}

fn token_response() -> Result<ScriptResponse, String> {
    Ok(ScriptResponse {
        code: 200,
        status: "OK".into(),
        headers: BTreeMap::from([("content-type".into(), "application/json".into())]),
        body: r#"{"token":"abc"}"#.into(),
    })
}

fn run_ok(script: &str, context: &ScriptContext) -> relay_script::ScriptOutcome {
    run(script, context).unwrap_or_else(|error| panic!("script failed: {error}"))
}

// S13: the callback receives the response and reads code, status, text() and headers.get.
#[test]
fn the_callback_receives_the_response() {
    let outcome = run_ok(
        r#"
        pm.sendRequest("https://api.example.com/token", function (error, response) {
          if (error) throw new Error("unexpected error: " + error);
          console.log(response.code + " " + response.status + " " + response.text());
          console.log("type=" + response.headers.get("content-type"));
        });
        "#,
        &context(
            ScriptStage::PreRequest,
            Some(SendRequest::new(|_request| token_response())),
        ),
    );
    let messages: Vec<&str> = outcome
        .console
        .iter()
        .map(|entry| entry.message.as_str())
        .collect();
    assert_eq!(
        messages,
        ["200 OK {\"token\":\"abc\"}", "type=application/json",]
    );
}

// S13: a host failure reaches the callback as `error`, never as a thrown script error.
#[test]
fn a_host_failure_arrives_as_error_not_a_throw() {
    let outcome = run_ok(
        r#"
        pm.sendRequest("https://api.example.com/token", function (error, response) {
          if (response !== undefined) throw new Error("no response expected");
          console.log("error was: " + error);
        });
        "#,
        &context(
            ScriptStage::PreRequest,
            Some(SendRequest::new(
                |_request| Err("connection refused".into()),
            )),
        ),
    );
    let messages: Vec<&str> = outcome
        .console
        .iter()
        .map(|entry| entry.message.as_str())
        .collect();
    assert_eq!(messages, ["error was: connection refused"]);
}

// S13: a script sends, then uses the response in `pm.test`.
#[test]
fn a_script_uses_the_response_in_pm_test() {
    let outcome = run_ok(
        r#"
        pm.sendRequest("https://api.example.com/token", function (error, response) {
          pm.test("token fetched", function () {
            if (error) throw error;
            pm.expect(response.json().token).to.eql("abc");
          });
        });
        "#,
        &context(
            ScriptStage::Test,
            Some(SendRequest::new(|_request| token_response())),
        ),
    );
    assert_eq!(outcome.tests.len(), 1);
    assert!(outcome.tests[0].passed, "{:?}", outcome.tests[0]);
    assert_eq!(outcome.tests[0].name, "token fetched");
}

// S13: an object argument with method, header (object and array forms) and body — plain
// string and `{ mode: "raw", raw }` — reaches the handler with those values; a string
// argument is a GET with no headers and no body.
#[test]
fn the_request_argument_reaches_the_handler() {
    let seen: Arc<Mutex<Vec<ScriptRequest>>> = Arc::new(Mutex::new(Vec::new()));
    let recorder = Arc::clone(&seen);
    let send = SendRequest::new(move |request: ScriptRequest| {
        recorder
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .push(request);
        token_response()
    });
    run_ok(
        r#"
        pm.sendRequest({
          url: "https://api.example.com/one",
          method: "POST",
          header: { "X-A": "1" },
          body: "raw text",
        }, function () {});
        pm.sendRequest({
          url: "https://api.example.com/two",
          method: "PUT",
          header: [{ key: "X-B", value: "2" }],
          body: { mode: "raw", raw: "second" },
        }, function () {});
        pm.sendRequest("https://api.example.com/three", function () {});
        "#,
        &context(ScriptStage::PreRequest, Some(send)),
    );
    let seen = seen.lock().unwrap_or_else(|error| error.into_inner());
    assert_eq!(
        *seen,
        vec![
            ScriptRequest {
                method: "POST".into(),
                url: "https://api.example.com/one".into(),
                headers: vec![("X-A".into(), "1".into())],
                body: Some("raw text".into()),
            },
            ScriptRequest {
                method: "PUT".into(),
                url: "https://api.example.com/two".into(),
                headers: vec![("X-B".into(), "2".into())],
                body: Some("second".into()),
            },
            ScriptRequest {
                method: "GET".into(),
                url: "https://api.example.com/three".into(),
                headers: Vec::new(),
                body: None,
            },
        ]
    );
}

// S13: the host's own timeout bounds a send, and the script's 5 s deadline stops a slower
// host as a Timeout, not a hang: the run fails with the kind, not an error from the callback.
#[test]
fn a_host_slower_than_the_deadline_ends_as_a_timeout() {
    let error = run(
        r#"
        pm.sendRequest("https://api.example.com/token", function () {});
        "#,
        &context(
            ScriptStage::PreRequest,
            Some(SendRequest::new(|_request| {
                thread::sleep(Duration::from_millis(5500));
                token_response()
            })),
        ),
    )
    .unwrap_err();
    assert_eq!(error.kind, ScriptErrorKind::Timeout);
}

// S14: a script blocked in a sleeping host call stops nothing else — other threads run
// their work to completion while the callback is still waiting.
#[test]
fn a_sleeping_callback_does_not_block_concurrent_work() {
    let in_flight = Arc::new(AtomicBool::new(false));
    let finished = Arc::new(AtomicBool::new(false));
    let callback_in_flight = Arc::clone(&in_flight);
    let callback_finished = Arc::clone(&finished);
    let send = SendRequest::new(move |_request| {
        callback_in_flight.store(true, Ordering::SeqCst);
        thread::sleep(Duration::from_millis(400));
        callback_finished.store(true, Ordering::SeqCst);
        token_response()
    });
    let script = thread::spawn(move || {
        run(
            "pm.sendRequest('https://api.example.com/token', function () {});",
            &context(ScriptStage::PreRequest, Some(send)),
        )
    });
    thread::sleep(Duration::from_millis(100));
    assert!(
        in_flight.load(Ordering::SeqCst),
        "the callback never started"
    );
    thread::sleep(Duration::from_millis(50));
    assert!(
        !finished.load(Ordering::SeqCst),
        "the concurrent work waited for the script"
    );
    let outcome = script.join().unwrap_or_else(|error| panic!("{error:?}"));
    assert!(
        outcome.is_ok(),
        "the script finished once the host answered"
    );
}
