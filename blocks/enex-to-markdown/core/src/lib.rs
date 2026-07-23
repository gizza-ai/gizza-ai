//! gizza-ai/enex-to-markdown core — convert an Evernote **ENEX** export into
//! clean Markdown (or plain text). An ENEX file is XML: a `<en-export>` with one
//! or more `<note>` entries, each carrying an ENML/HTML `<content>` body,
//! `<created>`/`<updated>` timestamps, `<tag>`s, `<note-attributes>` (source
//! URL), and base64 `<resource>` attachments. Pure-Rust (`quick-xml`, `htmd`,
//! `nanohtml2text`) so it runs on every backend including the chat Service
//! Worker. No wafer/wasm-bindgen deps here.

use quick_xml::events::Event;
use quick_xml::Reader;
use serde::Serialize;

/// Output body format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Markdown,
    Text,
}

impl Format {
    pub fn parse(s: &str) -> Result<Format, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "markdown" | "md" | "" => Ok(Format::Markdown),
            "text" | "txt" | "plain" => Ok(Format::Text),
            other => Err(format!("unknown format '{other}' (use markdown or text)")),
        }
    }
}

/// Where per-note metadata (title, dates, tags, source URL) is rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Metadata {
    /// A YAML `--- … ---` frontmatter block before the body.
    Frontmatter,
    /// A `# Title` heading plus an inline dates line and `#hashtag` tags.
    Inline,
    /// Only the `# Title` heading; dates, tags and source are dropped.
    None,
}

impl Metadata {
    pub fn parse(s: &str) -> Result<Metadata, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "frontmatter" | "yaml" | "" => Ok(Metadata::Frontmatter),
            "inline" => Ok(Metadata::Inline),
            "none" | "off" => Ok(Metadata::None),
            other => Err(format!(
                "unknown metadata '{other}' (use frontmatter, inline or none)"
            )),
        }
    }
}

/// Conversion options.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    pub format: Format,
    pub metadata: Metadata,
    /// List each note's attachments (filename, MIME, decoded size).
    pub attachments: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            format: Format::Markdown,
            metadata: Metadata::Frontmatter,
            attachments: true,
        }
    }
}

/// A decoded attachment reference (we report metadata, not the binary payload).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Resource {
    file_name: Option<String>,
    mime: Option<String>,
    data_b64: String,
}

/// One parsed `<note>`.
#[derive(Debug, Clone, Default)]
struct Note {
    title: Option<String>,
    created: Option<String>,
    updated: Option<String>,
    source_url: Option<String>,
    tags: Vec<String>,
    content_html: String,
    resources: Vec<Resource>,
}

/// The result of a conversion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Conversion {
    /// Number of notes found in the ENEX.
    pub notes: usize,
    /// The combined Markdown (or plain text) document.
    pub content: String,
}

/// Local name of an element, dropping any namespace prefix.
fn local_name(raw: &[u8]) -> &[u8] {
    match raw.iter().position(|&b| b == b':') {
        Some(i) => &raw[i + 1..],
        None => raw,
    }
}

/// Reformat an Evernote `YYYYMMDDThhmmssZ` timestamp as ISO-8601
/// (`YYYY-MM-DDThh:mm:ssZ`). Anything that doesn't match is passed through.
fn iso_date(raw: &str) -> String {
    let s = raw.trim();
    let b = s.as_bytes();
    if b.len() == 16 && b[8] == b'T' && b[15] == b'Z' && b[..8].iter().all(u8::is_ascii_digit) && b[9..15].iter().all(u8::is_ascii_digit) {
        format!(
            "{}-{}-{}T{}:{}:{}Z",
            &s[0..4],
            &s[4..6],
            &s[6..8],
            &s[9..11],
            &s[11..13],
            &s[13..15],
        )
    } else {
        s.to_string()
    }
}

