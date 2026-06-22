//! gizza-ai/candlestick-chart core — pure-Rust SVG candlestick (OHLC) chart
//! rendering shared by the chat skill block + CLI. No deps. Parses OHLC rows
//! from CSV and emits a standalone SVG: a price (y) axis with min/mid/max
//! labels, one candle per row (a thin high–low wick plus a thick open–close
//! body, colored green when close >= open and red otherwise), an optional
//! title, and a date label on the first/last candle when a date column is
//! present.

/// One parsed OHLC bar.
#[derive(Debug, Clone, PartialEq)]
pub struct Candle {
    pub label: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
}

const UP_COLOR: &str = "#16a34a"; // close >= open
const DOWN_COLOR: &str = "#dc2626"; // close < open

fn is_header(fields: &[&str]) -> bool {
    // Treat the row as a header if none of its fields parse as a finite number
    // (e.g. "open,high,low,close" or "date,o,h,l,c").
    fields
        .iter()
        .all(|f| f.trim().parse::<f64>().map(|v| !v.is_finite()).unwrap_or(true))
}

/// Parse CSV `data` into OHLC candles. Each non-empty row is comma-separated
/// with either 4 columns (open,high,low,close) or 5 columns
/// (label/date,open,high,low,close). A leading non-numeric header row is
/// skipped automatically. Rows whose values aren't finite or whose high < low
/// are rejected.
pub fn parse_ohlc(data: &str) -> Result<Vec<Candle>, String> {
    let mut candles = Vec::new();
    let mut first_data_row = true;
    for (lineno, raw) in data.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
        // Skip a header row (only at the first non-empty row).
        if first_data_row && is_header(&fields) {
            first_data_row = false;
            continue;
        }
        first_data_row = false;

        let (label, nums) = match fields.len() {
            4 => (String::new(), &fields[0..4]),
            5 => (fields[0].to_string(), &fields[1..5]),
            n => {
                return Err(format!(
                    "row {}: expected 4 (O,H,L,C) or 5 (label,O,H,L,C) columns, got {n}",
                    lineno + 1
                ))
            }
        };
        let mut parsed = [0f64; 4];
        for (i, tok) in nums.iter().enumerate() {
            let v: f64 = tok
                .parse()
                .map_err(|_| format!("row {}: not a number: '{tok}'", lineno + 1))?;
            if !v.is_finite() {
                return Err(format!("row {}: not a finite number: '{tok}'", lineno + 1));
            }
            parsed[i] = v;
        }
        let (open, high, low, close) = (parsed[0], parsed[1], parsed[2], parsed[3]);
        if high < low {
            return Err(format!(
                "row {}: high ({high}) is less than low ({low})",
                lineno + 1
            ));
        }
        candles.push(Candle {
            label,
            open,
            high,
            low,
            close,
        });
    }
    if candles.is_empty() {
        return Err("no OHLC rows found".into());
    }
    Ok(candles)
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:.4}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

const TREND_COLOR: &str = "#2563eb"; // closing-price trendline

