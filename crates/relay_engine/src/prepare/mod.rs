//! Turning a workspace request into a [`PreparedRequest`] (S4-S9).

mod auth;
mod body;
mod inspect;
mod resolver;
mod url;

use std::path::Path;
use std::time::Duration;

use relay_core::model::{CollectionNode, EnvironmentNode, KeyValue, Settings, Variable, Workspace};
use relay_core::template::{DynamicSource, Scope, ScopeEntry, ScopeKind, Scopes};
use relay_http::{PreparedRequest, RequestOptions};

use crate::error::ExecuteError;
use crate::select;
use resolver::Resolver;

pub use inspect::{Inspection, Span, inspect};

/// The timeout used when a request does not set one, in milliseconds.
pub const DEFAULT_TIMEOUT_MS: u64 = 30_000;

/// What the caller adds to a request execution.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExecuteOptions {
    /// The slug of the workspace environment to use, if any.
    pub environment: Option<String>,
    /// Local variables, the highest-precedence scope, in order; a later key wins.
    pub vars: Vec<(String, String)>,
}

/// A request ready to send.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prepared {
    /// The resolved request.
    pub request: PreparedRequest,
    /// Variable names that did not resolve, unique, in first-occurrence order. Their
    /// references are sent as literal text.
    pub unresolved: Vec<String>,
}

/// Prepares the request `selector` of `workspace` without any IO.
///
/// Variables resolve in the URL, then enabled params, enabled headers, the effective auth and
/// the body, which is the order unresolved names are reported in. Disabled rows are skipped
/// without resolving them.
///
/// `root` is the workspace root that relative file paths of bodies resolve against, and
/// `dynamic` produces the values of dynamic variables such as `{{$guid}}`.
///
/// # Errors
///
/// Fails when no request matches, when the environment is unknown or not allowed for the
/// request's collection, when the effective auth is not supported, or when GraphQL variables
/// are not valid JSON.
pub fn prepare(
    workspace: &Workspace,
    root: &Path,
    selector: &str,
    options: &ExecuteOptions,
    dynamic: &mut impl DynamicSource,
) -> Result<Prepared, ExecuteError> {
    let target = select::find(workspace, selector)?;
    let environment = select_environment(workspace, target.collection, options)?;
    let mut resolver = Resolver::new(
        scopes(workspace, target.collection, environment, options),
        dynamic,
    );
    let request = &target.request.request;

    let base = resolver.text(&request.url);
    let mut query = resolver.pairs(&request.params);
    let mut headers = resolver.pairs(&request.headers);
    let auth = auth::effective(&target);
    auth::apply(auth, &mut resolver, &mut headers, &mut query)?;
    let body = body::prepare(&request.body, root, &headers, &mut resolver)?;

    Ok(Prepared {
        request: PreparedRequest {
            method: request.method.as_str().to_owned(),
            url: url::with_query(base, &query),
            headers,
            body,
            options: request_options(request.settings),
        },
        unresolved: resolver.into_unresolved(),
    })
}

/// The environment `options` selects, checked against the collection allowlist (S5).
fn select_environment<'a>(
    workspace: &'a Workspace,
    collection: &CollectionNode,
    options: &ExecuteOptions,
) -> Result<Option<&'a EnvironmentNode>, ExecuteError> {
    let Some(slug) = options.environment.as_deref() else {
        return Ok(None);
    };
    let environment = workspace
        .environments
        .iter()
        .find(|environment| environment.slug == slug)
        .ok_or_else(|| ExecuteError::UnknownEnvironment(slug.to_owned()))?;
    if !collection
        .collection
        .allows_environment(environment.environment.id)
    {
        return Err(ExecuteError::EnvironmentNotAllowed {
            environment: slug.to_owned(),
            collection: collection.collection.name.clone(),
        });
    }
    Ok(Some(environment))
}

/// Globals < collection < environment < local `--var`s (S4).
pub(super) fn scopes(
    workspace: &Workspace,
    collection: &CollectionNode,
    environment: Option<&EnvironmentNode>,
    options: &ExecuteOptions,
) -> Scopes {
    let mut scopes = Scopes::default();
    *scopes.scope_mut(ScopeKind::Globals) = scope(&workspace.manifest.globals);
    *scopes.scope_mut(ScopeKind::Collection) = scope(&collection.collection.variables);
    if let Some(environment) = environment {
        *scopes.scope_mut(ScopeKind::Environment) = scope(&environment.environment.variables);
    }
    *scopes.scope_mut(ScopeKind::Local) = options
        .vars
        .iter()
        .map(|(key, value)| ScopeEntry::new(key.clone(), value.clone(), true))
        .collect();
    scopes
}

/// A secret with no value is left out so it does not resolve; any other variable with no
/// value resolves to empty text.
fn scope(variables: &[Variable]) -> Scope {
    variables
        .iter()
        .filter_map(|variable| match (&variable.value, variable.secret) {
            (None, true) => None,
            (value, _) => Some(ScopeEntry::new(
                variable.key.clone(),
                value.clone().unwrap_or_default(),
                variable.enabled,
            )),
        })
        .collect()
}

fn request_options(settings: Settings) -> RequestOptions {
    RequestOptions {
        timeout: Duration::from_millis(settings.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS)),
        follow_redirects: settings.follow_redirects.unwrap_or(true),
        verify_tls: settings.verify_tls.unwrap_or(true),
    }
}

/// The enabled rows of a key/value table.
pub(super) fn enabled(rows: &[KeyValue]) -> impl Iterator<Item = &KeyValue> {
    rows.iter().filter(|row| row.enabled)
}

/// Whether `headers` has one named `name`, compared case-insensitively.
fn has_header(headers: &[(String, String)], name: &str) -> bool {
    headers
        .iter()
        .any(|(key, _)| key.eq_ignore_ascii_case(name))
}
