//! The [`Engine`]: a loaded workspace, a Tokio runtime and an HTTP client.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use relay_core::model::{Request, Workspace};
use relay_core::template::SystemDynamicSource;
use relay_http::{HttpClient, HttpError, PreparedRequest, Response};
use relay_script::{RequestView, ScriptContext, ScriptOutcome, ScriptStage};
use relay_store::{ContentHash, Loaded, StoreError, Warning};
use tokio::runtime::Runtime;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::error::{ExecuteError, OpenError, SaveError};
use crate::prepare::{
    ExecuteOptions, Inspection, Prepared, inspect, prepare, prepare_draft, prepare_target, scopes,
    select_environment,
};
use crate::scripts as engine_scripts;

/// A loaded workspace that executes its requests. See the crate docs for the threading
/// model.
#[derive(Debug)]
pub struct Engine {
    runtime: Runtime,
    client: HttpClient,
    root: PathBuf,
    loaded: Loaded,
}

/// A request in flight.
#[derive(Debug)]
pub struct Execution {
    /// The HTTP method sent.
    pub method: String,
    /// The URL sent, query string included.
    pub url: String,
    /// Variable names that did not resolve, in first-occurrence order.
    pub unresolved: Vec<String>,
    /// Cancels this request only; the task then ends with [`HttpError::Cancelled`].
    pub cancel: CancellationToken,
    /// The task sending the request. Await it from any executor when only the response
    /// matters; an async front end that also wants the script results calls
    /// [`Execution::finish`] instead.
    pub handle: JoinHandle<Result<Response, HttpError>>,
    /// What the effective pre-request script produced before the request was prepared.
    /// Empty when the caller used [`Engine::send`] directly, which skips scripts.
    pub scripts: ScriptOutcome,
    /// What [`Execution::complete`] needs to run the effective test script once the
    /// response arrives; `None` when the caller used [`Engine::send`] directly.
    scripted: Option<Box<Scripted>>,
}

/// What [`Execution::complete`] needs to run the effective test script after the response,
/// resolved once when the execution started so finishing needs no engine borrow.
#[derive(Debug)]
struct Scripted {
    /// The effective test script source; `None` when the request has none to run.
    script: Option<String>,
    /// The `pm.request` view the pre-request stage used, the document that was sent.
    request: RequestView,
    /// The merged variables the test script reads, the pre-request writes applied.
    variables: BTreeMap<String, String>,
    /// What the script's own `pm.sendRequest` sends through (S13).
    transport: engine_scripts::Transport,
}

impl Scripted {
    /// Runs the effective test script with `response` in `pm.response` (S7). A script
    /// error is an error console entry in the outcome, never a failed request.
    fn run(self, response: &Response) -> ScriptOutcome {
        let Some(script) = self.script else {
            return ScriptOutcome::default();
        };
        let context = ScriptContext {
            stage: ScriptStage::Test,
            request: self.request,
            variables: self.variables,
            response: Some(engine_scripts::response_view(response)),
            send_request: None,
        };
        engine_scripts::run_off_thread(&script, context, self.transport)
    }
}

impl Execution {
    /// Awaits the response and runs the effective test script with it in `pm.response`
    /// (S7), so an async front end that owns the execution needs no engine borrow. This is
    /// what a front end calls instead of awaiting `handle` directly, which returns a
    /// response without the test results. The test script does not run when the request
    /// produced no response — a transport failure comes back alone and carries no results.
    ///
    /// # Errors
    ///
    /// The transport error, or the execution task panicking or being aborted.
    pub async fn finish(self) -> Result<Executed, HttpError> {
        let Execution {
            method,
            url,
            unresolved,
            handle,
            scripts,
            scripted,
            ..
        } = self;
        let response = handle
            .await
            .map_err(|error| HttpError::Request(error.to_string()))??;
        Ok(finalize(
            method, url, unresolved, scripts, scripted, response,
        ))
    }
}

/// Assembles `Executed`: the response beside the results of the effective test script,
/// pre-request entries first.
fn finalize(
    method: String,
    url: String,
    unresolved: Vec<String>,
    mut scripts: ScriptOutcome,
    scripted: Option<Box<Scripted>>,
    response: Response,
) -> Executed {
    if let Some(scripted) = scripted {
        let outcome = scripted.run(&response);
        engine_scripts::merge(&mut scripts, outcome);
    }
    Executed {
        method,
        url,
        unresolved,
        response,
        scripts,
    }
}

/// A finished request execution.
#[derive(Debug, Clone, PartialEq)]
pub struct Executed {
    /// The HTTP method sent.
    pub method: String,
    /// The URL sent, query string included.
    pub url: String,
    /// Variable names that did not resolve, in first-occurrence order.
    pub unresolved: Vec<String>,
    /// The response.
    pub response: Response,
    /// What the effective pre-request and test scripts produced: test results, console
    /// entries and variable writes, pre-request entries first. A failing test or a script
    /// error never fails the request (S7); script errors are reported as error console
    /// entries here.
    pub scripts: ScriptOutcome,
}

