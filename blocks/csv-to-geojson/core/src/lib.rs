//! csv-to-geojson core — pure compute, shared by the chat skill block and the web page.
//! Turns a CSV/TSV table (or a JSON array of objects) that carries latitude and
//! longitude columns into RFC 7946 GeoJSON: a FeatureCollection of Points, or a
//! single LineString/Polygon feature built from the rows in order.
//! No wafer/wasm-bindgen deps.

use serde_json::{Map, Value};

/// Hard cap on data rows (excluding the header). Keeps browser/chat runs bounded.
pub const MAX_ROWS: usize = 100_000;

/// Header names (normalized to lowercase alphanumerics) tried when `lat` is blank.
const LAT_NAMES: &[&str] = &[
    "latitude",
    "lat",
    "latdd",
    "latdeg",
    "latitudedeg",
    "latitudey",
    "ycoordinate",
    "ycoord",
    "northing",
    "y",
];
/// Header names tried when `lon` is blank.
const LON_NAMES: &[&str] = &[
    "longitude",
    "long",
    "lon",
    "lng",
    "londd",
    "londeg",
    "longitudedeg",
    "longitudex",
    "xcoordinate",
    "xcoord",
    "easting",
    "x",
];
/// Header names tried when `elevation` is blank.
const ELE_NAMES: &[&str] = &[
    "elevation",
    "altitude",
    "alt",
    "ele",
    "elev",
    "height",
    "zcoordinate",
    "z",
];

/// Loose prefixes used only after the exact-name pass fails; guarded by a
/// numeric/range sanity check so a column like `longname` is never picked.
const LAT_PREFIXES: &[&str] = &["latitude", "lat"];
const LON_PREFIXES: &[&str] = &["longitude", "long", "lon", "lng"];

/// Conversion settings. Every field mirrors one descriptor param.
#[derive(Debug, Clone)]
pub struct Options {
    /// Latitude column: header name or 1-based index; empty = auto-detect.
    pub lat: String,
    /// Longitude column: header name or 1-based index; empty = auto-detect.
    pub lon: String,
    /// Optional elevation column: header name or 1-based index; empty = auto-detect, `-` = off.
    pub elevation: String,
    /// `auto` | `comma` | `semicolon` | `tab` | `pipe`.
    pub delimiter: String,
    /// `points` | `line` | `polygon`.
    pub shape: String,
    /// `infer` | `string`.
    pub types: String,
    /// Coordinate decimal places; 0 = keep full precision.
    pub precision: i64,
    /// `skip` | `error` | `null`.
    pub invalid: String,
    /// Add an RFC 7946 `bbox` member.
    pub bbox: bool,
    /// Pretty-print with 2-space indent.
    pub pretty: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            lat: String::new(),
            lon: String::new(),
            elevation: String::new(),
            delimiter: "auto".into(),
            shape: "points".into(),
            types: "infer".into(),
            precision: 0,
            invalid: "skip".into(),
            bbox: false,
            pretty: true,
        }
    }
}

/// A row that produced a usable coordinate.
struct Point {
    lon: f64,
    lat: f64,
    ele: Option<f64>,
    props: Map<String, Value>,
}

