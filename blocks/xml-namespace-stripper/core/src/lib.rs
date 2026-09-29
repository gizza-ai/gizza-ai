//! gizza-ai/xml-namespace-stripper core — remove XML namespace declarations
//! (`xmlns`, `xmlns:prefix`) and element/attribute name prefixes, leaving every
//! other byte of the document alone. Pure-Rust (`quick-xml`), streaming; no
//! wafer/wasm-bindgen deps.
//!
//! Comments, processing instructions, CDATA, the DOCTYPE, the XML declaration,
//! text nodes and attribute values (entities and character references included)
//! round-trip through the reader/writer untouched. The reserved `xml` prefix is
//! never stripped — it is bound by the XML spec itself and never declared, so
//! `xml:lang` / `xml:space` / `xml:id` would lose their meaning.

use quick_xml::events::attributes::Attribute;
use quick_xml::events::{BytesEnd, BytesStart, Event};
use quick_xml::name::QName;
use quick_xml::reader::Reader;
use quick_xml::writer::Writer;
use std::borrow::Cow;

/// Largest document accepted, in bytes.
pub const MAX_INPUT: usize = 5_000_000;

/// The XML Schema instance namespace. Its `schemaLocation` /
/// `noNamespaceSchemaLocation` attributes reference namespaces a stripped
/// document no longer declares, so they are dangling once the prefixes go.
const XSI_NS: &[u8] = b"http://www.w3.org/2001/XMLSchema-instance";
const SCHEMA_HINTS: [&[u8]; 2] = [b"schemaLocation", b"noNamespaceSchemaLocation"];
/// Bound by the XML spec, never declared — see the module docs.
const RESERVED_PREFIX: &[u8] = b"xml";
/// Guard against a pathological rename loop; far above any real document.
const MAX_RENAME_TRIES: usize = 1000;

/// What to remove.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Declarations *and* prefixes (the default).
    All,
    /// Name prefixes only; the `xmlns` attributes stay.
    Prefixes,
    /// The `xmlns` attributes only; prefixed names stay.
    Declarations,
}

impl Mode {
    pub fn parse(s: &str) -> Result<Mode, String> {
        match s.trim() {
            "" | "all" => Ok(Mode::All),
            "prefixes" => Ok(Mode::Prefixes),
            "declarations" => Ok(Mode::Declarations),
            other => Err(format!(
                "unknown mode '{other}' — use 'all' (declarations and prefixes), \
                 'prefixes' (names only, keep the xmlns attributes) or \
                 'declarations' (xmlns attributes only, keep the prefixed names)"
            )),
        }
    }

    fn strips_prefixes(self) -> bool {
        matches!(self, Mode::All | Mode::Prefixes)
    }

    fn strips_decls(self) -> bool {
        matches!(self, Mode::All | Mode::Declarations)
    }

    fn as_str(self) -> &'static str {
        match self {
            Mode::All => "all",
            Mode::Prefixes => "prefixes",
            Mode::Declarations => "declarations",
        }
    }
}

/// What to do when two attributes on one element collapse onto the same name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnConflict {
    /// Keep both: the later one becomes `prefix_localname` (the default).
    Rename,
    /// Keep the first, drop the later one.
    First,
    /// Refuse, naming the clashing pair.
    Error,
}

impl OnConflict {
    pub fn parse(s: &str) -> Result<OnConflict, String> {
        match s.trim() {
            "" | "rename" => Ok(OnConflict::Rename),
            "first" => Ok(OnConflict::First),
            "error" => Ok(OnConflict::Error),
            other => Err(format!(
                "unknown conflicts '{other}' — use 'rename' (keep both, later one \
                 becomes prefix_name), 'first' (keep the first, drop the later one) \
                 or 'error' (refuse and name the pair)"
            )),
        }
    }
}

/// How the result is laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// Keep the document's own whitespace (the default).
    Preserve,
    /// Re-indent.
    Pretty,
    /// Collapse onto one line.
    Minify,
}

