//! geojson-to-svg core — render GeoJSON features into a standalone SVG map,
//! purely (no tile servers, no network). Shared by the chat skill block and the
//! web page. Parses GeoJSON with serde_json, computes a bounding box over all
//! coordinates, optionally applies a Web-Mercator projection, fits everything
//! into a width x height viewport (preserving aspect ratio, with a margin), and
//! draws each geometry: Polygons filled, LineStrings stroked, Points as circles.

use serde_json::Value;
use std::fmt::Write as _;

/// Rendering options.
pub struct Options {
    /// Output SVG width in px (the height is derived to preserve aspect ratio).
    pub width: f64,
    /// Padding (px) around the drawn map inside the viewport.
    pub padding: f64,
    /// Fill colour for polygons (any CSS colour string).
    pub fill: String,
    /// Stroke colour for polygon/line outlines and point rings.
    pub stroke: String,
    /// Stroke width in px.
    pub stroke_width: f64,
    /// Radius (px) for Point / MultiPoint markers.
    pub point_radius: f64,
    /// Background rectangle colour, or "none"/"transparent" for no background.
    pub background: String,
    /// Apply a spherical Web-Mercator projection (true) or plot raw lon/lat (false).
    pub mercator: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            width: 800.0,
            padding: 16.0,
            fill: "#cfe8ff".into(),
            stroke: "#1f6feb".into(),
            stroke_width: 1.0,
            point_radius: 4.0,
            background: "#ffffff".into(),
            mercator: true,
        }
    }
}

// A single projected ring/line/point list, kept as (x, y) pairs in source units.
type Pts = Vec<(f64, f64)>;

enum Shape {
    Polygon(Vec<Pts>), // outer + holes (rings)
    Line(Pts),
    Point((f64, f64)),
}

/// Project a (lon, lat) pair to planar coordinates. With Web-Mercator, x = lon
/// (in degrees → we keep degrees scaled later) and y uses the standard mercator
/// latitude transform. Without it, we plot lon/lat directly. Latitudes are
/// clamped to the mercator-valid range to avoid infinities at the poles.
fn project(lon: f64, lat: f64, mercator: bool) -> (f64, f64) {
    if mercator {
        let lat = lat.clamp(-85.05112878, 85.05112878);
        let rad = lat.to_radians();
        let y = (std::f64::consts::FRAC_PI_4 + rad / 2.0).tan().ln().to_degrees();
        (lon, y)
    } else {
        (lon, lat)
    }
}

fn coord_pair(v: &Value, mercator: bool) -> Result<(f64, f64), String> {
    let arr = v.as_array().ok_or("position must be an array")?;
    if arr.len() < 2 {
        return Err("position needs at least [lon, lat]".into());
    }
    let lon = arr[0].as_f64().ok_or("longitude is not a number")?;
    let lat = arr[1].as_f64().ok_or("latitude is not a number")?;
    if !lon.is_finite() || !lat.is_finite() {
        return Err("coordinate is not finite".into());
    }
    Ok(project(lon, lat, mercator))
}

fn ring(v: &Value, mercator: bool) -> Result<Pts, String> {
    let arr = v.as_array().ok_or("ring/line must be an array of positions")?;
    arr.iter().map(|p| coord_pair(p, mercator)).collect()
}

fn rings(v: &Value, mercator: bool) -> Result<Vec<Pts>, String> {
    let arr = v.as_array().ok_or("polygon must be an array of rings")?;
    arr.iter().map(|r| ring(r, mercator)).collect()
}

/// Walk a GeoJSON geometry object, appending the shapes it produces.
fn collect_geometry(geom: &Value, mercator: bool, out: &mut Vec<Shape>) -> Result<(), String> {
    let ty = geom
        .get("type")
        .and_then(|t| t.as_str())
        .ok_or("geometry has no \"type\"")?;
    let coords = geom.get("coordinates");
    match ty {
        "Point" => {
            let c = coords.ok_or("Point has no coordinates")?;
            out.push(Shape::Point(coord_pair(c, mercator)?));
        }
        "MultiPoint" => {
            for p in coords
                .and_then(|c| c.as_array())
                .ok_or("MultiPoint coordinates must be an array")?
            {
                out.push(Shape::Point(coord_pair(p, mercator)?));
            }
        }
        "LineString" => {
            out.push(Shape::Line(ring(coords.ok_or("LineString has no coordinates")?, mercator)?));
        }
        "MultiLineString" => {
            for l in coords
                .and_then(|c| c.as_array())
                .ok_or("MultiLineString coordinates must be an array")?
            {
                out.push(Shape::Line(ring(l, mercator)?));
            }
        }
        "Polygon" => {
            out.push(Shape::Polygon(rings(coords.ok_or("Polygon has no coordinates")?, mercator)?));
        }
        "MultiPolygon" => {
            for poly in coords
                .and_then(|c| c.as_array())
                .ok_or("MultiPolygon coordinates must be an array")?
            {
                out.push(Shape::Polygon(rings(poly, mercator)?));
            }
        }
        "GeometryCollection" => {
            for g in geom
                .get("geometries")
                .and_then(|g| g.as_array())
                .ok_or("GeometryCollection has no \"geometries\"")?
            {
                collect_geometry(g, mercator, out)?;
            }
        }
        other => return Err(format!("unsupported geometry type \"{other}\"")),
    }
    Ok(())
}

