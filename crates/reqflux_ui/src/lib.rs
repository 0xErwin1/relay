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
use reqflux_engine::state::AppState;

use crate::shell::Shell;
use crate::workspace::OpenedWorkspace;

/// The asset source the main window needs: the icons Bolt draws. Pass it to
/// `Application::with_assets` before [`open_main_window`]; without it, icons paint nothing.
pub fn assets() -> impl gpui::AssetSource {
    bolt::assets()
}

/// Installs Bolt with ReqFlux's brand and opens the main window on the workspace at
/// `workspace`, or on the empty state when it is `None`.
///
/// The workspace loads synchronously before the window opens; a load failure is shown in
/// the window, not returned.
pub fn open_main_window(workspace: Option<PathBuf>, cx: &mut App) -> anyhow::Result<()> {
    bolt::install(brand::reqflux(), cx);
    shell::bind_keys(cx);

    let bounds = Bounds::centered(None, tokens::MAIN_WINDOW_SIZE, cx);
    let options = WindowOptions {
        app_id: Some("reqflux".into()),
        titlebar: Some(TitlebarOptions {
            title: Some(bolt::brand(cx).name.clone()),
            ..Default::default()
        }),
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        ..Default::default()
    };

    let workspace = OpenedWorkspace::open(workspace);
    let state = match AppState::open_default() {
        Ok(state) => Some(state),
        Err(error) => {
            log::warn!("cannot open the app state; drafts stay in memory: {error}");
            None
        }
    };
    cx.open_window(options, |window, cx| {
        ui_automation::install(window, cx);
        let shell = cx.new(|cx| Shell::with_state(workspace, state, cx));
        cx.new(|cx| Root::new(shell, window, cx))
    })?;

    Ok(())
}
