//! kml-to-geojson core — convert KML (or a zipped KMZ archive) into GeoJSON,
//! and convert GeoJSON back into KML. Pure Rust, no wafer/wasm-bindgen deps;
//! shared by the chat skill block and the web page.
//!
//! The forward Placemark parse (geometry, `ExtendedData`, `TimeSpan`/
//! `TimeStamp`, `Style`/`styleUrl`/`StyleMap` → simplestyle properties) lives
//! in the `kml_parse` module. On top of it this crate adds:
//!
//! * **KMZ input** — a KMZ is a zip archive. Every surface here (chat, CLI,
//!   page) carries text, so a KMZ arrives base64-encoded; it is detected by its
//!   `UEsD` (`PK\x03\x04`) prefix, unzipped, and the `doc.kml` entry (or the
//!   first `*.kml`) is converted.
//! * **Folder paths** — KML's `<Folder>` nesting is flattened into a `folder`
//!   property (`"Trails/Day 1"`) on each feature, and read back on the reverse
//!   trip to rebuild the `<Folder>` tree.
//! * **Coordinate precision** — positions are rounded to a chosen number of
//!   decimal places (6 ≈ 0.1 m), which is what keeps a converted file small.
//! * **GeoJSON → KML** — Point/MultiPoint/LineString/MultiLineString/Polygon/
//!   MultiPolygon/GeometryCollection → the matching KML geometry
//!   (`MultiGeometry` for the plural forms), `name`/`description` properties →
//!   `<name>`/`<description>`, every other property → `<ExtendedData>`,
//!   simplestyle properties → an inline `<Style>` (`LineStyle`/`PolyStyle`/
//!   `IconStyle`), and a chosen `<altitudeMode>`.
//!
//! Both formats are WGS84 (KML is WGS84 by specification), so no reprojection
//! is involved in either direction.

use base64::Engine;
use quick_xml::events::Event;
use quick_xml::name::QName;
use quick_xml::reader::Reader;
use serde_json::{json, Map, Value};
use std::io::{Cursor, Read};

mod kml_parse;

/// Largest accepted input, in bytes. The wasm sandbox has 64 MiB total, and a
/// KML document expands substantially while parsing, so the input itself is
/// capped well below that.
pub const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;

/// Largest accepted KML entry inside a KMZ, in bytes (a zip-bomb guard: the
/// compressed archive can be far smaller than what it expands to).
pub const MAX_KMZ_ENTRY_BYTES: u64 = 8 * 1024 * 1024;

/// Which direction to convert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// KML XML or a base64 KMZ archive → a GeoJSON `FeatureCollection`.
    GeoJson,
    /// GeoJSON input → a KML document.
    Kml,
}

impl OutputFormat {
    pub fn parse(s: &str) -> Result<OutputFormat, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "geojson" | "" => Ok(OutputFormat::GeoJson),
            "kml" => Ok(OutputFormat::Kml),
            other => Err(format!("unknown output_format '{other}' (use geojson or kml)")),
        }
    }
}

/// How KML should interpret a position's altitude (GeoJSON → KML only).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AltitudeMode {
    ClampToGround,
    RelativeToGround,
    Absolute,
}

impl AltitudeMode {
    pub fn parse(s: &str) -> Result<AltitudeMode, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "clamp_to_ground" | "clamptoground" | "" => Ok(AltitudeMode::ClampToGround),
            "relative_to_ground" | "relativetoground" => Ok(AltitudeMode::RelativeToGround),
            "absolute" => Ok(AltitudeMode::Absolute),
            other => Err(format!(
                "unknown altitude_mode '{other}' (use clamp_to_ground, relative_to_ground, or absolute)"
            )),
        }
    }

    /// The KML spelling of this mode.
    pub fn as_kml(self) -> &'static str {
        match self {
            AltitudeMode::ClampToGround => "clampToGround",
            AltitudeMode::RelativeToGround => "relativeToGround",
            AltitudeMode::Absolute => "absolute",
        }
    }
}

pub struct Options {
    pub output_format: OutputFormat,
    /// KML → GeoJSON: fold each Placemark's resolved Style into simplestyle
    /// properties. GeoJSON → KML: turn those same properties back into an
    /// inline `<Style>`.
    pub include_styles: bool,
    /// KML → GeoJSON: record each Placemark's `<Folder>` path as a `folder`
    /// property. GeoJSON → KML: regroup features into `<Folder>` elements from
    /// that property.
    pub include_folders: bool,
    /// Decimal places kept on every coordinate (0–15).
    pub precision: u32,
    /// GeoJSON → KML: the `<Document>` name.
    pub document_name: String,
    /// GeoJSON → KML: the `<altitudeMode>` written on every geometry.
    pub altitude_mode: AltitudeMode,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            output_format: OutputFormat::GeoJson,
            include_styles: true,
            include_folders: true,
            precision: 6,
            document_name: "GeoJSON Export".to_string(),
            altitude_mode: AltitudeMode::ClampToGround,
        }
    }
}

