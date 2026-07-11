//! gps-location-remover core — remove ONLY the GPS/geolocation tags from a
//! photo's EXIF metadata, leaving camera make/model, lens, and exposure data
//! (ISO / aperture / shutter / timestamps) intact.
//!
//! Unlike a strip-all-metadata cleaner (see the sibling `strip-exif` block,
//! which drops the entire EXIF/GPS/XMP payload), this edits the EXIF TIFF
//! structure IN PLACE:
//!   1. locate the GPS sub-IFD, referenced by tag 0x8825 (GPSInfoIFDPointer)
//!      in the primary IFD (IFD0);
//!   2. zero the raw GPS coordinate bytes in the TIFF data area (so no forensic
//!      residue of the location remains — we don't merely unlink the IFD);
//!   3. empty the GPS IFD (entry count → 0) so no EXIF reader surfaces any GPS
//!      field.
//!
//! Because nothing is relocated, every other IFD, sub-IFD (Exif, Interop),
//! MakerNote, and the embedded thumbnail stay byte-for-byte valid — a full TIFF
//! re-serialize would risk corrupting MakerNotes (whose internal offsets are
//! absolute) and the thumbnail. The compressed pixel data is never re-encoded
//! (img-parts preserves the scan segments), so there is no quality loss.
//!
//! Pure Rust (`img-parts`), no wasm/wafer deps — runs on every backend.
//! JPEG and PNG (eXIf chunk) are supported.

use img_parts::jpeg::{markers, Jpeg, JpegSegment};
use img_parts::png::Png;
use img_parts::{Bytes, Error as ImgError, ImageEXIF};
use serde::Serialize;

/// EXIF/TIFF tag that, in IFD0, points to the GPS sub-IFD holding every GPS tag
/// (latitude, longitude, altitude, timestamp, datestamp, map datum, …).
const GPS_IFD_POINTER_TAG: u16 = 0x8825;

/// What was removed, for the LLM / caller.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GpsReport {
    /// Detected container format ("jpeg" | "png").
    pub format: String,
    /// Input size in bytes.
    pub input_bytes: usize,
    /// Output size in bytes.
    pub output_bytes: usize,
    /// True if the input carried GPS/location tags (now removed).
    pub had_gps: bool,
    /// Number of GPS tags removed (entries in the GPS sub-IFD).
    pub gps_tags_removed: usize,
    /// True if the input still has EXIF after removal — i.e. camera/exposure
    /// metadata was present and preserved.
    pub had_exif: bool,
}

const JPEG_MAGIC: &[u8] = &[0xFF, 0xD8, 0xFF];
const PNG_MAGIC: &[u8] = &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

/// Detected format: ("jpeg"|"png", mime, extension).
pub fn detect_format(bytes: &[u8]) -> Option<(&'static str, &'static str, &'static str)> {
    if bytes.starts_with(JPEG_MAGIC) {
        Some(("jpeg", "image/jpeg", "jpg"))
    } else if bytes.starts_with(PNG_MAGIC) {
        Some(("png", "image/png", "png"))
    } else {
        None
    }
}

fn map_eof(e: ImgError) -> String {
    format!("malformed image: could not parse the image container ({e})")
}

fn jpeg_exif_segment(exif: Vec<u8>) -> JpegSegment {
    let mut app1 = b"Exif\0\0".to_vec();
    app1.extend_from_slice(&exif);
    JpegSegment::new_with_contents(markers::APP1, Bytes::from(app1))
}

fn replace_jpeg_exif(jpeg: &mut Jpeg, exif: Vec<u8>) {
    if let Some(seg) = jpeg.segments_mut().iter_mut().find(|seg| {
        seg.marker() == markers::APP1 && seg.contents().starts_with(b"Exif\0\0")
    }) {
        *seg = jpeg_exif_segment(exif);
    } else {
        let idx = jpeg.segments().len().min(1);
        jpeg.segments_mut().insert(idx, jpeg_exif_segment(exif));
    }
}

/// Byte size of a single value of a TIFF field type (0 for unknown types).
fn tiff_type_size(ty: u16) -> u64 {
    match ty {
        1 | 2 | 6 | 7 => 1, // BYTE, ASCII, SBYTE, UNDEFINED
        3 | 8 => 2,         // SHORT, SSHORT
        4 | 9 | 11 => 4,    // LONG, SLONG, FLOAT
        5 | 10 | 12 => 8,   // RATIONAL, SRATIONAL, DOUBLE
        _ => 0,
    }
}

