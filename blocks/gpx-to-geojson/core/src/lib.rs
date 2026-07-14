//! gpx-to-geojson core — convert GPX/KML GPS tracks & waypoints into GeoJSON,
//! and convert a GeoJSON FeatureCollection/Feature/Geometry back into GPX.
//! Pure-Rust (`quick-xml` + `serde_json` with `preserve_order`), no
//! wafer/wasm-bindgen deps; shared by the chat skill block and the web page.
//!
//! Format is auto-detected from the input's shape (a `<gpx` or `<kml` root vs.
//! a JSON document) rather than a separate `input_format` parameter, matching
//! how every reviewed competitor converter behaves.
//!
//! GPX mapping: `trk`/`trkseg`/`trkpt` → LineString (MultiLineString for a
//! multi-segment track), `rte`/`rtept` → LineString, `wpt` → Point. Standard
//! tags (`name`, `cmt`, `desc`, `link`, `keywords`, `sym`, `type`) become
//! properties; elevation becomes the position's third coordinate; per-point
//! timestamps become a `coordTimes` property (matching the naming convention
//! used by the reference JS GPX/KML→GeoJSON library, for interchange
//! familiarity — no code was shared or copied).
//!
//! KML mapping: `Placemark`'s `Point`/`LineString`/`Polygon` → matching
//! GeoJSON geometry, `MultiGeometry` → `GeometryCollection`; `name`/
//! `description` → properties; `ExtendedData`/`SimpleData` → arbitrary
//! properties; `TimeSpan`/`TimeStamp` → `begin`/`end`/`time` properties;
//! `Style`/`styleUrl`/`StyleMap` (inline or shared, resolved in a first pass)
//! → simplestyle-spec-style properties (`stroke`, `stroke-width`,
//! `stroke-opacity`, `fill`, `fill-opacity`, `marker-color`), toggled off with
//! `include_styles = false`.
//!
//! Reverse (GeoJSON → GPX): Point → `wpt`, LineString/MultiLineString → a
//! `trk` with one `trkseg` per line, Polygon/MultiPolygon → a `trk` per
//! exterior ring (GPX has no polygon primitive, so interior rings/holes are
//! dropped — a stated limitation), GeometryCollection is flattened
//! recursively. A `coordTimes` property (flat or nested, mirroring the
//! forward direction) is restored as per-point `<time>` when present, so a
//! GPX → GeoJSON → GPX round trip preserves timestamps.

use quick_xml::encoding::Decoder;
use quick_xml::events::{BytesStart, BytesText, Event};
use quick_xml::name::QName;
use quick_xml::reader::Reader;
use serde_json::{json, Map, Value};
use std::collections::HashMap;

/// Decode an XML text/CDATA node's bytes to UTF-8 and unescape entities
/// (`&amp;`, `&lt;`, numeric refs, ...). Falls back to the raw decoded text if
/// unescaping fails (malformed entity), and to an empty string if decoding
/// itself fails (invalid UTF-8).
fn decode_text(t: &BytesText) -> String {
    match t.decode() {
        Ok(s) => quick_xml::escape::unescape(&s).map(|u| u.into_owned()).unwrap_or_else(|_| s.into_owned()),
        Err(_) => String::new(),
    }
}

/// Which direction to convert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// GPX/KML XML input → a GeoJSON `FeatureCollection`.
    GeoJson,
    /// GeoJSON input → a GPX document.
    Gpx,
}

impl OutputFormat {
    pub fn parse(s: &str) -> Result<OutputFormat, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "geojson" | "" => Ok(OutputFormat::GeoJson),
            "gpx" => Ok(OutputFormat::Gpx),
            other => Err(format!("unknown output_format '{other}' (use geojson or gpx)")),
        }
    }
}

pub struct Options {
    pub output_format: OutputFormat,
    /// KML only: fold Style/styleUrl/StyleMap color+width into simplestyle
    /// properties. Ignored for GPX input and for GeoJSON→GPX.
    pub include_styles: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options { output_format: OutputFormat::GeoJson, include_styles: true }
    }
}

/// Convert `input` per `opt`. See the module doc for the exact element/property
/// mapping in each direction.
pub fn convert(input: &str, opt: &Options) -> Result<String, String> {
    let trimmed = input.trim_start();
    if trimmed.is_empty() {
        return Err("input is empty".to_string());
    }
    let looks_xml = trimmed.starts_with('<');
    match opt.output_format {
        OutputFormat::GeoJson => {
            if !looks_xml {
                return Err(
                    "output_format=\"geojson\" expects GPX or KML XML input, but this looks \
                     like GeoJSON already. Set output_format=\"gpx\" to convert it to GPX instead."
                        .to_string(),
                );
            }
            if is_kml(trimmed) {
                kml_to_geojson(input, opt.include_styles)
            } else {
                gpx_to_geojson(input)
            }
        }
        OutputFormat::Gpx => {
            if looks_xml {
                return Err(
                    "output_format=\"gpx\" expects GeoJSON input (a Feature, FeatureCollection, \
                     or Geometry), but this looks like GPX/KML XML. Set output_format=\"geojson\" \
                     to convert GPX/KML into GeoJSON instead."
                        .to_string(),
                );
            }
            geojson_to_gpx(input)
        }
    }
}

