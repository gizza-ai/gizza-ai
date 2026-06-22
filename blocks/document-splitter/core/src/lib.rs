//! gizza-ai/document-splitter core — split a long Markdown or HTML document into
//! sections, one per top-level heading. Pure-Rust, dependency-free (besides serde
//! for the output struct).

use std::collections::HashMap;

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Section {
    pub filename: String,
    pub title: String,
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Markdown,
    Html,
}

impl Format {
    pub fn parse(s: &str) -> Result<Format, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "markdown" | "md" | "" => Ok(Format::Markdown),
            "html" => Ok(Format::Html),
            other => Err(format!("unknown format '{other}' (use markdown or html)")),
        }
    }
}

fn slug(title: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for c in title.trim().chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    let s = out.trim_matches('-').to_string();
    if s.is_empty() {
        "section".to_string()
    } else {
        s
    }
}

/// Markdown ATX heading level of a line (number of leading '#' followed by a
/// space), or None.
fn md_heading_level(line: &str) -> Option<usize> {
    let t = line.trim_start();
    let hashes = t.chars().take_while(|&c| c == '#').count();
    if hashes >= 1 && hashes <= 6 && t[hashes..].starts_with(' ') {
        Some(hashes)
    } else {
        None
    }
}

fn md_heading_text(line: &str) -> String {
    line.trim_start().trim_start_matches('#').trim().to_string()
}

fn split_markdown(text: &str) -> Vec<(String, String)> {
    let lines: Vec<&str> = text.lines().collect();
    // Top level = the smallest heading level present (default 1).
    let top = lines.iter().filter_map(|l| md_heading_level(l)).min().unwrap_or(1);

    let mut sections: Vec<(String, String)> = Vec::new();
    let mut cur_title: Option<String> = None;
    let mut cur: Vec<&str> = Vec::new();
    let flush = |title: &Option<String>, buf: &mut Vec<&str>, out: &mut Vec<(String, String)>| {
        let content = buf.join("\n");
        if title.is_some() || !content.trim().is_empty() {
            out.push((title.clone().unwrap_or_else(|| "intro".to_string()), content));
        }
        buf.clear();
    };
    for line in lines {
        if md_heading_level(line) == Some(top) {
            flush(&cur_title, &mut cur, &mut sections);
            cur_title = Some(md_heading_text(line));
            cur.push(line);
        } else {
            cur.push(line);
        }
    }
    flush(&cur_title, &mut cur, &mut sections);
    sections
}

/// Find the top-level HTML heading tag present (h1..h6) and split at it.
fn split_html(text: &str) -> Vec<(String, String)> {
    let lower = text.to_lowercase();
    let top = (1..=6).find(|n| lower.contains(&format!("<h{n}"))).unwrap_or(1);
    let open = format!("<h{top}");

    // Collect byte offsets where a top-level heading opens.
    let mut starts: Vec<usize> = Vec::new();
    let mut from = 0;
    while let Some(rel) = lower[from..].find(&open) {
        starts.push(from + rel);
        from += rel + open.len();
    }
    if starts.is_empty() {
        return vec![("intro".to_string(), text.to_string())];
    }

    let mut sections: Vec<(String, String)> = Vec::new();
    // Preamble before the first heading.
    if starts[0] > 0 && !text[..starts[0]].trim().is_empty() {
        sections.push(("intro".to_string(), text[..starts[0]].to_string()));
    }
    for (i, &s) in starts.iter().enumerate() {
        let end = starts.get(i + 1).copied().unwrap_or(text.len());
        let block = &text[s..end];
        let title = html_heading_text(block, top);
        sections.push((title, block.to_string()));
    }
    sections
}

fn html_heading_text(block: &str, level: usize) -> String {
    // Text between the first <hN ...> and </hN>, tags stripped.
    let close = format!("</h{level}>");
    let after_open = block.find('>').map(|i| i + 1).unwrap_or(0);
    let inner_end = block.to_lowercase().find(&close).unwrap_or(block.len());
    let inner = &block[after_open.min(inner_end)..inner_end];
    // strip any nested tags
    let mut out = String::new();
    let mut in_tag = false;
    for c in inner.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.trim().to_string()
}

/// Split `document` into sections, one per top-level heading.
pub fn split(document: &str, format: Format) -> Result<Vec<Section>, String> {
    if document.trim().is_empty() {
        return Err("input is empty".into());
    }
    let ext = match format {
        Format::Markdown => "md",
        Format::Html => "html",
    };
    let raw = match format {
        Format::Markdown => split_markdown(document),
        Format::Html => split_html(document),
    };
    if raw.is_empty() {
        return Err("no sections found".into());
    }

    // Build filenames, de-duplicating with a numeric suffix.
    let mut used: HashMap<String, usize> = HashMap::new();
    let mut out = Vec::with_capacity(raw.len());
    for (i, (title, content)) in raw.into_iter().enumerate() {
        let base = format!("{:02}-{}", i + 1, slug(&title));
        let n = used.entry(base.clone()).or_insert(0);
        let name = if *n == 0 { base.clone() } else { format!("{base}-{n}") };
        *n += 1;
        out.push(Section {
            filename: format!("{name}.{ext}"),
            title,
            content: content.trim_end().to_string(),
        });
    }
    Ok(out)
}

/// Human-readable preview (used by the page).
pub fn preview(document: &str, format: Format) -> Result<String, String> {
    let secs = split(document, format)?;
    let mut out = format!("{} section(s):\n", secs.len());
    for s in &secs {
        out.push_str(&format!("\n===== {} =====\n{}\n", s.filename, s.content));
    }
    Ok(out.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_splits_on_h1() {
        let doc = "# First\nalpha\n\n# Second\nbeta";
        let s = split(doc, Format::Markdown).unwrap();
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].title, "First");
        assert_eq!(s[0].filename, "01-first.md");
        assert!(s[0].content.contains("alpha"));
        assert_eq!(s[1].title, "Second");
    }

    #[test]
    fn preamble_becomes_intro() {
        let doc = "intro text\n\n# A\nbody";
        let s = split(doc, Format::Markdown).unwrap();
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].title, "intro");
        assert!(s[0].content.contains("intro text"));
    }

    #[test]
    fn uses_smallest_heading_level() {
        // No H1; top level is H2.
        let doc = "## One\na\n## Two\nb";
        let s = split(doc, Format::Markdown).unwrap();
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].title, "One");
    }

    #[test]
    fn duplicate_titles_get_suffix() {
        let doc = "# Intro\na\n# Intro\nb";
        let s = split(doc, Format::Markdown).unwrap();
        assert_eq!(s[0].filename, "01-intro.md");
        assert_eq!(s[1].filename, "02-intro.md"); // distinct via the index prefix
    }

    #[test]
    fn html_splits_on_h1() {
        let doc = "<h1>Alpha</h1><p>one</p><h1>Beta</h1><p>two</p>";
        let s = split(doc, Format::Html).unwrap();
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].title, "Alpha");
        assert_eq!(s[0].filename, "01-alpha.html");
        assert!(s[0].content.contains("one"));
    }

    #[test]
    fn errors() {
        assert!(split("", Format::Markdown).is_err());
        assert!(Format::parse("pdf").is_err());
    }
}
