//! gizza-ai/geofence-check core — test whether latitude/longitude points fall
//! inside a polygon (point-in-polygon). Pure-Rust (`serde_json` only), shared by
//! the chat block, the CLI, and the page.
//!
//! The polygon is parsed from GeoJSON (`Polygon`/`MultiPolygon`/`Feature`/
//! `FeatureCollection`, with interior rings = holes) or a simple ring given as
//! `lat,lon` CSV lines / a JSON array of coordinate pairs. Points are parsed from
//! CSV (`lat,lon[,label]`), a JSON array of pairs / `{lat,lon,label}` objects, or
//! GeoJSON Point/MultiPoint features. Containment uses the even-odd ray-casting
//! rule with explicit on-edge (boundary) detection.

use serde_json::{Map, Value};

/// Coordinate order for the *non-GeoJSON* input forms (CSV lines and JSON arrays
/// of numeric pairs). GeoJSON is always `[longitude, latitude]` per RFC 7946 and
/// ignores this setting.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CoordOrder {
    /// `lat,lon` — the first value is latitude (default).
    LatLon,
    /// `lon,lat` — the first value is longitude.
    LonLat,
}

impl CoordOrder {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "lat_lon" | "latlon" | "lat,lon" | "lat_lng" => Ok(CoordOrder::LatLon),
            "lon_lat" | "lonlat" | "lon,lat" | "lng_lat" => Ok(CoordOrder::LonLat),
            other => Err(format!(
                "invalid coord_order '{other}': expected 'lat_lon' or 'lon_lat'"
            )),
        }
    }
}

/// How a point that lies exactly on a polygon edge or vertex is classified.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Boundary {
    /// On-edge points count as `inside` (default).
    Inside,
    /// On-edge points count as `outside`.
    Outside,
    /// On-edge points get their own `boundary` status.
    Boundary,
}

impl Boundary {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "inside" | "in" => Ok(Boundary::Inside),
            "outside" | "out" => Ok(Boundary::Outside),
            "boundary" | "edge" | "on" => Ok(Boundary::Boundary),
            other => Err(format!(
                "invalid boundary '{other}': expected 'inside', 'outside' or 'boundary'"
            )),
        }
    }
}

/// Output serialization for the result table.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OutputFormat {
    Text,
    Csv,
    Json,
}

impl OutputFormat {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "text" | "txt" => Ok(OutputFormat::Text),
            "csv" => Ok(OutputFormat::Csv),
            "json" => Ok(OutputFormat::Json),
            other => Err(format!(
                "invalid output '{other}': expected 'text', 'csv' or 'json'"
            )),
        }
    }
}

/// A coordinate stored internally as `(x, y) = (longitude, latitude)`.
type Xy = (f64, f64);

/// A polygon: `rings[0]` is the outer ring, any further rings are holes.
type Polygon = Vec<Vec<Xy>>;

/// On-edge epsilon in degrees (~0.1 mm). Points within this of an edge are treated
/// as lying on the boundary.
const EPS: f64 = 1e-9;

/// One point to test, with an optional caller-supplied label.
struct Point {
    lon: f64,
    lat: f64,
    label: Option<String>,
}

/// The classification of a single point.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Status {
    Inside,
    Outside,
    OnBoundary,
}

impl Status {
    fn as_str(self) -> &'static str {
        match self {
            Status::Inside => "inside",
            Status::Outside => "outside",
            Status::OnBoundary => "boundary",
        }
    }
}

/// Validate a decimal-degree coordinate, with a hint when lat/lon look swapped.
fn validate_degrees(lon: f64, lat: f64) -> Result<(), String> {
    if !lon.is_finite() || !lat.is_finite() {
        return Err("coordinate values must be finite numbers".into());
    }
    if !(-90.0..=90.0).contains(&lat) {
        return Err(format!(
            "latitude {lat} is out of range (-90..90); if this looks like a longitude, \
             set coord_order to 'lon_lat'"
        ));
    }
    if !(-180.0..=180.0).contains(&lon) {
        return Err(format!(
            "longitude {lon} is out of range (-180..180); if this looks like a latitude, \
             set coord_order to 'lat_lon'"
        ));
    }
    Ok(())
}

