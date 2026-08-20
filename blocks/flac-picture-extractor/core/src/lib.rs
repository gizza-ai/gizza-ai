//! gizza-ai/flac-picture-extractor core — pull the embedded artwork out of a
//! native FLAC file's metadata and report everything the file declares about it.
//!
//! A FLAC stream is the ASCII marker `fLaC` followed by a chain of metadata
//! blocks. Each block starts with one byte (bit 7 = "this is the last block",
//! bits 6..0 = block type) and a 24-bit big-endian payload length. Artwork
//! lives in block type 6, `PICTURE`, whose payload is (all integers 32-bit
//! big-endian, in this order):
//!
//! ```text
//! picture type (the ID3v2 APIC table, 0-20)
//! MIME length, MIME string (ASCII; the literal "-->" means the payload is a URL)
//! description length, description (UTF-8)
//! width, height, colour depth in bits per pixel, number of indexed colours (0 if not indexed)
//! payload length, payload bytes
//! ```
//!
//! Two more places real files keep artwork, both handled here because tagging
//! libraries write them and a block-type-6-only reader silently misses them:
//!
//! * a base64 `METADATA_BLOCK_PICTURE` field inside the `VORBIS_COMMENT` block —
//!   the exact same payload layout, just base64-encoded; and
//! * the deprecated `COVERART` / `COVERARTMIME` field pair, which is base64 of
//!   the raw image bytes with no structure around them.
//!
//! The width/height a FLAC block *declares* are written by whichever tagger
//! made the file and are routinely wrong (or zero), so the image header itself
//! is also sniffed (PNG/JPEG/GIF/WebP/BMP/TIFF) and both numbers are reported.
//!
//! Pure Rust, no I/O and no std clock, so the one implementation serves the chat
//! block and the CLI alike.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;

/// Stop collecting after this many pictures. A hand-crafted file can declare
/// thousands; the report has to stay readable (and fit a chat context).
const MAX_PICTURES: usize = 64;

/// Longest description kept verbatim in the report. Longer ones are elided.
const MAX_DESCRIPTION_CHARS: usize = 300;

// ---------------------------------------------------------------------------
// Picture types (the ID3v2 APIC table the FLAC spec reuses verbatim)
// ---------------------------------------------------------------------------

/// `(type number, spec name, kebab-case slug used by the `picture_type` param)`.
pub const PICTURE_TYPES: [(u32, &str, &str); 21] = [
    (0, "Other", "other"),
    (1, "32x32 pixels file icon (PNG only)", "file-icon"),
    (2, "Other file icon", "other-file-icon"),
    (3, "Cover (front)", "front-cover"),
    (4, "Cover (back)", "back-cover"),
    (5, "Leaflet page", "leaflet-page"),
    (6, "Media (e.g. label side of CD)", "media"),
    (7, "Lead artist / lead performer / soloist", "lead-artist"),
    (8, "Artist / performer", "artist"),
    (9, "Conductor", "conductor"),
    (10, "Band / orchestra", "band"),
    (11, "Composer", "composer"),
    (12, "Lyricist / text writer", "lyricist"),
    (13, "Recording location", "recording-location"),
    (14, "During recording", "during-recording"),
    (15, "During performance", "during-performance"),
    (16, "Movie / video screen capture", "video-screen-capture"),
    (17, "A bright coloured fish", "bright-colored-fish"),
    (18, "Illustration", "illustration"),
    (19, "Band / artist logotype", "band-logo"),
    (20, "Publisher / studio logotype", "publisher-logo"),
];

/// Spec name for a picture-type number, or `"Unknown picture type"` for the
/// out-of-range values a broken tagger can write.
pub fn picture_type_name(n: u32) -> &'static str {
    PICTURE_TYPES
        .iter()
        .find(|(v, _, _)| *v == n)
        .map(|(_, name, _)| *name)
        .unwrap_or("Unknown picture type")
}

/// Kebab-case slug for a picture-type number, or `"unknown"`.
pub fn picture_type_slug(n: u32) -> &'static str {
    PICTURE_TYPES
        .iter()
        .find(|(v, _, _)| *v == n)
        .map(|(_, _, slug)| *slug)
        .unwrap_or("unknown")
}

/// Every value the `picture_type` selector accepts: `any` plus the 21 slugs.
pub fn picture_type_filter_values() -> Vec<&'static str> {
    let mut v = vec!["any"];
    v.extend(PICTURE_TYPES.iter().map(|(_, _, slug)| *slug));
    v
}

// ---------------------------------------------------------------------------
// Report types
// ---------------------------------------------------------------------------

/// Where in the file a picture was found. FLAC allows all three; which one a
/// file uses depends entirely on the tagger that wrote it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PictureSource {
    /// A native `PICTURE` metadata block (block type 6). The normal case.
    MetadataBlock,
    /// A base64 `METADATA_BLOCK_PICTURE` field inside `VORBIS_COMMENT`.
    VorbisComment,
    /// The deprecated `COVERART` / `COVERARTMIME` Vorbis field pair.
    VorbisCommentLegacy,
}

impl PictureSource {
    pub fn label(self) -> &'static str {
        match self {
            PictureSource::MetadataBlock => "PICTURE metadata block",
            PictureSource::VorbisComment => "VORBIS_COMMENT METADATA_BLOCK_PICTURE field",
            PictureSource::VorbisCommentLegacy => "VORBIS_COMMENT COVERART field (deprecated)",
        }
    }
}

/// One embedded picture, with the fields the file declares and the fields read
/// back out of the image's own header.
#[derive(Debug, Clone)]
pub struct Picture {
    /// 1-based position among all pictures found, in file order.
    pub index: usize,
    pub source: PictureSource,
    pub picture_type: u32,
    pub picture_type_name: &'static str,
    pub picture_type_slug: &'static str,
    /// MIME string as stored. The literal `-->` means `data` holds a URL.
    pub mime: String,
    pub description: String,
    /// `true` when `mime` is `-->`, i.e. the payload is a link, not an image.
    pub is_url: bool,
    pub declared_width: u32,
    pub declared_height: u32,
    /// Colour depth in bits per pixel, as declared.
    pub declared_depth: u32,
    /// Number of colours for an indexed image; 0 for non-indexed.
    pub declared_colors: u32,
    /// Image format read out of the payload's own header, e.g. `"PNG"`.
    pub detected_format: Option<&'static str>,
    pub detected_width: Option<u32>,
    pub detected_height: Option<u32>,
    /// The picture payload, byte for byte as stored.
    pub data: Vec<u8>,
}

impl Picture {
    /// Best available width: the image header's, falling back to the declared
    /// value.
    pub fn width(&self) -> u32 {
        self.detected_width.unwrap_or(self.declared_width)
    }

