//! The main window shell: title bar, activity rail and status bar on the desk, the sidebar
//! and document islands between them.

mod action_strip;
mod diamond;
mod document;
mod documents;
mod environment_menu;
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
    Context, Div, ElementId, Entity, FocusHandle, IntoElement, ParentElement, Render, Role,
    Stateful, Styled, Subscription, Window, div, prelude::*,
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
    /// The slug of the window's active environment; `None` is No environment.
    active_environment: Option<String>,
    /// Whether the title bar environment menu is open; not kept across restarts.
    environment_menu_open: bool,
    /// Focused while the environment menu is open, so Escape reaches it.
    environment_menu_focus: FocusHandle,
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
            active_environment: None,
            environment_menu_open: false,
            environment_menu_focus: cx.focus_handle(),
            _open_requests: open_requests,
        }
    }

    /// Opens the request `id` in a tab, inspecting it when it was not open.
    fn open_document(&mut self, id: &str, cx: &mut Context<Self>) {
        self.documents.open(id);
        if !self.requests.contains_key(id)
            && let OpenedWorkspace::Loaded(engine) = &self.workspace
        {
            match engine.inspect(id, self.active_environment.as_deref()) {
                Ok(inspection) => {
                    self.requests
                        .insert(id.to_owned(), RequestState::new(inspection));
                }
                Err(error) => log::warn!("cannot inspect request {id}: {error}"),
            }
        }
        cx.notify();
    }

    /// Makes `slug` the active environment and inspects every open request again against it,
    /// keeping its sub-tab; a request that can no longer be inspected is dropped.
    pub(super) fn set_environment(&mut self, slug: Option<String>, cx: &mut Context<Self>) {
        self.active_environment = slug;
        if let OpenedWorkspace::Loaded(engine) = &self.workspace {
            let environment = self.active_environment.as_deref();
            self.requests
                .retain(|id, state| match engine.inspect(id, environment) {
                    Ok(inspection) => {
                        state.inspection = inspection;
                        true
                    }
                    Err(error) => {
                        log::warn!("cannot inspect request {id}: {error}");
                        false
                    }
                });
        }
        cx.notify();
    }

    /// Opens the environment menu and focuses it, or closes it when open. Does nothing without
    /// a loaded workspace.
    fn toggle_environment_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !matches!(self.workspace, OpenedWorkspace::Loaded(_)) {
            return;
        }
        self.environment_menu_open = !self.environment_menu_open;
        if self.environment_menu_open {
            self.environment_menu_focus.focus(window, cx);
        }
        cx.notify();
    }

    /// The name of the active environment, `None` for No environment.
    fn environment_name(&self) -> Option<String> {
        let slug = self.active_environment.as_deref()?;
        match &self.workspace {
            OpenedWorkspace::Loaded(engine) => engine
                .workspace()
                .environments
                .iter()
                .find(|node| node.slug == slug)
                .map(|node| node.environment.name.clone()),
            OpenedWorkspace::None | OpenedWorkspace::Failed { .. } => None,
        }
    }

    /// Closes the environment menu without changing the active environment.
    fn close_environment_menu(&mut self, cx: &mut Context<Self>) {
        self.environment_menu_open = false;
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = bolt::colors(cx);

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(colors.desk)
            .child(title_bar::render(self, window, cx))
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
mod environment_tests;
#[cfg(test)]
mod request_tests;
#[cfg(test)]
mod sidebar_tests;
#[cfg(test)]
mod tests;
