//! Loading a workspace directory.
//!
//! Loading runs in three passes:
//!
//! 1. **Scan** the layout (S4) into a plan of entity files, skipping hidden entries, unknown
//!    files and `*.examples` directories silently, and warning about a directory that lacks
//!    its `collection.toml` or `folder.toml`, that is a symlink (which is not followed,
//!    so a hand-edited workspace cannot point loading outside the root), or that holds a
//!    request file and a folder directory sharing a slug (`SharedSlug`): both still load,
//!    but `Collection::order` holds slugs only, so an entry with the slug matches both.
//! 2. **Parse** every entity file in sorted path order (component-wise), which is the load
//!    order duplicate ids are resolved in (S8): the first file keeps an id, a later one gets a
//!    fresh id in memory only. Secret values are dropped with a warning (S10).
//! 3. **Assemble** the tree, reading sidecar scripts and ordering children (S5).
//!
//! Loading never writes. Scan warnings come first, then parse warnings in load order.

mod scan;

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use reqflux_core::model::{
    Collection, Environment, Folder, Id, Request, Workspace, WorkspaceManifest,
};
use serde::de::DeserializeOwned;

use crate::FORMAT;
use crate::error::{StoreError, Warning, WarningKind};
use crate::hash::ContentHash;
use crate::layout::manifest_file;
use crate::save::io_error;

/// A loaded workspace.
#[derive(Debug, Clone, PartialEq)]
pub struct Loaded {
    /// The workspace tree.
    pub workspace: Workspace,
    /// Non-fatal problems, in the order they were found.
    pub warnings: Vec<Warning>,
    /// The hash of every file read, keyed by its path under the root given to [`load`].
    /// Pass the entry of a file as `expected` when saving it.
    pub hashes: HashMap<PathBuf, ContentHash>,
}

impl Loaded {
    /// The hash of `path` as it was read, `None` when the load did not read it.
    #[must_use]
    pub fn hash(&self, path: &Path) -> Option<ContentHash> {
        self.hashes.get(path).copied()
    }
}

/// Loads the workspace at `root`.
///
/// # Errors
///
/// Fails when `reqflux.toml` is missing or declares an unsupported format (S6), when a file
/// cannot be read, or when an entity file does not parse. Every error names its path.
pub fn load(root: &Path) -> Result<Loaded, StoreError> {
    let mut reader = Reader::default();
    let manifest_path = manifest_file(root);
    let manifest_text = reader.read(&manifest_path)?;
    check_format(&manifest_path, &manifest_text)?;
    let manifest: WorkspaceManifest = parse(&manifest_path, &manifest_text)?;

    let plan = scan::scan(root, &mut reader.warnings)?;
    let mut documents = BTreeMap::new();
    documents.insert(manifest_path.clone(), Document::Manifest(manifest));
    for (path, kind) in plan.files() {
        let text = reader.read(&path)?;
        documents.insert(path.clone(), kind.parse(&path, &text)?);
    }
    reader.resolve(&mut documents);

    let manifest = match documents.remove(&manifest_path) {
        Some(Document::Manifest(manifest)) => manifest,
        _ => return Err(io_error(&manifest_path, io::ErrorKind::NotFound.into())),
    };
    let (environments, collections) = plan.assemble(&mut documents, &mut reader)?;
    Ok(Loaded {
        workspace: Workspace {
            manifest,
            environments,
            collections,
        },
        warnings: reader.warnings,
        hashes: reader.hashes,
    })
}

/// Reads the format with a lenient parse first, so a newer format with keys this version
/// does not understand reports the version and not a parse error.
fn check_format(path: &Path, text: &str) -> Result<(), StoreError> {
    let table: toml::Table = parse(path, text)?;
    let unsupported = || StoreError::UnsupportedFormat {
        path: path.to_owned(),
    };
    let format = table
        .get("format")
        .and_then(toml::Value::as_integer)
        .ok_or_else(unsupported)?;
    match format.cmp(&i64::from(FORMAT)) {
        std::cmp::Ordering::Greater => Err(StoreError::NewerFormat { found: format }),
        std::cmp::Ordering::Less => Err(unsupported()),
        std::cmp::Ordering::Equal => Ok(()),
    }
}

