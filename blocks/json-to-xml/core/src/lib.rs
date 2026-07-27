//! gizza-ai/json-to-xml core — render a JSON value as well-formed XML. Pure-Rust
//! (`serde_json` with `preserve_order`); no wafer/wasm-bindgen deps.
//!
//! Conversion rules (the inverse of the `xml-to-json` "badgerfish" mapping, so the
//! two round-trip with matching `attribute_prefix`/`text_key`):
//! - The whole JSON value is wrapped in one `root_element` (default `root`).
//! - An object member becomes a child element `<key>value</key>`; keys are
//!   sanitized to valid XML names.
//! - A member whose key starts with `attribute_prefix` (default `@`) and whose
//!   value is a scalar becomes an XML attribute on the parent element instead of
//!   a child (so `"@id": "1"` → `id="1"`). Set the prefix empty to disable.
//! - A member named `text_key` (default `#text`) with a scalar value becomes the
//!   parent element's text content (mixed content: attributes + text + children).
//! - An array renders each item as an `array_item_element` (default `item`) child.
//! - Strings/numbers/booleans become the element's text; `null` and empty
//!   objects/arrays become an empty self-closing element.
//! - `format` = `pretty` indents by `indent` spaces per level; `compact` emits a
//!   single line. An optional XML declaration can be prepended.

use serde_json::Value;

/// Options controlling the JSON→XML mapping.
pub struct Options {
    /// Name of the single root element that wraps the output (default `root`).
    pub root_element: String,
    /// Element name used for each item of a JSON array (default `item`).
    pub array_item_element: String,
    /// Pretty-print (indented, multi-line) when true; compact single line when false.
    pub pretty: bool,
    /// Spaces per indent level in pretty mode (ignored when compact).
    pub indent: usize,
    /// Prepend `<?xml version="1.0" encoding="UTF-8"?>`.
    pub xml_declaration: bool,
    /// Object keys starting with this prefix become attributes (empty disables).
    pub attribute_prefix: String,
    /// Object key whose scalar value becomes the element's text content.
    pub text_key: String,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            root_element: "root".to_string(),
            array_item_element: "item".to_string(),
            pretty: true,
            indent: 2,
            xml_declaration: false,
            attribute_prefix: "@".to_string(),
            text_key: "#text".to_string(),
        }
    }
}

/// Convert a JSON document to an XML string.
pub fn to_xml(json: &str, opt: &Options) -> Result<String, String> {
    if json.trim().is_empty() {
        return Err("input JSON is empty".to_string());
    }
    let value: Value = serde_json::from_str(json).map_err(|e| format!("invalid JSON: {e}"))?;

    let mut w = Writer {
        out: String::new(),
        opt,
    };
    if opt.xml_declaration {
        w.out.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
        if opt.pretty {
            w.out.push('\n');
        }
    }
    let root = sanitize_name(&opt.root_element, "root");
    w.write_element(&root, &value, 0);
    while w.out.ends_with('\n') {
        w.out.pop();
    }
    Ok(w.out)
}

struct Writer<'a> {
    out: String,
    opt: &'a Options,
}

