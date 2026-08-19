//! pdf-bookmarks core — read, build, and strip a PDF's outline (bookmark) tree.
//! No wafer/wasm-bindgen deps, so it is unit-testable on the host.
//!
//! A PDF's navigation panel is driven by the *outline tree* hanging off the
//! document catalog's `/Outlines` entry: a doubly-linked list of item
//! dictionaries, each with a `/Title`, a `/Dest` destination, optional colour
//! (`/C`) and style flags (`/F`), and its own `/First`/`/Last`/`/Count` for
//! nested children. This module exposes three operations over that tree:
//!
//!   * [`list`] — parse the existing outline into a [`Bookmark`] tree.
//!   * [`apply`] — write a bookmark tree (replacing or appending to whatever is
//!     there), re-serialize, and report what happened.
//!   * [`remove`] — drop the outline entirely.
//!
//! Bookmark trees come from [`parse_spec`], which accepts either an
//! indentation-based text outline (`Chapter 1 | 3`) or the same JSON shape
//! [`list`] emits, so a list → edit → apply round trip works.
//!
//! Titles are written per the PDF text-string convention: ASCII stays a plain
//! literal, anything else becomes UTF-16BE with a byte-order mark so accented
//! and CJK titles survive across viewers.

use lopdf::{Dictionary, Document, Object, ObjectId, StringFormat};
use std::collections::{BTreeMap, HashSet};

/// Deepest nesting the writer accepts (top level = 1). Viewers handle more, but
/// a deeper tree is almost always an indentation mistake in the input.
pub const MAX_DEPTH: usize = 6;
/// Upper bound on total bookmarks in one document, to bound memory/time.
pub const MAX_BOOKMARKS: usize = 5000;
/// Guard against a malformed/cyclic outline chain while reading.
const MAX_READ_ITEMS: usize = 20_000;

/// One outline entry.
#[derive(Debug, Clone, PartialEq)]
pub struct Bookmark {
    pub title: String,
    /// 1-based target page. `None` only when reading a PDF whose destination
    /// could not be resolved to a page.
    pub page: Option<u32>,
    pub bold: bool,
    pub italic: bool,
    /// RGB in 0.0–1.0, as PDF stores it. `None` = viewer default (black).
    pub color: Option<[f32; 3]>,
    pub children: Vec<Bookmark>,
}

impl Bookmark {
    pub fn new(title: impl Into<String>, page: u32) -> Self {
        Self {
            title: title.into(),
            page: Some(page),
            bold: false,
            italic: false,
            color: None,
            children: Vec::new(),
        }
    }

    /// `/F` style flags: bit 1 = italic, bit 2 = bold.
    fn format_flags(&self) -> i64 {
        (self.italic as i64) | ((self.bold as i64) << 1)
    }
}

/// How a bookmark click positions the target page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Zoom {
    /// Fit the whole page in the window (`/Fit`).
    #[default]
    Fit,
    /// Fit the page width, keep the vertical position (`/FitH`).
    FitWidth,
    /// Jump to the page, keep the reader's current zoom (`/XYZ null null null`).
    Keep,
}

impl Zoom {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "fit" => Ok(Zoom::Fit),
            "fit-width" | "fit_width" | "fitwidth" | "width" => Ok(Zoom::FitWidth),
            "keep" | "inherit" | "none" => Ok(Zoom::Keep),
            other => Err(format!(
                "invalid zoom '{other}' (expected 'fit', 'fit-width', or 'keep')"
            )),
        }
    }
}

/// Write-side options.
#[derive(Debug, Clone)]
pub struct Options {
    /// Replace any existing outline (true) or append after it (false).
    pub replace: bool,
    /// Show child bookmarks expanded (true) or collapsed (false).
    pub expanded: bool,
    /// Set `/PageMode /UseOutlines` so viewers open with the bookmark pane.
    pub show_pane: bool,
    pub zoom: Zoom,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            replace: true,
            expanded: true,
            show_pane: true,
            zoom: Zoom::Fit,
        }
    }
}

/// What [`list`] found.
#[derive(Debug, Clone, PartialEq)]
pub struct Outline {
    pub page_count: u32,
    /// Total entries at all levels.
    pub total: usize,
    pub bookmarks: Vec<Bookmark>,
}

/// What [`apply`] / [`remove`] produced.
#[derive(Debug, Clone)]
pub struct WriteResult {
    pub bytes: Vec<u8>,
    /// Entries written at all levels (0 for [`remove`]).
    pub total: usize,
    /// Top-level entries written.
    pub top_level: usize,
    /// Entries dropped from the document's previous outline.
    pub removed: usize,
    pub page_count: u32,
    /// Non-fatal repairs, e.g. an out-of-range page clamped to the last page.
    pub warnings: Vec<String>,
}

/// Read `pdf`'s outline tree. A PDF with no outline yields an empty list.
pub fn list(pdf: &[u8]) -> Result<Outline, String> {
    let doc = load(pdf)?;
    let pages = doc.get_pages();
    let bookmarks = read_outline(&doc, &pages);
    Ok(Outline {
        page_count: pages.len() as u32,
        total: count_all(&bookmarks),
        bookmarks,
    })
}

