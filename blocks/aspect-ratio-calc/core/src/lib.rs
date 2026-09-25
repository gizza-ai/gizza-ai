//! aspect-ratio-calc core — solve a missing width or height from a target
//! aspect ratio, simplify a width×height resolution to its reduced ratio, or
//! normalise a ratio on its own.
//!
//! Pure arithmetic: no wafer, no wasm-bindgen, no I/O, so the chat block, the
//! CLI and the browser page all run this exact code.

use serde_json::json;

/// Largest accepted dimension. A million-unit edge is already far past any real
/// asset and keeps the reduction maths inside comfortable float precision.
pub const MAX_DIMENSION: f64 = 1_000_000.0;

/// How a computed dimension is snapped to a usable number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rounding {
    /// Nearest whole unit (default).
    Nearest,
    /// Always round up — never crops.
    Up,
    /// Always round down — never overflows a budget.
    Down,
    /// Nearest even whole unit, which H.264/H.265 encoders require.
    Even,
    /// Keep the fraction (reported to 4 decimals).
    Exact,
}

pub fn parse_rounding(s: &str) -> Result<Rounding, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "nearest" | "round" => Ok(Rounding::Nearest),
        "up" | "ceil" => Ok(Rounding::Up),
        "down" | "floor" => Ok(Rounding::Down),
        "even" => Ok(Rounding::Even),
        "exact" | "none" => Ok(Rounding::Exact),
        other => Err(format!(
            "rounding {other:?} is not one of nearest, up, down, even, exact"
        )),
    }
}

/// What the caller wants back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// Multi-line human report (default).
    Summary,
    /// Just `WIDTHxHEIGHT`.
    Dimensions,
    /// Just the reduced ratio, e.g. `16:9`.
    Ratio,
    /// Just width ÷ height, to 4 decimals.
    Decimal,
    /// A CSS `aspect-ratio` declaration plus the legacy padding-top box.
    Css,
    /// Every field as JSON.
    Json,
}

pub fn parse_output_format(s: &str) -> Result<OutputFormat, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "summary" => Ok(OutputFormat::Summary),
        "dimensions" => Ok(OutputFormat::Dimensions),
        "ratio" => Ok(OutputFormat::Ratio),
        "decimal" => Ok(OutputFormat::Decimal),
        "css" => Ok(OutputFormat::Css),
        "json" => Ok(OutputFormat::Json),
        other => Err(format!(
            "output_format {other:?} is not one of summary, dimensions, ratio, decimal, css, json"
        )),
    }
}

/// Inputs to [`compute`]. A dimension of `0` (or a blank field) means "unknown,
/// solve for it".
#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    /// Target ratio: `16:9`, `4/5`, `1.85:1`, `1920x1080` or a bare decimal
    /// like `1.7778`. Empty means "derive it from width and height".
    pub ratio: String,
    /// Known width, or 0 to solve for it.
    pub width: f64,
    /// Known height, or 0 to solve for it.
    pub height: f64,
    pub rounding: Rounding,
    pub output_format: OutputFormat,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            ratio: String::new(),
            width: 0.0,
            height: 0.0,
            rounding: Rounding::Nearest,
            output_format: OutputFormat::Summary,
        }
    }
}

/// Which job the inputs described.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Both dimensions given, no ratio — reduce them.
    Simplify,
    /// Ratio + height given — solve the width.
    SolveWidth,
    /// Ratio + width given — solve the height.
    SolveHeight,
    /// Ratio only — normalise it, no pixel maths.
    RatioOnly,
    /// Ratio + both dimensions — show both ways to reach the ratio.
    Fit,
}

impl Mode {
    pub fn as_str(self) -> &'static str {
        match self {
            Mode::Simplify => "simplify",
            Mode::SolveWidth => "solve_width",
            Mode::SolveHeight => "solve_height",
            Mode::RatioOnly => "ratio_only",
            Mode::Fit => "fit",
        }
    }
}

/// One row of the standard-ratio reference table.
struct Standard {
    label: &'static str,
    name: &'static str,
    value: f64,
}

