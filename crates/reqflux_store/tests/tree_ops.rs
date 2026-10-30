//! Tree operations: create, rename, duplicate, delete, move and reorder (tree-crud S1-S7).

#![allow(
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test helpers fail the calling test"
)]

mod common;

use std::collections::BTreeMap;
use std::path::Path;

use common::*;
use pretty_assertions::assert_eq;
use reqflux_core::model::{CollectionNode, FolderNode, Id, Item, RequestNode};
use reqflux_store::{Loaded, Parent, Placed, StoreError};

/// `api` (order: invoices, health) holds folder `invoices` (order: list) with request
/// `list` and its sidecars, and the unlisted nested folder `archive` with `old`; `api` also
/// holds `health`. Collection `other` holds its own `health`.
fn fixture(root: &Path) {
    write(
        root,
        "collections/api/collection.toml",
        &collection_toml(1, "API", "order = [\"invoices\", \"health\"]\n"),
    );
    write(
        root,
        "collections/api/invoices/folder.toml",
        &folder_toml(2, "Invoices", "order = [\"list\"]\n"),
    );
    write(
        root,
        "collections/api/invoices/folder.pre.js",
        "// folder\n",
    );
    write(
        root,
        "collections/api/invoices/list.req.toml",
        &request_toml(3, "List"),
    );
    write(root, "collections/api/invoices/list.pre.js", "// pre\n");
    write(root, "collections/api/invoices/list.test.js", "// test\n");
    write(
        root,
        "collections/api/invoices/list.examples/ok.toml",
        "status = 200\n",
    );
    write(
        root,
        "collections/api/invoices/archive/folder.toml",
        &folder_toml(5, "Archive", ""),
    );
    write(
        root,
        "collections/api/invoices/archive/old.req.toml",
        &request_toml(6, "Old"),
    );
    write(
        root,
        "collections/api/health.req.toml",
        &request_toml(4, "Health"),
    );
    write(
        root,
        "collections/other/collection.toml",
        &collection_toml(10, "Other", ""),
    );
    write(
        root,
        "collections/other/health.req.toml",
        &request_toml(11, "Health"),
    );
}

fn setup() -> (tempfile::TempDir, Loaded) {
    let dir = minimal_workspace();
    fixture(dir.path());
    let loaded = load(dir.path());
    (dir, loaded)
}

/// Every file under `root` with its contents.
fn snapshot(root: &Path) -> BTreeMap<String, String> {
    files(root)
        .into_iter()
        .map(|relative| {
            let text = read(root, &relative);
            (relative, text)
        })
        .collect()
}

/// Reloads and asserts the workspace loads with no warnings.
fn reload(root: &Path) -> Loaded {
    let loaded = load(root);
    assert_eq!(warnings(&loaded), Vec::<String>::new());
    loaded
}

fn ok<T>(result: Result<T, StoreError>) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("operation failed: {error}"),
    }
}

fn err<T: std::fmt::Debug>(result: Result<T, StoreError>) -> String {
    match result {
        Ok(value) => panic!("operation unexpectedly succeeded: {value:?}"),
        Err(error) => error.to_string(),
    }
}

fn collection<'a>(loaded: &'a Loaded, slug: &str) -> &'a CollectionNode {
    match loaded
        .workspace
        .collections
        .iter()
        .find(|node| node.slug == slug)
    {
        Some(node) => node,
        None => panic!("no collection {slug}"),
    }
}

fn slugs(items: &[Item]) -> Vec<&str> {
    items.iter().map(Item::slug).collect()
}

fn folder<'a>(items: &'a [Item], slug: &str) -> &'a FolderNode {
    match items.iter().find(|item| item.slug() == slug) {
        Some(Item::Folder(node)) => node,
        _ => panic!("no folder {slug}"),
    }
}

fn request<'a>(items: &'a [Item], slug: &str) -> &'a RequestNode {
    match items.iter().find(|item| item.slug() == slug) {
        Some(Item::Request(node)) => node,
        _ => panic!("no request {slug}"),
    }
}

/// Every folder and request id under `items`.
fn ids(items: &[Item]) -> Vec<Id> {
    let mut out = Vec::new();
    for item in items {
        match item {
            Item::Folder(node) => {
                out.push(node.folder.id);
                out.extend(ids(&node.items));
            }
            Item::Request(node) => out.push(node.request.id),
        }
    }
    out
}

fn placed(root: &Path, id: Id, relative: &str) -> Placed {
    Placed {
        id,
        path: path(root, relative),
    }
}

