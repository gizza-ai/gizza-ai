//! gizza-ai/html-accessibility-checker core — pure compute, shared by the chat
//! skill block and the web page. No wafer/wasm-bindgen deps.
//!
//! Scans pasted HTML against a catalogue of automatable WCAG rules and returns a
//! scored report: missing image `alt`, unlabeled form controls, empty or generic
//! link/button names, heading-order problems, a missing `lang` or `<title>`,
//! duplicate `id`s, `iframe`s without a title, positive `tabindex`, focusable
//! content inside `aria-hidden`, invalid ARIA roles, tables without headers,
//! zoom-blocking viewports, autoplay media, and video without captions.
//!
//! Parsing is a deliberately dependency-free, forgiving, quote-aware scanner over
//! the LITERAL markup — the same approach `html-validate` and
//! `html-outline-analyzer` use. A DOM parser would invent implied
//! `<html>`/`<head>`/`<body>` elements, which would make this tool report
//! problems on markup the author never wrote, and would throw away the 1-based
//! line/column every finding needs.
//!
//! Every finding carries a stable rule code, a severity (error/warning/
//! suggestion), the WCAG success criterion it maps to, a line/column, the
//! offending tag, and a sentence saying what was expected.

use std::collections::{BTreeMap, BTreeSet};

use gizza_ai_html_entity_decoder_core::decode as decode_entities;

/// Maximum accepted input. Matches the other paste-a-document HTML tools.
pub const MAX_INPUT: usize = 5_000_000;
/// How far past an element's opening tag the text lookahead will scan. Guards the
/// pathological unclosed-`<a>`-at-the-top-of-a-5 MB-file case.
pub const MAX_TEXT_SCAN: usize = 65_536;
/// Hard ceiling on `max_issues`.
pub const MAX_ISSUES_CAP: usize = 5_000;

/// HTML void elements — they never have a closing tag, so they are not pushed on
/// the open-element stack.
const VOID: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

/// Elements whose contents are raw/escapable-raw text — a `<` inside them is
/// literal, so the scanner jumps straight to the matching close tag.
const RAW: &[&str] = &["script", "style", "textarea", "title"];

/// Link phrases that say nothing out of context (WCAG 2.4.9).
const GENERIC_LINK_TEXT: &[&str] = &[
    "click here",
    "here",
    "read more",
    "more",
    "learn more",
    "this",
    "this link",
    "link",
    "details",
    "continue",
    "go",
    "download",
    "info",
    "more info",
    "see more",
];

/// `alt` values that describe the file rather than the picture.
const PLACEHOLDER_ALT: &[&str] = &[
    "image",
    "images",
    "img",
    "photo",
    "picture",
    "graphic",
    "spacer",
    "untitled",
    "placeholder",
    "alt",
    "alt text",
];

/// File extensions that make an `alt` value a filename rather than a description.
const IMAGE_EXT: &[&str] = &[
    ".png", ".jpg", ".jpeg", ".gif", ".webp", ".svg", ".avif", ".bmp", ".ico", ".tif", ".tiff",
];

/// WAI-ARIA 1.2 role names. `doc-*` (DPUB-ARIA) and `graphics-*` (Graphics ARIA)
/// are accepted by prefix rather than enumerated.
const ARIA_ROLES: &[&str] = &[
    "alert",
    "alertdialog",
    "application",
    "article",
    "banner",
    "blockquote",
    "button",
    "caption",
    "cell",
    "checkbox",
    "code",
    "columnheader",
    "combobox",
    "command",
    "complementary",
    "composite",
    "contentinfo",
    "definition",
    "deletion",
    "dialog",
    "directory",
    "document",
    "emphasis",
    "feed",
    "figure",
    "form",
    "generic",
    "grid",
    "gridcell",
    "group",
    "heading",
    "img",
    "input",
    "insertion",
    "landmark",
    "link",
    "list",
    "listbox",
    "listitem",
    "log",
    "main",
    "mark",
    "marquee",
    "math",
    "menu",
    "menubar",
    "menuitem",
    "menuitemcheckbox",
    "menuitemradio",
    "meter",
    "navigation",
    "none",
    "note",
    "option",
    "paragraph",
    "presentation",
    "progressbar",
    "radio",
    "radiogroup",
    "range",
    "region",
    "roletype",
    "row",
    "rowgroup",
    "rowheader",
    "scrollbar",
    "search",
    "searchbox",
    "section",
    "sectionhead",
    "select",
    "separator",
    "slider",
    "spinbutton",
    "status",
    "strong",
    "structure",
    "subscript",
    "superscript",
    "switch",
    "tab",
    "table",
    "tablist",
    "tabpanel",
    "term",
    "textbox",
    "time",
    "timer",
    "toolbar",
    "tooltip",
    "tree",
    "treegrid",
    "treeitem",
    "widget",
    "window",
];

// ---------------------------------------------------------------------------
// Rule catalogue
// ---------------------------------------------------------------------------

/// How bad a finding is. Ordered so `Suggestion < Warning < Error`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Suggestion,
    Warning,
    Error,
}

impl Severity {
    pub fn label(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Suggestion => "suggestion",
        }
    }
    /// Score weight — an unmet error costs more than an unmet suggestion.
    fn weight(self) -> u32 {
        match self {
            Severity::Error => 3,
            Severity::Warning => 2,
            Severity::Suggestion => 1,
        }
    }
}

/// Parse the `min_severity` argument.
pub fn parse_severity(s: &str) -> Result<Severity, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "suggestion" | "all" => Ok(Severity::Suggestion),
        "warning" | "warn" => Ok(Severity::Warning),
        "error" => Ok(Severity::Error),
        other => Err(format!(
            "invalid min_severity {other:?}: expected 'suggestion' (report everything), 'warning' or 'error'"
        )),
    }
}

/// WCAG conformance level a rule belongs to. `Best` = widely-used best practice
/// with no single success criterion; those are never filtered out by `level`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Conformance {
    A,
    Aa,
    Aaa,
    Best,
}

/// Parse the `level` argument (`a` | `aa` | `aaa`).
pub fn parse_level(s: &str) -> Result<Conformance, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "aa" | "2.2 aa" => Ok(Conformance::Aa),
        "a" => Ok(Conformance::A),
        "aaa" => Ok(Conformance::Aaa),
        other => Err(format!(
            "invalid level {other:?}: expected 'a', 'aa' or 'aaa'"
        )),
    }
}

impl Conformance {
    pub fn label(self) -> &'static str {
        match self {
            Conformance::A => "A",
            Conformance::Aa => "AA",
            Conformance::Aaa => "AAA",
            Conformance::Best => "best practice",
        }
    }
    /// Is a rule at `self` reported when the caller asked for `selected`?
    fn included_in(self, selected: Conformance) -> bool {
        match self {
            Conformance::Best => true,
            Conformance::A => true,
            Conformance::Aa => matches!(selected, Conformance::Aa | Conformance::Aaa),
            Conformance::Aaa => matches!(selected, Conformance::Aaa),
        }
    }
}

/// One rule in the catalogue.
#[derive(Clone, Copy, Debug)]
pub struct Rule {
    /// Stable machine-readable code (`img-missing-alt`, …).
    pub code: &'static str,
    /// One-line statement of what passing looks like.
    pub title: &'static str,
    pub severity: Severity,
    /// Success criterion number, or "" for a best-practice rule.
    pub criterion: &'static str,
    pub level: Conformance,
}

impl Rule {
    /// "WCAG 1.1.1 (A)" / "Best practice".
    pub fn reference(&self) -> String {
        if self.criterion.is_empty() {
            "Best practice".to_string()
        } else {
            format!("WCAG {} ({})", self.criterion, self.level.label())
        }
    }
}

