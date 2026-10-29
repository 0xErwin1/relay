//! A text with its `{{name}}` references styled as the engine marks them: resolving ones in
//! the brand tint on its wash, unresolved ones in warning text on a warning wash, underlined.

use bolt::tokens::border;
use gpui::{AnyElement, App, Hsla, IntoElement, ParentElement, Styled, div};
use relay_engine::{Inspection, Span};

use crate::tokens::VARIABLE_WASH;

pub(super) fn render(inspection: &Inspection, text: &str, cx: &App) -> Vec<AnyElement> {
    let colors = bolt::colors(cx);
    let tint = bolt::brand(cx).accent_tint;
    inspection
        .spans(text)
        .into_iter()
        .map(|span| match span {
            Span::Literal(literal) => div().child(literal).into_any_element(),
            Span::Reference {
                raw,
                resolves: true,
                ..
            } => div()
                .text_color(tint)
                .bg(wash(tint))
                .child(raw)
                .into_any_element(),
            Span::Reference { raw, .. } => div()
                .text_color(colors.warning)
                .bg(wash(colors.warning))
                .border_b(border::THIN)
                .border_color(colors.warning)
                .child(raw)
                .into_any_element(),
        })
        .collect()
}

fn wash(color: Hsla) -> Hsla {
    Hsla {
        a: VARIABLE_WASH,
        ..color
    }
}
