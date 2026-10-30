//! Adding and deleting rows of the Params, Headers and body grids. Adding appends an empty
//! enabled row to the draft and opens its key cell; deleting removes the row. An open cell
//! field is committed first, so what was typed lands in its own row before rows shift.

use gpui::{Context, Window};
use reqflux_core::model::{Body, FormPart, KeyValue, Request};

use super::Shell;
use super::cell_edit::CellField;

impl Shell {
    /// Appends an empty enabled row to `table` (`params`, `headers` or `body`) of the draft
    /// of the request `id` and opens the new row's key cell. A form-data body gets a text
    /// part; a body that is neither urlencoded nor form-data is left alone.
    pub(super) fn add_grid_row(
        &mut self,
        id: &str,
        table: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.commit_cell_edit(window, cx);
        let mut added = None;
        self.edit_draft(id, |request| added = push_row(request, table), cx);
        if let Some(n) = added {
            self.start_cell_edit(id, table, n, CellField::Key, window, cx);
        }
    }

    /// Removes the 1-based row `n` of `table` from the draft of the request `id`. An open
    /// cell field is committed and closed first, so it never points at a shifted row.
    pub(super) fn delete_grid_row(
        &mut self,
        id: &str,
        table: &str,
        n: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.commit_cell_edit(window, cx);
        self.edit_draft(id, |request| remove_row(request, table, n), cx);
    }
}

/// Appends an empty enabled row to `table` of `request`, returning its 1-based number.
fn push_row(request: &mut Request, table: &str) -> Option<usize> {
    let rows = match table {
        "params" => &mut request.params,
        "headers" => &mut request.headers,
        "body" => match &mut request.body {
            Body::Urlencoded { fields } => fields,
            Body::FormData { parts } => {
                parts.push(FormPart::Text {
                    key: String::new(),
                    value: String::new(),
                    description: None,
                    enabled: true,
                });
                return Some(parts.len());
            }
            _ => return None,
        },
        _ => return None,
    };
    rows.push(KeyValue::new("", ""));
    Some(rows.len())
}

/// Removes the 1-based row `n` of `table` of `request`, if it exists.
fn remove_row(request: &mut Request, table: &str, n: usize) {
    let Some(index) = n.checked_sub(1) else {
        return;
    };
    match (table, &mut request.body) {
        ("params", _) => remove_at(&mut request.params, index),
        ("headers", _) => remove_at(&mut request.headers, index),
        ("body", Body::Urlencoded { fields }) => remove_at(fields, index),
        ("body", Body::FormData { parts }) => remove_at(parts, index),
        _ => {}
    }
}

fn remove_at<T>(rows: &mut Vec<T>, index: usize) {
    if index < rows.len() {
        rows.remove(index);
    }
}
