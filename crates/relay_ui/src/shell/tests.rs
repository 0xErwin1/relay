//! The window shell layout (S6) in each workspace state (S7).

use gpui::{Bounds, Pixels, TestAppContext, VisualTestContext, px, size};

use super::Shell;
use crate::workspace::OpenedWorkspace;
use crate::workspace_tests::demo_workspace;

fn bounds(cx: &mut VisualTestContext, selector: &'static str) -> Bounds<Pixels> {
    match cx.debug_bounds(selector) {
        Some(bounds) => bounds,
        None => panic!("{selector} is missing from the element tree"),
    }
}

fn render(cx: &mut TestAppContext, workspace: OpenedWorkspace) -> &mut VisualTestContext {
    let (_shell, cx) = cx.add_window_view(|_, _| Shell::new(workspace));
    cx.run_until_parked();
    cx
}

fn assert_layout(cx: &mut VisualTestContext) {
    let window = cx.update(|window, _| window.viewport_size());
    let title_bar = bounds(cx, "title-bar");
    let status_bar = bounds(cx, "status-bar");
    let sidebar = bounds(cx, "sidebar");
    let document = bounds(cx, "document");
    for selector in ["workspace-name", "environment-name", "status-workspace"] {
        bounds(cx, selector);
    }

    assert_eq!(title_bar.origin, gpui::point(px(0.0), px(0.0)));
    assert_eq!(title_bar.size, size(window.width, px(44.0)));
    assert_eq!(status_bar.size, size(window.width, px(38.0)));
    assert_eq!(status_bar.origin.y, window.height - px(38.0));

    assert_eq!(sidebar.origin, gpui::point(px(8.0), px(44.0)));
    assert_eq!(sidebar.size.width, px(290.0));
    assert_eq!(sidebar.size.height, window.height - px(44.0) - px(38.0));

    assert_eq!(
        document.origin,
        gpui::point(px(8.0 + 290.0 + 8.0), px(44.0))
    );
    assert_eq!(
        document.size.width,
        window.width - px(8.0 + 290.0 + 8.0 + 8.0)
    );
    assert_eq!(document.size.height, sidebar.size.height);
}

#[gpui::test]
fn empty_state_shows_the_open_hint(cx: &mut TestAppContext) {
    let cx = render(cx, OpenedWorkspace::None);
    assert_layout(cx);
    let message = bounds(cx, "document-message");
    assert!(bounds(cx, "document").contains(&message.origin));
}

#[gpui::test]
fn failed_state_shows_the_error(cx: &mut TestAppContext) {
    let workspace = OpenedWorkspace::Failed {
        path: "/nowhere".into(),
        error: "no relay.toml".to_owned(),
    };
    let cx = render(cx, workspace);
    assert_layout(cx);
    bounds(cx, "document-message");
}

#[gpui::test]
fn loaded_state_has_an_empty_document(cx: &mut TestAppContext) {
    let dir = demo_workspace();
    let workspace = OpenedWorkspace::open(Some(dir.path().to_path_buf()));
    let cx = render(cx, workspace);
    assert_layout(cx);
    assert!(cx.debug_bounds("document-message").is_none());
}

#[test]
fn chrome_labels_follow_the_workspace() {
    let dir = demo_workspace();
    let loaded = OpenedWorkspace::open(Some(dir.path().to_path_buf()));
    assert_eq!(super::title_bar::workspace_label(&loaded, "Relay"), "Demo");
    assert_eq!(
        super::title_bar::workspace_label(&OpenedWorkspace::None, "Relay"),
        "Relay"
    );
    assert_eq!(super::title_bar::ENVIRONMENT_PLACEHOLDER, "No environment");
}