/// Top-level: a FeatureCollection, a Feature, or a bare geometry.
fn collect_root(root: &Value, mercator: bool, out: &mut Vec<Shape>) -> Result<(), String> {
    let ty = root
        .get("type")
        .and_then(|t| t.as_str())
        .ok_or("input is not a GeoJSON object (no \"type\")")?;
    match ty {
        "FeatureCollection" => {
            for f in root
                .get("features")
                .and_then(|f| f.as_array())
                .ok_or("FeatureCollection has no \"features\" array")?
            {
                if let Some(g) = f.get("geometry") {
                    if !g.is_null() {
                        collect_geometry(g, mercator, out)?;
                    }
                }
            }
        }
        "Feature" => {
            if let Some(g) = root.get("geometry") {
                if !g.is_null() {
                    collect_geometry(g, mercator, out)?;
                }
            }
        }
        _ => collect_geometry(root, mercator, out)?,
    }
    Ok(())
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('"', "&quot;")
}

fn num(n: f64) -> String {
    // Compact, deterministic numbers: round to 2 dp, trim trailing zeros.
    let r = (n * 100.0).round() / 100.0;
    let s = format!("{r:.2}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-0" {
        "0".into()
    } else {
        s.to_string()
    }
}

/// Render GeoJSON text into a standalone SVG document string.
pub fn render(geojson: &str, opt: &Options) -> Result<String, String> {
    if geojson.trim().is_empty() {
        return Err("geojson is empty".into());
    }
    let width = opt.width;
    if !(16.0..=4096.0).contains(&width) {
        return Err("width must be between 16 and 4096".into());
    }
    let root: Value = serde_json::from_str(geojson).map_err(|e| format!("invalid JSON: {e}"))?;

    let mut shapes = Vec::new();
    collect_root(&root, opt.mercator, &mut shapes)?;
    if shapes.is_empty() {
        return Err("no drawable geometry found in the GeoJSON".into());
    }

    // Bounding box over every projected coordinate.
    let (mut minx, mut miny, mut maxx, mut maxy) =
        (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    let mut consider = |(x, y): (f64, f64)| {
        if x < minx { minx = x }
        if y < miny { miny = y }
        if x > maxx { maxx = x }
        if y > maxy { maxy = y }
    };
    for s in &shapes {
        match s {
            Shape::Point(p) => consider(*p),
            Shape::Line(l) => l.iter().for_each(|p| consider(*p)),
            Shape::Polygon(rs) => rs.iter().for_each(|r| r.iter().for_each(|p| consider(*p))),
        }
    }
    if !minx.is_finite() {
        return Err("could not compute a bounding box (no coordinates)".into());
    }

    let pad = opt.padding.max(0.0);
    let mut span_x = maxx - minx;
    let mut span_y = maxy - miny;
    // Degenerate (single point or vertical/horizontal line): give it room and
    // re-center the data so it lands in the middle of the padded box.
    if span_x <= 0.0 {
        span_x = 1.0;
        minx -= 0.5;
        maxx += 0.5;
    }
    if span_y <= 0.0 {
        span_y = 1.0;
        miny -= 0.5;
        maxy += 0.5;
    }

    let inner_w = (width - 2.0 * pad).max(1.0);
    // Preserve aspect ratio: scale by the limiting dimension. Derive height from
    // the data aspect so the whole map fits with equal scale on both axes.
    let scale = inner_w / span_x;
    let inner_h = span_y * scale;
    let height = inner_h + 2.0 * pad;

    // SVG y grows downward, GeoJSON/mercator y grows upward → flip.
    let tx = |x: f64| pad + (x - minx) * scale;
    let ty = |y: f64| pad + (maxy - y) * scale;

    let mut svg = String::new();
    write!(
        svg,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\">",
        num(width),
        num(height),
        num(width),
        num(height),
    )
    .unwrap();

    let bg = opt.background.trim();
    if !bg.is_empty() && bg != "none" && bg != "transparent" {
        write!(
            svg,
            "<rect width=\"{}\" height=\"{}\" fill=\"{}\"/>",
            num(width),
            num(height),
            esc(bg)
        )
        .unwrap();
    }

    let fill = esc(opt.fill.trim());
    let stroke = esc(opt.stroke.trim());
    let sw = num(opt.stroke_width.max(0.0));
    let r = num(opt.point_radius.max(0.1));

    write!(
        svg,
        "<g fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"{sw}\" \
         stroke-linejoin=\"round\" stroke-linecap=\"round\">"
    )
    .unwrap();

    let path_d = |pts: &Pts, close: bool| -> String {
        let mut d = String::new();
        for (i, (x, y)) in pts.iter().enumerate() {
            let cmd = if i == 0 { 'M' } else { 'L' };
            write!(d, "{cmd}{} {}", num(tx(*x)), num(ty(*y))).unwrap();
            if i + 1 < pts.len() {
                d.push(' ');
            }
        }
        if close {
            d.push('Z');
        }
        d
    };

    for s in &shapes {
        match s {
            Shape::Polygon(rs) => {
                let mut d = String::new();
                for ring in rs {
                    if ring.len() >= 2 {
                        d.push_str(&path_d(ring, true));
                    }
                }
                if !d.is_empty() {
                    // fill-rule evenodd renders holes (inner rings) as cutouts.
                    write!(svg, "<path fill-rule=\"evenodd\" d=\"{d}\"/>").unwrap();
                }
            }
            Shape::Line(l) => {
                if l.len() >= 2 {
                    write!(svg, "<path fill=\"none\" d=\"{}\"/>", path_d(l, false)).unwrap();
                }
            }
            Shape::Point(p) => {
                write!(
                    svg,
                    "<circle cx=\"{}\" cy=\"{}\" r=\"{r}\"/>",
                    num(tx(p.0)),
                    num(ty(p.1))
                )
                .unwrap();
            }
        }
    }

    svg.push_str("</g></svg>");
    Ok(svg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_polygon_feature_collection() {
        let gj = r#"{
          "type": "FeatureCollection",
          "features": [
            {"type":"Feature","geometry":{"type":"Polygon","coordinates":[[[0,0],[10,0],[10,10],[0,10],[0,0]]]},"properties":{}}
          ]
        }"#;
        let svg = render(gj, &Options::default()).unwrap();
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("</svg>"));
        assert!(svg.contains("<path"));
        assert!(svg.contains("fill-rule=\"evenodd\""));
    }

    #[test]
    fn renders_point_as_circle() {
        let gj = r#"{"type":"Point","coordinates":[5,5]}"#;
        let svg = render(gj, &Options::default()).unwrap();
        assert!(svg.contains("<circle"));
    }

    #[test]
    fn renders_linestring() {
        let gj = r#"{"type":"LineString","coordinates":[[0,0],[5,5],[10,0]]}"#;
        let svg = render(gj, &Options::default()).unwrap();
        assert!(svg.contains("<path fill=\"none\""));
    }

    #[test]
    fn handles_multipolygon_and_geometrycollection() {
        let gj = r#"{"type":"GeometryCollection","geometries":[
            {"type":"MultiPolygon","coordinates":[[[[0,0],[1,0],[1,1],[0,0]]]]},
            {"type":"MultiPoint","coordinates":[[2,2],[3,3]]}
        ]}"#;
        let svg = render(gj, &Options::default()).unwrap();
        assert!(svg.contains("<path"));
        assert!(svg.matches("<circle").count() == 2);
    }

    #[test]
    fn no_mercator_plots_raw() {
        let gj = r#"{"type":"LineString","coordinates":[[0,0],[0,80]]}"#;
        let merc = render(gj, &Options { mercator: true, ..Default::default() }).unwrap();
        let raw = render(gj, &Options { mercator: false, ..Default::default() }).unwrap();
        // Mercator stretches high latitudes, so the rendered heights differ.
        assert_ne!(merc, raw);
    }

    #[test]
    fn background_none_omits_rect() {
        let gj = r#"{"type":"Point","coordinates":[1,1]}"#;
        let svg = render(gj, &Options { background: "none".into(), ..Default::default() }).unwrap();
        assert!(!svg.contains("<rect"));
    }

    #[test]
    fn errors_on_invalid_json() {
        assert!(render("not json", &Options::default()).is_err());
    }

    #[test]
    fn errors_on_empty() {
        assert!(render("   ", &Options::default()).is_err());
    }

    #[test]
    fn errors_on_no_geometry() {
        let gj = r#"{"type":"FeatureCollection","features":[]}"#;
        assert!(render(gj, &Options::default()).is_err());
    }

    #[test]
    fn errors_on_unsupported_geometry() {
        let gj = r#"{"type":"Triangle","coordinates":[]}"#;
        assert!(render(gj, &Options::default()).is_err());
    }
}
