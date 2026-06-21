//! gizza-ai/xml-to-json core — convert an XML document into an equivalent JSON
//! structure, preserving attributes and nesting. Pure-Rust (`quick-xml` +
//! `serde_json` with `preserve_order`); no wafer/wasm-bindgen deps.
//!
//! Conversion rules (a "badgerfish"-style mapping that round-trips structure):
//! - Each element becomes a JSON object keyed by its (local) tag.
//! - Attributes become members prefixed by `attr_prefix` (default `@`).
//! - Repeated child tags collapse into a JSON array, in document order.
//! - Element text becomes a string value directly when the element has no
//!   attributes and no child elements; otherwise it is stored under
//!   `text_key` (default `#text`).
//! - With `coerce=true`, text that looks like an integer/float/bool/null is
//!   emitted as the corresponding JSON scalar instead of a string.
//! - The single root element becomes the sole key of the top-level object.

use quick_xml::events::Event;
use quick_xml::reader::Reader;
use serde_json::{Map, Value};

/// Options controlling the XML→JSON mapping.
pub struct Options {
    /// Prefix used for attribute keys (default `@`).
    pub attr_prefix: String,
    /// Key under which mixed text content is stored (default `#text`).
    pub text_key: String,
    /// Drop attributes entirely when false.
    pub include_attributes: bool,
    /// Coerce numeric/bool/null-looking text to JSON scalars.
    pub coerce: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            attr_prefix: "@".to_string(),
            text_key: "#text".to_string(),
            include_attributes: true,
            coerce: false,
        }
    }
}

/// Strip an `ns:local` prefix, returning the local name.
fn local_name(qname: &[u8]) -> String {
    let s = String::from_utf8_lossy(qname);
    match s.rsplit_once(':') {
        Some((_, local)) => local.to_string(),
        None => s.to_string(),
    }
}

/// An element being assembled on the parse stack.
struct Node {
    tag: String,
    attrs: Vec<(String, String)>,
    children: Vec<(String, Node)>,
    text: String,
}

impl Node {
    fn new(tag: String) -> Self {
        Node { tag, attrs: Vec::new(), children: Vec::new(), text: String::new() }
    }

    /// Convert this node's *content* (not its tag) into a JSON value.
    fn into_value(self, opt: &Options) -> Value {
        let has_attrs = opt.include_attributes && !self.attrs.is_empty();
        let has_children = !self.children.is_empty();
        let trimmed = self.text.trim();

        // Leaf with only text (or empty) and no attrs/children → scalar/string.
        if !has_attrs && !has_children {
            if trimmed.is_empty() {
                return Value::Null;
            }
            return scalar(trimmed, opt);
        }

        let mut map: Map<String, Value> = Map::new();

        if has_attrs {
            for (k, v) in self.attrs {
                map.insert(format!("{}{}", opt.attr_prefix, k), scalar(&v, opt));
            }
        }

        // Group children by tag in first-seen order, collapsing repeats to arrays.
        let mut order: Vec<String> = Vec::new();
        let mut grouped: std::collections::HashMap<String, Vec<Value>> =
            std::collections::HashMap::new();
        for (tag, node) in self.children {
            let val = node.into_value(opt);
            if !grouped.contains_key(&tag) {
                order.push(tag.clone());
            }
            grouped.entry(tag).or_default().push(val);
        }
        for tag in order {
            let mut vals = grouped.remove(&tag).unwrap();
            if vals.len() == 1 {
                map.insert(tag, vals.pop().unwrap());
            } else {
                map.insert(tag, Value::Array(vals));
            }
        }

        if !trimmed.is_empty() {
            map.insert(opt.text_key.clone(), scalar(trimmed, opt));
        }

        Value::Object(map)
    }
}

/// Turn a text fragment into a JSON scalar, coercing only when requested.
fn scalar(s: &str, opt: &Options) -> Value {
    if opt.coerce {
        match s {
            "true" => return Value::Bool(true),
            "false" => return Value::Bool(false),
            "null" => return Value::Null,
            _ => {}
        }
        // Keep leading-zero strings (e.g. "007") as strings to avoid data loss.
        let leading_zero = s.len() > 1 && s.starts_with('0') && s.as_bytes()[1].is_ascii_digit();
        if !leading_zero {
            if let Ok(i) = s.parse::<i64>() {
                return Value::from(i);
            }
            if let Ok(f) = s.parse::<f64>() {
                if f.is_finite() {
                    return Value::from(f);
                }
            }
        }
    }
    Value::String(s.to_string())
}

/// Convert XML text to a pretty-printed JSON string.
pub fn to_json(xml: &str, opt: &Options) -> Result<String, String> {
    let value = to_value(xml, opt)?;
    serde_json::to_string_pretty(&value).map_err(|e| e.to_string())
}

