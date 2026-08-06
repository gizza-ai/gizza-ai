//! gizza-ai/topojson-to-geojson core — expand a compact TopoJSON topology into
//! standard RFC 7946 GeoJSON.
//!
//! TopoJSON stores every shared boundary exactly once, as an *arc*, and every
//! geometry refers to arcs by index (a negative index `~i` = arc `-i - 1`
//! traversed backwards). When the topology is quantized it also carries a
//! `transform` (`scale` + `translate`), and each arc's positions are
//! delta-encoded: the first position is absolute, the rest are offsets from the
//! previous one. Expanding therefore means: decode every arc once, then stitch
//! the referenced arcs back together per ring/line, de-duplicating the shared
//! endpoint where two arcs meet.
//!
//! Pure Rust, no I/O — shared by the chat skill block and the web page.

use serde::Serialize;
use serde_json::{Map, Value};

/// Shape of the emitted GeoJSON document.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Output {
    /// An RFC 7946 `FeatureCollection` — keeps `properties` and `id`.
    FeatureCollection,
    /// A bare `GeometryCollection` — geometry only, no `Feature` wrapper.
    GeometryCollection,
}

impl Output {
    /// Parse the `output` param. Blank/unknown falls back to `feature-collection`.
    pub fn parse(s: &str) -> Output {
        match s.trim().to_ascii_lowercase().replace('_', "-").as_str() {
            "geometry-collection" | "geometrycollection" | "geometries" => Output::GeometryCollection,
            _ => Output::FeatureCollection,
        }
    }
}

/// Conversion options.
#[derive(Clone, Debug)]
pub struct Options {
    /// Name of the single topology object to expand. Blank = every object,
    /// merged into one collection in the order they appear.
    pub object: String,
    /// Emit features or bare geometries.
    pub output: Output,
    /// Add a 2D `bbox` computed from the emitted coordinates.
    pub include_bbox: bool,
    /// Decimal places to round coordinates to; negative = full precision.
    pub precision: i64,
    /// Spaces of indentation per level (clamped 0..=8); `0` minifies.
    pub indent: usize,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            object: String::new(),
            output: Output::FeatureCollection,
            include_bbox: false,
            precision: -1,
            indent: 2,
        }
    }
}

/// A single decoded position: `[x, y]` plus any extra dimensions carried through.
type Position = Vec<f64>;

/// The quantization transform, when the topology has one.
struct Transform {
    scale: [f64; 2],
    translate: [f64; 2],
}

impl Transform {
    /// Map one quantized position to its real-world coordinates. Dimensions
    /// beyond x/y (elevation, measures) pass through untouched, as they are not
    /// quantized.
    fn apply(&self, q: &[f64]) -> Position {
        let mut out = q.to_vec();
        out[0] = q[0] * self.scale[0] + self.translate[0];
        out[1] = q[1] * self.scale[1] + self.translate[1];
        out
    }
}

/// Expand `src` (TopoJSON text) into GeoJSON text.
pub fn topojson_to_geojson(src: &str, opts: &Options) -> Result<String, String> {
    let root: Value = serde_json::from_str(src.trim())
        .map_err(|e| format!("invalid JSON: {e}"))?;
    let topo = root
        .as_object()
        .ok_or_else(|| "expected a TopoJSON object at the top level, got a bare JSON value".to_string())?;

    check_is_topology(topo)?;

    let transform = parse_transform(topo)?;
    let arcs = decode_arcs(topo, transform.as_ref())?;
    let objects = topo
        .get("objects")
        .ok_or_else(|| "not a valid topology: missing the \"objects\" member".to_string())?
        .as_object()
        .ok_or_else(|| "\"objects\" must be an object mapping names to geometry objects".to_string())?;

    let selected = select_objects(objects, &opts.object)?;
    let ctx = Ctx { arcs: &arcs, transform: transform.as_ref(), precision: opts.precision };

    let doc = match opts.output {
        Output::FeatureCollection => {
            let mut features = Vec::new();
            for (name, obj) in &selected {
                collect_features(&ctx, name, obj, &mut features)?;
            }
            let mut m = Map::new();
            m.insert("type".into(), Value::String("FeatureCollection".into()));
            if opts.include_bbox {
                if let Some(bbox) = bbox_of(features.iter().filter_map(|f| f.get("geometry"))) {
                    m.insert("bbox".into(), bbox);
                }
            }
            m.insert("features".into(), Value::Array(features));
            Value::Object(m)
        }
        Output::GeometryCollection => {
            let mut geometries = Vec::new();
            for (name, obj) in &selected {
                collect_geometries(&ctx, name, obj, &mut geometries)?;
            }
            let mut m = Map::new();
            m.insert("type".into(), Value::String("GeometryCollection".into()));
            if opts.include_bbox {
                if let Some(bbox) = bbox_of(geometries.iter()) {
                    m.insert("bbox".into(), bbox);
                }
            }
            m.insert("geometries".into(), Value::Array(geometries));
            Value::Object(m)
        }
    };

    serialize(&doc, opts.indent.min(8))
}

