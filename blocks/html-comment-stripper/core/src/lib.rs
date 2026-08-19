//! gizza-ai/html-comment-stripper — remove `<!-- … -->` comments from markup
//! while leaving every other byte exactly where it was.
//!
//! The contract that makes this different from a minifier: **the output is the
//! input minus the comment bytes**. No whitespace collapsing, no tag
//! normalization, no attribute rewriting, no re-indentation.
//!
//! HTML is not well-formed XML, so this is a forgiving hand-rolled scanner
//! rather than a parser (the same conclusion `html-formatter`/`html-minifier`
//! reached). What it gets right that a `<!--.*?-->` regex cannot:
//!
//! * **Raw-text elements.** Inside `<script>`, `<style>`, `<textarea>` and
//!   `<title>` the string `<!--` is *not* a comment — the HTML tokenizer is in
//!   a raw-text state there. Those regions are copied through verbatim (except
//!   for the opt-in CSS pass inside `<style>`).
//! * **Quoted attribute values.** `<a title="<!-- hi -->">` contains no
//!   comment; the tag scanner skips quoted runs so a `>` or a `<!--` inside an
//!   attribute cannot start or end anything.
//! * **Comments do not nest.** Per the parser, `<!-- a <!-- b --> c -->` ends
//!   at the FIRST `-->`; ` c -->` is text. We match that rather than inventing
//!   a nesting rule.
//! * **Unterminated comments** are an error, not a silent truncation of the
//!   rest of the document.
//!
//! Conditional (`<!--[if …]>`), SSI (`<!--#include …-->`) and bang
//! (`<!--! …-->`) comments are recognized as kinds and kept by default, and an
//! arbitrary keep-list / remove-only-list is expressible with one regex.

use regex::Regex;

/// Largest accepted input, in bytes.
pub const MAX_INPUT_BYTES: usize = 5_000_000;

/// Raw-text / escapable-raw-text elements: `<!--` inside them is not a comment.
/// `<pre>` is deliberately absent — its content is ordinary element content, so
/// a comment inside a `<pre>` really is a comment.
const RAW_TEXT: [&str; 4] = ["script", "style", "textarea", "title"];

/// What a comment turned out to be, for keep-rules and the report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// `<!--[if lt IE 9]> … <![endif]-->` and the split downlevel-revealed forms.
    Conditional,
    /// `<!--#include virtual="…" -->` and friends — a server-side directive.
    Ssi,
    /// `<!--! … -->` — the conventional "important"/licence banner marker.
    Bang,
    /// Anything else.
    Plain,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Conditional => "conditional",
            Kind::Ssi => "ssi",
            Kind::Bang => "bang",
            Kind::Plain => "plain",
        }
    }
}

/// One comment found in the input.
#[derive(Debug, Clone)]
pub struct Comment {
    /// 1-based line number of the opening `<!--`.
    pub line: usize,
    pub kind: Kind,
    /// The comment including its `<!--`/`-->` delimiters.
    pub raw: String,
    /// The text between the delimiters.
    pub inner: String,
    pub removed: bool,
}

fn classify(inner: &str) -> Kind {
    let t = inner.trim_start();
    if t.starts_with("[if") || t.starts_with("<![endif]") || inner.trim_end().ends_with("<![endif]")
    {
        return Kind::Conditional;
    }
    if t.starts_with('#') {
        return Kind::Ssi;
    }
    if t.starts_with('!') {
        return Kind::Bang;
    }
    Kind::Plain
}

/// Lowercased tag name of a tag starting at `<`, or "" if it isn't a normal tag.
fn tag_name(s: &str) -> String {
    let rest = s
        .strip_prefix("</")
        .or_else(|| s.strip_prefix('<'))
        .unwrap_or(s);
    rest.chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect::<String>()
        .to_ascii_lowercase()
}

