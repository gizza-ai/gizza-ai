//! markdown-query core — pure compute, shared by the chat skill block and the
//! web page. No wafer/wasm-bindgen deps.
//!
//! A deterministic "jq for Markdown": parse a Markdown document with
//! `pulldown-cmark` (CommonMark + GFM tables) and pull out one kind of element —
//! headings, links, images, code blocks, or tables — rendering the result as
//! plain text, JSON, or reconstructed Markdown. Optionally annotate each item
//! with the 1-based source line it starts on.

use pulldown_cmark::{Alignment, CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use serde_json::json;

/// Which kind of element to pull out of the document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Extract {
    /// ATX/setext headings (`# Title`), with their level.
    Headings,
    /// Inline / reference links and autolinks, with text + destination.
    Links,
    /// Inline / reference images, with alt text + source.
    Images,
    /// Fenced or indented code blocks, with language (if fenced).
    CodeBlocks,
    /// GitHub-flavored pipe tables, with headers + rows.
    Tables,
}

pub fn parse_extract(s: &str) -> Result<Extract, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "headings" | "heading" | "headers" => Ok(Extract::Headings),
        "links" | "link" => Ok(Extract::Links),
        "images" | "image" | "img" => Ok(Extract::Images),
        "code_blocks" | "code-blocks" | "codeblocks" | "code" => Ok(Extract::CodeBlocks),
        "tables" | "table" => Ok(Extract::Tables),
        other => Err(format!(
            "extract {other:?} not supported (headings|links|images|code_blocks|tables)"
        )),
    }
}

/// How to render the extracted elements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// Human-readable plain text, one item per line (blocks for code/tables).
    Text,
    /// Pretty JSON: `{ "count": n, "<extract>": [ … ] }`.
    Json,
    /// Reconstructed Markdown for each item.
    Markdown,
}

pub fn parse_format(s: &str) -> Result<Format, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "text" => Ok(Format::Text),
        "json" => Ok(Format::Json),
        "markdown" | "md" => Ok(Format::Markdown),
        other => Err(format!(
            "format {other:?} not supported (text|json|markdown)"
        )),
    }
}

// --- typed items -----------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Align {
    None,
    Left,
    Center,
    Right,
}

impl Align {
    fn from_cmark(a: Alignment) -> Align {
        match a {
            Alignment::None => Align::None,
            Alignment::Left => Align::Left,
            Alignment::Center => Align::Center,
            Alignment::Right => Align::Right,
        }
    }
    /// The GFM separator-row cell, e.g. `:---:` for centered.
    fn sep(self) -> &'static str {
        match self {
            Align::None => "---",
            Align::Left => ":---",
            Align::Center => ":---:",
            Align::Right => "---:",
        }
    }
}

struct Heading {
    level: usize,
    text: String,
    line: usize,
}
struct Link {
    text: String,
    url: String,
    title: String,
    line: usize,
}
struct Image {
    alt: String,
    url: String,
    title: String,
    line: usize,
}
struct CodeBlock {
    language: String,
    code: String,
    line: usize,
}
struct Table {
    headers: Vec<String>,
    aligns: Vec<Align>,
    rows: Vec<Vec<String>>,
    line: usize,
}

// --- public entry points ---------------------------------------------------

/// Args accepted by [`run`] when the input is a JSON object. `markdown` is
/// required; the rest default (`extract`=headings, `format`=text, line numbers
/// off).
#[derive(serde::Deserialize)]
struct RunArgs {
    #[serde(default)]
    markdown: String,
    #[serde(default)]
    extract: String,
    #[serde(default)]
    format: String,
    #[serde(default)]
    include_line_numbers: bool,
}

/// JSON entry point (used by the chat runtime / CLI): `input` is a JSON object
/// `{ markdown, extract, format, include_line_numbers }`. Delegates to
/// [`query`].
pub fn run(input: &str) -> Result<String, String> {
    let args: RunArgs =
        serde_json::from_str(input).map_err(|e| format!("invalid JSON args: {e}"))?;
    let extract = parse_extract(&args.extract)?;
    let format = parse_format(&args.format)?;
    query(&args.markdown, extract, format, args.include_line_numbers)
}

