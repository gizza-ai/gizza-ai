//! gizza-ai/raw-deflate core — compress bytes with headerless **raw DEFLATE**
//! (RFC 1951): no gzip (RFC 1952) and no zlib (RFC 1950) wrapper, just the bare
//! deflate bit stream. No wafer/wasm-bindgen deps. Pure-Rust `flate2`
//! (miniz_oxide backend, wasm32-safe).
//!
//! Raw deflate has no magic bytes, no checksum (no CRC-32 / Adler-32) and no
//! length/filename header — it is the smallest of the three flate framings and
//! is what you embed inside other container formats (e.g. ZIP entries, PNG
//! IDAT after the zlib header, HTTP `Content-Encoding: deflate` bodies that omit
//! the wrapper). Inflate it again with any raw-inflate / `DeflateDecoder`.

use std::io::Write;

use flate2::{write::DeflateEncoder, Compression};

/// Compress `data` to a raw (headerless) DEFLATE stream. `level` is the deflate
/// level 1-9 (clamped; 6 is the usual default).
pub fn deflate(data: &[u8], level: u32) -> Result<Vec<u8>, String> {
    let lvl = level.clamp(1, 9);
    let mut enc = DeflateEncoder::new(Vec::new(), Compression::new(lvl));
    enc.write_all(data).map_err(|e| format!("deflate write failed: {e}"))?;
    enc.finish().map_err(|e| format!("deflate finish failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn inflate(bytes: &[u8]) -> Vec<u8> {
        let mut dec = flate2::read::DeflateDecoder::new(bytes);
        let mut out = Vec::new();
        dec.read_to_end(&mut out).unwrap();
        out
    }

    #[test]
    fn round_trips() {
        let data = b"hello, raw deflate world! ".repeat(50);
        let raw = deflate(&data, 6).unwrap();
        assert_eq!(inflate(&raw), data);
    }

    #[test]
    fn has_no_gzip_header() {
        // gzip starts 1F 8B. Raw deflate has no fixed magic — assert it's NOT a
        // gzip stream (the whole point of "headerless").
        let raw = deflate(b"the quick brown fox jumps over the lazy dog", 6).unwrap();
        assert_ne!(&raw[0..2], &[0x1F, 0x8B], "must not be gzip-wrapped");
    }

    #[test]
    fn compresses_repetitive_data() {
        let data = vec![b'A'; 10_000];
        let raw = deflate(&data, 9).unwrap();
        assert!(raw.len() < data.len() / 10, "repetitive data should compress a lot");
        assert_eq!(inflate(&raw), data);
    }

    #[test]
    fn level_is_clamped() {
        // out-of-range levels don't panic and still round-trip
        let data = b"some data here";
        for lvl in [0u32, 1, 9, 99] {
            let raw = deflate(data, lvl).unwrap();
            assert_eq!(inflate(&raw), data);
        }
    }

    #[test]
    fn higher_level_is_not_larger() {
        let data = b"abcabcabcabc repeating-ish payload ".repeat(80);
        let l1 = deflate(&data, 1).unwrap();
        let l9 = deflate(&data, 9).unwrap();
        assert!(l9.len() <= l1.len(), "level 9 should not be larger than level 1");
    }

    #[test]
    fn empty_input_ok() {
        let raw = deflate(b"", 6).unwrap();
        assert_eq!(inflate(&raw), Vec::<u8>::new());
    }
}
