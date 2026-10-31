//! The `Toggle theme` command (light-theme S4): cycles Dark, Light and System and saves the
//! choice in the app state.

use gpui::{Context, Window};

use super::Shell;
use crate::theme::save_mode;

impl Shell {
    /// Switches to the mode after the installed one and saves it.
    pub(super) fn toggle_theme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mode = bolt::mode(cx).next();
        bolt::set_mode(mode, window.appearance(), cx);
        save_mode(self.state.as_mut(), mode);
        cx.notify();
    }
}
