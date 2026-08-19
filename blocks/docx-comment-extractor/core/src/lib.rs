//! gizza-ai/docx-comment-extractor core — pull the tracked **review comments**
//! out of a `.docx` (Microsoft Word) file and emit them as a spreadsheet-ready
//! table (CSV/TSV/JSON/Markdown) with the author, the text that was commented
//! on, the timestamp, the thread structure, and the resolved/open status.
//!
//! Pure Rust with no wafer/wasm-bindgen deps, so it compiles natively for the
//! unit tests and to `wasm32-wasip1` for the chat/CLI block.
//!
//! ## Where Word keeps comments
//!
//! A `.docx` is a ZIP of Office Open XML parts. Comments live in three of them:
//!
//! - `word/comments.xml` — one `<w:comment w:id w:author w:initials w:date>` per
//!   comment, whose body is ordinary `<w:p>`/`<w:r>`/`<w:t>` paragraphs. This is
//!   the comment TEXT. Optional: a document with no comments has no such part.
//! - `word/document.xml` (plus headers/footers/foot+endnotes) — the ANCHOR. The
//!   commented range is delimited by `<w:commentRangeStart w:id>` …
//!   `<w:commentRangeEnd w:id>`, followed by a `<w:commentReference w:id>` run.
//!   The order those references appear in is the document order of the comments.
//! - `word/commentsExtended.xml` — the Word-2013+ thread/status extension:
//!   `<w15:commentEx w15:paraId w15:paraIdParent w15:done>`. `paraId` matches the
//!   `w14:paraId` attribute of a paragraph inside a comment, which is how a REPLY
//!   is linked to its parent and how the RESOLVED flag is recorded. Optional:
//!   without it every comment reads as a top-level, open comment.
//!
//! ## Distinct from `docx-text-extract`
//!
//! `docx-text-extract` converts the document BODY to Markdown/plain text and
//! ignores the review layer entirely. This tool reads only the review layer —
//! it never returns the document prose, just the commented spans.

use std::collections::HashMap;
use std::io::{Cursor, Read};

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

/// The body parts a comment anchor can live in, in the order they are scanned.
/// `word/document.xml` is required; the rest are optional and only present when
/// the document actually has headers/footers/notes.
const BODY_PART_PREFIXES: [&str; 4] = [
    "word/header",
    "word/footer",
    "word/footnotes",
    "word/endnotes",
];

/// Every column this tool can emit, in canonical order. Also the value accepted
/// by the `columns` parameter.
pub const ALL_COLUMNS: [&str; 10] = [
    "id",
    "parent_id",
    "author",
    "initials",
    "date",
    "time",
    "timestamp",
    "status",
    "anchor",
    "comment",
];

/// The default `columns` selection — the fields a reviewer triage sheet needs,
/// leaving `initials`/`timestamp` (redundant with `author`/`date`+`time`) opt-in.
pub const DEFAULT_COLUMNS: &str = "id,parent_id,author,date,time,status,anchor,comment";

/// One extracted review comment (a top-level comment or a reply).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Comment {
    /// Word's own `w:id` for the comment — stable within the document.
    pub id: String,
    /// The `id` of the comment this one replies to; empty for a top-level comment.
    pub parent_id: String,
    /// Display name of whoever wrote the comment.
    pub author: String,
    /// The author's initials as Word recorded them (often empty).
    pub initials: String,
    /// The raw `w:date` timestamp as stored (ISO 8601), or empty if absent.
    pub timestamp: String,
    /// `YYYY-MM-DD` split out of `timestamp`, or empty.
    pub date: String,
    /// `HH:MM` split out of `timestamp`, or empty.
    pub time: String,
    /// True when the thread was marked resolved (`w15:done="1"`).
    pub resolved: bool,
    /// The comment's own text, paragraphs joined with newlines.
    pub text: String,
    /// The document text the comment is anchored to (whitespace-collapsed). A
    /// reply with no range of its own inherits its parent's anchor.
    pub anchor: String,
}

impl Comment {
    /// True when this comment is a reply in a thread.
    pub fn is_reply(&self) -> bool {
        !self.parent_id.is_empty()
    }

    /// The value of one column for this comment (raw — the formatter applies
    /// any per-format sanitizing).
    fn column(&self, col: Column) -> &str {
        match col {
            Column::Id => &self.id,
            Column::ParentId => &self.parent_id,
            Column::Author => &self.author,
            Column::Initials => &self.initials,
            Column::Date => &self.date,
            Column::Time => &self.time,
            Column::Timestamp => &self.timestamp,
            Column::Status => {
                if self.resolved {
                    "resolved"
                } else {
                    "open"
                }
            }
            Column::Anchor => &self.anchor,
            Column::Comment => &self.text,
        }
    }
}

/// One output column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Column {
    Id,
    ParentId,
    Author,
    Initials,
    Date,
    Time,
    Timestamp,
    Status,
    Anchor,
    Comment,
}

