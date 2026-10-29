//! The environment diamond: a small square turned 45 degrees, as DBFlux marks environments.

use gpui::prelude::*;
use gpui::{App, Bounds, Hsla, PathBuilder, Pixels, Window, canvas, div, point};

use crate::tokens;

/// A filled diamond `tokens::ENVIRONMENT_DIAMOND` wide in `color`.
#[derive(IntoElement, Clone, Copy, Debug)]
pub(super) struct Diamond {
    color: Hsla,
}

impl Diamond {
    pub(super) fn new(color: impl Into<Hsla>) -> Self {
        Self {
            color: color.into(),
        }
    }
}

impl RenderOnce for Diamond {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div().size(tokens::ENVIRONMENT_DIAMOND).flex_none().child(
            canvas(
                |_, _, _| {},
                move |bounds, (), window, _| paint(bounds, self.color, window),
            )
            .size_full(),
        )
    }
}

fn paint(bounds: Bounds<Pixels>, color: Hsla, window: &mut Window) {
    let center = bounds.center();
    let half_width = bounds.size.width / 2.0;
    let half_height = bounds.size.height / 2.0;
    let mut builder = PathBuilder::fill();
    builder.move_to(point(center.x, bounds.top()));
    builder.line_to(point(center.x + half_width, center.y));
    builder.line_to(point(center.x, center.y + half_height));
    builder.line_to(point(center.x - half_width, center.y));
    builder.close();
    match builder.build() {
        Ok(path) => window.paint_path(path, color),
        Err(error) => log::warn!("diamond path failed to build: {error}"),
    }
}
