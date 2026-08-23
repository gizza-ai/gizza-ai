//! gizza-ai/xml-diff core — structural (semantic) diff of two XML documents.
//!
//! Both documents are parsed into element trees with `quick-xml`, then walked in
//! parallel. Attributes are compared as a sorted map (so attribute ORDER never
//! matters), text nodes are whitespace-normalized by default (so indentation and
//! line breaks never matter), and every difference is reported with an
//! XPath-style location such as `/catalog/book[2]/@id` or
//! `/catalog/book[2]/title/text()`.
//!
//! Pure-Rust (`quick-xml`, `serde_json`). No wafer/wasm-bindgen deps.

use quick_xml::events::Event;
use quick_xml::reader::Reader;
use serde::Serialize;
use std::collections::BTreeMap;

/// Maximum accepted size of ONE document, in bytes.
pub const MAX_BYTES: usize = 1_000_000;
/// Maximum accepted element nesting depth.
pub const MAX_DEPTH: usize = 500;
/// Serialized added/removed elements are truncated to this many characters.
const SNIPPET_CHARS: usize = 200;

/// How sibling child elements of a matched parent are paired up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchStrategy {
    /// Position by position (first child vs first child, …).
    Index,
    /// Longest-common-subsequence alignment: pure insertions/deletions are
    /// detected instead of shifting every later sibling.
    Lcs,
    /// Siblings are treated as a set — order is ignored entirely.
    Unordered,
}

impl MatchStrategy {
    pub fn parse(s: &str) -> Result<MatchStrategy, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "lcs" | "" => Ok(MatchStrategy::Lcs),
            "index" => Ok(MatchStrategy::Index),
            "unordered" | "set" => Ok(MatchStrategy::Unordered),
            other => Err(format!(
                "unknown match strategy '{other}' (use 'lcs', 'index' or 'unordered')"
            )),
        }
    }
}

/// Rendering of the diff report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Json,
    Text,
}

impl OutputFormat {
    pub fn parse(s: &str) -> Result<OutputFormat, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "json" | "" => Ok(OutputFormat::Json),
            "text" | "report" => Ok(OutputFormat::Text),
            other => Err(format!("unknown format '{other}' (use 'json' or 'text')")),
        }
    }
}

/// Comparison options.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    pub strategy: MatchStrategy,
    /// Collapse whitespace runs in text and drop whitespace-only text nodes.
    pub ignore_whitespace: bool,
    /// Drop XML comments before comparing.
    pub ignore_comments: bool,
    /// Compare local names only; drop namespace prefixes and `xmlns*` declarations.
    pub ignore_namespaces: bool,
    /// Compare text/attribute values numerically when both sides parse as numbers.
    pub numeric_text: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            strategy: MatchStrategy::Lcs,
            ignore_whitespace: true,
            ignore_comments: true,
            ignore_namespaces: false,
            numeric_text: false,
        }
    }
}

/// One difference between the two documents.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Change {
    /// XPath-style location, e.g. `/catalog/book[2]`, `/catalog/book[2]/@id`
    /// or `/catalog/book[2]/title/text()`.
    pub path: String,
    /// `"added"` (only in the second document), `"removed"` (only in the first)
    /// or `"changed"` (present in both with a different value).
    pub kind: String,
    /// The value in the first (left/old) document, if present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old: Option<String>,
    /// The value in the second (right/new) document, if present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
struct DiffReport {
    equal: bool,
    added: usize,
    removed: usize,
    changed: usize,
    changes: Vec<Change>,
}

// ---------------------------------------------------------------- parse ----

