//! Turning a workspace request into a [`PreparedRequest`] (S4-S9).

mod auth;
mod body;
mod inspect;
mod resolver;
mod url;

use std::path::Path;
use std::time::Duration;

use reqflux_core::model::{
    CollectionNode, EnvironmentNode, KeyValue, Request, Settings, Variable, Workspace,
};
use reqflux_core::template::{DynamicSource, Scope, ScopeEntry, ScopeKind, Scopes};
use reqflux_http::{PreparedRequest, RequestOptions};
use reqflux_script::VariableWrite;

use crate::error::ExecuteError;
use crate::select::{self, Target};
use crate::sent::{self, Masker, Secrets, SentRequest};
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
    /// The values history must mask; see [`Prepared::sent`].
    pub(crate) secrets: Secrets,
}

impl Prepared {
    /// The request as it will be sent, with every secret replaced by
    /// [`MASK`](crate::MASK): every value of every secret variable in scope, and the basic
    /// password, bearer token or API key value, wherever they appear in the URL, the
    /// headers and the body.
    #[must_use]
    pub fn sent(&self) -> SentRequest {
        sent::redact(&self.request, &self.masker())
    }

    /// What masks this request's secrets in any other text, such as the error of a failed
    /// send, the same way [`Prepared::sent`] masks them.
    #[must_use]
    pub fn masker(&self) -> Masker {
        self.secrets.masker()
    }
}

/// Percent-encodes `text` the way the query string carries it.
pub(crate) fn percent_encode(text: &str) -> String {
    url::encode(text)
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
    prepare_request(
        workspace,
        &target,
        &target.request.request,
        root,
        options,
        dynamic,
        &[],
    )
}

/// Prepares `draft` in the place of the request `selector` selects, without any IO
/// (request-editing S5).
///
/// The draft replaces only the request document. Its parents are the ones the workspace has
/// for the selected request, so the collection variables, the inherited folder and
/// collection auth, the environment allowlist and the settings defaults behave exactly as
/// for the stored request. Resolution works as in [`prepare`].
///
/// # Errors
///
/// The errors of [`prepare`] for the same `workspace`, `selector`, `options` and `draft`.
pub fn prepare_draft(
    workspace: &Workspace,
    root: &Path,
    selector: &str,
    draft: &Request,
    options: &ExecuteOptions,
    dynamic: &mut impl DynamicSource,
) -> Result<Prepared, ExecuteError> {
    let target = select::find(workspace, selector)?;
    prepare_request(workspace, &target, draft, root, options, dynamic, &[])
}

/// Prepares the request `target` selects with `request` as its document, applying the
/// variable writes a pre-request script made to this run's scopes (`S7`). The writes are
/// never persisted; they exist for this preparation only.
///
/// The errors of [`prepare`] for the same `workspace`, `target`, `options` and `request`.
pub(crate) fn prepare_target(
    workspace: &Workspace,
    target: &Target<'_>,
    request: &Request,
    root: &Path,
    options: &ExecuteOptions,
    dynamic: &mut impl DynamicSource,
    script_writes: &[VariableWrite],
) -> Result<Prepared, ExecuteError> {
    prepare_request(
        workspace,
        target,
        request,
        root,
        options,
        dynamic,
        script_writes,
    )
}

/// The one resolution path; `request` is the document to prepare, stored or draft, and
/// `target` supplies the parents the scopes, allowlist and inherited auth come from.
fn prepare_request(
    workspace: &Workspace,
    target: &Target<'_>,
    request: &Request,
    root: &Path,
    options: &ExecuteOptions,
    dynamic: &mut impl DynamicSource,
    script_writes: &[VariableWrite],
) -> Result<Prepared, ExecuteError> {
    let environment = select_environment(workspace, target.collection, options)?;
    let names = secret_names(workspace, target.collection, environment);
    let mut scope_map = scopes(workspace, target.collection, environment, options);
    crate::scripts::apply_writes(&mut scope_map, script_writes);
    let mut resolver = Resolver::new(scope_map, dynamic);

    let base = resolver.text(&request.url);
    let mut query = resolver.pairs(&request.params);
    let mut headers = resolver.pairs(&request.headers);
    let auth = auth::effective(request, target);
    let mut secrets = Secrets::default();
    auth::apply(auth, &mut resolver, &mut headers, &mut query, &mut secrets)?;
    let body = body::prepare(&request.body, root, &headers, &mut resolver)?;
    resolver.add_secret_values(&names, &mut secrets);

    Ok(Prepared {
        request: PreparedRequest {
            method: request.method.as_str().to_owned(),
            url: url::with_query(base, &query),
            headers,
            body,
            options: request_options(request.settings),
        },
        unresolved: resolver.into_unresolved(),
        secrets,
    })
}

/// The names of the secret variables in the scopes a request resolves with: the globals,
/// its collection and the selected environment.
fn secret_names<'a>(
    workspace: &'a Workspace,
    collection: &'a CollectionNode,
    environment: Option<&'a EnvironmentNode>,
) -> Vec<&'a str> {
    [
        workspace.manifest.globals.as_slice(),
        collection.collection.variables.as_slice(),
        environment.map_or(&[][..], |environment| {
            environment.environment.variables.as_slice()
        }),
    ]
    .into_iter()
    .flatten()
    .filter(|variable| variable.secret)
    .map(|variable| variable.key.as_str())
    .collect()
}

/// The environment `options` selects, checked against the collection allowlist (S5).
pub(crate) fn select_environment<'a>(
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
pub(crate) fn scopes(
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

/// The transport settings of one request: the request's own, with the workspace defaults
/// filled in. Shared so a script's `pm.sendRequest` respects the same settings (S13).
pub(crate) fn request_options(settings: Settings) -> RequestOptions {
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