/// Parse a numeric pair `[a, b]` honoring the coordinate order → `(lon, lat)`.
fn pair_to_xy(a: f64, b: f64, order: CoordOrder) -> (f64, f64) {
    match order {
        CoordOrder::LatLon => (b, a), // a=lat, b=lon
        CoordOrder::LonLat => (a, b), // a=lon, b=lat
    }
}

/// Read a GeoJSON coordinate array `[lon, lat, ...]` → `(lon, lat)`.
fn geojson_coord(v: &Value) -> Result<Xy, String> {
    let arr = v
        .as_array()
        .ok_or("invalid GeoJSON: a coordinate must be an array [lon, lat]")?;
    if arr.len() < 2 {
        return Err("invalid GeoJSON: a coordinate needs at least [lon, lat]".into());
    }
    let lon = arr[0]
        .as_f64()
        .ok_or("invalid GeoJSON: coordinate longitude is not a number")?;
    let lat = arr[1]
        .as_f64()
        .ok_or("invalid GeoJSON: coordinate latitude is not a number")?;
    Ok((lon, lat))
}

/// Split a GeoJSON `Polygon` coordinates value (array of rings) into rings.
fn geojson_polygon_rings(coords: &Value) -> Result<Polygon, String> {
    let rings = coords
        .as_array()
        .ok_or("invalid GeoJSON Polygon: coordinates must be an array of rings")?;
    if rings.is_empty() {
        return Err("invalid GeoJSON Polygon: no rings".into());
    }
    let mut out = Vec::with_capacity(rings.len());
    for ring in rings {
        let pts = ring
            .as_array()
            .ok_or("invalid GeoJSON Polygon: a ring must be an array of coordinates")?;
        let mut r = Vec::with_capacity(pts.len());
        for c in pts {
            r.push(geojson_coord(c)?);
        }
        out.push(close_ring(r));
    }
    Ok(out)
}

/// Ensure a ring is explicitly closed (first == last); a simple/CSV ring may omit it.
fn close_ring(mut r: Vec<Xy>) -> Vec<Xy> {
    if r.len() >= 2 && r[0] != r[r.len() - 1] {
        r.push(r[0]);
    }
    r
}

/// Pull polygons out of a parsed GeoJSON value (Polygon / MultiPolygon / Feature /
/// FeatureCollection / GeometryCollection). Non-polygon geometries are ignored.
fn geojson_polygons(root: &Value) -> Result<Vec<Polygon>, String> {
    let mut out = Vec::new();
    collect_polygons(root, &mut out)?;
    Ok(out)
}

fn collect_polygons(v: &Value, out: &mut Vec<Polygon>) -> Result<(), String> {
    let t = v
        .get("type")
        .and_then(Value::as_str)
        .ok_or("not GeoJSON: missing a top-level \"type\"")?;
    match t {
        "Polygon" => {
            let coords = v
                .get("coordinates")
                .ok_or("invalid GeoJSON Polygon: missing \"coordinates\"")?;
            out.push(geojson_polygon_rings(coords)?);
        }
        "MultiPolygon" => {
            let polys = v
                .get("coordinates")
                .and_then(Value::as_array)
                .ok_or("invalid GeoJSON MultiPolygon: missing \"coordinates\"")?;
            for p in polys {
                out.push(geojson_polygon_rings(p)?);
            }
        }
        "Feature" => {
            if let Some(g) = v.get("geometry") {
                if !g.is_null() {
                    collect_polygons(g, out)?;
                }
            }
        }
        "FeatureCollection" => {
            let feats = v
                .get("features")
                .and_then(Value::as_array)
                .ok_or("invalid GeoJSON FeatureCollection: missing a \"features\" array")?;
            for f in feats {
                collect_polygons(f, out)?;
            }
        }
        "GeometryCollection" => {
            let geoms = v
                .get("geometries")
                .and_then(Value::as_array)
                .ok_or("invalid GeoJSON GeometryCollection: missing \"geometries\"")?;
            for g in geoms {
                collect_polygons(g, out)?;
            }
        }
        // Non-area geometries carry no polygon; skip them.
        "Point" | "MultiPoint" | "LineString" | "MultiLineString" => {}
        other => return Err(format!("unsupported GeoJSON type '{other}' for a polygon")),
    }
    Ok(())
}

