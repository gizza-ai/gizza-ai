//! geojson-format core — pretty-print, minify, re-key, prune and bbox a single
//! GeoJSON document, and round its coordinate precision.
//!
//! Unlike `geojson-merge` (which flattens MANY inputs into one
//! `FeatureCollection`), this formats ONE document in place: a bare geometry
//! stays a bare geometry, a `Feature` stays a `Feature`, and a
//! `FeatureCollection` keeps its own top-level members.
//!
//! Pipeline (in this order, so every later step sees the earlier one's result):
//!   validate (RFC 7946, optional) → prune properties → round coordinates →
//!   fix polygon winding → add/strip bbox → re-key → serialize.
//!
//! Pure-Rust: `serde_json` with `preserve_order`, so member order is fully
//! under our control (needed for both `canonical` and `alpha` re-keying).

use serde::Serialize;
use serde_json::{Map, Value};

/// The seven RFC 7946 geometry object types.
pub const GEOMETRY_TYPES: [&str; 7] = [
    "Point",
    "MultiPoint",
    "LineString",
    "MultiLineString",
    "Polygon",
    "MultiPolygon",
    "GeometryCollection",
];

/// RFC 7946 member order used by `key_order = "canonical"`. Any member not in
/// this list keeps its original relative position, after these.
const CANONICAL_ORDER: [&str; 8] = [
    "type",
    "id",
    "bbox",
    "coordinates",
    "geometries",
    "geometry",
    "properties",
    "features",
];

/// How object members are re-keyed on output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyOrder {
    /// Leave every member exactly where it was.
    Keep,
    /// RFC 7946 member order for GeoJSON objects; `properties` contents untouched.
    Canonical,
    /// Sort every object's keys alphabetically, `properties` included.
    Alpha,
}

impl KeyOrder {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "keep" => Ok(Self::Keep),
            "canonical" => Ok(Self::Canonical),
            "alpha" => Ok(Self::Alpha),
            other => Err(format!(
                "unknown key_order \"{other}\" (expected \"keep\", \"canonical\", or \"alpha\")"
            )),
        }
    }
}

/// What to do with `bbox` members.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BboxMode {
    /// Leave existing `bbox` members alone.
    Keep,
    /// Recompute the document's own top-level `bbox`.
    Add,
    /// Recompute the top-level `bbox` AND every feature's `bbox`.
    Features,
    /// Remove every `bbox` member, at any depth.
    Strip,
}

impl BboxMode {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "keep" => Ok(Self::Keep),
            "add" => Ok(Self::Add),
            "features" => Ok(Self::Features),
            "strip" => Ok(Self::Strip),
            other => Err(format!(
                "unknown bbox mode \"{other}\" (expected \"keep\", \"add\", \"features\", or \"strip\")"
            )),
        }
    }
}

/// Polygon ring orientation handling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Winding {
    /// Leave ring order untouched.
    Keep,
    /// RFC 7946 §3.1.6 right-hand rule: exterior rings counterclockwise,
    /// interior rings (holes) clockwise.
    Rfc7946,
}

impl Winding {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "keep" => Ok(Self::Keep),
            "rfc7946" => Ok(Self::Rfc7946),
            other => Err(format!(
                "unknown winding \"{other}\" (expected \"keep\" or \"rfc7946\")"
            )),
        }
    }
}

/// Indentation character for pretty output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IndentChar {
    Space,
    Tab,
}

impl IndentChar {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "space" => Ok(Self::Space),
            "tab" => Ok(Self::Tab),
            other => Err(format!(
                "unknown indent_char \"{other}\" (expected \"space\" or \"tab\")"
            )),
        }
    }

    fn byte(self) -> u8 {
        match self {
            Self::Space => b' ',
            Self::Tab => b'\t',
        }
    }
}

/// Every knob the formatter exposes. Built once (see [`FormatOptions::from_strings`])
/// and shared by the chat/CLI block and the browser wrapper.
#[derive(Clone, Debug, PartialEq)]
pub struct FormatOptions {
    /// Indent units per level; 0 minifies to a single line. Capped at 8.
    pub indent: usize,
    pub indent_char: IndentChar,
    /// Decimal places to round every coordinate to; negative keeps full precision.
    pub precision: i64,
    pub key_order: KeyOrder,
    pub bbox: BboxMode,
    pub winding: Winding,
    /// If non-empty, keep ONLY these feature properties.
    pub keep_properties: Vec<String>,
    /// Always remove these feature properties (applied after `keep_properties`).
    pub drop_properties: Vec<String>,
    /// Remove properties whose value is `null`, `""`, `[]` or `{}`.
    pub drop_empty_properties: bool,
    /// Run the RFC 7946 conformance checks before formatting.
    pub validate: bool,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self {
            indent: 2,
            indent_char: IndentChar::Space,
            precision: -1,
            key_order: KeyOrder::Keep,
            bbox: BboxMode::Keep,
            winding: Winding::Keep,
            keep_properties: Vec::new(),
            drop_properties: Vec::new(),
            drop_empty_properties: false,
            validate: true,
        }
    }
}