impl Layout {
    pub fn parse(s: &str) -> Result<Layout, String> {
        match s.trim() {
            "" | "preserve" => Ok(Layout::Preserve),
            "pretty" => Ok(Layout::Pretty),
            "minify" => Ok(Layout::Minify),
            other => Err(format!(
                "unknown format '{other}' — use 'preserve' (keep the document's own \
                 whitespace), 'pretty' (re-indent) or 'minify' (one line)"
            )),
        }
    }

    fn reflows(self) -> bool {
        matches!(self, Layout::Pretty | Layout::Minify)
    }
}

/// What to return.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    /// The stripped XML (the default).
    Xml,
    /// A CSV summary of what was removed.
    Report,
}

impl Output {
    pub fn parse(s: &str) -> Result<Output, String> {
        match s.trim() {
            "" | "xml" => Ok(Output::Xml),
            "report" => Ok(Output::Report),
            other => Err(format!(
                "unknown output '{other}' — use 'xml' (the stripped document) or \
                 'report' (a CSV summary of what was removed)"
            )),
        }
    }
}

/// Resolved options, so the per-element helpers take one argument.
struct Opts {
    mode: Mode,
    keep: Vec<Vec<u8>>,
    on_conflict: OnConflict,
    remove_schema_hints: bool,
}

impl Opts {
    /// A prefix may be stripped unless it is the reserved `xml` prefix or the
    /// caller asked to keep it.
    fn strippable(&self, prefix: &[u8]) -> bool {
        prefix != RESERVED_PREFIX && !self.keep.iter().any(|k| k.as_slice() == prefix)
    }

    /// Is the declaration for `prefix` (empty = the default namespace) kept?
    /// The literal token `xmlns` in `keep` protects the default declaration,
    /// which has no prefix to name.
    fn keeps_decl(&self, prefix: &[u8]) -> bool {
        let wanted: &[u8] = if prefix.is_empty() { b"xmlns" } else { prefix };
        self.keep.iter().any(|k| k.as_slice() == wanted)
    }
}

/// One attribute on its way to the writer.
struct Attr {
    /// Name as it will be written.
    name: Vec<u8>,
    /// Raw (still-escaped) value bytes.
    value: Vec<u8>,
    /// Source prefix, for the rename-on-clash rule.
    prefix: Option<Vec<u8>>,
    /// An `xmlns`/`xmlns:prefix` declaration that survived.
    decl: bool,
    /// Name as it was written in the source, for error messages.
    original: Vec<u8>,
}

/// Counters behind `output=report`.
#[derive(Default)]
struct Stats {
    /// Unique removed declarations as (prefix, namespace URI), in source order.
    removed: Vec<(Vec<u8>, Vec<u8>)>,
    element_prefixes: usize,
    attr_prefixes: usize,
    schema_hints: usize,
    conflicts: usize,
}

impl Stats {
    fn record_decl(&mut self, prefix: &[u8], uri: &[u8]) {
        let seen = self
            .removed
            .iter()
            .any(|(p, u)| p.as_slice() == prefix && u.as_slice() == uri);
        if !seen {
            self.removed.push((prefix.to_vec(), uri.to_vec()));
        }
    }
}

