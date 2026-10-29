//! The response sub-tab row, `Body` and `Headers <n>` with the content type and the
//! `Pretty` / `Raw` toggle, and the Headers grid.

use bolt::chamfer::Chamfer;
use bolt::tokens::{border, chrome, cut, family, icon, space, text};
use bolt::{Icon, IconName};
use gpui::{
    AnyElement, App, Context, Div, FontWeight, ParentElement, Role, Stateful, Styled, div,
    prelude::*,
};

use super::model::{BodyView, ResponseTab};
use crate::shell::request::Received;
use crate::shell::rows::{labeled_row, part};
use crate::shell::send::RequestSend;
use crate::shell::{Shell, region};
use crate::tokens;

/// The row of the response `received` of the request `id`.
pub(super) fn render(
    id: &str,
    send: &RequestSend,
    received: &Received,
    cx: &mut Context<Shell>,
) -> Div {
    let colors = bolt::colors(cx);
    let headers = received.headers.len().to_string();
    let tabs = [
        (ResponseTab::Body, "Body", IconName::Braces, None),
        (
            ResponseTab::Headers,
            "Headers",
            IconName::List,
            Some(headers),
        ),
    ];
    let mut row = div()
        .flex_none()
        .h(tokens::SUB_TAB_BAR)
        .flex()
        .items_center()
        .gap(space::XS)
        .pl(space::MD)
        .pr(space::MD)
        .border_t(border::THIN)
        .border_color(colors.line)
        .children(tabs.map(|(tab, label, icon_name, count)| {
            let request = id.to_owned();
            chip(
                format!("response-tab-{}", label.to_lowercase()),
                label,
                icon_name,
                count,
                send.tab == tab,
                cx,
            )
            .on_click(cx.listener(move |this, _event, _window, cx| {
                this.select_response_tab(&request, tab, cx);
            }))
        }))
        .children(received.content_type().map(|content_type| {
            div()
                .ml(space::SM)
                .whitespace_nowrap()
                .font_family(family::MONO)
                .text_size(tokens::RESPONSE_META_TEXT)
                .text_color(colors.muted)
                .child(content_type.to_owned())
        }))
        .child(div().flex_1());
    if send.tab == ResponseTab::Body {
        let views = [
            (BodyView::Pretty, "Pretty", IconName::Braces),
            (BodyView::Raw, "Raw", IconName::List),
        ];
        row = row.children(views.map(|(view, label, icon_name)| {
            let request = id.to_owned();
            chip(
                format!("response-view-{}", label.to_lowercase()),
                label,
                icon_name,
                None,
                send.view == view,
                cx,
            )
            .on_click(cx.listener(move |this, _event, _window, cx| {
                this.select_body_view(&request, view, cx);
            }))
        }));
    }
    row
}

/// A selectable chip `label` with its icon and an optional count, element id `id`.
fn chip(
    id: String,
    label: &'static str,
    icon_name: IconName,
    count: Option<String>,
    selected: bool,
    cx: &App,
) -> Stateful<Div> {
    let colors = bolt::colors(cx);
    let (label_color, icon_color) = if selected {
        (colors.strong, bolt::brand(cx).accent_tint)
    } else {
        (colors.muted, colors.muted)
    };
    crate::shell::text::role(part(id), text::BODY, cx)
        .role(Role::Tab)
        .aria_label(label)
        .aria_selected(selected)
        .relative()
        .flex_none()
        .flex()
        .items_center()
        .gap(space::SM)
        .h(chrome::DOC_TAB)
        .pl(space::MD)
        .pr(space::MD)
        .whitespace_nowrap()
        .cursor_pointer()
        .when(selected, |chip| {
            chip.child(Chamfer::new(cut::CONTROL).fill(colors.raised))
        })
        .child(Icon::new(icon_name).size(icon::SM).color(icon_color))
        .child(
            div()
                .text_color(label_color)
                .when(selected, |label| label.font_weight(FontWeight::SEMIBOLD))
                .child(label),
        )
        .children(count.map(|count| {
            div()
                .font_family(family::MONO)
                .text_size(tokens::SMALL_MONO_TEXT)
                .text_color(colors.muted)
                .child(count)
        }))
}

/// The headers of `received`, one row `response-headers-row-<n>` each, name then value.
pub(super) fn headers(received: &Received, cx: &App) -> AnyElement {
    let colors = bolt::colors(cx);
    region("response-headers")
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .border_t(border::THIN)
        .border_color(colors.line)
        .font_family(family::MONO)
        .text_size(tokens::GRID_TEXT)
        .children(
            received
                .headers
                .iter()
                .enumerate()
                .map(|(index, (name, value))| {
                    labeled_row(
                        format!("response-headers-row-{}", index + 1),
                        format!("{name}: {value}"),
                    )
                    .flex()
                    .items_center()
                    .h(tokens::GRID_ROW)
                    .border_b(border::THIN)
                    .border_color(colors.row_divider)
                    .child(
                        div()
                            .w(tokens::GRID_KEY_COLUMN)
                            .flex_none()
                            .pl(space::MD)
                            .pr(space::MD)
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(colors.strong)
                            .child(name.clone()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .pr(space::MD)
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_color(colors.foreground)
                            .child(value.clone()),
                    )
                }),
        )
        .into_any_element()
}
