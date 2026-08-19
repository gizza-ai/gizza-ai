//! gizza-ai/exponential-smoother — chat skill block on the shared tool
//! abstraction. The chat schema is single-sourced from descriptor() (which also
//! drives the CLI + page query-params); handle() delegates to
//! block_utils::run_skill. No host calls — runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_exponential_smoother_core::{smooth, Options, MAX_FORECAST, MAX_POINTS};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    /// The numeric series to smooth.
    series: String,
    /// Which decay parameterisation to use: alpha, span, halflife, com, or auto.
    #[serde(default = "default_mode")]
    mode: String,
    /// Smoothing factor in (0, 1] for mode=alpha.
    #[serde(default = "default_alpha")]
    alpha: f64,
    /// N-period span for mode=span; alpha = 2/(span+1).
    #[serde(default = "default_span")]
    span: f64,
    /// Half-life in periods for mode=halflife; alpha = 1 - exp(-ln2/halflife).
    #[serde(default = "default_halflife")]
    halflife: f64,
    /// Center of mass for mode=com; alpha = 1/(1+com).
    #[serde(default = "default_com")]
    com: f64,
    /// Bias-corrected weighting (divide by the decaying weight sum).
    #[serde(default = "default_adjust")]
    adjust: bool,
    /// Weight relative to the last observation instead of the last position.
    #[serde(default)]
    ignore_na: bool,
    /// Observations required before a smoothed value is emitted.
    #[serde(default)]
    min_periods: usize,
    /// Flat future periods to project at the final smoothed level.
    #[serde(default)]
    forecast: usize,
    /// Output format: json, csv, or svg.
    #[serde(default = "default_output")]
    output: String,
}

fn default_mode() -> String {
    "alpha".into()
}
fn default_alpha() -> f64 {
    0.3
}
fn default_span() -> f64 {
    5.0
}
fn default_halflife() -> f64 {
    3.0
}
fn default_com() -> f64 {
    2.0
}
fn default_adjust() -> bool {
    true
}
fn default_output() -> String {
    "json".into()
}

impl From<Args> for Options {
    fn from(a: Args) -> Self {
        Options {
            mode: a.mode,
            alpha: a.alpha,
            span: a.span,
            halflife: a.halflife,
            com: a.com,
            adjust: a.adjust,
            ignore_na: a.ignore_na,
            min_periods: a.min_periods,
            forecast: a.forecast,
            output: a.output,
        }
    }
}

