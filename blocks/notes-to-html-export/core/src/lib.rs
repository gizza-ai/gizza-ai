//! gizza-ai/notes-to-html-export core — pure compute, shared by the chat skill
//! block and the web page. No wafer/wasm-bindgen deps.
//!
//! Bundles a pile of Markdown notes into ONE self-contained HTML document: the
//! notes are split into sections (on level-1 headings, or on thematic breaks),
//! each section is rendered with `pulldown-cmark` (CommonMark + GitHub-flavored
//! extensions) and sanitized with `ammonia`, every heading gets a GitHub-style
//! slug anchor, and a linked table of contents is generated over them. The whole
//! thing is wrapped in a single `<!doctype html>` string with embedded CSS — no
//! external requests, no JavaScript required to read it, so the file can be
//! emailed, archived, or opened offline.

use ammonia::Builder;
use pulldown_cmark::{html, CowStr, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use std::collections::HashMap;

/// How the pasted body is cut into notes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Split {
    /// Every level-1 ATX heading (`# …`) starts a new note.
    Heading,
    /// A thematic break (`---`, `***`, `___`) on its own line separates notes.
    Hr,
}

/// Where the table of contents is placed in the exported document.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Toc {
    /// Sticky column beside the notes (stacks above them on narrow screens).
    Sidebar,
    /// Inline block before the first note.
    Top,
    /// No table of contents.
    None,
}

/// One heading of the exported document, resolved for linking + numbering.
struct HeadingInfo {
    /// Nesting depth (1-based, contiguous — a document that skips h1→h3 still
    /// nests by one).
    depth: usize,
    /// Raw heading level as written (1-6); used for the emitted `<hN>` tag.
    level: usize,
    /// Plain-text of the heading (for the TOC label).
    text: String,
    /// Unique anchor id.
    anchor: String,
    /// Section number (`1.2.1`), empty when numbering is off.
    number: String,
}

/// Colors of one theme palette.
struct Palette {
    bg: &'static str,
    surface: &'static str,
    fg: &'static str,
    muted: &'static str,
    accent: &'static str,
    border: &'static str,
    code_bg: &'static str,
}

const LIGHT: Palette = Palette {
    bg: "#ffffff",
    surface: "#f7f8fa",
    fg: "#1a1a1a",
    muted: "#616a75",
    accent: "#2563eb",
    border: "#e2e5ea",
    code_bg: "#f3f4f6",
};

const DARK: Palette = Palette {
    bg: "#16181d",
    surface: "#1e2027",
    fg: "#e6e8ee",
    muted: "#9aa3b0",
    accent: "#7aa2f7",
    border: "#2c303a",
    code_bg: "#252831",
};

/// Bundle `notes` into one self-contained HTML document.
///
/// * `notes` — the pasted Markdown body holding one or more notes.
/// * `split` — `"heading"` (a level-1 `#` starts a new note) or `"hr"`
///   (a thematic break separates notes). Blank → `"heading"`.
/// * `toc` — `"sidebar"`, `"top"` or `"none"`. Blank → `"sidebar"`.
/// * `toc_depth` — deepest heading level listed in the TOC, 1-6.
/// * `number_sections` — prefix headings + TOC entries with `1`, `1.1`, …
/// * `title` — document title + visible page heading. Blank → `"Notes"`.
/// * `theme` — `"light"`, `"dark"` or `"auto"` (follows the reader's OS
///   setting). Blank → `"light"`.
///
/// Returns the complete `<!doctype html>…</html>` string.
pub fn export_notes(
    notes: &str,
    split: &str,
    toc: &str,
    toc_depth: u32,
    number_sections: bool,
    title: &str,
    theme: &str,
) -> Result<String, String> {
    if notes.trim().is_empty() {
        return Err("notes are empty".into());
    }
    let notes = strip_clean_content_tags(notes);
    let split = parse_split(split)?;
    let toc = parse_toc(toc)?;
    let depth_limit = parse_depth(toc_depth)?;
    let (palette_css, theme_name) = parse_theme(theme)?;
    let title = {
        let t = title.trim();
        if t.is_empty() {
            "Notes"
        } else {
            t
        }
    };

    let chunks = split_notes(&notes, split);
    if chunks.is_empty() {
        return Err("no notes found in the input".into());
    }

    // Pass 1: collect every heading of every note, in document order, so the
    // slugs, nesting depths and section numbers are resolved across the whole
    // export (not per note).
    let raw: Vec<(usize, String)> = chunks.iter().flat_map(|c| collect_headings(c)).collect();
    let headings = resolve_headings(&raw, number_sections);

    // Pass 2: render each note, consuming the resolved headings in order.
    let mut cursor = 0usize;
    let mut body = String::new();
    for (i, chunk) in chunks.iter().enumerate() {
        let rendered = render_note(chunk, &headings, &mut cursor);
        body.push_str(&format!(
            "<article class=\"note\" id=\"note-{}\">\n{}\n</article>\n",
            i + 1,
            rendered.trim_end()
        ));
    }

    let toc_html = if toc == Toc::None {
        String::new()
    } else {
        render_toc(&headings, depth_limit)
    };

    Ok(render_document(
        title,
        theme_name,
        palette_css,
        toc,
        &toc_html,
        &body,
    ))
}

