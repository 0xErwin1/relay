//! The seams where the engine's own features meet: an import opened and sent through the
//! engine, a draft against a file that moved underneath it, scripts that fetch tokens and
//! write variables against environments, files and transport failures, and an import run
//! twice. Each test pins the observable state one seam must keep.

#![allow(
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test helpers fail the calling test"
)]

mod common;

use std::fs;
use std::net::TcpListener;
use std::path::{Path, PathBuf};

use common::{id, id_text, snapshot, temp_dir, write};
use pretty_assertions::assert_eq;
use reqflux_core::model::{Body, Environment, Id, Method, RawLanguage, Request, Variable};
use reqflux_engine::{
    Engine, ExecuteOptions, Executed, Execution, ImportError, SaveError, VariableScope,
    VariableWrite, postman,
};
use reqflux_store::{ContentHash, Parent};
use tokio::runtime::Runtime;
use wiremock::matchers::{body_string, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const BASIC: &str = include_str!("../../reqflux_postman/tests/fixtures/collection-basic.json");
const STAGING: &str = include_str!("../../reqflux_postman/tests/fixtures/environment.json");

/// The mock server runs on its own thread; this runtime only starts it.
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

fn document(text: &str) -> serde_json::Value {
    match serde_json::from_str(text) {
        Ok(value) => value,
        Err(error) => panic!("fixture is not valid JSON: {error}"),
    }
}

fn manifest(root: &Path) {
    write(
        root,
        "reqflux.toml",
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

fn request(root: &Path, slug: &str, url: &str, extra: &str) {
    write(
        root,
        &format!("collections/api/{slug}.req.toml"),
        &format!(
            "id = \"{}\"\nname = \"{slug}\"\nmethod = \"GET\"\nurl = \"{url}\"\n{extra}",
            id_text(2)
        ),
    );
}

/// Writes `environments/<slug>.env.toml` with the one variable, through the store so the
/// on-disk format is the real one.
fn environment(root: &Path, slug: &str, key: &str, value: &str) {
    let document = Environment {
        id: Id::new(),
        name: slug.to_owned(),
        variables: vec![Variable::new(key, value)],
        extensions: toml::Table::new(),
    };
    match reqflux_store::save_environment(root, slug, &document, None) {
        Ok(_) => {}
        Err(error) => panic!("save environment {slug}: {error}"),
    }
}

fn engine(root: &Path) -> Engine {
    match common::open_engine(root) {
        Ok(engine) => engine,
        Err(error) => panic!("open: {error}"),
    }
}

fn execute(engine: &Engine, selector: &str, options: &ExecuteOptions) -> Executed {
    match engine.execute_blocking(selector, options) {
        Ok(executed) => executed,
        Err(error) => panic!("execute {selector}: {error}"),
    }
}

fn start(engine: &Engine, selector: &str, options: &ExecuteOptions) -> Execution {
    match engine.execute(selector, options) {
        Ok(execution) => execution,
        Err(error) => panic!("execute {selector}: {error}"),
    }
}

fn wait(engine: &Engine, execution: Execution) -> Executed {
    match engine.wait(execution) {
        Ok(executed) => executed,
        Err(error) => panic!("send: {error}"),
    }
}

/// The file of the workspace's request `c/r`. The slug is valid, so the checked layout
/// path cannot fail.
fn request_file(root: &Path) -> PathBuf {
    match reqflux_store::checked_request_file(root, &Parent::collection("c"), "r") {
        Ok(file) => file,
        Err(error) => panic!("request path: {error}"),
    }
}

fn read_bytes(path: &Path) -> Vec<u8> {
    match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) => panic!("read {}: {error}", path.display()),
    }
}

/// A port that was listening a moment ago and is closed now: connecting to it fails
/// immediately, without depending on which ports a machine keeps free.
fn closed_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap_or_else(|error| panic!("bind: {error}"));
    let port = listener
        .local_addr()
        .unwrap_or_else(|error| panic!("addr: {error}"))
        .port();
    drop(listener);
    port
}

