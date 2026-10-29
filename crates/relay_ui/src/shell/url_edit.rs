//! The editable URL: the read-only URL of the request line swaps for a text field seeded
//! with the request's URL as written. Typing marks the draft; Enter commits the text into
//! the draft and returns to the read-only line; Escape discards what was typed since the
//! field got focus and keeps the previous text. The field takes Escape while it has focus
//! through the input's own key context, so the shell's send-cancel binding keeps working
//! when no field is focused.

use gpui::{AppContext as _, Context, Entity, Subscription, Window};
use gpui_component::input::{InputEvent, InputState};
use relay_core::model::Request;

use super::Shell;

/// The URL field of a request under edit: its state, the id of the request it belongs to
/// and the URL the field was seeded with.
pub(crate) struct UrlEdit {
    /// The request the field edits.
    pub(crate) request_id: String,
    /// The input state of the field.
    pub(crate) input: Entity<InputState>,
    /// The URL the field was seeded with, restored on Escape.
    seed: String,
    /// Marks the draft on every change of the field's text.
    _change: Subscription,
}

impl Shell {
    /// Swaps the read-only URL of the request `id` for a focused field seeded with its URL
    /// as written.
    pub(super) fn start_url_edit(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        // A cell field that is open commits first, like a spreadsheet, so no edit is lost.
        if self.cell_edit.is_some() {
            self.commit_cell_edit(window, cx);
        }
        if self
            .url_edit
            .as_ref()
            .is_some_and(|edit| edit.request_id == id)
        {
            return;
        }
        let Some(seed) = self.seed_url(id) else {
            return;
        };
        let input = cx.new(|cx| InputState::new(window, cx));
        input.update(cx, |state, cx| {
            state.set_value(seed.clone(), window, cx);
            state.focus(window, cx);
        });
        let request_id = id.to_owned();
        let change = cx.subscribe(&input, |this, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change)
                && let Some(edit) = this.url_edit.as_ref()
            {
                let value = edit.input.read(cx).value().to_string();
                let request_id = edit.request_id.clone();
                this.set_draft_url(&request_id, value, cx);
            }
        });
        self.url_edit = Some(UrlEdit {
            request_id,
            input,
            seed,
            _change: change,
        });
        cx.notify();
    }

    /// The URL the field of the request `id` starts from: the draft's when it has one, else
    /// the saved request's URL as written.
    fn seed_url(&self, id: &str) -> Option<String> {
        if let Some(draft) = self.drafts.get(id) {
            return Some(draft.request.url.clone());
        }
        let saved: Option<&Request> = self
            .requests
            .get(id)
            .map(|state| &state.inspection.request.request);
        saved.map(|request| request.url.clone())
    }

    /// Enter in the field: commits its text into the draft and returns to the read-only
    /// line.
    pub(super) fn commit_url_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((request_id, input)) = self
            .url_edit
            .as_ref()
            .map(|edit| (edit.request_id.clone(), edit.input.clone()))
        else {
            return;
        };
        let value = input.read(cx).value().to_string();
        self.set_draft_url(&request_id, value, cx);
        self.close_url_edit(window, cx);
    }

    /// Escape in the field: discards what was typed since the field got focus and keeps the
    /// previous text. A draft that ends up equal to the saved request is dropped.
    pub(super) fn discard_url_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((request_id, seed)) = self
            .url_edit
            .as_ref()
            .map(|edit| (edit.request_id.clone(), edit.seed.clone()))
        else {
            return;
        };
        if self.drafts.contains_key(&request_id) {
            self.set_draft_url(&request_id, seed, cx);
        }
        let saved = self
            .requests
            .get(&request_id)
            .map(|state| state.inspection.request.request.clone());
        if self
            .drafts
            .get(&request_id)
            .is_some_and(|draft| Some(&draft.request) == saved.as_ref())
        {
            self.drafts.remove(&request_id);
            self.delete_stored_draft(&request_id);
            cx.notify();
        }
        self.close_url_edit(window, cx);
    }

    /// Drops the field and gives the focus back to the shell, so the window's keys
    /// (Ctrl+Enter, Escape) keep reaching the shell's key context.
    pub(super) fn close_url_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.url_edit.take().is_some() {
            self.focus.focus(window, cx);
            cx.notify();
        }
    }

    /// Drops the field when it no longer belongs to the active request `id`.
    pub(super) fn drop_stale_url_edit(&mut self, id: &str) {
        if self
            .url_edit
            .as_ref()
            .is_some_and(|edit| edit.request_id != id)
        {
            self.url_edit = None;
        }
    }

    /// Drops the field of the request `id`, without committing or discarding its text: the
    /// draft keeps what the field synced so far.
    pub(super) fn drop_url_edit_of(&mut self, id: &str) {
        if self
            .url_edit
            .as_ref()
            .is_some_and(|edit| edit.request_id == id)
        {
            self.url_edit = None;
        }
    }
}
