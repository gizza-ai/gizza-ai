//! gizza-ai/pdf-structure-inspector core — take the raw bytes of a PDF and
//! describe how the file is put together: the header version, the trailer
//! dictionary, every indirect object with its type and dictionary keys, and for
//! stream objects the `/Filter` chain and byte lengths, plus the encrypted and
//! linearized flags.
//!
//! This is a *structural* reader, the PDF equivalent of `hexdump -C` with the
//! syntax already parsed. It never decodes stream payloads, never extracts page
//! text or images, and never executes anything the document contains.
//!
//! The PDF arrives as text (a page form field or a chat argument), so the bytes
//! may be given as base64, as hex, or as a `data:application/pdf;base64,…` URL.
//!
//! No wafer/wasm-bindgen deps: compiles natively for unit tests, to
//! `wasm32-wasip1` (`wafer build`) and to `wasm32-unknown-unknown` (the page).

use base64::Engine;
use lopdf::{Dictionary, Document, Object, StringFormat};
use serde::Serialize;
use std::collections::BTreeMap;

/// Largest PDF we will decode, in bytes. Both surfaces hand us the file as
/// text, so anything bigger is a copy/paste that will not survive the trip.
pub const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
/// Default number of objects listed.
pub const DEFAULT_MAX_OBJECTS: u32 = 100;
/// Smallest accepted `max_objects`.
pub const MIN_MAX_OBJECTS: u32 = 1;
/// Largest accepted `max_objects`.
pub const MAX_MAX_OBJECTS: u32 = 5000;
/// Report sections, in the order the full report prints them.
pub const SECTIONS: [&str; 5] = ["all", "summary", "trailer", "objects", "streams"];
/// Output formats.
pub const FORMATS: [&str; 2] = ["text", "json"];

/// Longest dictionary-key list kept per object (a page's `/Resources` can carry
/// hundreds of font names; the report would stop being readable).
const MAX_KEYS_PER_OBJECT: usize = 24;
/// Longest rendered scalar value in the trailer / object summaries.
const MAX_VALUE_CHARS: usize = 120;

/// Everything the two surfaces can vary. Defaults produce the full text report
/// of the first 100 objects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    /// Which part of the report to print: `all`, `summary`, `trailer`,
    /// `objects`, or `streams`.
    pub section: String,
    /// Empty, `"12"` (any generation) or `"12 0"` (exact object id).
    pub object_id: String,
    /// Empty, or a dictionary key / `/Type` value to keep, e.g. `Font`.
    pub filter_key: String,
    /// Cap on the number of listed objects.
    pub max_objects: u32,
    /// `text` or `json`.
    pub format: String,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            section: "all".into(),
            object_id: String::new(),
            filter_key: String::new(),
            max_objects: DEFAULT_MAX_OBJECTS,
            format: "text".into(),
        }
    }
}

/// File-level facts about the document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DocumentInfo {
    /// Header version, e.g. `1.7`.
    pub pdf_version: String,
    /// Size of the decoded file.
    pub input_bytes: usize,
    /// How the input text was decoded: `base64`, `hex`, or `data-url`.
    pub input_encoding: String,
    /// Pages reachable from the page tree.
    pub page_count: usize,
    /// Indirect objects the parser recovered (including objects unpacked from
    /// `/ObjStm` object streams).
    pub object_count: usize,
    /// How many of those are stream objects.
    pub stream_count: usize,
    /// How many are `/ObjStm` object streams (compressed object containers).
    pub object_stream_count: usize,
    /// Highest object number in the file.
    pub highest_object_id: u32,
    /// `cross-reference table` (classic) or `cross-reference stream` (PDF 1.5+).
    pub xref_style: String,
    /// `/Size` of the cross-reference section: highest object number plus one.
    pub xref_size: u32,
    /// True when the trailer carries `/Encrypt`.
    pub encrypted: bool,
    /// True when the file carries a `/Linearized` parameter dictionary — it is
    /// laid out for byte-range ("fast web view") delivery.
    pub linearized: bool,
}

/// One `name → count` tally row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Tally {
    pub name: String,
    pub count: usize,
}

/// Object-population tallies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Counts {
    /// By PDF object kind: dictionary, stream, array, name, …
    pub by_kind: Vec<Tally>,
    /// By `/Type` value, for the objects that declare one.
    pub by_type: Vec<Tally>,
    /// By stream `/Filter` name.
    pub by_filter: Vec<Tally>,
}

/// One trailer entry, rendered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TrailerEntry {
    /// Key with its slash, e.g. `/Root`.
    pub key: String,
    /// Short rendering of the value, e.g. `3 0 R`.
    pub value: String,
}

/// One indirect object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ObjectInfo {
    /// `"12 0"` — object number and generation.
    pub id: String,
    pub number: u32,
    pub generation: u16,
    /// PDF object kind: `dictionary`, `stream`, `array`, `name`, `string`,
    /// `integer`, `real`, `boolean`, `null`, or `reference`.
    pub kind: String,
    /// `/Type` value, when the object declares one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_name: Option<String>,
    /// `/Subtype` value, when the object declares one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtype: Option<String>,
    /// Dictionary keys, with their slashes, capped at 24.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub keys: Vec<String>,
    /// True when `keys` was capped.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub keys_truncated: bool,
    /// `/Filter` chain of a stream object, outermost first.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub filters: Vec<String>,
    /// `/Length` as declared in the stream dictionary.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub declared_length: Option<i64>,
    /// Bytes actually carried by the stream, still encoded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_bytes: Option<usize>,
    /// Short rendering of a non-dictionary object's value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

