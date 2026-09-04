//! gltf-glb-converter core — pure compute, shared by the chat skill block and the web page.
//!
//! Converts a glTF 2.0 asset between the two containers the format ships in:
//!
//! * **`.gltf`** — a JSON document whose buffers and images live in external
//!   files (`scene.bin`, `color.png`) or in `data:` URIs.
//! * **`.glb`** — one binary file: a 12-byte header followed by a JSON chunk and
//!   an optional `BIN` chunk holding every buffer back to back.
//!
//! Both directions are covered, plus the two chores that surround them: pulling an
//! external `.bin` into the packed file, and moving image bytes between the binary
//! buffer and `data:` URIs. GLB is not text, so its bytes are pasted as base64 or
//! hex (a `data:model/gltf-binary;base64,…` URL works too) and binary output comes
//! back the same way.
//!
//! Nothing is re-encoded: accessor data is copied byte for byte, so a
//! GLB → glTF → GLB round trip on a single-buffer asset reproduces the original
//! bytes exactly. No I/O, no network.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;
use serde_json::{json, Map, Value};

/// Maximum size of the pasted model, in characters. Base64 is 4/3 the size of the
/// bytes it carries, so this admits a little over 16 MiB of GLB.
pub const MAX_TEXT_CHARS: usize = 24 * 1024 * 1024;
/// Maximum decoded size of any single binary input (the GLB, or the external
/// `.bin`), in bytes. Keeps the wasm sandbox (64 MiB) clear of the copies a
/// repack makes.
pub const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;

/// The GLB magic, `glTF` in ASCII.
const GLB_MAGIC: [u8; 4] = *b"glTF";
const CHUNK_JSON: u32 = 0x4E4F_534A; // "JSON"
const CHUNK_BIN: u32 = 0x004E_4942; // "BIN\0"

// ---------------------------------------------------------------- options ---

/// How the pasted model is encoded.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InputFormat {
    /// Sniff JSON text vs base64 / hex / `data:` bytes.
    Auto,
    /// glTF JSON text.
    Gltf,
    /// GLB bytes, base64.
    Base64,
    /// GLB bytes, hex.
    Hex,
}

impl InputFormat {
    pub fn parse(s: &str) -> Result<InputFormat, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" | "" => Ok(InputFormat::Auto),
            "gltf" | "json" => Ok(InputFormat::Gltf),
            "base64" | "b64" => Ok(InputFormat::Base64),
            "hex" => Ok(InputFormat::Hex),
            other => Err(format!(
                "unknown input format '{other}': expected 'auto', 'gltf', 'base64' or 'hex'"
            )),
        }
    }
}

/// The container to write.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Target {
    /// The other container from the one that came in.
    Auto,
    /// Binary GLB.
    Glb,
    /// glTF JSON.
    Gltf,
}

impl Target {
    pub fn parse(s: &str) -> Result<Target, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" | "flip" | "" => Ok(Target::Auto),
            "glb" => Ok(Target::Glb),
            "gltf" | "json" => Ok(Target::Gltf),
            other => Err(format!(
                "unknown target '{other}': expected 'auto', 'glb' or 'gltf'"
            )),
        }
    }
}

/// What to return.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Output {
    /// The converted model.
    File,
    /// A human-readable conversion report.
    Summary,
    /// Just the binary buffer — the `.bin` beside an unpacked `.gltf`.
    Buffer,
}

impl Output {
    pub fn parse(s: &str) -> Result<Output, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "file" | "model" | "" => Ok(Output::File),
            "summary" | "report" => Ok(Output::Summary),
            "buffer" | "bin" => Ok(Output::Buffer),
            other => Err(format!(
                "unknown output '{other}': expected 'file', 'summary' or 'buffer'"
            )),
        }
    }
}

/// Where image bytes should end up.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Images {
    /// Pack into the binary buffer when writing GLB; leave them alone otherwise.
    Auto,
    /// Move `data:` URI images into the binary buffer as buffer views.
    Buffer,
    /// Move buffer-view images out to `data:` URIs.
    Uri,
}

impl Images {
    pub fn parse(s: &str) -> Result<Images, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" | "" => Ok(Images::Auto),
            "buffer" | "embed" | "pack" => Ok(Images::Buffer),
            "uri" | "data-uri" | "datauri" | "separate" => Ok(Images::Uri),
            other => Err(format!(
                "unknown images mode '{other}': expected 'auto', 'buffer' or 'uri'"
            )),
        }
    }
}

/// How binary output is returned.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OutputEncoding {
    DataUrl,
    Base64,
    Hex,
}

impl OutputEncoding {
    pub fn parse(s: &str) -> Result<OutputEncoding, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "data-url" | "dataurl" | "data_url" | "" => Ok(OutputEncoding::DataUrl),
            "base64" | "b64" => Ok(OutputEncoding::Base64),
            "hex" => Ok(OutputEncoding::Hex),
            other => Err(format!(
                "unknown output encoding '{other}': expected 'data-url', 'base64' or 'hex'"
            )),
        }
    }
}

/// Every knob the converter takes besides the model itself.
#[derive(Clone, Debug)]
pub struct Options {
    /// Bytes of the external buffer file the `.gltf` references, base64 or hex.
    pub bin: String,
    pub input_format: InputFormat,
    pub to: Target,
    pub output: Output,
    pub images: Images,
    /// When writing glTF JSON, the `uri` to record for the buffer instead of
    /// embedding it as a `data:` URI. Blank embeds.
    pub buffer_uri: String,
    /// Pretty-print the glTF JSON.
    pub pretty: bool,
    pub output_encoding: OutputEncoding,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            bin: String::new(),
            input_format: InputFormat::Auto,
            to: Target::Auto,
            output: Output::File,
            images: Images::Auto,
            buffer_uri: String::new(),
            pretty: true,
            output_encoding: OutputEncoding::DataUrl,
        }
    }
}

// ------------------------------------------------------------- byte utils ---

fn pad4(n: usize) -> usize {
    (n + 3) & !3
}

