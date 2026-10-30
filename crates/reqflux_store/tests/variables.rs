//! Scope variable edits and environment create, rename and delete (variables-editor S1, S3).

#![allow(
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test helpers fail the calling test"
)]

mod common;

use std::path::Path;

use common::{files, id, id_text, minimal_workspace, read, write};
use pretty_assertions::assert_eq;
use reqflux_core::model::Variable;
use reqflux_store::{Loaded, ScopeFile, StoreError, VariableChange};

fn load(root: &Path) -> Loaded {
    match reqflux_store::load(root) {
        Ok(loaded) => loaded,
        Err(error) => panic!("load: {error}"),
    }
}

/// A workspace with one global, collection `api` (id 1) and environment `dev` (id 3).
fn scoped() -> tempfile::TempDir {
    let dir = minimal_workspace();
    let root = dir.path();
    write(
        root,
        "reqflux.toml",
        &format!(
            "id = \"{}\"\nname = \"W\"\nformat = 1\n\n[[globals]]\nkey = \"g\"\nvalue = \"1\"\n",
            id_text(900)
        ),
    );
    write(
        root,
        "collections/api/collection.toml",
        &format!("id = \"{}\"\nname = \"Api\"\n", id_text(1)),
    );
    write(
        root,
        "environments/dev.env.toml",
        &format!("id = \"{}\"\nname = \"dev\"\n", id_text(3)),
    );
    dir
}

fn apply(root: &Path, scope: ScopeFile, change: VariableChange) -> Result<(), StoreError> {
    let loaded = load(root);
    reqflux_store::plan_variables(root, &loaded, scope, change)?
        .write()
        .map(drop)
}

fn ok(result: Result<(), StoreError>) {
    if let Err(error) = result {
        panic!("variable edit: {error}");
    }
}

#[test]
fn add_replace_and_remove_in_every_scope_kind() {
    let dir = scoped();
    let root = dir.path();
    let scopes = [
        (ScopeFile::Globals, "reqflux.toml", 1),
        (
            ScopeFile::Collection(id(1)),
            "collections/api/collection.toml",
            0,
        ),
        (
            ScopeFile::Environment(id(3)),
            "environments/dev.env.toml",
            0,
        ),
    ];
    for (scope, file, index) in scopes {
        let before = read(root, file);
        ok(apply(
            root,
            scope,
            VariableChange::Add(Variable::new("host", "a.test")),
        ));
        assert_eq!(
            read(root, file),
            format!(
                "{before}\n[[{}]]\nkey = \"host\"\nvalue = \"a.test\"\n",
                table(scope)
            )
        );
        let mut changed = Variable::new("host", "b.test");
        changed.description = Some("Host".to_owned());
        changed.enabled = false;
        ok(apply(root, scope, VariableChange::Replace(index, changed)));
        assert_eq!(
            read(root, file),
            format!(
                "{before}\n[[{}]]\nkey = \"host\"\nvalue = \"b.test\"\ndescription = \"Host\"\nenabled = false\n",
                table(scope)
            )
        );
        ok(apply(root, scope, VariableChange::Remove(index)));
        assert_eq!(read(root, file), before);
    }
}

fn table(scope: ScopeFile) -> &'static str {
    match scope {
        ScopeFile::Globals => "globals",
        ScopeFile::Collection(_) | ScopeFile::Environment(_) => "variables",
    }
}

