//! The GPUI user interface.

mod ui_automation;
mod workspace;

use gpui::{App, AppContext as _, Bounds, TitlebarOptions, WindowBounds, WindowOptions, px, size};
use gpui_component::Root;

use crate::workspace::Workspace;

/// Initializes the UI globals and opens the main window.
pub fn open_main_window(cx: &mut App) -> anyhow::Result<()> {
    gpui_component::init(cx);

    let bounds = Bounds::centered(None, size(px(1200.0), px(800.0)), cx);
    let options = WindowOptions {
        app_id: Some("relay".into()),
        titlebar: Some(TitlebarOptions {
            title: Some("Relay".into()),
            ..Default::default()
        }),
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        ..Default::default()
    };

    cx.open_window(options, |window, cx| {
        ui_automation::install(window, cx);
        let workspace = cx.new(|_cx| Workspace::new());
        cx.new(|cx| Root::new(workspace, window, cx))
    })?;

    Ok(())
}
