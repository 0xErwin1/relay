//! Quitting on SIGINT and SIGTERM.
//!
//! The default action of both signals ends the process at once, so GPUI never runs its quit
//! hooks and the UI automation bridge leaves its discovery descriptor behind. Instead, the
//! signals only raise a flag, and a foreground task polls it and quits through GPUI.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use gpui::App;

/// How often the foreground task checks for a pending signal.
const POLL_INTERVAL: Duration = Duration::from_millis(100);

/// Quits the application through GPUI when SIGINT or SIGTERM arrives.
///
/// A failure to register a handler is logged and leaves that signal's default behavior.
pub(crate) fn quit_on_termination(cx: &mut App) {
    let requested = Arc::new(AtomicBool::new(false));
    for signal in [signal_hook::consts::SIGINT, signal_hook::consts::SIGTERM] {
        if let Err(error) = signal_hook::flag::register(signal, Arc::clone(&requested)) {
            log::warn!("could not handle signal {signal}: {error}");
        }
    }

    cx.spawn(async move |cx| {
        loop {
            cx.background_executor().timer(POLL_INTERVAL).await;
            if requested.load(Ordering::Relaxed) {
                log::info!("termination signal received, quitting");
                cx.update(|cx| cx.quit());
                break;
            }
        }
    })
    .detach();
}
