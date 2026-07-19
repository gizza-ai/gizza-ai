//! gizza-ai/gpx-to-csv core — turn a GPX document into a CSV table, one row per
//! track / route / waypoint point (lat, lon, elevation, time, optional derived
//! speed, and Garmin-style sensor extensions). Pure-Rust (`quick-xml`), no
//! wafer/wasm-bindgen deps — shared by the chat block, the CLI, and the page.

use quick_xml::events::Event;
use quick_xml::name::QName;
use quick_xml::reader::Reader;

/// Which point types to include in the output.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PointFilter {
    All,
    Track,
    Route,
    Waypoint,
}

impl PointFilter {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "all" => Ok(PointFilter::All),
            "track" => Ok(PointFilter::Track),
            "route" => Ok(PointFilter::Route),
            "waypoint" => Ok(PointFilter::Waypoint),
            other => Err(format!(
                "invalid points '{other}': expected one of all, track, route, waypoint"
            )),
        }
    }
    fn matches(self, kind: PointKind) -> bool {
        match self {
            PointFilter::All => true,
            PointFilter::Track => kind == PointKind::Track,
            PointFilter::Route => kind == PointKind::Route,
            PointFilter::Waypoint => kind == PointKind::Waypoint,
        }
    }
}

/// Output field separator.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Delimiter {
    Comma,
    Semicolon,
    Tab,
    Pipe,
}

impl Delimiter {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "comma" | "," => Ok(Delimiter::Comma),
            "semicolon" | ";" => Ok(Delimiter::Semicolon),
            "tab" | "\t" => Ok(Delimiter::Tab),
            "pipe" | "|" => Ok(Delimiter::Pipe),
            other => Err(format!(
                "invalid delimiter '{other}': expected one of comma, semicolon, tab, pipe"
            )),
        }
    }
    fn ch(self) -> char {
        match self {
            Delimiter::Comma => ',',
            Delimiter::Semicolon => ';',
            Delimiter::Tab => '\t',
            Delimiter::Pipe => '|',
        }
    }
}

/// How the `time` column is written (or dropped).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TimeFormat {
    Iso,
    Unix,
    None,
}

