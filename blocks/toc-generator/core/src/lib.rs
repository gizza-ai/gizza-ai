//! toc-generator core — build a linked table of contents from the headings of a
//! Markdown or HTML document. Pure compute, shared by the chat skill block and the
//! web page. No wafer/wasm-bindgen deps.

use std::collections::HashMap;

/// Which source syntax the document is written in.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Source {
    Markdown,
    Html,
}

/// One extracted heading: its level (1-6), display text, and an optional explicit
/// anchor id (only HTML headings can carry an `id` attribute).
struct Heading {
    level: u32,
    text: String,
    id: Option<String>,
}

/// Build a linked table of contents from a Markdown or HTML document.
///
/// * `document` — the source text.
/// * `input_format` — `"auto"` (detect), `"markdown"`, or `"html"`.
/// * `output_format` — `"markdown"` (nested bullet list of links) or `"html"`
///   (nested `<ul>`/`<ol>` of `<a href="#…">` links).
/// * `min_level` / `max_level` — heading levels to include (1-6 inclusive).
/// * `ordered` — number the list (`1.` / `<ol>`) instead of bullets (`-` / `<ul>`).
///
/// Anchors are GitHub-style slugs of the heading text (lowercased, punctuation
/// dropped, spaces → hyphens, duplicates suffixed `-1`, `-2`, …); an HTML heading
/// with an explicit `id` keeps that id so the link matches the real anchor.
pub fn generate(
    document: &str,
    input_format: &str,
    output_format: &str,
    min_level: u32,
    max_level: u32,
    ordered: bool,
) -> Result<String, String> {
    if document.trim().is_empty() {
        return Err("document is empty".into());
    }

    let min = clamp_level(min_level, 1);
    let max = clamp_level(max_level, 6);
    if min > max {
        return Err(format!(
            "min_level ({min}) must be less than or equal to max_level ({max})"
        ));
    }

    let source = match input_format.trim().to_ascii_lowercase().as_str() {
        "" | "auto" => detect_source(document),
        "markdown" | "md" => Source::Markdown,
        "html" | "htm" => Source::Html,
        other => {
            return Err(format!(
                "unknown input_format '{other}' (expected 'auto', 'markdown', or 'html')"
            ))
        }
    };

    let out_md = match output_format.trim().to_ascii_lowercase().as_str() {
        "" | "markdown" | "md" => true,
        "html" | "htm" => false,
        other => {
            return Err(format!(
                "unknown output_format '{other}' (expected 'markdown' or 'html')"
            ))
        }
    };

    let headings = match source {
        Source::Markdown => extract_markdown(document),
        Source::Html => extract_html(document),
    };

    let filtered: Vec<&Heading> = headings
        .iter()
        .filter(|h| h.level >= min && h.level <= max)
        .collect();
    if filtered.is_empty() {
        return Err("no headings found in the document for the selected levels".into());
    }

    // Resolve display text + a unique anchor for each heading.
    let mut seen: HashMap<String, u32> = HashMap::new();
    let mut levels: Vec<u32> = Vec::with_capacity(filtered.len());
    let mut display: Vec<String> = Vec::with_capacity(filtered.len());
    let mut anchors: Vec<String> = Vec::with_capacity(filtered.len());
    for h in &filtered {
        let text = h.text.trim().to_string();
        let base = match &h.id {
            Some(id) if !id.trim().is_empty() => id.trim().to_string(),
            _ => slugify(&text),
        };
        anchors.push(dedupe(&mut seen, base));
        levels.push(h.level);
        display.push(text);
    }

    // Normalize the levels to contiguous depths that step by at most one, so the
    // nesting is sane even when the document skips a level (e.g. h1 then h3).
    let depths = normalize_depths(&levels);

    if out_md {
        Ok(render_markdown(&depths, &display, &anchors, ordered))
    } else {
        Ok(render_html(&depths, &display, &anchors, ordered))
    }
}

fn clamp_level(level: u32, default: u32) -> u32 {
    if level == 0 {
        default
    } else {
        level.clamp(1, 6)
    }
}

/// Treat the input as HTML if it contains a recognizable `<h1>`…`<h6>` tag.
fn detect_source(doc: &str) -> Source {
    if find_html_heading(doc, 0).is_some() {
        Source::Html
    } else {
        Source::Markdown
    }
}

// ---------------------------------------------------------------------------
// Markdown extraction
// ---------------------------------------------------------------------------