/// Remove the GPS sub-IFD from an EXIF TIFF payload in place. Returns the edited
/// TIFF bytes and the number of GPS tags removed (0 if there was no GPS IFD).
/// Non-GPS metadata is left byte-for-byte; nothing is relocated.
fn strip_gps_from_tiff(tiff: &[u8]) -> Result<(Vec<u8>, usize), String> {
    if tiff.len() < 8 {
        return Err("EXIF payload is too short to be a valid TIFF header".to_string());
    }
    let le = match &tiff[0..2] {
        b"II" => true,
        b"MM" => false,
        _ => return Err("EXIF payload has no valid TIFF byte-order marker".to_string()),
    };
    let rd16 = |off: usize| -> Option<u16> {
        tiff.get(off..off + 2).map(|s| {
            let a = [s[0], s[1]];
            if le {
                u16::from_le_bytes(a)
            } else {
                u16::from_be_bytes(a)
            }
        })
    };
    let rd32 = |off: usize| -> Option<u32> {
        tiff.get(off..off + 4).map(|s| {
            let a = [s[0], s[1], s[2], s[3]];
            if le {
                u32::from_le_bytes(a)
            } else {
                u32::from_be_bytes(a)
            }
        })
    };

    if rd16(2).ok_or("truncated TIFF header")? != 42 {
        return Err("EXIF payload is not a valid TIFF (bad magic number)".to_string());
    }
    let ifd0 = rd32(4).ok_or("truncated TIFF header")? as usize;

    // Scan IFD0 for the GPS sub-IFD pointer.
    let ifd0_count = rd16(ifd0).ok_or("truncated or out-of-range IFD0")? as usize;
    let mut gps_ifd_off: Option<usize> = None;
    for i in 0..ifd0_count {
        let entry = ifd0 + 2 + i * 12;
        let tag = rd16(entry).ok_or("truncated IFD0 entry")?;
        if tag == GPS_IFD_POINTER_TAG {
            gps_ifd_off = Some(rd32(entry + 8).ok_or("truncated GPS IFD pointer")? as usize);
            break;
        }
    }
    let gps_off = match gps_ifd_off {
        Some(o) => o,
        None => return Ok((tiff.to_vec(), 0)), // no GPS — nothing to remove
    };

    let gps_count = rd16(gps_off).ok_or("truncated or out-of-range GPS IFD")? as usize;
    let mut out = tiff.to_vec();

    // Zero the raw GPS values that live out-of-line in the TIFF data area, so no
    // location coordinate bytes survive as forensic residue.
    for i in 0..gps_count {
        let entry = gps_off + 2 + i * 12;
        let ty = rd16(entry + 2).ok_or("truncated GPS IFD entry")?;
        let cnt = rd32(entry + 4).ok_or("truncated GPS IFD entry")? as u64;
        let size = tiff_type_size(ty).saturating_mul(cnt);
        if size > 4 {
            let data_off = rd32(entry + 8).ok_or("truncated GPS IFD entry")? as usize;
            if let Some(end) = data_off.checked_add(size as usize) {
                if let Some(slice) = out.get_mut(data_off..end) {
                    slice.fill(0);
                }
            }
        }
    }

    // Empty the GPS IFD: zero the count + entries + next-IFD-offset region. With
    // count == 0 a reader sees no GPS entries and reads a null next-IFD pointer;
    // the IFD0 pointer now targets an empty IFD, so no GPS field is exposed.
    let region_end = gps_off
        .saturating_add(2 + gps_count * 12 + 4)
        .min(out.len());
    if let Some(slice) = out.get_mut(gps_off..region_end) {
        slice.fill(0);
    }

    Ok((out, gps_count))
}

/// Remove only the GPS/location tags from a JPEG/PNG, returning the cleaned
/// bytes and a report. Camera and exposure metadata is preserved.
pub fn remove_gps(input: &[u8]) -> Result<(Vec<u8>, GpsReport), String> {
    let (format, _mime, _ext) = detect_format(input).ok_or_else(|| {
        "unsupported image format: only JPEG and PNG photos are supported for GPS removal"
            .to_string()
    })?;
    let bytes = Bytes::copy_from_slice(input);
    match format {
        "jpeg" => {
            let mut jpeg = Jpeg::from_bytes(bytes).map_err(map_eof)?;
            let (removed, had_exif) = match jpeg.exif() {
                Some(exif) => {
                    let (new_exif, n) = strip_gps_from_tiff(&exif)?;
                    if n > 0 {
                        replace_jpeg_exif(&mut jpeg, new_exif);
                    }
                    (n, true)
                }
                None => (0, false),
            };
            let out = jpeg.encoder().bytes().to_vec();
            Ok(finish(out, "jpeg", input.len(), removed, had_exif))
        }
        "png" => {
            let mut png = Png::from_bytes(bytes).map_err(map_eof)?;
            let (removed, had_exif) = match png.exif() {
                Some(exif) => {
                    let (new_exif, n) = strip_gps_from_tiff(&exif)?;
                    if n > 0 {
                        png.set_exif(Some(Bytes::from(new_exif)));
                    }
                    (n, true)
                }
                None => (0, false),
            };
            let out = png.encoder().bytes().to_vec();
            Ok(finish(out, "png", input.len(), removed, had_exif))
        }
        _ => unreachable!("detect_format only returns jpeg|png"),
    }
}

