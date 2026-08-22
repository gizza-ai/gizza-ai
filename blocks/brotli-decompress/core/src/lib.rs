//! brotli-decompress core — decode a Brotli-compressed (RFC 7932) payload that
//! has been pasted as **Base64** or **hex** back to the original bytes, and
//! render those bytes as text, hex, or Base64.
//!
//! This is the inline/readable half of the Brotli story in this repo: the
//! `file-compressor` block takes a real `.br` FILE (url/ref) and hands back a
//! download, while this block takes a pasted blob — an HTTP `content-encoding:
//! br` body, an asset-bundle chunk, a log line — and prints what is inside it.
//!
//! Brotli has **no magic number**, so the payload can never be sniffed up front.
//! `encoding = "auto"` therefore decodes each candidate transport encoding and
//! keeps whichever one actually Brotli-decompresses, and a wrong-codec blob is
//! only diagnosed *after* the Brotli attempt has failed (see `other_codec`).
//!
//! Pure compute, no wafer/wasm-bindgen deps — shared by the chat skill block and
//! the web page. Runs on every backend including the chat Service Worker.

use std::io::Read;

use base64::engine::general_purpose::{STANDARD as B64, STANDARD_NO_PAD as B64_NP, URL_SAFE_NO_PAD as B64_URL};
use base64::Engine;

/// Max compressed input accepted, in bytes. The payload lives in wasm linear
/// memory, so an unbounded paste would OOM-trap the tab instead of erroring.
pub const MAX_INPUT_BYTES: usize = 8 * 1024 * 1024; // 8 MiB

/// Max decompressed output, in bytes — the decompression-bomb guard. Matches
/// `file-compressor` / `lz4-decompress` / `lzma-decompress`.
pub const MAX_OUTPUT_BYTES: u64 = 16 * 1024 * 1024; // 16 MiB

/// How the pasted payload in `data` is encoded.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Encoding {
    /// Try hex, then Base64, and keep whichever actually Brotli-decodes.
    Auto,
    /// Standard or URL-safe Base64 (RFC 4648), padding optional.
    Base64,
    /// A hex string; ASCII whitespace and an optional `0x` prefix are ignored.
    Hex,
}

impl Encoding {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "auto" => Ok(Encoding::Auto),
            "base64" | "b64" => Ok(Encoding::Base64),
            "hex" => Ok(Encoding::Hex),
            other => Err(format!(
                "invalid encoding {other:?}: expected \"auto\", \"base64\", or \"hex\""
            )),
        }
    }
}

/// How to render the decompressed bytes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Output {
    /// UTF-8 text — the default; errors if the bytes are not valid UTF-8.
    Text,
    /// A lowercase hex string.
    Hex,
    /// Standard Base64 (RFC 4648), padded.
    Base64,
}

impl Output {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "text" => Ok(Output::Text),
            "hex" => Ok(Output::Hex),
            "base64" | "b64" => Ok(Output::Base64),
            other => Err(format!(
                "invalid output {other:?}: expected \"text\", \"hex\", or \"base64\""
            )),
        }
    }

    fn render(self, bytes: &[u8]) -> Result<String, String> {
        match self {
            Output::Text => String::from_utf8(bytes.to_vec()).map_err(|e| {
                format!(
                    "the decompressed data is not valid UTF-8 text (bad byte at offset {}) \
                     — set output to \"hex\" or \"base64\" to view the raw bytes",
                    e.utf8_error().valid_up_to()
                )
            }),
            Output::Hex => Ok(to_hex(bytes)),
            Output::Base64 => Ok(B64.encode(bytes)),
        }
    }
}

/// The result of a decompression, before rendering.
#[derive(Clone, Debug)]
pub struct Decoded {
    /// The decompressed bytes.
    pub bytes: Vec<u8>,
    /// Size of the compressed payload that was fed to the Brotli decoder.
    pub compressed_bytes: usize,
    /// Which transport encoding was actually used (useful when `encoding = auto`).
    pub encoding: Encoding,
}

fn to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

/// Strip ASCII whitespace (so a line-wrapped paste keeps working).
fn squeeze(s: &str) -> String {
    s.chars().filter(|c| !c.is_ascii_whitespace()).collect()
}

