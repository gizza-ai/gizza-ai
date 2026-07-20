//! gizza-ai/exif-edit core — write, edit, or selectively strip individual
//! EXIF fields (date taken, GPS position, artist, copyright, camera info, …)
//! on a JPEG or PNG photo **without re-encoding the pixels**. The compressed
//! image data is untouched byte-for-byte; only metadata segments/chunks change.
//!
//! Pure Rust (`img-parts` for the container splice, `kamadak-exif` for TIFF/EXIF
//! parse + rebuild) — no wasm/wafer deps, runs on every backend.
//!
//! Rewrite policy: when any EXIF field is set or removed, the EXIF segment is
//! REBUILT — every existing field is carried over except:
//!   - fields being replaced by an edit,
//!   - fields matched by a requested `remove` group,
//!   - the embedded thumbnail IFD (offset-bearing and a privacy leak — dropped
//!     and reported),
//!   - MakerNote (an opaque maker blob whose internal absolute offsets break on
//!     any rewrite — dropped and reported rather than silently corrupted),
//!   - offset/pointer bookkeeping tags (recomputed by the writer).
//! When only `xmp`/`iptc` removal is requested, the EXIF segment is left
//! byte-for-byte untouched (thumbnail and MakerNote survive).

use exif::experimental::Writer;
use exif::{Context, Field, In, Rational, Reader, Tag, Value};
use img_parts::jpeg::{markers, Jpeg, JpegSegment};
use img_parts::png::Png;
use img_parts::{Bytes, Error as ImgError, ImageEXIF};
use std::io::Cursor;

const JPEG_MAGIC: &[u8] = &[0xFF, 0xD8, 0xFF];
const PNG_MAGIC: &[u8] = &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
/// APP1 payload identifiers (JPEG). img-parts' `exif()` handles the EXIF one;
/// we match the XMP ones ourselves for `remove=xmp`.
const XMP_ID: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";
const XMP_EXT_ID: &[u8] = b"http://ns.adobe.com/xmp/extension/\0";
/// PNG iTXt keyword that carries the XMP packet.
const PNG_XMP_KEYWORD: &[u8] = b"XML:com.adobe.xmp\0";

/// The valid `remove` groups, in the order they are documented.
pub const REMOVE_GROUPS: [&str; 10] = [
    "gps",
    "date",
    "artist",
    "copyright",
    "description",
    "software",
    "camera",
    "serials",
    "xmp",
    "iptc",
];

/// Requested edits. Every field is optional; at least one set-field or one
/// remove group must be present (`validate` enforces this).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Edits {
    /// Sets DateTimeOriginal + DateTimeDigitized + DateTime (already normalized
    /// to EXIF "YYYY:MM:DD HH:MM:SS" by `parse_date`).
    pub date_taken: Option<String>,
    /// Decimal degrees, negative = south. Paired with `longitude`.
    pub latitude: Option<f64>,
    /// Decimal degrees, negative = west. Paired with `latitude`.
    pub longitude: Option<f64>,
    /// Meters; negative = below sea level.
    pub altitude: Option<f64>,
    pub artist: Option<String>,
    pub copyright: Option<String>,
    pub description: Option<String>,
    pub make: Option<String>,
    pub model: Option<String>,
    pub software: Option<String>,
    /// Validated `remove` group names (subset of `REMOVE_GROUPS`).
    pub remove: Vec<String>,
}

/// What happened, for the LLM / caller.
#[derive(Debug, Clone, PartialEq)]
pub struct EditReport {
    /// Detected container format ("jpeg" | "png").
    pub format: String,
    pub input_bytes: usize,
    pub output_bytes: usize,
    /// EXIF tag names written (set or replaced).
    pub fields_set: Vec<String>,
    /// EXIF tag names removed via `remove` groups.
    pub fields_removed: Vec<String>,
    /// Whole metadata segments/chunks removed (XMP APP1s / iTXt, IPTC APP13s).
    pub segments_removed: usize,
    /// True if the input carried an EXIF block before editing.
    pub had_exif: bool,
    /// True when a rebuild dropped the embedded thumbnail IFD.
    pub thumbnail_dropped: bool,
    /// True when a rebuild dropped an unrewritable MakerNote blob.
    pub makernote_dropped: bool,
}

/// Detected output format: ("jpeg"|"png", mime, extension).
pub fn detect_format(bytes: &[u8]) -> Option<(&'static str, &'static str, &'static str)> {
    if bytes.starts_with(JPEG_MAGIC) {
        Some(("jpeg", "image/jpeg", "jpg"))
    } else if bytes.starts_with(PNG_MAGIC) {
        Some(("png", "image/png", "png"))
    } else {
        None
    }
}

/// Parse a `remove` list ("gps, serials") into validated group names.
pub fn parse_remove(list: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for raw in list.split(',') {
        let g = raw.trim().to_ascii_lowercase();
        if g.is_empty() {
            continue;
        }
        if !REMOVE_GROUPS.contains(&g.as_str()) {
            return Err(format!(
                "unknown remove group '{g}': valid groups are {}",
                REMOVE_GROUPS.join(", ")
            ));
        }
        if !out.contains(&g) {
            out.push(g);
        }
    }
    Ok(out)
}