#[test]
fn an_empty_or_duplicate_key_is_refused_and_changes_nothing() {
    let dir = scoped();
    let root = dir.path();
    let before = read(root, "reqflux.toml");
    let empty = apply(
        root,
        ScopeFile::Globals,
        VariableChange::Add(Variable::new("  ", "x")),
    );
    assert!(
        matches!(empty, Err(StoreError::EmptyVariableKey)),
        "{empty:?}"
    );
    let duplicate = apply(
        root,
        ScopeFile::Globals,
        VariableChange::Add(Variable::new("g", "x")),
    );
    match duplicate {
        Err(error @ StoreError::DuplicateVariableKey { .. }) => {
            assert_eq!(
                error.to_string(),
                "the variable `g` already exists in this scope"
            );
        }
        other => panic!("expected a duplicate key, got {other:?}"),
    }
    ok(apply(
        root,
        ScopeFile::Globals,
        VariableChange::Add(Variable::new("h", "2")),
    ));
    let after_add = read(root, "reqflux.toml");
    let renamed = apply(
        root,
        ScopeFile::Globals,
        VariableChange::Replace(1, Variable::new("g", "2")),
    );
    assert!(
        matches!(renamed, Err(StoreError::DuplicateVariableKey { .. })),
        "{renamed:?}"
    );
    let missing = apply(root, ScopeFile::Globals, VariableChange::Remove(5));
    assert!(
        matches!(missing, Err(StoreError::NoVariable { index: 5 })),
        "{missing:?}"
    );
    assert_eq!(read(root, "reqflux.toml"), after_add);
    assert!(after_add.starts_with(&before));
    let unknown = apply(
        root,
        ScopeFile::Environment(id(77)),
        VariableChange::Remove(0),
    );
    assert!(
        matches!(unknown, Err(StoreError::UnknownEnvironment { .. })),
        "{unknown:?}"
    );
}

#[test]
fn keeping_the_same_key_at_its_own_index_is_allowed() {
    let dir = scoped();
    let root = dir.path();
    ok(apply(
        root,
        ScopeFile::Globals,
        VariableChange::Replace(0, Variable::new("g", "2")),
    ));
    assert!(read(root, "reqflux.toml").ends_with("key = \"g\"\nvalue = \"2\"\n"));
}

#[test]
fn a_secret_variable_is_written_without_its_value() {
    let dir = scoped();
    let root = dir.path();
    let mut secret = Variable::new("token", "s3cr3t");
    secret.secret = true;
    ok(apply(
        root,
        ScopeFile::Environment(id(3)),
        VariableChange::Add(secret),
    ));
    let text = read(root, "environments/dev.env.toml");
    assert!(!text.contains("s3cr3t"), "{text}");
    assert!(
        text.ends_with("[[variables]]\nkey = \"token\"\nsecret = true\n"),
        "{text}"
    );
}

#[test]
fn a_file_changed_on_disk_makes_the_edit_a_conflict() {
    let dir = scoped();
    let root = dir.path();
    let loaded = load(root);
    let changed = format!("id = \"{}\"\nname = \"Changed\"\n", id_text(3));
    write(root, "environments/dev.env.toml", &changed);
    let planned = reqflux_store::plan_variables(
        root,
        &loaded,
        ScopeFile::Environment(id(3)),
        VariableChange::Add(Variable::new("a", "b")),
    );
    assert!(
        matches!(planned, Err(StoreError::Conflict { .. })),
        "{planned:?}"
    );
    assert_eq!(read(root, "environments/dev.env.toml"), changed);
}

#[test]
fn a_write_after_the_plan_still_checks_the_hash() {
    let dir = scoped();
    let root = dir.path();
    let loaded = load(root);
    let planned = match reqflux_store::plan_variables(
        root,
        &loaded,
        ScopeFile::Globals,
        VariableChange::Add(Variable::new("a", "b")),
    ) {
        Ok(planned) => planned,
        Err(error) => panic!("plan: {error}"),
    };
    let changed = format!("id = \"{}\"\nname = \"X\"\nformat = 1\n", id_text(900));
    write(root, "reqflux.toml", &changed);
    let written = planned.write();
    assert!(
        matches!(written, Err(StoreError::Conflict { .. })),
        "{written:?}"
    );
    assert_eq!(read(root, "reqflux.toml"), changed);
}

#[test]
fn environments_are_created_numbered_with_new_ids() {
    let dir = minimal_workspace();
    let root = dir.path();
    let first = match reqflux_store::create_environment(root) {
        Ok(placed) => placed,
        Err(error) => panic!("create: {error}"),
    };
    let second = match reqflux_store::create_environment(root) {
        Ok(placed) => placed,
        Err(error) => panic!("create: {error}"),
    };
    assert_ne!(first.id, second.id);
    assert_eq!(
        first.path,
        root.join("environments/new-environment.env.toml")
    );
    assert_eq!(
        second.path,
        root.join("environments/new-environment-2.env.toml")
    );
    assert_eq!(
        read(root, "environments/new-environment-2.env.toml"),
        format!("id = \"{}\"\nname = \"New environment 2\"\n", second.id)
    );
    let loaded = load(root);
    let names: Vec<_> = loaded
        .workspace
        .environments
        .iter()
        .map(|node| node.environment.name.as_str())
        .collect();
    assert_eq!(names, ["New environment", "New environment 2"]);
}

