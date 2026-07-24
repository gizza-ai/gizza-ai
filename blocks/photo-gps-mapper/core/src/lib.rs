//! photo-gps-mapper core — extract EXIF GPS coordinates from a batch of photos
//! and render mapping-friendly exports (GeoJSON/CSV/GPX/KML/list). Pure Rust,
//! no wafer/wasm-bindgen deps.

use std::io::Cursor;

use exif::{In, Reader, Tag, Value};
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct InputPhoto {
    pub label: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Location {
    pub index: usize,
    pub source: String,
    pub latitude: f64,
    pub longitude: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub altitude_m: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Report {
    pub format: String,
    pub total: usize,
    pub with_gps: usize,
    pub without_gps: Vec<String>,
    pub locations: Vec<Location>,
    pub output: String,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum OutputFormat {
    GeoJson,
    Csv,
    Gpx,
    Kml,
    List,
}

impl OutputFormat {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "geojson" => Ok(Self::GeoJson),
            "csv" => Ok(Self::Csv),
            "gpx" => Ok(Self::Gpx),
            "kml" => Ok(Self::Kml),
            "list" => Ok(Self::List),
            other => Err(format!(
                "unsupported format `{other}` (expected geojson, csv, gpx, kml, or list)"
            )),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::GeoJson => "geojson",
            Self::Csv => "csv",
            Self::Gpx => "gpx",
            Self::Kml => "kml",
            Self::List => "list",
        }
    }
}

pub fn map_photos(
    photos: &[InputPhoto],
    format: OutputFormat,
    precision: u8,
) -> Result<Report, String> {
    if photos.is_empty() {
        return Err("photo-gps-mapper needs at least 1 image".into());
    }
    let precision = precision.min(10);
    let mut locations = Vec::new();
    let mut without_gps = Vec::new();

    for (idx, photo) in photos.iter().enumerate() {
        match extract_location(idx, &photo.label, &photo.bytes) {
            Ok(Some(loc)) => locations.push(loc),
            Ok(None) => without_gps.push(photo.label.clone()),
            Err(e) => return Err(format!("{}: {e}", photo.label)),
        }
    }

    if locations.is_empty() {
        return Err(format!(
            "none of the {} photo(s) contained EXIF GPS coordinates",
            photos.len()
        ));
    }

    let output = render_output(&locations, format, precision);
    Ok(Report {
        format: format.as_str().to_string(),
        total: photos.len(),
        with_gps: locations.len(),
        without_gps,
        locations,
        output,
    })
}

fn extract_location(index: usize, label: &str, bytes: &[u8]) -> Result<Option<Location>, String> {
    let exif = match Reader::new().read_from_container(&mut Cursor::new(bytes)) {
        Ok(exif) => exif,
        Err(exif::Error::NotFound(_)) => return Ok(None),
        Err(e) => return Err(format!("could not read EXIF metadata ({e})")),
    };

    let lat = match gps_coord(&exif, Tag::GPSLatitude, Tag::GPSLatitudeRef, true)? {
        Some(v) => v,
        None => return Ok(None),
    };
    let lon = match gps_coord(&exif, Tag::GPSLongitude, Tag::GPSLongitudeRef, false)? {
        Some(v) => v,
        None => return Ok(None),
    };
    let altitude_m = altitude(&exif)?;
    let timestamp =
        ascii_field(&exif, Tag::DateTimeOriginal).or_else(|| ascii_field(&exif, Tag::DateTime));

    Ok(Some(Location {
        index,
        source: label.to_string(),
        latitude: lat,
        longitude: lon,
        altitude_m,
        timestamp,
    }))
}

fn gps_coord(
    exif: &exif::Exif,
    value_tag: Tag,
    ref_tag: Tag,
    is_lat: bool,
) -> Result<Option<f64>, String> {
    let Some(field) = exif.get_field(value_tag, In::PRIMARY) else {
        return Ok(None);
    };
    let Value::Rational(parts) = &field.value else {
        return Err(format!("{value_tag:?} is not a rational DMS value"));
    };
    if parts.len() < 3 {
        return Err(format!("{value_tag:?} has fewer than 3 DMS components"));
    }
    let mut deg = rational(parts[0])? + rational(parts[1])? / 60.0 + rational(parts[2])? / 3600.0;
    let hemi = ascii_field(exif, ref_tag)
        .unwrap_or_default()
        .to_ascii_uppercase();
    if hemi.starts_with(if is_lat { 'S' } else { 'W' }) {
        deg = -deg;
    }
    Ok(Some(deg))
}

fn altitude(exif: &exif::Exif) -> Result<Option<f64>, String> {
    let Some(field) = exif.get_field(Tag::GPSAltitude, In::PRIMARY) else {
        return Ok(None);
    };
    let Value::Rational(vals) = &field.value else {
        return Err("GPSAltitude is not a rational value".into());
    };
    let Some(first) = vals.first() else {
        return Ok(None);
    };
    let mut alt = rational(*first)?;
    if let Some(field) = exif.get_field(Tag::GPSAltitudeRef, In::PRIMARY) {
        if let Value::Byte(bytes) = &field.value {
            if bytes.first() == Some(&1) {
                alt = -alt;
            }
        }
    }
    Ok(Some(alt))
}

fn rational(r: exif::Rational) -> Result<f64, String> {
    if r.denom == 0 {
        return Err("EXIF rational has denominator 0".into());
    }
    Ok(r.num as f64 / r.denom as f64)
}

fn ascii_field(exif: &exif::Exif, tag: Tag) -> Option<String> {
    let field = exif.get_field(tag, In::PRIMARY)?;
    match &field.value {
        Value::Ascii(items) => items.first().map(|b| {
            String::from_utf8_lossy(b)
                .trim_matches('\0')
                .trim()
                .to_string()
        }),
        _ => None,
    }
}

fn render_output(locations: &[Location], format: OutputFormat, precision: u8) -> String {
    match format {
        OutputFormat::GeoJson => render_geojson(locations, precision),
        OutputFormat::Csv => render_csv(locations, precision),
        OutputFormat::Gpx => render_gpx(locations, precision),
        OutputFormat::Kml => render_kml(locations, precision),
        OutputFormat::List => render_list(locations, precision),
    }
}

fn rounded(v: f64, precision: u8) -> f64 {
    let scale = 10_f64.powi(precision as i32);
    (v * scale).round() / scale
}

fn fmt(v: f64, precision: u8) -> String {
    format!("{:.*}", precision as usize, rounded(v, precision))
}

fn render_geojson(locations: &[Location], precision: u8) -> String {
    let features: Vec<serde_json::Value> = locations
        .iter()
        .map(|loc| {
            let mut props = serde_json::Map::new();
            props.insert(
                "source".into(),
                serde_json::Value::String(loc.source.clone()),
            );
            props.insert("index".into(), serde_json::Value::from(loc.index));
            if let Some(t) = &loc.timestamp {
                props.insert("timestamp".into(), serde_json::Value::String(t.clone()));
            }
            let mut coords = vec![
                serde_json::Value::from(rounded(loc.longitude, precision)),
                serde_json::Value::from(rounded(loc.latitude, precision)),
            ];
            if let Some(alt) = loc.altitude_m {
                coords.push(serde_json::Value::from(rounded(alt, precision)));
            }
            serde_json::json!({
                "type": "Feature",
                "properties": props,
                "geometry": { "type": "Point", "coordinates": coords }
            })
        })
        .collect();
    serde_json::to_string_pretty(&serde_json::json!({
        "type": "FeatureCollection",
        "features": features
    }))
    .expect("GeoJSON serialization cannot fail")
}

fn render_csv(locations: &[Location], precision: u8) -> String {
    let mut out = "source,latitude,longitude,altitude_m,timestamp\n".to_string();
    for loc in locations {
        out.push_str(&csv_escape(&loc.source));
        out.push(',');
        out.push_str(&fmt(loc.latitude, precision));
        out.push(',');
        out.push_str(&fmt(loc.longitude, precision));
        out.push(',');
        if let Some(alt) = loc.altitude_m {
            out.push_str(&fmt(alt, precision));
        }
        out.push(',');
        if let Some(t) = &loc.timestamp {
            out.push_str(&csv_escape(t));
        }
        out.push('\n');
    }
    out
}

fn render_gpx(locations: &[Location], precision: u8) -> String {
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<gpx version=\"1.1\" creator=\"photo-gps-mapper\" xmlns=\"http://www.topografix.com/GPX/1/1\">\n");
    for loc in locations {
        out.push_str(&format!(
            "  <wpt lat=\"{}\" lon=\"{}\">\n    <name>{}</name>\n",
            fmt(loc.latitude, precision),
            fmt(loc.longitude, precision),
            xml_escape(&loc.source)
        ));
        if let Some(alt) = loc.altitude_m {
            out.push_str(&format!("    <ele>{}</ele>\n", fmt(alt, precision)));
        }
        if let Some(t) = &loc.timestamp {
            out.push_str(&format!("    <desc>{}</desc>\n", xml_escape(t)));
        }
        out.push_str("  </wpt>\n");
    }
    out.push_str("</gpx>\n");
    out
}

