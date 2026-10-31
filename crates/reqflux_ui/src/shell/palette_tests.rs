//! The command palette (command-palette S1-S4) and the keymap (S5) in the window.

use gpui::{Entity, Focusable, TestAppContext, VisualTestContext};
use tempfile::TempDir;

use super::Shell;
use super::draft_tests::{bounds, click, present, render, state_file};
use super::send_tests::{self, Server, closed_port, serve, settle};
use crate::workspace_tests::{demo_workspace, write};

const API: &str = "01J00000000000000000000051";
const USERS: &str = "01J00000000000000000000052";
const HEALTH: &str = "01J00000000000000000000053";
const ADMIN: &str = "01J00000000000000000000054";
const DROP: &str = "01J00000000000000000000055";

/// `API` holding `GET List users` at `/users` and `GET Health` at `/health` on `port`, and
/// the folder `admin` holding `DELETE Drop user` at `/users/7`.
fn palette_workspace(port: u16) -> TempDir {
    let dir = demo_workspace();
    let root = dir.path();
    let request = |id: &str, name: &str, method: &str, path: &str| {
        format!(
            "id = \"{id}\"\nname = \"{name}\"\nmethod = \"{method}\"\n\
             url = \"http://127.0.0.1:{port}{path}\"\n"
        )
    };
    write(
        root,
        "collections/api/collection.toml",
        &format!("id = \"{API}\"\nname = \"API\"\norder = [\"users\", \"health\", \"admin\"]\n"),
    );
    write(
        root,
        "collections/api/users.req.toml",
        &request(USERS, "List users", "GET", "/users"),
    );
    write(
        root,
        "collections/api/health.req.toml",
        &request(HEALTH, "Health", "GET", "/health"),
    );
    write(
        root,
        "collections/api/admin/folder.toml",
        &format!("id = \"{ADMIN}\"\nname = \"admin\"\n"),
    );
    write(
        root,
        "collections/api/admin/drop.req.toml",
        &request(DROP, "Drop user", "DELETE", "/users/7"),
    );
    dir
}

/// Gives the focus to the shell, as the window has it before anything else is focused.
fn focus_shell(cx: &mut VisualTestContext, shell: &Entity<Shell>) {
    cx.update(|window, cx| {
        let focus = shell.read(cx).focus.clone();
        focus.focus(window, cx);
    });
    cx.run_until_parked();
}

fn press(cx: &mut VisualTestContext, keys: &str) {
    cx.simulate_keystrokes(keys);
    cx.run_until_parked();
}

fn type_query(cx: &mut VisualTestContext, query: &str) {
    cx.simulate_input(query);
    cx.run_until_parked();
}

fn is_open(cx: &mut VisualTestContext, shell: &Entity<Shell>) -> bool {
    cx.update(|_, cx| shell.read(cx).palette.is_some())
}

fn input_focused(cx: &mut VisualTestContext, shell: &Entity<Shell>) -> bool {
    cx.update(|window, cx| {
        shell
            .read(cx)
            .palette
            .as_ref()
            .is_some_and(|palette| palette.input.focus_handle(cx).is_focused(window))
    })
}

fn shell_focused(cx: &mut VisualTestContext, shell: &Entity<Shell>) -> bool {
    cx.update(|window, cx| shell.read(cx).focus.is_focused(window))
}

fn active(cx: &mut VisualTestContext, shell: &Entity<Shell>) -> Option<String> {
    cx.update(|_, cx| shell.read(cx).documents.active().map(str::to_owned))
}

fn open_ids(cx: &mut VisualTestContext, shell: &Entity<Shell>) -> Vec<String> {
    cx.update(|_, cx| shell.read(cx).documents.open_ids().to_vec())
}

fn showing_history(cx: &mut VisualTestContext, shell: &Entity<Shell>) -> bool {
    cx.update(|_, cx| shell.read(cx).sidebar.read(cx).showing_history())
}

