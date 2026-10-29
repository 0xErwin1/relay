//! The main window shell: title bar, activity rail and status bar on the desk, the sidebar
//! and document islands between them.

mod diamond;
mod document;
mod rail;
mod rows;
mod sidebar;
mod status_bar;
mod text;
mod title_bar;
mod tree_model;

pub(crate) use sidebar::bind_keys;

use bolt::tokens::chrome;
use gpui::{
    Context, Div, ElementId, Entity, IntoElement, ParentElement, Render, Role, Stateful, Styled,
    Window, div, prelude::*,
};

use self::sidebar::Sidebar;

use crate::workspace::OpenedWorkspace;

/// The root view of the main window.
pub(crate) struct Shell {
    workspace: OpenedWorkspace,
    sidebar: Entity<Sidebar>,
}

impl Shell {
    pub(crate) fn new(workspace: OpenedWorkspace, cx: &mut Context<Self>) -> Self {
        let sidebar = cx.new(|cx| Sidebar::new(&workspace, cx));
        Self { workspace, sidebar }
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
                    .child(rail::render())
                    .child(self.sidebar.clone())
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
mod sidebar_tests;
#[cfg(test)]
mod tests;
