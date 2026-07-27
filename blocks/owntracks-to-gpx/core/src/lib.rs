//! owntracks-to-gpx core — convert an OwnTracks location export into GPX 1.1.
//! Pure-Rust (`serde_json` only), no wafer/wasm-bindgen deps; shared by the chat
//! skill block and the web page.
//!
//! OwnTracks (the open-source phone location-logging app) records `location`
//! messages. Two shapes are exported in the wild and both are accepted here:
//!
//! - **JSON** — an array of location objects (`ocat --format json`, or the
//!   Recorder HTTP API `{"count":N,"data":[…]}`, or a single object). Each object
//!   is an MQTT/HTTP `location` payload: `{"_type":"location","lat":…,"lon":…,
//!   "tst":<epoch-seconds>,"alt":…,"acc":…,"vel":…,"cog":…,"batt":…,"tid":…}`.
//! - **`.rec` recorder format** — one message per line, tab-separated:
//!   `2024-01-01T12:00:00Z\t*\t{"_type":"location",…}`. The leading ISO timestamp
//!   is used as a fallback when a record omits `tst`; the JSON payload after the
//!   last tab carries the real fields.
//!
//! Only `_type == "location"` records are converted (transitions, waypoints,
//! lwt, etc. are skipped); a record with no `_type` but a lat/lon pair is still
//! accepted. Each point becomes a `<trkpt lat lon>` with `<ele>` (from `alt`) and
//! `<time>` (from `tst`, formatted as ISO-8601 UTC). All points land in one
//! `<trk>`; set `segment_gap_minutes` to break the track into `<trkseg>`s
//! wherever the gap between consecutive fixes exceeds that many minutes.

use serde_json::Value;

/// Custom namespace for the OwnTracks-specific per-point fields GPX 1.1 has no
/// core element for (accuracy, velocity, course, battery, tracker id).
const GPX_NS: &str = "http://www.topografix.com/GPX/1/1";
const OT_NS: &str = "http://owntracks.org/gpx/extensions/v1";
const XSI_NS: &str = "http://www.w3.org/2001/XMLSchema-instance";
const SCHEMA_LOCATION: &str =
    "http://www.topografix.com/GPX/1/1 http://www.topografix.com/GPX/1/1/gpx.xsd";

pub struct Options {
    /// Optional `<trk><name>`. Empty ⇒ no `<name>` element is emitted.
    pub track_name: String,
    /// Emit each point's accuracy/velocity/course/battery/tracker-id as
    /// `<extensions>` in the OwnTracks namespace (GPX 1.1 has no core element for
    /// any of them). Default true.
    pub include_extensions: bool,
    /// Start a new `<trkseg>` whenever the time gap between consecutive fixes
    /// exceeds this many minutes. 0 ⇒ keep every point in one segment.
    pub segment_gap_minutes: f64,
    /// Drop fixes whose reported accuracy (`acc`, metres) is worse than (numerically
    /// greater than) this. 0 ⇒ keep every point; points without an `acc` are kept.
    pub max_accuracy_meters: f64,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            track_name: String::new(),
            include_extensions: true,
            segment_gap_minutes: 0.0,
            max_accuracy_meters: 0.0,
        }
    }
}

#[derive(Default, Clone)]
struct Pt {
    lat: f64,
    lon: f64,
    /// Unix epoch seconds (`tst`), when present.
    tst: Option<i64>,
    /// ISO-8601 timestamp carried by a `.rec` line's first column, used only when
    /// `tst` is absent.
    iso_fallback: Option<String>,
    alt: Option<f64>,
    acc: Option<f64>,
    vel: Option<f64>,
    cog: Option<f64>,
    batt: Option<f64>,
    tid: Option<String>,
}

impl Pt {
    /// The point's timestamp as an epoch, if it can be determined (used for
    /// segment-gap splitting).
    fn epoch(&self) -> Option<i64> {
        self.tst.or_else(|| self.iso_fallback.as_deref().and_then(parse_iso_epoch))
    }

    /// The `<time>` string to emit: prefer `tst` (formatted UTC), else the raw
    /// `.rec` ISO string.
    fn time_str(&self) -> Option<String> {
        match self.tst {
            Some(t) => Some(epoch_to_iso(t)),
            None => self.iso_fallback.clone(),
        }
    }
}

