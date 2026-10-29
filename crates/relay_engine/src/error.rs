//! Engine errors.

use relay_http::HttpError;
use relay_store::StoreError;

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
}

impl ExecuteError {
    /// Whether the error comes from what the caller asked for (an unknown request or
    /// environment) rather than from executing it.
    #[must_use]
    pub fn is_usage(&self) -> bool {
        matches!(self, Self::UnknownRequest(_) | Self::UnknownEnvironment(_))
    }
}
