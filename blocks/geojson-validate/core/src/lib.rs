//! geojson-validate core — pure compute, shared by the chat skill block and the web page.
//! No wafer/wasm-bindgen deps.
//!
//! Walks one GeoJSON document and reports EVERY problem it finds instead of
//! stopping at the first one: structural/type errors with a JSON path, polygon
//! ring winding against the RFC 7946 right-hand rule, coordinate ranges and
//! non-numeric positions, ring closure and minimum ring size, bbox shape, plus
//! an optional family of "valid but problematic" warnings (duplicate nodes,
//! single-element multi-geometries, antimeridian crossings, excess precision).
//!
//! Malformed input is a FINDING, not a failure: invalid JSON or a bad document
//! type produces an INVALID report. `Err` is reserved for unusable option values.

use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

/// The seven geometry types RFC 7946 defines.
pub const GEOMETRY_TYPES: [&str; 7] = [
    "Point",
    "MultiPoint",
    "LineString",
    "MultiLineString",
    "Polygon",
    "MultiPolygon",
    "GeometryCollection",
];

// ------------------------------------------------------------------- options

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputFormat {
    Text,
    Json,
}

impl OutputFormat {
    fn parse(raw: &str) -> Result<Self, String> {
        match raw.trim() {
            "text" => Ok(OutputFormat::Text),
            "json" => Ok(OutputFormat::Json),
            other => Err(format!(
                "unknown output \"{other}\" (expected \"text\" or \"json\")"
            )),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub output: OutputFormat,
    /// Treat right-hand-rule violations as errors (true) or warnings (false).
    pub strict_winding: bool,
    /// Allow `bbox` members (still shape-checked); false reports every one.
    pub allow_bbox: bool,
    /// Emit the "valid but problematic" warning family.
    pub warn_problematic: bool,
    /// Warn above this many coordinate decimal places; -1 disables the check.
    pub max_precision: i64,
    /// Cap on issues LISTED per severity (counts always stay complete).
    pub max_issues: i64,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            output: OutputFormat::Text,
            strict_winding: true,
            allow_bbox: true,
            warn_problematic: true,
            max_precision: 6,
            max_issues: 50,
        }
    }
}

impl Options {
    /// Parse the raw surface values (chat/CLI/page all arrive as loose strings
    /// or already-typed scalars), so option validation lives in one place.
    pub fn parse(
        output: &str,
        strict_winding: bool,
        allow_bbox: bool,
        warn_problematic: bool,
        max_precision: i64,
        max_issues: i64,
    ) -> Result<Self, String> {
        if !(-1..=15).contains(&max_precision) {
            return Err(format!(
                "max_precision must be between -1 and 15, got {max_precision} (-1 disables the precision check)"
            ));
        }
        if !(1..=1000).contains(&max_issues) {
            return Err(format!(
                "max_issues must be between 1 and 1000, got {max_issues}"
            ));
        }
        Ok(Options {
            output: OutputFormat::parse(output)?,
            strict_winding,
            allow_bbox,
            warn_problematic,
            max_precision,
            max_issues,
        })
    }
}

// -------------------------------------------------------------------- report

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Issue {
    pub severity: Severity,
    /// Stable kebab-case rule id, e.g. `ring-not-closed` — safe to grep for.
    pub rule: &'static str,
    /// JSON path into the document, e.g. `features[0].geometry.coordinates[0]`.
    pub path: String,
    pub message: String,
}

#[derive(Clone, Debug, Default)]
pub struct Counts {
    pub features: usize,
    pub geometries: usize,
    pub by_type: BTreeMap<String, usize>,
    pub positions: usize,
    pub rings: usize,
    pub exterior_rings: usize,
    pub interior_rings: usize,
    pub bbox_members: usize,
    pub min_lon: Option<f64>,
    pub max_lon: Option<f64>,
    pub min_lat: Option<f64>,
    pub max_lat: Option<f64>,
}

#[derive(Clone, Debug)]
pub struct Report {
    pub document_type: Option<String>,
    pub issues: Vec<Issue>,
    pub counts: Counts,
}

impl Report {
    pub fn errors(&self) -> impl Iterator<Item = &Issue> {
        self.issues.iter().filter(|i| i.severity == Severity::Error)
    }
    pub fn warnings(&self) -> impl Iterator<Item = &Issue> {
        self.issues
            .iter()
            .filter(|i| i.severity == Severity::Warning)
    }
    pub fn error_count(&self) -> usize {
        self.errors().count()
    }
    pub fn warning_count(&self) -> usize {
        self.warnings().count()
    }
    pub fn valid(&self) -> bool {
        self.error_count() == 0
    }
}

// ------------------------------------------------------------------ walker

struct Ctx<'a> {
    opts: &'a Options,
    issues: Vec<Issue>,
    counts: Counts,
}

impl<'a> Ctx<'a> {
    fn err(&mut self, rule: &'static str, path: &str, message: impl Into<String>) {
        self.issues.push(Issue {
            severity: Severity::Error,
            rule,
            path: path.to_string(),
            message: message.into(),
        });
    }
    fn warn(&mut self, rule: &'static str, path: &str, message: impl Into<String>) {
        self.issues.push(Issue {
            severity: Severity::Warning,
            rule,
            path: path.to_string(),
            message: message.into(),
        });
    }
    /// A "valid but problematic" note — suppressed when warn_problematic is off.
    fn note(&mut self, rule: &'static str, path: &str, message: impl Into<String>) {
        if self.opts.warn_problematic {
            self.warn(rule, path, message);
        }
    }
}

fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

/// Compact display for a coordinate value (`1` not `1.0`, full precision kept).
fn num(n: f64) -> String {
    format!("{n}")
}

/// Decimal places in a JSON number's own text form; `None` for exponent forms.
fn decimals(v: &Value) -> Option<usize> {
    let s = match v {
        Value::Number(n) => n.to_string(),
        _ => return None,
    };
    if s.contains('e') || s.contains('E') {
        return None;
    }
    s.split_once('.').map(|(_, frac)| frac.len())
}

fn as_object<'v>(v: &'v Value, ctx: &mut Ctx, path: &str, what: &str) -> Option<&'v Map<String, Value>> {
    match v.as_object() {
        Some(o) => Some(o),
        None => {
            ctx.err(
                "not-an-object",
                path,
                format!("expected {what} (a JSON object), got {}", type_name(v)),
            );
            None
        }
    }
}

fn as_array<'v>(v: &'v Value, ctx: &mut Ctx, path: &str, what: &str) -> Option<&'v Vec<Value>> {
    match v.as_array() {
        Some(a) => Some(a),
        None => {
            ctx.err(
                "not-an-array",
                path,
                format!("expected {what} (a JSON array), got {}", type_name(v)),
            );
            None
        }
    }
}

/// Read a `type` member as a string, reporting a missing/mistyped one.
fn read_type(obj: &Map<String, Value>, ctx: &mut Ctx, path: &str) -> Option<String> {
    match obj.get("type") {
        None => {
            ctx.err(
                "missing-type",
                path,
                "no \"type\" member — every GeoJSON object needs one (\"FeatureCollection\", \"Feature\", or a geometry type)",
            );
            None
        }
        Some(Value::String(s)) => Some(s.clone()),
        Some(other) => {
            ctx.err(
                "type-not-a-string",
                path,
                format!("\"type\" must be a string, got {}", type_name(other)),
            );
            None
        }
    }
}

// ----------------------------------------------------------------- positions

/// Validate one position and return its (lon, lat) when usable.
fn check_position(v: &Value, ctx: &mut Ctx, path: &str) -> Option<(f64, f64)> {
    let arr = as_array(v, ctx, path, "a position (an array of numbers)")?;
    ctx.counts.positions += 1;
    if arr.len() < 2 {
        ctx.err(
            "position-too-short",
            path,
            format!(
                "a position needs at least 2 numbers [longitude, latitude], got {}",
                arr.len()
            ),
        );
        return None;
    }
    if arr.len() > 3 {
        ctx.err(
            "position-too-long",
            path,
            format!(
                "a position holds at most 3 numbers [longitude, latitude, altitude], got {}",
                arr.len()
            ),
        );
    }
    let mut ok = true;
    for (i, n) in arr.iter().enumerate() {
        match n.as_f64() {
            Some(f) if f.is_finite() => {
                if let (Some(d), true) = (decimals(n), ctx.opts.max_precision >= 0) {
                    if d > ctx.opts.max_precision as usize {
                        let max = ctx.opts.max_precision;
                        ctx.note(
                            "excess-precision",
                            &format!("{path}[{i}]"),
                            format!(
                                "{d} decimal places — more than {max}; RFC 7946 suggests trimming coordinate precision to what the data actually resolves"
                            ),
                        );
                    }
                }
            }
            Some(_) => {
                ok = false;
                ctx.err(
                    "coordinate-not-finite",
                    &format!("{path}[{i}]"),
                    "coordinate is not a finite number",
                );
            }
            None => {
                ok = false;
                ctx.err(
                    "coordinate-not-a-number",
                    &format!("{path}[{i}]"),
                    format!("expected a number, got {}", type_name(n)),
                );
            }
        }
    }
    if !ok {
        return None;
    }
    let lon = arr[0].as_f64().unwrap_or_default();
    let lat = arr[1].as_f64().unwrap_or_default();
    let mut in_range = true;
    if !(-180.0..=180.0).contains(&lon) {
        in_range = false;
        ctx.err(
            "longitude-out-of-range",
            &format!("{path}[0]"),
            format!(
                "longitude {} is outside -180..180 — GeoJSON positions are [longitude, latitude] in WGS 84; is the pair swapped or in a projected CRS?",
                num(lon)
            ),
        );
    }
    if !(-90.0..=90.0).contains(&lat) {
        in_range = false;
        ctx.err(
            "latitude-out-of-range",
            &format!("{path}[1]"),
            format!(
                "latitude {} is outside -90..90 — GeoJSON positions are [longitude, latitude] in WGS 84; is the pair swapped or in a projected CRS?",
                num(lat)
            ),
        );
    }
    if in_range {
        ctx.counts.min_lon = Some(ctx.counts.min_lon.map_or(lon, |v| v.min(lon)));
        ctx.counts.max_lon = Some(ctx.counts.max_lon.map_or(lon, |v| v.max(lon)));
        ctx.counts.min_lat = Some(ctx.counts.min_lat.map_or(lat, |v| v.min(lat)));
        ctx.counts.max_lat = Some(ctx.counts.max_lat.map_or(lat, |v| v.max(lat)));
    }
    Some((lon, lat))
}

