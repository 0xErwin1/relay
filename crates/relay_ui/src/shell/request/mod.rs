//! The open request under the document tabs: the request line, the sub-tab bar and the pane
//! of the selected sub-tab, which is empty until the panes exist.

mod line;
mod model;
mod spans;
mod sub_tabs;

pub(crate) use model::{RequestState, SubTab};

use gpui::{Context, Div, Stateful, Styled};

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
        region("request-pane").flex_1().min_h_0(),
    ]
}