/// Write `bookmarks` into `pdf`. With `opts.replace` false, the document's
/// existing entries are read back and kept ahead of the new ones.
pub fn apply(pdf: &[u8], bookmarks: Vec<Bookmark>, opts: &Options) -> Result<WriteResult, String> {
    let mut doc = load(pdf)?;
    let pages = doc.get_pages();
    let page_count = pages.len() as u32;
    if page_count == 0 {
        return Err("the PDF has no pages, so it cannot hold bookmarks".into());
    }

    let existing = read_outline(&doc, &pages);
    let removed = count_all(&existing);
    let mut tree = if opts.replace {
        bookmarks
    } else {
        let mut merged = existing;
        merged.extend(bookmarks);
        merged
    };

    let depth = depth_of(&tree);
    if depth > MAX_DEPTH {
        return Err(format!(
            "bookmarks nest {depth} levels deep, but at most {MAX_DEPTH} are supported"
        ));
    }
    let total = count_all(&tree);
    if total == 0 {
        return Err("no bookmarks to write — provide at least one entry".into());
    }
    if total > MAX_BOOKMARKS {
        return Err(format!(
            "{total} bookmarks exceeds the {MAX_BOOKMARKS}-bookmark limit for one document"
        ));
    }

    let mut warnings = Vec::new();
    clamp_pages(&mut tree, page_count, &mut warnings);

    // Drop the old outline objects before wiring in the new root, so replacing
    // an outline doesn't leave the previous tree behind as dead weight.
    for id in outline_object_ids(&doc) {
        doc.objects.remove(&id);
    }

    let page_ids: Vec<ObjectId> = pages.values().copied().collect();
    let root_id = doc.new_object_id();
    let built = emit_level(&mut doc, root_id, &tree, opts, &page_ids);
    let mut root = Dictionary::new();
    root.set("Type", Object::Name(b"Outlines".to_vec()));
    if let Some(first) = built.first {
        root.set("First", first);
    }
    if let Some(last) = built.last {
        root.set("Last", last);
    }
    root.set("Count", Object::Integer(built.visible));
    doc.objects.insert(root_id, Object::Dictionary(root));

    let catalog = doc
        .catalog_mut()
        .map_err(|e| format!("failed to read the document catalog: {e}"))?;
    catalog.set("Outlines", root_id);
    if opts.show_pane {
        catalog.set("PageMode", Object::Name(b"UseOutlines".to_vec()));
    } else if matches!(
        catalog.get(b"PageMode").map(|o| o.as_name()),
        Ok(Ok(b"UseOutlines"))
    ) {
        catalog.remove(b"PageMode");
    }

    Ok(WriteResult {
        bytes: save(&mut doc)?,
        total,
        top_level: tree.len(),
        removed,
        page_count,
        warnings,
    })
}

/// Strip the outline tree (and the bookmark-pane page mode) from `pdf`.
pub fn remove(pdf: &[u8]) -> Result<WriteResult, String> {
    let mut doc = load(pdf)?;
    let pages = doc.get_pages();
    let page_count = pages.len() as u32;
    let removed = count_all(&read_outline(&doc, &pages));
    if removed == 0 {
        return Err("this PDF has no bookmarks to remove".into());
    }
    for id in outline_object_ids(&doc) {
        doc.objects.remove(&id);
    }
    let catalog = doc
        .catalog_mut()
        .map_err(|e| format!("failed to read the document catalog: {e}"))?;
    catalog.remove(b"Outlines");
    if matches!(
        catalog.get(b"PageMode").map(|o| o.as_name()),
        Ok(Ok(b"UseOutlines"))
    ) {
        catalog.remove(b"PageMode");
    }
    Ok(WriteResult {
        bytes: save(&mut doc)?,
        total: 0,
        top_level: 0,
        removed,
        page_count,
        warnings: Vec::new(),
    })
}

/// One flat bookmark per page, titled from `label` with `{n}` replaced by the
/// page number (and `{total}` by the page count).
pub fn per_page(page_count: u32, label: &str) -> Vec<Bookmark> {
    let label = if label.trim().is_empty() {
        "Page {n}"
    } else {
        label
    };
    (1..=page_count)
        .map(|n| {
            let title = label
                .replace("{n}", &n.to_string())
                .replace("{total}", &page_count.to_string());
            Bookmark::new(title, n)
        })
        .collect()
}

/// Parse a bookmark spec: JSON (the shape [`list`] emits) when it starts with
/// `[`/`{`, otherwise the indented text outline.
pub fn parse_spec(spec: &str) -> Result<Vec<Bookmark>, String> {
    let trimmed = spec.trim_start();
    if trimmed.starts_with('[') || trimmed.starts_with('{') {
        parse_json_spec(trimmed)
    } else {
        parse_text_spec(spec)
    }
}

/// Parse the text outline format: one `Title | page [| attributes]` per line,
/// nesting by leading indentation, `#` for comments.
pub fn parse_text_spec(spec: &str) -> Result<Vec<Bookmark>, String> {
    let mut roots: Vec<Bookmark> = Vec::new();
    // (indent width, path of child indices into `roots`) for the open ancestors.
    let mut stack: Vec<(usize, Vec<usize>)> = Vec::new();

    for (idx, raw) in spec.lines().enumerate() {
        let lineno = idx + 1;
        if raw.trim().is_empty() || raw.trim_start().starts_with('#') {
            continue;
        }
        let indent = indent_width(raw);
        let bookmark = parse_line(raw.trim(), lineno)?;

        while let Some((top, _)) = stack.last() {
            if *top >= indent {
                stack.pop();
            } else {
                break;
            }
        }
        let path = match stack.last() {
            None => {
                roots.push(bookmark);
                vec![roots.len() - 1]
            }
            Some((_, parent_path)) => {
                let parent = node_at_mut(&mut roots, parent_path);
                parent.children.push(bookmark);
                let mut path = parent_path.clone();
                path.push(parent.children.len() - 1);
                path
            }
        };
        if path.len() > MAX_DEPTH {
            return Err(format!(
                "line {lineno}: indented {} levels deep, but at most {MAX_DEPTH} are supported",
                path.len()
            ));
        }
        stack.push((indent, path));
    }

    if roots.is_empty() {
        return Err(
            "no bookmarks found — write one per line as 'Title | page', e.g. 'Chapter 1 | 3'"
                .into(),
        );
    }
    Ok(roots)
}

