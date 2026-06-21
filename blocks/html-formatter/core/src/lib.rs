//! gizza-ai/html-formatter core — pretty-print HTML with consistent indentation.
//! A forgiving, dependency-free tokenizer (HTML is not well-formed XML): it
//! understands void elements, self-closing tags, comments/doctype, quoted
//! attributes (so `>` inside an attribute value is safe), and preserves the
//! verbatim contents of `pre`/`textarea`/`script`/`style`.

/// HTML void elements — they never have a closing tag, so they don't indent.
const VOID: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];
/// Elements whose contents must be preserved verbatim (not re-indented).
const RAW: &[&str] = &["pre", "textarea", "script", "style"];

fn tag_name(raw: &str) -> String {
    raw.trim_start_matches('<')
        .trim_start_matches('/')
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect::<String>()
        .to_ascii_lowercase()
}

/// Index just past the tag's closing `>`, respecting quoted attribute values.
fn scan_tag(b: &[u8], start: usize) -> usize {
    let mut j = start + 1;
    let mut quote = 0u8;
    while j < b.len() {
        let c = b[j];
        if quote != 0 {
            if c == quote {
                quote = 0;
            }
        } else if c == b'"' || c == b'\'' {
            quote = c;
        } else if c == b'>' {
            return j + 1;
        }
        j += 1;
    }
    b.len()
}

fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Pretty-print `html` using `indent_size` spaces per level (clamped 0..=8).
pub fn format(html: &str, indent_size: usize) -> Result<String, String> {
    if html.trim().is_empty() {
        return Err("no HTML input".into());
    }
    let unit = " ".repeat(indent_size.min(8));
    let b = html.as_bytes();
    let lower = html.to_ascii_lowercase();
    let n = b.len();
    let mut i = 0usize;
    let mut depth: usize = 0;
    let mut out = String::new();

    let mut line = |out: &mut String, depth: usize, s: &str| {
        for _ in 0..depth {
            out.push_str(&unit);
        }
        out.push_str(s);
        out.push('\n');
    };

    while i < n {
        if b[i] == b'<' {
            // Comment.
            if html[i..].starts_with("<!--") {
                let end = html[i..].find("-->").map(|p| i + p + 3).unwrap_or(n);
                line(&mut out, depth, html[i..end].trim());
                i = end;
                continue;
            }
            // Doctype / declaration / processing instruction.
            if i + 1 < n && (b[i + 1] == b'!' || b[i + 1] == b'?') {
                let end = scan_tag(b, i);
                line(&mut out, depth, html[i..end].trim());
                i = end;
                continue;
            }
            // A regular start/end/self-closing tag.
            let end = scan_tag(b, i);
            let raw = html[i..end].trim();
            let is_close = b.get(i + 1) == Some(&b'/');
            let name = tag_name(raw);
            let self_closing = raw.ends_with("/>");

            if is_close {
                depth = depth.saturating_sub(1);
                line(&mut out, depth, raw);
                i = end;
                continue;
            }

            // Raw element: emit open tag, verbatim inner, close tag.
            if RAW.contains(&name.as_str()) && !self_closing {
                let close_pat = format!("</{name}");
                let close_lt = lower[end..].find(&close_pat).map(|p| end + p).unwrap_or(n);
                let inner = &html[end..close_lt];
                line(&mut out, depth, raw);
                let trimmed = inner.trim_matches('\n');
                if !trimmed.trim().is_empty() {
                    for l in trimmed.split('\n') {
                        out.push_str(l);
                        out.push('\n');
                    }
                }
                if close_lt < n {
                    let close_end = scan_tag(b, close_lt);
                    line(&mut out, depth, html[close_lt..close_end].trim());
                    i = close_end;
                } else {
                    i = n;
                }
                continue;
            }

            // Ordinary open tag (or void / self-closing).
            line(&mut out, depth, raw);
            if !self_closing && !VOID.contains(&name.as_str()) {
                depth += 1;
            }
            i = end;
        } else {
            // Text run up to the next tag.
            let start = i;
            while i < n && b[i] != b'<' {
                i += 1;
            }
            let text = collapse_ws(&html[start..i]);
            if !text.is_empty() {
                line(&mut out, depth, &text);
            }
        }
    }

    Ok(out.trim_end().to_string() + "\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nests_block_elements() {
        let got = format("<div><p>hi</p></div>", 2).unwrap();
        assert_eq!(got, "<div>\n  <p>\n    hi\n  </p>\n</div>\n");
    }

    #[test]
    fn void_elements_do_not_indent() {
        let got = format("<div><br><img src=x></div>", 2).unwrap();
        assert_eq!(got, "<div>\n  <br>\n  <img src=x>\n</div>\n");
    }

    #[test]
    fn self_closing_does_not_indent() {
        let got = format("<div><br/></div>", 2).unwrap();
        assert_eq!(got, "<div>\n  <br/>\n</div>\n");
    }

    #[test]
    fn attribute_with_gt_is_safe() {
        let got = format(r#"<a title="x > y">t</a>"#, 2).unwrap();
        assert_eq!(got, "<a title=\"x > y\">\n  t\n</a>\n");
    }

    #[test]
    fn comment_and_doctype() {
        let got = format("<!DOCTYPE html><!-- hi --><p>x</p>", 2).unwrap();
        assert!(got.starts_with("<!DOCTYPE html>\n<!-- hi -->\n<p>"));
    }

    #[test]
    fn pre_preserved_verbatim() {
        let got = format("<body><pre>  a\n  b</pre></body>", 2).unwrap();
        // pre inner keeps its own spaces/newlines; tags indented at body's level.
        assert_eq!(got, "<body>\n  <pre>\n  a\n  b\n  </pre>\n</body>\n");
    }

    #[test]
    fn indent_size_configurable() {
        let got = format("<ul><li>a</li></ul>", 4).unwrap();
        assert_eq!(got, "<ul>\n    <li>\n        a\n    </li>\n</ul>\n");
    }

    #[test]
    fn errors_on_empty() {
        assert!(format("   ", 2).is_err());
    }
}