/// Decoded byte length of a base64 string, computed without allocating the
/// decoded buffer (payloads can be many MiB).
fn b64_decoded_len(s: &str) -> usize {
    let n = s.bytes().filter(|b| !b.is_ascii_whitespace()).count();
    if n == 0 {
        return 0;
    }
    let pad = s
        .bytes()
        .rev()
        .filter(|b| !b.is_ascii_whitespace())
        .take_while(|&b| b == b'=')
        .count();
    (n / 4) * 3 - pad.min(2)
}

/// Human-readable byte size (B / KB / MB).
fn human_size(bytes: usize) -> String {
    const KB: usize = 1024;
    const MB: usize = 1024 * 1024;
    if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

/// Parse every `<note>` out of the ENEX XML.
fn parse_notes(xml: &str) -> Result<Vec<Note>, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);

    let mut notes: Vec<Note> = Vec::new();
    let mut cur: Option<Note> = None;
    let mut cur_res: Option<Resource> = None;
    // Stack of open element local names (interior nodes we care about are shallow).
    let mut path: Vec<Vec<u8>> = Vec::new();
    // Per-`<tag>` text buffer, flushed on the tag's end event.
    let mut tag_buf = String::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let ln = local_name(e.name().as_ref()).to_vec();
                match ln.as_slice() {
                    b"note" => cur = Some(Note::default()),
                    b"resource" => cur_res = Some(Resource::default()),
                    b"tag" => tag_buf.clear(),
                    _ => {}
                }
                path.push(ln);
            }
            Ok(Event::End(e)) => {
                let ln = local_name(e.name().as_ref()).to_vec();
                match ln.as_slice() {
                    b"note" => {
                        if let Some(n) = cur.take() {
                            notes.push(n);
                        }
                    }
                    b"resource" => {
                        if let (Some(note), Some(res)) = (cur.as_mut(), cur_res.take()) {
                            note.resources.push(res);
                        }
                    }
                    b"tag" => {
                        let t = tag_buf.trim();
                        if !t.is_empty() {
                            if let Some(note) = cur.as_mut() {
                                note.tags.push(t.to_string());
                            }
                        }
                    }
                    _ => {}
                }
                path.pop();
            }
            Ok(Event::Text(t)) => {
                // route_text ignores text outside any element (empty path).
                let decoded = t.decode().unwrap_or_default();
                let text = quick_xml::escape::unescape(&decoded)
                    .map(|c| c.into_owned())
                    .unwrap_or_else(|_| decoded.into_owned());
                route_text(&text, &path, cur.as_mut(), cur_res.as_mut(), &mut tag_buf);
            }
            Ok(Event::CData(c)) => {
                let text = String::from_utf8_lossy(c.as_ref()).into_owned();
                route_text(&text, &path, cur.as_mut(), cur_res.as_mut(), &mut tag_buf);
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("malformed ENEX XML: {e}")),
            _ => {}
        }
    }
    Ok(notes)
}

/// Route a text/CDATA chunk to the current note/resource field by the innermost
/// open element name.
fn route_text(
    text: &str,
    path: &[Vec<u8>],
    note: Option<&mut Note>,
    res: Option<&mut Resource>,
    tag_buf: &mut String,
) {
    let Some(name) = path.last() else { return };
    match name.as_slice() {
        b"title" => {
            if let Some(n) = note {
                push_opt(&mut n.title, text);
            }
        }
        b"content" => {
            if let Some(n) = note {
                n.content_html.push_str(text);
            }
        }
        b"created" => {
            if let Some(n) = note {
                push_opt(&mut n.created, text);
            }
        }
        b"updated" => {
            if let Some(n) = note {
                push_opt(&mut n.updated, text);
            }
        }
        b"source-url" => {
            if let Some(n) = note {
                push_opt(&mut n.source_url, text);
            }
        }
        b"tag" => tag_buf.push_str(text),
        b"data" => {
            if let Some(r) = res {
                r.data_b64.push_str(text);
            }
        }
        b"mime" => {
            if let Some(r) = res {
                push_opt(&mut r.mime, text);
            }
        }
        b"file-name" => {
            if let Some(r) = res {
                push_opt(&mut r.file_name, text);
            }
        }
        _ => {}
    }
}

