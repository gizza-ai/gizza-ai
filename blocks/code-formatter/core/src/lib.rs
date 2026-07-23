//! gizza-ai/code-formatter core — one beautifier for HTML, CSS, JavaScript and
//! JSON. Pick the language (or let it auto-detect) and it re-indents the snippet
//! with the indentation you choose. Pure-Rust:
//! - **JavaScript** reuses the token-aware `js-beautify` pretty-printer.
//! - **JSON** is parsed + re-serialized with serde_json (key order preserved,
//!   input validated).
//! - **HTML** and **CSS** use small forgiving tokenizers defined here.
//!
//! Nothing is renamed, reordered or dropped for JS/HTML/CSS — only whitespace
//! and line breaks change. JSON is re-serialized from the parsed value.

use gizza_ai_js_beautify_core::{beautify_with, IndentChar};
use serde::Serialize;
use serde_json::Value;

/// The four languages this tool can beautify.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Language {
    Html,
    Css,
    JavaScript,
    Json,
}

/// One indentation unit: N spaces per level, or a single tab.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Indent {
    /// `n` spaces per level (clamped 1..=8).
    Spaces(usize),
    /// A single tab per level.
    Tab,
}

impl Indent {
    /// The indent unit as a string (`"  "`, `"\t"`, …).
    fn unit(self) -> String {
        match self {
            Indent::Tab => "\t".to_string(),
            Indent::Spaces(n) => " ".repeat(n.clamp(1, 8)),
        }
    }
}

/// Parse a language name (`"html"`, `"css"`, `"javascript"`/`"js"`, `"json"`).
/// Returns `None` for anything else (including `"auto"` — resolve that with
/// [`detect_language`] first).
pub fn parse_language(name: &str) -> Option<Language> {
    match name.trim().to_ascii_lowercase().as_str() {
        "html" => Some(Language::Html),
        "css" => Some(Language::Css),
        "javascript" | "js" => Some(Language::JavaScript),
        "json" => Some(Language::Json),
        _ => None,
    }
}

/// Best-effort language detection for `auto`. HTML (leading `<`) and valid JSON
/// are recognised reliably; CSS vs JavaScript is a heuristic. When in doubt it
/// falls back to JavaScript.
pub fn detect_language(src: &str) -> Language {
    let t = src.trim_start();
    if t.starts_with('<') {
        return Language::Html;
    }
    // Whole input parses as JSON → treat it as JSON.
    if serde_json::from_str::<Value>(src.trim()).is_ok() {
        return Language::Json;
    }
    if looks_like_css(src) {
        Language::Css
    } else {
        Language::JavaScript
    }
}

/// Heuristic: does this look more like CSS than JavaScript? CSS has `{ … }`
/// blocks whose bodies are `prop: value;` declarations and no JS-only tokens.
fn looks_like_css(src: &str) -> bool {
    let open = match src.find('{') {
        Some(i) => i,
        None => return false,
    };
    // JS-only markers anywhere → not CSS.
    for marker in ["=>", "function", "===", "var ", "let ", "const ", "return "] {
        if src.contains(marker) {
            return false;
        }
    }
    // The first block body should contain a `prop: value` declaration. The
    // property name of a CSS declaration is letters, digits, and `-`/`_` only
    // (a trailing `;` is optional for a single declaration), which rules out
    // most JS `key: value` object entries with expression keys.
    let after = &src[open + 1..];
    let body_end = after.find('}').unwrap_or(after.len());
    let body = &after[..body_end];
    let colon = match body.find(':') {
        Some(i) => i,
        None => return false,
    };
    let key = body[..colon].trim();
    !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Beautify `src` in the given `lang` with the given `indent`. Returns an error
/// on empty input, or on invalid JSON when `lang` is [`Language::Json`].
pub fn format(src: &str, lang: Language, indent: Indent) -> Result<String, String> {
    if src.trim().is_empty() {
        return Err("no input to format".into());
    }
    match lang {
        Language::Json => format_json(src, &indent.unit()),
        Language::Css => Ok(format_css(src, &indent.unit())),
        Language::Html => format_html(src, &indent.unit()),
        Language::JavaScript => {
            let ch = match indent {
                Indent::Tab => IndentChar::Tab,
                Indent::Spaces(_) => IndentChar::Space,
            };
            let n = match indent {
                Indent::Spaces(n) => n,
                Indent::Tab => 2,
            };
            beautify_with(src, n, ch)
        }
    }
}

// ---------------------------------------------------------------------------
// JSON
// ---------------------------------------------------------------------------

fn format_json(src: &str, unit: &str) -> Result<String, String> {
    let value: Value = serde_json::from_str(src).map_err(|e| format!("invalid JSON: {e}"))?;
    let pad = unit.as_bytes();
    let mut buf = Vec::new();
    let fmt = serde_json::ser::PrettyFormatter::with_indent(pad);
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, fmt);
    value
        .serialize(&mut ser)
        .map_err(|e| format!("serialize failed: {e}"))?;
    String::from_utf8(buf).map_err(|e| format!("utf8: {e}"))
}