/// Standard display / image / cinema ratios, landscape then portrait. Every
/// result names its closest entry so an awkward reduction like `683:384` is
/// still reported as "16:9, 0.05% off".
const STANDARDS: &[Standard] = &[
    Standard {
        label: "1:1",
        name: "Square",
        value: 1.0,
    },
    Standard {
        label: "5:4",
        name: "Classic monitor / 8x10 print",
        value: 1.25,
    },
    Standard {
        label: "4:3",
        name: "Standard / classic TV",
        value: 4.0 / 3.0,
    },
    Standard {
        label: "1.414:1",
        name: "ISO A-series paper, landscape",
        value: 1.414_213_6,
    },
    Standard {
        label: "3:2",
        name: "35 mm photo / DSLR",
        value: 1.5,
    },
    Standard {
        label: "16:10",
        name: "Widescreen monitor",
        value: 1.6,
    },
    Standard {
        label: "5:3",
        name: "Super 16 / 15:9",
        value: 5.0 / 3.0,
    },
    Standard {
        label: "16:9",
        name: "Widescreen HD video",
        value: 16.0 / 9.0,
    },
    Standard {
        label: "1.85:1",
        name: "Cinema flat",
        value: 1.85,
    },
    Standard {
        label: "1.91:1",
        name: "Social link card",
        value: 1.91,
    },
    Standard {
        label: "2:1",
        name: "Univisium",
        value: 2.0,
    },
    Standard {
        label: "21:9",
        name: "Ultrawide (64:27)",
        value: 64.0 / 27.0,
    },
    Standard {
        label: "2.35:1",
        name: "CinemaScope, classic",
        value: 2.35,
    },
    Standard {
        label: "2.39:1",
        name: "Anamorphic scope",
        value: 2.39,
    },
    Standard {
        label: "2.76:1",
        name: "Ultra Panavision",
        value: 2.76,
    },
    Standard {
        label: "3:1",
        name: "Panorama",
        value: 3.0,
    },
    Standard {
        label: "32:9",
        name: "Super ultrawide",
        value: 32.0 / 9.0,
    },
    Standard {
        label: "4:5",
        name: "Portrait photo / social feed",
        value: 0.8,
    },
    Standard {
        label: "3:4",
        name: "Portrait standard",
        value: 0.75,
    },
    Standard {
        label: "1:1.414",
        name: "ISO A-series paper, portrait",
        value: 1.0 / 1.414_213_6,
    },
    Standard {
        label: "2:3",
        name: "Portrait 35 mm photo",
        value: 2.0 / 3.0,
    },
    Standard {
        label: "10:16",
        name: "Portrait widescreen monitor",
        value: 0.625,
    },
    Standard {
        label: "9:16",
        name: "Vertical video / stories",
        value: 9.0 / 16.0,
    },
    Standard {
        label: "1:2",
        name: "Tall portrait",
        value: 0.5,
    },
    Standard {
        label: "9:19.5",
        name: "Tall phone screen",
        value: 9.0 / 19.5,
    },
];

/// A parsed ratio: its decimal value plus, when the input was written as a
/// pair, the two sides so the reduced `w:h` form survives.
struct ParsedRatio {
    value: f64,
    parts: Option<(f64, f64)>,
}

/// Parse a ratio written as `16:9`, `16/9`, `1920x1080`, `1.85:1`, or a bare
/// decimal like `1.7778`. Returns the decimal value.
pub fn parse_ratio(s: &str) -> Result<f64, String> {
    parse_ratio_parts(s).map(|p| p.value)
}

fn parse_ratio_parts(s: &str) -> Result<ParsedRatio, String> {
    let raw = s.trim();
    if raw.is_empty() {
        return Err("ratio must not be empty".into());
    }
    let cleaned = raw.to_ascii_lowercase().replace('×', "x").replace('÷', "/");
    let parsed = match cleaned.find([':', '/', 'x']) {
        Some(i) => {
            let (a, b) = cleaned.split_at(i);
            let w = parse_part(a, raw)?;
            let h = parse_part(&b[1..], raw)?;
            if h == 0.0 {
                return Err(format!(
                    "ratio {raw:?} divides by zero — the second number must be greater than 0"
                ));
            }
            ParsedRatio {
                value: w / h,
                parts: Some((w, h)),
            }
        }
        None => ParsedRatio {
            value: parse_part(&cleaned, raw)?,
            parts: None,
        },
    };
    if !parsed.value.is_finite() || parsed.value <= 0.0 {
        return Err(format!("ratio {raw:?} must describe a positive ratio"));
    }
    Ok(parsed)
}

