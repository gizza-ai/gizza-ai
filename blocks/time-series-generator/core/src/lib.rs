//! time-series-generator core — pure compute, shared by the chat skill block
//! and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Builds a synthetic, timestamped series out of four independent layers:
//!
//! 1. **index** — `start` plus `count` steps of `interval`, where the interval
//!    understands calendar units (`1mo`, `1q`, `1y`) as well as fixed ones;
//! 2. **trend** — `none` / `linear` / `exponential` / `logistic` / `random-walk`,
//!    all driven by the single `trend_strength` knob (its unit differs per mode);
//! 3. **seasonality** — one or more superimposed cycles (`period` and
//!    `amplitude` are comma lists, so a daily *and* a weekly cycle need no extra
//!    params), or an explicit 7-number `weekday_pattern`;
//! 4. **noise + data-quality damage** — `gaussian` / `uniform` / `ar1` noise,
//!    then random outliers and random missing cells.
//!
//! Layers 1-3 are the shared signal; every one of the `series` columns rides on
//! that same signal with its OWN noise, outlier and missing streams.
//!
//! `combine = "additive"` sums the layers; `combine = "multiplicative"` reads
//! `amplitude` and `noise_level` as fractions of the level instead.
//!
//! Randomness is a seeded, hand-rolled SplitMix64 PRNG feeding Box-Muller — the
//! same stream on every target, with no `getrandom`, so chat, CLI, page and
//! tests all agree for a given `seed`.

use chrono::{DateTime, Datelike, NaiveDate, NaiveDateTime};
use serde_json::{json, Map, Value};

/// Hard cap on the number of rows so a runaway request can't hang a browser tab.
pub const MAX_COUNT: usize = 100_000;
/// Hard cap on parallel value columns.
pub const MAX_SERIES: usize = 20;
/// Hard cap on total emitted values (`count × series`).
pub const MAX_CELLS: usize = 200_000;
/// Hard cap on rounding precision.
pub const MAX_DECIMALS: usize = 10;
/// Hard cap on superimposed seasonal cycles (entries in `period`).
pub const MAX_COMPONENTS: usize = 8;

const MS_PER_SEC: i64 = 1_000;
const MS_PER_MIN: i64 = 60 * MS_PER_SEC;
const MS_PER_HOUR: i64 = 60 * MS_PER_MIN;
const MS_PER_DAY: i64 = 24 * MS_PER_HOUR;
const MS_PER_WEEK: i64 = 7 * MS_PER_DAY;

// -------------------------------------------------------------------- rng ---

/// SplitMix64 — a tiny deterministic PRNG, identical on every target.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        // Offset the state so seed = 0 isn't a degenerate stream.
        Rng(seed ^ 0x9E37_79B9_7F4A_7C15)
    }
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Uniform double in `[0, 1)`.
    fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / 9_007_199_254_740_992.0 // 2^53
    }
    /// Uniform double in `(0, 1)` — never exactly 0, so `ln()` stays finite.
    fn unit_pos(&mut self) -> f64 {
        ((self.next_u64() >> 11) + 1) as f64 / 9_007_199_254_740_993.0 // 2^53 + 1
    }
    /// Standard normal via Box-Muller (two uniforms per draw, second discarded
    /// so the stream position depends only on the draw count).
    fn normal(&mut self) -> f64 {
        let u1 = self.unit_pos();
        let u2 = self.unit_pos();
        (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
    }
}

/// One independent stream per (purpose, series). Splitting rather than sharing
/// means adding noise doesn't shift where the missing cells land, and column 2
/// keeps its values when column 1 changes.
fn substream(seed: u64, purpose: u64, idx: u64) -> Rng {
    let mut mixer = Rng::new(
        seed ^ purpose.wrapping_mul(0xD1B5_4A32_D192_ED03) ^ idx.wrapping_mul(0xA0761_D6478_BD642),
    );
    // Two hops so neighbouring (purpose, idx) pairs decorrelate.
    let a = mixer.next_u64();
    let mut second = Rng::new(a);
    Rng::new(second.next_u64())
}

// ------------------------------------------------------------------ enums ---

#[derive(Clone, Copy, PartialEq, Debug)]
enum Trend {
    None,
    Linear,
    Exponential,
    Logistic,
    RandomWalk,
}

impl Trend {
    fn parse(s: &str) -> Result<Self, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "none" | "flat" => Trend::None,
            "linear" => Trend::Linear,
            "exponential" | "exp" => Trend::Exponential,
            "logistic" | "s-curve" | "scurve" => Trend::Logistic,
            "random-walk" | "random_walk" | "randomwalk" | "walk" => Trend::RandomWalk,
            other => return Err(format!(
                "trend must be one of none/linear/exponential/logistic/random-walk, got '{other}'"
            )),
        })
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Seasonality {
    None,
    Sine,
    Cosine,
    Square,
    Triangle,
    Sawtooth,
    Weekday,
}

impl Seasonality {
    fn parse(s: &str) -> Result<Self, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "none" => Seasonality::None,
            "sine" | "sin" => Seasonality::Sine,
            "cosine" | "cos" => Seasonality::Cosine,
            "square" => Seasonality::Square,
            "triangle" | "tri" => Seasonality::Triangle,
            "sawtooth" | "saw" => Seasonality::Sawtooth,
            "weekday" | "dayofweek" | "day-of-week" => Seasonality::Weekday,
            other => {
                return Err(format!(
                    "seasonality must be one of none/sine/cosine/square/triangle/sawtooth/weekday, got '{other}'"
                ))
            }
        })
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Noise {
    None,
    Gaussian,
    Uniform,
    Ar1,
}

impl Noise {
    fn parse(s: &str) -> Result<Self, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "none" => Noise::None,
            "gaussian" | "normal" => Noise::Gaussian,
            "uniform" => Noise::Uniform,
            "ar1" | "ar(1)" | "ar" => Noise::Ar1,
            other => {
                return Err(format!(
                    "noise must be one of none/gaussian/uniform/ar1, got '{other}'"
                ))
            }
        })
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Direction {
    Both,
    Up,
    Down,
}

impl Direction {
    fn parse(s: &str) -> Result<Self, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "both" => Direction::Both,
            "up" => Direction::Up,
            "down" => Direction::Down,
            other => {
                return Err(format!(
                    "outlier_direction must be one of both/up/down, got '{other}'"
                ))
            }
        })
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Combine {
    Additive,
    Multiplicative,
}

impl Combine {
    fn parse(s: &str) -> Result<Self, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "additive" | "add" | "sum" => Combine::Additive,
            "multiplicative" | "multiply" | "mul" => Combine::Multiplicative,
            other => {
                return Err(format!(
                    "combine must be one of additive/multiplicative, got '{other}'"
                ))
            }
        })
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Output {
    Csv,
    Tsv,
    Json,
    Ndjson,
    Stats,
}