/// Validate an array of positions; returns the ones that parsed cleanly.
fn check_positions(
    v: &Value,
    ctx: &mut Ctx,
    path: &str,
    min: usize,
    what: &str,
) -> Option<Vec<(f64, f64)>> {
    let arr = as_array(v, ctx, path, "an array of positions")?;
    if arr.len() < min {
        ctx.err(
            "too-few-positions",
            path,
            format!("{what} needs at least {min} positions, got {}", arr.len()),
        );
    }
    let mut pts = Vec::with_capacity(arr.len());
    let mut all_ok = true;
    for (i, p) in arr.iter().enumerate() {
        match check_position(p, ctx, &format!("{path}[{i}]")) {
            Some(pt) => pts.push(pt),
            None => all_ok = false,
        }
    }
    if !all_ok {
        return None;
    }
    Some(pts)
}

/// Consecutive-duplicate + antimeridian notes shared by lines and rings.
fn note_sequence_problems(pts: &[(f64, f64)], ctx: &mut Ctx, path: &str) {
    for i in 1..pts.len() {
        if pts[i] == pts[i - 1] {
            ctx.note(
                "duplicate-node",
                &format!("{path}[{i}]"),
                format!(
                    "repeats the previous position [{}, {}] — consecutive duplicates add no shape and break some geometry engines",
                    num(pts[i].0),
                    num(pts[i].1)
                ),
            );
        }
        if (pts[i].0 - pts[i - 1].0).abs() > 180.0 {
            ctx.note(
                "crosses-antimeridian",
                &format!("{path}[{i}]"),
                "the segment jumps more than 180° of longitude — RFC 7946 asks for geometries crossing the antimeridian to be split at ±180",
            );
        }
    }
}

/// Twice the signed area of a closed ring (positive = counterclockwise).
fn signed_area2(pts: &[(f64, f64)]) -> f64 {
    let mut sum = 0.0;
    for i in 0..pts.len() {
        let (x1, y1) = pts[i];
        let (x2, y2) = pts[(i + 1) % pts.len()];
        sum += x1 * y2 - x2 * y1;
    }
    sum
}

/// Validate one linear ring. `index` 0 is the exterior ring; the rest are holes.
fn check_ring(v: &Value, ctx: &mut Ctx, path: &str, index: usize) {
    ctx.counts.rings += 1;
    if index == 0 {
        ctx.counts.exterior_rings += 1;
    } else {
        ctx.counts.interior_rings += 1;
    }
    let Some(pts) = check_positions(v, ctx, path, 4, "a polygon ring") else {
        return;
    };
    if pts.len() < 4 {
        return;
    }
    note_sequence_problems(&pts, ctx, path);

    let closed = pts[0] == pts[pts.len() - 1];
    if !closed {
        ctx.err(
            "ring-not-closed",
            path,
            format!(
                "the ring is not closed — its last position must repeat its first ([{}, {}] vs [{}, {}])",
                num(pts[pts.len() - 1].0),
                num(pts[pts.len() - 1].1),
                num(pts[0].0),
                num(pts[0].1)
            ),
        );
        return;
    }

    let unique: BTreeSet<String> = pts[..pts.len() - 1]
        .iter()
        .map(|(x, y)| format!("{x},{y}"))
        .collect();
    if unique.len() < 3 {
        ctx.err(
            "ring-too-few-unique-nodes",
            path,
            format!(
                "the ring has only {} distinct position(s) — an area needs at least 3 before the closing repeat",
                unique.len()
            ),
        );
        return;
    }

    let area2 = signed_area2(&pts);
    if area2 == 0.0 {
        ctx.note(
            "zero-area-ring",
            path,
            "the ring encloses no area — its positions are collinear or cancel out",
        );
        return;
    }
    let ccw = area2 > 0.0;
    let (want_ccw, kind, expected) = if index == 0 {
        (true, "exterior ring", "counterclockwise")
    } else {
        (false, "interior ring (hole)", "clockwise")
    };
    if ccw != want_ccw {
        let got = if ccw { "counterclockwise" } else { "clockwise" };
        let msg = format!(
            "the {kind} winds {got} but RFC 7946's right-hand rule wants it {expected} — reversed winding makes some renderers fill the rest of the world instead of this shape"
        );
        if ctx.opts.strict_winding {
            ctx.err("winding-wrong", path, msg);
        } else {
            ctx.warn("winding-wrong", path, msg);
        }
    }
}

fn check_polygon(v: &Value, ctx: &mut Ctx, path: &str) {
    let Some(rings) = as_array(v, ctx, path, "an array of linear rings") else {
        return;
    };
    if rings.is_empty() {
        ctx.err(
            "empty-polygon",
            path,
            "a Polygon needs at least one linear ring (the exterior ring)",
        );
        return;
    }
    for (i, ring) in rings.iter().enumerate() {
        check_ring(ring, ctx, &format!("{path}[{i}]"), i);
    }
}

// ---------------------------------------------------------------- geometries

