//! GUI startup.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::rc::Rc;

use gpui::App;

/// Opens the main window on `workspace`, or on the default of [`workspace_dir`], and runs
/// until it closes.
pub(crate) fn open(workspace: Option<PathBuf>) -> ExitCode {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let workspace = match std::env::current_dir() {
        Ok(cwd) => workspace_dir(workspace, &cwd),
        Err(error) => {
            log::warn!("could not read the current directory: {error}");
            workspace
        }
    };

    let startup_error: Rc<RefCell<Option<anyhow::Error>>> = Rc::default();

    let error_slot = Rc::clone(&startup_error);
    gpui_platform::application()
        .with_assets(reqflux_ui::assets())
        .run(move |cx: &mut App| {
            crate::signals::quit_on_termination(cx);
            if let Err(error) = reqflux_ui::open_main_window(workspace.clone(), cx) {
                error_slot.replace(Some(error));
                cx.quit();
            }
        });

    match startup_error.take() {
        None => ExitCode::SUCCESS,
        Some(error) => {
            eprintln!("reqflux: failed to open the main window: {error:#}");
            ExitCode::FAILURE
        }
    }
}

/// The workspace to open: `requested`, else `cwd` when it holds a `reqflux.toml`, else none.
fn workspace_dir(requested: Option<PathBuf>, cwd: &Path) -> Option<PathBuf> {
    requested.or_else(|| {
        cwd.join("reqflux.toml")
            .is_file()
            .then(|| cwd.to_path_buf())
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn temp_dir() -> tempfile::TempDir {
        match tempfile::TempDir::new() {
            Ok(dir) => dir,
            Err(error) => panic!("temp dir: {error}"),
        }
    }

    #[test]
    fn a_requested_directory_wins() {
        let cwd = temp_dir();
        let requested = PathBuf::from("elsewhere");
        assert_eq!(
            workspace_dir(Some(requested.clone()), cwd.path()),
            Some(requested)
        );
    }

    #[test]
    fn the_current_directory_opens_when_it_holds_a_manifest() {
        let cwd = temp_dir();
        assert_eq!(workspace_dir(None, cwd.path()), None);
        if let Err(error) = fs::write(cwd.path().join("reqflux.toml"), "") {
            panic!("write: {error}");
        }
        assert_eq!(
            workspace_dir(None, cwd.path()),
            Some(cwd.path().to_path_buf())
        );
    }
}