/// Leading-whitespace width, counting a tab as 4 columns.
fn indent_width(line: &str) -> usize {
    line.chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .map(|c| if c == '\t' { 4 } else { 1 })
        .sum()
}

fn node_at_mut<'a>(roots: &'a mut Vec<Bookmark>, path: &[usize]) -> &'a mut Bookmark {
    let (first, rest) = path.split_first().expect("non-empty path");
    let mut node = &mut roots[*first];
    for i in rest {
        node = &mut node.children[*i];
    }
    node
}

/// Parse one already-trimmed spec line.
fn parse_line(line: &str, lineno: usize) -> Result<Bookmark, String> {
    let sep = if line.contains('|') { '|' } else { '\t' };
    let parts: Vec<&str> = line.split(sep).map(str::trim).collect();
    if parts.len() < 2 {
        return Err(format!(
            "line {lineno}: expected 'Title | page' (e.g. 'Chapter 1 | 3'), got {line:?}"
        ));
    }
    let title = |upto: usize| -> String { parts[..upto].join(&sep.to_string()).trim().to_string() };

    // The page number is the last field, or the second-to-last when a trailing
    // attributes field ("bold", "#c00000", …) is present.
    let last = parts.len() - 1;
    let (page_idx, attrs) = match parse_page(parts[last]) {
        Some(_) => (last, ""),
        None if parts.len() >= 3 && parse_page(parts[last - 1]).is_some() => {
            (last - 1, parts[last])
        }
        None => {
            return Err(format!(
                "line {lineno}: expected a 1-based page number after '{sep}', got {:?} — write \
                 'Title | page' or 'Title | page | bold #c00000'",
                parts[last]
            ))
        }
    };
    let page = parse_page(parts[page_idx]).expect("checked above");
    let title = title(page_idx);
    if title.is_empty() {
        return Err(format!("line {lineno}: bookmark title is empty"));
    }

    let mut bookmark = Bookmark::new(title, page);
    for token in attrs.split([' ', ',']).filter(|t| !t.is_empty()) {
        match token.to_ascii_lowercase().as_str() {
            "bold" => bookmark.bold = true,
            "italic" => bookmark.italic = true,
            other => {
                bookmark.color =
                    Some(parse_color(other).map_err(|e| format!("line {lineno}: {e}"))?)
            }
        }
    }
    Ok(bookmark)
}

/// A 1-based page number, or `None` when the field isn't one.
fn parse_page(field: &str) -> Option<u32> {
    let n: u32 = field.trim().trim_start_matches('p').trim().parse().ok()?;
    (n >= 1).then_some(n)
}

/// `#rgb`, `#rrggbb`, or a common colour name → PDF RGB in 0.0–1.0.
pub fn parse_color(spec: &str) -> Result<[f32; 3], String> {
    let s = spec.trim().to_ascii_lowercase();
    if let Some(hex) = s.strip_prefix('#') {
        let bytes: Vec<u8> = match hex.len() {
            3 => hex
                .chars()
                .map(|c| u8::from_str_radix(&c.to_string().repeat(2), 16))
                .collect::<Result<_, _>>()
                .map_err(|_| format!("invalid hex colour '{spec}'"))?,
            6 => (0..3)
                .map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16))
                .collect::<Result<_, _>>()
                .map_err(|_| format!("invalid hex colour '{spec}'"))?,
            _ => {
                return Err(format!(
                    "invalid colour '{spec}' (expected #rgb, #rrggbb, or a colour name)"
                ))
            }
        };
        return Ok([
            bytes[0] as f32 / 255.0,
            bytes[1] as f32 / 255.0,
            bytes[2] as f32 / 255.0,
        ]);
    }
    let named: [(&str, [u8; 3]); 18] = [
        ("black", [0, 0, 0]),
        ("white", [255, 255, 255]),
        ("red", [204, 0, 0]),
        ("green", [0, 128, 0]),
        ("blue", [0, 0, 204]),
        ("yellow", [230, 184, 0]),
        ("orange", [230, 115, 0]),
        ("purple", [128, 0, 128]),
        ("magenta", [204, 0, 204]),
        ("cyan", [0, 179, 179]),
        ("teal", [0, 128, 128]),
        ("navy", [0, 0, 128]),
        ("maroon", [128, 0, 0]),
        ("olive", [128, 128, 0]),
        ("brown", [140, 87, 51]),
        ("gray", [128, 128, 128]),
        ("grey", [128, 128, 128]),
        ("silver", [192, 192, 192]),
    ];
    named
        .iter()
        .find(|(name, _)| *name == s)
        .map(|(_, rgb)| [rgb[0] as f32 / 255.0, rgb[1] as f32 / 255.0, rgb[2] as f32 / 255.0])
        .ok_or_else(|| {
            format!("unknown bookmark attribute '{spec}' (expected 'bold', 'italic', or a colour like #c00000)")
        })
}

