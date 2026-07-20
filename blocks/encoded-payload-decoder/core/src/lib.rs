//! encoded-payload-decoder core — find and decode base64 / hex tokens and
//! gzip / zlib compressed streams embedded anywhere in a byte buffer, unwrap
//! nested layers, and surface hidden readable strings + a detected file type
//! for each binary payload. Pure-Rust (`base64`, `flate2`, `regex`), wasm-safe.

use base64::engine::general_purpose::{STANDARD_NO_PAD, URL_SAFE_NO_PAD};
use base64::Engine;
use flate2::read::{GzDecoder, ZlibDecoder};
use regex::bytes::Regex;
use serde::Serialize;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::io::Read;

/// Cap on a single decompressed / decoded payload (decompression-bomb defense).
pub const MAX_PAYLOAD: usize = 4 * 1024 * 1024;
/// Cap on total decoded bytes across the whole scan.
pub const MAX_TOTAL_DECODED: usize = 32 * 1024 * 1024;
/// Cap on the number of findings returned.
pub const MAX_FINDINGS: usize = 200;
/// Number of leading decoded bytes shown as hex for a binary payload.
const HEX_PREVIEW: usize = 32;
/// How many surfaced strings to list per binary payload.
const MAX_STRINGS: usize = 40;
/// Minimum run length for a surfaced string (matches `strings` default).
const STRING_MIN_LEN: usize = 4;

/// Scan options.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// Minimum length of a base64/hex token run to treat as a candidate.
    pub min_len: usize,
    /// Maximum nested-decode depth (0 = only scan the file itself).
    pub max_depth: usize,
}

impl Default for Options {
    fn default() -> Self {
        Options { min_len: 20, max_depth: 3 }
    }
}

/// One decoded payload found in the input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Finding {
    /// "base64", "hex", "gzip", or "zlib".
    pub encoding: String,
    /// Byte offset where the payload was found within its parent buffer.
    pub offset: usize,
    /// Nesting depth (0 = found directly in the file, 1 = inside a depth-0 payload, …).
    pub depth: usize,
    /// Number of decoded/decompressed bytes.
    pub bytes: usize,
    /// "text" or "binary".
    pub kind: String,
    /// Decoded UTF-8 text (present when `kind == "text"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Detected file type, e.g. "image/png" (present for recognized binaries).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_type: Option<String>,
    /// Hex of the first decoded bytes (present when `kind == "binary"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hex_preview: Option<String>,
    /// Readable strings surfaced from a binary payload (present when `kind == "binary"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strings: Option<Vec<String>>,
}

/// Full scan result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Report {
    pub findings: Vec<Finding>,
    pub count: usize,
    /// True if the findings or decoded-byte budget cap was hit.
    pub truncated: bool,
}

fn printable_ratio(bytes: &[u8]) -> f64 {
    if bytes.is_empty() {
        return 0.0;
    }
    let ok = bytes
        .iter()
        .filter(|&&b| b == b'\t' || b == b'\n' || b == b'\r' || (0x20..=0x7e).contains(&b))
        .count();
    ok as f64 / bytes.len() as f64
}

/// Magic-byte file-type sniff for common binary formats.
fn sniff(bytes: &[u8]) -> Option<&'static str> {
    let b = bytes;
    if b.starts_with(&[0x89, b'P', b'N', b'G']) {
        Some("image/png")
    } else if b.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if b.starts_with(b"GIF87a") || b.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if b.len() >= 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        Some("image/webp")
    } else if b.starts_with(b"%PDF-") {
        Some("application/pdf")
    } else if b.starts_with(&[0x50, 0x4B, 0x03, 0x04]) || b.starts_with(&[0x50, 0x4B, 0x05, 0x06]) {
        Some("application/zip")
    } else if b.starts_with(&[0x1F, 0x8B]) {
        Some("application/gzip")
    } else if b.starts_with(&[0x7F, b'E', b'L', b'F']) {
        Some("application/x-elf")
    } else if b.starts_with(&[0x4D, 0x5A]) {
        Some("application/x-msdownload")
    } else if b.starts_with(b"{\\rtf") {
        Some("application/rtf")
    } else if b.starts_with(&[0xD0, 0xCF, 0x11, 0xE0]) {
        Some("application/x-ole-storage")
    } else {
        None
    }
}