/// Convert a CSV/TSV/JSON table into GeoJSON text.
pub fn convert(data: &str, opts: &Options) -> Result<String, String> {
    let shape = opts.shape.trim();
    if !matches!(shape, "points" | "line" | "polygon") {
        return Err(format!(
            "unknown shape '{shape}' — use 'points', 'line' or 'polygon'"
        ));
    }
    let types = opts.types.trim();
    if !matches!(types, "infer" | "string") {
        return Err(format!("unknown types '{types}' — use 'infer' or 'string'"));
    }
    let invalid = opts.invalid.trim();
    if !matches!(invalid, "skip" | "error" | "null") {
        return Err(format!(
            "unknown invalid '{invalid}' — use 'skip', 'error' or 'null'"
        ));
    }
    if !(0..=15).contains(&opts.precision) {
        return Err(format!(
            "precision must be between 0 and 15 (got {}); 0 keeps full precision",
            opts.precision
        ));
    }

    let (headers, rows) = parse_table(data, opts.delimiter.trim())?;
    if rows.len() > MAX_ROWS {
        return Err(format!("too many rows: {} (limit {MAX_ROWS})", rows.len()));
    }

    let lat_idx = resolve_column(
        &opts.lat,
        &headers,
        &rows,
        LAT_NAMES,
        LAT_PREFIXES,
        "latitude",
    )?
    .ok_or_else(|| missing_column_msg("latitude", "lat", &headers))?;
    let lon_idx = resolve_column(
        &opts.lon,
        &headers,
        &rows,
        LON_NAMES,
        LON_PREFIXES,
        "longitude",
    )?
    .ok_or_else(|| missing_column_msg("longitude", "lon", &headers))?;
    if lat_idx == lon_idx {
        return Err(format!(
            "latitude and longitude resolve to the same column '{}' — set lat and lon explicitly",
            headers[lat_idx]
        ));
    }
    let ele_idx = if opts.elevation.trim() == "-" {
        None
    } else {
        resolve_column(
            &opts.elevation,
            &headers,
            &rows,
            ELE_NAMES,
            &[],
            "elevation",
        )?
        .filter(|i| *i != lat_idx && *i != lon_idx)
    };

    let mut points: Vec<Point> = Vec::new();
    let mut null_rows: Vec<Map<String, Value>> = Vec::new();
    // Point features in original row order; `None` marks a null-geometry row.
    let mut order: Vec<Option<usize>> = Vec::new();

    for (i, row) in rows.iter().enumerate() {
        let row_no = i + 1;
        let props = build_props(
            &headers,
            row,
            &[Some(lat_idx), Some(lon_idx), ele_idx],
            types,
        );
        let lat_raw = row.get(lat_idx).unwrap_or(&Value::Null);
        let lon_raw = row.get(lon_idx).unwrap_or(&Value::Null);
        let lat_v = cell_to_f64(lat_raw);
        let lon_v = cell_to_f64(lon_raw);

        let bad = match (lat_v, lon_v) {
            (None, _) => Some(format!("latitude {} is not a number", quote_cell(lat_raw))),
            (_, None) => Some(format!("longitude {} is not a number", quote_cell(lon_raw))),
            (Some(la), Some(lo)) => {
                if !(-90.0..=90.0).contains(&la) {
                    Some(format!("latitude {la} is outside -90..90"))
                } else if !(-180.0..=180.0).contains(&lo) {
                    Some(format!("longitude {lo} is outside -180..180"))
                } else {
                    None
                }
            }
        };

        if let Some(why) = bad {
            match invalid {
                "error" => return Err(format!("row {row_no}: {why}")),
                "null" => {
                    order.push(None);
                    null_rows.push(props);
                }
                _ => {}
            }
            continue;
        }

        let ele = ele_idx
            .and_then(|i| row.get(i))
            .and_then(cell_to_f64)
            .map(|v| round_to(v, opts.precision));
        order.push(Some(points.len()));
        points.push(Point {
            lon: round_to(lon_v.unwrap(), opts.precision),
            lat: round_to(lat_v.unwrap(), opts.precision),
            ele,
            props,
        });
    }

    match shape {
        "points" => Ok(render(
            build_point_collection(&points, &order, &mut null_rows.into_iter(), opts.bbox),
            opts.pretty,
        )),
        _ => {
            let needed = if shape == "line" { 2 } else { 3 };
            if points.len() < needed {
                return Err(format!(
                    "{shape} needs at least {needed} rows with valid coordinates, got {}",
                    points.len()
                ));
            }
            Ok(render(
                build_joined_feature(&points, shape, opts.bbox),
                opts.pretty,
            ))
        }
    }
}

fn render(v: Value, pretty: bool) -> String {
    if pretty {
        serde_json::to_string_pretty(&v).unwrap_or_default()
    } else {
        serde_json::to_string(&v).unwrap_or_default()
    }
}

// ---------------------------------------------------------------- output ----

