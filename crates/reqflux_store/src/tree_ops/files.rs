//! Moving, copying and removing the files of an entity. Rename and move share
//! [`relocate`]; every path here is composed from slugs the caller validated.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use reqflux_core::model::{FolderNode, Id, Item, RequestNode};
use tempfile::TempDir;

use super::naming::SlugFor;
use crate::error::StoreError;
use crate::layout::{
    COLLECTION_FILE, EXAMPLES_SUFFIX, FOLDER_FILE, PRE_SUFFIX, Parent, REQUEST_SUFFIX, TEST_SUFFIX,
    examples_dir_in, folder_file, request_file, request_file_in, script_files,
    validate_request_slug, validate_slug,
};
use crate::load::Loaded;
use crate::save::{check, io_error, save_document, write_script};

/// A folder directory or a request with its sidecars.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Shape {
    Folder,
    Request,
}

impl Shape {
    /// Validates the slug of an entity of this shape.
    pub(super) fn validate(self, slug: &str) -> Result<(), StoreError> {
        match self {
            Self::Folder => validate_slug(slug),
            Self::Request => validate_request_slug(slug),
        }
    }

    /// `folder.toml` or `<slug>.req.toml` of `slug` in `parent`. Raw: the parent and
    /// `slug` must already have been validated.
    pub(super) fn file(self, root: &Path, parent: &Parent, slug: &str) -> PathBuf {
        match self {
            Self::Folder => folder_file(root, parent, slug),
            Self::Request => request_file(root, parent, slug),
        }
    }

    /// What the slug names, for the free-slug check.
    pub(super) fn slug_for(self) -> SlugFor {
        match self {
            Self::Folder => SlugFor::Container,
            Self::Request => SlugFor::Request,
        }
    }
}

/// Moves the files of `from_slug` in `from_dir` to `to_slug` in `to_dir`: the folder
/// directory, or the request file first and then its sidecars and examples.
pub(super) fn relocate(
    shape: Shape,
    from_dir: &Path,
    from_slug: &str,
    to_dir: &Path,
    to_slug: &str,
) -> Result<(), StoreError> {
    match shape {
        Shape::Folder => rename(&from_dir.join(from_slug), &to_dir.join(to_slug)),
        Shape::Request => {
            rename(
                &request_file_in(from_dir, from_slug),
                &request_file_in(to_dir, to_slug),
            )?;
            let from = script_files(from_dir, from_slug);
            let to = script_files(to_dir, to_slug);
            rename_if_present(&from.pre_request, &to.pre_request)?;
            rename_if_present(&from.test, &to.test)?;
            rename_if_present(
                &examples_dir_in(from_dir, from_slug),
                &examples_dir_in(to_dir, to_slug),
            )
        }
    }
}

pub(super) fn rename(from: &Path, to: &Path) -> Result<(), StoreError> {
    fs::rename(from, to).map_err(|source| io_error(from, source))
}

fn rename_if_present(from: &Path, to: &Path) -> Result<(), StoreError> {
    match fs::rename(from, to) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        result => result.map_err(|source| io_error(from, source)),
    }
}

/// Removes a request file, then its sidecars and examples.
pub(super) fn remove_request(dir: &Path, slug: &str) -> Result<(), StoreError> {
    let file = request_file_in(dir, slug);
    fs::remove_file(&file).map_err(|source| io_error(&file, source))?;
    let scripts = script_files(dir, slug);
    for path in [scripts.pre_request, scripts.test] {
        match fs::remove_file(&path) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => {
                return Err(io_error(&path, error));
            }
            _ => {}
        }
    }
    let examples = examples_dir_in(dir, slug);
    match fs::remove_dir_all(&examples) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(io_error(&examples, error)),
        _ => Ok(()),
    }
}

/// Removes a folder or collection directory. It is first renamed into a hidden staging
/// directory beside it, so it leaves the tree in one rename. When removing the staged copy
/// fails, what is left of it is renamed back to `dir` before the error is returned.
pub(super) fn remove_tree(dir: &Path) -> Result<(), StoreError> {
    let parent = dir.parent().unwrap_or_else(|| Path::new("."));
    let staging = TempDir::new_in(parent).map_err(|source| io_error(parent, source))?;
    let staged = staging.path().join("deleted");
    rename(dir, &staged)?;
    if let Err(source) = fs::remove_dir_all(&staged) {
        let error = io_error(&staged, source);
        if let Err(restore) = fs::rename(&staged, dir) {
            let kept = staging.keep();
            log::error!(
                "could not restore {} from {}: {restore}",
                dir.display(),
                kept.display()
            );
        }
        return Err(error);
    }
    let leftover = staging.path().to_owned();
    staging
        .close()
        .map_err(|source| io_error(&leftover, source))
}

/// Fails with a conflict when a sidecar script of the request `slug` in `dir` changed,
/// appeared or vanished since `loaded` read it.
pub(super) fn check_scripts(loaded: &Loaded, dir: &Path, slug: &str) -> Result<(), StoreError> {
    let scripts = script_files(dir, slug);
    check(&scripts.pre_request, loaded.hash(&scripts.pre_request))?;
    check(&scripts.test, loaded.hash(&scripts.test))
}