/// Reject anything that clearly isn't a topology, with a message that names what
/// the input looks like instead.
fn check_is_topology(topo: &Map<String, Value>) -> Result<(), String> {
    match topo.get("type").and_then(Value::as_str) {
        Some("Topology") => Ok(()),
        Some(t @ ("FeatureCollection" | "Feature" | "GeometryCollection")) => Err(format!(
            "expected \"type\": \"Topology\", got \"{t}\" — this input is already GeoJSON, not TopoJSON"
        )),
        Some(t) => Err(format!("expected \"type\": \"Topology\", got \"{t}\"")),
        None => Err("not a valid topology: missing the \"type\": \"Topology\" member".into()),
    }
}

fn parse_transform(topo: &Map<String, Value>) -> Result<Option<Transform>, String> {
    let Some(t) = topo.get("transform") else { return Ok(None) };
    if t.is_null() {
        return Ok(None);
    }
    let t = t
        .as_object()
        .ok_or_else(|| "\"transform\" must be an object with \"scale\" and \"translate\"".to_string())?;
    let pair = |key: &str| -> Result<[f64; 2], String> {
        let a = t
            .get(key)
            .and_then(Value::as_array)
            .ok_or_else(|| format!("\"transform.{key}\" must be an array of two numbers"))?;
        if a.len() < 2 {
            return Err(format!(
                "\"transform.{key}\" must have two numbers, got {}",
                a.len()
            ));
        }
        let n = |i: usize| {
            a[i].as_f64()
                .ok_or_else(|| format!("\"transform.{key}[{i}]\" must be a number, got {}", kind_of(&a[i])))
        };
        Ok([n(0)?, n(1)?])
    };
    Ok(Some(Transform { scale: pair("scale")?, translate: pair("translate")? }))
}

/// Decode every arc once: delta-accumulate (when quantized) and apply the
/// transform. Arcs are shared between geometries, so decoding is done up front
/// and the result reused by index.
fn decode_arcs(topo: &Map<String, Value>, transform: Option<&Transform>) -> Result<Vec<Vec<Position>>, String> {
    let raw = match topo.get("arcs") {
        None | Some(Value::Null) => return Ok(Vec::new()),
        Some(v) => v
            .as_array()
            .ok_or_else(|| "\"arcs\" must be an array of arcs".to_string())?,
    };
    let mut out = Vec::with_capacity(raw.len());
    for (ai, arc) in raw.iter().enumerate() {
        let positions = arc
            .as_array()
            .ok_or_else(|| format!("arc {ai} must be an array of positions, got {}", kind_of(arc)))?;
        let mut decoded: Vec<Position> = Vec::with_capacity(positions.len());
        let (mut x, mut y) = (0.0f64, 0.0f64);
        for (pi, pos) in positions.iter().enumerate() {
            let p = read_position(pos, &format!("arc {ai} position {pi}"))?;
            match transform {
                // Quantized: positions are deltas from the previous one.
                Some(tf) => {
                    x += p[0];
                    y += p[1];
                    let mut q = p.clone();
                    q[0] = x;
                    q[1] = y;
                    decoded.push(tf.apply(&q));
                }
                // Un-quantized: positions are already absolute coordinates.
                None => decoded.push(p),
            }
        }
        if decoded.is_empty() {
            return Err(format!("arc {ai} is empty: an arc needs at least one position"));
        }
        out.push(decoded);
    }
    Ok(out)
}

fn read_position(v: &Value, what: &str) -> Result<Position, String> {
    let a = v
        .as_array()
        .ok_or_else(|| format!("{what} must be an array of numbers, got {}", kind_of(v)))?;
    if a.len() < 2 {
        return Err(format!("{what} must have at least 2 numbers, got {}", a.len()));
    }
    let mut out = Vec::with_capacity(a.len());
    for (i, n) in a.iter().enumerate() {
        out.push(
            n.as_f64()
                .ok_or_else(|| format!("{what}[{i}] must be a number, got {}", kind_of(n)))?,
        );
    }
    Ok(out)
}

