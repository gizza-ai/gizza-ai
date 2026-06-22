//! gizza-ai/zlib-compress core — compress bytes into a **zlib** stream (RFC
//! 1950): a 2-byte zlib header (CMF/FLG), the raw DEFLATE body (RFC 1951), then
//! a trailing 4-byte big-endian Adler-32 checksum of the *uncompressed* data.
//! No wafer/wasm-bindgen deps. Pure-Rust `flate2` (miniz_oxide backend,
//! wasm32-safe).
//!
//! zlib sits between gzip (RFC 1952; magic 1F 8B + CRC-32 + optional filename)
//! and raw DEFLATE (RFC 1951; no header, no checksum): it adds a tiny 2-byte
//! header + 4-byte Adler-32 but no filename/timestamp metadata. It is what PNG
//! IDAT chunks, the `Content-Encoding: deflate` HTTP body and many binary
//! formats embed. Inflate it again with any zlib-aware inflate / `ZlibDecoder`.

use std::io::Write;

use flate2::{write::ZlibEncoder, Compression};

/// Compress `data` to a zlib stream. `level` is the deflate level 1-9 (clamped;
/// 6 is the usual default). The output is `[CMF FLG] <deflate body> [Adler-32]`.
pub fn zlib_compress(data: &[u8], level: u32) -> Result<Vec<u8>, String> {
    let lvl = level.clamp(1, 9);
    let mut enc = ZlibEncoder::new(Vec::new(), Compression::new(lvl));
    enc.write_all(data).map_err(|e| format!("zlib write failed: {e}"))?;
    enc.finish().map_err(|e| format!("zlib finish failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn inflate(bytes: &[u8]) -> Vec<u8> {
        let mut dec = flate2::read::ZlibDecoder::new(bytes);
        let mut out = Vec::new();
        dec.read_to_end(&mut out).unwrap();
        out
    }

    #[test]
    fn round_trips() {
        let data = b"hello, zlib world! ".repeat(50);
        let z = zlib_compress(&data, 6).unwrap();
        assert_eq!(inflate(&z), data);
    }

    #[test]
    fn has_zlib_header() {
        // zlib's 2-byte header for deflate: CM nibble = 8, and the 16-bit
        // CMF*256+FLG must be a multiple of 31 (FCHECK, RFC 1950).
        let z = zlib_compress(b"the quick brown fox jumps over the lazy dog", 6).unwrap();
        assert_eq!(z[0] & 0x0F, 8, "compression method must be DEFLATE (CM=8)");
        let cmf = z[0] as u16;
        let flg = z[1] as u16;
        assert_eq!((cmf * 256 + flg) % 31, 0, "zlib header FCHECK must validate");
        // Not a gzip stream (1F 8B).
        assert_ne!(&z[0..2], &[0x1F, 0x8B], "must not be gzip-wrapped");
    }

    #[test]
    fn compresses_repetitive_data() {
        let data = vec![b'A'; 10_000];
        let z = zlib_compress(&data, 9).unwrap();
        assert!(z.len() < data.len() / 10, "repetitive data should compress a lot");
        assert_eq!(inflate(&z), data);
    }

    #[test]
    fn level_is_clamped() {
        let data = b"some data here";
        for lvl in [0u32, 1, 9, 99] {
            let z = zlib_compress(data, lvl).unwrap();
            assert_eq!(inflate(&z), data);
        }
    }

    #[test]
    fn higher_level_is_not_larger() {
        let data = b"abcabcabcabc repeating-ish payload ".repeat(80);
        let l1 = zlib_compress(&data, 1).unwrap();
        let l9 = zlib_compress(&data, 9).unwrap();
        assert!(l9.len() <= l1.len(), "level 9 should not be larger than level 1");
    }

    #[test]
    fn empty_input_ok() {
        let z = zlib_compress(b"", 6).unwrap();
        assert_eq!(inflate(&z), Vec::<u8>::new());
        // Even empty input still carries the 2-byte header + 4-byte Adler-32.
        assert!(z.len() >= 6, "zlib framing is always present");
    }
}
