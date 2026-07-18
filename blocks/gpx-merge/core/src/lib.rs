//! gpx-merge core — merge multiple GPX files/tracks into one GPX 1.1 document,
//! ordered chronologically by timestamp. Pure-Rust (`quick-xml`), no
//! wafer/wasm-bindgen deps; shared by the chat skill block and the web page.
//!
//! The whole input is parsed in a single streaming pass, so several GPX
//! documents pasted one after another (each with its own `<?xml ...?>` and
//! `<gpx>` root) are read together — every `<trk>`/`<trkseg>`/`<trkpt>`,
//! `<rte>`/`<rtept>`, and `<wpt>` is collected regardless of which document it
//! came from. Routes (`<rte>`) are normalized to a single-segment track.
//! Coordinate strings, elevation, and timestamps are preserved verbatim (never
//! reformatted) so precision survives a merge.
//!
//! Timestamps are compared lexically, which matches ISO-8601 / RFC-3339 UTC
//! (`...Z`) ordering used by essentially every GPS device. Points without a
//! `<time>` keep their input order and sort after timed points.

use quick_xml::encoding::Decoder;
use quick_xml::events::{BytesStart, BytesText, Event};
use quick_xml::name::QName;
use quick_xml::reader::Reader;

/// How the collected tracks/segments/points are laid out in the merged file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeMode {
    /// One `<trk>` with one `<trkseg>` — every point from every source combined
    /// into a single continuous track.
    SingleTrack,
    /// One `<trk>` whose `<trkseg>`s are the original segments kept intact
    /// (preserves the pauses between activities / multi-day stages).
    SingleTrackMultiSegment,
    /// Each source track stays its own `<trk>` in one file (its name preserved).
    SeparateTracks,
}

impl MergeMode {
    pub fn parse(s: &str) -> Result<MergeMode, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "single-track" | "" => Ok(MergeMode::SingleTrack),
            "single-track-multi-segment" => Ok(MergeMode::SingleTrackMultiSegment),
            "separate-tracks" => Ok(MergeMode::SeparateTracks),
            other => Err(format!(
                "unknown merge_mode '{other}' (use single-track, \
                 single-track-multi-segment, or separate-tracks)"
            )),
        }
    }
}

pub struct Options {
    pub merge_mode: MergeMode,
    /// Order points (single-track), segments (multi-segment), or tracks
    /// (separate-tracks) chronologically by their timestamp. When false, the
    /// input order is preserved.
    pub sort_by_time: bool,
    /// Drop consecutive duplicate points (identical lat, lon, and time) within
    /// each segment — useful when merged files overlap at their join.
    pub dedupe: bool,
    /// Carry `<wpt>` waypoints from every source into the merged file.
    pub include_waypoints: bool,
    /// Name for the merged track (single-track modes) and the fallback name for
    /// any unnamed source track (separate-tracks mode).
    pub track_name: String,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            merge_mode: MergeMode::SingleTrack,
            sort_by_time: true,
            dedupe: false,
            include_waypoints: true,
            track_name: "Merged track".to_string(),
        }
    }
}

#[derive(Default, Clone)]
struct Point {
    lat: String,
    lon: String,
    ele: Option<String>,
    time: Option<String>,
}

#[derive(Default, Clone)]
struct Waypoint {
    pt: Point,
    name: Option<String>,
    desc: Option<String>,
    sym: Option<String>,
}

