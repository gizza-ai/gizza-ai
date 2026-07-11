//! base-decoder core — auto-detect and recursively peel layered base encodings
//! (Base16/hex, Base32, Base45, Base58, Base64, Base85/Ascii85) until plaintext
//! or a recognized binary signature emerges. Pure Rust, no I/O — shared by the
//! chat skill block and the web page.
//!
//! Heuristic: at each layer, every base whose alphabet the current text matches
//! is attempted; a decode is accepted as a *text* layer when the result is valid
//! UTF-8 that is mostly printable (>= `MIN_TEXT_RATIO`), or as a terminal
//! *binary* layer when it starts with a known file signature. Every base decode
//! shrinks the buffer, so the peel always terminates (bounded further by
//! `max_depth`).

use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD};
use base64::Engine;

/// Reject absurdly large inputs (1 MB of text).
pub const MAX_INPUT: usize = 1_000_000;
/// Default number of layers to peel when the caller doesn't specify one.
pub const DEFAULT_DEPTH: usize = 8;
/// Lower/upper clamp for the caller-supplied depth.
pub const MIN_DEPTH: usize = 1;
pub const MAX_DEPTH: usize = 30;
/// Fraction of a decoded UTF-8 string that must be non-control (bar tab/nl/cr)
/// for the layer to be treated as text and peeled further.
const MIN_TEXT_RATIO: f64 = 0.85;
/// Single-byte decodes are almost always noise; require at least this many.
const MIN_TEXT_BYTES: usize = 2;
/// How many decoded bytes to show in the report's hex preview.
const HEX_PREVIEW: usize = 48;

/// Detection order — earlier entries win when two bases tie on text score.
const DECODERS: &[(&str, fn(&str) -> Option<Vec<u8>>)] = &[
    ("base64", try_base64),
    ("base32", try_base32),
    ("base16", try_base16),
    ("base58", try_base58),
    ("base85", try_base85),
    ("base45", try_base45),
];

/// The outcome of an auto-decode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodeResult {
    /// Encoding labels applied, outermost first (e.g. `["base64", "base32"]`).
    pub layers: Vec<&'static str>,
    /// The final decoded bytes.
    pub bytes: Vec<u8>,
    /// True when the final output is valid UTF-8 text.
    pub is_text: bool,
    /// Detected file signature when the output is binary (e.g. "PNG image").
    pub signature: Option<String>,
}

