//! The palette mode the user chose (light-theme S2, S4): kept in the app state under
//! [`THEME_SETTING`], installed before the first window opens, and followed to the OS
//! appearance in [`PaletteMode::System`].

use bolt::PaletteMode;
use gpui::{App, Window};
use reqflux_engine::state::AppState;

/// The app state setting that holds the palette mode.
pub(crate) const THEME_SETTING: &str = "theme";

/// The stored name of `mode`.
pub(crate) fn mode_name(mode: PaletteMode) -> &'static str {
    match mode {
        PaletteMode::Dark => "dark",
        PaletteMode::Light => "light",
        PaletteMode::System => "system",
    }
}

/// The mode stored as `name`, or `None` for a name this build does not know.
pub(crate) fn mode_from_name(name: &str) -> Option<PaletteMode> {
    match name {
        "dark" => Some(PaletteMode::Dark),
        "light" => Some(PaletteMode::Light),
        "system" => Some(PaletteMode::System),
        _ => None,
    }
}

/// The mode saved in `state`; Dark when there is no state, nothing saved, an unknown name or
/// a read failure (which is logged).
pub(crate) fn saved_mode(state: Option<&AppState>) -> PaletteMode {
    let Some(state) = state else {
        return PaletteMode::default();
    };
    match state.setting(THEME_SETTING) {
        Ok(Some(name)) => mode_from_name(&name).unwrap_or_else(|| {
            log::warn!("unknown theme {name:?} in the app state; using the default");
            PaletteMode::default()
        }),
        Ok(None) => PaletteMode::default(),
        Err(error) => {
            log::warn!("cannot read the theme from the app state: {error}");
            PaletteMode::default()
        }
    }
}

/// Saves `mode` in `state`, logging a failure: the switch itself already happened.
pub(crate) fn save_mode(state: Option<&mut AppState>, mode: PaletteMode) {
    let Some(state) = state else {
        return;
    };
    if let Err(error) = state.set_setting(THEME_SETTING, mode_name(mode)) {
        log::warn!("cannot save the theme in the app state: {error}");
    }
}

/// Installs Bolt with ReqFlux's brand in the mode saved in `state`.
pub(crate) fn install(state: Option<&AppState>, cx: &mut App) {
    bolt::install(crate::brand::reqflux(), saved_mode(state), cx);
}

/// Resolves the installed mode against `window`'s appearance now and whenever the OS changes
/// it, so [`PaletteMode::System`] follows the OS.
pub(crate) fn follow_appearance(window: &mut Window, cx: &mut App) {
    bolt::set_mode(bolt::mode(cx), window.appearance(), cx);
    window
        .observe_window_appearance(|window, cx| {
            if bolt::mode(cx) == PaletteMode::System {
                bolt::set_mode(PaletteMode::System, window.appearance(), cx);
            }
        })
        .detach();
}
