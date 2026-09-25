//! gizza-ai/anomaly-timeseries — flag anomalous points in an ordered numeric
//! series with rolling-baseline and seasonal-deviation rules. Thin chat-skill
//! wrapper; the chat schema is single-sourced from descriptor() (which also drives
//! the CLI); handle() delegates to block_utils::run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_anomaly_timeseries_core::{
    render, Options, MAX_DECIMALS, MAX_PERIOD, MAX_POINTS, MAX_THRESHOLD, MAX_WINDOW,
};
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
#[serde(default)]
struct Args {
    series: String,
    method: String,
    window: u32,
    min_periods: u32,
    threshold: f64,
    warn_threshold: f64,
    period: u32,
    tolerance: f64,
    direction: String,
    center: bool,
    only_anomalies: bool,
    decimals: u32,
    output: String,
}

impl Default for Args {
    fn default() -> Self {
        let o = Options::default();
        Args {
            series: String::new(),
            method: o.method,
            window: o.window,
            min_periods: o.min_periods,
            threshold: o.threshold,
            warn_threshold: o.warn_threshold,
            period: o.period,
            tolerance: o.tolerance,
            direction: o.direction,
            center: o.center,
            only_anomalies: o.only_anomalies,
            decimals: o.decimals,
            output: o.output,
        }
    }
}