/// Parse a user-supplied date into EXIF's "YYYY:MM:DD HH:MM:SS".
/// Accepts `YYYY-MM-DD HH:MM:SS`, EXIF's `YYYY:MM:DD HH:MM:SS`,
/// ISO `YYYY-MM-DDTHH:MM:SS`, or a bare date (midnight).
pub fn parse_date(input: &str) -> Result<String, String> {
    let s = input.trim();
    let err = || {
        format!(
            "invalid date_taken '{s}': expected YYYY-MM-DD HH:MM:SS \
             (also accepted: YYYY:MM:DD HH:MM:SS, YYYY-MM-DDTHH:MM:SS, or a bare YYYY-MM-DD)"
        )
    };
    let (date_part, time_part) = match s.split_once([' ', 'T']) {
        Some((d, t)) => (d, Some(t)),
        None => (s, None),
    };
    let d: Vec<&str> = date_part.split(['-', ':']).collect();
    if d.len() != 3 {
        return Err(err());
    }
    let year: u16 = d[0].parse().map_err(|_| err())?;
    let month: u8 = d[1].parse().map_err(|_| err())?;
    let day: u8 = d[2].parse().map_err(|_| err())?;
    if !(1..=9999).contains(&year) || !(1..=12).contains(&month) {
        return Err(err());
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days_in_month = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ => {
            if leap {
                29
            } else {
                28
            }
        }
    };
    if !(1..=days_in_month).contains(&day) {
        return Err(err());
    }
    let (hour, minute, second): (u8, u8, u8) = match time_part {
        None => (0, 0, 0),
        Some(t) => {
            let p: Vec<&str> = t.split(':').collect();
            if p.len() != 3 {
                return Err(err());
            }
            (
                p[0].parse().map_err(|_| err())?,
                p[1].parse().map_err(|_| err())?,
                p[2].parse().map_err(|_| err())?,
            )
        }
    };
    if hour > 23 || minute > 59 || second > 59 {
        return Err(err());
    }
    Ok(format!(
        "{year:04}:{month:02}:{day:02} {hour:02}:{minute:02}:{second:02}"
    ))
}

/// Validate cross-field rules: at least one edit, lat/lon pairing, ranges, and
/// set-vs-remove conflicts.
pub fn validate(edits: &Edits) -> Result<(), String> {
    let any_set = edits.date_taken.is_some()
        || edits.latitude.is_some()
        || edits.longitude.is_some()
        || edits.altitude.is_some()
        || edits.artist.is_some()
        || edits.copyright.is_some()
        || edits.description.is_some()
        || edits.make.is_some()
        || edits.model.is_some()
        || edits.software.is_some();
    if !any_set && edits.remove.is_empty() {
        return Err(format!(
            "nothing to do: set at least one field (date_taken, latitude+longitude, altitude, \
             artist, copyright, description, make, model, software) or pass remove= with one of: {}",
            REMOVE_GROUPS.join(", ")
        ));
    }
    if edits.latitude.is_some() != edits.longitude.is_some() {
        return Err("latitude and longitude must be provided together".into());
    }
    if let Some(lat) = edits.latitude {
        if !lat.is_finite() || !(-90.0..=90.0).contains(&lat) {
            return Err(format!("latitude {lat} out of range: must be -90..90"));
        }
    }
    if let Some(lon) = edits.longitude {
        if !lon.is_finite() || !(-180.0..=180.0).contains(&lon) {
            return Err(format!("longitude {lon} out of range: must be -180..180"));
        }
    }
    if let Some(alt) = edits.altitude {
        if !alt.is_finite() || !(-11000.0..=20000.0).contains(&alt) {
            return Err(format!(
                "altitude {alt} out of range: must be -11000..20000 meters"
            ));
        }
    }
    let conflicts: [(&str, bool); 7] = [
        ("date", edits.date_taken.is_some()),
        (
            "gps",
            edits.latitude.is_some() || edits.longitude.is_some() || edits.altitude.is_some(),
        ),
        ("artist", edits.artist.is_some()),
        ("copyright", edits.copyright.is_some()),
        ("description", edits.description.is_some()),
        ("software", edits.software.is_some()),
        ("camera", edits.make.is_some() || edits.model.is_some()),
    ];
    for (group, set) in conflicts {
        if set && edits.remove.iter().any(|g| g == group) {
            return Err(format!(
                "conflicting request: remove={group} while also setting a {group} field — drop one of the two"
            ));
        }
    }
    Ok(())
}

/// |decimal degrees| → (deg, min, sec×10000) with carry handled by integer math.
/// 1e-4 arc-second ≈ 3 mm — well past any camera's GPS precision.
fn to_dms_e4(abs: f64) -> (u32, u32, u32) {
    let total = (abs * 3600.0 * 10000.0).round() as u64;
    let deg = (total / 36_000_000) as u32;
    let rem = total % 36_000_000;
    let min = (rem / 600_000) as u32;
    let sec_e4 = (rem % 600_000) as u32;
    (deg, min, sec_e4)
}

