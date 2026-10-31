//! `request_get` and `request_update` (S3), with the window's auth masking (S7): a basic
//! password, a bearer token, an API key value and every field of another auth type are
//! masked on the way out, and a field that still holds the mask on the way in keeps the
//! stored value.

use reqflux_core::model::{Auth, Id, Item, Request, RequestNode, Workspace};
use reqflux_engine::MASK;
use serde_json::json;

use super::{EngineState, ItemArgs, UpdateArgs, encode, id_result, parse_id};
use crate::tools::ToolError;

/// A request found by id, with the collection and folders that hold it.
pub(super) struct Found<'a> {
    /// The ids and slugs of the collection and the enclosing folders, outermost first.
    pub(super) parents: Vec<(Id, &'a str)>,
    /// The request.
    pub(super) request: &'a RequestNode,
}

/// The request `id` in `workspace`, if any.
pub(super) fn find_request(workspace: &Workspace, id: Id) -> Option<Found<'_>> {
    workspace.collections.iter().find_map(|collection| {
        let mut parents = vec![(collection.collection.id, collection.slug.as_str())];
        find_in(&collection.items, id, &mut parents).map(|request| Found { parents, request })
    })
}

fn find_in<'a>(
    items: &'a [Item],
    id: Id,
    parents: &mut Vec<(Id, &'a str)>,
) -> Option<&'a RequestNode> {
    for item in items {
        match item {
            Item::Request(request) if request.request.id == id => return Some(request),
            Item::Request(_) => {}
            Item::Folder(folder) => {
                parents.push((folder.folder.id, folder.slug.as_str()));
                if let Some(request) = find_in(&folder.items, id, parents) {
                    return Some(request);
                }
                parents.pop();
            }
        }
    }
    None
}

fn unknown(id: Id) -> ToolError {
    ToolError::Engine(format!("no request has the id {id}"))
}

/// `request_get`: the request document as TOML with its auth secrets masked, plus its
/// parent's id and slug path (S3, S7).
///
/// # Errors
///
/// A bad or unknown id, no open workspace, or the document does not encode.
pub fn request_get(state: &mut EngineState, args: ItemArgs) -> Result<String, ToolError> {
    let id = parse_id(&args.id)?;
    let engine = state.reloaded()?;
    let found = find_request(engine.workspace(), id).ok_or_else(|| unknown(id))?;
    let mut request = found.request.request.clone();
    request.auth = masked(&request.auth);
    let document = toml::to_string(&request)
        .map_err(|error| ToolError::Engine(format!("could not encode the request: {error}")))?;
    let parent_path: Vec<&str> = found.parents.iter().map(|(_, slug)| *slug).collect();
    let parent_id = found.parents.last().map(|(id, _)| id.to_string());
    encode(&json!({
        "id": id.to_string(),
        "name": request.name,
        "parent_id": parent_id,
        "parent_path": parent_path.join("/"),
        "document": document,
    }))
}

/// `request_update`: replaces the request `id` with `document`, hash-checked against the
/// file as the server last loaded it, so an edit made since is a conflict and nothing is
/// written (S3). A masked auth secret keeps the stored value (S7).
///
/// # Errors
///
/// A bad or unknown id, a document that does not parse or carries another id, a mask
/// with no stored value to keep, or a conflict.
pub fn request_update(state: &mut EngineState, args: UpdateArgs) -> Result<String, ToolError> {
    let id = parse_id(&args.id)?;
    // No reload: the hash check compares against the file as last read, so a change made
    // since then is a conflict, never overwritten.
    let engine = state.engine()?;
    let mut request: Request = toml::from_str(&args.document).map_err(|error| {
        ToolError::Invalid(format!(
            "the document is not a request: {}",
            error.message()
        ))
    })?;
    if request.id != id {
        return Err(ToolError::Invalid(format!(
            "the document's id {} is not {id}; a request keeps its id",
            request.id
        )));
    }
    let stored = find_request(engine.workspace(), id).ok_or_else(|| unknown(id))?;
    request.auth = restored(request.auth, &stored.request.request.auth)?;

    let selector = id.to_string();
    let base = engine
        .request_base_hash(&selector)
        .map_err(|error| ToolError::Engine(error.to_string()))?;
    engine
        .save_request(&selector, &request, base)
        .map_err(|error| ToolError::Engine(error.to_string()))?;
    engine
        .reload()
        .map_err(|error| ToolError::Engine(format!("the workspace does not load: {error}")))?;
    id_result(id)
}

/// `value` masked unless it is empty, so an unset secret stays visibly unset.
fn mask(value: &str) -> String {
    if value.is_empty() {
        String::new()
    } else {
        MASK.to_owned()
    }
}

/// `auth` with its secret fields masked, as the window shows them.
pub(super) fn masked(auth: &Auth) -> Auth {
    match auth {
        Auth::Inherit | Auth::None => auth.clone(),
        Auth::Basic { username, password } => Auth::Basic {
            username: username.clone(),
            password: mask(password),
        },
        Auth::Bearer { token } => Auth::Bearer { token: mask(token) },
        Auth::ApiKey {
            key,
            value,
            location,
        } => Auth::ApiKey {
            key: key.clone(),
            value: mask(value),
            location: *location,
        },
        // The engine does not know which fields of other types hold credentials, so every
        // value is masked, as in the window.
        Auth::Other { kind, fields } => Auth::Other {
            kind: kind.clone(),
            fields: fields
                .keys()
                .map(|key| (key.clone(), toml::Value::String(MASK.to_owned())))
                .collect(),
        },
    }
}

/// `incoming` with every field that still holds the mask set back to the stored value.
fn restored(incoming: Auth, stored: &Auth) -> Result<Auth, ToolError> {
    let keep = |value: String, field: &str, stored: Option<&str>| {
        if value != MASK {
            return Ok(value);
        }
        stored.map(str::to_owned).ok_or_else(|| {
            ToolError::Invalid(format!(
                "the auth {field} holds the mask but the stored request has no {field} to keep"
            ))
        })
    };
    Ok(match incoming {
        Auth::Basic { username, password } => {
            let stored = match stored {
                Auth::Basic { password, .. } => Some(password.as_str()),
                _ => None,
            };
            Auth::Basic {
                username,
                password: keep(password, "password", stored)?,
            }
        }
        Auth::Bearer { token } => {
            let stored = match stored {
                Auth::Bearer { token } => Some(token.as_str()),
                _ => None,
            };
            Auth::Bearer {
                token: keep(token, "token", stored)?,
            }
        }
        Auth::ApiKey {
            key,
            value,
            location,
        } => {
            let stored = match stored {
                Auth::ApiKey { value, .. } => Some(value.as_str()),
                _ => None,
            };
            Auth::ApiKey {
                key,
                value: keep(value, "value", stored)?,
                location,
            }
        }
        Auth::Other { kind, fields } => {
            let stored_fields = match stored {
                Auth::Other {
                    kind: stored_kind,
                    fields,
                } if *stored_kind == kind => Some(fields),
                _ => None,
            };
            let mut restored = toml::Table::new();
            for (key, value) in fields {
                let value = match value {
                    toml::Value::String(text) if text == MASK => stored_fields
                        .and_then(|fields| fields.get(&key))
                        .cloned()
                        .ok_or_else(|| {
                            ToolError::Invalid(format!(
                                "the auth field {key} holds the mask but the stored request \
                                 has no {key} to keep"
                            ))
                        })?,
                    other => other,
                };
                restored.insert(key, value);
            }
            Auth::Other {
                kind,
                fields: restored,
            }
        }
        other @ (Auth::Inherit | Auth::None) => other,
    })
}
