//! The open request under the document tabs: the request line, the sub-tab bar and the pane
//! of the selected sub-tab: the Params and Headers grids or the read-only Body, Auth, Scripts
//! and Settings panes.

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

pub(crate) use line::accent_keycap;
pub(crate) use model::{RequestState, SubTab};
pub(crate) use send::{Completed, Failure, Received, SendState, StatusClass};
pub(crate) use send::{elapsed_text, size_text};

use bolt::tokens::space;
use gpui::{Context, Div, ParentElement, Stateful, StatefulInteractiveElement, Styled};

use super::{Shell, region};

/// The request `id` with its `state`, at its natural height over the response; `sending`
/// while its send is in flight.
pub(super) fn render(
    id: &str,
    state: &RequestState,
    sending: bool,
    cx: &mut Context<Shell>,
) -> [Stateful<Div>; 3] {
    [
        line::render(id, state, sending, cx),
        sub_tabs::render(id, state, cx),
        pane(state, cx),
    ]
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
        .flex_none()
        .overflow_y_scroll()
        .pb(space::MD)
        .children(content)
}
