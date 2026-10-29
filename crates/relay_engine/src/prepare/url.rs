//! The query string (S6).

/// Appends `query` to `base`, percent-encoded, after `&` when `base` already has a `?`.
pub(super) fn with_query(base: String, query: &[(String, String)]) -> String {
    if query.is_empty() {
        return base;
    }
    let encoded: Vec<String> = query
        .iter()
        .map(|(key, value)| format!("{}={}", encode(key), encode(value)))
        .collect();
    let separator = if base.contains('?') { '&' } else { '?' };
    format!("{base}{separator}{}", encoded.join("&"))
}

/// Percent-encodes every byte except the RFC 3986 unreserved characters.
fn encode(text: &str) -> String {
    let mut encoded = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}
