//! The main window shell: title bar and status bar on the desk, the sidebar and document
//! islands between them.

// Used by the sidebar view in the next change.
#[allow(dead_code)]
mod tree_model;
mod document;
mod sidebar;
mod status_bar;
mod title_bar;

use bolt::tokens::chrome;
use gpui::{
    Context, Div, ElementId, IntoElement, ParentElement, Render, Role, Stateful, Styled, Window,
    div, prelude::*,
};

use crate::workspace::OpenedWorkspace;

/// The root view of the main window.
#[derive(Debug)]
pub(crate) struct Shell {
    workspace: OpenedWorkspace,
}

impl Shell {
    pub(crate) fn new(workspace: OpenedWorkspace) -> Self {
        Self { workspace }
    }
}

impl Render for Shell {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = bolt::colors(cx);

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(colors.desk)
            .child(title_bar::render(&self.workspace, cx))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .gap(chrome::ISLAND_GAP)
                    .pl(chrome::ISLAND_GAP)
                    .pr(chrome::ISLAND_GAP)
                    .child(sidebar::render())
                    .child(document::render(&self.workspace, cx)),
            )
            .child(status_bar::render(&self.workspace, cx))
    }
}

/// A region the UI automation tree and the layout tests find by `id`.
fn region(id: &'static str) -> Stateful<Div> {
    div()
        .id(ElementId::Name(id.into()))
        .debug_selector(|| id.to_owned())
}

/// A [`region`] holding text, which the UI automation tree reports as the node's text.
fn text_region(id: &'static str) -> Stateful<Div> {
    region(id).role(Role::Label)
}

#[cfg(test)]
mod tests;
