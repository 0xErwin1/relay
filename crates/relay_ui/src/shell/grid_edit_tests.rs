//! The enable boxes of the Params and Headers grids (S11): a click toggles the row's
//! `enabled` state in the draft on the spot, marks the tab and persists, the grid shows
//! the draft's rows when a draft exists, and a secret value stays masked.

use std::path::Path;

use gpui::{Entity, TestAppContext, VisualTestContext};
use relay_core::model::Request;
use relay_engine::state::AppState;
use tempfile::TempDir;

use super::Shell;
use super::draft_tests::{bounds, click, marker, render, state_file};
use crate::workspace_tests::{demo_workspace, write};

const COLLECTION: &str = "01J00000000000000000000051";
const GRID: &str = "01J00000000000000000000052";
const WORKSPACE: &str = "01J00000000000000000000900";

/// A workspace whose collection `API` declares `base` and the valueless secret `token`,
/// holding `GET Grid` with two params and two headers, the second of which references the
/// secret.
fn grid_workspace() -> TempDir {
    let dir = demo_workspace();
    let root = dir.path();
    write(
        root,
        "collections/api/collection.toml",
        &format!(
            "id = \"{COLLECTION}\"\nname = \"API\"\n\n\
             [[variables]]\nkey = \"base\"\nvalue = \"https://h\"\n\n\
             [[variables]]\nkey = \"token\"\nsecret = true\n"
        ),
    );
    write(
        root,
        "collections/api/grid.req.toml",
        &format!(
            "id = \"{GRID}\"\nname = \"Grid\"\nmethod = \"GET\"\nurl = \"{{{{base}}}}/grid\"\n\n\
             [[params]]\nkey = \"page\"\nvalue = \"1\"\n\n\
             [[params]]\nkey = \"q\"\nvalue = \"relay\"\n\n\
             [[headers]]\nkey = \"X-One\"\nvalue = \"1\"\n\n\
             [[headers]]\nkey = \"Authorization\"\nvalue = \"Bearer {{{{token}}}}\"\n"
        ),
    );
    dir
}

/// Opens `GET Grid` in a tab, which lands on the Headers sub-tab.
fn open_grid(cx: &mut VisualTestContext) {
    click(cx, &format!("tree-row-{GRID}"));
}

/// The request the draft of `id` holds, if it has one.
fn draft_request(cx: &mut TestAppContext, shell: &Entity<Shell>, id: &str) -> Option<Request> {
    cx.update(|cx| {
        shell
            .read(cx)
            .drafts
            .get(id)
            .map(|draft| draft.request.clone())
    })
}

/// The request the app state file stores as the draft of `id`, if it has one.
fn stored_request(state_path: &Path, id: &str) -> Option<Request> {
    let state = match AppState::open(state_path) {
        Ok(state) => state,
        Err(error) => panic!("open state: {error}"),
    };
    state
        .drafts(WORKSPACE)
        .unwrap_or_default()
        .into_iter()
        .find(|(entity, _)| entity == id)
        .map(
            |(_, stored)| match toml::from_str::<Request>(&stored.payload) {
                Ok(request) => request,
                Err(error) => panic!("the stored draft did not parse: {error}"),
            },
        )
}

#[gpui::test]
fn clicking_a_header_enable_box_marks_the_tab_and_persists_the_draft(cx: &mut TestAppContext) {
    let dir = grid_workspace();
    let (_state_dir, state_path) = state_file();
    let (shell, cx) = render(cx, &dir, &state_path);
    open_grid(cx);
    assert!(!marker(cx, GRID), "no marker without a draft");
    assert!(draft_request(cx, &shell, GRID).is_none(), "no draft yet");

    click(cx, "request-headers-enabled-2");
    assert!(marker(cx, GRID), "the toggle marks the draft");
    let draft = draft_request(cx, &shell, GRID).expect("the toggle created the draft");
    assert!(!draft.headers[1].enabled, "the row is unchecked");
    assert!(draft.headers[0].enabled, "the other row is untouched");

    // A second shell over the same workspace and state file finds the draft again.
    let (shell_2, cx) = render(cx, &dir, &state_path);
    let stored = stored_request(&state_path, GRID).expect("the draft was persisted");
    assert!(!stored.headers[1].enabled, "the store has the row disabled");
    let restored = draft_request(cx, &shell_2, GRID).expect("the draft was restored");
    assert!(!restored.headers[1].enabled);
    assert!(restored.headers[0].enabled);
}