// The whole product path in one test: a Postman collection with a pre-request script and
// a test script is imported, opened with `Engine::open`, and its request sent — the
// pre-request's write reaches the URL and the test's result comes back with the response.
#[test]
fn an_imported_collection_runs_its_scripts_end_to_end() {
    let rt = runtime();
    let server = server(
        &rt,
        Mock::given(method("GET"))
            .and(path("/imported"))
            .respond_with(ok()),
    );
    let document = serde_json::json!({
        "info": { "_postman_id": "it-1", "name": "Integration" },
        "item": [{
            "name": "Ping",
            "event": [
                {
                    "listen": "prerequest",
                    "script": { "exec": ["pm.variables.set('route', 'imported');"] },
                },
                {
                    "listen": "test",
                    "script": {
                        "exec": [
                            "pm.test('status is 201', function () { pm.expect(pm.response.code).to.eql(201); });",
                        ],
                    },
                },
            ],
            "request": { "method": "GET", "url": format!("{}/{{{{route}}}}", server.uri()) },
        }],
    });
    let dir = temp_dir();
    let summary = match postman(dir.path(), &document, &[]) {
        Ok(summary) => summary,
        Err(error) => panic!("import: {error}"),
    };
    assert_eq!(summary.collection, "integration");
    assert_eq!(summary.requests, 1);
    assert!(
        dir.path()
            .join("collections/integration/ping.pre.js")
            .exists()
    );
    assert!(
        dir.path()
            .join("collections/integration/ping.test.js")
            .exists()
    );

    let engine = engine(dir.path());
    let executed = execute(&engine, "integration/ping", &ExecuteOptions::default());
    assert_eq!(executed.response.status, 201, "{:?}", executed.scripts);
    assert!(executed.url.ends_with("/imported"), "{}", executed.url);
    assert_eq!(executed.scripts.tests.len(), 1);
    assert!(
        executed.scripts.tests[0].passed,
        "{:?}",
        executed.scripts.tests
    );
    assert_eq!(executed.scripts.tests[0].name, "status is 201");
}

