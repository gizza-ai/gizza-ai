//! xml-structure-analyzer core — pure compute, shared by the chat skill block and the web page.
//!
//! Parses an XML document in ONE streaming pass (quick-xml pull parser, no DOM) and reports its
//! *shape* without transforming it: the collapsed element tree, per-tag counts, nesting depth
//! (max, average, and a per-level histogram), attribute usage, namespace declarations, node-type
//! tallies, the XML declaration/DOCTYPE, size, and structural warnings.
//!
//! Element paths are collapsed **by element name**, the XML analogue of the sibling
//! json-structure-analyzer's `[]` index-collapse: every `<book>` under `<catalog>` shares the
//! path `catalog/book`, so a 10 000-entry feed still summarizes to a readable tree.
//!
//! No wafer/wasm-bindgen deps — instantiates under both wasm32-wasip1 and wasm32-unknown-unknown.

use quick_xml::events::Event;
use quick_xml::reader::Reader;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

/// Output shape selector.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Format {
    /// Human-readable report with a box-drawing element tree (default).
    Text,
    /// Structured JSON report.
    Json,
    /// One CSV row per distinct element name (spreadsheet triage).
    Csv,
}

pub struct Options {
    pub format: Format,
    /// Max levels of the element tree to render (0 = the full tree).
    pub tree_depth: usize,
    /// Max rows in the per-tag count table (0 = every tag).
    pub top_tags: usize,
    /// Annotate the tree with attribute names and emit the attribute-usage table.
    pub show_attributes: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            format: Format::Text,
            tree_depth: 0,
            top_tags: 50,
            show_attributes: true,
        }
    }
}

/// Nesting past this many levels raises a "deep nesting" warning. XML nests more naturally than
/// JSON (a SOAP envelope is already 3–4 levels), so the threshold is looser than the JSON
/// analyzer's 5.
const DEEP_NESTING: u64 = 10;

/// Per attribute name, distinct values are counted exactly up to this many; beyond it the row
/// reports the cap and sets `unique_values_capped`, so an id-like attribute in a huge document
/// can't grow the tracking set without bound.
const MAX_TRACKED_VALUES: usize = 2000;

// ---------------------------------------------------------------------------------------------
// Report shape
// ---------------------------------------------------------------------------------------------

#[derive(Serialize, Default)]
struct Counts {
    /// Every element occurrence, self-closing elements included.
    elements: u64,
    /// Distinct element names (prefixes kept as written).
    unique_tags: u64,
    /// Attribute occurrences, EXCLUDING `xmlns`/`xmlns:*` namespace declarations.
    attributes: u64,
    /// Distinct attribute names (namespace declarations excluded).
    unique_attributes: u64,
    /// `xmlns` / `xmlns:prefix` declarations encountered (reported under `namespaces`).
    namespace_declarations: u64,
    /// Text nodes with at least one non-whitespace character.
    text_nodes: u64,
    comments: u64,
    cdata_sections: u64,
    /// `<?…?>` processing instructions (the `<?xml …?>` declaration is not one).
    processing_instructions: u64,
    /// Elements with neither child elements nor text content.
    empty_elements: u64,
    /// Elements holding BOTH text and child elements (mixed content).
    mixed_content_elements: u64,
    /// Most direct element children any single element has.
    max_children: u64,
}

#[derive(Serialize)]
struct SizeInfo {
    /// Byte length of the document exactly as supplied.
    bytes: usize,
    /// Line count (1 for a single-line document).
    lines: usize,
}

#[derive(Serialize)]
struct Declaration {
    version: String,
    encoding: Option<String>,
    standalone: Option<String>,
}

#[derive(Serialize)]
struct DepthLevel {
    /// 1 = the root element.
    depth: u64,
    elements: u64,
}

#[derive(Serialize, Clone)]
struct AttrCount {
    name: String,
    count: u64,
}

#[derive(Serialize)]
struct TagStat {
    tag: String,
    count: u64,
    min_depth: u64,
    max_depth: u64,
    /// Most direct element children seen on any occurrence of this tag.
    max_children: u64,
    /// Occurrences carrying non-whitespace text content.
    text_nodes: u64,
    /// Occurrences with neither child elements nor text.
    empty: u64,
    /// Attribute names seen on this tag with their occurrence counts (empty when
    /// `show_attributes` is off).
    attributes: Vec<AttrCount>,
}

#[derive(Serialize)]
struct AttrUsage {
    attribute: String,
    /// Total occurrences across the document.
    count: u64,
    /// Element names carrying this attribute, sorted.
    elements: Vec<String>,
    /// Distinct values seen (exact up to 2000 per attribute — see `unique_values_capped`).
    unique_values: u64,
    unique_values_capped: bool,
}

#[derive(Serialize)]
struct Namespace {
    /// The declared prefix; empty string for the default `xmlns="…"` declaration.
    prefix: String,
    uri: String,
    /// True when at least one element or attribute name actually uses this prefix.
    used: bool,
}

#[derive(Serialize)]
struct TreeNode {
    tag: String,
    /// Slash-joined path of element names from the root, e.g. `catalog/book/title`.
    path: String,
    /// How many elements in the document sit at this path.
    count: u64,
    /// Attribute names seen at this path (empty when `show_attributes` is off).
    attributes: Vec<AttrCount>,
    children: Vec<TreeNode>,
    /// True when this node's children were omitted by the `tree_depth` cap.
    children_truncated: bool,
}

