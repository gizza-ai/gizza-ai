//! metadata-privacy-linter core — pure compute, shared by the chat skill block.
//!
//! Scans an image's embedded metadata for privacy-sensitive fields and reports
//! *what would leak* if the image were shared, classified by category + risk:
//!   - **EXIF/TIFF** via `kamadak-exif` — GPS, camera/lens serials, owner name,
//!     software, timestamps, description/comment.
//!   - **XMP** — the XML packet is located in the raw bytes and scanned for known
//!     privacy property local-names (dc:creator, dc:rights, xmp:CreatorTool,
//!     exif GPS, aux serials, keywords).
//!   - **IPTC (IIM)** — the JPEG APP13 Photoshop IRB resource 0x0404 is parsed
//!     for By-line, Copyright, City/State/Country, Credit, Caption, Keywords, …
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker.

use std::io::Cursor;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use exif::{In, Tag, Value};
use serde::Serialize;

/// Risk level of a single leaking field. Ordered Low < Medium < High.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Risk {
    Low,
    Medium,
    High,
}

impl Risk {
    fn rank(self) -> u8 {
        match self {
            Risk::Low => 1,
            Risk::Medium => 2,
            Risk::High => 3,
        }
    }
    /// Parse the `min_risk` filter param. `all` keeps everything (threshold Low).
    fn threshold(min_risk: &str) -> Result<u8, String> {
        match min_risk {
            "all" => Ok(1),
            "medium" => Ok(2),
            "high" => Ok(3),
            other => Err(format!(
                "invalid min_risk {other:?}: expected one of all, medium, high"
            )),
        }
    }
}

/// A privacy-sensitive field discovered in the image's metadata.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Finding {
    /// Where it came from: "exif", "xmp", or "iptc".
    pub source: &'static str,
    /// Human-readable field name (e.g. "GPSLatitude", "By-line (creator)").
    pub field: String,
    /// Privacy category: location, device, personal, timestamp, software, description.
    pub category: &'static str,
    /// How sensitive this field is.
    pub risk: Risk,
    /// The field's value. Omitted when `reveal_values` is false so the report
    /// itself is shareable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

/// GPS coordinates decoded from EXIF to signed decimal degrees.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Gps {
    pub latitude: f64,
    pub longitude: f64,
}

/// The flat privacy report an LLM / CLI user reads directly.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    /// Detected container format: jpeg, png, tiff, webp, heif, gif, or "unknown".
    pub format: &'static str,
    /// True when nothing at or above the `min_risk` threshold was found.
    pub clean: bool,
    /// Number of findings after the `min_risk` filter.
    pub findings_count: usize,
    /// The leaking fields, most-sensitive first.
    pub findings: Vec<Finding>,
    /// Distinct categories present among the findings.
    pub categories: Vec<&'static str>,
    /// EXIF GPS decoded to decimal degrees (present only when found AND
    /// `reveal_values` is true).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gps: Option<Gps>,
    /// An OpenStreetMap link to the decoded GPS location.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gps_map_url: Option<String>,
    /// Plain-English "what would leak" summary.
    pub summary: String,
    /// Whether values were redacted (i.e. `reveal_values` was false).
    pub values_hidden: bool,
}

// --- categories -----------------------------------------------------------
const LOCATION: &str = "location";
const DEVICE: &str = "device";
const PERSONAL: &str = "personal";
const TIMESTAMP: &str = "timestamp";
const SOFTWARE: &str = "software";
const DESCRIPTION: &str = "description";

/// Detect the container format from magic bytes.
fn detect_format(b: &[u8]) -> &'static str {
    if b.len() >= 3 && b[0] == 0xFF && b[1] == 0xD8 && b[2] == 0xFF {
        "jpeg"
    } else if b.len() >= 8 && &b[0..8] == b"\x89PNG\r\n\x1a\n" {
        "png"
    } else if b.len() >= 4 && (&b[0..4] == b"II\x2a\x00" || &b[0..4] == b"MM\x00\x2a") {
        "tiff"
    } else if b.len() >= 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        "webp"
    } else if b.len() >= 12 && &b[4..8] == b"ftyp" {
        "heif"
    } else if b.len() >= 6 && (&b[0..6] == b"GIF87a" || &b[0..6] == b"GIF89a") {
        "gif"
    } else {
        "unknown"
    }
}

