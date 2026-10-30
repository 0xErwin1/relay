//! The sidebar collection tree without GPUI: the nodes, which of them are expanded, the
//! selected one, the rows that are visible and the keyboard navigation over them.

use std::collections::HashSet;

use reqflux_core::model::{CollectionNode, Item, Workspace};

/// What a tree node stands for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NodeKind {
    /// A collection, at the top level.
    Collection,
    /// A folder inside a collection or folder.
    Folder,
    /// A request, carrying its uppercase method.
    Request {
        /// The HTTP method, as the tree shows it.
        method: String,
    },
}

/// One node of the collection tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Node {
    /// The entity id.
    pub(crate) id: String,
    /// The display name.
    pub(crate) name: String,
    /// What the node stands for.
    pub(crate) kind: NodeKind,
    /// Children in store order.
    pub(crate) children: Vec<Node>,
}

impl Node {
    /// Whether the node can be expanded: collections and folders.
    pub(crate) fn is_expandable(&self) -> bool {
        !matches!(self.kind, NodeKind::Request { .. })
    }

    /// The number of direct children of a collection or folder; `None` for a request.
    pub(crate) fn child_count(&self) -> Option<usize> {
        self.is_expandable().then_some(self.children.len())
    }

    /// The number of requests under the node, at any depth, the node included.
    fn request_count(&self) -> usize {
        match self.kind {
            NodeKind::Request { .. } => 1,
            NodeKind::Collection | NodeKind::Folder => {
                self.children.iter().map(Node::request_count).sum()
            }
        }
    }

    /// The text the UI automation tree reads for the row: the name, prefixed by the method
    /// for a request.
    pub(crate) fn label(&self) -> String {
        match &self.kind {
            NodeKind::Request { method } => format!("{method} {}", self.name),
            NodeKind::Collection | NodeKind::Folder => self.name.clone(),
        }
    }
}

/// The tree nodes of every collection of `workspace`, in store order.
pub(crate) fn nodes(workspace: &Workspace) -> Vec<Node> {
    workspace.collections.iter().map(collection_node).collect()
}

fn collection_node(collection: &CollectionNode) -> Node {
    Node {
        id: collection.collection.id.to_string(),
        name: collection.collection.name.clone(),
        kind: NodeKind::Collection,
        children: collection.items.iter().map(item_node).collect(),
    }
}

fn item_node(item: &Item) -> Node {
    match item {
        Item::Folder(folder) => Node {
            id: folder.folder.id.to_string(),
            name: folder.folder.name.clone(),
            kind: NodeKind::Folder,
            children: folder.items.iter().map(item_node).collect(),
        },
        Item::Request(request) => Node {
            id: request.request.id.to_string(),
            name: request.request.name.clone(),
            kind: NodeKind::Request {
                method: request.request.method.as_str().to_owned(),
            },
            children: Vec::new(),
        },
    }
}

/// A row of the tree as it is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct VisibleRow<'a> {
    /// The node of the row.
    pub(crate) node: &'a Node,
    /// The nesting level, 0 for a collection.
    pub(crate) depth: usize,
    /// Whether the node is expanded; always false for a request.
    pub(crate) expanded: bool,
    /// The index of the parent row among the visible rows.
    pub(crate) parent: Option<usize>,
}

/// A change a navigation key makes.
enum Step {
    Collapse(String),
    Expand(String),
    Select(Option<String>),
}

/// The collection tree with its expansion and selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TreeState {
    roots: Vec<Node>,
    expanded: HashSet<String>,
    selected: Option<String>,
}

impl TreeState {
    /// A tree over `roots` with every collection expanded, every folder collapsed and
    /// nothing selected.
    pub(crate) fn new(roots: Vec<Node>) -> Self {
        let expanded = roots.iter().map(|root| root.id.clone()).collect();
        Self {
            roots,
            expanded,
            selected: None,
        }
    }

    /// Replaces the tree's roots, keeping the expansion and the selection; ids that no
    /// longer exist simply never match again.
    pub(crate) fn set_roots(&mut self, roots: Vec<Node>) {
        self.roots = roots;
    }

    /// The number of requests in every collection.
    pub(crate) fn request_count(&self) -> usize {
        self.roots.iter().map(Node::request_count).sum()
    }

    /// The number of collections.
    pub(crate) fn collection_count(&self) -> usize {
        self.roots.len()
    }

