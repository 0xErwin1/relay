//! Finding an entity of a loaded workspace by id, with the container that holds it.

use std::path::{Path, PathBuf};

use reqflux_core::model::{CollectionNode, FolderNode, Id, Item, RequestNode, Workspace};

use crate::error::StoreError;
use crate::hash::ContentHash;
use crate::layout::{FOLDER_FILE, Parent, collection_file, validate_parent};
use crate::load::Loaded;
use crate::save::{check, save_document};

/// A collection or folder, with the [`Parent`] its children live in.
#[derive(Debug, Clone)]
pub(super) struct Container<'a> {
    /// The location of the children.
    pub(super) inner: Parent,
    node: ContainerNode<'a>,
}

#[derive(Debug, Clone, Copy)]
enum ContainerNode<'a> {
    Collection(&'a CollectionNode),
    Folder(&'a FolderNode),
}

impl<'a> Container<'a> {
    fn collection(node: &'a CollectionNode) -> Self {
        Self {
            inner: Parent::collection(node.slug.clone()),
            node: ContainerNode::Collection(node),
        }
    }

    fn folder(&self, node: &'a FolderNode) -> Self {
        Self {
            inner: self.inner.child(node.slug.clone()),
            node: ContainerNode::Folder(node),
        }
    }

    pub(super) fn id(&self) -> Id {
        match self.node {
            ContainerNode::Collection(node) => node.collection.id,
            ContainerNode::Folder(node) => node.folder.id,
        }
    }

    pub(super) fn items(&self) -> &'a [Item] {
        match self.node {
            ContainerNode::Collection(node) => &node.items,
            ContainerNode::Folder(node) => &node.items,
        }
    }

    /// Validates every slug of the location, so the paths below are checked.
    pub(super) fn validate(&self) -> Result<(), StoreError> {
        validate_parent(&self.inner)
    }

    /// The directory the children live in.
    pub(super) fn dir(&self, root: &Path) -> PathBuf {
        self.inner.dir(root)
    }

    /// `collection.toml` or `folder.toml`.
    pub(super) fn file(&self, root: &Path) -> PathBuf {
        match self.node {
            ContainerNode::Collection(_) => collection_file(root, &self.inner.collection),
            ContainerNode::Folder(_) => self.dir(root).join(FOLDER_FILE),
        }
    }

    /// Fails with a conflict when the entity file changed since `loaded` read it.
    pub(super) fn check(&self, root: &Path, loaded: &Loaded) -> Result<(), StoreError> {
        let file = self.file(root);
        check(&file, loaded.hash(&file))
    }

    /// The children's slugs in display order, each once.
    pub(super) fn order(&self) -> Vec<String> {
        let mut order: Vec<String> = Vec::new();
        for item in self.items() {
            if !order.iter().any(|slug| slug == item.slug()) {
                order.push(item.slug().to_owned());
            }
        }
        order
    }

    /// Rewrites the entity file with `order`, hash-checked against `loaded`.
    pub(super) fn save_order(
        &self,
        root: &Path,
        loaded: &Loaded,
        order: Vec<String>,
    ) -> Result<ContentHash, StoreError> {
        let file = self.file(root);
        let expected = loaded.hash(&file);
        match self.node {
            ContainerNode::Collection(node) => {
                let mut document = node.collection.clone();
                document.order = order;
                save_document(&file, &document, expected)
            }
            ContainerNode::Folder(node) => {
                let mut document = node.folder.clone();
                document.order = order;
                save_document(&file, &document, expected)
            }
        }
    }
}

/// An entity found by id.
#[derive(Debug, Clone)]
pub(super) enum Entity<'a> {
    Collection(&'a CollectionNode),
    Folder {
        parent: Container<'a>,
        node: &'a FolderNode,
    },
    Request {
        parent: Container<'a>,
        node: &'a RequestNode,
    },
}

/// The entity with `id`.
pub(super) fn find(workspace: &Workspace, id: Id) -> Result<Entity<'_>, StoreError> {
    workspace
        .collections
        .iter()
        .find_map(|node| {
            if node.collection.id == id {
                Some(Entity::Collection(node))
            } else {
                find_in(&Container::collection(node), id)
            }
        })
        .ok_or(StoreError::NotFound { id })
}

fn find_in<'a>(parent: &Container<'a>, id: Id) -> Option<Entity<'a>> {
    parent.items().iter().find_map(|item| match item {
        Item::Request(node) if node.request.id == id => Some(Entity::Request {
            parent: parent.clone(),
            node,
        }),
        Item::Request(_) => None,
        Item::Folder(node) if node.folder.id == id => Some(Entity::Folder {
            parent: parent.clone(),
            node,
        }),
        Item::Folder(node) => find_in(&parent.folder(node), id),
    })
}

/// The collection or folder with `id`.
pub(super) fn find_container(workspace: &Workspace, id: Id) -> Result<Container<'_>, StoreError> {
    match find(workspace, id)? {
        Entity::Collection(node) => Ok(Container::collection(node)),
        Entity::Folder { parent, node } => Ok(parent.folder(node)),
        Entity::Request { .. } => Err(StoreError::NotAContainer { id }),
    }
}
