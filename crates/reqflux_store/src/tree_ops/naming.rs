//! Display names to slugs, and finding a free one in a directory.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::error::StoreError;
use crate::layout::{
    environment_file_in, examples_dir_in, request_file_in, script_files, validate_request_slug,
    validate_slug,
};
use crate::save::io_error;

/// What the slug names, which decides the reserved slugs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SlugFor {
    /// A collection or folder directory.
    Container,
    /// A request file, which cannot use a reserved slug.
    Request,
    /// An environment file, `<slug>.env.toml`.
    Environment,
}

/// Slugs a display name: lowercase ASCII alphanumerics, every other run of characters a
/// single `-`. A name without alphanumerics becomes `item`.
fn slugify(name: &str) -> String {
    let mut out = String::new();
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    match out.trim_matches('-') {
        "" => "item".to_owned(),
        trimmed => trimmed.to_owned(),
    }
}

/// The trimmed name and its validated slug.
///
/// # Errors
///
/// [`StoreError::EmptyName`], or [`StoreError::ReservedSlug`] for a request.
pub(super) fn named(name: &str, kind: SlugFor) -> Result<(String, String), StoreError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(StoreError::EmptyName);
    }
    let slug = slugify(name);
    match kind {
        SlugFor::Container | SlugFor::Environment => validate_slug(&slug)?,
        SlugFor::Request => validate_request_slug(&slug)?,
    }
    Ok((name.to_owned(), slug))
}

/// `base`, then `base 2`, `base 3`, ...: the first name whose slug is valid and free in
/// `dir`, with that slug.
pub(super) fn numbered(
    dir: &Path,
    base: &str,
    kind: SlugFor,
) -> Result<(String, String), StoreError> {
    let mut n: u64 = 1;
    loop {
        let candidate = if n == 1 {
            base.to_owned()
        } else {
            format!("{base} {n}")
        };
        if let Ok((name, slug)) = named(&candidate, kind)
            && holder(dir, &slug, kind)?.is_none()
        {
            return Ok((name, slug));
        }
        n += 1;
    }
}

/// Fails with [`StoreError::SlugTaken`] when an entry in `dir` holds `slug`.
pub(super) fn ensure_free(dir: &Path, slug: &str, kind: SlugFor) -> Result<(), StoreError> {
    match holder(dir, slug, kind)? {
        Some(path) => Err(StoreError::SlugTaken {
            path,
            slug: slug.to_owned(),
        }),
        None => Ok(()),
    }
}

/// The first existing entry of `dir` that holds `slug`: a directory or a request file, and
/// for a request also its sidecars. A container's slug only clashes with the first two,
/// so a folder named `folder` may sit beside `folder.pre.js`. In the environments directory
/// only `<slug>.env.toml` holds a slug. A broken symlink counts.
fn holder(dir: &Path, slug: &str, kind: SlugFor) -> Result<Option<PathBuf>, StoreError> {
    if kind == SlugFor::Environment {
        return first_existing([environment_file_in(dir, slug)]);
    }
    let mut candidates = vec![dir.join(slug), request_file_in(dir, slug)];
    if kind == SlugFor::Request {
        let scripts = script_files(dir, slug);
        candidates.extend([
            scripts.pre_request,
            scripts.test,
            examples_dir_in(dir, slug),
        ]);
    }
    first_existing(candidates)
}

fn first_existing(
    candidates: impl IntoIterator<Item = PathBuf>,
) -> Result<Option<PathBuf>, StoreError> {
    for path in candidates {
        match fs::symlink_metadata(&path) {
            Ok(_) => return Ok(Some(path)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(source) => return Err(io_error(&path, source)),
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_is_lowercase_kebab_ascii() {
        assert_eq!(slugify("New request 2"), "new-request-2");
        assert_eq!(slugify("  API_v2 !!Copy "), "api-v2-copy");
        assert_eq!(slugify("!!!"), "item");
    }
}
