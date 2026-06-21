//! gizza-ai/color-format-convert — convert a color between HEX, RGB(A), HSL, HSV
//! and CMYK. Thin wrapper; chat schema single-sourced from descriptor(); handler
//! delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_color_format_convert_core::convert;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    color: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None).param(
        Param::string("color")
            .required()
            .describe("A color in any common notation: #hex (3/4/6/8 digits), rgb()/rgba(), or hsl()/hsla()."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ColorFormatConvert;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/color-format-convert",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a color between HEX/RGB/HSL/HSV/CMYK",
    skill(
        description = "Convert a color between formats. Accepts #hex (3/4/6/8 digits), rgb()/rgba(), or hsl()/hsla(), and returns the equivalent hex, rgb, rgba, hsl, hsv, and cmyk representations plus the alpha. Runs locally.",
        parameters = schema_json()
    ),
)]
impl ColorFormatConvert {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "color-format-convert", |a: Args| {
            convert(&a.color).map_err(SkillError::InvalidArgs)
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
                    "color": { "type": "string", "description": "A color in any common notation: #hex (3/4/6/8 digits), rgb()/rgba(), or hsl()/hsla()." }
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
