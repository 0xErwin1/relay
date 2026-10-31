//! The `Set-Cookie` headers of a response parsed for the Cookies sub-tab, without GPUI. The
//! parse is read-only: no cookie is stored (the cookie jar is v1).

/// One `Set-Cookie` header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Cookie {
    Parsed(SetCookie),
    /// A header that does not parse, as received.
    Raw(String),
}

/// A parsed `Set-Cookie` header; attribute values are as received.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct SetCookie {
    pub(crate) name: String,
    /// The value, without surrounding double quotes.
    pub(crate) value: String,
    pub(crate) domain: Option<String>,
    pub(crate) path: Option<String>,
    pub(crate) expires: Option<String>,
    pub(crate) max_age: Option<String>,
    pub(crate) secure: bool,
    pub(crate) http_only: bool,
    pub(crate) same_site: Option<String>,
}

impl SetCookie {
    /// The attributes as one line: `Domain=… · Path=/ · Max-Age=60 · Secure · HttpOnly ·
    /// SameSite=Lax`, in that order, absent ones left out.
    pub(crate) fn attributes(&self) -> String {
        let named = [
            ("Domain", &self.domain),
            ("Path", &self.path),
            ("Expires", &self.expires),
            ("Max-Age", &self.max_age),
        ];
        let mut parts: Vec<String> = named
            .iter()
            .filter_map(|(name, value)| value.as_ref().map(|value| format!("{name}={value}")))
            .collect();
        if self.secure {
            parts.push("Secure".to_owned());
        }
        if self.http_only {
            parts.push("HttpOnly".to_owned());
        }
        if let Some(same_site) = &self.same_site {
            parts.push(format!("SameSite={same_site}"));
        }
        parts.join(" · ")
    }
}

/// How many `Set-Cookie` headers `headers` has, the count on the Cookies sub-tab.
pub(crate) fn cookie_count(headers: &[(String, String)]) -> usize {
    headers
        .iter()
        .filter(|(name, _)| name.eq_ignore_ascii_case("set-cookie"))
        .count()
}

/// Every `Set-Cookie` header of `headers`, matched case-insensitively, in received order.
pub(crate) fn cookies(headers: &[(String, String)]) -> Vec<Cookie> {
    headers
        .iter()
        .filter(|(name, _)| name.eq_ignore_ascii_case("set-cookie"))
        .map(|(_, value)| match parse(value) {
            Some(cookie) => Cookie::Parsed(cookie),
            None => Cookie::Raw(value.clone()),
        })
        .collect()
}

/// `header` as RFC 6265 reads it: `name=value` then `;`-separated attributes, names
/// matched case-insensitively and unknown ones ignored. `None` when the first pair has no
/// `=` or an empty name.
pub(crate) fn parse(header: &str) -> Option<SetCookie> {
    let mut parts = header.split(';');
    let (name, value) = parts.next()?.split_once('=')?;
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    let value = value.trim();
    let value = value
        .strip_prefix('"')
        .and_then(|inner| inner.strip_suffix('"'))
        .unwrap_or(value);
    let mut cookie = SetCookie {
        name: name.to_owned(),
        value: value.to_owned(),
        ..SetCookie::default()
    };
    for attribute in parts {
        let (key, value) = match attribute.split_once('=') {
            Some((key, value)) => (key.trim(), Some(value.trim().to_owned())),
            None => (attribute.trim(), None),
        };
        match key.to_ascii_lowercase().as_str() {
            "domain" => cookie.domain = value,
            "path" => cookie.path = value,
            "expires" => cookie.expires = value,
            "max-age" => cookie.max_age = value,
            "secure" => cookie.secure = true,
            "httponly" => cookie.http_only = true,
            "samesite" => cookie.same_site = value,
            _ => {}
        }
    }
    Some(cookie)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_attribute_parses() {
        let cookie = parse(
            "sid=abc123; Domain=example.com; Path=/api; Expires=Wed, 21 Oct 2026 07:28:00 GMT; \
             Max-Age=3600; Secure; HttpOnly; SameSite=Strict",
        );
        assert_eq!(
            cookie,
            Some(SetCookie {
                name: "sid".to_owned(),
                value: "abc123".to_owned(),
                domain: Some("example.com".to_owned()),
                path: Some("/api".to_owned()),
                expires: Some("Wed, 21 Oct 2026 07:28:00 GMT".to_owned()),
                max_age: Some("3600".to_owned()),
                secure: true,
                http_only: true,
                same_site: Some("Strict".to_owned()),
            })
        );
        assert_eq!(
            cookie.map(|cookie| cookie.attributes()).as_deref(),
            Some(
                "Domain=example.com · Path=/api · Expires=Wed, 21 Oct 2026 07:28:00 GMT · \
                 Max-Age=3600 · Secure · HttpOnly · SameSite=Strict"
            )
        );
    }

    #[test]
    fn attribute_names_are_case_insensitive_and_values_unquoted() {
        let cookie = parse("theme=\"dark mode\"; secure; HTTPONLY; samesite=lax; PATH=/");
        assert_eq!(
            cookie,
            Some(SetCookie {
                name: "theme".to_owned(),
                value: "dark mode".to_owned(),
                path: Some("/".to_owned()),
                secure: true,
                http_only: true,
                same_site: Some("lax".to_owned()),
                ..SetCookie::default()
            })
        );
        let empty = parse("gone=; Max-Age=0");
        assert_eq!(empty.as_ref().map(|cookie| cookie.value.as_str()), Some(""));
        assert_eq!(
            empty.map(|cookie| cookie.attributes()).as_deref(),
            Some("Max-Age=0")
        );
    }

    #[test]
    fn a_malformed_header_stays_raw() {
        assert_eq!(parse("no equals sign; Path=/"), None);
        assert_eq!(parse("=value"), None);
        let headers = vec![
            ("Set-Cookie".to_owned(), "a=1".to_owned()),
            ("content-type".to_owned(), "text/plain".to_owned()),
            ("set-cookie".to_owned(), "broken".to_owned()),
        ];
        assert_eq!(
            cookies(&headers),
            vec![
                Cookie::Parsed(SetCookie {
                    name: "a".to_owned(),
                    value: "1".to_owned(),
                    ..SetCookie::default()
                }),
                Cookie::Raw("broken".to_owned()),
            ]
        );
    }
}
