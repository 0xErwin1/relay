//! `reqflux import` end to end: the built binary against a Postman collection (S10).

#![allow(
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test helpers fail the calling test"
)]

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use pretty_assertions::assert_eq;
use tempfile::TempDir;

const COLLECTION: &str = r#"{
  "info": {
    "name": "Billing",
    "_postman_id": "6f3b1c1e-0000-4000-8000-000000000001",
    "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
  },
  "item": [
    {
      "name": "Invoices",
      "item": [
        {
          "name": "Create invoice",
          "request": {
            "method": "POST",
            "header": [
              { "key": "Content-Type", "value": "application/json" }
            ],
            "url": {
              "raw": "https://api.test/v1/invoices",
              "host": ["api", "test"],
              "path": ["v1", "invoices"]
            },
            "body": {
              "mode": "raw",
              "raw": "{\"currency\":\"usd\"}",
              "options": { "raw": { "language": "json" } }
            }
          }
        }
      ]
    }
  ]
}"#;

const ENVIRONMENT: &str = r#"{
  "name": "Staging",
  "values": [
    { "key": "base_url", "value": "https://api.staging.test", "enabled": true },
    { "key": "token", "value": "secret-value", "enabled": true, "type": "secret" }
  ]
}"#;

fn reqflux(args: &[&str]) -> Output {
    match Command::new(env!("CARGO_BIN_EXE_reqflux"))
        .args(args)
        .env_remove("RUST_LOG")
        .output()
    {
        Ok(output) => output,
        Err(error) => panic!("run reqflux: {error}"),
    }
}

fn write(dir: &Path, name: &str, content: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    if let Err(error) = fs::write(&path, content) {
        panic!("write {}: {error}", path.display());
    }
    path
}

fn temp() -> TempDir {
    match TempDir::new() {
        Ok(dir) => dir,
        Err(error) => panic!("temp dir: {error}"),
    }
}

#[test]
fn imports_a_collection_and_an_environment() {
    let dir = temp();
    let collection = write(dir.path(), "collection.json", COLLECTION);
    let environment = write(dir.path(), "staging.json", ENVIRONMENT);
    let workspace = dir.path().join("workspace");
    let output = reqflux(&[
        "import",
        collection.to_str().unwrap_or("collection.json"),
        "--workspace",
        workspace.to_str().unwrap_or("workspace"),
        "--environment",
        environment.to_str().unwrap_or("staging.json"),
    ]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(output.status.code(), Some(0), "stdout: {stdout}");
    assert!(stdout.contains("collection billing:"), "{stdout}");
    assert!(
        stdout.contains("1 folders, 1 requests, 1 environments"),
        "{stdout}"
    );

    assert!(
        workspace.join("reqflux.toml").exists(),
        "a manifest was written"
    );
    let request = workspace.join("collections/billing/invoices/create-invoice.req.toml");
    let saved = fs::read_to_string(&request).unwrap_or_default();
    assert!(saved.contains("api.test"), "{saved}");
    assert!(saved.contains("usd"), "{saved}");
    let environment = fs::read_to_string(workspace.join("environments/staging.env.toml"));
    let environment = environment.unwrap_or_default();
    assert!(environment.contains("base_url"), "{environment}");
    assert!(
        !environment.contains("secret-value"),
        "a secret value never reaches the file: {environment}"
    );
}

#[test]
fn a_second_import_refuses_to_overwrite() {
    let dir = temp();
    let collection = write(dir.path(), "collection.json", COLLECTION);
    let workspace = dir.path().join("workspace");
    let args = [
        "import",
        collection.to_str().unwrap_or("collection.json"),
        "--workspace",
        workspace.to_str().unwrap_or("workspace"),
    ];
    assert_eq!(reqflux(&args).status.code(), Some(0));
    let request = workspace.join("collections/billing/invoices/create-invoice.req.toml");
    let before = fs::read_to_string(&request).ok();
    let again = reqflux(&args);
    assert_eq!(again.status.code(), Some(1), "{again:?}");
    assert!(
        String::from_utf8_lossy(&again.stderr).contains("already exists"),
        "{}",
        String::from_utf8_lossy(&again.stderr)
    );
    let after = fs::read_to_string(&request).ok();
    assert_eq!(before, after, "the existing file is untouched");
}

#[test]
fn a_document_that_is_not_json_fails() {
    let dir = temp();
    let broken = write(dir.path(), "broken.json", "{ not json");
    let output = reqflux(&[
        "import",
        broken.to_str().unwrap_or("broken.json"),
        "--workspace",
        dir.path().join("workspace").to_str().unwrap_or("workspace"),
    ]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("is not JSON"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn a_missing_file_is_a_usage_error() {
    let dir = temp();
    let output = reqflux(&[
        "import",
        dir.path()
            .join("nothing.json")
            .to_str()
            .unwrap_or("nothing.json"),
        "--workspace",
        dir.path().to_str().unwrap_or("."),
    ]);
    assert_eq!(output.status.code(), Some(2), "{output:?}");
}
