//! The Body, Auth, Scripts and Settings panes without GPUI: the body mode, the auth type and
//! its fields with their masking, the settings text and the lines of a code block.

use reqflux_core::model::{ApiKeyLocation, Auth, Body, RawLanguage, Settings};
use reqflux_engine::DEFAULT_TIMEOUT_MS;

/// The body mode the pane header shows.
pub(crate) fn body_mode(body: &Body) -> String {
    match body {
        Body::None => "none".to_owned(),
        Body::Raw { language, .. } => format!("raw · {}", language_label(*language)),
        Body::Urlencoded { .. } => "urlencoded".to_owned(),
        Body::FormData { .. } => "form-data".to_owned(),
        Body::Binary { .. } => "binary".to_owned(),
        Body::Graphql { .. } => "GraphQL".to_owned(),
    }
}

fn language_label(language: RawLanguage) -> &'static str {
    match language {
        RawLanguage::Json => "JSON",
        RawLanguage::Xml => "XML",
        RawLanguage::Text => "Text",
        RawLanguage::Html => "HTML",
        RawLanguage::Javascript => "JavaScript",
    }
}

/// The auth type the pane header shows.
pub(crate) fn auth_type(auth: &Auth) -> &str {
    match auth {
        Auth::Inherit => "Inherit from parent",
        Auth::None => "No auth",
        Auth::Basic { .. } => "Basic",
        Auth::Bearer { .. } => "Bearer token",
        Auth::ApiKey { .. } => "API key",
        Auth::Other { kind, .. } => kind,
    }
}

/// An auth field row. A `masked` value is never shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AuthField {
    pub(crate) label: String,
    pub(crate) value: String,
    pub(crate) masked: bool,
}

impl AuthField {
    /// The automation id, `request-auth-field-<lowercase label>` with dashes for spaces.
    pub(crate) fn element_id(&self) -> String {
        format!(
            "request-auth-field-{}",
            self.label.to_lowercase().replace(' ', "-")
        )
    }
}

/// The fields of `auth`. Passwords, tokens, API key values and every field of other types are
/// always masked; other values when they reference a secret, as `references_secret` (the
/// engine's) decides.
pub(crate) fn auth_fields(auth: &Auth, references_secret: impl Fn(&str) -> bool) -> Vec<AuthField> {
    let field = |label: &str, value: &str, always: bool| AuthField {
        label: label.to_owned(),
        value: value.to_owned(),
        masked: always || references_secret(value),
    };
    match auth {
        Auth::Inherit | Auth::None => Vec::new(),
        Auth::Basic { username, password } => vec![
            field("Username", username, false),
            field("Password", password, true),
        ],
        Auth::Bearer { token } => vec![field("Token", token, true)],
        Auth::ApiKey {
            key,
            value,
            location,
        } => vec![
            field("Key", key, false),
            field("Value", value, true),
            field(
                "Add to",
                match location {
                    ApiKeyLocation::Header => "header",
                    ApiKeyLocation::Query => "query",
                },
                false,
            ),
        ],
        // The engine does not know which fields of other types hold credentials (an OAuth 2
        // client secret, a token), so every value is masked.
        Auth::Other { fields, .. } => fields
            .iter()
            .map(|(key, value)| match value.as_str() {
                Some(text) => field(key, text, true),
                None => field(key, &value.to_string(), true),
            })
            .collect(),
    }
}

/// The automation id and the text of each settings row, top to bottom.
pub(crate) fn settings_rows(settings: &Settings) -> [(&'static str, &'static str, String); 3] {
    let flag = |value: Option<bool>| match value {
        Some(true) => "on".to_owned(),
        Some(false) => "off".to_owned(),
        None => "Default (on)".to_owned(),
    };
    let timeout = match settings.timeout_ms {
        Some(ms) => format!("{ms} ms"),
        None => format!("Default ({DEFAULT_TIMEOUT_MS} ms)"),
    };
    [
        ("request-setting-timeout", "Timeout", timeout),
        (
            "request-setting-redirects",
            "Follow redirects",
            flag(settings.follow_redirects),
        ),
        (
            "request-setting-tls",
            "Verify TLS",
            flag(settings.verify_tls),
        ),
    ]
}

