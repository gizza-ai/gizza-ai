//! gizza-ai/decimal-to-fraction — convert decimals to exact or approximate fractions.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_decimal_to_fraction_core::{convert_json, Inputs};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize, Default)]
struct Args {
    #[serde(default)]
    decimal: String,
    repeating_digits: Option<f64>,
    tolerance: Option<f64>,
    max_denominator: Option<f64>,
    denominator: Option<f64>,
    #[serde(default = "d_rounding")]
    rounding: String,
    #[serde(default = "d_reduce")]
    reduce: bool,
}

fn d_rounding() -> String {
    "nearest".into()
}

fn d_reduce() -> bool {
    true
}

impl From<Args> for Inputs {
    fn from(a: Args) -> Self {
        Inputs {
            decimal: a.decimal,
            repeating_digits: a.repeating_digits,
            tolerance: a.tolerance,
            max_denominator: a.max_denominator,
            denominator: a.denominator,
            rounding: a.rounding,
            reduce: Some(a.reduce),
        }
    }
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("decimal").required().describe("Decimal number to convert. Accepts signs, thousands separators, percentages, scientific notation, and repeating notation such as 0.1(6)."))
        .param(Param::number("repeating_digits").default(0.0).describe("Optional count of trailing decimal digits that repeat when the input does not use parentheses or ellipsis notation."))
        .param(Param::number("tolerance").default(0.0).describe("Optional absolute error tolerance. Use 0 for the exact fraction, or values such as 0.000001 to find the simplest nearby fraction."))
        .param(Param::number("max_denominator").default(0.0).describe("Optional maximum denominator for continued-fraction approximation. Use 0 for no cap."))
        .param(Param::number("denominator").default(0.0).describe("Optional fixed denominator to snap onto, such as 16 for nearest sixteenth or 100 for nearest hundredth."))
        .param(Param::enumv("rounding", ["nearest", "up", "down"]).default("nearest").describe("Rounding direction for approximations: nearest, up (not below the decimal), or down (not above the decimal)."))
        .param(Param::boolean("reduce").default(true).describe("Reduce the output fraction to lowest terms. Turn off to keep a fixed denominator such as 8/16."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/decimal-to-fraction",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert decimals to exact or approximate fractions.",
    skill(
        description = "Convert a decimal into an exact fraction, mixed number, or simplest approximation within a tolerance or denominator cap. Supports repeating decimals, percentages, scientific notation, fixed denominators, rounding up/down/nearest, and optional fraction reduction. Returns JSON with numerator, denominator, mixed number, exactness, error, convergents and worked steps.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "decimal-to-fraction", |a: Args| {
            convert_json(&Inputs::from(a)).map_err(SkillError::InvalidArgs)
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
    fn args_defaults_match_descriptor_defaults() {
        let a: Args = serde_json::from_str(r#"{"decimal":"0.625"}"#).unwrap();
        assert_eq!(a.rounding, "nearest");
        assert!(a.reduce);
        assert_eq!(a.max_denominator, None);
    }

    #[test]
    fn args_flow_into_core_inputs() {
        let a: Args = serde_json::from_str(
            r#"{"decimal":"0.31","denominator":16,"rounding":"down","reduce":false}"#,
        )
        .unwrap();
        let i = Inputs::from(a);
        assert_eq!(i.decimal, "0.31");
        assert_eq!(i.denominator, Some(16.0));
        assert_eq!(i.rounding, "down");
        assert_eq!(i.reduce, Some(false));
    }

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let v: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(v["required"], serde_json::json!(["decimal"]));
        assert_eq!(
            v["properties"]["rounding"]["enum"],
            serde_json::json!(["nearest", "up", "down"])
        );
        assert_eq!(v["properties"]["reduce"]["default"], true);
    }
}