/// Strip namespaces from `xml`.
///
/// * `mode` — `all` (default) | `prefixes` | `declarations`.
/// * `keep` — comma-separated prefixes to leave alone, with their declarations;
///   the token `xmlns` protects the default (unprefixed) declaration.
/// * `conflicts` — `rename` (default) | `first` | `error`, for two attributes
///   that collapse onto one name.
/// * `remove_schema_hints` — also drop `schemaLocation` /
///   `noNamespaceSchemaLocation` attributes whose prefix resolves to the
///   XML-Schema-instance namespace.
/// * `format` — `preserve` (default) | `pretty` | `minify`.
/// * `indent` — spaces per level in `pretty` (0–16).
/// * `output` — `xml` (default) | `report`.
#[allow(clippy::too_many_arguments)]
pub fn strip(
    xml: &str,
    mode: &str,
    keep: &str,
    conflicts: &str,
    remove_schema_hints: bool,
    format: &str,
    indent: usize,
    output: &str,
) -> Result<String, String> {
    let opts = Opts {
        mode: Mode::parse(mode)?,
        keep: keep
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.as_bytes().to_vec())
            .collect(),
        on_conflict: OnConflict::parse(conflicts)?,
        remove_schema_hints,
    };
    let layout = Layout::parse(format)?;
    let output = Output::parse(output)?;

    if xml.trim().is_empty() {
        return Err("no XML input — paste an XML document to strip".into());
    }
    if xml.len() > MAX_INPUT {
        return Err(format!(
            "XML is {} bytes; the limit is {MAX_INPUT} bytes",
            xml.len()
        ));
    }

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(layout.reflows());
    let mut writer = match layout {
        Layout::Pretty => Writer::new_with_indent(Vec::new(), b' ', indent.min(16)),
        _ => Writer::new(Vec::new()),
    };

    // One frame of prefix → namespace URI bindings per open element, so a
    // schema-hint attribute is matched by the namespace its prefix actually
    // resolves to rather than by the prefix's spelling.
    let mut scopes: Vec<Vec<(Vec<u8>, Vec<u8>)>> = Vec::new();
    let mut stats = Stats::default();

    loop {
        match reader.read_event() {
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) => {
                scopes.push(collect_decls(&e)?);
                match rewrite_element(&e, &opts, &scopes, &mut stats)? {
                    Some(ne) => writer.write_event(Event::Start(ne)),
                    None => writer.write_event(Event::Start(e)),
                }
                .map_err(write_err)?;
            }
            Ok(Event::Empty(e)) => {
                scopes.push(collect_decls(&e)?);
                let rewritten = rewrite_element(&e, &opts, &scopes, &mut stats)?;
                scopes.pop();
                match rewritten {
                    Some(ne) => writer.write_event(Event::Empty(ne)),
                    None => writer.write_event(Event::Empty(e)),
                }
                .map_err(write_err)?;
            }
            Ok(Event::End(e)) => {
                scopes.pop();
                let bare = {
                    let name = e.name();
                    let (prefix, local) = split_qname(name.as_ref());
                    match prefix {
                        Some(p) if opts.mode.strips_prefixes() && opts.strippable(p) => {
                            Some(lossy(local))
                        }
                        _ => None,
                    }
                };
                match bare {
                    Some(local) => writer.write_event(Event::End(BytesEnd::new(local))),
                    None => writer.write_event(Event::End(e)),
                }
                .map_err(write_err)?;
            }
            Ok(event) => writer.write_event(event).map_err(write_err)?,
            Err(e) => {
                return Err(format!(
                    "XML is not well-formed at byte {}: {e}",
                    reader.error_position()
                ))
            }
        }
    }

    let out = String::from_utf8(writer.into_inner())
        .map_err(|e| format!("output is not valid UTF-8: {e}"))?;

    match output {
        Output::Xml => Ok(out),
        Output::Report => Ok(report(&stats, opts.mode, xml.len(), out.len())),
    }
}