fn build_point_collection(
    points: &[Point],
    order: &[Option<usize>],
    nulls: &mut dyn Iterator<Item = Map<String, Value>>,
    want_bbox: bool,
) -> Value {
    let mut features = Vec::with_capacity(order.len());
    for slot in order {
        match slot {
            Some(i) => {
                let p = &points[*i];
                features.push(feature(point_geometry(p), p.props.clone()));
            }
            None => features.push(feature(Value::Null, nulls.next().unwrap_or_default())),
        }
    }
    let mut fc = Map::new();
    fc.insert("type".into(), Value::String("FeatureCollection".into()));
    if want_bbox {
        if let Some(b) = bbox_of(points) {
            fc.insert("bbox".into(), b);
        }
    }
    fc.insert("features".into(), Value::Array(features));
    Value::Object(fc)
}

fn build_joined_feature(points: &[Point], shape: &str, want_bbox: bool) -> Value {
    let mut coords: Vec<Value> = points.iter().map(coord_of).collect();
    let geometry = if shape == "line" {
        let mut g = Map::new();
        g.insert("type".into(), Value::String("LineString".into()));
        g.insert("coordinates".into(), Value::Array(coords));
        Value::Object(g)
    } else {
        // RFC 7946: exterior rings wind counterclockwise and must be closed.
        if signed_area(points) > 0.0 {
            coords.reverse();
        }
        if coords.first() != coords.last() {
            coords.push(coords[0].clone());
        }
        let mut g = Map::new();
        g.insert("type".into(), Value::String("Polygon".into()));
        g.insert(
            "coordinates".into(),
            Value::Array(vec![Value::Array(coords)]),
        );
        Value::Object(g)
    };
    let mut f = Map::new();
    f.insert("type".into(), Value::String("Feature".into()));
    if want_bbox {
        if let Some(b) = bbox_of(points) {
            f.insert("bbox".into(), b);
        }
    }
    f.insert("geometry".into(), geometry);
    f.insert("properties".into(), Value::Object(shared_props(points)));
    Value::Object(f)
}

fn feature(geometry: Value, props: Map<String, Value>) -> Value {
    let mut f = Map::new();
    f.insert("type".into(), Value::String("Feature".into()));
    f.insert("geometry".into(), geometry);
    f.insert("properties".into(), Value::Object(props));
    Value::Object(f)
}

fn point_geometry(p: &Point) -> Value {
    let mut g = Map::new();
    g.insert("type".into(), Value::String("Point".into()));
    g.insert("coordinates".into(), coord_of(p));
    Value::Object(g)
}

fn coord_of(p: &Point) -> Value {
    let mut c = vec![num(p.lon), num(p.lat)];
    if let Some(e) = p.ele {
        c.push(num(e));
    }
    Value::Array(c)
}

fn num(v: f64) -> Value {
    serde_json::Number::from_f64(v)
        .map(Value::Number)
        .unwrap_or(Value::Null)
}

fn bbox_of(points: &[Point]) -> Option<Value> {
    let first = points.first()?;
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (first.lon, first.lat, first.lon, first.lat);
    for p in points {
        min_x = min_x.min(p.lon);
        min_y = min_y.min(p.lat);
        max_x = max_x.max(p.lon);
        max_y = max_y.max(p.lat);
    }
    Some(Value::Array(vec![
        num(min_x),
        num(min_y),
        num(max_x),
        num(max_y),
    ]))
}

/// Shoelace sum; > 0 means the ring is wound clockwise.
fn signed_area(points: &[Point]) -> f64 {
    let mut sum = 0.0;
    for i in 0..points.len() {
        let a = &points[i];
        let b = &points[(i + 1) % points.len()];
        sum += (b.lon - a.lon) * (b.lat + a.lat);
    }
    sum
}

/// Properties present with an identical value in EVERY row (line/polygon mode).
fn shared_props(points: &[Point]) -> Map<String, Value> {
    let mut out = Map::new();
    let Some(first) = points.first() else {
        return out;
    };
    for (k, v) in &first.props {
        if points.iter().all(|p| p.props.get(k) == Some(v)) {
            out.insert(k.clone(), v.clone());
        }
    }
    out
}

// ----------------------------------------------------------------- input ----

