//! The title bar: the workspace and environment over the sidebar column, the command search
//! at the center, and the approvals bell on the right. The environment selector opens the
//! environment menu; the workspace selector, the search and the bell do nothing yet.

use bolt::controls::{Field, IconButton};
use bolt::tokens::{border, chrome, field, icon, space};
use bolt::{Icon, IconName};
use gpui::{
    AnyElement, App, Context, FontWeight, Hsla, IntoElement, ParentElement, Pixels, Role, Stateful,
    Styled, Window, div, prelude::*,
};

use super::diamond::Diamond;
use super::{Shell, environment_menu, region, text_region};
use crate::tokens;

/// Shown in place of the environment when none is active.
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

/// The accessible label of the environment selector for the active environment `name`.
pub(super) fn environment_label(name: Option<&str>) -> String {
    format!("Environment: {}", name.unwrap_or("none"))
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
) -> Stateful<gpui::Div> {
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
fn context(shell: &Shell, window: &Window, cx: &mut Context<Shell>) -> impl IntoElement {
    let colors = bolt::colors(cx);
    let mut zone = region("title-context")
        .min_w_0()
        .flex()
        .items_center()
        .gap(border::THICK);
    match shell.workspace.name() {
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
                .child(environment_selector(shell, window, cx));
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

/// The environment selector, with the environment menu under it while open.
fn environment_selector(
    shell: &Shell,
    window: &Window,
    cx: &mut Context<Shell>,
) -> impl IntoElement {
    let colors = bolt::colors(cx);
    let name = shell.environment_name();
    let (diamond, label, color, weight) = match &name {
        Some(name) => (
            bolt::brand(cx).accent_tint,
            name.clone(),
            colors.strong,
            FontWeight::SEMIBOLD,
        ),
        None => (
            colors.muted,
            ENVIRONMENT_PLACEHOLDER.to_owned(),
            colors.muted,
            FontWeight::NORMAL,
        ),
    };
    let button = selector(
        "environment-selector",
        "environment-name",
        Some(Diamond::new(diamond).into_any_element()),
        label,
        color,
        weight,
        cx,
    )
    .role(Role::Button)
    .aria_label(environment_label(name.as_deref()))
    .aria_expanded(shell.environment_menu_open)
    .cursor_pointer()
    .on_click(cx.listener(|this, _event, window, cx| {
        this.toggle_environment_menu(window, cx);
    }));
    div()
        .relative()
        .child(button)
        .when(shell.environment_menu_open, |anchor| {
            anchor.child(environment_menu::render(shell, window, cx))
        })
}

pub(super) fn render(shell: &Shell, window: &Window, cx: &mut Context<Shell>) -> impl IntoElement {
    region("title-bar")
        .flex_none()
        .h(chrome::TITLE_BAR)
        .w_full()
        .flex()
        .items_center()
        .child(context(shell, window, cx))
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
