//! tcx-to-gpx core — convert Garmin Training Center XML (TCX) workout files into
//! GPX 1.1, preserving trackpoints (latitude/longitude/elevation/time) and basic
//! metadata. Pure-Rust (`quick-xml`), no wafer/wasm-bindgen deps; shared by the
//! chat skill block and the web page.
//!
//! TCX layout: `Activities/Activity/Lap/Track/Trackpoint` (and, for routes,
//! `Courses/Course/Track/Trackpoint`). Each `<Trackpoint>` may hold `<Position>`
//! (`<LatitudeDegrees>`/`<LongitudeDegrees>`), `<AltitudeMeters>`, `<Time>`,
//! `<HeartRateBpm><Value>`, and `<Cadence>`.
//!
//! Mapping to GPX 1.1:
//! - each `<Activity>` (or `<Course>`) becomes one `<trk>`;
//! - each TCX `<Track>` becomes one `<trkseg>` (so a multi-lap activity keeps its
//!   segment structure);
//! - each `<Trackpoint>` with a position becomes one `<trkpt lat lon>` carrying
//!   `<ele>` (from `AltitudeMeters`) and `<time>` (from `Time`) when present;
//! - the `Sport` attribute becomes the track's `<type>`, a `<Course>`'s `<Name>`
//!   (or an `<Activity>`'s `<Id>`) becomes the track's `<name>`, and the earliest
//!   trackpoint time becomes `<metadata><time>`;
//! - with `include_extensions` on (the default), heart rate and cadence are
//!   emitted as Garmin `TrackPointExtension` elements
//!   (`<gpxtpx:hr>`/`<gpxtpx:cad>`), which is where GPX consumers expect them —
//!   GPX 1.1 has no core element for either. Trackpoints without a position are
//!   skipped (a GPX `<trkpt>` requires lat/lon).

use quick_xml::events::{BytesStart, BytesText, Event};
use quick_xml::name::QName;
use quick_xml::reader::Reader;

/// GPX namespaces used in the output header.
const GPX_NS: &str = "http://www.topografix.com/GPX/1/1";
const GPXTPX_NS: &str = "http://www.garmin.com/xmlschemas/TrackPointExtension/v1";
const XSI_NS: &str = "http://www.w3.org/2001/XMLSchema-instance";
const SCHEMA_LOCATION: &str =
    "http://www.topografix.com/GPX/1/1 http://www.topografix.com/GPX/1/1/gpx.xsd";

pub struct Options {
    /// Emit heart rate and cadence as Garmin `TrackPointExtension` elements
    /// (`<gpxtpx:hr>`/`<gpxtpx:cad>`). GPX 1.1 has no core element for either, so
    /// turning this off drops them entirely. Default true.
    pub include_extensions: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options { include_extensions: true }
    }
}

/// Decode an XML text/CDATA node's bytes to UTF-8 and unescape entities. Falls
/// back to the raw decoded text if unescaping fails, and to an empty string if
/// decoding itself fails (invalid UTF-8).
fn decode_text(t: &BytesText) -> String {
    match t.decode() {
        Ok(s) => quick_xml::escape::unescape(&s)
            .map(|u| u.into_owned())
            .unwrap_or_else(|_| s.into_owned()),
        Err(_) => String::new(),
    }
}

/// Strip a `ns:local` prefix, returning the local name.
fn local_name(name: QName) -> String {
    let full = name.as_ref();
    let local = match full.iter().position(|&b| b == b':') {
        Some(i) => &full[i + 1..],
        None => full,
    };
    String::from_utf8_lossy(local).into_owned()
}

fn get_attr(e: &BytesStart, key: &str) -> Option<String> {
    for attr in e.attributes().flatten() {
        if local_name(QName(attr.key.as_ref())).eq_ignore_ascii_case(key) {
            return attr
                .unescape_value()
                .ok()
                .map(|v| v.into_owned())
                .or_else(|| Some(String::from_utf8_lossy(&attr.value).into_owned()));
        }
    }
    None
}

