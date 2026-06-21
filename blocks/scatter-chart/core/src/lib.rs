//! gizza-ai/scatter-chart core — render a scatter plot from x/y (and optional
//! category/size) data as an SVG chart. Pure-Rust (hand-built SVG, no drawing
//! deps). No wafer/wasm-bindgen deps.
//!
//! Input is JSON: an array of points, each either `[x, y]` or
//! `{"x":..,"y":..,"category":"..","size":..}`. Points are coloured by category
//! (with a legend) and sized by `size` when present.

use serde::Deserialize;

const PALETTE: [&str; 8] = [
    "#4e79a7", "#f28e2b", "#59a14f", "#e15759", "#76b7b2", "#edc948", "#b07aa1", "#ff9da7",
];

#[derive(Deserialize)]
#[serde(untagged)]
enum RawPoint {
    Pair([f64; 2]),
    Obj {
        x: f64,
        y: f64,
        #[serde(default)]
        category: Option<String>,
        #[serde(default)]
        size: Option<f64>,
    },
}

struct Point {
    x: f64,
    y: f64,
    category: Option<String>,
    size: Option<f64>,
}

fn parse_points(data: &str) -> Result<Vec<Point>, String> {
    let raw: Vec<RawPoint> = serde_json::from_str(data.trim())
        .map_err(|e| format!("`data` must be a JSON array of [x,y] pairs or {{x,y,...}} objects: {e}"))?;
    if raw.is_empty() {
        return Err("`data` is empty — provide at least one point".into());
    }
    let pts: Vec<Point> = raw
        .into_iter()
        .map(|r| match r {
            RawPoint::Pair([x, y]) => Point { x, y, category: None, size: None },
            RawPoint::Obj { x, y, category, size } => Point { x, y, category, size },
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

/// Render the scatter plot to an SVG string.
pub fn render_svg(data: &str, title: &str, width: u32, height: u32) -> Result<String, String> {
    let pts = parse_points(data)?;
    let width = width.clamp(200, 4000) as f64;
    let height = height.clamp(150, 4000) as f64;

    // Distinct categories in first-seen order.
    let mut cats: Vec<String> = Vec::new();
    for p in &pts {
        if let Some(c) = &p.category {
            if !cats.contains(c) {
                cats.push(c.clone());
            }
        }
    }
    let has_legend = !cats.is_empty();

    // Margins.
    let m_left = 60.0;
    let m_bottom = 44.0;
    let m_top = if title.is_empty() { 24.0 } else { 48.0 };
    let m_right = if has_legend { 140.0 } else { 24.0 };
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

    let sx = |x: f64| m_left + (x - xlo) / (xhi - xlo) * plot_w;
    let sy = |y: f64| m_top + plot_h - (y - ylo) / (yhi - ylo) * plot_h;

    // Size scaling.
    let sizes: Vec<f64> = pts.iter().filter_map(|p| p.size).collect();
    let (smin, smax) = if sizes.is_empty() {
        (0.0, 0.0)
    } else {
        (
            sizes.iter().cloned().fold(f64::INFINITY, f64::min),
            sizes.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        )
    };
    let radius = |p: &Point| -> f64 {
        match p.size {
            Some(s) if smax > smin => 3.0 + (s - smin) / (smax - smin) * 11.0,
            _ => 4.5,
        }
    };
    let color = |p: &Point| -> &str {
        match &p.category {
            Some(c) => {
                let i = cats.iter().position(|x| x == c).unwrap_or(0);
                PALETTE[i % PALETTE.len()]
            }
            None => PALETTE[0],
        }
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

    // Gridlines + tick labels (min / mid / max on each axis).
    for f in [0.0_f64, 0.5, 1.0] {
        let xv = xlo + (xhi - xlo) * f;
        let px = sx(xv);
        s.push_str(&format!(
            r##"<line x1="{px}" y1="{y0}" x2="{px}" y2="{y1}" stroke="#eee" stroke-width="1"/>"##,
            px = px as i64,
            y0 = ax_y0 as i64,
            y1 = m_top as i64
        ));
        s.push_str(&format!(
            r##"<text x="{px}" y="{ty}" text-anchor="middle" font-size="11" fill="#555">{lbl}</text>"##,
            px = px as i64,
            ty = (ax_y0 + 16.0) as i64,
            lbl = esc(&fmt_num(xv))
        ));
        let yv = ylo + (yhi - ylo) * f;
        let py = sy(yv);
        s.push_str(&format!(
            r##"<line x1="{x0}" y1="{py}" x2="{x1}" y2="{py}" stroke="#eee" stroke-width="1"/>"##,
            x0 = ax_x0 as i64,
            x1 = (m_left + plot_w) as i64,
            py = py as i64
        ));
        s.push_str(&format!(
            r##"<text x="{tx}" y="{py}" text-anchor="end" font-size="11" fill="#555" dy="4">{lbl}</text>"##,
            tx = (ax_x0 - 8.0) as i64,
            py = py as i64,
            lbl = esc(&fmt_num(yv))
        ));
    }

    // Points.
    for p in &pts {
        s.push_str(&format!(
            r##"<circle cx="{cx:.1}" cy="{cy:.1}" r="{r:.1}" fill="{c}" fill-opacity="0.72" stroke="#ffffff" stroke-width="0.6"/>"##,
            cx = sx(p.x),
            cy = sy(p.y),
            r = radius(p),
            c = color(p)
        ));
    }

    // Legend.
    if has_legend {
        let lx = m_left + plot_w + 16.0;
        let mut ly = m_top + 6.0;
        for (i, c) in cats.iter().enumerate() {
            let col = PALETTE[i % PALETTE.len()];
            s.push_str(&format!(
                r##"<circle cx="{cx}" cy="{cy}" r="6" fill="{col}"/>"##,
                cx = (lx + 6.0) as i64,
                cy = ly as i64
            ));
            s.push_str(&format!(
                r##"<text x="{tx}" y="{ty}" font-size="12" fill="#333">{lbl}</text>"##,
                tx = (lx + 18.0) as i64,
                ty = (ly + 4.0) as i64,
                lbl = esc(c)
            ));
            ly += 20.0;
        }
    }

    s.push_str("</svg>");
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_pairs() {
        let svg = render_svg("[[1,2],[3,4],[5,1]]", "", 600, 400).unwrap();
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        assert_eq!(svg.matches("<circle").count(), 3); // one per point, no legend
        assert!(svg.contains(r##"width="600""##));
    }

    #[test]
    fn objects_with_categories_get_legend_and_colors() {
        let data = r##"[{"x":1,"y":2,"category":"a"},{"x":2,"y":3,"category":"b"},{"x":3,"y":1,"category":"a"}]"##;
        let svg = render_svg(data, "Plot", 700, 500).unwrap();
        // 3 data points + 2 legend swatches = 5 circles.
        assert_eq!(svg.matches("<circle").count(), 5);
        assert!(svg.contains(">a<"));
        assert!(svg.contains(">b<"));
        assert!(svg.contains("Plot"));
        // two distinct palette colors used
        assert!(svg.contains(PALETTE[0]));
        assert!(svg.contains(PALETTE[1]));
    }

    #[test]
    fn size_scales_radius() {
        let data = r##"[{"x":0,"y":0,"size":1},{"x":1,"y":1,"size":100}]"##;
        let svg = render_svg(data, "", 400, 400).unwrap();
        // smallest -> r=3.0, largest -> r=14.0
        assert!(svg.contains(r##"r="3.0""##));
        assert!(svg.contains(r##"r="14.0""##));
    }

    #[test]
    fn axis_labels_present() {
        let svg = render_svg("[[0,0],[10,100]]", "", 500, 400).unwrap();
        // 3 ticks per axis = 6 tick labels (font-size 11).
        assert_eq!(svg.matches("font-size=\"11\"").count(), 6);
        // x mid tick = midpoint of padded bounds = 5.
        assert!(svg.contains(">5<"));
    }

    #[test]
    fn errors() {
        assert!(render_svg("", "", 400, 400).is_err());
        assert!(render_svg("[]", "", 400, 400).is_err());
        assert!(render_svg("not json", "", 400, 400).is_err());
        assert!(render_svg("[[1]]", "", 400, 400).is_err()); // not a pair
    }
}
