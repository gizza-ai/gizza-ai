//! jpg-stego-embed core — conceal an arbitrary file inside a JPEG while leaving
//! the picture byte-for-byte intact.
//!
//! Every pixel-based steganography approach (LSB, DCT) has to re-encode or is
//! destroyed by JPEG's lossy compression — which is why the browser LSB tools all
//! spit out a PNG. This block takes the other route: the JPEG's entropy-coded
//! image data is copied verbatim and the payload rides along in *container*
//! space, so the output is the same picture, pixel-identical, still a JPEG.
//!
//! Three placements (`Method`):
//!   * `App`     — chunked into `APP9` (0xFFE9) marker segments in the JPEG
//!                 header. Default: the file stays a structurally valid JPEG
//!                 from SOI to EOI with no stray trailing bytes.
//!   * `Comment` — the same chunking into `COM` (0xFFFE) comment segments.
//!   * `Append`  — one blob written after the EOI marker (the classic
//!                 `cat secret >> photo.jpg` trick, done properly).
//!
//! Container written into that space (big-endian, 20-byte header):
//!
//! ```text
//! "GZJS1"(5) | flags(1) | name_len(2) | crc32(4) | orig_len(4) | blob_len(4) | name | blob
//!              bit0 = deflated
//!              bit1 = AES-256-GCM encrypted
//! ```
//!
//! `crc32` and `orig_len` always describe the ORIGINAL payload, so [`extract`]
//! can verify a round trip regardless of the compress/encrypt combination. The
//! encrypted blob is exactly the `encrypt-file` block's self-describing format
//! (magic + salt + nonce + ciphertext/tag), reused rather than reinvented.
//!
//! Pure (no wafer/wasm deps): unit-testable on the host, reused by the chat
//! skill block. Image-bytes output → chat + CLI only, no page.

use flate2::write::{DeflateDecoder, DeflateEncoder};
use flate2::{Compression, Crc};
use std::io::Write;

/// Container magic. Also the needle [`extract`] searches for.
const MAGIC: &[u8; 5] = b"GZJS1";
/// magic(5) + flags(1) + name_len(2) + crc32(4) + orig_len(4) + blob_len(4).
const HEADER_LEN: usize = 20;
/// Identifier written at the start of every marker segment we own, so
/// extraction never mistakes an unrelated APP9/COM segment for ours.
const SEG_ID: &[u8; 5] = b"GZJS\0";
/// A JPEG segment's length field is a u16 that counts itself, so the payload of
/// one segment tops out at 65535 - 2, minus our identifier.
const MAX_SEGMENT_DATA: usize = 65535 - 2 - SEG_ID.len();

const FLAG_DEFLATED: u8 = 0b0000_0001;
const FLAG_ENCRYPTED: u8 = 0b0000_0010;

const MARKER_APP9: u8 = 0xE9;
const MARKER_COM: u8 = 0xFE;

/// Longest filename recorded in the container header.
const MAX_NAME_LEN: usize = 255;

/// Where the payload is written inside the carrier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// Chunked `APP9` application marker segments (default).
    App,
    /// Chunked `COM` comment marker segments.
    Comment,
    /// A single blob written after the JPEG's `EOI` marker.
    Append,
}

impl Method {
    /// The marker byte this method writes segments with, if any.
    fn marker(self) -> Option<u8> {
        match self {
            Self::App => Some(MARKER_APP9),
            Self::Comment => Some(MARKER_COM),
            Self::Append => None,
        }
    }

    /// Canonical parameter spelling, echoed back in reports.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::App => "app",
            Self::Comment => "comment",
            Self::Append => "append",
        }
    }
}

/// Parse the `method` parameter. Fixed choice → an actionable error listing all
/// accepted spellings.
pub fn parse_method(s: &str) -> Result<Method, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "app" => Ok(Method::App),
        "comment" => Ok(Method::Comment),
        "append" => Ok(Method::Append),
        other => Err(format!(
            "unknown method `{other}` — use app (APP9 segments, default), comment (COM segments) or append (after the EOI marker)"
        )),
    }
}

/// How the payload should be packed and where it goes.
#[derive(Debug, Clone)]
pub struct Options {
    pub method: Method,
    /// Filename recorded alongside the payload (`""` records none).
    pub filename: String,
    /// `Some(passphrase)` → AES-256-GCM encrypt the payload.
    pub password: Option<String>,
    /// Deflate the payload first (kept only when it actually shrinks it).
    pub compress: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            method: Method::App,
            filename: String::new(),
            password: None,
            compress: true,
        }
    }
}

