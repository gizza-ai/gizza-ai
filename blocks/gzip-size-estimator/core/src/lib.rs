//! gzip-size-estimator core — reports the raw and gzipped byte size of text.
//!
//! Pure compute shared by the chat skill block and the web page. No
//! wafer/wasm-bindgen deps. Uses flate2 (miniz_oxide, wasm32-safe) to gzip the
//! UTF-8 bytes of the input and report raw size, compressed size, the bytes
//! saved, and the compression ratio / percent reduction — so you can estimate
//! the over-the-wire transfer weight of a file served with gzip/Content-Encoding.

use std::io::Write;

use flate2::write::GzEncoder;
use flate2::Compression;

/// Lowest zlib deflate level (no compression). Used to clamp `level`.
pub const MIN_LEVEL: u32 = 0;
/// Highest zlib deflate level (best compression). Used to clamp `level`.
pub const MAX_LEVEL: u32 = 9;
/// Default deflate level — 6 matches the gzip/zlib default.
pub const DEFAULT_LEVEL: u32 = 6;

/// Compute the gzipped byte size of `bytes` at the given deflate `level`
/// (clamped to MIN_LEVEL..=MAX_LEVEL). Returns the number of bytes the gzip
/// stream occupies (header + deflate body + trailer).
pub fn gzip_len(bytes: &[u8], level: u32) -> Result<usize, String> {
    let level = level.clamp(MIN_LEVEL, MAX_LEVEL);
    let mut enc = GzEncoder::new(Vec::new(), Compression::new(level));
    enc.write_all(bytes)
        .map_err(|e| format!("gzip failed: {e}"))?;
    let out = enc.finish().map_err(|e| format!("gzip failed: {e}"))?;
    Ok(out.len())
}

/// Brotli quality used for the `Content-Encoding: br` comparison line — 11 is
/// the maximum (what a CDN serving precompressed `.br` assets typically uses).
pub const BROTLI_QUALITY: u32 = 11;

/// Compute the brotli-compressed byte size of `bytes` at the fixed
/// [`BROTLI_QUALITY`] (lgwin 22, the common default). Pure-Rust, wasm32-safe.
pub fn brotli_len(bytes: &[u8]) -> Result<usize, String> {
    let mut out = Vec::new();
    let mut reader = bytes;
    let params = brotli::enc::BrotliEncoderParams {
        quality: BROTLI_QUALITY as i32,
        lgwin: 22,
        ..Default::default()
    };
    brotli::BrotliCompress(&mut reader, &mut out, &params)
        .map_err(|e| format!("brotli failed: {e}"))?;
    Ok(out.len())
}

/// Format a byte count into a short human-readable string (e.g. "1.50 KB").
/// Uses binary units (1 KB = 1024 bytes), matching how transfer sizes are
/// usually reported by browser dev tools.
pub fn human_bytes(n: usize) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    let f = n as f64;
    if f < KB {
        format!("{n} B")
    } else if f < MB {
        format!("{:.2} KB", f / KB)
    } else {
        format!("{:.2} MB", f / MB)
    }
}

/// Estimate the raw and gzipped size of `input` (its UTF-8 bytes) and return a
/// human-readable multi-line report.
///
/// `level` is the deflate compression level (0–9, default 6). Returns `Err` only
/// for empty input or a gzip failure.
pub fn estimate(input: &str, level: u32) -> Result<String, String> {
    let bytes = input.as_bytes();
    let raw = bytes.len();
    if raw == 0 {
        return Err("input is empty — paste some code or text to measure".into());
    }
    let level = level.clamp(MIN_LEVEL, MAX_LEVEL);
    let gz = gzip_len(bytes, level)?;

    // gzip can be larger than the raw size for tiny inputs (the ~18-byte gzip
    // header + trailer outweighs any savings) — report a negative "saved" /
    // reduction honestly rather than clamping.
    let saved = raw as i64 - gz as i64;
    let pct = (saved as f64 / raw as f64) * 100.0;
    let ratio = raw as f64 / gz as f64;

    let mut out = String::new();
    out.push_str(&format!(
        "Raw size:          {} ({} bytes)\n",
        human_bytes(raw),
        raw
    ));
    out.push_str(&format!(
        "Gzipped size:      {} ({} bytes)  [level {}]\n",
        human_bytes(gz),
        gz,
        level
    ));
    if saved >= 0 {
        out.push_str(&format!(
            "Saved:             {} ({} bytes)\n",
            human_bytes(saved as usize),
            saved
        ));
    } else {
        out.push_str(&format!(
            "Saved:             -{} ({} bytes — gzip overhead exceeds savings on tiny input)\n",
            human_bytes((-saved) as usize),
            saved
        ));
    }
    out.push_str(&format!("Reduction:         {pct:.1}%\n"));
    out.push_str(&format!("Compression ratio: {ratio:.2}x\n"));

    // Brotli (`Content-Encoding: br`) comparison — usually a bit smaller than
    // gzip on text and supported by all modern browsers.
    let br = brotli_len(bytes)?;
    let br_pct = ((raw as i64 - br as i64) as f64 / raw as f64) * 100.0;
    out.push_str(&format!(
        "Brotli size:       {} ({} bytes)  [quality {}, {:.1}% reduction]",
        human_bytes(br),
        br,
        BROTLI_QUALITY,
        br_pct
    ));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimates_repetitive_text_compresses_well() {
        let input = "abcabcabc".repeat(200); // very compressible
        let report = estimate(&input, DEFAULT_LEVEL).unwrap();
        assert!(report.contains("Raw size:"), "got: {report}");
        assert!(report.contains("Gzipped size:"), "got: {report}");
        assert!(report.contains("Reduction:"), "got: {report}");
        assert!(report.contains("Brotli size:"), "got: {report}");
        // 1800 raw bytes of "abc" repeats must shrink dramatically.
        let gz = gzip_len(input.as_bytes(), DEFAULT_LEVEL).unwrap();
        assert!(gz < input.len() / 10, "expected big savings, gz={gz}");
        // brotli also compresses the repetitive input well.
        let br = brotli_len(input.as_bytes()).unwrap();
        assert!(br < input.len() / 10, "expected brotli savings, br={br}");
    }

    #[test]
    fn tiny_input_reports_negative_savings() {
        // "hi" is far smaller than gzip's header+trailer overhead.
        let report = estimate("hi", DEFAULT_LEVEL).unwrap();
        assert!(report.contains("gzip overhead"), "got: {report}");
    }

    #[test]
    fn level_is_clamped() {
        // Out-of-range levels are clamped, not errors.
        let bytes = "hello world ".repeat(50);
        let lo = gzip_len(bytes.as_bytes(), 0).unwrap();
        let hi = gzip_len(bytes.as_bytes(), 99).unwrap(); // clamps to 9
        assert!(hi <= lo, "level 9 should be <= level 0: hi={hi} lo={lo}");
        // report records the clamped level (9), not 99.
        let report = estimate(&bytes, 99).unwrap();
        assert!(report.contains("[level 9]"), "got: {report}");
    }

    #[test]
    fn rejects_empty_input() {
        let err = estimate("", DEFAULT_LEVEL).unwrap_err();
        assert!(err.contains("empty"), "got: {err}");
    }

    #[test]
    fn human_bytes_units() {
        assert_eq!(human_bytes(512), "512 B");
        assert_eq!(human_bytes(1536), "1.50 KB");
        assert_eq!(human_bytes(1024 * 1024 * 3), "3.00 MB");
    }
}
