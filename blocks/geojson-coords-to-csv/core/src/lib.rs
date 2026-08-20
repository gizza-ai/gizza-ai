//! gizza-ai/geojson-coords-to-csv core — explode every coordinate (vertex) in a
//! GeoJSON document into a flat CSV, ONE ROW PER COORDINATE PAIR.
//!
//! This is the coordinate-level sibling of `geojson-to-csv` (which emits one row
//! per FEATURE with the geometry as WKT or the first coordinate). Here a polygon
//! with 500 vertices becomes 500 rows of `longitude,latitude[,elevation]`, in the
//! order the positions appear in the document.
//!
//! Pure Rust (`serde_json`), no wafer/wasm-bindgen deps — shared by the chat
//! block, the CLI and the page.

use serde_json::{Map, Number, Value};
use std::collections::HashSet;

/// Largest accepted input (characters). Bigger documents should be split or run
/// through a desktop GIS tool.
pub const MAX_INPUT_CHARS: usize = 16 * 1024 * 1024;
/// Largest number of coordinate rows emitted.
pub const MAX_COORDS: usize = 200_000;
/// Largest number of property columns appended when `properties` is on.
pub const MAX_PROPERTY_COLUMNS: usize = 200;

/// Axis order of the two coordinate columns.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Order {
    /// `longitude,latitude` — GeoJSON's own storage order (default).
    LonLat,
    /// `latitude,longitude` — the order most mapping UIs display.
    LatLon,
}

impl Order {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "lonlat" | "lon_lat" | "lon,lat" | "xy" => Ok(Order::LonLat),
            "latlon" | "lat_lon" | "lat,lon" | "latlng" | "yx" => Ok(Order::LatLon),
            other => Err(format!(
                "invalid order '{other}': expected one of lonlat, latlon"
            )),
        }
    }
}

/// Which context columns are written before the coordinate columns.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Columns {
    /// Just the coordinate columns (default).
    Coords,
    /// A 1-based `index` column, then the coordinates.
    Indexed,
    /// `index`, `feature_index`, `feature_id`, `geometry_type`, then the coordinates.
    Feature,
    /// Everything: also `part`, `ring`, `position` and `ring_close`.
    Full,
}

impl Columns {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "coords" | "coordinates" | "minimal" => Ok(Columns::Coords),
            "indexed" | "index" => Ok(Columns::Indexed),
            "feature" | "features" => Ok(Columns::Feature),
            "full" | "all" => Ok(Columns::Full),
            other => Err(format!(
                "invalid columns '{other}': expected one of coords, indexed, feature, full"
            )),
        }
    }
    fn wants_index(self) -> bool {
        self != Columns::Coords
    }
    fn wants_feature(self) -> bool {
        matches!(self, Columns::Feature | Columns::Full)
    }
    fn wants_position(self) -> bool {
        self == Columns::Full
    }
}

/// Which geometry kinds contribute coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Shapes {
    All,
    Points,
    Lines,
    Polygons,
}

impl Shapes {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "all" => Ok(Shapes::All),
            "points" | "point" => Ok(Shapes::Points),
            "lines" | "line" | "linestrings" => Ok(Shapes::Lines),
            "polygons" | "polygon" => Ok(Shapes::Polygons),
            other => Err(format!(
                "invalid shapes '{other}': expected one of all, points, lines, polygons"
            )),
        }
    }
    fn accepts(self, geom_type: &str) -> bool {
        match self {
            Shapes::All => true,
            Shapes::Points => matches!(geom_type, "Point" | "MultiPoint"),
            Shapes::Lines => matches!(geom_type, "LineString" | "MultiLineString"),
            Shapes::Polygons => matches!(geom_type, "Polygon" | "MultiPolygon"),
        }
    }
}

/// When the third (elevation / Z) column is written.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Elevation {
    /// Include it only if at least one emitted coordinate carries a Z value (default).
    Auto,
    /// Always write the column, blank where a coordinate is 2-D.
    Always,
    /// Never write it.
    Never,
}

impl Elevation {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Ok(Elevation::Auto),
            "always" | "on" | "true" => Ok(Elevation::Always),
            "never" | "off" | "false" | "drop" => Ok(Elevation::Never),
            other => Err(format!(
                "invalid elevation '{other}': expected one of auto, always, never"
            )),
        }
    }
}

