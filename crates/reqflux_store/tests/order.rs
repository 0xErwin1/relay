//! S5: child order comes from the parent's `order` list.

mod common;

use common::*;
use pretty_assertions::assert_eq;
use reqflux_core::model::Item;

fn slugs(items: &[Item]) -> Vec<&str> {
    items.iter().map(Item::slug).collect()
}

#[test]
fn listed_children_follow_order_and_unlisted_append_by_name_then_slug() {
    let dir = minimal_workspace();
    let root = dir.path();
    let collection = collection_toml(1, "API", "order = [\"b\", \"missing\", \"a\"]\n");
    write(root, "collections/api/collection.toml", &collection);
    write(
        root,
        "collections/api/a.req.toml",
        &request_toml(2, "Alpha"),
    );
    write(
        root,
        "collections/api/b/folder.toml",
        &folder_toml(3, "Beta", ""),
    );
    write(
        root,
        "collections/api/c.req.toml",
        &request_toml(4, "Charlie"),
    );
    write(root, "collections/api/z.req.toml", &request_toml(5, "Able"));
    write(root, "collections/api/y.req.toml", &request_toml(6, "Able"));

    let loaded = load(root);

    assert_eq!(
        slugs(&loaded.workspace.collections[0].items),
        vec!["b", "a", "y", "z", "c"]
    );
    assert_eq!(warnings(&loaded), Vec::<String>::new());
    assert_eq!(read(root, "collections/api/collection.toml"), collection);
}

#[test]
fn folder_order_applies_to_its_own_children() {
    let dir = minimal_workspace();
    let root = dir.path();
    write(
        root,
        "collections/api/collection.toml",
        &collection_toml(1, "API", ""),
    );
    write(
        root,
        "collections/api/f/folder.toml",
        &folder_toml(2, "F", "order = [\"second\"]\n"),
    );
    write(
        root,
        "collections/api/f/first.req.toml",
        &request_toml(3, "First"),
    );
    write(
        root,
        "collections/api/f/second.req.toml",
        &request_toml(4, "Second"),
    );

    let loaded = load(root);

    let Some(Item::Folder(folder)) = loaded.workspace.collections[0].items.first() else {
        panic!("no folder")
    };
    assert_eq!(slugs(&folder.items), vec!["second", "first"]);
}

#[test]
fn collections_and_environments_sort_by_name_then_slug() {
    let dir = minimal_workspace();
    let root = dir.path();
    write(
        root,
        "collections/x/collection.toml",
        &collection_toml(1, "Beta", ""),
    );
    write(
        root,
        "collections/b/collection.toml",
        &collection_toml(2, "Alpha", ""),
    );
    write(
        root,
        "collections/a/collection.toml",
        &collection_toml(3, "Alpha", ""),
    );
    write(
        root,
        "environments/p.env.toml",
        &collection_toml(4, "Prod", ""),
    );
    write(
        root,
        "environments/d.env.toml",
        &collection_toml(5, "Dev", ""),
    );

    let loaded = load(root);

    let collections: Vec<_> = loaded
        .workspace
        .collections
        .iter()
        .map(|c| c.slug.as_str())
        .collect();
    let environments: Vec<_> = loaded
        .workspace
        .environments
        .iter()
        .map(|e| e.slug.as_str())
        .collect();
    assert_eq!(collections, vec!["a", "b", "x"]);
    assert_eq!(environments, vec!["d", "p"]);
}
