//! gizza-ai/location-history-parser core — turn a Google Takeout / Dawarich
//! location-history JSON export into a per-day summary of places visited and
//! distance traveled. Pure Rust (`serde_json` only, no wafer/wasm-bindgen deps)
//! so the same logic backs the chat block, the CLI, and the browser page.
//!
//! Accepted inputs (auto-detected):
//!   * Google classic `Records.json` — `{ "locations": [ { latitudeE7,
//!     longitudeE7, timestamp | timestampMs }, … ] }`.
//!   * Google Semantic Location History — `{ "timelineObjects": [ { placeVisit
//!     }, { activitySegment }, … ] }` (or a bare array of those).
//!   * Google on-device Timeline — `{ "semanticSegments": [ { visit } |
//!     { activity } | { timelinePath }, … ] }` (or a bare array of segments).
//!   * Dawarich / generic point arrays — `[ { latitude, longitude, timestamp },
//!     … ]`, and GeoJSON `Point` FeatureCollections with a time property.
//!
//! Places: named visits when the export carries them; otherwise stop detection
//! (dwell + radius) turns a raw GPS stream into discrete visited places.
//! Distance: an activity's own `distance`/`distanceMeters` when present, else
//! the great-circle (haversine) sum of consecutive points.

use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// Mean Earth radius in metres (WGS84 sphere) used for haversine distances.
pub const EARTH_R_M: f64 = 6_371_000.0;

/// Distance unit for the report.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Unit {
    Km,
    Mi,
}

impl Unit {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "km" | "kilometers" | "kilometres" => Ok(Unit::Km),
            "mi" | "miles" | "mile" => Ok(Unit::Mi),
            other => Err(format!("invalid unit '{other}': expected one of km, mi")),
        }
    }
    /// Convert metres into this unit.
    fn from_m(self, m: f64) -> f64 {
        match self {
            Unit::Km => m / 1000.0,
            Unit::Mi => m / 1609.344,
        }
    }
    fn label(self) -> &'static str {
        match self {
            Unit::Km => "km",
            Unit::Mi => "mi",
        }
    }
}

/// Output rendering format.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Output {
    Text,
    Csv,
    Json,
}

impl Output {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "text" | "summary" => Ok(Output::Text),
            "csv" => Ok(Output::Csv),
            "json" => Ok(Output::Json),
            other => Err(format!(
                "invalid output '{other}': expected one of text, csv, json"
            )),
        }
    }
}

/// Parsed run options.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// Hours to add to UTC timestamps before bucketing into calendar days.
    pub utc_offset_hours: f64,
    pub unit: Unit,
    /// Stop-detection proximity radius in metres (raw GPS streams only).
    pub place_radius_m: f64,
    /// Minimum dwell in minutes for a cluster to count as a visited place.
    pub min_stay_min: f64,
    pub output: Output,
}

/// A visited place (named export visit, or a synthesized stop).
#[derive(Debug, Clone)]
struct Visit {
    epoch: i64,
    name: Option<String>,
}

/// A movement segment carrying its own distance in metres.
#[derive(Debug, Clone)]
struct Trip {
    epoch: i64,
    meters: f64,
}

/// A raw timestamped coordinate.
#[derive(Debug, Clone, Copy)]
struct Point {
    epoch: i64,
    lat: f64,
    lon: f64,
}

/// Everything pulled out of the input, before per-day aggregation.
struct Parsed {
    visits: Vec<Visit>,
    trips: Vec<Trip>,
    points: Vec<Point>,
    format: &'static str,
}

/// One calendar day of the summary.
#[derive(Debug, Clone)]
pub struct DaySummary {
    pub date: String,
    pub places: usize,
    /// Named places for this day, de-duplicated in first-seen order (empty for
    /// stop-detected streams that have no names).
    pub place_names: Vec<String>,
    pub distance_m: f64,
}

/// The full report.
#[derive(Debug, Clone)]
pub struct Report {
    pub format: &'static str,
    pub days: Vec<DaySummary>,
    pub total_places: usize,
    pub total_distance_m: f64,
}

// ---------------------------------------------------------------------------
// Time + coordinate parsing helpers
// ---------------------------------------------------------------------------

