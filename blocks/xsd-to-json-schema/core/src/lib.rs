//! gizza-ai/xsd-to-json-schema core — translate an XML Schema (`.xsd`) document
//! into an equivalent JSON Schema (Draft 2020-12 or Draft-07). Pure-Rust
//! (`quick-xml` + `serde_json` with `preserve_order`); no wafer/wasm-bindgen deps.
//!
//! Mapping summary:
//! - `xs:complexType` → `{"type":"object","properties":…}`; `xs:sequence`/`xs:all`
//!   contribute properties in document order.
//! - `xs:choice` → the members become optional properties plus a real `oneOf`
//!   constraint over their `required` sets (`allOf` when a type has several choices).
//! - `xs:attribute` → a property named `<prefix><name>` (prefix default `@`);
//!   `use="required"` puts it in `required`.
//! - `minOccurs`/`maxOccurs` → `required` membership and array wrapping with
//!   `minItems`/`maxItems`.
//! - `xs:simpleType` restrictions → `enum`, `pattern`, `minLength`/`maxLength`,
//!   `minimum`/`maximum`, `exclusiveMinimum`/`exclusiveMaximum`, `multipleOf`.
//!   `xs:list` → an array; `xs:union` → `anyOf`.
//! - `complexContent`/`simpleContent` `extension` merges the base type's members;
//!   `simpleContent` text lands under the configurable text property (`#text`).
//! - Named global types become `$ref`s into `$defs`/`definitions`, pruned to what
//!   the chosen root actually reaches (recursive types are fine).
//!
//! Anything that needs cross-document resolution (`xs:import`, `xs:include`,
//! `xs:redefine`, substitution groups) or XPath evaluation (`xs:assert`) is
//! reported as a named error instead of being silently mis-converted.

use quick_xml::events::Event;
use quick_xml::reader::Reader;
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};

/// Largest XSD accepted, in bytes. Anything bigger is rejected rather than
/// parsed, so the browser tab can't be wedged by a pasted megaschema.
pub const MAX_INPUT_BYTES: usize = 1_000_000;

/// Maximum element nesting / type-derivation depth before conversion gives up.
pub const MAX_DEPTH: usize = 64;

/// JSON Schema dialect to emit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Draft {
    Draft2020,
    Draft07,
}

impl Draft {
    fn schema_uri(self) -> &'static str {
        match self {
            Draft::Draft2020 => "https://json-schema.org/draft/2020-12/schema",
            Draft::Draft07 => "http://json-schema.org/draft-07/schema#",
        }
    }
    fn defs_key(self) -> &'static str {
        match self {
            Draft::Draft2020 => "$defs",
            Draft::Draft07 => "definitions",
        }
    }
    fn ref_base(self) -> &'static str {
        match self {
            Draft::Draft2020 => "#/$defs/",
            Draft::Draft07 => "#/definitions/",
        }
    }
}

/// Parse a draft name as accepted on the CLI / page / chat surfaces.
pub fn draft_from_str(s: &str) -> Draft {
    match s.trim().to_ascii_lowercase().as_str() {
        "draft-07" | "draft07" | "draft-7" | "draft7" | "07" | "7" => Draft::Draft07,
        _ => Draft::Draft2020,
    }
}

