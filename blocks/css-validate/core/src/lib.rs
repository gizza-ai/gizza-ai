//! css-validate core — dependency-free CSS validation shared by the chat block
//! and browser page.

use std::collections::{BTreeMap, HashSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputFormat {
    Report,
    Json,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeverityFilter {
    All,
    Error,
    Warning,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FindingMode {
    Ignore,
    Warn,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Options {
    pub format: OutputFormat,
    pub severity: SeverityFilter,
    pub unknown_properties: FindingMode,
    pub vendor_prefixes: FindingMode,
    pub stats: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            format: OutputFormat::Report,
            severity: SeverityFilter::All,
            unknown_properties: FindingMode::Warn,
            vendor_prefixes: FindingMode::Ignore,
            stats: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Issue {
    pub severity: &'static str,
    pub line: usize,
    pub column: usize,
    pub message: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub rules: usize,
    pub declarations: usize,
    pub unique_properties: usize,
    pub custom_properties: usize,
    pub at_rules: usize,
    pub top_properties: Vec<(String, usize)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    pub valid: bool,
    pub issues: Vec<Issue>,
    pub stats: Stats,
}

#[derive(Clone, Debug)]
struct Block {
    selector: String,
    selector_offset: usize,
    body_start: usize,
    is_keyframes: bool,
}

const STANDARD_PROPERTIES: &[&str] = &[
    "align-content",
    "align-items",
    "align-self",
    "all",
    "animation",
    "animation-delay",
    "animation-direction",
    "animation-duration",
    "animation-fill-mode",
    "animation-iteration-count",
    "animation-name",
    "animation-play-state",
    "animation-timing-function",
    "appearance",
    "aspect-ratio",
    "backdrop-filter",
    "backface-visibility",
    "background",
    "background-attachment",
    "background-blend-mode",
    "background-clip",
    "background-color",
    "background-image",
    "background-origin",
    "background-position",
    "background-repeat",
    "background-size",
    "block-size",
    "border",
    "border-block",
    "border-block-color",
    "border-block-end",
    "border-block-start",
    "border-block-style",
    "border-block-width",
    "border-bottom",
    "border-bottom-color",
    "border-bottom-left-radius",
    "border-bottom-right-radius",
    "border-bottom-style",
    "border-bottom-width",
    "border-collapse",
    "border-color",
    "border-end-end-radius",
    "border-end-start-radius",
    "border-image",
    "border-image-outset",
    "border-image-repeat",
    "border-image-slice",
    "border-image-source",
    "border-image-width",
    "border-inline",
    "border-inline-color",
    "border-inline-end",
    "border-inline-start",
    "border-inline-style",
    "border-inline-width",
    "border-left",
    "border-left-color",
    "border-left-style",
    "border-left-width",
    "border-radius",
    "border-right",
    "border-right-color",
    "border-right-style",
    "border-right-width",
    "border-spacing",
    "border-start-end-radius",
    "border-start-start-radius",
    "border-style",
    "border-top",
    "border-top-color",
    "border-top-left-radius",
    "border-top-right-radius",
    "border-top-style",
    "border-top-width",
    "border-width",
    "bottom",
    "box-decoration-break",
    "box-shadow",
    "box-sizing",
    "break-after",
    "break-before",
    "break-inside",
    "caption-side",
    "caret-color",
    "clear",
    "clip",
    "clip-path",
    "color",
    "color-scheme",
    "column-count",
    "column-fill",
    "column-gap",
    "column-rule",
    "column-rule-color",
    "column-rule-style",
    "column-rule-width",
    "column-span",
    "column-width",
    "columns",
    "contain",
    "content",
    "counter-increment",
    "counter-reset",
    "counter-set",
    "cursor",
    "direction",
    "display",
    "empty-cells",
    "filter",
    "flex",
    "flex-basis",
    "flex-direction",
    "flex-flow",
    "flex-grow",
    "flex-shrink",
    "flex-wrap",
    "float",
    "font",
    "font-family",
    "font-feature-settings",
    "font-kerning",
    "font-language-override",
    "font-optical-sizing",
    "font-palette",
    "font-size",
    "font-size-adjust",
    "font-stretch",
    "font-style",
    "font-synthesis",
    "font-variant",
    "font-variant-alternates",
    "font-variant-caps",
    "font-variant-east-asian",
    "font-variant-ligatures",
    "font-variant-numeric",
    "font-variation-settings",
    "font-weight",
    "gap",
    "grid",
    "grid-area",
    "grid-auto-columns",
    "grid-auto-flow",
    "grid-auto-rows",
    "grid-column",
    "grid-column-end",
    "grid-column-gap",
    "grid-column-start",
    "grid-gap",
    "grid-row",
    "grid-row-end",
    "grid-row-gap",
    "grid-row-start",
    "grid-template",
    "grid-template-areas",
    "grid-template-columns",
    "grid-template-rows",
    "hanging-punctuation",
    "height",
    "hyphenate-character",
    "hyphens",
    "image-orientation",
    "image-rendering",
    "inline-size",
    "inset",
    "inset-block",
    "inset-block-end",
    "inset-block-start",
    "inset-inline",
    "inset-inline-end",
    "inset-inline-start",
    "isolation",
    "justify-content",
    "justify-items",
    "justify-self",
    "left",
    "letter-spacing",
    "line-break",
    "line-height",
    "list-style",
    "list-style-image",
    "list-style-position",
    "list-style-type",
    "margin",
    "margin-block",
    "margin-block-end",
    "margin-block-start",
    "margin-bottom",
    "margin-inline",
    "margin-inline-end",
    "margin-inline-start",
    "margin-left",
    "margin-right",
    "margin-top",
    "mask",
    "mask-border",
    "mask-clip",
    "mask-composite",
    "mask-image",
    "mask-mode",
    "mask-origin",
    "mask-position",
    "mask-repeat",
    "mask-size",
    "max-block-size",
    "max-height",
    "max-inline-size",
    "max-width",
    "min-block-size",
    "min-height",
    "min-inline-size",
    "min-width",
    "mix-blend-mode",
    "object-fit",
    "object-position",
    "offset",
    "offset-anchor",
    "offset-distance",
    "offset-path",
    "offset-position",
    "offset-rotate",
    "opacity",
    "order",
    "orphans",
    "outline",
    "outline-color",
    "outline-offset",
    "outline-style",
    "outline-width",
    "overflow",
    "overflow-anchor",
    "overflow-block",
    "overflow-clip-margin",
    "overflow-inline",
    "overflow-wrap",
    "overflow-x",
    "overflow-y",
    "overscroll-behavior",
    "overscroll-behavior-block",
    "overscroll-behavior-inline",
    "overscroll-behavior-x",
    "overscroll-behavior-y",
    "padding",
    "padding-block",
    "padding-block-end",
    "padding-block-start",
    "padding-bottom",
    "padding-inline",
    "padding-inline-end",
    "padding-inline-start",
    "padding-left",
    "padding-right",
    "padding-top",
    "page-break-after",
    "page-break-before",
    "page-break-inside",
    "paint-order",
    "perspective",
    "perspective-origin",
    "place-content",
    "place-items",
    "place-self",
    "pointer-events",
    "position",
    "print-color-adjust",
    "quotes",
    "resize",
    "right",
    "rotate",
    "row-gap",
    "scale",
    "scroll-behavior",
    "scroll-margin",
    "scroll-margin-block",
    "scroll-margin-block-end",
    "scroll-margin-block-start",
    "scroll-margin-bottom",
    "scroll-margin-inline",
    "scroll-margin-inline-end",
    "scroll-margin-inline-start",
    "scroll-margin-left",
    "scroll-margin-right",
    "scroll-margin-top",
    "scroll-padding",
    "scroll-padding-block",
    "scroll-padding-block-end",
    "scroll-padding-block-start",
    "scroll-padding-bottom",
    "scroll-padding-inline",
    "scroll-padding-inline-end",
    "scroll-padding-inline-start",
    "scroll-padding-left",
    "scroll-padding-right",
    "scroll-padding-top",
    "scroll-snap-align",
    "scroll-snap-stop",
    "scroll-snap-type",
    "scrollbar-color",
    "scrollbar-gutter",
    "scrollbar-width",
    "shape-image-threshold",
    "shape-margin",
    "shape-outside",
    "tab-size",
    "table-layout",
    "text-align",
    "text-align-last",
    "text-combine-upright",
    "text-decoration",
    "text-decoration-color",
    "text-decoration-line",
    "text-decoration-skip-ink",
    "text-decoration-style",
    "text-decoration-thickness",
    "text-emphasis",
    "text-emphasis-color",
    "text-emphasis-position",
    "text-emphasis-style",
    "text-indent",
    "text-justify",
    "text-orientation",
    "text-overflow",
    "text-rendering",
    "text-shadow",
    "text-transform",
    "text-underline-offset",
    "text-underline-position",
    "top",
    "touch-action",
    "transform",
    "transform-box",
    "transform-origin",
    "transform-style",
    "transition",
    "transition-delay",
    "transition-duration",
    "transition-property",
    "transition-timing-function",
    "translate",
    "unicode-bidi",
    "user-select",
    "vertical-align",
    "visibility",
    "white-space",
    "widows",
    "width",
    "will-change",
    "word-break",
    "word-spacing",
    "word-wrap",
    "writing-mode",
    "z-index",
];

const AT_RULES: &[&str] = &[
    "charset",
    "color-profile",
    "container",
    "counter-style",
    "font-face",
    "font-feature-values",
    "font-palette-values",
    "import",
    "keyframes",
    "layer",
    "media",
    "namespace",
    "page",
    "property",
    "scope",
    "starting-style",
    "supports",
];

pub fn parse_format(s: &str) -> Result<OutputFormat, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "report" | "text" => Ok(OutputFormat::Report),
        "json" => Ok(OutputFormat::Json),
        other => Err(format!(
            "invalid format {other:?}: expected 'report' or 'json'"
        )),
    }
}

pub fn parse_severity(s: &str) -> Result<SeverityFilter, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "all" => Ok(SeverityFilter::All),
        "error" | "errors" => Ok(SeverityFilter::Error),
        "warning" | "warnings" => Ok(SeverityFilter::Warning),
        other => Err(format!(
            "invalid severity {other:?}: expected 'all', 'error', or 'warning'"
        )),
    }
}

pub fn parse_mode(s: &str, name: &str) -> Result<FindingMode, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "ignore" => Ok(FindingMode::Ignore),
        "warn" | "warning" => Ok(FindingMode::Warn),
        "error" => Ok(FindingMode::Error),
        other => Err(format!(
            "invalid {name} {other:?}: expected 'ignore', 'warn', or 'error'"
        )),
    }
}

