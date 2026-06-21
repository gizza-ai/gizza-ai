//! gizza-ai/html-preview-bundler — combine HTML, CSS, and JS into one
//! self-contained HTML document. Chat schema single-sourced from descriptor();
//! handler delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_html_preview_bundler_core::bundle;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    html: String,
    #[serde(default)]
    css: String,
    #[serde(default)]
    js: String,
    #[serde(default)]
    title: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("html")
                .required()
                .describe("The HTML — a full document or just a body fragment."),
        )
        .param(Param::string("css").describe("CSS to inline in a <style> tag (optional)."))
        .param(Param::string("js").describe("JavaScript to inline in a <script> tag (optional)."))
        .param(
            Param::string("title")
                .describe("Page <title> used when wrapping a fragment (default \"Preview\")."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct HtmlPreviewBundler;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/html-preview-bundler",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Bundle HTML + CSS + JS into one self-contained file",
    skill(
        description = "Combine separate HTML, CSS, and JavaScript into one self-contained, runnable HTML document you can save, open, or share. If the HTML is a full document the CSS is injected before </head> and the JS before </body>; if it's a fragment it's wrapped in a minimal HTML5 page with the given title. Returns the complete HTML. Runs locally.",
        parameters = schema_json()
    ),
)]
impl HtmlPreviewBundler {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "html-preview-bundler", |a: Args| {
            bundle(&a.html, &a.css, &a.js, &a.title).map_err(SkillError::InvalidArgs)
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
                    "html":  { "type": "string", "description": "The HTML — a full document or just a body fragment." },
                    "css":   { "type": "string", "description": "CSS to inline in a <style> tag (optional)." },
                    "js":    { "type": "string", "description": "JavaScript to inline in a <script> tag (optional)." },
                    "title": { "type": "string", "description": "Page <title> used when wrapping a fragment (default \"Preview\")." }
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