/// Split a comma/newline-separated property-name list into trimmed names.
fn name_list(s: &str) -> Vec<String> {
    s.split([',', '\n'])
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(str::to_string)
        .collect()
}

impl FormatOptions {
    /// Build options from the raw surface values (CLI/chat integers + page
    /// strings), so enum parsing and its error messages live in exactly one place.
    #[allow(clippy::too_many_arguments)]
    pub fn from_strings(
        indent: i64,
        indent_char: &str,
        precision: i64,
        key_order: &str,
        bbox: &str,
        winding: &str,
        keep_properties: &str,
        drop_properties: &str,
        drop_empty_properties: bool,
        validate: bool,
    ) -> Result<Self, String> {
        if !(0..=8).contains(&indent) {
            return Err(format!("indent must be between 0 and 8, got {indent}"));
        }
        if !(-1..=15).contains(&precision) {
            return Err(format!(
                "precision must be between -1 and 15, got {precision}"
            ));
        }
        Ok(Self {
            indent: indent as usize,
            indent_char: IndentChar::parse(indent_char)?,
            precision,
            key_order: KeyOrder::parse(key_order)?,
            bbox: BboxMode::parse(bbox)?,
            winding: Winding::parse(winding)?,
            keep_properties: name_list(keep_properties),
            drop_properties: name_list(drop_properties),
            drop_empty_properties,
            validate,
        })
    }
}

// ---------------------------------------------------------------- validation

/// A human-readable name for what a JSON value actually is, for error messages.
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

fn as_array<'a>(v: &'a Value, path: &str, expected: &str) -> Result<&'a Vec<Value>, String> {
    v.as_array()
        .ok_or_else(|| format!("{path}: expected {expected}, got {}", type_name(v)))
}

/// Are two positions numerically identical (so `0` and `0.0` compare equal)?
fn positions_equal(a: &Value, b: &Value) -> bool {
    match (a.as_array(), b.as_array()) {
        (Some(x), Some(y)) => {
            x.len() == y.len()
                && x.iter()
                    .zip(y.iter())
                    .all(|(p, q)| match (p.as_f64(), q.as_f64()) {
                        (Some(m), Some(n)) => m == n,
                        _ => p == q,
                    })
        }
        _ => a == b,
    }
}

fn validate_position(v: &Value, path: &str) -> Result<(), String> {
    let arr = as_array(v, path, "a position (an array of numbers)")?;
    if arr.len() < 2 || arr.len() > 3 {
        return Err(format!(
            "{path}: expected a position of 2 or 3 numbers [longitude, latitude, altitude?], got {} value(s)",
            arr.len()
        ));
    }
    for (i, n) in arr.iter().enumerate() {
        if n.as_f64().is_none() {
            return Err(format!(
                "{path}[{i}]: expected a number, got {}",
                type_name(n)
            ));
        }
    }
    let lon = arr[0].as_f64().unwrap_or_default();
    let lat = arr[1].as_f64().unwrap_or_default();
    if !(-180.0..=180.0).contains(&lon) {
        return Err(format!(
            "{path}[0]: longitude {lon} is outside -180..180 — GeoJSON positions are [longitude, latitude] in WGS 84; is this pair swapped or in a projected CRS?"
        ));
    }
    if !(-90.0..=90.0).contains(&lat) {
        return Err(format!(
            "{path}[1]: latitude {lat} is outside -90..90 — GeoJSON positions are [longitude, latitude] in WGS 84; is this pair swapped or in a projected CRS?"
        ));
    }
    Ok(())
}

fn validate_positions(v: &Value, path: &str, min: usize, what: &str) -> Result<(), String> {
    let arr = as_array(v, path, "an array of positions")?;
    if arr.len() < min {
        return Err(format!(
            "{path}: {what} needs at least {min} positions, got {}",
            arr.len()
        ));
    }
    for (i, p) in arr.iter().enumerate() {
        validate_position(p, &format!("{path}[{i}]"))?;
    }
    Ok(())
}

fn validate_ring(v: &Value, path: &str) -> Result<(), String> {
    validate_positions(v, path, 4, "a polygon ring")?;
    let arr = v.as_array().unwrap();
    if !positions_equal(&arr[0], &arr[arr.len() - 1]) {
        return Err(format!(
            "{path}: the ring is not closed — its last position must repeat its first ({} vs {})",
            arr[0],
            arr[arr.len() - 1]
        ));
    }
    Ok(())
}