impl From<&Args> for Options {
    fn from(a: &Args) -> Self {
        Options {
            method: a.method.clone(),
            window: a.window,
            min_periods: a.min_periods,
            period: a.period,
            threshold: a.threshold,
            warn_threshold: a.warn_threshold,
            tolerance: a.tolerance,
            direction: a.direction.clone(),
            center: a.center,
            only_anomalies: a.only_anomalies,
            decimals: a.decimals,
            output: a.output.clone(),
        }
    }
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("series")
                .required()
                .describe("The metric history in time order: one number per line, `label,value` rows so dates keep their names (e.g. '2026-01-03, 118'), or a single line separated by commas, spaces, semicolons or tabs. A leading header row is skipped. Needs at least 3 values; max 20,000."),
        )
        .param(
            Param::enumv(
                "method",
                ["rolling_z", "rolling_mad", "seasonal_z", "combined"],
            )
            .default("rolling_z")
            .describe("Which rule decides. rolling_z (default) compares each point with the mean and standard deviation of its window. rolling_mad swaps in the median and scaled MAD, so spikes already inside the window cannot inflate the spread and mask the next one. seasonal_z compares each point with the points at the SAME position in the other cycles (same weekday, same month), which stops a routinely quiet Sunday being called an anomaly. combined takes the worse verdict of the rolling z-score and seasonal rules and reports which one fired."),
        )
        .param(
            Param::integer("window")
                .default(12)
                .min(2.0)
                .max(MAX_WINDOW as f64)
                .describe("How many neighbouring points form the rolling baseline, 2-1000 (default 12, e.g. a year of monthly data or a quarter of weekly). The point under test is always excluded from its own baseline. Ignored by method=seasonal_z."),
        )
        .param(
            Param::integer("min_periods")
                .default(0)
                .min(0.0)
                .max(MAX_WINDOW as f64)
                .describe("Fewest baseline points needed before a rolling verdict is given, 0-1000. 0 (default) requires the full window, so early rows come back as 'unscored' instead of judged on thin history; a smaller number starts scoring sooner. Never below 2, since a spread needs two points."),
        )
        .param(
            Param::number("threshold")
                .default(3.0)
                .min(0.0)
                .max(MAX_THRESHOLD)
                .describe("How many baseline spreads from the expected value counts as an anomaly (default 3, the usual production cutoff; 2 is roughly the top 5% of normally-distributed points). Points at or beyond it are marked 'critical' and listed in anomaly_indices."),
        )
        .param(
            Param::number("warn_threshold")
                .default(2.0)
                .min(0.0)
                .max(MAX_THRESHOLD)
                .describe("Watch-band cutoff (default 2). Points that reach it but stay under 'threshold' are marked 'warning' and counted separately — they never inflate the anomaly count. Set 0 to switch the watch band off. Must not exceed threshold."),
        )
        .param(
            Param::integer("period")
                .default(7)
                .min(2.0)
                .max(MAX_PERIOD as f64)
                .describe("Length of one seasonal cycle in data points, 2-1000 (default 7 for daily data with a weekly pattern; 12 for monthly, 24 for hourly). Used only by method=seasonal_z and combined, which need at least 3 points at each position in the cycle (2 x period + 1 values)."),
        )
        .param(
            Param::number("tolerance")
                .default(0.0)
                .min(0.0)
                .describe("Deadband in the series' OWN units (default 0 = off). A point whose gap from its expected value is this small or smaller is never flagged, however large its score — the fix for a near-flat metric where a rounding-level wobble scores like a crisis."),
        )
        .param(
            Param::enumv("direction", ["both", "above", "below"])
                .default("both")
                .describe("Which side to alert on: 'both' (default), 'above' for spikes only (traffic surges, error bursts), 'below' for drops only (a feed that stopped reporting)."),
        )
        .param(
            Param::boolean("center")
                .default(false)
                .describe("How the baseline is gathered. False (default) is the live-monitoring reading: only EARLIER points count, so a verdict never depends on the future and the first rows are unscored. True is the retrospective reading: a symmetric window around each point (and every other cycle for the seasonal rule), which scores the whole series including its start."),
        )
        .param(
            Param::boolean("only_anomalies")
                .default(false)
                .describe("Return only the rows marked 'warning' or 'critical' instead of every row (default false). The summary still counts the whole series."),
        )
        .param(
            Param::integer("decimals")
                .default(6)
                .min(0.0)
                .max(MAX_DECIMALS as f64)
                .describe("Decimal places to round every returned number to, 0-10 (default 6)."),
        )
        .param(
            Param::enumv("output", ["json", "table", "csv"])
                .default("json")
                .describe("Result shape: 'json' (default) gives the full report — per-point expected value, deviation, score, normal band, severity plus anomaly_indices/anomaly_values/anomaly_scores and a summary; 'table' is an aligned text table with a header and a plain-language reading; 'csv' is index,label,value,expected,deviation,score,lower,upper,severity,anomaly,rule for a spreadsheet."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct AnomalyTimeseries;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/anomaly-timeseries",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Flag anomalies in a time series with rolling z-score and seasonal rules",
    skill(
        description = "Find the anomalous points in a metric history using rolling-baseline and seasonal-deviation rules. Paste the series in time order — one number per line, `label,value` rows so dates keep their names, or one separated line (a header row is skipped). 'method' picks the rule: rolling_z (default) uses the rolling mean and standard deviation of the surrounding 'window' (default 12); rolling_mad uses the median and scaled MAD so spikes inside the window cannot mask the next one; seasonal_z compares each point with the same position in other cycles of length 'period' (default 7), so a routinely quiet weekend is not called an anomaly; combined takes the worse of the rolling and seasonal verdicts and reports which fired. Every baseline excludes the point under test, so a spike cannot hide inside its own mean. 'threshold' (default 3) flags anomalies, 'warn_threshold' (default 2) only labels near-misses, 'tolerance' ignores gaps too small to care about, 'direction' limits alerts to spikes or drops, 'min_periods' controls how much history is required before scoring, and 'center' switches between causal (past-only) and retrospective (symmetric) baselines. Returns each row's expected value, deviation, score, normal band and severity (unscored/normal/warning/critical), the anomaly indices, values and scores, and a summary with counts, anomaly rate and the worst point — as JSON, an aligned table, or CSV. Runs locally, so the metric history never leaves the device.",
        parameters = schema_json()
    ),
)]
impl AnomalyTimeseries {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "anomaly-timeseries", |a: Args| {
            render(&a.series, &Options::from(&a)).map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "series": {
                        "type": "string",
                        "description": "The metric history in time order: one number per line, `label,value` rows so dates keep their names (e.g. '2026-01-03, 118'), or a single line separated by commas, spaces, semicolons or tabs. A leading header row is skipped. Needs at least 3 values; max 20,000."
                    },
                    "method": {
                        "type": "string",
                        "enum": ["rolling_z", "rolling_mad", "seasonal_z", "combined"],
                        "default": "rolling_z",
                        "description": "Which rule decides. rolling_z (default) compares each point with the mean and standard deviation of its window. rolling_mad swaps in the median and scaled MAD, so spikes already inside the window cannot inflate the spread and mask the next one. seasonal_z compares each point with the points at the SAME position in the other cycles (same weekday, same month), which stops a routinely quiet Sunday being called an anomaly. combined takes the worse verdict of the rolling z-score and seasonal rules and reports which one fired."
                    },
                    "window": {
                        "type": "integer",
                        "minimum": 2,
                        "maximum": 1000,
                        "default": 12,
                        "description": "How many neighbouring points form the rolling baseline, 2-1000 (default 12, e.g. a year of monthly data or a quarter of weekly). The point under test is always excluded from its own baseline. Ignored by method=seasonal_z."
                    },
                    "min_periods": {
                        "type": "integer",
                        "minimum": 0,
                        "maximum": 1000,
                        "default": 0,
                        "description": "Fewest baseline points needed before a rolling verdict is given, 0-1000. 0 (default) requires the full window, so early rows come back as 'unscored' instead of judged on thin history; a smaller number starts scoring sooner. Never below 2, since a spread needs two points."
                    },
                    "threshold": {
                        "type": "number",
                        "minimum": 0,
                        "maximum": 1000,
                        "default": 3.0,
                        "description": "How many baseline spreads from the expected value counts as an anomaly (default 3, the usual production cutoff; 2 is roughly the top 5% of normally-distributed points). Points at or beyond it are marked 'critical' and listed in anomaly_indices."
                    },
                    "warn_threshold": {
                        "type": "number",
                        "minimum": 0,
                        "maximum": 1000,
                        "default": 2.0,
                        "description": "Watch-band cutoff (default 2). Points that reach it but stay under 'threshold' are marked 'warning' and counted separately — they never inflate the anomaly count. Set 0 to switch the watch band off. Must not exceed threshold."
                    },
                    "period": {
                        "type": "integer",
                        "minimum": 2,
                        "maximum": 1000,
                        "default": 7,
                        "description": "Length of one seasonal cycle in data points, 2-1000 (default 7 for daily data with a weekly pattern; 12 for monthly, 24 for hourly). Used only by method=seasonal_z and combined, which need at least 3 points at each position in the cycle (2 x period + 1 values)."
                    },
                    "tolerance": {
                        "type": "number",
                        "minimum": 0,
                        "default": 0.0,
                        "description": "Deadband in the series' OWN units (default 0 = off). A point whose gap from its expected value is this small or smaller is never flagged, however large its score — the fix for a near-flat metric where a rounding-level wobble scores like a crisis."
                    },
                    "direction": {
                        "type": "string",
                        "enum": ["both", "above", "below"],
                        "default": "both",
                        "description": "Which side to alert on: 'both' (default), 'above' for spikes only (traffic surges, error bursts), 'below' for drops only (a feed that stopped reporting)."
                    },
                    "center": {
                        "type": "boolean",
                        "default": false,
                        "description": "How the baseline is gathered. False (default) is the live-monitoring reading: only EARLIER points count, so a verdict never depends on the future and the first rows are unscored. True is the retrospective reading: a symmetric window around each point (and every other cycle for the seasonal rule), which scores the whole series including its start."
                    },
                    "only_anomalies": {
                        "type": "boolean",
                        "default": false,
                        "description": "Return only the rows marked 'warning' or 'critical' instead of every row (default false). The summary still counts the whole series."
                    },
                    "decimals": {
                        "type": "integer",
                        "minimum": 0,
                        "maximum": 10,
                        "default": 6,
                        "description": "Decimal places to round every returned number to, 0-10 (default 6)."
                    },
                    "output": {
                        "type": "string",
                        "enum": ["json", "table", "csv"],
                        "default": "json",
                        "description": "Result shape: 'json' (default) gives the full report — per-point expected value, deviation, score, normal band, severity plus anomaly_indices/anomaly_values/anomaly_scores and a summary; 'table' is an aligned text table with a header and a plain-language reading; 'csv' is index,label,value,expected,deviation,score,lower,upper,severity,anomaly,rule for a spreadsheet."
                    }
                },
                "required": ["series"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    /// The chat surface must work from `series` alone, on the documented defaults.
    #[test]
    fn args_default_to_a_trailing_rolling_z_run() {
        let a: Args = serde_json::from_str(r#"{"series":"10 11 9 10 12 10 11 40 10 11"}"#).unwrap();
        assert_eq!(a.method, "rolling_z");
        assert_eq!(a.window, 12);
        assert_eq!(a.min_periods, 0);
        assert_eq!(a.threshold, 3.0);
        assert_eq!(a.warn_threshold, 2.0);
        assert_eq!(a.period, 7);
        assert_eq!(a.tolerance, 0.0);
        assert_eq!(a.direction, "both");
        assert!(!a.center);
        assert!(!a.only_anomalies);
        assert_eq!(a.decimals, 6);
        assert_eq!(a.output, "json");
        // 10 points with a 12-point window: nothing has enough history yet, which
        // is the honest answer rather than a verdict on 2 neighbours.
        let out = render(&a.series, &Options::from(&a)).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["summary"]["scored"], 0);
        assert_eq!(v["summary"]["unscored"], 10);
    }

    #[test]
    fn a_shorter_window_finds_the_spike_through_the_chat_args() {
        let a: Args = serde_json::from_str(
            r#"{"series":"10 11 9 10 12 10 11 40 10 11","window":5,"min_periods":3,"output":"csv"}"#,
        )
        .unwrap();
        let out = render(&a.series, &Options::from(&a)).unwrap();
        assert!(out
            .lines()
            .nth(8)
            .unwrap()
            .contains(",critical,true,rolling"));
    }

    #[test]
    fn an_invalid_arg_is_reported_not_silently_defaulted() {
        let a: Args = serde_json::from_str(r#"{"series":"1 2 3","method":"lstm"}"#).unwrap();
        let e = render(&a.series, &Options::from(&a)).unwrap_err();
        assert!(
            e.contains("rolling_z, rolling_mad, seasonal_z, combined"),
            "{e}"
        );
    }

    #[test]
    fn max_points_is_advertised_in_the_series_description() {
        let schema = schema_json();
        assert!(schema.contains("max 20,000"));
        assert_eq!(MAX_POINTS, 20_000);
    }
}