fn u32le(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

fn to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

fn from_hex(s: &str) -> Result<Vec<u8>, String> {
    let cleaned: String = s
        .chars()
        .filter(|c| !c.is_whitespace() && *c != ':' && *c != '-' && *c != ',')
        .collect();
    if cleaned.len() % 2 != 0 {
        return Err(format!(
            "hex input has an odd number of digits ({}); every byte needs two",
            cleaned.len()
        ));
    }
    let bytes = cleaned.as_bytes();
    let mut out = Vec::with_capacity(cleaned.len() / 2);
    for pair in bytes.chunks(2) {
        let hi = (pair[0] as char)
            .to_digit(16)
            .ok_or_else(|| format!("'{}' is not a hex digit", pair[0] as char))?;
        let lo = (pair[1] as char)
            .to_digit(16)
            .ok_or_else(|| format!("'{}' is not a hex digit", pair[1] as char))?;
        out.push((hi * 16 + lo) as u8);
    }
    Ok(out)
}

fn from_base64(s: &str) -> Result<Vec<u8>, String> {
    let cleaned: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    B64.decode(cleaned.as_bytes())
        .or_else(|_| {
            base64::engine::general_purpose::URL_SAFE
                .decode(cleaned.as_bytes())
                .or_else(|_| {
                    base64::engine::general_purpose::STANDARD_NO_PAD.decode(cleaned.as_bytes())
                })
        })
        .map_err(|e| format!("not valid base64: {e}"))
}

/// Split `data:<mime>[;base64],<payload>` into its mime and its bytes.
fn decode_data_uri(uri: &str) -> Result<(String, Vec<u8>), String> {
    let rest = uri
        .strip_prefix("data:")
        .ok_or_else(|| format!("'{}' is not a data: URI", truncate(uri, 48)))?;
    let comma = rest.find(',').ok_or_else(|| {
        "malformed data: URI — no ',' separating the header from the payload".to_string()
    })?;
    let header = &rest[..comma];
    let payload = &rest[comma + 1..];
    let is_b64 = header.to_ascii_lowercase().ends_with(";base64");
    let mime = header
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    if !is_b64 {
        return Err(format!(
            "data: URI '{}' is not base64-encoded; percent-encoded data: URIs are not supported",
            truncate(uri, 48)
        ));
    }
    Ok((mime, from_base64(payload)?))
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(n).collect::<String>())
    }
}

fn human_bytes(n: usize) -> String {
    if n < 1024 {
        format!("{n} B")
    } else if n < 1024 * 1024 {
        format!("{:.1} KB", n as f64 / 1024.0)
    } else {
        format!("{:.2} MB", n as f64 / (1024.0 * 1024.0))
    }
}

/// Guess an image mime from its first bytes.
fn sniff_image_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        Some("image/png")
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else if bytes.starts_with(&[0xAB, b'K', b'T', b'X']) {
        Some("image/ktx2")
    } else if bytes.starts_with(b"\x73\x42") {
        Some("image/x-basis")
    } else {
        None
    }
}

fn encode_binary(bytes: &[u8], enc: OutputEncoding, mime: &str) -> String {
    match enc {
        OutputEncoding::DataUrl => format!("data:{mime};base64,{}", B64.encode(bytes)),
        OutputEncoding::Base64 => B64.encode(bytes),
        OutputEncoding::Hex => to_hex(bytes),
    }
}

// ------------------------------------------------------------ GLB codec -----

/// The pieces of a parsed GLB.
#[derive(Debug)]
struct Glb {
    json: String,
    bin: Option<Vec<u8>>,
    json_len: usize,
    bin_len: usize,
    notes: Vec<String>,
}

fn parse_glb(bytes: &[u8]) -> Result<Glb, String> {
    if bytes.len() < 12 {
        return Err(format!(
            "GLB is only {} bytes; the 12-byte header alone needs magic, version and length",
            bytes.len()
        ));
    }
    if bytes[0..4] != GLB_MAGIC {
        return Err(format!(
            "not a GLB: expected the magic 'glTF' (67 6c 54 46), got {} — if this is glTF JSON, it should start with '{{'",
            to_hex(&bytes[0..4])
        ));
    }
    let version = u32le(bytes, 4);
    if version != 2 {
        return Err(format!(
            "GLB container version {version} is not supported; only version 2 (glTF 2.0) is"
        ));
    }
    let declared = u32le(bytes, 8) as usize;
    let mut notes = Vec::new();
    let end = if declared > bytes.len() {
        return Err(format!(
            "GLB header declares a total length of {declared} bytes but only {} were supplied — the file looks truncated",
            bytes.len()
        ));
    } else {
        if declared < bytes.len() {
            notes.push(format!(
                "{} trailing bytes after the declared GLB length were ignored",
                bytes.len() - declared
            ));
        }
        declared.max(12)
    };

    let mut json: Option<String> = None;
    let mut json_len = 0usize;
    let mut bin: Option<Vec<u8>> = None;
    let mut bin_len = 0usize;
    let mut off = 12usize;
    while off + 8 <= end {
        let clen = u32le(bytes, off) as usize;
        let ctype = u32le(bytes, off + 4);
        let start = off + 8;
        if start + clen > end {
            return Err(format!(
                "chunk at offset {off} declares {clen} bytes but only {} remain in the file",
                end.saturating_sub(start)
            ));
        }
        let data = &bytes[start..start + clen];
        match ctype {
            CHUNK_JSON => {
                if json.is_some() {
                    return Err("GLB contains more than one JSON chunk".to_string());
                }
                json_len = clen;
                let text = std::str::from_utf8(data)
                    .map_err(|e| format!("GLB JSON chunk is not valid UTF-8: {e}"))?;
                json = Some(text.trim_end_matches(['\u{20}', '\0']).to_string());
            }
            CHUNK_BIN => {
                if bin.is_some() {
                    return Err("GLB contains more than one BIN chunk".to_string());
                }
                bin_len = clen;
                bin = Some(data.to_vec());
            }
            other => notes.push(format!(
                "unknown GLB chunk type 0x{other:08x} ({} bytes) was skipped",
                clen
            )),
        }
        off = start + pad4(clen);
    }
    let json = json.ok_or_else(|| {
        "GLB has no JSON chunk; the first chunk after the header must be of type 'JSON'".to_string()
    })?;
    Ok(Glb {
        json,
        bin,
        json_len,
        bin_len,
        notes,
    })
}

fn build_glb(json: &[u8], bin: &[u8]) -> Vec<u8> {
    let json_padded = pad4(json.len());
    let bin_padded = pad4(bin.len());
    let has_bin = !bin.is_empty();
    let total = 12 + 8 + json_padded + if has_bin { 8 + bin_padded } else { 0 };
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(&GLB_MAGIC);
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(json_padded as u32).to_le_bytes());
    out.extend_from_slice(&CHUNK_JSON.to_le_bytes());
    out.extend_from_slice(json);
    out.resize(12 + 8 + json_padded, b' ');
    if has_bin {
        out.extend_from_slice(&(bin_padded as u32).to_le_bytes());
        out.extend_from_slice(&CHUNK_BIN.to_le_bytes());
        out.extend_from_slice(bin);
        out.resize(total, 0);
    }
    out
}

// -------------------------------------------------------- input detection ---

enum Source {
    Json(String),
    Glb(Vec<u8>),
}

fn looks_hex(s: &str) -> bool {
    let mut count = 0usize;
    for c in s.chars() {
        if c.is_whitespace() || c == ':' || c == '-' || c == ',' {
            continue;
        }
        if !c.is_ascii_hexdigit() {
            return false;
        }
        count += 1;
    }
    count >= 8 && count % 2 == 0
}

