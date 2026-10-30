//! The Body, Auth and Settings panes read the draft (S1), and the Settings pane edits it
//! (S2): the timeout field, the redirects and TLS cycles, and their persistence (S8).

use gpui::{Entity, TestAppContext, VisualTestContext};
use reqflux_core::model::{Auth, Body, RawLanguage, Request};
use tempfile::TempDir;

use super::Shell;
use super::draft_tests::{bounds, click, marker, render, state_file};
use super::drafts::RequestDraft;
use crate::workspace_tests::{demo_workspace, write};

const COLLECTION: &str = "01J00000000000000000000061";
const TUNED: &str = "01J00000000000000000000062";
const FILE: &str = "collections/api/tuned.req.toml";

/// A workspace holding `GET Tuned`, with no body, no auth and a 5000 ms timeout.
fn settings_workspace() -> TempDir {
    let dir = demo_workspace();
    let root = dir.path();
    write(
        root,
        "collections/api/collection.toml",
        &format!("id = \"{COLLECTION}\"\nname = \"API\"\n"),
    );
    write(
        root,
        FILE,
        &format!(
            "id = \"{TUNED}\"\nname = \"Tuned\"\nmethod = \"GET\"\nurl = \"https://h/t\"\n\n\
             [settings]\ntimeout_ms = 5000\n"
        ),
    );
    dir
}

/// Opens `GET Tuned` on its Settings sub-tab.
fn open_settings(cx: &mut VisualTestContext) {
    click(cx, &format!("tree-row-{TUNED}"));
    click(cx, "request-tab-settings");
}

fn draft_request(cx: &mut TestAppContext, shell: &Entity<Shell>) -> Option<Request> {
    cx.update(|cx| {
        shell
            .read(cx)
            .drafts
            .get(TUNED)
            .map(|draft| draft.request.clone())
    })
}

/// The text of the open timeout field, if it is open.
fn field_text(cx: &mut TestAppContext, shell: &Entity<Shell>) -> Option<String> {
    cx.update(|cx| {
        shell
            .read(cx)
            .cell_edit
            .as_ref()
            .map(|edit| edit.input.read(cx).value().to_string())
    })
}

/// Opens the timeout field, replaces its text with `text` and presses `key`, if any.
fn type_timeout(cx: &mut VisualTestContext, text: &str, key: &str) {
    click(cx, "request-setting-timeout");
    assert!(
        bounds(cx, "request-setting-timeout-input").is_some(),
        "the field opened"
    );
    cx.simulate_keystrokes("ctrl-a backspace");
    for character in text.chars() {
        cx.simulate_keystrokes(&character.to_string());
    }
    if !key.is_empty() {
        cx.simulate_keystrokes(key);
    }
    cx.run_until_parked();
}

#[gpui::test]
fn the_panes_show_the_drafts_body_auth_and_settings(cx: &mut TestAppContext) {
    let dir = settings_workspace();
    let (_state_dir, state_path) = state_file();
    let (shell, cx) = render(cx, &dir, &state_path);
    click(cx, &format!("tree-row-{TUNED}"));
    shell.update(cx, |shell, _| {
        let Some(state) = shell.requests.get(TUNED) else {
            panic!("the request is open");
        };
        let mut request = state.inspection.request.request.clone();
        request.body = Body::Raw {
            language: RawLanguage::Text,
            text: "a\nb\nc".to_owned(),
        };
        request.auth = Auth::Bearer {
            token: "plain".to_owned(),
        };
        request.settings.timeout_ms = Some(1234);
        shell.drafts.insert(
            TUNED.to_owned(),
            RequestDraft {
                request,
                base_hash: None,
                conflicted: false,
            },
        );
    });

    click(cx, "request-tab-body");
    assert!(
        bounds(cx, "request-body-line-3").is_some(),
        "the draft body"
    );
    assert!(bounds(cx, "request-body-empty").is_none());
    click(cx, "request-tab-auth");
    assert!(
        bounds(cx, "request-auth-field-token").is_some(),
        "the draft auth"
    );
    click(cx, "request-tab-settings");
    click(cx, "request-setting-timeout");
    assert_eq!(field_text(cx, &shell).as_deref(), Some("1234"));
}