#[derive(Default, Clone)]
struct TrkPt {
    lat: Option<f64>,
    lon: Option<f64>,
    ele: Option<f64>,
    time: Option<String>,
    hr: Option<u32>,
    cad: Option<u32>,
}

#[derive(Default)]
struct Trk {
    name: Option<String>,
    kind: Option<String>,
    segs: Vec<Vec<TrkPt>>,
}

/// Which container element we are currently inside — used to route text nodes
/// (an `<Id>` under an `<Activity>` is a track name; a `<Time>` under a
/// `<Trackpoint>` is a point timestamp).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Ctx {
    Activity,
    Course,
    Lap,
    Track,
    Trackpoint,
    Position,
    HeartRate,
}

/// Convert a TCX document into a GPX 1.1 document per `opt`.
pub fn convert(input: &str, opt: &Options) -> Result<String, String> {
    let trimmed = input.trim_start();
    if trimmed.is_empty() {
        return Err("input is empty".to_string());
    }
    if !trimmed.starts_with('<') {
        return Err(
            "input does not look like TCX XML (expected a <TrainingCenterDatabase> document \
             beginning with '<'). Paste the contents of a Garmin .tcx file."
                .to_string(),
        );
    }

    let tracks = parse_tcx(input)?;

    if tracks.is_empty() {
        return Err(
            "no TCX activity or course with trackpoints was found (expected \
             <Activities>/<Activity>/<Lap>/<Track>/<Trackpoint> or <Courses>/<Course>/<Track>). \
             Trackpoints without a <Position> are skipped, so a file whose points all lack \
             latitude/longitude produces no track."
                .to_string(),
        );
    }

    Ok(build_gpx(&tracks, opt))
}