// --- S3: create -----------------------------------------------------------------

#[test]
fn a_new_request_is_named_and_numbered_and_appended_to_its_parent() {
    let (dir, loaded) = setup();
    let root = dir.path();

    let first = ok(reqflux_store::create_request(root, &loaded, id(1)));
    let loaded = reload(root);
    let second = ok(reqflux_store::create_request(root, &loaded, id(1)));

    assert_eq!(
        first.path,
        path(root, "collections/api/new-request.req.toml")
    );
    assert_eq!(
        second.path,
        path(root, "collections/api/new-request-2.req.toml")
    );
    assert_eq!(
        read(root, "collections/api/new-request-2.req.toml"),
        format!(
            "id = \"{}\"\nname = \"New request 2\"\nmethod = \"GET\"\nurl = \"\"\n",
            second.id
        )
    );
    let loaded = reload(root);
    let api = collection(&loaded, "api");
    assert_eq!(
        slugs(&api.items),
        vec!["invoices", "health", "new-request", "new-request-2"]
    );
    assert_eq!(request(&api.items, "new-request").request.id, first.id);
    assert_eq!(
        request(&api.items, "new-request").request.name,
        "New request"
    );
    assert_ne!(first.id, second.id);
}

#[test]
fn new_folders_and_collections_are_named_and_numbered() {
    let (dir, loaded) = setup();
    let root = dir.path();

    let first = ok(reqflux_store::create_folder(root, &loaded, id(2)));
    let second = ok(reqflux_store::create_folder(root, &reload(root), id(2)));
    let third = ok(reqflux_store::create_collection(root));
    let fourth = ok(reqflux_store::create_collection(root));

    assert_eq!(
        [first.path, second.path, third.path, fourth.path],
        [
            path(root, "collections/api/invoices/new-folder/folder.toml"),
            path(root, "collections/api/invoices/new-folder-2/folder.toml"),
            path(root, "collections/new-collection/collection.toml"),
            path(root, "collections/new-collection-2/collection.toml"),
        ]
    );
    let loaded = reload(root);
    let invoices = folder(&collection(&loaded, "api").items, "invoices");
    assert_eq!(
        slugs(&invoices.items),
        vec!["list", "archive", "new-folder", "new-folder-2"]
    );
    assert_eq!(
        folder(&invoices.items, "new-folder-2").folder.name,
        "New folder 2"
    );
    assert_eq!(
        collection(&loaded, "new-collection-2").collection.name,
        "New collection 2"
    );
}

#[test]
fn creating_under_an_unknown_parent_or_a_request_fails_without_writing() {
    let (dir, loaded) = setup();
    let root = dir.path();
    let before = snapshot(root);

    let unknown = err(reqflux_store::create_request(root, &loaded, id(99)));
    let request = err(reqflux_store::create_folder(root, &loaded, id(4)));

    assert_eq!(
        unknown,
        format!("no collection, folder or request has the id {}", id(99))
    );
    assert_eq!(request, format!("{} is not a collection or folder", id(4)));
    assert_eq!(snapshot(root), before);
}

// --- S4: rename -----------------------------------------------------------------

#[test]
fn renaming_a_request_moves_its_sidecars_and_keeps_its_id_and_position() {
    let (dir, loaded) = setup();
    let root = dir.path();

    let result = ok(reqflux_store::rename(root, &loaded, id(3), "All invoices"));

    assert_eq!(
        result,
        placed(
            root,
            id(3),
            "collections/api/invoices/all-invoices.req.toml"
        )
    );
    let after = snapshot(root);
    for gone in [
        "list.req.toml",
        "list.pre.js",
        "list.test.js",
        "list.examples/ok.toml",
    ] {
        assert!(!after.contains_key(&format!("collections/api/invoices/{gone}")));
    }
    assert_eq!(
        after["collections/api/invoices/all-invoices.pre.js"],
        "// pre\n"
    );
    assert_eq!(
        after["collections/api/invoices/all-invoices.test.js"],
        "// test\n"
    );
    assert_eq!(
        after["collections/api/invoices/all-invoices.examples/ok.toml"],
        "status = 200\n"
    );
    let loaded = reload(root);
    let invoices = folder(&collection(&loaded, "api").items, "invoices");
    assert_eq!(slugs(&invoices.items), vec!["all-invoices", "archive"]);
    let renamed = request(&invoices.items, "all-invoices");
    assert_eq!(renamed.request.id, id(3));
    assert_eq!(renamed.request.name, "All invoices");
    assert_eq!(renamed.scripts.pre_request.as_deref(), Some("// pre\n"));
}