pub fn parse_bool(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "" | "true" | "1" | "on" | "yes"
    )
}

fn mode_severity(mode: FindingMode) -> Option<&'static str> {
    match mode {
        FindingMode::Ignore => None,
        FindingMode::Warn => Some("warning"),
        FindingMode::Error => Some("error"),
    }
}

fn line_col(s: &str, off: usize) -> (usize, usize) {
    let mut line = 1usize;
    let mut col = 1usize;
    for ch in s[..off.min(s.len())].chars() {
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

fn push_issue(
    issues: &mut Vec<Issue>,
    css: &str,
    off: usize,
    severity: &'static str,
    msg: impl Into<String>,
) {
    let (line, column) = line_col(css, off);
    issues.push(Issue {
        severity,
        line,
        column,
        message: msg.into(),
    });
}

fn is_ident(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' || c == '-' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'))
}

fn is_standard_property(name: &str) -> bool {
    STANDARD_PROPERTIES.binary_search(&name).is_ok()
}

fn vendor_prefix(name: &str) -> Option<&'static str> {
    ["-webkit-", "-moz-", "-ms-", "-o-"]
        .into_iter()
        .find(|p| name.starts_with(p))
}

fn at_name(selector: &str) -> String {
    selector
        .trim_start()
        .trim_start_matches('@')
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect::<String>()
        .to_ascii_lowercase()
}