/// The stego JPEG plus everything a user needs to judge the result.
#[derive(Debug, Clone)]
pub struct EmbedReport {
    /// The carrier with the payload concealed in it — still a JPEG.
    pub output: Vec<u8>,
    /// Size of the payload as supplied.
    pub payload_bytes: usize,
    /// Size after compression/encryption, before framing.
    pub stored_bytes: usize,
    /// Size of the carrier as supplied.
    pub carrier_bytes: usize,
    /// Size of the produced JPEG.
    pub output_bytes: usize,
    /// How many marker segments were written (0 for `append`).
    pub segments: usize,
    /// Whether deflate was actually kept (it is dropped when it does not help).
    pub compressed: bool,
    /// Whether the payload was encrypted.
    pub encrypted: bool,
    /// Filename recorded in the container (empty when none).
    pub filename: String,
    pub method: Method,
}

impl EmbedReport {
    /// How much bigger the JPEG got, as a percentage of the carrier.
    pub fn growth_percent(&self) -> f64 {
        if self.carrier_bytes == 0 {
            return 0.0;
        }
        (self.output_bytes as f64 - self.carrier_bytes as f64) * 100.0 / self.carrier_bytes as f64
    }
}

/// What [`extract`] recovered.
#[derive(Debug, Clone)]
pub struct Extracted {
    pub payload: Vec<u8>,
    /// Filename recorded at embed time, or `""` if none was.
    pub filename: String,
    pub method: Method,
    pub compressed: bool,
    pub encrypted: bool,
}

/// Conceal `payload` inside the JPEG `carrier`.
///
/// The carrier's entropy-coded image data is never touched, so the output
/// renders as exactly the same picture. Any payload this tool hid previously is
/// replaced, not stacked (see `strip_existing`).
pub fn embed(carrier: &[u8], payload: &[u8], opts: &Options) -> Result<EmbedReport, String> {
    validate_jpeg(carrier)?;
    if payload.is_empty() {
        return Err("payload is empty — pass payload_text, payload_url or payload_ref".into());
    }

    let filename = sanitize_filename(&opts.filename);
    let carrier_bytes = carrier.len();

    let mut crc = Crc::new();
    crc.update(payload);
    let checksum = crc.sum();

    let mut flags = 0u8;
    let mut blob = payload.to_vec();

    if opts.compress {
        let deflated = deflate(payload)?;
        // Deflate can enlarge already-compressed payloads (zip, png, jpeg) —
        // keep it only when it wins, and report what actually happened.
        if deflated.len() < blob.len() {
            blob = deflated;
            flags |= FLAG_DEFLATED;
        }
    }

    if let Some(pass) = opts.password.as_deref() {
        if pass.is_empty() {
            return Err("password is set but empty — omit it or give a real passphrase".into());
        }
        blob = gizza_ai_encrypt_file_core::encrypt(&blob, pass)?;
        flags |= FLAG_ENCRYPTED;
    }

    let stored_bytes = blob.len();
    let container = build_container(flags, &filename, checksum, payload.len(), &blob);

    let base = strip_existing(carrier);
    let (output, segments) = match opts.method.marker() {
        Some(marker) => {
            let segs = build_segments(marker, &container);
            let at = insert_offset(&base);
            let mut out = Vec::with_capacity(base.len() + segs.len());
            out.extend_from_slice(&base[..at]);
            out.extend_from_slice(&segs);
            out.extend_from_slice(&base[at..]);
            let count = container.len().div_ceil(MAX_SEGMENT_DATA);
            (out, count)
        }
        None => {
            let mut out = Vec::with_capacity(base.len() + container.len());
            out.extend_from_slice(&base);
            out.extend_from_slice(&container);
            (out, 0)
        }
    };

    Ok(EmbedReport {
        output_bytes: output.len(),
        output,
        payload_bytes: payload.len(),
        stored_bytes,
        carrier_bytes,
        segments,
        compressed: flags & FLAG_DEFLATED != 0,
        encrypted: flags & FLAG_ENCRYPTED != 0,
        filename,
        method: opts.method,
    })
}