/// Truncate a value so a single verbose field can't dominate the report.
fn clip(s: &str) -> String {
    let s = s.trim();
    let mut out: String = s.chars().take(256).collect();
    if s.chars().count() > 256 {
        out.push('…');
    }
    out
}

/// Classify an EXIF tag into (field-name, category, risk). Returns None for
/// tags that carry no privacy signal (exposure, aperture, etc.).
fn classify_exif(tag: Tag) -> Option<(&'static str, &'static str, Risk)> {
    Some(match tag {
        // Location — GPS is the highest-risk leak (reveals home / routine).
        Tag::GPSLatitude | Tag::GPSLongitude | Tag::GPSAltitude | Tag::GPSDestLatitude
        | Tag::GPSDestLongitude => ("GPS coordinates", LOCATION, Risk::High),
        Tag::GPSAreaInformation | Tag::GPSProcessingMethod => {
            ("GPS area/method", LOCATION, Risk::Medium)
        }
        Tag::GPSDateStamp | Tag::GPSTimeStamp => ("GPS timestamp", LOCATION, Risk::Medium),
        // Device identity — serials are a fingerprint linking photos to one owner.
        Tag::BodySerialNumber | Tag::LensSerialNumber | Tag::CameraOwnerName => {
            ("Device serial / owner", DEVICE, Risk::High)
        }
        Tag::Make | Tag::Model | Tag::LensMake | Tag::LensModel => {
            ("Camera / lens model", DEVICE, Risk::Low)
        }
        // Personal identity.
        Tag::Artist => ("Artist / author", PERSONAL, Risk::High),
        Tag::Copyright => ("Copyright / owner", PERSONAL, Risk::Medium),
        // Timestamps — reveal daily patterns.
        Tag::DateTimeOriginal | Tag::DateTimeDigitized | Tag::DateTime => {
            ("Capture timestamp", TIMESTAMP, Risk::Medium)
        }
        // Software / host.
        Tag::Software => ("Editing software / host", SOFTWARE, Risk::Low),
        // Free text the author may have written.
        Tag::ImageDescription | Tag::UserComment => {
            ("Description / comment", DESCRIPTION, Risk::Medium)
        }
        _ => return None,
    })
}

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

/// Scan EXIF/TIFF, appending findings + returning decoded GPS if present.
fn scan_exif(bytes: &[u8], findings: &mut Vec<(&'static str, String, &'static str, Risk, String)>) -> Option<Gps> {
    let exif = exif::Reader::new()
        .read_from_container(&mut Cursor::new(bytes))
        .ok()?;

    // De-dup by (field-name, category) so the Lat + Lon + Alt trio collapses to
    // one "GPS coordinates" finding, and the several timestamp tags to one.
    let mut seen: Vec<(&'static str, &'static str)> = Vec::new();
    for f in exif.fields() {
        if let Some((name, cat, risk)) = classify_exif(f.tag) {
            if seen.contains(&(name, cat)) {
                continue;
            }
            seen.push((name, cat));
            let val = f.display_value().with_unit(&exif).to_string();
            findings.push(("exif", name.to_string(), cat, risk, clip(&val)));
        }
    }

    match (
        dms_to_decimal(&exif, Tag::GPSLatitude, Tag::GPSLatitudeRef),
        dms_to_decimal(&exif, Tag::GPSLongitude, Tag::GPSLongitudeRef),
    ) {
        (Some(lat), Some(lon)) => Some(Gps { latitude: lat, longitude: lon }),
        _ => None,
    }
}

// --- XMP ------------------------------------------------------------------

/// Locate the `<x:xmpmeta … </x:xmpmeta>` packet in the raw bytes.
fn find_xmp(bytes: &[u8]) -> Option<String> {
    let start = find_sub(bytes, b"<x:xmpmeta")?;
    let rel_end = find_sub(&bytes[start..], b"</x:xmpmeta>")?;
    let end = start + rel_end + b"</x:xmpmeta>".len();
    Some(String::from_utf8_lossy(&bytes[start..end]).into_owned())
}

fn find_sub(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || hay.len() < needle.len() {
        return None;
    }
    (0..=hay.len() - needle.len()).find(|&i| &hay[i..i + needle.len()] == needle)
}