fn check_geometry(v: &Value, ctx: &mut Ctx, path: &str, depth: usize) {
    let Some(obj) = as_object(v, ctx, path, "a geometry object") else {
        return;
    };
    let Some(gtype) = read_type(obj, ctx, path) else {
        return;
    };
    if !GEOMETRY_TYPES.contains(&gtype.as_str()) {
        ctx.err(
            "unknown-geometry-type",
            path,
            format!(
                "\"{gtype}\" is not a GeoJSON geometry type — expected one of {}",
                GEOMETRY_TYPES.join(", ")
            ),
        );
        return;
    }
    ctx.counts.geometries += 1;
    *ctx.counts.by_type.entry(gtype.clone()).or_insert(0) += 1;
    check_bbox(obj, ctx, path);

    if gtype == "GeometryCollection" {
        if obj.contains_key("coordinates") {
            ctx.err(
                "geometrycollection-with-coordinates",
                path,
                "a GeometryCollection carries a \"geometries\" array, never \"coordinates\"",
            );
        }
        let Some(geoms) = obj.get("geometries") else {
            ctx.err(
                "missing-geometries",
                path,
                "a GeometryCollection needs a \"geometries\" member",
            );
            return;
        };
        let Some(list) = as_array(geoms, ctx, &format!("{path}.geometries"), "an array of geometries")
        else {
            return;
        };
        if depth > 0 {
            ctx.note(
                "nested-geometrycollection",
                path,
                "a GeometryCollection inside another one — RFC 7946 discourages nesting because many consumers do not unwrap it",
            );
        }
        if list.len() == 1 {
            ctx.note(
                "single-element-collection",
                path,
                "a GeometryCollection holding one geometry — the geometry alone is usually clearer",
            );
        }
        for (i, g) in list.iter().enumerate() {
            check_geometry(g, ctx, &format!("{path}.geometries[{i}]"), depth + 1);
        }
        return;
    }

    let Some(coords) = obj.get("coordinates") else {
        ctx.err(
            "missing-coordinates",
            path,
            format!("a {gtype} needs a \"coordinates\" member"),
        );
        return;
    };
    let cpath = format!("{path}.coordinates");
    match gtype.as_str() {
        "Point" => {
            check_position(coords, ctx, &cpath);
        }
        "MultiPoint" => {
            if let Some(pts) = check_positions(coords, ctx, &cpath, 0, "a MultiPoint") {
                if pts.len() == 1 {
                    ctx.note(
                        "single-element-multi",
                        path,
                        "a MultiPoint holding one position — a Point says the same thing",
                    );
                }
            }
        }
        "LineString" => {
            if let Some(pts) = check_positions(coords, ctx, &cpath, 2, "a LineString") {
                note_sequence_problems(&pts, ctx, &cpath);
                if pts.len() >= 2 && pts.iter().all(|p| *p == pts[0]) {
                    ctx.note(
                        "zero-length-linestring",
                        &cpath,
                        "every position is identical — the line has no length",
                    );
                }
            }
        }
        "MultiLineString" => {
            if let Some(lines) = as_array(coords, ctx, &cpath, "an array of LineStrings") {
                if lines.len() == 1 {
                    ctx.note(
                        "single-element-multi",
                        path,
                        "a MultiLineString holding one line — a LineString says the same thing",
                    );
                }
                for (i, l) in lines.iter().enumerate() {
                    let lp = format!("{cpath}[{i}]");
                    if let Some(pts) = check_positions(l, ctx, &lp, 2, "a LineString") {
                        note_sequence_problems(&pts, ctx, &lp);
                    }
                }
            }
        }
        "Polygon" => check_polygon(coords, ctx, &cpath),
        "MultiPolygon" => {
            if let Some(polys) = as_array(coords, ctx, &cpath, "an array of Polygons") {
                if polys.len() == 1 {
                    ctx.note(
                        "single-element-multi",
                        path,
                        "a MultiPolygon holding one polygon — a Polygon says the same thing",
                    );
                }
                for (i, p) in polys.iter().enumerate() {
                    check_polygon(p, ctx, &format!("{cpath}[{i}]"));
                }
            }
        }
        _ => unreachable!("geometry type already checked against GEOMETRY_TYPES"),
    }
}

// --------------------------------------------------------------- bbox + crs

fn check_bbox(obj: &Map<String, Value>, ctx: &mut Ctx, path: &str) {
    let Some(bbox) = obj.get("bbox") else { return };
    let bpath = format!("{path}.bbox");
    ctx.counts.bbox_members += 1;
    if !ctx.opts.allow_bbox {
        ctx.err(
            "bbox-not-allowed",
            &bpath,
            "a \"bbox\" member is present but bounding boxes are switched off for this check",
        );
    }
    let Some(arr) = as_array(bbox, ctx, &bpath, "a bounding box array") else {
        return;
    };
    if arr.len() != 4 && arr.len() != 6 {
        ctx.err(
            "bbox-wrong-length",
            &bpath,
            format!(
                "a bbox holds 4 values [west, south, east, north] or 6 with altitudes, got {}",
                arr.len()
            ),
        );
        return;
    }
    let mut vals = Vec::with_capacity(arr.len());
    for (i, n) in arr.iter().enumerate() {
        match n.as_f64() {
            Some(f) if f.is_finite() => vals.push(f),
            _ => {
                ctx.err(
                    "bbox-not-a-number",
                    &format!("{bpath}[{i}]"),
                    format!("expected a finite number, got {}", type_name(n)),
                );
                return;
            }
        }
    }
    // 4-value boxes are [w, s, e, n]; 6-value boxes are [w, s, minAlt, e, n, maxAlt].
    let (w, s, e, n) = if vals.len() == 4 {
        (vals[0], vals[1], vals[2], vals[3])
    } else {
        (vals[0], vals[1], vals[3], vals[4])
    };
    for (label, v, lo, hi) in [
        ("west", w, -180.0, 180.0),
        ("east", e, -180.0, 180.0),
        ("south", s, -90.0, 90.0),
        ("north", n, -90.0, 90.0),
    ] {
        if !(lo..=hi).contains(&v) {
            ctx.err(
                "bbox-out-of-range",
                &bpath,
                format!(
                    "{label} value {} is outside {}..{}",
                    num(v),
                    num(lo),
                    num(hi)
                ),
            );
        }
    }
    if s > n {
        ctx.err(
            "bbox-order",
            &bpath,
            format!(
                "south {} is north of north {} — a bbox reads [west, south, east, north]",
                num(s),
                num(n)
            ),
        );
    }
    if w > e {
        ctx.note(
            "bbox-antimeridian",
            &bpath,
            format!(
                "west {} is east of east {} — valid only for a box that crosses the antimeridian",
                num(w),
                num(e)
            ),
        );
    }
}