#[derive(Debug, Clone, PartialEq, Eq)]
enum Kind {
    Element,
    Comment,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Node {
    kind: Kind,
    /// Element name (namespace prefix stripped when `ignore_namespaces`), or
    /// `comment()` for comment nodes.
    name: String,
    /// Attribute name → value, sorted (attribute order is never significant).
    attrs: BTreeMap<String, String>,
    /// Concatenated direct text/CDATA content (normalized), or the comment body.
    text: String,
    children: Vec<Node>,
}

impl Node {
    fn root() -> Node {
        Node {
            kind: Kind::Element,
            name: String::new(),
            attrs: BTreeMap::new(),
            text: String::new(),
            children: Vec::new(),
        }
    }
}

fn local_name(qname: &str) -> &str {
    match qname.rfind(':') {
        Some(i) => &qname[i + 1..],
        None => qname,
    }
}

fn normalize_text(s: &str, ignore_whitespace: bool) -> String {
    if ignore_whitespace {
        s.split_whitespace().collect::<Vec<_>>().join(" ")
    } else {
        s.to_string()
    }
}

/// Parse one document into a virtual root whose children are the top-level nodes.
fn parse(xml: &str, side: &str, opts: &Options) -> Result<Node, String> {
    if xml.len() > MAX_BYTES {
        return Err(format!(
            "the {side} XML is too large ({} bytes); the limit is {MAX_BYTES} bytes (1 MB) per document",
            xml.len()
        ));
    }
    if xml.trim().is_empty() {
        return Err(format!("no XML in the {side} document"));
    }

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);

    let mut stack: Vec<Node> = vec![Node::root()];

    loop {
        match reader.read_event() {
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) => {
                if stack.len() > MAX_DEPTH {
                    return Err(format!(
                        "the {side} XML nests deeper than {MAX_DEPTH} levels; simplify or split the document"
                    ));
                }
                stack.push(element_node(e.name().as_ref(), e.attributes(), opts, side)?);
            }
            Ok(Event::Empty(e)) => {
                let node = element_node(e.name().as_ref(), e.attributes(), opts, side)?;
                push_child(&mut stack, node);
            }
            Ok(Event::End(_)) => {
                // quick-xml's end-name check already guarantees a match.
                let mut node = stack.pop().ok_or_else(|| {
                    format!("the {side} XML has an end tag with no matching start tag")
                })?;
                node.text = normalize_text(&node.text, opts.ignore_whitespace);
                push_child(&mut stack, node);
            }
            Ok(Event::Text(e)) => {
                let raw = e
                    .unescape()
                    .map(|c| c.into_owned())
                    .unwrap_or_else(|_| String::from_utf8_lossy(e.as_ref()).into_owned());
                if opts.ignore_whitespace && raw.trim().is_empty() {
                    continue;
                }
                if let Some(top) = stack.last_mut() {
                    top.text.push_str(&raw);
                }
            }
            Ok(Event::CData(e)) => {
                let raw = String::from_utf8_lossy(e.as_ref()).into_owned();
                if opts.ignore_whitespace && raw.trim().is_empty() {
                    continue;
                }
                if let Some(top) = stack.last_mut() {
                    top.text.push_str(&raw);
                }
            }
            Ok(Event::Comment(e)) => {
                if opts.ignore_comments {
                    continue;
                }
                let body = String::from_utf8_lossy(e.as_ref()).into_owned();
                let node = Node {
                    kind: Kind::Comment,
                    name: "comment()".to_string(),
                    attrs: BTreeMap::new(),
                    text: normalize_text(&body, opts.ignore_whitespace),
                    children: Vec::new(),
                };
                push_child(&mut stack, node);
            }
            // Declaration, DOCTYPE and processing instructions are not compared.
            Ok(_) => {}
            Err(e) => {
                return Err(format!(
                    "the {side} XML is not well-formed at byte {}: {e}",
                    reader.error_position()
                ))
            }
        }
    }

    if stack.len() != 1 {
        return Err(format!(
            "the {side} XML has {} unclosed element(s)",
            stack.len() - 1
        ));
    }
    let mut root = stack.pop().expect("virtual root");
    root.text = normalize_text(&root.text, opts.ignore_whitespace);
    if !root.children.iter().any(|c| c.kind == Kind::Element) {
        return Err(format!("the {side} document has no XML elements"));
    }
    Ok(root)
}

fn push_child(stack: &mut Vec<Node>, node: Node) {
    if let Some(top) = stack.last_mut() {
        top.children.push(node);
    }
}