/// Convert an OwnTracks export (`input`) into a GPX 1.1 document per `opt`.
pub fn convert(input: &str, opt: &Options) -> Result<String, String> {
    let trimmed = input.trim_start();
    if trimmed.is_empty() {
        return Err("input is empty — paste an OwnTracks JSON export or the contents of a .rec recorder file".to_string());
    }

    let mut points = if trimmed.starts_with('[') || trimmed.starts_with('{') {
        parse_json(trimmed)?
    } else {
        parse_rec(input)?
    };

    if opt.max_accuracy_meters > 0.0 {
        points.retain(|p| p.acc.map_or(true, |a| a <= opt.max_accuracy_meters));
    }

    if points.is_empty() {
        return Err(
            "no OwnTracks location records were found (expected _type=\"location\" objects with \
             lat/lon, or .rec lines whose JSON payload carries them). Transition, waypoint, and \
             other non-location messages are skipped; a too-strict accuracy filter can also \
             remove every point."
                .to_string(),
        );
    }

    Ok(build_gpx(&points, opt))
}

/// Extract location points from a JSON export: a top-level array, a Recorder API
/// object (`{"data":[…]}`), or a single location object.
fn parse_json(s: &str) -> Result<Vec<Pt>, String> {
    let v: Value = match serde_json::from_str(s) {
        Ok(v) => v,
        // Not a single JSON document — try NDJSON (one location object per line),
        // which some exports and log dumps produce.
        Err(e) => match parse_ndjson(s) {
            Some(points) => return Ok(points),
            None => return Err(format!("input starts like JSON but did not parse: {e}")),
        },
    };
    let items: Vec<&Value> = match &v {
        Value::Array(a) => a.iter().collect(),
        Value::Object(o) => {
            // OwnTracks Recorder HTTP API wraps rows in {"count":N,"data":[…]};
            // some exports use {"locations":[…]}.
            if let Some(Value::Array(a)) = o.get("data").or_else(|| o.get("locations")) {
                a.iter().collect()
            } else {
                vec![&v]
            }
        }
        _ => {
            return Err("input JSON must be an array of location objects, a {\"data\":[…]} object, \
                        or a single location object"
                .to_string())
        }
    };

    let mut points = Vec::new();
    for it in items {
        if let Some(p) = pt_from_value(it) {
            points.push(p);
        }
    }
    Ok(points)
}

/// Try to parse newline-delimited JSON (one location object per line). Returns
/// `None` unless every non-blank line is a valid JSON object, so a genuinely
/// malformed single document still surfaces its own parse error.
fn parse_ndjson(s: &str) -> Option<Vec<Pt>> {
    let mut points = Vec::new();
    let mut saw_line = false;
    for raw in s.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        saw_line = true;
        let v: Value = serde_json::from_str(line).ok()?;
        if !v.is_object() {
            return None;
        }
        if let Some(p) = pt_from_value(&v) {
            points.push(p);
        }
    }
    if saw_line {
        Some(points)
    } else {
        None
    }
}

/// Parse the OwnTracks Recorder `.rec` format: one record per line, the JSON
/// payload following the last tab (the first column is an ISO timestamp).
fn parse_rec(input: &str) -> Result<Vec<Pt>, String> {
    let mut points = Vec::new();
    for raw in input.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        // The JSON payload is everything from the first '{'. The columns before it
        // are `<ISO-timestamp>\t<type>` (type is usually `*`).
        let brace = match line.find('{') {
            Some(i) => i,
            None => continue, // not a JSON-bearing record (e.g. a comment line)
        };
        let iso = line[..brace]
            .split('\t')
            .next()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty() && s.chars().next().is_some_and(|c| c.is_ascii_digit()))
            .map(|s| s.to_string());
        let json = &line[brace..];
        let v: Value = match serde_json::from_str(json) {
            Ok(v) => v,
            Err(_) => continue, // tolerate a malformed line rather than aborting the file
        };
        if let Some(mut p) = pt_from_value(&v) {
            if p.tst.is_none() {
                p.iso_fallback = iso;
            }
            points.push(p);
        }
    }
    Ok(points)
}

/// Build a `Pt` from one OwnTracks message `Value`, or `None` if it is not a
/// usable location (wrong `_type`, or missing lat/lon).
fn pt_from_value(v: &Value) -> Option<Pt> {
    let obj = v.as_object()?;
    // Only `location` messages carry a track fix. Accept a missing _type if the
    // object still has coordinates (some exports drop it).
    if let Some(t) = obj.get("_type").and_then(Value::as_str) {
        if t != "location" {
            return None;
        }
    }
    let lat = num(obj.get("lat"))?;
    let lon = num(obj.get("lon"))?;
    if !lat.is_finite() || !lon.is_finite() {
        return None;
    }
    Some(Pt {
        lat,
        lon,
        tst: obj.get("tst").and_then(Value::as_i64).or_else(|| num(obj.get("tst")).map(|f| f as i64)),
        iso_fallback: None,
        alt: num(obj.get("alt")),
        acc: num(obj.get("acc")),
        vel: num(obj.get("vel")),
        cog: num(obj.get("cog")),
        batt: num(obj.get("batt")),
        tid: obj.get("tid").and_then(Value::as_str).map(str::to_string),
    })
}

