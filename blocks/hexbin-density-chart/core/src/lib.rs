//! gizza-ai/hexbin-density-chart core — bin x/y point data into a hexagonal
//! grid and render the per-hex density as a heatmap (sequential colour ramp),
//! output as SVG or PNG. Pure-Rust: the SVG is hand-built, and PNG is produced
//! by rasterizing that SVG with resvg/tiny-skia (the same fs-free usvg setup as
//! the svg-to-png tool), so it instantiates in the wafer chat Service Worker.
//!
//! Input is JSON: an array of points, each either `[x, y]` or `{"x":..,"y":..}`.
//! Points are dropped into a flat-top hexagonal lattice in the plot area; each
//! occupied hexagon is drawn filled by its point count using a Viridis-like
//! sequential colour scale, with axes (min/mid/max ticks) and a count legend.

use serde::Deserialize;

/// Output image format.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OutputFormat {
    Svg,
    Png,
}

impl OutputFormat {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "svg" | "" => Ok(OutputFormat::Svg),
            "png" => Ok(OutputFormat::Png),
            other => Err(format!("unknown format '{other}' (expected 'svg' or 'png')")),
        }
    }
    pub fn extension(self) -> &'static str {
        match self {
            OutputFormat::Svg => "svg",
            OutputFormat::Png => "png",
        }
    }
    pub fn mime(self) -> &'static str {
        match self {
            OutputFormat::Svg => "image/svg+xml",
            OutputFormat::Png => "image/png",
        }
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawPoint {
    Pair([f64; 2]),
    Obj { x: f64, y: f64 },
}

struct Point {
    x: f64,
    y: f64,
}

