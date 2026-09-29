//! interpolation core — pure compute, shared by the chat skill block, the
//! `gizza` CLI and the browser page. No wafer/wasm-bindgen deps.
//!
//! Builds an interpolant that passes exactly through the supplied (x, y) points
//! and evaluates it at new x values. Five methods are supported:
//!
//! * `linear`     — straight line between neighbouring points.
//! * `cubic`      — classical cubic spline (natural, not-a-knot or clamped end
//!                  conditions) solved through the tridiagonal moment system.
//! * `monotone`   — shape-preserving cubic Hermite (Fritsch–Carlson / PCHIP);
//!                  never overshoots between points.
//! * `polynomial` — the single polynomial of degree n-1 through every point,
//!                  evaluated in Newton form for stability.
//! * `nearest`    — nearest-neighbour step lookup.
//!
//! Everything but `polynomial` is represented as a list of cubic pieces
//! `y = a + b·t + c·t² + d·t³` with `t = x - x_start`, which makes evaluation,
//! derivatives and the printed segment equations share one code path.

// ---------------------------------------------------------------------------
// Limits
// ---------------------------------------------------------------------------

/// Max bytes of `data` text accepted (~2 MB).
pub const MAX_BYTES: usize = 2_000_000;
/// Max data points accepted.
pub const MAX_POINTS: usize = 10_000;
/// Max x values `at` may contain.
pub const MAX_EVAL: usize = 5_000;
/// Max points the `resample` grid may contain.
pub const MAX_RESAMPLE: usize = 5_000;
/// Max points for `method = polynomial` (degree 29 is already far past the
/// point where Runge oscillation makes the fit useless).
pub const MAX_POLY_POINTS: usize = 30;
/// Point count above which a polynomial fit earns a Runge warning.
const RUNGE_WARN_POINTS: usize = 10;
/// Samples used to draw the curve in the SVG chart.
const CURVE_SAMPLES: usize = 240;

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// Everything except the data itself. `Default` matches the descriptor's
/// declared defaults, so the CLI, the chat schema and the page agree.
#[derive(Debug, Clone)]
pub struct Options {
    /// "linear" | "cubic" | "monotone" | "polynomial" | "nearest"
    pub method: String,
    /// x values to evaluate at, separated by commas/spaces/tabs/semicolons/newlines.
    pub at: String,
    /// Cubic-spline end conditions: "natural" | "not-a-knot" | "clamped".
    pub boundary: String,
    /// First derivative at the first point, for `boundary = clamped`.
    pub start_slope: f64,
    /// First derivative at the last point, for `boundary = clamped`.
    pub end_slope: f64,
    /// Outside the data range: "error" | "clamp" | "extend".
    pub extrapolate: String,
    /// Evenly spaced samples across the data range. 0 disables.
    pub resample: usize,
    /// 0 = value, 1 = first derivative, 2 = second derivative.
    pub derivative: usize,
    /// Decimal places for printed numbers (0-12).
    pub decimals: usize,
    /// Include the piecewise coefficients / segment equations in the output.
    pub coefficients: bool,
    /// "values" | "csv" | "json" | "svg"
    pub output: String,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            method: "linear".into(),
            at: String::new(),
            boundary: "natural".into(),
            start_slope: 0.0,
            end_slope: 0.0,
            extrapolate: "error".into(),
            resample: 0,
            derivative: 0,
            decimals: 6,
            coefficients: false,
            output: "values".into(),
        }
    }
}

// ---------------------------------------------------------------------------
// Number formatting
// ---------------------------------------------------------------------------

/// Round for display only — every computation runs at full `f64` precision.
/// Trailing zeros are trimmed, so 2.500000 prints as `2.5`.
fn fmt(v: f64, decimals: usize) -> String {
    if v.is_nan() {
        return "NaN".into();
    }
    if v.is_infinite() {
        return if v > 0.0 {
            "Infinity".into()
        } else {
            "-Infinity".into()
        };
    }
    let mut s = format!("{v:.decimals$}");
    if s.contains('.') {
        s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    }
    if s == "-0" {
        s = "0".into();
    }
    s
}

/// JSON numbers must stay parseable — non-finite values become `null`.
fn json_num(v: f64, decimals: usize) -> String {
    if v.is_finite() {
        fmt(v, decimals)
    } else {
        "null".into()
    }
}

fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
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
    out.push('"');
    out
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

fn parse_number(tok: &str, what: &str) -> Result<f64, String> {
    let t = tok.trim();
    let v: f64 = t
        .parse()
        .map_err(|_| format!("{what}: expected a number, got '{t}'"))?;
    if !v.is_finite() {
        return Err(format!("{what}: expected a finite number, got '{t}'"));
    }
    Ok(v)
}

fn split_fields(line: &str) -> Vec<&str> {
    line.split(|c: char| c == ',' || c == ';' || c.is_whitespace())
        .filter(|s| !s.trim().is_empty())
        .collect()
}

/// Parse the `at` / slope-style free-form numeric list.
fn parse_list(text: &str, what: &str) -> Result<Vec<f64>, String> {
    split_fields(text)
        .into_iter()
        .map(|t| parse_number(t, what))
        .collect()
}

/// A single data point.
#[derive(Debug, Clone, Copy)]
struct Point {
    x: f64,
    y: f64,
}

/// Read the data block. Accepts, in this order of preference:
/// * JSON: `[1,2,3]`, `[[x,y],…]`, `[{"x":…,"y":…},…]`
/// * two or more rows: every row `x,y` (2 fields) or every row `y` (1 field)
/// * a single row: a plain list of y values with x = 1, 2, 3 …
fn parse_points(data: &str) -> Result<(Vec<Point>, bool), String> {
    if data.len() > MAX_BYTES {
        return Err(format!(
            "data is {} bytes, which is over the {MAX_BYTES}-byte limit",
            data.len()
        ));
    }
    let trimmed = data.trim();
    if trimmed.is_empty() {
        return Err("data is empty: give at least two points, e.g. '1,2\\n2,4\\n3,9'".into());
    }

    let raw = if trimmed.starts_with('[') {
        parse_json_points(trimmed)?
    } else {
        parse_text_points(trimmed)?
    };

    if raw.len() < 2 {
        return Err(format!(
            "interpolation needs at least 2 points, got {}",
            raw.len()
        ));
    }
    if raw.len() > MAX_POINTS {
        return Err(format!(
            "{} points is over the {MAX_POINTS}-point limit",
            raw.len()
        ));
    }

    let was_sorted = raw.windows(2).all(|w| w[0].x < w[1].x);
    let mut pts = raw;
    pts.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));
    for w in pts.windows(2) {
        if w[0].x == w[1].x {
            return Err(format!(
                "x values must be unique: x = {} appears more than once",
                fmt(w[0].x, 12)
            ));
        }
    }
    Ok((pts, !was_sorted))
}