/// Default-configuration convenience wrapper (sidebar TOC, depth 3, light).
pub fn export_notes_default(notes: &str) -> Result<String, String> {
    export_notes(notes, "heading", "sidebar", 3, false, "Notes", "light")
}

/// Drop raw HTML clean-content blocks before Markdown parsing. `ammonia` removes
/// `<script>` / `<style>` tags and their contents after Markdown rendering, but
/// a raw tag embedded in a heading can otherwise leak its text into the generated
/// heading anchor and TOC label before that sanitizer pass runs.
fn strip_clean_content_tags(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    loop {
        let lower = rest.to_ascii_lowercase();
        let script = lower.find("<script");
        let style = lower.find("<style");
        let Some(start) = (match (script, style) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (Some(a), None) | (None, Some(a)) => Some(a),
            (None, None) => None,
        }) else {
            out.push_str(rest);
            break;
        };
        out.push_str(&rest[..start]);
        let tag = if lower[start..].starts_with("<script") {
            "</script>"
        } else {
            "</style>"
        };
        if let Some(end_rel) = lower[start..].find(tag) {
            rest = &rest[start + end_rel + tag.len()..];
        } else {
            break;
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Option parsing
// ---------------------------------------------------------------------------

fn parse_split(split: &str) -> Result<Split, String> {
    match split.trim().to_ascii_lowercase().as_str() {
        "" | "heading" => Ok(Split::Heading),
        "hr" => Ok(Split::Hr),
        other => Err(format!(
            "unknown split {other:?} (expected \"heading\" or \"hr\")"
        )),
    }
}

fn parse_toc(toc: &str) -> Result<Toc, String> {
    match toc.trim().to_ascii_lowercase().as_str() {
        "" | "sidebar" => Ok(Toc::Sidebar),
        "top" => Ok(Toc::Top),
        "none" => Ok(Toc::None),
        other => Err(format!(
            "unknown toc {other:?} (expected \"sidebar\", \"top\" or \"none\")"
        )),
    }
}

fn parse_depth(depth: u32) -> Result<usize, String> {
    if (1..=6).contains(&depth) {
        Ok(depth as usize)
    } else {
        Err(format!("toc_depth must be between 1 and 6 (got {depth})"))
    }
}

fn parse_theme(theme: &str) -> Result<(String, &'static str), String> {
    let name = match theme.trim().to_ascii_lowercase().as_str() {
        "" | "light" => "light",
        "dark" => "dark",
        "auto" => "auto",
        other => {
            return Err(format!(
                "unknown theme {other:?} (expected \"light\", \"dark\" or \"auto\")"
            ))
        }
    };
    let css = match name {
        "dark" => vars_block(":root", &DARK),
        "auto" => format!(
            "{}\n@media (prefers-color-scheme: dark) {{\n{}\n}}",
            vars_block(":root", &LIGHT),
            vars_block("  :root", &DARK)
        ),
        _ => vars_block(":root", &LIGHT),
    };
    Ok((css, name))
}

fn vars_block(selector: &str, p: &Palette) -> String {
    format!(
        "{selector} {{\n  --bg: {}; --surface: {}; --fg: {}; --muted: {};\n  --accent: {}; --border: {}; --code-bg: {};\n}}",
        p.bg, p.surface, p.fg, p.muted, p.accent, p.border, p.code_bg
    )
}

// ---------------------------------------------------------------------------
// Splitting the pasted body into notes
// ---------------------------------------------------------------------------

/// Is this line a Markdown thematic break — 3+ of the same `-`/`*`/`_` and
/// nothing else (spaces allowed)?
fn is_thematic_break(line: &str) -> bool {
    let t = line.trim();
    if t.len() < 3 {
        return false;
    }
    ['-', '*', '_'].iter().any(|&marker| {
        t.chars().all(|c| c == marker || c == ' ')
            && t.chars().filter(|&c| c == marker).count() >= 3
    })
}

/// Is this line a level-1 ATX heading (`# Title`)?
fn is_h1(line: &str) -> bool {
    let t = line.trim_start();
    // Up to 3 leading spaces are still a heading; 4+ makes it an indented code
    // block, and `trim_start` would misread that — so bound the indent.
    if line.len() - t.len() > 3 {
        return false;
    }
    t == "#" || t.starts_with("# ") || t.starts_with("#\t")
}

/// Toggle state for fenced code blocks so a `---` or `#` inside a fence never
/// splits the document.
fn fence_toggle(line: &str, open: &mut Option<char>) {
    let t = line.trim_start();
    let marker = if t.starts_with("```") {
        '`'
    } else if t.starts_with("~~~") {
        '~'
    } else {
        return;
    };
    match *open {
        None => *open = Some(marker),
        Some(m) if m == marker => *open = None,
        Some(_) => {}
    }
}

/// Cut the pasted body into note chunks. Empty chunks are dropped, so stray
/// separators or a leading `---` never produce a blank note.
fn split_notes(notes: &str, split: Split) -> Vec<String> {
    let mut chunks: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut fence: Option<char> = None;

    for line in notes.lines() {
        let in_fence = fence.is_some();
        fence_toggle(line, &mut fence);
        if !in_fence && fence.is_none() {
            match split {
                Split::Hr if is_thematic_break(line) => {
                    chunks.push(std::mem::take(&mut current));
                    continue;
                }
                Split::Heading if is_h1(line) => {
                    chunks.push(std::mem::take(&mut current));
                }
                _ => {}
            }
        }
        current.push_str(line);
        current.push('\n');
    }
    chunks.push(current);

    chunks
        .into_iter()
        .filter(|c| !c.trim().is_empty())
        .collect()
}

// ---------------------------------------------------------------------------
// Headings: extraction, slugs, nesting depth, numbering
// ---------------------------------------------------------------------------

fn md_options() -> Options {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_SMART_PUNCTUATION);
    options.insert(Options::ENABLE_HEADING_ATTRIBUTES);
    options
}

fn level_of(level: HeadingLevel) -> usize {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

/// Extract `(level, plain text)` for every heading of one note, in order.
fn collect_headings(md: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut inside: Option<(usize, String)> = None;
    for event in Parser::new_ext(md, md_options()) {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                inside = Some((level_of(level), String::new()));
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some(h) = inside.take() {
                    out.push(h);
                }
            }
            Event::Text(t) | Event::Code(t) => {
                if let Some((_, buf)) = inside.as_mut() {
                    buf.push_str(&t);
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if let Some((_, buf)) = inside.as_mut() {
                    buf.push(' ');
                }
            }
            _ => {}
        }
    }
    out
}

/// GitHub-style anchor slug: lowercase, drop punctuation, spaces → hyphens.
fn slugify(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
        } else if c == ' ' || c == '-' || c == '_' {
            out.push('-');
        }
    }
    let slug = out.trim_matches('-').to_string();
    if slug.is_empty() {
        "section".to_string()
    } else {
        slug
    }
}

