//! gizza-ai/xliff-to-json core — extract translation units from an XLIFF
//! document into a flat (or nested) JSON keyed by unit id. Pure-Rust
//! (`quick-xml` + `serde_json` with `preserve_order`); no wafer/wasm-bindgen
//! deps.
//!
//! Both dialects are handled by the same streaming pass, so a file's version
//! never has to be declared:
//! - **XLIFF 1.2** — `<file>` › (`<group>`…) › `<trans-unit id resname>` with
//!   `<source>`, `<target state=…>` and `<note>` as direct children.
//! - **XLIFF 2.x** — `<file>` › (`<group>`…) › `<unit id name>` › `<segment
//!   state=…>` › `<source>`/`<target>`, notes under `<notes><note>`.
//!
//! `<source>`/`<target>` are captured only when their *parent* is a
//! `<trans-unit>` (1.2) or a `<segment>` (2.x), which is what keeps
//! `<alt-trans>`, `<seg-source>`, `<mtc:match>` and `<originalData>` copies out
//! of the result. A 2.x `<unit>` with several `<segment>` children has them
//! concatenated in document order; `<ignorable>` is skipped.

use quick_xml::events::{BytesStart, Event};
use quick_xml::reader::Reader;
use serde_json::{Map, Value};

/// Shape of the emitted JSON.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    /// `{ id: { "source": …, "target": … } }` — the lossless default.
    Pairs,
    /// `{ id: target }` — a drop-in translation bundle.
    Target,
    /// `{ id: source }`.
    Source,
    /// `[ { "id": …, "source": …, "target": … }, … ]` — keeps order and duplicates.
    Array,
}

/// Which value becomes the object key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyBy {
    Id,
    Resname,
    SourceText,
}

/// How inline markup inside `<source>`/`<target>` is rendered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InlineTags {
    /// Each code/standalone element becomes its `equiv-text`, else `{id}`.
    Placeholder,
    /// Inline markup is dropped; only translatable text survives.
    Strip,
    /// The element's inner XML is preserved verbatim.
    Keep,
}

/// Options controlling extraction and output.
#[derive(Clone, Debug)]
pub struct Options {
    pub output: Shape,
    pub key: KeyBy,
    pub nested: bool,
    pub separator: String,
    pub include_empty_targets: bool,
    pub fallback_to_source: bool,
    pub inline_tags: InlineTags,
    pub include_metadata: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            output: Shape::Pairs,
            key: KeyBy::Id,
            nested: false,
            separator: ".".to_string(),
            include_empty_targets: true,
            fallback_to_source: false,
            inline_tags: InlineTags::Placeholder,
            include_metadata: false,
        }
    }
}

/// One extracted translation unit.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Unit {
    pub id: String,
    /// 1.2 `resname` / 2.x `name`; empty when the file does not set one.
    pub resname: String,
    pub source: String,
    pub target: String,
    /// Joined `<note>` text (newline separated).
    pub note: String,
    /// 1.2 `<target state>` / 2.x `<segment state>`.
    pub state: String,
    /// 1.2 `<trans-unit approved>`.
    pub approved: String,
    /// The enclosing `<file>`'s `original` (1.2) or `id` (2.x).
    pub file: String,
    /// Slash-joined ids of the enclosing `<group>` elements.
    pub group: String,
}

/// Strip an `ns:local` prefix, returning the local name.
fn local_name(qname: &[u8]) -> String {
    let s = String::from_utf8_lossy(qname);
    match s.rsplit_once(':') {
        Some((_, local)) => local.to_string(),
        None => s.into_owned(),
    }
}

/// Read an attribute by local (or fully-qualified) name.
fn attr(e: &BytesStart, want: &str) -> Option<String> {
    for a in e.attributes().flatten() {
        let raw = a.key.as_ref();
        if local_name(raw) == want || String::from_utf8_lossy(raw) == want {
            return a.unescape_value().ok().map(|v| v.into_owned());
        }
    }
    None
}

fn attr_or_empty(e: &BytesStart, want: &str) -> String {
    attr(e, want).unwrap_or_default()
}

/// Elements whose *content* is native markup rather than translatable text
/// (XLIFF 1.2/2.x paired and standalone codes). Their text is suppressed
/// unless `inline_tags = keep`.
fn is_code_tag(name: &str) -> bool {
    matches!(name, "bpt" | "ept" | "ph" | "it" | "x" | "bx" | "ex" | "sc" | "ec" | "cp")
}

