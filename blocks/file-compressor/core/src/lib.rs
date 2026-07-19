//! file-compressor core — a unified, pure-Rust file compressor/decompressor over
//! four general-purpose codecs. No wafer/wasm-bindgen deps, so the block and its
//! tests share one implementation.
//!
//! Codecs (all pure Rust, wasm32-safe — no C bindings):
//!  * **gzip** (RFC 1952) via `flate2` (miniz_oxide backend) — compress + decompress.
//!  * **xz** (XZ container / LZMA2) via `lzma-rust2` (tukaani xz-for-java port) —
//!    compress + decompress. Same encoder tuning as the `lzma-compress` block so
//!    the higher presets don't abort under the wasm runtime.
//!  * **brotli** (RFC 7932) via the pure-Rust `brotli` crate — compress + decompress.
//!  * **zstd** (RFC 8878) via `ruzstd` — **decompress only**. The standard zstd
//!    encoder is the C `zstd` library (`zstd-sys` needs a wasi C toolchain that
//!    isn't available), and the only pure-Rust encoder is an experimental port
//!    that warns of possible data loss — unsafe for a compression tool. zstd
//!    compression therefore returns a clear "unsupported" error.
//!
//! Decompression is bomb-guarded: a small, highly-compressible input that would
//! expand past `MAX_OUTPUT_BYTES` returns a clean error instead of growing the
//! wasm linear memory until it OOM-traps.

use std::io::{Read, Write};

/// Max decompressed size — guards against decompression bombs. Kept well below
/// the wasm sandbox ceiling so a bomb returns a clean error rather than trapping
/// while the output is base64-encoded into the envelope. Mirrors `lzma-decompress`.
pub const MAX_OUTPUT_BYTES: u64 = 16 * 1024 * 1024; // 16 MiB

/// Cap the wasm dictionary so the higher xz presets instantiate (mirrors
/// `lzma-compress`): preset-6 size, the largest that fits the runtime.
const MAX_WASM_DICT_SIZE: u32 = 1 << 23; // 8 MiB

/// The four codecs this tool understands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Gzip,
    Xz,
    Brotli,
    Zstd,
}

impl Format {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "gzip" => Ok(Format::Gzip),
            "xz" => Ok(Format::Xz),
            "brotli" => Ok(Format::Brotli),
            "zstd" => Ok(Format::Zstd),
            other => Err(format!(
                "unknown format {other:?} (expected gzip, xz, brotli, or zstd)"
            )),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Format::Gzip => "gzip",
            Format::Xz => "xz",
            Format::Brotli => "brotli",
            Format::Zstd => "zstd",
        }
    }

    /// Suffix appended on compress / stripped on decompress.
    pub fn suffix(self) -> &'static str {
        match self {
            Format::Gzip => ".gz",
            Format::Xz => ".xz",
            Format::Brotli => ".br",
            Format::Zstd => ".zst",
        }
    }

    /// MIME for a compressed stream of this codec (codec-specific where a
    /// registered type exists, else the generic binary type).
    pub fn compressed_mime(self) -> &'static str {
        match self {
            Format::Gzip => "application/gzip",
            Format::Xz => "application/x-xz",
            Format::Brotli => "application/octet-stream",
            Format::Zstd => "application/zstd",
        }
    }
}

/// Direction of the operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    Compress,
    Decompress,
}

impl Operation {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "compress" => Ok(Operation::Compress),
            "decompress" => Ok(Operation::Decompress),
            other => Err(format!(
                "unknown operation {other:?} (expected compress or decompress)"
            )),
        }
    }
}

/// Compress or decompress `data` with `format` at `level` (1-9, only used when
/// compressing). Returns the transformed bytes, or a clear error.
pub fn process(op: Operation, format: Format, data: &[u8], level: u32) -> Result<Vec<u8>, String> {
    match (op, format) {
        (Operation::Compress, Format::Gzip) => gzip_compress(data, level),
        (Operation::Compress, Format::Xz) => xz_compress(data, level),
        (Operation::Compress, Format::Brotli) => brotli_compress(data, level),
        (Operation::Compress, Format::Zstd) => Err(
            "zstd compression is not supported (the standard zstd encoder is a C \
             library that cannot build to wasm here). Decompress zstd here, or \
             compress with gzip, xz, or brotli instead."
                .to_string(),
        ),
        (Operation::Decompress, Format::Gzip) => gzip_decompress(data),
        (Operation::Decompress, Format::Xz) => xz_decompress(data),
        (Operation::Decompress, Format::Brotli) => brotli_decompress(data),
        (Operation::Decompress, Format::Zstd) => zstd_decompress(data),
    }
}