fn check_crs(obj: &Map<String, Value>, ctx: &mut Ctx, path: &str) {
    if obj.contains_key("crs") {
        ctx.note(
            "crs-member",
            &format!("{path}.crs"),
            "RFC 7946 dropped the \"crs\" member — coordinates are always WGS 84 (EPSG:4326) longitude/latitude, and some parsers reject the member outright",
        );
    }
}

// ------------------------------------------------------------------ features

fn check_feature(v: &Value, ctx: &mut Ctx, path: &str) {
    let Some(obj) = as_object(v, ctx, path, "a Feature object") else {
        return;
    };
    ctx.counts.features += 1;
    if let Some(t) = read_type(obj, ctx, path) {
        if t != "Feature" {
            ctx.err(
                "not-a-feature",
                path,
                format!("expected type \"Feature\", got \"{t}\""),
            );
            return;
        }
    }
    check_bbox(obj, ctx, path);
    check_crs(obj, ctx, path);

    match obj.get("properties") {
        None => ctx.err(
            "missing-properties",
            path,
            "a Feature needs a \"properties\" member — use {} or null when it carries no attributes",
        ),
        Some(Value::Null) | Some(Value::Object(_)) => {}
        Some(other) => ctx.err(
            "properties-wrong-type",
            &format!("{path}.properties"),
            format!("expected an object or null, got {}", type_name(other)),
        ),
    }

    if let Some(id) = obj.get("id") {
        if !id.is_string() && !id.is_number() {
            ctx.err(
                "id-wrong-type",
                &format!("{path}.id"),
                format!("a Feature \"id\" must be a string or a number, got {}", type_name(id)),
            );
        }
    }

    match obj.get("geometry") {
        None => ctx.err(
            "missing-geometry",
            path,
            "a Feature needs a \"geometry\" member — use null when the geometry is unknown",
        ),
        Some(Value::Null) => ctx.note(
            "null-geometry",
            &format!("{path}.geometry"),
            "the geometry is null — allowed by RFC 7946, but many consumers skip or reject such features",
        ),
        Some(g) => check_geometry(g, ctx, &format!("{path}.geometry"), 0),
    }
}

// ---------------------------------------------------------------- entrypoint

/// Walk one GeoJSON document and collect every issue.
pub fn validate(input: &str, opts: &Options) -> Result<Report, String> {
    if input.trim().is_empty() {
        return Err("input is empty — paste a GeoJSON FeatureCollection, Feature, or geometry".into());
    }
    let mut ctx = Ctx {
        opts,
        issues: Vec::new(),
        counts: Counts::default(),
    };
    let root: Value = match serde_json::from_str(input) {
        Ok(v) => v,
        Err(e) => {
            ctx.err(
                "invalid-json",
                "(root)",
                format!("input is not valid JSON: {e} — GeoJSON is JSON, so fix the syntax error first"),
            );
            return Ok(Report {
                document_type: None,
                issues: ctx.issues,
                counts: ctx.counts,
            });
        }
    };

    let mut document_type = None;
    if let Some(obj) = as_object(&root, &mut ctx, "(root)", "a GeoJSON object") {
        if let Some(t) = read_type(obj, &mut ctx, "(root)") {
            document_type = Some(t.clone());
            match t.as_str() {
                "FeatureCollection" => {
                    check_bbox(obj, &mut ctx, "(root)");
                    check_crs(obj, &mut ctx, "(root)");
                    match obj.get("features") {
                        None => ctx.err(
                            "missing-features",
                            "(root)",
                            "a FeatureCollection needs a \"features\" member (an array, possibly empty)",
                        ),
                        Some(f) => {
                            if let Some(list) =
                                as_array(f, &mut ctx, "(root).features", "an array of Features")
                            {
                                if list.is_empty() {
                                    ctx.note(
                                        "empty-featurecollection",
                                        "(root).features",
                                        "the collection holds no features — valid, but nothing will draw",
                                    );
                                }
                                for (i, feat) in list.iter().enumerate() {
                                    check_feature(feat, &mut ctx, &format!("features[{i}]"));
                                }
                            }
                        }
                    }
                }
                "Feature" => check_feature(&root, &mut ctx, "(root)"),
                t if GEOMETRY_TYPES.contains(&t) => {
                    check_crs(obj, &mut ctx, "(root)");
                    ctx.note(
                        "bare-geometry",
                        "(root)",
                        "the document is a bare geometry — valid GeoJSON, but many tools expect a Feature or FeatureCollection wrapper",
                    );
                    check_geometry(&root, &mut ctx, "(root)", 0);
                }
                other => ctx.err(
                    "unknown-document-type",
                    "(root)",
                    format!(
                        "\"{other}\" is not a GeoJSON type — expected \"FeatureCollection\", \"Feature\", or one of {}",
                        GEOMETRY_TYPES.join(", ")
                    ),
                ),
            }
        }
    }

    Ok(Report {
        document_type,
        issues: ctx.issues,
        counts: ctx.counts,
    })
}