/// Parse a hex string into bytes, ignoring ASCII whitespace and an optional
/// `0x` prefix. Rejects odd length and non-hex digits.
fn from_hex(s: &str) -> Result<Vec<u8>, String> {
    let cleaned = squeeze(s);
    let cleaned = cleaned
        .strip_prefix("0x")
        .or_else(|| cleaned.strip_prefix("0X"))
        .unwrap_or(&cleaned)
        .to_string();
    if cleaned.is_empty() {
        return Err("hex input is empty".into());
    }
    if cleaned.len() % 2 != 0 {
        return Err(format!(
            "hex input has an odd number of digits ({}); each byte needs two",
            cleaned.len()
        ));
    }
    (0..cleaned.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&cleaned[i..i + 2], 16)
                .map_err(|_| format!("invalid hex byte {:?}", &cleaned[i..i + 2]))
        })
        .collect()
}

/// Decode standard Base64, falling back to unpadded and URL-safe alphabets so a
/// payload lifted out of a URL or a JSON field still works.
fn from_base64(s: &str) -> Result<Vec<u8>, String> {
    let cleaned = squeeze(s);
    if cleaned.is_empty() {
        return Err("Base64 input is empty".into());
    }
    B64.decode(cleaned.as_bytes())
        .or_else(|_| B64_NP.decode(cleaned.trim_end_matches('=').as_bytes()))
        .or_else(|_| B64_URL.decode(cleaned.trim_end_matches('=').as_bytes()))
        .map_err(|e| format!("invalid Base64 input: {e}"))
}

/// Identify a non-Brotli compressed blob from its magic bytes, so the failure
/// message can name the codec AND the sibling tool that handles it. Only ever
/// consulted **after** a Brotli decode has already failed — Brotli has no magic
/// number, so an up-front sniff would false-reject perfectly valid input.
fn other_codec(bytes: &[u8]) -> Option<(&'static str, &'static str)> {
    let b = bytes;
    let starts = |sig: &[u8]| b.len() >= sig.len() && &b[..sig.len()] == sig;

    if starts(&[0x1F, 0x8B]) {
        return Some(("gzip", "gunzip"));
    }
    if starts(&[0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00]) {
        return Some(("xz", "lzma-decompress"));
    }
    if starts(&[0x5D, 0x00, 0x00]) {
        return Some(("raw LZMA", "lzma-decompress"));
    }
    if starts(&[0x04, 0x22, 0x4D, 0x18]) {
        return Some(("LZ4", "lz4-decompress"));
    }
    if starts(&[0x28, 0xB5, 0x2F, 0xFD]) {
        return Some(("zstd", "file-compressor"));
    }
    if starts(&[0x42, 0x5A, 0x68]) {
        return Some(("bzip2", "archive-extractor"));
    }
    if starts(&[0x50, 0x4B, 0x03, 0x04]) || starts(&[0x50, 0x4B, 0x05, 0x06]) {
        return Some(("ZIP", "unzip"));
    }
    if starts(&[0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C]) {
        return Some(("7-Zip", "7z-extract"));
    }
    if b.len() > 262 && &b[257..262] == b"ustar" {
        return Some(("tar", "archive-extractor"));
    }
    // zlib: CMF/FLG where the low nibble of CMF is 8 (deflate) and the 16-bit
    // big-endian header is a multiple of 31.
    if b.len() >= 2 && b[0] & 0x0F == 0x08 && ((b[0] as u16) << 8 | b[1] as u16) % 31 == 0 {
        return Some(("zlib", "raw-inflate"));
    }
    None
}

/// Run the Brotli decoder over `data` with the bomb guard applied.
fn brotli_decode(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    brotli::Decompressor::new(data, 4096)
        .take(MAX_OUTPUT_BYTES + 1)
        .read_to_end(&mut out)
        .map_err(|e| format!("Brotli decompression failed: {e}"))?;
    if out.len() as u64 > MAX_OUTPUT_BYTES {
        return Err(format!(
            "the decompressed data exceeds the {} MiB limit of this browser-local tool",
            MAX_OUTPUT_BYTES / (1024 * 1024)
        ));
    }
    Ok(out)
}

/// Turn a failed Brotli attempt into a message a user can act on: name the real
/// codec (and the sibling tool for it) when the magic bytes give it away.
fn brotli_error(bytes: &[u8], err: String) -> String {
    match other_codec(bytes) {
        Some((codec, tool)) => format!(
            "this payload is {codec}-compressed, not Brotli — use the {tool} tool instead"
        ),
        None => err,
    }
}