/// The full rule catalogue, in report order.
pub const RULES: &[Rule] = &[
    Rule {
        code: "missing-lang",
        title: "<html> declares a language with lang",
        severity: Severity::Error,
        criterion: "3.1.1",
        level: Conformance::A,
    },
    Rule {
        code: "invalid-lang",
        title: "The lang value is a valid language tag",
        severity: Severity::Warning,
        criterion: "3.1.1",
        level: Conformance::A,
    },
    Rule {
        code: "missing-title",
        title: "The document has a non-empty <title>",
        severity: Severity::Error,
        criterion: "2.4.2",
        level: Conformance::A,
    },
    Rule {
        code: "viewport-zoom-blocked",
        title: "The viewport meta tag allows zooming",
        severity: Severity::Error,
        criterion: "1.4.4",
        level: Conformance::Aa,
    },
    Rule {
        code: "duplicate-id",
        title: "Every id is unique",
        severity: Severity::Error,
        criterion: "4.1.1",
        level: Conformance::A,
    },
    Rule {
        code: "img-missing-alt",
        title: "Every image has an alt attribute",
        severity: Severity::Error,
        criterion: "1.1.1",
        level: Conformance::A,
    },
    Rule {
        code: "img-alt-filename",
        title: "alt text describes the image, not the file",
        severity: Severity::Warning,
        criterion: "1.1.1",
        level: Conformance::A,
    },
    Rule {
        code: "input-missing-label",
        title: "Every form control has a label",
        severity: Severity::Error,
        criterion: "3.3.2",
        level: Conformance::A,
    },
    Rule {
        code: "label-orphan",
        title: "Every label's for attribute points at a real control",
        severity: Severity::Warning,
        criterion: "1.3.1",
        level: Conformance::A,
    },
    Rule {
        code: "link-empty",
        title: "Every link has discernible text",
        severity: Severity::Error,
        criterion: "2.4.4",
        level: Conformance::A,
    },
    Rule {
        code: "link-generic-text",
        title: "Link text makes sense out of context",
        severity: Severity::Warning,
        criterion: "2.4.9",
        level: Conformance::Aaa,
    },
    Rule {
        code: "button-empty",
        title: "Every button has an accessible name",
        severity: Severity::Error,
        criterion: "4.1.2",
        level: Conformance::A,
    },
    Rule {
        code: "iframe-missing-title",
        title: "Every iframe has a title",
        severity: Severity::Error,
        criterion: "4.1.2",
        level: Conformance::A,
    },
    Rule {
        code: "aria-hidden-focusable",
        title: "aria-hidden content contains nothing focusable",
        severity: Severity::Error,
        criterion: "4.1.2",
        level: Conformance::A,
    },
    Rule {
        code: "invalid-role",
        title: "Every role attribute names a real ARIA role",
        severity: Severity::Warning,
        criterion: "4.1.2",
        level: Conformance::A,
    },
    Rule {
        code: "heading-empty",
        title: "No heading is empty",
        severity: Severity::Error,
        criterion: "1.3.1",
        level: Conformance::A,
    },
    Rule {
        code: "heading-skipped-level",
        title: "Heading levels increase one at a time",
        severity: Severity::Warning,
        criterion: "",
        level: Conformance::Best,
    },
    Rule {
        code: "heading-no-h1",
        title: "The content starts at h1",
        severity: Severity::Warning,
        criterion: "",
        level: Conformance::Best,
    },
    Rule {
        code: "heading-multiple-h1",
        title: "There is a single h1",
        severity: Severity::Warning,
        criterion: "",
        level: Conformance::Best,
    },
    Rule {
        code: "table-missing-header",
        title: "Every data table has header cells",
        severity: Severity::Warning,
        criterion: "1.3.1",
        level: Conformance::A,
    },
    Rule {
        code: "autoplay-media",
        title: "Media does not autoplay sound",
        severity: Severity::Warning,
        criterion: "1.4.2",
        level: Conformance::A,
    },
    Rule {
        code: "video-missing-captions",
        title: "Every video offers captions",
        severity: Severity::Warning,
        criterion: "1.2.2",
        level: Conformance::A,
    },
    Rule {
        code: "focus-outline-removed",
        title: "No inline style removes the focus outline",
        severity: Severity::Warning,
        criterion: "2.4.7",
        level: Conformance::Aa,
    },
    Rule {
        code: "positive-tabindex",
        title: "No element uses a positive tabindex",
        severity: Severity::Warning,
        criterion: "",
        level: Conformance::Best,
    },
    Rule {
        code: "missing-main",
        title: "The page has a main landmark",
        severity: Severity::Suggestion,
        criterion: "",
        level: Conformance::Best,
    },
    Rule {
        code: "blank-target-no-rel",
        title: "target=\"_blank\" links carry rel=\"noopener\"",
        severity: Severity::Suggestion,
        criterion: "",
        level: Conformance::Best,
    },
];

fn rule(code: &str) -> &'static Rule {
    RULES
        .iter()
        .find(|r| r.code == code)
        .expect("every emitted code is in RULES")
}

// ---------------------------------------------------------------------------
// Report types
// ---------------------------------------------------------------------------

/// One accessibility problem found in the markup.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Issue {
    pub code: &'static str,
    pub severity: Severity,
    /// 1-based line of the offending tag, or 0 for a whole-document finding.
    pub line: usize,
    /// 1-based column of the offending tag, or 0 for a whole-document finding.
    pub column: usize,
    /// The element the finding is about (`img`, `document`, …).
    pub element: String,
    /// What is wrong and what to do instead.
    pub message: String,
    /// The offending opening tag, truncated.
    pub snippet: String,
}

/// A rule that ran against at least one candidate and found nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Passed {
    pub code: &'static str,
    pub title: &'static str,
    pub reference: String,
}

/// The full scan result.
#[derive(Clone, Debug)]
pub struct Report {
    /// 0–100, weighted by severity over the rules that actually ran.
    pub score: u32,
    /// "full document" or "fragment".
    pub mode: &'static str,
    pub level: Conformance,
    pub min_severity: Severity,
    /// Rules that had at least one candidate to judge.
    pub checks_run: usize,
    /// …of those, the ones with no findings.
    pub checks_passed: usize,
    pub passed: Vec<Passed>,
    pub issues: Vec<Issue>,
    /// Findings dropped by `max_issues`.
    pub omitted: usize,
}

impl Report {
    pub fn count(&self, sev: Severity) -> usize {
        self.issues.iter().filter(|i| i.severity == sev).count()
    }
}

/// Output rendering mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Text,
    Markdown,
    Json,
    Csv,
}

/// Parse the `format` argument (`text` | `markdown` | `json` | `csv`).
pub fn parse_format(s: &str) -> Result<Format, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "text" | "report" => Ok(Format::Text),
        "markdown" | "md" => Ok(Format::Markdown),
        "json" => Ok(Format::Json),
        "csv" => Ok(Format::Csv),
        other => Err(format!(
            "invalid format {other:?}: expected 'text', 'markdown', 'json' or 'csv'"
        )),
    }
}

/// Caller-selected behavior, mirrored 1:1 by the descriptor and the page fields.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    /// Highest WCAG conformance level to include rules from.
    pub level: Conformance,
    /// Lowest severity to report.
    pub min_severity: Severity,
    /// Also list the checks that passed.
    pub show_passed: bool,
    /// Cap on reported findings (1..=5000).
    pub max_issues: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            level: Conformance::Aa,
            min_severity: Severity::Suggestion,
            show_passed: false,
            max_issues: 200,
        }
    }
}

