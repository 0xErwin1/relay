//! The Scripts pane: the pre-request and test scripts read-only with line numbers, or an
//! empty state each.

use gpui::{AnyElement, App, IntoElement};
use reqflux_engine::Inspection;

use super::code;

pub(super) fn render(inspection: &Inspection, cx: &App) -> Vec<AnyElement> {
    let scripts = &inspection.request.scripts;
    let sections = [
        (
            "Pre-request",
            "request-script-pre",
            &scripts.pre_request,
            "No pre-request script",
        ),
        (
            "Tests",
            "request-script-test",
            &scripts.test,
            "No test script",
        ),
    ];
    sections
        .into_iter()
        .flat_map(|(title, id, script, none)| {
            let content = match script {
                Some(text) => {
                    code::block(inspection, id.to_owned(), &format!("{id}-line"), text, cx)
                }
                None => code::empty(format!("{id}-empty"), none, cx),
            };
            [code::label(title, cx).into_any_element(), content]
        })
        .collect()
}