fn check_selector(css: &str, selector: &str, off: usize, issues: &mut Vec<Issue>) {
    let trimmed = selector.trim();
    if trimmed.is_empty() {
        push_issue(issues, css, off, "error", "rule has no selector before `{`");
        return;
    }
    if trimmed.starts_with('@') {
        let name = at_name(trimmed);
        if !AT_RULES.contains(&name.as_str()) {
            push_issue(
                issues,
                css,
                off,
                "warning",
                format!("unknown at-rule `@{name}`"),
            );
        }
        if matches!(
            name.as_str(),
            "media" | "supports" | "container" | "keyframes" | "layer"
        ) && trimmed == format!("@{name}")
        {
            push_issue(
                issues,
                css,
                off,
                "error",
                format!("`@{name}` needs a prelude before its block"),
            );
        }
        return;
    }
    if trimmed.split(',').any(|part| part.trim().is_empty()) {
        push_issue(
            issues,
            css,
            off,
            "error",
            "selector list contains an empty selector around `,`",
        );
    }
    if matches!(trimmed.chars().last(), Some('>' | '+' | '~' | ',')) {
        push_issue(
            issues,
            css,
            off + selector.len().saturating_sub(1),
            "error",
            "selector ends with a combinator or comma",
        );
    }
    if trimmed.matches('[').count() != trimmed.matches(']').count() {
        push_issue(
            issues,
            css,
            off,
            "error",
            "selector has unbalanced `[` and `]`",
        );
    }
    if trimmed.contains(":: ") || trimmed.ends_with(':') || trimmed.contains(" : ") {
        push_issue(
            issues,
            css,
            off,
            "warning",
            "selector may contain a nameless or malformed pseudo-class",
        );
    }
}

