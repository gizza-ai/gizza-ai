//! gizza-ai/gpx-analyzer core — parse a GPX track and report distance,
//! elevation gain/loss, duration, average/max speed, pace, per-km & per-mile
//! splits, and sensor extensions (heart rate, cadence, power, temperature).
//! Pure-Rust (`quick-xml`), no wafer/wasm-bindgen deps.

use quick_xml::events::Event;
use quick_xml::name::QName;
use quick_xml::reader::Reader;
use serde::Serialize;

/// One parsed track/route point.
#[derive(Debug, Clone, PartialEq, Default)]
struct Point {
    lat: f64,
    lon: f64,
    /// Elevation in metres, if the point carried an `<ele>`.
    ele: Option<f64>,
    /// Time as epoch seconds, if the point carried a `<time>`.
    time: Option<f64>,
    /// Raw ISO timestamp text (for start/end reporting).
    time_iso: Option<String>,
    /// Sensor extensions (Garmin TrackPointExtension etc.), if present.
    hr: Option<f64>,
    cad: Option<f64>,
    power: Option<f64>,
    temp: Option<f64>,
}

/// One distance split (a completed km or mile).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Split {
    /// 1-based split index.
    pub index: usize,
    /// Split length in the split unit (1.0 except a partial final split).
    pub length: f64,
    /// Time for this split, in seconds, when timestamps are present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_s: Option<f64>,
    /// Pace for this split (MM:SS per unit), when timed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pace: Option<String>,
    /// Elevation gain accumulated during this split, in metres, when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elevation_gain_m: Option<f64>,
}

/// Average + maximum of a sensor channel.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SensorStat {
    pub avg: f64,
    pub max: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Stats {
    /// Track name from the first `<name>` inside `<trk>`, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Number of points with coordinates.
    pub points: usize,
    /// Total great-circle distance in metres (summed point-to-point).
    pub distance_m: f64,
    pub distance_km: f64,
    pub distance_mi: f64,
    /// Total ascent / descent in metres (sum of positive / negative deltas).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elevation_gain_m: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elevation_loss_m: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_elevation_m: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_elevation_m: Option<f64>,
    /// First / last timestamp (ISO-8601, as written in the file).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time: Option<String>,
    /// Wall-clock duration first→last timed point, in seconds (and H:MM:SS).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_s: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_hms: Option<String>,
    /// Average speed over the whole duration, km/h and mph.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg_speed_kmh: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg_speed_mph: Option<f64>,
    /// Maximum instantaneous speed between two consecutive timed points.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_speed_kmh: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_speed_mph: Option<f64>,
    /// Average pace, min/km and min/mi (MM:SS strings).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg_pace_min_per_km: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg_pace_min_per_mi: Option<String>,
    /// Heart rate / cadence / power / temperature, when the track has them.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heart_rate_bpm: Option<SensorStat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cadence_rpm: Option<SensorStat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub power_w: Option<SensorStat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature_c: Option<SensorStat>,
    /// Per-kilometre splits (only when the track has timestamps).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub km_splits: Vec<Split>,
    /// Per-mile splits (only when the track has timestamps).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub mile_splits: Vec<Split>,
}

const EARTH_RADIUS_M: f64 = 6_371_000.0;
const M_PER_MI: f64 = 1609.344;
const M_PER_KM: f64 = 1000.0;

/// Great-circle distance between two lat/lon points, in metres (haversine).
fn haversine(a: &Point, b: &Point) -> f64 {
    let (lat1, lat2) = (a.lat.to_radians(), b.lat.to_radians());
    let dlat = (b.lat - a.lat).to_radians();
    let dlon = (b.lon - a.lon).to_radians();
    let h = (dlat / 2.0).sin().powi(2) + lat1.cos() * lat2.cos() * (dlon / 2.0).sin().powi(2);
    2.0 * EARTH_RADIUS_M * h.sqrt().asin()
}

/// Round to `n` decimal places.
fn round(v: f64, n: i32) -> f64 {
    let f = 10f64.powi(n);
    (v * f).round() / f
}

