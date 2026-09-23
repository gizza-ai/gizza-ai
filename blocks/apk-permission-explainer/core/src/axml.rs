//! Minimal decoder for Android's binary XML (AXML) resource format, plus a
//! plain-text XML path so the same extraction code handles a decoded manifest.
//!
//! Only what a manifest needs is implemented: the string pool, the resource map
//! (used to recover attribute names when the pool entry is blank — common in
//! obfuscated/AAPT2-optimised APKs), and START_ELEMENT attribute records. Chunk
//! types we don't care about (namespaces, CDATA, end tags) are skipped by their
//! declared size, so unknown future chunks can't derail the walk.
//!
//! Reference: AOSP `frameworks/base/libs/androidfw/include/androidfw/ResourceTypes.h`.

const RES_STRING_POOL_TYPE: u16 = 0x0001;
const RES_XML_TYPE: u16 = 0x0003;
const RES_XML_RESOURCE_MAP_TYPE: u16 = 0x0180;
const RES_XML_START_ELEMENT_TYPE: u16 = 0x0102;

const UTF8_FLAG: u32 = 1 << 8;
const NO_ENTRY: u32 = 0xFFFF_FFFF;

/// `android:` namespace URI — attributes carrying it are platform attributes.
pub const ANDROID_NS: &str = "http://schemas.android.com/apk/res/android";

/// Resource ids for the handful of platform attributes a manifest reader needs.
/// Used only when the string pool has an empty name for the attribute.
const ATTR_IDS: &[(u32, &str)] = &[
    (0x0101_0001, "label"),
    (0x0101_0003, "name"),
    (0x0101_0020, "permission"),
    (0x0101_0106, "process"),
    (0x0101_020c, "minSdkVersion"),
    (0x0101_021b, "versionCode"),
    (0x0101_021c, "versionName"),
    (0x0101_0270, "targetSdkVersion"),
    (0x0101_0271, "maxSdkVersion"),
    (0x0101_0272, "required"),
    (0x0101_0572, "compileSdkVersion"),
];

/// One attribute of a start element, with its namespace URI (when present).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attr {
    pub ns: Option<String>,
    pub name: String,
    pub value: String,
}

/// A start element, flattened in document order. Nesting isn't needed: every
/// manifest element we read is addressed by its own tag name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Element {
    pub name: String,
    pub attrs: Vec<Attr>,
}

impl Element {
    /// First attribute whose *local* name matches, preferring the `android:`
    /// namespaced one. Manifests never give a tag both `name` and
    /// `android:name`, but preferring the platform namespace keeps us honest.
    pub fn attr(&self, local: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|a| a.name == local && a.ns.as_deref() == Some(ANDROID_NS))
            .or_else(|| self.attrs.iter().find(|a| a.name == local))
            .map(|a| a.value.as_str())
    }

    /// An attribute that must NOT be namespaced — `<manifest package="…">`.
    pub fn plain_attr(&self, local: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|a| a.name == local && a.ns.is_none())
            .map(|a| a.value.as_str())
    }
}

fn u16_at(b: &[u8], off: usize) -> Result<u16, String> {
    b.get(off..off + 2)
        .map(|s| u16::from_le_bytes([s[0], s[1]]))
        .ok_or_else(|| truncated(off))
}