fn dms_value(coord: f64) -> Value {
    let (d, m, s_e4) = to_dms_e4(coord.abs());
    Value::Rational(vec![
        Rational { num: d, denom: 1 },
        Rational { num: m, denom: 1 },
        Rational {
            num: s_e4,
            denom: 10000,
        },
    ])
}

fn ascii(s: &str) -> Value {
    Value::Ascii(vec![s.as_bytes().to_vec()])
}

fn field(tag: Tag, value: Value) -> Field {
    Field {
        tag,
        ifd_num: In::PRIMARY,
        value,
    }
}

/// Does `tag` belong to a `remove` group?
fn in_remove_group(tag: Tag, group: &str) -> bool {
    match group {
        "gps" => tag.context() == Context::Gps,
        "date" => matches!(
            tag,
            Tag::DateTime
                | Tag::DateTimeOriginal
                | Tag::DateTimeDigitized
                | Tag::SubSecTime
                | Tag::SubSecTimeOriginal
                | Tag::SubSecTimeDigitized
                | Tag::OffsetTime
                | Tag::OffsetTimeOriginal
                | Tag::OffsetTimeDigitized
        ),
        // XPAuthor (0x9c9d) is the Windows-Explorer author tag.
        "artist" => tag == Tag::Artist || tag == Tag(Context::Tiff, 0x9c9d),
        "copyright" => tag == Tag::Copyright,
        "description" => {
            matches!(tag, Tag::ImageDescription | Tag::UserComment)
                // Windows XPTitle / XPComment / XPKeywords / XPSubject
                || matches!(tag, Tag(Context::Tiff, n) if (0x9c9b..=0x9c9f).contains(&n) && n != 0x9c9d)
        }
        "software" => tag == Tag::Software,
        "camera" => matches!(tag, Tag::Make | Tag::Model | Tag::LensMake | Tag::LensModel),
        "serials" => matches!(
            tag,
            Tag::BodySerialNumber
                | Tag::LensSerialNumber
                | Tag::CameraOwnerName
                | Tag::ImageUniqueID
        ),
        _ => false, // xmp/iptc are container-level, not EXIF fields
    }
}

/// Offset/pointer bookkeeping tags that must never be copied into a rebuilt
/// EXIF verbatim (the writer recomputes structure; stale offsets would lie).
fn is_offset_tag(tag: Tag) -> bool {
    matches!(
        tag,
        Tag::StripOffsets
            | Tag::StripByteCounts
            | Tag::TileOffsets
            | Tag::TileByteCounts
            | Tag::JPEGInterchangeFormat
            | Tag::JPEGInterchangeFormatLength
    )
}

/// The tags an edit replaces (so the carried-over copy is skipped).
fn replaced_tags(edits: &Edits) -> Vec<Tag> {
    let mut t = Vec::new();
    if edits.date_taken.is_some() {
        t.extend([Tag::DateTime, Tag::DateTimeOriginal, Tag::DateTimeDigitized]);
    }
    if edits.latitude.is_some() {
        // (paired with longitude by validate)
        t.extend([
            Tag::GPSLatitude,
            Tag::GPSLatitudeRef,
            Tag::GPSLongitude,
            Tag::GPSLongitudeRef,
        ]);
    }
    if edits.altitude.is_some() {
        t.extend([Tag::GPSAltitude, Tag::GPSAltitudeRef]);
    }
    if edits.artist.is_some() {
        t.push(Tag::Artist);
    }
    if edits.copyright.is_some() {
        t.push(Tag::Copyright);
    }
    if edits.description.is_some() {
        t.push(Tag::ImageDescription);
    }
    if edits.make.is_some() {
        t.push(Tag::Make);
    }
    if edits.model.is_some() {
        t.push(Tag::Model);
    }
    if edits.software.is_some() {
        t.push(Tag::Software);
    }
    t
}

/// New fields for the set-edits, in stable order. Returns (fields, names).
fn new_fields(edits: &Edits, have_gps_version: bool) -> (Vec<Field>, Vec<String>) {
    let mut fields = Vec::new();
    let mut names = Vec::new();
    if let Some(dt) = &edits.date_taken {
        for tag in [Tag::DateTime, Tag::DateTimeOriginal, Tag::DateTimeDigitized] {
            fields.push(field(tag, ascii(dt)));
            names.push(tag.to_string());
        }
    }
    let needs_gps_version =
        (edits.latitude.is_some() || edits.altitude.is_some()) && !have_gps_version;
    if needs_gps_version {
        fields.push(field(Tag::GPSVersionID, Value::Byte(vec![2, 3, 0, 0])));
        names.push(Tag::GPSVersionID.to_string());
    }
    if let (Some(lat), Some(lon)) = (edits.latitude, edits.longitude) {
        fields.push(field(
            Tag::GPSLatitudeRef,
            ascii(if lat < 0.0 { "S" } else { "N" }),
        ));
        fields.push(field(Tag::GPSLatitude, dms_value(lat)));
        fields.push(field(
            Tag::GPSLongitudeRef,
            ascii(if lon < 0.0 { "W" } else { "E" }),
        ));
        fields.push(field(Tag::GPSLongitude, dms_value(lon)));
        names.extend([
            Tag::GPSLatitudeRef.to_string(),
            Tag::GPSLatitude.to_string(),
            Tag::GPSLongitudeRef.to_string(),
            Tag::GPSLongitude.to_string(),
        ]);
    }
    if let Some(alt) = edits.altitude {
        fields.push(field(
            Tag::GPSAltitudeRef,
            Value::Byte(vec![u8::from(alt < 0.0)]),
        ));
        fields.push(field(
            Tag::GPSAltitude,
            Value::Rational(vec![Rational {
                num: (alt.abs() * 100.0).round() as u32,
                denom: 100,
            }]),
        ));
        names.extend([Tag::GPSAltitudeRef.to_string(), Tag::GPSAltitude.to_string()]);
    }
    for (tag, value) in [
        (Tag::Artist, &edits.artist),
        (Tag::Copyright, &edits.copyright),
        (Tag::ImageDescription, &edits.description),
        (Tag::Make, &edits.make),
        (Tag::Model, &edits.model),
        (Tag::Software, &edits.software),
    ] {
        if let Some(v) = value {
            fields.push(field(tag, ascii(v)));
            names.push(tag.to_string());
        }
    }
    (fields, names)
}

