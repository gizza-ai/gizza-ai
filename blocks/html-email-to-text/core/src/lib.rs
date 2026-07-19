//! gizza-ai/html-email-to-text core — pure compute, shared by the chat skill
//! block and the web page. No wafer/wasm-bindgen deps.
//!
//! Turns an HTML email BODY into a clean plain-text version, the way an email
//! client's "text/plain alternative" is generated: tags stripped, paragraphs and
//! lists preserved, HTML entities decoded, and — the email-specific part —
//! hyperlinks rendered in a chosen style and lines optionally hard-wrapped to the
//! classic ~72-column plain-text-email width.
//!
//! `<a href>` anchors are rewritten by us BEFORE the body is handed to
//! `nanohtml2text` so the link style is fully under our control:
//!   - `text`     — anchor text only, URL dropped (a bare stripper).
//!   - `inline`   — `anchor text (https://url)` inline after the text.
//!   - `footnote` — `anchor text[1]` inline, with a numbered `[1] https://url`
//!                  reference list appended at the end.

/// How hyperlinks are rendered in the plain-text output.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum LinkMode {
    /// Keep only the anchor text; drop the URL.
    Text,
    /// Append the URL in parentheses after the anchor text.
    Inline,
    /// Number each link and list the URLs at the bottom.
    Footnote,
}

impl LinkMode {
    fn parse(s: &str) -> Result<LinkMode, String> {
        match s.trim() {
            "" | "inline" => Ok(LinkMode::Inline),
            "text" => Ok(LinkMode::Text),
            "footnote" => Ok(LinkMode::Footnote),
            other => Err(format!(
                "unknown links mode {other:?}; expected 'text', 'inline', or 'footnote'"
            )),
        }
    }
}

/// Convert an HTML email `html` body to clean plain text.
///
/// * `links` — `"text"`, `"inline"` (default), or `"footnote"` (see [`LinkMode`]).
/// * `wrap`  — hard-wrap each line to at most this many columns on word
///   boundaries; `0` disables wrapping. Long words (e.g. URLs) are never split.
///
/// Errors on empty input or an unknown `links` mode.
pub fn convert(html: &str, links: &str, wrap: u32) -> Result<String, String> {
    if html.trim().is_empty() {
        return Err("input is empty".into());
    }
    let mode = LinkMode::parse(links)?;

    // 1. Rewrite <a href> anchors into plain-text-friendly HTML per the mode.
    let (rewritten, footnotes) = rewrite_links(html, mode);

    // 2. Strip the remaining markup to plain text (entities decoded here too).
    let raw = nanohtml2text::html2text(&rewritten);

    // 3. Normalize line endings, then collapse runs of 3+ newlines to 2.
    let body = normalize(&raw);

    // 4. Optionally hard-wrap the body lines (URLs / long tokens never split).
    let mut out = if wrap > 0 {
        wrap_lines(&body, wrap as usize)
    } else {
        body
    };

    // 5. Append the footnote reference list (kept unwrapped so URLs stay intact).
    if mode == LinkMode::Footnote && !footnotes.is_empty() {
        let mut refs = String::new();
        for (i, url) in footnotes.iter().enumerate() {
            refs.push_str(&format!("[{}] {}\n", i + 1, url));
        }
        out.push_str("\n\n");
        out.push_str(refs.trim_end());
    }

    Ok(out.trim().to_string())
}

/// CRLF -> LF, then collapse 3+ consecutive newlines to 2 and trim.
fn normalize(s: &str) -> String {
    let lf = s.replace("\r\n", "\n").replace('\r', "\n");
    let mut out = String::with_capacity(lf.len());
    let mut newline_run = 0usize;
    for ch in lf.chars() {
        if ch == '\n' {
            newline_run += 1;
            if newline_run <= 2 {
                out.push(ch);
            }
        } else {
            newline_run = 0;
            out.push(ch);
        }
    }
    out.trim().to_string()
}

