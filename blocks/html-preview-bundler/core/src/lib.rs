//! gizza-ai/html-preview-bundler core — combine separate HTML, CSS, and JS into
//! one self-contained, runnable HTML document. If the HTML is a full document the
//! CSS/JS are injected into it; if it's a fragment it's wrapped in a minimal
//! page. Pure-Rust, dependency-free.

/// Case-insensitive `find` of `needle` in `hay`, returning the byte index in
/// `hay` (ASCII-lowercasing both, which is byte-position preserving).
fn find_ci(hay: &str, needle: &str) -> Option<usize> {
    hay.to_ascii_lowercase().find(&needle.to_ascii_lowercase())
}

fn style_block(css: &str) -> String {
    format!("<style>\n{}\n</style>", css.trim_end())
}
fn script_block(js: &str) -> String {
    format!("<script>\n{}\n</script>", js.trim_end())
}

/// Insert `insert` immediately before the first occurrence of `marker`
/// (case-insensitive). Returns true if inserted.
fn insert_before(doc: &mut String, marker: &str, insert: &str) -> bool {
    if let Some(pos) = find_ci(doc, marker) {
        doc.insert_str(pos, &format!("{insert}\n"));
        true
    } else {
        false
    }
}

/// Bundle `html`, `css`, and `js` into one self-contained HTML document.
pub fn bundle(html: &str, css: &str, js: &str, title: &str) -> Result<String, String> {
    if html.trim().is_empty() && css.trim().is_empty() && js.trim().is_empty() {
        return Err("provide at least some HTML, CSS, or JS".into());
    }
    let title = if title.trim().is_empty() { "Preview" } else { title.trim() };
    let has_doc = find_ci(html, "<html").is_some();

    if has_doc {
        let mut doc = html.to_string();
        // CSS → before </head>, else before </body>, else prepend.
        if !css.trim().is_empty() {
            let style = style_block(css);
            if !insert_before(&mut doc, "</head>", &style)
                && !insert_before(&mut doc, "</body>", &style)
            {
                doc.insert_str(0, &format!("{style}\n"));
            }
        }
        // JS → before </body>, else append.
        if !js.trim().is_empty() {
            let script = script_block(js);
            if !insert_before(&mut doc, "</body>", &script) {
                doc.push('\n');
                doc.push_str(&script);
            }
        }
        Ok(doc)
    } else {
        // Wrap a fragment in a minimal document.
        let head_css = if css.trim().is_empty() {
            String::new()
        } else {
            format!("  {}\n", style_block(css).replace('\n', "\n  "))
        };
        let body_js = if js.trim().is_empty() {
            String::new()
        } else {
            format!("  {}\n", script_block(js).replace('\n', "\n  "))
        };
        Ok(format!(
            "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n  <meta charset=\"utf-8\">\n  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n  <title>{title}</title>\n{head_css}</head>\n<body>\n{body}\n{body_js}</body>\n</html>\n",
            title = html_escape(title),
            body = html.trim_end(),
        ))
    }
}

/// Minimal escaping for the <title> text.
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_a_fragment() {
        let out = bundle("<h1>Hi</h1>", "h1{color:red}", "console.log(1)", "My Page").unwrap();
        assert!(out.starts_with("<!DOCTYPE html>"));
        assert!(out.contains("<title>My Page</title>"));
        assert!(out.contains("<style>"));
        assert!(out.contains("h1{color:red}"));
        assert!(out.contains("<h1>Hi</h1>"));
        assert!(out.contains("<script>"));
        assert!(out.contains("console.log(1)"));
        // script comes after the body content
        assert!(out.find("<h1>Hi</h1>").unwrap() < out.find("console.log(1)").unwrap());
    }

    #[test]
    fn injects_into_full_document() {
        let html = "<!DOCTYPE html><html><head><title>T</title></head><body><p>x</p></body></html>";
        let out = bundle(html, ".a{}", "var z=1", "ignored").unwrap();
        // css before </head>, js before </body>
        assert!(out.contains("<style>"));
        assert!(out.find("<style>").unwrap() < out.find("</head>").unwrap());
        assert!(out.find("var z=1").unwrap() < out.find("</body>").unwrap());
        // original title kept (full doc not re-wrapped)
        assert!(out.contains("<title>T</title>"));
    }

    #[test]
    fn css_only_and_js_only() {
        let css_only = bundle("<p>a</p>", "p{}", "", "").unwrap();
        assert!(css_only.contains("<style>") && !css_only.contains("<script>"));
        let js_only = bundle("<p>a</p>", "", "1+1", "").unwrap();
        assert!(js_only.contains("<script>") && !js_only.contains("<style>"));
    }

    #[test]
    fn title_escaped() {
        let out = bundle("<p>a</p>", "", "", "A & B <x>").unwrap();
        assert!(out.contains("<title>A &amp; B &lt;x&gt;</title>"));
    }

    #[test]
    fn errors_when_all_empty() {
        assert!(bundle("  ", "", "  ", "t").is_err());
    }

    #[test]
    fn fragment_without_head_marker_still_injects_full_doc_path() {
        // A full doc missing </head>: CSS falls back to before </body>.
        let html = "<html><body><p>x</p></body></html>";
        let out = bundle(html, ".c{}", "", "").unwrap();
        assert!(out.contains("<style>"));
        assert!(out.find("<style>").unwrap() < out.find("</body>").unwrap());
    }
}
