//! gizza-ai/time-series-generator — deterministic synthetic time-series data.
//! Chat schema is single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to block_utils::run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_time_series_generator_core::{generate, Spec};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    #[serde(default = "default_start")]
    start: String,
    #[serde(default = "default_interval")]
    interval: String,
    #[serde(default = "default_count")]
    count: usize,
    #[serde(default = "default_base")]
    base: f64,
    #[serde(default = "default_trend")]
    trend: String,
    #[serde(default = "default_trend_strength")]
    trend_strength: f64,
    #[serde(default = "default_seasonality")]
    seasonality: String,
    #[serde(default = "default_period")]
    period: String,
    #[serde(default = "default_amplitude")]
    amplitude: String,
    #[serde(default = "default_weekday_pattern")]
    weekday_pattern: String,
    #[serde(default = "default_combine")]
    combine: String,
    #[serde(default = "default_noise")]
    noise: String,
    #[serde(default = "default_noise_level")]
    noise_level: f64,
    #[serde(default = "default_noise_phi")]
    noise_phi: f64,
    #[serde(default)]
    missing_rate: f64,
    #[serde(default)]
    outlier_rate: f64,
    #[serde(default = "default_outlier_magnitude")]
    outlier_magnitude: f64,
    #[serde(default = "default_outlier_direction")]
    outlier_direction: String,
    #[serde(default)]
    min_value: String,
    #[serde(default)]
    max_value: String,
    #[serde(default = "default_series")]
    series: usize,
    #[serde(default = "default_seed")]
    seed: u64,
    #[serde(default = "default_decimals")]
    decimals: usize,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default = "default_timestamp_format")]
    timestamp_format: String,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default)]
    labels: String,
}

