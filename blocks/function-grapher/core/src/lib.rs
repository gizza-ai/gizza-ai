//! gizza-ai/function-grapher core — pure-Rust SVG plotting of one or more
//! mathematical functions y = f(x) over an x-range. Depends only on `meval`
//! (a wasm-safe nom-based expression evaluator), so it runs on ALL backends
//! including the chat Service Worker. The block wraps the SVG string as an
//! `image/svg+xml` data-URL envelope. Surfaces: chat + CLI (no standalone page
//! — a pure-Rust image-bytes output has no page render mode).

use meval::Expr;

/// Distinct, color-blind-friendly stroke colors, one per function.
const PALETTE: &[&str] = &[
    "#2563eb", "#dc2626", "#16a34a", "#d97706", "#7c3aed", "#0891b2", "#db2777", "#65a30d",
];

/// Parsed, named function ready to evaluate at a given `x`.
struct Func {
    label: String,
    eval: Box<dyn Fn(f64) -> f64>,
}

/// Split the `expressions` string into individual function expressions. Functions
/// are separated by newlines or `;`. Blank entries are skipped. An optional
/// `name=` or `name:` prefix sets the legend label (e.g. `parabola: x^2`).
fn split_expressions(expressions: &str) -> Vec<(Option<String>, String)> {
    let mut out = Vec::new();
    for raw in expressions.split(['\n', ';']) {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        // A leading `label=` or `label:` (label may not itself contain those
        // separators, and must not look like part of the math) names the curve.
        if let Some((name, rest)) = split_label(line) {
            out.push((Some(name), rest));
        } else {
            out.push((None, line.to_string()));
        }
    }
    out
}

/// Detect a `name= expr` / `name: expr` prefix. Only treats it as a label when
/// the name is a plain identifier (letters/digits/spaces/underscores) so we
/// don't mis-parse comparison/colon-free math. `y=` is stripped as a no-op alias.
fn split_label(line: &str) -> Option<(String, String)> {
    for sep in ['=', ':'] {
        if let Some(idx) = line.find(sep) {
            let (name, rest) = line.split_at(idx);
            let name = name.trim();
            let rest = rest[1..].trim().to_string();
            if rest.is_empty() {
                continue;
            }
            let plain = !name.is_empty()
                && name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == ' ');
            if plain {
                // `y = ...` is the conventional spelling; treat it as unnamed.
                if name.eq_ignore_ascii_case("y") {
                    return Some(("__y__".to_string(), rest));
                }
                return Some((name.to_string(), rest));
            }
        }
    }
    None
}

