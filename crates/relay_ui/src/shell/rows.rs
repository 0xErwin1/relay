//! Sidebar rows: tree rows (chevron or method column, then the name) and environment rows.

use bolt::tokens::{border, chrome, family, space, text};
use bolt::{Chevron, ChevronDirection};
use gpui::{
    App, ClickEvent, Div, ElementId, FontWeight, InteractiveElement, IntoElement, ParentElement,
    Role, Stateful, Styled, Window, div, prelude::*, transparent_black,
};

use super::text as text_role;
use super::tree_model::{NodeKind, VisibleRow};
use crate::tokens::{self, MethodColors};

/// A row element the UI automation tree reports by `id`, with `label` as its text.
fn labeled_row(id: String, label: String) -> Stateful<Div> {
    let selector = id.clone();
    div()
        .id(ElementId::Name(id.into()))
        .debug_selector(move || selector)
        .role(Role::Label)
        .aria_label(label)
}

/// The tree row of `row`, with element id `tree-row-<entity id>`. A click on the chevron
/// of a collection or folder calls `on_toggle` instead of reaching the row.
pub(super) fn tree_row(
    row: VisibleRow<'_>,
    selected: bool,
    on_toggle: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    cx: &App,
) -> Stateful<Div> {
    let colors = bolt::colors(cx);
    let brand = bolt::brand(cx);
    let node = row.node;

    let mut element = text_role::role(
        labeled_row(format!("tree-row-{}", node.id), node.label()),
        text::BODY,
        cx,
    )
    .aria_selected(selected)
    .h(chrome::TREE_ROW)
    .flex_none()
    .flex()
    .items_center()
    .gap(space::HALF)
    .pl(space::SM + chrome::TREE_INDENT * row.depth as f32)
    .pr(space::MD)
    .border_l(border::THICK)
    .border_color(transparent_black())
    .whitespace_nowrap()
    .hover(|style| style.bg(colors.hover));
    if node.is_expandable() {
        element = element.aria_expanded(row.expanded);
    }
    if selected {
        element = element
            .bg(bolt::accent_washes(cx).selected_item)
            .border_color(brand.accent_tint);
    }

    let name_color = match (&node.kind, selected) {
        (_, true) => colors.strong,
        (NodeKind::Folder, false) => colors.muted,
        (NodeKind::Collection | NodeKind::Request { .. }, false) => colors.foreground,
    };
    let leading = match &node.kind {
        NodeKind::Request { method } => {
            let color = MethodColors::dark().of(method);
            div()
                .id(ElementId::Name(format!("tree-method-{}", node.id).into()))
                .w(tokens::TREE_METHOD_COLUMN)
                .flex_none()
                .overflow_hidden()
                .font_family(family::MONO)
                .text_size(tokens::METHOD_LABEL_SIZE)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(if selected { color.full } else { color.dim })
                .child(method.clone())
        }
        NodeKind::Collection | NodeKind::Folder => div()
            .id(ElementId::Name(format!("tree-chevron-{}", node.id).into()))
            .on_click(move |event, window, cx| {
                cx.stop_propagation();
                on_toggle(event, window, cx);
            })
            .flex_none()
            .child(Chevron::new(
                if row.expanded {
                    ChevronDirection::Down
                } else {
                    ChevronDirection::Right
                },
                colors.muted,
            )),
    };
    let weight = match node.kind {
        NodeKind::Collection => FontWeight::SEMIBOLD,
        NodeKind::Folder | NodeKind::Request { .. } => text::BODY.weight,
    };

    element = element.child(leading).child(
        div()
            .flex_1()
            .min_w_0()
            .overflow_hidden()
            .text_ellipsis()
            .font_weight(weight)
            .text_color(name_color)
            .child(node.name.clone()),
    );
    element
}

/// An environment row: the name, and the muted count of its variables on the right.
pub(super) fn environment_row(
    id: String,
    name: &str,
    variables: usize,
    cx: &App,
) -> impl IntoElement {
    let colors = bolt::colors(cx);
    text_role::role(labeled_row(id, name.to_owned()), text::BODY, cx)
        .h(chrome::TREE_ROW)
        .flex_none()
        .flex()
        .items_center()
        .gap(space::SM)
        .pl(space::MD)
        .pr(space::MD)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .child(name.to_owned()),
        )
        .child(
            text_role::role(div(), text::CAPTION, cx)
                .text_color(colors.muted)
                .child(variables.to_string()),
        )
}