/// Parse the `polygon` input into one or more polygons.
fn parse_polygon(input: &str, order: CoordOrder) -> Result<Vec<Polygon>, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("polygon is empty".into());
    }
    // JSON object → GeoJSON.
    if trimmed.starts_with('{') {
        let v: Value = serde_json::from_str(trimmed).map_err(|e| format!("invalid JSON: {e}"))?;
        let polys = geojson_polygons(&v)?;
        if polys.is_empty() {
            return Err("the GeoJSON contains no Polygon or MultiPolygon".into());
        }
        return validate_polys(polys);
    }
    // JSON array → a single ring of [a,b] pairs.
    let ring = if trimmed.starts_with('[') {
        let v: Value = serde_json::from_str(trimmed).map_err(|e| format!("invalid JSON: {e}"))?;
        let arr = v
            .as_array()
            .ok_or("invalid polygon: expected an array of [lat,lon] pairs")?;
        let mut r = Vec::with_capacity(arr.len());
        for (i, c) in arr.iter().enumerate() {
            let pair = c.as_array().ok_or_else(|| {
                format!("invalid polygon vertex #{}: expected a [lat,lon] pair", i + 1)
            })?;
            if pair.len() < 2 {
                return Err(format!("invalid polygon vertex #{}: needs two numbers", i + 1));
            }
            let a = pair[0]
                .as_f64()
                .ok_or_else(|| format!("invalid polygon vertex #{}: not a number", i + 1))?;
            let b = pair[1]
                .as_f64()
                .ok_or_else(|| format!("invalid polygon vertex #{}: not a number", i + 1))?;
            r.push(pair_to_xy(a, b, order));
        }
        r
    } else {
        // CSV lines: `lat,lon` (or per coord_order) per line.
        parse_csv_ring(trimmed, order)?
    };
    if ring.len() < 3 {
        return Err("a polygon ring needs at least 3 vertices".into());
    }
    validate_polys(vec![vec![close_ring(ring)]])
}

/// Parse CSV/whitespace lines into a ring of `(lon, lat)` vertices.
fn parse_csv_ring(text: &str, order: CoordOrder) -> Result<Vec<Xy>, String> {
    let mut ring = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        match parse_coord_line(line, order) {
            Ok((lon, lat, _)) => ring.push((lon, lat)),
            Err(e) => {
                // Allow a header row as the very first non-empty line.
                if ring.is_empty() && i == 0 {
                    continue;
                }
                return Err(format!("polygon line {}: {e}", i + 1));
            }
        }
    }
    Ok(ring)
}

/// Parse one `a,b[,label]` line → `(lon, lat, label)` honoring the coord order.
/// Accepts comma, semicolon, tab, or whitespace as the field separator.
fn parse_coord_line(line: &str, order: CoordOrder) -> Result<(f64, f64, Option<String>), String> {
    let fields: Vec<&str> = if line.contains(',') {
        line.split(',').collect()
    } else if line.contains(';') {
        line.split(';').collect()
    } else if line.contains('\t') {
        line.split('\t').collect()
    } else {
        line.split_whitespace().collect()
    };
    if fields.len() < 2 {
        return Err("expected two numbers per line".into());
    }
    let a: f64 = fields[0]
        .trim()
        .parse()
        .map_err(|_| format!("'{}' is not a number", fields[0].trim()))?;
    let b: f64 = fields[1]
        .trim()
        .parse()
        .map_err(|_| format!("'{}' is not a number", fields[1].trim()))?;
    let (lon, lat) = pair_to_xy(a, b, order);
    let label = fields
        .get(2..)
        .map(|rest| rest.join(",").trim().to_string())
        .filter(|s| !s.is_empty());
    Ok((lon, lat, label))
}

