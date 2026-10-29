//! Where each entity lives on disk (S4).
//!
//! ```text
//! <root>/relay.toml
//! <root>/environments/<slug>.env.toml
//! <root>/collections/<slug>/collection.toml   (+ collection.pre.js, collection.test.js)
//! <root>/collections/<slug>/.../<folder>/folder.toml   (+ folder.pre.js, folder.test.js)
//! <root>/collections/<slug>/.../<slug>.req.toml   (+ <slug>.pre.js, <slug>.test.js)
//! ```

use std::path::{Path, PathBuf};

use crate::error::StoreError;

pub(crate) const MANIFEST: &str = "relay.toml";
pub(crate) const ENVIRONMENTS: &str = "environments";
pub(crate) const COLLECTIONS: &str = "collections";
pub(crate) const COLLECTION_FILE: &str = "collection.toml";
pub(crate) const FOLDER_FILE: &str = "folder.toml";
pub(crate) const ENV_SUFFIX: &str = ".env.toml";
pub(crate) const REQUEST_SUFFIX: &str = ".req.toml";
pub(crate) const PRE_SUFFIX: &str = ".pre.js";
pub(crate) const TEST_SUFFIX: &str = ".test.js";
/// Directories reserved for saved response examples (a later feature); skipped silently.
pub(crate) const EXAMPLES_SUFFIX: &str = ".examples";

/// The container of a folder or request: a collection and the folders below it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parent {
    /// The collection slug.
    pub collection: String,
    /// Folder slugs from the collection down, outermost first.
    pub folders: Vec<String>,
}

impl Parent {
    /// The top level of collection `slug`.
    #[must_use]
    pub fn collection(slug: impl Into<String>) -> Self {
        Self {
            collection: slug.into(),
            folders: Vec::new(),
        }
    }

    /// The folder `slug` inside this parent.
    #[must_use]
    pub fn child(&self, slug: impl Into<String>) -> Self {
        let mut folders = self.folders.clone();
        folders.push(slug.into());
        Self {
            collection: self.collection.clone(),
            folders,
        }
    }

    /// The directory of this parent under `root`.
    #[must_use]
    pub fn dir(&self, root: &Path) -> PathBuf {
        let mut dir = collection_dir(root, &self.collection);
        dir.extend(&self.folders);
        dir
    }

    fn validate(&self) -> Result<(), StoreError> {
        validate_slug(&self.collection)?;
        self.folders.iter().try_for_each(|slug| validate_slug(slug))
    }
}

/// The entity whose sidecar scripts are meant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptOwner {
    /// Collection `slug`: `collection.pre.js`, `collection.test.js`.
    Collection(String),
    /// Folder `slug` in a parent: `folder.pre.js`, `folder.test.js`.
    Folder(Parent, String),
    /// Request `slug` in a parent: `<slug>.pre.js`, `<slug>.test.js`.
    Request(Parent, String),
}

/// The two sidecar script paths of an entity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptFiles {
    /// The pre-request script.
    pub pre_request: PathBuf,
    /// The test script.
    pub test: PathBuf,
}

impl ScriptOwner {
    /// The sidecar paths under `root`.
    #[must_use]
    pub fn files(&self, root: &Path) -> ScriptFiles {
        let (dir, stem) = match self {
            Self::Collection(slug) => (collection_dir(root, slug), "collection"),
            Self::Folder(parent, slug) => (parent.dir(root).join(slug), "folder"),
            Self::Request(parent, slug) => (parent.dir(root), slug.as_str()),
        };
        script_files(&dir, stem)
    }

    pub(crate) fn validate(&self) -> Result<(), StoreError> {
        match self {
            Self::Collection(slug) => validate_slug(slug),
            Self::Folder(parent, slug) => {
                parent.validate()?;
                validate_slug(slug)
            }
            Self::Request(parent, slug) => {
                parent.validate()?;
                validate_request_slug(slug)
            }
        }
    }
}

pub(crate) fn script_files(dir: &Path, stem: &str) -> ScriptFiles {
    ScriptFiles {
        pre_request: dir.join(format!("{stem}{PRE_SUFFIX}")),
        test: dir.join(format!("{stem}{TEST_SUFFIX}")),
    }
}

/// `<root>/relay.toml`.
#[must_use]
pub fn manifest_file(root: &Path) -> PathBuf {
    root.join(MANIFEST)
}

/// `<root>/environments/<slug>.env.toml`.
#[must_use]
pub fn environment_file(root: &Path, slug: &str) -> PathBuf {
    root.join(ENVIRONMENTS).join(format!("{slug}{ENV_SUFFIX}"))
}

/// `<root>/collections/<slug>/collection.toml`.
#[must_use]
pub fn collection_file(root: &Path, slug: &str) -> PathBuf {
    collection_dir(root, slug).join(COLLECTION_FILE)
}

/// `<parent dir>/<slug>/folder.toml`.
#[must_use]
pub fn folder_file(root: &Path, parent: &Parent, slug: &str) -> PathBuf {
    parent.dir(root).join(slug).join(FOLDER_FILE)
}

/// `<parent dir>/<slug>.req.toml`.
#[must_use]
pub fn request_file(root: &Path, parent: &Parent, slug: &str) -> PathBuf {
    parent.dir(root).join(format!("{slug}{REQUEST_SUFFIX}"))
}

pub(crate) fn collection_dir(root: &Path, slug: &str) -> PathBuf {
    root.join(COLLECTIONS).join(slug)
}

/// A slug is one visible path component: non-empty, no separator, not hidden.
/// Request slugs that would collide with the sidecar scripts of the containing collection
/// (`collection.pre.js`) or folder (`folder.pre.js`).
pub const RESERVED_REQUEST_SLUGS: [&str; 2] = ["collection", "folder"];

/// Whether `slug` is reserved for requests; see [`RESERVED_REQUEST_SLUGS`].
#[must_use]
pub fn is_reserved_request_slug(slug: &str) -> bool {
    RESERVED_REQUEST_SLUGS.contains(&slug)
}

pub(crate) fn validate_request_slug(slug: &str) -> Result<(), StoreError> {
    validate_slug(slug)?;
    if is_reserved_request_slug(slug) {
        return Err(StoreError::ReservedSlug {
            slug: slug.to_owned(),
        });
    }
    Ok(())
}

pub(crate) fn validate_slug(slug: &str) -> Result<(), StoreError> {
    let valid = !slug.is_empty() && !slug.starts_with('.') && !slug.contains(['/', '\\']);
    if valid {
        Ok(())
    } else {
        Err(StoreError::InvalidSlug {
            slug: slug.to_owned(),
        })
    }
}

pub(crate) fn validate_parent(parent: &Parent) -> Result<(), StoreError> {
    parent.validate()
}
