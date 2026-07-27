//! gpx-to-kml core — convert a GPX GPS document into KML for Google Earth.
//! Pure-Rust (`quick-xml`), no wafer/wasm-bindgen deps; shared by the chat
//! skill block and the web page.
//!
//! Mapping: a track's segments (`<trk>`/`<trkseg>`/`<trkpt>`) become a KML
//! `<LineString>` (wrapped in a `<MultiGeometry>` when the track has more than
//! one segment); a route (`<rte>`/`<rtept>`) becomes a `<LineString>`; a
//! waypoint (`<wpt>`) becomes a `<Point>`. Names and descriptions (`<name>`,
//! `<desc>`, falling back to `<cmt>`) are carried onto each Placemark;
//! elevation (`<ele>`) becomes the third `lon,lat,ele` coordinate; per-point
//! timestamps become a track `<TimeSpan>` (first→last point) and a waypoint
//! `<TimeStamp>`.
//!
//! Styling is emitted as two shared `<Style>` blocks the Placemarks reference:
//! a `LineStyle` (color + width) for tracks/routes and an `IconStyle` (color)
//! for waypoints. The user's CSS `#RRGGBB` + opacity% are converted to KML's
//! `aabbggrr` byte order (alpha, blue, green, red — the reverse of CSS with an
//! alpha byte prepended).

use quick_xml::encoding::Decoder;
use quick_xml::events::{BytesStart, BytesText, Event};
use quick_xml::name::QName;
use quick_xml::reader::Reader;

/// How Google Earth interprets each coordinate's altitude.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AltitudeMode {
    /// Ignore altitude; drape geometry on the terrain (KML `clampToGround`).
    ClampToGround,
    /// Altitude is metres above sea level (KML `absolute`).
    Absolute,
    /// Altitude is metres above the terrain (KML `relativeToGround`).
    RelativeToGround,
}

impl AltitudeMode {
    /// Parse the tool's snake_case parameter value.
    pub fn parse(s: &str) -> Result<AltitudeMode, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "clamp_to_ground" | "clamptoground" | "" => Ok(AltitudeMode::ClampToGround),
            "absolute" => Ok(AltitudeMode::Absolute),
            "relative_to_ground" | "relativetoground" => Ok(AltitudeMode::RelativeToGround),
            other => Err(format!(
                "unknown altitude_mode '{other}' (use clamp_to_ground, absolute, or relative_to_ground)"
            )),
        }
    }

    /// The KML `<altitudeMode>` token.
    fn kml(self) -> &'static str {
        match self {
            AltitudeMode::ClampToGround => "clampToGround",
            AltitudeMode::Absolute => "absolute",
            AltitudeMode::RelativeToGround => "relativeToGround",
        }
    }
}

/// Conversion options (all in-model page/CLI parameters besides the GPX input).
pub struct Options {
    /// Track/route line color as CSS `#RRGGBB` (or `#RGB`, `#` optional).
    pub line_color: String,
    /// Line width in pixels (Google Earth pen width).
    pub line_width: u32,
    /// Line opacity, 0 (transparent) – 100 (opaque).
    pub line_opacity: u32,
    /// Waypoint icon color as CSS `#RRGGBB`.
    pub waypoint_color: String,
    /// How Google Earth reads each coordinate's altitude.
    pub altitude_mode: AltitudeMode,
    /// Optional KML `<Document>` name; falls back to the GPX `<metadata><name>`,
    /// then a generic label. An empty string means "not set".
    pub document_name: Option<String>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            line_color: "#ef4444".to_string(),
            line_width: 4,
            line_opacity: 80,
            waypoint_color: "#3b82f6".to_string(),
            altitude_mode: AltitudeMode::ClampToGround,
            document_name: None,
        }
    }
}

