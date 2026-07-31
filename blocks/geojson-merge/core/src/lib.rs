//! geojson-merge core — combine multiple GeoJSON inputs into ONE FeatureCollection.
//!
//! Each input value is normalized (RFC 7946): a `FeatureCollection` contributes
//! its `features`, a bare `Feature` is taken as-is, and a bare geometry
//! (`Point`, `LineString`, `Polygon`, `GeometryCollection`, …) is wrapped into a
//! `Feature` with empty `properties`. Every feature is then concatenated, in
//! input order, into a single `{"type":"FeatureCollection","features":[…]}`.
//!
//! Options (all pure, browser-local): output `indent` (0 = minify), coordinate
//! `precision` rounding (shrinks file size), sequential id `renumber`ing (fixes
//! id collisions across sources), and optional per-feature `source_property`
//! tagging (stamps the 1-based input-document number for traceability).
//!
//! Pure-Rust (serde_json with preserve_order — property key order is kept).

use serde::Serialize;
use serde_json::{json, Map, Value};

/// The seven RFC 7946 geometry object types (a bare one is wrapped as a Feature).
const GEOMETRY_TYPES: [&str; 7] = [
    "Point",
    "MultiPoint",
    "LineString",
    "MultiLineString",
    "Polygon",
    "MultiPolygon",
    "GeometryCollection",
];

/// Normalize one top-level GeoJSON value into a list of Features. `doc` is the
/// 1-based input-document index, used only for error messages.
fn normalize(v: Value, doc: usize) -> Result<Vec<Value>, String> {
    match v.get("type").and_then(Value::as_str) {
        Some("FeatureCollection") => match v.get("features") {
            Some(Value::Array(features)) => Ok(features.clone()),
            _ => Err(format!(
                "GeoJSON value #{doc}: FeatureCollection is missing its \"features\" array"
            )),
        },
        Some("Feature") => Ok(vec![v]),
        Some(t) if GEOMETRY_TYPES.contains(&t) => {
            // Wrap a bare geometry into a Feature with empty properties.
            let mut feat = Map::new();
            feat.insert("type".to_string(), Value::from("Feature"));
            feat.insert("properties".to_string(), Value::Object(Map::new()));
            feat.insert("geometry".to_string(), v);
            Ok(vec![Value::Object(feat)])
        }
        Some(other) => Err(format!(
            "GeoJSON value #{doc}: unsupported type \"{other}\" (expected Feature, FeatureCollection, or a geometry)"
        )),
        None => Err(format!(
            "GeoJSON value #{doc} is not a GeoJSON object (missing a \"type\" member)"
        )),
    }
}

/// Stamp `key = doc` into a feature's `properties` (creating the object if the
/// feature has no/`null` properties). No-op on non-object entries.
fn tag_source(feature: &mut Value, key: &str, doc: usize) {
    if let Value::Object(m) = feature {
        match m.get_mut("properties") {
            Some(Value::Object(props)) => {
                props.insert(key.to_string(), Value::from(doc as u64));
            }
            _ => {
                let mut props = Map::new();
                props.insert(key.to_string(), Value::from(doc as u64));
                m.insert("properties".to_string(), Value::Object(props));
            }
        }
    }
}

/// Recursively round every number in a `coordinates` value (nested arrays) to
/// `p` decimal places.
fn round_coords(v: &mut Value, p: i64) {
    match v {
        Value::Array(a) => {
            for e in a.iter_mut() {
                round_coords(e, p);
            }
        }
        Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                let factor = 10f64.powi(p as i32);
                let rounded = (f * factor).round() / factor;
                // + 0.0 avoids emitting a "-0" for values that round to zero.
                if let Some(num) = serde_json::Number::from_f64(rounded + 0.0) {
                    *v = Value::Number(num);
                }
            }
        }
        _ => {}
    }
}