/// Convert `input` per `opt`. See the module doc for the exact mapping in each
/// direction.
pub fn convert(input: &str, opt: &Options) -> Result<String, String> {
    if input.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input is {} bytes; the limit is {MAX_INPUT_BYTES} bytes (2 MB). Split the document \
             or drop unused folders before converting.",
            input.len()
        ));
    }
    if opt.precision > 15 {
        return Err(format!("precision must be between 0 and 15 (got {})", opt.precision));
    }
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(
            "input is empty — paste a KML document, a base64-encoded KMZ archive, or a GeoJSON \
             document"
                .to_string(),
        );
    }

    match opt.output_format {
        OutputFormat::GeoJson => {
            let kml = source_kml(trimmed)?;
            kml_to_geojson(&kml, opt)
        }
        OutputFormat::Kml => {
            if trimmed.starts_with('<') || looks_like_base64_zip(trimmed) {
                return Err(
                    "output_format=\"kml\" expects GeoJSON input (a FeatureCollection, Feature, \
                     or bare geometry), but this looks like KML/KMZ already. Set \
                     output_format=\"geojson\" to convert KML into GeoJSON instead."
                        .to_string(),
                );
            }
            geojson_to_kml(trimmed, opt)
        }
    }
}

// ---------------------------------------------------------------------------
// Input detection: KML text vs. a base64 KMZ archive
// ---------------------------------------------------------------------------

/// Strip a `data:...;base64,` prefix and all whitespace, leaving the raw
/// base64 alphabet.
fn clean_base64(s: &str) -> String {
    let body = match s.find(";base64,") {
        Some(i) if s.starts_with("data:") => &s[i + ";base64,".len()..],
        _ => s,
    };
    body.chars().filter(|c| !c.is_whitespace()).collect()
}

/// True when the text is base64 whose first bytes are a zip local-file header
/// (`PK\x03\x04` encodes as `UEsD`) — i.e. a base64-encoded KMZ.
pub fn looks_like_base64_zip(s: &str) -> bool {
    let cleaned = clean_base64(s);
    cleaned.starts_with("UEsD")
        && cleaned.len() >= 8
        && cleaned
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=')
}

/// Resolve the input down to KML text: pass XML through, unzip a base64 KMZ,
/// and reject anything that looks like GeoJSON (with the fix in the message).
fn source_kml(trimmed: &str) -> Result<String, String> {
    if trimmed.starts_with('<') {
        if !trimmed.to_ascii_lowercase().contains("<kml") {
            return Err(
                "the input is XML but has no <kml> root element — this tool converts KML/KMZ \
                 map data"
                    .to_string(),
            );
        }
        return Ok(trimmed.to_string());
    }
    if looks_like_base64_zip(trimmed) {
        return kmz_to_kml(trimmed);
    }
    Err(
        "output_format=\"geojson\" expects a KML document or a base64-encoded KMZ archive, but \
         this looks like GeoJSON already. Set output_format=\"kml\" to convert GeoJSON into KML \
         instead."
            .to_string(),
    )
}

/// Decode a base64 KMZ and return its KML entry (`doc.kml` when present, else
/// the first `*.kml`).
fn kmz_to_kml(b64: &str) -> Result<String, String> {
    let mut cleaned = clean_base64(b64);
    while cleaned.len() % 4 != 0 {
        cleaned.push('=');
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(cleaned.as_bytes())
        .map_err(|e| format!("input starts like a base64 KMZ but is not valid base64: {e}"))?;

    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|e| format!("input is not a readable KMZ (zip) archive: {e}"))?;

    let mut names: Vec<String> = Vec::new();
    for i in 0..archive.len() {
        let name = archive
            .by_index(i)
            .map_err(|e| format!("could not read KMZ entry {i}: {e}"))?
            .name()
            .to_string();
        if !name.ends_with('/') {
            names.push(name);
        }
    }
    let pick = names
        .iter()
        .find(|n| n.rsplit('/').next().unwrap_or(n).eq_ignore_ascii_case("doc.kml"))
        .or_else(|| names.iter().find(|n| n.to_ascii_lowercase().ends_with(".kml")))
        .cloned()
        .ok_or_else(|| {
            format!(
                "the KMZ archive has no .kml entry (it contains: {})",
                if names.is_empty() { "nothing".to_string() } else { names.join(", ") }
            )
        })?;

    let mut entry = archive
        .by_name(&pick)
        .map_err(|e| format!("could not open '{pick}' inside the KMZ: {e}"))?;
    if entry.size() > MAX_KMZ_ENTRY_BYTES {
        return Err(format!(
            "'{pick}' inside the KMZ expands to {} bytes; the limit is {MAX_KMZ_ENTRY_BYTES} bytes (8 MB)",
            entry.size()
        ));
    }
    let mut xml = String::new();
    entry
        .read_to_string(&mut xml)
        .map_err(|e| format!("'{pick}' inside the KMZ is not valid UTF-8 text: {e}"))?;
    Ok(xml)
}

// ---------------------------------------------------------------------------
// KML → GeoJSON
// ---------------------------------------------------------------------------

/// Round `v` to `precision` decimal places, leaving non-finite values alone.
fn round_to(v: f64, precision: u32) -> f64 {
    if !v.is_finite() || precision >= 15 {
        return v;
    }
    let m = 10f64.powi(precision as i32);
    let r = (v * m).round() / m;
    // -0.0 reads badly in a coordinate list.
    if r == 0.0 {
        0.0
    } else {
        r
    }
}

