//! The title bar: the workspace on the left, the environment on the right.

use bolt::tokens::chrome;
use gpui::{App, IntoElement, ParentElement, Styled};

use super::{region, text_region};
use crate::workspace::OpenedWorkspace;

/// Shown in place of the environment until environments can be selected.
pub(crate) const ENVIRONMENT_PLACEHOLDER: &str = "No environment";

/// The workspace name, or `app_name` when none is loaded.
pub(crate) fn workspace_label<'a>(workspace: &'a OpenedWorkspace, app_name: &'a str) -> &'a str {
    workspace.name().unwrap_or(app_name)
}

pub(super) fn render(workspace: &OpenedWorkspace, cx: &App) -> impl IntoElement {
    let colors = bolt::colors(cx);
    let label = workspace_label(workspace, bolt::brand(cx).name.as_ref()).to_owned();

    region("title-bar")
        .flex_none()
        .h(chrome::TITLE_BAR)
        .w_full()
        .flex()
        .items_center()
        .justify_between()
        .pl(chrome::ISLAND_GAP)
        .pr(chrome::ISLAND_GAP)
        .child(
            text_region("workspace-name")
                .text_color(colors.foreground)
                .child(label),
        )
        .child(
            text_region("environment-name")
                .text_color(colors.muted)
                .child(ENVIRONMENT_PLACEHOLDER),
        )
}
