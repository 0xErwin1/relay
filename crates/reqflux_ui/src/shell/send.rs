//! Sending the active request from the shell: Send and Cancel, their keys, and the tasks
//! that prepare and await each send off the UI thread. See the engine's threading model.

use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{App, Context, KeyBinding, Task, Window, actions};
use reqflux_engine::{CancellationToken, ExecuteOptions, Executed, Execution, ScriptOutcome};

use super::Shell;
use super::last_request::{LastSend, path_of};
use super::request::{Completed, Failure, Received, SendState};
use super::response::{BodyView, ResponseTab};
use crate::workspace::OpenedWorkspace;

/// The key context of the main window, where the send keys are bound.
pub(crate) const SHELL_CONTEXT: &str = "Shell";

/// How often the elapsed time of a send in flight is redrawn.
const TICK: Duration = Duration::from_millis(100);

actions!(
    shell,
    [
        /// Sends the active request, replacing its send in flight.
        SendRequest,
        /// Cancels the send in flight of the active request.
        CancelRequest,
        /// Saves the draft of the active request.
        SaveRequest,
    ]
);

/// Binds Ctrl+Enter, Escape and Ctrl+S in [`SHELL_CONTEXT`].
pub(super) fn bind_keys(cx: &mut App) {
    let context = Some(SHELL_CONTEXT);
    cx.bind_keys([
        KeyBinding::new("ctrl-enter", SendRequest, context),
        KeyBinding::new("escape", CancelRequest, context),
        KeyBinding::new("ctrl-s", SaveRequest, context),
    ]);
}

/// The send of one open request: its state, the response sub-tab and body view, and, while
/// in flight, what keeps it running.
#[derive(Default)]
pub(crate) struct RequestSend {
    pub(crate) state: SendState,
    pub(crate) tab: ResponseTab,
    pub(crate) view: BodyView,
    running: Option<Running>,
    /// The task preparing the current send: it runs the engine's `execute` on the
    /// background executor and hands the execution to `running`. Held so it is not
    /// dropped, which would cancel the hand-off; the next send of this request replaces
    /// it, cancelling a prepare that has not finished yet.
    preparing: Option<Task<()>>,
}

/// A send in flight. Dropping it cancels the request and drops the tasks awaiting it.
struct Running {
    cancel: CancellationToken,
    _wait: Task<()>,
    _tick: Task<()>,
}

impl Drop for Running {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

impl Shell {
    /// Sends the active request.
    pub(super) fn send_active(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(id) = self.documents.active().map(str::to_owned) {
            self.send(&id, window, cx);
        }
    }

    /// Escape: closes the environment, auth type or body mode menu when one is open, else cancels the
    /// send in flight of the active request. The binding takes Escape before the menu's own
    /// key handler.
    pub(super) fn cancel_active(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.environment_menu_open {
            self.close_environment_menu(window, cx);
        } else if self.auth_menu_open {
            self.close_auth_menu(window, cx);
        } else if self.body_menu_open {
            self.close_body_menu(window, cx);
        } else if let Some(id) = self.documents.active().map(str::to_owned) {
            self.cancel(&id, cx);
        }
    }

    /// Ctrl+S: saves the draft of the active request.
    pub(super) fn save_active(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(id) = self.documents.active().map(str::to_owned) {
            self.save(&id, window, cx);
        }
    }

