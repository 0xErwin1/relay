//! Scanning the layout into a plan and assembling the tree from parsed documents.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use relay_core::model::{CollectionNode, EnvironmentNode, FolderNode, Item, RequestNode, Scripts};

use super::{Document, Kind, Reader};
use crate::error::{StoreError, Warning, WarningKind};
use crate::layout::{
    COLLECTION_FILE, COLLECTIONS, ENV_SUFFIX, ENVIRONMENTS, EXAMPLES_SUFFIX, FOLDER_FILE,
    REQUEST_SUFFIX, ScriptFiles, is_reserved_request_slug, script_files,
};
use crate::save::io_error;

/// The entity files found on disk, shaped as the tree they form.
#[derive(Debug)]
pub(super) struct Plan {
    environments: Vec<(String, PathBuf)>,
    collections: Vec<Container>,
}

/// A collection or folder directory.
#[derive(Debug)]
struct Container {
    slug: String,
    file: PathBuf,
    scripts: ScriptFiles,
    children: Vec<Child>,
}

#[derive(Debug)]
enum Child {
    Folder(Container),
    Request {
        slug: String,
        file: PathBuf,
        scripts: ScriptFiles,
    },
}

/// Scans `root` for entity files.
pub(super) fn scan(root: &Path, warnings: &mut Vec<Warning>) -> Result<Plan, StoreError> {
    let mut environments = Vec::new();
    for (name, path) in entries(&root.join(ENVIRONMENTS))? {
        if let Some(slug) = name
            .strip_suffix(ENV_SUFFIX)
            .filter(|slug| !slug.is_empty())
            && path.is_file()
        {
            environments.push((slug.to_owned(), path));
        }
    }
    let mut collections = Vec::new();
    for (name, path) in entries(&root.join(COLLECTIONS))? {
        if !path.is_dir() {
            continue;
        }
        match container(name, &path, "collection", COLLECTION_FILE, warnings)? {
            Some(collection) => collections.push(collection),
            None => warn_missing(warnings, path, COLLECTION_FILE),
        }
    }
    Ok(Plan {
        environments,
        collections,
    })
}

/// Reads a collection or folder directory, `None` when it has no entity file.
fn container(
    slug: String,
    dir: &Path,
    stem: &str,
    entity_file: &'static str,
    warnings: &mut Vec<Warning>,
) -> Result<Option<Container>, StoreError> {
    let file = dir.join(entity_file);
    if !file.is_file() {
        return Ok(None);
    }
    let mut children = Vec::new();
    for (name, path) in entries(dir)? {
        if path.is_dir() {
            if name.ends_with(EXAMPLES_SUFFIX) {
                continue;
            }
            match container(name, &path, "folder", FOLDER_FILE, warnings)? {
                Some(folder) => children.push(Child::Folder(folder)),
                None => warn_missing(warnings, path, FOLDER_FILE),
            }
        } else if let Some(slug) = name.strip_suffix(REQUEST_SUFFIX).filter(|s| !s.is_empty()) {
            if is_reserved_request_slug(slug) {
                let warning = Warning {
                    path,
                    kind: WarningKind::ReservedRequestSlug {
                        slug: slug.to_owned(),
                    },
                };
                log::warn!("{warning}");
                warnings.push(warning);
                continue;
            }
            children.push(Child::Request {
                slug: slug.to_owned(),
                scripts: script_files(dir, slug),
                file: path,
            });
        }
    }
    Ok(Some(Container {
        slug,
        file,
        scripts: script_files(dir, stem),
        children,
    }))
}

fn warn_missing(warnings: &mut Vec<Warning>, path: PathBuf, file: &'static str) {
    let warning = Warning {
        path,
        kind: WarningKind::MissingEntityFile { file },
    };
    log::warn!("{warning}");
    warnings.push(warning);
}

/// The visible entries of `dir` sorted by name; an absent directory has none. Hidden
/// entries (`.relay`, `.git`, ...) and names that are not UTF-8 are skipped.
fn entries(dir: &Path) -> Result<Vec<(String, PathBuf)>, StoreError> {
    let read = match fs::read_dir(dir) {
        Ok(read) => read,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => return Err(io_error(dir, source)),
    };
    let mut entries = Vec::new();
    for entry in read {
        let entry = entry.map_err(|source| io_error(dir, source))?;
        if let Ok(name) = entry.file_name().into_string()
            && !name.starts_with('.')
        {
            entries.push((name, entry.path()));
        }
    }
    entries.sort();
    Ok(entries)
}