#[derive(Serialize)]
struct Report {
    well_formed: bool,
    /// Name of the root element, prefix included as written.
    root: String,
    /// Deepest nesting level; the root element is depth 1.
    max_depth: u64,
    /// Mean depth across every element node, rounded to 2 decimals.
    avg_depth: f64,
    size: SizeInfo,
    declaration: Option<Declaration>,
    doctype: Option<String>,
    counts: Counts,
    depth_histogram: Vec<DepthLevel>,
    /// Collapsed element tree (one node per distinct element path).
    tree: Vec<TreeNode>,
    /// True when `tree_depth` cut the tree somewhere.
    tree_truncated: bool,
    /// Distinct element names ranked by occurrence count.
    tags: Vec<TagStat>,
    tags_truncated: bool,
    /// Distinct attribute names ranked by occurrence count (empty when `show_attributes` is off).
    attribute_usage: Vec<AttrUsage>,
    namespaces: Vec<Namespace>,
    warnings: Vec<String>,
}

// ---------------------------------------------------------------------------------------------
// Walk
// ---------------------------------------------------------------------------------------------

/// One arena node of the collapsed element tree: a distinct element PATH, not an occurrence.
struct RawNode {
    tag: String,
    count: u64,
    attrs: BTreeMap<String, u64>,
    /// Arena indices of child paths, in first-seen document order.
    children: Vec<usize>,
    /// Child tag name -> arena index, so a repeated sibling merges into one node.
    index: BTreeMap<String, usize>,
}

/// One open element while walking (a live occurrence, not a path).
struct Frame {
    tag: String,
    node: usize,
    children: u64,
    has_text: bool,
}

#[derive(Default)]
struct TagAccum {
    count: u64,
    min_depth: u64,
    max_depth: u64,
    max_children: u64,
    text_nodes: u64,
    empty: u64,
    attrs: BTreeMap<String, u64>,
}

#[derive(Default)]
struct AttrAccum {
    count: u64,
    elements: BTreeSet<String>,
    values: BTreeSet<String>,
    values_capped: bool,
}

struct Walk {
    arena: Vec<RawNode>,
    roots: Vec<usize>,
    root_index: BTreeMap<String, usize>,
    stack: Vec<Frame>,
    counts: Counts,
    max_depth: u64,
    depth_sum: u64,
    depth_hist: BTreeMap<u64, u64>,
    tags: BTreeMap<String, TagAccum>,
    attrs: BTreeMap<String, AttrAccum>,
    /// (prefix, uri) of every distinct namespace declaration, in sorted order.
    namespaces: BTreeSet<(String, String)>,
    used_prefixes: BTreeSet<String>,
    declaration: Option<Declaration>,
    doctype: Option<String>,
    root_name: Option<String>,
}

/// Find-or-create the tree node named `tag` under `parent` (None = document root level).
fn node_for(
    arena: &mut Vec<RawNode>,
    roots: &mut Vec<usize>,
    root_index: &mut BTreeMap<String, usize>,
    parent: Option<usize>,
    tag: &str,
) -> usize {
    // An existing sibling with the same name merges into one path node.
    let existing = match parent {
        Some(p) => arena[p].index.get(tag).copied(),
        None => root_index.get(tag).copied(),
    };
    if let Some(idx) = existing {
        return idx;
    }
    let idx = arena.len();
    arena.push(RawNode {
        tag: tag.to_string(),
        count: 0,
        attrs: BTreeMap::new(),
        children: Vec::new(),
        index: BTreeMap::new(),
    });
    match parent {
        Some(p) => {
            arena[p].children.push(idx);
            arena[p].index.insert(tag.to_string(), idx);
        }
        None => {
            roots.push(idx);
            root_index.insert(tag.to_string(), idx);
        }
    }
    idx
}