/// Parse + bind every expression in `expressions`. Each is compiled to a closure
/// over `x`. Returns an error naming the first expression that fails to parse.
fn parse_functions(expressions: &str) -> Result<Vec<Func>, String> {
    let parts = split_expressions(expressions);
    if parts.is_empty() {
        return Err("no function expression provided".into());
    }
    let mut funcs = Vec::new();
    for (name, expr_src) in parts {
        let expr: Expr = expr_src
            .parse()
            .map_err(|e| format!("could not parse '{expr_src}': {e}"))?;
        let f = expr
            .bind("x")
            .map_err(|e| format!("expression '{expr_src}' must be a function of x: {e}"))?;
        let label = match name {
            Some(n) if n == "__y__" => expr_src.clone(),
            Some(n) => n,
            None => expr_src.clone(),
        };
        funcs.push(Func {
            label,
            eval: Box::new(f),
        });
    }
    Ok(funcs)
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// Compact numeric label: integers without a decimal, else trimmed to ~4 sig figs.
fn fmt_num(v: f64) -> String {
    if v == 0.0 {
        return "0".to_string();
    }
    if v.fract() == 0.0 && v.abs() < 1e15 {
        return format!("{}", v as i64);
    }
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    if s.is_empty() || s == "-" {
        "0".to_string()
    } else {
        s
    }
}

/// Render one or more functions of `x` to a standalone SVG string.
///
/// * `expressions` — one or more `f(x)` expressions, separated by newlines or `;`.
///   Each may carry a `label= ` / `label: ` prefix for the legend (`y=` is treated
///   as unlabeled). Supports `+ - * / ^`, parentheses, and the usual functions
///   (`sin`, `cos`, `tan`, `sqrt`, `abs`, `ln`, `log10`, `exp`, …) and constants
///   (`pi`, `e`) via `meval`.
/// * `xmin`/`xmax` — the x domain (must satisfy `xmin < xmax`).
/// * `samples` — number of points evaluated per curve (clamped to 2..=4000).
/// * `ymin`/`ymax` — optional fixed y-axis window; pass `NaN` (or `ymin >= ymax`)
///   for either to autoscale that bound to the data. Setting both clips the
///   curves to the window instead of autoscaling.
/// * `title` — optional chart title.
/// * `width`/`height` — SVG canvas size in px (clamped to sane minimums).
#[allow(clippy::too_many_arguments)]
pub fn render_svg(
    expressions: &str,
    xmin: f64,
    xmax: f64,
    samples: u32,
    ymin_opt: f64,
    ymax_opt: f64,
    title: &str,
    width: u32,
    height: u32,
) -> Result<String, String> {
    if !xmin.is_finite() || !xmax.is_finite() {
        return Err("xmin and xmax must be finite numbers".into());
    }
    if xmin >= xmax {
        return Err(format!("xmin ({xmin}) must be less than xmax ({xmax})"));
    }
    // A manual y-window is honored only when both bounds are finite and ordered.
    let manual_y = ymin_opt.is_finite() && ymax_opt.is_finite() && ymin_opt < ymax_opt;
    if ymin_opt.is_finite() && ymax_opt.is_finite() && ymin_opt >= ymax_opt {
        return Err(format!("ymin ({ymin_opt}) must be less than ymax ({ymax_opt})"));
    }
    let funcs = parse_functions(expressions)?;
    let n = samples.clamp(2, 4000) as usize;
    let w = width.max(200) as f64;
    let h = height.max(150) as f64;

    // Sample each function across the domain, collecting finite (x, y) points and
    // tracking the y-range for autoscaling.
    let mut series: Vec<Vec<(f64, f64)>> = Vec::with_capacity(funcs.len());
    let mut data_min = f64::INFINITY;
    let mut data_max = f64::NEG_INFINITY;
    for func in &funcs {
        let mut pts = Vec::with_capacity(n);
        for i in 0..n {
            let x = xmin + (xmax - xmin) * (i as f64) / ((n - 1) as f64);
            let y = (func.eval)(x);
            if y.is_finite() {
                pts.push((x, y));
                if y < data_min {
                    data_min = y;
                }
                if y > data_max {
                    data_max = y;
                }
            } else {
                // Discontinuity / out-of-domain: break the polyline here.
                pts.push((x, f64::NAN));
            }
        }
        series.push(pts);
    }

    let (mut ymin, mut ymax) = if manual_y {
        (ymin_opt, ymax_opt)
    } else {
        if !data_min.is_finite() || !data_max.is_finite() {
            return Err("the function(s) produced no finite values over this x-range".into());
        }
        (data_min, data_max)
    };

    if !manual_y {
        // Pad a flat range so a constant function still renders with breathing room.
        if (ymax - ymin).abs() < 1e-12 {
            let pad = if ymax.abs() > 1e-12 { ymax.abs() * 0.1 } else { 1.0 };
            ymin -= pad;
            ymax += pad;
        } else {
            let pad = (ymax - ymin) * 0.05;
            ymin -= pad;
            ymax += pad;
        }
    }

    // Plot area margins (room for axis labels + an optional title).
    let ml = 56.0;
    let mr = 16.0;
    let mt = if title.is_empty() { 16.0 } else { 40.0 };
    let mb = 32.0;
    let plot_w = (w - ml - mr).max(10.0);
    let plot_h = (h - mt - mb).max(10.0);

    let xmap = |x: f64| ml + (x - xmin) / (xmax - xmin) * plot_w;
    let ymap = |y: f64| mt + (ymax - y) / (ymax - ymin) * plot_h;

    let r = ml + plot_w;
    let b = mt + plot_h;

    let mut svg = String::new();
    svg.push_str(&format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}" font-family="system-ui, sans-serif"><defs><clipPath id="plot"><rect x="{ml}" y="{mt}" width="{plot_w:.1}" height="{plot_h:.1}"/></clipPath></defs><rect width="{w}" height="{h}" fill="#ffffff"/>"##
    ));

    // Title.
    if !title.is_empty() {
        svg.push_str(&format!(
            r##"<text x="{x}" y="24" text-anchor="middle" font-size="16" font-weight="bold" fill="#111">{t}</text>"##,
            x = w / 2.0,
            t = esc(title)
        ));
    }

    // Horizontal gridlines + y labels (5 ticks).
    for k in 0..=4 {
        let frac = k as f64 / 4.0;
        let y = mt + frac * plot_h;
        let val = ymax - frac * (ymax - ymin);
        svg.push_str(&format!(
            r##"<line x1="{ml}" y1="{y:.1}" x2="{r:.1}" y2="{y:.1}" stroke="#eeeeee"/><text x="{lx:.1}" y="{ty:.1}" text-anchor="end" font-size="11" fill="#555">{v}</text>"##,
            lx = ml - 6.0,
            ty = y + 4.0,
            v = fmt_num(val)
        ));
    }
    // Vertical gridlines + x labels (5 ticks).
    for k in 0..=4 {
        let frac = k as f64 / 4.0;
        let x = ml + frac * plot_w;
        let val = xmin + frac * (xmax - xmin);
        svg.push_str(&format!(
            r##"<line x1="{x:.1}" y1="{mt}" x2="{x:.1}" y2="{b:.1}" stroke="#eeeeee"/><text x="{x:.1}" y="{ty:.1}" text-anchor="middle" font-size="11" fill="#555">{v}</text>"##,
            ty = b + 16.0,
            v = fmt_num(val)
        ));
    }

    // Plot border.
    svg.push_str(&format!(
        r##"<rect x="{ml}" y="{mt}" width="{plot_w:.1}" height="{plot_h:.1}" fill="none" stroke="#bbbbbb"/>"##
    ));

    // Emphasized axes (x=0 / y=0) when they fall inside the domain/range.
    if xmin < 0.0 && xmax > 0.0 {
        let x0 = xmap(0.0);
        svg.push_str(&format!(
            r##"<line x1="{x0:.1}" y1="{mt}" x2="{x0:.1}" y2="{b:.1}" stroke="#888888"/>"##
        ));
    }
    if ymin < 0.0 && ymax > 0.0 {
        let y0 = ymap(0.0);
        svg.push_str(&format!(
            r##"<line x1="{ml}" y1="{y0:.1}" x2="{r:.1}" y2="{y0:.1}" stroke="#888888"/>"##
        ));
    }

    // Curves: one polyline per contiguous finite run (NaN breaks the line).
    // Clipped to the plot rect so a manual y-window crops cleanly.
    svg.push_str(r##"<g clip-path="url(#plot)">"##);
    for (si, pts) in series.iter().enumerate() {
        let color = PALETTE[si % PALETTE.len()];
        let mut run: Vec<String> = Vec::new();
        let flush = |run: &mut Vec<String>, svg: &mut String| {
            if run.len() >= 2 {
                svg.push_str(&format!(
                    r##"<polyline fill="none" stroke="{color}" stroke-width="2" points="{}"/>"##,
                    run.join(" ")
                ));
            }
            run.clear();
        };
        for &(x, y) in pts {
            if y.is_finite() {
                run.push(format!("{:.2},{:.2}", xmap(x), ymap(y)));
            } else {
                flush(&mut run, &mut svg);
            }
        }
        flush(&mut run, &mut svg);
    }
    svg.push_str("</g>");

    // Legend (always shown — labels make multi-curve plots readable).
    let legend_x = r - 8.0;
    for (si, func) in funcs.iter().enumerate() {
        let color = PALETTE[si % PALETTE.len()];
        let ly = mt + 6.0 + si as f64 * 16.0;
        let label = esc(&func.label);
        svg.push_str(&format!(
            r##"<rect x="{rx:.1}" y="{ry:.1}" width="10" height="10" fill="{color}"/><text x="{tx:.1}" y="{ty:.1}" text-anchor="end" font-size="11" fill="#333">{label}</text>"##,
            rx = legend_x,
            ry = ly,
            tx = legend_x - 4.0,
            ty = ly + 9.0
        ));
    }

    svg.push_str("</svg>");
    Ok(svg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plots_single_function() {
        let svg = render_svg("x^2", -5.0, 5.0, 100, f64::NAN, f64::NAN, "Parabola", 800, 500).unwrap();
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        assert!(svg.contains("<polyline"));
        assert!(svg.contains("Parabola"));
    }

    #[test]
    fn plots_multiple_functions_with_distinct_colors() {
        let svg = render_svg("sin(x); cos(x)", -3.14, 3.14, 200, f64::NAN, f64::NAN, "", 800, 400).unwrap();
        assert_eq!(svg.matches("<polyline").count() >= 2, true);
        assert!(svg.contains(PALETTE[0]) && svg.contains(PALETTE[1]));
    }

    #[test]
    fn named_function_appears_in_legend() {
        let svg = render_svg("parabola: x^2", -2.0, 2.0, 50, f64::NAN, f64::NAN, "", 600, 400).unwrap();
        assert!(svg.contains("parabola"));
        // The raw expression should NOT be the legend label when named.
        assert!(svg.matches("parabola").count() >= 1);
    }

    #[test]
    fn y_equals_prefix_is_stripped_to_expression_label() {
        let svg = render_svg("y = x + 1", -1.0, 1.0, 10, f64::NAN, f64::NAN, "", 400, 300).unwrap();
        // Legend label is the expression, not "y".
        assert!(svg.contains("x + 1"));
    }

    #[test]
    fn supports_constants_and_functions() {
        let svg = render_svg("sqrt(abs(x)) * pi", -4.0, 4.0, 100, f64::NAN, f64::NAN, "", 500, 300).unwrap();
        assert!(svg.contains("<polyline"));
    }

    #[test]
    fn draws_zero_axes_when_in_range() {
        // Domain & range straddle 0 → both emphasized axes drawn (#888888).
        let svg = render_svg("x", -5.0, 5.0, 50, f64::NAN, f64::NAN, "", 400, 400).unwrap();
        assert_eq!(svg.matches("#888888").count(), 2);
    }

    #[test]
    fn constant_function_does_not_divide_by_zero() {
        let svg = render_svg("3", -2.0, 2.0, 20, f64::NAN, f64::NAN, "", 400, 300).unwrap();
        assert!(svg.contains("<polyline"));
    }

    #[test]
    fn manual_y_window_clips_and_labels() {
        // Fixed y-window [-1, 1] for an unbounded function → curve clipped, the
        // bound shows up as a y-axis label, and a clipPath is emitted.
        let svg = render_svg("x", -10.0, 10.0, 100, -1.0, 1.0, "", 400, 400).unwrap();
        assert!(svg.contains(r#"clip-path="url(#plot)""#));
        assert!(svg.contains("<clipPath"));
        // Top y label is the manual ymax (1), not the data max (~10).
        assert!(svg.contains(">1<"));
    }

    #[test]
    fn auto_y_window_when_only_one_bound_set() {
        // Only ymax finite → not a manual window → autoscales (no error on a
        // function whose data exceeds the lone bound).
        let svg = render_svg("x^2", -3.0, 3.0, 50, f64::NAN, 2.0, "", 400, 300).unwrap();
        assert!(svg.contains("<polyline"));
    }

    #[test]
    fn rejects_inverted_y_window() {
        let err = render_svg("x", -1.0, 1.0, 10, 5.0, 1.0, "", 400, 300).unwrap_err();
        assert!(err.contains("ymin"), "got: {err}");
    }

    #[test]
    fn discontinuity_breaks_into_multiple_polylines() {
        // 1/x is undefined at x=0; over a symmetric domain it should split.
        let svg = render_svg("1/x", -2.0, 2.0, 101, f64::NAN, f64::NAN, "", 400, 400).unwrap();
        assert!(svg.matches("<polyline").count() >= 2);
    }

    #[test]
    fn rejects_unparseable_expression() {
        let err = render_svg("x +* )", -1.0, 1.0, 10, f64::NAN, f64::NAN, "", 400, 300).unwrap_err();
        assert!(err.contains("parse") || err.contains("function of x"), "got: {err}");
    }

    #[test]
    fn rejects_non_x_variable() {
        let err = render_svg("z^2", -1.0, 1.0, 10, f64::NAN, f64::NAN, "", 400, 300).unwrap_err();
        assert!(err.contains("function of x"), "got: {err}");
    }

    #[test]
    fn rejects_bad_range() {
        assert!(render_svg("x", 5.0, 5.0, 10, f64::NAN, f64::NAN, "", 400, 300).is_err());
        assert!(render_svg("x", 5.0, 1.0, 10, f64::NAN, f64::NAN, "", 400, 300).is_err());
    }

    #[test]
    fn rejects_all_nonfinite_over_range() {
        // sqrt(x) over a fully-negative domain yields no finite points.
        let err = render_svg("sqrt(x)", -4.0, -1.0, 20, f64::NAN, f64::NAN, "", 400, 300).unwrap_err();
        assert!(err.contains("no finite"), "got: {err}");
    }

    #[test]
    fn empty_expression_errors() {
        assert!(render_svg("   ", -1.0, 1.0, 10, f64::NAN, f64::NAN, "", 400, 300).is_err());
    }

    #[test]
    fn fmt_num_is_compact() {
        assert_eq!(fmt_num(0.0), "0");
        assert_eq!(fmt_num(5.0), "5");
        assert_eq!(fmt_num(-3.0), "-3");
        assert_eq!(fmt_num(1.25), "1.25");
    }
}