/// Recover a payload hidden by [`embed`]. Kept here (and round-trip tested) so a
/// future `jpg-stego-extract` block reuses exactly this reader.
pub fn extract(stego: &[u8], password: Option<&str>) -> Result<Extracted, String> {
    validate_jpeg(stego)?;

    let (container, method) = match collect_segment_payload(stego) {
        Some((bytes, method)) => (bytes, method),
        None => match find_magic(stego) {
            Some(at) => (stego[at..].to_vec(), Method::Append),
            None => return Err("no hidden payload found in this JPEG".into()),
        },
    };

    let (flags, filename, checksum, orig_len, blob) = parse_container(&container)?;

    let mut bytes = blob;
    if flags & FLAG_ENCRYPTED != 0 {
        let pass = password.ok_or("this payload is encrypted — supply the password")?;
        bytes = gizza_ai_encrypt_file_core::decrypt(&bytes, pass)?;
    }
    if flags & FLAG_DEFLATED != 0 {
        bytes = inflate(&bytes)?;
    }

    if bytes.len() != orig_len {
        return Err(format!(
            "payload length mismatch: expected {orig_len} bytes, recovered {}",
            bytes.len()
        ));
    }
    let mut crc = Crc::new();
    crc.update(&bytes);
    if crc.sum() != checksum {
        return Err("payload checksum mismatch — the image was modified after embedding".into());
    }

    Ok(Extracted {
        payload: bytes,
        filename,
        method,
        compressed: flags & FLAG_DEFLATED != 0,
        encrypted: flags & FLAG_ENCRYPTED != 0,
    })
}

// ---------------------------------------------------------------------------
// Container framing
// ---------------------------------------------------------------------------

fn build_container(
    flags: u8,
    filename: &str,
    checksum: u32,
    orig_len: usize,
    blob: &[u8],
) -> Vec<u8> {
    let name = filename.as_bytes();
    let mut out = Vec::with_capacity(HEADER_LEN + name.len() + blob.len());
    out.extend_from_slice(MAGIC);
    out.push(flags);
    out.extend_from_slice(&(name.len() as u16).to_be_bytes());
    out.extend_from_slice(&checksum.to_be_bytes());
    out.extend_from_slice(&(orig_len as u32).to_be_bytes());
    out.extend_from_slice(&(blob.len() as u32).to_be_bytes());
    out.extend_from_slice(name);
    out.extend_from_slice(blob);
    out
}

type ParsedContainer = (u8, String, u32, usize, Vec<u8>);

fn parse_container(bytes: &[u8]) -> Result<ParsedContainer, String> {
    if bytes.len() < HEADER_LEN || &bytes[..5] != MAGIC {
        return Err("hidden data is not in this tool's container format".into());
    }
    let flags = bytes[5];
    let name_len = u16::from_be_bytes([bytes[6], bytes[7]]) as usize;
    let checksum = u32::from_be_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
    let orig_len = u32::from_be_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]) as usize;
    let blob_len = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]) as usize;

    let name_end = HEADER_LEN + name_len;
    let blob_end = name_end
        .checked_add(blob_len)
        .ok_or("hidden payload header declares an impossible size")?;
    if blob_end > bytes.len() {
        return Err(format!(
            "hidden payload is truncated: header declares {blob_end} bytes but only {} are present",
            bytes.len()
        ));
    }
    let filename = String::from_utf8_lossy(&bytes[HEADER_LEN..name_end]).into_owned();
    Ok((
        flags,
        filename,
        checksum,
        orig_len,
        bytes[name_end..blob_end].to_vec(),
    ))
}

// ---------------------------------------------------------------------------
// JPEG structure
// ---------------------------------------------------------------------------

fn validate_jpeg(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() < 4 || bytes[0] != 0xFF || bytes[1] != 0xD8 || bytes[2] != 0xFF {
        return Err(
            "carrier is not a JPEG (expected the FF D8 FF start-of-image marker) — this tool only \
             writes JPEG carriers; for PNG carriers use lsb-embed"
                .into(),
        );
    }
    Ok(())
}

/// Insert point for new marker segments: just past SOI and any leading run of
/// APPn segments, so a JFIF/EXIF header keeps its conventional first position.
fn insert_offset(jpeg: &[u8]) -> usize {
    let mut pos = 2usize;
    while pos + 4 <= jpeg.len() && jpeg[pos] == 0xFF {
        let marker = jpeg[pos + 1];
        if !(0xE0..=0xEF).contains(&marker) {
            break;
        }
        let len = u16::from_be_bytes([jpeg[pos + 2], jpeg[pos + 3]]) as usize;
        if len < 2 || pos + 2 + len > jpeg.len() {
            break;
        }
        pos += 2 + len;
    }
    pos
}