// ---------------------------------------------------------------------------
// CSS
// ---------------------------------------------------------------------------

fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Normalise a selector: collapse whitespace and tidy `,` spacing.
fn tidy_selector(sel: &str) -> String {
    collapse_ws(sel).replace(" ,", ",").replace(',', ", ").replace(",  ", ", ")
}

/// Emit one declaration (`prop: value;` or a bare at-rule like `@import "x";`),
/// normalising the `prop: value` spacing when a colon is present.
fn flush_decl(out: &mut String, buf: &mut String, depth: usize, unit: &str) {
    let decl = buf.trim();
    if !decl.is_empty() {
        for _ in 0..depth {
            out.push_str(unit);
        }
        let body = decl.trim_end_matches(';');
        if let Some(colon) = body.find(':') {
            let (prop, val) = body.split_at(colon);
            let val = &val[1..];
            out.push_str(&format!("{}: {};\n", collapse_ws(prop), collapse_ws(val)));
        } else {
            out.push_str(&collapse_ws(body));
            out.push_str(";\n");
        }
    }
    buf.clear();
}

/// Pretty-print CSS with `unit` per nesting level. Forgiving: preserves string
/// literals and `/* comments */`, handles nested at-rules (`@media { … }`), and
/// never reorders declarations.
pub fn format_css(src: &str, unit: &str) -> String {
    let chars: Vec<char> = src.chars().collect();
    let n = chars.len();
    let mut out = String::new();
    let mut buf = String::new();
    let mut depth: usize = 0;
    let mut i = 0;

    while i < n {
        let c = chars[i];
        // Comment: /* ... */
        if c == '/' && i + 1 < n && chars[i + 1] == '*' {
            let mut j = i + 2;
            while j + 1 < n && !(chars[j] == '*' && chars[j + 1] == '/') {
                j += 1;
            }
            let end = (j + 2).min(n);
            let comment: String = chars[i..end].iter().collect();
            if buf.trim().is_empty() {
                for _ in 0..depth {
                    out.push_str(unit);
                }
                out.push_str(comment.trim());
                out.push('\n');
                buf.clear();
            } else {
                buf.push(' ');
                buf.push_str(comment.trim());
            }
            i = end;
            continue;
        }
        // String literal — copy verbatim.
        if c == '"' || c == '\'' {
            buf.push(c);
            i += 1;
            while i < n {
                buf.push(chars[i]);
                if chars[i] == c {
                    i += 1;
                    break;
                }
                i += 1;
            }
            continue;
        }
        match c {
            '{' => {
                let sel = tidy_selector(buf.trim());
                for _ in 0..depth {
                    out.push_str(unit);
                }
                out.push_str(&sel);
                out.push_str(" {\n");
                depth += 1;
                buf.clear();
                i += 1;
            }
            '}' => {
                flush_decl(&mut out, &mut buf, depth, unit);
                depth = depth.saturating_sub(1);
                for _ in 0..depth {
                    out.push_str(unit);
                }
                out.push_str("}\n");
                buf.clear();
                i += 1;
            }
            ';' => {
                buf.push(';');
                flush_decl(&mut out, &mut buf, depth, unit);
                i += 1;
            }
            _ => {
                buf.push(c);
                i += 1;
            }
        }
    }
    // Trailing declaration with no closing `;`.
    flush_decl(&mut out, &mut buf, depth, unit);
    out.trim_end().to_string() + "\n"
}

// ---------------------------------------------------------------------------
// HTML
// ---------------------------------------------------------------------------

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

