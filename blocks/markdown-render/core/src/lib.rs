//! gizza-ai/markdown-render core — pure compute, shared by the chat skill block
//! and the web page. No wafer/wasm-bindgen deps. Renders Markdown (CommonMark +
//! GitHub-flavored extensions: tables, task lists, strikethrough, footnotes) into
//! HTML via `pulldown-cmark`, then sanitizes the result with `ammonia` so the
//! output is safe to inject into a page (no `<script>`, no event handlers, no
//! `javascript:` URLs).

use ammonia::Builder;
use pulldown_cmark::{html, Options, Parser};

/// Render `markdown` into sanitized HTML.
///
/// GitHub-flavored extensions enabled: tables, task lists, strikethrough,
/// footnotes, smart punctuation. The raw HTML is then run through `ammonia` to
/// strip anything unsafe (scripts, event handlers, dangerous URL schemes) while
/// keeping the formatting elements a Markdown document produces — including the
/// `<input type=checkbox disabled>` items task lists generate.
///
/// Errors as a human-readable string if the input is empty.
pub fn render(markdown: &str) -> Result<String, String> {
    if markdown.trim().is_empty() {
        return Err("input is empty".into());
    }

    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_SMART_PUNCTUATION);
    options.insert(Options::ENABLE_HEADING_ATTRIBUTES);

    let parser = Parser::new_ext(markdown, options);
    let mut unsafe_html = String::new();
    html::push_html(&mut unsafe_html, parser);

    Ok(sanitize(&unsafe_html))
}

/// Sanitize rendered HTML, keeping the elements/attributes a Markdown document
/// legitimately produces (notably the disabled checkboxes of task lists and the
/// `class`/`id` attributes pulldown-cmark emits for code blocks and footnotes).
fn sanitize(html: &str) -> String {
    // Task lists render `<input type=checkbox disabled>`; allow input + the
    // checkbox attributes (ammonia drops `<input>` by default).
    Builder::default()
        .add_tags(["input"])
        .add_tag_attributes("input", ["type", "checked", "disabled"])
        // pulldown-cmark tags code fences as `<code class="language-rust">` and
        // emits `id`/`class` for footnote refs/defs and heading attributes.
        .add_generic_attributes(["class", "id"])
        .clean(html)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headings_and_emphasis() {
        let out = render("# Title\n\nHello **world** and *italics*.").unwrap();
        assert!(out.contains("<h1>Title</h1>"), "got: {out}");
        assert!(out.contains("<strong>world</strong>"), "got: {out}");
        assert!(out.contains("<em>italics</em>"), "got: {out}");
    }

    #[test]
    fn tables_supported() {
        let md = "| a | b |\n|---|---|\n| 1 | 2 |";
        let out = render(md).unwrap();
        assert!(out.contains("<table>"), "got: {out}");
        assert!(out.contains("<th>a</th>"), "got: {out}");
        assert!(out.contains("<td>1</td>"), "got: {out}");
    }

    #[test]
    fn code_blocks_supported() {
        let md = "```rust\nlet x = 1;\n```";
        let out = render(md).unwrap();
        assert!(out.contains("<pre>"), "got: {out}");
        assert!(out.contains("language-rust"), "got: {out}");
        assert!(out.contains("let x = 1;"), "got: {out}");
    }

    #[test]
    fn task_lists_supported() {
        let md = "- [x] done\n- [ ] todo";
        let out = render(md).unwrap();
        assert!(out.contains("type=\"checkbox\""), "got: {out}");
        assert!(out.contains("checked"), "got: {out}");
        assert!(out.contains("disabled"), "got: {out}");
    }

    #[test]
    fn links_preserved() {
        let out = render("[link](https://x.test)").unwrap();
        assert!(out.contains(r#"href="https://x.test""#), "got: {out}");
    }

    #[test]
    fn scripts_are_sanitized() {
        let out = render("Hi <script>alert(1)</script> there").unwrap();
        assert!(!out.contains("<script"), "script must be stripped: {out}");
        assert!(!out.contains("alert(1)"), "script body must be stripped: {out}");
    }

    #[test]
    fn javascript_urls_sanitized() {
        let out = render("[x](javascript:alert(1))").unwrap();
        assert!(!out.contains("javascript:"), "js url must be stripped: {out}");
    }

    #[test]
    fn strikethrough_supported() {
        let out = render("~~gone~~").unwrap();
        assert!(out.contains("<del>gone</del>"), "got: {out}");
    }

    #[test]
    fn empty_input_errors() {
        assert!(render("   ").is_err());
    }
}