/// How repeated coordinates are dropped. The levels are cumulative.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Dedupe {
    /// Emit every position exactly as stored (default).
    None,
    /// Drop each polygon ring's closing position when it repeats the ring's first.
    RingClose,
    /// Ring-close, plus any position identical to the one emitted just before it
    /// in the same ring/part.
    Adjacent,
    /// Emit each distinct coordinate once across the whole document.
    All,
}

impl Dedupe {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "none" | "off" => Ok(Dedupe::None),
            "ring-close" | "ring_close" | "ringclose" | "close" => Ok(Dedupe::RingClose),
            "adjacent" | "consecutive" => Ok(Dedupe::Adjacent),
            "all" | "global" | "unique" => Ok(Dedupe::All),
            other => Err(format!(
                "invalid dedupe '{other}': expected one of none, ring-close, adjacent, all"
            )),
        }
    }
    fn drops_ring_close(self) -> bool {
        self != Dedupe::None
    }
    fn drops_adjacent(self) -> bool {
        matches!(self, Dedupe::Adjacent | Dedupe::All)
    }
    fn drops_global(self) -> bool {
        self == Dedupe::All
    }
}

/// Output field separator.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Delimiter {
    Comma,
    Semicolon,
    Tab,
    Pipe,
}

impl Delimiter {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "comma" | "," => Ok(Delimiter::Comma),
            "semicolon" | ";" => Ok(Delimiter::Semicolon),
            "tab" | "\t" | "\\t" => Ok(Delimiter::Tab),
            "pipe" | "|" => Ok(Delimiter::Pipe),
            other => Err(format!(
                "invalid delimiter '{other}': expected one of comma, semicolon, tab, pipe"
            )),
        }
    }
    fn ch(self) -> char {
        match self {
            Delimiter::Comma => ',',
            Delimiter::Semicolon => ';',
            Delimiter::Tab => '\t',
            Delimiter::Pipe => '|',
        }
    }
}

/// Decimal places for the coordinate columns: `None` keeps the number exactly as
/// written in the document.
fn parse_precision(precision: i32) -> Result<Option<usize>, String> {
    match precision {
        -1 => Ok(None),
        0..=15 => Ok(Some(precision as usize)),
        other => Err(format!(
            "invalid precision '{other}': expected -1 (keep the source value) or 0-15 decimal places"
        )),
    }
}

fn format_number(n: &Number, precision: Option<usize>) -> String {
    match precision {
        None => n.to_string(),
        Some(p) => match n.as_f64() {
            Some(v) => format!("{v:.p$}"),
            None => n.to_string(),
        },
    }
}

struct FeatureInfo {
    id: String,
    properties: Option<Map<String, Value>>,
}

struct Row {
    feature: usize,
    geometry_type: &'static str,
    part: usize,
    ring: Option<usize>,
    position: usize,
    ring_close: bool,
    lon: String,
    lat: String,
    elev: Option<String>,
}

struct Walker {
    precision: Option<usize>,
    shapes: Shapes,
    dedupe: Dedupe,
    rows: Vec<Row>,
    features: Vec<FeatureInfo>,
    seen: HashSet<(String, String, String)>,
    any_elevation: bool,
}

impl Walker {
    /// Turn one GeoJSON position (`[lon, lat]` or `[lon, lat, z]`) into formatted
    /// column strings. `where_` is used only to make errors locatable.
    fn position(&self, v: &Value, where_: &str) -> Result<(String, String, Option<String>), String> {
        let arr = v.as_array().ok_or_else(|| {
            format!("{where_}: expected a coordinate array like [longitude, latitude], found {}", kind_of(v))
        })?;
        if arr.len() < 2 {
            return Err(format!(
                "{where_}: a coordinate needs at least 2 numbers ([longitude, latitude]), found {}",
                arr.len()
            ));
        }
        let num = |i: usize| -> Result<&Number, String> {
            match &arr[i] {
                Value::Number(n) => Ok(n),
                other => Err(format!(
                    "{where_}: coordinate value {} must be a number, found {}",
                    i + 1,
                    kind_of(other)
                )),
            }
        };
        let lon = format_number(num(0)?, self.precision);
        let lat = format_number(num(1)?, self.precision);
        let elev = match arr.get(2) {
            Some(Value::Null) | None => None,
            Some(Value::Number(n)) => Some(format_number(n, self.precision)),
            Some(other) => {
                return Err(format!(
                    "{where_}: elevation must be a number, found {}",
                    kind_of(other)
                ))
            }
        };
        Ok((lon, lat, elev))
    }

