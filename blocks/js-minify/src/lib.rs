//! gizza-ai/js-minify — minify JavaScript by removing unnecessary whitespace,
//! line breaks and comments while preserving behavior. Chat schema single-sourced
//! from descriptor() (which also drives the CLI); handle() delegates to run_skill.
//! Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_js_minify_core::minify_with;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    code: String,
    #[serde(default = "default_true")]
    remove_comments: bool,
    #[serde(default = "default_true")]
    keep_license: bool,
}

fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("code")
                .required()
                .describe("The JavaScript source to minify."),
        )
        .param(
            Param::boolean("remove_comments")
                .default(true)
                .describe("Strip line and block comments. Default true."),
        )
        .param(
            Param::boolean("keep_license")
                .default(true)
                .describe("When removing comments, still preserve license/banner comments (/*! … */, @license, @preserve). Default true."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct JsMinify;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/js-minify",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Minify JavaScript by stripping whitespace and comments",
    skill(
        description = "Minify JavaScript: remove unnecessary whitespace, line breaks and comments to shrink the source, while preserving behavior. It is token-aware — strings, template literals and regular expressions are kept verbatim, identifiers are never renamed, and code is never reordered or dropped, so the minified output is semantically identical to the input. Newlines that matter for automatic semicolon insertion (e.g. after return) are preserved. remove_comments (default true) strips comments; keep_license (default true) still preserves /*! … */ / @license / @preserve banner comments. Runs locally.",
        parameters = schema_json()
    ),
)]
impl JsMinify {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "js-minify", |a: Args| {
            minify_with(&a.code, a.remove_comments, a.keep_license).map_err(SkillError::InvalidArgs)
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
                    "code": { "type": "string", "description": "The JavaScript source to minify." },
                    "remove_comments": { "type": "boolean", "default": true, "description": "Strip line and block comments. Default true." },
                    "keep_license": { "type": "boolean", "default": true, "description": "When removing comments, still preserve license/banner comments (/*! … */, @license, @preserve). Default true." }
                },
                "required": ["code"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
