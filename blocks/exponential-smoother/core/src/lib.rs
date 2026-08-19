//! exponential-smoother core — exponentially-weighted moving average (EWMA)
//! smoothing over a numeric series.
//!
//! The decay can be given four equivalent ways (all reduce to a single
//! smoothing factor `alpha`):
//!
//! ```text
//! alpha    = alpha                       for 0 < alpha <= 1
//! span     -> alpha = 2 / (span + 1)     for span >= 1
//! halflife -> alpha = 1 - exp(-ln 2 / halflife)   for halflife > 0
//! com      -> alpha = 1 / (1 + com)      for com >= 0
//! ```
//!
//! Two weighting conventions are supported, matching the standard definitions:
//!
//! * `adjust = true` (default) divides by the decaying weight sum, so early
//!   points are not biased toward the first observation:
//!   `y_t = (x_t + (1-a)x_{t-1} + … + (1-a)^t x_0) / (1 + (1-a) + … + (1-a)^t)`
//! * `adjust = false` is the plain recursion used by simple exponential
//!   smoothing (SES) and by finance EMAs: `y_0 = x_0`, `y_t = (1-a)y_{t-1} + a x_t`
//!
//! Missing observations (`na`, `nan`, `null`, `-`, empty slots) are carried
//! through as gaps; `ignore_na` picks whether the gap still consumes decay.
//!
//! `mode = "auto"` fits alpha by minimising the sum of squared one-step-ahead
//! forecast errors — the usual way an SES smoothing constant is chosen.
//!
//! Pure compute — no wafer / wasm-bindgen deps. Shared by the chat skill block,
//! the `gizza` CLI and the browser page.

/// Max data points accepted (bounds the JSON payload and keeps the `auto`
/// alpha search — a few hundred EWMA passes — responsive in a browser tab).
pub const MAX_POINTS: usize = 20_000;
/// Max bytes of `series` text accepted.
pub const MAX_BYTES: usize = 2_000_000;
/// Max future periods `forecast` may request.
pub const MAX_FORECAST: usize = 1_000;

/// Everything except the series itself. `Default` matches the descriptor's
/// declared defaults, so the CLI, the chat schema and the page agree.
#[derive(Debug, Clone)]
pub struct Options {
    /// How the decay is specified: `alpha` | `span` | `halflife` | `com` | `auto`.
    pub mode: String,
    pub alpha: f64,
    pub span: f64,
    pub halflife: f64,
    pub com: f64,
    /// Divide by the decaying weight sum (bias-corrected) instead of recursing.
    pub adjust: bool,
    /// Weight relative to the last *observation* rather than the last position.
    pub ignore_na: bool,
    /// Observations required before a smoothed value is emitted (0 behaves as 1).
    pub min_periods: usize,
    /// Future periods to project (flat, at the final smoothed level).
    pub forecast: usize,
    /// `json` | `csv` | `svg`.
    pub output: String,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            mode: "alpha".into(),
            alpha: 0.3,
            span: 5.0,
            halflife: 3.0,
            com: 2.0,
            adjust: true,
            ignore_na: false,
            min_periods: 0,
            forecast: 0,
            output: "json".into(),
        }
    }
}

// ---------------------------------------------------------------------------
// Number formatting
// ---------------------------------------------------------------------------

/// Round to 6 significant digits. Every reported number goes through this so
/// the output is stable and free of float noise like `1.6666666666666667`.
fn round6(v: f64) -> f64 {
    if v == 0.0 || !v.is_finite() {
        return v;
    }
    let exp = v.abs().log10().floor() as i32;
    // Guard the extremes: powi overflows past ~±308.
    if !(-300..=300).contains(&exp) {
        return v;
    }
    let p = 10f64.powi(5 - exp);
    (v * p).round() / p
}

fn fmt_num(v: f64) -> String {
    let r = round6(v);
    if r == r.trunc() && r.abs() < 1e15 {
        format!("{}", r as i64)
    } else {
        format!("{r}")
    }
}

