//! file-metadata-inspect core — surface the metadata embedded in a file,
//! whatever its format. Pure Rust, no I/O, unit-testable on the host.
//!
//! Four extractors, picked from the container sniffed out of the magic bytes
//! (`detect-file-type-core`), so a mislabelled `.txt` that is really a JPEG is
//! still read correctly:
//!
//! * **EXIF/TIFF** (`kamadak-exif`) — JPEG/TIFF/PNG/WebP/HEIF/AVIF: camera,
//!   exposure, timestamps, software, GPS (decoded to decimal degrees).
//! * **XMP** — the `<x:xmpmeta>` packet, found by scanning the raw bytes, so it
//!   works for images, PDFs and anything else that embeds one uncompressed.
//! * **PDF** (`lopdf`) — the trailer's `/Info` dictionary (Title, Author,
//!   Creator, Producer, CreationDate, ModDate…), plus version, page count and
//!   encryption state.
//! * **ZIP containers** (`zip` + `quick-xml`) — OOXML `docProps/core.xml` +
//!   `docProps/app.xml`, OpenDocument `meta.xml`, and EPUB OPF `<metadata>`.
//!
//! Anything else — or a supported container that simply carries nothing — comes
//! back as a clean "no supported metadata found" report, never an error and
//! never a panic. A malformed or truncated file degrades the same way: the
//! extractor that failed is reported in `notes` and the rest still runs.

use std::io::{Cursor, Read};

use exif::{In, Tag, Value};
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use serde::Serialize;

/// Per-group field cap. A camera-raw EXIF block can carry thousands of tags;
/// past this many the report stops being readable (and blows the chat context).
const MAX_FIELDS_PER_GROUP: usize = 200;
/// Long values (embedded thumbnails, huge keyword lists) are elided.
const MAX_VALUE_CHARS: usize = 512;
/// How far into the file to look for an XMP packet.
const MAX_XMP_SCAN: usize = 8 * 1024 * 1024;
/// Largest XMP packet we will parse.
const MAX_XMP_BYTES: usize = 1024 * 1024;
/// Largest container member (core.xml / app.xml / OPF) we will decompress.
const MAX_XML_BYTES: u64 = 4 * 1024 * 1024;

/// One metadata entry — a name as the format spells it, and a display value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Field {
    /// Field name as the format names it, e.g. `Make`, `dc:title`, `Producer`.
    pub name: String,
    /// Human-readable value (with units where the format provides them).
    pub value: String,
}

/// Fields that came from one metadata block, e.g. `EXIF`, `XMP`, `PDF Info`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Group {
    pub name: String,
    pub fields: Vec<Field>,
}

/// Capture location decoded from EXIF GPS tags.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Gps {
    pub latitude: f64,
    pub longitude: f64,
}

/// The full inspection result.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    /// Human-readable format name of the detected container, e.g. `PNG image`.
    pub format: String,
    /// Detected media type, e.g. `application/pdf`.
    pub mime: String,
    /// Coarse bucket: image / document / archive / …
    pub category: String,
    /// Size of the inspected file in bytes.
    pub bytes: usize,
    /// Total number of fields across every group.
    pub field_count: usize,
    /// Metadata blocks found, in extraction order. Empty when there are none.
    pub groups: Vec<Group>,
    /// Decoded capture coordinates, when the file carries EXIF GPS tags.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gps: Option<Gps>,
    /// One-line plain-English verdict — always present, including for files
    /// with no metadata at all.
    pub summary: String,
    /// Privacy-relevant observations and partial-failure notes.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

