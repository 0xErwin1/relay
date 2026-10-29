//! The status bar: the workspace name on the left. The warnings count joins it later.

use bolt::tokens::chrome;
use gpui::{App, IntoElement, ParentElement, Styled};

use super::title_bar::workspace_label;
use super::{region, text_region};
use crate::workspace::OpenedWorkspace;

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
}