/// Does this edit touch the EXIF block itself (vs only container-level XMP/IPTC)?
fn touches_exif(edits: &Edits) -> bool {
    !replaced_tags(edits).is_empty() || edits.remove.iter().any(|g| g != "xmp" && g != "iptc")
}

/// Apply the edits. Returns (output bytes, report).
pub fn edit(input: &[u8], edits: &Edits) -> Result<(Vec<u8>, EditReport), String> {
    validate(edits)?;
    let (format, _mime, _ext) = detect_format(input).ok_or_else(|| {
        "unsupported image format: only JPEG and PNG are supported (TIFF/WebP/HEIC are not)"
            .to_string()
    })?;
    let bytes = Bytes::copy_from_slice(input);
    match format {
        "jpeg" => edit_jpeg(bytes, input.len(), edits),
        "png" => edit_png(bytes, input.len(), edits),
        _ => unreachable!("detect_format only returns jpeg|png"),
    }
}

fn map_eof(e: ImgError) -> String {
    format!("malformed image: could not parse the image container ({e})")
}

/// The APP1 payload prefix that marks a JPEG EXIF segment.
const EXIF_DATA_PREFIX: &[u8] = b"Exif\0\0";

/// Replace (or drop) the JPEG EXIF APP1 segment. img-parts 0.3's own
/// `Jpeg::set_exif` hardcodes `segments.insert(3, …)` and PANICS on images
/// with fewer than 3 segments, so we splice the segment ourselves: right
/// after a leading APP0 (JFIF) if present, else first.
fn jpeg_set_exif(jpeg: &mut Jpeg, exif: Option<Vec<u8>>) {
    jpeg.segments_mut().retain(|seg| {
        !(seg.marker() == markers::APP1 && seg.contents().starts_with(EXIF_DATA_PREFIX))
    });
    if let Some(payload) = exif {
        let mut contents = Vec::with_capacity(EXIF_DATA_PREFIX.len() + payload.len());
        contents.extend_from_slice(EXIF_DATA_PREFIX);
        contents.extend_from_slice(&payload);
        let segment = JpegSegment::new_with_contents(markers::APP1, Bytes::from(contents));
        let pos = usize::from(
            jpeg.segments()
                .first()
                .is_some_and(|s| s.marker() == markers::APP0),
        );
        let pos = pos.min(jpeg.segments().len());
        jpeg.segments_mut().insert(pos, segment);
    }
}

/// Outcome of an EXIF rebuild: the new payload (None = drop the block) plus
/// what was set/removed/dropped along the way.
struct RebuiltExif {
    payload: Option<Vec<u8>>,
    fields_set: Vec<String>,
    fields_removed: Vec<String>,
    thumbnail_dropped: bool,
    makernote_dropped: bool,
}

