use gpui::{Context, IntoElement, ParentElement, Render, Styled, Window, div};

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
        let colors = bolt::colors(cx);

        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(colors.desk)
            .text_color(colors.muted)
            .child(bolt::brand(cx).name.clone())
    }
}