fn parse_table(data: &str, delimiter: &str) -> Result<(Vec<String>, Vec<Vec<Value>>), String> {
    let trimmed = data.trim_start_matches('\u{feff}').trim();
    if trimmed.is_empty() {
        return Err(
            "input is empty — paste a CSV table with a header row, or a JSON array of objects"
                .into(),
        );
    }
    if trimmed.starts_with('[') || trimmed.starts_with('{') {
        return parse_json_table(trimmed);
    }
    parse_csv_table(data.trim_start_matches('\u{feff}'), delimiter)
}

fn parse_csv_table(data: &str, delimiter: &str) -> Result<(Vec<String>, Vec<Vec<Value>>), String> {
    let d = resolve_delimiter(data, delimiter)?;
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(d)
        .flexible(true)
        .has_headers(false)
        .from_reader(data.as_bytes());
    let mut records = rdr.records();
    let header_rec = match records.next() {
        Some(r) => r.map_err(|e| format!("could not read the header row: {e}"))?,
        None => return Err("input is empty — paste a CSV table with a header row".into()),
    };
    let headers: Vec<String> = header_rec
        .iter()
        .enumerate()
        .map(|(i, h)| {
            let h = h.trim_start_matches('\u{feff}').trim();
            if h.is_empty() {
                format!("column_{}", i + 1)
            } else {
                h.to_string()
            }
        })
        .collect();
    if headers.is_empty() {
        return Err("the header row has no columns".into());
    }

    let mut rows = Vec::new();
    for rec in records {
        let rec = rec.map_err(|e| format!("could not read row {}: {e}", rows.len() + 1))?;
        if rec.iter().all(|c| c.trim().is_empty()) {
            continue;
        }
        let mut row: Vec<Value> = rec
            .iter()
            .take(headers.len())
            .map(|c| Value::String(c.to_string()))
            .collect();
        row.resize(headers.len(), Value::String(String::new()));
        rows.push(row);
        if rows.len() > MAX_ROWS {
            return Err(format!("too many rows (limit {MAX_ROWS})"));
        }
    }
    Ok((headers, rows))
}

fn parse_json_table(text: &str) -> Result<(Vec<String>, Vec<Vec<Value>>), String> {
    let v: Value = serde_json::from_str(text).map_err(|e| format!("invalid JSON input: {e}"))?;
    let objects: Vec<Value> = match v {
        Value::Array(items) => items,
        Value::Object(map) => {
            // A wrapper object such as {"rows":[…]} — otherwise treat it as one row.
            let nested = map.values().find(|v| {
                v.as_array()
                    .is_some_and(|a| a.first().is_some_and(Value::is_object))
            });
            match nested {
                Some(Value::Array(items)) => items.clone(),
                _ => vec![Value::Object(map)],
            }
        }
        _ => return Err("JSON input must be an array of objects".into()),
    };

    let mut headers: Vec<String> = Vec::new();
    for item in &objects {
        let obj = item
            .as_object()
            .ok_or("JSON input must be an array of objects (one object per row)")?;
        for k in obj.keys() {
            if !headers.iter().any(|h| h == k) {
                headers.push(k.clone());
            }
        }
    }
    if headers.is_empty() {
        return Err("JSON input has no rows with fields".into());
    }
    let rows = objects
        .iter()
        .map(|item| {
            let obj = item.as_object().unwrap();
            headers
                .iter()
                .map(|h| obj.get(h).cloned().unwrap_or(Value::Null))
                .collect()
        })
        .collect();
    Ok((headers, rows))
}

fn resolve_delimiter(data: &str, delimiter: &str) -> Result<u8, String> {
    Ok(match delimiter {
        "comma" | "" => b',',
        "semicolon" => b';',
        "tab" => b'\t',
        "pipe" => b'|',
        "auto" => sniff_delimiter(data),
        other => {
            return Err(format!(
                "unknown delimiter '{other}' — use 'auto', 'comma', 'semicolon', 'tab' or 'pipe'"
            ))
        }
    })
}

