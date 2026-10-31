//! The `settings` table of schema v5 (light-theme S4): small per-user preferences as text
//! keyed by name, such as the theme mode. The app owns each key's meaning and value format.

use super::{AppState, StateError, query_error};

/// The table schema version 5 adds.
pub(super) const SCHEMA_V5: &str = "
CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
";

impl AppState {
    /// The value stored under `key`, or `None` when it was never set.
    pub fn setting(&self, key: &str) -> Result<Option<String>, StateError> {
        let mut statement = self
            .conn
            .prepare("SELECT value FROM settings WHERE key = ?1")
            .map_err(query_error(&self.path))?;
        let mut rows = statement
            .query(rusqlite::params![key])
            .map_err(query_error(&self.path))?;
        match rows.next().map_err(query_error(&self.path))? {
            Some(row) => row.get(0).map(Some).map_err(query_error(&self.path)),
            None => Ok(None),
        }
    }

    /// Stores `value` under `key`, replacing any previous value.
    pub fn set_setting(&mut self, key: &str, value: &str) -> Result<(), StateError> {
        self.conn
            .execute(
                "INSERT INTO settings (key, value) VALUES (?1, ?2)
                 ON CONFLICT (key) DO UPDATE SET value = excluded.value",
                rusqlite::params![key, value],
            )
            .map_err(query_error(&self.path))?;
        Ok(())
    }
}
