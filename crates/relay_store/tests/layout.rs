//! S4: the on-disk layout the loader reads.

mod common;

use common::*;
use pretty_assertions::assert_eq;
use relay_core::model::{Item, Scripts};

#[test]
fn loads_every_entity_kind_and_sidecar_script_from_the_layout() {
    let dir = minimal_workspace();
    let root = dir.path();
    write(
        root,
        "environments/dev.env.toml",
        &collection_toml(1, "Dev", ""),
    );
    write(
        root,
        "collections/api/collection.toml",
        &collection_toml(2, "API", ""),
    );
    write(root, "collections/api/collection.pre.js", "pre();\n");
    write(root, "collections/api/collection.test.js", "test();\n");
    write(
        root,
        "collections/api/inv/folder.toml",
        &folder_toml(3, "Invoices", ""),
    );
    write(root, "collections/api/inv/folder.pre.js", "fpre();\n");
    write(root, "collections/api/inv/folder.test.js", "ftest();\n");
    write(
        root,
        "collections/api/inv/deep/folder.toml",
        &folder_toml(4, "Deep", ""),
    );
    write(
        root,
        "collections/api/inv/deep/list.req.toml",
        &request_toml(5, "List"),
    );
    write(root, "collections/api/inv/deep/list.pre.js", "rpre();\n");
    write(root, "collections/api/inv/deep/list.test.js", "rtest();\n");

    let loaded = load(root);
    let workspace = &loaded.workspace;

    assert_eq!(warnings(&loaded), Vec::<String>::new());
    assert_eq!(workspace.manifest.id, id(900));
    assert_eq!(workspace.environments.len(), 1);
    assert_eq!(workspace.environments[0].slug, "dev");
    assert_eq!(workspace.environments[0].environment.id, id(1));

    let collection = &workspace.collections[0];
    assert_eq!(collection.slug, "api");
    assert_eq!(collection.collection.id, id(2));
    assert_eq!(
        collection.scripts,
        Scripts {
            pre_request: Some("pre();\n".into()),
            test: Some("test();\n".into())
        }
    );
    let Some(Item::Folder(inv)) = collection.items.first() else {
        panic!("no folder")
    };
    assert_eq!(inv.slug, "inv");
    assert_eq!(inv.folder.id, id(3));
    assert_eq!(
        inv.scripts,
        Scripts {
            pre_request: Some("fpre();\n".into()),
            test: Some("ftest();\n".into())
        }
    );
    let Some(Item::Folder(deep)) = inv.items.first() else {
        panic!("no nested folder")
    };
    assert_eq!(deep.folder.id, id(4));
    let Some(Item::Request(list)) = deep.items.first() else {
        panic!("no request")
    };
    assert_eq!(list.slug, "list");
    assert_eq!(list.request.id, id(5));
    assert_eq!(
        list.scripts,
        Scripts {
            pre_request: Some("rpre();\n".into()),
            test: Some("rtest();\n".into())
        }
    );
}

#[test]
fn records_the_hash_of_every_file_read() {
    let dir = minimal_workspace();
    let root = dir.path();
    write(
        root,
        "collections/api/collection.toml",
        &collection_toml(2, "API", ""),
    );
    write(root, "collections/api/r.req.toml", &request_toml(5, "R"));
    write(root, "collections/api/r.test.js", "t();\n");

    let loaded = load(root);

    let mut paths: Vec<_> = loaded.hashes.keys().cloned().collect();
    paths.sort();
    assert_eq!(
        paths,
        vec![
            path(root, "collections/api/collection.toml"),
            path(root, "collections/api/r.req.toml"),
            path(root, "collections/api/r.test.js"),
            path(root, "relay.toml"),
        ]
    );
    assert_eq!(
        loaded.hash(&path(root, "collections/api/r.test.js")),
        Some(relay_store::ContentHash::of(b"t();\n"))
    );
}