/// The placeholder marker for one inline element.
fn marker(e: &BytesStart) -> String {
    for k in ["equiv-text", "equivText", "equiv", "disp"] {
        if let Some(v) = attr(e, k) {
            if !v.is_empty() {
                return v;
            }
        }
    }
    for k in ["id", "name", "ctype"] {
        if let Some(v) = attr(e, k) {
            if !v.is_empty() {
                return format!("{{{v}}}");
            }
        }
    }
    format!("{{{}}}", local_name(e.name().as_ref()))
}

/// Serialize a start/empty tag back to XML for `inline_tags = keep`.
fn raw_tag(e: &BytesStart, self_closing: bool) -> String {
    let inner = String::from_utf8_lossy(e.as_ref()).into_owned();
    if self_closing {
        format!("<{inner}/>")
    } else {
        format!("<{inner}>")
    }
}

/// Drop the pretty-printer's indentation from a text run without touching
/// whitespace that is part of the string.
///
/// XLIFF treats whitespace inside `<source>`/`<target>` as significant, so a
/// blanket `trim()` is wrong: an XLIFF 2.x `<unit>` whose segments partition
/// `"Hello world"` stores the space as the second segment's leading character,
/// and trimming every run silently concatenates them into `"Helloworld"`. Only
/// leading/trailing whitespace that spans a **newline** is unambiguously the
/// file's own layout, so that is all we remove.
fn trim_indentation(text: &str) -> String {
    let lead_end = text.len() - text.trim_start().len();
    let start = if text[..lead_end].contains('\n') { lead_end } else { 0 };
    let trail_start = text.trim_end().len().max(start);
    let end = if text[trail_start..].contains('\n') { trail_start } else { text.len() };
    text[start..end].to_string()
}

/// Which text field is currently being collected.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Field {
    Source,
    Target,
    Note,
}

/// In-progress text collection for one `<source>`/`<target>`/`<note>`.
struct Capture {
    field: Field,
    /// Element depth of the capturing element, so its `</…>` ends the capture.
    depth: usize,
    buf: String,
    preserve: bool,
    /// Nesting level inside code elements whose text must be suppressed.
    code_depth: usize,
}

/// Parser state shared by the `Start`/`Empty` handling.
struct Parser<'a> {
    inline: InlineTags,
    units: Vec<Unit>,
    path: Vec<String>,
    /// `xml:space="preserve"` inherited down the element path.
    preserve: Vec<bool>,
    groups: Vec<String>,
    current_file: String,
    cur: Option<Unit>,
    cap: Option<Capture>,
    root_name: Option<String>,
    _marker: std::marker::PhantomData<&'a ()>,
}

