//! The request body (S8).

use std::path::Path;

use reqflux_core::model::{Body, FormPart, RawLanguage};
use reqflux_core::template::DynamicSource;
use reqflux_http::{MultipartPart, MultipartValue, PreparedBody};

use super::has_header;
use super::resolver::Resolver;
use crate::error::ExecuteError;

const JSON: &str = "application/json";

/// Builds the body. File paths are resolved and joined to `root`, so relative paths are
/// relative to the workspace root and absolute ones are kept.
pub(super) fn prepare<D: DynamicSource>(
    body: &Body,
    root: &Path,
    headers: &[(String, String)],
    resolver: &mut Resolver<'_, D>,
) -> Result<PreparedBody, ExecuteError> {
    // A content type set by an enabled header replaces the default of the body mode.
    let default_type = |content_type: &str| {
        (!has_header(headers, "Content-Type")).then(|| content_type.to_owned())
    };
    Ok(match body {
        Body::None => PreparedBody::None,
        Body::Raw { language, text } => PreparedBody::Bytes {
            content_type: default_type(content_type(*language)),
            data: resolver.text(text).into_bytes(),
        },
        Body::Graphql { query, variables } => PreparedBody::Bytes {
            content_type: default_type(JSON),
            data: graphql(&resolver.text(query), &resolver.text(variables))?,
        },
        Body::Urlencoded { fields } => PreparedBody::Form(resolver.pairs(fields)),
        Body::FormData { parts } => PreparedBody::Multipart(
            parts
                .iter()
                .filter_map(|part| match part {
                    FormPart::Text {
                        key,
                        value,
                        enabled: true,
                        ..
                    } => Some(MultipartPart {
                        name: resolver.text(key),
                        value: MultipartValue::Text(resolver.text(value)),
                    }),
                    FormPart::File {
                        key,
                        path,
                        enabled: true,
                        ..
                    } => Some(MultipartPart {
                        name: resolver.text(key),
                        value: MultipartValue::File(root.join(resolver.text(path))),
                    }),
                    FormPart::Text { .. } | FormPart::File { .. } => None,
                })
                .collect(),
        ),
        Body::Binary { path } => PreparedBody::File(root.join(resolver.text(path))),
    })
}

fn content_type(language: RawLanguage) -> &'static str {
    match language {
        RawLanguage::Json => JSON,
        RawLanguage::Xml => "application/xml",
        RawLanguage::Text => "text/plain",
        RawLanguage::Html => "text/html",
        RawLanguage::Javascript => "application/javascript",
    }
}

/// `{"query": <query>, "variables": <variables>}`; blank variables send `null`.
fn graphql(query: &str, variables: &str) -> Result<Vec<u8>, ExecuteError> {
    let variables = if variables.trim().is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_str(variables)
            .map_err(|error| ExecuteError::GraphqlVariables(error.to_string()))?
    };
    let body = serde_json::json!({ "query": query, "variables": variables });
    Ok(body.to_string().into_bytes())
}
