//! opml-converter core — convert OPML subscription files (podcast/RSS exports)
//! between OPML, JSON, and CSV in any direction. No wafer/wasm-bindgen deps;
//! shared by the chat skill block and the web page.
//!
//! Model: an OPML document is a tree of `<outline>` elements, each carrying
//! attributes (text, title, type, xmlUrl, htmlUrl, …) and optional child
//! outlines (folders/categories). We parse any of the three formats into a
//! common [`Doc`] (an optional head title + a forest of [`Outline`] nodes) and
//! re-emit it in the target format.
//!
//! - **OPML ⇄ JSON** is a faithful round-trip of the whole tree: each outline
//!   becomes a JSON object of its attributes plus an `"outlines"` array of
//!   children (omitted when empty).
//! - **CSV** is the flat *feed list*: one row per feed (an outline with a
//!   non-empty `xmlUrl`). A `category` column records the slash-joined folder
//!   path so folders round-trip back to nested OPML/JSON. Pure container
//!   outlines (folders with no `xmlUrl`) are not emitted as their own rows.

use quick_xml::events::Event;
use quick_xml::name::QName;
use quick_xml::reader::Reader;
use serde_json::{Map, Value};

/// Separator used to join/split the folder path in the CSV `category` column.
const CAT_SEP: &str = " / ";

/// Supported conversion formats.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Format {
    Opml,
    Json,
    Csv,
}

impl Format {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "opml" | "xml" => Ok(Format::Opml),
            "json" => Ok(Format::Json),
            "csv" => Ok(Format::Csv),
            other => Err(format!(
                "invalid format {other:?}: expected \"opml\", \"json\", or \"csv\""
            )),
        }
    }
}

/// A single `<outline>` node: ordered attributes + child outlines.
#[derive(Clone, Default)]
struct Outline {
    attrs: Vec<(String, String)>,
    children: Vec<Outline>,
}

impl Outline {
    /// Look up an attribute case-insensitively (OPML keys like `xmlUrl` vary).
    fn get(&self, key: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.as_str())
    }

    fn set(&mut self, key: &str, val: String) {
        if let Some(slot) = self.attrs.iter_mut().find(|(k, _)| k.eq_ignore_ascii_case(key)) {
            slot.1 = val;
        } else {
            self.attrs.push((key.to_string(), val));
        }
    }

    /// True when this outline points at a feed (has a non-empty xmlUrl).
    fn is_feed(&self) -> bool {
        self.get("xmlUrl").map(|u| !u.trim().is_empty()).unwrap_or(false)
    }

    /// Best display label for a folder: `text` then `title`.
    fn label(&self) -> &str {
        self.get("text").or_else(|| self.get("title")).unwrap_or("")
    }
}

/// A parsed OPML document.
#[derive(Default)]
struct Doc {
    title: Option<String>,
    outlines: Vec<Outline>,
}

/// Convert `input` from one format to another.
///
/// `pretty` indents JSON / OPML output (ignored for CSV). Returns an error for
/// malformed input or an unrecognised format.
pub fn convert(input: &str, from: Format, to: Format, pretty: bool) -> Result<String, String> {
    let doc = match from {
        Format::Opml => parse_opml(input)?,
        Format::Json => parse_json(input)?,
        Format::Csv => parse_csv(input)?,
    };
    match to {
        Format::Opml => Ok(to_opml(&doc, pretty)),
        Format::Json => to_json(&doc, pretty),
        Format::Csv => to_csv(&doc),
    }
}

// ---------------------------------------------------------------------------
// OPML parsing
// ---------------------------------------------------------------------------

/// Strip any `ns:` prefix from an XML qualified name.
fn local_name(name: QName) -> String {
    let full = name.as_ref();
    let local = match full.iter().position(|&b| b == b':') {
        Some(i) => &full[i + 1..],
        None => full,
    };
    String::from_utf8_lossy(local).into_owned()
}

