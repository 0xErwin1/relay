//! Stable entity identifiers.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use ulid::Ulid;

/// The stable identifier of a workspace entity, serialized as a ULID string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id(Ulid);

impl Id {
    /// Generates a fresh identifier.
    #[must_use]
    pub fn new() -> Self {
        Self(Ulid::generate())
    }

    /// Returns the underlying ULID.
    #[must_use]
    pub fn ulid(self) -> Ulid {
        self.0
    }
}

impl Default for Id {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for Id {
    type Err = ulid::DecodeError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Ulid::from_string(text).map(Self)
    }
}

impl Serialize for Id {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Id {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}