/// Rebuild the EXIF payload (raw TIFF bytes, no "Exif\0\0" prefix) with the
/// edits applied. `existing` is the current payload if any.
fn rebuild_exif(existing: Option<&[u8]>, edits: &Edits) -> Result<RebuiltExif, String> {
    let mut fields: Vec<Field> = Vec::new();
    let mut fields_removed: Vec<String> = Vec::new();
    let mut thumbnail_dropped = false;
    let mut makernote_dropped = false;
    let mut have_gps_version = false;
    let replaced = replaced_tags(edits);

    if let Some(raw) = existing {
        let exif = Reader::new().read_raw(raw.to_vec()).map_err(|e| {
            format!(
                "the image's existing EXIF block could not be parsed ({e}); \
                 to discard it entirely use the strip-exif tool instead"
            )
        })?;
        let mut seen: Vec<(Tag, In)> = Vec::new();
        for f in exif.fields() {
            if f.ifd_num != In::PRIMARY {
                thumbnail_dropped = true;
                continue;
            }
            if f.tag == Tag::MakerNote {
                makernote_dropped = true;
                continue;
            }
            if is_offset_tag(f.tag) {
                continue;
            }
            if edits.remove.iter().any(|g| in_remove_group(f.tag, g)) {
                fields_removed.push(f.tag.to_string());
                continue;
            }
            if replaced.contains(&f.tag) {
                continue;
            }
            if seen.contains(&(f.tag, f.ifd_num)) {
                continue; // corrupt duplicate — keep the first
            }
            seen.push((f.tag, f.ifd_num));
            if f.tag == Tag::GPSVersionID {
                have_gps_version = true;
            }
            fields.push(Field {
                tag: f.tag,
                ifd_num: f.ifd_num,
                value: f.value.clone(),
            });
        }
    }

    let (added, fields_set) = new_fields(edits, have_gps_version);
    fields.extend(added);

    let payload = if fields.is_empty() {
        // Everything removed and nothing set → drop the EXIF block entirely.
        None
    } else {
        let mut writer = Writer::new();
        for f in &fields {
            writer.push_field(f);
        }
        let mut cursor = Cursor::new(Vec::new());
        writer
            .write(&mut cursor, true)
            .map_err(|e| format!("failed to encode the edited EXIF block ({e})"))?;
        Some(cursor.into_inner())
    };
    Ok(RebuiltExif {
        payload,
        fields_set,
        fields_removed,
        thumbnail_dropped,
        makernote_dropped,
    })
}

fn edit_jpeg(
    bytes: Bytes,
    input_len: usize,
    edits: &Edits,
) -> Result<(Vec<u8>, EditReport), String> {
    let mut jpeg = Jpeg::from_bytes(bytes).map_err(map_eof)?;
    let existing = jpeg.exif();
    let had_exif = existing.is_some();

    let mut rebuilt = RebuiltExif {
        payload: None,
        fields_set: Vec::new(),
        fields_removed: Vec::new(),
        thumbnail_dropped: false,
        makernote_dropped: false,
    };
    if touches_exif(edits) {
        rebuilt = rebuild_exif(existing.as_deref(), edits)?;
        jpeg_set_exif(&mut jpeg, rebuilt.payload.take());
    }

    let mut segments_removed = 0usize;
    if edits.remove.iter().any(|g| g == "xmp") {
        let before = jpeg.segments().len();
        jpeg.segments_mut().retain(|seg| {
            !(seg.marker() == markers::APP1
                && (seg.contents().starts_with(XMP_ID) || seg.contents().starts_with(XMP_EXT_ID)))
        });
        segments_removed += before - jpeg.segments().len();
    }
    if edits.remove.iter().any(|g| g == "iptc") {
        let before = jpeg.segments().len();
        jpeg.segments_mut()
            .retain(|seg| seg.marker() != markers::APP13);
        segments_removed += before - jpeg.segments().len();
    }

    let out = jpeg.encoder().bytes().to_vec();
    let report = EditReport {
        format: "jpeg".into(),
        input_bytes: input_len,
        output_bytes: out.len(),
        fields_set: rebuilt.fields_set,
        fields_removed: rebuilt.fields_removed,
        segments_removed,
        had_exif,
        thumbnail_dropped: rebuilt.thumbnail_dropped,
        makernote_dropped: rebuilt.makernote_dropped,
    };
    Ok((out, report))
}

fn edit_png(
    bytes: Bytes,
    input_len: usize,
    edits: &Edits,
) -> Result<(Vec<u8>, EditReport), String> {
    let mut png = Png::from_bytes(bytes).map_err(map_eof)?;
    let existing = png.exif();
    let had_exif = existing.is_some();

    let mut rebuilt = RebuiltExif {
        payload: None,
        fields_set: Vec::new(),
        fields_removed: Vec::new(),
        thumbnail_dropped: false,
        makernote_dropped: false,
    };
    if touches_exif(edits) {
        rebuilt = rebuild_exif(existing.as_deref(), edits)?;
        png.set_exif(rebuilt.payload.take().map(Bytes::from));
    }

    let mut segments_removed = 0usize;
    if edits.remove.iter().any(|g| g == "xmp") {
        let before = png.chunks().len();
        png.chunks_mut()
            .retain(|c| !(&c.kind() == b"iTXt" && c.contents().starts_with(PNG_XMP_KEYWORD)));
        segments_removed += before - png.chunks().len();
    }
    // `remove=iptc`: IPTC has no standard PNG carrier — nothing to do (0 segments).

    let out = png.encoder().bytes().to_vec();
    let report = EditReport {
        format: "png".into(),
        input_bytes: input_len,
        output_bytes: out.len(),
        fields_set: rebuilt.fields_set,
        fields_removed: rebuilt.fields_removed,
        segments_removed,
        had_exif,
        thumbnail_dropped: rebuilt.thumbnail_dropped,
        makernote_dropped: rebuilt.makernote_dropped,
    };
    Ok((out, report))
}

#[cfg(test)]
mod tests {
    use super::*;

    // SOI + APP0 (JFIF) + SOS + entropy data + EOI — enough framing for
    // img-parts to round-trip; metadata logic never decodes pixels.
    fn minimal_jpeg() -> Vec<u8> {
        vec![
            0xFF, 0xD8, // SOI
            0xFF, 0xE0, 0x00, 0x10, // APP0, len 16
            b'J', b'F', b'I', b'F', 0x00, 0x01, 0x01, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00,
            0xFF, 0xDA, 0x00, 0x08, 0x01, 0x01, 0x00, 0x00, 0x3F, 0x00, // SOS
            0x12, 0x34, // entropy-coded scan data
            0xFF, 0xD9, // EOI
        ]
    }