impl Output {
    fn parse(s: &str) -> Result<Self, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "csv" => Output::Csv,
            "tsv" => Output::Tsv,
            "json" => Output::Json,
            "ndjson" | "jsonl" => Output::Ndjson,
            "stats" => Output::Stats,
            other => {
                return Err(format!(
                    "output must be one of csv/tsv/json/ndjson/stats, got '{other}'"
                ))
            }
        })
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum TsFormat {
    Auto,
    Iso,
    Date,
    Epoch,
    Index,
}

impl TsFormat {
    fn parse(s: &str) -> Result<Self, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => TsFormat::Auto,
            "iso" | "iso8601" | "rfc3339" => TsFormat::Iso,
            "date" => TsFormat::Date,
            "epoch" | "unix" | "seconds" => TsFormat::Epoch,
            "index" | "step" => TsFormat::Index,
            other => {
                return Err(format!(
                    "timestamp_format must be one of auto/iso/date/epoch/index, got '{other}'"
                ))
            }
        })
    }
}

/// A fixed-width step, or a whole number of calendar months (which `1q` and
/// `1y` also reduce to, so month-end dates stay month-end).
#[derive(Clone, Copy, PartialEq, Debug)]
enum Step {
    Fixed(i64),
    Months(i64),
}

// ---------------------------------------------------------------- parsing ---

/// Split `12h` into (12, "h"); a bare unit (`h`) means 1.
fn split_qty_unit(s: &str) -> Result<(i64, String), String> {
    let t = s.trim().to_ascii_lowercase();
    let digits: String = t.chars().take_while(|c| c.is_ascii_digit()).collect();
    let unit: String = t.chars().skip(digits.len()).collect();
    let unit = unit.trim().to_string();
    let n = if digits.is_empty() {
        1
    } else {
        digits
            .parse::<i64>()
            .map_err(|_| format!("interval '{s}' has a number that is too large"))?
    };
    if unit.is_empty() {
        return Err(format!(
            "interval '{s}' is missing a unit — use ms, s, m (minute), h, d, w, mo (month), q or y, e.g. 15m or 1d"
        ));
    }
    Ok((n, unit))
}

/// Parse a step width like `15m`, `1h`, `1d`, `1w`, `1mo`, `1q`, `1y`.
fn parse_interval(s: &str) -> Result<Step, String> {
    let raw = s.trim();
    if raw.is_empty() {
        return Ok(Step::Fixed(MS_PER_DAY));
    }
    if raw.starts_with('-') || raw.starts_with('+') {
        return Err(format!(
            "interval must be positive, got '{raw}' — write 1d, 15m or 1mo"
        ));
    }
    let (n, unit) = split_qty_unit(raw)?;
    if n <= 0 {
        return Err(format!(
            "interval must be a positive multiple of a unit, got '{raw}'"
        ));
    }
    let step = match unit.as_str() {
        "ms" | "milli" | "millis" | "millisecond" | "milliseconds" => Step::Fixed(n),
        "s" | "sec" | "secs" | "second" | "seconds" => Step::Fixed(n * MS_PER_SEC),
        "m" | "min" | "mins" | "minute" | "minutes" => Step::Fixed(n * MS_PER_MIN),
        "h" | "hr" | "hrs" | "hour" | "hours" => Step::Fixed(n * MS_PER_HOUR),
        "d" | "day" | "days" => Step::Fixed(n * MS_PER_DAY),
        "w" | "wk" | "wks" | "week" | "weeks" => Step::Fixed(n * MS_PER_WEEK),
        "mo" | "mon" | "mth" | "month" | "months" => Step::Months(n),
        "q" | "qtr" | "quarter" | "quarters" => Step::Months(n * 3),
        "y" | "yr" | "yrs" | "year" | "years" => Step::Months(n * 12),
        other => {
            return Err(format!(
                "unknown interval unit '{other}' in '{raw}' — use ms, s, m (minute), h, d, w, mo (month), q or y"
            ))
        }
    };
    Ok(step)
}

/// Parse the start timestamp to milliseconds since the Unix epoch (UTC).
/// Accepts RFC-3339 / ISO-8601 (with or without an offset, `T` or space
/// separated), plain `YYYY-MM-DD`, and bare epoch numbers (>= 1e11 is read as
/// milliseconds, otherwise as seconds).
fn parse_start(s: &str) -> Result<i64, String> {
    let t = s.trim();
    if t.is_empty() {
        // Same instant as the descriptor default, so a blank field on the page
        // and an omitted param in chat produce identical rows.
        return Ok(1_704_067_200_000);
    }
    let numeric = t
        .chars()
        .all(|c| c.is_ascii_digit() || c == '-' || c == '+' || c == '.');
    if numeric && !t.trim_start_matches(['-', '+']).contains('-') {
        if let Ok(n) = t.parse::<f64>() {
            if n.is_finite() {
                return Ok(if n.abs() >= 1e11 {
                    n.round() as i64
                } else {
                    (n * 1000.0).round() as i64
                });
            }
        }
    }
    if let Ok(dt) = DateTime::parse_from_rfc3339(t) {
        return Ok(dt.timestamp_millis());
    }
    let norm = t.replacen(' ', "T", 1).replace('/', "-");
    if let Ok(dt) = DateTime::parse_from_rfc3339(&norm) {
        return Ok(dt.timestamp_millis());
    }
    let naive = norm.trim_end_matches('Z');
    for f in [
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%dT%H:%M",
    ] {
        if let Ok(dt) = NaiveDateTime::parse_from_str(naive, f) {
            return Ok(dt.and_utc().timestamp_millis());
        }
    }
    if let Ok(d) = NaiveDate::parse_from_str(naive, "%Y-%m-%d") {
        return Ok(d
            .and_hms_opt(0, 0, 0)
            .ok_or_else(|| format!("start '{s}' is outside the supported date range"))?
            .and_utc()
            .timestamp_millis());
    }
    Err(format!(
        "start '{s}' is not a timestamp — use 2024-01-01, 2024-01-01T09:30:00Z or an epoch number"
    ))
}

/// A comma-, space- or semicolon-separated list of numbers.
fn parse_list(s: &str, what: &str) -> Result<Vec<f64>, String> {
    let mut out = Vec::new();
    for tok in s.split(|c: char| c == ',' || c == ';' || c.is_whitespace()) {
        let tok = tok.trim();
        if tok.is_empty() {
            continue;
        }
        let v: f64 = tok
            .parse()
            .map_err(|_| format!("{what}: '{tok}' is not a number"))?;
        if !v.is_finite() {
            return Err(format!("{what}: '{tok}' is not a finite number"));
        }
        out.push(v);
    }
    Ok(out)
}

/// An optional numeric bound — blank means unbounded.
fn parse_bound(s: &str, what: &str) -> Result<Option<f64>, String> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(None);
    }
    let v: f64 = t
        .parse()
        .map_err(|_| format!("{what} '{t}' is not a number — leave it empty for no limit"))?;
    if !v.is_finite() {
        return Err(format!("{what} '{t}' is not a finite number"));
    }
    Ok(Some(v))
}

