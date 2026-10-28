//! Request authorization.

use serde::de::Error as _;
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// How a request, folder or collection authorizes. Serialized as an `[auth]` table with a
/// `type` key followed by the fields of that type.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Auth {
    /// Use the auth of the parent folder or collection. Omitted on disk.
    #[default]
    Inherit,
    /// Send no authorization.
    None,
    /// HTTP basic auth.
    Basic {
        /// The user name.
        username: String,
        /// The password.
        password: String,
    },
    /// A bearer token.
    Bearer {
        /// The token.
        token: String,
    },
    /// An API key sent in a header or the query.
    ApiKey {
        /// The header or query parameter name.
        key: String,
        /// The key value.
        value: String,
        /// Where the key is sent.
        location: ApiKeyLocation,
    },
    /// Any other auth type, kept unchanged on round trip.
    Other {
        /// The `type` value.
        kind: String,
        /// Every other key of the table. Written back in key order.
        fields: toml::Table,
    },
}

/// Where an API key is sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ApiKeyLocation {
    /// As a request header.
    #[default]
    Header,
    /// As a query parameter.
    Query,
}

impl Auth {
    /// Whether this is [`Auth::Inherit`], the default omitted on disk.
    #[must_use]
    pub fn is_inherit(&self) -> bool {
        matches!(self, Self::Inherit)
    }

    /// The `type` value written to disk.
    #[must_use]
    pub fn kind(&self) -> &str {
        match self {
            Self::Inherit => "inherit",
            Self::None => "none",
            Self::Basic { .. } => "basic",
            Self::Bearer { .. } => "bearer",
            Self::ApiKey { .. } => "apikey",
            Self::Other { kind, .. } => kind,
        }
    }
}

impl Serialize for Auth {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("type", self.kind())?;
        match self {
            Self::Inherit | Self::None => {}
            Self::Basic { username, password } => {
                map.serialize_entry("username", username)?;
                map.serialize_entry("password", password)?;
            }
            Self::Bearer { token } => map.serialize_entry("token", token)?,
            Self::ApiKey {
                key,
                value,
                location,
            } => {
                map.serialize_entry("key", key)?;
                map.serialize_entry("value", value)?;
                map.serialize_entry("location", location)?;
            }
            Self::Other { fields, .. } => {
                for (key, value) in fields {
                    map.serialize_entry(key, value)?;
                }
            }
        }
        map.end()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BasicFields {
    username: String,
    password: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BearerFields {
    token: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ApiKeyFields {
    key: String,
    value: String,
    location: ApiKeyLocation,
}

impl<'de> Deserialize<'de> for Auth {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let mut fields = toml::Table::deserialize(deserializer)?;
        let kind = match fields.remove("type") {
            Some(toml::Value::String(kind)) => kind,
            Some(_) => return Err(D::Error::custom("auth `type` must be a string")),
            None => return Err(D::Error::missing_field("type")),
        };
        let typed = || toml::Value::Table(fields.clone());
        // Known types whose keys do not match the modeled shape exactly are kept whole as
        // `Other`, so a save never rewrites what Relay did not understand.
        let modeled = match kind.as_str() {
            "inherit" if fields.is_empty() => Some(Self::Inherit),
            "none" if fields.is_empty() => Some(Self::None),
            "basic" => typed().try_into::<BasicFields>().ok().map(|f| Self::Basic {
                username: f.username,
                password: f.password,
            }),
            "bearer" => typed()
                .try_into::<BearerFields>()
                .ok()
                .map(|f| Self::Bearer { token: f.token }),
            "apikey" => typed()
                .try_into::<ApiKeyFields>()
                .ok()
                .map(|f| Self::ApiKey {
                    key: f.key,
                    value: f.value,
                    location: f.location,
                }),
            _ => None,
        };
        Ok(modeled.unwrap_or(Self::Other { kind, fields }))
    }
}