/// XMP privacy properties to look for: (local-name, field label, category, risk).
const XMP_PROPS: &[(&str, &str, &str, Risk)] = &[
    ("GPSLatitude", "XMP GPS latitude", LOCATION, Risk::High),
    ("GPSLongitude", "XMP GPS longitude", LOCATION, Risk::High),
    ("SerialNumber", "Device serial number", DEVICE, Risk::High),
    ("LensSerialNumber", "Lens serial number", DEVICE, Risk::High),
    ("creator", "Creator", PERSONAL, Risk::High),
    ("rights", "Rights / owner", PERSONAL, Risk::Medium),
    ("CreatorTool", "Creator tool / software", SOFTWARE, Risk::Low),
    ("CreateDate", "Create date", TIMESTAMP, Risk::Medium),
    ("ModifyDate", "Modify date", TIMESTAMP, Risk::Medium),
    ("DateTimeOriginal", "Original date/time", TIMESTAMP, Risk::Medium),
    ("subject", "Keywords / subject", DESCRIPTION, Risk::Low),
    ("description", "Description", DESCRIPTION, Risk::Medium),
];

/// Best-effort value extraction for an XMP property local-name — handles both
/// the attribute form (`ns:Prop="value"`) and the element form
/// (`<ns:Prop>value</ns:Prop>`, including an `rdf:Seq`/`rdf:li` wrapper).
fn xmp_value(xmp: &str, local: &str) -> Option<String> {
    if let Some(v) = xmp_attr(xmp, local) {
        return Some(v);
    }
    let pos = find_open_tag(xmp, local)?;
    let rest = &xmp[pos..];
    let content_end = rest.find('<').unwrap_or(rest.len());
    let direct = rest[..content_end].trim();
    if !direct.is_empty() {
        return Some(clip(direct));
    }
    // Value is wrapped, e.g. <rdf:Seq><rdf:li>Name</rdf:li></rdf:Seq>.
    if let Some(li_open) = rest.find("<rdf:li") {
        let after = &rest[li_open..];
        if let Some(gt) = after.find('>') {
            let inner = &after[gt + 1..];
            let inner_end = inner.find('<').unwrap_or(inner.len());
            let li = inner[..inner_end].trim();
            if !li.is_empty() {
                return Some(clip(li));
            }
        }
    }
    Some(String::new())
}

/// Attribute form: `local="value"`.
fn xmp_attr(xmp: &str, local: &str) -> Option<String> {
    let pat = format!("{local}=\"");
    let idx = xmp.find(&pat)?;
    let start = idx + pat.len();
    let rest = &xmp[start..];
    let end = rest.find('"')?;
    let v = rest[..end].trim();
    if v.is_empty() {
        None
    } else {
        Some(clip(v))
    }
}

/// Return the byte offset just after the `>` of an opening `<ns:local …>` tag.
fn find_open_tag(xmp: &str, local: &str) -> Option<usize> {
    let b = xmp.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'<' && i + 1 < b.len() && !matches!(b[i + 1], b'/' | b'?' | b'!') {
            let mut j = i + 1;
            while j < b.len() && !matches!(b[j], b'>' | b' ' | b'/' | b'\t' | b'\n' | b'\r') {
                j += 1;
            }
            let name = &xmp[i + 1..j];
            let ln = name.rsplit(':').next().unwrap_or(name);
            if ln == local {
                let mut k = j;
                while k < b.len() && b[k] != b'>' {
                    k += 1;
                }
                if k < b.len() {
                    return Some(k + 1);
                }
            }
        }
        i += 1;
    }
    None
}

fn scan_xmp(bytes: &[u8], findings: &mut Vec<(&'static str, String, &'static str, Risk, String)>) {
    let Some(xmp) = find_xmp(bytes) else { return };
    let mut seen: Vec<&'static str> = Vec::new();
    for &(local, label, cat, risk) in XMP_PROPS {
        if let Some(val) = xmp_value(&xmp, local) {
            if seen.contains(&label) {
                continue;
            }
            seen.push(label);
            findings.push(("xmp", label.to_string(), cat, risk, val));
        }
    }
}

// --- IPTC (IIM) -----------------------------------------------------------