fn parse_tcx(xml: &str) -> Result<Vec<Trk>, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut tracks: Vec<Trk> = Vec::new();
    let mut cur_trk = Trk::default();
    let mut cur_seg: Vec<TrkPt> = Vec::new();
    let mut cur_pt = TrkPt::default();

    let mut stack: Vec<Ctx> = Vec::new();
    let mut text_buf = String::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Err(e) => return Err(format!("malformed TCX/XML: {e}")),
            Ok(Event::Eof) => break,
            Ok(ev @ (Event::Start(_) | Event::Empty(_))) => {
                let is_empty = matches!(ev, Event::Empty(_));
                let e = match ev {
                    Event::Start(e) | Event::Empty(e) => e,
                    _ => unreachable!(),
                };
                let name = local_name(e.name());
                match name.as_str() {
                    "Activity" => {
                        cur_trk = Trk { kind: get_attr(&e, "Sport"), ..Default::default() };
                        if !is_empty {
                            stack.push(Ctx::Activity);
                        }
                    }
                    "Course" => {
                        cur_trk = Trk::default();
                        if !is_empty {
                            stack.push(Ctx::Course);
                        }
                    }
                    "Lap" if !is_empty => stack.push(Ctx::Lap),
                    "Track" => {
                        cur_seg = Vec::new();
                        if !is_empty {
                            stack.push(Ctx::Track);
                        }
                    }
                    "Trackpoint" => {
                        cur_pt = TrkPt::default();
                        if !is_empty {
                            stack.push(Ctx::Trackpoint);
                        }
                    }
                    "Position" if !is_empty => stack.push(Ctx::Position),
                    "HeartRateBpm" if !is_empty => stack.push(Ctx::HeartRate),
                    _ => {}
                }
                text_buf.clear();
            }
            Ok(Event::Text(t)) => text_buf.push_str(&decode_text(&t)),
            Ok(Event::CData(t)) => text_buf.push_str(&String::from_utf8_lossy(&t.into_inner())),
            Ok(Event::End(e)) => {
                let name = local_name(e.name());
                let s = text_buf.trim();
                match name.as_str() {
                    "LatitudeDegrees" if stack.last() == Some(&Ctx::Position) => {
                        cur_pt.lat = s.parse::<f64>().ok();
                    }
                    "LongitudeDegrees" if stack.last() == Some(&Ctx::Position) => {
                        cur_pt.lon = s.parse::<f64>().ok();
                    }
                    "AltitudeMeters" if stack.last() == Some(&Ctx::Trackpoint) => {
                        cur_pt.ele = s.parse::<f64>().ok();
                    }
                    "Time" if stack.last() == Some(&Ctx::Trackpoint) => {
                        if !s.is_empty() {
                            cur_pt.time = Some(s.to_string());
                        }
                    }
                    "Value" if stack.last() == Some(&Ctx::HeartRate) => {
                        cur_pt.hr = s.parse::<f64>().ok().map(|v| v.round() as u32);
                    }
                    "Cadence" if stack.last() == Some(&Ctx::Trackpoint) => {
                        cur_pt.cad = s.parse::<f64>().ok().map(|v| v.round() as u32);
                    }
                    "Id" if stack.last() == Some(&Ctx::Activity) => {
                        if cur_trk.name.is_none() && !s.is_empty() {
                            cur_trk.name = Some(s.to_string());
                        }
                    }
                    "Name" if stack.last() == Some(&Ctx::Course) => {
                        if !s.is_empty() {
                            cur_trk.name = Some(s.to_string());
                        }
                    }
                    "Position" if stack.last() == Some(&Ctx::Position) => {
                        stack.pop();
                    }
                    "HeartRateBpm" if stack.last() == Some(&Ctx::HeartRate) => {
                        stack.pop();
                    }
                    "Trackpoint" if stack.last() == Some(&Ctx::Trackpoint) => {
                        stack.pop();
                        if cur_pt.lat.is_some() && cur_pt.lon.is_some() {
                            cur_seg.push(std::mem::take(&mut cur_pt));
                        }
                    }
                    "Track" if stack.last() == Some(&Ctx::Track) => {
                        stack.pop();
                        if !cur_seg.is_empty() {
                            cur_trk.segs.push(std::mem::take(&mut cur_seg));
                        }
                    }
                    "Lap" if stack.last() == Some(&Ctx::Lap) => {
                        stack.pop();
                    }
                    "Activity" if stack.last() == Some(&Ctx::Activity) => {
                        stack.pop();
                        if !cur_trk.segs.is_empty() {
                            tracks.push(std::mem::take(&mut cur_trk));
                        }
                    }
                    "Course" if stack.last() == Some(&Ctx::Course) => {
                        stack.pop();
                        if !cur_trk.segs.is_empty() {
                            tracks.push(std::mem::take(&mut cur_trk));
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

    Ok(tracks)
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Format a coordinate/elevation `f64` without a trailing `.0` for whole values
/// but with full precision otherwise (e.g. `52.1`, `10`, `-122.084`).
fn fmt_num(v: f64) -> String {
    format!("{v}")
}

fn build_gpx(tracks: &[Trk], opt: &Options) -> String {
    // A trackpoint extension is only emitted when the option is on AND some point
    // actually carries heart rate or cadence; the gpxtpx namespace is declared
    // only then, keeping the output clean.
    let has_ext = opt.include_extensions
        && tracks
            .iter()
            .flat_map(|t| t.segs.iter())
            .flatten()
            .any(|p| p.hr.is_some() || p.cad.is_some());

    let earliest = tracks
        .iter()
        .flat_map(|t| t.segs.iter())
        .flatten()
        .filter_map(|p| p.time.clone())
        .min();

    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str("<gpx version=\"1.1\" creator=\"gizza-ai/tcx-to-gpx\"\n");
    out.push_str(&format!("     xmlns=\"{GPX_NS}\"\n"));
    if has_ext {
        out.push_str(&format!("     xmlns:gpxtpx=\"{GPXTPX_NS}\"\n"));
    }
    out.push_str(&format!("     xmlns:xsi=\"{XSI_NS}\"\n"));
    out.push_str(&format!("     xsi:schemaLocation=\"{SCHEMA_LOCATION}\">\n"));

    if let Some(t) = &earliest {
        out.push_str("  <metadata>\n");
        out.push_str(&format!("    <time>{}</time>\n", xml_escape(t)));
        out.push_str("  </metadata>\n");
    }

    for trk in tracks {
        out.push_str("  <trk>\n");
        if let Some(n) = &trk.name {
            out.push_str(&format!("    <name>{}</name>\n", xml_escape(n)));
        }
        if let Some(k) = &trk.kind {
            out.push_str(&format!("    <type>{}</type>\n", xml_escape(k)));
        }
        for seg in &trk.segs {
            out.push_str("    <trkseg>\n");
            for p in seg {
                let lat = fmt_num(p.lat.unwrap_or(0.0));
                let lon = fmt_num(p.lon.unwrap_or(0.0));
                out.push_str(&format!("      <trkpt lat=\"{lat}\" lon=\"{lon}\">\n"));
                if let Some(e) = p.ele {
                    out.push_str(&format!("        <ele>{}</ele>\n", fmt_num(e)));
                }
                if let Some(t) = &p.time {
                    out.push_str(&format!("        <time>{}</time>\n", xml_escape(t)));
                }
                if opt.include_extensions && (p.hr.is_some() || p.cad.is_some()) {
                    out.push_str("        <extensions>\n");
                    out.push_str("          <gpxtpx:TrackPointExtension>\n");
                    if let Some(hr) = p.hr {
                        out.push_str(&format!("            <gpxtpx:hr>{hr}</gpxtpx:hr>\n"));
                    }
                    if let Some(cad) = p.cad {
                        out.push_str(&format!("            <gpxtpx:cad>{cad}</gpxtpx:cad>\n"));
                    }
                    out.push_str("          </gpxtpx:TrackPointExtension>\n");
                    out.push_str("        </extensions>\n");
                }
                out.push_str("      </trkpt>\n");
            }
            out.push_str("    </trkseg>\n");
        }
        out.push_str("  </trk>\n");
    }

    out.push_str("</gpx>\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const HAPPY: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<TrainingCenterDatabase xmlns="http://www.garmin.com/xmlschemas/TrainingCenterDatabase/v2">
  <Activities>
    <Activity Sport="Running">
      <Id>2026-07-01T08:00:00Z</Id>
      <Lap StartTime="2026-07-01T08:00:00Z">
        <Track>
          <Trackpoint>
            <Time>2026-07-01T08:00:00Z</Time>
            <Position>
              <LatitudeDegrees>52.100</LatitudeDegrees>
              <LongitudeDegrees>5.100</LongitudeDegrees>
            </Position>
            <AltitudeMeters>10</AltitudeMeters>
            <HeartRateBpm><Value>120</Value></HeartRateBpm>
            <Cadence>82</Cadence>
          </Trackpoint>
          <Trackpoint>
            <Time>2026-07-01T08:05:00Z</Time>
            <Position>
              <LatitudeDegrees>52.101</LatitudeDegrees>
              <LongitudeDegrees>5.102</LongitudeDegrees>
            </Position>
            <AltitudeMeters>12</AltitudeMeters>
            <HeartRateBpm><Value>130</Value></HeartRateBpm>
          </Trackpoint>
        </Track>
      </Lap>
    </Activity>
  </Activities>
</TrainingCenterDatabase>"#;

    #[test]
    fn happy_two_trackpoints_with_timestamps() {
        let gpx = convert(HAPPY, &Options::default()).unwrap();
        // Two trackpoints, both coordinates and timestamps preserved.
        assert_eq!(gpx.matches("<trkpt ").count(), 2);
        assert!(gpx.contains("lat=\"52.1\" lon=\"5.1\""));
        assert!(gpx.contains("lat=\"52.101\" lon=\"5.102\""));
        assert!(gpx.contains("<ele>10</ele>"));
        assert!(gpx.contains("<ele>12</ele>"));
        assert!(gpx.contains("<time>2026-07-01T08:00:00Z</time>"));
        assert!(gpx.contains("<time>2026-07-01T08:05:00Z</time>"));
        // Metadata time = earliest trackpoint time.
        assert!(gpx.contains("<metadata>\n    <time>2026-07-01T08:00:00Z</time>"));
        // Sport → <type>, Activity Id → <name>.
        assert!(gpx.contains("<type>Running</type>"));
        assert!(gpx.contains("<name>2026-07-01T08:00:00Z</name>"));
        // Heart rate + cadence become TrackPointExtension elements by default.
        assert!(gpx.contains("xmlns:gpxtpx="));
        assert!(gpx.contains("<gpxtpx:hr>120</gpxtpx:hr>"));
        assert!(gpx.contains("<gpxtpx:cad>82</gpxtpx:cad>"));
        assert!(gpx.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
    }

    #[test]
    fn extensions_can_be_disabled() {
        let gpx = convert(HAPPY, &Options { include_extensions: false }).unwrap();
        assert!(!gpx.contains("gpxtpx"));
        assert!(!gpx.contains("<extensions>"));
        // Core trackpoint data is still preserved.
        assert!(gpx.contains("<time>2026-07-01T08:00:00Z</time>"));
    }

    #[test]
    fn course_name_becomes_track_name() {
        let tcx = r#"<TrainingCenterDatabase>
  <Courses><Course>
    <Name>Park Loop</Name>
    <Track>
      <Trackpoint><Position><LatitudeDegrees>1.0</LatitudeDegrees><LongitudeDegrees>2.0</LongitudeDegrees></Position></Trackpoint>
      <Trackpoint><Position><LatitudeDegrees>1.1</LatitudeDegrees><LongitudeDegrees>2.1</LongitudeDegrees></Position></Trackpoint>
    </Track>
  </Course></Courses>
</TrainingCenterDatabase>"#;
        let gpx = convert(tcx, &Options::default()).unwrap();
        assert!(gpx.contains("<name>Park Loop</name>"));
        assert_eq!(gpx.matches("<trkpt ").count(), 2);
    }

    #[test]
    fn multi_lap_becomes_multiple_segments() {
        let tcx = r#"<TrainingCenterDatabase><Activities><Activity Sport="Biking">
  <Lap><Track>
    <Trackpoint><Position><LatitudeDegrees>1.0</LatitudeDegrees><LongitudeDegrees>2.0</LongitudeDegrees></Position></Trackpoint>
  </Track></Lap>
  <Lap><Track>
    <Trackpoint><Position><LatitudeDegrees>3.0</LatitudeDegrees><LongitudeDegrees>4.0</LongitudeDegrees></Position></Trackpoint>
  </Track></Lap>
</Activity></Activities></TrainingCenterDatabase>"#;
        let gpx = convert(tcx, &Options::default()).unwrap();
        assert_eq!(gpx.matches("<trkseg>").count(), 2);
        assert_eq!(gpx.matches("<trk>").count(), 1);
    }

    #[test]
    fn empty_input_errors() {
        assert!(convert("   ", &Options::default()).is_err());
    }

    #[test]
    fn non_xml_input_errors() {
        let err = convert("just some text", &Options::default()).unwrap_err();
        assert!(err.contains("does not look like TCX"));
    }

    #[test]
    fn trackpoint_missing_position_yields_no_track_error() {
        // A well-formed activity whose only trackpoint lacks a <Position> — the
        // point is skipped, leaving no track, so conversion reports an error
        // rather than emitting an empty <gpx>.
        let tcx = r#"<TrainingCenterDatabase><Activities><Activity Sport="Running">
  <Id>2026-07-01T08:00:00Z</Id>
  <Lap><Track>
    <Trackpoint><Time>2026-07-01T08:00:00Z</Time><AltitudeMeters>10</AltitudeMeters></Trackpoint>
  </Track></Lap>
</Activity></Activities></TrainingCenterDatabase>"#;
        let err = convert(tcx, &Options::default()).unwrap_err();
        assert!(err.contains("no TCX activity or course with trackpoints"));
    }

    #[test]
    fn malformed_xml_errors() {
        let tcx = "<TrainingCenterDatabase><Activities><Activity><Lap><Track><Trackpoint>";
        assert!(convert(tcx, &Options::default()).is_err());
    }
}