    /// Best available height: the image header's, falling back to the declared
    /// value.
    pub fn height(&self) -> u32 {
        self.detected_height.unwrap_or(self.declared_height)
    }

    /// File extension for the payload, from the MIME string, falling back to
    /// the sniffed format, falling back to `bin`.
    pub fn ext(&self) -> &'static str {
        match self.mime.trim().to_ascii_lowercase().as_str() {
            "image/png" => return "png",
            "image/jpeg" | "image/jpg" | "image/pjpeg" => return "jpg",
            "image/gif" => return "gif",
            "image/webp" => return "webp",
            "image/bmp" | "image/x-ms-bmp" | "image/x-bmp" => return "bmp",
            "image/tiff" | "image/x-tiff" => return "tiff",
            "image/svg+xml" => return "svg",
            _ => {}
        }
        match self.detected_format {
            Some("PNG") => "png",
            Some("JPEG") => "jpg",
            Some("GIF") => "gif",
            Some("WebP") => "webp",
            Some("BMP") => "bmp",
            Some("TIFF") => "tiff",
            _ => "bin",
        }
    }

    /// A stable, filesystem-safe download name, e.g. `front-cover.jpg`.
    pub fn filename(&self) -> String {
        format!("{}.{}", self.picture_type_slug, self.ext())
    }

    /// The MIME to hand back with the bytes. Falls back to the sniffed format
    /// when the stored MIME is missing or not an `image/*` type.
    pub fn output_mime(&self) -> String {
        let m = self.mime.trim();
        if m.to_ascii_lowercase().starts_with("image/") {
            return m.to_ascii_lowercase();
        }
        match self.detected_format {
            Some("PNG") => "image/png".to_string(),
            Some("JPEG") => "image/jpeg".to_string(),
            Some("GIF") => "image/gif".to_string(),
            Some("WebP") => "image/webp".to_string(),
            Some("BMP") => "image/bmp".to_string(),
            Some("TIFF") => "image/tiff".to_string(),
            _ => "application/octet-stream".to_string(),
        }
    }

    /// One line for the "every picture in this file" inventory.
    pub fn inventory_line(&self) -> String {
        let dims = if self.width() > 0 && self.height() > 0 {
            format!("{}x{}", self.width(), self.height())
        } else {
            "dimensions unknown".to_string()
        };
        format!(
            "{}. {} — {} — {} — {} bytes — {}",
            self.index,
            self.picture_type_name,
            if self.mime.is_empty() {
                "(no MIME)"
            } else {
                self.mime.as_str()
            },
            dims,
            self.data.len(),
            self.source.label()
        )
    }
}

/// The handful of STREAMINFO fields worth showing next to the artwork.
#[derive(Debug, Clone, PartialEq)]
pub struct StreamInfo {
    pub sample_rate: u32,
    pub channels: u32,
    pub bits_per_sample: u32,
    pub total_samples: u64,
    /// `None` when the encoder wrote 0 total samples (streamed input).
    pub duration_seconds: Option<f64>,
}

/// Everything read out of one FLAC file.
#[derive(Debug, Clone)]
pub struct FlacReport {
    pub stream_info: Option<StreamInfo>,
    /// Metadata block names in file order, e.g. `["STREAMINFO", "PICTURE"]`.
    pub metadata_blocks: Vec<String>,
    pub pictures: Vec<Picture>,
    /// Bytes of ID3v2 tag skipped before the `fLaC` marker (0 for a clean file).
    pub id3v2_prefix_bytes: usize,
    /// Non-fatal observations: declared/actual mismatches, truncation, elisions.
    pub notes: Vec<String>,
}