/// Count candidate separators outside quotes on the first non-empty line.
fn sniff_delimiter(data: &str) -> u8 {
    let line = data.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    let mut in_quotes = false;
    let (mut comma, mut semi, mut tab, mut pipe) = (0u32, 0u32, 0u32, 0u32);
    for ch in line.chars() {
        match ch {
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => comma += 1,
            ';' if !in_quotes => semi += 1,
            '\t' if !in_quotes => tab += 1,
            '|' if !in_quotes => pipe += 1,
            _ => {}
        }
    }
    let best = comma.max(semi).max(tab).max(pipe);
    if best == 0 {
        b','
    } else if comma == best {
        b','
    } else if semi == best {
        b';'
    } else if tab == best {
        b'\t'
    } else {
        b'|'
    }
}

// --------------------------------------------------------------- columns ----

fn normalize(h: &str) -> String {
    h.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

fn missing_column_msg(kind: &str, param: &str, headers: &[String]) -> String {
    format!(
        "could not find a {kind} column — set {param} to a column name or 1-based index (headers: {})",
        headers.join(", ")
    )
}

/// Resolve one coordinate column from an explicit spec, else by auto-detection.
fn resolve_column(
    spec: &str,
    headers: &[String],
    rows: &[Vec<Value>],
    names: &[&str],
    prefixes: &[&str],
    kind: &str,
) -> Result<Option<usize>, String> {
    let spec = spec.trim();
    if !spec.is_empty() {
        if let Some(i) = headers.iter().position(|h| h.eq_ignore_ascii_case(spec)) {
            return Ok(Some(i));
        }
        let want = normalize(spec);
        if !want.is_empty() {
            if let Some(i) = headers.iter().position(|h| normalize(h) == want) {
                return Ok(Some(i));
            }
        }
        if let Ok(n) = spec.parse::<usize>() {
            if n >= 1 && n <= headers.len() {
                return Ok(Some(n - 1));
            }
            return Err(format!(
                "{kind} column index {n} is out of range 1..{}",
                headers.len()
            ));
        }
        return Err(format!(
            "{kind} column '{spec}' not found (headers: {})",
            headers.join(", ")
        ));
    }

    let normalized: Vec<String> = headers.iter().map(|h| normalize(h)).collect();
    for want in names {
        if let Some(i) = normalized.iter().position(|h| h == want) {
            return Ok(Some(i));
        }
    }
    // Loose prefix pass, only for columns that actually look like coordinates.
    for want in prefixes {
        for (i, h) in normalized.iter().enumerate() {
            if h.starts_with(want) && column_looks_numeric(rows, i) {
                return Ok(Some(i));
            }
        }
    }
    Ok(None)
}

/// True when the first few non-empty values of a column parse as finite numbers.
fn column_looks_numeric(rows: &[Vec<Value>], idx: usize) -> bool {
    let mut seen = 0;
    for row in rows {
        let Some(cell) = row.get(idx) else { continue };
        if is_blank(cell) {
            continue;
        }
        if cell_to_f64(cell).is_none() {
            return false;
        }
        seen += 1;
        if seen >= 20 {
            break;
        }
    }
    seen > 0
}

fn is_blank(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::String(s) => s.trim().is_empty(),
        _ => false,
    }
}

// ------------------------------------------------------------ cell values ----

fn quote_cell(v: &Value) -> String {
    match v {
        Value::String(s) if s.trim().is_empty() => "(empty)".into(),
        Value::String(s) => format!("'{s}'"),
        Value::Null => "(empty)".into(),
        other => format!("'{other}'"),
    }
}

/// Parse a coordinate cell. Accepts plain decimals and the European
/// comma-decimal form (`52,37`) used by semicolon-separated exports.
fn cell_to_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64().filter(|f| f.is_finite()),
        Value::String(s) => {
            let s = s.trim().trim_matches('"').trim();
            if s.is_empty() {
                return None;
            }
            if let Ok(f) = s.parse::<f64>() {
                return if f.is_finite() { Some(f) } else { None };
            }
            if s.matches(',').count() == 1 && !s.contains('.') {
                let swapped = s.replace(',', ".");
                if let Ok(f) = swapped.parse::<f64>() {
                    return if f.is_finite() { Some(f) } else { None };
                }
            }
            None
        }
        _ => None,
    }
}

fn round_to(v: f64, precision: i64) -> f64 {
    if precision <= 0 {
        return v;
    }
    let factor = 10f64.powi(precision as i32);
    let r = (v * factor).round() / factor;
    if r.is_finite() {
        r
    } else {
        v
    }
}