fn parse_part(s: &str, whole: &str) -> Result<f64, String> {
    s.trim()
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite() && *v >= 0.0)
        .ok_or_else(|| {
            format!(
                "could not read ratio {whole:?} — use a form like 16:9, 4/5, 1.85:1, 1920x1080 or 1.7778"
            )
        })
}

/// A blank page field arrives as an empty string or NaN; treat both, and an
/// explicit 0, as "unknown". Anything negative is a real mistake.
fn check_dimension(v: f64, label: &str) -> Result<f64, String> {
    if v.is_nan() || v == 0.0 {
        return Ok(0.0);
    }
    if !v.is_finite() || v < 0.0 {
        return Err(format!(
            "{label} must be a positive number of pixels (or blank to solve for it), got {v}"
        ));
    }
    if v > MAX_DIMENSION {
        return Err(format!(
            "{label} must be at most {} — that is already far past any real asset",
            MAX_DIMENSION as u64
        ));
    }
    Ok(v)
}

/// Everything a caller could want about one ratio calculation.
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    pub mode: Mode,
    /// Reduced ratio, e.g. `16:9`, or the `1.7778:1` form when the inputs do
    /// not reduce to small whole numbers.
    pub ratio: String,
    pub ratio_decimal: f64,
    pub ratio_x_to_1: String,
    pub orientation: &'static str,
    pub nearest_standard: &'static str,
    pub nearest_standard_name: &'static str,
    pub nearest_standard_deviation_percent: f64,
    /// Resolved dimensions, absent in `ratio_only` mode.
    pub width: Option<f64>,
    pub height: Option<f64>,
    /// Which dimension was computed rather than supplied.
    pub solved: Option<&'static str>,
    /// The unrounded value of the solved dimension, when rounding changed it.
    pub exact_solved: Option<f64>,
    /// `fit` mode only: the other way to reach the ratio (keeping the height).
    pub alt_width: Option<f64>,
    pub alt_height: Option<f64>,
    /// `fit` mode only: the reduced ratio of the frame the caller supplied.
    pub given_ratio: Option<String>,
    pub given_ratio_decimal: Option<f64>,
    pub total_pixels: Option<f64>,
    pub megapixels: Option<f64>,
    pub diagonal: Option<f64>,
    pub css_aspect_ratio: String,
    pub css_padding_top_percent: f64,
}

/// Solve, simplify or normalise, then render per `output_format`.
pub fn compute(opts: &Options) -> Result<String, String> {
    render(&analyze(opts)?, opts.output_format)
}

/// The one entry point the chat block, the CLI and the browser page all call:
/// the raw field values, with the two enums still as strings. A width or height
/// of 0 (or NaN, which is what a blank page field becomes) means "solve it".
pub fn run(
    ratio: &str,
    width: f64,
    height: f64,
    rounding: &str,
    output_format: &str,
) -> Result<String, String> {
    compute(&Options {
        ratio: ratio.to_string(),
        width,
        height,
        rounding: parse_rounding(rounding)?,
        output_format: parse_output_format(output_format)?,
    })
}

/// The calculation itself, independent of how it is printed.
pub fn analyze(opts: &Options) -> Result<Report, String> {
    let width = check_dimension(opts.width, "width")?;
    let height = check_dimension(opts.height, "height")?;
    let ratio_raw = opts.ratio.trim();

    if ratio_raw.is_empty() {
        if width <= 0.0 || height <= 0.0 {
            return Err(
                "nothing to calculate — give a ratio (e.g. 16:9) plus one dimension to solve for \
                 the other, or give both width and height to simplify them to a ratio"
                    .into(),
            );
        }
        return Ok(build(Mode::Simplify, width, height, None, opts));
    }

    let parsed = parse_ratio_parts(ratio_raw)?;
    match (width > 0.0, height > 0.0) {
        (false, false) => Ok(build(Mode::RatioOnly, 0.0, 0.0, Some(&parsed), opts)),
        (true, false) => {
            let h = snap(width / parsed.value, opts.rounding)?;
            Ok(build(Mode::SolveHeight, width, h, Some(&parsed), opts))
        }
        (false, true) => {
            let w = snap(height * parsed.value, opts.rounding)?;
            Ok(build(Mode::SolveWidth, w, height, Some(&parsed), opts))
        }
        (true, true) => Ok(build(Mode::Fit, width, height, Some(&parsed), opts)),
    }
}

