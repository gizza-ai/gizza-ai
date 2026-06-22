//! gizza-ai/heatmap-chart core — render an arbitrary numeric grid (rows of
//! CSV/space-separated numbers) as a color-coded SVG heatmap with an optional
//! per-cell value label and a min→max color legend. No deps; pure SVG string
//! building. Unlike correlation-heatmap (which computes a symmetric correlation
//! matrix), this visualizes the raw values of any M×N grid with a sequential
//! (RdYlBu-reversed: blue→yellow→red) colormap.

fn fmt_num(v: f64) -> String {
    if !v.is_finite() {
        return "".to_string();
    }
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:.3}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// Parse rows of comma/space/tab-separated numbers into a rectangular grid.
fn parse_grid(data: &str) -> Result<Vec<Vec<f64>>, String> {
    let mut rows: Vec<Vec<f64>> = Vec::new();
    for (li, line) in data.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut row = Vec::new();
        for tok in line.split([',', ' ', '\t']).filter(|t| !t.trim().is_empty()) {
            let v: f64 = tok
                .trim()
                .parse()
                .map_err(|_| format!("row {}: '{tok}' is not a number", li + 1))?;
            if !v.is_finite() {
                return Err(format!("row {}: non-finite value", li + 1));
            }
            row.push(v);
        }
        if !row.is_empty() {
            rows.push(row);
        }
    }
    if rows.is_empty() {
        return Err("no numeric rows found".into());
    }
    let ncol = rows[0].len();
    if rows.iter().any(|r| r.len() != ncol) {
        return Err("all rows must have the same number of columns".into());
    }
    Ok(rows)
}

/// Sequential colormap: t in [0,1] → RGB. RdYlBu reversed
/// (low = blue #2c7bb6, mid = pale yellow #ffffbf, high = red #d7191c).
pub fn color_at(t: f64) -> (u8, u8, u8) {
    let t = t.clamp(0.0, 1.0);
    const STOPS: [(f64, (f64, f64, f64)); 3] = [
        (0.0, (44.0, 123.0, 182.0)),
        (0.5, (255.0, 255.0, 191.0)),
        (1.0, (215.0, 25.0, 28.0)),
    ];
    let (mut lo, mut hi) = (&STOPS[0], &STOPS[1]);
    if t > 0.5 {
        lo = &STOPS[1];
        hi = &STOPS[2];
    }
    let span = hi.0 - lo.0;
    let f = if span == 0.0 { 0.0 } else { (t - lo.0) / span };
    let lerp = |a: f64, b: f64| a + (b - a) * f;
    (
        lerp(lo.1 .0, hi.1 .0).round() as u8,
        lerp(lo.1 .1, hi.1 .1).round() as u8,
        lerp(lo.1 .2, hi.1 .2).round() as u8,
    )
}

/// Pick a readable text color (black/white) for a cell of the given fill.
fn text_on(r: u8, g: u8, b: u8) -> &'static str {
    // Relative luminance (sRGB-ish) → dark text on light cells.
    let lum = 0.299 * r as f64 + 0.587 * g as f64 + 0.114 * b as f64;
    if lum > 140.0 {
        "#222222"
    } else {
        "#ffffff"
    }
}

fn labels_or(csv: &str, n: usize, prefix: &str) -> Vec<String> {
    let given: Vec<String> = csv
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if given.len() == n {
        given
    } else {
        (1..=n).map(|i| format!("{prefix}{i}")).collect()
    }
}

