//! Editing the tree from the sidebar: the row menu, the inline rename field, the delete
//! confirmation and the error of the last failed operation.
//!
//! The sidebar never touches the workspace: it emits a [`TreeCommand`], and the shell, which
//! owns the engine, runs it and reports back through [`Sidebar::tree_changed`] or
//! [`Sidebar::show_error`].

use bolt::chamfer::Chamfer;
use bolt::tokens::{border, chrome, cut, icon, space, text};
use bolt::{Icon, IconName};
use gpui::{
    AnyElement, App, Context, Entity, FocusHandle, InteractiveElement, IntoElement, ParentElement,
    Role, Styled, Window, div, prelude::*,
};
use gpui_component::input::{Enter, Escape, Input, InputState};

use super::request::option_menu;
use super::rows::part;
use super::sidebar::Sidebar;
use super::tree_model::{Node, NodeKind};
use super::{region, text as text_role, text_region};

/// The hover group of a tree row, which shows its action button.
pub(super) const ROW_GROUP: &str = "tree-row";

/// A tree operation the sidebar asks the shell to run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TreeCommand {
    /// Creates a collection.
    NewCollection,
    /// Creates a request at the end of the collection or folder with this id.
    NewRequest(String),
    /// Creates a folder at the end of the collection or folder with this id.
    NewFolder(String),
    /// Renames the entity `id` to `name`.
    Rename { id: String, name: String },
    /// Duplicates the request or folder with this id.
    Duplicate(String),
    /// Deletes the entity with this id and everything under it.
    Delete(String),
}

/// The open rename field: the entity it renames, the name it started from and its input.
pub(crate) struct Rename {
    id: String,
    original: String,
    input: Entity<InputState>,
}

/// What the sidebar shows while the tree is edited.
pub(crate) struct TreeEdit {
    /// The id of the row whose menu is open.
    menu: Option<String>,
    /// Focused while the menu is open.
    menu_focus: FocusHandle,
    rename: Option<Rename>,
    /// The entity the shell just created, which opens into rename once the field can be
    /// made with a window.
    pending_rename: Option<String>,
    /// The id of the entity whose delete waits for a confirmation.
    confirm: Option<String>,
    /// Why the last operation failed.
    error: Option<String>,
}

impl TreeEdit {
    pub(super) fn new(cx: &mut App) -> Self {
        Self {
            menu: None,
            menu_focus: cx.focus_handle(),
            rename: None,
            pending_rename: None,
            confirm: None,
            error: None,
        }
    }
}

/// The confirmation question for deleting the node of `kind` named `name` that holds
/// `requests` requests.
pub(crate) fn delete_question(kind: &NodeKind, name: &str, requests: usize) -> String {
    let noun = match kind {
        NodeKind::Collection => "collection",
        NodeKind::Folder => "folder",
        NodeKind::Request { .. } => return format!("Delete request \"{name}\"?"),
    };
    match requests {
        0 => format!("Delete {noun} \"{name}\"?"),
        1 => format!("Delete {noun} \"{name}\" and its 1 request?"),
        count => format!("Delete {noun} \"{name}\" and its {count} requests?"),
    }
}

impl Sidebar {
    /// Emits `command` for the shell to run. A command that creates an entity opens it into
    /// rename once the shell has reported it, which happens before the deferred step runs.
    pub(super) fn command(
        &mut self,
        command: TreeCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.edit.menu = None;
        cx.emit(command);
        cx.defer_in(window, |this, window, cx| {
            if let Some(id) = this.edit.pending_rename.take() {
                this.start_rename(&id, window, cx);
            }
        });
        cx.notify();
    }

    /// The shell ran an operation: the error clears, and `created`, when the operation made
    /// an entity, is selected and shown; `rename` also opens it into rename.
    pub(crate) fn tree_changed(
        &mut self,
        roots: Vec<Node>,
        created: Option<String>,
        rename: bool,
        cx: &mut Context<Self>,
    ) {
        self.edit.error = None;
        self.set_roots(roots, cx);
        if let Some(id) = created {
            self.update_tree(cx, |tree| tree.reveal(&id));
            if rename {
                self.edit.pending_rename = Some(id);
            }
        }
        cx.notify();
    }

    /// Shows why the last operation failed, above the tree.
    pub(crate) fn show_error(&mut self, error: String, cx: &mut Context<Self>) {
        self.edit.error = Some(error);
        cx.notify();
    }

    /// Why the last operation failed, while the sidebar shows it.
    #[cfg(test)]
    pub(crate) fn error(&self) -> Option<&str> {
        self.edit.error.as_deref()
    }

    /// The text of the rename field, while it is open.
    #[cfg(test)]
    pub(crate) fn rename_text(&self, cx: &App) -> Option<String> {
        self.edit
            .rename
            .as_ref()
            .map(|rename| rename.input.read(cx).value().to_string())
    }

