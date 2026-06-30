//! structured-data-validator core — pure compute, shared by the chat skill block
//! and the web page. Extracts JSON-LD, microdata and RDFa structured data from a
//! chunk of HTML, lists the entities it finds, and flags common schema.org
//! mistakes (invalid JSON-LD, missing @context / @type, orphan itemprop, a
//! typeof with no vocab, etc.). No wafer/wasm-bindgen deps — html5ever via
//! `scraper` is wasm32-safe (same engine html-table-extractor uses).

use scraper::{ElementRef, Html, Selector};
use serde::Serialize;
use serde_json::{Map, Value};

#[derive(Serialize, Clone)]
pub struct Issue {
    pub severity: String, // "error" | "warning"
    pub format: String,   // "json-ld" | "microdata" | "rdfa"
    pub message: String,
}

impl Issue {
    fn err(format: &str, message: impl Into<String>) -> Self {
        Issue { severity: "error".into(), format: format.into(), message: message.into() }
    }
    fn warn(format: &str, message: impl Into<String>) -> Self {
        Issue { severity: "warning".into(), format: format.into(), message: message.into() }
    }
}

#[derive(Serialize, Clone)]
pub struct Item {
    pub format: String,        // "json-ld" | "microdata" | "rdfa"
    pub types: Vec<String>,    // schema.org @type / itemtype / typeof values
    pub properties: Vec<String>,
}

#[derive(Serialize, Default)]
pub struct Counts {
    pub jsonld: usize,
    pub microdata: usize,
    pub rdfa: usize,
}

#[derive(Serialize, Default)]
pub struct Report {
    pub counts: Counts,
    pub items: Vec<Item>,
    pub issues: Vec<Issue>,
    pub valid: bool, // true when there are no error-severity issues
}

/// Output mode for the validator.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Report,
    Json,
}

pub fn parse_format(s: &str) -> Result<Format, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "report" | "text" => Ok(Format::Report),
        "json" => Ok(Format::Json),
        other => Err(format!("unknown format '{other}' (use 'report' or 'json')")),
    }
}

/// Validate the structured data in `html` and render it as a human-readable
/// report or a machine-readable JSON summary.
pub fn validate(html: &str, format: Format) -> Result<String, String> {
    if html.trim().is_empty() {
        return Err("provide some HTML to validate".into());
    }
    let report = analyze(html);
    match format {
        Format::Json => serde_json::to_string_pretty(&report).map_err(|e| e.to_string()),
        Format::Report => Ok(render(&report)),
    }
}

/// Build the structured-data report from HTML.
pub fn analyze(html: &str) -> Report {
    let doc = Html::parse_document(html);
    let mut report = Report::default();
    extract_jsonld(&doc, &mut report);
    extract_microdata(&doc, &mut report);
    extract_rdfa(&doc, &mut report);
    report.valid = !report.issues.iter().any(|i| i.severity == "error");
    report
}

// ---- JSON-LD -------------------------------------------------------------