/// Validate every vertex of every polygon is a sensible decimal-degree coordinate.
fn validate_polys(polys: Vec<Polygon>) -> Result<Vec<Polygon>, String> {
    for poly in &polys {
        for ring in poly {
            if ring.len() < 4 {
                return Err("a polygon ring needs at least 3 distinct vertices".into());
            }
            for &(lon, lat) in ring {
                validate_degrees(lon, lat)?;
            }
        }
    }
    Ok(polys)
}

/// Parse the `points` input into a list of points to test.
fn parse_points(input: &str, order: CoordOrder) -> Result<Vec<Point>, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("points is empty".into());
    }
    if trimmed.starts_with('{') {
        // A single GeoJSON object (Feature/FeatureCollection/geometry).
        let v: Value = serde_json::from_str(trimmed).map_err(|e| format!("invalid JSON: {e}"))?;
        let mut out = Vec::new();
        collect_geojson_points(&v, &mut out)?;
        if out.is_empty() {
            return Err("the GeoJSON contains no Point features".into());
        }
        return finalize_points(out);
    }
    if trimmed.starts_with('[') {
        let v: Value = serde_json::from_str(trimmed).map_err(|e| format!("invalid JSON: {e}"))?;
        let arr = v.as_array().ok_or("invalid points: expected a JSON array")?;
        let mut out = Vec::new();
        for (i, item) in arr.iter().enumerate() {
            out.push(json_point(item, order, i)?);
        }
        return finalize_points(out);
    }
    // CSV lines.
    let mut out = Vec::new();
    for (i, line) in trimmed.lines().enumerate() {
        let l = line.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        match parse_coord_line(l, order) {
            Ok((lon, lat, label)) => out.push(Point { lon, lat, label }),
            Err(e) => {
                if out.is_empty() && i == 0 {
                    continue; // header row
                }
                return Err(format!("points line {}: {e}", i + 1));
            }
        }
    }
    if out.is_empty() {
        return Err("no points found".into());
    }
    finalize_points(out)
}

/// Parse one JSON array element into a point: either a `[a,b]` pair or an object
/// with `lat`/`lon` (aliases `latitude`/`longitude`/`lng`) and optional `label`.
fn json_point(item: &Value, order: CoordOrder, idx: usize) -> Result<Point, String> {
    if let Some(pair) = item.as_array() {
        if pair.len() < 2 {
            return Err(format!("point #{}: expected a [lat,lon] pair", idx + 1));
        }
        let a = pair[0]
            .as_f64()
            .ok_or_else(|| format!("point #{}: not a number", idx + 1))?;
        let b = pair[1]
            .as_f64()
            .ok_or_else(|| format!("point #{}: not a number", idx + 1))?;
        let (lon, lat) = pair_to_xy(a, b, order);
        return Ok(Point {
            lon,
            lat,
            label: None,
        });
    }
    if let Some(obj) = item.as_object() {
        // A GeoJSON Feature/Point object embedded in the array.
        if obj.contains_key("type") {
            let mut out = Vec::new();
            collect_geojson_points(item, &mut out)?;
            if out.len() != 1 {
                return Err(format!(
                    "point #{}: a GeoJSON element must be a single Point",
                    idx + 1
                ));
            }
            return Ok(out.pop().unwrap());
        }
        let lat = obj_num(obj, &["lat", "latitude"])
            .ok_or_else(|| format!("point #{}: missing a lat/latitude field", idx + 1))?;
        let lon = obj_num(obj, &["lon", "lng", "long", "longitude"])
            .ok_or_else(|| format!("point #{}: missing a lon/longitude field", idx + 1))?;
        let label = obj
            .get("label")
            .or_else(|| obj.get("name"))
            .and_then(Value::as_str)
            .map(|s| s.to_string());
        return Ok(Point { lon, lat, label });
    }
    Err(format!("point #{}: expected a pair or an object", idx + 1))
}

fn obj_num(obj: &Map<String, Value>, keys: &[&str]) -> Option<f64> {
    keys.iter().find_map(|k| obj.get(*k).and_then(Value::as_f64))
}

