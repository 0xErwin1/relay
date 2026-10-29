//! The response tab row: the `Response` tab with the state of the send, the time and size at
//! the right, and the indeterminate progress bar under it while sending.

use bolt::chamfer::Chamfer;
use bolt::tokens::{Colors, cut, family, icon, space, text};
use bolt::{Icon, IconName};
use gpui::{
    Animation, AnimationExt, App, Div, FontWeight, Hsla, ParentElement, Stateful, Styled, div,
    prelude::*, relative,
};

use crate::shell::request::{Completed, SendState, StatusClass, elapsed_text, size_text};
use crate::shell::{region, text_region};
use crate::tokens;

/// The row of `state`, `None` before the first send.
pub(super) fn render(state: Option<&SendState>, cx: &App) -> Div {
    let colors = bolt::colors(cx);
    let tint = bolt::brand(cx).accent_tint;
    let (icon_name, icon_color, status, meta) = match state {
        None | Some(SendState::Idle) => (IconName::Send, tint, None, None),
        Some(SendState::Sending { started, .. }) => (
            IconName::Loader,
            tint,
            Some(("Sending".to_owned(), tint, None)),
            Some(("response-elapsed", elapsed_text(started.elapsed()))),
        ),
        Some(SendState::Done(Completed::Response(received))) => {
            let color = class_color(StatusClass::of(received.status), colors);
            (
                IconName::Send,
                tint,
                Some((
                    received.status_text(),
                    color,
                    Some(tokens::wash(color, tokens::RESPONSE_STATUS_WASH)),
                )),
                Some((
                    "response-meta",
                    format!(
                        "{} · {}",
                        elapsed_text(received.elapsed),
                        size_text(received.size())
                    ),
                )),
            )
        }
        Some(SendState::Done(Completed::Failure(failure))) => (
            IconName::TriangleAlert,
            colors.danger,
            Some(("Failed".to_owned(), colors.danger, None)),
            Some((
                "response-meta",
                format!("{} · {}", elapsed_text(failure.elapsed), size_text(0)),
            )),
        ),
    };
    let tab = crate::shell::text::role(div(), text::BODY, cx)
        .relative()
        .flex()
        .items_center()
        .gap(space::SM)
        .h(bolt::tokens::chrome::DOC_TAB)
        .pl(space::MD)
        .pr(space::MD)
        .whitespace_nowrap()
        .text_color(colors.strong)
        .font_weight(FontWeight::SEMIBOLD)
        .child(Chamfer::new(cut::CONTROL).fill(colors.raised))
        .child(Icon::new(icon_name).size(icon::MD).color(icon_color))
        .child("Response")
        .children(status.map(|(label, color, wash)| status_chip(label, color, wash)));
    div()
        .flex_none()
        .h(tokens::RESPONSE_TAB_ROW)
        .flex()
        .items_center()
        .gap(space::XS)
        .pl(space::MD)
        .pr(space::MD)
        .child(tab)
        .child(div().flex_1())
        .children(meta.map(|(id, label)| meta_label(id, label, colors)))
}

/// The status chip, element id `response-status`, in `color` on `wash` when there is one.
fn status_chip(label: String, color: Hsla, wash: Option<Hsla>) -> Stateful<Div> {
    text_region("response-status")
        .aria_label(label.clone())
        .relative()
        .pl(space::HALF)
        .pr(space::HALF)
        .font_family(family::MONO)
        .text_size(tokens::RESPONSE_STATUS_TEXT)
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(color)
        .children(wash.map(|wash| Chamfer::new(cut::KEYCAP).fill(wash)))
        .child(label)
}

/// A clock and `label` at the right of the row, element id `id`.
fn meta_label(id: &'static str, label: String, colors: &Colors) -> Stateful<Div> {
    text_region(id)
        .aria_label(label.clone())
        .flex_none()
        .flex()
        .items_center()
        .gap(space::HALF)
        .whitespace_nowrap()
        .font_family(family::MONO)
        .text_size(tokens::RESPONSE_META_TEXT)
        .text_color(colors.muted)
        .child(
            Icon::new(IconName::Clock)
                .size(icon::SM)
                .color(colors.muted),
        )
        .child(label)
}

/// The chip color of a status class.
fn class_color(class: StatusClass, colors: &Colors) -> Hsla {
    match class {
        StatusClass::Success => colors.success,
        StatusClass::Info => colors.info,
        StatusClass::Warning => colors.warning,
        StatusClass::Danger => colors.danger,
    }
}

/// The indeterminate progress bar, element id `response-progress`: an accent segment that
/// crosses the background track again and again.
pub(super) fn progress(cx: &App) -> Stateful<Div> {
    let segment = tokens::RESPONSE_PROGRESS_SEGMENT;
    region("response-progress")
        .flex_none()
        .relative()
        .h(tokens::RESPONSE_PROGRESS)
        .mx(space::MD)
        .mb(space::SM)
        .overflow_hidden()
        .bg(bolt::colors(cx).background)
        .child(
            div()
                .absolute()
                .top_0()
                .h_full()
                .w(relative(segment))
                .bg(bolt::brand(cx).accent)
                .with_animation(
                    "response-progress-segment",
                    Animation::new(tokens::RESPONSE_PROGRESS_PERIOD).repeat(),
                    move |bar, delta| bar.left(relative(delta * (1.0 + segment) - segment)),
                ),
        )
}
