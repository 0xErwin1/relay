//! The rmcp stdio server that exposes the `ui_*` tools (S6, S7, S8).
//!
//! Every tool handler delegates to the plain functions in [`crate::tools`]
//! under a lock on the shared [`ServerState`], and maps a [`ToolError`] to a
//! `CallToolResult` with `is_error: true`, so a failing bridge call is an
//! error result with the bridge's message, never a panic.

use std::path::PathBuf;
use std::sync::Arc;

use gpui_mcp_protocol::AppId;
use rmcp::{
    ServerHandler, ServiceExt as _,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{
        CallToolResult, ContentBlock, ErrorData, Implementation, ServerCapabilities, ServerConfig,
    },
    tool, tool_handler, tool_router,
    transport::stdio,
};
use tokio::sync::Mutex;

use crate::bridge::BridgeSession;
use crate::tools::{
    self, ClickArgs, FindArgs, IdArgs, KeyArgs, ScrollArgs, SelectTargetArgs, ServerState,
    TextArgs, ToolError, WaitArgs,
};

/// The Relay MCP server: one bridge session and the `ui_*` tool router.
#[derive(Clone, Debug)]
pub struct RelayMcp {
    state: Arc<Mutex<ServerState>>,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl RelayMcp {
    /// A server that drives the app through `session`.
    #[must_use]
    pub fn new(session: BridgeSession) -> Self {
        Self {
            state: Arc::new(Mutex::new(ServerState::new(session))),
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        name = "ui_list_targets",
        description = "List the live bridge targets of the app, each marked when it is selected"
    )]
    async fn ui_list_targets(&self) -> Result<CallToolResult, ErrorData> {
        let mut state = self.state.lock().await;
        into_result(tools::list_targets(&mut state).await)
    }

    #[tool(
        name = "ui_select_target",
        description = "Choose the target the ui_* tools drive for the rest of the session"
    )]
    async fn ui_select_target(
        &self,
        Parameters(args): Parameters<SelectTargetArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let mut state = self.state.lock().await;
        into_result(tools::select_target(&mut state, args).await)
    }

    #[tool(
        name = "ui_ping",
        description = "Check that the selected target is alive; returns its app id and pid"
    )]
    async fn ui_ping(&self) -> Result<CallToolResult, ErrorData> {
        let mut state = self.state.lock().await;
        into_result(tools::ping(&mut state).await)
    }

    #[tool(
        name = "ui_tree",
        description = "Return the latest semantic element tree with its generation, ids, labels, roles, bounds and state"
    )]
    async fn ui_tree(&self) -> Result<CallToolResult, ErrorData> {
        let mut state = self.state.lock().await;
        into_result(tools::tree(&mut state).await)
    }

    #[tool(
        name = "ui_find",
        description = "Find elements by label or id substring and role in the latest tree"
    )]
    async fn ui_find(
        &self,
        Parameters(args): Parameters<FindArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let mut state = self.state.lock().await;
        into_result(tools::find(&mut state, args).await)
    }

    #[tool(
        name = "ui_element",
        description = "Return one element with its parent, children, state, text and value"
    )]
    async fn ui_element(
        &self,
        Parameters(args): Parameters<IdArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let mut state = self.state.lock().await;
        into_result(tools::element(&mut state, args).await)
    }

    #[tool(
        name = "ui_element_bounds",
        description = "Return one element's bounds in logical window pixels"
    )]
    async fn ui_element_bounds(
        &self,
        Parameters(args): Parameters<IdArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let mut state = self.state.lock().await;
        into_result(tools::element_bounds(&mut state, args).await)
    }

    #[tool(
        name = "ui_wait_for_element",
        description = "Poll the tree until a visible element matches the query, then return it"
    )]
    async fn ui_wait_for_element(
        &self,
        Parameters(args): Parameters<WaitArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let mut state = self.state.lock().await;
        into_result(tools::wait_for_element(&mut state, args).await)
    }

    #[tool(
        name = "ui_click",
        description = "Click an element at its center through the app's pointer pipeline"
    )]
    async fn ui_click(
        &self,
        Parameters(args): Parameters<ClickArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let mut state = self.state.lock().await;
        into_result(tools::click(&mut state, args).await)
    }

    #[tool(
        name = "ui_type",
        description = "Type text into the focused field; the app refuses redacted fields"
    )]
    async fn ui_type(
        &self,
        Parameters(args): Parameters<TextArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let mut state = self.state.lock().await;
        into_result(tools::type_text(&mut state, args).await)
    }

    #[tool(
        name = "ui_key",
        description = "Dispatch one GPUI keystroke, such as `enter` or `ctrl-a`"
    )]
    async fn ui_key(
        &self,
        Parameters(args): Parameters<KeyArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let mut state = self.state.lock().await;
        into_result(tools::key(&mut state, args).await)
    }

    #[tool(
        name = "ui_focus",
        description = "Move the app's keyboard focus to one element"
    )]
    async fn ui_focus(
        &self,
        Parameters(args): Parameters<IdArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let mut state = self.state.lock().await;
        into_result(tools::focus(&mut state, args).await)
    }

    #[tool(
        name = "ui_scroll",
        description = "Scroll at an element's center or at window coordinates, with pixel deltas"
    )]
    async fn ui_scroll(
        &self,
        Parameters(args): Parameters<ScrollArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let mut state = self.state.lock().await;
        into_result(tools::scroll(&mut state, args).await)
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for RelayMcp {
    fn get_info(&self) -> ServerConfig {
        let capabilities = ServerCapabilities::builder().enable_tools().build();
        ServerConfig::new(capabilities)
            .with_server_info(Implementation::new("relay", env!("CARGO_PKG_VERSION")))
            .with_instructions(
                "Drive the running Relay window through its gpui-mcp bridge: call \
                 ui_list_targets first when more than one window may be running, then \
                 ui_select_target; a single live target is selected automatically. Read \
                 the tree with ui_tree or ui_find before any input. Coordinates are \
                 logical pixels relative to the selected window."
                    .to_owned(),
            )
    }
}

/// A tool result or an `isError` result carrying the failure's message (S8).
fn into_result(result: Result<String, ToolError>) -> Result<CallToolResult, ErrorData> {
    match result {
        Ok(text) => Ok(CallToolResult::success(vec![ContentBlock::text(text)])),
        Err(error) => Ok(CallToolResult::error(vec![ContentBlock::text(
            error.to_string(),
        )])),
    }
}

/// Serve the `ui_*` tools on stdio until the client disconnects (S1's
/// server half; the `relay mcp` subcommand is wired separately).
///
/// # Errors
///
/// Returns an error when the server cannot start or the transport fails.
pub async fn serve(endpoint_dir: PathBuf, app_id: AppId) -> anyhow::Result<()> {
    log::info!(target: "relay_mcp", "serving ui_* tools on stdio for app {app_id}");
    let server = RelayMcp::new(BridgeSession::new(endpoint_dir, app_id));
    let running = server.serve(stdio()).await?;
    running.waiting().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOOL_NAMES: [&str; 13] = [
        "ui_list_targets",
        "ui_select_target",
        "ui_ping",
        "ui_tree",
        "ui_find",
        "ui_element",
        "ui_element_bounds",
        "ui_wait_for_element",
        "ui_click",
        "ui_type",
        "ui_key",
        "ui_focus",
        "ui_scroll",
    ];

    #[allow(clippy::unwrap_used, clippy::expect_used)]
    fn server() -> RelayMcp {
        RelayMcp::new(BridgeSession::new(
            std::env::temp_dir().join("relay-mcp-server-tests"),
            AppId::new("relay").unwrap(),
        ))
    }

    #[test]
    fn every_ui_tool_is_registered() {
        let server = server();
        for name in TOOL_NAMES {
            assert!(server.tool_router.has_route(name), "{name} is registered");
        }
    }

    #[test]
    fn the_server_names_relay_with_the_crate_version() {
        let info = server().get_info();
        assert_eq!(info.server_info.name, "relay");
        assert_eq!(info.server_info.version, env!("CARGO_PKG_VERSION"));
        assert!(info.capabilities.tools.is_some());
    }

    #[allow(clippy::unwrap_used, clippy::expect_used)]
    #[test]
    fn a_tool_error_becomes_an_is_error_result_with_the_message() {
        let result =
            into_result(Err(ToolError::Invalid("boom".to_owned()))).unwrap_or_else(|error| {
                panic!("error results are values, not protocol errors: {error}")
            });
        assert_eq!(result.is_error, Some(true));
        let rmcp::model::ContentBlock::Text(text) = &result.content[0] else {
            panic!("expected a text block");
        };
        assert_eq!(text.text, "boom");
    }

    #[allow(clippy::unwrap_used, clippy::expect_used)]
    #[test]
    fn a_tool_success_is_a_text_result_without_is_error() {
        let result = into_result(Ok("{\"ok\": true}".to_owned()))
            .unwrap_or_else(|error| panic!("expected a result: {error}"));
        assert_eq!(result.is_error, Some(false), "success is not an error");
        let rmcp::model::ContentBlock::Text(text) = &result.content[0] else {
            panic!("expected a text block");
        };
        assert_eq!(text.text, "{\"ok\": true}");
    }
}