/// Walk a GeoJSON value collecting every Point / MultiPoint position.
fn collect_geojson_points(v: &Value, out: &mut Vec<Point>) -> Result<(), String> {
    let t = v
        .get("type")
        .and_then(Value::as_str)
        .ok_or("not GeoJSON: missing a top-level \"type\"")?;
    match t {
        "Point" => {
            let (lon, lat) = geojson_coord(
                v.get("coordinates")
                    .ok_or("invalid GeoJSON Point: missing \"coordinates\"")?,
            )?;
            out.push(Point {
                lon,
                lat,
                label: None,
            });
        }
        "MultiPoint" => {
            let coords = v
                .get("coordinates")
                .and_then(Value::as_array)
                .ok_or("invalid GeoJSON MultiPoint: missing \"coordinates\"")?;
            for c in coords {
                let (lon, lat) = geojson_coord(c)?;
                out.push(Point {
                    lon,
                    lat,
                    label: None,
                });
            }
        }
        "Feature" => {
            let label = v
                .get("properties")
                .and_then(|p| p.get("name").or_else(|| p.get("label")))
                .and_then(Value::as_str)
                .map(|s| s.to_string());
            let before = out.len();
            if let Some(g) = v.get("geometry") {
                if !g.is_null() {
                    collect_geojson_points(g, out)?;
                }
            }
            // Attach the feature's name to a single produced point.
            if let Some(lbl) = label {
                if out.len() == before + 1 {
                    out[before].label = Some(lbl);
                }
            }
        }
        "FeatureCollection" => {
            let feats = v
                .get("features")
                .and_then(Value::as_array)
                .ok_or("invalid GeoJSON FeatureCollection: missing a \"features\" array")?;
            for f in feats {
                collect_geojson_points(f, out)?;
            }
        }
        "GeometryCollection" => {
            let geoms = v
                .get("geometries")
                .and_then(Value::as_array)
                .ok_or("invalid GeoJSON GeometryCollection: missing \"geometries\"")?;
            for g in geoms {
                collect_geojson_points(g, out)?;
            }
        }
        other => return Err(format!("GeoJSON type '{other}' is not a point")),
    }
    Ok(())
}

/// Validate every point's coordinates are sane decimal degrees.
fn finalize_points(points: Vec<Point>) -> Result<Vec<Point>, String> {
    for p in &points {
        validate_degrees(p.lon, p.lat)?;
    }
    Ok(points)
}

/// Is `pt` on the segment `a`–`b` (within EPS)?
fn on_segment(pt: Xy, a: Xy, b: Xy) -> bool {
    let (px, py) = pt;
    let (ax, ay) = a;
    let (bx, by) = b;
    let dx = bx - ax;
    let dy = by - ay;
    let seg_len = (dx * dx + dy * dy).sqrt();
    if seg_len == 0.0 {
        // Degenerate edge: on it iff at the vertex.
        return ((px - ax).powi(2) + (py - ay).powi(2)).sqrt() <= EPS;
    }
    // Perpendicular distance from the (infinite) line.
    let cross = dx * (py - ay) - dy * (px - ax);
    if (cross.abs() / seg_len) > EPS {
        return false;
    }
    // Projection parameter must fall within [0, 1] (± EPS tolerance).
    let dot = (px - ax) * dx + (py - ay) * dy;
    dot >= -EPS * seg_len && dot <= seg_len * seg_len + EPS * seg_len
}

/// True if `pt` lies on any edge of any ring of `poly`.
fn on_polygon_boundary(pt: Xy, poly: &Polygon) -> bool {
    for ring in poly {
        for w in ring.windows(2) {
            if on_segment(pt, w[0], w[1]) {
                return true;
            }
        }
    }
    false
}

