//! KML → GeoJSON Placemark parsing.
//!
//! This module is a copy of the KML half of `blocks/gpx-to-geojson/core`
//! (`kml_to_geojson` and its style/coordinate helpers), kept here so this block
//! builds as a self-contained crate. A cross-block `path` dependency is not an
//! option: `scripts/build-block-wasm.sh` stages ONLY `block-utils` and the block
//! itself into the canonical wasm workspace, so a sibling-block path dependency
//! cannot resolve there (and Cargo hashes those absolute path identities into
//! the artifact, which is what the canonical staging exists to pin).
//!
//! Mapping: a `Placemark`'s `Point`/`LineString`/`Polygon` becomes the matching
//! GeoJSON geometry and `MultiGeometry` becomes a `GeometryCollection`;
//! `name`/`description` become properties; `ExtendedData`/`SimpleData` become
//! arbitrary properties; `TimeSpan`/`TimeStamp` become `begin`/`end`/`time`;
//! and (with `include_styles`) inline or shared `Style`/`styleUrl`/`StyleMap`
//! colors and widths are resolved into simplestyle-spec properties.
//!
//! The caller (`lib.rs`) adds what this block is actually for: KMZ input,
//! `<Folder>` paths, coordinate precision, and the reverse GeoJSON → KML trip.

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
