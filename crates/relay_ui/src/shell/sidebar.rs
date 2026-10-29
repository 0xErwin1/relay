//! The sidebar island. Its collection and environment tree comes later.

use bolt::Island;
use bolt::tokens::chrome;
use gpui::{IntoElement, ParentElement, Styled};

use super::region;

pub(super) fn render() -> impl IntoElement {
    region("sidebar")
        .flex_none()
        .w(chrome::SIDEBAR_WIDTH)
        .h_full()
        .child(Island::new().size_full().child(region("sidebar-content")))
}
