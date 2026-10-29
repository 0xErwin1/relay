//! The title bar: the workspace and environment over the sidebar column, the command search
//! at the center, and the approvals bell on the right. Only the labels are live; the
//! selectors, the search and the bell do nothing yet.

use bolt::controls::{Field, IconButton};
use bolt::tokens::{border, chrome, field, icon, space};
use bolt::{Icon, IconName};
use gpui::{AnyElement, App, FontWeight, Hsla, IntoElement, ParentElement, Pixels, Styled, div};

use super::diamond::Diamond;
use super::{region, text_region};
use crate::tokens;
use crate::workspace::OpenedWorkspace;

/// Shown in place of the environment until environments can be selected.
pub(crate) const ENVIRONMENT_PLACEHOLDER: &str = "No environment";

/// Shown in place of the workspace name when none is loaded.
pub(crate) const WORKSPACE_PLACEHOLDER: &str = "No workspace";

/// The placeholder of the command search field.
pub(crate) const COMMAND_SEARCH: &str = "Search or run a command";

/// The shortcut printed on the command search field.
pub(crate) const COMMAND_SEARCH_KEYS: &str = "Ctrl Shift P";

/// The left inset of the title bar: the desk gap, the activity rail and the gap before the
/// sidebar island, so the workspace selector sits over the sidebar column.
fn sidebar_column() -> Pixels {
    chrome::ISLAND_GAP + chrome::RAIL_WIDTH + chrome::ISLAND_GAP
}

/// A selector of the title bar: `leading`, then `label` in `color`, then a chevron.
fn selector(
    id: &'static str,
    label_id: &'static str,
    leading: Option<AnyElement>,
    label: String,
    color: Hsla,
    weight: FontWeight,
    cx: &App,
) -> impl IntoElement {
    let colors = bolt::colors(cx);
    region(id)
        .flex()
        .flex_none()
        .items_center()
        .gap(space::SM)
        .h(chrome::CONTROL)
        .pl(field::PADDING_X)
        .pr(space::SM)
        .text_size(field::FONT)
        .children(leading)
        .child(
            text_region(label_id)
                .text_color(color)
                .font_weight(weight)
                .whitespace_nowrap()
                .child(label),
        )
        .child(
            Icon::new(IconName::ChevronDown)
                .size(icon::SM)
                .color(colors.muted),
        )
}

/// The workspace and environment selectors. The zone keeps its content width, so on a narrow
/// window the command search shrinks instead of covering it.
fn context(workspace: &OpenedWorkspace, cx: &App) -> impl IntoElement {
    let colors = bolt::colors(cx);
    let mut zone = region("title-context")
        .min_w_0()
        .flex()
        .items_center()
        .gap(border::THICK);
    match workspace.name() {
        Some(name) => {
            zone = zone
                .child(selector(
                    "workspace-selector",
                    "workspace-name",
                    Some(
                        Icon::new(IconName::LayoutGrid)
                            .size(icon::MD)
                            .color(colors.muted)
                            .into_any_element(),
                    ),
                    name.to_owned(),
                    colors.strong,
                    FontWeight::SEMIBOLD,
                    cx,
                ))
                .child(
                    Icon::new(IconName::ChevronRight)
                        .size(icon::SM)
                        .color(colors.line_2),
                )
                .child(selector(
                    "environment-selector",
                    "environment-name",
                    Some(Diamond::new(colors.muted).into_any_element()),
                    ENVIRONMENT_PLACEHOLDER.to_owned(),
                    colors.muted,
                    FontWeight::NORMAL,
                    cx,
                ));
        }
        None => {
            zone = zone.child(selector(
                "workspace-selector",
                "workspace-name",
                None,
                WORKSPACE_PLACEHOLDER.to_owned(),
                colors.muted,
                FontWeight::NORMAL,
                cx,
            ));
        }
    }
    div()
        .flex_1()
        .flex()
        .child(div().flex().pl(sidebar_column()).child(zone))
}

pub(super) fn render(workspace: &OpenedWorkspace, cx: &App) -> impl IntoElement {
    region("title-bar")
        .flex_none()
        .h(chrome::TITLE_BAR)
        .w_full()
        .flex()
        .items_center()
        .child(context(workspace, cx))
        .child(
            Field::new("command-search", COMMAND_SEARCH)
                .leading_icon(IconName::Search)
                .trailing_keycap(COMMAND_SEARCH_KEYS)
                .w(tokens::COMMAND_SEARCH_WIDTH)
                .flex_shrink(1.0)
                .min_w_0(),
        )
        .child(
            div().flex_1().min_w_0().flex().justify_end().child(
                div().pr(space::MD).child(IconButton::new(
                    "approvals",
                    IconName::Bell,
                    "Approvals",
                )),
            ),
        )
}
