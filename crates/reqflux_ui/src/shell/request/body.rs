//! The Body pane: the mode row, which opens the mode menu, then the raw text, which opens a
//! multi-line editor, the urlencoded or form-data grid, which edits like the Params grid,
//! the read-only GraphQL query or binary file path, or the empty state. Every edit lands in
//! the request's draft.

use bolt::tokens::{family, space};
use bolt::{Icon, IconName};
use gpui::{AnyElement, Context, ParentElement, Styled, Window, div, prelude::*};
use gpui_component::input::{Enter, Escape, Textarea};
use reqflux_core::model::{Body, FormPart, KeyValue};
use reqflux_engine::Inspection;

use super::pane_model::body_mode;
use super::{body_menu, code, grid};
use crate::shell::Shell;
use crate::shell::rows::{labeled_row, part};
use crate::tokens;

/// The open raw text editor of the request `request_id`, `request-body-input`. Ctrl+Enter
/// sends before the editor turns it into a newline; Escape leaves the editor.
fn editor(shell: &Shell, request_id: &str, cx: &mut Context<Shell>) -> Option<AnyElement> {
    let edit = shell
        .body_edit
        .as_ref()
        .filter(|edit| edit.request_id == request_id)?;
    let colors = bolt::colors(cx);
    Some(
        part("request-body-input".to_owned())
            .pl(space::MD)
            .pr(space::MD)
            .pt(space::SM)
            .capture_action(cx.listener(|this, action: &Enter, window, cx| {
                if action.secondary {
                    cx.stop_propagation();
                    this.send_active(window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &Escape, window, cx| {
                this.close_body_edit(window, cx);
            }))
            .child(
                Textarea::new(&edit.input)
                    .appearance(false)
                    .bordered(false)
                    .text_size(tokens::GRID_TEXT)
                    .font_family(family::MONO)
                    .text_color(colors.foreground),
            )
            .into_any_element(),
    )
}

/// The pane of `body` of the request `request_id`, the draft's when the request has one,
/// else the saved file's.
pub(super) fn render(
    request_id: &str,
    inspection: &Inspection,
    body: &Body,
    shell: &Shell,
    window: &Window,
    cx: &mut Context<Shell>,
) -> Vec<AnyElement> {
    let mode = div()
        .relative()
        .child(
            labeled_row("request-body-mode".to_owned(), body_mode(body))
                .pl(space::MD)
                .pt(space::MD)
                .font_family(family::MONO)
                .text_size(tokens::SMALL_MONO_TEXT)
                .text_color(bolt::colors(cx).muted)
                .cursor_pointer()
                .child(body_mode(body))
                .on_click(cx.listener(|this, _event, window, cx| {
                    this.toggle_body_menu(window, cx);
                })),
        )
        .when(shell.body_menu_open, |anchor| {
            anchor.child(body_menu::render(request_id, body, shell, window, cx))
        })
        .into_any_element();
    let code_block = |text: &str, cx: &mut Context<Shell>| {
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
        Body::Raw { text, .. } => match editor(shell, request_id, cx) {
            Some(editor) => vec![editor],
            None => {
                let request = request_id.to_owned();
                let block = code_block(text, cx);
                vec![
                    part("request-body-raw".to_owned())
                        .cursor_text()
                        .on_click(cx.listener(move |this, _event, window, cx| {
                            this.start_body_edit(&request, window, cx);
                        }))
                        .child(block)
                        .into_any_element(),
                ]
            }
        },
        Body::Graphql { query, variables } => vec![
            code_block(query, cx),
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
                Some(request_id),
                shell,
                cx,
            )]
        }
        Body::FormData { parts } => {
            let (rows, files): (Vec<KeyValue>, Vec<bool>) = parts.iter().map(form_row).unzip();
            vec![grid::render(
                inspection,
                "body",
                "part",
                &rows,
                &files,
                Some(request_id),
                shell,
                cx,
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
