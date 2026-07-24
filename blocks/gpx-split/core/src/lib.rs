//! gizza-ai/gpx-split core — split one GPX track into multiple segments by
//! distance covered, elapsed time, or detected stop/pause gaps, and emit either
//! a new multi-track GPX or a per-segment text summary. Pure-Rust (`quick-xml`),
//! no wafer/wasm-bindgen deps; shared by the chat/CLI block and the web page.

use quick_xml::events::Event;
use quick_xml::name::QName;
use quick_xml::reader::Reader;
use serde::Serialize;

/// One parsed track point. Raw attribute/element strings are preserved so the
/// re-emitted GPX keeps the source coordinate precision exactly.
#[derive(Debug, Clone, Default)]
struct Point {
    lat: f64,
    lon: f64,
    lat_s: String,
    lon_s: String,
    ele_s: Option<String>,
    time_iso: Option<String>,
    time: Option<f64>,
}

/// How the track is cut into segments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Start a new segment each time the cumulative distance reaches a threshold.
    Distance,
    /// Start a new segment each time the elapsed time reaches a threshold.
    Time,
    /// Start a new segment wherever the time gap between two consecutive points
    /// exceeds a threshold (a paused recording or a rest stop).
    Stops,
}

impl Mode {
    pub fn parse(s: &str) -> Result<Mode, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "distance" => Ok(Mode::Distance),
            "time" => Ok(Mode::Time),
            "stops" | "stop" => Ok(Mode::Stops),
            other => Err(format!(
                "unknown mode '{other}': expected one of distance, time, stops"
            )),
        }
    }
}

/// Distance unit for the `distance` mode threshold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Km,
    Mi,
}

impl Unit {
    pub fn parse(s: &str) -> Result<Unit, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "km" | "kilometers" | "kilometres" => Ok(Unit::Km),
            "mi" | "miles" | "mile" => Ok(Unit::Mi),
            other => Err(format!("unknown unit '{other}': expected km or mi")),
        }
    }
    fn metres(self) -> f64 {
        match self {
            Unit::Km => M_PER_KM,
            Unit::Mi => M_PER_MI,
        }
    }
    fn label(self) -> &'static str {
        match self {
            Unit::Km => "km",
            Unit::Mi => "mi",
        }
    }
}

/// What to emit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    /// A GPX document with one named `<trk>` per segment.
    Gpx,
    /// A human-readable per-segment summary (distance, duration, points).
    Summary,
}

impl Output {
    pub fn parse(s: &str) -> Result<Output, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "gpx" => Ok(Output::Gpx),
            "summary" => Ok(Output::Summary),
            other => Err(format!("unknown output '{other}': expected gpx or summary")),
        }
    }
}

/// Fully-resolved split configuration.
#[derive(Debug, Clone)]
pub struct Config {
    pub mode: Mode,
    /// Segment length in the chosen unit (distance mode).
    pub distance: f64,
    pub unit: Unit,
    /// Segment length in minutes (time mode).
    pub time_min: f64,
    /// Minimum time gap in seconds that starts a new segment (stops mode).
    pub stop_gap_s: f64,
    pub output: Output,
}

/// Per-segment statistics reported on the chat/CLI JSON surface.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SegmentStat {
    /// 1-based segment index.
    pub index: usize,
    /// Number of points in the segment.
    pub points: usize,
    pub distance_km: f64,
    pub distance_mi: f64,
    /// Duration in seconds (only when the segment's points carry timestamps).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_s: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_hms: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time: Option<String>,
}

/// Full result for the chat/CLI JSON surface.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SplitResult {
    pub mode: String,
    pub segment_count: usize,
    pub segments: Vec<SegmentStat>,
    /// The split GPX document (only when `output = gpx`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpx: Option<String>,
}

const EARTH_RADIUS_M: f64 = 6_371_000.0;
const M_PER_MI: f64 = 1609.344;
const M_PER_KM: f64 = 1000.0;