fn parse_opml(input: &str) -> Result<Doc, String> {
    let mut reader = Reader::from_str(input);
    reader.config_mut().trim_text(true);
    let decoder = reader.decoder();

    let mut stack: Vec<Outline> = Vec::new();
    let mut roots: Vec<Outline> = Vec::new();
    let mut title: Option<String> = None;
    // Capture <title> text only in <head> (i.e. when no <outline> is open).
    let mut in_title = false;
    let mut saw_opml = false;

    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Err(e) => return Err(format!("malformed OPML/XML: {e}")),
            Ok(Event::Eof) => break,
            Ok(ev @ (Event::Start(_) | Event::Empty(_))) => {
                let is_empty = matches!(ev, Event::Empty(_));
                let e = match ev {
                    Event::Start(e) | Event::Empty(e) => e,
                    _ => unreachable!(),
                };
                let local = local_name(e.name()).to_ascii_lowercase();
                match local.as_str() {
                    "opml" => saw_opml = true,
                    "outline" => {
                        let mut node = Outline::default();
                        for attr in e.attributes().flatten() {
                            let key = local_name(QName(attr.key.as_ref()));
                            #[allow(deprecated)]
                            let val = attr
                                .decode_and_unescape_value(decoder)
                                .unwrap_or_default()
                                .into_owned();
                            node.attrs.push((key, val));
                        }
                        if is_empty {
                            add_node(node, &mut stack, &mut roots);
                        } else {
                            stack.push(node);
                        }
                    }
                    "title" if stack.is_empty() => in_title = true,
                    _ => {}
                }
            }
            Ok(Event::Text(t)) => {
                if in_title {
                    let s = t.decode().unwrap_or_default().trim().to_string();
                    if !s.is_empty() {
                        title = Some(s);
                    }
                }
            }
            Ok(Event::End(e)) => {
                let local = local_name(e.name()).to_ascii_lowercase();
                match local.as_str() {
                    "outline" => {
                        if let Some(node) = stack.pop() {
                            add_node(node, &mut stack, &mut roots);
                        }
                    }
                    "title" => in_title = false,
                    _ => {}
                }
            }
            _ => {}
        }
        buf.clear();
    }

    if !saw_opml && roots.is_empty() {
        return Err("not an OPML document: no <opml>/<outline> elements found".into());
    }
    Ok(Doc { title, outlines: roots })
}

/// Attach a finished outline to its parent (the open node) or to the roots.
fn add_node(node: Outline, stack: &mut [Outline], roots: &mut Vec<Outline>) {
    if let Some(parent) = stack.last_mut() {
        parent.children.push(node);
    } else {
        roots.push(node);
    }
}

// ---------------------------------------------------------------------------
// OPML emit
// ---------------------------------------------------------------------------

fn xml_escape_attr(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

fn xml_escape_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
    out
}

fn to_opml(doc: &Doc, pretty: bool) -> String {
    let nl = if pretty { "\n" } else { "" };
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>");
    out.push_str(nl);
    out.push_str("<opml version=\"2.0\">");
    out.push_str(nl);
    // head
    out.push_str(&indent(1, pretty));
    out.push_str("<head>");
    out.push_str(nl);
    if let Some(t) = &doc.title {
        out.push_str(&indent(2, pretty));
        out.push_str(&format!("<title>{}</title>", xml_escape_text(t)));
        out.push_str(nl);
    }
    out.push_str(&indent(1, pretty));
    out.push_str("</head>");
    out.push_str(nl);
    // body
    out.push_str(&indent(1, pretty));
    out.push_str("<body>");
    out.push_str(nl);
    for o in &doc.outlines {
        write_outline(&mut out, o, 2, pretty);
    }
    out.push_str(&indent(1, pretty));
    out.push_str("</body>");
    out.push_str(nl);
    out.push_str("</opml>");
    if pretty {
        out.push('\n');
    }
    out
}

fn indent(level: usize, pretty: bool) -> String {
    if pretty {
        "  ".repeat(level)
    } else {
        String::new()
    }
}