/// Convert a CSS hex color + opacity percentage into a KML `aabbggrr` color.
///
/// KML `<color>` is `aabbggrr` (alpha, blue, green, red) — the reverse byte
/// order of CSS `#rrggbb`, with an alpha byte prepended. Accepts `#RRGGBB`,
/// `RRGGBB`, `#RGB`, or `RGB`.
pub fn css_to_kml_color(css: &str, opacity_pct: u32) -> Result<String, String> {
    let h = css.trim().trim_start_matches('#');
    let (r, g, b) = match h.len() {
        6 => {
            let r = u8::from_str_radix(&h[0..2], 16);
            let g = u8::from_str_radix(&h[2..4], 16);
            let b = u8::from_str_radix(&h[4..6], 16);
            match (r, g, b) {
                (Ok(r), Ok(g), Ok(b)) => (r, g, b),
                _ => return Err(format!("invalid hex color '{css}' (expected #RRGGBB)")),
            }
        }
        3 => {
            // #RGB shorthand: each digit is doubled (#f00 → #ff0000).
            let expand = |c: &str| u8::from_str_radix(&c.repeat(2), 16);
            let r = expand(&h[0..1]);
            let g = expand(&h[1..2]);
            let b = expand(&h[2..3]);
            match (r, g, b) {
                (Ok(r), Ok(g), Ok(b)) => (r, g, b),
                _ => return Err(format!("invalid hex color '{css}' (expected #RGB)")),
            }
        }
        _ => {
            return Err(format!(
                "invalid color '{css}' (expected a #RRGGBB or #RGB hex value)"
            ))
        }
    };
    let opacity = opacity_pct.min(100);
    let a = ((opacity as f64 / 100.0) * 255.0).round() as u8;
    Ok(format!("{a:02x}{b:02x}{g:02x}{r:02x}"))
}

// ---------------------------------------------------------------------------
// GPX parsing (streaming quick-xml)
// ---------------------------------------------------------------------------

#[derive(Default, Clone)]
struct Pt {
    lat: f64,
    lon: f64,
    ele: Option<f64>,
    time: Option<String>,
}

#[derive(Default, Clone)]
struct Feature {
    name: Option<String>,
    desc: Option<String>,
    cmt: Option<String>,
}

impl Feature {
    /// Best available description text (desc, else cmt).
    fn description(&self) -> Option<&str> {
        self.desc.as_deref().or(self.cmt.as_deref())
    }
}

struct Track {
    meta: Feature,
    segs: Vec<Vec<Pt>>,
}

struct Route {
    meta: Feature,
    pts: Vec<Pt>,
}

struct Waypoint {
    meta: Feature,
    pt: Pt,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Ctx {
    Trk,
    Rte,
    TrkPt,
    RtePt,
    Wpt,
    Metadata,
}

fn decode_text(t: &BytesText) -> String {
    match t.decode() {
        Ok(s) => quick_xml::escape::unescape(&s)
            .map(|u| u.into_owned())
            .unwrap_or_else(|_| s.into_owned()),
        Err(_) => String::new(),
    }
}

fn local_name(name: QName) -> String {
    let full = name.as_ref();
    let local = match full.iter().position(|&b| b == b':') {
        Some(i) => &full[i + 1..],
        None => full,
    };
    String::from_utf8_lossy(local).into_owned()
}

fn get_attr(e: &BytesStart, decoder: Decoder, key: &str) -> Option<String> {
    for attr in e.attributes().flatten() {
        if local_name(QName(attr.key.as_ref())).eq_ignore_ascii_case(key) {
            #[allow(deprecated)]
            let val = attr.decode_and_unescape_value(decoder).ok()?;
            return Some(val.into_owned());
        }
    }
    None
}

fn read_pt(e: &BytesStart, decoder: Decoder) -> Result<Pt, String> {
    let lat =
        get_attr(e, decoder, "lat").ok_or("a <trkpt>/<rtept>/<wpt> is missing a lat attribute")?;
    let lon =
        get_attr(e, decoder, "lon").ok_or("a <trkpt>/<rtept>/<wpt> is missing a lon attribute")?;
    let lat: f64 = lat
        .trim()
        .parse()
        .map_err(|_| format!("invalid lat value '{lat}'"))?;
    let lon: f64 = lon
        .trim()
        .parse()
        .map_err(|_| format!("invalid lon value '{lon}'"))?;
    Ok(Pt {
        lat,
        lon,
        ele: None,
        time: None,
    })
}

fn set_feature_field(f: &mut Feature, field: &str, val: &str) {
    match field {
        "name" => f.name = Some(val.to_string()),
        "desc" => f.desc = Some(val.to_string()),
        "cmt" => f.cmt = Some(val.to_string()),
        _ => {}
    }
}

struct Parsed {
    doc_name: Option<String>,
    tracks: Vec<Track>,
    routes: Vec<Route>,
    waypoints: Vec<Waypoint>,
}

fn parse_gpx(xml: &str) -> Result<Parsed, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let decoder = reader.decoder();

    let mut doc_name: Option<String> = None;
    let mut tracks: Vec<Track> = Vec::new();
    let mut routes: Vec<Route> = Vec::new();
    let mut waypoints: Vec<Waypoint> = Vec::new();

