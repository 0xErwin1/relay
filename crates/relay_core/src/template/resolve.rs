//! Substitution of references.

use super::parser::{Segment, segments};
use super::{DynamicSource, DynamicVariable, Scopes};

/// The result of [`resolve`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    /// The text with every resolvable reference substituted; unresolved references stay
    /// literal, exactly as written.
    pub text: String,
    /// Names that did not resolve, unique, in first-occurrence order.
    pub unresolved: Vec<String>,
}

/// Resolves every `{{name}}` reference in `text`.
///
/// A name resolves against `scopes` first, highest precedence first, then as a dynamic
/// variable from `dynamic`, so an enabled scope entry named `$guid` shadows the dynamic one.
/// Each dynamic occurrence asks `dynamic` for a fresh value. Resolution is a single pass:
/// a substituted value is never scanned for references.
pub fn resolve(text: &str, scopes: &Scopes, dynamic: &mut impl DynamicSource) -> Resolved {
    let mut resolved = String::with_capacity(text.len());
    let mut unresolved = Vec::new();
    for segment in segments(text) {
        match segment {
            Segment::Literal(literal) => resolved.push_str(literal),
            Segment::Reference { raw, name } => {
                if let Some(value) = scopes.lookup(name) {
                    resolved.push_str(value);
                } else if let Some(variable) = DynamicVariable::parse(name) {
                    resolved.push_str(&dynamic.generate(variable));
                } else {
                    resolved.push_str(raw);
                    push_unique(&mut unresolved, name);
                }
            }
        }
    }
    Resolved {
        text: resolved,
        unresolved,
    }
}

/// Lists the names referenced in `text`, unique, in first-occurrence order, without
/// resolving them.
pub fn referenced_names(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    for segment in segments(text) {
        if let Segment::Reference { name, .. } = segment {
            push_unique(&mut names, name);
        }
    }
    names
}

fn push_unique(names: &mut Vec<String>, name: &str) {
    if !names.iter().any(|known| known == name) {
        names.push(name.to_owned());
    }
}
