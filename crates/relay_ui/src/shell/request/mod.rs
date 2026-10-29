//! The open request under the document tabs: the request line, the sub-tab bar and the pane
//! of the selected sub-tab: the Params and Headers grids or the read-only Body, Auth, Scripts
//! and Settings panes; under them, the outcome of the last send.

mod auth;
mod body;
mod code;
mod grid;
mod grid_model;
mod line;
mod model;
mod pane_model;
mod scripts;
mod send;
mod settings;
mod spans;
mod sub_tabs;

pub(crate) use model::{RequestState, SubTab};
pub(crate) use send::{Completed, Failure, Received, SendState};

use bolt::tokens::{border, space};
use gpui::{Context, Div, ParentElement, Stateful, StatefulInteractiveElement, Styled};

use super::{Shell, region, text_region};

/// The request `id` with its `state` and the state of its last send, filling the document
/// content.
pub(super) fn render(
    id: &str,
    state: &RequestState,
    send: Option<&SendState>,
    cx: &mut Context<Shell>,
) -> [Stateful<Div>; 4] {
    let sending = send.is_some_and(SendState::is_sending);
    [
        line::render(id, state, sending, cx),
        sub_tabs::render(id, state, cx),
        pane(state, cx),
        response(send, cx),
    ]
}

/// The response region, `response`: for now only the state text, `response-status`.
fn response(send: Option<&SendState>, cx: &mut Context<Shell>) -> Stateful<Div> {
    let colors = bolt::colors(cx);
    region("response")
        .flex_none()
        .border_t(border::THIN)
        .border_color(colors.line)
        .pl(space::MD)
        .pr(space::MD)
        .py(space::SM)
        .text_color(colors.strong)
        .children(
            send.and_then(SendState::text)
                .map(|text| text_region("response-status").child(text)),
        )
}

fn pane(state: &RequestState, cx: &mut Context<Shell>) -> Stateful<Div> {
    let inspection = &state.inspection;
    let request = &inspection.request.request;
    let content = match state.tab {
        SubTab::Params => vec![grid::render(
            inspection,
            "params",
            "param",
            &request.params,
            &[],
            cx,
        )],
        SubTab::Headers => vec![grid::render(
            inspection,
            "headers",
            "header",
            &request.headers,
            &[],
            cx,
        )],
        SubTab::Body => body::render(inspection, cx),
        SubTab::Auth => auth::render(inspection, cx),
        SubTab::Scripts => scripts::render(inspection, cx),
        SubTab::Settings => settings::render(&request.settings, cx),
    };
    region("request-pane")
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .pb(space::MD)
        .children(content)
}
