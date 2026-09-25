//! Infer an XSD (W3C XML Schema) document from one sample XML document.
//!
//! A streaming quick-xml pass builds a small DOM; a second pass merges every element that
//! shares a qualified name into one model (children with occurrence bounds, attributes with
//! presence counts, observed text values); the emitter renders that model as XSD in one of the
//! three classic layouts: Venetian Blind, Salami Slice or Russian Doll.
//!
//! Everything here is deterministic and allocation-only — no clock, no I/O — so the same logic
//! backs the chat block, the CLI and the browser page.

use quick_xml::events::{BytesStart, Event};
use quick_xml::reader::Reader;

/// Largest sample document accepted, in bytes.
pub const MAX_INPUT_BYTES: usize = 1_000_000;
/// Largest element nesting depth accepted.
pub const MAX_DEPTH: usize = 256;
/// Largest indent accepted, in spaces per level.
pub const MAX_INDENT: usize = 8;

const XSI_NS: &str = "http://www.w3.org/2001/XMLSchema-instance";
const XSD_NS: &str = "http://www.w3.org/2001/XMLSchema";

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// Where element declarations and type definitions live in the generated schema.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Design {
    /// One global element; every nested element and type declared inline beneath it.
    RussianDoll,
    /// Every distinct element global with an inline type; parents use `ref=`.
    SalamiSlice,
    /// One global element; every complex type global and named; children use `type=`.
    VenetianBlind,
}

/// How element and attribute text is typed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TypeInference {
    /// Recognise booleans, integers, decimals, doubles, dates, times and URIs.
    Smart,
    /// Declare every simple value as `xs:string`.
    StringOnly,
}

/// How tightly occurrence bounds follow the sample.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Occurrence {
    /// Bounds follow the sample: always-present stays required, repeats become unbounded.
    Restricted,
    /// Everything optional and repeatable; attributes never required.
    Relaxed,
}

/// Generation options. `Default` matches the descriptor defaults.
#[derive(Clone, Debug)]
pub struct Options {
    pub design: Design,
    pub type_inference: TypeInference,
    pub occurrence: Occurrence,
    /// Distinct-value cap for inferring `xs:enumeration` facets. 0 disables enumeration inference.
    pub enumerations: usize,
    /// Overrides the target namespace. Empty = use the sample root's namespace (possibly none).
    pub target_namespace: String,
    /// Spaces per indent level, 0..=[`MAX_INDENT`].
    pub indent: usize,
    /// Emit the `<?xml …?>` declaration ahead of `<xs:schema>`.
    pub declaration: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            design: Design::VenetianBlind,
            type_inference: TypeInference::Smart,
            occurrence: Occurrence::Restricted,
            enumerations: 0,
            target_namespace: String::new(),
            indent: 2,
            declaration: true,
        }
    }
}

/// Parses the `design` parameter; unknown values fall back to the default (venetian-blind).
pub fn design_from_str(s: &str) -> Design {
    match s.trim().to_ascii_lowercase().as_str() {
        "russian-doll" | "russian_doll" | "rd" => Design::RussianDoll,
        "salami-slice" | "salami_slice" | "ss" => Design::SalamiSlice,
        _ => Design::VenetianBlind,
    }
}

/// Parses the `type_inference` parameter; unknown values fall back to the default (smart).
pub fn type_inference_from_str(s: &str) -> TypeInference {
    match s.trim().to_ascii_lowercase().as_str() {
        "string" | "string-only" => TypeInference::StringOnly,
        _ => TypeInference::Smart,
    }
}

