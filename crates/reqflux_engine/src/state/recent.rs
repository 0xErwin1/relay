//! The `recent_workspaces` table of schema v4 (recent-workspaces S1): one row per workspace
//! directory the app opened, keyed by its path, newest first and capped at
//! [`RECENT_WORKSPACES_MAX`].
//!
//! Paths are stored as given. Callers pass a canonicalized absolute path so the same
//! directory reached two ways is one entry; the table does not canonicalize, because the
//! directory may no longer exist when an entry is removed. A path that is not valid UTF-8
//! is refused with [`StateError::NonUtf8Path`], never converted lossily.

use std::path::{Path, PathBuf};

use super::{AppState, StateError, query_error};

/// The table schema version 4 adds.
pub(super) const SCHEMA_V4: &str = "
CREATE TABLE IF NOT EXISTS recent_workspaces (
    path TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL,
    name TEXT NOT NULL,
    opened_at INTEGER NOT NULL
);
";

/// How many recent workspaces the app state keeps; recording one more drops the oldest.
pub const RECENT_WORKSPACES_MAX: usize = 20;

/// A workspace the app opened, as the recent list shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecentWorkspace {
    /// The workspace directory, as recorded (canonicalized and absolute by the caller).
    pub path: PathBuf,
    /// The workspace id when it was last opened.
    pub workspace_id: String,
    /// The workspace name when it was last opened.
    pub name: String,
    /// Milliseconds since the Unix epoch of the last open.
    pub opened_at_ms: i64,
}

impl AppState {
    /// Records that the workspace at `path` was opened at `opened_at_ms`: inserts it, or
    /// refreshes the id, name and time of an existing entry, then drops every entry past the
    /// newest [`RECENT_WORKSPACES_MAX`]. Pass a canonicalized absolute path.
    ///
    /// # Errors
    ///
    /// `path` is not valid UTF-8 ([`StateError::NonUtf8Path`], nothing is stored), or the
    /// write failed.
    pub fn record_recent_workspace(
        &mut self,
        path: &Path,
        workspace_id: &str,
        name: &str,
        opened_at_ms: i64,
    ) -> Result<(), StateError> {
        let text = self.path_text(path)?.to_owned();
        let limit = i64::try_from(RECENT_WORKSPACES_MAX).unwrap_or(i64::MAX);
        let transaction = self.conn.transaction().map_err(query_error(&self.path))?;
        transaction
            .execute(
                "INSERT INTO recent_workspaces (path, workspace_id, name, opened_at)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT (path) DO UPDATE SET
                     workspace_id = excluded.workspace_id,
                     name = excluded.name,
                     opened_at = excluded.opened_at",
                rusqlite::params![text, workspace_id, name, opened_at_ms],
            )
            .map_err(query_error(&self.path))?;
        transaction
            .execute(
                "DELETE FROM recent_workspaces WHERE path NOT IN (
                     SELECT path FROM recent_workspaces
                     ORDER BY opened_at DESC, path LIMIT ?1
                 )",
                rusqlite::params![limit],
            )
            .map_err(query_error(&self.path))?;
        transaction.commit().map_err(query_error(&self.path))
    }

    /// The recent workspaces, newest first.
    ///
    /// # Errors
    ///
    /// The read failed.
    pub fn recent_workspaces(&self) -> Result<Vec<RecentWorkspace>, StateError> {
        let mut statement = self
            .conn
            .prepare(
                "SELECT path, workspace_id, name, opened_at FROM recent_workspaces
                 ORDER BY opened_at DESC, path",
            )
            .map_err(query_error(&self.path))?;
        let rows = statement
            .query_map([], |row| {
                Ok(RecentWorkspace {
                    path: PathBuf::from(row.get::<_, String>(0)?),
                    workspace_id: row.get(1)?,
                    name: row.get(2)?,
                    opened_at_ms: row.get(3)?,
                })
            })
            .map_err(query_error(&self.path))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(query_error(&self.path))
    }

    /// Drops the recent entry for `path`, reporting whether there was one.
    ///
    /// # Errors
    ///
    /// `path` is not valid UTF-8 ([`StateError::NonUtf8Path`]), or the write failed.
    pub fn remove_recent_workspace(&mut self, path: &Path) -> Result<bool, StateError> {
        let text = self.path_text(path)?;
        let changed = self
            .conn
            .execute(
                "DELETE FROM recent_workspaces WHERE path = ?1",
                rusqlite::params![text],
            )
            .map_err(query_error(&self.path))?;
        Ok(changed > 0)
    }

    /// `path` as text, or [`StateError::NonUtf8Path`].
    fn path_text<'a>(&self, path: &'a Path) -> Result<&'a str, StateError> {
        path.to_str().ok_or_else(|| StateError::NonUtf8Path {
            path: self.path.clone(),
            rejected: path.to_owned(),
        })
    }
}
