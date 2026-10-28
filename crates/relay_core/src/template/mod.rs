//! `{{name}}` templating and variable scopes.
//!
//! [`resolve`] replaces every `{{name}}` reference in a text with the value of `name` from
//! [`Scopes`], or with a fresh dynamic value such as `{{$guid}}` from a [`DynamicSource`].
//! [`referenced_names`] lists the references without resolving them.

mod dynamic;
mod parser;
mod resolve;
mod scope;

pub use dynamic::{DynamicSource, DynamicVariable, SequentialDynamicSource, SystemDynamicSource};
pub use resolve::{Resolved, referenced_names, resolve};
pub use scope::{Scope, ScopeEntry, ScopeKind, Scopes};

#[cfg(test)]
mod tests;
