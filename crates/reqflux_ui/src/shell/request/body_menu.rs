//! The Body pane mode menu under the mode row: one row per editable mode. Clicking a row
//! switches the draft's body to it; Escape or a click outside closes the menu.

use gpui::{AnyElement, Context, Window};
use reqflux_core::model::Body;

use super::option_menu;
use crate::shell::Shell;
use crate::shell::body_edit::{BODY_MODES, body_kind};

/// The open menu for the request `request_id` whose body is `body`.
pub(super) fn render(
    request_id: &str,
    body: &Body,
    shell: &Shell,
    window: &Window,
    cx: &mut Context<Shell>,
) -> AnyElement {
    let current = body_kind(body);
    let options: Vec<_> = BODY_MODES
        .into_iter()
        .map(|(kind, name)| {
            let request = request_id.to_owned();
            option_menu::option(
                format!("body-mode-{kind}"),
                name.to_owned(),
                current == Some(kind),
                move |this, window, cx| this.set_body_mode(&request, kind, window, cx),
                cx,
            )
        })
        .collect();
    option_menu::render(
        ("body-mode-menu", "body-mode-menu-backdrop"),
        &shell.body_menu_focus,
        options,
        Shell::close_body_menu,
        window,
        cx,
    )
}
