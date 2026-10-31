//! The Variables panel of the sidebar island (variables-editor S6, S7), shown in place of
//! the collection tree while the rail's Environments view is active: Globals, each
//! collection and each environment, the `+` that creates an environment, an environment's
//! `…` menu with Rename and Delete, the delete confirmation, and the error or warning of the
//! last variable write.
//!
//! As with the tree, the sidebar never touches the workspace: it emits a
//! [`VariablesCommand`] and the shell runs it and reports back through
//! [`Sidebar::scopes_changed`] and [`Sidebar::show_variables_error`].

use bolt::IconName;
use bolt::chamfer::Chamfer;
use bolt::controls::IconButton;
use bolt::tokens::{border, chrome, cut, icon, space, text};
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, FocusHandle, InteractiveElement,
    IntoElement, ParentElement, Role, Styled, Window, div, prelude::*,
};
use gpui_component::input::{Enter, Escape, Input, InputState};
use reqflux_core::model::Id;
use reqflux_engine::SecretScope;

use super::request::option_menu;
use super::rows::{labeled_row, part};
use super::sidebar::Sidebar;
use super::variables_model::ScopeRow;
use super::{region, text as text_role, text_region};
use crate::tokens;

/// An environment or selection change the Variables panel asks the shell to make.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum VariablesCommand {
    /// Shows the variables of this scope in the document island.
    Select(SecretScope),
    /// Creates an environment.
    NewEnvironment,
    /// Renames the environment `id` to `name`.
    RenameEnvironment { id: Id, name: String },
    /// Deletes the environment `id`.
    DeleteEnvironment(Id),
}

/// The open rename field of an environment.
struct Rename {
    id: Id,
    original: String,
    input: Entity<InputState>,
}

/// What the Variables panel shows.
pub(crate) struct VariablesPanel {
    scopes: Vec<ScopeRow>,
    /// The scope whose variables the document island shows.
    selected: Option<SecretScope>,
    /// The environment whose menu is open.
    menu: Option<Id>,
    menu_focus: FocusHandle,
    rename: Option<Rename>,
    /// The environment the shell just created, which opens into rename once the field can
    /// be made with a window.
    pending_rename: Option<Id>,
    /// The environment whose delete waits for a confirmation.
    confirm: Option<Id>,
    /// Why the last write failed.
    error: Option<String>,
    /// A warning about the last write that succeeded.
    warning: Option<String>,
}

impl VariablesPanel {
    pub(super) fn new(scopes: Vec<ScopeRow>, cx: &mut App) -> Self {
        Self {
            scopes,
            selected: None,
            menu: None,
            menu_focus: cx.focus_handle(),
            rename: None,
            pending_rename: None,
            confirm: None,
            error: None,
            warning: None,
        }
    }

    /// The selected scope.
    pub(crate) fn selected(&self) -> Option<SecretScope> {
        self.selected
    }

    fn name(&self, id: Id) -> Option<&str> {
        self.scopes
            .iter()
            .find(|row| row.scope == SecretScope::Environment(id))
            .map(|row| row.name.as_str())
    }
}

impl Sidebar {
    /// Emits `command` for the shell to run. A created environment opens into rename once
    /// the shell has reported it, which happens before the deferred step runs.
    fn variables_command(
        &mut self,
        command: VariablesCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.variables.menu = None;
        cx.emit(command);
        cx.defer_in(window, |this, window, cx| {
            if let Some(id) = this.variables.pending_rename.take() {
                this.start_environment_rename(id, window, cx);
            }
        });
        cx.notify();
    }

    /// The shell ran a command or a write: the scopes are `scopes`, the error clears, a
    /// selection that no longer exists is dropped and `created` opens into rename.
    pub(crate) fn scopes_changed(
        &mut self,
        scopes: Vec<ScopeRow>,
        created: Option<Id>,
        cx: &mut Context<Self>,
    ) {
        let panel = &mut self.variables;
        panel.scopes = scopes;
        panel.error = None;
        if let Some(selected) = panel.selected
            && !panel.scopes.iter().any(|row| row.scope == selected)
        {
            panel.selected = None;
        }
        if let Some(id) = created {
            panel.selected = Some(SecretScope::Environment(id));
            panel.pending_rename = Some(id);
        }
        cx.notify();
    }

    /// Shows why the last write failed; the warning clears.
    pub(crate) fn show_variables_error(&mut self, error: String, cx: &mut Context<Self>) {
        self.variables.error = Some(error);
        self.variables.warning = None;
        cx.notify();
    }