impl Engine {
    /// Loads the workspace at `root` and starts the runtime.
    ///
    /// # Errors
    ///
    /// Fails when the workspace does not load or the runtime cannot start.
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, OpenError> {
        let root = root.into();
        let loaded = relay_store::load(&root)?;
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("relay-engine")
            .build()
            .map_err(OpenError::Runtime)?;
        Ok(Self {
            runtime,
            client: HttpClient::new(),
            root,
            loaded,
        })
    }

    /// Re-reads the workspace from disk into this engine, keeping its runtime, its client and
    /// any request already in flight on that runtime. A save uses this instead of opening a
    /// new engine, which would drop the runtime and with it the send it owns.
    ///
    /// # Errors
    ///
    /// The load error, when the workspace no longer loads; the engine keeps the workspace it
    /// had.
    pub fn reload(&mut self) -> Result<(), OpenError> {
        self.loaded = relay_store::load(&self.root)?;
        Ok(())
    }

    /// The workspace root.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The loaded workspace.
    #[must_use]
    pub fn workspace(&self) -> &Workspace {
        &self.loaded.workspace
    }

    /// The non-fatal problems found while loading, in order.
    #[must_use]
    pub fn warnings(&self) -> &[Warning] {
        &self.loaded.warnings
    }

    /// Prepares the request `selector` with fresh dynamic values. See [`prepare`].
    ///
    /// # Errors
    ///
    /// The errors of [`prepare`].
    pub fn prepare(
        &self,
        selector: &str,
        options: &ExecuteOptions,
    ) -> Result<Prepared, ExecuteError> {
        prepare(
            self.workspace(),
            &self.root,
            selector,
            options,
            &mut SystemDynamicSource,
        )
    }

    /// Prepares `draft` in the place of the request `selector` selects, keeping the parents
    /// the workspace has for it. See [`prepare_draft`].
    ///
    /// # Errors
    ///
    /// The errors of [`prepare_draft`].
    pub fn prepare_draft(
        &self,
        selector: &str,
        draft: &Request,
        options: &ExecuteOptions,
    ) -> Result<Prepared, ExecuteError> {
        prepare_draft(
            self.workspace(),
            &self.root,
            selector,
            draft,
            options,
            &mut SystemDynamicSource,
        )
    }

    /// Inspects the request `selector` against the environment slug `environment`. See
    /// [`inspect`].
    ///
    /// # Errors
    ///
    /// The errors of [`inspect`].
    pub fn inspect(
        &self,
        selector: &str,
        environment: Option<&str>,
    ) -> Result<Inspection, ExecuteError> {
        inspect(self.workspace(), &self.root, selector, environment)
    }

    /// The content hash of the request `selector`'s file as it was loaded, the base hash a
    /// draft on it carries so [`Engine::save_request`] can detect a moved file.
    ///
    /// # Errors
    ///
    /// No request has this id or slug path.
    pub fn request_base_hash(&self, selector: &str) -> Result<Option<ContentHash>, ExecuteError> {
        let target = crate::select::find(self.workspace(), selector)?;
        let file = relay_store::request_file(&self.root, &target.parent(), &target.request.slug);
        Ok(self.loaded.hash(&file))
    }

    /// Saves `request` as the document of the request `selector`'s file, atomically and
    /// hash-checked, and returns the new file hash.
    ///
    /// # Errors
    ///
    /// [`SaveError::Conflict`] when the file no longer matches `expected`: the draft is
    /// kept and the file is left as the other edit wrote it. [`SaveError::UnknownRequest`]
    /// when no request has this id or slug path. [`SaveError::Store`] for any other store
    /// error.
    pub fn save_request(
        &self,
        selector: &str,
        request: &Request,
        expected: Option<ContentHash>,
    ) -> Result<ContentHash, SaveError> {
        let target = crate::select::find(self.workspace(), selector)
            .map_err(|_| SaveError::UnknownRequest(selector.to_owned()))?;
        relay_store::save_request(
            &self.root,
            &target.parent(),
            &target.request.slug,
            request,
            expected,
        )
        .map_err(|error| match error {
            StoreError::Conflict { path } => SaveError::Conflict { path },
            other => SaveError::Store(other),
        })
    }

    /// Spawns a task on the engine runtime that sends `prepared`.
    #[must_use]
    pub fn send(&self, prepared: Prepared) -> Execution {
        let Prepared {
            request,
            unresolved,
        } = prepared;
        let PreparedRequest { method, url, .. } = &request;
        let (method, url) = (method.clone(), url.clone());
        let cancel = CancellationToken::new();
        let client = self.client.clone();
        let token = cancel.clone();
        let handle = self
            .runtime
            .spawn(async move { client.send(request, token).await });
        Execution {
            method,
            url,
            unresolved,
            cancel,
            handle,
            scripts: ScriptOutcome::default(),
            scripted: None,
        }
    }