fn write_outline(out: &mut String, o: &Outline, level: usize, pretty: bool) {
    let nl = if pretty { "\n" } else { "" };
    out.push_str(&indent(level, pretty));
    out.push_str("<outline");
    for (k, v) in &o.attrs {
        out.push_str(&format!(" {}=\"{}\"", k, xml_escape_attr(v)));
    }
    if o.children.is_empty() {
        out.push_str("/>");
        out.push_str(nl);
    } else {
        out.push('>');
        out.push_str(nl);
        for c in &o.children {
            write_outline(out, c, level + 1, pretty);
        }
        out.push_str(&indent(level, pretty));
        out.push_str("</outline>");
        out.push_str(nl);
    }
}

// ---------------------------------------------------------------------------
// JSON parse + emit
// ---------------------------------------------------------------------------

fn to_json(doc: &Doc, pretty: bool) -> Result<String, String> {
    let mut root = Map::new();
    if let Some(t) = &doc.title {
        root.insert("title".into(), Value::String(t.clone()));
    }
    root.insert(
        "outlines".into(),
        Value::Array(doc.outlines.iter().map(outline_to_value).collect()),
    );
    let v = Value::Object(root);
    if pretty {
        serde_json::to_string_pretty(&v)
    } else {
        serde_json::to_string(&v)
    }
    .map_err(|e| format!("JSON encode error: {e}"))
}

fn outline_to_value(o: &Outline) -> Value {
    let mut m = Map::new();
    for (k, v) in &o.attrs {
        m.insert(k.clone(), Value::String(v.clone()));
    }
    if !o.children.is_empty() {
        m.insert(
            "outlines".into(),
            Value::Array(o.children.iter().map(outline_to_value).collect()),
        );
    }
    Value::Object(m)
}

