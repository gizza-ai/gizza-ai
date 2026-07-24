//! gizza-ai/html-extract — run a CSS selector over pasted HTML and pull out the
//! text / inner HTML / outer HTML / a named attribute of every match. Thin
//! wrapper around the core; chat schema single-sourced from descriptor();
//! handler delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_html_extract_core::{extract, parse_extract};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    html: String,
    selector: String,
    #[serde(default)]
    extract: String,
    #[serde(default)]
    attr: String,
    #[serde(default = "default_limit")]
    limit: u64,
    #[serde(default = "default_true")]
    trim: bool,
}
fn default_limit() -> u64 {
    100
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("html").required().describe("The raw HTML to search."))
        .param(Param::string("selector").required().describe("A CSS selector, e.g. 'a.link', 'h2', or 'div#main > p'."))
        .param(Param::enumv("extract", ["text", "inner-html", "outer-html", "attr"]).default("text").describe("What to pull from each match: text (visible text, default), inner-html (children as HTML), outer-html (the element itself as HTML), or attr (a named attribute's value)."))
        .param(Param::string("attr").describe("Attribute name to read when extract=attr, e.g. href, src, or class. Required for attr mode; ignored otherwise."))
        .param(Param::integer("limit").default(100).min(1.0).describe("Maximum number of matches to return (default 100, minimum 1)."))
        .param(Param::boolean("trim").default(true).describe("Normalize whitespace: collapse runs of spaces/newlines in text and attributes and trim HTML ends. Default true."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct HtmlExtract;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/html-extract",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract text, HTML, or attributes from HTML by CSS selector",
    skill(
        description = "Run a CSS selector over pasted HTML and extract from every match. Set extract='text' (default, visible text), 'inner-html' (children as HTML), 'outer-html' (the element itself), or 'attr' plus an attr name (e.g. href) to read an attribute. limit caps matches (default 100, min 1); trim (default true) normalizes whitespace. Returns JSON with the match count and an array of values.",
        parameters = schema_json()
    )
)]
impl HtmlExtract {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "html-extract", |a: Args| {
            let mode = parse_extract(&a.extract).map_err(SkillError::InvalidArgs)?;
            extract(
                &a.html,
                &a.selector,
                mode,
                &a.attr,
                a.limit as usize,
                a.trim,
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
                    "html":     { "type": "string", "description": "The raw HTML to search." },
                    "selector": { "type": "string", "description": "A CSS selector, e.g. 'a.link', 'h2', or 'div#main > p'." },
                    "extract":  { "type": "string", "enum": ["text", "inner-html", "outer-html", "attr"], "default": "text", "description": "What to pull from each match: text (visible text, default), inner-html (children as HTML), outer-html (the element itself as HTML), or attr (a named attribute's value)." },
                    "attr":     { "type": "string", "description": "Attribute name to read when extract=attr, e.g. href, src, or class. Required for attr mode; ignored otherwise." },
                    "limit":    { "type": "integer", "minimum": 1, "default": 100, "description": "Maximum number of matches to return (default 100, minimum 1)." },
                    "trim":     { "type": "boolean", "default": true, "description": "Normalize whitespace: collapse runs of spaces/newlines in text and attributes and trim HTML ends. Default true." }
                },
                "required": ["html", "selector"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
