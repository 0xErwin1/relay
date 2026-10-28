//! The Relay binary: GUI startup and, later, the `mcp`, `run` and `import` subcommands.

use std::cell::RefCell;
use std::process::ExitCode;
use std::rc::Rc;

use gpui::App;

fn main() -> ExitCode {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let startup_error: Rc<RefCell<Option<anyhow::Error>>> = Rc::default();

    let error_slot = Rc::clone(&startup_error);
    gpui_platform::application().run(move |cx: &mut App| {
        if let Err(error) = relay_ui::open_main_window(cx) {
            error_slot.replace(Some(error));
            cx.quit();
        }
    });

    match startup_error.take() {
        None => ExitCode::SUCCESS,
        Some(error) => {
            eprintln!("relay: failed to open the main window: {error:#}");
            ExitCode::FAILURE
        }
    }
}
