//! The GPUI user interface.

mod brand;
mod tokens;
mod ui_automation;
mod workspace;

use gpui::{App, AppContext as _, Bounds, TitlebarOptions, WindowBounds, WindowOptions};
use gpui_component::Root;

use crate::workspace::Workspace;

/// Installs Bolt with Relay's brand and opens the main window.
pub fn open_main_window(cx: &mut App) -> anyhow::Result<()> {
    bolt::install(brand::relay(), cx);

    let bounds = Bounds::centered(None, tokens::MAIN_WINDOW_SIZE, cx);
    let options = WindowOptions {
        app_id: Some("relay".into()),
        titlebar: Some(TitlebarOptions {
            title: Some(bolt::brand(cx).name.clone()),
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
