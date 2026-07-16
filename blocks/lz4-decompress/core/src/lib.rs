//! gizza-ai/lz4-decompress core — decompress a standard LZ4 **frame** stream
//! (.lz4, frame magic 0x184D2204) back to its original bytes. No wafer/wasm-bindgen
//! deps. Pure-Rust `lz4_flex` (no C liblz4), so it runs on every backend. This is
//! the exact inverse of the lz4-compress block (and of any `lz4 -d` / `unlz4`).

use std::io::Read;

use lz4_flex::frame::FrameDecoder;

/// Max decompressed size (guards against LZ4 decompression bombs). Kept well
/// below the 64 MiB wasm sandbox so a bomb returns a clean error instead of
/// OOM-trapping while the bytes are base64-encoded into the output envelope.
const MAX_OUTPUT_BYTES: u64 = 16 * 1024 * 1024; // 16 MiB

/// Decompress an LZ4 **frame** stream. Validates the LZ4 frame magic number
/// (0x184D2204, little-endian on the wire: `04 22 4D 18`) up front so a wrong
/// file type gives a clear error rather than a confusing decode failure. Returns
/// the decompressed bytes.
pub fn unlz4(bytes: &[u8]) -> Result<Vec<u8>, String> {
    // LZ4 frame magic: 0x184D2204, little-endian → bytes 04 22 4D 18.
    if bytes.len() < 4 || bytes[0..4] != [0x04, 0x22, 0x4D, 0x18] {
        return Err("not an LZ4 frame (missing 04 22 4D 18 magic)".into());
    }
    let mut dec = FrameDecoder::new(bytes);
    let mut out = Vec::new();
    (&mut dec)
        .take(MAX_OUTPUT_BYTES + 1)
        .read_to_end(&mut out)
        .map_err(|e| format!("LZ4 decode failed: {e}"))?;
    if out.len() as u64 > MAX_OUTPUT_BYTES {
        return Err("decompressed data is too large".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lz4_flex::frame::FrameEncoder;
    use std::io::Write;

    fn lz4(data: &[u8]) -> Vec<u8> {
        let mut enc = FrameEncoder::new(Vec::new());
        enc.write_all(data).unwrap();
        enc.finish().unwrap()
    }

    #[test]
    fn round_trips() {
        let data = b"hello, lz4 world! ".repeat(50);
        let c = lz4(&data);
        assert_eq!(unlz4(&c).unwrap(), data);
    }

    #[test]
    fn decompresses_repetitive_data() {
        let data = vec![b'A'; 10_000];
        let c = lz4(&data);
        assert!(c.len() < data.len() / 10);
        assert_eq!(unlz4(&c).unwrap(), data);
    }

    #[test]
    fn empty_payload_round_trips() {
        let c = lz4(b"");
        assert_eq!(unlz4(&c).unwrap(), Vec::<u8>::new());
    }

    #[test]
    fn binary_round_trips() {
        let data: Vec<u8> = (0..=255u8).cycle().take(4096).collect();
        let c = lz4(&data);
        assert_eq!(unlz4(&c).unwrap(), data);
    }

    #[test]
    fn rejects_non_lz4() {
        assert!(unlz4(b"PK\x03\x04 not lz4").is_err());
        assert!(unlz4(&[]).is_err());
        assert!(unlz4(&[0x04, 0x22]).is_err());
        // gzip magic, not LZ4 → rejected.
        assert!(unlz4(&[0x1F, 0x8B, 0x08, 0x00]).is_err());
    }

    #[test]
    fn errors_on_truncated_frame() {
        let c = lz4(&b"some data here that is long enough to span a block".repeat(20));
        let truncated = &c[..c.len() / 2];
        assert!(unlz4(truncated).is_err());
    }
}
