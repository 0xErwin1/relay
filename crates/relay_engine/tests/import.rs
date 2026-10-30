//! Writing a Postman import into a workspace through the engine (S8, S9).

#![allow(
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test helpers fail the calling test"
)]

use std::fs;
use std::path::Path;

use pretty_assertions::assert_eq;
use relay_core::model::{Auth, Body, Item, RawLanguage};
use relay_engine::{Engine, ExecuteOptions, ImportError, postman};

const BASIC: &str = include_str!("../../relay_postman/tests/fixtures/collection-basic.json");
const STAGING: &str = include_str!("../../relay_postman/tests/fixtures/environment.json");

fn document(text: &str) -> serde_json::Value {
    match serde_json::from_str(text) {
        Ok(value) => value,
        Err(error) => panic!("fixture is not valid JSON: {error}"),
    }
}

/// Every file under `root` with its bytes, in path order, so a test can prove an import
/// left the workspace untouched.
fn snapshot(root: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>)>) {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) => panic!("read {}: {error}", dir.display()),
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => panic!("list {}: {error}", dir.display()),
        };
        let path = entry.path();
        if path.is_dir() {
            walk(root, &path, out);
        } else {
            let bytes = match fs::read(&path) {
                Ok(bytes) => bytes,
                Err(error) => panic!("read {}: {error}", path.display()),
            };
            out.push((
                path.strip_prefix(root).map_or_else(
                    |_| path.to_string_lossy().into_owned(),
                    |relative| relative.to_string_lossy().into_owned(),
                ),
                bytes,
            ));
        }
    }
}

#[test]
fn an_empty_directory_gets_a_whole_workspace() {
    let dir = tempfile::tempdir().expect("temp dir");
    let summary = match postman(dir.path(), &document(BASIC), &[document(STAGING)]) {
        Ok(summary) => summary,
        Err(error) => panic!("import: {error}"),
    };
    assert_eq!(summary.collection, "basic-collection");
    assert_eq!(summary.folders, 1);
    assert_eq!(summary.requests, 3);
    assert_eq!(summary.environments, 1);
    let root = dir.path();
    assert!(root.join("relay.toml").exists());
    assert!(
        root.join("collections/basic-collection/collection.toml")
            .exists()
    );
    assert!(
        root.join("collections/basic-collection/auth/folder.toml")
            .exists()
    );
    assert!(
        root.join("collections/basic-collection/auth/get-token.req.toml")
            .exists()
    );
    assert!(
        root.join("collections/basic-collection/auth/get-token.pre.js")
            .exists()
    );
    assert!(
        root.join("collections/basic-collection/auth/get-token.test.js")
            .exists()
    );
    assert!(
        root.join("collections/basic-collection/auth/get-token-2.req.toml")
            .exists()
    );
    assert!(
        root.join("collections/basic-collection/create-user.req.toml")
            .exists()
    );
    assert!(root.join("environments/staging.env.toml").exists());

    let engine = match Engine::open(root) {
        Ok(engine) => engine,
        Err(error) => panic!("open: {error}"),
    };
    let prepared = match engine.prepare("basic-collection/create-user", &ExecuteOptions::default())
    {
        Ok(prepared) => prepared,
        Err(error) => panic!("prepare: {error}"),
    };
    assert_eq!(prepared.request.method, "POST");
    assert_eq!(
        prepared.request.url,
        "api.example.com/users?sendWelcome=true"
    );
}

#[test]
fn a_raw_json_body_and_a_bearer_token_come_back_through_the_engine() {
    let dir = tempfile::tempdir().expect("temp dir");
    match postman(dir.path(), &document(BASIC), &[]) {
        Ok(_) => {}
        Err(error) => panic!("import: {error}"),
    }
    let engine = match Engine::open(dir.path()) {
        Ok(engine) => engine,
        Err(error) => panic!("open: {error}"),
    };
    let items = &engine.workspace().collections[0].items;
    let Item::Folder(auth) = &items[0] else {
        panic!("expected the Auth folder first");
    };
    let Item::Request(node) = &auth.items[0] else {
        panic!("expected the Get Token request");
    };
    assert_eq!(node.request.name, "Get Token");
    assert_eq!(
        node.request.body,
        Body::Raw {
            language: RawLanguage::Json,
            text: "{\"grant\":\"client\"}".to_owned(),
        }
    );
    assert_eq!(
        node.request.auth,
        Auth::Bearer {
            token: "secret-token".to_owned(),
        }
    );
    let prepared = match engine.prepare(
        "basic-collection/auth/get-token",
        &ExecuteOptions::default(),
    ) {
        Ok(prepared) => prepared,
        Err(error) => panic!("prepare: {error}"),
    };
    assert_eq!(prepared.request.url, "https://api.example.com/token");
    assert!(
        prepared
            .request
            .headers
            .contains(&("Authorization".to_owned(), "Bearer secret-token".to_owned()))
    );
}

#[test]
fn importing_over_an_existing_collection_fails_and_writes_nothing() {
    let dir = tempfile::tempdir().expect("temp dir");
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
    assert!(path.ends_with("collections/basic-collection/collection.toml"));
    assert_eq!(snapshot(dir.path()), before);
}

#[test]
fn an_environment_named_like_the_collection_gets_a_distinct_slug() {
    let dir = tempfile::tempdir().expect("temp dir");
    let environment = serde_json::json!({ "name": "Basic Collection", "values": [] });
    let summary = match postman(dir.path(), &document(BASIC), &[environment]) {
        Ok(summary) => summary,
        Err(error) => panic!("import: {error}"),
    };
    assert_eq!(summary.environments, 1);
    let engine = match Engine::open(dir.path()) {
        Ok(engine) => engine,
        Err(error) => panic!("open: {error}"),
    };
    assert_eq!(engine.workspace().collections.len(), 1);
    assert_eq!(engine.workspace().environments.len(), 1);
    assert_eq!(
        engine.workspace().environments[0].slug,
        "basic-collection-2"
    );
}

#[test]
fn a_document_that_does_not_map_writes_nothing() {
    let dir = tempfile::tempdir().expect("temp dir");
    let broken = serde_json::json!({ "info": { "_postman_id": "x" } });
    let error = match postman(dir.path(), &broken, &[document(STAGING)]) {
        Err(error) => error,
        Ok(_) => panic!("a collection without info.name should fail"),
    };
    let ImportError::Postman(inner) = &error else {
        panic!("expected Postman, got {error}");
    };
    assert_eq!(inner.to_string(), "collection is missing `info.name`");
    let entries = match fs::read_dir(dir.path()) {
        Ok(entries) => entries,
        Err(error) => panic!("read temp dir: {error}"),
    };
    assert_eq!(entries.count(), 0);
}
