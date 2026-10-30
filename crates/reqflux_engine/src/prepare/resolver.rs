//! Variable resolution that collects unresolved names across a whole request.

use reqflux_core::model::KeyValue;
use reqflux_core::template::{DynamicSource, Scopes, Segment, resolve, segments};

use crate::sent::Secrets;

use super::enabled;

/// Resolves texts against fixed scopes and remembers every unresolved name once, in
/// first-occurrence order.
pub(super) struct Resolver<'a, D> {
    scopes: Scopes,
    dynamic: &'a mut D,
    unresolved: Vec<String>,
    referenced: Vec<String>,
}

impl<'a, D: DynamicSource> Resolver<'a, D> {
    pub(super) fn new(scopes: Scopes, dynamic: &'a mut D) -> Self {
        Self {
            scopes,
            dynamic,
            unresolved: Vec::new(),
            referenced: Vec::new(),
        }
    }

    /// Resolves `text`; unresolved references stay literal.
    pub(super) fn text(&mut self, text: &str) -> String {
        for segment in segments(text) {
            if let Segment::Reference { name, .. } = segment
                && !self.referenced.iter().any(|known| known == name)
            {
                self.referenced.push(name.to_owned());
            }
        }
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

    /// Adds to `secrets` the value each referenced name in `secret_names` resolved to,
    /// the value of the scope that won, so a local or script override is the one masked.
    pub(super) fn add_secret_values(&self, secret_names: &[&str], secrets: &mut Secrets) {
        for name in &self.referenced {
            if secret_names.contains(&name.as_str())
                && let Some(value) = self.scopes.lookup(name)
            {
                secrets.add(value);
            }
        }
    }

    pub(super) fn into_unresolved(self) -> Vec<String> {
        self.unresolved
    }
}