/// Days since 1970-01-01 for a proleptic-Gregorian date (Howard Hinnant).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = y - era * 400; // [0, 399]
    let mp = if m > 2 { m - 3 } else { m + 9 }; // [0, 11]
    let doy = (153 * mp + 2) / 5 + d - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    era * 146097 + doe - 719468
}

/// Inverse of `days_from_civil`: days since epoch → (year, month, day).
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = z - era * 146097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Interpret an integer as either seconds or milliseconds since the epoch.
/// Timestamps in ms exceed 1e11 for any date past 1973, which no plausible
/// seconds value reaches until the year 5138, so the threshold cleanly
/// separates the two encodings.
fn epoch_from_int(v: i64) -> i64 {
    if v.abs() >= 100_000_000_000 {
        v / 1000
    } else {
        v
    }
}

/// Parse an ISO-8601 / RFC-3339 timestamp into epoch seconds (UTC).
/// Handles `Z`, `±HH:MM`, `±HHMM`, fractional seconds, and a space or `T`
/// separator. A bare `YYYY-MM-DD` is treated as midnight UTC.
fn epoch_from_iso(s: &str) -> Option<i64> {
    let s = s.trim();
    let b = s.as_bytes();
    if s.len() < 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let year: i64 = s.get(0..4)?.parse().ok()?;
    let month: i64 = s.get(5..7)?.parse().ok()?;
    let day: i64 = s.get(8..10)?.parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let mut secs = days_from_civil(year, month, day) * 86400;

    if s.len() > 10 {
        // Time part follows a 'T' or ' ' separator at index 10.
        let rest = &s[11..];
        let (time_part, offset_secs) = split_offset(rest)?;
        let mut it = time_part.split(':');
        let hh: i64 = it.next()?.parse().ok()?;
        let mm: i64 = it.next().unwrap_or("0").parse().ok()?;
        // Seconds may carry a fractional part; take the integer portion.
        let ss: i64 = it
            .next()
            .unwrap_or("0")
            .split('.')
            .next()
            .unwrap_or("0")
            .parse()
            .ok()?;
        secs += hh * 3600 + mm * 60 + ss;
        secs -= offset_secs;
    }
    Some(secs)
}

/// Split a time string into its clock portion and UTC offset in seconds.
fn split_offset(t: &str) -> Option<(&str, i64)> {
    if let Some(stripped) = t.strip_suffix(['Z', 'z']) {
        return Some((stripped, 0));
    }
    // Find a trailing '+' or '-' (offset sign), scanning from the right.
    let bytes = t.as_bytes();
    for i in (1..bytes.len()).rev() {
        if bytes[i] == b'+' || bytes[i] == b'-' {
            let sign = if bytes[i] == b'+' { 1 } else { -1 };
            let digits: String = t[i + 1..].chars().filter(|c| c.is_ascii_digit()).collect();
            let (oh, om): (i64, i64) = match digits.len() {
                4 => (digits[0..2].parse().ok()?, digits[2..4].parse().ok()?),
                2 => (digits.parse().ok()?, 0i64),
                _ => return None,
            };
            return Some((&t[..i], sign * (oh * 3600 + om * 60)));
        }
    }
    Some((t, 0)) // no offset → treat as UTC
}

/// Parse a JSON value (number or string) into epoch seconds.
fn epoch_of(v: &Value) -> Option<i64> {
    match v {
        Value::Number(n) => n
            .as_i64()
            .map(epoch_from_int)
            .or_else(|| n.as_f64().map(|f| epoch_from_int(f as i64))),
        Value::String(s) => {
            let t = s.trim();
            if !t.is_empty() && t.chars().all(|c| c.is_ascii_digit() || c == '-' || c == '+') {
                t.parse::<i64>().ok().map(epoch_from_int)
            } else {
                epoch_from_iso(t)
            }
        }
        _ => None,
    }
}

/// First present timestamp among the given keys of an object.
fn epoch_from_keys(obj: &Map<String, Value>, keys: &[&str]) -> Option<i64> {
    keys.iter().find_map(|k| obj.get(*k).and_then(epoch_of))
}

/// A number from any of the given keys (accepts numeric strings).
fn num_from_keys(obj: &Map<String, Value>, keys: &[&str]) -> Option<f64> {
    keys.iter().find_map(|k| match obj.get(*k) {
        Some(Value::Number(n)) => n.as_f64(),
        Some(Value::String(s)) => s.trim().parse::<f64>().ok(),
        _ => None,
    })
}

