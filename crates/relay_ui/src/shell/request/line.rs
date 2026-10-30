//! The request line: the read-only method chip, the URL with its references styled, and
//! the Send split button, which sends the request, or the Cancel button while it is in
//! flight. Clicking the URL swaps it for an editable field; the send options do nothing yet.

use bolt::chamfer::Chamfer;
use bolt::controls::Keycap;
use bolt::tokens::{border, chrome, cut, family, icon, space};
use bolt::{Icon, IconName};
use gpui::{
    App, Context, Div, Entity, FontWeight, ParentElement, Role, Stateful, Styled, div, prelude::*,
};
use gpui_component::input::{Enter, Escape, Input, InputState};

use super::model::RequestState;
use super::spans;
use crate::shell::{Shell, region};
use crate::tokens::{self, method_color};

/// The line of the request `id`, element id `request-line`.
pub(super) fn render(
    id: &str,
    state: &RequestState,
    sending: bool,
    shell: &Shell,
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
        .child(url(id, state, shell, cx))
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

/// The URL field, element id `request-url`: the URL as written, styled by its references,
/// which a click swaps for the editable field `request-url-input`. While the field of this
/// request is open, it takes the URL's place and takes Enter and Escape while focused.
fn url(id: &str, state: &RequestState, shell: &Shell, cx: &mut Context<Shell>) -> Stateful<Div> {
    if let Some(edit) = shell.url_edit.as_ref().filter(|edit| edit.request_id == id) {
        return editing_url(edit.input.clone(), cx);
    }
    let colors = bolt::colors(cx);
    let url = shell.displayed_url(id);
    let request = id.to_owned();
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
        .cursor_text()
        .child(
            Chamfer::new(cut::CONTROL)
                .fill(colors.background)
                .edge(colors.line, border::THIN),
        )
        .children(spans::render(&state.inspection, &url, cx))
        .on_click(cx.listener(move |this, _event, window, cx| {
            this.start_url_edit(&request, window, cx);
        }))
}

/// The editable URL field, element id `request-url-input`, over the input `input`.
fn editing_url(input: Entity<InputState>, cx: &mut Context<Shell>) -> Stateful<Div> {
    let colors = bolt::colors(cx);
    region("request-url")
        .role(Role::TextInput)
        .relative()
        .flex_1()
        .min_w_0()
        .h(tokens::URL_FIELD)
        .flex()
        .items_center()
        .pl(space::MD)
        .pr(space::MD)
        .font_family(family::MONO)
        .text_size(tokens::URL_TEXT)
        .text_color(colors.strong)
        .child(
            Chamfer::new(cut::CONTROL)
                .fill(colors.background)
                .edge(colors.line, border::THIN),
        )
        .on_action(cx.listener(|this, action: &Enter, window, cx| {
            // Ctrl+Enter maps to the input's `secondary-enter` while the field has focus:
            // it sends instead of committing.
            if action.secondary {
                this.send_active(window, cx);
            } else {
                this.commit_url_edit(window, cx);
            }
        }))
        .on_action(cx.listener(|this, _: &Escape, window, cx| {
            this.discard_url_edit(window, cx);
        }))
        .child(
            Input::new(&input)
                .id("request-url-input")
                .appearance(false)
                .bordered(false)
                .flex_1()
                .min_w_0()
                .text_size(tokens::URL_TEXT)
                .font_family(family::MONO)
                .text_color(colors.strong),
        )
}

/// The Send split button: `request-send`, which sends the request `id`, then
/// `request-send-options`.
fn send(id: &str, cx: &mut Context<Shell>) -> Div {
    let id = id.to_owned();
    let brand = bolt::brand(cx);
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
                .child(accent_keycap())
                .on_click(cx.listener(move |this, _event, window, cx| this.send(&id, window, cx))),
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

/// The `Ctrl ↵` keycap on an accent button.
pub(crate) fn accent_keycap() -> Div {
    div()
        .relative()
        .pl(tokens::KEYCAP_PADDING_X)
        .pr(tokens::KEYCAP_PADDING_X)
        .py(tokens::KEYCAP_PADDING_Y)
        .font_family(family::MONO)
        .text_size(tokens::SMALL_MONO_TEXT)
        .font_weight(FontWeight::NORMAL)
        .text_color(tokens::keycap_on_accent_text())
        .child(Chamfer::new(cut::KEYCAP).fill(tokens::keycap_on_accent_fill()))
        .child("Ctrl ↵")
}