fn check_finite(v: f64, what: &str) -> Result<(), String> {
    if v.is_finite() {
        Ok(())
    } else {
        Err(format!("{what} must be a finite number, got {v}"))
    }
}

fn check_range(v: f64, lo: f64, hi: f64, what: &str) -> Result<(), String> {
    check_finite(v, what)?;
    if v < lo || v > hi {
        return Err(format!("{what} must be between {lo} and {hi}, got {v}"));
    }
    Ok(())
}

// ------------------------------------------------------------------- time ---

fn last_day_of_month(y: i32, m: u32) -> u32 {
    let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
    NaiveDate::from_ymd_opt(ny, nm, 1)
        .and_then(|d| d.pred_opt())
        .map(|d| d.day())
        .unwrap_or(28)
}

/// Add whole calendar months, clamping the day to the target month's length
/// (so a Jan-31 start steps to Feb 28/29, not into March).
fn add_months(start_ms: i64, months: i64) -> Option<i64> {
    let dt = DateTime::from_timestamp_millis(start_ms)?;
    let day = dt.date_naive();
    let midnight = day.and_hms_opt(0, 0, 0)?.and_utc().timestamp_millis();
    let time_of_day = start_ms - midnight;
    let total = day.year() as i64 * 12 + day.month0() as i64 + months;
    let y = i32::try_from(total.div_euclid(12)).ok()?;
    let m = (total.rem_euclid(12) + 1) as u32;
    let d = day.day().min(last_day_of_month(y, m));
    let nd = NaiveDate::from_ymd_opt(y, m, d)?;
    nd.and_hms_opt(0, 0, 0)?
        .and_utc()
        .timestamp_millis()
        .checked_add(time_of_day)
}

fn ts_at(start_ms: i64, step: Step, i: usize) -> Result<i64, String> {
    let out = match step {
        Step::Fixed(w) => (i as i64)
            .checked_mul(w)
            .and_then(|d| start_ms.checked_add(d)),
        Step::Months(n) => (i as i64)
            .checked_mul(n)
            .and_then(|d| add_months(start_ms, d)),
    };
    out.filter(|ms| DateTime::from_timestamp_millis(*ms).is_some())
        .ok_or_else(|| {
            "the series runs past the supported date range — lower count or shorten the interval"
                .to_string()
        })
}

fn resolve_ts_format(fmt: TsFormat, step: Step, start_ms: i64) -> TsFormat {
    match fmt {
        TsFormat::Auto => match step {
            Step::Months(_) => TsFormat::Date,
            Step::Fixed(w) if w % MS_PER_DAY == 0 && start_ms.rem_euclid(MS_PER_DAY) == 0 => {
                TsFormat::Date
            }
            Step::Fixed(_) => TsFormat::Iso,
        },
        other => other,
    }
}

fn fmt_ts(ms: i64, i: usize, fmt: TsFormat) -> String {
    match fmt {
        TsFormat::Index => return i.to_string(),
        TsFormat::Epoch => {
            return if ms.rem_euclid(MS_PER_SEC) == 0 {
                (ms / MS_PER_SEC).to_string()
            } else {
                format!("{}", ms as f64 / 1000.0)
            }
        }
        _ => {}
    }
    // Every remaining variant needs the civil date; ts_at already proved it
    // converts, so the fallback is unreachable in practice.
    let Some(dt) = DateTime::from_timestamp_millis(ms) else {
        return ms.to_string();
    };
    match fmt {
        TsFormat::Date => dt.format("%Y-%m-%d").to_string(),
        _ => {
            if ms.rem_euclid(MS_PER_SEC) == 0 {
                dt.format("%Y-%m-%dT%H:%M:%SZ").to_string()
            } else {
                dt.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
            }
        }
    }
}

/// The JSON shape of a timestamp: a number for `epoch`/`index`, a string for
/// the civil formats.
fn ts_json(ms: i64, i: usize, fmt: TsFormat) -> Value {
    match fmt {
        TsFormat::Index => json!(i),
        TsFormat::Epoch => {
            if ms.rem_euclid(MS_PER_SEC) == 0 {
                json!(ms / MS_PER_SEC)
            } else {
                json!(ms as f64 / 1000.0)
            }
        }
        other => json!(fmt_ts(ms, i, other)),
    }
}

// ----------------------------------------------------------------- output ---

fn round_to(v: f64, decimals: usize) -> f64 {
    let f = 10f64.powi(decimals as i32);
    let r = (v * f).round() / f;
    // Kill the "-0" that a tiny negative value rounds to.
    if r == 0.0 {
        0.0
    } else {
        r
    }
}

fn fmt_val(v: f64, decimals: usize) -> String {
    format!("{:.*}", decimals, v)
}

// ------------------------------------------------------------------- spec ---

/// Every input, named — 27 positional arguments across three call sites was
/// asking for a silent mis-ordering.
#[derive(Debug, Clone)]
pub struct Spec<'a> {
    pub start: &'a str,
    pub interval: &'a str,
    pub count: usize,
    pub base: f64,
    pub trend: &'a str,
    pub trend_strength: f64,
    pub seasonality: &'a str,
    pub period: &'a str,
    pub amplitude: &'a str,
    pub weekday_pattern: &'a str,
    pub combine: &'a str,
    pub noise: &'a str,
    pub noise_level: f64,
    pub noise_phi: f64,
    pub missing_rate: f64,
    pub outlier_rate: f64,
    pub outlier_magnitude: f64,
    pub outlier_direction: &'a str,
    pub min_value: &'a str,
    pub max_value: &'a str,
    pub series: usize,
    pub seed: u64,
    pub decimals: usize,
    pub output: &'a str,
    pub timestamp_format: &'a str,
    pub header: bool,
    pub labels: &'a str,
}

impl Default for Spec<'_> {
    /// Mirrors the descriptor defaults exactly — chat, CLI, page and tests all
    /// start from the same series.
    fn default() -> Self {
        Spec {
            start: "2024-01-01",
            interval: "1d",
            count: 100,
            base: 100.0,
            trend: "linear",
            trend_strength: 0.5,
            seasonality: "sine",
            period: "7",
            amplitude: "10",
            weekday_pattern: "1.1, 1.05, 1, 1.05, 1.25, 0.8, 0.7",
            combine: "additive",
            noise: "gaussian",
            noise_level: 5.0,
            noise_phi: 0.7,
            missing_rate: 0.0,
            outlier_rate: 0.0,
            outlier_magnitude: 3.0,
            outlier_direction: "both",
            min_value: "",
            max_value: "",
            series: 1,
            seed: 42,
            decimals: 2,
            output: "csv",
            timestamp_format: "auto",
            header: true,
            labels: "",
        }
    }
}

/// One fully-resolved column: the values (None = a missing cell) plus the
/// damage counters, which `output = "stats"` reports.
struct Column {
    label: String,
    values: Vec<Option<f64>>,
    missing: usize,
    outliers: usize,
}

