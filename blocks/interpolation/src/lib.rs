//! gizza-ai/interpolation — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI + page query-params); handle() delegates to block_utils::run_skill. No
//! host calls — runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_interpolation_core::{interpolate, Options};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    /// The known points: x,y rows, a y-only column, a one-line y list, or JSON.
    data: String,
    /// Interpolation method: linear, cubic, monotone, polynomial, or nearest.
    #[serde(default = "default_method")]
    method: String,
    /// x values to evaluate the interpolant at.
    #[serde(default)]
    at: String,
    /// Cubic-spline end conditions: natural, not-a-knot, or clamped.
    #[serde(default = "default_boundary")]
    boundary: String,
    /// Slope at the first point for boundary = clamped.
    #[serde(default)]
    start_slope: f64,
    /// Slope at the last point for boundary = clamped.
    #[serde(default)]
    end_slope: f64,
    /// Behaviour outside the data range: error, clamp, or extend.
    #[serde(default = "default_extrapolate")]
    extrapolate: String,
    /// Evenly spaced samples across the data range; 0 disables.
    #[serde(default)]
    resample: usize,
    /// 0 = value, 1 = first derivative, 2 = second derivative.
    #[serde(default)]
    derivative: usize,
    /// Decimal places for printed numbers.
    #[serde(default = "default_decimals")]
    decimals: usize,
    /// Include the segment equations / coefficients in the output.
    #[serde(default)]
    coefficients: bool,
    /// Output format: values, csv, json, or svg.
    #[serde(default = "default_output")]
    output: String,
}

fn default_method() -> String {
    "linear".into()
}
fn default_boundary() -> String {
    "natural".into()
}
fn default_extrapolate() -> String {
    "error".into()
}
fn default_decimals() -> usize {
    6
}
fn default_output() -> String {
    "values".into()
}

impl From<Args> for Options {
    fn from(a: Args) -> Self {
        Options {
            method: a.method,
            at: a.at,
            boundary: a.boundary,
            start_slope: a.start_slope,
            end_slope: a.end_slope,
            extrapolate: a.extrapolate,
            resample: a.resample,
            derivative: a.derivative,
            decimals: a.decimals,
            coefficients: a.coefficients,
            output: a.output,
        }
    }
}