fn parse_points(data: &str) -> Result<Vec<Point>, String> {
    let raw: Vec<RawPoint> = serde_json::from_str(data.trim()).map_err(|e| {
        format!("`data` must be a JSON array of [x,y] pairs or {{\"x\":..,\"y\":..}} objects: {e}")
    })?;
    if raw.is_empty() {
        return Err("`data` is empty — provide at least one point".into());
    }
    let pts: Vec<Point> = raw
        .into_iter()
        .map(|r| match r {
            RawPoint::Pair([x, y]) => Point { x, y },
            RawPoint::Obj { x, y } => Point { x, y },
        })
        .collect();
    if pts.iter().any(|p| !p.x.is_finite() || !p.y.is_finite()) {
        return Err("all x and y values must be finite numbers".into());
    }
    Ok(pts)
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Format a number for an axis label: integer if whole, else up to 3 decimals.
fn fmt_num(v: f64) -> String {
    if (v.round() - v).abs() < 1e-9 {
        format!("{}", v.round() as i64)
    } else {
        let s = format!("{v:.3}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// A small Viridis-like sequential colour ramp (perceptually-ordered, dark→bright).
/// `t` in [0,1] → "#rrggbb".
fn viridis(t: f64) -> String {
    const STOPS: [(u8, u8, u8); 6] = [
        (68, 1, 84),    // 0.0 dark purple
        (59, 82, 139),  // 0.2 blue
        (33, 145, 140), // 0.4 teal
        (94, 201, 98),  // 0.6 green
        (173, 220, 48), // 0.8 yellow-green
        (253, 231, 37), // 1.0 yellow
    ];
    let t = t.clamp(0.0, 1.0);
    let seg = (STOPS.len() - 1) as f64;
    let pos = t * seg;
    let i = (pos.floor() as usize).min(STOPS.len() - 2);
    let f = pos - i as f64;
    let (r0, g0, b0) = STOPS[i];
    let (r1, g1, b1) = STOPS[i + 1];
    let lerp = |a: u8, b: u8| (a as f64 + (b as f64 - a as f64) * f).round() as u8;
    format!("#{:02x}{:02x}{:02x}", lerp(r0, r1), lerp(g0, g1), lerp(b0, b1))
}

/// One occupied hexagon: its centre (pixel coords) and its point count.
struct Bin {
    cx: f64,
    cy: f64,
    count: u32,
}

/// Build the SVG string for the hexbin density chart.
///
/// `radius` is the hexagon circumradius in pixels (clamped 4..=120); a larger
/// radius means coarser bins. The hex lattice is flat-top, laid over the plot
/// area; each point is assigned to the hexagon whose centre is nearest (within
/// the flat-top offset lattice).
pub fn render_svg(
    data: &str,
    title: &str,
    width: u32,
    height: u32,
    radius: f64,
) -> Result<String, String> {
    let pts = parse_points(data)?;
    let width = width.clamp(200, 4000) as f64;
    let height = height.clamp(150, 4000) as f64;
    let r = radius.clamp(4.0, 120.0);

    // Margins (room for a colour legend on the right).
    let m_left = 60.0;
    let m_bottom = 44.0;
    let m_top = if title.is_empty() { 24.0 } else { 48.0 };
    let m_right = 96.0;
    let plot_w = (width - m_left - m_right).max(20.0);
    let plot_h = (height - m_top - m_bottom).max(20.0);

    // Data bounds (pad so points aren't on the edge).
    let (mut xmin, mut xmax) = (f64::INFINITY, f64::NEG_INFINITY);
    let (mut ymin, mut ymax) = (f64::INFINITY, f64::NEG_INFINITY);
    for p in &pts {
        xmin = xmin.min(p.x);
        xmax = xmax.max(p.x);
        ymin = ymin.min(p.y);
        ymax = ymax.max(p.y);
    }
    if (xmax - xmin).abs() < 1e-12 {
        xmin -= 1.0;
        xmax += 1.0;
    }
    if (ymax - ymin).abs() < 1e-12 {
        ymin -= 1.0;
        ymax += 1.0;
    }
    let xpad = (xmax - xmin) * 0.05;
    let ypad = (ymax - ymin) * 0.05;
    let (xlo, xhi) = (xmin - xpad, xmax + xpad);
    let (ylo, yhi) = (ymin - ypad, ymax + ypad);

    // Map data → pixels (y inverted: data up = screen up).
    let sx = |x: f64| m_left + (x - xlo) / (xhi - xlo) * plot_w;
    let sy = |y: f64| m_top + plot_h - (y - ylo) / (yhi - ylo) * plot_h;

    // Flat-top hex lattice geometry. For a flat-top hexagon of circumradius r:
    //   horizontal spacing between column centres = 1.5 * r
    //   vertical spacing between rows             = sqrt(3) * r
    //   odd columns are shifted down by half a row (sqrt(3)/2 * r).
    let hx = 1.5 * r;
    let hy = 3.0_f64.sqrt() * r;

    // Bin every point. Assign to the nearest of the (col, col±1) candidate
    // centres so points near a column boundary land in the right hex.
    use std::collections::HashMap;
    let mut counts: HashMap<(i64, i64), u32> = HashMap::new();
    for p in &pts {
        let px = sx(p.x) - m_left;
        let py = sy(p.y) - m_top;
        // Nominal column from x.
        let col_f = px / hx;
        let mut best: Option<((i64, i64), f64)> = None;
        for dc in -1..=1 {
            let col = (col_f.round() as i64) + dc;
            let cxp = col as f64 * hx;
            let yoff = if col.rem_euclid(2) == 1 { hy / 2.0 } else { 0.0 };
            let row = ((py - yoff) / hy).round() as i64;
            let cyp = row as f64 * hy + yoff;
            let d2 = (px - cxp).powi(2) + (py - cyp).powi(2);
            if best.map_or(true, |(_, bd)| d2 < bd) {
                best = Some(((col, row), d2));
            }
        }
        if let Some((key, _)) = best {
            *counts.entry(key).or_insert(0) += 1;
        }
    }

    // Resolve bins to pixel centres + counts.
    let mut bins: Vec<Bin> = counts
        .iter()
        .map(|(&(col, row), &count)| {
            let yoff = if col.rem_euclid(2) == 1 { hy / 2.0 } else { 0.0 };
            Bin {
                cx: m_left + col as f64 * hx,
                cy: m_top + row as f64 * hy + yoff,
                count,
            }
        })
        .collect();
    // Stable draw order (low counts first so dense hexes sit on top).
    bins.sort_by(|a, b| a.count.cmp(&b.count));

    let max_count = bins.iter().map(|b| b.count).max().unwrap_or(1).max(1);

    // Flat-top hexagon vertex offsets (angles 0,60,...,300 degrees).
    let hex_path = |cx: f64, cy: f64| -> String {
        let mut pts = String::new();
        for k in 0..6 {
            let ang = std::f64::consts::PI / 3.0 * k as f64; // 0,60,... flat-top
            let vx = cx + r * ang.cos();
            let vy = cy + r * ang.sin();
            if k > 0 {
                pts.push(' ');
            }
            pts.push_str(&format!("{vx:.1},{vy:.1}"));
        }
        pts
    };

    let mut s = String::new();
    s.push_str(&format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}" font-family="sans-serif">"##,
        w = width as i64,
        h = height as i64
    ));
    s.push_str(&format!(
        r##"<rect width="{}" height="{}" fill="#ffffff"/>"##,
        width as i64, height as i64
    ));
    if !title.is_empty() {
        s.push_str(&format!(
            r##"<text x="{x}" y="28" text-anchor="middle" font-size="18" font-weight="bold" fill="#222">{t}</text>"##,
            x = (m_left + plot_w / 2.0) as i64,
            t = esc(title)
        ));
    }

    // Clip the hexes to the plot rectangle so partial hexes don't spill out.
    s.push_str(&format!(
        r##"<defs><clipPath id="plot"><rect x="{x}" y="{y}" width="{w}" height="{h}"/></clipPath></defs>"##,
        x = m_left as i64,
        y = m_top as i64,
        w = plot_w as i64,
        h = plot_h as i64
    ));
    s.push_str(r##"<g clip-path="url(#plot)">"##);
    for b in &bins {
        // Normalise count → [0,1] for the colour ramp (linear scale).
        let t = if max_count > 1 {
            (b.count as f64 - 1.0) / (max_count as f64 - 1.0)
        } else {
            1.0
        };
        s.push_str(&format!(
            r##"<polygon points="{pts}" fill="{c}" stroke="#ffffff" stroke-width="0.5"/>"##,
            pts = hex_path(b.cx, b.cy),
            c = viridis(t)
        ));
    }
    s.push_str("</g>");

    // Axes.
    let ax_x0 = m_left;
    let ax_y0 = m_top + plot_h;
    s.push_str(&format!(
        r##"<line x1="{x0}" y1="{y0}" x2="{x1}" y2="{y0}" stroke="#888" stroke-width="1"/>"##,
        x0 = ax_x0 as i64,
        y0 = ax_y0 as i64,
        x1 = (m_left + plot_w) as i64
    ));
    s.push_str(&format!(
        r##"<line x1="{x0}" y1="{y0}" x2="{x0}" y2="{y1}" stroke="#888" stroke-width="1"/>"##,
        x0 = ax_x0 as i64,
        y0 = ax_y0 as i64,
        y1 = m_top as i64
    ));

    // Tick labels (min / mid / max on each axis).
    for f in [0.0_f64, 0.5, 1.0] {
        let xv = xlo + (xhi - xlo) * f;
        let px = sx(xv);
        s.push_str(&format!(
            r##"<text x="{px}" y="{ty}" text-anchor="middle" font-size="11" fill="#555">{lbl}</text>"##,
            px = px as i64,
            ty = (ax_y0 + 16.0) as i64,
            lbl = esc(&fmt_num(xv))
        ));
        let yv = ylo + (yhi - ylo) * f;
        let py = sy(yv);
        s.push_str(&format!(
            r##"<text x="{tx}" y="{py}" text-anchor="end" font-size="11" fill="#555" dy="4">{lbl}</text>"##,
            tx = (ax_x0 - 8.0) as i64,
            py = py as i64,
            lbl = esc(&fmt_num(yv))
        ));
    }

    // Colour legend (a vertical gradient bar from 1 → max count).
    let lx = m_left + plot_w + 20.0;
    let lw = 14.0;
    let lh = plot_h.min(220.0);
    let ly0 = m_top;
    let steps = 24;
    for i in 0..steps {
        let frac = i as f64 / (steps - 1) as f64; // 0 bottom .. 1 top after invert
        let seg_y = ly0 + (1.0 - frac) * (lh - lh / steps as f64);
        s.push_str(&format!(
            r##"<rect x="{x}" y="{y:.1}" width="{w:.0}" height="{hh:.1}" fill="{c}"/>"##,
            x = lx as i64,
            y = seg_y,
            w = lw,
            hh = lh / steps as f64 + 1.0,
            c = viridis(frac)
        ));
    }
    s.push_str(&format!(
        r##"<text x="{tx}" y="{ty}" font-size="10" fill="#555">{m}</text>"##,
        tx = (lx + lw + 4.0) as i64,
        ty = (ly0 + 8.0) as i64,
        m = max_count
    ));
    s.push_str(&format!(
        r##"<text x="{tx}" y="{ty}" font-size="10" fill="#555">1</text>"##,
        tx = (lx + lw + 4.0) as i64,
        ty = (ly0 + lh) as i64
    ));

    s.push_str("</svg>");
    Ok(s)
}

/// Rasterize an SVG document into PNG bytes using resvg/tiny-skia. The usvg
/// `Options` are built field-by-field (NOT via `Options::default()`) and the
/// image-href resolver is replaced so no filesystem WASI imports are linked —
/// the same fs-free construction the svg-to-png tool relies on to instantiate
/// in the wafer runtime.
pub fn rasterize_png(svg: &str) -> Result<Vec<u8>, String> {
    let usvg_opts = usvg::Options {
        resources_dir: None,
        dpi: 96.0,
        font_family: "Times New Roman".to_string(),
        font_size: 12.0,
        languages: vec!["en".to_string()],
        shape_rendering: usvg::ShapeRendering::default(),
        text_rendering: usvg::TextRendering::default(),
        image_rendering: usvg::ImageRendering::default(),
        default_size: usvg::Size::from_wh(100.0, 100.0).unwrap(),
        image_href_resolver: usvg::ImageHrefResolver {
            resolve_data: Box::new(
                |mime: &str, data: std::sync::Arc<Vec<u8>>, _opts: &usvg::Options| match mime
                    .to_ascii_lowercase()
                    .as_str()
                {
                    "image/jpg" | "image/jpeg" => Some(usvg::ImageKind::JPEG(data)),
                    "image/png" => Some(usvg::ImageKind::PNG(data)),
                    "image/gif" => Some(usvg::ImageKind::GIF(data)),
                    "image/webp" => Some(usvg::ImageKind::WEBP(data)),
                    _ => None,
                },
            ),
            resolve_string: Box::new(|_href: &str, _opts: &usvg::Options| None),
        },
        style_sheet: None,
    };

    let tree =
        usvg::Tree::from_str(svg, &usvg_opts).map_err(|e| format!("could not parse SVG: {e}"))?;
    let size = tree.size();
    let out_w = size.width().round().max(1.0) as u32;
    let out_h = size.height().round().max(1.0) as u32;
    let mut pixmap = tiny_skia::Pixmap::new(out_w, out_h)
        .ok_or_else(|| format!("could not allocate a {out_w}x{out_h} canvas"))?;
    // White background (the SVG already fills white, but be safe).
    pixmap.fill(tiny_skia::Color::WHITE);
    resvg::render(&tree, tiny_skia::Transform::identity(), &mut pixmap.as_mut());
    pixmap
        .encode_png()
        .map_err(|e| format!("could not encode PNG: {e}"))
}

/// Top-level render: produce SVG text bytes or rasterized PNG bytes.
pub fn render(
    data: &str,
    title: &str,
    width: u32,
    height: u32,
    radius: f64,
    format: OutputFormat,
) -> Result<Vec<u8>, String> {
    let svg = render_svg(data, title, width, height, radius)?;
    match format {
        OutputFormat::Svg => Ok(svg.into_bytes()),
        OutputFormat::Png => rasterize_png(&svg),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_pairs_svg() {
        let svg = render_svg("[[1,2],[3,4],[5,1],[2,2],[3,3]]", "", 600, 400, 18.0).unwrap();
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        // at least one occupied hexagon polygon
        assert!(svg.contains("<polygon"));
        assert!(svg.contains(r##"width="600""##));
    }

    #[test]
    fn dense_cluster_makes_a_hot_hex() {
        // Many points in the same spot collapse to one (or few) hexes.
        let mut v = Vec::new();
        for _ in 0..50 {
            v.push("[5,5]");
        }
        // plus a couple of sparse points
        v.push("[0,0]");
        v.push("[10,10]");
        let data = format!("[{}]", v.join(","));
        let svg = render_svg(&data, "Density", 500, 400, 16.0).unwrap();
        // The brightest viridis stop should appear for the densest hex.
        assert!(svg.contains("#fde725")); // top-of-ramp yellow
        assert!(svg.contains("Density"));
    }

    #[test]
    fn title_and_legend_present() {
        let svg = render_svg("[[0,0],[1,1],[2,2]]", "T", 500, 400, 20.0).unwrap();
        assert!(svg.contains(">T<"));
        // legend draws the "1" minimum label
        assert!(svg.contains(">1<"));
    }

    #[test]
    fn axis_labels_present() {
        let svg = render_svg("[[0,0],[10,100]]", "", 500, 400, 20.0).unwrap();
        // 3 ticks per axis = 6 tick labels (font-size 11).
        assert_eq!(svg.matches("font-size=\"11\"").count(), 6);
    }

    #[test]
    fn radius_clamped() {
        // radius below the floor should still render (clamped to 4).
        let svg = render_svg("[[1,1],[2,2]]", "", 400, 400, 0.5).unwrap();
        assert!(svg.contains("<polygon"));
    }

    #[test]
    fn png_output_is_a_png() {
        let png = render("[[1,1],[2,2],[3,3],[1,2],[2,1]]", "", 400, 400, 20.0, OutputFormat::Png)
            .unwrap();
        // PNG magic number.
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        assert!(png.len() > 100);
    }

    #[test]
    fn format_parse() {
        assert_eq!(OutputFormat::parse("svg").unwrap(), OutputFormat::Svg);
        assert_eq!(OutputFormat::parse("PNG").unwrap(), OutputFormat::Png);
        assert_eq!(OutputFormat::parse("").unwrap(), OutputFormat::Svg);
        assert!(OutputFormat::parse("gif").is_err());
    }

    #[test]
    fn errors() {
        assert!(render_svg("", "", 400, 400, 20.0).is_err());
        assert!(render_svg("[]", "", 400, 400, 20.0).is_err());
        assert!(render_svg("not json", "", 400, 400, 20.0).is_err());
        assert!(render_svg("[[1]]", "", 400, 400, 20.0).is_err()); // not a pair
        assert!(render_svg("[[1,\"x\"]]", "", 400, 400, 20.0).is_err());
    }
}