/// Even-odd ray cast over one ring: true if the ray from `pt` crosses an odd
/// number of the ring's edges.
fn ray_crosses_ring(pt: Xy, ring: &[Xy]) -> bool {
    let (px, py) = pt;
    let mut inside = false;
    for w in ring.windows(2) {
        let (xi, yi) = w[0];
        let (xj, yj) = w[1];
        if ((yi > py) != (yj > py)) && (px < (xj - xi) * (py - yi) / (yj - yi) + xi) {
            inside = !inside;
        }
    }
    inside
}

/// Is `pt` strictly inside `poly` (outer ring minus holes) by even-odd parity?
fn inside_polygon(pt: Xy, poly: &Polygon) -> bool {
    poly.iter()
        .fold(false, |acc, ring| acc ^ ray_crosses_ring(pt, ring))
}

/// Classify one point against every polygon.
fn classify(pt: Xy, polys: &[Polygon], boundary: Boundary) -> Status {
    let on_edge = polys.iter().any(|p| on_polygon_boundary(pt, p));
    if on_edge {
        return match boundary {
            Boundary::Inside => Status::Inside,
            Boundary::Outside => Status::Outside,
            Boundary::Boundary => Status::OnBoundary,
        };
    }
    let inside = polys.iter().any(|p| inside_polygon(pt, p));
    if inside {
        Status::Inside
    } else {
        Status::Outside
    }
}

/// Trim a float for display: integers print without a trailing `.0`.
fn fmt_num(n: f64) -> String {
    if n == n.trunc() && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

/// Escape one CSV field per RFC 4180.
fn csv_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

struct Classified {
    point: Point,
    status: Status,
}

/// Run the geofence check end to end and render the requested output.
pub fn check(
    polygon: &str,
    points: &str,
    coord_order: &str,
    boundary: &str,
    output: &str,
) -> Result<String, String> {
    let order = CoordOrder::parse(coord_order)?;
    let bmode = Boundary::parse(boundary)?;
    let fmt = OutputFormat::parse(output)?;

    let polys = parse_polygon(polygon, order)?;
    let pts = parse_points(points, order)?;

    let results: Vec<Classified> = pts
        .into_iter()
        .map(|p| {
            let status = classify((p.lon, p.lat), &polys, bmode);
            Classified { point: p, status }
        })
        .collect();

    let total = results.len();
    let inside = results.iter().filter(|r| r.status == Status::Inside).count();
    let outside = results
        .iter()
        .filter(|r| r.status == Status::Outside)
        .count();
    let on_boundary = results
        .iter()
        .filter(|r| r.status == Status::OnBoundary)
        .count();
    let has_label = results.iter().any(|r| r.point.label.is_some());

    Ok(match fmt {
        OutputFormat::Text => render_text(&results, total, inside, outside, on_boundary, bmode),
        OutputFormat::Csv => render_csv(&results, has_label),
        OutputFormat::Json => render_json(&results, total, inside, outside, on_boundary, has_label),
    })
}

fn render_text(
    results: &[Classified],
    total: usize,
    inside: usize,
    outside: usize,
    on_boundary: usize,
    bmode: Boundary,
) -> String {
    let mut out = String::new();
    let noun = if total == 1 { "point" } else { "points" };
    if bmode == Boundary::Boundary {
        out.push_str(&format!(
            "{total} {noun}: {inside} inside, {outside} outside, {on_boundary} on boundary\n"
        ));
    } else {
        out.push_str(&format!("{total} {noun}: {inside} inside, {outside} outside\n"));
    }
    for (i, r) in results.iter().enumerate() {
        let coord = format!("{}, {}", fmt_num(r.point.lat), fmt_num(r.point.lon));
        let label = match &r.point.label {
            Some(l) => format!(" ({l})"),
            None => String::new(),
        };
        out.push_str(&format!(
            "#{}  {}{}  {}\n",
            i + 1,
            coord,
            label,
            r.status.as_str()
        ));
    }
    out.trim_end().to_string()
}

fn render_csv(results: &[Classified], has_label: bool) -> String {
    let mut out = String::new();
    if has_label {
        out.push_str("point,latitude,longitude,label,status\n");
    } else {
        out.push_str("point,latitude,longitude,status\n");
    }
    for (i, r) in results.iter().enumerate() {
        if has_label {
            out.push_str(&format!(
                "{},{},{},{},{}\n",
                i + 1,
                fmt_num(r.point.lat),
                fmt_num(r.point.lon),
                csv_field(r.point.label.as_deref().unwrap_or("")),
                r.status.as_str()
            ));
        } else {
            out.push_str(&format!(
                "{},{},{},{}\n",
                i + 1,
                fmt_num(r.point.lat),
                fmt_num(r.point.lon),
                r.status.as_str()
            ));
        }
    }
    out.trim_end().to_string()
}

fn render_json(
    results: &[Classified],
    total: usize,
    inside: usize,
    outside: usize,
    on_boundary: usize,
    has_label: bool,
) -> String {
    let points: Vec<Value> = results
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let mut m = Map::new();
            m.insert("point".into(), Value::from(i + 1));
            m.insert("latitude".into(), json_num(r.point.lat));
            m.insert("longitude".into(), json_num(r.point.lon));
            if has_label {
                m.insert(
                    "label".into(),
                    match &r.point.label {
                        Some(l) => Value::from(l.clone()),
                        None => Value::Null,
                    },
                );
            }
            m.insert("status".into(), Value::from(r.status.as_str()));
            Value::Object(m)
        })
        .collect();
    let mut summary = Map::new();
    summary.insert("total".into(), Value::from(total));
    summary.insert("inside".into(), Value::from(inside));
    summary.insert("outside".into(), Value::from(outside));
    summary.insert("boundary".into(), Value::from(on_boundary));
    let mut root = Map::new();
    root.insert("summary".into(), Value::Object(summary));
    root.insert("points".into(), Value::Array(points));
    serde_json::to_string_pretty(&Value::Object(root)).unwrap()
}

