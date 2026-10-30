//! The engine side of scripting (S7): the effective scripts, the views they read and the
//! variable writes they leave behind.
//!
//! The engine picks the most specific script that exists — the request's, else the
//! innermost folder's, else the collection's — runs it through [`relay_script`] and applies
//! its writes to the execution's scopes for this run only, never to disk.

use std::collections::{BTreeMap, BTreeSet};

use relay_core::model::{Body, Request};
use relay_core::template::{ScopeEntry, ScopeKind, Scopes};
use relay_http::Response;
use relay_script::{
    ConsoleEntry, ConsoleLevel, RequestView, ResponseView, ScriptContext, ScriptOutcome,
    ScriptStage, VariableScope, VariableWrite,
};

use crate::select::Target;

/// The effective pre-request script of `target`: the request's, else the innermost folder's
/// that has one, else the collection's.
pub(crate) fn effective_pre_request<'a>(target: &Target<'a>) -> Option<&'a str> {
    target
        .request
        .scripts
        .pre_request
        .as_deref()
        .or_else(|| {
            target
                .folders
                .iter()
                .rev()
                .find_map(|folder| folder.scripts.pre_request.as_deref())
        })
        .or(target.collection.scripts.pre_request.as_deref())
}

/// The effective test script of `target`, resolved like the pre-request one.
pub(crate) fn effective_test<'a>(target: &Target<'a>) -> Option<&'a str> {
    target
        .request
        .scripts
        .test
        .as_deref()
        .or_else(|| {
            target
                .folders
                .iter()
                .rev()
                .find_map(|folder| folder.scripts.test.as_deref())
        })
        .or(target.collection.scripts.test.as_deref())
}

/// The `pm.request` view of `request`: the stored document, before variable resolution.
pub(crate) fn request_view(request: &Request) -> RequestView {
    let headers = request
        .headers
        .iter()
        .filter(|row| row.enabled)
        .map(|row| (row.key.clone(), row.value.clone()))
        .collect();
    let body = match &request.body {
        Body::Raw { text, .. } => Some(text.clone()),
        _ => None,
    };
    RequestView {
        method: request.method.as_str().to_owned(),
        url: request.url.clone(),
        headers,
        body,
    }
}

/// The `pm.response` view of the response. The body decodes lossily; the JSON parse is best
/// effort, because the shim falls back to parsing the text.
pub(crate) fn response_view(response: &Response) -> ResponseView {
    let headers = response
        .headers
        .iter()
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect();
    ResponseView {
        code: response.status,
        status: response.reason.clone().unwrap_or_default(),
        response_time_ms: u64::try_from(response.elapsed.as_millis()).unwrap_or(u64::MAX),
        response_size: u64::try_from(response.size()).unwrap_or(u64::MAX),
        headers,
        body_text: String::from_utf8_lossy(&response.body).into_owned(),
        parsed_json: serde_json::from_slice(&response.body).unwrap_or(None),
    }
}

/// The merged variable view scripts read: one entry per name at the value of its
/// highest-precedence scope.
pub(crate) fn merged(scopes: &Scopes) -> BTreeMap<String, String> {
    let mut names = BTreeSet::new();
    for kind in ScopeKind::ALL {
        for entry in &scopes.scope(kind).entries {
            names.insert(entry.key.clone());
        }
    }
    let mut merged = BTreeMap::new();
    for name in names {
        if let Some(value) = scopes.lookup(&name) {
            merged.insert(name, value.to_owned());
        }
    }
    merged
}

/// Applies the writes a script made to this run's scopes: a write replaces the entries the
/// scope has for its key, an unset removes them. Nothing reaches the workspace.
pub(crate) fn apply_writes(scopes: &mut Scopes, writes: &[VariableWrite]) {
    for write in writes {
        let scope = scopes.scope_mut(kind_of(write.scope));
        if let Some(value) = &write.value {
            scope
                .entries
                .push(ScopeEntry::new(write.key.clone(), value.clone(), true));
        } else {
            scope.entries.retain(|entry| entry.key != write.key);
        }
    }
}

/// The scope kind a script scope writes to; `pm.variables` is the local scope.
fn kind_of(scope: VariableScope) -> ScopeKind {
    match scope {
        VariableScope::Variables => ScopeKind::Local,
        VariableScope::Environment => ScopeKind::Environment,
        VariableScope::CollectionVariables => ScopeKind::Collection,
        VariableScope::Globals => ScopeKind::Globals,
    }
}

/// Runs `script` with `context`, reporting a failure as an error console entry in the
/// outcome it collected so far: a script error never fails the request.
pub(crate) fn run_bounded(script: &str, context: &ScriptContext) -> ScriptOutcome {
    match relay_script::run(script, context) {
        Ok(outcome) => outcome,
        Err(error) => {
            let message = error.to_string();
            let mut outcome = error.outcome;
            outcome.console.push(ConsoleEntry {
                level: ConsoleLevel::Error,
                message,
            });
            outcome
        }
    }
}

/// Runs the effective script of `stage` and returns everything it produced.
///
/// A script error — a syntax error, a throw, a limit — never fails the request: the outcome
/// collected so far comes back with the error reported as an error console entry.
pub(crate) fn run_stage(
    stage: ScriptStage,
    target: &Target<'_>,
    request: &RequestView,
    scopes: &Scopes,
    response: Option<&Response>,
) -> ScriptOutcome {
    let script = match stage {
        ScriptStage::PreRequest => effective_pre_request(target),
        ScriptStage::Test => effective_test(target),
    };
    let Some(script) = script else {
        return ScriptOutcome::default();
    };
    let context = ScriptContext {
        stage,
        request: request.clone(),
        variables: merged(scopes),
        response: response.map(response_view),
    };
    run_bounded(script, &context)
}

/// Extends `outcome` with `other`, keeping the stage order: pre-request entries first.
pub(crate) fn merge(outcome: &mut ScriptOutcome, other: ScriptOutcome) {
    outcome.tests.extend(other.tests);
    outcome.console.extend(other.console);
    outcome.writes.extend(other.writes);
}
