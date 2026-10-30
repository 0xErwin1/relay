//! Editing the variables of one scope: the globals in `reqflux.toml`, a collection's
//! `collection.toml` or an environment's `.env.toml` (variables-editor S1).
//!
//! An edit has two steps so a caller can act between the checks and the write, as the
//! engine does to move a secret into the keyring: [`plan_variables`] validates the change
//! and checks the file against the hash `loaded` read, writing nothing; [`VariableWrite::write`]
//! then writes the file, hash-checked again, so a write landing in between is a conflict.

use std::path::{Path, PathBuf};

use reqflux_core::model::{Collection, Environment, Id, Variable, WorkspaceManifest};

use crate::error::StoreError;
use crate::hash::ContentHash;
use crate::layout::{collection_file, environment_file, manifest_file, validate_slug};
use crate::load::Loaded;
use crate::save::{check, save_document};

/// The scope whose variables are edited.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScopeFile {
    /// The workspace globals in `reqflux.toml`.
    Globals,
    /// The variables of the collection with this id.
    Collection(Id),
    /// The variables of the environment with this id.
    Environment(Id),
}

/// One change to a scope's variables. Indexes are positions in the scope's list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VariableChange {
    /// Appends a variable.
    Add(Variable),
    /// Replaces the variable at the index.
    Replace(usize, Variable),
    /// Removes the variable at the index.
    Remove(usize),
}

/// A validated, hash-checked scope edit that has not been written yet.
#[derive(Debug, Clone)]
pub struct VariableWrite {
    path: PathBuf,
    document: Document,
    expected: Option<ContentHash>,
}

#[derive(Debug, Clone)]
enum Document {
    Manifest(WorkspaceManifest),
    Collection(Collection),
    Environment(Environment),
}

impl Document {
    fn variables_mut(&mut self) -> &mut Vec<Variable> {
        match self {
            Self::Manifest(manifest) => &mut manifest.globals,
            Self::Collection(collection) => &mut collection.variables,
            Self::Environment(environment) => &mut environment.variables,
        }
    }
}

impl VariableWrite {
    /// The file the edit writes.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Writes the file atomically, refusing with a conflict when it changed since it was
    /// loaded. A secret's value is never written.
    ///
    /// # Errors
    ///
    /// [`StoreError::Conflict`], or an IO or serialization error naming the file.
    pub fn write(self) -> Result<ContentHash, StoreError> {
        match &self.document {
            Document::Manifest(manifest) => save_document(&self.path, manifest, self.expected),
            Document::Collection(collection) => {
                save_document(&self.path, collection, self.expected)
            }
            Document::Environment(environment) => {
                save_document(&self.path, environment, self.expected)
            }
        }
    }
}

/// The variables of `scope` as `loaded` read them.
///
/// # Errors
///
/// [`StoreError::NotFound`] for an unknown collection, [`StoreError::UnknownEnvironment`]
/// for an unknown environment.
pub fn scope_variables(loaded: &Loaded, scope: ScopeFile) -> Result<&[Variable], StoreError> {
    let workspace = &loaded.workspace;
    match scope {
        ScopeFile::Globals => Ok(&workspace.manifest.globals),
        ScopeFile::Collection(id) => workspace
            .collections
            .iter()
            .find(|node| node.collection.id == id)
            .map(|node| node.collection.variables.as_slice())
            .ok_or(StoreError::NotFound { id }),
        ScopeFile::Environment(id) => workspace
            .environments
            .iter()
            .find(|node| node.environment.id == id)
            .map(|node| node.environment.variables.as_slice())
            .ok_or(StoreError::UnknownEnvironment { id }),
    }
}

/// Validates `change` against the variables of `scope` and checks the scope's file against
/// `loaded`, writing nothing.
///
/// # Errors
///
/// [`StoreError::EmptyVariableKey`], [`StoreError::DuplicateVariableKey`] when another
/// variable of the scope has the key, [`StoreError::NoVariable`], an unknown scope as
/// [`scope_variables`], or a conflict when the file changed since `loaded` read it.
pub fn plan_variables(
    root: &Path,
    loaded: &Loaded,
    scope: ScopeFile,
    change: VariableChange,
) -> Result<VariableWrite, StoreError> {
    let (path, mut document) = locate(root, loaded, scope)?;
    let variables = document.variables_mut();
    match change {
        VariableChange::Add(variable) => {
            validate_key(variables, None, &variable.key)?;
            variables.push(without_secret_value(variable));
        }
        VariableChange::Replace(index, variable) => {
            validate_key(variables, Some(index), &variable.key)?;
            let slot = variables
                .get_mut(index)
                .ok_or(StoreError::NoVariable { index })?;
            *slot = without_secret_value(variable);
        }
        VariableChange::Remove(index) => {
            if index >= variables.len() {
                return Err(StoreError::NoVariable { index });
            }
            variables.remove(index);
        }
    }
    let expected = loaded.hash(&path);
    check(&path, expected)?;
    Ok(VariableWrite {
        path,
        document,
        expected,
    })
}

fn locate(
    root: &Path,
    loaded: &Loaded,
    scope: ScopeFile,
) -> Result<(PathBuf, Document), StoreError> {
    let workspace = &loaded.workspace;
    match scope {
        ScopeFile::Globals => Ok((
            manifest_file(root),
            Document::Manifest(workspace.manifest.clone()),
        )),
        ScopeFile::Collection(id) => {
            let node = workspace
                .collections
                .iter()
                .find(|node| node.collection.id == id)
                .ok_or(StoreError::NotFound { id })?;
            validate_slug(&node.slug)?;
            Ok((
                collection_file(root, &node.slug),
                Document::Collection(node.collection.clone()),
            ))
        }
        ScopeFile::Environment(id) => {
            let node = workspace
                .environments
                .iter()
                .find(|node| node.environment.id == id)
                .ok_or(StoreError::UnknownEnvironment { id })?;
            validate_slug(&node.slug)?;
            Ok((
                environment_file(root, &node.slug),
                Document::Environment(node.environment.clone()),
            ))
        }
    }
}

/// Fails when `key` is empty or another variable than the one at `own` has it.
fn validate_key(variables: &[Variable], own: Option<usize>, key: &str) -> Result<(), StoreError> {
    if key.trim().is_empty() {
        return Err(StoreError::EmptyVariableKey);
    }
    let taken = variables
        .iter()
        .enumerate()
        .any(|(index, variable)| Some(index) != own && variable.key == key);
    if taken {
        return Err(StoreError::DuplicateVariableKey {
            key: key.to_owned(),
        });
    }
    Ok(())
}

/// The serializer already skips a secret's value; dropping it here keeps it out of the
/// planned document too.
fn without_secret_value(mut variable: Variable) -> Variable {
    if variable.secret {
        variable.value = None;
    }
    variable
}
