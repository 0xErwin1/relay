//! S6: the workspace format version.

mod common;

use common::*;
use pretty_assertions::assert_eq;

#[test]
fn a_newer_format_fails_with_the_update_message() {
    let dir = workspace();
    write(
        dir.path(),
        "relay.toml",
        &format!("{}future = {{ x = 1 }}\n", manifest(2)),
    );

    assert_eq!(
        load_err(dir.path()).to_string(),
        "workspace format 2 is newer than this version of Relay supports (1); update Relay to open it"
    );
}

#[test]
fn a_missing_format_fails_naming_the_manifest() {
    let dir = workspace();
    write(
        dir.path(),
        "relay.toml",
        &format!("id = \"{}\"\nname = \"W\"\n", id_text(1)),
    );

    assert_eq!(
        load_err(dir.path()).to_string(),
        format!(
            "{}: missing or unsupported workspace format",
            path(dir.path(), "relay.toml").display()
        )
    );
}

#[test]
fn a_lower_or_non_integer_format_fails_naming_the_manifest() {
    for format in ["0", "\"1\""] {
        let dir = workspace();
        write(
            dir.path(),
            "relay.toml",
            &format!("id = \"{}\"\nname = \"W\"\nformat = {format}\n", id_text(1)),
        );

        assert_eq!(
            load_err(dir.path()).to_string(),
            format!(
                "{}: missing or unsupported workspace format",
                path(dir.path(), "relay.toml").display()
            )
        );
    }
}

#[test]
fn a_missing_manifest_fails_naming_it() {
    let dir = workspace();

    let message = load_err(dir.path()).to_string();

    let prefix = format!("{}: ", path(dir.path(), "relay.toml").display());
    assert!(message.starts_with(&prefix), "{message}");
}

#[test]
fn format_one_loads() {
    let dir = minimal_workspace();

    assert_eq!(load(dir.path()).workspace.manifest.format, 1);
}
