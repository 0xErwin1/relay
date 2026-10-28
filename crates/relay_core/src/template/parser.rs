//! Splits a text into literal runs and `{{name}}` references.
//!
//! A reference is `{{`, a name without `{` or `}`, and `}}`. The name is trimmed and must not
//! be empty. Anything else, such as an unclosed `{{`, `{{}}` or single braces, is literal.

/// A run of the source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Segment<'a> {
    /// Text copied as is.
    Literal(&'a str),
    /// A reference: `raw` is the whole `{{...}}`, `name` the trimmed name.
    Reference { raw: &'a str, name: &'a str },
}

/// Iterates over the segments of `text`; concatenating them yields `text`.
pub(super) fn segments(text: &str) -> Segments<'_> {
    Segments { rest: text }
}

/// Iterator returned by [`segments`].
#[derive(Debug, Clone)]
pub(super) struct Segments<'a> {
    rest: &'a str,
}

impl<'a> Iterator for Segments<'a> {
    type Item = Segment<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.rest.is_empty() {
            return None;
        }
        if let Some((raw, name, after)) = reference_at_start(self.rest) {
            self.rest = after;
            return Some(Segment::Reference { raw, name });
        }
        // Skip the first byte when it is a `{` that does not open a reference, so the scan
        // moves forward; `{` is ASCII, so the split is on a char boundary.
        let from = usize::from(self.rest.starts_with('{'));
        let end = self
            .rest
            .get(from..)
            .and_then(|tail| tail.find("{{"))
            .map_or(self.rest.len(), |offset| offset + from);
        let (literal, after) = self.rest.split_at(end);
        self.rest = after;
        Some(Segment::Literal(literal))
    }
}

/// Parses a reference at the start of `text`, returning its raw text, name and the remainder.
fn reference_at_start(text: &str) -> Option<(&str, &str, &str)> {
    let inner_and_rest = text.strip_prefix("{{")?;
    let close = inner_and_rest.find(['{', '}'])?;
    let (inner, after_inner) = inner_and_rest.split_at(close);
    let after = after_inner.strip_prefix("}}")?;
    let name = inner.trim();
    if name.is_empty() {
        return None;
    }
    let (raw, _) = text.split_at(text.len() - after.len());
    Some((raw, name, after))
}
