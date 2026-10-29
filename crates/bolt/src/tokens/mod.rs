//! Design tokens: the only place in Bolt that holds raw colors and sizes.
//!
//! Screens read these through named roles. The guardrail test in `tests/guardrail.rs`
//! rejects color literals and bare `px(` calls anywhere else in the crate.

mod colors;
mod metrics;
mod typography;

pub use colors::{Colors, accent_wash, hex, hex_alpha};
pub use metrics::{border, chrome, cut, icon, motion, space};
pub use typography::{BASE_SIZE, TextRole, TextTone, family, text};