/// Decode `data` from `encoding` and Brotli-decompress it.
pub fn decode(data: &str, encoding: Encoding) -> Result<Decoded, String> {
    if data.trim().is_empty() {
        return Err("input is empty: paste a Brotli payload as Base64 or hex".into());
    }

    // Build the candidate transports to try, in order.
    let candidates: Vec<(Encoding, Result<Vec<u8>, String>)> = match encoding {
        Encoding::Base64 => vec![(Encoding::Base64, from_base64(data))],
        Encoding::Hex => vec![(Encoding::Hex, from_hex(data))],
        // A short Base64 string can be entirely hex characters, so `auto` must
        // not pick by shape — it decodes each candidate and keeps the one that
        // actually Brotli-decompresses.
        Encoding::Auto => vec![
            (Encoding::Hex, from_hex(data)),
            (Encoding::Base64, from_base64(data)),
        ],
    };

    let decoded: Vec<(Encoding, Vec<u8>)> = candidates
        .iter()
        .filter_map(|(e, r)| r.as_ref().ok().map(|b| (*e, b.clone())))
        .collect();

    if decoded.is_empty() {
        // Every transport decode failed. With an explicit encoding, surface that
        // decoder's own message; with auto, say what was tried.
        return Err(match encoding {
            Encoding::Auto => format!(
                "could not read the input as hex or Base64 — hex: {}; Base64: {}",
                candidates[0].1.as_ref().err().cloned().unwrap_or_default(),
                candidates[1].1.as_ref().err().cloned().unwrap_or_default()
            ),
            _ => candidates[0].1.as_ref().err().cloned().unwrap_or_default(),
        });
    }

    for (enc, bytes) in &decoded {
        if bytes.len() > MAX_INPUT_BYTES {
            return Err(format!(
                "compressed input is {} bytes, over the {} MiB limit of this browser-local tool",
                bytes.len(),
                MAX_INPUT_BYTES / (1024 * 1024)
            ));
        }
        if let Ok(out) = brotli_decode(bytes) {
            return Ok(Decoded {
                bytes: out,
                compressed_bytes: bytes.len(),
                encoding: *enc,
            });
        }
    }

    // Nothing decompressed. Diagnose against the first candidate that at least
    // decoded as a transport (for `auto` that is hex when the paste is all-hex,
    // otherwise Base64).
    let (_, bytes) = &decoded[0];
    let err = brotli_decode(bytes).unwrap_err();
    Err(brotli_error(bytes, err))
}

/// Format the size summary prepended to the payload when `stats` is on.
fn stats_block(compressed: usize, decompressed: usize) -> String {
    let mut s = format!(
        "Compressed:   {compressed} bytes\nDecompressed: {decompressed} bytes\n"
    );
    if compressed > 0 && decompressed > 0 {
        let ratio = decompressed as f64 / compressed as f64;
        let saved = (1.0 - compressed as f64 / decompressed as f64) * 100.0;
        s.push_str(&format!(
            "Ratio:        {ratio:.2}x (decompressed / compressed)\nSpace saved:  {saved:.1}%\n"
        ));
    }
    s.push_str("\n");
    s
}