// -------------------------------------------------------------------- render

fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("{n} {one}")
    } else {
        format!("{n} {many}")
    }
}

fn geometry_breakdown(counts: &Counts) -> String {
    if counts.by_type.is_empty() {
        return String::new();
    }
    let parts: Vec<String> = counts
        .by_type
        .iter()
        .map(|(k, v)| format!("{k} {v}"))
        .collect();
    format!(" ({})", parts.join(", "))
}

fn render_issue_list(
    out: &mut String,
    heading: &str,
    issues: &[&Issue],
    max: usize,
) {
    if issues.is_empty() {
        return;
    }
    out.push('\n');
    out.push_str(heading);
    out.push('\n');
    for (i, issue) in issues.iter().take(max).enumerate() {
        out.push_str(&format!(
            "  {}. [{}] {}: {}\n",
            i + 1,
            issue.rule,
            issue.path,
            issue.message
        ));
    }
    if issues.len() > max {
        out.push_str(&format!(
            "  … and {} more (raise max_issues to list them)\n",
            issues.len() - max
        ));
    }
}

pub fn render_text(report: &Report, opts: &Options) -> String {
    let errors: Vec<&Issue> = report.errors().collect();
    let warnings: Vec<&Issue> = report.warnings().collect();
    let mut out = format!(
        "{} — {}, {}\n",
        if report.valid() { "VALID" } else { "INVALID" },
        plural(errors.len(), "error", "errors"),
        plural(warnings.len(), "warning", "warnings"),
    );
    let max = opts.max_issues as usize;
    render_issue_list(&mut out, "Errors", &errors, max);
    render_issue_list(&mut out, "Warnings", &warnings, max);

    let c = &report.counts;
    out.push_str("\nSummary\n");
    out.push_str(&format!(
        "  document type: {}\n",
        report.document_type.as_deref().unwrap_or("unknown")
    ));
    out.push_str(&format!("  features: {}\n", c.features));
    out.push_str(&format!(
        "  geometries: {}{}\n",
        c.geometries,
        geometry_breakdown(c)
    ));
    out.push_str(&format!("  positions: {}\n", c.positions));
    out.push_str(&format!(
        "  rings: {} ({} exterior, {} interior)\n",
        c.rings, c.exterior_rings, c.interior_rings
    ));
    out.push_str(&format!("  bbox members: {}\n", c.bbox_members));
    match (c.min_lon, c.max_lon, c.min_lat, c.max_lat) {
        (Some(w), Some(e), Some(s), Some(n)) => {
            out.push_str(&format!(
                "  bounds: longitude {} .. {}, latitude {} .. {}\n",
                num(w),
                num(e),
                num(s),
                num(n)
            ));
        }
        _ => out.push_str("  bounds: none (no usable coordinates)\n"),
    }
    out
}

fn issues_json(issues: &[&Issue], max: usize) -> Value {
    Value::Array(
        issues
            .iter()
            .take(max)
            .map(|i| {
                serde_json::json!({
                    "rule": i.rule,
                    "path": i.path,
                    "message": i.message,
                })
            })
            .collect(),
    )
}

pub fn render_json(report: &Report, opts: &Options) -> String {
    let errors: Vec<&Issue> = report.errors().collect();
    let warnings: Vec<&Issue> = report.warnings().collect();
    let max = opts.max_issues as usize;
    let c = &report.counts;
    let bounds = match (c.min_lon, c.max_lon, c.min_lat, c.max_lat) {
        (Some(w), Some(e), Some(s), Some(n)) => serde_json::json!({
            "min_lon": w, "max_lon": e, "min_lat": s, "max_lat": n
        }),
        _ => Value::Null,
    };
    let by_type: Map<String, Value> = c
        .by_type
        .iter()
        .map(|(k, v)| (k.clone(), Value::from(*v)))
        .collect();
    let doc = serde_json::json!({
        "valid": report.valid(),
        "error_count": errors.len(),
        "warning_count": warnings.len(),
        "errors": issues_json(&errors, max),
        "warnings": issues_json(&warnings, max),
        "truncated": {
            "errors": errors.len() > max,
            "warnings": warnings.len() > max,
        },
        "summary": {
            "document_type": report.document_type,
            "features": c.features,
            "geometries": { "total": c.geometries, "by_type": Value::Object(by_type) },
            "positions": c.positions,
            "rings": {
                "total": c.rings,
                "exterior": c.exterior_rings,
                "interior": c.interior_rings,
            },
            "bbox_members": c.bbox_members,
            "bounds": bounds,
        }
    });
    serde_json::to_string_pretty(&doc).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
}