/// Parse a `latLng` string such as `"40.71,-74.0"`, `"geo:40.71,-74.0"`, or
/// `"40.71°, -74.0°"` into `(lat, lon)`.
fn parse_latlng_str(s: &str) -> Option<(f64, f64)> {
    let s = s.trim().strip_prefix("geo:").unwrap_or_else(|| s.trim());
    let mut parts = s.split(',');
    let lat = parts.next()?.trim().trim_end_matches('°').trim().parse().ok()?;
    let lon = parts.next()?.trim().trim_end_matches('°').trim().parse().ok()?;
    Some((lat, lon))
}

/// Extract `(lat, lon)` from an object, trying E7 integers, plain fields, and
/// `latLng` strings.
fn latlng_from_obj(obj: &Map<String, Value>) -> Option<(f64, f64)> {
    if let (Some(lat), Some(lon)) = (
        num_from_keys(obj, &["latitudeE7", "latE7"]),
        num_from_keys(obj, &["longitudeE7", "lonE7", "lngE7"]),
    ) {
        return Some((lat / 1e7, lon / 1e7));
    }
    if let (Some(lat), Some(lon)) = (
        num_from_keys(obj, &["latitude", "lat"]),
        num_from_keys(obj, &["longitude", "lon", "lng", "long"]),
    ) {
        return Some((lat, lon));
    }
    if let Some(Value::String(s)) = obj.get("latLng").or_else(|| obj.get("latlng")) {
        return parse_latlng_str(s);
    }
    None
}

// ---------------------------------------------------------------------------
// Input format detection + extraction
// ---------------------------------------------------------------------------

fn parse_input(json: &str) -> Result<Parsed, String> {
    if json.trim().is_empty() {
        return Err(
            "no input provided: paste a Google Takeout or Dawarich location-history JSON export"
                .into(),
        );
    }
    let root: Value = serde_json::from_str(json).map_err(|e| format!("invalid JSON: {e}"))?;

    let mut p = Parsed {
        visits: Vec::new(),
        trips: Vec::new(),
        points: Vec::new(),
        format: "unknown",
    };

    match &root {
        Value::Object(obj) => {
            if let Some(Value::Array(locs)) = obj.get("locations") {
                p.format = "Google Records (classic Location History)";
                extract_records(locs, &mut p);
            } else if let Some(Value::Array(objs)) = obj.get("timelineObjects") {
                p.format = "Google Semantic Location History";
                extract_semantic(objs, &mut p);
            } else if let Some(Value::Array(segs)) = obj.get("semanticSegments") {
                p.format = "Google Timeline (on-device)";
                extract_timeline(segs, &mut p);
            } else if obj.get("type").and_then(Value::as_str) == Some("FeatureCollection") {
                p.format = "GeoJSON point features";
                if let Some(Value::Array(feats)) = obj.get("features") {
                    extract_geojson(feats, &mut p);
                }
            } else {
                return Err("unrecognized JSON: expected a Google Records export (a \"locations\" array), Semantic Location History (\"timelineObjects\"), on-device Timeline (\"semanticSegments\"), a GeoJSON FeatureCollection, or a top-level array of location points".into());
            }
        }
        Value::Array(arr) => {
            let first = arr.iter().find_map(Value::as_object);
            if let Some(f) = first {
                if f.contains_key("placeVisit") || f.contains_key("activitySegment") {
                    p.format = "Google Semantic Location History";
                    extract_semantic(arr, &mut p);
                } else if f.contains_key("visit")
                    || f.contains_key("activity")
                    || f.contains_key("timelinePath")
                {
                    p.format = "Google Timeline (on-device)";
                    extract_timeline(arr, &mut p);
                } else {
                    p.format = "Location point array (Dawarich / generic)";
                    extract_points(arr, &mut p);
                }
            } else {
                return Err("empty array: no location records to summarize".into());
            }
        }
        _ => {
            return Err(
                "unrecognized JSON: expected an object or array of location records".into(),
            )
        }
    }

    if p.visits.is_empty() && p.trips.is_empty() && p.points.is_empty() {
        return Err(format!(
            "no usable records found in the {} data: each entry needs a timestamp and coordinates",
            p.format
        ));
    }
    Ok(p)
}