/// Peel encoding layers off `input` until plaintext / a signature / `max_depth`.
pub fn auto_decode(input: &str, max_depth: usize) -> DecodeResult {
    let mut current: Vec<u8> = input.trim().as_bytes().to_vec();
    let mut layers: Vec<&'static str> = Vec::new();
    let mut signature: Option<String> = None;

    for _ in 0..max_depth {
        // Only valid UTF-8 text can be a candidate for further base decoding.
        let text = match std::str::from_utf8(&current) {
            Ok(t) => t.trim(),
            Err(_) => break,
        };
        if text.is_empty() {
            break;
        }

        let mut best_text: Option<(f64, &'static str, Vec<u8>)> = None;
        let mut best_bin: Option<(&'static str, Vec<u8>, String)> = None;

        for (name, decode) in DECODERS {
            let Some(bytes) = decode(text) else { continue };
            if bytes.is_empty() || bytes == current {
                continue;
            }
            match std::str::from_utf8(&bytes) {
                Ok(s) if bytes.len() >= MIN_TEXT_BYTES && text_ratio(s) >= MIN_TEXT_RATIO => {
                    let score = text_ratio(s);
                    if best_text.as_ref().map_or(true, |(b, _, _)| score > *b) {
                        best_text = Some((score, name, bytes));
                    }
                }
                _ => {
                    if best_bin.is_none() {
                        if let Some(sig) = detect_signature(&bytes) {
                            best_bin = Some((name, bytes, sig));
                        }
                    }
                }
            }
        }

        if let Some((_, name, bytes)) = best_text {
            layers.push(name);
            current = bytes;
        } else if let Some((name, bytes, sig)) = best_bin {
            layers.push(name);
            current = bytes;
            signature = Some(sig);
            break; // reached a recognizable binary target
        } else {
            break; // nothing decodes further → current is the plaintext
        }
    }

    let is_text = signature.is_none() && std::str::from_utf8(&current).is_ok();
    DecodeResult { layers, bytes: current, is_text, signature }
}

/// Top-level entry: validate, decode, and render for a surface.
/// `output` is "report" (default) or "plain".
pub fn decode(input: &str, max_depth: usize, output: &str) -> Result<String, String> {
    if input.len() > MAX_INPUT {
        return Err(format!(
            "input too large ({} bytes); maximum is {} bytes",
            input.len(),
            MAX_INPUT
        ));
    }
    let depth = max_depth.clamp(MIN_DEPTH, MAX_DEPTH);
    let plain = match output.trim().to_ascii_lowercase().as_str() {
        "" | "report" => false,
        "plain" => true,
        other => {
            return Err(format!(
                "output {other:?} not supported (expected \"report\" or \"plain\")"
            ))
        }
    };
    let res = auto_decode(input, depth);
    Ok(render(&res, plain, input.trim()))
}

/// Format a `DecodeResult` for display.
pub fn render(res: &DecodeResult, plain: bool, original: &str) -> String {
    if plain {
        return if res.is_text {
            String::from_utf8_lossy(&res.bytes).into_owned()
        } else {
            to_hex(&res.bytes)
        };
    }

    if res.layers.is_empty() {
        return format!(
            "No base encoding detected — the input isn't valid Base16/32/45/58/64/85 \
             (or is a single unrecognized layer). Left unchanged:\n{original}"
        );
    }

    let chain = res.layers.join(" → ");
    let n = res.layers.len();
    if res.is_text {
        let text = String::from_utf8_lossy(&res.bytes);
        format!("Detected {n} layer(s): {chain}\n\n{text}")
    } else {
        let sig = res.signature.as_deref().unwrap_or("binary data");
        let preview = &res.bytes[..res.bytes.len().min(HEX_PREVIEW)];
        let ellipsis = if res.bytes.len() > HEX_PREVIEW { "…" } else { "" };
        format!(
            "Detected {n} layer(s): {chain}\n\nBinary output — {sig} ({} bytes)\nHex: {}{ellipsis}",
            res.bytes.len(),
            to_hex(preview)
        )
    }
}

/// Fraction of chars in a UTF-8 string that are not control chars (tab/nl/cr ok).
fn text_ratio(s: &str) -> f64 {
    let total = s.chars().count();
    if total == 0 {
        return 0.0;
    }
    let good = s
        .chars()
        .filter(|c| !c.is_control() || matches!(c, '\n' | '\r' | '\t'))
        .count();
    good as f64 / total as f64
}

fn to_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Recognize a leading magic-byte signature (used to stop at binary targets).
fn detect_signature(b: &[u8]) -> Option<String> {
    const SIGS: &[(&[u8], &str)] = &[
        (b"\x89PNG\r\n\x1a\n", "PNG image"),
        (b"\xFF\xD8\xFF", "JPEG image"),
        (b"GIF87a", "GIF image"),
        (b"GIF89a", "GIF image"),
        (b"%PDF-", "PDF document"),
        (b"PK\x03\x04", "ZIP archive"),
        (b"PK\x05\x06", "ZIP archive"),
        (b"\x1F\x8B", "gzip data"),
        (b"BZh", "bzip2 data"),
        (b"\x7FELF", "ELF binary"),
        (b"\x78\x01", "zlib data"),
        (b"\x78\x9C", "zlib data"),
        (b"\x78\xDA", "zlib data"),
        (b"OggS", "Ogg media"),
        (b"RIFF", "RIFF (WAV/AVI/WebP)"),
        (b"\xD0\xCF\x11\xE0", "MS Office (OLE)"),
        (b"Rar!\x1a\x07", "RAR archive"),
        (b"ID3", "MP3 (ID3)"),
        (b"fLaC", "FLAC audio"),
        (b"\x00\x00\x01\x00", "ICO icon"),
    ];
    for (magic, name) in SIGS {
        if b.starts_with(magic) {
            return Some((*name).to_string());
        }
    }
    None
}

// ---- per-base decoders -----------------------------------------------------

fn strip_ws(text: &str) -> String {
    text.chars().filter(|c| !c.is_ascii_whitespace()).collect()
}

fn try_base16(text: &str) -> Option<Vec<u8>> {
    let cleaned = strip_ws(text);
    if cleaned.is_empty() || cleaned.len() % 2 != 0 {
        return None;
    }
    let b = cleaned.as_bytes();
    let mut out = Vec::with_capacity(cleaned.len() / 2);
    let mut i = 0;
    while i < b.len() {
        let hi = (b[i] as char).to_digit(16)?;
        let lo = (b[i + 1] as char).to_digit(16)?;
        out.push(((hi << 4) | lo) as u8);
        i += 2;
    }
    Some(out)
}

fn try_base32(text: &str) -> Option<Vec<u8>> {
    const ALPHA: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    let cleaned: String = strip_ws(text)
        .chars()
        .map(|c| c.to_ascii_uppercase())
        .collect();
    let body = cleaned.trim_end_matches('=');
    if body.is_empty() {
        return None;
    }
    let mut buffer: u64 = 0;
    let mut bits: u32 = 0;
    let mut out = Vec::new();
    for c in body.bytes() {
        let val = ALPHA.iter().position(|&x| x == c)? as u64;
        buffer = (buffer << 5) | val;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
        }
    }
    // Any leftover bits must be zero (canonical padding).
    if bits > 0 && (buffer & ((1u64 << bits) - 1)) != 0 {
        return None;
    }
    Some(out)
}

fn try_base64(text: &str) -> Option<Vec<u8>> {
    let cleaned = strip_ws(text);
    if cleaned.is_empty() {
        return None;
    }
    if !cleaned
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '=' | '-' | '_'))
    {
        return None;
    }
    for eng in [&STANDARD, &URL_SAFE, &STANDARD_NO_PAD, &URL_SAFE_NO_PAD] {
        if let Ok(bytes) = eng.decode(cleaned.as_bytes()) {
            return Some(bytes);
        }
    }
    None
}

