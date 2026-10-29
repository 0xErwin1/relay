//! Variable resolution that collects unresolved names across a whole request.

use relay_core::model::KeyValue;
use relay_core::template::{DynamicSource, Scopes, resolve};

use super::enabled;

/// Resolves texts against fixed scopes and remembers every unresolved name once, in
/// first-occurrence order.
pub(super) struct Resolver<'a, D> {
    scopes: Scopes,
    dynamic: &'a mut D,
    unresolved: Vec<String>,
}

impl<'a, D: DynamicSource> Resolver<'a, D> {
    pub(super) fn new(scopes: Scopes, dynamic: &'a mut D) -> Self {
        Self {
            scopes,
            dynamic,
            unresolved: Vec::new(),
        }
    }

    /// Resolves `text`; unresolved references stay literal.
    pub(super) fn text(&mut self, text: &str) -> String {
        let resolved = resolve(text, &self.scopes, self.dynamic);
        for name in resolved.unresolved {
            if !self.unresolved.contains(&name) {
                self.unresolved.push(name);
            }
        }
        resolved.text
    }

    /// Resolves the key and value of every enabled row, in order.
    pub(super) fn pairs(&mut self, rows: &[KeyValue]) -> Vec<(String, String)> {
        enabled(rows)
            .map(|row| (self.text(&row.key), self.text(&row.value)))
            .collect()
    }

    pub(super) fn into_unresolved(self) -> Vec<String> {
        self.unresolved
    }
}