/// Round every number under any `"coordinates"` key, in place.
fn round_coordinates(v: &mut Value, precision: u32) {
    match v {
        Value::Object(map) => {
            for (k, child) in map.iter_mut() {
                if k == "coordinates" {
                    round_numbers(child, precision);
                } else {
                    round_coordinates(child, precision);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                round_coordinates(item, precision);
            }
        }
        _ => {}
    }
}

fn round_numbers(v: &mut Value, precision: u32) {
    match v {
        Value::Array(items) => {
            for item in items {
                round_numbers(item, precision);
            }
        }
        Value::Number(_) => {
            if let Some(f) = v.as_f64() {
                *v = json!(round_to(f, precision));
            }
        }
        _ => {}
    }
}

/// Strip a `ns:local` prefix, returning the local name.
fn local_name(name: QName) -> String {
    let raw = String::from_utf8_lossy(name.as_ref()).to_string();
    match raw.rsplit_once(':') {
        Some((_, local)) => local.to_string(),
        None => raw,
    }
}

/// One `<Folder>` path per Placemark that carries at least one non-empty
/// `<coordinates>` element — i.e. per Placemark the reused parser turns into a
/// feature, in document order. Placemarks outside any folder get `""`.
fn folder_paths(xml: &str) -> Result<Vec<String>, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut paths: Vec<String> = Vec::new();
    // (depth of the <Folder> element, its <name>)
    let mut folders: Vec<(usize, String)> = Vec::new();
    let mut depth = 0usize;
    let mut in_placemark = false;
    let mut pm_has_coords = false;
    let mut pm_path = String::new();
    let mut capturing_folder_name = false;
    let mut capturing_coords = false;
    let mut text = String::new();
    let mut buf = Vec::new();

    let path_of = |folders: &[(usize, String)]| -> String {
        folders
            .iter()
            .map(|(_, n)| n.trim())
            .filter(|n| !n.is_empty())
            .collect::<Vec<_>>()
            .join("/")
    };

    loop {
        match reader.read_event_into(&mut buf) {
            Err(e) => return Err(format!("malformed KML/XML: {e}")),
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) => {
                let name = local_name(e.name());
                match name.as_str() {
                    "Folder" => folders.push((depth, String::new())),
                    "Placemark" => {
                        in_placemark = true;
                        pm_has_coords = false;
                        pm_path = path_of(&folders);
                    }
                    "name" if !in_placemark => {
                        if folders.last().is_some_and(|(fd, _)| *fd + 1 == depth) {
                            capturing_folder_name = true;
                        }
                    }
                    "coordinates" if in_placemark => capturing_coords = true,
                    _ => {}
                }
                depth += 1;
                text.clear();
            }
            Ok(Event::Text(t)) => {
                if capturing_folder_name || capturing_coords {
                    text.push_str(&String::from_utf8_lossy(&t));
                }
            }
            Ok(Event::CData(t)) => {
                if capturing_folder_name || capturing_coords {
                    text.push_str(&String::from_utf8_lossy(&t));
                }
            }
            Ok(Event::End(e)) => {
                depth = depth.saturating_sub(1);
                match local_name(e.name()).as_str() {
                    "name" if capturing_folder_name => {
                        if let Some(last) = folders.last_mut() {
                            let raw = text.trim();
                            last.1 = quick_xml::escape::unescape(raw)
                                .map(|c| c.into_owned())
                                .unwrap_or_else(|_| raw.to_string());
                        }
                        capturing_folder_name = false;
                    }
                    "coordinates" if capturing_coords => {
                        if !text.trim().is_empty() {
                            pm_has_coords = true;
                        }
                        capturing_coords = false;
                    }
                    "Placemark" => {
                        if in_placemark && pm_has_coords {
                            paths.push(std::mem::take(&mut pm_path));
                        }
                        in_placemark = false;
                    }
                    "Folder" => {
                        folders.pop();
                    }
                    _ => {}
                }
                text.clear();
            }
            _ => {}
        }
        buf.clear();
    }
    Ok(paths)
}

