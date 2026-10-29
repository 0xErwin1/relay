//! The open request under the document tabs: the request line, the sub-tab bar and the pane
//! of the selected sub-tab: the Params and Headers grids, empty for the other sub-tabs.

mod grid;
mod grid_model;
mod line;
mod model;
mod spans;
mod sub_tabs;

pub(crate) use model::{RequestState, SubTab};

use gpui::{Context, Div, ParentElement, Stateful, Styled};

use super::{Shell, region};

/// The request `id` with its `state`, filling the document content.
pub(super) fn render(
    id: &str,
    state: &RequestState,
    cx: &mut Context<Shell>,
) -> [Stateful<Div>; 3] {
    [
        line::render(state, cx),
        sub_tabs::render(id, state, cx),
        pane(state, cx),
    ]
}

fn pane(state: &RequestState, cx: &mut Context<Shell>) -> Stateful<Div> {
    let request = &state.inspection.request.request;
    let grid = match state.tab {
        SubTab::Params => Some(("params", "param", &request.params)),
        SubTab::Headers => Some(("headers", "header", &request.headers)),
        SubTab::Body | SubTab::Auth | SubTab::Scripts | SubTab::Settings => None,
    };
    region("request-pane")
        .flex_1()
        .min_h_0()
        .overflow_hidden()
        .children(
            grid.map(|(table, kind, rows)| grid::render(&state.inspection, table, kind, rows, cx)),
        )
}