/// Inspect a file's bytes and report every metadata block found.
///
/// Only an empty input is an error; every other input yields a `Report`.
pub fn inspect(bytes: &[u8]) -> Result<Report, String> {
    if bytes.is_empty() {
        return Err("input is empty — nothing to inspect".into());
    }
    let det = detect_container(bytes);
    let (format, mime, category, ext) = (det.kind, det.mime, det.category, det.ext);

    let mut groups: Vec<Group> = Vec::new();
    let mut notes: Vec<String> = Vec::new();
    let mut gps = None;

    match family_of(ext) {
        Family::Image => {
            let (g, coords, note) = read_exif(bytes);
            if let Some(g) = g {
                groups.push(g);
            }
            gps = coords;
            notes.extend(note);
        }
        Family::Pdf => {
            let (gs, note) = read_pdf(bytes);
            groups.extend(gs);
            notes.extend(note);
        }
        Family::Zip => {
            let (gs, note) = read_zip_container(bytes);
            groups.extend(gs);
            notes.extend(note);
        }
        Family::Other => {}
    }

    // An XMP packet can ride along in almost any uncompressed container (JPEG
    // APP1, PNG iTXt, a PDF metadata stream, plain XML sidecars), so scan for
    // one regardless of what the sniffed container was.
    if !groups.iter().any(|g| g.name == "XMP") {
        if let Some(g) = read_xmp(bytes) {
            groups.push(g);
        }
    }

    for g in &mut groups {
        if g.fields.len() > MAX_FIELDS_PER_GROUP {
            let dropped = g.fields.len() - MAX_FIELDS_PER_GROUP;
            g.fields.truncate(MAX_FIELDS_PER_GROUP);
            notes.push(format!(
                "{} has more fields than fit in one report — {dropped} field(s) after the first {MAX_FIELDS_PER_GROUP} were omitted",
                g.name
            ));
        }
    }
    groups.retain(|g| !g.fields.is_empty());

    let field_count: usize = groups.iter().map(|g| g.fields.len()).sum();
    if gps.is_some() {
        notes.push(
            "GPS coordinates are embedded — this file reveals where it was captured.".into(),
        );
    }
    let summary = summarize(format, field_count, &groups);

    Ok(Report {
        format: format.to_string(),
        mime: mime.to_string(),
        category: category.to_string(),
        bytes: bytes.len(),
        field_count,
        groups,
        gps,
        summary,
        notes,
    })
}

struct Detected {
    kind: &'static str,
    mime: &'static str,
    category: &'static str,
    ext: &'static str,
}

fn detect_container(bytes: &[u8]) -> Detected {
    if bytes.len() >= 3 && bytes[0] == 0xff && bytes[1] == 0xd8 && bytes[2] == 0xff {
        return Detected { kind: "JPEG image", mime: "image/jpeg", category: "image", ext: "jpg" };
    }
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Detected { kind: "PNG image", mime: "image/png", category: "image", ext: "png" };
    }
    if bytes.starts_with(b"II\x2a\x00") || bytes.starts_with(b"MM\x00\x2a") {
        return Detected { kind: "TIFF image", mime: "image/tiff", category: "image", ext: "tiff" };
    }
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return Detected { kind: "WebP image", mime: "image/webp", category: "image", ext: "webp" };
    }
    if bytes.len() >= 12 && &bytes[4..8] == b"ftyp" {
        let brand = &bytes[8..12];
        let (kind, mime, ext) = if matches!(brand, b"heic" | b"heix" | b"hevc" | b"hevx" | b"mif1" | b"msf1") {
            ("HEIF image", "image/heif", "heif")
        } else if matches!(brand, b"avif" | b"avis") {
            ("AVIF image", "image/avif", "avif")
        } else {
            ("ISO BMFF container", "application/octet-stream", "")
        };
        return Detected { kind, mime, category: if ext.is_empty() { "binary" } else { "image" }, ext };
    }
    if bytes.starts_with(b"%PDF-") {
        return Detected { kind: "PDF document", mime: "application/pdf", category: "document", ext: "pdf" };
    }
    if bytes.starts_with(b"PK\x03\x04") || bytes.starts_with(b"PK\x05\x06") || bytes.starts_with(b"PK\x07\x08") {
        return detect_zip_flavour(bytes);
    }
    Detected { kind: "unknown file", mime: "application/octet-stream", category: "unknown", ext: "" }
}

