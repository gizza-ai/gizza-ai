//! gizza-ai/js-css-minifier — chat skill block on the shared tool abstraction.
//! Minify JavaScript OR CSS and report the before/after byte sizes. The chat
//! schema is single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_js_css_minifier_core::{minify, Language};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    code: String,
    #[serde(default = "default_auto")]
    language: String,
    #[serde(default = "default_true")]
    remove_comments: bool,
    #[serde(default = "default_true")]
    report: bool,
}

fn default_auto() -> String {
    "auto".into()
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("code")
                .required()
                .describe("The JavaScript or CSS source to minify."),
        )
        .param(
            Param::enumv("language", ["auto", "css", "js"])
                .default("auto")
                .describe("Which language to minify as. 'auto' (default) detects CSS vs JavaScript from the source; set 'css' or 'js' to force it."),
        )
        .param(
            Param::boolean("remove_comments")
                .default(true)
                .describe("Strip comments. License/banner comments (/*! … */, @license, @preserve) are always kept. Default true."),
        )
        .param(
            Param::boolean("report")
                .default(true)
                .describe("Prepend a one-line size-report comment (original vs minified bytes and percent smaller). Default true."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct JsCssMinifier;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/js-css-minifier",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Minify JavaScript or CSS and report before/after byte sizes",
    skill(
        description = "Minify JavaScript or CSS and report the before/after byte sizes. Removes unnecessary whitespace, line breaks and comments to shrink the source without changing behavior. The JavaScript path is token-aware (strings, template literals and regular expressions kept verbatim, identifiers never renamed, ASI-preserving); the CSS path is structural-only (whitespace + comments + redundant separators) and protects strings, url(...) and calc()/parenthesized expressions, so the output is guaranteed equivalent. language (default 'auto') detects CSS vs JS from the source, or force it with 'css'/'js'. remove_comments (default true) strips comments but always keeps /*! … */ / @license / @preserve banners. report (default true) prepends a one-line size-report comment. Runs locally.",
        parameters = schema_json()
    ),
)]
impl JsCssMinifier {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "js-css-minifier", |a: Args| {
            minify(
                &a.code,
                Language::parse(&a.language),
                a.remove_comments,
                a.report,
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
                    "code": { "type": "string", "description": "The JavaScript or CSS source to minify." },
                    "language": { "type": "string", "enum": ["auto", "css", "js"], "default": "auto", "description": "Which language to minify as. 'auto' (default) detects CSS vs JavaScript from the source; set 'css' or 'js' to force it." },
                    "remove_comments": { "type": "boolean", "default": true, "description": "Strip comments. License/banner comments (/*! … */, @license, @preserve) are always kept. Default true." },
                    "report": { "type": "boolean", "default": true, "description": "Prepend a one-line size-report comment (original vs minified bytes and percent smaller). Default true." }
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
