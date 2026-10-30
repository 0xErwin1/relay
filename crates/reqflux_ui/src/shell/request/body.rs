//! The Body pane: the mode, then the raw text or GraphQL query read-only with line numbers,
//! the urlencoded or form-data grid, the binary file path or the empty state.

use bolt::tokens::{family, space};
use bolt::{Icon, IconName};
use gpui::{AnyElement, ParentElement, Styled, div, prelude::*};
use reqflux_core::model::{Body, FormPart, KeyValue};
use reqflux_engine::Inspection;

use super::pane_model::body_mode;
use super::{code, grid};
use crate::shell::Shell;
use crate::shell::rows::{labeled_row, part};
use crate::tokens;

pub(super) fn render(
    inspection: &Inspection,
    shell: &Shell,
    cx: &mut gpui::Context<Shell>,
) -> Vec<AnyElement> {
    let body = &inspection.request.request.body;
    let mode = labeled_row("request-body-mode".to_owned(), body_mode(body))
        .pl(space::MD)
        .pt(space::MD)
        .font_family(family::MONO)
        .text_size(tokens::SMALL_MONO_TEXT)
        .text_color(bolt::colors(cx).muted)
        .child(body_mode(body))
        .into_any_element();
    let code_block = |text: &str| {
        code::block(
            inspection,
            "request-body-code".to_owned(),
            "request-body-line",
            text,
            cx,
        )
    };
    let content = match body {
        Body::None => vec![code::empty(
            "request-body-empty".to_owned(),
            "This request has no body",
            cx,
        )],
        Body::Raw { text, .. } => vec![code_block(text)],
        Body::Graphql { query, variables } => vec![
            code_block(query),
            code::label("Variables", cx).into_any_element(),
            code::block(
                inspection,
                "request-body-variables".to_owned(),
                "request-body-variables-line",
                variables,
                cx,
            ),
        ],
        Body::Urlencoded { fields } => {
            vec![grid::render(
                inspection,
                "body",
                "field",
                fields,
                &[],
                None,
                shell,
                cx,
            )]
        }
        Body::FormData { parts } => {
            let (rows, files): (Vec<KeyValue>, Vec<bool>) = parts.iter().map(form_row).unzip();
            vec![grid::render(
                inspection, "body", "part", &rows, &files, None, shell, cx,
            )]
        }
        Body::Binary { path } => vec![
            part("request-body-file".to_owned())
                .flex()
                .items_center()
                .gap(space::SM)
                .pl(space::MD)
                .pt(space::MD)
                .font_family(family::MONO)
                .text_size(tokens::GRID_TEXT)
                .child(
                    Icon::new(IconName::File)
                        .size(tokens::GRID_TEXT)
                        .color(bolt::colors(cx).muted),
                )
                .child(div().child(path.clone()))
                .into_any_element(),
        ],
    };
    std::iter::once(mode).chain(content).collect()
}

/// A form part as a grid row, with whether its value is a file path.
fn form_row(part: &FormPart) -> (KeyValue, bool) {
    match part {
        FormPart::Text {
            key,
            value,
            description,
            enabled,
        } => (
            KeyValue {
                description: description.clone(),
                enabled: *enabled,
                ..KeyValue::new(key, value)
            },
            false,
        ),
        FormPart::File {
            key,
            path,
            description,
            enabled,
        } => (
            KeyValue {
                description: description.clone(),
                enabled: *enabled,
                ..KeyValue::new(key, path)
            },
            true,
        ),
    }
}
