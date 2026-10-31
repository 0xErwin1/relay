//! The theme command (light-theme S2-S4): Ctrl+Shift+T and the palette cycle Dark, Light and
//! System, the palette and product colors follow, and the choice persists in the app state.

use bolt::tokens::Colors;
use bolt::{Palette, PaletteMode};
use gpui::{Entity, TestAppContext, VisualTestContext};
use reqflux_engine::state::AppState;

use super::Shell;
use super::draft_tests::{render, state_file};
use crate::tokens::method_color;
use crate::workspace_tests::demo_workspace;

fn focus_shell(cx: &mut VisualTestContext, shell: &Entity<Shell>) {
    cx.update(|window, cx| {
        let focus = shell.read(cx).initial_focus();
        focus.focus(window, cx);
    });
    cx.run_until_parked();
}

fn press(cx: &mut VisualTestContext, keys: &str) {
    cx.simulate_keystrokes(keys);
    cx.run_until_parked();
}

fn mode(cx: &mut VisualTestContext) -> PaletteMode {
    cx.update(|_, cx| bolt::mode(cx))
}

fn saved(path: &std::path::Path) -> Option<String> {
    match AppState::open(path).and_then(|state| state.setting(crate::theme::THEME_SETTING)) {
        Ok(value) => value,
        Err(error) => panic!("read the setting: {error}"),
    }
}

#[gpui::test]
fn ctrl_shift_t_cycles_dark_light_system_and_the_palette_follows(cx: &mut TestAppContext) {
    let dir = demo_workspace();
    let (_state_dir, state_path) = state_file();
    let (shell, cx) = render(cx, &dir, &state_path);
    focus_shell(cx, &shell);
    assert_eq!(mode(cx), PaletteMode::Dark, "dark is the default");
    assert_eq!(cx.update(|_, cx| *bolt::colors(cx)), Colors::dark());
    let dark_get = cx.update(|_, cx| method_color("GET", bolt::palette(cx)));

    press(cx, "ctrl-shift-t");
    assert_eq!(mode(cx), PaletteMode::Light);
    assert_eq!(cx.update(|_, cx| *bolt::colors(cx)), Colors::light());
    let light_get = cx.update(|_, cx| method_color("GET", bolt::palette(cx)));
    assert_ne!(dark_get, light_get, "a product color follows the palette");
    assert_eq!(saved(&state_path).as_deref(), Some("light"));

    press(cx, "ctrl-shift-t");
    assert_eq!(mode(cx), PaletteMode::System);
    let appearance = cx.update(|window, _| window.appearance());
    let expected = PaletteMode::System.resolve(appearance).colors();
    assert_eq!(cx.update(|_, cx| *bolt::colors(cx)), expected);
    assert_eq!(saved(&state_path).as_deref(), Some("system"));

    press(cx, "ctrl-shift-t");
    assert_eq!(mode(cx), PaletteMode::Dark);
    assert_eq!(cx.update(|_, cx| *bolt::colors(cx)), Colors::dark());
    assert_eq!(saved(&state_path).as_deref(), Some("dark"));
}

#[gpui::test]
fn the_palette_lists_toggle_theme_and_runs_it(cx: &mut TestAppContext) {
    let dir = demo_workspace();
    let (_state_dir, state_path) = state_file();
    let (shell, cx) = render(cx, &dir, &state_path);
    let automation = gpui_mcp::Automation::isolated();
    cx.update(|window, _| automation.attach(window));
    focus_shell(cx, &shell);

    press(cx, "ctrl-k");
    cx.simulate_input("toggle theme");
    cx.run_until_parked();
    let tree = automation.snapshot();
    let first = tree
        .nodes
        .get("palette-row-1")
        .and_then(|row| row.label.clone());
    assert_eq!(first.as_deref(), Some("Toggle theme Ctrl+Shift+T"));

    press(cx, "enter");
    assert_eq!(mode(cx), PaletteMode::Light);
    assert_eq!(cx.update(|_, cx| bolt::palette(cx)), Palette::Light);
}

#[gpui::test]
fn the_choice_is_installed_again_from_the_same_state_file(cx: &mut TestAppContext) {
    let dir = demo_workspace();
    let (_state_dir, state_path) = state_file();
    {
        let (shell, cx) = render(cx, &dir, &state_path);
        focus_shell(cx, &shell);
        press(cx, "ctrl-shift-t");
        assert_eq!(mode(cx), PaletteMode::Light);
    }
    cx.update(|cx| bolt::install(crate::brand::reqflux(), PaletteMode::Dark, cx));

    let state = match AppState::open(&state_path) {
        Ok(state) => state,
        Err(error) => panic!("reopen the state: {error}"),
    };
    cx.update(|cx| crate::theme::install(Some(&state), cx));

    cx.update(|cx| {
        assert_eq!(bolt::mode(cx), PaletteMode::Light);
        assert_eq!(*bolt::colors(cx), Colors::light());
    });
    let (_shell, cx) = cx.add_window_view(|_, cx| {
        Shell::with_state(
            crate::workspace::OpenedWorkspace::open(Some(dir.path().to_path_buf())),
            Some(state),
            cx,
        )
    });
    cx.run_until_parked();
    assert_eq!(mode(cx), PaletteMode::Light, "a new shell keeps it");
}

#[test]
fn a_missing_or_unknown_setting_is_dark() {
    assert_eq!(crate::theme::saved_mode(None), PaletteMode::Dark);
    assert_eq!(
        crate::theme::mode_from_name("light"),
        Some(PaletteMode::Light)
    );
    assert_eq!(
        crate::theme::mode_from_name("system"),
        Some(PaletteMode::System)
    );
    assert_eq!(
        crate::theme::mode_from_name("dark"),
        Some(PaletteMode::Dark)
    );
    assert_eq!(crate::theme::mode_from_name("sepia"), None);
}
