//! The sidebar island: the `COLLECTIONS` header with the new-request button, the filter
//! field, the collection tree and a footer with the request and collection counts. The
//! button and the filter do nothing yet.
//!
//! The tree keeps keyboard focus as a whole; its rows are not focusable. Navigation runs
//! through the actions below, bound in the `SidebarTree` key context.

use bolt::controls::{Field, IconButton};
use bolt::tokens::{chrome, space, text};
use bolt::{IconName, Island};
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

/// The footer text: `<n> requests · <m> collections`, singular for one.
pub(crate) fn footer_text(requests: usize, collections: usize) -> String {
    let plural = |count: usize, noun: &str| match count {
        1 => format!("1 {noun}"),
        count => format!("{count} {noun}s"),
    };
    format!(
        "{} · {}",
        plural(requests, "request"),
        plural(collections, "collection")
    )
}

/// The sidebar view.
pub(crate) struct Sidebar {
    /// `None` when no workspace is loaded: the island stays empty.
    tree: Option<TreeState>,
    focus: FocusHandle,
}

impl Sidebar {
    pub(crate) fn new(workspace: &OpenedWorkspace, cx: &mut Context<Self>) -> Self {
        let tree = match workspace {
            OpenedWorkspace::Loaded(engine) => {
                Some(TreeState::new(tree_model::nodes(engine.workspace())))
            }
            OpenedWorkspace::None | OpenedWorkspace::Failed { .. } => None,
        };
        Self {
            tree,
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

fn header(cx: &App) -> impl IntoElement {
    region("collections-header")
        .flex_none()
        .h(tokens::SIDEBAR_HEADER)
        .flex()
        .items_center()
        .gap(space::SM)
        .pl(space::LG)
        .pr(space::MD)
        .child(
            text_role::role(text_region("collections-label"), text::LABEL, cx).child("COLLECTIONS"),
        )
        .child(div().flex_1())
        .child(IconButton::new("new-request", IconName::Plus, "New request").raised())
}

fn filter() -> impl IntoElement {
    div()
        .flex_none()
        .pl(space::MD)
        .pr(space::MD)
        .pb(tokens::SIDEBAR_FILTER_BOTTOM)
        .child(
            Field::new("tree-filter", "Filter requests")
                .leading_icon(IconName::ListFilter)
                .trailing_keycap("/")
                .w_full(),
        )
}

fn footer(tree: &TreeState, cx: &App) -> impl IntoElement {
    text_role::role(text_region("sidebar-footer"), text::CAPTION, cx)
        .flex_none()
        .h(tokens::SIDEBAR_FOOTER)
        .flex()
        .items_center()
        .pl(space::LG)
        .pr(space::LG)
        .whitespace_nowrap()
        .child(footer_text(tree.request_count(), tree.collection_count()))
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
                .child(header(cx))
                .child(filter())
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
                        .flex_1()
                        .min_h_0()
                        .overflow_hidden()
                        .flex()
                        .flex_col()
                        .children(tree_rows),
                )
                .child(footer(tree, cx));
        }

        region("sidebar")
            .flex_none()
            .w(chrome::SIDEBAR_WIDTH)
            .h_full()
            .child(Island::new().size_full().child(content))
    }
}