    /// Prepares and sends the request `selector`, running its effective pre-request script
    /// first (S7). The script runs on its own thread with a current-thread runtime (S14),
    /// bounded by the script time limit; its variable writes apply to this execution's
    /// scopes only, never to disk.
    ///
    /// # Errors
    ///
    /// The errors of [`prepare`]; transport errors come out of the [`Execution`].
    pub fn execute(
        &self,
        selector: &str,
        options: &ExecuteOptions,
    ) -> Result<Execution, ExecuteError> {
        let target = crate::select::find(self.workspace(), selector)?;
        let (prepared, scripts, scripted) =
            self.prepare_scripted(&target, &target.request.request, options)?;
        Ok(self.scripted_execution(prepared, scripts, scripted))
    }

    /// Prepares and sends `draft` in the place of the request `selector` selects, so a front
    /// end sends what the user edited, running the effective pre-request script first (S7);
    /// the script sees the draft. See [`Engine::execute`].
    ///
    /// # Errors
    ///
    /// The errors of [`prepare_draft`]; transport errors come out of the [`Execution`].
    pub fn execute_draft(
        &self,
        selector: &str,
        draft: &Request,
        options: &ExecuteOptions,
    ) -> Result<Execution, ExecuteError> {
        let target = crate::select::find(self.workspace(), selector)?;
        let (prepared, scripts, scripted) = self.prepare_scripted(&target, draft, options)?;
        Ok(self.scripted_execution(prepared, scripts, scripted))
    }

    /// Attaches the script state [`Execution::complete`] needs to run the effective test
    /// script: the script source and the variables it reads, resolved when the execution
    /// starts.
    fn scripted_execution(
        &self,
        prepared: Prepared,
        scripts: ScriptOutcome,
        scripted: Scripted,
    ) -> Execution {
        let mut execution = self.send(prepared);
        execution.scripts = scripts;
        execution.scripted = Some(Box::new(scripted));
        execution
    }

    /// Runs the effective pre-request script of `target` over the scopes the execution
    /// would resolve with, then prepares `request` with the script's writes applied. The
    /// script's outcome comes back even when the script failed, beside the state the test
    /// script reads later.
    fn prepare_scripted(
        &self,
        target: &crate::select::Target<'_>,
        request: &Request,
        options: &ExecuteOptions,
    ) -> Result<(Prepared, ScriptOutcome, Scripted), ExecuteError> {
        let environment = select_environment(self.workspace(), target.collection, options)?;
        let scope_view = scopes(self.workspace(), target.collection, environment, options);
        let view = engine_scripts::request_view(request);
        // A script's `pm.sendRequest` sends through the same client and settings the real
        // request will use (S13).
        let transport = engine_scripts::Transport::new(
            self.client.clone(),
            crate::prepare::request_options(request.settings),
        );
        let outcome = engine_scripts::run_stage(
            ScriptStage::PreRequest,
            target,
            &view,
            &scope_view,
            None,
            transport.clone(),
        );
        let mut scripted = Scripted {
            script: engine_scripts::effective_test(target).map(str::to_owned),
            request: view,
            variables: BTreeMap::new(),
            transport,
        };
        {
            let mut tested = scope_view;
            engine_scripts::apply_writes(&mut tested, &outcome.writes);
            scripted.variables = engine_scripts::merged(&tested);
        }
        let prepared = prepare_target(
            self.workspace(),
            target,
            request,
            &self.root,
            options,
            &mut SystemDynamicSource,
            &outcome.writes,
        )?;
        Ok((prepared, outcome, scripted))
    }

    /// Blocks the calling thread until `execution` finishes, then runs the effective test
    /// script with the response in `pm.response` (S7) and returns everything beside the
    /// response. A failing test or a script error never fails the request. Must not be
    /// called from inside an async context; async front ends call [`Execution::finish`].
    ///
    /// # Errors
    ///
    /// The transport error, or [`ExecuteError::Task`] when the task panicked or was aborted.
    pub fn wait(&self, execution: Execution) -> Result<Executed, ExecuteError> {
        let Execution {
            method,
            url,
            unresolved,
            handle,
            scripts,
            scripted,
            ..
        } = execution;
        let response = self
            .runtime
            .block_on(handle)
            .map_err(|error| ExecuteError::Task(error.to_string()))??;
        Ok(finalize(
            method, url, unresolved, scripts, scripted, response,
        ))
    }

    /// Prepares and sends the request `selector`, blocking until the response and the
    /// effective test script have run. Must not be called from inside an async context.
    ///
    /// # Errors
    ///
    /// The errors of [`Engine::execute`] and [`Engine::wait`].
    pub fn execute_blocking(
        &self,
        selector: &str,
        options: &ExecuteOptions,
    ) -> Result<Executed, ExecuteError> {
        let execution = self.execute(selector, options)?;
        self.wait(execution)
    }
}