/// Minimal JSON reader for the three accepted array shapes. Only numbers,
/// arrays and flat `{"x":…,"y":…}` objects are supported.
fn parse_json_points(text: &str) -> Result<Vec<Point>, String> {
    let inner = text
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .ok_or_else(|| "data looks like JSON but is not a closed [ … ] array".to_string())?;
    let mut items: Vec<String> = Vec::new();
    let mut depth = 0usize;
    let mut cur = String::new();
    for c in inner.chars() {
        match c {
            '[' | '{' => {
                depth += 1;
                cur.push(c);
            }
            ']' | '}' => {
                depth = depth.checked_sub(1).ok_or_else(|| {
                    "data looks like JSON but the brackets are unbalanced".to_string()
                })?;
                cur.push(c);
            }
            ',' if depth == 0 => {
                items.push(std::mem::take(&mut cur));
            }
            c => cur.push(c),
        }
    }
    if depth != 0 {
        return Err("data looks like JSON but the brackets are unbalanced".into());
    }
    if !cur.trim().is_empty() {
        items.push(cur);
    }

    let mut pts = Vec::with_capacity(items.len());
    for (i, item) in items.iter().enumerate() {
        let t = item.trim();
        let what = format!("data item {}", i + 1);
        if let Some(obj) = t.strip_prefix('{').and_then(|s| s.strip_suffix('}')) {
            let mut x = None;
            let mut y = None;
            for field in obj.split(',') {
                let (k, v) = field
                    .split_once(':')
                    .ok_or_else(|| format!("{what}: expected \"x\": number, \"y\": number"))?;
                let key = k.trim().trim_matches('"').to_ascii_lowercase();
                match key.as_str() {
                    "x" => x = Some(parse_number(v, &what)?),
                    "y" => y = Some(parse_number(v, &what)?),
                    other => {
                        return Err(format!(
                            "{what}: unexpected key '{other}', expected x and y"
                        ))
                    }
                }
            }
            match (x, y) {
                (Some(x), Some(y)) => pts.push(Point { x, y }),
                _ => return Err(format!("{what}: an object needs both an x and a y key")),
            }
        } else if let Some(pair) = t.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            let parts: Vec<&str> = pair.split(',').collect();
            if parts.len() != 2 {
                return Err(format!(
                    "{what}: a nested array must hold exactly 2 numbers, got {}",
                    parts.len()
                ));
            }
            pts.push(Point {
                x: parse_number(parts[0], &what)?,
                y: parse_number(parts[1], &what)?,
            });
        } else {
            pts.push(Point {
                x: (i + 1) as f64,
                y: parse_number(t, &what)?,
            });
        }
    }
    Ok(pts)
}

