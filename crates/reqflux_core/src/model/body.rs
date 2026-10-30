//! Request bodies and settings.

use serde::{Deserialize, Serialize};

use super::variable::{KeyValue, enabled_default, is_true};

/// A request body, serialized as a `[body]` table with a `mode` key.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "lowercase")]
pub enum Body {
    /// No body. Omitted on disk.
    #[default]
    None,
    /// Raw text with a language hint.
    Raw {
        /// The language hint.
        language: RawLanguage,
        /// The body text.
        #[serde(default)]
        text: String,
    },
    /// `application/x-www-form-urlencoded` fields.
    Urlencoded {
        /// The fields, in order.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        fields: Vec<KeyValue>,
    },
    /// `multipart/form-data` parts.
    #[serde(rename = "formdata")]
    FormData {
        /// The parts, in order.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        parts: Vec<FormPart>,
    },
    /// The bytes of a file.
    Binary {
        /// The file path.
        path: String,
    },
    /// A GraphQL query.
    Graphql {
        /// The query text.
        #[serde(default)]
        query: String,
        /// The variables, as JSON text.
        #[serde(default, skip_serializing_if = "String::is_empty")]
        variables: String,
    },
}

impl Body {
    /// Whether this is [`Body::None`], the default omitted on disk.
    #[must_use]
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

/// The language hint of a raw body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RawLanguage {
    /// JSON.
    Json,
    /// XML.
    Xml,
    /// Plain text.
    #[default]
    Text,
    /// HTML.
    Html,
    /// JavaScript.
    Javascript,
}

/// A part of a form-data body, tagged by `kind`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum FormPart {
    /// A text field.
    Text {
        /// The field name.
        key: String,
        /// The field value.
        #[serde(default)]
        value: String,
        /// A human description.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        /// Disabled parts are not sent.
        #[serde(default = "enabled_default", skip_serializing_if = "is_true")]
        enabled: bool,
    },
    /// A file field.
    File {
        /// The field name.
        key: String,
        /// The file path.
        path: String,
        /// A human description.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        /// Disabled parts are not sent.
        #[serde(default = "enabled_default", skip_serializing_if = "is_true")]
        enabled: bool,
    },
}

/// Per-request settings. `None` means the workspace default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Settings {
    /// The request timeout in milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    /// Whether redirects are followed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub follow_redirects: Option<bool>,
    /// Whether TLS certificates are verified.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verify_tls: Option<bool>,
}

impl Settings {
    /// Whether every setting is the workspace default.
    #[must_use]
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}