fn try_base58(text: &str) -> Option<Vec<u8>> {
    let cleaned = strip_ws(text);
    if cleaned.is_empty() {
        return None;
    }
    bs58::decode(cleaned).into_vec().ok()
}

fn try_base85(text: &str) -> Option<Vec<u8>> {
    let mut s = strip_ws(text);
    if let Some(rest) = s.strip_prefix("<~") {
        s = rest.to_string();
    }
    if let Some(rest) = s.strip_suffix("~>") {
        s = rest.to_string();
    }
    if s.is_empty() {
        return None;
    }
    let mut out = Vec::new();
    let mut group = [0u8; 5];
    let mut n = 0usize;
    for c in s.bytes() {
        if c == b'z' && n == 0 {
            out.extend_from_slice(&[0, 0, 0, 0]);
            continue;
        }
        if !(b'!'..=b'u').contains(&c) {
            return None;
        }
        group[n] = c - b'!';
        n += 1;
        if n == 5 {
            let mut val: u32 = 0;
            for &g in &group {
                val = val.checked_mul(85)?.checked_add(g as u32)?;
            }
            out.extend_from_slice(&val.to_be_bytes());
            n = 0;
        }
    }
    if n == 1 {
        return None; // a lone trailing char is invalid Ascii85
    }
    if n > 1 {
        let mut g = group;
        for slot in g.iter_mut().take(5).skip(n) {
            *slot = 84; // pad with 'u'
        }
        let mut val: u32 = 0;
        for &x in &g {
            val = val.checked_mul(85)?.checked_add(x as u32)?;
        }
        out.extend_from_slice(&val.to_be_bytes()[..n - 1]);
    }
    Some(out)
}