impl FlacReport {
    /// Pictures matching a `picture_type` selector value (`any` or a slug).
    pub fn matching(&self, type_filter: &str) -> Vec<&Picture> {
        if type_filter == "any" {
            return self.pictures.iter().collect();
        }
        self.pictures
            .iter()
            .filter(|p| p.picture_type_slug == type_filter)
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Selection
// ---------------------------------------------------------------------------

/// Normalise + validate a `picture_type` selector value. Accepts `any`, any of
/// the 21 slugs, and a bare type number (`"3"`), so an LLM that read the spec
/// table still gets a hit.
pub fn parse_type_filter(s: &str) -> Result<String, String> {
    let t = s.trim().to_ascii_lowercase().replace('_', "-");
    if t.is_empty() || t == "any" {
        return Ok("any".to_string());
    }
    if let Ok(n) = t.parse::<u32>() {
        if let Some((_, _, slug)) = PICTURE_TYPES.iter().find(|(v, _, _)| *v == n) {
            return Ok((*slug).to_string());
        }
        return Err(format!(
            "picture_type {n} is not one of the 0-20 picture types defined by the format"
        ));
    }
    if PICTURE_TYPES.iter().any(|(_, _, slug)| *slug == t) {
        return Ok(t);
    }
    Err(format!(
        "unknown picture_type {s:?}; expected \"any\" or one of: {}",
        PICTURE_TYPES
            .iter()
            .map(|(_, _, slug)| *slug)
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

/// Pick one picture: filter by type, then take the 1-based `index` within what
/// is left. Errors explain exactly what the file does contain.
pub fn select<'a>(
    report: &'a FlacReport,
    type_filter: &str,
    index: usize,
) -> Result<&'a Picture, String> {
    if index == 0 {
        return Err("picture_index is 1-based; the first picture is 1".to_string());
    }
    if report.pictures.is_empty() {
        return Err(format!(
            "this FLAC file has no embedded artwork: none of its {} metadata block(s) ({}) \
             carries a picture. Artwork stored in an MP3 ID3v2 APIC frame or an MP4 covr atom \
             lives in a different container and is not read by this tool.",
            report.metadata_blocks.len(),
            report.metadata_blocks.join(", ")
        ));
    }
    let matches = report.matching(type_filter);
    if matches.is_empty() {
        let available: Vec<String> = report
            .pictures
            .iter()
            .map(|p| format!("{} ({})", p.picture_type_slug, p.picture_type_name))
            .collect();
        return Err(format!(
            "no picture of type {:?} in this file; it contains: {}",
            type_filter,
            available.join(", ")
        ));
    }
    matches.get(index - 1).copied().ok_or_else(|| {
        if type_filter == "any" {
            format!(
                "picture_index {index} is out of range: this file has {} embedded picture(s)",
                matches.len()
            )
        } else {
            format!(
                "picture_index {index} is out of range: this file has {} picture(s) of type {:?}",
                matches.len(),
                type_filter
            )
        }
    })
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

struct Reader<'a> {
    b: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(b: &'a [u8]) -> Self {
        Reader { b, pos: 0 }
    }
    fn remaining(&self) -> usize {
        self.b.len().saturating_sub(self.pos)
    }
    fn take(&mut self, n: usize, what: &str) -> Result<&'a [u8], String> {
        if self.remaining() < n {
            return Err(format!(
                "truncated {what}: needed {n} more bytes, {} left",
                self.remaining()
            ));
        }
        let s = &self.b[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }
    fn u32be(&mut self, what: &str) -> Result<u32, String> {
        let s = self.take(4, what)?;
        Ok(u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
    }
    fn u32le(&mut self, what: &str) -> Result<u32, String> {
        let s = self.take(4, what)?;
        Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }
}

/// Length of a leading ID3v2 tag, or 0. Some taggers glue an ID3v2 tag in front
/// of the `fLaC` marker even though the format has no place for one; every real
/// FLAC reader skips it, so this one does too.
fn id3v2_prefix_len(bytes: &[u8]) -> usize {
    if bytes.len() < 10 || &bytes[0..3] != b"ID3" {
        return 0;
    }
    let flags = bytes[5];
    // The size is 4 sync-safe bytes: 7 significant bits each.
    let size = ((bytes[6] as usize & 0x7f) << 21)
        | ((bytes[7] as usize & 0x7f) << 14)
        | ((bytes[8] as usize & 0x7f) << 7)
        | (bytes[9] as usize & 0x7f);
    let footer = if flags & 0x10 != 0 { 10 } else { 0 };
    10 + size + footer
}

fn block_type_name(t: u8) -> String {
    match t {
        0 => "STREAMINFO".to_string(),
        1 => "PADDING".to_string(),
        2 => "APPLICATION".to_string(),
        3 => "SEEKTABLE".to_string(),
        4 => "VORBIS_COMMENT".to_string(),
        5 => "CUESHEET".to_string(),
        6 => "PICTURE".to_string(),
        127 => "INVALID (127)".to_string(),
        other => format!("RESERVED ({other})"),
    }
}

fn parse_stream_info(payload: &[u8]) -> Option<StreamInfo> {
    if payload.len() < 18 {
        return None;
    }
    // Bytes 10..18 pack: 20 bits sample rate, 3 bits channels-1,
    // 5 bits bits-per-sample-1, 36 bits total samples.
    let mut v: u64 = 0;
    for b in &payload[10..18] {
        v = (v << 8) | *b as u64;
    }
    let sample_rate = ((v >> 44) & 0xF_FFFF) as u32;
    let channels = (((v >> 41) & 0x7) as u32) + 1;
    let bits_per_sample = (((v >> 36) & 0x1F) as u32) + 1;
    let total_samples = v & 0xF_FFFF_FFFF;
    let duration_seconds = if total_samples > 0 && sample_rate > 0 {
        Some(total_samples as f64 / sample_rate as f64)
    } else {
        None
    };
    Some(StreamInfo {
        sample_rate,
        channels,
        bits_per_sample,
        total_samples,
        duration_seconds,
    })
}

/// Parse a `METADATA_BLOCK_PICTURE` payload — the body of a native PICTURE
/// block, and also (after base64-decoding) of the Vorbis-comment field of the
/// same name.
fn parse_picture_payload(
    payload: &[u8],
    source: PictureSource,
    notes: &mut Vec<String>,
) -> Result<Picture, String> {
    let mut r = Reader::new(payload);
    let picture_type = r.u32be("picture type")?;

    let mime_len = r.u32be("MIME length")? as usize;
    let mime_bytes = r.take(mime_len, "MIME string")?;
    let mime = String::from_utf8_lossy(mime_bytes).trim().to_string();

    let desc_len = r.u32be("description length")? as usize;
    let desc_bytes = r.take(desc_len, "description")?;
    let mut description = String::from_utf8_lossy(desc_bytes).to_string();
    if description.chars().count() > MAX_DESCRIPTION_CHARS {
        description = description.chars().take(MAX_DESCRIPTION_CHARS).collect();
        description.push('…');
        notes.push("picture description was longer than 300 characters and has been shortened in this report".to_string());
    }

    let declared_width = r.u32be("width")?;
    let declared_height = r.u32be("height")?;
    let declared_depth = r.u32be("colour depth")?;
    let declared_colors = r.u32be("indexed colour count")?;

    let data_len = r.u32be("payload length")? as usize;
    let available = r.remaining();
    let data = if data_len > available {
        notes.push(format!(
            "picture declares a {data_len}-byte payload but only {available} bytes are present; \
             the truncated payload is returned as-is"
        ));
        r.take(available, "picture payload")?.to_vec()
    } else {
        r.take(data_len, "picture payload")?.to_vec()
    };

    let is_url = mime == "-->";
    let (detected_format, detected_width, detected_height) = if is_url {
        (None, None, None)
    } else {
        sniff_image(&data)
    };

    Ok(Picture {
        index: 0, // assigned by the caller once the file order is known
        source,
        picture_type,
        picture_type_name: picture_type_name(picture_type),
        picture_type_slug: picture_type_slug(picture_type),
        mime,
        description,
        is_url,
        declared_width,
        declared_height,
        declared_depth,
        declared_colors,
        detected_format,
        detected_width,
        detected_height,
        data,
    })
}

/// Pull pictures out of a VORBIS_COMMENT payload: the base64
/// `METADATA_BLOCK_PICTURE` field and the deprecated `COVERART` pair.
fn parse_vorbis_comment_pictures(
    payload: &[u8],
    notes: &mut Vec<String>,
) -> Result<Vec<Picture>, String> {
    let mut r = Reader::new(payload);
    // Vorbis comments are little-endian, unlike everything else in FLAC.
    let vendor_len = r.u32le("vendor length")? as usize;
    r.take(vendor_len, "vendor string")?;
    let count = r.u32le("comment count")? as usize;

    let mut out = Vec::new();
    let mut legacy_data: Option<Vec<u8>> = None;
    let mut legacy_mime: Option<String> = None;

    for _ in 0..count {
        if r.remaining() < 4 {
            notes.push("VORBIS_COMMENT block ends before its declared comment count".to_string());
            break;
        }
        let len = r.u32le("comment length")? as usize;
        if r.remaining() < len {
            notes.push("VORBIS_COMMENT block ends mid-field".to_string());
            break;
        }
        let field = r.take(len, "comment")?;
        let Some(eq) = field.iter().position(|b| *b == b'=') else {
            continue;
        };
        let name = String::from_utf8_lossy(&field[..eq]).to_ascii_uppercase();
        let value = &field[eq + 1..];
        match name.as_str() {
            "METADATA_BLOCK_PICTURE" => {
                let raw = match B64.decode(strip_ws(value)) {
                    Ok(v) => v,
                    Err(e) => {
                        notes.push(format!(
                            "a METADATA_BLOCK_PICTURE comment is not valid base64 and was skipped: {e}"
                        ));
                        continue;
                    }
                };
                match parse_picture_payload(&raw, PictureSource::VorbisComment, notes) {
                    Ok(p) => out.push(p),
                    Err(e) => notes.push(format!(
                        "a METADATA_BLOCK_PICTURE comment could not be parsed and was skipped: {e}"
                    )),
                }
            }
            "COVERART" => match B64.decode(strip_ws(value)) {
                Ok(v) => legacy_data = Some(v),
                Err(e) => notes.push(format!(
                    "the deprecated COVERART comment is not valid base64 and was skipped: {e}"
                )),
            },
            "COVERARTMIME" => {
                legacy_mime = Some(String::from_utf8_lossy(value).trim().to_string());
            }
            _ => {}
        }
        if out.len() >= MAX_PICTURES {
            break;
        }
    }

    if let Some(data) = legacy_data {
        let (detected_format, detected_width, detected_height) = sniff_image(&data);
        let mime = legacy_mime.unwrap_or_default();
        notes.push(
            "artwork was found in the deprecated COVERART comment field, which carries no picture \
             type or dimensions; it is reported as a front cover"
                .to_string(),
        );
        out.push(Picture {
            index: 0,
            source: PictureSource::VorbisCommentLegacy,
            picture_type: 3,
            picture_type_name: picture_type_name(3),
            picture_type_slug: picture_type_slug(3),
            mime,
            description: String::new(),
            is_url: false,
            declared_width: 0,
            declared_height: 0,
            declared_depth: 0,
            declared_colors: 0,
            detected_format,
            detected_width,
            detected_height,
            data,
        });
    }
    Ok(out)
}

/// Base64 in a Vorbis comment is sometimes line-wrapped; the decoder is not.
fn strip_ws(v: &[u8]) -> Vec<u8> {
    v.iter()
        .copied()
        .filter(|b| !b.is_ascii_whitespace())
        .collect()
}

/// Walk a FLAC file's metadata and return everything found.
pub fn parse(bytes: &[u8]) -> Result<FlacReport, String> {
    if bytes.is_empty() {
        return Err("input is empty — provide a FLAC file".to_string());
    }
    if bytes.len() >= 4 && &bytes[0..4] == b"OggS" {
        return Err(
            "this is an Ogg container, not a native FLAC stream. Ogg-encapsulated FLAC is not \
             supported here — re-save the track as a native .flac file first."
                .to_string(),
        );
    }

    let id3v2_prefix_bytes = id3v2_prefix_len(bytes);
    let mut notes: Vec<String> = Vec::new();
    if id3v2_prefix_bytes > 0 {
        notes.push(format!(
            "skipped a {id3v2_prefix_bytes}-byte ID3v2 tag in front of the FLAC marker (some \
             taggers write one; the format has no place for it)"
        ));
    }
    let body = bytes.get(id3v2_prefix_bytes..).unwrap_or(&[]);
    if body.len() < 4 || &body[0..4] != b"fLaC" {
        return Err(format!(
            "not a FLAC file: expected the ASCII marker \"fLaC\"{}, found {}",
            if id3v2_prefix_bytes > 0 {
                " after the ID3v2 tag"
            } else {
                " at the start of the file"
            },
            describe_magic(body)
        ));
    }

    let mut stream_info = None;
    let mut metadata_blocks: Vec<String> = Vec::new();
    let mut pictures: Vec<Picture> = Vec::new();
    let mut pos = 4usize;

    loop {
        if body.len() < pos + 4 {
            if !metadata_blocks.is_empty() {
                notes.push(
                    "the metadata block chain is truncated — the file ends mid-header".to_string(),
                );
            }
            break;
        }
        let header = body[pos];
        let is_last = header & 0x80 != 0;
        let btype = header & 0x7f;
        let len = ((body[pos + 1] as usize) << 16)
            | ((body[pos + 2] as usize) << 8)
            | (body[pos + 3] as usize);
        pos += 4;
        let end = pos.saturating_add(len);
        if end > body.len() {
            notes.push(format!(
                "the {} block declares {len} bytes but the file ends first; it was skipped",
                block_type_name(btype)
            ));
            metadata_blocks.push(block_type_name(btype));
            break;
        }
        let payload = &body[pos..end];
        metadata_blocks.push(block_type_name(btype));

        match btype {
            0 => stream_info = parse_stream_info(payload),
            4 => match parse_vorbis_comment_pictures(payload, &mut notes) {
                Ok(mut v) => pictures.append(&mut v),
                Err(e) => notes.push(format!("VORBIS_COMMENT block could not be read: {e}")),
            },
            6 => {
                if pictures.len() < MAX_PICTURES {
                    match parse_picture_payload(payload, PictureSource::MetadataBlock, &mut notes) {
                        Ok(p) => pictures.push(p),
                        Err(e) => {
                            notes.push(format!("a PICTURE block could not be parsed: {e}"));
                        }
                    }
                }
            }
            _ => {}
        }

        pos = end;
        if is_last {
            break;
        }
    }

    if pictures.len() >= MAX_PICTURES {
        notes.push(format!(
            "stopped after the first {MAX_PICTURES} pictures; any further ones were not read"
        ));
        pictures.truncate(MAX_PICTURES);
    }

    // File order is only known once every block has been walked.
    for (i, p) in pictures.iter_mut().enumerate() {
        p.index = i + 1;
        if let (Some(w), Some(h)) = (p.detected_width, p.detected_height) {
            if (p.declared_width != 0 || p.declared_height != 0)
                && (p.declared_width != w || p.declared_height != h)
            {
                notes.push(format!(
                    "picture {} declares {}x{} but the image header says {}x{}; the header wins",
                    i + 1,
                    p.declared_width,
                    p.declared_height,
                    w,
                    h
                ));
            }
        }
        if p.is_url {
            notes.push(format!(
                "picture {} uses the \"-->\" MIME form: its payload is a link to an image, not the \
                 image itself",
                i + 1
            ));
        }
    }

    Ok(FlacReport {
        stream_info,
        metadata_blocks,
        pictures,
        id3v2_prefix_bytes,
        notes,
    })
}

fn describe_magic(b: &[u8]) -> String {
    if b.len() >= 3 && &b[0..3] == b"ID3" {
        return "an ID3v2 tag with no FLAC stream after it".to_string();
    }
    if b.len() >= 3 && b[0] == 0xff && (b[1] & 0xe0) == 0xe0 {
        return "MPEG audio (an MP3 — its artwork is in an ID3v2 APIC frame)".to_string();
    }
    if b.len() >= 12 && &b[4..8] == b"ftyp" {
        return "an ISO base-media file (MP4/M4A — its artwork is in a covr atom)".to_string();
    }
    if b.len() >= 4 && &b[0..4] == b"RIFF" {
        return "a RIFF container (WAV/AVI)".to_string();
    }
    let head: Vec<String> = b.iter().take(4).map(|x| format!("{x:02x}")).collect();
    if head.is_empty() {
        "nothing".to_string()
    } else {
        format!("bytes {}", head.join(" "))
    }
}

// ---------------------------------------------------------------------------
// Image-header sniffing — the declared dimensions are written by the tagger and
// are routinely wrong or zero, so the payload's own header is the truth.
// ---------------------------------------------------------------------------

/// `(format name, width, height)` read from an image's header.
pub fn sniff_image(d: &[u8]) -> (Option<&'static str>, Option<u32>, Option<u32>) {
    if d.len() >= 24 && d.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]) {
        if &d[12..16] == b"IHDR" {
            return (
                Some("PNG"),
                Some(u32::from_be_bytes([d[16], d[17], d[18], d[19]])),
                Some(u32::from_be_bytes([d[20], d[21], d[22], d[23]])),
            );
        }
        return (Some("PNG"), None, None);
    }
    if d.len() >= 4 && d[0] == 0xff && d[1] == 0xd8 {
        let (w, h) = jpeg_dimensions(d);
        return (Some("JPEG"), w, h);
    }
    if d.len() >= 10 && (d.starts_with(b"GIF87a") || d.starts_with(b"GIF89a")) {
        return (
            Some("GIF"),
            Some(u16::from_le_bytes([d[6], d[7]]) as u32),
            Some(u16::from_le_bytes([d[8], d[9]]) as u32),
        );
    }
    if d.len() >= 12 && d.starts_with(b"RIFF") && &d[8..12] == b"WEBP" {
        let (w, h) = webp_dimensions(d);
        return (Some("WebP"), w, h);
    }
    if d.len() >= 26 && d.starts_with(b"BM") {
        let dib = u32::from_le_bytes([d[14], d[15], d[16], d[17]]);
        if dib == 12 {
            return (
                Some("BMP"),
                Some(u16::from_le_bytes([d[18], d[19]]) as u32),
                Some(u16::from_le_bytes([d[20], d[21]]) as u32),
            );
        }
        let w = i32::from_le_bytes([d[18], d[19], d[20], d[21]]);
        let h = i32::from_le_bytes([d[22], d[23], d[24], d[25]]);
        return (Some("BMP"), Some(w.unsigned_abs()), Some(h.unsigned_abs()));
    }
    if d.len() >= 4 && (d.starts_with(b"II\x2a\x00") || d.starts_with(b"MM\x00\x2a")) {
        // Reading TIFF dimensions means walking an IFD; the format is vanishingly
        // rare as cover art, so it is identified but not measured.
        return (Some("TIFF"), None, None);
    }
    (None, None, None)
}

/// Walk JPEG segments to the first start-of-frame marker, which carries the
/// real pixel dimensions.
fn jpeg_dimensions(d: &[u8]) -> (Option<u32>, Option<u32>) {
    let mut i = 2usize;
    while i + 1 < d.len() {
        if d[i] != 0xff {
            i += 1;
            continue;
        }
        // Fill bytes: any number of 0xff may precede the marker code.
        let mut m = i + 1;
        while m < d.len() && d[m] == 0xff {
            m += 1;
        }
        if m >= d.len() {
            break;
        }
        let marker = d[m];
        // Standalone markers carry no length field.
        if marker == 0x01 || (0xd0..=0xd9).contains(&marker) {
            i = m + 1;
            continue;
        }
        if m + 2 >= d.len() {
            break;
        }
        let seg_len = u16::from_be_bytes([d[m + 1], d[m + 2]]) as usize;
        // SOF0..SOF15, excluding DHT (0xc4), JPG (0xc8) and DAC (0xcc).
        let is_sof =
            (0xc0..=0xcf).contains(&marker) && marker != 0xc4 && marker != 0xc8 && marker != 0xcc;
        if is_sof {
            // Segment body: precision(1), height(2), width(2).
            if m + 8 < d.len() {
                return (
                    Some(u16::from_be_bytes([d[m + 6], d[m + 7]]) as u32),
                    Some(u16::from_be_bytes([d[m + 4], d[m + 5]]) as u32),
                );
            }
            return (None, None);
        }
        if seg_len < 2 {
            break;
        }
        i = m + 1 + seg_len;
    }
    (None, None)
}

fn webp_dimensions(d: &[u8]) -> (Option<u32>, Option<u32>) {
    if d.len() < 30 {
        return (None, None);
    }
    match &d[12..16] {
        b"VP8 " => {
            // Frame tag (3 bytes), start code 9d 01 2a, then 14-bit w/h.
            if d.len() >= 30 && d[23] == 0x9d && d[24] == 0x01 && d[25] == 0x2a {
                let w = u16::from_le_bytes([d[26], d[27]]) & 0x3fff;
                let h = u16::from_le_bytes([d[28], d[29]]) & 0x3fff;
                return (Some(w as u32), Some(h as u32));
            }
            (None, None)
        }
        b"VP8L" => {
            if d.len() >= 25 && d[20] == 0x2f {
                let bits = u32::from_le_bytes([d[21], d[22], d[23], d[24]]);
                let w = (bits & 0x3fff) + 1;
                let h = ((bits >> 14) & 0x3fff) + 1;
                return (Some(w), Some(h));
            }
            (None, None)
        }
        b"VP8X" => {
            // Flags(4), canvas width-1 (24-bit LE), canvas height-1 (24-bit LE).
            if d.len() >= 30 {
                let w = (d[24] as u32) | ((d[25] as u32) << 8) | ((d[26] as u32) << 16);
                let h = (d[27] as u32) | ((d[28] as u32) << 8) | ((d[29] as u32) << 16);
                return (Some(w + 1), Some(h + 1));
            }
            (None, None)
        }
        _ => (None, None),
    }
}

// ---------------------------------------------------------------------------
// Report text — the human/LLM-readable field dump that travels with the bytes
// ---------------------------------------------------------------------------

/// The full metadata dump for the selected picture, plus an inventory of every
/// other picture in the file. This is what chat and the CLI read alongside the
/// returned image bytes.
pub fn report_text(report: &FlacReport, selected: &Picture) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "Extracted picture {} of {} from a FLAC file.\n",
        selected.index,
        report.pictures.len()
    ));
    out.push_str(&format!(
        "picture_type: {} ({} — {})\n",
        selected.picture_type, selected.picture_type_slug, selected.picture_type_name
    ));
    out.push_str(&format!(
        "mime: {}\n",
        if selected.mime.is_empty() {
            "(none stored)"
        } else {
            selected.mime.as_str()
        }
    ));
    out.push_str(&format!(
        "description: {}\n",
        if selected.description.is_empty() {
            "(none)"
        } else {
            selected.description.as_str()
        }
    ));
    out.push_str(&format!(
        "declared_dimensions: {}x{}\n",
        selected.declared_width, selected.declared_height
    ));
    match (selected.detected_width, selected.detected_height) {
        (Some(w), Some(h)) => out.push_str(&format!(
            "actual_dimensions: {}x{} (read from the {} header)\n",
            w,
            h,
            selected.detected_format.unwrap_or("image")
        )),
        _ => out.push_str("actual_dimensions: not readable from the image header\n"),
    }
    out.push_str(&format!(
        "color_depth_bits_per_pixel: {}\n",
        selected.declared_depth
    ));
    out.push_str(&format!(
        "indexed_colors: {}{}\n",
        selected.declared_colors,
        if selected.declared_colors == 0 {
            " (not an indexed-colour image)"
        } else {
            ""
        }
    ));
    out.push_str(&format!(
        "detected_format: {}\n",
        selected.detected_format.unwrap_or("unrecognised")
    ));
    out.push_str(&format!("bytes: {}\n", selected.data.len()));
    out.push_str(&format!("stored_in: {}\n", selected.source.label()));

    if let Some(si) = &report.stream_info {
        let dur = si
            .duration_seconds
            .map(|d| format!("{d:.2} s"))
            .unwrap_or_else(|| "unknown".to_string());
        out.push_str(&format!(
            "audio: {} Hz, {} channel(s), {}-bit, {}\n",
            si.sample_rate, si.channels, si.bits_per_sample, dur
        ));
    }
    out.push_str(&format!(
        "metadata_blocks: {}\n",
        report.metadata_blocks.join(", ")
    ));

    if report.pictures.len() > 1 {
        out.push_str("all pictures in this file:\n");
        for p in &report.pictures {
            out.push_str(&format!("  {}\n", p.inventory_line()));
        }
    }
    if !report.notes.is_empty() {
        out.push_str("notes:\n");
        for n in &report.notes {
            out.push_str(&format!("  - {n}\n"));
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// A 1x1 red PNG, byte for byte.
    pub(crate) fn png_1x1() -> Vec<u8> {
        vec![
            0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, // signature
            0x00, 0x00, 0x00, 0x0d, b'I', b'H', b'D', b'R', // IHDR length + type
            0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, // 1x1
            0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53, 0xde, // depth/colour + crc
            0x00, 0x00, 0x00, 0x0c, b'I', b'D', b'A', b'T', 0x08, 0xd7, 0x63, 0xf8, 0xcf, 0xc0,
            0x00, 0x00, 0x03, 0x01, 0x01, 0x00, 0x18, 0xdd, 0x8d, 0xb0, // IDAT + crc
            0x00, 0x00, 0x00, 0x00, b'I', b'E', b'N', b'D', 0xae, 0x42, 0x60, 0x82,
        ]
    }

    /// A minimal 4x3 baseline JPEG header (SOI + SOF0), enough to sniff.
    pub(crate) fn jpeg_stub_4x3() -> Vec<u8> {
        let mut v = vec![0xff, 0xd8]; // SOI
        v.extend_from_slice(&[0xff, 0xe0, 0x00, 0x04, 0x00, 0x00]); // tiny APP0
        v.extend_from_slice(&[0xff, 0xc0, 0x00, 0x0b, 0x08]); // SOF0, len 11, precision 8
        v.extend_from_slice(&[0x00, 0x03]); // height 3
        v.extend_from_slice(&[0x00, 0x04]); // width 4
        v.extend_from_slice(&[0x01, 0x01, 0x11, 0x00]); // 1 component
        v.extend_from_slice(&[0xff, 0xd9]); // EOI
        v
    }

    pub(crate) fn picture_payload(
        ptype: u32,
        mime: &str,
        desc: &str,
        w: u32,
        h: u32,
        depth: u32,
        colors: u32,
        data: &[u8],
    ) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&ptype.to_be_bytes());
        v.extend_from_slice(&(mime.len() as u32).to_be_bytes());
        v.extend_from_slice(mime.as_bytes());
        v.extend_from_slice(&(desc.len() as u32).to_be_bytes());
        v.extend_from_slice(desc.as_bytes());
        v.extend_from_slice(&w.to_be_bytes());
        v.extend_from_slice(&h.to_be_bytes());
        v.extend_from_slice(&depth.to_be_bytes());
        v.extend_from_slice(&colors.to_be_bytes());
        v.extend_from_slice(&(data.len() as u32).to_be_bytes());
        v.extend_from_slice(data);
        v
    }

    pub(crate) fn stream_info_payload() -> Vec<u8> {
        let mut v = vec![0u8; 34];
        v[0..2].copy_from_slice(&4096u16.to_be_bytes()); // min block size
        v[2..4].copy_from_slice(&4096u16.to_be_bytes()); // max block size
                                                         // 44100 Hz, 2 channels, 16-bit, 44100 total samples (1.0 s).
        let packed: u64 = (44100u64 << 44) | (1u64 << 41) | (15u64 << 36) | 44100u64;
        v[10..18].copy_from_slice(&packed.to_be_bytes());
        v
    }

    pub(crate) fn block(is_last: bool, btype: u8, payload: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        v.push(if is_last { 0x80 | btype } else { btype });
        let len = payload.len();
        v.push(((len >> 16) & 0xff) as u8);
        v.push(((len >> 8) & 0xff) as u8);
        v.push((len & 0xff) as u8);
        v.extend_from_slice(payload);
        v
    }

    /// A whole FLAC file: marker, STREAMINFO, the given extra blocks, and a
    /// single frame-ish tail so the file is not just metadata.
    pub(crate) fn flac_file(extra: Vec<(u8, Vec<u8>)>) -> Vec<u8> {
        let mut v = b"fLaC".to_vec();
        let last_is_streaminfo = extra.is_empty();
        v.extend_from_slice(&block(last_is_streaminfo, 0, &stream_info_payload()));
        for (i, (btype, payload)) in extra.iter().enumerate() {
            v.extend_from_slice(&block(i + 1 == extra.len(), *btype, payload));
        }
        v.extend_from_slice(&[0xff, 0xf8, 0x00, 0x00]); // start of an audio frame
        v
    }

    pub(crate) fn vorbis_comment_payload(fields: &[(&str, &str)]) -> Vec<u8> {
        let mut v = Vec::new();
        let vendor = b"gizza-test";
        v.extend_from_slice(&(vendor.len() as u32).to_le_bytes());
        v.extend_from_slice(vendor);
        v.extend_from_slice(&(fields.len() as u32).to_le_bytes());
        for (k, val) in fields {
            let f = format!("{k}={val}");
            v.extend_from_slice(&(f.len() as u32).to_le_bytes());
            v.extend_from_slice(f.as_bytes());
        }
        v
    }

    #[test]
    fn extracts_a_front_cover_png_with_every_declared_field() {
        let png = png_1x1();
        let payload = picture_payload(3, "image/png", "Front cover art", 1, 1, 24, 0, &png);
        let file = flac_file(vec![(6, payload)]);

        let report = parse(&file).unwrap();
        assert_eq!(report.metadata_blocks, vec!["STREAMINFO", "PICTURE"]);
        assert_eq!(report.pictures.len(), 1);

        let p = select(&report, "any", 1).unwrap();
        assert_eq!(p.index, 1);
        assert_eq!(p.source, PictureSource::MetadataBlock);
        assert_eq!(p.picture_type, 3);
        assert_eq!(p.picture_type_name, "Cover (front)");
        assert_eq!(p.picture_type_slug, "front-cover");
        assert_eq!(p.mime, "image/png");
        assert_eq!(p.description, "Front cover art");
        assert_eq!((p.declared_width, p.declared_height), (1, 1));
        assert_eq!(p.declared_depth, 24);
        assert_eq!(p.declared_colors, 0);
        assert_eq!(p.detected_format, Some("PNG"));
        assert_eq!((p.detected_width, p.detected_height), (Some(1), Some(1)));
        assert_eq!(p.data, png, "payload bytes come back byte-for-byte");
        assert_eq!(p.filename(), "front-cover.png");
        assert_eq!(p.output_mime(), "image/png");

        let si = report.stream_info.as_ref().unwrap();
        assert_eq!(si.sample_rate, 44100);
        assert_eq!(si.channels, 2);
        assert_eq!(si.bits_per_sample, 16);
        assert_eq!(si.duration_seconds, Some(1.0));
    }

    #[test]
    fn selects_by_index_and_by_picture_type() {
        let png = png_1x1();
        let jpg = jpeg_stub_4x3();
        let front = picture_payload(3, "image/png", "front", 1, 1, 24, 0, &png);
        let back = picture_payload(4, "image/jpeg", "back", 4, 3, 24, 0, &jpg);
        let artist = picture_payload(8, "image/jpeg", "artist", 4, 3, 24, 0, &jpg);
        let file = flac_file(vec![(6, front), (6, back), (6, artist)]);
        let report = parse(&file).unwrap();
        assert_eq!(report.pictures.len(), 3);

        assert_eq!(select(&report, "any", 2).unwrap().picture_type, 4);
        assert_eq!(
            select(&report, "back-cover", 1).unwrap().description,
            "back"
        );
        assert_eq!(select(&report, "artist", 1).unwrap().picture_type, 8);
        assert_eq!(
            select(&report, "artist", 1).unwrap().detected_width,
            Some(4)
        );
        assert_eq!(
            select(&report, "artist", 1).unwrap().filename(),
            "artist.jpg"
        );

        let err = select(&report, "any", 4).unwrap_err();
        assert!(err.contains("out of range"), "{err}");
        let err = select(&report, "conductor", 1).unwrap_err();
        assert!(err.contains("no picture of type"), "{err}");
        assert!(err.contains("front-cover"), "{err}");
    }

    #[test]
    fn reads_a_picture_stored_base64_in_the_vorbis_comment() {
        let png = png_1x1();
        let payload = picture_payload(3, "image/png", "", 0, 0, 0, 0, &png);
        let b64 = B64.encode(&payload);
        let vc = vorbis_comment_payload(&[("ARTIST", "Nobody"), ("METADATA_BLOCK_PICTURE", &b64)]);
        let file = flac_file(vec![(4, vc)]);

        let report = parse(&file).unwrap();
        assert_eq!(report.pictures.len(), 1);
        let p = select(&report, "front-cover", 1).unwrap();
        assert_eq!(p.source, PictureSource::VorbisComment);
        assert_eq!(p.data, png);
        // Declared dimensions are zero here; the header still gives the truth.
        assert_eq!((p.declared_width, p.declared_height), (0, 0));
        assert_eq!((p.width(), p.height()), (1, 1));
    }

    #[test]
    fn reads_the_deprecated_coverart_field_pair() {
        let png = png_1x1();
        let vc = vorbis_comment_payload(&[
            ("COVERARTMIME", "image/png"),
            ("COVERART", &B64.encode(&png)),
        ]);
        let file = flac_file(vec![(4, vc)]);

        let report = parse(&file).unwrap();
        let p = select(&report, "any", 1).unwrap();
        assert_eq!(p.source, PictureSource::VorbisCommentLegacy);
        assert_eq!(p.mime, "image/png");
        assert_eq!(p.picture_type, 3);
        assert_eq!(p.data, png);
        assert!(report
            .notes
            .iter()
            .any(|n| n.contains("deprecated COVERART")));
    }

    #[test]
    fn flags_declared_dimensions_that_disagree_with_the_image_header() {
        let png = png_1x1();
        let payload = picture_payload(3, "image/png", "", 500, 500, 24, 0, &png);
        let file = flac_file(vec![(6, payload)]);
        let report = parse(&file).unwrap();
        let p = select(&report, "any", 1).unwrap();
        assert_eq!((p.declared_width, p.declared_height), (500, 500));
        assert_eq!((p.width(), p.height()), (1, 1));
        assert!(
            report
                .notes
                .iter()
                .any(|n| n.contains("declares 500x500") && n.contains("1x1")),
            "{:?}",
            report.notes
        );
    }

    #[test]
    fn reports_the_url_mime_form_instead_of_returning_bogus_bytes() {
        let url = b"https://example.com/cover.jpg";
        let payload = picture_payload(3, "-->", "", 0, 0, 0, 0, url);
        let file = flac_file(vec![(6, payload)]);
        let report = parse(&file).unwrap();
        let p = select(&report, "any", 1).unwrap();
        assert!(p.is_url);
        assert_eq!(p.detected_format, None);
        assert_eq!(p.data, url);
        assert!(report.notes.iter().any(|n| n.contains("\"-->\"")));
    }

    #[test]
    fn skips_an_id3v2_tag_glued_in_front_of_the_flac_marker() {
        let png = png_1x1();
        let payload = picture_payload(3, "image/png", "", 1, 1, 24, 0, &png);
        let inner = flac_file(vec![(6, payload)]);
        let mut file = b"ID3\x04\x00\x00\x00\x00\x00\x0a".to_vec(); // 10-byte body
        file.extend_from_slice(&[0u8; 10]);
        file.extend_from_slice(&inner);

        let report = parse(&file).unwrap();
        assert_eq!(report.id3v2_prefix_bytes, 20);
        assert_eq!(select(&report, "any", 1).unwrap().data, png);
    }

    #[test]
    fn a_flac_with_no_picture_errors_with_what_it_does_contain() {
        let file = flac_file(vec![(1, vec![0u8; 16])]); // STREAMINFO + PADDING
        let report = parse(&file).unwrap();
        assert!(report.pictures.is_empty());
        let err = select(&report, "any", 1).unwrap_err();
        assert!(err.contains("no embedded artwork"), "{err}");
        assert!(err.contains("STREAMINFO"), "{err}");
        assert!(err.contains("PADDING"), "{err}");
    }

    #[test]
    fn a_non_flac_input_names_the_container_it_actually_got() {
        let err = parse(b"OggS\x00\x02rest of an ogg file").unwrap_err();
        assert!(err.contains("Ogg container"), "{err}");

        let mut mp4 = vec![0u8, 0, 0, 0x18];
        mp4.extend_from_slice(b"ftypM4A ");
        mp4.extend_from_slice(&[0u8; 8]);
        let err = parse(&mp4).unwrap_err();
        assert!(err.contains("not a FLAC file"), "{err}");
        assert!(err.contains("covr atom"), "{err}");

        let err = parse(b"").unwrap_err();
        assert!(err.contains("empty"), "{err}");
    }

    #[test]
    fn a_truncated_picture_block_is_reported_not_panicked_on() {
        let png = png_1x1();
        let mut payload = picture_payload(3, "image/png", "", 1, 1, 24, 0, &png);
        payload.truncate(payload.len() - 10); // chop the tail off the payload
        let file = flac_file(vec![(6, payload)]);
        let report = parse(&file).unwrap();
        assert_eq!(report.pictures.len(), 1);
        assert!(
            report.notes.iter().any(|n| n.contains("truncated payload")),
            "{:?}",
            report.notes
        );
        assert_eq!(
            select(&report, "any", 1).unwrap().data.len(),
            png.len() - 10
        );
    }

    #[test]
    fn type_filter_accepts_slugs_numbers_and_rejects_typos() {
        assert_eq!(parse_type_filter("any").unwrap(), "any");
        assert_eq!(parse_type_filter("").unwrap(), "any");
        assert_eq!(parse_type_filter("FRONT_COVER").unwrap(), "front-cover");
        assert_eq!(parse_type_filter("3").unwrap(), "front-cover");
        assert_eq!(parse_type_filter("20").unwrap(), "publisher-logo");
        assert!(parse_type_filter("99").unwrap_err().contains("0-20"));
        assert!(parse_type_filter("cover").unwrap_err().contains("unknown"));
        assert_eq!(picture_type_filter_values().len(), 22);
    }

    #[test]
    fn index_zero_is_rejected_because_indexes_are_one_based() {
        let png = png_1x1();
        let payload = picture_payload(3, "image/png", "", 1, 1, 24, 0, &png);
        let report = parse(&flac_file(vec![(6, payload)])).unwrap();
        assert!(select(&report, "any", 0).unwrap_err().contains("1-based"));
    }

    #[test]
    fn sniffs_every_supported_image_header() {
        assert_eq!(sniff_image(&png_1x1()), (Some("PNG"), Some(1), Some(1)));
        assert_eq!(
            sniff_image(&jpeg_stub_4x3()),
            (Some("JPEG"), Some(4), Some(3))
        );

        let mut gif = b"GIF89a".to_vec();
        gif.extend_from_slice(&[0x0a, 0x00, 0x14, 0x00]); // 10x20
        gif.extend_from_slice(&[0u8; 8]);
        assert_eq!(sniff_image(&gif), (Some("GIF"), Some(10), Some(20)));

        let mut bmp = b"BM".to_vec();
        bmp.extend_from_slice(&[0u8; 12]);
        bmp.extend_from_slice(&40u32.to_le_bytes()); // BITMAPINFOHEADER
        bmp.extend_from_slice(&7i32.to_le_bytes()); // width 7
        bmp.extend_from_slice(&(-9i32).to_le_bytes()); // top-down height 9
        bmp.extend_from_slice(&[0u8; 8]);
        assert_eq!(sniff_image(&bmp), (Some("BMP"), Some(7), Some(9)));

        let mut webp = b"RIFF".to_vec();
        webp.extend_from_slice(&0u32.to_le_bytes());
        webp.extend_from_slice(b"WEBPVP8X");
        webp.extend_from_slice(&10u32.to_le_bytes()); // chunk size
        webp.extend_from_slice(&[0, 0, 0, 0]); // flags
        webp.extend_from_slice(&[0x3f, 0x00, 0x00]); // canvas width-1 = 63
        webp.extend_from_slice(&[0x1f, 0x00, 0x00]); // canvas height-1 = 31
        assert_eq!(sniff_image(&webp), (Some("WebP"), Some(64), Some(32)));

        assert_eq!(sniff_image(b"not an image at all"), (None, None, None));
    }

    #[test]
    fn report_text_carries_every_field_and_the_full_inventory() {
        let png = png_1x1();
        let jpg = jpeg_stub_4x3();
        let front = picture_payload(3, "image/png", "Sleeve front", 1, 1, 24, 0, &png);
        let back = picture_payload(4, "image/jpeg", "", 4, 3, 24, 0, &jpg);
        let report = parse(&flac_file(vec![(6, front), (6, back)])).unwrap();
        let p = select(&report, "any", 1).unwrap();
        let text = report_text(&report, p);

        assert!(
            text.contains("picture_type: 3 (front-cover — Cover (front))"),
            "{text}"
        );
        assert!(text.contains("mime: image/png"), "{text}");
        assert!(text.contains("description: Sleeve front"), "{text}");
        assert!(text.contains("declared_dimensions: 1x1"), "{text}");
        assert!(
            text.contains("actual_dimensions: 1x1 (read from the PNG header)"),
            "{text}"
        );
        assert!(text.contains("color_depth_bits_per_pixel: 24"), "{text}");
        assert!(
            text.contains("indexed_colors: 0 (not an indexed-colour image)"),
            "{text}"
        );
        assert!(text.contains(&format!("bytes: {}", png.len())), "{text}");
        assert!(
            text.contains("audio: 44100 Hz, 2 channel(s), 16-bit, 1.00 s"),
            "{text}"
        );
        assert!(text.contains("all pictures in this file:"), "{text}");
        assert!(
            text.contains("2. Cover (back) — image/jpeg — 4x3"),
            "{text}"
        );
    }
}