/// Append `text` to an `Option<String>` field, creating it if absent.
fn push_opt(field: &mut Option<String>, text: &str) {
    match field {
        Some(s) => s.push_str(text),
        None => *field = Some(text.to_string()),
    }
}

fn trimmed_opt(o: &Option<String>) -> Option<&str> {
    o.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

/// Render a single note.
fn render_note(note: &Note, opts: &Options) -> String {
    let title = trimmed_opt(&note.title);
    let created = note.created.as_deref().map(iso_date);
    let updated = note.updated.as_deref().map(iso_date);
    let source = trimmed_opt(&note.source_url);

    // Body: ENML/HTML -> Markdown or plain text.
    let body = match opts.format {
        Format::Markdown => htmd::convert(&note.content_html).unwrap_or_default(),
        Format::Text => nanohtml2text::html2text(&note.content_html),
    };
    let body = body.trim();

    let mut out = String::new();
    let md = opts.format == Format::Markdown;

    match opts.metadata {
        Metadata::Frontmatter => {
            out.push_str("---\n");
            if let Some(t) = title {
                out.push_str(&format!("title: {}\n", yaml_scalar(t)));
            }
            if let Some(c) = &created {
                out.push_str(&format!("created: {c}\n"));
            }
            if let Some(u) = &updated {
                out.push_str(&format!("updated: {u}\n"));
            }
            if let Some(s) = source {
                out.push_str(&format!("source: {}\n", yaml_scalar(s)));
            }
            if !note.tags.is_empty() {
                let list = note
                    .tags
                    .iter()
                    .map(|t| yaml_scalar(t))
                    .collect::<Vec<_>>()
                    .join(", ");
                out.push_str(&format!("tags: [{list}]\n"));
            }
            out.push_str("---\n\n");
        }
        Metadata::Inline => {
            if let Some(t) = title {
                out.push_str(&if md { format!("# {t}\n\n") } else { format!("{t}\n\n") });
            }
            let mut meta_line: Vec<String> = Vec::new();
            if let Some(c) = &created {
                meta_line.push(format!("Created: {c}"));
            }
            if let Some(u) = &updated {
                meta_line.push(format!("Updated: {u}"));
            }
            if let Some(s) = source {
                meta_line.push(if md { format!("Source: <{s}>") } else { format!("Source: {s}") });
            }
            if !meta_line.is_empty() {
                let line = meta_line.join(" · ");
                out.push_str(&if md { format!("*{line}*\n\n") } else { format!("{line}\n\n") });
            }
            if !note.tags.is_empty() {
                let line = if md {
                    note.tags.iter().map(|t| format!("#{}", hashtagify(t))).collect::<Vec<_>>().join(" ")
                } else {
                    format!("Tags: {}", note.tags.join(", "))
                };
                out.push_str(&line);
                out.push_str("\n\n");
            }
        }
        Metadata::None => {
            if let Some(t) = title {
                out.push_str(&if md { format!("# {t}\n\n") } else { format!("{t}\n\n") });
            }
        }
    }

    out.push_str(body);

    // Attachments listing.
    if opts.attachments && !note.resources.is_empty() {
        out.push_str("\n\n");
        out.push_str(if md { "**Attachments:**\n" } else { "Attachments:\n" });
        for r in &note.resources {
            let name = trimmed_opt(&r.file_name).unwrap_or("(unnamed)");
            let mime = trimmed_opt(&r.mime).unwrap_or("application/octet-stream");
            let size = human_size(b64_decoded_len(&r.data_b64));
            out.push_str(&if md {
                format!("- `{name}` ({mime}, {size})\n")
            } else {
                format!("- {name} ({mime}, {size})\n")
            });
        }
    }

    out.trim_end().to_string()
}

/// Quote a YAML scalar only when it needs it (contains characters that would
/// break a plain scalar).
fn yaml_scalar(s: &str) -> String {
    let needs_quote = s.is_empty()
        || s.starts_with(|c: char| "!&*?|>%@`\"'#,[]{}:- ".contains(c))
        || s.ends_with(' ')
        || s.contains(": ")
        || s.contains(" #")
        || s.contains('\n');
    if needs_quote {
        format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        s.to_string()
    }
}

/// Turn a tag into a `#hashtag`-safe token (spaces -> `-`).
fn hashtagify(tag: &str) -> String {
    tag.trim()
        .chars()
        .map(|c| if c.is_whitespace() { '-' } else { c })
        .collect()
}

/// Convert an ENEX export into Markdown (or plain text).
pub fn convert(xml: &str, opts: Options) -> Result<Conversion, String> {
    if xml.trim().is_empty() {
        return Err("input is empty".into());
    }
    if !xml.contains("<en-export") && !xml.contains("<note") {
        return Err(
            "this doesn't look like an ENEX export (no <en-export> or <note> element found)".into(),
        );
    }

    let notes = parse_notes(xml)?;
    if notes.is_empty() {
        return Err("no <note> entries were found in the ENEX file".into());
    }

    let sep = match opts.format {
        Format::Markdown => "\n\n---\n\n",
        Format::Text => "\n\n\n",
    };
    let content = notes
        .iter()
        .map(|n| render_note(n, &opts))
        .collect::<Vec<_>>()
        .join(sep);

    Ok(Conversion {
        notes: notes.len(),
        content,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE en-export SYSTEM "http://xml.evernote.com/pub/evernote-export4.dtd">
<en-export export-date="20230405T101112Z" application="Evernote" version="10.x">
  <note>
    <title>First Note</title>
    <content><![CDATA[<?xml version="1.0" encoding="UTF-8"?><!DOCTYPE en-note SYSTEM "http://xml.evernote.com/pub/enml2.dtd"><en-note><h1>Hello</h1><p>Some <b>bold</b> text and a <a href="https://example.com">link</a>.</p><ul><li>one</li><li>two</li></ul></en-note>]]></content>
    <created>20230101T090000Z</created>
    <updated>20230102T101500Z</updated>
    <note-attributes><source-url>https://example.com/article</source-url></note-attributes>
    <tag>work</tag>
    <tag>reading list</tag>
    <resource>
      <data encoding="base64">aGVsbG8gd29ybGQ=</data>
      <mime>image/png</mime>
      <resource-attributes><file-name>diagram.png</file-name></resource-attributes>
    </resource>
  </note>
  <note>
    <title>Second Note</title>
    <content><![CDATA[<en-note><p>Just a paragraph.</p></en-note>]]></content>
    <created>20240310T120000Z</created>
  </note>
</en-export>"#;

    #[test]
    fn parses_multiple_notes_with_fields() {
        let notes = parse_notes(SAMPLE).unwrap();
        assert_eq!(notes.len(), 2);
        let n = &notes[0];
        assert_eq!(n.title.as_deref(), Some("First Note"));
        assert_eq!(n.created.as_deref(), Some("20230101T090000Z"));
        assert_eq!(n.updated.as_deref(), Some("20230102T101500Z"));
        assert_eq!(n.source_url.as_deref(), Some("https://example.com/article"));
        assert_eq!(n.tags, vec!["work", "reading list"]);
        assert!(n.content_html.contains("<h1>Hello</h1>"));
        assert_eq!(n.resources.len(), 1);
        assert_eq!(n.resources[0].file_name.as_deref(), Some("diagram.png"));
        assert_eq!(n.resources[0].mime.as_deref(), Some("image/png"));
    }

    #[test]
    fn frontmatter_markdown_multiple_notes() {
        let out = convert(SAMPLE, Options::default()).unwrap();
        assert_eq!(out.notes, 2);
        // Frontmatter with ISO dates + tags.
        assert!(out.content.contains("title: First Note"), "{}", out.content);
        assert!(out.content.contains("created: 2023-01-01T09:00:00Z"));
        assert!(out.content.contains("updated: 2023-01-02T10:15:00Z"));
        assert!(out.content.contains("source: https://example.com/article"));
        assert!(out.content.contains("tags: [work, reading list]"));
        // Body converted to Markdown.
        assert!(out.content.contains("# Hello"));
        assert!(out.content.contains("**bold**"));
        assert!(out.content.contains("[link](https://example.com)"));
        // Attachment listed with decoded size (11 bytes -> "11 B").
        assert!(out.content.contains("`diagram.png` (image/png, 11 B)"), "{}", out.content);
        // Two notes joined by a horizontal rule.
        assert!(out.content.contains("\n---\n"));
        assert!(out.content.contains("Second Note"));
    }

    #[test]
    fn inline_metadata_hashtags() {
        let opts = Options {
            metadata: Metadata::Inline,
            ..Options::default()
        };
        let out = convert(SAMPLE, opts).unwrap();
        assert!(out.content.contains("# First Note"));
        assert!(out.content.contains("Created: 2023-01-01T09:00:00Z"));
        assert!(out.content.contains("Source: <https://example.com/article>"));
        // "reading list" -> #reading-list
        assert!(out.content.contains("#work #reading-list"), "{}", out.content);
    }

    #[test]
    fn metadata_none_drops_dates_and_tags() {
        let opts = Options {
            metadata: Metadata::None,
            ..Options::default()
        };
        let out = convert(SAMPLE, opts).unwrap();
        assert!(out.content.contains("# First Note"));
        assert!(!out.content.contains("created:"));
        assert!(!out.content.contains("#work"));
    }

    #[test]
    fn text_format_strips_html() {
        let opts = Options {
            format: Format::Text,
            metadata: Metadata::Inline,
            ..Options::default()
        };
        let out = convert(SAMPLE, opts).unwrap();
        assert!(!out.content.contains("<h1>"));
        assert!(!out.content.contains('#'));
        assert!(out.content.contains("Hello"));
        assert!(out.content.contains("Tags: work, reading list"));
    }

    #[test]
    fn attachments_can_be_disabled() {
        let opts = Options {
            attachments: false,
            ..Options::default()
        };
        let out = convert(SAMPLE, opts).unwrap();
        assert!(!out.content.contains("Attachments"));
        assert!(!out.content.contains("diagram.png"));
    }

    #[test]
    fn empty_and_malformed_errors() {
        assert!(convert("   ", Options::default()).is_err());
        assert!(convert("just some plain text, not xml", Options::default()).is_err());
        // Has <note> markers but broken XML -> parse error.
        let broken = "<en-export><note><title>Oops</content></note>";
        assert!(convert(broken, Options::default()).is_err());
    }

    #[test]
    fn no_notes_errors() {
        let empty_export = r#"<en-export export-date="20230405T101112Z"></en-export>"#;
        assert!(convert(empty_export, Options::default()).is_err());
    }

    #[test]
    fn helpers() {
        assert_eq!(iso_date("20230101T090000Z"), "2023-01-01T09:00:00Z");
        assert_eq!(iso_date("not a date"), "not a date");
        assert_eq!(b64_decoded_len("aGVsbG8gd29ybGQ="), 11); // "hello world"
        assert_eq!(b64_decoded_len("YWI="), 2);
        assert_eq!(b64_decoded_len(""), 0);
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(2048), "2.0 KB");
        assert_eq!(hashtagify("reading list"), "reading-list");
        assert_eq!(yaml_scalar("plain"), "plain");
        assert_eq!(yaml_scalar("has: colon"), "\"has: colon\"");
    }
}