/// Represent a coordinate as an integer when it is whole, else a float.
fn json_num(n: f64) -> Value {
    if n == n.trunc() && n.abs() < 1e15 {
        Value::from(n as i64)
    } else {
        Value::from(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A square with corners (0,0)-(10,0)-(10,10)-(0,10) in lon/lat.
    const SQUARE: &str = "0,0\n10,0\n10,10\n0,10";

    #[test]
    fn point_inside_square() {
        // coord_order lat_lon: "5,5" = lat 5, lon 5 → inside.
        let out = check(SQUARE, "5,5", "lat_lon", "inside", "text").unwrap();
        assert!(out.contains("1 point: 1 inside, 0 outside"), "{out}");
        assert!(out.ends_with("inside"), "{out}");
    }

    #[test]
    fn point_outside_square() {
        let out = check(SQUARE, "20,20", "lat_lon", "inside", "text").unwrap();
        assert!(out.contains("0 inside, 1 outside"), "{out}");
        assert!(out.trim_end().ends_with("outside"), "{out}");
    }

    #[test]
    fn boundary_modes() {
        // Point at lat 0, lon 5 lies on the bottom edge of the square.
        let inside = check(SQUARE, "0,5", "lat_lon", "inside", "text").unwrap();
        assert!(inside.trim_end().ends_with("inside"), "{inside}");
        let outside = check(SQUARE, "0,5", "lat_lon", "outside", "text").unwrap();
        assert!(outside.trim_end().ends_with("outside"), "{outside}");
        let edge = check(SQUARE, "0,5", "lat_lon", "boundary", "text").unwrap();
        assert!(edge.trim_end().ends_with("boundary"), "{edge}");
        assert!(edge.contains("1 on boundary"), "{edge}");
    }

    #[test]
    fn geojson_polygon_with_hole() {
        // Outer 0..10 square with a hole 3..7. A point at (5,5) sits in the hole → outside.
        let poly = r#"{"type":"Polygon","coordinates":[
            [[0,0],[10,0],[10,10],[0,10],[0,0]],
            [[3,3],[7,3],[7,7],[3,7],[3,3]]
        ]}"#;
        let in_hole = check(poly, "5,5", "lat_lon", "inside", "text").unwrap();
        assert!(in_hole.trim_end().ends_with("outside"), "{in_hole}");
        // A point between the outer ring and the hole is inside.
        let in_ring = check(poly, "1,1", "lat_lon", "inside", "text").unwrap();
        assert!(in_ring.trim_end().ends_with("inside"), "{in_ring}");
    }

    #[test]
    fn coord_order_lon_lat() {
        // With lon_lat, "5,20" = lon 5, lat 20 → outside the 0..10 square.
        let out = check(SQUARE, "5,20", "lon_lat", "inside", "text").unwrap();
        assert!(out.trim_end().ends_with("outside"), "{out}");
        // "5,5" is inside regardless of order here (symmetric).
        let inside = check(SQUARE, "5,5", "lon_lat", "inside", "text").unwrap();
        assert!(inside.trim_end().ends_with("inside"), "{inside}");
    }

    #[test]
    fn geojson_points_use_lonlat() {
        // GeoJSON is [lon, lat] always. Point at lon 5, lat 5 → inside.
        let pts = r#"{"type":"Point","coordinates":[5,5]}"#;
        let out = check(SQUARE, pts, "lat_lon", "inside", "text").unwrap();
        assert!(out.trim_end().ends_with("inside"), "{out}");
    }

    #[test]
    fn csv_output_and_labels() {
        let pts = "5,5,Depot\n20,20,Far";
        let out = check(SQUARE, pts, "lat_lon", "inside", "csv").unwrap();
        assert_eq!(
            out,
            "point,latitude,longitude,label,status\n1,5,5,Depot,inside\n2,20,20,Far,outside"
        );
    }

    #[test]
    fn json_output_shape() {
        let out = check(SQUARE, "5,5", "lat_lon", "inside", "json").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["summary"]["total"], 1);
        assert_eq!(v["summary"]["inside"], 1);
        assert_eq!(v["points"][0]["status"], "inside");
        assert_eq!(v["points"][0]["latitude"], 5);
    }

    #[test]
    fn multipolygon_union() {
        let poly = r#"{"type":"MultiPolygon","coordinates":[
            [[[0,0],[2,0],[2,2],[0,2],[0,0]]],
            [[[10,10],[12,10],[12,12],[10,12],[10,10]]]
        ]}"#;
        // (11,11) is inside the second polygon.
        let out = check(poly, "11,11", "lat_lon", "inside", "text").unwrap();
        assert!(out.trim_end().ends_with("inside"), "{out}");
        // (5,5) is in neither.
        let out2 = check(poly, "5,5", "lat_lon", "inside", "text").unwrap();
        assert!(out2.trim_end().ends_with("outside"), "{out2}");
    }

    #[test]
    fn err_swapped_latitude() {
        // lat 200 is out of range → helpful error.
        let err = check(SQUARE, "200,5", "lat_lon", "inside", "text").unwrap_err();
        assert!(err.contains("out of range"), "{err}");
    }

    #[test]
    fn err_empty_polygon() {
        let err = check("", "5,5", "lat_lon", "inside", "text").unwrap_err();
        assert!(err.contains("polygon is empty"), "{err}");
    }

    #[test]
    fn err_too_few_vertices() {
        let err = check("0,0\n1,1", "5,5", "lat_lon", "inside", "text").unwrap_err();
        assert!(err.contains("at least 3"), "{err}");
    }

    #[test]
    fn err_bad_output() {
        let err = check(SQUARE, "5,5", "lat_lon", "inside", "xml").unwrap_err();
        assert!(err.contains("invalid output"), "{err}");
    }

    #[test]
    fn json_array_of_pairs() {
        let out = check(SQUARE, "[[5,5],[20,20]]", "lat_lon", "inside", "text").unwrap();
        assert!(out.contains("2 points: 1 inside, 1 outside"), "{out}");
    }

    #[test]
    fn simple_polygon_ring_auto_closes() {
        // A triangle given as lat,lon lines with no explicit closing vertex.
        let tri = "0,0\n0,10\n10,5";
        let out = check(tri, "1,5", "lat_lon", "inside", "text").unwrap();
        assert!(out.trim_end().ends_with("inside"), "{out}");
    }
}