fn snap(v: f64, rounding: Rounding) -> Result<f64, String> {
    if !v.is_finite() {
        return Err("the ratio produced a dimension that is not a finite number".into());
    }
    let snapped = match rounding {
        Rounding::Nearest => v.round(),
        Rounding::Up => v.ceil(),
        Rounding::Down => v.floor(),
        Rounding::Even => ((v / 2.0).round() * 2.0).max(2.0),
        Rounding::Exact => return Ok(round4(v)),
    };
    let snapped = snapped.max(if rounding == Rounding::Even { 2.0 } else { 1.0 });
    if snapped > MAX_DIMENSION {
        return Err(format!(
            "that ratio and dimension produce {} — past the {} limit",
            trim_num(snapped),
            MAX_DIMENSION as u64
        ));
    }
    Ok(snapped)
}

fn build(
    mode: Mode,
    width: f64,
    height: f64,
    parsed: Option<&ParsedRatio>,
    opts: &Options,
) -> Report {
    // In simplify mode the ratio IS the reduced frame; otherwise the requested
    // ratio leads and the frame is described alongside it.
    let value = match parsed {
        Some(p) => p.value,
        None => width / height,
    };
    let label = match parsed {
        Some(p) => match p.parts {
            Some((a, b)) => reduce_pair(a, b).unwrap_or_else(|| x_to_1(value)),
            None => x_to_1(value),
        },
        None => reduce_pair(width, height).unwrap_or_else(|| x_to_1(value)),
    };
    let (near, near_dev) = nearest_standard(value);

    let has_dims = mode != Mode::RatioOnly;
    let (solved, exact_solved) = match mode {
        Mode::SolveWidth => (Some("width"), exact_if_rounded(height * value, width)),
        Mode::SolveHeight => (Some("height"), exact_if_rounded(width / value, height)),
        _ => (None, None),
    };
    let (alt_width, alt_height) = if mode == Mode::Fit {
        // Keeping the width gives one frame at the target ratio; keeping the
        // height gives the other. Both are useful, neither is "the" answer.
        (
            snap(height * value, opts.rounding).ok(),
            snap(width / value, opts.rounding).ok(),
        )
    } else {
        (None, None)
    };
    let (given_ratio, given_ratio_decimal) = if mode == Mode::Fit {
        (
            Some(reduce_pair(width, height).unwrap_or_else(|| x_to_1(width / height))),
            Some(round4(width / height)),
        )
    } else {
        (None, None)
    };

    let (css_w, css_h) = css_pair(&label, value);

    Report {
        mode,
        ratio: label,
        ratio_decimal: round4(value),
        ratio_x_to_1: x_to_1(value),
        orientation: orientation_of(value),
        nearest_standard: near.label,
        nearest_standard_name: near.name,
        nearest_standard_deviation_percent: round3(near_dev),
        width: has_dims.then_some(width),
        height: has_dims.then_some(height),
        solved,
        exact_solved,
        alt_width,
        alt_height,
        given_ratio,
        given_ratio_decimal,
        total_pixels: has_dims.then(|| round4(width * height)),
        megapixels: has_dims.then(|| round2(width * height / 1_000_000.0)),
        diagonal: has_dims.then(|| round2((width * width + height * height).sqrt())),
        css_aspect_ratio: format!("{} / {}", trim_num(css_w), trim_num(css_h)),
        css_padding_top_percent: round4(100.0 / value),
    }
}

/// The unrounded value, but only when it actually differs from what shipped.
fn exact_if_rounded(exact: f64, used: f64) -> Option<f64> {
    ((exact - used).abs() > 1e-9).then(|| round4(exact))
}

/// The CSS `aspect-ratio` numerator/denominator: the reduced whole-number pair
/// when there is one, else the decimal against 1.
fn css_pair(label: &str, value: f64) -> (f64, f64) {
    if let Some((a, b)) = label.split_once(':') {
        if let (Ok(a), Ok(b)) = (a.parse::<f64>(), b.parse::<f64>()) {
            if b > 0.0 {
                return (a, b);
            }
        }
    }
    (round4(value), 1.0)
}

fn orientation_of(value: f64) -> &'static str {
    if (value - 1.0).abs() < 1e-9 {
        "square"
    } else if value > 1.0 {
        "landscape"
    } else {
        "portrait"
    }
}