fn element_node(
    raw_name: &[u8],
    attributes: quick_xml::events::attributes::Attributes,
    opts: &Options,
    side: &str,
) -> Result<Node, String> {
    let qname = String::from_utf8_lossy(raw_name).into_owned();
    let name = if opts.ignore_namespaces {
        local_name(&qname).to_string()
    } else {
        qname
    };

    let mut attrs = BTreeMap::new();
    for attr in attributes {
        let attr =
            attr.map_err(|e| format!("the {side} XML has an invalid attribute: {e}"))?;
        let key = String::from_utf8_lossy(attr.key.as_ref()).into_owned();
        if opts.ignore_namespaces && (key == "xmlns" || key.starts_with("xmlns:")) {
            continue;
        }
        let key = if opts.ignore_namespaces && key.contains(':') {
            local_name(&key).to_string()
        } else {
            key
        };
        let value = attr
            .unescape_value()
            .map(|c| c.into_owned())
            .unwrap_or_else(|_| String::from_utf8_lossy(attr.value.as_ref()).into_owned());
        attrs.insert(key, normalize_text(&value, opts.ignore_whitespace));
    }

    Ok(Node {
        kind: Kind::Element,
        name,
        attrs,
        text: String::new(),
        children: Vec::new(),
    })
}

// ----------------------------------------------------------------- diff ----

fn values_equal(a: &str, b: &str, opts: &Options) -> bool {
    if a == b {
        return true;
    }
    if opts.numeric_text {
        if let (Ok(x), Ok(y)) = (a.trim().parse::<f64>(), b.trim().parse::<f64>()) {
            return x == y;
        }
    }
    false
}

/// Canonical signature used for deep equality (and for LCS anchors). Children
/// signatures are sorted when the strategy ignores sibling order.
fn signature(n: &Node, opts: &Options, out: &mut String) {
    out.push('<');
    out.push_str(&n.name);
    for (k, v) in &n.attrs {
        out.push(' ');
        out.push_str(k);
        out.push('=');
        out.push_str(v);
    }
    out.push('>');
    out.push_str(&n.text);
    if opts.strategy == MatchStrategy::Unordered {
        let mut kids: Vec<String> = n
            .children
            .iter()
            .map(|c| {
                let mut s = String::new();
                signature(c, opts, &mut s);
                s
            })
            .collect();
        kids.sort();
        for k in kids {
            out.push_str(&k);
        }
    } else {
        for c in &n.children {
            signature(c, opts, out);
        }
    }
    out.push_str("</>");
}

fn sig_of(n: &Node, opts: &Options) -> String {
    let mut s = String::new();
    signature(n, opts, &mut s);
    s
}

/// Escape a value for the compact element serialization used in the report.
fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn serialize_node(n: &Node, out: &mut String) {
    if n.kind == Kind::Comment {
        out.push_str("<!--");
        out.push_str(&n.text);
        out.push_str("-->");
        return;
    }
    out.push('<');
    out.push_str(&n.name);
    for (k, v) in &n.attrs {
        out.push(' ');
        out.push_str(k);
        out.push_str("=\"");
        out.push_str(&esc(v));
        out.push('"');
    }
    if n.text.is_empty() && n.children.is_empty() {
        out.push_str("/>");
        return;
    }
    out.push('>');
    out.push_str(&esc(&n.text));
    for c in &n.children {
        serialize_node(c, out);
    }
    out.push_str("</");
    out.push_str(&n.name);
    out.push('>');
}

fn snippet(n: &Node) -> String {
    let mut s = String::new();
    serialize_node(n, &mut s);
    if s.chars().count() > SNIPPET_CHARS {
        let cut: String = s.chars().take(SNIPPET_CHARS).collect();
        format!("{cut}…")
    } else {
        s
    }
}

/// Path of `siblings[idx]` under `parent`. A `[n]` predicate is added only when
/// the parent has more than one child with that name.
fn child_path(parent: &str, siblings: &[Node], idx: usize) -> String {
    let name = &siblings[idx].name;
    let same: Vec<usize> = siblings
        .iter()
        .enumerate()
        .filter(|(_, s)| &s.name == name)
        .map(|(i, _)| i)
        .collect();
    let base = format!("{parent}/{name}");
    if same.len() > 1 {
        let pos = same.iter().position(|&i| i == idx).unwrap_or(0) + 1;
        format!("{base}[{pos}]")
    } else {
        base
    }
}

