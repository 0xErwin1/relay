//! The [`Engine`]: a loaded workspace, a Tokio runtime and an HTTP client.

use std::path::{Path, PathBuf};

use relay_core::model::{Request, Workspace};
use relay_core::template::SystemDynamicSource;
use relay_http::{HttpClient, HttpError, PreparedRequest, Response};
use relay_script::{RequestView, ScriptOutcome, ScriptStage, VariableWrite};
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
    /// The task sending the request. Await it from any executor.
    pub handle: JoinHandle<Result<Response, HttpError>>,
    /// What the effective pre-request script produced before the request was prepared.
    /// Empty when the caller used [`Engine::send`] directly, which skips scripts.
    pub scripts: ScriptOutcome,
    /// What [`Engine::wait`] needs to run the effective test script once the response
    /// arrives; `None` when the caller used [`Engine::send`] directly.
    scripted: Option<Box<Scripted>>,
}

/// What the engine needs to run the effective test script after the response.
#[derive(Debug)]
struct Scripted {
    /// The selector the request was executed under, re-resolved at test time.
    selector: String,
    /// The environment slug of the execution, if any.
    environment: Option<String>,
    /// The local variables of the execution.
    vars: Vec<(String, String)>,
    /// The `pm.request` view the pre-request stage used, the document that was sent.
    request: RequestView,
}

/// A finished request execution.
#[derive(Debug)]
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
    /// first (S7). The script runs on the calling thread, bounded by the script time limit;
    /// its variable writes apply to this execution's scopes only, never to disk.
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
        let (prepared, scripts, view) =
            self.prepare_scripted(&target, &target.request.request, options)?;
        Ok(self.scripted_execution(prepared, scripts, view, selector, options))
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
        let (prepared, scripts, view) = self.prepare_scripted(&target, draft, options)?;
        Ok(self.scripted_execution(prepared, scripts, view, selector, options))
    }

    /// Attaches the script state [`Engine::wait`] needs to run the effective test script.
    fn scripted_execution(
        &self,
        prepared: Prepared,
        scripts: ScriptOutcome,
        view: RequestView,
        selector: &str,
        options: &ExecuteOptions,
    ) -> Execution {
        let mut execution = self.send(prepared);
        execution.scripts = scripts;
        execution.scripted = Some(Box::new(Scripted {
            selector: selector.to_owned(),
            environment: options.environment.clone(),
            vars: options.vars.clone(),
            request: view,
        }));
        execution
    }

    /// Runs the effective pre-request script of `target` over the scopes the execution
    /// would resolve with, then prepares `request` with the script's writes applied. The
    /// script's outcome comes back even when the script failed.
    fn prepare_scripted(
        &self,
        target: &crate::select::Target<'_>,
        request: &Request,
        options: &ExecuteOptions,
    ) -> Result<(Prepared, ScriptOutcome, RequestView), ExecuteError> {
        let environment = select_environment(self.workspace(), target.collection, options)?;
        let scope_view = scopes(self.workspace(), target.collection, environment, options);
        let view = engine_scripts::request_view(request);
        let outcome =
            engine_scripts::run_stage(ScriptStage::PreRequest, target, &view, &scope_view, None);
        let prepared = prepare_target(
            self.workspace(),
            target,
            request,
            &self.root,
            options,
            &mut SystemDynamicSource,
            &outcome.writes,
        )?;
        Ok((prepared, outcome, view))
    }

    /// Blocks the calling thread until `execution` finishes, then runs the effective test
    /// script with the response in `pm.response` (S7) and returns everything beside the
    /// response. The test script reads the scopes the pre-request script left behind; a
    /// failing test or a script error never fails the request. Must not be called from
    /// inside an async context.
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
            mut scripts,
            scripted,
            ..
        } = execution;
        let response = self
            .runtime
            .block_on(handle)
            .map_err(|error| ExecuteError::Task(error.to_string()))??;
        if let Some(scripted) = scripted {
            let pre_writes = scripts.writes.clone();
            let outcome = self.run_test_script(*scripted, &response, &pre_writes)?;
            engine_scripts::merge(&mut scripts, outcome);
        }
        Ok(Executed {
            method,
            url,
            unresolved,
            response,
            scripts,
        })
    }

    /// Runs the effective test script of the request `scripted.selector` names, with the
    /// response in `pm.response` and the pre-request writes applied to the scopes.
    fn run_test_script(
        &self,
        scripted: Scripted,
        response: &Response,
        pre_writes: &[VariableWrite],
    ) -> Result<ScriptOutcome, ExecuteError> {
        let options = ExecuteOptions {
            environment: scripted.environment,
            vars: scripted.vars,
        };
        let target = crate::select::find(self.workspace(), &scripted.selector)?;
        let environment = select_environment(self.workspace(), target.collection, &options)?;
        let mut scope_view = scopes(self.workspace(), target.collection, environment, &options);
        engine_scripts::apply_writes(&mut scope_view, pre_writes);
        Ok(engine_scripts::run_stage(
            ScriptStage::Test,
            &target,
            &scripted.request,
            &scope_view,
            Some(response),
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
