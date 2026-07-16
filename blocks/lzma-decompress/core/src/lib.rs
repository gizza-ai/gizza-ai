//! gizza-ai/lzma-decompress core — decompress an LZMA / `.xz` stream back to its
//! original bytes. Pure-Rust `lzma-rust2` (a port of tukaani's xz-for-java) — no
//! C bindings, wasm32-safe. The inverse of `lzma-compress` (and of the `xz`
//! command-line tool).
//!
//! Two on-disk formats are auto-detected from the leading magic bytes:
//!  * **`.xz`** — the modern XZ container (LZMA2 + integrity check). Magic
//!    `FD 37 7A 58 5A 00`. Multiple concatenated streams are decoded as one.
//!  * **`.lzma` / "alone"** — the legacy raw-LZMA format produced by `lzma` /
//!    `xz --format=lzma` and old 7-Zip. Header = 1 properties byte, a 4-byte
//!    little-endian dictionary size, then an 8-byte uncompressed size.
//!
//! Both decode with any standard xz/lzma/7-Zip implementation; this is the
//! lossless inverse of the bytes those tools emit.

use std::io::Read;

use lzma_rust2::{LzmaReader, XzReader};

/// Max decompressed size (guards against xz/lzma decompression bombs). Kept well
/// below the 64 MiB wasm sandbox so a bomb returns a clean error instead of
/// OOM-trapping while the bytes are base64-encoded into the output envelope.
const MAX_OUTPUT_BYTES: u64 = 16 * 1024 * 1024; // 16 MiB

/// The `.xz` stream-header magic: `FD '7' 'z' 'X' 'Z' 00`.
const XZ_MAGIC: [u8; 6] = [0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00];

/// Which container the input was recognised as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// Modern XZ container (`.xz`).
    Xz,
    /// Legacy raw-LZMA ("alone") stream (`.lzma`).
    Lzma,
}

impl Format {
    pub fn label(self) -> &'static str {
        match self {
            Format::Xz => "xz",
            Format::Lzma => "lzma",
        }
    }
}

/// Decompress an `.xz` or legacy `.lzma` stream, auto-detecting the format from
/// the magic bytes. Returns the original bytes plus which format was detected.
pub fn lzma_decompress(data: &[u8]) -> Result<(Vec<u8>, Format), String> {
    if data.len() >= XZ_MAGIC.len() && data[..XZ_MAGIC.len()] == XZ_MAGIC {
        let out = decode_xz(data)?;
        Ok((out, Format::Xz))
    } else if looks_like_lzma_alone(data) {
        let out = decode_lzma_alone(data)?;
        Ok((out, Format::Lzma))
    } else {
        Err("input is not a recognised .xz or .lzma stream (bad magic bytes)".to_string())
    }
}

/// Heuristic for the legacy `.lzma` ("alone") header: a valid properties byte
/// (`lc + lp*9 + pb*45`, with `lc<=8`, `lp<=4`, `pb<=4` → byte < 225) followed by
/// at least the 4-byte dict size + 8-byte uncompressed size header.
fn looks_like_lzma_alone(data: &[u8]) -> bool {
    // 1 props + 4 dict-size + 8 uncomp-size = 13-byte header minimum.
    data.len() >= 13 && data[0] < 225
}

fn decode_xz(data: &[u8]) -> Result<Vec<u8>, String> {
    // allow_multiple_streams = true → concatenated .xz streams decode as one.
    let mut dec = XzReader::new(std::io::Cursor::new(data), true);
    let mut out = Vec::new();
    (&mut dec)
        .take(MAX_OUTPUT_BYTES + 1)
        .read_to_end(&mut out)
        .map_err(|e| format!("xz decompression failed: {e}"))?;
    if out.len() as u64 > MAX_OUTPUT_BYTES {
        return Err("decompressed data is too large".into());
    }
    Ok(out)
}