/// Great-circle distance between two points, in metres (haversine).
fn haversine(a: &Point, b: &Point) -> f64 {
    let (lat1, lat2) = (a.lat.to_radians(), b.lat.to_radians());
    let dlat = (b.lat - a.lat).to_radians();
    let dlon = (b.lon - a.lon).to_radians();
    let h = (dlat / 2.0).sin().powi(2) + lat1.cos() * lat2.cos() * (dlon / 2.0).sin().powi(2);
    2.0 * EARTH_RADIUS_M * h.sqrt().asin()
}

fn round(v: f64, n: i32) -> f64 {
    let f = 10f64.powi(n);
    (v * f).round() / f
}

/// Format seconds as `H:MM:SS`.
fn hms(total: f64) -> String {
    let t = total.max(0.0).round() as i64;
    format!("{}:{:02}:{:02}", t / 3600, (t % 3600) / 60, t % 60)
}

/// Parse an RFC 3339 / ISO-8601 GPX `<time>` into epoch seconds. Self-contained
/// (no chrono — the page target has no std clock). Ported from gpx-analyzer.
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
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    let mut epoch = days as f64 * 86400.0 + hour as f64 * 3600.0 + min as f64 * 60.0 + sec as f64;
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

fn local_name(name: QName) -> String {
    let full = name.as_ref();
    let local = match full.iter().position(|&b| b == b':') {
        Some(i) => &full[i + 1..],
        None => full,
    };
    String::from_utf8_lossy(local).into_owned()
}

/// Escape text for use in XML character data / attribute values.
fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

fn xml_unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

