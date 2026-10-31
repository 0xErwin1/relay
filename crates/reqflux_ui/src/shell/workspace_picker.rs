//! The seam over the OS dialogs the workspace menu opens (recent-workspaces S3): the app asks
//! the platform, tests answer with preset paths, since no test can drive an OS dialog.

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

use gpui::{App, PathPromptOptions};

/// The directory a dialog answered with, or `None` when it was dismissed or failed.
pub(crate) type Picked = Pin<Box<dyn Future<Output = Option<PathBuf>>>>;

/// Asks the user for a workspace directory.
pub(crate) trait WorkspacePicker {
    /// An existing directory to open.
    fn pick_folder(&self, cx: &App) -> Picked;
    /// A new directory to create a workspace in.
    fn pick_new(&self, cx: &App) -> Picked;
}

/// The platform's dialogs.
pub(crate) struct OsPicker;

impl WorkspacePicker for OsPicker {
    fn pick_folder(&self, cx: &App) -> Picked {
        let answer = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Open workspace".into()),
        });
        Box::pin(async move {
            match answer.await {
                Ok(Ok(paths)) => paths.and_then(|paths| paths.into_iter().next()),
                Ok(Err(error)) => {
                    log::warn!("the folder dialog failed: {error}");
                    None
                }
                Err(_) => None,
            }
        })
    }

    fn pick_new(&self, cx: &App) -> Picked {
        let start = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let answer = cx.prompt_for_new_path(&start, Some("workspace"));
        Box::pin(async move {
            match answer.await {
                Ok(Ok(path)) => path,
                Ok(Err(error)) => {
                    log::warn!("the new workspace dialog failed: {error}");
                    None
                }
                Err(_) => None,
            }
        })
    }
}

/// Answers every dialog with a preset path, `None` meaning dismissed.
#[cfg(test)]
#[derive(Default)]
pub(crate) struct TestPicker {
    pub(crate) folder: Option<PathBuf>,
    pub(crate) new: Option<PathBuf>,
}

#[cfg(test)]
impl WorkspacePicker for TestPicker {
    fn pick_folder(&self, _cx: &App) -> Picked {
        let folder = self.folder.clone();
        Box::pin(async move { folder })
    }

    fn pick_new(&self, _cx: &App) -> Picked {
        let new = self.new.clone();
        Box::pin(async move { new })
    }
}