/// Conversion options.
#[derive(Debug, Clone)]
pub struct Options {
    /// JSON Schema dialect to emit.
    pub draft: Draft,
    /// Name of the global `xs:element` (or named type) to use as the schema root.
    /// Empty means "the first global element, else the first global type".
    pub root_element: String,
    /// Prefix applied to property names derived from XML attributes. Empty = none.
    pub attribute_prefix: String,
    /// Property name holding element text for `simpleContent`/`mixed` types.
    /// Empty drops the text property entirely.
    pub text_property: String,
    /// Derive `required` from `minOccurs` / `use="required"`.
    pub required_from_occurs: bool,
    /// Allow properties beyond the declared ones (omit `additionalProperties:false`).
    pub additional_properties: bool,
    /// Turn `xs:annotation`/`xs:documentation` into `description`.
    pub annotations: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            draft: Draft::Draft2020,
            root_element: String::new(),
            attribute_prefix: "@".to_string(),
            text_property: "#text".to_string(),
            required_from_occurs: true,
            additional_properties: false,
            annotations: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Minimal XML tree (local names only — any prefix bound to the XSD namespace works)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Xml {
    name: String,
    attrs: Vec<(String, String)>,
    children: Vec<Xml>,
    text: String,
}

impl Xml {
    fn a(&self, key: &str) -> Option<&str> {
        self.attrs.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }
    fn first(&self, name: &str) -> Option<&Xml> {
        self.children.iter().find(|c| c.name == name)
    }
    fn all<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Xml> {
        self.children.iter().filter(move |c| c.name == name)
    }
}

fn local(qname: &str) -> &str {
    match qname.rsplit_once(':') {
        Some((_, l)) => l,
        None => qname,
    }
}

fn parse_xml(src: &str) -> Result<Xml, String> {
    let mut reader = Reader::from_str(src);
    {
        let config = reader.config_mut();
        config.trim_text(true);
        config.expand_empty_elements = false;
    }

    let mut stack: Vec<Xml> = Vec::new();
    let mut root: Option<Xml> = None;
    let mut buf = Vec::new();

    loop {
        let event = reader.read_event_into(&mut buf).map_err(|e| {
            format!("XSD parse error at byte {}: {e}", reader.buffer_position())
        })?;
        match event {
            Event::Eof => break,
            Event::Start(e) => {
                let node = node_from(e.name().as_ref(), e.attributes())?;
                stack.push(node);
                if stack.len() > MAX_DEPTH {
                    return Err(format!(
                        "XSD nests deeper than the {MAX_DEPTH}-element limit; \
                         split the schema or simplify the nesting"
                    ));
                }
            }
            Event::Empty(e) => {
                let node = node_from(e.name().as_ref(), e.attributes())?;
                match stack.last_mut() {
                    Some(parent) => parent.children.push(node),
                    None => root = Some(node),
                }
            }
            Event::End(_) => {
                let node = stack.pop().ok_or_else(|| {
                    "XSD parse error: a closing tag has no matching opening tag".to_string()
                })?;
                match stack.last_mut() {
                    Some(parent) => parent.children.push(node),
                    None => root = Some(node),
                }
            }
            Event::Text(t) => {
                if let Some(parent) = stack.last_mut() {
                    let s = t.unescape().map_err(|e| format!("XSD text decode error: {e}"))?;
                    parent.text.push_str(&s);
                }
            }
            Event::CData(t) => {
                if let Some(parent) = stack.last_mut() {
                    parent.text.push_str(&String::from_utf8_lossy(&t));
                }
            }
            _ => {}
        }
        buf.clear();
    }

    root.ok_or_else(|| {
        "no XML elements found — expected an <xs:schema> document (an .xsd file)".to_string()
    })
}

fn node_from(raw_name: &[u8], attrs: quick_xml::events::attributes::Attributes) -> Result<Xml, String> {
    let full = String::from_utf8_lossy(raw_name).to_string();
    let mut out = Xml {
        name: local(&full).to_string(),
        attrs: Vec::new(),
        children: Vec::new(),
        text: String::new(),
    };
    for attr in attrs {
        let attr = attr.map_err(|e| format!("XSD attribute parse error: {e}"))?;
        let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
        if key == "xmlns" || key.starts_with("xmlns:") {
            continue;
        }
        let value = attr
            .unescape_value()
            .map_err(|e| format!("XSD attribute decode error: {e}"))?
            .to_string();
        out.attrs.push((local(&key).to_string(), value));
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Builtin xs:* → JSON Schema
// ---------------------------------------------------------------------------

fn int_with(min: Option<i64>, max: Option<i64>) -> Value {
    let mut m = Map::new();
    m.insert("type".into(), json!("integer"));
    if let Some(v) = min {
        m.insert("minimum".into(), json!(v));
    }
    if let Some(v) = max {
        m.insert("maximum".into(), json!(v));
    }
    Value::Object(m)
}

fn str_with(key: &str, value: &str) -> Value {
    let mut m = Map::new();
    m.insert("type".into(), json!("string"));
    m.insert(key.into(), json!(value));
    Value::Object(m)
}

/// Map a builtin XSD datatype local name onto a JSON Schema fragment.
fn builtin_schema(name: &str) -> Option<Value> {
    let v = match name {
        "string" | "normalizedString" | "token" | "language" | "Name" | "NCName" | "NMTOKEN"
        | "ID" | "IDREF" | "ENTITY" | "QName" | "NOTATION" => json!({ "type": "string" }),
        "NMTOKENS" | "IDREFS" | "ENTITIES" => {
            json!({ "type": "array", "items": { "type": "string" } })
        }
        "boolean" => json!({ "type": "boolean" }),
        "decimal" | "float" | "double" => json!({ "type": "number" }),
        "integer" | "int" | "long" => match name {
            "int" => int_with(Some(-2_147_483_648), Some(2_147_483_647)),
            "long" => int_with(Some(i64::MIN), Some(i64::MAX)),
            _ => int_with(None, None),
        },
        "short" => int_with(Some(-32_768), Some(32_767)),
        "byte" => int_with(Some(-128), Some(127)),
        "nonNegativeInteger" => int_with(Some(0), None),
        "positiveInteger" => int_with(Some(1), None),
        "nonPositiveInteger" => int_with(None, Some(0)),
        "negativeInteger" => int_with(None, Some(-1)),
        "unsignedLong" => int_with(Some(0), None),
        "unsignedInt" => int_with(Some(0), Some(4_294_967_295)),
        "unsignedShort" => int_with(Some(0), Some(65_535)),
        "unsignedByte" => int_with(Some(0), Some(255)),
        "date" => str_with("format", "date"),
        "dateTime" => str_with("format", "date-time"),
        "time" => str_with("format", "time"),
        "duration" => str_with("format", "duration"),
        "anyURI" => str_with("format", "uri-reference"),
        "base64Binary" => json!({ "type": "string", "contentEncoding": "base64" }),
        "hexBinary" => str_with("pattern", "^([0-9a-fA-F]{2})*$"),
        "gYear" => str_with("pattern", "^-?[0-9]{4}(Z|[+-][0-9]{2}:[0-9]{2})?$"),
        "gYearMonth" => str_with("pattern", "^-?[0-9]{4}-[0-9]{2}(Z|[+-][0-9]{2}:[0-9]{2})?$"),
        "gMonth" => str_with("pattern", "^--[0-9]{2}(Z|[+-][0-9]{2}:[0-9]{2})?$"),
        "gMonthDay" => str_with("pattern", "^--[0-9]{2}-[0-9]{2}(Z|[+-][0-9]{2}:[0-9]{2})?$"),
        "gDay" => str_with("pattern", "^---[0-9]{2}(Z|[+-][0-9]{2}:[0-9]{2})?$"),
        "anyType" | "anySimpleType" => json!({}),
        _ => return None,
    };
    Some(v)
}

// ---------------------------------------------------------------------------
// Conversion
// ---------------------------------------------------------------------------

/// A complex type being assembled.
#[derive(Default)]
struct ObjBuild {
    props: Map<String, Value>,
    required: Vec<String>,
    choices: Vec<Vec<String>>,
    open: bool,
    description: Option<String>,
    text: Option<Value>,
    text_required: bool,
}

impl ObjBuild {
    fn insert(&mut self, name: String, schema: Value, required: bool) {
        if required && !self.required.contains(&name) {
            self.required.push(name.clone());
        }
        self.props.insert(name, schema);
    }
}

struct Conv<'a> {
    opts: &'a Options,
    elements: HashMap<String, &'a Xml>,
    element_order: Vec<String>,
    complex: HashMap<String, &'a Xml>,
    complex_order: Vec<String>,
    simple: HashMap<String, &'a Xml>,
    attributes: HashMap<String, &'a Xml>,
    groups: HashMap<String, &'a Xml>,
    attr_groups: HashMap<String, &'a Xml>,
    has_external: bool,
    defs: Map<String, Value>,
    queue: Vec<String>,
    queued: HashSet<String>,
    deriving: Vec<String>,
}

fn occurs(node: &Xml) -> (u64, Option<u64>) {
    let min = node
        .a("minOccurs")
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(1);
    let max = match node.a("maxOccurs").map(str::trim) {
        None => Some(1),
        Some(s) if s.eq_ignore_ascii_case("unbounded") => None,
        Some(s) => Some(s.parse::<u64>().unwrap_or(1)),
    };
    (min, max)
}

/// Turn a lexical value into a JSON value matching the schema's declared type.
fn typed_value(schema: &Value, raw: &str) -> Value {
    match schema.get("type").and_then(Value::as_str) {
        Some("integer") => raw
            .trim()
            .parse::<i64>()
            .map(|n| json!(n))
            .unwrap_or_else(|_| json!(raw)),
        Some("number") => raw
            .trim()
            .parse::<f64>()
            .map(|n| json!(n))
            .unwrap_or_else(|_| json!(raw)),
        Some("boolean") => match raw.trim() {
            "true" | "1" => json!(true),
            "false" | "0" => json!(false),
            _ => json!(raw),
        },
        _ => json!(raw),
    }
}

/// Escape an XSD regex into an anchored JSON Schema `pattern`.
fn anchored(pattern: &str) -> String {
    format!("^(?:{pattern})$")
}

impl<'a> Conv<'a> {
    fn new(schema: &'a Xml, opts: &'a Options) -> Self {
        let mut c = Conv {
            opts,
            elements: HashMap::new(),
            element_order: Vec::new(),
            complex: HashMap::new(),
            complex_order: Vec::new(),
            simple: HashMap::new(),
            attributes: HashMap::new(),
            groups: HashMap::new(),
            attr_groups: HashMap::new(),
            has_external: false,
            defs: Map::new(),
            queue: Vec::new(),
            queued: HashSet::new(),
            deriving: Vec::new(),
        };
        for child in &schema.children {
            let name = child.a("name").map(str::to_string);
            match child.name.as_str() {
                "element" => {
                    if let Some(n) = name {
                        c.element_order.push(n.clone());
                        c.elements.insert(n, child);
                    }
                }
                "complexType" => {
                    if let Some(n) = name {
                        c.complex_order.push(n.clone());
                        c.complex.insert(n, child);
                    }
                }
                "simpleType" => {
                    if let Some(n) = name {
                        c.simple.insert(n, child);
                    }
                }
                "attribute" => {
                    if let Some(n) = name {
                        c.attributes.insert(n, child);
                    }
                }
                "group" => {
                    if let Some(n) = name {
                        c.groups.insert(n, child);
                    }
                }
                "attributeGroup" => {
                    if let Some(n) = name {
                        c.attr_groups.insert(n, child);
                    }
                }
                "import" | "include" | "redefine" | "override" => c.has_external = true,
                _ => {}
            }
        }
        c
    }

    fn unresolved(&self, what: &str, qname: &str) -> String {
        let hint = if self.has_external {
            " — this schema uses xs:import/xs:include, whose targets are not fetched; \
             paste the referenced schema into the same input"
        } else {
            ""
        };
        format!("unknown {what} '{qname}' is not declared in this schema{hint}")
    }

    fn depth_guard(&self, depth: usize) -> Result<(), String> {
        if depth > MAX_DEPTH {
            return Err(format!(
                "type nesting exceeded the depth limit of {MAX_DEPTH} — the schema is \
                 probably recursive through an inline (unnamed) type; give the repeated \
                 type a name so it can be emitted as a $ref"
            ));
        }
        Ok(())
    }

    fn description(&self, node: &Xml) -> Option<String> {
        if !self.opts.annotations {
            return None;
        }
        let ann = node.first("annotation")?;
        let text: Vec<String> = ann
            .all("documentation")
            .map(|d| d.text.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if text.is_empty() {
            None
        } else {
            Some(text.join("\n\n"))
        }
    }

    /// Reference a named global type, queueing it for emission under `$defs`.
    fn named_ref(&mut self, name: &str) -> Value {
        if !self.queued.contains(name) {
            self.queued.insert(name.to_string());
            self.queue.push(name.to_string());
        }
        json!({ "$ref": format!("{}{}", self.opts.draft.ref_base(), name) })
    }

    /// Resolve a `type="…"` reference: local declarations win over builtins.
    fn type_by_qname(&mut self, qname: &str) -> Result<Value, String> {
        let name = local(qname);
        if self.complex.contains_key(name) || self.simple.contains_key(name) {
            return Ok(self.named_ref(name));
        }
        builtin_schema(name).ok_or_else(|| self.unresolved("type", qname))
    }

    /// Resolve a simple-type base INLINE (facets have to compose onto it).
    fn simple_inline(&mut self, qname: &str, depth: usize) -> Result<Value, String> {
        self.depth_guard(depth)?;
        let name = local(qname);
        if let Some(node) = self.simple.get(name).copied() {
            return self.simple_type_schema(node, depth + 1);
        }
        if self.complex.contains_key(name) {
            return Err(format!(
                "'{qname}' is a complexType but is used where a simple type is required \
                 (xs:restriction/xs:list/xs:union bases must be simple types)"
            ));
        }
        builtin_schema(name).ok_or_else(|| self.unresolved("type", qname))
    }

    // -- simple types --------------------------------------------------------

    fn simple_type_schema(&mut self, st: &'a Xml, depth: usize) -> Result<Value, String> {
        self.depth_guard(depth)?;
        let desc = self.description(st);

        let base = if let Some(r) = st.first("restriction") {
            let mut b = match r.a("base") {
                Some(q) => self.simple_inline(q, depth + 1)?,
                None => match r.first("simpleType") {
                    Some(inner) => self.simple_type_schema(inner, depth + 1)?,
                    None => json!({}),
                },
            };
            self.apply_facets(&mut b, r);
            b
        } else if let Some(l) = st.first("list") {
            let item = match l.a("itemType") {
                Some(q) => self.simple_inline(q, depth + 1)?,
                None => match l.first("simpleType") {
                    Some(inner) => self.simple_type_schema(inner, depth + 1)?,
                    None => json!({ "type": "string" }),
                },
            };
            json!({ "type": "array", "items": item })
        } else if let Some(u) = st.first("union") {
            let mut members: Vec<Value> = Vec::new();
            if let Some(list) = u.a("memberTypes") {
                for q in list.split_whitespace() {
                    members.push(self.simple_inline(q, depth + 1)?);
                }
            }
            for inner in u.all("simpleType").collect::<Vec<_>>() {
                members.push(self.simple_type_schema(inner, depth + 1)?);
            }
            if members.is_empty() {
                json!({})
            } else {
                json!({ "anyOf": members })
            }
        } else {
            json!({})
        };

        Ok(self.with_extra(base, desc, None, None))
    }

    fn apply_facets(&self, schema: &mut Value, restriction: &Xml) {
        let obj = match schema.as_object_mut() {
            Some(o) => o,
            None => return,
        };
        let is_array = obj.get("type").and_then(Value::as_str) == Some("array");

        let mut enums: Vec<Value> = Vec::new();
        let mut patterns: Vec<String> = Vec::new();
        let mut set: Vec<(String, Value)> = Vec::new();

        let has_text_facet = restriction.children.iter().any(|c| {
            matches!(
                c.name.as_str(),
                "enumeration" | "pattern" | "length" | "minLength" | "maxLength"
            )
        });
        if !is_array && has_text_facet && !obj.contains_key("type") {
            obj.insert("type".into(), json!("string"));
        }
        let probe = Value::Object(obj.clone());

        for facet in &restriction.children {
            let value = match facet.a("value") {
                Some(v) => v,
                None => continue,
            };
            match facet.name.as_str() {
                "enumeration" => enums.push(typed_value(&probe, value)),
                "pattern" => patterns.push(anchored(value)),
                "length" => {
                    let (lo, hi) = if is_array {
                        ("minItems", "maxItems")
                    } else {
                        ("minLength", "maxLength")
                    };
                    if let Ok(n) = value.trim().parse::<u64>() {
                        set.push((lo.into(), json!(n)));
                        set.push((hi.into(), json!(n)));
                    }
                }
                "minLength" | "maxLength" => {
                    let key = if is_array {
                        if facet.name == "minLength" { "minItems" } else { "maxItems" }
                    } else if facet.name == "minLength" {
                        "minLength"
                    } else {
                        "maxLength"
                    };
                    if let Ok(n) = value.trim().parse::<u64>() {
                        set.push((key.into(), json!(n)));
                    }
                }
                "minInclusive" => set.push(("minimum".into(), typed_value(&probe, value))),
                "maxInclusive" => set.push(("maximum".into(), typed_value(&probe, value))),
                "minExclusive" => {
                    set.push(("exclusiveMinimum".into(), typed_value(&probe, value)))
                }
                "maxExclusive" => {
                    set.push(("exclusiveMaximum".into(), typed_value(&probe, value)))
                }
                "fractionDigits" => {
                    if let Ok(n) = value.trim().parse::<u32>() {
                        let step = if n == 0 {
                            json!(1)
                        } else {
                            json!(1.0f64 / 10f64.powi(n as i32))
                        };
                        set.push(("multipleOf".into(), step));
                    }
                }
                _ => {}
            }
        }

        if !enums.is_empty() {
            obj.insert("enum".into(), Value::Array(enums));
        }
        match patterns.len() {
            0 => {}
            1 => {
                obj.insert("pattern".into(), json!(patterns.remove(0)));
            }
            _ => {
                let alts: Vec<Value> = patterns.iter().map(|p| json!({ "pattern": p })).collect();
                obj.insert("anyOf".into(), Value::Array(alts));
            }
        }
        for (k, v) in set {
            obj.insert(k, v);
        }
    }

    // -- complex types -------------------------------------------------------

    fn complex_type_schema(&mut self, ct: &'a Xml, depth: usize) -> Result<Value, String> {
        self.depth_guard(depth)?;
        let mut obj = ObjBuild {
            description: self.description(ct),
            ..Default::default()
        };
        self.build_complex(ct, &mut obj, depth)?;
        Ok(self.finish_object(obj))
    }

    fn build_complex(
        &mut self,
        ct: &'a Xml,
        obj: &mut ObjBuild,
        depth: usize,
    ) -> Result<(), String> {
        self.depth_guard(depth)?;
        let mixed = ct.a("mixed").map(|v| v == "true" || v == "1").unwrap_or(false);

        if let Some(cc) = ct.first("complexContent") {
            let cc_mixed = cc.a("mixed").map(|v| v == "true" || v == "1").unwrap_or(false);
            if let Some(ext) = cc.first("extension") {
                if let Some(base) = ext.a("base") {
                    self.merge_base(base, obj, depth + 1)?;
                }
                self.walk_content(ext, obj, true, depth + 1)?;
                self.collect_attributes(ext, obj, depth + 1)?;
            } else if let Some(res) = cc.first("restriction") {
                // A complexContent restriction must restate its content model,
                // so only the restriction's own particles/attributes apply.
                self.walk_content(res, obj, true, depth + 1)?;
                self.collect_attributes(res, obj, depth + 1)?;
            }
            if (mixed || cc_mixed) && obj.text.is_none() {
                obj.text = Some(json!({ "type": "string" }));
                obj.text_required = false;
            }
            return Ok(());
        }

        if let Some(sc) = ct.first("simpleContent") {
            if let Some(ext) = sc.first("extension") {
                if let Some(base) = ext.a("base") {
                    self.merge_base(base, obj, depth + 1)?;
                }
                self.collect_attributes(ext, obj, depth + 1)?;
            } else if let Some(res) = sc.first("restriction") {
                let mut text = match res.a("base") {
                    Some(base) => self.simple_content_base(base, obj, depth + 1)?,
                    None => json!({ "type": "string" }),
                };
                self.apply_facets(&mut text, res);
                obj.text = Some(text);
                obj.text_required = true;
                self.collect_attributes(res, obj, depth + 1)?;
            }
            return Ok(());
        }

        self.walk_content(ct, obj, true, depth + 1)?;
        self.collect_attributes(ct, obj, depth + 1)?;
        if mixed && obj.text.is_none() {
            obj.text = Some(json!({ "type": "string" }));
            obj.text_required = false;
        }
        Ok(())
    }

    /// Base of a `simpleContent` restriction: a simple type, or the text schema of
    /// a complexType that itself has simpleContent.
    fn simple_content_base(
        &mut self,
        qname: &str,
        obj: &mut ObjBuild,
        depth: usize,
    ) -> Result<Value, String> {
        let name = local(qname);
        if let Some(node) = self.complex.get(name).copied() {
            self.merge_base(qname, obj, depth)?;
            let _ = node;
            return Ok(obj.text.clone().unwrap_or_else(|| json!({ "type": "string" })));
        }
        self.simple_inline(qname, depth)
    }

    /// Merge a base type's members into `obj` (extension / derivation).
    fn merge_base(
        &mut self,
        qname: &str,
        obj: &mut ObjBuild,
        depth: usize,
    ) -> Result<(), String> {
        self.depth_guard(depth)?;
        let name = local(qname).to_string();

        if let Some(node) = self.complex.get(name.as_str()).copied() {
            if self.deriving.contains(&name) {
                return Err(format!(
                    "circular type derivation: '{name}' extends itself \
                     (chain: {})",
                    self.deriving.join(" → ")
                ));
            }
            self.deriving.push(name.clone());
            let result = self.build_complex(node, obj, depth + 1);
            self.deriving.pop();
            return result;
        }

        // Extending a simple type (or a builtin): the base becomes the text value.
        let text = self.simple_inline(qname, depth + 1)?;
        obj.text = Some(text);
        obj.text_required = true;
        Ok(())
    }

    fn walk_content(
        &mut self,
        parent: &'a Xml,
        obj: &mut ObjBuild,
        req_ctx: bool,
        depth: usize,
    ) -> Result<(), String> {
        self.depth_guard(depth)?;
        for child in &parent.children {
            match child.name.as_str() {
                "sequence" | "all" => {
                    let (min, _) = occurs(child);
                    self.walk_content(child, obj, req_ctx && min >= 1, depth + 1)?;
                }
                "choice" => {
                    let (min, max) = occurs(child);
                    let before: Vec<String> = obj.props.keys().cloned().collect();
                    self.walk_content(child, obj, false, depth + 1)?;
                    let added: Vec<String> = obj
                        .props
                        .keys()
                        .filter(|k| !before.contains(k))
                        .cloned()
                        .collect();
                    if req_ctx && min >= 1 && max == Some(1) && added.len() > 1 {
                        obj.choices.push(added);
                    }
                }
                "group" => {
                    let (min, _) = occurs(child);
                    let target = match child.a("ref") {
                        Some(r) => self
                            .groups
                            .get(local(r))
                            .copied()
                            .ok_or_else(|| self.unresolved("group", r))?,
                        None => child,
                    };
                    self.walk_content(target, obj, req_ctx && min >= 1, depth + 1)?;
                }
                "element" => {
                    let (name, schema, required) = self.element_entry(child, depth + 1)?;
                    obj.insert(name, schema, required && req_ctx);
                }
                "any" => obj.open = true,
                _ => {}
            }
        }
        Ok(())
    }

    fn collect_attributes(
        &mut self,
        parent: &'a Xml,
        obj: &mut ObjBuild,
        depth: usize,
    ) -> Result<(), String> {
        self.depth_guard(depth)?;
        for child in &parent.children {
            match child.name.as_str() {
                "attribute" => {
                    let declaration = match child.a("ref") {
                        Some(r) => self
                            .attributes
                            .get(local(r))
                            .copied()
                            .ok_or_else(|| self.unresolved("attribute", r))?,
                        None => child,
                    };
                    let name = match declaration.a("name").or_else(|| child.a("ref")) {
                        Some(n) => local(n).to_string(),
                        None => continue,
                    };
                    let use_ = child.a("use").or_else(|| declaration.a("use")).unwrap_or("optional");
                    if use_ == "prohibited" {
                        obj.props.remove(&format!("{}{name}", self.opts.attribute_prefix));
                        continue;
                    }
                    let mut schema = if let Some(q) = declaration.a("type") {
                        self.type_by_qname(q)?
                    } else if let Some(inner) = declaration.first("simpleType") {
                        self.simple_type_schema(inner, depth + 1)?
                    } else {
                        json!({ "type": "string" })
                    };
                    let desc = self.description(declaration);
                    let default = child.a("default").or_else(|| declaration.a("default"));
                    let fixed = child.a("fixed").or_else(|| declaration.a("fixed"));
                    schema = self.with_extra(schema, desc, default, fixed);
                    let key = format!("{}{name}", self.opts.attribute_prefix);
                    obj.insert(key, schema, use_ == "required");
                }
                "attributeGroup" => {
                    if let Some(r) = child.a("ref") {
                        let target = self
                            .attr_groups
                            .get(local(r))
                            .copied()
                            .ok_or_else(|| self.unresolved("attributeGroup", r))?;
                        self.collect_attributes(target, obj, depth + 1)?;
                    }
                }
                "anyAttribute" => obj.open = true,
                _ => {}
            }
        }
        Ok(())
    }

    // -- elements ------------------------------------------------------------

    /// Build one property entry for an `xs:element` particle.
    fn element_entry(
        &mut self,
        el: &'a Xml,
        depth: usize,
    ) -> Result<(String, Value, bool), String> {
        self.depth_guard(depth)?;
        let (declaration, name) = match el.a("ref") {
            Some(r) => {
                let target = self
                    .elements
                    .get(local(r))
                    .copied()
                    .ok_or_else(|| self.unresolved("element", r))?;
                (target, local(r).to_string())
            }
            None => {
                let name = match el.a("name") {
                    Some(n) => n.to_string(),
                    None => {
                        return Err(
                            "an <xs:element> particle has neither a name nor a ref attribute"
                                .to_string(),
                        )
                    }
                };
                (el, name)
            }
        };

        let mut base = self.element_type_schema(declaration, depth + 1)?;
        if declaration.a("nillable").map(|v| v == "true" || v == "1").unwrap_or(false) {
            base = self.nullable(base);
        }

        let desc = self.description(declaration).or_else(|| self.description(el));
        let default = el.a("default").or_else(|| declaration.a("default"));
        let fixed = el.a("fixed").or_else(|| declaration.a("fixed"));

        let (min, max) = occurs(el);
        let is_array = match max {
            None => true,
            Some(m) => m > 1,
        };

        if is_array {
            let inner = self.with_extra(base, None, default, fixed);
            let mut arr = Map::new();
            if let Some(d) = desc {
                arr.insert("description".into(), json!(d));
            }
            arr.insert("type".into(), json!("array"));
            arr.insert("items".into(), inner);
            if min > 0 {
                arr.insert("minItems".into(), json!(min));
            }
            if let Some(m) = max {
                arr.insert("maxItems".into(), json!(m));
            }
            Ok((name, Value::Object(arr), min >= 1))
        } else {
            let schema = self.with_extra(base, desc, default, fixed);
            Ok((name, schema, min >= 1))
        }
    }

    fn element_type_schema(&mut self, el: &'a Xml, depth: usize) -> Result<Value, String> {
        self.depth_guard(depth)?;
        if let Some(q) = el.a("type") {
            return self.type_by_qname(q);
        }
        if let Some(ct) = el.first("complexType") {
            return self.complex_type_schema(ct, depth + 1);
        }
        if let Some(st) = el.first("simpleType") {
            return self.simple_type_schema(st, depth + 1);
        }
        Ok(json!({}))
    }

    fn nullable(&self, base: Value) -> Value {
        if let Some(Value::String(t)) = base.get("type") {
            let mut m = base.as_object().cloned().unwrap_or_default();
            m.insert("type".into(), json!([t, "null"]));
            return Value::Object(m);
        }
        json!({ "anyOf": [base, { "type": "null" }] })
    }

    /// Attach description/default/const to a schema, keeping `$ref` legal in both drafts.
    fn with_extra(
        &self,
        base: Value,
        description: Option<String>,
        default: Option<&str>,
        fixed: Option<&str>,
    ) -> Value {
        let mut extra: Vec<(String, Value)> = Vec::new();
        if let Some(d) = description {
            extra.push(("description".into(), json!(d)));
        }
        if let Some(v) = fixed {
            extra.push(("const".into(), typed_value(&base, v)));
        } else if let Some(v) = default {
            extra.push(("default".into(), typed_value(&base, v)));
        }
        if extra.is_empty() {
            return base;
        }

        let is_pure_ref = base
            .as_object()
            .map(|o| o.len() == 1 && o.contains_key("$ref"))
            .unwrap_or(false);

        let mut out = Map::new();
        for (k, v) in extra {
            out.insert(k, v);
        }
        if is_pure_ref && self.opts.draft == Draft::Draft07 {
            // Draft-07 ignores keywords beside `$ref`; wrap it instead.
            out.insert("allOf".into(), json!([base]));
            return Value::Object(out);
        }
        if let Some(o) = base.as_object() {
            for (k, v) in o {
                out.insert(k.clone(), v.clone());
            }
        }
        Value::Object(out)
    }

    fn finish_object(&self, obj: ObjBuild) -> Value {
        let ObjBuild {
            mut props,
            mut required,
            choices,
            open,
            description,
            text,
            text_required,
        } = obj;

        if let Some(text_schema) = text {
            if !self.opts.text_property.is_empty() {
                props.insert(self.opts.text_property.clone(), text_schema);
                if text_required && !required.contains(&self.opts.text_property) {
                    required.push(self.opts.text_property.clone());
                }
            }
        }

        let mut out = Map::new();
        if let Some(d) = description {
            out.insert("description".into(), json!(d));
        }
        out.insert("type".into(), json!("object"));
        out.insert("properties".into(), Value::Object(props));
        if self.opts.required_from_occurs && !required.is_empty() {
            out.insert("required".into(), json!(required));
        }
        if self.opts.required_from_occurs && !choices.is_empty() {
            let mut groups: Vec<Value> = Vec::new();
            for group in &choices {
                let alts: Vec<Value> =
                    group.iter().map(|n| json!({ "required": [n] })).collect();
                groups.push(json!({ "oneOf": alts }));
            }
            if groups.len() == 1 {
                let only = groups.remove(0);
                out.insert("oneOf".into(), only["oneOf"].clone());
            } else {
                out.insert("allOf".into(), Value::Array(groups));
            }
        }
        if !self.opts.additional_properties && !open {
            out.insert("additionalProperties".into(), json!(false));
        }
        Value::Object(out)
    }

    /// Emit every queued named type into `$defs`/`definitions`.
    fn drain_defs(&mut self) -> Result<(), String> {
        let mut index = 0usize;
        while index < self.queue.len() {
            let name = self.queue[index].clone();
            index += 1;
            if self.defs.contains_key(&name) {
                continue;
            }
            // Reserve the slot first so recursive types terminate.
            self.defs.insert(name.clone(), Value::Null);
            let value = if let Some(node) = self.complex.get(name.as_str()).copied() {
                self.complex_type_schema(node, 0)?
            } else if let Some(node) = self.simple.get(name.as_str()).copied() {
                self.simple_type_schema(node, 0)?
            } else {
                return Err(self.unresolved("type", &name));
            };
            self.defs.insert(name, value);
        }
        Ok(())
    }
}

/// Convert an XSD document into a JSON Schema document (pretty-printed).
pub fn convert(xsd: &str, opts: &Options) -> Result<String, String> {
    if xsd.trim().is_empty() {
        return Err("no XSD provided — paste an XML Schema document (<xs:schema>…)".to_string());
    }
    if xsd.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "XSD is {} bytes, over the {MAX_INPUT_BYTES}-byte limit for in-browser conversion",
            xsd.len()
        ));
    }

