//! html-outline-analyzer core — pure compute, shared by the chat skill block and
//! the web page. No wafer/wasm-bindgen deps.
//!
//! Two answers about one pasted HTML document:
//!
//!   * **the heading outline** — every `<h1>`…`<h6>` in document order, nested by
//!     level, with its text, text length, `id`, 1-based line/column, and whether
//!     the markup hides it; plus the structural problems an outline can have
//!     (no h1, several h1s, a skipped level, an empty heading, duplicate heading
//!     text, a hidden heading);
//!   * **the element/tag counts** — how many element tags the document contains
//!     and how often each tag name is used.
//!
//! The scanner is deliberately dependency-free and works over the LITERAL markup
//! rather than a parsed DOM: an HTML parser invents implied `<html>`/`<head>`/
//! `<body>` elements, which would report tags the author never wrote. It is
//! quote-aware (a `>` inside an attribute value is not a tag end), skips
//! comments/doctypes/processing instructions, treats `<script>`/`<style>`/
//! `<textarea>`/`<title>` contents as raw text (a `<` in JavaScript is not a
//! tag), and counts a self-closing tag once.

use std::collections::BTreeMap;

use gizza_ai_html_entity_decoder_core::decode as decode_entities;

/// Maximum accepted input. Matches the other paste-a-document HTML tools.
pub const MAX_INPUT: usize = 5_000_000;
/// Maximum headings recorded; beyond this the outline is truncated.
pub const MAX_HEADINGS: usize = 5_000;
/// How far past a heading's opening tag the text lookahead will scan. Guards the
/// pathological unclosed-`<h1>`-at-the-top-of-a-5MB-file case.
pub const MAX_HEADING_TEXT_SCAN: usize = 65_536;

/// Elements whose contents are raw/escapable-raw text per the HTML spec — a `<`
/// inside them is literal, so the scanner jumps to the matching close tag.
/// (`<pre>` is NOT one of these: its contents are ordinary markup.)
const RAW: &[&str] = &["script", "style", "textarea", "title"];

/// One heading found in the document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Heading {
    /// 1-based position among ALL headings in the document, before any level
    /// filtering — so a filtered outline still says where each entry sits.
    pub index: usize,
    /// 1–6.
    pub level: u8,
    /// Visible text, entity-decoded and whitespace-collapsed.
    pub text: String,
    /// Characters in `text`.
    pub length: usize,
    /// The `id` attribute, or an empty string when there is none.
    pub id: String,
    /// True when the markup itself hides the heading (`hidden`,
    /// `aria-hidden="true"`, inline `display:none` / `visibility:hidden`).
    pub hidden: bool,
    /// 1-based line of the opening tag.
    pub line: usize,
    /// 1-based column of the opening tag.
    pub column: usize,
}

/// One tag name and how many times it opens an element.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TagCount {
    pub tag: String,
    pub count: usize,
}

/// One structural problem with the outline.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Issue {
    /// "error" or "warning".
    pub severity: &'static str,
    /// Stable machine-readable code (`no-h1`, `skipped-level`, …).
    pub code: &'static str,
    /// 1-based line, or 0 for a document-level finding with no single location.
    pub line: usize,
    /// What is wrong and what was expected instead.
    pub message: String,
}

/// The full analysis.
#[derive(Clone, Debug)]
pub struct Report {
    /// Headings inside the requested level window, in document order.
    pub headings: Vec<Heading>,
    /// Headings found in the whole document, before the level window.
    pub total_headings: usize,
    /// Counts for h1..h6 (index 0 = h1).
    pub level_counts: [usize; 6],
    /// Total element tags (opening + self-closing; closing tags are not counted).
    pub elements: usize,
    /// Distinct tag names used.
    pub distinct_tags: usize,
    /// The most-used tags, highest count first, ties broken alphabetically.
    pub tags: Vec<TagCount>,
    /// Distinct tags not shown because of the `top_tags` cap.
    pub tags_omitted: usize,
    /// Structural problems, document-level first, then in line order.
    pub issues: Vec<Issue>,
    /// True when the document had more than `MAX_HEADINGS` headings.
    pub truncated: bool,
    /// The requested level window, echoed for the renderers.
    pub min_level: u8,
    pub max_level: u8,
}

impl Report {
    /// Errors then warnings.
    pub fn error_count(&self) -> usize {
        self.issues.iter().filter(|i| i.severity == "error").count()
    }
    pub fn warning_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.severity == "warning")
            .count()
    }
}

/// Output rendering mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Tree,
    Markdown,
    Json,
    Csv,
}

/// Parse the `format` argument (`tree` | `markdown` | `json` | `csv`; blank → tree).
pub fn parse_format(s: &str) -> Result<Format, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "tree" | "text" => Ok(Format::Tree),
        "markdown" | "md" => Ok(Format::Markdown),
        "json" => Ok(Format::Json),
        "csv" => Ok(Format::Csv),
        other => Err(format!(
            "invalid format {other:?}: expected 'tree', 'markdown', 'json' or 'csv'"
        )),
    }
}

/// Caller-selected behavior.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    /// Lowest heading level to include in the outline (1–6).
    pub min_level: u8,
    /// Highest heading level to include in the outline (1–6).
    pub max_level: u8,
    /// Include the structural issue list.
    pub include_issues: bool,
    /// Include the element/tag count section.
    pub include_counts: bool,
    /// How many distinct tags to list (1–500).
    pub top_tags: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            min_level: 1,
            max_level: 6,
            include_issues: true,
            include_counts: true,
            top_tags: 20,
        }
    }
}