/// Decompress a pasted Brotli payload and render the result.
///
/// - `data` — the Brotli payload, encoded per `encoding`.
/// - `encoding` (`"auto"` | `"base64"` | `"hex"`, blank → `"auto"`).
/// - `output` (`"text"` | `"hex"` | `"base64"`, blank → `"text"`).
/// - `stats` — prepend a compressed/decompressed size summary.
pub fn run(data: &str, encoding: &str, output: &str, stats: bool) -> Result<String, String> {
    let enc = Encoding::parse(encoding)?;
    let out_fmt = Output::parse(output)?;
    let decoded = decode(data, enc)?;
    let rendered = out_fmt.render(&decoded.bytes)?;
    if stats {
        Ok(format!(
            "{}{rendered}",
            stats_block(decoded.compressed_bytes, decoded.bytes.len())
        ))
    } else {
        Ok(rendered)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn br(data: &[u8]) -> Vec<u8> {
        let mut params = brotli::enc::BrotliEncoderParams::default();
        params.quality = 9;
        params.lgwin = 22;
        let mut out = Vec::new();
        brotli::BrotliCompress(&mut &data[..], &mut out, &params).unwrap();
        out
    }

    fn br_b64(data: &[u8]) -> String {
        B64.encode(br(data))
    }

    // ── happy paths ─────────────────────────────────────────────────────────

    #[test]
    fn decompresses_base64_payload_to_text() {
        let payload = br_b64(b"{\"hello\":\"brotli\"}");
        assert_eq!(
            run(&payload, "base64", "text", false).unwrap(),
            "{\"hello\":\"brotli\"}"
        );
    }

    #[test]
    fn decompresses_hex_payload_to_text() {
        let hex = to_hex(&br(b"hex in, text out"));
        assert_eq!(run(&hex, "hex", "text", false).unwrap(), "hex in, text out");
    }

    #[test]
    fn auto_detects_base64() {
        let payload = br_b64(b"auto-detected as Base64");
        assert_eq!(
            run(&payload, "auto", "text", false).unwrap(),
            "auto-detected as Base64"
        );
    }

    #[test]
    fn auto_detects_hex() {
        let hex = to_hex(&br(b"auto-detected as hex"));
        assert_eq!(
            run(&hex, "auto", "text", false).unwrap(),
            "auto-detected as hex"
        );
        assert_eq!(decode(&hex, Encoding::Auto).unwrap().encoding, Encoding::Hex);
    }

    #[test]
    fn auto_prefers_whichever_actually_decompresses() {
        // A payload whose Base64 form happens to use only hex characters would
        // decode as hex too — `auto` must keep the one that really inflates.
        let payload = br_b64(b"ambiguity resolved by decoding, not by shape");
        let d = decode(&payload, Encoding::Auto).unwrap();
        assert_eq!(d.bytes, b"ambiguity resolved by decoding, not by shape");
    }

    #[test]
    fn blank_options_fall_back_to_defaults() {
        let payload = br_b64(b"defaults are auto + text");
        assert_eq!(
            run(&payload, "", "", false).unwrap(),
            "defaults are auto + text"
        );
    }

    #[test]
    fn accepts_line_wrapped_and_prefixed_input() {
        let raw = br(b"line wrapping must not matter");
        let wrapped = B64
            .encode(&raw)
            .as_bytes()
            .chunks(20)
            .map(|c| String::from_utf8(c.to_vec()).unwrap())
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(
            run(&wrapped, "base64", "text", false).unwrap(),
            "line wrapping must not matter"
        );
        let hex = format!("0x{}", to_hex(&raw));
        assert_eq!(
            run(&hex, "hex", "text", false).unwrap(),
            "line wrapping must not matter"
        );
    }

    #[test]
    fn url_safe_base64_is_accepted() {
        let raw = br(b"payload lifted straight out of a query string ~~~ \xff\xfe");
        let url = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&raw);
        assert_eq!(
            decode(&url, Encoding::Base64).unwrap().bytes,
            b"payload lifted straight out of a query string ~~~ \xff\xfe"
        );
    }

    #[test]
    fn binary_output_as_hex_and_base64() {
        let raw: Vec<u8> = (0..=255u8).collect();
        let payload = br_b64(&raw);
        let hex = run(&payload, "base64", "hex", false).unwrap();
        assert_eq!(hex, to_hex(&raw));
        let b64 = run(&payload, "base64", "base64", false).unwrap();
        assert_eq!(b64, B64.encode(&raw));
    }

    #[test]
    fn stats_report_sizes_and_ratio() {
        let raw = b"A".repeat(1000);
        let payload = br_b64(&raw);
        let out = run(&payload, "base64", "text", true).unwrap();
        assert!(out.contains("Decompressed: 1000 bytes"), "{out}");
        assert!(out.contains("Ratio:"), "{out}");
        assert!(out.contains("Space saved:"), "{out}");
        // The payload still follows the summary, after a blank line.
        assert!(out.ends_with(&String::from_utf8(raw).unwrap()), "{out}");
    }

    #[test]
    fn empty_payload_round_trips() {
        let payload = br_b64(b"");
        assert_eq!(run(&payload, "base64", "text", false).unwrap(), "");
    }

    #[test]
    fn decompresses_a_large_repetitive_payload() {
        let raw = b"gizza brotli decompress ".repeat(20_000);
        let compressed = br(&raw);
        assert!(compressed.len() < raw.len() / 50);
        let d = decode(&B64.encode(&compressed), Encoding::Base64).unwrap();
        assert_eq!(d.bytes.len(), raw.len());
        assert_eq!(d.compressed_bytes, compressed.len());
    }

    // ── error paths ─────────────────────────────────────────────────────────

    #[test]
    fn empty_input_is_an_error() {
        let e = run("   ", "auto", "text", false).unwrap_err();
        assert!(e.contains("empty"), "{e}");
    }

    #[test]
    fn invalid_encoding_is_an_error() {
        let e = run("AA", "rot13", "text", false).unwrap_err();
        assert!(e.contains("invalid encoding"), "{e}");
        assert!(e.contains("auto"), "{e}");
    }

    #[test]
    fn invalid_output_is_an_error() {
        let e = run("AA", "auto", "yaml", false).unwrap_err();
        assert!(e.contains("invalid output"), "{e}");
    }

    #[test]
    fn odd_length_hex_is_an_error() {
        let e = run("abc", "hex", "text", false).unwrap_err();
        assert!(e.contains("odd number of digits"), "{e}");
    }

    #[test]
    fn non_base64_input_is_an_error() {
        let e = run("not valid base64 !!!", "base64", "text", false).unwrap_err();
        assert!(e.contains("invalid Base64 input"), "{e}");
    }

    #[test]
    fn gzip_payload_names_the_right_tool() {
        // gzip magic + a deliberately broken body: the Brotli attempt fails
        // first, then the magic bytes explain why.
        let gz = [0x1F, 0x8B, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03];
        let e = decode(&B64.encode(gz), Encoding::Base64).unwrap_err();
        assert!(e.contains("gzip"), "{e}");
        assert!(e.contains("gunzip"), "{e}");
    }

    #[test]
    fn zip_payload_names_the_right_tool() {
        let zip = [0x50, 0x4B, 0x03, 0x04, 0x14, 0x00, 0x00, 0x00, 0x08, 0x00];
        let e = decode(&B64.encode(zip), Encoding::Base64).unwrap_err();
        assert!(e.contains("ZIP") && e.contains("unzip"), "{e}");
    }

    #[test]
    fn zstd_payload_names_the_right_tool() {
        let zst = [0x28, 0xB5, 0x2F, 0xFD, 0x24, 0x0A, 0x00, 0x00, 0x00, 0x00];
        let e = decode(&B64.encode(zst), Encoding::Base64).unwrap_err();
        assert!(e.contains("zstd") && e.contains("file-compressor"), "{e}");
    }

    #[test]
    fn lz4_and_xz_payloads_name_their_tools() {
        let lz4 = [0x04, 0x22, 0x4D, 0x18, 0x60, 0x40, 0x82, 0x00, 0x00, 0x00];
        let e = decode(&B64.encode(lz4), Encoding::Base64).unwrap_err();
        assert!(e.contains("LZ4") && e.contains("lz4-decompress"), "{e}");

        let xz = [0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00, 0x00, 0x04, 0xE6, 0xD6];
        let e = decode(&B64.encode(xz), Encoding::Base64).unwrap_err();
        assert!(e.contains("xz") && e.contains("lzma-decompress"), "{e}");
    }

    #[test]
    fn zlib_payload_names_the_right_tool() {
        // 0x78 0x9C is the ubiquitous zlib default-compression header.
        let z = [0x78, 0x9C, 0x03, 0x00, 0x00, 0x00, 0x00, 0x01];
        let e = decode(&B64.encode(z), Encoding::Base64).unwrap_err();
        assert!(e.contains("zlib") && e.contains("raw-inflate"), "{e}");
    }

    #[test]
    fn garbage_that_is_not_brotli_errors_without_a_codec_name() {
        // Random-looking bytes with no known magic: the raw decoder error stands.
        let junk: Vec<u8> = (0..64u8).map(|i| i.wrapping_mul(37).wrapping_add(11)).collect();
        let e = decode(&B64.encode(&junk), Encoding::Base64).unwrap_err();
        assert!(e.contains("Brotli decompression failed"), "{e}");
    }

    #[test]
    fn truncated_brotli_stream_errors() {
        let compressed = br(&b"a payload long enough to span several blocks ".repeat(200));
        let truncated = &compressed[..compressed.len() / 2];
        let e = decode(&B64.encode(truncated), Encoding::Base64).unwrap_err();
        assert!(e.contains("Brotli decompression failed"), "{e}");
    }

    #[test]
    fn non_utf8_output_as_text_explains_the_fix() {
        let payload = br_b64(&[0xFF, 0xFE, 0x00, 0x01]);
        let e = run(&payload, "base64", "text", false).unwrap_err();
        assert!(e.contains("not valid UTF-8"), "{e}");
        assert!(e.contains("hex") && e.contains("base64"), "{e}");
    }

    #[test]
    fn auto_reports_both_transports_when_neither_decodes() {
        let e = decode("!!! neither !!!", Encoding::Auto).unwrap_err();
        assert!(e.contains("hex") && e.contains("Base64"), "{e}");
    }

    #[test]
    fn oversized_compressed_input_is_rejected() {
        // Incompressible-ish bytes past the 8 MiB input cap.
        let big: Vec<u8> = (0..MAX_INPUT_BYTES + 16)
            .map(|i| (i.wrapping_mul(2654435761) >> 13) as u8)
            .collect();
        let e = decode(&B64.encode(&big), Encoding::Base64).unwrap_err();
        assert!(e.contains("over the 8 MiB limit"), "{e}");
    }
}