/// Build the properties object for one row, dropping the coordinate columns.
fn build_props(
    headers: &[String],
    row: &[Value],
    skip: &[Option<usize>],
    types: &str,
) -> Map<String, Value> {
    let mut props = Map::new();
    for (i, header) in headers.iter().enumerate() {
        if skip.iter().any(|s| *s == Some(i)) {
            continue;
        }
        let raw = row.get(i).cloned().unwrap_or(Value::Null);
        props.insert(header.clone(), coerce(raw, types));
    }
    props
}

fn coerce(v: Value, types: &str) -> Value {
    match (types, v) {
        ("infer", Value::String(s)) => infer_scalar(&s),
        ("string", Value::String(s)) => Value::String(s),
        ("string", Value::Null) => Value::Null,
        ("string", other) => Value::String(match &other {
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            _ => other.to_string(),
        }),
        (_, other) => other,
    }
}

/// CSV cells are all strings; recover numbers, booleans and empties.
/// Strings that would lose information when re-serialized (leading zeros,
/// leading `+`, thousands separators) deliberately stay strings.
fn infer_scalar(s: &str) -> Value {
    let t = s.trim();
    if t.is_empty() {
        return Value::Null;
    }
    if t.eq_ignore_ascii_case("true") {
        return Value::Bool(true);
    }
    if t.eq_ignore_ascii_case("false") {
        return Value::Bool(false);
    }
    if t != s || !looks_numeric(t) {
        return Value::String(s.to_string());
    }
    if let Ok(i) = t.parse::<i64>() {
        return Value::Number(i.into());
    }
    match t.parse::<f64>() {
        Ok(f) if f.is_finite() => num(f),
        _ => Value::String(s.to_string()),
    }
}