fn build_segments(marker: u8, container: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    for chunk in container.chunks(MAX_SEGMENT_DATA) {
        let seg_len = 2 + SEG_ID.len() + chunk.len();
        out.push(0xFF);
        out.push(marker);
        out.extend_from_slice(&(seg_len as u16).to_be_bytes());
        out.extend_from_slice(SEG_ID);
        out.extend_from_slice(chunk);
    }
    out
}

/// Walk the header segments and concatenate the payload of every APP9/COM
/// segment carrying our identifier, in file order.
fn collect_segment_payload(jpeg: &[u8]) -> Option<(Vec<u8>, Method)> {
    let mut collected = Vec::new();
    let mut method = None;
    for (marker, body) in header_segments(jpeg) {
        if body.len() < SEG_ID.len() || &body[..SEG_ID.len()] != SEG_ID {
            continue;
        }
        if method.is_none() {
            method = Some(if marker == MARKER_COM {
                Method::Comment
            } else {
                Method::App
            });
        }
        collected.extend_from_slice(&body[SEG_ID.len()..]);
    }
    match method {
        Some(m) if collected.starts_with(MAGIC) => Some((collected, m)),
        _ => None,
    }
}

/// `(marker, body)` for every length-prefixed segment before the scan starts.
fn header_segments(jpeg: &[u8]) -> Vec<(u8, &[u8])> {
    let mut segs = Vec::new();
    let mut pos = 2usize;
    while pos + 2 <= jpeg.len() {
        if jpeg[pos] != 0xFF {
            break;
        }
        let marker = jpeg[pos + 1];
        // Standalone markers carry no length field.
        if marker == 0x01 || (0xD0..=0xD8).contains(&marker) {
            pos += 2;
            continue;
        }
        // SOS starts the entropy-coded data; EOI ends the image.
        if marker == 0xDA || marker == 0xD9 {
            break;
        }
        if pos + 4 > jpeg.len() {
            break;
        }
        let len = u16::from_be_bytes([jpeg[pos + 2], jpeg[pos + 3]]) as usize;
        if len < 2 || pos + 2 + len > jpeg.len() {
            break;
        }
        segs.push((marker, &jpeg[pos + 4..pos + 2 + len]));
        pos += 2 + len;
    }
    segs
}

fn find_magic(bytes: &[u8]) -> Option<usize> {
    bytes.windows(MAGIC.len()).position(|w| w == MAGIC)
}