/// Parse a GPX document into its ordered track points plus the first track name.
fn parse_points(gpx: &str) -> Result<(Vec<Point>, Option<String>), String> {
    let mut reader = Reader::from_str(gpx);
    reader.config_mut().trim_text(false);
    let decoder = reader.decoder();

    let mut points: Vec<Point> = Vec::new();
    let mut name: Option<String> = None;

    let mut cur = Point::default();
    let mut have_lat = false;
    let mut have_lon = false;
    let mut in_point = false;
    let mut in_trk = false;
    // Text target: 1=ele, 2=time, 3=trk name
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
                            let val = attr.decode_and_unescape_value(decoder).unwrap_or_default();
                            match key.as_str() {
                                "lat" => {
                                    if let Ok(v) = val.trim().parse() {
                                        cur.lat = v;
                                        cur.lat_s = val.trim().to_string();
                                        have_lat = true;
                                    }
                                }
                                "lon" => {
                                    if let Ok(v) = val.trim().parse() {
                                        cur.lon = v;
                                        cur.lon_s = val.trim().to_string();
                                        have_lon = true;
                                    }
                                }
                                _ => {}
                            }
                        }
                        if is_empty {
                            if have_lat && have_lon {
                                points.push(cur.clone());
                            }
                            in_point = false;
                        }
                    }
                    "ele" if in_point => text_target = Some(1),
                    "time" if in_point => text_target = Some(2),
                    "name" if in_trk && name.is_none() => text_target = Some(3),
                    _ => {}
                }
            }
            Ok(Event::Text(t)) => {
                if let Some(target) = text_target {
                    let raw = xml_unescape(&t.decode().unwrap_or_default());
                    let s = raw.trim();
                    match target {
                        1 => cur.ele_s = Some(s.to_string()),
                        2 => {
                            cur.time = parse_time(s);
                            cur.time_iso = Some(s.to_string());
                        }
                        3 => {
                            let slot = name.get_or_insert_with(String::new);
                            slot.push_str(&raw);
                        }
                        _ => {}
                    }
                }
            }
            Ok(Event::GeneralRef(r)) => {
                if text_target == Some(3) {
                    let ch = if let Some(c) = r.resolve_char_ref().map_err(|e| format!("bad character reference in GPX: {e}"))? {
                        Some(c)
                    } else {
                        let ent = r.decode().map_err(|e| format!("bad entity reference in GPX: {e}"))?;
                        match ent.as_ref() {
                            "amp" => Some('&'),
                            "lt" => Some('<'),
                            "gt" => Some('>'),
                            "quot" => Some('"'),
                            "apos" => Some('\''),
                            _ => None,
                        }
                    };
                    if let Some(c) = ch {
                        name.get_or_insert_with(String::new).push(c);
                    }
                }
            }
            Ok(Event::End(e)) => {
                let local = local_name(e.name()).to_ascii_lowercase();
                match local.as_str() {
                    "ele" | "time" | "name" => text_target = None,
                    "trk" => in_trk = false,
                    "trkpt" | "rtept" | "wpt" => {
                        if have_lat && have_lon {
                            points.push(cur.clone());
                        }
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
    Ok((points, name))
}

/// True if at least one point carries a parseable timestamp.
fn any_timed(points: &[Point]) -> bool {
    points.iter().any(|p| p.time.is_some())
}

/// Cut the ordered points into segments per the config. Each returned segment
/// has at least one point; distance/time splits duplicate the boundary point so
/// consecutive segments stay geometrically contiguous, while stop splits leave
/// the genuine gap between segments.
fn segment(points: &[Point], cfg: &Config) -> Result<Vec<Vec<Point>>, String> {
    match cfg.mode {
        Mode::Distance => {
            let threshold = cfg.distance * cfg.unit.metres();
            if threshold <= 0.0 {
                return Err("distance must be greater than zero".into());
            }
            let mut segs: Vec<Vec<Point>> = Vec::new();
            let mut cur: Vec<Point> = vec![points[0].clone()];
            let mut acc = 0.0;
            for w in points.windows(2) {
                acc += haversine(&w[0], &w[1]);
                cur.push(w[1].clone());
                if acc >= threshold - 1e-9 {
                    segs.push(std::mem::take(&mut cur));
                    cur = vec![w[1].clone()]; // duplicate boundary point
                    acc = 0.0;
                }
            }
            if cur.len() >= 2 {
                segs.push(cur);
            }
            Ok(segs)
        }
        Mode::Time => {
            if !any_timed(points) {
                return Err(
                    "time mode needs <time> stamps on the track points; none found — use distance mode"
                        .into(),
                );
            }
            let threshold = cfg.time_min * 60.0;
            if threshold <= 0.0 {
                return Err("time_min must be greater than zero".into());
            }
            let mut segs: Vec<Vec<Point>> = Vec::new();
            let mut cur: Vec<Point> = vec![points[0].clone()];
            let mut seg_start = points[0].time;
            for w in points.windows(2) {
                cur.push(w[1].clone());
                if let (Some(t0), Some(t1)) = (seg_start, w[1].time) {
                    if t1 - t0 >= threshold - 1e-9 {
                        segs.push(std::mem::take(&mut cur));
                        cur = vec![w[1].clone()]; // duplicate boundary point
                        seg_start = w[1].time;
                    }
                }
            }
            if cur.len() >= 2 {
                segs.push(cur);
            }
            Ok(segs)
        }
        Mode::Stops => {
            if !any_timed(points) {
                return Err(
                    "stops mode needs <time> stamps on the track points to detect gaps; none found — use distance mode"
                        .into(),
                );
            }
            if cfg.stop_gap_s <= 0.0 {
                return Err("stop_gap_s must be greater than zero".into());
            }
            let mut segs: Vec<Vec<Point>> = Vec::new();
            let mut cur: Vec<Point> = vec![points[0].clone()];
            for w in points.windows(2) {
                let gap = match (w[0].time, w[1].time) {
                    (Some(a), Some(b)) => b - a,
                    _ => 0.0,
                };
                if gap > cfg.stop_gap_s {
                    // Genuine gap: the two points belong to different segments.
                    segs.push(std::mem::take(&mut cur));
                    cur = vec![w[1].clone()];
                } else {
                    cur.push(w[1].clone());
                }
            }
            if !cur.is_empty() {
                segs.push(cur);
            }
            Ok(segs)
        }
    }
}

/// Distance of a segment in metres (summed great-circle).
fn seg_distance_m(seg: &[Point]) -> f64 {
    seg.windows(2).map(|w| haversine(&w[0], &w[1])).sum()
}

fn seg_stat(index: usize, seg: &[Point]) -> SegmentStat {
    let d = seg_distance_m(seg);
    let timed: Vec<&Point> = seg.iter().filter(|p| p.time.is_some()).collect();
    let (duration_s, duration_hms, start_time, end_time) = if timed.len() >= 2 {
        let dur = (timed.last().unwrap().time.unwrap() - timed[0].time.unwrap()).max(0.0);
        (
            Some(round(dur, 1)),
            Some(hms(dur)),
            timed.first().and_then(|p| p.time_iso.clone()),
            timed.last().and_then(|p| p.time_iso.clone()),
        )
    } else {
        (None, None, None, None)
    };
    SegmentStat {
        index,
        points: seg.len(),
        distance_km: round(d / M_PER_KM, 3),
        distance_mi: round(d / M_PER_MI, 3),
        duration_s,
        duration_hms,
        start_time,
        end_time,
    }
}

/// Render the split segments as a GPX document with one named `<trk>` each.
fn build_gpx(name: &Option<String>, segs: &[Vec<Point>]) -> String {
    let base = name.as_deref().unwrap_or("Track");
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str(
        "<gpx version=\"1.1\" creator=\"gizza-ai/gpx-split\" xmlns=\"http://www.topografix.com/GPX/1/1\">\n",
    );
    for (i, seg) in segs.iter().enumerate() {
        out.push_str("  <trk>\n");
        out.push_str(&format!(
            "    <name>{} - Part {}</name>\n",
            xml_escape(base),
            i + 1
        ));
        out.push_str("    <trkseg>\n");
        for p in seg {
            out.push_str(&format!(
                "      <trkpt lat=\"{}\" lon=\"{}\">",
                xml_escape(&p.lat_s),
                xml_escape(&p.lon_s)
            ));
            if let Some(ele) = &p.ele_s {
                out.push_str(&format!("<ele>{}</ele>", xml_escape(ele)));
            }
            if let Some(t) = &p.time_iso {
                out.push_str(&format!("<time>{}</time>", xml_escape(t)));
            }
            out.push_str("</trkpt>\n");
        }
        out.push_str("    </trkseg>\n");
        out.push_str("  </trk>\n");
    }
    out.push_str("</gpx>\n");
    out
}

/// Format a float without a trailing `.0` (e.g. `5` not `5.0`).
fn trim_num(v: f64) -> String {
    if (v - v.round()).abs() < 1e-9 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v}")
    }
}