/// Surface entrypoint: every surface (chat, CLI, page) calls this with raw
/// values, so option parsing and its error messages live in one place.
#[allow(clippy::too_many_arguments)]
pub fn run(
    input: &str,
    output: &str,
    strict_winding: bool,
    allow_bbox: bool,
    warn_problematic: bool,
    max_precision: i64,
    max_issues: i64,
) -> Result<String, String> {
    let opts = Options::parse(
        output,
        strict_winding,
        allow_bbox,
        warn_problematic,
        max_precision,
        max_issues,
    )?;
    let report = validate(input, &opts)?;
    Ok(match opts.output {
        OutputFormat::Text => render_text(&report, &opts),
        OutputFormat::Json => render_json(&report, &opts),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> Options {
        Options::default()
    }

    fn rules(input: &str, o: &Options) -> Vec<&'static str> {
        validate(input, o)
            .unwrap()
            .issues
            .iter()
            .map(|i| i.rule)
            .collect()
    }

    const VALID_FC: &str = r#"{"type":"FeatureCollection","features":[
        {"type":"Feature","properties":{"name":"A"},
         "geometry":{"type":"Polygon","coordinates":[[[0,0],[1,0],[1,1],[0,1],[0,0]]]}}]}"#;

    #[test]
    fn accepts_a_valid_feature_collection() {
        let r = validate(VALID_FC, &opts()).unwrap();
        assert!(r.valid(), "{:?}", r.issues);
        assert_eq!(r.error_count(), 0);
        assert_eq!(r.warning_count(), 0);
        assert_eq!(r.counts.features, 1);
        assert_eq!(r.counts.geometries, 1);
        assert_eq!(r.counts.positions, 5);
        assert_eq!(r.counts.exterior_rings, 1);
        assert_eq!(r.counts.by_type.get("Polygon"), Some(&1));
    }

    #[test]
    fn text_output_leads_with_the_verdict_and_summary() {
        let out = run(VALID_FC, "text", true, true, true, 6, 50).unwrap();
        assert_eq!(
            out,
            "VALID — 0 errors, 0 warnings\n\nSummary\n  document type: FeatureCollection\n  \
             features: 1\n  geometries: 1 (Polygon 1)\n  positions: 5\n  \
             rings: 1 (1 exterior, 0 interior)\n  bbox members: 0\n  \
             bounds: longitude 0 .. 1, latitude 0 .. 1\n"
        );
    }

    #[test]
    fn json_output_carries_the_verdict_counts_and_summary() {
        let out = run(VALID_FC, "json", true, true, true, 6, 50).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["valid"], Value::Bool(true));
        assert_eq!(v["error_count"], 0);
        assert_eq!(v["summary"]["document_type"], "FeatureCollection");
        assert_eq!(v["summary"]["geometries"]["by_type"]["Polygon"], 1);
        assert_eq!(v["summary"]["bounds"]["max_lat"], 1.0);
    }

    #[test]
    fn reports_invalid_json_as_a_finding_not_a_failure() {
        let r = validate("{\"type\":", &opts()).unwrap();
        assert!(!r.valid());
        assert_eq!(r.issues[0].rule, "invalid-json");
        assert!(r.issues[0].message.starts_with("input is not valid JSON"));
    }

    #[test]
    fn rejects_an_unclosed_ring_with_a_path() {
        let doc = r#"{"type":"Polygon","coordinates":[[[0,0],[1,0],[1,1],[0,1]]]}"#;
        let r = validate(doc, &opts()).unwrap();
        let e = r.errors().next().unwrap();
        assert_eq!(e.rule, "ring-not-closed");
        assert_eq!(e.path, "(root).coordinates[0]");
    }

    #[test]
    fn reports_every_error_not_just_the_first() {
        let doc = r#"{"type":"FeatureCollection","features":[
            {"type":"Feature","properties":{},"geometry":{"type":"Point","coordinates":[200,0]}},
            {"type":"Feature","properties":{},"geometry":{"type":"Point","coordinates":[0,99]}},
            {"type":"Feature","geometry":{"type":"Point","coordinates":[0,0]}}]}"#;
        let r = validate(doc, &opts()).unwrap();
        assert_eq!(r.error_count(), 3);
        let got = rules(doc, &opts());
        assert!(got.contains(&"longitude-out-of-range"));
        assert!(got.contains(&"latitude-out-of-range"));
        assert!(got.contains(&"missing-properties"));
    }

    #[test]
    fn flags_a_clockwise_exterior_ring_and_a_counterclockwise_hole() {
        // Exterior wound CW, hole wound CCW — both backwards.
        let doc = r#"{"type":"Polygon","coordinates":[
            [[0,0],[0,4],[4,4],[4,0],[0,0]],
            [[1,1],[2,1],[2,2],[1,2],[1,1]]]}"#;
        let r = validate(doc, &opts()).unwrap();
        assert_eq!(r.error_count(), 2);
        assert!(r.errors().all(|e| e.rule == "winding-wrong"));
        assert!(r.errors().next().unwrap().message.contains("exterior ring"));
    }

    #[test]
    fn strict_winding_off_downgrades_winding_to_a_warning() {
        let doc = r#"{"type":"Polygon","coordinates":[[[0,0],[0,4],[4,4],[4,0],[0,0]]]}"#;
        let mut o = opts();
        o.strict_winding = false;
        let r = validate(doc, &o).unwrap();
        assert!(r.valid());
        assert!(r.warnings().any(|w| w.rule == "winding-wrong"));
    }

    #[test]
    fn rejects_non_numeric_and_short_positions() {
        let doc = r#"{"type":"MultiPoint","coordinates":[["a",0],[1]]}"#;
        let got = rules(doc, &opts());
        assert!(got.contains(&"coordinate-not-a-number"));
        assert!(got.contains(&"position-too-short"));
    }

    #[test]
    fn rejects_a_ring_without_three_distinct_nodes() {
        let doc = r#"{"type":"Polygon","coordinates":[[[0,0],[1,1],[0,0],[0,0]]]}"#;
        let got = rules(doc, &opts());
        assert!(got.contains(&"ring-too-few-unique-nodes"), "{got:?}");
    }

    #[test]
    fn checks_bbox_length_order_and_the_allow_toggle() {
        let short = r#"{"type":"Point","bbox":[0,0,1],"coordinates":[0,0]}"#;
        assert!(rules(short, &opts()).contains(&"bbox-wrong-length"));

        let flipped = r#"{"type":"Point","bbox":[0,9,1,2],"coordinates":[0,0]}"#;
        assert!(rules(flipped, &opts()).contains(&"bbox-order"));

        let ok = r#"{"type":"Point","bbox":[0,0,1,1],"coordinates":[0,0]}"#;
        let mut o = opts();
        assert!(validate(ok, &o).unwrap().valid());
        o.allow_bbox = false;
        assert!(rules(ok, &o).contains(&"bbox-not-allowed"));
    }

    #[test]
    fn problematic_warnings_can_be_switched_off() {
        let doc = r#"{"type":"Feature","crs":{"type":"name"},"properties":{},
            "geometry":{"type":"MultiPoint","coordinates":[[1.1234567,2]]}}"#;
        let got = rules(doc, &opts());
        assert!(got.contains(&"crs-member"));
        assert!(got.contains(&"single-element-multi"));
        assert!(got.contains(&"excess-precision"));

        let mut o = opts();
        o.warn_problematic = false;
        assert!(validate(doc, &o).unwrap().issues.is_empty());
    }

    #[test]
    fn precision_check_respects_its_threshold() {
        let doc = r#"{"type":"Point","coordinates":[1.1234567,2]}"#;
        let mut o = opts();
        o.max_precision = 7;
        assert!(!rules(doc, &o).contains(&"excess-precision"));
        o.max_precision = -1;
        assert!(!rules(doc, &o).contains(&"excess-precision"));
    }

    #[test]
    fn flags_duplicate_nodes_and_antimeridian_jumps() {
        let doc = r#"{"type":"LineString","coordinates":[[170,0],[170,0],[-170,0]]}"#;
        let got = rules(doc, &opts());
        assert!(got.contains(&"duplicate-node"));
        assert!(got.contains(&"crosses-antimeridian"));
    }

    #[test]
    fn walks_nested_geometry_collections() {
        let doc = r#"{"type":"GeometryCollection","geometries":[
            {"type":"GeometryCollection","geometries":[
                {"type":"Point","coordinates":[0,300]}]}]}"#;
        let r = validate(doc, &opts()).unwrap();
        let e = r.errors().next().unwrap();
        assert_eq!(e.rule, "latitude-out-of-range");
        assert_eq!(
            e.path,
            "(root).geometries[0].geometries[0].coordinates[1]"
        );
        assert!(rules(doc, &opts()).contains(&"nested-geometrycollection"));
    }

    #[test]
    fn rejects_unknown_types_and_non_objects() {
        assert!(rules(r#"{"type":"Polygone","coordinates":[]}"#, &opts())
            .contains(&"unknown-document-type"));
        assert!(rules("[1,2]", &opts()).contains(&"not-an-object"));
        assert!(rules(r#"{"features":[]}"#, &opts()).contains(&"missing-type"));
    }

    #[test]
    fn caps_the_listed_issues_but_keeps_the_counts() {
        let pts: Vec<String> = (0..5)
            .map(|_| r#"{"type":"Feature","properties":{},"geometry":{"type":"Point","coordinates":[999,0]}}"#.to_string())
            .collect();
        let doc = format!(
            r#"{{"type":"FeatureCollection","features":[{}]}}"#,
            pts.join(",")
        );
        let out = run(&doc, "text", true, true, true, 6, 2).unwrap();
        assert!(out.starts_with("INVALID — 5 errors, 0 warnings"), "{out}");
        assert!(out.contains("… and 3 more"), "{out}");
    }

    #[test]
    fn rejects_bad_option_values() {
        assert!(run("{}", "yaml", true, true, true, 6, 50)
            .unwrap_err()
            .starts_with("unknown output"));
        assert!(run("{}", "text", true, true, true, 99, 50)
            .unwrap_err()
            .starts_with("max_precision must be"));
        assert!(run("{}", "text", true, true, true, 6, 0)
            .unwrap_err()
            .starts_with("max_issues must be"));
        assert!(run("   ", "text", true, true, true, 6, 50)
            .unwrap_err()
            .starts_with("input is empty"));
    }
}
