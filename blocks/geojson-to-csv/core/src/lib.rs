//! gizza-ai/geojson-to-csv core — flatten GeoJSON features into a CSV table, one
//! row per feature: every `properties` field becomes a column (union across all
//! features, in first-seen order) and the geometry is emitted as WKT and/or
//! longitude/latitude columns. Pure-Rust (`serde_json`), no wafer/wasm-bindgen
//! deps — shared by the chat block, the CLI, and the page.

use serde_json::{Map, Value};

/// How each feature's geometry is written into the CSV.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GeometryMode {
    /// A single `geometry` column holding 2-D Well-Known Text (POINT, LINESTRING,
    /// POLYGON, …). Lossless for every geometry type.
    Wkt,
    /// `longitude` + `latitude` columns from the geometry's first coordinate
    /// (for a Point, its position).
    LonLat,
    /// Both the WKT column and the longitude/latitude columns.
    Both,
    /// Drop geometry entirely — properties only.
    None,
}

impl GeometryMode {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "wkt" => Ok(GeometryMode::Wkt),
            "lonlat" | "lon_lat" | "coordinates" => Ok(GeometryMode::LonLat),
            "both" => Ok(GeometryMode::Both),
            "none" | "off" => Ok(GeometryMode::None),
            other => Err(format!(
                "invalid geometry '{other}': expected one of wkt, lonlat, both, none"
            )),
        }
    }
    fn wants_wkt(self) -> bool {
        matches!(self, GeometryMode::Wkt | GeometryMode::Both)
    }
    fn wants_lonlat(self) -> bool {
        matches!(self, GeometryMode::LonLat | GeometryMode::Both)
    }
}

/// How nested property objects/arrays are represented.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NestedMode {
    /// Nested objects/arrays stay in one column as compact JSON text.
    Json,
    /// Nested objects/arrays are expanded into dot-notated leaf columns
    /// (`address.city`, `tags.0`).
    Flatten,
}

impl NestedMode {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "json" => Ok(NestedMode::Json),
            "flatten" | "flat" | "dot" => Ok(NestedMode::Flatten),
            other => Err(format!(
                "invalid nested '{other}': expected one of json, flatten"
            )),
        }
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
            "tab" | "\t" => Ok(Delimiter::Tab),
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

/// One parsed feature: its ordered properties and (optional) geometry object.
struct Feature {
    props: Map<String, Value>,
    geom: Option<Value>,
}

/// Pull the features out of a parsed GeoJSON root. Accepts a FeatureCollection,
/// a single Feature, a bare geometry, or a top-level array of any of those.
fn extract_features(root: &Value) -> Result<Vec<Feature>, String> {
    if let Some(arr) = root.as_array() {
        return arr.iter().map(feature_from_value).collect();
    }
    let t = root
        .get("type")
        .and_then(Value::as_str)
        .ok_or("not GeoJSON: the JSON has no top-level \"type\" (expected FeatureCollection, Feature, or a geometry like Point/Polygon)")?;
    if t == "FeatureCollection" {
        let feats = root
            .get("features")
            .and_then(Value::as_array)
            .ok_or("invalid FeatureCollection: missing a \"features\" array")?;
        feats.iter().map(feature_from_value).collect()
    } else {
        Ok(vec![feature_from_value(root)?])
    }
}

/// Interpret a single JSON value as a Feature (its `properties`/`geometry`) or a
/// bare geometry (empty properties).
fn feature_from_value(v: &Value) -> Result<Feature, String> {
    let obj = v
        .as_object()
        .ok_or("invalid feature: expected a JSON object")?;
    let t = obj.get("type").and_then(Value::as_str).unwrap_or("");
    if t == "Feature" {
        let props = match obj.get("properties") {
            Some(Value::Object(m)) => m.clone(),
            // properties may be null or omitted → treat as empty.
            _ => Map::new(),
        };
        let geom = match obj.get("geometry") {
            Some(Value::Null) | None => None,
            Some(g) => Some(g.clone()),
        };
        Ok(Feature { props, geom })
    } else if is_geometry_type(t) {
        Ok(Feature {
            props: Map::new(),
            geom: Some(v.clone()),
        })
    } else {
        Err(format!(
            "unrecognized GeoJSON object type '{t}': expected Feature or a geometry (Point, LineString, Polygon, …)"
        ))
    }
}

