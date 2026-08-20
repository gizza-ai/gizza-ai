//! gizza-ai/linear-interpolate-gaps — chat skill block on the shared tool abstraction.
//! Fills the blanks in an ordered numeric series by drawing a straight line between the
//! two nearest known values, with the controls the reference implementations already
//! taught people to expect: a max-gap limit, a fill direction, and an explicit policy
//! for the gaps that sit outside the known range. The chat schema is single-sourced from
//! `descriptor()` (which also drives the CLI); `handle()` delegates to
//! `block_utils::run_skill`. Pure compute — nothing is uploaded.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    layout: String,
    #[serde(default)]
    max_gap: i64,
    #[serde(default)]
    direction: String,
    #[serde(default)]
    edges: String,
    #[serde(default)]
    na_tokens: String,
    #[serde(default = "default_decimals")]
    decimals: i64,
    #[serde(default)]
    output: String,
}

fn default_decimals() -> i64 {
    6
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The series to fill, as text. One value per line, or separated by commas, semicolons, tabs or spaces. A blank field or an NA marker (empty, na, n/a, nan, null, none, nil, -, --, ?) is a gap; everything else must be a number. Rows of exactly two fields are read as 'x,y' pairs so uneven spacing is respected. A leading header row or label is skipped automatically. Max 100,000 values."),
        )
        .param(
            Param::enumv("layout", ["auto", "values", "xy"])
                .default("auto")
                .describe("How the input is read: 'auto' (default) treats the data as x,y pairs when there are 2+ rows and EVERY row has exactly two fields, otherwise as a plain list of y values; 'values' forces the plain list, where the positions 1,2,3… act as the x axis and every step is equal; 'xy' forces x,y pairs and errors on any row that has not got exactly two fields. In x,y layout the x column must be numeric and strictly increasing — only y may be missing."),
        )
        .param(
            Param::integer("max_gap")
                .default(0)
                .min(0.0)
                .max(100000.0)
                .describe("Longest run of consecutive missing values that may be filled: 3 fills runs of 1-3 blanks and leaves a run of 4 alone, which is how you avoid inventing a week of data from two months apart. 0 (default) means no limit. A run longer than the limit is not skipped entirely under direction 'both' — it is filled max_gap values in from EACH side and left blank in the middle; use 'forward' or 'backward' to fill from one side only."),
        )
        .param(
            Param::enumv("direction", ["both", "forward", "backward"])
                .default("both")
                .describe("Which side(s) a gap may be filled from: 'both' (default), 'forward' only after a known value (the ffill direction), 'backward' only before one. On interior gaps with no max_gap all three give the same straight line — direction starts to matter once max_gap clips a run, and it also decides whether the leading gap (needs backward) and the trailing gap (needs forward) are eligible at all."),
        )
        .param(
            Param::enumv("edges", ["leave", "hold", "extrapolate"])
                .default("leave")
                .describe("What to do with the gaps OUTSIDE the known range — the blanks before the first known value and after the last, where there is no second anchor to draw a line to: 'leave' (default) keeps them empty, because strict interpolation never invents data past the anchors; 'hold' repeats the nearest known value (the classic ffill/bfill); 'extrapolate' extends the straight line through the two nearest known points, which can go negative. With a single known value 'extrapolate' degrades to 'hold' — one point has no slope."),
        )
        .param(
            Param::string("na_tokens")
                .default("")
                .describe("Extra tokens that also count as missing, comma-separated and case-insensitive, e.g. 'missing,unknown,-999,#N/A'. The built-in markers (empty, na, n/a, nan, null, none, nil, -, --, ?) are always recognised; this list adds to them rather than replacing them. Use it for sentinel values a logger writes, so they become gaps instead of failing the parse as non-numbers."),
        )
        .param(
            Param::integer("decimals")
                .default(6)
                .min(0.0)
                .max(12.0)
                .describe("Maximum decimal places for the values this tool COMPUTES, 0-12 (default 6). Trailing zeros are trimmed, so a computed 2.50 prints as 2.5 and 17.500000 as 17.5. Set 0 for whole-number series like counts. Known values you typed are echoed back verbatim and are never reformatted or rounded."),
        )
        .param(
            Param::enumv("output", ["values", "csv", "json"])
                .default("values")
                .describe("Result shape: 'values' (default) prints the filled series one value per line — or 'x,y' per line in x,y layout — ready to paste straight back into a spreadsheet column; 'csv' adds a header row and a per-row status column of known/filled/missing so you can see which numbers are yours; 'json' returns the counts, the numeric array (an unfilled gap is null) and a per-gap report giving each run's start, end, length, kind (leading/interior/trailing), how many cells were filled and why the rest were not."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/linear-interpolate-gaps",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Fill the blanks in a numeric series by linear interpolation between the nearest known values.",
    skill(
        description = "Fill the missing values in an ordered numeric series by drawing a straight line between the two nearest known neighbours, the way a spreadsheet's fill-series or a dataframe's interpolate() does. Paste a column of readings with blanks or NA markers in it and get the same column back with the gaps filled and everything you typed echoed verbatim. Two-column rows are read as 'x,y' pairs, so a sensor that logged at 0, 5 and 20 minutes is interpolated against the real spacing rather than against row numbers — the difference between a right answer and a plausible one. layout picks that reading ('auto' (default), 'values', 'xy'); max_gap caps how many consecutive blanks may be filled, so a two-month hole is left honest instead of invented (0 = no limit); direction ('both' (default), 'forward', 'backward') decides which side a clipped run is filled from and whether the edge gaps are eligible; edges says what happens outside the known range, where there is no second anchor — 'leave' (default) keeps those blank, 'hold' repeats the nearest known value, 'extrapolate' extends the end slope; na_tokens adds your own missing markers such as '-999' or 'missing'; decimals rounds only the computed values (0-12, default 6, trailing zeros trimmed); output returns the plain series, a CSV with a known/filled/missing status per row, or JSON with counts plus a per-gap report of every run it filled or refused. Errors are specific: a non-numeric value names the position, out-of-order x values name the row. Runs entirely in the sandbox; nothing is uploaded.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "linear-interpolate-gaps", |a: Args| {
            let opts = gizza_ai_linear_interpolate_gaps_core::Options {
                layout: a.layout,
                max_gap: a.max_gap.clamp(0, 100_000) as usize,
                direction: a.direction,
                edges: a.edges,
                na_tokens: a.na_tokens,
                decimals: a.decimals.clamp(0, 12) as usize,
                output: a.output,
            };
            gizza_ai_linear_interpolate_gaps_core::interpolate_gaps(&a.input, &opts)
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

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional and
    /// reviewed. Authored 2026-08-20 for the initial linear-interpolate-gaps release.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "The series to fill, as text. One value per line, or separated by commas, semicolons, tabs or spaces. A blank field or an NA marker (empty, na, n/a, nan, null, none, nil, -, --, ?) is a gap; everything else must be a number. Rows of exactly two fields are read as 'x,y' pairs so uneven spacing is respected. A leading header row or label is skipped automatically. Max 100,000 values." },
                    "layout": { "type": "string", "enum": ["auto", "values", "xy"], "default": "auto", "description": "How the input is read: 'auto' (default) treats the data as x,y pairs when there are 2+ rows and EVERY row has exactly two fields, otherwise as a plain list of y values; 'values' forces the plain list, where the positions 1,2,3… act as the x axis and every step is equal; 'xy' forces x,y pairs and errors on any row that has not got exactly two fields. In x,y layout the x column must be numeric and strictly increasing — only y may be missing." },
                    "max_gap": { "type": "integer", "default": 0, "minimum": 0, "maximum": 100000, "description": "Longest run of consecutive missing values that may be filled: 3 fills runs of 1-3 blanks and leaves a run of 4 alone, which is how you avoid inventing a week of data from two months apart. 0 (default) means no limit. A run longer than the limit is not skipped entirely under direction 'both' — it is filled max_gap values in from EACH side and left blank in the middle; use 'forward' or 'backward' to fill from one side only." },
                    "direction": { "type": "string", "enum": ["both", "forward", "backward"], "default": "both", "description": "Which side(s) a gap may be filled from: 'both' (default), 'forward' only after a known value (the ffill direction), 'backward' only before one. On interior gaps with no max_gap all three give the same straight line — direction starts to matter once max_gap clips a run, and it also decides whether the leading gap (needs backward) and the trailing gap (needs forward) are eligible at all." },
                    "edges": { "type": "string", "enum": ["leave", "hold", "extrapolate"], "default": "leave", "description": "What to do with the gaps OUTSIDE the known range — the blanks before the first known value and after the last, where there is no second anchor to draw a line to: 'leave' (default) keeps them empty, because strict interpolation never invents data past the anchors; 'hold' repeats the nearest known value (the classic ffill/bfill); 'extrapolate' extends the straight line through the two nearest known points, which can go negative. With a single known value 'extrapolate' degrades to 'hold' — one point has no slope." },
                    "na_tokens": { "type": "string", "default": "", "description": "Extra tokens that also count as missing, comma-separated and case-insensitive, e.g. 'missing,unknown,-999,#N/A'. The built-in markers (empty, na, n/a, nan, null, none, nil, -, --, ?) are always recognised; this list adds to them rather than replacing them. Use it for sentinel values a logger writes, so they become gaps instead of failing the parse as non-numbers." },
                    "decimals": { "type": "integer", "default": 6, "minimum": 0, "maximum": 12, "description": "Maximum decimal places for the values this tool COMPUTES, 0-12 (default 6). Trailing zeros are trimmed, so a computed 2.50 prints as 2.5 and 17.500000 as 17.5. Set 0 for whole-number series like counts. Known values you typed are echoed back verbatim and are never reformatted or rounded." },
                    "output": { "type": "string", "enum": ["values", "csv", "json"], "default": "values", "description": "Result shape: 'values' (default) prints the filled series one value per line — or 'x,y' per line in x,y layout — ready to paste straight back into a spreadsheet column; 'csv' adds a header row and a per-row status column of known/filled/missing so you can see which numbers are yours; 'json' returns the counts, the numeric array (an unfilled gap is null) and a per-gap report giving each run's start, end, length, kind (leading/interior/trailing), how many cells were filled and why the rest were not." }
                },
                "required": ["input"],
                "additionalProperties": false
            }"#,
        )
        .expect("authored schema is valid JSON");
        let derived: serde_json::Value =
            serde_json::from_str(&schema_json()).expect("derived schema is valid JSON");
        assert_eq!(derived, authored);
    }

    #[test]
    fn descriptor_describes_every_param() {
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = derived["properties"].as_object().unwrap();
        assert_eq!(props.len(), 8);
        for (name, prop) in props {
            let d = prop["description"].as_str().unwrap_or("");
            assert!(d.len() > 20, "param '{name}' needs a real .describe()");
        }
    }
}