#[test]
fn ignores_relay_dir_hidden_entries_unknown_files_and_examples_silently() {
    let dir = minimal_workspace();
    let root = dir.path();
    write(root, ".relay/state.toml", "garbage = [");
    write(root, ".git/config", "x");
    write(root, "README.md", "# notes");
    write(root, "environments/notes.txt", "x");
    write(
        root,
        "collections/api/collection.toml",
        &collection_toml(2, "API", ""),
    );
    write(root, "collections/api/notes.md", "x");
    write(root, "collections/api/orphan.pre.js", "x");
    write(root, "collections/api/list.examples/ok.toml", "x = 1");
    write(root, "collections/api/.hidden/folder.toml", "broken = [");

    let loaded = load(root);

    assert_eq!(warnings(&loaded), Vec::<String>::new());
    assert_eq!(loaded.workspace.collections[0].items, Vec::new());
}

#[test]
fn a_request_with_a_reserved_slug_is_skipped_with_a_warning() {
    let dir = minimal_workspace();
    let root = dir.path();
    write(
        root,
        "collections/api/collection.toml",
        &collection_toml(2, "API", ""),
    );
    write(
        root,
        "collections/api/collection.pre.js",
        "collection script",
    );
    write(
        root,
        "collections/api/collection.req.toml",
        &request_toml(3, "Shadow"),
    );
    write(
        root,
        "collections/api/invoices/folder.toml",
        &folder_toml(4, "Invoices", ""),
    );
    write(
        root,
        "collections/api/invoices/folder.pre.js",
        "folder script",
    );
    write(
        root,
        "collections/api/invoices/folder.req.toml",
        &request_toml(5, "Shadow"),
    );

    let loaded = load(root);

    assert_eq!(
        warnings(&loaded),
        vec![
            format!(
                "{}: request slug `collection` is reserved; rename the file to load it",
                path(root, "collections/api/collection.req.toml").display()
            ),
            format!(
                "{}: request slug `folder` is reserved; rename the file to load it",
                path(root, "collections/api/invoices/folder.req.toml").display()
            ),
        ]
    );
    let collection = &loaded.workspace.collections[0];
    assert_eq!(
        collection.scripts.pre_request.as_deref(),
        Some("collection script")
    );
    let [Item::Folder(folder)] = collection.items.as_slice() else {
        panic!(
            "expected only the invoices folder, got {:?}",
            collection.items
        );
    };
    assert_eq!(folder.scripts.pre_request.as_deref(), Some("folder script"));
    assert_eq!(folder.items, Vec::new());
}

#[test]
fn warns_about_directories_without_their_entity_file() {
    let dir = minimal_workspace();
    let root = dir.path();
    write(root, "collections/loose/readme.md", "x");
    write(
        root,
        "collections/api/collection.toml",
        &collection_toml(2, "API", ""),
    );
    write(
        root,
        "collections/api/assets/logo.req.toml",
        &request_toml(5, "Logo"),
    );

    let loaded = load(root);

    assert_eq!(
        warnings(&loaded),
        vec![
            format!(
                "{}: directory has no folder.toml; it was ignored",
                path(root, "collections/api/assets").display()
            ),
            format!(
                "{}: directory has no collection.toml; it was ignored",
                path(root, "collections/loose").display()
            ),
        ]
    );
    assert_eq!(loaded.workspace.collections.len(), 1);
    assert_eq!(loaded.workspace.collections[0].items, Vec::new());
}

#[test]
fn an_empty_script_file_loads_as_no_script() {
    let dir = minimal_workspace();
    let root = dir.path();
    write(
        root,
        "collections/api/collection.toml",
        &collection_toml(2, "API", ""),
    );
    write(root, "collections/api/collection.pre.js", "");

    let loaded = load(root);

    assert_eq!(loaded.workspace.collections[0].scripts, Scripts::default());
}

#[test]
fn a_toml_parse_error_names_the_file() {
    let dir = minimal_workspace();
    let root = dir.path();
    write(
        root,
        "collections/api/collection.toml",
        &collection_toml(2, "API", ""),
    );
    write(root, "collections/api/bad.req.toml", "id = [");

    let message = load_err(root).to_string();

    let prefix = format!("{}: ", path(root, "collections/api/bad.req.toml").display());
    assert!(message.starts_with(&prefix), "{message}");
}
