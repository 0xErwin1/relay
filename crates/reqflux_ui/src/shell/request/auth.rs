//! The Auth pane: the type, then a label/value row per field. Masked values show only
//! `••••••••` with the SECRET badge, on screen and in the automation label. The type row
//! opens the type menu; Username, Key and the secrets open a field; Add to toggles header and
//! query. Every edit lands in the request's draft. Other auth types stay read-only.

use bolt::tokens::{family, space};
use gpui::{
    AnyElement, Context, Div, Entity, ParentElement, Stateful, Styled, Window, div, prelude::*,
};
use gpui_component::input::{Enter, Escape, Input, InputState};
use reqflux_core::model::Auth;
use reqflux_engine::Inspection;

use super::grid::secret_badge;
use super::grid_model::MASK;
use super::pane_model::{auth_fields, auth_type};
use super::{auth_menu, code, spans};
use crate::shell::Shell;
use crate::shell::rows::{labeled_row, part};
use crate::tokens;

/// What a click on a field row does.
#[derive(Clone, Copy)]
enum Edit {
    /// Opens the plain field (Username, Key), or the secret field (Password, Token, Value).
    Text { secret: bool },
    /// Toggles Add to between header and query.
    Location,
    /// Nothing: a field of an unsupported type.
    None,
}

fn edit_of(auth: &Auth, label: &str) -> Edit {
    match (auth, label) {
        (Auth::Other { .. }, _) => Edit::None,
        (_, "Username" | "Key") => Edit::Text { secret: false },
        (_, "Password" | "Token" | "Value") => Edit::Text { secret: true },
        (_, "Add to") => Edit::Location,
        _ => Edit::None,
    }
}

/// The open field over a row, `request-auth-input`.
fn input_field(input: &Entity<InputState>, cx: &mut Context<Shell>) -> Stateful<Div> {
    let colors = bolt::colors(cx);
    part("request-auth-input".to_owned())
        .flex_1()
        .min_w_0()
        .on_action(cx.listener(|this, action: &Enter, window, cx| {
            // Ctrl+Enter maps to the input's `secondary-enter` while the field has focus: it
            // sends instead of committing.
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
        )
}

/// The pane of `auth` of the request `request_id`, the draft's when the request has one,
/// else the saved file's.
pub(super) fn render(
    request_id: &str,
    inspection: &Inspection,
    auth: &Auth,
    shell: &Shell,
    window: &Window,
    cx: &mut Context<Shell>,
) -> Vec<AnyElement> {
    let muted = bolt::colors(cx).muted;
    let request = request_id.to_owned();
    let kind = div()
        .relative()
        .child(
            labeled_row("request-auth-type".to_owned(), auth_type(auth).to_owned())
                .pl(space::MD)
                .pt(space::MD)
                .pb(space::SM)
                .font_family(family::MONO)
                .text_size(tokens::SMALL_MONO_TEXT)
                .text_color(muted)
                .cursor_pointer()
                .child(auth_type(auth).to_owned())
                .on_click(cx.listener(|this, _event, window, cx| {
                    this.toggle_auth_menu(window, cx);
                })),
        )
        .when(shell.auth_menu_open, |anchor| {
            anchor.child(auth_menu::render(&request, auth, shell, window, cx))
        })
        .into_any_element();
    let fields: Vec<_> = auth_fields(auth, |text| inspection.references_secret(text))
        .into_iter()
        .map(|field| {
            let id = field.element_id();
            let edit = edit_of(auth, &field.label);
            if let Edit::Text { secret } = edit
                && shell.auth_edit_of(request_id, secret)
                && let Some(input) = shell.cell_edit.as_ref().map(|edit| edit.input.clone())
            {
                let input = input_field(&input, cx);
                return code::field(id, &field.label, "", input, cx).into_any_element();
            }
            let row = if field.masked {
                let value = div()
                    .flex()
                    .items_center()
                    .gap(space::SM)
                    .child(MASK)
                    .child(secret_badge(bolt::colors(cx)));
                code::field(id, &field.label, MASK, value, cx)
            } else {
                let value = div()
                    .flex()
                    .children(spans::render(inspection, &field.value, cx));
                code::field(id, &field.label, &field.value, value, cx)
            };
            let request = request.clone();
            match edit {
                Edit::Text { secret } => row
                    .cursor_text()
                    .on_click(cx.listener(move |this, _event, window, cx| {
                        this.start_auth_edit(&request, secret, window, cx);
                    }))
                    .into_any_element(),
                Edit::Location => row
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _event, window, cx| {
                        this.toggle_api_key_location(&request, window, cx);
                    }))
                    .into_any_element(),
                Edit::None => row.into_any_element(),
            }
        })
        .collect();
    let note = matches!(auth, Auth::Other { .. }).then(|| {
        code::empty(
            "request-auth-unsupported".to_owned(),
            "Not supported yet",
            cx,
        )
    });
    std::iter::once(kind).chain(fields).chain(note).collect()
}
