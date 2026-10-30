//! The bar over a response reopened from history: the `From history · <time>` label and the
//! Resend button, disabled with a note when the request no longer exists.

use bolt::chamfer::Chamfer;
use bolt::tokens::{cut, icon, space, text};
use bolt::{Icon, IconName};
use gpui::{
    Context, Div, ElementId, FontWeight, ParentElement, Role, Stateful, Styled, div, prelude::*,
};

use crate::shell::history::Reopened;
use crate::shell::{Shell, region, text as text_role, text_region};
use crate::tokens;

/// The note beside a disabled Resend.
pub(crate) const RESEND_NOTE: &str = "Resend needs the request to exist";

/// The bar of `reopened`, shown in the response of the request `id`.
pub(super) fn render(id: &str, reopened: &Reopened, cx: &mut Context<Shell>) -> Div {
    let label = reopened.label();
    div()
        .flex_none()
        .flex()
        .items_center()
        .gap(space::SM)
        .pl(space::MD)
        .pr(space::MD)
        .pb(space::SM)
        .child(
            text_role::role(text_region("response-history-label"), text::CAPTION, cx)
                .aria_label(label.clone())
                .whitespace_nowrap()
                .child(label),
        )
        .child(div().flex_1())
        .children((!reopened.resendable).then(|| {
            text_role::role(text_region("history-resend-note"), text::CAPTION, cx)
                .aria_label(RESEND_NOTE)
                .whitespace_nowrap()
                .child(RESEND_NOTE)
        }))
        .child(resend(id, reopened.resendable, cx))
}

/// The Resend button, element id `history-resend`; disabled, it sits under a blocker,
/// `history-resend-disabled`, that takes the pointer.
fn resend(id: &str, enabled: bool, cx: &mut Context<Shell>) -> Div {
    let colors = bolt::colors(cx);
    let brand = bolt::brand(cx);
    let (fill, color) = if enabled {
        (brand.accent, brand.on_accent)
    } else {
        (colors.raised, colors.muted)
    };
    let mut button: Stateful<Div> = region("history-resend")
        .role(Role::Button)
        .aria_label("Resend")
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
        .text_color(color)
        .child(Chamfer::new(cut::CONTROL).fill(fill))
        .child(Icon::new(IconName::RotateCcw).size(icon::MD).color(color))
        .child("Resend");
    if enabled {
        let id = id.to_owned();
        button = button.on_click(cx.listener(move |this, _event, window, cx| {
            this.resend(&id, window, cx);
        }));
    }
    div()
        .relative()
        .flex_none()
        .child(button)
        .when(!enabled, |element| {
            element.child(
                div()
                    .id(ElementId::Name("history-resend-disabled".into()))
                    .debug_selector(|| "history-resend-disabled".to_owned())
                    .absolute()
                    .inset_0()
                    .occlude(),
            )
        })
}
