//! Script execution through [`Engine`] against a local mock server (S7): the effective
//! scripts, their applied variable writes and the results beside the response.

#![allow(
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test helpers fail the calling test"
)]

mod common;

use std::path::Path;
use std::time::Duration;

use common::{id_text, temp_dir, write};
use pretty_assertions::assert_eq;
use relay_engine::{ExecuteOptions, VariableScope, VariableWrite};
use tokio::runtime::Runtime;
use wiremock::Mock;
use wiremock::matchers::{method, path};
use wiremock::{MockServer, ResponseTemplate};

/// The mock server runs on its own thread; this runtime only starts and inspects it, and
/// stays outside the engine's runtime so `execute_blocking` is not called from async code.
fn runtime() -> Runtime {
    match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => panic!("runtime: {error}"),
    }
}

fn server(runtime: &Runtime, mock: Mock) -> MockServer {
    runtime.block_on(async {
        let server = MockServer::start().await;
        mock.mount(&server).await;
        server
    })
}

fn ok() -> ResponseTemplate {
    ResponseTemplate::new(201).set_body_string(r#"{"ok":true}"#)
}

fn manifest(root: &Path) {
    write(
        root,
        "relay.toml",
        &format!("id = \"{}\"\nname = \"W\"\nformat = 1\n", id_text(900)),
    );
}

fn collection(root: &Path, extra: &str) {
    write(
        root,
        "collections/api/collection.toml",
        &format!("id = \"{}\"\nname = \"Api\"\n{extra}", id_text(1)),
    );
}

fn folder(root: &Path, slug: &str) {
    write(
        root,
        &format!("collections/api/{slug}/folder.toml"),
        &format!("id = \"{}\"\nname = \"{slug}\"\n", id_text(4)),
    );
}

fn request(root: &Path, slug: &str, in_folder: bool, url: &str) {
    let directory = if in_folder {
        "collections/api/f/".to_owned()
    } else {
        "collections/api/".to_owned()
    };
    write(
        root,
        &format!("{directory}{slug}.req.toml"),
        &format!(
            "id = \"{}\"\nname = \"{slug}\"\nmethod = \"GET\"\nurl = \"{url}\"\n",
            id_text(2)
        ),
    );
}

fn engine(root: &Path) -> relay_engine::Engine {
    match relay_engine::Engine::open(root) {
        Ok(engine) => engine,
        Err(error) => panic!("open: {error}"),
    }
}

fn execute(engine: &relay_engine::Engine, selector: &str) -> relay_engine::Executed {
    match engine.execute_blocking(selector, &ExecuteOptions::default()) {
        Ok(executed) => executed,
        Err(error) => panic!("execute: {error}"),
    }
}

/// Executes `selector` and finishes the execution with [`Execution::finish`], as an async
/// front end does.
fn finish(rt: &Runtime, engine: &relay_engine::Engine, selector: &str) -> relay_engine::Executed {
    let execution = match engine.execute(selector, &ExecuteOptions::default()) {
        Ok(execution) => execution,
        Err(error) => panic!("execute: {error}"),
    };
    match rt.block_on(execution.finish()) {
        Ok(executed) => executed,
        Err(error) => panic!("finish: {error}"),
    }
}

/// Blanks the wall-clock time of a response, which two sends never share.
fn without_elapsed(executed: &mut relay_engine::Executed) {
    executed.response.elapsed = Duration::ZERO;
}

#[test]
fn finish_returns_the_same_executed_as_execute_blocking() {
    let rt = runtime();
    let server = server(&rt, Mock::given(path("/finished")).respond_with(ok()));
    let dir = temp_dir();
    let root = dir.path();
    manifest(root);
    collection(root, "");
    request(root, "r", false, &format!("{}/finished", server.uri()));
    write(
        root,
        "collections/api/r.test.js",
        "pm.test(\"status\", function () {\n  pm.expect(pm.response.code).to.eql(201);\n});\nconsole.log(\"from the test\");\n",
    );
    let engine = engine(root);
    let mut expected = execute(&engine, "api/r");
    let mut executed = finish(&rt, &engine, "api/r");
    without_elapsed(&mut expected);
    without_elapsed(&mut executed);
    assert_eq!(executed, expected);
    assert!(executed.scripts.tests.iter().all(|test| test.passed));
}

#[test]
fn finish_keeps_a_failing_test_beside_a_successful_response() {
    let rt = runtime();
    let server = server(&rt, Mock::given(path("/failing")).respond_with(ok()));
    let dir = temp_dir();
    let root = dir.path();
    manifest(root);
    collection(root, "");
    request(root, "r", false, &format!("{}/failing", server.uri()));
    write(
        root,
        "collections/api/r.test.js",
        "pm.test(\"wrong code\", function () {\n  pm.expect(201, \"bad code\").to.eql(404);\n});\n",
    );
    let engine = engine(root);
    let mut expected = execute(&engine, "api/r");
    let mut executed = finish(&rt, &engine, "api/r");
    without_elapsed(&mut expected);
    without_elapsed(&mut executed);
    assert_eq!(executed, expected);
    assert_eq!(executed.response.status, 201);
    let test = &executed.scripts.tests[0];
    assert!(!test.passed);
    assert_eq!(
        test.message.as_deref(),
        Some("bad code: expected 201 to deeply equal 404")
    );
}

#[test]
fn a_pre_request_script_write_resolves_in_the_url() {
    let rt = runtime();
    let server = server(
        &rt,
        Mock::given(method("GET"))
            .and(path("/pre/from-script"))
            .respond_with(ok()),
    );
    let dir = temp_dir();
    let root = dir.path();
    manifest(root);
    collection(root, "");
    request(
        root,
        "r",
        false,
        &format!("{}/pre/{{{{token}}}}", server.uri()),
    );
    write(
        root,
        "collections/api/r.pre.js",
        "pm.variables.set(\"token\", \"from-script\");\n",
    );
    let engine = engine(root);
    let executed = execute(&engine, "api/r");
    assert_eq!(executed.url, format!("{}/pre/from-script", server.uri()));
    assert_eq!(executed.response.status, 201);
}

#[test]
fn the_most_specific_pre_request_script_wins() {
    let rt = runtime();
    let server = server(&rt, Mock::given(method("GET")).respond_with(ok()));
    let dir = temp_dir();
    let root = dir.path();
    let base = server.uri();
    manifest(root);
    collection(root, "");
    write(
        root,
        "collections/api/collection.pre.js",
        "pm.collectionVariables.set(\"who\", \"collection\");\n",
    );
    folder(root, "f");
    write(
        root,
        "collections/api/f/folder.pre.js",
        "pm.collectionVariables.set(\"who\", \"folder\");\n",
    );
    // The collection-only request, the folder one and the request-script one share `who`.
    request(
        root,
        "only_collection",
        false,
        &format!("{base}/c?who={{{{who}}}}"),
    );
    request(
        root,
        "only_folder",
        true,
        &format!("{base}/f?who={{{{who}}}}"),
    );
    request(root, "with_own", true, &format!("{base}/q?who={{{{who}}}}"));
    write(
        root,
        "collections/api/f/with_own.pre.js",
        "pm.collectionVariables.set(\"who\", \"request\");\n",
    );
    let engine = engine(root);
    let executed = execute(&engine, "api/only_collection");
    assert!(
        executed.url.ends_with("?who=collection"),
        "{}",
        executed.url
    );
    let executed = execute(&engine, "api/f/only_folder");
    assert!(executed.url.ends_with("?who=folder"), "{}", executed.url);
    let executed = execute(&engine, "api/f/with_own");
    assert!(executed.url.ends_with("?who=request"), "{}", executed.url);
}

#[test]
fn a_test_script_sees_the_response() {
    let rt = runtime();
    let server = server(&rt, Mock::given(path("/tested")).respond_with(ok()));
    let dir = temp_dir();
    let root = dir.path();
    manifest(root);
    collection(root, "");
    request(root, "r", false, &format!("{}/tested", server.uri()));
    write(
        root,
        "collections/api/r.test.js",
        "pm.test(\"status is 201\", function () {\n  pm.expect(pm.response.code).to.eql(201);\n});\npm.test(\"body ok\", function () {\n  pm.expect(pm.response.json().ok).to.eql(true);\n});\n",
    );
    let engine = engine(root);
    let executed = execute(&engine, "api/r");
    let names: Vec<&str> = executed
        .scripts
        .tests
        .iter()
        .map(|test| test.name.as_str())
        .collect();
    assert_eq!(names, ["status is 201", "body ok"]);
    assert!(
        executed.scripts.tests.iter().all(|test| test.passed),
        "{:?}",
        executed.scripts.tests
    );
}

#[test]
fn a_failing_test_does_not_fail_the_request() {
    let rt = runtime();
    let server = server(&rt, Mock::given(path("/failing")).respond_with(ok()));
    let dir = temp_dir();
    let root = dir.path();
    manifest(root);
    collection(root, "");
    request(root, "r", false, &format!("{}/failing", server.uri()));
    write(
        root,
        "collections/api/r.test.js",
        "pm.test(\"wrong code\", function () {\n  pm.expect(201, \"bad code\").to.eql(404);\n});\n",
    );
    let engine = engine(root);
    let executed = execute(&engine, "api/r");
    assert_eq!(executed.response.status, 201);
    let test = &executed.scripts.tests[0];
    assert!(!test.passed);
    assert_eq!(
        test.message.as_deref(),
        Some("bad code: expected 201 to deeply equal 404")
    );
}

#[test]
fn a_script_error_is_reported_in_the_outcome() {
    let rt = runtime();
    let server = server(&rt, Mock::given(path("/broken")).respond_with(ok()));
    let dir = temp_dir();
    let root = dir.path();
    manifest(root);
    collection(root, "");
    request(root, "r", false, &format!("{}/broken", server.uri()));
    write(
        root,
        "collections/api/r.pre.js",
        "this is not ((javascript\n",
    );
    let engine = engine(root);
    let executed = execute(&engine, "api/r");
    // The request still went out and came back.
    assert_eq!(executed.response.status, 201);
    assert_eq!(executed.scripts.tests, Vec::new());
    let errors: Vec<&str> = executed
        .scripts
        .console
        .iter()
        .map(|entry| entry.message.as_str())
        .collect();
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(errors[0].starts_with("pre-request script"), "{errors:?}");
}

#[test]
fn pre_request_writes_do_not_persist() {
    let rt = runtime();
    let server = server(&rt, Mock::given(method("GET")).respond_with(ok()));
    let dir = temp_dir();
    let root = dir.path();
    manifest(root);
    collection(
        root,
        "\n[[variables]]\nkey = \"who\"\nvalue = \"original\"\n",
    );
    request(
        root,
        "r",
        false,
        &format!("{}/p?who={{{{who}}}}", server.uri()),
    );
    // The script only writes while the stored value is still the original one, so a second
    // execution only reports the write again when the first one was not persisted.
    write(
        root,
        "collections/api/r.pre.js",
        "if (pm.collectionVariables.get(\"who\") === \"original\") {\n  pm.collectionVariables.set(\"who\", \"scripted\");\n}\n",
    );
    let engine = engine(root);
    let first = execute(&engine, "api/r");
    assert_eq!(
        first.scripts.writes,
        vec![VariableWrite {
            scope: VariableScope::CollectionVariables,
            key: "who".to_owned(),
            value: Some("scripted".to_owned()),
        }]
    );
    assert!(first.url.ends_with("?who=scripted"), "{}", first.url);
    let second = execute(&engine, "api/r");
    assert!(
        !second.scripts.writes.is_empty(),
        "the second execution rebuilt the scopes from the stored values"
    );
    let stored = &engine.workspace().collections[0].collection.variables;
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].key, "who");
    assert_eq!(stored[0].value.as_deref(), Some("original"));
}
