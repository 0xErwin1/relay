//! The effective auth and how it is sent (S7).

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use reqflux_core::model::{ApiKeyLocation, Auth, Request};
use reqflux_core::template::DynamicSource;

use super::has_header;
use super::resolver::Resolver;
use crate::error::ExecuteError;
use crate::select::Target;
use crate::sent::Secrets;

const AUTHORIZATION: &str = "Authorization";

/// The auth of the request, or of the nearest folder, then the collection, that does not
/// inherit. A collection that inherits means no auth. `request` is the document being
/// prepared, so a draft carries its own auth in front of the same walk.
pub(super) fn effective<'a>(request: &'a Request, target: &Target<'a>) -> &'a Auth {
    std::iter::once(&request.auth)
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
/// and then the auth fields are not resolved at all. The resolved password, token or key
/// value, and the encoded basic credentials, go to `secrets` so history masks them.
pub(super) fn apply<D: DynamicSource>(
    auth: &Auth,
    resolver: &mut Resolver<'_, D>,
    headers: &mut Vec<(String, String)>,
    query: &mut Vec<(String, String)>,
    secrets: &mut Secrets,
) -> Result<(), ExecuteError> {
    match auth {
        Auth::Inherit | Auth::None => {}
        Auth::Basic { username, password } => {
            if !has_header(headers, AUTHORIZATION) {
                let password = resolver.text(password);
                let credentials = format!("{}:{password}", resolver.text(username));
                let encoded = STANDARD.encode(credentials);
                secrets.add(password);
                secrets.add(encoded.clone());
                headers.push((AUTHORIZATION.to_owned(), format!("Basic {encoded}")));
            }
        }
        Auth::Bearer { token } => {
            if !has_header(headers, AUTHORIZATION) {
                let token = resolver.text(token);
                headers.push((AUTHORIZATION.to_owned(), format!("Bearer {token}")));
                secrets.add(token);
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
                ApiKeyLocation::Header => {
                    let value = resolver.text(value);
                    secrets.add(value.clone());
                    headers.push((key, value));
                }
                ApiKeyLocation::Query => {
                    let value = resolver.text(value);
                    secrets.add(value.clone());
                    query.push((key, value));
                }
            }
        }
        Auth::Other { kind, .. } => return Err(ExecuteError::UnsupportedAuth(kind.clone())),
    }
    Ok(())
}
