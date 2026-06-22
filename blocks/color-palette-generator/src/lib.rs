//! gizza-ai/color-palette-generator — generate a harmonious color palette from a
//! base color using classic color-theory schemes. Thin wrapper; chat schema
//! single-sourced from descriptor() (which also drives the CLI); handle()
//! delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_color_palette_generator_core::generate;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    color: String,
    #[serde(default = "default_scheme")]
    scheme: String,
    #[serde(default = "default_count")]
    count: f64,
}

fn default_scheme() -> String {
    "complementary".to_string()
}
fn default_count() -> f64 {
    5.0
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("color")
                .required()
                .describe("The base color in any common notation: #hex (3/6 digits), rgb(), or hsl()."),
        )
        .param(
            Param::enumv(
                "scheme",
                [
                    "complementary",
                    "analogous",
                    "triadic",
                    "split-complementary",
                    "tetradic",
                    "square",
                    "monochromatic",
                    "shades",
                    "tints",
                ],
            )
            .default("complementary")
            .describe("The color-harmony scheme to generate."),
        )
        .param(
            Param::integer("count")
                .default(5)
                .min(2.0)
                .max(12.0)
                .describe("How many colors for series schemes (analogous, monochromatic, shades, tints). Fixed schemes ignore this."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ColorPaletteGenerator;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/color-palette-generator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate a harmonious color palette from a base color",
    skill(
        description = "Generate a harmonious color palette from a base color using color theory. Accepts a base color (#hex, rgb(), or hsl()) and a scheme (complementary, analogous, triadic, split-complementary, tetradic, square, monochromatic, shades, tints), and returns palette swatches in hex, rgb and hsl. Runs locally.",
        parameters = schema_json()
    ),
)]
impl ColorPaletteGenerator {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "color-palette-generator", |a: Args| {
            generate(&a.color, &a.scheme, a.count as usize).map_err(SkillError::InvalidArgs)
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
                    "color": { "type": "string", "description": "The base color in any common notation: #hex (3/6 digits), rgb(), or hsl()." },
                    "scheme": {
                        "type": "string",
                        "enum": ["complementary","analogous","triadic","split-complementary","tetradic","square","monochromatic","shades","tints"],
                        "default": "complementary",
                        "description": "The color-harmony scheme to generate."
                    },
                    "count": {
                        "type": "integer",
                        "default": 5,
                        "minimum": 2,
                        "maximum": 12,
                        "description": "How many colors for series schemes (analogous, monochromatic, shades, tints). Fixed schemes ignore this."
                    }
                },
                "required": ["color"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
