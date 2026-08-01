//! video-gps-metadata-extractor core — pull the GPS location string that
//! QuickTime/MP4/MOV files embed in their metadata atoms, entirely in pure Rust
//! (no ffmpeg, no external byte-parsing crates). Shared by the chat skill block
//! and the web page.
//!
//! Where the location lives in an ISO base media / QuickTime container:
//!
//!   * `moov/udta/©xyz` — the classic QuickTime user-data atom (0xA9 'x' 'y' 'z')
//!     iPhones and many Android phones write. Payload is a QuickTime *text* atom:
//!     `[u16 byte-length][u16 language][UTF-8 text]`, the text being an ISO 6709
//!     coordinate string like `+37.7749-122.4194/` or `+37.7749-122.4194+9.500/`.
//!   * `moov/meta` (or file-level `meta`) `keys` + `ilst` — Apple's newer layout:
//!     a `keys` table names each metadata item (e.g.
//!     `com.apple.quicktime.location.ISO6709`) and the matching numbered `ilst`
//!     item carries the value inside a `data` box.
//!
//! We walk the box tree directly (`[u32 size][4-byte type][payload]`, with the
//! 64-bit `size==1` and to-EOF `size==0` forms handled), collect every location
//! string we find, and parse each ISO 6709 string into signed decimal latitude /
//! longitude / altitude. This is the *static* location tag — not the per-frame
//! GoPro GPMF telemetry track (a different, proprietary binary stream).

use serde::Serialize;

/// The QuickTime user-data key for a location string: 0xA9 'x' 'y' 'z'.
const XYZ: [u8; 4] = [0xA9, b'x', b'y', b'z'];
/// Apple metadata key whose `ilst` value is an ISO 6709 location string.
const APPLE_LOC_KEY: &str = "com.apple.quicktime.location.ISO6709";

/// Box types that contain child boxes we need to descend into.
const CONTAINERS: [&[u8; 4]; 5] = [b"moov", b"trak", b"udta", b"ilst", b"mdia"];

/// One decoded location tag.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Location {
    /// Where in the atom tree the value came from (e.g. `©xyz` or the Apple key).
    pub source: String,
    /// The raw ISO 6709 coordinate string exactly as stored.
    pub iso6709: String,
    pub latitude: f64,
    pub longitude: f64,
    /// Altitude in metres, when the ISO 6709 string carries a third component.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub altitude: Option<f64>,
    /// OpenStreetMap link centred on the coordinate.
    pub map_url: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    pub count: usize,
    pub locations: Vec<Location>,
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Extract GPS location tags from the QuickTime/MP4/MOV bytes in `input`.
///
/// - `input`: the video file bytes, encoded as a `base64` or `hex` string. Only
///   the small `moov`/metadata region is needed, so pasting the whole file is
///   not required — a truncated head works as long as it reaches the `moov` box.
/// - `input_format`: `"base64"` (default; standard or URL-safe, padding optional)
///   or `"hex"` (whitespace, `:` and `-` separators ignored).
/// - `output`: `"report"` (default, human-readable) or `"json"` (structured).
///
/// Returns `Err` on undecodable input or bytes that are not a recognizable
/// ISO base media / QuickTime container. A valid container with no location tag
/// is **not** an error — it returns a report saying none was found.
pub fn run(input: &str, input_format: &str, output: &str) -> Result<String, String> {
    let bytes = decode_bytes(input, input_format)?;
    let report = extract(&bytes)?;
    match output {
        "" | "report" => Ok(render_report(&report)),
        "json" => {
            serde_json::to_string_pretty(&report).map_err(|e| format!("serialize report: {e}"))
        }
        other => Err(format!(
            "invalid output {other:?}: expected \"report\" or \"json\""
        )),
    }
}

/// Parse the container bytes and collect every location tag.
pub fn extract(bytes: &[u8]) -> Result<Report, String> {
    if bytes.len() < 8 {
        return Err("input is too short to be an MP4/MOV file".into());
    }
    if !looks_like_isobmff(bytes) {
        return Err(
            "not a recognizable MP4/MOV/QuickTime file: no ISO base media box structure found \
             (expected an 'ftyp'/'moov'/'mdat'/'free' box at the start)"
                .into(),
        );
    }
    let mut locations = Vec::new();
    walk(bytes, 0, &mut locations);
    dedup(&mut locations);
    Ok(Report {
        count: locations.len(),
        locations,
    })
}