fn u32_at(b: &[u8], off: usize) -> Result<u32, String> {
    b.get(off..off + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
        .ok_or_else(|| truncated(off))
}

fn truncated(off: usize) -> String {
    format!("binary AndroidManifest.xml is truncated: no data at byte offset {off}")
}

/// True when the buffer starts with an AXML file header.
pub fn looks_like_axml(b: &[u8]) -> bool {
    b.len() >= 8 && u16::from_le_bytes([b[0], b[1]]) == RES_XML_TYPE && b[2] == 0x08 && b[3] == 0x00
}

/// Decode a binary AndroidManifest.xml into a flat list of start elements.
pub fn decode(buf: &[u8]) -> Result<Vec<Element>, String> {
    if !looks_like_axml(buf) {
        return Err("not a binary Android XML file (expected the 03 00 08 00 chunk header)".into());
    }
    let file_size = u32_at(buf, 4)? as usize;
    // A trailing-padded APK entry may declare less than it carries; never more.
    let end = file_size.min(buf.len()).max(8);

    let mut pool: Vec<String> = Vec::new();
    let mut res_map: Vec<u32> = Vec::new();
    let mut elements: Vec<Element> = Vec::new();

    let mut off = 8usize;
    while off + 8 <= end {
        let chunk_type = u16_at(buf, off)?;
        let header_size = u16_at(buf, off + 2)? as usize;
        let chunk_size = u32_at(buf, off + 4)? as usize;
        if chunk_size < 8 || header_size < 8 || header_size > chunk_size {
            return Err(format!(
                "binary AndroidManifest.xml has a malformed chunk at offset {off} \
                 (type 0x{chunk_type:04x}, header {header_size}, size {chunk_size})"
            ));
        }
        if off + chunk_size > end {
            return Err(format!(
                "binary AndroidManifest.xml chunk at offset {off} runs past the end of the file"
            ));
        }
        match chunk_type {
            RES_STRING_POOL_TYPE => pool = parse_string_pool(buf, off, chunk_size)?,
            RES_XML_RESOURCE_MAP_TYPE => {
                res_map = (header_size..chunk_size)
                    .step_by(4)
                    .take_while(|p| p + 4 <= chunk_size)
                    .map(|p| u32_at(buf, off + p))
                    .collect::<Result<Vec<_>, _>>()?;
            }
            RES_XML_START_ELEMENT_TYPE => elements.push(parse_start_element(
                buf,
                off,
                header_size,
                chunk_size,
                &pool,
                &res_map,
            )?),
            _ => {}
        }
        off += chunk_size;
    }
    Ok(elements)
}

/// Resolve a pool index, tolerating the 0xFFFFFFFF "no entry" sentinel and
/// out-of-range indices (which appear in hand-crafted/packed manifests).
fn pool_str(pool: &[String], idx: u32) -> Option<&str> {
    if idx == NO_ENTRY {
        return None;
    }
    pool.get(idx as usize).map(|s| s.as_str())
}

fn parse_string_pool(buf: &[u8], off: usize, chunk_size: usize) -> Result<Vec<String>, String> {
    let string_count = u32_at(buf, off + 8)? as usize;
    let flags = u32_at(buf, off + 16)?;
    let strings_start = u32_at(buf, off + 20)? as usize;
    let utf8 = flags & UTF8_FLAG != 0;
    // Offsets follow the 28-byte pool header; each is relative to strings_start.
    let mut out = Vec::with_capacity(string_count.min(4096));
    for i in 0..string_count {
        let rel = u32_at(buf, off + 28 + i * 4)? as usize;
        let at = off + strings_start + rel;
        if at >= off + chunk_size {
            return Err("binary AndroidManifest.xml string pool offset is out of range".into());
        }
        out.push(if utf8 {
            read_utf8(buf, at)?
        } else {
            read_utf16(buf, at)?
        });
    }
    Ok(out)
}

/// Pool lengths are 1 unit, or 2 when the high bit of the first unit is set.
fn read_len8(buf: &[u8], at: usize) -> Result<(usize, usize), String> {
    let first = *buf.get(at).ok_or_else(|| truncated(at))? as usize;
    if first & 0x80 != 0 {
        let second = *buf.get(at + 1).ok_or_else(|| truncated(at + 1))? as usize;
        Ok((((first & 0x7F) << 8) | second, 2))
    } else {
        Ok((first, 1))
    }
}

fn read_utf8(buf: &[u8], at: usize) -> Result<String, String> {
    // First length is the UTF-16 character count, second the byte count.
    let (_, n1) = read_len8(buf, at)?;
    let (byte_len, n2) = read_len8(buf, at + n1)?;
    let start = at + n1 + n2;
    let bytes = buf
        .get(start..start + byte_len)
        .ok_or_else(|| truncated(start))?;
    Ok(String::from_utf8_lossy(bytes).into_owned())
}

fn read_utf16(buf: &[u8], at: usize) -> Result<String, String> {
    let first = u16_at(buf, at)? as usize;
    let (len, hdr) = if first & 0x8000 != 0 {
        (
            (((first & 0x7FFF) << 16) | u16_at(buf, at + 2)? as usize),
            4,
        )
    } else {
        (first, 2)
    };
    let start = at + hdr;
    let units = (0..len)
        .map(|i| u16_at(buf, start + i * 2))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(String::from_utf16_lossy(&units))
}

fn parse_start_element(
    buf: &[u8],
    off: usize,
    header_size: usize,
    chunk_size: usize,
    pool: &[String],
    res_map: &[u32],
) -> Result<Element, String> {
    let ext = off + header_size;
    let name_idx = u32_at(buf, ext + 4)?;
    let attr_start = u16_at(buf, ext + 8)? as usize;
    let attr_size = u16_at(buf, ext + 10)? as usize;
    let attr_count = u16_at(buf, ext + 12)? as usize;
    let name = pool_str(pool, name_idx).unwrap_or_default().to_string();

    let mut attrs = Vec::with_capacity(attr_count.min(64));
    if attr_size >= 20 {
        for i in 0..attr_count {
            let a = ext + attr_start + i * attr_size;
            if a + 20 > off + chunk_size {
                break; // A lying attributeCount truncates the list, not the parse.
            }
            let ns = pool_str(pool, u32_at(buf, a)?).map(str::to_string);
            let attr_name_idx = u32_at(buf, a + 4)?;
            let raw_value = u32_at(buf, a + 8)?;
            let data_type = *buf.get(a + 15).ok_or_else(|| truncated(a + 15))?;
            let data = u32_at(buf, a + 16)?;

            let mut attr_name = pool_str(pool, attr_name_idx)
                .unwrap_or_default()
                .to_string();
            if attr_name.is_empty() {
                // AAPT2-optimised manifests blank the name; the resource map
                // holds the attribute's platform id at the same index.
                if let Some(id) = res_map.get(attr_name_idx as usize) {
                    if let Some((_, known)) = ATTR_IDS.iter().find(|(k, _)| k == id) {
                        attr_name = (*known).to_string();
                    }
                }
            }
            attrs.push(Attr {
                ns,
                name: attr_name,
                value: attr_value(pool, raw_value, data_type, data),
            });
        }
    }
    Ok(Element { name, attrs })
}

/// Render an attribute's value the way `aapt dump xmltree` would.
fn attr_value(pool: &[String], raw_value: u32, data_type: u8, data: u32) -> String {
    if let Some(s) = pool_str(pool, raw_value) {
        return s.to_string();
    }
    match data_type {
        0x00 => String::new(),                                        // TYPE_NULL
        0x01 | 0x02 => format!("@0x{data:08x}"),                      // reference / attribute
        0x03 => pool_str(pool, data).unwrap_or_default().to_string(), // TYPE_STRING
        0x04 => format!("{}", f32::from_bits(data)),                  // TYPE_FLOAT
        0x10 => format!("{}", data as i32),                           // TYPE_INT_DEC
        0x11 => format!("0x{data:x}"),                                // TYPE_INT_HEX
        0x12 => if data != 0 { "true" } else { "false" }.to_string(), // TYPE_INT_BOOLEAN
        _ => format!("0x{data:x}"),
    }
}

/// Parse a plain-text XML manifest into the same flat element list. Lets the
/// tool accept a `aapt`/`apktool`-decoded manifest pasted as text, and keeps
/// the page examples readable.
pub fn parse_text_xml(text: &str) -> Result<Vec<Element>, String> {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(text);
    let config = reader.config_mut();
    config.trim_text(true);
    config.check_end_names = false;

    let mut out = Vec::new();
    let mut buf_events = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let mut prefixes: Vec<(String, String)> = Vec::new();
                let mut attrs = Vec::new();
                for a in e.attributes().flatten() {
                    let key = String::from_utf8_lossy(a.key.as_ref()).into_owned();
                    let value = String::from_utf8_lossy(&a.value).into_owned();
                    if let Some(prefix) = key.strip_prefix("xmlns:") {
                        prefixes.push((prefix.to_string(), value));
                        continue;
                    }
                    if key == "xmlns" {
                        continue;
                    }
                    match key.split_once(':') {
                        Some((prefix, local)) => {
                            attrs.push((Some(prefix.to_string()), local.to_string(), value))
                        }
                        None => attrs.push((None, key, value)),
                    }
                }
                buf_events.push((
                    String::from_utf8_lossy(local_name(e.name().as_ref())).into_owned(),
                    prefixes,
                    attrs,
                ));
            }
            Ok(_) => {}
            Err(e) => return Err(format!("could not parse the XML manifest: {e}")),
        }
    }

    // Resolve prefixes against every declaration seen (manifests declare
    // `xmlns:android` on the root, which is an ancestor of every other tag).
    let mut known: Vec<(String, String)> = Vec::new();
    for (_, prefixes, _) in &buf_events {
        for p in prefixes {
            if !known.iter().any(|(k, _)| k == &p.0) {
                known.push(p.clone());
            }
        }
    }
    for (name, _, attrs) in buf_events {
        out.push(Element {
            name,
            attrs: attrs
                .into_iter()
                .map(|(prefix, local, value)| Attr {
                    ns: prefix.map(|p| {
                        known
                            .iter()
                            .find(|(k, _)| *k == p)
                            .map(|(_, uri)| uri.clone())
                            .unwrap_or(p)
                    }),
                    name: local,
                    value,
                })
                .collect(),
        });
    }
    Ok(out)
}

fn local_name(raw: &[u8]) -> &[u8] {
    match raw.iter().position(|&b| b == b':') {
        Some(i) => &raw[i + 1..],
        None => raw,
    }
}
