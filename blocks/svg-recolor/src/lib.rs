//! gizza-ai/svg-recolor — recolor an SVG icon by remapping its colors, on the
//! shared tool abstraction. The chat schema is single-sourced from descriptor()
//! (which also drives the CLI); handle() delegates to block_utils::run_skill.
//! Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_svg_recolor_core::run as recolor_run;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    svg: String,
    #[serde(default)]
    mapping: String,
    #[serde(default)]
    to: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("svg")
                .required()
                .describe("The SVG markup to recolor."),
        )
        .param(
            Param::string("mapping")
                .describe(
                    "Color remap as 'from=>to' pairs, separated by commas or newlines \
                     (e.g. '#000=>#ffffff, red=>#00ff00'). Source colors match in any \
                     syntax (#fff, #ffffff, rgb(), named). Ignored when 'to' is set.",
                ),
        )
        .param(
            Param::string("to")
                .describe(
                    "Monochrome mode: recolor EVERY color in the SVG to this single \
                     color (e.g. '#3366ff'). Takes precedence over 'mapping'. \
                     'none'/'transparent'/'currentColor' are preserved.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct SvgRecolor;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/svg-recolor",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Recolor an SVG by remapping its fill/stroke colors",
    skill(
        description = "Recolor an SVG icon by remapping its colors. Provide a 'mapping' of 'from=>to' color pairs (comma- or newline-separated, e.g. '#000=>#ffffff, red=>#00ff00') to swap specific colors — matching is format-insensitive, so a source '#ffffff' also matches '#fff' and 'rgb(255,255,255)'. Or set 'to' to a single color to recolor EVERY color in the SVG to that one tint (monochrome icon). Colors are rewritten in fill/stroke/stop-color/color/flood-color/lighting-color/solid-color attributes, inline style= declarations, and inside <style> blocks. Path data, geometry and structure are never changed; 'none', 'transparent' and 'currentColor' are preserved. Runs locally.",
        parameters = schema_json()
    ),
)]
impl SvgRecolor {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "svg-recolor", |a: Args| {
            recolor_run(&a.svg, &a.mapping, &a.to).map_err(SkillError::InvalidArgs)
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
                    "svg":     { "type": "string", "description": "The SVG markup to recolor." },
                    "mapping": { "type": "string", "description": "Color remap as 'from=>to' pairs, separated by commas or newlines (e.g. '#000=>#ffffff, red=>#00ff00'). Source colors match in any syntax (#fff, #ffffff, rgb(), named). Ignored when 'to' is set." },
                    "to":      { "type": "string", "description": "Monochrome mode: recolor EVERY color in the SVG to this single color (e.g. '#3366ff'). Takes precedence over 'mapping'. 'none'/'transparent'/'currentColor' are preserved." }
                },
                "required": ["svg"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
