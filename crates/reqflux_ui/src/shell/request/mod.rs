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
pub(super) mod sub_tabs;

pub(crate) use line::accent_keycap;
#[cfg(test)]
pub(crate) use model::count;
pub(crate) use model::{RequestState, SubTab};
pub(crate) use send::{Completed, Failure, Received, SendState, StatusClass};
pub(crate) use send::{elapsed_text, size_text};
#[cfg(test)]
pub(crate) use sub_tabs::shown_rows;

use bolt::tokens::space;
use gpui::{Context, Div, ParentElement, Stateful, StatefulInteractiveElement, Styled};

use super::{Shell, region};

/// The request `id` with its `state`, at its natural height over the response; `sending`
/// while its send is in flight.
pub(super) fn render(
    id: &str,
    state: &RequestState,
    sending: bool,
    shell: &Shell,
    cx: &mut Context<Shell>,
) -> [Stateful<Div>; 3] {
    [
        line::render(id, state, sending, shell, cx),
        sub_tabs::render(id, state, shell, cx),
        pane(id, state, shell, cx),
    ]
}

fn pane(id: &str, state: &RequestState, shell: &Shell, cx: &mut Context<Shell>) -> Stateful<Div> {
    let inspection = &state.inspection;
    // The grids show the draft's rows when the request has one, so a toggle is visible
    // immediately and survives a restart.
    let request = shell.displayed_request(id, &inspection.request.request);
    let content = match state.tab {
        SubTab::Params => vec![grid::render(
            inspection,
            "params",
            "param",
            &request.params,
            &[],
            Some(id),
            shell,
            cx,
        )],
        SubTab::Headers => vec![grid::render(
            inspection,
            "headers",
            "header",
            &request.headers,
            &[],
            Some(id),
            shell,
            cx,
        )],
        SubTab::Body => body::render(inspection, shell, cx),
        SubTab::Auth => auth::render(inspection, cx),
        SubTab::Scripts => scripts::render(inspection, cx),
        SubTab::Settings => settings::render(&inspection.request.request.settings, cx),
    };
    region("request-pane")
        .flex_none()
        .overflow_y_scroll()
        .pb(space::MD)
        .children(content)
}
