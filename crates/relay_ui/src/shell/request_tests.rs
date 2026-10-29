//! The request line (S4), the sub-tab bar (S5), the unresolved notice (S6) and their
//! automation ids (S7).

use gpui::{Entity, Modifiers, TestAppContext, VisualTestContext};
use tempfile::TempDir;

use super::Shell;
use super::request::SubTab;
use crate::workspace::OpenedWorkspace;
use crate::workspace_tests::{demo_workspace, write};

const COLLECTION: &str = "01J00000000000000000000011";
const INVOICE: &str = "01J00000000000000000000012";
const PLAIN: &str = "01J00000000000000000000013";

/// `Demo` with collection `API`, which declares `base`, holding `GET Invoice` at
/// `{{base}}/v1/{{missing}}` with enabled and disabled params and headers, a header that
/// uses the secret `token` and one with the unresolved `tenant`, and `GET Plain`.
fn request_workspace() -> TempDir {
    let dir = demo_workspace();
    let root = dir.path();
    write(
        root,
        "collections/api/collection.toml",
        &format!(
            "id = \"{COLLECTION}\"\nname = \"API\"\n\n[[variables]]\nkey = \"base\"\nvalue = \"https://h\"\n\n\
             [[variables]]\nkey = \"token\"\nsecret = true\n"
        ),
    );
    write(
        root,
        "collections/api/invoice.req.toml",
        &format!(
            "id = \"{INVOICE}\"\nname = \"Invoice\"\nmethod = \"GET\"\nurl = \"{{{{base}}}}/v1/{{{{missing}}}}\"\n\n\
             [[params]]\nkey = \"a\"\nvalue = \"1\"\n\n\
             [[params]]\nkey = \"b\"\nvalue = \"2\"\nenabled = false\n\n\
             [[headers]]\nkey = \"X-One\"\nvalue = \"1\"\n\n\
             [[headers]]\nkey = \"X-Two\"\nvalue = \"2\"\n\n\
             [[headers]]\nkey = \"X-Off\"\nvalue = \"3\"\nenabled = false\n\n\
             [[headers]]\nkey = \"Authorization\"\nvalue = \"Bearer {{{{token}}}}\"\n\n\
             [[headers]]\nkey = \"X-Tenant\"\nvalue = \"{{{{tenant}}}}\"\n"
        ),
    );
    write(
        root,
        "collections/api/plain.req.toml",
        &format!(
            "id = \"{PLAIN}\"\nname = \"Plain\"\nmethod = \"GET\"\nurl = \"{{{{base}}}}/ok\"\n"
        ),
    );
    dir
}

fn render(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext) {
    cx.update(super::bind_keys);
    let dir = request_workspace();
    let workspace = OpenedWorkspace::open(Some(dir.path().to_path_buf()));
    drop(dir);
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(workspace, cx));
    cx.run_until_parked();
    (shell, cx)
}

fn bounds(cx: &mut VisualTestContext, selector: &str) -> Option<gpui::Bounds<gpui::Pixels>> {
    cx.debug_bounds(Box::leak(selector.to_owned().into_boxed_str()))
}

fn present(cx: &mut VisualTestContext, selector: &str) -> gpui::Bounds<gpui::Pixels> {
    match bounds(cx, selector) {
        Some(found) => found,
        None => panic!("{selector} is missing"),
    }
}

fn click(cx: &mut VisualTestContext, selector: &str) {
    let target = present(cx, selector);
    cx.simulate_click(target.center(), Modifiers::none());
    cx.run_until_parked();
}

fn selected_tab(cx: &mut VisualTestContext, shell: &Entity<Shell>, id: &str) -> Option<SubTab> {
    cx.update(|_, cx| shell.read(cx).requests.get(id).map(|state| state.tab))
}

#[gpui::test]
fn the_request_line_sits_in_the_document_content(cx: &mut TestAppContext) {
    let (_shell, cx) = render(cx);
    click(cx, &format!("tree-row-{INVOICE}"));
    let content = present(cx, "document-content");
    for part in [
        "request-method",
        "request-url",
        "request-send",
        "request-send-options",
    ] {
        let found = present(cx, part);
        assert!(content.contains(&found.origin), "{part} in the content");
    }
    assert!(bounds(cx, "request-pane").is_some());
}

