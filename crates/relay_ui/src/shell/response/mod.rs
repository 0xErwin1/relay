//! The response under the request: the tab row with the status, time and size, then the
//! empty state before the first send, the progress bar while sending over the previous
//! outcome dimmed, and the outcome: the unresolved line, then the body and headers of a
//! response or the error banner of a failure, beside its Tests and Console panes.

mod body;
mod error;
mod model;
mod results;
mod sub_tabs;
mod tab_row;

pub(crate) use model::{BodyText, BodyView, ResponseTab};

use bolt::controls::Keycap;
use bolt::tokens::{border, space};
use gpui::{AnyElement, App, Context, Div, ParentElement, Stateful, Styled, div, prelude::*};

use super::request::{Completed, SendState};
use super::send::RequestSend;
use super::{Shell, region, text_region};
use crate::tokens;

/// The response of the request `id`, element id `response`, filling the column under the
/// request; `send` is `None` before its first send.
pub(super) fn render(
    id: &str,
    send: Option<&RequestSend>,
    cx: &mut Context<Shell>,
) -> Stateful<Div> {
    let state = send.map(|send| &send.state);
    let content = match (send, state) {
        (Some(send), Some(SendState::Sending { previous, .. })) => {
            let previous = previous
                .as_ref()
                .map(|previous| completed(id, send, previous, cx).opacity(tokens::RESPONSE_DIMMED));
            let mut parts = vec![tab_row::progress(cx).into_any_element()];
            parts.extend(previous.map(IntoElement::into_any_element));
            parts
        }
        (Some(send), Some(SendState::Done(outcome))) => {
            vec![completed(id, send, outcome, cx).into_any_element()]
        }
        _ => vec![empty(cx)],
    };
    let line_2 = bolt::colors(cx).line_2;
    region("response")
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .border_t(border::THIN)
        .border_color(line_2)
        .child(tab_row::render(state, cx))
        .children(content)
}

/// A finished send: the unresolved line, then the body or headers of a response or the
/// error banner of a failure.
fn completed(id: &str, send: &RequestSend, outcome: &Completed, cx: &mut Context<Shell>) -> Div {
    let unresolved = match outcome {
        Completed::Response(received) => &received.unresolved,
        Completed::Failure(failure) => &failure.unresolved,
    };
    let column = div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .children(model::unresolved_text(unresolved).map(|text| unresolved_line(text, cx)));
    match outcome {
        Completed::Response(received) => column
            .child(sub_tabs::render(id, send, received, cx))
            .child(match send.tab {
                ResponseTab::Body => body::render(received, send.view, cx),
                ResponseTab::Headers => sub_tabs::headers(received, cx),
                ResponseTab::Tests => results::tests(received, cx),
                ResponseTab::Console => results::console(received, cx),
            }),
        Completed::Failure(failure) => column.child(error::render(id, failure, cx)),
    }
}

/// `Sent with unresolved variables: <names>`, element id `response-unresolved`.
fn unresolved_line(text: String, cx: &App) -> Stateful<Div> {
    text_region("response-unresolved")
        .aria_label(text.clone())
        .flex_none()
        .pl(space::MD)
        .pr(space::MD)
        .pb(space::SM)
        .text_size(tokens::NOTICE_TEXT)
        .text_color(bolt::colors(cx).warning)
        .child(text)
}

/// The muted state before the first send, element id `response-empty`.
fn empty(cx: &App) -> AnyElement {
    let text = "Send the request to see its response";
    text_region("response-empty")
        .aria_label(text)
        .flex_1()
        .flex()
        .items_center()
        .justify_center()
        .gap(space::SM)
        .text_size(tokens::GRID_TEXT)
        .text_color(bolt::colors(cx).muted)
        .child(text)
        .child(Keycap::new("response-empty-keycap", "Ctrl ↵"))
        .into_any_element()
}