/// Parse an RFC 3339 / ISO-8601 timestamp (the GPX `<time>` format,
/// e.g. `2024-03-09T08:15:30Z`, with optional fractional second and `±HH:MM`
/// offset) into epoch seconds. Self-contained (no chrono — the page target has
/// no std clock, but plain calendar math is fine).
fn parse_time(s: &str) -> Option<f64> {
    let s = s.trim();
    let bytes = s.as_bytes();
    if bytes.len() < 19 {
        return None;
    }
    let num = |a: usize, b: usize| -> Option<i64> { s.get(a..b)?.parse().ok() };
    let year = num(0, 4)?;
    let month = num(5, 7)?;
    let day = num(8, 10)?;
    let hour = num(11, 13)?;
    let min = num(14, 16)?;
    let sec = num(17, 19)?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    // Days since Unix epoch (1970-01-01), civil-from-days (Howard Hinnant's algo).
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    let mut epoch = days as f64 * 86400.0 + hour as f64 * 3600.0 + min as f64 * 60.0 + sec as f64;
    // Optional fractional seconds.
    let mut i = 19;
    if bytes.get(19) == Some(&b'.') {
        let mut frac = 0.0;
        let mut scale = 0.1;
        i = 20;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            frac += (bytes[i] - b'0') as f64 * scale;
            scale *= 0.1;
            i += 1;
        }
        epoch += frac;
    }
    // Optional timezone offset (Z = UTC, or ±HH:MM → convert to UTC).
    if let Some(&c) = bytes.get(i) {
        if c == b'+' || c == b'-' {
            let oh = s.get(i + 1..i + 3)?.parse::<i64>().ok()?;
            let om = s
                .get(i + 4..i + 6)
                .and_then(|m| m.parse::<i64>().ok())
                .unwrap_or(0);
            let offset = (oh * 3600 + om * 60) as f64;
            epoch += if c == b'+' { -offset } else { offset };
        }
    }
    Some(epoch)
}

/// Format seconds as `H:MM:SS`.
fn hms(total: f64) -> String {
    let t = total.max(0.0).round() as i64;
    format!("{}:{:02}:{:02}", t / 3600, (t % 3600) / 60, t % 60)
}

/// Format a pace (seconds per unit) as `MM:SS`.
fn pace(secs_per_unit: f64) -> String {
    let t = secs_per_unit.max(0.0).round() as i64;
    format!("{}:{:02}", t / 60, t % 60)
}

/// Strip any `ns:` prefix from an XML qualified name.
fn local_name(name: QName) -> String {
    let full = name.as_ref();
    let local = match full.iter().position(|&b| b == b':') {
        Some(i) => &full[i + 1..],
        None => full,
    };
    String::from_utf8_lossy(local).into_owned()
}

/// Build splits of `unit` metres each. Walks the track segment by segment; when
/// the cumulative distance crosses a split boundary, the boundary timestamp is
/// linearly interpolated within the crossing segment (constant pace assumed over
/// the segment), giving each split a precise duration, pace, and elevation gain.
fn splits(points: &[Point], unit: f64) -> Vec<Split> {
    let has_ele = points.iter().any(|p| p.ele.is_some());
    let mut out = Vec::new();
    let mut idx = 1usize;
    let mut covered = 0.0; // metres accumulated into the current (open) split
    let mut split_start_time: Option<f64> = points.first().and_then(|p| p.time);
    let mut split_gain = 0.0f64;

    for w in points.windows(2) {
        let (a, b) = (&w[0], &w[1]);
        let seg = haversine(a, b);
        if seg <= 0.0 {
            continue;
        }
        // Segment elevation gain (positive deltas only), credited proportionally
        // as the segment is consumed below.
        let seg_gain = match (a.ele, b.ele) {
            (Some(ea), Some(eb)) if eb > ea => eb - ea,
            _ => 0.0,
        };
        // Per-metre time for this segment (constant-pace assumption).
        let tpm = match (a.time, b.time) {
            (Some(ta), Some(tb)) => Some((tb - ta) / seg),
            _ => None,
        };
        let mut seg_pos = 0.0; // metres consumed within this segment so far

        // Emit every full split boundary this segment crosses.
        while covered + (seg - seg_pos) >= unit - 1e-9 {
            let need = unit - covered; // metres still needed to close the split
            // Elevation gain credited to the closing split (proportional share).
            split_gain += seg_gain * (need / seg);
            // Boundary timestamp inside this segment.
            let boundary_time = match (a.time, tpm) {
                (Some(ta), Some(tpm)) => Some(ta + (seg_pos + need) * tpm),
                _ => None,
            };
            let dur = match (split_start_time, boundary_time) {
                (Some(t0), Some(tb)) => Some((tb - t0).max(0.0)),
                _ => None,
            };
            out.push(Split {
                index: idx,
                length: 1.0,
                duration_s: dur.map(|d| round(d, 1)),
                pace: dur.map(pace),
                elevation_gain_m: has_ele.then(|| round(split_gain, 1)),
            });
            idx += 1;
            seg_pos += need;
            covered = 0.0;
            split_gain = 0.0;
            split_start_time = boundary_time;
        }
        // Consume the remainder of the segment into the open split.
        let rem = seg - seg_pos;
        covered += rem;
        split_gain += seg_gain * (rem / seg);
    }

    // Final partial split.
    if covered > 1.0 {
        let length = covered / unit;
        let dur = match (split_start_time, points.last().and_then(|p| p.time)) {
            (Some(t0), Some(tn)) => Some((tn - t0).max(0.0)),
            _ => None,
        };
        out.push(Split {
            index: idx,
            length: round(length, 3),
            duration_s: dur.map(|d| round(d, 1)),
            // Pace normalized to a full unit (extrapolated from the partial).
            pace: dur.filter(|_| length > 0.0).map(|d| pace(d / length)),
            elevation_gain_m: has_ele.then(|| round(split_gain, 1)),
        });
    }
    out
}