#[derive(Debug, Clone, Copy)]
enum Pairing {
    Matched(usize, usize),
    LeftOnly(usize),
    RightOnly(usize),
}

fn pair_children(a: &[Node], b: &[Node], opts: &Options) -> Vec<Pairing> {
    match opts.strategy {
        MatchStrategy::Index => pair_by_index(0, a, 0, b),
        MatchStrategy::Lcs => pair_by_lcs(a, b, opts),
        MatchStrategy::Unordered => pair_unordered(a, b, opts),
    }
}

/// Positional pairing of the ranges `a[ai..]` / `b[bi..]`; a pair is only formed
/// when the two nodes share a name (otherwise it is a removal plus an addition).
fn pair_by_index(ai: usize, a: &[Node], bi: usize, b: &[Node]) -> Vec<Pairing> {
    let mut out = Vec::new();
    let common = a.len().min(b.len());
    for k in 0..common {
        if a[k].name == b[k].name && a[k].kind == b[k].kind {
            out.push(Pairing::Matched(ai + k, bi + k));
        } else {
            out.push(Pairing::LeftOnly(ai + k));
            out.push(Pairing::RightOnly(bi + k));
        }
    }
    for k in common..a.len() {
        out.push(Pairing::LeftOnly(ai + k));
    }
    for k in common..b.len() {
        out.push(Pairing::RightOnly(bi + k));
    }
    out
}

/// LCS over deep-equal children: identical subtrees become anchors, and the gaps
/// between anchors are paired positionally (so an edited sibling still reports as
/// a change rather than a removal plus an addition).
fn pair_by_lcs(a: &[Node], b: &[Node], opts: &Options) -> Vec<Pairing> {
    let asig: Vec<String> = a.iter().map(|n| sig_of(n, opts)).collect();
    let bsig: Vec<String> = b.iter().map(|n| sig_of(n, opts)).collect();
    let (n, m) = (a.len(), b.len());

    // Classic LCS table over signatures.
    let mut dp = vec![vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            dp[i][j] = if asig[i] == bsig[j] {
                dp[i + 1][j + 1] + 1
            } else {
                dp[i + 1][j].max(dp[i][j + 1])
            };
        }
    }

    let mut out = Vec::new();
    let (mut i, mut j) = (0usize, 0usize);
    let (mut gap_a, mut gap_b) = (Vec::new(), Vec::new());

    while i < n && j < m {
        if asig[i] == bsig[j] {
            flush_gap(&mut out, &mut gap_a, &mut gap_b, a, b);
            out.push(Pairing::Matched(i, j));
            i += 1;
            j += 1;
        } else if dp[i + 1][j] >= dp[i][j + 1] {
            gap_a.push(i);
            i += 1;
        } else {
            gap_b.push(j);
            j += 1;
        }
    }
    while i < n {
        gap_a.push(i);
        i += 1;
    }
    while j < m {
        gap_b.push(j);
        j += 1;
    }
    flush_gap(&mut out, &mut gap_a, &mut gap_b, a, b);
    out
}

/// Pair up one LCS gap: positionally, but only where names/kinds agree.
fn flush_gap(
    out: &mut Vec<Pairing>,
    gap_a: &mut Vec<usize>,
    gap_b: &mut Vec<usize>,
    a: &[Node],
    b: &[Node],
) {
    if gap_a.is_empty() && gap_b.is_empty() {
        return;
    }
    let common = gap_a.len().min(gap_b.len());
    for k in 0..common {
        let (x, y) = (gap_a[k], gap_b[k]);
        if a[x].name == b[y].name && a[x].kind == b[y].kind {
            out.push(Pairing::Matched(x, y));
        } else {
            out.push(Pairing::LeftOnly(x));
            out.push(Pairing::RightOnly(y));
        }
    }
    for &x in gap_a.iter().skip(common) {
        out.push(Pairing::LeftOnly(x));
    }
    for &y in gap_b.iter().skip(common) {
        out.push(Pairing::RightOnly(y));
    }
    gap_a.clear();
    gap_b.clear();
}