/// Classify an IPTC IIM record-2 dataset number → (label, category, risk).
fn classify_iptc(dataset: u8) -> Option<(&'static str, &'static str, Risk)> {
    Some(match dataset {
        80 => ("By-line (creator)", PERSONAL, Risk::High),
        85 => ("By-line title", PERSONAL, Risk::Medium),
        110 => ("Credit", PERSONAL, Risk::Medium),
        115 => ("Source", PERSONAL, Risk::Low),
        116 => ("Copyright notice", PERSONAL, Risk::Medium),
        122 => ("Caption writer", PERSONAL, Risk::Medium),
        90 => ("City", LOCATION, Risk::Medium),
        92 => ("Sub-location", LOCATION, Risk::Medium),
        95 => ("Province / state", LOCATION, Risk::Low),
        101 => ("Country", LOCATION, Risk::Low),
        55 => ("Date created", TIMESTAMP, Risk::Medium),
        60 => ("Time created", TIMESTAMP, Risk::Medium),
        25 => ("Keywords", DESCRIPTION, Risk::Low),
        105 => ("Headline", DESCRIPTION, Risk::Low),
        120 => ("Caption / abstract", DESCRIPTION, Risk::Low),
        _ => return None,
    })
}

/// Parse the IIM datasets in a raw 0x0404 resource block → (dataset, value).
fn parse_iim(data: &[u8], out: &mut Vec<(u8, String)>) {
    let mut p = 0;
    while p + 5 <= data.len() {
        if data[p] != 0x1C || data[p + 1] != 0x02 {
            p += 1;
            continue;
        }
        let dataset = data[p + 2];
        let len = ((data[p + 3] as usize) << 8) | data[p + 4] as usize;
        // Extended-length datasets (top bit set) are rare in practice; stop.
        if len & 0x8000 != 0 {
            break;
        }
        let start = p + 5;
        let end = start + len;
        if end > data.len() {
            break;
        }
        out.push((dataset, String::from_utf8_lossy(&data[start..end]).into_owned()));
        p = end;
    }
}

/// Walk a JPEG's APP13 Photoshop IRB for the 0x0404 (IPTC-NAA) resource.
fn parse_app13(data: &[u8], out: &mut Vec<(u8, String)>) {
    const SIG: &[u8] = b"Photoshop 3.0\0";
    if data.len() < SIG.len() || &data[..SIG.len()] != SIG {
        return;
    }
    let mut pos = SIG.len();
    while pos + 12 <= data.len() {
        if &data[pos..pos + 4] != b"8BIM" {
            break;
        }
        let id = ((data[pos + 4] as u16) << 8) | data[pos + 5] as u16;
        pos += 6;
        // Pascal string name: length byte + name, padded so (1 + name) is even.
        let nlen = data[pos] as usize;
        let mut nfield = 1 + nlen;
        if nfield % 2 != 0 {
            nfield += 1;
        }
        pos += nfield;
        if pos + 4 > data.len() {
            break;
        }
        let size = u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
            as usize;
        pos += 4;
        let end = pos + size;
        if end > data.len() {
            break;
        }
        if id == 0x0404 {
            parse_iim(&data[pos..end], out);
        }
        pos = end + (size & 1); // block data is padded to an even length
    }
}

/// Walk the JPEG marker segments to find APP13, returning IIM (dataset, value)s.
fn scan_iptc(bytes: &[u8], findings: &mut Vec<(&'static str, String, &'static str, Risk, String)>) {
    if bytes.len() < 4 || bytes[0] != 0xFF || bytes[1] != 0xD8 {
        return; // IPTC-in-IRB is a JPEG concern
    }
    let mut raw: Vec<(u8, String)> = Vec::new();
    let mut i = 2;
    while i + 4 <= bytes.len() {
        if bytes[i] != 0xFF {
            i += 1;
            continue;
        }
        let marker = bytes[i + 1];
        if marker == 0xD9 || marker == 0xDA {
            break; // EOI or start-of-scan → no more metadata segments
        }
        if (0xD0..=0xD7).contains(&marker) || marker == 0x01 {
            i += 2; // standalone marker, no length
            continue;
        }
        let len = ((bytes[i + 2] as usize) << 8) | bytes[i + 3] as usize;
        if len < 2 {
            break;
        }
        let seg_start = i + 4;
        let seg_end = seg_start + len - 2;
        if seg_end > bytes.len() {
            break;
        }
        if marker == 0xED {
            parse_app13(&bytes[seg_start..seg_end], &mut raw);
        }
        i = seg_end;
    }

    let mut seen: Vec<&'static str> = Vec::new();
    for (dataset, val) in raw {
        if let Some((label, cat, risk)) = classify_iptc(dataset) {
            if seen.contains(&label) {
                continue; // collapse repeatable fields (e.g. Keywords) to one
            }
            seen.push(label);
            findings.push(("iptc", label.to_string(), cat, risk, clip(&val)));
        }
    }
}