/// 1-based line and column of `byte_pos` within `input`. Takes `u64` because that is what
/// quick-xml's `Reader::buffer_position()` reports.
fn line_col(input: &str, byte_pos: u64) -> (usize, usize) {
    let mut p = (byte_pos as usize).min(input.len());
    while p > 0 && !input.is_char_boundary(p) {
        p -= 1;
    }
    let before = &input[..p];
    let line = before.matches('\n').count() + 1;
    let col = before
        .rsplit('\n')
        .next()
        .map(|l| l.chars().count())
        .unwrap_or(0)
        + 1;
    (line, col)
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn qname(raw: &[u8]) -> String {
    String::from_utf8_lossy(raw).into_owned()
}

/// Prefix part of a qualified name (`soap:Body` -> `soap`), or None when unprefixed.
fn prefix_of(name: &str) -> Option<String> {
    name.split_once(':').map(|(p, _)| p.to_string())
}

impl Walk {
    fn new() -> Self {
        Walk {
            arena: Vec::new(),
            roots: Vec::new(),
            root_index: BTreeMap::new(),
            stack: Vec::new(),
            counts: Counts::default(),
            max_depth: 0,
            depth_sum: 0,
            depth_hist: BTreeMap::new(),
            tags: BTreeMap::new(),
            attrs: BTreeMap::new(),
            namespaces: BTreeSet::new(),
            used_prefixes: BTreeSet::new(),
            declaration: None,
            doctype: None,
            root_name: None,
        }
    }

    /// Open an element: register its path node, depth, name stats and attributes.
    /// Returns the arena index of its path node plus its attribute count.
    fn open(&mut self, tag: &str, attrs: Vec<(String, String)>) -> usize {
        let parent = self.stack.last().map(|f| f.node);
        let node = node_for(
            &mut self.arena,
            &mut self.roots,
            &mut self.root_index,
            parent,
            tag,
        );
        self.arena[node].count += 1;

        let depth = self.stack.len() as u64 + 1;
        self.counts.elements += 1;
        self.depth_sum += depth;
        *self.depth_hist.entry(depth).or_insert(0) += 1;
        if depth > self.max_depth {
            self.max_depth = depth;
        }
        if depth == 1 && self.root_name.is_none() {
            self.root_name = Some(tag.to_string());
        }
        if let Some(p) = prefix_of(tag) {
            self.used_prefixes.insert(p);
        }

        let t = self
            .tags
            .entry(tag.to_string())
            .or_insert_with(|| TagAccum {
                min_depth: depth,
                ..Default::default()
            });
        t.count += 1;
        t.min_depth = t.min_depth.min(depth);
        t.max_depth = t.max_depth.max(depth);

        for (name, value) in attrs {
            if name == "xmlns" || name.starts_with("xmlns:") {
                self.counts.namespace_declarations += 1;
                let prefix = name.strip_prefix("xmlns:").unwrap_or("").to_string();
                self.namespaces.insert((prefix, value));
                continue;
            }
            self.counts.attributes += 1;
            if let Some(p) = prefix_of(&name) {
                self.used_prefixes.insert(p);
            }
            *self.arena[node].attrs.entry(name.clone()).or_insert(0) += 1;
            *self
                .tags
                .get_mut(tag)
                .expect("tag accumulator inserted above")
                .attrs
                .entry(name.clone())
                .or_insert(0) += 1;
            let a = self.attrs.entry(name).or_default();
            a.count += 1;
            a.elements.insert(tag.to_string());
            if a.values.len() < MAX_TRACKED_VALUES {
                a.values.insert(value);
            } else {
                a.values_capped = true;
            }
        }

        // The parent gains a child element; mixed content is decided when the parent closes.
        if let Some(f) = self.stack.last_mut() {
            f.children += 1;
        }
        node
    }

    /// Close an element occurrence, folding its per-occurrence facts into the tag stats.
    fn close(&mut self, frame: Frame) {
        self.counts.max_children = self.counts.max_children.max(frame.children);
        if frame.children == 0 && !frame.has_text {
            self.counts.empty_elements += 1;
        }
        if frame.children > 0 && frame.has_text {
            self.counts.mixed_content_elements += 1;
        }
        if let Some(t) = self.tags.get_mut(&frame.tag) {
            t.max_children = t.max_children.max(frame.children);
            if frame.has_text {
                t.text_nodes += 1;
            }
            if frame.children == 0 && !frame.has_text {
                t.empty += 1;
            }
        }
    }
}

/// Decode an element's attributes into `(name, value)` pairs.
fn read_attrs(e: &quick_xml::events::BytesStart<'_>) -> Result<Vec<(String, String)>, String> {
    let mut out = Vec::new();
    for a in e.attributes() {
        let a = a.map_err(|err| format!("invalid attribute: {err}"))?;
        let name = qname(a.key.as_ref());
        let value = a
            .unescape_value()
            .map_err(|err| format!("invalid attribute value on '{name}': {err}"))?
            .into_owned();
        out.push((name, value));
    }
    Ok(out)
}

/// One streaming pass over `xml`.
fn walk(xml: &str) -> Result<Walk, String> {
    let mut reader = Reader::from_str(xml);
    // Keep whitespace: mixed content vs. pretty-printing indentation is decided here, not by the
    // parser, so whitespace-only text must stay distinguishable from real text content.
    reader.config_mut().trim_text(false);
    let mut w = Walk::new();

    loop {
        let pos = reader.buffer_position();
        let ev = reader.read_event().map_err(|e| {
            let (line, col) = line_col(xml, pos);
            format!("not well-formed XML at line {line}, column {col}: {e}")
        })?;
        match ev {
            Event::Eof => break,
            Event::Start(e) => {
                let tag = qname(e.name().as_ref());
                let node = w.open(&tag, read_attrs(&e)?);
                w.stack.push(Frame {
                    tag,
                    node,
                    children: 0,
                    has_text: false,
                });
            }
            Event::Empty(e) => {
                // Self-closing: opens and closes in one event.
                let tag = qname(e.name().as_ref());
                let node = w.open(&tag, read_attrs(&e)?);
                w.close(Frame {
                    tag,
                    node,
                    children: 0,
                    has_text: false,
                });
            }
            Event::End(_) => match w.stack.pop() {
                Some(frame) => w.close(frame),
                // quick-xml's end-name check rejects a mismatch, so a pop failure means a stray
                // close tag with nothing open.
                None => {
                    let (line, col) = line_col(xml, pos);
                    return Err(format!(
                        "not well-formed XML at line {line}, column {col}: closing tag with no \
                         matching opening tag"
                    ));
                }
            },
            Event::Text(t) => {
                let raw = String::from_utf8_lossy(t.as_ref());
                if !raw.trim().is_empty() && !w.stack.is_empty() {
                    w.counts.text_nodes += 1;
                    if let Some(f) = w.stack.last_mut() {
                        f.has_text = true;
                    }
                }
            }
            Event::CData(t) => {
                w.counts.cdata_sections += 1;
                if !String::from_utf8_lossy(t.as_ref()).trim().is_empty() {
                    w.counts.text_nodes += 1;
                    if let Some(f) = w.stack.last_mut() {
                        f.has_text = true;
                    }
                }
            }
            Event::Comment(_) => w.counts.comments += 1,
            Event::PI(_) => w.counts.processing_instructions += 1,
            Event::Decl(d) => {
                let version = d
                    .version()
                    .map(|v| String::from_utf8_lossy(&v).into_owned())
                    .unwrap_or_else(|_| "1.0".to_string());
                let encoding = d
                    .encoding()
                    .and_then(|r| r.ok())
                    .map(|v| String::from_utf8_lossy(&v).into_owned());
                let standalone = d
                    .standalone()
                    .and_then(|r| r.ok())
                    .map(|v| String::from_utf8_lossy(&v).into_owned());
                w.declaration = Some(Declaration {
                    version,
                    encoding,
                    standalone,
                });
            }
            Event::DocType(d) => {
                w.doctype = Some(String::from_utf8_lossy(d.as_ref()).trim().to_string());
            }
        }
    }

    if let Some(open) = w.stack.last() {
        return Err(format!(
            "not well-formed XML: element <{}> is never closed ({} element(s) left open at \
             end of document)",
            open.tag,
            w.stack.len()
        ));
    }
    Ok(w)
}

// ---------------------------------------------------------------------------------------------
// Report assembly
// ---------------------------------------------------------------------------------------------

/// Turn the arena into serializable `TreeNode`s, honoring the `tree_depth` cap.
/// `level` is 1-based (the root element is level 1).
fn build_tree(
    arena: &[RawNode],
    ids: &[usize],
    parent_path: &str,
    level: usize,
    opts: &Options,
    truncated: &mut bool,
) -> Vec<TreeNode> {
    ids.iter()
        .map(|&id| {
            let n = &arena[id];
            let path = if parent_path.is_empty() {
                n.tag.clone()
            } else {
                format!("{parent_path}/{}", n.tag)
            };
            let at_cap = opts.tree_depth != 0 && level >= opts.tree_depth;
            let children_truncated = at_cap && !n.children.is_empty();
            if children_truncated {
                *truncated = true;
            }
            let children = if at_cap {
                Vec::new()
            } else {
                build_tree(arena, &n.children, &path, level + 1, opts, truncated)
            };
            TreeNode {
                tag: n.tag.clone(),
                path,
                count: n.count,
                attributes: if opts.show_attributes {
                    attr_counts(&n.attrs)
                } else {
                    Vec::new()
                },
                children,
                children_truncated,
            }
        })
        .collect()
}

/// Attribute map -> list sorted by count desc, then name asc (stable output).
fn attr_counts(m: &BTreeMap<String, u64>) -> Vec<AttrCount> {
    let mut v: Vec<AttrCount> = m
        .iter()
        .map(|(name, &count)| AttrCount {
            name: name.clone(),
            count,
        })
        .collect();
    v.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.name.cmp(&b.name)));
    v
}