#[test]
fn renaming_a_folder_moves_its_directory_and_renames_the_order_entry_in_place() {
    let (dir, loaded) = setup();
    let root = dir.path();

    let result = ok(reqflux_store::rename(root, &loaded, id(2), "Bills"));

    assert_eq!(
        result,
        placed(root, id(2), "collections/api/bills/folder.toml")
    );
    let loaded = reload(root);
    let api = collection(&loaded, "api");
    assert_eq!(slugs(&api.items), vec!["bills", "health"]);
    let bills = folder(&api.items, "bills");
    assert_eq!(
        (bills.folder.id, bills.folder.name.as_str()),
        (id(2), "Bills")
    );
    assert_eq!(ids(&bills.items), vec![id(3), id(5), id(6)]);
}

#[test]
fn renaming_a_collection_moves_its_directory() {
    let (dir, loaded) = setup();
    let root = dir.path();

    let result = ok(reqflux_store::rename(root, &loaded, id(10), "Payments"));

    assert_eq!(
        result,
        placed(root, id(10), "collections/payments/collection.toml")
    );
    let loaded = reload(root);
    let payments = collection(&loaded, "payments");
    assert_eq!(payments.collection.id, id(10));
    assert_eq!(slugs(&payments.items), vec!["health"]);
}

#[test]
fn renaming_to_the_same_slug_only_changes_the_name() {
    let (dir, loaded) = setup();
    let root = dir.path();

    ok(reqflux_store::rename(root, &loaded, id(4), "HEALTH"));

    let loaded = reload(root);
    let api = collection(&loaded, "api");
    assert_eq!(slugs(&api.items), vec!["invoices", "health"]);
    assert_eq!(request(&api.items, "health").request.name, "HEALTH");
}

#[test]
fn renaming_to_a_taken_reserved_or_empty_name_changes_nothing() {
    let (dir, loaded) = setup();
    let root = dir.path();
    let before = snapshot(root);

    let taken = err(reqflux_store::rename(root, &loaded, id(4), "Invoices"));
    let taken_collection = err(reqflux_store::rename(root, &loaded, id(10), "API"));
    let reserved = err(reqflux_store::rename(root, &loaded, id(4), "Folder"));
    let empty = err(reqflux_store::rename(root, &loaded, id(4), "  "));

    assert_eq!(
        taken,
        format!(
            "{}: the slug `invoices` is already taken in this parent",
            path(root, "collections/api/invoices").display()
        )
    );
    assert_eq!(
        taken_collection,
        format!(
            "{}: the slug `api` is already taken in this parent",
            path(root, "collections/api").display()
        )
    );
    assert_eq!(reserved, "request slug `folder` is reserved");
    assert_eq!(empty, "a name cannot be empty");
    assert_eq!(snapshot(root), before);
}

#[test]
fn renaming_a_file_changed_since_load_is_a_conflict_and_moves_nothing() {
    let (dir, loaded) = setup();
    let root = dir.path();
    write(
        root,
        "collections/api/health.req.toml",
        &request_toml(4, "Theirs"),
    );
    let before = snapshot(root);

    let result = err(reqflux_store::rename(root, &loaded, id(4), "Status"));

    assert_eq!(
        result,
        format!(
            "{} changed on disk since it was loaded",
            path(root, "collections/api/health.req.toml").display()
        )
    );
    assert_eq!(snapshot(root), before);
}

// --- S5: duplicate --------------------------------------------------------------