impl Column {
    /// The column's canonical name — also its header cell and JSON key.
    pub fn name(self) -> &'static str {
        match self {
            Column::Id => "id",
            Column::ParentId => "parent_id",
            Column::Author => "author",
            Column::Initials => "initials",
            Column::Date => "date",
            Column::Time => "time",
            Column::Timestamp => "timestamp",
            Column::Status => "status",
            Column::Anchor => "anchor",
            Column::Comment => "comment",
        }
    }

    fn parse(s: &str) -> Result<Column, String> {
        match s {
            "id" => Ok(Column::Id),
            "parent_id" => Ok(Column::ParentId),
            "author" => Ok(Column::Author),
            "initials" => Ok(Column::Initials),
            "date" => Ok(Column::Date),
            "time" => Ok(Column::Time),
            "timestamp" => Ok(Column::Timestamp),
            "status" => Ok(Column::Status),
            "anchor" => Ok(Column::Anchor),
            "comment" => Ok(Column::Comment),
            other => Err(format!(
                "unknown column {other:?}: expected one of {}",
                ALL_COLUMNS.join(", ")
            )),
        }
    }
}

/// Output table format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Csv,
    Tsv,
    Json,
    Markdown,
}

impl Format {
    /// Parse the `format` parameter value.
    pub fn parse(s: &str) -> Result<Format, String> {
        match s {
            "csv" => Ok(Format::Csv),
            "tsv" => Ok(Format::Tsv),
            "json" => Ok(Format::Json),
            "markdown" => Ok(Format::Markdown),
            other => Err(format!(
                "invalid format {other:?}: expected one of \"csv\", \"tsv\", \"json\", \"markdown\""
            )),
        }
    }
}

/// Which comments to keep, by resolved/open state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusFilter {
    All,
    Open,
    Resolved,
}

impl StatusFilter {
    /// Parse the `status` parameter value.
    pub fn parse(s: &str) -> Result<StatusFilter, String> {
        match s {
            "all" => Ok(StatusFilter::All),
            "open" => Ok(StatusFilter::Open),
            "resolved" => Ok(StatusFilter::Resolved),
            other => Err(format!(
                "invalid status {other:?}: expected one of \"all\", \"open\", \"resolved\""
            )),
        }
    }

    fn keeps(self, resolved: bool) -> bool {
        match self {
            StatusFilter::All => true,
            StatusFilter::Open => !resolved,
            StatusFilter::Resolved => resolved,
        }
    }
}

/// Everything the extraction is parameterized by. Built once per call so all
/// surfaces (chat, CLI) funnel through the same validation.
#[derive(Debug, Clone)]
pub struct Options {
    /// Output table format.
    pub format: Format,
    /// Columns to emit, in order.
    pub columns: Vec<Column>,
    /// Lower-cased author filter terms; empty keeps every author.
    pub authors: Vec<String>,
    /// Resolved/open filter.
    pub status: StatusFilter,
    /// Keep replies (comments that answer another comment).
    pub include_replies: bool,
    /// Replace newlines inside a cell with a single space, so one comment is one
    /// spreadsheet row.
    pub flatten_newlines: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            format: Format::Csv,
            columns: parse_columns(DEFAULT_COLUMNS).expect("DEFAULT_COLUMNS is valid"),
            authors: Vec::new(),
            status: StatusFilter::All,
            include_replies: true,
            flatten_newlines: true,
        }
    }
}

/// A successful extraction: the rendered table plus the counts a caller wants to
/// surface without re-parsing the output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Extraction {
    /// The rendered table in the requested format.
    pub output: String,
    /// Comments found in the document, before any filter.
    pub total: usize,
    /// Rows actually emitted (after the author/status/reply filters).
    pub returned: usize,
    /// Emitted rows that are replies.
    pub replies: usize,
    /// Emitted rows marked resolved.
    pub resolved: usize,
    /// Every distinct author in the document, sorted, before any filter.
    pub authors: Vec<String>,
}

/// Parse a comma-separated `columns` spec into the ordered column list.
/// An empty/blank spec falls back to [`DEFAULT_COLUMNS`].
pub fn parse_columns(spec: &str) -> Result<Vec<Column>, String> {
    let spec = spec.trim();
    let spec = if spec.is_empty() {
        DEFAULT_COLUMNS
    } else {
        spec
    };
    let mut out = Vec::new();
    for part in spec.split(',') {
        let name = part.trim();
        if name.is_empty() {
            continue;
        }
        let col = Column::parse(&name.to_ascii_lowercase())?;
        if !out.contains(&col) {
            out.push(col);
        }
    }
    if out.is_empty() {
        return Err(format!(
            "columns is empty: expected a comma-separated list of {}",
            ALL_COLUMNS.join(", ")
        ));
    }
    Ok(out)
}