fn split_declarations(body: &str, base: usize) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut quote: Option<char> = None;
    let mut paren = 0i32;
    for (i, ch) in body.char_indices() {
        if let Some(q) = quote {
            if ch == q {
                quote = None;
            }
            continue;
        }
        match ch {
            '\'' | '"' => quote = Some(ch),
            '(' => paren += 1,
            ')' => paren -= 1,
            ';' if paren <= 0 => {
                out.push((base + start, body[start..i].to_string()));
                start = i + 1;
                paren = 0;
            }
            _ => {}
        }
    }
    if body[start..].trim().is_empty() {
        return out;
    }
    out.push((base + start, body[start..].to_string()));
    out
}

fn find_colon(decl: &str) -> Option<usize> {
    let mut quote: Option<char> = None;
    let mut paren = 0i32;
    for (i, ch) in decl.char_indices() {
        if let Some(q) = quote {
            if ch == q {
                quote = None;
            }
            continue;
        }
        match ch {
            '\'' | '"' => quote = Some(ch),
            '(' => paren += 1,
            ')' => paren -= 1,
            ':' if paren <= 0 => return Some(i),
            _ => {}
        }
    }
    None
}

fn check_value(
    css: &str,
    value: &str,
    off: usize,
    prop: &str,
    declared_vars: &HashSet<String>,
    issues: &mut Vec<Issue>,
) {
    if value.contains("! important") || value.contains("!important!") || value.contains("!imporant")
    {
        push_issue(
            issues,
            css,
            off,
            "error",
            format!("`{prop}` has malformed `!important`"),
        );
    }
    for token in value.split(|c: char| c.is_whitespace() || matches!(c, ')' | '(' | ',')) {
        if token.starts_with('#') && token.len() > 1 {
            let hex = &token[1..];
            if !matches!(hex.len(), 3 | 4 | 6 | 8) || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
                push_issue(
                    issues,
                    css,
                    off,
                    "error",
                    format!("`{prop}` contains malformed hex color `{token}`"),
                );
            }
        }
    }
    let mut search = value;
    let mut delta = 0usize;
    while let Some(pos) = search.find("calc(") {
        let abs = off + delta + pos;
        let rest = &search[pos + 5..];
        match rest.find(')') {
            Some(end) => {
                let inner = &rest[..end];
                if inner.trim().is_empty() || inner.trim_end().ends_with(['+', '-', '*', '/']) {
                    push_issue(
                        issues,
                        css,
                        abs,
                        "error",
                        format!("`{prop}` has malformed `calc()` expression"),
                    );
                }
                if (inner.contains('+') || inner.contains('-'))
                    && !inner.contains(" + ")
                    && !inner.contains(" - ")
                    && !inner.trim_start().starts_with('-')
                {
                    push_issue(
                        issues,
                        css,
                        abs,
                        "warning",
                        format!("`{prop}` calc() should space `+` and `-` operators"),
                    );
                }
                delta += pos + 5 + end + 1;
                search = &search[pos + 5 + end + 1..];
            }
            None => {
                push_issue(
                    issues,
                    css,
                    abs,
                    "error",
                    format!("`{prop}` has unclosed `calc(`"),
                );
                break;
            }
        }
    }
    let mut rest = value;
    let mut value_delta = 0usize;
    while let Some(pos) = rest.find("var(") {
        let abs = off + value_delta + pos;
        let after = &rest[pos + 4..];
        if let Some(end) = after.find(')') {
            let name = after[..end].split(',').next().unwrap_or("").trim();
            if name.starts_with("--") && !declared_vars.contains(name) {
                push_issue(
                    issues,
                    css,
                    abs,
                    "warning",
                    format!("custom property `{name}` is referenced before it is declared"),
                );
            }
            value_delta += pos + 4 + end + 1;
            rest = &rest[pos + 4 + end + 1..];
        } else {
            push_issue(
                issues,
                css,
                abs,
                "error",
                format!("`{prop}` has unclosed `var(`"),
            );
            break;
        }
    }
}