fn validate_rings(v: &Value, path: &str) -> Result<(), String> {
    let arr = as_array(v, path, "an array of linear rings")?;
    if arr.is_empty() {
        return Err(format!(
            "{path}: a Polygon needs at least one (exterior) ring, got an empty array"
        ));
    }
    for (i, ring) in arr.iter().enumerate() {
        validate_ring(ring, &format!("{path}[{i}]"))?;
    }
    Ok(())
}

fn validate_geometry(v: &Value, path: &str) -> Result<(), String> {
    if v.is_null() {
        return Ok(()); // an unlocated Feature is legal (RFC 7946 §3.2)
    }
    let obj = v
        .as_object()
        .ok_or_else(|| format!("{path}: expected a geometry object, got {}", type_name(v)))?;
    let t = obj
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{path}: geometry is missing its \"type\" member"))?;
    if !GEOMETRY_TYPES.contains(&t) {
        return Err(format!(
            "{path}: unknown geometry type \"{t}\" (expected one of {})",
            GEOMETRY_TYPES.join(", ")
        ));
    }
    if t == "GeometryCollection" {
        let geoms = obj.get("geometries").ok_or_else(|| {
            format!("{path}: a GeometryCollection must have a \"geometries\" array")
        })?;
        let arr = as_array(
            geoms,
            &format!("{path}.geometries"),
            "an array of geometries",
        )?;
        for (i, g) in arr.iter().enumerate() {
            validate_geometry(g, &format!("{path}.geometries[{i}]"))?;
        }
        return Ok(());
    }
    let coords = obj
        .get("coordinates")
        .ok_or_else(|| format!("{path}: a {t} must have a \"coordinates\" member"))?;
    let cpath = format!("{path}.coordinates");
    match t {
        "Point" => validate_position(coords, &cpath),
        "MultiPoint" => validate_positions(coords, &cpath, 0, "a MultiPoint"),
        "LineString" => validate_positions(coords, &cpath, 2, "a LineString"),
        "MultiLineString" => {
            let arr = as_array(coords, &cpath, "an array of line strings")?;
            for (i, l) in arr.iter().enumerate() {
                validate_positions(l, &format!("{cpath}[{i}]"), 2, "a LineString")?;
            }
            Ok(())
        }
        "Polygon" => validate_rings(coords, &cpath),
        "MultiPolygon" => {
            let arr = as_array(coords, &cpath, "an array of polygons")?;
            for (i, p) in arr.iter().enumerate() {
                validate_rings(p, &format!("{cpath}[{i}]"))?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn validate_feature(v: &Value, path: &str) -> Result<(), String> {
    let obj = v
        .as_object()
        .ok_or_else(|| format!("{path}: expected a Feature object, got {}", type_name(v)))?;
    match obj.get("type").and_then(Value::as_str) {
        Some("Feature") => {}
        Some(other) => {
            return Err(format!(
                "{path}: expected \"type\": \"Feature\", got \"{other}\""
            ))
        }
        None => return Err(format!("{path}: Feature is missing its \"type\" member")),
    }
    match obj.get("properties") {
        None => {
            return Err(format!(
                "{path}: a Feature must have a \"properties\" member (use null or {{}} when it has none)"
            ))
        }
        Some(Value::Object(_)) | Some(Value::Null) => {}
        Some(other) => {
            return Err(format!(
                "{path}.properties: expected an object or null, got {}",
                type_name(other)
            ))
        }
    }
    match obj.get("geometry") {
        None => Err(format!(
            "{path}: a Feature must have a \"geometry\" member (use null for an unlocated feature)"
        )),
        Some(g) => validate_geometry(g, &format!("{path}.geometry")),
    }
}

fn validate_document(root: &Value) -> Result<(), String> {
    let t = root.get("type").and_then(Value::as_str).unwrap_or_default();
    match t {
        "FeatureCollection" => {
            let feats = root
                .get("features")
                .ok_or_else(|| "FeatureCollection is missing its \"features\" array".to_string())?;
            let arr = as_array(feats, "features", "an array of Features")?;
            for (i, f) in arr.iter().enumerate() {
                validate_feature(f, &format!("features[{i}]"))?;
            }
            Ok(())
        }
        "Feature" => validate_feature(root, "(root)"),
        _ => validate_geometry(root, "(root)"),
    }
}

// ---------------------------------------------------------------- transforms

/// Rebuild `m` without `key`, preserving the order of the remaining members
/// (`Map::remove` is a swap-remove under `preserve_order`).
fn remove_key(m: &mut Map<String, Value>, key: &str) {
    if !m.contains_key(key) {
        return;
    }
    let kept: Map<String, Value> = m
        .iter()
        .filter(|(k, _)| k.as_str() != key)
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    *m = kept;
}

fn is_empty_value(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::String(s) => s.is_empty(),
        Value::Array(a) => a.is_empty(),
        Value::Object(o) => o.is_empty(),
        _ => false,
    }
}

/// Apply the keep-list / drop-list / drop-empty rules to one feature's properties.
fn prune_properties(feature: &mut Value, opts: &FormatOptions) {
    let Some(obj) = feature.as_object_mut() else {
        return;
    };
    let Some(Value::Object(props)) = obj.get_mut("properties") else {
        return;
    };
    let kept: Map<String, Value> = props
        .iter()
        .filter(|(k, v)| {
            if !opts.keep_properties.is_empty() && !opts.keep_properties.iter().any(|w| w == *k) {
                return false;
            }
            if opts.drop_properties.iter().any(|w| w == *k) {
                return false;
            }
            if opts.drop_empty_properties && is_empty_value(v) {
                return false;
            }
            true
        })
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    *props = kept;
}

/// Recursively round every number inside a `coordinates` value to `p` decimals.
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
                // + 0.0 avoids emitting "-0" for values that round to zero.
                if let Some(num) = serde_json::Number::from_f64(rounded + 0.0) {
                    *v = Value::Number(num);
                }
            }
        }
        _ => {}
    }
}

