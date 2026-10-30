//! The QuickJS runtime: limits, evaluation and error classification.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use rquickjs::context::EvalOptions;
use rquickjs::{Context, Ctx, Runtime};

use crate::pm;
use crate::shim::PRELUDE;
use crate::{
    ConsoleEntry, ScriptContext, ScriptError, ScriptErrorKind, ScriptOutcome, ScriptStage,
    TestResult, VariableScope, VariableWrite,
};

/// QuickJS heap cap for one script run.
const MEMORY_LIMIT_BYTES: usize = 64 * 1024 * 1024;
/// Wall-clock cap enforced by the QuickJS interrupt handler.
const TIME_LIMIT: Duration = Duration::from_secs(5);

/// The [`State`] shared between the host functions and the sandbox driver.
type SharedState = Arc<Mutex<State>>;

/// Everything the host collects during one script run.
pub(super) struct State {
    variables: BTreeMap<String, String>,
    /// Writes and unsets applied during the run, checked before `variables` on a read.
    overlay: HashMap<(VariableScope, String), Option<String>>,
    writes: Vec<VariableWrite>,
    tests: Vec<TestResult>,
    console: Vec<ConsoleEntry>,
}

impl State {
    pub(super) fn new(context: &ScriptContext) -> Self {
        Self {
            variables: context.variables.clone(),
            overlay: HashMap::new(),
            writes: Vec::new(),
            tests: Vec::new(),
            console: Vec::new(),
        }
    }

    pub(super) fn var_get(&self, scope: VariableScope, key: &str) -> Option<String> {
        match self.overlay.get(&(scope, key.to_string())) {
            Some(Some(value)) => Some(value.clone()),
            Some(None) => None,
            None => self.variables.get(key).cloned(),
        }
    }

    pub(super) fn var_set(&mut self, scope: VariableScope, key: String, value: String) {
        self.writes.push(VariableWrite {
            scope,
            key: key.clone(),
            value: Some(value.clone()),
        });
        self.overlay.insert((scope, key), Some(value));
    }

    pub(super) fn var_unset(&mut self, scope: VariableScope, key: String) {
        self.writes.push(VariableWrite {
            scope,
            key: key.clone(),
            value: None,
        });
        self.overlay.insert((scope, key), None);
    }

    pub(super) fn record_test(&mut self, name: String, passed: bool, message: Option<String>) {
        self.tests.push(TestResult {
            name,
            passed,
            message,
        });
    }

    pub(super) fn record_console(&mut self, level: crate::ConsoleLevel, message: String) {
        self.console.push(ConsoleEntry { level, message });
    }

    pub(super) fn outcome(&self) -> ScriptOutcome {
        ScriptOutcome {
            tests: self.tests.clone(),
            console: self.console.clone(),
            writes: self.writes.clone(),
        }
    }
}

pub(super) fn run(script: &str, context: &ScriptContext) -> Result<ScriptOutcome, ScriptError> {
    let runtime = Runtime::new().map_err(|error| {
        internal(
            context.stage,
            format!("creating the runtime failed: {error}"),
        )
    })?;
    runtime.set_memory_limit(MEMORY_LIMIT_BYTES);
    let deadline = Instant::now() + TIME_LIMIT;
    // In rquickjs the handler returns `true` when the run must stop.
    runtime.set_interrupt_handler(Some(Box::new(move || Instant::now() >= deadline)));
    let js_context = Context::full(&runtime).map_err(|error| {
        internal(
            context.stage,
            format!("creating the context failed: {error}"),
        )
    })?;
    js_context.with(|ctx| execute(&ctx, script, context))
}

fn execute(
    ctx: &Ctx<'_>,
    script: &str,
    context: &ScriptContext,
) -> Result<ScriptOutcome, ScriptError> {
    let state: SharedState = Arc::new(Mutex::new(State::new(context)));
    pm::install(ctx, &state).map_err(|error| {
        internal(
            context.stage,
            format!("installing the pm shim failed: {error}"),
        )
    })?;
    eval_source(ctx, &pm::init_source(context), context, &state)?;
    eval_source(ctx, PRELUDE, context, &state)?;
    eval_source(ctx, script, context, &state)?;
    Ok(lock(&state).outcome())
}

fn lock(state: &SharedState) -> MutexGuard<'_, State> {
    state.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Evaluates one source in the sandbox, mapping a failure to a [`ScriptError`] that carries
/// the outcome collected so far.
fn eval_source(
    ctx: &Ctx<'_>,
    source: &str,
    context: &ScriptContext,
    state: &SharedState,
) -> Result<(), ScriptError> {
    let mut options = EvalOptions::default();
    options.strict = false;
    options.filename = Some("script".into());
    ctx.eval_with_options::<(), _>(source, options)
        .map_err(|error| {
            let exception = ctx.catch();
            let message = exception_message(&exception);
            let kind = classify(&error, &message).unwrap_or(ScriptErrorKind::Thrown);
            ScriptError {
                kind,
                stage: context.stage,
                message,
                outcome: current_outcome(state),
            }
        })
}

/// Maps a QuickJS failure to a failure kind from the exception text and the host error.
fn classify(error: &rquickjs::Error, message: &str) -> Option<ScriptErrorKind> {
    let lower = message.to_ascii_lowercase();
    if matches!(error, rquickjs::Error::Allocation) || lower.contains("out of memory") {
        return Some(ScriptErrorKind::MemoryLimit);
    }
    if lower.contains("interrupted") {
        return Some(ScriptErrorKind::Timeout);
    }
    if lower.contains("syntaxerror") {
        return Some(ScriptErrorKind::Syntax);
    }
    if matches!(error, rquickjs::Error::Exception) {
        return Some(ScriptErrorKind::Thrown);
    }
    None
}

/// Reads the message of a caught exception: its text, its `message`, its `name`, or a fallback.
fn exception_message(exception: &rquickjs::Value<'_>) -> String {
    if exception.type_of() == rquickjs::Type::String
        && let Some(text) = exception.as_string()
        && let Ok(text) = text.to_string()
    {
        return text;
    }
    if let Some(object) = exception.as_object() {
        let name = object.get::<_, Option<String>>("name").ok().flatten();
        let message = object.get::<_, Option<String>>("message").ok().flatten();
        match (name, message) {
            (Some(name), Some(message)) if !name.is_empty() => return format!("{name}: {message}"),
            (None, Some(message)) => return message,
            (Some(name), None) if !name.is_empty() => return name,
            _ => {}
        }
    }
    "unknown error".into()
}

fn current_outcome(state: &SharedState) -> ScriptOutcome {
    lock(state).outcome()
}

fn internal(stage: ScriptStage, message: String) -> ScriptError {
    ScriptError {
        kind: ScriptErrorKind::Internal,
        stage,
        message,
        outcome: ScriptOutcome::default(),
    }
}
