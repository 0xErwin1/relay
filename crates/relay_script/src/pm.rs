//! Host functions behind the `pm` shim and the context JSON the shim reads.
//!
//! Every host function locks the shared [`State`], touches it briefly and returns; none of
//! them runs JavaScript while holding the lock, so the script can call back into the host.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use rquickjs::Ctx;
use rquickjs::function::Func;

use crate::sandbox::State;
use crate::{ConsoleLevel, ScriptContext, VariableScope};

type CtxResult<T> = rquickjs::Result<T>;
type SharedState = Arc<Mutex<State>>;

pub(super) fn install(ctx: &Ctx<'_>, state: &SharedState) -> CtxResult<()> {
    let globals = ctx.globals();
    globals.set(
        "__relayVarGet",
        Func::from({
            let state = Arc::clone(state);
            move |scope: String, key: String| -> CtxResult<Option<String>> {
                Ok(lock(&state).var_get(parse_scope(&scope), &key))
            }
        }),
    )?;
    globals.set(
        "__relayVarSet",
        Func::from({
            let state = Arc::clone(state);
            move |scope: String, key: String, value: String| -> CtxResult<()> {
                lock(&state).var_set(parse_scope(&scope), key, value);
                Ok(())
            }
        }),
    )?;
    globals.set(
        "__relayVarUnset",
        Func::from({
            let state = Arc::clone(state);
            move |scope: String, key: String| -> CtxResult<()> {
                lock(&state).var_unset(parse_scope(&scope), key);
                Ok(())
            }
        }),
    )?;
    globals.set(
        "__relayTest",
        Func::from({
            let state = Arc::clone(state);
            move |name: String, passed: bool, message: Option<String>| -> CtxResult<()> {
                lock(&state).record_test(name, passed, message);
                Ok(())
            }
        }),
    )?;
    globals.set(
        "__relayConsole",
        Func::from({
            let state = Arc::clone(state);
            move |level: String, message: String| -> CtxResult<()> {
                lock(&state).record_console(parse_level(&level), message);
                Ok(())
            }
        }),
    )?;
    Ok(())
}

fn lock(state: &SharedState) -> MutexGuard<'_, State> {
    state.lock().unwrap_or_else(PoisonError::into_inner)
}

fn parse_scope(scope: &str) -> VariableScope {
    match scope {
        "environment" => VariableScope::Environment,
        "collectionVariables" => VariableScope::CollectionVariables,
        "globals" => VariableScope::Globals,
        _ => VariableScope::Variables,
    }
}

fn parse_level(level: &str) -> ConsoleLevel {
    match level {
        "info" => ConsoleLevel::Info,
        "warn" => ConsoleLevel::Warn,
        "error" => ConsoleLevel::Error,
        _ => ConsoleLevel::Log,
    }
}

/// Builds the `globalThis.__relayInit = JSON.parse("...")` source line the shim reads.
pub(super) fn init_source(context: &ScriptContext) -> String {
    let response = context.response.as_ref().map(|response| {
        let mut value = serde_json::json!({
            "code": response.code,
            "status": response.status,
            "responseTime": response.response_time_ms,
            "responseSize": response.response_size,
            "headers": response.headers,
            "_text": response.body_text,
        });
        if let Some(parsed) = &response.parsed_json
            && let Some(object) = value.as_object_mut()
        {
            object.insert("_parsed".into(), parsed.clone());
        }
        value
    });
    let init = serde_json::json!({
        "stage": context.stage.as_str(),
        "request": {
            "method": context.request.method,
            "url": context.request.url,
            "headers": context.request.headers,
            "body": context.request.body,
        },
        "response": response,
    });
    let json = serde_json::to_string(&init).unwrap_or_else(|_| "{}".into());
    format!(
        "globalThis.__relayInit = JSON.parse(\"{}\");",
        escape_js(&json)
    )
}

/// Escapes a JSON document so it is a valid double-quoted JavaScript string literal. The
/// backslash goes first so the JSON escape sequences survive as literal text for `JSON.parse`.
fn escape_js(json: &str) -> String {
    json.replace('\\', "\\\\").replace('"', "\\\"")
}
