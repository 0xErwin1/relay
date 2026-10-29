//! Bolt, the design system shared by Relay and DBFlux.
//!
//! Bolt owns everything that does not depend on the product: color roles and washes,
//! typography roles, spacing, cut corners, chrome metrics, the theme mapping onto
//! gpui-component, and generic components. A product supplies only its brand (accent,
//! name, logo) and any extra tokens of its own.
//!
//! This crate must never name a product, a protocol or a product type, so it can move to
//! its own repository without changes. See "Design system" in ARCHITECTURE.md.

pub mod tokens;

#[cfg(test)]
mod test_support;