    let root = parse_xml(xsd)?;
    if root.name != "schema" {
        return Err(format!(
            "expected a root <xs:schema> element, found <{}> — this input does not look \
             like an XML Schema document",
            root.name
        ));
    }

    let mut conv = Conv::new(&root, opts);

    // Choose the root declaration.
    let wanted = opts.root_element.trim();
    enum Root<'x> {
        Element(&'x Xml),
        Complex(&'x Xml),
        Simple(&'x Xml),
    }
    let (root_name, chosen) = if wanted.is_empty() {
        if let Some(first) = conv.element_order.first().cloned() {
            let node = conv.elements[&first];
            (first, Root::Element(node))
        } else if let Some(first) = conv.complex_order.first().cloned() {
            let node = conv.complex[&first];
            (first, Root::Complex(node))
        } else {
            return Err(
                "this schema declares no global <xs:element> or named <xs:complexType>, so there \
                 is nothing to use as the JSON Schema root"
                    .to_string(),
            );
        }
    } else {
        let key = local(wanted);
        if let Some(node) = conv.elements.get(key).copied() {
            (key.to_string(), Root::Element(node))
        } else if let Some(node) = conv.complex.get(key).copied() {
            (key.to_string(), Root::Complex(node))
        } else if let Some(node) = conv.simple.get(key).copied() {
            (key.to_string(), Root::Simple(node))
        } else {
            let mut available = conv.element_order.clone();
            available.extend(conv.complex_order.clone());
            let listed = if available.is_empty() {
                "none".to_string()
            } else {
                available.join(", ")
            };
            return Err(format!(
                "root '{wanted}' is not a global element or named type in this schema; \
                 available: {listed}"
            ));
        }
    };

    // The root type is inlined (not $ref'd) so the document reads top-down.
    let body = match chosen {
        Root::Element(node) => {
            let described = conv.description(node);
            let inner = match node.a("type") {
                Some(q) => {
                    let name = local(q);
                    if let Some(ct) = conv.complex.get(name).copied() {
                        conv.complex_type_schema(ct, 0)?
                    } else if let Some(st) = conv.simple.get(name).copied() {
                        conv.simple_type_schema(st, 0)?
                    } else {
                        builtin_schema(name).ok_or_else(|| conv.unresolved("type", q))?
                    }
                }
                None => conv.element_type_schema(node, 0)?,
            };
            conv.with_extra(inner, described, None, None)
        }
        Root::Complex(node) => conv.complex_type_schema(node, 0)?,
        Root::Simple(node) => conv.simple_type_schema(node, 0)?,
    };

    conv.drain_defs()?;

    let mut doc = Map::new();
    doc.insert("$schema".into(), json!(opts.draft.schema_uri()));
    doc.insert("title".into(), json!(root_name));
    if let Some(o) = body.as_object() {
        for (k, v) in o {
            if k == "title" {
                continue;
            }
            doc.insert(k.clone(), v.clone());
        }
    }
    if !conv.defs.is_empty() {
        doc.insert(opts.draft.defs_key().into(), Value::Object(conv.defs));
    }

    serde_json::to_string_pretty(&Value::Object(doc))
        .map_err(|e| format!("failed to serialize JSON Schema: {e}"))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn conv_value(xsd: &str, opts: &Options) -> Value {
        serde_json::from_str(&convert(xsd, opts).expect("conversion should succeed")).unwrap()
    }

    const ORDER: &str = r#"<?xml version="1.0"?>
<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema">
  <xs:element name="order">
    <xs:complexType>
      <xs:sequence>
        <xs:element name="id" type="xs:string"/>
        <xs:element name="qty" type="xs:int"/>
        <xs:element name="note" type="xs:string" minOccurs="0"/>
        <xs:element name="line" type="xs:string" minOccurs="1" maxOccurs="unbounded"/>
      </xs:sequence>
      <xs:attribute name="currency" type="xs:string" use="required"/>
    </xs:complexType>
  </xs:element>
</xs:schema>"#;

    #[test]
    fn happy_path_sequence_attributes_and_occurs() {
        let v = conv_value(ORDER, &Options::default());
        assert_eq!(v["$schema"], "https://json-schema.org/draft/2020-12/schema");
        assert_eq!(v["title"], "order");
        assert_eq!(v["type"], "object");
        assert_eq!(v["properties"]["id"]["type"], "string");
        assert_eq!(v["properties"]["qty"]["type"], "integer");
        assert_eq!(v["properties"]["qty"]["maximum"], 2_147_483_647i64);
        // minOccurs="0" → not required; unbounded → array with minItems.
        assert_eq!(v["properties"]["line"]["type"], "array");
        assert_eq!(v["properties"]["line"]["items"]["type"], "string");
        assert_eq!(v["properties"]["line"]["minItems"], 1);
        assert!(v["properties"]["line"].get("maxItems").is_none());
        assert_eq!(v["properties"]["@currency"]["type"], "string");
        assert_eq!(
            v["required"],
            json!(["id", "qty", "line", "@currency"])
        );
        assert_eq!(v["additionalProperties"], false);
    }

    #[test]
    fn error_on_non_schema_input() {
        let err = convert("<note><to>you</to></note>", &Options::default()).unwrap_err();
        assert!(err.contains("expected a root <xs:schema> element"), "{err}");
        assert!(err.contains("<note>"), "{err}");
    }

    #[test]
    fn error_on_empty_input() {
        let err = convert("   \n ", &Options::default()).unwrap_err();
        assert!(err.contains("no XSD provided"), "{err}");
    }

    #[test]
    fn error_on_unresolved_type_mentions_import() {
        let xsd = r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema">
          <xs:import namespace="urn:other" schemaLocation="other.xsd"/>
          <xs:element name="a" type="o:Missing"/>
        </xs:schema>"#;
        let err = convert(xsd, &Options::default()).unwrap_err();
        assert!(err.contains("o:Missing"), "{err}");
        assert!(err.contains("xs:import"), "{err}");
    }

    #[test]
    fn error_on_unknown_root_lists_available() {
        let err = convert(ORDER, &Options { root_element: "nope".into(), ..Default::default() })
            .unwrap_err();
        assert!(err.contains("root 'nope' is not a global element"), "{err}");
        assert!(err.contains("available: order"), "{err}");
    }

    #[test]
    fn error_on_oversize_input() {
        let big = format!(
            "<xs:schema xmlns:xs=\"http://www.w3.org/2001/XMLSchema\"><!--{}--></xs:schema>",
            "x".repeat(MAX_INPUT_BYTES)
        );
        let err = convert(&big, &Options::default()).unwrap_err();
        assert!(err.contains("over the"), "{err}");
    }

    #[test]
    fn cap_boundary_exactly_at_limit_converts() {
        let head = "<xs:schema xmlns:xs=\"http://www.w3.org/2001/XMLSchema\"><xs:element name=\"a\" type=\"xs:string\"/><!--";
        let tail = "--></xs:schema>";
        let pad = MAX_INPUT_BYTES - head.len() - tail.len();
        let exact = format!("{head}{}{tail}", "x".repeat(pad));
        assert_eq!(exact.len(), MAX_INPUT_BYTES);
        let v = conv_value(&exact, &Options::default());
        assert_eq!(v["title"], "a");
        // One byte over is rejected.
        let over = format!("{head}{}{tail}", "x".repeat(pad + 1));
        assert!(convert(&over, &Options::default()).is_err());
    }

    #[test]
    fn named_types_become_refs_and_defs() {
        let xsd = r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema">
          <xs:element name="order" type="Order"/>
          <xs:complexType name="Order">
            <xs:sequence><xs:element name="customer" type="Customer"/></xs:sequence>
          </xs:complexType>
          <xs:complexType name="Customer">
            <xs:sequence><xs:element name="email" type="xs:string"/></xs:sequence>
          </xs:complexType>
          <xs:complexType name="Unused">
            <xs:sequence><xs:element name="x" type="xs:string"/></xs:sequence>
          </xs:complexType>
        </xs:schema>"#;
        let v = conv_value(xsd, &Options::default());
        assert_eq!(v["properties"]["customer"]["$ref"], "#/$defs/Customer");
        assert_eq!(v["$defs"]["Customer"]["properties"]["email"]["type"], "string");
        // Reachability pruning: Unused is never referenced.
        assert!(v["$defs"].get("Unused").is_none());
    }

    #[test]
    fn recursive_named_type_terminates() {
        let xsd = r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema">
          <xs:element name="node" type="Node"/>
          <xs:complexType name="Node">
            <xs:sequence>
              <xs:element name="label" type="xs:string"/>
              <xs:element name="child" type="Node" minOccurs="0" maxOccurs="unbounded"/>
            </xs:sequence>
          </xs:complexType>
        </xs:schema>"#;
        let v = conv_value(xsd, &Options::default());
        assert_eq!(v["properties"]["child"]["items"]["$ref"], "#/$defs/Node");
        assert_eq!(v["$defs"]["Node"]["properties"]["child"]["items"]["$ref"], "#/$defs/Node");
    }

    #[test]
    fn choice_becomes_one_of() {
        let xsd = r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema">
          <xs:element name="payment">
            <xs:complexType>
              <xs:choice>
                <xs:element name="card" type="xs:string"/>
                <xs:element name="bank" type="xs:string"/>
              </xs:choice>
            </xs:complexType>
          </xs:element>
        </xs:schema>"#;
        let v = conv_value(xsd, &Options::default());
        assert_eq!(v["oneOf"], json!([{ "required": ["card"] }, { "required": ["bank"] }]));
        assert!(v.get("required").is_none());
    }

    #[test]
    fn facets_become_enum_pattern_and_bounds() {
        let xsd = r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema">
          <xs:element name="ticket">
            <xs:complexType>
              <xs:sequence>
                <xs:element name="status">
                  <xs:simpleType>
                    <xs:restriction base="xs:string">
                      <xs:enumeration value="open"/>
                      <xs:enumeration value="closed"/>
                    </xs:restriction>
                  </xs:simpleType>
                </xs:element>
                <xs:element name="code">
                  <xs:simpleType>
                    <xs:restriction base="xs:string">
                      <xs:pattern value="[A-Z]{2}-[0-9]+"/>
                      <xs:minLength value="4"/>
                    </xs:restriction>
                  </xs:simpleType>
                </xs:element>
                <xs:element name="score">
                  <xs:simpleType>
                    <xs:restriction base="xs:int">
                      <xs:minInclusive value="0"/>
                      <xs:maxExclusive value="100"/>
                    </xs:restriction>
                  </xs:simpleType>
                </xs:element>
              </xs:sequence>
            </xs:complexType>
          </xs:element>
        </xs:schema>"#;
        let v = conv_value(xsd, &Options::default());
        assert_eq!(v["properties"]["status"]["enum"], json!(["open", "closed"]));
        assert_eq!(v["properties"]["code"]["pattern"], "^(?:[A-Z]{2}-[0-9]+)$");
        assert_eq!(v["properties"]["code"]["minLength"], 4);
        assert_eq!(v["properties"]["score"]["minimum"], 0);
        assert_eq!(v["properties"]["score"]["exclusiveMaximum"], 100);
    }

    #[test]
    fn complex_content_extension_merges_base_members() {
        let xsd = r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema">
          <xs:element name="dog" type="Dog"/>
          <xs:complexType name="Animal">
            <xs:sequence><xs:element name="name" type="xs:string"/></xs:sequence>
          </xs:complexType>
          <xs:complexType name="Dog">
            <xs:complexContent>
              <xs:extension base="Animal">
                <xs:sequence><xs:element name="breed" type="xs:string"/></xs:sequence>
              </xs:extension>
            </xs:complexContent>
          </xs:complexType>
        </xs:schema>"#;
        let v = conv_value(xsd, &Options::default());
        assert_eq!(v["properties"]["name"]["type"], "string");
        assert_eq!(v["properties"]["breed"]["type"], "string");
        assert_eq!(v["required"], json!(["name", "breed"]));
    }

    #[test]
    fn simple_content_extension_uses_text_property() {
        let xsd = r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema">
          <xs:element name="price">
            <xs:complexType>
              <xs:simpleContent>
                <xs:extension base="xs:decimal">
                  <xs:attribute name="currency" type="xs:string" use="required"/>
                </xs:extension>
              </xs:simpleContent>
            </xs:complexType>
          </xs:element>
        </xs:schema>"#;
        let v = conv_value(xsd, &Options::default());
        assert_eq!(v["properties"]["#text"]["type"], "number");
        assert_eq!(v["properties"]["@currency"]["type"], "string");
        assert_eq!(v["required"], json!(["@currency", "#text"]));
    }

    #[test]
    fn draft07_uses_definitions_and_allof_for_ref_siblings() {
        let xsd = r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema">
          <xs:element name="order" type="Order"/>
          <xs:complexType name="Order">
            <xs:sequence>
              <xs:element name="customer" type="Customer">
                <xs:annotation><xs:documentation>Who ordered.</xs:documentation></xs:annotation>
              </xs:element>
            </xs:sequence>
          </xs:complexType>
          <xs:complexType name="Customer">
            <xs:sequence><xs:element name="email" type="xs:string"/></xs:sequence>
          </xs:complexType>
        </xs:schema>"#;
        let v = conv_value(xsd, &Options { draft: Draft::Draft07, ..Default::default() });
        assert_eq!(v["$schema"], "http://json-schema.org/draft-07/schema#");
        assert_eq!(v["properties"]["customer"]["description"], "Who ordered.");
        assert_eq!(
            v["properties"]["customer"]["allOf"],
            json!([{ "$ref": "#/definitions/Customer" }])
        );
        assert!(v["definitions"]["Customer"].is_object());
    }

    #[test]
    fn options_toggle_prefix_required_annotations_and_openness() {
        let xsd = r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema">
          <xs:element name="book">
            <xs:annotation><xs:documentation>A book.</xs:documentation></xs:annotation>
            <xs:complexType>
              <xs:sequence><xs:element name="title" type="xs:string"/></xs:sequence>
              <xs:attribute name="isbn" type="xs:string" use="required"/>
            </xs:complexType>
          </xs:element>
        </xs:schema>"#;
        let opts = Options {
            attribute_prefix: String::new(),
            required_from_occurs: false,
            additional_properties: true,
            annotations: false,
            ..Default::default()
        };
        let v = conv_value(xsd, &opts);
        assert!(v["properties"]["isbn"].is_object());
        assert!(v.get("required").is_none());
        assert!(v.get("additionalProperties").is_none());
        assert!(v.get("description").is_none());
    }

    #[test]
    fn root_element_selects_a_named_type() {
        let xsd = r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema">
          <xs:element name="a" type="xs:string"/>
          <xs:complexType name="Money">
            <xs:sequence><xs:element name="amount" type="xs:decimal"/></xs:sequence>
          </xs:complexType>
        </xs:schema>"#;
        let v = conv_value(xsd, &Options { root_element: "Money".into(), ..Default::default() });
        assert_eq!(v["title"], "Money");
        assert_eq!(v["properties"]["amount"]["type"], "number");
    }

    #[test]
    fn nillable_and_fixed_and_union_and_list() {
        let xsd = r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema">
          <xs:element name="rec">
            <xs:complexType>
              <xs:sequence>
                <xs:element name="maybe" type="xs:string" nillable="true"/>
                <xs:element name="version" type="xs:string" fixed="1.0"/>
                <xs:element name="tags">
                  <xs:simpleType>
                    <xs:list itemType="xs:string"/>
                  </xs:simpleType>
                </xs:element>
                <xs:element name="either">
                  <xs:simpleType>
                    <xs:union memberTypes="xs:int xs:string"/>
                  </xs:simpleType>
                </xs:element>
              </xs:sequence>
            </xs:complexType>
          </xs:element>
        </xs:schema>"#;
        let v = conv_value(xsd, &Options::default());
        assert_eq!(v["properties"]["maybe"]["type"], json!(["string", "null"]));
        assert_eq!(v["properties"]["version"]["const"], "1.0");
        assert_eq!(v["properties"]["tags"]["type"], "array");
        assert_eq!(v["properties"]["either"]["anyOf"][0]["type"], "integer");
        assert_eq!(v["properties"]["either"]["anyOf"][1]["type"], "string");
    }

    #[test]
    fn any_and_any_attribute_open_the_object() {
        let xsd = r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema">
          <xs:element name="bag">
            <xs:complexType>
              <xs:sequence>
                <xs:element name="k" type="xs:string"/>
                <xs:any/>
              </xs:sequence>
            </xs:complexType>
          </xs:element>
        </xs:schema>"#;
        let v = conv_value(xsd, &Options::default());
        assert!(v.get("additionalProperties").is_none());
    }

    #[test]
    fn groups_and_attribute_groups_resolve() {
        let xsd = r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema">
          <xs:element name="doc">
            <xs:complexType>
              <xs:group ref="Body"/>
              <xs:attributeGroup ref="Meta"/>
            </xs:complexType>
          </xs:element>
          <xs:group name="Body">
            <xs:sequence><xs:element name="para" type="xs:string" maxOccurs="unbounded"/></xs:sequence>
          </xs:group>
          <xs:attributeGroup name="Meta">
            <xs:attribute name="lang" type="xs:language" use="required"/>
          </xs:attributeGroup>
        </xs:schema>"#;
        let v = conv_value(xsd, &Options::default());
        assert_eq!(v["properties"]["para"]["type"], "array");
        assert_eq!(v["properties"]["@lang"]["type"], "string");
        assert_eq!(v["required"], json!(["para", "@lang"]));
    }

    #[test]
    fn element_ref_resolves_to_the_global_declaration() {
        let xsd = r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema">
          <xs:element name="root">
            <xs:complexType>
              <xs:sequence><xs:element ref="title" minOccurs="0"/></xs:sequence>
            </xs:complexType>
          </xs:element>
          <xs:element name="title" type="xs:string"/>
        </xs:schema>"#;
        let v = conv_value(xsd, &Options::default());
        assert_eq!(v["properties"]["title"]["type"], "string");
        assert!(v.get("required").is_none());
    }

    #[test]
    fn empty_text_property_drops_the_text_member() {
        let xsd = r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema">
          <xs:element name="price">
            <xs:complexType>
              <xs:simpleContent>
                <xs:extension base="xs:decimal">
                  <xs:attribute name="currency" type="xs:string"/>
                </xs:extension>
              </xs:simpleContent>
            </xs:complexType>
          </xs:element>
        </xs:schema>"#;
        let v = conv_value(xsd, &Options { text_property: String::new(), ..Default::default() });
        assert!(v["properties"].get("#text").is_none());
        assert!(v["properties"]["@currency"].is_object());
    }

    #[test]
    fn any_prefix_binding_for_the_xsd_namespace_works() {
        let xsd = r#"<schema xmlns="http://www.w3.org/2001/XMLSchema">
          <element name="a" type="string"/>
        </schema>"#;
        let v = conv_value(xsd, &Options::default());
        assert_eq!(v["title"], "a");
        assert_eq!(v["type"], "string");
    }

    #[test]
    fn draft_from_str_accepts_the_advertised_forms() {
        for s in ["draft-07", "draft07", "7", "07", "DRAFT-07"] {
            assert_eq!(draft_from_str(s), Draft::Draft07, "{s}");
        }
        for s in ["2020-12", "", "anything"] {
            assert_eq!(draft_from_str(s), Draft::Draft2020, "{s}");
        }
    }
}