impl Parser<'_> {
    fn parent(&self) -> &str {
        self.path.last().map(|s| s.as_str()).unwrap_or("")
    }

    fn grandparent(&self) -> &str {
        if self.path.len() >= 2 {
            self.path[self.path.len() - 2].as_str()
        } else {
            ""
        }
    }

    /// Handle a `<tag …>` or `<tag …/>`. Returns the element's inherited
    /// `xml:space` flag so `Start` can push it.
    fn open(&mut self, e: &BytesStart, self_closing: bool) -> bool {
        let name = local_name(e.name().as_ref());
        let inherited = *self.preserve.last().unwrap_or(&false);
        let preserve = match attr(e, "space").as_deref() {
            Some("preserve") => true,
            Some("default") => false,
            _ => inherited,
        };

        if self.root_name.is_none() {
            self.root_name = Some(name.clone());
        }

        // Inside a captured text run every element is inline content.
        if let Some(c) = self.cap.as_mut() {
            match self.inline {
                InlineTags::Keep => c.buf.push_str(&raw_tag(e, self_closing)),
                InlineTags::Placeholder => {
                    if is_code_tag(&name) {
                        if c.code_depth == 0 {
                            c.buf.push_str(&marker(e));
                        }
                        if !self_closing {
                            c.code_depth += 1;
                        }
                    }
                }
                InlineTags::Strip => {
                    if is_code_tag(&name) && !self_closing {
                        c.code_depth += 1;
                    }
                }
            }
            return preserve;
        }

        match name.as_str() {
            "file" => {
                // 1.2 identifies a file by `original`, 2.x by `id`.
                let orig = attr(e, "original").unwrap_or_default();
                self.current_file = if orig.is_empty() { attr_or_empty(e, "id") } else { orig };
            }
            "group" => {
                let id = attr(e, "id").unwrap_or_default();
                let id = if id.is_empty() { attr_or_empty(e, "resname") } else { id };
                if !self_closing {
                    self.groups.push(id);
                }
            }
            "trans-unit" | "unit" => {
                let resname = attr(e, "resname").unwrap_or_default();
                let resname = if resname.is_empty() { attr_or_empty(e, "name") } else { resname };
                let unit = Unit {
                    id: attr_or_empty(e, "id"),
                    resname,
                    approved: attr_or_empty(e, "approved"),
                    file: self.current_file.clone(),
                    group: self.groups.iter().filter(|g| !g.is_empty()).cloned().collect::<Vec<_>>().join("/"),
                    ..Unit::default()
                };
                if self_closing {
                    self.units.push(unit);
                } else {
                    self.cur = Some(unit);
                }
            }
            "segment" if self.parent() == "unit" => {
                if let (Some(u), Some(state)) = (self.cur.as_mut(), attr(e, "state")) {
                    if u.state.is_empty() {
                        u.state = state;
                    }
                }
            }
            "source" | "target" if self.cur.is_some() => {
                let p = self.parent();
                if p == "trans-unit" || p == "segment" {
                    if name == "target" {
                        if let (Some(u), Some(state)) = (self.cur.as_mut(), attr(e, "state")) {
                            if u.state.is_empty() {
                                u.state = state;
                            }
                        }
                    }
                    let field = if name == "source" { Field::Source } else { Field::Target };
                    if self_closing {
                        // `<target/>` — an explicitly empty translation.
                        self.commit(field, String::new(), preserve);
                    } else {
                        self.cap = Some(Capture {
                            field,
                            depth: self.path.len(),
                            buf: String::new(),
                            preserve,
                            code_depth: 0,
                        });
                    }
                }
            }
            "note" if self.cur.is_some() => {
                let p = self.parent();
                let in_12 = p == "trans-unit";
                let in_2x = p == "notes" && self.grandparent() == "unit";
                if (in_12 || in_2x) && !self_closing {
                    self.cap = Some(Capture {
                        field: Field::Note,
                        depth: self.path.len(),
                        buf: String::new(),
                        preserve,
                        code_depth: 0,
                    });
                }
            }
            _ => {}
        }
        preserve
    }

    /// Store a finished text run on the current unit.
    fn commit(&mut self, field: Field, text: String, preserve: bool) {
        let text = if preserve { text } else { trim_indentation(&text) };
        let Some(u) = self.cur.as_mut() else { return };
        match field {
            // Multiple 2.x `<segment>`s partition one string — concatenate.
            Field::Source => u.source.push_str(&text),
            Field::Target => u.target.push_str(&text),
            Field::Note => {
                if text.is_empty() {
                    return;
                }
                if !u.note.is_empty() {
                    u.note.push('\n');
                }
                u.note.push_str(&text);
            }
        }
    }

    fn close(&mut self, name: &str) {
        // End of a captured text run?
        if let Some(c) = self.cap.as_mut() {
            if self.path.len() == c.depth {
                let c = self.cap.take().expect("capture present");
                self.commit(c.field, c.buf, c.preserve);
                return;
            }
            match self.inline {
                InlineTags::Keep => c.buf.push_str(&format!("</{name}>")),
                _ => {
                    if is_code_tag(name) && c.code_depth > 0 {
                        c.code_depth -= 1;
                    }
                }
            }
            return;
        }

        match name {
            "group" => {
                self.groups.pop();
            }
            "trans-unit" | "unit" => {
                if let Some(u) = self.cur.take() {
                    self.units.push(u);
                }
            }
            "file" => self.current_file.clear(),
            _ => {}
        }
    }

    fn text(&mut self, s: &str) {
        if let Some(c) = self.cap.as_mut() {
            if self.inline == InlineTags::Keep || c.code_depth == 0 {
                c.buf.push_str(s);
            }
        }
    }
}

