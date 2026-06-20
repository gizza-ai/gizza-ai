//! gizza-ai/line-series-chart core — pure-Rust SVG line-chart rendering shared by
//! the chat skill block + CLI. No deps. Parses one or more numeric series and
//! emits a standalone SVG (axes, min/max/mid y labels, title, a colored polyline
//! per series, and a legend when there are multiple series).

const PALETTE: &[&str] = &[
    "#2563eb", "#dc2626", "#16a34a", "#d97706", "#7c3aed", "#0891b2", "#db2777", "#65a30d",
];

/// Parse `data` into series of f64. Series are separated by newlines or `;`; each
/// series is a comma/space separated list of numbers. Blank entries are skipped.
fn parse_series(data: &str) -> Result<Vec<Vec<f64>>, String> {
    let mut series = Vec::new();
    for line in data.split(['\n', ';']) {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut pts = Vec::new();
        for tok in line.split([',', ' ', '\t']) {
            let tok = tok.trim();
            if tok.is_empty() {
                continue;
            }
            let v: f64 = tok.parse().map_err(|_| format!("not a number: '{tok}'"))?;
            if !v.is_finite() {
                return Err(format!("not a finite number: '{tok}'"));
            }
            pts.push(v);
        }
        if !pts.is_empty() {
            series.push(pts);
        }
    }
    if series.is_empty() {
        return Err("no numeric data found".into());
    }
    Ok(series)
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn fmt_num(v: f64) -> String {
    // Compact label: integers without a decimal, else up to 3 sig-ish digits.
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:.3}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// Render the series in `data` to an SVG string. `title` is optional; `width`/
/// `height` are the SVG canvas size in px (clamped to sane minimums).
pub fn render_svg(data: &str, title: &str, width: u32, height: u32) -> Result<String, String> {
    let series = parse_series(data)?;
    let w = width.max(200) as f64;
    let h = height.max(150) as f64;
    let (ml, mr, mt, mb) = (55.0, 20.0, if title.is_empty() { 20.0 } else { 40.0 }, 30.0);
    let plot_w = w - ml - mr;
    let plot_h = h - mt - mb;

    let max_len = series.iter().map(|s| s.len()).max().unwrap_or(1).max(2);
    let mut ymin = f64::INFINITY;
    let mut ymax = f64::NEG_INFINITY;
    for s in &series {
        for &v in s {
            ymin = ymin.min(v);
            ymax = ymax.max(v);
        }
    }
    if ymin == ymax {
        ymin -= 1.0;
        ymax += 1.0;
    }
    let xmap = |i: usize| ml + (i as f64) / ((max_len - 1) as f64) * plot_w;
    let ymap = |v: f64| mt + (ymax - v) / (ymax - ymin) * plot_h;

    let mut svg = String::new();
    svg.push_str(&format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}" font-family="sans-serif">"##
    ));
    svg.push_str(&format!(r##"<rect width="{w}" height="{h}" fill="#ffffff"/>"##));
    if !title.is_empty() {
        svg.push_str(&format!(
            r##"<text x="{x}" y="24" text-anchor="middle" font-size="16" font-weight="bold" fill="#111">{t}</text>"##,
            x = w / 2.0, t = esc(title)
        ));
    }
    // Axes.
    svg.push_str(&format!(
        r##"<line x1="{ml}" y1="{t}" x2="{ml}" y2="{b}" stroke="#999"/><line x1="{ml}" y1="{b}" x2="{r}" y2="{b}" stroke="#999"/>"##,
        t = mt, b = mt + plot_h, r = ml + plot_w
    ));
    // y labels: min, mid, max.
    for (frac, val) in [(0.0, ymax), (0.5, (ymin + ymax) / 2.0), (1.0, ymin)] {
        let y = mt + frac * plot_h;
        svg.push_str(&format!(
            r##"<text x="{x}" y="{ty}" text-anchor="end" font-size="11" fill="#555">{v}</text><line x1="{ml}" y1="{y}" x2="{r}" y2="{y}" stroke="#eee"/>"##,
            x = ml - 6.0, ty = y + 4.0, v = fmt_num(val), r = ml + plot_w
        ));
    }
    // Series polylines.
    for (si, s) in series.iter().enumerate() {
        let color = PALETTE[si % PALETTE.len()];
        let pts: Vec<String> = s.iter().enumerate().map(|(i, &v)| format!("{:.1},{:.1}", xmap(i), ymap(v))).collect();
        svg.push_str(&format!(
            r##"<polyline fill="none" stroke="{color}" stroke-width="2" points="{}"/>"##,
            pts.join(" ")
        ));
    }
    // Legend (only when >1 series).
    if series.len() > 1 {
        for (si, _) in series.iter().enumerate() {
            let color = PALETTE[si % PALETTE.len()];
            let ly = mt + 4.0 + si as f64 * 16.0;
            svg.push_str(&format!(
                r##"<rect x="{lx}" y="{ry}" width="10" height="10" fill="{color}"/><text x="{tx}" y="{ty}" font-size="11" fill="#333">series {n}</text>"##,
                lx = ml + plot_w - 80.0, ry = ly, tx = ml + plot_w - 66.0, ty = ly + 9.0, n = si + 1
            ));
        }
    }
    svg.push_str("</svg>");
    Ok(svg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_series_svg() {
        let svg = render_svg("1,2,3,2,5", "My Chart", 800, 400).unwrap();
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("</svg>"));
        assert!(svg.contains("<polyline"));
        assert!(svg.contains("My Chart"));
    }

    #[test]
    fn multiple_series_get_distinct_colors_and_legend() {
        let svg = render_svg("1,2,3\n3,2,1", "", 800, 400).unwrap();
        assert_eq!(svg.matches("<polyline").count(), 2);
        assert!(svg.contains("series 1") && svg.contains("series 2"));
        assert!(svg.contains(PALETTE[0]) && svg.contains(PALETTE[1]));
    }

    #[test]
    fn parse_supports_semicolons_and_spaces() {
        let s = parse_series("1 2 3; 4 5 6").unwrap();
        assert_eq!(s.len(), 2);
        assert_eq!(s[0], vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn title_escaped() {
        let svg = render_svg("1,2", "a<b>&c", 400, 200).unwrap();
        assert!(svg.contains("a&lt;b&gt;&amp;c"));
        assert!(!svg.contains("a<b>&c"));
    }

    #[test]
    fn flat_series_does_not_divide_by_zero() {
        let svg = render_svg("5,5,5", "", 400, 200).unwrap();
        assert!(svg.contains("<polyline"));
    }

    #[test]
    fn non_numeric_errors() {
        assert!(render_svg("1,2,abc", "", 400, 200).is_err());
        assert!(render_svg("   ", "", 400, 200).is_err());
    }
}
