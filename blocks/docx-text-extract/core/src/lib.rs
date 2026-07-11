//! gizza-ai/docx-text-extract core — convert a `.docx` (Microsoft Word) document
//! into GitHub-Flavored Markdown **and** clean plain text, preserving the
//! document structure (headings, ordered/bullet lists, tables, hyperlinks, and
//! bold/italic emphasis).
//!
//! Pure Rust with no wafer/wasm-bindgen deps, so it compiles natively for the
//! unit tests and to `wasm32-wasip1` (the `wafer build` target) for the chat
//! block.
//!
//! ## What a DOCX is
//!
//! A `.docx` is a ZIP container of Office Open XML parts. The bits we read:
//! - `word/document.xml` — the WordprocessingML body (paragraphs `<w:p>`, runs
//!   `<w:r>`, text `<w:t>`, tables `<w:tbl>`). Required.
//! - `word/numbering.xml` — maps a paragraph's `numId`/`ilvl` to a list format
//!   (`decimal` → ordered `1.`, `bullet` → `-`). Optional; absent ⇒ bullets.
//! - `word/_rels/document.xml.rels` — maps a hyperlink relationship id to its
//!   external `Target` URL, so `<w:hyperlink r:id="rIdN">` becomes
//!   `[text](url)`. Optional.
//!
//! ## Distinct from `document-text-extract`
//!
//! `document-text-extract` flattens a PDF/DOCX/EPUB to **plain text only**. This
//! tool is DOCX-specific and additionally reconstructs the **Markdown structure**
//! (headings, lists, tables, links, emphasis) — the same split as
//! `pdf-extract-text` (text) vs `pdf-to-markdown` (structure).

use std::collections::HashMap;
use std::io::{Cursor, Read};

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

/// A successful conversion: the reconstructed Markdown, the flattened plain text,
/// and a couple of structure counts for the caller to surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conversion {
    /// GitHub-Flavored Markdown reconstructed from the document structure.
    pub markdown: String,
    /// The flattened plain text (paragraphs separated by newlines, no markup).
    pub text: String,
    /// Number of heading paragraphs detected (styled `Heading1`…/`Title`).
    pub headings: usize,
    /// Number of tables rendered as Markdown pipe tables.
    pub tables: usize,
}

/// Convert the bytes of a `.docx` file to Markdown + plain text.
///
/// Returns `Err` when the bytes are empty, are not a ZIP/DOCX container, or the
/// container is missing `word/document.xml`.
pub fn convert(bytes: &[u8]) -> Result<Conversion, String> {
    if bytes.is_empty() {
        return Err("input is empty".into());
    }
    if !(bytes.starts_with(b"PK\x03\x04") || bytes.starts_with(b"PK\x05\x06")) {
        return Err(
            "not a .docx file — expected a ZIP container (a DOCX is a ZIP of Office Open XML)"
                .into(),
        );
    }
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|e| format!("not a valid DOCX/ZIP container: {e}"))?;

    // Confirm this ZIP is actually a DOCX (and not, say, an EPUB or a plain ZIP).
    let is_docx = zip.file_names().any(|n| n == "word/document.xml");
    if !is_docx {
        return Err("not a .docx file — the ZIP has no word/document.xml part".into());
    }

    let document = read_entry(&mut zip, "word/document.xml")
        .ok_or_else(|| "DOCX is missing word/document.xml".to_string())?;
    let numbering = read_entry(&mut zip, "word/numbering.xml")
        .map(|b| Numbering::parse(&b))
        .unwrap_or_default();
    let rels = read_entry(&mut zip, "word/_rels/document.xml.rels")
        .map(|b| parse_hyperlink_rels(&b))
        .unwrap_or_default();

    let (markdown, headings, tables) = docx_to_markdown(&document, &numbering, &rels)?;
    let text = docx_to_text(&document)?;
    Ok(Conversion {
        markdown,
        text,
        headings,
        tables,
    })
}

/// Read a ZIP entry fully into a byte buffer, or `None` if it is absent/unreadable.
fn read_entry(zip: &mut zip::ZipArchive<Cursor<&[u8]>>, name: &str) -> Option<Vec<u8>> {
    let mut entry = zip.by_name(name).ok()?;
    let mut buf = Vec::new();
    entry.read_to_end(&mut buf).ok()?;
    Some(buf)
}