/// Set-like matching: identical subtrees pair up first (in any order), then the
/// leftovers pair up by element name, then whatever remains is added/removed.
fn pair_unordered(a: &[Node], b: &[Node], opts: &Options) -> Vec<Pairing> {
    let asig: Vec<String> = a.iter().map(|n| sig_of(n, opts)).collect();
    let bsig: Vec<String> = b.iter().map(|n| sig_of(n, opts)).collect();
    let mut used_b = vec![false; b.len()];
    let mut partner: Vec<Option<usize>> = vec![None; a.len()];

    for (i, sa) in asig.iter().enumerate() {
        if let Some(j) = (0..b.len()).find(|&j| !used_b[j] && &bsig[j] == sa) {
            used_b[j] = true;
            partner[i] = Some(j);
        }
    }
    for i in 0..a.len() {
        if partner[i].is_some() {
            continue;
        }
        if let Some(j) = (0..b.len())
            .find(|&j| !used_b[j] && b[j].name == a[i].name && b[j].kind == a[i].kind)
        {
            used_b[j] = true;
            partner[i] = Some(j);
        }
    }

    let mut out = Vec::new();
    for (i, p) in partner.iter().enumerate() {
        match p {
            Some(j) => out.push(Pairing::Matched(i, *j)),
            None => out.push(Pairing::LeftOnly(i)),
        }
    }
    for (j, used) in used_b.iter().enumerate() {
        if !used {
            out.push(Pairing::RightOnly(j));
        }
    }
    out
}

fn diff_nodes(path: &str, a: &Node, b: &Node, opts: &Options, out: &mut Vec<Change>) {
    // Attributes (compared as a sorted map — order is never significant).
    for (k, av) in &a.attrs {
        match b.attrs.get(k) {
            None => out.push(Change {
                path: format!("{path}/@{k}"),
                kind: "removed".into(),
                old: Some(av.clone()),
                new: None,
            }),
            Some(bv) if !values_equal(av, bv, opts) => out.push(Change {
                path: format!("{path}/@{k}"),
                kind: "changed".into(),
                old: Some(av.clone()),
                new: Some(bv.clone()),
            }),
            Some(_) => {}
        }
    }
    for (k, bv) in &b.attrs {
        if !a.attrs.contains_key(k) {
            out.push(Change {
                path: format!("{path}/@{k}"),
                kind: "added".into(),
                old: None,
                new: Some(bv.clone()),
            });
        }
    }

    // Direct text content.
    if !values_equal(&a.text, &b.text, opts) {
        let tp = format!("{path}/text()");
        if a.text.is_empty() {
            out.push(Change {
                path: tp,
                kind: "added".into(),
                old: None,
                new: Some(b.text.clone()),
            });
        } else if b.text.is_empty() {
            out.push(Change {
                path: tp,
                kind: "removed".into(),
                old: Some(a.text.clone()),
                new: None,
            });
        } else {
            out.push(Change {
                path: tp,
                kind: "changed".into(),
                old: Some(a.text.clone()),
                new: Some(b.text.clone()),
            });
        }
    }

    // Children.
    for pairing in pair_children(&a.children, &b.children, opts) {
        match pairing {
            Pairing::Matched(i, j) => {
                let cp = child_path(path, &a.children, i);
                diff_nodes(&cp, &a.children[i], &b.children[j], opts, out);
            }
            Pairing::LeftOnly(i) => out.push(Change {
                path: child_path(path, &a.children, i),
                kind: "removed".into(),
                old: Some(snippet(&a.children[i])),
                new: None,
            }),
            Pairing::RightOnly(j) => out.push(Change {
                path: child_path(path, &b.children, j),
                kind: "added".into(),
                old: None,
                new: Some(snippet(&b.children[j])),
            }),
        }
    }
}

// --------------------------------------------------------------- render ----

