//! gizza-ai/peak-detector — chat skill block on the shared tool abstraction.
//! Finds local maxima and minima in a pasted 1-D signal and reports each one's
//! index, value, prominence and width. The chat schema is single-sourced from
//! descriptor() (which also drives the CLI); handle() delegates to run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn default_rel_height() -> f64 {
    0.5
}

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default)]
    mode: String,
    #[serde(default)]
    separator: String,
    #[serde(default)]
    smooth: u64,
    #[serde(default)]
    min_value: String,
    #[serde(default)]
    max_value: String,
    #[serde(default)]
    threshold: f64,
    #[serde(default)]
    min_distance: u64,
    #[serde(default)]
    min_prominence: f64,
    #[serde(default)]
    min_width: f64,
    #[serde(default = "default_rel_height")]
    rel_height: f64,
    #[serde(default)]
    max_peaks: u64,
    #[serde(default)]
    sort_by: String,
    #[serde(default)]
    format: String,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe(
                    "The 1-D signal to analyse, as pasted text: one number per sample, in order. \
                     Values may be separated by commas, spaces, tabs, newlines, semicolons or pipes \
                     (see separator) — for example `0, 1, 0, 3, 0, 2, 0`. Integers, decimals, \
                     negatives and scientific notation (`1e6`) are all accepted; non-numeric tokens \
                     such as a pasted column header are skipped and counted. Up to 20000 samples \
                     per run. Indices in the report are 0-based, so the first sample is index 0.",
                ),
        )
        .param(
            Param::enumv("mode", ["maxima", "minima", "both"])
                .default("maxima")
                .describe(
                    "Which extrema to find. `maxima` (default) reports peaks — samples higher than \
                     both neighbours. `minima` reports valleys — samples lower than both neighbours; \
                     every measurement (prominence, width) is then the mirror image, measured on the \
                     way down. `both` reports peaks and valleys in one table with a `kind` column.",
                ),
        )
        .param(
            Param::enumv(
                "separator",
                ["auto", "comma", "newline", "space", "semicolon", "tab", "pipe"],
            )
            .default("auto")
            .describe(
                "How the samples are separated. `auto` (default) splits on any whitespace, comma, \
                 semicolon or pipe, which handles most pasted columns and CSV rows. Pick an explicit \
                 separator for ambiguous data — for example `newline` for one value per line, or \
                 `tab` for a column copied out of a spreadsheet. Blank entries are skipped.",
            ),
        )
        .param(
            Param::integer("smooth")
                .default(0)
                .min(0.0)
                .max(501.0)
                .describe(
                    "Moving-average window, in samples, applied BEFORE detection to stop noise \
                     registering as peaks. 0 or 1 (default 0) means no smoothing. Must be an odd \
                     number so the window stays centred on its sample — try 3, 5 or 7 for lightly \
                     noisy data, larger for very noisy data. Reported values are the smoothed ones; \
                     the window shrinks at the two ends so no artificial ramp is introduced.",
                ),
        )
        .param(
            Param::string("min_value")
                .default("")
                .describe(
                    "Amplitude floor: ignore any extremum whose value is below this number. Blank \
                     (default) means no floor. Applied to the value as you see it, so in `minima` \
                     mode it still means \"no lower than this\". Use it to skip the baseline wiggle \
                     of a signal whose real events all sit above a known level — for example \
                     `min_value=5` on a 0-10 signal.",
                ),
        )
        .param(
            Param::string("max_value")
                .default("")
                .describe(
                    "Amplitude ceiling: ignore any extremum whose value is above this number. Blank \
                     (default) means no ceiling. Combined with min_value it forms a band, so you can \
                     isolate mid-height features and drop a saturating spike — for example \
                     `min_value=5` with `max_value=7`. min_value above max_value is rejected, since \
                     no value could pass.",
                ),
        )
        .param(
            Param::number("threshold")
                .default(0.0)
                .min(0.0)
                .describe(
                    "Minimum vertical drop from the peak to BOTH of its immediate neighbours, in the \
                     signal's own units. 0 (default) turns the filter off. This is a purely local \
                     test — it rejects a peak that barely clears the samples touching it, without \
                     looking any further along the signal. Use min_prominence instead when you care \
                     how the peak compares to the wider curve.",
                ),
        )
        .param(
            Param::integer("min_distance")
                .default(0)
                .min(0.0)
                .max(20000.0)
                .describe(
                    "Minimum spacing, in samples, between reported peaks of the same kind. 0 \
                     (default) turns it off. Peaks are kept tallest-first: the most extreme peak \
                     survives and every peak closer than this many samples to an already-kept one is \
                     dropped. Use it to report one peak per event when a single event produces a \
                     cluster of samples above the surroundings.",
                ),
        )
        .param(
            Param::number("min_prominence")
                .default(0.0)
                .min(0.0)
                .describe(
                    "Minimum prominence: how far the signal must fall on BOTH sides of the peak \
                     before it rises back above that peak (or reaches an end of the data). 0 \
                     (default) turns it off. This is the standard \"is this a real peak or just a \
                     bump on the side of a bigger one\" test, and is usually the most effective \
                     single filter on noisy data — try a few percent of the signal's range.",
                ),
        )
        .param(
            Param::number("min_width")
                .default(0.0)
                .min(0.0)
                .describe(
                    "Minimum peak width in samples, measured across the peak at rel_height of its \
                     prominence with linear interpolation between samples. 0 (default) turns it off. \
                     Use it to reject a one-sample spike (width near 1) while keeping broad, \
                     genuine features — the complement of min_prominence, which judges height \
                     rather than breadth.",
                ),
        )
        .param(
            Param::number("rel_height")
                .default(0.5)
                .min(0.0)
                .max(1.0)
                .describe(
                    "Where the width is measured, as a fraction of the peak's prominence, 0-1 \
                     (default 0.5 = half prominence, the usual convention and the same reference \
                     point as a FWHM measurement). 1 measures right down at the peak's base, giving \
                     the widest number; a small value measures just below the tip. It changes the \
                     reported `width` and therefore what min_width rejects.",
                ),
        )
        .param(
            Param::integer("max_peaks")
                .default(0)
                .min(0.0)
                .max(10000.0)
                .describe(
                    "Report at most this many peaks, applied AFTER sorting, so with \
                     `sort_by=prominence` it gives you the N most significant peaks. 0 (default) \
                     reports every peak that passed the filters. The report always states how many \
                     peaks matched in total, so you can see what was withheld.",
                ),
        )
        .param(
            Param::enumv("sort_by", ["position", "prominence", "value"])
                .default("position")
                .describe(
                    "Row order in the report. `position` (default) keeps the peaks in the order they \
                     occur in the signal — best for reading a trace end to end. `prominence` puts the \
                     most significant peaks first, and `value` the highest-valued ones; pair either \
                     with max_peaks to get a top-N list.",
                ),
        )
        .param(
            Param::enumv("format", ["text", "json", "csv"])
                .default("text")
                .describe(
                    "Output format. `text` (default) is the readable report: a peak table plus the \
                     highest peak, deepest valley, mean spacing, signal statistics and the filters in \
                     force. `json` returns the full report object, including each peak's bases and \
                     interpolated width endpoints. `csv` returns just the peak table with a header \
                     row, ready to paste into a spreadsheet.",
                ),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/peak-detector",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Find local maxima and minima in a pasted 1-D signal, with prominence, height, distance and width filters.",
    skill(
        description = "Find the local maxima (peaks) and local minima (valleys) in a pasted 1-D signal and report each one's 0-based index, value, prominence and width. Candidates can be filtered by amplitude band (min_value/max_value), vertical drop to both immediate neighbours (threshold), spacing between peaks (min_distance), prominence (min_prominence) and width (min_width), evaluated in that documented order; an optional moving-average window (smooth) suppresses noise before detection. Flat-topped peaks collapse to the middle sample and the first and last sample are never peaks. Results can be sorted by position, prominence or value and capped with max_peaks. Accepts comma, space, tab, newline, semicolon or pipe separated values, decimals, negatives and scientific notation, up to 20000 samples. Text, JSON or CSV output. Report-only — it never changes the data.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "peak-detector", |a: Args| {
            gizza_ai_peak_detector_core::run_with_options(
                &a.data,
                &a.mode,
                &a.separator,
                a.smooth,
                &a.min_value,
                &a.max_value,
                a.threshold,
                a.min_distance,
                a.min_prominence,
                a.min_width,
                a.rel_height,
                a.max_peaks,
                &a.sort_by,
                &a.format,
            )
            .map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the chat/CLI schema is the contract other surfaces consume.
    #[test]
    fn schema_matches_expected() {
        let actual: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let expected: serde_json::Value = serde_json::from_str(EXPECTED_SCHEMA).unwrap();
        assert_eq!(
            actual["required"], expected["required"],
            "required params drifted"
        );
        let props = actual["properties"].as_object().unwrap();
        let exp_props = expected["properties"].as_object().unwrap();
        assert_eq!(
            props.keys().collect::<Vec<_>>(),
            exp_props.keys().collect::<Vec<_>>(),
            "param set drifted"
        );
        for (name, exp) in exp_props {
            let got = &props[name];
            assert_eq!(got["type"], exp["type"], "{name}: type drifted");
            assert_eq!(got["enum"], exp["enum"], "{name}: enum drifted");
            assert_eq!(got["default"], exp["default"], "{name}: default drifted");
            assert_eq!(got["minimum"], exp["minimum"], "{name}: minimum drifted");
            assert_eq!(got["maximum"], exp["maximum"], "{name}: maximum drifted");
            assert!(
                got["description"].as_str().is_some_and(|d| d.len() > 40),
                "{name}: needs a real .describe()"
            );
        }
    }

    /// Every core option is reachable from the schema — a param dropped from the
    /// descriptor would silently make that filter unusable from chat and CLI.
    #[test]
    fn schema_exposes_every_core_option() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = schema["properties"].as_object().unwrap();
        for name in [
            "data",
            "mode",
            "separator",
            "smooth",
            "min_value",
            "max_value",
            "threshold",
            "min_distance",
            "min_prominence",
            "min_width",
            "rel_height",
            "max_peaks",
            "sort_by",
            "format",
        ] {
            assert!(props.contains_key(name), "{name} missing from the schema");
        }
    }

    const EXPECTED_SCHEMA: &str = r#"{
      "required": ["data"],
      "properties": {
        "data":           { "type": "string" },
        "mode":           { "type": "string", "enum": ["maxima", "minima", "both"], "default": "maxima" },
        "separator":      { "type": "string", "enum": ["auto", "comma", "newline", "space", "semicolon", "tab", "pipe"], "default": "auto" },
        "smooth":         { "type": "integer", "default": 0, "minimum": 0, "maximum": 501 },
        "min_value":      { "type": "string", "default": "" },
        "max_value":      { "type": "string", "default": "" },
        "threshold":      { "type": "number", "default": 0.0, "minimum": 0 },
        "min_distance":   { "type": "integer", "default": 0, "minimum": 0, "maximum": 20000 },
        "min_prominence": { "type": "number", "default": 0.0, "minimum": 0 },
        "min_width":      { "type": "number", "default": 0.0, "minimum": 0 },
        "rel_height":     { "type": "number", "default": 0.5, "minimum": 0, "maximum": 1 },
        "max_peaks":      { "type": "integer", "default": 0, "minimum": 0, "maximum": 10000 },
        "sort_by":        { "type": "string", "enum": ["position", "prominence", "value"], "default": "position" },
        "format":         { "type": "string", "enum": ["text", "json", "csv"], "default": "text" }
      }
    }"#;
}