/// Build the plain-English "what would leak" summary from the categories present.
fn build_summary(categories: &[&'static str], min_risk: &str, gps: &Option<Gps>) -> String {
    if categories.is_empty() {
        return format!(
            "No privacy-sensitive metadata at or above the '{min_risk}' risk level. \
             Nothing would leak if this image were shared."
        );
    }
    // Fixed priority order so the summary reads the same regardless of scan order.
    let order = [
        (LOCATION, "GPS location"),
        (DEVICE, "camera/device identifiers"),
        (PERSONAL, "creator/owner identity"),
        (TIMESTAMP, "capture timestamps"),
        (DESCRIPTION, "descriptions/keywords"),
        (SOFTWARE, "editing software"),
    ];
    let phrases: Vec<&str> = order
        .iter()
        .filter(|(c, _)| categories.contains(c))
        .map(|(_, p)| *p)
        .collect();
    let mut s = format!("Would leak if shared: {}.", phrases.join(", "));
    if let Some(g) = gps {
        s.push_str(&format!(
            " GPS pinpoints {:.5}, {:.5}.",
            g.latitude, g.longitude
        ));
    }
    s
}

/// Scan image bytes for privacy-sensitive metadata.
///
/// `min_risk` filters findings ("all" | "medium" | "high"); `reveal_values`
/// controls whether concrete field values (and decoded GPS) are included, so
/// the report can itself be shared when false.
pub fn scan(bytes: &[u8], min_risk: &str, reveal_values: bool) -> Result<Report, String> {
    if bytes.is_empty() {
        return Err("input is empty".into());
    }
    let threshold = Risk::threshold(min_risk)?;
    let format = detect_format(bytes);

    // (source, field, category, risk, value)
    let mut raw: Vec<(&'static str, String, &'static str, Risk, String)> = Vec::new();
    let gps = scan_exif(bytes, &mut raw);
    scan_xmp(bytes, &mut raw);
    scan_iptc(bytes, &mut raw);

    if format == "unknown" && raw.is_empty() {
        return Err("not a recognized image, or no readable metadata".into());
    }

    // Filter by risk threshold, then sort most-sensitive first (stable within).
    raw.retain(|(_, _, _, risk, _)| risk.rank() >= threshold);
    raw.sort_by(|a, b| b.3.rank().cmp(&a.3.rank()));

    let findings: Vec<Finding> = raw
        .iter()
        .map(|(source, field, category, risk, value)| Finding {
            source,
            field: field.clone(),
            category,
            risk: *risk,
            value: if reveal_values {
                Some(value.clone())
            } else {
                None
            },
        })
        .collect();

    let mut categories: Vec<&'static str> = Vec::new();
    for f in &findings {
        if !categories.contains(&f.category) {
            categories.push(f.category);
        }
    }

    let summary = build_summary(&categories, min_risk, &gps);
    let (gps_out, gps_map_url) = match (&gps, reveal_values) {
        (Some(g), true) => (
            Some(g.clone()),
            Some(format!(
                "https://www.openstreetmap.org/?mlat={:.6}&mlon={:.6}#map=16/{:.6}/{:.6}",
                g.latitude, g.longitude, g.latitude, g.longitude
            )),
        ),
        _ => (None, None),
    };

    Ok(Report {
        format,
        clean: findings.is_empty(),
        findings_count: findings.len(),
        findings,
        categories,
        gps: gps_out,
        gps_map_url,
        summary,
        values_hidden: !reveal_values,
    })
}

fn decode_image(input: &str) -> Result<Vec<u8>, String> {
    let mut s = input.trim();
    if s.is_empty() {
        return Err("image_base64 is empty".into());
    }
    if s.starts_with("data:") {
        if let Some((_, b64)) = s.split_once(',') {
            s = b64;
        }
    }
    let compact: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    STANDARD
        .decode(compact.as_bytes())
        .map_err(|e| format!("image_base64 must be base64 image bytes or a data URL: {e}"))
}

fn risk_label(risk: Risk) -> &'static str {
    match risk {
        Risk::Low => "low",
        Risk::Medium => "medium",
        Risk::High => "high",
    }
}

