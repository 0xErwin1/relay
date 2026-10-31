//! The response sub-tab row, `Body`, `Headers <n>` and `Cookies <n>` with the content type
//! and the `Pretty` / `Raw` / `Preview` toggle, and the Headers grid.

use bolt::chamfer::Chamfer;
use bolt::tokens::{border, chrome, cut, family, icon, space, text};
use bolt::{Icon, IconName};
use gpui::{
    AnyElement, App, Context, Div, FontWeight, Hsla, ParentElement, Role, Stateful, Styled, div,
    prelude::*,
};

use super::cookies::cookie_count;
use super::model::{BodyView, ResponseTab};
use crate::shell::request::Received;
use crate::shell::rows::{labeled_row, part};
use crate::shell::send::RequestSend;
use crate::shell::{Shell, region};
use crate::tokens;

/// The tab the response view shows: a `Tests` or `Console` tab with nothing to show falls
/// back to `Body`, so a response without script results never leaves the pane without a
/// selected chip.
pub(super) fn effective_tab(tab: ResponseTab, received: &Received) -> ResponseTab {
    match tab {
        ResponseTab::Tests if received.scripts.tests.is_empty() => ResponseTab::Body,
        ResponseTab::Console if received.scripts.console.is_empty() => ResponseTab::Body,
        other => other,
    }
}

/// The body view the response shows: `Preview` of a body with nothing to preview falls
/// back to `Pretty`, so the row keeps a selected chip.
pub(super) fn effective_view(view: BodyView, received: &Received) -> BodyView {
    match view {
        BodyView::Preview if received.preview.is_none() => BodyView::Pretty,
        other => other,
    }
}

/// The row of the response `received` of the request `id`.
pub(super) fn render(
    id: &str,
    send: &RequestSend,
    received: &Received,
    cx: &mut Context<Shell>,
) -> Div {
    let colors = bolt::colors(cx);
    // A Tests or Console tab whose results are gone falls back to Body, so the row keeps a
    // selected chip.
    let tab = effective_tab(send.tab, received);
    let headers = received.headers.len().to_string();
    let cookie_count = cookie_count(&received.headers).to_string();
    let mut tabs = vec![
        (ResponseTab::Body, "Body", IconName::Braces, Vec::new()),
        (
            ResponseTab::Headers,
            "Headers",
            IconName::List,
            vec![(headers, colors.muted)],
        ),
        (
            ResponseTab::Cookies,
            "Cookies",
            IconName::Cookie,
            vec![(cookie_count, colors.muted)],
        ),
    ];
    tabs.extend(test_tabs(received, colors));
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
        .children(
            tabs.into_iter()
                .map(|(chip_tab, label, icon_name, counts)| {
                    let request = id.to_owned();
                    chip(
                        format!("response-tab-{}", label.to_lowercase()),
                        label,
                        icon_name,
                        counts,
                        tab == chip_tab,
                        cx,
                    )
                    .on_click(cx.listener(move |this, _event, _window, cx| {
                        this.select_response_tab(&request, chip_tab, cx);
                    }))
                }),
        )
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
    if tab == ResponseTab::Body {
        let selected = effective_view(send.view, received);
        let mut views = vec![
            (BodyView::Pretty, "Pretty", IconName::Braces),
            (BodyView::Raw, "Raw", IconName::List),
        ];
        if received.preview.is_some() {
            views.push((BodyView::Preview, "Preview", IconName::Eye));
        }
        row = row.children(views.into_iter().map(|(view, label, icon_name)| {
            let request = id.to_owned();
            chip(
                format!("response-view-{}", label.to_lowercase()),
                label,
                icon_name,
                Vec::new(),
                selected == view,
                cx,
            )
            .on_click(cx.listener(move |this, _event, _window, cx| {
                this.select_body_view(&request, view, cx);
            }))
        }));
    }
    row
}

/// The `Tests` and `Console` tabs of a response with something to show (S12): `Tests` with
/// its passed and failed counts, `Console` with its entry count.
fn test_tabs(received: &Received, colors: &bolt::tokens::Colors) -> Vec<Tab> {
    let mut tabs = Vec::new();
    if !received.scripts.tests.is_empty() {
        let passed = received
            .scripts
            .tests
            .iter()
            .filter(|test| test.passed)
            .count();
        let failed = received.scripts.tests.len() - passed;
        let mut counts = vec![(format!("{passed} passed"), colors.muted)];
        if failed > 0 {
            counts.push((format!("{failed} failed"), colors.danger));
        }
        tabs.push((
            ResponseTab::Tests,
            "Tests",
            IconName::CircleCheckBig,
            counts,
        ));
    }
    if !received.scripts.console.is_empty() {
        let counts = vec![(received.scripts.console.len().to_string(), colors.muted)];
        tabs.push((ResponseTab::Console, "Console", IconName::Code, counts));
    }
    tabs
}

/// One entry of the response sub-tab row.
type Tab = (ResponseTab, &'static str, IconName, Vec<(String, Hsla)>);

/// A selectable chip `label` with its icon and optional counts, element id `id`.
fn chip(
    id: String,
    label: &'static str,
    icon_name: IconName,
    counts: Vec<(String, Hsla)>,
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
        .children(counts.into_iter().map(|(count, color)| {
            div()
                .font_family(family::MONO)
                .text_size(tokens::SMALL_MONO_TEXT)
                .text_color(color)
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