/// Signed area of a closed ring (positive = counterclockwise), by the shoelace formula.
fn ring_area(ring: &[Value]) -> f64 {
    let pt = |v: &Value| -> Option<(f64, f64)> {
        let a = v.as_array()?;
        Some((a.first()?.as_f64()?, a.get(1)?.as_f64()?))
    };
    let mut sum = 0.0;
    for w in ring.windows(2) {
        if let (Some((x1, y1)), Some((x2, y2))) = (pt(&w[0]), pt(&w[1])) {
            sum += (x2 - x1) * (y2 + y1);
        }
    }
    // The shoelace sum above is positive for CLOCKWISE rings, so negate it.
    -sum / 2.0
}

fn orient_ring(ring: &mut Value, want_ccw: bool) {
    if let Value::Array(a) = ring {
        if a.len() < 4 {
            return;
        }
        let ccw = ring_area(a) > 0.0;
        if ccw != want_ccw {
            a.reverse();
        }
    }
}

fn orient_polygon(poly: &mut Value) {
    if let Value::Array(rings) = poly {
        for (i, ring) in rings.iter_mut().enumerate() {
            orient_ring(ring, i == 0);
        }
    }
}

/// Walk any GeoJSON value and apply `f` to every geometry object found.
fn each_geometry(v: &mut Value, f: &mut impl FnMut(&mut Value)) {
    // Read the type first so the object borrow ends before `f(v)` re-borrows.
    let kind = v.get("type").and_then(Value::as_str).map(str::to_string);
    match kind.as_deref() {
        Some("FeatureCollection") => {
            if let Some(Value::Array(feats)) = v.get_mut("features") {
                for feat in feats.iter_mut() {
                    each_geometry(feat, f);
                }
            }
        }
        Some("Feature") => {
            if let Some(g) = v.get_mut("geometry") {
                if !g.is_null() {
                    each_geometry(g, f);
                }
            }
        }
        Some("GeometryCollection") => {
            if let Some(Value::Array(geoms)) = v.get_mut("geometries") {
                for g in geoms.iter_mut() {
                    each_geometry(g, f);
                }
            }
            f(v);
        }
        Some(t) if GEOMETRY_TYPES.contains(&t) => f(v),
        _ => {}
    }
}

// --------------------------------------------------------------------- bbox

#[derive(Default)]
struct BboxAcc {
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
    min_z: f64,
    max_z: f64,
    seen: bool,
    all_3d: bool,
}

impl BboxAcc {
    fn new() -> Self {
        Self {
            min_x: f64::INFINITY,
            min_y: f64::INFINITY,
            max_x: f64::NEG_INFINITY,
            max_y: f64::NEG_INFINITY,
            min_z: f64::INFINITY,
            max_z: f64::NEG_INFINITY,
            seen: false,
            all_3d: true,
        }
    }

    fn add(&mut self, pos: &[Value]) {
        let (Some(x), Some(y)) = (
            pos.first().and_then(Value::as_f64),
            pos.get(1).and_then(Value::as_f64),
        ) else {
            return;
        };
        self.seen = true;
        self.min_x = self.min_x.min(x);
        self.min_y = self.min_y.min(y);
        self.max_x = self.max_x.max(x);
        self.max_y = self.max_y.max(y);
        match pos.get(2).and_then(Value::as_f64) {
            Some(z) => {
                self.min_z = self.min_z.min(z);
                self.max_z = self.max_z.max(z);
            }
            None => self.all_3d = false,
        }
    }

    fn value(&self) -> Option<Value> {
        if !self.seen {
            return None;
        }
        let nums: Vec<f64> = if self.all_3d {
            vec![
                self.min_x, self.min_y, self.min_z, self.max_x, self.max_y, self.max_z,
            ]
        } else {
            vec![self.min_x, self.min_y, self.max_x, self.max_y]
        };
        Some(Value::Array(
            nums.into_iter()
                .map(|n| {
                    serde_json::Number::from_f64(n + 0.0)
                        .map(Value::Number)
                        .unwrap_or(Value::Null)
                })
                .collect(),
        ))
    }
}

