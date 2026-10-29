//! The resolved request the transport sends.

use std::path::PathBuf;
use std::time::Duration;

/// A request with every variable, auth and setting already resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedRequest {
    /// The HTTP method, any token including custom verbs.
    pub method: String,
    /// The full URL, query string included.
    pub url: String,
    /// Headers in send order; a name may repeat.
    pub headers: Vec<(String, String)>,
    /// The request body.
    pub body: PreparedBody,
    /// Per-request transport settings.
    pub options: RequestOptions,
}

/// The body of a [`PreparedRequest`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreparedBody {
    /// No body.
    None,
    /// Raw bytes. `content_type` is sent unless a `Content-Type` header is already set.
    Bytes {
        /// The content type to send with the bytes.
        content_type: Option<String>,
        /// The bytes to send.
        data: Vec<u8>,
    },
    /// URL-encoded form fields, in order.
    Form(Vec<(String, String)>),
    /// A `multipart/form-data` body, parts in order.
    Multipart(Vec<MultipartPart>),
    /// A file streamed as the body.
    File(PathBuf),
}

/// One part of a multipart body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultipartPart {
    /// The form field name.
    pub name: String,
    /// The part value.
    pub value: MultipartValue,
}

/// The value of a [`MultipartPart`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MultipartValue {
    /// A text value.
    Text(String),
    /// A file, sent with its file name and a content type guessed from its extension
    /// (`application/octet-stream` when unknown).
    File(PathBuf),
}

/// Transport settings of one request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestOptions {
    /// Total time allowed for the request, body download included.
    pub timeout: Duration,
    /// Whether to follow redirects, at most 10.
    pub follow_redirects: bool,
    /// Whether to verify the server certificate.
    pub verify_tls: bool,
}