/// Classic Records.json `locations` array → points.
fn extract_records(locs: &[Value], p: &mut Parsed) {
    for loc in locs {
        if let Some(obj) = loc.as_object() {
            if let (Some(epoch), Some((lat, lon))) = (
                epoch_from_keys(obj, &["timestamp", "timestampMs"]),
                latlng_from_obj(obj),
            ) {
                p.points.push(Point { epoch, lat, lon });
            }
        }
    }
}

/// Semantic Location History `timelineObjects` → visits + trips.
fn extract_semantic(objs: &[Value], p: &mut Parsed) {
    for entry in objs {
        let obj = match entry.as_object() {
            Some(o) => o,
            None => continue,
        };
        if let Some(pv) = obj.get("placeVisit").and_then(Value::as_object) {
            let dur = pv.get("duration").and_then(Value::as_object);
            let epoch = dur
                .and_then(|d| epoch_from_keys(d, &["startTimestamp", "startTimestampMs"]))
                .or_else(|| epoch_from_keys(pv, &["startTimestamp", "startTimestampMs"]));
            if let Some(epoch) = epoch {
                let name = pv.get("location").and_then(Value::as_object).and_then(|l| {
                    l.get("name")
                        .and_then(Value::as_str)
                        .or_else(|| l.get("address").and_then(Value::as_str))
                        .map(clean_name)
                        .filter(|s| !s.is_empty())
                });
                p.visits.push(Visit { epoch, name });
            }
        }
        if let Some(seg) = obj.get("activitySegment").and_then(Value::as_object) {
            let dur = seg.get("duration").and_then(Value::as_object);
            let epoch = dur
                .and_then(|d| epoch_from_keys(d, &["startTimestamp", "startTimestampMs"]))
                .or_else(|| epoch_from_keys(seg, &["startTimestamp", "startTimestampMs"]));
            if let Some(epoch) = epoch {
                let meters = num_from_keys(seg, &["distance", "distanceMeters"]).or_else(|| {
                    let a = seg.get("startLocation").and_then(Value::as_object)?;
                    let b = seg.get("endLocation").and_then(Value::as_object)?;
                    Some(haversine_m(latlng_from_obj(a)?, latlng_from_obj(b)?))
                });
                if let Some(meters) = meters {
                    p.trips.push(Trip { epoch, meters });
                }
            }
        }
    }
}

/// On-device Timeline `semanticSegments` → visits + trips + path points.
fn extract_timeline(segs: &[Value], p: &mut Parsed) {
    for entry in segs {
        let seg = match entry.as_object() {
            Some(o) => o,
            None => continue,
        };
        let start = epoch_from_keys(seg, &["startTime", "startTimestamp"]);
        if seg.get("visit").is_some() {
            if let Some(epoch) = start {
                let cand = seg
                    .get("visit")
                    .and_then(Value::as_object)
                    .and_then(|v| v.get("topCandidate"))
                    .and_then(Value::as_object);
                let name = cand.and_then(|c| {
                    c.get("semanticType")
                        .and_then(Value::as_str)
                        .map(clean_name)
                        .filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("unknown"))
                });
                p.visits.push(Visit { epoch, name });
            }
        }
        if let Some(a) = seg.get("activity").and_then(Value::as_object) {
            if let Some(epoch) = start {
                let meters = num_from_keys(a, &["distanceMeters", "distance"]).or_else(|| {
                    let s = a.get("start").and_then(Value::as_object)?;
                    let e = a.get("end").and_then(Value::as_object)?;
                    Some(haversine_m(latlng_from_obj(s)?, latlng_from_obj(e)?))
                });
                if let Some(meters) = meters {
                    p.trips.push(Trip { epoch, meters });
                }
            }
        }
        if let Some(path) = seg.get("timelinePath").and_then(Value::as_array) {
            for pt in path {
                if let Some(po) = pt.as_object() {
                    let coords = po
                        .get("point")
                        .and_then(Value::as_str)
                        .and_then(parse_latlng_str)
                        .or_else(|| latlng_from_obj(po));
                    let epoch = epoch_from_keys(po, &["time", "timestamp"]).or(start);
                    if let (Some((lat, lon)), Some(epoch)) = (coords, epoch) {
                        p.points.push(Point { epoch, lat, lon });
                    }
                }
            }
        }
    }
}