/// Rewrite one element's name + attributes. `None` means nothing about this
/// element changed, so the caller writes the original event and the source's
/// own attribute spacing and quoting survive.
fn rewrite_element(
    e: &BytesStart,
    opts: &Opts,
    scopes: &[Vec<(Vec<u8>, Vec<u8>)>],
    stats: &mut Stats,
) -> Result<Option<BytesStart<'static>>, String> {
    let name = e.name();
    let (prefix, local) = split_qname(name.as_ref());
    let mut changed = false;
    let new_name: &[u8] = match prefix {
        Some(p) if opts.mode.strips_prefixes() && opts.strippable(p) => {
            changed = true;
            stats.element_prefixes += 1;
            local
        }
        _ => name.as_ref(),
    };

    let mut attrs: Vec<Attr> = Vec::new();
    for a in e.attributes() {
        let a = a.map_err(|err| attr_err(name.as_ref(), &err.to_string()))?;
        let key = a.key.as_ref();

        if let Some(decl_prefix) = decl_prefix(key) {
            if opts.mode.strips_decls() && !opts.keeps_decl(decl_prefix) {
                changed = true;
                stats.record_decl(decl_prefix, &a.value);
                continue;
            }
            attrs.push(Attr {
                name: key.to_vec(),
                value: a.value.to_vec(),
                prefix: None,
                decl: true,
                original: key.to_vec(),
            });
            continue;
        }

        let (aprefix, alocal) = split_qname(key);
        let dangling_hint = opts.remove_schema_hints
            && SCHEMA_HINTS.contains(&alocal)
            && aprefix.is_some_and(|p| resolve(scopes, p) == Some(XSI_NS));
        if dangling_hint {
            changed = true;
            stats.schema_hints += 1;
            continue;
        }

        let new_key = match aprefix {
            Some(p) if opts.mode.strips_prefixes() && opts.strippable(p) => {
                changed = true;
                stats.attr_prefixes += 1;
                alocal.to_vec()
            }
            _ => key.to_vec(),
        };
        attrs.push(Attr {
            name: new_key,
            value: a.value.to_vec(),
            prefix: aprefix.map(<[u8]>::to_vec),
            decl: false,
            original: key.to_vec(),
        });
    }

    // Two attributes can collapse onto one name (`xsi:type` next to a plain
    // `type`, or `a:id` next to `b:id`) — an outright XML name clash that the
    // usual local-name() recipe emits as broken markup.
    let mut seen: Vec<Vec<u8>> = Vec::new();
    let mut resolved: Vec<Attr> = Vec::with_capacity(attrs.len());
    for mut a in attrs {
        if a.decl || !seen.iter().any(|s| *s == a.name) {
            if !a.decl {
                seen.push(a.name.clone());
            }
            resolved.push(a);
            continue;
        }
        match opts.on_conflict {
            OnConflict::Error => {
                return Err(format!(
                    "attribute name clash on <{}>: '{}' becomes '{}', which another \
                     attribute already uses — use conflicts=rename to keep both (as \
                     '{}') or conflicts=first to drop the later one",
                    lossy(new_name),
                    lossy(&a.original),
                    lossy(&a.name),
                    lossy(&rename_target(&a, &seen))
                ));
            }
            OnConflict::First => {
                changed = true;
                stats.conflicts += 1;
            }
            OnConflict::Rename => {
                let target = rename_target(&a, &seen);
                changed = true;
                stats.conflicts += 1;
                a.name = target.clone();
                seen.push(target);
                resolved.push(a);
            }
        }
    }

    if !changed {
        return Ok(None);
    }

    let mut ne = BytesStart::new(lossy(new_name));
    for a in &resolved {
        ne.push_attribute(Attribute {
            key: QName(&a.name),
            value: Cow::Owned(requote(&a.value)),
        });
    }
    Ok(Some(ne.into_owned()))
}

/// Collect this element's own `xmlns` declarations. They are in scope for the
/// element itself, so this runs before its attributes are inspected.
fn collect_decls(e: &BytesStart) -> Result<Vec<(Vec<u8>, Vec<u8>)>, String> {
    let mut decls = Vec::new();
    for a in e.attributes() {
        let a = a.map_err(|err| attr_err(e.name().as_ref(), &err.to_string()))?;
        if let Some(prefix) = decl_prefix(a.key.as_ref()) {
            decls.push((prefix.to_vec(), a.value.to_vec()));
        }
    }
    Ok(decls)
}

/// `Some(b"")` for a default-namespace declaration, `Some(prefix)` for
/// `xmlns:prefix`, `None` for any other attribute.
fn decl_prefix(key: &[u8]) -> Option<&[u8]> {
    if key == b"xmlns" {
        Some(b"")
    } else {
        key.strip_prefix(b"xmlns:".as_slice())
    }
}

/// Split a qualified name into `(prefix, local)`. A name with no colon — or a
/// degenerate `:x` / `x:` — has no usable prefix and is left alone.
fn split_qname(name: &[u8]) -> (Option<&[u8]>, &[u8]) {
    match name.iter().position(|b| *b == b':') {
        Some(i) if i > 0 && i + 1 < name.len() => (Some(&name[..i]), &name[i + 1..]),
        _ => (None, name),
    }
}