/// Feed every position inside a `coordinates` value into the accumulator.
/// A position is an array whose first element is a number.
fn collect_positions(coords: &Value, acc: &mut BboxAcc) {
    if let Value::Array(a) = coords {
        if a.first().map(Value::is_number).unwrap_or(false) {
            acc.add(a);
        } else {
            for e in a {
                collect_positions(e, acc);
            }
        }
    }
}

/// Feed every position of every geometry under `v` into the accumulator.
fn collect_bbox(v: &Value, acc: &mut BboxAcc) {
    match v.get("type").and_then(Value::as_str) {
        Some("FeatureCollection") => {
            if let Some(feats) = v.get("features").and_then(Value::as_array) {
                for f in feats {
                    collect_bbox(f, acc);
                }
            }
        }
        Some("Feature") => {
            if let Some(g) = v.get("geometry") {
                collect_bbox(g, acc);
            }
        }
        Some("GeometryCollection") => {
            if let Some(geoms) = v.get("geometries").and_then(Value::as_array) {
                for g in geoms {
                    collect_bbox(g, acc);
                }
            }
        }
        Some(t) if GEOMETRY_TYPES.contains(&t) => {
            if let Some(coords) = v.get("coordinates") {
                collect_positions(coords, acc);
            }
        }
        _ => {}
    }
}

/// Bounding box of any GeoJSON value (Feature, FeatureCollection or geometry).
fn bbox_of(v: &Value) -> Option<Value> {
    let mut acc = BboxAcc::new();
    collect_bbox(v, &mut acc);
    acc.value()
}

/// Remove every `bbox` member at any depth.
fn strip_bbox(v: &mut Value) {
    match v {
        Value::Object(m) => {
            remove_key(m, "bbox");
            for (_, child) in m.iter_mut() {
                strip_bbox(child);
            }
        }
        Value::Array(a) => {
            for e in a.iter_mut() {
                strip_bbox(e);
            }
        }
        _ => {}
    }
}

// ----------------------------------------------------------------- re-keying

fn canonical_object(m: &mut Map<String, Value>) {
    let mut out = Map::new();
    for key in CANONICAL_ORDER {
        if let Some(v) = m.get(key) {
            out.insert(key.to_string(), v.clone());
        }
    }
    for (k, v) in m.iter() {
        if !CANONICAL_ORDER.contains(&k.as_str()) {
            out.insert(k.clone(), v.clone());
        }
    }
    *m = out;
}

/// Re-key GeoJSON objects into RFC 7946 member order, leaving the contents of
/// `properties` (user data) exactly as they were.
fn canonicalize(v: &mut Value) {
    let Some(m) = v.as_object_mut() else {
        return;
    };
    if let Some(Value::Array(feats)) = m.get_mut("features") {
        for f in feats.iter_mut() {
            canonicalize(f);
        }
    }
    if let Some(g) = m.get_mut("geometry") {
        canonicalize(g);
    }
    if let Some(Value::Array(geoms)) = m.get_mut("geometries") {
        for g in geoms.iter_mut() {
            canonicalize(g);
        }
    }
    canonical_object(m);
}

/// Sort every object's keys alphabetically, at any depth (properties included).
fn alphabetize(v: &mut Value) {
    match v {
        Value::Object(m) => {
            let mut keys: Vec<String> = m.keys().cloned().collect();
            keys.sort();
            let mut out = Map::new();
            for k in keys {
                let mut child = m.get(&k).cloned().unwrap_or(Value::Null);
                alphabetize(&mut child);
                out.insert(k, child);
            }
            *m = out;
        }
        Value::Array(a) => {
            for e in a.iter_mut() {
                alphabetize(e);
            }
        }
        _ => {}
    }
}

// --------------------------------------------------------------- serializing

fn write_out(value: &Value, opts: &FormatOptions) -> Result<String, String> {
    if opts.indent == 0 {
        return serde_json::to_string(value).map_err(|e| format!("serialize: {e}"));
    }
    let pad = vec![opts.indent_char.byte(); opts.indent.min(8)];
    let mut buf = Vec::new();
    let fmt = serde_json::ser::PrettyFormatter::with_indent(&pad);
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, fmt);
    value
        .serialize(&mut ser)
        .map_err(|e| format!("serialize: {e}"))?;
    String::from_utf8(buf).map_err(|e| format!("utf8: {e}"))
}

// ---------------------------------------------------------------- entrypoint

