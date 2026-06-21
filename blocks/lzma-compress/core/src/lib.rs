//! gizza-ai/lzma-compress core — compress bytes into a single-stream `.xz`
//! (XZ container around an LZMA2 filter), the format produced by the `xz`
//! command-line tool. Pure-Rust `lzma-rust2` (a port of tukaani's xz-for-java)
//! — no C bindings, wasm32-safe. Output validates with `xz -t` and decompresses
//! with any standard xz/7-Zip implementation. The inverse of an xz decompressor.
//!
//! `.xz` typically beats gzip on compression ratio at the cost of speed, which
//! is why it is the go-to format when smallest size matters.

use std::io::Write;

use lzma_rust2::{EncodeMode, MfType, XzOptions, XzWriter};

/// Compress `data` into an `.xz` stream. `level` is the LZMA preset 0-9
/// (clamped); higher = smaller output but slower / more memory. 6 is the `xz`
/// default. The result is a complete, standard `.xz` container.
///
/// Wasm note: two preset traits abort inside the wafer wasm runtime, so we
/// adapt the options while keeping output a fully-standard `.xz`:
///  * The higher presets (4-9) normally pick the BT4 (binary-tree) match finder,
///    which aborts under wasm — we swap it for HC4 (hash-chain) and keep Normal
///    optimal-parse mode, so the ratio stays strong (beats gzip -9).
///  * Presets 7-9 ask for a 16-64 MiB dictionary, which overflows the runtime's
///    linear memory — we cap the dictionary at 8 MiB (the preset-6 size, the
///    largest that instantiates). The higher levels still differ via their
///    larger `nice_len` / search depth. 8 MiB already covers the vast majority of
///    inputs (only files larger than the dictionary lose long-range matches).
/// The result runs identically on every backend, including the chat Service
/// Worker. Levels 0-3 (HC4, ≤4 MiB dict) are unchanged.
const MAX_WASM_DICT_SIZE: u32 = 1 << 23; // 8 MiB

pub fn xz_compress(data: &[u8], level: u32) -> Result<Vec<u8>, String> {
    let preset = level.clamp(0, 9);
    let mut opts = XzOptions::with_preset(preset);
    if opts.lzma_options.mf == MfType::Bt4 {
        opts.lzma_options.mf = MfType::Hc4;
        opts.lzma_options.mode = EncodeMode::Normal;
        // Give the hash-chain finder a useful search depth (BT4 used 0 = auto).
        if opts.lzma_options.depth_limit <= 0 {
            opts.lzma_options.depth_limit =
                (4 + opts.lzma_options.nice_len as i32 / 2).max(4);
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

#[cfg(test)]
mod tests {
    use super::*;
    use lzma_rust2::XzReader;
    use std::io::Read;

    fn xz_decompress(bytes: &[u8]) -> Vec<u8> {
        let mut dec = XzReader::new(std::io::Cursor::new(bytes), false);
        let mut out = Vec::new();
        dec.read_to_end(&mut out).unwrap();
        out
    }

    #[test]
    fn has_xz_magic() {
        let xz = xz_compress(b"hello", 6).unwrap();
        // .xz stream header magic: FD '7' 'z' 'X' 'Z' 00
        assert_eq!(&xz[0..6], &[0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00]);
    }

    #[test]
    fn round_trips() {
        let data = b"hello, lzma/xz world! ".repeat(50);
        let xz = xz_compress(&data, 6).unwrap();
        assert_eq!(xz_decompress(&xz), data);
    }

    #[test]
    fn compresses_repetitive_data() {
        // 180 KB of low-entropy text must shrink dramatically (the whole point
        // of this tool vs lzma-rs, whose trivial encoder would grow it).
        let data = b"The quick brown fox jumps over the lazy dog. ".repeat(4000);
        let xz = xz_compress(&data, 9).unwrap();
        assert!(
            xz.len() < data.len() / 50,
            "repetitive data should compress >50x, got {} -> {}",
            data.len(),
            xz.len()
        );
        assert_eq!(xz_decompress(&xz), data);
    }

    #[test]
    fn all_levels_round_trip() {
        let data = b"some representative payload \x00\x01\x02 with binary bytes".repeat(20);
        for lvl in 0u32..=9 {
            let xz = xz_compress(&data, lvl).unwrap();
            assert_eq!(xz_decompress(&xz), data, "level {lvl} round-trip");
        }
    }

    #[test]
    fn level_is_clamped() {
        // out-of-range levels don't panic and still produce a valid stream
        let data = b"clamp me";
        for lvl in [0u32, 9, 42, 1000] {
            let xz = xz_compress(data, lvl).unwrap();
            assert_eq!(xz_decompress(&xz), data);
        }
    }

    #[test]
    fn empty_input_ok() {
        let xz = xz_compress(b"", 6).unwrap();
        assert_eq!(&xz[0..6], &[0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00]);
        assert!(xz_decompress(&xz).is_empty());
    }
}