impl Writer<'_> {
    fn indent(&mut self, depth: usize) {
        if self.opt.pretty {
            for _ in 0..depth * self.opt.indent {
                self.out.push(' ');
            }
        }
    }

    fn newline(&mut self) {
        if self.opt.pretty {
            self.out.push('\n');
        }
    }

    /// Emit a complete element block for `value` under tag `name`, at `depth`.
    fn write_element(&mut self, name: &str, value: &Value, depth: usize) {
        match value {
            Value::Object(map) => {
                let mut attrs = String::new();
                let mut text: Option<String> = None;
                let mut children: Vec<(String, &Value)> = Vec::new();
                let prefix = &self.opt.attribute_prefix;
                for (k, v) in map {
                    if !prefix.is_empty()
                        && k.len() > prefix.len()
                        && k.starts_with(prefix)
                        && is_scalar(v)
                    {
                        let aname = sanitize_name(&k[prefix.len()..], "attr");
                        attrs.push(' ');
                        attrs.push_str(&aname);
                        attrs.push_str("=\"");
                        attrs.push_str(&escape_attr(&scalar_text(v)));
                        attrs.push('"');
                    } else if k == &self.opt.text_key && is_scalar(v) {
                        text = Some(scalar_text(v));
                    } else {
                        children.push((sanitize_name(k, "item"), v));
                    }
                }

                self.indent(depth);
                if children.is_empty() {
                    match &text {
                        Some(t) => {
                            self.out.push('<');
                            self.out.push_str(name);
                            self.out.push_str(&attrs);
                            self.out.push('>');
                            self.out.push_str(&escape_text(t));
                            self.out.push_str("</");
                            self.out.push_str(name);
                            self.out.push('>');
                        }
                        None => {
                            self.out.push('<');
                            self.out.push_str(name);
                            self.out.push_str(&attrs);
                            self.out.push_str("/>");
                        }
                    }
                    self.newline();
                } else {
                    self.out.push('<');
                    self.out.push_str(name);
                    self.out.push_str(&attrs);
                    self.out.push('>');
                    self.newline();
                    if let Some(t) = &text {
                        self.indent(depth + 1);
                        self.out.push_str(&escape_text(t));
                        self.newline();
                    }
                    for (cname, cval) in &children {
                        self.write_element(cname, cval, depth + 1);
                    }
                    self.indent(depth);
                    self.out.push_str("</");
                    self.out.push_str(name);
                    self.out.push('>');
                    self.newline();
                }
            }
            Value::Array(items) => {
                self.indent(depth);
                if items.is_empty() {
                    self.out.push('<');
                    self.out.push_str(name);
                    self.out.push_str("/>");
                    self.newline();
                } else {
                    self.out.push('<');
                    self.out.push_str(name);
                    self.out.push('>');
                    self.newline();
                    let item = sanitize_name(&self.opt.array_item_element, "item");
                    for it in items {
                        self.write_element(&item, it, depth + 1);
                    }
                    self.indent(depth);
                    self.out.push_str("</");
                    self.out.push_str(name);
                    self.out.push('>');
                    self.newline();
                }
            }
            Value::Null => {
                self.indent(depth);
                self.out.push('<');
                self.out.push_str(name);
                self.out.push_str("/>");
                self.newline();
            }
            scalar => {
                self.indent(depth);
                self.out.push('<');
                self.out.push_str(name);
                self.out.push('>');
                self.out.push_str(&escape_text(&scalar_text(scalar)));
                self.out.push_str("</");
                self.out.push_str(name);
                self.out.push('>');
                self.newline();
            }
        }
    }
}

fn is_scalar(v: &Value) -> bool {
    matches!(
        v,
        Value::String(_) | Value::Number(_) | Value::Bool(_) | Value::Null
    )
}

fn scalar_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        _ => String::new(),
    }
}

fn escape_text(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            _ => o.push(c),
        }
    }
    o
}

fn escape_attr(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            _ => o.push(c),
        }
    }
    o
}

