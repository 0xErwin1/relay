//! The Variables document (variables-editor S6): the variables of the scope the Variables
//! panel selected, as a grid like Params. Each row `variables-row-<n>` has its enable box,
//! key, shared value, current value, secret toggle, description and delete control; the
//! `Add` row appends one. A secret's value shows `••••••••` and its current value "same as
//! value". Current values of keys the scope does not declare follow as
//! `variables-undeclared-<n>`, with a read-only key, an editable current value and a
//! "not declared" hint, so values a script set are visible and clearable.

use bolt::chamfer::Chamfer;
use bolt::tokens::{Colors, border, cut, family, space, text};
use bolt::{Icon, IconName};
use gpui::{
    AnyElement, Context, Div, FontWeight, ParentElement, Pixels, Role, Stateful, Styled, Toggled,
    div, prelude::*,
};
use gpui_component::input::{Enter, Escape, Input};
use reqflux_core::model::Variable;
use reqflux_engine::SecretScope;

use super::rows::{labeled_row, part};
use super::variables_model::{Column, scope_title, undeclared};
use super::{Shell, region, text as text_role, text_region};
use crate::tokens;
use crate::workspace::OpenedWorkspace;

/// What a secret's value cell shows.
const MASK: &str = "••••••••";

/// The document of `scope`, `variables-document`, titled `variables-title`.
pub(super) fn render(shell: &Shell, scope: SecretScope, cx: &mut Context<Shell>) -> AnyElement {
    let title = match &shell.workspace {
        OpenedWorkspace::Loaded(engine) => scope_title(engine.workspace(), scope),
        OpenedWorkspace::None | OpenedWorkspace::Failed { .. } => None,
    }
    .unwrap_or_default();
    let variables = shell.variables_of(scope);
    let extra = undeclared(&shell.current, scope, &variables);
    let rows: Vec<_> = variables
        .iter()
        .enumerate()
        .map(|(index, variable)| declared_row(shell, scope, index + 1, variable, cx))
        .collect();
    let extra_rows: Vec<_> = extra
        .iter()
        .enumerate()
        .map(|(index, (key, value))| undeclared_row(shell, scope, index + 1, key, value, cx))
        .collect();
    let add = add_row(scope, cx);
    region("variables-document")
        .flex_1()
        .min_w_0()
        .min_h_0()
        .flex()
        .flex_col()
        .child(
            text_role::role(text_region("variables-title"), text::BODY, cx)
                .aria_label(title.clone())
                .flex_none()
                .p(space::MD)
                .child(title),
        )
        .child(header(bolt::colors(cx)))
        .child(
            div()
                .flex_1()
                .min_h_0()
                .overflow_hidden()
                .flex()
                .flex_col()
                .children(rows)
                .child(add)
                .children(extra_rows),
        )
        .into_any_element()
}

fn header(colors: &Colors) -> Div {
    div()
        .flex_none()
        .flex()
        .items_center()
        .h(tokens::GRID_HEADER)
        .bg(colors.background)
        .font_family(family::MONO)
        .text_size(tokens::GRID_HEADER_TEXT)
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(colors.strong)
        .child(column(tokens::GRID_NUMBER_COLUMN))
        .child(column(tokens::GRID_KEY_COLUMN).child("key"))
        .child(column(tokens::VARIABLES_VALUE_COLUMN).child("value"))
        .child(column(tokens::VARIABLES_VALUE_COLUMN).child("current value"))
        .child(column(tokens::VARIABLES_SECRET_COLUMN).child("secret"))
        .child(fill().child("description"))
}

