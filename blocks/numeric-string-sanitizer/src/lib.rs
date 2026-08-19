//! gizza-ai/numeric-string-sanitizer — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. The new-tool skill edits
//! descriptor()'s params + core::run to the tool's real inputs/logic.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_auto")]
    decimal_separator: String,
    #[serde(default = "default_strip")]
    percent: String,
    #[serde(default = "default_true")]
    magnitude_suffixes: bool,
    #[serde(default = "default_true")]
    parentheses_negative: bool,
    #[serde(default = "default_auto")]
    decimals: String,
    #[serde(default = "default_blank")]
    on_error: String,
    #[serde(default = "default_values")]
    output: String,
    #[serde(default)]
    stats: bool,
}

fn default_auto() -> String {
    "auto".to_string()
}
fn default_strip() -> String {
    "strip".to_string()
}
fn default_blank() -> String {
    "blank".to_string()
}
fn default_values() -> String {
    "values".to_string()
}
fn default_true() -> bool {
    true
}

fn parse_decimals(s: &str) -> Result<Option<u32>, String> {
    let t = s.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("auto") || t.eq_ignore_ascii_case("none") {
        return Ok(None);
    }
    let n: u32 = t
        .parse()
        .map_err(|_| format!("decimals must be auto or an integer 0-12 (got {t:?})"))?;
    if n > 12 {
        return Err(format!("decimals must be 0-12 (got {n})"));
    }
    Ok(Some(n))
}

/// Single source for the chat schema (and CLI). Edit the params to match the
/// tool's real inputs — e.g. `.param(Param::enumv("mode", ["a","b"]).default("a"))`,
/// `.param(Param::integer("n").min(1.0))`. Use Input::Image/Video/Document/File
/// for tools that take a url/ref media input (see image-resize / web-fetch).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("input").required().describe("Messy numeric cells to sanitize, one value per line. Examples: '$1,234.50 USD', '(250,00) €', '1.2K', '45.2%', or '1 234,56'. Max 20,000 rows."))
        .param(Param::enumv("decimal_separator", ["auto", "dot", "comma"]).default("auto").describe("Decimal convention: auto (infer one convention for the whole column), dot for 1,234.56, or comma for 1.234,56."))
        .param(Param::enumv("percent", ["strip", "divide"]).default("strip").describe("How to handle percent signs: strip keeps 45.2% as 45.2; divide converts it to 0.452."))
        .param(Param::boolean("magnitude_suffixes").default(true).describe("Expand finance suffixes K/M/B/T and bn/tn (default true): 1.2K becomes 1200, 3M becomes 3000000."))
        .param(Param::boolean("parentheses_negative").default(true).describe("Treat accounting parentheses as a negative sign (default true): (250.00) becomes -250."))
        .param(Param::enumv("decimals", ["auto", "0", "1", "2", "3", "4", "5", "6", "8", "10", "12"]).default("auto").describe("Optional rounding: auto keeps full precision, or choose 0-12 decimal places."))
        .param(Param::enumv("on_error", ["blank", "keep", "marker", "fail"]).default("blank").describe("What to emit for rows that cannot be parsed: blank, keep the original text, #ERROR marker, or fail the whole run."))
        .param(Param::enumv("output", ["values", "table", "json"]).default("values").describe("Output shape: cleaned values, a TSV audit table, or structured JSON."))
        .param(Param::boolean("stats").default(false).describe("Append count/sum/min/max/average summary statistics for parsed values."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/numeric-string-sanitizer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Strip currency, separators, units, percents, and whitespace from messy number cells",
    skill(
        description = "Clean messy spreadsheet number cells into machine-readable floats. Paste one value per line; the tool strips currency symbols, thousands separators, units, stray whitespace, accounting parentheses, trailing minus signs, percent signs, and optional K/M/B/T suffixes. Options: decimal_separator auto|dot|comma, percent strip|divide, magnitude_suffixes true|false, parentheses_negative true|false, decimals auto or 0-12, on_error blank|keep|marker|fail, output values|table|json, stats true|false. Max 20,000 rows.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "numeric-string-sanitizer", |a: Args| {
            let decimals = parse_decimals(&a.decimals).map_err(SkillError::InvalidArgs)?;
            gizza_ai_numeric_string_sanitizer_core::run(
                &a.input,
                &a.decimal_separator,
                &a.percent,
                a.magnitude_suffixes,
                a.parentheses_negative,
                decimals,
                &a.on_error,
                &a.output,
                a.stats,
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

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let v: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(v["properties"]["input"]["type"], "string");
        assert_eq!(
            v["properties"]["decimal_separator"]["enum"],
            serde_json::json!(["auto", "dot", "comma"])
        );
        assert_eq!(
            v["properties"]["percent"]["enum"],
            serde_json::json!(["strip", "divide"])
        );
        assert_eq!(v["properties"]["magnitude_suffixes"]["default"], true);
        assert_eq!(v["properties"]["parentheses_negative"]["default"], true);
        assert_eq!(
            v["properties"]["decimals"]["enum"],
            serde_json::json!(["auto", "0", "1", "2", "3", "4", "5", "6", "8", "10", "12"])
        );
        assert_eq!(
            v["properties"]["on_error"]["enum"],
            serde_json::json!(["blank", "keep", "marker", "fail"])
        );
        assert_eq!(
            v["properties"]["output"]["enum"],
            serde_json::json!(["values", "table", "json"])
        );
        assert_eq!(v["properties"]["stats"]["default"], false);
    }

    #[test]
    fn decimal_option_parser_accepts_auto_or_choice() {
        assert_eq!(parse_decimals("auto").unwrap(), None);
        assert_eq!(parse_decimals("2").unwrap(), Some(2));
        assert!(parse_decimals("13").unwrap_err().contains("0-12"));
    }
}
