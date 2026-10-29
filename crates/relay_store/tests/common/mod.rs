//! Helpers shared by the store integration tests.

#![allow(dead_code, unreachable_pub)]
#![allow(clippy::panic, reason = "test helpers fail the calling test")]

use std::fs;
use std::path::{Path, PathBuf};

use relay_core::model::Id;
use relay_store::{Loaded, StoreError};
use tempfile::TempDir;

/// A ULID made of a fixed prefix and `n`, so tests can name ids.
pub fn id_text(n: u32) -> String {
    format!("01J{n:023}")
}

pub fn id(n: u32) -> Id {
    match id_text(n).parse() {
        Ok(id) => id,
        Err(error) => panic!("invalid test id: {error}"),
    }
}

pub fn workspace() -> TempDir {
    match TempDir::new() {
        Ok(dir) => dir,
        Err(error) => panic!("temp dir: {error}"),
    }
}

/// A workspace root holding only a valid `relay.toml`.
pub fn minimal_workspace() -> TempDir {
    let dir = workspace();
    write(dir.path(), "relay.toml", &manifest(1));
    dir
}

pub fn manifest(format: u32) -> String {
    format!(
        "id = \"{}\"\nname = \"W\"\nformat = {format}\n",
        id_text(900)
    )
}

pub fn write(root: &Path, relative: &str, content: &str) {
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

pub fn read(root: &Path, relative: &str) -> String {
    let path = root.join(relative);
    match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => panic!("read {}: {error}", path.display()),
    }
}

pub fn path(root: &Path, relative: &str) -> PathBuf {
    root.join(relative)
}

/// Every file under `root`, relative and sorted.
pub fn files(root: &Path) -> Vec<String> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(error) => panic!("read dir {}: {error}", dir.display()),
        };
        for entry in entries {
            let path = match entry {
                Ok(entry) => entry.path(),
                Err(error) => panic!("dir entry: {error}"),
            };
            if path.is_dir() {
                walk(root, &path, out);
            } else if let Ok(relative) = path.strip_prefix(root) {
                out.push(relative.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort();
    out
}

pub fn load(root: &Path) -> Loaded {
    match relay_store::load(root) {
        Ok(loaded) => loaded,
        Err(error) => panic!("load failed: {error}"),
    }
}

pub fn load_err(root: &Path) -> StoreError {
    match relay_store::load(root) {
        Ok(_) => panic!("load unexpectedly succeeded"),
        Err(error) => error,
    }
}

pub fn warnings(loaded: &Loaded) -> Vec<String> {
    loaded.warnings.iter().map(ToString::to_string).collect()
}

pub fn collection_toml(n: u32, name: &str, extra: &str) -> String {
    format!("id = \"{}\"\nname = \"{name}\"\n{extra}", id_text(n))
}

pub fn folder_toml(n: u32, name: &str, extra: &str) -> String {
    format!("id = \"{}\"\nname = \"{name}\"\n{extra}", id_text(n))
}

pub fn request_toml(n: u32, name: &str) -> String {
    format!(
        "id = \"{}\"\nname = \"{name}\"\nmethod = \"GET\"\nurl = \"https://api.test\"\n",
        id_text(n)
    )
}