/// Resolve the `object` param to the topology objects to expand, preserving the
/// document's own ordering when all of them are selected.
fn select_objects<'a>(
    objects: &'a Map<String, Value>,
    wanted: &str,
) -> Result<Vec<(&'a str, &'a Value)>, String> {
    let wanted = wanted.trim();
    if wanted.is_empty() {
        return Ok(objects.iter().map(|(k, v)| (k.as_str(), v)).collect());
    }
    match objects.get_key_value(wanted) {
        Some((k, v)) => Ok(vec![(k.as_str(), v)]),
        None => {
            let names: Vec<&str> = objects.keys().map(String::as_str).collect();
            Err(if names.is_empty() {
                format!("no object named \"{wanted}\": this topology has no objects")
            } else {
                format!(
                    "no object named \"{wanted}\": this topology has {}",
                    names
                        .iter()
                        .map(|n| format!("\"{n}\""))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })
        }
    }
}

struct Ctx<'a> {
    arcs: &'a [Vec<Position>],
    transform: Option<&'a Transform>,
    precision: i64,
}

/// A top-level topology object becomes one Feature, or — when it is a
/// GeometryCollection — one Feature per member geometry (the topojson-client
/// convention, and what every scanned converter emits).
fn collect_features(ctx: &Ctx, name: &str, obj: &Value, out: &mut Vec<Value>) -> Result<(), String> {
    let o = obj
        .as_object()
        .ok_or_else(|| format!("object \"{name}\" must be a geometry object, got {}", kind_of(obj)))?;
    if o.get("type").and_then(Value::as_str) == Some("GeometryCollection") {
        for (i, g) in geometries_of(o, name)?.iter().enumerate() {
            let child = g.as_object().ok_or_else(|| {
                format!("object \"{name}\" geometry {i} must be a geometry object, got {}", kind_of(g))
            })?;
            out.push(feature(ctx, &format!("{name}[{i}]"), child)?);
        }
    } else {
        out.push(feature(ctx, name, o)?);
    }
    Ok(())
}

fn collect_geometries(ctx: &Ctx, name: &str, obj: &Value, out: &mut Vec<Value>) -> Result<(), String> {
    let o = obj
        .as_object()
        .ok_or_else(|| format!("object \"{name}\" must be a geometry object, got {}", kind_of(obj)))?;
    if o.get("type").and_then(Value::as_str) == Some("GeometryCollection") {
        for (i, g) in geometries_of(o, name)?.iter().enumerate() {
            let child = g.as_object().ok_or_else(|| {
                format!("object \"{name}\" geometry {i} must be a geometry object, got {}", kind_of(g))
            })?;
            out.push(geometry(ctx, &format!("{name}[{i}]"), child)?);
        }
    } else {
        out.push(geometry(ctx, name, o)?);
    }
    Ok(())
}

fn geometries_of<'a>(o: &'a Map<String, Value>, name: &str) -> Result<&'a Vec<Value>, String> {
    o.get("geometries")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("object \"{name}\" is a GeometryCollection but has no \"geometries\" array"))
}

/// Wrap one geometry as a Feature, carrying `id`, `bbox` and `properties` over.
fn feature(ctx: &Ctx, name: &str, o: &Map<String, Value>) -> Result<Value, String> {
    let geom = geometry(ctx, name, o)?;
    let mut m = Map::new();
    m.insert("type".into(), Value::String("Feature".into()));
    if let Some(id) = o.get("id") {
        if !id.is_null() {
            m.insert("id".into(), id.clone());
        }
    }
    // A geometry object may carry its own bbox; it is already in real-world
    // coordinates (the spec does not quantize bbox), so it passes straight through.
    if let Some(bbox) = o.get("bbox") {
        if !bbox.is_null() {
            m.insert("bbox".into(), bbox.clone());
        }
    }
    let props = match o.get("properties") {
        Some(Value::Object(p)) => Value::Object(p.clone()),
        _ => Value::Object(Map::new()),
    };
    m.insert("properties".into(), props);
    m.insert("geometry".into(), geom);
    Ok(Value::Object(m))
}

