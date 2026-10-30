//! The examples in ARCHITECTURE.md "File schemas" load as documented, so the docs cannot
//! drift from the format.

#![allow(clippy::panic, reason = "the helper fails the calling test")]

mod common;

use std::path::Path;

use common::*;
use pretty_assertions::assert_eq;
use reqflux_core::model::{Auth, Body, Item, Method};

/// The TOML code blocks under "### File schemas", in document order.
fn documented_blocks() -> Vec<String> {
    let architecture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ARCHITECTURE.md");
    let text = std::fs::read_to_string(&architecture)
        .unwrap_or_else(|error| panic!("{}: {error}", architecture.display()));
    let section = text
        .split_once("### File schemas")
        .map(|(_, rest)| rest)
        .unwrap_or_else(|| panic!("ARCHITECTURE.md has no File schemas section"));
    let section = section.split("\n## ").next().unwrap_or(section);
    section
        .split("```toml\n")
        .skip(1)
        .filter_map(|block| block.split_once("```").map(|(toml, _)| toml.to_owned()))
        .collect()
}

#[test]
fn the_documented_examples_load_without_warnings() {
    let blocks = documented_blocks();
    let [manifest, environment, collection, request] = blocks.as_slice() else {
        panic!("expected 4 TOML examples, found {}", blocks.len());
    };
    let dir = workspace();
    let root = dir.path();
    write(root, "reqflux.toml", manifest);
    write(root, "environments/staging.env.toml", environment);
    write(root, "collections/billing-api/collection.toml", collection);
    write(
        root,
        "collections/billing-api/create-invoice.req.toml",
        request,
    );

    let loaded = load(root);

    assert_eq!(warnings(&loaded), Vec::<String>::new());
    let workspace = &loaded.workspace;
    assert_eq!(workspace.manifest.globals.len(), 2);
    assert_eq!(workspace.environments[0].environment.variables.len(), 2);
    let collection = &workspace.collections[0];
    assert!(matches!(collection.collection.auth, Auth::Bearer { .. }));
    let [Item::Request(request)] = collection.items.as_slice() else {
        panic!("expected one request, got {:?}", collection.items);
    };
    let request = &request.request;
    assert_eq!(request.method, Method::POST);
    assert_eq!(request.url, "{{base_url}}/v1/invoices");
    assert_eq!(request.params[0].key, "expand");
    assert!(matches!(request.auth, Auth::ApiKey { .. }));
    assert!(matches!(request.body, Body::Raw { .. }));
    assert_eq!(request.settings.timeout_ms, Some(5000));
}
