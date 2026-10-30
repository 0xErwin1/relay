//! Rejects raw design values outside the token files.
//!
//! Colors and pixel sizes belong in Bolt or in `src/tokens.rs`, so every screen reads a named
//! role. The scan skips test files (`*tests.rs`) and each file from its first `#[cfg(test)]`
//! line on, because unit tests compare against literal values on purpose.
//!
//! The scan is line based. A trailing `//` comment is stripped, but a `//` inside a
//! double-quoted string literal is kept, so a URL never cuts the rest of the line off the
//! scan. A `"` inside a char literal would be misread as opening a string; screen code does
//! not use one. Struct literals of the raw color types (`Hsla {`, `Rgba {`) are rejected,
//! while a type position such as `-> Hsla {` or `impl Hsla {` is not a literal and stays
//! allowed.

#![allow(clippy::panic, reason = "the helpers fail the calling test")]

use std::fs;
use std::path::{Path, PathBuf};

/// Source paths, relative to `src/`, that may hold raw values: ReqFlux's own tokens and its
/// brand colors.
const TOKEN_PATHS: &[&str] = &["tokens.rs", "brand.rs"];

#[test]
fn source_has_no_raw_colors_or_pixels() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let offenders = scan(&src, TOKEN_PATHS);
    assert!(
        offenders.is_empty(),
        "raw design values outside the token files:\n{}",
        offenders.join("\n")
    );
}

/// Returns `file:line: text` for every offending line under `root`.
fn scan(root: &Path, allowed: &[&str]) -> Vec<String> {
    let mut files = Vec::new();
    collect(root, &mut files);
    files.sort();

    let mut offenders = Vec::new();
    for file in files {
        let relative = file.strip_prefix(root).unwrap_or(&file);
        let is_test_file = relative
            .file_name()
            .is_some_and(|name| name.to_string_lossy().ends_with("tests.rs"));
        if is_test_file || allowed.iter().any(|path| relative.starts_with(path)) {
            continue;
        }
        let text =
            fs::read_to_string(&file).unwrap_or_else(|error| panic!("{}: {error}", file.display()));
        for (index, line) in text.lines().enumerate() {
            if line.trim_start().starts_with("#[cfg(test)]") {
                break;
            }
            if is_offending(line) {
                offenders.push(format!("{}:{}: {}", file.display(), index + 1, line.trim()));
            }
        }
    }
    offenders
}

fn collect(dir: &Path, files: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(dir).unwrap_or_else(|error| panic!("{}: {error}", dir.display()));
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, files);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            files.push(path);
        }
    }
}

fn is_offending(line: &str) -> bool {
    let code = strip_comment(line);
    ["rgb(", "rgba(", "hsla(", "px("]
        .iter()
        .any(|call| has_call(code, call))
        || has_call(code, "Theme::")
        || has_word(code, "ActiveTheme")
        || ["Hsla", "Rgba"]
            .iter()
            .any(|word| has_struct_literal(code, word))
        || has_hex_color(code)
}

/// The line with a trailing `//` comment removed. A `//` inside a double-quoted string
/// literal is kept, so a URL on the line never hides what follows it.
fn strip_comment(line: &str) -> &str {
    let mut in_string = false;
    let mut chars = line.char_indices().peekable();
    while let Some((at, c)) = chars.next() {
        match c {
            '\\' if in_string => {
                chars.next();
            }
            '"' => in_string = !in_string,
            '/' if !in_string && chars.peek().is_some_and(|(_, next)| *next == '/') => {
                return &line[..at];
            }
            _ => {}
        }
    }
    line
}

/// `call` appears as a bare call, not as the tail of a longer identifier.
fn has_call(code: &str, call: &str) -> bool {
    code.match_indices(call).any(|(at, _)| {
        !code[..at]
            .chars()
            .next_back()
            .is_some_and(|c| c.is_alphanumeric() || c == '_')
    })
}

/// `word` appears as a whole identifier, not inside a longer one.
fn has_word(code: &str, word: &str) -> bool {
    code.match_indices(word).any(|(at, _)| {
        let bounded_before = !code[..at]
            .chars()
            .next_back()
            .is_some_and(|c| c.is_alphanumeric() || c == '_');
        let bounded_after = !code[at + word.len()..]
            .chars()
            .next()
            .is_some_and(|c| c.is_alphanumeric() || c == '_');
        bounded_before && bounded_after
    })
}

/// `Word` is followed by a `{` as a struct literal, as in `Hsla { a: 1.0, ..color }`. A type
/// position, such as `-> Hsla {` or `impl Hsla {`, is not a raw value and stays allowed.
fn has_struct_literal(code: &str, word: &str) -> bool {
    code.match_indices(word).any(|(at, _)| {
        let before = &code[..at];
        let opens_a_literal = code[at + word.len()..].trim_start().starts_with('{');
        opens_a_literal && !is_type_position(before)
    })
}

/// Whether the text before `Word {` puts `Word` in type position.
fn is_type_position(before: &str) -> bool {
    let trimmed = before.trim_end();
    let last_word = trimmed
        .rsplit(|c: char| !(c.is_alphanumeric() || c == '_'))
        .next()
        .unwrap_or("");
    trimmed.ends_with("->")
        || matches!(
            last_word,
            "impl" | "struct" | "enum" | "union" | "trait" | "for" | "type" | "dyn"
        )
}

/// `0x` followed by exactly 6 or 8 hex digits.
fn has_hex_color(code: &str) -> bool {
    code.match_indices("0x").any(|(at, _)| {
        let digits = code[at + 2..]
            .chars()
            .take_while(|c| c.is_ascii_hexdigit() || *c == '_')
            .filter(|c| *c != '_')
            .count();
        digits == 6 || digits == 8
    })
}

#[test]
fn detects_each_raw_value() {
    for line in [
        "div().bg(rgb(0x101010))",
        "let c = rgba(0x10101080);",
        "hsla(0.0, 0.0, 0.0, 1.0)",
        "const C: u32 = 0xFF6B5E;",
        "const C: u32 = 0xFF6B5E80;",
        "div().w(px(12.0))",
        "gpui::px(1.0)",
        "let theme = Theme::global(cx);",
        "use gpui_component::active_theme::ActiveTheme;",
        "let wash = Hsla { a: 0.12, ..color };",
        "let c = Rgba { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };",
        // A `//` inside a string used to cut the rest of the line off the scan; the raw value
        // after it must still be caught.
        "let url = \"https://example.com\"; let c = rgb(0xffffff);",
    ] {
        assert!(is_offending(line), "{line}");
    }
    for line in [
        "div().px_2()",
        "let mask = 0xFF;",
        "tokens::space::LG",
        "// rgb(0x101010) in a comment",
        // A `//` inside a string is not a comment.
        "let url = \"https://example.com//path\";",
        // Type positions are not struct literals.
        "fn wash(color: Hsla) -> Hsla { color }",
        "fn class_color(class: StatusClass) -> Hsla {",
    ] {
        assert!(!is_offending(line), "{line}");
    }
}
