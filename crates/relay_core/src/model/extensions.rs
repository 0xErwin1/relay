//! Unknown keys preserved across a load and save (S9).
//!
//! Each document flattens its unknown top-level keys into `extensions` on load, merging the
//! content of an existing `[extensions]` table, and writes them back as one `[extensions]`
//! table. A file written by Relay therefore always round-trips byte for byte; a hand-written
//! unknown top-level key moves under `[extensions]` on the first save and stays there.

use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serializer};

const KEY: &str = "extensions";

pub(crate) fn serialize<S: Serializer>(
    extensions: &toml::Table,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let mut map = serializer.serialize_map(None)?;
    if !extensions.is_empty() {
        map.serialize_entry(KEY, extensions)?;
    }
    map.end()
}

pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<toml::Table, D::Error> {
    let mut unknown = toml::Table::deserialize(deserializer)?;
    let mut extensions = match unknown.remove(KEY) {
        Some(toml::Value::Table(table)) => table,
        Some(other) => {
            let mut table = toml::Table::new();
            table.insert(KEY.to_owned(), other);
            table
        }
        None => toml::Table::new(),
    };
    extensions.extend(unknown);
    Ok(extensions)
}