    /// Shows `warning` about the write that just succeeded, or clears the error and the
    /// warning with `None`.
    pub(crate) fn show_variables_warning(
        &mut self,
        warning: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.variables.error = None;
        self.variables.warning = warning;
        cx.notify();
    }

    /// Escape: closes the menu or the delete confirmation. Returns whether it closed
    /// something.
    pub(super) fn dismiss_variables(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.variables.menu.is_some() {
            self.close_variables_menu(window, cx);
            true
        } else if self.variables.confirm.take().is_some() {
            cx.notify();
            true
        } else {
            false
        }
    }

    fn select_scope(&mut self, scope: SecretScope, window: &mut Window, cx: &mut Context<Self>) {
        self.variables.selected = Some(scope);
        self.variables_command(VariablesCommand::Select(scope), window, cx);
    }

    fn open_variables_menu(&mut self, id: Id, window: &mut Window, cx: &mut Context<Self>) {
        self.variables.menu = Some(id);
        self.variables.menu_focus.focus(window, cx);
        cx.notify();
    }

    fn close_variables_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.variables.menu = None;
        self.focus.focus(window, cx);
        cx.notify();
    }

    /// Opens the rename field over the name of the environment `id`, all selected.
    fn start_environment_rename(&mut self, id: Id, window: &mut Window, cx: &mut Context<Self>) {
        let Some(name) = self.variables.name(id).map(str::to_owned) else {
            return;
        };
        let input = cx.new(|cx| InputState::new(window, cx));
        input.update(cx, |state, cx| {
            state.set_value(name.clone(), window, cx);
            state.select_all(window, cx);
            state.focus(window, cx);
        });
        self.variables.menu = None;
        self.variables.rename = Some(Rename {
            id,
            original: name,
            input,
        });
        cx.notify();
    }

    /// Enter in the rename field: renames through the shell unless the name is unchanged.
    fn commit_environment_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(rename) = self.variables.rename.take() else {
            return;
        };
        let name = rename.input.read(cx).value().to_string();
        self.focus.focus(window, cx);
        if name == rename.original {
            cx.notify();
            return;
        }
        self.variables_command(
            VariablesCommand::RenameEnvironment {
                id: rename.id,
                name,
            },
            window,
            cx,
        );
    }

    fn cancel_environment_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.variables.rename.take().is_some() {
            self.focus.focus(window, cx);
            cx.notify();
        }
    }

    fn ask_delete(&mut self, id: Id, window: &mut Window, cx: &mut Context<Self>) {
        self.variables.menu = None;
        self.variables.confirm = Some(id);
        self.focus.focus(window, cx);
        cx.notify();
    }

    fn confirm_environment_delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(id) = self.variables.confirm.take() {
            self.variables_command(VariablesCommand::DeleteEnvironment(id), window, cx);
        }
    }

    fn cancel_environment_delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.variables.confirm = None;
        self.focus.focus(window, cx);
        cx.notify();
    }

    /// The Variables panel, `variables-panel`: the `VARIABLES` header with the `+`, the
    /// error or warning, the delete confirmation and the scope rows.
    pub(super) fn variables_panel(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let header = region("variables-header")
            .flex_none()
            .h(tokens::SIDEBAR_HEADER)
            .flex()
            .items_center()
            .gap(space::SM)
            .pl(space::LG)
            .pr(space::MD)
            .child(
                text_role::role(text_region("variables-label"), text::LABEL, cx).child("VARIABLES"),
            )
            .child(div().flex_1())
            .child(
                IconButton::new(
                    "variables-new-environment",
                    IconName::Plus,
                    "New environment",
                )
                .raised()
                .on_click(cx.listener(|this, _event, window, cx| {
                    this.variables_command(VariablesCommand::NewEnvironment, window, cx);
                })),
            );
        let rows: Vec<_> = self
            .variables
            .scopes
            .clone()
            .into_iter()
            .map(|row| self.scope_row(row, window, cx))
            .collect();
        region("variables-panel")
            .size_full()
            .flex()
            .flex_col()
            .child(header)
            .children(self.variables_message(cx))
            .children(self.environment_delete_prompt(cx))
            .child(
                region("variables-scopes")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .children(rows),
            )
            .into_any_element()
    }

    fn scope_row(&self, row: ScopeRow, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let selected = self.variables.selected == Some(row.scope);
        let environment = match row.scope {
            SecretScope::Environment(id) => Some(id),
            SecretScope::Global | SecretScope::Collection(_) => None,
        };
        let name: AnyElement = match environment.and_then(|id| self.rename_input(id, cx)) {
            Some(field) => field,
            None => div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .child(row.name.clone())
                .into_any_element(),
        };
        let actions = environment
            .map(|id| self.environment_actions(id, selected, window, cx))
            .unwrap_or_default();
        let colors = bolt::colors(cx);
        let hover = colors.hover;
        let strong = colors.strong;
        let wash = bolt::accent_washes(cx).selected_item;
        let scope = row.scope;
        text_role::role(labeled_row(row.element_id(), row.name), text::BODY, cx)
            .role(Role::ListItem)
            .aria_selected(selected)
            .relative()
            .flex_none()
            .h(chrome::TREE_ROW)
            .flex()
            .items_center()
            .gap(space::SM)
            .pl(space::LG)
            .pr(space::MD)
            .cursor_pointer()
            .group(ROW_GROUP)
            .hover(move |style| style.bg(hover))
            .when(selected, |this| this.bg(wash).text_color(strong))
            .child(name)
            .children(actions)
            .on_click(cx.listener(move |this, _event, window, cx| {
                this.select_scope(scope, window, cx);
            }))
            .into_any_element()
    }

    fn rename_input(&self, id: Id, cx: &mut Context<Self>) -> Option<AnyElement> {
        let rename = self
            .variables
            .rename
            .as_ref()
            .filter(|rename| rename.id == id)?;
        let colors = bolt::colors(cx);
        Some(
            region("variables-rename-input")
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
                    this.commit_environment_rename(window, cx);
                }))
                .on_action(cx.listener(|this, _: &Escape, window, cx| {
                    this.cancel_environment_rename(window, cx);
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

    /// The `…` button of the environment `id`, `variables-scope-menu-<id>`, and its menu
    /// while open.
    fn environment_actions(
        &self,
        id: Id,
        selected: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let muted = bolt::colors(cx).muted;
        let button = part(format!("variables-scope-menu-{id}"))
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
                bolt::Icon::new(IconName::Ellipsis)
                    .size(icon::SM)
                    .color(muted),
            )
            .on_click(cx.listener(move |this, _event, window, cx| {
                cx.stop_propagation();
                this.open_variables_menu(id, window, cx);
            }))
            .into_any_element();
        let mut actions = vec![button];
        if self.variables.menu == Some(id) {
            let items = vec![
                option_menu::option(
                    "variables-menu-rename".to_owned(),
                    "Rename".to_owned(),
                    false,
                    move |this: &mut Sidebar, window, cx| {
                        this.start_environment_rename(id, window, cx);
                    },
                    cx,
                ),
                option_menu::option(
                    "variables-menu-delete".to_owned(),
                    "Delete".to_owned(),
                    false,
                    move |this: &mut Sidebar, window, cx| this.ask_delete(id, window, cx),
                    cx,
                ),
            ];
            actions.push(option_menu::render(
                ("variables-menu", "variables-menu-backdrop"),
                &self.variables.menu_focus,
                items,
                Sidebar::close_variables_menu,
                window,
                cx,
            ));
        }
        actions
    }

    /// The error of the last failed write, `variables-error`, in the danger tone, or the
    /// warning of the last one that succeeded, `variables-warning`.
    fn variables_message(&self, cx: &App) -> Option<AnyElement> {
        let colors = bolt::colors(cx);
        let (id, message, color) = match (&self.variables.error, &self.variables.warning) {
            (Some(error), _) => ("variables-error", error.clone(), colors.danger),
            (None, Some(warning)) => ("variables-warning", warning.clone(), colors.warning),
            (None, None) => return None,
        };
        Some(
            text_role::role(text_region(id), text::CAPTION, cx)
                .aria_label(message.clone())
                .flex_none()
                .pl(space::LG)
                .pr(space::LG)
                .pb(space::SM)
                .text_color(color)
                .child(message)
                .into_any_element(),
        )
    }

    /// The delete confirmation, `variables-delete-prompt`, with `variables-delete-confirm`
    /// and `variables-delete-cancel`.
    fn environment_delete_prompt(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let id = self.variables.confirm?;
        let question = format!("Delete environment \"{}\"?", self.variables.name(id)?);
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
            text_role::role(region("variables-delete-prompt"), text::BODY, cx)
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
                            button("variables-delete-confirm", "Delete", cx)
                                .text_color(danger)
                                .on_click(cx.listener(|this, _event, window, cx| {
                                    this.confirm_environment_delete(window, cx);
                                })),
                        )
                        .child(button("variables-delete-cancel", "Cancel", cx).on_click(
                            cx.listener(|this, _event, window, cx| {
                                this.cancel_environment_delete(window, cx);
                            }),
                        )),
                )
                .into_any_element(),
        )
    }
}

/// The hover group of a scope row, which shows an environment's action button.
const ROW_GROUP: &str = "variables-row";