/// Short human phrase describing the split rule (for the summary header).
fn rule_phrase(cfg: &Config) -> String {
    match cfg.mode {
        Mode::Distance => format!("every {} {}", trim_num(cfg.distance), cfg.unit.label()),
        Mode::Time => format!("every {} min", trim_num(cfg.time_min)),
        Mode::Stops => format!("on gaps over {} s", trim_num(cfg.stop_gap_s)),
    }
}

fn mode_label(m: Mode) -> &'static str {
    match m {
        Mode::Distance => "distance",
        Mode::Time => "time",
        Mode::Stops => "stops",
    }
}

/// Compute the split and its per-segment stats.
#[allow(clippy::type_complexity)]
fn compute(
    gpx: &str,
    cfg: &Config,
) -> Result<(Option<String>, Vec<Vec<Point>>, Vec<SegmentStat>), String> {
    let (points, name) = parse_points(gpx)?;
    let segs = segment(&points, cfg)?;
    let stats: Vec<SegmentStat> = segs
        .iter()
        .enumerate()
        .map(|(i, s)| seg_stat(i + 1, s))
        .collect();
    Ok((name, segs, stats))
}

/// JSON for the chat/CLI surface.
pub fn split_json(gpx: &str, cfg: &Config) -> Result<String, String> {
    let (name, segs, stats) = compute(gpx, cfg)?;
    let gpx_out = match cfg.output {
        Output::Gpx => Some(build_gpx(&name, &segs)),
        Output::Summary => None,
    };
    let result = SplitResult {
        mode: mode_label(cfg.mode).to_string(),
        segment_count: segs.len(),
        segments: stats,
        gpx: gpx_out,
    };
    serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
}