/// Parse the JSON spec: either a bare array of entries or `{"bookmarks": [...]}`
/// (so [`list`] output can be edited and fed straight back in).
fn parse_json_spec(spec: &str) -> Result<Vec<Bookmark>, String> {
    let value: serde_json::Value =
        serde_json::from_str(spec).map_err(|e| format!("invalid JSON bookmark list: {e}"))?;
    let array = match &value {
        serde_json::Value::Array(a) => a.clone(),
        serde_json::Value::Object(o) => match o.get("bookmarks") {
            Some(serde_json::Value::Array(a)) => a.clone(),
            _ => {
                return Err(
                    "JSON object must have a \"bookmarks\" array of {title, page} entries".into(),
                )
            }
        },
        _ => return Err("JSON spec must be an array of {title, page} entries".into()),
    };
    let out = json_nodes(&array, 1)?;
    if out.is_empty() {
        return Err("the JSON bookmark list is empty — provide at least one entry".into());
    }
    Ok(out)
}

fn json_nodes(items: &[serde_json::Value], depth: usize) -> Result<Vec<Bookmark>, String> {
    if depth > MAX_DEPTH {
        return Err(format!(
            "bookmarks nest deeper than the {MAX_DEPTH} supported levels"
        ));
    }
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let obj = item
            .as_object()
            .ok_or_else(|| format!("each bookmark must be an object, got {item}"))?;
        let title = obj
            .get("title")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("bookmark is missing a string \"title\": {item}"))?
            .trim()
            .to_string();
        if title.is_empty() {
            return Err(format!("bookmark title is empty: {item}"));
        }
        let page = match obj.get("page") {
            Some(serde_json::Value::Number(n)) => {
                n.as_u64().filter(|n| *n >= 1).ok_or_else(|| {
                    format!("bookmark \"{title}\" needs a 1-based integer \"page\", got {n}")
                })? as u32
            }
            Some(serde_json::Value::String(s)) => parse_page(s).ok_or_else(|| {
                format!("bookmark \"{title}\" needs a 1-based integer \"page\", got {s:?}")
            })?,
            _ => {
                return Err(format!(
                    "bookmark \"{title}\" is missing a 1-based integer \"page\""
                ))
            }
        };
        let mut bookmark = Bookmark::new(title, page);
        bookmark.bold = obj.get("bold").and_then(|v| v.as_bool()).unwrap_or(false);
        bookmark.italic = obj.get("italic").and_then(|v| v.as_bool()).unwrap_or(false);
        if let Some(c) = obj.get("color").and_then(|v| v.as_str()) {
            if !c.trim().is_empty() {
                bookmark.color = Some(parse_color(c)?);
            }
        }
        if let Some(kids) = obj.get("children") {
            let kids = kids
                .as_array()
                .ok_or_else(|| format!("\"children\" must be an array: {item}"))?;
            bookmark.children = json_nodes(kids, depth + 1)?;
        }
        out.push(bookmark);
    }
    Ok(out)
}

fn load(pdf: &[u8]) -> Result<Document, String> {
    Document::load_mem(pdf).map_err(|e| format!("failed to parse PDF: {e}"))
}

fn save(doc: &mut Document) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    doc.save_to(&mut out)
        .map_err(|e| format!("failed to serialize PDF: {e}"))?;
    Ok(out)
}

pub fn count_all(nodes: &[Bookmark]) -> usize {
    nodes.iter().map(|n| 1 + count_all(&n.children)).sum()
}

fn depth_of(nodes: &[Bookmark]) -> usize {
    nodes
        .iter()
        .map(|n| 1 + depth_of(&n.children))
        .max()
        .unwrap_or(0)
}

/// Force every entry onto a real page, recording what had to be repaired.
fn clamp_pages(nodes: &mut [Bookmark], page_count: u32, warnings: &mut Vec<String>) {
    for node in nodes.iter_mut() {
        match node.page {
            Some(p) if p > page_count => {
                warnings.push(format!(
                    "\"{}\" points at page {p} but the PDF has {page_count} page(s) — moved to page {page_count}",
                    node.title
                ));
                node.page = Some(page_count);
            }
            Some(_) => {}
            None => {
                warnings.push(format!(
                    "\"{}\" had no resolvable destination — moved to page 1",
                    node.title
                ));
                node.page = Some(1);
            }
        }
        clamp_pages(&mut node.children, page_count, warnings);
    }
}

struct Built {
    first: Option<ObjectId>,
    last: Option<ObjectId>,
    /// Entries a viewer shows for this level, given the open/closed state.
    visible: i64,
}