fn extract_jsonld(doc: &Html, report: &mut Report) {
    let sel = Selector::parse(r#"script[type="application/ld+json"]"#).unwrap();
    for (i, script) in doc.select(&sel).enumerate() {
        let block = i + 1;
        let raw = script.inner_html();
        let raw = raw.trim();
        if raw.is_empty() {
            report
                .issues
                .push(Issue::warn("json-ld", format!("JSON-LD block {block} is empty.")));
            continue;
        }
        let val: Value = match serde_json::from_str(raw) {
            Ok(v) => v,
            Err(e) => {
                report.issues.push(Issue::err(
                    "json-ld",
                    format!("JSON-LD block {block}: invalid JSON ({e})."),
                ));
                continue;
            }
        };

        // @context applies to the whole block; check it once.
        let (has_ctx, ctx_str) = block_context(&val);
        if !has_ctx {
            report.issues.push(Issue::err(
                "json-ld",
                format!("JSON-LD block {block}: missing \"@context\" (schema.org markup requires it)."),
            ));
        } else if let Some(c) = ctx_str {
            if !c.to_ascii_lowercase().contains("schema.org") {
                report.issues.push(Issue::warn(
                    "json-ld",
                    format!("JSON-LD block {block}: \"@context\" does not reference schema.org."),
                ));
            }
        }

        let nodes = entities(&val);
        if nodes.is_empty() {
            report.issues.push(Issue::warn(
                "json-ld",
                format!("JSON-LD block {block}: no objects found (expected an entity, an array, or @graph)."),
            ));
            continue;
        }
        for node in nodes {
            report.counts.jsonld += 1;
            let types = type_strings(node.get("@type"));
            if types.is_empty() {
                report.issues.push(Issue::warn(
                    "json-ld",
                    format!("JSON-LD block {block}: an object has no \"@type\"."),
                ));
            }
            let properties: Vec<String> =
                node.keys().filter(|k| !k.starts_with('@')).cloned().collect();
            report.items.push(Item { format: "json-ld".into(), types, properties });
        }
    }
}

/// Does the block declare an @context, and what does it stringify to?
fn block_context(val: &Value) -> (bool, Option<String>) {
    let obj = match val {
        Value::Object(m) => Some(m),
        Value::Array(a) => a.iter().find_map(|e| e.as_object()),
        _ => None,
    };
    if let Some(m) = obj {
        if let Some(c) = m.get("@context") {
            return (true, Some(context_to_string(c)));
        }
    }
    (false, None)
}

fn context_to_string(c: &Value) -> String {
    match c {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// The top-level entity objects in a block: the object itself, the members of a
/// top-level array, or the members of an `@graph`.
fn entities(val: &Value) -> Vec<&Map<String, Value>> {
    match val {
        Value::Object(m) => match m.get("@graph") {
            Some(Value::Array(g)) => g.iter().filter_map(|e| e.as_object()).collect(),
            Some(Value::Object(o)) => vec![o],
            _ => vec![m],
        },
        Value::Array(a) => a.iter().filter_map(|e| e.as_object()).collect(),
        _ => vec![],
    }
}

fn type_strings(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::String(s)) => vec![s.clone()],
        Some(Value::Array(a)) => a.iter().filter_map(|e| e.as_str().map(String::from)).collect(),
        _ => vec![],
    }
}

// ---- Microdata -----------------------------------------------------------

fn extract_microdata(doc: &Html, report: &mut Report) {
    let scope_sel = Selector::parse("[itemscope]").unwrap();
    let prop_sel = Selector::parse("[itemprop]").unwrap();

    for item in doc.select(&scope_sel) {
        report.counts.microdata += 1;
        let types = match item.value().attr("itemtype") {
            Some(t) if !t.trim().is_empty() => {
                t.split_whitespace().map(String::from).collect::<Vec<_>>()
            }
            _ => {
                report.issues.push(Issue::warn(
                    "microdata",
                    "an itemscope element has no \"itemtype\" (add a schema.org type URL).",
                ));
                Vec::new()
            }
        };
        // Properties whose nearest enclosing itemscope is *this* element (so a
        // nested item's properties are attributed to the nested item, not here).
        let item_id = item.id();
        let mut properties = Vec::new();
        for p in doc.select(&prop_sel) {
            if nearest_scope(p, "itemscope") == Some(item_id) {
                if let Some(name) = p.value().attr("itemprop") {
                    if !name.trim().is_empty() {
                        properties.push(name.to_string());
                    }
                }
            }
        }
        report.items.push(Item { format: "microdata".into(), types, properties });
    }

    // itemprop with no enclosing itemscope is invalid microdata.
    let orphans = doc
        .select(&prop_sel)
        .filter(|p| nearest_scope(*p, "itemscope").is_none())
        .count();
    if orphans > 0 {
        report.issues.push(Issue::err(
            "microdata",
            format!("{orphans} \"itemprop\" attribute(s) are not inside any itemscope element."),
        ));
    }
}

/// The NodeId of the nearest *ancestor* (excluding the element itself) carrying
/// `attr`, if any.
fn nearest_scope(el: ElementRef, attr: &str) -> Option<ego_tree::NodeId> {
    el.ancestors()
        .find(|n| n.value().as_element().map_or(false, |e| e.attr(attr).is_some()))
        .map(|n| n.id())
}

// ---- RDFa ----------------------------------------------------------------

fn extract_rdfa(doc: &Html, report: &mut Report) {
    let typeof_sel = Selector::parse("[typeof]").unwrap();
    let prop_sel = Selector::parse("[property]").unwrap();

    for item in doc.select(&typeof_sel) {
        report.counts.rdfa += 1;
        let raw_type = item.value().attr("typeof").unwrap_or("").trim().to_string();
        let types: Vec<String> =
            raw_type.split_whitespace().map(String::from).collect();

        // A typeof needs a resolvable vocabulary: a prefixed CURIE (schema:Foo),
        // a full URL, or a `vocab` in scope on the element or an ancestor.
        let has_vocab = item.value().attr("vocab").is_some()
            || item
                .ancestors()
                .any(|n| n.value().as_element().map_or(false, |e| e.attr("vocab").is_some()));
        let resolvable =
            has_vocab || types.iter().any(|t| t.contains(':') || t.contains("//"));
        if !resolvable && !types.is_empty() {
            report.issues.push(Issue::warn(
                "rdfa",
                format!("typeof=\"{raw_type}\" has no vocab and is not a prefixed/absolute term."),
            ));
        }

        let item_id = item.id();
        let mut properties = Vec::new();
        for p in doc.select(&prop_sel) {
            if nearest_scope(p, "typeof") == Some(item_id) {
                if let Some(name) = p.value().attr("property") {
                    if !name.trim().is_empty() {
                        properties.push(name.to_string());
                    }
                }
            }
        }
        report.items.push(Item { format: "rdfa".into(), types, properties });
    }
}

// ---- Rendering -----------------------------------------------------------

fn render(r: &Report) -> String {
    let mut out = String::new();
    out.push_str("Structured data validation\n");
    out.push_str("==========================\n\n");
    out.push_str(&format!(
        "Found: {} JSON-LD, {} microdata, {} RDFa item(s)\n",
        r.counts.jsonld, r.counts.microdata, r.counts.rdfa
    ));
    let errors = r.issues.iter().filter(|i| i.severity == "error").count();
    let warnings = r.issues.iter().filter(|i| i.severity == "warning").count();
    if r.valid {
        out.push_str(&format!("Status: VALID — no errors, {warnings} warning(s)\n"));
    } else {
        out.push_str(&format!("Status: INVALID — {errors} error(s), {warnings} warning(s)\n"));
    }

    if r.items.is_empty() {
        out.push_str("\nNo JSON-LD, microdata or RDFa structured data was found.\n");
    } else {
        out.push_str("\nItems:\n");
        for it in &r.items {
            let ty = if it.types.is_empty() {
                "(untyped)".to_string()
            } else {
                it.types.join(", ")
            };
            let props = if it.properties.is_empty() {
                String::new()
            } else {
                format!(" — {}", it.properties.join(", "))
            };
            out.push_str(&format!("  [{}] {}{}\n", it.format, ty, props));
        }
    }

    if errors > 0 {
        out.push_str("\nErrors:\n");
        for i in r.issues.iter().filter(|i| i.severity == "error") {
            out.push_str(&format!("  x [{}] {}\n", i.format, i.message));
        }
    }
    if warnings > 0 {
        out.push_str("\nWarnings:\n");
        for i in r.issues.iter().filter(|i| i.severity == "warning") {
            out.push_str(&format!("  ! [{}] {}\n", i.format, i.message));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_jsonld_article() {
        let html = r#"<html><head>
            <script type="application/ld+json">
            {"@context":"https://schema.org","@type":"Article","headline":"Hi","author":{"@type":"Person","name":"Jane"}}
            </script></head><body></body></html>"#;
        let r = analyze(html);
        assert_eq!(r.counts.jsonld, 1);
        assert!(r.valid, "well-formed article should be valid");
        let item = &r.items[0];
        assert_eq!(item.types, vec!["Article"]);
        assert!(item.properties.contains(&"headline".to_string()));
    }

    #[test]
    fn invalid_json_is_error() {
        let html = r#"<script type="application/ld+json">{ not json }</script>"#;
        let r = analyze(html);
        assert!(!r.valid);
        assert!(r.issues.iter().any(|i| i.severity == "error" && i.message.contains("invalid JSON")));
    }

    #[test]
    fn missing_context_is_error() {
        let html = r#"<script type="application/ld+json">{"@type":"Thing","name":"x"}</script>"#;
        let r = analyze(html);
        assert!(!r.valid);
        assert!(r.issues.iter().any(|i| i.message.contains("missing \"@context\"")));
    }

    #[test]
    fn non_schema_context_warns() {
        let html = r#"<script type="application/ld+json">{"@context":"http://example.com","@type":"Thing"}</script>"#;
        let r = analyze(html);
        assert!(r.valid, "a non-schema context is a warning, not an error");
        assert!(r.issues.iter().any(|i| i.severity == "warning" && i.message.contains("schema.org")));
    }

    #[test]
    fn graph_yields_multiple_items() {
        let html = r#"<script type="application/ld+json">
        {"@context":"https://schema.org","@graph":[{"@type":"Organization","name":"A"},{"@type":"WebSite","name":"B"}]}
        </script>"#;
        let r = analyze(html);
        assert_eq!(r.counts.jsonld, 2);
        assert!(r.valid);
    }

    #[test]
    fn microdata_item_and_orphan() {
        let html = r#"<div itemscope itemtype="https://schema.org/Product">
            <span itemprop="name">Widget</span>
            <span itemprop="price">9.99</span>
        </div>
        <span itemprop="stray">oops</span>"#;
        let r = analyze(html);
        assert_eq!(r.counts.microdata, 1);
        let item = r.items.iter().find(|i| i.format == "microdata").unwrap();
        assert_eq!(item.types, vec!["https://schema.org/Product"]);
        assert!(item.properties.contains(&"name".to_string()));
        assert!(item.properties.contains(&"price".to_string()));
        // the stray itemprop outside any itemscope is an error
        assert!(!r.valid);
        assert!(r.issues.iter().any(|i| i.format == "microdata" && i.severity == "error"));
    }

    #[test]
    fn nested_microdata_properties_not_double_counted() {
        let html = r#"<div itemscope itemtype="https://schema.org/Product">
            <span itemprop="name">Widget</span>
            <div itemprop="brand" itemscope itemtype="https://schema.org/Brand">
                <span itemprop="name">Acme</span>
            </div>
        </div>"#;
        let r = analyze(html);
        assert_eq!(r.counts.microdata, 2);
        let product = r
            .items
            .iter()
            .find(|i| i.format == "microdata" && i.types == vec!["https://schema.org/Product"])
            .unwrap();
        // Product owns name + brand, but NOT the nested Brand's own name.
        assert!(product.properties.contains(&"name".to_string()));
        assert!(product.properties.contains(&"brand".to_string()));
        assert_eq!(
            product.properties.iter().filter(|p| *p == "name").count(),
            1,
            "nested name must not leak into the parent"
        );
    }

    #[test]
    fn microdata_without_itemtype_warns() {
        let html = r#"<div itemscope><span itemprop="name">x</span></div>"#;
        let r = analyze(html);
        assert_eq!(r.counts.microdata, 1);
        assert!(r.issues.iter().any(|i| i.format == "microdata" && i.message.contains("itemtype")));
    }

    #[test]
    fn rdfa_typeof_with_vocab() {
        let html = r#"<div vocab="https://schema.org/" typeof="Recipe">
            <span property="name">Soup</span>
        </div>"#;
        let r = analyze(html);
        assert_eq!(r.counts.rdfa, 1);
        let item = r.items.iter().find(|i| i.format == "rdfa").unwrap();
        assert_eq!(item.types, vec!["Recipe"]);
        assert!(item.properties.contains(&"name".to_string()));
        assert!(r.valid);
    }

    #[test]
    fn rdfa_typeof_without_vocab_warns() {
        let html = r#"<div typeof="Recipe"><span property="name">Soup</span></div>"#;
        let r = analyze(html);
        assert_eq!(r.counts.rdfa, 1);
        assert!(r.issues.iter().any(|i| i.format == "rdfa" && i.message.contains("vocab")));
    }

    #[test]
    fn prefixed_rdfa_needs_no_vocab() {
        let html = r#"<div typeof="schema:Person"><span property="schema:name">Jo</span></div>"#;
        let r = analyze(html);
        assert!(!r.issues.iter().any(|i| i.format == "rdfa" && i.message.contains("vocab")));
    }

    #[test]
    fn empty_input_errors() {
        assert!(validate("   ", Format::Report).is_err());
    }

    #[test]
    fn json_output_is_parseable() {
        let html = r#"<script type="application/ld+json">{"@context":"https://schema.org","@type":"Thing"}</script>"#;
        let out = validate(html, Format::Json).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["counts"]["jsonld"], 1);
        assert_eq!(v["valid"], true);
    }

    #[test]
    fn report_lists_no_data() {
        let out = validate("<p>nothing here</p>", Format::Report).unwrap();
        assert!(out.contains("No JSON-LD, microdata or RDFa"));
    }
}
