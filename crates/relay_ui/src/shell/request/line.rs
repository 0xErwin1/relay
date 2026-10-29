//! The request line: the read-only method chip, the URL with its references styled, and the
//! Send split button, which sends the request, or the Cancel button while it is in flight.
//! The send options do nothing yet.

use bolt::chamfer::Chamfer;
use bolt::controls::Keycap;
use bolt::tokens::{border, chrome, cut, family, icon, space};
use bolt::{Icon, IconName};
use gpui::{App, Context, Div, FontWeight, ParentElement, Role, Stateful, Styled, div, prelude::*};

use super::model::RequestState;
use super::spans;
use crate::shell::{Shell, region};
use crate::tokens::{self, method_color};

/// The line of the request `id`, element id `request-line`.
pub(super) fn render(
    id: &str,
    state: &RequestState,
    sending: bool,
    cx: &mut Context<Shell>,
) -> Stateful<Div> {
    let action = if sending {
        cancel(id, cx).into_any_element()
    } else {
        send(id, cx).into_any_element()
    };
    region("request-line")
        .flex_none()
        .h(tokens::REQUEST_LINE)
        .flex()
        .items_center()
        .gap(space::SM)
        .pl(space::MD)
        .pr(space::MD)
        .child(method(state, cx))
        .child(url(state, cx))
        .child(action)
}

/// The Cancel button shown while the request is in flight, element id `request-cancel`.
fn cancel(id: &str, cx: &mut Context<Shell>) -> Stateful<Div> {
    let colors = bolt::colors(cx);
    let brand = bolt::brand(cx);
    let id = id.to_owned();
    region("request-cancel")
        .role(Role::Button)
        .aria_label("Cancel")
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
        .text_color(colors.strong)
        .child(
            Chamfer::new(cut::CONTROL)
                .fill(colors.raised)
                .edge(colors.line, border::THIN),
        )
        .child(
            Icon::new(IconName::Loader)
                .size(icon::MD)
                .color(brand.accent_tint),
        )
        .child("Cancel")
        .child(Keycap::new("request-cancel-keycap", "Esc"))
        .on_click(cx.listener(move |this, _event, _window, cx| this.cancel(&id, cx)))
}

/// The method chip, element id `request-method`.
fn method(state: &RequestState, cx: &App) -> Stateful<Div> {
    let colors = bolt::colors(cx);
    let verb = state.inspection.request.request.method.to_string();
    region("request-method")
        .role(Role::Label)
        .aria_label(verb.clone())
        .relative()
        .flex_none()
        .w(tokens::METHOD_CHIP_WIDTH)
        .h(chrome::CONTROL)
        .flex()
        .items_center()
        .gap(space::SM)
        .pl(space::MD)
        .pr(space::MD)
        .child(
            Chamfer::new(cut::CONTROL)
                .fill(colors.raised)
                .edge(colors.line, border::THIN),
        )
        .child(
            div()
                .flex_1()
                .font_family(family::MONO)
                .text_size(tokens::REQUEST_LINE_TEXT)
                .font_weight(FontWeight::BOLD)
                .text_color(method_color(&verb, colors))
                .child(verb),
        )
        .child(
            Icon::new(IconName::ChevronDown)
                .size(icon::SM)
                .color(colors.muted),
        )
}

/// The URL field, element id `request-url`, labelled with the URL as written.
fn url(state: &RequestState, cx: &App) -> Stateful<Div> {
    let colors = bolt::colors(cx);
    let url = state.inspection.url.clone();
    region("request-url")
        .role(Role::Label)
        .aria_label(url.clone())
        .relative()
        .flex_1()
        .min_w_0()
        .h(tokens::URL_FIELD)
        .flex()
        .items_center()
        .pl(space::MD)
        .pr(space::MD)
        .overflow_hidden()
        .whitespace_nowrap()
        .font_family(family::MONO)
        .text_size(tokens::URL_TEXT)
        .text_color(colors.strong)
        .child(
            Chamfer::new(cut::CONTROL)
                .fill(colors.background)
                .edge(colors.line, border::THIN),
        )
        .children(spans::render(&state.inspection, &url, cx))
}

/// The Send split button: `request-send`, which sends the request `id`, then
/// `request-send-options`.
fn send(id: &str, cx: &mut Context<Shell>) -> Div {
    let id = id.to_owned();
    let brand = bolt::brand(cx);
    let keycap = div()
        .relative()
        .pl(tokens::KEYCAP_PADDING_X)
        .pr(tokens::KEYCAP_PADDING_X)
        .py(tokens::KEYCAP_PADDING_Y)
        .font_family(family::MONO)
        .text_size(tokens::SMALL_MONO_TEXT)
        .font_weight(FontWeight::NORMAL)
        .text_color(tokens::keycap_on_accent_text())
        .child(Chamfer::new(cut::KEYCAP).fill(tokens::keycap_on_accent_fill()))
        .child("Ctrl ↵");
    div()
        .relative()
        .flex_none()
        .h(tokens::URL_FIELD)
        .flex()
        .text_color(brand.on_accent)
        .child(Chamfer::new(cut::CONTROL).fill(brand.accent))
        .child(
            region("request-send")
                .role(Role::Button)
                .aria_label("Send")
                .flex()
                .items_center()
                .gap(space::SM)
                .pl(space::MD)
                .pr(space::MD)
                .text_size(tokens::REQUEST_LINE_TEXT)
                .font_weight(FontWeight::SEMIBOLD)
                .child(
                    Icon::new(IconName::Send)
                        .size(icon::MD)
                        .color(brand.on_accent),
                )
                .child("Send")
                .child(keycap)
                .on_click(cx.listener(move |this, _event, _window, cx| this.send(&id, cx))),
        )
        .child(div().w(border::THIN).bg(tokens::keycap_on_accent_fill()))
        .child(
            region("request-send-options")
                .role(Role::Button)
                .aria_label("Send options")
                .w(tokens::SEND_OPTIONS_WIDTH)
                .flex()
                .items_center()
                .justify_center()
                .child(
                    Icon::new(IconName::ChevronDown)
                        .size(icon::CHIP)
                        .color(brand.on_accent),
                ),
        )
}