/// Format one GeoJSON document per `opts`, returning the rewritten document.
///
/// The document's shape is preserved: a `FeatureCollection` stays a
/// `FeatureCollection`, a bare `Feature` stays a `Feature`, and a bare geometry
/// stays a bare geometry.
pub fn format_geojson(input: &str, opts: &FormatOptions) -> Result<String, String> {
    if input.trim().is_empty() {
        return Err("expected GeoJSON text, got an empty input".into());
    }
    let mut root: Value = serde_json::from_str(input).map_err(|e| {
        format!("input is not valid JSON: {e} — GeoJSON is JSON, so fix the syntax error first")
    })?;

    let Some(obj) = root.as_object() else {
        return Err(format!(
            "expected a GeoJSON object at the top level, got {} — a GeoJSON document is an object with a \"type\" member",
            type_name(&root)
        ));
    };
    let doc_type = match obj.get("type") {
        Some(Value::String(s)) => s.clone(),
        Some(other) => {
            return Err(format!(
                "(root).type: expected a string, got {}",
                type_name(other)
            ))
        }
        None => {
            return Err(
                "(root): missing the \"type\" member — expected \"FeatureCollection\", \"Feature\", or a geometry type"
                    .into(),
            )
        }
    };
    let is_collection = doc_type == "FeatureCollection";
    let is_feature = doc_type == "Feature";
    if !is_collection && !is_feature && !GEOMETRY_TYPES.contains(&doc_type.as_str()) {
        return Err(format!(
            "(root).type: unsupported GeoJSON type \"{doc_type}\" (expected \"FeatureCollection\", \"Feature\", or one of {})",
            GEOMETRY_TYPES.join(", ")
        ));
    }

    if opts.validate {
        validate_document(&root)?;
    }

    // 1. Prune feature properties.
    let pruning = !opts.keep_properties.is_empty()
        || !opts.drop_properties.is_empty()
        || opts.drop_empty_properties;
    if pruning {
        if is_collection {
            if let Some(Value::Array(feats)) = root.get_mut("features") {
                for f in feats.iter_mut() {
                    prune_properties(f, opts);
                }
            }
        } else if is_feature {
            prune_properties(&mut root, opts);
        }
    }

    // 2. Round coordinates.
    if opts.precision >= 0 {
        let p = opts.precision;
        each_geometry(&mut root, &mut |g| {
            if let Some(coords) = g.get_mut("coordinates") {
                round_coords(coords, p);
            }
        });
    }

    // 3. Fix polygon winding.
    if opts.winding == Winding::Rfc7946 {
        each_geometry(&mut root, &mut |g| {
            let t = g.get("type").and_then(Value::as_str).unwrap_or_default();
            let is_poly = t == "Polygon";
            let is_multi = t == "MultiPolygon";
            if let Some(coords) = g.get_mut("coordinates") {
                if is_poly {
                    orient_polygon(coords);
                } else if is_multi {
                    if let Value::Array(polys) = coords {
                        for p in polys.iter_mut() {
                            orient_polygon(p);
                        }
                    }
                }
            }
        });
    }

    // 4. bbox members.
    match opts.bbox {
        BboxMode::Keep => {}
        BboxMode::Strip => strip_bbox(&mut root),
        BboxMode::Add | BboxMode::Features => {
            if opts.bbox == BboxMode::Features && is_collection {
                if let Some(Value::Array(feats)) = root.get_mut("features") {
                    for f in feats.iter_mut() {
                        if let Some(bb) = bbox_of(f) {
                            if let Some(m) = f.as_object_mut() {
                                m.insert("bbox".to_string(), bb);
                            }
                        }
                    }
                }
            }
            if let Some(bb) = bbox_of(&root) {
                if let Some(m) = root.as_object_mut() {
                    m.insert("bbox".to_string(), bb);
                }
            }
        }
    }

    // 5. Re-key.
    match opts.key_order {
        KeyOrder::Keep => {}
        KeyOrder::Canonical => canonicalize(&mut root),
        KeyOrder::Alpha => alphabetize(&mut root),
    }

    write_out(&root, opts)
}