/// ASCII + UTF-16LE surfaced strings from a binary payload (bounded).
fn surface_strings(data: &[u8]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    // ASCII runs.
    let mut run: Vec<u8> = Vec::new();
    for &c in data {
        if c == b'\t' || (0x20..=0x7e).contains(&c) {
            run.push(c);
        } else {
            if run.len() >= STRING_MIN_LEN {
                out.push(String::from_utf8_lossy(&run).into_owned());
                if out.len() >= MAX_STRINGS {
                    return out;
                }
            }
            run.clear();
        }
    }
    if run.len() >= STRING_MIN_LEN {
        out.push(String::from_utf8_lossy(&run).into_owned());
    }
    // UTF-16LE runs (printable low byte + zero high byte).
    let mut i = 0;
    let mut wide: Vec<u8> = Vec::new();
    while i + 1 < data.len() && out.len() < MAX_STRINGS {
        let (lo, hi) = (data[i], data[i + 1]);
        if hi == 0 && (lo == b'\t' || (0x20..=0x7e).contains(&lo)) {
            wide.push(lo);
        } else {
            if wide.len() >= STRING_MIN_LEN {
                out.push(String::from_utf8_lossy(&wide).into_owned());
            }
            wide.clear();
        }
        i += 2;
    }
    if wide.len() >= STRING_MIN_LEN && out.len() < MAX_STRINGS {
        out.push(String::from_utf8_lossy(&wide).into_owned());
    }
    out.truncate(MAX_STRINGS);
    out
}

/// Decompress a gzip stream, bounded to `MAX_PAYLOAD`. Errors on invalid data.
pub fn decompress_gzip(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    GzDecoder::new(data)
        .take(MAX_PAYLOAD as u64)
        .read_to_end(&mut out)
        .map_err(|e| format!("gzip decode failed: {e}"))?;
    if out.is_empty() {
        return Err("gzip produced no output".into());
    }
    Ok(out)
}

/// Decompress a zlib (RFC 1950) stream, bounded to `MAX_PAYLOAD`. Errors on invalid data.
pub fn decompress_zlib(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    ZlibDecoder::new(data)
        .take(MAX_PAYLOAD as u64)
        .read_to_end(&mut out)
        .map_err(|e| format!("zlib decode failed: {e}"))?;
    if out.is_empty() {
        return Err("zlib produced no output".into());
    }
    Ok(out)
}

fn try_base64(token: &[u8]) -> Option<Vec<u8>> {
    let s = std::str::from_utf8(token).ok()?;
    let trimmed = s.trim_end_matches('=');
    if trimmed.len() % 4 == 1 {
        return None;
    }
    let has_std = trimmed.contains(['+', '/']);
    let has_url = trimmed.contains(['-', '_']);
    if has_std && has_url {
        return None; // a real token won't mix the two alphabets
    }
    let eng = if has_url { URL_SAFE_NO_PAD } else { STANDARD_NO_PAD };
    let out = eng.decode(trimmed.as_bytes()).ok()?;
    (!out.is_empty()).then_some(out)
}

fn try_hex(token: &[u8]) -> Option<Vec<u8>> {
    if token.len() % 2 != 0 {
        return None;
    }
    let hexval = |c: u8| -> Option<u8> {
        match c {
            b'0'..=b'9' => Some(c - b'0'),
            b'a'..=b'f' => Some(c - b'a' + 10),
            b'A'..=b'F' => Some(c - b'A' + 10),
            _ => None,
        }
    };
    let mut out = Vec::with_capacity(token.len() / 2);
    for pair in token.chunks(2) {
        out.push((hexval(pair[0])? << 4) | hexval(pair[1])?);
    }
    (!out.is_empty()).then_some(out)
}

fn hash_bytes(b: &[u8]) -> u64 {
    let mut h = DefaultHasher::new();
    b.hash(&mut h);
    h.finish()
}

/// True if decoded bytes look like a payload worth recursing into or reporting
/// (printable text or a compressed/known-binary stream), not random noise.
fn is_interesting(bytes: &[u8]) -> bool {
    if printable_ratio(bytes) >= 0.85 {
        return true;
    }
    if sniff(bytes).is_some() {
        return true;
    }
    // Compressed inner layer?
    bytes.starts_with(&[0x1F, 0x8B]) || is_zlib_header(bytes)
}

fn is_zlib_header(b: &[u8]) -> bool {
    if b.len() < 2 {
        return false;
    }
    // CMF/FLG: low nibble of CMF is deflate (8), and (CMF<<8|FLG) % 31 == 0.
    b[0] & 0x0F == 0x08 && (((b[0] as u16) << 8 | b[1] as u16) % 31 == 0)
}

struct Scanner<'a> {
    opts: Options,
    findings: Vec<Finding>,
    seen: HashSet<u64>,
    total_decoded: usize,
    truncated: bool,
    b64_re: &'a Regex,
    hex_re: &'a Regex,
}