/// Round the coordinates of a geometry object (recursing into a
/// GeometryCollection's `geometries`).
fn round_geometry(geom: &mut Value, p: i64) {
    if let Value::Object(m) = geom {
        if m.get("type").and_then(Value::as_str) == Some("GeometryCollection") {
            if let Some(Value::Array(geoms)) = m.get_mut("geometries") {
                for g in geoms.iter_mut() {
                    round_geometry(g, p);
                }
            }
        } else if let Some(coords) = m.get_mut("coordinates") {
            round_coords(coords, p);
        }
    }
}

fn write_pretty(value: &Value, indent: usize) -> Result<String, String> {
    if indent == 0 {
        return serde_json::to_string(value).map_err(|e| format!("serialize: {e}"));
    }
    let pad = vec![b' '; indent.min(8)];
    let mut buf = Vec::new();
    let fmt = serde_json::ser::PrettyFormatter::with_indent(&pad);
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, fmt);
    value
        .serialize(&mut ser)
        .map_err(|e| format!("serialize: {e}"))?;
    String::from_utf8(buf).map_err(|e| format!("utf8: {e}"))
}

/// Combine every GeoJSON value in `input` (whitespace/newline-separated —
/// line-delimited GeoJSON works too) into a single FeatureCollection.
///
/// * `indent` — output spaces per level (0 = minified single line, capped at 8).
/// * `precision` — round coordinates to this many decimals; `< 0` keeps full precision.
/// * `renumber` — assign each output feature a sequential integer `id` (0-based).
/// * `source_property` — if non-empty, tag each feature's properties with this
///   key set to its 1-based source-document number.
pub fn merge_geojson(
    input: &str,
    indent: usize,
    precision: i64,
    renumber: bool,
    source_property: &str,
) -> Result<String, String> {
    if input.trim().is_empty() {
        return Err("no GeoJSON input".into());
    }

    let stream = serde_json::Deserializer::from_str(input).into_iter::<Value>();
    let source_key = source_property.trim();
    let mut features: Vec<Value> = Vec::new();
    let mut doc = 0usize;
    for r in stream {
        doc += 1;
        let v = r.map_err(|e| format!("GeoJSON value #{doc} is invalid: {e}"))?;
        let mut fs = normalize(v, doc)?;
        if !source_key.is_empty() {
            for f in fs.iter_mut() {
                tag_source(f, source_key, doc);
            }
        }
        features.append(&mut fs);
    }
    if doc == 0 {
        return Err("no GeoJSON values found".into());
    }

    for (i, f) in features.iter_mut().enumerate() {
        if let Value::Object(m) = f {
            if renumber {
                m.insert("id".to_string(), Value::from(i as u64));
            }
            if precision >= 0 {
                if let Some(g) = m.get_mut("geometry") {
                    round_geometry(g, precision);
                }
            }
        }
    }

    let fc = json!({ "type": "FeatureCollection", "features": features });
    write_pretty(&fc, indent)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn parse(s: &str) -> Value {
        serde_json::from_str(s).unwrap()
    }

    #[test]
    fn merges_two_feature_collections() {
        let a = r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"n":"a"},"geometry":{"type":"Point","coordinates":[0,0]}}]}"#;
        let b = r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"n":"b"},"geometry":{"type":"Point","coordinates":[1,1]}}]}"#;
        let out = merge_geojson(&format!("{a}\n{b}"), 0, -1, false, "").unwrap();
        let v = parse(&out);
        assert_eq!(v["type"], "FeatureCollection");
        assert_eq!(v["features"].as_array().unwrap().len(), 2);
        assert_eq!(v["features"][0]["properties"]["n"], "a");
        assert_eq!(v["features"][1]["properties"]["n"], "b");
    }

    #[test]
    fn wraps_bare_geometry_as_feature() {
        let out =
            merge_geojson(r#"{"type":"Point","coordinates":[3,4]}"#, 0, -1, false, "").unwrap();
        let v = parse(&out);
        assert_eq!(v["features"][0]["type"], "Feature");
        assert_eq!(v["features"][0]["properties"], serde_json::json!({}));
        assert_eq!(v["features"][0]["geometry"]["type"], "Point");
        assert_eq!(v["features"][0]["geometry"]["coordinates"][0], 3);
    }

    #[test]
    fn accepts_bare_feature_and_collection_mixed() {
        let feat =
            r#"{"type":"Feature","properties":{},"geometry":{"type":"Point","coordinates":[0,0]}}"#;
        let fc = r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{},"geometry":{"type":"Point","coordinates":[1,1]}}]}"#;
        let out = merge_geojson(&format!("{feat} {fc}"), 0, -1, false, "").unwrap();
        let v = parse(&out);
        assert_eq!(v["features"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn renumbers_ids_sequentially() {
        let input = r#"{"type":"FeatureCollection","features":[
            {"type":"Feature","id":"x","properties":{},"geometry":{"type":"Point","coordinates":[0,0]}},
            {"type":"Feature","id":"x","properties":{},"geometry":{"type":"Point","coordinates":[1,1]}}
        ]}"#;
        let out = merge_geojson(input, 0, -1, true, "").unwrap();
        let v = parse(&out);
        assert_eq!(v["features"][0]["id"], 0);
        assert_eq!(v["features"][1]["id"], 1);
    }

    #[test]
    fn rounds_coordinate_precision() {
        let out = merge_geojson(
            r#"{"type":"Point","coordinates":[1.234567,8.987654]}"#,
            0,
            3,
            false,
            "",
        )
        .unwrap();
        let v = parse(&out);
        assert_eq!(v["features"][0]["geometry"]["coordinates"][0], 1.235);
        assert_eq!(v["features"][0]["geometry"]["coordinates"][1], 8.988);
    }

    #[test]
    fn rounds_precision_in_geometry_collection() {
        let input = r#"{"type":"GeometryCollection","geometries":[{"type":"Point","coordinates":[2.71828,3.14159]}]}"#;
        let out = merge_geojson(input, 0, 2, false, "").unwrap();
        let v = parse(&out);
        assert_eq!(
            v["features"][0]["geometry"]["geometries"][0]["coordinates"][0],
            2.72
        );
    }

    #[test]
    fn tags_source_document_number() {
        let a = r#"{"type":"Feature","properties":{"n":"a"},"geometry":{"type":"Point","coordinates":[0,0]}}"#;
        let b = r#"{"type":"Point","coordinates":[1,1]}"#;
        let out = merge_geojson(&format!("{a}\n{b}"), 0, -1, false, "src").unwrap();
        let v = parse(&out);
        assert_eq!(v["features"][0]["properties"]["src"], 1);
        assert_eq!(v["features"][0]["properties"]["n"], "a");
        assert_eq!(v["features"][1]["properties"]["src"], 2);
    }

    #[test]
    fn minifies_and_pretty_prints() {
        let one = r#"{"type":"Point","coordinates":[0,0]}"#;
        let min = merge_geojson(one, 0, -1, false, "").unwrap();
        assert!(!min.contains('\n'));
        let pretty = merge_geojson(one, 2, -1, false, "").unwrap();
        assert!(pretty.contains("\n  \"type\": \"FeatureCollection\""));
    }

    #[test]
    fn empty_collection_still_valid() {
        let out = merge_geojson(
            r#"{"type":"FeatureCollection","features":[]}"#,
            0,
            -1,
            false,
            "",
        )
        .unwrap();
        assert_eq!(out, r#"{"type":"FeatureCollection","features":[]}"#);
    }

    #[test]
    fn errors() {
        assert!(merge_geojson("", 0, -1, false, "").is_err());
        assert!(merge_geojson("   ", 0, -1, false, "").is_err());
        assert!(merge_geojson("{not json}", 0, -1, false, "").is_err());
        // unsupported top-level type
        assert!(merge_geojson(r#"{"type":"Nonsense"}"#, 0, -1, false, "").is_err());
        // missing type
        assert!(merge_geojson(r#"{"foo":1}"#, 0, -1, false, "").is_err());
        // FeatureCollection without features array
        assert!(merge_geojson(r#"{"type":"FeatureCollection"}"#, 0, -1, false, "").is_err());
    }
}
