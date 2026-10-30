//! HTTP methods, including custom verbs.

use std::borrow::Cow;
use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// An HTTP method stored as an uppercase verb. Custom verbs are allowed.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Method(Cow<'static, str>);

/// Why a string is not a valid HTTP method.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MethodError {
    /// The verb is empty.
    #[error("HTTP method is empty")]
    Empty,
    /// The verb contains whitespace.
    #[error("HTTP method `{0}` contains whitespace")]
    Whitespace(String),
}

impl Method {
    /// `GET`.
    pub const GET: Self = Self(Cow::Borrowed("GET"));
    /// `POST`.
    pub const POST: Self = Self(Cow::Borrowed("POST"));
    /// `PUT`.
    pub const PUT: Self = Self(Cow::Borrowed("PUT"));
    /// `PATCH`.
    pub const PATCH: Self = Self(Cow::Borrowed("PATCH"));
    /// `DELETE`.
    pub const DELETE: Self = Self(Cow::Borrowed("DELETE"));
    /// `HEAD`.
    pub const HEAD: Self = Self(Cow::Borrowed("HEAD"));
    /// `OPTIONS`.
    pub const OPTIONS: Self = Self(Cow::Borrowed("OPTIONS"));

    /// Builds a method from a verb, normalized to uppercase.
    ///
    /// # Errors
    ///
    /// Fails when the verb is empty or contains whitespace.
    pub fn new(verb: &str) -> Result<Self, MethodError> {
        if verb.is_empty() {
            return Err(MethodError::Empty);
        }
        if verb.chars().any(char::is_whitespace) {
            return Err(MethodError::Whitespace(verb.to_owned()));
        }
        Ok(Self(Cow::Owned(verb.to_uppercase())))
    }

    /// Returns the uppercase verb.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for Method {
    fn default() -> Self {
        Self::GET
    }
}

impl fmt::Display for Method {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for Method {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Method {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::new(&text).map_err(serde::de::Error::custom)
    }
}