impl Scanner<'_> {
    fn budget_left(&self) -> bool {
        self.findings.len() < MAX_FINDINGS && self.total_decoded < MAX_TOTAL_DECODED
    }

    /// Record a finding and, if depth budget remains, recurse into its bytes.
    fn record(&mut self, encoding: &str, offset: usize, depth: usize, decoded: Vec<u8>) {
        if !self.budget_left() {
            self.truncated = true;
            return;
        }
        let h = hash_bytes(&decoded);
        if !self.seen.insert(h) {
            return; // duplicate payload already reported
        }
        self.total_decoded = self.total_decoded.saturating_add(decoded.len());

        let (kind, text, file_type, hex_preview, strings) =
            if printable_ratio(&decoded) >= 0.85 && std::str::from_utf8(&decoded).is_ok() {
                ("text", Some(String::from_utf8_lossy(&decoded).into_owned()), None, None, None)
            } else {
                let hex: String =
                    decoded.iter().take(HEX_PREVIEW).map(|b| format!("{b:02x}")).collect();
                let strs = surface_strings(&decoded);
                (
                    "binary",
                    None,
                    sniff(&decoded).map(|s| s.to_string()),
                    Some(hex),
                    (!strs.is_empty()).then_some(strs),
                )
            };

        self.findings.push(Finding {
            encoding: encoding.to_string(),
            offset,
            depth,
            bytes: decoded.len(),
            kind: kind.to_string(),
            text,
            file_type,
            hex_preview,
            strings,
        });

        if depth < self.opts.max_depth {
            self.scan_buffer(&decoded, depth + 1);
        }
    }

    fn scan_buffer(&mut self, buf: &[u8], depth: usize) {
        if !self.budget_left() {
            self.truncated = true;
            return;
        }
        // 1. gzip streams (magic 1f 8b 08) anywhere in the buffer.
        let mut i = 0;
        while i + 3 < buf.len() {
            if buf[i] == 0x1F && buf[i + 1] == 0x8B && buf[i + 2] == 0x08 {
                if let Ok(out) = decompress_gzip(&buf[i..]) {
                    self.record("gzip", i, depth, out);
                }
                if !self.budget_left() {
                    return;
                }
            }
            i += 1;
        }
        // 2. zlib streams (valid CMF/FLG header) anywhere in the buffer.
        let mut j = 0;
        while j + 2 < buf.len() {
            if is_zlib_header(&buf[j..]) {
                if let Ok(out) = decompress_zlib(&buf[j..]) {
                    self.record("zlib", j, depth, out);
                }
                if !self.budget_left() {
                    return;
                }
            }
            j += 1;
        }
        // 3. base64 token runs.
        for m in self.b64_re.find_iter(buf) {
            if let Some(out) = try_base64(m.as_bytes()) {
                if is_interesting(&out) {
                    self.record("base64", m.start(), depth, out);
                }
            }
            if !self.budget_left() {
                return;
            }
        }
        // 4. hex token runs.
        for m in self.hex_re.find_iter(buf) {
            if let Some(out) = try_hex(m.as_bytes()) {
                if is_interesting(&out) {
                    self.record("hex", m.start(), depth, out);
                }
            }
            if !self.budget_left() {
                return;
            }
        }
    }
}

