//! lznt1-decompress core — decompress an LZNT1 blob (the format produced by
//! Windows `RtlCompressBuffer` / `RtlDecompressBuffer` with
//! `COMPRESSION_FORMAT_LZNT1`). LZNT1 is the legacy NTFS / registry-hive
//! compression scheme and is still used widely in forensics and malware
//! analysis: compressed registry-hive cells, hibernation-file pages, and the
//! configuration blobs stuffed into many malware families are all LZNT1.
//!
//! The wire format (see [MS-XCA] §2.5 / the documented `RtlDecompressBuffer`
//! behaviour) is a sequence of **chunks**. Each chunk starts with a 16-bit
//! little-endian header:
//!   * bit 15      — set when the chunk body is compressed (else stored verbatim);
//!   * bits 12..14 — a fixed `0b011` signature on compressed chunks;
//!   * bits 0..11  — (body length − 1), the number of bytes that follow.
//! A header word of `0x0000` marks the end of the stream.
//!
//! A compressed chunk body is a series of **flag groups**: one flag byte then up
//! to eight tokens. Bit *i* of the flag byte (LSB first) selects token *i*:
//! a clear bit is a single literal byte; a set bit is a 16-bit back-reference
//! whose length/displacement split depends on how many bytes have already been
//! emitted *into the current chunk* (the window grows as the chunk decodes).
//!
//! Pure compute, no wafer/wasm-bindgen deps — shared by the chat skill block and
//! the web page. Runs on every backend including the chat Service Worker.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;

/// How to interpret the supplied compressed blob.
#[derive(Clone, Copy, PartialEq, Eq)]
enum InFmt {
    /// A hex string (e.g. `1ab0 0020 ...` or `0x1ab0...`), case-insensitive — the default.
    Hex,
    /// Standard Base64 (RFC 4648), padding optional on decode.
    Base64,
}

impl InFmt {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "hex" => Ok(InFmt::Hex),
            "base64" | "b64" => Ok(InFmt::Base64),
            other => Err(format!(
                "invalid input_encoding {other:?}: expected \"hex\" or \"base64\""
            )),
        }
    }

    fn to_bytes(self, s: &str) -> Result<Vec<u8>, String> {
        match self {
            InFmt::Hex => parse_hex(s),
            InFmt::Base64 => {
                let cleaned: String = s.chars().filter(|c| !c.is_ascii_whitespace()).collect();
                B64.decode(cleaned.as_bytes())
                    .or_else(|_| {
                        base64::engine::general_purpose::STANDARD_NO_PAD
                            .decode(cleaned.trim_end_matches('=').as_bytes())
                    })
                    .map_err(|e| format!("invalid Base64 input: {e}"))
            }
        }
    }
}

/// How to render the decompressed bytes back to a string.
#[derive(Clone, Copy, PartialEq, Eq)]
enum OutFmt {
    /// UTF-8 text — fails if the output isn't valid UTF-8.
    Text,
    /// A lowercase hex string — the default (decompressed blobs are usually binary).
    Hex,
    /// Standard Base64 (RFC 4648).
    Base64,
}

impl OutFmt {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "hex" => Ok(OutFmt::Hex),
            "text" => Ok(OutFmt::Text),
            "base64" | "b64" => Ok(OutFmt::Base64),
            other => Err(format!(
                "invalid output_encoding {other:?}: expected \"hex\", \"text\", or \"base64\""
            )),
        }
    }

    fn from_bytes(self, bytes: &[u8]) -> Result<String, String> {
        match self {
            OutFmt::Text => String::from_utf8(bytes.to_vec()).map_err(|_| {
                "the decompressed bytes are not valid UTF-8 — set output_encoding to 'hex' or 'base64' to view them".into()
            }),
            OutFmt::Hex => Ok(to_hex(bytes)),
            OutFmt::Base64 => Ok(B64.encode(bytes)),
        }
    }
}