/// Read a JSON value as f64, accepting a number or a numeric string.
fn num(v: Option<&Value>) -> Option<f64> {
    match v? {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse::<f64>().ok(),
        _ => None,
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Format an `f64` without a spurious trailing `.0` for whole values but full
/// precision otherwise (`52.1`, `10`, `-122.084`).
fn fmt_num(v: f64) -> String {
    format!("{v}")
}

/// Convert Unix epoch seconds to an ISO-8601 UTC timestamp (`2024-01-01T12:00:00Z`)
/// using Howard Hinnant's civil-from-days algorithm — no clock, fully deterministic.
fn epoch_to_iso(tst: i64) -> String {
    let days = tst.div_euclid(86_400);
    let secs = tst.rem_euclid(86_400);
    let (h, m, s) = (secs / 3600, (secs % 3600) / 60, secs % 60);
    let (y, mo, d) = civil_from_days(days);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}Z")
}

/// Days since 1970-01-01 → (year, month, day). Valid for the full proleptic
/// Gregorian range (Howard Hinnant, "chrono-Compatible Low-Level Date Algorithms").
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Split points into segments wherever the epoch gap exceeds `gap_minutes`
/// (0 ⇒ one segment with every point).
fn segment(points: &[Pt], gap_minutes: f64) -> Vec<&[Pt]> {
    if gap_minutes <= 0.0 || points.len() < 2 {
        return vec![points];
    }
    let gap_secs = gap_minutes * 60.0;
    let mut segs = Vec::new();
    let mut start = 0usize;
    for i in 1..points.len() {
        if let (Some(prev), Some(cur)) = (points[i - 1].epoch(), points[i].epoch()) {
            if (cur - prev) as f64 > gap_secs {
                segs.push(&points[start..i]);
                start = i;
            }
        }
    }
    segs.push(&points[start..]);
    segs
}

fn build_gpx(points: &[Pt], opt: &Options) -> String {
    let has_ext = opt.include_extensions
        && points.iter().any(|p| {
            p.acc.is_some()
                || p.vel.is_some()
                || p.cog.is_some()
                || p.batt.is_some()
                || p.tid.is_some()
        });

    let earliest = points.iter().filter_map(Pt::time_str).min();

    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str("<gpx version=\"1.1\" creator=\"gizza-ai/owntracks-to-gpx\"\n");
    out.push_str(&format!("     xmlns=\"{GPX_NS}\"\n"));
    if has_ext {
        out.push_str(&format!("     xmlns:ot=\"{OT_NS}\"\n"));
    }
    out.push_str(&format!("     xmlns:xsi=\"{XSI_NS}\"\n"));
    out.push_str(&format!("     xsi:schemaLocation=\"{SCHEMA_LOCATION}\">\n"));

    if let Some(t) = &earliest {
        out.push_str("  <metadata>\n");
        out.push_str(&format!("    <time>{}</time>\n", xml_escape(t)));
        out.push_str("  </metadata>\n");
    }

    out.push_str("  <trk>\n");
    if !opt.track_name.is_empty() {
        out.push_str(&format!("    <name>{}</name>\n", xml_escape(&opt.track_name)));
    }
    for seg in segment(points, opt.segment_gap_minutes) {
        out.push_str("    <trkseg>\n");
        for p in seg {
            out.push_str(&format!(
                "      <trkpt lat=\"{}\" lon=\"{}\">\n",
                fmt_num(p.lat),
                fmt_num(p.lon)
            ));
            if let Some(e) = p.alt {
                out.push_str(&format!("        <ele>{}</ele>\n", fmt_num(e)));
            }
            if let Some(t) = p.time_str() {
                out.push_str(&format!("        <time>{}</time>\n", xml_escape(&t)));
            }
            if opt.include_extensions
                && (p.acc.is_some()
                    || p.vel.is_some()
                    || p.cog.is_some()
                    || p.batt.is_some()
                    || p.tid.is_some())
            {
                out.push_str("        <extensions>\n");
                if let Some(a) = p.acc {
                    out.push_str(&format!("          <ot:accuracy>{}</ot:accuracy>\n", fmt_num(a)));
                }
                if let Some(v) = p.vel {
                    out.push_str(&format!("          <ot:velocity>{}</ot:velocity>\n", fmt_num(v)));
                }
                if let Some(c) = p.cog {
                    out.push_str(&format!("          <ot:course>{}</ot:course>\n", fmt_num(c)));
                }
                if let Some(b) = p.batt {
                    out.push_str(&format!("          <ot:battery>{}</ot:battery>\n", fmt_num(b)));
                }
                if let Some(tid) = &p.tid {
                    out.push_str(&format!("          <ot:tid>{}</ot:tid>\n", xml_escape(tid)));
                }
                out.push_str("        </extensions>\n");
            }
            out.push_str("      </trkpt>\n");
        }
        out.push_str("    </trkseg>\n");
    }
    out.push_str("  </trk>\n");
    out.push_str("</gpx>\n");
    out
}

/// Parse an ISO-8601 `YYYY-MM-DDThh:mm:ss` (optional `Z`/offset ignored) into an
/// epoch, for segment-gap splitting of `.rec` records that lack `tst`. Best-effort:
/// returns `None` on anything it can't read.
fn parse_iso_epoch(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() < 19 {
        return None;
    }
    let g = |a: usize, z: usize| s.get(a..z)?.parse::<i64>().ok();
    let (y, mo, d) = (g(0, 4)?, g(5, 7)?, g(8, 10)?);
    let (h, mi, se) = (g(11, 13)?, g(14, 16)?, g(17, 19)?);
    Some(days_from_civil(y, mo as u32, d as u32) * 86_400 + h * 3600 + mi * 60 + se)
}

/// Inverse of `civil_from_days`: (year, month, day) → days since 1970-01-01.
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 } as i64;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    const JSON_ARRAY: &str = r#"[
      {"_type":"location","tid":"5f","acc":12,"batt":56,"vel":9,"cog":180,"lat":52.100,"lon":5.100,"alt":10,"tst":1704110400},
      {"_type":"location","tid":"5f","acc":8,"lat":52.101,"lon":5.102,"alt":12,"tst":1704110700},
      {"_type":"transition","lat":0.0,"lon":0.0,"tst":1704110800}
    ]"#;

    #[test]
    fn happy_json_array_two_points() {
        let gpx = convert(JSON_ARRAY, &Options::default()).unwrap();
        // The transition record is skipped; two location points remain.
        assert_eq!(gpx.matches("<trkpt ").count(), 2);
        assert!(gpx.contains("lat=\"52.1\" lon=\"5.1\""));
        assert!(gpx.contains("lat=\"52.101\" lon=\"5.102\""));
        assert!(gpx.contains("<ele>10</ele>"));
        // tst 1704110400 == 2024-01-01T12:00:00Z.
        assert!(gpx.contains("<time>2024-01-01T12:00:00Z</time>"));
        assert!(gpx.contains("<time>2024-01-01T12:05:00Z</time>"));
        assert!(gpx.contains("<metadata>\n    <time>2024-01-01T12:00:00Z</time>"));
        // Extensions on by default.
        assert!(gpx.contains("xmlns:ot="));
        assert!(gpx.contains("<ot:accuracy>12</ot:accuracy>"));
        assert!(gpx.contains("<ot:velocity>9</ot:velocity>"));
        assert!(gpx.contains("<ot:course>180</ot:course>"));
        assert!(gpx.contains("<ot:battery>56</ot:battery>"));
        assert!(gpx.contains("<ot:tid>5f</ot:tid>"));
        assert!(gpx.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
        assert!(gpx.trim_end().ends_with("</gpx>"));
        // One track, one segment by default.
        assert_eq!(gpx.matches("<trk>").count(), 1);
        assert_eq!(gpx.matches("<trkseg>").count(), 1);
    }

    #[test]
    fn extensions_can_be_disabled() {
        let opt = Options { include_extensions: false, ..Default::default() };
        let gpx = convert(JSON_ARRAY, &opt).unwrap();
        assert!(!gpx.contains("xmlns:ot="));
        assert!(!gpx.contains("<extensions>"));
        assert!(gpx.contains("<time>2024-01-01T12:00:00Z</time>"));
        assert!(gpx.contains("<ele>10</ele>"));
    }

    #[test]
    fn rec_format_with_iso_fallback_time() {
        // Second line's payload omits tst → the line's ISO column is used.
        let rec = "2024-01-01T12:00:00Z\t*\t{\"_type\":\"location\",\"lat\":1.0,\"lon\":2.0,\"tst\":1704110400}\n\
                   2024-01-01T12:05:00Z\t*\t{\"_type\":\"location\",\"lat\":1.1,\"lon\":2.1}\n\
                   2024-01-01T12:06:00Z\t*\t{\"_type\":\"lwt\",\"tst\":1704110760}";
        let gpx = convert(rec, &Options::default()).unwrap();
        assert_eq!(gpx.matches("<trkpt ").count(), 2); // lwt skipped
        assert!(gpx.contains("lat=\"1\" lon=\"2\""));
        assert!(gpx.contains("<time>2024-01-01T12:00:00Z</time>"));
        // Point 2 had no tst → fell back to the .rec line timestamp.
        assert!(gpx.contains("<time>2024-01-01T12:05:00Z</time>"));
    }

    #[test]
    fn recorder_api_data_wrapper() {
        let json = r#"{"count":1,"data":[{"_type":"location","lat":48.2,"lon":16.37,"tst":1704110400}]}"#;
        let gpx = convert(json, &Options::default()).unwrap();
        assert_eq!(gpx.matches("<trkpt ").count(), 1);
        assert!(gpx.contains("lat=\"48.2\" lon=\"16.37\""));
    }

    #[test]
    fn single_location_object() {
        let json = r#"{"_type":"location","lat":10.5,"lon":-20.25,"tst":1704110400}"#;
        let gpx = convert(json, &Options::default()).unwrap();
        assert_eq!(gpx.matches("<trkpt ").count(), 1);
        assert!(gpx.contains("lat=\"10.5\" lon=\"-20.25\""));
    }

    #[test]
    fn ndjson_one_object_per_line() {
        let nd = "{\"_type\":\"location\",\"lat\":1.0,\"lon\":2.0,\"tst\":1704110400}\n\
                  {\"_type\":\"location\",\"lat\":1.1,\"lon\":2.1,\"tst\":1704110700}";
        let gpx = convert(nd, &Options::default()).unwrap();
        assert_eq!(gpx.matches("<trkpt ").count(), 2);
        assert!(gpx.contains("lat=\"1.1\" lon=\"2.1\""));
    }

    #[test]
    fn segment_gap_splits_track() {
        // Three points; a 30-minute jump between #2 and #3.
        let json = r#"[
          {"_type":"location","lat":1.0,"lon":1.0,"tst":1704110400},
          {"_type":"location","lat":1.1,"lon":1.1,"tst":1704110700},
          {"_type":"location","lat":2.0,"lon":2.0,"tst":1704112500}
        ]"#;
        let opt = Options { segment_gap_minutes: 20.0, ..Default::default() };
        let gpx = convert(json, &opt).unwrap();
        assert_eq!(gpx.matches("<trk>").count(), 1);
        assert_eq!(gpx.matches("<trkseg>").count(), 2);
    }

    #[test]
    fn accuracy_filter_drops_bad_fixes() {
        let json = r#"[
          {"_type":"location","lat":1.0,"lon":1.0,"acc":8,"tst":1704110400},
          {"_type":"location","lat":1.1,"lon":1.1,"acc":250,"tst":1704110700},
          {"_type":"location","lat":1.2,"lon":1.2,"tst":1704111000}
        ]"#;
        let opt = Options { max_accuracy_meters: 50.0, ..Default::default() };
        let gpx = convert(json, &opt).unwrap();
        // The 250 m fix is dropped; the fix with no acc is kept.
        assert_eq!(gpx.matches("<trkpt ").count(), 2);
    }

    #[test]
    fn track_name_is_emitted() {
        let opt = Options { track_name: "Morning commute".into(), ..Default::default() };
        let gpx = convert(JSON_ARRAY, &opt).unwrap();
        assert!(gpx.contains("<name>Morning commute</name>"));
    }

    #[test]
    fn empty_input_errors() {
        assert!(convert("   ", &Options::default()).is_err());
    }

    #[test]
    fn no_location_records_errors() {
        let json = r#"[{"_type":"transition","lat":1.0,"lon":1.0,"tst":1}]"#;
        let err = convert(json, &Options::default()).unwrap_err();
        assert!(err.contains("no OwnTracks location records"));
    }

    #[test]
    fn malformed_json_errors() {
        let err = convert("[{\"_type\":\"location\",", &Options::default()).unwrap_err();
        assert!(err.contains("did not parse"));
    }

    #[test]
    fn epoch_roundtrip_matches_known_dates() {
        assert_eq!(epoch_to_iso(0), "1970-01-01T00:00:00Z");
        assert_eq!(epoch_to_iso(1704110400), "2024-01-01T12:00:00Z");
        assert_eq!(epoch_to_iso(1462958647), "2016-05-11T09:24:07Z");
        // Round-trip through the ISO parser.
        assert_eq!(parse_iso_epoch("2016-05-11T09:24:07Z"), Some(1462958647));
    }
}