/// Conservative JSON-number shape check: `-?(0|[1-9]\d*)(\.\d+)?([eE][+-]?\d+)?`.
fn looks_numeric(t: &str) -> bool {
    let body = t.strip_prefix('-').unwrap_or(t);
    let (mantissa, exponent) = match body.split_once(['e', 'E']) {
        Some((m, e)) => (m, Some(e)),
        None => (body, None),
    };
    let (int_part, frac_part) = match mantissa.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (mantissa, None),
    };
    if int_part.is_empty() || !int_part.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    if int_part.len() > 1 && int_part.starts_with('0') {
        return false; // "007" is an identifier, not a number
    }
    if let Some(f) = frac_part {
        if f.is_empty() || !f.bytes().all(|b| b.is_ascii_digit()) {
            return false;
        }
    }
    if let Some(e) = exponent {
        let e = e.strip_prefix(['+', '-']).unwrap_or(e);
        if e.is_empty() || !e.bytes().all(|b| b.is_ascii_digit()) {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compact(data: &str, opts: &Options) -> String {
        let mut o = opts.clone();
        o.pretty = false;
        convert(data, &o).unwrap()
    }

    #[test]
    fn converts_a_basic_lat_lon_table() {
        let csv = "name,lat,lon\nAlpha,40,-105\nBeta,41.25,-106.5\n";
        let out = compact(csv, &Options::default());
        assert_eq!(
            out,
            r#"{"type":"FeatureCollection","features":[{"type":"Feature","geometry":{"type":"Point","coordinates":[-105.0,40.0]},"properties":{"name":"Alpha"}},{"type":"Feature","geometry":{"type":"Point","coordinates":[-106.5,41.25]},"properties":{"name":"Beta"}}]}"#
        );
    }

    #[test]
    fn auto_detects_common_header_spellings() {
        let csv = "City,Longitude,Latitude\nDenver,-105,39.7\n";
        let out = compact(csv, &Options::default());
        assert!(out.contains(r#""coordinates":[-105.0,39.7]"#), "{out}");
        assert!(out.contains(r#""City":"Denver""#), "{out}");
    }

    #[test]
    fn explicit_columns_by_name_and_index() {
        let csv = "a,b,c\n1,40,-105\n";
        let by_name = compact(
            csv,
            &Options {
                lat: "b".into(),
                lon: "c".into(),
                ..Default::default()
            },
        );
        let by_index = compact(
            csv,
            &Options {
                lat: "2".into(),
                lon: "3".into(),
                ..Default::default()
            },
        );
        assert_eq!(by_name, by_index);
        assert!(by_name.contains(r#""properties":{"a":1}"#), "{by_name}");
    }

    #[test]
    fn infers_property_types_but_keeps_leading_zeros() {
        let csv = "lat,lon,zip,pop,open\n40,-105,00501,1200,true\n";
        let out = compact(csv, &Options::default());
        assert!(out.contains(r#""zip":"00501""#), "{out}");
        assert!(out.contains(r#""pop":1200"#), "{out}");
        assert!(out.contains(r#""open":true"#), "{out}");
    }

    #[test]
    fn types_string_keeps_everything_as_text() {
        let csv = "lat,lon,pop\n40,-105,1200\n";
        let out = compact(
            csv,
            &Options {
                types: "string".into(),
                ..Default::default()
            },
        );
        assert!(out.contains(r#""pop":"1200""#), "{out}");
    }

    #[test]
    fn semicolon_input_with_comma_decimals() {
        let csv = "name;lat;lon\nOslo;59,91;10,75\n";
        let out = compact(csv, &Options::default());
        assert!(out.contains(r#""coordinates":[10.75,59.91]"#), "{out}");
    }

    #[test]
    fn tab_and_pipe_are_sniffed() {
        let tsv = "lat\tlon\tname\n40\t-105\tA\n";
        assert!(compact(tsv, &Options::default()).contains(r#""name":"A""#));
        let psv = "lat|lon|name\n40|-105|A\n";
        assert!(compact(psv, &Options::default()).contains(r#""name":"A""#));
    }

    #[test]
    fn elevation_becomes_the_third_ordinate() {
        let csv = "lat,lon,elevation\n40,-105,1600\n";
        let out = compact(csv, &Options::default());
        assert!(
            out.contains(r#""coordinates":[-105.0,40.0,1600.0]"#),
            "{out}"
        );
        let off = compact(
            csv,
            &Options {
                elevation: "-".into(),
                ..Default::default()
            },
        );
        assert!(off.contains(r#""coordinates":[-105.0,40.0]"#), "{off}");
        assert!(off.contains(r#""elevation":1600"#), "{off}");
    }

    #[test]
    fn precision_rounds_coordinates() {
        let csv = "lat,lon\n40.123456789,-105.987654321\n";
        let out = compact(
            csv,
            &Options {
                precision: 4,
                ..Default::default()
            },
        );
        assert!(
            out.contains(r#""coordinates":[-105.9877,40.1235]"#),
            "{out}"
        );
    }

    #[test]
    fn invalid_rows_skip_error_or_null() {
        let csv = "lat,lon,name\n40,-105,ok\n,,bad\n";
        let skipped = compact(csv, &Options::default());
        assert_eq!(
            skipped.matches(r#""type":"Feature""#).count(),
            1,
            "{skipped}"
        );

        let nulled = compact(
            csv,
            &Options {
                invalid: "null".into(),
                ..Default::default()
            },
        );
        assert!(nulled.contains(r#""geometry":null"#), "{nulled}");
        assert!(nulled.contains(r#""name":"bad""#), "{nulled}");

        let err = convert(
            csv,
            &Options {
                invalid: "error".into(),
                ..Default::default()
            },
        )
        .unwrap_err();
        assert_eq!(err, "row 2: latitude (empty) is not a number");
    }

    #[test]
    fn out_of_range_coordinates_are_reported() {
        let csv = "lat,lon\n95,-105\n";
        let err = convert(
            csv,
            &Options {
                invalid: "error".into(),
                ..Default::default()
            },
        )
        .unwrap_err();
        assert_eq!(err, "row 1: latitude 95 is outside -90..90");
    }

    #[test]
    fn line_shape_joins_rows_and_keeps_shared_properties() {
        let csv = "lat,lon,route,leg\n40,-105,R1,1\n41,-106,R1,2\n";
        let out = compact(
            csv,
            &Options {
                shape: "line".into(),
                ..Default::default()
            },
        );
        assert_eq!(
            out,
            r#"{"type":"Feature","geometry":{"type":"LineString","coordinates":[[-105.0,40.0],[-106.0,41.0]]},"properties":{"route":"R1"}}"#
        );
    }

    #[test]
    fn polygon_closes_the_ring_counterclockwise() {
        // Authored clockwise; the ring must come back counterclockwise + closed.
        let csv = "lat,lon\n0,0\n1,0\n1,1\n0,1\n";
        let out = compact(
            csv,
            &Options {
                shape: "polygon".into(),
                ..Default::default()
            },
        );
        assert_eq!(
            out,
            r#"{"type":"Feature","geometry":{"type":"Polygon","coordinates":[[[1.0,0.0],[1.0,1.0],[0.0,1.0],[0.0,0.0],[1.0,0.0]]]},"properties":{}}"#
        );
    }

    #[test]
    fn line_needs_two_valid_rows() {
        let csv = "lat,lon\n40,-105\n";
        let err = convert(
            csv,
            &Options {
                shape: "line".into(),
                ..Default::default()
            },
        )
        .unwrap_err();
        assert_eq!(
            err,
            "line needs at least 2 rows with valid coordinates, got 1"
        );
    }

    #[test]
    fn bbox_is_added_when_requested() {
        let csv = "lat,lon\n40,-105\n41,-106\n";
        let out = compact(
            csv,
            &Options {
                bbox: true,
                ..Default::default()
            },
        );
        assert!(
            out.starts_with(r#"{"type":"FeatureCollection","bbox":[-106.0,40.0,-105.0,41.0]"#),
            "{out}"
        );
    }

    #[test]
    fn json_array_input_is_accepted() {
        let json = r#"[{"name":"A","lat":40,"lon":-105},{"name":"B","lat":41,"lon":-106}]"#;
        let out = compact(json, &Options::default());
        assert!(out.contains(r#""coordinates":[-105.0,40.0]"#), "{out}");
        assert!(out.contains(r#""name":"B""#), "{out}");
    }

    #[test]
    fn quoted_fields_and_ragged_rows_survive() {
        let csv = "name,lat,lon\n\"Denver, CO\",39.7,-105\nShort,40\n";
        let out = compact(csv, &Options::default());
        assert!(out.contains(r#""name":"Denver, CO""#), "{out}");
        // The ragged row has no longitude, so it is skipped by default.
        assert_eq!(out.matches(r#""type":"Feature""#).count(), 1, "{out}");
    }

    #[test]
    fn pretty_output_is_indented() {
        let out = convert("lat,lon\n40,-105\n", &Options::default()).unwrap();
        assert!(
            out.starts_with("{\n  \"type\": \"FeatureCollection\""),
            "{out}"
        );
    }

    #[test]
    fn empty_input_errors() {
        let err = convert("   ", &Options::default()).unwrap_err();
        assert!(err.starts_with("input is empty"), "{err}");
    }

    #[test]
    fn missing_coordinate_columns_error_lists_headers() {
        let err = convert("name,population\nA,10\n", &Options::default()).unwrap_err();
        assert_eq!(
            err,
            "could not find a latitude column — set lat to a column name or 1-based index (headers: name, population)"
        );
    }

    #[test]
    fn unknown_column_name_errors() {
        let err = convert(
            "lat,lon\n40,-105\n",
            &Options {
                lat: "nope".into(),
                ..Default::default()
            },
        )
        .unwrap_err();
        assert_eq!(err, "latitude column 'nope' not found (headers: lat, lon)");
    }

    #[test]
    fn prefix_detection_ignores_non_numeric_lookalikes() {
        let csv = "longname,latitude,longitude\nfoo,40,-105\n";
        let out = compact(csv, &Options::default());
        assert!(out.contains(r#""coordinates":[-105.0,40.0]"#), "{out}");
        assert!(out.contains(r#""longname":"foo""#), "{out}");
    }

    #[test]
    fn bad_precision_is_rejected() {
        let err = convert(
            "lat,lon\n40,-105\n",
            &Options {
                precision: 42,
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(
            err.starts_with("precision must be between 0 and 15"),
            "{err}"
        );
    }
}