/// Local element/attribute name with any `ns:` prefix stripped (WordprocessingML
/// tags are namespaced, e.g. `w:t`, `w:p`, `r:id`).
fn local_name(raw: &[u8]) -> &[u8] {
    match raw.iter().position(|&b| b == b':') {
        Some(i) => &raw[i + 1..],
        None => raw,
    }
}

/// Read the value of the attribute whose local name is `want` on `e`.
fn attr_local(e: &BytesStart, want: &[u8]) -> Option<String> {
    for a in e.attributes().flatten() {
        if local_name(a.key.as_ref()) == want {
            return String::from_utf8(a.value.into_owned()).ok();
        }
    }
    None
}

/// A `<w:val="...">`-style boolean toggle: absent/`1`/`true`/`on` ⇒ on;
/// `0`/`false`/`off` ⇒ off. Used for `<w:b>` (bold) and `<w:i>` (italic).
fn toggle_on(e: &BytesStart) -> bool {
    match attr_local(e, b"val") {
        None => true,
        Some(v) => !matches!(v.as_str(), "0" | "false" | "off"),
    }
}

/// Map a paragraph style id to a Markdown heading level (`1`..=`6`), or `None`
/// for a body paragraph. Recognizes Word's `Heading1`…`Heading9` and `Title`.
fn heading_level(style: &str) -> Option<usize> {
    let s = style.to_ascii_lowercase();
    if s == "title" {
        return Some(1);
    }
    if let Some(rest) = s.strip_prefix("heading") {
        if let Ok(n) = rest.trim().parse::<usize>() {
            return Some(n.clamp(1, 6));
        }
    }
    None
}

/// Escape the Markdown metacharacters that would otherwise be interpreted inside
/// inline text. `_` is intentionally NOT escaped (GFM treats intra-word
/// underscores literally, so escaping every `snake_case` word only adds noise).
fn escape_md(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(c, '\\' | '`' | '*' | '[' | ']') {
            o.push('\\');
        }
        o.push(c);
    }
    o
}

// ---------------------------------------------------------------------------
// numbering.xml — ordered vs bullet list detection
// ---------------------------------------------------------------------------

/// Parsed `word/numbering.xml`: enough to answer "is (numId, ilvl) an ordered
/// list?". Empty (the `Default`) means every list falls back to a bullet.
#[derive(Debug, Default, Clone)]
struct Numbering {
    /// `numId` → `abstractNumId`.
    num_to_abstract: HashMap<String, String>,
    /// `(abstractNumId, ilvl)` → `numFmt` (e.g. `decimal`, `bullet`).
    fmt: HashMap<(String, usize), String>,
}

impl Numbering {
    fn parse(xml: &[u8]) -> Self {
        let mut n = Numbering::default();
        let mut reader = Reader::from_reader(xml);
        let mut buf = Vec::new();
        // Current parse context.
        let mut cur_abstract: Option<String> = None; // inside <w:abstractNum>
        let mut cur_ilvl: Option<usize> = None; // inside <w:lvl>
        let mut cur_num: Option<String> = None; // inside <w:num>
        loop {
            let ev = match reader.read_event_into(&mut buf) {
                Ok(e) => e,
                Err(_) => break, // best-effort: a malformed numbering part ⇒ no data
            };
            match ev {
                Event::Start(e) | Event::Empty(e) => {
                    match local_name(e.name().as_ref()) {
                        b"abstractNum" => cur_abstract = attr_local(&e, b"abstractNumId"),
                        b"lvl" => cur_ilvl = attr_local(&e, b"ilvl").and_then(|v| v.parse().ok()),
                        b"numFmt" => {
                            if let (Some(a), Some(i), Some(f)) =
                                (&cur_abstract, cur_ilvl, attr_local(&e, b"val"))
                            {
                                n.fmt.insert((a.clone(), i), f);
                            }
                        }
                        b"num" => cur_num = attr_local(&e, b"numId"),
                        // `<w:abstractNumId w:val="M"/>` *element* inside `<w:num>`.
                        b"abstractNumId" => {
                            if let (Some(num), Some(v)) = (&cur_num, attr_local(&e, b"val")) {
                                n.num_to_abstract.insert(num.clone(), v);
                            }
                        }
                        _ => {}
                    }
                }
                Event::End(e) => match local_name(e.name().as_ref()) {
                    b"abstractNum" => cur_abstract = None,
                    b"lvl" => cur_ilvl = None,
                    b"num" => cur_num = None,
                    _ => {}
                },
                Event::Eof => break,
                _ => {}
            }
            buf.clear();
        }
        n
    }