    /// Escape: closes the menu or the delete confirmation and clears the error. Returns
    /// whether it closed something, so the shell does not also cancel a send.
    pub(crate) fn dismiss(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let had_error = self.edit.error.take().is_some();
        let closed = if self.edit.menu.is_some() {
            self.close_menu(window, cx);
            true
        } else {
            self.edit.confirm.take().is_some()
        };
        if had_error || closed {
            cx.notify();
        }
        closed
    }

    /// Opens the menu of the row `id`, selecting it.
    pub(super) fn open_menu(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.update_tree(cx, |tree| tree.select(id));
        self.edit.menu = Some(id.to_owned());
        self.edit.menu_focus.focus(window, cx);
        cx.notify();
    }

    fn close_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.edit.menu = None;
        self.focus.focus(window, cx);
        cx.notify();
    }

    /// Opens the rename field over the name of `id`, seeded with the name, all selected.
    fn start_rename(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(name) = self.node(id).map(|node| node.name.clone()) else {
            return;
        };
        let input = cx.new(|cx| InputState::new(window, cx));
        input.update(cx, |state, cx| {
            state.set_value(name.clone(), window, cx);
            state.select_all(window, cx);
            state.focus(window, cx);
        });
        self.edit.menu = None;
        self.edit.rename = Some(Rename {
            id: id.to_owned(),
            original: name,
            input,
        });
        cx.notify();
    }

    /// Enter in the rename field: renames through the shell unless the name is unchanged.
    fn commit_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(rename) = self.edit.rename.take() else {
            return;
        };
        let name = rename.input.read(cx).value().to_string();
        self.focus.focus(window, cx);
        if name == rename.original {
            cx.notify();
            return;
        }
        self.command(
            TreeCommand::Rename {
                id: rename.id,
                name,
            },
            window,
            cx,
        );
    }

    /// Escape in the rename field: closes it and keeps the name.
    fn cancel_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.edit.rename.take().is_some() {
            self.focus.focus(window, cx);
            cx.notify();
        }
    }

    /// The menu item `pick` on the row `id`.
    fn pick(&mut self, id: &str, pick: MenuItem, window: &mut Window, cx: &mut Context<Self>) {
        self.edit.menu = None;
        // New items go into a collection or folder; on a request, into the request's parent.
        let container = match self.node(id) {
            Some(node) if node.is_expandable() => Some(id.to_owned()),
            Some(_) | None => self.parent(id),
        };
        match (pick, container) {
            (MenuItem::NewRequest, Some(parent)) => {
                self.command(TreeCommand::NewRequest(parent), window, cx);
            }
            (MenuItem::NewFolder, Some(parent)) => {
                self.command(TreeCommand::NewFolder(parent), window, cx);
            }
            (MenuItem::NewRequest | MenuItem::NewFolder, None) => self.close_menu(window, cx),
            (MenuItem::Rename, _) => self.start_rename(id, window, cx),
            (MenuItem::Duplicate, _) => {
                self.command(TreeCommand::Duplicate(id.to_owned()), window, cx);
            }
            (MenuItem::Delete, _) => {
                self.edit.confirm = Some(id.to_owned());
                self.focus.focus(window, cx);
                cx.notify();
            }
        }
    }

    fn confirm_delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(id) = self.edit.confirm.take() {
            self.command(TreeCommand::Delete(id), window, cx);
        }
    }

    fn cancel_delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.edit.confirm = None;
        self.focus.focus(window, cx);
        cx.notify();
    }

    /// The rename field over the name of the row `id`, when it is open there.
    pub(super) fn rename_field(&self, id: &str, cx: &mut Context<Self>) -> Option<AnyElement> {
        let rename = self.edit.rename.as_ref().filter(|rename| rename.id == id)?;
        let colors = bolt::colors(cx);
        Some(
            region("tree-rename-input")
                .role(Role::TextInput)
                .relative()
                .flex_1()
                .min_w_0()
                .h(chrome::TREE_ROW)
                .flex()
                .items_center()
                .pl(space::HALF)
                .pr(space::HALF)
                .child(
                    Chamfer::new(cut::CONTROL)
                        .fill(colors.background)
                        .edge(colors.line, border::THIN),
                )
                .on_action(cx.listener(|this, _: &Enter, window, cx| {
                    this.commit_rename(window, cx);
                }))
                .on_action(cx.listener(|this, _: &Escape, window, cx| {
                    this.cancel_rename(window, cx);
                }))
                .child(
                    Input::new(&rename.input)
                        .appearance(false)
                        .bordered(false)
                        .flex_1()
                        .min_w_0()
                        .text_color(colors.strong),
                )
                .into_any_element(),
        )
    }

    /// The `…` button of the row `id`, shown while the row is selected or hovered, and the
    /// row's menu while it is open.
    pub(super) fn row_actions(
        &self,
        node: &Node,
        selected: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let colors = bolt::colors(cx);
        let id = node.id.clone();
        let button = part(format!("tree-row-menu-{id}"))
            .role(Role::Button)
            .aria_label("Actions")
            .flex_none()
            .cursor_pointer()
            .when(!selected, |button| {
                button
                    .invisible()
                    .group_hover(ROW_GROUP, |style| style.visible())
            })
            .child(
                Icon::new(IconName::Ellipsis)
                    .size(icon::SM)
                    .color(colors.muted),
            )
            .on_click(cx.listener(move |this, _event, window, cx| {
                cx.stop_propagation();
                this.open_menu(&id, window, cx);
            }))
            .into_any_element();
        let mut actions = vec![button];
        if self.edit.menu.as_deref() == Some(node.id.as_str()) {
            actions.push(self.menu(node, window, cx));
        }
        actions
    }

    fn menu(&self, node: &Node, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let items = MenuItem::ALL
            .into_iter()
            .filter(|item| item.applies_to(&node.kind))
            .map(|item| {
                let id = node.id.clone();
                option_menu::option(
                    format!("tree-menu-{}", item.slug()),
                    item.label().to_owned(),
                    false,
                    move |this: &mut Sidebar, window, cx| this.pick(&id, item, window, cx),
                    cx,
                )
            })
            .collect();
        option_menu::render(
            ("tree-menu", "tree-menu-backdrop"),
            &self.edit.menu_focus,
            items,
            Sidebar::close_menu,
            window,
            cx,
        )
    }

    /// The error of the last failed operation, `sidebar-error`, in the danger tone.
    pub(super) fn error_label(&self, cx: &App) -> Option<AnyElement> {
        let error = self.edit.error.clone()?;
        let colors = bolt::colors(cx);
        Some(
            text_role::role(text_region("sidebar-error"), text::CAPTION, cx)
                .aria_label(error.clone())
                .flex_none()
                .pl(space::LG)
                .pr(space::LG)
                .pb(space::SM)
                .text_color(colors.danger)
                .child(error)
                .into_any_element(),
        )
    }

    /// The delete confirmation, `tree-delete-prompt`, while a delete waits for it: the
    /// question and the `tree-delete-confirm` and `tree-delete-cancel` buttons.
    pub(super) fn delete_prompt(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let node = self.node(self.edit.confirm.as_deref()?)?;
        let question = delete_question(&node.kind, &node.name, node.request_count());
        let (hover, danger, strong) = {
            let colors = bolt::colors(cx);
            (colors.hover, colors.danger, colors.strong)
        };
        let button = |id: &'static str, label: &'static str, cx: &mut Context<Self>| {
            text_role::role(text_region(id), text::BODY, cx)
                .role(Role::Button)
                .aria_label(label)
                .cursor_pointer()
                .pl(space::SM)
                .pr(space::SM)
                .hover(move |style| style.bg(hover))
                .child(label)
        };
        Some(
            text_role::role(region("tree-delete-prompt"), text::BODY, cx)
                .role(Role::Dialog)
                .aria_label(question.clone())
                .flex_none()
                .flex()
                .flex_col()
                .gap(space::SM)
                .pl(space::LG)
                .pr(space::LG)
                .pb(space::SM)
                .child(div().text_color(strong).child(question))
                .child(
                    div()
                        .flex()
                        .gap(space::SM)
                        .child(
                            button("tree-delete-confirm", "Delete", cx)
                                .text_color(danger)
                                .on_click(cx.listener(|this, _event, window, cx| {
                                    this.confirm_delete(window, cx);
                                })),
                        )
                        .child(button("tree-delete-cancel", "Cancel", cx).on_click(
                            cx.listener(|this, _event, window, cx| this.cancel_delete(window, cx)),
                        )),
                )
                .into_any_element(),
        )
    }
}

/// An item of the row menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MenuItem {
    NewRequest,
    NewFolder,
    Rename,
    Duplicate,
    Delete,
}

impl MenuItem {
    const ALL: [Self; 5] = [
        Self::NewRequest,
        Self::NewFolder,
        Self::Rename,
        Self::Duplicate,
        Self::Delete,
    ];

    fn slug(self) -> &'static str {
        match self {
            Self::NewRequest => "new-request",
            Self::NewFolder => "new-folder",
            Self::Rename => "rename",
            Self::Duplicate => "duplicate",
            Self::Delete => "delete",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::NewRequest => "New request",
            Self::NewFolder => "New folder",
            Self::Rename => "Rename",
            Self::Duplicate => "Duplicate",
            Self::Delete => "Delete",
        }
    }

    /// Whether the item is offered on a row of `kind`: New folder on collections and
    /// folders, Duplicate on folders and requests.
    fn applies_to(self, kind: &NodeKind) -> bool {
        match self {
            Self::NewFolder => !matches!(kind, NodeKind::Request { .. }),
            Self::Duplicate => !matches!(kind, NodeKind::Collection),
            Self::NewRequest | Self::Rename | Self::Delete => true,
        }
    }
}
