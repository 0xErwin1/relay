//! Duplicate, delete, move and reorder.

use std::path::Path;

use reqflux_core::model::Id;

use super::Placed;
use super::files::{self, Shape};
use super::locate::{Container, Entity, find, find_container};
use super::naming::{SlugFor, ensure_free, numbered};
use crate::error::StoreError;
use crate::layout::{collection_dir, collection_file, validate_slug};
use crate::load::Loaded;
use crate::save::check;

/// A folder or request found by id, with the container that holds it.
struct Child<'a> {
    parent: Container<'a>,
    shape: Shape,
    slug: &'a str,
}

impl<'a> Child<'a> {
    /// The folder or request `id`; a collection is refused for `operation`.
    fn find(loaded: &'a Loaded, id: Id, operation: &'static str) -> Result<Self, StoreError> {
        let child = match find(&loaded.workspace, id)? {
            Entity::Collection(_) => return Err(StoreError::CollectionOperation { operation }),
            Entity::Folder { parent, node } => Self {
                parent,
                shape: Shape::Folder,
                slug: &node.slug,
            },
            Entity::Request { parent, node } => Self {
                parent,
                shape: Shape::Request,
                slug: &node.slug,
            },
        };
        child.parent.validate()?;
        child.shape.validate(child.slug)?;
        Ok(child)
    }

    /// Fails with a conflict when the child's entity file or its parent's changed.
    fn check(&self, root: &Path, loaded: &Loaded) -> Result<(), StoreError> {
        let file = self.shape.file(root, &self.parent.inner, self.slug);
        check(&file, loaded.hash(&file))?;
        self.parent.check(root, loaded)
    }

    /// The parent's order without this child.
    fn order_without(&self) -> Vec<String> {
        let mut order = self.parent.order();
        order.retain(|slug| slug != self.slug);
        order
    }
}

/// Duplicates the folder (recursively) or request `id` next to the original as
/// `<name> copy` (then `<name> copy 2`, ...), with sidecars and examples and a new id for
/// every copied entity, inserted right after the original in the parent's `order`.
///
/// # Errors
///
/// [`StoreError::CollectionOperation`] for a collection, [`StoreError::NotFound`], a
/// conflict when the parent file changed since `loaded` read it, or an IO error.
pub fn duplicate(root: &Path, loaded: &Loaded, id: Id) -> Result<Placed, StoreError> {
    let entity = find(&loaded.workspace, id)?;
    let child = Child::find(loaded, id, "duplicated")?;
    child.parent.check(root, loaded)?;
    let dir = child.parent.dir(root);
    let (new_id, slug) = match entity {
        Entity::Folder { node, .. } => {
            let base = format!("{} copy", node.folder.name);
            let (name, slug) = numbered(&dir, &base, SlugFor::Container)?;
            let source = dir.join(&node.slug);
            (
                files::copy_folder(node, &name, &source, &dir.join(&slug))?,
                slug,
            )
        }
        Entity::Request { node, .. } => {
            let base = format!("{} copy", node.request.name);
            let (name, slug) = numbered(&dir, &base, SlugFor::Request)?;
            (
                files::copy_request(node, Some(&name), &dir, &dir, &slug)?,
                slug,
            )
        }
        Entity::Collection(_) => {
            return Err(StoreError::CollectionOperation {
                operation: "duplicated",
            });
        }
    };
    let mut order = child.parent.order();
    let after = order
        .iter()
        .position(|entry| entry == child.slug)
        .map_or(order.len(), |index| index + 1);
    order.insert(after, slug.clone());
    child.parent.save_order(root, loaded, order)?;
    Ok(Placed {
        id: new_id,
        path: child.shape.file(root, &child.parent.inner, &slug),
    })
}

/// Deletes the entity `id`: a request with its sidecars and examples, or a folder or
/// collection with everything under it, and the parent's `order` entry.
///
/// # Errors
///
/// [`StoreError::NotFound`], a conflict when the entity file, a file under it or the
/// parent file changed since `loaded` read it, or an IO error.
pub fn delete(root: &Path, loaded: &Loaded, id: Id) -> Result<(), StoreError> {
    if let Entity::Collection(node) = find(&loaded.workspace, id)? {
        validate_slug(&node.slug)?;
        let file = collection_file(root, &node.slug);
        check(&file, loaded.hash(&file))?;
        let dir = collection_dir(root, &node.slug);
        files::check_items(loaded, &dir, &node.items)?;
        return files::remove_tree(&dir);
    }
    let child = Child::find(loaded, id, "deleted")?;
    child.check(root, loaded)?;
    let dir = child.parent.dir(root);
    match find(&loaded.workspace, id)? {
        Entity::Folder { node, .. } => {
            let folder = dir.join(child.slug);
            files::check_items(loaded, &folder, &node.items)?;
            files::remove_tree(&folder)?;
        }
        _ => files::remove_request(&dir, child.slug)?,
    }
    child
        .parent
        .save_order(root, loaded, child.order_without())
        .map(drop)
}

/// Moves the folder or request `id` into the collection or folder `parent` at `index`
/// among its children (clamped to the end), carrying sidecars, examples and children. The
/// slug is kept; within the same parent this is [`reorder`].
///
/// # Errors
///
/// [`StoreError::CollectionOperation`] for a collection, [`StoreError::NotFound`],
/// [`StoreError::NotAContainer`], [`StoreError::MoveIntoDescendant`] for a folder moved
/// into itself or below, [`StoreError::SlugTaken`] when the target holds the slug, a
/// conflict when the entity, source or target parent file changed since `loaded` read it,
/// or an IO error.
pub fn move_item(
    root: &Path,
    loaded: &Loaded,
    id: Id,
    parent: Id,
    index: usize,
) -> Result<Placed, StoreError> {
    let child = Child::find(loaded, id, "moved")?;
    let target = find_container(&loaded.workspace, parent)?;
    if child.shape == Shape::Folder {
        let own = child.parent.inner.child(child.slug);
        if target.inner.collection == own.collection
            && target.inner.folders.starts_with(&own.folders)
        {
            return Err(StoreError::MoveIntoDescendant);
        }
    }
    if target.id() == child.parent.id() {
        return reorder(root, loaded, id, index).map(|()| Placed {
            id,
            path: child.shape.file(root, &child.parent.inner, child.slug),
        });
    }
    target.validate()?;
    child.check(root, loaded)?;
    target.check(root, loaded)?;
    let to = target.dir(root);
    ensure_free(&to, child.slug)?;
    files::relocate(
        child.shape,
        &child.parent.dir(root),
        child.slug,
        &to,
        child.slug,
    )?;
    let mut order = target.order();
    order.insert(index.min(order.len()), child.slug.to_owned());
    target.save_order(root, loaded, order)?;
    child
        .parent
        .save_order(root, loaded, child.order_without())?;
    Ok(Placed {
        id,
        path: child.shape.file(root, &target.inner, child.slug),
    })
}

/// Moves the folder or request `id` to `index` among its siblings (clamped to the end),
/// rewriting only the parent's `order`.
///
/// # Errors
///
/// [`StoreError::CollectionOperation`] for a collection, [`StoreError::NotFound`], a
/// conflict when the parent file changed since `loaded` read it, or an IO error.
pub fn reorder(root: &Path, loaded: &Loaded, id: Id, index: usize) -> Result<(), StoreError> {
    let child = Child::find(loaded, id, "reordered")?;
    child.parent.check(root, loaded)?;
    let mut order = child.order_without();
    order.insert(index.min(order.len()), child.slug.to_owned());
    child.parent.save_order(root, loaded, order).map(drop)
}
