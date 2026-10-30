//! Secret variables resolved from the secret store (keyring-secrets S1-S5, S8). Every test
//! uses the in-memory store; none touches the OS keyring.

#![allow(
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test helpers fail the calling test"
)]

mod common;

use std::path::Path;
use std::sync::Arc;

use common::{id, id_text, temp_dir, write};
use pretty_assertions::assert_eq;
use reqflux_engine::{
    Engine, ExecuteOptions, MemorySecretStore, Prepared, SecretError, SecretKey, SecretScope,
    SecretStore,
};

const WORKSPACE: u32 = 900;
const COLLECTION: u32 = 1;
const ENVIRONMENT: u32 = 3;

fn manifest(root: &Path, globals: &str) {
    write(
        root,
        "reqflux.toml",
        &format!(
            "id = \"{}\"\nname = \"W\"\nformat = 1\n{globals}",
            id_text(WORKSPACE)
        ),
    );
}

fn collection(root: &Path, variables: &str) {
    write(
        root,
        "collections/api/collection.toml",
        &format!(
            "id = \"{}\"\nname = \"Api\"\n{variables}",
            id_text(COLLECTION)
        ),
    );
}

fn environment(root: &Path, variables: &str) {
    write(
        root,
        "environments/dev.env.toml",
        &format!(
            "id = \"{}\"\nname = \"dev\"\n{variables}",
            id_text(ENVIRONMENT)
        ),
    );
}

fn request(root: &Path, url: &str, extra: &str) {
    write(
        root,
        "collections/api/r.req.toml",
        &format!(
            "id = \"{}\"\nname = \"r\"\nmethod = \"GET\"\nurl = \"{url}\"\n{extra}",
            id_text(2)
        ),
    );
}

const SECRET_TOKEN: &str = "\n[[variables]]\nkey = \"token\"\nsecret = true\n";

fn key(scope: SecretScope, name: &str) -> SecretKey {
    SecretKey {
        workspace_id: id(WORKSPACE),
        scope,
        key: name.to_owned(),
    }
}

fn store_with(entries: &[(SecretKey, &str)]) -> Arc<MemorySecretStore> {
    let store = MemorySecretStore::new();
    for (key, value) in entries {
        if let Err(error) = store.set(key, value) {
            panic!("set: {error}");
        }
    }
    Arc::new(store)
}

fn open(root: &Path, store: Arc<dyn SecretStore>) -> Engine {
    match Engine::open_with_secrets(root, store) {
        Ok(engine) => engine,
        Err(error) => panic!("open: {error}"),
    }
}

fn prepared(engine: &Engine, options: &ExecuteOptions) -> Prepared {
    match engine.prepare("api/r", options) {
        Ok(prepared) => prepared,
        Err(error) => panic!("prepare: {error}"),
    }
}

fn header<'a>(prepared: &'a Prepared, name: &str) -> Option<&'a str> {
    prepared
        .request
        .headers
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.as_str())
}

#[test]
fn s1_account_strings_name_the_workspace_scope_and_key() {
    let workspace = id_text(WORKSPACE);
    let collection = id_text(COLLECTION);
    let environment = id_text(ENVIRONMENT);
    assert_eq!(
        key(SecretScope::Global, "token").account(),
        format!("{workspace}/global/token")
    );
    assert_eq!(
        key(SecretScope::Collection(id(COLLECTION)), "token").account(),
        format!("{workspace}/collection/{collection}/token")
    );
    assert_eq!(
        key(SecretScope::Environment(id(ENVIRONMENT)), "token").account(),
        format!("{workspace}/environment/{environment}/token")
    );
    assert_eq!(reqflux_engine::SECRET_SERVICE, "reqflux");
}

#[test]
fn s2_the_memory_store_reads_writes_deletes_and_reports_set() {
    let store = MemorySecretStore::new();
    let token = key(SecretScope::Global, "token");
    assert!(matches!(store.get(&token), Ok(None)));
    assert!(matches!(store.is_set(&token), Ok(false)));
    assert!(store.set(&token, "s3cr3t").is_ok());
    assert!(matches!(
        store.get(&token).as_ref().map(Option::as_deref),
        Ok(Some("s3cr3t"))
    ));
    assert!(matches!(store.is_set(&token), Ok(true)));
    assert_eq!(store.accounts(), vec![token.account()]);
    assert!(store.delete(&token).is_ok());
    assert!(matches!(store.get(&token), Ok(None)));
    assert!(
        store.delete(&token).is_ok(),
        "deleting a missing entry is fine"
    );
}

#[test]
fn s2_an_unavailable_store_errors_without_the_value() {
    let store = MemorySecretStore::unavailable("no Secret Service on this bus");
    let token = key(SecretScope::Global, "token");
    let error = match store.set(&token, "s3cr3t-value") {
        Ok(()) => panic!("set unexpectedly succeeded"),
        Err(error) => error,
    };
    assert!(matches!(error, SecretError::Unavailable(_)));
    let text = error.to_string();
    assert!(!text.contains("s3cr3t-value"), "{text}");
    assert!(text.contains("OS keyring is not available"), "{text}");
    assert!(text.contains("no Secret Service on this bus"), "{text}");
    assert!(matches!(
        store.get(&token),
        Err(SecretError::Unavailable(_))
    ));
}