// --------------------------------------------------------------- generate ---

/// Generate the series and render it in the requested format.
pub fn generate(spec: &Spec) -> Result<String, String> {
    let trend = Trend::parse(spec.trend)?;
    let seasonality = Seasonality::parse(spec.seasonality)?;
    let noise = Noise::parse(spec.noise)?;
    let direction = Direction::parse(spec.outlier_direction)?;
    let combine = Combine::parse(spec.combine)?;
    let output = Output::parse(spec.output)?;
    let ts_format_req = TsFormat::parse(spec.timestamp_format)?;

    if spec.count == 0 || spec.count > MAX_COUNT {
        return Err(format!(
            "count must be between 1 and {MAX_COUNT}, got {}",
            spec.count
        ));
    }
    if spec.series == 0 || spec.series > MAX_SERIES {
        return Err(format!(
            "series must be between 1 and {MAX_SERIES}, got {}",
            spec.series
        ));
    }
    if spec.count * spec.series > MAX_CELLS {
        return Err(format!(
            "count {} x series {} is {} values, over the {MAX_CELLS} cap — lower count or series",
            spec.count,
            spec.series,
            spec.count * spec.series
        ));
    }
    if spec.decimals > MAX_DECIMALS {
        return Err(format!(
            "decimals must be between 0 and {MAX_DECIMALS}, got {}",
            spec.decimals
        ));
    }
    check_finite(spec.base, "base")?;
    check_finite(spec.trend_strength, "trend_strength")?;
    if spec.noise_level < 0.0 || !spec.noise_level.is_finite() {
        return Err(format!(
            "noise_level must be 0 or more, got {}",
            spec.noise_level
        ));
    }
    check_range(spec.noise_phi, -0.99, 0.99, "noise_phi")?;
    check_range(spec.missing_rate, 0.0, 1.0, "missing_rate")?;
    check_range(spec.outlier_rate, 0.0, 1.0, "outlier_rate")?;
    if spec.outlier_magnitude < 0.0 || !spec.outlier_magnitude.is_finite() {
        return Err(format!(
            "outlier_magnitude must be 0 or more, got {}",
            spec.outlier_magnitude
        ));
    }

    let min_value = parse_bound(spec.min_value, "min_value")?;
    let max_value = parse_bound(spec.max_value, "max_value")?;
    if let (Some(lo), Some(hi)) = (min_value, max_value) {
        if lo > hi {
            return Err(format!(
                "min_value {lo} is above max_value {hi} — nothing could satisfy both"
            ));
        }
    }

    let labels = resolve_labels(spec.labels, spec.series)?;
    let step = parse_interval(spec.interval)?;
    let start_ms = parse_start(spec.start)?;
    let ts: Vec<i64> = (0..spec.count)
        .map(|i| ts_at(start_ms, step, i))
        .collect::<Result<_, _>>()?;
    let ts_format = resolve_ts_format(ts_format_req, step, start_ms);

    // ---- layer 2: trend --------------------------------------------------
    let level = trend_levels(trend, spec.base, spec.trend_strength, spec.count, spec.seed)?;

    // ---- layer 3: seasonality -------------------------------------------
    let season = seasonal_terms(seasonality, spec, &ts)?;

    // ---- shared signal ---------------------------------------------------
    let signal: Vec<f64> = (0..spec.count)
        .map(|i| match combine {
            Combine::Additive => level[i] + season[i],
            Combine::Multiplicative => level[i] * (1.0 + season[i]),
        })
        .collect();

    // ---- layer 4: per-column noise + damage ------------------------------
    let mut columns = Vec::with_capacity(spec.series);
    for (s, label) in labels.iter().enumerate() {
        let mut noise_rng = substream(spec.seed, 2, s as u64);
        let mut outlier_rng = substream(spec.seed, 3, s as u64);
        let mut missing_rng = substream(spec.seed, 4, s as u64);
        let mut prev = 0.0f64;
        let mut values = Vec::with_capacity(spec.count);
        let mut missing = 0usize;
        let mut outliers = 0usize;

        for (i, sig) in signal.iter().enumerate() {
            let e = match noise {
                Noise::None => 0.0,
                Noise::Gaussian => spec.noise_level * noise_rng.normal(),
                Noise::Uniform => spec.noise_level * (2.0 * noise_rng.unit() - 1.0),
                Noise::Ar1 => {
                    let z = noise_rng.normal();
                    prev = if i == 0 {
                        // Start on the stationary distribution so the first
                        // rows aren't systematically quieter than the rest.
                        spec.noise_level * z
                    } else {
                        spec.noise_phi * prev
                            + spec.noise_level * (1.0 - spec.noise_phi * spec.noise_phi).sqrt() * z
                    };
                    prev
                }
            };
            let mut v = match combine {
                Combine::Additive => sig + e,
                Combine::Multiplicative => sig * (1.0 + e),
            };

            // Draw unconditionally so the stream position never depends on the
            // rates — raising outlier_rate adds spikes without moving the rest.
            let hit = outlier_rng.unit();
            let side = outlier_rng.unit();
            if hit < spec.outlier_rate {
                let up = match direction {
                    Direction::Up => true,
                    Direction::Down => false,
                    Direction::Both => side < 0.5,
                };
                // Up multiplies, down divides — both keep the sign of v.
                v = if up {
                    v * (1.0 + spec.outlier_magnitude)
                } else {
                    v / (1.0 + spec.outlier_magnitude)
                };
                outliers += 1;
            }

            if let Some(lo) = min_value {
                v = v.max(lo);
            }
            if let Some(hi) = max_value {
                v = v.min(hi);
            }

            if missing_rng.unit() < spec.missing_rate {
                missing += 1;
                values.push(None);
            } else {
                values.push(Some(round_to(v, spec.decimals)));
            }
        }
        columns.push(Column {
            label: label.clone(),
            values,
            missing,
            outliers,
        });
    }

    let ts_col = if ts_format == TsFormat::Index {
        "index"
    } else {
        "timestamp"
    };

    Ok(match output {
        Output::Csv => render_delimited(spec, &ts, ts_col, ts_format, &columns, ','),
        Output::Tsv => render_delimited(spec, &ts, ts_col, ts_format, &columns, '\t'),
        Output::Json => render_json(spec, &ts, ts_col, ts_format, &columns, step),
        Output::Ndjson => render_ndjson(&ts, ts_col, ts_format, &columns),
        Output::Stats => render_stats(
            spec,
            &ts,
            ts_format,
            &columns,
            step,
            trend,
            seasonality,
            noise,
            combine,
        ),
    })
}

