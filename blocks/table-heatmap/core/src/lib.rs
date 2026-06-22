//! gizza-ai/table-heatmap core — apply spreadsheet-style color-scale conditional
//! formatting to a CSV/table and render a styled HTML `<table>` with shaded cells.
//! Pure-Rust (`csv`). No wafer/wasm-bindgen deps.

fn delim_byte(d: &str) -> Result<u8, String> {
    Ok(match d {
        "" | "," | "comma" => b',',
        "\t" | "tab" | "\\t" => b'\t',
        ";" | "semicolon" => b';',
        "|" | "pipe" => b'|',
        other => {
            let b = other.as_bytes();
            if b.len() == 1 {
                b[0]
            } else {
                return Err(format!(
                    "delimiter must be a single char or tab/comma/semicolon/pipe, got '{other}'"
                ));
            }
        }
    })
}

/// Built-in color scales. Each maps a normalized t in [0,1] (low→high) to an RGB color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scale {
    /// Sequential white → green (low values pale, high values green).
    GreenScale,
    /// Sequential white → blue.
    BlueScale,
    /// Diverging red → yellow → green (low=red, mid=yellow, high=green) — spreadsheet "good/bad".
    RedYellowGreen,
    /// Diverging green → yellow → red (low=green, high=red).
    GreenYellowRed,
    /// Diverging blue → white → red (low=blue, mid=white, high=red).
    BlueWhiteRed,
}

impl Scale {
    pub fn parse(s: &str) -> Result<Scale, String> {
        match s.trim().to_ascii_lowercase().replace('_', "-").as_str() {
            "" | "red-yellow-green" | "rdylgn" => Ok(Scale::RedYellowGreen),
            "green-yellow-red" | "gnylrd" => Ok(Scale::GreenYellowRed),
            "blue-white-red" | "bwr" => Ok(Scale::BlueWhiteRed),
            "green" | "green-scale" | "greens" => Ok(Scale::GreenScale),
            "blue" | "blue-scale" | "blues" => Ok(Scale::BlueScale),
            other => Err(format!(
                "unknown scale '{other}' (use red-yellow-green, green-yellow-red, blue-white-red, green, or blue)"
            )),
        }
    }

    /// `t` in [0,1] (already clamped). Returns (r,g,b) in 0..=255.
    fn color(self, t: f64) -> (u8, u8, u8) {
        let t = t.clamp(0.0, 1.0);
        match self {
            Scale::GreenScale => lerp((255, 255, 255), (68, 160, 71), t),
            Scale::BlueScale => lerp((255, 255, 255), (40, 110, 200), t),
            Scale::RedYellowGreen => {
                // low=red, mid=yellow, high=green
                diverging((230, 88, 80), (255, 235, 132), (87, 171, 90), t)
            }
            Scale::GreenYellowRed => diverging((87, 171, 90), (255, 235, 132), (230, 88, 80), t),
            Scale::BlueWhiteRed => diverging((58, 110, 200), (245, 245, 245), (220, 80, 70), t),
        }
    }
}

fn lerp(a: (u8, u8, u8), b: (u8, u8, u8), t: f64) -> (u8, u8, u8) {
    let f = |x: u8, y: u8| (x as f64 + (y as f64 - x as f64) * t).round().clamp(0.0, 255.0) as u8;
    (f(a.0, b.0), f(a.1, b.1), f(a.2, b.2))
}

fn diverging(lo: (u8, u8, u8), mid: (u8, u8, u8), hi: (u8, u8, u8), t: f64) -> (u8, u8, u8) {
    if t <= 0.5 {
        lerp(lo, mid, t / 0.5)
    } else {
        lerp(mid, hi, (t - 0.5) / 0.5)
    }
}

/// Choose black or white text for legibility against a background color (WCAG luminance).
fn text_color(bg: (u8, u8, u8)) -> &'static str {
    let lum = 0.2126 * bg.0 as f64 + 0.7152 * bg.1 as f64 + 0.0722 * bg.2 as f64;
    if lum > 150.0 {
        "#111"
    } else {
        "#fff"
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Parse a cell as a number, tolerating surrounding whitespace, thousands commas,
/// a leading currency sign, percent sign, and parentheses-as-negative accounting style.
fn parse_num(s: &str) -> Option<f64> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    let mut neg = false;
    let mut body = t;
    if body.starts_with('(') && body.ends_with(')') {
        neg = true;
        body = &body[1..body.len() - 1];
    }
    let mut percent = false;
    let cleaned: String = body
        .chars()
        .filter(|c| {
            if *c == '%' {
                percent = true;
                false
            } else {
                !matches!(c, ',' | '$' | '€' | '£' | ' ')
            }
        })
        .collect();
    let mut v: f64 = cleaned.parse().ok()?;
    if percent {
        v /= 100.0;
    }
    if neg {
        v = -v;
    }
    Some(v)
}