fn parse_text_points(text: &str) -> Result<Vec<Point>, String> {
    let mut rows: Vec<(usize, Vec<&str>)> = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let fields = split_fields(line);
        if fields.is_empty() {
            continue;
        }
        rows.push((i + 1, fields));
    }
    if rows.is_empty() {
        return Err("data has no usable rows".into());
    }

    // Drop a leading header row (any row whose first field is not a number).
    if rows.len() > 1 && rows[0].1[0].trim().parse::<f64>().is_err() {
        rows.remove(0);
    }

    // A single remaining row is a plain list of y values.
    if rows.len() == 1 {
        let (line_no, fields) = &rows[0];
        return fields
            .iter()
            .enumerate()
            .map(|(i, t)| {
                Ok(Point {
                    x: (i + 1) as f64,
                    y: parse_number(t, &format!("line {line_no}, value {}", i + 1))?,
                })
            })
            .collect();
    }

    let widths: Vec<usize> = rows.iter().map(|(_, f)| f.len()).collect();
    let all_pairs = widths.iter().all(|&w| w == 2);
    let all_single = widths.iter().all(|&w| w == 1);
    if !all_pairs && !all_single {
        let (line_no, fields) = rows
            .iter()
            .find(|(_, f)| f.len() != widths[0])
            .expect("mixed widths");
        return Err(format!(
            "every row must have the same shape — 'x,y' pairs or a single y value; line {line_no} has {} fields",
            fields.len()
        ));
    }

    rows.iter()
        .enumerate()
        .map(|(i, (line_no, fields))| {
            if all_pairs {
                Ok(Point {
                    x: parse_number(fields[0], &format!("line {line_no}, x"))?,
                    y: parse_number(fields[1], &format!("line {line_no}, y"))?,
                })
            } else {
                Ok(Point {
                    x: (i + 1) as f64,
                    y: parse_number(fields[0], &format!("line {line_no}, y"))?,
                })
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Interpolants
// ---------------------------------------------------------------------------

/// One cubic piece on `[x_start, x_end]`: `y = a + b·t + c·t² + d·t³`,
/// `t = x - x_start`.
#[derive(Debug, Clone, Copy)]
struct Segment {
    x_start: f64,
    x_end: f64,
    a: f64,
    b: f64,
    c: f64,
    d: f64,
}

impl Segment {
    fn eval(&self, x: f64, order: usize) -> f64 {
        let t = x - self.x_start;
        match order {
            0 => self.a + t * (self.b + t * (self.c + t * self.d)),
            1 => self.b + t * (2.0 * self.c + t * 3.0 * self.d),
            _ => 2.0 * self.c + 6.0 * self.d * t,
        }
    }
}

/// The single polynomial through every point, kept in Newton form for
/// evaluation and expanded to monomial form only for display.
#[derive(Debug, Clone)]
struct Poly {
    centers: Vec<f64>,
    newton: Vec<f64>,
    mono: Vec<f64>,
}

impl Poly {
    fn eval(&self, x: f64, order: usize) -> f64 {
        // p_k(x) = c_k + (x - x_k)·p_{k+1}(x); carry the first two derivatives
        // through the same downward recurrence.
        let n = self.newton.len();
        let mut p = self.newton[n - 1];
        let mut p1 = 0.0f64;
        let mut p2 = 0.0f64;
        for i in (0..n - 1).rev() {
            let t = x - self.centers[i];
            p2 = 2.0 * p1 + t * p2;
            p1 = p + t * p1;
            p = self.newton[i] + t * p;
        }
        match order {
            0 => p,
            1 => p1,
            _ => p2,
        }
    }
}

#[derive(Debug, Clone)]
enum Interp {
    Piecewise(Vec<Segment>),
    Polynomial(Poly),
}

impl Interp {
    fn eval(&self, x: f64, order: usize) -> f64 {
        match self {
            Interp::Polynomial(p) => p.eval(x, order),
            Interp::Piecewise(segs) => {
                // Last segment whose start is <= x; the first segment covers
                // everything below the range (that is the `extend` behaviour).
                let mut lo = 0usize;
                let mut hi = segs.len() - 1;
                while lo < hi {
                    let mid = (lo + hi + 1) / 2;
                    if segs[mid].x_start <= x {
                        lo = mid;
                    } else {
                        hi = mid - 1;
                    }
                }
                segs[lo].eval(x, order)
            }
        }
    }
}

fn linear_segments(p: &[Point]) -> Vec<Segment> {
    p.windows(2)
        .map(|w| Segment {
            x_start: w[0].x,
            x_end: w[1].x,
            a: w[0].y,
            b: (w[1].y - w[0].y) / (w[1].x - w[0].x),
            c: 0.0,
            d: 0.0,
        })
        .collect()
}

/// Nearest-neighbour: two constant pieces per interval, split at the midpoint.
/// A point exactly on a midpoint takes the value of the point to its right.
fn nearest_segments(p: &[Point]) -> Vec<Segment> {
    let mut segs = Vec::with_capacity((p.len() - 1) * 2);
    for w in p.windows(2) {
        let mid = 0.5 * (w[0].x + w[1].x);
        segs.push(Segment {
            x_start: w[0].x,
            x_end: mid,
            a: w[0].y,
            b: 0.0,
            c: 0.0,
            d: 0.0,
        });
        segs.push(Segment {
            x_start: mid,
            x_end: w[1].x,
            a: w[1].y,
            b: 0.0,
            c: 0.0,
            d: 0.0,
        });
    }
    segs
}

/// Solve a tridiagonal system in place (Thomas algorithm).
fn thomas(sub: &[f64], diag: &[f64], sup: &[f64], rhs: &[f64]) -> Result<Vec<f64>, String> {
    let n = diag.len();
    let mut c = vec![0.0; n];
    let mut d = vec![0.0; n];
    if diag[0] == 0.0 {
        return Err("the spline system is singular for this data".into());
    }
    c[0] = sup[0] / diag[0];
    d[0] = rhs[0] / diag[0];
    for i in 1..n {
        let denom = diag[i] - sub[i] * c[i - 1];
        if denom == 0.0 {
            return Err("the spline system is singular for this data".into());
        }
        c[i] = if i + 1 < n { sup[i] / denom } else { 0.0 };
        d[i] = (rhs[i] - sub[i] * d[i - 1]) / denom;
    }
    let mut m = vec![0.0; n];
    m[n - 1] = d[n - 1];
    for i in (0..n - 1).rev() {
        m[i] = d[i] - c[i] * m[i + 1];
    }
    Ok(m)
}

/// Cubic spline via the second-derivative (moment) system.
fn cubic_segments(p: &[Point], boundary: &str, s0: f64, s1: f64) -> Result<Vec<Segment>, String> {
    let n = p.len();
    let h: Vec<f64> = p.windows(2).map(|w| w[1].x - w[0].x).collect();
    let delta: Vec<f64> = p
        .windows(2)
        .map(|w| (w[1].y - w[0].y) / (w[1].x - w[0].x))
        .collect();

    // Interior continuity row i (1 ..= n-2):
    //   h[i-1]·M[i-1] + 2(h[i-1]+h[i])·M[i] + h[i]·M[i+1] = 6(delta[i] − delta[i-1])
    let r = |i: usize| 6.0 * (delta[i] - delta[i - 1]);

    let m: Vec<f64> = if boundary == "not-a-knot" {
        if n == 3 {
            // With three points not-a-knot degenerates to the unique quadratic
            // through them, whose second derivative is constant.
            let a2 = (delta[1] - delta[0]) / (p[2].x - p[0].x);
            vec![2.0 * a2; 3]
        } else {
            // The end conditions make M0 and M[n-1] linear in their neighbours,
            // so substitute them out and solve the (n-2)-row tridiagonal system
            // in M1 … M[n-2] (eliminating in the other direction leaves a zero
            // pivot on evenly spaced data).
            let nn = n - 2;
            let mut sub = vec![0.0; nn];
            let mut diag = vec![0.0; nn];
            let mut sup = vec![0.0; nn];
            let mut rhs = vec![0.0; nn];
            for j in 0..nn {
                let i = j + 1;
                sub[j] = h[i - 1];
                diag[j] = 2.0 * (h[i - 1] + h[i]);
                sup[j] = h[i];
                rhs[j] = r(i);
            }
            // M0 = ((h0+h1)·M1 − h0·M2)/h1
            let (h0, h1) = (h[0], h[1]);
            diag[0] = h0 * (h0 + h1) / h1 + 2.0 * (h0 + h1);
            sup[0] = h1 - h0 * h0 / h1;
            sub[0] = 0.0;
            // M[n-1] = ((ha+hb)·M[n-2] − hb·M[n-3])/ha
            let (ha, hb) = (h[n - 3], h[n - 2]);
            diag[nn - 1] = 2.0 * (ha + hb) + hb * (ha + hb) / ha;
            sub[nn - 1] = ha - hb * hb / ha;
            sup[nn - 1] = 0.0;

            let inner = thomas(&sub, &diag, &sup, &rhs)?;
            let mut m = vec![0.0; n];
            m[1..n - 1].copy_from_slice(&inner);
            m[0] = ((h0 + h1) * m[1] - h0 * m[2]) / h1;
            m[n - 1] = ((ha + hb) * m[n - 2] - hb * m[n - 3]) / ha;
            m
        }
    } else {
        let mut sub = vec![0.0; n];
        let mut diag = vec![0.0; n];
        let mut sup = vec![0.0; n];
        let mut rhs = vec![0.0; n];
        for i in 1..n - 1 {
            sub[i] = h[i - 1];
            diag[i] = 2.0 * (h[i - 1] + h[i]);
            sup[i] = h[i];
            rhs[i] = r(i);
        }
        if boundary == "clamped" {
            diag[0] = 2.0 * h[0];
            sup[0] = h[0];
            rhs[0] = 6.0 * (delta[0] - s0);
            sub[n - 1] = h[n - 2];
            diag[n - 1] = 2.0 * h[n - 2];
            rhs[n - 1] = 6.0 * (s1 - delta[n - 2]);
        } else {
            // natural: zero curvature at both ends.
            diag[0] = 1.0;
            diag[n - 1] = 1.0;
        }
        thomas(&sub, &diag, &sup, &rhs)?
    };

    Ok((0..n - 1)
        .map(|i| Segment {
            x_start: p[i].x,
            x_end: p[i + 1].x,
            a: p[i].y,
            b: delta[i] - h[i] * (2.0 * m[i] + m[i + 1]) / 6.0,
            c: m[i] / 2.0,
            d: (m[i + 1] - m[i]) / (6.0 * h[i]),
        })
        .collect())
}

/// Shape-preserving cubic Hermite (Fritsch–Carlson / PCHIP).
fn monotone_segments(p: &[Point]) -> Vec<Segment> {
    let n = p.len();
    let h: Vec<f64> = p.windows(2).map(|w| w[1].x - w[0].x).collect();
    let delta: Vec<f64> = p
        .windows(2)
        .map(|w| (w[1].y - w[0].y) / (w[1].x - w[0].x))
        .collect();
    let mut m = vec![0.0; n];

    if n == 2 {
        m[0] = delta[0];
        m[1] = delta[0];
    } else {
        for i in 1..n - 1 {
            if delta[i - 1] * delta[i] <= 0.0 {
                m[i] = 0.0;
            } else {
                let w1 = 2.0 * h[i] + h[i - 1];
                let w2 = h[i] + 2.0 * h[i - 1];
                m[i] = (w1 + w2) / (w1 / delta[i - 1] + w2 / delta[i]);
            }
        }
        m[0] = endpoint_slope(h[0], h[1], delta[0], delta[1]);
        m[n - 1] = endpoint_slope(h[n - 2], h[n - 3], delta[n - 2], delta[n - 3]);
    }

    (0..n - 1)
        .map(|i| Segment {
            x_start: p[i].x,
            x_end: p[i + 1].x,
            a: p[i].y,
            b: m[i],
            c: (3.0 * delta[i] - 2.0 * m[i] - m[i + 1]) / h[i],
            d: (m[i] + m[i + 1] - 2.0 * delta[i]) / (h[i] * h[i]),
        })
        .collect()
}

/// One-sided three-point endpoint slope, clipped so the end piece stays
/// monotone (the standard PCHIP end rule).
fn endpoint_slope(h_near: f64, h_far: f64, d_near: f64, d_far: f64) -> f64 {
    let mut s = ((2.0 * h_near + h_far) * d_near - h_near * d_far) / (h_near + h_far);
    if s * d_near <= 0.0 {
        s = 0.0;
    } else if d_near * d_far <= 0.0 && s.abs() > (3.0 * d_near).abs() {
        s = 3.0 * d_near;
    }
    s
}

fn build_polynomial(p: &[Point]) -> Poly {
    let n = p.len();
    let centers: Vec<f64> = p.iter().map(|q| q.x).collect();
    // Newton divided differences, computed in place.
    let mut c: Vec<f64> = p.iter().map(|q| q.y).collect();
    for j in 1..n {
        for i in (j..n).rev() {
            c[i] = (c[i] - c[i - 1]) / (centers[i] - centers[i - j]);
        }
    }
    // Expand to monomial form for the printed equation.
    let mut mono = vec![c[n - 1]];
    for i in (0..n - 1).rev() {
        let mut next = vec![0.0; mono.len() + 1];
        for (k, coeff) in mono.iter().enumerate() {
            next[k + 1] += coeff;
            next[k] -= centers[i] * coeff;
        }
        next[0] += c[i];
        mono = next;
    }
    while mono.len() > 1 && mono.last() == Some(&0.0) {
        mono.pop();
    }
    Poly {
        centers,
        newton: c,
        mono,
    }
}

// ---------------------------------------------------------------------------
// Report
// ---------------------------------------------------------------------------

struct Evaluation {
    x: f64,
    value: f64,
    source: &'static str,
    extrapolated: bool,
}

struct Report {
    method: String,
    boundary: Option<String>,
    extrapolate: String,
    derivative: usize,
    decimals: usize,
    n_points: usize,
    x_min: f64,
    x_max: f64,
    evaluations: Vec<Evaluation>,
    interp: Interp,
    points: Vec<Point>,
    coefficients: bool,
    warnings: Vec<String>,
}

impl Report {
    /// Column/label for the reported number, which depends on `derivative`.
    fn value_label(&self) -> &'static str {
        match self.derivative {
            0 => "y",
            1 => "dy/dx",
            _ => "d2y/dx2",
        }
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Interpolate `data` and evaluate the result, rendering per `opts.output`.
pub fn interpolate(data: &str, opts: &Options) -> Result<String, String> {
    let method = opts.method.trim().to_ascii_lowercase();
    if !matches!(
        method.as_str(),
        "linear" | "cubic" | "monotone" | "polynomial" | "nearest"
    ) {
        return Err(format!(
            "method must be linear, cubic, monotone, polynomial, or nearest (got '{method}')"
        ));
    }
    let boundary = opts.boundary.trim().to_ascii_lowercase();
    if !matches!(boundary.as_str(), "natural" | "not-a-knot" | "clamped") {
        return Err(format!(
            "boundary must be natural, not-a-knot, or clamped (got '{boundary}')"
        ));
    }
    let extrapolate = opts.extrapolate.trim().to_ascii_lowercase();
    if !matches!(extrapolate.as_str(), "error" | "clamp" | "extend") {
        return Err(format!(
            "extrapolate must be error, clamp, or extend (got '{extrapolate}')"
        ));
    }
    let output = opts.output.trim().to_ascii_lowercase();
    if !matches!(output.as_str(), "values" | "csv" | "json" | "svg") {
        return Err(format!(
            "output must be values, csv, json, or svg (got '{output}')"
        ));
    }
    if opts.derivative > 2 {
        return Err(format!(
            "derivative must be 0, 1, or 2 (got {})",
            opts.derivative
        ));
    }
    if opts.decimals > 12 {
        return Err(format!("decimals must be 0-12 (got {})", opts.decimals));
    }
    if opts.resample == 1 {
        return Err("resample must be 0 (off) or at least 2".into());
    }
    if opts.resample > MAX_RESAMPLE {
        return Err(format!(
            "resample must be at most {MAX_RESAMPLE} (got {})",
            opts.resample
        ));
    }
    if !opts.start_slope.is_finite() || !opts.end_slope.is_finite() {
        return Err("start_slope and end_slope must be finite numbers".into());
    }

    let (points, was_resorted) = parse_points(data)?;
    let n = points.len();
    let min_points = match method.as_str() {
        "cubic" => 3,
        _ => 2,
    };
    if n < min_points {
        return Err(format!(
            "method '{method}' needs at least {min_points} points, got {n}"
        ));
    }
    if method == "polynomial" && n > MAX_POLY_POINTS {
        return Err(format!(
            "method 'polynomial' is capped at {MAX_POLY_POINTS} points (got {n}) — use cubic or monotone for larger series"
        ));
    }

    let mut warnings: Vec<String> = Vec::new();
    if was_resorted {
        warnings.push("data rows were not in ascending x order and were sorted".into());
    }
    if method == "polynomial" && n > RUNGE_WARN_POINTS {
        warnings.push(format!(
            "a degree-{} polynomial through {n} points can oscillate badly between them (Runge's phenomenon) — cubic or monotone is usually a better fit",
            n - 1
        ));
    }

    let interp = match method.as_str() {
        "linear" => Interp::Piecewise(linear_segments(&points)),
        "nearest" => Interp::Piecewise(nearest_segments(&points)),
        "monotone" => Interp::Piecewise(monotone_segments(&points)),
        "cubic" => Interp::Piecewise(cubic_segments(
            &points,
            &boundary,
            opts.start_slope,
            opts.end_slope,
        )?),
        _ => Interp::Polynomial(build_polynomial(&points)),
    };

    let x_min = points[0].x;
    let x_max = points[n - 1].x;

    // Build the evaluation grid: explicit `at` values first, then the resample
    // grid, then — if neither was given — the midpoint of every interval.
    let at_values = parse_list(&opts.at, "at")?;
    if at_values.len() > MAX_EVAL {
        return Err(format!(
            "at holds {} values, which is over the {MAX_EVAL}-value limit",
            at_values.len()
        ));
    }
    let mut targets: Vec<(f64, &'static str)> = at_values.iter().map(|&x| (x, "at")).collect();
    if opts.resample >= 2 {
        let steps = opts.resample - 1;
        for i in 0..opts.resample {
            let x = x_min + (x_max - x_min) * (i as f64) / (steps as f64);
            targets.push((x, "resample"));
        }
    }
    if targets.is_empty() {
        for w in points.windows(2) {
            targets.push((0.5 * (w[0].x + w[1].x), "midpoint"));
        }
    }

    let mut evaluations = Vec::with_capacity(targets.len());
    let mut extrapolated_count = 0usize;
    for (x, source) in targets {
        let outside = x < x_min || x > x_max;
        if outside {
            match extrapolate.as_str() {
                "error" => {
                    return Err(format!(
                        "x = {} is outside the data range [{}, {}] — set extrapolate to clamp or extend to allow it",
                        fmt(x, opts.decimals),
                        fmt(x_min, opts.decimals),
                        fmt(x_max, opts.decimals)
                    ))
                }
                _ => extrapolated_count += 1,
            }
        }
        let value = if outside && extrapolate == "clamp" {
            let edge = if x < x_min { x_min } else { x_max };
            if opts.derivative == 0 {
                interp.eval(edge, 0)
            } else {
                0.0
            }
        } else {
            interp.eval(x, opts.derivative)
        };
        evaluations.push(Evaluation {
            x,
            value,
            source,
            extrapolated: outside,
        });
    }
    if extrapolated_count > 0 {
        warnings.push(format!(
            "{extrapolated_count} evaluation x value(s) lie outside [{}, {}] and were {}",
            fmt(x_min, opts.decimals),
            fmt(x_max, opts.decimals),
            if extrapolate == "clamp" {
                "clamped to the nearest endpoint"
            } else {
                "extrapolated"
            }
        ));
    }

    let report = Report {
        boundary: (method == "cubic").then(|| boundary.clone()),
        method,
        extrapolate,
        derivative: opts.derivative,
        decimals: opts.decimals,
        n_points: n,
        x_min,
        x_max,
        evaluations,
        interp,
        points,
        coefficients: opts.coefficients,
        warnings,
    };

    Ok(match output.as_str() {
        "values" => render_values(&report),
        "csv" => render_csv(&report),
        "json" => render_json(&report),
        _ => render_svg(&report),
    })
}

// ---------------------------------------------------------------------------
// Equations
// ---------------------------------------------------------------------------

fn term(coeff: f64, var: &str, decimals: usize, first: bool) -> String {
    if coeff == 0.0 {
        return String::new();
    }
    let sign = if coeff < 0.0 {
        if first {
            "-"
        } else {
            " - "
        }
    } else if first {
        ""
    } else {
        " + "
    };
    let mag = fmt(coeff.abs(), decimals);
    if var.is_empty() {
        format!("{sign}{mag}")
    } else if mag == "1" {
        format!("{sign}{var}")
    } else {
        format!("{sign}{mag}{var}")
    }
}

fn segment_equation(s: &Segment, decimals: usize) -> String {
    let base = fmt(s.x_start, decimals);
    let t = if s.x_start == 0.0 {
        "x".to_string()
    } else if s.x_start < 0.0 {
        format!("(x + {})", fmt(-s.x_start, decimals))
    } else {
        format!("(x - {base})")
    };
    let mut out = String::from("y = ");
    let mut first = true;
    for (coeff, var) in [
        (s.a, String::new()),
        (s.b, t.clone()),
        (s.c, format!("{t}^2")),
        (s.d, format!("{t}^3")),
    ] {
        let piece = term(coeff, &var, decimals, first);
        if !piece.is_empty() {
            out.push_str(&piece);
            first = false;
        }
    }
    if first {
        out.push('0');
    }
    out
}

fn polynomial_equation(p: &Poly, decimals: usize) -> String {
    let mut out = String::from("y = ");
    let mut first = true;
    for (k, &coeff) in p.mono.iter().enumerate() {
        let var = match k {
            0 => String::new(),
            1 => "x".into(),
            _ => format!("x^{k}"),
        };
        let piece = term(coeff, &var, decimals, first);
        if !piece.is_empty() {
            out.push_str(&piece);
            first = false;
        }
    }
    if first {
        out.push('0');
    }
    out
}

// ---------------------------------------------------------------------------
// Renderers
// ---------------------------------------------------------------------------

fn render_values(r: &Report) -> String {
    let mut s = String::new();
    for e in &r.evaluations {
        s.push_str(&fmt(e.x, r.decimals));
        s.push(',');
        s.push_str(&fmt(e.value, r.decimals));
        s.push('\n');
    }
    if r.coefficients {
        s.push('\n');
        match &r.interp {
            Interp::Polynomial(p) => {
                s.push_str(&format!(
                    "degree {}: {}\n",
                    p.mono.len().saturating_sub(1),
                    polynomial_equation(p, r.decimals)
                ));
            }
            Interp::Piecewise(segs) => {
                for seg in segs {
                    s.push_str(&format!(
                        "[{}, {}]  {}\n",
                        fmt(seg.x_start, r.decimals),
                        fmt(seg.x_end, r.decimals),
                        segment_equation(seg, r.decimals)
                    ));
                }
            }
        }
    }
    for w in &r.warnings {
        s.push_str(&format!("\n# {w}\n"));
    }
    s
}

fn render_csv(r: &Report) -> String {
    let mut s = format!("x,{},source,extrapolated\n", r.value_label());
    for e in &r.evaluations {
        s.push_str(&format!(
            "{},{},{},{}\n",
            fmt(e.x, r.decimals),
            fmt(e.value, r.decimals),
            e.source,
            e.extrapolated
        ));
    }
    if r.coefficients {
        s.push('\n');
        match &r.interp {
            Interp::Polynomial(p) => {
                s.push_str("power,coefficient\n");
                for (k, &c) in p.mono.iter().enumerate() {
                    s.push_str(&format!("{k},{}\n", fmt(c, r.decimals)));
                }
            }
            Interp::Piecewise(segs) => {
                s.push_str("x_start,x_end,a,b,c,d\n");
                for seg in segs {
                    s.push_str(&format!(
                        "{},{},{},{},{},{}\n",
                        fmt(seg.x_start, r.decimals),
                        fmt(seg.x_end, r.decimals),
                        fmt(seg.a, r.decimals),
                        fmt(seg.b, r.decimals),
                        fmt(seg.c, r.decimals),
                        fmt(seg.d, r.decimals)
                    ));
                }
            }
        }
    }
    if !r.warnings.is_empty() {
        s.push_str("\nwarning\n");
        for w in &r.warnings {
            s.push_str(&format!("{}\n", w.replace(',', ";")));
        }
    }
    s
}

fn render_json(r: &Report) -> String {
    let d = r.decimals;
    let mut s = String::from("{\n");
    s.push_str(&format!("  \"method\": {},\n", json_str(&r.method)));
    if let Some(b) = &r.boundary {
        s.push_str(&format!("  \"boundary\": {},\n", json_str(b)));
    }
    s.push_str(&format!(
        "  \"extrapolate\": {},\n",
        json_str(&r.extrapolate)
    ));
    s.push_str(&format!("  \"derivative\": {},\n", r.derivative));
    s.push_str(&format!(
        "  \"value_label\": {},\n",
        json_str(r.value_label())
    ));
    s.push_str(&format!("  \"n_points\": {},\n", r.n_points));
    s.push_str(&format!("  \"x_min\": {},\n", json_num(r.x_min, d)));
    s.push_str(&format!("  \"x_max\": {},\n", json_num(r.x_max, d)));

    s.push_str("  \"evaluations\": [\n");
    for (i, e) in r.evaluations.iter().enumerate() {
        s.push_str(&format!(
            "    {{\"x\": {}, \"value\": {}, \"source\": {}, \"extrapolated\": {}}}{}\n",
            json_num(e.x, d),
            json_num(e.value, d),
            json_str(e.source),
            e.extrapolated,
            if i + 1 == r.evaluations.len() {
                ""
            } else {
                ","
            }
        ));
    }
    s.push_str("  ]");

    if r.coefficients {
        match &r.interp {
            Interp::Polynomial(p) => {
                s.push_str(",\n  \"polynomial\": {\n");
                s.push_str(&format!(
                    "    \"degree\": {},\n",
                    p.mono.len().saturating_sub(1)
                ));
                let coeffs: Vec<String> = p.mono.iter().map(|&c| json_num(c, d)).collect();
                s.push_str(&format!("    \"coefficients\": [{}],\n", coeffs.join(", ")));
                s.push_str(&format!(
                    "    \"equation\": {}\n  }}",
                    json_str(&polynomial_equation(p, d))
                ));
            }
            Interp::Piecewise(segs) => {
                s.push_str(",\n  \"segments\": [\n");
                for (i, seg) in segs.iter().enumerate() {
                    s.push_str(&format!(
                        "    {{\"x_start\": {}, \"x_end\": {}, \"a\": {}, \"b\": {}, \"c\": {}, \"d\": {}, \"equation\": {}}}{}\n",
                        json_num(seg.x_start, d),
                        json_num(seg.x_end, d),
                        json_num(seg.a, d),
                        json_num(seg.b, d),
                        json_num(seg.c, d),
                        json_num(seg.d, d),
                        json_str(&segment_equation(seg, d)),
                        if i + 1 == segs.len() { "" } else { "," }
                    ));
                }
                s.push_str("  ]");
            }
        }
    }

    if !r.warnings.is_empty() {
        let ws: Vec<String> = r.warnings.iter().map(|w| json_str(w)).collect();
        s.push_str(&format!(",\n  \"warnings\": [{}]", ws.join(", ")));
    }
    s.push_str("\n}");
    s
}

fn render_svg(r: &Report) -> String {
    const W: f64 = 760.0;
    const H: f64 = 380.0;
    const ML: f64 = 60.0;
    const MR: f64 = 20.0;
    const MT: f64 = 34.0;
    const MB: f64 = 40.0;
    let pw = W - ML - MR;
    let ph = H - MT - MB;

    // Horizontal span covers the data and every evaluated x.
    let mut lo = r.x_min;
    let mut hi = r.x_max;
    for e in &r.evaluations {
        lo = lo.min(e.x);
        hi = hi.max(e.x);
    }
    if hi <= lo {
        hi = lo + 1.0;
    }

    let mut curve: Vec<(f64, f64)> = Vec::with_capacity(CURVE_SAMPLES);
    for i in 0..CURVE_SAMPLES {
        let x = lo + (hi - lo) * (i as f64) / ((CURVE_SAMPLES - 1) as f64);
        let y = r.interp.eval(x.clamp(r.x_min, r.x_max), 0);
        curve.push((x, y));
    }

    let mut ylo = f64::INFINITY;
    let mut yhi = f64::NEG_INFINITY;
    for p in &r.points {
        ylo = ylo.min(p.y);
        yhi = yhi.max(p.y);
    }
    for &(_, y) in &curve {
        if y.is_finite() {
            ylo = ylo.min(y);
            yhi = yhi.max(y);
        }
    }
    if !ylo.is_finite() || !yhi.is_finite() {
        ylo = 0.0;
        yhi = 1.0;
    }
    if (yhi - ylo).abs() < f64::EPSILON {
        ylo -= 1.0;
        yhi += 1.0;
    }
    let pad = (yhi - ylo) * 0.08;
    ylo -= pad;
    yhi += pad;

    let sx = |x: f64| ML + (x - lo) / (hi - lo) * pw;
    let sy = |y: f64| MT + ph - (y - ylo) / (yhi - ylo) * ph;

    let mut s = String::new();
    s.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{W}\" height=\"{H}\" viewBox=\"0 0 {W} {H}\" role=\"img\" aria-label=\"Interpolated curve through the data points\">\n"
    ));
    s.push_str(&format!(
        "  <rect width=\"{W}\" height=\"{H}\" fill=\"#ffffff\"/>\n"
    ));
    let title = match &r.boundary {
        Some(b) => format!("{} interpolation ({b}) — {} points", r.method, r.n_points),
        None => format!("{} interpolation — {} points", r.method, r.n_points),
    };
    s.push_str(&format!(
        "  <text x=\"{ML}\" y=\"22\" font-family=\"system-ui, sans-serif\" font-size=\"13\" fill=\"#334155\">{title}</text>\n"
    ));

    // Axis frame + min/max tick labels.
    s.push_str(&format!(
        "  <rect x=\"{ML}\" y=\"{MT}\" width=\"{pw}\" height=\"{ph}\" fill=\"none\" stroke=\"#94a3b8\" stroke-width=\"1\"/>\n"
    ));
    for (i, x) in [lo, (lo + hi) / 2.0, hi].iter().enumerate() {
        let gx = sx(*x);
        if i == 1 {
            s.push_str(&format!(
                "  <line x1=\"{gx:.2}\" y1=\"{MT}\" x2=\"{gx:.2}\" y2=\"{:.2}\" stroke=\"#e2e8f0\" stroke-width=\"1\"/>\n",
                MT + ph
            ));
        }
        s.push_str(&format!(
            "  <text x=\"{gx:.2}\" y=\"{:.2}\" text-anchor=\"middle\" font-family=\"system-ui, sans-serif\" font-size=\"11\" fill=\"#64748b\">{}</text>\n",
            MT + ph + 18.0,
            fmt(*x, 4)
        ));
    }
    for y in [ylo, yhi] {
        let gy = sy(y);
        s.push_str(&format!(
            "  <text x=\"{:.2}\" y=\"{:.2}\" text-anchor=\"end\" font-family=\"system-ui, sans-serif\" font-size=\"11\" fill=\"#64748b\">{}</text>\n",
            ML - 8.0,
            gy + 4.0,
            fmt(y, 4)
        ));
    }

    // The interpolant.
    let mut d = String::new();
    for (i, &(x, y)) in curve.iter().enumerate() {
        if !y.is_finite() {
            continue;
        }
        d.push_str(&format!(
            "{}{:.2} {:.2}",
            if i == 0 { "M" } else { "L" },
            sx(x),
            sy(y.clamp(ylo, yhi))
        ));
        d.push(' ');
    }
    s.push_str(&format!(
        "  <path d=\"{}\" fill=\"none\" stroke=\"#2563eb\" stroke-width=\"2\" stroke-linejoin=\"round\"/>\n",
        d.trim_end()
    ));

    // Data points, then the evaluated points.
    for p in &r.points {
        s.push_str(&format!(
            "  <circle cx=\"{:.2}\" cy=\"{:.2}\" r=\"3.2\" fill=\"#0f172a\"/>\n",
            sx(p.x),
            sy(p.y.clamp(ylo, yhi))
        ));
    }
    if r.derivative == 0 {
        for e in &r.evaluations {
            if e.value.is_finite() {
                s.push_str(&format!(
                    "  <rect x=\"{:.2}\" y=\"{:.2}\" width=\"6\" height=\"6\" fill=\"none\" stroke=\"#dc2626\" stroke-width=\"1.5\"/>\n",
                    sx(e.x) - 3.0,
                    sy(e.value.clamp(ylo, yhi)) - 3.0
                ));
            }
        }
    }

    s.push_str("  <circle cx=\"640\" cy=\"20\" r=\"3\" fill=\"#0f172a\"/>\n");
    s.push_str("  <text x=\"650\" y=\"24\" font-family=\"system-ui, sans-serif\" font-size=\"11\" fill=\"#64748b\">data</text>\n");
    s.push_str("  <line x1=\"686\" y1=\"20\" x2=\"706\" y2=\"20\" stroke=\"#2563eb\" stroke-width=\"2\"/>\n");
    s.push_str("  <text x=\"710\" y=\"24\" font-family=\"system-ui, sans-serif\" font-size=\"11\" fill=\"#64748b\">fit</text>\n");
    s.push_str("</svg>\n");
    s
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(method: &str, at: &str) -> Options {
        Options {
            method: method.into(),
            at: at.into(),
            ..Default::default()
        }
    }

    #[test]
    fn linear_halfway_between_two_points() {
        let out = interpolate("1,10\n3,30", &opts("linear", "2")).unwrap();
        assert_eq!(out, "2,20\n");
    }

    #[test]
    fn linear_defaults_to_interval_midpoints() {
        let out = interpolate("1,10\n3,30\n5,40", &Options::default()).unwrap();
        assert_eq!(out, "2,20\n4,35\n");
    }

    #[test]
    fn cubic_reproduces_a_cubic_exactly() {
        // y = x^3 sampled at 0..4; not-a-knot recovers the generating cubic.
        let data = "0,0\n1,1\n2,8\n3,27\n4,64";
        let mut o = opts("cubic", "1.5, 2.5");
        o.boundary = "not-a-knot".into();
        o.decimals = 6;
        let out = interpolate(data, &o).unwrap();
        assert_eq!(out, "1.5,3.375\n2.5,15.625\n");
    }

    #[test]
    fn natural_spline_has_zero_second_derivative_at_the_ends() {
        let mut o = opts("cubic", "0, 4");
        o.derivative = 2;
        let out = interpolate("0,0\n1,1\n2,8\n3,27\n4,64", &o).unwrap();
        assert_eq!(out, "0,0\n4,0\n");
    }

    #[test]
    fn clamped_spline_honours_the_requested_end_slopes() {
        let mut o = opts("cubic", "0, 4");
        o.boundary = "clamped".into();
        o.start_slope = 0.0;
        o.end_slope = 48.0;
        o.derivative = 1;
        let out = interpolate("0,0\n1,1\n2,8\n3,27\n4,64", &o).unwrap();
        assert_eq!(out, "0,0\n4,48\n");
    }

    #[test]
    fn polynomial_recovers_a_quadratic() {
        let mut o = opts("polynomial", "4");
        o.extrapolate = "extend".into();
        o.coefficients = true;
        let out = interpolate("1,1\n2,4\n3,9", &o).unwrap();
        assert!(out.starts_with("4,16\n\ndegree 2: y = x^2\n"), "{out}");
        assert!(out.contains("were extrapolated"), "{out}");
    }

    #[test]
    fn monotone_never_overshoots_a_step() {
        // A flat run followed by a jump: a natural spline overshoots here,
        // PCHIP must stay inside [0, 1].
        let out = interpolate("0,0\n1,0\n2,0\n3,1\n4,1\n5,1", &opts("monotone", "2.5")).unwrap();
        let y: f64 = out.trim().split(',').nth(1).unwrap().parse().unwrap();
        assert!((0.0..=1.0).contains(&y), "pchip overshot: {y}");
    }

    #[test]
    fn nearest_rounds_a_midpoint_up() {
        let out = interpolate("1,10\n2,20", &opts("nearest", "1.4, 1.5, 1.6")).unwrap();
        assert_eq!(out, "1.4,10\n1.5,20\n1.6,20\n");
    }

    #[test]
    fn y_only_series_uses_one_based_x() {
        let out = interpolate("10\n20\n30", &opts("linear", "2.5")).unwrap();
        assert_eq!(out, "2.5,25\n");
    }

    #[test]
    fn json_shapes_accepted() {
        let pairs = interpolate("[[1,10],[3,30]]", &opts("linear", "2")).unwrap();
        let objs = interpolate(
            "[{\"x\":1,\"y\":10},{\"x\":3,\"y\":30}]",
            &opts("linear", "2"),
        )
        .unwrap();
        let flat = interpolate("[10, 20, 30]", &opts("linear", "1.5")).unwrap();
        assert_eq!(pairs, "2,20\n");
        assert_eq!(objs, "2,20\n");
        assert_eq!(flat, "1.5,15\n");
    }

    #[test]
    fn header_row_is_skipped_and_rows_are_sorted() {
        let mut o = opts("linear", "2");
        o.output = "json".into();
        let out = interpolate("x,y\n3,30\n1,10", &o).unwrap();
        assert!(out.contains("\"value\": 20"), "{out}");
        assert!(out.contains("were sorted"), "{out}");
    }

    #[test]
    fn resample_walks_the_whole_range() {
        let mut o = opts("linear", "");
        o.resample = 5;
        let out = interpolate("0,0\n4,8", &o).unwrap();
        assert_eq!(out, "0,0\n1,2\n2,4\n3,6\n4,8\n");
    }

    #[test]
    fn extrapolate_modes_differ() {
        let mut o = opts("linear", "5");
        assert!(interpolate("1,10\n3,30", &o)
            .unwrap_err()
            .contains("outside the data range"));
        o.extrapolate = "clamp".into();
        let clamped = interpolate("1,10\n3,30", &o).unwrap();
        assert!(clamped.starts_with("5,30\n"), "{clamped}");
        assert!(
            clamped.contains("were clamped to the nearest endpoint"),
            "{clamped}"
        );
        o.extrapolate = "extend".into();
        let extended = interpolate("1,10\n3,30", &o).unwrap();
        assert!(extended.starts_with("5,50\n"), "{extended}");
        assert!(extended.contains("were extrapolated"), "{extended}");
    }

    #[test]
    fn csv_output_carries_source_and_coefficients() {
        let mut o = opts("linear", "2");
        o.output = "csv".into();
        o.coefficients = true;
        let out = interpolate("1,10\n3,30", &o).unwrap();
        assert_eq!(
            out,
            "x,y,source,extrapolated\n2,20,at,false\n\nx_start,x_end,a,b,c,d\n1,3,10,10,0,0\n"
        );
    }

    #[test]
    fn svg_output_draws_points_and_a_curve() {
        let mut o = opts("cubic", "");
        o.output = "svg".into();
        let out = interpolate("0,0\n1,1\n2,8\n3,27", &o).unwrap();
        assert!(out.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""));
        assert!(out.trim_end().ends_with("</svg>"));
        // 4 data points + 3 evaluated midpoints + 1 legend marker.
        assert_eq!(out.matches("<circle").count(), 5);
        assert_eq!(out.matches("<rect").count(), 5);
    }

    #[test]
    fn decimals_control_printed_precision() {
        let mut o = opts("linear", "1.5");
        o.decimals = 2;
        assert_eq!(interpolate("1,1\n2,1.3333333", &o).unwrap(), "1.5,1.17\n");
        o.decimals = 0;
        assert_eq!(interpolate("1,1\n2,1.3333333", &o).unwrap(), "2,1\n");
    }

    // ---- error paths ----

    #[test]
    fn duplicate_x_is_rejected() {
        let err = interpolate("1,10\n1,20\n2,30", &opts("linear", "1.5")).unwrap_err();
        assert!(err.contains("x values must be unique"), "{err}");
    }

    #[test]
    fn cubic_needs_three_points() {
        let err = interpolate("1,10\n2,20", &opts("cubic", "1.5")).unwrap_err();
        assert!(err.contains("needs at least 3 points"), "{err}");
    }

    #[test]
    fn non_numeric_cell_names_the_line() {
        let err = interpolate("1,10\n2,oops\n3,30", &opts("linear", "1.5")).unwrap_err();
        assert!(err.contains("line 2, y"), "{err}");
        assert!(err.contains("got 'oops'"), "{err}");
    }

    #[test]
    fn ragged_rows_are_rejected() {
        let err = interpolate("1,10\n2,20,99\n3,30", &opts("linear", "1.5")).unwrap_err();
        assert!(err.contains("every row must have the same shape"), "{err}");
    }

    #[test]
    fn unknown_option_values_are_rejected() {
        let mut o = opts("spline", "1");
        assert!(interpolate("1,1\n2,2", &o)
            .unwrap_err()
            .contains("method must be"));
        o.method = "linear".into();
        o.output = "yaml".into();
        assert!(interpolate("1,1\n2,2", &o)
            .unwrap_err()
            .contains("output must be"));
        o.output = "values".into();
        o.derivative = 3;
        assert!(interpolate("1,1\n2,2", &o)
            .unwrap_err()
            .contains("derivative must be"));
    }

    #[test]
    fn polynomial_point_cap_is_enforced() {
        let data: String = (0..MAX_POLY_POINTS + 1)
            .map(|i| format!("{i},{i}\n"))
            .collect();
        let err = interpolate(&data, &opts("polynomial", "0.5")).unwrap_err();
        assert!(err.contains("capped at 30 points"), "{err}");
    }

    #[test]
    fn empty_data_explains_the_format() {
        let err = interpolate("   ", &opts("linear", "1")).unwrap_err();
        assert!(err.contains("at least two points"), "{err}");
    }
}