fn sensor_stat(vals: &[f64]) -> Option<SensorStat> {
    if vals.is_empty() {
        return None;
    }
    let sum: f64 = vals.iter().sum();
    let max = vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    Some(SensorStat {
        avg: round(sum / vals.len() as f64, 1),
        max: round(max, 1),
    })
}

/// Parse a GPX document and compute track statistics. Returns an error for
/// malformed XML or a document with fewer than two usable points.
pub fn analyze(gpx: &str) -> Result<Stats, String> {
    let mut reader = Reader::from_str(gpx);
    reader.config_mut().trim_text(true);
    let decoder = reader.decoder();

    let mut points: Vec<Point> = Vec::new();
    let mut name: Option<String> = None;

    let mut cur: Point = Point::default();
    let mut have_lat = false;
    let mut have_lon = false;
    let mut in_point = false;
    let mut in_trk = false;
    // Text target: 1=ele, 2=time, 3=trk name, 4=hr, 5=cad, 6=power, 7=temp
    let mut text_target: Option<u8> = None;

    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Err(e) => return Err(format!("malformed GPX/XML: {e}")),
            Ok(Event::Eof) => break,
            Ok(ev @ (Event::Start(_) | Event::Empty(_))) => {
                let is_empty = matches!(ev, Event::Empty(_));
                let e = match ev {
                    Event::Start(e) | Event::Empty(e) => e,
                    _ => unreachable!(),
                };
                let local = local_name(e.name()).to_ascii_lowercase();
                match local.as_str() {
                    "trk" => in_trk = true,
                    "trkpt" | "rtept" | "wpt" => {
                        in_point = true;
                        cur = Point::default();
                        have_lat = false;
                        have_lon = false;
                        for attr in e.attributes().flatten() {
                            let key = local_name(QName(attr.key.as_ref())).to_ascii_lowercase();
                            #[allow(deprecated)]
                            let val = attr
                                .decode_and_unescape_value(decoder)
                                .unwrap_or_default();
                            match key.as_str() {
                                "lat" => {
                                    if let Ok(v) = val.trim().parse() {
                                        cur.lat = v;
                                        have_lat = true;
                                    }
                                }
                                "lon" => {
                                    if let Ok(v) = val.trim().parse() {
                                        cur.lon = v;
                                        have_lon = true;
                                    }
                                }
                                _ => {}
                            }
                        }
                        if is_empty {
                            commit_point(&mut points, &cur, have_lat, have_lon);
                            in_point = false;
                        }
                    }
                    "ele" if in_point => text_target = Some(1),
                    "time" if in_point => text_target = Some(2),
                    "name" if in_trk && name.is_none() => text_target = Some(3),
                    // Sensor extensions (Garmin TrackPointExtension + common variants).
                    "hr" | "heartrate" if in_point => text_target = Some(4),
                    "cad" | "cadence" if in_point => text_target = Some(5),
                    "power" | "watts" if in_point => text_target = Some(6),
                    "atemp" | "temp" | "temperature" if in_point => text_target = Some(7),
                    _ => {}
                }
            }
            Ok(Event::Text(t)) => {
                if let Some(target) = text_target {
                    let raw = t.decode().unwrap_or_default().into_owned();
                    let s = raw.trim();
                    match target {
                        1 => cur.ele = s.parse().ok(),
                        2 => {
                            cur.time = parse_time(s);
                            cur.time_iso = Some(s.to_string());
                        }
                        3 => name = Some(s.to_string()),
                        4 => cur.hr = s.parse().ok(),
                        5 => cur.cad = s.parse().ok(),
                        6 => cur.power = s.parse().ok(),
                        7 => cur.temp = s.parse().ok(),
                        _ => {}
                    }
                }
            }
            Ok(Event::End(e)) => {
                let local = local_name(e.name()).to_ascii_lowercase();
                match local.as_str() {
                    "ele" | "time" | "name" | "hr" | "heartrate" | "cad" | "cadence" | "power"
                    | "watts" | "atemp" | "temp" | "temperature" => text_target = None,
                    "trk" => in_trk = false,
                    "trkpt" | "rtept" | "wpt" => {
                        commit_point(&mut points, &cur, have_lat, have_lon);
                        in_point = false;
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        buf.clear();
    }

    if points.len() < 2 {
        return Err(
            "no track found: a GPX track needs at least two points with lat/lon coordinates".into(),
        );
    }

    // Distance + max instantaneous speed.
    let mut distance = 0.0;
    let mut max_speed_ms = 0.0f64; // metres per second
    for w in points.windows(2) {
        let seg = haversine(&w[0], &w[1]);
        distance += seg;
        if let (Some(ta), Some(tb)) = (w[0].time, w[1].time) {
            let dt = tb - ta;
            if dt > 0.0 {
                max_speed_ms = max_speed_ms.max(seg / dt);
            }
        }
    }

    // Elevation.
    let have_ele = points.iter().any(|p| p.ele.is_some());
    let (mut gain, mut loss) = (0.0, 0.0);
    let (mut min_ele, mut max_ele) = (f64::INFINITY, f64::NEG_INFINITY);
    let mut last_ele: Option<f64> = None;
    if have_ele {
        for p in &points {
            if let Some(e) = p.ele {
                min_ele = min_ele.min(e);
                max_ele = max_ele.max(e);
                if let Some(prev) = last_ele {
                    let d = e - prev;
                    if d > 0.0 {
                        gain += d;
                    } else {
                        loss += -d;
                    }
                }
                last_ele = Some(e);
            }
        }
    }

    // Duration (first→last timed point) + start/end ISO.
    let timed: Vec<&Point> = points.iter().filter(|p| p.time.is_some()).collect();
    let duration = if timed.len() >= 2 {
        Some((timed.last().unwrap().time.unwrap() - timed[0].time.unwrap()).max(0.0))
    } else {
        None
    };
    let start_time = timed.first().and_then(|p| p.time_iso.clone());
    let end_time = timed.last().and_then(|p| p.time_iso.clone());

    let km = distance / M_PER_KM;
    let mi = distance / M_PER_MI;

    let (avg_speed_kmh, avg_speed_mph, avg_pace_km, avg_pace_mi, dur_hms) = match duration {
        Some(d) if d > 0.0 => {
            let hours = d / 3600.0;
            (
                Some(round(km / hours, 3)),
                Some(round(mi / hours, 3)),
                (km > 0.0).then(|| pace(d / km)),
                (mi > 0.0).then(|| pace(d / mi)),
                Some(hms(d)),
            )
        }
        Some(d) => (None, None, None, None, Some(hms(d))),
        None => (None, None, None, None, None),
    };

    let (max_speed_kmh, max_speed_mph) = if max_speed_ms > 0.0 {
        (
            Some(round(max_speed_ms * 3.6, 3)),
            Some(round(max_speed_ms * 3600.0 / M_PER_MI, 3)),
        )
    } else {
        (None, None)
    };

    // Sensor stats.
    let hr: Vec<f64> = points.iter().filter_map(|p| p.hr).collect();
    let cad: Vec<f64> = points.iter().filter_map(|p| p.cad).collect();
    let pow: Vec<f64> = points.iter().filter_map(|p| p.power).collect();
    let tmp: Vec<f64> = points.iter().filter_map(|p| p.temp).collect();

    // Splits (only meaningful when timed; gate on duration presence).
    let (km_splits, mile_splits) = if duration.is_some() {
        (splits(&points, M_PER_KM), splits(&points, M_PER_MI))
    } else {
        (Vec::new(), Vec::new())
    };

    Ok(Stats {
        name,
        points: points.len(),
        distance_m: round(distance, 2),
        distance_km: round(km, 3),
        distance_mi: round(mi, 3),
        elevation_gain_m: have_ele.then(|| round(gain, 1)),
        elevation_loss_m: have_ele.then(|| round(loss, 1)),
        min_elevation_m: (have_ele && min_ele.is_finite()).then(|| round(min_ele, 1)),
        max_elevation_m: (have_ele && max_ele.is_finite()).then(|| round(max_ele, 1)),
        start_time,
        end_time,
        duration_s: duration.map(|d| round(d, 1)),
        duration_hms: dur_hms,
        avg_speed_kmh,
        avg_speed_mph,
        max_speed_kmh,
        max_speed_mph,
        avg_pace_min_per_km: avg_pace_km,
        avg_pace_min_per_mi: avg_pace_mi,
        heart_rate_bpm: sensor_stat(&hr),
        cadence_rpm: sensor_stat(&cad),
        power_w: sensor_stat(&pow),
        temperature_c: sensor_stat(&tmp),
        km_splits,
        mile_splits,
    })
}

fn commit_point(points: &mut Vec<Point>, cur: &Point, have_lat: bool, have_lon: bool) {
    if have_lat && have_lon {
        points.push(cur.clone());
    }
}

/// JSON for the chat/CLI surface.
pub fn analyze_json(gpx: &str) -> Result<String, String> {
    let s = analyze(gpx)?;
    serde_json::to_string_pretty(&s).map_err(|e| e.to_string())
}

/// Human-readable text for the page surface.
pub fn render(gpx: &str) -> Result<String, String> {
    let s = analyze(gpx)?;
    let mut out = String::new();
    if let Some(n) = &s.name {
        out.push_str(&format!("Track: {n}\n"));
    }
    out.push_str(&format!("Points:    {}\n", s.points));
    out.push_str(&format!(
        "Distance:  {:.2} km  ({:.2} mi)\n",
        s.distance_km, s.distance_mi
    ));
    if let (Some(g), Some(l)) = (s.elevation_gain_m, s.elevation_loss_m) {
        out.push_str(&format!("Elevation: +{g} m / -{l} m"));
        if let (Some(mn), Some(mx)) = (s.min_elevation_m, s.max_elevation_m) {
            out.push_str(&format!("  (min {mn} m, max {mx} m)"));
        }
        out.push('\n');
    }
    if let Some(d) = &s.duration_hms {
        out.push_str(&format!("Duration:  {d}\n"));
    }
    if let (Some(kmh), Some(mph)) = (s.avg_speed_kmh, s.avg_speed_mph) {
        out.push_str(&format!("Avg speed: {kmh:.2} km/h  ({mph:.2} mph)\n"));
    }
    if let (Some(kmh), Some(mph)) = (s.max_speed_kmh, s.max_speed_mph) {
        out.push_str(&format!("Max speed: {kmh:.2} km/h  ({mph:.2} mph)\n"));
    }
    if let (Some(pk), Some(pm)) = (&s.avg_pace_min_per_km, &s.avg_pace_min_per_mi) {
        out.push_str(&format!("Avg pace:  {pk} min/km  ({pm} min/mi)\n"));
    }
    if let Some(hr) = &s.heart_rate_bpm {
        out.push_str(&format!(
            "Heart rate: avg {} bpm, max {} bpm\n",
            hr.avg, hr.max
        ));
    }
    if let Some(c) = &s.cadence_rpm {
        out.push_str(&format!("Cadence:   avg {} rpm, max {} rpm\n", c.avg, c.max));
    }
    if let Some(p) = &s.power_w {
        out.push_str(&format!("Power:     avg {} W, max {} W\n", p.avg, p.max));
    }
    if let Some(t) = &s.temperature_c {
        out.push_str(&format!("Temp:      avg {} °C, max {} °C\n", t.avg, t.max));
    }
    if !s.km_splits.is_empty() {
        out.push_str("\nKm splits:\n");
        for sp in &s.km_splits {
            let len = if (sp.length - 1.0).abs() < 1e-9 {
                String::new()
            } else {
                format!(" ({:.2} km)", sp.length)
            };
            match &sp.pace {
                Some(p) => out.push_str(&format!("  {:>2}: {} /km{}\n", sp.index, p, len)),
                None => out.push_str(&format!("  {:>2}:{}\n", sp.index, len)),
            }
        }
    }
    Ok(out.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<gpx version="1.1" creator="test" xmlns="http://www.topografix.com/GPX/1/1">
  <trk>
    <name>Morning Run</name>
    <trkseg>
      <trkpt lat="40.000000" lon="-105.000000"><ele>1600</ele><time>2024-03-09T08:00:00Z</time></trkpt>
      <trkpt lat="40.001000" lon="-105.000000"><ele>1610</ele><time>2024-03-09T08:01:00Z</time></trkpt>
      <trkpt lat="40.002000" lon="-105.000000"><ele>1605</ele><time>2024-03-09T08:02:00Z</time></trkpt>
    </trkseg>
  </trk>
</gpx>"#;

    #[test]
    fn parses_name_and_points() {
        let s = analyze(SAMPLE).unwrap();
        assert_eq!(s.name.as_deref(), Some("Morning Run"));
        assert_eq!(s.points, 3);
    }

    #[test]
    fn distance_is_about_two_segments_of_111m() {
        let s = analyze(SAMPLE).unwrap();
        assert!(
            (s.distance_m - 222.4).abs() < 2.0,
            "distance {} not ~222 m",
            s.distance_m
        );
    }

    #[test]
    fn elevation_gain_and_loss() {
        let s = analyze(SAMPLE).unwrap();
        assert_eq!(s.elevation_gain_m, Some(10.0));
        assert_eq!(s.elevation_loss_m, Some(5.0));
        assert_eq!(s.min_elevation_m, Some(1600.0));
        assert_eq!(s.max_elevation_m, Some(1610.0));
    }

    #[test]
    fn duration_speed_pace_and_times() {
        let s = analyze(SAMPLE).unwrap();
        assert_eq!(s.duration_s, Some(120.0));
        assert_eq!(s.duration_hms.as_deref(), Some("0:02:00"));
        assert_eq!(s.start_time.as_deref(), Some("2024-03-09T08:00:00Z"));
        assert_eq!(s.end_time.as_deref(), Some("2024-03-09T08:02:00Z"));
        assert!(s.avg_speed_kmh.is_some());
        assert!(s.max_speed_kmh.is_some());
        // max instantaneous >= average for a roughly constant pace.
        assert!(s.max_speed_kmh.unwrap() >= s.avg_speed_kmh.unwrap() - 0.01);
        assert!(s.avg_pace_min_per_km.is_some());
    }

    #[test]
    fn handles_self_closing_and_no_elevation() {
        let gpx = r#"<gpx><trk><trkseg>
            <trkpt lat="0.0" lon="0.0"/>
            <trkpt lat="0.0" lon="0.001"/>
        </trkseg></trk></gpx>"#;
        let s = analyze(gpx).unwrap();
        assert_eq!(s.points, 2);
        assert!(s.elevation_gain_m.is_none());
        assert!(s.duration_s.is_none());
        assert!(s.avg_speed_kmh.is_none());
        assert!(s.max_speed_kmh.is_none());
        assert!(s.km_splits.is_empty());
    }

    #[test]
    fn route_and_waypoint_elements_also_count() {
        let gpx = r#"<gpx><rte>
            <rtept lat="0.0" lon="0.0"/>
            <rtept lat="0.0" lon="0.001"/>
        </rte></gpx>"#;
        assert_eq!(analyze(gpx).unwrap().points, 2);
    }

    #[test]
    fn parse_time_handles_offset_and_fraction() {
        let z = parse_time("2024-03-09T08:00:00Z").unwrap();
        let frac = parse_time("2024-03-09T08:00:00.500Z").unwrap();
        assert!((frac - z - 0.5).abs() < 1e-6);
        let off = parse_time("2024-03-09T09:00:00+01:00").unwrap();
        assert!((off - z).abs() < 1e-6);
    }

    #[test]
    fn error_on_empty_or_no_points() {
        assert!(analyze("").is_err());
        assert!(analyze("<gpx><trk><trkseg></trkseg></trk></gpx>").is_err());
    }

    #[test]
    fn error_on_malformed_xml() {
        assert!(analyze("<gpx><trk><trkpt lat=").is_err());
    }

    #[test]
    fn json_and_text_render_smoke() {
        let j = analyze_json(SAMPLE).unwrap();
        assert!(j.contains("\"distance_km\""));
        let t = render(SAMPLE).unwrap();
        assert!(t.contains("Morning Run"));
        assert!(t.contains("Distance:"));
        assert!(t.contains("Max speed:"));
    }

    #[test]
    fn parses_garmin_sensor_extensions() {
        // Two points ~1 km apart over 5 minutes, with HR/cadence/power/temp.
        let gpx = r#"<gpx xmlns:gpxtpx="http://www.garmin.com/xmlschemas/TrackPointExtension/v1">
          <trk><trkseg>
            <trkpt lat="0.0" lon="0.0"><ele>10</ele><time>2024-01-01T00:00:00Z</time>
              <extensions><gpxtpx:TrackPointExtension>
                <gpxtpx:hr>120</gpxtpx:hr><gpxtpx:cad>80</gpxtpx:cad>
                <gpxtpx:atemp>15</gpxtpx:atemp><power>200</power>
              </gpxtpx:TrackPointExtension></extensions></trkpt>
            <trkpt lat="0.009" lon="0.0"><ele>20</ele><time>2024-01-01T00:05:00Z</time>
              <extensions><gpxtpx:TrackPointExtension>
                <gpxtpx:hr>160</gpxtpx:hr><gpxtpx:cad>90</gpxtpx:cad>
                <gpxtpx:atemp>17</gpxtpx:atemp><power>300</power>
              </gpxtpx:TrackPointExtension></extensions></trkpt>
          </trkseg></trk></gpx>"#;
        let s = analyze(gpx).unwrap();
        let hr = s.heart_rate_bpm.expect("hr");
        assert_eq!(hr.avg, 140.0);
        assert_eq!(hr.max, 160.0);
        assert_eq!(s.cadence_rpm.unwrap().avg, 85.0);
        assert_eq!(s.power_w.unwrap().max, 300.0);
        assert_eq!(s.temperature_c.unwrap().avg, 16.0);
    }

    #[test]
    fn splits_produced_for_a_long_timed_track() {
        // ~2.2 km straight north over 22 minutes → expect 2 full km splits + 1 partial.
        let mut gpx = String::from("<gpx><trk><trkseg>");
        for i in 0..=22 {
            let lat = 0.001 * i as f64; // 0.001 deg ≈ 111 m → 22 pts ≈ 2.45 km
            let min = i;
            gpx.push_str(&format!(
                "<trkpt lat=\"{lat}\" lon=\"0.0\"><ele>{}</ele><time>2024-01-01T00:{min:02}:00Z</time></trkpt>",
                100 + i
            ));
        }
        gpx.push_str("</trkseg></trk></gpx>");
        let s = analyze(&gpx).unwrap();
        assert!(
            s.km_splits.len() >= 2,
            "expected >=2 km splits, got {}",
            s.km_splits.len()
        );
        // first two splits are full (length 1.0) and have a pace
        assert_eq!(s.km_splits[0].length, 1.0);
        assert!(s.km_splits[0].pace.is_some());
        assert!(s.km_splits[0].duration_s.unwrap() > 0.0);
        // sum of split durations ≈ total duration
        let sum: f64 = s.km_splits.iter().filter_map(|x| x.duration_s).sum();
        assert!(
            (sum - s.duration_s.unwrap()).abs() < 5.0,
            "split durations {sum} vs total {:?}",
            s.duration_s
        );
    }
}
