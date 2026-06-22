//! gizza-ai/lz4-compress core — compress bytes into a standard LZ4 **frame**
//! stream (.lz4, frame magic 0x184D2204). No wafer/wasm-bindgen deps. Pure-Rust
//! `lz4_flex` (no C liblz4), so it runs on every backend. LZ4 favours speed over
//! ratio — it is among the fastest general-purpose codecs. The inverse is any
//! standard `lz4 -d` / `unlz4`.

use std::io::Write;

use lz4_flex::frame::FrameEncoder;

/// Compress `data` into the LZ4 frame format. LZ4 has a single (fast) compression
/// mode — there is no 1-9 level knob like gzip/bzip2 — so the output is purely a
/// function of the input. Returns the framed `.lz4` bytes.
pub fn lz4(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut enc = FrameEncoder::new(Vec::new());
    enc.write_all(data)
        .map_err(|e| format!("lz4 write failed: {e}"))?;
    enc.finish().map_err(|e| format!("lz4 finish failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use lz4_flex::frame::FrameDecoder;
    use std::io::Read;

    fn unlz4(bytes: &[u8]) -> Vec<u8> {
        let mut dec = FrameDecoder::new(bytes);
        let mut out = Vec::new();
        dec.read_to_end(&mut out).unwrap();
        out
    }

    #[test]
    fn round_trips() {
        let data = b"hello, lz4 world! ".repeat(50);
        let c = lz4(&data).unwrap();
        // LZ4 frame magic: 0x184D2204, little-endian on the wire.
        assert_eq!(&c[0..4], &[0x04, 0x22, 0x4D, 0x18]);
        assert_eq!(unlz4(&c), data);
    }

    #[test]
    fn compresses_repetitive_data() {
        let data = vec![b'A'; 10_000];
        let c = lz4(&data).unwrap();
        assert!(c.len() < data.len() / 10, "repetitive data should compress a lot");
        assert_eq!(unlz4(&c), data);
    }

    #[test]
    fn empty_input_ok() {
        let c = lz4(b"").unwrap();
        assert_eq!(&c[0..4], &[0x04, 0x22, 0x4D, 0x18]);
        assert_eq!(unlz4(&c), Vec::<u8>::new());
    }

    #[test]
    fn binary_round_trips() {
        let data: Vec<u8> = (0..=255u8).cycle().take(4096).collect();
        let c = lz4(&data).unwrap();
        assert_eq!(unlz4(&c), data);
    }
}