#[test]
fn duplicating_a_folder_copies_everything_with_new_ids_after_the_original() {
    let (dir, loaded) = setup();
    let root = dir.path();

    let first = ok(reqflux_store::duplicate(root, &loaded, id(2)));
    let second = ok(reqflux_store::duplicate(root, &reload(root), id(2)));

    assert_eq!(
        first.path,
        path(root, "collections/api/invoices-copy/folder.toml")
    );
    assert_eq!(
        second.path,
        path(root, "collections/api/invoices-copy-2/folder.toml")
    );
    let after = snapshot(root);
    for copied in [
        "folder.pre.js",
        "list.pre.js",
        "list.test.js",
        "list.examples/ok.toml",
    ] {
        assert_eq!(
            after[&format!("collections/api/invoices-copy/{copied}")],
            after[&format!("collections/api/invoices/{copied}")]
        );
    }
    let loaded = reload(root);
    let api = collection(&loaded, "api");
    assert_eq!(
        slugs(&api.items),
        vec!["invoices", "invoices-copy-2", "invoices-copy", "health"]
    );
    let original = folder(&api.items, "invoices");
    let copy = folder(&api.items, "invoices-copy");
    assert_eq!(copy.folder.id, first.id);
    assert_eq!(copy.folder.name, "Invoices copy");
    assert_eq!(
        folder(&api.items, "invoices-copy-2").folder.name,
        "Invoices copy 2"
    );
    assert_eq!(slugs(&copy.items), slugs(&original.items));
    assert_eq!(slugs(&folder(&copy.items, "archive").items), vec!["old"]);
    let mut all = vec![original.folder.id, copy.folder.id, second.id];
    all.extend(ids(&original.items));
    all.extend(ids(&copy.items));
    all.extend(ids(&folder(&api.items, "invoices-copy-2").items));
    let count = all.len();
    all.sort();
    all.dedup();
    assert_eq!(all.len(), count, "every copied entity has a new id");
}

#[test]
fn duplicating_a_request_copies_its_sidecars_next_to_it() {
    let (dir, loaded) = setup();
    let root = dir.path();

    let copy = ok(reqflux_store::duplicate(root, &loaded, id(3)));

    assert_eq!(
        copy.path,
        path(root, "collections/api/invoices/list-copy.req.toml")
    );
    assert_eq!(
        read(root, "collections/api/invoices/list-copy.examples/ok.toml"),
        "status = 200\n"
    );
    let loaded = reload(root);
    let invoices = folder(&collection(&loaded, "api").items, "invoices");
    assert_eq!(slugs(&invoices.items), vec!["list", "list-copy", "archive"]);
    let node = request(&invoices.items, "list-copy");
    assert_eq!(
        (node.request.id, node.request.name.as_str()),
        (copy.id, "List copy")
    );
    assert_eq!(node.scripts.test.as_deref(), Some("// test\n"));
}

#[test]
fn a_collection_cannot_be_duplicated_or_moved() {
    let (dir, loaded) = setup();
    let root = dir.path();
    let before = snapshot(root);

    let duplicated = err(reqflux_store::duplicate(root, &loaded, id(1)));
    let moved = err(reqflux_store::move_item(root, &loaded, id(1), id(10), 0));

    assert_eq!(duplicated, "a collection cannot be duplicated");
    assert_eq!(moved, "a collection cannot be moved");
    assert_eq!(snapshot(root), before);
}

// --- S6: delete -----------------------------------------------------------------

#[test]
fn deleting_a_folder_removes_its_children_and_its_order_entry() {
    let (dir, loaded) = setup();
    let root = dir.path();

    ok(reqflux_store::delete(root, &loaded, id(2)));

    assert!(
        files(root)
            .iter()
            .all(|file| !file.starts_with("collections/api/invoices"))
    );
    let loaded = reload(root);
    assert_eq!(slugs(&collection(&loaded, "api").items), vec!["health"]);
    assert_eq!(
        read(root, "collections/api/collection.toml"),
        collection_toml(1, "API", "order = [\"health\"]\n")
    );
}

#[test]
fn deleting_a_request_removes_its_sidecars() {
    let (dir, loaded) = setup();
    let root = dir.path();

    ok(reqflux_store::delete(root, &loaded, id(3)));

    assert_eq!(
        files(root)
            .into_iter()
            .filter(|file| file.starts_with("collections/api/invoices/"))
            .collect::<Vec<_>>(),
        vec![
            "collections/api/invoices/archive/folder.toml",
            "collections/api/invoices/archive/old.req.toml",
            "collections/api/invoices/folder.pre.js",
            "collections/api/invoices/folder.toml",
        ]
    );
    let loaded = reload(root);
    let invoices = folder(&collection(&loaded, "api").items, "invoices");
    assert_eq!(slugs(&invoices.items), vec!["archive"]);
}

#[test]
fn deleting_a_collection_removes_its_directory() {
    let (dir, loaded) = setup();
    let root = dir.path();

    ok(reqflux_store::delete(root, &loaded, id(10)));

    assert!(
        files(root)
            .iter()
            .all(|file| !file.starts_with("collections/other"))
    );
    let loaded = reload(root);
    assert_eq!(loaded.workspace.collections.len(), 1);
}

// --- S7: move -------------------------------------------------------------------

