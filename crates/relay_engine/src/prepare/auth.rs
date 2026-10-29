//! The effective auth and how it is sent (S7).

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use relay_core::model::{ApiKeyLocation, Auth};
use relay_core::template::DynamicSource;

use super::has_header;
use super::resolver::Resolver;
use crate::error::ExecuteError;
use crate::select::Target;

const AUTHORIZATION: &str = "Authorization";

/// The auth of the request, or of the nearest folder, then the collection, that does not
/// inherit. A collection that inherits means no auth.
pub(super) fn effective<'a>(target: &Target<'a>) -> &'a Auth {
    std::iter::once(&target.request.request.auth)
        .chain(
            target
                .folders
                .iter()
                .rev()
                .map(|folder| &folder.folder.auth),
        )
        .chain(std::iter::once(&target.collection.collection.auth))
        .find(|auth| !auth.is_inherit())
        .unwrap_or(&Auth::None)
}

/// Adds `auth` to `headers` or `query`. An enabled request header with the same name wins,
/// and then the auth fields are not resolved at all.
pub(super) fn apply<D: DynamicSource>(
    auth: &Auth,
    resolver: &mut Resolver<'_, D>,
    headers: &mut Vec<(String, String)>,
    query: &mut Vec<(String, String)>,
) -> Result<(), ExecuteError> {
    match auth {
        Auth::Inherit | Auth::None => {}
        Auth::Basic { username, password } => {
            if !has_header(headers, AUTHORIZATION) {
                let credentials =
                    format!("{}:{}", resolver.text(username), resolver.text(password));
                headers.push((
                    AUTHORIZATION.to_owned(),
                    format!("Basic {}", STANDARD.encode(credentials)),
                ));
            }
        }
        Auth::Bearer { token } => {
            if !has_header(headers, AUTHORIZATION) {
                headers.push((
                    AUTHORIZATION.to_owned(),
                    format!("Bearer {}", resolver.text(token)),
                ));
            }
        }
        Auth::ApiKey {
            key,
            value,
            location,
        } => {
            let key = resolver.text(key);
            match location {
                ApiKeyLocation::Header if has_header(headers, &key) => {}
                ApiKeyLocation::Header => headers.push((key, resolver.text(value))),
                ApiKeyLocation::Query => query.push((key, resolver.text(value))),
            }
        }
        Auth::Other { kind, .. } => return Err(ExecuteError::UnsupportedAuth(kind.clone())),
    }
    Ok(())
}