/// Emit one sibling level of outline items (recursing into children) and return
/// the level's `/First`, `/Last`, and visible-entry count.
fn emit_level(
    doc: &mut Document,
    parent: ObjectId,
    nodes: &[Bookmark],
    opts: &Options,
    page_ids: &[ObjectId],
) -> Built {
    let ids: Vec<ObjectId> = nodes.iter().map(|_| doc.new_object_id()).collect();
    let mut visible = nodes.len() as i64;
    for (i, node) in nodes.iter().enumerate() {
        let id = ids[i];
        let mut dict = Dictionary::new();
        dict.set("Title", encode_pdf_string(&node.title));
        dict.set("Parent", parent);
        if i > 0 {
            dict.set("Prev", ids[i - 1]);
        }
        if i + 1 < ids.len() {
            dict.set("Next", ids[i + 1]);
        }
        let page = node.page.unwrap_or(1).max(1) as usize;
        let page_id = page_ids[(page - 1).min(page_ids.len().saturating_sub(1))];
        dict.set("Dest", dest_array(page_id, opts.zoom));
        let flags = node.format_flags();
        if flags != 0 {
            dict.set("F", Object::Integer(flags));
        }
        if let Some(c) = node.color {
            dict.set(
                "C",
                Object::Array(vec![c[0].into(), c[1].into(), c[2].into()]),
            );
        }
        if !node.children.is_empty() {
            let sub = emit_level(doc, id, &node.children, opts, page_ids);
            if let Some(first) = sub.first {
                dict.set("First", first);
            }
            if let Some(last) = sub.last {
                dict.set("Last", last);
            }
            // Open items carry a positive count of everything they reveal;
            // closed items a negative count of what opening them would reveal.
            if opts.expanded {
                dict.set("Count", Object::Integer(sub.visible));
                visible += sub.visible;
            } else {
                dict.set("Count", Object::Integer(-(node.children.len() as i64)));
            }
        }
        doc.objects.insert(id, Object::Dictionary(dict));
    }
    Built {
        first: ids.first().copied(),
        last: ids.last().copied(),
        visible,
    }
}

/// The `/Dest` array for a page + zoom mode.
fn dest_array(page_id: ObjectId, zoom: Zoom) -> Object {
    let page = Object::Reference(page_id);
    Object::Array(match zoom {
        Zoom::Fit => vec![page, Object::Name(b"Fit".to_vec())],
        Zoom::FitWidth => vec![page, Object::Name(b"FitH".to_vec()), Object::Null],
        Zoom::Keep => vec![
            page,
            Object::Name(b"XYZ".to_vec()),
            Object::Null,
            Object::Null,
            Object::Null,
        ],
    })
}

/// Every object id belonging to the current outline tree (items plus their
/// `/A` action dictionaries), so a rewrite can delete the old tree.
fn outline_object_ids(doc: &Document) -> Vec<ObjectId> {
    let Some(root) = doc
        .catalog()
        .ok()
        .and_then(|c| c.get(b"Outlines").ok())
        .and_then(|o| o.as_reference().ok())
    else {
        return Vec::new();
    };
    let mut ids = vec![root];
    let mut seen: HashSet<ObjectId> = [root].into_iter().collect();
    let mut queue: Vec<ObjectId> = first_child(doc, root).into_iter().collect();
    while let Some(id) = queue.pop() {
        if !seen.insert(id) || ids.len() > MAX_READ_ITEMS {
            continue;
        }
        ids.push(id);
        let Ok(dict) = doc.get_dictionary(id) else {
            continue;
        };
        if let Ok(action) = dict.get(b"A").and_then(|o| o.as_reference()) {
            ids.push(action);
        }
        for key in [b"First".as_slice(), b"Next".as_slice()] {
            if let Ok(next) = dict.get(key).and_then(|o| o.as_reference()) {
                queue.push(next);
            }
        }
    }
    ids
}

fn first_child(doc: &Document, id: ObjectId) -> Option<ObjectId> {
    doc.get_dictionary(id)
        .ok()?
        .get(b"First")
        .ok()?
        .as_reference()
        .ok()
}

/// Read the document's outline into a [`Bookmark`] tree (empty when absent or
/// unreadable — a missing outline is not an error).
fn read_outline(doc: &Document, pages: &BTreeMap<u32, ObjectId>) -> Vec<Bookmark> {
    let page_numbers: Vec<(ObjectId, u32)> = pages.iter().map(|(n, id)| (*id, *n)).collect();
    let Some(first) = doc
        .catalog()
        .ok()
        .and_then(|c| c.get(b"Outlines").ok())
        .and_then(|o| o.as_reference().ok())
        .and_then(|root| first_child(doc, root))
    else {
        return Vec::new();
    };
    let mut seen = HashSet::new();
    read_siblings(doc, first, &page_numbers, &mut seen, 1)
}

fn read_siblings(
    doc: &Document,
    first: ObjectId,
    pages: &[(ObjectId, u32)],
    seen: &mut HashSet<ObjectId>,
    depth: usize,
) -> Vec<Bookmark> {
    let mut out = Vec::new();
    let mut current = Some(first);
    while let Some(id) = current {
        if !seen.insert(id) || seen.len() > MAX_READ_ITEMS || depth > MAX_DEPTH {
            break;
        }
        let Ok(dict) = doc.get_dictionary(id) else {
            break;
        };
        let title = dict
            .get(b"Title")
            .ok()
            .and_then(|o| resolve(doc, o).as_str().ok())
            .map(decode_pdf_string)
            .unwrap_or_default();
        let mut bookmark = Bookmark {
            title,
            page: dest_page(doc, dict, pages),
            bold: false,
            italic: false,
            color: read_color(doc, dict),
            children: Vec::new(),
        };
        if let Ok(flags) = dict.get(b"F").and_then(|o| resolve(doc, o).as_i64()) {
            bookmark.italic = flags & 1 != 0;
            bookmark.bold = flags & 2 != 0;
        }
        if let Ok(child) = dict.get(b"First").and_then(|o| o.as_reference()) {
            bookmark.children = read_siblings(doc, child, pages, seen, depth + 1);
        }
        out.push(bookmark);
        current = dict.get(b"Next").ok().and_then(|o| o.as_reference().ok());
    }
    out
}

