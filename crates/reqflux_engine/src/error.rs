//! Engine errors.

use std::path::PathBuf;

use reqflux_http::HttpError;
use reqflux_store::StoreError;

/// Why a workspace could not be opened.
#[derive(Debug, thiserror::Error)]
pub enum OpenError {
    /// The workspace does not load.
    #[error(transparent)]
    Load(#[from] StoreError),
    /// The Tokio runtime could not start.
    #[error("could not start the async runtime: {0}")]
    Runtime(std::io::Error),
}

/// Why a request could not be prepared or sent.
#[derive(Debug, thiserror::Error)]
pub enum ExecuteError {
    /// No request has this id or slug path.
    #[error("no request matches {0}")]
    UnknownRequest(String),
    /// No workspace environment has this slug.
    #[error("unknown environment {0}")]
    UnknownEnvironment(String),
    /// The collection's environment allowlist excludes the environment.
    #[error("environment {environment} is not allowed for collection {collection}")]
    EnvironmentNotAllowed {
        /// The environment slug.
        environment: String,
        /// The collection name.
        collection: String,
    },
    /// The effective auth is a type the engine cannot send yet.
    #[error("auth type {0} is not supported yet")]
    UnsupportedAuth(String),
    /// The GraphQL variables do not parse as JSON.
    #[error("graphql variables are not valid JSON: {0}")]
    GraphqlVariables(String),
    /// The transport failed.
    #[error(transparent)]
    Http(#[from] HttpError),
    /// The execution task panicked or was aborted.
    #[error("execution task failed: {0}")]
    Task(String),
    /// The store failed, for example when a slug from the workspace cannot form a path.
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Why a Postman import could not be written (S9).
#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    /// The document does not map to the ReqFlux model (S8).
    #[error(transparent)]
    Postman(#[from] reqflux_postman::PostmanError),
    /// The store failed to write `path`.
    #[error("{}: {message}", path.display())]
    Store {
        /// The file being written.
        path: PathBuf,
        /// What went wrong.
        message: String,
    },
    /// The file exists; an import never overwrites (S9).
    #[error("{} already exists; an import never overwrites a file", path.display())]
    Exists {
        /// The existing file.
        path: PathBuf,
    },
}

/// Why a request document could not be saved back to its file.
#[derive(Debug, thiserror::Error)]
pub enum SaveError {
    /// The file changed since the workspace was loaded; the draft is kept (S6).
    #[error("{} changed on disk since the workspace was loaded", path.display())]
    Conflict {
        /// The file that was left untouched.
        path: PathBuf,
    },
    /// No request has this id or slug path; nothing was written.
    #[error("no request matches {0}")]
    UnknownRequest(String),
    /// The store failed for any other reason.
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl ExecuteError {
    /// Whether the error comes from what the caller asked for (an unknown request or
    /// environment) rather than from executing it.
    #[must_use]
    pub fn is_usage(&self) -> bool {
        matches!(self, Self::UnknownRequest(_) | Self::UnknownEnvironment(_))
    }
}
