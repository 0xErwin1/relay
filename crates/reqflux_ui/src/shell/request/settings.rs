//! The Settings pane: timeout, redirects and TLS verification, or their defaults.

use gpui::{AnyElement, App, div, prelude::*};
use reqflux_core::model::Settings;

use super::code;
use super::pane_model::settings_rows;

pub(super) fn render(settings: &Settings, cx: &App) -> Vec<AnyElement> {
    settings_rows(settings)
        .into_iter()
        .map(|(id, label, text)| {
            code::field(id.to_owned(), label, &text, div().child(text.clone()), cx)
        })
        .collect()
}