#[test]
fn moving_a_request_carries_its_sidecars_to_the_index_in_the_target() {
    let (dir, loaded) = setup();
    let root = dir.path();

    let result = ok(reqflux_store::move_item(root, &loaded, id(3), id(1), 0));

    assert_eq!(result, placed(root, id(3), "collections/api/list.req.toml"));
    let after = snapshot(root);
    assert_eq!(after["collections/api/list.pre.js"], "// pre\n");
    assert_eq!(
        after["collections/api/list.examples/ok.toml"],
        "status = 200\n"
    );
    assert!(!after.contains_key("collections/api/invoices/list.req.toml"));
    let loaded = reload(root);
    let api = collection(&loaded, "api");
    assert_eq!(slugs(&api.items), vec!["list", "invoices", "health"]);
    assert_eq!(
        slugs(&folder(&api.items, "invoices").items),
        vec!["archive"]
    );
    assert_eq!(request(&api.items, "list").request.id, id(3));
}

#[test]
fn moving_a_folder_carries_its_children_to_another_collection() {
    let (dir, loaded) = setup();
    let root = dir.path();

    let result = ok(reqflux_store::move_item(root, &loaded, id(5), id(10), 5));

    assert_eq!(
        result,
        placed(root, id(5), "collections/other/archive/folder.toml")
    );
    let loaded = reload(root);
    let other = collection(&loaded, "other");
    assert_eq!(slugs(&other.items), vec!["health", "archive"]);
    assert_eq!(ids(&folder(&other.items, "archive").items), vec![id(6)]);
}

#[test]
fn moving_onto_a_taken_slug_changes_nothing() {
    let (dir, loaded) = setup();
    let root = dir.path();
    let before = snapshot(root);

    let result = err(reqflux_store::move_item(root, &loaded, id(4), id(10), 0));

    assert_eq!(
        result,
        format!(
            "{}: the slug `health` is already taken in this parent",
            path(root, "collections/other/health.req.toml").display()
        )
    );
    assert_eq!(snapshot(root), before);
}

#[test]
fn a_folder_cannot_move_into_itself_or_its_descendants() {
    let (dir, loaded) = setup();
    let root = dir.path();
    let before = snapshot(root);

    let into_self = err(reqflux_store::move_item(root, &loaded, id(2), id(2), 0));
    let into_child = err(reqflux_store::move_item(root, &loaded, id(2), id(5), 0));
    let into_request = err(reqflux_store::move_item(root, &loaded, id(2), id(4), 0));

    let message = "a folder cannot move into itself or one of its descendants";
    assert_eq!(
        (into_self.as_str(), into_child.as_str()),
        (message, message)
    );
    assert_eq!(
        into_request,
        format!("{} is not a collection or folder", id(4))
    );
    assert_eq!(snapshot(root), before);
}

// --- S2: reorder ----------------------------------------------------------------

#[test]
fn reordering_rewrites_only_the_parent() {
    let (dir, loaded) = setup();
    let root = dir.path();
    let before = snapshot(root);

    ok(reqflux_store::reorder(root, &loaded, id(4), 0));

    let mut after = snapshot(root);
    assert_eq!(
        after.remove("collections/api/collection.toml"),
        Some(collection_toml(
            1,
            "API",
            "order = [\"health\", \"invoices\"]\n"
        ))
    );
    let mut before = before;
    before.remove("collections/api/collection.toml");
    assert_eq!(after, before);
    let loaded = reload(root);
    assert_eq!(
        slugs(&collection(&loaded, "api").items),
        vec!["health", "invoices"]
    );
}

#[test]
fn moving_within_the_same_parent_reorders() {
    let (dir, loaded) = setup();
    let root = dir.path();

    ok(reqflux_store::move_item(root, &loaded, id(2), id(1), 1));

    let loaded = reload(root);
    assert_eq!(
        slugs(&collection(&loaded, "api").items),
        vec!["health", "invoices"]
    );
}

// --- Existing behavior ----------------------------------------------------------

#[test]
fn save_request_still_round_trips_byte_for_byte() {
    let (dir, loaded) = setup();
    let root = dir.path();
    let before = snapshot(root);
    let parent = Parent::collection("api").child("invoices");
    let node = request(
        &folder(&collection(&loaded, "api").items, "invoices").items,
        "list",
    );
    let file = path(root, "collections/api/invoices/list.req.toml");

    ok(reqflux_store::save_request(
        root,
        &parent,
        "list",
        &node.request,
        loaded.hash(&file),
    ));

    assert_eq!(snapshot(root), before);
}