#[test]
fn renaming_an_environment_moves_its_file_and_keeps_its_id() {
    let dir = scoped();
    let root = dir.path();
    let loaded = load(root);
    let placed = match reqflux_store::rename_environment(root, &loaded, id(3), " Staging EU ") {
        Ok(placed) => placed,
        Err(error) => panic!("rename: {error}"),
    };
    assert_eq!(placed.id, id(3));
    assert_eq!(placed.path, root.join("environments/staging-eu.env.toml"));
    assert_eq!(
        read(root, "environments/staging-eu.env.toml"),
        format!("id = \"{}\"\nname = \"Staging EU\"\n", id_text(3))
    );
    assert!(!root.join("environments/dev.env.toml").exists());
}

#[test]
fn an_environment_rename_clash_or_empty_name_changes_nothing() {
    let dir = scoped();
    let root = dir.path();
    write(
        root,
        "environments/prod.env.toml",
        &format!("id = \"{}\"\nname = \"prod\"\n", id_text(4)),
    );
    let before = files(root);
    let dev = read(root, "environments/dev.env.toml");
    let loaded = load(root);
    let clash = reqflux_store::rename_environment(root, &loaded, id(3), "Prod");
    assert!(
        matches!(clash, Err(StoreError::SlugTaken { .. })),
        "{clash:?}"
    );
    let empty = reqflux_store::rename_environment(root, &loaded, id(3), "  ");
    assert!(matches!(empty, Err(StoreError::EmptyName)), "{empty:?}");
    assert_eq!(files(root), before);
    assert_eq!(read(root, "environments/dev.env.toml"), dev);
}

#[test]
fn deleting_an_environment_removes_its_file_hash_checked() {
    let dir = scoped();
    let root = dir.path();
    let loaded = load(root);
    let changed = format!("id = \"{}\"\nname = \"edited\"\n", id_text(3));
    write(root, "environments/dev.env.toml", &changed);
    let conflict = reqflux_store::delete_environment(root, &loaded, id(3));
    assert!(
        matches!(conflict, Err(StoreError::Conflict { .. })),
        "{conflict:?}"
    );
    assert_eq!(read(root, "environments/dev.env.toml"), changed);
    let loaded = load(root);
    if let Err(error) = reqflux_store::delete_environment(root, &loaded, id(3)) {
        panic!("delete: {error}");
    }
    assert!(!root.join("environments/dev.env.toml").exists());
}

#[test]
fn save_environment_and_save_collection_round_trip_byte_for_byte() {
    let dir = scoped();
    let root = dir.path();
    let env_text = format!(
        "id = \"{}\"\nname = \"dev\"\n\n[[variables]]\nkey = \"a\"\nvalue = \"1\"\n",
        id_text(3)
    );
    write(root, "environments/dev.env.toml", &env_text);
    let col_text = format!(
        "id = \"{}\"\nname = \"Api\"\n\n[[variables]]\nkey = \"b\"\nsecret = true\n",
        id_text(1)
    );
    write(root, "collections/api/collection.toml", &col_text);
    let loaded = load(root);
    let env = &loaded.workspace.environments[0];
    let env_file = root.join("environments/dev.env.toml");
    if let Err(error) =
        reqflux_store::save_environment(root, &env.slug, &env.environment, loaded.hash(&env_file))
    {
        panic!("save environment: {error}");
    }
    let col = &loaded.workspace.collections[0];
    let col_file = root.join("collections/api/collection.toml");
    if let Err(error) =
        reqflux_store::save_collection(root, &col.slug, &col.collection, loaded.hash(&col_file))
    {
        panic!("save collection: {error}");
    }
    assert_eq!(read(root, "environments/dev.env.toml"), env_text);
    assert_eq!(read(root, "collections/api/collection.toml"), col_text);
}
