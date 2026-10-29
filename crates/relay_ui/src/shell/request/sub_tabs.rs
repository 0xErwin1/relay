//! The sub-tab bar: Params, Headers, Body, Auth, Scripts and Settings, with the row counts of
//! Params and Headers, and at the right the problem of the inspection or else the unresolved
//! variables notice.

use bolt::chamfer::Chamfer;
use bolt::tokens::{border, chrome, cut, family, icon, space, text};
use bolt::{Icon, IconName};
use gpui::{
    Context, Div, FontWeight, Hsla, ParentElement, Role, Stateful, Styled, div, prelude::*,
};

use super::model::{RequestState, SubTab, count, unresolved_text};
use crate::shell::rows::part;
use crate::shell::{Shell, region, text as text_role};
use crate::tokens;

/// The bar, element id `request-tabs`, for the request `id`.
pub(super) fn render(
    id: &str,
    state: &RequestState,
    shell: &Shell,
    cx: &mut Context<Shell>,
) -> Stateful<Div> {
    let colors = bolt::colors(cx);
    let notice = match &state.inspection.problem {
        Some(problem) => Some(notice("request-problem", problem.clone(), colors.danger)),
        None => unresolved_text(state.inspection.unresolved.len())
            .map(|label| notice("request-unresolved", label, colors.warning)),
    };
    region("request-tabs")
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
            SubTab::ALL.map(|tab| sub_tab(id, state, shown_rows(id, state, shell, tab), tab, cx)),
        )
        .child(div().flex_1())
        .children(notice)
}

/// The number of enabled rows `tab` shows next to its label: counted on the draft's rows
/// when the request has one, so toggling a row moves the number, and on the file's
/// otherwise.
pub(crate) fn shown_rows(
    id: &str,
    state: &RequestState,
    shell: &Shell,
    tab: SubTab,
) -> Option<usize> {
    let request = shell.displayed_request(id, &state.inspection.request.request);
    count(request, tab)
}

/// A notice at the right of the bar, element id `id`, in `color`.
fn notice(id: &'static str, label: String, color: Hsla) -> Stateful<Div> {
    region(id)
        .role(Role::Label)
        .aria_label(label.clone())
        .flex_none()
        .flex()
        .items_center()
        .gap(space::HALF)
        .whitespace_nowrap()
        .text_size(tokens::NOTICE_TEXT)
        .text_color(color)
        .child(
            Icon::new(IconName::TriangleAlert)
                .size(icon::CHIP)
                .color(color),
        )
        .child(label)
}

fn icon_of(tab: SubTab) -> IconName {
    match tab {
        SubTab::Params => IconName::Hash,
        SubTab::Headers => IconName::List,
        SubTab::Body => IconName::Braces,
        SubTab::Auth => IconName::KeyRound,
        SubTab::Scripts => IconName::Code,
        SubTab::Settings => IconName::SlidersHorizontal,
    }
}

/// The sub-tab `tab`, element id `request-tab-<name>`, selected when it is the state's tab.
fn sub_tab(
    id: &str,
    state: &RequestState,
    rows: Option<usize>,
    tab: SubTab,
    cx: &mut Context<Shell>,
) -> Stateful<Div> {
    let colors = bolt::colors(cx);
    let selected = state.tab == tab;
    let (label_color, weight, icon_color) = if selected {
        (
            colors.strong,
            FontWeight::SEMIBOLD,
            bolt::brand(cx).accent_tint,
        )
    } else {
        (colors.muted, text::BODY.weight, colors.muted)
    };
    let request = id.to_owned();
    text_role::role(part(tab.element_id()), text::BODY, cx)
        .role(Role::Tab)
        .aria_label(tab.label())
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
        .child(Icon::new(icon_of(tab)).size(icon::MD).color(icon_color))
        .child(
            div()
                .text_color(label_color)
                .font_weight(weight)
                .child(tab.label()),
        )
        .children(rows.map(|rows| {
            text_role::role(
                part(format!("request-tab-{}-count", tab.label().to_lowercase())),
                text::LABEL,
                cx,
            )
            .font_family(family::MONO)
            .text_size(tokens::SMALL_MONO_TEXT)
            .text_color(colors.muted)
            .child(rows.to_string())
        }))
        .on_click(cx.listener(move |this, _event, _window, cx| {
            this.select_sub_tab(&request, tab, cx);
        }))
}