fn render_text(r: &DiffReport) -> String {
    if r.equal {
        return "No differences: the two XML documents are equivalent.".to_string();
    }
    let total = r.changes.len();
    let mut s = format!(
        "{total} difference{}: {} added, {} removed, {} changed\n",
        if total == 1 { "" } else { "s" },
        r.added,
        r.removed,
        r.changed
    );
    for c in &r.changes {
        match c.kind.as_str() {
            "added" => s.push_str(&format!(
                "+ {}  {}\n",
                c.path,
                c.new.as_deref().unwrap_or("")
            )),
            "removed" => s.push_str(&format!(
                "- {}  {}\n",
                c.path,
                c.old.as_deref().unwrap_or("")
            )),
            _ => s.push_str(&format!(
                "~ {}  {} -> {}\n",
                c.path,
                c.old.as_deref().unwrap_or(""),
                c.new.as_deref().unwrap_or("")
            )),
        }
    }
    s.trim_end().to_string()
}

/// Diff two XML documents and render the report.
///
/// `indent` is the JSON output indentation in spaces (0 minifies); it is ignored
/// for `OutputFormat::Text`.
pub fn diff_documents(
    left: &str,
    right: &str,
    opts: &Options,
    format: OutputFormat,
    indent: usize,
) -> Result<String, String> {
    let a = parse(left, "first (left)", opts)?;
    let b = parse(right, "second (right)", opts)?;

    let mut changes = Vec::new();
    diff_nodes("", &a, &b, opts, &mut changes);

    let added = changes.iter().filter(|c| c.kind == "added").count();
    let removed = changes.iter().filter(|c| c.kind == "removed").count();
    let changed = changes.iter().filter(|c| c.kind == "changed").count();
    let report = DiffReport {
        equal: changes.is_empty(),
        added,
        removed,
        changed,
        changes,
    };

    match format {
        OutputFormat::Text => Ok(render_text(&report)),
        OutputFormat::Json => {
            let n = indent.min(8);
            if n == 0 {
                serde_json::to_string(&report)
                    .map_err(|e| format!("failed to render the report: {e}"))
            } else {
                let mut buf = Vec::new();
                let pad = vec![b' '; n];
                let fmt = serde_json::ser::PrettyFormatter::with_indent(&pad);
                let mut ser = serde_json::Serializer::with_formatter(&mut buf, fmt);
                serde::Serialize::serialize(&report, &mut ser)
                    .map_err(|e| format!("failed to render the report: {e}"))?;
                Ok(String::from_utf8_lossy(&buf).into_owned())
            }
        }
    }
}

