//! End-to-end execution through [`Engine`] against a local mock server.

#![allow(
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test helpers fail the calling test"
)]

mod common;

use std::path::Path;

use common::{id_text, temp_dir, write};
use pretty_assertions::assert_eq;
use relay_core::model::{Body, Id, Method, RawLanguage, Request};
use relay_engine::{Engine, ExecuteOptions, Executed};
use tokio::runtime::Runtime;
use wiremock::matchers::{body_string, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

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

fn received(runtime: &Runtime, server: &MockServer) -> Vec<wiremock::Request> {
    match runtime.block_on(server.received_requests()) {
        Some(requests) => requests,
        None => panic!("request recording is disabled"),
    }
}

/// Writes a workspace whose collection `api` has `collection_extra` and a request
/// `api/r` with `request_extra`, and executes it.
fn execute(root: &Path, collection_extra: &str, request_extra: &str, url: &str) -> Executed {
    write(
        root,
        "relay.toml",
        &format!("id = \"{}\"\nname = \"W\"\nformat = 1\n", id_text(900)),
    );
    write(
        root,
        "collections/api/collection.toml",
        &format!(
            "id = \"{}\"\nname = \"Api\"\n{collection_extra}",
            id_text(1)
        ),
    );
    write(
        root,
        "collections/api/r.req.toml",
        &format!(
            "id = \"{}\"\nname = \"R\"\nmethod = \"POST\"\nurl = \"{url}\"\n{request_extra}",
            id_text(2)
        ),
    );
    let engine = match Engine::open(root) {
        Ok(engine) => engine,
        Err(error) => panic!("open: {error}"),
    };
    match engine.execute_blocking("api/r", &ExecuteOptions::default()) {
        Ok(executed) => executed,
        Err(error) => panic!("execute: {error}"),
    }
}

fn ok() -> ResponseTemplate {
    ResponseTemplate::new(201).set_body_string("done")
}

#[test]
fn raw_json_body_with_its_content_type() {
    let rt = runtime();
    let server = server(
        &rt,
        Mock::given(method("POST"))
            .and(path("/raw"))
            .and(header("content-type", "application/json"))
            .and(body_string(r#"{"a": 1}"#))
            .respond_with(ok()),
    );
    let dir = temp_dir();
    let executed = execute(
        dir.path(),
        "",
        "[body]\nmode = \"raw\"\nlanguage = \"json\"\ntext = '{\"a\": 1}'\n",
        &format!("{}/raw", server.uri()),
    );
    assert_eq!(executed.response.status, 201);
    assert_eq!(executed.response.body, b"done");
    assert_eq!(executed.method, "POST");
    assert_eq!(executed.url, format!("{}/raw", server.uri()));
}

#[test]
fn graphql_body() {
    let rt = runtime();
    let server = server(
        &rt,
        Mock::given(path("/gql"))
            .and(header("content-type", "application/json"))
            .and(body_string(r#"{"query":"{ me }","variables":null}"#))
            .respond_with(ok()),
    );
    let dir = temp_dir();
    let executed = execute(
        dir.path(),
        "",
        "[body]\nmode = \"graphql\"\nquery = \"{ me }\"\n",
        &format!("{}/gql", server.uri()),
    );
    assert_eq!(executed.response.status, 201);
}

#[test]
fn urlencoded_body() {
    let rt = runtime();
    let server = server(
        &rt,
        Mock::given(path("/form"))
            .and(header("content-type", "application/x-www-form-urlencoded"))
            .and(body_string("a=1&b=x+y"))
            .respond_with(ok()),
    );
    let dir = temp_dir();
    let executed = execute(
        dir.path(),
        "",
        "[body]\nmode = \"urlencoded\"\n\n[[body.fields]]\nkey = \"a\"\nvalue = \"1\"\n\n[[body.fields]]\nkey = \"b\"\nvalue = \"x y\"\n",
        &format!("{}/form", server.uri()),
    );
    assert_eq!(executed.response.status, 201);
}

#[test]
fn formdata_body_reads_files_relative_to_the_root() {
    let rt = runtime();
    let server = server(&rt, Mock::given(path("/multipart")).respond_with(ok()));
    let dir = temp_dir();
    write(dir.path(), "files/note.txt", "file contents");
    let executed = execute(
        dir.path(),
        "",
        "[body]\nmode = \"formdata\"\n\n[[body.parts]]\nkind = \"text\"\nkey = \"name\"\nvalue = \"ana\"\n\n[[body.parts]]\nkind = \"file\"\nkey = \"doc\"\npath = \"files/note.txt\"\n",
        &format!("{}/multipart", server.uri()),
    );
    assert_eq!(executed.response.status, 201);
    let requests = received(&rt, &server);
    let body = String::from_utf8_lossy(&requests[0].body).into_owned();
    assert!(body.contains("name=\"name\"\r\n\r\nana\r\n"), "{body}");
    assert!(body.contains("filename=\"note.txt\""), "{body}");
    assert!(body.contains("file contents"), "{body}");
}

#[test]
fn binary_body_streams_a_file_relative_to_the_root() {
    let rt = runtime();
    let server = server(
        &rt,
        Mock::given(path("/upload"))
            .and(body_string("raw bytes"))
            .respond_with(ok()),
    );
    let dir = temp_dir();
    write(dir.path(), "data.bin", "raw bytes");
    let executed = execute(
        dir.path(),
        "",
        "[body]\nmode = \"binary\"\npath = \"data.bin\"\n",
        &format!("{}/upload", server.uri()),
    );
    assert_eq!(executed.response.status, 201);
}

#[test]
fn request_inherits_the_collection_auth() {
    let rt = runtime();
    let server = server(
        &rt,
        Mock::given(path("/auth"))
            .and(header("authorization", "Bearer from-collection"))
            .respond_with(ok()),
    );
    let dir = temp_dir();
    let executed = execute(
        dir.path(),
        "\n[auth]\ntype = \"bearer\"\ntoken = \"{{token}}\"\n\n[[variables]]\nkey = \"token\"\nvalue = \"from-collection\"\n",
        "",
        &format!("{}/auth", server.uri()),
    );
    assert_eq!(executed.response.status, 201);
    assert_eq!(executed.unresolved, Vec::<String>::new());
}

#[test]
fn execute_draft_sends_the_draft_not_the_stored_request() {
    let rt = runtime();
    let server = server(
        &rt,
        Mock::given(method("POST"))
            .and(path("/draft"))
            .and(header("content-type", "application/json"))
            .and(body_string(r#"{"draft": true}"#))
            .respond_with(ok()),
    );
    let dir = temp_dir();
    write(
        dir.path(),
        "relay.toml",
        &format!("id = \"{}\"\nname = \"W\"\nformat = 1\n", id_text(900)),
    );
    write(
        dir.path(),
        "collections/api/collection.toml",
        &format!("id = \"{}\"\nname = \"Api\"\n", id_text(1)),
    );
    write(
        dir.path(),
        "collections/api/r.req.toml",
        &format!(
            "id = \"{}\"\nname = \"R\"\nmethod = \"GET\"\nurl = \"{}/stored\"\n",
            id_text(2),
            server.uri()
        ),
    );
    let engine = match Engine::open(dir.path()) {
        Ok(engine) => engine,
        Err(error) => panic!("open: {error}"),
    };
    let mut draft = Request::new(
        Id::new(),
        "R",
        Method::POST,
        format!("{}/draft", server.uri()),
    );
    draft.body = Body::Raw {
        language: RawLanguage::Json,
        text: r#"{"draft": true}"#.to_owned(),
    };
    let execution = match engine.execute_draft("api/r", &draft, &ExecuteOptions::default()) {
        Ok(execution) => execution,
        Err(error) => panic!("execute draft: {error}"),
    };
    let (method, url) = (execution.method.clone(), execution.url.clone());
    let executed = match engine.wait(execution) {
        Ok(executed) => executed,
        Err(error) => panic!("send: {error}"),
    };
    assert_eq!(executed.response.status, 201);
    assert_eq!(method, "POST");
    assert_eq!(url, format!("{}/draft", server.uri()));
}

#[test]
fn execute_returns_a_handle_any_executor_can_await() {
    let rt = runtime();
    let server = server(&rt, Mock::given(path("/async")).respond_with(ok()));
    let dir = temp_dir();
    // Writes the workspace through the blocking path first, then executes again
    // asynchronously from the test runtime.
    let first = execute(dir.path(), "", "", &format!("{}/async", server.uri()));
    assert_eq!(first.response.status, 201);
    let engine = match Engine::open(dir.path()) {
        Ok(engine) => engine,
        Err(error) => panic!("open: {error}"),
    };
    let execution = match engine.execute("api/r", &ExecuteOptions::default()) {
        Ok(execution) => execution,
        Err(error) => panic!("execute: {error}"),
    };
    let response = match rt.block_on(execution.handle) {
        Ok(Ok(response)) => response,
        Ok(Err(error)) => panic!("send: {error}"),
        Err(error) => panic!("join: {error}"),
    };
    assert_eq!(response.status, 201);
}

#[test]
fn cancelling_an_execution_ends_it_with_request_cancelled() {
    let rt = runtime();
    let server = server(
        &rt,
        Mock::given(path("/slow")).respond_with(ok().set_delay(std::time::Duration::from_secs(10))),
    );
    let dir = temp_dir();
    write(
        dir.path(),
        "relay.toml",
        &format!("id = \"{}\"\nname = \"W\"\nformat = 1\n", id_text(900)),
    );
    write(
        dir.path(),
        "collections/api/collection.toml",
        &format!("id = \"{}\"\nname = \"Api\"\n", id_text(1)),
    );
    write(
        dir.path(),
        "collections/api/r.req.toml",
        &format!(
            "id = \"{}\"\nname = \"R\"\nmethod = \"GET\"\nurl = \"{}/slow\"\n",
            id_text(2),
            server.uri()
        ),
    );
    let engine = match Engine::open(dir.path()) {
        Ok(engine) => engine,
        Err(error) => panic!("open: {error}"),
    };
    let execution = match engine.execute("api/r", &ExecuteOptions::default()) {
        Ok(execution) => execution,
        Err(error) => panic!("execute: {error}"),
    };
    execution.cancel.cancel();
    match engine.wait(execution) {
        Ok(executed) => panic!("unexpected response {}", executed.response.status),
        Err(error) => assert_eq!(error.to_string(), "request cancelled"),
    }
}
