//! The secret store values one preparation resolves with (keyring-secrets S2, S3).

use std::collections::HashMap;

use reqflux_core::model::{CollectionNode, EnvironmentNode, Variable, Workspace};

use crate::secrets::{SecretError, SecretKey, SecretScope, SecretStore};

/// The stored values of the secret variables that have no value in the workspace files, read
/// once per preparation, and why some could not be read.
#[derive(Debug, Default)]
pub(crate) struct StoredSecrets {
    values: HashMap<(SecretScope, String), String>,
    /// Why secrets were not read from the store, reported once per preparation.
    pub(crate) problem: Option<String>,
}

impl StoredSecrets {
    /// Reads every valueless secret of the globals, `collection` and `environment` from
    /// `store`. A store that is unavailable stops the reads and leaves every secret unset; a
    /// failed read leaves that secret unset. Either is recorded in `problem`, never as an
    /// error, so the send still prepares.
    pub(crate) fn read(
        store: &dyn SecretStore,
        workspace: &Workspace,
        collection: &CollectionNode,
        environment: Option<&EnvironmentNode>,
    ) -> Self {
        let workspace_id = workspace.manifest.id;
        let scopes = [
            (SecretScope::Global, workspace.manifest.globals.as_slice()),
            (
                SecretScope::Collection(collection.collection.id),
                collection.collection.variables.as_slice(),
            ),
        ]
        .into_iter()
        .chain(environment.map(|environment| {
            (
                SecretScope::Environment(environment.environment.id),
                environment.environment.variables.as_slice(),
            )
        }));
        let mut stored = Self::default();
        for (scope, variables) in scopes {
            for variable in variables
                .iter()
                .filter(|variable| is_valueless_secret(variable))
            {
                let key = SecretKey {
                    workspace_id,
                    scope,
                    key: variable.key.clone(),
                };
                match store.get(&key) {
                    Ok(Some(value)) => {
                        stored.values.insert((scope, key.key), value);
                    }
                    Ok(None) => {}
                    Err(error) => {
                        let unavailable = matches!(error, SecretError::Unavailable(_));
                        stored.problem.get_or_insert_with(|| {
                            format!("secret variables were not read from the keyring: {error}")
                        });
                        if unavailable {
                            return stored;
                        }
                    }
                }
            }
        }
        stored
    }

    /// The stored value of the secret `key` declared in `scope`, if any.
    pub(crate) fn value(&self, scope: SecretScope, key: &str) -> Option<&str> {
        self.values
            .get(&(scope, key.to_owned()))
            .map(String::as_str)
    }
}

/// Whether `variable` is a secret whose value must come from the store.
fn is_valueless_secret(variable: &Variable) -> bool {
    variable.secret && variable.value.is_none()
}
