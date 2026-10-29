//! What a front end shows about a request before it is sent, with no environment selected.

use std::path::Path;

use relay_core::model::{RequestNode, Variable, Workspace};
use relay_core::template::{DynamicVariable, Scopes, Segment, SequentialDynamicSource, segments};

use super::{ExecuteOptions, enabled, prepare, scopes, url};
use crate::error::ExecuteError;
use crate::select;

/// A request as a front end shows it before sending it, with no environment selected.
#[derive(Debug, Clone, PartialEq)]
pub struct Inspection {
    /// The selected request with its scripts, as stored.
    pub request: RequestNode,
    /// The request URL as written, with the enabled params appended as written after the
    /// separator [`prepare`] uses.
    pub url: String,
    /// The names [`prepare`] reports unresolved with no environment, in its order. Empty
    /// when the request cannot be prepared, for example with an auth type the engine does
    /// not send yet.
    pub unresolved: Vec<String>,
    scopes: Scopes,
    secrets: Vec<String>,
}

/// A run of a text split by [`Inspection::spans`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Span {
    /// Text that is not a reference.
    Literal(String),
    /// A `{{name}}` reference.
    Reference {
        /// The whole `{{...}}` as written.
        raw: String,
        /// The trimmed name.
        name: String,
        /// Whether the name resolves with no environment: a variable of the globals or the
        /// collection, or a dynamic variable such as `$guid`.
        resolves: bool,
        /// Whether a variable with this name is secret in the globals, the collection or an
        /// environment the collection allows.
        secret: bool,
    },
}

impl Inspection {
    /// Splits `text` into literal runs and references; concatenating the runs yields `text`.
    #[must_use]
    pub fn spans(&self, text: &str) -> Vec<Span> {
        segments(text)
            .map(|segment| match segment {
                Segment::Literal(literal) => Span::Literal(literal.to_owned()),
                Segment::Reference { raw, name } => Span::Reference {
                    raw: raw.to_owned(),
                    name: name.to_owned(),
                    resolves: self.scopes.lookup(name).is_some()
                        || DynamicVariable::parse(name).is_some(),
                    secret: self.secrets.iter().any(|secret| secret == name),
                },
            })
            .collect()
    }

    /// Whether `text` references a secret variable.
    #[must_use]
    pub fn references_secret(&self, text: &str) -> bool {
        self.spans(text)
            .iter()
            .any(|span| matches!(span, Span::Reference { secret: true, .. }))
    }
}

/// Inspects the request `selector` of `workspace`, whose file paths resolve against `root`,
/// without any IO.
///
/// # Errors
///
/// Fails when no request matches `selector`.
pub fn inspect(
    workspace: &Workspace,
    root: &Path,
    selector: &str,
) -> Result<Inspection, ExecuteError> {
    let target = select::find(workspace, selector)?;
    let request = &target.request.request;
    let collection = &target.collection.collection;

    let params: Vec<(String, String)> = enabled(&request.params)
        .map(|row| (row.key.clone(), row.value.clone()))
        .collect();
    let options = ExecuteOptions::default();
    let unresolved = prepare(
        workspace,
        root,
        selector,
        &options,
        &mut SequentialDynamicSource::default(),
    )
    .map(|prepared| prepared.unresolved)
    .unwrap_or_default();

    let environments = workspace
        .environments
        .iter()
        .filter(|environment| collection.allows_environment(environment.environment.id))
        .map(|environment| environment.environment.variables.as_slice());
    let secrets = [
        workspace.manifest.globals.as_slice(),
        collection.variables.as_slice(),
    ]
    .into_iter()
    .chain(environments)
    .flatten()
    .filter(|variable| variable.secret)
    .map(|variable: &Variable| variable.key.clone())
    .collect();

    Ok(Inspection {
        request: target.request.clone(),
        url: url::append(request.url.clone(), &params),
        unresolved,
        scopes: scopes(workspace, target.collection, None, &options),
        secrets,
    })
}