/// Column names: explicit `labels`, else `value` for one column and
/// `value_1 … value_n` for several.
fn resolve_labels(labels: &str, series: usize) -> Result<Vec<String>, String> {
    let given: Vec<String> = labels
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if given.is_empty() {
        return Ok(if series == 1 {
            vec!["value".to_string()]
        } else {
            (1..=series).map(|i| format!("value_{i}")).collect()
        });
    }
    if given.len() != series {
        return Err(format!(
            "labels has {} name(s) but series is {series} — give one name per column or leave it empty",
            given.len()
        ));
    }
    Ok(given)
}

/// The trend level at each step. `trend_strength` means: units per step
/// (linear), percent per step (exponential), total rise over the whole series
/// (logistic), or the per-step standard deviation (random-walk).
fn trend_levels(
    trend: Trend,
    base: f64,
    strength: f64,
    count: usize,
    seed: u64,
) -> Result<Vec<f64>, String> {
    Ok(match trend {
        Trend::None => vec![base; count],
        Trend::Linear => (0..count).map(|i| base + strength * i as f64).collect(),
        Trend::Exponential => {
            let rate = 1.0 + strength / 100.0;
            if rate <= 0.0 {
                return Err(format!(
                    "trend_strength {strength} means an exponential rate of {rate} per step — it must be above -100 (percent per step)"
                ));
            }
            let mut out = Vec::with_capacity(count);
            let mut v = base;
            for _ in 0..count {
                out.push(v);
                v *= rate;
            }
            out
        }
        Trend::Logistic => {
            if count == 1 {
                vec![base]
            } else {
                let span = (count - 1) as f64;
                let k = 10.0 / span;
                let mid = span / 2.0;
                let f = |x: f64| 1.0 / (1.0 + (-k * (x - mid)).exp());
                let (f0, f1) = (f(0.0), f(span));
                let denom = f1 - f0;
                let rise = strength * span;
                (0..count)
                    .map(|i| {
                        let x = i as f64;
                        let norm = if denom.abs() < 1e-12 {
                            x / span
                        } else {
                            (f(x) - f0) / denom
                        };
                        base + rise * norm
                    })
                    .collect()
            }
        }
        Trend::RandomWalk => {
            // The walk is part of the SHARED signal, so every column drifts
            // together and only the noise differs.
            let mut rng = substream(seed, 1, 0);
            let mut acc = base;
            let mut out = Vec::with_capacity(count);
            for i in 0..count {
                if i > 0 {
                    acc += strength * rng.normal();
                }
                out.push(acc);
            }
            out
        }
    })
}

/// The seasonal term at each step — additive units, or a fraction of the level
/// when `combine = "multiplicative"`.
fn seasonal_terms(seasonality: Seasonality, spec: &Spec, ts: &[i64]) -> Result<Vec<f64>, String> {
    if seasonality == Seasonality::None {
        return Ok(vec![0.0; spec.count]);
    }
    let amps = parse_list(spec.amplitude, "amplitude")?;
    if amps.is_empty() {
        return Err(
            "amplitude is empty — give one number per period, e.g. \"8, 4\" for two cycles"
                .to_string(),
        );
    }

    if seasonality == Seasonality::Weekday {
        let pattern = parse_list(spec.weekday_pattern, "weekday_pattern")?;
        if pattern.len() != 7 {
            return Err(format!(
                "weekday_pattern needs exactly 7 numbers (Monday to Sunday), got {}",
                pattern.len()
            ));
        }
        // Mean-centre so the pattern only changes shape, never the average, then
        // rescale so the largest deviation is exactly `amplitude` — the 7 numbers
        // set the SHAPE and amplitude sets the size, whatever units they're in.
        let mean = pattern.iter().sum::<f64>() / 7.0;
        let centred: Vec<f64> = pattern.iter().map(|p| p - mean).collect();
        let peak = centred.iter().fold(0.0f64, |m, c| m.max(c.abs()));
        let scale = if peak < 1e-12 { 0.0 } else { amps[0] / peak };
        return Ok(ts
            .iter()
            .map(|ms| {
                let day = DateTime::from_timestamp_millis(*ms)
                    .map(|dt| dt.weekday().num_days_from_monday() as usize)
                    .unwrap_or(0);
                centred[day] * scale
            })
            .collect());
    }

    let periods = parse_list(spec.period, "period")?;
    if periods.is_empty() {
        return Err(
            "period is empty — give the cycle length in steps, e.g. \"24\" or \"24, 168\""
                .to_string(),
        );
    }
    if periods.len() > MAX_COMPONENTS {
        return Err(format!(
            "period lists {} cycles, over the {MAX_COMPONENTS} cap",
            periods.len()
        ));
    }
    if amps.len() != 1 && amps.len() != periods.len() {
        return Err(format!(
            "amplitude has {} value(s) but period has {} — give one amplitude per period, or a single amplitude for all of them",
            amps.len(),
            periods.len()
        ));
    }
    for p in &periods {
        if *p <= 0.0 {
            return Err(format!(
                "period must be a positive number of steps, got {p}"
            ));
        }
    }

    let mut out = vec![0.0; spec.count];
    for (c, p) in periods.iter().enumerate() {
        let a = if amps.len() == 1 { amps[0] } else { amps[c] };
        for (i, slot) in out.iter_mut().enumerate() {
            let frac = (i as f64 / p).rem_euclid(1.0);
            let phase = std::f64::consts::TAU * frac;
            *slot += a * match seasonality {
                Seasonality::Sine => phase.sin(),
                Seasonality::Cosine => phase.cos(),
                Seasonality::Square => {
                    if frac < 0.5 {
                        1.0
                    } else {
                        -1.0
                    }
                }
                Seasonality::Triangle => {
                    if frac < 0.25 {
                        4.0 * frac
                    } else if frac < 0.75 {
                        2.0 - 4.0 * frac
                    } else {
                        4.0 * frac - 4.0
                    }
                }
                Seasonality::Sawtooth => 2.0 * frac - 1.0,
                // Handled above; unreachable here.
                Seasonality::Weekday | Seasonality::None => 0.0,
            };
        }
    }
    Ok(out)
}

// --------------------------------------------------------------- renderers --