fn nearest_standard(value: f64) -> (&'static Standard, f64) {
    let mut best = &STANDARDS[0];
    let mut best_dev = f64::INFINITY;
    for s in STANDARDS {
        let dev = ((value / s.value) - 1.0).abs() * 100.0;
        if dev < best_dev {
            best_dev = dev;
            best = s;
        }
    }
    (best, best_dev)
}

/// Reduce a pair to small whole numbers, scaling away up to 6 decimal places
/// first so `1.85:1` reduces to `37:20`. `None` when the pair will not land on
/// manageable integers (a long bare decimal, say).
fn reduce_pair(a: f64, b: f64) -> Option<String> {
    if !a.is_finite() || !b.is_finite() || a <= 0.0 || b <= 0.0 {
        return None;
    }
    for k in 0..=6u32 {
        let scale = 10f64.powi(k as i32);
        let (sa, sb) = (a * scale, b * scale);
        if sa > 1e12 || sb > 1e12 {
            return None;
        }
        if is_whole(sa) && is_whole(sb) {
            let (ia, ib) = (sa.round() as u128, sb.round() as u128);
            let g = gcd(ia, ib).max(1);
            let (ra, rb) = (ia / g, ib / g);
            // A reduction into six-figure sides is noise, not a ratio.
            if ra > 999_999 || rb > 999_999 {
                return None;
            }
            return Some(format!("{ra}:{rb}"));
        }
    }
    None
}

fn is_whole(v: f64) -> bool {
    (v - v.round()).abs() < 1e-6 * v.abs().max(1.0)
}

fn gcd(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}

fn x_to_1(value: f64) -> String {
    format!("{}:1", trim_num(round4(value)))
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}
fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}
fn round4(v: f64) -> f64 {
    (v * 10_000.0).round() / 10_000.0
}

/// Print a float without a trailing `.0` so `1080` never reads as `1080.0`.
fn trim_num(v: f64) -> String {
    let s = format!("{v}");
    s.strip_suffix(".0").unwrap_or(&s).to_string()
}

/// Group the integer part with thin commas: `2073600` → `2,073,600`.
fn thousands(v: f64) -> String {
    let s = trim_num(v);
    let (int_part, rest) = match s.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (s.as_str(), None),
    };
    let (sign, digits) = match int_part.strip_prefix('-') {
        Some(d) => ("-", d),
        None => ("", int_part),
    };
    let mut grouped = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(c);
    }
    match rest {
        Some(f) => format!("{sign}{grouped}.{f}"),
        None => format!("{sign}{grouped}"),
    }
}

/// Render a finished report in the requested shape.
pub fn render(r: &Report, format: OutputFormat) -> Result<String, String> {
    match format {
        OutputFormat::Summary => Ok(summary(r)),
        OutputFormat::Dimensions => match (r.width, r.height) {
            (Some(w), Some(h)) => Ok(format!("{}x{}", trim_num(w), trim_num(h))),
            _ => Err(
                "output_format=dimensions needs pixel dimensions — give a width or a height \
                 alongside the ratio"
                    .into(),
            ),
        },
        OutputFormat::Ratio => Ok(r.ratio.clone()),
        OutputFormat::Decimal => Ok(trim_num(r.ratio_decimal)),
        OutputFormat::Css => Ok(format!(
            "aspect-ratio: {};\npadding-top: {}%; /* legacy ratio box */",
            r.css_aspect_ratio,
            trim_num(r.css_padding_top_percent)
        )),
        OutputFormat::Json => Ok(to_json(r)),
    }
}

fn deviation_note(dev: f64) -> String {
    if dev < 0.005 {
        "exact match".into()
    } else {
        format!("{}% off", trim_num(dev))
    }
}

