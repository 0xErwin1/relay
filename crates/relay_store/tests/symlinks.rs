//! A symlinked directory under `collections/` is not followed: a hand-edited workspace
//! could point it outside the root. Symlinked files keep being read like regular files.

#![cfg(unix)]

mod common;

use std::os::unix::fs::symlink;

use common::*;
use pretty_assertions::assert_eq;

#[test]
fn a_symlinked_collection_directory_is_skipped_with_a_warning() {
    let dir = minimal_workspace();
    let root = dir.path();
    let outside = workspace();
    write(
        outside.path(),
        "collection.toml",
        &collection_toml(1, "Outside", ""),
    );
    write(outside.path(), "r.req.toml", &request_toml(2, "R"));
    write(
        root,
        "collections/api/collection.toml",
        &collection_toml(3, "API", ""),
    );
    symlink(outside.path(), path(root, "collections/linked")).expect("symlink");

    let loaded = load(root);

    assert_eq!(
        warnings(&loaded),
        vec![format!(
            "{}: directory is a symlink; it was ignored",
            path(root, "collections/linked").display()
        )]
    );
    let slugs: Vec<_> = loaded
        .workspace
        .collections
        .iter()
        .map(|collection| collection.slug.clone())
        .collect();
    assert_eq!(slugs, vec!["api"]);
    assert!(
        !loaded
            .hashes
            .keys()
            .any(|file| file.starts_with(outside.path())),
        "no file outside the root was read"
    );
}

#[test]
fn a_symlinked_folder_directory_is_skipped_with_a_warning() {
    let dir = minimal_workspace();
    let root = dir.path();
    let outside = workspace();
    write(
        outside.path(),
        "folder.toml",
        &folder_toml(1, "Outside", ""),
    );
    write(
        root,
        "collections/api/collection.toml",
        &collection_toml(2, "API", ""),
    );
    symlink(outside.path(), path(root, "collections/api/linked")).expect("symlink");

    let loaded = load(root);

    assert_eq!(
        warnings(&loaded),
        vec![format!(
            "{}: directory is a symlink; it was ignored",
            path(root, "collections/api/linked").display()
        )]
    );
    assert_eq!(loaded.workspace.collections[0].items, Vec::new());
}

#[test]
fn a_symlinked_file_is_still_read_like_a_regular_file() {
    let dir = minimal_workspace();
    let root = dir.path();
    let outside = workspace();
    write(
        outside.path(),
        "linked.req.toml",
        &request_toml(1, "Linked"),
    );
    write(
        root,
        "collections/api/collection.toml",
        &collection_toml(2, "API", ""),
    );
    symlink(
        outside.path().join("linked.req.toml"),
        path(root, "collections/api/linked.req.toml"),
    )
    .expect("symlink");

    let loaded = load(root);

    assert_eq!(warnings(&loaded), Vec::<String>::new());
    let collection = &loaded.workspace.collections[0];
    assert_eq!(collection.items.len(), 1);
    assert_eq!(collection.items[0].slug(), "linked");
    assert_eq!(
        loaded.hash(&path(root, "collections/api/linked.req.toml")),
        Some(relay_store::ContentHash::of(
            read(root, "collections/api/linked.req.toml").as_bytes()
        ))
    );
}