/// The first top-level box must have a plausible size and a printable,
/// known-ish 4-char type — enough to reject arbitrary/base64-garbage bytes
/// without a false negative on real files.
fn looks_like_isobmff(bytes: &[u8]) -> bool {
    let size = read_u32(bytes, 0) as usize;
    let ty = &bytes[4..8];
    if !ty.iter().all(|&b| b.is_ascii_graphic() || b == b' ') {
        return false;
    }
    // size==1 (64-bit) and size==0 (to EOF) are legal; otherwise it must fit.
    let size_ok = size == 0 || size == 1 || (8..=bytes.len()).contains(&size);
    let known_first = matches!(
        ty,
        b"ftyp" | b"moov" | b"mdat" | b"free" | b"skip" | b"wide" | b"pnot" | b"styp" | b"uuid"
    );
    size_ok && (known_first || ty == XYZ)
}

// ---------------------------------------------------------------------------
// Box walking
// ---------------------------------------------------------------------------

/// Recursively walk the boxes in `data` (a slice starting at a box boundary),
/// appending any location tags found. `depth` guards against pathological
/// nesting.
fn walk(data: &[u8], depth: u32, out: &mut Vec<Location>) {
    if depth > 32 {
        return;
    }
    let mut pos = 0usize;
    while pos + 8 <= data.len() {
        let size32 = read_u32(data, pos) as usize;
        let ty = [data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7]];
        let mut header = 8usize;
        let box_end = match size32 {
            // 64-bit size in the 8 bytes following the type.
            1 => {
                if pos + 16 > data.len() {
                    break;
                }
                let big = read_u64(data, pos + 8) as usize;
                header = 16;
                if big < header {
                    break;
                }
                pos.checked_add(big).unwrap_or(data.len()).min(data.len())
            }
            // Extends to the end of this container.
            0 => data.len(),
            n if n < 8 => break, // malformed: size can't be smaller than the header
            n => pos.checked_add(n).unwrap_or(data.len()).min(data.len()),
        };
        if box_end <= pos + header {
            // Empty or malformed payload; still advance to avoid an infinite loop.
            if box_end <= pos {
                break;
            }
            pos = box_end;
            continue;
        }
        let payload = &data[pos + header..box_end];

        if ty == XYZ {
            if let Some(text) = parse_quicktime_text(payload) {
                push_location(out, "©xyz (udta)", &text);
            }
        } else if &ty == b"meta" {
            walk(meta_children(payload), depth + 1, out);
        } else if &ty == b"ilst" {
            parse_ilst(data, pos + header, box_end, out);
        } else if CONTAINERS.iter().any(|c| *c == &ty) {
            walk(payload, depth + 1, out);
        }

        pos = box_end;
    }
}

/// A `meta` box is a FullBox (4-byte version/flags before its children) in ISO
/// base media, but a plain container (no version/flags) in classic QuickTime.
/// Disambiguate by checking whether the first child begins right at the start of
/// the payload or 4 bytes in.
fn meta_children(payload: &[u8]) -> &[u8] {
    if is_box_start(payload) {
        payload
    } else if payload.len() >= 4 && is_box_start(&payload[4..]) {
        &payload[4..]
    } else {
        // Fall back to the ISO fullbox layout; a wrong guess just finds nothing.
        payload.get(4..).unwrap_or(&[])
    }
}

/// Does `data` begin with a plausible box header (sane size + printable type)?
fn is_box_start(data: &[u8]) -> bool {
    if data.len() < 8 {
        return false;
    }
    let size = read_u32(data, 0) as usize;
    let ty = &data[4..8];
    let type_ok = ty.iter().all(|&b| b.is_ascii_graphic() || b == b' ');
    let size_ok = size == 0 || size == 1 || (8..=data.len()).contains(&size);
    type_ok && size_ok
}