fn summary(r: &Report) -> String {
    let mut lines: Vec<String> = Vec::new();

    if let (Some(which), Some(w), Some(h)) = (r.solved, r.width, r.height) {
        let solved_value = if which == "width" { w } else { h };
        let mut line = format!(
            "{}: {} px",
            if which == "width" { "Width" } else { "Height" },
            trim_num(solved_value)
        );
        if let Some(exact) = r.exact_solved {
            line.push_str(&format!(" (rounded from {})", trim_num(exact)));
        }
        lines.push(line);
    }

    if let (Some(w), Some(h)) = (r.width, r.height) {
        if r.mode == Mode::Fit {
            lines.push(format!(
                "Given: {} x {} px — {} ({})",
                trim_num(w),
                trim_num(h),
                r.given_ratio.clone().unwrap_or_default(),
                x_to_1(r.given_ratio_decimal.unwrap_or_default())
            ));
            if let Some(ah) = r.alt_height {
                lines.push(format!(
                    "At {}, keep the width:  {} x {} px",
                    r.ratio,
                    trim_num(w),
                    trim_num(ah)
                ));
            }
            if let Some(aw) = r.alt_width {
                lines.push(format!(
                    "At {}, keep the height: {} x {} px",
                    r.ratio,
                    trim_num(aw),
                    trim_num(h)
                ));
            }
        } else {
            lines.push(format!("Dimensions: {} x {} px", trim_num(w), trim_num(h)));
        }
    }

    lines.push(format!("Aspect ratio: {} ({})", r.ratio, r.ratio_x_to_1));
    lines.push(format!("Orientation: {}", r.orientation));
    lines.push(format!(
        "Nearest standard: {} — {} ({})",
        r.nearest_standard,
        r.nearest_standard_name,
        deviation_note(r.nearest_standard_deviation_percent)
    ));
    if let (Some(total), Some(mp)) = (r.total_pixels, r.megapixels) {
        lines.push(format!(
            "Total pixels: {} ({} MP)",
            thousands(total),
            trim_num(mp)
        ));
    }
    if let Some(d) = r.diagonal {
        lines.push(format!("Diagonal: {} px", thousands(d)));
    }
    lines.push(format!(
        "CSS: aspect-ratio: {}; (legacy padding-top: {}%)",
        r.css_aspect_ratio,
        trim_num(r.css_padding_top_percent)
    ));
    lines.join("\n")
}

