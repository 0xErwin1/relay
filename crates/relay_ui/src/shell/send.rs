//! Sending the active request from the shell: Send and Cancel, their keys, and the task that
//! awaits each send on GPUI's executor. See the engine's threading model.

use std::time::{Duration, Instant};

use gpui::{App, Context, KeyBinding, Task, Window, actions};
use relay_engine::{CancellationToken, ExecuteOptions, Execution};

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
    ]
);

/// Binds Ctrl+Enter and Escape in [`SHELL_CONTEXT`].
pub(super) fn bind_keys(cx: &mut App) {
    let context = Some(SHELL_CONTEXT);
    cx.bind_keys([
        KeyBinding::new("ctrl-enter", SendRequest, context),
        KeyBinding::new("escape", CancelRequest, context),
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
    pub(super) fn send_active(&mut self, cx: &mut Context<Self>) {
        if let Some(id) = self.documents.active().map(str::to_owned) {
            self.send(&id, cx);
        }
    }

    /// Escape: closes the environment menu when it is open, else cancels the send in flight
    /// of the active request. The binding takes Escape before the menu's own key handler.
    pub(super) fn cancel_active(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.environment_menu_open {
            self.close_environment_menu(window, cx);
        } else if let Some(id) = self.documents.active().map(str::to_owned) {
            self.cancel(&id, cx);
        }
    }

    /// Sends the request `id` with the active environment, cancelling its send in flight. A
    /// request the engine cannot prepare fails at once.
    pub(super) fn send(&mut self, id: &str, cx: &mut Context<Self>) {
        let OpenedWorkspace::Loaded(engine) = &self.workspace else {
            return;
        };
        if let Some(send) = self.sends.get_mut(id) {
            send.running = None;
        }
        let options = ExecuteOptions {
            environment: self.active_environment.clone(),
            ..ExecuteOptions::default()
        };
        let executed = engine.execute(id, &options);
        let (method, url) = match &executed {
            Ok(execution) => (execution.method.clone(), execution.url.clone()),
            Err(_) => self
                .requests
                .get(id)
                .map_or_else(Default::default, |state| {
                    let request = &state.inspection.request.request;
                    (
                        request.method.as_str().to_owned(),
                        state.inspection.url.clone(),
                    )
                }),
        };
        self.last_send = Some(LastSend {
            id: id.to_owned(),
            method,
            path: path_of(&url),
        });
        let send = self.sends.entry(id.to_owned()).or_default();
        send.running = None;
        match executed {
            Ok(execution) => {
                let started = Instant::now();
                send.state.start(started);
                send.running = Some(spawn(id, execution, started, cx));
            }
            Err(error) => send.state.finish(Completed::Failure(Failure {
                error: error.to_string(),
                elapsed: Duration::ZERO,
                unresolved: Vec::new(),
            })),
        }
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
    let Execution {
        unresolved,
        cancel,
        handle,
        ..
    } = execution;
    let key = id.to_owned();
    let wait = cx.spawn(async move |this, cx| {
        let failure = |error: String, unresolved| {
            Completed::Failure(Failure {
                error,
                elapsed: started.elapsed(),
                unresolved,
            })
        };
        let completed = match handle.await {
            Ok(Ok(response)) => {
                // Splitting a large body into lines stays off the UI thread.
                let received = cx
                    .background_executor()
                    .spawn(async move { Received::new(response, unresolved) })
                    .await;
                Completed::Response(received)
            }
            Ok(Err(error)) => failure(error.to_string(), unresolved),
            Err(error) => failure(error.to_string(), unresolved),
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