#[test]
fn s3_a_global_secret_resolves_from_the_store() {
    let dir = temp_dir();
    let root = dir.path();
    manifest(root, "\n[[globals]]\nkey = \"token\"\nsecret = true\n");
    collection(root, "");
    request(root, "https://h/{{token}}", "");
    let store = store_with(&[(key(SecretScope::Global, "token"), "g-secret")]);
    let engine = open(root, store);

    let prepared = prepared(&engine, &ExecuteOptions::default());

    assert_eq!(prepared.request.url, "https://h/g-secret");
    assert!(prepared.unresolved.is_empty());
    assert_eq!(prepared.secret_problem, None);
}

#[test]
fn s3_a_collection_secret_resolves_by_collection_id() {
    let dir = temp_dir();
    let root = dir.path();
    manifest(root, "");
    collection(root, SECRET_TOKEN);
    request(root, "https://h/{{token}}", "");
    let store = store_with(&[(
        key(SecretScope::Collection(id(COLLECTION)), "token"),
        "c-secret",
    )]);
    let engine = open(root, store);

    assert_eq!(
        prepared(&engine, &ExecuteOptions::default()).request.url,
        "https://h/c-secret"
    );
}

#[test]
fn s3_an_environment_secret_resolves_by_environment_id() {
    let dir = temp_dir();
    let root = dir.path();
    manifest(root, "");
    collection(root, "");
    environment(root, SECRET_TOKEN);
    request(root, "https://h/{{token}}", "");
    let store = store_with(&[
        (key(SecretScope::Global, "token"), "wrong-scope"),
        (
            key(SecretScope::Environment(id(ENVIRONMENT)), "token"),
            "e-secret",
        ),
    ]);
    let engine = open(root, store);
    let options = ExecuteOptions {
        environment: Some("dev".to_owned()),
        ..ExecuteOptions::default()
    };

    assert_eq!(
        prepared(&engine, &options).request.url,
        "https://h/e-secret"
    );
}

#[test]
fn s1_a_renamed_collection_keeps_resolving() {
    let dir = temp_dir();
    let root = dir.path();
    manifest(root, "");
    collection(root, SECRET_TOKEN);
    request(root, "https://h/{{token}}", "");
    let store = store_with(&[(
        key(SecretScope::Collection(id(COLLECTION)), "token"),
        "c-secret",
    )]);
    let mut engine = open(root, store);
    if let Err(error) = engine.rename(id(COLLECTION), "Renamed") {
        panic!("rename: {error}");
    }
    let slug = engine.workspace().collections[0].slug.clone();
    assert_ne!(slug, "api");

    let prepared = match engine.prepare(&format!("{slug}/r"), &ExecuteOptions::default()) {
        Ok(prepared) => prepared,
        Err(error) => panic!("prepare: {error}"),
    };
    assert_eq!(prepared.request.url, "https://h/c-secret");
}

#[test]
fn s3_a_var_override_wins_over_the_store() {
    let dir = temp_dir();
    let root = dir.path();
    manifest(root, "");
    collection(root, SECRET_TOKEN);
    request(root, "https://h/{{token}}", "");
    let store = store_with(&[(
        key(SecretScope::Collection(id(COLLECTION)), "token"),
        "c-secret",
    )]);
    let engine = open(root, store);
    let options = ExecuteOptions {
        vars: vec![("token".to_owned(), "from-var".to_owned())],
        ..ExecuteOptions::default()
    };

    let prepared = prepared(&engine, &options);
    assert_eq!(prepared.request.url, "https://h/from-var");
    // Both the winner and the shadowed keyring value are masked.
    let masker = prepared.masker();
    assert_eq!(masker.mask("from-var c-secret"), "•••••••• ••••••••");
}

#[test]
fn s3_an_unset_secret_stays_unresolved() {
    let dir = temp_dir();
    let root = dir.path();
    manifest(root, "");
    collection(root, SECRET_TOKEN);
    request(root, "https://h/{{token}}", "");
    let engine = open(root, Arc::new(MemorySecretStore::new()));

    let prepared = prepared(&engine, &ExecuteOptions::default());
    assert_eq!(prepared.request.url, "https://h/{{token}}");
    assert_eq!(prepared.unresolved, vec!["token".to_owned()]);
    assert_eq!(prepared.secret_problem, None);
}

