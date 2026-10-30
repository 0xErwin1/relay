//! The UI automation bridge installed in every window.
//!
//! [`install`] attaches a `gpui-mcp` bridge to a window so `reqflux mcp`, running as
//! the same user, can discover it, read its rendered element tree and inject
//! input. See "UI automation" in ARCHITECTURE.md for the trust model.

use std::collections::HashMap;

use gpui::{App, Global, Window, WindowId};
use gpui_mcp::{AppId, BridgeConfig, BridgeHandle};

const APP_ID: &str = "reqflux";
const DISPLAY_NAME: &str = "ReqFlux";

/// Bridge handles keyed by the window they serve.
///
/// `BridgeHandle` is not `Send` and stops its listener when dropped, so the
/// handles are owned on the foreground thread for as long as their window exists.
#[derive(Default)]
struct Bridges(HashMap<WindowId, BridgeHandle>);

impl Global for Bridges {}

/// Installs the automation bridge into `window`.
///
/// The bridge lives until the window closes or the application quits, at which
/// point its listener stops and its discovery descriptor is removed. A failure
/// is logged and leaves the window usable without automation.
pub(crate) fn install(window: &mut Window, cx: &mut App) {
    let app_id = match AppId::new(APP_ID) {
        Ok(app_id) => app_id,
        Err(error) => {
            log::error!("UI automation bridge: invalid app id {APP_ID:?}: {error}");
            return;
        }
    };
    let config = BridgeConfig::new(app_id, DISPLAY_NAME);

    let handle = match BridgeHandle::install(window, cx, config) {
        Ok(handle) => handle,
        Err(error) => {
            log::error!("UI automation bridge failed to start: {error}");
            return;
        }
    };

    log::info!(
        "UI automation bridge listening, descriptor {}",
        handle.endpoint_path().display()
    );

    let window_id = window.window_handle().window_id();
    registry(cx).0.insert(window_id, handle);
}

fn registry(cx: &mut App) -> &mut Bridges {
    if !cx.has_global::<Bridges>() {
        cx.on_window_closed(|cx, window_id| {
            if let Some(handle) = cx.default_global::<Bridges>().0.remove(&window_id) {
                handle.shutdown();
            }
        })
        .detach();

        // The platform can exit without closing windows one by one, so the
        // descriptors would outlive the process without this.
        cx.on_app_quit(|cx| {
            for (_, handle) in cx.default_global::<Bridges>().0.drain() {
                handle.shutdown();
            }
            async {}
        })
        .detach();
    }

    cx.default_global::<Bridges>()
}
