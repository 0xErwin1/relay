//! The Params and Headers grids, as DBFlux draws data: a header row, then one row per entry
//! with its 1-based number, the key, the value with its references styled and the
//! description. Disabled rows are muted, rows with an unresolved reference carry a warning
//! wash, and a value that references a secret shows `••••••••` with a SECRET badge.

use bolt::chamfer::Chamfer;
use bolt::tokens::{Colors, border, cut, family, space};
use bolt::{Icon, IconName};
use gpui::{AnyElement, App, Div, FontWeight, Hsla, ParentElement, Styled, div, prelude::*};
use relay_core::model::KeyValue;
use relay_engine::Inspection;

use super::grid_model::{has_unresolved, masked_text, row_id, row_label};
use super::spans;
use crate::shell::rows::{labeled_row, part};
use crate::tokens;

/// The grid of `rows`, a `table` of `kind` (`params` and `param`, or `headers` and `header`).
pub(super) fn render(
    inspection: &Inspection,
    table: &str,
    kind: &'static str,
    rows: &[KeyValue],
    cx: &App,
) -> AnyElement {
    let colors = bolt::colors(cx);
    if rows.is_empty() {
        return part("request-grid-empty".to_owned())
            .pl(space::MD)
            .pt(space::MD)
            .text_size(tokens::GRID_TEXT)
            .text_color(colors.muted)
            .child(format!("No {table}"))
            .into_any_element();
    }
    div()
        .flex()
        .flex_col()
        .child(header(kind, colors))
        .children(
            rows.iter()
                .enumerate()
                .map(|(index, row)| data_row(inspection, table, index + 1, row, cx)),
        )
        .into_any_element()
}

fn header(kind: &'static str, colors: &Colors) -> Div {
    let hint = div()
        .ml(space::HALF)
        .font_weight(FontWeight::NORMAL)
        .text_size(tokens::SMALL_MONO_TEXT)
        .text_color(colors.muted)
        .child(kind);
    div()
        .flex()
        .items_center()
        .h(tokens::GRID_HEADER)
        .bg(colors.background)
        .font_family(family::MONO)
        .text_size(tokens::GRID_HEADER_TEXT)
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(colors.strong)
        .child(cell(tokens::GRID_NUMBER_COLUMN))
        .child(
            cell(tokens::GRID_KEY_COLUMN)
                .flex()
                .child("key")
                .child(hint),
        )
        .child(cell(tokens::GRID_VALUE_COLUMN).child("value"))
        .child(cell_fill().child("description"))
}

fn data_row(
    inspection: &Inspection,
    table: &str,
    n: usize,
    row: &KeyValue,
    cx: &App,
) -> AnyElement {
    let colors = bolt::colors(cx);
    let value_spans = inspection.spans(&row.value);
    let unresolved = has_unresolved(&value_spans) || has_unresolved(&inspection.spans(&row.key));
    let secret = inspection.references_secret(&row.value);
    let off = !row.enabled;
    let tone = |on: Hsla| if off { colors.line_2 } else { on };
    let number_color = if unresolved {
        colors.warning
    } else {
        colors.line_2
    };

    let value = cell(tokens::GRID_VALUE_COLUMN)
        .flex()
        .items_center()
        .gap(space::SM)
        .text_color(tone(colors.foreground));
    let value = if secret {
        value
            .child(masked_text(&value_spans))
            .child(secret_badge(colors))
    } else {
        value.children(spans::render(inspection, &row.value, cx))
    };

    labeled_row(row_id(table, n), row_label(&row.key, &row.value, secret))
        .flex()
        .items_center()
        .h(tokens::GRID_ROW)
        .border_b(border::THIN)
        .border_color(colors.row_divider)
        .when(unresolved, |this| {
            this.bg(tokens::unresolved_row_wash(colors))
        })
        .font_family(family::MONO)
        .text_size(tokens::GRID_TEXT)
        .child(
            div()
                .w(tokens::GRID_NUMBER_COLUMN)
                .flex_none()
                .pr(space::MD)
                .text_right()
                .text_color(number_color)
                .child(n.to_string()),
        )
        .child(
            cell(tokens::GRID_KEY_COLUMN)
                .text_color(tone(colors.strong))
                .child(row.key.clone()),
        )
        .child(value)
        .child(
            cell_fill()
                .font_family(family::INTERFACE)
                .text_color(tone(colors.muted))
                .child(row.description.clone().unwrap_or_default()),
        )
        .into_any_element()
}

fn secret_badge(colors: &Colors) -> Div {
    div()
        .relative()
        .flex()
        .items_center()
        .gap(space::XS)
        .pl(space::HALF)
        .pr(space::HALF)
        .font_family(family::INTERFACE)
        .text_size(tokens::BADGE_TEXT)
        .font_weight(FontWeight::BOLD)
        .text_color(colors.muted)
        .child(Chamfer::new(cut::KEYCAP).fill(colors.raised))
        .child(
            Icon::new(IconName::Lock)
                .size(tokens::BADGE_TEXT)
                .color(colors.muted),
        )
        .child("SECRET")
}

fn cell(width: gpui::Pixels) -> Div {
    div()
        .w(width)
        .flex_none()
        .pl(space::MD)
        .pr(space::MD)
        .overflow_hidden()
        .whitespace_nowrap()
}

fn cell_fill() -> Div {
    div()
        .flex_1()
        .min_w_0()
        .pl(space::MD)
        .pr(space::MD)
        .overflow_hidden()
        .whitespace_nowrap()
}