/// GeoJSON `Point` features with a time property → points.
fn extract_geojson(feats: &[Value], p: &mut Parsed) {
    for f in feats {
        let obj = match f.as_object() {
            Some(o) => o,
            None => continue,
        };
        let coords = obj
            .get("geometry")
            .and_then(Value::as_object)
            .filter(|g| g.get("type").and_then(Value::as_str) == Some("Point"))
            .and_then(|g| g.get("coordinates"))
            .and_then(Value::as_array);
        let props = obj.get("properties").and_then(Value::as_object);
        if let (Some(c), Some(props)) = (coords, props) {
            if let (Some(lon), Some(lat)) =
                (c.first().and_then(Value::as_f64), c.get(1).and_then(Value::as_f64))
            {
                if let Some(epoch) = epoch_from_keys(props, &["timestamp", "time", "ts", "date"]) {
                    p.points.push(Point { epoch, lat, lon });
                }
            }
        }
    }
}

/// Generic point array (Dawarich, etc.) → points.
fn extract_points(arr: &[Value], p: &mut Parsed) {
    for entry in arr {
        if let Some(obj) = entry.as_object() {
            if let (Some(epoch), Some((lat, lon))) = (
                epoch_from_keys(obj, &["timestamp", "time", "ts", "date"]),
                latlng_from_obj(obj),
            ) {
                p.points.push(Point { epoch, lat, lon });
            }
        }
    }
}

/// Trim and collapse internal whitespace in a place name.
fn clean_name(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ---------------------------------------------------------------------------
// Geometry
// ---------------------------------------------------------------------------

/// Great-circle distance between two `(lat, lon)` points, in metres.
fn haversine_m(a: (f64, f64), b: (f64, f64)) -> f64 {
    let (lat1, lon1) = a;
    let (lat2, lon2) = b;
    let (r1, r2) = (lat1.to_radians(), lat2.to_radians());
    let dlat = (lat2 - lat1).to_radians();
    let dlon = (lon2 - lon1).to_radians();
    let h = (dlat / 2.0).sin().powi(2) + r1.cos() * r2.cos() * (dlon / 2.0).sin().powi(2);
    2.0 * EARTH_R_M * h.sqrt().min(1.0).asin()
}

// ---------------------------------------------------------------------------
// Aggregation
// ---------------------------------------------------------------------------

/// Local calendar-day key `YYYY-MM-DD` for an epoch, shifted by the offset.
fn day_key(epoch: i64, offset_secs: i64) -> String {
    let days = (epoch + offset_secs).div_euclid(86400);
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}")
}

#[derive(Default)]
struct DayAcc {
    places: usize,
    names: Vec<String>,
    distance_m: f64,
}

/// Build the per-day report from parsed input.
fn summarize_parsed(p: &Parsed, opts: &Options) -> Report {
    let offset = (opts.utc_offset_hours * 3600.0).round() as i64;
    let mut days: BTreeMap<String, DayAcc> = BTreeMap::new();

    // --- Places ---
    if !p.visits.is_empty() {
        for v in &p.visits {
            let acc = days.entry(day_key(v.epoch, offset)).or_default();
            acc.places += 1;
            if let Some(name) = &v.name {
                if !acc.names.iter().any(|n| n == name) {
                    acc.names.push(name.clone());
                }
            }
        }
    } else if !p.points.is_empty() {
        for stop in detect_stops(&p.points, opts) {
            days.entry(day_key(stop, offset)).or_default().places += 1;
        }
    }

    // --- Distance ---
    if !p.trips.is_empty() {
        for t in &p.trips {
            days.entry(day_key(t.epoch, offset)).or_default().distance_m += t.meters.max(0.0);
        }
    } else if !p.points.is_empty() {
        let mut pts = p.points.clone();
        pts.sort_by_key(|pt| pt.epoch);
        for w in pts.windows(2) {
            let d = haversine_m((w[0].lat, w[0].lon), (w[1].lat, w[1].lon));
            days.entry(day_key(w[0].epoch, offset)).or_default().distance_m += d;
        }
    }

    let mut out_days = Vec::new();
    let mut total_places = 0;
    let mut total_distance_m = 0.0;
    for (date, acc) in days {
        total_places += acc.places;
        total_distance_m += acc.distance_m;
        out_days.push(DaySummary {
            date,
            places: acc.places,
            place_names: acc.names,
            distance_m: acc.distance_m,
        });
    }

    Report {
        format: p.format,
        days: out_days,
        total_places,
        total_distance_m,
    }
}

