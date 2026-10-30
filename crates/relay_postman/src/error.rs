//! Why a Postman document does not import (S8).
//!
//! Every message names what is wrong and where it is, using the path of the item inside
//! the collection, like `item[3].request`.

/// A document that does not import.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PostmanError {
    /// The document is not a JSON object.
    #[error("collection must be a JSON object")]
    NotAnObject,
    /// The collection has no `info.name`.
    #[error("collection is missing `info.name`")]
    MissingName,
    /// The environment has no `name`.
    #[error("environment is missing `name`")]
    MissingEnvironmentName,
    /// The item at `path` is neither a folder nor a request.
    #[error("`{path}`: item is neither a folder nor a request")]
    UnknownItem {
        /// The path of the item, like `item[0].item[3]`.
        path: String,
    },
    /// The item at `path` has no name.
    #[error("`{path}`: item is missing a name")]
    MissingItemName {
        /// The path of the item.
        path: String,
    },
    /// The request at `path` has no method.
    #[error("`{path}`: request is missing a method")]
    MissingMethod {
        /// The path of the request.
        path: String,
    },
    /// The request at `path` has no URL.
    #[error("`{path}`: request is missing a URL")]
    MissingUrl {
        /// The path of the request.
        path: String,
    },
    /// The body of mode `mode` at `path` lacks its payload key `field`.
    #[error("`{path}`: body mode `{mode}` is missing `{field}`")]
    MissingBodyPayload {
        /// The path of the request.
        path: String,
        /// The Postman body mode.
        mode: String,
        /// The payload key the mode needs.
        field: &'static str,
    },
    /// The request at `path` has a verb that is not a method.
    #[error("`{path}`: `{verb}` is not an HTTP method")]
    BadMethod {
        /// The path of the request.
        path: String,
        /// The verb Postman gave.
        verb: String,
    },
}