fn try_base45(text: &str) -> Option<Vec<u8>> {
    const ALPHA: &[u8; 45] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ $%*+-./:";
    // Base45 uses space as a data symbol, so strip only line-break whitespace.
    let s: String = text
        .chars()
        .filter(|c| !matches!(c, '\n' | '\r' | '\t'))
        .collect();
    if s.is_empty() {
        return None;
    }
    let vals: Option<Vec<u32>> = s
        .bytes()
        .map(|c| ALPHA.iter().position(|&x| x == c).map(|p| p as u32))
        .collect();
    let vals = vals?;
    let mut out = Vec::new();
    let mut i = 0;
    while i + 3 <= vals.len() {
        let n = vals[i] + vals[i + 1] * 45 + vals[i + 2] * 45 * 45;
        if n > 0xFFFF {
            return None;
        }
        out.push((n >> 8) as u8);
        out.push((n & 0xFF) as u8);
        i += 3;
    }
    match vals.len() - i {
        0 => {}
        2 => {
            let n = vals[i] + vals[i + 1] * 45;
            if n > 0xFF {
                return None;
            }
            out.push(n as u8);
        }
        _ => return None, // remainder of 1 is invalid Base45
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_layer_base64_text() {
        // base64("Hello, World!")
        let r = auto_decode("SGVsbG8sIFdvcmxkIQ==", 8);
        assert_eq!(r.layers, ["base64"]);
        assert!(r.is_text);
        assert_eq!(r.bytes, b"Hello, World!");
    }

    #[test]
    fn nested_base64_over_base64() {
        // base64(base64("Hello, World!"))
        let r = auto_decode("U0dWc2JHOHNJRmR2Y214a0lRPT0=", 8);
        assert_eq!(r.layers, ["base64", "base64"]);
        assert_eq!(r.bytes, b"Hello, World!");
    }

    #[test]
    fn base32_layer() {
        // base32("hello world") = NBSWY3DPEB3W64TMMQ======
        let r = auto_decode("NBSWY3DPEB3W64TMMQ======", 8);
        assert_eq!(r.layers, ["base32"]);
        assert_eq!(r.bytes, b"hello world");
    }

    #[test]
    fn hex_layer() {
        // hex("hello") = 68656c6c6f
        let r = auto_decode("68656c6c6f", 8);
        assert_eq!(r.layers, ["base16"]);
        assert_eq!(r.bytes, b"hello");
    }

    #[test]
    fn depth_cap_stops_early() {
        // With max_depth = 1 only the outer layer is peeled.
        let r = auto_decode("U0dWc2JHOHNJRmR2Y214a0lRPT0=", 1);
        assert_eq!(r.layers.len(), 1);
        assert_eq!(r.bytes, b"SGVsbG8sIFdvcmxkIQ==");
    }

    #[test]
    fn plaintext_left_unchanged() {
        let r = auto_decode("Hello, World!", 8);
        assert!(r.layers.is_empty());
        assert_eq!(r.bytes, b"Hello, World!");
    }

    #[test]
    fn binary_signature_stops_peel() {
        // hex of a PNG header → recognized as a binary target, not text.
        let png_hex = "89504e470d0a1a0a0000000d49484452";
        let r = auto_decode(png_hex, 8);
        assert_eq!(r.layers, ["base16"]);
        assert!(!r.is_text);
        assert_eq!(r.signature.as_deref(), Some("PNG image"));
    }

    #[test]
    fn whitespace_and_newlines_ignored() {
        let r = auto_decode("SGVs bG8s\nIFdv cmxk IQ==", 8);
        assert_eq!(r.bytes, b"Hello, World!");
    }

    #[test]
    fn render_report_and_plain() {
        let r = auto_decode("U0dWc2JHOHNJRmR2Y214a0lRPT0=", 8);
        assert_eq!(
            render(&r, false, "U0dWc2JHOHNJRmR2Y214a0lRPT0="),
            "Detected 2 layer(s): base64 → base64\n\nHello, World!"
        );
        assert_eq!(render(&r, true, ""), "Hello, World!");
    }

    #[test]
    fn decode_entry_validates_output_mode() {
        assert!(decode("SGVsbG8sIFdvcmxkIQ==", 8, "sideways").is_err());
        assert_eq!(
            decode("SGVsbG8sIFdvcmxkIQ==", 8, "plain").unwrap(),
            "Hello, World!"
        );
    }

    #[test]
    fn oversize_input_errors() {
        let big = "A".repeat(MAX_INPUT + 1);
        assert!(decode(&big, 8, "report").is_err());
    }

    #[test]
    fn no_decode_report_echoes_input() {
        let out = decode("just plain words here", 8, "report").unwrap();
        assert!(out.starts_with("No base encoding detected"));
        assert!(out.contains("just plain words here"));
    }
}
