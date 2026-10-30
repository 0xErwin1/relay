//! The Auth pane type menu under the type row: one row per auth type. Clicking a row
//! switches the draft's auth to it; Escape or a click outside closes the menu.

use bolt::chamfer::Chamfer;
use bolt::tokens::{border, cut, icon, space, text};
use bolt::{Icon, IconName};
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, KeyDownEvent, MouseButton, ParentElement,
    Point, Role, Styled, Window, anchored, deferred, div, prelude::*,
};
use reqflux_core::model::Auth;

use super::pane_model::auth_type;
use crate::shell::auth_edit::{AUTH_KINDS, empty_auth};
use crate::shell::rows::labeled_row;
use crate::shell::{Shell, region, text as text_role};
use crate::tokens;

/// The row that switches the auth of `request_id` to `kind`.
fn option(
    request_id: &str,
    kind: &'static str,
    selected: bool,
    cx: &mut Context<Shell>,
) -> AnyElement {
    let colors = bolt::colors(cx);
    let tint = bolt::brand(cx).accent_tint;
    let name = empty_auth(kind)
        .as_ref()
        .map_or(kind, |auth| auth_type(auth))
        .to_owned();
    let request = request_id.to_owned();
    text_role::role(
        labeled_row(format!("auth-type-{kind}"), name.clone()),
        text::BODY,
        cx,
    )
    .role(Role::MenuItem)
    .aria_selected(selected)
    .flex()
    .items_center()
    .gap(space::MD)
    .h(tokens::AUTH_OPTION)
    .pl(space::MD)
    .pr(space::MD)
    .cursor_pointer()
    .hover(|style| style.bg(colors.hover))
    .when(selected, |row| {
        row.bg(bolt::accent_washes(cx).selected_item)
            .text_color(colors.strong)
    })
    .child(div().flex_1().min_w_0().overflow_hidden().child(name))
    .when(selected, |row| {
        row.child(Icon::new(IconName::Check).size(icon::MD).color(tint))
    })
    .on_click(cx.listener(move |this, _event, window, cx| {
        this.set_auth_type(&request, kind, window, cx);
    }))
    .into_any_element()
}

/// The open menu for the request `request_id` whose auth is `auth`, drawn above the islands
/// and anchored under the type row, over a backdrop the size of the window that closes it on
/// a click outside.
pub(super) fn render(
    request_id: &str,
    auth: &Auth,
    shell: &Shell,
    window: &Window,
    cx: &mut Context<Shell>,
) -> AnyElement {
    let options: Vec<_> = AUTH_KINDS
        .into_iter()
        .map(|kind| option(request_id, kind, auth.kind() == kind, cx))
        .collect();
    let colors = bolt::colors(cx);
    let menu = region("auth-type-menu")
        .role(Role::Menu)
        .track_focus(&shell.auth_menu_focus)
        .occlude()
        .relative()
        .w(tokens::AUTH_MENU_WIDTH)
        .py(space::HALF)
        .flex()
        .flex_col()
        .child(
            Chamfer::new(cut::OVERLAY)
                .fill(colors.raised)
                .edge(colors.line_2, border::THIN),
        )
        .children(options)
        .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
            if event.keystroke.key == "escape" {
                this.close_auth_menu(window, cx);
            }
        }));
    let backdrop = region("auth-type-menu-backdrop")
        .occlude()
        .w(window.viewport_size().width)
        .h(window.viewport_size().height)
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, _event, window, cx| this.close_auth_menu(window, cx)),
        );
    div()
        .absolute()
        .top_full()
        .left_0()
        .child(deferred(anchored().position(Point::default()).child(backdrop)).with_priority(1))
        .child(deferred(anchored().child(menu)).with_priority(2))
        .into_any_element()
}
