//! gizza-ai/odt-to-markdown core — convert an OpenDocument Text (`.odt`) file
//! into clean Markdown (or plain text).
//!
//! An `.odt` is a ZIP container: `content.xml` holds the document body in ODF
//! XML, `styles.xml` the named styles, `meta.xml` the title/author metadata. We
//! read the ZIP with `zip`, build a small DOM with `quick-xml`, then render the
//! `office:text` body. Flat OpenDocument XML (`.fodt`, a single XML file with no
//! ZIP wrapper) is accepted too.
//!
//! Pure Rust, no I/O — safe on every backend including the chat Service Worker.

use std::collections::{HashMap, HashSet};
use std::io::{Cursor, Read};

use quick_xml::events::Event;
use quick_xml::Reader;
use serde::Serialize;

/// Repeat counts (`number-columns-repeated`) are clamped — LibreOffice writes
/// 1024 for the empty filler columns at the right edge of a table.
const MAX_REPEAT: usize = 64;
/// Style inheritance (`style:parent-style-name`) chain depth cap.
const MAX_STYLE_DEPTH: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Markdown,
    Text,
}

impl Mode {
    pub fn parse(s: &str) -> Result<Mode, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "markdown" | "md" | "" => Ok(Mode::Markdown),
            "text" | "txt" | "plain" => Ok(Mode::Text),
            other => Err(format!("unknown format '{other}' (use markdown or text)")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Doc {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creator: Option<String>,
    pub content: String,
    pub paragraphs: usize,
    pub tables: usize,
    pub images: usize,
}

// ---------------------------------------------------------------- tiny XML DOM

#[derive(Debug)]
enum Child {
    Text(String),
    Elem(Node),
}

#[derive(Debug, Default)]
struct Node {
    /// Local name with any `ns:` prefix stripped (`text:h` → `h`).
    name: String,
    /// Attributes keyed by local name (`xlink:href` → `href`).
    attrs: Vec<(String, String)>,
    children: Vec<Child>,
}

impl Node {
    fn attr(&self, key: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    fn elems(&self) -> impl Iterator<Item = &Node> {
        self.children.iter().filter_map(|c| match c {
            Child::Elem(n) => Some(n),
            Child::Text(_) => None,
        })
    }

    /// First descendant (pre-order) with this local name.
    fn find(&self, name: &str) -> Option<&Node> {
        for child in self.elems() {
            if child.name == name {
                return Some(child);
            }
            if let Some(found) = child.find(name) {
                return Some(found);
            }
        }
        None
    }

    /// Concatenated raw text of the whole subtree.
    fn text(&self) -> String {
        fn walk(n: &Node, out: &mut String) {
            for c in &n.children {
                match c {
                    Child::Text(t) => out.push_str(t),
                    Child::Elem(e) => walk(e, out),
                }
            }
        }
        let mut out = String::new();
        walk(self, &mut out);
        out
    }
}

fn local_name(raw: &[u8]) -> &[u8] {
    match raw.iter().position(|&b| b == b':') {
        Some(i) => &raw[i + 1..],
        None => raw,
    }
}

fn node_from(e: &quick_xml::events::BytesStart) -> Node {
    let attrs = e
        .attributes()
        .flatten()
        .map(|a| {
            let key = String::from_utf8_lossy(local_name(a.key.as_ref())).into_owned();
            let val = a
                .unescape_value()
                .map(|v| v.into_owned())
                .unwrap_or_else(|_| String::from_utf8_lossy(&a.value).into_owned());
            (key, val)
        })
        .collect();
    Node {
        name: String::from_utf8_lossy(local_name(e.name().as_ref())).into_owned(),
        attrs,
        children: Vec::new(),
    }
}

/// Resolve a `&name;` / `&#nn;` reference. quick-xml reports these separately
/// from text, so predefined entities have to be mapped back by hand.
fn entity_text(r: &quick_xml::events::BytesRef) -> String {
    if let Ok(Some(c)) = r.resolve_char_ref() {
        return c.to_string();
    }
    match r.decode().unwrap_or_default().as_ref() {
        "amp" => "&",
        "lt" => "<",
        "gt" => ">",
        "apos" => "'",
        "quot" => "\"",
        _ => "",
    }
    .to_string()
}

/// Parse XML into a DOM. Malformed tails are tolerated: whatever parsed before
/// the error is kept so a slightly-broken document still yields its text.
fn parse_xml(xml: &str) -> Node {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().allow_dangling_amp = true;
    reader.config_mut().check_end_names = false;

    let mut stack: Vec<Node> = vec![Node::default()];
    let push_text = |stack: &mut Vec<Node>, s: String| {
        if s.is_empty() {
            return;
        }
        let top = stack.last_mut().expect("root always present");
        match top.children.last_mut() {
            Some(Child::Text(prev)) => prev.push_str(&s),
            _ => top.children.push(Child::Text(s)),
        }
    };

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => stack.push(node_from(&e)),
            Ok(Event::Empty(e)) => {
                let n = node_from(&e);
                stack
                    .last_mut()
                    .expect("root always present")
                    .children
                    .push(Child::Elem(n));
            }
            Ok(Event::End(_)) => {
                if stack.len() > 1 {
                    let n = stack.pop().expect("len > 1");
                    stack
                        .last_mut()
                        .expect("root always present")
                        .children
                        .push(Child::Elem(n));
                }
            }
            Ok(Event::Text(t)) => {
                push_text(&mut stack, t.decode().unwrap_or_default().into_owned())
            }
            Ok(Event::CData(t)) => {
                push_text(&mut stack, t.decode().unwrap_or_default().into_owned())
            }
            Ok(Event::GeneralRef(r)) => push_text(&mut stack, entity_text(&r)),
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    // Close anything still open so no content is dropped.
    while stack.len() > 1 {
        let n = stack.pop().expect("len > 1");
        stack
            .last_mut()
            .expect("root always present")
            .children
            .push(Child::Elem(n));
    }
    stack.pop().expect("root always present")
}

// ------------------------------------------------------------------- ODF styles

#[derive(Default, Clone)]
struct RawStyle {
    bold: Option<bool>,
    italic: Option<bool>,
    parent: Option<String>,
}

#[derive(Default)]
struct Styles {
    bold: HashSet<String>,
    italic: HashSet<String>,
    /// `text:list-style` name → true when the list is numbered.
    lists: HashMap<String, bool>,
}

fn is_bold(weight: &str) -> bool {
    let w = weight.trim().to_ascii_lowercase();
    w == "bold" || w == "bolder" || w.parse::<u32>().map(|n| n >= 600).unwrap_or(false)
}

fn collect_styles(
    node: &Node,
    raw: &mut HashMap<String, RawStyle>,
    lists: &mut HashMap<String, bool>,
) {
    for child in node.elems() {
        match child.name.as_str() {
            "style" | "default-style" => {
                if let Some(name) = child.attr("name") {
                    let mut st = RawStyle {
                        parent: child.attr("parent-style-name").map(str::to_string),
                        ..RawStyle::default()
                    };
                    if let Some(tp) = child.find("text-properties") {
                        st.bold = tp.attr("font-weight").map(is_bold);
                        st.italic = tp
                            .attr("font-style")
                            .map(|v| matches!(v.trim(), "italic" | "oblique"));
                    }
                    raw.insert(name.to_string(), st);
                }
            }
            "list-style" => {
                if let Some(name) = child.attr("name") {
                    lists.insert(
                        name.to_string(),
                        child.find("list-level-style-number").is_some(),
                    );
                }
            }
            _ => {}
        }
        collect_styles(child, raw, lists);
    }
}

/// Flatten the `parent-style-name` chains into "is this style bold / italic".
fn resolve_styles(raw: &HashMap<String, RawStyle>) -> (HashSet<String>, HashSet<String>) {
    let (mut bold, mut italic) = (HashSet::new(), HashSet::new());
    for name in raw.keys() {
        let (mut b, mut i) = (None, None);
        let mut cur = Some(name.clone());
        for _ in 0..MAX_STYLE_DEPTH {
            let Some(key) = cur.take() else { break };
            let Some(st) = raw.get(&key) else { break };
            if b.is_none() {
                b = st.bold;
            }
            if i.is_none() {
                i = st.italic;
            }
            if b.is_some() && i.is_some() {
                break;
            }
            cur = st.parent.clone();
        }
        if b == Some(true) {
            bold.insert(name.clone());
        }
        if i == Some(true) {
            italic.insert(name.clone());
        }
    }
    (bold, italic)
}

// ----------------------------------------------------------------- text helpers

/// ODF collapses runs of XML whitespace to one space (that is what `text:s` is
/// for), so do the same before emitting.
fn collapse_ws(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_ws = false;
    for c in s.chars() {
        if c.is_whitespace() {
            if !in_ws {
                out.push(' ');
            }
            in_ws = true;
        } else {
            out.push(c);
            in_ws = false;
        }
    }
    out
}

/// Escape the inline Markdown metacharacters. `<`/`>` are escaped too so a
/// document can never smuggle raw HTML into the output.
fn escape_md_into(s: &str, out: &mut String) {
    for c in s.chars() {
        if matches!(c, '\\' | '*' | '_' | '[' | ']' | '`' | '<' | '>' | '|') {
            out.push('\\');
        }
        out.push(c);
    }
}

fn escape_md(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    escape_md_into(s, &mut out);
    out
}

/// A paragraph starting with `#`, `-`, `1.` … would become a heading/list, so
/// escape the first character. (`*`, `_`, `[`, `` ` ``, `>` are escaped inline.)
fn escape_leading(s: &str) -> String {
    let first = match s.chars().next() {
        Some(c) => c,
        None => return s.to_string(),
    };
    let looks_like_block = matches!(first, '#' | '-' | '+' | '=' | '~' | ':')
        || (first.is_ascii_digit()
            && s.chars()
                .skip_while(|c| c.is_ascii_digit())
                .next()
                .is_some_and(|c| c == '.' || c == ')'));
    if !looks_like_block {
        return s.to_string();
    }
    if first.is_ascii_digit() {
        // Escape the delimiter, not the digits: `1\. item`.
        let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
        return format!("{}\\{}", digits, &s[digits.len()..]);
    }
    format!("\\{s}")
}

/// Wrap `s` in emphasis markers, keeping any surrounding spaces outside them
/// (`** bold **` is not emphasis in Markdown).
fn wrap_emphasis(s: &str, bold: bool, italic: bool) -> String {
    if (!bold && !italic) || s.trim().is_empty() {
        return s.to_string();
    }
    let lead: String = s.chars().take_while(|c| c.is_whitespace()).collect();
    let trail: String = {
        let rev: String = s.chars().rev().take_while(|c| c.is_whitespace()).collect();
        rev.chars().rev().collect()
    };
    let core = &s[lead.len()..s.len() - trail.len()];
    let marker = match (bold, italic) {
        (true, true) => "***",
        (true, false) => "**",
        _ => "*",
    };
    format!("{lead}{marker}{core}{marker}{trail}")
}

/// Schemes that must never become a clickable Markdown link.
fn is_unsafe_url(url: &str) -> bool {
    let head: String = url
        .chars()
        .take_while(|c| *c != ':')
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_ascii_lowercase();
    matches!(head.as_str(), "javascript" | "vbscript" | "data" | "file")
}

fn fmt_url(url: &str) -> String {
    let t = url
        .trim()
        .replace(' ', "%20")
        .replace('<', "%3C")
        .replace('>', "%3E");
    if t.contains('(') || t.contains(')') {
        format!("<{t}>")
    } else {
        t
    }
}

fn flatten(s: &str) -> String {
    collapse_ws(s).trim().to_string()
}

/// Markdown table cells are single-line; keep the line structure as `<br>`.
fn flatten_cell(s: &str, mode: Mode) -> String {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    match mode {
        Mode::Text => flatten(trimmed),
        Mode::Markdown => trimmed
            .lines()
            .map(str::trim_end)
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>()
            .join("<br>"),
    }
}

/// Prefix the first line with `marker` and indent the rest to line up under it.
fn indent_item(body: &str, marker: &str) -> String {
    let pad = " ".repeat(marker.chars().count());
    let mut out = String::new();
    for (i, line) in body.lines().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        if i == 0 {
            out.push_str(marker);
            out.push_str(line);
        } else if !line.is_empty() {
            out.push_str(&pad);
            out.push_str(line);
        }
    }
    out
}

fn indent_block(body: &str, spaces: usize) -> String {
    let pad = " ".repeat(spaces);
    body.lines()
        .map(|l| {
            if l.is_empty() {
                String::new()
            } else {
                format!("{pad}{l}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// `text:h` level: `text:outline-level` when present, else the trailing number
/// of the style name (`Heading_20_2` → 2).
fn heading_level(n: &Node) -> usize {
    if let Some(v) = n
        .attr("outline-level")
        .and_then(|v| v.trim().parse::<usize>().ok())
    {
        return v.clamp(1, 6);
    }
    if let Some(style) = n.attr("style-name") {
        let last = style
            .split(|c: char| !c.is_ascii_digit())
            .filter(|s| !s.is_empty())
            .next_back()
            .and_then(|s| s.parse::<usize>().ok());
        if let Some(v) = last.filter(|v| *v > 0) {
            return v.clamp(1, 6);
        }
    }
    1
}

// --------------------------------------------------------------------- renderer

/// Elements whose content is inline even when they turn up at block level
/// (an anchored image or a stray hyperlink directly under `office:text`).
fn is_inline_elem(name: &str) -> bool {
    matches!(
        name,
        "span" | "a" | "frame" | "note" | "line-break" | "tab" | "s" | "image"
    )
}

/// Elements carrying no document text (or text the reader must not see, such
/// as comments and rejected tracked changes).
fn is_skipped_elem(name: &str) -> bool {
    matches!(
        name,
        "annotation"
            | "annotation-end"
            | "tracked-changes"
            | "sequence-decls"
            | "variable-decls"
            | "user-field-decls"
            | "forms"
            | "soft-page-break"
            | "note-citation"
    )
}

struct Renderer<'a> {
    mode: Mode,
    styles: &'a Styles,
    notes: Vec<String>,
    paragraphs: usize,
    tables: usize,
    images: usize,
    /// Numbering of the list currently being rendered, for nested lists that
    /// omit their own `text:style-name`.
    list_ordered: bool,
}

impl<'a> Renderer<'a> {
    fn new(mode: Mode, styles: &'a Styles) -> Self {
        Renderer {
            mode,
            styles,
            notes: Vec::new(),
            paragraphs: 0,
            tables: 0,
            images: 0,
            list_ordered: false,
        }
    }

    // --- block level

    fn blocks(&mut self, children: &[Child]) -> String {
        let mut out: Vec<String> = Vec::new();
        for child in children {
            match child {
                Child::Text(t) => {
                    let s = collapse_ws(t);
                    if !s.trim().is_empty() {
                        let mut buf = String::new();
                        self.push_text(&mut buf, &s);
                        out.push(buf.trim().to_string());
                    }
                }
                Child::Elem(n) => {
                    if let Some(s) = self.block_elem(n) {
                        if !s.trim().is_empty() {
                            out.push(s);
                        }
                    }
                }
            }
        }
        out.join("\n\n")
    }

    fn block_elem(&mut self, n: &Node) -> Option<String> {
        if is_skipped_elem(&n.name) {
            return None;
        }
        match n.name.as_str() {
            "h" => {
                self.paragraphs += 1;
                let mut buf = String::new();
                self.inline(&n.children, &mut buf);
                let text = flatten(&buf);
                if text.is_empty() {
                    return None;
                }
                Some(match self.mode {
                    Mode::Markdown => format!("{} {}", "#".repeat(heading_level(n)), text),
                    Mode::Text => text,
                })
            }
            "p" => {
                self.paragraphs += 1;
                let mut buf = String::new();
                self.inline(&n.children, &mut buf);
                let text = buf.trim().to_string();
                if text.is_empty() {
                    return None;
                }
                Some(match self.mode {
                    Mode::Markdown => escape_leading(&text),
                    Mode::Text => text,
                })
            }
            "list" => {
                let s = self.list(n, self.list_ordered);
                (!s.trim().is_empty()).then_some(s)
            }
            "table" => {
                self.tables += 1;
                let s = self.table(n);
                (!s.trim().is_empty()).then_some(s)
            }
            _ if is_inline_elem(&n.name) => {
                let mut buf = String::new();
                self.inline_elem(n, &mut buf);
                let text = buf.trim().to_string();
                (!text.is_empty()).then_some(text)
            }
            // Containers we do not model (text:section, text:index-body,
            // office:text itself, …): keep their block children.
            _ => {
                let s = self.blocks(&n.children);
                (!s.trim().is_empty()).then_some(s)
            }
        }
    }

    fn list(&mut self, n: &Node, parent_ordered: bool) -> String {
        let ordered = n
            .attr("style-name")
            .and_then(|s| self.styles.lists.get(s).copied())
            .unwrap_or(parent_ordered);
        let prev_ordered = self.list_ordered;
        self.list_ordered = ordered;

        let mut items: Vec<String> = Vec::new();
        let mut index = 1usize;
        for child in n.elems() {
            match child.name.as_str() {
                "list-item" | "list-header" => {
                    let body = self.blocks(&child.children).replace("\n\n", "\n");
                    if body.trim().is_empty() {
                        continue;
                    }
                    let marker = if child.name == "list-header" {
                        String::new()
                    } else if ordered {
                        let m = format!("{index}. ");
                        index += 1;
                        m
                    } else {
                        "- ".to_string()
                    };
                    items.push(indent_item(&body, &marker));
                }
                // A nested list that is a direct child of the list.
                "list" => {
                    let nested = self.list(child, ordered);
                    if !nested.trim().is_empty() {
                        items.push(indent_block(&nested, 2));
                    }
                }
                _ => {}
            }
        }

        self.list_ordered = prev_ordered;
        items.join("\n")
    }

    fn table(&mut self, n: &Node) -> String {
        let mut rows: Vec<Vec<String>> = Vec::new();
        self.collect_rows(n, &mut rows);
        rows.retain(|r| !r.is_empty());
        if rows.is_empty() {
            return String::new();
        }
        let width = rows.iter().map(Vec::len).max().unwrap_or(0);
        match self.mode {
            Mode::Text => rows
                .iter()
                .map(|r| r.join(" | "))
                .collect::<Vec<_>>()
                .join("\n"),
            Mode::Markdown => {
                let mut out = String::new();
                for (i, row) in rows.iter().enumerate() {
                    let mut cells = row.clone();
                    cells.resize(width, String::new());
                    out.push_str(&format!("| {} |\n", cells.join(" | ")));
                    if i == 0 {
                        out.push_str(&format!("|{}|\n", vec![" --- "; width].join("|")));
                    }
                }
                out.trim_end().to_string()
            }
        }
    }

    fn collect_rows(&mut self, n: &Node, rows: &mut Vec<Vec<String>>) {
        for child in n.elems() {
            match child.name.as_str() {
                "table-row" => {
                    let cells = self.row_cells(child);
                    let repeat = child
                        .attr("number-rows-repeated")
                        .and_then(|v| v.trim().parse::<usize>().ok())
                        .unwrap_or(1)
                        .clamp(1, MAX_REPEAT);
                    // Repeated *empty* rows are LibreOffice filler, not content.
                    let repeat = if cells.is_empty() { 1 } else { repeat };
                    for _ in 0..repeat {
                        rows.push(cells.clone());
                    }
                }
                // Row groupings: table:table-header-rows / table-rows / row-group.
                "table-header-rows" | "table-rows" | "table-row-group" => {
                    self.collect_rows(child, rows)
                }
                _ => {}
            }
        }
    }

    fn row_cells(&mut self, row: &Node) -> Vec<String> {
        let mut cells: Vec<String> = Vec::new();
        for child in row.elems() {
            let (is_cell, covered) = match child.name.as_str() {
                "table-cell" => (true, false),
                "covered-table-cell" => (true, true),
                _ => (false, false),
            };
            if !is_cell {
                continue;
            }
            let text = if covered {
                String::new()
            } else {
                let body = self.blocks(&child.children);
                flatten_cell(&body, self.mode)
            };
            let repeat = child
                .attr("number-columns-repeated")
                .and_then(|v| v.trim().parse::<usize>().ok())
                .unwrap_or(1)
                .clamp(1, MAX_REPEAT);
            // Don't multiply the 1024 empty filler cells at the row's edge.
            let repeat = if text.is_empty() { 1 } else { repeat };
            for _ in 0..repeat {
                cells.push(text.clone());
            }
        }
        while cells.last().is_some_and(String::is_empty) {
            cells.pop();
        }
        cells
    }

    // --- inline level

    fn push_text(&self, out: &mut String, s: &str) {
        // Whitespace was already collapsed per text node; don't let the join of
        // two nodes produce a double space.
        let s = if s.starts_with(' ')
            && (out.is_empty() || out.ends_with(' ') || out.ends_with('\n'))
        {
            &s[1..]
        } else {
            s
        };
        if s.is_empty() {
            return;
        }
        match self.mode {
            Mode::Markdown => escape_md_into(s, out),
            Mode::Text => out.push_str(s),
        }
    }

    fn inline(&mut self, children: &[Child], out: &mut String) {
        for child in children {
            match child {
                Child::Text(t) => {
                    let s = collapse_ws(t);
                    self.push_text(out, &s);
                }
                Child::Elem(n) => self.inline_elem(n, out),
            }
        }
    }

    fn inline_elem(&mut self, n: &Node, out: &mut String) {
        if is_skipped_elem(&n.name) {
            return;
        }
        match n.name.as_str() {
            "line-break" => out.push_str(match self.mode {
                Mode::Markdown => "  \n",
                Mode::Text => "\n",
            }),
            "tab" => out.push_str(match self.mode {
                Mode::Markdown => " ",
                Mode::Text => "\t",
            }),
            "s" => {
                let count = n
                    .attr("c")
                    .and_then(|v| v.trim().parse::<usize>().ok())
                    .unwrap_or(1)
                    .clamp(1, MAX_REPEAT);
                for _ in 0..count {
                    out.push(' ');
                }
            }
            "span" => {
                let mut inner = String::new();
                self.inline(&n.children, &mut inner);
                let style = n.attr("style-name").unwrap_or("");
                let markdown = self.mode == Mode::Markdown;
                let bold = markdown && self.styles.bold.contains(style);
                let italic = markdown && self.styles.italic.contains(style);
                out.push_str(&wrap_emphasis(&inner, bold, italic));
            }
            "a" => {
                let mut inner = String::new();
                self.inline(&n.children, &mut inner);
                let href = n.attr("href").unwrap_or("").trim();
                let label_empty = inner.trim().is_empty();
                match self.mode {
                    Mode::Text => {
                        if label_empty {
                            out.push_str(href);
                        } else {
                            out.push_str(&inner);
                        }
                    }
                    Mode::Markdown => {
                        if href.is_empty() || is_unsafe_url(href) {
                            // Never emit a clickable javascript:/data: link.
                            if label_empty {
                                out.push_str(&escape_md(href));
                            } else {
                                out.push_str(&inner);
                            }
                        } else {
                            let label = if label_empty { escape_md(href) } else { inner };
                            out.push_str(&format!("[{label}]({})", fmt_url(href)));
                        }
                    }
                }
            }
            "frame" => self.frame(n, out),
            "image" => self.image(n, None, out),
            "note" => {
                // Reserve the number before rendering: a note may contain notes.
                self.notes.push(String::new());
                let index = self.notes.len();
                let body = n
                    .find("note-body")
                    .map(|b| {
                        let blocks = self.blocks(&b.children);
                        flatten(&blocks)
                    })
                    .unwrap_or_default();
                self.notes[index - 1] = body;
                out.push_str(&match self.mode {
                    Mode::Markdown => format!("[^{index}]"),
                    Mode::Text => format!("[{index}]"),
                });
            }
            // Fields, bookmarks, references, ruby … — keep their text.
            _ => self.inline(&n.children, out),
        }
    }

    /// `draw:frame` wraps an image, a text box or an embedded object.
    fn frame(&mut self, n: &Node, out: &mut String) {
        if let Some(img) = n.find("image") {
            let alt = n
                .find("title")
                .map(Node::text)
                .filter(|s| !s.trim().is_empty())
                .or_else(|| {
                    n.find("desc")
                        .map(Node::text)
                        .filter(|s| !s.trim().is_empty())
                })
                .or_else(|| n.attr("name").map(str::to_string))
                .unwrap_or_else(|| "image".to_string());
            self.image(img, Some(flatten(&alt)), out);
            return;
        }
        if let Some(text_box) = n.find("text-box") {
            let body = self.blocks(&text_box.children);
            let flat = flatten(&body);
            if !flat.is_empty() {
                if !out.is_empty() && !out.ends_with(' ') {
                    out.push(' ');
                }
                out.push_str(&flat);
            }
        }
    }

    fn image(&mut self, img: &Node, alt: Option<String>, out: &mut String) {
        let href = img.attr("href").unwrap_or("").trim();
        self.images += 1;
        if self.mode != Mode::Markdown || href.is_empty() || is_unsafe_url(href) {
            return;
        }
        let alt = alt.unwrap_or_else(|| "image".to_string());
        out.push_str(&format!("![{}]({})", escape_md(&alt), fmt_url(href)));
    }
}

// ------------------------------------------------------------------ entry point

fn read_entry(zip: &mut zip::ZipArchive<Cursor<&[u8]>>, name: &str) -> Option<String> {
    let mut f = zip.by_name(name).ok()?;
    let mut s = String::new();
    f.read_to_string(&mut s).ok()?;
    Some(s)
}

fn meta_field(meta: &Node, name: &str) -> Option<String> {
    meta.find(name)
        .map(|n| flatten(&n.text()))
        .filter(|s| !s.is_empty())
}

/// Convert an OpenDocument Text file (`.odt` ZIP, or flat `.fodt` XML) into
/// Markdown or plain text.
pub fn convert(bytes: &[u8], mode: Mode) -> Result<Doc, String> {
    if bytes.is_empty() {
        return Err("input is empty".into());
    }

    let (content_xml, styles_xml, meta_xml) = if bytes.starts_with(b"PK") {
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes))
            .map_err(|e| format!("not a valid ODT (ZIP) container: {e}"))?;
        let content = read_entry(&mut zip, "content.xml")
            .ok_or("this ZIP has no content.xml — expected an OpenDocument Text (.odt) file")?;
        let styles = read_entry(&mut zip, "styles.xml");
        let meta = read_entry(&mut zip, "meta.xml");
        (content, styles, meta)
    } else {
        // Flat OpenDocument XML (.fodt): one XML file, no ZIP wrapper.
        let text = std::str::from_utf8(bytes).map_err(|_| {
            "input is neither an ODT (ZIP) container nor flat OpenDocument XML (.fodt)".to_string()
        })?;
        if !text.contains("office:document") {
            return Err(
                "input is neither an ODT (ZIP) container nor flat OpenDocument XML (.fodt)".into(),
            );
        }
        (text.to_string(), None, None)
    };

    let content = parse_xml(&content_xml);
    let styles_doc = styles_xml.as_deref().map(parse_xml);
    let meta_doc = meta_xml.as_deref().map(parse_xml);

    let mut raw = HashMap::new();
    let mut lists = HashMap::new();
    if let Some(doc) = &styles_doc {
        collect_styles(doc, &mut raw, &mut lists);
    }
    collect_styles(&content, &mut raw, &mut lists);
    let (bold, italic) = resolve_styles(&raw);
    let styles = Styles {
        bold,
        italic,
        lists,
    };

    // office:body → office:text. Look the body up first: office:automatic-styles
    // can contain a <number:text> element that a plain search would hit first.
    let body = content
        .find("body")
        .ok_or("content.xml has no office:body — not an OpenDocument file")?;
    let text_body = body
        .find("text")
        .ok_or("this OpenDocument file is not a *text* document (no office:text body)")?;

    let mut renderer = Renderer::new(mode, &styles);
    let mut out = renderer.blocks(&text_body.children);

    if !renderer.notes.is_empty() {
        let notes = renderer
            .notes
            .iter()
            .enumerate()
            .map(|(i, body)| match mode {
                Mode::Markdown => format!("[^{}]: {}", i + 1, body),
                Mode::Text => format!("[{}] {}", i + 1, body),
            })
            .collect::<Vec<_>>()
            .join("\n");
        out.push_str("\n\n");
        out.push_str(&notes);
    }

    if out.trim().is_empty() {
        return Err("the document contained no extractable text".into());
    }

    let meta_node = meta_doc
        .as_ref()
        .and_then(|d| d.find("meta"))
        .or_else(|| content.find("meta"));
    let (title, creator) = match meta_node {
        Some(m) => (
            meta_field(m, "title"),
            meta_field(m, "creator").or_else(|| meta_field(m, "initial-creator")),
        ),
        None => (None, None),
    };

    Ok(Doc {
        title,
        creator,
        content: out,
        paragraphs: renderer.paragraphs,
        tables: renderer.tables,
        images: renderer.images,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    const NS: &str = concat!(
        r#" xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0""#,
        r#" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0""#,
        r#" xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0""#,
        r#" xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0""#,
        r#" xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0""#,
        r#" xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0""#,
        r#" xmlns:xlink="http://www.w3.org/1999/xlink""#,
    );

    /// Wrap a body fragment in a content.xml, with the standard automatic styles.
    fn content_xml(body: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-content{NS}>
  <office:automatic-styles>
    <style:style style:name="T1" style:family="text"><style:text-properties fo:font-weight="bold"/></style:style>
    <style:style style:name="T2" style:family="text"><style:text-properties fo:font-style="italic"/></style:style>
    <style:style style:name="T3" style:family="text" style:parent-style-name="T1"/>
    <text:list-style style:name="LNum"><text:list-level-style-number text:level="1"/></text:list-style>
    <text:list-style style:name="LBul"><text:list-level-style-bullet text:level="1"/></text:list-style>
  </office:automatic-styles>
  <office:body><office:text>{body}</office:text></office:body>
</office:document-content>"#
        )
    }

    fn meta_xml(title: &str, creator: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-meta xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:meta="urn:oasis:names:tc:opendocument:xmlns:meta:1.0">
  <office:meta><dc:title>{title}</dc:title><dc:creator>{creator}</dc:creator></office:meta>
</office:document-meta>"#
        )
    }

    /// Build a minimal but valid .odt in memory.
    fn build_odt(body: &str, meta: Option<&str>) -> Vec<u8> {
        use zip::write::SimpleFileOptions;
        let mut buf = Vec::new();
        {
            let mut w = zip::ZipWriter::new(Cursor::new(&mut buf));
            let opts = SimpleFileOptions::default();
            let mut add = |name: &str, data: &str| {
                w.start_file(name, opts).unwrap();
                w.write_all(data.as_bytes()).unwrap();
            };
            add("mimetype", "application/vnd.oasis.opendocument.text");
            add("content.xml", &content_xml(body));
            if let Some(m) = meta {
                add("meta.xml", m);
            }
            w.finish().unwrap();
        }
        buf
    }

    #[test]
    fn converts_a_minimal_odt() {
        let body = concat!(
            r#"<text:h text:outline-level="1">Report</text:h>"#,
            r#"<text:p>Hello <text:span text:style-name="T1">world</text:span>.</text:p>"#,
            r#"<text:h text:outline-level="2">Details</text:h>"#,
            r#"<text:p>Second <text:span text:style-name="T2">paragraph</text:span>.</text:p>"#,
        );
        let odt = build_odt(body, Some(&meta_xml("Quarterly Report", "Ada Lovelace")));
        let doc = convert(&odt, Mode::Markdown).unwrap();

        assert_eq!(doc.title.as_deref(), Some("Quarterly Report"));
        assert_eq!(doc.creator.as_deref(), Some("Ada Lovelace"));
        assert_eq!(doc.paragraphs, 4);
        assert_eq!(
            doc.content,
            "# Report\n\nHello **world**.\n\n## Details\n\nSecond *paragraph*."
        );
    }

    #[test]
    fn plain_text_mode_drops_markup() {
        let body = concat!(
            r#"<text:h text:outline-level="1">Report</text:h>"#,
            r#"<text:p>Hello <text:span text:style-name="T1">world</text:span>.</text:p>"#,
        );
        let odt = build_odt(body, None);
        let doc = convert(&odt, Mode::Text).unwrap();
        assert_eq!(doc.content, "Report\n\nHello world.");
        assert!(doc.title.is_none());
    }

    #[test]
    fn inherits_bold_through_parent_style() {
        let body = r#"<text:p><text:span text:style-name="T3">inherited</text:span></text:p>"#;
        let doc = convert(&build_odt(body, None), Mode::Markdown).unwrap();
        assert_eq!(doc.content, "**inherited**");
    }

    #[test]
    fn renders_lists_ordered_unordered_and_nested() {
        let body = concat!(
            r#"<text:list text:style-name="LBul">"#,
            r#"<text:list-item><text:p>first</text:p></text:list-item>"#,
            r#"<text:list-item><text:p>second</text:p>"#,
            r#"<text:list text:style-name="LNum">"#,
            r#"<text:list-item><text:p>inner a</text:p></text:list-item>"#,
            r#"<text:list-item><text:p>inner b</text:p></text:list-item>"#,
            r#"</text:list>"#,
            r#"</text:list-item></text:list>"#,
        );
        let doc = convert(&build_odt(body, None), Mode::Markdown).unwrap();
        assert_eq!(doc.content, "- first\n- second\n  1. inner a\n  2. inner b");
    }

    #[test]
    fn nested_list_inherits_numbering_when_style_is_absent() {
        let body = concat!(
            r#"<text:list text:style-name="LNum">"#,
            r#"<text:list-item><text:p>one</text:p>"#,
            r#"<text:list><text:list-item><text:p>one-a</text:p></text:list-item></text:list>"#,
            r#"</text:list-item></text:list>"#,
        );
        let doc = convert(&build_odt(body, None), Mode::Markdown).unwrap();
        assert_eq!(doc.content, "1. one\n   1. one-a");
    }

    #[test]
    fn renders_a_table_as_gfm() {
        let body = concat!(
            r#"<table:table table:name="T">"#,
            r#"<table:table-column table:number-columns-repeated="2"/>"#,
            r#"<table:table-row><table:table-cell><text:p>Name</text:p></table:table-cell>"#,
            r#"<table:table-cell><text:p>Qty</text:p></table:table-cell></table:table-row>"#,
            r#"<table:table-row><table:table-cell><text:p>Bolt</text:p></table:table-cell>"#,
            r#"<table:table-cell><text:p>12</text:p></table:table-cell></table:table-row>"#,
            r#"<table:table-row><table:table-cell table:number-columns-repeated="1024"/></table:table-row>"#,
            r#"</table:table>"#,
        );
        let doc = convert(&build_odt(body, None), Mode::Markdown).unwrap();
        assert_eq!(doc.tables, 1);
        assert_eq!(doc.content, "| Name | Qty |\n| --- | --- |\n| Bolt | 12 |");

        let plain = convert(&build_odt(body, None), Mode::Text).unwrap();
        assert_eq!(plain.content, "Name | Qty\nBolt | 12");
    }

    #[test]
    fn renders_hyperlinks_and_blocks_unsafe_schemes() {
        let body = concat!(
            r#"<text:p>See <text:a xlink:href="https://example.com/a(b)">the docs</text:a>.</text:p>"#,
            r#"<text:p><text:a xlink:href="javascript:alert(1)">click</text:a></text:p>"#,
        );
        let doc = convert(&build_odt(body, None), Mode::Markdown).unwrap();
        assert_eq!(
            doc.content,
            "See [the docs](<https://example.com/a(b)>).\n\nclick"
        );

        let plain = convert(&build_odt(body, None), Mode::Text).unwrap();
        assert_eq!(plain.content, "See the docs.\n\nclick");
    }

    #[test]
    fn renders_images_line_breaks_tabs_and_spaces() {
        let body = concat!(
            r#"<text:p>a<text:line-break/>b<text:s text:c="3"/>c</text:p>"#,
            r#"<text:p><draw:frame draw:name="Diagram"><draw:image xlink:href="Pictures/1.png"/></draw:frame></text:p>"#,
        );
        let doc = convert(&build_odt(body, None), Mode::Markdown).unwrap();
        assert_eq!(doc.images, 1);
        assert_eq!(doc.content, "a  \nb   c\n\n![Diagram](Pictures/1.png)");

        let plain = convert(&build_odt(body, None), Mode::Text).unwrap();
        assert_eq!(plain.content, "a\nb   c");
    }

    #[test]
    fn collects_footnotes_at_the_end() {
        let body = concat!(
            r#"<text:p>Claim<text:note text:note-class="footnote">"#,
            r#"<text:note-citation>1</text:note-citation>"#,
            r#"<text:note-body><text:p>The source.</text:p></text:note-body>"#,
            r#"</text:note> stands.</text:p>"#,
        );
        let doc = convert(&build_odt(body, None), Mode::Markdown).unwrap();
        assert_eq!(doc.content, "Claim[^1] stands.\n\n[^1]: The source.");

        let plain = convert(&build_odt(body, None), Mode::Text).unwrap();
        assert_eq!(plain.content, "Claim[1] stands.\n\n[1] The source.");
    }

    #[test]
    fn escapes_markdown_metacharacters_and_skips_comments() {
        let body = concat!(
            r#"<text:p>2 * 3 &lt; 7 and a_b [x]</text:p>"#,
            r#"<text:p># not a heading</text:p>"#,
            r#"<text:p>1. not a list</text:p>"#,
            r#"<office:annotation><text:p>private note</text:p></office:annotation>"#,
        );
        let doc = convert(&build_odt(body, None), Mode::Markdown).unwrap();
        assert_eq!(
            doc.content,
            "2 \\* 3 \\< 7 and a\\_b \\[x\\]\n\n\\# not a heading\n\n1\\. not a list"
        );
        assert!(!doc.content.contains("private note"));
    }

    #[test]
    fn reads_flat_opendocument_xml() {
        let flat = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document{NS} xmlns:dc="http://purl.org/dc/elements/1.1/">
  <office:meta><dc:title>Flat Doc</dc:title></office:meta>
  <office:body><office:text><text:h text:outline-level="1">Hi</text:h></office:text></office:body>
</office:document>"#
        );
        let doc = convert(flat.as_bytes(), Mode::Markdown).unwrap();
        assert_eq!(doc.title.as_deref(), Some("Flat Doc"));
        assert_eq!(doc.content, "# Hi");
    }

    #[test]
    fn falls_back_to_the_style_name_for_the_heading_level() {
        let body = r#"<text:h text:style-name="Heading_20_3">Deep</text:h>"#;
        let doc = convert(&build_odt(body, None), Mode::Markdown).unwrap();
        assert_eq!(doc.content, "### Deep");
    }

    #[test]
    fn errors() {
        assert!(convert(b"", Mode::Markdown).is_err());
        assert!(convert(b"not a zip and not xml", Mode::Markdown).is_err());
        assert!(Mode::parse("pdf").is_err());

        // A ZIP without content.xml.
        let mut buf = Vec::new();
        {
            let mut w = zip::ZipWriter::new(Cursor::new(&mut buf));
            w.start_file("hello.txt", zip::write::SimpleFileOptions::default())
                .unwrap();
            w.write_all(b"hi").unwrap();
            w.finish().unwrap();
        }
        let err = convert(&buf, Mode::Markdown).unwrap_err();
        assert!(err.contains("content.xml"), "unexpected error: {err}");

        // A spreadsheet body is not a text document.
        let ods = {
            let mut buf = Vec::new();
            let mut w = zip::ZipWriter::new(Cursor::new(&mut buf));
            w.start_file("content.xml", zip::write::SimpleFileOptions::default())
                .unwrap();
            w.write_all(
                format!(
                    r#"<office:document-content{NS}><office:body><office:spreadsheet/></office:body></office:document-content>"#
                )
                .as_bytes(),
            )
            .unwrap();
            w.finish().unwrap();
            buf
        };
        let err = convert(&ods, Mode::Markdown).unwrap_err();
        assert!(err.contains("text* document"), "unexpected error: {err}");

        // A valid but empty text body.
        let empty = build_odt("<text:p></text:p>", None);
        assert!(convert(&empty, Mode::Markdown).is_err());
    }

    #[test]
    fn mode_parse_accepts_aliases() {
        assert_eq!(Mode::parse("md").unwrap(), Mode::Markdown);
        assert_eq!(Mode::parse("MARKDOWN").unwrap(), Mode::Markdown);
        assert_eq!(Mode::parse("txt").unwrap(), Mode::Text);
        assert_eq!(Mode::parse(" plain ").unwrap(), Mode::Text);
    }
}