fn to_json(r: &Report) -> String {
    let mut v = json!({
        "mode": r.mode.as_str(),
        "ratio": r.ratio,
        "ratio_decimal": r.ratio_decimal,
        "ratio_x_to_1": r.ratio_x_to_1,
        "orientation": r.orientation,
        "nearest_standard": r.nearest_standard,
        "nearest_standard_name": r.nearest_standard_name,
        "nearest_standard_deviation_percent": r.nearest_standard_deviation_percent,
        "css_aspect_ratio": r.css_aspect_ratio,
        "css_padding_top_percent": r.css_padding_top_percent,
    });
    let map = v.as_object_mut().expect("json! built an object");
    let mut put = |k: &str, value: Option<serde_json::Value>| {
        if let Some(value) = value {
            map.insert(k.into(), value);
        }
    };
    put("width", r.width.map(|x| json!(x)));
    put("height", r.height.map(|x| json!(x)));
    put("solved", r.solved.map(|x| json!(x)));
    put("exact_solved", r.exact_solved.map(|x| json!(x)));
    put("alt_width", r.alt_width.map(|x| json!(x)));
    put("alt_height", r.alt_height.map(|x| json!(x)));
    put("given_ratio", r.given_ratio.clone().map(|x| json!(x)));
    put(
        "given_ratio_decimal",
        r.given_ratio_decimal.map(|x| json!(x)),
    );
    put("total_pixels", r.total_pixels.map(|x| json!(x)));
    put("megapixels", r.megapixels.map(|x| json!(x)));
    put("diagonal", r.diagonal.map(|x| json!(x)));
    serde_json::to_string_pretty(&v).unwrap_or_else(|e| format!("could not encode JSON: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(ratio: &str, width: f64, height: f64) -> Options {
        Options {
            ratio: ratio.into(),
            width,
            height,
            ..Options::default()
        }
    }

    #[test]
    fn simplifies_a_resolution_to_its_ratio() {
        let out = compute(&opts("", 1920.0, 1080.0)).unwrap();
        assert!(out.starts_with("Dimensions: 1920 x 1080 px\n"), "{out}");
        assert!(out.contains("Aspect ratio: 16:9 (1.7778:1)"), "{out}");
        assert!(out.contains("Orientation: landscape"), "{out}");
        assert!(
            out.contains("Nearest standard: 16:9 — Widescreen HD video (exact match)"),
            "{out}"
        );
        assert!(out.contains("Total pixels: 2,073,600 (2.07 MP)"), "{out}");
        assert!(
            out.contains("CSS: aspect-ratio: 16 / 9; (legacy padding-top: 56.25%)"),
            "{out}"
        );
    }

    #[test]
    fn simplify_ratio_only_output() {
        let mut o = opts("", 1366.0, 768.0);
        o.output_format = OutputFormat::Ratio;
        assert_eq!(compute(&o).unwrap(), "683:384");
        o.output_format = OutputFormat::Decimal;
        assert_eq!(compute(&o).unwrap(), "1.7786");
    }

    #[test]
    fn computes_the_missing_height() {
        let mut o = opts("16:9", 1920.0, 0.0);
        o.output_format = OutputFormat::Dimensions;
        assert_eq!(compute(&o).unwrap(), "1920x1080");
        let summary = compute(&opts("16:9", 1920.0, 0.0)).unwrap();
        assert!(summary.starts_with("Height: 1080 px\n"), "{summary}");
    }

    #[test]
    fn computes_the_missing_width() {
        let mut o = opts("16:9", 0.0, 1080.0);
        o.output_format = OutputFormat::Dimensions;
        assert_eq!(compute(&o).unwrap(), "1920x1080");
        let summary = compute(&opts("9:16", 0.0, 1920.0)).unwrap();
        assert!(summary.starts_with("Width: 1080 px\n"), "{summary}");
        assert!(summary.contains("Orientation: portrait"), "{summary}");
    }

    #[test]
    fn a_resolution_works_as_the_ratio_so_resizing_needs_no_extra_field() {
        let mut o = opts("1920x1080", 1280.0, 0.0);
        o.output_format = OutputFormat::Dimensions;
        assert_eq!(compute(&o).unwrap(), "1280x720");
        assert_eq!(
            compute(&opts("1920x1080", 0.0, 0.0))
                .unwrap()
                .lines()
                .next(),
            Some("Aspect ratio: 16:9 (1.7778:1)")
        );
    }

    #[test]
    fn reports_the_unrounded_value_when_rounding_moved_it() {
        let out = compute(&opts("1.85:1", 1920.0, 0.0)).unwrap();
        assert!(
            out.starts_with("Height: 1038 px (rounded from 1037.8378)"),
            "{out}"
        );
        assert!(out.contains("Aspect ratio: 37:20 (1.85:1)"), "{out}");
    }

    #[test]
    fn rounding_modes_change_the_solved_dimension() {
        let mut o = opts("1.85:1", 1920.0, 0.0);
        o.output_format = OutputFormat::Dimensions;
        for (mode, expected) in [
            (Rounding::Nearest, "1920x1038"),
            (Rounding::Up, "1920x1038"),
            (Rounding::Down, "1920x1037"),
            (Rounding::Even, "1920x1038"),
            (Rounding::Exact, "1920x1037.8378"),
        ] {
            o.rounding = mode;
            assert_eq!(compute(&o).unwrap(), expected, "{mode:?}");
        }
        // 3:2 from 1000 wide is 666.67 — the even snap is visible here.
        let mut o = opts("3:2", 1000.0, 0.0);
        o.output_format = OutputFormat::Dimensions;
        o.rounding = Rounding::Even;
        assert_eq!(compute(&o).unwrap(), "1000x666");
    }

    #[test]
    fn fit_mode_shows_both_ways_to_reach_the_ratio() {
        let out = compute(&opts("16:9", 1920.0, 1200.0)).unwrap();
        assert!(out.contains("Given: 1920 x 1200 px — 8:5 (1.6:1)"), "{out}");
        assert!(
            out.contains("At 16:9, keep the width:  1920 x 1080 px"),
            "{out}"
        );
        assert!(
            out.contains("At 16:9, keep the height: 2133 x 1200 px"),
            "{out}"
        );
    }

    #[test]
    fn css_output_is_ready_to_paste() {
        let mut o = opts("16:9", 0.0, 0.0);
        o.output_format = OutputFormat::Css;
        assert_eq!(
            compute(&o).unwrap(),
            "aspect-ratio: 16 / 9;\npadding-top: 56.25%; /* legacy ratio box */"
        );
    }

    #[test]
    fn json_output_carries_the_structured_fields() {
        let mut o = opts("16:9", 1920.0, 0.0);
        o.output_format = OutputFormat::Json;
        let v: serde_json::Value = serde_json::from_str(&compute(&o).unwrap()).unwrap();
        assert_eq!(v["mode"], "solve_height");
        assert_eq!(v["height"], 1080.0);
        assert_eq!(v["solved"], "height");
        assert_eq!(v["ratio"], "16:9");
        assert_eq!(v["megapixels"], 2.07);
        assert!(v.get("alt_width").is_none(), "alt_* is fit-mode only");
    }

    #[test]
    fn bare_decimal_ratios_fall_back_to_the_x_to_1_form() {
        let out = compute(&opts("1.7778", 1920.0, 0.0)).unwrap();
        assert!(out.contains("Aspect ratio: 1.7778:1 (1.7778:1)"), "{out}");
        assert!(out.contains("Nearest standard: 16:9"), "{out}");
    }

    #[test]
    fn nothing_to_calculate_is_an_actionable_error() {
        let e = compute(&opts("", 1920.0, 0.0)).unwrap_err();
        assert!(e.contains("give a ratio"), "{e}");
        assert!(compute(&opts("", 0.0, 0.0)).is_err());
    }

    #[test]
    fn rejects_bad_ratios_and_dimensions() {
        assert!(compute(&opts("16:0", 100.0, 0.0)).is_err());
        assert!(compute(&opts("banana", 100.0, 0.0)).is_err());
        let e = compute(&opts("16:9", -5.0, 0.0)).unwrap_err();
        assert!(e.contains("width must be a positive number"), "{e}");
        let e = compute(&opts("16:9", MAX_DIMENSION + 1.0, 0.0)).unwrap_err();
        assert!(e.contains("at most 1000000"), "{e}");
    }

    #[test]
    fn a_giant_ratio_cannot_overflow_the_dimension_cap() {
        let e = compute(&opts("1000:1", 0.0, 999_999.0)).unwrap_err();
        assert!(e.contains("past the 1000000 limit"), "{e}");
    }

    #[test]
    fn dimensions_output_needs_a_dimension() {
        let mut o = opts("16:9", 0.0, 0.0);
        o.output_format = OutputFormat::Dimensions;
        let e = compute(&o).unwrap_err();
        assert!(e.contains("needs pixel dimensions"), "{e}");
    }

    #[test]
    fn square_and_portrait_orientations_are_named() {
        assert!(compute(&opts("", 500.0, 500.0))
            .unwrap()
            .contains("Orientation: square"));
        assert!(compute(&opts("", 1080.0, 1920.0))
            .unwrap()
            .contains("Orientation: portrait"));
        assert!(compute(&opts("", 1080.0, 1920.0))
            .unwrap()
            .contains("Aspect ratio: 9:16"));
    }

    #[test]
    fn enum_parsers_accept_their_vocabularies_and_reject_others() {
        assert_eq!(parse_rounding("").unwrap(), Rounding::Nearest);
        assert_eq!(parse_rounding("EVEN").unwrap(), Rounding::Even);
        assert!(parse_rounding("sideways").is_err());
        assert_eq!(parse_output_format("").unwrap(), OutputFormat::Summary);
        assert_eq!(parse_output_format("css").unwrap(), OutputFormat::Css);
        assert!(parse_output_format("yaml").is_err());
    }

    #[test]
    fn run_takes_the_raw_field_values() {
        assert_eq!(
            run("16:9", 1920.0, 0.0, "", "dimensions").unwrap(),
            "1920x1080"
        );
        // A blank page number field arrives as NaN; it means "solve for me".
        assert_eq!(
            run("4:5", f64::NAN, 1000.0, "nearest", "dimensions").unwrap(),
            "800x1000"
        );
        assert_eq!(run("", 1920.0, 1080.0, "", "ratio").unwrap(), "16:9");
        assert!(run("16:9", 1920.0, 0.0, "sideways", "summary").is_err());
        assert!(run("16:9", 1920.0, 0.0, "nearest", "yaml").is_err());
    }

    #[test]
    fn ratio_parser_accepts_every_advertised_form() {
        for (input, expected) in [
            ("16:9", 16.0 / 9.0),
            ("16/9", 16.0 / 9.0),
            ("1920x1080", 16.0 / 9.0),
            ("1920×1080", 16.0 / 9.0),
            ("1.85:1", 1.85),
            ("1.7778", 1.7778),
        ] {
            let got = parse_ratio(input).unwrap();
            assert!((got - expected).abs() < 1e-6, "{input} -> {got}");
        }
    }
}
