//! gizza-ai/bzip2-compress core — compress bytes into a bzip2 (.bz2) stream
//! using the Burrows–Wheeler transform. No wafer/wasm-bindgen deps. Pure-Rust
//! `banzai` encoder (wasm32-safe), so it runs on every backend. The inverse is
//! any standard `bunzip2`/`bzip2 -d`.

use std::io::BufWriter;

/// Compress `data` to a bzip2 stream. `level` is the block-size multiplier 1-9
/// (clamped); each unit is a 100 KB BWT block — 9 (the usual default) gives the
/// best ratio. bzip2 typically beats gzip on text but is slower.
pub fn bzip2(data: &[u8], level: usize) -> Result<Vec<u8>, String> {
    let lvl = level.clamp(1, 9);
    let mut out: Vec<u8> = Vec::new();
    {
        let writer = BufWriter::new(&mut out);
        banzai::encode(std::io::Cursor::new(data), writer, lvl)
            .map_err(|e| format!("bzip2 encode failed: {e}"))?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn bunzip2(bytes: &[u8]) -> Vec<u8> {
        let mut dec = bzip2_rs::DecoderReader::new(bytes);
        let mut out = Vec::new();
        dec.read_to_end(&mut out).unwrap();
        out
    }

    #[test]
    fn round_trips() {
        let data = b"hello, bzip2 world! ".repeat(50);
        let bz = bzip2(&data, 9).unwrap();
        // bzip2 magic: "BZh" + level digit
        assert_eq!(&bz[0..3], b"BZh");
        assert_eq!(bunzip2(&bz), data);
    }

    #[test]
    fn compresses_repetitive_data() {
        let data = vec![b'A'; 10_000];
        let bz = bzip2(&data, 9).unwrap();
        assert!(bz.len() < data.len() / 10, "repetitive data should compress a lot");
        assert_eq!(bunzip2(&bz), data);
    }

    #[test]
    fn level_is_clamped() {
        // out-of-range levels don't panic and still round-trip
        let data = b"some data here to compress";
        for lvl in [0usize, 1, 9, 99] {
            let bz = bzip2(data, lvl).unwrap();
            assert_eq!(bunzip2(&bz), data);
        }
    }

    #[test]
    fn header_records_block_size() {
        // the 4th byte is the block-size digit ('1'..='9')
        let bz = bzip2(b"x", 6).unwrap();
        assert_eq!(bz[3], b'6');
    }
}
