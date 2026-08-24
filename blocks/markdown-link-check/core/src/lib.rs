//! markdown-link-check core — pure compute, shared by the chat skill block and the web page.
//!
//! A dependency-free, fully OFFLINE structural link checker for Markdown. It scans a
//! document for every link and image, classifies each target, and reports the problems
//! that can be decided without touching the network:
//!
//!   - malformed link syntax (empty target, unclosed `(`, reversed `(text)[url]`,
//!     a space between the text and the URL);
//!   - reference-style problems (undefined reference, duplicate definition, unused
//!     definition);
//!   - broken in-document anchors (`#section` with no matching heading id);
//!   - hygiene warnings (image with no alt text, unencoded space in a URL, malformed
//!     `mailto:`, optionally insecure `http://`).
//!
//! Live HTTP status of external URLs and on-disk existence of relative paths are
//! deliberately NOT attempted — there is no network or filesystem on the page/wasm
//! surface, so every result here is deterministic and reproducible.
//!
//! Fenced code blocks and inline code spans are masked before scanning, so link-like
//! text inside a snippet never produces a finding.

pub const MAX_BYTES: usize = 1_000_000;

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warn,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warn => "warn",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Anchor,
    External,
    Relative,
    Mailto,
    Image,
    Empty,
    Reference,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Anchor => "anchor",
            Kind::External => "external",
            Kind::Relative => "relative",
            Kind::Mailto => "mailto",
            Kind::Image => "image",
            Kind::Empty => "empty",
            Kind::Reference => "reference",
        }
    }
}

/// One finding. `line`/`col` are 1-based, counted in characters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub line: usize,
    pub col: usize,
    pub rule: &'static str,
    pub severity: Severity,
    pub message: String,
}

/// One link or image found in the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub line: usize,
    pub col: usize,
    pub kind: Kind,
    /// Link text, or the alt text for an image.
    pub text: String,
    /// The resolved destination (a reference link resolves through its definition).
    pub target: String,
    pub issues: Vec<Issue>,
}

impl Link {
    /// `error` if any issue is an error, `warn` if only warnings, else `ok`.
    pub fn status(&self) -> &'static str {
        if self.issues.iter().any(|i| i.severity == Severity::Error) {
            "error"
        } else if self.issues.is_empty() {
            "ok"
        } else {
            "warn"
        }
    }
}

/// Everything the scan learned about a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Analysis {
    pub links: Vec<Link>,
    /// Findings about reference definitions (not tied to a single link site).
    pub doc_issues: Vec<Issue>,
    /// Heading ids the document exposes, in document order.
    pub anchors: Vec<String>,
}

// ---------------------------------------------------------------------------
// Small character helpers
// ---------------------------------------------------------------------------