/// Parse an `ilst` (Apple item list): each child is a numbered item box whose
/// index refers into the sibling `keys` table; the value sits in a `data` box.
/// We resolve the key name from the enclosing `meta`'s `keys` table when we can,
/// and otherwise fall back to recognising an ISO 6709 string by shape.
fn parse_ilst(container: &[u8], start: usize, end: usize, out: &mut Vec<Location>) {
    // The `keys` table lives as a sibling of this `ilst` inside the same `meta`.
    let keys = find_keys_table(container);
    let items = &container[start..end];
    let mut pos = 0usize;
    while pos + 8 <= items.len() {
        let size = read_u32(items, pos) as usize;
        if size < 8 || pos + size > items.len() {
            break;
        }
        // The item box type is its 1-based index into the keys table.
        let index = read_u32(items, pos + 4) as usize;
        let payload = &items[pos + 8..pos + size];
        if let Some(text) = parse_data_box(payload) {
            let key = keys.get(index.wrapping_sub(1)).map(String::as_str);
            let is_loc = matches!(key, Some(k) if k == APPLE_LOC_KEY || k.contains("location"))
                || (key.is_none() && looks_like_iso6709(&text));
            if is_loc {
                let src = key.unwrap_or("ilst location").to_string();
                push_location(out, &src, &text);
            }
        }
        pos += size;
    }
}

/// Scan `container` for the sibling `keys` table and return its ordered list of
/// key names. Best-effort: an empty list disables key-name resolution (we then
/// fall back to ISO-6709 shape detection).
fn find_keys_table(container: &[u8]) -> Vec<String> {
    let mut pos = 0usize;
    while pos + 8 <= container.len() {
        let size = read_u32(container, pos) as usize;
        if size < 8 || pos + size > container.len() {
            break;
        }
        if &container[pos + 4..pos + 8] == b"keys" {
            return parse_keys(&container[pos + 8..pos + size]);
        }
        pos += size;
    }
    Vec::new()
}

/// Parse a `keys` FullBox payload: `[u32 version/flags][u32 entry-count]` then
/// each entry `[u32 size][4-byte namespace][key string]`.
fn parse_keys(payload: &[u8]) -> Vec<String> {
    let mut keys = Vec::new();
    if payload.len() < 8 {
        return keys;
    }
    let count = read_u32(payload, 4) as usize;
    let mut pos = 8usize;
    for _ in 0..count {
        if pos + 8 > payload.len() {
            break;
        }
        let size = read_u32(payload, pos) as usize;
        if size < 8 || pos + size > payload.len() {
            break;
        }
        // bytes [pos+4..pos+8] are the namespace (e.g. "mdta"); the rest is the key.
        let name = String::from_utf8_lossy(&payload[pos + 8..pos + size]).into_owned();
        keys.push(name);
        pos += size;
    }
    keys
}