/// Surface-facing entrypoint: the chat/CLI block and the browser wrapper both
/// call this with raw values, so option parsing and its error messages live in
/// exactly one place ([`FormatOptions::from_strings`]).
#[allow(clippy::too_many_arguments)]
pub fn run(
    input: &str,
    indent: i64,
    indent_char: &str,
    precision: i64,
    key_order: &str,
    bbox: &str,
    winding: &str,
    keep_properties: &str,
    drop_properties: &str,
    drop_empty_properties: bool,
    validate: bool,
) -> Result<String, String> {
    let opts = FormatOptions::from_strings(
        indent,
        indent_char,
        precision,
        key_order,
        bbox,
        winding,
        keep_properties,
        drop_properties,
        drop_empty_properties,
        validate,
    )?;
    format_geojson(input, &opts)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FC: &str = r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"name":"A","note":"","rank":2},"geometry":{"type":"Point","coordinates":[12.3456789,-9.8765432]}}]}"#;

    fn opts() -> FormatOptions {
        FormatOptions::default()
    }

    fn parse(s: &str) -> Value {
        serde_json::from_str(s).unwrap()
    }

    #[test]
    fn pretty_prints_with_two_space_indent() {
        let out = format_geojson(FC, &opts()).unwrap();
        assert!(
            out.starts_with("{\n  \"type\": \"FeatureCollection\""),
            "{out}"
        );
        assert_eq!(parse(&out)["features"][0]["properties"]["name"], "A");
    }

    #[test]
    fn minifies_when_indent_is_zero() {
        let o = FormatOptions {
            indent: 0,
            ..opts()
        };
        let out = format_geojson(FC, &o).unwrap();
        assert!(!out.contains('\n'));
        assert!(out.starts_with("{\"type\":\"FeatureCollection\""), "{out}");
    }

    #[test]
    fn indents_with_tabs() {
        let o = FormatOptions {
            indent: 1,
            indent_char: IndentChar::Tab,
            ..opts()
        };
        let out = format_geojson(FC, &o).unwrap();
        assert!(out.contains("\n\t\"type\": \"FeatureCollection\""), "{out}");
    }

    #[test]
    fn rounds_coordinate_precision() {
        let o = FormatOptions {
            indent: 0,
            precision: 4,
            ..opts()
        };
        let out = format_geojson(FC, &o).unwrap();
        assert!(out.contains("[12.3457,-9.8765]"), "{out}");
    }

    #[test]
    fn precision_zero_yields_whole_degrees() {
        let o = FormatOptions {
            indent: 0,
            precision: 0,
            ..opts()
        };
        let out = format_geojson(FC, &o).unwrap();
        assert!(out.contains("[12.0,-10.0]"), "{out}");
    }

    #[test]
    fn keeps_the_document_shape_for_a_bare_geometry() {
        let o = FormatOptions {
            indent: 0,
            ..opts()
        };
        let out = format_geojson(r#"{"type":"Point","coordinates":[1,2]}"#, &o).unwrap();
        assert_eq!(out, r#"{"type":"Point","coordinates":[1,2]}"#);
    }

    #[test]
    fn canonical_key_order_rewrites_geojson_members_only() {
        let src = r#"{"geometry":{"coordinates":[1,2],"type":"Point"},"properties":{"z":1,"a":2},"type":"Feature","id":7}"#;
        let o = FormatOptions {
            indent: 0,
            key_order: KeyOrder::Canonical,
            ..opts()
        };
        let out = format_geojson(src, &o).unwrap();
        assert_eq!(
            out,
            r#"{"type":"Feature","id":7,"geometry":{"type":"Point","coordinates":[1,2]},"properties":{"z":1,"a":2}}"#
        );
    }

    #[test]
    fn alpha_key_order_sorts_everything_including_properties() {
        let src = r#"{"type":"Feature","properties":{"z":1,"a":2},"geometry":{"type":"Point","coordinates":[1,2]}}"#;
        let o = FormatOptions {
            indent: 0,
            key_order: KeyOrder::Alpha,
            ..opts()
        };
        let out = format_geojson(src, &o).unwrap();
        assert_eq!(
            out,
            r#"{"geometry":{"coordinates":[1,2],"type":"Point"},"properties":{"a":2,"z":1},"type":"Feature"}"#
        );
    }

    #[test]
    fn adds_a_two_dimensional_bbox() {
        let o = FormatOptions {
            indent: 0,
            bbox: BboxMode::Add,
            ..opts()
        };
        let src = r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{},"geometry":{"type":"LineString","coordinates":[[0,1],[4,-2]]}}]}"#;
        let out = format_geojson(src, &o).unwrap();
        assert_eq!(parse(&out)["bbox"], parse("[0.0,-2.0,4.0,1.0]"));
    }

    #[test]
    fn adds_a_three_dimensional_bbox_when_every_position_has_altitude() {
        let o = FormatOptions {
            indent: 0,
            bbox: BboxMode::Features,
            ..opts()
        };
        let src = r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{},"geometry":{"type":"LineString","coordinates":[[0,1,10],[4,-2,30]]}}]}"#;
        let out = format_geojson(src, &o).unwrap();
        let v = parse(&out);
        assert_eq!(v["bbox"], parse("[0.0,-2.0,10.0,4.0,1.0,30.0]"));
        assert_eq!(
            v["features"][0]["bbox"],
            parse("[0.0,-2.0,10.0,4.0,1.0,30.0]")
        );
    }

    #[test]
    fn strips_every_bbox() {
        let src = r#"{"type":"FeatureCollection","bbox":[0,0,1,1],"features":[{"type":"Feature","bbox":[0,0,1,1],"properties":{},"geometry":{"type":"Point","coordinates":[0,0]}}]}"#;
        let o = FormatOptions {
            indent: 0,
            bbox: BboxMode::Strip,
            ..opts()
        };
        let out = format_geojson(src, &o).unwrap();
        assert!(!out.contains("bbox"), "{out}");
    }

    #[test]
    fn rewinds_polygon_rings_to_the_right_hand_rule() {
        // Exterior ring given clockwise; hole given counterclockwise — both wrong.
        let src = r#"{"type":"Polygon","coordinates":[[[0,0],[0,10],[10,10],[10,0],[0,0]],[[2,2],[4,2],[4,4],[2,4],[2,2]]]}"#;
        let o = FormatOptions {
            indent: 0,
            winding: Winding::Rfc7946,
            ..opts()
        };
        let out = format_geojson(src, &o).unwrap();
        let v = parse(&out);
        let outer: Vec<Value> = v["coordinates"][0].as_array().unwrap().clone();
        let hole: Vec<Value> = v["coordinates"][1].as_array().unwrap().clone();
        assert!(
            ring_area(&outer) > 0.0,
            "exterior ring must be counterclockwise"
        );
        assert!(ring_area(&hole) < 0.0, "hole must be clockwise");
    }

    #[test]
    fn keeps_only_the_listed_properties() {
        let o = FormatOptions {
            indent: 0,
            keep_properties: vec!["name".into()],
            ..opts()
        };
        let out = format_geojson(FC, &o).unwrap();
        assert!(out.contains(r#""properties":{"name":"A"}"#), "{out}");
    }

    #[test]
    fn drops_listed_and_empty_properties() {
        let o = FormatOptions {
            indent: 0,
            drop_properties: vec!["rank".into()],
            drop_empty_properties: true,
            ..opts()
        };
        let out = format_geojson(FC, &o).unwrap();
        assert!(out.contains(r#""properties":{"name":"A"}"#), "{out}");
    }

    #[test]
    fn rejects_invalid_json() {
        let err = format_geojson("{\"type\":", &opts()).unwrap_err();
        assert!(err.starts_with("input is not valid JSON"), "{err}");
    }

    #[test]
    fn rejects_an_unclosed_polygon_ring() {
        let src = r#"{"type":"Polygon","coordinates":[[[0,0],[0,1],[1,1],[1,0]]]}"#;
        let err = format_geojson(src, &opts()).unwrap_err();
        assert!(
            err.contains("(root).coordinates[0]: the ring is not closed"),
            "{err}"
        );
    }

    #[test]
    fn rejects_an_out_of_range_longitude_and_hints_at_swapped_pairs() {
        let src = r#"{"type":"Point","coordinates":[200,10]}"#;
        let err = format_geojson(src, &opts()).unwrap_err();
        assert!(err.contains("longitude 200 is outside -180..180"), "{err}");
        assert!(err.contains("swapped"), "{err}");
    }

    #[test]
    fn reports_the_path_of_a_bad_feature() {
        let src = r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{},"geometry":{"type":"Point","coordinates":[0,0]}},{"type":"Feature","properties":{}}]}"#;
        let err = format_geojson(src, &opts()).unwrap_err();
        assert!(
            err.contains("features[1]: a Feature must have a \"geometry\" member"),
            "{err}"
        );
    }

    #[test]
    fn validation_can_be_turned_off() {
        let src = r#"{"type":"Point","coordinates":[200,10]}"#;
        let o = FormatOptions {
            indent: 0,
            validate: false,
            ..opts()
        };
        assert_eq!(
            format_geojson(src, &o).unwrap(),
            r#"{"type":"Point","coordinates":[200,10]}"#
        );
    }

    #[test]
    fn rejects_an_empty_input() {
        let err = format_geojson("   ", &opts()).unwrap_err();
        assert_eq!(err, "expected GeoJSON text, got an empty input");
    }

    #[test]
    fn rejects_a_non_geojson_type() {
        let err = format_geojson(r#"{"type":"Topology","objects":{}}"#, &opts()).unwrap_err();
        assert!(
            err.contains("unsupported GeoJSON type \"Topology\""),
            "{err}"
        );
    }

    #[test]
    fn option_parsing_reports_unknown_values() {
        let err = FormatOptions::from_strings(
            2, "space", -1, "sorted", "keep", "keep", "", "", false, true,
        )
        .unwrap_err();
        assert!(err.contains("unknown key_order \"sorted\""), "{err}");
    }

    #[test]
    fn option_parsing_accepts_a_comma_separated_property_list() {
        let o = FormatOptions::from_strings(
            4,
            "tab",
            6,
            "canonical",
            "add",
            "rfc7946",
            "name, id",
            "note",
            true,
            false,
        )
        .unwrap();
        assert_eq!(
            o.keep_properties,
            vec!["name".to_string(), "id".to_string()]
        );
        assert_eq!(o.drop_properties, vec!["note".to_string()]);
        assert_eq!(o.indent_char, IndentChar::Tab);
        assert_eq!(o.bbox, BboxMode::Add);
        assert!(!o.validate);
    }
}
