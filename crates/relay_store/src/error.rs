//! Store errors and load warnings.

use std::fmt;
use std::io;
use std::path::PathBuf;

use relay_core::model::Id;

use crate::FORMAT;

/// Why a load or save failed. Every variant that concerns a file names it.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// Reading, writing or listing a path failed.
    #[error("{}: {source}", path.display())]
    Io {
        /// The path involved.
        path: PathBuf,
        /// The underlying error.
        source: io::Error,
    },
    /// A TOML file does not parse as its entity.
    #[error("{}: {source}", path.display())]
    Parse {
        /// The file.
        path: PathBuf,
        /// The parser error, with line and column.
        source: toml::de::Error,
    },
    /// An entity does not serialize to TOML.
    #[error("{}: {source}", path.display())]
    Serialize {
        /// The file being written.
        path: PathBuf,
        /// The serializer error.
        source: toml::ser::Error,
    },
    /// `relay.toml` declares a format newer than [`FORMAT`] (S6).
    #[error(
        "workspace format {found} is newer than this version of Relay supports ({FORMAT}); update Relay to open it"
    )]
    NewerFormat {
        /// The declared format.
        found: i64,
    },
    /// `relay.toml` has no format or one older than [`FORMAT`] (S6).
    #[error("{}: missing or unsupported workspace format", path.display())]
    UnsupportedFormat {
        /// The manifest path.
        path: PathBuf,
    },
    /// The file changed since the caller read it, or exists when creating (S7).
    #[error("{} changed on disk since it was loaded", path.display())]
    Conflict {
        /// The file that was left untouched.
        path: PathBuf,
    },
    /// A slug is not a single, visible path component.
    #[error("invalid slug `{slug}`")]
    InvalidSlug {
        /// The rejected slug.
        slug: String,
    },
    /// A request slug would share its sidecar scripts with the collection or folder that
    /// contains it (`collection.pre.js`, `folder.pre.js`).
    #[error("request slug `{slug}` is reserved")]
    ReservedSlug {
        /// The rejected slug.
        slug: String,
    },
}

/// A non-fatal problem found while loading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Warning {
    /// The file or directory concerned.
    pub path: PathBuf,
    /// What happened.
    pub kind: WarningKind,
}

/// The kinds of load warnings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WarningKind {
    /// The entity id was already taken by an earlier file in load order; a fresh id was
    /// assigned in memory (S8).
    DuplicateId {
        /// The id in the file.
        id: Id,
        /// The earlier file that keeps it.
        other: PathBuf,
    },
    /// A secret variable carried a value, which was dropped (S10).
    SecretValue {
        /// The variable key.
        key: String,
    },
    /// A directory lacks its entity file (`folder.toml` or `collection.toml`) and was skipped.
    MissingEntityFile {
        /// The expected file name.
        file: &'static str,
    },
    /// A request file uses a reserved slug; it was skipped so its sidecar scripts are not
    /// confused with those of its collection or folder.
    ReservedRequestSlug {
        /// The reserved slug.
        slug: String,
    },
    /// A directory is a symlink; it was not followed so a hand-edited workspace cannot
    /// make loading read files outside the root.
    SymlinkedDirectory,
}

impl fmt::Display for Warning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let path = self.path.display();
        match &self.kind {
            WarningKind::DuplicateId { id, other } => write!(
                f,
                "{path}: id {id} is already used by {}; assigned a new id",
                other.display()
            ),
            WarningKind::SecretValue { key } => write!(
                f,
                "{path}: secret variable {key} has a value in the workspace file; it was ignored"
            ),
            WarningKind::MissingEntityFile { file } => {
                write!(f, "{path}: directory has no {file}; it was ignored")
            }
            WarningKind::ReservedRequestSlug { slug } => write!(
                f,
                "{path}: request slug `{slug}` is reserved; rename the file to load it"
            ),
            WarningKind::SymlinkedDirectory => {
                write!(f, "{path}: directory is a symlink; it was ignored")
            }
        }
    }
}