impl Options {
    fn validate(&self) -> Result<(), String> {
        if !(1..=6).contains(&self.min_level) {
            return Err(format!(
                "invalid min_level {}: expected a heading level from 1 to 6",
                self.min_level
            ));
        }
        if !(1..=6).contains(&self.max_level) {
            return Err(format!(
                "invalid max_level {}: expected a heading level from 1 to 6",
                self.max_level
            ));
        }
        if self.min_level > self.max_level {
            return Err(format!(
                "invalid level window: min_level {} is deeper than max_level {} — expected min_level <= max_level",
                self.min_level, self.max_level
            ));
        }
        if !(1..=500).contains(&self.top_tags) {
            return Err(format!(
                "invalid top_tags {}: expected a count from 1 to 500",
                self.top_tags
            ));
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Scanner primitives
// ---------------------------------------------------------------------------

/// The lowercase tag name that begins at `raw` (`<div …>` / `</div>` → "div").
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

/// `h1`…`h6` → the level, anything else → `None`.
fn heading_level(name: &str) -> Option<u8> {
    let bytes = name.as_bytes();
    if bytes.len() == 2 && bytes[0] == b'h' && (b'1'..=b'6').contains(&bytes[1]) {
        Some(bytes[1] - b'0')
    } else {
        None
    }
}

/// Count the newlines in `b[from..to)` into the running line counter.
fn advance(b: &[u8], from: usize, to: usize, line: &mut usize, line_start: &mut usize) {
    for (k, &c) in b.iter().enumerate().take(to.min(b.len())).skip(from) {
        if c == b'\n' {
            *line += 1;
            *line_start = k + 1;
        }
    }
}

/// Read attribute `name` out of an opening tag's source. Returns the raw
/// (still entity-encoded) value; a valueless attribute yields `Some("")`.
fn attr(tag: &str, name: &str) -> Option<String> {
    let b = tag.as_bytes();
    // Step past `<tagname`.
    let mut i = 1;
    while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'-' || b[i] == b':') {
        i += 1;
    }
    while i < b.len() {
        while i < b.len() && (b[i] as char).is_ascii_whitespace() {
            i += 1;
        }
        if i >= b.len() || b[i] == b'>' || b[i] == b'/' {
            return None;
        }
        let key_start = i;
        while i < b.len()
            && !(b[i] as char).is_ascii_whitespace()
            && b[i] != b'='
            && b[i] != b'>'
            && b[i] != b'/'
        {
            i += 1;
        }
        let key = tag[key_start..i].to_ascii_lowercase();
        // Optional whitespace around '='.
        let mut j = i;
        while j < b.len() && (b[j] as char).is_ascii_whitespace() {
            j += 1;
        }
        if j < b.len() && b[j] == b'=' {
            j += 1;
            while j < b.len() && (b[j] as char).is_ascii_whitespace() {
                j += 1;
            }
            let (value, next) = if j < b.len() && (b[j] == b'"' || b[j] == b'\'') {
                let quote = b[j];
                let start = j + 1;
                let mut k = start;
                while k < b.len() && b[k] != quote {
                    k += 1;
                }
                (
                    tag[start..k.min(tag.len())].to_string(),
                    (k + 1).min(b.len()),
                )
            } else {
                let start = j;
                let mut k = j;
                while k < b.len() && !(b[k] as char).is_ascii_whitespace() && b[k] != b'>' {
                    k += 1;
                }
                (tag[start..k].to_string(), k)
            };
            if key == name {
                return Some(value);
            }
            i = next;
        } else {
            if key == name {
                return Some(String::new());
            }
            i = j;
        }
    }
    None
}

/// Decode character references and collapse runs of whitespace to one space.
fn normalize_text(raw: &str) -> String {
    let decoded = decode_entities(raw, "keep").unwrap_or_else(|_| raw.to_string());
    decoded.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Does this opening tag hide the element via the markup alone?
fn markup_hidden(tag: &str) -> bool {
    if let Some(v) = attr(tag, "hidden") {
        // `hidden="until-found"` still renders once the browser reveals it.
        if !v.trim().eq_ignore_ascii_case("until-found") {
            return true;
        }
    }
    if attr(tag, "aria-hidden")
        .map(|v| v.trim().eq_ignore_ascii_case("true"))
        .unwrap_or(false)
    {
        return true;
    }
    if let Some(style) = attr(tag, "style") {
        let flat: String = style
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>()
            .to_ascii_lowercase();
        if flat.contains("display:none") || flat.contains("visibility:hidden") {
            return true;
        }
    }
    false
}

/// Extract a heading's visible text, starting just past its opening tag.
/// Stops at the matching `</hN>`, at the next heading's opening tag (the
/// heading was never closed), at `MAX_HEADING_TEXT_SCAN` bytes, or at EOF.
fn heading_text(html: &str, from: usize, level: u8) -> String {
    let b = html.as_bytes();
    let limit = from.saturating_add(MAX_HEADING_TEXT_SCAN).min(b.len());
    let mut out = String::new();
    let mut j = from;
    let mut text_start = from;
    while j < limit {
        if b[j] != b'<' {
            j += 1;
            continue;
        }
        out.push_str(&html[text_start..j]);
        if html[j..].starts_with("<!--") {
            j = html[j + 4..]
                .find("-->")
                .map(|p| j + 4 + p + 3)
                .unwrap_or(b.len());
            text_start = j.min(b.len());
            continue;
        }
        let end = scan_tag(b, j).unwrap_or(b.len());
        let raw = &html[j..end.min(html.len())];
        let name = tag_name(raw);
        let closing = b.get(j + 1) == Some(&b'/');
        if closing && heading_level(&name) == Some(level) {
            return normalize_text(&out);
        }
        if !closing && heading_level(&name).is_some() {
            // Unclosed heading — the next heading starts the following section.
            return normalize_text(&out);
        }
        if !closing && RAW.contains(&name.as_str()) && !raw.trim_end().ends_with("/>") {
            // Raw text (e.g. a <style> inside a heading) is markup, not visible text.
            let close = format!("</{name}");
            j = html[end..]
                .to_ascii_lowercase()
                .find(&close)
                .map(|p| end + p)
                .unwrap_or(b.len());
            text_start = j.min(b.len());
            continue;
        }
        j = end;
        text_start = j.min(b.len());
    }
    out.push_str(&html[text_start.min(html.len())..limit.min(html.len())]);
    normalize_text(&out)
}

// ---------------------------------------------------------------------------
// Analysis
// ---------------------------------------------------------------------------

/// Scan `html` and build the outline + tag counts.
pub fn analyze(html: &str, opts: &Options) -> Result<Report, String> {
    opts.validate()?;
    if html.trim().is_empty() {
        return Err("input is empty: paste an HTML document or fragment to analyze".into());
    }
    if html.len() > MAX_INPUT {
        return Err(format!(
            "input is {} bytes: the maximum is {} bytes (about 5 MB) — split the document and analyze it in parts",
            html.len(),
            MAX_INPUT
        ));
    }

    let b = html.as_bytes();
    let n = b.len();
    let mut i = 0usize;
    let mut line = 1usize;
    let mut line_start = 0usize;

    let mut all: Vec<Heading> = Vec::new();
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut elements = 0usize;
    let mut truncated = false;

    while i < n {
        if b[i] != b'<' {
            if b[i] == b'\n' {
                line += 1;
                line_start = i + 1;
            }
            i += 1;
            continue;
        }
        let tag_line = line;
        let tag_col = i - line_start + 1;

        // Comment.
        if html[i..].starts_with("<!--") {
            let end = html[i + 4..]
                .find("-->")
                .map(|p| i + 4 + p + 3)
                .unwrap_or(n);
            advance(b, i, end, &mut line, &mut line_start);
            i = end;
            continue;
        }
        // Doctype / CDATA / processing instruction.
        if b.get(i + 1) == Some(&b'!') || b.get(i + 1) == Some(&b'?') {
            let end = scan_tag(b, i).unwrap_or(n);
            advance(b, i, end, &mut line, &mut line_start);
            i = end;
            continue;
        }
        // Closing tag — not an element, just skipped.
        if b.get(i + 1) == Some(&b'/') {
            let end = scan_tag(b, i).unwrap_or(n);
            advance(b, i, end, &mut line, &mut line_start);
            i = end;
            continue;
        }
        // A `<` that does not start a tag (`a < b`, `<3`) is literal text.
        if !b.get(i + 1).is_some_and(|c| c.is_ascii_alphabetic()) {
            i += 1;
            continue;
        }

        let end = scan_tag(b, i).unwrap_or(n);
        let raw = &html[i..end.min(html.len())];
        let name = tag_name(raw);
        if name.is_empty() {
            advance(b, i, end, &mut line, &mut line_start);
            i = end;
            continue;
        }
        elements += 1;
        *counts.entry(name.clone()).or_insert(0) += 1;
        let self_closing = raw.trim_end().ends_with("/>");

        if let Some(level) = heading_level(&name) {
            if !self_closing {
                if all.len() < MAX_HEADINGS {
                    let text = heading_text(html, end, level);
                    all.push(Heading {
                        index: all.len() + 1,
                        level,
                        length: text.chars().count(),
                        text,
                        id: attr(raw, "id")
                            .map(|v| normalize_text(&v))
                            .unwrap_or_default(),
                        hidden: markup_hidden(raw),
                        line: tag_line,
                        column: tag_col,
                    });
                } else {
                    truncated = true;
                }
            }
        } else if RAW.contains(&name.as_str()) && !self_closing {
            // Skip raw text; resume at the closing tag so it is scanned normally.
            let close = format!("</{name}");
            let stop = html[end..]
                .to_ascii_lowercase()
                .find(&close)
                .map(|p| end + p)
                .unwrap_or(n);
            advance(b, i, stop, &mut line, &mut line_start);
            i = stop;
            continue;
        }

        advance(b, i, end, &mut line, &mut line_start);
        i = end;
    }

    let mut level_counts = [0usize; 6];
    for h in &all {
        level_counts[(h.level - 1) as usize] += 1;
    }

    let issues = if opts.include_issues {
        find_issues(&all, &level_counts)
    } else {
        Vec::new()
    };

    let distinct_tags = counts.len();
    let mut tags: Vec<TagCount> = counts
        .into_iter()
        .map(|(tag, count)| TagCount { tag, count })
        .collect();
    // Highest count first; alphabetical within a tie (BTreeMap already gives
    // alphabetical order, and sort_by is stable, so ties stay alphabetical).
    tags.sort_by(|a, b| b.count.cmp(&a.count));
    let tags_omitted = tags.len().saturating_sub(opts.top_tags);
    tags.truncate(opts.top_tags);

    let headings: Vec<Heading> = all
        .into_iter()
        .filter(|h| h.level >= opts.min_level && h.level <= opts.max_level)
        .collect();

    Ok(Report {
        headings,
        total_headings: level_counts.iter().sum(),
        level_counts,
        elements,
        distinct_tags,
        tags,
        tags_omitted,
        issues,
        truncated,
        min_level: opts.min_level,
        max_level: opts.max_level,
    })
}

/// Every structural problem the outline has. Computed over ALL headings, not the
/// filtered window — a level filter must not hide a real skipped level.
fn find_issues(all: &[Heading], level_counts: &[usize; 6]) -> Vec<Issue> {
    let mut doc: Vec<Issue> = Vec::new();
    let mut located: Vec<Issue> = Vec::new();

    if all.is_empty() {
        doc.push(Issue {
            severity: "warning",
            code: "no-headings",
            line: 0,
            message: "No <h1>–<h6> elements were found. A document with no headings has no outline; screen readers and search engines both rely on one.".into(),
        });
        return doc;
    }

    if level_counts[0] == 0 {
        doc.push(Issue {
            severity: "error",
            code: "no-h1",
            line: 0,
            message: format!(
                "No <h1>. The document has {} heading(s) but no top-level one — expected exactly one <h1> naming the page.",
                all.len()
            ),
        });
    } else if level_counts[0] > 1 {
        let lines: Vec<String> = all
            .iter()
            .filter(|h| h.level == 1)
            .map(|h| h.line.to_string())
            .collect();
        doc.push(Issue {
            severity: "warning",
            code: "multiple-h1",
            line: 0,
            message: format!(
                "{} <h1> elements (lines {}). Expected one — demote the extras to <h2>, or keep several only if the page really is several documents.",
                level_counts[0],
                lines.join(", ")
            ),
        });
    }

    if all[0].level != 1 {
        located.push(Issue {
            severity: "warning",
            code: "first-not-h1",
            line: all[0].line,
            message: format!(
                "The first heading is <h{}>, not <h1>. Expected the outline to start at level 1.",
                all[0].level
            ),
        });
    }

    // Skipped levels: going deeper by more than one step leaves a hole.
    let mut prev: Option<&Heading> = None;
    for h in all {
        if let Some(p) = prev {
            if h.level > p.level + 1 {
                located.push(Issue {
                    severity: "error",
                    code: "skipped-level",
                    line: h.line,
                    message: format!(
                        "Heading level jumps from <h{}> (line {}) to <h{}> — expected <h{}> next. Skipping a level breaks the outline.",
                        p.level,
                        p.line,
                        h.level,
                        p.level + 1
                    ),
                });
            }
        }
        prev = Some(h);
    }

    for h in all {
        if h.text.is_empty() {
            located.push(Issue {
                severity: "error",
                code: "empty-heading",
                line: h.line,
                message: format!(
                    "<h{}> has no text. Expected wording that names the section; an empty heading is announced as a blank one.",
                    h.level
                ),
            });
        }
        if h.hidden {
            located.push(Issue {
                severity: "warning",
                code: "hidden-heading",
                line: h.line,
                message: format!(
                    "<h{}> \"{}\" is hidden by the markup (hidden / aria-hidden / inline display or visibility). It still occupies a slot in the outline.",
                    h.level, h.text
                ),
            });
        }
    }

    // Duplicate heading text, reported once per distinct text.
    let mut seen: BTreeMap<String, Vec<&Heading>> = BTreeMap::new();
    for h in all {
        if !h.text.is_empty() {
            seen.entry(h.text.to_lowercase()).or_default().push(h);
        }
    }
    for group in seen.values() {
        if group.len() > 1 {
            let lines: Vec<String> = group.iter().map(|h| h.line.to_string()).collect();
            located.push(Issue {
                severity: "warning",
                code: "duplicate-text",
                line: group[1].line,
                message: format!(
                    "\"{}\" is used by {} headings (lines {}). Expected distinct wording so each section is identifiable.",
                    group[0].text,
                    group.len(),
                    lines.join(", ")
                ),
            });
        }
    }

    located.sort_by_key(|i| i.line);
    doc.extend(located);
    doc
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

/// Analyze `html` and render the report in `format`.
pub fn analyze_to_string(html: &str, format: Format, opts: &Options) -> Result<String, String> {
    let report = analyze(html, opts)?;
    Ok(match format {
        Format::Tree => render_tree(&report, opts),
        Format::Markdown => render_markdown(&report, opts),
        Format::Json => render_json(&report, opts),
        Format::Csv => render_csv(&report, opts)?,
    })
}

fn level_breakdown(level_counts: &[usize; 6]) -> String {
    let parts: Vec<String> = (0..6)
        .filter(|&k| level_counts[k] > 0)
        .map(|k| format!("h{}:{}", k + 1, level_counts[k]))
        .collect();
    if parts.is_empty() {
        "none".into()
    } else {
        parts.join("  ")
    }
}

fn outline_caption(r: &Report) -> String {
    if r.min_level == 1 && r.max_level == 6 {
        format!("OUTLINE ({} headings)", r.total_headings)
    } else {
        format!(
            "OUTLINE (h{}–h{}: {} of {} headings)",
            r.min_level,
            r.max_level,
            r.headings.len(),
            r.total_headings
        )
    }
}

fn render_tree(r: &Report, opts: &Options) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "HTML OUTLINE — {} heading(s) · {} element(s) · {} distinct tag(s)\n",
        r.total_headings, r.elements, r.distinct_tags
    ));
    out.push_str(&format!("Levels: {}\n", level_breakdown(&r.level_counts)));
    if opts.include_issues {
        out.push_str(&format!(
            "Issues: {} error(s), {} warning(s)\n",
            r.error_count(),
            r.warning_count()
        ));
    }
    if r.truncated {
        out.push_str(&format!(
            "Note: only the first {MAX_HEADINGS} headings were recorded.\n"
        ));
    }

    out.push_str(&format!("\n{}\n", outline_caption(r)));
    if r.headings.is_empty() {
        out.push_str("  (no headings in this range)\n");
    }
    for h in &r.headings {
        let indent = "  ".repeat((h.level.saturating_sub(r.min_level)) as usize);
        let text = if h.text.is_empty() {
            "(empty)".to_string()
        } else {
            h.text.clone()
        };
        let mut suffix = String::new();
        if !h.id.is_empty() {
            suffix.push_str(&format!("  #{}", h.id));
        }
        if h.hidden {
            suffix.push_str("  [hidden]");
        }
        out.push_str(&format!(
            "{:>5}  {}h{}  {}{}\n",
            h.line, indent, h.level, text, suffix
        ));
    }

    if opts.include_issues {
        out.push_str(&format!("\nISSUES ({})\n", r.issues.len()));
        if r.issues.is_empty() {
            out.push_str("  none — the heading outline is well formed\n");
        }
        for issue in &r.issues {
            let at = if issue.line == 0 {
                "     ".to_string()
            } else {
                format!("{:>5}", issue.line)
            };
            out.push_str(&format!(
                "{}  {:<7}  {}: {}\n",
                at, issue.severity, issue.code, issue.message
            ));
        }
    }

    if opts.include_counts {
        let shown = r.tags.len();
        out.push_str(&format!(
            "\nTAG COUNTS ({} of {} distinct tag(s), {} element(s) total)\n",
            shown, r.distinct_tags, r.elements
        ));
        if r.tags.is_empty() {
            out.push_str("  (no elements)\n");
        }
        let width = r
            .tags
            .iter()
            .map(|t| t.tag.len())
            .max()
            .unwrap_or(3)
            .min(30);
        for t in &r.tags {
            out.push_str(&format!("  {:<width$}  {:>6}\n", t.tag, t.count));
        }
        if r.tags_omitted > 0 {
            out.push_str(&format!(
                "  … and {} more distinct tag(s) — raise top_tags to see them\n",
                r.tags_omitted
            ));
        }
    }
    out
}

fn md_escape(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

fn render_markdown(r: &Report, opts: &Options) -> String {
    let mut out = String::new();
    out.push_str("## Summary\n\n| Metric | Value |\n| --- | --- |\n");
    out.push_str(&format!("| Headings | {} |\n", r.total_headings));
    out.push_str(&format!(
        "| Levels used | {} |\n",
        md_escape(&level_breakdown(&r.level_counts))
    ));
    out.push_str(&format!("| Elements | {} |\n", r.elements));
    out.push_str(&format!("| Distinct tags | {} |\n", r.distinct_tags));
    if opts.include_issues {
        out.push_str(&format!("| Errors | {} |\n", r.error_count()));
        out.push_str(&format!("| Warnings | {} |\n", r.warning_count()));
    }

    out.push_str(&format!("\n## {}\n\n", outline_caption(r)));
    if r.headings.is_empty() {
        out.push_str("_No headings in this range._\n");
    }
    for h in &r.headings {
        let indent = "  ".repeat((h.level.saturating_sub(r.min_level)) as usize);
        let text = if h.text.is_empty() {
            "_(empty)_".to_string()
        } else {
            md_escape(&h.text)
        };
        let mut suffix = format!(" _(line {}", h.line);
        if !h.id.is_empty() {
            suffix.push_str(&format!(", #{}", md_escape(&h.id)));
        }
        if h.hidden {
            suffix.push_str(", hidden");
        }
        suffix.push_str(")_");
        out.push_str(&format!("{indent}- **h{}** {}{}\n", h.level, text, suffix));
    }

    if opts.include_issues {
        out.push_str("\n## Issues\n\n");
        if r.issues.is_empty() {
            out.push_str("_None — the heading outline is well formed._\n");
        } else {
            out.push_str("| Severity | Line | Code | Detail |\n| --- | --- | --- | --- |\n");
            for issue in &r.issues {
                let at = if issue.line == 0 {
                    "—".to_string()
                } else {
                    issue.line.to_string()
                };
                out.push_str(&format!(
                    "| {} | {} | `{}` | {} |\n",
                    issue.severity,
                    at,
                    issue.code,
                    md_escape(&issue.message)
                ));
            }
        }
    }

    if opts.include_counts {
        out.push_str(&format!(
            "\n## Tag counts ({} of {} distinct)\n\n",
            r.tags.len(),
            r.distinct_tags
        ));
        out.push_str("| Tag | Count |\n| --- | --- |\n");
        for t in &r.tags {
            out.push_str(&format!("| `{}` | {} |\n", md_escape(&t.tag), t.count));
        }
        if r.tags_omitted > 0 {
            out.push_str(&format!(
                "\n_{} more distinct tag(s) not shown — raise `top_tags` to see them._\n",
                r.tags_omitted
            ));
        }
    }
    out
}

/// Nest the flat, document-ordered headings into a tree by level.
fn nest(headings: &[Heading]) -> Vec<serde_json::Value> {
    use serde_json::{json, Value};
    let mut roots: Vec<Value> = Vec::new();
    // Stack of paths into `roots`, one entry per open ancestor.
    let mut stack: Vec<(u8, Vec<usize>)> = Vec::new();

    for h in headings {
        while stack.last().is_some_and(|(lvl, _)| *lvl >= h.level) {
            stack.pop();
        }
        let node = json!({
            "index": h.index,
            "level": h.level,
            "text": h.text,
            "length": h.length,
            "id": h.id,
            "hidden": h.hidden,
            "line": h.line,
            "column": h.column,
            "children": []
        });
        let path = match stack.last() {
            None => {
                roots.push(node);
                vec![roots.len() - 1]
            }
            Some((_, parent)) => {
                let mut cur = &mut roots[parent[0]];
                for step in &parent[1..] {
                    cur = &mut cur["children"][*step];
                }
                let kids = cur["children"]
                    .as_array_mut()
                    .expect("children is an array");
                kids.push(node);
                let mut p = parent.clone();
                p.push(kids.len() - 1);
                p
            }
        };
        stack.push((h.level, path));
    }
    roots
}

fn render_json(r: &Report, opts: &Options) -> String {
    use serde_json::{json, Map, Value};
    let mut root = Map::new();

    let mut levels = Map::new();
    for k in 0..6 {
        levels.insert(format!("h{}", k + 1), json!(r.level_counts[k]));
    }
    let mut summary = Map::new();
    summary.insert("headings".into(), json!(r.total_headings));
    summary.insert("levels".into(), Value::Object(levels));
    summary.insert("elements".into(), json!(r.elements));
    summary.insert("distinct_tags".into(), json!(r.distinct_tags));
    if opts.include_issues {
        summary.insert("errors".into(), json!(r.error_count()));
        summary.insert("warnings".into(), json!(r.warning_count()));
        summary.insert("well_formed".into(), json!(r.issues.is_empty()));
    }
    summary.insert("truncated".into(), json!(r.truncated));
    root.insert("summary".into(), Value::Object(summary));

    root.insert(
        "outline".into(),
        Value::Array(
            r.headings
                .iter()
                .map(|h| {
                    json!({
                        "index": h.index,
                        "level": h.level,
                        "text": h.text,
                        "length": h.length,
                        "id": h.id,
                        "hidden": h.hidden,
                        "line": h.line,
                        "column": h.column
                    })
                })
                .collect(),
        ),
    );
    root.insert("outline_tree".into(), Value::Array(nest(&r.headings)));

    if opts.include_issues {
        root.insert(
            "issues".into(),
            Value::Array(
                r.issues
                    .iter()
                    .map(|i| {
                        json!({
                            "severity": i.severity,
                            "code": i.code,
                            "line": i.line,
                            "message": i.message
                        })
                    })
                    .collect(),
            ),
        );
    }
    if opts.include_counts {
        root.insert(
            "tags".into(),
            Value::Array(
                r.tags
                    .iter()
                    .map(|t| json!({ "tag": t.tag, "count": t.count }))
                    .collect(),
            ),
        );
        root.insert("tags_omitted".into(), json!(r.tags_omitted));
    }
    serde_json::to_string_pretty(&Value::Object(root))
        .unwrap_or_else(|e| format!("{{\"error\":{e:?}}}"))
}

fn render_csv(r: &Report, opts: &Options) -> Result<String, String> {
    let mut w = csv::Writer::from_writer(Vec::new());
    w.write_record([
        "index", "level", "tag", "text", "length", "id", "hidden", "line", "column",
    ])
    .map_err(|e| format!("csv write failed: {e}"))?;
    for h in &r.headings {
        w.write_record([
            h.index.to_string(),
            h.level.to_string(),
            format!("h{}", h.level),
            h.text.clone(),
            h.length.to_string(),
            h.id.clone(),
            h.hidden.to_string(),
            h.line.to_string(),
            h.column.to_string(),
        ])
        .map_err(|e| format!("csv write failed: {e}"))?;
    }
    let mut out = String::from_utf8(
        w.into_inner()
            .map_err(|e| format!("csv write failed: {e}"))?,
    )
    .map_err(|e| format!("csv output was not valid UTF-8: {e}"))?;

    if opts.include_issues {
        let mut w = csv::Writer::from_writer(Vec::new());
        w.write_record(["severity", "code", "line", "message"])
            .map_err(|e| format!("csv write failed: {e}"))?;
        for i in &r.issues {
            w.write_record([
                i.severity.to_string(),
                i.code.to_string(),
                i.line.to_string(),
                i.message.clone(),
            ])
            .map_err(|e| format!("csv write failed: {e}"))?;
        }
        out.push('\n');
        out.push_str(
            &String::from_utf8(
                w.into_inner()
                    .map_err(|e| format!("csv write failed: {e}"))?,
            )
            .map_err(|e| format!("csv output was not valid UTF-8: {e}"))?,
        );
    }

    if opts.include_counts {
        let mut w = csv::Writer::from_writer(Vec::new());
        w.write_record(["tag", "count"])
            .map_err(|e| format!("csv write failed: {e}"))?;
        for t in &r.tags {
            w.write_record([t.tag.clone(), t.count.to_string()])
                .map_err(|e| format!("csv write failed: {e}"))?;
        }
        out.push('\n');
        out.push_str(
            &String::from_utf8(
                w.into_inner()
                    .map_err(|e| format!("csv write failed: {e}"))?,
            )
            .map_err(|e| format!("csv output was not valid UTF-8: {e}"))?,
        );
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = "<!doctype html>\n<html><head><title>Docs</title></head>\n<body>\n<h1>Getting started</h1>\n<h2 id=\"install\">Install</h2>\n<p>Text</p>\n<h3>macOS</h3>\n<h3>Windows</h3>\n<h2>Configure</h2>\n<h4>Advanced flags</h4>\n</body></html>\n";

    fn opts() -> Options {
        Options::default()
    }

    #[test]
    fn happy_path_tree_lists_every_heading_in_order() {
        let out = analyze_to_string(DOC, Format::Tree, &opts()).unwrap();
        assert!(out.contains("HTML OUTLINE — 6 heading(s)"), "{out}");
        assert!(out.contains("h1  Getting started"), "{out}");
        assert!(out.contains("  h2  Install  #install"), "{out}");
        assert!(out.contains("    h3  macOS"), "{out}");
        // Skipped level h2 -> h4 is reported as an error.
        assert!(out.contains("skipped-level"), "{out}");
        assert!(out.contains("Levels: h1:1  h2:2  h3:2  h4:1"), "{out}");
    }

    #[test]
    fn error_on_empty_input() {
        let err = analyze("   \n ", &opts()).unwrap_err();
        assert!(err.contains("input is empty"), "{err}");
    }

    #[test]
    fn error_on_bad_level_window() {
        let o = Options {
            min_level: 4,
            max_level: 2,
            ..Options::default()
        };
        let err = analyze("<h1>x</h1>", &o).unwrap_err();
        assert!(
            err.contains("min_level 4 is deeper than max_level 2"),
            "{err}"
        );
    }

    #[test]
    fn parse_format_accepts_the_four_modes_and_rejects_others() {
        assert_eq!(parse_format("").unwrap(), Format::Tree);
        assert_eq!(parse_format("Tree").unwrap(), Format::Tree);
        assert_eq!(parse_format("markdown").unwrap(), Format::Markdown);
        assert_eq!(parse_format("json").unwrap(), Format::Json);
        assert_eq!(parse_format("csv").unwrap(), Format::Csv);
        assert!(parse_format("yaml").unwrap_err().contains("invalid format"));
    }

    #[test]
    fn tag_counts_are_source_fidelity_no_implied_elements() {
        // A bare fragment: a DOM parser would invent html/head/body. We must not.
        let r = analyze("<div><p>a</p><p>b</p><br></div>", &opts()).unwrap();
        assert_eq!(r.elements, 4, "div + p + p + br");
        assert_eq!(r.distinct_tags, 3);
        let map: Vec<(&str, usize)> = r.tags.iter().map(|t| (t.tag.as_str(), t.count)).collect();
        assert_eq!(map, vec![("p", 2), ("br", 1), ("div", 1)]);
    }

    #[test]
    fn closing_tags_and_comments_and_doctype_are_not_counted() {
        let r = analyze("<!doctype html><!-- <div><div> --><span>x</span>", &opts()).unwrap();
        assert_eq!(r.elements, 1);
        assert_eq!(
            r.tags,
            vec![TagCount {
                tag: "span".into(),
                count: 1
            }]
        );
    }

    #[test]
    fn raw_text_elements_do_not_leak_fake_tags() {
        let html = "<h1>Title</h1><script>if (a < b) { x('<div>') }</script><style>a{}</style>";
        let r = analyze(html, &opts()).unwrap();
        assert_eq!(r.elements, 3, "h1 + script + style only");
        assert_eq!(r.headings[0].text, "Title");
    }

    #[test]
    fn quoted_gt_inside_an_attribute_is_not_a_tag_end() {
        let r = analyze("<h1 data-x=\"a > b\" id=\"t\">Hi</h1>", &opts()).unwrap();
        assert_eq!(r.elements, 1);
        assert_eq!(r.headings[0].text, "Hi");
        assert_eq!(r.headings[0].id, "t");
    }

    #[test]
    fn heading_text_strips_inner_tags_decodes_entities_and_collapses_space() {
        let r = analyze(
            "<h2>  Tips &amp;\n  <em>tricks</em>&nbsp;here </h2>",
            &opts(),
        )
        .unwrap();
        assert_eq!(r.headings[0].text, "Tips & tricks here");
        assert_eq!(r.headings[0].length, 18);
        // The <em> still counts as an element.
        assert_eq!(r.elements, 2);
    }

    #[test]
    fn line_and_column_are_one_based() {
        let r = analyze("<p>x</p>\n\n  <h1>Top</h1>", &opts()).unwrap();
        assert_eq!(r.headings[0].line, 3);
        assert_eq!(r.headings[0].column, 3);
    }

    #[test]
    fn issue_no_h1_when_headings_start_deeper() {
        let r = analyze("<h2>A</h2><h3>B</h3>", &opts()).unwrap();
        let codes: Vec<&str> = r.issues.iter().map(|i| i.code).collect();
        assert!(codes.contains(&"no-h1"), "{codes:?}");
        assert!(codes.contains(&"first-not-h1"), "{codes:?}");
        assert_eq!(r.error_count(), 1);
    }

    #[test]
    fn issue_multiple_h1_and_duplicate_text() {
        let r = analyze("<h1>Docs</h1><h1>docs</h1>", &opts()).unwrap();
        let codes: Vec<&str> = r.issues.iter().map(|i| i.code).collect();
        assert!(codes.contains(&"multiple-h1"), "{codes:?}");
        assert!(codes.contains(&"duplicate-text"), "{codes:?}");
    }

    #[test]
    fn issue_empty_and_hidden_headings() {
        let r = analyze(
            "<h1>Top</h1><h2></h2><h2 style=\"display: none\">Ghost</h2><h2 hidden>Gone</h2><h2 aria-hidden=\"true\">Aria</h2>",
            &opts(),
        )
        .unwrap();
        let codes: Vec<&str> = r.issues.iter().map(|i| i.code).collect();
        assert_eq!(codes.iter().filter(|c| **c == "empty-heading").count(), 1);
        assert_eq!(codes.iter().filter(|c| **c == "hidden-heading").count(), 3);
    }

    #[test]
    fn hidden_until_found_is_not_flagged() {
        let r = analyze("<h1 hidden=\"until-found\">Top</h1>", &opts()).unwrap();
        assert!(!r.headings[0].hidden);
        assert!(!r.issues.iter().any(|i| i.code == "hidden-heading"));
    }

    #[test]
    fn no_headings_is_reported_as_a_warning_not_a_crash() {
        let r = analyze("<div><p>Just text</p></div>", &opts()).unwrap();
        assert_eq!(r.total_headings, 0);
        assert_eq!(r.issues.len(), 1);
        assert_eq!(r.issues[0].code, "no-headings");
        assert_eq!(r.error_count(), 0);
    }

    #[test]
    fn well_formed_outline_has_no_issues() {
        let r = analyze("<h1>A</h1><h2>B</h2><h3>C</h3><h2>D</h2>", &opts()).unwrap();
        assert!(r.issues.is_empty(), "{:?}", r.issues);
    }

    #[test]
    fn level_window_filters_the_outline_but_not_the_issues() {
        let o = Options {
            min_level: 2,
            max_level: 3,
            ..Options::default()
        };
        let r = analyze(DOC, &o).unwrap();
        assert_eq!(r.headings.len(), 4, "h2 h3 h3 h2");
        assert!(r.headings.iter().all(|h| h.level >= 2 && h.level <= 3));
        assert_eq!(r.total_headings, 6);
        // The h2 -> h4 skip is outside the window but still reported.
        assert!(r.issues.iter().any(|i| i.code == "skipped-level"));
    }

    #[test]
    fn top_tags_caps_the_histogram_and_reports_the_remainder() {
        let o = Options {
            top_tags: 2,
            ..Options::default()
        };
        let r = analyze("<a></a><b></b><i></i><u></u>", &o).unwrap();
        assert_eq!(r.tags.len(), 2);
        assert_eq!(r.tags_omitted, 2);
        assert_eq!(r.distinct_tags, 4);
    }

    #[test]
    fn json_has_a_nested_tree_and_a_flat_outline() {
        let out = analyze_to_string(DOC, Format::Json, &opts()).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["summary"]["headings"], 6);
        assert_eq!(v["summary"]["levels"]["h3"], 2);
        assert_eq!(v["outline"].as_array().unwrap().len(), 6);
        let tree = v["outline_tree"].as_array().unwrap();
        assert_eq!(tree.len(), 1, "one h1 root");
        assert_eq!(tree[0]["text"], "Getting started");
        let kids = tree[0]["children"].as_array().unwrap();
        assert_eq!(kids.len(), 2, "two h2 children");
        assert_eq!(kids[0]["children"].as_array().unwrap().len(), 2);
        assert_eq!(kids[1]["children"].as_array().unwrap()[0]["level"], 4);
        assert_eq!(v["summary"]["well_formed"], false);
    }

    #[test]
    fn csv_quotes_commas_in_heading_text() {
        let out = analyze_to_string(
            "<h1>Ship, then iterate</h1>",
            Format::Csv,
            &Options {
                include_issues: false,
                include_counts: false,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(
            out,
            "index,level,tag,text,length,id,hidden,line,column\n1,1,h1,\"Ship, then iterate\",18,,false,1,1\n"
        );
    }

    #[test]
    fn sections_can_be_switched_off() {
        let o = Options {
            include_issues: false,
            include_counts: false,
            ..Options::default()
        };
        let out = analyze_to_string(DOC, Format::Tree, &o).unwrap();
        assert!(!out.contains("ISSUES"), "{out}");
        assert!(!out.contains("TAG COUNTS"), "{out}");
        assert!(out.contains("OUTLINE"), "{out}");
    }

    #[test]
    fn markdown_renders_nested_bullets() {
        let out = analyze_to_string(DOC, Format::Markdown, &opts()).unwrap();
        assert!(out.contains("- **h1** Getting started _(line 4)_"), "{out}");
        assert!(
            out.contains("  - **h2** Install _(line 5, #install)_"),
            "{out}"
        );
        assert!(out.contains("| `skipped-level` |"), "{out}");
        assert!(out.contains("| `h3` | 2 |"), "{out}");
    }

    #[test]
    fn unclosed_heading_still_gets_its_own_text() {
        let r = analyze("<h1>First\n<h2>Second</h2>", &opts()).unwrap();
        assert_eq!(r.headings.len(), 2);
        assert_eq!(r.headings[0].text, "First");
        assert_eq!(r.headings[1].text, "Second");
    }

    #[test]
    fn oversized_input_is_rejected_with_the_limit() {
        let big = "a".repeat(MAX_INPUT + 1);
        let err = analyze(&big, &opts()).unwrap_err();
        assert!(err.contains("the maximum is 5000000 bytes"), "{err}");
    }

    #[test]
    fn uppercase_tags_are_normalized() {
        let r = analyze("<DIV><H1 ID=\"top\">Hi</H1></DIV>", &opts()).unwrap();
        assert_eq!(r.headings[0].level, 1);
        assert_eq!(r.headings[0].id, "top");
        assert_eq!(
            r.tags,
            vec![
                TagCount {
                    tag: "div".into(),
                    count: 1
                },
                TagCount {
                    tag: "h1".into(),
                    count: 1
                },
            ]
        );
    }
}