fn render_kml(locations: &[Location], precision: u8) -> String {
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<kml xmlns=\"http://www.opengis.net/kml/2.2\">\n<Document>\n");
    for loc in locations {
        let alt = loc.altitude_m.unwrap_or(0.0);
        out.push_str(&format!(
            "  <Placemark><name>{}</name><Point><coordinates>{},{},{}</coordinates></Point></Placemark>\n",
            xml_escape(&loc.source),
            fmt(loc.longitude, precision),
            fmt(loc.latitude, precision),
            fmt(alt, precision)
        ));
    }
    out.push_str("</Document>\n</kml>\n");
    out
}

fn render_list(locations: &[Location], precision: u8) -> String {
    let mut out = String::new();
    for loc in locations {
        out.push_str(&format!(
            "{}: {}, {}",
            loc.source,
            fmt(loc.latitude, precision),
            fmt(loc.longitude, precision)
        ));
        if let Some(alt) = loc.altitude_m {
            out.push_str(&format!(" (alt {} m)", fmt(alt, precision)));
        }
        out.push('\n');
    }
    out
}

fn csv_escape(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn push_ifd_entry(b: &mut Vec<u8>, tag: u16, ty: u16, count: u32, value_or_offset: u32) {
        b.extend_from_slice(&tag.to_le_bytes());
        b.extend_from_slice(&ty.to_le_bytes());
        b.extend_from_slice(&count.to_le_bytes());
        b.extend_from_slice(&value_or_offset.to_le_bytes());
    }

    fn tiff_with_gps() -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(b"II");
        b.extend_from_slice(&42u16.to_le_bytes());
        b.extend_from_slice(&8u32.to_le_bytes());

        let datetime = b"2026:07:24 04:18:18\0";
        let ifd0_count = 2u16;
        let ifd0_data_start = 8 + 2 + ifd0_count as usize * 12 + 4;
        let dt_off = ifd0_data_start as u32;
        let gps_ifd_off = (ifd0_data_start + datetime.len()) as u32;

        b.extend_from_slice(&ifd0_count.to_le_bytes());
        push_ifd_entry(&mut b, 0x0132, 2, datetime.len() as u32, dt_off);
        push_ifd_entry(&mut b, 0x8825, 4, 1, gps_ifd_off);
        b.extend_from_slice(&0u32.to_le_bytes());
        assert_eq!(b.len(), ifd0_data_start);
        b.extend_from_slice(datetime);
        assert_eq!(b.len(), gps_ifd_off as usize);

        let gps_count = 6u16;
        let gps_data_start = gps_ifd_off as usize + 2 + gps_count as usize * 12 + 4;
        let lat_off = gps_data_start as u32;
        let lon_off = lat_off + 24;
        let alt_off = lon_off + 24;
        b.extend_from_slice(&gps_count.to_le_bytes());
        push_ifd_entry(&mut b, 0x0001, 2, 2, u32::from_le_bytes([b'N', 0, 0, 0]));
        push_ifd_entry(&mut b, 0x0002, 5, 3, lat_off);
        push_ifd_entry(&mut b, 0x0003, 2, 2, u32::from_le_bytes([b'W', 0, 0, 0]));
        push_ifd_entry(&mut b, 0x0004, 5, 3, lon_off);
        push_ifd_entry(&mut b, 0x0005, 1, 1, 1); // below sea level
        push_ifd_entry(&mut b, 0x0006, 5, 1, alt_off);
        b.extend_from_slice(&0u32.to_le_bytes());
        assert_eq!(b.len(), gps_data_start);
        for (num, den) in [(51u32, 1u32), (30, 1), (0, 1)] {
            b.extend_from_slice(&num.to_le_bytes());
            b.extend_from_slice(&den.to_le_bytes());
        }
        for (num, den) in [(0u32, 1u32), (7, 1), (30, 1)] {
            b.extend_from_slice(&num.to_le_bytes());
            b.extend_from_slice(&den.to_le_bytes());
        }
        b.extend_from_slice(&12u32.to_le_bytes());
        b.extend_from_slice(&1u32.to_le_bytes());
        b
    }

    fn tiff_without_gps() -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(b"II");
        b.extend_from_slice(&42u16.to_le_bytes());
        b.extend_from_slice(&8u32.to_le_bytes());
        b.extend_from_slice(&0u16.to_le_bytes());
        b.extend_from_slice(&0u32.to_le_bytes());
        b
    }

    #[test]
    fn extracts_gps_and_renders_geojson() {
        let report = map_photos(
            &[InputPhoto {
                label: "westminster.tif".into(),
                bytes: tiff_with_gps(),
            }],
            OutputFormat::GeoJson,
            6,
        )
        .unwrap();
        assert_eq!(report.total, 1);
        assert_eq!(report.with_gps, 1);
        assert!((report.locations[0].latitude - 51.5).abs() < 0.000001);
        assert!((report.locations[0].longitude + 0.125).abs() < 0.000001);
        assert_eq!(report.locations[0].altitude_m, Some(-12.0));
        assert!(report.output.contains("FeatureCollection"));
        assert!(report.output.contains("-0.125"));
    }

    #[test]
    fn csv_lists_missing_photos() {
        let report = map_photos(
            &[
                InputPhoto {
                    label: "gps.tif".into(),
                    bytes: tiff_with_gps(),
                },
                InputPhoto {
                    label: "plain.tif".into(),
                    bytes: tiff_without_gps(),
                },
            ],
            OutputFormat::Csv,
            3,
        )
        .unwrap();
        assert_eq!(report.total, 2);
        assert_eq!(report.with_gps, 1);
        assert_eq!(report.without_gps, vec!["plain.tif"]);
        assert_eq!(
            report.output,
            "source,latitude,longitude,altitude_m,timestamp\ngps.tif,51.500,-0.125,-12.000,2026:07:24 04:18:18\n"
        );
    }

    #[test]
    fn errors_when_no_photo_has_gps() {
        let err = map_photos(
            &[InputPhoto {
                label: "plain.tif".into(),
                bytes: tiff_without_gps(),
            }],
            OutputFormat::List,
            6,
        )
        .unwrap_err();
        assert!(err.contains("none of the 1 photo(s) contained EXIF GPS"));
    }

    #[test]
    fn rejects_unknown_format() {
        let err = OutputFormat::parse("xlsx").unwrap_err();
        assert!(err.contains("unsupported format"));
    }

    #[test]
    fn xml_outputs_escape_names() {
        let report = map_photos(
            &[InputPhoto {
                label: "a&b.tif".into(),
                bytes: tiff_with_gps(),
            }],
            OutputFormat::Kml,
            2,
        )
        .unwrap();
        assert!(report.output.contains("a&amp;b.tif"));
        assert!(report.output.contains("-0.13,51.50,-12.00"));
    }
}