fn declared_row(
    shell: &Shell,
    scope: SecretScope,
    n: usize,
    variable: &Variable,
    cx: &mut Context<Shell>,
) -> AnyElement {
    let enable = enable_box(scope, n, variable, cx);
    let key = cell(shell, scope, Column::Key, n, tokens::GRID_KEY_COLUMN, cx)
        .child(variable.key.clone())
        .into_any_element();
    let value = if variable.secret {
        cell(
            shell,
            scope,
            Column::Value,
            n,
            tokens::VARIABLES_VALUE_COLUMN,
            cx,
        )
        .child(part(format!("variables-masked-{n}")).child(MASK))
        .into_any_element()
    } else {
        cell(
            shell,
            scope,
            Column::Value,
            n,
            tokens::VARIABLES_VALUE_COLUMN,
            cx,
        )
        .child(variable.value.clone().unwrap_or_default())
        .into_any_element()
    };
    let current = if variable.secret {
        let muted = bolt::colors(cx).muted;
        static_cell(
            Column::Current.element_id(n),
            tokens::VARIABLES_VALUE_COLUMN,
        )
        .text_color(muted)
        .font_family(family::INTERFACE)
        .child(part(format!("variables-same-{n}")).child("same as value"))
        .into_any_element()
    } else {
        let text = shell
            .current
            .get(scope, &variable.key)
            .map(str::to_owned)
            .unwrap_or_default();
        cell(
            shell,
            scope,
            Column::Current,
            n,
            tokens::VARIABLES_VALUE_COLUMN,
            cx,
        )
        .child(text)
        .into_any_element()
    };
    let secret = secret_toggle(scope, n, variable, cx);
    let description = cell(shell, scope, Column::Description, n, Pixels::ZERO, cx)
        .flex_1()
        .font_family(family::INTERFACE)
        .child(variable.description.clone().unwrap_or_default())
        .into_any_element();
    let delete = delete_control(scope, n, &variable.key, cx);
    let colors = bolt::colors(cx);
    let tone = if variable.enabled {
        colors.foreground
    } else {
        colors.line_2
    };
    row(format!("variables-row-{n}"), variable.key.clone(), colors)
        .text_color(tone)
        .child(enable)
        .child(number(n, colors))
        .child(key)
        .child(value)
        .child(current)
        .child(secret)
        .child(description)
        .child(delete)
        .into_any_element()
}

fn undeclared_row(
    shell: &Shell,
    scope: SecretScope,
    n: usize,
    key: &str,
    value: &str,
    cx: &mut Context<Shell>,
) -> AnyElement {
    let current = cell(
        shell,
        scope,
        Column::Undeclared,
        n,
        tokens::VARIABLES_VALUE_COLUMN,
        cx,
    )
    .child(value.to_owned())
    .into_any_element();
    let colors = bolt::colors(cx);
    row(format!("variables-undeclared-{n}"), key.to_owned(), colors)
        .text_color(colors.foreground)
        .child(
            div()
                .flex_none()
                .w(tokens::GRID_ENABLED_BOX)
                .ml(space::MD)
                .mr(space::SM),
        )
        .child(number(n, colors))
        .child(
            static_cell(
                format!("variables-undeclared-key-{n}"),
                tokens::GRID_KEY_COLUMN,
            )
            .text_color(colors.muted)
            .child(key.to_owned()),
        )
        .child(
            static_cell(
                format!("variables-undeclared-hint-{n}"),
                tokens::VARIABLES_VALUE_COLUMN,
            )
            .font_family(family::INTERFACE)
            .text_color(colors.muted)
            .child("not declared"),
        )
        .child(current)
        .into_any_element()
}

fn row(id: String, label: String, colors: &Colors) -> Stateful<Div> {
    labeled_row(id, label)
        .flex_none()
        .flex()
        .items_center()
        .h(tokens::GRID_ROW)
        .border_b(border::THIN)
        .border_color(colors.row_divider)
        .font_family(family::MONO)
        .text_size(tokens::GRID_TEXT)
}

fn number(n: usize, colors: &Colors) -> Div {
    div()
        .w(tokens::GRID_NUMBER_COLUMN)
        .flex_none()
        .pr(space::MD)
        .text_right()
        .text_color(colors.line_2)
        .child(n.to_string())
}