fn finish(
    out: Vec<u8>,
    format: &str,
    input_len: usize,
    gps_tags_removed: usize,
    had_exif: bool,
) -> (Vec<u8>, GpsReport) {
    let output_bytes = out.len();
    let report = GpsReport {
        format: format.to_string(),
        input_bytes: input_len,
        output_bytes,
        had_gps: gps_tags_removed > 0,
        gps_tags_removed,
        had_exif,
    };
    (out, report)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- fixtures ---------------------------------------------------------

    // SOI + APP0 (JFIF) + SOS + a byte of entropy data + EOI. img-parts needs the
    // SOS..EOI tail to round-trip parse; pixels are never decoded here.
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

    // Build a little-endian TIFF/EXIF payload: IFD0 with Make="ACME\0" and a GPS
    // sub-IFD pointer; the GPS IFD holds one out-of-line RATIONAL[3] latitude.
    // Layout is laid out by hand so offsets are exact.
    fn tiff_with_make_and_gps() -> Vec<u8> {
        let mut b: Vec<u8> = Vec::new();
        let u16 = |v: u16| v.to_le_bytes();
        let u32 = |v: u32| v.to_le_bytes();

        // Header (8 bytes): "II", magic 42, IFD0 offset = 8.
        b.extend_from_slice(b"II");
        b.extend_from_slice(&u16(42));
        b.extend_from_slice(&u32(8));

        // IFD0 @ 8: 2 entries, next = 0. Data area laid out after.
        let make_off: u32 = 38; // "ACME\0" (5 bytes > 4 → out of line)
        let gps_ifd_off: u32 = 43; // GPS IFD start
        // entry 0: Make (0x010F) ASCII[5] @ make_off
        b.extend_from_slice(&u16(2)); // count
        b.extend_from_slice(&u16(0x010F));
        b.extend_from_slice(&u16(2)); // ASCII
        b.extend_from_slice(&u32(5));
        b.extend_from_slice(&u32(make_off));
        // entry 1: GPS IFD pointer (0x8825) LONG[1] = gps_ifd_off
        b.extend_from_slice(&u16(GPS_IFD_POINTER_TAG));
        b.extend_from_slice(&u16(4)); // LONG
        b.extend_from_slice(&u32(1));
        b.extend_from_slice(&u32(gps_ifd_off));
        // next IFD offset = 0
        b.extend_from_slice(&u32(0));
        debug_assert_eq!(b.len(), 38);
        // Make string @ 38
        b.extend_from_slice(b"ACME\0");
        debug_assert_eq!(b.len(), 43);

        // GPS IFD @ 43: 1 entry (GPSLatitude 0x0002 RATIONAL[3]) @ gps_data_off.
        let gps_data_off: u32 = 61;
        b.extend_from_slice(&u16(1)); // count
        b.extend_from_slice(&u16(0x0002)); // GPSLatitude
        b.extend_from_slice(&u16(5)); // RATIONAL
        b.extend_from_slice(&u32(3));
        b.extend_from_slice(&u32(gps_data_off));
        b.extend_from_slice(&u32(0)); // next IFD = 0
        debug_assert_eq!(b.len(), 61);
        // GPS latitude: 51/1, 30/1, 0/1 (deg, min, sec) — 3 rationals, 24 bytes.
        for (num, den) in [(51u32, 1u32), (30, 1), (0, 1)] {
            b.extend_from_slice(&u32(num));
            b.extend_from_slice(&u32(den));
        }
        debug_assert_eq!(b.len(), 85);
        b
    }

    fn tiff_make_only() -> Vec<u8> {
        let mut b: Vec<u8> = Vec::new();
        let u16 = |v: u16| v.to_le_bytes();
        let u32 = |v: u32| v.to_le_bytes();
        b.extend_from_slice(b"II");
        b.extend_from_slice(&u16(42));
        b.extend_from_slice(&u32(8));
        // IFD0 @ 8: 1 entry (Make), next = 0.
        b.extend_from_slice(&u16(1));
        b.extend_from_slice(&u16(0x010F));
        b.extend_from_slice(&u16(2));
        b.extend_from_slice(&u32(5));
        b.extend_from_slice(&u32(26)); // make_off
        b.extend_from_slice(&u32(0)); // next
        debug_assert_eq!(b.len(), 26);
        b.extend_from_slice(b"ACME\0");
        b
    }

    fn jpeg_with_exif(tiff: &[u8]) -> Vec<u8> {
        use img_parts::jpeg::{markers, JpegSegment};
        let mut jpeg = Jpeg::from_bytes(Bytes::from(minimal_jpeg())).unwrap();
        // Insert the APP1 (EXIF) segment manually — img-parts' set_exif mis-computes
        // the insert index on a JPEG that has no existing EXIF segment. The "Exif\0\0"
        // identifier prefix is what jpeg.exif() strips to hand back the raw TIFF.
        let mut app1 = b"Exif\0\0".to_vec();
        app1.extend_from_slice(tiff);
        let seg = JpegSegment::new_with_contents(markers::APP1, Bytes::from(app1));
        jpeg.segments_mut().insert(1, seg);
        jpeg.encoder().bytes().to_vec()
    }

    /// Collect the Debug names of every EXIF field kamadak surfaces from bytes.
    fn exif_field_names(image: &[u8]) -> Vec<String> {
        let exif = exif::Reader::new()
            .read_from_container(&mut std::io::Cursor::new(image))
            .expect("output should still contain readable EXIF");
        exif.fields()
            .map(|f| format!("{:?}", f.tag))
            .collect()
    }

    // ---- tests ------------------------------------------------------------

    #[test]
    fn removes_gps_keeps_camera() {
        let img = jpeg_with_exif(&tiff_with_make_and_gps());
        let (out, report) = remove_gps(&img).unwrap();

        assert_eq!(report.format, "jpeg");
        assert!(report.had_gps, "input should have carried GPS");
        assert_eq!(report.gps_tags_removed, 1);
        assert!(report.had_exif, "camera EXIF should be present + preserved");

        let names = exif_field_names(&out);
        assert!(
            names.iter().any(|n| n.contains("Make") || n.contains("271")),
            "camera Make must be preserved, got {names:?}"
        );
        assert!(
            !names.iter().any(|n| n.to_ascii_lowercase().contains("gps")),
            "every GPS field must be gone, got {names:?}"
        );
        // The raw latitude bytes (51,30,0) must be zeroed, not just unlinked.
        let out_exif = Jpeg::from_bytes(Bytes::from(out)).unwrap().exif().unwrap();
        assert!(
            !out_exif.windows(4).any(|w| w == 51u32.to_le_bytes()),
            "raw GPS coordinate residue must be zeroed"
        );
    }

    #[test]
    fn no_gps_is_noop_but_keeps_camera() {
        let img = jpeg_with_exif(&tiff_make_only());
        let (out, report) = remove_gps(&img).unwrap();

        assert!(!report.had_gps, "there was no GPS to remove");
        assert_eq!(report.gps_tags_removed, 0);
        assert!(report.had_exif);

        let names = exif_field_names(&out);
        assert!(
            names.iter().any(|n| n.contains("Make") || n.contains("271")),
            "Make preserved, got {names:?}"
        );
    }

    #[test]
    fn image_without_any_exif_is_unchanged() {
        let img = minimal_jpeg();
        let (out, report) = remove_gps(&img).unwrap();
        assert!(!report.had_gps);
        assert!(!report.had_exif, "no EXIF was present");
        // Round-trips as a valid JPEG.
        assert!(Jpeg::from_bytes(Bytes::from(out)).is_ok());
    }

    #[test]
    fn rejects_non_image_input() {
        let err = remove_gps(b"this is not an image at all").unwrap_err();
        assert!(err.contains("unsupported image format"), "got: {err}");
    }

    #[test]
    fn strip_gps_from_tiff_rejects_bad_byte_order() {
        let err = strip_gps_from_tiff(b"XX\x2a\x00\x08\x00\x00\x00").unwrap_err();
        assert!(err.contains("byte-order marker"), "got: {err}");
    }

    #[test]
    fn detect_format_recognizes_jpeg_and_png() {
        assert_eq!(detect_format(&[0xFF, 0xD8, 0xFF, 0xE0]).unwrap().0, "jpeg");
        assert_eq!(detect_format(PNG_MAGIC).unwrap().0, "png");
        assert!(detect_format(b"nope").is_none());
    }
}