/// Parse an Apple item's `data` box: `[u32 size]"data"[u32 type-indicator][u32
/// locale][value]`. Returns the value as UTF-8 text.
fn parse_data_box(payload: &[u8]) -> Option<String> {
    if payload.len() < 16 {
        return None;
    }
    let size = read_u32(payload, 0) as usize;
    if &payload[4..8] != b"data" || size < 16 || size > payload.len() {
        return None;
    }
    // Skip the `data` header (8) + type-indicator (4) + locale (4).
    let text = &payload[16..size];
    let s = String::from_utf8_lossy(text).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// Parse a QuickTime text atom payload: `[u16 length][u16 language][text]`.
/// Falls back to treating the whole payload as UTF-8 when the length prefix is
/// absent or inconsistent (some muxers store the bare string).
fn parse_quicktime_text(payload: &[u8]) -> Option<String> {
    if payload.len() >= 4 {
        let len = read_u16(payload, 0) as usize;
        if len > 0 && 4 + len <= payload.len() {
            let s = String::from_utf8_lossy(&payload[4..4 + len])
                .trim()
                .to_string();
            if !s.is_empty() {
                return Some(s);
            }
        }
    }
    let s = String::from_utf8_lossy(payload).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

// ---------------------------------------------------------------------------
// ISO 6709 parsing
// ---------------------------------------------------------------------------

/// Build a `Location` from a raw ISO 6709 string and push it, if it parses.
fn push_location(out: &mut Vec<Location>, source: &str, raw: &str) {
    if let Some((lat, lon, alt)) = parse_iso6709(raw) {
        let map_url =
            format!("https://www.openstreetmap.org/?mlat={lat}&mlon={lon}#map=15/{lat}/{lon}");
        out.push(Location {
            source: source.to_string(),
            iso6709: raw.trim_end_matches('/').to_string(),
            latitude: lat,
            longitude: lon,
            altitude: alt,
            map_url,
        });
    }
}

/// Cheap shape test used when we have no key name to confirm a location value.
fn looks_like_iso6709(s: &str) -> bool {
    let t = s.trim();
    (t.starts_with('+') || t.starts_with('-')) && parse_iso6709(t).is_some()
}

/// Parse an ISO 6709 string of the decimal-degrees form used by QuickTime, e.g.
/// `+37.7749-122.4194/` or `+37.7749-122.4194+9.500/` (optional altitude, with a
/// trailing `/`). Returns `(latitude, longitude, altitude)`.
fn parse_iso6709(s: &str) -> Option<(f64, f64, Option<f64>)> {
    let s = s.trim().trim_end_matches('/');
    // Split into signed numeric fields: each begins with '+' or '-'.
    let mut fields: Vec<String> = Vec::new();
    let mut cur = String::new();
    for c in s.chars() {
        if (c == '+' || c == '-') && !cur.is_empty() {
            fields.push(std::mem::take(&mut cur));
        }
        if c == '+' || c == '-' || c == '.' || c.is_ascii_digit() {
            cur.push(c);
        } else {
            // Any other character ends the coordinate group.
            break;
        }
    }
    if !cur.is_empty() {
        fields.push(cur);
    }
    if fields.len() < 2 {
        return None;
    }
    let lat: f64 = fields[0].parse().ok()?;
    let lon: f64 = fields[1].parse().ok()?;
    if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
        return None;
    }
    let alt = fields.get(2).and_then(|f| f.parse::<f64>().ok());
    Some((round6(lat), round6(lon), alt.map(round6)))
}

fn round6(v: f64) -> f64 {
    (v * 1e6).round() / 1e6
}

/// Drop exact duplicate coordinates (the same tag can appear in both `©xyz` and
/// the Apple `ilst`), keeping the first-seen source.
fn dedup(locs: &mut Vec<Location>) {
    let mut seen: Vec<(f64, f64)> = Vec::new();
    locs.retain(|l| {
        let key = (l.latitude, l.longitude);
        if seen.contains(&key) {
            false
        } else {
            seen.push(key);
            true
        }
    });
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn render_report(r: &Report) -> String {
    if r.locations.is_empty() {
        return "No GPS location metadata found.\n\nChecked the QuickTime/MP4 location atoms \
                (©xyz in moov/udta and com.apple.quicktime.location.ISO6709 in the meta/ilst \
                table). This file carries no embedded location tag."
            .to_string();
    }
    let mut out = String::new();
    let noun = if r.count == 1 {
        "location"
    } else {
        "locations"
    };
    out.push_str(&format!("GPS {noun} found: {}\n", r.count));
    for (i, l) in r.locations.iter().enumerate() {
        out.push_str(&format!("\n#{}  {}\n", i + 1, l.source));
        out.push_str(&format!("  ISO 6709    {}\n", l.iso6709));
        out.push_str(&format!("  Latitude    {}\n", l.latitude));
        out.push_str(&format!("  Longitude   {}\n", l.longitude));
        if let Some(alt) = l.altitude {
            out.push_str(&format!("  Altitude    {alt} m\n"));
        }
        out.push_str(&format!("  Map         {}\n", l.map_url));
    }
    out.trim_end().to_string()
}

// ---------------------------------------------------------------------------
// Byte helpers + decoding
// ---------------------------------------------------------------------------

fn read_u16(b: &[u8], o: usize) -> u16 {
    ((b[o] as u16) << 8) | b[o + 1] as u16
}
fn read_u32(b: &[u8], o: usize) -> u32 {
    ((b[o] as u32) << 24) | ((b[o + 1] as u32) << 16) | ((b[o + 2] as u32) << 8) | b[o + 3] as u32
}
fn read_u64(b: &[u8], o: usize) -> u64 {
    ((read_u32(b, o) as u64) << 32) | read_u32(b, o + 4) as u64
}

fn decode_bytes(input: &str, input_format: &str) -> Result<Vec<u8>, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("input is empty: paste the MP4/MOV file bytes as base64 or hex".into());
    }
    match input_format {
        "" | "base64" => decode_base64(trimmed),
        "hex" => decode_hex(trimmed),
        other => Err(format!(
            "invalid input_format {other:?}: expected \"base64\" or \"hex\""
        )),
    }
}

fn decode_hex(s: &str) -> Result<Vec<u8>, String> {
    let compact: String = s
        .chars()
        .filter(|c| !c.is_whitespace() && *c != ':' && *c != '-')
        .collect();
    if compact.len() % 2 != 0 {
        return Err("invalid hex: odd number of digits".into());
    }
    let bytes = compact.as_bytes();
    let mut out = Vec::with_capacity(compact.len() / 2);
    for pair in bytes.chunks(2) {
        let hi = hex_val(pair[0])?;
        let lo = hex_val(pair[1])?;
        out.push((hi << 4) | lo);
    }
    Ok(out)
}