/// Parse a comma-separated author filter into lower-cased match terms.
/// Blank ⇒ no filter.
pub fn parse_authors(spec: &str) -> Vec<String> {
    spec.split(',')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Extract the review comments from the bytes of a `.docx` file and render them
/// with `opts`.
///
/// Returns `Err` when the bytes are empty, are not a ZIP/DOCX container, or the
/// container has no `word/document.xml`. A document that simply has no comments
/// is NOT an error — it yields a header-only table and `total == 0`.
pub fn extract(bytes: &[u8], opts: &Options) -> Result<Extraction, String> {
    let comments = parse(bytes)?;

    let total = comments.len();
    let mut authors: Vec<String> = comments
        .iter()
        .map(|c| c.author.clone())
        .filter(|a| !a.is_empty())
        .collect();
    authors.sort();
    authors.dedup();

    let kept: Vec<&Comment> = comments
        .iter()
        .filter(|c| opts.include_replies || !c.is_reply())
        .filter(|c| opts.status.keeps(c.resolved))
        .filter(|c| author_matches(&c.author, &opts.authors))
        .collect();

    let output = render(&kept, opts);
    Ok(Extraction {
        output,
        total,
        returned: kept.len(),
        replies: kept.iter().filter(|c| c.is_reply()).count(),
        resolved: kept.iter().filter(|c| c.resolved).count(),
        authors,
    })
}

/// Parse the bytes of a `.docx` into its review comments, in document order.
/// Exposed so a caller can post-process the structured comments directly.
pub fn parse(bytes: &[u8]) -> Result<Vec<Comment>, String> {
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
    if !zip.file_names().any(|n| n == "word/document.xml") {
        return Err("not a .docx file — the ZIP has no word/document.xml part".into());
    }

    // The comment TEXT. Absent ⇒ the document has no comments at all.
    let raws = match read_entry(&mut zip, "word/comments.xml") {
        Some(bytes) => parse_comments_part(&bytes)?,
        None => return Ok(Vec::new()),
    };

    // The thread/status extension. Absent ⇒ everything is top-level and open.
    let ext = read_entry(&mut zip, "word/commentsExtended.xml")
        .map(|b| parse_comments_extended(&b))
        .unwrap_or_default();

    // The anchors + document order, from every body part that can host a
    // comment range (document, headers, footers, foot/endnotes).
    let mut anchor_parts = vec!["word/document.xml".to_string()];
    let mut extra: Vec<String> = zip
        .file_names()
        .filter(|n| {
            *n != "word/document.xml"
                && n.ends_with(".xml")
                && BODY_PART_PREFIXES.iter().any(|p| n.starts_with(p))
        })
        .map(|n| n.to_string())
        .collect();
    extra.sort();
    anchor_parts.append(&mut extra);

    let mut anchors: HashMap<String, String> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    for part in &anchor_parts {
        if let Some(bytes) = read_entry(&mut zip, part) {
            scan_anchors(&bytes, &mut anchors, &mut order)?;
        }
    }

    // paraId → comment id, so `w15:paraIdParent` can be resolved to a comment.
    let mut para_to_id: HashMap<&str, &str> = HashMap::new();
    for raw in &raws {
        for pid in &raw.para_ids {
            para_to_id.insert(pid.as_str(), raw.id.as_str());
        }
    }

    let mut comments: Vec<Comment> = raws
        .iter()
        .map(|raw| {
            // Word records the extension against the comment's LAST paragraph;
            // scan backwards so that one wins, but tolerate other layouts.
            let ex = raw
                .para_ids
                .iter()
                .rev()
                .find_map(|pid| ext.get(pid.as_str()));
            let parent_id = ex
                .and_then(|e| e.parent.as_deref())
                .and_then(|p| para_to_id.get(p).copied())
                .unwrap_or("")
                .to_string();
            Comment {
                id: raw.id.clone(),
                parent_id,
                author: raw.author.clone(),
                initials: raw.initials.clone(),
                date: split_date(&raw.date),
                time: split_time(&raw.date),
                timestamp: raw.date.clone(),
                resolved: ex.map(|e| e.done).unwrap_or(false),
                text: raw.text.trim().to_string(),
                anchor: anchors
                    .get(&raw.id)
                    .map(|a| collapse_ws(a))
                    .unwrap_or_default(),
            }
        })
        .collect();

    // A reply usually carries no comment range of its own — show it against the
    // span its thread is about rather than leaving the cell blank.
    let anchor_by_id: HashMap<String, String> = comments
        .iter()
        .map(|c| (c.id.clone(), c.anchor.clone()))
        .collect();
    for c in comments.iter_mut() {
        if c.anchor.is_empty() && !c.parent_id.is_empty() {
            if let Some(parent_anchor) = anchor_by_id.get(&c.parent_id) {
                c.anchor = parent_anchor.clone();
            }
        }
    }

    // Document order: the sequence the `<w:commentReference>` runs appear in.
    // Anything unreferenced (an orphaned comment part) keeps comments.xml order,
    // after the referenced ones.
    let pos: HashMap<&str, usize> = order
        .iter()
        .enumerate()
        .map(|(i, id)| (id.as_str(), i))
        .collect();
    let mut indexed: Vec<(usize, usize, Comment)> = comments
        .into_iter()
        .enumerate()
        .map(|(i, c)| {
            let p = pos.get(c.id.as_str()).copied().unwrap_or(usize::MAX);
            (p, i, c)
        })
        .collect();
    indexed.sort_by_key(|(p, i, _)| (*p, *i));
    Ok(indexed.into_iter().map(|(_, _, c)| c).collect())
}

/// True when `author` passes the filter terms (case-insensitive substring match;
/// an empty term list keeps everything).
fn author_matches(author: &str, terms: &[String]) -> bool {
    if terms.is_empty() {
        return true;
    }
    let a = author.to_lowercase();
    terms.iter().any(|t| a.contains(t.as_str()))
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn render(rows: &[&Comment], opts: &Options) -> String {
    match opts.format {
        Format::Csv => render_delimited(rows, opts, ','),
        Format::Tsv => render_delimited(rows, opts, '\t'),
        Format::Json => render_json(rows, opts),
        Format::Markdown => render_markdown(rows, opts),
    }
}

/// CSV (RFC 4180 quoting) or TSV (no quoting exists in TSV, so separators and
/// newlines are replaced with spaces).
fn render_delimited(rows: &[&Comment], opts: &Options, sep: char) -> String {
    let cell = |raw: &str| -> String {
        let v = if opts.flatten_newlines || sep == '\t' {
            collapse_ws(raw)
        } else {
            raw.to_string()
        };
        if sep == '\t' {
            v.replace('\t', " ")
        } else if v.contains([',', '"', '\n', '\r']) {
            format!("\"{}\"", v.replace('"', "\"\""))
        } else {
            v
        }
    };
    let mut out = String::new();
    let header: Vec<String> = opts.columns.iter().map(|c| cell(c.name())).collect();
    out.push_str(&header.join(&sep.to_string()));
    for r in rows {
        out.push('\n');
        let line: Vec<String> = opts.columns.iter().map(|c| cell(r.column(*c))).collect();
        out.push_str(&line.join(&sep.to_string()));
    }
    out
}

fn render_json(rows: &[&Comment], opts: &Options) -> String {
    if rows.is_empty() {
        return "[]".to_string();
    }
    let mut out = String::from("[\n");
    for (i, r) in rows.iter().enumerate() {
        out.push_str("  {");
        for (j, c) in opts.columns.iter().enumerate() {
            if j > 0 {
                out.push_str(", ");
            }
            let raw = r.column(*c);
            let v = if opts.flatten_newlines {
                collapse_ws(raw)
            } else {
                raw.to_string()
            };
            out.push('"');
            out.push_str(c.name());
            out.push_str("\": \"");
            out.push_str(&json_escape(&v));
            out.push('"');
        }
        out.push('}');
        if i + 1 < rows.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push(']');
    out
}

fn render_markdown(rows: &[&Comment], opts: &Options) -> String {
    // Markdown table cells cannot hold a raw newline or an unescaped pipe.
    let cell = |raw: &str| -> String { collapse_ws(raw).replace('|', "\\|") };
    let mut out = String::from("| ");
    out.push_str(
        &opts
            .columns
            .iter()
            .map(|c| c.name().to_string())
            .collect::<Vec<_>>()
            .join(" | "),
    );
    out.push_str(" |\n| ");
    out.push_str(&vec!["---"; opts.columns.len()].join(" | "));
    out.push_str(" |");
    for r in rows {
        out.push_str("\n| ");
        out.push_str(
            &opts
                .columns
                .iter()
                .map(|c| cell(r.column(*c)))
                .collect::<Vec<_>>()
                .join(" | "),
        );
        out.push_str(" |");
    }
    out
}

/// Escape a string for a JSON double-quoted scalar.
fn json_escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            '\u{08}' => o.push_str("\\b"),
            '\u{0c}' => o.push_str("\\f"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o
}

/// Collapse every run of whitespace (including newlines) to a single space and
/// trim the ends — what a spreadsheet cell wants.
fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `YYYY-MM-DD` out of an ISO-8601 `w:date`, or empty if it isn't one.
fn split_date(ts: &str) -> String {
    let b = ts.as_bytes();
    if ts.is_ascii() && b.len() >= 10 && b[4] == b'-' && b[7] == b'-' {
        ts[..10].to_string()
    } else {
        String::new()
    }
}

/// `HH:MM` out of an ISO-8601 `w:date`, or empty if it isn't one.
fn split_time(ts: &str) -> String {
    let b = ts.as_bytes();
    if ts.is_ascii() && b.len() >= 16 && (b[10] == b'T' || b[10] == b' ') && b[13] == b':' {
        ts[11..16].to_string()
    } else {
        String::new()
    }
}

// ---------------------------------------------------------------------------
// OOXML parsing
// ---------------------------------------------------------------------------

/// Read a ZIP entry fully into a byte buffer, or `None` if it is absent/unreadable.
fn read_entry(zip: &mut zip::ZipArchive<Cursor<&[u8]>>, name: &str) -> Option<Vec<u8>> {
    let mut entry = zip.by_name(name).ok()?;
    let mut buf = Vec::new();
    entry.read_to_end(&mut buf).ok()?;
    Some(buf)
}

/// Local element/attribute name with any `ns:` prefix stripped (OOXML tags are
/// namespaced, e.g. `w:comment`, `w14:paraId`, `w15:done`).
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

/// A `<w:comment>` exactly as `word/comments.xml` stores it.
#[derive(Debug, Default)]
struct RawComment {
    id: String,
    author: String,
    initials: String,
    date: String,
    text: String,
    /// `w14:paraId` of every paragraph in the comment body — the join key to
    /// `word/commentsExtended.xml`.
    para_ids: Vec<String>,
}

fn parse_comments_part(xml: &[u8]) -> Result<Vec<RawComment>, String> {
    let mut out: Vec<RawComment> = Vec::new();
    let mut reader = Reader::from_reader(xml);
    let mut buf = Vec::new();
    let mut cur: Option<RawComment> = None;
    let mut in_text = false;
    loop {
        let ev = reader
            .read_event_into(&mut buf)
            .map_err(|e| format!("malformed word/comments.xml: {e}"))?;
        match ev {
            Event::Start(e) | Event::Empty(e) => match local_name(e.name().as_ref()) {
                b"comment" => {
                    // A `<w:comment/>` with no body still counts as a comment.
                    if let Some(done) = cur.take() {
                        out.push(done);
                    }
                    cur = Some(RawComment {
                        id: attr_local(&e, b"id").unwrap_or_default(),
                        author: attr_local(&e, b"author").unwrap_or_default(),
                        initials: attr_local(&e, b"initials").unwrap_or_default(),
                        date: attr_local(&e, b"date").unwrap_or_default(),
                        ..Default::default()
                    });
                }
                b"p" => {
                    if let (Some(c), Some(pid)) = (cur.as_mut(), attr_local(&e, b"paraId")) {
                        c.para_ids.push(pid);
                    }
                }
                b"t" => in_text = true,
                b"tab" => {
                    if let Some(c) = cur.as_mut() {
                        c.text.push('\t');
                    }
                }
                b"br" | b"cr" => {
                    if let Some(c) = cur.as_mut() {
                        c.text.push('\n');
                    }
                }
                _ => {}
            },
            Event::End(e) => match local_name(e.name().as_ref()) {
                b"t" => in_text = false,
                b"p" => {
                    if let Some(c) = cur.as_mut() {
                        c.text.push('\n');
                    }
                }
                b"comment" => {
                    if let Some(done) = cur.take() {
                        out.push(done);
                    }
                }
                _ => {}
            },
            Event::Text(t) if in_text => {
                let decoded = t
                    .decode()
                    .map_err(|e| format!("failed to decode comment text: {e}"))?;
                if let Some(c) = cur.as_mut() {
                    c.text.push_str(&decoded);
                }
            }
            Event::GeneralRef(r) if in_text => {
                if let Some(ch) = decode_general_ref(&r)? {
                    if let Some(c) = cur.as_mut() {
                        c.text.push(ch);
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    if let Some(done) = cur.take() {
        out.push(done);
    }
    Ok(out)
}

/// One `<w15:commentEx>` record: the thread parent + the resolved flag.
#[derive(Debug, Default, Clone)]
struct CommentEx {
    parent: Option<String>,
    done: bool,
}

fn parse_comments_extended(xml: &[u8]) -> HashMap<String, CommentEx> {
    let mut out = HashMap::new();
    let mut reader = Reader::from_reader(xml);
    let mut buf = Vec::new();
    loop {
        // Best effort: a malformed extension part just means no thread/status
        // data, never a failed extraction.
        let ev = match reader.read_event_into(&mut buf) {
            Ok(e) => e,
            Err(_) => break,
        };
        match ev {
            Event::Start(e) | Event::Empty(e) if local_name(e.name().as_ref()) == b"commentEx" => {
                if let Some(pid) = attr_local(&e, b"paraId") {
                    let done = attr_local(&e, b"done")
                        .map(|v| matches!(v.as_str(), "1" | "true" | "on"))
                        .unwrap_or(false);
                    out.insert(
                        pid,
                        CommentEx {
                            parent: attr_local(&e, b"paraIdParent"),
                            done,
                        },
                    );
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    out
}

/// Walk a body part, accumulating the text inside every open comment range into
/// `anchors` and appending each `<w:commentReference>` id to `order`.
fn scan_anchors(
    xml: &[u8],
    anchors: &mut HashMap<String, String>,
    order: &mut Vec<String>,
) -> Result<(), String> {
    let mut reader = Reader::from_reader(xml);
    let mut buf = Vec::new();
    // Comment ranges may nest and overlap, so several can be open at once.
    let mut active: Vec<String> = Vec::new();
    let mut in_text = false;
    let push = |active: &Vec<String>, anchors: &mut HashMap<String, String>, s: &str| {
        for id in active {
            anchors.entry(id.clone()).or_default().push_str(s);
        }
    };
    loop {
        let ev = reader
            .read_event_into(&mut buf)
            .map_err(|e| format!("malformed DOCX body part: {e}"))?;
        match ev {
            Event::Start(e) | Event::Empty(e) => match local_name(e.name().as_ref()) {
                b"commentRangeStart" => {
                    if let Some(id) = attr_local(&e, b"id") {
                        anchors.entry(id.clone()).or_default();
                        active.push(id);
                    }
                }
                b"commentRangeEnd" => {
                    if let Some(id) = attr_local(&e, b"id") {
                        active.retain(|a| *a != id);
                    }
                }
                b"commentReference" => {
                    if let Some(id) = attr_local(&e, b"id") {
                        if !order.contains(&id) {
                            order.push(id);
                        }
                    }
                }
                b"t" => in_text = true,
                b"tab" => push(&active, anchors, " "),
                b"br" | b"cr" => push(&active, anchors, " "),
                _ => {}
            },
            Event::End(e) => match local_name(e.name().as_ref()) {
                b"t" => in_text = false,
                b"p" => push(&active, anchors, " "),
                _ => {}
            },
            Event::Text(t) if in_text => {
                let decoded = t
                    .decode()
                    .map_err(|e| format!("failed to decode DOCX text: {e}"))?;
                push(&active, anchors, &decoded);
            }
            Event::GeneralRef(r) if in_text => {
                if let Some(ch) = decode_general_ref(&r)? {
                    push(&active, anchors, &ch.to_string());
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(())
}

/// Resolve a quick-xml `GeneralRef` (an `&...;` entity or `&#...;` char ref) to a
/// single `char`. Numeric refs resolve directly; the five predefined named
/// entities map here (OOXML parts only ever use these + numeric refs).
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
    use std::io::Write;

    use super::*;

    const DOCUMENT_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
<w:body>
<w:p><w:r><w:t xml:space="preserve">Intro paragraph. </w:t></w:r>
<w:commentRangeStart w:id="1"/><w:r><w:t>the quick, brown fox</w:t></w:r><w:commentRangeEnd w:id="1"/>
<w:r><w:commentReference w:id="1"/></w:r>
<w:r><w:commentReference w:id="2"/></w:r></w:p>
<w:p><w:commentRangeStart w:id="3"/><w:r><w:t>second</w:t></w:r><w:r><w:t xml:space="preserve"> anchor</w:t></w:r><w:commentRangeEnd w:id="3"/>
<w:r><w:commentReference w:id="3"/></w:r></w:p>
</w:body></w:document>"#;

    const COMMENTS_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:w14="http://schemas.microsoft.com/office/word/2010/wordml">
<w:comment w:id="1" w:author="Jane Doe" w:initials="JD" w:date="2026-01-15T10:30:00Z">
  <w:p w14:paraId="0A000001"><w:r><w:t>Please clarify this claim.</w:t></w:r></w:p>
  <w:p w14:paraId="0A00000A"><w:r><w:t>Cite a source.</w:t></w:r></w:p>
</w:comment>
<w:comment w:id="2" w:author="Sam Ito" w:initials="SI" w:date="2026-01-16T09:05:00Z">
  <w:p w14:paraId="0A000002"><w:r><w:t>Agreed &#8212; I&apos;ll rewrite it.</w:t></w:r></w:p>
</w:comment>
<w:comment w:id="3" w:author="Jane Doe" w:initials="JD" w:date="2026-01-17T14:00:00Z">
  <w:p w14:paraId="0A000003"><w:r><w:t>Fixed, thanks.</w:t></w:r></w:p>
</w:comment>
</w:comments>"#;

    const COMMENTS_EXTENDED_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w15:commentsEx xmlns:w15="http://schemas.microsoft.com/office/word/2012/wordml">
<w15:commentEx w15:paraId="0A00000A" w15:done="0"/>
<w15:commentEx w15:paraId="0A000002" w15:paraIdParent="0A00000A" w15:done="0"/>
<w15:commentEx w15:paraId="0A000003" w15:done="1"/>
</w15:commentsEx>"#;

    /// Build an in-memory .docx from `(name, contents)` parts.
    fn docx(parts: &[(&str, &str)]) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut zw = zip::ZipWriter::new(Cursor::new(&mut buf));
            let opts = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            for (name, body) in parts {
                zw.start_file(*name, opts).unwrap();
                zw.write_all(body.as_bytes()).unwrap();
            }
            zw.finish().unwrap();
        }
        buf
    }

    fn sample() -> Vec<u8> {
        docx(&[
            ("word/document.xml", DOCUMENT_XML),
            ("word/comments.xml", COMMENTS_XML),
            ("word/commentsExtended.xml", COMMENTS_EXTENDED_XML),
        ])
    }

    #[test]
    fn extracts_default_csv() {
        let e = extract(&sample(), &Options::default()).unwrap();
        assert_eq!(
            e.output,
            "id,parent_id,author,date,time,status,anchor,comment\n\
             1,,Jane Doe,2026-01-15,10:30,open,\"the quick, brown fox\",Please clarify this claim. Cite a source.\n\
             2,1,Sam Ito,2026-01-16,09:05,open,\"the quick, brown fox\",Agreed — I'll rewrite it.\n\
             3,,Jane Doe,2026-01-17,14:00,resolved,second anchor,\"Fixed, thanks.\""
        );
        assert_eq!(e.total, 3);
        assert_eq!(e.returned, 3);
        assert_eq!(e.replies, 1);
        assert_eq!(e.resolved, 1);
        assert_eq!(
            e.authors,
            vec!["Jane Doe".to_string(), "Sam Ito".to_string()]
        );
    }

    #[test]
    fn threads_replies_and_inherits_parent_anchor() {
        let cs = parse(&sample()).unwrap();
        assert_eq!(cs.len(), 3);
        // Reply 2 is linked to comment 1 through the paraId of comment 1's LAST
        // paragraph, and borrows its parent's anchored span.
        assert_eq!(cs[1].id, "2");
        assert_eq!(cs[1].parent_id, "1");
        assert!(cs[1].is_reply());
        assert_eq!(cs[1].anchor, "the quick, brown fox");
        assert!(!cs[0].is_reply());
        assert!(cs[2].resolved);
        assert_eq!(cs[2].anchor, "second anchor");
        assert_eq!(cs[0].initials, "JD");
        assert_eq!(cs[0].timestamp, "2026-01-15T10:30:00Z");
    }

    #[test]
    fn document_order_wins_over_file_order() {
        // comments.xml lists 1,2,3; document.xml references them 1,2,3 as well,
        // but an unreferenced comment must sort last rather than vanish.
        let doc = r#"<w:document xmlns:w="w"><w:body><w:p>
            <w:r><w:commentReference w:id="3"/></w:r>
            <w:r><w:commentReference w:id="1"/></w:r></w:p></w:body></w:document>"#;
        let bytes = docx(&[
            ("word/document.xml", doc),
            ("word/comments.xml", COMMENTS_XML),
            ("word/commentsExtended.xml", COMMENTS_EXTENDED_XML),
        ]);
        let ids: Vec<String> = parse(&bytes).unwrap().into_iter().map(|c| c.id).collect();
        assert_eq!(ids, vec!["3", "1", "2"]);
    }

    #[test]
    fn author_filter_is_case_insensitive_substring() {
        let opts = Options {
            authors: parse_authors("jane"),
            ..Default::default()
        };
        let e = extract(&sample(), &opts).unwrap();
        assert_eq!(e.returned, 2);
        assert_eq!(
            e.total, 3,
            "total counts the whole document, not the filter"
        );
        assert!(!e.output.contains("Sam Ito"));
        // The unfiltered author roster still lists everyone.
        assert_eq!(e.authors.len(), 2);
    }

    #[test]
    fn status_and_reply_filters() {
        let resolved = extract(
            &sample(),
            &Options {
                status: StatusFilter::Resolved,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(resolved.returned, 1);
        assert!(resolved.output.contains("Fixed, thanks."));

        let open = extract(
            &sample(),
            &Options {
                status: StatusFilter::Open,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(open.returned, 2);

        let no_replies = extract(
            &sample(),
            &Options {
                include_replies: false,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(no_replies.returned, 2);
        assert_eq!(no_replies.replies, 0);
        assert!(!no_replies.output.contains("Sam Ito"));
    }

    #[test]
    fn columns_selection_and_order_is_honored() {
        let opts = Options {
            columns: parse_columns("author, comment").unwrap(),
            ..Default::default()
        };
        let e = extract(&sample(), &opts).unwrap();
        assert_eq!(
            e.output.lines().next().unwrap(),
            "author,comment",
            "header follows the requested order"
        );
        assert!(e.output.contains("Sam Ito,Agreed"));
        assert!(!e.output.contains("2026-01-15"));
    }

    #[test]
    fn columns_reject_unknown_name() {
        let err = parse_columns("author,page").unwrap_err();
        assert!(err.contains("unknown column \"page\""), "err was: {err}");
        assert!(err.contains("timestamp"), "error lists the valid names");
    }

    #[test]
    fn tsv_json_and_markdown_formats() {
        let one = Options {
            columns: parse_columns("id,author,comment").unwrap(),
            authors: parse_authors("Sam Ito"),
            ..Default::default()
        };

        let tsv = extract(
            &sample(),
            &Options {
                format: Format::Tsv,
                ..one.clone()
            },
        )
        .unwrap();
        assert_eq!(
            tsv.output,
            "id\tauthor\tcomment\n2\tSam Ito\tAgreed — I'll rewrite it."
        );

        let json = extract(
            &sample(),
            &Options {
                format: Format::Json,
                ..one.clone()
            },
        )
        .unwrap();
        assert_eq!(
            json.output,
            "[\n  {\"id\": \"2\", \"author\": \"Sam Ito\", \"comment\": \"Agreed — I'll rewrite it.\"}\n]"
        );

        let md = extract(
            &sample(),
            &Options {
                format: Format::Markdown,
                ..one
            },
        )
        .unwrap();
        assert_eq!(
            md.output,
            "| id | author | comment |\n| --- | --- | --- |\n| 2 | Sam Ito | Agreed — I'll rewrite it. |"
        );
    }

    #[test]
    fn json_is_an_empty_array_when_nothing_matches() {
        let e = extract(
            &sample(),
            &Options {
                format: Format::Json,
                authors: parse_authors("nobody"),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(e.output, "[]");
        assert_eq!(e.returned, 0);
    }

    #[test]
    fn multi_paragraph_comment_keeps_newlines_when_not_flattened() {
        let opts = Options {
            columns: parse_columns("comment").unwrap(),
            flatten_newlines: false,
            authors: parse_authors("jane doe"),
            ..Default::default()
        };
        let e = extract(&sample(), &opts).unwrap();
        // The two-paragraph comment stays one CSV field, quoted, with its break.
        assert!(
            e.output
                .contains("\"Please clarify this claim.\nCite a source.\""),
            "output was: {}",
            e.output
        );
    }

    #[test]
    fn document_without_comments_yields_header_only() {
        let bytes = docx(&[("word/document.xml", DOCUMENT_XML)]);
        let e = extract(&bytes, &Options::default()).unwrap();
        assert_eq!(e.total, 0);
        assert_eq!(e.returned, 0);
        assert_eq!(
            e.output,
            "id,parent_id,author,date,time,status,anchor,comment"
        );
        assert!(e.authors.is_empty());
    }

    #[test]
    fn missing_extension_part_means_all_top_level_and_open() {
        let bytes = docx(&[
            ("word/document.xml", DOCUMENT_XML),
            ("word/comments.xml", COMMENTS_XML),
        ]);
        let cs = parse(&bytes).unwrap();
        assert_eq!(cs.len(), 3);
        assert!(cs.iter().all(|c| !c.is_reply() && !c.resolved));
    }

    #[test]
    fn finds_comments_anchored_in_a_header() {
        let doc = r#"<w:document xmlns:w="w"><w:body><w:p/></w:body></w:document>"#;
        let header = r#"<w:hdr xmlns:w="w"><w:p>
            <w:commentRangeStart w:id="1"/><w:r><w:t>header span</w:t></w:r><w:commentRangeEnd w:id="1"/>
            <w:r><w:commentReference w:id="1"/></w:r></w:p></w:hdr>"#;
        let bytes = docx(&[
            ("word/document.xml", doc),
            ("word/header1.xml", header),
            ("word/comments.xml", COMMENTS_XML),
        ]);
        let cs = parse(&bytes).unwrap();
        assert_eq!(cs[0].id, "1");
        assert_eq!(cs[0].anchor, "header span");
    }

    #[test]
    fn rejects_empty_input() {
        assert_eq!(parse(b"").unwrap_err(), "input is empty");
    }

    #[test]
    fn rejects_non_zip_input() {
        let err = parse(b"%PDF-1.7 not a word file").unwrap_err();
        assert!(err.contains("expected a ZIP container"), "err was: {err}");
    }

    #[test]
    fn rejects_zip_that_is_not_a_docx() {
        let bytes = docx(&[("mimetype", "application/epub+zip")]);
        let err = parse(&bytes).unwrap_err();
        assert!(err.contains("no word/document.xml part"), "err was: {err}");
    }

    #[test]
    fn format_and_status_parse_errors_name_the_valid_values() {
        let f = Format::parse("xlsx").unwrap_err();
        assert!(
            f.contains("invalid format \"xlsx\"") && f.contains("markdown"),
            "err: {f}"
        );
        let s = StatusFilter::parse("done").unwrap_err();
        assert!(
            s.contains("invalid status \"done\"") && s.contains("resolved"),
            "err: {s}"
        );
    }

    #[test]
    fn csv_quotes_separators_and_doubles_quotes() {
        let c = Comment {
            id: "9".into(),
            author: "Ann \"A\" Lee".into(),
            text: "has, a comma".into(),
            ..Default::default()
        };
        let opts = Options {
            columns: parse_columns("id,author,comment").unwrap(),
            ..Default::default()
        };
        let out = render(&[&c], &opts);
        assert_eq!(
            out,
            "id,author,comment\n9,\"Ann \"\"A\"\" Lee\",\"has, a comma\""
        );
    }

    #[test]
    fn timestamp_split_tolerates_a_missing_or_odd_date() {
        assert_eq!(split_date("2026-01-15T10:30:00Z"), "2026-01-15");
        assert_eq!(split_time("2026-01-15T10:30:00Z"), "10:30");
        assert_eq!(split_date(""), "");
        assert_eq!(split_time("2026-01-15"), "");
        assert_eq!(split_date("not-a-date"), "");
    }
}
