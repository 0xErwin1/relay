//! The main window shell: title bar, activity rail and status bar on the desk, the sidebar
//! and document islands between them.

mod action_strip;
mod diamond;
mod document;
mod documents;
mod path_bar;
mod rail;
mod request;
mod rows;
mod sidebar;
mod status_bar;
mod tabs;
mod text;
mod title_bar;
mod tree_model;

pub(crate) use sidebar::bind_keys;

use std::collections::HashMap;

use bolt::tokens::chrome;
use gpui::{
    Context, Div, ElementId, Entity, IntoElement, ParentElement, Render, Role, Stateful, Styled,
    Subscription, Window, div, prelude::*,
};

use self::documents::Documents;
use self::request::{RequestState, SubTab};
use self::sidebar::{OpenRequest, Sidebar};
use self::tree_model::Node;

use crate::workspace::OpenedWorkspace;

/// The root view of the main window.
///
/// It owns the open documents: the sidebar reports opened requests through
/// [`OpenRequest`] events, and the document island renders and changes [`Documents`].
pub(crate) struct Shell {
    workspace: OpenedWorkspace,
    sidebar: Entity<Sidebar>,
    /// The collection trees of the loaded workspace, which tabs and the path bar read names
    /// and locations from; empty without one.
    roots: Vec<Node>,
    documents: Documents,
    /// The state of each open request that the engine could inspect, by request id.
    requests: HashMap<String, RequestState>,
    _open_requests: Subscription,
}

impl Shell {
    pub(crate) fn new(workspace: OpenedWorkspace, cx: &mut Context<Self>) -> Self {
        let sidebar = cx.new(|cx| Sidebar::new(&workspace, cx));
        let roots = match &workspace {
            OpenedWorkspace::Loaded(engine) => tree_model::nodes(engine.workspace()),
            OpenedWorkspace::None | OpenedWorkspace::Failed { .. } => Vec::new(),
        };
        let open_requests = cx.subscribe(&sidebar, |this, _, OpenRequest(id), cx| {
            this.open_document(id, cx);
        });
        Self {
            workspace,
            sidebar,
            roots,
            documents: Documents::default(),
            requests: HashMap::new(),
            _open_requests: open_requests,
        }
    }

    /// Opens the request `id` in a tab, inspecting it when it was not open.
    fn open_document(&mut self, id: &str, cx: &mut Context<Self>) {
        self.documents.open(id);
        if !self.requests.contains_key(id)
            && let OpenedWorkspace::Loaded(engine) = &self.workspace
        {
            match engine.inspect(id) {
                Ok(inspection) => {
                    self.requests
                        .insert(id.to_owned(), RequestState::new(inspection));
                }
                Err(error) => log::warn!("cannot inspect request {id}: {error}"),
            }
        }
        cx.notify();
    }

    /// Selects the sub-tab `tab` of the request `id`.
    fn select_sub_tab(&mut self, id: &str, tab: SubTab, cx: &mut Context<Self>) {
        if let Some(state) = self.requests.get_mut(id) {
            state.tab = tab;
            cx.notify();
        }
    }

    /// Activates the tab of the request `id`.
    fn activate_document(&mut self, id: &str, cx: &mut Context<Self>) {
        self.documents.activate(id);
        cx.notify();
    }

    /// Closes the tab of the request `id`.
    fn close_document(&mut self, id: &str, cx: &mut Context<Self>) {
        self.documents.close(id);
        self.requests.remove(id);
        cx.notify();
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
                    .child(document::render(self, cx)),
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
mod document_tests;
#[cfg(test)]
mod request_tests;
#[cfg(test)]
mod sidebar_tests;
#[cfg(test)]
mod tests;
