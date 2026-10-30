//! The Auth pane: the type, then a label/value row per field. Masked values show only
//! `••••••••` with the SECRET badge, on screen and in the automation label.

use bolt::tokens::{family, space};
use gpui::{AnyElement, App, ParentElement, Styled, div, prelude::*};
use reqflux_core::model::Auth;
use reqflux_engine::Inspection;

use super::grid::secret_badge;
use super::grid_model::MASK;
use super::pane_model::{auth_fields, auth_type};
use super::{code, spans};
use crate::shell::rows::labeled_row;
use crate::tokens;

pub(super) fn render(inspection: &Inspection, cx: &App) -> Vec<AnyElement> {
    let colors = bolt::colors(cx);
    let auth = &inspection.request.request.auth;
    let kind = labeled_row("request-auth-type".to_owned(), auth_type(auth).to_owned())
        .pl(space::MD)
        .pt(space::MD)
        .pb(space::SM)
        .font_family(family::MONO)
        .text_size(tokens::SMALL_MONO_TEXT)
        .text_color(colors.muted)
        .child(auth_type(auth).to_owned())
        .into_any_element();
    let fields = auth_fields(auth, |text| inspection.references_secret(text))
        .into_iter()
        .map(|field| {
            let id = field.element_id();
            if field.masked {
                let value = div()
                    .flex()
                    .items_center()
                    .gap(space::SM)
                    .child(MASK)
                    .child(secret_badge(colors));
                code::field(id, &field.label, MASK, value, cx)
            } else {
                let value = div()
                    .flex()
                    .children(spans::render(inspection, &field.value, cx));
                code::field(id, &field.label, &field.value, value, cx)
            }
        });
    let note = matches!(auth, Auth::Other { .. }).then(|| {
        code::empty(
            "request-auth-unsupported".to_owned(),
            "Not supported yet",
            cx,
        )
    });
    std::iter::once(kind).chain(fields).chain(note).collect()
}
