//! gizza-ai/raw-inflate core — decompress headerless **raw DEFLATE** (RFC 1951)
//! back to the original bytes. No gzip (RFC 1952) and no zlib (RFC 1950)
//! wrapper is expected — just the bare deflate bit stream. No wafer/wasm-bindgen
//! deps. Pure-Rust `flate2` (miniz_oxide backend, wasm32-safe).
//!
//! Raw deflate has no magic bytes, no checksum (no CRC-32 / Adler-32) and no
//! length/filename header — it is the smallest of the three flate framings and
//! is what other formats embed (ZIP entries, PNG IDAT after the zlib header,
//! HTTP `Content-Encoding: deflate` bodies that omit the wrapper). This inverts
//! `raw-deflate` / any `DeflateEncoder` output.

use std::io::Read;

use flate2::read::DeflateDecoder;

/// Max decompressed size (guards against DEFLATE decompression bombs). Kept well
/// below the 64 MiB wasm sandbox so a bomb returns a clean error instead of
/// OOM-trapping while the bytes are base64-encoded into the output envelope.
const MAX_OUTPUT_BYTES: u64 = 16 * 1024 * 1024; // 16 MiB

/// Inflate a raw (headerless) DEFLATE stream back to its original bytes.
pub fn inflate(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut dec = DeflateDecoder::new(data);
    let mut out = Vec::new();
    (&mut dec)
        .take(MAX_OUTPUT_BYTES + 1)
        .read_to_end(&mut out)
        .map_err(|e| format!("not a valid raw DEFLATE stream: {e}"))?;
    if out.len() as u64 > MAX_OUTPUT_BYTES {
        return Err("decompressed data is too large".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn deflate(data: &[u8], level: u32) -> Vec<u8> {
        let mut enc =
            flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::new(level));
        enc.write_all(data).unwrap();
        enc.finish().unwrap()
    }

    #[test]
    fn round_trips() {
        let data = b"hello, raw deflate world! ".repeat(50);
        let raw = deflate(&data, 6);
        assert_eq!(inflate(&raw).unwrap(), data);
    }

    #[test]
    fn inflates_highly_compressed() {
        let data = vec![b'A'; 10_000];
        let raw = deflate(&data, 9);
        assert!(raw.len() < data.len() / 10);
        assert_eq!(inflate(&raw).unwrap(), data);
    }

    #[test]
    fn rejects_decompression_bomb() {
        // A few KiB of deflate expands past the cap; must error cleanly rather
        // than grow linear memory until the wasm sandbox OOM-traps.
        let data = vec![0u8; MAX_OUTPUT_BYTES as usize + 1];
        let raw = deflate(&data, 9);
        assert!(raw.len() < 1024 * 1024, "bomb input should stay tiny");
        let err = inflate(&raw).unwrap_err();
        assert!(err.contains("too large"), "got: {err}");
    }

    #[test]
    fn empty_input_round_trips() {
        // empty -> deflate -> inflate should give empty back
        let raw = deflate(b"", 6);
        assert_eq!(inflate(&raw).unwrap(), Vec::<u8>::new());
    }

    #[test]
    fn rejects_gzip_stream() {
        // A gzip stream (1F 8B header) is NOT raw deflate and must error, not
        // silently decode garbage.
        let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::new(6));
        enc.write_all(b"the quick brown fox").unwrap();
        let gz = enc.finish().unwrap();
        assert_eq!(&gz[0..2], &[0x1F, 0x8B]);
        assert!(inflate(&gz).is_err(), "gzip wrapper must be rejected by raw inflate");
    }

    // NOTE on truncation: flate2's streaming `read::DeflateDecoder` +
    // `read_to_end` is lenient about a stream that ends early — once a final
    // (BFINAL) block has been consumed it stops, and miniz_oxide does not treat
    // a missing trailing byte as a hard error. So truncating a *valid* stream is
    // NOT a reliable error case and is intentionally not asserted; structurally
    // invalid bit patterns (gzip wrapper, random garbage below) ARE rejected.

    #[test]
    fn rejects_random_garbage() {
        // Random bytes are very unlikely to be a complete valid deflate stream.
        let garbage: Vec<u8> = (0..256u32).map(|i| (i * 37 + 11) as u8).collect();
        assert!(inflate(&garbage).is_err(), "garbage must not decode");
    }
}