/// Single-source param descriptor → chat schema (and CLI + page query-params).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("series")
                .required()
                .describe("The numeric series to smooth, separated by commas, spaces, tabs, semicolons, or newlines. A JSON array of numbers works too, a leading text header is skipped, and na/nan/null/none/- mark a missing observation."),
        )
        .param(
            Param::enumv("mode", ["alpha", "span", "halflife", "com", "auto"])
                .default("alpha")
                .describe("Which decay setting to read: alpha uses the smoothing factor directly, span uses alpha=2/(span+1), halflife uses alpha=1-exp(-ln2/halflife), com uses alpha=1/(1+com), and auto fits alpha by minimising the one-step-ahead squared forecast error. Default alpha."),
        )
        .param(
            Param::number("alpha")
                .default(0.3)
                .min(0.0)
                .max(1.0)
                .describe("Smoothing factor for mode=alpha. Must be greater than 0 and at most 1: higher reacts faster to new points, lower smooths harder. Default 0.3."),
        )
        .param(
            Param::number("span")
                .default(5.0)
                .min(1.0)
                .describe("N-period span for mode=span, the familiar 'N-day EMA' setting. Must be at least 1 and maps to alpha = 2/(span+1). Default 5."),
        )
        .param(
            Param::number("halflife")
                .default(3.0)
                .min(0.0)
                .describe("Half-life in periods for mode=halflife — how many periods until an observation's weight halves. Must be greater than 0 and maps to alpha = 1 - exp(-ln2/halflife). Default 3."),
        )
        .param(
            Param::number("com")
                .default(2.0)
                .min(0.0)
                .describe("Center of mass for mode=com. Must be 0 or greater and maps to alpha = 1/(1+com). Default 2."),
        )
        .param(
            Param::boolean("adjust")
                .default(true)
                .describe("Divide by the decaying weight sum so early points are not biased toward the first observation. Turn off for the plain recursion y_t = (1-alpha)*y_(t-1) + alpha*x_t used by simple exponential smoothing and finance EMAs. Default true."),
        )
        .param(
            Param::boolean("ignore_na")
                .default(false)
                .describe("Weight points by their position among the observations instead of their position in the series, so a gap does not consume any decay. Default false."),
        )
        .param(
            Param::integer("min_periods")
                .default(0)
                .min(0.0)
                .max(MAX_POINTS as f64)
                .describe("Observations required before a smoothed value is reported; earlier positions come back as null. 0 and 1 both emit from the first observation. Default 0."),
        )
        .param(
            Param::integer("forecast")
                .default(0)
                .min(0.0)
                .max(MAX_FORECAST as f64)
                .describe("Future periods to project. Exponential smoothing has a flat forecast, so every projected period sits at the final smoothed level. Default 0."),
        )
        .param(
            Param::enumv("output", ["json", "csv", "svg"])
                .default("json")
                .describe("Output format: a JSON report, a CSV table of index/value/smoothed/error, or a self-contained SVG chart. Default json."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/exponential-smoother",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Smooth a numeric series with an exponentially-weighted moving average.",
    skill(
        description = "Apply exponentially-weighted moving average (EWMA) smoothing to a numeric series. Set the decay as a smoothing factor (alpha), an N-period span, a half-life, or a center of mass, or let mode=auto fit alpha by minimising the one-step-ahead squared forecast error. 'adjust' picks bias-corrected weighting or the plain simple-exponential-smoothing recursion, gaps (na/null/-) are supported with an 'ignore_na' weighting switch, 'min_periods' blanks the warm-up, and 'forecast' projects flat future periods at the final level. Returns the smoothed series, the equivalent alpha/span/halflife/com, one-step-ahead SSE/MSE/RMSE/MAE/MAPE, and JSON, CSV, or an SVG chart. Runs locally — the data never leaves the device.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "exponential-smoother", |a: Args| {
            let series = a.series.clone();
            let opts: Options = a.into();
            smooth(&series, &opts).map_err(SkillError::InvalidArgs)
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
    /// (Number defaults serialize as floats — write `5.0`, not `5`.)
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "additionalProperties": false,
                "required": ["series"],
                "properties": {
                    "series": {
                        "type": "string",
                        "description": "The numeric series to smooth, separated by commas, spaces, tabs, semicolons, or newlines. A JSON array of numbers works too, a leading text header is skipped, and na/nan/null/none/- mark a missing observation."
                    },
                    "mode": {
                        "type": "string",
                        "enum": ["alpha", "span", "halflife", "com", "auto"],
                        "default": "alpha",
                        "description": "Which decay setting to read: alpha uses the smoothing factor directly, span uses alpha=2/(span+1), halflife uses alpha=1-exp(-ln2/halflife), com uses alpha=1/(1+com), and auto fits alpha by minimising the one-step-ahead squared forecast error. Default alpha."
                    },
                    "alpha": {
                        "type": "number",
                        "minimum": 0,
                        "maximum": 1,
                        "default": 0.3,
                        "description": "Smoothing factor for mode=alpha. Must be greater than 0 and at most 1: higher reacts faster to new points, lower smooths harder. Default 0.3."
                    },
                    "span": {
                        "type": "number",
                        "minimum": 1,
                        "default": 5.0,
                        "description": "N-period span for mode=span, the familiar 'N-day EMA' setting. Must be at least 1 and maps to alpha = 2/(span+1). Default 5."
                    },
                    "halflife": {
                        "type": "number",
                        "minimum": 0,
                        "default": 3.0,
                        "description": "Half-life in periods for mode=halflife — how many periods until an observation's weight halves. Must be greater than 0 and maps to alpha = 1 - exp(-ln2/halflife). Default 3."
                    },
                    "com": {
                        "type": "number",
                        "minimum": 0,
                        "default": 2.0,
                        "description": "Center of mass for mode=com. Must be 0 or greater and maps to alpha = 1/(1+com). Default 2."
                    },
                    "adjust": {
                        "type": "boolean",
                        "default": true,
                        "description": "Divide by the decaying weight sum so early points are not biased toward the first observation. Turn off for the plain recursion y_t = (1-alpha)*y_(t-1) + alpha*x_t used by simple exponential smoothing and finance EMAs. Default true."
                    },
                    "ignore_na": {
                        "type": "boolean",
                        "default": false,
                        "description": "Weight points by their position among the observations instead of their position in the series, so a gap does not consume any decay. Default false."
                    },
                    "min_periods": {
                        "type": "integer",
                        "minimum": 0,
                        "maximum": 20000,
                        "default": 0,
                        "description": "Observations required before a smoothed value is reported; earlier positions come back as null. 0 and 1 both emit from the first observation. Default 0."
                    },
                    "forecast": {
                        "type": "integer",
                        "minimum": 0,
                        "maximum": 1000,
                        "default": 0,
                        "description": "Future periods to project. Exponential smoothing has a flat forecast, so every projected period sits at the final smoothed level. Default 0."
                    },
                    "output": {
                        "type": "string",
                        "enum": ["json", "csv", "svg"],
                        "default": "json",
                        "description": "Output format: a JSON report, a CSV table of index/value/smoothed/error, or a self-contained SVG chart. Default json."
                    }
                }
            }"#,
        )
        .unwrap();
        let actual: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(actual, authored);
    }

    /// The Serde defaults must match the descriptor's declared defaults, or a
    /// chat call that omits a param would behave differently from the CLI.
    #[test]
    fn serde_defaults_match_the_descriptor_defaults() {
        let a: Args = serde_json::from_str(r#"{"series":"1 2 3"}"#).unwrap();
        let o: Options = a.into();
        let d = Options::default();
        assert_eq!(o.mode, d.mode);
        assert_eq!(o.alpha, d.alpha);
        assert_eq!(o.span, d.span);
        assert_eq!(o.halflife, d.halflife);
        assert_eq!(o.com, d.com);
        assert_eq!(o.adjust, d.adjust);
        assert_eq!(o.ignore_na, d.ignore_na);
        assert_eq!(o.min_periods, d.min_periods);
        assert_eq!(o.forecast, d.forecast);
        assert_eq!(o.output, d.output);
    }
}