impl ObjectInfo {
    /// True for stream objects — what `section=streams` keeps.
    pub fn is_stream(&self) -> bool {
        self.kind == "stream"
    }
}

/// The full parse of one PDF, before `section` / `max_objects` narrow it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Inspection {
    pub document: DocumentInfo,
    pub counts: Counts,
    pub trailer: Vec<TrailerEntry>,
    /// Every object, ordered by object number then generation.
    pub objects: Vec<ObjectInfo>,
}

/// Which objects `object_id` keeps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IdFilter {
    /// Every generation of this object number.
    Number(u32),
    /// One exact `number generation` pair.
    Exact(u32, u16),
}

impl IdFilter {
    fn keeps(&self, o: &ObjectInfo) -> bool {
        match *self {
            IdFilter::Number(n) => o.number == n,
            IdFilter::Exact(n, g) => o.number == n && o.generation == g,
        }
    }
}

/// Inspect a PDF supplied as base64, hex, or a base64 `data:` URL.
///
/// `Err` carries a message meant for the user: an undecodable input, a file
/// that is not a PDF, or an out-of-range option.
pub fn run(input: &str, opts: &Options) -> Result<String, String> {
    let section = normalize_choice(&opts.section, &SECTIONS, "section", "all")?;
    let format = normalize_choice(&opts.format, &FORMATS, "format", "text")?;
    if !(MIN_MAX_OBJECTS..=MAX_MAX_OBJECTS).contains(&opts.max_objects) {
        return Err(format!(
            "max_objects must be between {MIN_MAX_OBJECTS} and {MAX_MAX_OBJECTS}, got {}",
            opts.max_objects
        ));
    }
    let id_filter = parse_object_id(&opts.object_id)?;
    let key_filter = normalize_key(&opts.filter_key);

    let (bytes, encoding) = decode_pdf_input(input)?;
    let inspection = inspect(&bytes, encoding)?;

    // Narrow the object list: section first (streams only), then the two
    // explicit filters. `matched` counts what survived before max_objects.
    let mut objects: Vec<&ObjectInfo> = inspection
        .objects
        .iter()
        .filter(|o| section != "streams" || o.is_stream())
        .filter(|o| id_filter.map(|f| f.keeps(o)).unwrap_or(true))
        .filter(|o| {
            key_filter
                .as_deref()
                .map(|k| matches_key(o, k))
                .unwrap_or(true)
        })
        .collect();
    let matched = objects.len();
    objects.truncate(opts.max_objects as usize);

    let view = View {
        section,
        matched,
        objects,
        id_filter: &opts.object_id,
        key_filter: &opts.filter_key,
        max_objects: opts.max_objects,
    };
    Ok(match format {
        "json" => render_json(&inspection, &view),
        _ => render_text(&inspection, &view),
    })
}

/// The narrowed object list plus the filters that produced it — what the two
/// renderers read.
struct View<'a> {
    section: &'static str,
    matched: usize,
    objects: Vec<&'a ObjectInfo>,
    id_filter: &'a str,
    key_filter: &'a str,
    max_objects: u32,
}

impl View<'_> {
    fn truncated(&self) -> bool {
        self.matched > self.objects.len()
    }
    fn filtered(&self) -> bool {
        !self.id_filter.trim().is_empty() || !self.key_filter.trim().is_empty()
    }
    fn wants(&self, part: &str) -> bool {
        match part {
            "summary" => matches!(self.section, "all" | "summary"),
            "trailer" => matches!(self.section, "all" | "trailer"),
            _ => matches!(self.section, "all" | "objects" | "streams"),
        }
    }
}

// ---------------------------------------------------------------- decoding --

/// Decode the pasted text into PDF bytes, returning the encoding that worked
/// (`base64`, `hex` or `data-url`).
pub fn decode_pdf_input(input: &str) -> Result<(Vec<u8>, &'static str), String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(
            "no PDF supplied — paste the file as base64, as hex, or as a \
             data:application/pdf;base64,… URL"
                .into(),
        );
    }

    let (payload, encoding) = match trimmed.strip_prefix("data:") {
        Some(rest) => {
            let comma = rest.find(',').ok_or_else(|| {
                "data: URL is missing the ',' that separates the header from the payload"
                    .to_string()
            })?;
            if !rest[..comma].to_ascii_lowercase().contains("base64") {
                return Err(
                    "only base64 data: URLs are supported — re-export the PDF as \
                     data:application/pdf;base64,…"
                        .into(),
                );
            }
            (&rest[comma + 1..], "data-url")
        }
        None => (trimmed, ""),
    };

    let compact: String = payload.chars().filter(|c| !c.is_whitespace()).collect();
    if compact.is_empty() {
        return Err("the input decoded to no bytes — the payload is empty".into());
    }

    // A base64 PDF always starts "JVBERi0" (that is "%PDF-"), which is not
    // hex, so "all hex digits" only ever picks up genuine hex dumps.
    let bytes = if encoding.is_empty() && looks_like_hex(&compact) {
        decode_hex(&compact)?
    } else {
        decode_base64(&compact)?
    };
    let encoding = if !encoding.is_empty() {
        encoding
    } else if looks_like_hex(&compact) {
        "hex"
    } else {
        "base64"
    };

    if bytes.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "PDF is too large: {} bytes (limit {MAX_INPUT_BYTES})",
            bytes.len()
        ));
    }
    if !bytes.windows(4).any(|w| w == b"%PDF") {
        return Err(
            "the decoded bytes have no %PDF header — this does not look like a PDF \
             (check that the whole file was copied)"
                .into(),
        );
    }
    Ok((bytes, encoding))
}

