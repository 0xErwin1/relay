//! What a response body previews as, without GPUI: the kind detected from the content type
//! or the image magic bytes, the size limits, and the readable text of an HTML page.

/// The image formats Preview decodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ImageKind {
    Png,
    Jpeg,
    Gif,
    Webp,
}

impl ImageKind {
    /// The name the preview meta shows: `PNG`, `JPEG`, `GIF` or `WebP`.
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Png => "PNG",
            Self::Jpeg => "JPEG",
            Self::Gif => "GIF",
            Self::Webp => "WebP",
        }
    }
}

/// What a body previews as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PreviewKind {
    Image(ImageKind),
    Html,
}

/// The largest image Preview decodes, in bytes.
pub(crate) const IMAGE_LIMIT: usize = 20 * 1024 * 1024;
/// The largest HTML page Preview extracts text from, in bytes.
pub(crate) const HTML_LIMIT: usize = 5 * 1024 * 1024;

/// What `body` previews as: by `content_type` first, then by the PNG, JPEG, GIF and WebP
/// magic bytes; `None` when it is neither an image nor HTML.
pub(crate) fn preview_kind(content_type: Option<&str>, body: &[u8]) -> Option<PreviewKind> {
    content_type
        .and_then(kind_of_type)
        .or_else(|| image_of_bytes(body).map(PreviewKind::Image))
}

/// Whether a body of `len` bytes is over the limit of `kind`.
pub(crate) fn too_large(kind: PreviewKind, len: usize) -> bool {
    let limit = match kind {
        PreviewKind::Image(_) => IMAGE_LIMIT,
        PreviewKind::Html => HTML_LIMIT,
    };
    len > limit
}

fn kind_of_type(content_type: &str) -> Option<PreviewKind> {
    let essence = content_type
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    match essence.as_str() {
        "image/png" => Some(PreviewKind::Image(ImageKind::Png)),
        "image/jpeg" | "image/jpg" | "image/pjpeg" => Some(PreviewKind::Image(ImageKind::Jpeg)),
        "image/gif" => Some(PreviewKind::Image(ImageKind::Gif)),
        "image/webp" => Some(PreviewKind::Image(ImageKind::Webp)),
        "text/html" | "application/xhtml+xml" => Some(PreviewKind::Html),
        _ => None,
    }
}

fn image_of_bytes(body: &[u8]) -> Option<ImageKind> {
    if body.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(ImageKind::Png)
    } else if body.starts_with(b"\xFF\xD8\xFF") {
        Some(ImageKind::Jpeg)
    } else if body.starts_with(b"GIF87a") || body.starts_with(b"GIF89a") {
        Some(ImageKind::Gif)
    } else if body.starts_with(b"RIFF") && body.get(8..12) == Some(b"WEBP".as_slice()) {
        Some(ImageKind::Webp)
    } else {
        None
    }
}

/// The readable parts of an HTML page.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct HtmlText {
    /// The `<title>`, whitespace collapsed; `None` when missing or empty.
    pub(crate) title: Option<String>,
    /// The text outside the title, one paragraph per block element, whitespace collapsed
    /// and empty paragraphs dropped.
    pub(crate) paragraphs: Vec<String>,
}

/// Elements that start a new paragraph.
const BLOCKS: &[&str] = &[
    "address",
    "article",
    "aside",
    "blockquote",
    "br",
    "dd",
    "div",
    "dl",
    "dt",
    "figcaption",
    "footer",
    "form",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "header",
    "hr",
    "li",
    "main",
    "nav",
    "ol",
    "p",
    "pre",
    "section",
    "table",
    "td",
    "th",
    "tr",
    "ul",
];

