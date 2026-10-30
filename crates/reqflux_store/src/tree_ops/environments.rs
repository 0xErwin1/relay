//! Creating, renaming and deleting environments (variables-editor S3).
//!
//! An environment is one file, `environments/<slug>.env.toml`, so each operation is a
//! single write, rename or removal after its checks. Deleting an environment leaves the
//! keyring entries of its secrets in place (keyring-secrets S7).

use std::fs;
use std::path::Path;

use reqflux_core::model::{Environment, EnvironmentNode, Id};

use super::Placed;
use super::files;
use super::naming::{SlugFor, ensure_free, named, numbered};
use crate::error::StoreError;
use crate::layout::{environment_file, environments_dir, validate_slug};
use crate::load::Loaded;
use crate::save::{check, io_error, save_document};

/// Creates the environment `New environment` (then `New environment 2`, ...) with a new id
/// and no variables.
///
/// # Errors
///
/// An IO error naming the path.
pub fn create_environment(root: &Path) -> Result<Placed, StoreError> {
    let (name, slug) = numbered(
        &environments_dir(root),
        "New environment",
        SlugFor::Environment,
    )?;
    let environment = Environment {
        id: Id::new(),
        name,
        variables: Vec::new(),
        extensions: toml::Table::new(),
    };
    let path = environment_file(root, &slug);
    save_document(&path, &environment, None)?;
    Ok(Placed {
        id: environment.id,
        path,
    })
}

/// Renames the environment `id`: its display name and its slug, so its file follows the
/// name. The id is kept.
///
/// # Errors
///
/// [`StoreError::EmptyName`], [`StoreError::SlugTaken`] when another environment holds the
/// new slug, [`StoreError::UnknownEnvironment`], a conflict when the file changed since
/// `loaded` read it, or an IO error.
pub fn rename_environment(
    root: &Path,
    loaded: &Loaded,
    id: Id,
    name: &str,
) -> Result<Placed, StoreError> {
    let node = find(loaded, id)?;
    let (name, slug) = named(name, SlugFor::Environment)?;
    validate_slug(&node.slug)?;
    let old = environment_file(root, &node.slug);
    let expected = loaded.hash(&old);
    check(&old, expected)?;
    if slug != node.slug {
        ensure_free(&environments_dir(root), &slug, SlugFor::Environment)?;
        files::rename(&old, &environment_file(root, &slug))?;
    }
    let mut environment = node.environment.clone();
    environment.name = name;
    let path = environment_file(root, &slug);
    save_document(&path, &environment, expected)?;
    Ok(Placed { id, path })
}

/// Deletes the file of the environment `id`.
///
/// # Errors
///
/// [`StoreError::UnknownEnvironment`], a conflict when the file changed since `loaded` read
/// it, or an IO error.
pub fn delete_environment(root: &Path, loaded: &Loaded, id: Id) -> Result<(), StoreError> {
    let node = find(loaded, id)?;
    validate_slug(&node.slug)?;
    let path = environment_file(root, &node.slug);
    check(&path, loaded.hash(&path))?;
    fs::remove_file(&path).map_err(|source| io_error(&path, source))
}

fn find(loaded: &Loaded, id: Id) -> Result<&EnvironmentNode, StoreError> {
    loaded
        .workspace
        .environments
        .iter()
        .find(|node| node.environment.id == id)
        .ok_or(StoreError::UnknownEnvironment { id })
}
