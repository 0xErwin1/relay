//! The vertical strip of request actions at the left of the active request: Save, Revert,
//! Undo, Redo, then the split toggle. Every action is disabled until editing exists: the
//! buttons keep their muted icon and a blocker over each one swallows hover and clicks and
//! shows the tooltip.

use bolt::IconName;
use bolt::controls::IconButton;
use bolt::tokens::{action_strip, border};
use gpui::{
    App, Div, ElementId, InteractiveElement, IntoElement, ParentElement, Stateful,
    StatefulInteractiveElement, Styled, div,
};
use gpui_component::tooltip::Tooltip;

use super::region;

/// The actions of the strip: id suffix, icon and label, in order. `None` is the separator.
pub(super) const ACTIONS: [Option<(&str, IconName, &str)>; 6] = [
    Some(("save", IconName::Save, "Save")),
    Some(("revert", IconName::RotateCcw, "Revert")),
    Some(("undo", IconName::Undo2, "Undo")),
    Some(("redo", IconName::Redo2, "Redo")),
    None,
    Some(("split", IconName::Columns2, "Split request and response")),
];

/// The strip, element id `request-actions`; each action is `request-action-<id>`.
pub(super) fn render(cx: &App) -> Stateful<Div> {
    let colors = bolt::colors(cx);
    region("request-actions")
        .flex_none()
        .w(action_strip::WIDTH)
        .h_full()
        .flex()
        .flex_col()
        .items_center()
        .gap(action_strip::GAP)
        .py(action_strip::PADDING_Y)
        .border_r(border::THIN)
        .border_color(colors.line)
        .children(ACTIONS.iter().map(|action| {
            match action {
                Some((id, icon, label)) => disabled_action(id, *icon, label).into_any_element(),
                None => div()
                    .flex_none()
                    .w(action_strip::SEPARATOR)
                    .h(border::THIN)
                    .my(action_strip::SEPARATOR_MARGIN)
                    .bg(colors.line)
                    .into_any_element(),
            }
        }))
}

/// A disabled action: the ghost button under a blocker that takes the pointer.
fn disabled_action(id: &str, icon: IconName, label: &'static str) -> Div {
    div()
        .relative()
        .flex_none()
        .child(IconButton::new(format!("request-action-{id}"), icon, label))
        .child(
            div()
                .id(ElementId::Name(
                    format!("request-action-{id}-disabled").into(),
                ))
                .absolute()
                .inset_0()
                .occlude()
                .tooltip(move |window, cx| Tooltip::new(label).build(window, cx)),
        )
}
