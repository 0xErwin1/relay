use gpui::TestAppContext;
use gpui_component::theme::Theme;

use super::*;
use crate::test_support::rgba_of;
use crate::tokens::hex;

fn brand() -> Brand {
    Brand {
        name: "Product".into(),
        accent: hex(0x2B45B8),
        accent_hover: hex(0x3451C8),
        accent_pressed: hex(0x1F3389),
        accent_tint: hex(0x9DB1FF),
        on_accent: hex(0xFFFFFF),
    }
}

#[gpui::test]
fn install_maps_roles_onto_the_component_theme(cx: &mut TestAppContext) {
    cx.update(|cx| install(brand(), cx));
    cx.update(|cx| {
        let colors = Colors::dark();
        let theme = Theme::global(cx);

        assert!(theme.is_dark());
        assert_eq!(theme.colors.background, colors.background);
        assert_eq!(theme.colors.foreground, colors.foreground);
        assert_eq!(theme.colors.muted_foreground, colors.muted);
        assert_eq!(theme.colors.border, colors.line);
        assert_eq!(theme.colors.popover, colors.panel);
        assert_eq!(theme.colors.title_bar, colors.desk);
        assert_eq!(theme.colors.status_bar, colors.desk);
        assert_eq!(theme.colors.overlay, colors.scrim);
        assert_eq!(theme.colors.danger, colors.danger);
        assert_eq!(theme.colors.primary, brand().accent);
        assert_eq!(theme.colors.primary_foreground, brand().on_accent);
        assert_eq!(theme.colors.ring, brand().accent_tint);
        assert_eq!(theme.colors.caret, brand().accent_tint);
        assert_eq!(rgba_of(theme.colors.selection), (0x9DB1FF, 0.25));
        assert_eq!(rgba_of(theme.colors.list_active), (0x9DB1FF, 0.12));
        assert_eq!(rgba_of(theme.colors.table_active), (0x9DB1FF, 0.07));
        assert_eq!(rgba_of(theme.colors.drop_target), (0x9DB1FF, 0.10));

        assert_eq!(theme.font_family.as_ref(), family::INTERFACE);
        assert_eq!(theme.mono_font_family.as_ref(), family::MONO);
        assert_eq!(theme.radius, cut::RADIUS);
        assert_eq!(theme.radius_lg, cut::RADIUS);
    });
}

/// Widgets paint from `Theme::tokens`, which must follow the mapped colors rather than
/// gpui-component's default dark theme.
#[gpui::test]
fn component_tokens_follow_the_mapped_colors(cx: &mut TestAppContext) {
    cx.update(|cx| install(brand(), cx));
    cx.update(|cx| {
        let theme = Theme::global(cx);
        assert_eq!(theme.tokens.primary.color, brand().accent);
        assert_eq!(theme.tokens.button_primary.color, brand().accent);
        assert_eq!(
            theme.tokens.button_primary_hover.color,
            brand().accent_hover
        );
        assert_eq!(theme.tokens.button.color, Colors::dark().raised);
        assert_eq!(theme.tokens.background.color, Colors::dark().background);
        assert_eq!(theme.tokens.list_active.color, theme.colors.list_active);
    });
}

#[gpui::test]
fn accessors_read_the_installed_roles(cx: &mut TestAppContext) {
    cx.update(|cx| {
        assert_eq!(*colors(cx), Colors::dark(), "before install");
        install(brand(), cx);
        assert_eq!(*colors(cx), Colors::dark());
        assert_eq!(*brand_of(cx), brand());
        assert_eq!(*accent_washes(cx), brand().washes());
    });
}

fn brand_of(cx: &App) -> &Brand {
    super::brand(cx)
}

/// A second install replaces the brand everywhere.
#[gpui::test]
fn install_again_replaces_the_brand(cx: &mut TestAppContext) {
    let other = Brand {
        accent: hex(0x702963),
        accent_tint: hex(0xD48CC8),
        ..brand()
    };
    cx.update(|cx| install(brand(), cx));
    cx.update(|cx| install(other.clone(), cx));
    cx.update(|cx| {
        assert_eq!(Theme::global(cx).colors.primary, other.accent);
        assert_eq!(Theme::global(cx).tokens.primary.color, other.accent);
        assert_eq!(super::brand(cx).accent_tint, other.accent_tint);
    });
}
