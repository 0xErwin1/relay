//! Inline-script tests for the QuickJS sandbox and the Postman `pm` API (S1-S6, S8).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test helpers fail the calling test"
)]

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use relay_script::{
    ConsoleEntry, ConsoleLevel, RequestView, ResponseView, ScriptContext, ScriptError,
    ScriptErrorKind, ScriptOutcome, ScriptStage, VariableScope, VariableWrite, run,
};

fn request_view() -> RequestView {
    RequestView {
        method: "POST".into(),
        url: "https://api.example.com/users?limit=1".into(),
        headers: BTreeMap::from([
            ("content-type".into(), "application/json".into()),
            ("authorization".into(), "Bearer token".into()),
        ]),
        body: Some(r#"{"name":"ada"}"#.into()),
    }
}

fn context(stage: ScriptStage) -> ScriptContext {
    ScriptContext {
        stage,
        request: request_view(),
        variables: BTreeMap::from([
            ("token".into(), "secret".into()),
            ("count".into(), "2".into()),
        ]),
        response: None,
    }
}

fn test_context() -> ScriptContext {
    let mut context = context(ScriptStage::Test);
    context.response = Some(ResponseView {
        code: 201,
        status: "Created".into(),
        response_time_ms: 120,
        response_size: 13,
        headers: BTreeMap::from([("content-type".into(), "application/json".into())]),
        body_text: r#"{"ok":true}"#.into(),
        parsed_json: Some(serde_json::json!({ "ok": true })),
    });
    context
}

fn run_ok(script: &str, context: &ScriptContext) -> ScriptOutcome {
    run(script, context).unwrap_or_else(|error| panic!("script failed: {error}"))
}

// S2: writes are collected per scope, and an unset is a write with `None`.
#[test]
fn writes_two_scopes_and_unset() {
    let outcome = run_ok(
        r#"
        pm.environment.set("envKey", "envValue");
        pm.globals.set("globalKey", "globalValue");
        pm.environment.unset("envKey");
        "#,
        &context(ScriptStage::PreRequest),
    );
    assert_eq!(
        outcome.writes,
        vec![
            VariableWrite {
                scope: VariableScope::Environment,
                key: "envKey".into(),
                value: Some("envValue".into()),
            },
            VariableWrite {
                scope: VariableScope::Globals,
                key: "globalKey".into(),
                value: Some("globalValue".into()),
            },
            VariableWrite {
                scope: VariableScope::Environment,
                key: "envKey".into(),
                value: None,
            },
        ]
    );
    assert!(outcome.tests.is_empty());
    assert!(outcome.console.is_empty());
}

// S2: `pm.variables.get` reads the merged view, and `set` on it writes to the local scope.
#[test]
fn variables_get_reads_merged_view_and_set_writes_local_scope() {
    let outcome = run_ok(
        r#"
        if (pm.variables.get("token") !== "secret") throw new Error("merged read");
        pm.variables.set("count", "3");
        "#,
        &context(ScriptStage::Test),
    );
    assert_eq!(
        outcome.writes,
        vec![VariableWrite {
            scope: VariableScope::Variables,
            key: "count".into(),
            value: Some("3".into()),
        }]
    );
}

// S5: a passing test is recorded, and a failing one keeps its message.
#[test]
fn tests_record_pass_and_failure_with_message() {
    let outcome = run_ok(
        r#"
        pm.test("reads merged", () => {
            pm.expect(pm.variables.get("token")).to.equal("secret");
        });
        pm.test("fails with message", () => {
            pm.expect(1).to.equal(2);
        });
        "#,
        &context(ScriptStage::Test),
    );
    assert_eq!(outcome.tests.len(), 2);
    assert_eq!(outcome.tests[0].name, "reads merged");
    assert!(outcome.tests[0].passed);
    assert_eq!(outcome.tests[0].message, None);
    assert_eq!(outcome.tests[1].name, "fails with message");
    assert!(!outcome.tests[1].passed);
    let message = outcome.tests[1].message.clone().unwrap_or_default();
    assert!(message.contains("deeply equal"), "message: {message}");
}

// S5: every listed matcher passes and fails, and an unknown matcher fails loudly.
#[test]
fn every_matcher_passes_and_fails_and_unknown_matcher_fails() {
    const CASES: &[(&str, &str, &str)] = &[
        (r#""a""#, r#"to.equal("a")"#, r#""b""#),
        (r#"({a: 1})"#, r#"to.eql({a: 1})"#, r#"({a: 2})"#),
        ("1", "to.be.ok", "0"),
        (r#""x""#, r#"to.be.a("string")"#, "1"),
        ("true", "to.be.true", "false"),
        ("false", "to.be.false", "true"),
        ("null", "to.be.null", "0"),
        ("[1, 2]", "to.include(2)", "[3, 4]"),
        ("({a: 1})", r#"to.have.property("a")"#, "({})"),
        ("6", "to.be.above(5)", "4"),
        ("4", "to.be.below(5)", "6"),
        (r#""abc""#, r#"to.match("^ab")"#, r#""xyz""#),
    ];
    let passing: String = CASES
        .iter()
        .map(|(value, matcher, _)| {
            format!(r#"pm.test("pass", () => {{ pm.expect({value}).{matcher}; }});"#)
        })
        .collect::<Vec<_>>()
        .join("\n");
    let failing: String = CASES
        .iter()
        .map(|(_, matcher, other)| {
            format!(r#"pm.test("fail", () => {{ pm.expect({other}).{matcher}; }});"#)
        })
        .collect::<Vec<_>>()
        .join("\n");
    let outcome = run_ok(&passing, &context(ScriptStage::Test));
    assert_eq!(outcome.tests.len(), CASES.len());
    assert!(
        outcome.tests.iter().all(|result| result.passed),
        "all matchers should pass: {:?}",
        outcome.tests
    );

    let outcome = run_ok(&failing, &context(ScriptStage::Test));
    assert_eq!(outcome.tests.len(), CASES.len());
    assert!(
        outcome.tests.iter().all(|result| !result.passed),
        "all matchers should fail: {:?}",
        outcome.tests
    );
    assert!(
        outcome
            .tests
            .iter()
            .all(|result| result.message.as_deref().is_some_and(|m| !m.is_empty()))
    );

    let outcome = run_ok(
        r#"pm.test("unknown", () => { pm.expect(1).to.somethingElse(2); });"#,
        &context(ScriptStage::Test),
    );
    assert_eq!(outcome.tests.len(), 1);
    assert!(!outcome.tests[0].passed);
    let message = outcome.tests[0].message.clone().unwrap_or_default();
    assert!(message.contains("unknown matcher"), "message: {message}");

    let outcome = run_ok(
        r#"pm.test("custom", () => { pm.expect(1, "custom message").to.equal(2); });"#,
        &context(ScriptStage::Test),
    );
    let message = outcome.tests[0].message.clone().unwrap_or_default();
    assert!(message.starts_with("custom message:"), "message: {message}");
}

// S4: the response view works in a test stage and is absent in a pre-request stage.
#[test]
fn response_view_in_test_stage_and_absent_in_pre_request() {
    let outcome = run_ok(
        r#"
        pm.test("response view", () => {
            if (pm.response.code !== 201) throw new Error("code");
            if (pm.response.status !== "Created") throw new Error("status");
            if (pm.response.responseTime !== 120) throw new Error("responseTime");
            if (pm.response.responseSize !== 13) throw new Error("responseSize");
            if (pm.response.headers.get("Content-Type") !== "application/json") throw new Error("header");
            if (pm.response.text() !== '{"ok":true}') throw new Error("text");
            if (pm.response.json().ok !== true) throw new Error("json");
        });
        "#,
        &test_context(),
    );
    assert!(
        outcome.tests.iter().all(|result| result.passed),
        "{:?}",
        outcome.tests
    );

    let outcome = run_ok(
        r#"
        pm.test("no response", () => {
            if (typeof pm.response !== "undefined") throw new Error("pm.response exists");
        });
        "#,
        &context(ScriptStage::PreRequest),
    );
    assert!(
        outcome.tests.iter().all(|result| result.passed),
        "{:?}",
        outcome.tests
    );
}

// S3: `pm.request` is a read-only view of the request being sent.
#[test]
fn request_view_reads_the_context() {
    let outcome = run_ok(
        r#"
        pm.test("request view", () => {
            if (pm.request.method !== "POST") throw new Error("method");
            if (pm.request.url !== "https://api.example.com/users?limit=1") throw new Error("url");
            if (pm.request.url.toString() !== pm.request.url) throw new Error("url.toString");
            if (pm.request.headers.get("Authorization") !== "Bearer token") throw new Error("header");
            if (pm.request.headers.get("x-missing") !== undefined) throw new Error("missing header");
            if (pm.request.body.raw !== '{"name":"ada"}') throw new Error("body");
        });
        "#,
        &context(ScriptStage::Test),
    );
    assert!(
        outcome.tests.iter().all(|result| result.passed),
        "{:?}",
        outcome.tests
    );

    let mut no_body = context(ScriptStage::PreRequest);
    no_body.request.body = None;
    let outcome = run_ok(
        r#"
        pm.test("no body", () => {
            if (typeof pm.request.body !== "undefined") throw new Error("body exists");
        });
        "#,
        &no_body,
    );
    assert!(
        outcome.tests.iter().all(|result| result.passed),
        "{:?}",
        outcome.tests
    );
}

// S6: console entries record their level and Postman-style stringification.
#[test]
fn console_records_levels_and_stringified_values() {
    let outcome = run_ok(
        r#"
        console.log("hello");
        console.info(42);
        console.warn({a: 1});
        console.error(true);
        "#,
        &context(ScriptStage::Test),
    );
    assert_eq!(
        outcome.console,
        vec![
            ConsoleEntry {
                level: ConsoleLevel::Log,
                message: "hello".into(),
            },
            ConsoleEntry {
                level: ConsoleLevel::Info,
                message: "42".into(),
            },
            ConsoleEntry {
                level: ConsoleLevel::Warn,
                message: r#"{"a":1}"#.into(),
            },
            ConsoleEntry {
                level: ConsoleLevel::Error,
                message: "true".into(),
            },
        ]
    );
}

// S8: a syntax error names the stage and never panics.
#[test]
fn syntax_error_names_the_stage() {
    let error: ScriptError = run("if ( {", &context(ScriptStage::PreRequest)).unwrap_err();
    assert_eq!(error.kind, ScriptErrorKind::Syntax);
    assert_eq!(error.stage, ScriptStage::PreRequest);
    assert!(error.to_string().contains("pre-request"));
}

// S8: a thrown error names the stage and never panics.
#[test]
fn thrown_error_names_the_stage() {
    let error: ScriptError =
        run("throw new Error('boom');", &context(ScriptStage::Test)).unwrap_err();
    assert_eq!(error.kind, ScriptErrorKind::Thrown);
    assert_eq!(error.stage, ScriptStage::Test);
    assert!(error.message.contains("boom"), "message: {}", error.message);
    assert!(error.to_string().contains("test"));
}

// S8: an endless loop stops at the time limit, keeps the outcome so far, and stays bounded.
#[test]
fn endless_loop_stops_at_the_time_limit() {
    let started = Instant::now();
    let error: ScriptError = run(
        r#"console.log("before"); while (true) {}"#,
        &context(ScriptStage::Test),
    )
    .unwrap_err();
    let elapsed = started.elapsed();
    assert_eq!(error.kind, ScriptErrorKind::Timeout);
    assert_eq!(error.stage, ScriptStage::Test);
    assert!(elapsed < Duration::from_secs(10), "elapsed: {elapsed:?}");
    assert_eq!(error.outcome.console.len(), 1);
    assert_eq!(error.outcome.console[0].message, "before");
}

// S8: allocating far beyond the memory limit fails with the memory error, not a crash. The
// string is repeated in one eager allocation because QuickJS rope strings defer copying.
#[test]
fn memory_limit_stops_big_allocations() {
    let error: ScriptError = run(
        r#"let s = "x".repeat(134217728);"#,
        &context(ScriptStage::Test),
    )
    .unwrap_err();
    assert_eq!(error.kind, ScriptErrorKind::MemoryLimit);
    assert_eq!(error.stage, ScriptStage::Test);
}

// S2: a get after a set in the same run sees the written value.
#[test]
fn get_after_set_sees_the_written_value() {
    let outcome = run_ok(
        r#"
        pm.environment.set("page", "2");
        pm.test("sees write", () => {
            pm.expect(pm.environment.get("page")).to.equal("2");
        });
        pm.environment.unset("page");
        pm.test("unset hides", () => {
            pm.expect(pm.environment.get("page")).to.equal(undefined);
        });
        "#,
        &context(ScriptStage::Test),
    );
    assert!(
        outcome.tests.iter().all(|result| result.passed),
        "{:?}",
        outcome.tests
    );
    assert_eq!(outcome.writes.len(), 2);
}
