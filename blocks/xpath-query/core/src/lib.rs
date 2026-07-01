//! gizza-ai/xpath-query core — evaluate an XPath 1.0 expression against an XML or
//! XHTML document using `sxd-xpath` + `sxd-document`, both pure-Rust with no I/O or
//! WASI/host deps, so it runs in the gizza wafer runtime (chat SW), the CLI, and the
//! browser page alike.
//!
//! XPath 1.0 evaluates to one of four value types: a node-set, a string, a number, or
//! a boolean. A node-set query (e.g. `//book/title`) selects zero, one, or many nodes,
//! each returned as one output string — either the node's string value (text content)
//! or its serialized outer XML, depending on `output`. A scalar result (e.g.
//! `count(//book)`, `name(/*)`, `//x > 1`) is returned as a single output string.

use sxd_document::dom::{ChildOfElement, ChildOfRoot, Element};
use sxd_document::parser;
use sxd_xpath::nodeset::Node;
use sxd_xpath::{Context, Factory, Value};

/// What to emit for each matched node when the XPath result is a node-set.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Output {
    /// The node's string value (text content of an element, an attribute's value, …).
    Value,
    /// The node's serialized outer XML (the element and its descendants).
    Xml,
}

impl Output {
    pub fn parse(s: &str) -> Result<Output, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "value" | "" => Ok(Output::Value),
            "xml" | "node" | "outer" => Ok(Output::Xml),
            other => Err(format!("unknown output '{other}' (use 'value' or 'xml')")),
        }
    }
}

/// Evaluate `expression` (an XPath 1.0 expression) against the XML/XHTML in `xml`.
///
/// - A node-set result yields one output per matched node (in document order), rendered
///   per `output` (string value or serialized XML).
/// - A string/number/boolean result yields a single output string.
///
/// Errors on invalid XML input or an invalid/unsupported XPath expression.
pub fn query_xpath(expression: &str, xml: &str, output: Output) -> Result<Vec<String>, String> {
    if expression.trim().is_empty() {
        return Err("XPath expression is empty".into());
    }
    let package = parser::parse(xml).map_err(|e| format!("invalid XML input: {e}"))?;
    let document = package.as_document();

    let factory = Factory::new();
    let xpath = factory
        .build(expression)
        .map_err(|e| format!("invalid XPath: {e}"))?
        .ok_or_else(|| "invalid XPath: empty expression".to_string())?;

    let context = Context::new();
    let value = xpath
        .evaluate(&context, document.root())
        .map_err(|e| format!("XPath evaluation error: {e}"))?;

    match value {
        Value::Nodeset(ns) => {
            let mut out = Vec::new();
            for node in ns.document_order() {
                out.push(render_node(node, output));
            }
            Ok(out)
        }
        Value::String(s) => Ok(vec![s]),
        Value::Number(n) => Ok(vec![format_number(n)]),
        Value::Boolean(b) => Ok(vec![b.to_string()]),
    }
}

/// Render a single matched node according to `output`.
fn render_node(node: Node, output: Output) -> String {
    match output {
        Output::Value => node.string_value(),
        Output::Xml => match node {
            Node::Element(e) => serialize_element(e),
            Node::Attribute(a) => format!("{}=\"{}\"", a.name().local_part(), escape_attr(a.value())),
            Node::Text(t) => escape_text(t.text()),
            Node::Comment(c) => format!("<!--{}-->", c.text()),
            Node::ProcessingInstruction(p) => match p.value() {
                Some(v) => format!("<?{} {}?>", p.target(), v),
                None => format!("<?{}?>", p.target()),
            },
            // The document root: serialize its element/comment/PI children.
            Node::Root(r) => r
                .children()
                .into_iter()
                .map(|c| match c {
                    ChildOfRoot::Element(e) => serialize_element(e),
                    ChildOfRoot::Comment(c) => format!("<!--{}-->", c.text()),
                    ChildOfRoot::ProcessingInstruction(p) => match p.value() {
                        Some(v) => format!("<?{} {}?>", p.target(), v),
                        None => format!("<?{}?>", p.target()),
                    },
                })
                .collect::<Vec<_>>()
                .join(""),
            Node::Namespace(n) => format!("xmlns:{}=\"{}\"", n.prefix(), escape_attr(n.uri())),
        },
    }
}

