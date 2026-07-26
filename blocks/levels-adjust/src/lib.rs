//! gizza-ai/levels-adjust — numeric Levels adjustment (chat skill block).
//! The descriptor is the single source for chat schema, CLI, and manifest sync;
//! the handler delegates to the pure Rust core.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_levels_adjust_core::summary;
use serde::Deserialize;
use wafer_sdk::*;

fn default_input_black() -> f64 {
    0.0
}
fn default_input_white() -> f64 {
    255.0
}
fn default_gamma() -> f64 {
    1.0
}
fn default_output_black() -> f64 {
    0.0
}
fn default_output_white() -> f64 {
    255.0
}
fn default_true() -> bool {
    true
}

#[derive(Deserialize)]
struct Args {
    values: String,
    #[serde(default = "default_input_black")]
    input_black: f64,
    #[serde(default = "default_input_white")]
    input_white: f64,
    #[serde(default = "default_gamma")]
    gamma: f64,
    #[serde(default = "default_output_black")]
    output_black: f64,
    #[serde(default = "default_output_white")]
    output_white: f64,
    #[serde(default = "default_true")]
    clamp: bool,
}

/// Single source for the chat schema (and CLI/manifest).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("values")
                .required()
                .describe("Tone/sample values to remap, separated by spaces, commas, semicolons, or newlines. Defaults assume an 8-bit 0–255 domain, but any finite numeric domain works when you set the black/white points explicitly."),
        )
        .param(
            Param::number("input_black")
                .default(0.0)
                .min(-1024.0)
                .max(1024.0)
                .describe("Input black point. Values at this level map to output_black; lower values clamp to output_black when clamp=true. Default 0."),
        )
        .param(
            Param::number("input_white")
                .default(255.0)
                .min(-1024.0)
                .max(4096.0)
                .describe("Input white point. Values at this level map to output_white; higher values clamp to output_white when clamp=true. Must differ from input_black. Default 255."),
        )
        .param(
            Param::number("gamma")
                .default(1.0)
                .min(0.1)
                .max(5.0)
                .describe("Midtone gamma. The normalized value is raised to 1/gamma: gamma > 1 brightens midtones, gamma < 1 darkens them, endpoints stay fixed. Must be greater than 0. Default 1."),
        )
        .param(
            Param::number("output_black")
                .default(0.0)
                .min(-1024.0)
                .max(4096.0)
                .describe("Output black level. Use values above 0 to lift blacks; swap output_black/output_white to invert tones. Default 0."),
        )
        .param(
            Param::number("output_white")
                .default(255.0)
                .min(-1024.0)
                .max(4096.0)
                .describe("Output white level. Use values below 255 to lower whites; swap output_black/output_white to invert tones. Default 255."),
        )
        .param(
            Param::boolean("clamp")
                .default(true)
                .describe("When true (default), saturate values outside the input black/white range to the output endpoints. When false, extrapolate past the endpoints."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/levels-adjust",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Remap numeric tone values with input black/white points, midtone gamma, and output levels",
    skill(
        description = "Apply a photographic Levels transfer curve to a list of numeric values. Provide `values` as spaces, commas, semicolons, or newlines. `input_black` and `input_white` define the input range (defaults 0 and 255; they must differ). `gamma` is the midtone control (>0; values above 1 brighten midtones, below 1 darken) and is applied as normalized^(1/gamma). `output_black` and `output_white` define the output range; swapping them inverts tones. `clamp=true` (default) saturates out-of-range values to the output endpoints; `clamp=false` extrapolates. This is pure numeric levels math, not a raster image editor or histogram tool.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "levels-adjust", |a: Args| {
            summary(
                &a.values,
                a.input_black,
                a.input_white,
                a.gamma,
                a.output_black,
                a.output_white,
                a.clamp,
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
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "values": { "type": "string", "description": "Tone/sample values to remap, separated by spaces, commas, semicolons, or newlines. Defaults assume an 8-bit 0–255 domain, but any finite numeric domain works when you set the black/white points explicitly." },
                    "input_black": { "type": "number", "default": 0.0, "minimum": -1024, "maximum": 1024, "description": "Input black point. Values at this level map to output_black; lower values clamp to output_black when clamp=true. Default 0." },
                    "input_white": { "type": "number", "default": 255.0, "minimum": -1024, "maximum": 4096, "description": "Input white point. Values at this level map to output_white; higher values clamp to output_white when clamp=true. Must differ from input_black. Default 255." },
                    "gamma": { "type": "number", "default": 1.0, "minimum": 0.1, "maximum": 5, "description": "Midtone gamma. The normalized value is raised to 1/gamma: gamma > 1 brightens midtones, gamma < 1 darkens them, endpoints stay fixed. Must be greater than 0. Default 1." },
                    "output_black": { "type": "number", "default": 0.0, "minimum": -1024, "maximum": 4096, "description": "Output black level. Use values above 0 to lift blacks; swap output_black/output_white to invert tones. Default 0." },
                    "output_white": { "type": "number", "default": 255.0, "minimum": -1024, "maximum": 4096, "description": "Output white level. Use values below 255 to lower whites; swap output_black/output_white to invert tones. Default 255." },
                    "clamp": { "type": "boolean", "default": true, "description": "When true (default), saturate values outside the input black/white range to the output endpoints. When false, extrapolate past the endpoints." }
                },
                "required": ["values"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
