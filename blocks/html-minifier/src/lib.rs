//! gizza-ai/html-minifier — minify HTML (collapse whitespace, remove comments,
//! normalize tag whitespace). Chat schema single-sourced from descriptor();
//! handler delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_html_minifier_core::minify;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    html: String,
    #[serde(default = "default_true")]
    remove_comments: bool,
}

fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("html").required().describe("The HTML to minify."))
        .param(
            Param::boolean("remove_comments")
                .default(true)
                .describe("Remove <!-- … --> comments (default true)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct HtmlMinifier;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/html-minifier",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Minify HTML (collapse whitespace, strip comments)",
    skill(
        description = "Minify HTML: collapse indentation/whitespace between tags, normalize whitespace inside tags, and (by default) remove comments. Significant inline spacing is preserved, and the verbatim contents of pre/textarea/script/style are kept intact. Set remove_comments=false to keep comments. Runs locally.",
        parameters = schema_json()
    ),
)]
impl HtmlMinifier {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "html-minifier", |a: Args| {
            minify(&a.html, a.remove_comments).map_err(SkillError::InvalidArgs)
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
                    "html":            { "type": "string", "description": "The HTML to minify." },
                    "remove_comments": { "type": "boolean", "default": true, "description": "Remove <!-- … --> comments (default true)." }
                },
                "required": ["html"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