fn is_geometry_type(t: &str) -> bool {
    matches!(
        t,
        "Point"
            | "MultiPoint"
            | "LineString"
            | "MultiLineString"
            | "Polygon"
            | "MultiPolygon"
            | "GeometryCollection"
    )
}

/// Format a JSON scalar into a CSV cell, or `None` if it is a container.
fn scalar_cell(v: &Value) -> Option<String> {
    match v {
        Value::Null => Some(String::new()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Number(n) => Some(n.to_string()),
        Value::String(s) => Some(s.clone()),
        _ => None,
    }
}

/// Produce the ordered (column, value) property cells for one feature.
fn property_cells(props: &Map<String, Value>, nested: NestedMode) -> Vec<(String, String)> {
    let mut out = Vec::new();
    match nested {
        NestedMode::Json => {
            for (k, v) in props {
                let cell = scalar_cell(v)
                    .unwrap_or_else(|| serde_json::to_string(v).unwrap_or_default());
                out.push((k.clone(), cell));
            }
        }
        NestedMode::Flatten => {
            for (k, v) in props {
                flatten_into(k.clone(), v, &mut out);
            }
        }
    }
    out
}

/// Recursively expand a value into dot-notated leaf cells. Empty objects/arrays
/// contribute no column.
fn flatten_into(prefix: String, v: &Value, out: &mut Vec<(String, String)>) {
    match v {
        Value::Object(m) => {
            for (k, val) in m {
                flatten_into(format!("{prefix}.{k}"), val, out);
            }
        }
        Value::Array(a) => {
            for (i, val) in a.iter().enumerate() {
                flatten_into(format!("{prefix}.{i}"), val, out);
            }
        }
        _ => {
            // scalar (incl. null → blank)
            out.push((prefix, scalar_cell(v).unwrap_or_default()));
        }
    }
}

/// Format a numeric coordinate component.
fn fmt_num(v: &Value) -> Option<String> {
    v.as_number().map(|n| n.to_string())
}

/// Format a single position `[lon, lat, …]` as WKT `"lon lat"` (2-D).
fn fmt_pos(pos: &Value) -> Option<String> {
    let a = pos.as_array()?;
    let x = fmt_num(a.first()?)?;
    let y = fmt_num(a.get(1)?)?;
    Some(format!("{x} {y}"))
}

/// Format an array of positions as `"lon lat, lon lat, …"`.
fn fmt_ring(ring: &Value) -> Option<String> {
    let a = ring.as_array()?;
    let parts: Option<Vec<String>> = a.iter().map(fmt_pos).collect();
    Some(parts?.join(", "))
}

/// Format an array of rings as `"(…), (…)"`.
fn fmt_rings(rings: &Value) -> Option<String> {
    let a = rings.as_array()?;
    let parts: Option<Vec<String>> = a
        .iter()
        .map(|r| fmt_ring(r).map(|s| format!("({s})")))
        .collect();
    Some(parts?.join(", "))
}

/// Build the 2-D WKT string for a geometry object, or `None` if it is malformed
/// or a null geometry.
fn to_wkt(geom: &Value) -> Option<String> {
    let t = geom.get("type")?.as_str()?;
    if t == "GeometryCollection" {
        let geoms = geom.get("geometries")?.as_array()?;
        let parts: Option<Vec<String>> = geoms.iter().map(to_wkt).collect();
        return Some(format!("GEOMETRYCOLLECTION ({})", parts?.join(", ")));
    }
    let coords = geom.get("coordinates")?;
    match t {
        "Point" => Some(format!("POINT ({})", fmt_pos(coords)?)),
        "MultiPoint" => {
            let a = coords.as_array()?;
            let parts: Option<Vec<String>> =
                a.iter().map(|p| fmt_pos(p).map(|s| format!("({s})"))).collect();
            Some(format!("MULTIPOINT ({})", parts?.join(", ")))
        }
        "LineString" => Some(format!("LINESTRING ({})", fmt_ring(coords)?)),
        "MultiLineString" => Some(format!("MULTILINESTRING ({})", fmt_rings(coords)?)),
        "Polygon" => Some(format!("POLYGON ({})", fmt_rings(coords)?)),
        "MultiPolygon" => {
            let a = coords.as_array()?;
            let parts: Option<Vec<String>> = a
                .iter()
                .map(|poly| fmt_rings(poly).map(|s| format!("({s})")))
                .collect();
            Some(format!("MULTIPOLYGON ({})", parts?.join(", ")))
        }
        _ => None,
    }
}

/// Find the first coordinate position of a geometry (for lon/lat columns).
fn first_position(geom: &Value) -> Option<(String, String)> {
    let t = geom.get("type")?.as_str()?;
    if t == "GeometryCollection" {
        let start = geom.get("geometries")?.as_array()?.first()?;
        return first_position(start);
    }
    descend_position(geom.get("coordinates")?)
}

/// Drill through nested coordinate arrays to the first `[lon, lat]` position.
fn descend_position(v: &Value) -> Option<(String, String)> {
    let a = v.as_array()?;
    let first = a.first()?;
    if first.is_number() {
        // `a` itself is a position.
        Some((fmt_num(a.first()?)?, fmt_num(a.get(1)?)?))
    } else {
        descend_position(first)
    }
}

/// RFC-4180 escape: quote a field containing the delimiter, a quote, CR, or LF.
fn escape(field: &str, delim: char) -> String {
    let needs = field.contains(delim)
        || field.contains('"')
        || field.contains('\n')
        || field.contains('\r');
    if needs {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

/// Convert a GeoJSON document into CSV. See the module docs / descriptor for the
/// column set and parameter semantics.
pub fn convert(
    geojson: &str,
    geometry: GeometryMode,
    nested: NestedMode,
    delimiter: Delimiter,
    header: bool,
) -> Result<String, String> {
    if geojson.trim().is_empty() {
        return Err("no GeoJSON provided: paste a GeoJSON document (a FeatureCollection, Feature, or geometry)".into());
    }
    let root: Value = serde_json::from_str(geojson).map_err(|e| format!("invalid JSON: {e}"))?;
    let features = extract_features(&root)?;
    if features.is_empty() {
        return Err(
            "no features found: the FeatureCollection has an empty \"features\" array".into(),
        );
    }

    // Per-feature property cells + the global column order (first-seen).
    let per_feature: Vec<Vec<(String, String)>> = features
        .iter()
        .map(|f| property_cells(&f.props, nested))
        .collect();

    let mut prop_cols: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for cells in &per_feature {
        for (col, _) in cells {
            if seen.insert(col.clone()) {
                prop_cols.push(col.clone());
            }
        }
    }

    // Geometry columns, appended after the property columns.
    let mut geom_cols: Vec<&str> = Vec::new();
    if geometry.wants_lonlat() {
        geom_cols.push("longitude");
        geom_cols.push("latitude");
    }
    if geometry.wants_wkt() {
        geom_cols.push("geometry");
    }

    if prop_cols.is_empty() && geom_cols.is_empty() {
        return Err("nothing to output: geometry=none and the features have no properties".into());
    }

    let delim = delimiter.ch();
    let mut all_cols: Vec<String> = prop_cols.clone();
    all_cols.extend(geom_cols.iter().map(|s| s.to_string()));

    let mut out = String::new();
    if header {
        let line: Vec<String> = all_cols.iter().map(|c| escape(c, delim)).collect();
        out.push_str(&line.join(&delim.to_string()));
        out.push('\n');
    }

    for (feature, cells) in features.iter().zip(per_feature.iter()) {
        let lookup: std::collections::HashMap<&str, &str> =
            cells.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();

        let mut fields: Vec<String> = Vec::with_capacity(all_cols.len());
        for col in &prop_cols {
            fields.push(lookup.get(col.as_str()).copied().unwrap_or("").to_string());
        }
        // Geometry cells (computed per feature).
        if geometry.wants_lonlat() {
            let (lon, lat) = match &feature.geom {
                Some(g) => first_position(g).unwrap_or_default(),
                None => (String::new(), String::new()),
            };
            fields.push(lon);
            fields.push(lat);
        }
        if geometry.wants_wkt() {
            let wkt = feature.geom.as_ref().and_then(to_wkt).unwrap_or_default();
            fields.push(wkt);
        }

        let line: Vec<String> = fields.iter().map(|f| escape(f, delim)).collect();
        out.push_str(&line.join(&delim.to_string()));
        out.push('\n');
    }

    Ok(out.trim_end_matches('\n').to_string())
}

/// String-argument convenience used by the chat block, CLI, and web wrapper.
pub fn convert_str(
    geojson: &str,
    geometry: &str,
    nested: &str,
    delimiter: &str,
    header: bool,
) -> Result<String, String> {
    convert(
        geojson,
        GeometryMode::parse(geometry)?,
        NestedMode::parse(nested)?,
        Delimiter::parse(delimiter)?,
        header,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const FC: &str = r#"{
      "type": "FeatureCollection",
      "features": [
        { "type": "Feature",
          "properties": { "name": "Alpha", "pop": 1200 },
          "geometry": { "type": "Point", "coordinates": [-105.0, 40.0] } },
        { "type": "Feature",
          "properties": { "name": "Beta", "pop": 800 },
          "geometry": { "type": "Point", "coordinates": [-106.5, 41.25] } }
      ]
    }"#;

    fn conv(gj: &str, g: &str, n: &str, d: &str, h: bool) -> String {
        convert_str(gj, g, n, d, h).unwrap()
    }

    #[test]
    fn feature_collection_wkt_default() {
        let out = conv(FC, "wkt", "json", "comma", true);
        assert_eq!(
            out,
            "name,pop,geometry\n\
             Alpha,1200,POINT (-105.0 40.0)\n\
             Beta,800,POINT (-106.5 41.25)"
        );
    }

    #[test]
    fn lonlat_columns() {
        let out = conv(FC, "lonlat", "json", "comma", true);
        assert_eq!(
            out,
            "name,pop,longitude,latitude\n\
             Alpha,1200,-105.0,40.0\n\
             Beta,800,-106.5,41.25"
        );
    }

    #[test]
    fn both_geometry_columns() {
        let out = conv(FC, "both", "json", "comma", true);
        assert_eq!(
            out.lines().next().unwrap(),
            "name,pop,longitude,latitude,geometry"
        );
        assert!(out.contains("Alpha,1200,-105.0,40.0,POINT (-105.0 40.0)"));
    }

    #[test]
    fn geometry_none_properties_only() {
        let out = conv(FC, "none", "json", "comma", true);
        assert_eq!(out, "name,pop\nAlpha,1200\nBeta,800");
    }

    #[test]
    fn header_off() {
        let out = conv(FC, "wkt", "json", "comma", false);
        assert!(!out.starts_with("name"));
        assert_eq!(out.lines().count(), 2);
    }

    #[test]
    fn delimiters() {
        assert!(conv(FC, "none", "json", "semicolon", true).starts_with("name;pop"));
        assert!(conv(FC, "none", "json", "tab", true).starts_with("name\tpop"));
        assert!(conv(FC, "none", "json", "pipe", true).starts_with("name|pop"));
    }

    #[test]
    fn union_columns_first_seen_order() {
        // second feature adds a new property; blanks fill where absent.
        let gj = r#"{"type":"FeatureCollection","features":[
          {"type":"Feature","properties":{"a":1},"geometry":null},
          {"type":"Feature","properties":{"a":2,"b":"x"},"geometry":null}
        ]}"#;
        let out = conv(gj, "none", "json", "comma", true);
        assert_eq!(out, "a,b\n1,\n2,x");
    }

    #[test]
    fn nested_json_default_stringifies() {
        let gj = r#"{"type":"Feature","properties":{"info":{"city":"Denver","zip":"80202"},"tags":["a","b"]},"geometry":null}"#;
        let out = conv(gj, "none", "json", "comma", true);
        assert_eq!(
            out,
            "info,tags\n\"{\"\"city\"\":\"\"Denver\"\",\"\"zip\"\":\"\"80202\"\"}\",\"[\"\"a\"\",\"\"b\"\"]\""
        );
    }

    #[test]
    fn nested_flatten_dot_notation() {
        let gj = r#"{"type":"Feature","properties":{"info":{"city":"Denver","zip":"80202"},"tags":["a","b"]},"geometry":null}"#;
        let out = conv(gj, "none", "flatten", "comma", true);
        assert_eq!(out, "info.city,info.zip,tags.0,tags.1\nDenver,80202,a,b");
    }

    #[test]
    fn single_feature_and_bare_geometry() {
        let feat = r#"{"type":"Feature","properties":{"n":"solo"},"geometry":{"type":"Point","coordinates":[1,2]}}"#;
        assert_eq!(
            conv(feat, "wkt", "json", "comma", true),
            "n,geometry\nsolo,POINT (1 2)"
        );
        // bare geometry → no properties, geometry only.
        let geom = r#"{"type":"Point","coordinates":[3,4]}"#;
        assert_eq!(
            conv(geom, "wkt", "json", "comma", true),
            "geometry\nPOINT (3 4)"
        );
    }

    #[test]
    fn top_level_array_of_features() {
        let arr = r#"[
          {"type":"Feature","properties":{"n":"one"},"geometry":null},
          {"type":"Feature","properties":{"n":"two"},"geometry":null}
        ]"#;
        assert_eq!(conv(arr, "none", "json", "comma", true), "n\none\ntwo");
    }

    #[test]
    fn wkt_all_geometry_types() {
        // pipe delimiter so the commas inside WKT don't trigger CSV quoting.
        let wkt = |g: &str| -> String {
            let gj = format!(r#"{{"type":"Feature","properties":{{}},"geometry":{g}}}"#);
            convert_str(&gj, "wkt", "json", "pipe", false).unwrap()
        };
        assert_eq!(
            wkt(r#"{"type":"LineString","coordinates":[[30,10],[10,30],[40,40]]}"#),
            "LINESTRING (30 10, 10 30, 40 40)"
        );
        assert_eq!(
            wkt(r#"{"type":"Polygon","coordinates":[[[30,10],[40,40],[20,40],[10,20],[30,10]]]}"#),
            "POLYGON ((30 10, 40 40, 20 40, 10 20, 30 10))"
        );
        assert_eq!(
            wkt(r#"{"type":"MultiPoint","coordinates":[[10,40],[40,30]]}"#),
            "MULTIPOINT ((10 40), (40 30))"
        );
        assert_eq!(
            wkt(r#"{"type":"MultiLineString","coordinates":[[[10,10],[20,20]],[[40,40],[30,30]]]}"#),
            "MULTILINESTRING ((10 10, 20 20), (40 40, 30 30))"
        );
        assert_eq!(
            wkt(r#"{"type":"MultiPolygon","coordinates":[[[[30,20],[45,40],[10,40],[30,20]]]]}"#),
            "MULTIPOLYGON (((30 20, 45 40, 10 40, 30 20)))"
        );
        assert_eq!(
            wkt(r#"{"type":"GeometryCollection","geometries":[{"type":"Point","coordinates":[4,6]},{"type":"LineString","coordinates":[[4,6],[7,10]]}]}"#),
            "GEOMETRYCOLLECTION (POINT (4 6), LINESTRING (4 6, 7 10))"
        );
    }

    #[test]
    fn first_position_from_polygon_for_lonlat() {
        let gj = r#"{"type":"Feature","properties":{},"geometry":{"type":"Polygon","coordinates":[[[30,10],[40,40],[20,40],[30,10]]]}}"#;
        let out = conv(gj, "lonlat", "json", "comma", true);
        assert_eq!(out, "longitude,latitude\n30,10");
    }

    #[test]
    fn null_geometry_blank_cells() {
        let gj = r#"{"type":"Feature","properties":{"n":"x"},"geometry":null}"#;
        assert_eq!(
            conv(gj, "both", "json", "comma", true),
            "n,longitude,latitude,geometry\nx,,,"
        );
    }

    #[test]
    fn rfc4180_escaping() {
        let gj = r#"{"type":"Feature","properties":{"label":"Cafe, \"The Spot\""},"geometry":null}"#;
        assert_eq!(
            conv(gj, "none", "json", "comma", true),
            "label\n\"Cafe, \"\"The Spot\"\"\""
        );
    }

    #[test]
    fn errors_are_helpful() {
        assert!(convert_str("", "wkt", "json", "comma", true)
            .unwrap_err()
            .contains("no GeoJSON"));
        assert!(convert_str("{bad", "wkt", "json", "comma", true)
            .unwrap_err()
            .contains("invalid JSON"));
        assert!(convert_str("{\"foo\":1}", "wkt", "json", "comma", true)
            .unwrap_err()
            .contains("not GeoJSON"));
        assert!(
            convert_str(r#"{"type":"FeatureCollection","features":[]}"#, "wkt", "json", "comma", true)
                .unwrap_err()
                .contains("no features")
        );
        assert!(convert_str(
            r#"{"type":"Feature","properties":{},"geometry":null}"#,
            "none",
            "json",
            "comma",
            true
        )
        .unwrap_err()
        .contains("nothing to output"));
        assert!(GeometryMode::parse("nope").is_err());
        assert!(NestedMode::parse("nope").is_err());
        assert!(Delimiter::parse("colon").is_err());
    }
}