    fn minimal_png() -> Vec<u8> {
        vec![
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // signature
            0x00, 0x00, 0x00, 0x0D, // IHDR len
            b'I', b'H', b'D', b'R', 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06,
            0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4, 0x89, // IHDR data + crc
            0x00, 0x00, 0x00, 0x0A, // IDAT len
            b'I', b'D', b'A', b'T', 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01,
            0x0D, 0x0A, 0x2D, 0xB4, // IDAT data + crc
            0x00, 0x00, 0x00, 0x00, // IEND len
            b'I', b'E', b'N', b'D', 0xAE, 0x42, 0x60, 0x82, // IEND + crc
        ]
    }

    /// JPEG carrying EXIF (Artist "Ada", Model "CamX", GPS 10°N 20°E, a
    /// thumbnail-IFD field, and a MakerNote) built with the same writer the
    /// production path uses.
    fn jpeg_with_exif() -> Vec<u8> {
        let fields = vec![
            Field {
                tag: Tag::Artist,
                ifd_num: In::PRIMARY,
                value: ascii("Ada"),
            },
            Field {
                tag: Tag::Model,
                ifd_num: In::PRIMARY,
                value: ascii("CamX"),
            },
            Field {
                tag: Tag::GPSVersionID,
                ifd_num: In::PRIMARY,
                value: Value::Byte(vec![2, 3, 0, 0]),
            },
            Field {
                tag: Tag::GPSLatitudeRef,
                ifd_num: In::PRIMARY,
                value: ascii("N"),
            },
            Field {
                tag: Tag::GPSLatitude,
                ifd_num: In::PRIMARY,
                value: dms_value(10.0),
            },
            Field {
                tag: Tag::GPSLongitudeRef,
                ifd_num: In::PRIMARY,
                value: ascii("E"),
            },
            Field {
                tag: Tag::GPSLongitude,
                ifd_num: In::PRIMARY,
                value: dms_value(20.0),
            },
            Field {
                tag: Tag::MakerNote,
                ifd_num: In::PRIMARY,
                value: Value::Undefined(b"maker-secret".to_vec(), 0),
            },
            Field {
                tag: Tag::ImageDescription,
                ifd_num: In::THUMBNAIL,
                value: ascii("thumb"),
            },
        ];
        let mut writer = Writer::new();
        for f in &fields {
            writer.push_field(f);
        }
        let mut cursor = Cursor::new(Vec::new());
        writer.write(&mut cursor, true).unwrap();
        let mut jpeg = Jpeg::from_bytes(Bytes::from(minimal_jpeg())).unwrap();
        jpeg_set_exif(&mut jpeg, Some(cursor.into_inner()));
        jpeg.encoder().bytes().to_vec()
    }

    fn read_exif(bytes: &[u8]) -> exif::Exif {
        Reader::new()
            .read_from_container(&mut Cursor::new(bytes))
            .unwrap()
    }

    fn ascii_of(exif: &exif::Exif, tag: Tag) -> String {
        let f = exif.get_field(tag, In::PRIMARY).unwrap();
        match &f.value {
            Value::Ascii(v) => String::from_utf8(v[0].clone()).unwrap(),
            other => panic!("expected Ascii for {tag}, got {other:?}"),
        }
    }

    #[test]
    fn sets_date_on_jpeg_without_exif() {
        let img = minimal_jpeg();
        let edits = Edits {
            date_taken: Some(parse_date("2024-06-01 14:30:05").unwrap()),
            ..Default::default()
        };
        let (out, report) = edit(&img, &edits).unwrap();
        assert_eq!(report.format, "jpeg");
        assert!(!report.had_exif);
        assert_eq!(report.fields_set.len(), 3, "DateTime + Original + Digitized");
        let exif = read_exif(&out);
        for tag in [Tag::DateTime, Tag::DateTimeOriginal, Tag::DateTimeDigitized] {
            assert_eq!(ascii_of(&exif, tag), "2024:06:01 14:30:05");
        }
        // pixels untouched: the scan bytes + EOI survive verbatim
        let tail: &[u8] = &[
            0xFF, 0xDA, 0x00, 0x08, 0x01, 0x01, 0x00, 0x00, 0x3F, 0x00, 0x12, 0x34, 0xFF, 0xD9,
        ];
        assert!(
            out.windows(tail.len()).any(|w| w == tail),
            "scan data must be byte-identical"
        );
    }

