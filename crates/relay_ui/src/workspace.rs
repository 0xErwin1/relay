use gpui::{Context, IntoElement, ParentElement, Render, Styled, Window, div};
use gpui_component::ActiveTheme as _;

/// The root view of the main window.
#[derive(Debug)]
pub(crate) struct Workspace;

impl Workspace {
    pub(crate) fn new() -> Self {
        Self
    }
}

impl Render for Workspace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(theme.background)
            .text_color(theme.muted_foreground)
            .child("Relay")
    }
}
