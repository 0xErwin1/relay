//! The draft of an edited request: what the shell holds in memory and how it persists in
//! the per-user app state. An edit never touches the workspace file until save; the draft
//! carries the whole request document, so later editable surfaces need no new storage.

use std::collections::HashMap;

use gpui::{Context, Window};
use relay_core::model::Request;
use relay_engine::state::Draft;
use relay_engine::{Engine, SaveError};

use super::Shell;
use super::tree_model;
use crate::workspace::OpenedWorkspace;

/// The unsaved edit of a request: the document as edited, the content hash of the file the
/// edit started from, and whether the file moved underneath the draft.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RequestDraft {
    /// The whole request document, as edited.
    pub(crate) request: Request,
    /// The content hash of the file the draft was made from, as text; the base a save is
    /// checked against.
    pub(crate) base_hash: Option<String>,
    /// Whether the saved file is known to have changed underneath the draft.
    pub(crate) conflicted: bool,
}

impl Shell {
    /// Restores the drafts of the loaded workspace from the app state, inspecting their
    /// requests as drafted. A payload that does not parse is dropped with a warning, never
    /// panicked on.
    pub(crate) fn load_drafts(&mut self) {
        let Some(workspace_id) = self.workspace_id.clone() else {
            return;
        };
        let Some(state) = self.state.as_ref() else {
            return;
        };
        let stored = match state.drafts(&workspace_id) {
            Ok(stored) => stored,
            Err(error) => {
                log::warn!("cannot read the drafts of workspace {workspace_id}: {error}");
                return;
            }
        };
        for (entity_id, stored) in stored {
            let request = match toml::from_str::<Request>(&stored.payload) {
                Ok(request) => request,
                Err(error) => {
                    log::warn!("the draft of request {entity_id} did not parse: {error}");
                    continue;
                }
            };
            let conflicted = self.file_moved(&entity_id, stored.base_hash.as_deref());
            self.drafts.insert(
                entity_id,
                RequestDraft {
                    request,
                    base_hash: stored.base_hash,
                    conflicted,
                },
            );
        }
    }

    /// Whether the request `id`'s file no longer has the content hash `base_hash` as the
    /// workspace was loaded, so a draft made from it no longer matches the file.
    fn file_moved(&self, id: &str, base_hash: Option<&str>) -> bool {
        let OpenedWorkspace::Loaded(engine) = &self.workspace else {
            return false;
        };
        let current = engine
            .request_base_hash(id)
            .ok()
            .flatten()
            .map(|hash| hash.to_string());
        current.as_deref() != base_hash
    }

    /// Puts `url` into the draft of the request `id`, creating the draft from the saved
    /// request when there is none, and persists it in the app state.
    pub(super) fn set_draft_url(&mut self, id: &str, url: String, cx: &mut Context<Self>) {
        self.edit_draft(id, |request| request.url = url, cx);
    }

    /// Toggles the `enabled` state of the 1-based row `n` of `table` (`params` or
    /// `headers`) in the draft of the request `id`, creating the draft from the saved
    /// request when there is none, and persists it. Rows of the inactive sub-tab are never
    /// touched, because only the active one renders.
    pub(super) fn toggle_grid_row(
        &mut self,
        id: &str,
        table: &str,
        n: usize,
        cx: &mut Context<Self>,
    ) {
        self.edit_draft(
            id,
            |request| {
                let rows = match table {
                    "params" => &mut request.params,
                    _ => &mut request.headers,
                };
                if let Some(row) = n.checked_sub(1).and_then(|index| rows.get_mut(index)) {
                    row.enabled = !row.enabled;
                }
            },
            cx,
        );
    }

    /// Edits the draft of the request `id` with `edit`, creating the draft from the saved
    /// request when there is none, and persists it in the app state.
    fn edit_draft(&mut self, id: &str, edit: impl FnOnce(&mut Request), cx: &mut Context<Self>) {
        let OpenedWorkspace::Loaded(engine) = &self.workspace else {
            return;
        };
        let base_hash = engine
            .request_base_hash(id)
            .ok()
            .flatten()
            .map(|hash| hash.to_string());
        match self.drafts.get_mut(id) {
            Some(draft) => edit(&mut draft.request),
            None => {
                let Some(mut request) = self
                    .requests
                    .get(id)
                    .map(|state| state.inspection.request.request.clone())
                else {
                    return;
                };
                edit(&mut request);
                self.drafts.insert(
                    id.to_owned(),
                    RequestDraft {
                        request,
                        base_hash,
                        conflicted: false,
                    },
                );
            }
        }
        self.store_draft(id);
        cx.notify();
    }