/// Greedy stop detection: a maximal run of consecutive points that stay within
/// `place_radius_m` of the run's anchor and span at least `min_stay_min`
/// counts as one visited place. Returns each stop's start epoch.
fn detect_stops(points: &[Point], opts: &Options) -> Vec<i64> {
    let mut pts = points.to_vec();
    pts.sort_by_key(|p| p.epoch);
    let min_stay_s = (opts.min_stay_min * 60.0).max(0.0) as i64;
    let radius = opts.place_radius_m.max(0.0);
    let mut stops = Vec::new();
    let mut i = 0;
    let n = pts.len();
    while i < n {
        let anchor = (pts[i].lat, pts[i].lon);
        let mut j = i;
        while j + 1 < n && haversine_m(anchor, (pts[j + 1].lat, pts[j + 1].lon)) <= radius {
            j += 1;
        }
        if j > i && pts[j].epoch - pts[i].epoch >= min_stay_s {
            stops.push(pts[i].epoch);
        }
        i = j + 1;
    }
    stops
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

fn render(report: &Report, opts: &Options) -> String {
    match opts.output {
        Output::Text => render_text(report, opts),
        Output::Csv => render_csv(report, opts),
        Output::Json => render_json(report, opts),
    }
}

fn render_text(report: &Report, opts: &Options) -> String {
    let u = opts.unit;
    let mut out = String::new();
    for d in &report.days {
        out.push_str(&format!(
            "{} — {}, {:.2} {}\n",
            d.date,
            plural(d.places, "place", "places"),
            u.from_m(d.distance_m),
            u.label()
        ));
    }
    out.push('\n');
    out.push_str(&format!(
        "Total over {}: {}, {:.2} {}",
        plural(report.days.len(), "day", "days"),
        plural(report.total_places, "place", "places"),
        u.from_m(report.total_distance_m),
        u.label()
    ));
    out
}

fn render_csv(report: &Report, opts: &Options) -> String {
    let u = opts.unit;
    let mut out = format!("date,places,distance_{}\n", u.label());
    for d in &report.days {
        out.push_str(&format!("{},{},{:.2}\n", d.date, d.places, u.from_m(d.distance_m)));
    }
    out.trim_end_matches('\n').to_string()
}

fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

fn render_json(report: &Report, opts: &Options) -> String {
    let u = opts.unit;
    let days: Vec<Value> = report
        .days
        .iter()
        .map(|d| {
            serde_json::json!({
                "date": d.date,
                "places": d.places,
                "place_names": d.place_names,
                "distance": round2(u.from_m(d.distance_m)),
            })
        })
        .collect();
    let v = serde_json::json!({
        "source_format": report.format,
        "unit": u.label(),
        "days": days,
        "totals": {
            "days": report.days.len(),
            "places": report.total_places,
            "distance": round2(u.from_m(report.total_distance_m)),
        }
    });
    serde_json::to_string_pretty(&v).unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Public entry points
// ---------------------------------------------------------------------------

/// Parse + summarize, returning the structured report.
pub fn summarize(json: &str, opts: &Options) -> Result<Report, String> {
    let parsed = parse_input(json)?;
    Ok(summarize_parsed(&parsed, opts))
}

/// Full pipeline used by the chat block, CLI, and web wrapper: parse string
/// options, summarize, and render the chosen output format.
pub fn run(
    json: &str,
    output: &str,
    unit: &str,
    utc_offset_hours: f64,
    place_radius_m: f64,
    min_stay_min: f64,
) -> Result<String, String> {
    let opts = Options {
        utc_offset_hours,
        unit: Unit::parse(unit)?,
        place_radius_m,
        min_stay_min,
        output: Output::parse(output)?,
    };
    let report = summarize(json, &opts)?;
    Ok(render(&report, &opts))
}

#[cfg(test)]
mod tests {
    use super::*;

    const OPTS: Options = Options {
        utc_offset_hours: 0.0,
        unit: Unit::Km,
        place_radius_m: 150.0,
        min_stay_min: 10.0,
        output: Output::Text,
    };

    // Classic Semantic Location History — explicit visits + activity distances.
    const SEMANTIC: &str = r#"{"timelineObjects":[
      {"placeVisit":{"location":{"latitudeE7":400000000,"longitudeE7":-740000000,"name":"Home"},"duration":{"startTimestamp":"2024-01-01T08:00:00Z"}}},
      {"activitySegment":{"distance":5000,"duration":{"startTimestamp":"2024-01-01T09:00:00Z"}}},
      {"placeVisit":{"location":{"latitudeE7":401000000,"longitudeE7":-740000000,"name":"Office"},"duration":{"startTimestamp":"2024-01-01T10:00:00Z"}}},
      {"activitySegment":{"distanceMeters":"3000","duration":{"startTimestamp":"2024-01-02T09:00:00Z"}}},
      {"placeVisit":{"location":{"latitudeE7":400000000,"longitudeE7":-740000000,"name":"Home"},"duration":{"startTimestamp":"2024-01-02T18:00:00Z"}}}
    ]}"#;

    #[test]
    fn semantic_text_exact() {
        let out = run(SEMANTIC, "text", "km", 0.0, 150.0, 10.0).unwrap();
        assert_eq!(
            out,
            "2024-01-01 — 2 places, 5.00 km\n\
             2024-01-02 — 1 place, 3.00 km\n\
             \n\
             Total over 2 days: 3 places, 8.00 km"
        );
    }

    #[test]
    fn semantic_report_fields() {
        let r = summarize(SEMANTIC, &OPTS).unwrap();
        assert_eq!(r.format, "Google Semantic Location History");
        assert_eq!(r.days.len(), 2);
        assert_eq!(r.total_places, 3);
        assert_eq!(r.days[0].place_names, vec!["Home", "Office"]);
        assert!((r.total_distance_m - 8000.0).abs() < 1e-6);
    }

    #[test]
    fn semantic_miles() {
        let out = run(SEMANTIC, "text", "mi", 0.0, 150.0, 10.0).unwrap();
        // 5000 m = 3.11 mi, total 8000 m = 4.97 mi.
        assert!(out.contains("2024-01-01 — 2 places, 3.11 mi"));
        assert!(out.contains("Total over 2 days: 3 places, 4.97 mi"));
    }

    #[test]
    fn semantic_csv() {
        let out = run(SEMANTIC, "csv", "km", 0.0, 150.0, 10.0).unwrap();
        assert_eq!(
            out,
            "date,places,distance_km\n\
             2024-01-01,2,5.00\n\
             2024-01-02,1,3.00"
        );
    }

    #[test]
    fn semantic_json() {
        let out = run(SEMANTIC, "json", "km", 0.0, 150.0, 10.0).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["totals"]["places"], 3);
        assert_eq!(v["days"][0]["place_names"][1], "Office");
        assert_eq!(v["days"][0]["distance"], 5.0);
        assert_eq!(v["unit"], "km");
    }

    #[test]
    fn utc_offset_shifts_day() {
        // A visit at 02:00 UTC lands on the previous day at UTC-5.
        let j = r#"{"timelineObjects":[
          {"placeVisit":{"location":{"latitudeE7":400000000,"longitudeE7":-740000000,"name":"X"},"duration":{"startTimestamp":"2024-01-02T02:00:00Z"}}}
        ]}"#;
        let utc = summarize(j, &OPTS).unwrap();
        assert_eq!(utc.days[0].date, "2024-01-02");
        let mut o = OPTS;
        o.utc_offset_hours = -5.0;
        let local = summarize(j, &o).unwrap();
        assert_eq!(local.days[0].date, "2024-01-01");
    }

    #[test]
    fn records_classic_points_distance_and_stops() {
        // Two points at Home (30 min apart) → one stop; then a 1° hop north.
        let j = r#"{"locations":[
          {"latitudeE7":400000000,"longitudeE7":-740000000,"timestampMs":"1704067200000"},
          {"latitudeE7":400000000,"longitudeE7":-740000000,"timestamp":"2024-01-01T00:30:00Z"},
          {"latitudeE7":410000000,"longitudeE7":-740000000,"timestamp":"2024-01-01T01:00:00Z"}
        ]}"#;
        let r = summarize(j, &OPTS).unwrap();
        assert_eq!(r.format, "Google Records (classic Location History)");
        assert_eq!(r.days.len(), 1);
        assert_eq!(r.days[0].places, 1); // the Home cluster is a stop
        // 1 degree of latitude ≈ 111.19 km.
        assert!((r.days[0].distance_m - 111_194.0).abs() < 200.0);
    }

    #[test]
    fn dawarich_generic_point_array() {
        let j = r#"[
          {"latitude":40.0,"longitude":-74.0,"timestamp":1704067200},
          {"latitude":40.5,"longitude":-74.0,"timestamp":1704070800}
        ]"#;
        let r = summarize(j, &OPTS).unwrap();
        assert_eq!(r.format, "Location point array (Dawarich / generic)");
        assert_eq!(r.days.len(), 1);
        assert!(r.total_distance_m > 50_000.0); // ~55 km north
    }

    #[test]
    fn on_device_timeline_latlng_strings() {
        let j = r#"{"semanticSegments":[
          {"startTime":"2024-03-10T08:00:00-05:00","endTime":"2024-03-10T09:00:00-05:00","visit":{"topCandidate":{"placeLocation":{"latLng":"40.0, -74.0"},"semanticType":"Home"}}},
          {"startTime":"2024-03-10T09:00:00-05:00","endTime":"2024-03-10T09:30:00-05:00","activity":{"distanceMeters":"2500"}}
        ]}"#;
        let r = summarize(j, &OPTS).unwrap();
        assert_eq!(r.format, "Google Timeline (on-device)");
        assert_eq!(r.days[0].date, "2024-03-10");
        assert_eq!(r.days[0].places, 1);
        assert_eq!(r.days[0].place_names, vec!["Home"]);
        assert!((r.total_distance_m - 2500.0).abs() < 1e-6);
    }

    #[test]
    fn geojson_point_features() {
        let j = r#"{"type":"FeatureCollection","features":[
          {"type":"Feature","geometry":{"type":"Point","coordinates":[-74.0,40.0]},"properties":{"timestamp":"2024-05-01T10:00:00Z"}},
          {"type":"Feature","geometry":{"type":"Point","coordinates":[-74.0,40.2]},"properties":{"timestamp":"2024-05-01T11:00:00Z"}}
        ]}"#;
        let r = summarize(j, &OPTS).unwrap();
        assert_eq!(r.format, "GeoJSON point features");
        assert_eq!(r.days[0].date, "2024-05-01");
        assert!(r.total_distance_m > 20_000.0);
    }

    #[test]
    fn errors_are_helpful() {
        assert!(run("", "text", "km", 0.0, 150.0, 10.0)
            .unwrap_err()
            .contains("no input"));
        assert!(run("{bad", "text", "km", 0.0, 150.0, 10.0)
            .unwrap_err()
            .contains("invalid JSON"));
        assert!(run(r#"{"foo":1}"#, "text", "km", 0.0, 150.0, 10.0)
            .unwrap_err()
            .contains("unrecognized JSON"));
        assert!(run("[]", "text", "km", 0.0, 150.0, 10.0)
            .unwrap_err()
            .contains("empty array"));
        assert!(run(r#"{"locations":[{"foo":1}]}"#, "text", "km", 0.0, 150.0, 10.0)
            .unwrap_err()
            .contains("no usable records"));
        assert!(Unit::parse("furlongs").is_err());
        assert!(Output::parse("xml").is_err());
    }

    #[test]
    fn civil_roundtrip() {
        for &(y, m, d) in &[(1970, 1, 1), (2024, 2, 29), (2024, 1, 1), (1999, 12, 31)] {
            let days = days_from_civil(y, m, d);
            assert_eq!(civil_from_days(days), (y, m, d));
        }
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        // 2024-01-01T00:00:00Z = 1704067200.
        assert_eq!(days_from_civil(2024, 1, 1) * 86400, 1_704_067_200);
    }
}
