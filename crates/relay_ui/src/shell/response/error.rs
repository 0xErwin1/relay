//! The error banner of a failed send, with the Retry button that sends again.

use bolt::chamfer::Chamfer;
use bolt::tokens::{cut, family, icon, space};
use bolt::{Icon, IconName};
use gpui::{Context, Div, FontWeight, ParentElement, Role, Stateful, Styled, div, prelude::*};

use crate::shell::request::{Failure, accent_keycap};
use crate::shell::{Shell, region};
use crate::tokens;

/// The banner of `failure`, element id `response-error`, with `response-retry`, which sends
/// the request `id` again.
pub(super) fn render(id: &str, failure: &Failure, cx: &mut Context<Shell>) -> Div {
    let colors = bolt::colors(cx);
    let banner = region("response-error")
        .role(Role::Alert)
        .aria_label(format!("Request failed. {}", failure.error))
        .h(tokens::RESPONSE_ERROR_BANNER)
        .flex()
        .items_center()
        .gap(space::MD)
        .pl(space::MD)
        .pr(space::SM)
        .border_l(tokens::RESPONSE_ERROR_EDGE)
        .border_color(colors.danger)
        .bg(tokens::wash(colors.danger, tokens::RESPONSE_ERROR_WASH))
        .text_size(tokens::RESPONSE_ERROR_TEXT)
        .child(
            Icon::new(IconName::TriangleAlert)
                .size(icon::MD)
                .color(colors.danger),
        )
        .child(
            div()
                .flex_none()
                .whitespace_nowrap()
                .font_weight(FontWeight::BOLD)
                .text_color(colors.strong)
                .child("Request failed."),
        )
        .child(
            div()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .font_family(family::MONO)
                .text_size(tokens::GRID_TEXT)
                .text_color(colors.foreground)
                .child(failure.error.clone()),
        )
        .child(div().flex_1())
        .child(retry(id, cx));
    div()
        .flex_none()
        .pl(space::MD)
        .pr(space::MD)
        .pb(space::MD)
        .child(banner)
}

/// The primary Retry button, element id `response-retry`.
fn retry(id: &str, cx: &mut Context<Shell>) -> Stateful<Div> {
    let brand = bolt::brand(cx);
    let id = id.to_owned();
    region("response-retry")
        .role(Role::Button)
        .aria_label("Retry")
        .relative()
        .flex_none()
        .h(tokens::URL_FIELD)
        .flex()
        .items_center()
        .gap(space::SM)
        .pl(space::MD)
        .pr(space::MD)
        .text_size(tokens::REQUEST_LINE_TEXT)
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(brand.on_accent)
        .child(Chamfer::new(cut::CONTROL).fill(brand.accent))
        .child(
            Icon::new(IconName::RotateCcw)
                .size(icon::MD)
                .color(brand.on_accent),
        )
        .child("Retry")
        .child(accent_keycap())
        .on_click(cx.listener(move |this, _event, window, cx| this.send(&id, window, cx)))
}