/// Expand one TopoJSON geometry into a GeoJSON geometry.
fn geometry(ctx: &Ctx, name: &str, o: &Map<String, Value>) -> Result<Value, String> {
    let ty = match o.get("type") {
        // The spec allows a null-type geometry, which is a null GeoJSON geometry.
        None | Some(Value::Null) => return Ok(Value::Null),
        Some(v) => v
            .as_str()
            .ok_or_else(|| format!("geometry \"{name}\" has a non-string \"type\""))?,
    };

    let coordinates = match ty {
        "GeometryCollection" => {
            let mut geoms = Vec::new();
            for (i, g) in geometries_of(o, name)?.iter().enumerate() {
                let child = g.as_object().ok_or_else(|| {
                    format!("geometry \"{name}\" member {i} must be an object, got {}", kind_of(g))
                })?;
                geoms.push(geometry(ctx, &format!("{name}[{i}]"), child)?);
            }
            let mut m = Map::new();
            m.insert("type".into(), Value::String("GeometryCollection".into()));
            m.insert("geometries".into(), Value::Array(geoms));
            return Ok(Value::Object(m));
        }
        "Point" => pos_value(ctx, &point(ctx, coords_of(o, name)?, name)?),
        "MultiPoint" => {
            let list = coords_array(o, name)?;
            let mut out = Vec::with_capacity(list.len());
            for (i, p) in list.iter().enumerate() {
                out.push(pos_value(ctx, &point(ctx, p, &format!("{name}[{i}]"))?));
            }
            Value::Array(out)
        }
        "LineString" => line_value(ctx, &line(ctx, &arcs_of(o, name)?, name)?),
        "MultiLineString" => {
            let list = arcs_array(o, name)?;
            let mut out = Vec::with_capacity(list.len());
            for (i, l) in list.iter().enumerate() {
                let idx = as_index_list(l, &format!("{name}[{i}]"))?;
                out.push(line_value(ctx, &line(ctx, &idx, &format!("{name}[{i}]"))?));
            }
            Value::Array(out)
        }
        "Polygon" => polygon_value(ctx, arcs_array(o, name)?, name)?,
        "MultiPolygon" => {
            let list = arcs_array(o, name)?;
            let mut out = Vec::with_capacity(list.len());
            for (i, poly) in list.iter().enumerate() {
                let rings = poly.as_array().ok_or_else(|| {
                    format!("geometry \"{name}\" polygon {i} must be an array of rings, got {}", kind_of(poly))
                })?;
                out.push(polygon_value(ctx, rings, &format!("{name}[{i}]"))?);
            }
            Value::Array(out)
        }
        other => {
            return Err(format!(
                "geometry \"{name}\" has unknown type \"{other}\": expected Point, MultiPoint, \
                 LineString, MultiLineString, Polygon, MultiPolygon or GeometryCollection"
            ))
        }
    };

    let mut m = Map::new();
    m.insert("type".into(), Value::String(ty.into()));
    m.insert("coordinates".into(), coordinates);
    Ok(Value::Object(m))
}

fn coords_of<'a>(o: &'a Map<String, Value>, name: &str) -> Result<&'a Value, String> {
    o.get("coordinates")
        .ok_or_else(|| format!("geometry \"{name}\" is missing \"coordinates\""))
}

fn coords_array<'a>(o: &'a Map<String, Value>, name: &str) -> Result<&'a Vec<Value>, String> {
    coords_of(o, name)?
        .as_array()
        .ok_or_else(|| format!("geometry \"{name}\" \"coordinates\" must be an array"))
}

fn arcs_of<'a>(o: &'a Map<String, Value>, name: &str) -> Result<Vec<i64>, String> {
    let v = o
        .get("arcs")
        .ok_or_else(|| format!("geometry \"{name}\" is missing \"arcs\""))?;
    as_index_list(v, name)
}

fn arcs_array<'a>(o: &'a Map<String, Value>, name: &str) -> Result<&'a Vec<Value>, String> {
    o.get("arcs")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("geometry \"{name}\" must have an \"arcs\" array"))
}

fn as_index_list(v: &Value, name: &str) -> Result<Vec<i64>, String> {
    let a = v
        .as_array()
        .ok_or_else(|| format!("geometry \"{name}\" \"arcs\" must be an array of arc indexes, got {}", kind_of(v)))?;
    let mut out = Vec::with_capacity(a.len());
    for x in a {
        out.push(
            x.as_i64()
                .ok_or_else(|| format!("geometry \"{name}\" arc index must be an integer, got {}", kind_of(x)))?,
        );
    }
    Ok(out)
}

/// A standalone Point/MultiPoint position is quantized but *not* delta-encoded,
/// so it only gets the transform applied.
fn point(ctx: &Ctx, v: &Value, name: &str) -> Result<Position, String> {
    let p = read_position(v, &format!("geometry \"{name}\" coordinates"))?;
    Ok(match ctx.transform {
        Some(tf) => tf.apply(&p),
        None => p,
    })
}