/// Optional fixed color-scale bounds (like Excel's Number-type min/mid/max anchors).
/// Any field left `None` falls back to the data-derived value.
#[derive(Debug, Clone, Copy, Default)]
pub struct Bounds {
    /// Value mapped to the low end of the scale (defaults to the data minimum).
    pub min: Option<f64>,
    /// For diverging (3-color) scales, the value mapped to the scale's midpoint
    /// (defaults to halfway between min and max). Ignored for sequential scales.
    pub midpoint: Option<f64>,
    /// Value mapped to the high end of the scale (defaults to the data maximum).
    pub max: Option<f64>,
}

/// Render `data` (CSV) as an HTML table with color-scale conditional formatting on numeric cells.
///
/// * `scale` — color scale to use.
/// * `has_header` — treat the first row as a header (not shaded, not scaled).
/// * `per_column` — when true each numeric column is scaled against its own min/max;
///   when false a single global min/max over all numeric cells is used. Ignored when
///   `bounds.min` and `bounds.max` are both fixed.
/// * `bounds` — optional fixed min/midpoint/max anchors; unset fields use the data range.
/// * `delimiter` — field separator.
pub fn heatmap(
    data: &str,
    scale: Scale,
    has_header: bool,
    per_column: bool,
    bounds: Bounds,
    delimiter: &str,
) -> Result<String, String> {
    if let (Some(lo), Some(hi)) = (bounds.min, bounds.max) {
        if lo >= hi {
            return Err(format!("min ({lo}) must be less than max ({hi})"));
        }
    }
    if data.trim().is_empty() {
        return Err("input is empty".into());
    }
    let delim = delim_byte(delimiter)?;
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .has_headers(false)
        .flexible(true)
        .from_reader(data.as_bytes());
    let records: Vec<csv::StringRecord> = rdr
        .records()
        .collect::<Result<_, _>>()
        .map_err(|e| format!("CSV parse error: {e}"))?;
    if records.is_empty() {
        return Err("no rows found".into());
    }
    let width = records.iter().map(|r| r.len()).max().unwrap_or(0);
    fn cell<'a>(rec: &'a csv::StringRecord, i: usize) -> &'a str {
        rec.get(i).unwrap_or("")
    }

    let (header, body): (Option<Vec<String>>, &[csv::StringRecord]) = if has_header {
        (
            Some((0..width).map(|i| cell(&records[0], i).to_string()).collect()),
            &records[1..],
        )
    } else {
        (None, &records[..])
    };

    if body.is_empty() {
        return Err("no data rows to format (only a header was provided)".into());
    }

    // Parse numeric values for every body cell.
    let nums: Vec<Vec<Option<f64>>> = body
        .iter()
        .map(|rec| (0..width).map(|i| parse_num(cell(rec, i))).collect())
        .collect();

    // Compute the data min/max: per-column, or one global range. When both fixed
    // bounds are supplied, per_column is moot — every column uses the fixed range.
    let data_ranges: Vec<Option<(f64, f64)>> = if per_column {
        (0..width)
            .map(|c| col_bounds(nums.iter().map(|row| row[c])))
            .collect()
    } else {
        let g = col_bounds(nums.iter().flat_map(|row| row.iter().copied()));
        vec![g; width]
    };

    // Resolve each column to a (min, mid, max) triple, applying any fixed overrides.
    // mid is only meaningful for diverging scales; default is the data midpoint.
    let col_ranges: Vec<Option<(f64, f64, f64)>> = data_ranges
        .iter()
        .map(|r| {
            r.map(|(dmin, dmax)| {
                let min = bounds.min.unwrap_or(dmin);
                let max = bounds.max.unwrap_or(dmax);
                let mid = bounds.midpoint.unwrap_or((min + max) / 2.0);
                (min, mid, max)
            })
        })
        .collect();

    let mut out = String::from("<table style=\"border-collapse:collapse;font-family:system-ui,Arial,sans-serif;font-size:14px\">\n");
    if let Some(h) = &header {
        out.push_str("  <thead>\n    <tr>");
        for cell in h {
            out.push_str(&format!(
                "<th style=\"padding:6px 10px;border:1px solid #ccc;background:#f3f3f3;text-align:left\">{}</th>",
                html_escape(cell)
            ));
        }
        out.push_str("</tr>\n  </thead>\n");
    }
    out.push_str("  <tbody>\n");
    for (r, rec) in body.iter().enumerate() {
        out.push_str("    <tr>");
        for c in 0..width {
            let raw = cell(rec, c);
            let style = match (nums[r][c], col_ranges[c]) {
                (Some(v), Some((min, mid, max))) => {
                    let t = norm(v, min, mid, max);
                    let (rr, gg, bb) = scale.color(t);
                    format!(
                        "padding:6px 10px;border:1px solid #ddd;text-align:right;background:rgb({rr},{gg},{bb});color:{}",
                        text_color((rr, gg, bb))
                    )
                }
                _ => "padding:6px 10px;border:1px solid #ddd;text-align:left".to_string(),
            };
            out.push_str(&format!("<td style=\"{style}\">{}</td>", html_escape(raw)));
        }
        out.push_str("</tr>\n");
    }
    out.push_str("  </tbody>\n</table>");
    Ok(out)
}