/// Scan `data` for embedded base64 / hex / gzip / zlib payloads and return every
/// decoded layer found (recursively, up to `opts.max_depth`).
pub fn scan(data: &[u8], opts: Options) -> Report {
    let min = opts.min_len.max(4);
    // Token regexes built from min_len. base64: std OR url-safe alphabet run.
    let b64_re =
        Regex::new(&format!(r"(?-u:[A-Za-z0-9+/_\-]{{{min},}}={{0,2}})")).expect("b64 regex");
    // hex: even length enforced in try_hex; require at least `min` chars.
    let hex_re = Regex::new(&format!(r"(?-u:[0-9a-fA-F]{{{min},}})")).expect("hex regex");

    let mut scanner = Scanner {
        opts: Options { min_len: min, ..opts },
        findings: Vec::new(),
        seen: HashSet::new(),
        total_decoded: 0,
        truncated: false,
        b64_re: &b64_re,
        hex_re: &hex_re,
    };
    scanner.scan_buffer(data, 0);

    let truncated = scanner.truncated;
    let count = scanner.findings.len();
    Report { findings: scanner.findings, count, truncated }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::{GzEncoder, ZlibEncoder};
    use flate2::Compression;
    use std::io::Write;

    fn gzip(data: &[u8]) -> Vec<u8> {
        let mut e = GzEncoder::new(Vec::new(), Compression::default());
        e.write_all(data).unwrap();
        e.finish().unwrap()
    }
    fn zlib(data: &[u8]) -> Vec<u8> {
        let mut e = ZlibEncoder::new(Vec::new(), Compression::default());
        e.write_all(data).unwrap();
        e.finish().unwrap()
    }

    #[test]
    fn finds_base64_text_in_noise() {
        // "Hello, hidden world!" base64 = SGVsbG8sIGhpZGRlbiB3b3JsZCE=
        let input = b"prefix junk log=SGVsbG8sIGhpZGRlbiB3b3JsZCE= suffix";
        let r = scan(input, Options::default());
        assert_eq!(r.count, 1);
        assert_eq!(r.findings[0].encoding, "base64");
        assert_eq!(r.findings[0].kind, "text");
        assert_eq!(r.findings[0].text.as_deref(), Some("Hello, hidden world!"));
        assert_eq!(r.findings[0].depth, 0);
    }

    #[test]
    fn finds_whole_file_gzip() {
        let payload = b"the quick brown fox jumps over the lazy dog, repeatedly and repeatedly";
        let g = gzip(payload);
        let r = scan(&g, Options::default());
        let gz = r.findings.iter().find(|f| f.encoding == "gzip").unwrap();
        assert_eq!(gz.offset, 0);
        assert_eq!(gz.kind, "text");
        assert_eq!(gz.text.as_deref(), Some(std::str::from_utf8(payload).unwrap()));
    }

    #[test]
    fn nested_base64_of_gzip_unwraps() {
        // base64( gzip("nested secret payload text here for length") )
        let payload = b"nested secret payload text here for length";
        let g = gzip(payload);
        let b64 = STANDARD_NO_PAD.encode(&g);
        let input = format!("data:{b64}");
        let r = scan(input.as_bytes(), Options { min_len: 12, max_depth: 3 });
        assert!(r.findings.iter().any(|f| f.encoding == "base64" && f.depth == 0));
        let inner = r
            .findings
            .iter()
            .find(|f| f.encoding == "gzip" && f.depth == 1)
            .expect("nested gzip layer");
        assert_eq!(inner.text.as_deref(), Some("nested secret payload text here for length"));
    }

    #[test]
    fn finds_zlib_stream() {
        let payload = b"zlib compressed content that is definitely long enough to matter";
        let z = zlib(payload);
        let mut input = b"AAAA".to_vec();
        input.extend_from_slice(&z);
        let r = scan(&input, Options::default());
        let zf = r.findings.iter().find(|f| f.encoding == "zlib").unwrap();
        assert_eq!(zf.offset, 4);
        assert_eq!(
            zf.text.as_deref(),
            Some("zlib compressed content that is definitely long enough to matter")
        );
    }

    #[test]
    fn surfaces_strings_from_binary_payload() {
        // base64 of a PNG-magic + readable-strings binary blob.
        let mut blob = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        blob.extend_from_slice(&[0, 1, 2, 3]);
        blob.extend_from_slice(b"secret-flag{surfaced}");
        blob.extend_from_slice(&[0xFF, 0xFE]);
        let b64 = STANDARD_NO_PAD.encode(&blob);
        let r = scan(b64.as_bytes(), Options { min_len: 12, max_depth: 2 });
        let f = r.findings.iter().find(|f| f.encoding == "base64").unwrap();
        assert_eq!(f.kind, "binary");
        assert_eq!(f.file_type.as_deref(), Some("image/png"));
        assert!(f.strings.as_ref().unwrap().iter().any(|s| s.contains("secret-flag{surfaced}")));
    }

    #[test]
    fn ignores_random_alphanumeric_noise() {
        // A long run of identical letters base64-decodes to non-printable
        // garbage with no recognizable file type → not reported.
        let input = b"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxq";
        let r = scan(input, Options::default());
        assert_eq!(r.count, 0);
    }

    #[test]
    fn decompress_gzip_errors_on_bad_data() {
        // Valid gzip magic but garbage body → Err (the error path).
        let bad = [0x1F, 0x8B, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03, 0xFF, 0xFF];
        assert!(decompress_gzip(&bad).is_err());
    }

    #[test]
    fn decompress_zlib_errors_on_bad_data() {
        assert!(decompress_zlib(&[0x78, 0x9C, 0x00, 0x01, 0x02]).is_err());
    }

    #[test]
    fn empty_input_is_empty_report() {
        let r = scan(b"", Options::default());
        assert_eq!(r.count, 0);
        assert!(!r.truncated);
    }
}
