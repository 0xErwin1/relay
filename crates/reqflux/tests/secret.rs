//! `reqflux secret` end to end: the built binary against a keyring that is not available
//! (keyring-secrets S6, S8).
//!
//! Every run points D-Bus at a socket that does not exist, so the Linux Secret Service
//! cannot be reached and no test touches the real keyring. On macOS the Keychain is not
//! behind D-Bus; there these tests reach it, though only `set` would write and it targets
//! a temporary workspace id.

#![allow(clippy::panic, reason = "test helpers fail the calling test")]

use std::fs;
use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use pretty_assertions::assert_eq;
use tempfile::TempDir;

const NO_BUS: &str = "unix:path=/nonexistent/reqflux-test-bus";

fn write(root: &Path, relative: &str, content: &str) {
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

/// Global secret `token`, plain global `base`, collection `api` with secret `key` and
/// environment `dev` with secret `pass`.
fn workspace() -> TempDir {
    let dir = TempDir::new().unwrap_or_else(|error| panic!("temp dir: {error}"));
    let root = dir.path();
    write(
        root,
        "reqflux.toml",
        "id = \"01J00000000000000000000900\"\nname = \"W\"\nformat = 1\n\n[[globals]]\nkey = \"token\"\nsecret = true\n\n[[globals]]\nkey = \"base\"\nvalue = \"http://x\"\n",
    );
    write(
        root,
        "collections/api/collection.toml",
        "id = \"01J00000000000000000000003\"\nname = \"Api\"\n\n[[variables]]\nkey = \"key\"\nsecret = true\n",
    );
    write(
        root,
        "environments/dev.env.toml",
        "id = \"01J00000000000000000000001\"\nname = \"Dev\"\n\n[[variables]]\nkey = \"pass\"\nsecret = true\n",
    );
    dir
}

/// Runs the binary with `stdin` piped in.
fn reqflux(args: &[&str], stdin: &str) -> Output {
    let child = Command::new(env!("CARGO_BIN_EXE_reqflux"))
        .args(args)
        .env_remove("RUST_LOG")
        .env("DBUS_SESSION_BUS_ADDRESS", NO_BUS)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();
    let mut child = child.unwrap_or_else(|error| panic!("spawn reqflux: {error}"));
    if let Some(mut input) = child.stdin.take()
        && let Err(error) = input.write_all(stdin.as_bytes())
    {
        panic!("write stdin: {error}");
    }
    child
        .wait_with_output()
        .unwrap_or_else(|error| panic!("wait reqflux: {error}"))
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn root(dir: &TempDir) -> String {
    dir.path().to_string_lossy().into_owned()
}

fn assert_usage(output: &Output, stderr: &str) {
    assert_eq!(text(&output.stderr), stderr);
    assert_eq!(text(&output.stdout), "");
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn usage_errors_exit_2_and_name_the_problem() {
    let dir = workspace();
    let root = root(&dir);
    let cases = [
        (
            vec!["secret", "set", "web", "key", "--workspace", &root],
            "error: unknown scope web: expected global, a collection slug or env:<slug>\n",
        ),
        (
            vec!["secret", "unset", "global", "base", "--workspace", &root],
            "error: base in global is not a secret variable\n",
        ),
        (
            vec!["secret", "set", "env:dev", "nope", "--workspace", &root],
            "error: env:dev has no secret variable nope\n",
        ),
        (
            vec!["secret", "set", "global", "token", "--workspace", &root],
            "error: the value for token is empty\n",
        ),
    ];
    for (args, stderr) in cases {
        assert_usage(&reqflux(&args, "\n"), stderr);
    }
}

#[test]
fn a_workspace_that_fails_to_load_is_a_usage_error() {
    let dir = TempDir::new().unwrap_or_else(|error| panic!("temp dir: {error}"));
    let output = reqflux(&["secret", "list", &root(&dir)], "");
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(text(&output.stdout), "");
    assert!(text(&output.stderr).starts_with("error: "), "{output:?}");
}

// Only Linux has a keyring the test can make unavailable (a bus that does not exist);
// elsewhere this would reach the real keyring.
#[cfg(target_os = "linux")]
#[test]
fn list_without_a_keyring_says_unknown_and_warns() {
    let dir = workspace();
    let output = reqflux(&["secret", "list", &root(&dir)], "");
    assert_eq!(
        text(&output.stdout),
        "global\ttoken\tunknown\napi\tkey\tunknown\nenv:dev\tpass\tunknown\n"
    );
    let stderr = text(&output.stderr);
    assert!(
        stderr.starts_with("warning: could not read the keyring: the OS keyring is not available"),
        "{stderr}"
    );
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
    assert_eq!(output.status.code(), Some(0));
}

// Only Linux has a keyring the test can make unavailable (a bus that does not exist);
// elsewhere this would reach the real keyring.
#[cfg(target_os = "linux")]
#[test]
fn set_from_stdin_without_a_keyring_exits_1_and_never_shows_the_value() {
    let dir = workspace();
    let output = reqflux(
        &[
            "secret",
            "set",
            "global",
            "token",
            "--workspace",
            &root(&dir),
        ],
        "hunter2\n",
    );
    let stderr = text(&output.stderr);
    assert!(
        stderr.starts_with("error: the OS keyring is not available"),
        "{stderr}"
    );
    assert!(!stderr.contains("hunter2") && !text(&output.stdout).contains("hunter2"));
    assert_eq!(text(&output.stdout), "");
    assert_eq!(output.status.code(), Some(1));
    for entry in walk(dir.path()) {
        let content = fs::read(&entry).unwrap_or_default();
        assert!(!text(&content).contains("hunter2"), "{}", entry.display());
    }
}

#[test]
fn the_value_is_never_an_argument() {
    let dir = workspace();
    let output = reqflux(
        &[
            "secret",
            "set",
            "global",
            "token",
            "hunter2",
            "--workspace",
            &root(&dir),
        ],
        "",
    );
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn help_explains_scopes_stdin_and_exit_codes() {
    for args in [
        vec!["secret", "--help"],
        vec!["secret", "set", "--help"],
        vec!["secret", "unset", "--help"],
        vec!["secret", "list", "--help"],
    ] {
        let output = reqflux(&args, "");
        let help = text(&output.stdout);
        for expected in ["env:<slug>", "Exit codes", "never"] {
            assert!(
                help.contains(expected),
                "{args:?}: {expected} missing: {help}"
            );
        }
        if args.get(1) != Some(&"list") {
            assert!(help.contains("stdin"), "{args:?}: stdin missing: {help}");
        }
        assert_eq!(output.status.code(), Some(0));
    }
}

fn walk(root: &Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else {
                files.push(path);
            }
        }
    }
    files
}