/// Parse a hex string into bytes, ignoring ASCII whitespace and an optional
/// `0x` prefix. Rejects odd length or non-hex digits.
fn parse_hex(s: &str) -> Result<Vec<u8>, String> {
    let cleaned: String = s
        .trim()
        .trim_start_matches("0x")
        .trim_start_matches("0X")
        .chars()
        .filter(|c| !c.is_ascii_whitespace())
        .collect();
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

fn to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

/// Decompress one compressed chunk body. `body` is the bytes following a
/// compressed chunk header (excluding the header itself). Returns the chunk's
/// decompressed bytes.
fn decompress_chunk(body: &[u8]) -> Result<Vec<u8>, String> {
    let mut out: Vec<u8> = Vec::new();
    let mut i = 0usize;
    while i < body.len() {
        let flags = body[i];
        i += 1;
        for bit in 0..8u8 {
            if i >= body.len() {
                break;
            }
            if flags & (1 << bit) == 0 {
                // Literal byte.
                out.push(body[i]);
                i += 1;
            } else {
                // 16-bit back-reference token.
                if i + 1 >= body.len() {
                    return Err(
                        "truncated LZNT1 chunk: back-reference token is missing its second byte"
                            .into(),
                    );
                }
                let token = (body[i] as u16) | ((body[i + 1] as u16) << 8);
                i += 2;

                // The length/displacement split depends on the current window
                // size (bytes already emitted into this chunk).
                let mut pos = out.len().wrapping_sub(1);
                let mut length_mask: u16 = 0x0FFF;
                let mut disp_shift: u32 = 12;
                while pos >= 0x10 {
                    length_mask >>= 1;
                    disp_shift -= 1;
                    pos >>= 1;
                }
                let length = ((token & length_mask) as usize) + 3;
                let disp = ((token >> disp_shift) as usize) + 1;

                if disp > out.len() {
                    return Err(format!(
                        "invalid LZNT1 back-reference: displacement {disp} exceeds the {} bytes decoded so far",
                        out.len()
                    ));
                }
                // Copy `length` bytes from `disp` behind the cursor, one at a
                // time so overlapping (run-length) copies work.
                let start = out.len() - disp;
                for k in 0..length {
                    let b = out[start + k];
                    out.push(b);
                }
            }
        }
    }
    Ok(out)
}

/// Decompress a full LZNT1 blob (one or more chunks) into the original bytes.
pub fn decompress(data: &[u8]) -> Result<Vec<u8>, String> {
    if data.is_empty() {
        return Err("input is empty: provide an LZNT1-compressed blob".into());
    }
    let mut out: Vec<u8> = Vec::new();
    let mut i = 0usize;
    let mut saw_chunk = false;
    while i + 2 <= data.len() {
        let header = (data[i] as u16) | ((data[i + 1] as u16) << 8);
        // A zero header word terminates the stream (trailing padding).
        if header == 0 {
            break;
        }
        i += 2;
        let body_len = ((header & 0x0FFF) as usize) + 1;
        let compressed = header & 0x8000 != 0;
        if i + body_len > data.len() {
            return Err(format!(
                "truncated LZNT1 stream: chunk header claims {body_len} body bytes but only {} remain",
                data.len() - i
            ));
        }
        let body = &data[i..i + body_len];
        i += body_len;
        if compressed {
            out.extend_from_slice(&decompress_chunk(body)?);
        } else {
            // Stored (uncompressed) chunk — copy the body verbatim.
            out.extend_from_slice(body);
        }
        saw_chunk = true;
    }
    if !saw_chunk {
        return Err(
            "input is not a valid LZNT1 stream: too short for a chunk header (need at least 2 bytes)"
                .into(),
        );
    }
    Ok(out)
}

/// Decompress `data` (a compressed LZNT1 blob in `input_encoding`) and render
/// the result in `output_encoding`.
///
/// - `input_encoding` (`"hex"` | `"base64"`, blank → `"hex"`): how the blob is encoded.
/// - `output_encoding` (`"hex"` | `"text"` | `"base64"`, blank → `"hex"`): how the
///   decompressed bytes are rendered. `"text"` errors if the output isn't valid UTF-8.
pub fn run(data: &str, input_encoding: &str, output_encoding: &str) -> Result<String, String> {
    let in_fmt = InFmt::parse(input_encoding)?;
    let out_fmt = OutFmt::parse(output_encoding)?;
    let bytes = in_fmt.to_bytes(data)?;
    let decompressed = decompress(&bytes)?;
    out_fmt.from_bytes(&decompressed)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Build a single compressed-chunk LZNT1 blob from a raw chunk body.
    fn wrap_compressed(body: &[u8]) -> Vec<u8> {
        let header: u16 = 0xB000 | ((body.len() as u16) - 1);
        let mut v = vec![(header & 0xFF) as u8, (header >> 8) as u8];
        v.extend_from_slice(body);
        v
    }

    #[test]
    fn literals_only() {
        // flag = 0x00 (all literals), bytes "ABC".
        let body = [0x00, b'A', b'B', b'C'];
        let blob = wrap_compressed(&body);
        let bytes = decompress(&blob).unwrap();
        assert_eq!(bytes, b"ABC");
    }

    #[test]
    fn back_reference_run() {
        // "ABC" then a back-reference (length=3, disp=3) → "ABCABC".
        // token = 0x2000: low 12 bits = 0 → length 3; high 4 bits = 2 → disp 3.
        let body = [0x08, b'A', b'B', b'C', 0x00, 0x20];
        let blob = wrap_compressed(&body);
        let bytes = decompress(&blob).unwrap();
        assert_eq!(bytes, b"ABCABC");
    }

    #[test]
    fn overlapping_run() {
        // "A" then back-reference length=5, disp=1 → "AAAAAA" (RLE-style overlap).
        // token: low 12 bits = length-3 = 2; pos=len-1=0 so full 12/4 split,
        // disp-1 = 0 → high bits 0. token = 0x0002.
        let body = [0x02, b'A', 0x02, 0x00];
        let blob = wrap_compressed(&body);
        let bytes = decompress(&blob).unwrap();
        assert_eq!(bytes, b"AAAAAA");
    }

    #[test]
    fn stored_chunk() {
        // Uncompressed chunk: header bit 15 clear, signature bits clear.
        let body = b"raw bytes";
        let header: u16 = (body.len() as u16) - 1; // not compressed
        let mut blob = vec![(header & 0xFF) as u8, (header >> 8) as u8];
        blob.extend_from_slice(body);
        let bytes = decompress(&blob).unwrap();
        assert_eq!(bytes, b"raw bytes");
    }

    #[test]
    fn run_hex_to_text() {
        // hex of the "ABCABC" blob, output as text.
        let blob = wrap_compressed(&[0x08, b'A', b'B', b'C', 0x00, 0x20]);
        let hex = to_hex(&blob);
        let out = run(&hex, "hex", "text").unwrap();
        assert_eq!(out, "ABCABC");
    }

    #[test]
    fn run_base64_input() {
        let blob = wrap_compressed(&[0x00, b'h', b'i']);
        let b64 = B64.encode(&blob);
        let out = run(&b64, "base64", "text").unwrap();
        assert_eq!(out, "hi");
    }

    #[test]
    fn longer_realistic_roundtrip() {
        // "abcabcabcabc": literals "abc", then back-reference length=9, disp=3.
        // pos after 3 bytes = 2 → full 12/4 split. length-3=6, disp-1=2.
        // token = (2 << 12) | 6 = 0x2006.
        let body = [0x08, b'a', b'b', b'c', 0x06, 0x20];
        let blob = wrap_compressed(&body);
        let bytes = decompress(&blob).unwrap();
        assert_eq!(bytes, b"abcabcabcabc");
    }

    #[test]
    fn empty_is_error() {
        assert!(run("", "hex", "hex").is_err());
    }

    #[test]
    fn bad_input_encoding() {
        assert!(run("ab", "rot13", "hex").is_err());
    }

    #[test]
    fn bad_output_encoding() {
        let blob = to_hex(&wrap_compressed(&[0x00, b'x']));
        assert!(run(&blob, "hex", "yaml").is_err());
    }

    #[test]
    fn odd_hex_is_error() {
        assert!(run("abc", "hex", "hex").is_err());
    }

    #[test]
    fn truncated_token_is_error() {
        // Compressed chunk whose flag claims a back-ref but body ends early.
        // flag = 0x01 (token 0 is a back-ref) followed by only one byte.
        let blob = wrap_compressed(&[0x01, 0x00]);
        assert!(decompress(&blob).is_err());
    }
}