fn hex_val(c: u8) -> Result<u8, String> {
    match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        b'A'..=b'F' => Ok(c - b'A' + 10),
        _ => Err(format!("invalid hex digit {:?}", c as char)),
    }
}

/// Standard + URL-safe base64, padding optional.
fn decode_base64(s: &str) -> Result<Vec<u8>, String> {
    const INVALID: u8 = 255;
    let val = |c: u8| -> u8 {
        match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => INVALID,
        }
    };
    let mut buf = 0u32;
    let mut bits = 0u32;
    let mut out = Vec::new();
    for &c in s.as_bytes() {
        if c == b'=' || c.is_ascii_whitespace() {
            continue;
        }
        let v = val(c);
        if v == INVALID {
            return Err(format!("invalid base64 character {:?}", c as char));
        }
        buf = (buf << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- box builders for synthetic fixtures ------------------------------

    fn box_of(ty: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let size = (8 + payload.len()) as u32;
        let mut v = size.to_be_bytes().to_vec();
        v.extend_from_slice(ty);
        v.extend_from_slice(payload);
        v
    }

    /// A minimal file: `ftyp` + `moov` wrapping the given payload.
    fn file_with_moov(moov_payload: &[u8]) -> Vec<u8> {
        let mut v = box_of(b"ftyp", b"qt  \x00\x00\x02\x00qt  ");
        v.extend(box_of(b"moov", moov_payload));
        v
    }

    fn xyz_atom(text: &str) -> Vec<u8> {
        let mut payload = (text.len() as u16).to_be_bytes().to_vec();
        payload.extend_from_slice(&0x15c7u16.to_be_bytes()); // language code
        payload.extend_from_slice(text.as_bytes());
        box_of(&XYZ, &payload)
    }

    fn apple_data_box(value: &str) -> Vec<u8> {
        let mut data_payload = vec![0, 0, 0, 1]; // type indicator (UTF-8)
        data_payload.extend_from_slice(&[0, 0, 0, 0]); // locale
        data_payload.extend_from_slice(value.as_bytes());
        box_of(b"data", &data_payload)
    }

    #[test]
    fn extracts_xyz_from_udta() {
        let udta = box_of(b"udta", &xyz_atom("+37.7749-122.4194/"));
        let file = file_with_moov(&udta);
        let r = extract(&file).unwrap();
        assert_eq!(r.count, 1);
        assert_eq!(r.locations[0].latitude, 37.7749);
        assert_eq!(r.locations[0].longitude, -122.4194);
        assert_eq!(r.locations[0].altitude, None);
        assert!(r.locations[0].source.contains("xyz"));
        assert!(r.locations[0].map_url.contains("mlat=37.7749"));
    }

    #[test]
    fn extracts_xyz_with_altitude() {
        let udta = box_of(b"udta", &xyz_atom("+51.5074-000.1278+35.000/"));
        let file = file_with_moov(&udta);
        let r = extract(&file).unwrap();
        assert_eq!(r.count, 1);
        assert_eq!(r.locations[0].latitude, 51.5074);
        assert_eq!(r.locations[0].longitude, -0.1278);
        assert_eq!(r.locations[0].altitude, Some(35.0));
    }

    #[test]
    fn extracts_apple_keys_ilst_location() {
        // keys table with one key = the Apple location key.
        let key_entry = {
            let name = APPLE_LOC_KEY.as_bytes();
            let size = (8 + name.len()) as u32;
            let mut e = size.to_be_bytes().to_vec();
            e.extend_from_slice(b"mdta");
            e.extend_from_slice(name);
            e
        };
        let mut keys_payload = vec![0, 0, 0, 0]; // version/flags
        keys_payload.extend_from_slice(&1u32.to_be_bytes()); // entry count
        keys_payload.extend(key_entry);
        let keys = box_of(b"keys", &keys_payload);

        // item box: type == index 1 (0x00000001), wrapping the data box.
        let item = box_of(&[0, 0, 0, 1], &apple_data_box("+48.8566+002.3522/"));
        let ilst = box_of(b"ilst", &item);

        let mut meta_payload = vec![0, 0, 0, 0]; // meta fullbox version/flags
        meta_payload.extend(keys);
        meta_payload.extend(ilst);
        let meta = box_of(b"meta", &meta_payload);
        let file = file_with_moov(&meta);

        let r = extract(&file).unwrap();
        assert_eq!(r.count, 1, "report: {r:?}");
        assert_eq!(r.locations[0].latitude, 48.8566);
        assert_eq!(r.locations[0].longitude, 2.3522);
        assert!(r.locations[0].source.contains("location"));
    }

    #[test]
    fn quicktime_meta_without_fullbox_header() {
        // Classic QuickTime `meta` is a plain container (no version/flags).
        let item = box_of(&[0, 0, 0, 1], &apple_data_box("+12.3400+056.7800/"));
        let ilst = box_of(b"ilst", &item);
        let meta = box_of(b"meta", &ilst); // NO 4-byte version/flags prefix
        let file = file_with_moov(&meta);
        let r = extract(&file).unwrap();
        assert_eq!(r.count, 1, "report: {r:?}");
        assert_eq!(r.locations[0].latitude, 12.34);
        assert_eq!(r.locations[0].longitude, 56.78);
    }

    #[test]
    fn no_gps_is_not_an_error() {
        let udta = box_of(b"udta", &box_of(b"\xa9nam", b"\x00\x05\x15\xc7title"));
        let file = file_with_moov(&udta);
        let r = extract(&file).unwrap();
        assert_eq!(r.count, 0);
        let text = render_report(&r);
        assert!(text.contains("No GPS location"), "got: {text}");
    }

    #[test]
    fn dedup_merges_xyz_and_ilst_same_coord() {
        // udta/©xyz AND a meta/ilst with the same coordinate → one entry.
        let udta = box_of(b"udta", &xyz_atom("+10.0000+020.0000/"));
        let item = box_of(&[0, 0, 0, 1], &apple_data_box("+10.0000+020.0000/"));
        let ilst = box_of(b"ilst", &item);
        let mut meta_payload = vec![0, 0, 0, 0];
        meta_payload.extend(ilst);
        let meta = box_of(b"meta", &meta_payload);
        let mut moov = udta;
        moov.extend(meta);
        let file = file_with_moov(&moov);
        let r = extract(&file).unwrap();
        assert_eq!(r.count, 1);
    }

    #[test]
    fn rejects_non_container_bytes() {
        let err = extract(b"this is definitely not an mp4 file at all!!").unwrap_err();
        assert!(err.contains("not a recognizable"), "got: {err}");
    }

    #[test]
    fn rejects_out_of_range_coords() {
        // latitude 99 is invalid → not treated as a location.
        let udta = box_of(b"udta", &xyz_atom("+99.0000+200.0000/"));
        let file = file_with_moov(&udta);
        let r = extract(&file).unwrap();
        assert_eq!(r.count, 0);
    }

    #[test]
    fn parse_iso6709_forms() {
        assert_eq!(
            parse_iso6709("+37.7749-122.4194/"),
            Some((37.7749, -122.4194, None))
        );
        assert_eq!(
            parse_iso6709("+51.5074-000.1278+35.5/"),
            Some((51.5074, -0.1278, Some(35.5)))
        );
        assert_eq!(
            parse_iso6709("-33.8688+151.2093/"),
            Some((-33.8688, 151.2093, None))
        );
        assert_eq!(parse_iso6709("garbage"), None);
    }

    #[test]
    fn run_end_to_end_hex_and_json() {
        let udta = box_of(b"udta", &xyz_atom("+37.7749-122.4194/"));
        let file = file_with_moov(&udta);
        let hex: String = file.iter().map(|b| format!("{b:02x}")).collect();
        let report = run(&hex, "hex", "report").unwrap();
        assert!(report.contains("GPS location found: 1"), "got: {report}");
        assert!(report.contains("Latitude    37.7749"), "got: {report}");
        let json = run(&hex, "hex", "json").unwrap();
        assert!(json.contains("\"latitude\": 37.7749"), "got: {json}");
        assert!(json.contains("\"count\": 1"), "got: {json}");
    }

    #[test]
    fn decode_and_input_errors() {
        assert!(run("", "base64", "report").unwrap_err().contains("empty"));
        let udta = box_of(b"udta", &xyz_atom("+1.0+2.0/"));
        let file = file_with_moov(&udta);
        let hex: String = file.iter().map(|b| format!("{b:02x}")).collect();
        assert!(run(&hex, "hex", "yaml")
            .unwrap_err()
            .contains("invalid output"));
        assert!(run(&hex, "morse", "report")
            .unwrap_err()
            .contains("invalid input_format"));
    }
}
