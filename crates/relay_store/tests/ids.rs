//! S8: duplicate ids get a fresh id in load order (sorted paths).

mod common;

use common::*;
use pretty_assertions::assert_eq;
use relay_core::model::Item;

fn request_ids(items: &[Item]) -> Vec<(String, relay_core::model::Id)> {
    items
        .iter()
        .filter_map(|item| match item {
            Item::Request(request) => Some((request.slug.clone(), request.request.id)),
            Item::Folder(_) => None,
        })
        .collect()
}

#[test]
fn the_later_file_in_sorted_path_order_gets_a_new_id_and_a_warning() {
    let dir = minimal_workspace();
    let root = dir.path();
    write(
        root,
        "collections/api/collection.toml",
        &collection_toml(1, "API", ""),
    );
    write(root, "collections/api/a.req.toml", &request_toml(7, "A"));
    write(root, "collections/api/b.req.toml", &request_toml(7, "B"));
    let before = read(root, "collections/api/b.req.toml");

    let loaded = load(root);

    let ids = request_ids(&loaded.workspace.collections[0].items);
    assert_eq!(ids[0], ("a".to_owned(), id(7)));
    assert_eq!(ids[1].0, "b");
    assert_ne!(ids[1].1, id(7));
    assert_eq!(
        warnings(&loaded),
        vec![format!(
            "{}: id {} is already used by {}; assigned a new id",
            path(root, "collections/api/b.req.toml").display(),
            id_text(7),
            path(root, "collections/api/a.req.toml").display()
        )]
    );
    assert_eq!(read(root, "collections/api/b.req.toml"), before);
}

#[test]
fn load_order_is_by_path_components_across_entity_kinds() {
    let dir = workspace();
    let root = dir.path();
    // Sorted: collections/api/a/... < collections/api/b.req.toml < environments/... < relay.toml
    write(
        root,
        "relay.toml",
        &format!("id = \"{}\"\nname = \"W\"\nformat = 1\n", id_text(7)),
    );
    write(
        root,
        "environments/dev.env.toml",
        &collection_toml(7, "Dev", ""),
    );
    write(
        root,
        "collections/api/collection.toml",
        &collection_toml(1, "API", ""),
    );
    write(
        root,
        "collections/api/a/folder.toml",
        &folder_toml(2, "A", ""),
    );
    write(root, "collections/api/a/x.req.toml", &request_toml(7, "X"));
    write(root, "collections/api/b.req.toml", &request_toml(2, "B"));

    let loaded = load(root);

    let first = path(root, "collections/api/a/x.req.toml");
    assert_eq!(
        warnings(&loaded),
        vec![
            format!(
                "{}: id {} is already used by {}; assigned a new id",
                path(root, "collections/api/b.req.toml").display(),
                id_text(2),
                path(root, "collections/api/a/folder.toml").display()
            ),
            format!(
                "{}: id {} is already used by {}; assigned a new id",
                path(root, "environments/dev.env.toml").display(),
                id_text(7),
                first.display()
            ),
            format!(
                "{}: id {} is already used by {}; assigned a new id",
                path(root, "relay.toml").display(),
                id_text(7),
                first.display()
            ),
        ]
    );
    assert_ne!(loaded.workspace.manifest.id, id(7));
    assert_ne!(loaded.workspace.environments[0].environment.id, id(7));
}
