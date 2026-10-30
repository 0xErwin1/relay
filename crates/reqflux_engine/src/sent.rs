//! The request as sent, redacted for history (history S3).
//!
//! Preparing a request collects the values that must never be stored in clear: the resolved
//! value of every secret variable the request referenced, and the resolved basic password,
//! bearer token and API key value whether they came from a variable or were typed. The
//! redacted copy replaces each of them by [`MASK`] wherever it appears in the URL, the
//! headers and the body, as text. Text replacement masks a secret also where something
//! else produced the same characters, so a very short secret value can mask more than it
//! produced; that is the safe side and there is no length floor. Only an empty value is
//! skipped, since it occurs everywhere and masks nothing.

use std::fmt;

use reqflux_http::{PreparedBody, PreparedRequest};

/// What a secret is replaced with in a recorded request.
pub const MASK: &str = "••••••••";

/// The request a send put on the wire, with every secret replaced by [`MASK`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SentRequest {
    /// The HTTP method.
    pub method: String,
    /// The URL, query string included.
    pub url: String,
    /// The headers in sent order, auth included.
    pub headers: Vec<(String, String)>,
    /// The body.
    pub body: SentBody,
}

/// The body of a [`SentRequest`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SentBody {
    /// No body.
    None,
    /// A text body: raw and GraphQL bodies as sent, URL-encoded forms in their wire form.
    Text(String),
    /// A body that is not kept as text, described: a multipart form, a file, or bytes
    /// that are not UTF-8.
    Omitted(String),
}

/// The values a send must never record in clear. Its `Debug` output shows only how many
/// there are, so a logged [`Prepared`](crate::Prepared) never prints one.
#[derive(Clone, Default, PartialEq, Eq)]
pub(crate) struct Secrets(Vec<String>);

impl fmt::Debug for Secrets {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Secrets({} values)", self.0.len())
    }
}

impl Secrets {
    /// Adds `value`, unless it is empty or already known.
    pub(crate) fn add(&mut self, value: impl Into<String>) {
        let value = value.into();
        if !value.is_empty() && !self.0.contains(&value) {
            self.0.push(value);
        }
    }

    /// The values to replace, each as written and percent-encoded the way the query
    /// string and a URL-encoded form carry it, longest first so a secret that contains
    /// another is masked whole.
    fn patterns(&self) -> Vec<String> {
        let mut patterns: Vec<String> = Vec::new();
        for value in &self.0 {
            let forms = [
                value.clone(),
                crate::prepare::percent_encode(value),
                url::form_urlencoded::byte_serialize(value.as_bytes()).collect(),
            ];
            for form in forms {
                if !patterns.contains(&form) {
                    patterns.push(form);
                }
            }
        }
        patterns.sort_by_key(|pattern| std::cmp::Reverse(pattern.len()));
        patterns
    }
}

/// The redacted copy of `request`.
pub(crate) fn redact(request: &PreparedRequest, secrets: &Secrets) -> SentRequest {
    let patterns = secrets.patterns();
    let mask = |text: &str| mask_text(text, &patterns);
    SentRequest {
        method: request.method.clone(),
        url: mask(&request.url),
        headers: request
            .headers
            .iter()
            .map(|(key, value)| (mask(key), mask(value)))
            .collect(),
        body: match &request.body {
            PreparedBody::None => SentBody::None,
            PreparedBody::Bytes { data, .. } => match std::str::from_utf8(data) {
                Ok(text) => SentBody::Text(mask(text)),
                Err(_) => SentBody::Omitted(format!("{} bytes of binary data", data.len())),
            },
            PreparedBody::Form(fields) => SentBody::Text(mask(
                &url::form_urlencoded::Serializer::new(String::new())
                    .extend_pairs(fields)
                    .finish(),
            )),
            PreparedBody::Multipart(parts) => {
                SentBody::Omitted(format!("multipart form data, {} parts", parts.len()))
            }
            PreparedBody::File(path) => SentBody::Omitted(format!("file {}", path.display())),
        },
    }
}

/// `text` with every pattern replaced by [`MASK`].
fn mask_text(text: &str, patterns: &[String]) -> String {
    patterns.iter().fold(text.to_owned(), |text, pattern| {
        text.replace(pattern.as_str(), MASK)
    })
}