/// Parse an XLIFF document into translation units, in document order.
pub fn extract_units(xliff: &str, inline: InlineTags) -> Result<Vec<Unit>, String> {
    if xliff.trim().is_empty() {
        return Err("input XLIFF is empty — paste an .xlf/.xliff document".to_string());
    }

    let mut reader = Reader::from_str(xliff);
    let config = reader.config_mut();
    config.trim_text(false);
    config.expand_empty_elements = false;

    let mut p = Parser {
        inline,
        units: Vec::new(),
        path: Vec::new(),
        preserve: vec![false],
        groups: Vec::new(),
        current_file: String::new(),
        cur: None,
        cap: None,
        root_name: None,
        _marker: std::marker::PhantomData,
    };
    let mut buf = Vec::new();

    loop {
        let ev = reader
            .read_event_into(&mut buf)
            .map_err(|e| format!("XML parse error at byte {}: {e}", reader.buffer_position()))?;
        match ev {
            Event::Eof => break,
            Event::Start(e) => {
                let preserve = p.open(&e, false);
                p.path.push(local_name(e.name().as_ref()));
                p.preserve.push(preserve);
            }
            Event::Empty(e) => {
                p.open(&e, true);
            }
            Event::End(e) => {
                let name = local_name(e.name().as_ref());
                p.path.pop();
                p.preserve.pop();
                p.close(&name);
            }
            Event::Text(t) => {
                let txt = t.unescape().map_err(|e| {
                    format!("XML text decode error at byte {}: {e}", reader.buffer_position())
                })?;
                p.text(&txt);
            }
            Event::CData(t) => {
                let txt = String::from_utf8_lossy(&t.into_inner()).into_owned();
                p.text(&txt);
            }
            // Comments, PIs, declarations and doctype carry no unit content.
            _ => {}
        }
        buf.clear();
    }

    if p.units.is_empty() {
        let root = p.root_name.unwrap_or_default();
        let detail = if root.is_empty() {
            "the document has no root element".to_string()
        } else if root == "xliff" {
            "the <xliff> root contains none".to_string()
        } else {
            format!("the document root is <{root}>, not <xliff>")
        };
        return Err(format!(
            "no translation units found: expected <trans-unit> (XLIFF 1.2) or <unit> (XLIFF 2.x) elements, but {detail}"
        ));
    }

    Ok(p.units)
}

/// Resolve one unit's object key under the selected `key` mode.
fn key_of(u: &Unit, key: KeyBy) -> String {
    match key {
        KeyBy::Id => u.id.clone(),
        // CAT tools populate resname inconsistently — fall back rather than drop.
        KeyBy::Resname => {
            if u.resname.is_empty() {
                u.id.clone()
            } else {
                u.resname.clone()
            }
        }
        KeyBy::SourceText => {
            if u.source.is_empty() {
                u.id.clone()
            } else {
                u.source.clone()
            }
        }
    }
}

/// Add the optional metadata members to a record.
fn add_metadata(map: &mut Map<String, Value>, u: &Unit) {
    for (k, v) in [
        ("resname", &u.resname),
        ("note", &u.note),
        ("state", &u.state),
        ("approved", &u.approved),
        ("file", &u.file),
        ("group", &u.group),
    ] {
        if !v.is_empty() {
            map.insert(k.to_string(), Value::String(v.clone()));
        }
    }
}

/// Build one unit's value for the pairs/array shapes.
fn record(u: &Unit, opt: &Options, with_id: bool) -> Value {
    let mut map = Map::new();
    if with_id {
        map.insert("id".to_string(), Value::String(u.id.clone()));
    }
    map.insert("source".to_string(), Value::String(u.source.clone()));
    map.insert("target".to_string(), Value::String(effective_target(u, opt)));
    if opt.include_metadata {
        add_metadata(&mut map, u);
    }
    Value::Object(map)
}

/// The target text after the source-fallback rule.
fn effective_target(u: &Unit, opt: &Options) -> String {
    if u.target.is_empty() && opt.fallback_to_source {
        u.source.clone()
    } else {
        u.target.clone()
    }
}

/// Insert `value` at the `parts` path, erroring on a leaf/branch collision.
fn insert_nested(
    root: &mut Map<String, Value>,
    parts: &[&str],
    value: Value,
    full_key: &str,
) -> Result<(), String> {
    let (head, rest) = parts.split_first().expect("non-empty path");
    if rest.is_empty() {
        // Last wins, matching the flat shape's duplicate-key rule.
        root.insert((*head).to_string(), value);
        return Ok(());
    }
    let entry = root.entry((*head).to_string()).or_insert_with(|| Value::Object(Map::new()));
    match entry {
        Value::Object(child) => insert_nested(child, rest, value, full_key),
        _ => Err(format!(
            "cannot nest key '{full_key}': '{head}' already holds a value. Turn nesting off or pick a separator the ids do not contain."
        )),
    }
}

