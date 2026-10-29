//! The [`Engine`]: a loaded workspace, a Tokio runtime and an HTTP client.

use std::path::{Path, PathBuf};

use relay_core::model::{Request, Workspace};
use relay_core::template::SystemDynamicSource;
use relay_http::{HttpClient, HttpError, PreparedRequest, Response};
use relay_store::{Loaded, Warning};
use tokio::runtime::Runtime;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::error::{ExecuteError, OpenError};
use crate::prepare::{ExecuteOptions, Inspection, Prepared, inspect, prepare, prepare_draft};

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
        }
    }

    /// Prepares and sends the request `selector`.
    ///
    /// # Errors
    ///
    /// The errors of [`prepare`]; transport errors come out of the [`Execution`].
    pub fn execute(
        &self,
        selector: &str,
        options: &ExecuteOptions,
    ) -> Result<Execution, ExecuteError> {
        Ok(self.send(self.prepare(selector, options)?))
    }

    /// Prepares and sends `draft` in the place of the request `selector` selects, so a front
    /// end sends what the user edited. See [`prepare_draft`].
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
        Ok(self.send(self.prepare_draft(selector, draft, options)?))
    }

    /// Blocks the calling thread until `execution` finishes. Must not be called from inside
    /// an async context.
    ///
    /// # Errors
    ///
    /// The transport error, or [`ExecuteError::Task`] when the task panicked or was aborted.
    pub fn wait(&self, execution: Execution) -> Result<Response, ExecuteError> {
        self.runtime
            .block_on(execution.handle)
            .map_err(|error| ExecuteError::Task(error.to_string()))?
            .map_err(ExecuteError::from)
    }

    /// Prepares and sends the request `selector`, blocking until the response arrives. Must
    /// not be called from inside an async context.
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
        let (method, url, unresolved) = (
            execution.method.clone(),
            execution.url.clone(),
            execution.unresolved.clone(),
        );
        let response = self.wait(execution)?;
        Ok(Executed {
            method,
            url,
            unresolved,
            response,
        })
    }
}