/// Typed entry point: pull `extract` elements out of `markdown` and render them
/// as `format`. When `include_line_numbers` is set, each item is annotated with
/// the 1-based source line it begins on. Empty input is an error; a document
/// with no matching elements is not.
pub fn query(
    markdown: &str,
    extract: Extract,
    format: Format,
    include_line_numbers: bool,
) -> Result<String, String> {
    if markdown.trim().is_empty() {
        return Err("input Markdown is empty".into());
    }
    match extract {
        Extract::Headings => {
            render_headings(collect_headings(markdown), format, include_line_numbers)
        }
        Extract::Links => render_links(collect_links(markdown), format, include_line_numbers),
        Extract::Images => render_images(collect_images(markdown), format, include_line_numbers),
        Extract::CodeBlocks => {
            render_code_blocks(collect_code_blocks(markdown), format, include_line_numbers)
        }
        Extract::Tables => render_tables(collect_tables(markdown), format, include_line_numbers),
    }
}

// --- parsing helpers -------------------------------------------------------

fn options() -> Options {
    let mut o = Options::empty();
    o.insert(Options::ENABLE_TABLES);
    o.insert(Options::ENABLE_STRIKETHROUGH);
    o.insert(Options::ENABLE_TASKLISTS);
    o
}

/// 1-based source line containing byte `offset`.
fn line_at(md: &str, offset: usize) -> usize {
    let end = offset.min(md.len());
    md.as_bytes()[..end].iter().filter(|&&b| b == b'\n').count() + 1
}