fn check_missing_semicolon(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    STANDARD_PROPERTIES
        .iter()
        .any(|p| lower.contains(&format!(" {p}:")) || lower.contains(&format!("\n{p}:")))
}

fn parse_declarations(
    css: &str,
    body_start: usize,
    body_end: usize,
    keyframes_level: bool,
    options: &Options,
    issues: &mut Vec<Issue>,
    stats: &mut Stats,
    prop_counts: &mut BTreeMap<String, usize>,
    declared_vars: &mut HashSet<String>,
) {
    let body = &css[body_start..body_end];
    for (frag_off, raw) in split_declarations(body, body_start) {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        let leading = raw.len() - raw.trim_start().len();
        let off = frag_off + leading;
        if keyframes_level && trimmed.ends_with('}') {
            continue;
        }
        let Some(colon) = find_colon(trimmed) else {
            push_issue(
                issues,
                css,
                off,
                "error",
                format!(
                    "declaration `{}` is missing `:`",
                    trimmed.lines().next().unwrap_or(trimmed)
                ),
            );
            continue;
        };
        let prop = trimmed[..colon].trim().to_ascii_lowercase();
        let value = trimmed[colon + 1..].trim();
        if prop.is_empty() {
            push_issue(
                issues,
                css,
                off,
                "error",
                "declaration has an empty property name",
            );
            continue;
        }
        if !is_ident(&prop) {
            push_issue(
                issues,
                css,
                off,
                "error",
                format!("`{prop}` is not a valid CSS property name"),
            );
        }
        if value.is_empty() {
            push_issue(
                issues,
                css,
                off + colon + 1,
                "error",
                format!("`{prop}` has an empty value"),
            );
        }
        stats.declarations += 1;
        *prop_counts.entry(prop.clone()).or_insert(0) += 1;
        if prop.starts_with("--") {
            stats.custom_properties += 1;
            declared_vars.insert(prop.clone());
        } else if let Some(prefix) = vendor_prefix(&prop) {
            if let Some(sev) = mode_severity(options.vendor_prefixes) {
                push_issue(
                    issues,
                    css,
                    off,
                    sev,
                    format!("vendor-prefixed property `{prop}` uses `{prefix}`"),
                );
            }
        } else if !is_standard_property(&prop) {
            if let Some(sev) = mode_severity(options.unknown_properties) {
                push_issue(
                    issues,
                    css,
                    off,
                    sev,
                    format!("unknown CSS property `{prop}`"),
                );
            }
        }
        if check_missing_semicolon(value) {
            push_issue(issues, css, off + colon + 1, "warning", format!("`{prop}` value appears to contain another declaration; a semicolon may be missing"));
        }
        check_value(css, value, off + colon + 1, &prop, declared_vars, issues);
    }
}

