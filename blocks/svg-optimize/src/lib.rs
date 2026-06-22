//! gizza-ai/svg-optimize — minify and clean up SVG markup (SVGO-style) on the
//! shared tool abstraction. The chat schema is single-sourced from descriptor()
//! (which also drives the CLI); handle() delegates to block_utils::run_skill.
//! Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_svg_optimize_core::{optimize, Options};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    svg: String,
    #[serde(default = "default_true")]
    remove_comments: bool,
    #[serde(default = "default_true")]
    remove_metadata: bool,
    #[serde(default)]
    remove_ids: bool,
    #[serde(default)]
    remove_dimensions: bool,
}

fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("svg").required().describe("The SVG markup to optimize."))
        .param(
            Param::boolean("remove_comments")
                .default(true)
                .describe("Remove <!-- … --> comments (default true)."),
        )
        .param(
            Param::boolean("remove_metadata")
                .default(true)
                .describe(
                    "Strip editor metadata: <metadata>, inkscape:/sodipodi: elements and \
                     attributes, and unused editor xmlns declarations (default true).",
                ),
        )
        .param(
            Param::boolean("remove_ids")
                .default(false)
                .describe(
                    "Remove id and class attributes from every element (default false — \
                     they may be referenced by CSS, JS or <use>).",
                ),
        )
        .param(
            Param::boolean("remove_dimensions")
                .default(false)
                .describe(
                    "Remove width/height from the root <svg> when a viewBox is present so \
                     it scales responsively (default false).",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn build_options(a: &Args) -> Options {
    Options {
        remove_comments: a.remove_comments,
        remove_metadata: a.remove_metadata,
        remove_ids: a.remove_ids,
        remove_dimensions: a.remove_dimensions,
    }
}

#[cfg(target_arch = "wasm32")]
struct SvgOptimize;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/svg-optimize",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Minify and clean up SVG markup (SVGO-style)",
    skill(
        description = "Optimize SVG markup: collapse whitespace and indentation, remove comments, the <?xml?> declaration and <!DOCTYPE>, strip editor metadata (<metadata>, inkscape:/sodipodi: elements + attributes, unused editor xmlns), and normalize tag whitespace. Conservative by default — path data and numbers are never rewritten, so the rendered image is unchanged. Set remove_comments=false to keep comments; remove_ids=true to also drop id/class; remove_dimensions=true to drop root width/height when a viewBox exists. The verbatim contents of <script>, <style> and <text> are preserved. Runs locally.",
        parameters = schema_json()
    ),
)]
impl SvgOptimize {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "svg-optimize", |a: Args| {
            let opts = build_options(&a);
            optimize(&a.svg, &opts).map_err(SkillError::InvalidArgs)
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
                    "svg":               { "type": "string", "description": "The SVG markup to optimize." },
                    "remove_comments":   { "type": "boolean", "default": true, "description": "Remove <!-- … --> comments (default true)." },
                    "remove_metadata":   { "type": "boolean", "default": true, "description": "Strip editor metadata: <metadata>, inkscape:/sodipodi: elements and attributes, and unused editor xmlns declarations (default true)." },
                    "remove_ids":        { "type": "boolean", "default": false, "description": "Remove id and class attributes from every element (default false — they may be referenced by CSS, JS or <use>)." },
                    "remove_dimensions": { "type": "boolean", "default": false, "description": "Remove width/height from the root <svg> when a viewBox is present so it scales responsively (default false)." }
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