/// Index of the `close` that balances the `open` at `start` (which must be `open`).
/// Backslash escapes are skipped. `None` if it is never closed on this slice.
fn find_close(chars: &[char], start: usize, open: char, close: char) -> Option<usize> {
    let mut depth = 0usize;
    let mut i = start;
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' {
            i += 2;
            continue;
        }
        if c == open {
            depth += 1;
        } else if c == close {
            depth -= 1;
            if depth == 0 {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

/// Blank out inline code spans (`` `x` ``) with spaces so columns are preserved and
/// link-like text inside code is never scanned.
fn mask_code_spans(chars: &[char]) -> Vec<char> {
    let mut out = chars.to_vec();
    let mut i = 0;
    while i < out.len() {
        if out[i] == '`' {
            let start = i;
            let mut n = 0;
            while i < out.len() && out[i] == '`' {
                n += 1;
                i += 1;
            }
            // Look for a closing run of exactly the same length.
            let mut j = i;
            while j < out.len() {
                if out[j] == '`' {
                    let run_start = j;
                    let mut m = 0;
                    while j < out.len() && out[j] == '`' {
                        m += 1;
                        j += 1;
                    }
                    if m == n {
                        for slot in out.iter_mut().take(j).skip(start) {
                            *slot = ' ';
                        }
                        i = j;
                        break;
                    }
                    let _ = run_start;
                } else {
                    j += 1;
                }
            }
            if j >= out.len() {
                // Unterminated span — leave the rest of the line as-is.
                break;
            }
        } else {
            i += 1;
        }
    }
    out
}

/// Strip Markdown link/image syntax down to its visible text (used for heading slugs).
fn unlink(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '!' && i + 1 < chars.len() && chars[i + 1] == '[' {
            i += 1;
            continue;
        }
        if chars[i] == '[' {
            if let Some(close) = find_close(&chars, i, '[', ']') {
                let inner: String = chars[i + 1..close].iter().collect();
                let mut j = close + 1;
                if j < chars.len() && (chars[j] == '(' || chars[j] == '[') {
                    let (o, c) = if chars[j] == '(' {
                        ('(', ')')
                    } else {
                        ('[', ']')
                    };
                    if let Some(cl) = find_close(&chars, j, o, c) {
                        j = cl + 1;
                    }
                }
                out.push_str(&unlink(&inner));
                i = j;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// GitHub-style heading slug: lower-cased, non-alphanumerics dropped, spaces → `-`,
/// `_` and `-` kept.
fn slugify(text: &str) -> String {
    let mut s = String::new();
    for c in unlink(text).chars() {
        if c.is_alphanumeric() {
            for l in c.to_lowercase() {
                s.push(l);
            }
        } else if c == ' ' || c == '\t' || c == '-' {
            s.push('-');
        } else if c == '_' {
            s.push('_');
        }
    }
    s
}

/// Reference labels are case-insensitive with runs of whitespace collapsed.
fn normalize_label(label: &str) -> String {
    let mut s = String::new();
    let mut prev_space = false;
    for c in label.trim().chars() {
        if c.is_whitespace() {
            if !prev_space {
                s.push(' ');
            }
            prev_space = true;
        } else {
            for l in c.to_lowercase() {
                s.push(l);
            }
            prev_space = false;
        }
    }
    s
}

fn is_fence(trimmed: &str) -> Option<char> {
    if trimmed.starts_with("```") {
        Some('`')
    } else if trimmed.starts_with("~~~") {
        Some('~')
    } else {
        None
    }
}

/// ATX heading → the text after the hashes (closing hashes stripped).
fn atx_heading(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    if line.len() - trimmed.len() > 3 || !trimmed.starts_with('#') {
        return None;
    }
    let hashes = trimmed.chars().take_while(|&c| c == '#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest = &trimmed[hashes..];
    if !rest.is_empty() && !rest.starts_with(' ') && !rest.starts_with('\t') {
        return None;
    }
    let mut text = rest.trim().to_string();
    // Strip a closing run of hashes (`## Title ##`).
    let stripped = text.trim_end_matches('#');
    if stripped.len() != text.len() && (stripped.is_empty() || stripped.ends_with(' ')) {
        text = stripped.trim_end().to_string();
    }
    Some(text)
}

/// `## Title {#custom-id}` → (`Title`, Some("custom-id")).
fn split_custom_id(text: &str) -> (String, Option<String>) {
    let t = text.trim_end();
    if let Some(open) = t.rfind("{#") {
        if t.ends_with('}') {
            let id = &t[open + 2..t.len() - 1];
            if !id.is_empty() && !id.contains(char::is_whitespace) {
                return (t[..open].trim_end().to_string(), Some(id.to_string()));
            }
        }
    }
    (t.to_string(), None)
}

/// Collect every `id="…"` / `name="…"` attribute value on a raw HTML line.
fn html_ids(line: &str, out: &mut Vec<String>) {
    for attr in ["id=", "name="] {
        let mut rest = line;
        while let Some(pos) = rest.find(attr) {
            rest = &rest[pos + attr.len()..];
            let quote = match rest.chars().next() {
                Some(q @ ('"' | '\'')) => q,
                _ => continue,
            };
            if let Some(end) = rest[1..].find(quote) {
                let val = &rest[1..1 + end];
                if !val.is_empty() {
                    out.push(val.to_string());
                }
                rest = &rest[1 + end..];
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Reference definitions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct RefDef {
    label: String,
    url: String,
    line: usize,
    col: usize,
    used: bool,
}

/// `[label]: url "title"` at the start of a line (≤3 spaces of indent).
fn parse_ref_def(chars: &[char]) -> Option<(String, String, usize)> {
    let indent = chars.iter().take_while(|c| **c == ' ').count();
    if indent > 3 || chars.get(indent) != Some(&'[') {
        return None;
    }
    let close = find_close(chars, indent, '[', ']')?;
    if chars.get(close + 1) != Some(&':') {
        return None;
    }
    let label: String = chars[indent + 1..close].iter().collect();
    if label.trim().is_empty() {
        return None;
    }
    let rest: String = chars[close + 2..].iter().collect();
    let rest = rest.trim();
    let url = rest
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim_start_matches('<')
        .trim_end_matches('>')
        .to_string();
    Some((label, url, indent + 1))
}

// ---------------------------------------------------------------------------
// Classification
// ---------------------------------------------------------------------------

fn has_scheme(target: &str) -> bool {
    let mut chars = target.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() => {}
        _ => return false,
    }
    let mut seen = 1;
    for c in chars {
        if c == ':' {
            return seen > 1;
        }
        if c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.' {
            seen += 1;
        } else {
            return false;
        }
    }
    false
}

fn looks_like_email(s: &str) -> bool {
    let mut parts = s.splitn(2, '@');
    let local = parts.next().unwrap_or("");
    let domain = match parts.next() {
        Some(d) => d,
        None => return false,
    };
    !local.is_empty()
        && !domain.is_empty()
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !domain.contains(char::is_whitespace)
        && !local.contains(char::is_whitespace)
}

fn classify(target: &str, is_image: bool) -> Kind {
    if is_image {
        return Kind::Image;
    }
    if target.is_empty() {
        return Kind::Empty;
    }
    if target.starts_with('#') {
        return Kind::Anchor;
    }
    let lower = target.to_ascii_lowercase();
    if lower.starts_with("mailto:") || (!target.contains('/') && looks_like_email(target)) {
        return Kind::Mailto;
    }
    if has_scheme(target) {
        return Kind::External;
    }
    Kind::Relative
}

// ---------------------------------------------------------------------------
// Scan
// ---------------------------------------------------------------------------

/// A raw link site found by the line scanner, before target resolution.
struct Raw {
    line: usize,
    col: usize,
    is_image: bool,
    text: String,
    /// `Some(target)` for an inline link/autolink, `None` for a reference link.
    inline_target: Option<String>,
    /// Set for reference links.
    reference: Option<String>,
    /// Angle-bracketed inline destination (`<a b>`), which legitimately allows spaces.
    bracketed: bool,
    syntax: Vec<Issue>,
}

fn scan_line(line_no: usize, chars: &[char], out: &mut Vec<Raw>) {
    let n = chars.len();
    let mut i = 0;
    while i < n {
        let c = chars[i];

        // Autolink: <https://example.com> or <user@example.com>
        if c == '<' {
            if let Some(end) = chars[i..].iter().position(|&x| x == '>').map(|p| i + p) {
                let inner: String = chars[i + 1..end].iter().collect();
                if !inner.is_empty()
                    && !inner.contains(char::is_whitespace)
                    && (has_scheme(&inner) || looks_like_email(&inner))
                {
                    out.push(Raw {
                        line: line_no,
                        col: i + 1,
                        is_image: false,
                        text: inner.clone(),
                        inline_target: Some(inner),
                        reference: None,
                        bracketed: true,
                        syntax: Vec::new(),
                    });
                    i = end + 1;
                    continue;
                }
            }
            i += 1;
            continue;
        }

        // Reversed link syntax: (text)[url]
        if c == '(' {
            if let Some(pclose) = find_close(chars, i, '(', ')') {
                if chars.get(pclose + 1) == Some(&'[') {
                    if let Some(bclose) = find_close(chars, pclose + 1, '[', ']') {
                        let inner: String = chars[pclose + 2..bclose].iter().collect();
                        if !inner.is_empty() && !inner.contains(char::is_whitespace) {
                            out.push(Raw {
                                line: line_no,
                                col: i + 1,
                                is_image: false,
                                text: chars[i + 1..pclose].iter().collect(),
                                inline_target: Some(inner),
                                reference: None,
                                bracketed: false,
                                syntax: vec![Issue {
                                    line: line_no,
                                    col: i + 1,
                                    rule: "ML008",
                                    severity: Severity::Error,
                                    message:
                                        "reversed link syntax (text)[url] — Markdown needs [text](url)"
                                            .into(),
                                }],
                            });
                            i = bclose + 1;
                            continue;
                        }
                    }
                }
            }
            i += 1;
            continue;
        }

        // Links and images: [text](url) / ![alt](url) / [text][ref] / [text][]
        let is_image = c == '!' && chars.get(i + 1) == Some(&'[');
        if c == '[' || is_image {
            let start = i;
            let bracket = if is_image { i + 1 } else { i };
            let close = match find_close(chars, bracket, '[', ']') {
                Some(x) => x,
                None => {
                    i += 1;
                    continue;
                }
            };
            let text: String = chars[bracket + 1..close].iter().collect();
            let mut j = close + 1;

            // `[text] (url)` — the space breaks the link.
            let mut gap = j;
            while gap < n && chars[gap] == ' ' {
                gap += 1;
            }
            if gap > j && chars.get(gap) == Some(&'(') {
                if let Some(pclose) = find_close(chars, gap, '(', ')') {
                    out.push(Raw {
                        line: line_no,
                        col: start + 1,
                        is_image,
                        text,
                        inline_target: Some(chars[gap + 1..pclose].iter().collect()),
                        reference: None,
                        bracketed: false,
                        syntax: vec![Issue {
                            line: line_no,
                            col: start + 1,
                            rule: "ML013",
                            severity: Severity::Error,
                            message:
                                "space between the link text and the URL — this renders as literal text"
                                    .into(),
                        }],
                    });
                    i = pclose + 1;
                    continue;
                }
            }

            if chars.get(j) == Some(&'(') {
                match find_close(chars, j, '(', ')') {
                    Some(pclose) => {
                        let inside: String = chars[j + 1..pclose].iter().collect();
                        let inside = inside.trim();
                        let bracketed = inside.starts_with('<');
                        let dest = if bracketed {
                            inside
                                .trim_start_matches('<')
                                .split('>')
                                .next()
                                .unwrap_or("")
                                .to_string()
                        } else {
                            // The destination runs to the first whitespace, and what
                            // follows must be a quoted title. If it isn't, the "title"
                            // is really an unencoded space in the URL — keep it whole so
                            // ML009 can say so.
                            match inside.find(char::is_whitespace) {
                                Some(sp) => {
                                    let rest = inside[sp..].trim_start();
                                    if rest.starts_with('"')
                                        || rest.starts_with('\'')
                                        || rest.starts_with('(')
                                    {
                                        inside[..sp].to_string()
                                    } else {
                                        inside.to_string()
                                    }
                                }
                                None => inside.to_string(),
                            }
                        };
                        out.push(Raw {
                            line: line_no,
                            col: start + 1,
                            is_image,
                            text,
                            inline_target: Some(dest),
                            reference: None,
                            bracketed,
                            syntax: Vec::new(),
                        });
                        i = pclose + 1;
                        continue;
                    }
                    None => {
                        out.push(Raw {
                            line: line_no,
                            col: start + 1,
                            is_image,
                            text,
                            inline_target: Some(String::new()),
                            reference: None,
                            bracketed: false,
                            syntax: vec![Issue {
                                line: line_no,
                                col: start + 1,
                                rule: "ML012",
                                severity: Severity::Error,
                                message: "unclosed link syntax — the '(' after the link text is \
                                          never closed on this line"
                                    .into(),
                            }],
                        });
                        i = close + 1;
                        continue;
                    }
                }
            }

            if chars.get(j) == Some(&'[') {
                if let Some(bclose) = find_close(chars, j, '[', ']') {
                    let label: String = chars[j + 1..bclose].iter().collect();
                    let label = if label.trim().is_empty() {
                        text.clone()
                    } else {
                        label
                    };
                    out.push(Raw {
                        line: line_no,
                        col: start + 1,
                        is_image,
                        text,
                        inline_target: None,
                        reference: Some(label),
                        bracketed: false,
                        syntax: Vec::new(),
                    });
                    i = bclose + 1;
                    continue;
                }
            }

            // Shortcut reference `[label]` — only counted when a definition exists,
            // which the caller decides; record it and let resolution drop it.
            out.push(Raw {
                line: line_no,
                col: start + 1,
                is_image,
                text: text.clone(),
                inline_target: None,
                reference: Some(text),
                bracketed: false,
                syntax: Vec::new(),
            });
            j = close + 1;
            i = j;
            continue;
        }

        i += 1;
    }
}

/// Scan `md` and report every structural link problem it can decide offline.
pub fn analyze(md: &str, check_anchors: bool, flag_insecure: bool) -> Result<Analysis, String> {
    let lines: Vec<&str> = md.split('\n').collect();

    // --- pass 1: headings, explicit ids, reference definitions ---
    let mut anchors: Vec<String> = Vec::new();
    let mut slug_counts: Vec<(String, usize)> = Vec::new();
    let mut defs: Vec<RefDef> = Vec::new();
    let mut doc_issues: Vec<Issue> = Vec::new();
    let mut def_lines: Vec<bool> = vec![false; lines.len()];
    let mut in_fence: Option<char> = None;
    let mut fenced: Vec<bool> = vec![false; lines.len()];

    let register = |text: &str, anchors: &mut Vec<String>, counts: &mut Vec<(String, usize)>| {
        let (plain, custom) = split_custom_id(text);
        let base = slugify(&plain);
        if !base.is_empty() {
            let slug = match counts.iter_mut().find(|(s, _)| *s == base) {
                Some((_, n)) => {
                    *n += 1;
                    format!("{base}-{}", *n)
                }
                None => {
                    counts.push((base.clone(), 0));
                    base
                }
            };
            anchors.push(slug);
        }
        if let Some(id) = custom {
            anchors.push(id);
        }
    };

    for (idx, raw) in lines.iter().enumerate() {
        let trimmed = raw.trim_start();
        match in_fence {
            Some(f) => {
                fenced[idx] = true;
                if is_fence(trimmed) == Some(f) {
                    in_fence = None;
                }
                continue;
            }
            None => {
                if let Some(f) = is_fence(trimmed) {
                    in_fence = Some(f);
                    fenced[idx] = true;
                    continue;
                }
            }
        }

        if let Some(text) = atx_heading(raw) {
            register(&text, &mut anchors, &mut slug_counts);
            continue;
        }
        // Setext heading: `===`/`---` underlining the previous non-blank prose line.
        if idx > 0 && !fenced[idx - 1] {
            let t = trimmed.trim_end();
            let underline = (!t.is_empty() && t.chars().all(|c| c == '='))
                || (t.len() > 1 && t.chars().all(|c| c == '-'));
            if underline {
                let prev = lines[idx - 1].trim();
                if !prev.is_empty() && atx_heading(lines[idx - 1]).is_none() {
                    register(prev, &mut anchors, &mut slug_counts);
                    continue;
                }
            }
        }

        html_ids(raw, &mut anchors);

        let chars: Vec<char> = raw.chars().collect();
        if let Some((label, url, col)) = parse_ref_def(&chars) {
            def_lines[idx] = true;
            let norm = normalize_label(&label);
            if defs.iter().any(|d| d.label == norm) {
                doc_issues.push(Issue {
                    line: idx + 1,
                    col,
                    rule: "ML005",
                    severity: Severity::Error,
                    message: format!(
                        "duplicate reference definition [{label}] — the first definition wins, \
                         this one is ignored"
                    ),
                });
            } else {
                defs.push(RefDef {
                    label: norm,
                    url,
                    line: idx + 1,
                    col,
                    used: false,
                });
            }
        }
    }

    // --- pass 2: links ---
    let mut raws: Vec<Raw> = Vec::new();
    for (idx, raw) in lines.iter().enumerate() {
        if fenced[idx] || def_lines[idx] {
            continue;
        }
        let masked = mask_code_spans(&raw.chars().collect::<Vec<char>>());
        scan_line(idx + 1, &masked, &mut raws);
    }

    // --- pass 3: resolve + judge ---
    let mut links: Vec<Link> = Vec::new();
    for r in raws {
        let mut issues = r.syntax;
        let (target, kind) = match (&r.inline_target, &r.reference) {
            (Some(t), _) => {
                let k = classify(t, r.is_image);
                (t.clone(), k)
            }
            (None, Some(label)) => {
                let norm = normalize_label(label);
                match defs.iter_mut().find(|d| d.label == norm) {
                    Some(d) => {
                        d.used = true;
                        let url = d.url.clone();
                        let k = classify(&url, r.is_image);
                        (url, k)
                    }
                    None => {
                        // A bare `[label]` with no definition is ordinary prose, not a link.
                        if r.text == *label && !r.is_image {
                            continue;
                        }
                        issues.push(Issue {
                            line: r.line,
                            col: r.col,
                            rule: "ML004",
                            severity: Severity::Error,
                            message: format!(
                                "undefined reference [{label}] — no matching [{label}]: definition \
                                 in this document"
                            ),
                        });
                        (format!("[{label}]"), Kind::Reference)
                    }
                }
            }
            (None, None) => (String::new(), Kind::Empty),
        };

        let has_syntax_error = issues.iter().any(|i| i.severity == Severity::Error);

        if !has_syntax_error {
            if target.is_empty() {
                issues.push(Issue {
                    line: r.line,
                    col: r.col,
                    rule: "ML001",
                    severity: Severity::Error,
                    message: "empty link target — the () destination is missing".into(),
                });
            } else if target.contains(' ') && !r.bracketed {
                issues.push(Issue {
                    line: r.line,
                    col: r.col,
                    rule: "ML009",
                    severity: Severity::Warn,
                    message: format!(
                        "unencoded space in target '{target}' — use %20 or wrap the URL in <>"
                    ),
                });
            }

            if r.is_image {
                if r.text.trim().is_empty() {
                    issues.push(Issue {
                        line: r.line,
                        col: r.col,
                        rule: "ML003",
                        severity: Severity::Warn,
                        message: "image has no alt text — add a description for screen readers"
                            .into(),
                    });
                }
            } else if r.text.trim().is_empty() {
                issues.push(Issue {
                    line: r.line,
                    col: r.col,
                    rule: "ML002",
                    severity: Severity::Warn,
                    message: "empty link text — there is nothing for a reader to click".into(),
                });
            }

            if kind == Kind::Mailto {
                let addr = target
                    .strip_prefix("mailto:")
                    .or_else(|| target.strip_prefix("MAILTO:"))
                    .unwrap_or(&target);
                let addr = addr.split('?').next().unwrap_or(addr);
                if !looks_like_email(addr) {
                    issues.push(Issue {
                        line: r.line,
                        col: r.col,
                        rule: "ML010",
                        severity: Severity::Error,
                        message: format!(
                            "malformed mail address '{addr}' — expected name@host.tld"
                        ),
                    });
                }
            }

            if check_anchors && kind == Kind::Anchor {
                let frag = &target[1..];
                if !frag.is_empty() {
                    let lower = frag.to_lowercase();
                    let known = anchors
                        .iter()
                        .any(|a| *a == frag || a.to_lowercase() == lower);
                    if !known {
                        issues.push(Issue {
                            line: r.line,
                            col: r.col,
                            rule: "ML007",
                            severity: Severity::Error,
                            message: format!(
                                "broken anchor '#{frag}' — no heading in this document produces \
                                 that id"
                            ),
                        });
                    }
                }
            }

            if flag_insecure && target.to_ascii_lowercase().starts_with("http://") {
                issues.push(Issue {
                    line: r.line,
                    col: r.col,
                    rule: "ML011",
                    severity: Severity::Warn,
                    message: "insecure http:// link — prefer https://".into(),
                });
            }
        }

        links.push(Link {
            line: r.line,
            col: r.col,
            kind,
            text: r.text,
            target,
            issues,
        });
    }

    for d in &defs {
        if !d.used {
            doc_issues.push(Issue {
                line: d.line,
                col: d.col,
                rule: "ML006",
                severity: Severity::Warn,
                message: "unused reference definition — no link in this document refers to it"
                    .into(),
            });
        }
    }
    doc_issues.sort_by_key(|i| (i.line, i.col, i.rule));

    Ok(Analysis {
        links,
        doc_issues,
        anchors,
    })
}

// ---------------------------------------------------------------------------
// Reporting
// ---------------------------------------------------------------------------

fn kind_matches(filter: &str, kind: Kind) -> bool {
    filter == "all" || filter == kind.as_str()
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
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

fn md_escape(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

struct Selection {
    links: Vec<Link>,
    issues: Vec<Issue>,
    errors: usize,
    warnings: usize,
}

fn select(analysis: &Analysis, link_kind: &str) -> Selection {
    let links: Vec<Link> = analysis
        .links
        .iter()
        .filter(|l| kind_matches(link_kind, l.kind))
        .cloned()
        .collect();
    let mut issues: Vec<Issue> = links.iter().flat_map(|l| l.issues.clone()).collect();
    if link_kind == "all" || link_kind == "reference" {
        issues.extend(analysis.doc_issues.iter().cloned());
    }
    issues.sort_by(|a, b| (a.line, a.col, a.rule).cmp(&(b.line, b.col, b.rule)));
    let errors = issues
        .iter()
        .filter(|i| i.severity == Severity::Error)
        .count();
    let warnings = issues.len() - errors;
    Selection {
        links,
        issues,
        errors,
        warnings,
    }
}

fn render_text(sel: &Selection, show_ok: bool) -> String {
    let mut s = String::new();
    if !sel.issues.is_empty() {
        s.push_str("Issues\n");
        for i in &sel.issues {
            s.push_str(&format!(
                "  {}:{}  {}  {}  {}\n",
                i.line,
                i.col,
                i.severity.as_str(),
                i.rule,
                i.message
            ));
        }
        s.push('\n');
    }
    if show_ok && !sel.links.is_empty() {
        s.push_str("Links\n");
        for l in &sel.links {
            s.push_str(&format!(
                "  {}:{}  {}  {}  [{}] -> {}\n",
                l.line,
                l.col,
                l.kind.as_str(),
                l.status(),
                l.text,
                if l.target.is_empty() {
                    "(empty)"
                } else {
                    &l.target
                }
            ));
        }
        s.push('\n');
    }
    if sel.issues.is_empty() {
        s.push_str(&format!(
            "No link problems found — {} link(s) checked.",
            sel.links.len()
        ));
    } else {
        s.push_str(&format!(
            "{} error(s), {} warning(s) in {} link(s) checked.",
            sel.errors,
            sel.warnings,
            sel.links.len()
        ));
    }
    s
}

fn render_markdown(sel: &Selection, show_ok: bool) -> String {
    let mut s = String::new();
    if sel.issues.is_empty() {
        s.push_str("No link problems found.\n\n");
    } else {
        s.push_str("| Line | Severity | Rule | Message |\n| --- | --- | --- | --- |\n");
        for i in &sel.issues {
            s.push_str(&format!(
                "| {}:{} | {} | {} | {} |\n",
                i.line,
                i.col,
                i.severity.as_str(),
                i.rule,
                md_escape(&i.message)
            ));
        }
        s.push('\n');
    }
    if show_ok && !sel.links.is_empty() {
        s.push_str("| Line | Kind | Status | Text | Target |\n| --- | --- | --- | --- | --- |\n");
        for l in &sel.links {
            s.push_str(&format!(
                "| {}:{} | {} | {} | {} | {} |\n",
                l.line,
                l.col,
                l.kind.as_str(),
                l.status(),
                md_escape(&l.text),
                md_escape(&l.target)
            ));
        }
        s.push('\n');
    }
    s.push_str(&format!(
        "**{} error(s), {} warning(s)** in {} link(s) checked.",
        sel.errors,
        sel.warnings,
        sel.links.len()
    ));
    s
}

fn render_json(sel: &Selection, show_ok: bool) -> String {
    let mut s = String::from("{\n");
    s.push_str(&format!("  \"checked\": {},\n", sel.links.len()));
    s.push_str(&format!("  \"errors\": {},\n", sel.errors));
    s.push_str(&format!("  \"warnings\": {},\n", sel.warnings));
    s.push_str("  \"issues\": [");
    for (n, i) in sel.issues.iter().enumerate() {
        s.push_str(if n == 0 { "\n" } else { ",\n" });
        s.push_str(&format!(
            "    {{ \"line\": {}, \"col\": {}, \"severity\": \"{}\", \"rule\": \"{}\", \"message\": \"{}\" }}",
            i.line,
            i.col,
            i.severity.as_str(),
            i.rule,
            json_escape(&i.message)
        ));
    }
    s.push_str(if sel.issues.is_empty() {
        "],\n"
    } else {
        "\n  ],\n"
    });
    s.push_str("  \"links\": [");
    let shown: Vec<&Link> = sel
        .links
        .iter()
        .filter(|l| show_ok || !l.issues.is_empty())
        .collect();
    for (n, l) in shown.iter().enumerate() {
        s.push_str(if n == 0 { "\n" } else { ",\n" });
        s.push_str(&format!(
            "    {{ \"line\": {}, \"col\": {}, \"kind\": \"{}\", \"status\": \"{}\", \"text\": \"{}\", \"target\": \"{}\" }}",
            l.line,
            l.col,
            l.kind.as_str(),
            l.status(),
            json_escape(&l.text),
            json_escape(&l.target)
        ));
    }
    s.push_str(if shown.is_empty() { "]\n}" } else { "\n  ]\n}" });
    s
}

/// Top-level entry shared by every surface.
pub fn run(
    markdown: &str,
    link_kind: &str,
    report_format: &str,
    show_ok: bool,
    check_anchors: bool,
    flag_insecure: bool,
) -> Result<String, String> {
    if markdown.trim().is_empty() {
        return Err("no Markdown input".into());
    }
    if markdown.len() > MAX_BYTES {
        return Err(format!(
            "input is {} bytes; the limit is {} bytes (1 MB)",
            markdown.len(),
            MAX_BYTES
        ));
    }
    let link_kind = if link_kind.trim().is_empty() {
        "all"
    } else {
        link_kind.trim()
    };
    if !matches!(
        link_kind,
        "all" | "anchor" | "external" | "relative" | "mailto" | "image" | "empty" | "reference"
    ) {
        return Err(format!(
            "unknown link_kind '{link_kind}' (use all, anchor, external, relative, mailto, image, \
             empty or reference)"
        ));
    }
    let report_format = if report_format.trim().is_empty() {
        "text"
    } else {
        report_format.trim()
    };

    let analysis = analyze(markdown, check_anchors, flag_insecure)?;
    let sel = select(&analysis, link_kind);

    match report_format {
        "text" => Ok(render_text(&sel, show_ok)),
        "markdown" => Ok(render_markdown(&sel, show_ok)),
        "json" => Ok(render_json(&sel, show_ok)),
        other => Err(format!(
            "unknown report_format '{other}' (use text, markdown or json)"
        )),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn check(md: &str) -> Analysis {
        analyze(md, true, false).unwrap()
    }

    fn rules(md: &str) -> Vec<&'static str> {
        let a = check(md);
        let mut r: Vec<&'static str> = a
            .links
            .iter()
            .flat_map(|l| l.issues.iter().map(|i| i.rule))
            .collect();
        r.extend(a.doc_issues.iter().map(|i| i.rule));
        r.sort_unstable();
        r
    }

    #[test]
    fn clean_document_has_no_issues() {
        let md =
            "# Install\n\nSee [the guide](https://example.com/guide) and [Install](#install).\n";
        assert!(rules(md).is_empty(), "{:?}", rules(md));
    }

    #[test]
    fn empty_input_is_an_error() {
        assert!(run("   ", "all", "text", false, true, false).is_err());
    }

    #[test]
    fn over_cap_input_is_an_error() {
        let big = "a".repeat(MAX_BYTES + 1);
        let err = run(&big, "all", "text", false, true, false).unwrap_err();
        assert!(err.contains("limit is"), "{err}");
    }

    #[test]
    fn at_cap_boundary_is_accepted() {
        let mut md = String::from("# H\n\n[a](https://example.com)\n");
        md.push_str(&"x".repeat(MAX_BYTES - md.len()));
        assert_eq!(md.len(), MAX_BYTES);
        assert!(run(&md, "all", "text", false, true, false).is_ok());
    }

    #[test]
    fn broken_anchor_is_reported() {
        let md = "# Getting Started\n\n[jump](#instalation)\n";
        assert_eq!(rules(md), vec!["ML007"]);
    }

    #[test]
    fn anchor_matches_github_slug_with_punctuation() {
        let md = "## Why `gizza`, though?\n\n[why](#why-gizza-though)\n";
        assert!(rules(md).is_empty(), "{:?}", rules(md));
    }

    #[test]
    fn duplicate_headings_get_numbered_anchors() {
        let md = "# Notes\n\n# Notes\n\n[a](#notes) [b](#notes-1) [c](#notes-2)\n";
        assert_eq!(rules(md), vec!["ML007"]);
    }

    #[test]
    fn setext_and_custom_id_headings_register_anchors() {
        let md = "Overview\n========\n\n## Deep dive {#deep}\n\n[a](#overview) [b](#deep) [c](#deep-dive)\n";
        assert!(rules(md).is_empty(), "{:?}", rules(md));
    }

    #[test]
    fn html_anchor_targets_are_honoured() {
        let md = "<a id=\"manual\"></a>\n\n[go](#manual)\n";
        assert!(rules(md).is_empty(), "{:?}", rules(md));
    }

    #[test]
    fn empty_target_and_empty_text_are_reported() {
        let md = "# H\n\n[docs]() and [](https://example.com)\n";
        assert_eq!(rules(md), vec!["ML001", "ML002"]);
    }

    #[test]
    fn image_without_alt_text_warns() {
        let md = "# H\n\n![](logo.png)\n";
        assert_eq!(rules(md), vec!["ML003"]);
    }

    #[test]
    fn undefined_reference_is_reported() {
        let md = "# H\n\nSee [the docs][guide].\n";
        assert_eq!(rules(md), vec!["ML004"]);
    }

    #[test]
    fn duplicate_reference_definition_is_reported() {
        let md = "# H\n\n[a][x]\n\n[x]: https://example.com/one\n[x]: https://example.com/two\n";
        assert_eq!(rules(md), vec!["ML005"]);
    }

    #[test]
    fn unused_reference_definition_warns() {
        let md = "# H\n\nNo links here.\n\n[orphan]: https://example.com\n";
        assert_eq!(rules(md), vec!["ML006"]);
    }

    #[test]
    fn collapsed_and_shortcut_references_resolve() {
        let md = "# H\n\n[guide][] and [guide] again.\n\n[guide]: https://example.com\n";
        assert!(rules(md).is_empty(), "{:?}", rules(md));
    }

    #[test]
    fn bare_brackets_are_not_links() {
        let md = "# H\n\nStatus: [WIP] and [TODO] remain.\n";
        let a = check(md);
        assert!(a.links.is_empty(), "{:?}", a.links);
    }

    #[test]
    fn reversed_link_syntax_is_reported() {
        let md = "# H\n\n(the guide)[https://example.com]\n";
        assert_eq!(rules(md), vec!["ML008"]);
    }

    #[test]
    fn space_between_text_and_url_is_reported() {
        let md = "# H\n\n[the guide] (https://example.com)\n";
        assert_eq!(rules(md), vec!["ML013"]);
    }

    #[test]
    fn unclosed_link_syntax_is_reported() {
        let md = "# H\n\n[the guide](https://example.com\n";
        assert_eq!(rules(md), vec!["ML012"]);
    }

    #[test]
    fn unencoded_space_warns_but_angle_brackets_are_fine() {
        let md = "# H\n\n[a](my file.md) and [b](<my file.md>)\n";
        assert_eq!(rules(md), vec!["ML009"]);
    }

    #[test]
    fn malformed_mailto_is_reported() {
        let md = "# H\n\n[mail](mailto:someone-at-example.com) and [ok](mailto:hi@example.com)\n";
        assert_eq!(rules(md), vec!["ML010"]);
    }

    #[test]
    fn insecure_links_only_flagged_when_requested() {
        let md = "# H\n\n[a](http://example.com)\n";
        assert!(analyze(md, true, false)
            .unwrap()
            .links
            .iter()
            .all(|l| l.issues.is_empty()));
        let a = analyze(md, true, true).unwrap();
        assert_eq!(a.links[0].issues[0].rule, "ML011");
    }

    #[test]
    fn code_fences_and_code_spans_are_skipped() {
        let md = "# H\n\n```\n[broken](#nope\n```\n\nInline `[broken](#nope` stays quiet.\n";
        let a = check(md);
        assert!(a.links.is_empty(), "{:?}", a.links);
    }

    #[test]
    fn autolinks_are_classified() {
        let md = "# H\n\n<https://example.com> and <hi@example.com>\n";
        let a = check(md);
        assert_eq!(a.links.len(), 2);
        assert_eq!(a.links[0].kind, Kind::External);
        assert_eq!(a.links[1].kind, Kind::Mailto);
    }

    #[test]
    fn kinds_are_classified() {
        let md = "# H\n\n[a](https://example.com) [b](./rel.md) [c](#h) [d](mailto:hi@example.com) ![e](i.png) [f]()\n";
        let a = check(md);
        let kinds: Vec<Kind> = a.links.iter().map(|l| l.kind).collect();
        assert_eq!(
            kinds,
            vec![
                Kind::External,
                Kind::Relative,
                Kind::Anchor,
                Kind::Mailto,
                Kind::Image,
                Kind::Empty
            ]
        );
    }

    #[test]
    fn anchor_check_can_be_disabled() {
        let md = "# H\n\n[x](#nope)\n";
        assert!(analyze(md, false, false)
            .unwrap()
            .links
            .iter()
            .all(|l| l.issues.is_empty()));
    }

    #[test]
    fn link_kind_filter_narrows_the_report() {
        let md = "# H\n\n[a](#nope) and ![](i.png)\n";
        let out = run(md, "image", "text", false, true, false).unwrap();
        assert!(out.contains("ML003"), "{out}");
        assert!(!out.contains("ML007"), "{out}");
    }

    #[test]
    fn text_report_is_exact() {
        let md = "# Install\n\n[jump](#setup)\n";
        assert_eq!(
            run(md, "all", "text", false, true, false).unwrap(),
            "Issues\n  3:1  error  ML007  broken anchor '#setup' — no heading in this document \
             produces that id\n\n1 error(s), 0 warning(s) in 1 link(s) checked."
        );
    }

    #[test]
    fn clean_text_report_is_exact() {
        let md = "# Install\n\n[jump](#install)\n";
        assert_eq!(
            run(md, "all", "text", false, true, false).unwrap(),
            "No link problems found — 1 link(s) checked."
        );
    }

    #[test]
    fn show_ok_lists_every_link() {
        let md = "# Install\n\n[jump](#install)\n";
        let out = run(md, "all", "text", true, true, false).unwrap();
        assert!(
            out.contains("Links\n  3:1  anchor  ok  [jump] -> #install"),
            "{out}"
        );
    }

    #[test]
    fn markdown_report_is_a_table() {
        let md = "# Install\n\n[jump](#setup)\n";
        let out = run(md, "all", "markdown", false, true, false).unwrap();
        assert!(
            out.starts_with("| Line | Severity | Rule | Message |"),
            "{out}"
        );
        assert!(out.contains("| 3:1 | error | ML007 |"), "{out}");
    }

    #[test]
    fn json_report_parses_as_json_shape() {
        let md = "# Install\n\n[jump](#setup)\n";
        let out = run(md, "all", "json", false, true, false).unwrap();
        assert!(out.contains("\"checked\": 1"), "{out}");
        assert!(out.contains("\"errors\": 1"), "{out}");
        assert!(out.contains("\"rule\": \"ML007\""), "{out}");
        assert!(out.contains("\"kind\": \"anchor\""), "{out}");
    }

    #[test]
    fn unknown_report_format_is_an_error() {
        let err = run("# H\n", "all", "xml", false, true, false).unwrap_err();
        assert!(err.contains("unknown report_format"), "{err}");
    }

    #[test]
    fn unknown_link_kind_is_an_error() {
        let err = run("# H\n", "video", "text", false, true, false).unwrap_err();
        assert!(err.contains("unknown link_kind"), "{err}");
    }
}