fn looks_like_hex(s: &str) -> bool {
    s.len() >= 8 && s.len() % 2 == 0 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

fn decode_hex(s: &str) -> Result<Vec<u8>, String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len() / 2);
    for pair in b.chunks(2) {
        let hi = (pair[0] as char).to_digit(16).unwrap() as u8;
        let lo = (pair[1] as char).to_digit(16).unwrap() as u8;
        out.push(hi << 4 | lo);
    }
    Ok(out)
}

fn decode_base64(s: &str) -> Result<Vec<u8>, String> {
    // Accept the padded and unpadded forms of both alphabets: a PDF pasted out
    // of a URL or a JSON blob can arrive in any of the four.
    let unpadded = s.trim_end_matches('=');
    base64::engine::general_purpose::STANDARD_NO_PAD
        .decode(unpadded)
        .or_else(|_| base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(unpadded))
        .map_err(|e| format!("input is neither valid base64 nor a plain hex dump of the file: {e}"))
}

// ---------------------------------------------------------------- parsing ---

/// Parse the PDF and describe its structure. `encoding` is carried through to
/// the report so the output says how the bytes were read.
pub fn inspect(bytes: &[u8], encoding: &str) -> Result<Inspection, String> {
    let doc = Document::load_mem(bytes).map_err(|e| format!("failed to parse PDF: {e}"))?;

    let mut by_kind: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut by_type: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_filter: BTreeMap<String, usize> = BTreeMap::new();
    let mut objects: Vec<ObjectInfo> = Vec::with_capacity(doc.objects.len());
    let mut stream_count = 0usize;
    let mut object_stream_count = 0usize;
    let mut linearized = false;

    for (id, obj) in &doc.objects {
        let kind = object_kind(obj);
        *by_kind.entry(kind).or_insert(0) += 1;

        let dict = match obj {
            Object::Dictionary(d) => Some(d),
            Object::Stream(s) => Some(&s.dict),
            _ => None,
        };
        let mut info = ObjectInfo {
            id: format!("{} {}", id.0, id.1),
            number: id.0,
            generation: id.1,
            kind: kind.to_string(),
            type_name: None,
            subtype: None,
            keys: Vec::new(),
            keys_truncated: false,
            filters: Vec::new(),
            declared_length: None,
            stream_bytes: None,
            value: None,
        };

        if let Some(d) = dict {
            if d.has(b"Linearized") {
                linearized = true;
            }
            info.type_name = name_value(d, b"Type");
            info.subtype = name_value(d, b"Subtype");
            let all_keys: Vec<String> = d
                .iter()
                .map(|(k, _)| format!("/{}", String::from_utf8_lossy(k)))
                .collect();
            info.keys_truncated = all_keys.len() > MAX_KEYS_PER_OBJECT;
            info.keys = all_keys.into_iter().take(MAX_KEYS_PER_OBJECT).collect();
            if let Some(t) = &info.type_name {
                *by_type.entry(t.clone()).or_insert(0) += 1;
            }
        }

        if let Object::Stream(s) = obj {
            stream_count += 1;
            if info.type_name.as_deref() == Some("/ObjStm") {
                object_stream_count += 1;
            }
            info.filters = filter_names(&s.dict);
            for f in &info.filters {
                *by_filter.entry(f.clone()).or_insert(0) += 1;
            }
            info.declared_length = match s.dict.get(b"Length") {
                Ok(Object::Integer(n)) => Some(*n),
                // /Length is allowed to be an indirect reference.
                Ok(Object::Reference(r)) => match doc.get_object(*r) {
                    Ok(Object::Integer(n)) => Some(*n),
                    _ => None,
                },
                _ => None,
            };
            info.stream_bytes = Some(s.content.len());
        } else if dict.is_none() {
            info.value = Some(render_value(obj));
        }

        objects.push(info);
    }

    let trailer = doc
        .trailer
        .iter()
        .map(|(k, v)| TrailerEntry {
            key: format!("/{}", String::from_utf8_lossy(k)),
            value: render_value(v),
        })
        .collect();

    let document = DocumentInfo {
        pdf_version: doc.version.clone(),
        input_bytes: bytes.len(),
        input_encoding: encoding.to_string(),
        page_count: doc.get_pages().len(),
        object_count: doc.objects.len(),
        stream_count,
        object_stream_count,
        highest_object_id: doc.max_id,
        xref_style: match doc.reference_table.cross_reference_type {
            lopdf::xref::XrefType::CrossReferenceStream => "cross-reference stream".into(),
            lopdf::xref::XrefType::CrossReferenceTable => "cross-reference table".into(),
        },
        xref_size: doc.reference_table.size,
        encrypted: doc.is_encrypted(),
        linearized,
    };

    Ok(Inspection {
        document,
        counts: Counts {
            by_kind: tallies(by_kind.into_iter().map(|(k, v)| (k.to_string(), v))),
            by_type: tallies(by_type.into_iter()),
            by_filter: tallies(by_filter.into_iter()),
        },
        trailer,
        objects,
    })
}

