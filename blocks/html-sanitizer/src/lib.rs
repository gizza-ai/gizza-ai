//! gizza-ai/html-sanitizer — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn default_mode() -> String {
    "safe-html".to_string()
}
fn default_true() -> bool {
    true
}

#[derive(Deserialize)]
struct Args {
    html: String,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default = "default_true")]
    allow_links: bool,
    #[serde(default = "default_true")]
    allow_images: bool,
    #[serde(default)]
    allow_styles: bool,
    #[serde(default = "default_true")]
    keep_classes: bool,
    #[serde(default)]
    keep_comments: bool,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("html")
                .required()
                .describe("The HTML document or snippet to sanitize. Paste markup from an editor, CMS, email, or scraped page; script/style blocks and unsafe tags are stripped before output."),
        )
        .param(
            Param::enumv("mode", ["safe-html", "plain-text"])
                .default("safe-html")
                .describe("Output format: 'safe-html' returns cleaned allowlisted markup (default); 'plain-text' returns visible text after dangerous content is removed."),
        )
        .param(
            Param::boolean("allow_links")
                .default(true)
                .describe("Keep href/src-style URL attributes when they use safe schemes such as http, https, mailto, tel, or relative URLs. Turn off to remove all URL attributes."),
        )
        .param(
            Param::boolean("allow_images")
                .default(true)
                .describe("Keep safe <img> tags and image URLs, including data:image URLs. Turn off to remove images entirely."),
        )
        .param(
            Param::boolean("allow_styles")
                .default(false)
                .describe("Keep inline style attributes only when they do not contain obvious script vectors. <style> blocks are always removed."),
        )
        .param(
            Param::boolean("keep_classes")
                .default(true)
                .describe("Keep class and id attributes for styling hooks. Turn off for lean CMS-ready markup without pasted editor classes or IDs."),
        )
        .param(
            Param::boolean("keep_comments")
                .default(false)
                .describe("Keep HTML comments in safe-html mode. Comments are always removed before plain-text output."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/html-sanitizer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Sanitize pasted HTML into safe markup or plain text",
    skill(
        description = "Sanitize pasted HTML into safe allowlisted markup or plain text. Removes script/style blocks, event handlers, unsafe URL schemes, SVG/math/media/form/embed tags, and disallowed attributes. Choose mode='safe-html' (default) or 'plain-text', and optionally keep or drop links, images, inline styles, classes/IDs, and comments. Runs locally and returns the sanitized result as text.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "html-sanitizer", |a: Args| {
            gizza_ai_html_sanitizer_core::run(
                &a.html,
                &a.mode,
                a.allow_links,
                a.allow_images,
                a.allow_styles,
                a.keep_classes,
                a.keep_comments,
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
        let authored = serde_json::json!({
            "type": "object",
            "properties": {
                "html": { "type": "string", "description": "The HTML document or snippet to sanitize. Paste markup from an editor, CMS, email, or scraped page; script/style blocks and unsafe tags are stripped before output." },
                "mode": { "type": "string", "enum": ["safe-html", "plain-text"], "default": "safe-html", "description": "Output format: 'safe-html' returns cleaned allowlisted markup (default); 'plain-text' returns visible text after dangerous content is removed." },
                "allow_links": { "type": "boolean", "default": true, "description": "Keep href/src-style URL attributes when they use safe schemes such as http, https, mailto, tel, or relative URLs. Turn off to remove all URL attributes." },
                "allow_images": { "type": "boolean", "default": true, "description": "Keep safe <img> tags and image URLs, including data:image URLs. Turn off to remove images entirely." },
                "allow_styles": { "type": "boolean", "default": false, "description": "Keep inline style attributes only when they do not contain obvious script vectors. <style> blocks are always removed." },
                "keep_classes": { "type": "boolean", "default": true, "description": "Keep class and id attributes for styling hooks. Turn off for lean CMS-ready markup without pasted editor classes or IDs." },
                "keep_comments": { "type": "boolean", "default": false, "description": "Keep HTML comments in safe-html mode. Comments are always removed before plain-text output." }
            },
            "required": ["html"],
            "additionalProperties": false
        });
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