    #[allow(clippy::too_many_arguments)]
    fn push(
        &mut self,
        feature: usize,
        geometry_type: &'static str,
        part: usize,
        ring: Option<usize>,
        position: usize,
        ring_close: bool,
        coord: (String, String, Option<String>),
    ) -> Result<(), String> {
        if self.dedupe.drops_global() {
            let key = (
                coord.0.clone(),
                coord.1.clone(),
                coord.2.clone().unwrap_or_default(),
            );
            if !self.seen.insert(key) {
                return Ok(());
            }
        }
        if self.rows.len() >= MAX_COORDS {
            return Err(format!(
                "too many coordinates: this document has more than the {MAX_COORDS} rows this tool \
                 emits. Filter it down (see the shapes option) or split the file."
            ));
        }
        if coord.2.is_some() {
            self.any_elevation = true;
        }
        self.rows.push(Row {
            feature,
            geometry_type,
            part,
            ring,
            position,
            ring_close,
            lon: coord.0,
            lat: coord.1,
            elev: coord.2,
        });
        Ok(())
    }

    /// Emit one linear sequence of positions (a LineString, a polygon ring, or a
    /// single point wrapped in a one-element slice).
    #[allow(clippy::too_many_arguments)]
    fn line(
        &mut self,
        feature: usize,
        geometry_type: &'static str,
        part: usize,
        ring: Option<usize>,
        positions: &[Value],
        where_: &str,
    ) -> Result<(), String> {
        let mut coords = Vec::with_capacity(positions.len());
        for (i, p) in positions.iter().enumerate() {
            coords.push(self.position(p, &format!("{where_} position {}", i + 1))?);
        }
        // Ring-close: a closed ring repeats its first position at the end.
        let closes = ring.is_some() && coords.len() >= 2 && coords[0] == coords[coords.len() - 1];
        let closing_index = coords.len().saturating_sub(1);
        let mut len = coords.len();
        if closes && self.dedupe.drops_ring_close() {
            len -= 1;
        }
        let mut previous: Option<(String, String, Option<String>)> = None;
        for (i, coord) in coords.into_iter().take(len).enumerate() {
            if self.dedupe.drops_adjacent() && previous.as_ref() == Some(&coord) {
                continue;
            }
            previous = Some(coord.clone());
            let ring_close = closes && i == closing_index;
            self.push(feature, geometry_type, part, ring, i + 1, ring_close, coord)?;
        }
        Ok(())
    }

    fn geometry(
        &mut self,
        feature: usize,
        geometry: &Value,
        part: &mut usize,
        depth: usize,
        where_: &str,
    ) -> Result<(), String> {
        if depth > 8 {
            return Err(format!(
                "{where_}: GeometryCollection nesting is deeper than 8 levels"
            ));
        }
        let obj = geometry.as_object().ok_or_else(|| {
            format!("{where_}: expected a geometry object, found {}", kind_of(geometry))
        })?;
        let ty = obj
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("{where_}: geometry is missing a string \"type\""))?;

        if ty == "GeometryCollection" {
            let members = obj
                .get("geometries")
                .and_then(Value::as_array)
                .ok_or_else(|| format!("{where_}: GeometryCollection needs a \"geometries\" array"))?;
            for (i, member) in members.iter().enumerate() {
                let inner = format!("{where_} geometry {}", i + 1);
                self.geometry(feature, member, part, depth + 1, &inner)?;
            }
            return Ok(());
        }

