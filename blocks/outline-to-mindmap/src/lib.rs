//! gizza-ai/outline-to-mindmap — turn an indented text outline into a mind-map
//! SVG. The chat schema is single-sourced from descriptor() (which also drives
//! the CLI); handle() delegates to block_utils::run_skill. Pure-Rust layout +
//! SVG emit, so it runs on every backend including the chat Service Worker. The
//! SVG markup is returned as text (like dot-to-svg / svg-optimize).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_outline_to_mindmap_core::{render, Direction, Options};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    outline: String,
    #[serde(default = "default_direction")]
    direction: String,
    #[serde(default = "default_true")]
    colorful: bool,
    #[serde(default)]
    dark_mode: bool,
    #[serde(default = "default_title")]
    title: String,
}

fn default_direction() -> String {
    "right".to_string()
}
fn default_true() -> bool {
    true
}
fn default_title() -> String {
    "Mind Map".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("outline").required().describe(
                "The indented text outline to map. Each line is a node; deeper indentation \
                 (spaces or tabs) nests it under the line above. Optional leading bullet/number \
                 markers (-, *, +, •, 1.) are stripped. A single top-level line becomes the \
                 central topic; multiple top-level lines hang under a synthetic center.",
            ),
        )
        .param(
            Param::enumv("direction", ["right", "down"]).default("right").describe(
                "Layout direction: 'right' (default) places the center on the left with branches \
                 growing rightward; 'down' places the center on top with branches growing downward.",
            ),
        )
        .param(
            Param::boolean("colorful").default(true).describe(
                "Give each top-level branch (and its descendants) a distinct color (default true). \
                 Set false for a neutral monochrome map.",
            ),
        )
        .param(
            Param::boolean("dark_mode").default(false).describe(
                "Recolor for a dark background: dark canvas with light text (default false).",
            ),
        )
        .param(
            Param::string("title").default("Mind Map").describe(
                "Label for the central node when the outline has more than one top-level item \
                 (default 'Mind Map'). Ignored when the outline already has a single root.",
            ),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn run(a: Args) -> Result<String, String> {
    let opts = Options {
        direction: Direction::parse(&a.direction),
        colorful: a.colorful,
        dark_mode: a.dark_mode,
        title: a.title,
    };
    render(&a.outline, &opts)
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/outline-to-mindmap",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Outline to mind-map SVG",
    skill(
        description = "Turn an indented text outline into a mind-map SVG. Pass `outline` as indented text — each line is a node and deeper indentation (spaces or tabs) nests it under the line above; leading bullet or number markers are stripped. A single top-level line is the central topic; multiple top-level lines hang under a synthetic center (label it with `title`). `direction` is 'right' (center on the left, branches rightward — default) or 'down' (center on top, branches downward). `colorful` colors each top-level branch (default true); `dark_mode` recolors for a dark background. Returns standalone, scalable SVG markup as text. Runs locally on the device.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "outline-to-mindmap", |a: Args| {
            run(a).map_err(SkillError::InvalidArgs)
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
                    "outline": { "type": "string", "description": "The indented text outline to map. Each line is a node; deeper indentation (spaces or tabs) nests it under the line above. Optional leading bullet/number markers (-, *, +, •, 1.) are stripped. A single top-level line becomes the central topic; multiple top-level lines hang under a synthetic center." },
                    "direction": { "type": "string", "enum": ["right", "down"], "default": "right", "description": "Layout direction: 'right' (default) places the center on the left with branches growing rightward; 'down' places the center on top with branches growing downward." },
                    "colorful": { "type": "boolean", "default": true, "description": "Give each top-level branch (and its descendants) a distinct color (default true). Set false for a neutral monochrome map." },
                    "dark_mode": { "type": "boolean", "default": false, "description": "Recolor for a dark background: dark canvas with light text (default false)." },
                    "title": { "type": "string", "default": "Mind Map", "description": "Label for the central node when the outline has more than one top-level item (default 'Mind Map'). Ignored when the outline already has a single root." }
                },
                "required": ["outline"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
