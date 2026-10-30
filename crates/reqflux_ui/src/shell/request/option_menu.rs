//! A pane menu under a row: one row per option, drawn above the islands over a backdrop
//! that closes it on a click outside. Escape closes it too. The Auth type and Body mode
//! menus are this menu.

use bolt::chamfer::Chamfer;
use bolt::tokens::{border, cut, icon, space, text};
use bolt::{Icon, IconName};
use gpui::{
    AnyElement, Context, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent, MouseButton,
    ParentElement, Point, Role, Styled, Window, anchored, deferred, div, prelude::*,
};

use crate::shell::rows::labeled_row;
use crate::shell::{Shell, region, text as text_role};
use crate::tokens;

/// The option row `id` named `name`, checked when `selected`; a click runs `pick`.
pub(super) fn option(
    id: String,
    name: String,
    selected: bool,
    pick: impl Fn(&mut Shell, &mut Window, &mut Context<Shell>) + 'static,
    cx: &mut Context<Shell>,
) -> AnyElement {
    let colors = bolt::colors(cx);
    let tint = bolt::brand(cx).accent_tint;
    text_role::role(labeled_row(id, name.clone()), text::BODY, cx)
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
        .on_click(cx.listener(move |this, _event, window, cx| pick(this, window, cx)))
        .into_any_element()
}

/// The open menu `id` of `options`, focused through `focus`, anchored under its row;
/// Escape and a click outside its `backdrop` run `close`.
pub(super) fn render(
    (id, backdrop): (&'static str, &'static str),
    focus: &FocusHandle,
    options: Vec<AnyElement>,
    close: fn(&mut Shell, &mut Window, &mut Context<Shell>),
    window: &Window,
    cx: &mut Context<Shell>,
) -> AnyElement {
    let colors = bolt::colors(cx);
    let menu = region(id)
        .role(Role::Menu)
        .track_focus(focus)
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
        .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
            if event.keystroke.key == "escape" {
                close(this, window, cx);
            }
        }));
    let backdrop = region(backdrop)
        .occlude()
        .w(window.viewport_size().width)
        .h(window.viewport_size().height)
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _event, window, cx| close(this, window, cx)),
        );
    div()
        .absolute()
        .top_full()
        .left_0()
        .child(deferred(anchored().position(Point::default()).child(backdrop)).with_priority(1))
        .child(deferred(anchored().child(menu)).with_priority(2))
        .into_any_element()
}
