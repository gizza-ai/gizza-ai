//! gizza-ai/absolute-value-transformer — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_abs")]
    operation: String,
    #[serde(default = "default_auto")]
    separator: String,
    #[serde(default = "default_same")]
    output_separator: String,
    #[serde(default = "default_auto")]
    decimals: String,
    #[serde(default = "default_fail")]
    on_error: String,
    #[serde(default = "default_values")]
    output: String,
    #[serde(default)]
    stats: bool,
}

fn default_abs() -> String {
    "abs".to_string()
}
fn default_auto() -> String {
    "auto".to_string()
}
fn default_same() -> String {
    "same".to_string()
}
fn default_fail() -> String {
    "fail".to_string()
}
fn default_values() -> String {
    "values".to_string()
}

/// `auto` (full precision) or a 0-6 decimal-place count.
fn parse_decimals(s: &str) -> Result<Option<u32>, String> {
    let t = s.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("auto") || t.eq_ignore_ascii_case("none") {
        return Ok(None);
    }
    let n: u32 = t
        .parse()
        .map_err(|_| format!("decimals must be auto or an integer 0-6 (got {t:?})"))?;
    if n > 6 {
        return Err(format!("decimals must be 0-6 (got {n})"));
    }
    Ok(Some(n))
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("The numeric column to transform, one value per line (commas, semicolons, tabs, pipes, and spaces also work). Plain decimals and scientific notation only, e.g. '-3', '4.5', '-1.2e3'. Max 20,000 values."))
        .param(Param::enumv("operation", ["abs", "sign", "negate", "force-negative"]).default("abs").describe("The transform to apply to every value: 'abs' (default) drops the sign so -3 becomes 3; 'sign' returns -1, 0, or 1; 'negate' flips the sign so -3 becomes 3 and 4 becomes -4; 'force-negative' returns -|x| so every value becomes a debit."))
        .param(Param::enumv("separator", ["auto", "newline", "comma", "space", "semicolon", "tab", "pipe"]).default("auto").describe("How the input values are separated. 'auto' (default) splits on any of newline, comma, semicolon, pipe, or whitespace; pick an explicit separator when the data contains the others literally."))
        .param(Param::enumv("output_separator", ["same", "newline", "comma", "space", "semicolon", "tab", "pipe"]).default("same").describe("How to join the transformed values. 'same' (default) mirrors the input separator, so a pasted column comes back as a column and a comma list as a comma list."))
        .param(Param::enumv("decimals", ["auto", "0", "1", "2", "3", "4", "5", "6"]).default("auto").describe("Rounding for the transformed values: 'auto' (default) keeps full precision and prints whole numbers without a trailing .0, or choose 0-6 fixed decimal places."))
        .param(Param::enumv("on_error", ["fail", "skip", "keep", "blank"]).default("fail").describe("What to do with a value that is not a plain number: 'fail' (default) stops and names the offending value; 'skip' drops it; 'keep' echoes the original text; 'blank' emits an empty value so the column stays aligned."))
        .param(Param::enumv("output", ["values", "table", "json"]).default("values").describe("Output shape: 'values' (default) is the transformed column only; 'table' is a TSV audit table with the original next to the result; 'json' is a structured report with counts."))
        .param(Param::boolean("stats").default(false).describe("Append a count/sum/min/max/mean summary of the transformed values (default false)."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/absolute-value-transformer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Apply absolute value, sign extraction, or negation to a numeric column",
    skill(
        description = "Transform the sign of every number in a pasted column. Choose operation abs (|x|), sign (-1/0/1), negate (flip every sign), or force-negative (-|x|). Options: separator auto|newline|comma|space|semicolon|tab|pipe, output_separator same|<separator name>, decimals auto or 0-6, on_error fail|skip|keep|blank, output values|table|json, stats true|false. Values must be plain decimals or scientific notation — clean currency symbols and thousands separators first. Max 20,000 values.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "absolute-value-transformer", |a: Args| {
            let decimals = parse_decimals(&a.decimals).map_err(SkillError::InvalidArgs)?;
            gizza_ai_absolute_value_transformer_core::run(
                &a.data,
                &a.operation,
                &a.separator,
                &a.output_separator,
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
        assert_eq!(v["properties"]["data"]["type"], "string");
        assert_eq!(v["required"], serde_json::json!(["data"]));
        assert_eq!(
            v["properties"]["operation"]["enum"],
            serde_json::json!(["abs", "sign", "negate", "force-negative"])
        );
        assert_eq!(v["properties"]["operation"]["default"], "abs");
        assert_eq!(
            v["properties"]["separator"]["enum"],
            serde_json::json!(["auto", "newline", "comma", "space", "semicolon", "tab", "pipe"])
        );
        assert_eq!(v["properties"]["separator"]["default"], "auto");
        assert_eq!(
            v["properties"]["output_separator"]["enum"],
            serde_json::json!(["same", "newline", "comma", "space", "semicolon", "tab", "pipe"])
        );
        assert_eq!(v["properties"]["output_separator"]["default"], "same");
        assert_eq!(
            v["properties"]["decimals"]["enum"],
            serde_json::json!(["auto", "0", "1", "2", "3", "4", "5", "6"])
        );
        assert_eq!(
            v["properties"]["on_error"]["enum"],
            serde_json::json!(["fail", "skip", "keep", "blank"])
        );
        assert_eq!(v["properties"]["on_error"]["default"], "fail");
        assert_eq!(
            v["properties"]["output"]["enum"],
            serde_json::json!(["values", "table", "json"])
        );
        assert_eq!(v["properties"]["stats"]["default"], false);
    }

    #[test]
    fn decimals_option_parser_accepts_auto_or_choice() {
        assert_eq!(parse_decimals("auto").unwrap(), None);
        assert_eq!(parse_decimals("0").unwrap(), Some(0));
        assert_eq!(parse_decimals("6").unwrap(), Some(6));
        assert!(parse_decimals("7").unwrap_err().contains("0-6"));
        assert!(parse_decimals("two").unwrap_err().contains("auto"));
    }
}
