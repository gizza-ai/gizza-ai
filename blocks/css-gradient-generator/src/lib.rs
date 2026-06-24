//! gizza-ai/css-gradient-generator — chat skill block on the shared tool
//! abstraction. The chat schema is single-sourced from descriptor() (which also
//! drives the CLI); handle() delegates to block_utils::run_skill. No host calls
//! — runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    colors: String,
    #[serde(default)]
    gradient_type: String,
    /// Linear direction / conic start angle in degrees. f64 (wasm BigInt gotcha).
    #[serde(default)]
    angle: f64,
    #[serde(default)]
    shape: String,
    #[serde(default)]
    repeating: bool,
    #[serde(default)]
    interpolation: String,
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("colors")
                .required()
                .describe("The color stops, separated by commas or newlines. Each is a CSS color (#f00, #ff0000, #ff000080, red, rgb(255,0,0), hsl(...)) optionally followed by a position (#f00 25%, red 0px, blue 100). A bare number is read as a percentage. With 2+ colors and no explicit positions, evenly-spaced percentages are filled in. At least 2 stops are required."),
        )
        .param(
            Param::enumv("gradient_type", ["linear", "radial", "conic"])
                .default("linear")
                .describe("The gradient function: 'linear' (default), 'radial', or 'conic'."),
        )
        .param(
            Param::number("angle")
                .default(180.0)
                .min(-360.0)
                .max(360.0)
                .describe("For 'linear', the direction in degrees (0 = up, 90 = to the right, 180 = down — the default). For 'conic', the starting angle ('from <angle>deg'). Ignored for 'radial'."),
        )
        .param(
            Param::enumv("shape", ["ellipse", "circle"])
                .default("ellipse")
                .describe("For 'radial' only: the gradient shape, 'ellipse' (default) or 'circle'. Ignored for linear/conic."),
        )
        .param(
            Param::boolean("repeating")
                .default(false)
                .describe("When true, emit a repeating-*-gradient() (tile the color stops). Most useful with explicit pixel positions, e.g. stripes. Default false."),
        )
        .param(
            Param::string("interpolation")
                .describe("Optional color-interpolation color space for a perceptually-smoother blend, prepended as an 'in <space>' clause (e.g. linear-gradient(in oklch 90deg, ...)). Use a space such as 'srgb', 'oklab', 'oklch', 'hsl', 'lch', 'display-p3'. For the polar spaces (hsl/hwb/lch/oklch) you may append a hue method: 'oklch longer', 'hsl increasing' (shorter|longer|increasing|decreasing). Blank (default) = no clause."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/css-gradient-generator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "CSS Gradient Generator skill",
    skill(
        description = "Generate a CSS gradient declaration from color stops. Returns a ready-to-paste `background-image: linear-gradient(...);` (or radial/conic) line. Provide the `colors` as a comma- or newline-separated list of CSS colors, optionally with positions (e.g. '#ff0000, #0000ff' or '#fff 0%, #000 100%'); with no positions, evenly-spaced percentages are added. Set gradient_type='linear' (default), 'radial', or 'conic'. For linear, `angle` is the direction in degrees (90 = left-to-right, 180 = top-to-bottom default); for conic it's the start angle. For radial, `shape` is 'ellipse' (default) or 'circle'. Set repeating=true for a repeating-*-gradient (stripes/tiles).",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "css-gradient-generator", |a: Args| {
            gizza_ai_css_gradient_generator_core::generate(
                &a.gradient_type,
                a.angle,
                &a.shape,
                &a.colors,
                a.repeating,
                &a.interpolation,
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
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "colors": { "type": "string", "description": "The color stops, separated by commas or newlines. Each is a CSS color (#f00, #ff0000, #ff000080, red, rgb(255,0,0), hsl(...)) optionally followed by a position (#f00 25%, red 0px, blue 100). A bare number is read as a percentage. With 2+ colors and no explicit positions, evenly-spaced percentages are filled in. At least 2 stops are required." },
                    "gradient_type": { "type": "string", "enum": ["linear", "radial", "conic"], "default": "linear", "description": "The gradient function: 'linear' (default), 'radial', or 'conic'." },
                    "angle": { "type": "number", "minimum": -360, "maximum": 360, "default": 180.0, "description": "For 'linear', the direction in degrees (0 = up, 90 = to the right, 180 = down — the default). For 'conic', the starting angle ('from <angle>deg'). Ignored for 'radial'." },
                    "shape": { "type": "string", "enum": ["ellipse", "circle"], "default": "ellipse", "description": "For 'radial' only: the gradient shape, 'ellipse' (default) or 'circle'. Ignored for linear/conic." },
                    "repeating": { "type": "boolean", "default": false, "description": "When true, emit a repeating-*-gradient() (tile the color stops). Most useful with explicit pixel positions, e.g. stripes. Default false." },
                    "interpolation": { "type": "string", "description": "Optional color-interpolation color space for a perceptually-smoother blend, prepended as an 'in <space>' clause (e.g. linear-gradient(in oklch 90deg, ...)). Use a space such as 'srgb', 'oklab', 'oklch', 'hsl', 'lch', 'display-p3'. For the polar spaces (hsl/hwb/lch/oklch) you may append a hue method: 'oklch longer', 'hsl increasing' (shorter|longer|increasing|decreasing). Blank (default) = no clause." }
                },
                "required": ["colors"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
