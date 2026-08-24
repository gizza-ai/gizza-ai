//! gizza-ai/percent-decimal-converter — chat skill block on the shared tool abstraction.
//! Converts a whole column (or a bare one-value-per-line list) between percentage
//! strings and decimal fractions, in either direction or auto-detected per cell.
//! The chat schema is single-sourced from `descriptor()` (which also drives the
//! CLI); `handle()` delegates to `block_utils::run_skill`. Pure compute — the text
//! is parsed and rewritten in the sandbox, nothing is uploaded.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default)]
    direction: String,
    #[serde(default)]
    unit: String,
    #[serde(default)]
    columns: String,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default)]
    delimiter: String,
    #[serde(default = "default_decimals")]
    decimals: i64,
    #[serde(default)]
    trim_zeros: bool,
    #[serde(default = "default_true")]
    suffix: bool,
}

fn default_true() -> bool {
    true
}
fn default_decimals() -> i64 {
    -1
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("The text to convert: a delimited table, or the commonest case, a bare list with one value per line pasted out of a spreadsheet column. Row order, column count and delimiter are preserved, and CRLF line endings are accepted. Cells that are not plain numbers — text, blanks, currency like $12.99, scientific notation like 1e5 — are copied through untouched rather than mangled. Max 5,000,000 bytes."),
        )
        .param(
            Param::enumv(
                "direction",
                ["auto", "percent_to_decimal", "decimal_to_percent"],
            )
            .default("auto")
            .describe("Which way to convert. 'percent_to_decimal' divides by the unit scale, so 12.5% becomes 0.125. 'decimal_to_percent' multiplies, so 0.125 becomes 12.5%. 'auto' (the default) decides per cell from the value itself: a cell carrying a %, ‰, bp or bps marker is already on the percent side and is brought DOWN to a fraction, while a bare number is taken UP to the percent side — so a mixed column sorts itself out in one pass."),
        )
        .param(
            Param::enumv("unit", ["percent", "permille", "basis_points"])
                .default("percent")
                .describe("The unit on the percent side of the conversion, i.e. how far the decimal point moves. 'percent' (default) is two places, 1 = 100%. 'permille' is three, 1 = 1000‰. 'basis_points' is four, 1 = 10000 bps, the finance convention where a 0.25% rate change is 25 bps. The matching symbol is what 'suffix' appends, and what 'auto' looks for on input."),
        )
        .param(
            Param::string("columns")
                .default("")
                .describe("Which columns to convert. Blank (the default) means every column, which is what a one-column list wants. Otherwise a comma-separated list of 1-based indices ('2'), header names ('rate') when header is on, or a mix ('rate,4'). Cells in unselected columns are copied through unchanged, so an id or date column beside the rates stays intact."),
        )
        .param(
            Param::boolean("header")
                .default(true)
                .describe("When true (the default) row 1 is a header: it is never converted, so a column titled 2024 stays 2024 instead of turning into 202400%, and its names can be used in 'columns'. Turn it off for a headerless table or a bare list of numbers — every row is then data and columns must be given as indices."),
        )
        .param(
            Param::string("delimiter")
                .default("comma")
                .describe("Field separator for both reading and writing: a name ('comma' — the default, 'tab', 'semicolon', 'pipe') or any single character. A one-value-per-line list has no separator at all, so the default is fine. Use 'semicolon' for a European export where the comma is the decimal mark."),
        )
        .param(
            Param::integer("decimals")
                .default(-1)
                .min(-1.0)
                .max(12.0)
                .describe("Precision: how many digits to keep after the decimal point, 0 to 12, padding with zeros so a column lines up. -1 (the default) is exact mode — the full converted value with no padding and no rounding, which is always possible here because the conversion shifts the decimal point rather than multiplying a float. Rounding is half-away-from-zero, on the digit string, so 0.005 to 0 places is 1%."),
        )
        .param(
            Param::boolean("trim_zeros")
                .default(false)
                .describe("Drop trailing fractional zeros after rounding, which turns 'decimals' into an AT MOST width rather than an exact one: with decimals=4, 50% becomes 0.5 instead of 0.5000, while 33.3333% still becomes 0.3333. Off by default, since padded columns line up. Ignored in exact mode (decimals=-1), which never pads in the first place."),
        )
        .param(
            Param::boolean("suffix")
                .default(true)
                .describe("Append the unit symbol — %, ‰ or ' bps' — when a value lands on the percent side. On by default, giving 12.5%. Turn it off to get the bare number 12.5, which is what you want when the column heading already says percent or the result feeds another tool. Values converted TO a decimal fraction never get a symbol either way."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/percent-decimal-converter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a whole column or list between percentages and decimal fractions, either way.",
    skill(
        description = "Convert a column of percentages to decimal fractions, or decimals to percentages, in one pass over a whole table or a bare one-value-per-line list. direction is 'percent_to_decimal' (12.5% becomes 0.125), 'decimal_to_percent' (0.125 becomes 12.5%), or 'auto' (the default), which decides PER CELL from the marker on the value — anything ending in %, ‰, bp or bps is brought down to a fraction and a bare number is taken up to the percent side, so a mixed column sorts itself out. unit sets how far the point moves: percent (two places), permille (three) or basis_points (four, so 0.25% is 25 bps). The conversion SHIFTS THE DECIMAL POINT on the digits that were typed rather than multiplying a binary float, so 0.1 becomes exactly 10% and not 10.000000000000002%, and 12.345% becomes exactly 0.12345. decimals fixes the precision from 0 to 12 places with zero padding, or -1 (the default) for the exact value; trim_zeros makes that an at-most width by dropping padding; suffix (on by default) appends the %, ‰ or bps symbol. columns is blank for every column or a mix of header names and 1-based indices, header keeps row 1 out of the conversion, and delimiter is comma/tab/semicolon/pipe or any single character. Cells that are not plain numbers — text, blanks, currency, scientific notation — pass through untouched, while surrounding whitespace, non-breaking spaces, a space before the symbol, thousands separators inside a quoted field, and upper- or lower-case bp/bps are all tolerated. Max 5,000,000 bytes. Runs entirely in the sandbox; nothing is uploaded.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "percent-decimal-converter", |a: Args| {
            gizza_ai_percent_decimal_converter_core::convert_csv(
                &a.data,
                &a.direction,
                &a.unit,
                &a.columns,
                a.header,
                &a.delimiter,
                a.decimals,
                a.trim_zeros,
                a.suffix,
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

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional and
    /// reviewed. Authored 2026-08-23 for the initial percent-decimal-converter release.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "data": { "type": "string", "description": "The text to convert: a delimited table, or the commonest case, a bare list with one value per line pasted out of a spreadsheet column. Row order, column count and delimiter are preserved, and CRLF line endings are accepted. Cells that are not plain numbers — text, blanks, currency like $12.99, scientific notation like 1e5 — are copied through untouched rather than mangled. Max 5,000,000 bytes." },
                    "direction": { "type": "string", "enum": ["auto", "percent_to_decimal", "decimal_to_percent"], "default": "auto", "description": "Which way to convert. 'percent_to_decimal' divides by the unit scale, so 12.5% becomes 0.125. 'decimal_to_percent' multiplies, so 0.125 becomes 12.5%. 'auto' (the default) decides per cell from the value itself: a cell carrying a %, ‰, bp or bps marker is already on the percent side and is brought DOWN to a fraction, while a bare number is taken UP to the percent side — so a mixed column sorts itself out in one pass." },
                    "unit": { "type": "string", "enum": ["percent", "permille", "basis_points"], "default": "percent", "description": "The unit on the percent side of the conversion, i.e. how far the decimal point moves. 'percent' (default) is two places, 1 = 100%. 'permille' is three, 1 = 1000‰. 'basis_points' is four, 1 = 10000 bps, the finance convention where a 0.25% rate change is 25 bps. The matching symbol is what 'suffix' appends, and what 'auto' looks for on input." },
                    "columns": { "type": "string", "default": "", "description": "Which columns to convert. Blank (the default) means every column, which is what a one-column list wants. Otherwise a comma-separated list of 1-based indices ('2'), header names ('rate') when header is on, or a mix ('rate,4'). Cells in unselected columns are copied through unchanged, so an id or date column beside the rates stays intact." },
                    "header": { "type": "boolean", "default": true, "description": "When true (the default) row 1 is a header: it is never converted, so a column titled 2024 stays 2024 instead of turning into 202400%, and its names can be used in 'columns'. Turn it off for a headerless table or a bare list of numbers — every row is then data and columns must be given as indices." },
                    "delimiter": { "type": "string", "default": "comma", "description": "Field separator for both reading and writing: a name ('comma' — the default, 'tab', 'semicolon', 'pipe') or any single character. A one-value-per-line list has no separator at all, so the default is fine. Use 'semicolon' for a European export where the comma is the decimal mark." },
                    "decimals": { "type": "integer", "minimum": -1, "maximum": 12, "default": -1, "description": "Precision: how many digits to keep after the decimal point, 0 to 12, padding with zeros so a column lines up. -1 (the default) is exact mode — the full converted value with no padding and no rounding, which is always possible here because the conversion shifts the decimal point rather than multiplying a float. Rounding is half-away-from-zero, on the digit string, so 0.005 to 0 places is 1%." },
                    "trim_zeros": { "type": "boolean", "default": false, "description": "Drop trailing fractional zeros after rounding, which turns 'decimals' into an AT MOST width rather than an exact one: with decimals=4, 50% becomes 0.5 instead of 0.5000, while 33.3333% still becomes 0.3333. Off by default, since padded columns line up. Ignored in exact mode (decimals=-1), which never pads in the first place." },
                    "suffix": { "type": "boolean", "default": true, "description": "Append the unit symbol — %, ‰ or ' bps' — when a value lands on the percent side. On by default, giving 12.5%. Turn it off to get the bare number 12.5, which is what you want when the column heading already says percent or the result feeds another tool. Values converted TO a decimal fraction never get a symbol either way." }
                },
                "required": ["data"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
