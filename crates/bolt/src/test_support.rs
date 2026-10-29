//! Helpers shared by the unit tests.

use gpui::{Hsla, Rgba};

/// `0xRRGGBB` and the alpha of `color`, rounded to 8 bits per channel.
pub(crate) fn rgba_of(color: Hsla) -> (u32, f32) {
    let rgba = Rgba::from(color);
    let channel = |value: f32| (value * 255.0).round() as u32;
    (
        (channel(rgba.r) << 16) | (channel(rgba.g) << 8) | channel(rgba.b),
        rgba.a,
    )
}
