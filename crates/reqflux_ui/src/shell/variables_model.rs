//! The Variables panel and document without GPUI (variables-editor S6): the scopes the panel
//! lists, the variables a scope declares, the key a new variable gets and the current values
//! a scope holds for keys it does not declare.

use reqflux_core::model::{Variable, Workspace};
use reqflux_engine::{CurrentValues, SecretScope};

/// The key a new variable starts with, numbered when taken.
pub(crate) const NEW_KEY: &str = "new_variable";

/// One row of the Variables panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ScopeRow {
    pub(crate) scope: SecretScope,
    pub(crate) name: String,
}

impl ScopeRow {
    /// The element id of the row: `variables-scope-globals`,
    /// `variables-scope-collection-<id>` or `variables-scope-environment-<id>`.
    pub(crate) fn element_id(&self) -> String {
        match self.scope {
            SecretScope::Global => "variables-scope-globals".to_owned(),
            SecretScope::Collection(id) => format!("variables-scope-collection-{id}"),
            SecretScope::Environment(id) => format!("variables-scope-environment-{id}"),
        }
    }
}

/// The rows of the panel: Globals, the collections in workspace order, then the
/// environments by name.
pub(crate) fn scope_rows(workspace: &Workspace) -> Vec<ScopeRow> {
    let mut environments: Vec<_> = workspace
        .environments
        .iter()
        .map(|node| ScopeRow {
            scope: SecretScope::Environment(node.environment.id),
            name: node.environment.name.clone(),
        })
        .collect();
    environments.sort_by(|left, right| left.name.cmp(&right.name));
    std::iter::once(ScopeRow {
        scope: SecretScope::Global,
        name: "Globals".to_owned(),
    })
    .chain(workspace.collections.iter().map(|node| ScopeRow {
        scope: SecretScope::Collection(node.collection.id),
        name: node.collection.name.clone(),
    }))
    .chain(environments)
    .collect()
}

/// The variables `scope` declares, in file order; `None` when the scope no longer exists.
pub(crate) fn scope_variables(workspace: &Workspace, scope: SecretScope) -> Option<&[Variable]> {
    match scope {
        SecretScope::Global => Some(&workspace.manifest.globals),
        SecretScope::Collection(id) => workspace
            .collections
            .iter()
            .find(|node| node.collection.id == id)
            .map(|node| node.collection.variables.as_slice()),
        SecretScope::Environment(id) => workspace
            .environments
            .iter()
            .find(|node| node.environment.id == id)
            .map(|node| node.environment.variables.as_slice()),
    }
}

/// The title of the document of `scope`: `Globals`, `Collection <name>` or
/// `Environment <name>`.
pub(crate) fn scope_title(workspace: &Workspace, scope: SecretScope) -> Option<String> {
    let row = scope_rows(workspace)
        .into_iter()
        .find(|row| row.scope == scope)?;
    Some(match scope {
        SecretScope::Global => row.name,
        SecretScope::Collection(_) => format!("Collection {}", row.name),
        SecretScope::Environment(_) => format!("Environment {}", row.name),
    })
}

/// [`NEW_KEY`], or `new_variable_<n>` with the first `n` from 2 that `variables` does not
/// use.
pub(crate) fn new_key(variables: &[Variable]) -> String {
    let taken = |key: &str| variables.iter().any(|variable| variable.key == key);
    if !taken(NEW_KEY) {
        return NEW_KEY.to_owned();
    }
    (2..)
        .map(|n| format!("{NEW_KEY}_{n}"))
        .find(|key| !taken(key))
        .unwrap_or_default()
}

/// The current values of `scope` whose key `variables` does not declare, as `(key, value)`
/// sorted by key: values a script set for a variable no file declares.
pub(crate) fn undeclared(
    current: &CurrentValues,
    scope: SecretScope,
    variables: &[Variable],
) -> Vec<(String, String)> {
    let mut found: Vec<_> = current
        .iter()
        .filter(|(at, key, _)| {
            *at == scope && !variables.iter().any(|variable| variable.key == *key)
        })
        .map(|(_, key, value)| (key.to_owned(), value.to_owned()))
        .collect();
    found.sort();
    found
}

/// The cell of a Variables document row that a field edits. Each is a distinct table name
/// in the shell's cell field slot, and `<table>-<n>` is the cell's element id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Column {
    Key,
    Value,
    Current,
    Description,
    /// The current value of a key the scope does not declare.
    Undeclared,
}

impl Column {
    const ALL: [Self; 5] = [
        Self::Key,
        Self::Value,
        Self::Current,
        Self::Description,
        Self::Undeclared,
    ];

    pub(crate) fn table(self) -> &'static str {
        match self {
            Self::Key => "variables-key",
            Self::Value => "variables-value",
            Self::Current => "variables-current",
            Self::Description => "variables-description",
            Self::Undeclared => "variables-undeclared-current",
        }
    }

    pub(crate) fn from_table(table: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|column| column.table() == table)
    }

    /// The element id of the cell in the 1-based row `n`.
    pub(crate) fn element_id(self, n: usize) -> String {
        format!("{}-{n}", self.table())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_key_is_numbered_when_taken() {
        assert_eq!(new_key(&[]), "new_variable");
        let taken = [
            Variable::new("new_variable", ""),
            Variable::new("new_variable_2", ""),
        ];
        assert_eq!(new_key(&taken), "new_variable_3");
    }

    #[test]
    fn undeclared_values_skip_declared_keys_and_other_scopes() {
        let mut current = CurrentValues::default();
        current.insert(SecretScope::Global, "token", "a");
        current.insert(SecretScope::Global, "base", "b");
        let declared = [Variable::new("base", "")];
        assert_eq!(
            undeclared(&current, SecretScope::Global, &declared),
            vec![("token".to_owned(), "a".to_owned())]
        );
    }

    #[test]
    fn columns_round_trip_through_their_table() {
        for column in Column::ALL {
            assert_eq!(Column::from_table(column.table()), Some(column));
        }
        assert_eq!(Column::from_table("params"), None);
    }
}