        let known: &'static str = match ty {
            "Point" => "Point",
            "MultiPoint" => "MultiPoint",
            "LineString" => "LineString",
            "MultiLineString" => "MultiLineString",
            "Polygon" => "Polygon",
            "MultiPolygon" => "MultiPolygon",
            other => {
                return Err(format!(
                    "{where_}: unsupported geometry type '{other}'. Expected Point, MultiPoint, \
                     LineString, MultiLineString, Polygon, MultiPolygon or GeometryCollection."
                ))
            }
        };
        if !self.shapes.accepts(known) {
            return Ok(());
        }
        let coordinates = obj
            .get("coordinates")
            .ok_or_else(|| format!("{where_}: {known} is missing \"coordinates\""))?;
        if coordinates.is_null() {
            return Ok(());
        }
        let arr = coordinates.as_array().ok_or_else(|| {
            format!(
                "{where_}: {known} \"coordinates\" must be an array, found {}",
                kind_of(coordinates)
            )
        })?;

        match known {
            "Point" => {
                *part += 1;
                let coord = self.position(coordinates, where_)?;
                self.push(feature, known, *part, None, 1, false, coord)?;
            }
            "MultiPoint" => {
                for (i, p) in arr.iter().enumerate() {
                    *part += 1;
                    let coord = self.position(p, &format!("{where_} point {}", i + 1))?;
                    self.push(feature, known, *part, None, 1, false, coord)?;
                }
            }
            "LineString" => {
                *part += 1;
                let p = *part;
                self.line(feature, known, p, None, arr, where_)?;
            }
            "MultiLineString" => {
                for (i, line) in arr.iter().enumerate() {
                    let positions = line.as_array().ok_or_else(|| {
                        format!("{where_}: MultiLineString member {} must be an array", i + 1)
                    })?;
                    *part += 1;
                    let p = *part;
                    let inner = format!("{where_} line {}", i + 1);
                    self.line(feature, known, p, None, positions, &inner)?;
                }
            }
            "Polygon" => {
                *part += 1;
                let p = *part;
                self.rings(feature, known, p, arr, where_)?;
            }
            "MultiPolygon" => {
                for (i, poly) in arr.iter().enumerate() {
                    let rings = poly.as_array().ok_or_else(|| {
                        format!("{where_}: MultiPolygon member {} must be an array", i + 1)
                    })?;
                    *part += 1;
                    let p = *part;
                    let inner = format!("{where_} polygon {}", i + 1);
                    self.rings(feature, known, p, rings, &inner)?;
                }
            }
            _ => unreachable!(),
        }
        Ok(())
    }

    fn rings(
        &mut self,
        feature: usize,
        geometry_type: &'static str,
        part: usize,
        rings: &[Value],
        where_: &str,
    ) -> Result<(), String> {
        for (i, ring) in rings.iter().enumerate() {
            let positions = ring
                .as_array()
                .ok_or_else(|| format!("{where_}: ring {} must be an array", i + 1))?;
            let inner = format!("{where_} ring {}", i + 1);
            self.line(feature, geometry_type, part, Some(i + 1), positions, &inner)?;
        }
        Ok(())
    }
}

