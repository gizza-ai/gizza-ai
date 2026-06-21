//! gizza-ai/strip-exif core — remove metadata (EXIF, GPS, XMP, comments) from a
//! JPEG or PNG image **without re-encoding the pixels**. The compressed image
//! data is preserved byte-for-byte; only the metadata segments/chunks are
//! dropped, so the result is visually identical and not recompressed.
//!
//! Pure Rust (`img-parts`), no wasm/wafer deps — runs on every backend.
//!
//! Privacy policy: strip every personal/identifying metadata segment while
//! keeping what's needed to render the image faithfully.
//!   - JPEG: drop APP1 (EXIF/GPS, XMP), APP13 (Photoshop/IPTC), COM (comments).
//!           Keep APP0 (JFIF density) and APP2 (ICC colour profile) so colours
//!           and aspect render correctly.
//!   - PNG:  drop the ancillary text/metadata chunks (tEXt, zTXt, iTXt, eXIf,
//!           tIME, dSIG). Keep all critical chunks (IHDR, PLTE, IDAT, IEND) and
//!           rendering-relevant ancillaries (gAMA, cHRM, sRGB, iCCP, bKGD, pHYs,
//!           tRNS, sBIT).

use img_parts::jpeg::{markers, Jpeg};
use img_parts::png::Png;
use img_parts::{Bytes, Error as ImgError, ImageEXIF};
use serde::Serialize;

/// What was stripped, for the LLM / caller.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StripReport {
    /// Detected container format ("jpeg" | "png").
    pub format: String,
    /// Input size in bytes.
    pub input_bytes: usize,
    /// Output size in bytes (always <= input).
    pub output_bytes: usize,
    /// Bytes removed (input - output).
    pub removed_bytes: usize,
    /// Number of metadata segments/chunks removed.
    pub segments_removed: usize,
    /// True if the input carried EXIF data (now removed).
    pub had_exif: bool,
}

/// JPEG segments we strip: APP1 (EXIF, GPS, XMP), APP13 (Photoshop/IPTC),
/// COM (free-text comment).
fn jpeg_marker_is_metadata(marker: u8) -> bool {
    marker == markers::APP1 || marker == markers::APP13 || marker == markers::COM
}

/// PNG ancillary chunks we strip (text + exif + timestamp + digital signature).
/// Critical chunks and colour/rendering ancillaries are kept.
fn png_chunk_is_metadata(kind: &[u8; 4]) -> bool {
    matches!(
        kind,
        b"tEXt" | b"zTXt" | b"iTXt" | b"eXIf" | b"tIME" | b"dSIG"
    )
}

const JPEG_MAGIC: &[u8] = &[0xFF, 0xD8, 0xFF];
const PNG_MAGIC: &[u8] = &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

/// Detected output format: ("jpeg"|"png", mime, extension).
pub fn detect_format(bytes: &[u8]) -> Option<(&'static str, &'static str, &'static str)> {
    if bytes.starts_with(JPEG_MAGIC) {
        Some(("jpeg", "image/jpeg", "jpg"))
    } else if bytes.starts_with(PNG_MAGIC) {
        Some(("png", "image/png", "png"))
    } else {
        None
    }
}

/// Strip all metadata from a JPEG/PNG, returning the cleaned bytes and a report.
pub fn strip(input: &[u8]) -> Result<(Vec<u8>, StripReport), String> {
    let (format, _mime, _ext) = detect_format(input).ok_or_else(|| {
        "unsupported image format: only JPEG and PNG are supported for metadata removal".to_string()
    })?;
    let bytes = Bytes::copy_from_slice(input);
    match format {
        "jpeg" => strip_jpeg(bytes, input.len()),
        "png" => strip_png(bytes, input.len()),
        _ => unreachable!("detect_format only returns jpeg|png"),
    }
}

fn map_eof(e: ImgError) -> String {
    format!("malformed image: could not parse the image container ({e})")
}

fn strip_jpeg(bytes: Bytes, input_len: usize) -> Result<(Vec<u8>, StripReport), String> {
    let mut jpeg = Jpeg::from_bytes(bytes).map_err(map_eof)?;
    let had_exif = jpeg.exif().is_some();
    let before = jpeg.segments().len();
    jpeg.segments_mut()
        .retain(|seg| !jpeg_marker_is_metadata(seg.marker()));
    let segments_removed = before - jpeg.segments().len();
    let out = jpeg.encoder().bytes().to_vec();
    Ok(finish(out, "jpeg", input_len, segments_removed, had_exif))
}

fn strip_png(bytes: Bytes, input_len: usize) -> Result<(Vec<u8>, StripReport), String> {
    let mut png = Png::from_bytes(bytes).map_err(map_eof)?;
    let had_exif = png.chunks().iter().any(|c| &c.kind() == b"eXIf");
    let before = png.chunks().len();
    png.chunks_mut()
        .retain(|c| !png_chunk_is_metadata(&c.kind()));
    let segments_removed = before - png.chunks().len();
    let out = png.encoder().bytes().to_vec();
    Ok(finish(out, "png", input_len, segments_removed, had_exif))
}