/// Rewrite every `<a ... href=...>inner</a>` in `html` according to `mode`,
/// returning the rewritten HTML plus the ordered list of footnote URLs (empty
/// unless `mode == Footnote`). We always rewrite anchors ourselves — even in
/// `Text` mode — because `nanohtml2text` otherwise appends `(url)` to every link
/// on its own; stripping the anchors first keeps the link style fully ours.
fn rewrite_links(html: &str, mode: LinkMode) -> (String, Vec<String>) {
    let lower = html.to_ascii_lowercase();
    let mut out = String::with_capacity(html.len());
    let mut footnotes: Vec<String> = Vec::new();
    let mut i = 0usize;

    while i < html.len() {
        let Some(rel) = lower[i..].find("<a") else {
            out.push_str(&html[i..]);
            break;
        };
        let start = i + rel; // index of '<'
        let after = start + 2; // just past "<a"
        // Only treat "<a" as an anchor start if followed by whitespace, '>' or '/'.
        let is_anchor = html[after..]
            .chars()
            .next()
            .map(|c| c.is_whitespace() || c == '>' || c == '/')
            .unwrap_or(false);
        if !is_anchor {
            out.push_str(&html[i..after]);
            i = after;
            continue;
        }
        // Copy everything before the anchor verbatim.
        out.push_str(&html[i..start]);

        // Find the end of the opening tag.
        let Some(gt_rel) = html[after..].find('>') else {
            out.push_str(&html[start..]);
            break;
        };
        let tag_end = after + gt_rel; // index of '>'
        let href = extract_href(&html[after..tag_end]);

        // Find the matching </a>.
        let content_start = tag_end + 1;
        let Some(close_rel) = lower[content_start..].find("</a") else {
            // Unclosed anchor: emit the inner content and stop rewriting.
            out.push_str(&html[content_start..]);
            break;
        };
        let close_start = content_start + close_rel;
        let inner = &html[content_start..close_start];
        let resume = lower[close_start..]
            .find('>')
            .map(|r| close_start + r + 1)
            .unwrap_or(close_start + 3);

        emit_link(&mut out, &mut footnotes, inner, href.as_deref(), mode);
        i = resume;
    }

    (out, footnotes)
}

/// Emit the anchor `inner` HTML followed by the URL rendered per `mode`.
fn emit_link(
    out: &mut String,
    footnotes: &mut Vec<String>,
    inner: &str,
    href: Option<&str>,
    mode: LinkMode,
) {
    // Keep the inner markup so nanohtml2text renders the visible link text.
    out.push_str(inner);

    let Some(raw_href) = href else { return };
    let href = raw_href.trim();
    if href.is_empty() {
        return;
    }
    // Skip in-page anchors and non-navigational schemes — they add noise.
    let lower_href = href.to_ascii_lowercase();
    if href.starts_with('#')
        || lower_href.starts_with("javascript:")
        || lower_href.starts_with("data:")
    {
        return;
    }
    // mailto:/tel: links read better without the scheme prefix.
    let display = href
        .strip_prefix("mailto:")
        .or_else(|| href.strip_prefix("tel:"))
        .unwrap_or(href);
    let display = decode_entities(display);

    // Don't duplicate when the visible text already IS the URL/address.
    let inner_text = strip_tags(inner);
    let inner_text = inner_text.trim();
    if inner_text == display || inner_text == href {
        return;
    }

    match mode {
        LinkMode::Inline => {
            out.push_str(" (");
            out.push_str(&display);
            out.push(')');
        }
        LinkMode::Footnote => {
            footnotes.push(display);
            out.push_str(&format!("[{}]", footnotes.len()));
        }
        LinkMode::Text => {}
    }
}

/// Extract the value of the `href` attribute from an opening-tag attribute
/// string, matching `href` only as a whole attribute name.
fn extract_href(attrs: &str) -> Option<String> {
    let bytes = attrs.as_bytes();
    let lower = attrs.to_ascii_lowercase();
    let lb = lower.as_bytes();
    let mut i = 0usize;
    while let Some(rel) = lower[i..].find("href") {
        let pos = i + rel;
        // Preceding char must be whitespace or start-of-string (not data-href).
        let ok_prev = pos == 0 || bytes[pos - 1].is_ascii_whitespace();
        let mut j = pos + 4;
        while j < bytes.len() && lb[j].is_ascii_whitespace() {
            j += 1;
        }
        if ok_prev && j < bytes.len() && bytes[j] == b'=' {
            j += 1;
            while j < bytes.len() && lb[j].is_ascii_whitespace() {
                j += 1;
            }
            if j >= bytes.len() {
                return None;
            }
            let quote = bytes[j];
            if quote == b'"' || quote == b'\'' {
                j += 1;
                let vstart = j;
                while j < bytes.len() && bytes[j] != quote {
                    j += 1;
                }
                return Some(attrs[vstart..j].to_string());
            }
            // Unquoted value: read until whitespace.
            let vstart = j;
            while j < bytes.len() && !bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            return Some(attrs[vstart..j].to_string());
        }
        i = pos + 4;
    }
    None
}

/// Crude tag stripper for comparing an anchor's visible text to its URL.
fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for ch in s.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    decode_entities(out.trim())
}

/// Decode the handful of HTML entities that show up in URLs / short text.
/// `&amp;` is decoded LAST so `&amp;lt;` round-trips to the literal `&lt;`.
fn decode_entities(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
}

