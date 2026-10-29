//! The document island: the tab row and the path bar, empty until documents open, over the
//! open document or the reason there is none.

use bolt::Island;
use bolt::tokens::{border, chrome};
use gpui::{App, IntoElement, ParentElement, Styled};

use super::{region, text_region};
use crate::tokens;
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
            .child(region("document-tabs").flex_none().h(chrome::DOC_TAB_ROW))
            .child(
                region("document-path")
                    .flex_none()
                    .h(tokens::PATH_BAR)
                    .border_t(border::THIN)
                    .border_color(colors.line),
            )
            .child(
                region("document-body")
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .border_t(border::THIN)
                    .border_color(colors.line)
                    .child(content),
            ),
    )
}