/// The title and readable text of `html`: tags stripped, `script`, `style` and comments
/// dropped, character references decoded and whitespace collapsed. A hand-written scanner,
/// not a parser: it never fails, it only reads less well on broken markup.
pub(crate) fn html_text(html: &str) -> HtmlText {
    let mut out = HtmlText::default();
    let mut paragraph = String::new();
    let mut title: Option<String> = None;
    let mut rest = html;
    while let Some(start) = rest.find('<') {
        let (text, tail) = rest.split_at(start);
        match title.as_mut() {
            Some(title) => title.push_str(text),
            None => paragraph.push_str(text),
        }
        if let Some(after) = tail.strip_prefix("<!--") {
            rest = after
                .find("-->")
                .map_or("", |end| after.get(end + 3..).unwrap_or(""));
            continue;
        }
        let Some(end) = tag_end(tail) else {
            // An unclosed `<` is text.
            paragraph.push_str(tail);
            rest = "";
            break;
        };
        let tag = tail.get(1..end).unwrap_or_default();
        rest = tail.get(end + 1..).unwrap_or_default();
        let closing = tag.starts_with('/');
        let name = tag
            .trim_start_matches('/')
            .split(|c: char| c.is_whitespace() || c == '/')
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        match name.as_str() {
            "script" | "style" if !closing => {
                rest = skip_past_close(rest, &name);
            }
            "title" if !closing => title = Some(String::new()),
            "title" => {
                if let Some(text) = title.take() {
                    let text = collapse(&decode_entities(&text));
                    if out.title.is_none() && !text.is_empty() {
                        out.title = Some(text);
                    }
                }
            }
            name if BLOCKS.contains(&name) => flush(&mut paragraph, &mut out.paragraphs),
            _ => {}
        }
    }
    paragraph.push_str(rest);
    flush(&mut paragraph, &mut out.paragraphs);
    out
}

/// The byte index of the `>` that ends the tag `text` starts with, skipping quoted
/// attribute values.
fn tag_end(text: &str) -> Option<usize> {
    let mut quote = None;
    for (index, c) in text.char_indices().skip(1) {
        match (quote, c) {
            (Some(open), c) if c == open => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => quote = Some(c),
            (None, '>') => return Some(index),
            (None, _) => {}
        }
    }
    None
}

/// `text` after the closing tag `</name>`, matched case-insensitively; empty when it does
/// not close.
fn skip_past_close<'a>(text: &'a str, name: &str) -> &'a str {
    let needle = format!("</{name}");
    let lower = text.to_ascii_lowercase();
    let Some(start) = lower.find(&needle) else {
        return "";
    };
    let tail = text.get(start..).unwrap_or_default();
    tag_end(tail).map_or("", |end| tail.get(end + 1..).unwrap_or_default())
}

/// Ends the current paragraph: decodes and collapses it and keeps it unless empty.
fn flush(paragraph: &mut String, paragraphs: &mut Vec<String>) {
    let text = collapse(&decode_entities(paragraph));
    if !text.is_empty() {
        paragraphs.push(text);
    }
    paragraph.clear();
}

/// `text` with every run of whitespace replaced by one space and the ends trimmed.
fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `text` with the common named and every numeric character reference decoded; an unknown
/// reference stays as written.
fn decode_entities(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('&') {
        let (before, tail) = rest.split_at(start);
        out.push_str(before);
        let decoded = tail
            .get(1..)
            .and_then(|body| {
                body.find(';')
                    .filter(|end| *end <= 10)
                    .map(|end| (body, end))
            })
            .and_then(|(body, end)| {
                let name = body.get(..end)?;
                Some((entity(name)?, end + 2))
            });
        match decoded {
            Some((c, len)) => {
                out.push(c);
                rest = tail.get(len..).unwrap_or_default();
            }
            None => {
                out.push('&');
                rest = tail.get(1..).unwrap_or_default();
            }
        }
    }
    out.push_str(rest);
    out
}