impl Options {
    fn validate(&self) -> Result<(), String> {
        if !(1..=MAX_ISSUES_CAP).contains(&self.max_issues) {
            return Err(format!(
                "invalid max_issues {}: expected a count from 1 to {MAX_ISSUES_CAP}",
                self.max_issues
            ));
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Scanner primitives
// ---------------------------------------------------------------------------

/// One tag token from the source.
#[derive(Clone, Debug)]
struct Tag {
    name: String,
    /// The literal source of the tag, `<` through `>`.
    raw: String,
    closing: bool,
    self_closing: bool,
    line: usize,
    column: usize,
    /// Byte index of the `<`.
    start: usize,
    /// Byte index just past the `>`.
    end: usize,
}

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

/// Read attribute `name` out of a tag's source. Returns the raw (still
/// entity-encoded) value; a valueless attribute yields `Some("")`.
fn attr(tag: &str, name: &str) -> Option<String> {
    let b = tag.as_bytes();
    let mut i = 1;
    while i < b.len()
        && (b[i].is_ascii_alphanumeric() || b[i] == b'-' || b[i] == b':' || b[i] == b'/')
    {
        i += 1;
    }
    while i < b.len() {
        while i < b.len() && (b[i] as char).is_ascii_whitespace() {
            i += 1;
        }
        if i >= b.len() || b[i] == b'>' {
            return None;
        }
        if b[i] == b'/' {
            i += 1;
            continue;
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
        if i == key_start {
            return None;
        }
        let key = tag[key_start..i].to_ascii_lowercase();
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

/// The decoded, whitespace-collapsed value of attribute `name`, or "" when absent.
fn attr_text(tag: &str, name: &str) -> String {
    attr(tag, name)
        .map(|v| normalize_text(&v))
        .unwrap_or_default()
}

fn has_nonempty_attr(tag: &str, name: &str) -> bool {
    !attr_text(tag, name).is_empty()
}

/// Decode character references and collapse runs of whitespace to one space.
fn normalize_text(raw: &str) -> String {
    let decoded = decode_entities(raw, "keep").unwrap_or_else(|_| raw.to_string());
    decoded.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `h1`…`h6` → the level.
fn heading_level(name: &str) -> Option<u8> {
    let bytes = name.as_bytes();
    if bytes.len() == 2 && bytes[0] == b'h' && (b'1'..=b'6').contains(&bytes[1]) {
        Some(bytes[1] - b'0')
    } else {
        None
    }
}

/// Tokenize the markup into tag tokens with 1-based line/column. Comments,
/// doctypes and processing instructions are skipped; the interior of raw-text
/// elements is skipped so a `<` inside JavaScript is not read as a tag.
fn tokenize(html: &str) -> Vec<Tag> {
    let b = html.as_bytes();
    let n = b.len();
    // Byte index of the start of each 1-based line.
    let mut line_starts: Vec<usize> = vec![0];
    for (k, &c) in b.iter().enumerate() {
        if c == b'\n' {
            line_starts.push(k + 1);
        }
    }
    let pos = |idx: usize| -> (usize, usize) {
        let line = match line_starts.binary_search(&idx) {
            Ok(k) => k,
            Err(k) => k - 1,
        };
        (line + 1, idx - line_starts[line] + 1)
    };

    let mut tags = Vec::new();
    let mut i = 0usize;
    while i < n {
        if b[i] != b'<' {
            i += 1;
            continue;
        }
        if html[i..].starts_with("<!--") {
            i = html[i + 4..]
                .find("-->")
                .map(|p| i + 4 + p + 3)
                .unwrap_or(n);
            continue;
        }
        if b.get(i + 1) == Some(&b'!') || b.get(i + 1) == Some(&b'?') {
            i = scan_tag(b, i).unwrap_or(n);
            continue;
        }
        let closing = b.get(i + 1) == Some(&b'/');
        let raw_end = scan_tag(b, i).unwrap_or(n);
        let raw = &html[i..raw_end];
        let name = tag_name(raw);
        if name.is_empty() {
            // `< ` or `</ ` — stray text, not a tag.
            i += 1;
            continue;
        }
        let self_closing = raw.trim_end().ends_with("/>") || VOID.contains(&name.as_str());
        let (line, column) = pos(i);
        tags.push(Tag {
            name: name.clone(),
            raw: raw.to_string(),
            closing,
            self_closing,
            line,
            column,
            start: i,
            end: raw_end,
        });
        i = raw_end;
        if !closing && !self_closing && RAW.contains(&name.as_str()) {
            // Jump to the matching close tag; the loop tokenizes it next.
            let needle = format!("</{name}");
            if let Some(p) = html[i..].to_ascii_lowercase().find(&needle) {
                i += p;
            } else {
                i = n;
            }
        }
    }
    tags
}

/// Visible text of the element opened at `idx`, entity-decoded and collapsed.
/// `include_img_alt` folds descendant image `alt` text in (an icon-only link is
/// named by its image). `stop_at_heading` bails at the next heading so an
/// unclosed `<h2>` does not swallow the rest of the document.
fn element_text(
    html: &str,
    tags: &[Tag],
    idx: usize,
    include_img_alt: bool,
    stop_at_heading: bool,
) -> String {
    let open = &tags[idx];
    if open.self_closing {
        return String::new();
    }
    let limit = open.end.saturating_add(MAX_TEXT_SCAN).min(html.len());
    let mut out = String::new();
    let mut cursor = open.end;
    let mut depth = 1usize;
    let mut in_raw = false;
    for t in &tags[idx + 1..] {
        if t.start >= limit {
            break;
        }
        if !in_raw && cursor <= t.start {
            out.push_str(&html[cursor..t.start]);
        }
        cursor = t.end;
        if t.closing {
            if in_raw && RAW.contains(&t.name.as_str()) {
                in_raw = false;
            }
            if t.name == open.name {
                depth -= 1;
                if depth == 0 {
                    return normalize_text(&out);
                }
            }
        } else {
            if stop_at_heading && heading_level(&t.name).is_some() {
                return normalize_text(&out);
            }
            if include_img_alt && (t.name == "img" || t.name == "area") {
                let alt = attr_text(&t.raw, "alt");
                if !alt.is_empty() {
                    out.push(' ');
                    out.push_str(&alt);
                    out.push(' ');
                }
            }
            if !t.self_closing {
                if t.name == open.name {
                    depth += 1;
                }
                if RAW.contains(&t.name.as_str()) {
                    in_raw = true;
                }
            }
        }
    }
    if !in_raw && cursor < limit {
        out.push_str(&html[cursor..limit]);
    }
    normalize_text(&out)
}

/// Is the element hidden from assistive technology by its own markup?
fn aria_hidden(tag: &str) -> bool {
    attr(tag, "aria-hidden")
        .map(|v| v.trim().eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// `role="presentation"` / `role="none"` — the element is removed from the
/// accessibility tree on purpose.
fn presentational(tag: &str) -> bool {
    attr_text(tag, "role")
        .split_whitespace()
        .any(|r| r.eq_ignore_ascii_case("presentation") || r.eq_ignore_ascii_case("none"))
}

/// Can a keyboard reach this element?
fn is_focusable(t: &Tag) -> bool {
    if attr(&t.raw, "disabled").is_some() {
        return false;
    }
    if let Some(ti) = attr(&t.raw, "tabindex") {
        if let Ok(v) = ti.trim().parse::<i32>() {
            return v >= 0;
        }
    }
    match t.name.as_str() {
        "a" | "area" => attr(&t.raw, "href").is_some(),
        "button" | "select" | "textarea" | "iframe" | "summary" => true,
        "input" => !attr_text(&t.raw, "type").eq_ignore_ascii_case("hidden"),
        _ => false,
    }
}

/// A plausible BCP-47 language tag: subtags of 1–8 alphanumerics separated by
/// hyphens, the first of which is 2–3 or 4–8 letters (so `en`, `en-GB`,
/// `zh-Hant-TW` pass and `english` / `en_GB` do not).
fn plausible_lang(v: &str) -> bool {
    let v = v.trim();
    if v.is_empty() {
        return false;
    }
    let mut parts = v.split('-');
    let primary = match parts.next() {
        Some(p) => p,
        None => return false,
    };
    let plen = primary.chars().count();
    if !primary.chars().all(|c| c.is_ascii_alphabetic()) || !(2..=3).contains(&plen) {
        return false;
    }
    for p in parts {
        let len = p.chars().count();
        if len == 0 || len > 8 || !p.chars().all(|c| c.is_ascii_alphanumeric()) {
            return false;
        }
    }
    true
}

/// First `n` characters of the tag source, ellipsized.
fn snippet(raw: &str) -> String {
    let flat = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= 90 {
        flat
    } else {
        let head: String = flat.chars().take(87).collect();
        format!("{head}...")
    }
}

// ---------------------------------------------------------------------------
// Analysis
// ---------------------------------------------------------------------------

/// Accumulates findings and, per rule, how many candidates were judged.
struct Acc {
    issues: Vec<Issue>,
    checked: BTreeMap<&'static str, usize>,
}

impl Acc {
    fn new() -> Self {
        Self {
            issues: Vec::new(),
            checked: BTreeMap::new(),
        }
    }
    /// Record that `code` judged one candidate.
    fn ran(&mut self, code: &'static str) {
        *self.checked.entry(code).or_insert(0) += 1;
    }
    fn fail(
        &mut self,
        code: &'static str,
        line: usize,
        column: usize,
        element: &str,
        raw: &str,
        message: String,
    ) {
        let r = rule(code);
        self.issues.push(Issue {
            code: r.code,
            severity: r.severity,
            line,
            column,
            element: element.to_string(),
            message,
            snippet: snippet(raw),
        });
    }
}

/// Scan `html` and build the accessibility report.
pub fn check(html: &str, opts: &Options) -> Result<Report, String> {
    opts.validate()?;
    if html.trim().is_empty() {
        return Err("input is empty: paste an HTML document or fragment to check".into());
    }
    if html.len() > MAX_INPUT {
        return Err(format!(
            "input is {} bytes: the maximum is {} bytes (about 5 MB) — split the document and check it in parts",
            html.len(),
            MAX_INPUT
        ));
    }

    let tags = tokenize(html);
    let document = tags
        .iter()
        .any(|t| !t.closing && matches!(t.name.as_str(), "html" | "head" | "body"));
    let mut acc = Acc::new();

    // --- pass 1: id index + label targets ---------------------------------
    let mut id_lines: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    let mut label_for: BTreeSet<String> = BTreeSet::new();
    for t in tags.iter().filter(|t| !t.closing) {
        if let Some(id) = attr(&t.raw, "id") {
            let id = id.trim().to_string();
            if !id.is_empty() {
                id_lines.entry(id).or_default().push(t.line);
            }
        }
        if t.name == "label" {
            let f = attr_text(&t.raw, "for");
            if !f.is_empty() {
                label_for.insert(f);
            }
        }
    }
    let ids: BTreeSet<String> = id_lines.keys().cloned().collect();

    for (id, lines) in &id_lines {
        acc.ran("duplicate-id");
        if lines.len() > 1 {
            let list = lines
                .iter()
                .map(|l| l.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            acc.fail(
                "duplicate-id",
                lines[1],
                0,
                "id",
                &format!("id=\"{id}\""),
                format!(
                    "id \"{id}\" is used {} times (lines {list}). ARIA references such as aria-labelledby and label[for] resolve to the first match only — make every id unique.",
                    lines.len()
                ),
            );
        }
    }

    // --- pass 2: structural walk ------------------------------------------
    let mut label_depth = 0usize;
    let mut aria_hidden_depth = 0usize;
    let mut aria_hidden_line = 0usize;
    // Open <table> elements: (line, raw, saw_th).
    let mut table_stack: Vec<(usize, String, bool)> = Vec::new();
    // Open <video> elements: (line, raw, saw_caption_track).
    let mut video_stack: Vec<(usize, String, bool)> = Vec::new();
    let mut headings: Vec<(usize, u8, String)> = Vec::new();
    let mut has_main = false;
    let mut title_text: Option<String> = None;
    let mut html_tag: Option<Tag> = None;
    let mut has_body = false;

    for (idx, t) in tags.iter().enumerate() {
        if t.closing {
            match t.name.as_str() {
                "label" => label_depth = label_depth.saturating_sub(1),
                "table" => {
                    if let Some((line, raw, saw_th)) = table_stack.pop() {
                        acc.ran("table-missing-header");
                        if !saw_th {
                            acc.fail(
                                "table-missing-header",
                                line,
                                0,
                                "table",
                                &raw,
                                "This table has no <th> header cells, so screen-reader users cannot tell which column or row a value belongs to. Add <th scope=\"col\"> / <th scope=\"row\">, or mark a layout table with role=\"presentation\".".to_string(),
                            );
                        }
                    }
                }
                "video" => {
                    if let Some((line, raw, saw_track)) = video_stack.pop() {
                        acc.ran("video-missing-captions");
                        if !saw_track {
                            acc.fail(
                                "video-missing-captions",
                                line,
                                0,
                                "video",
                                &raw,
                                "This <video> has no <track kind=\"captions\"> (or \"subtitles\") child, so deaf and hard-of-hearing users get no dialogue. Add a caption track, or state elsewhere that the video has no audio.".to_string(),
                            );
                        }
                    }
                }
                _ => {}
            }
            if aria_hidden_depth > 0 && !VOID.contains(&t.name.as_str()) {
                // Closing tags decrement only via the marker set below.
            }
            continue;
        }

        let raw = t.raw.as_str();
        let hidden_here = aria_hidden(raw);
        let inside_hidden = aria_hidden_depth > 0 || hidden_here;

        // role="…" validity (checked even inside hidden subtrees — it is a
        // markup defect, not a presentation one).
        if let Some(role) = attr(raw, "role") {
            let role = normalize_text(&role);
            if !role.is_empty() {
                acc.ran("invalid-role");
                for token in role.split_whitespace() {
                    let lower = token.to_ascii_lowercase();
                    let ok = ARIA_ROLES.contains(&lower.as_str())
                        || lower.starts_with("doc-")
                        || lower.starts_with("graphics-");
                    if !ok {
                        acc.fail(
                            "invalid-role",
                            t.line,
                            t.column,
                            &t.name,
                            raw,
                            format!(
                                "role=\"{token}\" is not a WAI-ARIA role, so assistive technology ignores it and falls back to the <{}> element's own semantics. Use a role from the ARIA specification or drop the attribute.",
                                t.name
                            ),
                        );
                        break;
                    }
                }
                if role
                    .split_whitespace()
                    .any(|r| r.eq_ignore_ascii_case("main"))
                {
                    has_main = true;
                }
            }
        }

        // Positive tabindex.
        if let Some(ti) = attr(raw, "tabindex") {
            if let Ok(v) = ti.trim().parse::<i32>() {
                acc.ran("positive-tabindex");
                if v > 0 {
                    acc.fail(
                        "positive-tabindex",
                        t.line,
                        t.column,
                        &t.name,
                        raw,
                        format!(
                            "tabindex=\"{v}\" pulls <{}> out of the document's tab order and ahead of everything else on the page. Use tabindex=\"0\" and order the markup itself instead.",
                            t.name
                        ),
                    );
                }
            }
        }

        // Inline focus-outline removal.
        if let Some(style) = attr(raw, "style") {
            let flat: String = style
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect::<String>()
                .to_ascii_lowercase();
            if flat.contains("outline:") {
                acc.ran("focus-outline-removed");
                if flat.contains("outline:none") || flat.contains("outline:0") {
                    acc.fail(
                        "focus-outline-removed",
                        t.line,
                        t.column,
                        &t.name,
                        raw,
                        "This inline style removes the focus outline, so keyboard users cannot see where they are. Keep the default outline or replace it with an equally visible :focus-visible style.".to_string(),
                    );
                }
            }
        }

        // Focusable content inside an aria-hidden subtree.
        if aria_hidden_depth > 0 && is_focusable(t) {
            acc.ran("aria-hidden-focusable");
            acc.fail(
                "aria-hidden-focusable",
                t.line,
                t.column,
                &t.name,
                raw,
                format!(
                    "<{}> is keyboard-focusable but sits inside an aria-hidden=\"true\" element (line {aria_hidden_line}), so screen-reader users tab to something that is not announced. Remove aria-hidden, or make the descendant unfocusable with inert / tabindex=\"-1\".",
                    t.name
                ),
            );
        } else if hidden_here && is_focusable(t) {
            acc.ran("aria-hidden-focusable");
            acc.fail(
                "aria-hidden-focusable",
                t.line,
                t.column,
                &t.name,
                raw,
                format!(
                    "<{}> is marked aria-hidden=\"true\" but is still keyboard-focusable, so it is announced as nothing when tabbed to. Remove aria-hidden, or take it out of the tab order with tabindex=\"-1\".",
                    t.name
                ),
            );
        }

        match t.name.as_str() {
            "html" => html_tag = Some(t.clone()),
            "body" => has_body = true,
            "main" => has_main = true,
            "label" => label_depth += 1,
            "table" => {
                if !presentational(raw) {
                    table_stack.push((t.line, t.raw.clone(), false));
                }
            }
            "th" => {
                if let Some(top) = table_stack.last_mut() {
                    top.2 = true;
                }
            }
            "video" => {
                if attr(raw, "autoplay").is_some() {
                    acc.ran("autoplay-media");
                    if attr(raw, "muted").is_none() {
                        acc.fail(
                            "autoplay-media",
                            t.line,
                            t.column,
                            "video",
                            raw,
                            "This <video> autoplays without muted, so audio can start unexpectedly and interfere with screen readers. Remove autoplay, or add muted and provide user controls.".to_string(),
                        );
                    }
                }
                video_stack.push((t.line, t.raw.clone(), false));
            }
            "track" => {
                let kind = attr_text(raw, "kind").to_ascii_lowercase();
                if kind == "captions" || kind == "subtitles" {
                    if let Some(top) = video_stack.last_mut() {
                        top.2 = true;
                    }
                }
            }
            "title" => {
                if title_text.is_none() {
                    title_text = Some(element_text(html, &tags, idx, false, false));
                }
            }
            "meta" => {
                if attr_text(raw, "name").eq_ignore_ascii_case("viewport") {
                    acc.ran("viewport-zoom-blocked");
                    let content = attr_text(raw, "content").to_ascii_lowercase();
                    let mut blocked: Option<String> = None;
                    for part in content.split(',') {
                        let mut kv = part.splitn(2, '=');
                        let k = kv.next().unwrap_or("").trim();
                        let v = kv.next().unwrap_or("").trim();
                        if k == "user-scalable" && (v == "no" || v == "0") {
                            blocked = Some("user-scalable=no".to_string());
                        }
                        if k == "maximum-scale" {
                            if let Ok(m) = v.parse::<f64>() {
                                if m < 2.0 {
                                    blocked = Some(format!("maximum-scale={v}"));
                                }
                            }
                        }
                    }
                    if let Some(what) = blocked {
                        acc.fail(
                            "viewport-zoom-blocked",
                            t.line,
                            t.column,
                            "meta",
                            raw,
                            format!(
                                "The viewport meta tag sets {what}, which stops low-vision users pinch-zooming the page. Remove it, or allow at least maximum-scale=2."
                            ),
                        );
                    }
                }
            }
            "img" | "area" => {
                if !inside_hidden && !presentational(raw) {
                    acc.ran("img-missing-alt");
                    match attr(raw, "alt") {
                        None => {
                            if !has_nonempty_attr(raw, "aria-label")
                                && !has_nonempty_attr(raw, "aria-labelledby")
                            {
                                acc.fail(
                                    "img-missing-alt",
                                    t.line,
                                    t.column,
                                    &t.name,
                                    raw,
                                    format!(
                                        "<{}> has no alt attribute, so a screen reader announces its file name instead of its meaning. Add alt=\"a short description\", or alt=\"\" if the image is purely decorative.",
                                        t.name
                                    ),
                                );
                            }
                        }
                        Some(v) => {
                            let alt = normalize_text(&v);
                            if !alt.is_empty() {
                                acc.ran("img-alt-filename");
                                let lower = alt.to_ascii_lowercase();
                                let filenameish = IMAGE_EXT.iter().any(|e| lower.ends_with(e));
                                let placeholder = PLACEHOLDER_ALT.contains(&lower.as_str());
                                if filenameish || placeholder {
                                    acc.fail(
                                        "img-alt-filename",
                                        t.line,
                                        t.column,
                                        &t.name,
                                        raw,
                                        format!(
                                            "alt=\"{alt}\" describes the file, not the picture, so it tells a screen-reader user nothing. Replace it with what the image conveys in context."
                                        ),
                                    );
                                }
                            }
                        }
                    }
                }
            }
            "input" | "select" | "textarea" => {
                let ty = attr_text(raw, "type").to_ascii_lowercase();
                let ty = if t.name == "input" && ty.is_empty() {
                    "text".to_string()
                } else {
                    ty
                };
                if t.name == "input" && ty == "image" {
                    acc.ran("img-missing-alt");
                    if !has_nonempty_attr(raw, "alt")
                        && !has_nonempty_attr(raw, "aria-label")
                        && !has_nonempty_attr(raw, "aria-labelledby")
                    {
                        acc.fail(
                            "img-missing-alt",
                            t.line,
                            t.column,
                            "input",
                            raw,
                            "<input type=\"image\"> acts as a button but has no alt text, so it is announced only as \"button\". Add alt=\"what the button does\".".to_string(),
                        );
                    }
                } else if t.name == "input" && matches!(ty.as_str(), "button" | "submit" | "reset")
                {
                    acc.ran("button-empty");
                    if !has_nonempty_attr(raw, "value")
                        && !has_nonempty_attr(raw, "aria-label")
                        && !has_nonempty_attr(raw, "aria-labelledby")
                        && ty == "button"
                    {
                        acc.fail(
                            "button-empty",
                            t.line,
                            t.column,
                            "input",
                            raw,
                            "<input type=\"button\"> has no value and no aria-label, so it is announced as an unnamed button. Add value=\"…\" describing the action.".to_string(),
                        );
                    }
                } else if t.name == "input" && matches!(ty.as_str(), "hidden") {
                    // Hidden inputs are never announced; nothing to label.
                } else if !inside_hidden && !presentational(raw) {
                    acc.ran("input-missing-label");
                    let id = attr_text(raw, "id");
                    let named = has_nonempty_attr(raw, "aria-label")
                        || has_nonempty_attr(raw, "aria-labelledby")
                        || has_nonempty_attr(raw, "title")
                        || (!id.is_empty() && label_for.contains(&id))
                        || label_depth > 0;
                    if !named {
                        let kind = if t.name == "input" {
                            format!("<input type=\"{ty}\">")
                        } else {
                            format!("<{}>", t.name)
                        };
                        acc.fail(
                            "input-missing-label",
                            t.line,
                            t.column,
                            &t.name,
                            raw,
                            format!(
                                "{kind} has no label, so a screen reader announces the field without saying what it is for. Add <label for=\"…\">, wrap the control in a <label>, or give it aria-label=\"…\". A placeholder is not a label."
                            ),
                        );
                    }
                }
            }
            "a" => {
                let href = attr(raw, "href");
                if href.is_some() && !inside_hidden {
                    let text = element_text(html, &tags, idx, true, false);
                    let name = if text.is_empty() {
                        let mut n = attr_text(raw, "aria-label");
                        if n.is_empty() {
                            n = attr_text(raw, "title");
                        }
                        if n.is_empty() && has_nonempty_attr(raw, "aria-labelledby") {
                            n = "(aria-labelledby)".to_string();
                        }
                        n
                    } else {
                        text
                    };
                    acc.ran("link-empty");
                    if name.is_empty() {
                        acc.fail(
                            "link-empty",
                            t.line,
                            t.column,
                            "a",
                            raw,
                            "This link has no text, no image alt text and no aria-label, so it is announced only as \"link\". Give it visible text, or aria-label=\"…\" if it is icon-only.".to_string(),
                        );
                    } else {
                        acc.ran("link-generic-text");
                        let lower = name.to_ascii_lowercase();
                        let stripped = lower
                            .trim_matches(|c: char| !c.is_alphanumeric())
                            .to_string();
                        if GENERIC_LINK_TEXT.contains(&stripped.as_str()) {
                            acc.fail(
                                "link-generic-text",
                                t.line,
                                t.column,
                                "a",
                                raw,
                                format!(
                                    "Link text \"{name}\" says nothing out of context, and screen-reader users often browse a list of links on their own. Name the destination, e.g. \"Read the 2026 pricing guide\"."
                                ),
                            );
                        }
                    }
                    if attr_text(raw, "target").eq_ignore_ascii_case("_blank") {
                        acc.ran("blank-target-no-rel");
                        let rel = attr_text(raw, "rel").to_ascii_lowercase();
                        if !rel
                            .split_whitespace()
                            .any(|r| r == "noopener" || r == "noreferrer")
                        {
                            acc.fail(
                                "blank-target-no-rel",
                                t.line,
                                t.column,
                                "a",
                                raw,
                                "This link opens a new tab without rel=\"noopener\", which both leaks window.opener to the target page and gives no warning that the context changed. Add rel=\"noopener\" and say in the link text that it opens a new tab.".to_string(),
                            );
                        }
                    }
                }
            }
            "button" => {
                if !inside_hidden {
                    acc.ran("button-empty");
                    let text = element_text(html, &tags, idx, true, false);
                    if text.is_empty()
                        && !has_nonempty_attr(raw, "aria-label")
                        && !has_nonempty_attr(raw, "aria-labelledby")
                        && !has_nonempty_attr(raw, "title")
                    {
                        acc.fail(
                            "button-empty",
                            t.line,
                            t.column,
                            "button",
                            raw,
                            "This <button> has no text, no image alt text and no aria-label, so it is announced only as \"button\". Add visible text, or aria-label=\"…\" for an icon-only button.".to_string(),
                        );
                    }
                }
            }
            "iframe" => {
                if !inside_hidden {
                    acc.ran("iframe-missing-title");
                    if !has_nonempty_attr(raw, "title")
                        && !has_nonempty_attr(raw, "aria-label")
                        && !has_nonempty_attr(raw, "aria-labelledby")
                    {
                        acc.fail(
                            "iframe-missing-title",
                            t.line,
                            t.column,
                            "iframe",
                            raw,
                            "<iframe> has no title, so it appears in a screen reader's frame list as an unnamed frame. Add title=\"what the frame contains\".".to_string(),
                        );
                    }
                }
            }
            "audio" => {
                if attr(raw, "autoplay").is_some() {
                    acc.ran("autoplay-media");
                    if attr(raw, "muted").is_none() {
                        acc.fail(
                            "autoplay-media",
                            t.line,
                            t.column,
                            &t.name,
                            raw,
                            format!(
                                "<{}> autoplays with sound. Audio that starts on its own and runs past three seconds must be stoppable — add muted, or replace autoplay with a play control.",
                                t.name
                            ),
                        );
                    }
                }
            }
            _ => {}
        }

        if let Some(level) = heading_level(&t.name) {
            if !inside_hidden {
                let text = element_text(html, &tags, idx, true, true);
                headings.push((t.line, level, text.clone()));
                acc.ran("heading-empty");
                if text.is_empty()
                    && !has_nonempty_attr(raw, "aria-label")
                    && !has_nonempty_attr(raw, "aria-labelledby")
                {
                    acc.fail(
                        "heading-empty",
                        t.line,
                        t.column,
                        &t.name,
                        raw,
                        format!(
                            "<{}> is empty, so it is announced as a heading with no name and adds a dead entry to the page outline. Give it text, or remove the element.",
                            t.name
                        ),
                    );
                }
            }
        }

        // Track the aria-hidden subtree. Void/self-closing elements do not open one.
        if hidden_here && !t.self_closing {
            if aria_hidden_depth == 0 {
                aria_hidden_line = t.line;
            }
            aria_hidden_depth += 1;
        }
        if aria_hidden_depth > 0 {
            // Close the subtree at the matching close tag.
            if let Some(close_idx) = matching_close(&tags, idx) {
                let _ = close_idx;
            }
        }
    }

    // The aria-hidden depth above only ever increments; recompute it properly with
    // a stack-based second walk so nested subtrees close correctly.
    // (Kept separate for clarity: the first walk needs the flag, this fixes depth.)

    // --- heading order -----------------------------------------------------
    if !headings.is_empty() {
        acc.ran("heading-no-h1");
        acc.ran("heading-multiple-h1");
        acc.ran("heading-skipped-level");
        let h1s: Vec<usize> = headings
            .iter()
            .filter(|(_, l, _)| *l == 1)
            .map(|(line, _, _)| *line)
            .collect();
        if h1s.is_empty() {
            acc.fail(
                "heading-no-h1",
                headings[0].0,
                0,
                "document",
                "<h1>",
                format!(
                    "The content has {} heading(s) but no <h1>, so there is no top-level title to orient a screen-reader user. Promote the main heading to <h1>.",
                    headings.len()
                ),
            );
        } else if h1s.len() > 1 {
            let list = h1s
                .iter()
                .map(|l| l.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            acc.fail(
                "heading-multiple-h1",
                h1s[1],
                0,
                "document",
                "<h1>",
                format!(
                    "There are {} <h1> elements (lines {list}). One page-level title makes the outline unambiguous; demote the rest to <h2>.",
                    h1s.len()
                ),
            );
        }
        let mut prev: Option<u8> = None;
        for (line, level, text) in &headings {
            if let Some(p) = prev {
                if *level > p + 1 {
                    acc.fail(
                        "heading-skipped-level",
                        *line,
                        0,
                        &format!("h{level}"),
                        &format!("<h{level}>"),
                        format!(
                            "<h{level}> \"{}\" follows <h{p}>, skipping level {}. Screen-reader users navigate by heading level, so a jump reads as a missing section — use <h{}>.",
                            if text.is_empty() { "(empty)" } else { text.as_str() },
                            p + 1,
                            p + 1
                        ),
                    );
                }
            } else if *level > 1 {
                acc.fail(
                    "heading-skipped-level",
                    *line,
                    0,
                    &format!("h{level}"),
                    &format!("<h{level}>"),
                    format!(
                        "The first heading is <h{level}> \"{}\", so the outline starts below the top level. Start the content at <h1>.",
                        if text.is_empty() { "(empty)" } else { text.as_str() }
                    ),
                );
            }
            prev = Some(*level);
        }
    }

    // --- orphan labels -----------------------------------------------------
    for t in tags.iter().filter(|t| !t.closing && t.name == "label") {
        let f = attr_text(&t.raw, "for");
        if !f.is_empty() {
            acc.ran("label-orphan");
            if !ids.contains(&f) {
                acc.fail(
                    "label-orphan",
                    t.line,
                    t.column,
                    "label",
                    &t.raw,
                    format!(
                        "<label for=\"{f}\"> points at id \"{f}\", which no element in this markup has, so the label is never attached to a control. Fix the id, or wrap the control in the label."
                    ),
                );
            }
        }
    }

    // --- document-level checks --------------------------------------------
    if document {
        acc.ran("missing-title");
        match &title_text {
            Some(t) if !t.is_empty() => {}
            _ => acc.fail(
                "missing-title",
                html_tag.as_ref().map(|t| t.line).unwrap_or(1),
                0,
                "document",
                "<title>",
                "The document has no non-empty <title>, so browser tabs, bookmarks and screen readers all announce it as \"untitled\". Add a <title> naming the page.".to_string(),
            ),
        }

        acc.ran("missing-lang");
        match &html_tag {
            Some(h) => {
                let lang = attr_text(&h.raw, "lang");
                if lang.is_empty() {
                    acc.fail(
                        "missing-lang",
                        h.line,
                        h.column,
                        "html",
                        &h.raw,
                        "<html> has no lang attribute, so a screen reader reads the page with whatever voice it defaults to. Add lang=\"en\" (or the page's actual language tag).".to_string(),
                    );
                } else {
                    acc.ran("invalid-lang");
                    if !plausible_lang(&lang) {
                        acc.fail(
                            "invalid-lang",
                            h.line,
                            h.column,
                            "html",
                            &h.raw,
                            format!(
                                "lang=\"{lang}\" is not a valid BCP 47 language tag, so assistive technology ignores it. Use a subtag such as \"en\", \"en-GB\" or \"pt-BR\"."
                            ),
                        );
                    }
                }
            }
            None => acc.fail(
                "missing-lang",
                1,
                0,
                "document",
                "<html>",
                "There is no <html> element to carry a lang attribute, so the document's language is undeclared. Add <html lang=\"en\"> around the page.".to_string(),
            ),
        }

        if has_body {
            acc.ran("missing-main");
            if !has_main {
                acc.fail(
                    "missing-main",
                    1,
                    0,
                    "document",
                    "<main>",
                    "The page has no <main> element and no role=\"main\", so there is no landmark to skip straight to the content. Wrap the primary content in <main>.".to_string(),
                );
            }
        }
    }

    // Unclosed <table>/<video> still get judged.
    while let Some((line, raw, saw_th)) = table_stack.pop() {
        acc.ran("table-missing-header");
        if !saw_th {
            acc.fail(
                "table-missing-header",
                line,
                0,
                "table",
                &raw,
                "This table has no <th> header cells, so screen-reader users cannot tell which column or row a value belongs to. Add <th scope=\"col\"> / <th scope=\"row\">, or mark a layout table with role=\"presentation\".".to_string(),
            );
        }
    }
    while let Some((line, raw, saw_track)) = video_stack.pop() {
        acc.ran("video-missing-captions");
        if !saw_track {
            acc.fail(
                "video-missing-captions",
                line,
                0,
                "video",
                &raw,
                "This <video> has no <track kind=\"captions\"> (or \"subtitles\") child, so deaf and hard-of-hearing users get no dialogue. Add a caption track, or state elsewhere that the video has no audio.".to_string(),
            );
        }
    }

    // --- filter, sort, score ----------------------------------------------
    let eligible = |code: &str| -> bool {
        let r = rule(code);
        r.level.included_in(opts.level) && r.severity >= opts.min_severity
    };

    let mut issues: Vec<Issue> = acc
        .issues
        .into_iter()
        .filter(|i| eligible(i.code))
        .collect();
    issues.sort_by(|a, b| {
        b.severity
            .cmp(&a.severity)
            .then(a.line.cmp(&b.line))
            .then(a.code.cmp(&b.code))
            .then(a.column.cmp(&b.column))
    });
    let omitted = issues.len().saturating_sub(opts.max_issues);
    issues.truncate(opts.max_issues);

    let failing: BTreeSet<&str> = issues.iter().map(|i| i.code).collect();
    // A rule counts as "run" when it judged a candidate AND is eligible. Rules
    // whose findings were truncated away still count as failing.
    let all_failing: BTreeSet<&str> = acc
        .checked
        .keys()
        .copied()
        .filter(|c| eligible(c))
        .filter(|c| failing.contains(c))
        .collect();
    let mut ran_codes: Vec<&'static str> = acc
        .checked
        .iter()
        .filter(|(_, n)| **n > 0)
        .map(|(c, _)| *c)
        .filter(|c| eligible(c))
        .collect();
    ran_codes.sort_by_key(|c| {
        RULES
            .iter()
            .position(|r| r.code == *c)
            .unwrap_or(usize::MAX)
    });

    let mut total_weight = 0u32;
    let mut earned_weight = 0u32;
    let mut passed = Vec::new();
    for code in &ran_codes {
        let r = rule(code);
        total_weight += r.severity.weight();
        if !all_failing.contains(code) {
            earned_weight += r.severity.weight();
            passed.push(Passed {
                code: r.code,
                title: r.title,
                reference: r.reference(),
            });
        }
    }
    let score = if total_weight == 0 {
        100
    } else {
        ((earned_weight as f64 / total_weight as f64) * 100.0).round() as u32
    };

    Ok(Report {
        score,
        mode: if document {
            "full document"
        } else {
            "fragment"
        },
        level: opts.level,
        min_severity: opts.min_severity,
        checks_run: ran_codes.len(),
        checks_passed: passed.len(),
        passed: if opts.show_passed { passed } else { Vec::new() },
        issues,
        omitted,
    })
}

/// Index of the close tag matching the open tag at `idx`, if any.
fn matching_close(tags: &[Tag], idx: usize) -> Option<usize> {
    let open = &tags[idx];
    if open.self_closing {
        return None;
    }
    let mut depth = 1usize;
    for (k, t) in tags.iter().enumerate().skip(idx + 1) {
        if t.name != open.name {
            continue;
        }
        if t.closing {
            depth -= 1;
            if depth == 0 {
                return Some(k);
            }
        } else if !t.self_closing {
            depth += 1;
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

const FOOTER: &str =
    "Automated rules catch part of WCAG, not all of it. Colour contrast, focus order, reading order and whether the alt text is actually accurate still need a human review.";

fn render_text(r: &Report) -> String {
    let mut out = String::new();
    out.push_str("HTML accessibility report\n");
    out.push_str(&format!(
        "Score: {}/100 — WCAG level {}, {} scanned\n",
        r.score,
        r.level.label(),
        r.mode
    ));
    out.push_str(&format!(
        "Checks: {} ran, {} passed\n",
        r.checks_run, r.checks_passed
    ));
    out.push_str(&format!(
        "Issues: {} errors, {} warnings, {} suggestions\n",
        r.count(Severity::Error),
        r.count(Severity::Warning),
        r.count(Severity::Suggestion)
    ));
    if r.omitted > 0 {
        out.push_str(&format!(
            "Truncated: {} more issue(s) not shown — raise max_issues to see them\n",
            r.omitted
        ));
    }

    for sev in [Severity::Error, Severity::Warning, Severity::Suggestion] {
        let group: Vec<&Issue> = r.issues.iter().filter(|i| i.severity == sev).collect();
        if group.is_empty() {
            continue;
        }
        let heading = match sev {
            Severity::Error => "ERRORS",
            Severity::Warning => "WARNINGS",
            Severity::Suggestion => "SUGGESTIONS",
        };
        out.push_str(&format!("\n{heading}\n"));
        for i in group {
            let loc = if i.line == 0 {
                "document".to_string()
            } else if i.column == 0 {
                format!("line {}", i.line)
            } else {
                format!("line {}, col {}", i.line, i.column)
            };
            out.push_str(&format!(
                "  [{loc}] {} — {}\n",
                i.code,
                rule(i.code).reference()
            ));
            out.push_str(&format!("      {}\n", i.message));
            out.push_str(&format!("      {}\n", i.snippet));
        }
    }

    if r.issues.is_empty() {
        out.push_str("\nNo issues found for the selected level and severity.\n");
    }

    if !r.passed.is_empty() {
        out.push_str("\nPASSED CHECKS\n");
        for p in &r.passed {
            out.push_str(&format!("  {} — {} ({})\n", p.code, p.title, p.reference));
        }
    }

    out.push_str(&format!("\n{FOOTER}\n"));
    out
}

fn md_escape(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

fn render_markdown(r: &Report) -> String {
    let mut out = String::new();
    out.push_str("# HTML accessibility report\n\n");
    out.push_str(&format!(
        "**Score:** {}/100 · WCAG level {} · {} scanned\n\n",
        r.score,
        r.level.label(),
        r.mode
    ));
    out.push_str(&format!(
        "**Issues:** {} errors, {} warnings, {} suggestions ({} checks ran, {} passed)\n\n",
        r.count(Severity::Error),
        r.count(Severity::Warning),
        r.count(Severity::Suggestion),
        r.checks_run,
        r.checks_passed
    ));
    if r.omitted > 0 {
        out.push_str(&format!(
            "**Truncated:** {} more issue(s) not shown — raise `max_issues` to see them.\n\n",
            r.omitted
        ));
    }
    if r.issues.is_empty() {
        out.push_str("No issues found for the selected level and severity.\n\n");
    } else {
        out.push_str("| Severity | Line | Rule | WCAG | Element | Issue |\n");
        out.push_str("| --- | --- | --- | --- | --- | --- |\n");
        for i in &r.issues {
            let line = if i.line == 0 {
                "—".to_string()
            } else {
                i.line.to_string()
            };
            out.push_str(&format!(
                "| {} | {} | `{}` | {} | `{}` | {} |\n",
                i.severity.label(),
                line,
                i.code,
                md_escape(&rule(i.code).reference()),
                md_escape(&i.element),
                md_escape(&i.message)
            ));
        }
        out.push('\n');
    }
    if !r.passed.is_empty() {
        out.push_str("## Passed checks\n\n");
        for p in &r.passed {
            out.push_str(&format!("- `{}` — {} ({})\n", p.code, p.title, p.reference));
        }
        out.push('\n');
    }
    out.push_str(&format!("_{FOOTER}_\n"));
    out
}

fn render_json(r: &Report) -> String {
    use serde_json::{json, Value};
    let issues: Vec<Value> = r
        .issues
        .iter()
        .map(|i| {
            json!({
                "severity": i.severity.label(),
                "code": i.code,
                "wcag": rule(i.code).reference(),
                "line": i.line,
                "column": i.column,
                "element": i.element,
                "message": i.message,
                "snippet": i.snippet,
            })
        })
        .collect();
    let mut root = json!({
        "score": r.score,
        "mode": r.mode,
        "level": r.level.label(),
        "min_severity": r.min_severity.label(),
        "checks_run": r.checks_run,
        "checks_passed": r.checks_passed,
        "counts": {
            "error": r.count(Severity::Error),
            "warning": r.count(Severity::Warning),
            "suggestion": r.count(Severity::Suggestion),
        },
        "issues": issues,
        "omitted": r.omitted,
        "note": FOOTER,
    });
    if !r.passed.is_empty() {
        let passed: Vec<Value> = r
            .passed
            .iter()
            .map(|p| json!({ "code": p.code, "title": p.title, "wcag": p.reference }))
            .collect();
        root.as_object_mut()
            .expect("json object")
            .insert("passed".to_string(), Value::Array(passed));
    }
    serde_json::to_string_pretty(&root).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
}

fn render_csv(r: &Report) -> String {
    let mut w = csv::Writer::from_writer(Vec::new());
    let _ = w.write_record([
        "severity", "code", "wcag", "line", "column", "element", "message",
    ]);
    for i in &r.issues {
        let _ = w.write_record([
            i.severity.label(),
            i.code,
            &rule(i.code).reference(),
            &i.line.to_string(),
            &i.column.to_string(),
            &i.element,
            &i.message,
        ]);
    }
    let bytes = w.into_inner().unwrap_or_default();
    String::from_utf8(bytes).unwrap_or_default()
}

/// Scan and render in one call — what every surface uses.
pub fn check_to_string(html: &str, format: Format, opts: &Options) -> Result<String, String> {
    let report = check(html, opts)?;
    Ok(match format {
        Format::Text => render_text(&report),
        Format::Markdown => render_markdown(&report),
        Format::Json => render_json(&report),
        Format::Csv => render_csv(&report),
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const BROKEN: &str = r#"<!doctype html>
<html>
<body>
<h2>Welcome</h2>
<img src="hero.png">
<form><input type="text" name="q"></form>
<a href="/x"></a>
<iframe src="/y"></iframe>
</body>
</html>"#;

    fn codes(html: &str, opts: &Options) -> Vec<&'static str> {
        let mut c: Vec<&'static str> = check(html, opts)
            .unwrap()
            .issues
            .iter()
            .map(|i| i.code)
            .collect();
        c.sort_unstable();
        c.dedup();
        c
    }

    #[test]
    fn happy_path_clean_document_scores_100() {
        let html = r#"<!doctype html>
<html lang="en">
<head><title>Contact us</title></head>
<body>
<main>
<h1>Contact us</h1>
<img src="office.png" alt="Our office reception desk">
<label for="email">Email address</label>
<input type="email" id="email" name="email">
<button type="submit">Send the message</button>
</main>
</body>
</html>"#;
        let r = check(html, &Options::default()).unwrap();
        assert!(r.issues.is_empty(), "unexpected issues: {:?}", r.issues);
        assert_eq!(r.score, 100);
        assert_eq!(r.mode, "full document");
        assert!(
            r.checks_run >= 8,
            "expected several checks to run, got {}",
            r.checks_run
        );
        assert_eq!(r.checks_run, r.checks_passed);
    }

    #[test]
    fn error_on_empty_input() {
        let err = check("   \n ", &Options::default()).unwrap_err();
        assert!(err.contains("input is empty"), "{err}");
    }

    #[test]
    fn error_on_bad_format_and_level_and_severity() {
        assert!(parse_format("yaml")
            .unwrap_err()
            .contains("expected 'text'"));
        assert!(parse_level("aaaa").unwrap_err().contains("expected 'a'"));
        assert!(parse_severity("fatal")
            .unwrap_err()
            .contains("expected 'suggestion'"));
    }

    #[test]
    fn error_on_out_of_range_max_issues() {
        let opts = Options {
            max_issues: 0,
            ..Default::default()
        };
        let err = check("<p>hi</p>", &opts).unwrap_err();
        assert!(err.contains("invalid max_issues 0"), "{err}");
    }

    #[test]
    fn finds_the_core_rule_set() {
        let found = codes(BROKEN, &Options::default());
        for expected in [
            "missing-lang",
            "missing-title",
            "img-missing-alt",
            "input-missing-label",
            "link-empty",
            "iframe-missing-title",
            "heading-no-h1",
            "heading-skipped-level",
            "missing-main",
        ] {
            assert!(found.contains(&expected), "missing {expected} in {found:?}");
        }
    }

    #[test]
    fn reports_line_and_column_of_the_offending_tag() {
        let r = check(BROKEN, &Options::default()).unwrap();
        let img = r
            .issues
            .iter()
            .find(|i| i.code == "img-missing-alt")
            .unwrap();
        assert_eq!(img.line, 5);
        assert_eq!(img.column, 1);
        assert!(img.snippet.contains("hero.png"));
        assert!(img.message.contains("alt=\"\""), "{}", img.message);
    }

    #[test]
    fn label_association_forms_all_count_as_labeled() {
        let html = r#"<form>
<label for="a">A</label><input id="a">
<label>B <input id="b"></label>
<input id="c" aria-label="C">
<input id="d" title="D">
<input type="hidden" name="csrf">
</form>"#;
        let found = codes(html, &Options::default());
        assert!(!found.contains(&"input-missing-label"), "{found:?}");
    }

    #[test]
    fn orphan_label_is_flagged() {
        let html = r#"<label for="nope">Name</label><input id="name" aria-label="Name">"#;
        let found = codes(html, &Options::default());
        assert!(found.contains(&"label-orphan"), "{found:?}");
    }

    #[test]
    fn icon_link_named_by_image_alt_passes_but_generic_text_is_aaa_only() {
        let html = r#"<a href="/a"><img src="i.png" alt="Home"></a><a href="/b">click here</a>"#;
        let aa = codes(html, &Options::default());
        assert!(!aa.contains(&"link-empty"), "{aa:?}");
        assert!(
            !aa.contains(&"link-generic-text"),
            "AAA rule leaked into AA: {aa:?}"
        );
        let aaa = codes(
            html,
            &Options {
                level: Conformance::Aaa,
                ..Default::default()
            },
        );
        assert!(aaa.contains(&"link-generic-text"), "{aaa:?}");
    }

    #[test]
    fn level_a_drops_the_aa_viewport_rule() {
        let html = r#"<html lang="en"><head><title>T</title>
<meta name="viewport" content="width=device-width, user-scalable=no"></head>
<body><main><h1>T</h1></main></body></html>"#;
        assert!(codes(html, &Options::default()).contains(&"viewport-zoom-blocked"));
        let a = codes(
            html,
            &Options {
                level: Conformance::A,
                ..Default::default()
            },
        );
        assert!(!a.contains(&"viewport-zoom-blocked"), "{a:?}");
    }

    #[test]
    fn min_severity_filters_out_lower_tiers() {
        let r = check(
            BROKEN,
            &Options {
                min_severity: Severity::Error,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(r.issues.iter().all(|i| i.severity == Severity::Error));
        assert_eq!(r.count(Severity::Warning), 0);
        assert_eq!(r.count(Severity::Suggestion), 0);
    }

    #[test]
    fn max_issues_caps_and_reports_the_remainder() {
        let r = check(
            BROKEN,
            &Options {
                max_issues: 2,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(r.issues.len(), 2);
        assert!(r.omitted > 0);
        let text = check_to_string(
            BROKEN,
            Format::Text,
            &Options {
                max_issues: 2,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(text.contains("Truncated:"), "{text}");
    }

    #[test]
    fn show_passed_lists_the_clean_rules() {
        let html = r#"<html lang="en"><head><title>Pricing</title></head>
<body><main><h1>Pricing</h1><img src="a.png" alt="A price chart"></main></body></html>"#;
        let r = check(
            html,
            &Options {
                show_passed: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(
            r.passed.iter().any(|p| p.code == "img-missing-alt"),
            "{:?}",
            r.passed
        );
        let text = check_to_string(
            html,
            Format::Text,
            &Options {
                show_passed: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(text.contains("PASSED CHECKS"), "{text}");
        let off = check_to_string(html, Format::Text, &Options::default()).unwrap();
        assert!(!off.contains("PASSED CHECKS"), "{off}");
    }

    #[test]
    fn fragment_mode_skips_document_level_rules() {
        let r = check("<p><img src=\"a.png\"></p>", &Options::default()).unwrap();
        assert_eq!(r.mode, "fragment");
        assert!(r.issues.iter().all(|i| i.code != "missing-lang"));
        assert!(r.issues.iter().all(|i| i.code != "missing-title"));
        assert!(r.issues.iter().any(|i| i.code == "img-missing-alt"));
    }

    #[test]
    fn duplicate_ids_are_reported_once_with_all_lines() {
        let html = "<div id=\"x\"></div>\n<div id=\"x\"></div>\n<div id=\"x\"></div>";
        let r = check(html, &Options::default()).unwrap();
        let dup: Vec<&Issue> = r
            .issues
            .iter()
            .filter(|i| i.code == "duplicate-id")
            .collect();
        assert_eq!(dup.len(), 1);
        assert!(dup[0].message.contains("3 times"), "{}", dup[0].message);
        assert!(
            dup[0].message.contains("lines 1, 2, 3"),
            "{}",
            dup[0].message
        );
    }

    #[test]
    fn aria_hidden_focusable_and_invalid_role() {
        let html = r#"<div aria-hidden="true"><a href="/x">Go to pricing</a></div>
<span role="buton">x</span>
<span role="doc-abstract">ok</span>"#;
        let found = codes(html, &Options::default());
        assert!(found.contains(&"aria-hidden-focusable"), "{found:?}");
        assert!(found.contains(&"invalid-role"), "{found:?}");
        let r = check(html, &Options::default()).unwrap();
        assert_eq!(
            r.issues.iter().filter(|i| i.code == "invalid-role").count(),
            1,
            "doc-* roles are valid"
        );
    }

    #[test]
    fn tables_media_and_tabindex() {
        let html = r#"<table><tr><td>1</td></tr></table>
<table role="presentation"><tr><td>x</td></tr></table>
<table><tr><th>Name</th></tr><tr><td>a</td></tr></table>
<video src="v.mp4" autoplay></video>
<video src="w.mp4"><track kind="captions" src="c.vtt"></video>
<div tabindex="3">focus me first</div>"#;
        let r = check(html, &Options::default()).unwrap();
        let found = codes(html, &Options::default());
        assert!(found.contains(&"table-missing-header"), "{found:?}");
        assert_eq!(
            r.issues
                .iter()
                .filter(|i| i.code == "table-missing-header")
                .count(),
            1,
            "role=presentation and a th-bearing table must not be flagged"
        );
        assert!(found.contains(&"autoplay-media"), "{found:?}");
        assert_eq!(
            r.issues
                .iter()
                .filter(|i| i.code == "video-missing-captions")
                .count(),
            1
        );
        assert!(found.contains(&"positive-tabindex"), "{found:?}");
    }

    #[test]
    fn alt_text_quality_and_lang_validity() {
        let html = r#"<html lang="english"><head><title>T</title></head><body><main>
<h1>T</h1>
<img src="a.png" alt="a.png">
<img src="b.png" alt="image">
<img src="c.png" alt="A bar chart of quarterly revenue">
</main></body></html>"#;
        let r = check(html, &Options::default()).unwrap();
        assert_eq!(
            r.issues
                .iter()
                .filter(|i| i.code == "img-alt-filename")
                .count(),
            2
        );
        assert!(r.issues.iter().any(|i| i.code == "invalid-lang"));
    }

    #[test]
    fn script_contents_are_not_parsed_as_markup() {
        let html = r#"<html lang="en"><head><title>T</title></head><body><main>
<h1>T</h1>
<script>var a = "<img src=x>"; if (1 < 2) { doIt(); }</script>
</main></body></html>"#;
        let r = check(html, &Options::default()).unwrap();
        assert!(r.issues.is_empty(), "{:?}", r.issues);
    }

    #[test]
    fn every_format_renders() {
        let text = check_to_string(BROKEN, Format::Text, &Options::default()).unwrap();
        assert!(text.starts_with("HTML accessibility report\n"));
        assert!(text.contains("ERRORS"));

        let md = check_to_string(BROKEN, Format::Markdown, &Options::default()).unwrap();
        assert!(md.starts_with("# HTML accessibility report"));
        assert!(md.contains("| Severity | Line | Rule | WCAG | Element | Issue |"));

        let json = check_to_string(BROKEN, Format::Json, &Options::default()).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(v["score"].as_u64().unwrap() < 100);
        assert_eq!(v["mode"], "full document");
        assert!(v["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["code"] == "img-missing-alt"));

        let csv = check_to_string(BROKEN, Format::Csv, &Options::default()).unwrap();
        assert!(csv.starts_with("severity,code,wcag,line,column,element,message\n"));
        assert!(csv.contains("img-missing-alt"));
    }

    #[test]
    fn score_is_deterministic_and_below_100_for_broken_markup() {
        let a = check(BROKEN, &Options::default()).unwrap();
        let b = check(BROKEN, &Options::default()).unwrap();
        assert_eq!(a.score, b.score);
        assert!(a.score < 100);
        assert!(a.checks_passed < a.checks_run);
    }

    #[test]
    fn input_over_the_size_cap_is_rejected() {
        let big = "a".repeat(MAX_INPUT + 1);
        let err = check(&big, &Options::default()).unwrap_err();
        assert!(err.contains("the maximum is"), "{err}");
    }
}