fn default_start() -> String {
    "2024-01-01".into()
}
fn default_interval() -> String {
    "1d".into()
}
fn default_count() -> usize {
    100
}
fn default_base() -> f64 {
    100.0
}
fn default_trend() -> String {
    "linear".into()
}
fn default_trend_strength() -> f64 {
    0.5
}
fn default_seasonality() -> String {
    "sine".into()
}
fn default_period() -> String {
    "7".into()
}
fn default_amplitude() -> String {
    "10".into()
}
fn default_weekday_pattern() -> String {
    "1.1, 1.05, 1, 1.05, 1.25, 0.8, 0.7".into()
}
fn default_combine() -> String {
    "additive".into()
}
fn default_noise() -> String {
    "gaussian".into()
}
fn default_noise_level() -> f64 {
    5.0
}
fn default_noise_phi() -> f64 {
    0.7
}
fn default_outlier_magnitude() -> f64 {
    3.0
}
fn default_outlier_direction() -> String {
    "both".into()
}
fn default_series() -> usize {
    1
}
fn default_seed() -> u64 {
    42
}
fn default_decimals() -> usize {
    2
}
fn default_output() -> String {
    "csv".into()
}
fn default_timestamp_format() -> String {
    "auto".into()
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("start").default("2024-01-01").describe("First timestamp. Accepts YYYY-MM-DD, RFC3339/ISO datetimes, or a Unix epoch seconds value. Default 2024-01-01."))
        .param(Param::string("interval").default("1d").describe("Step between rows. Use a positive number plus unit: ms, s, m, h, d, w, mo, q or y, for example 15m, 1h, 1d or 1mo. Calendar months/quarters/years keep month ends aligned. Default 1d."))
        .param(Param::integer("count").default(100).min(1.0).max(100000.0).describe("Rows to generate, 1-100000. The total count x series is capped at 200000 emitted values. Default 100."))
        .param(Param::number("base").default(100.0).describe("Starting level of the series before trend, seasonality and noise are applied. Default 100."))
        .param(Param::enumv("trend", ["none", "linear", "exponential", "logistic", "random-walk"]).default("linear").describe("Trend layer. none keeps the base flat; linear adds trend_strength per step; exponential compounds by trend_strength percent per step; logistic makes an S-curve across the requested count; random-walk adds seeded increments. Default linear."))
        .param(Param::number("trend_strength").default(0.5).describe("Trend amount. For linear/random-walk it is units per step, for exponential it is percent per step, and for logistic it is the total rise from first to last row. Default 0.5."))
        .param(Param::enumv("seasonality", ["none", "sine", "cosine", "square", "triangle", "sawtooth", "weekday"]).default("sine").describe("Seasonality shape. Cyclic shapes use period and amplitude; weekday uses weekday_pattern as Monday through Sunday multipliers. Default sine."))
        .param(Param::string("period").default("7").describe("Cycle length in rows. Comma-separated lists superimpose multiple cycles, such as period='24,168' for daily and weekly hourly data. Default 7."))
        .param(Param::string("amplitude").default("10").describe("Seasonal amplitude. May be a comma-separated list matching period; in multiplicative mode values are fractions of the level, otherwise they are additive units. Default 10."))
        .param(Param::string("weekday_pattern").default("1.1, 1.05, 1, 1.05, 1.25, 0.8, 0.7").describe("Seven Monday-through-Sunday values used when seasonality=weekday. The pattern is mean-centred and scaled by amplitude so it works with additive or multiplicative combine modes."))
        .param(Param::enumv("combine", ["additive", "multiplicative"]).default("additive").describe("How trend and seasonal/noise layers combine. additive sums units; multiplicative treats amplitude and noise_level as fractions of the current level. Default additive."))
        .param(Param::enumv("noise", ["none", "gaussian", "uniform", "ar1"]).default("gaussian").describe("Noise process. gaussian draws normal noise, uniform draws in +/- noise_level, and ar1 creates autocorrelated noise controlled by noise_phi. Default gaussian."))
        .param(Param::number("noise_level").default(5.0).min(0.0).describe("Noise scale. For gaussian it is the standard deviation, for uniform it is the half-width, and for multiplicative mode it is a fraction of the level. Default 5."))
        .param(Param::number("noise_phi").default(0.7).min(-0.99).max(0.99).describe("AR(1) correlation coefficient used only when noise=ar1. Must be between -0.99 and 0.99. Default 0.7."))
        .param(Param::number("missing_rate").default(0.0).min(0.0).max(1.0).describe("Probability that each generated value is blank/null, from 0 to 1. CSV/TSV emit an empty cell; JSON/NDJSON emit null. Default 0."))
        .param(Param::number("outlier_rate").default(0.0).min(0.0).max(1.0).describe("Probability that each generated value receives an outlier multiplier, from 0 to 1. Default 0."))
        .param(Param::number("outlier_magnitude").default(3.0).min(0.0).describe("Outlier size as a multiple of the value: 3 means upward outliers become value*(1+3), downward outliers become value/(1+3). Default 3."))
        .param(Param::enumv("outlier_direction", ["both", "up", "down"]).default("both").describe("Allowed outlier direction: both, up only, or down only. Default both."))
        .param(Param::string("min_value").default("").describe("Optional numeric lower clamp applied after outliers. Leave blank for no lower bound."))
        .param(Param::string("max_value").default("").describe("Optional numeric upper clamp applied after outliers. Leave blank for no upper bound."))
        .param(Param::integer("series").default(1).min(1.0).max(20.0).describe("Number of parallel value columns, 1-20. Columns share the trend/seasonal signal but have independent seeded noise, missingness and outliers. Default 1."))
        .param(Param::integer("seed").default(42).min(0.0).describe("Seed for the deterministic SplitMix64 random streams. The same settings and seed return identical output in chat, CLI and page. Default 42."))
        .param(Param::integer("decimals").default(2).min(0.0).max(10.0).describe("Digits after the decimal point, 0-10. Use 0 for integer-like counts. Default 2."))
        .param(Param::enumv("output", ["csv", "tsv", "json", "ndjson", "stats"]).default("csv").describe("Result format. csv/tsv emit rows, json emits one object with rows, ndjson emits one object per row, and stats reports achieved min/max/mean/sd plus missing/outlier counts. Default csv."))
        .param(Param::enumv("timestamp_format", ["auto", "iso", "date", "epoch", "index"]).default("auto").describe("Timestamp output format. auto uses dates for day-or-larger intervals and ISO datetimes for sub-day intervals; epoch emits seconds; index emits row numbers. Default auto."))
        .param(Param::boolean("header").default(true).describe("Include the timestamp/value header row in csv and tsv output. Ignored by json, ndjson and stats. Default true."))
        .param(Param::string("labels").default("").describe("Optional comma-separated labels for value columns. Supply one label per series; leave blank to use value, value_2 and so on."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/time-series-generator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate deterministic synthetic time-series data with trend, seasonality and noise",
    skill(
        description = "Generate synthetic time-series data for tests, demos and examples. The tool builds a timestamp index from start, interval and count; layers configurable trend, seasonality, seeded noise, missing values and outliers; supports one or more parallel series; clamps and rounds values; and returns CSV, TSV, JSON, NDJSON or a stats summary. Randomness is deterministic from seed so the same settings reproduce exactly across chat, CLI and browser. Runs locally and does not upload data.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "time-series-generator", |a: Args| {
            let spec = Spec {
                start: &a.start,
                interval: &a.interval,
                count: a.count,
                base: a.base,
                trend: &a.trend,
                trend_strength: a.trend_strength,
                seasonality: &a.seasonality,
                period: &a.period,
                amplitude: &a.amplitude,
                weekday_pattern: &a.weekday_pattern,
                combine: &a.combine,
                noise: &a.noise,
                noise_level: a.noise_level,
                noise_phi: a.noise_phi,
                missing_rate: a.missing_rate,
                outlier_rate: a.outlier_rate,
                outlier_magnitude: a.outlier_magnitude,
                outlier_direction: &a.outlier_direction,
                min_value: &a.min_value,
                max_value: &a.max_value,
                series: a.series,
                seed: a.seed,
                decimals: a.decimals,
                output: &a.output,
                timestamp_format: &a.timestamp_format,
                header: a.header,
                labels: &a.labels,
            };
            generate(&spec).map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_json_has_no_todos_and_required_defaults() {
        let v: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = v.get("properties").unwrap();
        assert!(props
            .get("trend")
            .unwrap()
            .get("enum")
            .unwrap()
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("random-walk")));
        assert_eq!(
            props.get("count").unwrap().get("default"),
            Some(&serde_json::json!(100))
        );
        assert_eq!(
            props.get("header").unwrap().get("default"),
            Some(&serde_json::json!(true))
        );
        let s = serde_json::to_string(&v).unwrap();
        assert!(!s.contains("TODO"));
    }
}