#[derive(Default, Clone)]
struct Track {
    name: Option<String>,
    segs: Vec<Vec<Point>>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Ctx {
    Trk,
    Rte,
    TrkPt,
    RtePt,
    Wpt,
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

fn read_pt(e: &BytesStart, decoder: Decoder) -> Result<Point, String> {
    let lat = get_attr(e, decoder, "lat")
        .ok_or("a <trkpt>/<rtept>/<wpt> is missing its lat attribute")?;
    let lon = get_attr(e, decoder, "lon")
        .ok_or("a <trkpt>/<rtept>/<wpt> is missing its lon attribute")?;
    // Validate the coordinates are numeric, but keep the original strings so
    // precision is preserved exactly in the output.
    lat.trim()
        .parse::<f64>()
        .map_err(|_| format!("invalid lat value '{lat}'"))?;
    lon.trim()
        .parse::<f64>()
        .map_err(|_| format!("invalid lon value '{lon}'"))?;
    Ok(Point {
        lat: lat.trim().to_string(),
        lon: lon.trim().to_string(),
        ele: None,
        time: None,
    })
}

/// Parse every track, route, and waypoint out of the (possibly multi-document)
/// GPX text in one streaming pass.
fn parse(xml: &str) -> Result<(Vec<Track>, Vec<Waypoint>), String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let decoder = reader.decoder();

    let mut tracks: Vec<Track> = Vec::new();
    let mut waypoints: Vec<Waypoint> = Vec::new();

    let mut cur_track = Track::default();
    let mut cur_seg: Vec<Point> = Vec::new();
    let mut cur_route = Track::default();
    let mut cur_route_seg: Vec<Point> = Vec::new();
    let mut cur_pt = Point::default();
    let mut cur_wpt = Waypoint::default();

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
                        cur_track = Track::default();
                        stack.push(Ctx::Trk);
                    }
                    "trkseg" => cur_seg = Vec::new(),
                    "rte" => {
                        cur_route = Track::default();
                        cur_route_seg = Vec::new();
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
                            cur_route_seg.push(cur_pt.clone());
                        } else {
                            stack.push(Ctx::RtePt);
                        }
                    }
                    "wpt" => {
                        cur_wpt = Waypoint {
                            pt: read_pt(&e, decoder)?,
                            ..Default::default()
                        };
                        if is_empty {
                            waypoints.push(cur_wpt.clone());
                        } else {
                            stack.push(Ctx::Wpt);
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
                        let s = text_buf.trim();
                        if !s.is_empty() {
                            match stack.last() {
                                Some(Ctx::TrkPt) | Some(Ctx::RtePt) => {
                                    cur_pt.ele = Some(s.to_string())
                                }
                                Some(Ctx::Wpt) => cur_wpt.pt.ele = Some(s.to_string()),
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
                                Some(Ctx::Wpt) => cur_wpt.pt.time = Some(s.to_string()),
                                _ => {}
                            }
                        }
                    }
                    "name" => {
                        let s = text_buf.trim();
                        if !s.is_empty() {
                            match stack.last() {
                                Some(Ctx::Trk) => cur_track.name = Some(s.to_string()),
                                Some(Ctx::Rte) => cur_route.name = Some(s.to_string()),
                                Some(Ctx::Wpt) => cur_wpt.name = Some(s.to_string()),
                                _ => {}
                            }
                        }
                    }
                    "desc" => {
                        let s = text_buf.trim();
                        if !s.is_empty() {
                            if let Some(Ctx::Wpt) = stack.last() {
                                cur_wpt.desc = Some(s.to_string());
                            }
                        }
                    }
                    "sym" => {
                        let s = text_buf.trim();
                        if !s.is_empty() {
                            if let Some(Ctx::Wpt) = stack.last() {
                                cur_wpt.sym = Some(s.to_string());
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
                            cur_route_seg.push(cur_pt.clone());
                            stack.pop();
                        }
                    }
                    "trkseg" => {
                        if !cur_seg.is_empty() {
                            cur_track.segs.push(std::mem::take(&mut cur_seg));
                        }
                    }
                    "trk" => {
                        if let Some(Ctx::Trk) = stack.last() {
                            if !cur_track.segs.is_empty() {
                                tracks.push(std::mem::take(&mut cur_track));
                            }
                            stack.pop();
                        }
                    }
                    "rte" => {
                        if let Some(Ctx::Rte) = stack.last() {
                            if !cur_route_seg.is_empty() {
                                cur_route.segs.push(std::mem::take(&mut cur_route_seg));
                                tracks.push(std::mem::take(&mut cur_route));
                            }
                            stack.pop();
                        }
                    }
                    "wpt" => {
                        if let Some(Ctx::Wpt) = stack.last() {
                            waypoints.push(cur_wpt.clone());
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

    Ok((tracks, waypoints))
}

fn earliest_time(pts: &[Point]) -> Option<&str> {
    pts.iter().filter_map(|p| p.time.as_deref()).min()
}

/// Sort key that keeps timed items in chronological (lexical) order and pushes
/// untimed items to the end while preserving their relative input order (via a
/// stable sort).
fn time_key(t: Option<&str>) -> (bool, String) {
    match t {
        Some(s) => (false, s.to_string()),
        None => (true, String::new()),
    }
}

fn dedupe_seg(seg: &mut Vec<Point>) {
    seg.dedup_by(|a, b| a.lat == b.lat && a.lon == b.lon && a.time == b.time);
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn emit_pt(tag: &str, p: &Point, indent: &str, out: &mut String) {
    out.push_str(&format!(
        "{indent}<{tag} lat=\"{}\" lon=\"{}\">\n",
        xml_escape(&p.lat),
        xml_escape(&p.lon)
    ));
    if let Some(e) = &p.ele {
        out.push_str(&format!("{indent}  <ele>{}</ele>\n", xml_escape(e)));
    }
    if let Some(t) = &p.time {
        out.push_str(&format!("{indent}  <time>{}</time>\n", xml_escape(t)));
    }
    out.push_str(&format!("{indent}</{tag}>\n"));
}

fn emit_seg(seg: &[Point], out: &mut String) {
    out.push_str("    <trkseg>\n");
    for p in seg {
        emit_pt("trkpt", p, "      ", out);
    }
    out.push_str("    </trkseg>\n");
}

/// Merge the GPX documents in `input` per `opt`, returning one GPX 1.1 document.
pub fn merge(input: &str, opt: &Options) -> Result<String, String> {
    if input.trim().is_empty() {
        return Err("input is empty — paste one or more GPX documents".to_string());
    }

    let (mut tracks, mut waypoints) = parse(input)?;

    if tracks.is_empty() && (waypoints.is_empty() || !opt.include_waypoints) {
        return Err(
            "no GPX tracks, routes, or waypoints found (expected at least one \
             <trk>, <rte>, or <wpt> element)"
                .to_string(),
        );
    }

    // Dedupe first (per segment) so ordering operates on clean segments.
    if opt.dedupe {
        for t in &mut tracks {
            for seg in &mut t.segs {
                dedupe_seg(seg);
            }
            t.segs.retain(|s| !s.is_empty());
        }
        tracks.retain(|t| !t.segs.is_empty());
    }

    let mut body = String::new();

    // Waypoints (before tracks, GPX-schema order).
    if opt.include_waypoints && !waypoints.is_empty() {
        if opt.sort_by_time {
            waypoints.sort_by_key(|w| time_key(w.pt.time.as_deref()));
        }
        for w in &waypoints {
            body.push_str(&format!(
                "  <wpt lat=\"{}\" lon=\"{}\">\n",
                xml_escape(&w.pt.lat),
                xml_escape(&w.pt.lon)
            ));
            if let Some(e) = &w.pt.ele {
                body.push_str(&format!("    <ele>{}</ele>\n", xml_escape(e)));
            }
            if let Some(t) = &w.pt.time {
                body.push_str(&format!("    <time>{}</time>\n", xml_escape(t)));
            }
            if let Some(n) = &w.name {
                body.push_str(&format!("    <name>{}</name>\n", xml_escape(n)));
            }
            if let Some(d) = &w.desc {
                body.push_str(&format!("    <desc>{}</desc>\n", xml_escape(d)));
            }
            if let Some(s) = &w.sym {
                body.push_str(&format!("    <sym>{}</sym>\n", xml_escape(s)));
            }
            body.push_str("  </wpt>\n");
        }
    }

    match opt.merge_mode {
        MergeMode::SingleTrack => {
            let mut pts: Vec<Point> = tracks
                .into_iter()
                .flat_map(|t| t.segs.into_iter().flatten())
                .collect();
            if opt.sort_by_time {
                pts.sort_by_key(|p| time_key(p.time.as_deref()));
            }
            if opt.dedupe {
                dedupe_seg(&mut pts);
            }
            if !pts.is_empty() {
                body.push_str("  <trk>\n");
                body.push_str(&format!("    <name>{}</name>\n", xml_escape(&opt.track_name)));
                emit_seg(&pts, &mut body);
                body.push_str("  </trk>\n");
            }
        }
        MergeMode::SingleTrackMultiSegment => {
            let mut segs: Vec<Vec<Point>> =
                tracks.into_iter().flat_map(|t| t.segs.into_iter()).collect();
            if opt.sort_by_time {
                segs.sort_by_key(|s| time_key(earliest_time(s)));
            }
            let segs: Vec<Vec<Point>> = segs.into_iter().filter(|s| !s.is_empty()).collect();
            if !segs.is_empty() {
                body.push_str("  <trk>\n");
                body.push_str(&format!("    <name>{}</name>\n", xml_escape(&opt.track_name)));
                for seg in &segs {
                    emit_seg(seg, &mut body);
                }
                body.push_str("  </trk>\n");
            }
        }
        MergeMode::SeparateTracks => {
            if opt.sort_by_time {
                tracks.sort_by_key(|t| {
                    let e = t.segs.iter().flatten().filter_map(|p| p.time.as_deref()).min();
                    time_key(e)
                });
            }
            for t in &tracks {
                let name = t.name.clone().unwrap_or_else(|| opt.track_name.clone());
                body.push_str("  <trk>\n");
                body.push_str(&format!("    <name>{}</name>\n", xml_escape(&name)));
                for seg in &t.segs {
                    emit_seg(seg, &mut body);
                }
                body.push_str("  </trk>\n");
            }
        }
    }

    if body.is_empty() {
        return Err("nothing to merge — no track points found after parsing".to_string());
    }

    Ok(format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <gpx version=\"1.1\" creator=\"gizza-ai/gpx-merge\" \
         xmlns=\"http://www.topografix.com/GPX/1/1\">\n\
         {body}</gpx>\n"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = r#"<?xml version="1.0"?>
<gpx version="1.1"><trk><name>Morning</name><trkseg>
<trkpt lat="1.0" lon="2.0"><ele>10</ele><time>2024-01-01T08:00:00Z</time></trkpt>
<trkpt lat="1.1" lon="2.1"><time>2024-01-01T08:05:00Z</time></trkpt>
</trkseg></trk></gpx>"#;

    const B: &str = r#"<?xml version="1.0"?>
<gpx version="1.1"><trk><name>Evening</name><trkseg>
<trkpt lat="3.0" lon="4.0"><time>2024-01-01T07:00:00Z</time></trkpt>
<trkpt lat="3.1" lon="4.1"><time>2024-01-01T09:00:00Z</time></trkpt>
</trkseg></trk></gpx>"#;

    #[test]
    fn single_track_sorted_by_time() {
        let input = format!("{A}\n{B}");
        let out = merge(&input, &Options::default()).unwrap();
        // 07:00 (B) then 08:00 (A) then 08:05 (A) then 09:00 (B).
        let order: Vec<&str> = out
            .lines()
            .filter(|l| l.contains("<trkpt"))
            .map(|l| l.trim())
            .collect();
        assert_eq!(order.len(), 4);
        assert!(order[0].contains("lat=\"3.0\""), "07:00 point should be first");
        assert!(order[1].contains("lat=\"1.0\""), "08:00 point second");
        assert!(order[2].contains("lat=\"1.1\""), "08:05 point third");
        assert!(order[3].contains("lat=\"3.1\""), "09:00 point last");
        // One combined track/segment.
        assert_eq!(out.matches("<trk>").count(), 1);
        assert_eq!(out.matches("<trkseg>").count(), 1);
        assert!(out.contains("<name>Merged track</name>"));
        // Elevation preserved verbatim.
        assert!(out.contains("<ele>10</ele>"));
    }

    #[test]
    fn separate_tracks_orders_tracks_and_keeps_names() {
        let input = format!("{A}\n{B}");
        let opt = Options {
            merge_mode: MergeMode::SeparateTracks,
            ..Default::default()
        };
        let out = merge(&input, &opt).unwrap();
        assert_eq!(out.matches("<trk>").count(), 2);
        // Evening (earliest point 07:00) sorts before Morning (08:00).
        let evening = out.find("Evening").unwrap();
        let morning = out.find("Morning").unwrap();
        assert!(evening < morning, "earlier track should come first");
    }

    #[test]
    fn multi_segment_keeps_segment_breaks() {
        let input = format!("{A}\n{B}");
        let opt = Options {
            merge_mode: MergeMode::SingleTrackMultiSegment,
            ..Default::default()
        };
        let out = merge(&input, &opt).unwrap();
        assert_eq!(out.matches("<trk>").count(), 1);
        assert_eq!(out.matches("<trkseg>").count(), 2);
    }

    #[test]
    fn dedupe_drops_consecutive_duplicate_join_point() {
        // B starts at the same point A ends on.
        let a = r#"<gpx><trk><trkseg>
<trkpt lat="1.0" lon="2.0"><time>2024-01-01T08:00:00Z</time></trkpt>
<trkpt lat="1.5" lon="2.5"><time>2024-01-01T08:05:00Z</time></trkpt>
</trkseg></trk></gpx>"#;
        let b = r#"<gpx><trk><trkseg>
<trkpt lat="1.5" lon="2.5"><time>2024-01-01T08:05:00Z</time></trkpt>
<trkpt lat="2.0" lon="3.0"><time>2024-01-01T08:10:00Z</time></trkpt>
</trkseg></trk></gpx>"#;
        let input = format!("{a}\n{b}");
        let opt = Options {
            dedupe: true,
            ..Default::default()
        };
        let out = merge(&input, &opt).unwrap();
        assert_eq!(out.matches("<trkpt").count(), 3, "duplicate join point removed");
    }

    #[test]
    fn waypoints_carried_and_toggleable() {
        let w = r#"<gpx><wpt lat="5.0" lon="6.0"><name>Camp</name></wpt>
<trk><trkseg><trkpt lat="1.0" lon="2.0"/></trkseg></trk></gpx>"#;
        let with = merge(w, &Options::default()).unwrap();
        assert!(with.contains("<wpt lat=\"5.0\" lon=\"6.0\">"));
        assert!(with.contains("<name>Camp</name>"));
        let opt = Options {
            include_waypoints: false,
            ..Default::default()
        };
        let without = merge(w, &opt).unwrap();
        assert!(!without.contains("<wpt"));
    }

    #[test]
    fn preserves_input_order_when_sort_disabled() {
        let input = format!("{A}\n{B}");
        let opt = Options {
            sort_by_time: false,
            ..Default::default()
        };
        let out = merge(&input, &opt).unwrap();
        let first = out.lines().find(|l| l.contains("<trkpt")).unwrap();
        assert!(first.contains("lat=\"1.0\""), "A's first point stays first");
    }

    #[test]
    fn routes_are_merged_as_tracks() {
        let r = r#"<gpx><rte><name>Planned</name>
<rtept lat="1.0" lon="2.0"/><rtept lat="1.1" lon="2.1"/></rte></gpx>"#;
        let out = merge(r, &Options::default()).unwrap();
        assert_eq!(out.matches("<trkpt").count(), 2);
    }

    #[test]
    fn empty_input_errors() {
        let err = merge("   ", &Options::default()).unwrap_err();
        assert!(err.contains("empty"));
    }

    #[test]
    fn no_geo_data_errors() {
        let err = merge("<gpx></gpx>", &Options::default()).unwrap_err();
        assert!(err.contains("no GPX tracks"));
    }

    #[test]
    fn invalid_coord_errors() {
        let err = merge(
            r#"<gpx><trk><trkseg><trkpt lat="oops" lon="2.0"/></trkseg></trk></gpx>"#,
            &Options::default(),
        )
        .unwrap_err();
        assert!(err.contains("invalid lat"));
    }

    #[test]
    fn merge_mode_parse() {
        assert_eq!(MergeMode::parse("single-track").unwrap(), MergeMode::SingleTrack);
        assert_eq!(
            MergeMode::parse("separate-tracks").unwrap(),
            MergeMode::SeparateTracks
        );
        assert!(MergeMode::parse("bogus").is_err());
    }
}