fn decode_lzma_alone(data: &[u8]) -> Result<Vec<u8>, String> {
    // Cap the decoder's dictionary allocation. `new_mem_limit` sizes the
    // dictionary from the .lzma header's declared dict-size field; with no cap a
    // crafted header can declare a multi-GB dictionary that gets allocated up
    // front — before any output — and OOM-traps the 64 MiB wasm sandbox, which
    // the output `.take()` cap below cannot guard. 32 MiB comfortably covers any
    // dictionary a file actually decodable within the sandbox could use; a larger
    // declaration is rejected here as a clean error instead of trapping.
    const MAX_DICT_BYTES: u32 = 32 * 1024 * 1024;
    let mut dec = LzmaReader::new_mem_limit(std::io::Cursor::new(data), MAX_DICT_BYTES, None)
        .map_err(|e| format!("invalid .lzma header: {e}"))?;
    let mut out = Vec::new();
    (&mut dec)
        .take(MAX_OUTPUT_BYTES + 1)
        .read_to_end(&mut out)
        .map_err(|e| format!("lzma decompression failed: {e}"))?;
    if out.len() as u64 > MAX_OUTPUT_BYTES {
        return Err("decompressed data is too large".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lzma_rust2::{XzOptions, XzWriter};
    use std::io::Write;

    fn xz_compress(data: &[u8]) -> Vec<u8> {
        let mut opts = XzOptions::with_preset(1);
        // preset 1 already uses the hash-chain match finder; cap the dict for
        // constrained runtimes (mirrors the lzma-compress encoder).
        if opts.lzma_options.dict_size > (1 << 23) {
            opts.lzma_options.dict_size = 1 << 23;
        }
        let mut enc = XzWriter::new(Vec::new(), opts).unwrap();
        enc.write_all(data).unwrap();
        enc.finish().unwrap()
    }

    #[test]
    fn round_trips_xz() {
        let data = b"hello, lzma/xz world! ".repeat(50);
        let xz = xz_compress(&data);
        let (out, fmt) = lzma_decompress(&xz).unwrap();
        assert_eq!(out, data);
        assert_eq!(fmt, Format::Xz);
    }

    #[test]
    fn round_trips_empty_xz() {
        let xz = xz_compress(b"");
        let (out, fmt) = lzma_decompress(&xz).unwrap();
        assert!(out.is_empty());
        assert_eq!(fmt, Format::Xz);
    }

    #[test]
    fn rejects_decompression_bomb() {
        // A highly compressible payload expands past the cap; must error cleanly
        // rather than grow linear memory until the wasm sandbox OOM-traps.
        let data = vec![0u8; MAX_OUTPUT_BYTES as usize + 1];
        let xz = xz_compress(&data);
        assert!(xz.len() < 1024 * 1024, "bomb input should stay tiny");
        let err = lzma_decompress(&xz).unwrap_err();
        assert!(err.contains("too large"), "got: {err}");
    }

    #[test]
    fn round_trips_binary() {
        let data: Vec<u8> = (0u32..5000).map(|i| (i % 256) as u8).collect();
        let xz = xz_compress(&data);
        let (out, _) = lzma_decompress(&xz).unwrap();
        assert_eq!(out, data);
    }

    #[test]
    fn detects_xz_magic_bad_body_errors() {
        // A buffer with the xz magic but garbage body must fail cleanly, not panic.
        let mut bad = XZ_MAGIC.to_vec();
        bad.extend_from_slice(&[0u8; 32]);
        assert!(lzma_decompress(&bad).is_err());
    }

    #[test]
    fn rejects_non_lzma() {
        // Bytes that match neither magic → clear error (never a panic).
        let err = lzma_decompress(b"this is just plain text, not compressed at all!!").unwrap_err();
        assert!(!err.is_empty());
    }

    #[test]
    fn rejects_truncated() {
        assert!(lzma_decompress(b"").is_err());
        assert!(lzma_decompress(&[0xFD, 0x37]).is_err());
    }
}
