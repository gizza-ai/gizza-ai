//! gizza-ai/image-metadata-viewer core — read EXIF/TIFF metadata (camera, GPS,
//! timestamps) from an image. Pure-Rust (`kamadak-exif`). Returns the full field
//! list plus decoded GPS coordinates when present.

use std::io::Cursor;

use exif::{In, Tag, Value};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FieldOut {
    /// IFD the field belongs to ("0" = primary/0th, "1" = thumbnail, etc).
    pub ifd: String,
    /// Tag name, e.g. "Make", "ExposureTime", "GPSLatitude".
    pub tag: String,
    /// Human-readable value (with units where applicable).
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Gps {
    pub latitude: f64,
    pub longitude: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Meta {
    pub count: usize,
    pub fields: Vec<FieldOut>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gps: Option<Gps>,
}

/// Convert a 3-rational [deg, min, sec] + hemisphere ref to signed decimal.
fn dms_to_decimal(exif: &exif::Exif, coord: Tag, refr: Tag) -> Option<f64> {
    let v = exif.get_field(coord, In::PRIMARY)?;
    let parts = match &v.value {
        Value::Rational(r) if r.len() >= 3 => [r[0].to_f64(), r[1].to_f64(), r[2].to_f64()],
        _ => return None,
    };
    let mut deg = parts[0] + parts[1] / 60.0 + parts[2] / 3600.0;
    if let Some(rf) = exif.get_field(refr, In::PRIMARY) {
        let s = rf.display_value().to_string().to_ascii_uppercase();
        if s.contains('S') || s.contains('W') {
            deg = -deg;
        }
    }
    Some((deg * 1e6).round() / 1e6)
}

/// Parse image bytes and extract metadata.
pub fn read_metadata(bytes: &[u8]) -> Result<Meta, String> {
    if bytes.is_empty() {
        return Err("input is empty".into());
    }
    let exif = exif::Reader::new()
        .read_from_container(&mut Cursor::new(bytes))
        .map_err(|e| format!("no readable metadata: {e}"))?;

    let fields: Vec<FieldOut> = exif
        .fields()
        .map(|f| FieldOut {
            ifd: f.ifd_num.to_string(),
            tag: f.tag.to_string(),
            value: f.display_value().with_unit(&exif).to_string(),
        })
        .collect();

    let gps = match (
        dms_to_decimal(&exif, Tag::GPSLatitude, Tag::GPSLatitudeRef),
        dms_to_decimal(&exif, Tag::GPSLongitude, Tag::GPSLongitudeRef),
    ) {
        (Some(lat), Some(lon)) => Some(Gps { latitude: lat, longitude: lon }),
        _ => None,
    };

    Ok(Meta { count: fields.len(), fields, gps })
}

/// Human-readable rendering (used where a text view is wanted).
pub fn render(bytes: &[u8]) -> Result<String, String> {
    let m = read_metadata(bytes)?;
    if m.count == 0 {
        return Ok("No EXIF metadata found.".to_string());
    }
    let mut out = format!("{} metadata field(s):\n", m.count);
    for f in &m.fields {
        out.push_str(&format!("[{}] {}: {}\n", f.ifd, f.tag, f.value));
    }
    if let Some(g) = &m.gps {
        out.push_str(&format!("\nGPS: {}, {}\n", g.latitude, g.longitude));
    }
    Ok(out.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Build a tiny JPEG carrying an EXIF APP1 with Make="ACME" using the `img-parts`-free
    /// approach: hand-assemble a minimal JPEG with an EXIF segment is complex, so instead
    /// we test against a real-ish EXIF blob embedded below.
    /// A minimal valid TIFF/EXIF (little-endian) with one IFD0 field: Make = "T".
    fn tiff_exif_make() -> Vec<u8> {
        // TIFF header (LE) + 1 IFD entry: tag 0x010F (Make), type ASCII, count 2, value "T\0".
        let mut t: Vec<u8> = Vec::new();
        t.extend_from_slice(b"II"); // little-endian
        t.extend_from_slice(&42u16.to_le_bytes()); // magic
        t.extend_from_slice(&8u32.to_le_bytes()); // IFD0 offset
        t.extend_from_slice(&1u16.to_le_bytes()); // 1 entry
        t.extend_from_slice(&0x010Fu16.to_le_bytes()); // Make
        t.extend_from_slice(&2u16.to_le_bytes()); // ASCII
        t.extend_from_slice(&2u32.to_le_bytes()); // count
        t.extend_from_slice(b"T\0\0\0"); // inline value "T\0"
        t.extend_from_slice(&0u32.to_le_bytes()); // next IFD = 0
        t
    }

    fn jpeg_with_exif(tiff: &[u8]) -> Vec<u8> {
        let mut j: Vec<u8> = vec![0xFF, 0xD8]; // SOI
        let payload_len = 6 + tiff.len(); // "Exif\0\0" + tiff
        j.push(0xFF);
        j.push(0xE1); // APP1
        j.write_all(&((payload_len + 2) as u16).to_be_bytes()).unwrap();
        j.extend_from_slice(b"Exif\0\0");
        j.extend_from_slice(tiff);
        j.extend_from_slice(&[0xFF, 0xD9]); // EOI
        j
    }

    #[test]
    fn reads_make_field() {
        let jpeg = jpeg_with_exif(&tiff_exif_make());
        let m = read_metadata(&jpeg).unwrap();
        assert!(m.count >= 1);
        let make = m.fields.iter().find(|f| f.tag == "Make").expect("Make field");
        assert!(make.value.contains('T'), "got {:?}", make.value);
        assert!(m.gps.is_none());
    }

    #[test]
    fn render_lists_fields() {
        let jpeg = jpeg_with_exif(&tiff_exif_make());
        let r = render(&jpeg).unwrap();
        assert!(r.contains("Make"));
    }

    #[test]
    fn errors_on_garbage() {
        assert!(read_metadata(b"").is_err());
        assert!(read_metadata(b"not an image with exif").is_err());
    }
}
