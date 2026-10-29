//! The sidebar tree and environments (S8), the warnings count and keyboard navigation (S9).

use gpui::{Entity, Focusable, TestAppContext, VisualTestContext, px};
use tempfile::TempDir;

use super::Shell;
use super::sidebar::Sidebar;
use super::status_bar::warnings_label;
use crate::workspace::OpenedWorkspace;
use crate::workspace_tests::{demo_workspace, write};

const API: &str = "01J00000000000000000000001";
const USERS: &str = "01J00000000000000000000002";
const LIST: &str = "01J00000000000000000000003";
const HEALTH: &str = "01J00000000000000000000004";
const AUTH: &str = "01J00000000000000000000005";
const TOKEN: &str = "01J00000000000000000000006";
const DEV: &str = "01J00000000000000000000007";
const PROD: &str = "01J00000000000000000000008";

/// `Demo` with collection `API` (folder `users` holding `GET List users`, then
/// `GET Health`), collection `Auth` (`POST Token`), environments `dev` and `prod`, and a
/// directory without `folder.toml`, which loads with one warning.
fn sidebar_workspace() -> TempDir {
    let dir = demo_workspace();
    let root = dir.path();
    let request = |id: &str, name: &str, method: &str| {
        format!(
            "id = \"{id}\"\nname = \"{name}\"\nmethod = \"{method}\"\nurl = \"https://example.test\"\n"
        )
    };
    write(
        root,
        "collections/api/collection.toml",
        &format!("id = \"{API}\"\nname = \"API\"\norder = [\"users\", \"health\"]\n"),
    );
    write(
        root,
        "collections/api/users/folder.toml",
        &format!("id = \"{USERS}\"\nname = \"users\"\n"),
    );
    write(
        root,
        "collections/api/users/list.req.toml",
        &request(LIST, "List users", "GET"),
    );
    write(
        root,
        "collections/api/health.req.toml",
        &request(HEALTH, "Health", "GET"),
    );
    write(
        root,
        "collections/api/broken/stray.req.toml",
        &request("01J00000000000000000000009", "Stray", "GET"),
    );
    write(
        root,
        "collections/auth/collection.toml",
        &format!("id = \"{AUTH}\"\nname = \"Auth\"\n"),
    );
    write(
        root,
        "collections/auth/token.req.toml",
        &request(TOKEN, "Token", "POST"),
    );
    write(
        root,
        "environments/dev.env.toml",
        &format!("id = \"{DEV}\"\nname = \"dev\"\n"),
    );
    write(
        root,
        "environments/prod.env.toml",
        &format!("id = \"{PROD}\"\nname = \"prod\"\n"),
    );
    dir
}

fn render<'a>(
    cx: &'a mut TestAppContext,
    dir: &TempDir,
) -> (Entity<Sidebar>, &'a mut VisualTestContext) {
    cx.update(super::bind_keys);
    let workspace = OpenedWorkspace::open(Some(dir.path().to_path_buf()));
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(workspace, cx));
    cx.run_until_parked();
    let sidebar = cx.update(|_, cx| shell.read(cx).sidebar.clone());
    (sidebar, cx)
}

fn row(id: &str) -> String {
    format!("tree-row-{id}")
}

fn present(cx: &mut VisualTestContext, selector: &str) -> bool {
    cx.debug_bounds(Box::leak(selector.to_owned().into_boxed_str()))
        .is_some()
}

fn selected(cx: &mut VisualTestContext, sidebar: &Entity<Sidebar>) -> Option<String> {
    cx.update(|_, cx| sidebar.read(cx).selected().map(str::to_owned))
}

#[gpui::test]
fn sidebar_shows_sections_rows_and_environments(cx: &mut TestAppContext) {
    let dir = sidebar_workspace();
    let (_sidebar, cx) = render(cx, &dir);

    for selector in ["collections-label", "environments-label", "status-warnings"] {
        assert!(present(cx, selector), "{selector} is missing");
    }
    for id in [API, USERS, HEALTH, AUTH, TOKEN] {
        assert!(present(cx, &row(id)), "row {id} is missing");
    }
    assert!(!present(cx, &row(LIST)), "folders start collapsed");
    for id in [DEV, PROD] {
        assert!(present(cx, &format!("env-row-{id}")), "environment {id}");
    }

    let order: Vec<_> = [API, USERS, HEALTH, AUTH, TOKEN]
        .iter()
        .filter_map(|id| cx.debug_bounds(Box::leak(row(id).into_boxed_str())))
        .collect();
    assert_eq!(order.len(), 5);
    for pair in order.windows(2) {
        if let [above, below] = pair {
            assert_eq!(below.origin.y, above.origin.y + px(26.0), "store order");
            assert_eq!(above.size.height, px(26.0));
        }
    }
}

#[test]
fn warnings_label_counts_and_is_absent_at_zero() {
    assert_eq!(warnings_label(0), None);
    assert_eq!(warnings_label(1).as_deref(), Some("1 warning"));
    assert_eq!(warnings_label(3).as_deref(), Some("3 warnings"));
    let dir = sidebar_workspace();
    let opened = OpenedWorkspace::open(Some(dir.path().to_path_buf()));
    assert_eq!(opened.warning_count(), 1);
}

#[gpui::test]
fn status_bar_has_no_warnings_without_load_warnings(cx: &mut TestAppContext) {
    let dir = demo_workspace();
    let (_sidebar, cx) = render(cx, &dir);
    assert!(!present(cx, "status-warnings"));
    assert!(present(cx, "collections-label"));
}

#[gpui::test]
fn keys_move_the_selection_when_the_tree_has_focus(cx: &mut TestAppContext) {
    let dir = sidebar_workspace();
    let (sidebar, cx) = render(cx, &dir);
    cx.update(|window, cx| sidebar.read(cx).focus_handle(cx).focus(window, cx));

    cx.simulate_keystrokes("down");
    assert_eq!(selected(cx, &sidebar).as_deref(), Some(API));
    cx.simulate_keystrokes("down right");
    assert_eq!(selected(cx, &sidebar).as_deref(), Some(USERS));
    cx.run_until_parked();
    assert!(present(cx, &row(LIST)), "Right expands the folder");
    cx.simulate_keystrokes("right");
    assert_eq!(selected(cx, &sidebar).as_deref(), Some(LIST));
    cx.simulate_keystrokes("left");
    assert_eq!(selected(cx, &sidebar).as_deref(), Some(USERS));
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert!(!present(cx, &row(LIST)), "Enter collapses the folder");
    cx.simulate_keystrokes("up up");
    assert_eq!(selected(cx, &sidebar).as_deref(), Some(API));
}

#[gpui::test]
fn clicking_a_row_selects_it_and_focuses_the_tree(cx: &mut TestAppContext) {
    let dir = sidebar_workspace();
    let (sidebar, cx) = render(cx, &dir);
    let Some(token) = cx.debug_bounds(Box::leak(row(TOKEN).into_boxed_str())) else {
        panic!("token row is missing");
    };
    cx.simulate_click(token.center(), gpui::Modifiers::none());
    assert_eq!(selected(cx, &sidebar).as_deref(), Some(TOKEN));
    cx.simulate_keystrokes("up");
    assert_eq!(selected(cx, &sidebar).as_deref(), Some(AUTH));

    let Some(api) = cx.debug_bounds(Box::leak(row(API).into_boxed_str())) else {
        panic!("API row is missing");
    };
    cx.simulate_click(api.center(), gpui::Modifiers::none());
    assert_eq!(selected(cx, &sidebar).as_deref(), Some(API));
    assert!(present(cx, &row(USERS)), "a row click does not collapse");
}