/// The lines of a code block, numbered from 1. An empty text is one empty line.
pub(crate) fn numbered_lines(text: &str) -> Vec<(usize, &str)> {
    let mut lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        lines.push("");
    }
    lines
        .into_iter()
        .enumerate()
        .map(|(index, line)| (index + 1, line))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secret(text: &str) -> bool {
        text.contains("{{token}}")
    }

    fn masks(fields: &[AuthField]) -> Vec<(&str, bool)> {
        fields
            .iter()
            .map(|field| (field.label.as_str(), field.masked))
            .collect()
    }

    #[test]
    fn body_modes_name_the_raw_language() {
        let raw = Body::Raw {
            language: RawLanguage::Json,
            text: String::new(),
        };
        assert_eq!(body_mode(&raw), "raw · JSON");
        assert_eq!(body_mode(&Body::None), "none");
        let graphql = Body::Graphql {
            query: String::new(),
            variables: String::new(),
        };
        assert_eq!(body_mode(&graphql), "GraphQL");
    }

    #[test]
    fn plain_tokens_and_passwords_are_masked() {
        let bearer = Auth::Bearer {
            token: "plain".to_owned(),
        };
        assert_eq!(masks(&auth_fields(&bearer, secret)), [("Token", true)]);
        let basic = Auth::Basic {
            username: "me".to_owned(),
            password: "plain".to_owned(),
        };
        assert_eq!(
            masks(&auth_fields(&basic, secret)),
            [("Username", false), ("Password", true)]
        );
        assert_eq!(auth_type(&basic), "Basic");
    }

    #[test]
    fn an_api_key_value_referencing_a_secret_is_masked() {
        let auth = Auth::ApiKey {
            key: "X-Key".to_owned(),
            value: "{{token}}".to_owned(),
            location: ApiKeyLocation::Query,
        };
        let fields = auth_fields(&auth, secret);
        assert_eq!(
            masks(&fields),
            [("Key", false), ("Value", true), ("Add to", false)]
        );
        let ids: Vec<_> = fields.iter().map(AuthField::element_id).collect();
        assert_eq!(
            ids.last().map(String::as_str),
            Some("request-auth-field-add-to")
        );
    }

    #[test]
    fn every_field_of_another_auth_type_is_masked() {
        let mut fields = toml::Table::new();
        fields.insert("client_secret".to_owned(), "plain".into());
        fields.insert("expires_in".to_owned(), 3600.into());
        let other = Auth::Other {
            kind: "oauth2".to_owned(),
            fields,
        };
        let masked: Vec<_> = auth_fields(&other, |_| false)
            .into_iter()
            .map(|field| (field.label, field.masked))
            .collect();
        assert_eq!(
            masked,
            [
                ("client_secret".to_owned(), true),
                ("expires_in".to_owned(), true)
            ]
        );
    }

    #[test]
    fn a_plain_api_key_value_is_masked() {
        let auth = Auth::ApiKey {
            key: "X-Key".to_owned(),
            value: "plain-key".to_owned(),
            location: ApiKeyLocation::Header,
        };
        assert_eq!(
            masks(&auth_fields(&auth, |_| false)),
            [("Key", false), ("Value", true), ("Add to", false)]
        );
    }

    #[test]
    fn settings_show_defaults() {
        let texts: Vec<_> = settings_rows(&Settings::default())
            .into_iter()
            .map(|(_, _, text)| text)
            .collect();
        assert_eq!(
            texts,
            ["Default (30000 ms)", "Default (on)", "Default (on)"]
        );
        let set = Settings {
            timeout_ms: Some(5000),
            follow_redirects: Some(false),
            verify_tls: None,
        };
        assert_eq!(settings_rows(&set)[0].2, "5000 ms");
    }

    #[test]
    fn lines_are_numbered_from_one() {
        assert_eq!(numbered_lines("a\nb"), [(1, "a"), (2, "b")]);
        assert_eq!(numbered_lines(""), [(1, "")]);
    }
}