impl Plan {
    /// Every entity file with its kind.
    pub(super) fn files(&self) -> Vec<(PathBuf, Kind)> {
        fn walk(container: &Container, kind: Kind, out: &mut Vec<(PathBuf, Kind)>) {
            out.push((container.file.clone(), kind));
            for child in &container.children {
                match child {
                    Child::Folder(folder) => walk(folder, Kind::Folder, out),
                    Child::Request { file, .. } => out.push((file.clone(), Kind::Request)),
                }
            }
        }
        let mut out: Vec<_> = self
            .environments
            .iter()
            .map(|(_, path)| (path.clone(), Kind::Environment))
            .collect();
        for collection in &self.collections {
            walk(collection, Kind::Collection, &mut out);
        }
        out
    }

    /// Builds the tree from the parsed documents, reading sidecar scripts.
    pub(super) fn assemble(
        self,
        documents: &mut BTreeMap<PathBuf, Document>,
        reader: &mut Reader,
    ) -> Result<(Vec<EnvironmentNode>, Vec<CollectionNode>), StoreError> {
        let mut environments = Vec::new();
        for (slug, path) in self.environments {
            if let Some(Document::Environment(environment)) = documents.remove(&path) {
                environments.push(EnvironmentNode { slug, environment });
            }
        }
        environments
            .sort_by(|a, b| by_name(&a.environment.name, &a.slug, &b.environment.name, &b.slug));

        let mut collections = Vec::new();
        for container in self.collections {
            let Some(Document::Collection(collection)) = documents.remove(&container.file) else {
                continue;
            };
            let scripts = read_scripts(reader, &container.scripts)?;
            let items = items(container.children, &collection.order, documents, reader)?;
            collections.push(CollectionNode {
                slug: container.slug,
                collection,
                scripts,
                items,
            });
        }
        collections
            .sort_by(|a, b| by_name(&a.collection.name, &a.slug, &b.collection.name, &b.slug));
        Ok((environments, collections))
    }
}

fn items(
    children: Vec<Child>,
    order: &[String],
    documents: &mut BTreeMap<PathBuf, Document>,
    reader: &mut Reader,
) -> Result<Vec<Item>, StoreError> {
    let mut items = Vec::new();
    for child in children {
        match child {
            Child::Folder(container) => {
                let Some(Document::Folder(folder)) = documents.remove(&container.file) else {
                    continue;
                };
                let scripts = read_scripts(reader, &container.scripts)?;
                let children = self::items(container.children, &folder.order, documents, reader)?;
                items.push(Item::Folder(FolderNode {
                    slug: container.slug,
                    folder,
                    scripts,
                    items: children,
                }));
            }
            Child::Request {
                slug,
                file,
                scripts,
            } => {
                let Some(Document::Request(request)) = documents.remove(&file) else {
                    continue;
                };
                let scripts = read_scripts(reader, &scripts)?;
                items.push(Item::Request(RequestNode {
                    slug,
                    request,
                    scripts,
                }));
            }
        }
    }
    Ok(ordered(items, order))
}

/// Children listed in `order` first, in that order; slugs that match nothing are skipped.
/// The rest follow sorted by name, then slug.
fn ordered(mut rest: Vec<Item>, order: &[String]) -> Vec<Item> {
    let mut items = Vec::with_capacity(rest.len());
    for slug in order {
        let (listed, unlisted): (Vec<_>, Vec<_>) =
            rest.into_iter().partition(|item| item.slug() == slug);
        items.extend(listed);
        rest = unlisted;
    }
    rest.sort_by(|a, b| by_name(item_name(a), a.slug(), item_name(b), b.slug()));
    items.extend(rest);
    items
}

fn item_name(item: &Item) -> &str {
    match item {
        Item::Folder(folder) => &folder.folder.name,
        Item::Request(request) => &request.request.name,
    }
}

fn by_name(name_a: &str, slug_a: &str, name_b: &str, slug_b: &str) -> Ordering {
    name_a.cmp(name_b).then_with(|| slug_a.cmp(slug_b))
}

fn read_scripts(reader: &mut Reader, files: &ScriptFiles) -> Result<Scripts, StoreError> {
    Ok(Scripts {
        pre_request: reader.read_script(&files.pre_request)?,
        test: reader.read_script(&files.test)?,
    })
}
