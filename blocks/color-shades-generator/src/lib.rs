//! gizza-ai/color-shades-generator — produce a ramp of tints, shades and tones
//! for a base color (incl. a Tailwind-style 50-900 scale). Thin wrapper; chat
//! schema single-sourced from descriptor() (which also drives the CLI); handle()
//! delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_color_shades_generator_core::generate;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    color: String,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default = "default_count")]
    count: f64,
}

fn default_mode() -> String {
    "scale".to_string()
}
fn default_count() -> f64 {
    9.0
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("color")
                .required()
                .describe("The base color in any common notation: #hex (3/6 digits), rgb(), or hsl()."),
        )
        .param(
            Param::enumv("mode", ["scale", "tints", "shades", "tones"])
                .default("scale")
                .describe("scale = Tailwind-style 50-900 ramp; tints = lighten toward white; shades = darken toward black; tones = desaturate toward gray."),
        )
        .param(
            Param::integer("count")
                .default(9)
                .min(2.0)
                .max(12.0)
                .describe("How many steps for the tints/shades/tones series modes. Ignored by 'scale' (always 11 weights)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ColorShadesGenerator;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/color-shades-generator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate a ramp of tints, shades and tones for a base color",
    skill(
        description = "Generate a ramp of tints, shades and tones for a base color. Accepts a base color (#hex, rgb(), or hsl()) and a mode: scale = Tailwind-style 50-950 named scale, tints = lighten toward white, shades = darken toward black, tones = desaturate toward gray. Returns each step as hex, rgb and hsl. Runs locally.",
        parameters = schema_json()
    ),
)]
impl ColorShadesGenerator {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "color-shades-generator", |a: Args| {
            generate(&a.color, &a.mode, a.count as usize).map_err(SkillError::InvalidArgs)
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
                    "mode": {
                        "type": "string",
                        "enum": ["scale","tints","shades","tones"],
                        "default": "scale",
                        "description": "scale = Tailwind-style 50-900 ramp; tints = lighten toward white; shades = darken toward black; tones = desaturate toward gray."
                    },
                    "count": {
                        "type": "integer",
                        "default": 9,
                        "minimum": 2,
                        "maximum": 12,
                        "description": "How many steps for the tints/shades/tones series modes. Ignored by 'scale' (always 11 weights)."
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