    /// Sends the request `id` with the active environment, cancelling its send in flight.
    /// Preparing runs on GPUI's background executor — it runs the effective pre-request
    /// script, which can take seconds — so the window never waits for a script (S14). The
    /// send shows as in flight at once; a request the engine cannot prepare fails when the
    /// prepare task returns.
    pub(super) fn send(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        // The cell field shows text the draft does not have yet: commit it, so a send never
        // silently drops what was typed.
        self.commit_cell_edit(window, cx);
        let OpenedWorkspace::Loaded(engine) = &self.workspace else {
            return;
        };
        let engine = Arc::clone(engine);
        if let Some(send) = self.sends.get_mut(id) {
            send.running = None;
        }
        let options = ExecuteOptions {
            environment: self.active_environment.clone(),
            ..ExecuteOptions::default()
        };
        // A request with a draft sends the draft, not the file on disk.
        let drafted = self.drafts.get(id).map(|draft| draft.request.clone());
        // Until the execution reports its own method and URL, the request view labels the
        // status bar and the send in flight.
        let (method, url) = self
            .requests
            .get(id)
            .map_or_else(Default::default, |state| {
                let request = &state.inspection.request.request;
                (
                    request.method.as_str().to_owned(),
                    state.inspection.url.clone(),
                )
            });
        self.last_send = Some(LastSend {
            id: id.to_owned(),
            method,
            path: path_of(&url),
        });
        let started = Instant::now();
        let send = self.sends.entry(id.to_owned()).or_default();
        send.running = None;
        send.state.start(started);
        let key = id.to_owned();
        let selector = key.clone();
        // Preparing runs the effective pre-request script, which can take seconds: it runs
        // on GPUI's background executor, never on the UI thread (S14).
        let prepare = cx.background_executor().spawn(async move {
            match drafted {
                Some(draft) => engine.execute_draft(&selector, &draft, &options),
                None => engine.execute(&selector, &options),
            }
        });
        send.preparing = Some(cx.spawn(async move |this, cx| {
            match prepare.await {
                Ok(execution) => {
                    let (method, path) =
                        (execution.method.clone(), path_of(&execution.url));
                    if let Err(error) = this.update(cx, |shell, cx| {
                        let running = spawn(&key, execution, started, cx);
                        match shell.sends.get_mut(&key) {
                            Some(send) => {
                                // The status bar names what was actually sent, unless a
                                // newer send replaced this one meanwhile.
                                if matches!(&send.state, SendState::Sending { started: at, .. } if *at == started)
                                {
                                    shell.last_send = Some(LastSend {
                                        id: key.clone(),
                                        method,
                                        path,
                                    });
                                }
                                send.running = Some(running);
                            }
                            // The tab closed while the execution prepared; dropping
                            // `running` cancels it.
                            None => drop(running),
                        }
                        cx.notify();
                    }) {
                        log::debug!("send of {key} ended after its shell: {error}");
                    }
                }
                Err(error) => {
                    let completed = Completed::Failure(Failure {
                        error: error.to_string(),
                        elapsed: Duration::ZERO,
                        unresolved: Vec::new(),
                        // The prepare failed before any script could run.
                        scripts: ScriptOutcome::default(),
                    });
                    if let Err(error) =
                        this.update(cx, |shell, cx| shell.finish_send(&key, completed, cx))
                    {
                        log::debug!("send of {key} failed after its shell: {error}");
                    }
                }
            }
        }));
        cx.notify();
    }

    /// Cancels the send in flight of the request `id`; it then fails with
    /// `request cancelled`.
    pub(super) fn cancel(&mut self, id: &str, cx: &mut Context<Self>) {
        if let Some(running) = self.sends.get(id).and_then(|send| send.running.as_ref()) {
            running.cancel.cancel();
            cx.notify();
        }
    }

    /// Selects the response sub-tab `tab` of the request `id`.
    pub(super) fn select_response_tab(
        &mut self,
        id: &str,
        tab: ResponseTab,
        cx: &mut Context<Self>,
    ) {
        if let Some(send) = self.sends.get_mut(id) {
            send.tab = tab;
            cx.notify();
        }
    }

    /// Shows the response body of the request `id` as `view`.
    pub(super) fn select_body_view(&mut self, id: &str, view: BodyView, cx: &mut Context<Self>) {
        if let Some(send) = self.sends.get_mut(id) {
            send.view = view;
            cx.notify();
        }
    }

    fn finish_send(&mut self, id: &str, completed: Completed, cx: &mut Context<Self>) {
        if let Some(send) = self.sends.get_mut(id) {
            send.state.finish(completed);
            send.running = None;
            cx.notify();
        }
    }
}

/// Awaits `execution` on GPUI's executor and redraws every [`TICK`] until it ends.
fn spawn(id: &str, execution: Execution, started: Instant, cx: &mut Context<Shell>) -> Running {
    let cancel = execution.cancel.clone();
    let unresolved = execution.unresolved.clone();
    // The pre-request has already run when the transport fails; its results live on the
    // execution, so keep them to show beside the error banner.
    let scripts = execution.scripts.clone();
    let key = id.to_owned();
    let wait = cx.spawn(async move |this, cx| {
        let completed = match execution.finish().await {
            Ok(executed) => {
                let Executed {
                    response,
                    unresolved,
                    scripts,
                    ..
                } = executed;
                // Splitting a large body into lines stays off the UI thread.
                let received = cx
                    .background_executor()
                    .spawn(async move { Received::new(response, unresolved, scripts) })
                    .await;
                Completed::Response(received)
            }
            Err(error) => Completed::Failure(Failure {
                error: error.to_string(),
                elapsed: started.elapsed(),
                unresolved,
                scripts,
            }),
        };
        if let Err(error) = this.update(cx, |shell, cx| shell.finish_send(&key, completed, cx)) {
            log::debug!("send of {key} ended after its shell: {error}");
        }
    });
    let tick = cx.spawn(async move |this, cx| {
        loop {
            cx.background_executor().timer(TICK).await;
            if this.update(cx, |_, cx| cx.notify()).is_err() {
                break;
            }
        }
    });
    Running {
        cancel,
        _wait: wait,
        _tick: tick,
    }
}
