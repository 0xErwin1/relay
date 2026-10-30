//! The main window shell: title bar, activity rail and status bar on the desk, the sidebar
//! and document islands between them.

mod action_strip;
mod auth_edit;
mod body_edit;
mod cell_edit;
mod diamond;
mod document;
mod documents;
mod drafts;
mod environment_menu;
mod last_request;
mod method_edit;
mod path_bar;
mod rail;
mod request;
mod response;
mod row_edit;
mod rows;
mod send;
mod setting_edit;
mod sidebar;
mod sidebar_edit;
mod status_bar;
mod tabs;
mod text;
mod title_bar;
mod tree_model;
mod tree_ops;
mod url_edit;

use std::collections::HashMap;

use bolt::tokens::chrome;
use gpui::{
    App, Context, Div, ElementId, Entity, FocusHandle, IntoElement, ParentElement, Render, Role,
    Stateful, Styled, Subscription, Window, div, prelude::*,
};
use reqflux_engine::state::AppState;

use self::body_edit::BodyEdit;
use self::cell_edit::CellEdit;
use self::documents::Documents;
use self::drafts::Drafts;
use self::last_request::LastSend;
use self::request::{RequestState, SubTab};
use self::send::{CancelRequest, RequestSend, SHELL_CONTEXT, SaveRequest, SendRequest};
use self::sidebar::{OpenRequest, Sidebar};
use self::sidebar_edit::TreeCommand;
use self::tree_model::Node;
use self::url_edit::UrlEdit;

use crate::workspace::OpenedWorkspace;

/// Binds the keys of the main window.
pub(crate) fn bind_keys(cx: &mut App) {
    sidebar::bind_keys(cx);
    send::bind_keys(cx);
}

/// The root view of the main window.
///
/// It owns the open documents: the sidebar reports opened requests through
/// [`OpenRequest`] events and tree edits through [`TreeCommand`] events, and the document island renders and changes [`Documents`].
pub(crate) struct Shell {
    workspace: OpenedWorkspace,
    sidebar: Entity<Sidebar>,
    /// The collection trees of the loaded workspace, which tabs and the path bar read names
    /// and locations from; empty without one.
    roots: Vec<Node>,
    documents: Documents,
    /// The state of each open request that the engine could inspect, by request id.
    requests: HashMap<String, RequestState>,
    /// The send of each open request that was sent, by request id; dropped with its tab.
    sends: HashMap<String, RequestSend>,
    /// The request the window sent last, for the status bar.
    last_send: Option<LastSend>,
    /// The draft of each edited request, by request id; restored from the app state and
    /// persisted on every edit.
    drafts: Drafts,
    /// Why the last save of a request's draft failed, by request id, when it failed for a
    /// reason other than a conflict: the tab row shows it as a danger label.
    save_errors: HashMap<String, String>,
    /// The URL field of the request under edit, while it has focus.
    url_edit: Option<UrlEdit>,
    /// The cell field of the Params or Headers grid, the Settings timeout field or an Auth
    /// field, under edit, while it has focus.
    cell_edit: Option<CellEdit>,
    /// The raw body editor of the request under edit.
    body_edit: Option<BodyEdit>,
    /// The per-user app state the drafts persist in; `None` when it could not be opened,
    /// which keeps drafts in memory only.
    state: Option<AppState>,
    /// The id of the loaded workspace, the key drafts are stored under.
    workspace_id: Option<String>,
    /// The slug of the window's active environment; `None` is No environment.
    active_environment: Option<String>,
    /// Whether the title bar environment menu is open; not kept across restarts.
    environment_menu_open: bool,
    /// Focused while the environment menu is open, so Escape reaches it.
    environment_menu_focus: FocusHandle,
    /// Whether the Auth pane type menu of the active request is open.
    auth_menu_open: bool,
    /// Focused while the auth type menu is open, so Escape reaches it.
    auth_menu_focus: FocusHandle,
    /// Whether the Body pane mode menu of the active request is open.
    body_menu_open: bool,
    /// Focused while the body mode menu is open, so Escape reaches it.
    body_menu_focus: FocusHandle,
    /// Whether the method menu of the active request is open.
    method_menu_open: bool,
    /// Focused while the method menu is open, so Escape reaches it.
    method_menu_focus: FocusHandle,
    /// The shell's own focus, given back when the environment menu closes, so the window's
    /// keys (Ctrl+Enter, Escape) keep reaching the shell's key context.
    focus: FocusHandle,
    _open_requests: Subscription,
    _tree_commands: Subscription,
}

impl Shell {
    /// A shell without the app state, for tests: drafts work but stay in memory. The app
    /// opens the state once and builds the shell through [`Shell::with_state`].
    #[cfg(test)]
    pub(crate) fn new(workspace: OpenedWorkspace, cx: &mut Context<Self>) -> Self {
        Self::with_state(workspace, None, cx)
    }