fn extract_markdown(doc: &str) -> Vec<Heading> {
    let mut out = Vec::new();
    let lines: Vec<&str> = doc.lines().collect();
    let mut in_fence = false;
    let mut fence_char = ' ';
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim_start();

        // Fenced code blocks (``` or ~~~) — headings inside are ignored.
        if let Some(c) = fence_marker(trimmed) {
            if !in_fence {
                in_fence = true;
                fence_char = c;
            } else if c == fence_char {
                in_fence = false;
            }
            i += 1;
            continue;
        }
        if in_fence {
            i += 1;
            continue;
        }

        // ATX heading: up to 3 leading spaces, then 1-6 '#', then space or EOL.
        if line.len() - trimmed.len() <= 3 {
            if let Some((level, text)) = parse_atx(trimmed) {
                out.push(Heading {
                    level,
                    text,
                    id: None,
                });
                i += 1;
                continue;
            }
        }

        // Setext heading: a non-blank text line underlined by '=' (h1) or '-' (h2).
        if !trimmed.is_empty() && parse_atx(trimmed).is_none() && fence_marker(trimmed).is_none() {
            if let Some(next) = lines.get(i + 1) {
                if let Some(level) = setext_level(next) {
                    out.push(Heading {
                        level,
                        text: trimmed.trim().to_string(),
                        id: None,
                    });
                    i += 2;
                    continue;
                }
            }
        }

        i += 1;
    }

    for h in &mut out {
        h.text = strip_md_inline(&h.text);
    }
    out
}

/// Returns the fence character (`` ` `` or `~`) if the line opens/closes a fence.
fn fence_marker(trimmed: &str) -> Option<char> {
    let bytes = trimmed.as_bytes();
    if bytes.len() >= 3 && (bytes[0] == b'`' || bytes[0] == b'~') {
        let c = bytes[0];
        if bytes[..3].iter().all(|&b| b == c) {
            return Some(c as char);
        }
    }
    None
}