#[gpui::test]
fn ctrl_k_opens_the_palette_and_down_enter_opens_a_request(cx: &mut TestAppContext) {
    let dir = palette_workspace(closed_port());
    let (_state_dir, state_path) = state_file();
    let (shell, cx) = render(cx, &dir, &state_path);
    let automation = gpui_mcp::Automation::isolated();
    cx.update(|window, _| automation.attach(window));
    focus_shell(cx, &shell);

    press(cx, "ctrl-k");
    assert!(is_open(cx, &shell));
    assert!(input_focused(cx, &shell), "the query field has the focus");
    present(cx, "palette");
    present(cx, "palette-input");
    let tree = automation.snapshot();
    assert!(tree.nodes.contains_key("palette"), "{:?}", tree.diagnostics);
    assert!(tree.nodes.contains_key("palette-input"));
    let first = tree
        .nodes
        .get("palette-row-1")
        .and_then(|row| row.label.clone());
    assert_eq!(first.as_deref(), Some("Send request Ctrl+Enter"));

    type_query(cx, "get");
    let tree = automation.snapshot();
    let label = |n: usize| {
        tree.nodes
            .get(&format!("palette-row-{n}"))
            .and_then(|row| row.label.clone())
    };
    assert_eq!(label(1).as_deref(), Some("GET List users API"));
    assert_eq!(label(2).as_deref(), Some("GET Health API"));
    assert_eq!(label(3), None, "only the GET requests match");

    press(cx, "down");
    press(cx, "enter");
    assert!(!is_open(cx, &shell));
    assert_eq!(active(cx, &shell).as_deref(), Some(HEALTH));
    assert!(!showing_history(cx, &shell), "the Collections panel shows");
}

#[gpui::test]
fn ctrl_p_and_the_search_field_open_it_and_a_click_outside_closes_it(cx: &mut TestAppContext) {
    let dir = palette_workspace(closed_port());
    let (_state_dir, state_path) = state_file();
    let (shell, cx) = render(cx, &dir, &state_path);
    focus_shell(cx, &shell);

    press(cx, "ctrl-p");
    assert!(is_open(cx, &shell));
    press(cx, "escape");
    assert!(!is_open(cx, &shell));
    assert!(
        shell_focused(cx, &shell),
        "the focus goes back to the shell"
    );

    click(cx, "command-search");
    assert!(is_open(cx, &shell));
    assert!(input_focused(cx, &shell));
    click(cx, "palette-backdrop");
    assert!(!is_open(cx, &shell));
    assert!(bounds(cx, "palette").is_none());
}

#[gpui::test]
fn a_request_is_found_by_method_and_url_and_shows_its_path(cx: &mut TestAppContext) {
    let dir = palette_workspace(closed_port());
    let (_state_dir, state_path) = state_file();
    let (shell, cx) = render(cx, &dir, &state_path);
    let automation = gpui_mcp::Automation::isolated();
    cx.update(|window, _| automation.attach(window));
    focus_shell(cx, &shell);

    press(cx, "ctrl-k");
    type_query(cx, "delete users/7");
    let tree = automation.snapshot();
    let first = tree
        .nodes
        .get("palette-row-1")
        .and_then(|row| row.label.clone());
    assert_eq!(first.as_deref(), Some("DELETE Drop user API / admin"));

    // A click runs a row.
    click(cx, "palette-row-1");
    assert_eq!(active(cx, &shell).as_deref(), Some(DROP));
    assert!(!is_open(cx, &shell));
}

#[gpui::test]
fn escape_closes_the_palette_and_ctrl_enter_still_sends(cx: &mut TestAppContext) {
    let dir = palette_workspace(serve(Server::Silent));
    let (_state_dir, state_path) = state_file();
    let (shell, cx) = render(cx, &dir, &state_path);
    click(cx, &format!("tree-row-{HEALTH}"));
    press(cx, "ctrl-k");
    assert!(is_open(cx, &shell));
    press(cx, "escape");
    assert!(!is_open(cx, &shell));
    assert!(
        send_tests::response(cx, &shell, HEALTH).is_none(),
        "closing the palette cancels nothing"
    );

    press(cx, "ctrl-enter");
    assert_eq!(
        send_tests::response(cx, &shell, HEALTH).as_deref(),
        Some("Sending")
    );
    press(cx, "escape");
    assert_eq!(
        settle(cx, &shell, HEALTH).as_deref(),
        Some("Failed: request cancelled")
    );
}

