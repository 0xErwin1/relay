//! The open documents without GPUI: which requests have a tab, in which order, and which
//! one is active; and where a request sits in the collection tree, for the path bar.

use super::tree_model::{Node, NodeKind};

/// The requests open in document tabs, in tab order, and the active one.
///
/// Invariant: `active` is `None` exactly when `open` is empty, and otherwise one of `open`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Documents {
    open: Vec<String>,
    active: Option<String>,
}

impl Documents {
    /// The open request ids, in tab order.
    pub(crate) fn open_ids(&self) -> &[String] {
        &self.open
    }

    /// The id of the active request.
    pub(crate) fn active(&self) -> Option<&str> {
        self.active.as_deref()
    }

    /// Opens the request `id` in a new tab at the end and activates it, or activates its
    /// tab when it is already open.
    pub(crate) fn open(&mut self, id: &str) {
        if !self.is_open(id) {
            self.open.push(id.to_owned());
        }
        self.active = Some(id.to_owned());
    }

    /// Activates the tab of `id`; does nothing when `id` is not open.
    pub(crate) fn activate(&mut self, id: &str) {
        if self.is_open(id) {
            self.active = Some(id.to_owned());
        }
    }

    /// Closes the tab of `id`. When it was active, its neighbor becomes active: the next
    /// tab, else the previous one, else none.
    pub(crate) fn close(&mut self, id: &str) {
        let Some(index) = self.open.iter().position(|open| open == id) else {
            return;
        };
        self.open.remove(index);
        if self.active() == Some(id) {
            let neighbor = self.open.get(index).or_else(|| {
                index
                    .checked_sub(1)
                    .and_then(|previous| self.open.get(previous))
            });
            self.active = neighbor.cloned();
        }
    }

    fn is_open(&self, id: &str) -> bool {
        self.open.iter().any(|open| open == id)
    }
}

/// Where a request sits: its collection, the folders down to it and its own name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Location {
    /// The collection name.
    pub(crate) collection: String,
    /// The folder names, outermost first.
    pub(crate) folders: Vec<String>,
    /// The request name.
    pub(crate) name: String,
    /// The uppercase HTTP method.
    pub(crate) method: String,
}

/// The location of the request `id` among the collection trees `roots`; `None` when no
/// request has that id.
pub(crate) fn locate(roots: &[Node], id: &str) -> Option<Location> {
    roots.iter().find_map(|collection| {
        let mut folders = Vec::new();
        let (name, method) = find_request(&collection.children, id, &mut folders)?;
        Some(Location {
            collection: collection.name.clone(),
            folders,
            name,
            method,
        })
    })
}

/// The name and method of the request `id` under `nodes`, pushing onto `folders` the names
/// of the folders on the way down to it; `folders` is left as it was when it is not found.
fn find_request(nodes: &[Node], id: &str, folders: &mut Vec<String>) -> Option<(String, String)> {
    nodes.iter().find_map(|node| match &node.kind {
        NodeKind::Request { method } => {
            (node.id == id).then(|| (node.name.clone(), method.clone()))
        }
        NodeKind::Folder | NodeKind::Collection => {
            folders.push(node.name.clone());
            let found = find_request(&node.children, id, folders);
            if found.is_none() {
                folders.pop();
            }
            found
        }
    })
}

#[cfg(test)]
#[path = "documents_tests.rs"]
mod tests;
