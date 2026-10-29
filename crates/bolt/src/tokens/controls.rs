//! Metrics of the shell controls: icon button, keycap, field, rail button and status chip.
//!
//! Heights shared with other controls live in [`super::chrome`]; cuts in [`super::cut`].

/// The icon-only button.
pub mod icon_button {
    use gpui::{Pixels, px};

    /// Width of an icon button on a [`crate::tokens::chrome::CONTROL`] height.
    pub const WIDTH: Pixels = px(32.0);
}

/// The keycap that prints a shortcut.
pub mod keycap {
    use gpui::{Pixels, px};

    /// Text size, in the mono family.
    pub const FONT: Pixels = px(10.5);
    /// Line height.
    pub const LINE_HEIGHT: Pixels = px(14.0);
    /// Horizontal padding.
    pub const PADDING_X: Pixels = px(6.0);
    /// Vertical padding.
    pub const PADDING_Y: Pixels = px(2.0);
}

/// The single-line field.
pub mod field {
    use gpui::{Pixels, px};

    /// Horizontal padding.
    pub const PADDING_X: Pixels = px(10.0);
    /// Gap between the leading icon, the text and the trailing keycap.
    pub const GAP: Pixels = px(8.0);
    /// Text size.
    pub const FONT: Pixels = px(12.5);
}

/// The activity rail button.
pub mod rail {
    use gpui::{Pixels, px};

    /// Side of the square button.
    pub const BUTTON: Pixels = px(38.0);
    /// Distance of the count badge from the button's top and right edges.
    pub const BADGE_INSET: Pixels = px(3.0);
    /// Horizontal padding of the count badge.
    pub const BADGE_PADDING_X: Pixels = px(3.0);
    /// Text size of the count badge.
    pub const BADGE_FONT: Pixels = px(9.0);
    /// Line height, and so height, of the count badge.
    pub const BADGE_LINE_HEIGHT: Pixels = px(13.0);
    /// The largest count a badge prints; larger counts read as this number and a plus.
    pub const BADGE_MAX: usize = 99;
}

/// The status bar chip.
pub mod status_chip {
    use gpui::{Pixels, px};

    /// Horizontal padding.
    pub const PADDING_X: Pixels = px(12.0);
    /// Gap between the icon and the text.
    pub const GAP: Pixels = px(8.0);
    /// Text size.
    pub const FONT: Pixels = px(12.0);
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::px;

    #[test]
    fn control_metrics_match_the_design_system() {
        assert_eq!(icon_button::WIDTH, px(32.0));
        assert_eq!(
            [
                keycap::FONT,
                keycap::LINE_HEIGHT,
                keycap::PADDING_X,
                keycap::PADDING_Y
            ],
            [px(10.5), px(14.0), px(6.0), px(2.0)]
        );
        assert_eq!(
            [field::PADDING_X, field::GAP, field::FONT],
            [px(10.0), px(8.0), px(12.5)]
        );
        assert_eq!(
            [
                rail::BUTTON,
                rail::BADGE_INSET,
                rail::BADGE_PADDING_X,
                rail::BADGE_FONT,
                rail::BADGE_LINE_HEIGHT
            ],
            [px(38.0), px(3.0), px(3.0), px(9.0), px(13.0)]
        );
        assert_eq!(rail::BADGE_MAX, 99);
        assert_eq!(
            [status_chip::PADDING_X, status_chip::GAP, status_chip::FONT],
            [px(12.0), px(8.0), px(12.0)]
        );
    }
}