/// Stitch the referenced arcs into one continuous list of positions. Consecutive
/// arcs share their touching endpoint, so the previous tail is dropped before
/// the next arc is appended; a negative index means the arc runs backwards.
fn line(ctx: &Ctx, indexes: &[i64], name: &str) -> Result<Vec<Position>, String> {
    if indexes.is_empty() {
        return Err(format!("geometry \"{name}\" references no arcs"));
    }
    let mut points: Vec<Position> = Vec::new();
    for &i in indexes {
        let idx = if i < 0 { (-i - 1) as usize } else { i as usize };
        let arc = ctx.arcs.get(idx).ok_or_else(|| {
            format!(
                "geometry \"{name}\" references arc {i}, but the topology has {} arc{}",
                ctx.arcs.len(),
                if ctx.arcs.len() == 1 { "" } else { "s" }
            )
        })?;
        if !points.is_empty() {
            points.pop();
        }
        let n = arc.len();
        points.extend(arc.iter().cloned());
        if i < 0 {
            let start = points.len() - n;
            points[start..].reverse();
        }
    }
    // Guard against a degenerate one-position arc: a LineString needs 2 positions.
    if points.len() < 2 {
        points.push(points[0].clone());
    }
    Ok(points)
}

/// A ring is a closed line; pad a degenerate ring out to the 4 positions
/// RFC 7946 requires.
fn ring(ctx: &Ctx, indexes: &[i64], name: &str) -> Result<Vec<Position>, String> {
    let mut points = line(ctx, indexes, name)?;
    while points.len() < 4 {
        points.push(points[0].clone());
    }
    Ok(points)
}

fn polygon_value(ctx: &Ctx, rings: &[Value], name: &str) -> Result<Value, String> {
    let mut out = Vec::with_capacity(rings.len());
    for (i, r) in rings.iter().enumerate() {
        let idx = as_index_list(r, &format!("{name}[{i}]"))?;
        out.push(line_value(ctx, &ring(ctx, &idx, &format!("{name}[{i}]"))?));
    }
    Ok(Value::Array(out))
}

fn line_value(ctx: &Ctx, points: &[Position]) -> Value {
    Value::Array(points.iter().map(|p| pos_value(ctx, p)).collect())
}

fn pos_value(ctx: &Ctx, p: &[f64]) -> Value {
    Value::Array(p.iter().map(|&n| num(round(n, ctx.precision))).collect())
}

/// Round to `places` decimals. Quantized topologies decode to values like
/// `-179.99999999999997`; rounding removes that float noise.
fn round(v: f64, places: i64) -> f64 {
    if places < 0 || !v.is_finite() || places > 15 {
        return v;
    }
    let f = 10f64.powi(places as i32);
    let r = (v * f).round() / f;
    // `-0.0` serializes as `-0.0`; normalize it to plain zero.
    if r == 0.0 {
        0.0
    } else {
        r
    }
}

/// Coordinates are always emitted as JSON numbers; a non-finite value can't be
/// represented in JSON, so it degrades to null rather than producing invalid JSON.
fn num(v: f64) -> Value {
    serde_json::Number::from_f64(v).map(Value::Number).unwrap_or(Value::Null)
}

/// 2D bounding box over every position in the given geometries, in RFC 7946
/// order: `[west, south, east, north]`.
fn bbox_of<'a, I: Iterator<Item = &'a Value>>(geometries: I) -> Option<Value> {
    let (mut minx, mut miny) = (f64::INFINITY, f64::INFINITY);
    let (mut maxx, mut maxy) = (f64::NEG_INFINITY, f64::NEG_INFINITY);
    let mut seen = false;
    for g in geometries {
        scan_positions(g, &mut |x, y| {
            seen = true;
            minx = minx.min(x);
            miny = miny.min(y);
            maxx = maxx.max(x);
            maxy = maxy.max(y);
        });
    }
    if !seen {
        return None;
    }
    Some(Value::Array(vec![num(minx), num(miny), num(maxx), num(maxy)]))
}

fn scan_positions(v: &Value, f: &mut impl FnMut(f64, f64)) {
    match v {
        Value::Array(a) => {
            // A position is an array whose first element is a number; anything
            // else is a nesting level (ring, polygon, part list).
            if a.first().map_or(false, Value::is_number) {
                if let (Some(x), Some(y)) = (a.first().and_then(Value::as_f64), a.get(1).and_then(Value::as_f64)) {
                    f(x, y);
                }
            } else {
                for e in a {
                    scan_positions(e, f);
                }
            }
        }
        Value::Object(o) => {
            if let Some(c) = o.get("coordinates") {
                scan_positions(c, f);
            }
            if let Some(g) = o.get("geometries") {
                scan_positions(g, f);
            }
        }
        _ => {}
    }
}