fn classify_bytes(bytes: Vec<u8>, what: &str) -> Result<Source, String> {
    if bytes.starts_with(&GLB_MAGIC) {
        return Ok(Source::Glb(bytes));
    }
    let leading = bytes.iter().position(|b| !b.is_ascii_whitespace());
    if leading.map(|i| bytes[i]) == Some(b'{') {
        let text = String::from_utf8(bytes)
            .map_err(|e| format!("{what} decoded to bytes that are not valid UTF-8 JSON: {e}"))?;
        return Ok(Source::Json(text));
    }
    Err(format!(
        "{what} decoded to {} bytes that are neither a GLB (which starts with the magic 'glTF') nor glTF JSON (which starts with '{{')",
        bytes.len()
    ))
}

fn decode_source(model: &str, fmt: InputFormat) -> Result<Source, String> {
    let trimmed = model.trim();
    if trimmed.is_empty() {
        return Err(
            "no model supplied: paste glTF JSON text, or a GLB's bytes as base64 or hex"
                .to_string(),
        );
    }
    if model.len() > MAX_TEXT_CHARS {
        return Err(format!(
            "model is {} characters, over the {} limit",
            model.len(),
            MAX_TEXT_CHARS
        ));
    }
    let source = match fmt {
        InputFormat::Gltf => {
            if !trimmed.starts_with('{') {
                return Err(format!(
                    "input_format is 'gltf' but the text starts with '{}' instead of '{{' — glTF JSON is a JSON object",
                    truncate(trimmed, 16)
                ));
            }
            Source::Json(trimmed.to_string())
        }
        InputFormat::Base64 => {
            let payload = if trimmed.starts_with("data:") {
                decode_data_uri(trimmed)?.1
            } else {
                from_base64(trimmed)?
            };
            classify_bytes(payload, "the base64 input")?
        }
        InputFormat::Hex => classify_bytes(from_hex(trimmed)?, "the hex input")?,
        InputFormat::Auto => {
            if trimmed.starts_with('{') {
                Source::Json(trimmed.to_string())
            } else if trimmed.starts_with("data:") {
                classify_bytes(decode_data_uri(trimmed)?.1, "the data: URL")?
            } else if looks_hex(trimmed) {
                classify_bytes(from_hex(trimmed)?, "the hex input")?
            } else if let Ok(bytes) = from_base64(trimmed) {
                classify_bytes(bytes, "the base64 input")?
            } else {
                return Err(
                    "could not tell what was pasted: expected glTF JSON starting with '{', or a GLB's bytes as base64 (starts 'Z2xURg'), hex (starts '676c5446') or a data:model/gltf-binary;base64 URL"
                        .to_string(),
                );
            }
        }
    };
    if let Source::Glb(bytes) = &source {
        if bytes.len() > MAX_INPUT_BYTES {
            return Err(format!(
                "GLB is {}, over the {} limit",
                human_bytes(bytes.len()),
                human_bytes(MAX_INPUT_BYTES)
            ));
        }
    }
    Ok(source)
}

// ------------------------------------------------------------- repacking ----