/// Serialize an element and its subtree to outer XML.
fn serialize_element(e: Element) -> String {
    let name = e.name().local_part();
    let mut s = format!("<{name}");
    // Attributes sorted by local name for deterministic output.
    let mut attrs = e.attributes();
    attrs.sort_by_key(|a| a.name().local_part().to_string());
    for a in attrs {
        s.push_str(&format!(" {}=\"{}\"", a.name().local_part(), escape_attr(a.value())));
    }
    let children = e.children();
    if children.is_empty() {
        s.push_str("/>");
        return s;
    }
    s.push('>');
    for child in children {
        match child {
            ChildOfElement::Element(c) => s.push_str(&serialize_element(c)),
            ChildOfElement::Text(t) => s.push_str(&escape_text(t.text())),
            ChildOfElement::Comment(c) => s.push_str(&format!("<!--{}-->", c.text())),
            ChildOfElement::ProcessingInstruction(p) => {
                s.push_str(&match p.value() {
                    Some(v) => format!("<?{} {}?>", p.target(), v),
                    None => format!("<?{}?>", p.target()),
                });
            }
        }
    }
    s.push_str(&format!("</{name}>"));
    s
}

/// Format an XPath number like XPath 1.0 string(): integers without a trailing `.0`.
fn format_number(n: f64) -> String {
    if n.is_nan() {
        return "NaN".into();
    }
    if n.is_infinite() {
        return if n > 0.0 { "Infinity".into() } else { "-Infinity".into() };
    }
    if n == n.trunc() && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

fn escape_text(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn escape_attr(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    const CATALOG: &str = r#"<catalog>
        <book id="b1"><title>Rust</title><price>30</price></book>
        <book id="b2"><title>XML</title><price>9</price></book>
        <book id="b3"><title>XPath</title><price>12</price></book>
    </catalog>"#;

    #[test]
    fn select_text_values() {
        let out = query_xpath("//book/title", CATALOG, Output::Value).unwrap();
        assert_eq!(out, vec!["Rust", "XML", "XPath"]);
    }

    #[test]
    fn select_attribute_value() {
        let out = query_xpath("//book/@id", CATALOG, Output::Value).unwrap();
        assert_eq!(out, vec!["b1", "b2", "b3"]);
    }

    #[test]
    fn predicate_filter() {
        let out = query_xpath("//book[price < 10]/title", CATALOG, Output::Value).unwrap();
        assert_eq!(out, vec!["XML"]);
    }

    #[test]
    fn count_function_is_number() {
        let out = query_xpath("count(//book)", CATALOG, Output::Value).unwrap();
        assert_eq!(out, vec!["3"]);
    }

    #[test]
    fn string_function() {
        let out = query_xpath("name(/*)", CATALOG, Output::Value).unwrap();
        assert_eq!(out, vec!["catalog"]);
    }

    #[test]
    fn boolean_result() {
        let out = query_xpath("count(//book) > 2", CATALOG, Output::Value).unwrap();
        assert_eq!(out, vec!["true"]);
    }

    #[test]
    fn output_xml_serializes_node() {
        let out = query_xpath("//book[@id='b2']", CATALOG, Output::Xml).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0], r#"<book id="b2"><title>XML</title><price>9</price></book>"#);
    }

    #[test]
    fn output_xml_escapes_text() {
        let out = query_xpath("/*", "<r>a &amp; b &lt; c</r>", Output::Xml).unwrap();
        assert_eq!(out, vec!["<r>a &amp; b &lt; c</r>"]);
    }

    #[test]
    fn empty_node_serializes_self_closing() {
        let out = query_xpath("//br", "<doc><br/></doc>", Output::Xml).unwrap();
        assert_eq!(out, vec!["<br/>"]);
    }

    #[test]
    fn no_match_is_empty() {
        let out = query_xpath("//magazine", CATALOG, Output::Value).unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn output_parse() {
        assert!(Output::parse("value").unwrap() == Output::Value);
        assert!(Output::parse("XML").unwrap() == Output::Xml);
        assert!(Output::parse("").unwrap() == Output::Value);
        assert!(Output::parse("bogus").is_err());
    }

    #[test]
    fn errors() {
        assert!(query_xpath("//x", "<not-closed>", Output::Value).is_err()); // bad XML
        assert!(query_xpath("", "<r/>", Output::Value).is_err()); // empty expr
        assert!(query_xpath("//[", "<r/>", Output::Value).is_err()); // invalid xpath
    }
}