/// Follow references (bounded) to the object a value really points at.
fn resolve<'a>(doc: &'a Document, object: &'a Object) -> &'a Object {
    let mut current = object;
    for _ in 0..8 {
        match current.as_reference() {
            Ok(id) => match doc.get_object(id) {
                Ok(next) => current = next,
                Err(_) => return current,
            },
            Err(_) => return current,
        }
    }
    current
}

fn read_color(doc: &Document, dict: &Dictionary) -> Option<[f32; 3]> {
    let array = resolve(doc, dict.get(b"C").ok()?).as_array().ok()?;
    if array.len() < 3 {
        return None;
    }
    let mut rgb = [0.0f32; 3];
    for (i, slot) in rgb.iter_mut().enumerate() {
        *slot = resolve(doc, &array[i]).as_float().ok()?.clamp(0.0, 1.0);
    }
    (rgb != [0.0, 0.0, 0.0]).then_some(rgb)
}

/// Resolve an outline item's destination to a 1-based page number, via `/Dest`
/// or a `/A` GoTo action, and either a page reference or a page index.
fn dest_page(doc: &Document, dict: &Dictionary, pages: &[(ObjectId, u32)]) -> Option<u32> {
    let dest = dict
        .get(b"Dest")
        .ok()
        .map(|o| resolve(doc, o))
        .or_else(|| {
            let action = resolve(doc, dict.get(b"A").ok()?).as_dict().ok()?;
            Some(resolve(doc, action.get(b"D").ok()?))
        })?;
    let target = match dest {
        Object::Array(a) => a.first()?,
        // A named destination needs the catalog's name tree, which this reader
        // does not walk; the caller reports it as unresolved.
        _ => return None,
    };
    match target {
        Object::Reference(id) => pages.iter().find(|(p, _)| p == id).map(|(_, n)| *n),
        Object::Integer(i) => u32::try_from(*i).ok().map(|i| i + 1),
        _ => None,
    }
}

/// Decode a PDF text string: UTF-16BE when it carries a BOM, else
/// PDFDocEncoding (approximated as Latin-1, exact for ASCII).
fn decode_pdf_string(bytes: &[u8]) -> String {
    if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
        let units: Vec<u16> = bytes[2..]
            .chunks(2)
            .map(|c| {
                if c.len() == 2 {
                    u16::from_be_bytes([c[0], c[1]])
                } else {
                    c[0] as u16
                }
            })
            .collect();
        String::from_utf16_lossy(&units)
    } else {
        bytes.iter().map(|&b| b as char).collect()
    }
}