pub fn validate(css: &str, options: &Options) -> Result<Report, String> {
    if css.trim().is_empty() {
        return Err("input is empty: paste CSS to validate".into());
    }
    let bytes = css.as_bytes();
    let mut i = 0usize;
    let mut text_start = 0usize;
    let mut stack: Vec<Block> = Vec::new();
    let mut quote: Option<u8> = None;
    let mut paren = 0i32;
    let mut issues = Vec::new();
    let mut stats = Stats::default();
    let mut prop_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut declared_vars: HashSet<String> = HashSet::new();

    while i < bytes.len() {
        let b = bytes[i];
        if let Some(q) = quote {
            if b == b'\\' {
                i += 2;
                continue;
            }
            if b == q {
                quote = None;
            }
            i += 1;
            continue;
        }
        if b == b'/' && bytes.get(i + 1) == Some(&b'*') {
            match css[i + 2..].find("*/") {
                Some(end) => {
                    i += 2 + end + 2;
                    continue;
                }
                None => {
                    push_issue(
                        &mut issues,
                        css,
                        i,
                        "error",
                        "unterminated comment: `/*` is never closed with `*/`",
                    );
                    break;
                }
            }
        }
        match b {
            b'\'' | b'"' => {
                quote = Some(b);
                i += 1;
            }
            b'(' => {
                paren += 1;
                i += 1;
            }
            b')' => {
                if paren == 0 {
                    push_issue(&mut issues, css, i, "error", "unmatched closing `)`");
                } else {
                    paren -= 1;
                }
                i += 1;
            }
            b'{' if paren == 0 => {
                let selector = css[text_start..i].trim().to_string();
                let selector_off = text_start
                    + css[text_start..i]
                        .len()
                        .saturating_sub(css[text_start..i].trim_start().len());
                check_selector(css, &selector, selector_off, &mut issues);
                if selector.trim_start().starts_with('@') {
                    stats.at_rules += 1;
                } else {
                    stats.rules += 1;
                }
                let name = at_name(&selector);
                stack.push(Block {
                    selector,
                    selector_offset: selector_off,
                    body_start: i + 1,
                    is_keyframes: name == "keyframes",
                });
                text_start = i + 1;
                i += 1;
            }
            b'}' if paren == 0 => {
                if let Some(block) = stack.pop() {
                    let keyframes_level = stack.last().map(|b| b.is_keyframes).unwrap_or(false);
                    if !block.selector.trim_start().starts_with('@') && !keyframes_level {
                        parse_declarations(
                            css,
                            block.body_start,
                            i,
                            false,
                            options,
                            &mut issues,
                            &mut stats,
                            &mut prop_counts,
                            &mut declared_vars,
                        );
                    }
                    text_start = i + 1;
                } else {
                    push_issue(
                        &mut issues,
                        css,
                        i,
                        "error",
                        "stray closing `}` with no matching `{`",
                    );
                    text_start = i + 1;
                }
                i += 1;
            }
            _ => i += 1,
        }
    }
    if quote.is_some() {
        push_issue(
            &mut issues,
            css,
            css.len().saturating_sub(1),
            "error",
            "unterminated quoted string",
        );
    }
    if paren > 0 {
        push_issue(
            &mut issues,
            css,
            css.len().saturating_sub(1),
            "error",
            "unclosed `(` in stylesheet",
        );
    }
    for block in stack.iter().rev() {
        push_issue(
            &mut issues,
            css,
            block.selector_offset,
            "error",
            format!(
                "block `{}` is never closed with `}}`",
                block.selector.trim()
            ),
        );
    }
    if stack.is_empty() {
        let trailing = css[text_start..].trim();
        if !trailing.is_empty() && !trailing.starts_with('@') {
            push_issue(
                &mut issues,
                css,
                text_start
                    + css[text_start..]
                        .len()
                        .saturating_sub(css[text_start..].trim_start().len()),
                "error",
                "stray text outside a CSS rule; expected `selector { ... }`",
            );
        }
    }

    stats.unique_properties = prop_counts.len();
    let mut top: Vec<(String, usize)> = prop_counts.into_iter().collect();
    top.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    top.truncate(5);
    stats.top_properties = top;
    issues.sort_by_key(|i| (i.line, i.column, if i.severity == "error" { 0 } else { 1 }));
    let valid = !issues.iter().any(|i| i.severity == "error");
    Ok(Report {
        valid,
        issues,
        stats,
    })
}