// ── gzip ────────────────────────────────────────────────────────────────────

fn gzip_compress(data: &[u8], level: u32) -> Result<Vec<u8>, String> {
    use flate2::{write::GzEncoder, Compression};
    let mut enc = GzEncoder::new(Vec::new(), Compression::new(level.clamp(1, 9)));
    enc.write_all(data).map_err(|e| format!("gzip write failed: {e}"))?;
    enc.finish().map_err(|e| format!("gzip finish failed: {e}"))
}

fn gzip_decompress(data: &[u8]) -> Result<Vec<u8>, String> {
    let dec = flate2::read::GzDecoder::new(data);
    read_capped(dec, "gzip")
}

// ── xz ──────────────────────────────────────────────────────────────────────

fn xz_compress(data: &[u8], level: u32) -> Result<Vec<u8>, String> {
    use lzma_rust2::{EncodeMode, MfType, XzOptions, XzWriter};
    let mut opts = XzOptions::with_preset(level.clamp(0, 9));
    // BT4 (chosen by presets 4-9) aborts under wasm — swap for HC4 + Normal
    // parse, keeping the output a fully-standard `.xz` stream (see lzma-compress).
    if opts.lzma_options.mf == MfType::Bt4 {
        opts.lzma_options.mf = MfType::Hc4;
        opts.lzma_options.mode = EncodeMode::Normal;
        if opts.lzma_options.depth_limit <= 0 {
            opts.lzma_options.depth_limit = (4 + opts.lzma_options.nice_len as i32 / 2).max(4);
        }
    }
    if opts.lzma_options.dict_size > MAX_WASM_DICT_SIZE {
        opts.lzma_options.dict_size = MAX_WASM_DICT_SIZE;
    }
    let mut enc =
        XzWriter::new(Vec::new(), opts).map_err(|e| format!("xz writer init failed: {e}"))?;
    enc.write_all(data).map_err(|e| format!("xz write failed: {e}"))?;
    enc.finish().map_err(|e| format!("xz finish failed: {e}"))
}

fn xz_decompress(data: &[u8]) -> Result<Vec<u8>, String> {
    use lzma_rust2::XzReader;
    // allow_multiple_streams = true → concatenated .xz streams decode as one.
    let dec = XzReader::new(std::io::Cursor::new(data), true);
    read_capped(dec, "xz")
}

// ── brotli ──────────────────────────────────────────────────────────────────

fn brotli_compress(data: &[u8], level: u32) -> Result<Vec<u8>, String> {
    // Brotli quality is 0-11; map the tool's 1-9 level linearly onto that range
    // so level 9 reaches near-max quality (11) and level 1 is fast (~1).
    let quality = ((level.clamp(1, 9) as f64 - 1.0) / 8.0 * 11.0).round() as i32;
    let mut params = brotli::enc::BrotliEncoderParams::default();
    params.quality = quality.clamp(0, 11);
    params.lgwin = 22; // 4 MiB window — the standard default.
    let mut out = Vec::new();
    brotli::BrotliCompress(&mut &data[..], &mut out, &params)
        .map_err(|e| format!("brotli compression failed: {e}"))?;
    Ok(out)
}

fn brotli_decompress(data: &[u8]) -> Result<Vec<u8>, String> {
    let dec = brotli::Decompressor::new(data, 4096);
    read_capped(dec, "brotli")
}

// ── zstd (decompress only) ───────────────────────────────────────────────────

fn zstd_decompress(data: &[u8]) -> Result<Vec<u8>, String> {
    let dec = ruzstd::decoding::StreamingDecoder::new(std::io::Cursor::new(data))
        .map_err(|e| format!("invalid zstd stream: {e}"))?;
    read_capped(dec, "zstd")
}