fn parse<T: DeserializeOwned>(path: &Path, text: &str) -> Result<T, StoreError> {
    toml::from_str(text).map_err(|error| StoreError::Parse {
        path: path.to_owned(),
        location: crate::error::toml_error_location(text, &error),
    })
}

/// The kind of entity file the scan found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Environment,
    Collection,
    Folder,
    Request,
}

impl Kind {
    fn parse(self, path: &Path, text: &str) -> Result<Document, StoreError> {
        Ok(match self {
            Self::Environment => Document::Environment(parse(path, text)?),
            Self::Collection => Document::Collection(parse(path, text)?),
            Self::Folder => Document::Folder(parse(path, text)?),
            Self::Request => Document::Request(parse(path, text)?),
        })
    }
}

/// A parsed entity file.
#[derive(Debug)]
enum Document {
    Manifest(WorkspaceManifest),
    Environment(Environment),
    Collection(Collection),
    Folder(Folder),
    Request(Request),
}

impl Document {
    fn id_mut(&mut self) -> &mut Id {
        match self {
            Self::Manifest(document) => &mut document.id,
            Self::Environment(document) => &mut document.id,
            Self::Collection(document) => &mut document.id,
            Self::Folder(document) => &mut document.id,
            Self::Request(document) => &mut document.id,
        }
    }

    fn strip_secret_values(&mut self) -> Vec<String> {
        match self {
            Self::Manifest(document) => document.strip_secret_values(),
            Self::Environment(document) => document.strip_secret_values(),
            Self::Collection(document) => document.strip_secret_values(),
            Self::Folder(_) | Self::Request(_) => Vec::new(),
        }
    }
}

/// Reads files, recording their hashes, and collects warnings.
#[derive(Debug, Default)]
struct Reader {
    warnings: Vec<Warning>,
    hashes: HashMap<PathBuf, ContentHash>,
}

impl Reader {
    fn read(&mut self, path: &Path) -> Result<String, StoreError> {
        let bytes = fs::read(path).map_err(|source| io_error(path, source))?;
        self.hashes.insert(path.to_owned(), ContentHash::of(&bytes));
        String::from_utf8(bytes)
            .map_err(|error| io_error(path, io::Error::new(io::ErrorKind::InvalidData, error)))
    }

    /// Reads an optional sidecar script. An absent or empty file is no script.
    fn read_script(&mut self, path: &Path) -> Result<Option<String>, StoreError> {
        match self.read(path) {
            Ok(text) => Ok(Some(text).filter(|text| !text.is_empty())),
            Err(StoreError::Io { source, .. }) if source.kind() == io::ErrorKind::NotFound => {
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }

    /// Resolves duplicate ids and strips secret values in load order (sorted paths).
    fn resolve(&mut self, documents: &mut BTreeMap<PathBuf, Document>) {
        let mut owners: HashMap<Id, PathBuf> = HashMap::new();
        for (path, document) in documents.iter_mut() {
            let id = document.id_mut();
            if let Some(other) = owners.get(id) {
                let kind = WarningKind::DuplicateId {
                    id: *id,
                    other: other.clone(),
                };
                self.warn(path, kind);
                *id = fresh_id(&owners);
            }
            owners.insert(*id, path.clone());
            for key in document.strip_secret_values() {
                self.warn(path, WarningKind::SecretValue { key });
            }
        }
    }

    fn warn(&mut self, path: &Path, kind: WarningKind) {
        log::warn!(
            "{}",
            Warning {
                path: path.to_owned(),
                kind: kind.clone()
            }
        );
        self.warnings.push(Warning {
            path: path.to_owned(),
            kind,
        });
    }
}

/// A new id no file uses yet.
fn fresh_id(owners: &HashMap<Id, PathBuf>) -> Id {
    loop {
        let id = Id::new();
        if !owners.contains_key(&id) {
            return id;
        }
    }
}
