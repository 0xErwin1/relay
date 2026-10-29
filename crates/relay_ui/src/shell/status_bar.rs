//! The status bar: the workspace name on the left, then the load warnings count.

use bolt::tokens::chrome;
use gpui::{App, IntoElement, ParentElement, Styled};

use super::title_bar::workspace_label;
use super::{region, text_region};
use crate::workspace::OpenedWorkspace;

/// `<n> warnings` for the load warnings, or `None` when there are none.
pub(crate) fn warnings_label(count: usize) -> Option<String> {
    match count {
        0 => None,
        1 => Some("1 warning".to_owned()),
        n => Some(format!("{n} warnings")),
    }
}

pub(super) fn render(workspace: &OpenedWorkspace, cx: &App) -> impl IntoElement {
    let colors = bolt::colors(cx);
    let label = workspace_label(workspace, bolt::brand(cx).name.as_ref()).to_owned();

    region("status-bar")
        .flex_none()
        .h(chrome::STATUS_BAR)
        .w_full()
        .flex()
        .items_center()
        .gap(chrome::ISLAND_GAP)
        .pl(chrome::ISLAND_GAP)
        .pr(chrome::ISLAND_GAP)
        .text_color(colors.muted)
        .child(text_region("status-workspace").child(label))
        .children(
            warnings_label(workspace.warning_count())
                .map(|warnings| text_region("status-warnings").child(warnings)),
        )
}
