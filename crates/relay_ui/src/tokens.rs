//! Relay's own tokens, on top of Bolt: method labels, the draft marker, syntax colors, shell
//! sizes and the main window size. The only `relay_ui` file besides `brand.rs` that holds raw values.

use bolt::tokens::{Colors, hex, hex_alpha};
use gpui::{Hsla, Pixels, Size, px, size};

/// The label color of the uppercase HTTP `verb`: Bolt's semantic hues, as DBFlux colors its
/// object kinds. GET success, POST warning, PUT info, PATCH the null purple, DELETE danger,
/// and HEAD, OPTIONS and custom verbs cyan.
pub(crate) fn method_color(verb: &str, colors: &Colors) -> Hsla {
    match verb {
        "GET" => colors.success,
        "POST" => colors.warning,
        "PUT" => colors.info,
        "PATCH" => method_patch(),
        "DELETE" => colors.danger,
        _ => method_other(),
    }
}

/// The PATCH label: the purple Bolt gives `null` in data views.
fn method_patch() -> Hsla {
    hex(0xB79CFF)
}

/// HEAD, OPTIONS and custom verbs.
fn method_other() -> Hsla {
    hex(0x6FD3D8)
}

/// Body syntax colors: quiet keys, values carry the data.
#[allow(
    dead_code,
    reason = "read by the response body view, which does not exist yet"
)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SyntaxColors {
    pub(crate) key: Hsla,
    pub(crate) string: Hsla,
    /// Numbers, booleans and null.
    pub(crate) number: Hsla,
    pub(crate) punct: Hsla,
}

#[allow(
    dead_code,
    reason = "read by the response body view, which does not exist yet"
)]
impl SyntaxColors {
    pub(crate) fn dark() -> Self {
        Self {
            key: hex(0xA9A4B2),
            string: hex(0x7BE0A0),
            number: hex(0xB79CFF),
            punct: hex(0x8E8996),
        }
    }
}

/// The width of the method column of a tree row: `OPTIONS`, the longest standard verb, in
/// JetBrains Mono at `METHOD_LABEL_SIZE` (about 42 px), plus room before the name.
pub(crate) const TREE_METHOD_COLUMN: Pixels = px(46.0);

/// The size of a method label in a tree row or tab.
pub(crate) const METHOD_LABEL_SIZE: Pixels = px(10.0);

/// The size of the child count on the right of a collection or folder row.
pub(crate) const TREE_COUNT_SIZE: Pixels = px(11.0);

/// The gap between the parts of a tree row.
pub(crate) const TREE_ROW_GAP: Pixels = px(7.0);

/// Where the guide line of the first indent level sits from the left edge of a tree row; each
/// deeper level adds `chrome::TREE_INDENT`.
pub(crate) const TREE_GUIDE_OFFSET: Pixels = px(19.0);

/// The height of the sidebar header that holds `COLLECTIONS` and the new-request button.
pub(crate) const SIDEBAR_HEADER: Pixels = px(46.0);

/// The bottom padding under the sidebar filter field.
pub(crate) const SIDEBAR_FILTER_BOTTOM: Pixels = px(10.0);

/// The height of the sidebar footer with the request and collection counts.
pub(crate) const SIDEBAR_FOOTER: Pixels = px(40.0);

/// The width of the command search field at the center of the title bar.
pub(crate) const COMMAND_SEARCH_WIDTH: Pixels = px(420.0);

/// The side of the environment diamond in the title bar.
pub(crate) const ENVIRONMENT_DIAMOND: Pixels = px(7.0);

/// The height of the document path bar under the tab row.
pub(crate) const PATH_BAR: Pixels = px(28.0);

/// The horizontal padding of the document path bar.
pub(crate) const PATH_BAR_PADDING_X: Pixels = px(14.0);

/// The side of the chevron between path bar segments.
pub(crate) const PATH_SEPARATOR: Pixels = px(10.0);

/// The height of the request line: method, URL and Send.
pub(crate) const REQUEST_LINE: Pixels = px(48.0);

/// The width of the method chip on the request line.
pub(crate) const METHOD_CHIP_WIDTH: Pixels = px(96.0);

/// The height of the URL field and the Send button.
pub(crate) const URL_FIELD: Pixels = px(32.0);

/// The width of the options part of the Send split button.
pub(crate) const SEND_OPTIONS_WIDTH: Pixels = px(28.0);

/// The text size of the method chip and the Send button.
pub(crate) const REQUEST_LINE_TEXT: Pixels = px(12.5);

/// The text size of the URL.
pub(crate) const URL_TEXT: Pixels = px(13.0);

