//! gizza-ai/gzip-compress core — compress bytes into a single-member gzip (.gz)
//! stream, embedding the original filename in the gzip header (FNAME) when given.
//! No wafer/wasm-bindgen deps. Pure-Rust `flate2` (miniz_oxide backend,
//! wasm32-safe). The inverse of gunzip.

use std::io::Write;

use flate2::{Compression, GzBuilder};

/// Compress `data` to gzip. `level` is the deflate level 1-9 (clamped; 6 is the
/// usual default). `filename`, if non-empty, is stored in the gzip FNAME header
/// so a later decompress can recover it.
pub fn gzip(data: &[u8], filename: Option<&str>, level: u32) -> Result<Vec<u8>, String> {
    let lvl = level.clamp(1, 9);
    let mut builder = GzBuilder::new();
    if let Some(name) = filename.filter(|s| !s.is_empty()) {
        builder = builder.filename(name);
    }
    let mut enc = builder.write(Vec::new(), Compression::new(lvl));
    enc.write_all(data).map_err(|e| format!("gzip write failed: {e}"))?;
    enc.finish().map_err(|e| format!("gzip finish failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn gunzip(bytes: &[u8]) -> (Vec<u8>, Option<String>) {
        let mut dec = flate2::read::GzDecoder::new(bytes);
        let name = dec.header().and_then(|h| h.filename()).map(|f| String::from_utf8_lossy(f).to_string());
        let mut out = Vec::new();
        dec.read_to_end(&mut out).unwrap();
        (out, name)
    }

    #[test]
    fn round_trips() {
        let data = b"hello, gzip world! ".repeat(50);
        let gz = gzip(&data, Some("greeting.txt"), 6).unwrap();
        // gzip magic
        assert_eq!(&gz[0..2], &[0x1F, 0x8B]);
        let (back, name) = gunzip(&gz);
        assert_eq!(back, data);
        assert_eq!(name.as_deref(), Some("greeting.txt"));
    }

    #[test]
    fn compresses_repetitive_data() {
        let data = vec![b'A'; 10_000];
        let gz = gzip(&data, None, 9).unwrap();
        assert!(gz.len() < data.len() / 10, "repetitive data should compress a lot");
        let (back, name) = gunzip(&gz);
        assert_eq!(back, data);
        assert_eq!(name, None);
    }

    #[test]
    fn level_is_clamped() {
        // out-of-range levels don't panic and still round-trip
        let data = b"some data here";
        for lvl in [0u32, 1, 9, 99] {
            let gz = gzip(data, None, lvl).unwrap();
            assert_eq!(gunzip(&gz).0, data);
        }
    }

    #[test]
    fn empty_input_ok() {
        let gz = gzip(b"", None, 6).unwrap();
        assert_eq!(&gz[0..2], &[0x1F, 0x8B]);
        assert_eq!(gunzip(&gz).0, Vec::<u8>::new());
    }
}