/// Fails with a conflict when an entity file or sidecar script anywhere under `dir`
/// differs from what `loaded` read, including one added since. Symlinks are not followed
/// and examples directories are skipped.
pub(super) fn check_tree(loaded: &Loaded, dir: &Path) -> Result<(), StoreError> {
    for entry in fs::read_dir(dir).map_err(|source| io_error(dir, source))? {
        let entry = entry.map_err(|source| io_error(dir, source))?;
        let path = entry.path();
        let kind = entry
            .file_type()
            .map_err(|source| io_error(&path, source))?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if kind.is_dir() {
            if !name.ends_with(EXAMPLES_SUFFIX) {
                check_tree(loaded, &path)?;
            }
        } else if kind.is_file() && is_tracked(&name) {
            check(&path, loaded.hash(&path))?;
        }
    }
    Ok(())
}

/// Whether the loader reads a file with this name: an entity file or a sidecar script.
fn is_tracked(name: &str) -> bool {
    name == FOLDER_FILE
        || name == COLLECTION_FILE
        || [REQUEST_SUFFIX, PRE_SUFFIX, TEST_SUFFIX]
            .iter()
            .any(|suffix| name.ends_with(suffix))
}

/// Fails with a conflict when an entity file or sidecar script of `items` under `dir`
/// changed or vanished since `loaded` read it.
pub(super) fn check_items(loaded: &Loaded, dir: &Path, items: &[Item]) -> Result<(), StoreError> {
    for item in items {
        match item {
            Item::Folder(node) => {
                let inner = dir.join(&node.slug);
                let file = inner.join(FOLDER_FILE);
                check(&file, loaded.hash(&file))?;
                check_scripts(loaded, &inner, "folder")?;
                check_items(loaded, &inner, &node.items)?;
            }
            Item::Request(node) => {
                let file = request_file_in(dir, &node.slug);
                check(&file, loaded.hash(&file))?;
                check_scripts(loaded, dir, &node.slug)?;
            }
        }
    }
    Ok(())
}

/// Writes a copy of the folder `node`, read from `source`, to the new directory `dir`,
/// with a new id for it and every entity below; `name` replaces its display name. The
/// copy is built in a hidden staging directory beside `dir` and renamed into place, so it
/// appears in one rename. Returns the new folder id.
pub(super) fn copy_folder(
    node: &FolderNode,
    name: &str,
    source: &Path,
    dir: &Path,
) -> Result<Id, StoreError> {
    let parent = dir.parent().unwrap_or_else(|| Path::new("."));
    let staging = TempDir::new_in(parent).map_err(|source| io_error(parent, source))?;
    let staged = staging.path().join("copy");
    let id = write_folder(node, Some(name), source, &staged)?;
    rename(&staged, dir)?;
    let leftover = staging.path().to_owned();
    staging
        .close()
        .map_err(|source| io_error(&leftover, source))?;
    Ok(id)
}

fn write_folder(
    node: &FolderNode,
    name: Option<&str>,
    source: &Path,
    dir: &Path,
) -> Result<Id, StoreError> {
    let mut folder = node.folder.clone();
    folder.id = Id::new();
    if let Some(name) = name {
        name.clone_into(&mut folder.name);
    }
    save_document(&dir.join(FOLDER_FILE), &folder, None)?;
    let scripts = script_files(dir, "folder");
    write_script(&scripts.pre_request, node.scripts.pre_request.as_deref())?;
    write_script(&scripts.test, node.scripts.test.as_deref())?;
    for item in &node.items {
        match item {
            Item::Folder(child) => {
                validate_slug(&child.slug)?;
                write_folder(
                    child,
                    None,
                    &source.join(&child.slug),
                    &dir.join(&child.slug),
                )?;
            }
            Item::Request(child) => {
                validate_request_slug(&child.slug)?;
                copy_request(child, None, source, dir, &child.slug)?;
            }
        }
    }
    Ok(folder.id)
}

/// Writes a copy of the request `node`, read from `source`, as `slug` in `dir` with a new
/// id: sidecars and examples first, the request file last, so a failure leaves only files
/// the loader ignores. Returns the new id.
pub(super) fn copy_request(
    node: &RequestNode,
    name: Option<&str>,
    source: &Path,
    dir: &Path,
    slug: &str,
) -> Result<Id, StoreError> {
    let mut request = node.request.clone();
    request.id = Id::new();
    if let Some(name) = name {
        name.clone_into(&mut request.name);
    }
    let scripts = script_files(dir, slug);
    write_script(&scripts.pre_request, node.scripts.pre_request.as_deref())?;
    write_script(&scripts.test, node.scripts.test.as_deref())?;
    let examples = examples_dir_in(source, &node.slug);
    if examples.is_dir() {
        copy_dir(&examples, &examples_dir_in(dir, slug))?;
    }
    save_document(&request_file_in(dir, slug), &request, None)?;
    Ok(request.id)
}

/// Copies the regular files and directories under `from` to `to`; symlinks are skipped
/// so a copy never reads outside the workspace.
fn copy_dir(from: &Path, to: &Path) -> Result<(), StoreError> {
    fs::create_dir_all(to).map_err(|source| io_error(to, source))?;
    for entry in fs::read_dir(from).map_err(|source| io_error(from, source))? {
        let entry = entry.map_err(|source| io_error(from, source))?;
        let path = entry.path();
        let kind = entry
            .file_type()
            .map_err(|source| io_error(&path, source))?;
        let target = to.join(entry.file_name());
        if kind.is_dir() {
            copy_dir(&path, &target)?;
        } else if kind.is_file() {
            fs::copy(&path, &target).map_err(|source| io_error(&path, source))?;
        }
    }
    Ok(())
}