fn build_report(input: &str, opts: &Options) -> Result<Report, String> {
    if input.trim().is_empty() {
        return Err("no XML provided — paste an XML document to analyze".into());
    }
    let w = walk(input)?;

    let root =
        match &w.root_name {
            Some(r) => r.clone(),
            None => return Err(
                "no elements found — an XML document needs a root element (only a declaration, \
                 comment, or text was found)"
                    .into(),
            ),
        };

    let mut counts = w.counts;
    counts.unique_tags = w.tags.len() as u64;
    counts.unique_attributes = w.attrs.len() as u64;

    let avg_depth = if counts.elements == 0 {
        0.0
    } else {
        round2(w.depth_sum as f64 / counts.elements as f64)
    };

    let depth_histogram: Vec<DepthLevel> = w
        .depth_hist
        .iter()
        .map(|(&depth, &elements)| DepthLevel { depth, elements })
        .collect();

    let mut tree_truncated = false;
    let tree = build_tree(&w.arena, &w.roots, "", 1, opts, &mut tree_truncated);

    // Tags: count desc, then name asc.
    let mut tags: Vec<TagStat> = w
        .tags
        .iter()
        .map(|(tag, t)| TagStat {
            tag: tag.clone(),
            count: t.count,
            min_depth: t.min_depth,
            max_depth: t.max_depth,
            max_children: t.max_children,
            text_nodes: t.text_nodes,
            empty: t.empty,
            attributes: if opts.show_attributes {
                attr_counts(&t.attrs)
            } else {
                Vec::new()
            },
        })
        .collect();
    tags.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.tag.cmp(&b.tag)));
    let tags_truncated = opts.top_tags != 0 && tags.len() > opts.top_tags;
    if opts.top_tags != 0 {
        tags.truncate(opts.top_tags);
    }

    // Attribute usage: count desc, then name asc.
    let mut attribute_usage: Vec<AttrUsage> = if opts.show_attributes {
        w.attrs
            .iter()
            .map(|(attribute, a)| AttrUsage {
                attribute: attribute.clone(),
                count: a.count,
                elements: a.elements.iter().cloned().collect(),
                unique_values: a.values.len() as u64,
                unique_values_capped: a.values_capped,
            })
            .collect()
    } else {
        Vec::new()
    };
    attribute_usage.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then_with(|| a.attribute.cmp(&b.attribute))
    });

    let namespaces: Vec<Namespace> = w
        .namespaces
        .iter()
        .map(|(prefix, uri)| Namespace {
            prefix: prefix.clone(),
            uri: uri.clone(),
            // The default namespace applies to every unprefixed element, so it is always in use
            // once declared; a prefixed declaration is only "used" if a name references it.
            used: prefix.is_empty() || w.used_prefixes.contains(prefix),
        })
        .collect();

    let size = SizeInfo {
        bytes: input.len(),
        lines: input.lines().count().max(1),
    };

    // Warnings — the "structural health" section competitors surface.
    let mut warnings = Vec::new();
    if w.max_depth > DEEP_NESTING {
        warnings.push(format!(
            "Deep nesting: {} levels (> {} is hard to read and query)",
            w.max_depth, DEEP_NESTING
        ));
    }
    if counts.mixed_content_elements > 0 {
        warnings.push(format!(
            "{} element(s) hold both text and child elements (mixed content) — text can be lost \
             by naive XML-to-JSON/CSV conversion",
            counts.mixed_content_elements
        ));
    }
    if counts.empty_elements > 0 {
        warnings.push(format!(
            "{} element(s) are empty (no child elements, no text)",
            counts.empty_elements
        ));
    }
    let unused: Vec<&str> = namespaces
        .iter()
        .filter(|n| !n.used)
        .map(|n| n.prefix.as_str())
        .collect();
    if !unused.is_empty() {
        warnings.push(format!(
            "Namespace prefix(es) declared but never used: {}",
            unused.join(", ")
        ));
    }
    if w.roots.len() > 1 {
        warnings.push(format!(
            "{} top-level elements found — a well-formed XML document has exactly one root",
            w.roots.len()
        ));
    }
    if tags_truncated {
        warnings.push(format!(
            "Tag table truncated to the {} most common of {} distinct tags (raise or zero \
             top_tags to see all)",
            opts.top_tags,
            w.tags.len()
        ));
    }
    if tree_truncated {
        warnings.push(format!(
            "Element tree cut at {} level(s) deep (set tree_depth to 0 for the full tree)",
            opts.tree_depth
        ));
    }
    if attribute_usage.iter().any(|a| a.unique_values_capped) {
        warnings.push(format!(
            "Distinct-value counts are capped at {MAX_TRACKED_VALUES} per attribute; capped rows \
             are flagged"
        ));
    }

    Ok(Report {
        well_formed: true,
        root,
        max_depth: w.max_depth,
        avg_depth,
        size,
        declaration: w.declaration,
        doctype: w.doctype,
        counts,
        depth_histogram,
        tree,
        tree_truncated,
        tags,
        tags_truncated,
        attribute_usage,
        namespaces,
        warnings,
    })
}