/// Innermost binding for `prefix`, searching enclosing scopes outward.
fn resolve<'a>(scopes: &'a [Vec<(Vec<u8>, Vec<u8>)>], prefix: &[u8]) -> Option<&'a [u8]> {
    for frame in scopes.iter().rev() {
        for (p, uri) in frame.iter().rev() {
            if p.as_slice() == prefix {
                return Some(uri.as_slice());
            }
        }
    }
    None
}

/// `a:x` clashing with an existing `x` becomes `a_x`; if that is taken too (or
/// the clashing attribute had no prefix) a `_2`, `_3`, … suffix is appended.
fn rename_target(a: &Attr, seen: &[Vec<u8>]) -> Vec<u8> {
    let base = match &a.prefix {
        Some(p) => {
            let mut b = p.clone();
            b.push(b'_');
            b.extend_from_slice(&a.name);
            b
        }
        None => a.name.clone(),
    };
    if !seen.iter().any(|s| *s == base) {
        return base;
    }
    for n in 2..MAX_RENAME_TRIES {
        let mut candidate = base.clone();
        candidate.extend_from_slice(format!("_{n}").as_bytes());
        if !seen.iter().any(|s| *s == candidate) {
            return candidate;
        }
    }
    base
}

/// Attribute values are carried through RAW so entities and character
/// references survive byte-for-byte. The writer always uses double quotes, so a
/// value that was single-quoted in the source may hold a literal `"` — escape
/// exactly that one character and nothing else.
fn requote(raw: &[u8]) -> Vec<u8> {
    if !raw.contains(&b'"') {
        return raw.to_vec();
    }
    let mut out = Vec::with_capacity(raw.len() + 16);
    for b in raw {
        if *b == b'"' {
            out.extend_from_slice(b"&quot;");
        } else {
            out.push(*b);
        }
    }
    out
}

fn report(stats: &Stats, mode: Mode, input_len: usize, output_len: usize) -> String {
    let mut s = String::from("metric,value\n");
    s.push_str(&format!("mode,{}\n", mode.as_str()));
    s.push_str(&format!("declarations_removed,{}\n", stats.removed.len()));
    s.push_str(&format!(
        "element_prefixes_stripped,{}\n",
        stats.element_prefixes
    ));
    s.push_str(&format!(
        "attribute_prefixes_stripped,{}\n",
        stats.attr_prefixes
    ));
    s.push_str(&format!(
        "schema_references_removed,{}\n",
        stats.schema_hints
    ));
    s.push_str(&format!(
        "attribute_clashes_resolved,{}\n",
        stats.conflicts
    ));
    s.push_str(&format!("input_bytes,{input_len}\n"));
    s.push_str(&format!("output_bytes,{output_len}\n"));
    if !stats.removed.is_empty() {
        s.push_str("\nprefix,namespace_uri\n");
        for (prefix, uri) in &stats.removed {
            let label = if prefix.is_empty() {
                "(default)".to_string()
            } else {
                lossy(prefix)
            };
            s.push_str(&format!("{},{}\n", csv_field(&label), csv_field(&lossy(uri))));
        }
    }
    s
}

fn csv_field(v: &str) -> String {
    if v.contains(',') || v.contains('"') || v.contains('\n') {
        format!("\"{}\"", v.replace('"', "\"\""))
    } else {
        v.to_string()
    }
}

fn attr_err(element: &[u8], err: &str) -> String {
    format!("cannot read the attributes of <{}>: {err}", lossy(element))
}

fn write_err(e: quick_xml::Error) -> String {
    format!("failed to write XML: {e}")
}