#[test]
fn s2_an_unavailable_store_leaves_secrets_unresolved_and_says_why() {
    let dir = temp_dir();
    let root = dir.path();
    manifest(root, "\n[[globals]]\nkey = \"other\"\nsecret = true\n");
    collection(root, SECRET_TOKEN);
    request(root, "https://h/{{token}}/{{other}}", "");
    let engine = open(
        root,
        Arc::new(MemorySecretStore::unavailable(
            "no Secret Service on this bus",
        )),
    );

    let prepared = prepared(&engine, &ExecuteOptions::default());
    assert_eq!(prepared.request.url, "https://h/{{token}}/{{other}}");
    assert_eq!(
        prepared.unresolved,
        vec!["token".to_owned(), "other".to_owned()]
    );
    assert_eq!(
        prepared.secret_problem.as_deref(),
        Some(
            "secret variables were not read from the keyring: the OS keyring is not \
             available: no Secret Service on this bus"
        )
    );

    let inspection = match engine.inspect("api/r", None) {
        Ok(inspection) => inspection,
        Err(error) => panic!("inspect: {error}"),
    };
    assert_eq!(inspection.secret_problem, prepared.secret_problem);
    assert_eq!(inspection.problem, None);
}

#[test]
fn s4_inspection_reports_a_keyring_secret_resolved_and_masked() {
    let dir = temp_dir();
    let root = dir.path();
    manifest(root, "");
    collection(root, SECRET_TOKEN);
    request(root, "https://h/{{token}}", "");
    let store = store_with(&[(
        key(SecretScope::Collection(id(COLLECTION)), "token"),
        "c-secret",
    )]);
    let engine = open(root, store);

    let inspection = match engine.inspect("api/r", None) {
        Ok(inspection) => inspection,
        Err(error) => panic!("inspect: {error}"),
    };
    assert!(inspection.unresolved.is_empty());
    assert_eq!(inspection.secret_problem, None);
    assert!(inspection.references_secret("{{token}}"));
    assert!(matches!(
        inspection.spans("{{token}}").as_slice(),
        [reqflux_engine::Span::Reference {
            resolves: true,
            secret: true,
            ..
        }]
    ));
    assert!(!format!("{inspection:?}").contains("c-secret"));
}

#[test]
fn s5_the_sent_copy_and_the_masker_mask_a_keyring_value() {
    let dir = temp_dir();
    let root = dir.path();
    manifest(root, "");
    collection(root, SECRET_TOKEN);
    request(
        root,
        "https://h/{{token}}",
        "\n[[headers]]\nkey = \"X-Token\"\nvalue = \"{{token}}\"\n",
    );
    let store = store_with(&[(
        key(SecretScope::Collection(id(COLLECTION)), "token"),
        "c-secret",
    )]);
    let engine = open(root, store);

    let prepared = prepared(&engine, &ExecuteOptions::default());
    assert_eq!(header(&prepared, "X-Token"), Some("c-secret"));
    let sent = prepared.sent();
    assert_eq!(sent.url, "https://h/••••••••");
    assert!(
        sent.headers
            .iter()
            .any(|(key, value)| key == "X-Token" && value == "••••••••")
    );
    assert_eq!(prepared.masker().mask("echo c-secret"), "echo ••••••••");
}

#[test]
fn s3_a_draft_execution_resolves_from_the_store() {
    let dir = temp_dir();
    let root = dir.path();
    manifest(root, "");
    collection(root, SECRET_TOKEN);
    request(root, "http://127.0.0.1:9/{{token}}", "");
    let store = store_with(&[(
        key(SecretScope::Collection(id(COLLECTION)), "token"),
        "c-secret",
    )]);
    let engine = open(root, store);
    let draft = engine.workspace().collections[0]
        .items
        .iter()
        .find_map(|item| match item {
            reqflux_core::model::Item::Request(node) => Some(node.request.clone()),
            reqflux_core::model::Item::Folder(_) => None,
        });
    let Some(draft) = draft else {
        panic!("no request");
    };

    let execution = match engine.execute_draft("api/r", &draft, &ExecuteOptions::default()) {
        Ok(execution) => execution,
        Err(error) => panic!("execute: {error}"),
    };
    execution.cancel.cancel();
    assert_eq!(execution.url, "http://127.0.0.1:9/c-secret");
    assert!(execution.unresolved.is_empty());
    assert_eq!(execution.secret_problem, None);
    assert_eq!(execution.sent.url, "http://127.0.0.1:9/••••••••");
}

#[test]
fn a_workspace_without_secrets_prepares_as_before() {
    let dir = temp_dir();
    let root = dir.path();
    manifest(root, "\n[[globals]]\nkey = \"host\"\nvalue = \"h\"\n");
    collection(root, "");
    request(root, "https://{{host}}/{{missing}}", "");
    let store = Arc::new(MemorySecretStore::unavailable("must not be read"));
    let engine = open(root, store);

    let prepared = prepared(&engine, &ExecuteOptions::default());
    let pure = match reqflux_engine::prepare(
        engine.workspace(),
        root,
        "api/r",
        &ExecuteOptions::default(),
        &mut reqflux_core::template::SequentialDynamicSource::default(),
    ) {
        Ok(prepared) => prepared,
        Err(error) => panic!("prepare: {error}"),
    };
    assert_eq!(prepared, pure);
    assert_eq!(prepared.request.url, "https://h/{{missing}}");
    assert_eq!(prepared.unresolved, vec!["missing".to_owned()]);
    assert_eq!(prepared.secret_problem, None);
}