/// Rewrite every `"bufferView": n` in the document through `remap`.
fn remap_buffer_views(value: &mut Value, remap: &[Option<usize>]) -> Result<(), String> {
    match value {
        Value::Object(map) => {
            for (key, v) in map.iter_mut() {
                if key == "bufferView" {
                    if let Some(old) = v.as_u64() {
                        let old = old as usize;
                        let new = remap
                            .get(old)
                            .copied()
                            .flatten()
                            .ok_or_else(|| format!("buffer view {old} is still referenced after it was moved out to a data: URI; re-run with images=auto"))?;
                        *v = json!(new);
                        continue;
                    }
                }
                remap_buffer_views(v, remap)?;
            }
            Ok(())
        }
        Value::Array(items) => {
            for item in items.iter_mut() {
                remap_buffer_views(item, remap)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn array_len(doc: &Value, key: &str) -> usize {
    doc.get(key)
        .and_then(|v| v.as_array())
        .map_or(0, |a| a.len())
}

// ------------------------------------------------------------ conversion ----

struct Conversion {
    doc: Value,
    blob: Vec<u8>,
    /// Human-readable notes worth surfacing in the summary.
    notes: Vec<String>,
    images_to_buffer: usize,
    images_to_uri: usize,
    repacked: bool,
}

/// Resolve every declared buffer into bytes.
fn resolve_buffers(
    doc: &Value,
    src_bin: Option<&Vec<u8>>,
    external: Option<&Vec<u8>>,
    notes: &mut Vec<String>,
) -> Result<Vec<Vec<u8>>, String> {
    let buffers = match doc.get("buffers").and_then(|v| v.as_array()) {
        Some(a) => a,
        None => return Ok(Vec::new()),
    };
    let mut external_left = external;
    let mut out = Vec::with_capacity(buffers.len());
    for (i, buf) in buffers.iter().enumerate() {
        let byte_length = buf
            .get("byteLength")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| format!("buffers[{i}] has no numeric byteLength"))?
            as usize;
        let uri = buf.get("uri").and_then(|v| v.as_str());
        let mut bytes = match uri {
            None => src_bin.cloned().ok_or_else(|| {
                format!(
                    "buffers[{i}] has no uri, which only a GLB's BIN chunk may do — a .gltf buffer must carry a uri"
                )
            })?,
            Some(u) if u.starts_with("data:") => decode_data_uri(u)
                .map_err(|e| format!("buffers[{i}]: {e}"))?
                .1,
            Some(u) => {
                let taken = external_left.take().ok_or_else(|| {
                    format!(
                        "buffers[{i}] points at the external file '{}' ({}), which this tool cannot read. Paste that file's bytes as base64 into the external buffer field.",
                        truncate(u, 60),
                        human_bytes(byte_length)
                    )
                })?;
                notes.push(format!(
                    "external buffer '{}' was supplied and packed in",
                    truncate(u, 40)
                ));
                taken.clone()
            }
        };
        if bytes.len() < byte_length {
            return Err(format!(
                "buffers[{i}] declares byteLength {byte_length} but only {} bytes are available",
                bytes.len()
            ));
        }
        if bytes.len() > byte_length {
            bytes.truncate(byte_length);
        }
        out.push(bytes);
    }
    Ok(out)
}

fn plan_and_repack(
    mut doc: Value,
    buffers: Vec<Vec<u8>>,
    images: Images,
    target: Target,
    mut notes: Vec<String>,
) -> Result<Conversion, String> {
    let effective = match images {
        Images::Auto if target == Target::Glb => Images::Buffer,
        Images::Auto => Images::Auto,
        other => other,
    };

    // Which image entries move, and in which direction.
    let mut to_buffer: Vec<usize> = Vec::new(); // image index, data: uri -> buffer view
    let mut to_uri: Vec<usize> = Vec::new(); // image index, buffer view -> data: uri
    if let Some(list) = doc.get("images").and_then(|v| v.as_array()) {
        for (i, img) in list.iter().enumerate() {
            let has_view = img.get("bufferView").and_then(|v| v.as_u64()).is_some();
            let data_uri = img
                .get("uri")
                .and_then(|v| v.as_str())
                .map(|u| u.starts_with("data:"))
                .unwrap_or(false);
            match effective {
                Images::Buffer if data_uri && !has_view => to_buffer.push(i),
                Images::Uri if has_view => to_uri.push(i),
                _ => {}
            }
        }
    }

    let needs_repack = buffers.len() > 1 || !to_buffer.is_empty() || !to_uri.is_empty();
    if !needs_repack {
        let blob = buffers.into_iter().next().unwrap_or_default();
        return Ok(Conversion {
            doc,
            blob,
            notes,
            images_to_buffer: 0,
            images_to_uri: 0,
            repacked: false,
        });
    }

    if doc
        .get("extensionsUsed")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .any(|e| e.as_str() == Some("EXT_meshopt_compression"))
        })
        .unwrap_or(false)
    {
        return Err(
            "this asset uses EXT_meshopt_compression, whose buffer views carry their own buffer offsets, so its bytes cannot be relocated. Convert it with a single buffer and images=auto, or decompress it first."
                .to_string(),
        );
    }

    // Slice out every existing buffer view.
    let existing = doc
        .get("bufferViews")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let mut slices: Vec<Vec<u8>> = Vec::with_capacity(existing.len());
    for (i, view) in existing.iter().enumerate() {
        let buffer = view.get("buffer").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let offset = view.get("byteOffset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let length = view
            .get("byteLength")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| format!("bufferViews[{i}] has no numeric byteLength"))?
            as usize;
        let src = buffers.get(buffer).ok_or_else(|| {
            format!(
                "bufferViews[{i}] references buffer {buffer}, but the asset declares {}",
                buffers.len()
            )
        })?;
        if offset + length > src.len() {
            return Err(format!(
                "bufferViews[{i}] reads bytes {}..{} of buffer {buffer}, which is only {} bytes",
                offset,
                offset + length,
                src.len()
            ));
        }
        slices.push(src[offset..offset + length].to_vec());
    }

    // Images moving OUT to data: URIs — their views are dropped.
    let mut dropped: Vec<usize> = Vec::new();
    let images_to_uri = to_uri.len();
    for &img_index in &to_uri {
        let (view_index, mime) = {
            let img = &doc["images"][img_index];
            let vi = img["bufferView"].as_u64().unwrap() as usize;
            let mime = img
                .get("mimeType")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .or_else(|| {
                    slices
                        .get(vi)
                        .and_then(|b| sniff_image_mime(b))
                        .map(String::from)
                })
                .unwrap_or_else(|| "application/octet-stream".to_string());
            (vi, mime)
        };
        let bytes = slices
            .get(view_index)
            .ok_or_else(|| {
                format!(
                    "images[{img_index}] references buffer view {view_index}, which does not exist"
                )
            })?
            .clone();
        let entry = doc["images"][img_index].as_object_mut().unwrap();
        entry.remove("bufferView");
        entry.insert(
            "uri".to_string(),
            json!(format!("data:{mime};base64,{}", B64.encode(&bytes))),
        );
        entry.insert("mimeType".to_string(), json!(mime));
        dropped.push(view_index);
    }

    // Rebuild the blob from the retained views, 4-byte aligned.
    let mut blob: Vec<u8> = Vec::new();
    let mut remap: Vec<Option<usize>> = vec![None; existing.len()];
    let mut new_views: Vec<Value> = Vec::new();
    for (i, view) in existing.iter().enumerate() {
        if dropped.contains(&i) {
            continue;
        }
        while blob.len() % 4 != 0 {
            blob.push(0);
        }
        let offset = blob.len();
        blob.extend_from_slice(&slices[i]);
        let mut out = view.as_object().cloned().unwrap_or_default();
        out.insert("buffer".to_string(), json!(0));
        out.insert("byteOffset".to_string(), json!(offset));
        out.insert("byteLength".to_string(), json!(slices[i].len()));
        remap[i] = Some(new_views.len());
        new_views.push(Value::Object(out));
    }

    // Every surviving reference has to follow the retained views to their new
    // indices. This runs BEFORE the views appended below, whose indices are
    // already final and must not be remapped.
    if let Some(obj) = doc.as_object_mut() {
        for (key, value) in obj.iter_mut() {
            if key == "bufferViews" {
                continue;
            }
            remap_buffer_views(value, &remap)?;
        }
    }

    // Images moving IN from data: URIs — appended as fresh views.
    let images_to_buffer = to_buffer.len();
    for &img_index in &to_buffer {
        let uri = doc["images"][img_index]["uri"]
            .as_str()
            .unwrap()
            .to_string();
        let (mime, bytes) =
            decode_data_uri(&uri).map_err(|e| format!("images[{img_index}]: {e}"))?;
        let mime = if mime.is_empty() || mime == "application/octet-stream" {
            sniff_image_mime(&bytes).unwrap_or("image/png").to_string()
        } else {
            mime
        };
        while blob.len() % 4 != 0 {
            blob.push(0);
        }
        let offset = blob.len();
        blob.extend_from_slice(&bytes);
        let view_index = new_views.len();
        new_views.push(json!({
            "buffer": 0,
            "byteOffset": offset,
            "byteLength": bytes.len(),
        }));
        let entry = doc["images"][img_index].as_object_mut().unwrap();
        entry.remove("uri");
        entry.insert("mimeType".to_string(), json!(mime));
        entry.insert("bufferView".to_string(), json!(view_index));
    }

    if !new_views.is_empty() {
        doc["bufferViews"] = Value::Array(new_views);
    } else if doc.get("bufferViews").is_some() {
        doc.as_object_mut().unwrap().remove("bufferViews");
    }
    if buffers.len() > 1 {
        notes.push(format!("{} buffers were merged into one", buffers.len()));
    }

    Ok(Conversion {
        doc,
        blob,
        notes,
        images_to_buffer,
        images_to_uri,
        repacked: true,
    })
}

// ------------------------------------------------------------------ run -----

/// Convert a glTF asset between the `.gltf` and `.glb` containers.
pub fn convert(model: &str, opt: &Options) -> Result<String, String> {
    let source = decode_source(model, opt.input_format)?;
    let source_is_glb = matches!(source, Source::Glb(_));

    let external = if opt.bin.trim().is_empty() {
        None
    } else {
        let bytes = if opt.bin.trim().starts_with("data:") {
            decode_data_uri(opt.bin.trim())?.1
        } else if looks_hex(opt.bin.trim()) {
            from_hex(opt.bin.trim())?
        } else {
            from_base64(opt.bin.trim()).map_err(|e| {
                format!("external buffer is neither base64, hex nor a data: URI: {e}")
            })?
        };
        if bytes.len() > MAX_INPUT_BYTES {
            return Err(format!(
                "external buffer is {}, over the {} limit",
                human_bytes(bytes.len()),
                human_bytes(MAX_INPUT_BYTES)
            ));
        }
        Some(bytes)
    };

    let (json_text, src_bin, in_json_len, in_bin_len, in_total, mut notes) = match source {
        Source::Json(text) => {
            let len = text.len();
            (text, None, len, 0usize, len, Vec::new())
        }
        Source::Glb(bytes) => {
            let total = bytes.len();
            let glb = parse_glb(&bytes)?;
            (
                glb.json,
                glb.bin,
                glb.json_len,
                glb.bin_len,
                total,
                glb.notes,
            )
        }
    };

    let doc: Value = serde_json::from_str(&json_text).map_err(|e| {
        format!(
            "glTF JSON could not be parsed: {e}. The document must be a single JSON object starting with '{{'."
        )
    })?;
    if !doc.is_object() {
        return Err("glTF JSON must be an object, not an array or a scalar".to_string());
    }
    let gltf_version = doc
        .get("asset")
        .and_then(|a| a.get("version"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if gltf_version.is_empty() {
        return Err(
            "glTF JSON has no asset.version — every glTF 2.0 document must declare \"asset\": { \"version\": \"2.0\" }"
                .to_string(),
        );
    }
    if !gltf_version.starts_with('2') {
        return Err(format!(
            "asset.version is '{gltf_version}'; this converter handles glTF 2.0 only"
        ));
    }
    let generator = doc
        .get("asset")
        .and_then(|a| a.get("generator"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let target = match opt.to {
        Target::Auto => {
            if source_is_glb {
                Target::Gltf
            } else {
                Target::Glb
            }
        }
        t => t,
    };

    let buffers = resolve_buffers(&doc, src_bin.as_ref(), external.as_ref(), &mut notes)?;
    let buffer_count_in = buffers.len();
    let bytes_in: usize = buffers.iter().map(|b| b.len()).sum();
    let counts = Counts::of(&doc);

    let conv = plan_and_repack(doc, buffers, opt.images, target, notes)?;
    let Conversion {
        mut doc,
        blob,
        mut notes,
        images_to_buffer,
        images_to_uri,
        repacked,
    } = conv;

    // Rewrite the buffer list for the chosen container.
    let mut buffer_uri_written = String::new();
    if !blob.is_empty() {
        let mut entry = Map::new();
        match target {
            Target::Glb => {
                entry.insert("byteLength".to_string(), json!(blob.len()));
            }
            _ => {
                let uri = if opt.buffer_uri.trim().is_empty() {
                    format!("data:application/octet-stream;base64,{}", B64.encode(&blob))
                } else {
                    opt.buffer_uri.trim().to_string()
                };
                buffer_uri_written = if opt.buffer_uri.trim().is_empty() {
                    format!("embedded data: URI ({})", human_bytes(blob.len()))
                } else {
                    opt.buffer_uri.trim().to_string()
                };
                entry.insert("uri".to_string(), json!(uri));
                entry.insert("byteLength".to_string(), json!(blob.len()));
            }
        }
        doc["buffers"] = Value::Array(vec![Value::Object(entry)]);
    } else if doc.get("buffers").is_some() {
        doc.as_object_mut().unwrap().remove("buffers");
        notes.push("the asset declares no binary data, so no BIN chunk was written".to_string());
    }

    let json_out = if target == Target::Glb || !opt.pretty {
        serde_json::to_string(&doc)
    } else {
        serde_json::to_string_pretty(&doc)
    }
    .map_err(|e| format!("could not re-serialize the glTF JSON: {e}"))?;

    let (rendered, out_len, out_json_len, out_bin_len) = match target {
        Target::Glb => {
            let glb = build_glb(json_out.as_bytes(), &blob);
            let len = glb.len();
            (
                encode_binary(&glb, opt.output_encoding, "model/gltf-binary"),
                len,
                pad4(json_out.len()),
                if blob.is_empty() { 0 } else { pad4(blob.len()) },
            )
        }
        _ => {
            let len = json_out.len();
            (json_out, len, len, 0)
        }
    };

    match opt.output {
        Output::File => Ok(rendered),
        Output::Buffer => {
            if blob.is_empty() {
                Err("this asset has no binary buffer to extract — every buffer view is empty or absent".to_string())
            } else {
                Ok(encode_binary(
                    &blob,
                    opt.output_encoding,
                    "application/octet-stream",
                ))
            }
        }
        Output::Summary => Ok(summary(SummaryInput {
            source_is_glb,
            target,
            gltf_version: &gltf_version,
            generator: &generator,
            in_total,
            in_json_len,
            in_bin_len,
            buffer_count_in,
            bytes_in,
            counts: &counts,
            out_len,
            out_json_len,
            out_bin_len,
            blob_len: blob.len(),
            buffer_uri_written: &buffer_uri_written,
            images_to_buffer,
            images_to_uri,
            repacked,
            output_encoding: opt.output_encoding,
            notes: &notes,
            extensions: doc
                .get("extensionsUsed")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|e| e.as_str())
                        .map(String::from)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
        })),
    }
}

// -------------------------------------------------------------- summary -----

struct Counts {
    scenes: usize,
    nodes: usize,
    meshes: usize,
    primitives: usize,
    materials: usize,
    textures: usize,
    images: usize,
    animations: usize,
    skins: usize,
    cameras: usize,
    accessors: usize,
    buffer_views: usize,
    vertices: usize,
    triangles: usize,
}

impl Counts {
    fn of(doc: &Value) -> Counts {
        let mut primitives = 0usize;
        let mut vertices = 0usize;
        let mut triangles = 0usize;
        let accessor_count = |i: Option<u64>| -> usize {
            i.and_then(|i| {
                doc.get("accessors")
                    .and_then(|a| a.get(i as usize))
                    .and_then(|a| a.get("count"))
                    .and_then(|c| c.as_u64())
            })
            .unwrap_or(0) as usize
        };
        if let Some(meshes) = doc.get("meshes").and_then(|v| v.as_array()) {
            for mesh in meshes {
                if let Some(prims) = mesh.get("primitives").and_then(|v| v.as_array()) {
                    primitives += prims.len();
                    for prim in prims {
                        let mode = prim.get("mode").and_then(|m| m.as_u64()).unwrap_or(4);
                        let verts = accessor_count(
                            prim.get("attributes")
                                .and_then(|a| a.get("POSITION"))
                                .and_then(|v| v.as_u64()),
                        );
                        vertices += verts;
                        let indices = prim.get("indices").and_then(|v| v.as_u64());
                        let count = if indices.is_some() {
                            accessor_count(indices)
                        } else {
                            verts
                        };
                        if mode == 4 {
                            triangles += count / 3;
                        }
                    }
                }
            }
        }
        Counts {
            scenes: array_len(doc, "scenes"),
            nodes: array_len(doc, "nodes"),
            meshes: array_len(doc, "meshes"),
            primitives,
            materials: array_len(doc, "materials"),
            textures: array_len(doc, "textures"),
            images: array_len(doc, "images"),
            animations: array_len(doc, "animations"),
            skins: array_len(doc, "skins"),
            cameras: array_len(doc, "cameras"),
            accessors: array_len(doc, "accessors"),
            buffer_views: array_len(doc, "bufferViews"),
            vertices,
            triangles,
        }
    }
}

struct SummaryInput<'a> {
    source_is_glb: bool,
    target: Target,
    gltf_version: &'a str,
    generator: &'a str,
    in_total: usize,
    in_json_len: usize,
    in_bin_len: usize,
    buffer_count_in: usize,
    bytes_in: usize,
    counts: &'a Counts,
    out_len: usize,
    out_json_len: usize,
    out_bin_len: usize,
    blob_len: usize,
    buffer_uri_written: &'a str,
    images_to_buffer: usize,
    images_to_uri: usize,
    repacked: bool,
    output_encoding: OutputEncoding,
    notes: &'a [String],
    extensions: Vec<String>,
}

fn row(out: &mut String, label: &str, value: String) {
    out.push_str(&format!("  {label:<18}{value}\n"));
}

fn summary(s: SummaryInput) -> String {
    let mut out = String::new();
    out.push_str("glTF / GLB conversion\n\n");
    let from = if s.source_is_glb { "GLB" } else { "glTF JSON" };
    let to = if s.target == Target::Glb {
        "GLB"
    } else {
        "glTF JSON"
    };
    row(&mut out, "Direction", format!("{from} -> {to}"));
    row(&mut out, "glTF version", s.gltf_version.to_string());
    if !s.generator.is_empty() {
        row(&mut out, "Generator", truncate(s.generator, 60));
    }

    out.push_str("\nInput\n");
    row(&mut out, "Size", human_bytes(s.in_total));
    if s.source_is_glb {
        row(&mut out, "JSON chunk", human_bytes(s.in_json_len));
        row(
            &mut out,
            "BIN chunk",
            if s.in_bin_len == 0 {
                "none".to_string()
            } else {
                human_bytes(s.in_bin_len)
            },
        );
    }
    row(
        &mut out,
        "Buffers",
        format!("{} ({})", s.buffer_count_in, human_bytes(s.bytes_in)),
    );

    out.push_str("\nContents\n");
    let c = s.counts;
    row(
        &mut out,
        "Scenes / nodes",
        format!("{} / {}", c.scenes, c.nodes),
    );
    row(
        &mut out,
        "Meshes",
        format!("{} ({} primitives)", c.meshes, c.primitives),
    );
    row(
        &mut out,
        "Geometry",
        format!("{} vertices, {} triangles", c.vertices, c.triangles),
    );
    row(
        &mut out,
        "Materials",
        format!(
            "{} ({} textures, {} images)",
            c.materials, c.textures, c.images
        ),
    );
    row(
        &mut out,
        "Animations / skins",
        format!("{} / {}", c.animations, c.skins),
    );
    row(
        &mut out,
        "Accessors / views",
        format!("{} / {}", c.accessors, c.buffer_views),
    );
    if c.cameras > 0 {
        row(&mut out, "Cameras", c.cameras.to_string());
    }
    if !s.extensions.is_empty() {
        row(&mut out, "Extensions", s.extensions.join(", "));
    }

    out.push_str("\nOutput\n");
    row(&mut out, "Size", human_bytes(s.out_len));
    if s.target == Target::Glb {
        row(&mut out, "JSON chunk", human_bytes(s.out_json_len));
        row(
            &mut out,
            "BIN chunk",
            if s.out_bin_len == 0 {
                "none".to_string()
            } else {
                human_bytes(s.out_bin_len)
            },
        );
        row(
            &mut out,
            "Returned as",
            match s.output_encoding {
                OutputEncoding::DataUrl => "data:model/gltf-binary;base64 URL",
                OutputEncoding::Base64 => "raw base64 bytes",
                OutputEncoding::Hex => "raw hex bytes",
            }
            .to_string(),
        );
    } else {
        row(
            &mut out,
            "Buffer",
            if s.blob_len == 0 {
                "none".to_string()
            } else {
                s.buffer_uri_written.to_string()
            },
        );
    }
    row(
        &mut out,
        "Byte-exact copy",
        if s.repacked {
            "no - buffer views were relocated".to_string()
        } else {
            "yes - accessor bytes were copied unchanged".to_string()
        },
    );
    if s.images_to_buffer > 0 {
        row(
            &mut out,
            "Images packed",
            format!(
                "{} moved from data: URIs into the buffer",
                s.images_to_buffer
            ),
        );
    }
    if s.images_to_uri > 0 {
        row(
            &mut out,
            "Images extracted",
            format!("{} moved from the buffer into data: URIs", s.images_to_uri),
        );
    }

    if !s.notes.is_empty() {
        out.push_str("\nNotes\n");
        for note in s.notes {
            out.push_str(&format!("  - {note}\n"));
        }
    }
    out
}

/// Convenience wrapper taking raw string/bool arguments, in the same order as the
/// descriptor params and the page fields.
#[allow(clippy::too_many_arguments)]
pub fn run(
    model: &str,
    bin: &str,
    input_format: &str,
    to: &str,
    output: &str,
    images: &str,
    buffer_uri: &str,
    pretty: bool,
    output_encoding: &str,
) -> Result<String, String> {
    let opt = Options {
        bin: bin.to_string(),
        input_format: InputFormat::parse(input_format)?,
        to: Target::parse(to)?,
        output: Output::parse(output)?,
        images: Images::parse(images)?,
        buffer_uri: buffer_uri.to_string(),
        pretty,
        output_encoding: OutputEncoding::parse(output_encoding)?,
    };
    convert(model, &opt)
}

// ------------------------------------------------------------------ tests ---

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal but complete glTF 2.0 triangle with one external buffer.
    const TRIANGLE_EXTERNAL: &str = r#"{
      "asset": { "version": "2.0", "generator": "test" },
      "scene": 0,
      "scenes": [{ "nodes": [0] }],
      "nodes": [{ "mesh": 0 }],
      "meshes": [{ "primitives": [{ "attributes": { "POSITION": 0 } }] }],
      "accessors": [{ "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3",
                      "min": [0,0,0], "max": [1,1,0] }],
      "bufferViews": [{ "buffer": 0, "byteOffset": 0, "byteLength": 36 }],
      "buffers": [{ "uri": "tri.bin", "byteLength": 36 }]
    }"#;

    /// The 36 bytes those three VEC3 float positions occupy.
    fn triangle_bin() -> Vec<u8> {
        let coords: [f32; 9] = [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0];
        coords.iter().flat_map(|f| f.to_le_bytes()).collect()
    }

    fn embedded_triangle() -> String {
        TRIANGLE_EXTERNAL.replace(
            "\"uri\": \"tri.bin\"",
            &format!(
                "\"uri\": \"data:application/octet-stream;base64,{}\"",
                B64.encode(triangle_bin())
            ),
        )
    }

    #[test]
    fn packs_embedded_gltf_into_glb() {
        let out = run(
            &embedded_triangle(),
            "",
            "auto",
            "auto",
            "file",
            "auto",
            "",
            true,
            "base64",
        )
        .unwrap();
        let bytes = from_base64(&out).unwrap();
        assert_eq!(&bytes[0..4], b"glTF");
        assert_eq!(u32le(&bytes, 4), 2);
        assert_eq!(u32le(&bytes, 8) as usize, bytes.len());
        let glb = parse_glb(&bytes).unwrap();
        assert_eq!(glb.bin.as_ref().unwrap()[..36], triangle_bin()[..]);
        let doc: Value = serde_json::from_str(&glb.json).unwrap();
        assert!(doc["buffers"][0].get("uri").is_none());
        assert_eq!(doc["buffers"][0]["byteLength"], 36);
    }

    #[test]
    fn packs_external_bin_supplied_separately() {
        let out = run(
            TRIANGLE_EXTERNAL,
            &B64.encode(triangle_bin()),
            "gltf",
            "glb",
            "file",
            "auto",
            "",
            true,
            "data-url",
        )
        .unwrap();
        assert!(out.starts_with("data:model/gltf-binary;base64,"));
        let bytes = from_base64(out.split_once(",").unwrap().1).unwrap();
        let glb = parse_glb(&bytes).unwrap();
        assert_eq!(glb.bin.unwrap()[..36], triangle_bin()[..]);
    }

    #[test]
    fn unpacks_glb_back_to_self_contained_gltf() {
        let packed = run(
            &embedded_triangle(),
            "",
            "auto",
            "glb",
            "file",
            "auto",
            "",
            true,
            "base64",
        )
        .unwrap();
        let back = run(
            &packed, "", "base64", "auto", "file", "auto", "", true, "base64",
        )
        .unwrap();
        assert!(back.starts_with('{'));
        let doc: Value = serde_json::from_str(&back).unwrap();
        let uri = doc["buffers"][0]["uri"].as_str().unwrap();
        assert!(uri.starts_with("data:application/octet-stream;base64,"));
        assert_eq!(decode_data_uri(uri).unwrap().1, triangle_bin());
        assert_eq!(doc["buffers"][0]["byteLength"], 36);
    }

    #[test]
    fn round_trip_glb_to_gltf_to_glb_is_byte_exact() {
        let glb = run(
            &embedded_triangle(),
            "",
            "auto",
            "glb",
            "file",
            "auto",
            "",
            true,
            "base64",
        )
        .unwrap();
        let gltf = run(
            &glb, "", "base64", "gltf", "file", "auto", "", true, "base64",
        )
        .unwrap();
        let again = run(&gltf, "", "gltf", "glb", "file", "auto", "", true, "base64").unwrap();
        assert_eq!(glb, again);
    }

    #[test]
    fn external_buffer_uri_keeps_the_bin_outside() {
        let glb = run(
            &embedded_triangle(),
            "",
            "auto",
            "glb",
            "file",
            "auto",
            "",
            true,
            "base64",
        )
        .unwrap();
        let gltf = run(
            &glb,
            "",
            "base64",
            "gltf",
            "file",
            "auto",
            "scene.bin",
            true,
            "base64",
        )
        .unwrap();
        let doc: Value = serde_json::from_str(&gltf).unwrap();
        assert_eq!(doc["buffers"][0]["uri"], "scene.bin");
        let bin = run(
            &glb,
            "",
            "base64",
            "gltf",
            "buffer",
            "auto",
            "scene.bin",
            true,
            "base64",
        )
        .unwrap();
        assert_eq!(from_base64(&bin).unwrap(), triangle_bin());
    }

    #[test]
    fn images_move_into_and_out_of_the_buffer() {
        let png: Vec<u8> = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 1, 2, 3];
        let with_image = embedded_triangle().replace(
            "\"scene\": 0,",
            &format!(
                "\"scene\": 0, \"images\": [{{ \"uri\": \"data:image/png;base64,{}\" }}],",
                B64.encode(&png)
            ),
        );
        let glb = run(
            &with_image,
            "",
            "auto",
            "glb",
            "file",
            "buffer",
            "",
            true,
            "base64",
        )
        .unwrap();
        let parsed = parse_glb(&from_base64(&glb).unwrap()).unwrap();
        let doc: Value = serde_json::from_str(&parsed.json).unwrap();
        assert!(doc["images"][0].get("uri").is_none());
        assert_eq!(doc["images"][0]["mimeType"], "image/png");
        let view = doc["images"][0]["bufferView"].as_u64().unwrap() as usize;
        let vb = &doc["bufferViews"][view];
        let off = vb["byteOffset"].as_u64().unwrap() as usize;
        let len = vb["byteLength"].as_u64().unwrap() as usize;
        assert_eq!(&parsed.bin.as_ref().unwrap()[off..off + len], &png[..]);
        // The accessor's view must have followed the remap.
        assert_eq!(doc["accessors"][0]["bufferView"], 0);

        // ...and back out again.
        let gltf = run(
            &glb, "", "base64", "gltf", "file", "uri", "", true, "base64",
        )
        .unwrap();
        let doc: Value = serde_json::from_str(&gltf).unwrap();
        let uri = doc["images"][0]["uri"].as_str().unwrap();
        assert_eq!(decode_data_uri(uri).unwrap().1, png);
        assert!(doc["images"][0].get("bufferView").is_none());
        assert_eq!(doc["bufferViews"].as_array().unwrap().len(), 1);
        assert_eq!(doc["accessors"][0]["bufferView"], 0);
    }

    #[test]
    fn merges_multiple_buffers_into_one() {
        let half: Vec<u8> = triangle_bin()[..18].to_vec();
        let rest: Vec<u8> = triangle_bin()[18..].to_vec();
        let two = format!(
            r#"{{
              "asset": {{ "version": "2.0" }},
              "meshes": [{{ "primitives": [{{ "attributes": {{ "POSITION": 0 }} }}] }}],
              "accessors": [{{ "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3" }}],
              "bufferViews": [
                {{ "buffer": 0, "byteOffset": 0, "byteLength": 18 }},
                {{ "buffer": 1, "byteOffset": 0, "byteLength": 18 }}
              ],
              "buffers": [
                {{ "uri": "data:application/octet-stream;base64,{}", "byteLength": 18 }},
                {{ "uri": "data:application/octet-stream;base64,{}", "byteLength": 18 }}
              ]
            }}"#,
            B64.encode(&half),
            B64.encode(&rest)
        );
        let glb = run(&two, "", "gltf", "glb", "file", "auto", "", true, "base64").unwrap();
        let parsed = parse_glb(&from_base64(&glb).unwrap()).unwrap();
        let doc: Value = serde_json::from_str(&parsed.json).unwrap();
        assert_eq!(doc["buffers"].as_array().unwrap().len(), 1);
        assert_eq!(doc["bufferViews"][1]["buffer"], 0);
        assert_eq!(doc["bufferViews"][1]["byteOffset"], 20); // 18 rounded up to 4
        assert_eq!(parsed.bin.as_ref().unwrap()[..18], half[..]);
        assert_eq!(parsed.bin.as_ref().unwrap()[20..38], rest[..]);
    }

    #[test]
    fn summary_reports_the_conversion() {
        let out = run(
            &embedded_triangle(),
            "",
            "auto",
            "glb",
            "summary",
            "auto",
            "",
            true,
            "data-url",
        )
        .unwrap();
        assert!(out.contains("Direction         glTF JSON -> GLB"), "{out}");
        assert!(
            out.contains("Geometry          3 vertices, 1 triangles"),
            "{out}"
        );
        assert!(out.contains("Byte-exact copy   yes"), "{out}");
    }

    #[test]
    fn hex_input_is_accepted() {
        let glb = run(
            &embedded_triangle(),
            "",
            "auto",
            "glb",
            "file",
            "auto",
            "",
            true,
            "hex",
        )
        .unwrap();
        assert!(glb.starts_with("676c5446"));
        let back = run(&glb, "", "hex", "gltf", "file", "auto", "", true, "base64").unwrap();
        assert!(back.starts_with('{'));
    }

    #[test]
    fn pretty_false_emits_compact_json() {
        let compact = run(
            &embedded_triangle(),
            "",
            "auto",
            "gltf",
            "file",
            "auto",
            "",
            false,
            "base64",
        )
        .unwrap();
        assert!(!compact.contains("\n"));
        let pretty = run(
            &embedded_triangle(),
            "",
            "auto",
            "gltf",
            "file",
            "auto",
            "",
            true,
            "base64",
        )
        .unwrap();
        assert!(pretty.contains("\n  \"asset\""));
    }

    // --- errors -------------------------------------------------------------

    #[test]
    fn missing_external_buffer_is_explained() {
        let err = run(
            TRIANGLE_EXTERNAL,
            "",
            "gltf",
            "glb",
            "file",
            "auto",
            "",
            true,
            "base64",
        )
        .unwrap_err();
        assert!(err.contains("tri.bin"), "{err}");
        assert!(err.contains("external buffer field"), "{err}");
    }

    #[test]
    fn non_gltf_bytes_are_rejected() {
        let err = run(
            &B64.encode(b"\x89PNG\r\n\x1a\n not a model"),
            "",
            "base64",
            "auto",
            "file",
            "auto",
            "",
            true,
            "base64",
        )
        .unwrap_err();
        assert!(err.contains("neither a GLB"), "{err}");
    }

    #[test]
    fn truncated_glb_is_reported() {
        let glb = from_base64(
            &run(
                &embedded_triangle(),
                "",
                "auto",
                "glb",
                "file",
                "auto",
                "",
                true,
                "base64",
            )
            .unwrap(),
        )
        .unwrap();
        let err = run(
            &B64.encode(&glb[..glb.len() - 8]),
            "",
            "base64",
            "gltf",
            "file",
            "auto",
            "",
            true,
            "base64",
        )
        .unwrap_err();
        assert!(err.contains("truncated"), "{err}");
    }

    #[test]
    fn gltf_1_is_rejected() {
        let err = run(
            r#"{"asset":{"version":"1.0"}}"#,
            "",
            "gltf",
            "glb",
            "file",
            "auto",
            "",
            true,
            "base64",
        )
        .unwrap_err();
        assert!(err.contains("glTF 2.0 only"), "{err}");
    }

    #[test]
    fn missing_asset_version_is_rejected() {
        let err = run(
            r#"{"meshes":[]}"#,
            "",
            "gltf",
            "glb",
            "file",
            "auto",
            "",
            true,
            "base64",
        )
        .unwrap_err();
        assert!(err.contains("asset.version"), "{err}");
    }

    #[test]
    fn unknown_enum_values_are_rejected() {
        let err = run(
            &embedded_triangle(),
            "",
            "auto",
            "fbx",
            "file",
            "auto",
            "",
            true,
            "base64",
        )
        .unwrap_err();
        assert!(err.contains("expected 'auto', 'glb' or 'gltf'"), "{err}");
        let err = run(
            &embedded_triangle(),
            "",
            "auto",
            "auto",
            "file",
            "auto",
            "",
            true,
            "yaml",
        )
        .unwrap_err();
        assert!(
            err.contains("expected 'data-url', 'base64' or 'hex'"),
            "{err}"
        );
    }

    #[test]
    fn empty_input_is_explained() {
        let err = run("  ", "", "auto", "auto", "file", "auto", "", true, "base64").unwrap_err();
        assert!(err.contains("no model supplied"), "{err}");
    }

    #[test]
    fn buffer_output_without_binary_data_errors() {
        let err = run(
            r#"{"asset":{"version":"2.0"},"scenes":[{"nodes":[]}]}"#,
            "",
            "gltf",
            "glb",
            "buffer",
            "auto",
            "",
            true,
            "base64",
        )
        .unwrap_err();
        assert!(err.contains("no binary buffer"), "{err}");
    }

    #[test]
    fn meshopt_assets_refuse_relocation() {
        let doc = embedded_triangle().replace(
            "\"scene\": 0,",
            "\"scene\": 0, \"extensionsUsed\": [\"EXT_meshopt_compression\"], \"images\": [{ \"uri\": \"data:image/png;base64,iVBORw0KGgo=\" }],",
        );
        let err = run(
            &doc, "", "gltf", "glb", "file", "buffer", "", true, "base64",
        )
        .unwrap_err();
        assert!(err.contains("EXT_meshopt_compression"), "{err}");
    }
}