fn parse_json(input: &str) -> Result<Doc, String> {
    let v: Value =
        serde_json::from_str(input.trim()).map_err(|e| format!("invalid JSON: {e}"))?;
    let obj = v
        .as_object()
        .ok_or("JSON root must be an object like {\"title\":…,\"outlines\":[…]}")?;
    let title = obj
        .get("title")
        .and_then(|t| t.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    let arr = obj
        .get("outlines")
        .or_else(|| obj.get("feeds"))
        .or_else(|| obj.get("children"));
    let outlines = match arr {
        Some(Value::Array(items)) => items.iter().map(value_to_outline).collect::<Result<_, _>>()?,
        Some(_) => return Err("\"outlines\" must be an array".into()),
        None => return Err("JSON object must have an \"outlines\" array".into()),
    };
    Ok(Doc { title, outlines })
}

fn value_to_outline(v: &Value) -> Result<Outline, String> {
    let obj = v.as_object().ok_or("each outline must be a JSON object")?;
    let mut node = Outline::default();
    let mut children: Vec<Outline> = Vec::new();
    for (k, val) in obj {
        if k == "outlines" || k == "children" {
            match val {
                Value::Array(items) => {
                    for it in items {
                        children.push(value_to_outline(it)?);
                    }
                }
                Value::Null => {}
                _ => return Err(format!("\"{k}\" must be an array of outlines")),
            }
            continue;
        }
        let s = match val {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Null => continue,
            _ => return Err(format!("outline attribute \"{k}\" must be a scalar value")),
        };
        node.attrs.push((k.clone(), s));
    }
    node.children = children;
    Ok(node)
}

// ---------------------------------------------------------------------------
// CSV emit (flatten) + parse (rebuild folders)
// ---------------------------------------------------------------------------

/// Walk the tree, emitting one (category_path, &feed) per feed outline.
fn flatten<'a>(node: &'a Outline, path: &[String], rows: &mut Vec<(String, &'a Outline)>) {
    if node.is_feed() {
        rows.push((path.join(CAT_SEP), node));
    }
    if !node.children.is_empty() {
        let mut child_path = path.to_vec();
        // The node's label becomes a folder segment for its children (whether it
        // is a pure container or a feed that also nests sub-feeds).
        let label = node.label();
        if !label.is_empty() {
            child_path.push(label.to_string());
        }
        for c in &node.children {
            flatten(c, &child_path, rows);
        }
    }
}

fn to_csv(doc: &Doc) -> Result<String, String> {
    let mut rows: Vec<(String, &Outline)> = Vec::new();
    for o in &doc.outlines {
        flatten(o, &[], &mut rows);
    }

    // Column order: category first, then the common feed columns that appear,
    // then any other attribute keys in first-seen order.
    let preferred = ["text", "title", "type", "xmlUrl", "htmlUrl"];
    let mut cols: Vec<String> = Vec::new();
    for p in preferred {
        if rows.iter().any(|(_, o)| o.get(p).is_some()) {
            cols.push(p.to_string());
        }
    }
    for (_, o) in &rows {
        for (k, _) in &o.attrs {
            if !cols.iter().any(|c| c.eq_ignore_ascii_case(k)) {
                cols.push(k.clone());
            }
        }
    }
    if cols.is_empty() {
        // No feeds at all — still emit a usable header.
        cols = vec!["text".into(), "xmlUrl".into()];
    }

    let mut header = vec!["category".to_string()];
    header.extend(cols.iter().cloned());

    let mut wtr = csv::WriterBuilder::new().from_writer(vec![]);
    wtr.write_record(&header)
        .map_err(|e| format!("CSV write error: {e}"))?;
    for (cat, o) in &rows {
        let mut rec: Vec<String> = Vec::with_capacity(header.len());
        rec.push(cat.clone());
        for c in &cols {
            rec.push(o.get(c).unwrap_or("").to_string());
        }
        wtr.write_record(&rec)
            .map_err(|e| format!("CSV write error: {e}"))?;
    }
    let bytes = wtr
        .into_inner()
        .map_err(|e| format!("CSV write error: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("CSV encode error: {e}"))
}

fn parse_csv(input: &str) -> Result<Doc, String> {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(input.as_bytes());
    let headers: Vec<String> = rdr
        .headers()
        .map_err(|e| format!("CSV read error: {e}"))?
        .iter()
        .map(|h| h.to_string())
        .collect();
    if headers.is_empty() {
        return Err("CSV has no header row".into());
    }
    // Find the category column (if any); everything else is an outline attribute.
    let cat_idx = headers.iter().position(|h| h.eq_ignore_ascii_case("category"));

    let mut roots: Vec<Outline> = Vec::new();
    for (i, rec) in rdr.records().enumerate() {
        let rec = rec.map_err(|e| format!("CSV read error (row {}): {e}", i + 2))?;
        let mut feed = Outline::default();
        let mut category = String::new();
        for (j, field) in rec.iter().enumerate() {
            let Some(col) = headers.get(j) else { continue };
            if Some(j) == cat_idx {
                category = field.to_string();
                continue;
            }
            if !field.is_empty() {
                feed.set(col, field.to_string());
            }
        }
        if feed.attrs.is_empty() {
            continue; // blank row
        }
        let folders: Vec<String> = category
            .split(CAT_SEP)
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();
        insert_feed(&mut roots, &folders, feed);
    }
    Ok(Doc { title: None, outlines: roots })
}

/// Insert a feed under a folder path, creating folder outlines as needed.
fn insert_feed(level: &mut Vec<Outline>, folders: &[String], feed: Outline) {
    let mut cur = level;
    for name in folders {
        // Find an existing folder (non-feed outline whose label matches).
        let idx = cur.iter().position(|o| !o.is_feed() && o.label() == name);
        let idx = match idx {
            Some(i) => i,
            None => {
                let mut folder = Outline::default();
                folder.attrs.push(("text".into(), name.clone()));
                cur.push(folder);
                cur.len() - 1
            }
        };
        cur = &mut cur[idx].children;
    }
    cur.push(feed);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <head>
    <title>My Subscriptions</title>
  </head>
  <body>
    <outline text="Tech" title="Tech">
      <outline type="rss" text="Example Feed" xmlUrl="https://example.com/rss" htmlUrl="https://example.com"/>
    </outline>
    <outline type="rss" text="Daily News" xmlUrl="https://news.example.org/feed"/>
  </body>
</opml>"#;

    #[test]
    fn opml_to_json_keeps_tree_and_title() {
        let out = convert(SAMPLE, Format::Opml, Format::Json, true).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["title"], "My Subscriptions");
        // First top-level outline is the Tech folder with one child feed.
        assert_eq!(v["outlines"][0]["text"], "Tech");
        assert_eq!(v["outlines"][0]["outlines"][0]["xmlUrl"], "https://example.com/rss");
        // Attribute order preserved: type before text (as authored).
        assert_eq!(v["outlines"][1]["xmlUrl"], "https://news.example.org/feed");
    }

    #[test]
    fn opml_to_csv_flattens_with_category() {
        let out = convert(SAMPLE, Format::Opml, Format::Csv, true).unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert!(lines[0].starts_with("category,"), "header: {}", lines[0]);
        // The Tech feed carries its folder as the category.
        assert!(out.contains("Tech,Example Feed"), "csv:\n{out}");
        // The top-level feed has an empty category.
        assert!(
            out.lines().any(|l| l.starts_with(",Daily News")),
            "csv:\n{out}"
        );
    }

    #[test]
    fn csv_round_trips_back_to_opml_folders() {
        let csv = convert(SAMPLE, Format::Opml, Format::Csv, true).unwrap();
        let opml = convert(&csv, Format::Csv, Format::Opml, true).unwrap();
        // Folder reconstructed as a container outline holding the feed.
        assert!(opml.contains("<outline text=\"Tech\">"), "opml:\n{opml}");
        assert!(opml.contains("xmlUrl=\"https://example.com/rss\""), "opml:\n{opml}");
        assert!(opml.contains("xmlUrl=\"https://news.example.org/feed\""));
    }

    #[test]
    fn json_round_trips_back_to_opml() {
        let json = convert(SAMPLE, Format::Opml, Format::Json, false).unwrap();
        let opml = convert(&json, Format::Json, Format::Opml, true).unwrap();
        assert!(opml.contains("<title>My Subscriptions</title>"));
        assert!(opml.contains("text=\"Tech\""));
        assert!(opml.contains("xmlUrl=\"https://example.com/rss\""));
    }

    #[test]
    fn escapes_special_chars_in_attributes() {
        let json = r#"{"outlines":[{"text":"A & B <C>","type":"rss","xmlUrl":"https://x/?a=1&b=2"}]}"#;
        let opml = convert(json, Format::Json, Format::Opml, true).unwrap();
        assert!(opml.contains("text=\"A &amp; B &lt;C&gt;\""), "opml:\n{opml}");
        assert!(opml.contains("xmlUrl=\"https://x/?a=1&amp;b=2\""));
        // And it parses back to the original text.
        let back = convert(&opml, Format::Opml, Format::Json, false).unwrap();
        let v: Value = serde_json::from_str(&back).unwrap();
        assert_eq!(v["outlines"][0]["text"], "A & B <C>");
    }

    #[test]
    fn rejects_non_opml_input() {
        let err = convert("just some text", Format::Opml, Format::Json, true).unwrap_err();
        assert!(err.contains("not an OPML"), "err: {err}");
    }

    #[test]
    fn rejects_bad_format() {
        assert!(Format::parse("yaml").is_err());
        assert_eq!(Format::parse("OPML").unwrap(), Format::Opml);
        assert_eq!(Format::parse("csv").unwrap(), Format::Csv);
    }

    #[test]
    fn csv_to_json_builds_nested_folders() {
        let csv = "category,text,xmlUrl\nNews / World,BBC,https://bbc/feed\n,Solo,https://solo/feed\n";
        let json = convert(csv, Format::Csv, Format::Json, true).unwrap();
        let v: Value = serde_json::from_str(&json).unwrap();
        // News -> World -> BBC
        assert_eq!(v["outlines"][0]["text"], "News");
        assert_eq!(v["outlines"][0]["outlines"][0]["text"], "World");
        assert_eq!(v["outlines"][0]["outlines"][0]["outlines"][0]["text"], "BBC");
        // Solo feed at top level.
        assert_eq!(v["outlines"][1]["xmlUrl"], "https://solo/feed");
    }
}