#[gpui::test]
fn a_timeout_commit_lands_in_the_draft_and_survives_a_restart(cx: &mut TestAppContext) {
    let dir = settings_workspace();
    let (_state_dir, state_path) = state_file();
    let (shell, cx) = render(cx, &dir, &state_path);
    open_settings(cx);

    type_timeout(cx, "2500", "enter");
    assert!(bounds(cx, "request-setting-timeout-input").is_none());
    assert!(marker(cx, TUNED), "the edit marks the tab");
    let draft = draft_request(cx, &shell).expect("the commit created the draft");
    assert_eq!(draft.settings.timeout_ms, Some(2500));

    let (shell_2, cx) = render(cx, &dir, &state_path);
    let restored = draft_request(cx, &shell_2).expect("the draft was restored");
    assert_eq!(restored.settings.timeout_ms, Some(2500));

    open_settings(cx);
    cx.simulate_keystrokes("ctrl-s");
    cx.run_until_parked();
    let file = std::fs::read_to_string(dir.path().join(FILE)).unwrap_or_default();
    assert!(
        file.contains("timeout_ms = 2500"),
        "Ctrl+S wrote it: {file}"
    );
}

#[gpui::test]
fn an_empty_timeout_commit_stores_the_default(cx: &mut TestAppContext) {
    let dir = settings_workspace();
    let (_state_dir, state_path) = state_file();
    let (shell, cx) = render(cx, &dir, &state_path);
    open_settings(cx);

    type_timeout(cx, "", "enter");
    let draft = draft_request(cx, &shell).expect("the commit created the draft");
    assert_eq!(draft.settings.timeout_ms, None);
}

#[gpui::test]
fn invalid_or_unchanged_timeouts_and_escape_create_no_draft(cx: &mut TestAppContext) {
    let dir = settings_workspace();
    let (_state_dir, state_path) = state_file();
    let (shell, cx) = render(cx, &dir, &state_path);
    open_settings(cx);

    type_timeout(cx, "12s", "enter");
    assert!(bounds(cx, "request-setting-timeout-input").is_none());
    assert!(
        draft_request(cx, &shell).is_none(),
        "invalid text is rejected"
    );
    type_timeout(cx, "5000", "enter");
    assert!(draft_request(cx, &shell).is_none(), "the same value");
    type_timeout(cx, "77", "escape");
    assert!(bounds(cx, "request-setting-timeout-input").is_none());
    assert!(draft_request(cx, &shell).is_none(), "Escape discards");
    assert!(!marker(cx, TUNED));

    type_timeout(cx, "900", "enter");
    type_timeout(cx, "-1", "enter");
    let draft = draft_request(cx, &shell).expect("the draft");
    assert_eq!(
        draft.settings.timeout_ms,
        Some(900),
        "the draft is unchanged"
    );
}

#[gpui::test]
fn redirects_and_tls_cycle_default_on_off(cx: &mut TestAppContext) {
    let dir = settings_workspace();
    let (_state_dir, state_path) = state_file();
    let (shell, cx) = render(cx, &dir, &state_path);
    open_settings(cx);

    for (id, read) in [
        (
            "request-setting-redirects",
            (|request: &Request| request.settings.follow_redirects) as fn(&Request) -> _,
        ),
        ("request-setting-tls", |request: &Request| {
            request.settings.verify_tls
        }),
    ] {
        for expected in [Some(true), Some(false), None] {
            click(cx, id);
            let draft = draft_request(cx, &shell).expect("the click created the draft");
            assert_eq!(read(&draft), expected, "{id}");
        }
    }
    assert!(marker(cx, TUNED));

    click(cx, "request-setting-tls");
    let (shell_2, cx) = render(cx, &dir, &state_path);
    let restored = draft_request(cx, &shell_2).expect("the draft was restored");
    assert_eq!(restored.settings.verify_tls, Some(true));
}

#[gpui::test]
fn a_toggle_click_and_ctrl_enter_commit_the_open_timeout(cx: &mut TestAppContext) {
    let dir = settings_workspace();
    let (_state_dir, state_path) = state_file();
    let (shell, cx) = render(cx, &dir, &state_path);
    open_settings(cx);

    type_timeout(cx, "3000", "");
    click(cx, "request-setting-redirects");
    let draft = draft_request(cx, &shell).expect("the draft");
    assert_eq!(draft.settings.timeout_ms, Some(3000), "committed first");
    assert_eq!(draft.settings.follow_redirects, Some(true));

    type_timeout(cx, "4000", "ctrl-enter");
    let (sent, open) = cx.update(|_, cx| {
        let shell = shell.read(cx);
        (shell.sends.contains_key(TUNED), shell.cell_edit.is_some())
    });
    assert!(sent, "Ctrl+Enter sends");
    assert!(!open, "the field closed");
    let draft = draft_request(cx, &shell).expect("the draft");
    assert_eq!(draft.settings.timeout_ms, Some(4000), "what was sent");
}
