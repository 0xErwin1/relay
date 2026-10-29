//! The document island: the tab row and the path bar over the active document, or the reason
//! there is none.
//!
//! The active request shows the action strip at its left; the request itself is the next
//! slice.

use bolt::Island;
use bolt::tokens::border;
use gpui::{Context, IntoElement, ParentElement, Styled};

use super::documents::locate;
use super::{Shell, action_strip, path_bar, region, tabs, text_region};

pub(super) fn render(shell: &Shell, cx: &mut Context<Shell>) -> impl IntoElement {
    let colors = bolt::colors(cx);
    let active = shell
        .documents
        .active()
        .and_then(|id| locate(&shell.roots, id));

    let mut body = region("document-body")
        .flex_1()
        .min_h_0()
        .flex()
        .border_t(border::THIN)
        .border_color(colors.line);
    body = match (shell.workspace.document_message(), &active) {
        (Some(message), _) => body.items_center().justify_center().child(
            text_region("document-message")
                .text_color(colors.muted)
                .child(message),
        ),
        (None, Some(_)) => body
            .child(action_strip::render(cx))
            .child(region("document-content").flex_1().min_w_0()),
        (None, None) => body.child(region("document-content").flex_1()),
    };

    region("document").flex_1().min_w_0().h_full().child(
        Island::new()
            .size_full()
            .child(tabs::render(shell, cx))
            .child(path_bar::render(active.as_ref(), cx))
            .child(body),
    )
}