/// Build the JSON value for the extracted units.
pub fn to_value(units: &[Unit], opt: &Options) -> Result<Value, String> {
    if opt.nested && opt.separator.is_empty() {
        return Err("separator must not be empty when nested output is enabled".to_string());
    }

    let kept: Vec<&Unit> = units
        .iter()
        .filter(|u| opt.include_empty_targets || !u.target.is_empty())
        .collect();

    if opt.output == Shape::Array {
        return Ok(Value::Array(kept.iter().map(|u| record(u, opt, true)).collect()));
    }

    let flat: Vec<(String, Value)> = kept
        .iter()
        .map(|u| {
            let value = match opt.output {
                Shape::Target => Value::String(effective_target(u, opt)),
                Shape::Source => Value::String(u.source.clone()),
                _ => record(u, opt, false),
            };
            (key_of(u, opt.key), value)
        })
        .collect();

    if !opt.nested {
        let mut map = Map::new();
        for (k, v) in flat {
            map.insert(k, v);
        }
        return Ok(Value::Object(map));
    }

    let mut root = Map::new();
    for (k, v) in flat {
        let parts: Vec<&str> = k.split(opt.separator.as_str()).filter(|p| !p.is_empty()).collect();
        if parts.is_empty() {
            return Err(format!(
                "cannot nest key '{k}': it is empty once split on '{}'",
                opt.separator
            ));
        }
        insert_nested(&mut root, &parts, v, &k)?;
    }
    Ok(Value::Object(root))
}

