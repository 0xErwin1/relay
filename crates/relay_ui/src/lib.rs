//! The GPUI user interface.

mod brand;
mod shell;
mod tokens;
mod ui_automation;
mod workspace;
#[cfg(test)]
mod workspace_tests;

use std::path::PathBuf;

use gpui::{App, AppContext as _, Bounds, TitlebarOptions, WindowBounds, WindowOptions};
use gpui_component::Root;

use crate::shell::Shell;
use crate::workspace::OpenedWorkspace;

/// Installs Bolt with Relay's brand and opens the main window on the workspace at
/// `workspace`, or on the empty state when it is `None`.
///
/// The workspace loads synchronously before the window opens; a load failure is shown in
/// the window, not returned.
pub fn open_main_window(workspace: Option<PathBuf>, cx: &mut App) -> anyhow::Result<()> {
    bolt::install(brand::relay(), cx);
    shell::bind_keys(cx);

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

    let workspace = OpenedWorkspace::open(workspace);
    cx.open_window(options, |window, cx| {
        ui_automation::install(window, cx);
        let shell = cx.new(|cx| Shell::new(workspace, cx));
        cx.new(|cx| Root::new(shell, window, cx))
    })?;

    Ok(())
}
