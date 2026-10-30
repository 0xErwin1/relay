//! The request as sent, redacted for history (history S3).
//!
//! Preparing a request collects the values that must never be stored in clear: the resolved
//! value of every secret variable the request referenced, and the resolved basic password,
//! bearer token and API key value whether they came from a variable or were typed. The
//! redacted copy replaces each of them by [`MASK`] wherever it appears in the URL, the
//! header values and the body, as text, in the forms a request carries it: as written,
//! percent-encoded, form-encoded and JSON-string-escaped. Header names are never masked.
//! The values of `Authorization`, `Proxy-Authorization` and `Cookie` are always masked
//! whole, keeping an auth scheme word, since a script can transform a secret (base64, for
//! example) beyond what text matching finds. A [`Masker`] applies the same replacement to
//! any other text, such as the error of a failed send. Text replacement masks a secret also where something
//! else produced the same characters, so a very short secret value can mask more than it
//! produced; that is the safe side and there is no length floor. Only an empty value is
//! skipped, since it occurs everywhere and masks nothing.

use std::fmt;
use std::sync::Arc;

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
/// there are, so it never prints one.
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

    /// The [`Masker`] of these values.
    pub(crate) fn masker(&self) -> Masker {
        Masker {
            patterns: self.patterns().into(),
        }
    }

    /// The values to replace, each as written, percent-encoded the way the query string
    /// and a URL-encoded form carry it, and escaped the way a JSON string carries it,
    /// longest first so a secret that contains another is masked whole.
    fn patterns(&self) -> Vec<String> {
        let mut patterns: Vec<String> = Vec::new();
        for value in &self.0 {
            let forms = [
                value.clone(),
                crate::prepare::percent_encode(value),
                url::form_urlencoded::byte_serialize(value.as_bytes()).collect(),
                json_escaped(value),
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

/// Replaces the secret values of one send by [`MASK`] in any text, in every form
/// [`SentRequest`] masks them. A front end masks the error of a failed send with it before
/// recording or showing that error, since a transport error can echo the URL. Cheap to
/// clone; its `Debug` output shows only how many patterns it has.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Masker {
    patterns: Arc<[String]>,
}

impl fmt::Debug for Masker {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Masker({} patterns)", self.patterns.len())
    }
}

impl Masker {
    /// `text` with every secret value replaced by [`MASK`].
    #[must_use]
    pub fn mask(&self, text: &str) -> String {
        self.patterns.iter().fold(text.to_owned(), |text, pattern| {
            text.replace(pattern.as_str(), MASK)
        })
    }
}

/// `value` as a JSON string carries it, without the quotes.
fn json_escaped(value: &str) -> String {
    let quoted = serde_json::Value::from(value).to_string();
    quoted
        .strip_prefix('"')
        .and_then(|inner| inner.strip_suffix('"'))
        .map_or(quoted.clone(), str::to_owned)
}

/// Headers whose whole value is a credential.
const CREDENTIAL_HEADERS: [&str; 3] = ["authorization", "proxy-authorization", "cookie"];

/// The recorded value of the header `name`: a credential header masked whole, keeping the
/// auth scheme word of `Authorization` and `Proxy-Authorization`; any other masked by text.
fn header_value(name: &str, value: &str, masker: &Masker) -> String {
    if !CREDENTIAL_HEADERS
        .iter()
        .any(|credential| name.eq_ignore_ascii_case(credential))
    {
        return masker.mask(value);
    }
    let scheme = (!name.eq_ignore_ascii_case("cookie"))
        .then(|| value.trim_start().split_once(' '))
        .flatten()
        .map(|(scheme, _)| scheme)
        .filter(|scheme| !scheme.is_empty() && !masker.mask(scheme).contains(MASK));
    match scheme {
        Some(scheme) => format!("{scheme} {MASK}"),
        None => MASK.to_owned(),
    }
}

/// The redacted copy of `request`.
pub(crate) fn redact(request: &PreparedRequest, masker: &Masker) -> SentRequest {
    let mask = |text: &str| masker.mask(text);
    SentRequest {
        method: request.method.clone(),
        url: mask(&request.url),
        headers: request
            .headers
            .iter()
            .map(|(key, value)| (key.clone(), header_value(key, value, masker)))
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