// ---------------------------------------------------------------------------------------------
// Renderers
// ---------------------------------------------------------------------------------------------

/// Render one tree level with box-drawing connectors, depth-first.
fn render_tree(out: &mut String, nodes: &[TreeNode], prefix: &str, top: bool) {
    for (i, n) in nodes.iter().enumerate() {
        let last = i + 1 == nodes.len();
        let (branch, carry) = if top {
            ("", "")
        } else if last {
            ("└─ ", "   ")
        } else {
            ("├─ ", "│  ")
        };
        let attrs = if n.attributes.is_empty() {
            String::new()
        } else {
            let names: Vec<String> = n.attributes.iter().map(|a| a.name.clone()).collect();
            format!("  [{}]", names.join(" "))
        };
        let more = if n.children_truncated { "  …" } else { "" };
        out.push_str(&format!(
            "{prefix}{branch}{} ({}){attrs}{more}\n",
            n.tag, n.count
        ));
        if !n.children.is_empty() {
            render_tree(out, &n.children, &format!("{prefix}{carry}"), false);
        }
    }
}

fn render_text(r: &Report) -> String {
    let mut out = String::new();
    out.push_str("XML Structure Analysis\n");
    out.push_str("======================\n");
    out.push_str(&format!("Root element:  {}\n", r.root));
    out.push_str(&format!("Max depth:     {}\n", r.max_depth));
    out.push_str(&format!("Avg depth:     {}\n", r.avg_depth));
    out.push_str(&format!(
        "Size:          {} bytes, {} lines\n",
        r.size.bytes, r.size.lines
    ));
    match &r.declaration {
        Some(d) => {
            let enc = d
                .encoding
                .clone()
                .unwrap_or_else(|| "unspecified".to_string());
            let sa = d
                .standalone
                .clone()
                .unwrap_or_else(|| "unspecified".to_string());
            out.push_str(&format!(
                "Declaration:   version {}, encoding {enc}, standalone {sa}\n",
                d.version
            ));
        }
        None => out.push_str("Declaration:   (none)\n"),
    }
    if let Some(dt) = &r.doctype {
        out.push_str(&format!("Doctype:       {dt}\n"));
    }
    out.push('\n');

    out.push_str("Node counts\n");
    out.push_str(&format!("  elements:           {}\n", r.counts.elements));
    out.push_str(&format!("  unique tags:        {}\n", r.counts.unique_tags));
    out.push_str(&format!("  attributes:         {}\n", r.counts.attributes));
    out.push_str(&format!(
        "  unique attributes:  {}\n",
        r.counts.unique_attributes
    ));
    out.push_str(&format!(
        "  namespace decls:    {}\n",
        r.counts.namespace_declarations
    ));
    out.push_str(&format!("  text nodes:         {}\n", r.counts.text_nodes));
    out.push_str(&format!("  comments:           {}\n", r.counts.comments));
    out.push_str(&format!(
        "  CDATA sections:     {}\n",
        r.counts.cdata_sections
    ));
    out.push_str(&format!(
        "  processing instr.:  {}\n",
        r.counts.processing_instructions
    ));
    out.push_str(&format!(
        "  empty elements:     {}\n",
        r.counts.empty_elements
    ));
    out.push_str(&format!(
        "  mixed content:      {}\n",
        r.counts.mixed_content_elements
    ));
    out.push_str(&format!(
        "  max children:       {}\n",
        r.counts.max_children
    ));
    out.push('\n');

    out.push_str("Elements per depth\n");
    for d in &r.depth_histogram {
        out.push_str(&format!("  {:>3}  {}\n", d.depth, d.elements));
    }
    out.push('\n');

    out.push_str("Element tree\n");
    render_tree(&mut out, &r.tree, "", true);
    out.push('\n');

    out.push_str(&format!("Tag counts ({} shown)\n", r.tags.len()));
    out.push_str("   count  depth  children  text  empty  tag\n");
    for t in &r.tags {
        let depth = if t.min_depth == t.max_depth {
            format!("{}", t.min_depth)
        } else {
            format!("{}-{}", t.min_depth, t.max_depth)
        };
        out.push_str(&format!(
            "  {:>6}  {:>5}  {:>8}  {:>4}  {:>5}  {}\n",
            t.count, depth, t.max_children, t.text_nodes, t.empty, t.tag
        ));
    }
    if r.tags_truncated {
        out.push_str("  … (truncated)\n");
    }
    out.push('\n');

    out.push_str(&format!(
        "Attribute usage ({} shown)\n",
        r.attribute_usage.len()
    ));
    if r.attribute_usage.is_empty() {
        out.push_str("  (none)\n");
    } else {
        out.push_str("   count  values  attribute  →  on elements\n");
        for a in &r.attribute_usage {
            let values = if a.unique_values_capped {
                format!("{}+", a.unique_values)
            } else {
                a.unique_values.to_string()
            };
            out.push_str(&format!(
                "  {:>6}  {:>6}  {}  →  {}\n",
                a.count,
                values,
                a.attribute,
                a.elements.join(", ")
            ));
        }
    }
    out.push('\n');

    out.push_str(&format!("Namespaces ({})\n", r.namespaces.len()));
    if r.namespaces.is_empty() {
        out.push_str("  (none declared)\n");
    } else {
        for n in &r.namespaces {
            let prefix = if n.prefix.is_empty() {
                "(default)".to_string()
            } else {
                n.prefix.clone()
            };
            let used = if n.used { "" } else { "  (unused)" };
            out.push_str(&format!("  {prefix}  =  {}{used}\n", n.uri));
        }
    }

    if !r.warnings.is_empty() {
        out.push('\n');
        out.push_str("Warnings\n");
        for w in &r.warnings {
            out.push_str(&format!("  • {w}\n"));
        }
    }

    out
}

