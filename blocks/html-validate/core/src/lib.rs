//! html-validate core — pure compute, shared by the chat skill block and the web
//! page. No wafer/wasm-bindgen deps (JSON is hand-built, no serde).
//!
//! A forgiving, dependency-free HTML scanner (HTML is not well-formed XML) that
//! reports the three problem classes a hand-check misses:
//!   * **syntax errors** — an unterminated tag (`<div` with no `>`), an
//!     unterminated comment (`<!--` with no `-->`), or a tag with no name (`< >`);
//!   * **unclosed tags** — an element opened but never closed by end of input;
//!   * **nesting issues** — a closing tag that crosses another still-open element
//!     (`<b><i></b>`) or a stray closing tag with no matching open (`</span>`).
//!
//! It understands void elements (never closed), self-closing tags, comments,
//! doctype/declarations, quoted attribute values (so `>` inside an attribute is
//! safe), and preserves the verbatim contents of `script`/`style`/`textarea`/`pre`
//! (a `<` inside JS or text is not a tag). Every issue carries a 1-based line and
//! column so it can be located in the source.

/// HTML void elements — they never have a closing tag, so they are not pushed on
/// the open-element stack and a `</void>` closing tag is flagged as a warning.
const VOID: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

/// Elements whose contents are CDATA/raw text — a `<` inside them is literal, not
/// a tag — so the scanner skips straight to the matching close tag.
const RAW: &[&str] = &["script", "style", "textarea", "pre"];

/// One reported problem.
#[derive(Clone, PartialEq, Eq)]
pub struct Issue {
    /// "error" or "warning".
    pub severity: &'static str,
    /// 1-based line of the offending token.
    pub line: usize,
    /// 1-based column of the offending token.
    pub column: usize,
    /// Human-readable explanation of what was expected.
    pub message: String,
}

/// Full validation result.
pub struct Report {
    pub issues: Vec<Issue>,
    /// Count of open/close tag pairs and void/self-closing tags seen.
    pub elements: usize,
    /// true when there are no error-severity issues.
    pub valid: bool,
}

/// Output rendering mode.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Report,
    Json,
}

/// Parse the `format` argument (`report` | `json`; blank → report).
pub fn parse_format(s: &str) -> Result<Format, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "report" | "text" => Ok(Format::Report),
        "json" => Ok(Format::Json),
        other => Err(format!(
            "invalid format {other:?}: expected 'report' or 'json'"
        )),
    }
}

/// The lowercase tag name that begins at `raw` (`<div ...>` / `</div>` → "div").
fn tag_name(raw: &str) -> String {
    raw.trim_start_matches('<')
        .trim_start_matches('/')
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == ':')
        .collect::<String>()
        .to_ascii_lowercase()
}

/// Index just past the tag's closing `>`, respecting quoted attribute values, or
/// `None` if the tag is never terminated before end of input.
fn scan_tag(b: &[u8], start: usize) -> Option<usize> {
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
            return Some(j + 1);
        }
        j += 1;
    }
    None
}

