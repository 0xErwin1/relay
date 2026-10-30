//! The in-memory workspace tree the store assembles from the per-file documents.
//!
//! Each node pairs a document with the data that lives outside it: its slug (the file or
//! directory name that parent `order` lists refer to), its sidecar scripts and its children.
//! The engine, the UI and MCP share this tree; the store maps it back to files.

use super::documents::{Collection, Environment, Folder, Request, WorkspaceManifest};

/// A loaded workspace.
#[derive(Debug, Clone, PartialEq)]
pub struct Workspace {
    /// `reqflux.toml`.
    pub manifest: WorkspaceManifest,
    /// The workspace environments.
    pub environments: Vec<EnvironmentNode>,
    /// The workspace collections.
    pub collections: Vec<CollectionNode>,
}

/// An environment and its file slug.
#[derive(Debug, Clone, PartialEq)]
pub struct EnvironmentNode {
    /// The file stem before `.env.toml`.
    pub slug: String,
    /// The environment document.
    pub environment: Environment,
}

/// Pre-request and test scripts, stored in sidecar `.js` files.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Scripts {
    /// The pre-request script (`*.pre.js`).
    pub pre_request: Option<String>,
    /// The test script (`*.test.js`).
    pub test: Option<String>,
}

/// A collection with its scripts and children.
#[derive(Debug, Clone, PartialEq)]
pub struct CollectionNode {
    /// The collection directory name.
    pub slug: String,
    /// `collection.toml`.
    pub collection: Collection,
    /// `collection.pre.js` and `collection.test.js`.
    pub scripts: Scripts,
    /// Children in display order.
    pub items: Vec<Item>,
}

/// A folder with its scripts and children.
#[derive(Debug, Clone, PartialEq)]
pub struct FolderNode {
    /// The folder directory name.
    pub slug: String,
    /// `folder.toml`.
    pub folder: Folder,
    /// `folder.pre.js` and `folder.test.js`.
    pub scripts: Scripts,
    /// Children in display order.
    pub items: Vec<Item>,
}

/// A request with its scripts.
#[derive(Debug, Clone, PartialEq)]
pub struct RequestNode {
    /// The file stem before `.req.toml`.
    pub slug: String,
    /// The request document.
    pub request: Request,
    /// `<slug>.pre.js` and `<slug>.test.js`.
    pub scripts: Scripts,
}

/// A child of a collection or folder.
#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    /// A nested folder.
    Folder(FolderNode),
    /// A request.
    Request(RequestNode),
}

impl Item {
    /// The slug parent `order` lists refer to.
    #[must_use]
    pub fn slug(&self) -> &str {
        match self {
            Self::Folder(folder) => &folder.slug,
            Self::Request(request) => &request.slug,
        }
    }
}

fn requests_in(items: &[Item]) -> Box<dyn Iterator<Item = &RequestNode> + '_> {
    Box::new(
        items
            .iter()
            .flat_map(|item| -> Box<dyn Iterator<Item = &RequestNode> + '_> {
                match item {
                    Item::Folder(folder) => requests_in(&folder.items),
                    Item::Request(request) => Box::new(std::iter::once(request)),
                }
            }),
    )
}

impl CollectionNode {
    /// Every request of the collection, depth first in display order.
    pub fn requests(&self) -> impl Iterator<Item = &RequestNode> {
        requests_in(&self.items)
    }
}

impl FolderNode {
    /// Every request of the folder, depth first in display order.
    pub fn requests(&self) -> impl Iterator<Item = &RequestNode> {
        requests_in(&self.items)
    }
}
