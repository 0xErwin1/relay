//! The editable Settings pane: the timeout row opens a field in the shell's cell field slot,
//! so it commits, discards and gives the focus back exactly as a grid cell does, and an open
//! field elsewhere commits first. The redirects and TLS rows cycle Default, on, off.

use gpui::{AppContext as _, Context, Window};
use gpui_component::input::InputState;
use reqflux_core::model::Settings;

use super::Shell;
use super::cell_edit::{CellEdit, CellField};

/// The table name the timeout field records in its [`CellEdit`], which no grid uses.
pub(crate) const SETTINGS_TABLE: &str = "settings";

/// A setting the pane cycles on click.
#[derive(Clone, Copy)]
pub(crate) enum Toggle {
    FollowRedirects,
    VerifyTls,
}

impl Toggle {
    fn slot(self, settings: &mut Settings) -> &mut Option<bool> {
        match self {
            Self::FollowRedirects => &mut settings.follow_redirects,
            Self::VerifyTls => &mut settings.verify_tls,
        }
    }
}

impl Shell {
    /// Opens a focused field over the timeout of the request `id`, seeded with the timeout
    /// the pane shows, empty for the default. An open field elsewhere commits first.
    pub(super) fn start_timeout_edit(
        &mut self,
        id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.timeout_edit_of(id) {
            return;
        }
        self.commit_cell_edit(window, cx);
        let Some(state) = self.requests.get(id) else {
            return;
        };
        let seed = self
            .displayed_request(id, &state.inspection.request.request)
            .settings
            .timeout_ms
            .map(|ms| ms.to_string())
            .unwrap_or_default();
        let input = cx.new(|cx| InputState::new(window, cx));
        input.update(cx, |state, cx| {
            state.set_value(seed, window, cx);
            state.focus(window, cx);
        });
        self.cell_edit = Some(CellEdit {
            request_id: id.to_owned(),
            table: SETTINGS_TABLE.to_owned(),
            n: 0,
            field: CellField::Value,
            input,
            secret: false,
        });
        cx.notify();
    }

    /// Whether the open field is the timeout field of the request `id`.
    pub(super) fn timeout_edit_of(&self, id: &str) -> bool {
        self.cell_edit
            .as_ref()
            .is_some_and(|edit| edit.request_id == id && edit.table == SETTINGS_TABLE)
    }

    /// Puts the timeout `text` into the draft of the request `id`: a whole number of
    /// milliseconds, or empty for the default. Other text is rejected and leaves the draft
    /// as it was; committing the timeout the request already has, without a draft, creates
    /// none.
    pub(super) fn set_draft_timeout(&mut self, id: &str, text: &str, cx: &mut Context<Self>) {
        let text = text.trim();
        let timeout = if text.is_empty() {
            None
        } else {
            match text.parse::<u64>() {
                Ok(ms) => Some(ms),
                Err(_) => {
                    log::debug!("rejected a timeout that is not a whole number of ms");
                    return;
                }
            }
        };
        if !self.drafts.contains_key(id) {
            let current = self
                .requests
                .get(id)
                .map(|state| state.inspection.request.request.settings.timeout_ms);
            if current == Some(timeout) {
                return;
            }
        }
        self.edit_draft(id, |request| request.settings.timeout_ms = timeout, cx);
    }

    /// Cycles `toggle` of the request `id` in its draft: Default, on, off, Default. An open
    /// field commits first, so the click never drops what was typed.
    pub(super) fn cycle_setting(
        &mut self,
        id: &str,
        toggle: Toggle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.commit_cell_edit(window, cx);
        self.edit_draft(
            id,
            |request| {
                let slot = toggle.slot(&mut request.settings);
                *slot = match *slot {
                    None => Some(true),
                    Some(true) => Some(false),
                    Some(false) => None,
                };
            },
            cx,
        );
    }
}
