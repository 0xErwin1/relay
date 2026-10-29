//! The document island: the open document, or why there is none.

use bolt::Island;
use gpui::{App, IntoElement, ParentElement, Styled};

use super::{region, text_region};
use crate::workspace::OpenedWorkspace;

pub(super) fn render(workspace: &OpenedWorkspace, cx: &App) -> impl IntoElement {
    let colors = bolt::colors(cx);
    let content = match workspace.document_message() {
        Some(message) => text_region("document-message")
            .text_color(colors.muted)
            .child(message),
        None => region("document-content"),
    };

    region("document").flex_1().min_w_0().h_full().child(
        Island::new()
            .size_full()
            .items_center()
            .justify_center()
            .child(content),
    )
}
