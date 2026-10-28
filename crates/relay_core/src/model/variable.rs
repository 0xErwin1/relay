//! Variables and key/value entries.

use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize, Serializer};

/// A named variable of a scope: workspace globals, collection or environment.
///
/// A secret variable never serializes its value (S10).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Variable {
    /// The name used in `{{key}}`.
    pub key: String,
    /// The shared value. Always `None` on disk for a secret.
    #[serde(default)]
    pub value: Option<String>,
    /// A human description.
    #[serde(default)]
    pub description: Option<String>,
    /// Whether the value is kept out of workspace files.
    #[serde(default)]
    pub secret: bool,
    /// Disabled variables do not resolve.
    #[serde(default = "enabled_default")]
    pub enabled: bool,
}

impl Variable {
    /// Builds an enabled, non-secret variable.
    #[must_use]
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: Some(value.into()),
            description: None,
            secret: false,
            enabled: true,
        }
    }
}

impl Serialize for Variable {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct("Variable", 5)?;
        state.serialize_field("key", &self.key)?;
        match self.value.as_ref().filter(|_| !self.secret) {
            Some(value) => state.serialize_field("value", value)?,
            None => state.skip_field("value")?,
        }
        match &self.description {
            Some(description) => state.serialize_field("description", description)?,
            None => state.skip_field("description")?,
        }
        if self.secret {
            state.serialize_field("secret", &true)?;
        } else {
            state.skip_field("secret")?;
        }
        if self.enabled {
            state.skip_field("enabled")?;
        } else {
            state.serialize_field("enabled", &false)?;
        }
        state.end()
    }
}

/// Drops the value of every secret variable and returns the keys that had one,
/// in order, so the loader can report them (S10).
pub fn strip_secret_values(variables: &mut [Variable]) -> Vec<String> {
    variables
        .iter_mut()
        .filter(|variable| variable.secret)
        .filter_map(|variable| variable.value.take().map(|_| variable.key.clone()))
        .collect()
}

/// A key/value entry of params, headers or a urlencoded body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyValue {
    /// The entry name.
    pub key: String,
    /// The entry value, possibly templated.
    #[serde(default)]
    pub value: String,
    /// A human description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Disabled entries are not sent.
    #[serde(default = "enabled_default", skip_serializing_if = "is_true")]
    pub enabled: bool,
}

impl KeyValue {
    /// Builds an enabled entry without description.
    #[must_use]
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
            description: None,
            enabled: true,
        }
    }
}

pub(crate) fn enabled_default() -> bool {
    true
}

#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde skip_serializing_if signature"
)]
pub(crate) fn is_true(value: &bool) -> bool {
    *value
}