fn finish(
    out: Vec<u8>,
    format: &str,
    input_len: usize,
    segments_removed: usize,
    had_exif: bool,
) -> (Vec<u8>, StripReport) {
    let output_bytes = out.len();
    let removed_bytes = input_len.saturating_sub(output_bytes);
    let report = StripReport {
        format: format.to_string(),
        input_bytes: input_len,
        output_bytes,
        removed_bytes,
        segments_removed,
        had_exif,
    };
    (out, report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use img_parts::jpeg::{Jpeg, JpegSegment};
    use img_parts::png::{Png, PngChunk};
    use img_parts::Bytes;

    /// Build a small JPEG carrying an APP1 (EXIF) segment + a COM comment. We
    /// push the segments directly (not via set_exif) so the fixture stays valid
    /// regardless of img-parts' insertion heuristics.
    fn make_jpeg_with_metadata() -> Vec<u8> {
        let mut jpeg = Jpeg::from_bytes(Bytes::from(minimal_jpeg())).unwrap();
        // APP1 with the "Exif\0\0" identifier so jpeg.exif() detects it.
        let exif =
            JpegSegment::new_with_contents(markers::APP1, Bytes::from_static(b"Exif\0\0II*\0\x08\0\0\0"));
        jpeg.segments_mut().insert(1, exif);
        let com = JpegSegment::new_with_contents(markers::COM, Bytes::from_static(b"secret note"));
        jpeg.segments_mut().insert(2, com);
        jpeg.encoder().bytes().to_vec()
    }

    // SOI + APP0 (JFIF) + SOS + a byte of entropy data + EOI. img-parts needs the
    // SOS..EOI tail to round-trip parse; the framing is all the metadata-removal
    // logic exercises (it never decodes pixels).
    fn minimal_jpeg() -> Vec<u8> {
        vec![
            0xFF, 0xD8, // SOI
            0xFF, 0xE0, 0x00, 0x10, // APP0, len 16
            b'J', b'F', b'I', b'F', 0x00, 0x01, 0x01, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00,
            0xFF, 0xDA, 0x00, 0x08, 0x01, 0x01, 0x00, 0x00, 0x3F, 0x00, // SOS (len 8)
            0x12, 0x34, // entropy-coded scan data
            0xFF, 0xD9, // EOI
        ]
    }

    fn make_png_with_metadata() -> Vec<u8> {
        let mut png = Png::from_bytes(Bytes::from(minimal_png())).unwrap();
        let text = PngChunk::new([b't', b'E', b'X', b't'], Bytes::from_static(b"Comment\0hello"));
        png.chunks_mut().insert(1, text);
        let exif = PngChunk::new([b'e', b'X', b'I', b'f'], Bytes::from_static(b"II*\0\x08\0\0\0"));
        png.chunks_mut().insert(1, exif);
        png.encoder().bytes().to_vec()
    }

    fn minimal_png() -> Vec<u8> {
        vec![
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // signature
            0x00, 0x00, 0x00, 0x0D, // IHDR len
            b'I', b'H', b'D', b'R', 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06,
            0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4, 0x89, // IHDR data + crc
            0x00, 0x00, 0x00, 0x0A, // IDAT len
            b'I', b'D', b'A', b'T', 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01,
            0x0D, 0x0A, 0x2D, 0xB4, // IDAT data + crc
            0x00, 0x00, 0x00, 0x00, // IEND len
            b'I', b'E', b'N', b'D', 0xAE, 0x42, 0x60, 0x82, // IEND + crc
        ]
    }

    #[test]
    fn strips_jpeg_exif_and_comment_keeps_jfif() {
        let img = make_jpeg_with_metadata();
        let (out, report) = strip(&img).unwrap();
        assert_eq!(report.format, "jpeg");
        assert!(report.had_exif, "input should have had EXIF");
        assert!(report.segments_removed >= 2, "should remove APP1 + COM");
        let parsed = Jpeg::from_bytes(Bytes::from(out.clone())).unwrap();
        assert!(parsed.exif().is_none(), "EXIF must be gone");
        assert!(
            parsed.segments().iter().any(|s| s.marker() == markers::APP0),
            "JFIF APP0 must be kept"
        );
        assert!(out.len() < img.len(), "output should be smaller");
    }

    #[test]
    fn strips_png_text_and_exif_keeps_critical() {
        let img = make_png_with_metadata();
        let (out, report) = strip(&img).unwrap();
        assert_eq!(report.format, "png");
        assert!(report.had_exif);
        assert!(report.segments_removed >= 2, "tEXt + eXIf removed");
        let parsed = Png::from_bytes(Bytes::from(out)).unwrap();
        assert!(
            !parsed.chunks().iter().any(|c| &c.kind() == b"tEXt"),
            "tEXt must be gone"
        );
        assert!(
            !parsed.chunks().iter().any(|c| &c.kind() == b"eXIf"),
            "eXIf must be gone"
        );
        assert!(parsed.chunks().iter().any(|c| &c.kind() == b"IHDR"));
        assert!(parsed.chunks().iter().any(|c| &c.kind() == b"IDAT"));
        assert!(parsed.chunks().iter().any(|c| &c.kind() == b"IEND"));
    }

    #[test]
    fn clean_image_is_noop_but_valid() {
        let img = minimal_png();
        let (out, report) = strip(&img).unwrap();
        assert_eq!(report.segments_removed, 0);
        assert!(!report.had_exif);
        assert!(Png::from_bytes(Bytes::from(out)).is_ok());
    }

    #[test]
    fn rejects_non_image() {
        let err = strip(b"not an image at all").unwrap_err();
        assert!(err.contains("unsupported"), "got: {err}");
    }

    #[test]
    fn detect_format_jpeg_and_png() {
        assert_eq!(detect_format(&[0xFF, 0xD8, 0xFF, 0xE0]).unwrap().0, "jpeg");
        assert_eq!(
            detect_format(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]).unwrap().0,
            "png"
        );
        assert!(detect_format(b"GIF89a").is_none());
    }
}
