//! Bolt, the design system shared by Relay and DBFlux.
//!
//! Bolt owns everything that does not depend on the product: color roles and washes,
//! typography roles, spacing, cut corners, chrome metrics, the theme mapping onto
//! gpui-component, and generic components. A product supplies only its brand (accent,
//! name, logo) and any extra tokens of its own.
//!
//! This crate must never name a product, a protocol or a product type, so it can move to
//! its own repository without changes. See "Design system" in ARCHITECTURE.md.
//!
//! A product calls [`install`] with its [`Brand`] before opening a window, then reads
//! roles through [`colors`], [`brand`], [`accent_washes`] and the constants in [`tokens`].

mod brand;
pub mod chamfer;
pub mod fonts;
mod theme;
pub mod tokens;

#[cfg(test)]
mod test_support;

pub use brand::{AccentWashes, Brand};
pub use theme::{accent_washes, brand, colors, install};
