//! The editable Body pane: the mode menu switches the body mode, and clicking the raw text
//! swaps it for a multi-line editor whose text lands in the draft as it is typed, as the
//! URL field's does. Enter inserts a newline, Ctrl+Enter sends and Escape gives the focus
//! back to the shell, keeping the text, which is already in the draft. The urlencoded and
//! form-data grids edit through the grid cells with the table `body`. GraphQL and binary
//! bodies stay read-only.

use gpui::{AppContext as _, Context, Entity, Subscription, Window};
use gpui_component::input::{InputEvent, TextareaState};
use reqflux_core::model::{Body, FormPart, RawLanguage};

use super::Shell;

/// The modes the mode menu offers, by their menu id and name, in menu order.
pub(crate) const BODY_MODES: [(&str, &str); 6] = [
    ("none", "none"),
    ("json", "raw · JSON"),
    ("xml", "raw · XML"),
    ("text", "raw · Text"),
    ("urlencoded", "urlencoded"),
    ("formdata", "form-data"),
];

/// The rows the raw text editor shows.
const EDITOR_ROWS: usize = 12;

/// The menu id of the mode of `body`, or `None` for a mode the menu does not offer.
pub(crate) fn body_kind(body: &Body) -> Option<&'static str> {
    Some(match body {
        Body::None => "none",
        Body::Raw {
            language: RawLanguage::Json,
            ..
        } => "json",
        Body::Raw {
            language: RawLanguage::Xml,
            ..
        } => "xml",
        Body::Raw {
            language: RawLanguage::Text,
            ..
        } => "text",
        Body::Urlencoded { .. } => "urlencoded",
        Body::FormData { .. } => "formdata",
        _ => return None,
    })
}

/// `body` switched to the mode `kind`: a raw body keeps its text across raw languages,
/// any other switch starts empty.
fn switched_body(body: &Body, kind: &str) -> Option<Body> {
    let language = match kind {
        "none" => return Some(Body::None),
        "urlencoded" => return Some(Body::Urlencoded { fields: Vec::new() }),
        "formdata" => return Some(Body::FormData { parts: Vec::new() }),
        "json" => RawLanguage::Json,
        "xml" => RawLanguage::Xml,
        "text" => RawLanguage::Text,
        _ => return None,
    };
    let text = match body {
        Body::Raw { text, .. } => text.clone(),
        _ => String::new(),
    };
    Some(Body::Raw { language, text })
}

/// The key, the value (a file part's path) and the enabled flag of the 1-based row `n` of
/// the urlencoded or form-data `body`.
pub(crate) fn body_cell(
    body: &mut Body,
    n: usize,
) -> Option<(&mut String, &mut String, &mut bool)> {
    let index = n.checked_sub(1)?;
    match body {
        Body::Urlencoded { fields } => fields
            .get_mut(index)
            .map(|field| (&mut field.key, &mut field.value, &mut field.enabled)),
        Body::FormData { parts } => parts.get_mut(index).map(|part| match part {
            FormPart::Text {
                key,
                value,
                enabled,
                ..
            } => (key, value, enabled),
            FormPart::File {
                key, path, enabled, ..
            } => (key, path, enabled),
        }),
        _ => None,
    }
}

/// The raw text editor of a request: the request it edits and its state.
pub(crate) struct BodyEdit {
    pub(crate) request_id: String,
    pub(crate) input: Entity<TextareaState>,
    /// Puts the text into the draft on every change.
    _change: Subscription,
}

impl Shell {
    /// The body the Body pane of the request `id` shows: the draft's, else the file's.
    fn displayed_body(&self, id: &str) -> Option<Body> {
        let state = self.requests.get(id)?;
        Some(
            self.displayed_request(id, &state.inspection.request.request)
                .body
                .clone(),
        )
    }

    /// Opens the mode menu and focuses it, or closes it when open.
    pub(super) fn toggle_body_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.body_menu_open {
            self.close_body_menu(window, cx);
        } else {
            self.body_menu_open = true;
            self.body_menu_focus.focus(window, cx);
            cx.notify();
        }
    }

    /// Closes the mode menu and gives the focus back.
    pub(super) fn close_body_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.body_menu_open = false;
        self.restore_focus(window, cx);
    }

    /// Switches the body of the request `id` to the mode `kind`. Picking the mode it
    /// already has changes nothing. An open field commits first.
    pub(super) fn set_body_mode(
        &mut self,
        id: &str,
        kind: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_body_menu(window, cx);
        self.commit_cell_edit(window, cx);
        self.drop_body_edit_of(id);
        let Some(current) = self.displayed_body(id) else {
            return;
        };
        if body_kind(&current) == Some(kind) {
            return;
        }
        if let Some(body) = switched_body(&current, kind) {
            self.edit_draft(id, |request| request.body = body, cx);
        }
    }

    /// Swaps the raw text of the request `id` for a focused multi-line editor seeded with
    /// it. An open cell field commits first.
    pub(super) fn start_body_edit(
        &mut self,
        id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.commit_cell_edit(window, cx);
        if self.body_edit_of(id) {
            return;
        }
        let Some(Body::Raw { text, .. }) = self.displayed_body(id) else {
            return;
        };
        let input = cx.new(|cx| TextareaState::new(window, cx).rows(EDITOR_ROWS));
        input.update(cx, |state, cx| {
            state.set_value(text, window, cx);
            state.focus(window, cx);
        });
        let change = cx.subscribe(&input, |this, input, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change)
                && let Some(id) = this.body_edit.as_ref().map(|edit| edit.request_id.clone())
            {
                let value = input.read(cx).value().to_string();
                this.set_draft_body_text(&id, value, cx);
            }
        });
        self.body_edit = Some(BodyEdit {
            request_id: id.to_owned(),
            input,
            _change: change,
        });
        cx.notify();
    }

    /// Whether the raw text editor is open over the request `id`.
    pub(super) fn body_edit_of(&self, id: &str) -> bool {
        self.body_edit
            .as_ref()
            .is_some_and(|edit| edit.request_id == id)
    }

    /// Puts `text` into the raw body of the request `id`'s draft. The text the request
    /// already has, without a draft, creates none.
    fn set_draft_body_text(&mut self, id: &str, value: String, cx: &mut Context<Self>) {
        if !self.drafts.contains_key(id)
            && matches!(self.displayed_body(id), Some(Body::Raw { text, .. }) if text == value)
        {
            return;
        }
        self.edit_draft(
            id,
            |request| {
                if let Body::Raw { text, .. } = &mut request.body {
                    *text = value;
                }
            },
            cx,
        );
    }

    /// Escape in the editor: closes it and gives the focus back to the shell. The text is
    /// already in the draft.
    pub(super) fn close_body_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.body_edit.take().is_some() {
            self.focus.focus(window, cx);
            cx.notify();
        }
    }

    /// Drops the editor of the request `id`; the draft keeps what it synced.
    pub(super) fn drop_body_edit_of(&mut self, id: &str) {
        if self.body_edit_of(id) {
            self.body_edit = None;
        }
    }

    /// Drops the editor when it no longer belongs to the active request `id`.
    pub(super) fn drop_stale_body_edit(&mut self, id: &str) {
        if self
            .body_edit
            .as_ref()
            .is_some_and(|edit| edit.request_id != id)
        {
            self.body_edit = None;
        }
    }
}