impl TimeFormat {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "iso" => Ok(TimeFormat::Iso),
            "unix" | "epoch" => Ok(TimeFormat::Unix),
            "none" | "off" => Ok(TimeFormat::None),
            other => Err(format!(
                "invalid time_format '{other}': expected one of iso, unix, none"
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
enum PointKind {
    #[default]
    Track,
    Route,
    Waypoint,
}

impl PointKind {
    fn label(self) -> &'static str {
        match self {
            PointKind::Track => "track",
            PointKind::Route => "route",
            PointKind::Waypoint => "waypoint",
        }
    }
}

/// One parsed point. Coordinate / elevation / sensor values keep their raw
/// source text (so precision is preserved verbatim in the CSV); numeric parses
/// are only used for the derived-speed calculation.
#[derive(Debug, Clone, Default)]
struct Point {
    kind: PointKind,
    name: String,
    desc: String,
    lat_raw: String,
    lon_raw: String,
    lat: f64,
    lon: f64,
    ele: String,
    time_iso: String,
    time_epoch: Option<f64>,
    hr: String,
    cad: String,
    power: String,
    temp: String,
    /// Sequence id — points sharing a value are adjacent along one trkseg / rte
    /// (waypoints each get a unique id so they never chain for speed).
    seg: u32,
    /// Derived ground speed at this point, km/h (distance from the previous
    /// point in the same sequence ÷ Δt). None for the first point of a segment
    /// or when a timestamp is missing.
    speed: Option<f64>,
}

const EARTH_RADIUS_M: f64 = 6_371_000.0;

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

/// Parse an RFC 3339 / ISO-8601 timestamp (the GPX `<time>` format, e.g.
/// `2024-03-09T08:15:30Z`, optional fractional second and `±HH:MM` offset) into
/// epoch seconds. Self-contained (no chrono — the page target has no std clock).
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
    // Days since Unix epoch (civil-from-days, Howard Hinnant's algorithm).
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

/// Strip any `ns:` prefix from an XML qualified name.
fn local_name(name: QName) -> String {
    let full = name.as_ref();
    let local = match full.iter().position(|&b| b == b':') {
        Some(i) => &full[i + 1..],
        None => full,
    };
    String::from_utf8_lossy(local).into_owned()
}

/// Parse a GPX document into an ordered list of points (track, then route, then
/// waypoint — in document order). Errors on malformed XML.
fn parse(gpx: &str) -> Result<Vec<Point>, String> {
    if gpx.trim().is_empty() {
        return Err("no GPX provided: paste a GPX document (an XML file with <trkpt>, <rtept>, or <wpt> elements)".into());
    }
    let mut reader = Reader::from_str(gpx);
    reader.config_mut().trim_text(true);
    let decoder = reader.decoder();

    let mut points: Vec<Point> = Vec::new();
    let mut cur = Point::default();
    let mut have_lat = false;
    let mut have_lon = false;
    let mut in_point = false;
    let mut seg_counter: u32 = 0;
    let mut cur_seg: u32 = 0;
    // Text target: 1=ele 2=time 3=name 4=hr 5=cad 6=power 7=temp 8=desc
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
                let kind = match local.as_str() {
                    "trkpt" => Some(PointKind::Track),
                    "rtept" => Some(PointKind::Route),
                    "wpt" => Some(PointKind::Waypoint),
                    _ => None,
                };
                match local.as_str() {
                    "trkseg" | "rte" => {
                        // A new track segment / route begins a fresh speed chain.
                        seg_counter += 1;
                        cur_seg = seg_counter;
                    }
                    _ if kind.is_some() => {
                        let kind = kind.unwrap();
                        in_point = true;
                        cur = Point::default();
                        cur.kind = kind;
                        have_lat = false;
                        have_lon = false;
                        cur.seg = if kind == PointKind::Waypoint {
                            seg_counter += 1;
                            seg_counter
                        } else {
                            cur_seg
                        };
                        for attr in e.attributes().flatten() {
                            let key = local_name(QName(attr.key.as_ref())).to_ascii_lowercase();
                            #[allow(deprecated)]
                            let val = attr.decode_and_unescape_value(decoder).unwrap_or_default();
                            let val = val.trim();
                            match key.as_str() {
                                "lat" => {
                                    cur.lat_raw = val.to_string();
                                    if let Ok(v) = val.parse() {
                                        cur.lat = v;
                                        have_lat = true;
                                    }
                                }
                                "lon" => {
                                    cur.lon_raw = val.to_string();
                                    if let Ok(v) = val.parse() {
                                        cur.lon = v;
                                        have_lon = true;
                                    }
                                }
                                _ => {}
                            }
                        }
                        if is_empty {
                            commit(&mut points, &cur, have_lat, have_lon);
                            in_point = false;
                        }
                    }
                    "ele" if in_point => text_target = Some(1),
                    "time" if in_point => text_target = Some(2),
                    "name" if in_point => text_target = Some(3),
                    "hr" | "heartrate" if in_point => text_target = Some(4),
                    "cad" | "cadence" if in_point => text_target = Some(5),
                    "power" | "watts" if in_point => text_target = Some(6),
                    "atemp" | "temp" | "wtemp" | "temperature" if in_point => text_target = Some(7),
                    "desc" | "cmt" if in_point => text_target = Some(8),
                    _ => {}
                }
            }
            Ok(Event::Text(t)) => {
                if let Some(target) = text_target {
                    let raw = t.decode().unwrap_or_default().into_owned();
                    let s = raw.trim();
                    match target {
                        1 => cur.ele = s.to_string(),
                        2 => {
                            cur.time_iso = s.to_string();
                            cur.time_epoch = parse_time(s);
                        }
                        3 => cur.name = s.to_string(),
                        4 => cur.hr = s.to_string(),
                        5 => cur.cad = s.to_string(),
                        6 => cur.power = s.to_string(),
                        7 => cur.temp = s.to_string(),
                        8 => cur.desc = s.to_string(),
                        _ => {}
                    }
                }
            }
            Ok(Event::End(e)) => {
                let local = local_name(e.name()).to_ascii_lowercase();
                match local.as_str() {
                    "ele" | "time" | "name" | "hr" | "heartrate" | "cad" | "cadence" | "power"
                    | "watts" | "atemp" | "temp" | "wtemp" | "temperature" | "desc" | "cmt" => {
                        text_target = None
                    }
                    "trkpt" | "rtept" | "wpt" => {
                        commit(&mut points, &cur, have_lat, have_lon);
                        in_point = false;
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        buf.clear();
    }

    // Derive per-point speed along each sequence.
    for i in 1..points.len() {
        if points[i].seg != points[i - 1].seg {
            continue;
        }
        if let (Some(t0), Some(t1)) = (points[i - 1].time_epoch, points[i].time_epoch) {
            let dt = t1 - t0;
            if dt > 0.0 {
                let dist = haversine(&points[i - 1], &points[i]);
                points[i].speed = Some(dist / dt * 3.6);
            }
        }
    }

    Ok(points)
}

fn commit(points: &mut Vec<Point>, cur: &Point, have_lat: bool, have_lon: bool) {
    if have_lat && have_lon {
        points.push(cur.clone());
    }
}

/// RFC-4180 escape: quote a field that contains the delimiter, a double-quote,
/// CR, or LF, doubling any interior quote.
fn escape(field: &str, delim: char) -> String {
    let needs = field.contains(delim)
        || field.contains('"')
        || field.contains('\n')
        || field.contains('\r');
    if needs {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

/// Format epoch seconds — integer when whole, else the shortest decimal.
fn fmt_epoch(e: f64) -> String {
    if e.fract() == 0.0 {
        format!("{}", e as i64)
    } else {
        format!("{e}")
    }
}

/// Convert a GPX document into CSV. See the module docs / descriptor for the
/// column set and parameter semantics.
pub fn convert(
    gpx: &str,
    points: PointFilter,
    delimiter: Delimiter,
    header: bool,
    time_format: TimeFormat,
    speed: bool,
) -> Result<String, String> {
    let all = parse(gpx)?;
    if all.is_empty() {
        return Err("no points found: a GPX document needs <trkpt>, <rtept>, or <wpt> elements with lat/lon coordinates".into());
    }
    let rows: Vec<&Point> = all.iter().filter(|p| points.matches(p.kind)).collect();
    if rows.is_empty() {
        return Err(format!(
            "no matching points: this GPX has no {} points (try points=all)",
            match points {
                PointFilter::Track => "track",
                PointFilter::Route => "route",
                PointFilter::Waypoint => "waypoint",
                PointFilter::All => "usable",
            }
        ));
    }

    let show_time = time_format != TimeFormat::None;
    // Sensor columns are emitted only when at least one output row carries them.
    let has_hr = rows.iter().any(|p| !p.hr.is_empty());
    let has_cad = rows.iter().any(|p| !p.cad.is_empty());
    let has_power = rows.iter().any(|p| !p.power.is_empty());
    let has_temp = rows.iter().any(|p| !p.temp.is_empty());

    let delim = delimiter.ch();
    let mut out = String::new();

    let mut cols: Vec<&str> = vec!["point_type", "name", "latitude", "longitude", "elevation_m"];
    if show_time {
        cols.push("time");
    }
    if speed {
        cols.push("speed_kmh");
    }
    if has_hr {
        cols.push("heart_rate_bpm");
    }
    if has_cad {
        cols.push("cadence_rpm");
    }
    if has_power {
        cols.push("power_w");
    }
    if has_temp {
        cols.push("temperature_c");
    }

    if header {
        let line: Vec<String> = cols.iter().map(|c| escape(c, delim)).collect();
        out.push_str(&line.join(&delim.to_string()));
        out.push('\n');
    }

    for p in &rows {
        let mut fields: Vec<String> = Vec::with_capacity(cols.len());
        // point name falls back to the point's description/comment when unnamed.
        let name = if !p.name.is_empty() {
            &p.name
        } else {
            &p.desc
        };
        fields.push(p.kind.label().to_string());
        fields.push(name.clone());
        fields.push(p.lat_raw.clone());
        fields.push(p.lon_raw.clone());
        fields.push(p.ele.clone());
        if show_time {
            let cell = match time_format {
                TimeFormat::Iso => p.time_iso.clone(),
                TimeFormat::Unix => {
                    if p.time_iso.is_empty() {
                        String::new()
                    } else {
                        match p.time_epoch {
                            Some(e) => fmt_epoch(e),
                            None => {
                                return Err(format!(
                                    "could not parse timestamp '{}' for time_format=unix; use time_format=iso to keep the original text",
                                    p.time_iso
                                ))
                            }
                        }
                    }
                }
                TimeFormat::None => unreachable!(),
            };
            fields.push(cell);
        }
        if speed {
            fields.push(match p.speed {
                Some(v) => format!("{}", round(v, 2)),
                None => String::new(),
            });
        }
        if has_hr {
            fields.push(p.hr.clone());
        }
        if has_cad {
            fields.push(p.cad.clone());
        }
        if has_power {
            fields.push(p.power.clone());
        }
        if has_temp {
            fields.push(p.temp.clone());
        }

        let line: Vec<String> = fields.iter().map(|f| escape(f, delim)).collect();
        out.push_str(&line.join(&delim.to_string()));
        out.push('\n');
    }

    Ok(out.trim_end_matches('\n').to_string())
}

/// String-argument convenience used by the chat block, CLI, and web wrapper.
pub fn convert_str(
    gpx: &str,
    points: &str,
    delimiter: &str,
    header: bool,
    time_format: &str,
    speed: bool,
) -> Result<String, String> {
    convert(
        gpx,
        PointFilter::parse(points)?,
        Delimiter::parse(delimiter)?,
        header,
        TimeFormat::parse(time_format)?,
        speed,
    )
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
    </trkseg>
  </trk>
</gpx>"#;

    fn conv(gpx: &str, p: &str, d: &str, h: bool, t: &str, s: bool) -> String {
        convert_str(gpx, p, d, h, t, s).unwrap()
    }

    #[test]
    fn basic_track_iso_header() {
        let out = conv(SAMPLE, "all", "comma", true, "iso", false);
        assert_eq!(
            out,
            "point_type,name,latitude,longitude,elevation_m,time\n\
             track,,40.000000,-105.000000,1600,2024-03-09T08:00:00Z\n\
             track,,40.001000,-105.000000,1610,2024-03-09T08:01:00Z"
        );
    }

    #[test]
    fn header_false_drops_header_row() {
        let out = conv(SAMPLE, "all", "comma", false, "iso", false);
        assert!(!out.starts_with("point_type"));
        assert_eq!(out.lines().count(), 2);
    }

    #[test]
    fn time_format_none_omits_time_column() {
        let out = conv(SAMPLE, "all", "comma", true, "none", false);
        assert_eq!(out.lines().next().unwrap(), "point_type,name,latitude,longitude,elevation_m");
        assert!(!out.contains("2024-03-09"));
    }

    #[test]
    fn time_format_unix_epoch() {
        let out = conv(SAMPLE, "all", "comma", true, "unix", false);
        // 2024-03-09T08:00:00Z == 1709971200
        assert!(out.contains(",1709971200"), "got {out}");
    }

    #[test]
    fn speed_column_derived() {
        // ~111 m north in 60 s ≈ 6.68 km/h; first point blank.
        let out = conv(SAMPLE, "all", "comma", true, "iso", true);
        assert!(out.lines().next().unwrap().ends_with(",speed_kmh"));
        let last = out.lines().last().unwrap();
        assert!(last.contains(",6.") || last.contains(",7."), "speed in {last}");
        // first data row has empty speed cell (ends with comma).
        let first_row = out.lines().nth(1).unwrap();
        assert!(first_row.ends_with(','), "first speed empty: {first_row}");
    }

    #[test]
    fn delimiters_semicolon_tab_pipe() {
        assert!(conv(SAMPLE, "all", "semicolon", true, "none", false).contains("point_type;name;"));
        assert!(conv(SAMPLE, "all", "tab", true, "none", false).contains("point_type\tname\t"));
        assert!(conv(SAMPLE, "all", "pipe", true, "none", false).contains("point_type|name|"));
    }

    #[test]
    fn point_filter_track_route_waypoint() {
        let gpx = r#"<gpx>
          <wpt lat="1.0" lon="2.0"><name>Home</name></wpt>
          <rte><rtept lat="3.0" lon="4.0"/><rtept lat="3.1" lon="4.1"/></rte>
          <trk><trkseg><trkpt lat="5.0" lon="6.0"/></trkseg></trk>
        </gpx>"#;
        let all = conv(gpx, "all", "comma", false, "none", false);
        assert_eq!(all.lines().count(), 4);
        let wpt = conv(gpx, "waypoint", "comma", false, "none", false);
        assert_eq!(wpt, "waypoint,Home,1.0,2.0,");
        let rte = conv(gpx, "route", "comma", false, "none", false);
        assert_eq!(rte.lines().count(), 2);
        assert!(rte.lines().all(|l| l.starts_with("route,")));
        let trk = conv(gpx, "track", "comma", false, "none", false);
        assert_eq!(trk, "track,,5.0,6.0,");
    }

    #[test]
    fn waypoint_name_and_desc_fallback() {
        let gpx = r#"<gpx>
          <wpt lat="1.0" lon="2.0"><desc>Trail head</desc></wpt>
        </gpx>"#;
        let out = conv(gpx, "all", "comma", false, "none", false);
        assert_eq!(out, "waypoint,Trail head,1.0,2.0,");
    }

    #[test]
    fn sensor_columns_only_when_present() {
        let gpx = r#"<gpx xmlns:gpxtpx="http://www.garmin.com/xmlschemas/TrackPointExtension/v1">
          <trk><trkseg>
            <trkpt lat="0.0" lon="0.0"><ele>10</ele><time>2024-01-01T00:00:00Z</time>
              <extensions><gpxtpx:TrackPointExtension>
                <gpxtpx:hr>120</gpxtpx:hr><gpxtpx:cad>80</gpxtpx:cad>
                <gpxtpx:atemp>15</gpxtpx:atemp><power>200</power>
              </gpxtpx:TrackPointExtension></extensions></trkpt>
          </trkseg></trk></gpx>"#;
        let out = conv(gpx, "all", "comma", true, "iso", false);
        let header = out.lines().next().unwrap();
        assert!(header.ends_with("heart_rate_bpm,cadence_rpm,power_w,temperature_c"), "{header}");
        assert!(out.contains(",120,80,200,15"));
    }

    #[test]
    fn rfc4180_escaping() {
        let gpx = r#"<gpx>
          <wpt lat="1.0" lon="2.0"><name>Cafe, "The Spot"</name></wpt>
        </gpx>"#;
        let out = conv(gpx, "all", "comma", false, "none", false);
        assert_eq!(out, "waypoint,\"Cafe, \"\"The Spot\"\"\",1.0,2.0,");
    }

    #[test]
    fn coordinates_preserved_verbatim() {
        let gpx = r#"<gpx><trk><trkseg>
          <trkpt lat="40.0000000" lon="-105.1234500"/>
        </trkseg></trk></gpx>"#;
        let out = conv(gpx, "all", "comma", false, "none", false);
        assert!(out.contains("40.0000000,-105.1234500"), "{out}");
    }

    #[test]
    fn errors_are_helpful() {
        assert!(convert_str("", "all", "comma", true, "iso", false)
            .unwrap_err()
            .contains("no GPX"));
        assert!(convert_str("<gpx></gpx>", "all", "comma", true, "iso", false)
            .unwrap_err()
            .contains("no points"));
        assert!(convert_str("<gpx><trkpt lat=", "all", "comma", true, "iso", false)
            .unwrap_err()
            .contains("malformed"));
        // filter with no match
        let only_wpt = r#"<gpx><wpt lat="1" lon="2"/></gpx>"#;
        assert!(convert_str(only_wpt, "track", "comma", true, "iso", false)
            .unwrap_err()
            .contains("no matching"));
        // bad enum
        assert!(PointFilter::parse("nope").is_err());
        assert!(Delimiter::parse("colon").is_err());
        assert!(TimeFormat::parse("rfc").is_err());
    }

    #[test]
    fn bad_time_for_unix_errors_but_iso_keeps_text() {
        let gpx = r#"<gpx><trk><trkseg>
          <trkpt lat="1.0" lon="2.0"><time>not-a-date</time></trkpt>
        </trkseg></trk></gpx>"#;
        assert!(convert_str(gpx, "all", "comma", true, "unix", false)
            .unwrap_err()
            .contains("could not parse timestamp"));
        // iso keeps the raw text untouched.
        let iso = conv(gpx, "all", "comma", true, "iso", false);
        assert!(iso.contains(",not-a-date"));
    }

    #[test]
    fn self_closing_points_and_no_elevation() {
        let gpx = r#"<gpx><trk><trkseg>
          <trkpt lat="0.0" lon="0.0"/>
          <trkpt lat="0.0" lon="0.001"/>
        </trkseg></trk></gpx>"#;
        let out = conv(gpx, "all", "comma", true, "none", false);
        // elevation column present but empty.
        assert!(out.contains("track,,0.0,0.0,\n"));
    }
}