/// Text for the page surface: the split GPX (output=gpx) or a summary table.
pub fn render(gpx: &str, cfg: &Config) -> Result<String, String> {
    let (name, segs, stats) = compute(gpx, cfg)?;
    match cfg.output {
        Output::Gpx => Ok(build_gpx(&name, &segs).trim_end().to_string()),
        Output::Summary => {
            let mut out = String::new();
            out.push_str(&format!(
                "Split into {} segment{} ({}, {}).\n",
                segs.len(),
                if segs.len() == 1 { "" } else { "s" },
                mode_label(cfg.mode),
                rule_phrase(cfg)
            ));
            for s in &stats {
                out.push_str(&format!(
                    "\nSegment {}: {:.2} km ({:.2} mi), {} points",
                    s.index, s.distance_km, s.distance_mi, s.points
                ));
                if let Some(h) = &s.duration_hms {
                    out.push_str(&format!(", {h}"));
                }
                if let (Some(a), Some(b)) = (&s.start_time, &s.end_time) {
                    out.push_str(&format!("  [{a} → {b}]"));
                }
            }
            Ok(out.trim_end().to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(mode: Mode) -> Config {
        Config {
            mode,
            distance: 1.0,
            unit: Unit::Km,
            time_min: 5.0,
            stop_gap_s: 120.0,
            output: Output::Gpx,
        }
    }

    /// ~2.45 km straight north over 22 min, one point per minute.
    fn long_track() -> String {
        let mut gpx = String::from("<gpx><trk><name>Run</name><trkseg>");
        for i in 0..=22 {
            let lat = 0.001 * i as f64; // 0.001 deg ≈ 111 m
            gpx.push_str(&format!(
                "<trkpt lat=\"{lat:.6}\" lon=\"0.000000\"><ele>{}</ele><time>2024-01-01T00:{i:02}:00Z</time></trkpt>",
                100 + i
            ));
        }
        gpx.push_str("</trkseg></trk></gpx>");
        gpx
    }

    #[test]
    fn distance_split_makes_multiple_full_segments() {
        let c = cfg(Mode::Distance); // every 1 km
        let (_, segs, stats) = compute(&long_track(), &c).unwrap();
        // ~2.45 km → 2 full km segments + a partial remainder.
        assert!(segs.len() >= 2, "got {} segments", segs.len());
        assert!(stats[0].distance_km >= 0.99, "first {}", stats[0].distance_km);
        // Consecutive segments are contiguous (boundary point duplicated).
        assert_eq!(
            segs[0].last().unwrap().lat_s,
            segs[1].first().unwrap().lat_s
        );
    }

    #[test]
    fn time_split_by_ten_minutes() {
        let mut c = cfg(Mode::Time);
        c.time_min = 10.0;
        let (_, segs, stats) = compute(&long_track(), &c).unwrap();
        // 22 min → segments of ~10 min each.
        assert!(segs.len() >= 2, "got {}", segs.len());
        assert!(stats[0].duration_s.unwrap() >= 599.0);
    }

    #[test]
    fn stops_split_at_a_time_gap() {
        // Two clusters separated by a 20-minute recording gap.
        let gpx = r#"<gpx><trk><trkseg>
            <trkpt lat="0.0" lon="0.0"><time>2024-01-01T00:00:00Z</time></trkpt>
            <trkpt lat="0.001" lon="0.0"><time>2024-01-01T00:01:00Z</time></trkpt>
            <trkpt lat="0.002" lon="0.0"><time>2024-01-01T00:21:00Z</time></trkpt>
            <trkpt lat="0.003" lon="0.0"><time>2024-01-01T00:22:00Z</time></trkpt>
        </trkseg></trk></gpx>"#;
        let mut c = cfg(Mode::Stops);
        c.stop_gap_s = 120.0; // 2 min
        let (_, segs, _) = compute(gpx, &c).unwrap();
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[0].len(), 2);
        assert_eq!(segs[1].len(), 2);
        // No boundary-point duplication for a genuine gap.
        assert_ne!(segs[0].last().unwrap().lat_s, segs[1].first().unwrap().lat_s);
    }

    #[test]
    fn gpx_output_is_valid_and_reparseable() {
        let c = cfg(Mode::Distance);
        let out = render(&long_track(), &c).unwrap();
        assert!(out.contains("<gpx"));
        assert!(out.contains("Run - Part 1"));
        assert!(out.contains("<ele>100</ele>"));
        // The emitted GPX round-trips back through the parser.
        let (pts, _) = parse_points(&out).unwrap();
        assert!(pts.len() > 20);
    }

    #[test]
    fn summary_output_lists_each_segment() {
        let mut c = cfg(Mode::Distance);
        c.output = Output::Summary;
        let out = render(&long_track(), &c).unwrap();
        assert!(out.starts_with("Split into"));
        assert!(out.contains("Segment 1:"));
        assert!(out.contains("km"));
    }

    #[test]
    fn miles_unit_makes_fewer_segments_than_km() {
        let track = long_track();
        let mut km = cfg(Mode::Distance);
        km.distance = 1.0;
        km.unit = Unit::Km;
        let mut mi = cfg(Mode::Distance);
        mi.distance = 1.0;
        mi.unit = Unit::Mi; // 1 mile > 1 km → fewer cuts
        let n_km = compute(&track, &km).unwrap().1.len();
        let n_mi = compute(&track, &mi).unwrap().1.len();
        assert!(n_km > n_mi, "km {n_km} should exceed mi {n_mi}");
    }

    #[test]
    fn json_surface_includes_segments_and_gpx() {
        let c = cfg(Mode::Distance);
        let j = split_json(&long_track(), &c).unwrap();
        assert!(j.contains("\"segment_count\""));
        assert!(j.contains("\"distance_km\""));
        assert!(j.contains("\"gpx\""));
    }

    #[test]
    fn summary_json_omits_gpx() {
        let mut c = cfg(Mode::Distance);
        c.output = Output::Summary;
        let j = split_json(&long_track(), &c).unwrap();
        assert!(!j.contains("\"gpx\""));
    }

    #[test]
    fn time_mode_without_timestamps_errors() {
        let gpx = r#"<gpx><trk><trkseg>
            <trkpt lat="0.0" lon="0.0"/>
            <trkpt lat="0.0" lon="0.001"/>
        </trkseg></trk></gpx>"#;
        let mut c = cfg(Mode::Time);
        c.output = Output::Summary;
        assert!(render(gpx, &c).is_err());
    }

    #[test]
    fn error_on_empty_or_single_point() {
        let c = cfg(Mode::Distance);
        assert!(split_json("", &c).is_err());
        assert!(split_json(
            "<gpx><trk><trkseg><trkpt lat=\"0\" lon=\"0\"/></trkseg></trk></gpx>",
            &c
        )
        .is_err());
    }

    #[test]
    fn error_on_malformed_xml() {
        let c = cfg(Mode::Distance);
        assert!(split_json("<gpx><trk><trkpt lat=", &c).is_err());
    }

    #[test]
    fn xml_special_chars_in_name_are_escaped() {
        let gpx = r#"<gpx><trk><name>A &amp; B</name><trkseg>
            <trkpt lat="0.0" lon="0.0"/>
            <trkpt lat="0.001" lon="0.0"/>
        </trkseg></trk></gpx>"#;
        let c = cfg(Mode::Distance);
        let out = render(gpx, &c).unwrap();
        assert!(out.contains("A &amp; B - Part 1"));
    }

    #[test]
    fn parse_helpers_reject_bad_values() {
        assert!(Mode::parse("bogus").is_err());
        assert!(Unit::parse("furlongs").is_err());
        assert!(Output::parse("pdf").is_err());
        assert_eq!(Mode::parse("Stops").unwrap(), Mode::Stops);
        assert_eq!(Unit::parse("miles").unwrap(), Unit::Mi);
    }
}