#[gpui::test]
fn new_request_from_the_palette_creates_it(cx: &mut TestAppContext) {
    let dir = palette_workspace(closed_port());
    let (_state_dir, state_path) = state_file();
    let (shell, cx) = render(cx, &dir, &state_path);
    focus_shell(cx, &shell);

    press(cx, "ctrl-k");
    type_query(cx, "new request");
    press(cx, "enter");

    assert!(!is_open(cx, &shell));
    assert!(
        dir.path()
            .join("collections/api/new-request.req.toml")
            .exists(),
        "the request lands in the first collection"
    );
    present(cx, "tree-rename-input");
}

#[gpui::test]
fn ctrl_n_creates_a_request_in_the_selected_folder(cx: &mut TestAppContext) {
    let dir = palette_workspace(closed_port());
    let (_state_dir, state_path) = state_file();
    let (_shell, cx) = render(cx, &dir, &state_path);

    click(cx, &format!("tree-row-{ADMIN}"));
    press(cx, "ctrl-n");

    assert!(
        dir.path()
            .join("collections/api/admin/new-request.req.toml")
            .exists()
    );
}

#[gpui::test]
fn tab_keys_close_and_cycle_tabs_and_ctrl_2_shows_history(cx: &mut TestAppContext) {
    let dir = palette_workspace(closed_port());
    let (_state_dir, state_path) = state_file();
    let (shell, cx) = render(cx, &dir, &state_path);
    click(cx, &format!("tree-row-{USERS}"));
    click(cx, &format!("tree-row-{HEALTH}"));
    assert_eq!(active(cx, &shell).as_deref(), Some(HEALTH));

    press(cx, "ctrl-tab");
    assert_eq!(active(cx, &shell).as_deref(), Some(USERS), "wraps around");
    press(cx, "ctrl-shift-tab");
    assert_eq!(active(cx, &shell).as_deref(), Some(HEALTH));

    press(cx, "ctrl-w");
    assert_eq!(open_ids(cx, &shell), vec![USERS.to_owned()]);
    assert_eq!(active(cx, &shell).as_deref(), Some(USERS));

    press(cx, "ctrl-2");
    assert!(showing_history(cx, &shell));
    present(cx, "history-header");
    press(cx, "ctrl-1");
    assert!(!showing_history(cx, &shell));
}

#[gpui::test]
fn a_disabled_command_is_listed_and_does_nothing(cx: &mut TestAppContext) {
    let dir = palette_workspace(closed_port());
    let (_state_dir, state_path) = state_file();
    let (shell, cx) = render(cx, &dir, &state_path);
    focus_shell(cx, &shell);

    // No request is open: Send request is the first row, disabled.
    press(cx, "ctrl-k");
    present(cx, "palette-row-1");
    press(cx, "enter");
    assert!(
        is_open(cx, &shell),
        "a disabled command keeps the palette open"
    );
    click(cx, "palette-row-1");
    assert!(is_open(cx, &shell));
    assert!(cx.update(|_, cx| shell.read(cx).sends.is_empty()));
    assert_eq!(active(cx, &shell), None);
}

/// Ctrl+Enter, Escape and Ctrl+S keep working in a request, and typing in the URL field
/// reaches the field, not the palette.
#[gpui::test]
fn the_request_keys_keep_working_and_the_url_field_keeps_its_keys(cx: &mut TestAppContext) {
    let dir = palette_workspace(serve(Server::Silent));
    let (_state_dir, state_path) = state_file();
    let (shell, cx) = render(cx, &dir, &state_path);
    click(cx, &format!("tree-row-{HEALTH}"));
    press(cx, "ctrl-enter");
    assert_eq!(
        send_tests::response(cx, &shell, HEALTH).as_deref(),
        Some("Sending")
    );
    press(cx, "escape");
    assert_eq!(
        settle(cx, &shell, HEALTH).as_deref(),
        Some("Failed: request cancelled")
    );

    click(cx, "request-url");
    press(cx, "end");
    press(cx, "k p");
    assert!(
        !is_open(cx, &shell),
        "letters typed in the field stay there"
    );
    let url = cx.update(|_, cx| shell.read(cx).displayed_url(HEALTH));
    assert!(url.ends_with("/healthkp"), "{url}");
    press(cx, "enter");

    press(cx, "ctrl-s");
    assert!(
        cx.update(|_, cx| shell.read(cx).drafts.is_empty()),
        "Ctrl+S saves the draft"
    );
}