fn csv_escape(s: &str, delim: char) -> String {
    if s.contains(delim) || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn render_delimited(
    spec: &Spec,
    ts: &[i64],
    ts_col: &str,
    ts_format: TsFormat,
    columns: &[Column],
    delim: char,
) -> String {
    let mut out = String::new();
    if spec.header {
        out.push_str(&csv_escape(ts_col, delim));
        for c in columns {
            out.push(delim);
            out.push_str(&csv_escape(&c.label, delim));
        }
        out.push('\n');
    }
    for (i, ms) in ts.iter().enumerate() {
        out.push_str(&csv_escape(&fmt_ts(*ms, i, ts_format), delim));
        for c in columns {
            out.push(delim);
            if let Some(v) = c.values[i] {
                out.push_str(&fmt_val(v, spec.decimals));
            }
        }
        out.push('\n');
    }
    out.trim_end_matches('\n').to_string()
}

fn row_object(
    i: usize,
    ms: i64,
    ts_col: &str,
    ts_format: TsFormat,
    columns: &[Column],
) -> Map<String, Value> {
    let mut row = Map::new();
    row.insert(ts_col.to_string(), ts_json(ms, i, ts_format));
    for c in columns {
        row.insert(
            c.label.clone(),
            match c.values[i] {
                Some(v) => json!(v),
                None => Value::Null,
            },
        );
    }
    row
}

fn render_json(
    spec: &Spec,
    ts: &[i64],
    ts_col: &str,
    ts_format: TsFormat,
    columns: &[Column],
    step: Step,
) -> String {
    let rows: Vec<Value> = ts
        .iter()
        .enumerate()
        .map(|(i, ms)| Value::Object(row_object(i, *ms, ts_col, ts_format, columns)))
        .collect();
    let doc = json!({
        "count": spec.count,
        "series": spec.series,
        "labels": columns.iter().map(|c| c.label.clone()).collect::<Vec<_>>(),
        "start": fmt_ts(ts[0], 0, TsFormat::Iso),
        "end": fmt_ts(ts[ts.len() - 1], ts.len() - 1, TsFormat::Iso),
        "interval": describe_step(step),
        "seed": spec.seed,
        "missing": columns.iter().map(|c| c.missing).sum::<usize>(),
        "outliers": columns.iter().map(|c| c.outliers).sum::<usize>(),
        "rows": rows,
    });
    serde_json::to_string_pretty(&doc).unwrap_or_else(|_| doc.to_string())
}

fn render_ndjson(ts: &[i64], ts_col: &str, ts_format: TsFormat, columns: &[Column]) -> String {
    ts.iter()
        .enumerate()
        .map(|(i, ms)| Value::Object(row_object(i, *ms, ts_col, ts_format, columns)).to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

fn describe_step(step: Step) -> String {
    match step {
        Step::Fixed(w) => {
            for (unit, size) in [
                ("w", MS_PER_WEEK),
                ("d", MS_PER_DAY),
                ("h", MS_PER_HOUR),
                ("m", MS_PER_MIN),
                ("s", MS_PER_SEC),
            ] {
                if w % size == 0 {
                    return format!("{}{unit}", w / size);
                }
            }
            format!("{w}ms")
        }
        Step::Months(n) if n % 12 == 0 => format!("{}y", n / 12),
        Step::Months(n) if n % 3 == 0 => format!("{}q", n / 3),
        Step::Months(n) => format!("{n}mo"),
    }
}

#[allow(clippy::too_many_arguments)]
fn render_stats(
    spec: &Spec,
    ts: &[i64],
    ts_format: TsFormat,
    columns: &[Column],
    step: Step,
    trend: Trend,
    seasonality: Seasonality,
    noise: Noise,
    combine: Combine,
) -> String {
    let dec = spec.decimals.max(3);
    let mut out = String::new();
    out.push_str(&format!(
        "{} point(s) x {} series — {} to {}, every {}, seed {}\n",
        spec.count,
        spec.series,
        fmt_ts(ts[0], 0, ts_format),
        fmt_ts(ts[ts.len() - 1], ts.len() - 1, ts_format),
        describe_step(step),
        spec.seed
    ));
    out.push_str(&format!(
        "Signal: base {}, trend {:?} (strength {}), seasonality {:?}, combine {:?}\n",
        spec.base, trend, spec.trend_strength, seasonality, combine
    ));
    out.push_str(&format!(
        "Noise: {:?} (level {}, phi {}) — missing_rate {}, outlier_rate {}\n",
        noise, spec.noise_level, spec.noise_phi, spec.missing_rate, spec.outlier_rate
    ));
    for c in columns {
        let present: Vec<f64> = c.values.iter().flatten().copied().collect();
        out.push('\n');
        out.push_str(&format!("{}:\n", c.label));
        if present.is_empty() {
            out.push_str("  every value is missing\n");
            continue;
        }
        let n = present.len() as f64;
        let mean = present.iter().sum::<f64>() / n;
        let var = present.iter().map(|v| (v - mean).powi(2)).sum::<f64>()
            / if n > 1.0 { n - 1.0 } else { 1.0 };
        let min = present.iter().fold(f64::INFINITY, |a, b| a.min(*b));
        let max = present.iter().fold(f64::NEG_INFINITY, |a, b| a.max(*b));
        out.push_str(&format!("  count    {}\n", present.len()));
        out.push_str(&format!("  min      {}\n", fmt_val(min, dec)));
        out.push_str(&format!("  max      {}\n", fmt_val(max, dec)));
        out.push_str(&format!("  mean     {}\n", fmt_val(mean, dec)));
        out.push_str(&format!("  sd       {}\n", fmt_val(var.sqrt(), dec)));
        out.push_str(&format!(
            "  first    {}\n",
            fmt_val(*present.first().unwrap(), dec)
        ));
        out.push_str(&format!(
            "  last     {}\n",
            fmt_val(*present.last().unwrap(), dec)
        ));
        out.push_str(&format!("  missing  {}\n", c.missing));
        out.push_str(&format!("  outliers {}\n", c.outliers));
    }
    out.trim_end_matches('\n').to_string()
}

// ------------------------------------------------------------------ tests ---

#[cfg(test)]
mod tests {
    use super::*;

    fn quiet<'a>() -> Spec<'a> {
        // A noise-free spec: every assertion below is then exact arithmetic.
        Spec {
            count: 3,
            trend: "none",
            seasonality: "none",
            noise: "none",
            ..Spec::default()
        }
    }

    #[test]
    fn linear_trend_is_exact_and_dated() {
        let got = generate(&Spec {
            trend: "linear",
            trend_strength: 10.0,
            ..quiet()
        })
        .unwrap();
        assert_eq!(
            got,
            "timestamp,value\n2024-01-01,100.00\n2024-01-02,110.00\n2024-01-03,120.00"
        );
    }

    #[test]
    fn flat_series_with_no_header_and_tsv() {
        let got = generate(&Spec {
            header: false,
            output: "tsv",
            decimals: 0,
            ..quiet()
        })
        .unwrap();
        assert_eq!(got, "2024-01-01\t100\n2024-01-02\t100\n2024-01-03\t100");
    }

    #[test]
    fn exponential_trend_compounds_by_percent_per_step() {
        let got = generate(&Spec {
            trend: "exponential",
            trend_strength: 10.0,
            decimals: 1,
            ..quiet()
        })
        .unwrap();
        assert_eq!(
            got,
            "timestamp,value\n2024-01-01,100.0\n2024-01-02,110.0\n2024-01-03,121.0"
        );
    }

    #[test]
    fn logistic_trend_spans_the_same_total_rise_as_linear() {
        let got = generate(&Spec {
            count: 11,
            trend: "logistic",
            trend_strength: 10.0,
            output: "json",
            ..quiet()
        })
        .unwrap();
        let v: Value = serde_json::from_str(&got).unwrap();
        let rows = v["rows"].as_array().unwrap();
        // 10 steps x strength 10 = a 100-unit rise, S-shaped, midpoint halfway.
        assert_eq!(rows[0]["value"].as_f64().unwrap(), 100.0);
        assert_eq!(rows[10]["value"].as_f64().unwrap(), 200.0);
        assert_eq!(rows[5]["value"].as_f64().unwrap(), 150.0);
        assert!(rows[1]["value"].as_f64().unwrap() < 110.0, "slow start");
    }

    #[test]
    fn cosine_seasonality_peaks_at_the_cycle_start() {
        let got = generate(&Spec {
            count: 4,
            seasonality: "cosine",
            period: "4",
            amplitude: "10",
            decimals: 1,
            ..quiet()
        })
        .unwrap();
        assert_eq!(
            got,
            "timestamp,value\n2024-01-01,110.0\n2024-01-02,100.0\n2024-01-03,90.0\n2024-01-04,100.0"
        );
    }

    #[test]
    fn square_and_sawtooth_shapes_are_distinct() {
        let sq = generate(&Spec {
            count: 4,
            seasonality: "square",
            period: "4",
            amplitude: "10",
            decimals: 0,
            header: false,
            ..quiet()
        })
        .unwrap();
        assert_eq!(
            sq,
            "2024-01-01,110\n2024-01-02,110\n2024-01-03,90\n2024-01-04,90"
        );
        let saw = generate(&Spec {
            count: 4,
            seasonality: "sawtooth",
            period: "4",
            amplitude: "10",
            decimals: 0,
            header: false,
            ..quiet()
        })
        .unwrap();
        assert_eq!(
            saw,
            "2024-01-01,90\n2024-01-02,95\n2024-01-03,100\n2024-01-04,105"
        );
    }

    #[test]
    fn two_periods_superimpose() {
        let both = generate(&Spec {
            count: 2,
            seasonality: "cosine",
            period: "4, 2",
            amplitude: "10, 5",
            decimals: 1,
            header: false,
            ..quiet()
        })
        .unwrap();
        // i = 0: both cycles at their peak → 100 + 10 + 5.
        assert!(both.starts_with("2024-01-01,115.0"));
    }

    #[test]
    fn weekday_pattern_is_mean_centred_and_scaled_to_amplitude() {
        // 2024-01-01 is a Monday; the default pattern peaks on Friday.
        let got = generate(&Spec {
            count: 7,
            seasonality: "weekday",
            amplitude: "20",
            weekday_pattern: "1, 1, 1, 1, 2, 1, 1",
            decimals: 2,
            header: false,
            ..quiet()
        })
        .unwrap();
        let lines: Vec<&str> = got.lines().collect();
        // Six days at -mean, Friday at the +20 peak.
        assert_eq!(lines[4], "2024-01-05,120.00");
        assert_eq!(lines[0], "2024-01-01,96.67");
    }

    #[test]
    fn multiplicative_combine_reads_amplitude_as_a_fraction() {
        let got = generate(&Spec {
            count: 4,
            seasonality: "cosine",
            period: "4",
            amplitude: "0.2",
            combine: "multiplicative",
            decimals: 1,
            header: false,
            ..quiet()
        })
        .unwrap();
        assert!(got.starts_with("2024-01-01,120.0"), "got {got}");
    }

    #[test]
    fn calendar_months_keep_month_ends() {
        let got = generate(&Spec {
            start: "2024-01-31",
            interval: "1mo",
            count: 4,
            header: false,
            decimals: 0,
            ..quiet()
        })
        .unwrap();
        assert_eq!(
            got,
            "2024-01-31,100\n2024-02-29,100\n2024-03-31,100\n2024-04-30,100"
        );
    }

    #[test]
    fn sub_day_interval_switches_auto_timestamps_to_iso() {
        let got = generate(&Spec {
            start: "2024-03-01T06:00:00Z",
            interval: "90m",
            count: 2,
            header: false,
            decimals: 0,
            ..quiet()
        })
        .unwrap();
        assert_eq!(got, "2024-03-01T06:00:00Z,100\n2024-03-01T07:30:00Z,100");
    }

    #[test]
    fn epoch_and_index_timestamp_formats() {
        let epoch = generate(&Spec {
            count: 2,
            timestamp_format: "epoch",
            header: false,
            decimals: 0,
            ..quiet()
        })
        .unwrap();
        assert_eq!(epoch, "1704067200,100\n1704153600,100");
        let idx = generate(&Spec {
            count: 2,
            timestamp_format: "index",
            decimals: 0,
            ..quiet()
        })
        .unwrap();
        assert_eq!(idx, "index,value\n0,100\n1,100");
    }

    #[test]
    fn seed_is_deterministic_and_changing_it_changes_the_data() {
        let a = generate(&Spec {
            noise: "gaussian",
            ..quiet()
        })
        .unwrap();
        let b = generate(&Spec {
            noise: "gaussian",
            ..quiet()
        })
        .unwrap();
        assert_eq!(a, b, "same seed must reproduce the series exactly");
        let c = generate(&Spec {
            noise: "gaussian",
            seed: 43,
            ..quiet()
        })
        .unwrap();
        assert_ne!(a, c);
    }

    #[test]
    fn parallel_series_share_the_signal_but_not_the_noise() {
        let got = generate(&Spec {
            count: 5,
            series: 3,
            noise: "gaussian",
            noise_level: 5.0,
            output: "json",
            ..quiet()
        })
        .unwrap();
        let v: Value = serde_json::from_str(&got).unwrap();
        assert_eq!(v["labels"], json!(["value_1", "value_2", "value_3"]));
        let r0 = &v["rows"][0];
        assert_ne!(r0["value_1"], r0["value_2"]);
        assert_ne!(r0["value_2"], r0["value_3"]);
    }

    #[test]
    fn labels_name_the_columns() {
        let got = generate(&Spec {
            count: 1,
            series: 2,
            labels: "north, south",
            ..quiet()
        })
        .unwrap();
        assert_eq!(got, "timestamp,north,south\n2024-01-01,100.00,100.00");
    }

    #[test]
    fn ar1_noise_is_autocorrelated() {
        let spec = Spec {
            count: 400,
            noise: "ar1",
            noise_level: 5.0,
            noise_phi: 0.9,
            output: "json",
            decimals: 6,
            ..quiet()
        };
        let v: Value = serde_json::from_str(&generate(&spec).unwrap()).unwrap();
        let xs: Vec<f64> = v["rows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["value"].as_f64().unwrap())
            .collect();
        let mean = xs.iter().sum::<f64>() / xs.len() as f64;
        let num: f64 = xs.windows(2).map(|w| (w[0] - mean) * (w[1] - mean)).sum();
        let den: f64 = xs.iter().map(|x| (x - mean).powi(2)).sum();
        assert!(num / den > 0.6, "lag-1 correlation was {}", num / den);
    }

    #[test]
    fn missing_rate_blanks_cells_in_csv_and_nulls_them_in_json() {
        let csv = generate(&Spec {
            count: 200,
            missing_rate: 0.5,
            header: false,
            ..quiet()
        })
        .unwrap();
        let blanks = csv.lines().filter(|l| l.ends_with(',')).count();
        assert!((60..=140).contains(&blanks), "{blanks} blank cells");
        let js: Value = serde_json::from_str(
            &generate(&Spec {
                count: 200,
                missing_rate: 0.5,
                output: "json",
                ..quiet()
            })
            .unwrap(),
        )
        .unwrap();
        assert_eq!(js["missing"].as_u64().unwrap(), blanks as u64);
        assert!(js["rows"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["value"].is_null()));
    }

    #[test]
    fn outliers_respect_direction_and_bounds_clamp_them() {
        let up = generate(&Spec {
            count: 50,
            outlier_rate: 0.5,
            outlier_magnitude: 4.0,
            outlier_direction: "up",
            header: false,
            decimals: 0,
            ..quiet()
        })
        .unwrap();
        assert!(up.contains(",500"), "an up outlier is 5x the level");
        assert!(!up.contains(",20\n"), "no down outliers in up mode");

        let clamped = generate(&Spec {
            count: 50,
            outlier_rate: 1.0,
            outlier_magnitude: 4.0,
            outlier_direction: "up",
            max_value: "150",
            header: false,
            decimals: 0,
            ..quiet()
        })
        .unwrap();
        assert!(clamped.lines().all(|l| l.ends_with(",150")));
    }

    #[test]
    fn min_value_floors_a_noisy_series() {
        let got = generate(&Spec {
            count: 300,
            base: 5.0,
            noise: "gaussian",
            noise_level: 20.0,
            min_value: "0",
            output: "json",
            ..quiet()
        })
        .unwrap();
        let v: Value = serde_json::from_str(&got).unwrap();
        assert!(v["rows"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["value"].as_f64().unwrap() >= 0.0));
    }

    #[test]
    fn ndjson_emits_one_object_per_line() {
        let got = generate(&Spec {
            count: 2,
            output: "ndjson",
            ..quiet()
        })
        .unwrap();
        let lines: Vec<&str> = got.lines().collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], r#"{"timestamp":"2024-01-01","value":100.0}"#);
    }

    #[test]
    fn stats_reports_the_achieved_shape() {
        let got = generate(&Spec {
            count: 10,
            trend: "linear",
            trend_strength: 1.0,
            output: "stats",
            ..quiet()
        })
        .unwrap();
        assert!(
            got.contains("10 point(s) x 1 series — 2024-01-01 to 2024-01-10, every 1d, seed 42")
        );
        assert!(got.contains("  min      100.000"));
        assert!(got.contains("  max      109.000"));
        assert!(got.contains("  missing  0"));
    }

    #[test]
    fn rejects_bad_enums_and_out_of_range_numbers() {
        for (spec, needle) in [
            (
                Spec {
                    trend: "quadratic",
                    ..quiet()
                },
                "trend must be one of",
            ),
            (
                Spec {
                    seasonality: "wiggle",
                    ..quiet()
                },
                "seasonality must be one of",
            ),
            (
                Spec {
                    noise: "pink",
                    ..quiet()
                },
                "noise must be one of",
            ),
            (
                Spec {
                    outlier_direction: "sideways",
                    ..quiet()
                },
                "outlier_direction must be one of",
            ),
            (
                Spec {
                    output: "parquet",
                    ..quiet()
                },
                "output must be one of",
            ),
            (
                Spec {
                    timestamp_format: "rfc822",
                    ..quiet()
                },
                "timestamp_format must be one of",
            ),
            (
                Spec {
                    combine: "average",
                    ..quiet()
                },
                "combine must be one of",
            ),
            (
                Spec {
                    count: 0,
                    ..quiet()
                },
                "count must be between 1 and 100000",
            ),
            (
                Spec {
                    count: MAX_COUNT + 1,
                    ..quiet()
                },
                "count must be between 1 and 100000",
            ),
            (
                Spec {
                    series: 21,
                    ..quiet()
                },
                "series must be between 1 and 20",
            ),
            (
                Spec {
                    count: 50_000,
                    series: 5,
                    ..quiet()
                },
                "over the 200000 cap",
            ),
            (
                Spec {
                    decimals: 11,
                    ..quiet()
                },
                "decimals must be between 0 and 10",
            ),
            (
                Spec {
                    noise_level: -1.0,
                    ..quiet()
                },
                "noise_level must be 0 or more",
            ),
            (
                Spec {
                    noise_phi: 1.5,
                    ..quiet()
                },
                "noise_phi must be between",
            ),
            (
                Spec {
                    missing_rate: 2.0,
                    ..quiet()
                },
                "missing_rate must be between 0 and 1",
            ),
            (
                Spec {
                    outlier_rate: -0.1,
                    ..quiet()
                },
                "outlier_rate must be between 0 and 1",
            ),
            (
                Spec {
                    interval: "1fortnight",
                    ..quiet()
                },
                "unknown interval unit",
            ),
            (
                Spec {
                    interval: "5",
                    ..quiet()
                },
                "is missing a unit",
            ),
            (
                Spec {
                    start: "yesterday",
                    ..quiet()
                },
                "is not a timestamp",
            ),
            (
                Spec {
                    seasonality: "sine",
                    period: "0",
                    ..quiet()
                },
                "period must be a positive number of steps",
            ),
            (
                Spec {
                    seasonality: "sine",
                    period: "7, 30",
                    amplitude: "1, 2, 3",
                    ..quiet()
                },
                "give one amplitude per period",
            ),
            (
                Spec {
                    seasonality: "weekday",
                    weekday_pattern: "1, 2, 3",
                    ..quiet()
                },
                "weekday_pattern needs exactly 7 numbers",
            ),
            (
                Spec {
                    series: 2,
                    labels: "only-one",
                    ..quiet()
                },
                "give one name per column",
            ),
            (
                Spec {
                    min_value: "10",
                    max_value: "5",
                    ..quiet()
                },
                "is above max_value",
            ),
            (
                Spec {
                    min_value: "low",
                    ..quiet()
                },
                "is not a number",
            ),
            (
                Spec {
                    trend: "exponential",
                    trend_strength: -150.0,
                    ..quiet()
                },
                "it must be above -100",
            ),
        ] {
            let err = generate(&spec).expect_err(&format!("expected an error mentioning {needle}"));
            assert!(
                err.contains(needle),
                "error '{err}' did not mention '{needle}'"
            );
        }
    }

    #[test]
    fn cap_boundaries_are_inclusive() {
        assert!(generate(&Spec {
            count: MAX_COUNT,
            series: 1,
            output: "stats",
            ..quiet()
        })
        .is_ok());
        assert!(generate(&Spec {
            count: 10_000,
            series: MAX_SERIES,
            output: "stats",
            ..quiet()
        })
        .is_ok());
    }
}