/// 1-based (line, column) of byte offset `off` within `b`.
fn line_col(b: &[u8], off: usize) -> (usize, usize) {
    let mut line = 1usize;
    let mut col = 1usize;
    for &c in &b[..off.min(b.len())] {
        if c == b'\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

/// An element currently open, awaiting its close tag.
struct Open {
    name: String,
    line: usize,
    column: usize,
}

/// Validate `html`, returning every syntax error, unclosed tag, and nesting issue.
pub fn validate(html: &str) -> Result<Report, String> {
    if html.trim().is_empty() {
        return Err("input is empty: paste an HTML document or snippet to validate".into());
    }
    let b = html.as_bytes();
    let lower = html.to_ascii_lowercase();
    let n = b.len();
    let mut i = 0usize;
    let mut stack: Vec<Open> = Vec::new();
    let mut issues: Vec<Issue> = Vec::new();
    let mut elements = 0usize;

    let err = |issues: &mut Vec<Issue>, off: usize, message: String| {
        let (line, column) = line_col(b, off);
        issues.push(Issue {
            severity: "error",
            line,
            column,
            message,
        });
    };

    while i < n {
        if b[i] != b'<' {
            i += 1;
            continue;
        }
        // A `<` not starting a tag (`a < b`, `<3`) is literal text in HTML.
        let next = b.get(i + 1).copied();
        let starts_tag = matches!(next, Some(c) if c.is_ascii_alphabetic() || c == b'/' || c == b'!' || c == b'?');
        if !starts_tag {
            i += 1;
            continue;
        }

        // Comment: <!-- ... -->
        if html[i..].starts_with("<!--") {
            match html[i + 4..].find("-->") {
                Some(p) => i = i + 4 + p + 3,
                None => {
                    err(
                        &mut issues,
                        i,
                        "unterminated comment: '<!--' is never closed with '-->'".into(),
                    );
                    break;
                }
            }
            continue;
        }
        // Doctype / declaration / processing instruction: <!...> or <?...>
        if next == Some(b'!') || next == Some(b'?') {
            match scan_tag(b, i) {
                Some(end) => i = end,
                None => {
                    err(
                        &mut issues,
                        i,
                        "unterminated declaration: no closing '>' before end of input".into(),
                    );
                    break;
                }
            }
            continue;
        }

        // A start / end / self-closing tag.
        let end = match scan_tag(b, i) {
            Some(end) => end,
            None => {
                err(
                    &mut issues,
                    i,
                    "unterminated tag: no closing '>' before end of input".into(),
                );
                break;
            }
        };
        let raw = &html[i..end];
        let is_close = next == Some(b'/');
        let name = tag_name(raw);
        let self_closing = raw.trim_end().ends_with("/>");
        let (line, column) = line_col(b, i);

        if name.is_empty() {
            let what = if is_close { "closing tag" } else { "tag" };
            let lead = if is_close { "/" } else { "" };
            err(
                &mut issues,
                i,
                format!("malformed {what}: '<{lead}' has no tag name"),
            );
            i = end;
            continue;
        }

        if is_close {
            if VOID.contains(&name.as_str()) {
                issues.push(Issue {
                    severity: "warning",
                    line,
                    column,
                    message: format!(
                        "`{name}` is a void element and must not have a closing tag `</{name}>`"
                    ),
                });
                i = end;
                continue;
            }
            match stack.iter().rposition(|o| o.name == name) {
                Some(pos) => {
                    // Everything opened after `pos` crosses this close tag → unclosed.
                    for o in stack.drain(pos + 1..) {
                        err(
                            &mut issues,
                            i,
                            format!(
                                "`<{}>` (opened at line {}:{}) is not closed before `</{name}>` — overlapping/misnested tags",
                                o.name, o.line, o.column
                            ),
                        );
                    }
                    stack.pop(); // pop the matched `name`
                    elements += 1;
                }
                None => err(
                    &mut issues,
                    i,
                    format!("unexpected closing tag `</{name}>`: no matching opening `<{name}>`"),
                ),
            }
            i = end;
            continue;
        }

        // Opening (or void / self-closing) tag.
        if VOID.contains(&name.as_str()) || self_closing {
            elements += 1;
            i = end;
            continue;
        }

        // Raw-text element: skip its verbatim contents to the matching close tag.
        if RAW.contains(&name.as_str()) {
            let close_pat = format!("</{name}");
            match lower[end..].find(&close_pat) {
                Some(p) => {
                    let close_lt = end + p;
                    match scan_tag(b, close_lt) {
                        Some(close_end) => {
                            elements += 1;
                            i = close_end;
                        }
                        None => {
                            err(
                                &mut issues,
                                close_lt,
                                "unterminated tag: no closing '>' before end of input".into(),
                            );
                            i = n;
                        }
                    }
                }
                None => {
                    err(
                        &mut issues,
                        i,
                        format!("`<{name}>` (opened at line {line}:{column}) is never closed with `</{name}>`"),
                    );
                    i = n;
                }
            }
            continue;
        }

        stack.push(Open { name, line, column });
        i = end;
    }

    // Anything left open at end of input is unclosed.
    for o in stack.drain(..) {
        issues.push(Issue {
            severity: "error",
            line: o.line,
            column: o.column,
            message: format!("`<{}>` is never closed with `</{}>`", o.name, o.name),
        });
    }

    // Deterministic, source-order presentation.
    issues.sort_by_key(|it| (it.line, it.column));
    let valid = !issues.iter().any(|it| it.severity == "error");
    Ok(Report {
        issues,
        elements,
        valid,
    })
}

/// Escape a string for inclusion in a JSON document.
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// Render a report as human-readable text.
fn render_text(r: &Report) -> String {
    let errors = r.issues.iter().filter(|i| i.severity == "error").count();
    let warnings = r.issues.len() - errors;
    let mut out = String::new();
    if r.valid && warnings == 0 {
        out.push_str("Valid HTML — no syntax errors, unclosed tags, or nesting issues found.");
        if r.elements > 0 {
            out.push_str(&format!("\nChecked {} element(s).", r.elements));
        }
        return out;
    }
    out.push_str(&format!(
        "{}: {} error(s), {} warning(s) in {} element(s).\n",
        if r.valid {
            "HTML has warnings"
        } else {
            "Invalid HTML"
        },
        errors,
        warnings,
        r.elements
    ));
    for it in &r.issues {
        out.push_str(&format!(
            "\n  {:<7} line {}:{}  {}",
            it.severity, it.line, it.column, it.message
        ));
    }
    out
}

/// Render a report as a machine-readable JSON object.
fn render_json(r: &Report) -> String {
    let errors = r.issues.iter().filter(|i| i.severity == "error").count();
    let warnings = r.issues.len() - errors;
    let mut out = String::new();
    out.push_str(&format!(
        "{{\"valid\":{},\"errors\":{},\"warnings\":{},\"elements\":{},\"issues\":[",
        r.valid, errors, warnings, r.elements
    ));
    for (k, it) in r.issues.iter().enumerate() {
        if k > 0 {
            out.push(',');
        }
        out.push_str(&format!(
            "{{\"severity\":\"{}\",\"line\":{},\"column\":{},\"message\":\"{}\"}}",
            it.severity,
            it.line,
            it.column,
            json_escape(&it.message)
        ));
    }
    out.push_str("]}");
    out
}

/// Validate `html` and render it as `format` (`report` text or `json`).
pub fn run(html: &str, format: &str) -> Result<String, String> {
    let fmt = parse_format(format)?;
    let report = validate(html)?;
    Ok(match fmt {
        Format::Report => render_text(&report),
        Format::Json => render_json(&report),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_document_reports_no_issues() {
        let r = validate("<html><head><title>Hi</title></head><body><p>Hello</p></body></html>")
            .unwrap();
        assert!(r.valid);
        assert!(r.issues.is_empty());
        assert!(r.elements >= 5);
    }

    #[test]
    fn void_and_self_closing_need_no_close() {
        let r = validate("<div><br><img src=\"a.png\"><hr/></div>").unwrap();
        assert!(
            r.valid,
            "{:?}",
            r.issues.iter().map(|i| &i.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn unclosed_tag_is_flagged() {
        let r = validate("<div><p>text").unwrap();
        assert!(!r.valid);
        assert!(r
            .issues
            .iter()
            .any(|i| i.message.contains("<p>") && i.message.contains("never closed")));
        assert!(r
            .issues
            .iter()
            .any(|i| i.message.contains("<div>") && i.message.contains("never closed")));
    }

    #[test]
    fn misnested_tags_are_flagged() {
        let r = validate("<b><i>hi</b></i>").unwrap();
        assert!(!r.valid);
        assert!(r
            .issues
            .iter()
            .any(|i| i.message.contains("misnested") || i.message.contains("overlapping")));
    }

    #[test]
    fn stray_close_tag_is_flagged() {
        let r = validate("<p>hi</p></span>").unwrap();
        assert!(!r.valid);
        assert!(
            r.issues
                .iter()
                .any(|i| i.message.contains("unexpected closing tag")
                    && i.message.contains("</span>"))
        );
    }

    #[test]
    fn unterminated_comment_is_error() {
        let r = validate("<p>hi</p><!-- oops").unwrap();
        assert!(!r.valid);
        assert!(r
            .issues
            .iter()
            .any(|i| i.message.contains("unterminated comment")));
    }

    #[test]
    fn unterminated_tag_is_error() {
        let r = validate("<div class=\"x\"").unwrap();
        assert!(!r.valid);
        assert!(r
            .issues
            .iter()
            .any(|i| i.message.contains("unterminated tag")));
    }

    #[test]
    fn script_content_is_not_parsed_as_tags() {
        let r = validate("<div><script>if (a < b && c > d) {}</script></div>").unwrap();
        assert!(
            r.valid,
            "{:?}",
            r.issues.iter().map(|i| &i.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn less_than_in_text_is_not_a_tag() {
        let r = validate("<p>1 < 2 and 3 > 2</p>").unwrap();
        assert!(
            r.valid,
            "{:?}",
            r.issues.iter().map(|i| &i.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn closing_void_element_warns() {
        let r = validate("<div><br></br></div>").unwrap();
        assert!(r.valid, "void close is only a warning");
        assert!(r
            .issues
            .iter()
            .any(|i| i.severity == "warning" && i.message.contains("void element")));
    }

    #[test]
    fn json_output_is_well_formed() {
        let out = run("<div><p>hi", "json").unwrap();
        assert!(out.starts_with("{\"valid\":false"));
        assert!(out.contains("\"issues\":["));
        assert!(out.contains("never closed"));
    }

    #[test]
    fn empty_input_errors() {
        assert!(run("   ", "report").is_err());
    }

    #[test]
    fn invalid_format_errors() {
        assert!(run("<p></p>", "xml").is_err());
    }

    #[test]
    fn line_and_column_are_reported() {
        let r = validate("<section>\n  <div>text").unwrap();
        // <div> opened at line 2, column 3 is never closed — the issue anchors to
        // the open position.
        assert!(r
            .issues
            .iter()
            .any(|i| i.line == 2 && i.column == 3 && i.message.contains("<div>")));
    }
}
