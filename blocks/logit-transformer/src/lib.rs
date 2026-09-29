//! gizza-ai/logit-transformer — chat skill block on the shared tool abstraction.
//! Converts probability columns to log-odds with the logit transform and converts
//! log-odds back to probabilities with the inverse logit / sigmoid.
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
    base: String,
    #[serde(default)]
    separator: String,
    #[serde(default)]
    output_separator: String,
    #[serde(default)]
    on_boundary: String,
    #[serde(default = "default_epsilon")]
    epsilon: f64,
    #[serde(default)]
    decimals: String,
    #[serde(default)]
    output: String,
}

fn default_epsilon() -> f64 {
    0.000001
}

fn parse_decimals(value: &str) -> Result<Option<u32>, String> {
    let v = value.trim();
    if v.is_empty() || v.eq_ignore_ascii_case("auto") {
        return Ok(None);
    }
    let n: u32 = v
        .parse()
        .map_err(|_| format!("decimals must be 'auto' or an integer from 0 to 8 (got {value:?})"))?;
    if n > 8 {
        return Err(format!("decimals must be 'auto' or 0-8 (got {n})"));
    }
    Ok(Some(n))
}

/// Single source for the chat schema, CLI parameters, and page controls.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .multiline()
                .describe("Column of numbers to transform. For direction=logit, values are probabilities in 0-1; a trailing percent sign is accepted, so 90% means 0.9. For direction=inverse, values are log-odds on the whole real line. Paste one value per line, or use comma, space, semicolon, tab, or pipe separators."),
        )
        .param(
            Param::enumv("direction", ["logit", "inverse"])
                .default("logit")
                .describe("Transform direction. 'logit' (default) converts probabilities to log-odds with log(p/(1-p)). 'inverse' converts log-odds back to probabilities with the inverse logit / sigmoid."),
        )
        .param(
            Param::enumv("base", ["e", "2", "10"])
                .default("e")
                .describe("Logarithm base. 'e' (default) is the natural-log scale used by logistic regression; '2' and '10' are available for binary or common-log odds. The inverse direction uses the same base so round trips stay consistent."),
        )
        .param(
            Param::enumv("separator", ["auto", "newline", "comma", "space", "semicolon", "tab", "pipe"])
                .default("auto")
                .describe("How to split the input values. 'auto' (default) accepts newlines, commas, spaces, semicolons, tabs, and pipes; choose an explicit separator when empty fields or exact column shape matter."),
        )
        .param(
            Param::enumv("output_separator", ["same", "newline", "comma", "space", "semicolon", "tab", "pipe"])
                .default("same")
                .describe("How to join the output values. 'same' (default) mirrors the detected or selected input separator; choose newline, comma, space, semicolon, tab, or pipe for a specific downstream format."),
        )
        .param(
            Param::enumv("on_boundary", ["fail", "clamp", "skip", "blank", "infinity"])
                .default("fail")
                .describe("What to do with p=0 or p=1 in the logit direction, where log-odds are undefined. 'fail' names the row and stops. 'clamp' nudges to epsilon and 1-epsilon. 'skip' drops the value. 'blank' preserves row alignment with an empty cell. 'infinity' emits -Infinity or Infinity."),
        )
        .param(
            Param::number("epsilon")
                .default(0.000001)
                .min(0.0)
                .max(0.5)
                .describe("Clamp distance used only when on_boundary=clamp. Must be greater than 0 and less than 0.5. Default 0.000001."),
        )
        .param(
            Param::string("decimals")
                .default("auto")
                .describe("Rounding for numeric output: 'auto' keeps full precision, or enter an integer from 0 to 8 for fixed decimal places. Infinities are printed as Infinity / -Infinity."),
        )
        .param(
            Param::enumv("output", ["values", "table", "json"])
                .default("values")
                .describe("Output shape. 'values' (default) returns only the transformed column. 'table' returns input, odds, and transformed value as tab-separated columns. 'json' returns structured rows with counts and errors."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/logit-transformer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Transform probabilities to log-odds and back",
    skill(
        description = "Apply the logit transform to probability columns and the inverse logit / sigmoid to log-odds. Paste one value or a whole column; percentages such as 90% are accepted in the logit direction. Choose log base e, 2, or 10, mirror or set separators, round to fixed decimals, and decide how p=0/p=1 boundaries should behave: fail, epsilon-clamp, skip, blank, or emit ±Infinity. Table and JSON outputs can show the odds alongside each transformed value.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "logit-transformer", |a: Args| {
            let decimals = parse_decimals(&a.decimals).map_err(SkillError::InvalidArgs)?;
            gizza_ai_logit_transformer_core::run(
                &a.data,
                &a.direction,
                &a.base,
                &a.separator,
                &a.output_separator,
                &a.on_boundary,
                a.epsilon,
                decimals,
                &a.output,
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
    fn schema_has_expected_controls() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = schema["properties"].as_object().unwrap();
        assert!(props["data"]["description"].as_str().unwrap().contains("probabilities"));
        assert_eq!(props["direction"]["enum"], serde_json::json!(["logit", "inverse"]));
        assert_eq!(props["base"]["enum"], serde_json::json!(["e", "2", "10"]));
        assert_eq!(props["on_boundary"]["enum"], serde_json::json!(["fail", "clamp", "skip", "blank", "infinity"]));
        assert_eq!(props["output"]["enum"], serde_json::json!(["values", "table", "json"]));
        assert_eq!(schema["required"], serde_json::json!(["data"]));
    }

    #[test]
    fn decimals_parser_accepts_auto_and_bounds() {
        assert_eq!(parse_decimals("auto").unwrap(), None);
        assert_eq!(parse_decimals("4").unwrap(), Some(4));
        assert!(parse_decimals("9").unwrap_err().contains("0-8"));
    }
}
