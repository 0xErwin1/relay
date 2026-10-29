//! The sidebar island: the collection tree under `COLLECTIONS` and the environment names
//! under `ENVIRONMENTS`.
//!
//! The tree keeps keyboard focus as a whole; its rows are not focusable. Navigation runs
//! through the actions below, bound in the `SidebarTree` key context.

use bolt::Island;
use bolt::tokens::{chrome, space, text};
use gpui::{
    App, Context, FocusHandle, Focusable, InteractiveElement, IntoElement, KeyBinding,
    ParentElement, Render, Styled, Window, actions, div, prelude::*,
};

use super::tree_model::{self, TreeState};
use super::{region, rows, text as text_role, text_region};
use crate::tokens;
use crate::workspace::OpenedWorkspace;

/// The key context of the collection tree.
pub(crate) const TREE_CONTEXT: &str = "SidebarTree";

actions!(
    sidebar_tree,
    [
        /// Selects the previous visible row.
        SelectPrevious,
        /// Selects the next visible row.
        SelectNext,
        /// Collapses the selected row, or selects its parent.
        CollapseOrSelectParent,
        /// Expands the selected row, or selects its first child.
        ExpandOrSelectChild,
        /// Expands or collapses the selected collection or folder.
        ToggleSelected,
    ]
);

/// Binds the tree navigation keys in [`TREE_CONTEXT`].
pub(crate) fn bind_keys(cx: &mut App) {
    let context = Some(TREE_CONTEXT);
    cx.bind_keys([
        KeyBinding::new("up", SelectPrevious, context),
        KeyBinding::new("down", SelectNext, context),
        KeyBinding::new("left", CollapseOrSelectParent, context),
        KeyBinding::new("right", ExpandOrSelectChild, context),
        KeyBinding::new("enter", ToggleSelected, context),
    ]);
}

/// An environment as the sidebar lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct EnvironmentRow {
    id: String,
    name: String,
    variables: usize,
}

/// The sidebar view.
pub(crate) struct Sidebar {
    /// `None` when no workspace is loaded: the island stays empty.
    tree: Option<TreeState>,
    environments: Vec<EnvironmentRow>,
    focus: FocusHandle,
}

impl Sidebar {
    pub(crate) fn new(workspace: &OpenedWorkspace, cx: &mut Context<Self>) -> Self {
        let (tree, environments) = match workspace {
            OpenedWorkspace::Loaded(engine) => {
                let workspace = engine.workspace();
                let environments = workspace
                    .environments
                    .iter()
                    .map(|node| EnvironmentRow {
                        id: node.environment.id.to_string(),
                        name: node.environment.name.clone(),
                        variables: node.environment.variables.len(),
                    })
                    .collect();
                (
                    Some(TreeState::new(tree_model::nodes(workspace))),
                    environments,
                )
            }
            OpenedWorkspace::None | OpenedWorkspace::Failed { .. } => (None, Vec::new()),
        };
        Self {
            tree,
            environments,
            focus: cx.focus_handle(),
        }
    }

    /// The id of the selected tree node.
    #[cfg(test)]
    pub(crate) fn selected(&self) -> Option<&str> {
        self.tree.as_ref().and_then(TreeState::selected)
    }

    fn update_tree(&mut self, cx: &mut Context<Self>, change: impl FnOnce(&mut TreeState)) {
        if let Some(tree) = &mut self.tree {
            change(tree);
            cx.notify();
        }
    }

    /// Selects the row `id` and focuses the tree; `toggle` also expands or collapses it.
    fn click_row(&mut self, id: &str, toggle: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.update_tree(cx, |tree| {
            tree.select(id);
            if toggle {
                tree.toggle(id);
            }
        });
        self.focus.focus(window, cx);
    }
}

impl Focusable for Sidebar {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

fn section_header(id: &'static str, label: &'static str, cx: &App) -> impl IntoElement {
    text_role::role(text_region(id), text::LABEL, cx)
        .flex_none()
        .h(tokens::SECTION_HEADER)
        .flex()
        .items_center()
        .pl(space::MD)
        .child(label)
}

impl Render for Sidebar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut content = region("sidebar-content").size_full().flex().flex_col();

        if let Some(tree) = &self.tree {
            let tree_rows = tree.visible_rows().into_iter().map(|row| {
                let selected = tree.selected() == Some(row.node.id.as_str());
                let id = row.node.id.clone();
                let chevron_id = id.clone();
                let on_toggle = cx.listener(move |this, _event, window, cx| {
                    this.click_row(&chevron_id, true, window, cx);
                });
                rows::tree_row(row, selected, on_toggle, cx).on_click(cx.listener(
                    move |this, _event, window, cx| this.click_row(&id, false, window, cx),
                ))
            });
            content = content
                .child(section_header("collections-label", "COLLECTIONS", cx))
                .child(
                    region("collection-tree")
                        .track_focus(&self.focus)
                        .key_context(TREE_CONTEXT)
                        .on_action(cx.listener(|this, _: &SelectPrevious, _, cx| {
                            this.update_tree(cx, TreeState::select_previous);
                        }))
                        .on_action(cx.listener(|this, _: &SelectNext, _, cx| {
                            this.update_tree(cx, TreeState::select_next);
                        }))
                        .on_action(cx.listener(|this, _: &CollapseOrSelectParent, _, cx| {
                            this.update_tree(cx, TreeState::collapse_or_select_parent);
                        }))
                        .on_action(cx.listener(|this, _: &ExpandOrSelectChild, _, cx| {
                            this.update_tree(cx, TreeState::expand_or_select_child);
                        }))
                        .on_action(cx.listener(|this, _: &ToggleSelected, _, cx| {
                            this.update_tree(cx, TreeState::toggle_selected);
                        }))
                        .flex()
                        .flex_col()
                        .children(tree_rows),
                )
                .child(div().h(space::MD).flex_none())
                .child(section_header("environments-label", "ENVIRONMENTS", cx))
                .children(self.environments.iter().map(|environment| {
                    rows::environment_row(
                        format!("env-row-{}", environment.id),
                        &environment.name,
                        environment.variables,
                        cx,
                    )
                }));
        }

        region("sidebar")
            .flex_none()
            .w(chrome::SIDEBAR_WIDTH)
            .h_full()
            .child(Island::new().size_full().child(content))
    }
}