fn filtered_issues<'a>(issues: &'a [Issue], filter: SeverityFilter) -> Vec<&'a Issue> {
    issues
        .iter()
        .filter(|i| match filter {
            SeverityFilter::All => true,
            SeverityFilter::Error => i.severity == "error",
            SeverityFilter::Warning => i.severity == "warning",
        })
        .collect()
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
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

fn render_text(report: &Report, options: &Options) -> String {
    let shown = filtered_issues(&report.issues, options.severity);
    let errors = report
        .issues
        .iter()
        .filter(|i| i.severity == "error")
        .count();
    let warnings = report
        .issues
        .iter()
        .filter(|i| i.severity == "warning")
        .count();
    let mut out = String::new();
    if report.valid && warnings == 0 {
        out.push_str("Valid CSS — no errors or warnings found.");
    } else if report.valid {
        out.push_str(&format!(
            "CSS has warnings: {errors} error(s), {warnings} warning(s)."
        ));
    } else {
        out.push_str(&format!(
            "Invalid CSS: {errors} error(s), {warnings} warning(s)."
        ));
    }
    if options.stats {
        out.push_str(&format!(
            "\nChecked {} rule(s), {} declaration(s), {} unique propertie(s).",
            report.stats.rules, report.stats.declarations, report.stats.unique_properties
        ));
        if !report.stats.top_properties.is_empty() {
            let top = report
                .stats
                .top_properties
                .iter()
                .map(|(p, n)| format!("{p}×{n}"))
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!("\nTop properties: {top}."));
        }
    }
    if shown.is_empty() {
        if !report.issues.is_empty() {
            out.push_str("\nNo issues match the selected severity filter.");
        }
        return out;
    }
    for it in shown {
        out.push_str(&format!(
            "\n\n  {:<7} line {}:{}  {}",
            it.severity, it.line, it.column, it.message
        ));
    }
    out
}