    /// Builds the shell over `state`, the per-user app state the drafts persist in. The
    /// app opens it once and passes it in; tests pass their own file so no test touches
    /// the user's state.
    pub(crate) fn with_state(
        workspace: OpenedWorkspace,
        state: Option<AppState>,
        cx: &mut Context<Self>,
    ) -> Self {
        let sidebar = cx.new(|cx| Sidebar::new(&workspace, cx));
        let roots = match &workspace {
            OpenedWorkspace::Loaded(engine) => tree_model::nodes(engine.workspace()),
            OpenedWorkspace::None | OpenedWorkspace::Failed { .. } => Vec::new(),
        };
        let workspace_id = match &workspace {
            OpenedWorkspace::Loaded(engine) => Some(engine.workspace().manifest.id.to_string()),
            OpenedWorkspace::None | OpenedWorkspace::Failed { .. } => None,
        };
        let open_requests = cx.subscribe(&sidebar, |this, _, OpenRequest(id), cx| {
            this.open_document(id, cx);
        });
        let tree_commands = cx.subscribe(&sidebar, |this, _, command: &TreeCommand, cx| {
            this.run_tree_command(command, cx);
        });
        let mut shell = Self {
            workspace,
            sidebar,
            roots,
            documents: Documents::default(),
            requests: HashMap::new(),
            sends: HashMap::new(),
            last_send: None,
            drafts: HashMap::new(),
            save_errors: HashMap::new(),
            url_edit: None,
            cell_edit: None,
            body_edit: None,
            state,
            workspace_id,
            active_environment: None,
            environment_menu_open: false,
            environment_menu_focus: cx.focus_handle(),
            auth_menu_open: false,
            auth_menu_focus: cx.focus_handle(),
            body_menu_open: false,
            body_menu_focus: cx.focus_handle(),
            method_menu_open: false,
            method_menu_focus: cx.focus_handle(),
            focus: cx.focus_handle(),
            _open_requests: open_requests,
            _tree_commands: tree_commands,
        };
        shell.load_drafts();
        shell
    }

    /// Opens the request `id` in a tab, inspecting it when it was not open.
    fn open_document(&mut self, id: &str, cx: &mut Context<Self>) {
        self.documents.open(id);
        self.drop_stale_url_edit(id);
        self.drop_stale_cell_edit(id);
        self.drop_stale_body_edit(id);
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
        self.inspect_open_requests(cx);
    }

    /// Inspects every open request again against the active environment, keeping its
    /// sub-tab; a request that can no longer be inspected is dropped.
    pub(super) fn inspect_open_requests(&mut self, cx: &mut Context<Self>) {
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

    /// Closes the environment menu without changing the active environment and gives the
    /// focus back: to an open text field, which keeps the keyboard focus the menu took,
    /// else to the shell, so the window's keys (Ctrl+Enter, Escape) keep reaching the
    /// shell's key context.
    fn close_environment_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.environment_menu_open = false;
        self.restore_focus(window, cx);
    }

    /// Gives the focus back after a menu closes: to an open text field, else to the shell.
    fn restore_focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let field = self
            .cell_edit
            .as_ref()
            .map(|edit| edit.input.clone())
            .or_else(|| self.url_edit.as_ref().map(|edit| edit.input.clone()));
        match field {
            Some(input) => input.update(cx, |state, cx| state.focus(window, cx)),
            None => self.focus.focus(window, cx),
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

    /// Activates the tab of the request `id`. A cell field that is open commits first,
    /// like clicking another cell does, so switching tabs keeps what was typed.
    fn activate_document(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.documents.activate(id);
        self.auth_menu_open = false;
        self.body_menu_open = false;
        self.method_menu_open = false;
        self.drop_stale_url_edit(id);
        self.drop_stale_body_edit(id);
        self.commit_cell_edit(window, cx);
        cx.notify();
    }

    /// Closes the document of the request `id`, cancelling its send in flight. Its draft,
    /// if it has one, stays in the app state and marks the tab when it is opened again.
    fn close_document(&mut self, id: &str, cx: &mut Context<Self>) {
        self.documents.close(id);
        self.requests.remove(id);
        self.sends.remove(id);
        self.save_errors.remove(id);
        self.drop_url_edit_of(id);
        self.drop_cell_edit_of(id);
        self.drop_body_edit_of(id);
        cx.notify();
    }
}

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = bolt::colors(cx);

        div()
            .key_context(SHELL_CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &SendRequest, window, cx| {
                this.send_active(window, cx);
            }))
            .on_action(cx.listener(|this, _: &CancelRequest, window, cx| {
                this.cancel_active(window, cx);
            }))
            .on_action(cx.listener(|this, _: &SaveRequest, window, cx| {
                this.save_active(window, cx);
            }))
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
                    .child(document::render(self, window, cx)),
            )
            .child(status_bar::render(self, cx))
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
mod auth_edit_tests;
#[cfg(test)]
mod body_edit_tests;
#[cfg(test)]
mod document_tests;
#[cfg(test)]
mod draft_tests;
#[cfg(test)]
mod environment_tests;
#[cfg(test)]
mod grid_edit_tests;
#[cfg(test)]
mod interaction_tests;
#[cfg(test)]
mod last_request_tests;
#[cfg(test)]
mod method_edit_tests;
#[cfg(test)]
mod request_tests;
#[cfg(test)]
mod response_tests;
#[cfg(test)]
mod row_edit_tests;
#[cfg(test)]
mod send_tests;
#[cfg(test)]
mod settings_edit_tests;
#[cfg(test)]
mod sidebar_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tree_edit_tests;
