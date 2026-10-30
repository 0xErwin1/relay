//! Writing a Postman import into a workspace (S9).
//!
//! [`postman`] maps the JSON documents with [`relay_postman`](crate) (no IO) and then
//! writes the entities through `relay_store`, atomically, after checking that nothing it
//! would write already exists: an import never overwrites a collection, folder, request
//! or environment that is already there.

use std::path::{Path, PathBuf};

use relay_core::model::{Id, Item, WorkspaceManifest};
use relay_store::{
    Parent, ScriptHashes, ScriptOwner, collection_file, environment_file, folder_file,
    is_reserved_request_slug, manifest_file, request_file, save_collection, save_environment,
    save_folder, save_manifest, save_request, save_scripts,
};
use serde_json::Value;

use crate::error::ImportError;

/// What an import wrote (S9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportSummary {
    /// The workspace root the import wrote to.
    pub root: PathBuf,
    /// The slug of the collection directory that was written.
    pub collection: String,
    /// Folders written, at any depth.
    pub folders: usize,
    /// Requests written, at any depth.
    pub requests: usize,
    /// Environments written.
    pub environments: usize,
    /// Non-fatal problems found while importing. Store saves are atomic and fail hard,
    /// so an import reports none today; the field keeps the summary stable when the
    /// store grows save-time warnings.
    pub warnings: Vec<String>,
}

/// Imports a Postman collection v2.1 document and any number of environment documents
/// into the workspace at `root` (S9).
///
/// When `root` has no `relay.toml`, a workspace manifest is created first, named after
/// the collection. An existing workspace keeps its manifest and gains the collection
/// beside what is there. Every target file is checked before anything is written — the
/// entities, the two sidecar scripts of every collection, folder and request, and the
/// request slugs against the reserved ones — so a collision fails with
/// [`ImportError::Exists`] (a reserved slug with [`ImportError::Store`]) and leaves the
/// workspace untouched.
///
/// # Errors
///
/// [`ImportError::Postman`] when a document does not map (S8), [`ImportError::Exists`]
/// when a target file already exists, [`ImportError::Store`] when the store cannot write.
pub fn postman(
    root: &Path,
    collection: &Value,
    environments: &[Value],
) -> Result<ImportSummary, ImportError> {
    let imported = relay_postman::collection(collection)?;
    let mut taken = imported.slugs.clone();
    let mut environment_nodes = Vec::with_capacity(environments.len());
    for value in environments {
        environment_nodes.push(relay_postman::environment_in(value, &mut taken)?);
    }
    let node = imported.collection;
    let manifest_path = manifest_file(root);
    let create_manifest = !manifest_path.exists();

    let mut targets = Vec::new();
    if create_manifest {
        targets.push(manifest_path.clone());
    }
    targets.push(collection_file(root, &node.slug));
    targets.extend(sidecars(&ScriptOwner::Collection(node.slug.clone()), root));
    collect_targets(
        root,
        &node.items,
        &Parent::collection(&node.slug),
        &mut targets,
    )?;
    for environment in &environment_nodes {
        targets.push(environment_file(root, &environment.slug));
    }
    for target in &targets {
        if target.exists() {
            return Err(ImportError::Exists {
                path: target.clone(),
            });
        }
    }

    if create_manifest {
        let manifest = WorkspaceManifest {
            id: Id::new(),
            name: node.collection.name.clone(),
            format: relay_store::FORMAT,
            globals: Vec::new(),
            extensions: toml::Table::new(),
        };
        save_manifest(root, &manifest, None).map_err(|error| store_error(&manifest_path, error))?;
    }
    let mut summary = ImportSummary {
        root: root.to_owned(),
        collection: node.slug.clone(),
        folders: 0,
        requests: 0,
        environments: environment_nodes.len(),
        warnings: Vec::new(),
    };
    save_collection(root, &node.slug, &node.collection, None)
        .map_err(|error| store_error(&collection_file(root, &node.slug), error))?;
    save_scripts(
        root,
        &ScriptOwner::Collection(node.slug.clone()),
        &node.scripts,
        ScriptHashes::default(),
    )
    .map_err(|error| store_error(&collection_file(root, &node.slug), error))?;
    write_items(
        root,
        &Parent::collection(&node.slug),
        &node.items,
        &mut summary,
    )?;
    for environment in &environment_nodes {
        save_environment(root, &environment.slug, &environment.environment, None)
            .map_err(|error| store_error(&environment_file(root, &environment.slug), error))?;
    }
    Ok(summary)
}

fn write_items(
    root: &Path,
    parent: &Parent,
    items: &[Item],
    summary: &mut ImportSummary,
) -> Result<(), ImportError> {
    for item in items {
        match item {
            Item::Folder(folder) => {
                let file = folder_file(root, parent, &folder.slug);
                save_folder(root, parent, &folder.slug, &folder.folder, None)
                    .map_err(|error| store_error(&file, error))?;
                save_scripts(
                    root,
                    &ScriptOwner::Folder(parent.clone(), folder.slug.clone()),
                    &folder.scripts,
                    ScriptHashes::default(),
                )
                .map_err(|error| store_error(&file, error))?;
                write_items(root, &parent.child(&folder.slug), &folder.items, summary)?;
                summary.folders += 1;
            }
            Item::Request(request) => {
                let file = request_file(root, parent, &request.slug);
                save_request(root, parent, &request.slug, &request.request, None)
                    .map_err(|error| store_error(&file, error))?;
                save_scripts(
                    root,
                    &ScriptOwner::Request(parent.clone(), request.slug.clone()),
                    &request.scripts,
                    ScriptHashes::default(),
                )
                .map_err(|error| store_error(&file, error))?;
                summary.requests += 1;
            }
        }
    }
    Ok(())
}

/// Collects every file the import would write for `items` under `parent`: the entity
/// files and the two sidecar scripts of each folder and request. A request with a slug
/// the store reserves (`collection`, `folder`) fails here, before anything is written.
fn collect_targets(
    root: &Path,
    items: &[Item],
    parent: &Parent,
    targets: &mut Vec<PathBuf>,
) -> Result<(), ImportError> {
    for item in items {
        match item {
            Item::Folder(folder) => {
                targets.push(folder_file(root, parent, &folder.slug));
                targets.extend(sidecars(
                    &ScriptOwner::Folder(parent.clone(), folder.slug.clone()),
                    root,
                ));
                collect_targets(root, &folder.items, &parent.child(&folder.slug), targets)?;
            }
            Item::Request(request) => {
                let file = request_file(root, parent, &request.slug);
                if is_reserved_request_slug(&request.slug) {
                    return Err(ImportError::Store {
                        path: file,
                        message: format!("request slug `{}` is reserved", request.slug),
                    });
                }
                targets.push(file);
                targets.extend(sidecars(
                    &ScriptOwner::Request(parent.clone(), request.slug.clone()),
                    root,
                ));
            }
        }
    }
    Ok(())
}

/// The two sidecar script paths of `owner`, in the order the store writes them.
fn sidecars(owner: &ScriptOwner, root: &Path) -> [PathBuf; 2] {
    let files = owner.files(root);
    [files.pre_request, files.test]
}

/// Maps a store failure on `target` to an [`ImportError`]. A conflict means the file
/// appeared between the preflight and the write, which is an existing file again.
fn store_error(target: &Path, error: relay_store::StoreError) -> ImportError {
    match error {
        relay_store::StoreError::Conflict { path } => ImportError::Exists { path },
        other => ImportError::Store {
            path: target.to_owned(),
            message: other.to_string(),
        },
    }
}