fn render_json(report: &Report, options: &Options) -> String {
    let shown = filtered_issues(&report.issues, options.severity);
    let errors = report
        .issues
        .iter()
        .filter(|i| i.severity == "error")
        .count();
    let warnings = report
        .issues
        .iter()
        .filter(|i| i.severity == "warning")
        .count();
    let mut out = format!(
        "{{\"valid\":{},\"errors\":{},\"warnings\":{},\"rules\":{},\"declarations\":{},\"issues\":[",
        report.valid, errors, warnings, report.stats.rules, report.stats.declarations
    );
    for (idx, it) in shown.iter().enumerate() {
        if idx > 0 {
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
    out.push(']');
    if options.stats {
        out.push_str(&format!(",\"stats\":{{\"unique_properties\":{},\"custom_properties\":{},\"at_rules\":{},\"top_properties\":[", report.stats.unique_properties, report.stats.custom_properties, report.stats.at_rules));
        for (idx, (name, count)) in report.stats.top_properties.iter().enumerate() {
            if idx > 0 {
                out.push(',');
            }
            out.push_str(&format!(
                "{{\"property\":\"{}\",\"count\":{}}}",
                json_escape(name),
                count
            ));
        }
        out.push_str("]}");
    }
    out.push('}');
    out
}

pub fn run_with_options(css: &str, options: Options) -> Result<String, String> {
    let report = validate(css, &options)?;
    Ok(match options.format {
        OutputFormat::Report => render_text(&report, &options),
        OutputFormat::Json => render_json(&report, &options),
    })
}

pub fn run(
    css: &str,
    format: &str,
    severity: &str,
    unknown_properties: &str,
    vendor_prefixes: &str,
    stats: &str,
) -> Result<String, String> {
    let options = Options {
        format: parse_format(format)?,
        severity: parse_severity(severity)?,
        unknown_properties: parse_mode(unknown_properties, "unknown_properties")?,
        vendor_prefixes: parse_mode(vendor_prefixes, "vendor_prefixes")?,
        stats: parse_bool(stats),
    };
    run_with_options(css, options)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_css_reports_success() {
        let out = run(
            ".card { color: #333; margin: 1rem; --gap: 8px; padding: var(--gap); }",
            "report",
            "all",
            "warn",
            "ignore",
            "true",
        )
        .unwrap();
        assert!(out.starts_with("Valid CSS"), "{out}");
        assert!(out.contains("4 declaration"));
    }

    #[test]
    fn finds_multiple_errors_and_warnings() {
        let css = ".bad, { colr: red width: 10px; color: #12zz; }\n.orphan }\n.next { color }";
        let out = run(css, "report", "all", "warn", "ignore", "true").unwrap();
        assert!(out.contains("Invalid CSS"), "{out}");
        assert!(out.contains("empty selector"), "{out}");
        assert!(out.contains("unknown CSS property `colr`"), "{out}");
        assert!(out.contains("semicolon may be missing"), "{out}");
        assert!(out.contains("malformed hex color"), "{out}");
        assert!(out.contains("stray closing `}`"), "{out}");
        assert!(out.contains("missing `:`"), "{out}");
    }

    #[test]
    fn filters_and_modes_work() {
        let css = ".x { -webkit-transform: scale(1); colr: red; }";
        let out = run(css, "report", "error", "error", "warn", "false").unwrap();
        assert!(out.contains("Invalid CSS"));
        assert!(out.contains("unknown CSS property `colr`"));
        assert!(!out.contains("vendor-prefixed"));
    }

    #[test]
    fn json_output_is_machine_readable() {
        let out = run(
            ".x { color: var(--missing); }",
            "json",
            "all",
            "warn",
            "ignore",
            "true",
        )
        .unwrap();
        assert!(out.starts_with("{\"valid\":true"), "{out}");
        assert!(out.contains("\"warnings\":1"), "{out}");
        assert!(out.contains("custom property `--missing`"), "{out}");
        assert!(out.contains("\"stats\""), "{out}");
    }

    #[test]
    fn rejects_empty_and_bad_enum() {
        assert!(run("  ", "report", "all", "warn", "ignore", "true").is_err());
        assert!(run(".x{color:red}", "xml", "all", "warn", "ignore", "true").is_err());
        assert!(run(".x{color:red}", "report", "fatal", "warn", "ignore", "true").is_err());
    }
}
