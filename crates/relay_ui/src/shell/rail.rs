//! The activity rail on the desk: one button per view, Settings at the bottom. Only
//! Collections exists yet, so it is the active entry and the others do nothing.

use bolt::IconName;
use bolt::controls::RailButton;
use bolt::tokens::{chrome, space};
use gpui::{IntoElement, ParentElement, Styled, div};

use super::region;

/// The views of the rail above the spacer: element id, icon and label.
pub(crate) const VIEWS: [(&str, IconName, &str); 5] = [
    ("rail-collections", IconName::Box, "Collections"),
    ("rail-history", IconName::RotateCcwClock, "History"),
    ("rail-environments", IconName::Layers, "Environments"),
    ("rail-approvals", IconName::Bot, "MCP approvals"),
    ("rail-audit", IconName::FingerprintPattern, "Audit log"),
];

/// The element id of the Settings button at the bottom of the rail.
pub(crate) const SETTINGS: &str = "rail-settings";

pub(super) fn render() -> impl IntoElement {
    region("activity-rail")
        .flex_none()
        .w(chrome::RAIL_WIDTH)
        .h_full()
        .flex()
        .flex_col()
        .items_center()
        .gap(space::HALF)
        .pt(space::XS)
        .pb(space::XS)
        .children(VIEWS.iter().map(|&(id, icon, label)| {
            RailButton::new(id, icon, label).active(id == "rail-collections")
        }))
        .child(div().flex_1())
        .child(RailButton::new(SETTINGS, IconName::Settings, "Settings"))
}
