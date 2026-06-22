//! gizza-ai/html-to-markdown core — pure compute, shared by the chat skill block
//! and the web page. No wafer/wasm-bindgen deps. Converts an HTML fragment or
//! whole page into clean Markdown via `htmd` (html5ever-based), preserving
//! headings, links, lists, code blocks, tables, etc.

/// Convert `html` into Markdown. Errors as a human-readable string if the input
/// is empty or the converter fails.
pub fn convert(html: &str) -> Result<String, String> {
    if html.trim().is_empty() {
        return Err("input is empty".into());
    }
    htmd::convert(html)
        .map(|md| md.trim().to_string())
        .map_err(|e| format!("HTML conversion failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headings_and_paragraphs() {
        let md = convert("<h1>Title</h1><p>Hello <b>world</b>.</p>").unwrap();
        assert!(md.contains("# Title"), "got: {md}");
        assert!(md.contains("**world**"), "got: {md}");
    }

    #[test]
    fn links_preserved() {
        let md = convert(r#"<a href="https://x.test">link</a>"#).unwrap();
        assert!(md.contains("[link](https://x.test)"), "got: {md}");
    }

    #[test]
    fn unordered_list() {
        let md = convert("<ul><li>a</li><li>b</li></ul>").unwrap();
        // htmd renders bullets as "*   a" (asterisk + spaces); accept * or -.
        assert!(md.contains('a') && md.contains('b'), "got: {md}");
        assert!(
            md.lines().all(|l| {
                let t = l.trim_start();
                t.is_empty() || t.starts_with('*') || t.starts_with('-')
            }),
            "every list line should be a bullet, got: {md}"
        );
    }

    #[test]
    fn code_block() {
        let md = convert("<pre><code>let x = 1;</code></pre>").unwrap();
        assert!(md.contains("let x = 1;"));
        assert!(md.contains("```"), "got: {md}");
    }

    #[test]
    fn empty_input_errors() {
        assert!(convert("   ").is_err());
    }
}