    #[test]
    fn sets_gps_with_southern_western_refs() {
        let img = minimal_jpeg();
        let edits = Edits {
            latitude: Some(-33.8688),
            longitude: Some(-70.6693),
            altitude: Some(-12.5),
            ..Default::default()
        };
        let (out, _report) = edit(&img, &edits).unwrap();
        let exif = read_exif(&out);
        assert_eq!(ascii_of(&exif, Tag::GPSLatitudeRef), "S");
        assert_eq!(ascii_of(&exif, Tag::GPSLongitudeRef), "W");
        let lat = exif.get_field(Tag::GPSLatitude, In::PRIMARY).unwrap();
        match &lat.value {
            Value::Rational(r) => {
                assert_eq!((r[0].num, r[0].denom), (33, 1));
                assert_eq!((r[1].num, r[1].denom), (52, 1));
                // 0.8688° = 3127.68 s → 52 min + 7.68 s
                assert_eq!((r[2].num, r[2].denom), (76800, 10000));
            }
            other => panic!("expected Rational, got {other:?}"),
        }
        let alt_ref = exif.get_field(Tag::GPSAltitudeRef, In::PRIMARY).unwrap();
        assert!(
            matches!(&alt_ref.value, Value::Byte(b) if b == &vec![1u8]),
            "below sea level"
        );
        let alt = exif.get_field(Tag::GPSAltitude, In::PRIMARY).unwrap();
        assert!(
            matches!(&alt.value, Value::Rational(r) if r[0].num == 1250 && r[0].denom == 100)
        );
        assert!(exif.get_field(Tag::GPSVersionID, In::PRIMARY).is_some());
    }

    #[test]
    fn preserves_unrelated_fields_and_replaces_edited_ones() {
        let img = jpeg_with_exif();
        let edits = Edits {
            copyright: Some("(c) 2026 Grace".into()),
            model: Some("CamY".into()),
            ..Default::default()
        };
        let (out, report) = edit(&img, &edits).unwrap();
        assert!(report.had_exif);
        assert!(report.thumbnail_dropped, "thumbnail IFD is dropped on rebuild");
        assert!(report.makernote_dropped, "MakerNote is dropped on rebuild");
        let exif = read_exif(&out);
        assert_eq!(ascii_of(&exif, Tag::Artist), "Ada", "unrelated field preserved");
        assert_eq!(ascii_of(&exif, Tag::Model), "CamY", "edited field replaced");
        assert_eq!(ascii_of(&exif, Tag::Copyright), "(c) 2026 Grace");
        assert_eq!(ascii_of(&exif, Tag::GPSLatitudeRef), "N", "GPS preserved");
        assert!(
            exif.fields().all(|f| f.ifd_num == In::PRIMARY),
            "no thumbnail IFD in output"
        );
        assert!(exif.get_field(Tag::MakerNote, In::PRIMARY).is_none());
    }

    #[test]
    fn remove_gps_keeps_everything_else() {
        let img = jpeg_with_exif();
        let edits = Edits {
            remove: vec!["gps".into()],
            ..Default::default()
        };
        let (out, report) = edit(&img, &edits).unwrap();
        assert!(report
            .fields_removed
            .iter()
            .any(|n| n.contains("GPSLatitude")));
        let exif = read_exif(&out);
        assert!(exif.get_field(Tag::GPSLatitude, In::PRIMARY).is_none());
        assert!(exif.get_field(Tag::GPSLongitude, In::PRIMARY).is_none());
        assert_eq!(ascii_of(&exif, Tag::Artist), "Ada");
        assert_eq!(ascii_of(&exif, Tag::Model), "CamX");
    }

    #[test]
    fn remove_everything_drops_the_exif_block() {
        let img = jpeg_with_exif();
        let edits = Edits {
            remove: vec!["gps".into(), "artist".into(), "camera".into()],
            ..Default::default()
        };
        let (out, _report) = edit(&img, &edits).unwrap();
        let parsed = Jpeg::from_bytes(Bytes::from(out)).unwrap();
        assert!(
            parsed.exif().is_none(),
            "empty EXIF must be dropped, not written"
        );
    }

    #[test]
    fn remove_xmp_drops_only_the_xmp_segment_and_keeps_exif_bytes() {
        let mut jpeg = Jpeg::from_bytes(Bytes::from(jpeg_with_exif())).unwrap();
        let xmp = JpegSegment::new_with_contents(
            markers::APP1,
            Bytes::from([XMP_ID, b"<x:xmpmeta>gps-here</x:xmpmeta>" as &[u8]].concat()),
        );
        // insert BEFORE the SOS scan segment — segments after SOS are not
        // re-parseable (they'd sit inside the entropy-coded data).
        jpeg.segments_mut().insert(2, xmp);
        let img = jpeg.encoder().bytes().to_vec();
        let exif_before = Jpeg::from_bytes(Bytes::from(img.clone()))
            .unwrap()
            .exif()
            .unwrap();

        let edits = Edits {
            remove: vec!["xmp".into()],
            ..Default::default()
        };
        let (out, report) = edit(&img, &edits).unwrap();
        assert_eq!(report.segments_removed, 1);
        assert!(!report.thumbnail_dropped, "EXIF untouched for xmp-only removal");
        let parsed = Jpeg::from_bytes(Bytes::from(out.clone())).unwrap();
        assert_eq!(
            parsed.exif().unwrap(),
            exif_before,
            "EXIF payload byte-identical when only XMP is removed"
        );
        assert!(!out.windows(4).any(|w| w == b"gps-"), "XMP content gone");
    }

