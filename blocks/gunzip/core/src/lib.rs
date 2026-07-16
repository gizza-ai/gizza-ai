//! gizza-ai/gunzip core — decompress a gzip (.gz) blob back to its original
//! bytes. No wafer/wasm-bindgen deps. Pure-Rust `flate2` (miniz_oxide backend,
//! wasm32-safe). Recovers the original filename from the gzip header when present
//! and guards against decompression bombs with an output cap.

use std::io::Read;

use flate2::read::GzDecoder;

/// Max decompressed size (guards against gzip bombs). Kept well below the
/// 64 MiB wasm sandbox so a bomb returns a clean error instead of OOM-trapping
/// while the decompressed bytes are base64-encoded into the output envelope.
const MAX_OUTPUT_BYTES: u64 = 16 * 1024 * 1024; // 16 MiB

/// Decompress a single-member gzip stream. Returns the decompressed bytes and
/// the original filename embedded in the gzip header (FNAME), if any.
pub fn gunzip(bytes: &[u8]) -> Result<(Vec<u8>, Option<String>), String> {
    if bytes.len() < 2 || bytes[0] != 0x1F || bytes[1] != 0x8B {
        return Err("not a gzip file (missing 1F 8B magic)".into());
    }
    let mut dec = GzDecoder::new(bytes);
    let mut out = Vec::new();
    (&mut dec)
        .take(MAX_OUTPUT_BYTES + 1)
        .read_to_end(&mut out)
        .map_err(|e| format!("gzip decode failed: {e}"))?;
    if out.len() as u64 > MAX_OUTPUT_BYTES {
        return Err("decompressed data is too large".into());
    }
    let name = dec
        .header()
        .and_then(|h| h.filename())
        .map(|f| String::from_utf8_lossy(f).to_string())
        .filter(|s| !s.is_empty());
    Ok((out, name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn gzip_with_name(data: &[u8], name: Option<&str>) -> Vec<u8> {
        let mut builder = flate2::GzBuilder::new();
        if let Some(n) = name {
            builder = builder.filename(n);
        }
        let mut enc = builder.write(Vec::new(), flate2::Compression::default());
        enc.write_all(data).unwrap();
        enc.finish().unwrap()
    }

    #[test]
    fn round_trips_with_embedded_filename() {
        let original = b"hello world, this is the original content\n";
        let gz = gzip_with_name(original, Some("orig.txt"));
        let (out, name) = gunzip(&gz).unwrap();
        assert_eq!(out, original);
        assert_eq!(name.as_deref(), Some("orig.txt"));
    }

    #[test]
    fn round_trips_without_filename() {
        let original = b"no name in header";
        let gz = gzip_with_name(original, None);
        let (out, name) = gunzip(&gz).unwrap();
        assert_eq!(out, original);
        assert_eq!(name, None);
    }

    #[test]
    fn rejects_non_gzip() {
        assert!(gunzip(b"PK\x03\x04 not gzip").is_err());
        assert!(gunzip(&[]).is_err());
        assert!(gunzip(&[0x1F]).is_err());
    }

    #[test]
    fn errors_on_truncated_gzip() {
        let gz = gzip_with_name(b"some data here", None);
        // Chop off the trailing CRC/length + part of the deflate stream.
        let truncated = &gz[..gz.len() / 2];
        assert!(gunzip(truncated).is_err());
    }
}