fn lossy(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `strip` with every default: mode=all, no kept prefixes, rename on clash,
    /// schema hints removed, whitespace preserved, XML out.
    fn all(xml: &str) -> Result<String, String> {
        strip(xml, "all", "", "rename", true, "preserve", 2, "xml")
    }

    #[test]
    fn strips_soap_envelope() {
        let xml = concat!(
            r#"<soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/">"#,
            r#"<soap:Body><m:GetPrice xmlns:m="https://example.com/prices">"#,
            r#"<m:Item>Apples</m:Item></m:GetPrice></soap:Body></soap:Envelope>"#
        );
        assert_eq!(
            all(xml).unwrap(),
            "<Envelope><Body><GetPrice><Item>Apples</Item></GetPrice></Body></Envelope>"
        );
    }

    #[test]
    fn strips_default_namespace() {
        let xml = r#"<feed xmlns="http://www.w3.org/2005/Atom"><title>Posts</title></feed>"#;
        assert_eq!(all(xml).unwrap(), "<feed><title>Posts</title></feed>");
    }

    #[test]
    fn strips_attribute_prefixes() {
        let xml = concat!(
            r#"<r xmlns:x="https://example.com/x" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">"#,
            r#"<c x:id="7" xsi:type="Decimal">4</c></r>"#
        );
        assert_eq!(
            all(xml).unwrap(),
            r#"<r><c id="7" type="Decimal">4</c></r>"#
        );
    }

    #[test]
    fn keeps_unprefixed_attributes_and_values_verbatim() {
        let xml = r#"<n:r xmlns:n="https://example.com/n" note="a &amp; b" q='say "hi"'/>"#;
        assert_eq!(all(xml).unwrap(), r#"<r note="a &amp; b" q="say &quot;hi&quot;"/>"#);
    }

    #[test]
    fn preserves_prolog_doctype_comment_pi_and_cdata() {
        let xml = concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
            "<!DOCTYPE n:r SYSTEM \"r.dtd\">\n",
            "<n:r xmlns:n=\"https://example.com/n\">\n",
            "  <!-- keep me -->\n",
            "  <?render mode=\"fast\"?>\n",
            "  <n:c><![CDATA[a < b && c]]></n:c>\n",
            "</n:r>"
        );
        let out = all(xml).unwrap();
        assert!(out.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n"));
        assert!(out.contains("<!DOCTYPE n:r SYSTEM \"r.dtd\">"));
        assert!(out.contains("<!-- keep me -->"));
        assert!(out.contains("<?render mode=\"fast\"?>"));
        assert!(out.contains("<c><![CDATA[a < b && c]]></c>"));
        // preserve keeps the document's own newlines and indentation.
        assert!(out.contains("<r>\n  <!-- keep me -->"));
    }

    #[test]
    fn untouched_document_is_byte_identical() {
        let xml = "<?xml version='1.0'?>\n<r a='1'  b = \"2\" >\n  <c/>\n</r>\n";
        assert_eq!(all(xml).unwrap(), xml);
    }

    #[test]
    fn reserved_xml_prefix_is_preserved() {
        let xml = r#"<n:r xmlns:n="https://example.com/n" xml:lang="en"><n:c xml:space="preserve">x</n:c></n:r>"#;
        assert_eq!(
            all(xml).unwrap(),
            r#"<r xml:lang="en"><c xml:space="preserve">x</c></r>"#
        );
    }

    #[test]
    fn keeps_listed_prefix_with_its_declaration() {
        let xml = concat!(
            r#"<soap:Envelope xmlns:soap="https://example.com/soap" xmlns:m="https://example.com/m">"#,
            r#"<soap:Body><m:Item m:id="1"/></soap:Body></soap:Envelope>"#
        );
        let out = strip(xml, "all", "soap", "rename", true, "preserve", 2, "xml").unwrap();
        assert_eq!(
            out,
            concat!(
                r#"<soap:Envelope xmlns:soap="https://example.com/soap">"#,
                r#"<soap:Body><Item id="1"/></soap:Body></soap:Envelope>"#
            )
        );
    }

    #[test]
    fn xmlns_token_keeps_the_default_declaration() {
        let xml = r#"<feed xmlns="https://example.com/atom" xmlns:d="https://example.com/d"><d:x/></feed>"#;
        let out = strip(xml, "all", "xmlns", "rename", true, "preserve", 2, "xml").unwrap();
        assert_eq!(out, r#"<feed xmlns="https://example.com/atom"><x/></feed>"#);
    }

    #[test]
    fn mode_prefixes_keeps_declarations() {
        let xml = r#"<n:r xmlns:n="https://example.com/n"><n:c n:id="1"/></n:r>"#;
        let out = strip(xml, "prefixes", "", "rename", true, "preserve", 2, "xml").unwrap();
        assert_eq!(
            out,
            r#"<r xmlns:n="https://example.com/n"><c id="1"/></r>"#
        );
    }

    #[test]
    fn mode_declarations_keeps_prefixed_names() {
        let xml = r#"<n:r xmlns:n="https://example.com/n"><n:c n:id="1"/></n:r>"#;
        let out = strip(xml, "declarations", "", "rename", true, "preserve", 2, "xml").unwrap();
        assert_eq!(out, r#"<n:r><n:c n:id="1"/></n:r>"#);
    }

    #[test]
    fn removes_schema_hints_by_resolved_namespace_only() {
        let xsi = "http://www.w3.org/2001/XMLSchema-instance";
        // Prefix `q` is bound to the XSI namespace → its schemaLocation goes.
        let xml = format!(
            r#"<r xmlns:q="{xsi}" q:schemaLocation="https://example.com/s s.xsd" q:noNamespaceSchemaLocation="s.xsd"/>"#
        );
        assert_eq!(all(&xml).unwrap(), "<r/>");

        // Prefix spelled `xsi` but bound elsewhere → the attribute is data, kept.
        let decoy = r#"<r xmlns:xsi="https://example.com/not-schema" xsi:schemaLocation="keep me"/>"#;
        assert_eq!(all(decoy).unwrap(), r#"<r schemaLocation="keep me"/>"#);

        // Opt out and the hint survives, prefix stripped like any other.
        let kept = format!(r#"<r xmlns:q="{xsi}" q:schemaLocation="s.xsd"/>"#);
        let out = strip(&kept, "all", "", "rename", false, "preserve", 2, "xml").unwrap();
        assert_eq!(out, r#"<r schemaLocation="s.xsd"/>"#);
    }

    #[test]
    fn conflict_rename_keeps_both_attributes() {
        let xml = concat!(
            r#"<r xmlns:a="https://example.com/a" xmlns:b="https://example.com/b">"#,
            r#"<c a:id="1" b:id="2"/></r>"#
        );
        assert_eq!(all(xml).unwrap(), r#"<r><c id="1" b_id="2"/></r>"#);
    }

    #[test]
    fn conflict_rename_handles_plain_attribute_last() {
        let xml = r#"<r xmlns:a="https://example.com/a"><c a:id="1" id="2"/></r>"#;
        assert_eq!(all(xml).unwrap(), r#"<r><c id="1" id_2="2"/></r>"#);
    }

    #[test]
    fn conflict_first_drops_the_later_attribute() {
        let xml = concat!(
            r#"<r xmlns:a="https://example.com/a" xmlns:b="https://example.com/b">"#,
            r#"<c a:id="1" b:id="2"/></r>"#
        );
        let out = strip(xml, "all", "", "first", true, "preserve", 2, "xml").unwrap();
        assert_eq!(out, r#"<r><c id="1"/></r>"#);
    }

    #[test]
    fn conflict_error_names_the_clashing_pair() {
        let xml = concat!(
            r#"<r xmlns:a="https://example.com/a" xmlns:b="https://example.com/b">"#,
            r#"<c a:id="1" b:id="2"/></r>"#
        );
        let err = strip(xml, "all", "", "error", true, "preserve", 2, "xml").unwrap_err();
        assert!(err.contains("attribute name clash on <c>"), "{err}");
        assert!(err.contains("'b:id'"), "{err}");
        assert!(err.contains("'b_id'"), "{err}");
    }

    #[test]
    fn pretty_reindents_and_minify_collapses() {
        let xml = "<n:r xmlns:n=\"https://example.com/n\">\n\n   <n:c>hi</n:c>\n</n:r>";
        let pretty = strip(xml, "all", "", "rename", true, "pretty", 4, "xml").unwrap();
        assert_eq!(pretty, "<r>\n    <c>hi</c>\n</r>");
        let mini = strip(xml, "all", "", "rename", true, "minify", 2, "xml").unwrap();
        assert_eq!(mini, "<r><c>hi</c></r>");
    }

    #[test]
    fn report_lists_every_removed_declaration() {
        let xml = concat!(
            r#"<soap:Envelope xmlns:soap="https://example.com/soap" xmlns="https://example.com/d">"#,
            r#"<soap:Body><item soap:id="1"/></soap:Body></soap:Envelope>"#
        );
        let out = strip(xml, "all", "", "rename", true, "preserve", 2, "report").unwrap();
        assert!(out.starts_with("metric,value\n"), "{out}");
        assert!(out.contains("mode,all\n"), "{out}");
        assert!(out.contains("declarations_removed,2\n"), "{out}");
        // soap:Envelope + soap:Body; <item> was never prefixed.
        assert!(out.contains("element_prefixes_stripped,2\n"), "{out}");
        assert!(out.contains("attribute_prefixes_stripped,1\n"), "{out}");
        assert!(out.contains("attribute_clashes_resolved,0\n"), "{out}");
        assert!(out.contains("\nprefix,namespace_uri\n"), "{out}");
        assert!(out.contains("soap,https://example.com/soap\n"), "{out}");
        assert!(out.contains("(default),https://example.com/d\n"), "{out}");
    }

    #[test]
    fn report_counts_schema_references_and_clashes() {
        let xsi = "http://www.w3.org/2001/XMLSchema-instance";
        let xml = format!(
            r#"<r xmlns:q="{xsi}" xmlns:a="https://example.com/a" q:schemaLocation="s.xsd" a:id="1" id="2"/>"#
        );
        let out = strip(&xml, "all", "", "rename", true, "preserve", 2, "report").unwrap();
        assert!(out.contains("schema_references_removed,1\n"), "{out}");
        assert!(out.contains("attribute_clashes_resolved,1\n"), "{out}");
    }

    #[test]
    fn rejects_empty_input() {
        let err = all("   \n ").unwrap_err();
        assert!(err.contains("no XML input"), "{err}");
    }

    #[test]
    fn rejects_malformed_xml_with_a_position() {
        let err = all("<a><b></a>").unwrap_err();
        assert!(err.contains("not well-formed at byte"), "{err}");
        assert!(all("not xml at all <").is_err());
    }

    #[test]
    fn rejects_oversized_input() {
        let big = format!("<r>{}</r>", "x".repeat(MAX_INPUT));
        let err = all(&big).unwrap_err();
        assert!(err.contains(&format!("the limit is {MAX_INPUT} bytes")), "{err}");
    }

    #[test]
    fn rejects_unknown_option_values() {
        let xml = "<r/>";
        assert!(strip(xml, "nope", "", "rename", true, "preserve", 2, "xml")
            .unwrap_err()
            .contains("unknown mode 'nope'"));
        assert!(strip(xml, "all", "", "nope", true, "preserve", 2, "xml")
            .unwrap_err()
            .contains("unknown conflicts 'nope'"));
        assert!(strip(xml, "all", "", "rename", true, "nope", 2, "xml")
            .unwrap_err()
            .contains("unknown format 'nope'"));
        assert!(strip(xml, "all", "", "rename", true, "preserve", 2, "nope")
            .unwrap_err()
            .contains("unknown output 'nope'"));
    }

    #[test]
    fn indent_is_clamped_not_rejected() {
        let out = strip("<n:r xmlns:n='https://example.com/n'><n:c/></n:r>", "all", "", "rename", true, "pretty", 99, "xml")
            .unwrap();
        assert_eq!(out, format!("<r>\n{}<c/>\n</r>", " ".repeat(16)));
    }

    #[test]
    fn nested_scopes_resolve_the_innermost_binding() {
        let xsi = "http://www.w3.org/2001/XMLSchema-instance";
        // `q` is XSI at the root but rebound to a data namespace on <inner>,
        // so only the outer schemaLocation is a dangling schema reference.
        let xml = format!(
            concat!(
                r#"<r xmlns:q="{xsi}" q:schemaLocation="s.xsd">"#,
                r#"<inner xmlns:q="https://example.com/q" q:schemaLocation="data"/></r>"#
            ),
            xsi = xsi
        );
        assert_eq!(
            all(&xml).unwrap(),
            r#"<r><inner schemaLocation="data"/></r>"#
        );
    }
}
