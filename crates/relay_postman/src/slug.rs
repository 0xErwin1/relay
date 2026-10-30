//! Name slugs: lowercase, kebab, ASCII (S2), unique under a parent (S2) and against the
//! slugs an import already used (S6).

use std::collections::BTreeSet;

/// Slugs a display name: lowercase ASCII alphanumerics, every other run of characters a
/// single `-` (S2). Never empty: a name without alphanumerics becomes `item`.
#[must_use]
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        "item".to_owned()
    } else {
        trimmed.to_owned()
    }
}

/// Returns `base`, or `base-2`, `base-3`, ..., the first slug not in `used`, and records
/// the result (S2). Callers keep one set per parent.
pub fn unique(base: &str, used: &mut BTreeSet<String>) -> String {
    let mut candidate = base.to_owned();
    let mut n = 1;
    while used.contains(&candidate) {
        n += 1;
        candidate = format!("{base}-{n}");
    }
    used.insert(candidate.clone());
    candidate
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_is_lowercase_kebab_ascii() {
        assert_eq!(slug("Auth Requests"), "auth-requests");
        assert_eq!(slug("API_v2 !!Endpoints"), "api-v2-endpoints");
        assert_eq!(slug("  --Update User--  "), "update-user");
    }

    #[test]
    fn slug_falls_back_for_names_without_ascii_letters() {
        assert_eq!(slug("Ünïcode"), "n-code");
        assert_eq!(slug("!!!"), "item");
        assert_eq!(slug(""), "item");
    }

    #[test]
    fn unique_appends_the_first_free_suffix_in_document_order() {
        let mut used = BTreeSet::new();
        assert_eq!(unique("get", &mut used), "get");
        assert_eq!(unique("get", &mut used), "get-2");
        assert_eq!(unique("get", &mut used), "get-3");
        assert_eq!(unique("get-2", &mut used), "get-2-2");
    }
}