fn kind_of(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

fn feature_id(v: &Value) -> String {
    match v.get("id") {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

fn property_text(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

fn csv_field(s: &str, delimiter: char) -> String {
    if s.contains(delimiter) || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Explode every coordinate in `geojson` into a flat CSV, one row per position.
///
/// String options are the lenient `parse` forms of the enums above; `precision`
/// is `-1` (keep the source value) or `0..=15` decimal places.
#[allow(clippy::too_many_arguments)]
pub fn convert_str(
    geojson: &str,
    order: &str,
    columns: &str,
    shapes: &str,
    elevation: &str,
    precision: i32,
    dedupe: &str,
    properties: bool,
    delimiter: &str,
    header: bool,
) -> Result<String, String> {
    let order = Order::parse(order)?;
    let columns = Columns::parse(columns)?;
    let shapes = Shapes::parse(shapes)?;
    let elevation = Elevation::parse(elevation)?;
    let precision = parse_precision(precision)?;
    let dedupe = Dedupe::parse(dedupe)?;
    let delimiter = Delimiter::parse(delimiter)?;
    let sep = delimiter.ch();

    let trimmed = geojson.trim();
    if trimmed.is_empty() {
        return Err("no GeoJSON provided: paste a FeatureCollection, Feature or geometry".into());
    }
    if trimmed.len() > MAX_INPUT_CHARS {
        return Err(format!(
            "input is too large ({} characters): this tool accepts up to {MAX_INPUT_CHARS} characters",
            trimmed.len()
        ));
    }
    let doc: Value = serde_json::from_str(trimmed)
        .map_err(|e| format!("invalid JSON: {e}. Expected a GeoJSON document."))?;

    let mut walker = Walker {
        precision,
        shapes,
        dedupe,
        rows: Vec::new(),
        features: Vec::new(),
        seen: HashSet::new(),
        any_elevation: false,
    };

    // Accept a FeatureCollection, a single Feature, a bare geometry, or a
    // top-level array of any of those.
    let entries: Vec<&Value> = match &doc {
        Value::Array(items) => items.iter().collect(),
        Value::Object(obj) => match obj.get("type").and_then(Value::as_str) {
            Some("FeatureCollection") => {
                let features = obj
                    .get("features")
                    .and_then(Value::as_array)
                    .ok_or_else(|| "FeatureCollection needs a \"features\" array".to_string())?;
                features.iter().collect()
            }
            Some(_) => vec![&doc],
            None => {
                return Err(
                    "not GeoJSON: the top-level object has no \"type\" (expected FeatureCollection, \
                     Feature or a geometry type)"
                        .into(),
                )
            }
        },
        other => {
            return Err(format!(
                "not GeoJSON: expected an object or an array, found {}",
                kind_of(other)
            ))
        }
    };

    let null = Value::Null;
    for (i, entry) in entries.iter().enumerate() {
        let where_ = format!("feature {}", i + 1);
        let obj = entry
            .as_object()
            .ok_or_else(|| format!("{where_}: expected an object, found {}", kind_of(entry)))?;
        let ty = obj
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("{where_}: missing a string \"type\""))?;

        let (geometry, info) = if ty == "Feature" {
            let props = match obj.get("properties") {
                Some(Value::Object(m)) => Some(m.clone()),
                _ => None,
            };
            (
                obj.get("geometry").unwrap_or(&null),
                FeatureInfo {
                    id: feature_id(entry),
                    properties: props,
                },
            )
        } else {
            (
                *entry,
                FeatureInfo {
                    id: feature_id(entry),
                    properties: None,
                },
            )
        };

        walker.features.push(info);
        let index = walker.features.len() - 1;
        if geometry.is_null() {
            continue;
        }
        let mut part = 0usize;
        walker.geometry(index, geometry, &mut part, 0, &where_)?;
    }

    let with_elevation = match elevation {
        Elevation::Auto => walker.any_elevation,
        Elevation::Always => true,
        Elevation::Never => false,
    };

    // Property columns: the union of keys, in first-seen order, over the features
    // that actually produced rows.
    let mut property_keys: Vec<String> = Vec::new();
    if properties {
        let mut seen_features: HashSet<usize> = HashSet::new();
        for row in &walker.rows {
            if !seen_features.insert(row.feature) {
                continue;
            }
            if let Some(map) = &walker.features[row.feature].properties {
                for key in map.keys() {
                    if !property_keys.iter().any(|k| k == key) {
                        property_keys.push(key.clone());
                        if property_keys.len() > MAX_PROPERTY_COLUMNS {
                            return Err(format!(
                                "too many property columns: this tool appends up to \
                                 {MAX_PROPERTY_COLUMNS}. Turn the properties option off."
                            ));
                        }
                    }
                }
            }
        }
    }

    let mut out = String::new();
    if header {
        let mut names: Vec<String> = Vec::new();
        if columns.wants_index() {
            names.push("index".into());
        }
        if columns.wants_feature() {
            names.push("feature_index".into());
            names.push("feature_id".into());
            names.push("geometry_type".into());
        }
        if columns.wants_position() {
            names.push("part".into());
            names.push("ring".into());
            names.push("position".into());
            names.push("ring_close".into());
        }
        match order {
            Order::LonLat => {
                names.push("longitude".into());
                names.push("latitude".into());
            }
            Order::LatLon => {
                names.push("latitude".into());
                names.push("longitude".into());
            }
        }
        if with_elevation {
            names.push("elevation".into());
        }
        names.extend(property_keys.iter().cloned());
        out.push_str(
            &names
                .iter()
                .map(|n| csv_field(n, sep))
                .collect::<Vec<_>>()
                .join(&sep.to_string()),
        );
        out.push('\n');
    }

    for (i, row) in walker.rows.iter().enumerate() {
        let mut fields: Vec<String> = Vec::new();
        if columns.wants_index() {
            fields.push((i + 1).to_string());
        }
        if columns.wants_feature() {
            fields.push((row.feature + 1).to_string());
            fields.push(walker.features[row.feature].id.clone());
            fields.push(row.geometry_type.to_string());
        }
        if columns.wants_position() {
            fields.push(row.part.to_string());
            fields.push(row.ring.map(|r| r.to_string()).unwrap_or_default());
            fields.push(row.position.to_string());
            fields.push(row.ring_close.to_string());
        }
        match order {
            Order::LonLat => {
                fields.push(row.lon.clone());
                fields.push(row.lat.clone());
            }
            Order::LatLon => {
                fields.push(row.lat.clone());
                fields.push(row.lon.clone());
            }
        }
        if with_elevation {
            fields.push(row.elev.clone().unwrap_or_default());
        }
        for key in &property_keys {
            let text = walker.features[row.feature]
                .properties
                .as_ref()
                .and_then(|m| m.get(key))
                .map(property_text)
                .unwrap_or_default();
            fields.push(text);
        }
        out.push_str(
            &fields
                .iter()
                .map(|f| csv_field(f, sep))
                .collect::<Vec<_>>()
                .join(&sep.to_string()),
        );
        out.push('\n');
    }

    if out.ends_with('\n') {
        out.pop();
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FC: &str = r#"{
      "type": "FeatureCollection",
      "features": [
        { "type": "Feature", "id": "a", "properties": { "name": "Trail" },
          "geometry": { "type": "LineString", "coordinates": [[-105.1,40.1],[-105.2,40.2]] } },
        { "type": "Feature", "properties": { "name": "Site", "tags": ["x"] },
          "geometry": { "type": "Point", "coordinates": [10.5, 20.25, 1500] } }
      ]
    }"#;

    fn plain(geojson: &str) -> Result<String, String> {
        convert_str(
            geojson, "lonlat", "coords", "all", "auto", -1, "none", false, "comma", true,
        )
    }

    #[test]
    fn happy_path_flattens_every_position() {
        let csv = plain(FC).unwrap();
        assert_eq!(
            csv,
            "longitude,latitude,elevation\n-105.1,40.1,\n-105.2,40.2,\n10.5,20.25,1500"
        );
    }

    #[test]
    fn invalid_json_is_an_error() {
        let err = plain("{not json").unwrap_err();
        assert!(err.starts_with("invalid JSON:"), "{err}");
    }

    #[test]
    fn empty_input_is_an_error() {
        assert!(plain("   ").unwrap_err().contains("no GeoJSON provided"));
    }

    #[test]
    fn top_level_object_without_type_is_an_error() {
        let err = plain(r#"{"features":[]}"#).unwrap_err();
        assert!(err.contains("no \"type\""), "{err}");
    }

    #[test]
    fn unsupported_geometry_type_names_the_alternatives() {
        let err = plain(r#"{"type":"Circle","coordinates":[0,0]}"#).unwrap_err();
        assert!(err.contains("unsupported geometry type 'Circle'"), "{err}");
        assert!(err.contains("MultiPolygon"), "{err}");
    }

    #[test]
    fn short_coordinate_is_an_error() {
        let err = plain(r#"{"type":"Point","coordinates":[1]}"#).unwrap_err();
        assert!(err.contains("at least 2 numbers"), "{err}");
    }

    #[test]
    fn non_numeric_coordinate_is_an_error() {
        let err = plain(r#"{"type":"Point","coordinates":["1","2"]}"#).unwrap_err();
        assert!(err.contains("must be a number"), "{err}");
    }

    #[test]
    fn bare_geometry_and_no_header() {
        let csv = convert_str(
            r#"{"type":"Point","coordinates":[1,2]}"#,
            "lonlat",
            "coords",
            "all",
            "auto",
            -1,
            "none",
            false,
            "comma",
            false,
        )
        .unwrap();
        assert_eq!(csv, "1,2");
    }

    #[test]
    fn latlon_order_swaps_columns_and_header() {
        let csv = convert_str(
            r#"{"type":"Point","coordinates":[1,2]}"#,
            "latlon",
            "coords",
            "all",
            "never",
            -1,
            "none",
            false,
            "comma",
            true,
        )
        .unwrap();
        assert_eq!(csv, "latitude,longitude\n2,1");
    }

    #[test]
    fn polygon_ring_close_is_dropped_on_request() {
        let poly = r#"{"type":"Polygon","coordinates":[[[0,0],[1,0],[1,1],[0,0]]]}"#;
        let kept = plain(poly).unwrap();
        assert_eq!(kept.lines().count(), 5); // header + 4 positions
        let closed = convert_str(
            poly, "lonlat", "coords", "all", "auto", -1, "ring-close", false, "comma", false,
        )
        .unwrap();
        assert_eq!(closed, "0,0\n1,0\n1,1");
    }

    #[test]
    fn adjacent_dedupe_collapses_repeats_globally_too() {
        let line = r#"{"type":"LineString","coordinates":[[0,0],[0,0],[1,1],[0,0]]}"#;
        let adjacent = convert_str(
            line, "lonlat", "coords", "all", "auto", -1, "adjacent", false, "comma", false,
        )
        .unwrap();
        assert_eq!(adjacent, "0,0\n1,1\n0,0");
        let all = convert_str(
            line, "lonlat", "coords", "all", "auto", -1, "all", false, "comma", false,
        )
        .unwrap();
        assert_eq!(all, "0,0\n1,1");
    }

    #[test]
    fn full_columns_number_parts_rings_and_positions() {
        let csv = convert_str(
            r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[1,0],[1,1],[0,0]]],[[[5,5],[6,5],[6,6],[5,5]]]]}"#,
            "lonlat",
            "full",
            "all",
            "never",
            -1,
            "ring-close",
            false,
            "comma",
            true,
        )
        .unwrap();
        assert_eq!(
            csv,
            "index,feature_index,feature_id,geometry_type,part,ring,position,ring_close,longitude,latitude\n\
             1,1,,MultiPolygon,1,1,1,false,0,0\n\
             2,1,,MultiPolygon,1,1,2,false,1,0\n\
             3,1,,MultiPolygon,1,1,3,false,1,1\n\
             4,1,,MultiPolygon,2,1,1,false,5,5\n\
             5,1,,MultiPolygon,2,1,2,false,6,5\n\
             6,1,,MultiPolygon,2,1,3,false,6,6"
        );
    }

    #[test]
    fn ring_close_flags_the_repeated_closing_vertex_and_holes_are_numbered() {
        let csv = convert_str(
            r#"{"type":"Polygon","coordinates":[[[0,0],[4,0],[4,4],[0,0]],[[1,1],[2,1],[2,2],[1,1]]]}"#,
            "lonlat",
            "full",
            "all",
            "never",
            -1,
            "none",
            false,
            "comma",
            false,
        )
        .unwrap();
        // Ring 1 is the outer ring, ring 2 the hole; each ring's 4th position is
        // the repeat of its first and is flagged rather than silently kept.
        assert_eq!(
            csv,
            "1,1,,Polygon,1,1,1,false,0,0\n\
             2,1,,Polygon,1,1,2,false,4,0\n\
             3,1,,Polygon,1,1,3,false,4,4\n\
             4,1,,Polygon,1,1,4,true,0,0\n\
             5,1,,Polygon,1,2,1,false,1,1\n\
             6,1,,Polygon,1,2,2,false,2,1\n\
             7,1,,Polygon,1,2,3,false,2,2\n\
             8,1,,Polygon,1,2,4,true,1,1"
        );
    }

    #[test]
    fn feature_columns_carry_id_and_geometry_type() {
        let csv = convert_str(
            FC, "lonlat", "feature", "all", "never", -1, "none", false, "comma", true,
        )
        .unwrap();
        assert_eq!(
            csv,
            "index,feature_index,feature_id,geometry_type,longitude,latitude\n\
             1,1,a,LineString,-105.1,40.1\n\
             2,1,a,LineString,-105.2,40.2\n\
             3,2,,Point,10.5,20.25"
        );
    }

    #[test]
    fn properties_are_repeated_on_every_coordinate_row() {
        let csv = convert_str(
            FC, "lonlat", "coords", "all", "never", -1, "none", true, "comma", true,
        )
        .unwrap();
        assert_eq!(
            csv,
            "longitude,latitude,name,tags\n\
             -105.1,40.1,Trail,\n\
             -105.2,40.2,Trail,\n\
             10.5,20.25,Site,\"[\"\"x\"\"]\""
        );
    }

    #[test]
    fn shapes_filter_keeps_only_the_chosen_kind() {
        let csv = convert_str(
            FC, "lonlat", "coords", "points", "never", -1, "none", false, "comma", false,
        )
        .unwrap();
        assert_eq!(csv, "10.5,20.25");
    }

    #[test]
    fn precision_rounds_and_pads_coordinates() {
        let csv = convert_str(
            r#"{"type":"Point","coordinates":[-105.123456,40.5]}"#,
            "lonlat",
            "coords",
            "all",
            "auto",
            3,
            "none",
            false,
            "comma",
            false,
        )
        .unwrap();
        assert_eq!(csv, "-105.123,40.500");
    }

    #[test]
    fn elevation_always_writes_a_blank_column() {
        let csv = convert_str(
            r#"{"type":"Point","coordinates":[1,2]}"#,
            "lonlat",
            "coords",
            "all",
            "always",
            -1,
            "none",
            false,
            "comma",
            true,
        )
        .unwrap();
        assert_eq!(csv, "longitude,latitude,elevation\n1,2,");
    }

    #[test]
    fn delimiters_and_quoting_follow_rfc_4180() {
        let csv = convert_str(
            r#"{"type":"Feature","properties":{"note":"a;b"},"geometry":{"type":"Point","coordinates":[1,2]}}"#,
            "lonlat",
            "coords",
            "all",
            "never",
            -1,
            "none",
            true,
            "semicolon",
            true,
        )
        .unwrap();
        assert_eq!(csv, "longitude;latitude;note\n1;2;\"a;b\"");
    }

    #[test]
    fn geometry_collection_walks_every_member() {
        let csv = convert_str(
            r#"{"type":"GeometryCollection","geometries":[{"type":"Point","coordinates":[1,2]},{"type":"LineString","coordinates":[[3,4],[5,6]]}]}"#,
            "lonlat",
            "full",
            "all",
            "never",
            -1,
            "none",
            false,
            "comma",
            false,
        )
        .unwrap();
        assert_eq!(
            csv,
            "1,1,,Point,1,,1,false,1,2\n\
             2,1,,LineString,2,,1,false,3,4\n\
             3,1,,LineString,2,,2,false,5,6"
        );
    }

    #[test]
    fn null_geometry_features_are_skipped() {
        let csv = plain(
            r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{},"geometry":null},{"type":"Feature","properties":{},"geometry":{"type":"Point","coordinates":[7,8]}}]}"#,
        )
        .unwrap();
        assert_eq!(csv, "longitude,latitude\n7,8");
    }

    #[test]
    fn top_level_array_of_geometries_is_accepted() {
        let csv = plain(r#"[{"type":"Point","coordinates":[1,2]},{"type":"Point","coordinates":[3,4]}]"#)
            .unwrap();
        assert_eq!(csv, "longitude,latitude\n1,2\n3,4");
    }

    #[test]
    fn invalid_option_values_are_reported() {
        let err = convert_str(
            r#"{"type":"Point","coordinates":[1,2]}"#,
            "nope",
            "coords",
            "all",
            "auto",
            -1,
            "none",
            false,
            "comma",
            true,
        )
        .unwrap_err();
        assert!(err.contains("invalid order 'nope'"), "{err}");
        let err = convert_str(
            r#"{"type":"Point","coordinates":[1,2]}"#,
            "lonlat",
            "coords",
            "all",
            "auto",
            42,
            "none",
            false,
            "comma",
            true,
        )
        .unwrap_err();
        assert!(err.contains("invalid precision '42'"), "{err}");
    }

    #[test]
    fn empty_feature_collection_yields_just_the_header() {
        let csv = plain(r#"{"type":"FeatureCollection","features":[]}"#).unwrap();
        assert_eq!(csv, "longitude,latitude");
    }
}
