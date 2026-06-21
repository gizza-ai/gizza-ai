//! gizza-ai/html-minifier core — minify HTML by collapsing whitespace, removing
//! comments, and normalizing whitespace inside tags. A forgiving, dependency-free
//! tokenizer that preserves the verbatim contents of `pre`/`textarea`/`script`/
//! `style` and keeps inline spacing where it matters.

/// Elements whose contents must be preserved verbatim.
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

/// Collapse runs of whitespace *outside* quoted strings to single spaces, so
/// `<a   href = "x">` → `<a href = "x">` (values are left untouched).
fn normalize_tag(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut quote = 0u8;
    let mut prev_ws = false;
    for &c in raw.as_bytes() {
        if quote != 0 {
            out.push(c as char);
            if c == quote {
                quote = 0;
            }
            continue;
        }
        if c == b'"' || c == b'\'' {
            quote = c;
            out.push(c as char);
            prev_ws = false;
        } else if c.is_ascii_whitespace() {
            if !prev_ws {
                out.push(' ');
            }
            prev_ws = true;
        } else {
            out.push(c as char);
            prev_ws = false;
        }
    }
    out
}

fn collapse_internal(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Minify `html`. When `remove_comments` is true (default), `<!-- … -->` blocks
/// are dropped.
pub fn minify(html: &str, remove_comments: bool) -> Result<String, String> {
    if html.trim().is_empty() {
        return Err("no HTML input".into());
    }
    let b = html.as_bytes();
    let lower = html.to_ascii_lowercase();
    let n = b.len();
    let mut i = 0usize;
    let mut out = String::with_capacity(n);

    while i < n {
        if b[i] == b'<' {
            // Comment.
            if html[i..].starts_with("<!--") {
                let end = html[i..].find("-->").map(|p| i + p + 3).unwrap_or(n);
                if !remove_comments {
                    out.push_str(&html[i..end]);
                }
                i = end;
                continue;
            }
            // Doctype / declaration / PI — keep, normalized.
            if i + 1 < n && (b[i + 1] == b'!' || b[i + 1] == b'?') {
                let end = scan_tag(b, i);
                out.push_str(&normalize_tag(&html[i..end]));
                i = end;
                continue;
            }
            let end = scan_tag(b, i);
            let raw = &html[i..end];
            let name = tag_name(raw);
            let self_closing = raw.trim_end().ends_with("/>");

            // Raw element: emit open + verbatim inner + close.
            if RAW.contains(&name.as_str()) && !self_closing && b.get(i + 1) != Some(&b'/') {
                out.push_str(&normalize_tag(raw));
                let close_pat = format!("</{name}");
                let close_lt = lower[end..].find(&close_pat).map(|p| end + p).unwrap_or(n);
                out.push_str(&html[end..close_lt]); // verbatim inner
                if close_lt < n {
                    let close_end = scan_tag(b, close_lt);
                    out.push_str(&html[close_lt..close_end]);
                    i = close_end;
                } else {
                    i = n;
                }
                continue;
            }

            out.push_str(&normalize_tag(raw));
            i = end;
        } else {
            let start = i;
            while i < n && b[i] != b'<' {
                i += 1;
            }
            let t = &html[start..i];
            if t.trim().is_empty() {
                // Whitespace-only node: drop indentation (had a newline), else
                // keep a single significant inline space.
                if !t.contains('\n') && !t.is_empty() {
                    out.push(' ');
                }
            } else {
                // A leading/trailing space is significant only when it isn't
                // indentation (i.e. contains no newline).
                let lead_ws: String = t.chars().take_while(|c| c.is_whitespace()).collect();
                let trail_ws: String = t.chars().rev().take_while(|c| c.is_whitespace()).collect();
                let lead = !lead_ws.is_empty() && !lead_ws.contains('\n');
                let trail = !trail_ws.is_empty() && !trail_ws.contains('\n');
                if lead {
                    out.push(' ');
                }
                out.push_str(&collapse_internal(t));
                if trail {
                    out.push(' ');
                }
            }
        }
    }

    Ok(out.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapses_indentation() {
        let got = minify("<div>\n  <p>\n    hello world\n  </p>\n</div>", true).unwrap();
        assert_eq!(got, "<div><p>hello world</p></div>");
    }

    #[test]
    fn removes_comments_by_default() {
        let got = minify("<p>a</p><!-- note --><p>b</p>", true).unwrap();
        assert_eq!(got, "<p>a</p><p>b</p>");
    }

    #[test]
    fn keeps_comments_when_disabled() {
        let got = minify("<p>a</p><!-- note -->", false).unwrap();
        assert_eq!(got, "<p>a</p><!-- note -->");
    }

    #[test]
    fn normalizes_tag_whitespace_but_not_values() {
        let got = minify(r#"<a    href = "x y">t</a>"#, true).unwrap();
        assert_eq!(got, r#"<a href = "x y">t</a>"#);
    }

    #[test]
    fn preserves_pre_verbatim() {
        let got = minify("<div>\n <pre>  a\n  b</pre>\n</div>", true).unwrap();
        assert_eq!(got, "<div><pre>  a\n  b</pre></div>");
    }

    #[test]
    fn keeps_inline_space_between_text() {
        let got = minify("<b>a</b> <b>b</b>", true).unwrap();
        assert_eq!(got, "<b>a</b> <b>b</b>");
    }

    #[test]
    fn errors_on_empty() {
        assert!(minify("   ", true).is_err());
    }
}