    /// The id of the selected node.
    pub(crate) fn selected(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    /// Whether `id` is a request anywhere in the tree, shown or not.
    pub(crate) fn is_request(&self, id: &str) -> bool {
        fn find(nodes: &[Node], id: &str) -> bool {
            nodes.iter().any(|node| match node.kind {
                NodeKind::Request { .. } => node.id == id,
                NodeKind::Collection | NodeKind::Folder => find(&node.children, id),
            })
        }
        find(&self.roots, id)
    }

    /// Selects the node `id`.
    pub(crate) fn select(&mut self, id: &str) {
        self.selected = Some(id.to_owned());
    }

    /// The rows that are shown: every root, and the children of each expanded node, depth
    /// first in store order.
    pub(crate) fn visible_rows(&self) -> Vec<VisibleRow<'_>> {
        let mut rows = Vec::new();
        for root in &self.roots {
            self.push_rows(root, 0, None, &mut rows);
        }
        rows
    }

    fn push_rows<'a>(
        &'a self,
        node: &'a Node,
        depth: usize,
        parent: Option<usize>,
        rows: &mut Vec<VisibleRow<'a>>,
    ) {
        let expanded = node.is_expandable() && self.expanded.contains(&node.id);
        let index = rows.len();
        rows.push(VisibleRow {
            node,
            depth,
            expanded,
            parent,
        });
        if expanded {
            for child in &node.children {
                self.push_rows(child, depth + 1, Some(index), rows);
            }
        }
    }

    fn selected_index(rows: &[VisibleRow<'_>], selected: Option<&str>) -> Option<usize> {
        let selected = selected?;
        rows.iter().position(|row| row.node.id == selected)
    }

    /// Up: selects the previous visible row; the first row when nothing is selected.
    pub(crate) fn select_previous(&mut self) {
        let target = {
            let rows = self.visible_rows();
            let index = match Self::selected_index(&rows, self.selected()) {
                Some(index) => index.saturating_sub(1),
                None => 0,
            };
            rows.get(index).map(|row| row.node.id.clone())
        };
        if let Some(id) = target {
            self.selected = Some(id);
        }
    }

    /// Down: selects the next visible row; the first row when nothing is selected.
    pub(crate) fn select_next(&mut self) {
        let target = {
            let rows = self.visible_rows();
            let index = match Self::selected_index(&rows, self.selected()) {
                Some(index) => (index + 1).min(rows.len().saturating_sub(1)),
                None => 0,
            };
            rows.get(index).map(|row| row.node.id.clone())
        };
        if let Some(id) = target {
            self.selected = Some(id);
        }
    }

    /// Left: collapses the selected node when it is expanded, otherwise selects its parent.
    pub(crate) fn collapse_or_select_parent(&mut self) {
        let step = {
            let rows = self.visible_rows();
            let row =
                Self::selected_index(&rows, self.selected()).and_then(|index| rows.get(index));
            row.map(|row| {
                if row.expanded {
                    Step::Collapse(row.node.id.clone())
                } else {
                    let parent = row.parent.and_then(|parent| rows.get(parent));
                    Step::Select(parent.map(|parent| parent.node.id.clone()))
                }
            })
        };
        self.apply(step);
    }

    /// Right: expands the selected node when it is collapsed, otherwise selects its first
    /// child. Does nothing on a request.
    pub(crate) fn expand_or_select_child(&mut self) {
        let step = {
            let rows = self.visible_rows();
            let index = Self::selected_index(&rows, self.selected());
            let row = index.and_then(|index| rows.get(index).map(|row| (index, row)));
            row.filter(|(_, row)| row.node.is_expandable())
                .map(|(index, row)| {
                    if row.expanded {
                        let child = rows
                            .get(index + 1)
                            .filter(|next| next.parent == Some(index));
                        Step::Select(child.map(|child| child.node.id.clone()))
                    } else {
                        Step::Expand(row.node.id.clone())
                    }
                })
        };
        self.apply(step);
    }

    fn apply(&mut self, step: Option<Step>) {
        match step {
            Some(Step::Collapse(id)) => {
                self.expanded.remove(&id);
            }
            Some(Step::Expand(id)) => {
                self.expanded.insert(id);
            }
            Some(Step::Select(Some(id))) => self.selected = Some(id),
            Some(Step::Select(None)) | None => {}
        }
    }

    /// Enter: expands or collapses the selected collection or folder.
    pub(crate) fn toggle_selected(&mut self) {
        if let Some(id) = self.selected.clone() {
            self.toggle(&id);
        }
    }

    /// Expands or collapses the node `id` when it is a collection or folder.
    pub(crate) fn toggle(&mut self, id: &str) {
        let expandable = self
            .visible_rows()
            .iter()
            .any(|row| row.node.id == id && row.node.is_expandable());
        if expandable && !self.expanded.remove(id) {
            self.expanded.insert(id.to_owned());
        }
    }
}

#[cfg(test)]
#[path = "tree_model_tests.rs"]
mod tests;