/// The cell `column` of the 1-based row `n`: the open field when it is under edit, else a
/// cell whose click opens it. A zero `width` leaves the width to the caller.
fn cell(
    shell: &Shell,
    scope: SecretScope,
    column: Column,
    n: usize,
    width: Pixels,
    cx: &mut Context<Shell>,
) -> Stateful<Div> {
    let owner = scope.path();
    let editing = shell
        .cell_edit
        .as_ref()
        .filter(|edit| edit.request_id == owner && edit.table == column.table() && edit.n == n);
    let base = static_cell(column.element_id(n), width);
    match editing {
        Some(edit) => {
            let foreground = bolt::colors(cx).foreground;
            base.on_action(cx.listener(|this, action: &Enter, window, cx| {
                if action.secondary {
                    this.send_active(window, cx);
                } else {
                    this.commit_cell_edit(window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &Escape, window, cx| {
                this.discard_cell_edit(window, cx);
            }))
            .child(
                Input::new(&edit.input)
                    .id("variables-cell-input")
                    .appearance(false)
                    .bordered(false)
                    .flex_1()
                    .min_w_0()
                    .text_size(tokens::GRID_TEXT)
                    .font_family(family::MONO)
                    .text_color(foreground),
            )
        }
        None => base
            .cursor_text()
            .on_click(cx.listener(move |this, _event, window, cx| {
                this.start_variable_edit(scope, column, n, window, cx);
            })),
    }
}

fn static_cell(id: String, width: Pixels) -> Stateful<Div> {
    let cell = part(id)
        .h_full()
        .flex()
        .items_center()
        .flex_none()
        .pl(space::MD)
        .pr(space::MD)
        .min_w_0()
        .overflow_hidden()
        .whitespace_nowrap();
    if width > Pixels::ZERO {
        cell.w(width)
    } else {
        cell
    }
}

/// The enable box `variables-enabled-<n>`: a cut box with a tint check when enabled.
fn enable_box(
    scope: SecretScope,
    n: usize,
    variable: &Variable,
    cx: &mut Context<Shell>,
) -> Stateful<Div> {
    let colors = bolt::colors(cx);
    let tint = bolt::brand(cx).accent_tint;
    part(format!("variables-enabled-{n}"))
        .role(Role::CheckBox)
        .aria_label(format!("Enable {}", variable.key))
        .aria_toggled(toggled(variable.enabled))
        .cursor_pointer()
        .flex_none()
        .ml(space::MD)
        .mr(space::SM)
        .w(tokens::GRID_ENABLED_BOX)
        .h(tokens::GRID_ENABLED_BOX)
        .flex()
        .items_center()
        .justify_center()
        .child(
            Chamfer::new(cut::KEYCAP)
                .fill(colors.raised)
                .edge(colors.line, border::THIN),
        )
        .when(variable.enabled, |this| {
            this.child(
                Icon::new(IconName::Check)
                    .size(tokens::BADGE_TEXT)
                    .color(tint),
            )
        })
        .on_click(cx.listener(move |this, _event, window, cx| {
            this.toggle_variable_enabled(scope, n, window, cx);
        }))
}

/// The secret toggle `variables-secret-<n>`: a lock, tinted while the variable is secret.
fn secret_toggle(
    scope: SecretScope,
    n: usize,
    variable: &Variable,
    cx: &mut Context<Shell>,
) -> Stateful<Div> {
    let color = if variable.secret {
        bolt::brand(cx).accent_tint
    } else {
        bolt::colors(cx).line_2
    };
    part(format!("variables-secret-{n}"))
        .role(Role::CheckBox)
        .aria_label(format!("Secret {}", variable.key))
        .aria_toggled(toggled(variable.secret))
        .cursor_pointer()
        .flex_none()
        .w(tokens::VARIABLES_SECRET_COLUMN)
        .h_full()
        .flex()
        .items_center()
        .pl(space::MD)
        .child(
            Icon::new(IconName::Lock)
                .size(tokens::GRID_TEXT)
                .color(color),
        )
        .on_click(cx.listener(move |this, _event, window, cx| {
            this.toggle_variable_secret(scope, n, window, cx);
        }))
}

fn delete_control(
    scope: SecretScope,
    n: usize,
    key: &str,
    cx: &mut Context<Shell>,
) -> Stateful<Div> {
    let muted = bolt::colors(cx).muted;
    part(format!("variables-delete-{n}"))
        .role(Role::Button)
        .aria_label(format!("Delete {key}"))
        .cursor_pointer()
        .flex_none()
        .pl(space::MD)
        .pr(space::MD)
        .child(
            Icon::new(IconName::Close)
                .size(tokens::GRID_TEXT)
                .color(muted),
        )
        .on_click(cx.listener(move |this, _event, window, cx| {
            this.delete_variable(scope, n, window, cx);
        }))
}

/// The `Add` row `variables-add`.
fn add_row(scope: SecretScope, cx: &mut Context<Shell>) -> Stateful<Div> {
    let muted = bolt::colors(cx).muted;
    part("variables-add".to_owned())
        .role(Role::Button)
        .aria_label("Add a variable")
        .cursor_pointer()
        .flex_none()
        .flex()
        .items_center()
        .gap(space::SM)
        .h(tokens::GRID_ROW)
        .pl(space::MD)
        .text_size(tokens::GRID_TEXT)
        .text_color(muted)
        .child(
            Icon::new(IconName::Plus)
                .size(tokens::GRID_TEXT)
                .color(muted),
        )
        .child("Add")
        .on_click(cx.listener(move |this, _event, window, cx| {
            this.add_variable(scope, window, cx);
        }))
}

fn toggled(on: bool) -> Toggled {
    if on { Toggled::True } else { Toggled::False }
}

fn column(width: Pixels) -> Div {
    div()
        .w(width)
        .flex_none()
        .pl(space::MD)
        .pr(space::MD)
        .overflow_hidden()
        .whitespace_nowrap()
}

fn fill() -> Div {
    div()
        .flex_1()
        .min_w_0()
        .pl(space::MD)
        .pr(space::MD)
        .overflow_hidden()
        .whitespace_nowrap()
}