/// Byte index just past the `>` that closes the tag starting at `i`, skipping
/// quoted attribute values so a `>` inside one doesn't end the tag early.
fn scan_tag(b: &[u8], i: usize) -> usize {
    let n = b.len();
    let mut j = i + 1;
    let mut quote = 0u8;
    while j < n {
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
    n
}

/// Strip `/* … */` comments from a CSS fragment, skipping string literals so a
/// `/*` inside `content: "/*"` is left alone. Returns the cleaned CSS and the
/// number of comments removed.
fn strip_css_comments(css: &str) -> (String, usize) {
    let b = css.as_bytes();
    let n = b.len();
    let mut out = String::with_capacity(n);
    let mut i = 0usize;
    let mut removed = 0usize;
    // Advance past one whole UTF-8 char starting at `p`.
    let next = |p: usize| {
        let mut j = p + 1;
        while j < n && (b[j] & 0xC0) == 0x80 {
            j += 1;
        }
        j
    };
    while i < n {
        let c = b[i];
        if c == b'"' || c == b'\'' {
            let quote = c;
            out.push(c as char);
            i += 1;
            while i < n {
                let d = b[i];
                let j = next(i);
                out.push_str(&css[i..j]);
                i = j;
                if d == b'\\' && i < n {
                    let k = next(i);
                    out.push_str(&css[i..k]);
                    i = k;
                } else if d == quote {
                    break;
                }
            }
            continue;
        }
        if c == b'/' && i + 1 < n && b[i + 1] == b'*' {
            match css[i + 2..].find("*/") {
                Some(p) => i = i + 2 + p + 2,
                None => i = n,
            }
            removed += 1;
            continue;
        }
        // Copy one char (multi-byte safe: advance to the next char boundary).
        let j = next(i);
        out.push_str(&css[i..j]);
        i = j;
    }
    (out, removed)
}

/// Should this comment be removed?
fn should_remove(
    kind: Kind,
    inner: &str,
    keep_conditional: bool,
    keep_ssi: bool,
    keep_bang: bool,
    re: Option<&Regex>,
    pattern_only: bool,
) -> bool {
    if let Some(re) = re {
        let hit = re.is_match(inner);
        if pattern_only {
            // Only comments matching the pattern go, whatever kind they are.
            return hit;
        }
        if hit {
            return false; // keep-list wins over everything below
        }
    }
    match kind {
        Kind::Conditional => !keep_conditional,
        Kind::Ssi => !keep_ssi,
        Kind::Bang => !keep_bang,
        Kind::Plain => true,
    }
}

/// Drop lines that became blank, and (when `collapse`) fold runs of blank lines
/// into one. `blanked` holds the 0-based indices of output lines that were made
/// whitespace-only by a removal — a line that was already blank in the input is
/// left alone by `trim`.
fn tidy_lines(text: &str, blanked: &[usize], collapse: bool) -> String {
    let had_trailing_newline = text.ends_with('\n');
    let mut kept: Vec<&str> = Vec::new();
    for (idx, line) in text.split('\n').enumerate() {
        let is_blanked = blanked.binary_search(&idx).is_ok() && line.trim().is_empty();
        if is_blanked {
            continue;
        }
        if collapse
            && line.trim().is_empty()
            && kept.last().map(|l| l.trim().is_empty()).unwrap_or(false)
        {
            continue;
        }
        kept.push(line);
    }
    // `split('\n')` yields a trailing "" for text ending in a newline; if that
    // sentinel survived, joining restores the newline. If it was dropped, put
    // the trailing newline back by hand.
    let mut out = kept.join("\n");
    if had_trailing_newline && !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

fn escape_field(s: &str) -> String {
    let flat = s.replace('\r', " ").replace('\n', "\\n");
    if flat.contains(',') || flat.contains('"') {
        format!("\"{}\"", flat.replace('"', "\"\""))
    } else {
        flat
    }
}

/// Remove HTML comments from `html`.
///
/// * `keep_conditional` / `keep_ssi` / `keep_bang` — protect those kinds.
/// * `pattern` — a regex over each comment's INNER text; blank disables it.
/// * `pattern_mode` — `"keep"` (matches are protected) or `"only"` (matches are
///   the ONLY things removed).
/// * `remove_css_comments` — also strip `/* … */` inside `<style>` blocks.
/// * `blank_lines` — `"keep"` | `"trim"` | `"collapse"`.
/// * `output` — `"html"` | `"report"` | `"comments"`.
#[allow(clippy::too_many_arguments)]
pub fn strip(
    html: &str,
    keep_conditional: bool,
    keep_ssi: bool,
    keep_bang: bool,
    pattern: &str,
    pattern_mode: &str,
    remove_css_comments: bool,
    blank_lines: &str,
    output: &str,
) -> Result<String, String> {
    if html.trim().is_empty() {
        return Err("no HTML input".into());
    }
    if html.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input is {} bytes, over the {MAX_INPUT_BYTES}-byte limit",
            html.len()
        ));
    }
    let pattern_only = match pattern_mode.trim() {
        "" | "keep" => false,
        "only" => true,
        other => {
            return Err(format!(
                "unknown pattern_mode '{other}' — use 'keep' or 'only'"
            ))
        }
    };
    let (trim_blank, collapse_blank) = match blank_lines.trim() {
        "" | "keep" => (false, false),
        "trim" => (true, false),
        "collapse" => (true, true),
        other => {
            return Err(format!(
                "unknown blank_lines '{other}' — use 'keep', 'trim' or 'collapse'"
            ))
        }
    };
    if !matches!(output.trim(), "" | "html" | "report" | "comments") {
        return Err(format!(
            "unknown output '{}' — use 'html', 'report' or 'comments'",
            output.trim()
        ));
    }
    let re = if pattern.is_empty() {
        None
    } else {
        Some(Regex::new(pattern).map_err(|e| {
            let msg = e.to_string();
            let detail = msg.lines().last().unwrap_or("").trim();
            format!("invalid pattern: {}", if detail.is_empty() { &msg } else { detail })
        })?)
    };
    if pattern_only && re.is_none() {
        return Err("pattern_mode 'only' needs a pattern — with an empty pattern nothing would be removed".into());
    }

    let b = html.as_bytes();
    let n = b.len();
    let lower = html.to_ascii_lowercase();
    let mut out = String::with_capacity(n);
    let mut found: Vec<Comment> = Vec::new();
    let mut css_removed = 0usize;
    // 0-based indices of OUTPUT lines a removal left whitespace-only.
    let mut touched_lines: Vec<usize> = Vec::new();
    let mut i = 0usize;
    let mut line = 1usize; // 1-based input line of byte `i`

    while i < n {
        if b[i] == b'<' && html[i..].starts_with("<!--") {
            let end = match html[i + 4..].find("-->") {
                Some(p) => i + 4 + p + 3,
                None => {
                    return Err(format!(
                        "unterminated comment: '<!--' at line {line} is never closed with '-->'"
                    ))
                }
            };
            let raw = &html[i..end];
            let inner = &html[i + 4..end - 3];
            let kind = classify(inner);
            let remove = should_remove(
                kind,
                inner,
                keep_conditional,
                keep_ssi,
                keep_bang,
                re.as_ref(),
                pattern_only,
            );
            found.push(Comment {
                line,
                kind,
                raw: raw.to_string(),
                inner: inner.to_string(),
                removed: remove,
            });
            if remove {
                touched_lines.push(out.matches('\n').count());
            } else {
                out.push_str(raw);
            }
            line += raw.matches('\n').count();
            i = end;
            continue;
        }
        if b[i] == b'<' {
            let end = scan_tag(b, i);
            let raw = &html[i..end];
            out.push_str(raw);
            line += raw.matches('\n').count();
            let name = tag_name(raw);
            let is_close = b.get(i + 1) == Some(&b'/');
            let self_closing = raw.trim_end().ends_with("/>");
            i = end;
            // Raw-text element: copy its content through untouched.
            if !is_close && !self_closing && RAW_TEXT.contains(&name.as_str()) {
                let close_pat = format!("</{name}");
                let close_lt = lower[i..].find(&close_pat).map(|p| i + p).unwrap_or(n);
                let body = &html[i..close_lt];
                if remove_css_comments && name == "style" {
                    let (cleaned, cnt) = strip_css_comments(body);
                    css_removed += cnt;
                    out.push_str(&cleaned);
                } else {
                    out.push_str(body);
                }
                line += body.matches('\n').count();
                i = close_lt;
            }
            continue;
        }
        // Text run up to the next '<'.
        let start = i;
        while i < n && b[i] != b'<' {
            i += 1;
        }
        let t = &html[start..i];
        out.push_str(t);
        line += t.matches('\n').count();
    }

    match output.trim() {
        "comments" => {
            if found.is_empty() {
                return Ok("no comments found\n".into());
            }
            let mut s = String::from("line,kind,action,comment\n");
            for c in &found {
                s.push_str(&format!(
                    "{},{},{},{}\n",
                    c.line,
                    c.kind.as_str(),
                    if c.removed { "removed" } else { "kept" },
                    escape_field(&c.raw)
                ));
            }
            Ok(s)
        }
        "report" => {
            let cleaned = if trim_blank {
                touched_lines.sort_unstable();
                touched_lines.dedup();
                tidy_lines(&out, &touched_lines, collapse_blank)
            } else {
                out
            };
            let removed = found.iter().filter(|c| c.removed).count();
            let kept = found.len() - removed;
            let count = |k: Kind| {
                found
                    .iter()
                    .filter(|c| c.kind == k && c.removed)
                    .count()
            };
            let before = html.len();
            let after = cleaned.len();
            let saved = before.saturating_sub(after);
            let pct = if before == 0 {
                0.0
            } else {
                saved as f64 * 100.0 / before as f64
            };
            let mut s = String::from("metric,value\n");
            s.push_str(&format!("comments_found,{}\n", found.len()));
            s.push_str(&format!("comments_removed,{removed}\n"));
            s.push_str(&format!("comments_kept,{kept}\n"));
            s.push_str(&format!("removed_plain,{}\n", count(Kind::Plain)));
            s.push_str(&format!("removed_conditional,{}\n", count(Kind::Conditional)));
            s.push_str(&format!("removed_ssi,{}\n", count(Kind::Ssi)));
            s.push_str(&format!("removed_bang,{}\n", count(Kind::Bang)));
            s.push_str(&format!("css_comments_removed,{css_removed}\n"));
            s.push_str(&format!("bytes_before,{before}\n"));
            s.push_str(&format!("bytes_after,{after}\n"));
            s.push_str(&format!("bytes_saved,{saved}\n"));
            s.push_str(&format!("percent_smaller,{pct:.2}\n"));
            Ok(s)
        }
        _ => {
            if trim_blank {
                touched_lines.sort_unstable();
                touched_lines.dedup();
                Ok(tidy_lines(&out, &touched_lines, collapse_blank))
            } else {
                Ok(out)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(html: &str) -> String {
        strip(html, true, true, true, "", "keep", false, "keep", "html").unwrap()
    }

    #[test]
    fn removes_a_plain_comment_and_leaves_everything_else_byte_identical() {
        assert_eq!(
            plain("<p>a</p>\n  <!-- note -->\n<p>b</p>\n"),
            "<p>a</p>\n  \n<p>b</p>\n"
        );
    }

    #[test]
    fn empty_input_is_an_error() {
        assert_eq!(strip("   ", true, true, true, "", "keep", false, "keep", "html").unwrap_err(), "no HTML input");
    }

    #[test]
    fn unterminated_comment_is_an_error_not_a_truncation() {
        let e = strip("<p>a</p>\n<!-- oops\n<p>b</p>", true, true, true, "", "keep", false, "keep", "html")
            .unwrap_err();
        assert!(e.contains("unterminated comment"), "{e}");
        assert!(e.contains("line 2"), "{e}");
    }

    #[test]
    fn conditional_comments_are_kept_by_default_and_removable() {
        let h = "<!--[if lt IE 9]><script src=s.js></script><![endif]--><p>x</p>";
        assert_eq!(plain(h), h);
        assert_eq!(
            strip(h, false, true, true, "", "keep", false, "keep", "html").unwrap(),
            "<p>x</p>"
        );
    }

    #[test]
    fn ssi_and_bang_comments_are_kept_by_default() {
        let h = "<!--#include virtual=\"/h.html\" --><!--! (c) me --><!-- drop -->x";
        assert_eq!(
            plain(h),
            "<!--#include virtual=\"/h.html\" --><!--! (c) me -->x"
        );
        assert_eq!(
            strip(h, true, false, false, "", "keep", false, "keep", "html").unwrap(),
            "x"
        );
    }

    #[test]
    fn comment_like_text_inside_script_and_style_is_not_a_comment() {
        let h = "<script>var s = \"<!-- not a comment -->\";</script>\n<style>a{color:red}</style>";
        assert_eq!(plain(h), h);
    }

    #[test]
    fn comment_inside_a_quoted_attribute_is_not_a_comment() {
        let h = "<a title=\"<!-- hi -->\" href=\"#\">x</a>";
        assert_eq!(plain(h), h);
    }

    #[test]
    fn comments_do_not_nest_first_close_wins() {
        assert_eq!(plain("<!-- a <!-- b --> c --><p>x</p>"), " c --><p>x</p>");
    }

    #[test]
    fn keep_pattern_protects_matching_comments() {
        let h = "<!-- KEEPME build 7 --><!-- internal todo -->x";
        assert_eq!(
            strip(h, true, true, true, "KEEPME", "keep", false, "keep", "html").unwrap(),
            "<!-- KEEPME build 7 -->x"
        );
    }

    #[test]
    fn only_mode_removes_just_the_matching_comments() {
        let h = "<!-- wp:paragraph --><p>x</p><!-- /wp:paragraph --><!-- keep this -->";
        assert_eq!(
            strip(h, true, true, true, r"^\s*/?wp:", "only", false, "keep", "html").unwrap(),
            "<p>x</p><!-- keep this -->"
        );
    }

    #[test]
    fn only_mode_can_target_a_protected_kind() {
        let h = "<!--[if IE]>a<![endif]--><!--#include x -->y";
        assert_eq!(
            strip(h, true, true, true, r"^\[if", "only", false, "keep", "html").unwrap(),
            "<!--#include x -->y"
        );
    }

    #[test]
    fn only_mode_without_a_pattern_is_an_error() {
        let e = strip("<p><!--x--></p>", true, true, true, "", "only", false, "keep", "html")
            .unwrap_err();
        assert!(e.contains("needs a pattern"), "{e}");
    }

    #[test]
    fn invalid_pattern_is_reported() {
        let e = strip("<p><!--x--></p>", true, true, true, "(", "keep", false, "keep", "html")
            .unwrap_err();
        assert!(e.starts_with("invalid pattern"), "{e}");
    }

    #[test]
    fn css_comments_are_opt_in_and_string_aware() {
        let h = "<style>/* hide */a{color:red}b{content:\"/* keep */\"}</style>";
        assert_eq!(plain(h), h);
        assert_eq!(
            strip(h, true, true, true, "", "keep", true, "keep", "html").unwrap(),
            "<style>a{color:red}b{content:\"/* keep */\"}</style>"
        );
    }

    #[test]
    fn trim_drops_lines_left_blank_but_keeps_pre_existing_blank_lines() {
        let h = "<p>a</p>\n<!-- gone -->\n\n<p>b</p>\n";
        assert_eq!(
            strip(h, true, true, true, "", "keep", false, "trim", "html").unwrap(),
            "<p>a</p>\n\n<p>b</p>\n"
        );
        assert_eq!(
            strip(h, true, true, true, "", "keep", false, "collapse", "html").unwrap(),
            "<p>a</p>\n\n<p>b</p>\n"
        );
    }

    #[test]
    fn collapse_folds_runs_of_blank_lines() {
        let h = "<p>a</p>\n<!-- x -->\n\n\n<p>b</p>\n";
        assert_eq!(
            strip(h, true, true, true, "", "keep", false, "collapse", "html").unwrap(),
            "<p>a</p>\n\n<p>b</p>\n"
        );
    }

    #[test]
    fn trim_keeps_a_line_that_still_has_markup_on_it() {
        assert_eq!(
            strip("<p>a</p><!-- x -->\n<p>b</p>\n", true, true, true, "", "keep", false, "trim", "html")
                .unwrap(),
            "<p>a</p>\n<p>b</p>\n"
        );
    }

    #[test]
    fn report_counts_kinds_and_savings() {
        let r = strip(
            "<!-- a --><!--[if IE]>b<![endif]--><p>x</p>",
            true,
            true,
            true,
            "",
            "keep",
            false,
            "keep",
            "report",
        )
        .unwrap();
        assert!(r.contains("comments_found,2\n"), "{r}");
        assert!(r.contains("comments_removed,1\n"), "{r}");
        assert!(r.contains("comments_kept,1\n"), "{r}");
        assert!(r.contains("removed_plain,1\n"), "{r}");
        assert!(r.contains("bytes_saved,10\n"), "{r}");
    }

    #[test]
    fn comments_output_lists_line_kind_and_action() {
        let r = strip(
            "<p>a</p>\n<!-- one -->\n<!--! two -->",
            true,
            true,
            true,
            "",
            "keep",
            false,
            "keep",
            "comments",
        )
        .unwrap();
        assert_eq!(
            r,
            "line,kind,action,comment\n2,plain,removed,<!-- one -->\n3,bang,kept,<!--! two -->\n"
        );
    }

    #[test]
    fn unknown_option_values_are_rejected() {
        assert!(strip("<p><!--x--></p>", true, true, true, "", "nope", false, "keep", "html")
            .unwrap_err()
            .contains("unknown pattern_mode"));
        assert!(strip("<p><!--x--></p>", true, true, true, "", "keep", false, "nope", "html")
            .unwrap_err()
            .contains("unknown blank_lines"));
        assert!(strip("<p><!--x--></p>", true, true, true, "", "keep", false, "keep", "nope")
            .unwrap_err()
            .contains("unknown output"));
    }

    #[test]
    fn over_the_cap_is_rejected_at_the_boundary() {
        let at_cap = "<p>".to_string() + &"x".repeat(MAX_INPUT_BYTES - 3);
        assert_eq!(at_cap.len(), MAX_INPUT_BYTES);
        assert!(strip(&at_cap, true, true, true, "", "keep", false, "keep", "html").is_ok());
        let over = at_cap + "x";
        assert!(strip(&over, true, true, true, "", "keep", false, "keep", "html")
            .unwrap_err()
            .contains("over the 5000000-byte limit"));
    }

    #[test]
    fn multibyte_text_survives_css_stripping() {
        let h = "<style>/* x */a{content:\"héllo — ok\"}</style>";
        assert_eq!(
            strip(h, true, true, true, "", "keep", true, "keep", "html").unwrap(),
            "<style>a{content:\"héllo — ok\"}</style>"
        );
    }
}