/// The opacity of the wash behind a `{{name}}` reference in the URL.
pub(crate) const VARIABLE_WASH: f32 = 0.12;

/// The horizontal and vertical padding of the keycap on the Send button.
pub(crate) const KEYCAP_PADDING_X: Pixels = px(6.0);
pub(crate) const KEYCAP_PADDING_Y: Pixels = px(2.0);

/// The text size of keycaps and sub-tab row counts.
pub(crate) const SMALL_MONO_TEXT: Pixels = px(10.5);

/// The keycap fill on the accent: the accent darkened.
pub(crate) fn keycap_on_accent_fill() -> Hsla {
    hex_alpha(0x000000, 0.25)
}

/// The keycap text on the accent.
pub(crate) fn keycap_on_accent_text() -> Hsla {
    hex_alpha(0xFFFFFF, 0.82)
}

/// The height of the request sub-tab bar.
pub(crate) const SUB_TAB_BAR: Pixels = px(42.0);

/// The text size of the unresolved notice at the right of the sub-tab bar.
pub(crate) const NOTICE_TEXT: Pixels = px(12.0);

/// The heights of the Params and Headers grid header and data rows.
pub(crate) const GRID_HEADER: Pixels = px(32.0);
pub(crate) const GRID_ROW: Pixels = px(29.0);

/// The text sizes of the grid header and data rows.
pub(crate) const GRID_HEADER_TEXT: Pixels = px(12.0);
pub(crate) const GRID_TEXT: Pixels = px(12.5);

/// The grid columns: row number, key and value; the description fills the rest.
pub(crate) const GRID_NUMBER_COLUMN: Pixels = px(46.0);
pub(crate) const GRID_KEY_COLUMN: Pixels = px(200.0);
pub(crate) const GRID_VALUE_COLUMN: Pixels = px(420.0);

/// The text and icon size of the SECRET badge.
pub(crate) const BADGE_TEXT: Pixels = px(10.0);

/// The height of a code line in the Body and Scripts panes.
pub(crate) const CODE_LINE: Pixels = px(20.0);

/// The width of the label column of the Auth and Settings rows.
pub(crate) const PANE_LABEL_COLUMN: Pixels = px(160.0);

/// The opacity of the warning wash on a grid row with an unresolved reference.
const UNRESOLVED_ROW_WASH: f32 = 0.05;

/// The wash of a grid row with an unresolved reference: the warning, faint.
pub(crate) fn unresolved_row_wash(colors: &Colors) -> Hsla {
    Hsla {
        a: UNRESOLVED_ROW_WASH,
        ..colors.warning
    }
}

/// The size the main window opens at.
pub(crate) const MAIN_WINDOW_SIZE: Size<Pixels> = size(px(1200.0), px(800.0));

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use gpui::Rgba;

    /// `0xRRGGBB` of an opaque `color`.
    pub(crate) fn rgb_of(color: Hsla) -> u32 {
        let rgba = Rgba::from(color);
        assert_eq!(rgba.a, 1.0);
        let channel = |value: f32| (value * 255.0).round() as u32;
        (channel(rgba.r) << 16) | (channel(rgba.g) << 8) | channel(rgba.b)
    }

    #[test]
    fn methods_use_the_bolt_semantic_hues() {
        let colors = Colors::dark();
        let pairs = [
            ("GET", 0x7BE0A0),
            ("POST", 0xFFC23D),
            ("PUT", 0x6EA8FF),
            ("PATCH", 0xB79CFF),
            ("DELETE", 0xFF6B5E),
        ];
        for (verb, rgb) in pairs {
            assert_eq!(rgb_of(method_color(verb, &colors)), rgb, "{verb}");
        }
        assert_eq!(method_color("GET", &colors), colors.success);
        assert_eq!(method_color("DELETE", &colors), colors.danger);
    }

    #[test]
    fn other_verbs_are_cyan() {
        let colors = Colors::dark();
        for verb in ["HEAD", "OPTIONS", "PURGE"] {
            assert_eq!(rgb_of(method_color(verb, &colors)), 0x6FD3D8, "{verb}");
        }
    }

    #[test]
    fn syntax_colors_match_the_design_system() {
        let syntax = SyntaxColors::dark();
        assert_eq!(rgb_of(syntax.key), 0xA9A4B2);
        assert_eq!(rgb_of(syntax.string), 0x7BE0A0);
        assert_eq!(rgb_of(syntax.number), 0xB79CFF);
        assert_eq!(rgb_of(syntax.punct), 0x8E8996);
    }
}