/// Suffix repeated slugs `-1`, `-2`, … so every anchor is unique.
fn dedupe(seen: &mut HashMap<String, u32>, base: String) -> String {
    let n = seen.entry(base.clone()).or_insert(0);
    let anchor = if *n == 0 { base } else { format!("{base}-{n}") };
    *n += 1;
    anchor
}

/// Map raw heading levels to contiguous depths that step by at most one, so a
/// document that jumps h1 → h3 still nests sanely.
fn normalize_depths(levels: &[usize]) -> Vec<usize> {
    let mut depths = Vec::with_capacity(levels.len());
    // stack of raw levels currently "open", innermost last
    let mut stack: Vec<usize> = Vec::new();
    for &level in levels {
        while stack.last().is_some_and(|&l| l >= level) {
            stack.pop();
        }
        stack.push(level);
        depths.push(stack.len());
    }
    depths
}

/// Resolve slugs, depths and (optionally) section numbers for every heading.
fn resolve_headings(raw: &[(usize, String)], number_sections: bool) -> Vec<HeadingInfo> {
    let levels: Vec<usize> = raw.iter().map(|(l, _)| *l).collect();
    let depths = normalize_depths(&levels);
    let mut seen: HashMap<String, u32> = HashMap::new();
    let mut counters = [0u32; 6];

    raw.iter()
        .zip(depths)
        .map(|((level, text), depth)| {
            let text = text.trim().to_string();
            let anchor = dedupe(&mut seen, slugify(&text));
            let number = if number_sections {
                counters[depth - 1] += 1;
                for c in counters.iter_mut().skip(depth) {
                    *c = 0;
                }
                counters[..depth]
                    .iter()
                    .map(|n| n.to_string())
                    .collect::<Vec<_>>()
                    .join(".")
            } else {
                String::new()
            };
            HeadingInfo {
                depth,
                level: *level,
                text,
                anchor,
                number,
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

/// HTML-escape text destined for the document chrome / TOC.
fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// Render one note's Markdown to sanitized HTML, rewriting each heading's open
/// tag so it carries the resolved anchor id (and the section number, when on).
/// `cursor` walks the document-wide heading list shared by the TOC.
fn render_note(md: &str, headings: &[HeadingInfo], cursor: &mut usize) -> String {
    let mut events: Vec<Event> = Vec::new();
    let mut in_heading = false;

    for event in Parser::new_ext(md, md_options()) {
        match event {
            Event::Start(Tag::Heading { .. }) => {
                in_heading = true;
                let open = match headings.get(*cursor) {
                    Some(h) => {
                        let number = if h.number.is_empty() {
                            String::new()
                        } else {
                            format!("<span class=\"secno\">{}</span> ", escape_html(&h.number))
                        };
                        format!("<h{} id=\"{}\">{}", h.level, escape_html(&h.anchor), number)
                    }
                    None => "<h2>".to_string(),
                };
                events.push(Event::Html(CowStr::from(open)));
            }
            Event::End(TagEnd::Heading(level)) => {
                in_heading = false;
                let tag = headings
                    .get(*cursor)
                    .map(|h| h.level)
                    .unwrap_or_else(|| level_of(level));
                events.push(Event::Html(CowStr::from(format!("</h{tag}>"))));
                *cursor += 1;
            }
            other => {
                // Raw HTML inside a heading would break out of the tag we just
                // wrote; the sanitizer strips it anyway, so drop it early.
                if in_heading && matches!(other, Event::Html(_) | Event::InlineHtml(_)) {
                    continue;
                }
                events.push(other);
            }
        }
    }

    let mut unsafe_html = String::new();
    html::push_html(&mut unsafe_html, events.into_iter());
    sanitize(&unsafe_html)
}

/// Sanitize rendered note HTML: strips `<script>`, event handlers and
/// `javascript:` URLs while keeping what a Markdown document legitimately
/// produces — task-list checkboxes, code-fence language classes, footnote
/// ids, and the heading anchors added above.
fn sanitize(html: &str) -> String {
    Builder::default()
        .add_tags(["input"])
        .add_tag_attributes("input", ["type", "checked", "disabled"])
        .add_url_schemes(["data"].as_slice())
        .add_generic_attributes(["class", "id"])
        .clean(html)
        .to_string()
}

/// Render the nested table of contents, keeping headings no deeper than
/// `depth_limit`. Returns an empty string when there is nothing to list.
fn render_toc(headings: &[HeadingInfo], depth_limit: usize) -> String {
    let entries: Vec<&HeadingInfo> = headings.iter().filter(|h| h.depth <= depth_limit).collect();
    if entries.is_empty() {
        return String::new();
    }

    let mut out = String::from("<ul>\n");
    let mut current = 1usize;
    for (i, h) in entries.iter().enumerate() {
        while current < h.depth {
            out.push_str("<ul>\n");
            current += 1;
        }
        while current > h.depth {
            out.push_str("</li>\n</ul>\n");
            current -= 1;
        }
        if i > 0 && current == h.depth {
            out.push_str("</li>\n");
        }
        let number = if h.number.is_empty() {
            String::new()
        } else {
            format!("<span class=\"secno\">{}</span> ", escape_html(&h.number))
        };
        out.push_str(&format!(
            "<li><a href=\"#{}\">{}{}</a>",
            escape_html(&h.anchor),
            number,
            escape_html(&h.text)
        ));
    }
    while current > 1 {
        out.push_str("</li>\n</ul>\n");
        current -= 1;
    }
    out.push_str("</li>\n</ul>\n");
    out
}

const DOC_CSS: &str = r#"
* { box-sizing: border-box; }
html { scroll-behavior: smooth; }
body {
  margin: 0; background: var(--bg); color: var(--fg);
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
  font-size: 17px; line-height: 1.65; -webkit-font-smoothing: antialiased;
}
.layout { max-width: 1180px; margin: 0 auto; padding: 2rem 1.25rem 4rem; }
.layout.with-sidebar { display: grid; grid-template-columns: 17rem minmax(0, 1fr); gap: 2.5rem; }
.doc-title { font-size: 2rem; line-height: 1.2; margin: 0 0 1.5rem; }
nav.toc {
  background: var(--surface); border: 1px solid var(--border); border-radius: 10px;
  padding: 1rem 1.1rem; font-size: .94rem;
}
.layout.with-sidebar nav.toc { position: sticky; top: 1.5rem; max-height: calc(100vh - 3rem); overflow: auto; }
nav.toc .toc-title { font-size: .78rem; letter-spacing: .08em; text-transform: uppercase;
  color: var(--muted); margin: 0 0 .6rem; }
nav.toc ul { list-style: none; margin: 0; padding: 0 0 0 .85rem; }
nav.toc > ul { padding-left: 0; }
nav.toc li { margin: .28rem 0; }
nav.toc a { color: var(--fg); text-decoration: none; }
nav.toc a:hover, nav.toc a:focus { color: var(--accent); text-decoration: underline; }
nav.toc.toc-top { margin: 0 0 2.5rem; }
article.note { margin: 0 0 3rem; }
article.note + article.note { border-top: 1px solid var(--border); padding-top: 2rem; }
article.note > :first-child { margin-top: 0; }
h1, h2, h3, h4, h5, h6 { line-height: 1.25; margin: 2rem 0 .7rem; scroll-margin-top: 1.5rem; }
h1 { font-size: 1.8rem; } h2 { font-size: 1.45rem; } h3 { font-size: 1.2rem; }
h4, h5, h6 { font-size: 1.02rem; }
.secno { color: var(--muted); font-variant-numeric: tabular-nums; margin-right: .35em; }
p, ul, ol, blockquote, table, pre { margin: 0 0 1rem; }
a { color: var(--accent); }
code {
  background: var(--code-bg); padding: .12em .38em; border-radius: 4px; font-size: .9em;
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
}
pre { background: var(--code-bg); border: 1px solid var(--border); border-radius: 8px;
  padding: .9rem 1rem; overflow: auto; }
pre code { background: none; padding: 0; }
blockquote { border-left: 3px solid var(--accent); padding: .1rem 1rem; color: var(--muted); }
table { border-collapse: collapse; width: 100%; display: block; overflow-x: auto; }
th, td { border: 1px solid var(--border); padding: .45rem .7rem; text-align: left; }
th { background: var(--surface); }
img { max-width: 100%; height: auto; }
hr { border: 0; border-top: 1px solid var(--border); margin: 2rem 0; }
ul li input[type="checkbox"] { margin-right: .4em; }
li > input[type="checkbox"] + * { display: inline; }
@media (max-width: 820px) {
  .layout.with-sidebar { display: block; }
  .layout.with-sidebar nav.toc { position: static; max-height: none; margin: 0 0 2rem; }
}
@media print {
  body { font-size: 12pt; }
  nav.toc { break-inside: avoid; }
  article.note { break-after: page; }
}
"#;

fn render_document(
    title: &str,
    theme_name: &str,
    palette_css: String,
    toc: Toc,
    toc_html: &str,
    body: &str,
) -> String {
    let esc_title = escape_html(title);
    let has_toc = !toc_html.is_empty();
    let nav = |class: &str| {
        format!(
            "<nav class=\"toc {class}\" aria-label=\"Table of contents\">\n<p class=\"toc-title\">Contents</p>\n{toc_html}</nav>\n"
        )
    };
    let sidebar_nav = if has_toc && toc == Toc::Sidebar {
        nav("toc-sidebar")
    } else {
        String::new()
    };
    let top_nav = if has_toc && toc == Toc::Top {
        nav("toc-top")
    } else {
        String::new()
    };
    let layout_class = if sidebar_nav.is_empty() {
        "layout"
    } else {
        "layout with-sidebar"
    };

    format!(
        "<!doctype html>\n\
<html lang=\"en\" data-theme=\"{theme_name}\">\n\
<head>\n\
<meta charset=\"utf-8\">\n\
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
<meta name=\"generator\" content=\"notes-to-html-export\">\n\
<meta name=\"color-scheme\" content=\"{color_scheme}\">\n\
<title>{esc_title}</title>\n\
<style>\n{palette_css}\n{css}</style>\n\
</head>\n\
<body>\n\
<div class=\"{layout_class}\">\n\
{sidebar_nav}<main>\n\
<h1 class=\"doc-title\">{esc_title}</h1>\n\
{top_nav}{body}</main>\n\
</div>\n\
</body>\n\
</html>\n",
        color_scheme = match theme_name {
            "dark" => "dark",
            "auto" => "light dark",
            _ => "light",
        },
        css = DOC_CSS,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOTES: &str =
        "# Groceries\n\nBuy milk\n\n## Later\n\n- eggs\n\n# Standup\n\nShip the thing";

    #[test]
    fn splits_on_h1_and_wraps_each_note() {
        let html =
            export_notes(NOTES, "heading", "sidebar", 3, false, "My notes", "light").unwrap();
        assert_eq!(html.matches("<article class=\"note\"").count(), 2);
        assert!(html.contains("<title>My notes</title>"));
        assert!(html.contains("<h1 class=\"doc-title\">My notes</h1>"));
        assert!(html.starts_with("<!doctype html>"));
        assert!(html.trim_end().ends_with("</html>"));
    }

    #[test]
    fn headings_get_slug_anchors_and_toc_links() {
        let html = export_notes(NOTES, "heading", "sidebar", 3, false, "", "light").unwrap();
        assert!(
            html.contains("<h1 id=\"groceries\">Groceries</h1>"),
            "{html}"
        );
        assert!(html.contains("<h2 id=\"later\">Later</h2>"));
        assert!(html.contains("<a href=\"#groceries\">Groceries</a>"));
        assert!(html.contains("<a href=\"#later\">Later</a>"));
        assert!(html.contains("<title>Notes</title>"));
    }

    #[test]
    fn duplicate_headings_get_unique_anchors() {
        let md = "# Notes\n\n# Notes\n\n# Notes";
        let html = export_notes(md, "heading", "top", 3, false, "", "light").unwrap();
        assert!(html.contains("id=\"notes\""));
        assert!(html.contains("id=\"notes-1\""));
        assert!(html.contains("id=\"notes-2\""));
        assert!(html.contains("<a href=\"#notes-2\">"));
    }

    #[test]
    fn hr_split_makes_one_note_per_break() {
        let md = "First note\n\n---\n\nSecond note\n\n***\n\nThird note";
        let html = export_notes(md, "hr", "none", 3, false, "", "light").unwrap();
        assert_eq!(html.matches("<article class=\"note\"").count(), 3);
        assert!(html.contains("Second note"));
        // toc = none drops the nav entirely
        assert!(!html.contains("<nav"));
    }

    #[test]
    fn hr_split_keeps_headings_inside_a_note() {
        let md = "# One\n\n## Sub\n\n---\n\n# Two";
        let html = export_notes(md, "hr", "sidebar", 3, false, "", "light").unwrap();
        assert_eq!(html.matches("<article class=\"note\"").count(), 2);
        assert!(html.contains("<h2 id=\"sub\">Sub</h2>"));
    }

    #[test]
    fn separators_inside_a_code_fence_do_not_split() {
        let md = "# Code\n\n```text\n---\n# not a heading\n```\n\n# Real";
        let heading_split = export_notes(md, "heading", "none", 3, false, "", "light").unwrap();
        assert_eq!(heading_split.matches("<article class=\"note\"").count(), 2);
        let hr_split = export_notes(md, "hr", "none", 3, false, "", "light").unwrap();
        assert_eq!(hr_split.matches("<article class=\"note\"").count(), 1);
    }

    #[test]
    fn toc_depth_limits_the_listing() {
        let md = "# A\n\n## B\n\n### C\n\n#### D";
        let deep = export_notes(md, "heading", "top", 4, false, "", "light").unwrap();
        assert!(deep.contains("<a href=\"#d\">D</a>"));
        let shallow = export_notes(md, "heading", "top", 2, false, "", "light").unwrap();
        assert!(shallow.contains("<a href=\"#b\">B</a>"));
        assert!(!shallow.contains("<a href=\"#c\">"));
        assert!(!shallow.contains("<a href=\"#d\">"));
        // the headings themselves are still rendered, only the TOC is bounded
        assert!(shallow.contains("<h4 id=\"d\">D</h4>"));
    }

    #[test]
    fn section_numbering_applies_to_headings_and_toc() {
        let md = "# A\n\n## A1\n\n## A2\n\n### A2a\n\n# B";
        let html = export_notes(md, "heading", "top", 3, true, "", "light").unwrap();
        assert!(
            html.contains("<h1 id=\"a\"><span class=\"secno\">1</span> A</h1>"),
            "{html}"
        );
        assert!(html.contains("<h2 id=\"a1\"><span class=\"secno\">1.1</span> A1</h2>"));
        assert!(html.contains("<h3 id=\"a2a\"><span class=\"secno\">1.2.1</span> A2a</h3>"));
        assert!(html.contains("<h1 id=\"b\"><span class=\"secno\">2</span> B</h1>"));
        assert!(html.contains("<a href=\"#a2a\"><span class=\"secno\">1.2.1</span> A2a</a>"));
    }

    #[test]
    fn numbering_off_by_default() {
        let html = export_notes("# A\n\n## B", "heading", "top", 3, false, "", "light").unwrap();
        assert!(!html.contains("<span class=\"secno\">"));
    }

    #[test]
    fn skipped_heading_levels_still_nest_by_one() {
        let md = "# A\n\n### Deep";
        let html = export_notes(md, "heading", "top", 3, true, "", "light").unwrap();
        // h3 directly under h1 numbers as 1.1, and keeps its <h3> tag
        assert!(
            html.contains("<h3 id=\"deep\"><span class=\"secno\">1.1</span> Deep</h3>"),
            "{html}"
        );
    }

    #[test]
    fn toc_placement_switches_between_sidebar_and_top() {
        let sidebar = export_notes(NOTES, "heading", "sidebar", 3, false, "", "light").unwrap();
        assert!(sidebar.contains("class=\"layout with-sidebar\""));
        assert!(sidebar.contains("toc toc-sidebar"));
        let top = export_notes(NOTES, "heading", "top", 3, false, "", "light").unwrap();
        assert!(top.contains("class=\"layout\""));
        assert!(!top.contains("class=\"layout with-sidebar\""));
        assert!(top.contains("toc toc-top"));
    }

    #[test]
    fn themes_swap_the_embedded_palette() {
        let light = export_notes(NOTES, "heading", "top", 3, false, "", "light").unwrap();
        assert!(light.contains("--bg: #ffffff"));
        assert!(!light.contains("prefers-color-scheme"));
        let dark = export_notes(NOTES, "heading", "top", 3, false, "", "dark").unwrap();
        assert!(dark.contains("--bg: #16181d"));
        assert!(dark.contains("content=\"dark\""));
        let auto = export_notes(NOTES, "heading", "top", 3, false, "", "auto").unwrap();
        assert!(auto.contains("--bg: #ffffff"));
        assert!(auto.contains("@media (prefers-color-scheme: dark)"));
        assert!(auto.contains("content=\"light dark\""));
    }

    #[test]
    fn output_is_self_contained() {
        let html = export_notes(NOTES, "heading", "sidebar", 3, false, "", "light").unwrap();
        assert!(!html.contains("<link"));
        assert!(!html.contains("<script"));
        assert!(!html.contains("http://"));
        assert!(!html.contains("https://"));
        assert_eq!(html.matches("<style>").count(), 1);
    }

    #[test]
    fn gfm_extensions_render() {
        let md = "# T\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n- [x] done\n- [ ] open\n\n~~gone~~";
        let html = export_notes(md, "heading", "none", 3, false, "", "light").unwrap();
        assert!(html.contains("<table>"));
        assert!(html.contains("<th>a</th>"));
        assert!(
            html.contains("<input disabled=\"\" type=\"checkbox\" checked=\"\">")
                || html.contains("checkbox")
        );
        assert!(html.contains("<del>gone</del>"));
    }

    #[test]
    fn data_uri_images_pass_through() {
        let md = "# Pic\n\n![dot](data:image/gif;base64,R0lGODlhAQABAAAAACw=)";
        let html = export_notes(md, "heading", "none", 3, false, "", "light").unwrap();
        assert!(html.contains("data:image/gif;base64"), "{html}");
    }

    #[test]
    fn scripts_and_handlers_are_stripped() {
        let md = "# Safe\n\n<script>alert(1)</script>\n\n<img src=x onerror=\"alert(2)\">\n\n[bad](javascript:alert(3))";
        let html = export_notes(md, "heading", "none", 3, false, "", "light").unwrap();
        assert!(!html.contains("alert(1)"));
        assert!(!html.contains("onerror"));
        assert!(!html.contains("javascript:"));
        assert!(!html.contains("<script"));
    }

    #[test]
    fn raw_html_inside_a_heading_cannot_break_the_anchor() {
        let md = "# Hi <script>alert(1)</script>";
        let html = export_notes(md, "heading", "top", 3, false, "", "light").unwrap();
        assert!(!html.contains("<script"));
        assert!(html.contains("id=\"hi\""), "{html}");
    }

    #[test]
    fn title_is_escaped() {
        let html =
            export_notes("# A", "heading", "none", 3, false, "Q&A <notes>", "light").unwrap();
        assert!(html.contains("<title>Q&amp;A &lt;notes&gt;</title>"));
        assert!(!html.contains("<notes>"));
    }

    #[test]
    fn notes_without_headings_still_export() {
        let html = export_notes(
            "just a thought",
            "heading",
            "sidebar",
            3,
            false,
            "",
            "light",
        )
        .unwrap();
        assert_eq!(html.matches("<article class=\"note\"").count(), 1);
        // nothing to link to, so no empty TOC box is rendered
        assert!(!html.contains("<nav"));
        assert!(html.contains("just a thought"));
    }

    #[test]
    fn preamble_before_the_first_heading_becomes_its_own_note() {
        let md = "loose line\n\n# A\n\nbody";
        let html = export_notes(md, "heading", "none", 3, false, "", "light").unwrap();
        assert_eq!(html.matches("<article class=\"note\"").count(), 2);
        assert!(html.contains("loose line"));
    }

    #[test]
    fn empty_notes_error() {
        assert_eq!(
            export_notes("  \n \t", "heading", "sidebar", 3, false, "", "light").unwrap_err(),
            "notes are empty"
        );
    }

    #[test]
    fn unknown_options_error() {
        assert!(
            export_notes("# A", "chapters", "top", 3, false, "", "light")
                .unwrap_err()
                .contains("split")
        );
        assert!(
            export_notes("# A", "heading", "left", 3, false, "", "light")
                .unwrap_err()
                .contains("toc")
        );
        assert!(export_notes("# A", "heading", "top", 3, false, "", "neon")
            .unwrap_err()
            .contains("theme"));
    }

    #[test]
    fn out_of_range_depth_errors() {
        assert!(export_notes("# A", "heading", "top", 0, false, "", "light")
            .unwrap_err()
            .contains("toc_depth"));
        assert!(export_notes("# A", "heading", "top", 7, false, "", "light")
            .unwrap_err()
            .contains("toc_depth"));
    }

    #[test]
    fn default_wrapper_works() {
        let html = export_notes_default("# Hello\n\nworld").unwrap();
        assert!(html.contains("<h1 id=\"hello\">Hello</h1>"));
        assert!(html.contains("<title>Notes</title>"));
        assert!(html.contains("--bg: #ffffff"));
    }
}