    /// Is the list identified by `num_id` at indent level `ilvl` an ordered
    /// (numbered) list? Unknown ids / a missing part default to bullet.
    fn is_ordered(&self, num_id: &str, ilvl: usize) -> bool {
        let Some(abstract_id) = self.num_to_abstract.get(num_id) else {
            return false;
        };
        match self.fmt.get(&(abstract_id.clone(), ilvl)) {
            Some(f) => f != "bullet" && f != "none",
            None => false,
        }
    }
}

/// Parse `word/_rels/document.xml.rels` into `relationshipId → target URL` for
/// hyperlink relationships only.
fn parse_hyperlink_rels(xml: &[u8]) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let mut reader = Reader::from_reader(xml);
    let mut buf = Vec::new();
    loop {
        let ev = match reader.read_event_into(&mut buf) {
            Ok(e) => e,
            Err(_) => break,
        };
        match ev {
            Event::Start(e) | Event::Empty(e)
                if local_name(e.name().as_ref()) == b"Relationship" =>
            {
                let ty = attr_local(&e, b"Type").unwrap_or_default();
                if ty.ends_with("/hyperlink") {
                    if let (Some(id), Some(target)) =
                        (attr_local(&e, b"Id"), attr_local(&e, b"Target"))
                    {
                        map.insert(id, target);
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    map
}

// ---------------------------------------------------------------------------
// document.xml → Markdown
// ---------------------------------------------------------------------------

/// Accumulated inline state for the paragraph currently being read.
#[derive(Default)]
struct Para {
    /// Inline Markdown built so far (escaped text + emphasis/link markup).
    text: String,
    /// Paragraph style id (`<w:pStyle w:val>`), if any.
    style: Option<String>,
    /// `(ilvl, ordered)` when the paragraph is a list item (`<w:numPr>`).
    list: Option<(usize, bool)>,
    /// Current run's numbering ids while scanning `<w:numPr>`.
    num_id: Option<String>,
    num_ilvl: usize,
    /// Current run emphasis (reset at each `<w:r>`).
    bold: bool,
    italic: bool,
    /// Byte offset in `text` where the current hyperlink's label began, plus its
    /// resolved target — set between `<w:hyperlink>` start and end.
    link: Option<(usize, Option<String>)>,
}

/// Convert a `document.xml` body to GitHub-Flavored Markdown. Returns
/// `(markdown, heading_count, table_count)`.
fn docx_to_markdown(
    xml: &[u8],
    numbering: &Numbering,
    rels: &HashMap<String, String>,
) -> Result<(String, usize, usize), String> {
    let mut reader = Reader::from_reader(xml);
    let mut buf = Vec::new();

    let mut md = String::new();
    let mut after_list = false; // last emitted block was a list item
    let mut headings = 0usize;
    let mut tables = 0usize;

    let mut stack: Vec<Vec<u8>> = Vec::new();
    let in_elem = |stack: &[Vec<u8>], name: &[u8]| stack.iter().any(|n| n.as_slice() == name);

    let mut para = Para::default();
    // Table state (single level; nested tables render into the outer document).
    let mut table_rows: Vec<Vec<String>> = Vec::new();
    let mut cur_row: Vec<String> = Vec::new();
    let mut cell: Option<String> = None; // Some while inside a `<w:tc>`
    let mut in_table = 0usize; // `<w:tbl>` nesting depth

    loop {
        let event = reader
            .read_event_into(&mut buf)
            .map_err(|e| format!("malformed DOCX XML: {e}"))?;
        match event {
            Event::Start(e) => {
                let name = local_name(e.name().as_ref()).to_vec();
                match name.as_slice() {
                    b"p" => para = Para::default(),
                    b"r" => {
                        para.bold = false;
                        para.italic = false;
                    }
                    b"pStyle" if in_elem(&stack, b"pPr") => {
                        para.style = attr_local(&e, b"val");
                    }
                    b"b" if in_elem(&stack, b"r") => para.bold = toggle_on(&e),
                    b"i" if in_elem(&stack, b"r") => para.italic = toggle_on(&e),
                    b"hyperlink" => {
                        let target = attr_local(&e, b"id").and_then(|id| rels.get(&id).cloned());
                        para.link = Some((para.text.len(), target));
                    }
                    b"tbl" => {
                        in_table += 1;
                        table_rows = Vec::new();
                    }
                    b"tr" if in_table > 0 => cur_row = Vec::new(),
                    b"tc" if in_table > 0 => cell = Some(String::new()),
                    _ => {}
                }
                stack.push(name);
            }
            Event::Empty(e) => {
                match local_name(e.name().as_ref()) {
                    b"pStyle" if in_elem(&stack, b"pPr") => para.style = attr_local(&e, b"val"),
                    b"b" if in_elem(&stack, b"r") => para.bold = toggle_on(&e),
                    b"i" if in_elem(&stack, b"r") => para.italic = toggle_on(&e),
                    b"ilvl" if in_elem(&stack, b"numPr") => {
                        para.num_ilvl = attr_local(&e, b"val")
                            .and_then(|v| v.parse().ok())
                            .unwrap_or(0);
                    }
                    b"numId" if in_elem(&stack, b"numPr") => para.num_id = attr_local(&e, b"val"),
                    b"tab" if in_elem(&stack, b"r") => para.text.push(' '),
                    b"br" | b"cr" if in_elem(&stack, b"r") => para.text.push(' '),
                    _ => {}
                }
            }
            Event::Text(t) if in_elem(&stack, b"t") => {
                let decoded = t
                    .decode()
                    .map_err(|e| format!("failed to decode DOCX text: {e}"))?;
                push_run(&mut para, &escape_md(&decoded));
            }
            // Entity/character references inside a `<w:t>` run.
            Event::GeneralRef(r) if in_elem(&stack, b"t") => {
                if let Some(c) = decode_general_ref(&r)? {
                    let mut s = String::new();
                    s.push(c);
                    push_run(&mut para, &escape_md(&s));
                }
            }
            Event::End(e) => {
                let name = local_name(e.name().as_ref()).to_vec();
                stack.pop();
                match name.as_slice() {
                    b"numPr" => {
                        // Finished the list marker block: classify the list item.
                        let ordered = para
                            .num_id
                            .as_deref()
                            .map(|id| numbering.is_ordered(id, para.num_ilvl))
                            .unwrap_or(false);
                        para.list = Some((para.num_ilvl, ordered));
                    }
                    b"hyperlink" => {
                        if let Some((start, target)) = para.link.take() {
                            if start <= para.text.len() {
                                let label = para.text.split_off(start);
                                match target {
                                    Some(url) if !label.is_empty() => {
                                        para.text.push_str(&format!("[{label}]({url})"));
                                    }
                                    // Internal anchor or empty label: keep the text.
                                    _ => para.text.push_str(&label),
                                }
                            }
                        }
                    }
                    b"p" => {
                        let content = para.text.trim_end().to_string();
                        if let Some(buf) = cell.as_mut() {
                            // Paragraph inside a table cell: append inline text,
                            // joining multiple paragraphs with a soft break.
                            if !buf.is_empty() && !content.is_empty() {
                                buf.push_str("<br>");
                            }
                            buf.push_str(&content);
                        } else {
                            emit_block(&mut md, &mut after_list, &mut headings, &para, &content);
                        }
                    }
                    b"tc" => {
                        if let Some(buf) = cell.take() {
                            cur_row.push(buf);
                        }
                    }
                    b"tr" => {
                        if !cur_row.is_empty() {
                            table_rows.push(std::mem::take(&mut cur_row));
                        }
                    }
                    b"tbl" => {
                        in_table = in_table.saturating_sub(1);
                        if !table_rows.is_empty() {
                            close_list(&mut md, &mut after_list);
                            md.push_str(&render_table(&table_rows));
                            md.push_str("\n\n");
                            tables += 1;
                        }
                        table_rows = Vec::new();
                    }
                    _ => {}
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    // Collapse 3+ consecutive newlines to a blank line and trim the ends.
    let md = normalize_blank_lines(&md);
    Ok((md, headings, tables))
}

/// Append run text to the current paragraph, wrapping it in the run's active
/// emphasis markers.
fn push_run(para: &mut Para, escaped: &str) {
    if escaped.is_empty() {
        return;
    }
    let wrapped = match (para.bold, para.italic) {
        (true, true) => format!("***{escaped}***"),
        (true, false) => format!("**{escaped}**"),
        (false, true) => format!("*{escaped}*"),
        (false, false) => escaped.to_string(),
    };
    para.text.push_str(&wrapped);
}

/// Emit one finished top-level paragraph as a Markdown block (heading, list item,
/// or body paragraph), maintaining list/paragraph spacing.
fn emit_block(
    md: &mut String,
    after_list: &mut bool,
    headings: &mut usize,
    para: &Para,
    content: &str,
) {
    if let Some(level) = para.style.as_deref().and_then(heading_level) {
        close_list(md, after_list);
        for _ in 0..level {
            md.push('#');
        }
        md.push(' ');
        md.push_str(content);
        md.push_str("\n\n");
        *headings += 1;
        return;
    }
    if let Some((ilvl, ordered)) = para.list {
        // First item is already preceded by a blank line (prior blocks end with
        // "\n\n"); items are separated by single newlines.
        for _ in 0..ilvl {
            md.push_str("  ");
        }
        md.push_str(if ordered { "1. " } else { "- " });
        md.push_str(content);
        md.push('\n');
        *after_list = true;
        return;
    }
    close_list(md, after_list);
    if content.is_empty() {
        return; // empty spacer paragraph — normalization keeps one blank line
    }
    md.push_str(content);
    md.push_str("\n\n");
}

/// Ensure a blank line terminates a list before a following non-list block.
fn close_list(md: &mut String, after_list: &mut bool) {
    if *after_list {
        md.push('\n');
        *after_list = false;
    }
}

/// Render collected table rows as a GitHub-Flavored Markdown pipe table. The
/// first row is treated as the header.
fn render_table(rows: &[Vec<String>]) -> String {
    let cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    if cols == 0 {
        return String::new();
    }
    let cell = |rows: &[Vec<String>], r: usize, c: usize| -> String {
        rows.get(r)
            .and_then(|row| row.get(c))
            .map(|s| s.replace('|', "\\|"))
            .unwrap_or_default()
    };
    let mut out = String::new();
    // Header row.
    out.push('|');
    for c in 0..cols {
        out.push(' ');
        out.push_str(&cell(rows, 0, c));
        out.push_str(" |");
    }
    out.push('\n');
    // Separator.
    out.push('|');
    for _ in 0..cols {
        out.push_str(" --- |");
    }
    // Body rows.
    for r in 1..rows.len() {
        out.push('\n');
        out.push('|');
        for c in 0..cols {
            out.push(' ');
            out.push_str(&cell(rows, r, c));
            out.push_str(" |");
        }
    }
    out
}

/// Collapse runs of 3+ newlines to exactly two and trim leading/trailing blank
/// space, so paragraph spacing is uniform.
fn normalize_blank_lines(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut newlines = 0usize;
    for c in s.chars() {
        if c == '\n' {
            newlines += 1;
            if newlines <= 2 {
                out.push('\n');
            }
        } else {
            newlines = 0;
            out.push(c);
        }
    }
    out.trim().to_string()
}

// ---------------------------------------------------------------------------
// document.xml → plain text (structure-free flatten)
// ---------------------------------------------------------------------------

/// Flatten a WordprocessingML `document.xml` body to plain text: `<w:t>` is text,
/// `<w:p>` ends a paragraph (newline), `<w:tab>` is a tab, `<w:br>`/`<w:cr>` are
/// line breaks. Mirrors the proven `document-text-extract` flattener.
fn docx_to_text(xml: &[u8]) -> Result<String, String> {
    let mut reader = Reader::from_reader(xml);
    let mut buf = Vec::new();
    let mut out = String::new();
    let mut in_text = false;
    loop {
        match reader
            .read_event_into(&mut buf)
            .map_err(|e| format!("malformed DOCX XML: {e}"))?
        {
            Event::Start(e) if local_name(e.name().as_ref()) == b"t" => in_text = true,
            Event::End(e) => match local_name(e.name().as_ref()) {
                b"t" => in_text = false,
                b"p" => out.push('\n'),
                _ => {}
            },
            Event::Empty(e) => match local_name(e.name().as_ref()) {
                b"tab" => out.push('\t'),
                b"br" | b"cr" => out.push('\n'),
                _ => {}
            },
            Event::Text(t) if in_text => {
                let decoded = t
                    .decode()
                    .map_err(|e| format!("failed to decode DOCX text: {e}"))?;
                out.push_str(&decoded);
            }
            Event::GeneralRef(r) if in_text => {
                if let Some(c) = decode_general_ref(&r)? {
                    out.push(c);
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    while out.ends_with('\n') {
        out.pop();
    }
    Ok(out)
}

/// Resolve a quick-xml `GeneralRef` (an `&...;` entity or `&#...;` char ref) to a
/// single `char`. Numeric refs resolve directly; the five predefined named
/// entities map here (DOCX bodies only ever use these + numeric refs).
fn decode_general_ref(r: &quick_xml::events::BytesRef) -> Result<Option<char>, String> {
    if let Some(c) = r
        .resolve_char_ref()
        .map_err(|e| format!("bad character reference in DOCX: {e}"))?
    {
        return Ok(Some(c));
    }
    let name = r
        .decode()
        .map_err(|e| format!("bad entity reference in DOCX: {e}"))?;
    Ok(match name.as_ref() {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Write};

    use zip::write::SimpleFileOptions;
    use zip::CompressionMethod;

    use super::*;

    /// Build an in-memory ZIP from `(name, bytes)` entries (stored, uncompressed).
    fn zip_of(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut w = zip::ZipWriter::new(Cursor::new(&mut buf));
            let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
            for (name, data) in entries {
                w.start_file(*name, opts).unwrap();
                w.write_all(data).unwrap();
            }
            w.finish().unwrap();
        }
        buf
    }

    /// A DOCX carrying `body` as the `<w:body>` content, with optional extra parts.
    fn make_docx_with(body: &str, extra: &[(&str, &[u8])]) -> Vec<u8> {
        let xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
<w:body>{body}</w:body></w:document>"#
        );
        let xml_bytes = xml.into_bytes();
        let mut entries: Vec<(&str, &[u8])> = vec![("[Content_Types].xml", b"<Types/>")];
        entries.push(("word/document.xml", xml_bytes.as_slice()));
        for e in extra {
            entries.push(*e);
        }
        zip_of(&entries)
    }

    fn make_docx(body: &str) -> Vec<u8> {
        make_docx_with(body, &[])
    }

    #[test]
    fn empty_input_is_an_error() {
        let err = convert(b"").unwrap_err();
        assert!(err.contains("empty"), "err was: {err}");
    }

    #[test]
    fn non_zip_is_an_error() {
        let err = convert(b"%PDF-1.7 not a docx").unwrap_err();
        assert!(err.contains("not a .docx"), "err was: {err}");
    }

    #[test]
    fn zip_without_document_part_is_an_error() {
        let bogus = zip_of(&[("readme.txt", b"hello")]);
        let err = convert(&bogus).unwrap_err();
        assert!(err.contains("no word/document.xml"), "err was: {err}");
    }

    #[test]
    fn plain_paragraphs_to_text_and_markdown() {
        let docx = make_docx(
            r#"<w:p><w:r><w:t>Hello World</w:t></w:r></w:p>
<w:p><w:r><w:t>Second</w:t><w:tab/><w:t>A &amp; B</w:t></w:r></w:p>"#,
        );
        let c = convert(&docx).unwrap();
        assert_eq!(c.text, "Hello World\nSecond\tA & B");
        assert_eq!(c.markdown, "Hello World\n\nSecond A & B");
        assert_eq!(c.headings, 0);
        assert_eq!(c.tables, 0);
    }

    #[test]
    fn headings_use_pstyle_level() {
        let docx = make_docx(
            r#"<w:p><w:pPr><w:pStyle w:val="Title"/></w:pPr><w:r><w:t>My Doc</w:t></w:r></w:p>
<w:p><w:pPr><w:pStyle w:val="Heading2"/></w:pPr><w:r><w:t>Section</w:t></w:r></w:p>
<w:p><w:r><w:t>Body</w:t></w:r></w:p>"#,
        );
        let c = convert(&docx).unwrap();
        assert_eq!(c.markdown, "# My Doc\n\n## Section\n\nBody");
        assert_eq!(c.headings, 2);
        // Plain text drops the heading markup.
        assert_eq!(c.text, "My Doc\nSection\nBody");
    }

    #[test]
    fn bold_and_italic_runs_become_emphasis() {
        let docx = make_docx(
            r#"<w:p><w:r><w:t>plain </w:t></w:r><w:r><w:rPr><w:b/></w:rPr><w:t>bold</w:t></w:r><w:r><w:t> and </w:t></w:r><w:r><w:rPr><w:i/></w:rPr><w:t>ital</w:t></w:r></w:p>"#,
        );
        let c = convert(&docx).unwrap();
        assert_eq!(c.markdown, "plain **bold** and *ital*");
    }

    #[test]
    fn paragraph_mark_formatting_does_not_bold_runs() {
        // `<w:b/>` inside `<w:pPr><w:rPr>` is paragraph-mark formatting and must
        // NOT bold the run text.
        let docx = make_docx(
            r#"<w:p><w:pPr><w:rPr><w:b/></w:rPr></w:pPr><w:r><w:t>not bold</w:t></w:r></w:p>"#,
        );
        let c = convert(&docx).unwrap();
        assert_eq!(c.markdown, "not bold");
    }

    #[test]
    fn bullet_list_without_numbering_defaults_to_dashes() {
        let docx = make_docx(
            r#"<w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr></w:pPr><w:r><w:t>First</w:t></w:r></w:p>
<w:p><w:pPr><w:numPr><w:ilvl w:val="1"/><w:numId w:val="1"/></w:numPr></w:pPr><w:r><w:t>Nested</w:t></w:r></w:p>
<w:p><w:r><w:t>After</w:t></w:r></w:p>"#,
        );
        let c = convert(&docx).unwrap();
        assert_eq!(c.markdown, "- First\n  - Nested\n\nAfter");
    }

    #[test]
    fn ordered_list_uses_numbering_part() {
        let numbering = br#"<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
<w:abstractNum w:abstractNumId="0"><w:lvl w:ilvl="0"><w:numFmt w:val="decimal"/></w:lvl></w:abstractNum>
<w:num w:numId="7"><w:abstractNumId w:val="0"/></w:num>
</w:numbering>"#;
        let docx = make_docx_with(
            r#"<w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="7"/></w:numPr></w:pPr><w:r><w:t>Step one</w:t></w:r></w:p>
<w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="7"/></w:numPr></w:pPr><w:r><w:t>Step two</w:t></w:r></w:p>"#,
            &[("word/numbering.xml", numbering)],
        );
        let c = convert(&docx).unwrap();
        assert_eq!(c.markdown, "1. Step one\n1. Step two");
    }

    #[test]
    fn hyperlink_uses_rels_target() {
        let rels = br#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId9" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com/docs" TargetMode="External"/>
</Relationships>"#;
        let docx = make_docx_with(
            r#"<w:p><w:r><w:t>See </w:t></w:r><w:hyperlink r:id="rId9"><w:r><w:t>the docs</w:t></w:r></w:hyperlink></w:p>"#,
            &[("word/_rels/document.xml.rels", rels)],
        );
        let c = convert(&docx).unwrap();
        assert_eq!(c.markdown, "See [the docs](https://example.com/docs)");
    }

    #[test]
    fn table_becomes_pipe_table() {
        let docx = make_docx(
            r#"<w:tbl>
<w:tr><w:tc><w:p><w:r><w:t>Name</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Age</w:t></w:r></w:p></w:tc></w:tr>
<w:tr><w:tc><w:p><w:r><w:t>Ada</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>36</w:t></w:r></w:p></w:tc></w:tr>
</w:tbl>"#,
        );
        let c = convert(&docx).unwrap();
        assert_eq!(c.markdown, "| Name | Age |\n| --- | --- |\n| Ada | 36 |");
        assert_eq!(c.tables, 1);
    }

    #[test]
    fn escapes_markdown_metacharacters() {
        let docx = make_docx(r#"<w:p><w:r><w:t>a * b [c] `code`</w:t></w:r></w:p>"#);
        let c = convert(&docx).unwrap();
        assert_eq!(c.markdown, r"a \* b \[c\] \`code\`");
        // Plain text keeps the literal characters.
        assert_eq!(c.text, "a * b [c] `code`");
    }
}