/// Coerce an arbitrary key into a valid XML name (letters/digits/`_`/`:`/`-`/`.`,
/// with a valid start char); fall back to `fallback` when nothing usable remains.
fn sanitize_name(raw: &str, fallback: &str) -> String {
    let mapped: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == ':' || c == '-' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if mapped.is_empty() {
        return fallback.to_string();
    }
    let first = mapped.chars().next().unwrap();
    if first.is_ascii_alphabetic() || first == '_' || first == ':' {
        mapped
    } else {
        format!("_{mapped}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn xml(json: &str, opt: &Options) -> String {
        to_xml(json, opt).unwrap()
    }

    #[test]
    fn simple_object() {
        let out = xml(r#"{"name":"Dune"}"#, &Options::default());
        assert_eq!(out, "<root>\n  <name>Dune</name>\n</root>");
    }

    #[test]
    fn attributes_and_nesting() {
        let out = xml(
            r#"{"book":{"@id":"1","title":"Dune"}}"#,
            &Options::default(),
        );
        assert_eq!(
            out,
            "<root>\n  <book id=\"1\">\n    <title>Dune</title>\n  </book>\n</root>"
        );
    }

    #[test]
    fn mixed_content_text_key() {
        let out = xml(
            r##"{"p":{"@class":"x","#text":"hello"}}"##,
            &Options::default(),
        );
        assert_eq!(out, "<root>\n  <p class=\"x\">hello</p>\n</root>");
    }

    #[test]
    fn array_uses_item_element() {
        let out = xml(r#"{"books":["A","B"]}"#, &Options::default());
        assert_eq!(
            out,
            "<root>\n  <books>\n    <item>A</item>\n    <item>B</item>\n  </books>\n</root>"
        );
    }

    #[test]
    fn custom_root_and_item_names() {
        let opt = Options {
            root_element: "catalog".to_string(),
            array_item_element: "book".to_string(),
            ..Options::default()
        };
        let out = xml(r#"["A","B"]"#, &opt);
        assert_eq!(
            out,
            "<catalog>\n  <book>A</book>\n  <book>B</book>\n</catalog>"
        );
    }

    #[test]
    fn compact_has_no_whitespace() {
        let opt = Options {
            pretty: false,
            ..Options::default()
        };
        let out = xml(r#"{"a":1,"b":{"c":2}}"#, &opt);
        assert_eq!(out, "<root><a>1</a><b><c>2</c></b></root>");
    }

    #[test]
    fn indent_spaces_configurable() {
        let opt = Options {
            indent: 4,
            ..Options::default()
        };
        let out = xml(r#"{"a":1}"#, &opt);
        assert_eq!(out, "<root>\n    <a>1</a>\n</root>");
    }

    #[test]
    fn xml_declaration_prepended() {
        let opt = Options {
            xml_declaration: true,
            ..Options::default()
        };
        let out = xml(r#"{"a":1}"#, &opt);
        assert_eq!(
            out,
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<root>\n  <a>1</a>\n</root>"
        );
    }

    #[test]
    fn null_and_empty_are_self_closing() {
        assert_eq!(
            xml(r#"{"a":null}"#, &Options::default()),
            "<root>\n  <a/>\n</root>"
        );
        assert_eq!(
            xml(r#"{"a":[]}"#, &Options::default()),
            "<root>\n  <a/>\n</root>"
        );
        assert_eq!(
            xml(r#"{"a":{}}"#, &Options::default()),
            "<root>\n  <a/>\n</root>"
        );
    }

    #[test]
    fn numbers_and_booleans() {
        let out = xml(r#"{"n":42,"f":1.5,"b":true}"#, &Options::default());
        assert_eq!(
            out,
            "<root>\n  <n>42</n>\n  <f>1.5</f>\n  <b>true</b>\n</root>"
        );
    }

    #[test]
    fn special_chars_are_escaped() {
        let out = xml(r#"{"x":"a & b < c > d"}"#, &Options::default());
        assert_eq!(out, "<root>\n  <x>a &amp; b &lt; c &gt; d</x>\n</root>");
        let a = xml(r#"{"e":{"@q":"a\"b&c"}}"#, &Options::default());
        assert!(a.contains("q=\"a&quot;b&amp;c\""), "attr escaped: {a}");
    }

    #[test]
    fn attributes_disabled_when_prefix_empty() {
        let opt = Options {
            attribute_prefix: String::new(),
            ..Options::default()
        };
        let out = xml(r#"{"e":{"@id":"1"}}"#, &opt);
        // With no prefix, "@id" is just a (sanitized) child element.
        assert_eq!(out, "<root>\n  <e>\n    <_id>1</_id>\n  </e>\n</root>");
    }

    #[test]
    fn invalid_key_is_sanitized() {
        let out = xml(r#"{"1 bad!":"x"}"#, &Options::default());
        assert_eq!(out, "<root>\n  <_1_bad_>x</_1_bad_>\n</root>");
    }

    #[test]
    fn top_level_scalar() {
        assert_eq!(xml("42", &Options::default()), "<root>42</root>");
        assert_eq!(xml(r#""hi""#, &Options::default()), "<root>hi</root>");
    }

    #[test]
    fn err_on_empty() {
        assert!(to_xml("   ", &Options::default()).is_err());
    }

    #[test]
    fn err_on_invalid_json() {
        assert!(to_xml("{bad", &Options::default()).is_err());
    }
}