    #[test]
    fn remove_iptc_drops_app13() {
        let mut jpeg = Jpeg::from_bytes(Bytes::from(minimal_jpeg())).unwrap();
        let iptc = JpegSegment::new_with_contents(
            markers::APP13,
            Bytes::from_static(b"Photoshop 3.0\08BIM\x04\x04iptc-here"),
        );
        jpeg.segments_mut().insert(1, iptc);
        let img = jpeg.encoder().bytes().to_vec();
        let edits = Edits {
            remove: vec!["iptc".into()],
            ..Default::default()
        };
        let (out, report) = edit(&img, &edits).unwrap();
        assert_eq!(report.segments_removed, 1);
        assert!(!out.windows(5).any(|w| w == b"8BIM\x04"), "IPTC gone");
    }

    #[test]
    fn sets_date_on_png_via_exif_chunk() {
        let img = minimal_png();
        let edits = Edits {
            date_taken: Some(parse_date("2020-02-29").unwrap()), // leap day, bare date
            artist: Some("Ada".into()),
            ..Default::default()
        };
        let (out, report) = edit(&img, &edits).unwrap();
        assert_eq!(report.format, "png");
        let exif = read_exif(&out);
        assert_eq!(ascii_of(&exif, Tag::DateTimeOriginal), "2020:02:29 00:00:00");
        assert_eq!(ascii_of(&exif, Tag::Artist), "Ada");
        // IDAT untouched
        let idat: &[u8] = &[
            b'I', b'D', b'A', b'T', 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01,
        ];
        assert!(out.windows(idat.len()).any(|w| w == idat));
    }

    #[test]
    fn parse_date_accepts_documented_forms() {
        assert_eq!(parse_date("2024-06-01 14:30:05").unwrap(), "2024:06:01 14:30:05");
        assert_eq!(parse_date("2024:06:01 14:30:05").unwrap(), "2024:06:01 14:30:05");
        assert_eq!(parse_date("2024-06-01T14:30:05").unwrap(), "2024:06:01 14:30:05");
        assert_eq!(parse_date("2024-06-01").unwrap(), "2024:06:01 00:00:00");
    }

    #[test]
    fn parse_date_rejects_invalid() {
        for bad in [
            "yesterday",
            "2024-13-01",
            "2024-02-30",
            "2023-02-29", // not a leap year
            "2024-06-01 25:00:00",
            "2024-06",
            "",
        ] {
            assert!(parse_date(bad).is_err(), "should reject {bad:?}");
        }
    }

    #[test]
    fn validate_rejects_lat_without_lon_and_noop_and_conflicts() {
        let e = Edits {
            latitude: Some(1.0),
            ..Default::default()
        };
        assert!(validate(&e).unwrap_err().contains("together"));
        assert!(validate(&Edits::default())
            .unwrap_err()
            .contains("nothing to do"));
        let e = Edits {
            artist: Some("Ada".into()),
            remove: vec!["artist".into()],
            ..Default::default()
        };
        assert!(validate(&e).unwrap_err().contains("conflicting"));
        let e = Edits {
            altitude: Some(1.0),
            remove: vec!["gps".into()],
            ..Default::default()
        };
        assert!(validate(&e).unwrap_err().contains("conflicting"));
    }

    #[test]
    fn parse_remove_validates_vocabulary() {
        assert_eq!(parse_remove("gps, serials").unwrap(), vec!["gps", "serials"]);
        assert_eq!(parse_remove("GPS,gps").unwrap(), vec!["gps"], "dedupe + case-fold");
        assert!(parse_remove("gps,thumbnail")
            .unwrap_err()
            .contains("unknown remove group"));
    }

    #[test]
    fn rejects_unsupported_format() {
        let gif = b"GIF89a\x01\x00\x01\x00";
        let edits = Edits {
            artist: Some("Ada".into()),
            ..Default::default()
        };
        let err = edit(gif, &edits).unwrap_err();
        assert!(err.contains("only JPEG and PNG"), "got: {err}");
    }

    #[test]
    fn rejects_malformed_existing_exif() {
        let mut jpeg = Jpeg::from_bytes(Bytes::from(minimal_jpeg())).unwrap();
        jpeg_set_exif(&mut jpeg, Some(b"not-tiff-at-all".to_vec()));
        let img = jpeg.encoder().bytes().to_vec();
        let edits = Edits {
            artist: Some("Ada".into()),
            ..Default::default()
        };
        let err = edit(&img, &edits).unwrap_err();
        assert!(err.contains("could not be parsed"), "got: {err}");
        assert!(err.contains("strip-exif"), "should point at strip-exif: {err}");
    }

    #[test]
    fn dms_conversion_carries_and_rounds() {
        // 48.8584° → 48° 51' 30.24" (sec×10⁴ = 302400)
        assert_eq!(to_dms_e4(48.8584), (48, 51, 302_400));
        // rounding at the very edge carries all the way up to the next degree
        assert_eq!(to_dms_e4(0.999_999_999_9), (1, 0, 0));
        assert_eq!(to_dms_e4(0.0), (0, 0, 0));
        assert_eq!(to_dms_e4(180.0), (180, 0, 0));
    }
}