/// Single-source param descriptor → chat schema (and CLI + page query-params).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("The known data points. One 'x,y' pair per line is the usual form (comma, semicolon, tab or space separated) — e.g. '0,0\\n1,1\\n2,8\\n3,27'. A single-column list is read as y values with x = 1, 2, 3 …, a one-line list does the same, and JSON is accepted as [1,2,3], [[x,y],…] or [{\"x\":…,\"y\":…},…]. A non-numeric first row is treated as a header and skipped. Rows may be in any order (they are sorted by x), but every x must be unique. Max 10,000 points / 2,000,000 bytes."),
        )
        .param(
            Param::enumv("method", ["linear", "cubic", "monotone", "polynomial", "nearest"])
                .default("linear")
                .describe("Which interpolant to build through the points: 'linear' (default) joins neighbours with straight lines and needs 2+ points; 'cubic' fits a classical cubic spline with continuous first and second derivatives and needs 3+ points; 'monotone' fits a shape-preserving PCHIP cubic that never overshoots between points, which is what you want for cumulative or physically bounded data; 'polynomial' fits the single degree n-1 curve through every point (2-30 points, and it oscillates badly past about 10 — see Runge's phenomenon); 'nearest' is a step lookup that returns the y of the closest x, rounding a tie up to the point on the right."),
        )
        .param(
            Param::string("at")
                .describe("The x values to evaluate at, separated by commas, spaces, tabs, semicolons or newlines — e.g. '1.5, 2.5, 3.75'. Values are reported in the order you give them. Leave it blank and set `resample` for an evenly spaced sweep instead; leave BOTH blank and the tool evaluates the midpoint of every interval, which is the plain 'what sits between my points' answer. Max 5,000 values."),
        )
        .param(
            Param::enumv("boundary", ["natural", "not-a-knot", "clamped"])
                .default("natural")
                .describe("End conditions for method = cubic (ignored by the other methods). 'natural' (default) sets the curvature to zero at both ends, which is the textbook spline and keeps the ends straight. 'not-a-knot' instead forces the third derivative to stay continuous across the second and second-to-last points, so a cubic spline through samples of a real cubic reproduces it exactly — this is what MATLAB's spline() does. 'clamped' pins the slope at each end to `start_slope` and `end_slope`."),
        )
        .param(
            Param::number("start_slope")
                .default(0.0)
                .describe("The first derivative dy/dx to impose at the FIRST data point when boundary = clamped. Default 0, which starts the curve flat. Ignored for every other boundary and method."),
        )
        .param(
            Param::number("end_slope")
                .default(0.0)
                .describe("The first derivative dy/dx to impose at the LAST data point when boundary = clamped. Default 0, which ends the curve flat. Ignored for every other boundary and method."),
        )
        .param(
            Param::enumv("extrapolate", ["error", "clamp", "extend"])
                .default("error")
                .describe("What to do with an x outside the range covered by your points: 'error' (default) refuses and names the range, because interpolation is only defined between the anchors; 'clamp' returns the nearest endpoint's y (and 0 for a derivative), giving a flat continuation; 'extend' continues the end piece's polynomial, which follows the trend but can run away fast — a polynomial fit especially so."),
        )
        .param(
            Param::integer("resample")
                .default(0)
                .min(0.0)
                .max(5000.0)
                .describe("Sample the interpolant at this many evenly spaced x values from the first point to the last, inclusive — 11 gives you deciles of the range. 0 (default) turns it off; 1 is rejected because an evenly spaced sweep needs at least two ends. Combine with `at` to get both your own x values and a sweep in one run."),
        )
        .param(
            Param::integer("derivative")
                .default(0)
                .min(0.0)
                .max(2.0)
                .describe("Report the value (0, the default), the first derivative dy/dx (1) or the second derivative d²y/dx² (2) of the interpolant at each x. Useful for reading a rate of change or curvature straight off the fitted curve. With method = linear the first derivative is the segment slope and the second is 0; with method = nearest both are 0."),
        )
        .param(
            Param::integer("decimals")
                .default(6)
                .min(0.0)
                .max(12.0)
                .describe("Decimal places for every printed number, 0-12 (default 6). Trailing zeros are trimmed, so a computed 2.500000 prints as 2.5. This affects DISPLAY only — the interpolation itself always runs at full 64-bit floating-point precision."),
        )
        .param(
            Param::boolean("coefficients")
                .default(false)
                .describe("Also return the fitted curve itself, not just the evaluated values: the per-interval cubic coefficients a, b, c, d with their readable equation 'y = a + b(x - x_start) + …' for linear/cubic/monotone/nearest, or the expanded monomial coefficients and 'y = c0 + c1x + …' for polynomial. Default false."),
        )
        .param(
            Param::enumv("output", ["values", "csv", "json", "svg"])
                .default("values")
                .describe("Result format: 'values' (default) prints one 'x,value' per line, ready to paste into a spreadsheet column; 'csv' adds a header plus per-row source (at/resample/midpoint) and extrapolated columns, and appends the coefficient table when `coefficients` is on; 'json' returns the full report — method, range, evaluations, segments and warnings — for scripting; 'svg' draws a self-contained chart of the data points, the fitted curve and the evaluated x values."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/interpolation",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Interpolate between data points with linear, cubic-spline, monotone, polynomial or nearest methods.",
    skill(
        description = "Build an interpolant that passes exactly through a set of (x, y) points and evaluate it at new x values. Choose linear, cubic spline (natural, not-a-knot or clamped), shape-preserving monotone PCHIP, a single polynomial through every point, or nearest-neighbour. Evaluate at your own x list, at an evenly spaced resampling grid, or at every interval midpoint; optionally report the first or second derivative, control extrapolation outside the data range, and return the segment equations. Output as plain values, CSV, a JSON report or an SVG chart.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "interpolation", |a: Args| {
            let data = a.data.clone();
            let opts: Options = a.into();
            interpolate(&data, &opts).map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the authored schema the chat surface, the CLI and the page
    /// query-params all consume. Regenerate this literal whenever descriptor()
    /// changes — never loosen it.
    #[test]
    fn schema_json_matches_the_authored_descriptor() {
        let v: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(v["type"], "object");
        assert_eq!(v["required"], serde_json::json!(["data"]));
        assert_eq!(v["additionalProperties"], serde_json::json!(false));

        let p = &v["properties"];
        let names: Vec<&str> = p.as_object().unwrap().keys().map(|s| s.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "at",
                "boundary",
                "coefficients",
                "data",
                "decimals",
                "derivative",
                "end_slope",
                "extrapolate",
                "method",
                "output",
                "resample",
                "start_slope",
            ]
        );

        assert_eq!(
            p["method"]["enum"],
            serde_json::json!(["linear", "cubic", "monotone", "polynomial", "nearest"])
        );
        assert_eq!(p["method"]["default"], "linear");
        assert_eq!(
            p["boundary"]["enum"],
            serde_json::json!(["natural", "not-a-knot", "clamped"])
        );
        assert_eq!(p["boundary"]["default"], "natural");
        assert_eq!(
            p["extrapolate"]["enum"],
            serde_json::json!(["error", "clamp", "extend"])
        );
        assert_eq!(p["extrapolate"]["default"], "error");
        assert_eq!(
            p["output"]["enum"],
            serde_json::json!(["values", "csv", "json", "svg"])
        );
        assert_eq!(p["output"]["default"], "values");

        assert_eq!(p["start_slope"]["type"], "number");
        assert_eq!(p["start_slope"]["default"], 0.0);
        assert_eq!(p["end_slope"]["default"], 0.0);
        assert_eq!(p["resample"]["type"], "integer");
        assert_eq!(p["resample"]["default"], 0);
        assert_eq!(p["resample"]["minimum"], 0.0);
        assert_eq!(p["resample"]["maximum"], 5000.0);
        assert_eq!(p["derivative"]["default"], 0);
        assert_eq!(p["derivative"]["maximum"], 2.0);
        assert_eq!(p["decimals"]["default"], 6);
        assert_eq!(p["decimals"]["maximum"], 12.0);
        assert_eq!(p["coefficients"]["type"], "boolean");
        assert_eq!(p["coefficients"]["default"], false);
        assert_eq!(p["at"]["type"], "string");
        assert_eq!(p["data"]["type"], "string");
    }

    #[test]
    fn every_param_has_an_actionable_description() {
        let v: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        for (name, prop) in v["properties"].as_object().unwrap() {
            let d = prop["description"].as_str().unwrap_or("");
            assert!(d.len() > 40, "param '{name}' needs a real .describe()");
        }
    }

    #[test]
    fn args_defaults_match_the_schema_defaults() {
        let a: Args = serde_json::from_str(r#"{"data":"1,1\n2,2"}"#).unwrap();
        let o: Options = a.into();
        let d = Options::default();
        assert_eq!(o.method, d.method);
        assert_eq!(o.boundary, d.boundary);
        assert_eq!(o.extrapolate, d.extrapolate);
        assert_eq!(o.output, d.output);
        assert_eq!(o.decimals, d.decimals);
        assert_eq!(o.resample, d.resample);
        assert_eq!(o.derivative, d.derivative);
        assert_eq!(o.coefficients, d.coefficients);
        assert_eq!(o.start_slope, d.start_slope);
        assert_eq!(o.end_slope, d.end_slope);
    }
}
