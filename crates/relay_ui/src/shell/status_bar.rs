//! The status bar: chips on the desk. With no request sent it only shows the load warnings
//! count, and only when there are warnings.

use bolt::IconName;
use bolt::controls::StatusChip;
use bolt::tokens::chrome;
use gpui::{App, IntoElement, ParentElement, Styled};

use super::region;
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

    region("status-bar")
        .flex_none()
        .h(chrome::STATUS_BAR)
        .w_full()
        .flex()
        .items_center()
        .gap(chrome::ISLAND_GAP)
        .pl(chrome::ISLAND_GAP)
        .pr(chrome::ISLAND_GAP)
        .children(warnings_label(workspace.warning_count()).map(|warnings| {
            StatusChip::new("status-warnings", warnings)
                .icon(IconName::TriangleAlert)
                .icon_color(colors.warning)
        }))
}
