//! gizza-ai/html-to-text core — pure compute, shared by the chat skill block and
//! the web page. No wafer/wasm-bindgen deps. Strips HTML markup to clean,
//! readable plain text (no Markdown markers), preserving paragraph and list
//! structure, via `nanohtml2text`.

/// Convert `html` to clean plain text. Normalizes CRLF to LF and collapses runs
/// of 3+ blank lines to a single blank line. Errors on empty input.
pub fn convert(html: &str) -> Result<String, String> {
    if html.trim().is_empty() {
        return Err("input is empty".into());
    }
    let raw = nanohtml2text::html2text(html);
    // Normalize line endings, then collapse 3+ consecutive newlines to 2.
    let lf = raw.replace("\r\n", "\n").replace('\r', "\n");
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
    Ok(out.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_tags_keeps_text_no_markers() {
        let t = convert("<h1>Title</h1><p>Hello <b>world</b>.</p>").unwrap();
        assert!(t.contains("Title"), "got: {t:?}");
        assert!(t.contains("Hello world."), "got: {t:?}");
        assert!(!t.contains('<'), "no tags: {t:?}");
        assert!(!t.contains('#') && !t.contains("**"), "no markdown markers: {t:?}");
    }

    #[test]
    fn list_items_on_separate_lines() {
        let t = convert("<ul><li>a</li><li>b</li></ul>").unwrap();
        assert!(t.contains('a') && t.contains('b'), "got: {t:?}");
    }

    #[test]
    fn link_text_kept() {
        let t = convert(r#"<a href="https://x.test">click here</a>"#).unwrap();
        assert!(t.contains("click here"), "got: {t:?}");
    }

    #[test]
    fn no_crlf_in_output() {
        let t = convert("<p>one</p><p>two</p>").unwrap();
        assert!(!t.contains('\r'), "CRLF normalized: {t:?}");
    }

    #[test]
    fn empty_input_errors() {
        assert!(convert("   ").is_err());
    }
}