/// Render the heatmap SVG. `show_values` prints each cell's value; `col_labels`
/// / `row_labels` are optional comma-separated names; `title` optional.
pub fn render_svg(
    data: &str,
    show_values: bool,
    col_labels: &str,
    row_labels: &str,
    title: &str,
) -> Result<String, String> {
    let grid = parse_grid(data)?;
    let nrow = grid.len();
    let ncol = grid[0].len();

    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    for r in &grid {
        for &v in r {
            min = min.min(v);
            max = max.max(v);
        }
    }
    let range = max - min;
    let norm = |v: f64| if range == 0.0 { 0.5 } else { (v - min) / range };

    let cols = labels_or(col_labels, ncol, "c");
    let rows = labels_or(row_labels, nrow, "r");

    let cell = 48.0_f64;
    let left_pad = 70.0_f64; // row labels
    let top_pad = if title.is_empty() { 28.0 } else { 50.0 };
    let legend_w = 70.0_f64;
    let grid_x = left_pad;
    let grid_y = top_pad;
    let w = grid_x + ncol as f64 * cell + legend_w + 30.0;
    let h = grid_y + nrow as f64 * cell + 16.0;

    let mut svg = String::new();
    svg.push_str(&format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}" font-family="sans-serif"><rect width="{w}" height="{h}" fill="#ffffff"/>"##
    ));
    if !title.is_empty() {
        svg.push_str(&format!(
            r##"<text x="{x}" y="30" text-anchor="middle" font-size="16" font-weight="bold" fill="#111">{t}</text>"##,
            x = (grid_x + ncol as f64 * cell) / 2.0,
            t = esc(title)
        ));
    }

    // Column labels (top).
    for (j, lab) in cols.iter().enumerate() {
        let cx = grid_x + j as f64 * cell + cell / 2.0;
        svg.push_str(&format!(
            r##"<text x="{cx}" y="{y}" text-anchor="middle" font-size="11" fill="#333">{l}</text>"##,
            y = grid_y - 6.0,
            l = esc(lab)
        ));
    }

    for (i, row) in grid.iter().enumerate() {
        // Row label (left).
        let ly = grid_y + i as f64 * cell + cell / 2.0 + 4.0;
        svg.push_str(&format!(
            r##"<text x="{lx}" y="{ly}" text-anchor="end" font-size="11" fill="#333">{l}</text>"##,
            lx = grid_x - 8.0,
            l = esc(&rows[i])
        ));
        for (j, &v) in row.iter().enumerate() {
            let x = grid_x + j as f64 * cell;
            let y = grid_y + i as f64 * cell;
            let (r, g, b) = color_at(norm(v));
            svg.push_str(&format!(
                r##"<rect x="{x}" y="{y}" width="{cell}" height="{cell}" fill="#{r:02x}{g:02x}{b:02x}" stroke="#ffffff"/>"##
            ));
            if show_values {
                svg.push_str(&format!(
                    r##"<text x="{tx}" y="{ty}" text-anchor="middle" font-size="11" fill="{c}">{val}</text>"##,
                    tx = x + cell / 2.0,
                    ty = y + cell / 2.0 + 4.0,
                    c = text_on(r, g, b),
                    val = esc(&fmt_num(v))
                ));
            }
        }
    }

    // Legend: a vertical gradient bar with min/max labels.
    let lx = grid_x + ncol as f64 * cell + 24.0;
    let lh = (nrow as f64 * cell).min(220.0).max(80.0);
    let ly0 = grid_y;
    let steps = 24;
    for s in 0..steps {
        let frac_top = s as f64 / steps as f64;
        // top of the bar = max (t=1), bottom = min (t=0)
        let t = 1.0 - frac_top;
        let (r, g, b) = color_at(t);
        let seg_y = ly0 + frac_top * lh;
        let seg_h = lh / steps as f64 + 1.0;
        svg.push_str(&format!(
            r##"<rect x="{lx}" y="{seg_y}" width="16" height="{seg_h}" fill="#{r:02x}{g:02x}{b:02x}"/>"##
        ));
    }
    svg.push_str(&format!(
        r##"<rect x="{lx}" y="{ly0}" width="16" height="{lh}" fill="none" stroke="#999"/>"##
    ));
    svg.push_str(&format!(
        r##"<text x="{tx}" y="{ty}" font-size="10" fill="#333">{m}</text>"##,
        tx = lx + 20.0,
        ty = ly0 + 8.0,
        m = esc(&fmt_num(max))
    ));
    svg.push_str(&format!(
        r##"<text x="{tx}" y="{ty}" font-size="10" fill="#333">{m}</text>"##,
        tx = lx + 20.0,
        ty = ly0 + lh,
        m = esc(&fmt_num(min))
    ));

    svg.push_str("</svg>");
    Ok(svg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colormap_endpoints() {
        assert_eq!(color_at(0.0), (44, 123, 182));
        assert_eq!(color_at(0.5), (255, 255, 191));
        assert_eq!(color_at(1.0), (215, 25, 28));
    }

    #[test]
    fn renders_grid_with_values() {
        let svg = render_svg("1,2\n3,4", true, "", "", "Heat").unwrap();
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        assert!(svg.contains("Heat"));
        // cell values present
        assert!(svg.contains(">1<") && svg.contains(">4<"));
        // legend min/max
        assert!(svg.contains(">4<") && svg.contains(">1<"));
    }

    #[test]
    fn hides_values_when_disabled() {
        let svg = render_svg("10,20\n30,40", false, "", "", "").unwrap();
        // Non-extreme cell values (20, 30) must not appear as text when hidden.
        // (10 and 40 are the legend min/max labels, so we don't check those.)
        assert!(!svg.contains(">20<"));
        assert!(!svg.contains(">30<"));
        assert!(svg.contains("<rect")); // cells still drawn
    }

    #[test]
    fn custom_labels_render() {
        let svg = render_svg("1,2\n3,4", true, "Jan,Feb", "North,South", "").unwrap();
        assert!(svg.contains(">Jan<") && svg.contains(">Feb<"));
        assert!(svg.contains(">North<") && svg.contains(">South<"));
    }

    #[test]
    fn errors() {
        assert!(render_svg("", true, "", "", "").is_err());
        assert!(render_svg("1,2\n3", true, "", "", "").is_err()); // ragged
        assert!(render_svg("1,x", true, "", "", "").is_err()); // non-numeric
    }

    #[test]
    fn constant_grid_is_midcolor() {
        // All equal → normalized 0.5 → mid color, no panic.
        let svg = render_svg("5,5\n5,5", true, "", "", "").unwrap();
        assert!(svg.contains("#ffffbf"));
    }
}
