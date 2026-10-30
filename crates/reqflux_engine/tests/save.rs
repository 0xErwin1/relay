//! Saving an edited request document back to its file through the [`Engine`].

#![allow(
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test helpers fail the calling test"
)]

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::{id, id_text, temp_dir, write};
use pretty_assertions::assert_eq;
use reqflux_core::model::{Body, Item, KeyValue, Method, RawLanguage, Request};
use reqflux_engine::{Engine, SaveError};
use reqflux_store::{ContentHash, Parent, load};

/// The file of the workspace's request `c/r`. The slug is valid, so the checked layout
/// path cannot fail.
fn request_file(dir: &Path) -> PathBuf {
    match reqflux_store::checked_request_file(dir, &Parent::collection("c"), "r") {
        Ok(file) => file,
        Err(error) => panic!("request path: {error}"),
    }
}

/// Writes a workspace whose collection `c` holds request `c/r` with the document
/// `request`, created through the store so the on-disk format is the real one.
fn workspace_with(root: &Path, request: &Request) {
    write(
        root,
        "reqflux.toml",
        &format!("id = \"{}\"\nname = \"W\"\nformat = 1\n", id_text(900)),
    );
    write(
        root,
        "collections/c/collection.toml",
        &format!("id = \"{}\"\nname = \"C\"\n", id_text(1)),
    );
    match reqflux_store::save_request(root, &Parent::collection("c"), "r", request, None) {
        Ok(_) => {}
        Err(error) => panic!("create request file: {error}"),
    }
}

fn open(root: &Path) -> Engine {
    match Engine::open(root) {
        Ok(engine) => engine,
        Err(error) => panic!("open: {error}"),
    }
}

/// The request document of `c/r` as the engine loaded it.
fn loaded_request(engine: &Engine) -> Request {
    match &engine.workspace().collections[0].items[0] {
        Item::Request(node) => node.request.clone(),
        Item::Folder(_) => panic!("expected a request"),
    }
}

fn base_of(engine: &Engine, selector: &str) -> Option<ContentHash> {
    match engine.request_base_hash(selector) {
        Ok(base) => base,
        Err(error) => panic!("base hash of {selector}: {error}"),
    }
}

fn save(
    engine: &Engine,
    selector: &str,
    request: &Request,
    expected: Option<ContentHash>,
) -> ContentHash {
    match engine.save_request(selector, request, expected) {
        Ok(hash) => hash,
        Err(error) => panic!("save {selector}: {error}"),
    }
}

/// A draft document: a new method, URL, a param, a header and a raw body.
fn draft() -> Request {
    let mut request = Request::new(id(2), "R", Method::PUT, "https://example.com/edited");
    request.params.push(KeyValue::new("page", "2"));
    request.headers.push(KeyValue::new("x-trace", "t1"));
    request.body = Body::Raw {
        language: RawLanguage::Json,
        text: r#"{"a": 1}"#.to_owned(),
    };
    request
}

#[test]
fn base_hash_matches_the_hash_the_store_reported_at_load_time() {
    let dir = temp_dir();
    workspace_with(
        dir.path(),
        &Request::new(id(2), "R", Method::GET, "https://old"),
    );
    let engine = open(dir.path());
    let file = request_file(dir.path());
    let stored = match load(dir.path()) {
        Ok(loaded) => loaded.hash(&file),
        Err(error) => panic!("load: {error}"),
    };
    assert_eq!(base_of(&engine, "c/r"), stored);
    assert!(stored.is_some());
    match engine.request_base_hash("c/missing") {
        Err(error) => assert!(error.is_usage(), "unexpected error: {error}"),
        Ok(base) => panic!("base hash of an unknown request: {base:?}"),
    }
}

#[test]
fn save_writes_the_request_file() {
    let dir = temp_dir();
    workspace_with(
        dir.path(),
        &Request::new(id(2), "R", Method::GET, "https://old"),
    );
    let engine = open(dir.path());
    let base = base_of(&engine, "c/r");
    let document = draft();
    save(&engine, "c/r", &document, base);
    drop(engine);
    let reloaded = open(dir.path());
    let request = loaded_request(&reloaded);
    assert_eq!(request.method, Method::PUT);
    assert_eq!(request.url, "https://example.com/edited");
    assert_eq!(request.params, vec![KeyValue::new("page", "2")]);
    assert_eq!(request.headers, vec![KeyValue::new("x-trace", "t1")]);
    assert_eq!(
        request.body,
        Body::Raw {
            language: RawLanguage::Json,
            text: r#"{"a": 1}"#.to_owned(),
        }
    );
}

#[test]
fn save_with_the_current_hash_returns_the_new_file_hash() {
    let dir = temp_dir();
    workspace_with(
        dir.path(),
        &Request::new(id(2), "R", Method::GET, "https://old"),
    );
    let engine = open(dir.path());
    let base = base_of(&engine, "c/r");
    let new_hash = save(&engine, "c/r", &draft(), base);
    let file = request_file(dir.path());
    let bytes = match fs::read(&file) {
        Ok(bytes) => bytes,
        Err(error) => panic!("read {}: {error}", file.display()),
    };
    assert_eq!(new_hash, ContentHash::of(&bytes));
    drop(engine);
    assert_eq!(base_of(&open(dir.path()), "c/r"), Some(new_hash));
}

#[test]
fn save_with_a_stale_hash_conflicts_and_keeps_the_other_edit() {
    let dir = temp_dir();
    workspace_with(
        dir.path(),
        &Request::new(id(2), "R", Method::GET, "https://old"),
    );
    let engine = open(dir.path());
    let base = base_of(&engine, "c/r");
    // Someone else edits the file after the workspace was loaded.
    let other = Request::new(id(2), "R", Method::POST, "https://example.com/other");
    match reqflux_store::save_request(dir.path(), &Parent::collection("c"), "r", &other, base) {
        Ok(_) => {}
        Err(error) => panic!("other edit: {error}"),
    }
    let file = request_file(dir.path());
    match engine.save_request("c/r", &draft(), base) {
        Err(SaveError::Conflict { path }) => assert_eq!(path, file),
        Err(error) => panic!("expected a conflict, got: {error}"),
        Ok(hash) => panic!("save unexpectedly succeeded: {hash}"),
    }
    drop(engine);
    // The other edit is still on disk.
    let reloaded = load(dir.path()).unwrap_or_else(|error| panic!("load: {error}"));
    let request = match &reloaded.workspace.collections[0].items[0] {
        Item::Request(node) => &node.request,
        Item::Folder(_) => panic!("expected a request"),
    };
    assert_eq!(request.url, "https://example.com/other");
}

#[test]
fn save_with_an_unknown_selector_errors_and_writes_nothing() {
    let dir = temp_dir();
    workspace_with(
        dir.path(),
        &Request::new(id(2), "R", Method::GET, "https://old"),
    );
    let engine = open(dir.path());
    let file = request_file(dir.path());
    let before = match fs::read(&file) {
        Ok(bytes) => ContentHash::of(&bytes),
        Err(error) => panic!("read {}: {error}", file.display()),
    };
    match engine.save_request("c/missing", &draft(), None) {
        Err(SaveError::UnknownRequest(_)) => {}
        Err(error) => panic!("expected an unknown request, got: {error}"),
        Ok(hash) => panic!("save unexpectedly succeeded: {hash}"),
    }
    let after = match fs::read(&file) {
        Ok(bytes) => ContentHash::of(&bytes),
        Err(error) => panic!("read {}: {error}", file.display()),
    };
    assert_eq!(before, after);
}