    let mut cur_track_meta = Feature::default();
    let mut cur_track_segs: Vec<Vec<Pt>> = Vec::new();
    let mut cur_seg: Vec<Pt> = Vec::new();

    let mut cur_route_meta = Feature::default();
    let mut cur_route_pts: Vec<Pt> = Vec::new();

    let mut cur_pt = Pt::default();
    let mut cur_wpt_meta = Feature::default();
    let mut cur_wpt_pt = Pt::default();

    let mut stack: Vec<Ctx> = Vec::new();
    let mut in_extensions = false;
    let mut ext_depth: u32 = 0;
    let mut text_buf = String::new();
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
                let name = local_name(e.name());

                if in_extensions {
                    if !is_empty {
                        ext_depth += 1;
                    }
                    buf.clear();
                    continue;
                }

                match name.as_str() {
                    "trk" => {
                        cur_track_meta = Feature::default();
                        cur_track_segs = Vec::new();
                        stack.push(Ctx::Trk);
                    }
                    "trkseg" => cur_seg = Vec::new(),
                    "rte" => {
                        cur_route_meta = Feature::default();
                        cur_route_pts = Vec::new();
                        stack.push(Ctx::Rte);
                    }
                    "trkpt" => {
                        cur_pt = read_pt(&e, decoder)?;
                        if is_empty {
                            cur_seg.push(cur_pt.clone());
                        } else {
                            stack.push(Ctx::TrkPt);
                        }
                    }
                    "rtept" => {
                        cur_pt = read_pt(&e, decoder)?;
                        if is_empty {
                            cur_route_pts.push(cur_pt.clone());
                        } else {
                            stack.push(Ctx::RtePt);
                        }
                    }
                    "wpt" => {
                        cur_wpt_meta = Feature::default();
                        cur_wpt_pt = read_pt(&e, decoder)?;
                        if is_empty {
                            waypoints.push(Waypoint {
                                meta: cur_wpt_meta.clone(),
                                pt: cur_wpt_pt.clone(),
                            });
                        } else {
                            stack.push(Ctx::Wpt);
                        }
                    }
                    "metadata" => {
                        if !is_empty {
                            stack.push(Ctx::Metadata);
                        }
                    }
                    "extensions" => {
                        if !is_empty {
                            in_extensions = true;
                            ext_depth = 1;
                        }
                    }
                    _ => {}
                }
                text_buf.clear();
            }
            Ok(Event::Text(t)) => {
                if in_extensions {
                    buf.clear();
                    continue;
                }
                text_buf.push_str(&decode_text(&t));
            }
            Ok(Event::CData(t)) => {
                if in_extensions {
                    buf.clear();
                    continue;
                }
                text_buf.push_str(&String::from_utf8_lossy(&t.into_inner()));
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name());
                if in_extensions {
                    ext_depth = ext_depth.saturating_sub(1);
                    if ext_depth == 0 {
                        in_extensions = false;
                    }
                    buf.clear();
                    continue;
                }
                match name.as_str() {
                    "ele" => {
                        if let Ok(v) = text_buf.trim().parse::<f64>() {
                            match stack.last() {
                                Some(Ctx::TrkPt) | Some(Ctx::RtePt) => cur_pt.ele = Some(v),
                                Some(Ctx::Wpt) => cur_wpt_pt.ele = Some(v),
                                _ => {}
                            }
                        }
                    }
                    "time" => {
                        let s = text_buf.trim();
                        if !s.is_empty() {
                            match stack.last() {
                                Some(Ctx::TrkPt) | Some(Ctx::RtePt) => {
                                    cur_pt.time = Some(s.to_string())
                                }
                                Some(Ctx::Wpt) => cur_wpt_pt.time = Some(s.to_string()),
                                _ => {}
                            }
                        }
                    }
                    "name" | "desc" | "cmt" => {
                        let s = text_buf.trim();
                        if !s.is_empty() {
                            match stack.last() {
                                Some(Ctx::Trk) => set_feature_field(&mut cur_track_meta, &name, s),
                                Some(Ctx::Rte) => set_feature_field(&mut cur_route_meta, &name, s),
                                Some(Ctx::Wpt) => set_feature_field(&mut cur_wpt_meta, &name, s),
                                Some(Ctx::Metadata) => {
                                    if name == "name" && doc_name.is_none() {
                                        doc_name = Some(s.to_string());
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    "trkpt" => {
                        if let Some(Ctx::TrkPt) = stack.last() {
                            cur_seg.push(cur_pt.clone());
                            stack.pop();
                        }
                    }
                    "rtept" => {
                        if let Some(Ctx::RtePt) = stack.last() {
                            cur_route_pts.push(cur_pt.clone());
                            stack.pop();
                        }
                    }
                    "trkseg" => {
                        if !cur_seg.is_empty() {
                            cur_track_segs.push(std::mem::take(&mut cur_seg));
                        }
                    }
                    "trk" => {
                        if let Some(Ctx::Trk) = stack.last() {
                            if !cur_track_segs.is_empty() {
                                tracks.push(Track {
                                    meta: cur_track_meta.clone(),
                                    segs: std::mem::take(&mut cur_track_segs),
                                });
                            }
                            stack.pop();
                        }
                    }
                    "rte" => {
                        if let Some(Ctx::Rte) = stack.last() {
                            if !cur_route_pts.is_empty() {
                                routes.push(Route {
                                    meta: cur_route_meta.clone(),
                                    pts: std::mem::take(&mut cur_route_pts),
                                });
                            }
                            stack.pop();
                        }
                    }
                    "wpt" => {
                        if let Some(Ctx::Wpt) = stack.last() {
                            waypoints.push(Waypoint {
                                meta: cur_wpt_meta.clone(),
                                pt: cur_wpt_pt.clone(),
                            });
                            stack.pop();
                        }
                    }
                    "metadata" => {
                        if let Some(Ctx::Metadata) = stack.last() {
                            stack.pop();
                        }
                    }
                    _ => {}
                }
                text_buf.clear();
            }
            _ => {}
        }
        buf.clear();
    }

    Ok(Parsed {
        doc_name,
        tracks,
        routes,
        waypoints,
    })
}

// ---------------------------------------------------------------------------
// KML emission
// ---------------------------------------------------------------------------

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Format an `f64` compactly: whole numbers lose the trailing `.0`.
fn fmt_num(v: f64) -> String {
    let mut s = format!("{v}");
    if s == "-0" {
        s = "0".to_string();
    }
    s
}

fn coord_str(p: &Pt) -> String {
    match p.ele {
        Some(e) => format!("{},{},{}", fmt_num(p.lon), fmt_num(p.lat), fmt_num(e)),
        None => format!("{},{}", fmt_num(p.lon), fmt_num(p.lat)),
    }
}

fn coords_line(pts: &[Pt]) -> String {
    pts.iter().map(coord_str).collect::<Vec<_>>().join(" ")
}

/// First and last per-point timestamps across a set of segments, if both ends
/// carry a time — used to emit a track `<TimeSpan>`.
fn track_timespan(segs: &[Vec<Pt>]) -> Option<(String, String)> {
    let flat: Vec<&Pt> = segs.iter().flatten().collect();
    let begin = flat.iter().find_map(|p| p.time.clone());
    let end = flat.iter().rev().find_map(|p| p.time.clone());
    match (begin, end) {
        (Some(b), Some(e)) => Some((b, e)),
        _ => None,
    }
}

fn push_placemark_head(out: &mut String, meta: &Feature) {
    out.push_str("    <Placemark>\n");
    if let Some(n) = &meta.name {
        out.push_str(&format!("      <name>{}</name>\n", xml_escape(n)));
    }
    if let Some(d) = meta.description() {
        out.push_str(&format!(
            "      <description>{}</description>\n",
            xml_escape(d)
        ));
    }
}

fn push_linestring(out: &mut String, indent: &str, pts: &[Pt], mode: AltitudeMode) {
    out.push_str(&format!("{indent}<LineString>\n"));
    out.push_str(&format!("{indent}  <tessellate>1</tessellate>\n"));
    out.push_str(&format!(
        "{indent}  <altitudeMode>{}</altitudeMode>\n",
        mode.kml()
    ));
    out.push_str(&format!(
        "{indent}  <coordinates>{}</coordinates>\n",
        coords_line(pts)
    ));
    out.push_str(&format!("{indent}</LineString>\n"));
}

/// Emit a track/route as a Placemark. A multi-segment track becomes a
/// `<MultiGeometry>` of `<LineString>`s; a single segment is a bare
/// `<LineString>`.
fn push_line_feature(out: &mut String, meta: &Feature, segs: &[Vec<Pt>], mode: AltitudeMode) {
    let non_empty: Vec<&Vec<Pt>> = segs.iter().filter(|s| !s.is_empty()).collect();
    if non_empty.is_empty() {
        return;
    }
    push_placemark_head(out, meta);
    out.push_str("      <styleUrl>#lineStyle</styleUrl>\n");
    if let Some((begin, end)) = track_timespan(segs) {
        out.push_str("      <TimeSpan>\n");
        out.push_str(&format!("        <begin>{}</begin>\n", xml_escape(&begin)));
        out.push_str(&format!("        <end>{}</end>\n", xml_escape(&end)));
        out.push_str("      </TimeSpan>\n");
    }
    if non_empty.len() == 1 {
        push_linestring(out, "      ", non_empty[0], mode);
    } else {
        out.push_str("      <MultiGeometry>\n");
        for seg in &non_empty {
            push_linestring(out, "        ", seg, mode);
        }
        out.push_str("      </MultiGeometry>\n");
    }
    out.push_str("    </Placemark>\n");
}

fn push_waypoint(out: &mut String, wpt: &Waypoint, mode: AltitudeMode) {
    push_placemark_head(out, &wpt.meta);
    out.push_str("      <styleUrl>#waypointStyle</styleUrl>\n");
    if let Some(t) = &wpt.pt.time {
        out.push_str("      <TimeStamp>\n");
        out.push_str(&format!("        <when>{}</when>\n", xml_escape(t)));
        out.push_str("      </TimeStamp>\n");
    }
    out.push_str("      <Point>\n");
    out.push_str(&format!(
        "        <altitudeMode>{}</altitudeMode>\n",
        mode.kml()
    ));
    out.push_str(&format!(
        "        <coordinates>{}</coordinates>\n",
        coord_str(&wpt.pt)
    ));
    out.push_str("      </Point>\n");
    out.push_str("    </Placemark>\n");
}

/// Convert a GPX document into a KML 2.2 document per `opt`. See the module doc
/// for the exact element mapping.
pub fn convert(gpx: &str, opt: &Options) -> Result<String, String> {
    if gpx.trim().is_empty() {
        return Err("input is empty".to_string());
    }
    let line_kml = css_to_kml_color(&opt.line_color, opt.line_opacity)?;
    let wpt_kml = css_to_kml_color(&opt.waypoint_color, 100)?;

    let parsed = parse_gpx(gpx)?;
    if parsed.tracks.is_empty() && parsed.routes.is_empty() && parsed.waypoints.is_empty() {
        return Err(
            "no GPX track, route, or waypoint found (expected a <trk>, <rte>, or <wpt> element)"
                .to_string(),
        );
    }

    let doc_name = opt
        .document_name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| parsed.doc_name.clone())
        .unwrap_or_else(|| "GPS data".to_string());

    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str("<kml xmlns=\"http://www.opengis.net/kml/2.2\">\n");
    out.push_str("  <Document>\n");
    out.push_str(&format!("    <name>{}</name>\n", xml_escape(&doc_name)));
    out.push_str("    <Style id=\"lineStyle\">\n");
    out.push_str("      <LineStyle>\n");
    out.push_str(&format!("        <color>{line_kml}</color>\n"));
    out.push_str(&format!("        <width>{}</width>\n", opt.line_width));
    out.push_str("      </LineStyle>\n");
    out.push_str("    </Style>\n");
    out.push_str("    <Style id=\"waypointStyle\">\n");
    out.push_str("      <IconStyle>\n");
    out.push_str(&format!("        <color>{wpt_kml}</color>\n"));
    out.push_str("      </IconStyle>\n");
    out.push_str("    </Style>\n");

    for trk in &parsed.tracks {
        push_line_feature(&mut out, &trk.meta, &trk.segs, opt.altitude_mode);
    }
    for rte in &parsed.routes {
        push_line_feature(
            &mut out,
            &rte.meta,
            std::slice::from_ref(&rte.pts),
            opt.altitude_mode,
        );
    }
    for wpt in &parsed.waypoints {
        push_waypoint(&mut out, wpt, opt.altitude_mode);
    }

    out.push_str("  </Document>\n");
    out.push_str("</kml>\n");
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> Options {
        Options::default()
    }

    #[test]
    fn css_hex_and_opacity_to_kml_aabbggrr() {
        // #ef4444 at 80% → alpha round(0.8*255)=204=0xcc, then bb,gg,rr.
        assert_eq!(css_to_kml_color("#ef4444", 80).unwrap(), "cc4444ef");
        // Fully opaque blue waypoint icon.
        assert_eq!(css_to_kml_color("#3b82f6", 100).unwrap(), "fff6823b");
        // Transparent, and #RGB shorthand.
        assert_eq!(css_to_kml_color("#000000", 0).unwrap(), "00000000");
        assert_eq!(css_to_kml_color("f00", 100).unwrap(), "ff0000ff");
    }

    #[test]
    fn rejects_bad_color() {
        assert!(css_to_kml_color("not-a-color", 100).is_err());
    }

    #[test]
    fn happy_track_with_elevation_and_timestamps() {
        let gpx =
            "<?xml version=\"1.0\"?>\n<gpx version=\"1.1\"><metadata><name>Trip</name></metadata>\
<trk><name>Morning Run</name><trkseg>\
<trkpt lat=\"52.100\" lon=\"5.100\"><ele>10</ele><time>2026-07-01T08:00:00Z</time></trkpt>\
<trkpt lat=\"52.101\" lon=\"5.102\"><ele>12</ele><time>2026-07-01T08:05:00Z</time></trkpt>\
</trkseg></trk>\
<wpt lat=\"52.2\" lon=\"5.3\"><name>Camp</name><desc>Rest stop</desc><ele>100</ele></wpt>\
</gpx>";
        let kml = convert(gpx, &opts()).unwrap();
        assert!(kml.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n"));
        assert!(kml.contains("<kml xmlns=\"http://www.opengis.net/kml/2.2\">"));
        assert!(kml.contains("    <name>Trip</name>"));
        assert!(kml.contains("        <color>cc4444ef</color>"));
        assert!(kml.contains("      <name>Morning Run</name>"));
        assert!(kml.contains("        <begin>2026-07-01T08:00:00Z</begin>"));
        assert!(kml.contains("        <end>2026-07-01T08:05:00Z</end>"));
        assert!(kml.contains("        <coordinates>5.1,52.1,10 5.102,52.101,12</coordinates>"));
        assert!(kml.contains("      <description>Rest stop</description>"));
        assert!(kml.contains("        <coordinates>5.3,52.2,100</coordinates>"));
    }

    #[test]
    fn multi_segment_track_becomes_multigeometry() {
        let gpx = "<gpx><trk><name>Loop</name>\
<trkseg><trkpt lat=\"1\" lon=\"1\"/><trkpt lat=\"2\" lon=\"2\"/></trkseg>\
<trkseg><trkpt lat=\"3\" lon=\"3\"/><trkpt lat=\"4\" lon=\"4\"/></trkseg>\
</trk></gpx>";
        let kml = convert(gpx, &opts()).unwrap();
        assert!(kml.contains("<MultiGeometry>"));
        assert_eq!(kml.matches("<LineString>").count(), 2);
        assert!(kml.contains("<coordinates>1,1 2,2</coordinates>"));
        assert!(kml.contains("<coordinates>3,3 4,4</coordinates>"));
    }

    #[test]
    fn route_becomes_linestring_and_absolute_mode() {
        let gpx = "<gpx><rte><name>Route A</name><rtept lat=\"1\" lon=\"1\"/><rtept lat=\"2\" lon=\"2\"/></rte></gpx>";
        let opt = Options {
            altitude_mode: AltitudeMode::Absolute,
            ..Options::default()
        };
        let kml = convert(gpx, &opt).unwrap();
        assert!(kml.contains("<name>Route A</name>"));
        assert!(kml.contains("<altitudeMode>absolute</altitudeMode>"));
        assert!(kml.contains("<coordinates>1,1 2,2</coordinates>"));
    }

    #[test]
    fn document_name_override_wins_over_metadata() {
        let gpx = "<gpx><metadata><name>Ignored</name></metadata><wpt lat=\"1\" lon=\"1\"/></gpx>";
        let opt = Options {
            document_name: Some("My Map".to_string()),
            ..Options::default()
        };
        let kml = convert(gpx, &opt).unwrap();
        assert!(kml.contains("<name>My Map</name>"));
        assert!(!kml.contains("Ignored"));
    }

    #[test]
    fn error_on_no_features() {
        let gpx = "<gpx version=\"1.1\"></gpx>";
        let err = convert(gpx, &opts()).unwrap_err();
        assert!(err.contains("no GPX track, route, or waypoint"));
    }

    #[test]
    fn error_on_empty_input() {
        assert!(convert("   ", &opts()).unwrap_err().contains("empty"));
    }
}