/// Quote a CSV field only when it needs it (comma, quote, CR/LF).
fn csv_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// CSV: one row per distinct element name. The `attributes` column holds `name=count` pairs
/// separated by `;` (blank when `show_attributes` is off).
fn render_csv(r: &Report) -> String {
    let mut out = String::new();
    out.push_str("tag,count,min_depth,max_depth,max_children,text_nodes,empty,attributes\n");
    for t in &r.tags {
        let attrs = t
            .attributes
            .iter()
            .map(|a| format!("{}={}", a.name, a.count))
            .collect::<Vec<_>>()
            .join(";");
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{}\n",
            csv_field(&t.tag),
            t.count,
            t.min_depth,
            t.max_depth,
            t.max_children,
            t.text_nodes,
            t.empty,
            csv_field(&attrs)
        ));
    }
    out
}

/// Analyze `input` (an XML document) and return a structural report per `opts`.
pub fn analyze(input: &str, opts: &Options) -> Result<String, String> {
    let report = build_report(input, opts)?;
    match opts.format {
        Format::Text => Ok(render_text(&report)),
        Format::Json => {
            serde_json::to_string_pretty(&report).map_err(|e| format!("serialization error: {e}"))
        }
        Format::Csv => Ok(render_csv(&report)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    const CATALOG: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<catalog>
  <book id="b1"><title>Dune</title><price currency="USD">9.99</price></book>
  <book id="b2"><title>Emma</title><price currency="EUR">7.50</price></book>
  <meta/>
</catalog>"#;

    fn json_report(input: &str) -> Value {
        let opts = Options {
            format: Format::Json,
            ..Default::default()
        };
        serde_json::from_str(&analyze(input, &opts).unwrap()).unwrap()
    }

    #[test]
    fn reports_root_depth_and_counts() {
        let r = json_report(CATALOG);
        assert_eq!(r["well_formed"], true);
        assert_eq!(r["root"], "catalog");
        // catalog(1) -> book(2) -> title/price(3)
        assert_eq!(r["max_depth"], 3);
        // 1 catalog + 2 book + 2 title + 2 price + 1 meta = 8
        assert_eq!(r["counts"]["elements"], 8);
        assert_eq!(r["counts"]["unique_tags"], 5);
        // id x2 + currency x2; xmlns declarations excluded (none here).
        assert_eq!(r["counts"]["attributes"], 4);
        assert_eq!(r["counts"]["unique_attributes"], 2);
        assert_eq!(r["counts"]["text_nodes"], 4);
        assert_eq!(r["counts"]["empty_elements"], 1); // <meta/>
        assert_eq!(r["counts"]["max_children"], 3); // catalog has book, book, meta
        assert_eq!(r["declaration"]["version"], "1.0");
        assert_eq!(r["declaration"]["encoding"], "UTF-8");
    }

    #[test]
    fn element_tree_collapses_repeated_siblings() {
        let r = json_report(CATALOG);
        let tree = r["tree"].as_array().unwrap();
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0]["tag"], "catalog");
        assert_eq!(tree[0]["path"], "catalog");
        let kids = tree[0]["children"].as_array().unwrap();
        // Two <book> elements collapse into ONE path node with count 2.
        let book = kids.iter().find(|c| c["tag"] == "book").unwrap();
        assert_eq!(book["count"], 2);
        assert_eq!(book["path"], "catalog/book");
        let title = book["children"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["tag"] == "title")
            .unwrap();
        assert_eq!(title["count"], 2);
        assert_eq!(title["path"], "catalog/book/title");
    }

    #[test]
    fn tag_counts_rank_by_frequency_with_depth_range() {
        let r = json_report(CATALOG);
        let tags = r["tags"].as_array().unwrap();
        let book = tags.iter().find(|t| t["tag"] == "book").unwrap();
        assert_eq!(book["count"], 2);
        assert_eq!(book["min_depth"], 2);
        assert_eq!(book["max_depth"], 2);
        assert_eq!(book["max_children"], 2);
        let title = tags.iter().find(|t| t["tag"] == "title").unwrap();
        assert_eq!(title["text_nodes"], 2);
        // Most frequent first (book/title/price all 2 → alphabetical tiebreak).
        assert_eq!(tags[0]["count"], 2);
    }

    #[test]
    fn attribute_usage_lists_owning_elements_and_value_counts() {
        let r = json_report(CATALOG);
        let au = r["attribute_usage"].as_array().unwrap();
        let currency = au.iter().find(|a| a["attribute"] == "currency").unwrap();
        assert_eq!(currency["count"], 2);
        assert_eq!(currency["elements"][0], "price");
        assert_eq!(currency["unique_values"], 2); // USD, EUR
        assert_eq!(currency["unique_values_capped"], false);
        let id = au.iter().find(|a| a["attribute"] == "id").unwrap();
        assert_eq!(id["elements"][0], "book");
    }

    #[test]
    fn depth_histogram_counts_each_level() {
        let r = json_report(CATALOG);
        let h = r["depth_histogram"].as_array().unwrap();
        assert_eq!(h[0]["depth"], 1);
        assert_eq!(h[0]["elements"], 1); // catalog
        assert_eq!(h[1]["depth"], 2);
        assert_eq!(h[1]["elements"], 3); // book, book, meta
        assert_eq!(h[2]["elements"], 4); // title x2, price x2
        assert_eq!(r["avg_depth"], 2.38); // (1 + 2*3 + 3*4) / 8 = 2.375
    }

    #[test]
    fn namespaces_report_prefixes_and_usage() {
        let xml = r#"<soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/"
                                   xmlns="urn:body" xmlns:unused="urn:nope">
                       <soap:Body><Ping/></soap:Body>
                     </soap:Envelope>"#;
        let r = json_report(xml);
        assert_eq!(r["root"], "soap:Envelope");
        assert_eq!(r["counts"]["namespace_declarations"], 3);
        // xmlns declarations are NOT counted as ordinary attributes.
        assert_eq!(r["counts"]["attributes"], 0);
        let ns = r["namespaces"].as_array().unwrap();
        let soap = ns.iter().find(|n| n["prefix"] == "soap").unwrap();
        assert_eq!(soap["used"], true);
        let unused = ns.iter().find(|n| n["prefix"] == "unused").unwrap();
        assert_eq!(unused["used"], false);
        let dflt = ns.iter().find(|n| n["prefix"] == "").unwrap();
        assert_eq!(dflt["uri"], "urn:body");
        let warns = r["warnings"].as_array().unwrap();
        assert!(warns
            .iter()
            .any(|w| w.as_str().unwrap().contains("never used")));
    }

    #[test]
    fn mixed_content_and_node_types_are_tallied() {
        let xml = "<doc><!-- note --><?php echo 1; ?><p>hi <b>there</b></p>\
                   <raw><![CDATA[<tag/>]]></raw></doc>";
        let r = json_report(xml);
        assert_eq!(r["counts"]["comments"], 1);
        assert_eq!(r["counts"]["processing_instructions"], 1);
        assert_eq!(r["counts"]["cdata_sections"], 1);
        assert_eq!(r["counts"]["mixed_content_elements"], 1); // <p> has text + <b>
        let warns = r["warnings"].as_array().unwrap();
        assert!(warns
            .iter()
            .any(|w| w.as_str().unwrap().contains("mixed content")));
    }

    #[test]
    fn tree_depth_caps_the_tree_and_flags_it() {
        let opts = Options {
            format: Format::Json,
            tree_depth: 2,
            ..Default::default()
        };
        let r: Value = serde_json::from_str(&analyze(CATALOG, &opts).unwrap()).unwrap();
        assert_eq!(r["tree_truncated"], true);
        let book = r["tree"][0]["children"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["tag"] == "book")
            .unwrap()
            .clone();
        assert_eq!(book["children_truncated"], true);
        assert!(book["children"].as_array().unwrap().is_empty());
    }

    #[test]
    fn top_tags_truncates_and_zero_lists_all() {
        let opts = Options {
            format: Format::Json,
            top_tags: 2,
            ..Default::default()
        };
        let r: Value = serde_json::from_str(&analyze(CATALOG, &opts).unwrap()).unwrap();
        assert_eq!(r["tags"].as_array().unwrap().len(), 2);
        assert_eq!(r["tags_truncated"], true);

        let opts = Options {
            format: Format::Json,
            top_tags: 0,
            ..Default::default()
        };
        let r: Value = serde_json::from_str(&analyze(CATALOG, &opts).unwrap()).unwrap();
        assert_eq!(r["tags"].as_array().unwrap().len(), 5);
        assert_eq!(r["tags_truncated"], false);
    }

    #[test]
    fn show_attributes_off_drops_attribute_detail() {
        let opts = Options {
            format: Format::Json,
            show_attributes: false,
            ..Default::default()
        };
        let r: Value = serde_json::from_str(&analyze(CATALOG, &opts).unwrap()).unwrap();
        // Attribute TOTALS still count; the per-name breakdown is suppressed.
        assert_eq!(r["counts"]["attributes"], 4);
        assert!(r["attribute_usage"].as_array().unwrap().is_empty());
        let book = r["tags"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["tag"] == "book")
            .unwrap()
            .clone();
        assert!(book["attributes"].as_array().unwrap().is_empty());
        assert!(r["tree"][0]["attributes"].as_array().unwrap().is_empty());
    }

    #[test]
    fn text_format_renders_tree_and_sections() {
        let out = analyze(CATALOG, &Options::default()).unwrap();
        assert!(out.contains("XML Structure Analysis"));
        assert!(out.contains("Root element:  catalog"));
        assert!(out.contains("Max depth:     3"));
        assert!(out.contains("Element tree"));
        assert!(out.contains("catalog (1)"));
        assert!(out.contains("├─ book (2)  [id]"));
        assert!(out.contains("└─ price (2)  [currency]"));
        assert!(out.contains("Tag counts"));
        assert!(out.contains("Attribute usage"));
        assert!(out.contains("Elements per depth"));
    }

    #[test]
    fn csv_format_emits_one_row_per_tag() {
        let opts = Options {
            format: Format::Csv,
            ..Default::default()
        };
        let out = analyze(CATALOG, &opts).unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(
            lines[0],
            "tag,count,min_depth,max_depth,max_children,text_nodes,empty,attributes"
        );
        assert_eq!(lines.len(), 6); // header + 5 distinct tags
        assert!(lines.iter().any(|l| l.starts_with("book,2,2,2,2,0,0,id=2")));
        assert!(lines
            .iter()
            .any(|l| l.starts_with("price,2,3,3,0,2,0,currency=2")));
    }

    #[test]
    fn deep_nesting_warns() {
        // 12 levels deep: a1 … a12.
        let mut xml = String::new();
        for i in 1..=12 {
            xml.push_str(&format!("<a{i}>"));
        }
        for i in (1..=12).rev() {
            xml.push_str(&format!("</a{i}>"));
        }
        let r = json_report(&xml);
        assert_eq!(r["max_depth"], 12);
        let warns = r["warnings"].as_array().unwrap();
        assert!(warns
            .iter()
            .any(|w| w.as_str().unwrap().contains("Deep nesting")));
    }

    #[test]
    fn self_closing_root_is_depth_one() {
        let r = json_report("<empty/>");
        assert_eq!(r["root"], "empty");
        assert_eq!(r["max_depth"], 1);
        assert_eq!(r["counts"]["elements"], 1);
        assert_eq!(r["counts"]["empty_elements"], 1);
        assert_eq!(r["avg_depth"], 1.0);
    }

    #[test]
    fn multiple_roots_warn() {
        let r = json_report("<a/><b/>");
        let warns = r["warnings"].as_array().unwrap();
        assert!(warns
            .iter()
            .any(|w| w.as_str().unwrap().contains("top-level elements")));
    }

    #[test]
    fn doctype_is_reported() {
        let r = json_report("<!DOCTYPE note SYSTEM \"note.dtd\"><note><to>Bo</to></note>");
        assert!(r["doctype"].as_str().unwrap().contains("note"));
    }

    #[test]
    fn mismatched_end_tag_errors_with_position() {
        let err = analyze("<a>\n  <b>x</c>\n</a>", &Options::default()).unwrap_err();
        assert!(err.contains("not well-formed XML"), "got: {err}");
        assert!(err.contains("line 2"), "got: {err}");
    }

    #[test]
    fn unclosed_element_errors() {
        let err = analyze("<a><b></a>", &Options::default()).unwrap_err();
        assert!(err.contains("not well-formed XML"), "got: {err}");
    }

    #[test]
    fn truly_unclosed_root_errors() {
        // quick-xml reaches EOF with <a> still open.
        let err = analyze("<a><b/>", &Options::default()).unwrap_err();
        assert!(err.contains("never closed"), "got: {err}");
    }

    #[test]
    fn empty_input_errors() {
        let err = analyze("   \n ", &Options::default()).unwrap_err();
        assert!(err.contains("no XML provided"));
    }

    #[test]
    fn document_without_elements_errors() {
        let err = analyze(
            "<?xml version=\"1.0\"?><!-- nothing here -->",
            &Options::default(),
        )
        .unwrap_err();
        assert!(err.contains("no elements found"), "got: {err}");
    }
}