#[gpui::test]
fn the_six_sub_tabs_run_left_to_right(cx: &mut TestAppContext) {
    let (_shell, cx) = render(cx);
    click(cx, &format!("tree-row-{INVOICE}"));
    let lefts: Vec<_> = SubTab::ALL
        .iter()
        .map(|tab| present(cx, &tab.element_id()).origin.x)
        .collect();
    assert!(lefts.windows(2).all(|pair| pair.first() < pair.get(1)));
}

#[gpui::test]
fn headers_is_the_default_and_clicking_selects(cx: &mut TestAppContext) {
    let (shell, cx) = render(cx);
    click(cx, &format!("tree-row-{INVOICE}"));
    assert_eq!(selected_tab(cx, &shell, INVOICE), Some(SubTab::Headers));
    let counts = cx.update(|_, cx| {
        shell
            .read(cx)
            .requests
            .get(INVOICE)
            .map(|state| (state.count(SubTab::Params), state.count(SubTab::Headers)))
    });
    assert_eq!(counts, Some((Some(1), Some(4))));

    click(cx, "request-tab-body");
    assert_eq!(selected_tab(cx, &shell, INVOICE), Some(SubTab::Body));
}

#[gpui::test]
fn the_unresolved_notice_follows_the_request(cx: &mut TestAppContext) {
    let (_shell, cx) = render(cx);
    click(cx, &format!("tree-row-{INVOICE}"));
    assert!(bounds(cx, "request-unresolved").is_some());

    click(cx, &format!("tree-row-{PLAIN}"));
    assert!(bounds(cx, "request-unresolved").is_none());
}

#[gpui::test]
fn closing_the_tab_drops_its_state(cx: &mut TestAppContext) {
    let (shell, cx) = render(cx);
    click(cx, &format!("tree-row-{INVOICE}"));
    assert!(selected_tab(cx, &shell, INVOICE).is_some());

    click(cx, &format!("doc-tab-close-{INVOICE}"));
    assert_eq!(selected_tab(cx, &shell, INVOICE), None);
}

fn rows(cx: &mut VisualTestContext, table: &str) -> Vec<gpui::Bounds<gpui::Pixels>> {
    (1..=10)
        .map_while(|n| bounds(cx, &format!("request-{table}-row-{n}")))
        .collect()
}

#[gpui::test]
fn headers_render_every_row_in_order_inside_the_pane(cx: &mut TestAppContext) {
    let (_shell, cx) = render(cx);
    click(cx, &format!("tree-row-{INVOICE}"));
    let pane = present(cx, "request-pane");
    let headers = rows(cx, "headers");
    assert_eq!(
        headers.len(),
        5,
        "enabled, disabled, secret and unresolved rows"
    );
    assert!(headers.iter().all(|row| pane.contains(&row.origin)));
    assert!(headers.windows(2).all(|pair| match pair {
        [above, below] => above.origin.y < below.origin.y,
        _ => false,
    }));
    assert!(rows(cx, "params").is_empty());
}

#[gpui::test]
fn params_replace_the_headers_grid(cx: &mut TestAppContext) {
    let (_shell, cx) = render(cx);
    click(cx, &format!("tree-row-{INVOICE}"));
    click(cx, "request-tab-params");
    assert_eq!(rows(cx, "params").len(), 2);
    assert!(rows(cx, "headers").is_empty());
    assert!(bounds(cx, "request-grid-empty").is_none());
}

#[gpui::test]
fn an_empty_table_shows_the_empty_state(cx: &mut TestAppContext) {
    let (_shell, cx) = render(cx);
    click(cx, &format!("tree-row-{PLAIN}"));
    assert!(rows(cx, "headers").is_empty());
    let pane = present(cx, "request-pane");
    assert!(pane.contains(&present(cx, "request-grid-empty").origin));
}