/// KML text → a pretty-printed GeoJSON `FeatureCollection`.
fn kml_to_geojson(xml: &str, opt: &Options) -> Result<String, String> {
    let raw = kml_parse::kml_to_geojson(xml, opt.include_styles)?;
    let mut val: Value =
        serde_json::from_str(&raw).map_err(|e| format!("internal: unreadable GeoJSON: {e}"))?;

    if opt.include_folders {
        let paths = folder_paths(xml)?;
        if let Some(feats) = val.get_mut("features").and_then(Value::as_array_mut) {
            // Only annotate when the two passes agree feature-for-feature; a
            // mismatch means an exotic document shape, and a missing `folder`
            // property is far better than a wrong one.
            if paths.len() == feats.len() {
                for (feat, path) in feats.iter_mut().zip(paths) {
                    if path.is_empty() {
                        continue;
                    }
                    if let Some(props) = feat.get_mut("properties").and_then(Value::as_object_mut) {
                        props.insert("folder".to_string(), json!(path));
                    }
                }
            }
        }
    }

    round_coordinates(&mut val, opt.precision);
    serde_json::to_string_pretty(&val).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// GeoJSON → KML
// ---------------------------------------------------------------------------

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Property names that become KML structure rather than `<ExtendedData>`.
const RESERVED_PROPS: [&str; 10] = [
    "name",
    "description",
    "desc",
    "folder",
    "stroke",
    "stroke-width",
    "stroke-opacity",
    "fill",
    "fill-opacity",
    "marker-color",
];

/// `#rrggbb` (or `#rgb`) + an optional 0–1 opacity → KML's `aabbggrr`.
fn kml_color(hex: &str, opacity: Option<f64>) -> Option<String> {
    let digits = hex.trim().strip_prefix('#')?;
    let full: String = match digits.len() {
        3 => digits.chars().flat_map(|c| [c, c]).collect(),
        6 => digits.to_string(),
        _ => return None,
    };
    if !full.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let a = (opacity.unwrap_or(1.0).clamp(0.0, 1.0) * 255.0).round() as u8;
    let (r, g, b) = (&full[0..2], &full[2..4], &full[4..6]);
    Some(format!("{a:02x}{}{}{}", b.to_ascii_lowercase(), g.to_ascii_lowercase(), r.to_ascii_lowercase()))
}

/// Format one GeoJSON position as KML's `lon,lat[,alt]`.
fn kml_position(pos: &Value, precision: u32) -> Result<String, String> {
    let arr = pos.as_array().ok_or("expected a [longitude, latitude] coordinate pair")?;
    let lon = arr.first().and_then(Value::as_f64).ok_or("coordinate is missing a longitude")?;
    let lat = arr.get(1).and_then(Value::as_f64).ok_or("coordinate is missing a latitude")?;
    let alt = arr.get(2).and_then(Value::as_f64);
    let lon = round_to(lon, precision);
    let lat = round_to(lat, precision);
    Ok(match alt {
        Some(a) => format!("{lon},{lat},{}", round_to(a, precision)),
        None => format!("{lon},{lat}"),
    })
}

fn kml_positions(list: &Value, precision: u32) -> Result<String, String> {
    let arr = list.as_array().ok_or("expected an array of coordinates")?;
    let parts: Result<Vec<String>, String> =
        arr.iter().map(|p| kml_position(p, precision)).collect();
    Ok(parts?.join(" "))
}

/// One GeoJSON geometry → its KML element(s). Returns `None` for an empty
/// geometry (no coordinates), which is skipped rather than emitted.
fn geometry_xml(geom: &Value, opt: &Options, indent: &str) -> Result<Option<String>, String> {
    let gtype = geom
        .get("type")
        .and_then(Value::as_str)
        .ok_or("geometry is missing a \"type\"")?;
    let coords = geom.get("coordinates");
    let am = opt.altitude_mode.as_kml();
    let i = indent;
    let i2 = format!("{indent}  ");
    let i3 = format!("{indent}    ");
    let i4 = format!("{indent}      ");

    let multi = |children: Vec<String>| -> Option<String> {
        if children.is_empty() {
            None
        } else {
            Some(format!("{i}<MultiGeometry>\n{}{i}</MultiGeometry>\n", children.concat()))
        }
    };

    match gtype {
        "Point" => {
            let c = match coords {
                Some(c) if c.as_array().is_some_and(|a| !a.is_empty()) => c,
                _ => return Ok(None),
            };
            Ok(Some(format!(
                "{i}<Point>\n{i2}<altitudeMode>{am}</altitudeMode>\n{i2}<coordinates>{}</coordinates>\n{i}</Point>\n",
                kml_position(c, opt.precision)?
            )))
        }
        "LineString" => {
            let c = match coords {
                Some(c) if c.as_array().is_some_and(|a| a.len() >= 2) => c,
                _ => return Ok(None),
            };
            Ok(Some(format!(
                "{i}<LineString>\n{i2}<altitudeMode>{am}</altitudeMode>\n{i2}<coordinates>{}</coordinates>\n{i}</LineString>\n",
                kml_positions(c, opt.precision)?
            )))
        }
        "Polygon" => {
            let rings = match coords.and_then(Value::as_array) {
                Some(r) if !r.is_empty() => r,
                _ => return Ok(None),
            };
            let mut s = format!("{i}<Polygon>\n{i2}<altitudeMode>{am}</altitudeMode>\n");
            for (idx, ring) in rings.iter().enumerate() {
                let tag = if idx == 0 { "outerBoundaryIs" } else { "innerBoundaryIs" };
                s.push_str(&format!(
                    "{i2}<{tag}>\n{i3}<LinearRing>\n{i4}<coordinates>{}</coordinates>\n{i3}</LinearRing>\n{i2}</{tag}>\n",
                    kml_positions(ring, opt.precision)?
                ));
            }
            s.push_str(&format!("{i}</Polygon>\n"));
            Ok(Some(s))
        }
        "MultiPoint" | "MultiLineString" | "MultiPolygon" => {
            let inner_type = match gtype {
                "MultiPoint" => "Point",
                "MultiLineString" => "LineString",
                _ => "Polygon",
            };
            let parts = coords.and_then(Value::as_array).cloned().unwrap_or_default();
            let mut children = Vec::new();
            for part in parts {
                let child = json!({ "type": inner_type, "coordinates": part });
                if let Some(x) = geometry_xml(&child, opt, &i2)? {
                    children.push(x);
                }
            }
            Ok(multi(children))
        }
        "GeometryCollection" => {
            let geoms = geom.get("geometries").and_then(Value::as_array).cloned().unwrap_or_default();
            let mut children = Vec::new();
            for g in &geoms {
                if let Some(x) = geometry_xml(g, opt, &i2)? {
                    children.push(x);
                }
            }
            Ok(multi(children))
        }
        other => Err(format!(
            "unsupported GeoJSON geometry type '{other}' (expected Point, MultiPoint, LineString, \
             MultiLineString, Polygon, MultiPolygon, or GeometryCollection)"
        )),
    }
}

/// An inline `<Style>` built from the feature's simplestyle properties, or
/// `None` when it has none (or styles are switched off).
fn style_xml(props: &Map<String, Value>, opt: &Options, indent: &str) -> Option<String> {
    if !opt.include_styles {
        return None;
    }
    let s = |k: &str| props.get(k).and_then(Value::as_str).map(str::to_string);
    let n = |k: &str| props.get(k).and_then(Value::as_f64);

    let i = indent;
    let i2 = format!("{indent}  ");
    let i3 = format!("{indent}    ");
    let mut body = String::new();

    if let Some(color) = s("stroke").and_then(|c| kml_color(&c, n("stroke-opacity"))) {
        body.push_str(&format!("{i2}<LineStyle>\n{i3}<color>{color}</color>\n"));
        if let Some(w) = n("stroke-width") {
            body.push_str(&format!("{i3}<width>{}</width>\n", round_to(w, 3)));
        }
        body.push_str(&format!("{i2}</LineStyle>\n"));
    }
    if let Some(color) = s("fill").and_then(|c| kml_color(&c, n("fill-opacity"))) {
        body.push_str(&format!("{i2}<PolyStyle>\n{i3}<color>{color}</color>\n{i2}</PolyStyle>\n"));
    }
    if let Some(color) = s("marker-color").and_then(|c| kml_color(&c, Some(1.0))) {
        body.push_str(&format!("{i2}<IconStyle>\n{i3}<color>{color}</color>\n{i2}</IconStyle>\n"));
    }
    if body.is_empty() {
        None
    } else {
        Some(format!("{i}<Style>\n{body}{i}</Style>\n"))
    }
}

/// `<ExtendedData>` for every property that isn't already KML structure.
fn extended_data_xml(props: &Map<String, Value>, indent: &str) -> Option<String> {
    let i = indent;
    let i2 = format!("{indent}  ");
    let i3 = format!("{indent}    ");
    let mut body = String::new();
    for (k, v) in props {
        if RESERVED_PROPS.contains(&k.as_str()) || v.is_null() {
            continue;
        }
        let text = match v {
            Value::String(s) => s.clone(),
            Value::Bool(b) => b.to_string(),
            Value::Number(num) => num.to_string(),
            other => other.to_string(),
        };
        body.push_str(&format!(
            "{i2}<Data name=\"{}\">\n{i3}<value>{}</value>\n{i2}</Data>\n",
            xml_escape(k),
            xml_escape(&text)
        ));
    }
    if body.is_empty() {
        None
    } else {
        Some(format!("{i}<ExtendedData>\n{body}{i}</ExtendedData>\n"))
    }
}

/// One GeoJSON feature → a `<Placemark>` (or `None` when it has no geometry).
fn placemark_xml(feat: &Value, opt: &Options, indent: &str) -> Result<Option<String>, String> {
    let geom = match feat.get("geometry") {
        Some(g) if !g.is_null() => g,
        _ => return Ok(None),
    };
    let empty = Map::new();
    let props = feat.get("properties").and_then(Value::as_object).unwrap_or(&empty);
    let i = indent;
    let i2 = format!("{indent}  ");

    let body = match geometry_xml(geom, opt, &i2)? {
        Some(b) => b,
        None => return Ok(None),
    };

    let mut s = format!("{i}<Placemark>\n");
    if let Some(name) = props.get("name").and_then(Value::as_str) {
        s.push_str(&format!("{i2}<name>{}</name>\n", xml_escape(name)));
    }
    let desc = props
        .get("description")
        .and_then(Value::as_str)
        .or_else(|| props.get("desc").and_then(Value::as_str));
    if let Some(d) = desc {
        s.push_str(&format!("{i2}<description>{}</description>\n", xml_escape(d)));
    }
    if let Some(st) = style_xml(props, opt, &i2) {
        s.push_str(&st);
    }
    if let Some(ed) = extended_data_xml(props, &i2) {
        s.push_str(&ed);
    }
    s.push_str(&body);
    s.push_str(&format!("{i}</Placemark>\n"));
    Ok(Some(s))
}

/// A `<Folder>` tree keyed by path segment, in first-seen order.
#[derive(Default)]
struct FolderNode {
    children: Vec<(String, FolderNode)>,
    placemarks: Vec<Value>,
}

impl FolderNode {
    fn insert(&mut self, segments: &[&str], feat: Value) {
        match segments.split_first() {
            None => self.placemarks.push(feat),
            Some((head, rest)) => {
                if let Some(idx) = self.children.iter().position(|(n, _)| n == head) {
                    self.children[idx].1.insert(rest, feat);
                } else {
                    let mut node = FolderNode::default();
                    node.insert(rest, feat);
                    self.children.push(((*head).to_string(), node));
                }
            }
        }
    }

    fn render(&self, opt: &Options, indent: &str, any: &mut bool) -> Result<String, String> {
        let mut out = String::new();
        for feat in &self.placemarks {
            if let Some(pm) = placemark_xml(feat, opt, indent)? {
                *any = true;
                out.push_str(&pm);
            }
        }
        for (name, node) in &self.children {
            let inner = node.render(opt, &format!("{indent}  "), any)?;
            if inner.is_empty() {
                continue;
            }
            out.push_str(&format!(
                "{indent}<Folder>\n{indent}  <name>{}</name>\n{inner}{indent}</Folder>\n",
                xml_escape(name)
            ));
        }
        Ok(out)
    }
}

/// GeoJSON text → a KML document.
fn geojson_to_kml(json_text: &str, opt: &Options) -> Result<String, String> {
    let val: Value = serde_json::from_str(json_text).map_err(|e| format!("invalid GeoJSON: {e}"))?;

    let features: Vec<Value> = match val.get("type").and_then(Value::as_str) {
        Some("FeatureCollection") => val
            .get("features")
            .and_then(Value::as_array)
            .cloned()
            .ok_or("FeatureCollection is missing a \"features\" array")?,
        Some("Feature") => vec![val.clone()],
        Some(_) => vec![json!({ "type": "Feature", "geometry": val.clone(), "properties": {} })],
        None => {
            return Err(
                "input does not look like GeoJSON (missing a top-level \"type\")".to_string()
            )
        }
    };
    if features.is_empty() {
        return Err("no features found in the GeoJSON input".to_string());
    }

    let mut root = FolderNode::default();
    for feat in features {
        let path = if opt.include_folders {
            feat.get("properties")
                .and_then(|p| p.get("folder"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string()
        } else {
            String::new()
        };
        let segments: Vec<&str> = path.split('/').map(str::trim).filter(|s| !s.is_empty()).collect();
        root.insert(&segments, feat);
    }

    let mut any = false;
    let body = root.render(opt, "    ", &mut any)?;
    if !any {
        return Err(
            "no Point/LineString/Polygon geometry was found to convert to KML — every feature was \
             empty or had a null geometry"
                .to_string(),
        );
    }

    let doc_name = if opt.document_name.trim().is_empty() {
        "GeoJSON Export"
    } else {
        opt.document_name.trim()
    };
    Ok(format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <kml xmlns=\"http://www.opengis.net/kml/2.2\">\n\
         \x20 <Document>\n\
         \x20   <name>{}</name>\n{body}\x20 </Document>\n</kml>\n",
        xml_escape(doc_name)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geo(input: &str, opt: Options) -> Result<Value, String> {
        let out = convert(input, &opt)?;
        serde_json::from_str(&out).map_err(|e| e.to_string())
    }

    fn kmz_base64(entries: &[(&str, &str)]) -> String {
        use zip::write::SimpleFileOptions;
        let mut buf: Vec<u8> = Vec::new();
        {
            let mut w = zip::ZipWriter::new(Cursor::new(&mut buf));
            let opts = SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            for (name, body) in entries {
                w.start_file(*name, opts).unwrap();
                use std::io::Write;
                w.write_all(body.as_bytes()).unwrap();
            }
            w.finish().unwrap();
        }
        base64::engine::general_purpose::STANDARD.encode(&buf)
    }

    const POINT_KML: &str = r#"<?xml version="1.0"?>
<kml xmlns="http://www.opengis.net/kml/2.2"><Document><Placemark>
<name>Trailhead</name><description>Parking lot</description>
<Point><coordinates>-122.0841234567,37.4212345678,15</coordinates></Point>
</Placemark></Document></kml>"#;

    // --- KML → GeoJSON ------------------------------------------------------

    #[test]
    fn kml_placemark_becomes_point_feature() {
        let fc = geo(POINT_KML, Options::default()).unwrap();
        assert_eq!(fc["type"], "FeatureCollection");
        let f = &fc["features"][0];
        assert_eq!(f["geometry"], json!({ "type": "Point", "coordinates": [-122.084123, 37.421235, 15.0] }));
        assert_eq!(f["properties"]["name"], "Trailhead");
        assert_eq!(f["properties"]["description"], "Parking lot");
    }

    #[test]
    fn precision_zero_rounds_to_whole_degrees() {
        let opt = Options { precision: 0, ..Options::default() };
        let fc = geo(POINT_KML, opt).unwrap();
        assert_eq!(fc["features"][0]["geometry"]["coordinates"], json!([-122.0, 37.0, 15.0]));
    }

    #[test]
    fn folder_path_becomes_a_property() {
        let kml = r#"<kml><Document><name>Places</name>
<Folder><name>Trails</name>
  <Folder><name>Day 1</name>
    <Placemark><name>Start</name><Point><coordinates>1,2</coordinates></Point></Placemark>
  </Folder>
  <Placemark><name>Overview</name><Point><coordinates>3,4</coordinates></Point></Placemark>
</Folder>
<Placemark><name>Loose</name><Point><coordinates>5,6</coordinates></Point></Placemark>
</Document></kml>"#;
        let fc = geo(kml, Options::default()).unwrap();
        let feats = fc["features"].as_array().unwrap();
        assert_eq!(feats.len(), 3);
        assert_eq!(feats[0]["properties"]["folder"], "Trails/Day 1");
        assert_eq!(feats[1]["properties"]["folder"], "Trails");
        assert_eq!(feats[2]["properties"].get("folder"), None);
    }

    #[test]
    fn folders_can_be_switched_off() {
        let kml = r#"<kml><Folder><name>Trails</name>
<Placemark><Point><coordinates>1,2</coordinates></Point></Placemark></Folder></kml>"#;
        let opt = Options { include_folders: false, ..Options::default() };
        let fc = geo(kml, opt).unwrap();
        assert_eq!(fc["features"][0]["properties"].get("folder"), None);
    }

    #[test]
    fn kml_styles_land_as_simplestyle_properties() {
        let kml = r#"<kml><Document>
<Style id="road"><LineStyle><color>ff0000ff</color><width>4</width></LineStyle></Style>
<Placemark><styleUrl>#road</styleUrl>
<LineString><coordinates>1,2 3,4</coordinates></LineString></Placemark>
</Document></kml>"#;
        let fc = geo(kml, Options::default()).unwrap();
        let p = &fc["features"][0]["properties"];
        assert_eq!(p["stroke"], "#ff0000");
        assert_eq!(p["stroke-width"], 4.0);

        let plain = geo(kml, Options { include_styles: false, ..Options::default() }).unwrap();
        assert_eq!(plain["features"][0]["properties"].get("stroke"), None);
    }

    #[test]
    fn kmz_base64_input_is_unzipped_and_converted() {
        let b64 = kmz_base64(&[("doc.kml", POINT_KML)]);
        let fc = geo(&b64, Options::default()).unwrap();
        assert_eq!(fc["features"][0]["properties"]["name"], "Trailhead");
    }

    #[test]
    fn kmz_falls_back_to_the_first_kml_entry() {
        let b64 = kmz_base64(&[("files/icon.txt", "not kml"), ("layers/route.kml", POINT_KML)]);
        let fc = geo(&b64, Options::default()).unwrap();
        assert_eq!(fc["features"][0]["properties"]["name"], "Trailhead");
    }

    #[test]
    fn kmz_without_a_kml_entry_errors() {
        let b64 = kmz_base64(&[("readme.txt", "nothing here")]);
        let err = geo(&b64, Options::default()).unwrap_err();
        assert!(err.contains("no .kml entry"), "unexpected error: {err}");
        assert!(err.contains("readme.txt"), "error should list what it found: {err}");
    }

    #[test]
    fn geojson_input_with_geojson_output_explains_the_fix() {
        let err = geo(r#"{"type":"FeatureCollection","features":[]}"#, Options::default()).unwrap_err();
        assert!(err.contains("output_format=\"kml\""), "unexpected error: {err}");
    }

    #[test]
    fn empty_input_errors() {
        let err = convert("   \n ", &Options::default()).unwrap_err();
        assert!(err.contains("input is empty"), "unexpected error: {err}");
    }

    #[test]
    fn non_kml_xml_errors() {
        let err = convert("<gpx version=\"1.1\"><wpt lat=\"1\" lon=\"2\"/></gpx>", &Options::default())
            .unwrap_err();
        assert!(err.contains("<kml> root element"), "unexpected error: {err}");
    }

    #[test]
    fn oversized_input_errors_with_the_limit() {
        let big = "<kml>".to_string() + &" ".repeat(MAX_INPUT_BYTES) + "</kml>";
        let err = convert(&big, &Options::default()).unwrap_err();
        assert!(err.contains("the limit is"), "unexpected error: {err}");
    }

    // --- GeoJSON → KML ------------------------------------------------------

    fn to_kml(input: &str, opt: Options) -> Result<String, String> {
        convert(input, &Options { output_format: OutputFormat::Kml, ..opt })
    }

    #[test]
    fn geojson_point_becomes_a_placemark() {
        let gj = r#"{"type":"Feature","properties":{"name":"Camp","description":"Tent"},
                     "geometry":{"type":"Point","coordinates":[5.1,52.1,10]}}"#;
        let kml = to_kml(gj, Options::default()).unwrap();
        assert!(kml.contains("<name>Camp</name>"), "{kml}");
        assert!(kml.contains("<description>Tent</description>"), "{kml}");
        assert!(kml.contains("<coordinates>5.1,52.1,10</coordinates>"), "{kml}");
        assert!(kml.contains("<altitudeMode>clampToGround</altitudeMode>"), "{kml}");
    }

    #[test]
    fn altitude_mode_is_configurable() {
        let gj = r#"{"type":"Point","coordinates":[1,2,3]}"#;
        let kml = to_kml(gj, Options { altitude_mode: AltitudeMode::Absolute, ..Options::default() })
            .unwrap();
        assert!(kml.contains("<altitudeMode>absolute</altitudeMode>"), "{kml}");
    }

    #[test]
    fn simplestyle_properties_become_a_kml_style() {
        let gj = r##"{"type":"Feature",
          "properties":{"stroke":"#ff0000","stroke-width":4,"stroke-opacity":0.5,
                        "fill":"#00ff00","fill-opacity":1,"marker-color":"#0000ff"},
          "geometry":{"type":"LineString","coordinates":[[1,2],[3,4]]}}"##;
        let kml = to_kml(gj, Options::default()).unwrap();
        // KML colors are aabbggrr: 50% alpha red -> 80 00 00 ff.
        assert!(kml.contains("<color>800000ff</color>"), "{kml}");
        assert!(kml.contains("<width>4</width>"), "{kml}");
        assert!(kml.contains("<color>ff00ff00</color>"), "{kml}");
        assert!(kml.contains("<color>ffff0000</color>"), "{kml}");

        let plain = to_kml(gj, Options { include_styles: false, ..Options::default() }).unwrap();
        assert!(!plain.contains("<Style>"), "{plain}");
    }

    #[test]
    fn polygon_holes_become_inner_boundaries() {
        let gj = r#"{"type":"Polygon","coordinates":[
            [[0,0],[4,0],[4,4],[0,4],[0,0]],
            [[1,1],[2,1],[2,2],[1,2],[1,1]]]}"#;
        let kml = to_kml(gj, Options::default()).unwrap();
        assert!(kml.contains("<outerBoundaryIs>"), "{kml}");
        assert!(kml.contains("<innerBoundaryIs>"), "{kml}");
        assert!(kml.contains("<coordinates>1,1 2,1 2,2 1,2 1,1</coordinates>"), "{kml}");
    }

    #[test]
    fn multi_geometries_become_multigeometry() {
        let gj = r#"{"type":"MultiLineString","coordinates":[[[1,1],[2,2]],[[3,3],[4,4]]]}"#;
        let kml = to_kml(gj, Options::default()).unwrap();
        assert!(kml.contains("<MultiGeometry>"), "{kml}");
        assert_eq!(kml.matches("<LineString>").count(), 2, "{kml}");
    }

    #[test]
    fn folder_property_rebuilds_the_folder_tree() {
        let gj = r#"{"type":"FeatureCollection","features":[
          {"type":"Feature","properties":{"name":"Start","folder":"Trails/Day 1"},
           "geometry":{"type":"Point","coordinates":[1,2]}},
          {"type":"Feature","properties":{"name":"Loose"},
           "geometry":{"type":"Point","coordinates":[5,6]}}]}"#;
        let kml = to_kml(gj, Options::default()).unwrap();
        assert!(kml.contains("<name>Trails</name>"), "{kml}");
        assert!(kml.contains("<name>Day 1</name>"), "{kml}");
        assert_eq!(kml.matches("<Folder>").count(), 2, "{kml}");

        let flat = to_kml(gj, Options { include_folders: false, ..Options::default() }).unwrap();
        assert!(!flat.contains("<Folder>"), "{flat}");
    }

    #[test]
    fn unknown_properties_become_extended_data() {
        let gj = r#"{"type":"Feature","properties":{"name":"P","surface":"gravel","km":12},
                     "geometry":{"type":"Point","coordinates":[1,2]}}"#;
        let kml = to_kml(gj, Options::default()).unwrap();
        assert!(kml.contains("<Data name=\"surface\">"), "{kml}");
        assert!(kml.contains("<value>gravel</value>"), "{kml}");
        assert!(kml.contains("<value>12</value>"), "{kml}");
    }

    #[test]
    fn document_name_is_used_and_escaped() {
        let gj = r#"{"type":"Point","coordinates":[1,2]}"#;
        let kml =
            to_kml(gj, Options { document_name: "Trip <2026>".into(), ..Options::default() }).unwrap();
        assert!(kml.contains("<name>Trip &lt;2026&gt;</name>"), "{kml}");
    }

    #[test]
    fn kml_input_with_kml_output_explains_the_fix() {
        let err = to_kml(POINT_KML, Options::default()).unwrap_err();
        assert!(err.contains("output_format=\"geojson\""), "unexpected error: {err}");
    }

    #[test]
    fn unsupported_geometry_type_errors() {
        let gj = r#"{"type":"Feature","properties":{},"geometry":{"type":"Circle","coordinates":[1,2]}}"#;
        let err = to_kml(gj, Options::default()).unwrap_err();
        assert!(err.contains("unsupported GeoJSON geometry type 'Circle'"), "unexpected error: {err}");
    }

    #[test]
    fn geojson_without_a_type_errors() {
        let err = to_kml(r#"{"features":[]}"#, Options::default()).unwrap_err();
        assert!(err.contains("missing a top-level"), "unexpected error: {err}");
    }

    #[test]
    fn round_trip_keeps_names_and_positions() {
        let fc = geo(POINT_KML, Options::default()).unwrap();
        let kml = to_kml(&fc.to_string(), Options::default()).unwrap();
        assert!(kml.contains("<name>Trailhead</name>"), "{kml}");
        assert!(kml.contains("<coordinates>-122.084123,37.421235,15</coordinates>"), "{kml}");
    }

    #[test]
    fn bad_output_format_is_rejected() {
        assert!(OutputFormat::parse("shapefile").is_err());
        assert!(AltitudeMode::parse("underground").is_err());
    }
}