// ── shared helpers ───────────────────────────────────────────────────────────

/// Read a decoder to end with the bomb guard applied.
fn read_capped<R: Read>(dec: R, codec: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    dec.take(MAX_OUTPUT_BYTES + 1)
        .read_to_end(&mut out)
        .map_err(|e| format!("{codec} decompression failed: {e}"))?;
    if out.len() as u64 > MAX_OUTPUT_BYTES {
        return Err("decompressed data is too large".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(format: Format, level: u32) {
        let data = b"file-compressor unified codec payload \x00\x01\x02 ".repeat(200);
        let comp = process(Operation::Compress, format, &data, level).unwrap();
        assert!(
            comp.len() < data.len(),
            "{} should shrink repetitive data ({} -> {})",
            format.label(),
            data.len(),
            comp.len()
        );
        let back = process(Operation::Decompress, format, &comp, level).unwrap();
        assert_eq!(back, data, "{} round-trip", format.label());
    }

    #[test]
    fn gzip_round_trips() {
        roundtrip(Format::Gzip, 6);
    }

    #[test]
    fn xz_round_trips() {
        roundtrip(Format::Xz, 6);
    }

    #[test]
    fn brotli_round_trips() {
        roundtrip(Format::Brotli, 9);
    }

    #[test]
    fn all_levels_compress_and_round_trip() {
        for &fmt in &[Format::Gzip, Format::Xz, Format::Brotli] {
            for lvl in 1u32..=9 {
                let data = b"level sweep payload ".repeat(50);
                let comp = process(Operation::Compress, fmt, &data, lvl).unwrap();
                let back = process(Operation::Decompress, fmt, &comp, lvl).unwrap();
                assert_eq!(back, data, "{} level {lvl}", fmt.label());
            }
        }
    }

    #[test]
    fn zstd_compress_is_unsupported() {
        let err = process(Operation::Compress, Format::Zstd, b"hello", 6).unwrap_err();
        assert!(err.contains("zstd compression is not supported"), "got: {err}");
    }

    #[test]
    fn zstd_decompress_round_trips_from_reference() {
        // A real zstd frame for the bytes "hello zstd" (frame magic 28 B5 2F FD,
        // trailing xxHash checksum), produced by the reference `zstd` CLI.
        // Confirms `ruzstd` decodes genuine zstd output even though we can't
        // produce zstd here.
        let frame: [u8; 23] = [
            40, 181, 47, 253, 4, 88, 81, 0, 0, 104, 101, 108, 108, 111, 32, 122, 115, 116, 100,
            207, 219, 96, 156,
        ];
        let out = process(Operation::Decompress, Format::Zstd, &frame, 6).unwrap();
        assert_eq!(out, b"hello zstd");
    }

    #[test]
    fn bad_decompress_errors_cleanly() {
        // Random bytes are not a valid stream for any codec → clean error, no panic.
        let junk = b"this is definitely not a compressed stream!!";
        for &fmt in &[Format::Gzip, Format::Xz, Format::Brotli, Format::Zstd] {
            let res = process(Operation::Decompress, fmt, junk, 6);
            assert!(res.is_err(), "{} should reject junk", fmt.label());
        }
    }

    #[test]
    fn rejects_decompression_bomb() {
        // A tiny gzip that expands past the cap must error rather than OOM.
        let big = vec![0u8; MAX_OUTPUT_BYTES as usize + 1024];
        let comp = process(Operation::Compress, Format::Gzip, &big, 9).unwrap();
        assert!(comp.len() < 1024 * 1024, "bomb input should stay tiny");
        let err = process(Operation::Decompress, Format::Gzip, &comp, 6).unwrap_err();
        assert!(err.contains("too large"), "got: {err}");
    }

    #[test]
    fn format_and_operation_parse_errors() {
        assert!(Format::parse("lz4").is_err());
        assert!(Operation::parse("shrink").is_err());
        assert_eq!(Format::parse("brotli").unwrap(), Format::Brotli);
        assert_eq!(Operation::parse("decompress").unwrap(), Operation::Decompress);
    }
}