fn parse_atx(trimmed: &str) -> Option<(u32, String)> {
    let hashes = trimmed.chars().take_while(|&c| c == '#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest = &trimmed[hashes..];
    // The '#' run must be followed by a space (or be the whole line).
    if !rest.is_empty() && !rest.starts_with(' ') && !rest.starts_with('\t') {
        return None;
    }
    // Drop an optional closing run of '#'.
    let text = rest.trim().trim_end_matches('#').trim_end().to_string();
    Some((hashes as u32, text))
}

fn setext_level(line: &str) -> Option<u32> {
    let t = line.trim();
    if t.is_empty() {
        return None;
    }
    if t.chars().all(|c| c == '=') {
        Some(1)
    } else if t.chars().all(|c| c == '-') {
        Some(2)
    } else {
        None
    }
}

/// Strip inline Markdown formatting from heading text, leaving plain display text.
fn strip_md_inline(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '\\' => {
                if i + 1 < chars.len() {
                    out.push(chars[i + 1]);
                    i += 2;
                } else {
                    i += 1;
                }
            }
            '`' => i += 1,
            '*' | '_' => i += 1,
            '!' if i + 1 < chars.len() && chars[i + 1] == '[' => i += 1,
            '[' => {
                // Link / image: take the label, skip the (url) or [ref] that follows.
                if let Some(close) = find_char(&chars, i + 1, ']') {
                    let label: String = chars[i + 1..close].iter().collect();
                    out.push_str(&strip_md_inline(&label));
                    let mut j = close + 1;
                    if let Some(&n) = chars.get(j) {
                        if n == '(' {
                            if let Some(p) = find_char(&chars, j + 1, ')') {
                                j = p + 1;
                            }
                        } else if n == '[' {
                            if let Some(p) = find_char(&chars, j + 1, ']') {
                                j = p + 1;
                            }
                        }
                    }
                    i = j;
                } else {
                    out.push(c);
                    i += 1;
                }
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn find_char(chars: &[char], from: usize, target: char) -> Option<usize> {
    chars[from..].iter().position(|&c| c == target).map(|p| from + p)
}

// ---------------------------------------------------------------------------
// HTML extraction
// ---------------------------------------------------------------------------

/// Find the byte index of the next `<h1>`…`<h6>` opening tag at or after `from`.
fn find_html_heading(doc: &str, from: usize) -> Option<usize> {
    let lb = doc.as_bytes();
    let mut i = from;
    while i + 2 < lb.len() {
        if lb[i] == b'<' && (lb[i + 1] == b'h' || lb[i + 1] == b'H') {
            let d = lb[i + 2];
            if (b'1'..=b'6').contains(&d) {
                let after = lb.get(i + 3).copied().unwrap_or(b'>');
                if matches!(after, b'>' | b' ' | b'\t' | b'\n' | b'\r' | b'/') {
                    return Some(i);
                }
            }
        }
        i += 1;
    }
    None
}

fn extract_html(doc: &str) -> Vec<Heading> {
    let mut out = Vec::new();
    let lower = doc.to_ascii_lowercase();
    let mut pos = 0;
    while let Some(start) = find_html_heading(doc, pos) {
        let level = (doc.as_bytes()[start + 2] - b'0') as u32;
        // End of the opening tag.
        let Some(gt_rel) = lower[start..].find('>') else {
            break;
        };
        let gt = start + gt_rel;
        let open_tag = &doc[start..=gt];
        let id = extract_id(open_tag);

        let close = format!("</h{level}");
        let content_start = gt + 1;
        if let Some(rel) = lower[content_start..].find(&close) {
            let content_end = content_start + rel;
            let inner = &doc[content_start..content_end];
            out.push(Heading {
                level,
                text: strip_tags(inner),
                id,
            });
            pos = content_end + close.len();
        } else {
            // No closing tag — take the rest of the document as the heading text.
            let inner = &doc[content_start..];
            out.push(Heading {
                level,
                text: strip_tags(inner),
                id,
            });
            break;
        }
    }
    out
}

/// Pull the `id="…"` attribute value out of an opening tag, if present.
fn extract_id(open_tag: &str) -> Option<String> {
    let lower = open_tag.to_ascii_lowercase();
    let mut search = 0;
    while let Some(rel) = lower[search..].find("id") {
        let idx = search + rel;
        // Must be a standalone `id` attribute: preceded by whitespace / '<'.
        let prev_ok = idx == 0
            || matches!(lower.as_bytes()[idx - 1], b' ' | b'\t' | b'\n' | b'\r' | b'<');
        let j = idx + 2;
        let after = lower[j..].trim_start();
        if prev_ok && after.starts_with('=') {
            let eq = j + lower[j..].find('=').unwrap() + 1;
            let val = open_tag[eq..].trim_start();
            let quote = val.chars().next();
            let value = match quote {
                Some('"') => val[1..].split('"').next().unwrap_or("").to_string(),
                Some('\'') => val[1..].split('\'').next().unwrap_or("").to_string(),
                _ => val
                    .split(|c: char| c.is_whitespace() || c == '>' || c == '/')
                    .next()
                    .unwrap_or("")
                    .to_string(),
            };
            let value = value.trim().to_string();
            if !value.is_empty() {
                return Some(value);
            }
        }
        search = idx + 2;
    }
    None
}

/// Remove HTML tags and decode the handful of common entities, returning plain text.
fn strip_tags(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    decode_entities(&out)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn decode_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ")
}

// ---------------------------------------------------------------------------
// Slugs
// ---------------------------------------------------------------------------

fn slugify(text: &str) -> String {
    let mut s = String::new();
    for c in text.chars() {
        if c.is_alphanumeric() {
            s.extend(c.to_lowercase());
        } else if c == ' ' {
            s.push('-');
        } else if c == '-' || c == '_' {
            s.push(c);
        }
    }
    s
}

fn dedupe(seen: &mut HashMap<String, u32>, base: String) -> String {
    let base = if base.is_empty() {
        "section".to_string()
    } else {
        base
    };
    match seen.get_mut(&base) {
        Some(n) => {
            *n += 1;
            format!("{base}-{n}")
        }
        None => {
            seen.insert(base.clone(), 0);
            base
        }
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

/// Map heading levels to 0-based depths that increase by at most one per step.
fn normalize_depths(levels: &[u32]) -> Vec<usize> {
    let mut depths = Vec::with_capacity(levels.len());
    let mut stack: Vec<u32> = Vec::new();
    for &lvl in levels {
        while let Some(&top) = stack.last() {
            if top >= lvl {
                stack.pop();
            } else {
                break;
            }
        }
        depths.push(stack.len());
        stack.push(lvl);
    }
    depths
}

fn render_markdown(depths: &[usize], display: &[String], anchors: &[String], ordered: bool) -> String {
    let marker = if ordered { "1." } else { "-" };
    let mut lines = Vec::with_capacity(depths.len());
    for ((d, text), anchor) in depths.iter().zip(display).zip(anchors) {
        let indent = "  ".repeat(*d);
        lines.push(format!(
            "{indent}{marker} [{}](#{anchor})",
            escape_md_label(text)
        ));
    }
    lines.join("\n")
}

fn render_html(depths: &[usize], display: &[String], anchors: &[String], ordered: bool) -> String {
    let tag = if ordered { "ol" } else { "ul" };
    let mut out = String::new();
    let mut prev: isize = -1;
    for ((d, text), anchor) in depths.iter().zip(display).zip(anchors) {
        let d = *d as isize;
        if d > prev {
            for _ in 0..(d - prev) {
                out.push_str(&format!("<{tag}>\n"));
            }
        } else {
            out.push_str("</li>\n");
            for _ in 0..(prev - d) {
                out.push_str(&format!("</{tag}>\n</li>\n"));
            }
        }
        out.push_str(&format!("<li><a href=\"#{anchor}\">{}</a>", escape_html(text)));
        prev = d;
    }
    if prev >= 0 {
        out.push_str("</li>\n");
        for _ in 0..prev {
            out.push_str(&format!("</{tag}>\n</li>\n"));
        }
        out.push_str(&format!("</{tag}>"));
    }
    out
}

fn escape_md_label(s: &str) -> String {
    s.replace('\\', "\\\\").replace('[', "\\[").replace(']', "\\]")
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_basic_toc() {
        let doc = "# Title\n## Setup\n## Usage\n### Details";
        let out = generate(doc, "auto", "markdown", 1, 6, false).unwrap();
        assert_eq!(
            out,
            "- [Title](#title)\n  - [Setup](#setup)\n  - [Usage](#usage)\n    - [Details](#details)"
        );
    }

    #[test]
    fn duplicate_headings_get_unique_anchors() {
        let doc = "# Notes\n# Notes\n# Notes";
        let out = generate(doc, "markdown", "markdown", 1, 6, false).unwrap();
        assert_eq!(out, "- [Notes](#notes)\n- [Notes](#notes-1)\n- [Notes](#notes-2)");
    }

    #[test]
    fn level_filter_excludes_out_of_range() {
        let doc = "# A\n## B\n### C\n#### D";
        let out = generate(doc, "markdown", "markdown", 2, 3, false).unwrap();
        assert_eq!(out, "- [B](#b)\n  - [C](#c)");
    }

    #[test]
    fn ordered_markdown_uses_numbers() {
        let doc = "# One\n# Two";
        let out = generate(doc, "markdown", "markdown", 1, 6, true).unwrap();
        assert_eq!(out, "1. [One](#one)\n1. [Two](#two)");
    }

    #[test]
    fn strips_inline_formatting_and_links() {
        let doc = "# **Bold** and `code` and [a link](http://x)";
        let out = generate(doc, "markdown", "markdown", 1, 6, false).unwrap();
        assert_eq!(out, "- [Bold and code and a link](#bold-and-code-and-a-link)");
    }

    #[test]
    fn ignores_headings_inside_code_fences() {
        let doc = "# Real\n```\n# Not A Heading\n```\n## Also Real";
        let out = generate(doc, "markdown", "markdown", 1, 6, false).unwrap();
        assert_eq!(out, "- [Real](#real)\n  - [Also Real](#also-real)");
    }

    #[test]
    fn setext_headings_are_recognized() {
        let doc = "Chapter One\n===========\n\nSection\n-------";
        let out = generate(doc, "markdown", "markdown", 1, 6, false).unwrap();
        assert_eq!(out, "- [Chapter One](#chapter-one)\n  - [Section](#section)");
    }

    #[test]
    fn html_input_uses_existing_ids() {
        let doc = "<h1 id=\"intro\">Introduction</h1><h2>Getting Started</h2>";
        let out = generate(doc, "auto", "markdown", 1, 6, false).unwrap();
        assert_eq!(
            out,
            "- [Introduction](#intro)\n  - [Getting Started](#getting-started)"
        );
    }

    #[test]
    fn html_output_nests_lists() {
        let doc = "# A\n## B\n# C";
        let out = generate(doc, "markdown", "html", 1, 6, false).unwrap();
        assert_eq!(
            out,
            "<ul>\n<li><a href=\"#a\">A</a><ul>\n<li><a href=\"#b\">B</a></li>\n</ul>\n</li>\n<li><a href=\"#c\">C</a></li>\n</ul>"
        );
    }

    #[test]
    fn html_strips_inner_tags_and_entities() {
        let doc = "<h2>Tom &amp; <code>Jerry</code></h2>";
        let out = generate(doc, "html", "markdown", 1, 6, false).unwrap();
        assert_eq!(out, "- [Tom & Jerry](#tom--jerry)");
    }

    #[test]
    fn skipped_level_normalizes_depth() {
        let doc = "# A\n### Deep";
        let out = generate(doc, "markdown", "markdown", 1, 6, false).unwrap();
        assert_eq!(out, "- [A](#a)\n  - [Deep](#deep)");
    }

    #[test]
    fn empty_document_errors() {
        assert!(generate("   ", "auto", "markdown", 1, 6, false).is_err());
    }

    #[test]
    fn no_headings_errors() {
        assert!(generate("just a paragraph", "markdown", "markdown", 1, 6, false).is_err());
    }

    #[test]
    fn min_greater_than_max_errors() {
        assert!(generate("# A", "markdown", "markdown", 4, 2, false).is_err());
    }

    #[test]
    fn invalid_format_errors() {
        assert!(generate("# A", "xml", "markdown", 1, 6, false).is_err());
        assert!(generate("# A", "markdown", "pdf", 1, 6, false).is_err());
    }
}