/// Collapse runs of whitespace (incl. newlines) to single spaces and trim.
fn clean(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn collect_headings(md: &str) -> Vec<Heading> {
    let mut out = Vec::new();
    let mut cur: Option<(usize, usize, String)> = None; // (level, line, text)
    for (ev, range) in Parser::new_ext(md, options()).into_offset_iter() {
        match ev {
            Event::Start(Tag::Heading { level, .. }) => {
                cur = Some((level as usize, line_at(md, range.start), String::new()));
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some((level, line, text)) = cur.take() {
                    out.push(Heading {
                        level,
                        text: clean(&text),
                        line,
                    });
                }
            }
            Event::Text(t) | Event::Code(t) => {
                if let Some((_, _, text)) = cur.as_mut() {
                    text.push_str(&t);
                }
            }
            _ => {}
        }
    }
    out
}

fn collect_links(md: &str) -> Vec<Link> {
    let mut out = Vec::new();
    // stack of (url, title, line, text) to handle nesting.
    let mut stack: Vec<(String, String, usize, String)> = Vec::new();
    for (ev, range) in Parser::new_ext(md, options()).into_offset_iter() {
        match ev {
            Event::Start(Tag::Link {
                dest_url, title, ..
            }) => {
                stack.push((
                    dest_url.to_string(),
                    title.to_string(),
                    line_at(md, range.start),
                    String::new(),
                ));
            }
            Event::End(TagEnd::Link) => {
                if let Some((url, title, line, text)) = stack.pop() {
                    out.push(Link {
                        text: clean(&text),
                        url,
                        title,
                        line,
                    });
                }
            }
            Event::Text(t) | Event::Code(t) => {
                if let Some((_, _, _, text)) = stack.last_mut() {
                    text.push_str(&t);
                }
            }
            _ => {}
        }
    }
    out
}

fn collect_images(md: &str) -> Vec<Image> {
    let mut out = Vec::new();
    let mut stack: Vec<(String, String, usize, String)> = Vec::new();
    for (ev, range) in Parser::new_ext(md, options()).into_offset_iter() {
        match ev {
            Event::Start(Tag::Image {
                dest_url, title, ..
            }) => {
                stack.push((
                    dest_url.to_string(),
                    title.to_string(),
                    line_at(md, range.start),
                    String::new(),
                ));
            }
            Event::End(TagEnd::Image) => {
                if let Some((url, title, line, alt)) = stack.pop() {
                    out.push(Image {
                        alt: clean(&alt),
                        url,
                        title,
                        line,
                    });
                }
            }
            Event::Text(t) | Event::Code(t) => {
                if let Some((_, _, _, alt)) = stack.last_mut() {
                    alt.push_str(&t);
                }
            }
            _ => {}
        }
    }
    out
}

fn collect_code_blocks(md: &str) -> Vec<CodeBlock> {
    let mut out = Vec::new();
    let mut cur: Option<(String, usize, String)> = None; // (language, line, code)
    for (ev, range) in Parser::new_ext(md, options()).into_offset_iter() {
        match ev {
            Event::Start(Tag::CodeBlock(kind)) => {
                let language = match kind {
                    CodeBlockKind::Fenced(info) => {
                        // "rust", or "rust,ignore" → "rust".
                        info.split(&[',', ' '][..])
                            .next()
                            .unwrap_or("")
                            .trim()
                            .to_string()
                    }
                    CodeBlockKind::Indented => String::new(),
                };
                cur = Some((language, line_at(md, range.start), String::new()));
            }
            Event::End(TagEnd::CodeBlock) => {
                if let Some((language, line, code)) = cur.take() {
                    // pulldown emits a trailing newline for the block; drop it.
                    let code = code.strip_suffix('\n').unwrap_or(&code).to_string();
                    out.push(CodeBlock {
                        language,
                        code,
                        line,
                    });
                }
            }
            Event::Text(t) => {
                if let Some((_, _, code)) = cur.as_mut() {
                    code.push_str(&t);
                }
            }
            _ => {}
        }
    }
    out
}

fn collect_tables(md: &str) -> Vec<Table> {
    let mut out = Vec::new();
    let mut aligns: Vec<Align> = Vec::new();
    let mut headers: Vec<String> = Vec::new();
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut cur_row: Vec<String> = Vec::new();
    let mut cur_cell: Option<String> = None;
    let mut in_head = false;
    let mut line = 0usize;
    for (ev, range) in Parser::new_ext(md, options()).into_offset_iter() {
        match ev {
            Event::Start(Tag::Table(a)) => {
                aligns = a.into_iter().map(Align::from_cmark).collect();
                headers = Vec::new();
                rows = Vec::new();
                cur_row = Vec::new();
                line = line_at(md, range.start);
            }
            Event::Start(Tag::TableHead) => {
                in_head = true;
                cur_row = Vec::new();
            }
            Event::Start(Tag::TableRow) => {
                cur_row = Vec::new();
            }
            Event::Start(Tag::TableCell) => {
                cur_cell = Some(String::new());
            }
            Event::Text(t) | Event::Code(t) => {
                if let Some(cell) = cur_cell.as_mut() {
                    cell.push_str(&t);
                }
            }
            Event::End(TagEnd::TableCell) => {
                if let Some(cell) = cur_cell.take() {
                    cur_row.push(clean(&cell));
                }
            }
            Event::End(TagEnd::TableHead) => {
                headers = std::mem::take(&mut cur_row);
                in_head = false;
            }
            Event::End(TagEnd::TableRow) => {
                if !in_head {
                    rows.push(std::mem::take(&mut cur_row));
                }
            }
            Event::End(TagEnd::Table) => {
                out.push(Table {
                    headers: std::mem::take(&mut headers),
                    aligns: std::mem::take(&mut aligns),
                    rows: std::mem::take(&mut rows),
                    line,
                });
            }
            _ => {}
        }
    }
    out
}

// --- rendering -------------------------------------------------------------

/// `L12\t` when line numbers are requested, else empty. Used to prefix text.
fn ln_text(line: usize, on: bool) -> String {
    if on {
        format!("L{line}\t")
    } else {
        String::new()
    }
}

/// `<!-- L12 -->\n` when line numbers are requested, else empty. Keeps the
/// reconstructed Markdown valid.
fn ln_md(line: usize, on: bool) -> String {
    if on {
        format!("<!-- L{line} -->\n")
    } else {
        String::new()
    }
}

fn pretty(v: serde_json::Value) -> Result<String, String> {
    serde_json::to_string_pretty(&v).map_err(|e| format!("failed to serialize output: {e}"))
}

fn empty_text(label: &str) -> String {
    format!("No {label} found.")
}

fn render_headings(items: Vec<Heading>, format: Format, ln: bool) -> Result<String, String> {
    match format {
        Format::Json => {
            let arr: Vec<_> = items
                .iter()
                .map(|h| {
                    let mut o = json!({ "level": h.level, "text": h.text });
                    if ln {
                        o["line"] = json!(h.line);
                    }
                    o
                })
                .collect();
            pretty(json!({ "count": items.len(), "headings": arr }))
        }
        Format::Text => {
            if items.is_empty() {
                return Ok(empty_text("headings"));
            }
            Ok(items
                .iter()
                .map(|h| {
                    let indent = "  ".repeat(h.level.saturating_sub(1));
                    format!("{}{indent}{}", ln_text(h.line, ln), h.text)
                })
                .collect::<Vec<_>>()
                .join("\n"))
        }
        Format::Markdown => {
            if items.is_empty() {
                return Ok(empty_text("headings"));
            }
            Ok(items
                .iter()
                .map(|h| {
                    let hashes = "#".repeat(h.level.clamp(1, 6));
                    format!("{}{hashes} {}", ln_md(h.line, ln), h.text)
                })
                .collect::<Vec<_>>()
                .join("\n"))
        }
    }
}

fn render_links(items: Vec<Link>, format: Format, ln: bool) -> Result<String, String> {
    match format {
        Format::Json => {
            let arr: Vec<_> = items
                .iter()
                .map(|l| {
                    let mut o = json!({ "text": l.text, "url": l.url });
                    if !l.title.is_empty() {
                        o["title"] = json!(l.title);
                    }
                    if ln {
                        o["line"] = json!(l.line);
                    }
                    o
                })
                .collect();
            pretty(json!({ "count": items.len(), "links": arr }))
        }
        Format::Text => {
            if items.is_empty() {
                return Ok(empty_text("links"));
            }
            Ok(items
                .iter()
                .map(|l| {
                    let title = if l.title.is_empty() {
                        String::new()
                    } else {
                        format!(" — {}", l.title)
                    };
                    format!("{}{} ({}){title}", ln_text(l.line, ln), l.text, l.url)
                })
                .collect::<Vec<_>>()
                .join("\n"))
        }
        Format::Markdown => {
            if items.is_empty() {
                return Ok(empty_text("links"));
            }
            Ok(items
                .iter()
                .map(|l| {
                    format!(
                        "{}{}",
                        ln_md(l.line, ln),
                        md_link(&l.text, &l.url, &l.title, false)
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"))
        }
    }
}

fn render_images(items: Vec<Image>, format: Format, ln: bool) -> Result<String, String> {
    match format {
        Format::Json => {
            let arr: Vec<_> = items
                .iter()
                .map(|i| {
                    let mut o = json!({ "alt": i.alt, "url": i.url });
                    if !i.title.is_empty() {
                        o["title"] = json!(i.title);
                    }
                    if ln {
                        o["line"] = json!(i.line);
                    }
                    o
                })
                .collect();
            pretty(json!({ "count": items.len(), "images": arr }))
        }
        Format::Text => {
            if items.is_empty() {
                return Ok(empty_text("images"));
            }
            Ok(items
                .iter()
                .map(|i| format!("{}{} ({})", ln_text(i.line, ln), i.alt, i.url))
                .collect::<Vec<_>>()
                .join("\n"))
        }
        Format::Markdown => {
            if items.is_empty() {
                return Ok(empty_text("images"));
            }
            Ok(items
                .iter()
                .map(|i| {
                    format!(
                        "{}{}",
                        ln_md(i.line, ln),
                        md_link(&i.alt, &i.url, &i.title, true)
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"))
        }
    }
}

/// Build a Markdown link (`[text](url)`) or image (`![alt](url)`), with an
/// optional `"title"`.
fn md_link(text: &str, url: &str, title: &str, image: bool) -> String {
    let bang = if image { "!" } else { "" };
    if title.is_empty() {
        format!("{bang}[{text}]({url})")
    } else {
        format!("{bang}[{text}]({url} \"{title}\")")
    }
}

fn render_code_blocks(items: Vec<CodeBlock>, format: Format, ln: bool) -> Result<String, String> {
    match format {
        Format::Json => {
            let arr: Vec<_> = items
                .iter()
                .map(|c| {
                    let mut o = json!({ "language": c.language, "code": c.code });
                    if ln {
                        o["line"] = json!(c.line);
                    }
                    o
                })
                .collect();
            pretty(json!({ "count": items.len(), "code_blocks": arr }))
        }
        Format::Text => {
            if items.is_empty() {
                return Ok(empty_text("code blocks"));
            }
            Ok(items
                .iter()
                .map(|c| {
                    let header = if c.language.is_empty() {
                        "code:".to_string()
                    } else {
                        format!("{}:", c.language)
                    };
                    format!("{}{header}\n{}", ln_text(c.line, ln), c.code)
                })
                .collect::<Vec<_>>()
                .join("\n\n"))
        }
        Format::Markdown => {
            if items.is_empty() {
                return Ok(empty_text("code blocks"));
            }
            Ok(items
                .iter()
                .map(|c| format!("{}```{}\n{}\n```", ln_md(c.line, ln), c.language, c.code))
                .collect::<Vec<_>>()
                .join("\n\n"))
        }
    }
}

fn render_tables(items: Vec<Table>, format: Format, ln: bool) -> Result<String, String> {
    match format {
        Format::Json => {
            let arr: Vec<_> = items
                .iter()
                .map(|t| {
                    let mut o = json!({ "headers": t.headers, "rows": t.rows });
                    if ln {
                        o["line"] = json!(t.line);
                    }
                    o
                })
                .collect();
            pretty(json!({ "count": items.len(), "tables": arr }))
        }
        Format::Text => {
            if items.is_empty() {
                return Ok(empty_text("tables"));
            }
            Ok(items
                .iter()
                .map(|t| {
                    let mut lines = vec![t.headers.join(" | ")];
                    for r in &t.rows {
                        lines.push(r.join(" | "));
                    }
                    format!("{}{}", ln_text(t.line, ln), lines.join("\n"))
                })
                .collect::<Vec<_>>()
                .join("\n\n"))
        }
        Format::Markdown => {
            if items.is_empty() {
                return Ok(empty_text("tables"));
            }
            Ok(items
                .iter()
                .map(|t| format!("{}{}", ln_md(t.line, ln), md_table(t)))
                .collect::<Vec<_>>()
                .join("\n\n"))
        }
    }
}

/// Reconstruct a GFM pipe table from typed headers/aligns/rows.
fn md_table(t: &Table) -> String {
    let cols = t.headers.len();
    let mut lines = Vec::new();
    lines.push(format!("| {} |", t.headers.join(" | ")));
    let seps: Vec<&str> = (0..cols)
        .map(|i| t.aligns.get(i).copied().unwrap_or(Align::None).sep())
        .collect();
    lines.push(format!("| {} |", seps.join(" | ")));
    for r in &t.rows {
        lines.push(format!("| {} |", r.join(" | ")));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = "# Title\n\nSome [home](https://ex.test \"Home\") and [docs](/docs).\n\n![logo](img/logo.png)\n\n## Sub\n\n```rust\nlet x = 1;\n```\n\n| a | b |\n|:--|--:|\n| 1 | 2 |\n| 3 | 4 |\n";

    #[test]
    fn headings_json() {
        let out = query(DOC, Extract::Headings, Format::Json, false).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["count"], 2);
        assert_eq!(v["headings"][0]["level"], 1);
        assert_eq!(v["headings"][0]["text"], "Title");
        assert_eq!(v["headings"][1]["level"], 2);
        assert_eq!(v["headings"][1]["text"], "Sub");
        // line numbers omitted when off
        assert!(v["headings"][0]["line"].is_null());
    }

    #[test]
    fn headings_text_with_line_numbers() {
        let out = query(DOC, Extract::Headings, Format::Text, true).unwrap();
        assert!(out.starts_with("L1\tTitle"), "{out}");
        // level-2 heading is indented
        assert!(out.contains("L7\t  Sub"), "{out}");
    }

    #[test]
    fn headings_markdown() {
        let out = query("# A\n\n### C\n", Extract::Headings, Format::Markdown, false).unwrap();
        assert_eq!(out, "# A\n### C");
    }

    #[test]
    fn links_json_with_title_and_line() {
        let out = query(DOC, Extract::Links, Format::Json, true).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["count"], 2);
        assert_eq!(v["links"][0]["text"], "home");
        assert_eq!(v["links"][0]["url"], "https://ex.test");
        assert_eq!(v["links"][0]["title"], "Home");
        assert_eq!(v["links"][0]["line"], 3);
        // second link has no title → field omitted
        assert!(v["links"][1]["title"].is_null());
    }

    #[test]
    fn links_markdown_roundtrips_title() {
        let out = query(DOC, Extract::Links, Format::Markdown, false).unwrap();
        assert!(out.contains("[home](https://ex.test \"Home\")"), "{out}");
        assert!(out.contains("[docs](/docs)"), "{out}");
    }

    #[test]
    fn images_text() {
        let out = query(DOC, Extract::Images, Format::Text, false).unwrap();
        assert_eq!(out, "logo (img/logo.png)");
    }

    #[test]
    fn images_markdown() {
        let out = query(DOC, Extract::Images, Format::Markdown, false).unwrap();
        assert_eq!(out, "![logo](img/logo.png)");
    }

    #[test]
    fn code_blocks_json() {
        let out = query(DOC, Extract::CodeBlocks, Format::Json, false).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["count"], 1);
        assert_eq!(v["code_blocks"][0]["language"], "rust");
        assert_eq!(v["code_blocks"][0]["code"], "let x = 1;");
    }

    #[test]
    fn code_blocks_markdown_roundtrips() {
        let out = query(DOC, Extract::CodeBlocks, Format::Markdown, false).unwrap();
        assert_eq!(out, "```rust\nlet x = 1;\n```");
    }

    #[test]
    fn tables_json_and_markdown() {
        let j = query(DOC, Extract::Tables, Format::Json, false).unwrap();
        let v: serde_json::Value = serde_json::from_str(&j).unwrap();
        assert_eq!(v["count"], 1);
        assert_eq!(v["tables"][0]["headers"][0], "a");
        assert_eq!(v["tables"][0]["rows"][0][0], "1");
        assert_eq!(v["tables"][0]["rows"][1][1], "4");

        let m = query(DOC, Extract::Tables, Format::Markdown, false).unwrap();
        assert!(m.contains("| a | b |"), "{m}");
        assert!(m.contains("| :--- | ---: |"), "{m}");
        assert!(m.contains("| 1 | 2 |"), "{m}");
    }

    #[test]
    fn no_matches_is_empty_not_error() {
        let j = query(
            "plain text, nothing here",
            Extract::Links,
            Format::Json,
            false,
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&j).unwrap();
        assert_eq!(v["count"], 0);
        let t = query(
            "plain text, nothing here",
            Extract::Links,
            Format::Text,
            false,
        )
        .unwrap();
        assert_eq!(t, "No links found.");
    }

    #[test]
    fn run_parses_json_args() {
        let input = r##"{"markdown":"# Hi\n\n[x](y)","extract":"links","format":"json"}"##;
        let out = run(input).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["count"], 1);
        assert_eq!(v["links"][0]["url"], "y");
    }

    #[test]
    fn run_defaults_to_headings_text() {
        let out = run(r##"{"markdown":"# Only"}"##).unwrap();
        assert_eq!(out, "Only");
    }

    #[test]
    fn err_empty_markdown() {
        assert!(query("   ", Extract::Headings, Format::Text, false).is_err());
        assert!(run(r##"{"markdown":"  "}"##).is_err());
    }

    #[test]
    fn err_bad_extract_and_format() {
        assert!(parse_extract("bogus").is_err());
        assert!(parse_format("xml").is_err());
        assert!(run(r##"{"markdown":"# x","extract":"nope"}"##).is_err());
        assert!(run(r##"{"markdown":"# x","format":"nope"}"##).is_err());
    }

    #[test]
    fn err_invalid_json() {
        assert!(run("not json").is_err());
    }

    #[test]
    fn parse_forms() {
        assert_eq!(parse_extract("").unwrap(), Extract::Headings);
        assert_eq!(parse_extract("CODE_BLOCKS").unwrap(), Extract::CodeBlocks);
        assert_eq!(parse_extract("images").unwrap(), Extract::Images);
        assert_eq!(parse_format("MD").unwrap(), Format::Markdown);
        assert_eq!(parse_format("").unwrap(), Format::Text);
    }
}
