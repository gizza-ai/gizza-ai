//! gizza-ai/scientific-calculator — scientific/complex expression evaluator.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    expression: String,
    #[serde(default)]
    variables: String,
    #[serde(default = "default_angle_unit")]
    angle_unit: String,
    #[serde(default = "default_precision")]
    precision: usize,
    #[serde(default = "default_notation")]
    notation: String,
    #[serde(default = "default_complex_form")]
    complex_form: String,
    #[serde(default)]
    group_digits: bool,
    #[serde(default = "default_output_format")]
    output_format: String,
}

fn default_angle_unit() -> String {
    "radians".into()
}
fn default_precision() -> usize {
    12
}
fn default_notation() -> String {
    "auto".into()
}
fn default_complex_form() -> String {
    "rectangular".into()
}
fn default_output_format() -> String {
    "text".into()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("expression").required().describe("Expression or worksheet to evaluate. Use one expression per line or separate statements with semicolons; assignments like 'x = 2pi' and 'ans' are supported."))
        .param(Param::string("variables").describe("Optional variable assignments evaluated before the expression, one per line, such as 'r = 5' or 'theta = pi/4'."))
        .param(Param::enumv("angle_unit", ["radians", "degrees", "gradians"]).default("radians").describe("Angle unit for trigonometric functions and inverse-trig results."))
        .param(Param::integer("precision").default(12).min(1.0).max(200.0).describe("Significant digits to display. Values above 15 preserve interface compatibility but calculations use double precision."))
        .param(Param::enumv("notation", ["auto", "fixed", "scientific", "engineering"]).default("auto").describe("How real and complex components are formatted."))
        .param(Param::enumv("complex_form", ["rectangular", "polar"]).default("rectangular").describe("Display complex results as a + bi or magnitude-angle form."))
        .param(Param::boolean("group_digits").default(false).describe("Add thousands separators to formatted real numbers."))
        .param(Param::enumv("output_format", ["text", "json"]).default("text").describe("Return plain text for humans or JSON with numeric real/imaginary fields."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn run_args(a: Args) -> Result<String, String> {
    let opts = gizza_ai_scientific_calculator_core::Options {
        expression: a.expression,
        variables: a.variables,
        angle_unit: gizza_ai_scientific_calculator_core::AngleUnit::parse(&a.angle_unit)?,
        precision: a.precision,
        notation: gizza_ai_scientific_calculator_core::Notation::parse(&a.notation)?,
        complex_form: gizza_ai_scientific_calculator_core::ComplexForm::parse(&a.complex_form)?,
        group_digits: a.group_digits,
        output_format: gizza_ai_scientific_calculator_core::OutputFormat::parse(&a.output_format)?,
    };
    gizza_ai_scientific_calculator_core::evaluate(opts)
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/scientific-calculator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Evaluate scientific, complex, and worksheet-style math expressions.",
    skill(
        description = "Evaluate scientific calculator expressions with functions, constants, variables, complex numbers, angle modes, and text or JSON output.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "scientific-calculator", |a: Args| {
            run_args(a).map_err(SkillError::InvalidArgs)
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
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived["type"], "object");
        assert!(derived["required"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("expression")));
        for name in [
            "expression",
            "variables",
            "angle_unit",
            "precision",
            "notation",
            "complex_form",
            "group_digits",
            "output_format",
        ] {
            assert!(
                derived["properties"][name]["description"].is_string(),
                "{name} missing description"
            );
        }
        assert_eq!(derived["additionalProperties"], false);
    }
}