fn tallies(it: impl Iterator<Item = (String, usize)>) -> Vec<Tally> {
    let mut v: Vec<Tally> = it.map(|(name, count)| Tally { name, count }).collect();
    v.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.name.cmp(&b.name)));
    v
}

fn object_kind(o: &Object) -> &'static str {
    match o {
        Object::Null => "null",
        Object::Boolean(_) => "boolean",
        Object::Integer(_) => "integer",
        Object::Real(_) => "real",
        Object::Name(_) => "name",
        Object::String(..) => "string",
        Object::Array(_) => "array",
        Object::Dictionary(_) => "dictionary",
        Object::Stream(_) => "stream",
        Object::Reference(_) => "reference",
    }
}

fn name_value(d: &Dictionary, key: &[u8]) -> Option<String> {
    match d.get(key) {
        Ok(Object::Name(n)) => Some(format!("/{}", String::from_utf8_lossy(n))),
        _ => None,
    }
}

/// `/Filter` is a single name or an array of names, outermost first.
fn filter_names(d: &Dictionary) -> Vec<String> {
    match d.get(b"Filter") {
        Ok(Object::Name(n)) => vec![format!("/{}", String::from_utf8_lossy(n))],
        Ok(Object::Array(a)) => a
            .iter()
            .filter_map(|o| match o {
                Object::Name(n) => Some(format!("/{}", String::from_utf8_lossy(n))),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// One-line rendering of a PDF value, short enough for a report column.
fn render_value(o: &Object) -> String {
    let s = match o {
        Object::Null => "null".to_string(),
        Object::Boolean(b) => b.to_string(),
        Object::Integer(n) => n.to_string(),
        Object::Real(r) => r.to_string(),
        Object::Name(n) => format!("/{}", String::from_utf8_lossy(n)),
        Object::String(bytes, StringFormat::Hexadecimal) => {
            format!(
                "<{}>",
                bytes.iter().map(|b| format!("{b:02X}")).collect::<String>()
            )
        }
        Object::String(bytes, _) => format!("({})", String::from_utf8_lossy(bytes)),
        Object::Reference((n, g)) => format!("{n} {g} R"),
        Object::Array(a) => {
            let inner: Vec<String> = a.iter().take(8).map(render_value).collect();
            let more = if a.len() > 8 { ", …" } else { "" };
            format!("[{}{}]", inner.join(" "), more)
        }
        Object::Dictionary(d) => format!(
            "<< {} >>",
            d.iter()
                .take(8)
                .map(|(k, _)| format!("/{}", String::from_utf8_lossy(k)))
                .collect::<Vec<_>>()
                .join(" ")
        ),
        Object::Stream(s) => format!("stream ({} bytes)", s.content.len()),
    };
    truncate(&s, MAX_VALUE_CHARS)
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.replace(['\n', '\r'], " ");
    }
    let head: String = s.chars().take(max).collect();
    format!("{}…", head.replace(['\n', '\r'], " "))
}

// ---------------------------------------------------------------- filters ---

/// Accept `12`, `12 0`, `12,0` and `12 0 R`.
fn parse_object_id(raw: &str) -> Result<Option<IdFilter>, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(None);
    }
    let cleaned = raw.trim_end_matches(['R', 'r']).replace(',', " ");
    let parts: Vec<&str> = cleaned.split_whitespace().collect();
    let bad = || {
        format!(
            "object_id must be an object number like '12' or a number and generation \
             like '12 0', got '{raw}'"
        )
    };
    match parts.as_slice() {
        [n] => Ok(Some(IdFilter::Number(n.parse().map_err(|_| bad())?))),
        [n, g] => Ok(Some(IdFilter::Exact(
            n.parse().map_err(|_| bad())?,
            g.parse().map_err(|_| bad())?,
        ))),
        _ => Err(bad()),
    }
}

/// Lower-cased, slash-stripped needle; `None` when no filter was asked for.
fn normalize_key(raw: &str) -> Option<String> {
    let k = raw.trim().trim_start_matches('/').trim();
    if k.is_empty() {
        None
    } else {
        Some(k.to_ascii_lowercase())
    }
}

/// An object matches when it carries that dictionary key or declares it as its
/// `/Type`. Both comparisons ignore case and a leading slash.
fn matches_key(o: &ObjectInfo, needle: &str) -> bool {
    let eq = |s: &str| s.trim_start_matches('/').eq_ignore_ascii_case(needle);
    o.keys.iter().any(|k| eq(k)) || o.type_name.as_deref().is_some_and(eq)
}

/// Case-insensitive match of a fixed-choice option, with an empty value falling
/// back to `default`.
fn normalize_choice(
    raw: &str,
    allowed: &[&'static str],
    label: &str,
    default: &'static str,
) -> Result<&'static str, String> {
    let v = raw.trim().to_ascii_lowercase();
    if v.is_empty() {
        return Ok(default);
    }
    allowed
        .iter()
        .copied()
        .find(|a| *a == v)
        .ok_or_else(|| format!("{label} must be one of {}, got '{raw}'", allowed.join(", ")))
}

// -------------------------------------------------------------- rendering ---

fn render_text(insp: &Inspection, view: &View) -> String {
    let d = &insp.document;
    let mut out = String::new();
    out.push_str(&format!(
        "PDF {} · {} object{} · {} page{} · {} bytes ({} input)\n",
        d.pdf_version,
        d.object_count,
        plural(d.object_count),
        d.page_count,
        plural(d.page_count),
        d.input_bytes,
        d.input_encoding,
    ));

    if view.wants("summary") {
        out.push_str("\nDocument\n");
        kv(&mut out, "Version", &d.pdf_version);
        kv(&mut out, "Pages", &d.page_count.to_string());
        kv(&mut out, "Objects", &d.object_count.to_string());
        kv(&mut out, "Streams", &d.stream_count.to_string());
        kv(
            &mut out,
            "Object streams",
            &d.object_stream_count.to_string(),
        );
        kv(
            &mut out,
            "Highest object id",
            &d.highest_object_id.to_string(),
        );
        kv(
            &mut out,
            "Cross-reference",
            &format!("{} (/Size {})", d.xref_style, d.xref_size),
        );
        kv(&mut out, "Encrypted", yes_no(d.encrypted));
        kv(&mut out, "Linearized", yes_no(d.linearized));

        push_tallies(&mut out, "Object kinds", &insp.counts.by_kind);
        push_tallies(&mut out, "Object types", &insp.counts.by_type);
        push_tallies(&mut out, "Stream filters", &insp.counts.by_filter);
    }

    if view.wants("trailer") {
        out.push_str("\nTrailer\n");
        if insp.trailer.is_empty() {
            out.push_str("  (the trailer dictionary is empty)\n");
        }
        let width = insp.trailer.iter().map(|e| e.key.len()).max().unwrap_or(0);
        for e in &insp.trailer {
            out.push_str(&format!(
                "  {:<width$}  {}\n",
                e.key,
                e.value,
                width = width
            ));
        }
    }

    if view.wants("objects") {
        let label = if view.section == "streams" {
            "Stream objects"
        } else {
            "Objects"
        };
        out.push_str(&format!(
            "\n{label} ({} shown of {} matched)\n",
            view.objects.len(),
            view.matched
        ));
        if view.objects.is_empty() {
            out.push_str(&no_match_note(view));
        }
        for o in &view.objects {
            out.push_str(&format!("  {}  {}", o.id, o.kind));
            if let Some(t) = &o.type_name {
                out.push_str(&format!("  {t}"));
            }
            if let Some(s) = &o.subtype {
                out.push_str(&format!(" {s}"));
            }
            out.push('\n');
            if o.is_stream() {
                let filters = if o.filters.is_empty() {
                    "none (raw bytes)".to_string()
                } else {
                    o.filters.join(" → ")
                };
                out.push_str(&format!("      filters: {filters}\n"));
                let declared = match o.declared_length {
                    Some(n) => n.to_string(),
                    None => "unknown".to_string(),
                };
                out.push_str(&format!(
                    "      length: {} bytes stored, /Length {}\n",
                    o.stream_bytes.unwrap_or(0),
                    declared
                ));
            }
            if !o.keys.is_empty() {
                let more = if o.keys_truncated { " …" } else { "" };
                out.push_str(&format!("      keys: {}{}\n", o.keys.join(" "), more));
            }
            if let Some(v) = &o.value {
                out.push_str(&format!("      value: {v}\n"));
            }
        }
        if view.truncated() {
            out.push_str(&format!(
                "  … {} more object(s) not shown — raise max_objects (currently {}).\n",
                view.matched - view.objects.len(),
                view.max_objects
            ));
        }
    }

    out
}

/// Why an object list came back empty — the filters are the usual reason.
fn no_match_note(view: &View) -> String {
    let mut bits: Vec<String> = Vec::new();
    if !view.id_filter.trim().is_empty() {
        bits.push(format!("object_id '{}'", view.id_filter.trim()));
    }
    if !view.key_filter.trim().is_empty() {
        bits.push(format!("filter_key '{}'", view.key_filter.trim()));
    }
    if view.section == "streams" && bits.is_empty() {
        return "  (this PDF has no stream objects)\n".to_string();
    }
    if bits.is_empty() {
        return "  (this PDF has no indirect objects)\n".to_string();
    }
    format!("  (nothing matched {})\n", bits.join(" and "))
}

fn render_json(insp: &Inspection, view: &View) -> String {
    let mut root = serde_json::Map::new();
    root.insert("section".into(), view.section.into());
    root.insert(
        "document".into(),
        serde_json::to_value(&insp.document).unwrap_or(serde_json::Value::Null),
    );
    if view.wants("summary") {
        root.insert(
            "counts".into(),
            serde_json::to_value(&insp.counts).unwrap_or(serde_json::Value::Null),
        );
    }
    if view.wants("trailer") {
        root.insert(
            "trailer".into(),
            serde_json::to_value(&insp.trailer).unwrap_or(serde_json::Value::Null),
        );
    }
    if view.wants("objects") {
        let items = serde_json::to_value(&view.objects).unwrap_or(serde_json::Value::Null);
        root.insert(
            "objects".into(),
            serde_json::json!({
                "matched": view.matched,
                "shown": view.objects.len(),
                "truncated": view.truncated(),
                "filtered": view.filtered(),
                "items": items,
            }),
        );
    }
    serde_json::to_string_pretty(&serde_json::Value::Object(root))
        .unwrap_or_else(|e| format!("{{\"error\":\"serialize report: {e}\"}}"))
}

fn kv(out: &mut String, label: &str, value: &str) {
    out.push_str(&format!("  {label:<18}{value}\n"));
}

fn push_tallies(out: &mut String, heading: &str, rows: &[Tally]) {
    if rows.is_empty() {
        return;
    }
    out.push_str(&format!("\n{heading}\n"));
    let width = rows.iter().map(|r| r.name.len()).max().unwrap_or(0);
    for r in rows {
        out.push_str(&format!(
            "  {:<width$}  {}\n",
            r.name,
            r.count,
            width = width
        ));
    }
}

fn yes_no(b: bool) -> &'static str {
    if b {
        "yes"
    } else {
        "no"
    }
}

fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(s: &str) -> Object {
        Object::Name(s.as_bytes().to_vec())
    }

    fn dict(pairs: Vec<(&str, Object)>) -> Dictionary {
        let mut d = Dictionary::new();
        for (k, v) in pairs {
            d.set(k.as_bytes().to_vec(), v);
        }
        d
    }

    /// A one-page PDF with a content stream and an embedded font-ish stream —
    /// enough structure to exercise every section of the report.
    fn sample_pdf() -> Vec<u8> {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let content_id = doc.add_object(Object::Stream(lopdf::Stream::new(
            dict(vec![]),
            b"BT /F1 24 Tf 20 100 Td (Hi) Tj ET".to_vec(),
        )));
        let font_id = doc.add_object(Object::Dictionary(dict(vec![
            ("Type", name("Font")),
            ("Subtype", name("Type1")),
            ("BaseFont", name("Helvetica")),
        ])));
        let page_id = doc.add_object(Object::Dictionary(dict(vec![
            ("Type", name("Page")),
            ("Parent", Object::Reference(pages_id)),
            ("Contents", Object::Reference(content_id)),
            (
                "Resources",
                Object::Dictionary(dict(vec![(
                    "Font",
                    Object::Dictionary(dict(vec![("F1", Object::Reference(font_id))])),
                )])),
            ),
            (
                "MediaBox",
                Object::Array(vec![
                    Object::Integer(0),
                    Object::Integer(0),
                    Object::Integer(612),
                    Object::Integer(792),
                ]),
            ),
        ])));
        doc.objects.insert(
            pages_id,
            Object::Dictionary(dict(vec![
                ("Type", name("Pages")),
                ("Kids", Object::Array(vec![Object::Reference(page_id)])),
                ("Count", Object::Integer(1)),
            ])),
        );
        let catalog_id = doc.add_object(Object::Dictionary(dict(vec![
            ("Type", name("Catalog")),
            ("Pages", Object::Reference(pages_id)),
        ])));
        doc.trailer
            .set(b"Root".to_vec(), Object::Reference(catalog_id));
        let mut buf = Vec::new();
        doc.save_to(&mut buf).expect("write sample pdf");
        buf
    }

    pub(super) fn sample_b64() -> String {
        base64::engine::general_purpose::STANDARD.encode(sample_pdf())
    }

    #[test]
    fn text_report_covers_version_counts_trailer_and_streams() {
        let out = run(&sample_b64(), &Options::default()).expect("valid pdf");
        assert!(out.starts_with("PDF 1.5 · "), "header line: {out}");
        assert!(out.contains("Encrypted         no"), "{out}");
        assert!(out.contains("Linearized        no"), "{out}");
        assert!(out.contains("Pages             1"), "{out}");
        // Trailer keys are rendered with their values.
        assert!(out.contains("Trailer"), "{out}");
        assert!(out.contains("/Root"), "{out}");
        assert!(out.contains("/Size"), "{out}");
        // Object types tallied, and the content stream reported with its length.
        assert!(out.contains("/Catalog"), "{out}");
        assert!(out.contains("/Font"), "{out}");
        assert!(out.contains("filters: none (raw bytes)"), "{out}");
        assert!(out.contains("33 bytes stored"), "{out}");
    }

    #[test]
    fn hex_and_data_url_inputs_decode_to_the_same_report() {
        let bytes = sample_pdf();
        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
        let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        let data_url = format!("data:application/pdf;base64,{b64}");

        let opts = Options {
            section: "summary".into(),
            ..Options::default()
        };
        let from_b64 = run(&b64, &opts).unwrap();
        let from_hex = run(&hex, &opts).unwrap();
        let from_url = run(&data_url, &opts).unwrap();

        assert!(from_b64.contains("(base64 input)"));
        assert!(from_hex.contains("(hex input)"));
        assert!(from_url.contains("(data-url input)"));
        // Same document, so everything after the encoding label matches.
        let strip = |s: &str| {
            s.replace("(base64 input)", "")
                .replace("(hex input)", "")
                .replace("(data-url input)", "")
        };
        assert_eq!(strip(&from_b64), strip(&from_hex));
        assert_eq!(strip(&from_b64), strip(&from_url));
    }

    #[test]
    fn whitespace_and_unpadded_base64_are_accepted() {
        let b64 = sample_b64();
        let wrapped: String = b64
            .as_bytes()
            .chunks(64)
            .map(|c| format!("{}\n", String::from_utf8_lossy(c)))
            .collect();
        assert!(run(&wrapped, &Options::default()).is_ok());
        assert!(run(b64.trim_end_matches('='), &Options::default()).is_ok());
    }

    #[test]
    fn sections_select_what_the_report_prints() {
        let b64 = sample_b64();
        let sect = |s: &str| {
            run(
                &b64,
                &Options {
                    section: s.into(),
                    ..Options::default()
                },
            )
            .unwrap()
        };

        let summary = sect("summary");
        assert!(summary.contains("Document"));
        assert!(!summary.contains("\nTrailer\n"));
        assert!(!summary.contains("Objects ("));

        let trailer = sect("trailer");
        assert!(trailer.contains("\nTrailer\n"));
        assert!(!trailer.contains("Object kinds"));

        let objects = sect("objects");
        assert!(objects.contains("Objects ("));
        assert!(!objects.contains("\nTrailer\n"));

        let streams = sect("streams");
        assert!(streams.contains("Stream objects ("));
        // Only the content stream is a stream object.
        assert!(streams.contains("stream\n") || streams.contains("  stream"));
        assert!(!streams.contains("/Catalog"));
    }

    #[test]
    fn object_id_accepts_bare_number_and_number_generation() {
        let b64 = sample_b64();
        let by_number = run(
            &b64,
            &Options {
                section: "objects".into(),
                object_id: "1".into(),
                ..Options::default()
            },
        )
        .unwrap();
        assert!(by_number.contains("(1 shown of 1 matched)"), "{by_number}");
        assert!(by_number.contains("  1 0  "), "{by_number}");

        let exact = run(
            &b64,
            &Options {
                section: "objects".into(),
                object_id: "1 0".into(),
                ..Options::default()
            },
        )
        .unwrap();
        assert!(exact.contains("(1 shown of 1 matched)"), "{exact}");

        // A generation that does not exist matches nothing, but is not an error.
        let missing = run(
            &b64,
            &Options {
                section: "objects".into(),
                object_id: "1 7".into(),
                ..Options::default()
            },
        )
        .unwrap();
        assert!(missing.contains("(0 shown of 0 matched)"), "{missing}");
        assert!(
            missing.contains("nothing matched object_id '1 7'"),
            "{missing}"
        );
    }

    #[test]
    fn filter_key_matches_dictionary_keys_and_type_values() {
        let b64 = sample_b64();
        let opts = |k: &str| Options {
            section: "objects".into(),
            filter_key: k.into(),
            ..Options::default()
        };
        // /Type value, with and without the slash, in any case.
        for needle in ["Font", "/Font", "font"] {
            let out = run(&b64, &opts(needle)).unwrap();
            assert!(out.contains("/Font"), "{needle}: {out}");
            assert!(out.contains("(1 shown of 1 matched)"), "{needle}: {out}");
        }
        // Dictionary key: only the page carries /MediaBox.
        let page = run(&b64, &opts("MediaBox")).unwrap();
        assert!(page.contains("/Page"), "{page}");
        assert!(page.contains("(1 shown of 1 matched)"), "{page}");

        let none = run(&b64, &opts("NoSuchKey")).unwrap();
        assert!(
            none.contains("nothing matched filter_key 'NoSuchKey'"),
            "{none}"
        );
    }

    #[test]
    fn max_objects_caps_the_listing_and_says_so() {
        let out = run(
            &sample_b64(),
            &Options {
                section: "objects".into(),
                max_objects: 2,
                ..Options::default()
            },
        )
        .unwrap();
        assert!(out.contains("(2 shown of 6 matched)"), "{out}");
        assert!(out.contains("more object(s) not shown"), "{out}");
    }

    #[test]
    fn json_format_is_parseable_and_carries_the_structure() {
        let out = run(
            &sample_b64(),
            &Options {
                format: "json".into(),
                ..Options::default()
            },
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).expect("valid json");
        assert_eq!(v["section"], "all");
        assert_eq!(v["document"]["pdf_version"], "1.5");
        assert_eq!(v["document"]["page_count"], 1);
        assert_eq!(v["document"]["encrypted"], false);
        assert_eq!(v["document"]["linearized"], false);
        assert_eq!(v["document"]["stream_count"], 2);
        assert_eq!(v["objects"]["truncated"], false);
        assert!(v["trailer"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["key"] == "/Root"));
        let items = v["objects"]["items"].as_array().unwrap();
        assert_eq!(
            items.len(),
            v["objects"]["shown"].as_u64().unwrap() as usize
        );
        let stream = items
            .iter()
            .find(|o| o["kind"] == "stream")
            .expect("a stream object");
        assert_eq!(stream["stream_bytes"], 33);
        assert!(v["counts"]["by_type"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["name"] == "/Page"));
    }

    #[test]
    fn json_narrows_with_the_section() {
        let b64 = sample_b64();
        let json = |s: &str| -> serde_json::Value {
            let out = run(
                &b64,
                &Options {
                    section: s.into(),
                    format: "json".into(),
                    ..Options::default()
                },
            )
            .unwrap();
            serde_json::from_str(&out).unwrap()
        };
        let summary = json("summary");
        assert!(summary.get("counts").is_some());
        assert!(summary.get("trailer").is_none());
        assert!(summary.get("objects").is_none());

        let trailer = json("trailer");
        assert!(trailer.get("trailer").is_some());
        assert!(trailer.get("counts").is_none());

        let streams = json("streams");
        let items = streams["objects"]["items"].as_array().unwrap();
        assert!(!items.is_empty());
        assert!(items.iter().all(|o| o["kind"] == "stream"));
    }

    #[test]
    fn empty_input_is_rejected_with_the_accepted_encodings() {
        let err = run("   ", &Options::default()).unwrap_err();
        assert!(err.contains("base64"), "{err}");
        assert!(err.contains("hex"), "{err}");
    }

    #[test]
    fn non_pdf_and_undecodable_inputs_are_rejected() {
        let not_pdf = base64::engine::general_purpose::STANDARD.encode(b"hello world, not a pdf");
        let err = run(&not_pdf, &Options::default()).unwrap_err();
        assert!(err.contains("%PDF"), "{err}");

        let err = run("not base64 ***", &Options::default()).unwrap_err();
        assert!(
            err.contains("neither valid base64 nor a plain hex dump"),
            "{err}"
        );

        // Right header, truncated body: lopdf refuses it.
        let truncated = base64::engine::general_purpose::STANDARD.encode(b"%PDF-1.4\n% broken");
        let err = run(&truncated, &Options::default()).unwrap_err();
        assert!(err.starts_with("failed to parse PDF"), "{err}");
    }

    #[test]
    fn bad_options_name_the_offending_value() {
        let b64 = sample_b64();
        let err = run(
            &b64,
            &Options {
                section: "nope".into(),
                ..Options::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("section must be one of"), "{err}");

        let err = run(
            &b64,
            &Options {
                format: "yaml".into(),
                ..Options::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("format must be one of"), "{err}");

        let err = run(
            &b64,
            &Options {
                max_objects: 0,
                ..Options::default()
            },
        )
        .unwrap_err();
        assert!(
            err.contains("max_objects must be between 1 and 5000"),
            "{err}"
        );

        let err = run(
            &b64,
            &Options {
                max_objects: 5001,
                ..Options::default()
            },
        )
        .unwrap_err();
        assert!(
            err.contains("max_objects must be between 1 and 5000"),
            "{err}"
        );

        let err = run(
            &b64,
            &Options {
                object_id: "twelve".into(),
                ..Options::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("object_id must be an object number"), "{err}");
    }

    #[test]
    fn data_url_must_be_base64_and_well_formed() {
        let err = run(
            "data:application/pdf;charset=utf-8,%PDF-1.4",
            &Options::default(),
        )
        .unwrap_err();
        assert!(err.contains("only base64 data: URLs"), "{err}");

        let err = run("data:application/pdf;base64", &Options::default()).unwrap_err();
        assert!(err.contains("missing the ','"), "{err}");
    }

    #[test]
    fn object_id_syntax_variants_parse() {
        assert_eq!(parse_object_id("").unwrap(), None);
        assert_eq!(parse_object_id("  ").unwrap(), None);
        assert_eq!(parse_object_id("12").unwrap(), Some(IdFilter::Number(12)));
        assert_eq!(
            parse_object_id(" 12 0 ").unwrap(),
            Some(IdFilter::Exact(12, 0))
        );
        assert_eq!(
            parse_object_id("12,0").unwrap(),
            Some(IdFilter::Exact(12, 0))
        );
        assert_eq!(
            parse_object_id("12 0 R").unwrap(),
            Some(IdFilter::Exact(12, 0))
        );
        assert!(parse_object_id("1 2 3").is_err());
        assert!(parse_object_id("-1").is_err());
    }

    #[test]
    fn empty_and_default_options_are_interchangeable() {
        // The page sends blank strings for untouched selects; they must behave
        // exactly like the defaults rather than erroring.
        let b64 = sample_b64();
        let blank = Options {
            section: String::new(),
            format: String::new(),
            ..Options::default()
        };
        assert_eq!(
            run(&b64, &blank).unwrap(),
            run(&b64, &Options::default()).unwrap()
        );
    }
}