/// Parses the `occurrence` parameter; unknown values fall back to the default (restricted).
pub fn occurrence_from_str(s: &str) -> Occurrence {
    match s.trim().to_ascii_lowercase().as_str() {
        "relaxed" => Occurrence::Relaxed,
        _ => Occurrence::Restricted,
    }
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct Node {
    ns: String,
    local: String,
    /// Non-namespace attributes: (local name, value). `xmlns*` and `xsi:*` are excluded.
    attrs: Vec<(String, String)>,
    nil: bool,
    children: Vec<Node>,
    text: String,
}

/// One lexical namespace scope: (prefix, uri). The default namespace uses the empty prefix.
type Scope = Vec<(String, String)>;

fn split_qname(q: &str) -> (&str, &str) {
    match q.split_once(':') {
        Some((p, l)) => (p, l),
        None => ("", q),
    }
}

fn resolve(scopes: &[Scope], prefix: &str) -> String {
    for frame in scopes.iter().rev() {
        for (p, uri) in frame.iter().rev() {
            if p == prefix {
                return uri.clone();
            }
        }
    }
    String::new()
}

/// Builds a node from a start/empty tag, pushing its namespace frame onto `scopes`.
/// The caller pops the frame (immediately for an empty tag, at the matching end tag otherwise).
fn node_from(e: &BytesStart<'_>, scopes: &mut Vec<Scope>) -> Result<Node, String> {
    let mut frame: Scope = Vec::new();
    let mut raw: Vec<(String, String)> = Vec::new();
    for attr in e.attributes() {
        let attr = attr.map_err(|err| format!("XML attribute parse error: {err}"))?;
        let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
        let value = attr
            .unescape_value()
            .map_err(|err| format!("XML attribute decode error in '{key}': {err}"))?
            .to_string();
        if key == "xmlns" {
            frame.push((String::new(), value));
        } else if let Some(p) = key.strip_prefix("xmlns:") {
            frame.push((p.to_string(), value));
        } else {
            raw.push((key, value));
        }
    }
    scopes.push(frame);

    let full = String::from_utf8_lossy(e.name().as_ref()).to_string();
    let (prefix, local) = split_qname(&full);
    if local.is_empty() {
        return Err(format!("XML parse error: element '<{full}>' has an empty local name"));
    }
    let ns = resolve(scopes, prefix);

    let mut attrs: Vec<(String, String)> = Vec::new();
    let mut nil = false;
    for (key, value) in raw {
        let (ap, al) = split_qname(&key);
        // Per the XML namespaces spec an unprefixed attribute is in no namespace, even when a
        // default namespace is in scope — so only prefixed attributes get resolved.
        let ans = if ap.is_empty() { String::new() } else { resolve(scopes, ap) };
        if ans == XSI_NS {
            if al == "nil" && matches!(value.trim(), "true" | "1") {
                nil = true;
            }
            continue;
        }
        if !attrs.iter().any(|(n, _)| n == al) {
            attrs.push((al.to_string(), value));
        }
    }

    Ok(Node { ns, local: local.to_string(), attrs, nil, children: Vec::new(), text: String::new() })
}

fn parse_document(src: &str) -> Result<Node, String> {
    let mut reader = Reader::from_str(src);
    {
        let config = reader.config_mut();
        config.trim_text(false);
        config.expand_empty_elements = false;
    }

    let mut stack: Vec<Node> = Vec::new();
    let mut scopes: Vec<Scope> = Vec::new();
    let mut root: Option<Node> = None;
    let mut buf = Vec::new();

    loop {
        let event = reader
            .read_event_into(&mut buf)
            .map_err(|e| format!("XML parse error at byte {}: {e}", reader.buffer_position()))?;
        match event {
            Event::Eof => break,
            Event::Start(e) => {
                let node = node_from(&e, &mut scopes)?;
                stack.push(node);
                if stack.len() > MAX_DEPTH {
                    return Err(format!(
                        "XML nests deeper than the {MAX_DEPTH}-element limit; \
                         trim the sample to a representative excerpt"
                    ));
                }
            }
            Event::Empty(e) => {
                let node = node_from(&e, &mut scopes)?;
                scopes.pop();
                match stack.last_mut() {
                    Some(parent) => parent.children.push(node),
                    None => {
                        if root.is_some() {
                            return Err(trailing_root_err(&node.local));
                        }
                        root = Some(node);
                    }
                }
            }
            Event::End(_) => {
                scopes.pop();
                let node = stack
                    .pop()
                    .ok_or_else(|| "XML parse error: a closing tag has no matching opening tag".to_string())?;
                match stack.last_mut() {
                    Some(parent) => parent.children.push(node),
                    None => {
                        if root.is_some() {
                            return Err(trailing_root_err(&node.local));
                        }
                        root = Some(node);
                    }
                }
            }
            Event::Text(t) => {
                if let Some(parent) = stack.last_mut() {
                    let s = t.unescape().map_err(|e| format!("XML text decode error: {e}"))?;
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

    if !stack.is_empty() {
        let open = stack.last().map(|n| n.local.clone()).unwrap_or_default();
        return Err(format!("XML parse error: '<{open}>' is never closed"));
    }

    root.ok_or_else(|| {
        "no XML elements found — expected a sample XML document, e.g. \
         '<order id=\"1\"><item>pen</item></order>'"
            .to_string()
    })
}

fn trailing_root_err(local: &str) -> String {
    format!(
        "XML parse error: '<{local}>' is a second top-level element — an XML document needs \
         exactly one root; wrap the samples in a single parent element"
    )
}

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
struct AttrModel {
    present: usize,
    values: Vec<String>,
}

#[derive(Debug)]
struct ChildStat {
    min: usize,
    max: usize,
}

#[derive(Debug)]
struct ElemModel {
    local: String,
    instances: usize,
    child_order: Vec<String>,
    child_stats: Vec<ChildStat>,
    attr_order: Vec<String>,
    attr_models: Vec<AttrModel>,
    text_values: Vec<String>,
    any_text: bool,
    has_children: bool,
    has_attrs: bool,
    /// False once any instance showed its children in a different order than first seen.
    ordered: bool,
    nillable: bool,
    foreign_children: bool,
}

impl ElemModel {
    fn new(local: &str) -> Self {
        ElemModel {
            local: local.to_string(),
            instances: 0,
            child_order: Vec::new(),
            child_stats: Vec::new(),
            attr_order: Vec::new(),
            attr_models: Vec::new(),
            text_values: Vec::new(),
            any_text: false,
            has_children: false,
            has_attrs: false,
            ordered: true,
            nillable: false,
            foreign_children: false,
        }
    }
}

#[derive(Debug, Default)]
struct Models {
    keys: Vec<String>,
    items: Vec<ElemModel>,
}

impl Models {
    fn index_of(&self, key: &str) -> Option<usize> {
        self.keys.iter().position(|k| k == key)
    }
    fn ensure(&mut self, key: &str, local: &str) -> usize {
        match self.index_of(key) {
            Some(i) => i,
            None => {
                self.keys.push(key.to_string());
                self.items.push(ElemModel::new(local));
                self.keys.len() - 1
            }
        }
    }
}

fn qkey(ns: &str, local: &str) -> String {
    format!("{{{ns}}}{local}")
}

fn collect(node: &Node, doc_ns: &str, models: &mut Models) {
    let key = qkey(&node.ns, &node.local);
    let i = models.ensure(&key, &node.local);

    {
        let m = &mut models.items[i];
        let before = m.instances;
        m.instances += 1;
        if node.nil {
            m.nillable = true;
        }
        let trimmed = node.text.trim();
        if !trimmed.is_empty() {
            m.any_text = true;
        }
        if !node.children.is_empty() {
            m.has_children = true;
        }
        if !node.attrs.is_empty() {
            m.has_attrs = true;
        }
        if node.children.is_empty() {
            m.text_values.push(trimmed.to_string());
        }

        let mut counts: Vec<usize> = vec![0; m.child_order.len()];
        let mut last_idx: Option<usize> = None;
        for c in &node.children {
            if c.ns != doc_ns {
                m.foreign_children = true;
                continue;
            }
            let ck = qkey(&c.ns, &c.local);
            let idx = match m.child_order.iter().position(|k| *k == ck) {
                Some(x) => x,
                None => {
                    m.child_order.push(ck);
                    // A child first seen after earlier instances was absent from all of them.
                    m.child_stats.push(ChildStat {
                        min: if before > 0 { 0 } else { usize::MAX },
                        max: 0,
                    });
                    counts.push(0);
                    m.child_order.len() - 1
                }
            };
            counts[idx] += 1;
            if let Some(l) = last_idx {
                if idx < l {
                    m.ordered = false;
                }
            }
            last_idx = Some(idx);
        }
        for (idx, st) in m.child_stats.iter_mut().enumerate() {
            let c = counts[idx];
            st.min = if st.min == usize::MAX { c } else { st.min.min(c) };
            st.max = st.max.max(c);
        }

        for (an, av) in &node.attrs {
            let ai = match m.attr_order.iter().position(|k| k == an) {
                Some(x) => x,
                None => {
                    m.attr_order.push(an.clone());
                    m.attr_models.push(AttrModel::default());
                    m.attr_order.len() - 1
                }
            };
            m.attr_models[ai].present += 1;
            m.attr_models[ai].values.push(av.clone());
        }
    }

    for c in &node.children {
        if c.ns == doc_ns {
            collect(c, doc_ns, models);
        }
    }
}

// ---------------------------------------------------------------------------
// Type inference
// ---------------------------------------------------------------------------

fn is_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

fn unsigned_int_ok(b: &str) -> bool {
    // Leading zeros mean a code, not a number (ZIP codes, product codes) — keep them text.
    is_digits(b) && !(b.len() > 1 && b.starts_with('0'))
}

fn unsigned_decimal_ok(b: &str) -> bool {
    match b.split_once('.') {
        Some((i, f)) => {
            let digits_ok = i.bytes().all(|c| c.is_ascii_digit()) && f.bytes().all(|c| c.is_ascii_digit());
            let has_digit = !i.is_empty() || !f.is_empty();
            let no_pad = !(i.len() > 1 && i.starts_with('0'));
            digits_ok && has_digit && no_pad
        }
        None => false,
    }
}

fn unsigned(s: &str) -> &str {
    s.strip_prefix(['+', '-']).unwrap_or(s)
}

fn looks_int(s: &str) -> bool {
    unsigned_int_ok(unsigned(s))
}

fn looks_decimal(s: &str) -> bool {
    unsigned_decimal_ok(unsigned(s))
}

fn looks_double(s: &str) -> bool {
    if matches!(s, "INF" | "+INF" | "-INF" | "NaN") {
        return true;
    }
    let lower = s.to_ascii_lowercase();
    match lower.split_once('e') {
        Some((mant, exp)) => {
            let m = unsigned(mant);
            (unsigned_int_ok(m) || unsigned_decimal_ok(m)) && is_digits(unsigned(exp))
        }
        None => false,
    }
}

fn two_digits(b: &[u8], i: usize) -> bool {
    b.len() > i + 1 && b[i].is_ascii_digit() && b[i + 1].is_ascii_digit()
}

fn num2(b: &[u8], i: usize) -> u32 {
    (b[i] - b'0') as u32 * 10 + (b[i + 1] - b'0') as u32
}

/// Consumes a lexical `YYYY-MM-DD`, returning the remaining text (a timezone, or empty).
fn take_date(s: &str) -> Option<&str> {
    let b = s.as_bytes();
    let mut i = if b.first() == Some(&b'-') { 1 } else { 0 };
    let year_start = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    if i - year_start < 4 {
        return None;
    }
    if b.get(i) != Some(&b'-') {
        return None;
    }
    i += 1;
    if !two_digits(b, i) {
        return None;
    }
    let month = num2(b, i);
    i += 2;
    if b.get(i) != Some(&b'-') {
        return None;
    }
    i += 1;
    if !two_digits(b, i) {
        return None;
    }
    let day = num2(b, i);
    i += 2;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    Some(&s[i..])
}

/// Consumes a lexical `HH:MM:SS(.fff)?`, returning the remaining text.
fn take_time(s: &str) -> Option<&str> {
    let b = s.as_bytes();
    let mut i = 0;
    if !two_digits(b, i) {
        return None;
    }
    let hour = num2(b, i);
    i += 2;
    if b.get(i) != Some(&b':') {
        return None;
    }
    i += 1;
    if !two_digits(b, i) {
        return None;
    }
    let minute = num2(b, i);
    i += 2;
    if b.get(i) != Some(&b':') {
        return None;
    }
    i += 1;
    if !two_digits(b, i) {
        return None;
    }
    let second = num2(b, i);
    i += 2;
    if b.get(i) == Some(&b'.') {
        i += 1;
        let frac_start = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i == frac_start {
            return None;
        }
    }
    if hour > 24 || minute > 59 || second > 60 {
        return None;
    }
    Some(&s[i..])
}

fn tz_ok(rest: &str) -> bool {
    if rest.is_empty() || rest == "Z" {
        return true;
    }
    let b = rest.as_bytes();
    b.len() == 6 && (b[0] == b'+' || b[0] == b'-') && b[3] == b':' && two_digits(b, 1) && two_digits(b, 4)
}

fn looks_date(s: &str) -> bool {
    take_date(s).map(tz_ok).unwrap_or(false)
}

fn looks_time(s: &str) -> bool {
    take_time(s).map(tz_ok).unwrap_or(false)
}

fn looks_datetime(s: &str) -> bool {
    match take_date(s).and_then(|rest| rest.strip_prefix('T')) {
        Some(t) => take_time(t).map(tz_ok).unwrap_or(false),
        None => false,
    }
}

fn looks_uri(s: &str) -> bool {
    if s.chars().any(char::is_whitespace) {
        return false;
    }
    let lower = s.to_ascii_lowercase();
    ["http://", "https://", "ftp://", "urn:", "mailto:"]
        .iter()
        .any(|p| lower.starts_with(p))
}

/// Picks the narrowest builtin `xs:*` type that every observed value fits.
///
/// Empty values are ignored; an all-empty set is `xs:string`. Integers widen by observed
/// range (`xs:int` → `xs:long` → `xs:integer`) rather than collapsing to the smallest
/// builtin, so a two-digit sample does not permanently constrain a field.
pub fn infer_xsd_type(values: &[String], mode: TypeInference) -> &'static str {
    if mode == TypeInference::StringOnly {
        return "xs:string";
    }
    let vals: Vec<&str> = values.iter().map(|v| v.trim()).filter(|v| !v.is_empty()).collect();
    if vals.is_empty() {
        return "xs:string";
    }
    // "0"/"1" are legal xs:boolean but ambiguous with integers — only true/false infer boolean.
    if vals.iter().all(|v| matches!(*v, "true" | "false")) {
        return "xs:boolean";
    }
    if vals.iter().all(|v| looks_int(v)) {
        let mut lo = i64::MAX;
        let mut hi = i64::MIN;
        for v in &vals {
            match v.parse::<i64>() {
                Ok(n) => {
                    lo = lo.min(n);
                    hi = hi.max(n);
                }
                Err(_) => return "xs:integer",
            }
        }
        if lo >= i32::MIN as i64 && hi <= i32::MAX as i64 {
            return "xs:int";
        }
        return "xs:long";
    }
    if vals.iter().all(|v| looks_int(v) || looks_decimal(v)) {
        return "xs:decimal";
    }
    if vals.iter().all(|v| looks_int(v) || looks_decimal(v) || looks_double(v)) {
        return "xs:double";
    }
    if vals.iter().all(|v| looks_datetime(v)) {
        return "xs:dateTime";
    }
    if vals.iter().all(|v| looks_date(v)) {
        return "xs:date";
    }
    if vals.iter().all(|v| looks_time(v)) {
        return "xs:time";
    }
    if vals.iter().all(|v| looks_uri(v)) {
        return "xs:anyURI";
    }
    "xs:string"
}

// ---------------------------------------------------------------------------
// Emission
// ---------------------------------------------------------------------------

fn esc(s: &str) -> String {
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

enum Shape {
    /// Text only — a builtin simple type.
    Simple,
    /// Has child elements.
    ComplexChildren,
    /// Text plus attributes — `xs:simpleContent`.
    SimpleContentAttrs,
    /// Attributes only, no text and no children.
    EmptyAttrs,
}

fn shape_of(m: &ElemModel) -> Shape {
    if m.has_children {
        Shape::ComplexChildren
    } else if m.has_attrs {
        if m.any_text {
            Shape::SimpleContentAttrs
        } else {
            Shape::EmptyAttrs
        }
    } else {
        Shape::Simple
    }
}

struct Emitter<'a> {
    opts: &'a Options,
    models: &'a Models,
    prefix: String,
    out: String,
}

impl<'a> Emitter<'a> {
    fn pad(&self, level: usize) -> String {
        " ".repeat(self.opts.indent * level)
    }

    fn line(&mut self, level: usize, s: &str) {
        let pad = self.pad(level);
        self.out.push_str(&pad);
        self.out.push_str(s);
        self.out.push('\n');
    }

    fn type_ref(&self, local: &str) -> String {
        format!("{}{}Type", self.prefix, local)
    }

    fn occurs_attrs(&self, min: usize, max: usize) -> String {
        match self.opts.occurrence {
            Occurrence::Relaxed => " minOccurs=\"0\" maxOccurs=\"unbounded\"".to_string(),
            Occurrence::Restricted => {
                let mut s = String::new();
                if min == 0 {
                    s.push_str(" minOccurs=\"0\"");
                }
                // A sample showing N>1 repeats proves repetition, not an upper bound of N.
                if max > 1 {
                    s.push_str(" maxOccurs=\"unbounded\"");
                }
                s
            }
        }
    }

    /// Distinct values worth turning into an `xs:enumeration` restriction, if enabled.
    fn enum_values(&self, values: &[String]) -> Option<Vec<String>> {
        if self.opts.enumerations == 0 || values.is_empty() {
            return None;
        }
        let mut distinct: Vec<String> = Vec::new();
        for v in values {
            let t = v.trim();
            if t.is_empty() {
                return None;
            }
            if !distinct.iter().any(|d| d == t) {
                distinct.push(t.to_string());
                if distinct.len() > self.opts.enumerations {
                    return None;
                }
            }
        }
        // A single distinct value is one observation, not a value domain.
        if distinct.len() >= 2 {
            Some(distinct)
        } else {
            None
        }
    }

    fn write_enum_simple_type(&mut self, level: usize, base: &str, values: &[String]) {
        self.line(level, "<xs:simpleType>");
        self.line(level + 1, &format!("<xs:restriction base=\"{base}\">"));
        for v in values {
            self.line(level + 2, &format!("<xs:enumeration value=\"{}\"/>", esc(v)));
        }
        self.line(level + 1, "</xs:restriction>");
        self.line(level, "</xs:simpleType>");
    }

    /// `<xs:element name="…">` for an element whose content is a builtin simple type.
    fn write_simple_element(&mut self, level: usize, m: &ElemModel, occurs: &str) {
        let base = infer_xsd_type(&m.text_values, self.opts.type_inference);
        let nil = if m.nillable { " nillable=\"true\"" } else { "" };
        match self.enum_values(&m.text_values) {
            Some(values) => {
                self.line(level, &format!("<xs:element name=\"{}\"{occurs}{nil}>", esc(&m.local)));
                self.write_enum_simple_type(level + 1, base, &values);
                self.line(level, "</xs:element>");
            }
            None => self.line(
                level,
                &format!("<xs:element name=\"{}\" type=\"{base}\"{occurs}{nil}/>", esc(&m.local)),
            ),
        }
    }

    fn write_attributes(&mut self, level: usize, m: &ElemModel) {
        for (idx, name) in m.attr_order.iter().enumerate() {
            let am = &m.attr_models[idx];
            let required = self.opts.occurrence == Occurrence::Restricted && am.present == m.instances;
            let use_attr = if required { " use=\"required\"" } else { "" };
            let base = infer_xsd_type(&am.values, self.opts.type_inference);
            match self.enum_values(&am.values) {
                Some(values) => {
                    self.line(level, &format!("<xs:attribute name=\"{}\"{use_attr}>", esc(name)));
                    self.write_enum_simple_type(level + 1, base, &values);
                    self.line(level, "</xs:attribute>");
                }
                None => self.line(
                    level,
                    &format!("<xs:attribute name=\"{}\" type=\"{base}\"{use_attr}/>", esc(name)),
                ),
            }
        }
    }

    /// The `<xs:sequence>` / `<xs:choice>` content model of a complex element.
    fn write_content_model(&mut self, level: usize, m: &ElemModel, path: &mut Vec<String>) -> Result<(), String> {
        // A sample whose siblings appear in varying order (or that mixes in foreign-namespace
        // elements) cannot be described by xs:sequence — it would reject its own sample.
        let unordered = !m.ordered || m.foreign_children;
        if unordered {
            self.line(level, "<xs:choice minOccurs=\"0\" maxOccurs=\"unbounded\">");
        } else {
            self.line(level, "<xs:sequence>");
        }
        for (idx, ck) in m.child_order.iter().enumerate() {
            let occurs = if unordered {
                String::new()
            } else {
                let st = &m.child_stats[idx];
                // min>1 is still just "present"; only 0 vs 1+ is trustworthy from one sample.
                self.occurs_attrs(st.min.min(1), st.max)
            };
            let ci = self.models.index_of(ck).expect("child model exists");
            self.write_child(level + 1, ci, &occurs, path)?;
        }
        if m.foreign_children {
            self.line(
                level + 1,
                "<xs:any namespace=\"##other\" processContents=\"lax\"/>",
            );
        }
        if unordered {
            self.line(level, "</xs:choice>");
        } else {
            self.line(level, "</xs:sequence>");
        }
        Ok(())
    }

    /// One child declaration inside a parent's content model, per design.
    fn write_child(
        &mut self,
        level: usize,
        ci: usize,
        occurs: &str,
        path: &mut Vec<String>,
    ) -> Result<(), String> {
        let m = &self.models.items[ci];
        let local = m.local.clone();
        let nil = if m.nillable { " nillable=\"true\"" } else { "" };
        match self.opts.design {
            Design::SalamiSlice => {
                self.line(
                    level,
                    &format!("<xs:element ref=\"{}{}\"{occurs}/>", self.prefix, esc(&local)),
                );
                Ok(())
            }
            Design::VenetianBlind => {
                if matches!(shape_of(m), Shape::Simple) {
                    self.write_simple_element(level, &self.models.items[ci].clone_shallow(), occurs);
                } else {
                    let tref = self.type_ref(&local);
                    self.line(
                        level,
                        &format!("<xs:element name=\"{}\" type=\"{tref}\"{occurs}{nil}/>", esc(&local)),
                    );
                }
                Ok(())
            }
            Design::RussianDoll => {
                if matches!(shape_of(m), Shape::Simple) {
                    self.write_simple_element(level, &self.models.items[ci].clone_shallow(), occurs);
                    return Ok(());
                }
                let key = self.models.keys[ci].clone();
                if path.contains(&key) {
                    return Err(format!(
                        "the sample nests <{local}> inside itself; the russian-doll design \
                         declares every type inline and cannot express a recursive structure — \
                         use design=venetian-blind (named global types) or design=salami-slice \
                         (global elements) for this document"
                    ));
                }
                self.line(level, &format!("<xs:element name=\"{}\"{occurs}{nil}>", esc(&local)));
                path.push(key);
                self.write_complex_type(level + 1, ci, None, path)?;
                path.pop();
                self.line(level, "</xs:element>");
                Ok(())
            }
        }
    }

    /// `<xs:complexType[ name="…"]>` … `</xs:complexType>` for a complex element model.
    fn write_complex_type(
        &mut self,
        level: usize,
        ci: usize,
        name: Option<&str>,
        path: &mut Vec<String>,
    ) -> Result<(), String> {
        let m = self.models.items[ci].clone_shallow();
        let name_attr = name.map(|n| format!(" name=\"{}\"", esc(n))).unwrap_or_default();
        match shape_of(&m) {
            Shape::ComplexChildren => {
                let mixed = if m.any_text { " mixed=\"true\"" } else { "" };
                self.line(level, &format!("<xs:complexType{name_attr}{mixed}>"));
                self.write_content_model(level + 1, &m, path)?;
                self.write_attributes(level + 1, &m);
                self.line(level, "</xs:complexType>");
            }
            Shape::SimpleContentAttrs => {
                let base = infer_xsd_type(&m.text_values, self.opts.type_inference);
                self.line(level, &format!("<xs:complexType{name_attr}>"));
                self.line(level + 1, "<xs:simpleContent>");
                self.line(level + 2, &format!("<xs:extension base=\"{base}\">"));
                self.write_attributes(level + 3, &m);
                self.line(level + 2, "</xs:extension>");
                self.line(level + 1, "</xs:simpleContent>");
                self.line(level, "</xs:complexType>");
            }
            Shape::EmptyAttrs | Shape::Simple => {
                self.line(level, &format!("<xs:complexType{name_attr}>"));
                self.write_attributes(level + 1, &m);
                self.line(level, "</xs:complexType>");
            }
        }
        Ok(())
    }
}

impl ElemModel {
    /// A detached copy used while the emitter holds `&mut self` — models are small.
    fn clone_shallow(&self) -> ElemModel {
        ElemModel {
            local: self.local.clone(),
            instances: self.instances,
            child_order: self.child_order.clone(),
            child_stats: self.child_stats.iter().map(|c| ChildStat { min: c.min, max: c.max }).collect(),
            attr_order: self.attr_order.clone(),
            attr_models: self
                .attr_models
                .iter()
                .map(|a| AttrModel { present: a.present, values: a.values.clone() })
                .collect(),
            text_values: self.text_values.clone(),
            any_text: self.any_text,
            has_children: self.has_children,
            has_attrs: self.has_attrs,
            ordered: self.ordered,
            nillable: self.nillable,
            foreign_children: self.foreign_children,
        }
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Infers an XSD document from a sample XML document.
pub fn generate(xml: &str, opts: &Options) -> Result<String, String> {
    if xml.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "XML sample is {} bytes; the limit is {MAX_INPUT_BYTES} bytes — trim it to a \
             representative excerpt (every distinct element only has to appear once)",
            xml.len()
        ));
    }
    if xml.trim().is_empty() {
        return Err(
            "no XML provided — paste a sample XML document, e.g. \
             '<order id=\"1\"><item>pen</item></order>'"
                .to_string(),
        );
    }
    if opts.indent > MAX_INDENT {
        return Err(format!(
            "indent is {}; expected 0 to {MAX_INDENT} spaces per level",
            opts.indent
        ));
    }

    let root = parse_document(xml)?;
    let doc_ns = root.ns.clone();
    let mut models = Models::default();
    collect(&root, &doc_ns, &mut models);

    let tns = if opts.target_namespace.trim().is_empty() {
        doc_ns.clone()
    } else {
        opts.target_namespace.trim().to_string()
    };
    let prefix = if tns.is_empty() { String::new() } else { "tns:".to_string() };

    let mut em = Emitter { opts, models: &models, prefix, out: String::new() };

    if opts.declaration {
        em.out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    }
    if tns.is_empty() {
        em.out.push_str(&format!("<xs:schema xmlns:xs=\"{XSD_NS}\">\n"));
    } else {
        em.out.push_str(&format!("<xs:schema xmlns:xs=\"{XSD_NS}\"\n"));
        em.out.push_str(&format!("           xmlns:tns=\"{}\"\n", esc(&tns)));
        em.out.push_str(&format!("           targetNamespace=\"{}\"\n", esc(&tns)));
        em.out.push_str("           elementFormDefault=\"qualified\"\n");
        em.out.push_str("           attributeFormDefault=\"unqualified\">\n");
    }

    let mut path: Vec<String> = Vec::new();
    match opts.design {
        Design::RussianDoll => {
            let m = models.items[0].clone_shallow();
            let nil = if m.nillable { " nillable=\"true\"" } else { "" };
            if matches!(shape_of(&m), Shape::Simple) {
                em.write_simple_element(1, &m, "");
            } else {
                em.line(1, &format!("<xs:element name=\"{}\"{nil}>", esc(&m.local)));
                path.push(models.keys[0].clone());
                em.write_complex_type(2, 0, None, &mut path)?;
                path.pop();
                em.line(1, "</xs:element>");
            }
        }
        Design::SalamiSlice => {
            for ci in 0..models.items.len() {
                let m = models.items[ci].clone_shallow();
                let nil = if m.nillable { " nillable=\"true\"" } else { "" };
                if matches!(shape_of(&m), Shape::Simple) {
                    em.write_simple_element(1, &m, "");
                } else {
                    em.line(1, &format!("<xs:element name=\"{}\"{nil}>", esc(&m.local)));
                    em.write_complex_type(2, ci, None, &mut path)?;
                    em.line(1, "</xs:element>");
                }
            }
        }
        Design::VenetianBlind => {
            let root_m = models.items[0].clone_shallow();
            let nil = if root_m.nillable { " nillable=\"true\"" } else { "" };
            if matches!(shape_of(&root_m), Shape::Simple) {
                em.write_simple_element(1, &root_m, "");
            } else {
                let tref = em.type_ref(&root_m.local);
                em.line(
                    1,
                    &format!("<xs:element name=\"{}\" type=\"{tref}\"{nil}/>", esc(&root_m.local)),
                );
            }
            for ci in 0..models.items.len() {
                let m = models.items[ci].clone_shallow();
                if matches!(shape_of(&m), Shape::Simple) {
                    continue;
                }
                let type_name = format!("{}Type", m.local);
                em.write_complex_type(1, ci, Some(&type_name), &mut path)?;
            }
        }
    }

    em.out.push_str("</xs:schema>\n");
    Ok(em.out)
}

/// Convenience entry point used by the chat block, CLI, and browser wrapper.
#[allow(clippy::too_many_arguments)]
pub fn run(
    xml: &str,
    design: &str,
    type_inference: &str,
    occurrence: &str,
    enumerations: f64,
    target_namespace: &str,
    indent: f64,
    declaration: bool,
) -> Result<String, String> {
    if !enumerations.is_finite() || enumerations < 0.0 || enumerations.fract() != 0.0 {
        return Err(format!(
            "invalid enumerations {enumerations}: expected a whole number >= 0"
        ));
    }
    if !indent.is_finite() || indent < 0.0 || indent.fract() != 0.0 {
        return Err(format!("invalid indent {indent}: expected a whole number 0 to {MAX_INDENT}"));
    }
    let opts = Options {
        design: design_from_str(design),
        type_inference: type_inference_from_str(type_inference),
        occurrence: occurrence_from_str(occurrence),
        enumerations: enumerations as usize,
        target_namespace: target_namespace.trim().to_string(),
        indent: indent as usize,
        declaration,
    };
    generate(xml, &opts)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn gen(xml: &str, opts: &Options) -> String {
        generate(xml, opts).expect("generation succeeds")
    }

    #[test]
    fn happy_path_venetian_blind_nested_and_typed() {
        let xsd = gen(
            "<order id=\"7\"><total>19.95</total><item>pen</item><item>pad</item></order>",
            &Options::default(),
        );
        assert!(xsd.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n"), "{xsd}");
        assert!(xsd.contains("<xs:element name=\"order\" type=\"orderType\"/>"), "{xsd}");
        assert!(xsd.contains("<xs:complexType name=\"orderType\">"), "{xsd}");
        assert!(xsd.contains("<xs:element name=\"total\" type=\"xs:decimal\"/>"), "{xsd}");
        assert!(
            xsd.contains("<xs:element name=\"item\" type=\"xs:string\" maxOccurs=\"unbounded\"/>"),
            "{xsd}"
        );
        assert!(xsd.contains("<xs:attribute name=\"id\" type=\"xs:int\" use=\"required\"/>"), "{xsd}");
        assert!(xsd.trim_end().ends_with("</xs:schema>"), "{xsd}");
    }

    #[test]
    fn error_on_malformed_xml_names_the_position() {
        let err = generate("<a><b></a>", &Options::default()).unwrap_err();
        assert!(err.contains("XML parse error"), "{err}");
    }

    #[test]
    fn error_on_empty_input_shows_an_example() {
        let err = generate("   ", &Options::default()).unwrap_err();
        assert!(err.contains("paste a sample XML document"), "{err}");
        assert!(err.contains("<order"), "{err}");
    }

    #[test]
    fn error_on_oversize_input_states_the_cap() {
        let big = format!("<a>{}</a>", "x".repeat(MAX_INPUT_BYTES));
        let err = generate(&big, &Options::default()).unwrap_err();
        assert!(err.contains(&MAX_INPUT_BYTES.to_string()), "{err}");
    }

    #[test]
    fn error_on_second_root_element() {
        let err = generate("<a/><b/>", &Options::default()).unwrap_err();
        assert!(err.contains("second top-level element"), "{err}");
    }

    #[test]
    fn cardinality_optional_child_gets_min_occurs_zero() {
        let xsd = gen(
            "<list><row><a>1</a><b>2</b></row><row><a>3</a></row></list>",
            &Options::default(),
        );
        assert!(xsd.contains("<xs:element name=\"a\" type=\"xs:int\"/>"), "{xsd}");
        assert!(xsd.contains("<xs:element name=\"b\" type=\"xs:int\" minOccurs=\"0\"/>"), "{xsd}");
        assert!(xsd.contains("<xs:element name=\"row\" type=\"rowType\" maxOccurs=\"unbounded\"/>"), "{xsd}");
    }

    #[test]
    fn child_first_seen_in_a_later_instance_is_optional() {
        let xsd = gen(
            "<list><row><a>1</a></row><row><a>2</a><note>hi</note></row></list>",
            &Options::default(),
        );
        assert!(xsd.contains("<xs:element name=\"note\" type=\"xs:string\" minOccurs=\"0\"/>"), "{xsd}");
    }

    #[test]
    fn optional_attribute_is_not_required() {
        let xsd = gen("<l><r id=\"1\" tag=\"x\"/><r id=\"2\"/></l>", &Options::default());
        assert!(xsd.contains("<xs:attribute name=\"id\" type=\"xs:int\" use=\"required\"/>"), "{xsd}");
        assert!(xsd.contains("<xs:attribute name=\"tag\" type=\"xs:string\"/>"), "{xsd}");
    }

    #[test]
    fn primitive_type_inference_covers_the_advertised_builtins() {
        let xsd = gen(
            "<r><b>true</b><i>42</i><big>9000000000</big><d>1.5</d><e>1.5e3</e>\
             <dt>2026-09-24T08:30:00Z</dt><day>2026-09-24</day><t>08:30:00</t>\
             <u>https://example.com/a</u><s>hello</s><zip>01234</zip></r>",
            &Options::default(),
        );
        for (name, ty) in [
            ("b", "xs:boolean"),
            ("i", "xs:int"),
            ("big", "xs:long"),
            ("d", "xs:decimal"),
            ("e", "xs:double"),
            ("dt", "xs:dateTime"),
            ("day", "xs:date"),
            ("t", "xs:time"),
            ("u", "xs:anyURI"),
            ("s", "xs:string"),
            // Leading-zero codes stay text rather than becoming numbers.
            ("zip", "xs:string"),
        ] {
            assert!(
                xsd.contains(&format!("<xs:element name=\"{name}\" type=\"{ty}\"/>")),
                "expected {name} to be {ty}\n{xsd}"
            );
        }
    }

    #[test]
    fn string_only_mode_declares_every_value_as_string() {
        let opts = Options { type_inference: TypeInference::StringOnly, ..Options::default() };
        let xsd = gen("<r id=\"5\"><n>42</n></r>", &opts);
        assert!(xsd.contains("<xs:element name=\"n\" type=\"xs:string\"/>"), "{xsd}");
        assert!(xsd.contains("<xs:attribute name=\"id\" type=\"xs:string\" use=\"required\"/>"), "{xsd}");
    }

    #[test]
    fn mixed_type_values_widen_to_decimal() {
        let xsd = gen("<r><n>1</n><n>2.5</n></r>", &Options::default());
        assert!(xsd.contains("type=\"xs:decimal\""), "{xsd}");
    }

    #[test]
    fn russian_doll_nests_every_type_inline() {
        let opts = Options { design: Design::RussianDoll, ..Options::default() };
        let xsd = gen("<order><item sku=\"a1\">pen</item></order>", &opts);
        assert!(xsd.contains("<xs:element name=\"order\">"), "{xsd}");
        assert!(!xsd.contains("<xs:complexType name="), "russian doll has no named types\n{xsd}");
        assert!(xsd.contains("<xs:element name=\"item\">"), "{xsd}");
        assert!(xsd.contains("<xs:simpleContent>"), "{xsd}");
        assert!(xsd.contains("<xs:extension base=\"xs:string\">"), "{xsd}");
    }

    #[test]
    fn russian_doll_rejects_recursion_with_an_actionable_message() {
        let opts = Options { design: Design::RussianDoll, ..Options::default() };
        let err = generate("<folder><folder><name>a</name></folder></folder>", &opts).unwrap_err();
        assert!(err.contains("recursive"), "{err}");
        assert!(err.contains("venetian-blind"), "{err}");
    }

    #[test]
    fn venetian_blind_handles_recursion_via_named_types() {
        let xsd = gen("<folder><folder><name>a</name></folder></folder>", &Options::default());
        assert!(xsd.contains("<xs:complexType name=\"folderType\">"), "{xsd}");
        assert!(xsd.contains("<xs:element name=\"folder\" type=\"folderType\""), "{xsd}");
    }

    #[test]
    fn salami_slice_declares_every_element_globally_and_refs_them() {
        let opts = Options { design: Design::SalamiSlice, ..Options::default() };
        let xsd = gen("<order><item>pen</item></order>", &opts);
        assert!(xsd.contains("<xs:element ref=\"item\"/>"), "{xsd}");
        assert!(xsd.contains("<xs:element name=\"item\" type=\"xs:string\"/>"), "{xsd}");
        assert!(xsd.contains("<xs:element name=\"order\">"), "{xsd}");
    }

    #[test]
    fn relaxed_occurrence_makes_everything_optional_and_repeatable() {
        let opts = Options { occurrence: Occurrence::Relaxed, ..Options::default() };
        let xsd = gen("<order id=\"1\"><item>pen</item></order>", &opts);
        assert!(
            xsd.contains("<xs:element name=\"item\" type=\"xs:string\" minOccurs=\"0\" maxOccurs=\"unbounded\"/>"),
            "{xsd}"
        );
        assert!(xsd.contains("<xs:attribute name=\"id\" type=\"xs:int\"/>"), "{xsd}");
        assert!(!xsd.contains("use=\"required\""), "{xsd}");
    }

    #[test]
    fn namespaced_sample_gets_a_target_namespace_and_qualified_form() {
        let xsd = gen("<o xmlns=\"urn:demo\"><i>pen</i></o>", &Options::default());
        assert!(xsd.contains("targetNamespace=\"urn:demo\""), "{xsd}");
        assert!(xsd.contains("xmlns:tns=\"urn:demo\""), "{xsd}");
        assert!(xsd.contains("elementFormDefault=\"qualified\""), "{xsd}");
        assert!(xsd.contains("<xs:element name=\"o\" type=\"tns:oType\"/>"), "{xsd}");
    }

    #[test]
    fn target_namespace_option_overrides_a_namespace_free_sample() {
        let opts = Options { target_namespace: "urn:mine".into(), ..Options::default() };
        let xsd = gen("<o><i>pen</i></o>", &opts);
        assert!(xsd.contains("targetNamespace=\"urn:mine\""), "{xsd}");
        assert!(xsd.contains("<xs:element name=\"o\" type=\"tns:oType\"/>"), "{xsd}");
    }

    #[test]
    fn foreign_namespace_children_become_a_wildcard() {
        let xsd = gen(
            "<e xmlns=\"urn:a\" xmlns:x=\"urn:b\"><i>1</i><x:other>2</x:other></e>",
            &Options::default(),
        );
        assert!(xsd.contains("<xs:any namespace=\"##other\" processContents=\"lax\"/>"), "{xsd}");
        assert!(xsd.contains("<xs:choice minOccurs=\"0\" maxOccurs=\"unbounded\">"), "{xsd}");
        assert!(!xsd.contains("name=\"other\""), "foreign elements are not declared\n{xsd}");
    }

    #[test]
    fn varying_sibling_order_falls_back_to_a_repeating_choice() {
        let xsd = gen(
            "<l><r><a>1</a><b>2</b></r><r><b>3</b><a>4</a></r></l>",
            &Options::default(),
        );
        assert!(xsd.contains("<xs:choice minOccurs=\"0\" maxOccurs=\"unbounded\">"), "{xsd}");
        assert!(!xsd.contains("<xs:sequence>\n      <xs:element name=\"a\""), "{xsd}");
    }

    #[test]
    fn mixed_content_is_marked_mixed() {
        let xsd = gen("<p>hello <b>world</b> again</p>", &Options::default());
        assert!(xsd.contains("<xs:complexType name=\"pType\" mixed=\"true\">"), "{xsd}");
    }

    #[test]
    fn xsi_nil_marks_the_element_nillable_and_xsi_attrs_are_not_declared() {
        let xsd = gen(
            "<r xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\
             <n xsi:nil=\"true\"/><n>5</n></r>",
            &Options::default(),
        );
        assert!(xsd.contains("nillable=\"true\""), "{xsd}");
        assert!(!xsd.contains("name=\"nil\""), "xsi:nil is not a real attribute\n{xsd}");
    }

    #[test]
    fn enumerations_off_by_default_and_opt_in_by_cap() {
        let off = gen(
            "<l><r s=\"new\">a</r><r s=\"old\">b</r><r s=\"new\">a</r></l>",
            &Options::default(),
        );
        assert!(!off.contains("<xs:enumeration"), "{off}");

        let opts = Options { enumerations: 5, ..Options::default() };
        let on = gen("<l><r s=\"new\">a</r><r s=\"old\">b</r><r s=\"new\">a</r></l>", &opts);
        assert!(on.contains("<xs:enumeration value=\"new\"/>"), "{on}");
        assert!(on.contains("<xs:enumeration value=\"old\"/>"), "{on}");
        assert!(on.contains("<xs:restriction base=\"xs:string\">"), "{on}");
    }

    #[test]
    fn enumeration_cap_is_respected() {
        let opts = Options { enumerations: 2, ..Options::default() };
        let xsd = gen("<l><r>a</r><r>b</r><r>c</r></l>", &opts);
        assert!(!xsd.contains("<xs:enumeration"), "3 distinct values exceed the cap of 2\n{xsd}");
    }

    #[test]
    fn declaration_and_indent_options_shape_the_output() {
        let opts = Options { declaration: false, indent: 4, ..Options::default() };
        let xsd = gen("<order><item>pen</item></order>", &opts);
        assert!(!xsd.contains("<?xml"), "{xsd}");
        assert!(xsd.starts_with("<xs:schema "), "{xsd}");
        assert!(xsd.contains("\n    <xs:element name=\"order\""), "{xsd}");

        let flat = Options { indent: 0, ..Options::default() };
        let xsd0 = gen("<order><item>pen</item></order>", &flat);
        assert!(xsd0.contains("\n<xs:element name=\"order\""), "{xsd0}");
    }

    #[test]
    fn indent_over_the_cap_is_rejected() {
        let opts = Options { indent: 9, ..Options::default() };
        let err = generate("<a/>", &opts).unwrap_err();
        assert!(err.contains("expected 0 to 8"), "{err}");
    }

    #[test]
    fn attribute_values_and_text_are_escaped_in_the_output() {
        let opts = Options { enumerations: 4, ..Options::default() };
        let xsd = gen("<l><r>a&amp;b</r><r>c&lt;d</r></l>", &opts);
        assert!(xsd.contains("<xs:enumeration value=\"a&amp;b\"/>"), "{xsd}");
        assert!(xsd.contains("<xs:enumeration value=\"c&lt;d\"/>"), "{xsd}");
    }

    #[test]
    fn empty_element_with_attributes_only_has_no_content_model() {
        let xsd = gen("<r><img src=\"a.png\"/></r>", &Options::default());
        assert!(xsd.contains("<xs:complexType name=\"imgType\">"), "{xsd}");
        assert!(!xsd.contains("<xs:sequence>\n    <xs:attribute"), "{xsd}");
        assert!(xsd.contains("<xs:attribute name=\"src\" type=\"xs:string\" use=\"required\"/>"), "{xsd}");
    }

    #[test]
    fn parser_option_helpers_round_trip_and_default_safely() {
        assert_eq!(design_from_str("russian-doll"), Design::RussianDoll);
        assert_eq!(design_from_str("salami-slice"), Design::SalamiSlice);
        assert_eq!(design_from_str("venetian-blind"), Design::VenetianBlind);
        assert_eq!(design_from_str("nonsense"), Design::VenetianBlind);
        assert_eq!(type_inference_from_str("string"), TypeInference::StringOnly);
        assert_eq!(type_inference_from_str(""), TypeInference::Smart);
        assert_eq!(occurrence_from_str("relaxed"), Occurrence::Relaxed);
        assert_eq!(occurrence_from_str("restricted"), Occurrence::Restricted);
    }

    #[test]
    fn cdata_and_comments_are_handled_as_text() {
        let xsd = gen("<r><!-- note --><n><![CDATA[42]]></n></r>", &Options::default());
        assert!(xsd.contains("<xs:element name=\"n\" type=\"xs:int\"/>"), "{xsd}");
    }
}