/// Render OHLC `data` to a standalone SVG candlestick chart. `title` is
/// optional; `width`/`height` are the canvas size in px (clamped to sane
/// minimums); when `trendline` is true, a line connecting each bar's closing
/// price is overlaid.
pub fn render_svg(
    data: &str,
    title: &str,
    width: u32,
    height: u32,
    trendline: bool,
) -> Result<String, String> {
    let candles = parse_ohlc(data)?;
    let w = width.max(200) as f64;
    let h = height.max(150) as f64;
    let has_labels = candles.iter().any(|c| !c.label.is_empty());
    let (ml, mr, mt, mb) = (
        60.0,
        20.0,
        if title.is_empty() { 20.0 } else { 40.0 },
        if has_labels { 40.0 } else { 24.0 },
    );
    let plot_w = w - ml - mr;
    let plot_h = h - mt - mb;

    let mut ymin = f64::INFINITY;
    let mut ymax = f64::NEG_INFINITY;
    for c in &candles {
        ymin = ymin.min(c.low);
        ymax = ymax.max(c.high);
    }
    if ymin == ymax {
        ymin -= 1.0;
        ymax += 1.0;
    }
    let n = candles.len();
    // Each candle gets an equal horizontal slot; the body fills ~60% of it.
    let slot = plot_w / n as f64;
    let body_w = (slot * 0.6).max(1.0);
    let xcenter = |i: usize| ml + (i as f64 + 0.5) * slot;
    let ymap = |v: f64| mt + (ymax - v) / (ymax - ymin) * plot_h;

    let mut svg = String::new();
    svg.push_str(&format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}" font-family="sans-serif">"##
    ));
    svg.push_str(&format!(r##"<rect width="{w}" height="{h}" fill="#ffffff"/>"##));
    if !title.is_empty() {
        svg.push_str(&format!(
            r##"<text x="{x}" y="24" text-anchor="middle" font-size="16" font-weight="bold" fill="#111">{t}</text>"##,
            x = w / 2.0,
            t = esc(title)
        ));
    }
    // Axes.
    svg.push_str(&format!(
        r##"<line x1="{ml}" y1="{t}" x2="{ml}" y2="{b}" stroke="#999"/><line x1="{ml}" y1="{b}" x2="{r}" y2="{b}" stroke="#999"/>"##,
        t = mt,
        b = mt + plot_h,
        r = ml + plot_w
    ));
    // y labels + gridlines: max, mid, min.
    for (frac, val) in [(0.0, ymax), (0.5, (ymin + ymax) / 2.0), (1.0, ymin)] {
        let y = mt + frac * plot_h;
        svg.push_str(&format!(
            r##"<text x="{x}" y="{ty}" text-anchor="end" font-size="11" fill="#555">{v}</text><line x1="{ml}" y1="{y}" x2="{r}" y2="{y}" stroke="#eee"/>"##,
            x = ml - 6.0,
            ty = y + 4.0,
            v = fmt_num(val),
            r = ml + plot_w
        ));
    }
    // Candles.
    for (i, c) in candles.iter().enumerate() {
        let cx = xcenter(i);
        let up = c.close >= c.open;
        let color = if up { UP_COLOR } else { DOWN_COLOR };
        // Wick (high -> low).
        svg.push_str(&format!(
            r##"<line x1="{cx:.1}" y1="{yh:.1}" x2="{cx:.1}" y2="{yl:.1}" stroke="{color}" stroke-width="1"/>"##,
            yh = ymap(c.high),
            yl = ymap(c.low)
        ));
        // Body (open -> close); ensure a minimum 1px height for doji bars.
        let y_top = ymap(c.open.max(c.close));
        let y_bot = ymap(c.open.min(c.close));
        let bh = (y_bot - y_top).max(1.0);
        let bx = cx - body_w / 2.0;
        svg.push_str(&format!(
            r##"<rect x="{bx:.1}" y="{y_top:.1}" width="{bw:.1}" height="{bh:.1}" fill="{color}"/>"##,
            bw = body_w
        ));
    }
    // Optional closing-price trendline overlaid on the candles.
    if trendline && n > 1 {
        let pts: Vec<String> = candles
            .iter()
            .enumerate()
            .map(|(i, c)| format!("{:.1},{:.1}", xcenter(i), ymap(c.close)))
            .collect();
        svg.push_str(&format!(
            r##"<polyline fill="none" stroke="{TREND_COLOR}" stroke-width="1.5" stroke-dasharray="4 3" points="{}"/>"##,
            pts.join(" ")
        ));
    }
    // x labels on the first and last candle when labels are present.
    if has_labels {
        let ty = mt + plot_h + 16.0;
        let first = &candles[0];
        if !first.label.is_empty() {
            svg.push_str(&format!(
                r##"<text x="{x:.1}" y="{ty:.1}" text-anchor="middle" font-size="11" fill="#555">{l}</text>"##,
                x = xcenter(0),
                l = esc(&first.label)
            ));
        }
        if n > 1 {
            let last = &candles[n - 1];
            if !last.label.is_empty() {
                svg.push_str(&format!(
                    r##"<text x="{x:.1}" y="{ty:.1}" text-anchor="middle" font-size="11" fill="#555">{l}</text>"##,
                    x = xcenter(n - 1),
                    l = esc(&last.label)
                ));
            }
        }
    }
    svg.push_str("</svg>");
    Ok(svg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_four_column_rows() {
        let c = parse_ohlc("10,15,8,12\n12,14,11,11").unwrap();
        assert_eq!(c.len(), 2);
        assert_eq!(
            c[0],
            Candle { label: String::new(), open: 10.0, high: 15.0, low: 8.0, close: 12.0 }
        );
        assert_eq!(c[1].close, 11.0);
    }

    #[test]
    fn parses_labeled_rows_and_skips_header() {
        let c = parse_ohlc("date,open,high,low,close\n2024-01-01,10,15,8,12\n2024-01-02,12,16,11,15")
            .unwrap();
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].label, "2024-01-01");
        assert_eq!(c[1].label, "2024-01-02");
        assert_eq!(c[0].open, 10.0);
    }

    #[test]
    fn renders_svg_with_up_and_down_colors() {
        // First candle up (close>open), second down (close<open).
        let svg = render_svg("10,15,8,14\n14,15,9,10", "Prices", 800, 400, false).unwrap();
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("</svg>"));
        assert!(svg.contains("Prices"));
        assert!(svg.contains(UP_COLOR));
        assert!(svg.contains(DOWN_COLOR));
        // background rect + one body rect per candle.
        assert_eq!(svg.matches("<rect").count(), 1 + 2);
    }

    #[test]
    fn title_escaped() {
        let svg = render_svg("1,2,0,1", "a<b>&c", 400, 200, false).unwrap();
        assert!(svg.contains("a&lt;b&gt;&amp;c"));
        assert!(!svg.contains(">a<b>&c<"));
    }

    #[test]
    fn flat_prices_do_not_divide_by_zero() {
        let svg = render_svg("5,5,5,5", "", 400, 200, false).unwrap();
        assert!(svg.contains("<rect"));
    }

    #[test]
    fn label_is_rendered() {
        let svg = render_svg("Mon,10,15,8,12", "", 400, 200, false).unwrap();
        assert!(svg.contains(">Mon<"));
    }

    #[test]
    fn trendline_adds_polyline_only_when_enabled() {
        let with = render_svg("10,15,8,14\n14,15,9,10", "", 400, 200, true).unwrap();
        let without = render_svg("10,15,8,14\n14,15,9,10", "", 400, 200, false).unwrap();
        assert!(with.contains("<polyline"));
        assert!(with.contains(TREND_COLOR));
        assert!(!without.contains("<polyline"));
    }

    #[test]
    fn rejects_bad_input() {
        assert!(parse_ohlc("1,2,3").is_err()); // wrong column count
        assert!(parse_ohlc("1,2,abc,4").is_err()); // non-numeric
        assert!(parse_ohlc("   ").is_err()); // empty
        assert!(parse_ohlc("10,5,15,12").is_err()); // high < low
    }
}