#[gpui::test]
fn clicking_the_box_again_re_enables_the_row(cx: &mut TestAppContext) {
    let dir = grid_workspace();
    let (_state_dir, state_path) = state_file();
    let (shell, cx) = render(cx, &dir, &state_path);
    open_grid(cx);

    click(cx, "request-headers-enabled-2");
    click(cx, "request-headers-enabled-2");
    assert!(marker(cx, GRID), "the draft stays");
    let draft = draft_request(cx, &shell, GRID).expect("the draft stays");
    assert!(draft.headers[1].enabled, "the row is checked again");
    let stored = stored_request(&state_path, GRID).expect("the draft was persisted");
    assert!(stored.headers[1].enabled, "the store agrees");
}

#[gpui::test]
fn a_param_and_a_header_toggle_share_one_draft(cx: &mut TestAppContext) {
    let dir = grid_workspace();
    let (_state_dir, state_path) = state_file();
    let (shell, cx) = render(cx, &dir, &state_path);
    open_grid(cx);

    click(cx, "request-headers-enabled-1");
    click(cx, "request-tab-params");
    click(cx, "request-params-enabled-2");
    let draft = draft_request(cx, &shell, GRID).expect("one draft holds both edits");
    assert!(!draft.headers[0].enabled, "the header edit is kept");
    assert!(!draft.params[1].enabled, "the param edit landed too");
    assert!(draft.params[0].enabled, "the other param is untouched");
}

#[gpui::test]
fn the_first_toggle_creates_the_draft_from_the_file(cx: &mut TestAppContext) {
    let dir = grid_workspace();
    let (_state_dir, state_path) = state_file();
    let (shell, cx) = render(cx, &dir, &state_path);
    open_grid(cx);
    assert!(draft_request(cx, &shell, GRID).is_none());

    click(cx, "request-headers-enabled-2");
    let draft = draft_request(cx, &shell, GRID).expect("the toggle created the draft");
    assert_eq!(draft.url, "{{base}}/grid", "the file's URL");
    assert_eq!(draft.params.len(), 2, "the file's params");
    assert!(
        draft.params[0].enabled && draft.params[1].enabled,
        "the file's params"
    );
    assert_eq!(draft.headers.len(), 2, "the file's headers");
    assert!(draft.headers[0].enabled, "the file's first header");
    assert!(!draft.headers[1].enabled, "plus the change");
    let seeded = cx.update(|_, cx| {
        shell
            .read(cx)
            .drafts
            .get(GRID)
            .map(|draft| draft.base_hash.is_some())
    });
    assert_eq!(
        seeded,
        Some(true),
        "seeded from the file, as the URL field does"
    );
}

#[gpui::test]
fn toggling_a_row_keeps_the_secret_masked(cx: &mut TestAppContext) {
    let dir = grid_workspace();
    let (_state_dir, state_path) = state_file();
    let (shell, cx) = render(cx, &dir, &state_path);
    open_grid(cx);
    assert!(
        bounds(cx, "request-headers-secret-2").is_some(),
        "the secret value is masked before the toggle"
    );

    click(cx, "request-headers-enabled-1");
    assert!(
        bounds(cx, "request-headers-secret-2").is_some(),
        "the secret value is still masked after the toggle"
    );
    let draft = draft_request(cx, &shell, GRID).expect("the draft");
    assert_eq!(
        draft.headers[1].value, "Bearer {{token}}",
        "the stored reference is untouched"
    );
}

#[gpui::test]
fn the_body_grid_has_no_enable_boxes(cx: &mut TestAppContext) {
    let dir = grid_workspace();
    let (_state_dir, state_path) = state_file();
    let (_shell, cx) = render(cx, &dir, &state_path);
    open_grid(cx);
    click(cx, "request-tab-body");
    assert!(draftless_body_has_no_rows(cx), "the body grid has rows");
    assert!(
        bounds(cx, "request-body-enabled-1").is_none(),
        "body rows are not editable in this slice"
    );
}

/// The body of `GET Grid` shows its empty state, so the body grid never renders.
fn draftless_body_has_no_rows(cx: &mut VisualTestContext) -> bool {
    bounds(cx, "request-body-empty").is_some()
}
