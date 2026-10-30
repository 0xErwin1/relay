//! Environment JSON mapping (S6): `name`, `values` and the slug, unique against the
//! collection slugs of the same import.

use std::collections::BTreeSet;

use relay_core::model::{Environment, EnvironmentNode, Id};
use serde_json::Value;

use crate::collection::{environment_variables, extensions};
use crate::error::PostmanError;
use crate::slug::{slug, unique};

/// Maps a Postman environment JSON document (S6).
///
/// # Errors
///
/// Fails when the document is not an object or has no `name`.
pub fn environment(value: &Value) -> Result<EnvironmentNode, PostmanError> {
    let mut taken = BTreeSet::new();
    environment_in(value, &mut taken)
}

/// Maps an environment whose slug must not collide with `taken`, which carries the
/// collection slugs of the same import and grows with each environment (S6).
///
/// # Errors
///
/// Fails when the document is not an object or has no `name`.
pub fn environment_in(
    value: &Value,
    taken: &mut BTreeSet<String>,
) -> Result<EnvironmentNode, PostmanError> {
    let object = value.as_object().ok_or(PostmanError::NotAnObject)?;
    let name = object
        .get("name")
        .and_then(Value::as_str)
        .ok_or(PostmanError::MissingEnvironmentName)?;
    Ok(EnvironmentNode {
        slug: unique(&slug(name), taken),
        environment: Environment {
            id: Id::new(),
            name: name.to_owned(),
            variables: environment_variables(object.get("values")),
            extensions: extensions(object, &["name", "values"], &[]),
        },
    })
}