/// Encode a Rust string as a PDF text string: a plain literal for ASCII, or
/// UTF-16BE with a leading BOM when any character is non-ASCII.
fn encode_pdf_string(s: &str) -> Object {
    if s.is_ascii() {
        Object::String(s.as_bytes().to_vec(), StringFormat::Literal)
    } else {
        let mut bytes = vec![0xFE, 0xFF];
        for unit in s.encode_utf16() {
            bytes.extend_from_slice(&unit.to_be_bytes());
        }
        Object::String(bytes, StringFormat::Literal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::dictionary;

    /// Build a minimal `pages`-page PDF with no outline.
    fn blank_pdf(pages: u32) -> Vec<u8> {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let kids: Vec<Object> = (0..pages)
            .map(|_| {
                doc.add_object(dictionary! {
                    "Type" => "Page",
                    "Parent" => pages_id,
                    "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
                })
                .into()
            })
            .collect();
        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages", "Kids" => kids, "Count" => pages as i64,
            }),
        );
        let catalog_id = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
        doc.trailer.set("Root", catalog_id);
        let mut out = Vec::new();
        doc.save_to(&mut out).unwrap();
        out
    }

    fn titles(nodes: &[Bookmark]) -> Vec<&str> {
        nodes.iter().map(|n| n.title.as_str()).collect()
    }

    #[test]
    fn parses_flat_text_spec() {
        let spec = "Cover | 1\nChapter 1 | 2\nChapter 2 | 7";
        let parsed = parse_spec(spec).unwrap();
        assert_eq!(titles(&parsed), ["Cover", "Chapter 1", "Chapter 2"]);
        assert_eq!(parsed[2].page, Some(7));
        assert!(parsed.iter().all(|n| n.children.is_empty()));
    }

    #[test]
    fn parses_nested_text_spec_with_comments_and_tabs() {
        let spec = "# outline\nPart I | 1\n\tChapter 1 | 2\n\t\tSection 1.1 | 3\n  \nPart II | 9";
        let parsed = parse_spec(spec).unwrap();
        assert_eq!(titles(&parsed), ["Part I", "Part II"]);
        assert_eq!(titles(&parsed[0].children), ["Chapter 1"]);
        assert_eq!(titles(&parsed[0].children[0].children), ["Section 1.1"]);
        assert_eq!(parsed[0].children[0].children[0].page, Some(3));
    }

    #[test]
    fn parses_attributes_and_tab_separator() {
        let parsed = parse_spec("Summary | 4 | bold #ff0000\nNotes\t12").unwrap();
        assert!(parsed[0].bold && !parsed[0].italic);
        assert_eq!(parsed[0].color, Some([1.0, 0.0, 0.0]));
        assert_eq!(parsed[1].page, Some(12));
        assert_eq!(parsed[1].color, None);
    }

    #[test]
    fn dedents_back_to_an_outer_level() {
        let parsed = parse_spec("A | 1\n    A1 | 2\n        A1a | 3\n    A2 | 4\nB | 5").unwrap();
        assert_eq!(titles(&parsed), ["A", "B"]);
        assert_eq!(titles(&parsed[0].children), ["A1", "A2"]);
        assert_eq!(titles(&parsed[0].children[0].children), ["A1a"]);
    }

    #[test]
    fn rejects_line_without_a_page() {
        let err = parse_spec("Chapter 1").unwrap_err();
        assert!(err.contains("expected 'Title | page'"), "got: {err}");
    }

    #[test]
    fn rejects_zero_and_unparseable_pages() {
        assert!(parse_spec("Intro | 0").unwrap_err().contains("page number"));
        assert!(parse_spec("Intro | ten")
            .unwrap_err()
            .contains("page number"));
    }

    #[test]
    fn rejects_unknown_attribute() {
        let err = parse_spec("Intro | 1 | sparkly").unwrap_err();
        assert!(err.contains("unknown bookmark attribute"), "got: {err}");
    }

    #[test]
    fn rejects_too_deep_indentation() {
        let mut spec = String::new();
        for level in 0..MAX_DEPTH + 1 {
            spec.push_str(&format!("{}L{level} | 1\n", "  ".repeat(level)));
        }
        let err = parse_spec(&spec).unwrap_err();
        assert!(err.contains("at most 6"), "got: {err}");
    }

    #[test]
    fn parses_json_spec_in_both_shapes() {
        let array = r#"[{"title":"Intro","page":1,"bold":true,"children":[{"title":"Why","page":2,"color":"navy"}]}]"#;
        let parsed = parse_spec(array).unwrap();
        assert!(parsed[0].bold);
        assert_eq!(parsed[0].children[0].page, Some(2));
        assert_eq!(parsed[0].children[0].color, Some([0.0, 0.0, 128.0 / 255.0]));

        let wrapped = r#"{"bookmarks":[{"title":"Intro","page":"3"}]}"#;
        assert_eq!(parse_spec(wrapped).unwrap()[0].page, Some(3));
    }

    #[test]
    fn rejects_json_entry_without_page() {
        let err = parse_spec(r#"[{"title":"Intro"}]"#).unwrap_err();
        assert!(err.contains("1-based integer \"page\""), "got: {err}");
    }

    #[test]
    fn applies_and_reads_back_a_nested_outline() {
        let pdf = blank_pdf(10);
        assert!(list(&pdf).unwrap().bookmarks.is_empty());

        let spec = parse_spec("Part I | 1\n  Chapter 1 | 2\n  Chapter 2 | 5\nPart II | 8").unwrap();
        let result = apply(&pdf, spec, &Options::default()).unwrap();
        assert_eq!((result.total, result.top_level, result.removed), (4, 2, 0));
        assert!(result.warnings.is_empty());

        let outline = list(&result.bytes).unwrap();
        assert_eq!(outline.page_count, 10);
        assert_eq!(outline.total, 4);
        assert_eq!(titles(&outline.bookmarks), ["Part I", "Part II"]);
        assert_eq!(
            titles(&outline.bookmarks[0].children),
            ["Chapter 1", "Chapter 2"]
        );
        assert_eq!(outline.bookmarks[0].children[1].page, Some(5));
        assert_eq!(outline.bookmarks[1].page, Some(8));
    }

    #[test]
    fn round_trips_styles_colors_and_non_ascii_titles() {
        let pdf = blank_pdf(3);
        let spec = parse_spec("Résumé — 概要 | 2 | bold italic #3366cc").unwrap();
        let result = apply(&pdf, spec, &Options::default()).unwrap();
        let read = list(&result.bytes).unwrap();
        let entry = &read.bookmarks[0];
        assert_eq!(entry.title, "Résumé — 概要");
        assert!(entry.bold && entry.italic);
        let c = entry.color.unwrap();
        assert!(
            (c[0] - 0.2).abs() < 0.01 && (c[2] - 0.8).abs() < 0.01,
            "got {c:?}"
        );
    }

    #[test]
    fn replace_drops_the_previous_outline_and_append_keeps_it() {
        let pdf = blank_pdf(4);
        let first = apply(&pdf, parse_spec("Old | 1").unwrap(), &Options::default()).unwrap();

        let replaced = apply(
            &first.bytes,
            parse_spec("New | 2").unwrap(),
            &Options::default(),
        )
        .unwrap();
        assert_eq!(replaced.removed, 1);
        assert_eq!(titles(&list(&replaced.bytes).unwrap().bookmarks), ["New"]);

        let appended = apply(
            &first.bytes,
            parse_spec("New | 3").unwrap(),
            &Options {
                replace: false,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(appended.total, 2);
        let read = list(&appended.bytes).unwrap();
        assert_eq!(titles(&read.bookmarks), ["Old", "New"]);
        assert_eq!(read.bookmarks[1].page, Some(3));
    }

    #[test]
    fn out_of_range_pages_clamp_with_a_warning() {
        let pdf = blank_pdf(3);
        let result = apply(
            &pdf,
            parse_spec("Appendix | 99").unwrap(),
            &Options::default(),
        )
        .unwrap();
        assert_eq!(result.warnings.len(), 1);
        assert!(
            result.warnings[0].contains("moved to page 3"),
            "{:?}",
            result.warnings
        );
        assert_eq!(list(&result.bytes).unwrap().bookmarks[0].page, Some(3));
    }

    #[test]
    fn collapsed_outline_uses_negative_child_counts() {
        let pdf = blank_pdf(4);
        let spec = parse_spec("Part | 1\n  Chapter | 2").unwrap();
        let collapsed = apply(
            &pdf,
            spec.clone(),
            &Options {
                expanded: false,
                ..Options::default()
            },
        )
        .unwrap();
        assert!(
            count_key(&collapsed.bytes, "Count") <= -1,
            "parent should be closed"
        );
        let expanded = apply(&pdf, spec, &Options::default()).unwrap();
        assert!(
            count_key(&expanded.bytes, "Count") >= 1,
            "parent should be open"
        );
    }

    /// The `/Count` of the first outline ITEM (not the root) in a rendered PDF.
    fn count_key(pdf: &[u8], key: &str) -> i64 {
        let doc = Document::load_mem(pdf).unwrap();
        let root = doc
            .catalog()
            .unwrap()
            .get(b"Outlines")
            .unwrap()
            .as_reference()
            .unwrap();
        let first = doc
            .get_dictionary(root)
            .unwrap()
            .get(b"First")
            .unwrap()
            .as_reference()
            .unwrap();
        doc.get_dictionary(first)
            .unwrap()
            .get(key.as_bytes())
            .unwrap()
            .as_i64()
            .unwrap()
    }

    #[test]
    fn show_pane_sets_and_clears_the_outline_page_mode() {
        let pdf = blank_pdf(2);
        let shown = apply(&pdf, parse_spec("A | 1").unwrap(), &Options::default()).unwrap();
        assert_eq!(page_mode(&shown.bytes).as_deref(), Some("UseOutlines"));
        let hidden = apply(
            &shown.bytes,
            parse_spec("A | 1").unwrap(),
            &Options {
                show_pane: false,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(page_mode(&hidden.bytes), None);
    }

    fn page_mode(pdf: &[u8]) -> Option<String> {
        let doc = Document::load_mem(pdf).unwrap();
        let name = doc
            .catalog()
            .unwrap()
            .get(b"PageMode")
            .ok()?
            .as_name()
            .ok()?;
        Some(String::from_utf8_lossy(name).to_string())
    }

    #[test]
    fn zoom_modes_emit_the_expected_destination() {
        let pdf = blank_pdf(2);
        for (zoom, expected) in [
            (Zoom::Fit, "/Fit"),
            (Zoom::FitWidth, "/FitH"),
            (Zoom::Keep, "/XYZ"),
        ] {
            let result = apply(
                &pdf,
                parse_spec("A | 2").unwrap(),
                &Options {
                    zoom,
                    ..Options::default()
                },
            )
            .unwrap();
            let rendered = String::from_utf8_lossy(&result.bytes).to_string();
            assert!(
                rendered.contains(expected),
                "{zoom:?} should emit {expected}"
            );
            // The destination must still resolve to the right page.
            assert_eq!(list(&result.bytes).unwrap().bookmarks[0].page, Some(2));
        }
    }

    #[test]
    fn per_page_labels_every_page() {
        let pdf = blank_pdf(3);
        let result = apply(
            &pdf,
            per_page(3, "Sheet {n} of {total}"),
            &Options::default(),
        )
        .unwrap();
        assert_eq!(
            titles(&list(&result.bytes).unwrap().bookmarks),
            ["Sheet 1 of 3", "Sheet 2 of 3", "Sheet 3 of 3"]
        );
        assert_eq!(per_page(2, "")[0].title, "Page 1");
    }

    #[test]
    fn remove_strips_the_outline_and_the_page_mode() {
        let pdf = blank_pdf(2);
        let with = apply(
            &pdf,
            parse_spec("A | 1\n  B | 2").unwrap(),
            &Options::default(),
        )
        .unwrap();
        let stripped = remove(&with.bytes).unwrap();
        assert_eq!(stripped.removed, 2);
        assert!(list(&stripped.bytes).unwrap().bookmarks.is_empty());
        assert_eq!(page_mode(&stripped.bytes), None);
    }

    #[test]
    fn remove_on_a_pdf_without_bookmarks_errors() {
        let err = remove(&blank_pdf(1)).unwrap_err();
        assert!(err.contains("no bookmarks to remove"), "got: {err}");
    }

    #[test]
    fn apply_rejects_an_empty_tree() {
        let err = apply(&blank_pdf(1), Vec::new(), &Options::default()).unwrap_err();
        assert!(err.contains("no bookmarks to write"), "got: {err}");
    }

    #[test]
    fn rejects_non_pdf_bytes() {
        assert!(list(b"not a pdf").unwrap_err().contains("failed to parse"));
        assert!(apply(
            b"not a pdf",
            vec![Bookmark::new("A", 1)],
            &Options::default()
        )
        .unwrap_err()
        .contains("failed to parse"));
    }

    #[test]
    fn zoom_parse_accepts_aliases_and_rejects_junk() {
        assert_eq!(Zoom::parse("FIT-WIDTH").unwrap(), Zoom::FitWidth);
        assert_eq!(Zoom::parse("").unwrap(), Zoom::Fit);
        assert!(Zoom::parse("magnify").unwrap_err().contains("invalid zoom"));
    }
}