/// Parse the string form of every option, then convert. This is the single
/// entry point the block, the CLI and the page all call.
#[allow(clippy::too_many_arguments)]
pub fn run(
    xliff: &str,
    output: &str,
    key: &str,
    nested: bool,
    separator: &str,
    include_empty_targets: bool,
    fallback_to_source: bool,
    inline_tags: &str,
    include_metadata: bool,
) -> Result<String, String> {
    let opt = Options {
        output: match output {
            "pairs" => Shape::Pairs,
            "target" => Shape::Target,
            "source" => Shape::Source,
            "array" => Shape::Array,
            other => {
                return Err(format!(
                    "output must be pairs, target, source or array; got '{other}'"
                ))
            }
        },
        key: match key {
            "id" => KeyBy::Id,
            "resname" => KeyBy::Resname,
            "source" => KeyBy::SourceText,
            other => return Err(format!("key must be id, resname or source; got '{other}'")),
        },
        inline_tags: match inline_tags {
            "placeholder" => InlineTags::Placeholder,
            "strip" => InlineTags::Strip,
            "keep" => InlineTags::Keep,
            other => {
                return Err(format!(
                    "inline_tags must be placeholder, strip or keep; got '{other}'"
                ))
            }
        },
        nested,
        separator: separator.to_string(),
        include_empty_targets,
        fallback_to_source,
        include_metadata,
    };

    let units = extract_units(xliff, opt.inline_tags)?;
    let value = to_value(&units, &opt)?;
    serde_json::to_string_pretty(&value).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const XLIFF_12: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<xliff version="1.2" xmlns="urn:oasis:names:tc:xliff:document:1.2">
  <file original="app.ts" source-language="en" target-language="de" datatype="plaintext">
    <body>
      <trans-unit id="greeting" resname="home.greeting" approved="yes">
        <source>Hello</source>
        <target state="translated">Hallo</target>
        <note>Shown on the home page</note>
      </trans-unit>
      <trans-unit id="farewell">
        <source>Goodbye</source>
        <target state="new"></target>
      </trans-unit>
    </body>
  </file>
</xliff>"#;

    const XLIFF_2X: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<xliff xmlns="urn:oasis:names:tc:xliff:document:2.0" version="2.0" srcLang="en" trgLang="fr">
  <file id="f1" original="app.ts">
    <unit id="greeting" name="home.greeting">
      <notes><note>Home page</note></notes>
      <segment state="translated">
        <source>Hello</source>
        <target>Bonjour</target>
      </segment>
      <segment state="translated">
        <source> world</source>
        <target> le monde</target>
      </segment>
    </unit>
    <unit id="farewell">
      <segment>
        <source>Goodbye</source>
      </segment>
    </unit>
  </file>
</xliff>"#;

    fn units_12() -> Vec<Unit> {
        extract_units(XLIFF_12, InlineTags::Placeholder).unwrap()
    }

    #[test]
    fn extracts_xliff_12_trans_units() {
        let u = units_12();
        assert_eq!(u.len(), 2);
        assert_eq!(u[0].id, "greeting");
        assert_eq!(u[0].resname, "home.greeting");
        assert_eq!(u[0].source, "Hello");
        assert_eq!(u[0].target, "Hallo");
        assert_eq!(u[0].note, "Shown on the home page");
        assert_eq!(u[0].state, "translated");
        assert_eq!(u[0].approved, "yes");
        assert_eq!(u[0].file, "app.ts");
        assert_eq!(u[1].target, "");
    }

    #[test]
    fn extracts_xliff_2x_units_and_joins_segments() {
        let u = extract_units(XLIFF_2X, InlineTags::Placeholder).unwrap();
        assert_eq!(u.len(), 2);
        assert_eq!(u[0].source, "Hello world");
        assert_eq!(u[0].target, "Bonjour le monde");
        assert_eq!(u[0].resname, "home.greeting");
        assert_eq!(u[0].note, "Home page");
        assert_eq!(u[0].state, "translated");
        assert_eq!(u[0].file, "app.ts");
        assert_eq!(u[1].target, "");
    }

    #[test]
    fn default_output_is_source_target_pairs_by_id() {
        let json = run(XLIFF_12, "pairs", "id", false, ".", true, false, "placeholder", false).unwrap();
        assert_eq!(
            json,
            "{\n  \"greeting\": {\n    \"source\": \"Hello\",\n    \"target\": \"Hallo\"\n  },\n  \"farewell\": {\n    \"source\": \"Goodbye\",\n    \"target\": \"\"\n  }\n}"
        );
    }

    #[test]
    fn target_shape_emits_a_flat_bundle() {
        let json = run(XLIFF_12, "target", "id", false, ".", true, false, "placeholder", false).unwrap();
        let v: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["greeting"], "Hallo");
        assert_eq!(v["farewell"], "");
    }

    #[test]
    fn source_fallback_fills_untranslated_units() {
        let json = run(XLIFF_12, "target", "id", false, ".", true, true, "placeholder", false).unwrap();
        let v: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["farewell"], "Goodbye");
    }

    #[test]
    fn empty_targets_can_be_filtered_out() {
        let json = run(XLIFF_12, "pairs", "id", false, ".", false, false, "placeholder", false).unwrap();
        let v: Value = serde_json::from_str(&json).unwrap();
        assert!(v.get("greeting").is_some());
        assert!(v.get("farewell").is_none());
    }

    #[test]
    fn filter_wins_over_fallback() {
        // An untranslated unit stays filtered out even when fallback is on.
        let json = run(XLIFF_12, "target", "id", false, ".", false, true, "placeholder", false).unwrap();
        let v: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v.as_object().unwrap().len(), 1);
        assert!(v.get("farewell").is_none());
    }

    #[test]
    fn keys_by_resname_falling_back_to_id() {
        let json = run(XLIFF_12, "target", "resname", false, ".", true, false, "placeholder", false).unwrap();
        let v: Value = serde_json::from_str(&json).unwrap();
        assert!(v.get("home.greeting").is_some());
        // farewell has no resname → keyed by id
        assert!(v.get("farewell").is_some());
    }

    #[test]
    fn keys_by_source_text() {
        let json = run(XLIFF_12, "target", "source", false, ".", true, false, "placeholder", false).unwrap();
        let v: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["Hello"], "Hallo");
    }

    #[test]
    fn array_shape_preserves_order_and_ids() {
        let json = run(XLIFF_12, "array", "id", false, ".", true, false, "placeholder", false).unwrap();
        let v: Value = serde_json::from_str(&json).unwrap();
        let arr = v.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["id"], "greeting");
        assert_eq!(arr[1]["id"], "farewell");
    }

    #[test]
    fn metadata_is_opt_in() {
        let plain = run(XLIFF_12, "pairs", "id", false, ".", true, false, "placeholder", false).unwrap();
        assert!(!plain.contains("\"state\""));
        let rich = run(XLIFF_12, "pairs", "id", false, ".", true, false, "placeholder", true).unwrap();
        let v: Value = serde_json::from_str(&rich).unwrap();
        assert_eq!(v["greeting"]["state"], "translated");
        assert_eq!(v["greeting"]["approved"], "yes");
        assert_eq!(v["greeting"]["note"], "Shown on the home page");
        assert_eq!(v["greeting"]["file"], "app.ts");
        assert_eq!(v["greeting"]["resname"], "home.greeting");
    }

    #[test]
    fn nested_output_splits_dotted_ids() {
        let x = r#"<xliff version="1.2"><file original="a"><body>
          <trans-unit id="home.title"><source>T</source><target>Titel</target></trans-unit>
          <trans-unit id="home.body"><source>B</source><target>Text</target></trans-unit>
        </body></file></xliff>"#;
        let json = run(x, "target", "id", true, ".", true, false, "placeholder", false).unwrap();
        let v: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["home"]["title"], "Titel");
        assert_eq!(v["home"]["body"], "Text");
    }

    #[test]
    fn nested_output_reports_a_leaf_branch_collision() {
        let x = r#"<xliff version="1.2"><file original="a"><body>
          <trans-unit id="home"><source>H</source><target>H</target></trans-unit>
          <trans-unit id="home.title"><source>T</source><target>T</target></trans-unit>
        </body></file></xliff>"#;
        let err = run(x, "target", "id", true, ".", true, false, "placeholder", false).unwrap_err();
        assert!(err.contains("cannot nest key 'home.title'"), "got: {err}");
        assert!(err.contains("already holds a value"), "got: {err}");
    }

    #[test]
    fn nested_output_rejects_an_empty_separator() {
        let err = run(XLIFF_12, "target", "id", true, "", true, false, "placeholder", false).unwrap_err();
        assert_eq!(err, "separator must not be empty when nested output is enabled");
    }

    const INLINE: &str = r#"<xliff version="1.2"><file original="a"><body>
      <trans-unit id="hi">
        <source>Hello <x id="INTERPOLATION" equiv-text="{{name}}"/>, <bpt id="1">&lt;b&gt;</bpt>welcome<ept id="1">&lt;/b&gt;</ept>!</source>
        <target>Hallo <x id="INTERPOLATION" equiv-text="{{name}}"/>, <bpt id="1">&lt;b&gt;</bpt>willkommen<ept id="1">&lt;/b&gt;</ept>!</target>
      </trans-unit>
    </body></file></xliff>"#;

    #[test]
    fn inline_placeholder_keeps_interpolations() {
        let u = extract_units(INLINE, InlineTags::Placeholder).unwrap();
        assert_eq!(u[0].source, "Hello {{name}}, {1}welcome{1}!");
    }

    #[test]
    fn inline_strip_drops_markup_and_code_text() {
        let u = extract_units(INLINE, InlineTags::Strip).unwrap();
        assert_eq!(u[0].source, "Hello , welcome!");
        assert_eq!(u[0].target, "Hallo , willkommen!");
    }

    #[test]
    fn inline_keep_preserves_inner_xml() {
        let u = extract_units(INLINE, InlineTags::Keep).unwrap();
        assert!(u[0].source.contains(r#"<x id="INTERPOLATION" equiv-text="{{name}}"/>"#));
        assert!(u[0].source.contains("<bpt id=\"1\">"));
    }

    #[test]
    fn group_wrapped_units_are_walked_not_rejected() {
        let x = r#"<xliff version="1.2"><file original="app"><body>
          <group id="menu"><group id="file">
            <trans-unit id="open"><source>Open</source><target>Öffnen</target></trans-unit>
          </group></group>
        </body></file></xliff>"#;
        let json = run(x, "pairs", "id", false, ".", true, false, "placeholder", true).unwrap();
        let v: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["open"]["target"], "Öffnen");
        assert_eq!(v["open"]["group"], "menu/file");
    }

    #[test]
    fn alt_trans_and_seg_source_are_ignored() {
        let x = r#"<xliff version="1.2"><file original="a"><body>
          <trans-unit id="u1">
            <source>Cat</source>
            <seg-source><mrk mtype="seg" mid="1">Cat</mrk></seg-source>
            <target>Katze</target>
            <alt-trans match-quality="80%"><source>Cat</source><target>Kater</target></alt-trans>
          </trans-unit>
        </body></file></xliff>"#;
        let u = extract_units(x, InlineTags::Placeholder).unwrap();
        assert_eq!(u.len(), 1);
        assert_eq!(u[0].source, "Cat");
        assert_eq!(u[0].target, "Katze");
    }

    #[test]
    fn xml_space_preserve_keeps_padding() {
        let x = r#"<xliff version="1.2"><file original="a"><body>
          <trans-unit id="pad" xml:space="preserve"><source>  spaced  </source><target>  weit  </target></trans-unit>
          <trans-unit id="trim"><source>
            trimmed
          </source><target>gekürzt</target></trans-unit>
        </body></file></xliff>"#;
        let u = extract_units(x, InlineTags::Placeholder).unwrap();
        assert_eq!(u[0].source, "  spaced  ");
        assert_eq!(u[1].source, "trimmed");
    }

    #[test]
    fn only_newline_spanning_indentation_is_trimmed() {
        // Same-line padding is part of the string (it is what joins 2.x
        // segments); indentation that wraps a newline is the file's layout.
        let x = r#"<xliff version="1.2"><file original="a"><body>
          <trans-unit id="pad"><source>Hello </source><target> Hallo</target></trans-unit>
          <trans-unit id="wrapped"><source>
            Goodbye
          </source><target>Tschüss</target></trans-unit>
        </body></file></xliff>"#;
        let u = extract_units(x, InlineTags::Placeholder).unwrap();
        assert_eq!(u[0].source, "Hello ");
        assert_eq!(u[0].target, " Hallo");
        assert_eq!(u[1].source, "Goodbye");
    }

    #[test]
    fn cdata_and_entities_are_decoded() {
        let x = r#"<xliff version="1.2"><file original="a"><body>
          <trans-unit id="e"><source>A &amp; B</source><target><![CDATA[C & D]]></target></trans-unit>
        </body></file></xliff>"#;
        let u = extract_units(x, InlineTags::Placeholder).unwrap();
        assert_eq!(u[0].source, "A & B");
        assert_eq!(u[0].target, "C & D");
    }

    #[test]
    fn duplicate_ids_last_wins_in_objects_and_all_survive_in_arrays() {
        let x = r#"<xliff version="1.2">
          <file original="a"><body><trans-unit id="k"><source>A</source><target>Eins</target></trans-unit></body></file>
          <file original="b"><body><trans-unit id="k"><source>A</source><target>Zwei</target></trans-unit></body></file>
        </xliff>"#;
        let obj: Value =
            serde_json::from_str(&run(x, "target", "id", false, ".", true, false, "placeholder", false).unwrap())
                .unwrap();
        assert_eq!(obj["k"], "Zwei");
        let arr: Value =
            serde_json::from_str(&run(x, "array", "id", false, ".", true, false, "placeholder", true).unwrap())
                .unwrap();
        assert_eq!(arr.as_array().unwrap().len(), 2);
        assert_eq!(arr[0]["file"], "a");
        assert_eq!(arr[1]["file"], "b");
    }

    #[test]
    fn empty_input_is_an_error() {
        let err = run("   ", "pairs", "id", false, ".", true, false, "placeholder", false).unwrap_err();
        assert!(err.contains("input XLIFF is empty"), "got: {err}");
    }

    #[test]
    fn malformed_xml_reports_a_position() {
        let err = run(
            "<xliff version=\"1.2\"><file><body><trans-unit id=\"a\"><source>x</target></body>",
            "pairs",
            "id",
            false,
            ".",
            true,
            false,
            "placeholder",
            false,
        )
        .unwrap_err();
        assert!(err.starts_with("XML parse error at byte"), "got: {err}");
    }

    #[test]
    fn non_xliff_input_names_the_root_it_found() {
        let err = run(
            "<rss version=\"2.0\"><channel><title>t</title></channel></rss>",
            "pairs",
            "id",
            false,
            ".",
            true,
            false,
            "placeholder",
            false,
        )
        .unwrap_err();
        assert!(err.contains("<trans-unit>"), "got: {err}");
        assert!(err.contains("the document root is <rss>"), "got: {err}");
    }

    #[test]
    fn empty_xliff_root_says_so() {
        let err = run(
            "<xliff version=\"1.2\"><file original=\"a\"><body></body></file></xliff>",
            "pairs",
            "id",
            false,
            ".",
            true,
            false,
            "placeholder",
            false,
        )
        .unwrap_err();
        assert!(err.contains("the <xliff> root contains none"), "got: {err}");
    }

    #[test]
    fn bad_enum_values_are_rejected_by_name() {
        assert!(run(XLIFF_12, "nope", "id", false, ".", true, false, "placeholder", false)
            .unwrap_err()
            .contains("output must be pairs, target, source or array"));
        assert!(run(XLIFF_12, "pairs", "nope", false, ".", true, false, "placeholder", false)
            .unwrap_err()
            .contains("key must be id, resname or source"));
        assert!(run(XLIFF_12, "pairs", "id", false, ".", true, false, "nope", false)
            .unwrap_err()
            .contains("inline_tags must be placeholder, strip or keep"));
    }
}
