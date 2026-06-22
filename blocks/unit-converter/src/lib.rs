//! gizza-ai/unit-converter — chat skill block on the shared tool abstraction.
//! Converts a numeric value between two units of the same physical category
//! (length, mass, temperature, volume, area, speed, data size, time). The chat
//! schema is single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to block_utils::run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_unit_converter_core::convert_to_string;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    value: f64,
    from: String,
    to: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::number("value")
                .required()
                .describe("The numeric quantity to convert."),
        )
        .param(
            Param::string("from")
                .required()
                .describe("Source unit, e.g. km, miles, lb, celsius, gallon, mph, GB, hours. Names and symbols/aliases accepted."),
        )
        .param(
            Param::string("to")
                .required()
                .describe("Target unit (must be the same category as 'from', e.g. both length, both temperature)."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/unit-converter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert between units of measure",
    skill(
        description = "Convert a numeric value between two units of the same category. Supports length (m, km, cm, mm, mile, yard, foot, inch, nautical-mile), mass (kg, g, mg, tonne, pound, ounce, stone, ton), temperature (celsius, fahrenheit, kelvin, rankine, reaumur), volume (litre, ml, gallon-us/uk, quart, pint, cup, fluid-ounce, tbsp, tsp), area (m2, km2, hectare, acre, sq-mile/yard/foot/inch), speed (m/s, km/h, mph, ft/s, knot), data size (byte, bit, KB/MB/GB/TB decimal and KiB/MiB/GiB/TiB binary), and time (s, ms, min, hour, day, week, month, year). Unit names, plurals, and symbols/aliases are accepted. 'from' and 'to' must be the same category. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "unit-converter", |a: Args| {
            convert_to_string(a.value, &a.from, &a.to).map_err(SkillError::InvalidArgs)
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
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "value": { "type": "number", "description": "The numeric quantity to convert." },
                    "from":  { "type": "string", "description": "Source unit, e.g. km, miles, lb, celsius, gallon, mph, GB, hours. Names and symbols/aliases accepted." },
                    "to":    { "type": "string", "description": "Target unit (must be the same category as 'from', e.g. both length, both temperature)." }
                },
                "required": ["value", "from", "to"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