/// Remove a payload this tool wrote earlier so repeated runs replace rather than
/// accumulate. Only our own segments (identifier-matched) and a trailing
/// container sitting immediately after an EOI marker are touched.
fn strip_existing(jpeg: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(jpeg.len());
    out.extend_from_slice(&jpeg[..2]);

    let mut pos = 2usize;
    while pos + 2 <= jpeg.len() {
        if jpeg[pos] != 0xFF {
            break;
        }
        let marker = jpeg[pos + 1];
        if marker == 0x01 || (0xD0..=0xD8).contains(&marker) {
            out.extend_from_slice(&jpeg[pos..pos + 2]);
            pos += 2;
            continue;
        }
        if marker == 0xDA || marker == 0xD9 {
            break;
        }
        if pos + 4 > jpeg.len() {
            break;
        }
        let len = u16::from_be_bytes([jpeg[pos + 2], jpeg[pos + 3]]) as usize;
        if len < 2 || pos + 2 + len > jpeg.len() {
            break;
        }
        let body = &jpeg[pos + 4..pos + 2 + len];
        let ours = (marker == MARKER_APP9 || marker == MARKER_COM)
            && body.len() >= SEG_ID.len()
            && &body[..SEG_ID.len()] == SEG_ID;
        if !ours {
            out.extend_from_slice(&jpeg[pos..pos + 2 + len]);
        }
        pos += 2 + len;
    }

    let mut rest = &jpeg[pos..];
    // An appended container of ours always starts right after the EOI marker.
    if let Some(at) = find_magic(rest) {
        if at >= 2 && rest[at - 2] == 0xFF && rest[at - 1] == 0xD9 {
            rest = &rest[..at];
        }
    }
    out.extend_from_slice(rest);
    out
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn sanitize_filename(name: &str) -> String {
    let base = name
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("")
        .trim()
        .replace(['\0', '\n', '\r'], "");
    base.chars().take(MAX_NAME_LEN).collect()
}

fn deflate(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut enc = DeflateEncoder::new(Vec::new(), Compression::best());
    enc.write_all(data).map_err(|e| format!("compress: {e}"))?;
    enc.finish().map_err(|e| format!("compress: {e}"))
}

fn inflate(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut dec = DeflateDecoder::new(Vec::new());
    dec.write_all(data)
        .map_err(|e| format!("decompress: {e}"))?;
    dec.finish().map_err(|e| format!("decompress: {e}"))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// A structurally valid minimal JPEG skeleton: SOI, a JFIF APP0, a short
    /// scan and EOI. Enough for every structural path here (no pixel decoding
    /// happens in this crate); `tests/real_jpeg.rs` covers a real photo.
    fn skeleton() -> Vec<u8> {
        let mut v = vec![0xFF, 0xD8];
        // APP0 / JFIF, length 16
        v.extend_from_slice(&[0xFF, 0xE0, 0x00, 0x10]);
        v.extend_from_slice(b"JFIF\0");
        v.extend_from_slice(&[0x01, 0x02, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00]);
        // SOS, length 4, then two bytes of "entropy data"
        v.extend_from_slice(&[0xFF, 0xDA, 0x00, 0x04, 0x11, 0x22]);
        v.extend_from_slice(&[0xFF, 0xD9]);
        v
    }

    #[test]
    fn happy_path_round_trip_in_app_segments() {
        let carrier = skeleton();
        let secret = b"meet at 7";
        let opts = Options {
            filename: "note.txt".into(),
            ..Options::default()
        };
        let rep = embed(&carrier, secret, &opts).unwrap();

        assert_eq!(rep.method, Method::App);
        assert_eq!(rep.segments, 1);
        assert!(rep.output_bytes > rep.carrier_bytes);
        // Still a JPEG, and the scan data is untouched at the tail.
        assert_eq!(&rep.output[..3], &[0xFF, 0xD8, 0xFF]);
        assert!(rep.output.ends_with(&[0xFF, 0xDA, 0x00, 0x04, 0x11, 0x22, 0xFF, 0xD9]));

        let got = extract(&rep.output, None).unwrap();
        assert_eq!(got.payload, secret);
        assert_eq!(got.filename, "note.txt");
        assert_eq!(got.method, Method::App);
    }

    #[test]
    fn error_on_non_jpeg_carrier() {
        let png = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        let err = embed(&png, b"x", &Options::default()).unwrap_err();
        assert!(err.contains("not a JPEG"), "unexpected error: {err}");
        assert!(err.contains("lsb-embed"), "error should point at the alternative: {err}");
    }

    #[test]
    fn error_on_empty_payload() {
        let err = embed(&skeleton(), b"", &Options::default()).unwrap_err();
        assert!(err.contains("payload is empty"), "unexpected error: {err}");
    }

    #[test]
    fn every_method_round_trips() {
        for method in [Method::App, Method::Comment, Method::Append] {
            let opts = Options {
                method,
                ..Options::default()
            };
            let rep = embed(&skeleton(), b"payload for a method test", &opts).unwrap();
            assert_eq!(rep.method, method);
            if method == Method::Append {
                assert_eq!(rep.segments, 0);
            } else {
                assert_eq!(rep.segments, 1);
            }
            let got = extract(&rep.output, None).unwrap();
            assert_eq!(got.payload, b"payload for a method test");
            assert_eq!(got.method, method, "method should be reported back");
        }
    }

    #[test]
    fn password_encrypts_and_requires_the_same_password() {
        let opts = Options {
            password: Some("hunter2".into()),
            ..Options::default()
        };
        let rep = embed(&skeleton(), b"top secret", &opts).unwrap();
        assert!(rep.encrypted);
        // The plaintext must not survive anywhere in the output.
        assert!(find(&rep.output, b"top secret").is_none());

        assert!(extract(&rep.output, None).is_err(), "must refuse without a password");
        assert!(extract(&rep.output, Some("wrong")).is_err());
        assert_eq!(extract(&rep.output, Some("hunter2")).unwrap().payload, b"top secret");
    }

    #[test]
    fn empty_password_is_rejected() {
        let opts = Options {
            password: Some(String::new()),
            ..Options::default()
        };
        let err = embed(&skeleton(), b"x", &opts).unwrap_err();
        assert!(err.contains("empty"), "unexpected error: {err}");
    }

    #[test]
    fn compression_shrinks_repetitive_payloads_and_is_dropped_when_useless() {
        let repetitive = vec![b'a'; 20_000];
        let rep = embed(&skeleton(), &repetitive, &Options::default()).unwrap();
        assert!(rep.compressed);
        assert!(rep.stored_bytes < 1_000, "stored {} bytes", rep.stored_bytes);
        assert_eq!(extract(&rep.output, None).unwrap().payload, repetitive);

        // Incompressible bytes: deflate loses, so the raw payload is stored.
        let noise = pseudo_random(4096);
        let rep = embed(&skeleton(), &noise, &Options::default()).unwrap();
        assert!(!rep.compressed, "deflate should be dropped when it does not help");
        assert_eq!(extract(&rep.output, None).unwrap().payload, noise);
    }

    #[test]
    fn compress_false_stores_the_payload_verbatim() {
        let opts = Options {
            compress: false,
            ..Options::default()
        };
        let rep = embed(&skeleton(), &vec![b'a'; 5_000], &opts).unwrap();
        assert!(!rep.compressed);
        assert_eq!(rep.stored_bytes, 5_000);
    }

    #[test]
    fn large_payload_spans_multiple_segments() {
        // Two chunks' worth of incompressible-ish data.
        let big: Vec<u8> = (0..100_000u32).map(|i| (i.wrapping_mul(2654435761) >> 11) as u8).collect();
        let opts = Options {
            compress: false,
            ..Options::default()
        };
        let rep = embed(&skeleton(), &big, &opts).unwrap();
        assert_eq!(rep.segments, 2, "100 KB must split across two 64 KB segments");
        assert_eq!(extract(&rep.output, None).unwrap().payload, big);
    }

    #[test]
    fn re_embedding_replaces_rather_than_stacks() {
        for method in [Method::App, Method::Comment, Method::Append] {
            let opts = Options {
                method,
                compress: false,
                ..Options::default()
            };
            // Same-length payloads, so any size difference is stacking, not the
            // payload itself.
            let first = embed(&skeleton(), b"first payload", &opts).unwrap();
            let second = embed(&first.output, b"other payload", &opts).unwrap();
            assert_eq!(second.output_bytes, first.output_bytes, "{method:?} grew twice");
            assert_eq!(extract(&second.output, None).unwrap().payload, b"other payload");
        }
    }

    #[test]
    fn tampering_with_the_payload_is_detected() {
        let opts = Options {
            compress: false,
            ..Options::default()
        };
        let rep = embed(&skeleton(), b"exactly these bytes", &opts).unwrap();
        let mut broken = rep.output.clone();
        let at = find(&broken, b"exactly").unwrap();
        broken[at] = b'X';
        let err = extract(&broken, None).unwrap_err();
        assert!(err.contains("checksum"), "unexpected error: {err}");
    }

    #[test]
    fn extracting_a_clean_jpeg_says_so() {
        let err = extract(&skeleton(), None).unwrap_err();
        assert!(err.contains("no hidden payload"), "unexpected error: {err}");
    }

    #[test]
    fn parse_method_accepts_the_advertised_values_and_rejects_others() {
        assert_eq!(parse_method("app").unwrap(), Method::App);
        assert_eq!(parse_method("COMMENT").unwrap(), Method::Comment);
        assert_eq!(parse_method(" append ").unwrap(), Method::Append);
        assert_eq!(parse_method("").unwrap(), Method::App);
        assert!(parse_method("dct").unwrap_err().contains("unknown method"));
    }

    #[test]
    fn filename_is_sanitized_to_a_basename() {
        let opts = Options {
            filename: "../../etc/passwd".into(),
            ..Options::default()
        };
        let rep = embed(&skeleton(), b"x", &opts).unwrap();
        assert_eq!(rep.filename, "passwd");
        assert_eq!(extract(&rep.output, None).unwrap().filename, "passwd");
    }

    #[test]
    fn growth_percent_reports_the_size_impact() {
        let rep = embed(&skeleton(), b"tiny", &Options::default()).unwrap();
        let expected = (rep.output_bytes - rep.carrier_bytes) as f64 * 100.0 / rep.carrier_bytes as f64;
        assert!((rep.growth_percent() - expected).abs() < 1e-9);
    }

    /// A deterministic xorshift64* stream. Unlike a strided multiply, its output
    /// carries no residual structure for deflate to exploit, which is what the
    /// "incompressible payload" cases here need.
    fn pseudo_random(len: usize) -> Vec<u8> {
        let mut state = 0x2545_F491_4F6C_DD1Du64;
        (0..len)
            .map(|_| {
                state ^= state >> 12;
                state ^= state << 25;
                state ^= state >> 27;
                (state.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 56) as u8
            })
            .collect()
    }

    fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack.windows(needle.len()).position(|w| w == needle)
    }
}
