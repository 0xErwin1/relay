//! The JavaScript libraries the sandbox serves through `require`.
//!
//! They are vendored under `assets/`, unchanged, with their versions and licences recorded in
//! `assets/LICENSES.md`. A module is compiled the first time a script requires it, so a script
//! that needs none pays nothing.

/// The source of the module `name`, when Relay bundles it.
pub(super) fn source(name: &str) -> Option<&'static str> {
    match name {
        "lodash" => Some(include_str!("../assets/lodash.min.js")),
        "moment" => Some(include_str!("../assets/moment.min.js")),
        "chai" => Some(include_str!("../assets/chai.js")),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bundled_modules_are_served_and_nothing_else() {
        for name in ["lodash", "moment", "chai"] {
            let source = source(name).unwrap_or_else(|| panic!("{name} is bundled"));
            assert!(source.len() > 10_000, "{name} carries its library");
        }
        assert!(source("jquery").is_none());
        assert!(source("").is_none());
    }
}
