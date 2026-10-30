//! The Settings pane: timeout, redirects and TLS verification, or their defaults. A click on
//! the timeout opens a field over it; a click on redirects or TLS cycles Default, on, off.
//! Every edit lands in the request's draft.

use bolt::tokens::family;
use gpui::{AnyElement, Context, div, prelude::*};
use gpui_component::input::{Enter, Escape, Input};
use reqflux_core::model::Settings;

use super::code;
use super::pane_model::settings_rows;
use crate::shell::Shell;
use crate::shell::rows::part;
use crate::shell::setting_edit::Toggle;
use crate::tokens;

pub(super) fn render(
    request_id: &str,
    settings: &Settings,
    shell: &Shell,
    cx: &mut Context<Shell>,
) -> Vec<AnyElement> {
    let editing = shell
        .cell_edit
        .as_ref()
        .filter(|_| shell.timeout_edit_of(request_id))
        .map(|edit| edit.input.clone());
    settings_rows(settings)
        .into_iter()
        .map(|(id, label, text)| {
            let request = request_id.to_owned();
            match id {
                "request-setting-timeout" => match &editing {
                    Some(input) => {
                        let colors = bolt::colors(cx);
                        let field = part("request-setting-timeout-input".to_owned())
                            .flex_1()
                            .min_w_0()
                            .on_action(cx.listener(|this, action: &Enter, window, cx| {
                                // Ctrl+Enter maps to the input's `secondary-enter` while the
                                // field has focus: it sends instead of committing.
                                if action.secondary {
                                    this.send_active(window, cx);
                                } else {
                                    this.commit_cell_edit(window, cx);
                                }
                            }))
                            .on_action(cx.listener(|this, _: &Escape, window, cx| {
                                this.discard_cell_edit(window, cx);
                            }))
                            .child(
                                Input::new(input)
                                    .appearance(false)
                                    .bordered(false)
                                    .min_w(tokens::GRID_VALUE_COLUMN)
                                    .text_size(tokens::GRID_TEXT)
                                    .font_family(family::MONO)
                                    .text_color(colors.foreground),
                            );
                        code::field(id.to_owned(), label, &text, field, cx).into_any_element()
                    }
                    None => code::field(id.to_owned(), label, &text, div().child(text.clone()), cx)
                        .cursor_text()
                        .on_click(cx.listener(move |this, _event, window, cx| {
                            this.start_timeout_edit(&request, window, cx);
                        }))
                        .into_any_element(),
                },
                "request-setting-redirects" | "request-setting-tls" => {
                    let toggle = if id == "request-setting-tls" {
                        Toggle::VerifyTls
                    } else {
                        Toggle::FollowRedirects
                    };
                    code::field(id.to_owned(), label, &text, div().child(text.clone()), cx)
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _event, window, cx| {
                            this.cycle_setting(&request, toggle, window, cx);
                        }))
                        .into_any_element()
                }
                // A row this pane does not know how to edit stays read-only rather than
                // silently toggling another setting.
                _ => code::field(id.to_owned(), label, &text, div().child(text.clone()), cx)
                    .into_any_element(),
            }
        })
        .collect()
}
