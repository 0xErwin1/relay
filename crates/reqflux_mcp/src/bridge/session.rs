//! One MCP session's choice of bridge target: automatic with a single live
//! app, explicit with several, and never a silent switch to another app.

use std::path::PathBuf;

use gpui_mcp_protocol::AppId;
use thiserror::Error;

use super::client::BridgeClient;
use super::descriptor::{TargetId, TargetSummary};
use super::discover::{DiscoveryError, discover};

/// Why a session could not pick a target to talk to.
#[derive(Debug, Error)]
pub enum SessionError {
    /// The endpoint directory could not be read.
    #[error(transparent)]
    Discovery(#[from] DiscoveryError),
    /// No app with this identifier is running.
    #[error("no {app_id} app is running; start it, then call ui_list_targets again")]
    NoTarget {
        /// The application identifier the session looks for.
        app_id: String,
    },
    /// Several apps are running and none was chosen.
    #[error("{count} {app_id} apps are running; choose one with ui_select_target")]
    Ambiguous {
        /// The application identifier the session looks for.
        app_id: String,
        /// How many targets are live.
        count: usize,
    },
    /// The target id is not a live target.
    #[error("no live target has the id {0}")]
    UnknownTarget(TargetId),
    /// The selected target stopped running.
    #[error("the selected target {0} is no longer running; call ui_select_target")]
    SelectedTargetGone(TargetId),
}

/// The targets a session may talk to and the one it talks to.
#[derive(Debug)]
pub struct BridgeSession {
    endpoint_dir: PathBuf,
    app_id: AppId,
    selected: Option<TargetId>,
}

impl BridgeSession {
    /// A session that looks for `app_id` under `endpoint_dir`, with nothing
    /// selected yet.
    #[must_use]
    pub fn new(endpoint_dir: PathBuf, app_id: AppId) -> Self {
        Self {
            endpoint_dir,
            app_id,
            selected: None,
        }
    }

    /// The target the session talks to, once one is chosen.
    #[must_use]
    pub fn selected(&self) -> Option<&TargetId> {
        self.selected.as_ref()
    }

    /// The live targets, each marked when it is the session's. The only live
    /// target is selected automatically.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::Discovery`] when the endpoint directory cannot
    /// be read.
    pub async fn list_targets(&mut self) -> Result<Vec<TargetSummary>, SessionError> {
        let targets = discover(&self.endpoint_dir, Some(&self.app_id)).await?;
        if self.selected.is_none() && targets.len() == 1 {
            self.selected = targets
                .first()
                .map(super::descriptor::ValidatedTarget::target_id);
        }
        let selected = self.selected.clone();
        Ok(targets
            .iter()
            .map(|target| {
                let mut summary = target.summary();
                summary.selected = selected.as_ref() == Some(&summary.target_id);
                summary
            })
            .collect())
    }

    /// Chooses the live target `target_id` for the rest of the session.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::UnknownTarget`] when no live target has that
    /// id, and [`SessionError::Discovery`] when the endpoint directory cannot
    /// be read.
    pub async fn select_target(&mut self, target_id: &TargetId) -> Result<(), SessionError> {
        let targets = discover(&self.endpoint_dir, Some(&self.app_id)).await?;
        if !targets
            .iter()
            .any(|target| target.target_id() == *target_id)
        {
            return Err(SessionError::UnknownTarget(target_id.clone()));
        }
        self.selected = Some(target_id.clone());
        Ok(())
    }

    /// A client for the session's target, selecting the only live one when
    /// nothing is selected.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::NoTarget`] with no live target,
    /// [`SessionError::Ambiguous`] with several and none selected,
    /// [`SessionError::SelectedTargetGone`] when the selected target stopped
    /// running, and [`SessionError::Discovery`] when the endpoint directory
    /// cannot be read.
    pub async fn client(&mut self) -> Result<BridgeClient, SessionError> {
        let targets = discover(&self.endpoint_dir, Some(&self.app_id)).await?;
        let index = match &self.selected {
            Some(selected) => targets
                .iter()
                .position(|target| target.target_id() == *selected)
                .ok_or_else(|| SessionError::SelectedTargetGone(selected.clone()))?,
            None => match targets.len() {
                0 => {
                    return Err(SessionError::NoTarget {
                        app_id: self.app_id.as_str().to_owned(),
                    });
                }
                1 => 0,
                count => {
                    return Err(SessionError::Ambiguous {
                        app_id: self.app_id.as_str().to_owned(),
                        count,
                    });
                }
            },
        };
        let target = targets
            .into_iter()
            .nth(index)
            .ok_or_else(|| SessionError::NoTarget {
                app_id: self.app_id.as_str().to_owned(),
            })?;
        self.selected = Some(target.target_id());
        Ok(BridgeClient::new(target))
    }
}