    /// Persists the draft of the request `id` in the app state; a failure is logged and
    /// leaves the draft in memory.
    fn store_draft(&mut self, id: &str) {
        let Some(workspace_id) = self.workspace_id.clone() else {
            return;
        };
        let Some(draft) = self.drafts.get(id).cloned() else {
            return;
        };
        let payload = match toml::to_string(&draft.request) {
            Ok(payload) => payload,
            Err(error) => {
                log::warn!("cannot serialize the draft of {id}: {error}");
                return;
            }
        };
        let Some(state) = self.state.as_mut() else {
            return;
        };
        if let Err(error) = state.put_draft(&workspace_id, id, Draft::new(payload, draft.base_hash))
        {
            log::warn!("cannot store the draft of {id}: {error}");
        }
    }

    /// Deletes the stored draft of the request `id` from the app state; a failure is
    /// logged.
    pub(super) fn delete_stored_draft(&mut self, id: &str) {
        if let (Some(state), Some(workspace_id)) =
            (self.state.as_mut(), self.workspace_id.as_deref())
            && let Err(error) = state.delete_draft(workspace_id, id)
        {
            log::warn!("cannot forget the draft of {id}: {error}");
        }
    }

    /// The URL the request line of `id` shows as written: the draft's when it has one.
    pub(super) fn displayed_url(&self, id: &str) -> String {
        match self.drafts.get(id) {
            Some(draft) => draft.request.url.clone(),
            None => self
                .requests
                .get(id)
                .map_or_else(String::new, |state| state.inspection.url.clone()),
        }
    }

    /// The request the view of `id` shows: the draft's when it has one, else `fallback`,
    /// the saved request as inspected.
    pub(super) fn displayed_request<'a>(&'a self, id: &str, fallback: &'a Request) -> &'a Request {
        self.drafts.get(id).map_or(fallback, |draft| &draft.request)
    }

    /// Saves the draft of the request `id` into its file, hash-checked against the base the
    /// draft was made from, and discards the draft. The workspace is reloaded from disk, so
    /// the tree, the request view and later sends see the saved file.
    pub(super) fn save(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.drop_url_edit_of(id);
        let Some(draft) = self.drafts.get(id).cloned() else {
            return;
        };
        let OpenedWorkspace::Loaded(engine) = &self.workspace else {
            return;
        };
        let current = match engine.request_base_hash(id) {
            Ok(current) => current,
            Err(error) => {
                log::warn!("cannot save {id}: {error}");
                return;
            }
        };
        // A live draft was made from the file as this workspace loaded it; when the two no
        // longer agree, the file moved underneath the draft after a restart.
        let loaded = current.as_ref().map(|hash| hash.to_string());
        if loaded.as_deref() != draft.base_hash.as_deref() {
            self.mark_conflicted(id, cx);
            return;
        }
        match engine.save_request(id, &draft.request, current) {
            Ok(_) => {
                self.drafts.remove(id);
                self.delete_stored_draft(id);
                self.reload_workspace(cx);
            }
            Err(SaveError::Conflict { .. }) => self.mark_conflicted(id, cx),
            Err(error) => log::warn!("cannot save {id}: {error}"),
        }
        self.focus.focus(window, cx);
    }

    /// Marks the draft of the request `id` conflicted: the saved file moved underneath it,
    /// so saving would overwrite the other edit.
    fn mark_conflicted(&mut self, id: &str, cx: &mut Context<Self>) {
        if let Some(draft) = self.drafts.get_mut(id) {
            draft.conflicted = true;
            cx.notify();
        }
    }

    /// Reverts the draft of the request `id`: the view goes back to the saved file, which
    /// the workspace is reloaded for.
    pub(super) fn revert(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.drop_url_edit_of(id);
        if self.drafts.remove(id).is_some() {
            self.delete_stored_draft(id);
            self.reload_workspace(cx);
        }
        self.focus.focus(window, cx);
    }

    /// Loads the workspace from disk into the same loaded state, keeping the active
    /// environment, the open requests and their sub-tabs. A failure keeps the loaded
    /// workspace as it was.
    fn reload_workspace(&mut self, cx: &mut Context<Self>) {
        let root = match &self.workspace {
            OpenedWorkspace::Loaded(engine) => engine.root().to_path_buf(),
            OpenedWorkspace::None | OpenedWorkspace::Failed { .. } => return,
        };
        match Engine::open(root) {
            Ok(engine) => {
                self.workspace_id = Some(engine.workspace().manifest.id.to_string());
                let roots = tree_model::nodes(engine.workspace());
                self.sidebar
                    .update(cx, |sidebar, cx| sidebar.set_roots(roots, cx));
                self.workspace = OpenedWorkspace::Loaded(Box::new(engine));
            }
            Err(error) => log::warn!("cannot reload the workspace: {error}"),
        }
        self.inspect_open_requests(cx);
    }
}

/// The map of drafts the shell keeps, by request id.
pub(crate) type Drafts = HashMap<String, RequestDraft>;
