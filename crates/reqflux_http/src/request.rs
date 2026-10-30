//! The resolved request the transport sends.

use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

/// A request with every variable, auth and setting already resolved.
///
/// Its `Debug` output never shows a resolved value that can hold a secret: it shows the
/// method, the header names and the shape of the body, but not the URL, the header values
/// or the body content.
#[derive(Clone, PartialEq, Eq)]
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

impl fmt::Debug for PreparedRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names: Vec<&str> = self.headers.iter().map(|(name, _)| name.as_str()).collect();
        let body = match &self.body {
            PreparedBody::None => "none".to_owned(),
            PreparedBody::Bytes { data, .. } => format!("{} bytes", data.len()),
            PreparedBody::Form(fields) => format!("form, {} fields", fields.len()),
            PreparedBody::Multipart(parts) => format!("multipart, {} parts", parts.len()),
            PreparedBody::File(_) => "file".to_owned(),
        };
        formatter
            .debug_struct("PreparedRequest")
            .field("method", &self.method)
            .field("url", &"<redacted>")
            .field("headers", &names)
            .field("body", &body)
            .field("options", &self.options)
            .finish()
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_never_prints_the_url_header_values_or_the_body() {
        let request = PreparedRequest {
            method: "POST".to_owned(),
            url: "https://api.test/?key=url-secret".to_owned(),
            headers: vec![(
                "Authorization".to_owned(),
                "Bearer header-secret".to_owned(),
            )],
            body: PreparedBody::Bytes {
                content_type: None,
                data: b"body-secret".to_vec(),
            },
            options: RequestOptions {
                timeout: Duration::from_secs(1),
                follow_redirects: true,
                verify_tls: true,
            },
        };

        let debug = format!("{request:?}");

        assert!(
            debug.contains("POST") && debug.contains("Authorization"),
            "{debug}"
        );
        for secret in ["url-secret", "header-secret", "body-secret"] {
            assert!(!debug.contains(secret), "{debug}");
        }
    }
}
