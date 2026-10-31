//! What the command palette finds, without GPUI (command-palette S2): a fuzzy subsequence
//! scorer and the requests of a workspace as the palette lists them.

use reqflux_core::model::{Item, Workspace};

/// What every matched character scores.
const BASE: u32 = 1;

/// What a matched character scores on top when it follows the previous match directly.
const CONTIGUOUS: u32 = 4;

/// What a matched character scores on top when it starts a word.
const WORD_START: u32 = 6;

/// How well `query` matches `target` as a case-insensitive subsequence, higher being
/// better; `None` when it does not match. Whitespace in the query is ignored, so
/// `get users` matches `GET Users`. Contiguous runs and word starts score higher, and the
/// best alignment of the query over the target counts.
pub(crate) fn score(query: &str, target: &str) -> Option<u32> {
    let query: Vec<char> = query
        .chars()
        .filter(|character| !character.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect();
    if query.is_empty() {
        return Some(0);
    }
    let original: Vec<char> = target.chars().collect();
    let lower: Vec<char> = original.iter().map(|&character| fold(character)).collect();
    let starts: Vec<u32> = original
        .iter()
        .enumerate()
        .map(|(index, &character)| {
            let previous = index.checked_sub(1).and_then(|before| original.get(before));
            if starts_word(previous.copied(), character) {
                WORD_START
            } else {
                0
            }
        })
        .collect();
    // `best` holds, per target position, the best score of the query so far with its last
    // character matched exactly there.
    let mut best: Vec<Option<u32>> = vec![None; lower.len()];
    for (index, &wanted) in query.iter().enumerate() {
        // At position `j`: `adjacent` is the previous row at `j - 1`, `gap` the best of the
        // previous row before `j - 1`.
        let mut adjacent: Option<u32> = None;
        let mut gap: Option<u32> = None;
        best = lower
            .iter()
            .zip(&starts)
            .zip(&best)
            .map(|((&character, &start), &previous)| {
                let value = (character == wanted)
                    .then(|| {
                        if index == 0 {
                            Some(BASE + start)
                        } else {
                            let contiguous =
                                adjacent.map(|score| score + BASE + CONTIGUOUS + start);
                            let skipped = gap.map(|score| score + BASE + start);
                            contiguous.max(skipped)
                        }
                    })
                    .flatten();
                gap = gap.max(adjacent);
                adjacent = previous;
                value
            })
            .collect();
    }
    best.into_iter().flatten().max()
}

/// `character` lowercased, kept when its lowercase is more than one character.
fn fold(character: char) -> char {
    let mut lower = character.to_lowercase();
    match (lower.next(), lower.next()) {
        (Some(single), None) => single,
        _ => character,
    }
}

/// Whether `character` starts a word after `previous`: at the start, after a character that
/// is not alphanumeric, or an uppercase letter after a lowercase one.
fn starts_word(previous: Option<char>, character: char) -> bool {
    match previous {
        None => true,
        Some(previous) if !previous.is_alphanumeric() => true,
        Some(previous) => previous.is_lowercase() && character.is_uppercase(),
    }
}

/// A request as the palette lists and matches it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RequestEntry {
    /// The request id.
    pub(crate) id: String,
    /// The uppercase HTTP method.
    pub(crate) method: String,
    /// The request name.
    pub(crate) name: String,
    /// The URL as written in the request file.
    pub(crate) url: String,
    /// The collection and folder names down to the request, joined by ` / `.
    pub(crate) path: String,
}

impl RequestEntry {
    /// `METHOD name`, the row's title.
    pub(crate) fn title(&self) -> String {
        format!("{} {}", self.method, self.name)
    }

    /// How well `query` matches the request: by `METHOD name` or by `METHOD url`.
    pub(crate) fn score(&self, query: &str) -> Option<u32> {
        let by_name = score(query, &self.title());
        let by_url = score(query, &format!("{} {}", self.method, self.url));
        by_name.max(by_url)
    }
}

/// Every request of `workspace`, in tree order.
pub(crate) fn requests(workspace: &Workspace) -> Vec<RequestEntry> {
    let mut found = Vec::new();
    for collection in &workspace.collections {
        let mut path = vec![collection.collection.name.clone()];
        collect(&collection.items, &mut path, &mut found);
    }
    found
}

fn collect(items: &[Item], path: &mut Vec<String>, found: &mut Vec<RequestEntry>) {
    for item in items {
        match item {
            Item::Folder(folder) => {
                path.push(folder.folder.name.clone());
                collect(&folder.items, path, found);
                path.pop();
            }
            Item::Request(request) => found.push(RequestEntry {
                id: request.request.id.to_string(),
                method: request.request.method.as_str().to_owned(),
                name: request.request.name.clone(),
                url: request.request.url.clone(),
                path: path.join(" / "),
            }),
        }
    }
}

#[cfg(test)]
#[path = "palette_match_tests.rs"]
mod tests;
