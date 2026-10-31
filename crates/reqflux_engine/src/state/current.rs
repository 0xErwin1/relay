//! The `current_values` table of schema v3 (variables-editor S4): one row per current value,
//! keyed by workspace id, scope and key. The scope is the text of
//! [`SecretScope::path`](crate::SecretScope::path): `global`, `collection/<id>` or
//! `environment/<id>`. The table never holds a secret's value; the engine keeps those in the
//! secret store.

use super::{AppState, StateError, query_error};
use crate::current::CurrentValues;
use crate::secrets::SecretScope;

/// The table schema version 3 adds.
pub(super) const SCHEMA_V3: &str = "
CREATE TABLE IF NOT EXISTS current_values (
    workspace_id TEXT NOT NULL,
    scope TEXT NOT NULL,
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (workspace_id, scope, key)
);
";

impl AppState {
    /// Every current value of `workspace_id`.
    ///
    /// # Errors
    ///
    /// The read failed, or a row names a scope this build cannot parse
    /// ([`StateError::Corrupt`]).
    pub fn current_values(&self, workspace_id: &str) -> Result<CurrentValues, StateError> {
        let mut statement = self
            .conn
            .prepare(
                "SELECT scope, key, value FROM current_values
                 WHERE workspace_id = ?1 ORDER BY scope, key",
            )
            .map_err(query_error(&self.path))?;
        let rows = statement
            .query_map(rusqlite::params![workspace_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(query_error(&self.path))?;
        let mut values = CurrentValues::new();
        for row in rows {
            let (scope, key, value) = row.map_err(query_error(&self.path))?;
            let Some(parsed) = SecretScope::from_path(&scope) else {
                return Err(StateError::Corrupt {
                    path: self.path.clone(),
                    reason: format!("a current value has the unknown scope {scope}"),
                });
            };
            values.insert(parsed, key, value);
        }
        Ok(values)
    }

    /// Sets the current value of `key` in `scope` of `workspace_id`, replacing any previous
    /// one. The caller never passes a secret's value; see [`crate::CurrentValues`].
    ///
    /// # Errors
    ///
    /// The write failed.
    pub fn set_current_value(
        &mut self,
        workspace_id: &str,
        scope: SecretScope,
        key: &str,
        value: &str,
    ) -> Result<(), StateError> {
        self.conn
            .execute(
                "INSERT INTO current_values (workspace_id, scope, key, value)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT (workspace_id, scope, key) DO UPDATE SET value = excluded.value",
                rusqlite::params![workspace_id, scope.path(), key, value],
            )
            .map_err(query_error(&self.path))?;
        Ok(())
    }

    /// Clears the current value of `key` in `scope` of `workspace_id`, so the variable
    /// resolves to its shared value again; reports whether there was one.
    ///
    /// # Errors
    ///
    /// The write failed.
    pub fn clear_current_value(
        &mut self,
        workspace_id: &str,
        scope: SecretScope,
        key: &str,
    ) -> Result<bool, StateError> {
        let changed = self
            .conn
            .execute(
                "DELETE FROM current_values WHERE workspace_id = ?1 AND scope = ?2 AND key = ?3",
                rusqlite::params![workspace_id, scope.path(), key],
            )
            .map_err(query_error(&self.path))?;
        Ok(changed > 0)
    }

    /// Clears every current value of `workspace_id`, reporting how many there were.
    ///
    /// # Errors
    ///
    /// The write failed.
    pub fn clear_workspace_current_values(
        &mut self,
        workspace_id: &str,
    ) -> Result<usize, StateError> {
        self.conn
            .execute(
                "DELETE FROM current_values WHERE workspace_id = ?1",
                rusqlite::params![workspace_id],
            )
            .map_err(query_error(&self.path))
    }
}
