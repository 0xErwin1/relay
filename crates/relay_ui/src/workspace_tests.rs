//! Opening a workspace: the three states of S7.

use std::fs;
use std::path::Path;

use tempfile::TempDir;

use crate::workspace::OpenedWorkspace;

pub(crate) fn write(root: &Path, relative: &str, content: &str) {
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

/// A workspace named `Demo` with no collections.
pub(crate) fn demo_workspace() -> TempDir {
    let dir = match TempDir::new() {
        Ok(dir) => dir,
        Err(error) => panic!("temp dir: {error}"),
    };
    write(
        dir.path(),
        "relay.toml",
        "id = \"01J00000000000000000000900\"\nname = \"Demo\"\nformat = 1\n",
    );
    dir
}

#[test]
fn no_directory_is_the_empty_state() {
    let opened = OpenedWorkspace::open(None);
    assert!(matches!(opened, OpenedWorkspace::None));
    assert_eq!(opened.name(), None);
    assert_eq!(
        opened.document_message().as_deref(),
        Some("Open a workspace: relay <directory>")
    );
}

#[test]
fn a_loaded_workspace_has_its_name_and_no_message() {
    let dir = demo_workspace();
    let opened = OpenedWorkspace::open(Some(dir.path().to_path_buf()));
    assert!(matches!(opened, OpenedWorkspace::Loaded(_)), "{opened:?}");
    assert_eq!(opened.name(), Some("Demo"));
    assert_eq!(opened.document_message(), None);
}

#[test]
fn a_workspace_that_fails_to_load_reports_path_and_error() {
    let dir = match TempDir::new() {
        Ok(dir) => dir,
        Err(error) => panic!("temp dir: {error}"),
    };
    let opened = OpenedWorkspace::open(Some(dir.path().to_path_buf()));
    let OpenedWorkspace::Failed { path, error } = &opened else {
        panic!("expected a load failure, got {opened:?}");
    };
    assert_eq!(path, dir.path());
    assert!(!error.is_empty());
    assert_eq!(opened.name(), None);
    assert_eq!(
        opened.document_message(),
        Some(format!("Could not open {}: {error}", dir.path().display()))
    );
}