/// Map a value to t in [0,1] for the color scale, anchoring `mid` at 0.5 so a
/// diverging scale's neutral color lands on the midpoint. Degenerate ranges → 0.5.
fn norm(v: f64, min: f64, mid: f64, max: f64) -> f64 {
    if (max - min).abs() < f64::EPSILON {
        return 0.5;
    }
    // Use the midpoint only when it sits strictly inside (min,max); otherwise a
    // plain linear min→max mapping.
    if mid > min && mid < max {
        if v <= mid {
            (0.5 * (v - min) / (mid - min)).clamp(0.0, 0.5)
        } else {
            (0.5 + 0.5 * (v - mid) / (max - mid)).clamp(0.5, 1.0)
        }
    } else {
        ((v - min) / (max - min)).clamp(0.0, 1.0)
    }
}

/// min/max over an iterator of optional numbers, ignoring None. Returns None if no numbers.
fn col_bounds<I: IntoIterator<Item = Option<f64>>>(it: I) -> Option<(f64, f64)> {
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    let mut any = false;
    for v in it.into_iter().flatten() {
        if v.is_finite() {
            any = true;
            if v < min {
                min = v;
            }
            if v > max {
                max = v;
            }
        }
    }
    if any {
        Some((min, max))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_numbers_tolerantly() {
        assert_eq!(parse_num("1,234"), Some(1234.0));
        assert_eq!(parse_num("$1,234.50"), Some(1234.50));
        assert_eq!(parse_num("(50)"), Some(-50.0));
        assert_eq!(parse_num("12%"), Some(0.12));
        assert_eq!(parse_num("  -3.5 "), Some(-3.5));
        assert_eq!(parse_num("hello"), None);
        assert_eq!(parse_num(""), None);
    }

    fn auto() -> Bounds {
        Bounds::default()
    }

    #[test]
    fn header_row_not_shaded() {
        let out = heatmap("name,score\nAda,10\nBo,20", Scale::RedYellowGreen, true, true, auto(), ",").unwrap();
        assert!(out.contains("<thead>"));
        assert!(out.contains("<th style=\"padding:6px 10px;border:1px solid #ccc;background:#f3f3f3;text-align:left\">name</th>"));
        // text cell (Ada) has no background rgb()
        assert!(out.contains("text-align:left\">Ada</td>"));
        // numeric cells shaded
        assert!(out.contains("background:rgb("));
    }

    #[test]
    fn min_gets_low_color_max_gets_high_color_red_yellow_green() {
        // single column 0..100; min=red-ish, max=green-ish
        let out = heatmap("v\n0\n100", Scale::RedYellowGreen, true, true, auto(), ",").unwrap();
        // low value cell should contain the red anchor, high the green anchor
        assert!(out.contains("background:rgb(230,88,80)"), "low=red: {out}");
        assert!(out.contains("background:rgb(87,171,90)"), "high=green: {out}");
    }

    #[test]
    fn per_column_vs_global_scaling_differ() {
        let data = "a,b\n0,100\n10,200";
        let per_col = heatmap(data, Scale::GreenScale, true, true, auto(), ",").unwrap();
        let global = heatmap(data, Scale::GreenScale, false, false, auto(), ",").unwrap();
        // No header in global → no thead
        assert!(per_col.contains("<thead>"));
        assert!(!global.contains("<thead>"));
        // Per-column and global scaling produce different shading.
        assert_ne!(per_col, global);
    }

    #[test]
    fn no_header_synthesizes_no_thead() {
        let out = heatmap("1,2\n3,4", Scale::BlueScale, false, true, auto(), ",").unwrap();
        assert!(!out.contains("<thead>"));
        assert!(out.contains("<tbody>"));
    }

    #[test]
    fn constant_column_uses_midpoint() {
        let out = heatmap("v\n5\n5\n5", Scale::BlueWhiteRed, true, true, auto(), ",").unwrap();
        // midpoint of blue-white-red is white-ish
        assert!(out.contains("background:rgb(245,245,245)"), "{out}");
    }

    #[test]
    fn fixed_bounds_anchor_the_scale() {
        // Force 0..100 even though data is only 40..60: 40 is no longer the "min" color.
        let auto_out = heatmap("v\n40\n60", Scale::GreenScale, true, true, auto(), ",").unwrap();
        let fixed = heatmap(
            "v\n40\n60",
            Scale::GreenScale,
            true,
            true,
            Bounds { min: Some(0.0), max: Some(100.0), midpoint: None },
            ",",
        )
        .unwrap();
        // auto: 40 is the column min → pure white; fixed: 40 maps to t=0.4 (greenish).
        assert!(auto_out.contains("background:rgb(255,255,255)"), "auto min=white: {auto_out}");
        assert!(!fixed.contains("background:rgb(255,255,255)"), "fixed: no cell at pure white: {fixed}");
    }

    #[test]
    fn explicit_midpoint_shifts_neutral_color() {
        // diverging blue-white-red, data 0..100. Default midpoint=50 → 25 is bluish.
        // Set midpoint=25 → the value 25 itself becomes the white neutral.
        let out = heatmap(
            "v\n0\n25\n100",
            Scale::BlueWhiteRed,
            true,
            true,
            Bounds { min: None, max: None, midpoint: Some(25.0) },
            ",",
        )
        .unwrap();
        // value 25 sits exactly on the (custom) midpoint → the white neutral color.
        assert!(out.contains("background:rgb(245,245,245)"), "midpoint=white: {out}");
    }

    #[test]
    fn min_ge_max_errors() {
        assert!(heatmap(
            "v\n1\n2",
            Scale::GreenScale,
            true,
            true,
            Bounds { min: Some(10.0), max: Some(5.0), midpoint: None },
            ","
        )
        .is_err());
    }

    #[test]
    fn norm_midpoint_anchored() {
        assert_eq!(norm(50.0, 0.0, 50.0, 100.0), 0.5);
        assert_eq!(norm(0.0, 0.0, 50.0, 100.0), 0.0);
        assert_eq!(norm(100.0, 0.0, 50.0, 100.0), 1.0);
        // off-center midpoint
        assert_eq!(norm(25.0, 0.0, 25.0, 100.0), 0.5);
        // degenerate
        assert_eq!(norm(5.0, 5.0, 5.0, 5.0), 0.5);
    }

    #[test]
    fn html_escapes_cells() {
        let out = heatmap("x\n<b>&\"", Scale::GreenScale, true, true, auto(), ",").unwrap();
        assert!(out.contains("&lt;b&gt;&amp;&quot;"));
    }

    #[test]
    fn tab_delimiter() {
        let out = heatmap("a\tb\n1\t2", Scale::GreenScale, true, true, auto(), "tab").unwrap();
        assert!(out.contains("<th"));
    }

    #[test]
    fn text_color_contrast() {
        assert_eq!(text_color((255, 255, 255)), "#111");
        assert_eq!(text_color((20, 20, 20)), "#fff");
    }

    #[test]
    fn errors() {
        assert!(heatmap("", Scale::GreenScale, true, true, auto(), ",").is_err());
        assert!(heatmap("only,a,header", Scale::GreenScale, true, true, auto(), ",").is_err());
        assert!(Scale::parse("rainbow").is_err());
    }

    #[test]
    fn scale_aliases() {
        assert_eq!(Scale::parse("").unwrap(), Scale::RedYellowGreen);
        assert_eq!(Scale::parse("green_yellow_red").unwrap(), Scale::GreenYellowRed);
        assert_eq!(Scale::parse("BWR").unwrap(), Scale::BlueWhiteRed);
        assert_eq!(Scale::parse("greens").unwrap(), Scale::GreenScale);
    }
}
