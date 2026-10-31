//! Workspace creation (recent-workspaces S2): a missing or empty directory gets a
//! `reqflux.toml` through the store; anything else is refused and nothing is written.

#![allow(
    clippy::panic,
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test helpers fail the calling test"
)]

mod common;

use std::fs;
use std::path::Path;

use common::{snapshot, temp_dir, write};
use pretty_assertions::assert_eq;
use reqflux_engine::{CreateWorkspaceError, Engine, create_workspace};

fn assert_opens_as(dir: &Path, name: &str, id: &reqflux_core::model::Id) {
    let engine = Engine::open(dir).unwrap();
    assert!(
        engine.warnings().is_empty(),
        "warnings: {:?}",
        engine.warnings()
    );
    assert_eq!(engine.workspace().manifest.name, name);
    assert_eq!(&engine.workspace().manifest.id, id);
    assert_eq!(engine.workspace().manifest.format, 1);
}

#[test]
fn s2_a_missing_directory_is_created_with_its_parents() {
    let tmp = temp_dir();
    let dir = tmp.path().join("a/b/api");

    let id = create_workspace(&dir, "My API").unwrap();

    assert!(dir.join("reqflux.toml").is_file());
    assert_opens_as(&dir, "My API", &id);
}

#[test]
fn s2_an_empty_directory_gets_the_manifest() {
    let tmp = temp_dir();

    let id = create_workspace(tmp.path(), "Empty").unwrap();

    let names: Vec<_> = fs::read_dir(tmp.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(names, vec![std::ffi::OsString::from("reqflux.toml")]);
    assert_opens_as(tmp.path(), "Empty", &id);
}

#[test]
fn s2_two_workspaces_get_different_ids() {
    let tmp = temp_dir();
    let a = create_workspace(&tmp.path().join("a"), "A").unwrap();
    let b = create_workspace(&tmp.path().join("b"), "B").unwrap();
    assert_ne!(a, b);
}

#[test]
fn s2_a_non_empty_directory_is_refused_and_left_untouched() {
    let tmp = temp_dir();
    write(tmp.path(), "notes.txt", "hello");
    let before = snapshot(tmp.path());

    let error = create_workspace(tmp.path(), "W").unwrap_err();

    assert!(
        matches!(error, CreateWorkspaceError::NotEmpty(_)),
        "got {error:?}"
    );
    assert!(error.to_string().contains("not empty"), "{error}");
    assert_eq!(snapshot(tmp.path()), before);
}

#[test]
fn s2_an_existing_workspace_is_refused_and_left_untouched() {
    let tmp = temp_dir();
    create_workspace(tmp.path(), "First").unwrap();
    let before = snapshot(tmp.path());

    let error = create_workspace(tmp.path(), "Second").unwrap_err();

    assert!(
        matches!(error, CreateWorkspaceError::AlreadyWorkspace(_)),
        "got {error:?}"
    );
    assert!(error.to_string().contains("already a workspace"), "{error}");
    assert_eq!(snapshot(tmp.path()), before);
}

#[test]
fn s2_a_file_is_refused_and_left_untouched() {
    let tmp = temp_dir();
    write(tmp.path(), "file", "x");
    let target = tmp.path().join("file");
    let before = snapshot(tmp.path());

    let error = create_workspace(&target, "W").unwrap_err();

    assert!(
        matches!(error, CreateWorkspaceError::NotADirectory(_)),
        "got {error:?}"
    );
    assert!(error.to_string().contains("not a directory"), "{error}");
    assert_eq!(snapshot(tmp.path()), before);
}

#[test]
fn s2_an_empty_name_is_refused_and_nothing_is_written() {
    let tmp = temp_dir();
    let missing = tmp.path().join("new");

    for name in ["", "   "] {
        let error = create_workspace(&missing, name).unwrap_err();
        assert!(
            matches!(error, CreateWorkspaceError::EmptyName),
            "got {error:?}"
        );
    }
    let error = create_workspace(tmp.path(), "").unwrap_err();
    assert!(matches!(error, CreateWorkspaceError::EmptyName));

    assert!(!missing.exists());
    assert!(snapshot(tmp.path()).is_empty());
}
