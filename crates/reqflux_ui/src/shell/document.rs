//! The document island: the tab row and the path bar over the active document, or the reason
//! there is none.
//!
//! The active request shows the action strip at its left and the request beside it, with
//! its response under it. A history entry whose request no longer exists shows read-only in
//! its place, `history-readonly`: the recorded name, method and URL over the recorded
//! response.
//!
//! While the Variables panel has a scope selected, the island shows that scope's Variables
//! document instead.

use bolt::Island;
use bolt::tokens::{border, space, text};
use gpui::{Context, IntoElement, ParentElement, Styled, Window, prelude::*};

use super::documents::locate;
use super::history::Orphan;
use super::sidebar_history::Panel;
use super::{
    Shell, action_strip, empty_state, path_bar, region, request, response, tabs, text as text_role,
    text_region, variables_document,
};

pub(super) fn render(shell: &Shell, window: &Window, cx: &mut Context<Shell>) -> impl IntoElement {
    let variables_scope = {
        let sidebar = shell.sidebar.read(cx);
        (sidebar.panel == Panel::Variables)
            .then(|| sidebar.variables.selected())
            .flatten()
    };
    if let (Some(scope), None) = (variables_scope, shell.workspace.document_message()) {
        return region("document").flex_1().min_w_0().h_full().child(
            Island::new()
                .size_full()
                .child(variables_document::render(shell, scope, cx)),
        );
    }
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
        (Some(message), _) => body
            .items_center()
            .justify_center()
            .child(empty_state::render(shell, message, cx)),
        (None, _) if shell.orphan.is_some() => match &shell.orphan {
            Some(orphan) => body.child(read_only(shell, orphan, cx)),
            None => body,
        },
        (None, Some(_)) => {
            let content = region("document-content")
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col();
            let active_id = shell.documents.active();
            let drafted = active_id.is_some_and(|id| shell.drafts.contains_key(id));
            let state = active_id.and_then(|id| shell.requests.get(id).map(|state| (id, state)));
            body.child(action_strip::render(active_id, drafted, cx))
                .child(match state {
                    Some((id, state)) => {
                        let send = shell.sends.get(id);
                        let sending = send.is_some_and(|send| send.state.is_sending());
                        content
                            .children(request::render(id, state, sending, shell, window, cx))
                            .child(response::render(id, send, cx))
                    }
                    None => content,
                })
        }
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

/// The read-only view of `orphan`: its recorded name, method and URL over its response.
fn read_only(shell: &Shell, orphan: &Orphan, cx: &mut Context<Shell>) -> impl IntoElement {
    let line = orphan.request_line();
    let title = format!("{} (deleted)", orphan.name);
    region("history-readonly")
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .child(
            text_role::role(text_region("history-readonly-name"), text::BODY, cx)
                .aria_label(title.clone())
                .flex_none()
                .pt(space::SM)
                .pl(space::MD)
                .pr(space::MD)
                .child(title),
        )
        .child(
            text_role::role(text_region("history-readonly-request"), text::BODY, cx)
                .aria_label(line.clone())
                .flex_none()
                .p(space::MD)
                .whitespace_nowrap()
                .overflow_hidden()
                .child(line),
        )
        .child(response::render(
            &orphan.request_id,
            shell.sends.get(&orphan.request_id),
            cx,
        ))
}