fn fmt_opt(v: Option<f64>) -> String {
    match v {
        Some(x) => fmt_num(x),
        None => "null".into(),
    }
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// A token that stands for "no observation here".
fn is_missing_token(t: &str) -> bool {
    matches!(
        t.to_ascii_lowercase().as_str(),
        "na" | "n/a" | "nan" | "null" | "none" | "nil" | "-" | "." | "?"
    )
}

/// A token that looks like a column header rather than a datum.
fn is_header_token(t: &str) -> bool {
    !t.is_empty() && t.chars().all(|c| c.is_ascii_alphabetic() || c == '_' || c == ' ')
}

/// Split on any of comma / semicolon / whitespace / JSON punctuation, so a
/// pasted column, a one-line list and a JSON array of numbers all work.
fn tokenize(text: &str) -> Vec<String> {
    text.split(|c: char| {
        c.is_whitespace() || matches!(c, ',' | ';' | '|' | '[' | ']' | '"' | '\'')
    })
    .filter(|t| !t.is_empty())
    .map(|t| t.to_string())
    .collect()
}

/// Parse the series into aligned `(value, present)` vectors.
fn parse_series(text: &str) -> Result<(Vec<f64>, Vec<bool>), String> {
    if text.len() > MAX_BYTES {
        return Err(format!(
            "series is too large ({} bytes); the limit is {MAX_BYTES} bytes",
            text.len()
        ));
    }
    let mut tokens = tokenize(text);
    if tokens.is_empty() {
        return Err("series is empty; paste at least one numeric value".into());
    }
    // Drop a leading text header such as "value" or "sales".
    if is_header_token(&tokens[0]) && !is_missing_token(&tokens[0]) {
        tokens.remove(0);
    }
    if tokens.is_empty() {
        return Err("series is empty; paste at least one numeric value".into());
    }
    if tokens.len() > MAX_POINTS {
        return Err(format!(
            "series has {} points; the limit is {MAX_POINTS}",
            tokens.len()
        ));
    }

    let mut values = Vec::with_capacity(tokens.len());
    let mut present = Vec::with_capacity(tokens.len());
    for (i, t) in tokens.iter().enumerate() {
        if is_missing_token(t) {
            values.push(f64::NAN);
            present.push(false);
            continue;
        }
        // Tolerate thousands separators inside an otherwise numeric token.
        let cleaned: String = t.chars().filter(|c| *c != '_').collect();
        match cleaned.parse::<f64>() {
            Ok(v) if v.is_finite() => {
                values.push(v);
                present.push(true);
            }
            Ok(_) => {
                return Err(format!(
                    "value {} at position {} is not finite",
                    t,
                    i + 1
                ))
            }
            Err(_) => {
                return Err(format!(
                    "could not read \"{}\" at position {} as a number (use na, null or - for a gap)",
                    t,
                    i + 1
                ))
            }
        }
    }
    if !present.iter().any(|p| *p) {
        return Err("series contains no numeric values, only gaps".into());
    }
    Ok((values, present))
}

// ---------------------------------------------------------------------------
// EWMA
// ---------------------------------------------------------------------------

/// The core EWMA recursion, written so that `adjust` and `ignore_na` are exact
/// (not approximated by a post-hoc correction).
///
/// `new_wt` is 1 when adjusting (weights accumulate and are divided out) and
/// `alpha` otherwise (the weight of the incoming point in the plain recursion).
pub fn ewma(
    values: &[f64],
    present: &[bool],
    alpha: f64,
    adjust: bool,
    ignore_na: bool,
    min_periods: usize,
) -> Vec<Option<f64>> {
    let n = values.len();
    let mut out = vec![None; n];
    if n == 0 {
        return out;
    }
    let minp = min_periods.max(1);
    let decay = 1.0 - alpha;
    let new_wt = if adjust { 1.0 } else { alpha };

    let mut weighted = f64::NAN;
    let mut have = false;
    let mut old_wt = 1.0;
    let mut nobs = 0usize;

    for i in 0..n {
        let is_obs = present[i];
        if is_obs {
            nobs += 1;
        }
        if have {
            if is_obs || !ignore_na {
                old_wt *= decay;
                if is_obs {
                    // Skip the update when the values already agree: it is a
                    // no-op numerically but keeps the weight bookkeeping exact.
                    if weighted != values[i] {
                        weighted = (old_wt * weighted + new_wt * values[i]) / (old_wt + new_wt);
                    }
                    if adjust {
                        old_wt += new_wt;
                    } else {
                        old_wt = 1.0;
                    }
                }
            }
        } else if is_obs {
            weighted = values[i];
            have = true;
            old_wt = 1.0;
        }
        if have && nobs >= minp {
            out[i] = Some(weighted);
        }
    }
    out
}

/// One-step-ahead forecast errors: the forecast for period `t` is the smoothed
/// level after period `t-1`. Returns `(sse, sae, sape, count, ape_count)`.
fn error_terms(
    values: &[f64],
    present: &[bool],
    smoothed: &[Option<f64>],
) -> (f64, f64, f64, usize, usize) {
    let (mut sse, mut sae, mut sape) = (0.0, 0.0, 0.0);
    let (mut n, mut n_ape) = (0usize, 0usize);
    for t in 1..values.len() {
        if !present[t] {
            continue;
        }
        let Some(f) = smoothed[t - 1] else { continue };
        let e = values[t] - f;
        sse += e * e;
        sae += e.abs();
        n += 1;
        if values[t] != 0.0 {
            sape += (e / values[t]).abs();
            n_ape += 1;
        }
    }
    (sse, sae, sape, n, n_ape)
}

/// Fit alpha by minimising the one-step-ahead SSE — the standard way an SES
/// smoothing constant is chosen. A coarse grid brackets the minimum (the SSE
/// curve is not guaranteed unimodal), then golden-section refines it.
fn fit_alpha(values: &[f64], present: &[bool], adjust: bool, ignore_na: bool, minp: usize) -> f64 {
    let objective = |a: f64| -> f64 {
        let s = ewma(values, present, a, adjust, ignore_na, minp);
        let (sse, _, _, n, _) = error_terms(values, present, &s);
        if n == 0 {
            f64::INFINITY
        } else {
            sse
        }
    };

    const STEPS: usize = 200; // grid of 0.005 over (0, 1]
    let mut best_a = 1.0 / STEPS as f64;
    let mut best = objective(best_a);
    for i in 2..=STEPS {
        let a = i as f64 / STEPS as f64;
        let v = objective(a);
        if v < best {
            best = v;
            best_a = a;
        }
    }
    if !best.is_finite() {
        return best_a;
    }

    // Golden-section refinement inside the bracketing grid cell.
    let step = 1.0 / STEPS as f64;
    let mut lo = (best_a - step).max(1e-6);
    let mut hi = (best_a + step).min(1.0);
    const INV_PHI: f64 = 0.618_033_988_749_894_9;
    let mut c = hi - (hi - lo) * INV_PHI;
    let mut d = lo + (hi - lo) * INV_PHI;
    let (mut fc, mut fd) = (objective(c), objective(d));
    for _ in 0..60 {
        if hi - lo < 1e-7 {
            break;
        }
        if fc < fd {
            hi = d;
            d = c;
            fd = fc;
            c = hi - (hi - lo) * INV_PHI;
            fc = objective(c);
        } else {
            lo = c;
            c = d;
            fc = fd;
            d = lo + (hi - lo) * INV_PHI;
            fd = objective(d);
        }
    }
    let refined = (lo + hi) / 2.0;
    if objective(refined) <= best {
        refined.clamp(1e-6, 1.0)
    } else {
        best_a
    }
}

// ---------------------------------------------------------------------------
// Option validation
// ---------------------------------------------------------------------------

/// Resolve the requested decay parameterisation down to a single alpha.
/// Returns `(alpha, fitted)`.
fn resolve_alpha(
    opts: &Options,
    values: &[f64],
    present: &[bool],
    minp: usize,
) -> Result<(f64, bool), String> {
    match opts.mode.as_str() {
        "alpha" => {
            if !(opts.alpha > 0.0 && opts.alpha <= 1.0) {
                return Err(format!(
                    "alpha must be greater than 0 and at most 1 (got {})",
                    fmt_num(opts.alpha)
                ));
            }
            Ok((opts.alpha, false))
        }
        "span" => {
            if !(opts.span >= 1.0) || !opts.span.is_finite() {
                return Err(format!(
                    "span must be at least 1 (got {})",
                    fmt_num(opts.span)
                ));
            }
            Ok((2.0 / (opts.span + 1.0), false))
        }
        "halflife" => {
            if !(opts.halflife > 0.0) || !opts.halflife.is_finite() {
                return Err(format!(
                    "halflife must be greater than 0 (got {})",
                    fmt_num(opts.halflife)
                ));
            }
            Ok((1.0 - (-std::f64::consts::LN_2 / opts.halflife).exp(), false))
        }
        "com" => {
            if !(opts.com >= 0.0) || !opts.com.is_finite() {
                return Err(format!(
                    "com (center of mass) must be 0 or greater (got {})",
                    fmt_num(opts.com)
                ));
            }
            Ok((1.0 / (1.0 + opts.com), false))
        }
        "auto" => {
            if present.iter().filter(|p| **p).count() < 2 {
                return Err(
                    "auto needs at least 2 numeric values to fit alpha; set mode=alpha instead"
                        .into(),
                );
            }
            Ok((
                fit_alpha(values, present, opts.adjust, opts.ignore_na, minp),
                true,
            ))
        }
        other => Err(format!(
            "unknown mode \"{other}\"; use alpha, span, halflife, com or auto"
        )),
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

struct Report {
    values: Vec<f64>,
    present: Vec<bool>,
    smoothed: Vec<Option<f64>>,
    forecast: Vec<f64>,
    alpha: f64,
    fitted: bool,
    mode: String,
    adjust: bool,
    ignore_na: bool,
    min_periods: usize,
    missing: usize,
    rmse: Option<f64>,
    mse: Option<f64>,
    mae: Option<f64>,
    mape: Option<f64>,
    sse: Option<f64>,
    err_n: usize,
}

fn render_json(r: &Report) -> String {
    let smoothed: Vec<String> = r.smoothed.iter().map(|v| fmt_opt(*v)).collect();
    let values: Vec<String> = r
        .values
        .iter()
        .zip(&r.present)
        .map(|(v, p)| if *p { fmt_num(*v) } else { "null".into() })
        .collect();
    let forecast: Vec<String> = r.forecast.iter().map(|v| fmt_num(*v)).collect();
    let level = r
        .smoothed
        .iter()
        .rev()
        .find_map(|v| *v)
        .map(fmt_num)
        .unwrap_or_else(|| "null".into());

    let span = 2.0 / r.alpha - 1.0;
    let com = 1.0 / r.alpha - 1.0;
    let halflife = if r.alpha >= 1.0 {
        "null".to_string()
    } else {
        fmt_num(-std::f64::consts::LN_2 / (1.0 - r.alpha).ln())
    };

    let mut s = String::new();
    s.push_str("{\n");
    s.push_str(&format!("  \"count\": {},\n", r.values.len()));
    s.push_str(&format!("  \"missing\": {},\n", r.missing));
    s.push_str(&format!("  \"mode\": \"{}\",\n", json_escape(&r.mode)));
    s.push_str(&format!("  \"alpha\": {},\n", fmt_num(r.alpha)));
    s.push_str(&format!("  \"alpha_fitted\": {},\n", r.fitted));
    s.push_str(&format!("  \"span\": {},\n", fmt_num(span)));
    s.push_str(&format!("  \"halflife\": {halflife},\n"));
    s.push_str(&format!("  \"com\": {},\n", fmt_num(com)));
    s.push_str(&format!("  \"adjust\": {},\n", r.adjust));
    s.push_str(&format!("  \"ignore_na\": {},\n", r.ignore_na));
    s.push_str(&format!("  \"min_periods\": {},\n", r.min_periods));
    s.push_str(&format!("  \"level\": {level},\n"));
    s.push_str(&format!("  \"values\": [{}],\n", values.join(", ")));
    s.push_str(&format!("  \"smoothed\": [{}],\n", smoothed.join(", ")));
    s.push_str(&format!("  \"forecast\": [{}],\n", forecast.join(", ")));
    s.push_str("  \"errors\": {\n");
    s.push_str(&format!("    \"n\": {},\n", r.err_n));
    s.push_str(&format!("    \"sse\": {},\n", fmt_opt(r.sse)));
    s.push_str(&format!("    \"mse\": {},\n", fmt_opt(r.mse)));
    s.push_str(&format!("    \"rmse\": {},\n", fmt_opt(r.rmse)));
    s.push_str(&format!("    \"mae\": {},\n", fmt_opt(r.mae)));
    s.push_str(&format!("    \"mape\": {}\n", fmt_opt(r.mape)));
    s.push_str("  }\n");
    s.push('}');
    s
}

fn render_csv(r: &Report) -> String {
    let mut s = String::from("index,value,smoothed,error\n");
    for i in 0..r.values.len() {
        let value = if r.present[i] {
            fmt_num(r.values[i])
        } else {
            String::new()
        };
        let smoothed = match r.smoothed[i] {
            Some(v) => fmt_num(v),
            None => String::new(),
        };
        let error = if i > 0 && r.present[i] {
            match r.smoothed[i - 1] {
                Some(f) => fmt_num(r.values[i] - f),
                None => String::new(),
            }
        } else {
            String::new()
        };
        s.push_str(&format!("{},{},{},{}\n", i + 1, value, smoothed, error));
    }
    for (k, v) in r.forecast.iter().enumerate() {
        s.push_str(&format!(
            "{},,{},\n",
            r.values.len() + k + 1,
            fmt_num(*v)
        ));
    }
    s
}

fn render_svg(r: &Report) -> String {
    const W: f64 = 720.0;
    const H: f64 = 320.0;
    const PAD: f64 = 32.0;

    let total = r.values.len() + r.forecast.len();
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for (v, p) in r.values.iter().zip(&r.present) {
        if *p {
            lo = lo.min(*v);
            hi = hi.max(*v);
        }
    }
    for v in r.smoothed.iter().flatten().chain(r.forecast.iter()) {
        lo = lo.min(*v);
        hi = hi.max(*v);
    }
    if !lo.is_finite() || !hi.is_finite() {
        lo = 0.0;
        hi = 1.0;
    }
    if (hi - lo).abs() < f64::EPSILON {
        lo -= 1.0;
        hi += 1.0;
    }

    let sx = |i: usize| -> f64 {
        if total <= 1 {
            W / 2.0
        } else {
            PAD + (W - 2.0 * PAD) * (i as f64) / ((total - 1) as f64)
        }
    };
    let sy = |v: f64| -> f64 { H - PAD - (H - 2.0 * PAD) * (v - lo) / (hi - lo) };

    // Raw series: break the polyline at gaps so a missing point is visible.
    let mut raw_paths: Vec<String> = Vec::new();
    let mut run: Vec<String> = Vec::new();
    for i in 0..r.values.len() {
        if r.present[i] {
            run.push(format!("{:.2},{:.2}", sx(i), sy(r.values[i])));
        } else if run.len() > 1 {
            raw_paths.push(run.join(" "));
            run.clear();
        } else {
            run.clear();
        }
    }
    if run.len() > 1 {
        raw_paths.push(run.join(" "));
    }

    let smooth_pts: Vec<String> = r
        .smoothed
        .iter()
        .enumerate()
        .filter_map(|(i, v)| v.map(|v| format!("{:.2},{:.2}", sx(i), sy(v))))
        .collect();

    let mut fc_pts: Vec<String> = Vec::new();
    if !r.forecast.is_empty() {
        if let Some(last) = r.smoothed.iter().enumerate().rev().find_map(|(i, v)| v.map(|v| (i, v)))
        {
            fc_pts.push(format!("{:.2},{:.2}", sx(last.0), sy(last.1)));
        }
        for (k, v) in r.forecast.iter().enumerate() {
            fc_pts.push(format!("{:.2},{:.2}", sx(r.values.len() + k), sy(*v)));
        }
    }

    let mut s = String::new();
    s.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {W} {H}\" width=\"{W}\" height=\"{H}\" role=\"img\" aria-label=\"Exponentially smoothed series\">\n"
    ));
    s.push_str(&format!(
        "  <rect width=\"{W}\" height=\"{H}\" fill=\"#ffffff\"/>\n"
    ));
    s.push_str(&format!(
        "  <line x1=\"{PAD}\" y1=\"{:.2}\" x2=\"{:.2}\" y2=\"{:.2}\" stroke=\"#d0d7de\" stroke-width=\"1\"/>\n",
        H - PAD,
        W - PAD,
        H - PAD
    ));
    for pts in &raw_paths {
        s.push_str(&format!(
            "  <polyline points=\"{pts}\" fill=\"none\" stroke=\"#9aa4b2\" stroke-width=\"1.5\"/>\n"
        ));
    }
    for i in 0..r.values.len() {
        if r.present[i] {
            s.push_str(&format!(
                "  <circle cx=\"{:.2}\" cy=\"{:.2}\" r=\"2.2\" fill=\"#9aa4b2\"/>\n",
                sx(i),
                sy(r.values[i])
            ));
        }
    }
    if smooth_pts.len() > 1 {
        s.push_str(&format!(
            "  <polyline points=\"{}\" fill=\"none\" stroke=\"#1f6feb\" stroke-width=\"2.5\" stroke-linejoin=\"round\"/>\n",
            smooth_pts.join(" ")
        ));
    }
    if fc_pts.len() > 1 {
        s.push_str(&format!(
            "  <polyline points=\"{}\" fill=\"none\" stroke=\"#1f6feb\" stroke-width=\"2\" stroke-dasharray=\"6 4\"/>\n",
            fc_pts.join(" ")
        ));
    }
    s.push_str(&format!(
        "  <text x=\"{PAD}\" y=\"20\" font-family=\"system-ui, sans-serif\" font-size=\"13\" fill=\"#57606a\">EWMA alpha = {}</text>\n",
        fmt_num(r.alpha)
    ));
    s.push_str("</svg>");
    s
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Smooth `series` according to `opts` and render the requested output format.
pub fn smooth(series: &str, opts: &Options) -> Result<String, String> {
    match opts.output.as_str() {
        "json" | "csv" | "svg" => {}
        other => {
            return Err(format!(
                "unknown output \"{other}\"; use json, csv or svg"
            ))
        }
    }
    if opts.forecast > MAX_FORECAST {
        return Err(format!(
            "forecast must be at most {MAX_FORECAST} periods (got {})",
            opts.forecast
        ));
    }

    let (values, present) = parse_series(series)?;
    let n = values.len();
    if opts.min_periods > n {
        return Err(format!(
            "min_periods ({}) is larger than the {n} data points supplied",
            opts.min_periods
        ));
    }
    let minp = opts.min_periods.max(1);

    let (alpha, fitted) = resolve_alpha(opts, &values, &present, minp)?;
    let smoothed = ewma(
        &values,
        &present,
        alpha,
        opts.adjust,
        opts.ignore_na,
        opts.min_periods,
    );

    // Simple exponential smoothing has a flat forecast function: every future
    // period sits at the final smoothed level.
    let level = smoothed.iter().rev().find_map(|v| *v);
    let forecast = match level {
        Some(l) => vec![l; opts.forecast],
        None => Vec::new(),
    };

    let (sse, sae, sape, err_n, ape_n) = error_terms(&values, &present, &smoothed);
    let report = Report {
        missing: present.iter().filter(|p| !**p).count(),
        values,
        present,
        smoothed,
        forecast,
        alpha,
        fitted,
        mode: opts.mode.clone(),
        adjust: opts.adjust,
        ignore_na: opts.ignore_na,
        min_periods: opts.min_periods,
        sse: (err_n > 0).then_some(sse),
        mse: (err_n > 0).then(|| sse / err_n as f64),
        rmse: (err_n > 0).then(|| (sse / err_n as f64).sqrt()),
        mae: (err_n > 0).then(|| sae / err_n as f64),
        mape: (ape_n > 0).then(|| 100.0 * sape / ape_n as f64),
        err_n,
    };

    Ok(match opts.output.as_str() {
        "csv" => render_csv(&report),
        "svg" => render_svg(&report),
        _ => render_json(&report),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(mode: &str) -> Options {
        Options {
            mode: mode.into(),
            ..Default::default()
        }
    }

    #[test]
    fn adjusted_ewma_matches_the_closed_form_weighted_average() {
        // alpha = 0.5, adjust = true:
        //   y0 = 1
        //   y1 = (2 + .5*1) / (1 + .5)        = 1.66667
        //   y2 = (3 + .5*2 + .25*1) / 1.75    = 2.42857
        let s = ewma(&[1.0, 2.0, 3.0], &[true; 3], 0.5, true, false, 0);
        assert_eq!(s[0], Some(1.0));
        assert!((s[1].unwrap() - 5.0 / 3.0).abs() < 1e-12);
        assert!((s[2].unwrap() - 4.25 / 1.75).abs() < 1e-12);
    }

    #[test]
    fn unadjusted_ewma_is_the_plain_ses_recursion() {
        // y0 = 1; y1 = .5*1 + .5*2 = 1.5; y2 = .5*1.5 + .5*3 = 2.25
        let s = ewma(&[1.0, 2.0, 3.0], &[true; 3], 0.5, false, false, 0);
        assert_eq!(s, vec![Some(1.0), Some(1.5), Some(2.25)]);
    }

    #[test]
    fn alpha_one_tracks_the_series_exactly() {
        let s = ewma(&[10.0, 20.0, 30.0], &[true; 3], 1.0, true, false, 0);
        assert_eq!(s, vec![Some(10.0), Some(20.0), Some(30.0)]);
    }

    #[test]
    fn span_halflife_and_com_map_onto_alpha() {
        let json = smooth("1 2 3 4", &Options { span: 5.0, ..opts("span") }).unwrap();
        assert!(json.contains("\"alpha\": 0.333333"), "{json}");
        assert!(json.contains("\"span\": 5"), "{json}");

        let json = smooth("1 2 3 4", &Options { com: 3.0, ..opts("com") }).unwrap();
        assert!(json.contains("\"alpha\": 0.25"), "{json}");
        assert!(json.contains("\"com\": 3"), "{json}");

        // alpha = 1 - exp(-ln2/1) = 0.5
        let json = smooth("1 2 3 4", &Options { halflife: 1.0, ..opts("halflife") }).unwrap();
        assert!(json.contains("\"alpha\": 0.5"), "{json}");
        assert!(json.contains("\"halflife\": 1"), "{json}");
    }

    #[test]
    fn gaps_are_carried_and_ignore_na_changes_the_weighting() {
        // With ignore_na = false the gap still consumes one decay step.
        let kept = ewma(&[1.0, f64::NAN, 3.0], &[true, false, true], 0.5, true, false, 0);
        // old_wt = .5 * .5 = .25 → (0.25*1 + 1*3)/1.25 = 2.6
        assert!((kept[2].unwrap() - 2.6).abs() < 1e-12);
        // The level is defined at every period: a gap carries the last level
        // forward rather than blanking (the reference EWMA behaviour).
        assert_eq!(kept[1], Some(1.0));

        // With ignore_na = true the gap is skipped entirely → same as [1, 3].
        let skipped = ewma(&[1.0, f64::NAN, 3.0], &[true, false, true], 0.5, true, true, 0);
        // old_wt = .5 → (0.5*1 + 1*3)/1.5 = 2.33333
        assert!((skipped[2].unwrap() - 7.0 / 3.0).abs() < 1e-12);
    }

    #[test]
    fn min_periods_blanks_the_warm_up() {
        let s = ewma(&[1.0, 2.0, 3.0, 4.0], &[true; 4], 0.5, true, false, 3);
        assert_eq!(s[0], None);
        assert_eq!(s[1], None);
        assert!(s[2].is_some());
        assert!(s[3].is_some());
    }

    #[test]
    fn auto_mode_fits_a_high_alpha_to_a_pure_trend() {
        // A clean ramp is best tracked by following the data closely.
        let json = smooth("1 2 3 4 5 6 7 8 9 10", &opts("auto")).unwrap();
        assert!(json.contains("\"alpha_fitted\": true"), "{json}");
        let a: f64 = json
            .split("\"alpha\": ")
            .nth(1)
            .unwrap()
            .split(',')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        assert!(a > 0.9, "expected a fast alpha for a ramp, got {a}");
    }

    #[test]
    fn auto_mode_fits_a_low_alpha_to_pure_noise_around_a_mean() {
        let json = smooth("10 -10 10 -10 10 -10 10 -10 10 -10", &opts("auto")).unwrap();
        let a: f64 = json
            .split("\"alpha\": ")
            .nth(1)
            .unwrap()
            .split(',')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        assert!(a < 0.2, "expected a slow alpha for zero-mean noise, got {a}");
    }

    #[test]
    fn forecast_is_flat_at_the_final_level() {
        let json = smooth(
            "1 2 3 4",
            &Options {
                forecast: 3,
                ..opts("alpha")
            },
        )
        .unwrap();
        let fc = json
            .split("\"forecast\": [")
            .nth(1)
            .unwrap()
            .split(']')
            .next()
            .unwrap();
        let parts: Vec<&str> = fc.split(", ").collect();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0], parts[1]);
        assert_eq!(parts[1], parts[2]);
        assert!(json.contains(&format!("\"level\": {}", parts[0])), "{json}");
    }

    #[test]
    fn error_metrics_are_reported() {
        let json = smooth("3 3 3 3", &opts("alpha")).unwrap();
        // A constant series is forecast perfectly after the first point.
        assert!(json.contains("\"sse\": 0"), "{json}");
        assert!(json.contains("\"rmse\": 0"), "{json}");
        assert!(json.contains("\"mape\": 0"), "{json}");
        assert!(json.contains("\"n\": 3"), "{json}");
    }

    #[test]
    fn separators_headers_and_json_arrays_all_parse() {
        for text in [
            "1,2,3",
            "1;2;3",
            "1 2 3",
            "1\n2\n3",
            "1\t2\t3",
            "[1, 2, 3]",
            "value\n1\n2\n3",
        ] {
            let json = smooth(text, &opts("alpha")).unwrap();
            assert!(json.contains("\"count\": 3"), "{text} → {json}");
        }
    }

    #[test]
    fn csv_output_has_a_header_and_one_row_per_point() {
        let csv = smooth(
            "1 2 3",
            &Options {
                output: "csv".into(),
                forecast: 1,
                ..opts("alpha")
            },
        )
        .unwrap();
        let lines: Vec<&str> = csv.trim_end().lines().collect();
        assert_eq!(lines[0], "index,value,smoothed,error");
        assert_eq!(lines.len(), 5); // header + 3 points + 1 forecast
        assert!(lines[1].starts_with("1,1,1,"), "{csv}");
    }

    #[test]
    fn svg_output_is_a_chart() {
        let svg = smooth(
            "1 2 3 4",
            &Options {
                output: "svg".into(),
                ..opts("alpha")
            },
        )
        .unwrap();
        assert!(svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""));
        assert!(svg.ends_with("</svg>"));
        assert!(svg.contains("<polyline"));
    }

    // ---- error paths --------------------------------------------------

    #[test]
    fn an_empty_series_is_rejected() {
        let err = smooth("   ", &opts("alpha")).unwrap_err();
        assert!(err.contains("empty"), "{err}");
    }

    #[test]
    fn a_non_numeric_value_is_rejected_and_named() {
        let err = smooth("1, 2, apples, 4", &opts("alpha")).unwrap_err();
        assert!(err.contains("apples") && err.contains("position 3"), "{err}");
    }

    #[test]
    fn out_of_range_decay_settings_are_rejected() {
        assert!(smooth("1 2", &Options { alpha: 0.0, ..opts("alpha") })
            .unwrap_err()
            .contains("alpha"));
        assert!(smooth("1 2", &Options { alpha: 1.5, ..opts("alpha") })
            .unwrap_err()
            .contains("alpha"));
        assert!(smooth("1 2", &Options { span: 0.5, ..opts("span") })
            .unwrap_err()
            .contains("span"));
        assert!(smooth("1 2", &Options { halflife: 0.0, ..opts("halflife") })
            .unwrap_err()
            .contains("halflife"));
        assert!(smooth("1 2", &Options { com: -1.0, ..opts("com") })
            .unwrap_err()
            .contains("com"));
    }

    #[test]
    fn unknown_mode_and_output_are_rejected() {
        assert!(smooth("1 2", &opts("holt")).unwrap_err().contains("holt"));
        assert!(smooth(
            "1 2",
            &Options {
                output: "pdf".into(),
                ..opts("alpha")
            }
        )
        .unwrap_err()
        .contains("pdf"));
    }

    #[test]
    fn caps_are_enforced() {
        let big: String = (0..MAX_POINTS + 1)
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(smooth(&big, &opts("alpha")).unwrap_err().contains("limit"));

        assert!(smooth(
            "1 2 3",
            &Options {
                forecast: MAX_FORECAST + 1,
                ..opts("alpha")
            }
        )
        .unwrap_err()
        .contains("forecast"));

        assert!(smooth(
            "1 2 3",
            &Options {
                min_periods: 9,
                ..opts("alpha")
            }
        )
        .unwrap_err()
        .contains("min_periods"));
    }

    #[test]
    fn a_series_of_only_gaps_is_rejected() {
        let err = smooth("na, null, -", &opts("alpha")).unwrap_err();
        assert!(err.contains("no numeric values"), "{err}");
    }

    #[test]
    fn auto_needs_two_observations() {
        let err = smooth("5", &opts("auto")).unwrap_err();
        assert!(err.contains("at least 2"), "{err}");
    }
}
