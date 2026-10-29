//! Relay's own tokens, on top of Bolt: method labels, the draft marker, syntax colors and
//! the main window size. The only `relay_ui` file besides `brand.rs` that holds raw values.

#![allow(
    dead_code,
    reason = "the method, draft and syntax tokens are read by the sidebar and editor screens"
)]

use bolt::tokens::hex;
use gpui::{Hsla, Pixels, Size, px, size};

/// The color of one HTTP method label: full strength in the URL bar, dimmed in the tree
/// and in inactive tabs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct MethodColor {
    pub(crate) full: Hsla,
    /// The full color blended over the panel, solid, so it keeps AA on the selected item.
    pub(crate) dim: Hsla,
}

fn method(full: u32, dim: u32) -> MethodColor {
    MethodColor {
        full: hex(full),
        dim: hex(dim),
    }
}

/// Method label colors. They are their own tokens, never aliases of status colors.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct MethodColors {
    pub(crate) get: MethodColor,
    pub(crate) post: MethodColor,
    pub(crate) put: MethodColor,
    pub(crate) patch: MethodColor,
    pub(crate) delete: MethodColor,
    /// HEAD, OPTIONS and custom verbs.
    pub(crate) other: MethodColor,
}

impl MethodColors {
    pub(crate) fn dark() -> Self {
        Self {
            get: method(0x6FD3A0, 0x57A27D),
            post: method(0xF2A65A, 0xB98048),
            put: method(0x5FCFD6, 0x4B9FA5),
            patch: method(0xC3D865, 0x96A650),
            delete: method(0xFF8C80, 0xD4766C),
            other: method(0xB4AEC0, 0x938E9D),
        }
    }
}

/// Body syntax colors: quiet keys, values carry the data.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SyntaxColors {
    pub(crate) key: Hsla,
    pub(crate) string: Hsla,
    /// Numbers, booleans and null.
    pub(crate) number: Hsla,
    pub(crate) punct: Hsla,
}

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

/// The unsaved draft marker, and nothing else.
pub(crate) fn draft() -> Hsla {
    hex(0xF28AC9)
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
    fn method_colors_match_the_design_system() {
        let methods = MethodColors::dark();
        let pairs = [
            (methods.get, 0x6FD3A0, 0x57A27D),
            (methods.post, 0xF2A65A, 0xB98048),
            (methods.put, 0x5FCFD6, 0x4B9FA5),
            (methods.patch, 0xC3D865, 0x96A650),
            (methods.delete, 0xFF8C80, 0xD4766C),
            (methods.other, 0xB4AEC0, 0x938E9D),
        ];
        for (color, full, dim) in pairs {
            assert_eq!(rgb_of(color.full), full, "{full:06X}");
            assert_eq!(rgb_of(color.dim), dim, "{dim:06X}");
        }
    }

    #[test]
    fn draft_and_syntax_match_the_design_system() {
        assert_eq!(rgb_of(draft()), 0xF28AC9);
        let syntax = SyntaxColors::dark();
        assert_eq!(rgb_of(syntax.key), 0xA9A4B2);
        assert_eq!(rgb_of(syntax.string), 0x7BE0A0);
        assert_eq!(rgb_of(syntax.number), 0xB79CFF);
        assert_eq!(rgb_of(syntax.punct), 0x8E8996);
    }
}