fn is_kml(trimmed: &str) -> bool {
    // Heuristic (matches every reviewed competitor's auto-detection): a real
    // KML document's root element is <kml>, which appears near the very start
    // (optionally after an XML declaration/comment/BOM).
    trimmed.to_ascii_lowercase().contains("<kml")
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

// ---------------------------------------------------------------------------
// GPX → GeoJSON
// ---------------------------------------------------------------------------

#[derive(Default, Clone)]
struct Pt {
    lat: f64,
    lon: f64,
    ele: Option<f64>,
    time: Option<String>,
}

fn coord_array(p: &Pt) -> Vec<f64> {
    match p.ele {
        Some(e) => vec![p.lon, p.lat, e],
        None => vec![p.lon, p.lat],
    }
}

#[derive(Default, Clone)]
struct LineMeta {
    name: Option<String>,
    cmt: Option<String>,
    desc: Option<String>,
    link: Option<String>,
    keywords: Option<String>,
    sym: Option<String>,
    kind: Option<String>,
}

#[derive(Default, Clone)]
struct WptRec {
    pt: Pt,
    name: Option<String>,
    cmt: Option<String>,
    desc: Option<String>,
    link: Option<String>,
    sym: Option<String>,
    kind: Option<String>,
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

fn read_pt(e: &BytesStart, decoder: Decoder) -> Result<Pt, String> {
    let lat = get_attr(e, decoder, "lat").ok_or("a <trkpt>/<rtept>/<wpt> is missing a lat attribute")?;
    let lon = get_attr(e, decoder, "lon").ok_or("a <trkpt>/<rtept>/<wpt> is missing a lon attribute")?;
    let lat: f64 = lat.trim().parse().map_err(|_| format!("invalid lat value '{lat}'"))?;
    let lon: f64 = lon.trim().parse().map_err(|_| format!("invalid lon value '{lon}'"))?;
    Ok(Pt { lat, lon, ele: None, time: None })
}

fn set_line_field(m: &mut LineMeta, field: &str, val: &str) {
    match field {
        "name" => m.name = Some(val.to_string()),
        "cmt" => m.cmt = Some(val.to_string()),
        "desc" => m.desc = Some(val.to_string()),
        "keywords" => m.keywords = Some(val.to_string()),
        "sym" => m.sym = Some(val.to_string()),
        "type" => m.kind = Some(val.to_string()),
        _ => {}
    }
}

fn set_wpt_field(w: &mut WptRec, field: &str, val: &str) {
    match field {
        "name" => w.name = Some(val.to_string()),
        "cmt" => w.cmt = Some(val.to_string()),
        "desc" => w.desc = Some(val.to_string()),
        "sym" => w.sym = Some(val.to_string()),
        "type" => w.kind = Some(val.to_string()),
        _ => {}
    }
}

/// Build a LineString/MultiLineString feature from one or more point segments
/// (a track may have several `trkseg`; a route always has exactly one line).
fn build_line_feature(meta: &LineMeta, segs: &[Vec<Pt>]) -> Option<Value> {
    let non_empty: Vec<&Vec<Pt>> = segs.iter().filter(|s| !s.is_empty()).collect();
    if non_empty.is_empty() {
        return None;
    }
    let mut coords_all: Vec<Vec<Vec<f64>>> = Vec::new();
    let mut times_all: Vec<Vec<Value>> = Vec::new();
    let mut any_time = false;
    for seg in &non_empty {
        let mut coords = Vec::new();
        let mut times = Vec::new();
        for p in seg.iter() {
            coords.push(coord_array(p));
            match &p.time {
                Some(t) => {
                    times.push(Value::String(t.clone()));
                    any_time = true;
                }
                None => times.push(Value::Null),
            }
        }
        coords_all.push(coords);
        times_all.push(times);
    }
    let geometry = if coords_all.len() == 1 {
        json!({ "type": "LineString", "coordinates": coords_all[0] })
    } else {
        json!({ "type": "MultiLineString", "coordinates": coords_all })
    };
    let mut props = Map::new();
    if let Some(v) = &meta.name {
        props.insert("name".into(), json!(v));
    }
    if let Some(v) = &meta.cmt {
        props.insert("cmt".into(), json!(v));
    }
    if let Some(v) = &meta.desc {
        props.insert("desc".into(), json!(v));
    }
    if let Some(v) = &meta.link {
        props.insert("link".into(), json!(v));
    }
    if let Some(v) = &meta.keywords {
        props.insert("keywords".into(), json!(v));
    }
    if let Some(v) = &meta.sym {
        props.insert("sym".into(), json!(v));
    }
    if let Some(v) = &meta.kind {
        props.insert("type".into(), json!(v));
    }
    if any_time {
        if times_all.len() == 1 {
            props.insert("coordTimes".into(), Value::Array(times_all.into_iter().next().unwrap()));
        } else {
            props.insert("coordTimes".into(), Value::Array(times_all.into_iter().map(Value::Array).collect()));
        }
    }
    Some(json!({ "type": "Feature", "geometry": geometry, "properties": Value::Object(props) }))
}

/// Parse a GPX document into a GeoJSON `FeatureCollection` text.
pub fn gpx_to_geojson(xml: &str) -> Result<String, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let decoder = reader.decoder();

    let mut tracks: Vec<(LineMeta, Vec<Vec<Pt>>)> = Vec::new();
    let mut routes: Vec<(LineMeta, Vec<Pt>)> = Vec::new();
    let mut waypoints: Vec<WptRec> = Vec::new();

    let mut cur_track_meta = LineMeta::default();
    let mut cur_track_segs: Vec<Vec<Pt>> = Vec::new();
    let mut cur_seg: Vec<Pt> = Vec::new();

    let mut cur_route_meta = LineMeta::default();
    let mut cur_route_pts: Vec<Pt> = Vec::new();

    let mut cur_pt = Pt::default();
    let mut cur_wpt = WptRec::default();

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
                        cur_track_meta = LineMeta::default();
                        cur_track_segs = Vec::new();
                        stack.push(Ctx::Trk);
                    }
                    "trkseg" => {
                        cur_seg = Vec::new();
                    }
                    "rte" => {
                        cur_route_meta = LineMeta::default();
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
                        cur_wpt = WptRec { pt: read_pt(&e, decoder)?, ..Default::default() };
                        if is_empty {
                            waypoints.push(cur_wpt.clone());
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
                    "link" => {
                        if let Some(href) = get_attr(&e, decoder, "href") {
                            match stack.last() {
                                Some(Ctx::Trk) => cur_track_meta.link = Some(href),
                                Some(Ctx::Rte) => cur_route_meta.link = Some(href),
                                Some(Ctx::Wpt) => cur_wpt.link = Some(href),
                                _ => {}
                            }
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
                                Some(Ctx::Wpt) => cur_wpt.pt.ele = Some(v),
                                _ => {}
                            }
                        }
                    }
                    "time" => {
                        let s = text_buf.trim();
                        if !s.is_empty() {
                            match stack.last() {
                                Some(Ctx::TrkPt) | Some(Ctx::RtePt) => cur_pt.time = Some(s.to_string()),
                                Some(Ctx::Wpt) => cur_wpt.pt.time = Some(s.to_string()),
                                _ => {}
                            }
                        }
                    }
                    "name" | "cmt" | "desc" | "keywords" | "sym" | "type" => {
                        let s = text_buf.trim();
                        if !s.is_empty() {
                            match stack.last() {
                                Some(Ctx::Trk) => set_line_field(&mut cur_track_meta, &name, s),
                                Some(Ctx::Rte) => set_line_field(&mut cur_route_meta, &name, s),
                                Some(Ctx::Wpt) => set_wpt_field(&mut cur_wpt, &name, s),
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
                                tracks.push((cur_track_meta.clone(), std::mem::take(&mut cur_track_segs)));
                            }
                            stack.pop();
                        }
                    }
                    "rte" => {
                        if let Some(Ctx::Rte) = stack.last() {
                            if !cur_route_pts.is_empty() {
                                routes.push((cur_route_meta.clone(), std::mem::take(&mut cur_route_pts)));
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

    if tracks.is_empty() && routes.is_empty() && waypoints.is_empty() {
        return Err(
            "no GPX track, route, or waypoint found (expected a <trk>, <rte>, or <wpt> element)"
                .to_string(),
        );
    }

    let mut features: Vec<Value> = Vec::new();
    for (meta, segs) in &tracks {
        if let Some(f) = build_line_feature(meta, segs) {
            features.push(f);
        }
    }
    for (meta, pts) in &routes {
        if let Some(f) = build_line_feature(meta, std::slice::from_ref(pts)) {
            features.push(f);
        }
    }
    for w in &waypoints {
        let mut props = Map::new();
        if let Some(v) = &w.name {
            props.insert("name".into(), json!(v));
        }
        if let Some(v) = &w.cmt {
            props.insert("cmt".into(), json!(v));
        }
        if let Some(v) = &w.desc {
            props.insert("desc".into(), json!(v));
        }
        if let Some(v) = &w.link {
            props.insert("link".into(), json!(v));
        }
        if let Some(v) = &w.sym {
            props.insert("sym".into(), json!(v));
        }
        if let Some(v) = &w.kind {
            props.insert("type".into(), json!(v));
        }
        if let Some(v) = &w.pt.time {
            props.insert("time".into(), json!(v));
        }
        features.push(json!({
            "type": "Feature",
            "geometry": { "type": "Point", "coordinates": coord_array(&w.pt) },
            "properties": Value::Object(props)
        }));
    }

    let fc = json!({ "type": "FeatureCollection", "features": features });
    serde_json::to_string_pretty(&fc).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// KML → GeoJSON
// ---------------------------------------------------------------------------

#[derive(Default, Clone)]
struct KmlStyle {
    stroke: Option<String>,
    stroke_opacity: Option<f64>,
    stroke_width: Option<f64>,
    fill: Option<String>,
    fill_opacity: Option<f64>,
    has_fill: Option<bool>,
    marker_color: Option<String>,
}

/// KML colors are `aabbggrr` hex (alpha, blue, green, red). Returns
/// (`#rrggbb`, alpha as a 0..1 opacity fraction).
fn parse_kml_color(s: &str) -> Option<(String, f64)> {
    let s = s.trim();
    if s.len() != 8 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let aa = u8::from_str_radix(&s[0..2], 16).ok()?;
    let bb = &s[2..4];
    let gg = &s[4..6];
    let rr = &s[6..8];
    let hex = format!("#{}{}{}", rr, gg, bb).to_ascii_lowercase();
    let opacity = ((aa as f64 / 255.0) * 1000.0).round() / 1000.0;
    Some((hex, opacity))
}

/// Pass 1: collect every shared `<Style id="...">` and `<StyleMap id="...">`
/// (resolved to the "normal" pair's style id) so Placemark `styleUrl`
/// references can be resolved regardless of document order.
fn collect_kml_styles(xml: &str) -> Result<(HashMap<String, KmlStyle>, HashMap<String, String>), String> {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum C {
        Style,
        LineStyle,
        PolyStyle,
        IconStyle,
        Pair,
    }

    let mut styles: HashMap<String, KmlStyle> = HashMap::new();
    let mut stylemaps: HashMap<String, String> = HashMap::new();

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let decoder = reader.decoder();

    let mut stack: Vec<C> = Vec::new();
    let mut cur_style_id: Option<String> = None;
    let mut cur_style = KmlStyle::default();
    let mut cur_stylemap_id: Option<String> = None;
    let mut cur_pairs: Vec<(String, String)> = Vec::new();
    let mut pair_key = String::new();
    let mut pair_url = String::new();
    let mut text_buf = String::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Err(e) => return Err(format!("malformed KML/XML: {e}")),
            Ok(Event::Eof) => break,
            Ok(ev @ (Event::Start(_) | Event::Empty(_))) => {
                let is_empty = matches!(ev, Event::Empty(_));
                let e = match ev {
                    Event::Start(e) | Event::Empty(e) => e,
                    _ => unreachable!(),
                };
                let name = local_name(e.name());
                match name.as_str() {
                    "Style" => {
                        cur_style_id = get_attr(&e, decoder, "id");
                        cur_style = KmlStyle::default();
                        if !is_empty {
                            stack.push(C::Style);
                        } else if let Some(id) = cur_style_id.take() {
                            styles.insert(id, cur_style.clone());
                        }
                    }
                    "LineStyle" if !is_empty => stack.push(C::LineStyle),
                    "PolyStyle" if !is_empty => stack.push(C::PolyStyle),
                    "IconStyle" if !is_empty => stack.push(C::IconStyle),
                    "StyleMap" => {
                        cur_stylemap_id = get_attr(&e, decoder, "id");
                        cur_pairs = Vec::new();
                        // No stack marker needed: StyleMap has no leaf children of its
                        // own, only <Pair> elements, which push/pop C::Pair themselves.
                    }
                    "Pair" if !is_empty => {
                        pair_key.clear();
                        pair_url.clear();
                        stack.push(C::Pair);
                    }
                    _ => {}
                }
                text_buf.clear();
            }
            Ok(Event::Text(t)) => {
                text_buf.push_str(&decode_text(&t));
            }
            Ok(Event::CData(t)) => {
                text_buf.push_str(&String::from_utf8_lossy(&t.into_inner()));
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name());
                match name.as_str() {
                    "color" => {
                        if let Some((hex, op)) = parse_kml_color(text_buf.trim()) {
                            match stack.last() {
                                Some(C::LineStyle) => {
                                    cur_style.stroke = Some(hex);
                                    cur_style.stroke_opacity = Some(op);
                                }
                                Some(C::PolyStyle) => {
                                    cur_style.fill = Some(hex);
                                    cur_style.fill_opacity = Some(op);
                                }
                                Some(C::IconStyle) => cur_style.marker_color = Some(hex),
                                _ => {}
                            }
                        }
                    }
                    "width" => {
                        if let Some(C::LineStyle) = stack.last() {
                            if let Ok(v) = text_buf.trim().parse::<f64>() {
                                cur_style.stroke_width = Some(v);
                            }
                        }
                    }
                    "fill" => {
                        if let Some(C::PolyStyle) = stack.last() {
                            cur_style.has_fill = Some(text_buf.trim() != "0");
                        }
                    }
                    "key" => {
                        if let Some(C::Pair) = stack.last() {
                            pair_key = text_buf.trim().to_string();
                        }
                    }
                    "styleUrl" => {
                        if let Some(C::Pair) = stack.last() {
                            pair_url = text_buf.trim().trim_start_matches('#').to_string();
                        }
                    }
                    "LineStyle" | "PolyStyle" | "IconStyle" => {
                        if stack.last().is_some() {
                            stack.pop();
                        }
                    }
                    "Pair" => {
                        if let Some(C::Pair) = stack.last() {
                            cur_pairs.push((pair_key.clone(), pair_url.clone()));
                            stack.pop();
                        }
                    }
                    "Style" => {
                        if let Some(C::Style) = stack.last() {
                            if let Some(id) = cur_style_id.take() {
                                styles.insert(id, cur_style.clone());
                            }
                            stack.pop();
                        }
                    }
                    "StyleMap" => {
                        if let Some(id) = cur_stylemap_id.take() {
                            let target = cur_pairs
                                .iter()
                                .find(|(k, _)| k == "normal")
                                .map(|(_, u)| u.clone())
                                .or_else(|| cur_pairs.first().map(|(_, u)| u.clone()));
                            if let Some(t) = target {
                                stylemaps.insert(id, t);
                            }
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

    Ok((styles, stylemaps))
}

fn parse_coords_text(text: &str) -> Vec<Vec<f64>> {
    text.split_whitespace()
        .filter_map(|tuple| {
            let parts: Vec<&str> = tuple.split(',').collect();
            if parts.len() < 2 {
                return None;
            }
            let lon: f64 = parts[0].trim().parse().ok()?;
            let lat: f64 = parts[1].trim().parse().ok()?;
            if let Some(alt_str) = parts.get(2) {
                if let Ok(alt) = alt_str.trim().parse::<f64>() {
                    return Some(vec![lon, lat, alt]);
                }
            }
            Some(vec![lon, lat])
        })
        .collect()
}

/// Parse a KML document into a GeoJSON `FeatureCollection` text.
pub fn kml_to_geojson(xml: &str, include_styles: bool) -> Result<String, String> {
    let (styles, stylemaps) = collect_kml_styles(xml)?;

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let decoder = reader.decoder();

    let mut features: Vec<Value> = Vec::new();

    // Placemark-scoped state.
    let mut in_placemark = false;
    let mut pm_name: Option<String> = None;
    let mut pm_desc: Option<String> = None;
    let mut pm_style_url: Option<String> = None;
    let mut pm_inline_style: Option<KmlStyle> = None;
    let mut pm_extended: Map<String, Value> = Map::new();
    let mut pm_time: Option<String> = None;
    let mut pm_begin: Option<String> = None;
    let mut pm_end: Option<String> = None;
    let mut placemark_geoms: Vec<Value> = Vec::new();
    let mut multi_stack: Vec<Vec<Value>> = Vec::new();

    // Geometry-leaf state.
    let mut in_point = false;
    let mut in_linestring = false;
    let mut in_outer_ring = false;
    let mut in_inner_ring = false;
    let mut point_coord: Vec<f64> = Vec::new();
    let mut line_coords: Vec<Vec<f64>> = Vec::new();
    let mut poly_outer: Vec<Vec<f64>> = Vec::new();
    let mut poly_inners: Vec<Vec<Vec<f64>>> = Vec::new();
    let mut cur_inner: Vec<Vec<f64>> = Vec::new();

    // Inline-style state.
    let mut in_style = false;
    let mut in_line_style = false;
    let mut in_poly_style = false;
    let mut in_icon_style = false;
    let mut style_building = KmlStyle::default();

    // ExtendedData / SimpleData state.
    let mut in_extended_data = false;
    let mut data_name: Option<String> = None;
    let mut simple_data_name: Option<String> = None;

    // TimeSpan / TimeStamp state.
    let mut in_timespan = false;
    let mut in_timestamp = false;

    let mut text_buf = String::new();
    let mut buf = Vec::new();

    macro_rules! push_geom {
        ($g:expr) => {{
            if let Some(top) = multi_stack.last_mut() {
                top.push($g);
            } else {
                placemark_geoms.push($g);
            }
        }};
    }

    loop {
        match reader.read_event_into(&mut buf) {
            Err(e) => return Err(format!("malformed KML/XML: {e}")),
            Ok(Event::Eof) => break,
            Ok(ev @ (Event::Start(_) | Event::Empty(_))) => {
                let is_empty = matches!(ev, Event::Empty(_));
                let e = match ev {
                    Event::Start(e) | Event::Empty(e) => e,
                    _ => unreachable!(),
                };
                let name = local_name(e.name());
                match name.as_str() {
                    "Placemark" => {
                        in_placemark = true;
                        pm_name = None;
                        pm_desc = None;
                        pm_style_url = None;
                        pm_inline_style = None;
                        pm_extended = Map::new();
                        pm_time = None;
                        pm_begin = None;
                        pm_end = None;
                        placemark_geoms = Vec::new();
                        multi_stack = Vec::new();
                        if is_empty {
                            in_placemark = false;
                        }
                    }
                    "MultiGeometry" => multi_stack.push(Vec::new()),
                    "Point" => {
                        in_point = true;
                        point_coord = Vec::new();
                        if is_empty {
                            in_point = false;
                        }
                    }
                    "LineString" => {
                        in_linestring = true;
                        line_coords = Vec::new();
                        if is_empty {
                            in_linestring = false;
                        }
                    }
                    "Polygon" => {
                        poly_outer = Vec::new();
                        poly_inners = Vec::new();
                    }
                    "outerBoundaryIs" => in_outer_ring = true,
                    "innerBoundaryIs" => {
                        in_inner_ring = true;
                        cur_inner = Vec::new();
                    }
                    "Style" => {
                        if in_placemark {
                            in_style = true;
                            style_building = KmlStyle::default();
                            if is_empty {
                                in_style = false;
                                pm_inline_style = Some(style_building.clone());
                            }
                        }
                    }
                    "LineStyle" if in_style => in_line_style = true,
                    "PolyStyle" if in_style => in_poly_style = true,
                    "IconStyle" if in_style => in_icon_style = true,
                    "ExtendedData" => in_extended_data = true,
                    "Data" if in_extended_data => data_name = get_attr(&e, decoder, "name"),
                    "SimpleData" if in_extended_data => simple_data_name = get_attr(&e, decoder, "name"),
                    "TimeSpan" => in_timespan = true,
                    "TimeStamp" => in_timestamp = true,
                    _ => {}
                }
                text_buf.clear();
            }
            Ok(Event::Text(t)) => {
                text_buf.push_str(&decode_text(&t));
            }
            Ok(Event::CData(t)) => {
                text_buf.push_str(&String::from_utf8_lossy(&t.into_inner()));
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name());
                match name.as_str() {
                    "name" if in_placemark && !in_extended_data => {
                        let s = text_buf.trim();
                        if !s.is_empty() {
                            pm_name = Some(s.to_string());
                        }
                    }
                    "description" if in_placemark && !in_extended_data => {
                        let s = text_buf.trim();
                        if !s.is_empty() {
                            pm_desc = Some(s.to_string());
                        }
                    }
                    "styleUrl" if in_placemark => {
                        let s = text_buf.trim().trim_start_matches('#');
                        if !s.is_empty() {
                            pm_style_url = Some(s.to_string());
                        }
                    }
                    "coordinates" => {
                        let parsed = parse_coords_text(&text_buf);
                        if in_point {
                            point_coord = parsed.into_iter().next().unwrap_or_default();
                        } else if in_linestring {
                            line_coords = parsed;
                        } else if in_outer_ring {
                            poly_outer = parsed;
                        } else if in_inner_ring {
                            cur_inner = parsed;
                        }
                    }
                    "Point" => {
                        in_point = false;
                        if !point_coord.is_empty() {
                            push_geom!(json!({ "type": "Point", "coordinates": point_coord.clone() }));
                        }
                    }
                    "LineString" => {
                        in_linestring = false;
                        if !line_coords.is_empty() {
                            push_geom!(json!({ "type": "LineString", "coordinates": line_coords.clone() }));
                        }
                    }
                    "outerBoundaryIs" => in_outer_ring = false,
                    "innerBoundaryIs" => {
                        in_inner_ring = false;
                        if !cur_inner.is_empty() {
                            poly_inners.push(std::mem::take(&mut cur_inner));
                        }
                    }
                    "Polygon" => {
                        if !poly_outer.is_empty() {
                            let mut rings = vec![poly_outer.clone()];
                            rings.extend(poly_inners.clone());
                            push_geom!(json!({ "type": "Polygon", "coordinates": rings }));
                        }
                    }
                    "MultiGeometry" => {
                        if let Some(geoms) = multi_stack.pop() {
                            push_geom!(json!({ "type": "GeometryCollection", "geometries": geoms }));
                        }
                    }
                    "color" => {
                        if let Some((hex, op)) = parse_kml_color(text_buf.trim()) {
                            if in_line_style {
                                style_building.stroke = Some(hex);
                                style_building.stroke_opacity = Some(op);
                            } else if in_poly_style {
                                style_building.fill = Some(hex);
                                style_building.fill_opacity = Some(op);
                            } else if in_icon_style {
                                style_building.marker_color = Some(hex);
                            }
                        }
                    }
                    "width" if in_line_style => {
                        if let Ok(v) = text_buf.trim().parse::<f64>() {
                            style_building.stroke_width = Some(v);
                        }
                    }
                    "fill" if in_poly_style => {
                        style_building.has_fill = Some(text_buf.trim() != "0");
                    }
                    "LineStyle" => in_line_style = false,
                    "PolyStyle" => in_poly_style = false,
                    "IconStyle" => in_icon_style = false,
                    "Style" => {
                        if in_style {
                            in_style = false;
                            pm_inline_style = Some(style_building.clone());
                        }
                    }
                    "value" if in_extended_data => {
                        if let Some(dn) = &data_name {
                            let s = text_buf.trim();
                            if !s.is_empty() {
                                pm_extended.insert(dn.clone(), json!(s));
                            }
                        }
                    }
                    "Data" => data_name = None,
                    "SimpleData" if in_extended_data => {
                        if let Some(sn) = simple_data_name.take() {
                            let s = text_buf.trim();
                            if !s.is_empty() {
                                pm_extended.insert(sn, json!(s));
                            }
                        }
                    }
                    "ExtendedData" => in_extended_data = false,
                    "begin" if in_timespan => {
                        let s = text_buf.trim();
                        if !s.is_empty() {
                            pm_begin = Some(s.to_string());
                        }
                    }
                    "end" if in_timespan => {
                        let s = text_buf.trim();
                        if !s.is_empty() {
                            pm_end = Some(s.to_string());
                        }
                    }
                    "when" if in_timestamp => {
                        let s = text_buf.trim();
                        if !s.is_empty() {
                            pm_time = Some(s.to_string());
                        }
                    }
                    "TimeSpan" => in_timespan = false,
                    "TimeStamp" => in_timestamp = false,
                    "Placemark" => {
                        if in_placemark {
                            finish_placemark(
                                &mut features,
                                &pm_name,
                                &pm_desc,
                                &pm_style_url,
                                &pm_inline_style,
                                &pm_extended,
                                &pm_time,
                                &pm_begin,
                                &pm_end,
                                &placemark_geoms,
                                include_styles,
                                &styles,
                                &stylemaps,
                            );
                            in_placemark = false;
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

    if features.is_empty() {
        return Err(
            "no KML Placemark with a Point/LineString/Polygon/MultiGeometry geometry was found"
                .to_string(),
        );
    }

    let fc = json!({ "type": "FeatureCollection", "features": features });
    serde_json::to_string_pretty(&fc).map_err(|e| e.to_string())
}

#[allow(clippy::too_many_arguments)]
fn finish_placemark(
    features: &mut Vec<Value>,
    name: &Option<String>,
    desc: &Option<String>,
    style_url: &Option<String>,
    inline_style: &Option<KmlStyle>,
    extended: &Map<String, Value>,
    time: &Option<String>,
    begin: &Option<String>,
    end: &Option<String>,
    geoms: &[Value],
    include_styles: bool,
    styles: &HashMap<String, KmlStyle>,
    stylemaps: &HashMap<String, String>,
) {
    if geoms.is_empty() {
        return;
    }
    let geometry = if geoms.len() == 1 {
        geoms[0].clone()
    } else {
        json!({ "type": "GeometryCollection", "geometries": geoms })
    };

    let mut props = Map::new();
    if let Some(v) = name {
        props.insert("name".into(), json!(v));
    }
    if let Some(v) = desc {
        props.insert("description".into(), json!(v));
    }
    for (k, v) in extended {
        props.insert(k.clone(), v.clone());
    }
    if let Some(v) = begin {
        props.insert("begin".into(), json!(v));
    }
    if let Some(v) = end {
        props.insert("end".into(), json!(v));
    }
    if let Some(v) = time {
        props.insert("time".into(), json!(v));
    }

    if include_styles {
        let resolved = inline_style.clone().or_else(|| {
            style_url.as_ref().and_then(|id| {
                stylemaps
                    .get(id)
                    .and_then(|target| styles.get(target))
                    .or_else(|| styles.get(id))
                    .cloned()
            })
        });
        if let Some(st) = resolved {
            if let Some(v) = &st.stroke {
                props.insert("stroke".into(), json!(v));
            }
            if let Some(v) = st.stroke_opacity {
                props.insert("stroke-opacity".into(), json!(v));
            }
            if let Some(v) = st.stroke_width {
                props.insert("stroke-width".into(), json!(v));
            }
            if st.has_fill != Some(false) {
                if let Some(v) = &st.fill {
                    props.insert("fill".into(), json!(v));
                    if let Some(op) = st.fill_opacity {
                        props.insert("fill-opacity".into(), json!(op));
                    }
                }
            }
            if let Some(v) = &st.marker_color {
                props.insert("marker-color".into(), json!(v));
            }
        }
    }

    features.push(json!({ "type": "Feature", "geometry": geometry, "properties": Value::Object(props) }));
}

// ---------------------------------------------------------------------------
// GeoJSON → GPX
// ---------------------------------------------------------------------------

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn coord3(c: &[Value]) -> Result<(f64, f64, Option<f64>), String> {
    let lon = c.first().and_then(Value::as_f64).ok_or("coordinate is missing a longitude")?;
    let lat = c.get(1).and_then(Value::as_f64).ok_or("coordinate is missing a latitude")?;
    let ele = c.get(2).and_then(Value::as_f64);
    Ok((lon, lat, ele))
}

fn coords_2d(v: &Value) -> Result<Vec<Vec<f64>>, String> {
    let arr = v.as_array().ok_or("expected a coordinate array")?;
    arr.iter()
        .map(|pos| {
            let parr = pos.as_array().ok_or("expected a [lon, lat] coordinate pair")?;
            let (lon, lat, ele) = coord3(parr)?;
            Ok(match ele {
                Some(e) => vec![lon, lat, e],
                None => vec![lon, lat],
            })
        })
        .collect()
}

fn value_to_time(v: &Value) -> Option<String> {
    v.as_str().map(|s| s.to_string())
}

/// Normalize a feature's `coordTimes` property into one `Vec<Option<String>>`
/// per line segment, matching `segments`' shape (flat for a single line,
/// nested for a `MultiLineString`'s several segments).
fn normalize_coord_times(v: Option<&Value>, seg_count: usize) -> Vec<Vec<Option<String>>> {
    let Some(v) = v else {
        return vec![Vec::new(); seg_count.max(1)];
    };
    let Some(arr) = v.as_array() else {
        return vec![Vec::new(); seg_count.max(1)];
    };
    let is_nested = arr.first().map(Value::is_array).unwrap_or(seg_count > 1);
    if is_nested {
        arr.iter()
            .map(|seg| seg.as_array().map(|a| a.iter().map(value_to_time).collect()).unwrap_or_default())
            .collect()
    } else {
        vec![arr.iter().map(value_to_time).collect()]
    }
}

fn emit_track(name: Option<&str>, desc: Option<&str>, segments: &[Vec<Vec<f64>>], times: &[Vec<Option<String>>], trks: &mut String) {
    if segments.iter().all(|s| s.is_empty()) {
        return;
    }
    trks.push_str("  <trk>\n");
    if let Some(n) = name {
        trks.push_str(&format!("    <name>{}</name>\n", xml_escape(n)));
    }
    if let Some(d) = desc {
        trks.push_str(&format!("    <desc>{}</desc>\n", xml_escape(d)));
    }
    for (i, seg) in segments.iter().enumerate() {
        if seg.is_empty() {
            continue;
        }
        trks.push_str("    <trkseg>\n");
        let seg_times = times.get(i);
        for (j, pos) in seg.iter().enumerate() {
            let lon = pos.first().copied().unwrap_or(0.0);
            let lat = pos.get(1).copied().unwrap_or(0.0);
            trks.push_str(&format!("      <trkpt lat=\"{lat}\" lon=\"{lon}\">\n"));
            if let Some(ele) = pos.get(2) {
                trks.push_str(&format!("        <ele>{ele}</ele>\n"));
            }
            if let Some(Some(t)) = seg_times.and_then(|st| st.get(j)) {
                trks.push_str(&format!("        <time>{}</time>\n", xml_escape(t)));
            }
            trks.push_str("      </trkpt>\n");
        }
        trks.push_str("    </trkseg>\n");
    }
    trks.push_str("  </trk>\n");
}

fn emit_geometry(
    geom: &Value,
    name: Option<&str>,
    desc: Option<&str>,
    time: Option<&str>,
    coord_times: Option<&Value>,
    wpts: &mut String,
    trks: &mut String,
    any: &mut bool,
) -> Result<(), String> {
    let kind = geom.get("type").and_then(Value::as_str).unwrap_or("");
    match kind {
        "Point" => {
            let c = geom.get("coordinates").and_then(Value::as_array).ok_or("Point geometry is missing coordinates")?;
            let (lon, lat, ele) = coord3(c)?;
            wpts.push_str(&format!("  <wpt lat=\"{lat}\" lon=\"{lon}\">\n"));
            if let Some(e) = ele {
                wpts.push_str(&format!("    <ele>{e}</ele>\n"));
            }
            if let Some(t) = time {
                wpts.push_str(&format!("    <time>{}</time>\n", xml_escape(t)));
            }
            if let Some(n) = name {
                wpts.push_str(&format!("    <name>{}</name>\n", xml_escape(n)));
            }
            if let Some(d) = desc {
                wpts.push_str(&format!("    <desc>{}</desc>\n", xml_escape(d)));
            }
            wpts.push_str("  </wpt>\n");
            *any = true;
        }
        "MultiPoint" => {
            let coords = geom.get("coordinates").and_then(Value::as_array).ok_or("MultiPoint geometry is missing coordinates")?;
            for c in coords {
                let carr = c.as_array().ok_or("a MultiPoint coordinate must be an array")?;
                let pt = json!({ "type": "Point", "coordinates": carr });
                emit_geometry(&pt, name, desc, time, None, wpts, trks, any)?;
            }
        }
        "LineString" => {
            let coords = geom.get("coordinates").and_then(Value::as_array).ok_or("LineString geometry is missing coordinates")?;
            let segs = vec![coords_2d(&Value::Array(coords.clone()))?];
            let times = normalize_coord_times(coord_times, 1);
            emit_track(name, desc, &segs, &times, trks);
            *any = true;
        }
        "MultiLineString" => {
            let lines = geom.get("coordinates").and_then(Value::as_array).ok_or("MultiLineString geometry is missing coordinates")?;
            let segs: Vec<Vec<Vec<f64>>> = lines.iter().map(coords_2d).collect::<Result<_, _>>()?;
            let times = normalize_coord_times(coord_times, segs.len());
            emit_track(name, desc, &segs, &times, trks);
            *any = true;
        }
        "Polygon" => {
            let rings = geom.get("coordinates").and_then(Value::as_array).ok_or("Polygon geometry is missing coordinates")?;
            let outer = rings.first().ok_or("Polygon geometry is missing an exterior ring")?;
            let segs = vec![coords_2d(outer)?];
            emit_track(name, desc, &segs, &[], trks);
            *any = true;
        }
        "MultiPolygon" => {
            let polys = geom.get("coordinates").and_then(Value::as_array).ok_or("MultiPolygon geometry is missing coordinates")?;
            for poly in polys {
                let rings = poly.as_array().ok_or("a MultiPolygon polygon must be an array of rings")?;
                if let Some(outer) = rings.first() {
                    let segs = vec![coords_2d(outer)?];
                    emit_track(name, desc, &segs, &[], trks);
                    *any = true;
                }
            }
        }
        "GeometryCollection" => {
            let geoms = geom.get("geometries").and_then(Value::as_array).ok_or("GeometryCollection is missing geometries")?;
            for g in geoms {
                emit_geometry(g, name, desc, time, None, wpts, trks, any)?;
            }
        }
        "" => return Err("a feature has no geometry \"type\"".to_string()),
        other => return Err(format!("unsupported GeoJSON geometry type '{other}'")),
    }
    Ok(())
}

/// Convert GeoJSON (a `FeatureCollection`, a bare `Feature`, or a bare
/// geometry) into a GPX 1.1 document.
pub fn geojson_to_gpx(json_text: &str) -> Result<String, String> {
    let val: Value = serde_json::from_str(json_text).map_err(|e| format!("invalid GeoJSON: {e}"))?;

    let features: Vec<Value> = match val.get("type").and_then(Value::as_str) {
        Some("FeatureCollection") => val
            .get("features")
            .and_then(Value::as_array)
            .cloned()
            .ok_or("FeatureCollection is missing a \"features\" array")?,
        Some("Feature") => vec![val.clone()],
        Some(_) => vec![json!({ "type": "Feature", "geometry": val.clone(), "properties": {} })],
        None => {
            return Err("input does not look like GeoJSON (missing a top-level \"type\")".to_string());
        }
    };

    if features.is_empty() {
        return Err("no features found in the GeoJSON input".to_string());
    }

    let mut wpts = String::new();
    let mut trks = String::new();
    let mut any = false;

    for feat in &features {
        let geom = feat.get("geometry").cloned().unwrap_or(Value::Null);
        if geom.is_null() {
            continue;
        }
        let props = feat.get("properties").cloned().unwrap_or(json!({}));
        let name = props.get("name").and_then(Value::as_str);
        let desc = props
            .get("desc")
            .and_then(Value::as_str)
            .or_else(|| props.get("description").and_then(Value::as_str));
        let time = props.get("time").and_then(Value::as_str);
        let coord_times = props.get("coordTimes");
        emit_geometry(&geom, name, desc, time, coord_times, &mut wpts, &mut trks, &mut any)?;
    }

    if !any {
        return Err(
            "no Point/MultiPoint/LineString/MultiLineString/Polygon/MultiPolygon geometry was \
             found to convert to GPX"
                .to_string(),
        );
    }

    Ok(format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<gpx version=\"1.1\" creator=\"gizza-ai gpx-to-geojson\" xmlns=\"http://www.topografix.com/GPX/1/1\">\n{wpts}{trks}</gpx>\n"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geojson(input: &str, output_format: OutputFormat, include_styles: bool) -> Result<Value, String> {
        let out = convert(input, &Options { output_format, include_styles })?;
        serde_json::from_str(&out).map_err(|e| e.to_string())
    }

    #[test]
    fn gpx_track_with_ele_time_and_name_becomes_linestring() {
        let gpx = r#"<?xml version="1.0"?>
<gpx version="1.1"><trk><name>Morning Run</name><desc>Loop</desc><trkseg>
<trkpt lat="52.1" lon="5.1"><ele>10</ele><time>2026-07-01T08:00:00Z</time></trkpt>
<trkpt lat="52.2" lon="5.2"><ele>12</ele><time>2026-07-01T08:05:00Z</time></trkpt>
</trkseg></trk></gpx>"#;
        let fc = geojson(gpx, OutputFormat::GeoJson, true).unwrap();
        assert_eq!(fc["type"], "FeatureCollection");
        let f = &fc["features"][0];
        assert_eq!(f["geometry"]["type"], "LineString");
        assert_eq!(f["geometry"]["coordinates"], json!([[5.1, 52.1, 10.0], [5.2, 52.2, 12.0]]));
        assert_eq!(f["properties"]["name"], "Morning Run");
        assert_eq!(f["properties"]["desc"], "Loop");
        assert_eq!(
            f["properties"]["coordTimes"],
            json!(["2026-07-01T08:00:00Z", "2026-07-01T08:05:00Z"])
        );
    }

    #[test]
    fn gpx_multi_segment_track_becomes_multilinestring() {
        let gpx = r#"<gpx version="1.1"><trk><trkseg>
<trkpt lat="1" lon="1"></trkpt><trkpt lat="2" lon="2"></trkpt>
</trkseg><trkseg>
<trkpt lat="3" lon="3"></trkpt><trkpt lat="4" lon="4"></trkpt>
</trkseg></trk></gpx>"#;
        let fc = geojson(gpx, OutputFormat::GeoJson, true).unwrap();
        let geom = &fc["features"][0]["geometry"];
        assert_eq!(geom["type"], "MultiLineString");
        assert_eq!(geom["coordinates"][0], json!([[1.0, 1.0], [2.0, 2.0]]));
        assert_eq!(geom["coordinates"][1], json!([[3.0, 3.0], [4.0, 4.0]]));
    }

    #[test]
    fn gpx_waypoint_becomes_point() {
        let gpx = r#"<gpx version="1.1"><wpt lat="10" lon="20"><name>Camp</name><ele>100</ele></wpt></gpx>"#;
        let fc = geojson(gpx, OutputFormat::GeoJson, true).unwrap();
        let f = &fc["features"][0];
        assert_eq!(f["geometry"], json!({ "type": "Point", "coordinates": [20.0, 10.0, 100.0] }));
        assert_eq!(f["properties"]["name"], "Camp");
    }

    #[test]
    fn gpx_route_becomes_linestring() {
        let gpx = r#"<gpx version="1.1"><rte><name>Route A</name>
<rtept lat="1" lon="1"/><rtept lat="2" lon="2"/></rte></gpx>"#;
        let fc = geojson(gpx, OutputFormat::GeoJson, true).unwrap();
        let f = &fc["features"][0];
        assert_eq!(f["geometry"]["type"], "LineString");
        assert_eq!(f["properties"]["name"], "Route A");
    }

    #[test]
    fn gpx_with_no_track_route_or_waypoint_errors() {
        let gpx = r#"<gpx version="1.1"><metadata><name>Empty</name></metadata></gpx>"#;
        let err = geojson(gpx, OutputFormat::GeoJson, true).unwrap_err();
        assert!(err.contains("no GPX track"), "unexpected error: {err}");
    }

    #[test]
    fn kml_point_placemark_with_name_and_description() {
        let kml = r#"<?xml version="1.0"?><kml><Document><Placemark>
<name>HQ</name><description>Main office</description>
<Point><coordinates>-122.1,37.4,15</coordinates></Point>
</Placemark></Document></kml>"#;
        let fc = geojson(kml, OutputFormat::GeoJson, true).unwrap();
        let f = &fc["features"][0];
        assert_eq!(f["geometry"], json!({ "type": "Point", "coordinates": [-122.1, 37.4, 15.0] }));
        assert_eq!(f["properties"]["name"], "HQ");
        assert_eq!(f["properties"]["description"], "Main office");
    }

    #[test]
    fn kml_linestring_placemark() {
        let kml = r#"<kml><Placemark><name>Trail</name>
<LineString><coordinates>0,0 1,1 2,2</coordinates></LineString>
</Placemark></kml>"#;
        let fc = geojson(kml, OutputFormat::GeoJson, true).unwrap();
        let geom = &fc["features"][0]["geometry"];
        assert_eq!(geom["type"], "LineString");
        assert_eq!(geom["coordinates"], json!([[0.0, 0.0], [1.0, 1.0], [2.0, 2.0]]));
    }

    #[test]
    fn kml_polygon_with_hole() {
        let kml = r#"<kml><Placemark><Polygon>
<outerBoundaryIs><LinearRing><coordinates>0,0 4,0 4,4 0,4 0,0</coordinates></LinearRing></outerBoundaryIs>
<innerBoundaryIs><LinearRing><coordinates>1,1 2,1 2,2 1,2 1,1</coordinates></LinearRing></innerBoundaryIs>
</Polygon></Placemark></kml>"#;
        let fc = geojson(kml, OutputFormat::GeoJson, true).unwrap();
        let geom = &fc["features"][0]["geometry"];
        assert_eq!(geom["type"], "Polygon");
        assert_eq!(geom["coordinates"].as_array().unwrap().len(), 2);
        assert_eq!(geom["coordinates"][0][0], json!([0.0, 0.0]));
        assert_eq!(geom["coordinates"][1][0], json!([1.0, 1.0]));
    }

    #[test]
    fn kml_multigeometry_becomes_geometrycollection() {
        let kml = r#"<kml><Placemark><MultiGeometry>
<Point><coordinates>1,2</coordinates></Point>
<LineString><coordinates>0,0 1,1</coordinates></LineString>
</MultiGeometry></Placemark></kml>"#;
        let fc = geojson(kml, OutputFormat::GeoJson, true).unwrap();
        let geom = &fc["features"][0]["geometry"];
        assert_eq!(geom["type"], "GeometryCollection");
        assert_eq!(geom["geometries"].as_array().unwrap().len(), 2);
        assert_eq!(geom["geometries"][0]["type"], "Point");
        assert_eq!(geom["geometries"][1]["type"], "LineString");
    }

    #[test]
    fn kml_extended_data_becomes_properties() {
        let kml = r#"<kml><Placemark><Point><coordinates>1,2</coordinates></Point>
<ExtendedData>
<Data name="population"><value>42</value></Data>
<SimpleData name="state">CA</SimpleData>
</ExtendedData>
</Placemark></kml>"#;
        let fc = geojson(kml, OutputFormat::GeoJson, true).unwrap();
        let props = &fc["features"][0]["properties"];
        assert_eq!(props["population"], "42");
        assert_eq!(props["state"], "CA");
    }

    #[test]
    fn kml_inline_style_becomes_simplestyle_properties() {
        let kml = r#"<kml><Placemark>
<Style><LineStyle><color>ff0000ff</color><width>3</width></LineStyle></Style>
<LineString><coordinates>0,0 1,1</coordinates></LineString>
</Placemark></kml>"#;
        let fc = geojson(kml, OutputFormat::GeoJson, true).unwrap();
        let props = &fc["features"][0]["properties"];
        assert_eq!(props["stroke"], "#ff0000");
        assert_eq!(props["stroke-opacity"], 1.0);
        assert_eq!(props["stroke-width"], 3.0);
    }

    #[test]
    fn kml_shared_style_via_styleurl_and_stylemap_resolves() {
        let kml = r#"<kml><Document>
<Style id="lineStyle1"><LineStyle><color>ff00ff00</color></LineStyle></Style>
<StyleMap id="map1"><Pair><key>normal</key><styleUrl>#lineStyle1</styleUrl></Pair>
<Pair><key>highlight</key><styleUrl>#lineStyle1</styleUrl></Pair></StyleMap>
<Placemark><styleUrl>#map1</styleUrl>
<LineString><coordinates>0,0 1,1</coordinates></LineString>
</Placemark>
</Document></kml>"#;
        let fc = geojson(kml, OutputFormat::GeoJson, true).unwrap();
        let props = &fc["features"][0]["properties"];
        assert_eq!(props["stroke"], "#00ff00");
    }

    #[test]
    fn kml_include_styles_false_omits_style_properties() {
        let kml = r#"<kml><Placemark>
<Style><LineStyle><color>ff0000ff</color></LineStyle></Style>
<LineString><coordinates>0,0 1,1</coordinates></LineString>
</Placemark></kml>"#;
        let fc = geojson(kml, OutputFormat::GeoJson, false).unwrap();
        let props = &fc["features"][0]["properties"];
        assert!(props.get("stroke").is_none());
    }

    #[test]
    fn kml_with_no_placemark_geometry_errors() {
        let kml = r#"<kml><Document><name>Empty</name></Document></kml>"#;
        let err = geojson(kml, OutputFormat::GeoJson, true).unwrap_err();
        assert!(err.contains("no KML Placemark"), "unexpected error: {err}");
    }

    #[test]
    fn geojson_point_and_linestring_round_trip_through_gpx() {
        let input = r#"{"type":"FeatureCollection","features":[
            {"type":"Feature","properties":{"name":"Camp"},"geometry":{"type":"Point","coordinates":[20,10,100]}},
            {"type":"Feature","properties":{"name":"Loop"},"geometry":{"type":"LineString","coordinates":[[1,1],[2,2]]}}
        ]}"#;
        let gpx = convert(input, &Options { output_format: OutputFormat::Gpx, include_styles: true }).unwrap();
        assert!(gpx.contains("<wpt lat=\"10\" lon=\"20\">"));
        assert!(gpx.contains("<name>Camp</name>"));
        assert!(gpx.contains("<trk>"));
        assert!(gpx.contains("<trkpt lat=\"1\" lon=\"1\">"));

        // Round-trip: GeoJSON -> GPX -> GeoJSON should preserve the shapes.
        let fc = geojson(&gpx, OutputFormat::GeoJson, true).unwrap();
        let feats = fc["features"].as_array().unwrap();
        assert_eq!(feats.len(), 2);
        assert!(feats.iter().any(|f| f["geometry"]["type"] == "Point" && f["properties"]["name"] == "Camp"));
        assert!(feats.iter().any(|f| f["geometry"]["type"] == "LineString" && f["properties"]["name"] == "Loop"));
    }

    #[test]
    fn geojson_multilinestring_preserves_coord_times_round_trip() {
        let input = r#"{"type":"Feature","properties":{"coordTimes":[["2026-01-01T00:00:00Z","2026-01-01T00:01:00Z"]]},"geometry":{"type":"MultiLineString","coordinates":[[[1,1],[2,2]]]}}"#;
        let gpx = convert(input, &Options { output_format: OutputFormat::Gpx, include_styles: true }).unwrap();
        assert!(gpx.contains("<time>2026-01-01T00:00:00Z</time>"));
        assert!(gpx.contains("<time>2026-01-01T00:01:00Z</time>"));
    }

    #[test]
    fn geojson_polygon_becomes_track_from_exterior_ring() {
        let input = r#"{"type":"Feature","geometry":{"type":"Polygon","coordinates":[[[0,0],[4,0],[4,4],[0,4],[0,0]]]}}"#;
        let gpx = convert(input, &Options { output_format: OutputFormat::Gpx, include_styles: true }).unwrap();
        assert!(gpx.contains("<trk>"));
        assert!(gpx.contains("<trkpt lat=\"0\" lon=\"0\">"));
        assert!(gpx.contains("<trkpt lat=\"4\" lon=\"4\">"));
    }

    #[test]
    fn invalid_geojson_errors() {
        let err = convert("not json", &Options { output_format: OutputFormat::Gpx, include_styles: true }).unwrap_err();
        assert!(err.contains("invalid GeoJSON"), "unexpected error: {err}");
    }

    #[test]
    fn output_format_gpx_on_xml_input_gives_helpful_error() {
        let gpx = r#"<gpx version="1.1"><wpt lat="1" lon="1"/></gpx>"#;
        let err = convert(gpx, &Options { output_format: OutputFormat::Gpx, include_styles: true }).unwrap_err();
        assert!(err.contains("output_format=\"gpx\""), "unexpected error: {err}");
    }

    #[test]
    fn output_format_geojson_on_geojson_input_gives_helpful_error() {
        let input = r#"{"type":"Feature","geometry":{"type":"Point","coordinates":[1,2]}}"#;
        let err = convert(input, &Options { output_format: OutputFormat::GeoJson, include_styles: true }).unwrap_err();
        assert!(err.contains("output_format=\"geojson\""), "unexpected error: {err}");
    }

    #[test]
    fn empty_input_errors() {
        let err = convert("", &Options::default()).unwrap_err();
        assert_eq!(err, "input is empty");
    }
}