fn render_text(report: &Report) -> String {
    let mut out = String::new();
    out.push_str("metadata privacy report\n");
    out.push_str("=======================\n");
    out.push_str(&format!("Format: {}\n", report.format));
    out.push_str(&format!("Clean: {}\n", report.clean));
    out.push_str(&format!("Findings: {}\n", report.findings_count));
    out.push_str(&format!("Values hidden: {}\n", report.values_hidden));
    out.push_str(&format!("Summary: {}\n", report.summary));
    if let Some(g) = &report.gps {
        out.push_str(&format!("GPS: {:.6}, {:.6}\n", g.latitude, g.longitude));
    }
    if let Some(url) = &report.gps_map_url {
        out.push_str(&format!("Map: {url}\n"));
    }
    if !report.findings.is_empty() {
        out.push_str("\nFindings:\n");
        for f in &report.findings {
            out.push_str(&format!(
                "- [{}] {} {} ({})",
                risk_label(f.risk), f.source, f.field, f.category
            ));
            if let Some(v) = &f.value {
                out.push_str(&format!(": {v}"));
            }
            out.push('\n');
        }
    }
    out.trim_end().to_string()
}

/// Decode a base64/data-URL image, scan it, and render either a text report or JSON.
pub fn run(
    image_base64: &str,
    min_risk: &str,
    reveal_values: bool,
    output: &str,
) -> Result<String, String> {
    let mode = if output.trim().is_empty() {
        "report"
    } else {
        output.trim()
    };
    if !matches!(mode, "report" | "json") {
        return Err(format!("invalid output {mode:?}: expected report or json"));
    }
    let risk = if min_risk.trim().is_empty() {
        "all"
    } else {
        min_risk.trim()
    };
    let bytes = decode_image(image_base64)?;
    let report = scan(&bytes, risk, reveal_values)?;
    if mode == "json" {
        serde_json::to_string_pretty(&report).map_err(|e| format!("JSON encode error: {e}"))
    } else {
        Ok(render_text(&report))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    // --- EXIF fixtures ----------------------------------------------------

    /// Minimal little-endian TIFF/EXIF with IFD0 { Make="ACME", GPSInfo→GPS IFD }
    /// and a GPS IFD encoding lat 40°42'N, lon 74°0'E.
    fn tiff_make_and_gps() -> Vec<u8> {
        let mut t: Vec<u8> = Vec::new();
        // Header.
        t.extend_from_slice(b"II");
        t.extend_from_slice(&42u16.to_le_bytes());
        t.extend_from_slice(&8u32.to_le_bytes()); // IFD0 @ 8

        // IFD0 @ 8: 2 entries (2 + 24 + 4 = 30 bytes → ends @ 38).
        t.extend_from_slice(&2u16.to_le_bytes());
        // Make (0x010F) ASCII count 5 "ACME\0" → 5 bytes, stored @ offset 38.
        push_entry(&mut t, 0x010F, 2, 5, 38);
        // GPSInfo (0x8825) LONG count 1 → GPS IFD @ 44.
        push_entry(&mut t, 0x8825, 4, 1, 44);
        t.extend_from_slice(&0u32.to_le_bytes()); // next IFD

        // Make value @ 38 (5 bytes) then pad to 44.
        t.extend_from_slice(b"ACME\0"); // 38..43
        t.push(0); // pad 43 → 44

        // GPS IFD @ 44: 4 entries (2 + 48 + 4 = 54 → ends @ 98).
        t.extend_from_slice(&4u16.to_le_bytes());
        push_inline_ascii(&mut t, 0x0001, b"N\0"); // GPSLatitudeRef
        push_entry(&mut t, 0x0002, 5, 3, 98); // GPSLatitude rationals @ 98
        push_inline_ascii(&mut t, 0x0003, b"E\0"); // GPSLongitudeRef
        push_entry(&mut t, 0x0004, 5, 3, 122); // GPSLongitude rationals @ 122
        t.extend_from_slice(&0u32.to_le_bytes()); // next IFD

        // Rational data.
        push_rational(&mut t, 40, 1); // lat deg
        push_rational(&mut t, 42, 1); // lat min
        push_rational(&mut t, 0, 1); // lat sec  (98..122)
        push_rational(&mut t, 74, 1); // lon deg
        push_rational(&mut t, 0, 1); // lon min
        push_rational(&mut t, 0, 1); // lon sec  (122..146)
        t
    }

    fn push_entry(t: &mut Vec<u8>, tag: u16, typ: u16, count: u32, value_or_offset: u32) {
        t.extend_from_slice(&tag.to_le_bytes());
        t.extend_from_slice(&typ.to_le_bytes());
        t.extend_from_slice(&count.to_le_bytes());
        t.extend_from_slice(&value_or_offset.to_le_bytes());
    }

    fn push_inline_ascii(t: &mut Vec<u8>, tag: u16, val: &[u8]) {
        t.extend_from_slice(&tag.to_le_bytes());
        t.extend_from_slice(&2u16.to_le_bytes()); // ASCII
        t.extend_from_slice(&(val.len() as u32).to_le_bytes());
        let mut buf = [0u8; 4];
        buf[..val.len()].copy_from_slice(val);
        t.extend_from_slice(&buf);
    }

    fn push_rational(t: &mut Vec<u8>, num: u32, den: u32) {
        t.extend_from_slice(&num.to_le_bytes());
        t.extend_from_slice(&den.to_le_bytes());
    }

    fn jpeg_with_exif(tiff: &[u8]) -> Vec<u8> {
        let mut j: Vec<u8> = vec![0xFF, 0xD8]; // SOI
        let payload_len = 6 + tiff.len(); // "Exif\0\0" + tiff
        j.push(0xFF);
        j.push(0xE1); // APP1
        j.write_all(&((payload_len + 2) as u16).to_be_bytes()).unwrap();
        j.extend_from_slice(b"Exif\0\0");
        j.extend_from_slice(tiff);
        j.extend_from_slice(&[0xFF, 0xD9]); // EOI
        j
    }

    // --- IPTC fixture -----------------------------------------------------

    /// A JPEG carrying an APP13 IRB with an IPTC 0x0404 resource holding a
    /// By-line (2:80) = "Jane Doe" and City (2:90) = "Berlin".
    fn jpeg_with_iptc() -> Vec<u8> {
        fn iim(dataset: u8, val: &[u8]) -> Vec<u8> {
            let mut v = vec![0x1C, 0x02, dataset];
            v.extend_from_slice(&(val.len() as u16).to_be_bytes());
            v.extend_from_slice(val);
            v
        }
        let mut iptc = Vec::new();
        iptc.extend_from_slice(&iim(80, b"Jane Doe"));
        iptc.extend_from_slice(&iim(90, b"Berlin"));

        let mut irb = Vec::new();
        irb.extend_from_slice(b"Photoshop 3.0\0");
        irb.extend_from_slice(b"8BIM");
        irb.extend_from_slice(&0x0404u16.to_be_bytes());
        irb.push(0); // empty Pascal name → padded to even (1 byte)
        irb.push(0);
        irb.extend_from_slice(&(iptc.len() as u32).to_be_bytes());
        irb.extend_from_slice(&iptc);
        if iptc.len() % 2 != 0 {
            irb.push(0);
        }

        let mut j: Vec<u8> = vec![0xFF, 0xD8]; // SOI
        j.push(0xFF);
        j.push(0xED); // APP13
        j.write_all(&((irb.len() + 2) as u16).to_be_bytes()).unwrap();
        j.extend_from_slice(&irb);
        j.extend_from_slice(&[0xFF, 0xD9]);
        j
    }

    // --- tests ------------------------------------------------------------

    #[test]
    fn finds_gps_and_device_from_exif() {
        let jpeg = jpeg_with_exif(&tiff_make_and_gps());
        let r = scan(&jpeg, "all", true).unwrap();
        assert_eq!(r.format, "jpeg");
        assert!(!r.clean);
        let gps = r.findings.iter().find(|f| f.category == "location").unwrap();
        assert_eq!(gps.risk, Risk::High);
        assert!(r.findings.iter().any(|f| f.category == "device"));
        let g = r.gps.expect("decoded gps");
        assert!((g.latitude - 40.7).abs() < 1e-4, "lat {}", g.latitude);
        assert!((g.longitude - 74.0).abs() < 1e-4, "lon {}", g.longitude);
        assert!(r.gps_map_url.unwrap().contains("openstreetmap"));
        assert!(r.summary.contains("GPS location"));
    }

    #[test]
    fn min_risk_high_drops_low_findings() {
        let jpeg = jpeg_with_exif(&tiff_make_and_gps());
        let all = scan(&jpeg, "all", true).unwrap();
        let high = scan(&jpeg, "high", true).unwrap();
        assert!(all.findings_count > high.findings_count);
        assert!(high.findings.iter().all(|f| f.risk == Risk::High));
        // Camera model (low) is present at "all" but filtered at "high".
        assert!(all.findings.iter().any(|f| f.category == "device"));
        assert!(!high.findings.iter().any(|f| f.category == "device"));
    }

    #[test]
    fn reveal_values_false_redacts_values_and_gps() {
        let jpeg = jpeg_with_exif(&tiff_make_and_gps());
        let r = scan(&jpeg, "all", false).unwrap();
        assert!(r.values_hidden);
        assert!(r.findings.iter().all(|f| f.value.is_none()));
        assert!(r.gps.is_none());
        assert!(r.gps_map_url.is_none());
        // Findings are still reported so the leak is visible.
        assert!(!r.clean);
    }

    #[test]
    fn parses_iptc_byline_and_city() {
        let jpeg = jpeg_with_iptc();
        let r = scan(&jpeg, "all", true).unwrap();
        let byline = r
            .findings
            .iter()
            .find(|f| f.source == "iptc" && f.field.contains("By-line"))
            .expect("By-line finding");
        assert_eq!(byline.risk, Risk::High);
        assert_eq!(byline.value.as_deref(), Some("Jane Doe"));
        assert!(r
            .findings
            .iter()
            .any(|f| f.source == "iptc" && f.value.as_deref() == Some("Berlin")));
    }

    #[test]
    fn scans_xmp_creator_and_serial() {
        let xmp = "prefix<x:xmpmeta xmlns:x='adobe:ns:meta/'>\
            <rdf:RDF><rdf:Description aux:SerialNumber=\"SN12345\" xmp:CreatorTool=\"GIMP 2.10\">\
            <dc:creator><rdf:Seq><rdf:li>Ada Lovelace</rdf:li></rdf:Seq></dc:creator>\
            </rdf:Description></rdf:RDF></x:xmpmeta>suffix";
        let png = {
            let mut v = b"\x89PNG\r\n\x1a\n".to_vec();
            v.extend_from_slice(xmp.as_bytes());
            v
        };
        let r = scan(&png, "all", true).unwrap();
        assert_eq!(r.format, "png");
        let creator = r
            .findings
            .iter()
            .find(|f| f.field == "Creator")
            .expect("creator");
        assert_eq!(creator.value.as_deref(), Some("Ada Lovelace"));
        let serial = r
            .findings
            .iter()
            .find(|f| f.field == "Device serial number")
            .expect("serial");
        assert_eq!(serial.value.as_deref(), Some("SN12345"));
        assert_eq!(serial.risk, Risk::High);
    }

    #[test]
    fn clean_image_reports_no_leak() {
        // A PNG header with no metadata packet.
        let png = b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dIHDR".to_vec();
        let r = scan(&png, "all", true).unwrap();
        assert!(r.clean);
        assert_eq!(r.findings_count, 0);
        assert!(r.summary.contains("Nothing would leak"));
    }

    #[test]
    fn errors_on_empty_and_garbage() {
        assert!(scan(b"", "all", true).is_err());
        assert!(scan(b"not an image at all", "all", true).is_err());
    }

    #[test]
    fn errors_on_bad_min_risk() {
        let png = b"\x89PNG\r\n\x1a\n".to_vec();
        assert!(scan(&png, "sometimes", true).is_err());
    }
}