fn serialize(v: &Value, indent: usize) -> Result<String, String> {
    if indent == 0 {
        return serde_json::to_string(v).map_err(|e| format!("could not serialize GeoJSON: {e}"));
    }
    let pad = " ".repeat(indent);
    let fmt = serde_json::ser::PrettyFormatter::with_indent(pad.as_bytes());
    let mut buf = Vec::new();
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, fmt);
    v.serialize(&mut ser).map_err(|e| format!("could not serialize GeoJSON: {e}"))?;
    String::from_utf8(buf).map_err(|e| format!("could not serialize GeoJSON: {e}"))
}

/// Human-readable JSON kind, for error messages that say what was found.
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Two unit squares sharing their middle edge, quantized with
    /// scale [0.5, 0.5] / translate [10, 20]:
    ///   arc 0 = the shared edge (11,20) → (11,21)
    ///   arc 1 = the west square's outer path
    ///   arc 2 = the east square's outer path
    /// The east square reuses arc 0 backwards as `-1`.
    const SQUARES: &str = r#"{
      "type": "Topology",
      "transform": { "scale": [0.5, 0.5], "translate": [10, 20] },
      "objects": {
        "blocks": {
          "type": "GeometryCollection",
          "geometries": [
            { "type": "Polygon", "id": "a", "properties": { "name": "West" }, "arcs": [[0, 1]] },
            { "type": "Polygon", "id": "b", "properties": { "name": "East" }, "arcs": [[2, -1]] }
          ]
        }
      },
      "arcs": [
        [[2, 0], [0, 2]],
        [[2, 2], [-2, 0], [0, -2], [2, 0]],
        [[2, 0], [2, 0], [0, 2], [-2, 0]]
      ]
    }"#;

    fn opts(indent: usize) -> Options {
        Options { indent, ..Default::default() }
    }

    #[test]
    fn expands_shared_arcs_with_transform() {
        let out = topojson_to_geojson(SQUARES, &opts(0)).unwrap();
        assert_eq!(
            out,
            r#"{"type":"FeatureCollection","features":[{"type":"Feature","id":"a","properties":{"name":"West"},"geometry":{"type":"Polygon","coordinates":[[[11.0,20.0],[11.0,21.0],[10.0,21.0],[10.0,20.0],[11.0,20.0]]]}},{"type":"Feature","id":"b","properties":{"name":"East"},"geometry":{"type":"Polygon","coordinates":[[[11.0,20.0],[12.0,20.0],[12.0,21.0],[11.0,21.0],[11.0,20.0]]]}}]}"#
        );
    }

    #[test]
    fn reversed_arc_reference_runs_backwards() {
        // The east ring ends by walking the shared arc from (11,21) back to
        // (11,20) — the reversal is what closes the ring.
        let v: Value = serde_json::from_str(&topojson_to_geojson(SQUARES, &opts(0)).unwrap()).unwrap();
        let east = &v["features"][1]["geometry"]["coordinates"][0];
        assert_eq!(east[3], serde_json::json!([11.0, 21.0]));
        assert_eq!(east[4], serde_json::json!([11.0, 20.0]));
        assert_eq!(east[0], east[4], "ring is closed");
    }

    #[test]
    fn pretty_prints_by_default() {
        let out = topojson_to_geojson(SQUARES, &Options::default()).unwrap();
        assert!(out.starts_with("{\n  \"type\": \"FeatureCollection\","), "got {out}");
    }

    #[test]
    fn selects_a_single_named_object() {
        let src = r#"{"type":"Topology","objects":{
            "roads":{"type":"LineString","arcs":[0]},
            "rails":{"type":"LineString","arcs":[1]}},
            "arcs":[[[0,0],[1,1]],[[5,5],[6,6]]]}"#;
        let out = topojson_to_geojson(src, &Options { object: "rails".into(), indent: 0, ..Default::default() }).unwrap();
        assert_eq!(
            out,
            r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{},"geometry":{"type":"LineString","coordinates":[[5.0,5.0],[6.0,6.0]]}}]}"#
        );
    }

    #[test]
    fn merges_every_object_when_no_name_given() {
        let src = r#"{"type":"Topology","objects":{
            "roads":{"type":"LineString","arcs":[0]},
            "rails":{"type":"LineString","arcs":[1]}},
            "arcs":[[[0,0],[1,1]],[[5,5],[6,6]]]}"#;
        let v: Value = serde_json::from_str(&topojson_to_geojson(src, &opts(0)).unwrap()).unwrap();
        assert_eq!(v["features"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn untransformed_arcs_are_absolute_not_delta() {
        // No "transform" member: arc positions are already real coordinates.
        let src = r#"{"type":"Topology","objects":{"l":{"type":"LineString","arcs":[0]}},
            "arcs":[[[1,2],[3,4],[5,6]]]}"#;
        let v: Value = serde_json::from_str(&topojson_to_geojson(src, &opts(0)).unwrap()).unwrap();
        assert_eq!(
            v["features"][0]["geometry"]["coordinates"],
            serde_json::json!([[1.0, 2.0], [3.0, 4.0], [5.0, 6.0]])
        );
    }

    #[test]
    fn points_are_transformed_but_not_delta_decoded() {
        let src = r#"{"type":"Topology","transform":{"scale":[0.5,0.5],"translate":[10,20]},
            "objects":{"p":{"type":"GeometryCollection","geometries":[
              {"type":"Point","coordinates":[2,4]},
              {"type":"MultiPoint","coordinates":[[2,4],[6,8]]}]}},
            "arcs":[]}"#;
        let v: Value = serde_json::from_str(&topojson_to_geojson(src, &opts(0)).unwrap()).unwrap();
        assert_eq!(v["features"][0]["geometry"]["coordinates"], serde_json::json!([11.0, 22.0]));
        // Each MultiPoint position stands alone — no running total between them.
        assert_eq!(
            v["features"][1]["geometry"]["coordinates"],
            serde_json::json!([[11.0, 22.0], [13.0, 24.0]])
        );
    }

    #[test]
    fn handles_multipolygon_and_multilinestring() {
        let src = r#"{"type":"Topology","objects":{"g":{"type":"GeometryCollection","geometries":[
              {"type":"MultiLineString","arcs":[[0],[1]]},
              {"type":"MultiPolygon","arcs":[[[0,1]]]}]}},
            "arcs":[[[0,0],[0,1]],[[0,1],[1,1],[0,0]]]}"#;
        let v: Value = serde_json::from_str(&topojson_to_geojson(src, &opts(0)).unwrap()).unwrap();
        assert_eq!(v["features"][0]["geometry"]["type"], "MultiLineString");
        assert_eq!(v["features"][0]["geometry"]["coordinates"].as_array().unwrap().len(), 2);
        let poly = &v["features"][1]["geometry"]["coordinates"][0][0];
        assert_eq!(poly.as_array().unwrap().len(), 4, "ring stitched from both arcs");
    }

    #[test]
    fn preserves_id_properties_and_geometry_bbox() {
        let src = r#"{"type":"Topology","objects":{"g":{"type":"Point","id":7,
            "bbox":[1,2,1,2],"properties":{"z":"y","a":1},"coordinates":[1,2]}},"arcs":[]}"#;
        let out = topojson_to_geojson(src, &opts(0)).unwrap();
        // Key order (id, bbox, properties, geometry) and property order both survive.
        assert!(out.contains(r#""id":7,"bbox":[1,2,1,2],"properties":{"z":"y","a":1}"#), "got {out}");
    }

    #[test]
    fn missing_properties_become_an_empty_object() {
        let src = r#"{"type":"Topology","objects":{"g":{"type":"Point","coordinates":[1,2]}},"arcs":[]}"#;
        let out = topojson_to_geojson(src, &opts(0)).unwrap();
        assert!(out.contains(r#""properties":{}"#), "got {out}");
    }

    #[test]
    fn geometry_collection_output_drops_the_feature_wrapper() {
        let src = r#"{"type":"Topology","objects":{"g":{"type":"Point","properties":{"a":1},"coordinates":[1,2]}},"arcs":[]}"#;
        let out = topojson_to_geojson(
            src,
            &Options { output: Output::GeometryCollection, indent: 0, ..Default::default() },
        )
        .unwrap();
        assert_eq!(
            out,
            r#"{"type":"GeometryCollection","geometries":[{"type":"Point","coordinates":[1.0,2.0]}]}"#
        );
    }

    #[test]
    fn bbox_covers_only_the_emitted_coordinates() {
        let src = r#"{"type":"Topology","objects":{
            "a":{"type":"LineString","arcs":[0]},
            "b":{"type":"LineString","arcs":[1]}},
            "arcs":[[[0,0],[1,3]],[[100,100],[200,200]]]}"#;
        // Selecting one object must not inherit the other object's extent.
        let out = topojson_to_geojson(
            src,
            &Options { object: "a".into(), include_bbox: true, indent: 0, ..Default::default() },
        )
        .unwrap();
        assert!(out.contains(r#""bbox":[0.0,0.0,1.0,3.0]"#), "got {out}");
        assert!(!topojson_to_geojson(src, &opts(0)).unwrap().contains("bbox"), "off by default");
    }

    #[test]
    fn precision_rounds_away_quantization_noise() {
        // 0.1 * 3 + 0.0 is 0.30000000000000004 in binary floating point.
        let src = r#"{"type":"Topology","transform":{"scale":[0.1,0.1],"translate":[0,0]},
            "objects":{"p":{"type":"Point","coordinates":[3,3]}},"arcs":[]}"#;
        let raw = topojson_to_geojson(src, &opts(0)).unwrap();
        assert!(raw.contains("0.30000000000000004"), "got {raw}");
        let rounded =
            topojson_to_geojson(src, &Options { precision: 6, indent: 0, ..Default::default() }).unwrap();
        assert!(rounded.contains(r#""coordinates":[0.3,0.3]"#), "got {rounded}");
    }

    #[test]
    fn extra_dimensions_pass_through_unquantized() {
        let src = r#"{"type":"Topology","transform":{"scale":[0.5,0.5],"translate":[0,0]},
            "objects":{"p":{"type":"Point","coordinates":[2,4,999]}},"arcs":[]}"#;
        let out = topojson_to_geojson(src, &opts(0)).unwrap();
        assert!(out.contains(r#""coordinates":[1.0,2.0,999.0]"#), "got {out}");
    }

    #[test]
    fn null_type_geometry_becomes_a_null_geometry() {
        let src = r#"{"type":"Topology","objects":{"g":{"type":null,"properties":{"a":1}}},"arcs":[]}"#;
        let out = topojson_to_geojson(src, &opts(0)).unwrap();
        assert!(out.contains(r#""geometry":null"#), "got {out}");
    }

    #[test]
    fn rejects_an_out_of_range_arc_index() {
        let src = r#"{"type":"Topology","objects":{"g":{"type":"LineString","arcs":[5]}},
            "arcs":[[[0,0],[1,1]]]}"#;
        let err = topojson_to_geojson(src, &opts(0)).unwrap_err();
        assert!(err.contains("references arc 5") && err.contains("has 1 arc"), "got {err}");
    }

    #[test]
    fn rejects_an_unknown_geometry_type() {
        let src = r#"{"type":"Topology","objects":{"g":{"type":"Circle","arcs":[0]}},
            "arcs":[[[0,0],[1,1]]]}"#;
        let err = topojson_to_geojson(src, &opts(0)).unwrap_err();
        assert!(err.contains("unknown type \"Circle\""), "got {err}");
    }

    #[test]
    fn rejects_an_unknown_object_name_and_lists_the_real_ones() {
        let err = topojson_to_geojson(SQUARES, &Options { object: "nope".into(), ..Default::default() })
            .unwrap_err();
        assert!(err.contains("no object named \"nope\"") && err.contains("\"blocks\""), "got {err}");
    }

    #[test]
    fn rejects_geojson_input_with_a_pointed_message() {
        let err = topojson_to_geojson(r#"{"type":"FeatureCollection","features":[]}"#, &opts(0)).unwrap_err();
        assert!(err.contains("already GeoJSON"), "got {err}");
    }

    #[test]
    fn rejects_invalid_json_with_line_and_column() {
        let err = topojson_to_geojson("{\"type\": \"Topology\",}", &opts(0)).unwrap_err();
        assert!(err.starts_with("invalid JSON:") && err.contains("line"), "got {err}");
    }

    #[test]
    fn rejects_a_malformed_arc_position() {
        let src = r#"{"type":"Topology","objects":{"g":{"type":"LineString","arcs":[0]}},
            "arcs":[[[0,0],["x",1]]]}"#;
        let err = topojson_to_geojson(src, &opts(0)).unwrap_err();
        assert!(err.contains("arc 0 position 1") && err.contains("must be a number"), "got {err}");
    }

    #[test]
    fn rejects_a_missing_objects_member() {
        let err = topojson_to_geojson(r#"{"type":"Topology","arcs":[]}"#, &opts(0)).unwrap_err();
        assert!(err.contains("missing the \"objects\" member"), "got {err}");
    }

    #[test]
    fn output_parse_falls_back_to_feature_collection() {
        assert_eq!(Output::parse("geometry-collection"), Output::GeometryCollection);
        assert_eq!(Output::parse("GEOMETRY_COLLECTION"), Output::GeometryCollection);
        assert_eq!(Output::parse(""), Output::FeatureCollection);
        assert_eq!(Output::parse("wat"), Output::FeatureCollection);
    }
}