/// Convenience wrapper taking the raw string parameters used by every surface.
#[allow(clippy::too_many_arguments)]
pub fn diff_raw(
    left: &str,
    right: &str,
    strategy: &str,
    ignore_whitespace: bool,
    ignore_comments: bool,
    ignore_namespaces: bool,
    numeric_text: bool,
    format: &str,
    indent: usize,
) -> Result<String, String> {
    let opts = Options {
        strategy: MatchStrategy::parse(strategy)?,
        ignore_whitespace,
        ignore_comments,
        ignore_namespaces,
        numeric_text,
    };
    diff_documents(left, right, &opts, OutputFormat::parse(format)?, indent)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn json(left: &str, right: &str, opts: &Options) -> serde_json::Value {
        serde_json::from_str(&diff_documents(left, right, opts, OutputFormat::Json, 0).unwrap())
            .unwrap()
    }

    #[test]
    fn reports_attribute_and_text_changes_with_paths() {
        let v = json(
            r#"<catalog><book id="1"><title>Rust</title></book></catalog>"#,
            r#"<catalog><book id="2"><title>Rust 2</title></book></catalog>"#,
            &Options::default(),
        );
        assert_eq!(v["equal"], false);
        assert_eq!(v["changed"], 2);
        assert_eq!(v["changes"][0]["path"], "/catalog/book/@id");
        assert_eq!(v["changes"][0]["old"], "1");
        assert_eq!(v["changes"][0]["new"], "2");
        assert_eq!(v["changes"][1]["path"], "/catalog/book/title/text()");
    }

    #[test]
    fn ignores_attribute_order_and_insignificant_whitespace() {
        let v = json(
            "<a x=\"1\" y=\"2\"><b>hi</b></a>",
            "<a y=\"2\"   x=\"1\">\n  <b>  hi  </b>\n</a>",
            &Options::default(),
        );
        assert_eq!(v["equal"], true, "report was {v}");
        assert_eq!(v["changes"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn exact_whitespace_mode_reports_text_difference() {
        let opts = Options {
            ignore_whitespace: false,
            ..Options::default()
        };
        let v = json("<a><b>hi</b></a>", "<a><b>  hi  </b></a>", &opts);
        assert_eq!(v["equal"], false);
        assert_eq!(v["changes"][0]["path"], "/a/b/text()");
    }

    #[test]
    fn lcs_detects_a_middle_insertion_without_shifting_siblings() {
        let left = "<r><i>1</i><i>3</i></r>";
        let right = "<r><i>1</i><i>2</i><i>3</i></r>";
        let v = json(left, right, &Options::default());
        assert_eq!(v["added"], 1, "report was {v}");
        assert_eq!(v["changed"], 0);
        assert_eq!(v["changes"][0]["kind"], "added");
        assert_eq!(v["changes"][0]["new"], "<i>2</i>");
    }

    #[test]
    fn index_strategy_shifts_after_a_middle_insertion() {
        let opts = Options {
            strategy: MatchStrategy::Index,
            ..Options::default()
        };
        let v = json("<r><i>1</i><i>3</i></r>", "<r><i>1</i><i>2</i><i>3</i></r>", &opts);
        // Positional comparison: i[2] changes 3 -> 2 and a third item is added.
        assert_eq!(v["changed"], 1, "report was {v}");
        assert_eq!(v["added"], 1);
        assert_eq!(v["changes"][0]["path"], "/r/i[2]/text()");
    }

    #[test]
    fn unordered_strategy_ignores_sibling_order() {
        let opts = Options {
            strategy: MatchStrategy::Unordered,
            ..Options::default()
        };
        let v = json("<r><a>1</a><b>2</b></r>", "<r><b>2</b><a>1</a></r>", &opts);
        assert_eq!(v["equal"], true, "report was {v}");
    }

    #[test]
    fn default_strategy_reports_reordered_siblings() {
        let v = json(
            "<r><a>1</a><b>2</b></r>",
            "<r><b>2</b><a>1</a></r>",
            &Options::default(),
        );
        assert_eq!(v["equal"], false);
    }

    #[test]
    fn comments_ignored_by_default_and_compared_when_asked() {
        let left = "<a><!-- one --><b/></a>";
        let right = "<a><!-- two --><b/></a>";
        assert_eq!(json(left, right, &Options::default())["equal"], true);

        let opts = Options {
            ignore_comments: false,
            ..Options::default()
        };
        let v = json(left, right, &opts);
        assert_eq!(v["equal"], false, "report was {v}");
        assert!(v["changes"][0]["path"]
            .as_str()
            .unwrap()
            .contains("comment()"));
    }

    #[test]
    fn cdata_is_compared_as_plain_text() {
        let v = json("<a><![CDATA[hi]]></a>", "<a>hi</a>", &Options::default());
        assert_eq!(v["equal"], true, "report was {v}");
    }

    #[test]
    fn namespace_prefixes_can_be_ignored() {
        let left = r#"<ns:a xmlns:ns="urn:x"><ns:b>1</ns:b></ns:a>"#;
        let right = r#"<p:a xmlns:p="urn:x"><p:b>1</p:b></p:a>"#;
        assert_eq!(json(left, right, &Options::default())["equal"], false);
        let opts = Options {
            ignore_namespaces: true,
            ..Options::default()
        };
        assert_eq!(json(left, right, &opts)["equal"], true);
    }

    #[test]
    fn numeric_text_comparison_is_opt_in() {
        let left = "<a v=\"1.0\">2.50</a>";
        let right = "<a v=\"1\">2.5</a>";
        assert_eq!(json(left, right, &Options::default())["changed"], 2);
        let opts = Options {
            numeric_text: true,
            ..Options::default()
        };
        assert_eq!(json(left, right, &opts)["equal"], true);
    }

    #[test]
    fn added_and_removed_elements_carry_a_snippet() {
        let v = json(
            r#"<r><keep/><gone a="1"/></r>"#,
            r#"<r><keep/><fresh>x</fresh></r>"#,
            &Options::default(),
        );
        assert_eq!(v["removed"], 1, "report was {v}");
        assert_eq!(v["added"], 1);
        let kinds: Vec<&str> = v["changes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["old"].as_str().or(c["new"].as_str()).unwrap())
            .collect();
        assert!(kinds.contains(&r#"<gone a="1"/>"#), "{kinds:?}");
        assert!(kinds.contains(&"<fresh>x</fresh>"), "{kinds:?}");
    }

    #[test]
    fn text_render_is_compact() {
        let out = diff_documents(
            r#"<a v="1"/>"#,
            r#"<a v="2"/>"#,
            &Options::default(),
            OutputFormat::Text,
            2,
        )
        .unwrap();
        assert_eq!(out, "1 difference: 0 added, 0 removed, 1 changed\n~ /a/@v  1 -> 2");
    }

    #[test]
    fn text_render_says_equivalent() {
        let out = diff_documents(
            "<a/>",
            "<a></a>",
            &Options::default(),
            OutputFormat::Text,
            2,
        )
        .unwrap();
        assert_eq!(out, "No differences: the two XML documents are equivalent.");
    }

    #[test]
    fn json_indent_is_applied() {
        let out =
            diff_documents("<a/>", "<a/>", &Options::default(), OutputFormat::Json, 2).unwrap();
        assert!(out.starts_with("{\n  \"equal\": true"), "{out}");
    }

    #[test]
    fn malformed_xml_is_rejected_with_the_side_and_position() {
        let err = diff_documents("<a><b></a>", "<a/>", &Options::default(), OutputFormat::Json, 0)
            .unwrap_err();
        assert!(err.contains("first (left)"), "{err}");
        assert!(err.contains("not well-formed"), "{err}");
    }

    #[test]
    fn empty_input_is_rejected() {
        let err =
            diff_documents("   ", "<a/>", &Options::default(), OutputFormat::Json, 0).unwrap_err();
        assert!(err.contains("no XML in the first (left) document"), "{err}");
    }

    #[test]
    fn oversized_document_is_rejected_at_the_boundary() {
        let filler = "x".repeat(MAX_BYTES - "<a></a>".len());
        let at_cap = format!("<a>{filler}</a>");
        assert_eq!(at_cap.len(), MAX_BYTES);
        assert!(
            diff_documents(&at_cap, "<a/>", &Options::default(), OutputFormat::Json, 0).is_ok(),
            "a document of exactly MAX_BYTES must be accepted"
        );

        let over = format!("<a>{filler}y</a>");
        let err = diff_documents(&over, "<a/>", &Options::default(), OutputFormat::Json, 0)
            .unwrap_err();
        assert!(err.contains("too large"), "{err}");
        assert!(err.contains("1000000"), "{err}");
    }

    #[test]
    fn too_deep_nesting_is_rejected() {
        let deep = "<a>".repeat(MAX_DEPTH + 5) + &"</a>".repeat(MAX_DEPTH + 5);
        let err =
            diff_documents(&deep, "<a/>", &Options::default(), OutputFormat::Json, 0).unwrap_err();
        assert!(err.contains("nests deeper"), "{err}");
    }

    #[test]
    fn unknown_strategy_and_format_are_rejected() {
        let err = diff_raw("<a/>", "<a/>", "fuzzy", true, true, false, false, "json", 0)
            .unwrap_err();
        assert!(err.contains("unknown match strategy"), "{err}");
        let err =
            diff_raw("<a/>", "<a/>", "lcs", true, true, false, false, "yaml", 0).unwrap_err();
        assert!(err.contains("unknown format"), "{err}");
    }
}