/// Hard-wrap each line of `s` to at most `width` columns on spaces. Existing
/// line breaks are preserved; a single word longer than `width` (e.g. a URL) is
/// left whole rather than split.
fn wrap_lines(s: &str, width: usize) -> String {
    if width == 0 {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len() + s.len() / width.max(1) + 1);
    for (li, line) in s.split('\n').enumerate() {
        if li > 0 {
            out.push('\n');
        }
        if line.chars().count() <= width || !line.contains(' ') {
            out.push_str(line);
            continue;
        }
        let mut cur = 0usize;
        let mut first = true;
        for word in line.split(' ').filter(|w| !w.is_empty()) {
            let wlen = word.chars().count();
            if first {
                out.push_str(word);
                cur = wlen;
                first = false;
            } else if cur + 1 + wlen <= width {
                out.push(' ');
                out.push_str(word);
                cur += 1 + wlen;
            } else {
                out.push('\n');
                out.push_str(word);
                cur = wlen;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_tags_keeps_text() {
        let t = convert("<h1>Hi</h1><p>Hello <b>world</b>.</p>", "inline", 0).unwrap();
        assert!(t.contains("Hi"), "got: {t:?}");
        assert!(t.contains("Hello world."), "got: {t:?}");
        assert!(!t.contains('<'), "no tags: {t:?}");
    }

    #[test]
    fn inline_link_shows_url_in_parens() {
        let t = convert(
            r#"<p>Please <a href="https://example.com/x">click here</a> now.</p>"#,
            "inline",
            0,
        )
        .unwrap();
        assert_eq!(t, "Please click here (https://example.com/x) now.");
    }

    #[test]
    fn text_mode_drops_url() {
        let t = convert(
            r#"<a href="https://example.com/x">click here</a>"#,
            "text",
            0,
        )
        .unwrap();
        assert_eq!(t, "click here");
    }

    #[test]
    fn footnote_mode_numbers_and_lists_urls() {
        let t = convert(
            r#"<p>See <a href="https://a.example/1">docs</a> and <a href="https://b.example/2">blog</a>.</p>"#,
            "footnote",
            0,
        )
        .unwrap();
        assert_eq!(
            t,
            "See docs[1] and blog[2].\n\n[1] https://a.example/1\n[2] https://b.example/2"
        );
    }

    #[test]
    fn decodes_entities_and_amp_in_url() {
        let t = convert(
            r#"<a href="https://x.example/s?a=1&amp;b=2">Search &amp; go</a>"#,
            "inline",
            0,
        )
        .unwrap();
        assert_eq!(t, "Search & go (https://x.example/s?a=1&b=2)");
    }

    #[test]
    fn no_duplicate_when_text_is_url() {
        let t = convert(
            r#"<a href="https://example.com">https://example.com</a>"#,
            "inline",
            0,
        )
        .unwrap();
        assert_eq!(t, "https://example.com");
    }

    #[test]
    fn mailto_scheme_stripped() {
        let t = convert(
            r#"<a href="mailto:hi@example.com">email us</a>"#,
            "inline",
            0,
        )
        .unwrap();
        assert_eq!(t, "email us (hi@example.com)");
    }

    #[test]
    fn skips_anchor_only_and_javascript_links() {
        let t = convert(
            r##"<a href="#top">Top</a> <a href="javascript:void(0)">Menu</a>"##,
            "inline",
            0,
        )
        .unwrap();
        assert_eq!(t, "Top Menu");
    }

    #[test]
    fn wrap_hard_wraps_long_lines_on_spaces() {
        let t = convert("<p>one two three four five six</p>", "text", 12).unwrap();
        for line in t.lines() {
            assert!(line.chars().count() <= 12, "line too long: {line:?}");
        }
        assert!(t.contains('\n'), "expected a wrap: {t:?}");
    }

    #[test]
    fn wrap_never_splits_a_long_url() {
        let url = "https://example.com/a/very/long/path/that/exceeds/the/wrap/width";
        let html = format!(r#"<a href="{url}">link</a>"#);
        let t = convert(&html, "inline", 20).unwrap();
        assert!(t.contains(url), "URL must stay intact: {t:?}");
    }

    #[test]
    fn does_not_match_non_anchor_tags() {
        let t = convert("<article><p>Body text</p></article>", "inline", 0).unwrap();
        assert!(t.contains("Body text"), "got: {t:?}");
    }

    #[test]
    fn empty_input_errors() {
        assert!(convert("   ", "inline", 0).is_err());
    }

    #[test]
    fn unknown_links_mode_errors() {
        let e = convert("<p>hi</p>", "sideways", 0).unwrap_err();
        assert!(e.contains("unknown links mode"), "got: {e:?}");
    }
}