/// Pretty-print `html` with `unit` per nesting level. Forgiving tokenizer (HTML
/// is not well-formed XML): understands void/self-closing tags, comments,
/// doctype and quoted attributes, and preserves `pre`/`textarea`/`script`/`style`
/// contents verbatim.
pub fn format_html(html: &str, unit: &str) -> Result<String, String> {
    if html.trim().is_empty() {
        return Err("no HTML input".into());
    }
    let b = html.as_bytes();
    let lower = html.to_ascii_lowercase();
    let n = b.len();
    let mut i = 0usize;
    let mut depth: usize = 0;
    let mut out = String::new();

    let line = |out: &mut String, depth: usize, s: &str| {
        for _ in 0..depth {
            out.push_str(unit);
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
    fn json_pretty_with_two_spaces() {
        let out = format(r#"{"b":1,"a":[1,2]}"#, Language::Json, Indent::Spaces(2)).unwrap();
        assert_eq!(out, "{\n  \"b\": 1,\n  \"a\": [\n    1,\n    2\n  ]\n}");
    }

    #[test]
    fn json_preserves_key_order() {
        let out = format(r#"{"z":1,"a":2}"#, Language::Json, Indent::Spaces(2)).unwrap();
        assert!(out.find("\"z\"").unwrap() < out.find("\"a\"").unwrap());
    }

    #[test]
    fn json_tab_indent() {
        let out = format(r#"{"a":1}"#, Language::Json, Indent::Tab).unwrap();
        assert_eq!(out, "{\n\t\"a\": 1\n}");
    }

    #[test]
    fn json_invalid_errors() {
        let err = format("{not json}", Language::Json, Indent::Spaces(2)).unwrap_err();
        assert!(err.contains("invalid JSON"));
    }

    #[test]
    fn css_basic_declarations() {
        let out = format("a{color:red;margin:0}", Language::Css, Indent::Spaces(2)).unwrap();
        assert_eq!(out, "a {\n  color: red;\n  margin: 0;\n}\n");
    }

    #[test]
    fn css_nested_at_rule() {
        let out = format(
            "@media screen{.a{color:red}}",
            Language::Css,
            Indent::Spaces(2),
        )
        .unwrap();
        assert_eq!(out, "@media screen {\n  .a {\n    color: red;\n  }\n}\n");
    }

    #[test]
    fn css_preserves_string_with_semicolon() {
        let out = format("a{content:\"a;b\"}", Language::Css, Indent::Spaces(2)).unwrap();
        assert_eq!(out, "a {\n  content: \"a;b\";\n}\n");
    }

    #[test]
    fn css_tab_indent() {
        let out = format("a{color:red}", Language::Css, Indent::Tab).unwrap();
        assert_eq!(out, "a {\n\tcolor: red;\n}\n");
    }

    #[test]
    fn html_nests_block_elements() {
        let out = format("<div><p>hi</p></div>", Language::Html, Indent::Spaces(2)).unwrap();
        assert_eq!(out, "<div>\n  <p>\n    hi\n  </p>\n</div>\n");
    }

    #[test]
    fn html_tab_indent() {
        let out = format("<div><p>hi</p></div>", Language::Html, Indent::Tab).unwrap();
        assert_eq!(out, "<div>\n\t<p>\n\t\thi\n\t</p>\n</div>\n");
    }

    #[test]
    fn javascript_reindents() {
        let out =
            format("function f(){return 1;}", Language::JavaScript, Indent::Spaces(2)).unwrap();
        assert!(out.contains("function f() {"));
        assert!(out.contains("  return 1;"));
    }

    #[test]
    fn empty_input_errors() {
        assert!(format("   ", Language::Css, Indent::Spaces(2)).is_err());
        assert!(format("   ", Language::JavaScript, Indent::Spaces(2)).is_err());
    }

    #[test]
    fn detect_html_json_css_js() {
        assert_eq!(detect_language("<div>x</div>"), Language::Html);
        assert_eq!(detect_language(r#"{"a":1}"#), Language::Json);
        assert_eq!(detect_language("a{color:red}"), Language::Css);
        assert_eq!(
            detect_language("function f(){return 1}"),
            Language::JavaScript
        );
    }

    #[test]
    fn parse_language_aliases() {
        assert_eq!(parse_language("JS"), Some(Language::JavaScript));
        assert_eq!(parse_language("JSON"), Some(Language::Json));
        assert_eq!(parse_language("auto"), None);
    }
}