// A draft whose saved file changed underneath: saving conflicts and the file keeps the
// other writer's bytes, while a send uses the draft's document.
#[test]
fn a_draft_sends_its_document_when_the_file_moved_underneath() {
    let rt = runtime();
    let server = server(
        &rt,
        Mock::given(method("POST"))
            .and(path("/draft"))
            .and(body_string(r#"{"draft": true}"#))
            .respond_with(ok()),
    );
    let dir = temp_dir();
    manifest(dir.path());
    write(
        dir.path(),
        "collections/c/collection.toml",
        &format!("id = \"{}\"\nname = \"C\"\n", id_text(1)),
    );
    let stored = Request::new(id(2), "R", Method::GET, "https://example.com/stored");
    match reqflux_store::save_request(dir.path(), &Parent::collection("c"), "r", &stored, None) {
        Ok(_) => {}
        Err(error) => panic!("create request file: {error}"),
    }
    let engine = engine(dir.path());
    let base = match engine.request_base_hash("c/r") {
        Ok(base) => base,
        Err(error) => panic!("base hash: {error}"),
    };

    // Another writer (a `git pull`, an agent) changes the file after the workspace was
    // loaded.
    let other = Request::new(id(2), "R", Method::POST, "https://example.com/other");
    let other_hash = match reqflux_store::save_request(
        dir.path(),
        &Parent::collection("c"),
        "r",
        &other,
        base,
    ) {
        Ok(hash) => hash,
        Err(error) => panic!("other edit: {error}"),
    };

    let mut draft = Request::new(id(2), "R", Method::POST, format!("{}/draft", server.uri()));
    draft.body = Body::Raw {
        language: RawLanguage::Json,
        text: r#"{"draft": true}"#.to_owned(),
    };

    // Saving the draft conflicts: the file no longer matches the base hash.
    match engine.save_request("c/r", &draft, base) {
        Err(SaveError::Conflict { path }) => assert_eq!(path, request_file(dir.path())),
        Err(error) => panic!("expected a conflict, got: {error}"),
        Ok(hash) => panic!("save unexpectedly succeeded: {hash}"),
    }
    // The send uses the draft's document, not the stored request and not the other
    // writer's edit.
    let execution = match engine.execute_draft("c/r", &draft, &ExecuteOptions::default()) {
        Ok(execution) => execution,
        Err(error) => panic!("execute draft: {error}"),
    };
    assert_eq!(execution.method, "POST");
    assert_eq!(execution.url, format!("{}/draft", server.uri()));
    let executed = wait(&engine, execution);
    assert_eq!(executed.response.status, 201, "{:?}", executed.scripts);
    // And the file on disk still holds exactly the other writer's bytes, after the
    // conflict and after the send.
    let file = request_file(dir.path());
    assert_eq!(ContentHash::of(&read_bytes(&file)), other_hash);
}

// S13 seam again, through the environment scope: a pre-request `pm.sendRequest` fetches a
// token into an environment variable and the request that follows on the wire carries it;
// the environment file itself never changes.
#[test]
fn a_pre_request_send_request_token_reaches_the_request_through_the_environment() {
    let rt = runtime();
    let token = server(
        &rt,
        Mock::given(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"token":"abc"}"#)),
    );
    let main = server(
        &rt,
        Mock::given(method("GET"))
            .and(path("/main"))
            .respond_with(ok()),
    );
    let dir = temp_dir();
    let root = dir.path();
    manifest(root);
    collection(root, "");
    request(
        root,
        "r",
        &format!("{}/main", main.uri()),
        "\n[[headers]]\nkey = \"Authorization\"\nvalue = \"Bearer {{token}}\"\n",
    );
    environment(root, "staging", "host", "staging.example.com");
    write(
        root,
        "collections/api/r.pre.js",
        &format!(
            "pm.sendRequest(\"{}/token\", function (error, response) {{\n  if (error) {{ throw error; }}\n  pm.environment.set(\"token\", response.json().token);\n}});\n",
            token.uri()
        ),
    );
    let before = snapshot(root);
    let engine = engine(root);
    let options = ExecuteOptions {
        environment: Some("staging".to_owned()),
        ..ExecuteOptions::default()
    };
    let executed = execute(&engine, "api/r", &options);
    assert_eq!(executed.response.status, 201, "{:?}", executed.scripts);
    assert!(
        executed.scripts.console.is_empty(),
        "{:?}",
        executed.scripts
    );
    // The token came out of the environment scope onto the wire.
    let received = match rt.block_on(main.received_requests()) {
        Some(requests) => requests,
        None => panic!("request recording is disabled"),
    };
    assert_eq!(received.len(), 1);
    assert_eq!(
        received[0]
            .headers
            .get("authorization")
            .and_then(|value| value.to_str().ok()),
        Some("Bearer abc")
    );
    // The script's environment write applies to this run only: the environment file keeps
    // the bytes it had.
    assert_eq!(snapshot(root), before);
}

// A send whose transport fails after the pre-request ran: the outcome the execution
// already carries keeps the console and the write, nothing reaches the file, and the next
// run rebuilds its scopes from the stored values.
#[test]
fn a_transport_failure_keeps_the_pre_request_output_and_persists_nothing() {
    let port = closed_port();
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
        &format!("http://127.0.0.1:{port}/x?who={{{{who}}}}"),
        "",
    );
    // The script only writes while the stored value is still the original one, so a
    // second execution only reports the write again when the first one was not persisted.
    write(
        root,
        "collections/api/r.pre.js",
        "console.log(\"before send\");\nif (pm.collectionVariables.get(\"who\") === \"original\") {\n  pm.collectionVariables.set(\"who\", \"scripted\");\n}\n",
    );
    let before = snapshot(root);
    let engine = engine(root);
    let execution = start(&engine, "api/r", &ExecuteOptions::default());
    // The pre-request's write resolved in this run's URL before the send failed.
    assert!(execution.url.ends_with("who=scripted"), "{}", execution.url);
    let scripts = execution.scripts.clone();
    match engine.wait(execution) {
        Err(error) => assert!(
            error.to_string().contains("could not connect"),
            "unexpected error: {error}"
        ),
        Ok(executed) => panic!("unexpected response {}", executed.response.status),
    }
    // The console and the write survived the transport failure on the execution.
    assert_eq!(scripts.console.len(), 1, "{scripts:?}");
    assert_eq!(scripts.console[0].message, "before send");
    assert_eq!(
        scripts.writes,
        vec![VariableWrite {
            scope: VariableScope::CollectionVariables,
            key: "who".to_owned(),
            value: Some("scripted".to_owned()),
        }]
    );
    // Nothing reached the workspace files.
    assert_eq!(snapshot(root), before);
    // The next run rebuilt its scopes from the stored values: the first run's write did
    // not persist into them.
    let second = start(&engine, "api/r", &ExecuteOptions::default());
    assert!(
        !second.scripts.writes.is_empty(),
        "the second run lost the stored value"
    );
    assert!(second.url.ends_with("who=scripted"), "{}", second.url);
}

// An environment changed between two runs: the second run resolves with the new values and
// the first run's script writes did not persist into the file or the reloaded workspace.
#[test]
fn an_environment_change_between_runs_resolves_with_the_new_values() {
    let rt = runtime();
    let server = server(&rt, Mock::given(method("GET")).respond_with(ok()));
    let dir = temp_dir();
    let root = dir.path();
    manifest(root);
    collection(root, "");
    request(
        root,
        "r",
        &format!("{}/run?host={{{{host}}}}", server.uri()),
        "",
    );
    write(
        root,
        "collections/api/r.pre.js",
        "pm.environment.set(\"extra\", \"from-run\");\n",
    );
    environment(root, "staging", "host", "first");
    let options = ExecuteOptions {
        environment: Some("staging".to_owned()),
        ..ExecuteOptions::default()
    };
    let untouched = snapshot(root);
    let first = engine(root);
    let executed = execute(&first, "api/r", &options);
    assert!(executed.url.ends_with("host=first"), "{}", executed.url);
    assert_eq!(
        executed.scripts.writes,
        vec![VariableWrite {
            scope: VariableScope::Environment,
            key: "extra".to_owned(),
            value: Some("from-run".to_owned()),
        }]
    );
    // The first run's write never reached the environment file.
    assert_eq!(snapshot(root), untouched);

    // The environment changes underneath (a `git pull`, another editor), like a file
    // written by hand or by a teammate.
    write(
        root,
        "environments/staging.env.toml",
        &format!(
            "id = \"{}\"\nname = \"staging\"\n\n[[variables]]\nkey = \"host\"\nvalue = \"second\"\n",
            id_text(700)
        ),
    );
    let changed = snapshot(root);
    drop(first);
    let reopened = engine(root);
    let executed = execute(&reopened, "api/r", &options);
    assert!(executed.url.ends_with("host=second"), "{}", executed.url);
    // The reloaded environment holds only the file's variables: the first run's write is
    // not in it.
    let variables = &reopened.workspace().environments[0].environment.variables;
    assert_eq!(variables.len(), 1);
    assert_eq!(variables[0].key, "host");
    assert_eq!(variables[0].value.as_deref(), Some("second"));
    // And the second run persisted nothing either.
    assert_eq!(snapshot(root), changed);
}

// The same workspace imported twice: the second import fails with `Exists` and every file
// byte is unchanged.
#[test]
fn importing_the_same_collection_twice_fails_and_changes_nothing() {
    let dir = temp_dir();
    match postman(dir.path(), &document(BASIC), &[document(STAGING)]) {
        Ok(_) => {}
        Err(error) => panic!("first import: {error}"),
    }
    let before = snapshot(dir.path());
    let error = match postman(dir.path(), &document(BASIC), &[document(STAGING)]) {
        Err(error) => error,
        Ok(_) => panic!("second import should fail"),
    };
    let ImportError::Exists { path } = &error else {
        panic!("expected Exists, got {error}");
    };
    assert!(
        path.ends_with("collections/basic-collection/collection.toml"),
        "{path:?}"
    );
    assert_eq!(snapshot(dir.path()), before);
}