fn detect_zip_flavour(bytes: &[u8]) -> Detected {
    let has = |name: &str| {
        zip::ZipArchive::new(Cursor::new(bytes))
            .ok()
            .and_then(|mut z| z.by_name(name).ok().map(|_| ()))
            .is_some()
    };
    if has("word/document.xml") || has("docProps/core.xml") {
        return Detected { kind: "Office Open XML document", mime: "application/vnd.openxmlformats-officedocument.wordprocessingml.document", category: "document", ext: "docx" };
    }
    if has("xl/workbook.xml") {
        return Detected { kind: "Office Open XML spreadsheet", mime: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet", category: "document", ext: "xlsx" };
    }
    if has("ppt/presentation.xml") {
        return Detected { kind: "Office Open XML presentation", mime: "application/vnd.openxmlformats-officedocument.presentationml.presentation", category: "document", ext: "pptx" };
    }
    if has("META-INF/container.xml") {
        return Detected { kind: "EPUB ebook", mime: "application/epub+zip", category: "document", ext: "epub" };
    }
    if has("mimetype") && has("meta.xml") {
        return Detected { kind: "OpenDocument file", mime: "application/vnd.oasis.opendocument", category: "document", ext: "odt" };
    }
    Detected { kind: "ZIP archive", mime: "application/zip", category: "archive", ext: "zip" }
}

/// Which extractor a sniffed extension routes to.
enum Family {
    Image,
    Pdf,
    Zip,
    Other,
}

fn family_of(ext: &str) -> Family {
    match ext {
        "jpg" | "jpeg" | "png" | "tif" | "tiff" | "webp" | "heic" | "heif" | "avif" => {
            Family::Image
        }
        "pdf" => Family::Pdf,
        "zip" | "docx" | "xlsx" | "pptx" | "odt" | "ods" | "odp" | "epub" => Family::Zip,
        _ => Family::Other,
    }
}

/// One-line verdict. Files with nothing embedded get the explicit
/// "no supported metadata found" wording rather than an empty report.
fn summarize(format: &str, field_count: usize, groups: &[Group]) -> String {
    if field_count == 0 {
        return format!(
            "No supported metadata found in this {format} — it carries no EXIF, XMP, PDF Info or document properties."
        );
    }
    let names: Vec<&str> = groups.iter().map(|g| g.name.as_str()).collect();
    format!(
        "{field_count} metadata field(s) found in this {format}, from: {}.",
        names.join(", ")
    )
}

/// Clamp a value so one enormous field can't swamp the report.
fn clamp_value(v: &str) -> String {
    let v = v.trim();
    if v.chars().count() <= MAX_VALUE_CHARS {
        return v.to_string();
    }
    let head: String = v.chars().take(MAX_VALUE_CHARS).collect();
    format!("{head}… (truncated)")
}

// ---------------------------------------------------------------- EXIF / TIFF

/// Convert a 3-rational [deg, min, sec] + hemisphere ref to signed decimal.
fn dms_to_decimal(exif: &exif::Exif, coord: Tag, refr: Tag) -> Option<f64> {
    let v = exif.get_field(coord, In::PRIMARY)?;
    let parts = match &v.value {
        Value::Rational(r) if r.len() >= 3 => [r[0].to_f64(), r[1].to_f64(), r[2].to_f64()],
        _ => return None,
    };
    let mut deg = parts[0] + parts[1] / 60.0 + parts[2] / 3600.0;
    if let Some(rf) = exif.get_field(refr, In::PRIMARY) {
        let s = rf.display_value().to_string().to_ascii_uppercase();
        if s.contains('S') || s.contains('W') {
            deg = -deg;
        }
    }
    Some((deg * 1e6).round() / 1e6)
}

/// Read the EXIF/TIFF block. A file with no EXIF is not an error — it just
/// yields no group (and no note, since "image without EXIF" is normal).
fn read_exif(bytes: &[u8]) -> (Option<Group>, Option<Gps>, Option<String>) {
    let exif = match exif::Reader::new().read_from_container(&mut Cursor::new(bytes)) {
        Ok(e) => e,
        Err(exif::Error::NotFound(_)) => return (None, None, None),
        Err(e) => {
            return (
                None,
                None,
                Some(format!("EXIF block present but unreadable: {e}")),
            )
        }
    };

    let fields: Vec<Field> = exif
        .fields()
        .map(|f| Field {
            // Thumbnail-IFD tags repeat the primary names — keep them apart.
            name: if f.ifd_num == In::PRIMARY {
                f.tag.to_string()
            } else {
                format!("{} [IFD {}]", f.tag, f.ifd_num)
            },
            value: clamp_value(&f.display_value().with_unit(&exif).to_string()),
        })
        .collect();

    let gps = match (
        dms_to_decimal(&exif, Tag::GPSLatitude, Tag::GPSLatitudeRef),
        dms_to_decimal(&exif, Tag::GPSLongitude, Tag::GPSLongitudeRef),
    ) {
        (Some(latitude), Some(longitude)) => Some(Gps {
            latitude,
            longitude,
        }),
        _ => None,
    };

    if fields.is_empty() {
        return (None, gps, None);
    }
    (
        Some(Group {
            name: "EXIF".into(),
            fields,
        }),
        gps,
        None,
    )
}

// ------------------------------------------------------------------------ XMP

/// First index of `pat` in `hay`.
fn find(hay: &[u8], pat: &[u8]) -> Option<usize> {
    if pat.is_empty() || hay.len() < pat.len() {
        return None;
    }
    hay.windows(pat.len()).position(|w| w == pat)
}

/// Locate the `<x:xmpmeta>…</x:xmpmeta>` packet in raw file bytes.
fn find_xmp_packet(bytes: &[u8]) -> Option<&[u8]> {
    let scan = &bytes[..bytes.len().min(MAX_XMP_SCAN)];
    let start = find(scan, b"<x:xmpmeta")?;
    const END: &[u8] = b"</x:xmpmeta>";
    let end = find(&scan[start..], END)? + start + END.len();
    let packet = &scan[start..end];
    (packet.len() <= MAX_XMP_BYTES).then_some(packet)
}

/// Extract the XMP packet's properties, if the file carries one.
fn read_xmp(bytes: &[u8]) -> Option<Group> {
    let packet = find_xmp_packet(bytes)?;
    let fields = flatten_xml(packet, /* description_attrs */ true);
    (!fields.is_empty()).then(|| Group {
        name: "XMP".into(),
        fields,
    })
}

/// Local name of a possibly-prefixed qname (`dc:title` → `title`).
fn local_name(raw: &[u8]) -> &[u8] {
    match raw.iter().position(|&b| b == b':') {
        Some(i) => &raw[i + 1..],
        None => raw,
    }
}

/// A qname as a lossy string, for display (`dc:title` stays `dc:title`).
fn qname_string(raw: &[u8]) -> String {
    String::from_utf8_lossy(raw).into_owned()
}

/// Append `name = value`, merging repeats (RDF bags, multi-valued `dc:creator`)
/// into one `a; b` entry instead of emitting duplicate rows.
fn push_field(out: &mut Vec<Field>, name: String, value: String) {
    let value = clamp_value(&value);
    if value.is_empty() {
        return;
    }
    if let Some(existing) = out.iter_mut().find(|f| f.name == name) {
        if !existing.value.split("; ").any(|v| v == value) {
            existing.value = clamp_value(&format!("{}; {}", existing.value, value));
        }
        return;
    }
    out.push(Field { name, value });
}

/// Pull the metadata-bearing leaves out of a small XML document.
///
/// Records the text of every leaf element (keyed by its qname), unwrapping RDF
/// `Bag`/`Seq`/`Alt` containers so `dc:creator/rdf:Seq/rdf:li` reports as
/// `dc:creator`. With `description_attrs`, XMP's attribute shorthand form
/// (`<rdf:Description tiff:Make="ACME"/>`) is picked up too. Best-effort: a
/// malformed document yields whatever was parsed before the error.
fn flatten_xml(xml: &[u8], description_attrs: bool) -> Vec<Field> {
    let mut reader = Reader::from_reader(xml);
    let mut buf = Vec::new();
    let mut stack: Vec<Vec<u8>> = Vec::new();
    let mut out: Vec<Field> = Vec::new();

    loop {
        let ev = match reader.read_event_into(&mut buf) {
            Ok(e) => e,
            Err(_) => break, // best-effort: keep what we already parsed
        };
        match ev {
            Event::Start(e) => {
                if description_attrs {
                    push_description_attrs(&e, &mut out);
                }
                stack.push(e.name().as_ref().to_vec());
            }
            Event::Empty(e) => {
                if description_attrs {
                    push_description_attrs(&e, &mut out);
                }
            }
            Event::End(_) => {
                stack.pop();
            }
            Event::Text(t) => {
                let text = match t.decode() {
                    Ok(s) => s.trim().to_string(),
                    Err(_) => continue,
                };
                if text.is_empty() {
                    continue;
                }
                if let Some(name) = owning_property(&stack) {
                    push_field(&mut out, name, text);
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
        if out.len() >= MAX_FIELDS_PER_GROUP {
            break;
        }
    }
    out
}

/// The property name a text node belongs to: normally its parent element, but
/// for `<prop><rdf:Seq><rdf:li>text` it is the great-grandparent `prop`.
fn owning_property(stack: &[Vec<u8>]) -> Option<String> {
    let last = stack.last()?;
    let name = if local_name(last) == b"li" {
        // …/prop/Bag|Seq|Alt/li  → walk back to `prop`.
        stack.get(stack.len().checked_sub(3)?)?
    } else {
        last
    };
    match local_name(name) {
        // Structural RDF/XMP wrappers carry no value of their own.
        b"RDF" | b"Description" | b"xmpmeta" | b"Bag" | b"Seq" | b"Alt" => None,
        _ => Some(qname_string(name)),
    }
}

/// XMP's shorthand form puts properties in `rdf:Description` attributes.
fn push_description_attrs(e: &BytesStart, out: &mut Vec<Field>) {
    if local_name(e.name().as_ref()) != b"Description" {
        return;
    }
    for attr in e.attributes().flatten() {
        let key = attr.key.as_ref().to_vec();
        if key.starts_with(b"xmlns") || local_name(&key) == b"about" {
            continue;
        }
        if let Ok(v) = attr.unescape_value() {
            push_field(out, qname_string(&key), v.trim().to_string());
        }
    }
}

// ------------------------------------------------------------------------ PDF

/// Decode a PDF text string: UTF-16BE when it carries a BOM, else
/// PDFDocEncoding (approximated as Latin-1, exact for ASCII).
fn decode_pdf_string(bytes: &[u8]) -> String {
    if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
        let units: Vec<u16> = bytes[2..]
            .chunks(2)
            .map(|c| {
                if c.len() == 2 {
                    u16::from_be_bytes([c[0], c[1]])
                } else {
                    c[0] as u16
                }
            })
            .collect();
        String::from_utf16_lossy(&units)
    } else {
        bytes.iter().map(|&b| b as char).collect()
    }
}

/// Render a PDF date string (`D:20240102030405+01'00'`) as `2024-01-02 03:04:05
/// +01:00`. Anything that doesn't match is returned unchanged.
fn format_pdf_date(raw: &str) -> String {
    let d = raw.strip_prefix("D:").unwrap_or(raw);
    if d.len() < 4 || !d[..4].bytes().all(|b| b.is_ascii_digit()) {
        return raw.to_string();
    }
    let part = |from: usize, len: usize| -> Option<&str> {
        let s = d.get(from..from + len)?;
        s.bytes().all(|b| b.is_ascii_digit()).then_some(s)
    };
    let mut out = d[..4].to_string();
    for (from, sep) in [(4, "-"), (6, "-")] {
        match part(from, 2) {
            Some(p) => out.push_str(&format!("{sep}{p}")),
            None => return out,
        }
    }
    for (from, sep) in [(8, " "), (10, ":"), (12, ":")] {
        match part(from, 2) {
            Some(p) => out.push_str(&format!("{sep}{p}")),
            None => return out,
        }
    }
    // Trailing offset: `+01'00'`, `-0500`, or `Z`.
    let tz = d[14..].replace('\'', ":");
    let tz = tz.trim_end_matches(':');
    if !tz.is_empty() {
        out.push(' ');
        out.push_str(tz);
    }
    out
}

/// PDF header version, e.g. `1.7`, straight off the `%PDF-x.y` magic.
fn pdf_header_version(bytes: &[u8]) -> Option<String> {
    let head = bytes.get(..9)?;
    let v = head.strip_prefix(b"%PDF-")?;
    let v = String::from_utf8_lossy(&v[..v.len().min(3)]).into_owned();
    v.chars()
        .next()
        .is_some_and(|c| c.is_ascii_digit())
        .then_some(v)
}

/// Read the PDF document-information dictionary plus structural facts.
fn read_pdf(bytes: &[u8]) -> (Vec<Group>, Vec<String>) {
    use lopdf::{Document, Object};

    let mut notes = Vec::new();
    let mut doc_fields: Vec<Field> = Vec::new();
    if let Some(v) = pdf_header_version(bytes) {
        doc_fields.push(Field {
            name: "PDF version".into(),
            value: v,
        });
    }

    let doc = match Document::load_mem(bytes) {
        Ok(d) => d,
        Err(e) => {
            notes.push(format!(
                "the PDF structure could not be parsed ({e}) — only the header was read"
            ));
            let groups = if doc_fields.is_empty() {
                vec![]
            } else {
                vec![Group {
                    name: "PDF document".into(),
                    fields: doc_fields,
                }]
            };
            return (groups, notes);
        }
    };

    doc_fields.push(Field {
        name: "Pages".into(),
        value: doc.get_pages().len().to_string(),
    });
    if doc.trailer.get(b"Encrypt").is_ok() {
        doc_fields.push(Field {
            name: "Encrypted".into(),
            value: "yes".into(),
        });
        notes.push(
            "this PDF is encrypted — text and embedded metadata streams may be unreadable".into(),
        );
    }

    // The Info dictionary is either referenced from the trailer or inline.
    let info = match doc.trailer.get(b"Info") {
        Ok(Object::Reference(id)) => doc.get_dictionary(*id).ok().cloned(),
        Ok(Object::Dictionary(d)) => Some(d.clone()),
        _ => None,
    };
    let mut info_fields: Vec<Field> = Vec::new();
    if let Some(dict) = info {
        for (key, value) in dict.iter() {
            let name = String::from_utf8_lossy(key).into_owned();
            let is_date = name.ends_with("Date");
            let rendered = match value {
                Object::String(b, _) => {
                    let s = decode_pdf_string(b);
                    if is_date {
                        format_pdf_date(&s)
                    } else {
                        s
                    }
                }
                Object::Name(n) => String::from_utf8_lossy(n).into_owned(),
                Object::Integer(i) => i.to_string(),
                Object::Real(r) => r.to_string(),
                Object::Boolean(b) => b.to_string(),
                _ => continue, // streams/arrays aren't document-info metadata
            };
            push_field(&mut info_fields, name, rendered);
        }
    }

    let mut groups = vec![Group {
        name: "PDF document".into(),
        fields: doc_fields,
    }];
    if !info_fields.is_empty() {
        groups.push(Group {
            name: "PDF Info".into(),
            fields: info_fields,
        });
    }
    // The catalog's /Metadata stream holds XMP — usually uncompressed, but read
    // it through lopdf so a Flate-compressed one is still surfaced.
    if let Some(xmp) = pdf_xmp(&doc) {
        let fields = flatten_xml(&xmp, true);
        if !fields.is_empty() {
            groups.push(Group {
                name: "XMP".into(),
                fields,
            });
        }
    }
    (groups, notes)
}

/// Decoded bytes of the catalog's `/Metadata` XMP stream, if present.
fn pdf_xmp(doc: &lopdf::Document) -> Option<Vec<u8>> {
    use lopdf::Object;
    let catalog = doc.catalog().ok()?;
    let obj = match catalog.get(b"Metadata").ok()? {
        Object::Reference(id) => doc.get_object(*id).ok()?,
        other => other,
    };
    let stream = obj.as_stream().ok()?;
    stream
        .decompressed_content()
        .ok()
        .or_else(|| Some(stream.content.clone()))
}

// ------------------------------------------------------- ZIP containers (OOXML / ODF / EPUB)

/// Read one container member, capped so a zip bomb can't blow up memory.
fn read_zip_entry(zip: &mut zip::ZipArchive<Cursor<&[u8]>>, name: &str) -> Option<Vec<u8>> {
    let entry = zip.by_name(name).ok()?;
    if entry.size() > MAX_XML_BYTES {
        return None;
    }
    let mut buf = Vec::with_capacity(entry.size() as usize);
    entry.take(MAX_XML_BYTES).read_to_end(&mut buf).ok()?;
    Some(buf)
}

/// Document properties out of an OOXML / OpenDocument / EPUB container.
fn read_zip_container(bytes: &[u8]) -> (Vec<Group>, Vec<String>) {
    let mut zip = match zip::ZipArchive::new(Cursor::new(bytes)) {
        Ok(z) => z,
        Err(e) => return (vec![], vec![format!("the ZIP container is unreadable: {e}")]),
    };

    let mut groups = Vec::new();
    // OOXML (docx/xlsx/pptx): the two standard property parts.
    for (entry, group) in [
        ("docProps/core.xml", "Document properties"),
        ("docProps/app.xml", "Application properties"),
    ] {
        if let Some(xml) = read_zip_entry(&mut zip, entry) {
            let fields = flatten_xml(&xml, false);
            if !fields.is_empty() {
                groups.push(Group {
                    name: group.into(),
                    fields,
                });
            }
        }
    }
    // OpenDocument (odt/ods/odp).
    if groups.is_empty() {
        if let Some(xml) = read_zip_entry(&mut zip, "meta.xml") {
            let fields = flatten_xml(&xml, false);
            if !fields.is_empty() {
                groups.push(Group {
                    name: "Document properties".into(),
                    fields,
                });
            }
        }
    }
    // EPUB: the OPF package document named by META-INF/container.xml. Only its
    // `<metadata>` leaves carry text, so the same flattening is safe here.
    if groups.is_empty() {
        if let Some(opf_path) = epub_opf_path(&mut zip) {
            if let Some(xml) = read_zip_entry(&mut zip, &opf_path) {
                let fields = flatten_xml(&xml, false);
                if !fields.is_empty() {
                    groups.push(Group {
                        name: "EPUB metadata".into(),
                        fields,
                    });
                }
            }
        }
    }
    (groups, vec![])
}

/// `<rootfile full-path="…">` from an EPUB's `META-INF/container.xml`.
fn epub_opf_path(zip: &mut zip::ZipArchive<Cursor<&[u8]>>) -> Option<String> {
    let xml = read_zip_entry(zip, "META-INF/container.xml")?;
    let mut reader = Reader::from_reader(xml.as_slice());
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == b"rootfile" =>
            {
                for attr in e.attributes().flatten() {
                    if local_name(attr.key.as_ref()) == b"full-path" {
                        return String::from_utf8(attr.value.into_owned()).ok();
                    }
                }
            }
            Ok(Event::Eof) | Err(_) => return None,
            _ => {}
        }
        buf.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Minimal little-endian TIFF/EXIF with one IFD0 field: Make = "T".
    fn tiff_exif_make() -> Vec<u8> {
        let mut t: Vec<u8> = Vec::new();
        t.extend_from_slice(b"II");
        t.extend_from_slice(&42u16.to_le_bytes());
        t.extend_from_slice(&8u32.to_le_bytes()); // IFD0 offset
        t.extend_from_slice(&1u16.to_le_bytes()); // 1 entry
        t.extend_from_slice(&0x010Fu16.to_le_bytes()); // Make
        t.extend_from_slice(&2u16.to_le_bytes()); // ASCII
        t.extend_from_slice(&2u32.to_le_bytes()); // count
        t.extend_from_slice(b"T\0\0\0");
        t.extend_from_slice(&0u32.to_le_bytes()); // no next IFD
        t
    }

    fn jpeg_with_segments(segments: &[(u8, Vec<u8>)]) -> Vec<u8> {
        let mut j: Vec<u8> = vec![0xFF, 0xD8];
        for (marker, payload) in segments {
            j.push(0xFF);
            j.push(*marker);
            j.write_all(&((payload.len() + 2) as u16).to_be_bytes())
                .unwrap();
            j.extend_from_slice(payload);
        }
        j.extend_from_slice(&[0xFF, 0xD9]);
        j
    }

    fn jpeg_with_exif() -> Vec<u8> {
        let mut payload = b"Exif\0\0".to_vec();
        payload.extend_from_slice(&tiff_exif_make());
        jpeg_with_segments(&[(0xE1, payload)])
    }

    const XMP: &str = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/">
      <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
        <rdf:Description rdf:about="" xmlns:dc="http://purl.org/dc/elements/1.1/"
                         xmlns:xmp="http://ns.adobe.com/xap/1.0/" xmp:CreatorTool="Gizza Test">
          <dc:title><rdf:Alt><rdf:li xml:lang="x-default">Holiday</rdf:li></rdf:Alt></dc:title>
          <dc:creator><rdf:Seq><rdf:li>Ada</rdf:li><rdf:li>Grace</rdf:li></rdf:Seq></dc:creator>
        </rdf:Description>
      </rdf:RDF>
    </x:xmpmeta>"#;

    fn jpeg_with_xmp() -> Vec<u8> {
        let mut payload = b"http://ns.adobe.com/xap/1.0/\0".to_vec();
        payload.extend_from_slice(XMP.as_bytes());
        jpeg_with_segments(&[(0xE1, payload)])
    }

    fn pdf_with_info(fields: &[(&str, &str)]) -> Vec<u8> {
        use lopdf::{dictionary, Document, Object, StringFormat};
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 200.into(), 200.into()],
        });
        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Count" => 1,
                "Kids" => vec![page_id.into()],
            }),
        );
        let catalog_id = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
        doc.trailer.set("Root", catalog_id);
        let mut info = lopdf::Dictionary::new();
        for (k, v) in fields {
            info.set(
                k.as_bytes().to_vec(),
                Object::String(v.as_bytes().to_vec(), StringFormat::Literal),
            );
        }
        let info_id = doc.add_object(info);
        doc.trailer.set("Info", info_id);
        let mut out = Vec::new();
        doc.save_to(&mut out).unwrap();
        out
    }

    fn zip_with(entries: &[(&str, &str)]) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut w = zip::ZipWriter::new(Cursor::new(&mut buf));
            let opts = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            for (name, body) in entries {
                w.start_file(*name, opts).unwrap();
                w.write_all(body.as_bytes()).unwrap();
            }
            w.finish().unwrap();
        }
        buf
    }

    #[test]
    fn reads_exif_from_jpeg() {
        let r = inspect(&jpeg_with_exif()).unwrap();
        assert_eq!(r.format, "JPEG image");
        let exif = r.groups.iter().find(|g| g.name == "EXIF").expect("EXIF group");
        let make = exif.fields.iter().find(|f| f.name == "Make").expect("Make");
        assert!(make.value.contains('T'), "got {:?}", make.value);
        assert!(r.gps.is_none());
        assert!(r.summary.contains("metadata field(s) found"), "{}", r.summary);
    }

    #[test]
    fn reads_xmp_packet_and_unwraps_rdf_containers() {
        let r = inspect(&jpeg_with_xmp()).unwrap();
        let xmp = r.groups.iter().find(|g| g.name == "XMP").expect("XMP group");
        let get = |n: &str| xmp.fields.iter().find(|f| f.name == n).map(|f| f.value.clone());
        assert_eq!(get("dc:title").as_deref(), Some("Holiday"));
        // Both rdf:li values merge into one dc:creator entry.
        assert_eq!(get("dc:creator").as_deref(), Some("Ada; Grace"));
        // Attribute shorthand on rdf:Description is picked up too.
        assert_eq!(get("xmp:CreatorTool").as_deref(), Some("Gizza Test"));
        // Structural RDF wrappers never become fields.
        assert!(xmp.fields.iter().all(|f| !f.name.contains("rdf:")));
    }

    #[test]
    fn reads_pdf_info_dictionary() {
        let pdf = pdf_with_info(&[
            ("Title", "Quarterly Report"),
            ("Producer", "gizza"),
            ("CreationDate", "D:20240102030405+01'00'"),
        ]);
        let r = inspect(&pdf).unwrap();
        assert_eq!(r.mime, "application/pdf");
        let doc = r.groups.iter().find(|g| g.name == "PDF document").expect("doc group");
        assert_eq!(
            doc.fields.iter().find(|f| f.name == "Pages").map(|f| f.value.as_str()),
            Some("1")
        );
        let info = r.groups.iter().find(|g| g.name == "PDF Info").expect("info group");
        let get = |n: &str| info.fields.iter().find(|f| f.name == n).map(|f| f.value.clone());
        assert_eq!(get("Title").as_deref(), Some("Quarterly Report"));
        assert_eq!(get("Producer").as_deref(), Some("gizza"));
        assert_eq!(get("CreationDate").as_deref(), Some("2024-01-02 03:04:05 +01:00"));
    }

    #[test]
    fn reads_ooxml_doc_props() {
        let core = r#"<?xml version="1.0"?>
            <cp:coreProperties xmlns:cp="x" xmlns:dc="y" xmlns:dcterms="z">
              <dc:title>Budget</dc:title>
              <dc:creator>Ada Lovelace</dc:creator>
              <cp:lastModifiedBy>Grace Hopper</cp:lastModifiedBy>
              <dcterms:created>2024-01-02T03:04:05Z</dcterms:created>
            </cp:coreProperties>"#;
        let app = r#"<Properties><Application>Microsoft Office Word</Application><Company>ACME</Company></Properties>"#;
        let docx = zip_with(&[
            ("[Content_Types].xml", "<Types/>"),
            ("word/document.xml", "<w:document/>"),
            ("docProps/core.xml", core),
            ("docProps/app.xml", app),
        ]);
        let r = inspect(&docx).unwrap();
        let props = r
            .groups
            .iter()
            .find(|g| g.name == "Document properties")
            .expect("core props");
        let get = |n: &str| props.fields.iter().find(|f| f.name == n).map(|f| f.value.clone());
        assert_eq!(get("dc:creator").as_deref(), Some("Ada Lovelace"));
        assert_eq!(get("cp:lastModifiedBy").as_deref(), Some("Grace Hopper"));
        let appg = r
            .groups
            .iter()
            .find(|g| g.name == "Application properties")
            .expect("app props");
        assert!(appg
            .fields
            .iter()
            .any(|f| f.name == "Company" && f.value == "ACME"));
    }

    #[test]
    fn unsupported_format_reports_cleanly() {
        let r = inspect(b"just some plain text, no metadata anywhere").unwrap();
        assert_eq!(r.field_count, 0);
        assert!(r.groups.is_empty());
        assert!(
            r.summary.starts_with("No supported metadata found"),
            "got {:?}",
            r.summary
        );
    }

    #[test]
    fn image_without_exif_is_not_an_error() {
        // 8-byte PNG signature only — a valid sniff, no metadata, no panic.
        let r = inspect(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]).unwrap();
        assert_eq!(r.format, "PNG image");
        assert_eq!(r.field_count, 0);
        assert!(r.summary.contains("No supported metadata found"));
    }

    #[test]
    fn truncated_containers_do_not_panic() {
        // A PDF header with nothing behind it, and a ZIP magic with no entries.
        let pdf = inspect(b"%PDF-1.7\n%garbage").unwrap();
        assert!(pdf.notes.iter().any(|n| n.contains("could not be parsed")));
        assert!(pdf
            .groups
            .iter()
            .any(|g| g.fields.iter().any(|f| f.name == "PDF version" && f.value == "1.7")));

        let zip = inspect(b"PK\x03\x04truncated").unwrap();
        assert!(zip.summary.contains("No supported metadata found") || zip.field_count > 0);
    }

    #[test]
    fn empty_input_errors() {
        assert!(inspect(b"").is_err());
    }

    #[test]
    fn pdf_dates_that_do_not_match_are_passed_through() {
        assert_eq!(format_pdf_date("D:2024"), "2024");
        assert_eq!(format_pdf_date("not a date"), "not a date");
        assert_eq!(format_pdf_date("D:20240102030405Z"), "2024-01-02 03:04:05 Z");
    }

    #[test]
    fn long_values_are_clamped() {
        let long = "x".repeat(MAX_VALUE_CHARS + 50);
        assert!(clamp_value(&long).ends_with("… (truncated)"));
        assert_eq!(clamp_value("  spaced  "), "spaced");
    }
}