/// The character of the reference `&name;`.
fn entity(name: &str) -> Option<char> {
    if let Some(number) = name.strip_prefix('#') {
        let code = match number.strip_prefix(['x', 'X']) {
            Some(hex) => u32::from_str_radix(hex, 16).ok()?,
            None => number.parse().ok()?,
        };
        return char::from_u32(code);
    }
    match name {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        "nbsp" => Some('\u{a0}'),
        "copy" => Some('©'),
        "reg" => Some('®'),
        "hellip" => Some('…'),
        "mdash" => Some('—'),
        "ndash" => Some('–'),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG_MAGIC: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";

    #[test]
    fn the_kind_comes_from_the_content_type_first() {
        assert_eq!(
            preview_kind(Some("image/png"), b""),
            Some(PreviewKind::Image(ImageKind::Png))
        );
        assert_eq!(
            preview_kind(Some("IMAGE/JPEG; q=1"), b""),
            Some(PreviewKind::Image(ImageKind::Jpeg))
        );
        assert_eq!(
            preview_kind(Some("image/gif"), b""),
            Some(PreviewKind::Image(ImageKind::Gif))
        );
        assert_eq!(
            preview_kind(Some("image/webp"), b""),
            Some(PreviewKind::Image(ImageKind::Webp))
        );
        assert_eq!(
            preview_kind(Some("text/html; charset=utf-8"), b"{}"),
            Some(PreviewKind::Html)
        );
        assert_eq!(
            preview_kind(Some("application/xhtml+xml"), b""),
            Some(PreviewKind::Html)
        );
    }

    #[test]
    fn the_kind_falls_back_to_the_image_magic_bytes() {
        let png = preview_kind(Some("application/octet-stream"), PNG_MAGIC);
        assert_eq!(png, Some(PreviewKind::Image(ImageKind::Png)));
        assert_eq!(
            preview_kind(None, b"\xFF\xD8\xFF\xE0rest"),
            Some(PreviewKind::Image(ImageKind::Jpeg))
        );
        assert_eq!(
            preview_kind(None, b"GIF89a.."),
            Some(PreviewKind::Image(ImageKind::Gif))
        );
        assert_eq!(
            preview_kind(None, b"GIF87a.."),
            Some(PreviewKind::Image(ImageKind::Gif))
        );
        assert_eq!(
            preview_kind(None, b"RIFF\0\0\0\0WEBPVP8 "),
            Some(PreviewKind::Image(ImageKind::Webp))
        );
        assert_eq!(preview_kind(None, b"RIFF\0\0\0\0WAVE"), None);
    }

    #[test]
    fn json_text_and_unknown_bodies_have_no_preview() {
        assert_eq!(preview_kind(Some("application/json"), br#"{"a":1}"#), None);
        assert_eq!(preview_kind(Some("text/plain"), b"<html>"), None);
        assert_eq!(preview_kind(None, b""), None);
        assert_eq!(preview_kind(Some("image/svg+xml"), b"<svg/>"), None);
    }

    #[test]
    fn images_over_20_mb_and_html_over_5_mb_are_too_large() {
        let image = PreviewKind::Image(ImageKind::Png);
        assert!(!too_large(image, 20 * 1024 * 1024));
        assert!(too_large(image, 20 * 1024 * 1024 + 1));
        assert!(!too_large(PreviewKind::Html, 5 * 1024 * 1024));
        assert!(too_large(PreviewKind::Html, 5 * 1024 * 1024 + 1));
    }

    #[test]
    fn html_text_has_the_title_and_the_readable_paragraphs() {
        let page = "<!doctype html><html><head><TITLE> Fish &amp; Chips </TITLE>\
            <style>p { color: red }</style>\
            <script type=\"text/javascript\">if (a < b) { alert('</p>') }</SCRIPT></head>\
            <body><!-- hidden --><h1>Menu</h1>\n<p>Cod   and\n\t<b>chips</b>&nbsp;&lt;3</p>\
            <p></p><div>&#169; &#x41;&unknown; a&b</div></body></html>";
        assert_eq!(
            html_text(page),
            HtmlText {
                title: Some("Fish & Chips".to_owned()),
                paragraphs: vec![
                    "Menu".to_owned(),
                    "Cod and chips <3".to_owned(),
                    "© A&unknown; a&b".to_owned(),
                ],
            }
        );
    }

    #[test]
    fn html_text_survives_broken_markup() {
        assert_eq!(
            html_text("<p>open <b class='x>y'>bold"),
            HtmlText {
                title: None,
                paragraphs: vec!["open bold".to_owned()],
            }
        );
        assert_eq!(html_text("<title></title>a < b").title, None);
        assert_eq!(
            html_text("<title></title>a < b").paragraphs,
            vec!["a < b".to_owned()]
        );
        assert_eq!(
            html_text("<script>never closed").paragraphs,
            Vec::<String>::new()
        );
    }
}
