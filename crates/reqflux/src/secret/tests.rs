#![allow(clippy::panic, reason = "test helpers fail the calling test")]

use std::fs;
use std::path::Path;
use std::sync::Arc;

use pretty_assertions::assert_eq;
use reqflux_engine::{Engine, MemorySecretStore};
use tempfile::TempDir;

use super::*;

const WORKSPACE: &str = "01J00000000000000000000900";
const COLLECTION: &str = "01J00000000000000000000003";
const ENVIRONMENT: &str = "01J00000000000000000000001";

fn write(root: &Path, relative: &str, content: &str) {
    let path = root.join(relative);
    if let Some(parent) = path.parent()
        && let Err(error) = fs::create_dir_all(parent)
    {
        panic!("create {}: {error}", parent.display());
    }
    if let Err(error) = fs::write(&path, content) {
        panic!("write {}: {error}", path.display());
    }
}

/// Globals `token` (secret) and `base` (plain), collection `api` with secret `key`, and
/// environments `dev` with secret `pass` and `prod` with none.
fn engine() -> (TempDir, Engine) {
    let dir = TempDir::new().unwrap_or_else(|error| panic!("temp dir: {error}"));
    let root = dir.path();
    write(
        root,
        "reqflux.toml",
        &format!(
            "id = \"{WORKSPACE}\"\nname = \"W\"\nformat = 1\n\n[[globals]]\nkey = \"token\"\nsecret = true\n\n[[globals]]\nkey = \"base\"\nvalue = \"http://x\"\n"
        ),
    );
    write(
        root,
        "collections/api/collection.toml",
        &format!(
            "id = \"{COLLECTION}\"\nname = \"Api\"\n\n[[variables]]\nkey = \"key\"\nsecret = true\n"
        ),
    );
    write(
        root,
        "environments/dev.env.toml",
        &format!(
            "id = \"{ENVIRONMENT}\"\nname = \"Dev\"\n\n[[variables]]\nkey = \"pass\"\nsecret = true\n"
        ),
    );
    write(
        root,
        "environments/prod.env.toml",
        "id = \"01J00000000000000000000002\"\nname = \"Prod\"\n",
    );
    let engine = Engine::open_with_secrets(root, Arc::new(MemorySecretStore::new()))
        .unwrap_or_else(|error| panic!("open: {error}"));
    (dir, engine)
}

fn usage(result: Result<(), Failure>) -> String {
    match result {
        Err(Failure::Usage(message)) => message,
        other => panic!("expected a usage error, got {other:?}"),
    }
}

#[test]
fn set_stores_under_the_account_of_each_scope() {
    let (_dir, engine) = engine();
    let store = MemorySecretStore::new();
    let workspace = engine.workspace();
    for (scope, key) in [("global", "token"), ("api", "key"), ("env:dev", "pass")] {
        if let Err(error) = set(workspace, &store, scope, key, "s3cret") {
            panic!("set {scope} {key}: {error:?}");
        }
    }
    assert_eq!(
        store.accounts(),
        [
            format!("{WORKSPACE}/collection/{COLLECTION}/key"),
            format!("{WORKSPACE}/environment/{ENVIRONMENT}/pass"),
            format!("{WORKSPACE}/global/token"),
        ]
    );
}

#[test]
fn set_rejects_a_key_that_is_not_a_secret_variable() {
    let (_dir, engine) = engine();
    let store = MemorySecretStore::new();
    let workspace = engine.workspace();
    assert_eq!(
        usage(set(workspace, &store, "global", "base", "v")),
        "base in global is not a secret variable"
    );
    assert_eq!(
        usage(set(workspace, &store, "env:dev", "nope", "v")),
        "env:dev has no secret variable nope"
    );
    assert_eq!(store.accounts(), Vec::<String>::new());
}

#[test]
fn set_rejects_an_unknown_scope() {
    let (_dir, engine) = engine();
    let store = MemorySecretStore::new();
    let workspace = engine.workspace();
    assert_eq!(
        usage(set(workspace, &store, "web", "key", "v")),
        "unknown scope web: expected global, a collection slug or env:<slug>"
    );
    assert_eq!(
        usage(set(workspace, &store, "env:staging", "pass", "v")),
        "unknown scope env:staging: expected global, a collection slug or env:<slug>"
    );
    assert_eq!(store.accounts(), Vec::<String>::new());
}

#[test]
fn set_rejects_an_empty_value() {
    let (_dir, engine) = engine();
    let store = MemorySecretStore::new();
    assert_eq!(
        usage(set(engine.workspace(), &store, "global", "token", "")),
        "the value for token is empty"
    );
    assert_eq!(store.accounts(), Vec::<String>::new());
}

#[test]
fn read_value_strips_one_trailing_newline() {
    for (input, expected) in [
        ("abc\n", "abc"),
        ("abc\r\n", "abc"),
        ("abc\n\n", "abc\n"),
        ("abc", "abc"),
        ("\n", ""),
    ] {
        match read_value(input.as_bytes()) {
            Ok(value) => assert_eq!(value, expected, "{input:?}"),
            Err(error) => panic!("{input:?}: {error:?}"),
        }
    }
}

#[test]
fn unset_deletes_and_is_idempotent() {
    let (_dir, engine) = engine();
    let store = MemorySecretStore::new();
    let workspace = engine.workspace();
    for _ in 0..2 {
        if let Err(error) = set(workspace, &store, "api", "key", "v") {
            panic!("set: {error:?}");
        }
        for _ in 0..2 {
            if let Err(error) = unset(workspace, &store, "api", "key") {
                panic!("unset: {error:?}");
            }
        }
        assert_eq!(store.accounts(), Vec::<String>::new());
    }
    assert_eq!(
        usage(unset(workspace, &store, "global", "base")),
        "base in global is not a secret variable"
    );
}

#[test]
fn list_shows_each_secret_set_or_not_set_and_never_a_value() {
    let (_dir, engine) = engine();
    let store = MemorySecretStore::new();
    let workspace = engine.workspace();
    if let Err(error) = set(workspace, &store, "env:dev", "pass", "hunter2") {
        panic!("set: {error:?}");
    }
    let listing = list(workspace, &store);
    assert_eq!(
        listing.lines,
        [
            "global\ttoken\tnot set",
            "api\tkey\tnot set",
            "env:dev\tpass\tset",
        ]
    );
    assert_eq!(listing.warnings, Vec::<String>::new());
    assert!(!listing.lines.concat().contains("hunter2"));
}

#[test]
fn an_unavailable_store_fails_set_and_lists_unknown_with_a_warning() {
    let (_dir, engine) = engine();
    let store = MemorySecretStore::unavailable("no Secret Service");
    let workspace = engine.workspace();
    match set(workspace, &store, "global", "token", "hunter2") {
        Err(Failure::Store(message)) => {
            assert_eq!(
                message,
                "the OS keyring is not available: no Secret Service"
            );
        }
        other => panic!("expected a store failure, got {other:?}"),
    }
    match unset(workspace, &store, "global", "token") {
        Err(Failure::Store(_)) => {}
        other => panic!("expected a store failure, got {other:?}"),
    }
    let listing = list(workspace, &store);
    assert_eq!(
        listing.lines,
        [
            "global\ttoken\tunknown",
            "api\tkey\tunknown",
            "env:dev\tpass\tunknown",
        ]
    );
    assert_eq!(
        listing.warnings,
        ["could not read the keyring: the OS keyring is not available: no Secret Service"]
    );
}