/// Convert XML text to a `serde_json::Value`.
pub fn to_value(xml: &str, opt: &Options) -> Result<Value, String> {
    if xml.trim().is_empty() {
        return Err("input XML is empty".to_string());
    }
    let mut reader = Reader::from_str(xml);
    let config = reader.config_mut();
    config.trim_text(false);
    config.expand_empty_elements = false;

    let mut stack: Vec<Node> = Vec::new();
    let mut root: Option<(String, Node)> = None;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Err(e) => {
                return Err(format!(
                    "XML parse error at position {}: {}",
                    reader.buffer_position(),
                    e
                ))
            }
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) => {
                let tag = local_name(e.name().as_ref());
                let mut node = Node::new(tag);
                read_attrs(&e, &mut node)?;
                stack.push(node);
            }
            Ok(Event::Empty(e)) => {
                let tag = local_name(e.name().as_ref());
                let mut node = Node::new(tag.clone());
                read_attrs(&e, &mut node)?;
                attach(&mut stack, &mut root, tag, node)?;
            }
            Ok(Event::End(_)) => {
                let node = stack.pop().ok_or_else(|| "unexpected closing tag".to_string())?;
                let tag = node.tag.clone();
                attach(&mut stack, &mut root, tag, node)?;
            }
            Ok(Event::Text(t)) => {
                let txt = t.unescape().map_err(|e| e.to_string())?;
                if let Some(top) = stack.last_mut() {
                    top.text.push_str(&txt);
                }
            }
            Ok(Event::CData(t)) => {
                let txt = String::from_utf8_lossy(&t.into_inner()).into_owned();
                if let Some(top) = stack.last_mut() {
                    top.text.push_str(&txt);
                }
            }
            // Comments, PIs, declarations, doctype: ignored.
            _ => {}
        }
        buf.clear();
    }

    if !stack.is_empty() {
        return Err("unclosed XML element(s)".to_string());
    }

    match root {
        Some((tag, node)) => {
            let mut map = Map::new();
            map.insert(tag, node.into_value(opt));
            Ok(Value::Object(map))
        }
        None => Err("no XML root element found".to_string()),
    }
}

/// Pop a finished node onto its parent (or set it as the document root).
fn attach(
    stack: &mut Vec<Node>,
    root: &mut Option<(String, Node)>,
    tag: String,
    node: Node,
) -> Result<(), String> {
    if let Some(parent) = stack.last_mut() {
        parent.children.push((tag, node));
        Ok(())
    } else if root.is_none() {
        *root = Some((tag, node));
        Ok(())
    } else {
        Err("multiple root elements: XML must have exactly one root".to_string())
    }
}

fn read_attrs(e: &quick_xml::events::BytesStart, node: &mut Node) -> Result<(), String> {
    for attr in e.attributes() {
        let attr = attr.map_err(|err| err.to_string())?;
        let key = local_name(attr.key.as_ref());
        let val = attr
            .unescape_value()
            .map_err(|err| err.to_string())?
            .into_owned();
        node.attrs.push((key, val));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn json(xml: &str, opt: &Options) -> Value {
        to_value(xml, opt).unwrap()
    }

    #[test]
    fn simple_element_text() {
        let v = json("<name>Dune</name>", &Options::default());
        assert_eq!(v, serde_json::json!({"name": "Dune"}));
    }

    #[test]
    fn attributes_and_nesting() {
        let xml = r#"<book id="1"><title>Dune</title><author>Herbert</author></book>"#;
        let v = json(xml, &Options::default());
        assert_eq!(
            v,
            serde_json::json!({
                "book": {
                    "@id": "1",
                    "title": "Dune",
                    "author": "Herbert"
                }
            })
        );
    }

    #[test]
    fn repeated_children_become_array() {
        let xml = "<catalog><book>A</book><book>B</book></catalog>";
        let v = json(xml, &Options::default());
        assert_eq!(v, serde_json::json!({"catalog": {"book": ["A", "B"]}}));
    }

    #[test]
    fn mixed_text_uses_text_key() {
        let xml = r#"<p class="x">hello</p>"#;
        let v = json(xml, &Options::default());
        assert_eq!(v, serde_json::json!({"p": {"@class": "x", "#text": "hello"}}));
    }

    #[test]
    fn custom_attr_prefix_and_text_key() {
        let opt = Options {
            attr_prefix: "$".to_string(),
            text_key: "_".to_string(),
            ..Options::default()
        };
        let xml = r#"<p id="1">hi</p>"#;
        let v = json(xml, &opt);
        assert_eq!(v, serde_json::json!({"p": {"$id": "1", "_": "hi"}}));
    }

    #[test]
    fn drop_attributes() {
        let opt = Options { include_attributes: false, ..Options::default() };
        let xml = r#"<p id="1">hi</p>"#;
        let v = json(xml, &opt);
        assert_eq!(v, serde_json::json!({"p": "hi"}));
    }

    #[test]
    fn coerce_scalars() {
        let opt = Options { coerce: true, ..Options::default() };
        let xml = "<r><n>42</n><f>1.5</f><b>true</b><s>hi</s><z>007</z></r>";
        let v = json(xml, &opt);
        assert_eq!(
            v,
            serde_json::json!({"r": {"n": 42, "f": 1.5, "b": true, "s": "hi", "z": "007"}})
        );
    }

    #[test]
    fn empty_element_is_null() {
        let v = json("<root><a/></root>", &Options::default());
        assert_eq!(v, serde_json::json!({"root": {"a": null}}));
    }

    #[test]
    fn cdata_and_entities_decoded() {
        let xml = "<r><a><![CDATA[a & b]]></a><b>x &amp; y</b></r>";
        let v = json(xml, &Options::default());
        assert_eq!(v, serde_json::json!({"r": {"a": "a & b", "b": "x & y"}}));
    }

    #[test]
    fn namespace_prefix_stripped() {
        let xml = r#"<ns:root xmlns:ns="urn:x"><ns:a>1</ns:a></ns:root>"#;
        let v = json(xml, &Options::default());
        assert_eq!(v["root"]["a"], serde_json::json!("1"));
    }

    #[test]
    fn pretty_output_is_valid_json() {
        let s = to_json("<a>1</a>", &Options::default()).unwrap();
        let _: Value = serde_json::from_str(&s).unwrap();
        assert!(s.contains("\"a\""));
    }

    #[test]
    fn err_on_empty() {
        assert!(to_value("   ", &Options::default()).is_err());
    }

    #[test]
    fn err_on_malformed() {
        assert!(to_value("<a><b></a>", &Options::default()).is_err());
    }

    #[test]
    fn err_on_multiple_roots() {
        assert!(to_value("<a/><b/>", &Options::default()).is_err());
    }
}
